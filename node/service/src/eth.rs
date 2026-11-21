use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{future, prelude::*};
// Substrate
use sc_client_api::BlockchainEvents;
use sc_executor::HostFunctions;
use sc_network::service::traits::NetworkService;
use sc_network_sync::SyncingService;
use sc_service::{error::Error as ServiceError, Configuration, SpawnTaskHandle, TaskManager};
use sc_transaction_pool::TransactionPoolHandle;
use sp_api::ConstructRuntimeApi;
use sp_consensus_babe::SlotDuration;
use sp_core::{H256, U256};
use sp_runtime::traits::Block as BlockT;
// Frontier
pub use fc_consensus::FrontierBlockImport;
use fc_rpc::EthTask;
pub use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
pub use fc_storage::{StorageOverride, StorageOverrideHandler};

use vitreus_power_plant_runtime::TransactionConverter;

use crate::Block;

/// Full backend.
type FullBackend<B> = sc_service::TFullBackend<B>;
/// Full client.
type FullClient<B, RA, HF> = sc_service::TFullClient<B, RA, sc_executor::WasmExecutor<HF>>;

/// Frontier DB backend type.
pub type FrontierBackend<B, C> = fc_db::Backend<B, C>;

pub fn db_config_dir(config: &Configuration) -> PathBuf {
    config.base_path.config_dir(config.chain_spec.id())
}

/// Available frontier backend types.
#[derive(Debug, Copy, Clone, Default, clap::ValueEnum)]
pub enum BackendType {
    /// Either RocksDb or ParityDb as per inherited from the global backend settings.
    #[default]
    KeyValue,
    /// Sql database with custom log indexing.
    Sql,
}

/// The ethereum-compatibility configuration used to run a node.
#[derive(Clone, Debug, clap::Parser)]
pub struct EthConfiguration {
    /// Maximum number of logs in a query.
    #[arg(long, default_value = "10000")]
    pub max_past_logs: u32,

    /// Maximum number of blocks that eth_getLogs searches in one query. No limit by default.
    #[arg(long, default_value_t = u32::MAX)]
    pub max_block_range: u32,

    /// Maximum fee history cache size.
    #[arg(long, default_value = "2048")]
    pub fee_history_limit: u64,

    #[arg(long)]
    pub enable_dev_signer: bool,

    /// The dynamic-fee pallet target gas price set by block author
    #[arg(long, default_value = "1")]
    pub target_gas_price: u64,

    /// Maximum allowed gas limit will be `block.gas_limit * execute_gas_limit_multiplier`
    /// when using eth_call/eth_estimateGas.
    #[arg(long, default_value = "10")]
    pub execute_gas_limit_multiplier: u64,

    /// Size in bytes of the LRU cache for block data.
    #[arg(long, default_value = "50")]
    pub eth_log_block_cache: usize,

    /// Size in bytes of the LRU cache for transactions statuses data.
    #[arg(long, default_value = "50")]
    pub eth_statuses_cache: usize,

    /// Sets the frontier backend type (KeyValue or Sql)
    #[arg(long, value_enum, ignore_case = true, default_value_t = BackendType::default())]
    pub frontier_backend_type: BackendType,

    // Sets the SQL backend's pool size.
    #[arg(long, default_value = "100")]
    pub frontier_sql_backend_pool_size: u32,

    /// Sets the SQL backend's query timeout in number of VM ops.
    #[arg(long, default_value = "10000000")]
    pub frontier_sql_backend_num_ops_timeout: u32,

    /// Sets the SQL backend's auxiliary thread limit.
    #[arg(long, default_value = "4")]
    pub frontier_sql_backend_thread_count: u32,

    /// Sets the SQL backend's query timeout in number of VM ops.
    /// Default value is 200MB.
    #[arg(long, default_value = "209715200")]
    pub frontier_sql_backend_cache_size: u64,
}

pub struct FrontierPartialComponents {
    pub filter_pool: Option<FilterPool>,
    pub fee_history_cache: FeeHistoryCache,
    pub fee_history_cache_limit: FeeHistoryCacheLimit,
}

