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

#![deny(unused_results)]

pub mod benchmarking;
pub mod eth;
mod fake_runtime_api;
mod grandpa_support;
mod parachains_db;
mod relay_chain_selection;

#[cfg(feature = "full-node")]
pub mod overseer;

#[cfg(feature = "full-node")]
pub use self::overseer::{OverseerGen, OverseerGenArgs, RealOverseerGen};

#[cfg(test)]
mod tests;

#[cfg(feature = "full-node")]
use {
    grandpa::{self, FinalityProofProvider as GrandpaFinalityProofProvider},
    gum::info,
    polkadot_node_core_approval_voting::{
        self as approval_voting_subsystem, Config as ApprovalVotingConfig,
    },
    polkadot_node_core_av_store::Config as AvailabilityConfig,
    polkadot_node_core_av_store::Error as AvailabilityError,
    polkadot_node_core_candidate_validation::Config as CandidateValidationConfig,
    polkadot_node_core_chain_selection::{
        self as chain_selection_subsystem, Config as ChainSelectionConfig,
    },
    polkadot_node_core_dispute_coordinator::Config as DisputeCoordinatorConfig,
    polkadot_node_network_protocol::{
        peer_set::PeerSetProtocolNames, request_response::ReqProtocolNames,
    },
    sc_client_api::BlockBackend,
    sc_transaction_pool_api::OffchainTransactionPoolFactory,
    sp_core::traits::SpawnNamed, sp_core::U256,
    sp_trie::PrefixedMemoryDB,
};

use polkadot_node_subsystem_util::database::Database;

#[cfg(feature = "full-node")]
pub use {
    polkadot_overseer::{Handle, Overseer, OverseerConnector, OverseerHandle},
    polkadot_primitives::runtime_api::ParachainHost,
    relay_chain_selection::SelectRelayChain,
    sc_client_api::AuxStore,
    sp_authority_discovery::AuthorityDiscoveryApi,
    sp_blockchain::{HeaderBackend, HeaderMetadata},
    sp_consensus_babe::BabeApi,
};

#[cfg(feature = "full-node")]
use polkadot_node_subsystem::jaeger;

use std::{cell::RefCell, sync::Arc, time::Duration};
use std::path::Path;

use prometheus_endpoint::Registry;
#[cfg(feature = "full-node")]
use service::KeystoreContainer;
use service::RpcHandlers;
use telemetry::TelemetryWorker;
#[cfg(feature = "full-node")]
use telemetry::{Telemetry, TelemetryWorkerHandle};

pub use consensus_common::{Proposal, SelectChain};
use fc_consensus::FrontierBlockImport;
use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
use mmr_gadget::MmrGadget;
pub use polkadot_primitives::{Block, BlockId, BlockNumber, CollatorPair, Hash, Id as ParaId};
pub use sc_client_api::{Backend, CallExecutor};
pub use sc_consensus::{BlockImport, LongestChain};
pub use sc_executor::NativeExecutionDispatch;
use sc_executor::{HeapAllocStrategy, WasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY};
pub use service::{
    config::{DatabaseSource, PrometheusConfig},
    ChainSpec, Configuration, Error as SubstrateServiceError, PruningMode, Role, RuntimeGenesis,
    TFullBackend, TFullCallExecutor, TFullClient, TaskManager, TransactionPoolOptions,
};
pub use sp_api::{ApiRef, ConstructRuntimeApi, Core as CoreApi, ProvideRuntimeApi, StateBackend, TransactionFor};
pub use sp_runtime::{
    generic,
    traits::{
        self as runtime_traits, BlakeTwo256, Block as BlockT, HashFor, Header as HeaderT, NumberFor,
    },
};

use eth::{BackendType, db_config_dir, EthConfiguration, FrontierBackend, FrontierPartialComponents, new_frontier_partial, spawn_frontier_tasks};
use vitreus_power_plant_runtime::{RuntimeApi, TransactionConverter};

#[cfg(feature = "kusama-native")]
pub use {kusama_runtime, kusama_runtime_constants};
#[cfg(feature = "polkadot-native")]
pub use {polkadot_runtime, polkadot_runtime_constants};
#[cfg(feature = "rococo-native")]
pub use {rococo_runtime, rococo_runtime_constants};
#[cfg(feature = "westend-native")]
pub use {westend_runtime, westend_runtime_constants};

#[cfg(feature = "full-node")]
pub type FullBackend = service::TFullBackend<Block>;

#[cfg(feature = "full-node")]
pub type FullClient = service::TFullClient<
    Block,
    RuntimeApi,
    WasmExecutor<(sp_io::SubstrateHostFunctions, frame_benchmarking::benchmarking::HostFunctions)>,
>;

/// Provides the header and block number for a hash.
///
/// Decouples `sc_client_api::Backend` and `sp_blockchain::HeaderBackend`.
pub trait HeaderProvider<Block, Error = sp_blockchain::Error>: Send + Sync + 'static
where
    Block: BlockT,
    Error: std::fmt::Debug + Send + Sync + 'static,
{
    /// Obtain the header for a hash.
    fn header(
        &self,
        hash: <Block as BlockT>::Hash,
    ) -> Result<Option<<Block as BlockT>::Header>, Error>;
    /// Obtain the block number for a hash.
    fn number(
        &self,
        hash: <Block as BlockT>::Hash,
    ) -> Result<Option<<<Block as BlockT>::Header as HeaderT>::Number>, Error>;
}

impl<Block, T> HeaderProvider<Block> for T
where
    Block: BlockT,
    T: sp_blockchain::HeaderBackend<Block> + 'static,
{
    fn header(
        &self,
        hash: Block::Hash,
    ) -> sp_blockchain::Result<Option<<Block as BlockT>::Header>> {
        <Self as sp_blockchain::HeaderBackend<Block>>::header(self, hash)
    }
    fn number(
        &self,
        hash: Block::Hash,
    ) -> sp_blockchain::Result<Option<<<Block as BlockT>::Header as HeaderT>::Number>> {
        <Self as sp_blockchain::HeaderBackend<Block>>::number(self, hash)
    }
}

