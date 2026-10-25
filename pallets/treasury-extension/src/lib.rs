//! Treasury Pallet
//!
//! This pallet provides the implementation for a Treasury system within a Substrate-based blockchain.
//! The Treasury is responsible for managing on-chain funds and redistributing them based on proposals, allowing governance to determine the allocation of resources.
//!
//! # Features
//! - Manages funds using a Treasury model, where spending proposals can be submitted and approved by governance participants.
//! - Handles imbalances using the `OnUnbalanced` trait to ensure funds are appropriately managed.
//! - Supports custom configurations for proposal thresholds and Treasury parameters.
//! - Works in conjunction with other pallets like `pallet_treasury` for treasury-specific balance operations.
//!
//! # Structure
//! - Defines core traits such as `Currency`, `Get`, `Imbalance`, and `OnUnbalanced` to handle balance changes and ensure stability.
//! - Uses `Permill` for defining percentage-based parameters, making it flexible for configuring spending limits.
//! - Includes multiple modules such as `mock` for testing, `tests` for unit testing, and `benchmarking` for runtime performance measurement.
//!
//! # Usage
//! - Integrate this pallet in your runtime to enable treasury functionality, allowing for decentralized funding and resource allocation.
//! - Configure treasury-related parameters by implementing runtime configuration traits to suit your chain's specific needs.
//!
//! # Dependencies
//! - Relies on `frame_support` for fundamental utilities like balance management and configuration traits.
//! - Uses `sp_arithmetic` for safe arithmetic operations and ensuring balance-related calculations are handled accurately.
//! - The `weights` module provides weight information to manage the execution costs of the Treasury's dispatchable functions.
//!
//! # Important Notes
//! - The Treasury system is highly reliant on good governance to prevent misuse of funds; make sure governance mechanisms are well defined.
//! - Always validate and benchmark any changes to ensure the system remains performant, especially when modifying spending thresholds or proposal requirements.
//! - Proper testing should be done using both unit tests (`tests.rs`) and mock environments (`mock.rs`) to verify correctness.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    pallet_prelude::*,
    traits::{
        fungible::Inspect,
        tokens::{Fortitude, Preservation},
        Currency, Get, Imbalance, IsType, OnUnbalanced,
    },
};
use pallet_treasury::{BalanceOf, NegativeImbalanceOf, PositiveImbalanceOf};
use sp_arithmetic::{
    traits::{Saturating, Zero},
    FixedPointNumber, FixedU64, Permill,
};
use vitreus_runtime_common::{OnSessionChange, QuotePriceFromEnergy, SessionIndex, SwapFromEnergy};

pub use pallet::*;
pub use weights::WeightInfo;

/// The balance in which the exchanges measure both energy and native currency.
type ExchangeBalanceOf<T, I> =
    <<T as Config<I>>::EnergyAsset as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

type ExchangeAssetOf<T, Exchange> =
    <Exchange as SwapFromEnergy<<T as frame_system::Config>::AccountId>>::EnergyAsset;

