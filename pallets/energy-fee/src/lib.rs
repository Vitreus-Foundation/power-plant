//! Energy Fee Pallet
//!
//! A dual-token transaction fee system that uses both native tokens (VTRS) and energy tokens (VNRG)
//! for transaction fees with dynamic fee adjustments based on network conditions.
//!
//! # Overview
//!
//! This pallet implements a sophisticated fee mechanism that:
//! - Supports payment in both VTRS and VNRG tokens
//! - Dynamically adjusts fees based on block fullness
//! - Provides automatic token exchange for fee payments
//! - Manages energy burning thresholds
//! - Integrates with EVM transaction fee handling
//!
//! # Security Notes
//!
//! Important security considerations when using this pallet:
//! 1. Fee burning thresholds protect against network spam and DoS attacks
//! 2. Block fullness thresholds prevent network congestion
//! 3. Energy rate manipulation is prevented through controlled exchange mechanisms
//! 4. Only authorized origins can modify system parameters
//!
//! # Fee Calculation
//!
//! Fees are calculated based on:
//! - Base fee amount
//! - Dynamic multiplier based on block fullness
//! - Custom fee logic for specific extrinsics
//! - EVM-specific fee calculations
//!
//! # Interface
//!
//! Key traits:
//! - `OnChargeTransaction`: Handles standard transaction fee withdrawal
//! - `OnChargeEVMTransaction`: Handles EVM transaction fee withdrawal
//! - `MultiplierUpdate`: Controls fee multiplier adjustments
//!
//! # Configuration
//!
//! Required configuration parameters:
//! - `ManageOrigin`: Authority allowed to modify pallet parameters
//! - `GetConstantFee`: Base fee value
//! - `CustomFee`: Custom fee calculation logic
//! - `EnergyAsset`: Fee token (VNRG) operations
//! - `EnergyExchange`: Token exchange mechanism
//!
//! # Warning
//!
//! Modifying fee parameters can significantly impact network economics and security.
//! Changes should be carefully considered and gradually implemented.

#![cfg_attr(not(feature = "std"), no_std)]

pub use crate::extension::CheckEnergyFee;
pub use crate::traits::CustomFee;
use frame_support::dispatch::{DispatchClass, DispatchInfo, PostDispatchInfo};
use frame_support::traits::{
    fungible::{Balanced, Credit, Inspect, Mutate},
    tokens::{Fortitude, Imbalance, Precision, Preservation},
};
pub use pallet::*;
pub(crate) use pallet_evm::{AddressMapping, OnChargeEVMTransaction};
pub use pallet_transaction_payment::{
    Config as TransactionPaymentConfig, Multiplier, MultiplierUpdate, OnChargeTransaction,
};
use vitreus_runtime_common::{OnEnergyBurn, QuotePriceNativeForEnergy, SwapNativeForEnergy};

use sp_arithmetic::{traits::CheckedAdd, ArithmeticError::Overflow};
use sp_core::{RuntimeDebug, H160, U256};
use sp_runtime::{
    traits::{Convert, DispatchInfoOf, Dispatchable, Get, PostDispatchInfoOf, Saturating, Zero},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    DispatchError, Perbill, Permill, Perquintill,
};

#[cfg(test)]
pub(crate) mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub mod extension;
pub mod traits;

pub(crate) type BalanceOf<T> =
    <<T as pallet_evm::Config>::Currency as Inspect<AccountIdOf<T>>>::Balance;
pub(crate) type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub type FeeCreditOf<T> =
    Credit<<T as frame_system::Config>::AccountId, <T as Config>::EnergyAsset>;

/// Fee type inferred from call info
#[derive(PartialEq, Eq, RuntimeDebug)]
pub enum CallFee<Balance> {
    Regular(Balance),
    // The EVM fee is charged separately
    EVM(Balance),
}

impl<Balance> CallFee<Balance> {
    pub fn into_inner(self) -> Balance {
        match self {
            Self::Regular(fee) => fee,
            Self::EVM(fee) => fee,
        }
    }
}

