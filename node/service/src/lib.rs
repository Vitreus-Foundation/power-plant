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
mod parachains_db;

#[cfg(feature = "full-node")]
use {
    gum::info,
    polkadot_node_core_approval_voting::{
        self as approval_voting_subsystem, Config as ApprovalVotingConfig,
    },
    polkadot_node_core_av_store::Config as AvailabilityConfig,
    polkadot_node_core_candidate_validation::Config as CandidateValidationConfig,
    polkadot_node_core_chain_selection::{
        self as chain_selection_subsystem, Config as ChainSelectionConfig,
    },
    polkadot_node_core_dispute_coordinator::Config as DisputeCoordinatorConfig,
    polkadot_node_network_protocol::{
        peer_set::PeerSetProtocolNames, request_response::ReqProtocolNames,
    },
    polkadot_service::ExtendedOverseerGenArgs,
    sc_client_api::BlockBackend,
    sc_consensus_grandpa::{self, FinalityProofProvider as GrandpaFinalityProofProvider},
    sc_transaction_pool_api::OffchainTransactionPoolFactory,
    sp_core::traits::SpawnNamed,
    sp_core::U256,
};

use polkadot_node_subsystem_util::database::Database;

#[cfg(feature = "full-node")]
pub use {
    polkadot_node_network_protocol::peer_set::PeerSet,
    polkadot_node_subsystem_types::DefaultSubsystemClient,
    polkadot_overseer::{Handle, Overseer, OverseerConnector, OverseerHandle},
    polkadot_primitives::runtime_api::ParachainHost,
    polkadot_service::{
        Error, HeaderProvider, HeaderProviderProvider, IsParachainNode, NewFullParams, OverseerGen,
        OverseerGenArgs, SelectRelayChain, ValidatorOverseerGen,
    },
    sc_client_api::AuxStore,
    sc_network_sync::{service::network::Network, strategy::warp::WarpSyncProvider},
    sp_authority_discovery::AuthorityDiscoveryApi,
    sp_blockchain::{HeaderBackend, HeaderMetadata},
    sp_consensus_babe::BabeApi,
};

#[cfg(feature = "full-node")]
use polkadot_node_subsystem::jaeger;

#[cfg(feature = "full-node")]
use sc_service::KeystoreContainer;
use sc_service::RpcHandlers;
use sc_telemetry::TelemetryWorker;
#[cfg(feature = "full-node")]
use sc_telemetry::{Telemetry, TelemetryWorkerHandle};
use std::{collections::HashMap, path::Path};
use std::{sync::Arc, time::Duration};

pub use sp_consensus::{Proposal, SelectChain};
use sp_consensus_beefy::ecdsa_crypto;

use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
use mmr_gadget::MmrGadget;

pub use polkadot_primitives::{Block, BlockId, BlockNumber, CollatorPair, Hash, Id as ParaId};
use polkadot_service::workers;
pub use sc_client_api::{Backend, CallExecutor};
pub use sc_consensus::{BlockImport, BoxBlockImport, LongestChain};
pub use sc_executor::NativeExecutionDispatch;
use sc_executor::{HeapAllocStrategy, WasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY};

pub use sc_service::{
    config::{DatabaseSource, PrometheusConfig},
    ChainSpec, Configuration, Error as SubstrateServiceError, PruningMode, Role, TFullBackend,
    TFullCallExecutor, TFullClient, TaskManager, TransactionPoolOptions,
};
pub use sp_api::{ApiRef, ConstructRuntimeApi, Core as CoreApi, ProvideRuntimeApi};
pub use sp_runtime::{
    generic,
    traits::{self as runtime_traits, BlakeTwo256, Block as BlockT, Header as HeaderT, NumberFor},
};

use eth::{
    db_config_dir, new_frontier_partial, spawn_frontier_tasks, BackendType, EthConfiguration,
    FrontierBackend, FrontierPartialComponents, StorageOverride, StorageOverrideHandler,
};

