#![cfg_attr(not(feature = "std"), no_std)]

pub use crate::extension::CheckEnergyFee;
pub use crate::traits::{CustomFee, Exchange, TokenExchange};
use frame_support::dispatch::RawOrigin;
use frame_support::traits::{
    fungible::{Balanced, Credit, Inspect},
    tokens::{Fortitude, Imbalance, Precision, Preservation},
};
pub use pallet::*;
use pallet_asset_rate::Pallet as AssetRatePallet;
pub(crate) use pallet_evm::{AddressMapping, OnChargeEVMTransaction};
pub use pallet_transaction_payment::{Config as TransactionPaymentConfig, OnChargeTransaction};

use sp_arithmetic::{
    traits::CheckedAdd,
    ArithmeticError::{Overflow, Underflow},
};
use sp_core::{H160, U256};
use sp_runtime::{
    traits::{CheckedSub, DispatchInfoOf, Get, PostDispatchInfoOf, Zero},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    DispatchError,
};

#[cfg(test)]
pub(crate) mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub mod extension;
pub mod traits;

pub(crate) type BalanceOf<T> = <T as pallet_asset_rate::Config>::Balance;

/// Fee type inferred from call info
pub enum CallFee<Balance> {
    Custom(Balance),
    Stock,
    // The EVM fee is charged separately
    EVM(Balance),
}

