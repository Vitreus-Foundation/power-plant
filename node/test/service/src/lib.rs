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

//! Polkadot test service only.

#![warn(missing_docs)]

pub mod chain_spec;

pub use chain_spec::*;
use futures::{future::Future, stream::StreamExt};
use polkadot_node_primitives::{CollationGenerationConfig, CollatorFn};
use polkadot_node_subsystem::messages::{CollationGenerationMessage, CollatorProtocolMessage};
use polkadot_overseer::Handle;
use polkadot_primitives::{Balance, CollatorPair, HeadData, Id as ParaId, ValidationCode};
use polkadot_runtime_common::BlockHashCount;
use polkadot_runtime_parachains::paras::{ParaGenesisArgs, ParaKind};
use polkadot_service::{Error, FullClient, IsCollator, NewFull, PrometheusConfig};
use vitreus_power_plant_runtime::{
    AccountId, ParasCall, ParasSudoWrapperCall, Runtime, SignedExtra, SignedPayload, SudoCall,
    UncheckedExtrinsic, VERSION,
};

use sc_chain_spec::ChainSpec;
use sc_client_api::BlockchainEvents;
use sc_network::{
    config::{NetworkConfiguration, TransportConfig},
    multiaddr, NetworkStateInfo,
};
use sc_service::{
    config::{
        DatabaseSource, KeystoreConfig, MultiaddrWithPeerId, WasmExecutionMethod,
        WasmtimeInstantiationStrategy,
    },
    BasePath, BlocksPruning, Configuration, Role, RpcHandlers, TaskManager,
};
use sp_arithmetic::traits::SaturatedConversion;
use sp_blockchain::HeaderBackend;
use sp_core::{ecdsa, Pair};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{codec::Encode, generic};
use sp_state_machine::BasicExternalities;
use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};
use substrate_test_client::{
    BlockchainEventsExt, RpcHandlersExt, RpcTransactionError, RpcTransactionOutput,
};

/// The client type being used by the test service.
pub type Client = FullClient;

pub use polkadot_service::FullBackend;

/// Create a new full node.
#[sc_tracing::logging::prefix_logs_with(config.network.node_name.as_str())]
pub fn new_full(
    config: Configuration,
    is_collator: IsCollator,
    worker_program_path: Option<PathBuf>,
) -> Result<NewFull, Error> {
    let eth_config = polkadot_service::eth::EthConfiguration {
        max_past_logs: 10000,
        fee_history_limit: 2048,
        enable_dev_signer: false,
        target_gas_price: 1,
        execute_gas_limit_multiplier: 10,
        eth_log_block_cache: 50,
        eth_statuses_cache: 50,
        frontier_backend_type: Default::default(),
        frontier_sql_backend_pool_size: 100,
        frontier_sql_backend_num_ops_timeout: 10000000,
        frontier_sql_backend_thread_count: 4,
        frontier_sql_backend_cache_size: 209715200,
    };

    polkadot_service::new_full(
        config,
        eth_config,
        is_collator,
        None,
        true,
        None,
        None,
        worker_program_path,
        false,
        polkadot_service::RealOverseerGen,
        None,
        None,
        None,
        None,
    )
}

/// Returns a prometheus config usable for testing.
pub fn test_prometheus_config(port: u16) -> PrometheusConfig {
    PrometheusConfig::new_with_default_registry(
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
        "test-chain".to_string(),
    )
}

/// Create a Polkadot `Configuration`.
///
/// By default an in-memory socket will be used, therefore you need to provide boot
/// nodes if you want the future node to be connected to other nodes.
///
/// The `storage_update_func` function will be executed in an externalities provided environment
/// and can be used to make adjustments to the runtime genesis storage.
pub fn node_config(
    storage_update_func: impl Fn(),
    tokio_handle: tokio::runtime::Handle,
    key: Sr25519Keyring,
    boot_nodes: Vec<MultiaddrWithPeerId>,
    is_validator: bool,
) -> Configuration {
    let base_path = BasePath::new_temp_dir().expect("could not create temporary directory");
    let root = base_path.path().join(key.to_string());
    let role = if is_validator { Role::Authority } else { Role::Full };
    let key_seed = key.to_seed();
    let mut spec = polkadot_local_testnet_config();
    let mut storage = spec.as_storage_builder().build_storage().expect("could not build storage");

    BasicExternalities::execute_with_storage(&mut storage, storage_update_func);
    spec.set_storage(storage);

    let mut network_config = NetworkConfiguration::new(
        key_seed.to_string(),
        "network/test/0.1",
        Default::default(),
        None,
    );

    network_config.boot_nodes = boot_nodes;

    network_config.allow_non_globals_in_dht = true;

    let addr: multiaddr::Multiaddr = multiaddr::Protocol::Memory(rand::random()).into();
    network_config.listen_addresses.push(addr.clone());

    network_config.public_addresses.push(addr);

    network_config.transport = TransportConfig::MemoryOnly;

    Configuration {
        impl_name: "polkadot-test-node".to_string(),
        impl_version: "0.1".to_string(),
        role,
        tokio_handle,
        transaction_pool: Default::default(),
        network: network_config,
        keystore: KeystoreConfig::InMemory,
        database: DatabaseSource::RocksDb { path: root.join("db"), cache_size: 128 },
        trie_cache_maximum_size: Some(64 * 1024 * 1024),
        state_pruning: Default::default(),
        blocks_pruning: BlocksPruning::KeepFinalized,
        chain_spec: Box::new(spec),
        wasm_method: WasmExecutionMethod::Compiled {
            instantiation_strategy: WasmtimeInstantiationStrategy::PoolingCopyOnWrite,
        },
        wasm_runtime_overrides: Default::default(),
        rpc_addr: Default::default(),
        rpc_max_request_size: Default::default(),
        rpc_max_response_size: Default::default(),
        rpc_max_connections: Default::default(),
        rpc_cors: None,
        rpc_methods: Default::default(),
        rpc_id_provider: None,
        rpc_max_subs_per_conn: Default::default(),
        rpc_port: 9944,
        prometheus_config: None,
        telemetry_endpoints: None,
        default_heap_pages: None,
        offchain_worker: Default::default(),
        force_authoring: false,
        disable_grandpa: false,
        dev_key_seed: Some(key_seed),
        tracing_targets: None,
        tracing_receiver: Default::default(),
        max_runtime_instances: 8,
        runtime_cache_size: 2,
        announce_block: true,
        data_path: root,
        base_path,
        informant_output_format: Default::default(),
    }
}

