//! Transaction Validation Extension for Energy Fee Pallet
//!
//! This module provides a signed extension `CheckEnergyFee` that validates transactions
//! based on energy fee requirements during the pre-dispatch phase. It ensures that
//! transactions meet energy consumption limits and fee thresholds before execution.
//!
//! # Security Features
//!
//! 1. Pre-dispatch Validation
//!    - Validates transaction fees before execution
//!    - Prevents energy threshold violations
//!    - Special handling for sudo calls
//!
//! 2. Resource Protection
//!    - Guards against energy exhaustion attacks
//!    - Enforces energy consumption limits
//!    - Maintains network stability
//!
//! # Implementation Details
//!
//! The `CheckEnergyFee` extension:
//! - Implements `SignedExtension` trait for transaction validation
//! - Computes and validates custom fee logic
//! - Integrates with both standard and EVM transactions
//! - Bypasses checks for sudo operations
//!
//! # Usage Example
//!
//! ```rust
//! # use frame_support::dispatch::DispatchInfo;
//! # use pallet_energy_fee::extension::CheckEnergyFee;
//! # fn example<T: pallet_energy_fee::Config>() {
//! let extension = CheckEnergyFee::<T>::new();
//! // Used automatically in transaction pipeline
//! # }
//! ```
//!
//! # Warning
//!
//! This extension is critical for network security. Modifications should be
//! carefully tested as they directly impact:
//! - Transaction validation
//! - Network resource consumption
//! - DoS resistance
//!
//! # Technical Notes
//!
//! - Requires `Config` trait implementation with sudo support
//! - Uses `DispatchInfo` for fee calculation
//! - Implements SCALE encoding/decoding
//! - Thread-safe implementation (`Send + Sync`)

#![allow(clippy::new_without_default)]

use crate::{BalanceOf, CallFee, Config, CustomFee, Pallet};
use core::fmt::Debug;
use frame_support::dispatch::{Callable, DispatchInfo};
use frame_support::traits::IsSubType;
use pallet_sudo::{Config as SudoConfig, Pallet as SudoPallet};
use pallet_transaction_payment::{
    Config as TransactionPaymentConfig, OnChargeTransaction, Pallet as TransactionPaymentPallet,
};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{DispatchInfoOf, Dispatchable, SignedExtension},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
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

impl<T: Config + SudoConfig + Send + Sync> SignedExtension for CheckEnergyFee<T>
where
    <T as frame_system::Config>::RuntimeCall:
        Dispatchable<Info = DispatchInfo> + IsSubType<<SudoPallet<T> as Callable<T>>::RuntimeCall>,
    <T as TransactionPaymentConfig>::OnChargeTransaction:
        OnChargeTransaction<T, Balance = BalanceOf<T>>,
{
    type AdditionalSigned = ();
    type Call = <T as frame_system::Config>::RuntimeCall;
    type AccountId = T::AccountId;
    type Pre = ();
    const IDENTIFIER: &'static str = "CheckEnergyFee";

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    fn pre_dispatch(
        self,
        _who: &Self::AccountId,
        call: &Self::Call,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        // Check if call is sudo
        if call.is_sub_type().is_some() {
            return Ok(());
        }

        let fee = TransactionPaymentPallet::<T>::compute_fee(len as u32, info, 0u32.into());
        let fee = match T::CustomFee::dispatch_info_to_fee(call, Some(info), Some(fee)) {
            CallFee::Regular(custom_fee) | CallFee::EVM(custom_fee) => custom_fee,
        };
        Pallet::<T>::validate_call_fee(fee).map_err(|_| {
            TransactionValidityError::Invalid(InvalidTransaction::ExhaustsResources)
        })?;
        Ok(())
    }
}