/// The energy sold and the native currency received for it.
type SaleOf<T, I> = (ExchangeBalanceOf<T, I>, ExchangeBalanceOf<T, I>);

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// Pallet which burns a fraction of the treasury's balance.
    /// In case if treasury didn't spend more than `T::SpendThreshold`, this pallet burns
    /// leftover funds.
    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_treasury::Config<I> {
        /// Because this pallet emits events, it depends on the runtime definition of an event.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Energy asset.
        type EnergyAsset: Inspect<Self::AccountId>;

        /// Static Energy asset.
        type StaticEnergyAsset: Inspect<Self::AccountId, Balance = ExchangeBalanceOf<Self, I>>;

        /// Liquid Energy asset.
        type LiquidEnergyAsset: Inspect<Self::AccountId, Balance = ExchangeBalanceOf<Self, I>>;

        /// A type used for swapping Energy for native currency.
        type EnergyExchange: QuotePriceFromEnergy<
                Balance = ExchangeBalanceOf<Self, I>,
                EnergyAsset = Self::EnergyAsset,
            > + SwapFromEnergy<
                Self::AccountId,
                Balance = ExchangeBalanceOf<Self, I>,
                EnergyAsset = Self::EnergyAsset,
            >;

        /// A type used for swapping Static Energy for native currency.
        type StaticEnergyExchange: QuotePriceFromEnergy<
                Balance = ExchangeBalanceOf<Self, I>,
                EnergyAsset = Self::StaticEnergyAsset,
            > + SwapFromEnergy<
                Self::AccountId,
                Balance = ExchangeBalanceOf<Self, I>,
                EnergyAsset = Self::StaticEnergyAsset,
            >;

        /// A type used for swapping Liquid Energy for native currency.
        type LiquidEnergyExchange: QuotePriceFromEnergy<
                Balance = ExchangeBalanceOf<Self, I>,
                EnergyAsset = Self::LiquidEnergyAsset,
            > + SwapFromEnergy<
                Self::AccountId,
                Balance = ExchangeBalanceOf<Self, I>,
                EnergyAsset = Self::LiquidEnergyAsset,
            >;

        /// Funds ratio to be recycled.
        type SpendThreshold: Get<Permill>;

        /// What to do with the recycled funds
        type OnRecycled: OnUnbalanced<NegativeImbalanceOf<Self, I>>;

        /// The target balance the treasury aims to maintain.
        #[pallet::constant]
        type TreasuryTargetBalance: Get<BalanceOf<Self, I>>;

        /// The base rate used for fee recycling calculations.
        #[pallet::constant]
        type FeeRecyclingBaseRate: Get<Permill>;

        /// A multiplier adjusting the recycling rate based on deviations from `TreasuryTargetBalance`.
        #[pallet::constant]
        type FeeRecyclingScalingFactor: Get<Permill>;

        /// Weight information for functions in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn fee_recycling_rate)]
    pub type FeeRecyclingRate<T: Config<I>, I: 'static = ()> = StorageValue<_, Permill, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        Recycled {
            recyled_funds: BalanceOf<T, I>,
        },
        /// The treasury sold its energy for native currency.
        EnergyExchanged {
            /// Energy sold.
            energy: ExchangeBalanceOf<T, I>,
            /// Static Energy sold.
            static_energy: ExchangeBalanceOf<T, I>,
            /// Liquid Energy sold.
            liquid_energy: ExchangeBalanceOf<T, I>,
            /// Native currency received for it.
            native: ExchangeBalanceOf<T, I>,
        },
        /// The treasury failed to sell some of its energy for native currency.
        EnergyExchangeFailed {
            /// Why the Energy sale failed, if it did.
            energy: Option<DispatchError>,
            /// Why the Static Energy sale failed, if it did.
            static_energy: Option<DispatchError>,
            /// Why the Liquid Energy sale failed, if it did.
            liquid_energy: Option<DispatchError>,
        },
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    fn treasury_balance() -> BalanceOf<T, I> {
        pallet_treasury::Pallet::<T, I>::pot()
    }

    fn exchange_energy() {
        let account_id = pallet_treasury::Pallet::<T, I>::account_id();

        let energy = Self::sell_energy::<T::EnergyExchange>(&account_id);
        let static_energy = Self::sell_energy::<T::StaticEnergyExchange>(&account_id);
        let liquid_energy = Self::sell_energy::<T::LiquidEnergyExchange>(&account_id);

        if energy.is_err() || static_energy.is_err() || liquid_energy.is_err() {
            Self::deposit_event(Event::EnergyExchangeFailed {
                energy: energy.err(),
                static_energy: static_energy.err(),
                liquid_energy: liquid_energy.err(),
            });
        }

        let mut native = ExchangeBalanceOf::<T, I>::zero();

        let (energy, received) = energy.unwrap_or_default();
        native.saturating_accrue(received);

        let (static_energy, received) = static_energy.unwrap_or_default();
        native.saturating_accrue(received);

        let (liquid_energy, received) = liquid_energy.unwrap_or_default();
        native.saturating_accrue(received);

        if energy.is_zero() && static_energy.is_zero() && liquid_energy.is_zero() {
            return;
        }

        Self::deposit_event(Event::EnergyExchanged {
            energy,
            static_energy,
            liquid_energy,
            native,
        });
    }

    /// Sells the treasury's energy of the exchange's asset down to its minimum balance, but no
    /// more than the exchange can pay for, and returns the amount sold and the native currency
    /// received for it.
    fn sell_energy<Exchange>(account_id: &T::AccountId) -> Result<SaleOf<T, I>, DispatchError>
    where
        Exchange: QuotePriceFromEnergy<Balance = ExchangeBalanceOf<T, I>>
            + SwapFromEnergy<T::AccountId, Balance = ExchangeBalanceOf<T, I>>,
        ExchangeAssetOf<T, Exchange>: Inspect<T::AccountId, Balance = ExchangeBalanceOf<T, I>>,
    {
        let balance = ExchangeAssetOf::<T, Exchange>::reducible_balance(
            account_id,
            Preservation::Preserve,
            Fortitude::Polite,
        );
        let max_amount_out = Exchange::max_amount_out();

        // Nothing to sell, or an exchange with nothing to pay out, is not a failure.
        if balance.is_zero() || max_amount_out.is_zero() {
            return Ok((Zero::zero(), Zero::zero()));
        }

        // A swap for more than `max_amount_out` fails as a whole, so sell at most the input that
        // buys exactly that. The quote leaves the fee out: with a fee the payout is only smaller.
        // Without a quote the exchange can't price the energy, and that is a failure.
        let max_amount_in = Exchange::quote_price_tokens_for_exact_tokens(max_amount_out, false)
            .ok_or(DispatchError::Unavailable)?;
        let mut amount = balance.min(max_amount_in);

        // An exchange that rounds the quoted input up can ask for a unit more than it can pay
        // for, and won't quote that sale: then sell a unit less.
        // TODO: once the exchange traits can tell the largest input an exchange pays for, take it
        // from there instead of stepping back here.
        let mut amount_out = Exchange::quote_price_exact_tokens_for_tokens(amount, false);
        if amount_out.is_none() {
            amount.saturating_dec();
            amount_out = Exchange::quote_price_exact_tokens_for_tokens(amount, false);
        }

        // Rounding can leave a sale that buys nothing: dust at the exchange or in the treasury.
        // A missing quote here means such a sale, not a failure: the energy has a price by now.
        if amount_out.unwrap_or_default().is_zero() {
            return Ok((Zero::zero(), Zero::zero()));
        }

        Exchange::swap_exact_tokens_for_tokens(account_id.clone(), amount, true)
            .map(|received| (amount, received))
    }

    fn update_fee_recycling_rate(total_weight: &mut Weight) {
        let target_balance = T::TreasuryTargetBalance::get();

        let rate = if !target_balance.is_zero() {
            total_weight.saturating_accrue(T::DbWeight::get().reads_writes(3, 0));

            let base_rate = FixedU64::from(T::FeeRecyclingBaseRate::get());
            let scaling_factor = FixedU64::from(T::FeeRecyclingScalingFactor::get());
            let ratio =
                FixedU64::saturating_from_rational(Self::treasury_balance(), target_balance);

            base_rate
                .saturating_add(scaling_factor)
                .saturating_sub(scaling_factor * ratio)
                .into_clamped_perthing()
        } else {
            Permill::zero()
        };

        total_weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
        FeeRecyclingRate::<T, I>::put(rate);
    }
}

