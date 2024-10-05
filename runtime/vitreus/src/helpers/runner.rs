use crate::GetConstantGasLimit;
use fp_evm::UsedGas;
use frame_support::dispatch::{DispatchInfo, GetDispatchInfo};
use frame_support::pallet_prelude::Weight;
use frame_support::traits::{fungible::Inspect, Currency};
use pallet_energy_fee::{CallFee, CustomFee};
use pallet_evm::{runner::stack::Runner, AddressMapping, Call};
use pallet_evm::{CallInfo, CreateInfo, Runner as EvmRunner, RunnerError};
use pallet_nac_managing;
use pallet_transaction_payment::OnChargeTransaction;
use sp_core::{H160, H256, U256};
use sp_runtime::traits::Dispatchable;
use sp_std::{marker::PhantomData, vec::Vec};

pub struct NacRunner<T> {
    _marker: PhantomData<T>,
}

pub const VALIDATE_ACCESS_LEVEL: u8 = 1;
pub const CREATE_ACCESS_LEVEL: u8 = 3;
pub const CALL_ACCESS_LEVEL: u8 = 1;

impl <T> EvmRunner<T> for NacRunner<T>
where
    T: pallet_evm::Config + pallet_nac_managing::Config + pallet_energy_fee::Config + pallet_transaction_payment::Config,
    <<T as pallet_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: TryFrom<U256> + Into<U256>,
    <T as frame_system::Config>::RuntimeCall: From<Call<T>> + GetDispatchInfo + Dispatchable<Info = DispatchInfo>,
    <T as pallet_balances::Config>::Balance: Into<U256>,
    <T as pallet_transaction_payment::Config>::OnChargeTransaction:
        OnChargeTransaction<T, Balance = <T as pallet_balances::Config>::Balance>,
{
    type Error = pallet_evm::Error<T>;

    fn validate(
        source: H160,
        target: Option<H160>,
        input: Vec<u8>,
        value: U256,
        gas_limit: u64,
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        nonce: Option<U256>,
        access_list: Vec<(H160, Vec<H256>)>,
        is_transactional: bool,
        weight_limit: Option<Weight>,
        proof_size_base_cost: Option<u64>,
        evm_config: &pallet_evm::EvmConfig
    ) -> Result<(), RunnerError<Self::Error>> {
        Self::evm_user_has_permission(source, weight_limit, VALIDATE_ACCESS_LEVEL)?;

        Runner::validate(
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

    fn call(
        source: H160,
        target: H160,
        input: Vec<u8>,
        value: U256,
        _gas_limit: u64,
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        nonce: Option<U256>,
        access_list: Vec<(H160, Vec<H256>)>,
        is_transactional: bool,
        validate: bool,
        weight_limit: Option<Weight>,
        proof_size_base_cost: Option<u64>,
        config: &pallet_evm::EvmConfig,
    ) -> Result<CallInfo, RunnerError<Self::Error>> {
        let gas_limit = GetConstantGasLimit::get().as_u64();
        Self::evm_user_has_permission(source, weight_limit, CALL_ACCESS_LEVEL)?;
        let call = Call::new_call_variant_call(
            source,
            target,
            input.clone(),
            value,
            gas_limit,
            max_fee_per_gas.unwrap_or(U256::zero()),
            max_priority_fee_per_gas,
            nonce,
            access_list.clone()
        ).into();
        let gas = Self::calculate_gas(call);

        Runner::call(
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
        ).map(|call_info| {
            let mut call_info = call_info;
            call_info.used_gas = gas;
            call_info
        })
    }

    fn create(
        source: H160,
        init: Vec<u8>,
        value: U256,
        _gas_limit: u64,
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        nonce: Option<U256>,
        access_list: Vec<(H160, Vec<H256>)>,
        is_transactional: bool,
        validate: bool,
        weight_limit: Option<Weight>,
        proof_size_base_cost: Option<u64>,
        config: &pallet_evm::EvmConfig,
    ) -> Result<CreateInfo, RunnerError<Self::Error>> {
        let gas_limit = GetConstantGasLimit::get().as_u64();
        Self::evm_user_has_permission(source, weight_limit, CREATE_ACCESS_LEVEL)?;
        let call = Call::new_call_variant_create(
            source,
            init.clone(),
            value,
            gas_limit,
            max_fee_per_gas.unwrap_or(U256::zero()),
            max_priority_fee_per_gas,
            nonce,
            access_list.clone()
        ).into();
        let gas = Self::calculate_gas(call);

        Runner::create(
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
        ).map(|call_info| {
            let mut call_info = call_info;
            call_info.used_gas = gas;
            call_info
        })
    }

    fn create2(source: H160, init: Vec<u8>, salt: H256, value: U256, _gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, validate: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, config: &pallet_evm::EvmConfig) -> Result<CreateInfo, RunnerError<Self::Error>> {
        let gas_limit = GetConstantGasLimit::get().as_u64();
        Self::evm_user_has_permission(source, weight_limit, CREATE_ACCESS_LEVEL)?;
        let call = Call::new_call_variant_create2(
            source,
            init.clone(),
            salt,
            value,
            gas_limit,
            max_fee_per_gas.unwrap_or(U256::zero()),
            max_priority_fee_per_gas,
            nonce,
            access_list.clone()
        ).into();
        let gas = Self::calculate_gas(call);

        Runner::create2(
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
        ).map(|call_info| {
            let mut call_info = call_info;
            call_info.used_gas = gas;
            call_info
        })
    }
}

impl<T> NacRunner<T>
where
    T: pallet_evm::Config
        + pallet_nac_managing::Config
        + pallet_transaction_payment::Config
        + pallet_energy_fee::Config,
    T::RuntimeCall: GetDispatchInfo + Dispatchable<Info = DispatchInfo>,
    <T as pallet_balances::Config>::Balance: Into<U256>,
    <T as pallet_transaction_payment::Config>::OnChargeTransaction:
        OnChargeTransaction<T, Balance = <T as pallet_balances::Config>::Balance>,
{
    fn evm_user_has_permission(
        source: H160,
        weight_limit: Option<Weight>,
        access_level: u8,
    ) -> Result<(), RunnerError<pallet_evm::Error<T>>> {
        let account_id = <T as pallet_evm::Config>::AddressMapping::into_account_id(source);

        if !pallet_nac_managing::Pallet::<T>::user_has_access(account_id, access_level) {
            return Err(RunnerError {
                error: pallet_evm::Error::Undefined,
                weight: weight_limit.unwrap_or_default(),
            });
        };

        Ok(())
    }

    // TODO: need to update this structure
    fn calculate_gas(call: T::RuntimeCall) -> UsedGas {
        let call_fee =
            <T as pallet_energy_fee::Config>::CustomFee::dispatch_info_to_fee(&call, None, None);
        let gas = match call_fee {
            CallFee::Regular(fee) => fee,
            CallFee::EVM(fee) => fee,
        };
        UsedGas { standard: U256::from(0), effective: U256::from(0) }
    }
}
