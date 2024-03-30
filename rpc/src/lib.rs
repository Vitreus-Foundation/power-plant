//! A collection of node-specific RPC methods.

use std::sync::Arc;

use futures::channel::mpsc;
use jsonrpsee::RpcModule;
// Substrate
use sc_client_api::{
    backend::{Backend, StorageProvider},
    client::BlockchainEvents,
    AuxStore,
};
use sc_consensus_babe::BabeWorkerHandle;
use sc_consensus_beefy::communication::notification::{
    BeefyBestBlockStream, BeefyVersionedFinalityProofStream,
};
use sc_consensus_grandpa::{
    FinalityProofProvider, GrandpaJustificationStream, SharedAuthoritySet, SharedVoterState,
};
use sc_consensus_manual_seal::rpc::EngineCommand;
pub use sc_rpc::SubscriptionTaskExecutor;
use sc_rpc_api::DenyUnsafe;
use sc_service::TransactionPool;
use sc_transaction_pool::ChainApi;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_keystore::KeystorePtr;
use sp_runtime::traits::Block as BlockT;
// Runtime
use vitreus_power_plant_runtime::{opaque::Block, AccountId, Balance, BlockNumber, Hash, Nonce};

mod eth;
pub use self::eth::{create_eth, overrides_handle, EthDeps};

/// Extra dependencies for Node
pub struct NodeDeps {
    /// Node name defined during boot
    pub name: String,
}

/// Extra dependencies for BABE.
pub struct BabeDeps {
    /// The keystore that manages the keys of the node.
    pub keystore: KeystorePtr,
    /// The worker handle.
    pub worker_handle: BabeWorkerHandle<Block>,
}

/// Extra dependencies for GRANDPA
pub struct GrandpaDeps<B> {
    /// Voting round info.
    pub shared_voter_state: SharedVoterState,
    /// Authority set info.
    pub shared_authority_set: SharedAuthoritySet<Hash, BlockNumber>,
    /// Receives notifications about justification events from Grandpa.
    pub justification_stream: GrandpaJustificationStream<Block>,
    /// Executor to drive the subscription manager in the Grandpa RPC handler.
    pub subscription_executor: SubscriptionTaskExecutor,
    /// Finality proof provider.
    pub finality_provider: Arc<FinalityProofProvider<B, Block>>,
}

/// Dependencies for BEEFY
pub struct BeefyDeps {
    /// Receives notifications about finality proof events from BEEFY.
    pub beefy_finality_proof_stream: BeefyVersionedFinalityProofStream<Block>,
    /// Receives notifications about best block events from BEEFY.
    pub beefy_best_block_stream: BeefyBestBlockStream<Block>,
    /// Executor to drive the subscription manager in the BEEFY RPC handler.
    pub subscription_executor: sc_rpc::SubscriptionTaskExecutor,
}

/// Full client dependencies.
pub struct FullDeps<C, P, BE, A: ChainApi, CT, SC> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// The SelectChain Strategy
    pub select_chain: SC,
    /// A copy of the chain spec.
    pub chain_spec: Box<dyn sc_chain_spec::ChainSpec>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
    /// Manual seal command sink
    pub command_sink: Option<mpsc::Sender<EngineCommand<Hash>>>,
    /// Ethereum-compatibility specific dependencies.
    pub eth: EthDeps<C, P, A, CT, Block>,
    /// BABE specific dependencies.
    pub babe: BabeDeps,
    /// GRANDPA specific dependencies.
    pub grandpa: GrandpaDeps<BE>,
    /// BEEFY specific dependencies.
    pub beefy: BeefyDeps,
    /// Backend used by the node.
    pub backend: Arc<BE>,
    /// Node specific dependencies.
    pub node: NodeDeps,
}

pub struct DefaultEthConfig<C, BE>(std::marker::PhantomData<(C, BE)>);

