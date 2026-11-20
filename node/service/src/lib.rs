// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Polkadot service. Specialized wrapper over substrate service.

#![allow(
    clippy::large_enum_variant,
    clippy::needless_borrow,
    clippy::needless_borrows_for_generic_args,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_cast
)]
#![deny(unused_results)]

pub mod benchmarking;
pub mod eth;

// The upstream `polkadot-service` builder in the upstream format: our changes are its diff
// against the upstream files.
#[cfg(feature = "full-node")]
#[rustfmt::skip]
#[allow(clippy::all)]
pub mod builder;

// The upstream `polkadot-service` module as it is, so syncing it means copying the upstream
// files again.
#[rustfmt::skip]
#[allow(dead_code, clippy::all)]
mod parachains_db;

#[cfg(feature = "full-node")]
pub use crate::builder::{new_full, NewFull, NewFullParams};

#[cfg(feature = "full-node")]
use crate::builder::{new_partial, new_partial_basics};

#[cfg(feature = "full-node")]
use {
    polkadot_node_core_approval_voting as approval_voting_subsystem,
    polkadot_node_core_chain_selection as chain_selection_subsystem,
};

use polkadot_node_subsystem_util::database::Database;
use polkadot_overseer::SpawnGlue;

#[cfg(feature = "full-node")]
pub use {
    polkadot_overseer::{Handle, Overseer, OverseerConnector, OverseerHandle},
    polkadot_primitives::runtime_api::ParachainHost,
    polkadot_service::{
        overseer::{
            self, ExtendedOverseerGenArgs, OverseerGen, OverseerGenArgs, ValidatorOverseerGen,
        },
        workers, Chain, Error, IdentifyVariant, IsParachainNode, SelectRelayChain,
    },
    sc_client_api::AuxStore,
    sp_authority_discovery::AuthorityDiscoveryApi,
    sp_blockchain::{HeaderBackend, HeaderMetadata},
    sp_consensus_babe::BabeApi,
};

use std::sync::Arc;

use sc_service::SpawnTaskHandle;

pub use polkadot_primitives::{Block, BlockId, BlockNumber, CollatorPair, Hash, Id as ParaId};
pub use sc_client_api::{Backend, CallExecutor};
pub use sc_consensus::{BlockImport, LongestChain};
pub use sc_executor::NativeExecutionDispatch;
use sc_executor::WasmExecutor;
pub use sc_service::{
    config::{DatabaseSource, PrometheusConfig},
    ChainSpec, Configuration, Error as SubstrateServiceError, PruningMode, Role, TFullBackend,
    TFullCallExecutor, TFullClient, TaskManager, TransactionPoolOptions,
};
pub use sp_api::{ApiRef, ConstructRuntimeApi, Core as CoreApi, ProvideRuntimeApi};
pub use sp_consensus::{Proposal, SelectChain};
pub use sp_runtime::{
    generic,
    traits::{self as runtime_traits, BlakeTwo256, Block as BlockT, Header as HeaderT, NumberFor},
};

pub use vitreus_power_plant_runtime::RuntimeApi;

use eth::{EthConfiguration, FrontierBackend};

#[cfg(feature = "full-node")]
pub type FullBackend = sc_service::TFullBackend<Block>;

#[cfg(feature = "full-node")]
pub type FullClient = sc_service::TFullClient<
    Block,
    RuntimeApi,
    WasmExecutor<(sp_io::SubstrateHostFunctions, frame_benchmarking::benchmarking::HostFunctions)>,
>;

/// The minimum period of blocks on which justifications will be
/// imported and generated.
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

/// The number of hours to keep finalized data in the availability store for live networks.
const KEEP_FINALIZED_FOR_LIVE_NETWORKS: u32 = 25;

#[cfg(feature = "full-node")]
pub fn open_database(db_source: &DatabaseSource) -> Result<Arc<dyn Database>, Error> {
    let parachains_db = match db_source {
        DatabaseSource::RocksDb { path, .. } => parachains_db::open_creating_rocksdb(
            path.clone(),
            parachains_db::CacheSizes::default(),
        )?,
        DatabaseSource::ParityDb { path, .. } => parachains_db::open_creating_paritydb(
            path.parent().ok_or(Error::DatabasePathRequired)?.into(),
            parachains_db::CacheSizes::default(),
        )?,
        DatabaseSource::Auto { paritydb_path, rocksdb_path, .. } => {
            if paritydb_path.is_dir() && paritydb_path.exists() {
                parachains_db::open_creating_paritydb(
                    paritydb_path.parent().ok_or(Error::DatabasePathRequired)?.into(),
                    parachains_db::CacheSizes::default(),
                )?
            } else {
                parachains_db::open_creating_rocksdb(
                    rocksdb_path.clone(),
                    parachains_db::CacheSizes::default(),
                )?
            }
        },
        DatabaseSource::Custom { .. } => {
            unimplemented!("No polkadot subsystem db for custom source.");
        },
    };
    Ok(parachains_db)
}

/// The methods `polkadot-service` keeps private on [`IsParachainNode`], for the builder.
#[cfg(feature = "full-node")]
trait IsParachainNodeHelper {
    fn is_collator(&self) -> bool;
    fn is_full_node(&self) -> bool;
    fn is_running_alongside_parachain_node(&self) -> bool;
}