pub fn new_frontier_partial(
    config: &EthConfiguration,
) -> Result<FrontierPartialComponents, ServiceError> {
    Ok(FrontierPartialComponents {
        filter_pool: Some(Arc::new(Mutex::new(BTreeMap::new()))),
        fee_history_cache: Arc::new(Mutex::new(BTreeMap::new())),
        fee_history_cache_limit: config.fee_history_limit,
    })
}

/// A set of APIs that ethereum-compatible runtimes must implement.
pub trait EthCompatRuntimeApiCollection<Block: BlockT>:
    sp_api::ApiExt<Block>
    + fp_rpc::ConvertTransactionRuntimeApi<Block>
    + fp_rpc::EthereumRuntimeRPCApi<Block>
{
}

impl<Block, Api> EthCompatRuntimeApiCollection<Block> for Api
where
    Block: BlockT,
    Api: sp_api::ApiExt<Block>
        + fp_rpc::ConvertTransactionRuntimeApi<Block>
        + fp_rpc::EthereumRuntimeRPCApi<Block>,
{
}

pub fn spawn_frontier_tasks<B, RA, HF>(
    task_manager: &TaskManager,
    client: Arc<FullClient<B, RA, HF>>,
    backend: Arc<FullBackend<B>>,
    frontier_backend: Arc<FrontierBackend<B, FullClient<B, RA, HF>>>,
    filter_pool: Option<FilterPool>,
    storage_override: Arc<dyn StorageOverride<B>>,
    fee_history_cache: FeeHistoryCache,
    fee_history_cache_limit: FeeHistoryCacheLimit,
    sync: Arc<SyncingService<B>>,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<
            fc_mapping_sync::EthereumBlockNotification<B>,
        >,
    >,
) where
    B: BlockT<Hash = H256>,
    RA: ConstructRuntimeApi<B, FullClient<B, RA, HF>>,
    RA: Send + Sync + 'static,
    RA::RuntimeApi: EthCompatRuntimeApiCollection<B>,
    HF: HostFunctions + 'static,
{
    // Spawn main mapping sync worker background task.
    match &*frontier_backend {
        fc_db::Backend::KeyValue(b) => {
            task_manager.spawn_essential_handle().spawn(
                "frontier-mapping-sync-worker",
                Some("frontier"),
                fc_mapping_sync::kv::MappingSyncWorker::new(
                    client.import_notification_stream(),
                    Duration::new(6, 0),
                    client.clone(),
                    backend,
                    storage_override.clone(),
                    b.clone(),
                    3,
                    0u32.into(),
                    fc_mapping_sync::SyncStrategy::Normal,
                    sync,
                    pubsub_notification_sinks,
                )
                .for_each(|()| future::ready(())),
            );
        },
        fc_db::Backend::Sql(b) => {
            task_manager.spawn_essential_handle().spawn_blocking(
                "frontier-mapping-sync-worker",
                Some("frontier"),
                fc_mapping_sync::sql::SyncWorker::run(
                    client.clone(),
                    backend,
                    b.clone(),
                    client.import_notification_stream(),
                    fc_mapping_sync::sql::SyncWorkerConfig {
                        read_notification_timeout: Duration::from_secs(30),
                        check_indexed_blocks_interval: Duration::from_secs(60),
                    },
                    fc_mapping_sync::SyncStrategy::Parachain,
                    sync,
                    pubsub_notification_sinks,
                ),
            );
        },
    }

    // Spawn Frontier EthFilterApi maintenance task.
    if let Some(filter_pool) = filter_pool {
        // Each filter is allowed to stay in the pool for 100 blocks.
        const FILTER_RETAIN_THRESHOLD: u64 = 100;
        task_manager.spawn_essential_handle().spawn(
            "frontier-filter-pool",
            Some("frontier"),
            EthTask::filter_pool_task(client.clone(), filter_pool, FILTER_RETAIN_THRESHOLD),
        );
    }

    // Spawn Frontier FeeHistory cache maintenance task.
    task_manager.spawn_essential_handle().spawn(
        "frontier-fee-history",
        Some("frontier"),
        EthTask::fee_history_task(
            client,
            storage_override,
            fee_history_cache,
            fee_history_cache_limit,
        ),
    );
}