impl<T: Config<I>, I: 'static> pallet_treasury::SpendFunds<T, I> for Pallet<T, I> {
    fn spend_funds(
        budget_remaining: &mut BalanceOf<T, I>,
        imbalance: &mut PositiveImbalanceOf<T, I>,
        total_weight: &mut Weight,
        missed_any: &mut bool,
    ) {
        // Just to make sure that treasury won't burn funds
        *missed_any = true;

        let fraction_for_recycle = T::SpendThreshold::get().mul_ceil(Self::treasury_balance());
        let imbalance_amount = imbalance.peek();

        // imbalance amount is greater than amount for recycle, no need to continue
        if fraction_for_recycle <= imbalance_amount {
            *total_weight += T::DbWeight::get().reads_writes(1, 0);
            return;
        }

        let unrecycled_amount = fraction_for_recycle.saturating_sub(imbalance_amount);
        let (debit, credit) = T::Currency::pair(unrecycled_amount);
        imbalance.subsume(debit);
        T::OnRecycled::on_unbalanced(credit);
        Self::deposit_event(Event::Recycled { recyled_funds: unrecycled_amount });

        *budget_remaining = budget_remaining.saturating_sub(unrecycled_amount);
        *total_weight += <T as pallet::Config<I>>::WeightInfo::spend_funds();

        Self::update_fee_recycling_rate(total_weight);
    }
}

impl<T: Config<I>, I: 'static> OnSessionChange for Pallet<T, I> {
    fn on_new_session(_index: SessionIndex) {
        Self::exchange_energy()
    }
}

impl<T: Config<I>, I: 'static> Get<Permill> for Pallet<T, I> {
    fn get() -> Permill {
        Self::fee_recycling_rate()
    }
}