/// Decoupling the provider.
///
/// Mandated since `trait HeaderProvider` can only be
/// implemented once for a generic `T`.
pub trait HeaderProviderProvider<Block>: Send + Sync + 'static
where
    Block: BlockT,
{
    type Provider: HeaderProvider<Block> + 'static;

    fn header_provider(&self) -> &Self::Provider;
}

impl<Block, T> HeaderProviderProvider<Block> for T
where
    Block: BlockT,
    T: sc_client_api::Backend<Block> + 'static,
{
    type Provider = <T as sc_client_api::Backend<Block>>::Blockchain;

    fn header_provider(&self) -> &Self::Provider {
        self.blockchain()
    }
}

/// Available Sealing methods.
#[derive(Copy, Clone, Debug, Default, clap::ValueEnum)]
pub enum Sealing {
    /// Seal using rpc method.
    #[default]
    Manual,
    /// Seal when transaction is executed.
    Instant,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    AddrFormatInvalid(#[from] std::net::AddrParseError),

    #[error(transparent)]
    Sub(#[from] SubstrateServiceError),

    #[error(transparent)]
    Blockchain(#[from] sp_blockchain::Error),

    #[error(transparent)]
    Consensus(#[from] consensus_common::Error),

    #[error("Failed to create an overseer")]
    Overseer(#[from] polkadot_overseer::SubsystemError),

    #[error(transparent)]
    Prometheus(#[from] prometheus_endpoint::PrometheusError),

    #[error(transparent)]
    Telemetry(#[from] telemetry::Error),

    #[error(transparent)]
    Jaeger(#[from] polkadot_node_subsystem::jaeger::JaegerError),

    #[cfg(feature = "full-node")]
    #[error(transparent)]
    Availability(#[from] AvailabilityError),

    #[error("Authorities require the real overseer implementation")]
    AuthoritiesRequireRealOverseer,

    #[cfg(feature = "full-node")]
    #[error("Creating a custom database is required for validators")]
    DatabasePathRequired,

    #[cfg(feature = "full-node")]
    #[error("Expected at least one of polkadot, kusama, westend or rococo runtime feature")]
    NoRuntime,
}

/// Can be called for a `Configuration` to identify which network the configuration targets.
pub trait IdentifyVariant {
    /// Returns true if this configuration is for a development network.
    fn is_dev(&self) -> bool;
}

impl IdentifyVariant for Box<dyn ChainSpec> {
    fn is_dev(&self) -> bool {
        self.id().ends_with("dev")
    }
}

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

/// Initialize the `Jeager` collector. The destination must listen
/// on the given address and port for `UDP` packets.
#[cfg(any(test, feature = "full-node"))]
fn jaeger_launch_collector_with_agent(
    spawner: impl SpawnNamed,
    config: &Configuration,
    agent: Option<std::net::SocketAddr>,
) -> Result<(), Error> {
    if let Some(agent) = agent {
        let cfg = jaeger::JaegerConfig::builder()
            .agent(agent)
            .named(&config.network.node_name)
            .build();

        jaeger::Jaeger::new(cfg).launch(spawner)?;
    }
    Ok(())
}

#[cfg(feature = "full-node")]
type FullSelectChain = relay_chain_selection::SelectRelayChain<FullBackend>;
#[cfg(feature = "full-node")]
type FullGrandpaBlockImport<ChainSelection = FullSelectChain> =
    grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, ChainSelection>;
#[cfg(feature = "full-node")]
type FullBeefyBlockImport<InnerBlockImport> =
    beefy::import::BeefyBlockImport<Block, FullBackend, FullClient, InnerBlockImport>;

type BoxBlockImport<Client> = sc_consensus::BoxBlockImport<Block, TransactionFor<Client, Block>>;

#[cfg(feature = "full-node")]
struct Basics {
    task_manager: TaskManager,
    client: Arc<FullClient>,
    backend: Arc<FullBackend>,
    keystore_container: KeystoreContainer,
    telemetry: Option<Telemetry>,
}

#[cfg(feature = "full-node")]
fn new_partial_basics(
    config: &mut Configuration,
    jaeger_agent: Option<std::net::SocketAddr>,
    telemetry_worker_handle: Option<TelemetryWorkerHandle>,
) -> Result<Basics, Error> {
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(move |endpoints| -> Result<_, telemetry::Error> {
            let (worker, mut worker_handle) = if let Some(worker_handle) = telemetry_worker_handle {
                (None, worker_handle)
            } else {
                let worker = TelemetryWorker::new(16)?;
                let worker_handle = worker.handle();
                (Some(worker), worker_handle)
            };
            let telemetry = worker_handle.new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let heap_pages = config
        .default_heap_pages
        .map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |h| HeapAllocStrategy::Static { extra_pages: h as _ });

    let executor = WasmExecutor::builder()
        .with_execution_method(config.wasm_method)
        .with_onchain_heap_alloc_strategy(heap_pages)
        .with_offchain_heap_alloc_strategy(heap_pages)
        .with_max_runtime_instances(config.max_runtime_instances)
        .with_runtime_cache_size(config.runtime_cache_size)
        .build();

    let (client, backend, keystore_container, task_manager) =
        service::new_full_parts::<Block, RuntimeApi, _>(
            &config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        if let Some(worker) = worker {
            task_manager.spawn_handle().spawn(
                "telemetry",
                Some("telemetry"),
                Box::pin(worker.run()),
            );
        }
        telemetry
    });

    jaeger_launch_collector_with_agent(task_manager.spawn_handle(), &*config, jaeger_agent)?;

    Ok(Basics { task_manager, client, backend, keystore_container, telemetry })
}

#[cfg(feature = "full-node")]
fn new_partial<ChainSelection>(
    config: &mut Configuration,
    eth_config: &EthConfiguration,
    Basics { task_manager, backend, client, keystore_container, telemetry }: Basics,
    select_chain: ChainSelection,
) -> Result<
    service::PartialComponents<
        FullClient,
        FullBackend,
        ChainSelection,
        sc_consensus::DefaultImportQueue<Block, FullClient>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            impl Fn(
                polkadot_rpc::SubscriptionTaskExecutor
            ) -> (polkadot_rpc::BabeDeps, polkadot_rpc::GrandpaDeps<FullBackend>, polkadot_rpc::BeefyDeps),
            (
                babe::BabeBlockImport<
                    Block,
                    FullClient,
                    FullBeefyBlockImport<FullGrandpaBlockImport<ChainSelection>>,
                >,
                grandpa::LinkHalf<Block, FullClient, ChainSelection>,
                babe::BabeLink<Block>,
                beefy::BeefyVoterLinks<Block>,
            ),
            grandpa::SharedVoterState,
            sp_consensus_babe::SlotDuration,
            Option<Telemetry>,
            FrontierBackend,
            Arc<fc_rpc::OverrideHandle<Block>>,
        ),
    >,
    Error,
>
where
    ChainSelection: 'static + SelectChain<Block>,
{
    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let (grandpa_block_import, grandpa_link) = grandpa::block_import_with_authority_set_hard_forks(
        client.clone(),
        &(client.clone() as Arc<_>),
        select_chain.clone(),
        Vec::new(),
        telemetry.as_ref().map(|x| x.handle()),
    )?;
    let justification_import = grandpa_block_import.clone();

    let overrides = fc_storage::overrides_handle(client.clone());
    let frontier_backend = match eth_config.frontier_backend_type {
        BackendType::KeyValue => FrontierBackend::KeyValue(fc_db::kv::Backend::open(
            Arc::clone(&client),
            &config.database,
            &db_config_dir(config),
        ).map_err(SubstrateServiceError::Other)?),
        BackendType::Sql => {
            let db_path = db_config_dir(config).join("sql");
            std::fs::create_dir_all(&db_path).expect("failed creating sql db directory");
            let backend = futures::executor::block_on(fc_db::sql::Backend::new(
                fc_db::sql::BackendConfig::Sqlite(fc_db::sql::SqliteBackendConfig {
                    path: Path::new("sqlite:///")
                        .join(db_path)
                        .join("frontier.db3")
                        .to_str()
                        .unwrap(),
                    create_if_missing: true,
                    thread_count: eth_config.frontier_sql_backend_thread_count,
                    cache_size: eth_config.frontier_sql_backend_cache_size,
                }),
                eth_config.frontier_sql_backend_pool_size,
                std::num::NonZeroU32::new(eth_config.frontier_sql_backend_num_ops_timeout),
                overrides.clone(),
            ))
                .unwrap_or_else(|err| panic!("failed creating sql backend: {:?}", err));
            FrontierBackend::Sql(backend)
        },
    };

    let _frontier_block_import =
        FrontierBlockImport::new(grandpa_block_import.clone(), client.clone());

    let (beefy_block_import, beefy_voter_links, beefy_rpc_links) =
        beefy::beefy_block_import_and_links(
            grandpa_block_import,
            backend.clone(),
            client.clone(),
            config.prometheus_registry().cloned(),
        );

    let babe_config = babe::configuration(&*client)?;
    let (block_import, babe_link) =
        babe::block_import(babe_config.clone(), beefy_block_import, client.clone())?;

    let slot_duration = babe_link.config().slot_duration();
    let target_gas_price = eth_config.target_gas_price;
    let (import_queue, babe_worker_handle) = babe::import_queue(babe::ImportQueueParams {
        link: babe_link.clone(),
        block_import: block_import.clone(),
        justification_import: Some(Box::new(justification_import)),
        client: client.clone(),
        select_chain: select_chain.clone(),
        create_inherent_data_providers: move |_, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

            let slot =
				sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*timestamp,
					slot_duration,
				);
            let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));

            Ok((slot, timestamp, dynamic_fee))
        },
        spawner: &task_manager.spawn_essential_handle(),
        registry: config.prometheus_registry(),
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
    })?;

    let justification_stream = grandpa_link.justification_stream();
    let shared_authority_set = grandpa_link.shared_authority_set().clone();
    let shared_voter_state = grandpa::SharedVoterState::empty();
    let finality_proof_provider = GrandpaFinalityProofProvider::new_for_service(
        backend.clone(),
        Some(shared_authority_set.clone()),
    );

    let import_setup = (block_import, grandpa_link, babe_link, beefy_voter_links);
    let rpc_setup = shared_voter_state.clone();

    let rpc_deps_builder = {
        let keystore = keystore_container.keystore();

        move |subscription_executor: polkadot_rpc::SubscriptionTaskExecutor|
              -> (polkadot_rpc::BabeDeps, polkadot_rpc::GrandpaDeps<FullBackend>, polkadot_rpc::BeefyDeps) {

            let babe = polkadot_rpc::BabeDeps {
                worker_handle: babe_worker_handle.clone(),
                keystore: keystore.clone(),
            };
            let grandpa = polkadot_rpc::GrandpaDeps {
                shared_voter_state: shared_voter_state.clone(),
                shared_authority_set: shared_authority_set.clone(),
                justification_stream: justification_stream.clone(),
                subscription_executor: subscription_executor.clone(),
                finality_provider: finality_proof_provider.clone(),
            };
            let beefy = polkadot_rpc::BeefyDeps {
                beefy_finality_proof_stream: beefy_rpc_links.from_voter_justif_stream.clone(),
                beefy_best_block_stream: beefy_rpc_links.from_voter_best_beefy_stream.clone(),
                subscription_executor,
            };

            (babe, grandpa, beefy)
        }
    };

    Ok(service::PartialComponents {
        client,
        backend,
        task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other: (rpc_deps_builder, import_setup, rpc_setup, slot_duration, telemetry, frontier_backend, overrides),
    })
}

#[cfg(feature = "full-node")]
pub struct NewFull {
    pub task_manager: TaskManager,
    pub client: Arc<FullClient>,
    pub overseer_handle: Option<Handle>,
    pub network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
    pub sync_service: Arc<sc_network_sync::SyncingService<Block>>,
    pub rpc_handlers: RpcHandlers,
    pub backend: Arc<FullBackend>,
}

/// Is this node a collator?
#[cfg(feature = "full-node")]
#[derive(Clone)]
pub enum IsCollator {
    /// This node is a collator.
    Yes(CollatorPair),
    /// This node is not a collator.
    No,
}

#[cfg(feature = "full-node")]
impl std::fmt::Debug for IsCollator {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        use sp_core::Pair;
        match self {
            IsCollator::Yes(pair) => write!(fmt, "Yes({})", pair.public()),
            IsCollator::No => write!(fmt, "No"),
        }
    }
}

#[cfg(feature = "full-node")]
impl IsCollator {
    /// Is this a collator?
    fn is_collator(&self) -> bool {
        matches!(self, Self::Yes(_))
    }
}

pub const AVAILABILITY_CONFIG: AvailabilityConfig = AvailabilityConfig {
    col_data: parachains_db::REAL_COLUMNS.col_availability_data,
    col_meta: parachains_db::REAL_COLUMNS.col_availability_meta,
};

/// Create a new full node of arbitrary runtime and executor.
///
/// This is an advanced feature and not recommended for general use. Generally, `build_full` is
/// a better choice.
///
/// `overseer_enable_anyways` always enables the overseer, based on the provided `OverseerGenerator`,
/// regardless of the role the node has. The relay chain selection (longest or disputes-aware) is
/// still determined based on the role of the node. Likewise for authority discovery.
#[cfg(feature = "full-node")]
pub fn new_full<OverseerGenerator>(
    mut config: Configuration,
    eth_config: EthConfiguration,
    is_collator: IsCollator,
    grandpa_pause: Option<(u32, u32)>,
    enable_beefy: bool,
    jaeger_agent: Option<std::net::SocketAddr>,
    telemetry_worker_handle: Option<TelemetryWorkerHandle>,
    program_path: Option<std::path::PathBuf>,
    overseer_enable_anyways: bool,
    overseer_gen: OverseerGenerator,
    overseer_message_channel_capacity_override: Option<usize>,
    _malus_finality_delay: Option<u32>,
    hwbench: Option<sc_sysinfo::HwBench>,
    sealing: Option<Sealing>,
) -> Result<NewFull, Error>
where
    OverseerGenerator: OverseerGen,
{
    use polkadot_node_network_protocol::request_response::IncomingRequest;
    use sc_network_common::sync::warp::WarpSyncParams;

    let is_offchain_indexing_enabled = config.offchain_worker.indexing_enabled;
    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks = Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());

    // Warn the user that BEEFY is still experimental.
    if enable_beefy {
        gum::warn!("BEEFY is still experimental, usage on a production network is discouraged.");
    }

    let disable_grandpa = config.disable_grandpa;
    let name = config.network.node_name.clone();

    let basics = new_partial_basics(&mut config, jaeger_agent, telemetry_worker_handle)?;

    let prometheus_registry = config.prometheus_registry().cloned();

    let overseer_connector = OverseerConnector::default();
    let overseer_handle = Handle::new(overseer_connector.handle());

    let keystore = basics.keystore_container.local_keystore();
    let auth_or_collator = role.is_authority() || is_collator.is_collator();
    let pvf_checker_enabled = role.is_authority() && !is_collator.is_collator();

    let select_chain = if auth_or_collator {
        let metrics =
            polkadot_node_subsystem_util::metrics::Metrics::register(prometheus_registry.as_ref())?;

        SelectRelayChain::new_with_overseer(
            basics.backend.clone(),
            overseer_handle.clone(),
            metrics,
            Some(basics.task_manager.spawn_handle()),
        )
    } else {
        SelectRelayChain::new_longest_chain(basics.backend.clone())
    };

    let service::PartialComponents::<_, _, SelectRelayChain<_>, _, _, _> {
        client,
        backend,
        mut task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other: (rpc_deps_builder, import_setup, rpc_setup, slot_duration, mut telemetry, frontier_backend, overrides),
    } = new_partial::<SelectRelayChain<_>>(&mut config, &eth_config, basics, select_chain)?;

    let FrontierPartialComponents { filter_pool, fee_history_cache, fee_history_cache_limit } = new_frontier_partial(&eth_config)?;

    let shared_voter_state = rpc_setup;
    let auth_disc_publish_non_global_ips = config.network.allow_non_globals_in_dht;
    let mut net_config = sc_network::config::FullNetworkConfiguration::new(&config.network);

    let genesis_hash = client.block_hash(0).ok().flatten().expect("Genesis block exists; qed");

    // Note: GrandPa is pushed before the Polkadot-specific protocols. This doesn't change
    // anything in terms of behaviour, but makes the logs more consistent with the other
    // Substrate nodes.
    let grandpa_protocol_name = grandpa::protocol_standard_name(&genesis_hash, &config.chain_spec);
    if sealing.is_none() {
        net_config.add_notification_protocol(grandpa::grandpa_peers_set_config(
            grandpa_protocol_name.clone(),
        ));
    }

    let beefy_gossip_proto_name =
        beefy::gossip_protocol_name(&genesis_hash, config.chain_spec.fork_id());
    // `beefy_on_demand_justifications_handler` is given to `beefy-gadget` task to be run,
    // while `beefy_req_resp_cfg` is added to `config.network.request_response_protocols`.
    let (beefy_on_demand_justifications_handler, beefy_req_resp_cfg) =
        beefy::communication::request_response::BeefyJustifsRequestHandler::new(
            &genesis_hash,
            config.chain_spec.fork_id(),
            client.clone(),
            prometheus_registry.clone(),
        );
    if enable_beefy {
        net_config.add_notification_protocol(beefy::communication::beefy_peers_set_config(
            beefy_gossip_proto_name.clone(),
        ));
        net_config.add_request_response_protocol(beefy_req_resp_cfg);
    }

    let peerset_protocol_names =
        PeerSetProtocolNames::new(genesis_hash, config.chain_spec.fork_id());

    {
        use polkadot_network_bridge::{peer_sets_info, IsAuthority};
        let is_authority = if role.is_authority() { IsAuthority::Yes } else { IsAuthority::No };
        for config in peer_sets_info(is_authority, &peerset_protocol_names) {
            net_config.add_notification_protocol(config);
        }
    }

    let req_protocol_names = ReqProtocolNames::new(&genesis_hash, config.chain_spec.fork_id());

    let (pov_req_receiver, cfg) = IncomingRequest::get_config_receiver(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (chunk_req_receiver, cfg) = IncomingRequest::get_config_receiver(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (collation_req_receiver, cfg) = IncomingRequest::get_config_receiver(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (available_data_req_receiver, cfg) =
        IncomingRequest::get_config_receiver(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (statement_req_receiver, cfg) = IncomingRequest::get_config_receiver(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (dispute_req_receiver, cfg) = IncomingRequest::get_config_receiver(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);

    let warp_sync_params = if sealing.is_none() {
        let warp_sync: Arc<dyn sc_network::config::WarpSyncProvider<Block>> =
            Arc::new(grandpa::warp_proof::NetworkProvider::new(
                backend.clone(),
                import_setup.1.shared_authority_set().clone(),
                Vec::new(),
            ));
        Some(WarpSyncParams::WithProvider(warp_sync))
    } else {
        None
    };

    let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
        service::build_network(service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_params,
        })?;

    if config.offchain_worker.enabled {
        use futures::FutureExt;

        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-work",
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: network.clone(),
                is_validator: role.is_authority(),
                enable_http_requests: false,
                custom_extensions: move |_| vec![],
            })
            .run(client.clone(), task_manager.spawn_handle())
            .boxed(),
        );
    }

    let parachains_db = open_database(&config.database)?;

    let approval_voting_config = ApprovalVotingConfig {
        col_approval_data: parachains_db::REAL_COLUMNS.col_approval_data,
        slot_duration_millis: slot_duration.as_millis() as u64,
    };

    let candidate_validation_config = CandidateValidationConfig {
        artifacts_cache_path: config
            .database
            .path()
            .ok_or(Error::DatabasePathRequired)?
            .join("pvf-artifacts"),
        program_path: match program_path {
            None => std::env::current_exe()?,
            Some(p) => p,
        },
    };

    let chain_selection_config = ChainSelectionConfig {
        col_data: parachains_db::REAL_COLUMNS.col_chain_selection_data,
        stagnant_check_interval: Default::default(),
        stagnant_check_mode: chain_selection_subsystem::StagnantCheckMode::PruneOnly,
    };

    let dispute_coordinator_config = DisputeCoordinatorConfig {
        col_dispute_data: parachains_db::REAL_COLUMNS.col_dispute_coordinator_data,
    };

    // Channel for the rpc handler to communicate with the authorship task.
    let (command_sink, commands_stream) = futures::channel::mpsc::channel(1000);

    // Sinks for pubsub notifications.
    // Everytime a new subscription is created, a new mpsc channel is added to the sink pool.
    // The MappingSyncWorker sends through the channel on block import and the subscription emits a notification to the subscriber on receiving a message through this channel.
    // This way we avoid race conditions when using native substrate block import notification stream.
    let pubsub_notification_sinks: fc_mapping_sync::EthereumBlockNotificationSinks<
        fc_mapping_sync::EthereumBlockNotification<Block>,
    > = Default::default();
    let pubsub_notification_sinks = Arc::new(pubsub_notification_sinks);

    // for ethereum-compatibility rpc.
    config.rpc_id_provider = Some(Box::new(fc_rpc::EthereumSubIdProvider));
    let eth_rpc_params = polkadot_rpc::EthDeps {
        client: client.clone(),
        pool: transaction_pool.clone(),
        graph: transaction_pool.pool().clone(),
        converter: Some(TransactionConverter),
        is_authority: config.role.is_authority(),
        enable_dev_signer: eth_config.enable_dev_signer,
        network: network.clone(),
        sync: sync_service.clone(),
        frontier_backend: match frontier_backend.clone() {
            fc_db::Backend::KeyValue(b) => Arc::new(b),
            fc_db::Backend::Sql(b) => Arc::new(b),
        },
        overrides: overrides.clone(),
        block_data_cache: Arc::new(fc_rpc::EthBlockDataCacheTask::new(
            task_manager.spawn_handle(),
            overrides.clone(),
            eth_config.eth_log_block_cache,
            eth_config.eth_statuses_cache,
            prometheus_registry.clone(),
        )),
        filter_pool: filter_pool.clone(),
        max_past_logs: eth_config.max_past_logs,
        fee_history_cache: fee_history_cache.clone(),
        fee_history_cache_limit,
        execute_gas_limit_multiplier: eth_config.execute_gas_limit_multiplier,
        forced_parent_hashes: None,
    };

    let rpc_builder = {
        let client = client.clone();
        let transaction_pool = transaction_pool.clone();
        let select_chain = select_chain.clone();
        let chain_spec = config.chain_spec.cloned_box();
        let pubsub_notification_sinks = pubsub_notification_sinks.clone();
        let backend = backend.clone();
        let node_name = name.clone();

        move |deny_unsafe, subscription_executor: polkadot_rpc::SubscriptionTaskExecutor| {
            let (babe, grandpa, beefy) = rpc_deps_builder(subscription_executor.clone());

            let deps = polkadot_rpc::FullDeps {
                client: client.clone(),
                pool: transaction_pool.clone(),
                select_chain: select_chain.clone(),
                chain_spec: chain_spec.cloned_box(),
                deny_unsafe,
                command_sink: if sealing.is_some() { Some(command_sink.clone()) } else { None },
                eth: eth_rpc_params.clone(),
                babe,
                grandpa,
                beefy,
                backend: backend.clone(),
                node: polkadot_rpc::NodeDeps { name: node_name.clone() },
            };

            polkadot_rpc::create_full(deps, subscription_executor, pubsub_notification_sinks.clone()).map_err(Into::into)
        }
    };

    let rpc_handlers = service::spawn_tasks(service::SpawnTasksParams {
        config,
        backend: backend.clone(),
        client: client.clone(),
        keystore: keystore_container.keystore(),
        network: network.clone(),
        sync_service: sync_service.clone(),
        rpc_builder: Box::new(rpc_builder),
        transaction_pool: transaction_pool.clone(),
        task_manager: &mut task_manager,
        system_rpc_tx,
        tx_handler_controller,
        telemetry: telemetry.as_mut(),
    })?;

    spawn_frontier_tasks(
        &task_manager,
        client.clone(),
        backend.clone(),
        frontier_backend,
        filter_pool,
        overrides,
        fee_history_cache,
        fee_history_cache_limit,
        sync_service.clone(),
        pubsub_notification_sinks,
    );

    if let Some(hwbench) = hwbench {
        sc_sysinfo::print_hwbench(&hwbench);
        if !SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench) && role.is_authority() {
            log::warn!(
				"⚠️  The hardware does not meet the minimal requirements for role 'Authority' find out more at:\n\
				https://wiki.polkadot.network/docs/maintain-guides-how-to-validate-polkadot#reference-hardware"
			);
        }

        if let Some(ref mut telemetry) = telemetry {
            let telemetry_handle = telemetry.handle();
            task_manager.spawn_handle().spawn(
                "telemetry_hwbench",
                None,
                sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
            );
        }
    }

    let (block_import, link_half, babe_link, beefy_links) = import_setup;

    let overseer_client = client.clone();
    let spawner = task_manager.spawn_handle();

    let authority_discovery_service = if auth_or_collator || overseer_enable_anyways {
        use futures::StreamExt;
        use sc_network::{Event, NetworkEventStream};

        let authority_discovery_role = if role.is_authority() {
            sc_authority_discovery::Role::PublishAndDiscover(keystore_container.keystore())
        } else {
            // don't publish our addresses when we're not an authority (collator, cumulus, ..)
            sc_authority_discovery::Role::Discover
        };
        let dht_event_stream =
            network.event_stream("authority-discovery").filter_map(|e| async move {
                match e {
                    Event::Dht(e) => Some(e),
                    _ => None,
                }
            });
        let (worker, service) = sc_authority_discovery::new_worker_and_service_with_config(
            sc_authority_discovery::WorkerConfig {
                publish_non_global_ips: auth_disc_publish_non_global_ips,
                // Require that authority discovery records are signed.
                strict_record_validation: true,
                ..Default::default()
            },
            client.clone(),
            network.clone(),
            Box::pin(dht_event_stream),
            authority_discovery_role,
            prometheus_registry.clone(),
        );

        task_manager.spawn_handle().spawn(
            "authority-discovery-worker",
            Some("authority-discovery"),
            Box::pin(worker.run()),
        );
        Some(service)
    } else {
        None
    };

    let overseer_handle = if let Some(authority_discovery_service) = authority_discovery_service {
        let (overseer, overseer_handle) = overseer_gen
            .generate::<service::SpawnTaskHandle, FullClient>(
                overseer_connector,
                OverseerGenArgs {
                    keystore,
                    runtime_client: overseer_client.clone(),
                    parachains_db,
                    network_service: network.clone(),
                    sync_service: sync_service.clone(),
                    authority_discovery_service,
                    pov_req_receiver,
                    chunk_req_receiver,
                    collation_req_receiver,
                    available_data_req_receiver,
                    statement_req_receiver,
                    dispute_req_receiver,
                    registry: prometheus_registry.as_ref(),
                    spawner,
                    is_collator,
                    approval_voting_config,
                    availability_config: AVAILABILITY_CONFIG,
                    candidate_validation_config,
                    chain_selection_config,
                    dispute_coordinator_config,
                    pvf_checker_enabled,
                    overseer_message_channel_capacity_override,
                    req_protocol_names,
                    peerset_protocol_names,
                    offchain_transaction_pool_factory: OffchainTransactionPoolFactory::new(
                        transaction_pool.clone(),
                    ),
                },
            )
            .map_err(|e| {
                gum::error!("Failed to init overseer: {}", e);
                e
            })?;
        let handle = Handle::new(overseer_handle.clone());

        {
            let handle = handle.clone();
            task_manager.spawn_essential_handle().spawn_blocking(
                "overseer",
                None,
                Box::pin(async move {
                    use futures::{pin_mut, select, FutureExt};

                    let forward = polkadot_overseer::forward_events(overseer_client, handle);

                    let forward = forward.fuse();
                    let overseer_fut = overseer.run().fuse();

                    pin_mut!(overseer_fut);
                    pin_mut!(forward);

                    select! {
                        () = forward => (),
                        () = overseer_fut => (),
                        complete => (),
                    }
                }),
            );
        }
        Some(handle)
    } else {
        assert!(
            !auth_or_collator,
            "Precondition congruence (false) is guaranteed by manual checking. qed"
        );
        None
    };

    if role.is_authority() {
        // manual-seal authorship
        if let Some(sealing) = sealing {
            run_manual_seal_authorship(
                &eth_config,
                sealing,
                client.clone(),
                transaction_pool,
                select_chain,
                Box::new(block_import),
                &task_manager,
                prometheus_registry.as_ref(),
                telemetry.as_ref(),
                commands_stream,
            )?;

            network_starter.start_network();
            log::info!("Manual Seal Ready");
            return Ok(NewFull {
                task_manager,
                client,
                overseer_handle,
                network,
                sync_service,
                rpc_handlers,
                backend,
            });
        }

        let proposer = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        let client_clone = client.clone();
        let overseer_handle =
            overseer_handle.as_ref().ok_or(Error::AuthoritiesRequireRealOverseer)?.clone();
        let slot_duration = babe_link.config().slot_duration();
        let babe_config = babe::BabeParams {
            keystore: keystore_container.keystore(),
            client: client.clone(),
            select_chain,
            block_import,
            env: proposer,
            sync_oracle: sync_service.clone(),
            justification_sync_link: sync_service.clone(),
            create_inherent_data_providers: move |parent, ()| {
                let client_clone = client_clone.clone();
                let overseer_handle = overseer_handle.clone();

                async move {
                    let parachain =
                        polkadot_node_core_parachains_inherent::ParachainsInherentDataProvider::new(
                            client_clone.clone(),
                            overseer_handle,
                            parent,
                        );

                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let slot =
                        sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                            *timestamp,
                            slot_duration,
                        );

                    let storage_proof =
                        sp_transaction_storage_proof::registration::new_data_provider(
                            &*client_clone,
                            &parent,
                        )?;

                    Ok((slot, timestamp, parachain, storage_proof))
                }
            },
            force_authoring,
            backoff_authoring_blocks,
            babe_link,
            block_proposal_slot_portion: babe::SlotProportion::new(2f32 / 3f32),
            max_block_proposal_slot_portion: None,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        };

        let babe = babe::start_babe(babe_config)?;
        task_manager.spawn_essential_handle().spawn_blocking("babe", None, babe);
    }

    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore_opt = if role.is_authority() { Some(keystore_container.keystore()) } else { None };

    if enable_beefy {
        let justifications_protocol_name = beefy_on_demand_justifications_handler.protocol_name();
        let network_params = beefy::BeefyNetworkParams {
            network: network.clone(),
            sync: sync_service.clone(),
            gossip_protocol_name: beefy_gossip_proto_name,
            justifications_protocol_name,
            _phantom: core::marker::PhantomData::<Block>,
        };
        let payload_provider = beefy_primitives::mmr::MmrRootProvider::new(client.clone());
        let beefy_params = beefy::BeefyParams {
            client: client.clone(),
            backend: backend.clone(),
            payload_provider,
            runtime: client.clone(),
            key_store: keystore_opt.clone(),
            network_params,
            min_block_delta: 8,
            prometheus_registry: prometheus_registry.clone(),
            links: beefy_links,
            on_demand_justifications_handler: beefy_on_demand_justifications_handler,
        };

        let gadget = beefy::start_beefy_gadget::<_, _, _, _, _, _, _>(beefy_params);

        // BEEFY currently only runs on testnets, if it fails we'll
        // bring the node down with it to make sure it is noticed.
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("beefy-gadget", None, gadget);

        if is_offchain_indexing_enabled {
            task_manager.spawn_handle().spawn_blocking(
                "mmr-gadget",
                None,
                MmrGadget::start(
                    client.clone(),
                    backend.clone(),
                    sp_mmr_primitives::INDEXING_PREFIX.to_vec(),
                ),
            );
        }
    }

    let config = grandpa::Config {
        // FIXME substrate#1578 make this available through chainspec
        // Grandpa performance can be improved a bit by tuning this parameter, see:
        // https://github.com/paritytech/polkadot/issues/5464
        gossip_duration: Duration::from_millis(1000),
        justification_period: 512,
        name: Some(name),
        observer_enabled: false,
        keystore: keystore_opt,
        local_role: role,
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        protocol_name: grandpa_protocol_name,
    };

    let enable_grandpa = !disable_grandpa && sealing.is_none();
    if enable_grandpa {
        // start the full GRANDPA voter
        // NOTE: unlike in substrate we are currently running the full
        // GRANDPA voter protocol for all full nodes (regardless of whether
        // they're validators or not). at this point the full voter should
        // provide better guarantees of block and vote data availability than
        // the observer.

        // add a custom voting rule to temporarily stop voting for new blocks
        // after the given pause block is finalized and restarting after the
        // given delay.
        let mut builder = grandpa::VotingRulesBuilder::default();

        #[cfg(not(feature = "malus"))]
        let _malus_finality_delay = None;

        if let Some(delay) = _malus_finality_delay {
            info!(?delay, "Enabling malus finality delay",);
            builder = builder.add(grandpa::BeforeBestBlockBy(delay));
        };

        let voting_rule = match grandpa_pause {
            Some((block, delay)) => {
                info!(
                    block_number = %block,
                    delay = %delay,
                    "GRANDPA scheduled voting pause set for block #{} with a duration of {} blocks.",
                    block,
                    delay,
                );

                builder.add(grandpa_support::PauseAfterBlockFor(block, delay)).build()
            },
            None => builder.build(),
        };

        let grandpa_config = grandpa::GrandpaParams {
            config,
            link: link_half,
            network: network.clone(),
            sync: sync_service.clone(),
            voting_rule,
            prometheus_registry: prometheus_registry.clone(),
            shared_voter_state,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
        };

        task_manager.spawn_essential_handle().spawn_blocking(
            "grandpa-voter",
            None,
            grandpa::run_grandpa_voter(grandpa_config)?,
        );
    }

    network_starter.start_network();

    Ok(NewFull {
        task_manager,
        client,
        overseer_handle,
        network,
        sync_service,
        rpc_handlers,
        backend,
    })
}

/// Builds a new object suitable for chain operations.
#[cfg(feature = "full-node")]
pub fn new_chain_ops(
    config: &mut Configuration,
    eth_config: &EthConfiguration,
    jaeger_agent: Option<std::net::SocketAddr>,
) -> Result<
    (
        Arc<FullClient>,
        Arc<FullBackend>,
        sc_consensus::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
        TaskManager,
        FrontierBackend,
    ),
    Error,
> {
    config.keystore = service::config::KeystoreConfig::InMemory;

    let basics = new_partial_basics(config, jaeger_agent, None)?;

    use ::sc_consensus::LongestChain;
    // use the longest chain selection, since there is no overseer available
    let chain_selection = LongestChain::new(basics.backend.clone());

    let service::PartialComponents { client, backend, import_queue, task_manager, other, .. } =
        new_partial::<LongestChain<_, Block>>(config, eth_config, basics, chain_selection)?;
    Ok((client, backend, import_queue, task_manager, other.5))
}

/// Build a full node.
///
/// The actual "flavor", aka if it will use `Polkadot`, `Rococo` or `Kusama` is determined based on
/// [`IdentifyVariant`] using the chain spec.
///
/// `overseer_enable_anyways` always enables the overseer, based on the provided `OverseerGenerator`,
/// regardless of the role the node has. The relay chain selection (longest or disputes-aware) is
/// still determined based on the role of the node. Likewise for authority discovery.
#[cfg(feature = "full-node")]
pub fn build_full(
    config: Configuration,
    eth_config: EthConfiguration,
    is_collator: IsCollator,
    grandpa_pause: Option<(u32, u32)>,
    enable_beefy: bool,
    jaeger_agent: Option<std::net::SocketAddr>,
    telemetry_worker_handle: Option<TelemetryWorkerHandle>,
    overseer_enable_anyways: bool,
    overseer_gen: impl OverseerGen,
    overseer_message_channel_override: Option<usize>,
    malus_finality_delay: Option<u32>,
    hwbench: Option<sc_sysinfo::HwBench>,
    sealing: Option<Sealing>,
) -> Result<NewFull, Error> {
    new_full(
        config,
        eth_config,
        is_collator,
        grandpa_pause,
        enable_beefy,
        jaeger_agent,
        telemetry_worker_handle,
        None,
        overseer_enable_anyways,
        overseer_gen,
        overseer_message_channel_override,
        malus_finality_delay,
        hwbench,
        sealing
    )
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

    revert_approval_voting(parachains_db.clone(), hash)?;
    revert_chain_selection(parachains_db, hash)?;
    // Revert Substrate consensus related components
    babe::revert(client.clone(), backend, blocks)?;
    grandpa::revert(client, blocks)?;

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

fn revert_approval_voting(db: Arc<dyn Database>, hash: Hash) -> sp_blockchain::Result<()> {
    let config = approval_voting_subsystem::Config {
        col_approval_data: parachains_db::REAL_COLUMNS.col_approval_data,
        slot_duration_millis: Default::default(),
    };

    let approval_voting = approval_voting_subsystem::ApprovalVotingSubsystem::with_config(
        config,
        db,
        Arc::new(sc_keystore::LocalKeystore::in_memory()),
        Box::new(consensus_common::NoNetwork),
        approval_voting_subsystem::Metrics::default(),
    );

    approval_voting
        .revert_to(hash)
        .map_err(|err| sp_blockchain::Error::Backend(err.to_string()))
}

fn run_manual_seal_authorship(
    eth_config: &EthConfiguration,
    sealing: Sealing,
    client: Arc<FullClient>,
    transaction_pool: Arc<sc_transaction_pool::FullPool<Block, FullClient>>,
    select_chain: FullSelectChain,
    block_import: BoxBlockImport<FullClient>,
    task_manager: &TaskManager,
    prometheus_registry: Option<&Registry>,
    telemetry: Option<&Telemetry>,
    commands_stream: futures::channel::mpsc::Receiver<sc_consensus_manual_seal::rpc::EngineCommand<Hash>>,
) -> Result<(), service::Error> {
    let proposer_factory = sc_basic_authorship::ProposerFactory::new(
        task_manager.spawn_handle(),
        client.clone(),
        transaction_pool.clone(),
        prometheus_registry,
        telemetry.as_ref().map(|x| x.handle()),
    );

    thread_local!(static TIMESTAMP: RefCell<u64> = RefCell::new(0));

    /// Provide a mock duration starting at 0 in millisecond for timestamp inherent.
    /// Each call will increment timestamp by slot_duration making Aura think time has passed.
    struct MockTimestampInherentDataProvider;

    #[async_trait::async_trait]
    impl sp_inherents::InherentDataProvider for MockTimestampInherentDataProvider {
        async fn provide_inherent_data(
            &self,
            inherent_data: &mut sp_inherents::InherentData,
        ) -> Result<(), sp_inherents::Error> {
            TIMESTAMP.with(|x| {
                *x.borrow_mut() += vitreus_power_plant_runtime::SLOT_DURATION;
                inherent_data.put_data(sp_timestamp::INHERENT_IDENTIFIER, &*x.borrow())
            })
        }

        async fn try_handle_error(
            &self,
            _identifier: &sp_inherents::InherentIdentifier,
            _error: &[u8],
        ) -> Option<Result<(), sp_inherents::Error>> {
            // The pallet never reports error.
            None
        }
    }

    let target_gas_price = eth_config.target_gas_price;
    let create_inherent_data_providers = move |_, ()| async move {
        let timestamp = MockTimestampInherentDataProvider;
        let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
        Ok((timestamp, dynamic_fee))
    };

    let manual_seal = match sealing {
        Sealing::Manual => futures::future::Either::Left(sc_consensus_manual_seal::run_manual_seal(
            sc_consensus_manual_seal::ManualSealParams {
                block_import,
                env: proposer_factory,
                client,
                pool: transaction_pool,
                commands_stream,
                select_chain,
                consensus_data_provider: None,
                create_inherent_data_providers,
            },
        )),
        Sealing::Instant => futures::future::Either::Right(sc_consensus_manual_seal::run_instant_seal(
            sc_consensus_manual_seal::InstantSealParams {
                block_import,
                env: proposer_factory,
                client,
                pool: transaction_pool,
                select_chain,
                consensus_data_provider: None,
                create_inherent_data_providers,
            },
        )),
    };

    // we spawn the future on a background thread managed by service.
    task_manager
        .spawn_essential_handle()
        .spawn_blocking("manual-seal", None, manual_seal);
    Ok(())
}
