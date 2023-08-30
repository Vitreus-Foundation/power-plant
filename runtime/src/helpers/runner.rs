use frame_support::pallet_prelude::Weight;
use frame_support::traits::Currency;
use pallet_evm::{runner::stack::Runner, AddressMapping};
use pallet_evm::{CallInfo, CreateInfo, Runner as EvmRunner, RunnerError};
use pallet_nac_managing;
use sp_core::{H160, H256, U256};
use sp_std::{marker::PhantomData, vec::Vec};

pub struct NacRunner<T> {
    _marker: PhantomData<T>,
}

pub const VALIDATE_ACCESS_LEVEL: u8 = 1;
pub const CREATE_ACCESS_LEVEL: u8 = 2;
pub const CALL_ACCESS_LEVEL: u8 = 1;

impl <T> EvmRunner<T> for NacRunner<T>
where
    T: pallet_evm::Config + pallet_nac_managing::Config,
    <<T as pallet_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: TryFrom<U256> + Into<U256>,
{
    type Error = pallet_evm::Error<T>;

    fn validate(source: H160, target: Option<H160>, input: Vec<u8>, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, evm_config: &pallet_evm::EvmConfig) -> Result<(), RunnerError<Self::Error>> {
        evm_user_has_permission(source, weight_limit, VALIDATE_ACCESS_LEVEL)?;

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

    fn call(source: H160, target: H160, input: Vec<u8>, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, validate: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, config: &pallet_evm::EvmConfig) -> Result<CallInfo, RunnerError<Self::Error>> {
        evm_user_has_permission(source, weight_limit, CALL_ACCESS_LEVEL)?;

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
        )
    }

    fn create(source: H160, init: Vec<u8>, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, validate: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, config: &pallet_evm::EvmConfig) -> Result<CreateInfo, RunnerError<Self::Error>> {
        evm_user_has_permission(source, weight_limit, CREATE_ACCESS_LEVEL)?;

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
        )
    }

    fn create2(source: H160, init: Vec<u8>, salt: H256, value: U256, gas_limit: u64, max_fee_per_gas: Option<U256>, max_priority_fee_per_gas: Option<U256>, nonce: Option<U256>, access_list: Vec<(H160, Vec<H256>)>, is_transactional: bool, validate: bool, weight_limit: Option<Weight>, proof_size_base_cost: Option<u64>, config: &pallet_evm::EvmConfig) -> Result<CreateInfo, RunnerError<Self::Error>> {
        evm_user_has_permission(source, weight_limit, CREATE_ACCESS_LEVEL)?;

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
        )
    }
}

fn evm_user_has_permission<T: pallet_evm::Config + pallet_nac_managing::Config>(
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