/// Opens the Frontier database in the format `eth_config` selects, and the storage override that
/// reads Ethereum data from the runtime state.
pub fn open_frontier_backend(
    client: &Arc<crate::FullClient>,
    config: &Configuration,
    eth_config: &EthConfiguration,
) -> Result<
    (FrontierBackend<Block, crate::FullClient>, Arc<dyn StorageOverride<Block>>),
    ServiceError,
> {
    let storage_override = Arc::new(StorageOverrideHandler::new(client.clone()));
    let frontier_backend = match eth_config.frontier_backend_type {
        BackendType::KeyValue => FrontierBackend::KeyValue(Arc::new(
            fc_db::kv::Backend::open(Arc::clone(client), &config.database, &db_config_dir(config))
                .map_err(ServiceError::Other)?,
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
            .unwrap_or_else(|err| panic!("failed creating sql backend: {err:?}"));
            FrontierBackend::Sql(Arc::new(backend))
        },
    };

    Ok((frontier_backend, storage_override))
}

/// Builds the node's RPC: the methods of `basic` plus our own and the Ethereum ones.
pub fn rpc_extensions_builder(
    basic: Box<
        dyn Fn(
            polkadot_rpc::SubscriptionTaskExecutor,
        ) -> Result<polkadot_rpc::RpcExtension, ServiceError>,
    >,
    config: &Configuration,
    eth_config: &EthConfiguration,
    client: Arc<crate::FullClient>,
    pool: Arc<TransactionPoolHandle<Block, crate::FullClient>>,
    network: Arc<dyn NetworkService>,
    sync_service: Arc<SyncingService<Block>>,
    frontier: &FrontierPartialComponents,
    frontier_backend: Arc<FrontierBackend<Block, crate::FullClient>>,
    storage_override: Arc<dyn StorageOverride<Block>>,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<
            fc_mapping_sync::EthereumBlockNotification<Block>,
        >,
    >,
    spawn_handle: SpawnTaskHandle,
    slot_duration: SlotDuration,
) -> impl Fn(polkadot_rpc::SubscriptionTaskExecutor) -> Result<polkadot_rpc::RpcExtension, ServiceError>
{
    let is_authority = config.role.is_authority();
    let enable_dev_signer = eth_config.enable_dev_signer;
    let max_past_logs = eth_config.max_past_logs;
    let max_block_range = eth_config.max_block_range;
    let execute_gas_limit_multiplier = eth_config.execute_gas_limit_multiplier;
    let filter_pool = frontier.filter_pool.clone();
    let fee_history_cache = frontier.fee_history_cache.clone();
    let fee_history_cache_limit = frontier.fee_history_cache_limit;
    let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
        spawn_handle,
        storage_override.clone(),
        eth_config.eth_log_block_cache,
        eth_config.eth_statuses_cache,
        config.prometheus_registry().cloned(),
    ));

    let target_gas_price = eth_config.target_gas_price;
    let pending_create_inherent_data_providers = move |_, ()| async move {
        let current = sp_timestamp::InherentDataProvider::from_system_time();
        let next_slot = current.timestamp().as_millis() + slot_duration.as_millis();
        let timestamp = sp_timestamp::InherentDataProvider::new(next_slot.into());
        let slot =
            sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                *timestamp,
                slot_duration,
            );
        let dynamic_fee = fp_dynamic_fee::InherentDataProvider(U256::from(target_gas_price));
        Ok((slot, timestamp, dynamic_fee))
    };

    let node_name = config.network.node_name.clone();

    move |subscription_executor: polkadot_rpc::SubscriptionTaskExecutor| {
        let eth_deps = vitreus_rpc::EthDeps {
            client: client.clone(),
            pool: pool.clone(),
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
            max_block_range,
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
            basic(subscription_executor.clone())?,
            deps,
            subscription_executor,
            pubsub_notification_sinks.clone(),
        )
        .map_err(Into::into)
    }
}