// TODO: remove possibility to pay tips and increase call priority
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        traits::{EnsureOrigin, Hooks},
        weights::Weight,
    };
    use frame_system::pallet_prelude::*;
    use sp_arithmetic::FixedU128;

    /// Pallet which implements fee withdrawal traits
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_transaction_payment::Config
        + pallet_asset_rate::Config
        + pallet_evm::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Defines who can manage parameters of this pallet
        type ManageOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// Get constant fee value
        type GetConstantFee: Get<BalanceOf<Self>>;
        /// Calculates custom fee for selected pallets/extrinsics/execution scenarios
        type CustomFee: CustomFee<
            Self::RuntimeCall,
            DispatchInfoOf<Self::RuntimeCall>,
            BalanceOf<Self>,
            Self::GetConstantFee,
        >;
        /// Fee token manipulation traits
        type FeeTokenBalanced: Balanced<Self::AccountId>
            + Inspect<Self::AccountId, Balance = BalanceOf<Self>>;
        /// Chain currency (main token) manipulation traits
        type MainTokenBalanced: Balanced<Self::AccountId>
            + Inspect<Self::AccountId, Balance = BalanceOf<Self>>;
        /// Exchange main token -> fee token
        /// Could not be used for fee token -> main token exchange
        type EnergyExchange: TokenExchange<
            Self::AccountId,
            Self::MainTokenBalanced,
            Self::FeeTokenBalanced,
            BalanceOf<Self>,
        >;
        /// Used for initializing the pallet
        type EnergyAssetId: Get<Self::AssetId>;
    }

    #[pallet::storage]
    #[pallet::getter(fn burned_energy)]
    pub type BurnedEnergy<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn burned_energy_threshold)]
    pub type BurnedEnergyThreshold<T: Config> = StorageValue<_, BalanceOf<T>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Energy fee is paid to execute transaction [who, fee_amount]
        EnergyFeePaid { who: T::AccountId, amount: BalanceOf<T> },
        /// The burned energy threshold was updated [new_threshold]
        BurnedEnergyThresholdUpdated { new_threshold: BalanceOf<T> },
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub initial_energy_rate: FixedU128,
        pub _config: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let _ = AssetRatePallet::<T>::create(
                RawOrigin::Root.into(),
                T::EnergyAssetId::get(),
                self.initial_energy_rate,
            );
        }
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
    }

    impl<T: Config> OnChargeTransaction<T> for Pallet<T> {
        type Balance = BalanceOf<T>;
        type LiquidityInfo = Option<Credit<T::AccountId, T::FeeTokenBalanced>>;

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

            let fee = match T::CustomFee::dispatch_info_to_fee(call, dispatch_info) {
                CallFee::Custom(custom_fee) => custom_fee,
                CallFee::EVM(custom_fee) => {
                    Self::on_low_balance_exchange(who, custom_fee).map_err(|_| {
                        TransactionValidityError::Invalid(InvalidTransaction::Payment)
                    })?;
                    return Ok(None);
                },
                _ => fee,
            };

            Self::on_low_balance_exchange(who, fee)
                .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

            let imbalance = T::FeeTokenBalanced::withdraw(
                who,
                fee,
                Precision::Exact,
                Preservation::Expendable,
                Fortitude::Force,
            )
            .map(|imbalance| {
                Self::deposit_event(Event::<T>::EnergyFeePaid { who: who.clone(), amount: fee });
                imbalance
            })
            .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
            Self::update_burned_energy(imbalance.peek())
                .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
            Ok(Some(imbalance))
        }

        // TODO: make a refund for calls non-elligible for custom fee
        fn correct_and_deposit_fee(
            _who: &T::AccountId,
            _dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
            _post_info: &PostDispatchInfoOf<T::RuntimeCall>,
            _corrected_fee: Self::Balance,
            _tip: Self::Balance,
            _already_withdrawn: Self::LiquidityInfo,
        ) -> Result<(), TransactionValidityError> {
            Ok(())
        }
    }

    impl<T: Config> OnChargeEVMTransaction<T> for Pallet<T> {
        // Kept type as Option to satisfy bound of Default
        type LiquidityInfo = Option<Credit<T::AccountId, T::FeeTokenBalanced>>;

        fn withdraw_fee(
            who: &H160,
            fee: U256,
        ) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
            if fee.is_zero() {
                return Ok(None);
            }

            let const_energy_fee = T::GetConstantFee::get();
            let account_id = <T as pallet_evm::Config>::AddressMapping::into_account_id(*who);

            Self::on_low_balance_exchange(&account_id, const_energy_fee)
                .map_err(|_| pallet_evm::Error::<T>::BalanceLow)?;

            let imbalance = T::FeeTokenBalanced::withdraw(
                &account_id,
                const_energy_fee,
                Precision::Exact,
                Preservation::Expendable,
                Fortitude::Force,
            )
            .map(|imbalance| {
                Self::deposit_event(Event::<T>::EnergyFeePaid {
                    who: account_id,
                    amount: const_energy_fee,
                });
                imbalance
            })
            .map_err(|_| pallet_evm::Error::<T>::BalanceLow)?;
            Self::update_burned_energy(imbalance.peek())
                .map_err(|_| pallet_evm::Error::<T>::FeeOverflow)?;
            Ok(Some(imbalance))
        }

        fn correct_and_deposit_fee(
            _who: &H160,
            _corrected_fee: U256,
            _base_fee: U256,
            _already_withdrawn: Self::LiquidityInfo,
        ) -> Self::LiquidityInfo {
            None
        }

        // TODO: handle EVM priority fee
        fn pay_priority_fee(_tip: Self::LiquidityInfo) {
            // Default Ethereum behaviour: issue the tip to the block author.
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Check if user `who` owns reducible balance of token used for chargin fees
    /// of at least `amount`, and if no, then exchange missing funds for user `who` using
    /// `T::EnergyExchange`
    fn on_low_balance_exchange(
        who: &T::AccountId,
        amount: BalanceOf<T>,
    ) -> Result<(), DispatchError> {
        let current_balance =
            T::FeeTokenBalanced::reducible_balance(who, Preservation::Expendable, Fortitude::Force);
        if current_balance < amount {
            let missing_balance = amount
                .checked_sub(&current_balance)
                .ok_or(DispatchError::Arithmetic(Underflow))?; // sanity check
            T::EnergyExchange::exchange_from_output(who, missing_balance).map(|_| ())
        } else {
            Ok(())
        }
    }

    fn update_burned_energy(amount: BalanceOf<T>) -> Result<(), DispatchError> {
        BurnedEnergy::<T>::mutate(|current_burned| {
            *current_burned =
                current_burned.checked_add(&amount).ok_or(DispatchError::Arithmetic(Overflow))?;
            Ok(())
        })
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
