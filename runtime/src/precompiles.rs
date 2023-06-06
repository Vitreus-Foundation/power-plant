use pallet_evm::{
    IsPrecompileResult, Precompile, PrecompileFailure, PrecompileHandle, PrecompileResult,
    PrecompileSet,
};
use sp_core::{Get, H160, U256};
use sp_std::convert::TryFrom;
use sp_std::marker::PhantomData;

use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};

use pallet_evm::{GasWeightMapping, Log};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};

// use balance_erc20::*;
use pallet_evm_precompile_balances_erc20::{Erc20BalancesPrecompile, Erc20Metadata, BalanceOf};

// mod balance_erc20;

pub type EvmResult<T = ()> = Result<T, PrecompileFailure>;

pub struct FrontierPrecompiles<R>(PhantomData<R>);

impl<R> FrontierPrecompiles<R>
where
    R: pallet_evm::Config,
{
    pub fn new() -> Self {
        Self(Default::default())
    }
    pub fn used_addresses() -> [H160; 8] {
        [hash(1), hash(2), hash(3), hash(4), hash(5), hash(1024), hash(1025), hash(2048)]
    }
}

impl<Runtime> PrecompileSet for FrontierPrecompiles<Runtime>
where
    Runtime: pallet_balances::Config + pallet_evm::Config + pallet_timestamp::Config,
    Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
    Runtime::RuntimeCall: From<pallet_balances::Call<Runtime>>,
    <Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
    BalanceOf<Runtime>: TryFrom<U256> + Into<U256>,
    <Runtime as pallet_timestamp::Config>::Moment: Into<U256>,
{
    fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
        match handle.code_address() {
            // Ethereum precompiles :
            a if a == hash(1) => Some(ECRecover::execute(handle)),
            a if a == hash(2) => Some(Sha256::execute(handle)),
            a if a == hash(3) => Some(Ripemd160::execute(handle)),
            a if a == hash(4) => Some(Identity::execute(handle)),
            a if a == hash(5) => Some(Modexp::execute(handle)),
            // Non-Frontier specific nor Ethereum precompiles :
            a if a == hash(1024) => Some(Sha3FIPS256::execute(handle)),
            a if a == hash(1025) => Some(ECRecoverPublicKey::execute(handle)),
            // vitreus specific precompiles
            // a if a == hash(2048) => {
            //     Some(balance_erc20::Erc20BalancesPrecompile::<Runtime>::execute(handle))
            // },
            _ => None,
        }
    }

    fn is_precompile(&self, address: H160, _gas: u64) -> IsPrecompileResult {
        IsPrecompileResult::Answer {
            is_precompile: Self::used_addresses().contains(&address),
            extra_cost: 0,
        }
    }
}

fn hash(a: u64) -> H160 {
    H160::from_low_u64_be(a)
}

// /// Cost of a Substrate DB write in gas.
// pub fn db_write_gas_cost<Runtime: pallet_evm::Config + frame_system::Config>() -> u64 {
//     <Runtime as pallet_evm::Config>::GasWeightMapping::weight_to_gas(
//         <Runtime as frame_system::Config>::DbWeight::get().writes(1),
//     )
// }
//
// /// Cost of a Substrate DB read in gas.
// pub fn db_read_gas_cost<Runtime: pallet_evm::Config + frame_system::Config>() -> u64 {
//     <Runtime as pallet_evm::Config>::GasWeightMapping::weight_to_gas(
//         <Runtime as frame_system::Config>::DbWeight::get().reads(1),
//     )
// }
//
// pub fn log_costs(topics: usize, data_len: usize) -> EvmResult<u64> {
//     // Cost calculation is copied from EVM code that is not publicly exposed by the crates.
//     // https://github.com/rust-blockchain/evm/blob/master/gasometer/src/costs.rs#L148
//
//     const G_LOG: u64 = 375;
//     const G_LOGDATA: u64 = 8;
//     const G_LOGTOPIC: u64 = 375;
//
//     let topic_cost = G_LOGTOPIC
//         .checked_mul(topics as u64)
//         .ok_or(PrecompileFailure::Error { exit_status: ExitError::OutOfGas })?;
//
//     let data_cost = G_LOGDATA
//         .checked_mul(data_len as u64)
//         .ok_or(PrecompileFailure::Error { exit_status: ExitError::OutOfGas })?;
//
//     G_LOG
//         .checked_add(topic_cost)
//         .ok_or(PrecompileFailure::Error { exit_status: ExitError::OutOfGas })?
//         .checked_add(data_cost)
//         .ok_or(PrecompileFailure::Error { exit_status: ExitError::OutOfGas })
// }
//
// /// Create a 3-topics log.
// pub fn log3(
//     address: impl Into<H160>,
//     topic0: impl Into<H256>,
//     topic1: impl Into<H256>,
//     topic2: impl Into<H256>,
//     data: impl Into<Vec<u8>>,
// ) -> Log {
//     Log {
//         address: address.into(),
//         topics: vec![topic0.into(), topic1.into(), topic2.into()],
//         data: data.into(),
//     }
// }