impl<C, BE> fc_rpc::EthConfig<Block, C> for DefaultEthConfig<C, BE>
where
    C: sc_client_api::StorageProvider<Block, BE> + Sync + Send + 'static,
    BE: Backend<Block> + 'static,
{
    type EstimateGasAdapter = ();
    type RuntimeStorageOverride =
        fc_rpc::frontier_backend_client::SystemAccountId20StorageOverride<Block, C, BE>;
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, BE, A, CT, SC>(
    deps: FullDeps<C, P, BE, A, CT, SC>,
    subscription_task_executor: SubscriptionTaskExecutor,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<
            fc_mapping_sync::EthereumBlockNotification<Block>,
        >,
    >,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: CallApiAt<Block> + ProvideRuntimeApi<Block>,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: sp_block_builder::BlockBuilder<Block>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: mmr_rpc::MmrRuntimeApi<Block, <Block as sp_runtime::traits::Block>::Hash, BlockNumber>,
    C::Api: fp_rpc::ConvertTransactionRuntimeApi<Block>,
    C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
    C::Api: sc_consensus_babe::BabeApi<Block>,
    C::Api: energy_fee_rpc::EnergyFeeRuntimeApi<Block>,
    C::Api: vitreus_utility_runtime_api::UtilityApi<Block>,
    C::Api: energy_generation_rpc::EnergyGenerationRuntimeApi<Block>,
    C: AuxStore,
    C: BlockchainEvents<Block> + 'static,
    C: HeaderBackend<Block>
        + HeaderMetadata<Block, Error = BlockChainError>
        + StorageProvider<Block, BE>,
    BE: Backend<Block> + 'static,
    P: TransactionPool<Block = Block> + 'static,
    A: ChainApi<Block = Block> + 'static,
    CT: fp_rpc::ConvertTransaction<<Block as BlockT>::Extrinsic> + Send + Sync + 'static,
    SC: sp_consensus::SelectChain<Block> + 'static,
{
    use energy_fee_rpc::{EnergyFee, EnergyFeeApiServer};
    use energy_generation_rpc::{EnergyGeneration, EnergyGenerationApiServer};
    use mmr_rpc::{Mmr, MmrApiServer};
    use node_rpc_server::{Node, NodeApiServer};
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use sc_consensus_babe_rpc::{Babe, BabeApiServer};
    use sc_consensus_beefy_rpc::{Beefy, BeefyApiServer};
    use sc_consensus_grandpa_rpc::{Grandpa, GrandpaApiServer};
    use sc_consensus_manual_seal::rpc::{ManualSeal, ManualSealApiServer};
    use sc_sync_state_rpc::{SyncState, SyncStateApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};
    use substrate_state_trie_migration_rpc::{StateMigration, StateMigrationApiServer};

    let mut io = RpcModule::new(());
    let FullDeps {
        client,
        pool,
        select_chain,
        chain_spec,
        deny_unsafe,
        command_sink,
        eth,
        babe,
        grandpa,
        beefy,
        backend,
        node,
    } = deps;
    let BabeDeps { keystore, worker_handle } = babe;
    let GrandpaDeps {
        shared_voter_state,
        shared_authority_set,
        justification_stream,
        subscription_executor,
        finality_provider,
    } = grandpa;
    let NodeDeps { name } = node;

    io.merge(Node::new(name).into_rpc())?;
    io.merge(StateMigration::new(client.clone(), backend.clone(), deny_unsafe).into_rpc())?;
    io.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
    io.merge(TransactionPayment::new(client.clone()).into_rpc())?;
    io.merge(EnergyFee::new(client.clone()).into_rpc())?;
    io.merge(EnergyGeneration::new(client.clone()).into_rpc())?;
    io.merge(
        Babe::new(client.clone(), worker_handle.clone(), keystore, select_chain, deny_unsafe)
            .into_rpc(),
    )?;
    io.merge(
        Mmr::new(
            client.clone(),
            backend
                .offchain_storage()
                .ok_or("Backend doesn't provide the required offchain storage")?,
        )
        .into_rpc(),
    )?;
    io.merge(
        Grandpa::new(
            subscription_executor,
            shared_authority_set.clone(),
            shared_voter_state,
            justification_stream,
            finality_provider,
        )
        .into_rpc(),
    )?;
    io.merge(SyncState::new(chain_spec, client, shared_authority_set, worker_handle)?.into_rpc())?;
    io.merge(
        Beefy::<Block>::new(
            beefy.beefy_finality_proof_stream,
            beefy.beefy_best_block_stream,
            beefy.subscription_executor,
        )?
        .into_rpc(),
    )?;

    if let Some(command_sink) = command_sink {
        io.merge(
            // We provide the rpc handler with the sending end of the channel to allow the rpc
            // send EngineCommands to the background block authorship task.
            ManualSeal::new(command_sink).into_rpc(),
        )?;
    }

    // Ethereum compatibility RPCs
    let io = create_eth::<_, _, _, _, _, _, DefaultEthConfig<C, BE>>(
        io,
        eth,
        subscription_task_executor,
        pubsub_notification_sinks,
    )?;

    Ok(io)
}