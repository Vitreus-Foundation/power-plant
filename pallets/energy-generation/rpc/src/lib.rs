use jsonrpsee::{
    core::{Error as RpcError, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{traits::Block as BlockT, Perbill};
use std::sync::Arc;

use pallet_reputation::ReputationTier;

// Runtime API imports.
pub use energy_generation_runtime_api::EnergyGenerationApi as EnergyGenerationRuntimeApi;

#[rpc(server, client)]
pub trait EnergyGenerationApi<BlockHash> {
    #[method(name = "energyGeneration_reputationTierAdditionalReward")]
    fn reputation_tier_additional_reward(
        &self,
        tier: ReputationTier,
        at: Option<BlockHash>,
    ) -> RpcResult<Perbill>;
}

pub struct EnergyGeneration<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> EnergyGeneration<C, B> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block> EnergyGenerationApiServer<<Block as BlockT>::Hash> for EnergyGeneration<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EnergyGenerationRuntimeApi<Block>,
{
    fn reputation_tier_additional_reward(
        &self,
        tier: ReputationTier,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Perbill> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.reputation_tier_additional_reward(at, tier)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
