use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::{ErrorCode, ErrorObject},
};
use parity_scale_codec::Encode;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{traits::Block as BlockT, FixedU128, FixedU64};
use std::sync::Arc;

// Runtime API imports.
pub use energy_generation_runtime_api::EnergyGenerationApi as EnergyGenerationRuntimeApi;

#[rpc(server, client)]
pub trait EnergyGenerationApi<BlockHash, AccountId> {
    #[method(name = "energyGeneration_energyRewardPerStake")]
    fn energy_reward_per_stake(&self, at: Option<BlockHash>) -> RpcResult<FixedU128>;

    #[method(name = "energyGeneration_validatorExposureMultiplier")]
    fn validator_exposure_multiplier(
        &self,
        at: Option<BlockHash>,
        account: AccountId,
    ) -> RpcResult<FixedU64>;

    #[method(name = "energyGeneration_cooperatorExposureMultiplier")]
    fn cooperator_exposure_multiplier(
        &self,
        at: Option<BlockHash>,
        account: AccountId,
    ) -> RpcResult<FixedU64>;
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

impl<C, Block, AccountId> EnergyGenerationApiServer<<Block as BlockT>::Hash, AccountId>
    for EnergyGeneration<C, Block>
where
    Block: BlockT,
    AccountId: Encode,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EnergyGenerationRuntimeApi<Block, AccountId>,
{
    fn energy_reward_per_stake(&self, at: Option<<Block as BlockT>::Hash>) -> RpcResult<FixedU128> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        api.energy_reward_per_stake(at).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query energy_reward_per_stake.",
                Some(e.to_string()),
            )
        })
    }

    fn validator_exposure_multiplier(
        &self,
        at: Option<<Block as BlockT>::Hash>,
        account: AccountId,
    ) -> RpcResult<FixedU64> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        api.validator_exposure_multiplier(at, account).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query validator_exposure_multiplier.",
                Some(e.to_string()),
            )
        })
    }

    fn cooperator_exposure_multiplier(
        &self,
        at: Option<<Block as BlockT>::Hash>,
        account: AccountId,
    ) -> RpcResult<FixedU64> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        api.cooperator_exposure_multiplier(at, account).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query cooperator_exposure_multiplier.",
                Some(e.to_string()),
            )
        })
    }
}