/// Run a test validator node that uses the test runtime and specified `config`.
pub fn run_validator_node(
    config: Configuration,
    worker_program_path: Option<PathBuf>,
) -> PolkadotTestNode {
    let multiaddr = config.network.listen_addresses[0].clone();
    let NewFull { task_manager, client, network, rpc_handlers, overseer_handle, .. } =
        new_full(config, IsCollator::No, worker_program_path)
            .expect("could not create Polkadot test service");

    let overseer_handle = overseer_handle.expect("test node must have an overseer handle");
    let peer_id = network.local_peer_id().clone();
    let addr = MultiaddrWithPeerId { multiaddr, peer_id };

    PolkadotTestNode { task_manager, client, overseer_handle, addr, rpc_handlers }
}

/// Run a test collator node that uses the test runtime.
///
/// The node will be using an in-memory socket, therefore you need to provide boot nodes if you
/// want it to be connected to other nodes.
///
/// The `storage_update_func` function will be executed in an externalities provided environment
/// and can be used to make adjustments to the runtime genesis storage.
///
/// # Note
///
/// The collator functionality still needs to be registered at the node! This can be done using
/// [`PolkadotTestNode::register_collator`].
pub fn run_collator_node(
    tokio_handle: tokio::runtime::Handle,
    key: Sr25519Keyring,
    storage_update_func: impl Fn(),
    boot_nodes: Vec<MultiaddrWithPeerId>,
    collator_pair: CollatorPair,
) -> PolkadotTestNode {
    let config = node_config(storage_update_func, tokio_handle, key, boot_nodes, false);
    let multiaddr = config.network.listen_addresses[0].clone();
    let NewFull { task_manager, client, network, rpc_handlers, overseer_handle, .. } =
        new_full(config, IsCollator::Yes(collator_pair), None)
            .expect("could not create Polkadot test service");

    let overseer_handle = overseer_handle.expect("test node must have an overseer handle");
    let peer_id = network.local_peer_id().clone();
    let addr = MultiaddrWithPeerId { multiaddr, peer_id };

    PolkadotTestNode { task_manager, client, overseer_handle, addr, rpc_handlers }
}

/// A Polkadot test node instance used for testing.
pub struct PolkadotTestNode {
    /// `TaskManager`'s instance.
    pub task_manager: TaskManager,
    /// Client's instance.
    pub client: Arc<Client>,
    /// A handle to Overseer.
    pub overseer_handle: Handle,
    /// The `MultiaddrWithPeerId` to this node. This is useful if you want to pass it as "boot node" to other nodes.
    pub addr: MultiaddrWithPeerId,
    /// `RPCHandlers` to make RPC queries.
    pub rpc_handlers: RpcHandlers,
}

impl PolkadotTestNode {
    /// Send a sudo call to this node.
    async fn send_sudo(
        &self,
        call: impl Into<vitreus_power_plant_runtime::RuntimeCall>,
        caller: ecdsa::Pair,
        nonce: u32,
    ) -> Result<(), RpcTransactionError> {
        let sudo = SudoCall::sudo { call: Box::new(call.into()) };

        let extrinsic = construct_extrinsic(&*self.client, sudo, caller, nonce);
        self.rpc_handlers.send_transaction(extrinsic.into()).await.map(drop)
    }

