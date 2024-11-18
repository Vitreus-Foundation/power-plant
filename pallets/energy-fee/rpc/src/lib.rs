//! # Energy Fee RPC Implementation
//!
//! JSON-RPC interface for querying energy fees and exchange rates.
//!
//! ## RPC Methods
//!
//! ### Gas Estimation
//! - `energyFee_estimateGas`: Estimates gas cost for a transaction
//! - Parameters:
//!   - Call request details
//!   - Optional block hash
//! - Returns: Estimated gas in U256
//!
//! ### Fee Estimation
//! - `energyFee_estimateCallFee`: Estimates total fee for a runtime call
//! - Parameters:
//!   - Account ID
//!   - Encoded call data
//!   - Optional block hash
//! - Returns: Fee details including breakdown
//!
//! ### Exchange Rate
//! - `energyFee_vtrsToVnrgSwapRate`: Gets current VTRS/VNRG exchange rate
//! - Parameters:
//!   - Optional block hash
//! - Returns: Exchange rate as u128
//!
//! ## Implementation Details
//! - Uses runtime API to perform calculations
//! - Falls back to best block if hash not specified
//! - Handles encoding/decoding of parameters
//! - Provides detailed error information
//! - Thread-safe client access
//!
//! The RPC interface enables external systems to estimate fees
//! and exchange rates without submitting transactions.

use ethereum_types::U256;
use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::{ErrorCode, ErrorObject},
};
use parity_scale_codec::{Codec, Decode};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;
// Runtime API imports.
pub use energy_fee_runtime_api::EnergyFeeApi as EnergyFeeRuntimeApi;
use energy_fee_runtime_api::{CallRequest, FeeDetails};

#[rpc(server, client)]
pub trait EnergyFeeApi<BlockHash, AccountId, Balance, Call> {
    #[method(name = "energyFee_estimateGas")]
    fn estimate_gas(&self, request: CallRequest, at: Option<BlockHash>) -> RpcResult<U256>;

    #[method(name = "energyFee_estimateCallFee")]
    fn estimate_call_fee(
        &self,
        account: AccountId,
        encoded_call: Bytes,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<FeeDetails<Balance>>>;

    #[method(name = "energyFee_vtrsToVnrgSwapRate")]
    fn vtrs_to_vnrg_swap_rate(&self, at: Option<BlockHash>) -> RpcResult<Option<u128>>;
}

pub struct EnergyFee<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> EnergyFee<C, B> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, Balance, Call>
    EnergyFeeApiServer<<Block as BlockT>::Hash, AccountId, Balance, Call> for EnergyFee<C, Block>
where
    Block: BlockT,
    AccountId: Codec,
    Balance: Codec,
    Call: Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EnergyFeeRuntimeApi<Block, AccountId, Balance, Call>,
{
    fn estimate_gas(
        &self,
        request: CallRequest,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<U256> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.estimate_gas(at, request).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query estimate_gas.",
                Some(e.to_string()),
            )
        })
    }

    fn estimate_call_fee(
        &self,
        account: AccountId,
        encoded_call: Bytes,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Option<FeeDetails<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );

        let call = Decode::decode(&mut &*encoded_call).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to decode call.",
                Some(e.to_string()),
            )
        })?;

        api.estimate_call_fee(at, account, call).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query estimate_call_fee.",
                Some(e.to_string()),
            )
        })
    }

    fn vtrs_to_vnrg_swap_rate(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Option<u128>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.vtrs_to_vnrg_swap_rate(at).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query vtrs_to_vnrg_swap_rate.",
                Some(e.to_string()),
            )
        })
    }
}
