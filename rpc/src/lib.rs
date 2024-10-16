//! A collection of node-specific RPC methods.

use std::sync::Arc;

use jsonrpsee::RpcModule;

// Substrate
use sc_client_api::{backend::StorageProvider, client::BlockchainEvents, AuxStore, UsageProvider};
use sc_rpc::SubscriptionTaskExecutor;
use sc_service::TransactionPool;
use sc_transaction_pool::ChainApi;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus::SelectChain;
use sp_consensus_babe::BabeApi;
use sp_consensus_beefy::AuthorityIdBound;
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::RuntimeAppPublic;

// Runtime
use vitreus_power_plant_runtime::{
    opaque::Block, AccountId, Balance, BlockNumber, Nonce, RuntimeCall,
};

mod consensus_data_providers;
mod eth;
pub use self::eth::{create_eth, EthDeps};

/// Extra dependencies for Node
pub struct NodeDeps {
    /// Node name defined during boot
    pub name: String,
}

/// Extra dependencies.
pub struct ExtraDeps<C, P, A: ChainApi, CT, CIDP> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Ethereum-compatibility specific dependencies.
    pub eth: EthDeps<Block, C, P, A, CT, CIDP>,
    /// Node specific dependencies.
    pub node: NodeDeps,
}

pub struct DefaultEthConfig<C, BE>(std::marker::PhantomData<(C, BE)>);

impl<C, BE> fc_rpc::EthConfig<Block, C> for DefaultEthConfig<C, BE>
where
    C: StorageProvider<Block, BE> + Sync + Send + 'static,
    BE: sc_client_api::Backend<Block> + 'static,
{
    type EstimateGasAdapter = ();
    type RuntimeStorageOverride =
        fc_rpc::frontier_backend_client::SystemAccountId20StorageOverride<Block, C, BE>;
}

/// Instantiate all RPC extensions.
pub fn create_full<C, P, A, CT, CIDP, B>(
    mut io: RpcModule<()>,
    ExtraDeps { client, eth, node }: ExtraDeps<C, P, A, CT, CIDP>,
    subscription_task_executor: SubscriptionTaskExecutor,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<
            fc_mapping_sync::EthereumBlockNotification<Block>,
        >,
    >,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + AuxStore
        + HeaderMetadata<Block, Error = BlockChainError>
        + CallApiAt<Block>
        + BlockchainEvents<Block>
        + UsageProvider<Block>
        + StorageProvider<Block, B>
        + 'static,
    C::Api: BlockBuilder<Block>,
    C::Api: fp_rpc::ConvertTransactionRuntimeApi<Block>,
    C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
    C::Api: energy_fee_rpc::EnergyFeeRuntimeApi<Block, AccountId, Balance, RuntimeCall>,
    C::Api: energy_generation_rpc::EnergyGenerationRuntimeApi<Block>,
    C::Api: vitreus_utility_runtime_api::UtilityApi<Block>,
    P: TransactionPool<Block = Block> + 'static,
    A: ChainApi<Block = Block> + 'static,
    CT: fp_rpc::ConvertTransaction<<Block as BlockT>::Extrinsic> + Send + Sync + 'static,
    CIDP: CreateInherentDataProviders<Block, ()> + Send + 'static,
    B: sc_client_api::Backend<Block> + Send + Sync + 'static,
{
    use energy_fee_rpc::{EnergyFee, EnergyFeeApiServer};
    use energy_generation_rpc::{EnergyGeneration, EnergyGenerationApiServer};
    use node_rpc_server::{Node, NodeApiServer};

    io.merge(EnergyFee::new(client.clone()).into_rpc())?;
    io.merge(EnergyGeneration::new(client.clone()).into_rpc())?;
    io.merge(Node::new(node.name).into_rpc())?;

    // Ethereum compatibility RPCs
    let io = create_eth::<_, _, _, _, _, _, _, DefaultEthConfig<C, B>>(
        io,
        eth,
        subscription_task_executor,
        pubsub_notification_sinks,
    )?;

    Ok(io)
}

/// A copy of `polkadot_rpc::create_full` because the original function supports only AccountId32
pub fn create_basic<C, P, SC, B, AuthorityId>(
    polkadot_rpc::FullDeps {
        client,
        pool,
        select_chain,
        chain_spec,
        deny_unsafe,
        babe,
        grandpa,
        beefy,
        backend,
    }: polkadot_rpc::FullDeps<C, P, SC, B, AuthorityId>,
) -> Result<polkadot_rpc::RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + AuxStore
        + HeaderMetadata<Block, Error = BlockChainError>
        + Send
        + Sync
        + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: mmr_rpc::MmrRuntimeApi<Block, <Block as sp_runtime::traits::Block>::Hash, BlockNumber>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: BabeApi<Block>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + Sync + Send + 'static,
    SC: SelectChain<Block> + 'static,
    B: sc_client_api::Backend<Block> + Send + Sync + 'static,
    B::State: sc_client_api::StateBackend<sp_runtime::traits::HashingFor<Block>>,
    AuthorityId: AuthorityIdBound,
    <AuthorityId as RuntimeAppPublic>::Signature: Send + Sync,
{
    use mmr_rpc::{Mmr, MmrApiServer};
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use sc_consensus_babe_rpc::{Babe, BabeApiServer};
    use sc_consensus_beefy_rpc::{Beefy, BeefyApiServer};
    use sc_consensus_grandpa_rpc::{Grandpa, GrandpaApiServer};
    use sc_rpc_spec_v2::chain_spec::{ChainSpec, ChainSpecApiServer};
    use sc_sync_state_rpc::{SyncState, SyncStateApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};
    use substrate_state_trie_migration_rpc::{StateMigration, StateMigrationApiServer};

    let mut io = RpcModule::new(());
    let polkadot_rpc::BabeDeps { babe_worker_handle, keystore } = babe;
    let polkadot_rpc::GrandpaDeps {
        shared_voter_state,
        shared_authority_set,
        justification_stream,
        subscription_executor,
        finality_provider,
    } = grandpa;

    let chain_name = chain_spec.name().to_string();
    let genesis_hash = client.hash(0).ok().flatten().expect("Genesis block exists; qed");
    let properties = chain_spec.properties();

    io.merge(ChainSpec::new(chain_name, genesis_hash, properties).into_rpc())?;
    io.merge(StateMigration::new(client.clone(), backend.clone(), deny_unsafe).into_rpc())?;
    io.merge(System::new(client.clone(), pool.clone(), deny_unsafe).into_rpc())?;
    io.merge(TransactionPayment::new(client.clone()).into_rpc())?;
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
        Babe::new(client.clone(), babe_worker_handle.clone(), keystore, select_chain, deny_unsafe)
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
    io.merge(
        SyncState::new(chain_spec, client, shared_authority_set, babe_worker_handle)?.into_rpc(),
    )?;

    io.merge(
        Beefy::<Block, AuthorityId>::new(
            beefy.beefy_finality_proof_stream,
            beefy.beefy_best_block_stream,
            beefy.subscription_executor,
        )?
        .into_rpc(),
    )?;

    Ok(io)
}