use vitreus_power_plant_runtime::{RuntimeApi, TransactionConverter};

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
type FullSelectChain = SelectRelayChain<FullBackend>;
#[cfg(feature = "full-node")]
type FullGrandpaBlockImport<ChainSelection = FullSelectChain> =
    sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, ChainSelection>;
#[cfg(feature = "full-node")]
type FullBeefyBlockImport<InnerBlockImport, AuthorityId> =
    sc_consensus_beefy::import::BeefyBlockImport<
        Block,
        FullBackend,
        FullClient,
        InnerBlockImport,
        AuthorityId,
    >;

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
        .map(move |endpoints| -> Result<_, sc_telemetry::Error> {
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
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
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
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        ChainSelection,
        sc_consensus::DefaultImportQueue<Block>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            impl Fn(
                polkadot_rpc::DenyUnsafe,
                polkadot_rpc::SubscriptionTaskExecutor,
            ) -> Result<polkadot_rpc::RpcExtension, SubstrateServiceError>,
            (
                sc_consensus_babe::BabeBlockImport<
                    Block,
                    FullClient,
                    FullBeefyBlockImport<
                        FullGrandpaBlockImport<ChainSelection>,
                        ecdsa_crypto::AuthorityId,
                    >,
                >,
                sc_consensus_grandpa::LinkHalf<Block, FullClient, ChainSelection>,
                sc_consensus_babe::BabeLink<Block>,
                sc_consensus_beefy::BeefyVoterLinks<Block, ecdsa_crypto::AuthorityId>,
            ),
            sc_consensus_grandpa::SharedVoterState,
            sp_consensus_babe::SlotDuration,
            Option<Telemetry>,
            FrontierBackend<Block, FullClient>,
            Arc<dyn StorageOverride<Block>>,
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

    let grandpa_hard_forks = Vec::new();

    let (grandpa_block_import, grandpa_link) =
        sc_consensus_grandpa::block_import_with_authority_set_hard_forks(
            client.clone(),
            GRANDPA_JUSTIFICATION_PERIOD,
            &(client.clone() as Arc<_>),
            select_chain.clone(),
            grandpa_hard_forks,
            telemetry.as_ref().map(|x| x.handle()),
        )?;
    let justification_import = grandpa_block_import.clone();

    let storage_override = Arc::new(StorageOverrideHandler::new(client.clone()));
    let frontier_backend = match eth_config.frontier_backend_type {
        BackendType::KeyValue => FrontierBackend::KeyValue(Arc::new(
            fc_db::kv::Backend::open(Arc::clone(&client), &config.database, &db_config_dir(config))
                .map_err(SubstrateServiceError::Other)?,
        )),
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
                storage_override.clone(),
            ))
            .unwrap_or_else(|err| panic!("failed creating sql backend: {:?}", err));
            FrontierBackend::Sql(Arc::new(backend))
        },
    };

    let (beefy_block_import, beefy_voter_links, beefy_rpc_links) =
        sc_consensus_beefy::beefy_block_import_and_links(
            grandpa_block_import,
            backend.clone(),
            client.clone(),
            config.prometheus_registry().cloned(),
        );

    let babe_config = sc_consensus_babe::configuration(&*client)?;
    let (block_import, babe_link) =
        sc_consensus_babe::block_import(babe_config.clone(), beefy_block_import, client.clone())?;

    let slot_duration = babe_link.config().slot_duration();
    let target_gas_price = eth_config.target_gas_price;
    let (import_queue, babe_worker_handle) =
        sc_consensus_babe::import_queue(sc_consensus_babe::ImportQueueParams {
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
                let dynamic_fee =
                    fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));

                Ok((slot, timestamp, dynamic_fee))
            },
            spawner: &task_manager.spawn_essential_handle(),
            registry: config.prometheus_registry(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
        })?;

    let justification_stream = grandpa_link.justification_stream();
    let shared_authority_set = grandpa_link.shared_authority_set().clone();
    let shared_voter_state = sc_consensus_grandpa::SharedVoterState::empty();
    let finality_proof_provider = GrandpaFinalityProofProvider::new_for_service(
        backend.clone(),
        Some(shared_authority_set.clone()),
    );

    let import_setup = (block_import, grandpa_link, babe_link, beefy_voter_links);
    let rpc_setup = shared_voter_state.clone();

    let rpc_extensions_builder = {
        let client = client.clone();
        let keystore = keystore_container.keystore();
        let transaction_pool = transaction_pool.clone();
        let select_chain = select_chain.clone();
        let chain_spec = config.chain_spec.cloned_box();
        let backend = backend.clone();

        move |deny_unsafe,
              subscription_executor: polkadot_rpc::SubscriptionTaskExecutor|
              -> Result<polkadot_rpc::RpcExtension, sc_service::Error> {
            let deps = polkadot_rpc::FullDeps {
                client: client.clone(),
                pool: transaction_pool.clone(),
                select_chain: select_chain.clone(),
                chain_spec: chain_spec.cloned_box(),
                deny_unsafe,
                babe: polkadot_rpc::BabeDeps {
                    babe_worker_handle: babe_worker_handle.clone(),
                    keystore: keystore.clone(),
                },
                grandpa: polkadot_rpc::GrandpaDeps {
                    shared_voter_state: shared_voter_state.clone(),
                    shared_authority_set: shared_authority_set.clone(),
                    justification_stream: justification_stream.clone(),
                    subscription_executor: subscription_executor.clone(),
                    finality_provider: finality_proof_provider.clone(),
                },
                beefy: polkadot_rpc::BeefyDeps::<ecdsa_crypto::AuthorityId> {
                    beefy_finality_proof_stream: beefy_rpc_links.from_voter_justif_stream.clone(),
                    beefy_best_block_stream: beefy_rpc_links.from_voter_best_beefy_stream.clone(),
                    subscription_executor,
                },
                backend: backend.clone(),
            };

            vitreus_rpc::create_basic(deps).map_err(Into::into)
        }
    };

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other: (
            rpc_extensions_builder,
            import_setup,
            rpc_setup,
            slot_duration,
            telemetry,
            frontier_backend,
            storage_override,
        ),
    })
}