// TODO: remove possibility to pay tips and increase call priority
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::{OptionQuery, ValueQuery, *},
        traits::{EnsureOrigin, Hooks, OnUnbalanced},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use sp_arithmetic::traits::One;

    /// Pallet which implements fee withdrawal traits
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_transaction_payment::Config + pallet_evm::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Defines who can manage parameters of this pallet.
        type ManageOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Get constant fee value.
        type GetConstantFee: Get<BalanceOf<Self>>;

        /// Calculates custom fee for selected pallets/extrinsics/execution scenarios.
        type CustomFee: CustomFee<
            Self::RuntimeCall,
            DispatchInfoOf<Self::RuntimeCall>,
            BalanceOf<Self>,
            Self::GetConstantFee,
        >;

        /// Energy asset.
        type EnergyAsset: Balanced<Self::AccountId>
            + Inspect<Self::AccountId, Balance = BalanceOf<Self>>
            + Mutate<Self::AccountId>;

        /// Static Energy asset.
        type StaticEnergyAsset: Balanced<Self::AccountId>
            + Inspect<Self::AccountId, Balance = BalanceOf<Self>>
            + Mutate<Self::AccountId>;

        /// Liquid Energy asset.
        type LiquidEnergyAsset: Balanced<Self::AccountId>
            + Inspect<Self::AccountId, Balance = BalanceOf<Self>>
            + Mutate<Self::AccountId>;

        /// A type used for swapping native currency for energy.
        type EnergyExchange: QuotePriceNativeForEnergy<Balance = BalanceOf<Self>>
            + SwapNativeForEnergy<Self::AccountId, Balance = BalanceOf<Self>>;

        /// Handler for when a fee has been withdrawn.
        type OnWithdrawFee: OnWithdrawFeeHandler<Self::AccountId>;

        /// Handler for when energy has been burned.
        type OnEnergyBurn: OnEnergyBurn<BalanceOf<Self>>;

        /// Proportion of the transaction fee that is recycled.
        type FeeRecyclingRate: Get<Permill>;

        /// Recipient of the recycled fees.
        type FeeRecyclingDestination: OnUnbalanced<FeeCreditOf<Self>>;
    }

    #[pallet::storage]
    #[pallet::getter(fn burned_energy)]
    pub type BurnedEnergy<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn burned_energy_threshold)]
    pub type BurnedEnergyThreshold<T: Config> = StorageValue<_, BalanceOf<T>, OptionQuery>;

    #[pallet::type_value]
    pub fn DefaultBlockFullnessThreshold<T: Config>() -> Perquintill {
        Perquintill::one()
    }

    #[pallet::storage]
    #[pallet::getter(fn block_fullness_threshold)]
    pub type BlockFullnessThreshold<T: Config> =
        StorageValue<_, Perquintill, ValueQuery, DefaultBlockFullnessThreshold<T>>;

    #[pallet::type_value]
    pub fn DefaultFeeMultiplier<T: Config>() -> Multiplier {
        Multiplier::one()
    }

    #[pallet::storage]
    #[pallet::getter(fn upper_fee_multiplier)]
    pub type UpperFeeMultiplier<T: Config> =
        StorageValue<_, Multiplier, ValueQuery, DefaultFeeMultiplier<T>>;

    #[pallet::storage]
    #[pallet::getter(fn base_fee)]
    pub type BaseFee<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery, T::GetConstantFee>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Energy fee is paid to execute transaction [who, fee_amount]
        EnergyFeePaid {
            who: T::AccountId,
            amount: BalanceOf<T>,
        },
        /// The burned energy threshold was updated [new_threshold]
        BurnedEnergyThresholdUpdated {
            new_threshold: BalanceOf<T>,
        },
        BlockFullnessThresholdUpdated {
            new_threshold: Perquintill,
        },
        UpperFeeMultiplierUpdated {
            new_multiplier: Multiplier,
        },
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            BurnedEnergy::<T>::put(BalanceOf::<T>::zero());
            T::DbWeight::get().writes(1)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn update_burned_energy_threshold(
            origin: OriginFor<T>,
            new_threshold: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            T::ManageOrigin::ensure_origin(origin)?;
            BurnedEnergyThreshold::<T>::put(new_threshold);
            Self::deposit_event(Event::<T>::BurnedEnergyThresholdUpdated { new_threshold });
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn update_block_fullness_threshold(
            origin: OriginFor<T>,
            new_threshold: Perquintill,
        ) -> DispatchResultWithPostInfo {
            T::ManageOrigin::ensure_origin(origin)?;
            BlockFullnessThreshold::<T>::put(new_threshold);
            Self::deposit_event(Event::<T>::BlockFullnessThresholdUpdated { new_threshold });
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn update_upper_fee_multiplier(
            origin: OriginFor<T>,
            new_multiplier: Multiplier,
        ) -> DispatchResultWithPostInfo {
            T::ManageOrigin::ensure_origin(origin)?;
            UpperFeeMultiplier::<T>::put(new_multiplier);
            Self::deposit_event(Event::<T>::UpperFeeMultiplierUpdated { new_multiplier });
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn update_base_fee(
            origin: OriginFor<T>,
            new_base_fee: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            T::ManageOrigin::ensure_origin(origin)?;
            BaseFee::<T>::put(new_base_fee);
            Ok(().into())
        }
    }

    impl<T: Config> OnChargeTransaction<T> for Pallet<T>
    where
        T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
    {
        type Balance = BalanceOf<T>;
        type LiquidityInfo = Option<Credit<T::AccountId, T::EnergyAsset>>;

        fn withdraw_fee(
            who: &T::AccountId,
            call: &T::RuntimeCall,
            dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
            fee: Self::Balance,
            _tip: Self::Balance,
        ) -> Result<Self::LiquidityInfo, TransactionValidityError> {
            if fee.is_zero() {
                return Ok(None);
            }

            let fee = match T::CustomFee::dispatch_info_to_fee(call, Some(dispatch_info), Some(fee))
            {
                CallFee::Regular(fee) => fee,
                CallFee::EVM(fee) => {
                    Self::ensure_sufficient_energy(who, fee).map_err(|_| {
                        TransactionValidityError::Invalid(InvalidTransaction::Payment)
                    })?;
                    Zero::zero()
                },
            };

            if fee.is_zero() {
                return Ok(None);
            }

            Self::ensure_sufficient_energy(who, fee)
                .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

            let imbalance = T::EnergyAsset::withdraw(
                who,
                fee,
                Precision::Exact,
                Preservation::Expendable,
                Fortitude::Force,
            )
            .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

            T::OnWithdrawFee::on_withdraw_fee(who);

            Ok(Some(imbalance))
        }

        fn correct_and_deposit_fee(
            who: &T::AccountId,
            dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
            post_info: &PostDispatchInfoOf<T::RuntimeCall>,
            _corrected_fee: Self::Balance,
            tip: Self::Balance,
            already_withdrawn: Self::LiquidityInfo,
        ) -> Result<(), TransactionValidityError> {
            if let Some(paid) = already_withdrawn {
                let refund_imbalance = if post_info.pays_fee(dispatch_info) == Pays::No {
                    T::EnergyAsset::deposit(
                        who,
                        // don't refund tip
                        paid.peek().saturating_sub(tip),
                        Precision::BestEffort,
                    )
                    .ok()
                } else {
                    None
                };

                // merge the imbalance caused by paying the fees and refunding parts of it again.
                let adjusted_paid: Credit<T::AccountId, T::EnergyAsset> = paid
                    .offset(refund_imbalance.unwrap_or_default())
                    .same()
                    .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

                let actual_fee = adjusted_paid.peek();
                let (tip, fee) = adjusted_paid.split(tip);

                let recycled_amount = T::FeeRecyclingRate::get() * fee.peek();
                let (recycled, burned) = fee.split(recycled_amount);

                T::FeeRecyclingDestination::on_unbalanced(recycled);

                Self::update_burned_energy(burned.peek());
                Self::update_burned_energy(tip.peek());

                Self::deposit_event(Event::<T>::EnergyFeePaid {
                    who: who.clone(),
                    amount: actual_fee,
                });
            }

            Ok(())
        }
    }

    impl<T: Config> OnChargeEVMTransaction<T> for Pallet<T> {
        // Kept type as Option to satisfy bound of Default
        type LiquidityInfo = Option<Credit<T::AccountId, T::EnergyAsset>>;

        fn withdraw_fee(
            who: &H160,
            fee: U256,
        ) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
            if fee.is_zero() {
                return Ok(None);
            }

            let const_energy_fee = T::CustomFee::ethereum_fee();
            let account_id = <T as pallet_evm::Config>::AddressMapping::into_account_id(*who);

            Self::ensure_sufficient_energy(&account_id, const_energy_fee)
                .map_err(|_| pallet_evm::Error::<T>::BalanceLow)?;

            let imbalance = T::EnergyAsset::withdraw(
                &account_id,
                const_energy_fee,
                Precision::Exact,
                Preservation::Expendable,
                Fortitude::Force,
            )
            .inspect(|_| {
                Self::deposit_event(Event::<T>::EnergyFeePaid {
                    who: account_id.clone(),
                    amount: const_energy_fee,
                });
            })
            .map_err(|_| pallet_evm::Error::<T>::BalanceLow)?;

            T::OnWithdrawFee::on_withdraw_fee(&account_id);

            Ok(Some(imbalance))
        }

        fn correct_and_deposit_fee(
            _who: &H160,
            _corrected_fee: U256,
            _base_fee: U256,
            already_withdrawn: Self::LiquidityInfo,
        ) -> Self::LiquidityInfo {
            if let Some(credit) = already_withdrawn {
                let recycled_amount = T::FeeRecyclingRate::get() * credit.peek();
                let (recycled, burned) = credit.split(recycled_amount);

                T::FeeRecyclingDestination::on_unbalanced(recycled);
                Self::update_burned_energy(burned.peek());
            };
            None
        }

        // TODO: investigate whether we need this (and check if it influences the chain behaviour)
        fn pay_priority_fee(_tip: Self::LiquidityInfo) {
            // Default Ethereum behaviour: issue the tip to the block author.
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Check if user `who` owns reducible balance of token used for charging fees
    /// of at least `amount`, and if no, then exchange missing funds for user `who` using
    /// `T::EnergyExchange`
    fn ensure_sufficient_energy(
        who: &T::AccountId,
        mut required: BalanceOf<T>,
    ) -> Result<(), DispatchError> {
        required.saturating_reduce(T::EnergyAsset::reducible_balance(
            who,
            Preservation::Expendable,
            Fortitude::Force,
        ));

        if required > BalanceOf::<T>::zero() {
            let amount = Self::exchange_energy::<T::StaticEnergyAsset>(who, required)?;
            required.saturating_reduce(amount);
        }

        if required > BalanceOf::<T>::zero() {
            let amount = Self::exchange_energy::<T::LiquidEnergyAsset>(who, required)?;
            required.saturating_reduce(amount);
        }

        if required > BalanceOf::<T>::zero() {
            T::EnergyExchange::swap_tokens_for_exact_tokens(who.clone(), required, true)?;
        }

        Ok(())
    }

    fn exchange_energy<A>(
        who: &T::AccountId,
        required: BalanceOf<T>,
    ) -> Result<BalanceOf<T>, DispatchError>
    where
        A: Inspect<T::AccountId, Balance = BalanceOf<T>> + Mutate<T::AccountId>,
    {
        if A::reducible_balance(who, Preservation::Expendable, Fortitude::Force).is_zero() {
            return Ok(BalanceOf::<T>::zero());
        }

        let amount = A::burn_from(
            who,
            required,
            Preservation::Expendable,
            Precision::BestEffort,
            Fortitude::Force,
        )?;

        T::EnergyAsset::mint_into(who, amount)
    }

    /// Calculate fee as native and energy parts based on the presence of energy tokens
    pub fn calculate_fee_parts(
        who: &T::AccountId,
        required: BalanceOf<T>,
    ) -> Option<(BalanceOf<T>, BalanceOf<T>)> {
        let mut total_energy =
            T::EnergyAsset::reducible_balance(who, Preservation::Expendable, Fortitude::Force);

        if total_energy < required {
            total_energy.saturating_accrue(T::StaticEnergyAsset::reducible_balance(
                who,
                Preservation::Expendable,
                Fortitude::Force,
            ));
        }

        if total_energy < required {
            total_energy.saturating_accrue(T::LiquidEnergyAsset::reducible_balance(
                who,
                Preservation::Expendable,
                Fortitude::Force,
            ));
        }

        if total_energy < required {
            T::EnergyExchange::quote_price_tokens_for_exact_tokens(
                required.saturating_sub(total_energy),
                true,
            )
            .map(|amount_in| (total_energy, amount_in))
        } else {
            Some((required, BalanceOf::<T>::zero()))
        }
    }

    fn update_burned_energy(amount: BalanceOf<T>) {
        T::OnEnergyBurn::on_energy_burn(amount);

        BurnedEnergy::<T>::mutate(|total| total.saturating_accrue(amount));
    }

    fn validate_call_fee(fee_amount: BalanceOf<T>) -> Result<(), DispatchError> {
        let attempted_burned = Self::burned_energy()
            .checked_add(&fee_amount)
            .ok_or(DispatchError::Arithmetic(Overflow))?;
        let threshold = Self::burned_energy_threshold();
        match threshold {
            Some(threshold) if attempted_burned <= threshold => Ok(()),
            None => Ok(()),
            _ => Err(DispatchError::Exhausted),
        }
    }
}

impl<T: Config> Convert<Multiplier, Multiplier> for Pallet<T> {
    fn convert(_previous: Multiplier) -> Multiplier {
        let min_multiplier = DefaultFeeMultiplier::<T>::get();
        let max_multiplier = Self::upper_fee_multiplier();

        let weights = T::BlockWeights::get();
        // the computed ratio is only among the normal class.
        let normal_max_weight =
            weights.get(DispatchClass::Normal).max_total.unwrap_or(weights.max_block);
        let current_block_weight = <frame_system::Pallet<T>>::block_weight();
        let normal_block_weight =
            current_block_weight.get(DispatchClass::Normal).min(normal_max_weight);

        // Normalize dimensions so they can be compared. Ensure (defensive) max weight is non-zero.
        let normalized_ref_time = Perbill::from_rational(
            normal_block_weight.ref_time(),
            normal_max_weight.ref_time().max(1),
        );
        let normalized_proof_size = Perbill::from_rational(
            normal_block_weight.proof_size(),
            normal_max_weight.proof_size().max(1),
        );

        // Pick the limiting dimension. If the proof size is the limiting dimension, then the
        // multiplier is adjusted by the proof size. Otherwise, it is adjusted by the ref time.
        let (normal_limiting_dimension, max_limiting_dimension) =
            if normalized_ref_time < normalized_proof_size {
                (normal_block_weight.proof_size(), normal_max_weight.proof_size())
            } else {
                (normal_block_weight.ref_time(), normal_max_weight.ref_time())
            };

        let block_fullness_threshold = Self::block_fullness_threshold();

        let threshold_weight = (block_fullness_threshold * max_limiting_dimension) as u128;
        let block_weight = normal_limiting_dimension as u128;

        if threshold_weight <= block_weight {
            max_multiplier
        } else {
            min_multiplier
        }
    }
}

impl<T: Config> MultiplierUpdate for Pallet<T> {
    fn min() -> Multiplier {
        // Minimal possible value of Multiplier type.
        // Do not confuse with a standard multiplier value.
        // Used for integrity tests in TransactionPayment pallet.
        Default::default()
    }
    fn max() -> Multiplier {
        // Maximal possible value of Multiplier type.
        // Do not confuse with an upper multiplier value.
        // Used for integrity tests in TransactionPayment pallet.
        <Multiplier as sp_runtime::traits::Bounded>::max_value()
    }
    fn target() -> Perquintill {
        // Reading BlockFulnessThreshold from storage causes
        // runtime integrity error during runtime tests due to
        // usage inside pallet_transaction_payment::integrity_test
        // outside any Externalities environment
        DefaultBlockFullnessThreshold::<T>::get()
    }

    fn variability() -> Multiplier {
        // It seems that this function isn't used anywhere
        // Moreover, variability factor is not variance or any such term.
        // It is associated with a particular formula for multiplier
        // calculation, so I can't find a reasonable way to
        // define it in terms of simple switching between low and
        // high multiplier.
        Default::default()
    }
}

/// Handler for when a fee has been withdrawn.
pub trait OnWithdrawFeeHandler<AccountId> {
    fn on_withdraw_fee(who: &AccountId);
}

impl<AccountId> OnWithdrawFeeHandler<AccountId> for () {
    fn on_withdraw_fee(_who: &AccountId) {}
}
