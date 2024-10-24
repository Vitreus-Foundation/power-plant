///
/// # Script Overview
///
/// This Rust script defines an RPC (Remote Procedure Call) API for the `EnergyGeneration` pallet within a Substrate-based blockchain. It includes methods to query additional rewards based on reputation tiers and to retrieve current energy per stake currency. The script implements the RPC trait using `jsonrpsee` and integrates runtime APIs for interacting with the blockchain state.
///
/// # Key Components
///
/// - **RPC Trait Definition**:
///   - The `EnergyGenerationApi` trait, marked with `#[rpc(server, client)]`, defines the RPC methods for querying blockchain data.
///   - `reputation_tier_additional_reward(tier: ReputationTier, at: Option<BlockHash>) -> RpcResult<Perbill>`: Retrieves the additional reward percentage (`Perbill`) for a given reputation tier.
///   - `current_energy_per_stake_currency(at: Option<BlockHash>) -> RpcResult<u128>`: Retrieves the current energy value per stake currency at a specific block.
///
/// - **RPC Implementation**:
///   - The `EnergyGeneration` struct contains the RPC logic and interacts with the runtime API via the client.
///   - The methods fetch runtime data by interacting with the blockchain through `ProvideRuntimeApi` and `HeaderBackend`.
///
/// # Developer Notes
///
/// - The `EnergyGenerationApi` trait is designed for use in a Substrate environment, leveraging the runtime API to interact with blockchain state.
/// - The implementation of each RPC method utilizes error handling, returning an `ErrorObject` if an internal error occurs during runtime API access.
/// - The `EnergyGeneration` struct is responsible for managing RPC interactions, using the best block as the default when a specific block hash is not provided.
///
/// # Usage
///
/// This script should be used by developers looking to expose on-chain energy generation metrics and reputation-based rewards via RPC. It is designed for seamless integration with Substrate nodes that provide the necessary runtime API implementations.
///
/// - **Dependencies**:
///   - `jsonrpsee`: Provides the framework for creating JSON-RPC servers and clients.
///   - `sp_api` and `sp_blockchain`: Used to interact with the Substrate runtime API and blockchain backend.
///   - `energy_generation_runtime_api`: This provides the definitions for the energy generation-related runtime APIs.
///
/// - **Extending Functionality**:
///   - Developers can extend the RPC methods or add new methods to expose more information about the energy generation mechanism or other related features.
///   - Proper error handling and testing are crucial to ensure reliability when interacting with blockchain state.
///
/// # Example Usage
///
/// Integrate this RPC module in the node service to allow clients to remotely query the energy generation statistics and reputation-based reward structures.


use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::{ErrorCode, ErrorObject},
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

    #[method(name = "energyGeneration_currentEnergyPerStakeCurrency")]
    fn current_energy_per_stake_currency(&self, at: Option<BlockHash>) -> RpcResult<u128>;
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
        api.reputation_tier_additional_reward(at, tier).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query reputation_tier_additional_reward.",
                Some(e.to_string()),
            )
        })
    }

    fn current_energy_per_stake_currency(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<u128> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.current_energy_per_stake_currency(at).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query current_energy_per_stake_currency.",
                Some(e.to_string()),
            )
        })
    }
}
