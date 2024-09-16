use ethereum_types::U256;
use jsonrpsee::{
    core::{Error as RpcError, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
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
        api.estimate_gas(at, request)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
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

        let call = Decode::decode(&mut &*encoded_call)
            .map_err(|e| CallError::InvalidParams(anyhow::Error::new(e)))?;

        api.estimate_call_fee(at, account, call)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
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
        api.vtrs_to_vnrg_swap_rate(at)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
