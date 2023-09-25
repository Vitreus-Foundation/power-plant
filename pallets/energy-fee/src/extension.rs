use crate::{CallFee, Config, CustomFee, Pallet};
use frame_support::dispatch::fmt::Debug;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{DispatchInfoOf, SignedExtension},
    transaction_validity::TransactionValidityError,
};
use sp_std::marker::PhantomData;

/// A structure to validate transactions based on user call's fee during the pre-dispatch phase.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckEnergyFee<T: Config>(PhantomData<T>);

impl<T: Config> Debug for CheckEnergyFee<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("CheckEnergyFee").finish()
    }
}

impl<T: Config> CheckEnergyFee<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync + pallet_transaction_payment::Config> SignedExtension
    for CheckEnergyFee<T>
{
    type AdditionalSigned = ();
    type Call = T::RuntimeCall;
    type AccountId = T::AccountId;
    type Pre = ();
    const IDENTIFIER: &'static str = "CheckEnergyFee";

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        let fee = match T::CustomFee::dispatch_info_to_fee(call, info) {
            CallFee::Custom(custom_fee) | CallFee::EVM(custom_fee) => custom_fee,
            _ => {
                pallet_transaction_payment::Pallet::<T>::compute_fee(len as u32, info, 0u32.into())
            },
        };
        Pallet::<T>::validate_call_fee(fee)
            .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::ExhaustsResources));
        Ok(())
    }
}
