#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::tokens::fungibles::{Balanced, Inspect};
use frame_support::traits::tokens::{Fortitude, Precision, Preservation};
use frame_support::weights::{WeightToFeeCoefficient, WeightToFeePolynomial};
pub use pallet::*;
pub use pallet_transaction_payment::OnChargeTransaction;
use smallvec::{smallvec, SmallVec};
use sp_runtime::traits::{DispatchInfoOf, PostDispatchInfoOf, Zero};
use sp_runtime::Perbill;

#[cfg(test)]
pub(crate) mod mock;

#[cfg(test)]
mod tests;

/// Custom fee calculation for specified scenarios
pub trait CustomFee<RuntimeCall, DispatchInfo, Balance> {
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: &DispatchInfo,
    ) -> Option<Balance>;
}

type BalanceOf<T> =
    <<T as Config>::Balanced as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_transaction_payment::Config + pallet_assets::Config
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Hook for energy withdraw
        type Balanced: Balanced<Self::AccountId>;
        /// Get energy asset id
        type GetEnergyAssetId: Get<<Self::Balanced as Inspect<Self::AccountId>>::AssetId>;
        /// Get constant fee value in energy units
        type GetConstantEnergyFee: Get<Self::Balance>;
        /// Calculates custom fee for selected pallets/extrinsics/execution scenarios
        type CustomFee: CustomFee<
            Self::RuntimeCall,
            DispatchInfoOf<Self::RuntimeCall>,
            BalanceOf<Self>,
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
        type LiquidityInfo = ();

        fn withdraw_fee(
            who: &T::AccountId,
            call: &T::RuntimeCall,
            dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
            fee: Self::Balance,
            _tip: Self::Balance,
        ) -> Result<Self::LiquidityInfo, TransactionValidityError> {
            if fee.is_zero() {
                return Ok(());
            }

            let energy_asset_id = T::GetEnergyAssetId::get();
            let fee =
                if let Some(custom_fee) = T::CustomFee::dispatch_info_to_fee(call, dispatch_info) {
                    custom_fee
                } else {
                    fee
                };

            match T::Balanced::withdraw(
                energy_asset_id,
                who,
                fee,
                Precision::Exact,
                Preservation::Preserve,
                Fortitude::Force,
            ) {
                Ok(_) => {
                    Self::deposit_event(Event::<T>::EnergyFeePaid {
                        who: who.clone(),
                        amount: fee,
                    });
                    Ok(())
                },
                Err(_) => Err(InvalidTransaction::Payment.into()),
            }
        }

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

    impl<T: Config> WeightToFeePolynomial for Pallet<T> {
        type Balance = T::Balance;
        fn polynomial() -> SmallVec<[WeightToFeeCoefficient<Self::Balance>; 4]> {
            smallvec!(WeightToFeeCoefficient {
                coeff_integer: T::GetConstantEnergyFee::get(),
                coeff_frac: Perbill::zero(),
                negative: false,
                degree: 0,
            })
        }
    }
}
