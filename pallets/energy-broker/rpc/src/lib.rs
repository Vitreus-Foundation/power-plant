use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::{ErrorCode, ErrorObject},
};
use parity_scale_codec::{Decode, Encode};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{traits::Block as BlockT, FixedU128, Percent};
use std::sync::Arc;

// Runtime API imports.
pub use energy_broker_runtime_api::EnergyBrokerApi as EnergyBrokerRuntimeApi;

#[rpc(server, client)]
pub trait EnergyBrokerApi<BlockHash> {
    #[method(name = "energyBroker_estimateEnergyFromNative")]
    fn estimate_energy_from_native(
        &self,
        amount: NumberOrHex,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<NumberOrHex>>;

    #[method(name = "energyBroker_estimateNativeFromEnergy")]
    fn estimate_native_from_energy(
        &self,
        amount: NumberOrHex,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<NumberOrHex>>;

    #[method(name = "energyBroker_energyExchangeRate")]
    fn energy_exchange_rate(&self, at: Option<BlockHash>) -> RpcResult<Option<FixedU128>>;

    #[method(name = "energyBroker_currentWarehouseLevel")]
    fn current_warehouse_level(&self, at: Option<BlockHash>) -> RpcResult<Percent>;
}

pub struct EnergyBroker<C, B, AccountId, AssetKind, Balance> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<(B, AccountId, AssetKind, Balance)>,
}

impl<C, B, AccountId, AssetKind, Balance> EnergyBroker<C, B, AccountId, AssetKind, Balance> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, AssetKind, Balance> EnergyBrokerApiServer<<Block as BlockT>::Hash>
    for EnergyBroker<C, Block, AccountId, AssetKind, Balance>
where
    Block: BlockT,
    AccountId: Encode + Send + Sync + 'static,
    AssetKind: Encode + Send + Sync + 'static,
    Balance: Decode + Encode + Into<NumberOrHex> + TryFrom<NumberOrHex> + Send + Sync + 'static,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EnergyBrokerRuntimeApi<Block, AccountId, AssetKind, Balance>,
{
    fn estimate_energy_from_native(
        &self,
        amount: NumberOrHex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Option<NumberOrHex>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        let amount = amount.try_into().map_err(|_| {
            ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                "Unable to parse amount.",
                None::<()>,
            )
        })?;

        api.estimate_energy_from_native(at, amount)
            .map(|v| v.map(Into::into))
            .map_err(|e| {
                ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Unable to query estimate_energy_from_native.",
                    Some(e.to_string()),
                )
            })
    }

    fn estimate_native_from_energy(
        &self,
        amount: NumberOrHex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Option<NumberOrHex>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        let amount = amount.try_into().map_err(|_| {
            ErrorObject::owned(
                ErrorCode::InvalidParams.code(),
                "Unable to parse amount.",
                None::<()>,
            )
        })?;

        api.estimate_native_from_energy(at, amount)
            .map(|v| v.map(Into::into))
            .map_err(|e| {
                ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Unable to query estimate_native_from_energy.",
                    Some(e.to_string()),
                )
            })
    }

    fn energy_exchange_rate(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Option<FixedU128>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        api.energy_exchange_rate(at).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query energy_exchange_rate.",
                Some(e.to_string()),
            )
        })
    }

    fn current_warehouse_level(&self, at: Option<<Block as BlockT>::Hash>) -> RpcResult<Percent> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        api.current_warehouse_level(at).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query current_warehouse_level.",
                Some(e.to_string()),
            )
        })
    }
}