#[cfg(feature = "full-node")]
pub struct NewFull {
    pub task_manager: TaskManager,
    pub client: Arc<FullClient>,
    pub overseer_handle: Option<Handle>,
    pub network: Arc<dyn sc_network::service::traits::NetworkService>,
    pub sync_service: Arc<sc_network_sync::SyncingService<Block>>,
    pub rpc_handlers: RpcHandlers,
    pub backend: Arc<FullBackend>,
}

trait IsParachainNodeHelper {
    fn is_collator(&self) -> bool;
    fn is_full_node(&self) -> bool;
    fn is_running_alongside_parachain_node(&self) -> bool;
}

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

pub const AVAILABILITY_CONFIG: AvailabilityConfig = AvailabilityConfig {
    col_data: parachains_db::REAL_COLUMNS.col_availability_data,
    col_meta: parachains_db::REAL_COLUMNS.col_availability_meta,
};

/// Create a new full node of arbitrary runtime and executor.
///
/// This is an advanced feature and not recommended for general use. Generally, `build_full` is
/// a better choice.
///
/// `workers_path` is used to get the path to the directory where auxiliary worker binaries reside.
/// If not specified, the main binary's directory is searched first, then `/usr/lib/polkadot` is
/// searched. If the path points to an executable rather then directory, that executable is used
/// both as preparation and execution worker (supposed to be used for tests only).
#[cfg(feature = "full-node")]
pub fn new_full<
    OverseerGenerator: OverseerGen,
    Network: sc_network::NetworkBackend<Block, <Block as BlockT>::Hash>,
