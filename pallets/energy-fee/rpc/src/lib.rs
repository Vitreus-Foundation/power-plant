use ethereum_types::U256;
use jsonrpsee::{
    core::{Error as RpcError, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

// Runtime API imports.
use energy_fee_runtime_api::CallRequest;
pub use energy_fee_runtime_api::EnergyFeeApi as EnergyFeeRuntimeApi;

#[rpc(server, client)]
pub trait EnergyFeeApi<BlockHash> {
    #[method(name = "energyFee_estimateGas")]
    fn estimate_gas(&self, request: CallRequest, at: Option<BlockHash>) -> RpcResult<U256>;
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

impl<C, Block> EnergyFeeApiServer<<Block as BlockT>::Hash> for EnergyFee<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EnergyFeeRuntimeApi<Block>,
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
}