#[cfg(feature = "full-node")]
impl IsParachainNodeHelper for IsParachainNode {
    fn is_collator(&self) -> bool {
        matches!(self, Self::Collator(_))
    }

    fn is_full_node(&self) -> bool {
        matches!(self, Self::FullNode)
    }

    fn is_running_alongside_parachain_node(&self) -> bool {
        self.is_collator() || self.is_full_node()
    }
}

#[cfg(feature = "full-node")]
macro_rules! chain_ops {
    ($config:expr, $eth_config:expr, $telemetry_worker_handle:expr) => {{
        let telemetry_worker_handle = $telemetry_worker_handle;
        let eth_config = $eth_config;
        let mut config = $config;
        let basics = new_partial_basics(config, telemetry_worker_handle)?;

        use ::sc_consensus::LongestChain;
        // use the longest chain selection, since there is no overseer available
        let chain_selection = LongestChain::new(basics.backend.clone());

        let sc_service::PartialComponents {
            client,
            backend,
            import_queue,
            task_manager,
            other,
            ..
        } = new_partial::<LongestChain<_, Block>>(
            &mut config,
            &eth_config,
            basics,
            chain_selection,
        )?;

        Ok((client, backend, import_queue, task_manager, other.5))
    }};
}

/// Builds a new object suitable for chain operations.
#[cfg(feature = "full-node")]
pub fn new_chain_ops(
    config: &mut Configuration,
    eth_config: &EthConfiguration,
) -> Result<
    (
        Arc<FullClient>,
        Arc<FullBackend>,
        sc_consensus::BasicQueue<Block>,
        TaskManager,
        FrontierBackend<Block, FullClient>,
    ),
    Error,
> {
    config.keystore = sc_service::config::KeystoreConfig::InMemory;

    chain_ops!(config, eth_config, None)
}

/// Build a full node.
#[cfg(feature = "full-node")]
pub fn build_full<OverseerGenerator: OverseerGen>(
    config: Configuration,
    eth_config: EthConfiguration,
    params: NewFullParams<OverseerGenerator>,
) -> Result<NewFull, Error> {
    match config.network.network_backend {
        sc_network::config::NetworkBackendType::Libp2p => {
            new_full::<_, sc_network::NetworkWorker<Block, Hash>>(config, eth_config, params)
        },
        sc_network::config::NetworkBackendType::Litep2p => {
            new_full::<_, sc_network::Litep2pNetworkBackend>(config, eth_config, params)
        },
    }
}

/// Reverts the node state down to at most the last finalized block.
///
/// In particular this reverts:
/// - `ApprovalVotingSubsystem` data in the parachains-db;
/// - `ChainSelectionSubsystem` data in the parachains-db;
/// - Low level Babe and Grandpa consensus data.
#[cfg(feature = "full-node")]
pub fn revert_backend(
    client: Arc<FullClient>,
    backend: Arc<FullBackend>,
    blocks: BlockNumber,
    config: Configuration,
    task_handle: SpawnTaskHandle,
) -> Result<(), Error> {
    let best_number = client.info().best_number;
    let finalized = client.info().finalized_number;
    let revertible = blocks.min(best_number - finalized);

    if revertible == 0 {
        return Ok(());
    }

    let number = best_number - revertible;
    let hash = client.block_hash_from_id(&BlockId::Number(number))?.ok_or(
        sp_blockchain::Error::Backend(format!(
            "Unexpected hash lookup failure for block number: {}",
            number
        )),
    )?;

    let parachains_db = open_database(&config.database)
        .map_err(|err| sp_blockchain::Error::Backend(err.to_string()))?;

    revert_approval_voting(parachains_db.clone(), hash, task_handle)?;
    revert_chain_selection(parachains_db, hash)?;
    // Revert Substrate consensus related components
    sc_consensus_babe::revert(client.clone(), backend, blocks)?;
    sc_consensus_grandpa::revert(client, blocks)?;

    Ok(())
}

fn revert_chain_selection(db: Arc<dyn Database>, hash: Hash) -> sp_blockchain::Result<()> {
    let config = chain_selection_subsystem::Config {
        col_data: parachains_db::REAL_COLUMNS.col_chain_selection_data,
        stagnant_check_interval: chain_selection_subsystem::StagnantCheckInterval::never(),
        stagnant_check_mode: chain_selection_subsystem::StagnantCheckMode::PruneOnly,
    };

    let chain_selection = chain_selection_subsystem::ChainSelectionSubsystem::new(config, db);

    chain_selection
        .revert_to(hash)
        .map_err(|err| sp_blockchain::Error::Backend(err.to_string()))
}

fn revert_approval_voting(
    db: Arc<dyn Database>,
    hash: Hash,
    task_handle: SpawnTaskHandle,
) -> sp_blockchain::Result<()> {
    let config = approval_voting_subsystem::Config {
        col_approval_data: parachains_db::REAL_COLUMNS.col_approval_data,
        slot_duration_millis: Default::default(),
    };

    let approval_voting = approval_voting_subsystem::ApprovalVotingSubsystem::with_config(
        config,
        db,
        Arc::new(sc_keystore::LocalKeystore::in_memory()),
        Box::new(sp_consensus::NoNetwork),
        approval_voting_subsystem::Metrics::default(),
        Arc::new(SpawnGlue(task_handle)),
    );

    approval_voting
        .revert_to(hash)
        .map_err(|err| sp_blockchain::Error::Backend(err.to_string()))
}
