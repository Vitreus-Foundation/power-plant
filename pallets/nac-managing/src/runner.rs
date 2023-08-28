use sp_std::{
    convert::Infallible, marker::PhantomData, rc::Rc,
    collections::btree_set::BTreeSet, vec::Vec, mem, cmp::{min, max},
};
use sp_core::{H160, U256, H256};
use sp_runtime::{TransactionOutcome, traits::UniqueSaturatedInto, DispatchResult, DispatchError, MultiAddress};
use frame_support::{
    ensure, traits::{Get, Currency, ExistenceRequirement},
    storage::{StorageMap, StorageDoubleMap},
};
use pallet_evm::{Runner as EvmRunner, RunnerError, CallInfo, CreateInfo};
use frame_support::pallet_prelude::Weight;
use frame_system::pallet_prelude;
use crate::{Config, pallet};

pub struct NacRunner<T:Config> {
    _marker: PhantomData<T>,
}

const VALIDATE_ACCESS_LEVEL: u8 = 1;
const CREATE_ACCESS_LEVEL: u8 = 2;
const CALL_ACCESS_LEVEL: u8 = 2;

impl <T: Config> EvmRunner<T> for NacRunner<T> {
    type Error = pallet_evm::Error<T>;

    fn validate(source: H160, target: Option<H160>, input: Vec<u8>, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, evm_config: &pallet_evm::EvmConfig) -> Result<(), RunnerError<Self::Error>> {
        if !crate::Pallet::<T>::user_has_access(source.clone(), VALIDATE_ACCESS_LEVEL) {
            return Err(RunnerError {
                // error: pallet:Error::<T>::NoPermissions
                error: pallet_evm::Error::Undefined,
                weight: weight_limit.unwrap_or_default(),
            })
        };

        <T as Config>::Runner::validate(
            source,
            target,
            input,
            value,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce,
            access_list,
            is_transactional,
            weight_limit,
            proof_size_base_cost,
            evm_config
        )
    }

    fn call(source: H160, target: H160, input: Vec<u8>, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, validate: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, config: &pallet_evm::EvmConfig) -> Result<CallInfo, RunnerError<Self::Error>> {
        if !crate::Pallet::<T>::user_has_access(source.clone(), CALL_ACCESS_LEVEL) {
            return Err(RunnerError {
                // error: pallet:Error::<T>::NoPermissions
                error: pallet_evm::Error::Undefined,
                weight: weight_limit.unwrap_or_default()
            })
        }

        <T as Config>::Runner::call(
            source,
            target,
            input,
            value,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce,
            access_list,
            is_transactional,
            validate,
            weight_limit,
            proof_size_base_cost,
            config
        )
    }

    fn create(source: H160, init: Vec<u8>, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, validate: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, config: &pallet_evm::EvmConfig) -> Result<CreateInfo, RunnerError<Self::Error>> {
        if !crate::Pallet::<T>::user_has_access(source.clone(), CREATE_ACCESS_LEVEL) {
            return Err(RunnerError {
                // error: pallet:Error::<T>::NoPermissions
                error: pallet_evm::Error::Undefined,
                weight: weight_limit.unwrap_or_default()
            })
        }

        <T as Config>::Runner::create(
            source,
            init,
            value,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce,
            access_list,
            is_transactional,
            validate,
            weight_limit,
            proof_size_base_cost,
            config
        )
    }

    fn create2(source: H160, init: Vec<u8>, salt: H256, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, validate: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, config: &pallet_evm::EvmConfig) -> Result<CreateInfo, RunnerError<Self::Error>> {
        if !crate::Pallet::<T>::user_has_access(source.clone(), CREATE_ACCESS_LEVEL) {
            return Err(RunnerError {
                // error: pallet:Error::<T>::NoPermissions
                error: pallet_evm::Error::Undefined,
                weight: weight_limit.unwrap_or_default()
            })
        }

        <T as Config>::Runner::create2(
            source,
            init,
            salt,
            value,
            gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce,
            access_list,
            is_transactional,
            validate,
            weight_limit,
            proof_size_base_cost,
            config
        )
    }
}