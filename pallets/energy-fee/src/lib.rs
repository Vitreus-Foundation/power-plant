#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{
    fungible::{Balanced, Credit, Inspect},
    tokens::{Fortitude, Precision, Preservation},
};
pub use pallet::*;
pub(crate) use pallet_evm::{AddressMapping, OnChargeEVMTransaction};
pub use pallet_transaction_payment::OnChargeTransaction;

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

pub mod traits;
pub use crate::traits::{CustomFee, Exchange, TokenExchange};

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
    use frame_support::pallet_prelude::*;

    /// Pallet which implements fee withdrawal traits
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_transaction_payment::Config
        + pallet_assets::Config
        + pallet_evm::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Get constant fee value
        type GetConstantFee: Get<Self::Balance>;
        /// Calculates custom fee for selected pallets/extrinsics/execution scenarios
        type CustomFee: CustomFee<
            Self::RuntimeCall,
            DispatchInfoOf<Self::RuntimeCall>,
            Self::Balance,
            Self::GetConstantFee,
        >;
        /// Fee token manipulation traits
        type FeeTokenBalanced: Balanced<Self::AccountId>
            + Inspect<Self::AccountId, Balance = Self::Balance>;
        /// Chain currency (main token) manipulation traits
        type MainTokenBalanced: Balanced<Self::AccountId>
            + Inspect<Self::AccountId, Balance = Self::Balance>;
        /// Exchange main token -> fee token
        /// Could not be used for fee token -> main token exchange
        type EnergyExchange: TokenExchange<
            Self::AccountId,
            Self::MainTokenBalanced,
            Self::FeeTokenBalanced,
            Self::Balance,
        >;
        /// Energy exchange rate (1 Main token = `EnergyRate.0`/`EnergyRate.1` Fee token)
        #[pallet::constant]
        type EnergyRate: Get<(Self::Balance, Self::Balance)>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Energy fee is paid to execute transaction [who, fee amount]
        EnergyFeePaid { who: T::AccountId, amount: T::Balance },
    }

    impl<T: Config> OnChargeTransaction<T> for Pallet<T> {
        type Balance = T::Balance;
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

            T::FeeTokenBalanced::withdraw(
                who,
                fee,
                Precision::Exact,
                Preservation::Protect,
                Fortitude::Force,
            )
            .map(|imbalance| {
                Self::deposit_event(Event::<T>::EnergyFeePaid { who: who.clone(), amount: fee });
                Some(imbalance)
            })
            .map_err(|_| InvalidTransaction::Payment.into())
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

            T::FeeTokenBalanced::withdraw(
                &account_id,
                const_energy_fee,
                Precision::Exact,
                Preservation::Protect,
                Fortitude::Force,
            )
            .map(|imbalance| {
                Self::deposit_event(Event::<T>::EnergyFeePaid {
                    who: account_id,
                    amount: const_energy_fee,
                });
                Some(imbalance)
            })
            .map_err(|_| pallet_evm::Error::<T>::BalanceLow)
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
        amount: T::Balance,
    ) -> Result<(), DispatchError> {
        let current_balance =
            T::FeeTokenBalanced::reducible_balance(who, Preservation::Protect, Fortitude::Force);
        let minimum_amount = current_balance
            .is_zero()
            .then(T::FeeTokenBalanced::minimum_balance)
            .unwrap_or(T::Balance::zero());
        if current_balance < amount {
            let missing_balance = amount
                .checked_add(&minimum_amount)
                .ok_or(DispatchError::Arithmetic(Overflow))?
                .checked_sub(&current_balance)
                .ok_or(DispatchError::Arithmetic(Underflow))?; // sanity check
            T::EnergyExchange::exchange_from_output(who, missing_balance).map(|_| ())
        } else {
            Ok(())
        }
    }
}