    /// Send an extrinsic to this node.
    pub async fn send_extrinsic(
        &self,
        function: impl Into<vitreus_power_plant_runtime::RuntimeCall>,
        caller: ecdsa::Pair,
    ) -> Result<RpcTransactionOutput, RpcTransactionError> {
        let extrinsic = construct_extrinsic(&*self.client, function, caller, 0);

        self.rpc_handlers.send_transaction(extrinsic.into()).await
    }

    /// Register a parachain at this relay chain.
    pub async fn register_parachain(
        &self,
        id: ParaId,
        validation_code: impl Into<ValidationCode>,
        genesis_head: impl Into<HeadData>,
    ) -> Result<(), RpcTransactionError> {
        let validation_code: ValidationCode = validation_code.into();
        let call = ParasSudoWrapperCall::sudo_schedule_para_initialize {
            id,
            genesis: ParaGenesisArgs {
                genesis_head: genesis_head.into(),
                validation_code: validation_code.clone(),
                para_kind: ParaKind::Parachain,
            },
        };

        self.send_sudo(call, test_accounts::alice(), 0).await?;

        // Bypass pvf-checking.
        let call = ParasCall::add_trusted_validation_code { validation_code };
        self.send_sudo(call, test_accounts::alice(), 1).await
    }

    /// Wait for `count` blocks to be imported in the node and then exit. This function will not return if no blocks
    /// are ever created, thus you should restrict the maximum amount of time of the test execution.
    pub fn wait_for_blocks(&self, count: usize) -> impl Future<Output = ()> {
        self.client.wait_for_blocks(count)
    }

    /// Wait for `count` blocks to be finalized and then exit. Similarly with `wait_for_blocks` this function will
    /// not return if no block are ever finalized.
    pub async fn wait_for_finalized_blocks(&self, count: usize) {
        let mut import_notification_stream = self.client.finality_notification_stream();
        let mut blocks = HashSet::new();

        while let Some(notification) = import_notification_stream.next().await {
            blocks.insert(notification.hash);
            if blocks.len() == count {
                break;
            }
        }
    }

    /// Register the collator functionality in the overseer of this node.
    pub async fn register_collator(
        &mut self,
        collator_key: CollatorPair,
        para_id: ParaId,
        collator: CollatorFn,
    ) {
        let config = CollationGenerationConfig { key: collator_key, collator, para_id };

        self.overseer_handle
            .send_msg(CollationGenerationMessage::Initialize(config), "Collator")
            .await;

        self.overseer_handle
            .send_msg(CollatorProtocolMessage::CollateOn(para_id), "Collator")
            .await;
    }
}

/// Construct an extrinsic that can be applied to the test runtime.
pub fn construct_extrinsic(
    client: &Client,
    function: impl Into<vitreus_power_plant_runtime::RuntimeCall>,
    caller: ecdsa::Pair,
    nonce: u32,
) -> UncheckedExtrinsic {
    let function = function.into();
    let current_block_hash = client.info().best_hash;
    let current_block = client.info().best_number.saturated_into();
    let genesis_block = client.hash(0).unwrap().unwrap();
    let period =
        BlockHashCount::get().checked_next_power_of_two().map(|c| c / 2).unwrap_or(2) as u64;
    let tip = 0;
    let extra: SignedExtra = (
        frame_system::CheckNonZeroSender::<Runtime>::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
        frame_system::CheckNonce::<Runtime>::from(nonce),
        frame_system::CheckWeight::<Runtime>::new(),
        pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
        pallet_energy_fee::CheckEnergyFee::<Runtime>::new(),
    );
    let raw_payload = SignedPayload::from_raw(
        function.clone(),
        extra.clone(),
        (
            (),
            VERSION.spec_version,
            VERSION.transaction_version,
            genesis_block,
            current_block_hash,
            (),
            (),
            (),
            (),
        ),
    );
    let signature = raw_payload.using_encoded(|e| caller.sign(e));
    UncheckedExtrinsic::new_signed(
        function.clone(),
        AccountId::from(caller.public()),
        vitreus_power_plant_runtime::Signature::new(signature),
        extra,
    )
}

/// Construct a transfer extrinsic.
pub fn construct_transfer_extrinsic(
    client: &Client,
    origin: ecdsa::Pair,
    dest: ecdsa::Pair,
    value: Balance,
) -> UncheckedExtrinsic {
    let function = vitreus_power_plant_runtime::RuntimeCall::Balances(
        pallet_balances::Call::transfer_allow_death { dest: dest.public().into(), value },
    );

    construct_extrinsic(client, function, origin, 0)
}

#[allow(missing_docs)]
pub mod test_accounts {
    use sp_core::crypto::Pair;
    use sp_core::ecdsa;

    pub fn alice() -> ecdsa::Pair {
        derive_dev("Alice")
    }

    pub fn bob() -> ecdsa::Pair {
        derive_dev("Bob")
    }

    pub fn charlie() -> ecdsa::Pair {
        derive_dev("Charlie")
    }

    fn derive_dev(seed: &str) -> ecdsa::Pair {
        Pair::from_string(&format!("//{}", seed), None).expect("static values are valid; qed")
    }
}
