#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::tokens::{currency::Currency, WithdrawReasons, ExistenceRequirement};

pub use pallet::*;
pub use pallet_transaction_payment::OnChargeTransaction;
pub(crate) use pallet_evm::{OnChargeEVMTransaction, AddressMapping};

use sp_core::{H160, U256};
use sp_runtime::traits::{DispatchInfoOf, PostDispatchInfoOf, Zero, Get};


// #[cfg(test)]
// pub(crate) mod mock;

// #[cfg(test)]
// mod tests;

/// Fee type inferred from call info
pub enum CallFee<Balance> {
    Custom(Balance),
    Stock,
    // The EVM fee is charged separately
    EVM(Balance)
}

/// Custom fee calculation for specified scenarios
pub trait CustomFee<RuntimeCall, DispatchInfo, Balance, ConstantFee>
    where ConstantFee: Get<Balance>
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: &DispatchInfo,
    ) -> CallFee<Balance>;
}

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type CurrencyOf<T> = <T as Config>::Currency;
type BalanceOf<T> = <CurrencyOf<T> as Currency<AccountIdOf<T>>>::Balance;
type NegativeImbalanceOf<T> = <CurrencyOf<T> as Currency<AccountIdOf<T>>>::NegativeImbalance;

// TODO: remove possibility to pay tips and increase call priority
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_transaction_payment::Config + pallet_assets::Config + pallet_evm::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Currency type for fee withdraw
        type Currency: Currency<Self::AccountId>;
        /// Get constant fee value
        type GetConstantFee: Get<BalanceOf<Self>>;
        /// Calculates custom fee for selected pallets/extrinsics/execution scenarios
        type CustomFee: CustomFee<
            Self::RuntimeCall,
            DispatchInfoOf<Self::RuntimeCall>,
            BalanceOf<Self>,
            Self::GetConstantFee
        >;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Energy fee is paid to execute transaction [who, fee amount]
        EnergyFeePaid { who: T::AccountId, amount: BalanceOf<T> },
    }

    impl<T: Config> OnChargeTransaction<T> for Pallet<T> {
        type Balance = BalanceOf<T>;
        type LiquidityInfo = Option<NegativeImbalanceOf<T>>;

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

            // The fee won't be charged yet in case of EVM call so we check in advance
            // even though possible withdrawal errors are handled afterwards
            let const_energy_fee = T::GetConstantFee::get();
            ensure!(
                CurrencyOf::<T>::free_balance(who) > const_energy_fee,
                TransactionValidityError::Invalid(InvalidTransaction::Payment)
            );

            let fee = match T::CustomFee::dispatch_info_to_fee(call, dispatch_info) {
                CallFee::Custom(custom_fee) => custom_fee,
                CallFee::EVM(_) => return Ok(None),
                _ => fee
            };

            CurrencyOf::<T>::withdraw(
                who,
                fee,
                WithdrawReasons::FEE,
                ExistenceRequirement::KeepAlive
            ).map(|imbalance| {
                Self::deposit_event(Event::<T>::EnergyFeePaid {
                    who: who.clone(),
                    amount: fee,
                });
                Some(imbalance)
            }).map_err(|_| InvalidTransaction::Payment.into()) 
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
        type LiquidityInfo = Option<NegativeImbalanceOf<T>>;

        fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
            if fee.is_zero() {
                return Ok(None);
            }

            let const_energy_fee = T::GetConstantFee::get();
            let account_id = <T as pallet_evm::Config>::AddressMapping::into_account_id(*who);

            CurrencyOf::<T>::withdraw(
                &account_id,
                const_energy_fee,
                WithdrawReasons::FEE,
                ExistenceRequirement::KeepAlive
            ).map(|imbalance| {
                Self::deposit_event(Event::<T>::EnergyFeePaid {
                    who: account_id,
                    amount: const_energy_fee,
                });
                Some(imbalance)
            }).map_err(|_| pallet_evm::Error::<T>::BalanceLow)
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