>(
    mut config: Configuration,
    eth_config: EthConfiguration,
    NewFullParams {
        is_parachain_node,
        enable_beefy,
        force_authoring_backoff,
        jaeger_agent,
        telemetry_worker_handle,
        node_version,
        secure_validator_mode,
        workers_path,
        workers_names,
        overseer_gen,
        overseer_message_channel_capacity_override,
        malus_finality_delay: _malus_finality_delay,
        hwbench,
        execute_workers_max_num,
        prepare_workers_soft_max_num,
        prepare_workers_hard_max_num,
    }: NewFullParams<OverseerGenerator>,
) -> Result<NewFull, Error> {
    use polkadot_availability_recovery::FETCH_CHUNKS_THRESHOLD;
    use polkadot_node_network_protocol::request_response::IncomingRequest;
    use sc_network_sync::WarpSyncParams;

    let is_offchain_indexing_enabled = config.offchain_worker.indexing_enabled;
    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks = if !force_authoring_backoff {
        None
    } else {
        Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default())
    };

    let disable_grandpa = config.disable_grandpa;
    let name = config.network.node_name.clone();

    let basics = new_partial_basics(&mut config, jaeger_agent, telemetry_worker_handle)?;

    let prometheus_registry = config.prometheus_registry().cloned();

    let overseer_connector = OverseerConnector::default();
    let overseer_handle = Handle::new(overseer_connector.handle());

    let keystore = basics.keystore_container.local_keystore();
    let auth_or_collator = role.is_authority() || is_parachain_node.is_collator();

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

    let sc_service::PartialComponents::<_, _, SelectRelayChain<_>, _, _, _> {
        client,
        backend,
        mut task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other:
            (
                rpc_extensions_builder,
                import_setup,
                rpc_setup,
                slot_duration,
                mut telemetry,
                frontier_backend,
                storage_override,
            ),
    } = new_partial::<SelectRelayChain<_>>(&mut config, &eth_config, basics, select_chain)?;

    let FrontierPartialComponents { filter_pool, fee_history_cache, fee_history_cache_limit } =
        new_frontier_partial(&eth_config)?;

    let metrics = Network::register_notification_metrics(
        config.prometheus_config.as_ref().map(|cfg| &cfg.registry),
    );
    let shared_voter_state = rpc_setup;
    let auth_disc_publish_non_global_ips = config.network.allow_non_globals_in_dht;
    let auth_disc_public_addresses = config.network.public_addresses.clone();

    let mut net_config =
        sc_network::config::FullNetworkConfiguration::<_, _, Network>::new(&config.network);

    let genesis_hash = client.block_hash(0).ok().flatten().expect("Genesis block exists; qed");
    let peer_store_handle = net_config.peer_store_handle();

    // Note: GrandPa is pushed before the Polkadot-specific protocols. This doesn't change
    // anything in terms of behaviour, but makes the logs more consistent with the other
    // Substrate nodes.
    let grandpa_protocol_name =
        sc_consensus_grandpa::protocol_standard_name(&genesis_hash, &config.chain_spec);
    let (grandpa_protocol_config, grandpa_notification_service) =
        sc_consensus_grandpa::grandpa_peers_set_config::<_, Network>(
            grandpa_protocol_name.clone(),
            metrics.clone(),
            Arc::clone(&peer_store_handle),
        );
    net_config.add_notification_protocol(grandpa_protocol_config);

    let beefy_gossip_proto_name =
        sc_consensus_beefy::gossip_protocol_name(&genesis_hash, config.chain_spec.fork_id());
    // `beefy_on_demand_justifications_handler` is given to `beefy-gadget` task to be run,
    // while `beefy_req_resp_cfg` is added to `config.network.request_response_protocols`.
    let (beefy_on_demand_justifications_handler, beefy_req_resp_cfg) =
        sc_consensus_beefy::communication::request_response::BeefyJustifsRequestHandler::new::<
            _,
            Network,
        >(
            &genesis_hash, config.chain_spec.fork_id(), client.clone(), prometheus_registry.clone()
        );
    let beefy_notification_service = match enable_beefy {
        false => None,
        true => {
            let (beefy_notification_config, beefy_notification_service) =
                sc_consensus_beefy::communication::beefy_peers_set_config::<_, Network>(
                    beefy_gossip_proto_name.clone(),
                    metrics.clone(),
                    Arc::clone(&peer_store_handle),
                );

            net_config.add_notification_protocol(beefy_notification_config);
            net_config.add_request_response_protocol(beefy_req_resp_cfg);
            Some(beefy_notification_service)
        },
    };

    // validation/collation protocols are enabled only if `Overseer` is enabled
    let peerset_protocol_names =
        PeerSetProtocolNames::new(genesis_hash, config.chain_spec.fork_id());

    // If this is a validator or running alongside a parachain node, we need to enable the
    // networking protocols.
    //
    // Collators and parachain full nodes require the collator and validator networking to send
    // collations and to be able to recover PoVs.
    let notification_services =
        if role.is_authority() || is_parachain_node.is_running_alongside_parachain_node() {
            use polkadot_network_bridge::{peer_sets_info, IsAuthority};
            let is_authority = if role.is_authority() { IsAuthority::Yes } else { IsAuthority::No };

            peer_sets_info::<_, Network>(
                is_authority,
                &peerset_protocol_names,
                metrics.clone(),
                Arc::clone(&peer_store_handle),
            )
            .into_iter()
            .map(|(config, (peerset, service))| {
                net_config.add_notification_protocol(config);
                (peerset, service)
            })
            .collect::<HashMap<PeerSet, Box<dyn sc_network::NotificationService>>>()
        } else {
            std::collections::HashMap::new()
        };

    let req_protocol_names = ReqProtocolNames::new(&genesis_hash, config.chain_spec.fork_id());

    let (collation_req_v1_receiver, cfg) =
        IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (collation_req_v2_receiver, cfg) =
        IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (available_data_req_receiver, cfg) =
        IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (pov_req_receiver, cfg) =
        IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (chunk_req_v1_receiver, cfg) =
        IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);
    let (chunk_req_v2_receiver, cfg) =
        IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
    net_config.add_request_response_protocol(cfg);

    let grandpa_hard_forks = Vec::new();

    let warp_sync = Arc::new(sc_consensus_grandpa::warp_proof::NetworkProvider::new(
        backend.clone(),
        import_setup.1.shared_authority_set().clone(),
        grandpa_hard_forks,
    ));

    let ext_overseer_args = if is_parachain_node.is_running_alongside_parachain_node() {
        None
    } else {
        let parachains_db = open_database(&config.database)?;
        let candidate_validation_config = if role.is_authority() {
            let (prep_worker_path, exec_worker_path) = workers::determine_workers_paths(
                workers_path,
                workers_names,
                node_version.clone(),
            )?;
            log::info!("🚀 Using prepare-worker binary at: {:?}", prep_worker_path);
            log::info!("🚀 Using execute-worker binary at: {:?}", exec_worker_path);

            Some(CandidateValidationConfig {
                artifacts_cache_path: config
                    .database
                    .path()
                    .ok_or(Error::DatabasePathRequired)?
                    .join("pvf-artifacts"),
                node_version,
                secure_validator_mode,
                prep_worker_path,
                exec_worker_path,
                pvf_execute_workers_max_num: execute_workers_max_num.unwrap_or(2),
                pvf_prepare_workers_soft_max_num: prepare_workers_soft_max_num.unwrap_or(1),
                pvf_prepare_workers_hard_max_num: prepare_workers_hard_max_num.unwrap_or(2),
            })
        } else {
            None
        };
        let (statement_req_receiver, cfg) =
            IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
        net_config.add_request_response_protocol(cfg);
        let (candidate_req_v2_receiver, cfg) =
            IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
        net_config.add_request_response_protocol(cfg);
        let (dispute_req_receiver, cfg) =
            IncomingRequest::get_config_receiver::<_, Network>(&req_protocol_names);
        net_config.add_request_response_protocol(cfg);
        let approval_voting_config = ApprovalVotingConfig {
            col_approval_data: parachains_db::REAL_COLUMNS.col_approval_data,
            slot_duration_millis: slot_duration.as_millis() as u64,
        };
        let dispute_coordinator_config = DisputeCoordinatorConfig {
            col_dispute_data: parachains_db::REAL_COLUMNS.col_dispute_coordinator_data,
        };
        let chain_selection_config = ChainSelectionConfig {
            col_data: parachains_db::REAL_COLUMNS.col_chain_selection_data,
            stagnant_check_interval: Default::default(),
            stagnant_check_mode: chain_selection_subsystem::StagnantCheckMode::PruneOnly,
        };

        let fetch_chunks_threshold = Some(FETCH_CHUNKS_THRESHOLD);

        Some(ExtendedOverseerGenArgs {
            keystore,
            parachains_db,
            candidate_validation_config,
            availability_config: AVAILABILITY_CONFIG,
            pov_req_receiver,
            chunk_req_v1_receiver,
            chunk_req_v2_receiver,
            statement_req_receiver,
            candidate_req_v2_receiver,
            approval_voting_config,
            dispute_req_receiver,
            dispute_coordinator_config,
            chain_selection_config,
            fetch_chunks_threshold,
        })
    };

    let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_params: Some(WarpSyncParams::WithProvider(warp_sync)),
            block_relay: None,
            metrics,
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
                network_provider: Arc::new(network.clone()),
                is_validator: role.is_authority(),
                enable_http_requests: false,
                custom_extensions: move |_| vec![],
            })
            .run(client.clone(), task_manager.spawn_handle())
            .boxed(),
        );
    }

    let frontier_backend = Arc::new(frontier_backend);

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

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();
        let network = network.clone();
        let sync_service = sync_service.clone();

        let is_authority = role.is_authority();
        let enable_dev_signer = eth_config.enable_dev_signer;
        let max_past_logs = eth_config.max_past_logs;
        let execute_gas_limit_multiplier = eth_config.execute_gas_limit_multiplier;
        let filter_pool = filter_pool.clone();
        let frontier_backend = frontier_backend.clone();
        let pubsub_notification_sinks = pubsub_notification_sinks.clone();
        let storage_override = storage_override.clone();
        let fee_history_cache = fee_history_cache.clone();
        let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
            task_manager.spawn_handle(),
            storage_override.clone(),
            eth_config.eth_log_block_cache,
            eth_config.eth_statuses_cache,
            prometheus_registry.clone(),
        ));

        let target_gas_price = eth_config.target_gas_price;
        let pending_create_inherent_data_providers = move |_, ()| async move {
            let current = sp_timestamp::InherentDataProvider::from_system_time();
            let next_slot = current.timestamp().as_millis() + slot_duration.as_millis();
            let timestamp = sp_timestamp::InherentDataProvider::new(next_slot.into());
            let slot = sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                *timestamp,
                slot_duration,
            );
            let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
            Ok((slot, timestamp, dynamic_fee))
        };

        let node_name = name.clone();

        move |deny_unsafe, subscription_executor: polkadot_rpc::SubscriptionTaskExecutor| {
            let eth_deps = vitreus_rpc::EthDeps {
                client: client.clone(),
                pool: pool.clone(),
                graph: pool.pool().clone(),
                converter: Some(TransactionConverter),
                is_authority,
                enable_dev_signer,
                network: network.clone(),
                sync: sync_service.clone(),
                frontier_backend: match &*frontier_backend {
                    fc_db::Backend::KeyValue(b) => b.clone(),
                    fc_db::Backend::Sql(b) => b.clone(),
                },
                storage_override: storage_override.clone(),
                block_data_cache: block_data_cache.clone(),
                filter_pool: filter_pool.clone(),
                max_past_logs,
                fee_history_cache: fee_history_cache.clone(),
                fee_history_cache_limit,
                execute_gas_limit_multiplier,
                forced_parent_hashes: None,
                pending_create_inherent_data_providers,
            };

            let node_deps = vitreus_rpc::NodeDeps { name: node_name.clone() };

            let deps =
                vitreus_rpc::ExtraDeps { client: client.clone(), eth: eth_deps, node: node_deps };

            vitreus_rpc::create_full(
                rpc_extensions_builder(deny_unsafe, subscription_executor.clone())?,
                deps,
                subscription_executor,
                pubsub_notification_sinks.clone(),
            )
            .map_err(Into::into)
        }
    };

    let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        config,
        backend: backend.clone(),
        client: client.clone(),
        keystore: keystore_container.keystore(),
        network: network.clone(),
        sync_service: sync_service.clone(),
        rpc_builder: Box::new(rpc_extensions_builder),
        transaction_pool: transaction_pool.clone(),
        task_manager: &mut task_manager,
        system_rpc_tx,
        tx_handler_controller,
        telemetry: telemetry.as_mut(),
    })?;

    if let Some(hwbench) = hwbench {
        sc_sysinfo::print_hwbench(&hwbench);
        match SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench) {
            Err(err) if role.is_authority() => {
                log::warn!(
				"⚠️  The hardware does not meet the minimal requirements {} for role 'Authority' find out more at:\n\
				https://wiki.polkadot.network/docs/maintain-guides-how-to-validate-polkadot#reference-hardware",
				err
			);
            },
            _ => {},
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

    let authority_discovery_service =
		// We need the authority discovery if this node is either a validator or running alongside a parachain node.
		// Parachains node require the authority discovery for finding relay chain validators for sending
		// their PoVs or recovering PoVs.
		if role.is_authority() || is_parachain_node.is_running_alongside_parachain_node() {
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
                public_addresses: auth_disc_public_addresses,
                // Require that authority discovery records are signed.
                strict_record_validation: true,
                ..Default::default()
            },
            client.clone(),
            Arc::new(network.clone()),
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

    let runtime_client = Arc::new(DefaultSubsystemClient::new(
        overseer_client.clone(),
        OffchainTransactionPoolFactory::new(transaction_pool.clone()),
    ));

    let overseer_handle = if let Some(authority_discovery_service) = authority_discovery_service {
        let (overseer, overseer_handle) = overseer_gen
            .generate::<sc_service::SpawnTaskHandle, DefaultSubsystemClient<FullClient>>(
                overseer_connector,
                OverseerGenArgs {
                    runtime_client,
                    network_service: network.clone(),
                    sync_service: sync_service.clone(),
                    authority_discovery_service,
                    collation_req_v1_receiver,
                    collation_req_v2_receiver,
                    available_data_req_receiver,
                    registry: prometheus_registry.as_ref(),
                    spawner,
                    is_parachain_node,
                    overseer_message_channel_capacity_override,
                    req_protocol_names,
                    peerset_protocol_names,
                    notification_services,
                },
                ext_overseer_args,
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

    spawn_frontier_tasks(
        &task_manager,
        client.clone(),
        backend.clone(),
        frontier_backend,
        filter_pool,
        storage_override,
        fee_history_cache,
        fee_history_cache_limit,
        sync_service.clone(),
        pubsub_notification_sinks,
    );

    if role.is_authority() {
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
        let target_gas_price = eth_config.target_gas_price;
        let babe_config = sc_consensus_babe::BabeParams {
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
                            client_clone,
                            overseer_handle,
                            parent,
                        );

                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let slot =
                        sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                            *timestamp,
                            slot_duration,
                        );

                    let dynamic_fee =
                        fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));

                    Ok((slot, timestamp, parachain, dynamic_fee))
                }
            },
            force_authoring,
            backoff_authoring_blocks,
            babe_link,
            block_proposal_slot_portion: sc_consensus_babe::SlotProportion::new(2f32 / 3f32),
            max_block_proposal_slot_portion: None,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        };

        let babe = sc_consensus_babe::start_babe(babe_config)?;
        task_manager.spawn_essential_handle().spawn_blocking("babe", None, babe);
    }

    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore_opt = if role.is_authority() { Some(keystore_container.keystore()) } else { None };

    // beefy is enabled if its notification service exists
    if let Some(notification_service) = beefy_notification_service {
        let justifications_protocol_name = beefy_on_demand_justifications_handler.protocol_name();
        let network_params = sc_consensus_beefy::BeefyNetworkParams {
            network: Arc::new(network.clone()),
            sync: sync_service.clone(),
            gossip_protocol_name: beefy_gossip_proto_name,
            justifications_protocol_name,
            notification_service,
            _phantom: core::marker::PhantomData::<Block>,
        };
        let payload_provider = sp_consensus_beefy::mmr::MmrRootProvider::new(client.clone());
        let beefy_params = sc_consensus_beefy::BeefyParams {
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
            is_authority: role.is_authority(),
        };

        let gadget = sc_consensus_beefy::start_beefy_gadget::<
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            ecdsa_crypto::AuthorityId,
        >(beefy_params);

        // BEEFY is part of consensus, if it fails we'll bring the node down with it to make sure it
        // is noticed.
        task_manager
            .spawn_essential_handle()
            .spawn_blocking("beefy-gadget", None, gadget);
    }
    // When offchain indexing is enabled, MMR gadget should also run.
    if is_offchain_indexing_enabled {
        task_manager.spawn_essential_handle().spawn_blocking(
            "mmr-gadget",
            None,
            MmrGadget::start(
                client.clone(),
                backend.clone(),
                sp_mmr_primitives::INDEXING_PREFIX.to_vec(),
            ),
        );
    }

    let config = sc_consensus_grandpa::Config {
        // FIXME substrate#1578 make this available through chainspec
        // Grandpa performance can be improved a bit by tuning this parameter, see:
        // https://github.com/paritytech/polkadot/issues/5464
        gossip_duration: Duration::from_millis(1000),
        justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
        name: Some(name),
        observer_enabled: false,
        keystore: keystore_opt,
        local_role: role,
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        protocol_name: grandpa_protocol_name,
    };

    let enable_grandpa = !disable_grandpa;
    if enable_grandpa {
        // start the full GRANDPA voter
        // NOTE: unlike in substrate we are currently running the full
        // GRANDPA voter protocol for all full nodes (regardless of whether
        // they're validators or not). at this point the full voter should
        // provide better guarantees of block and vote data availability than
        // the observer.

        let mut voting_rules_builder = sc_consensus_grandpa::VotingRulesBuilder::default();

        #[cfg(not(feature = "malus"))]
        let _malus_finality_delay = None;

        if let Some(delay) = _malus_finality_delay {
            info!(?delay, "Enabling malus finality delay",);
            voting_rules_builder =
                voting_rules_builder.add(sc_consensus_grandpa::BeforeBestBlockBy(delay));
        };

        let grandpa_config = sc_consensus_grandpa::GrandpaParams {
            config,
            link: link_half,
            network: network.clone(),
            sync: sync_service.clone(),
            voting_rule: voting_rules_builder.build(),
            prometheus_registry: prometheus_registry.clone(),
            shared_voter_state,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            notification_service: grandpa_notification_service,
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
        };

        task_manager.spawn_essential_handle().spawn_blocking(
            "grandpa-voter",
            None,
            sc_consensus_grandpa::run_grandpa_voter(grandpa_config)?,
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

#[cfg(feature = "full-node")]
macro_rules! chain_ops {
    ($config:expr, $eth_config:expr, $jaeger_agent:expr, $telemetry_worker_handle:expr) => {{
        let telemetry_worker_handle = $telemetry_worker_handle;
        let jaeger_agent = $jaeger_agent;
        let eth_config = $eth_config;
        let mut config = $config;
        let basics = new_partial_basics(config, jaeger_agent, telemetry_worker_handle)?;

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
    jaeger_agent: Option<std::net::SocketAddr>,
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

    chain_ops!(config, eth_config, jaeger_agent, None)
}

/// Build a full node.
///
/// The actual "flavor", aka if it will use `Polkadot`, `Rococo` or `Kusama` is determined based on
/// [`IdentifyVariant`] using the chain spec.
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

fn revert_approval_voting(db: Arc<dyn Database>, hash: Hash) -> sp_blockchain::Result<()> {
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
    );

    approval_voting
        .revert_to(hash)
        .map_err(|err| sp_blockchain::Error::Backend(err.to_string()))
}
