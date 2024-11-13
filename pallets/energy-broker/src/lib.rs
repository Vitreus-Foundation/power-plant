//! # Vitreus Energy Broker pallet

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![allow(clippy::result_unit_err, clippy::too_many_arguments)]

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock;

pub use pallet::*;

use frame_support::{
    traits::{
        fungibles::{Balanced, Inspect, Mutate},
        tokens::{
            Balance,
            Fortitude::Polite,
            Precision::Exact,
            Preservation::{self, Expendable, Preserve},
        },
    },
    PalletId,
};
use sp_runtime::{
    traits::{
        AccountIdConversion, CheckedDiv, CheckedMul, Ensure, Get, IntegerSquareRoot, One,
        StaticLookup, Zero,
    },
    DispatchError, Saturating, TokenError,
};

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_arithmetic::traits::Unsigned;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The type in which the assets for swapping are measured.
        type Balance: Balance;

        /// A type used for calculations concerning the `Balance` type to avoid possible overflows.
        type HigherPrecisionBalance: IntegerSquareRoot
            + One
            + Ensure
            + Unsigned
            + From<u32>
            + From<Self::Balance>
            + TryInto<Self::Balance>;

        /// Type of asset class, sourced from [`Config::Assets`], utilized to offer liquidity.
        type AssetKind: Parameter + MaxEncodedLen;

        /// Registry of assets utilized for providing liquidity.
        type Assets: Inspect<Self::AccountId, AssetId = Self::AssetKind, Balance = Self::Balance>
            + Mutate<Self::AccountId>
            + Balanced<Self::AccountId>;

        /// A type used for conversion between an energy balance and an asset balance.
        type BalanceConverter: EnergyBalanceConverter<Self::Balance, Self::AssetKind>;

        /// A % the energy broker will take of every swap. Represents 10ths of a percent.
        #[pallet::constant]
        type SwapFee: Get<u32>;

        /// Identifier of native asset.
        #[pallet::constant]
        type NativeAsset: Get<Self::AssetKind>;

        /// Identifier of energy asset.
        #[pallet::constant]
        type EnergyAsset: Get<Self::AssetKind>;
    }

    // Pallet's events.
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A successful call of the `ForceAddLiquidity` extrinsic will create this event.
        LiquidityAdded {
            /// The account that the liquidity was taken from.
            source: T::AccountId,
            /// The asset that was added.
            asset: T::AssetKind,
            /// The amount that was added.
            amount: T::Balance,
        },
        /// Assets have been converted from one to another. Both `SwapExactTokenForToken`
        /// and `SwapTokenForExactToken` will generate this event.
        SwapExecuted {
            /// Which account was the instigator of the swap.
            who: T::AccountId,
            /// The swapped assets.
            path: (T::AssetKind, T::AssetKind),
            /// The amount of the first asset that was swapped.
            amount_in: T::Balance,
            /// The amount of the second asset that was received.
            amount_out: T::Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// An overflow happened.
        Overflow,
        /// Amount can't be zero.
        ZeroAmount,
        /// The destination account cannot exist with the swapped funds.
        BelowMinimum,
        /// Insufficient liquidity in the energy broker.
        InsufficientLiquidity,
        /// Calculated amount out is less than provided minimum amount.
        ProvidedMinimumNotSufficientForSwap,
        /// Provided maximum amount is not sufficient for swap.
        ProvidedMaximumNotSufficientForSwap,
        /// The provided path contains an invalid asset.
        InvalidPath,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    /// Pallet's callable functions.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add liquidity to the energy broker.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(200_000_000, 20000))]
        pub fn force_add_liquidity(
            origin: OriginFor<T>,
            source: AccountIdLookupOf<T>,
            asset: T::AssetKind,
            amount: T::Balance,
            keep_alive: bool,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let source = T::Lookup::lookup(source)?;
            let preservation = match keep_alive {
                true => Preserve,
                false => Expendable,
            };

            T::Assets::transfer(asset.clone(), &source, &Self::account_id(), amount, preservation)?;

            Self::deposit_event(Event::LiquidityAdded { source, asset, amount });

            Ok(())
        }

        /// Swap the exact amount of input asset into output asset.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(200_000_000, 20000))]
        pub fn swap_exact_tokens_for_tokens(
            origin: OriginFor<T>,
            path: (T::AssetKind, T::AssetKind),
            amount_in: T::Balance,
            amount_out_min: Option<T::Balance>,
            keep_alive: bool,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            Self::do_swap_exact_tokens_for_tokens(
                sender,
                path,
                amount_in,
                amount_out_min,
                keep_alive,
            )
            .map(|_| ())
        }

        /// Swap any amount of input asset to get the exact amount of output asset.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(200_000_000, 20000))]
        pub fn swap_tokens_for_exact_tokens(
            origin: OriginFor<T>,
            path: (T::AssetKind, T::AssetKind),
            amount_out: T::Balance,
            amount_in_max: Option<T::Balance>,
            keep_alive: bool,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;

            Self::do_swap_tokens_for_exact_tokens(
                sender,
                path,
                amount_out,
                amount_in_max,
                keep_alive,
            )
            .map(|_| ())
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of the energy broker.
        pub fn account_id() -> T::AccountId {
            const ID: PalletId = PalletId(*b"energybr");
            AccountIdConversion::<T::AccountId>::into_account_truncating(&ID)
        }

        /// Calculates amount out.
        ///
        /// Given an input amount and swap path, returns the output amount
        /// of the other asset and the swap fee amount.
        pub fn get_amount_out(
            amount_in: T::Balance,
            path: &(T::AssetKind, T::AssetKind),
        ) -> Result<(T::Balance, T::Balance), Error<T>> {
            ensure!(amount_in > Zero::zero(), Error::<T>::ZeroAmount);

            let exchange_in = Self::to_amount_with_fee_deducted(amount_in)?;
            let fee = amount_in.saturating_sub(exchange_in);

            let amount_out = match path {
                (asset_in, asset_out) if *asset_in == T::EnergyAsset::get() => {
                    T::BalanceConverter::energy_to_asset_balance(asset_out.clone(), exchange_in)
                },
                (asset_in, asset_out) if *asset_out == T::EnergyAsset::get() => {
                    T::BalanceConverter::asset_to_energy_balance(asset_in.clone(), exchange_in)
                },
                _ => None,
            }
            .ok_or(Error::InvalidPath)?;

            ensure!(amount_out > Zero::zero(), Error::<T>::ZeroAmount);

            Ok((amount_out, fee))
        }

        /// Calculates amount in.
        ///
        /// Given an output amount and swap path, returns the input amount
        /// of the other asset and the swap fee amount.
        pub fn get_amount_in(
            amount_out: T::Balance,
            path: &(T::AssetKind, T::AssetKind),
        ) -> Result<(T::Balance, T::Balance), Error<T>> {
            ensure!(amount_out > Zero::zero(), Error::<T>::ZeroAmount);

            let exchange_in = match path {
                (asset_in, asset_out) if *asset_in == T::EnergyAsset::get() => {
                    T::BalanceConverter::asset_to_energy_balance(asset_out.clone(), amount_out)
                },
                (asset_in, asset_out) if *asset_out == T::EnergyAsset::get() => {
                    T::BalanceConverter::energy_to_asset_balance(asset_in.clone(), amount_out)
                },
                _ => None,
            }
            .ok_or(Error::InvalidPath)?;

            // Correct the input amount if it calculates to zero
            // to prevent swap failure when buying energy.
            let exchange_in = exchange_in.max(1u8.into());

            let amount_in = Self::to_amount_with_fee_included(exchange_in)?;
            let fee = amount_in.saturating_sub(exchange_in);

            Ok((amount_in, fee))
        }

        /// Swap the exact amount of native asset into energy asset.
        pub fn swap_exact_native_for_energy(
            sender: T::AccountId,
            amount_in: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            Self::do_swap_exact_tokens_for_tokens(
                sender,
                (T::NativeAsset::get(), T::EnergyAsset::get()),
                amount_in,
                None,
                true,
            )
        }

        /// Swap any amount of native asset to get the exact amount of energy asset.
        pub fn swap_native_for_exact_energy(
            sender: T::AccountId,
            amount_out: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            Self::do_swap_tokens_for_exact_tokens(
                sender,
                (T::NativeAsset::get(), T::EnergyAsset::get()),
                amount_out,
                None,
                true,
            )
        }

        fn do_swap_exact_tokens_for_tokens(
            sender: T::AccountId,
            path: (T::AssetKind, T::AssetKind),
            amount_in: T::Balance,
            amount_out_min: Option<T::Balance>,
            keep_alive: bool,
        ) -> Result<T::Balance, DispatchError> {
            if let Some(amount_out_min) = amount_out_min {
                ensure!(amount_out_min > Zero::zero(), Error::<T>::ZeroAmount);
            }

            let (amount_out, fee) = Self::get_amount_out(amount_in, &path)?;

            if let Some(amount_out_min) = amount_out_min {
                ensure!(
                    amount_out >= amount_out_min,
                    Error::<T>::ProvidedMinimumNotSufficientForSwap
                );
            }

            Self::do_swap(&sender, &path, (amount_in, amount_out), fee, keep_alive)?;

            Self::deposit_event(Event::SwapExecuted { who: sender, path, amount_in, amount_out });

            Ok(amount_out)
        }

        fn do_swap_tokens_for_exact_tokens(
            sender: T::AccountId,
            path: (T::AssetKind, T::AssetKind),
            amount_out: T::Balance,
            amount_in_max: Option<T::Balance>,
            keep_alive: bool,
        ) -> Result<T::Balance, DispatchError> {
            if let Some(amount_in_max) = amount_in_max {
                ensure!(amount_in_max > Zero::zero(), Error::<T>::ZeroAmount);
            }

            let (amount_in, fee) = Self::get_amount_in(amount_out, &path)?;

            if let Some(amount_in_max) = amount_in_max {
                ensure!(
                    amount_in <= amount_in_max,
                    Error::<T>::ProvidedMaximumNotSufficientForSwap
                );
            }

            Self::do_swap(&sender, &path, (amount_in, amount_out), fee, keep_alive)?;

            Self::deposit_event(Event::SwapExecuted { who: sender, path, amount_in, amount_out });

            Ok(amount_in)
        }

        fn do_swap(
            sender: &T::AccountId,
            path: &(T::AssetKind, T::AssetKind),
            amounts: (T::Balance, T::Balance),
            fee_part: T::Balance,
            keep_alive: bool,
        ) -> DispatchResult {
            let (asset_in, asset_out) = path;
            let (amount_in, amount_out) = amounts;

            let broker_account = Self::account_id();

            let reserve =
                T::Assets::reducible_balance(asset_out.clone(), &broker_account, Preserve, Polite);
            ensure!(reserve >= amount_out, Error::<T>::InsufficientLiquidity);

            let preservation = match keep_alive {
                true => Preserve,
                false => Expendable,
            };

            if preservation == Preserve {
                let free =
                    T::Assets::reducible_balance(asset_in.clone(), sender, preservation, Polite);
                ensure!(free >= amount_in, TokenError::NotExpendable);
            }

            Self::transfer(
                asset_in.clone(),
                sender,
                &broker_account,
                amount_in,
                fee_part,
                preservation,
            )?;

            Self::transfer(
                asset_out.clone(),
                &broker_account,
                sender,
                amount_out,
                Zero::zero(),
                Preserve,
            )?;

            Ok(())
        }

        fn transfer(
            asset: T::AssetKind,
            source: &T::AccountId,
            dest: &T::AccountId,
            amount: T::Balance,
            _fee_part: T::Balance,
            preservation: Preservation,
        ) -> DispatchResult {
            let credit = T::Assets::withdraw(asset, source, amount, Exact, preservation, Polite)?;

            T::Assets::resolve(dest, credit).map_err(|_| Error::<T>::BelowMinimum)?;

            Ok(())
        }

        fn to_amount_with_fee_deducted(amount: T::Balance) -> Result<T::Balance, Error<T>> {
            T::HigherPrecisionBalance::from(amount)
                .checked_mul(
                    &(T::HigherPrecisionBalance::from(1000u32) - (T::SwapFee::get().into())),
                )
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&T::HigherPrecisionBalance::from(1000u32))
                .ok_or(Error::<T>::Overflow)?
                .try_into()
                .map_err(|_| Error::<T>::Overflow)
        }

        fn to_amount_with_fee_included(amount: T::Balance) -> Result<T::Balance, Error<T>> {
            T::HigherPrecisionBalance::from(amount)
                .checked_mul(&T::HigherPrecisionBalance::from(1000u32))
                .ok_or(Error::<T>::Overflow)?
                .checked_div(
                    &(T::HigherPrecisionBalance::from(1000u32) - (T::SwapFee::get().into())),
                )
                .ok_or(Error::<T>::Overflow)?
                .try_into()
                .map_err(|_| Error::<T>::Overflow)
        }
    }
}

/// Conversion between energy balance and asset balance.
pub trait EnergyBalanceConverter<Balance, AssetId> {
    /// Converts an asset balance into an energy balance.
    fn asset_to_energy_balance(asset_id: AssetId, balance: Balance) -> Option<Balance>;

    /// Converts an energy balance into an asset balance.
    fn energy_to_asset_balance(asset_id: AssetId, balance: Balance) -> Option<Balance>;
}
