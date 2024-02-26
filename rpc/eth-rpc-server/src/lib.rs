extern crate alloc;
use alloc::sync::Arc;
use ethereum_types::{H160, H256, H64, U256, U64};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
};
use std::collections::BTreeMap;
use std::marker::PhantomData;

use fc_rpc::{frontier_backend_client, internal_err, Eth, EthConfig};
use fc_rpc_core::types::*;
use fp_rpc::{ConvertTransaction, ConvertTransactionRuntimeApi, EthereumRuntimeRPCApi};
use sc_client_api::backend::{Backend, StorageProvider};
use sc_transaction_pool::{ChainApi, Pool};
use sc_transaction_pool_api::{InPoolTransaction, TransactionPool};
use sp_api::{ApiRef, CallApiAt, Core, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, Header};
use vitreus_utility_runtime_api::UtilityApi;

pub struct EthExtension<B: BlockT, C, P, BE, A: ChainApi> {
    client: Arc<C>,
    _pool: Arc<P>,
    graph: Arc<Pool<A>>,
    backend: Arc<dyn fc_db::BackendReader<B> + Send + Sync>,
    _marker: PhantomData<(B, BE)>,
}

impl<B, C, P, BE, A: ChainApi> EthExtension<B, C, P, BE, A>
where
    B: BlockT,
    C: ProvideRuntimeApi<B>,
    C::Api: BlockBuilderApi<B> + EthereumRuntimeRPCApi<B> + UtilityApi<B>,
    C: HeaderBackend<B> + StorageProvider<B, BE> + 'static,
    BE: Backend<B> + 'static,
    P: TransactionPool<Block = B> + 'static,
    A: ChainApi<Block = B> + 'static,
{
    pub fn new(
        client: Arc<C>,
        pool: Arc<P>,
        graph: Arc<Pool<A>>,
        backend: Arc<dyn fc_db::BackendReader<B> + Send + Sync>,
    ) -> Self {
        Self { client, _pool: pool, graph, backend, _marker: Default::default() }
    }

    fn pending_runtime_api<'a>(client: &'a C, graph: &'a Pool<A>) -> RpcResult<ApiRef<'a, C::Api>> {
        // In case of Pending, we need an overlayed state to query over.
        let api = client.runtime_api();
        let best_hash = client.info().best_hash;
        // Get all transactions in the ready queue.
        let xts: Vec<<B as BlockT>::Extrinsic> = graph
            .validated_pool()
            .ready()
            .map(|in_pool_tx| in_pool_tx.data().clone())
            .collect::<Vec<<B as BlockT>::Extrinsic>>();
        // Manually initialize the overlay.
        if let Ok(Some(header)) = client.header(best_hash) {
            let parent_hash = *header.parent_hash();
            api.initialize_block(parent_hash, &header)
                .map_err(|e| internal_err(format!("Runtime api access error: {:?}", e)))?;
            // Apply the ready queue to the best block's state.
            for xt in xts {
                let _ = api.apply_extrinsic(best_hash, xt);
            }
            Ok(api)
        } else {
            Err(internal_err(format!("Cannot get header for block {:?}", best_hash)))
        }
    }

    pub async fn balance(&self, address: H160, number: Option<BlockNumber>) -> RpcResult<U256> {
        let number = number.unwrap_or(BlockNumber::Latest);
        if number == BlockNumber::Pending {
            let api = Self::pending_runtime_api(self.client.as_ref(), self.graph.as_ref())?;
            Ok(api
                .balance(self.client.info().best_hash, address)
                .map_err(|err| internal_err(format!("fetch runtime chain id failed: {:?}", err)))?)
        } else if let Ok(Some(id)) = frontier_backend_client::native_block_id::<B, C>(
            self.client.as_ref(),
            self.backend.as_ref(),
            Some(number),
        )
        .await
        {
            let substrate_hash = self
                .client
                .expect_block_hash_from_id(&id)
                .map_err(|_| internal_err(format!("Expect block number from id: {}", id)))?;

            Ok(self
                .client
                .runtime_api()
                .balance(substrate_hash, address)
                .map_err(|err| internal_err(format!("fetch runtime chain id failed: {:?}", err)))?)
        } else {
            Ok(U256::zero())
        }
    }
}

#[rpc(server)]
#[async_trait]
pub trait EthExtensionApi {
    /// Returns balance of the given account.
    #[method(name = "eth_getBalance")]
    async fn balance(&self, address: H160, number: Option<BlockNumber>) -> RpcResult<U256>;
}

#[async_trait]
impl<B, C, P, BE, A> EthExtensionApiServer for EthExtension<B, C, P, BE, A>
where
    B: BlockT,
    C: CallApiAt<B> + ProvideRuntimeApi<B>,
    C::Api: BlockBuilderApi<B>
        + ConvertTransactionRuntimeApi<B>
        + EthereumRuntimeRPCApi<B>
        + UtilityApi<B>,
    C: HeaderBackend<B> + StorageProvider<B, BE> + 'static,
    BE: Backend<B> + 'static,
    P: TransactionPool<Block = B> + 'static,
    A: ChainApi<Block = B> + 'static,
{
    async fn balance(&self, address: H160, number: Option<BlockNumber>) -> RpcResult<U256> {
        self.balance(address, number).await
    }
}

/// Eth rpc interface.
#[rpc(server)]
#[async_trait]
pub trait EthApi {
    // ########################################################################
    // Client
    // ########################################################################

    /// Returns protocol version encoded as a string (quotes are necessary).
    #[method(name = "eth_protocolVersion")]
    fn protocol_version(&self) -> RpcResult<u64>;

    /// Returns an object with data about the sync status or false. (wtf?)
    #[method(name = "eth_syncing")]
    fn syncing(&self) -> RpcResult<SyncStatus>;

    /// Returns block author.
    #[method(name = "eth_coinbase")]
    fn author(&self) -> RpcResult<H160>;

    /// Returns accounts list.
    #[method(name = "eth_accounts")]
    fn accounts(&self) -> RpcResult<Vec<H160>>;

    /// Returns highest block number.
    #[method(name = "eth_blockNumber")]
    fn block_number(&self) -> RpcResult<U256>;

    /// Returns the chain ID used for transaction signing at the
    /// current best block. None is returned if not
    /// available.
    #[method(name = "eth_chainId")]
    fn chain_id(&self) -> RpcResult<Option<U64>>;

    // ########################################################################
    // Block
    // ########################################################################

    /// Returns block with given hash.
    #[method(name = "eth_getBlockByHash")]
    async fn block_by_hash(&self, hash: H256, full: bool) -> RpcResult<Option<RichBlock>>;

    /// Returns block with given number.
    #[method(name = "eth_getBlockByNumber")]
    async fn block_by_number(
        &self,
        number: BlockNumber,
        full: bool,
    ) -> RpcResult<Option<RichBlock>>;

    /// Returns the number of transactions in a block with given hash.
    #[method(name = "eth_getBlockTransactionCountByHash")]
    async fn block_transaction_count_by_hash(&self, hash: H256) -> RpcResult<Option<U256>>;

    /// Returns the number of transactions in a block with given block number.
    #[method(name = "eth_getBlockTransactionCountByNumber")]
    async fn block_transaction_count_by_number(
        &self,
        number: BlockNumber,
    ) -> RpcResult<Option<U256>>;

    /// Returns the number of uncles in a block with given hash.
    #[method(name = "eth_getUncleCountByBlockHash")]
    fn block_uncles_count_by_hash(&self, hash: H256) -> RpcResult<U256>;

    /// Returns the number of uncles in a block with given block number.
    #[method(name = "eth_getUncleCountByBlockNumber")]
    fn block_uncles_count_by_number(&self, number: BlockNumber) -> RpcResult<U256>;

    /// Returns an uncles at given block and index.
    #[method(name = "eth_getUncleByBlockHashAndIndex")]
    fn uncle_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<RichBlock>>;

    /// Returns an uncles at given block and index.
    #[method(name = "eth_getUncleByBlockNumberAndIndex")]
    fn uncle_by_block_number_and_index(
        &self,
        number: BlockNumber,
        index: Index,
    ) -> RpcResult<Option<RichBlock>>;

    // ########################################################################
    // Transaction
    // ########################################################################

    /// Get transaction by its hash.
    #[method(name = "eth_getTransactionByHash")]
    async fn transaction_by_hash(&self, hash: H256) -> RpcResult<Option<Transaction>>;

    /// Returns transaction at given block hash and index.
    #[method(name = "eth_getTransactionByBlockHashAndIndex")]
    async fn transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<Transaction>>;

    /// Returns transaction by given block number and index.
    #[method(name = "eth_getTransactionByBlockNumberAndIndex")]
    async fn transaction_by_block_number_and_index(
        &self,
        number: BlockNumber,
        index: Index,
    ) -> RpcResult<Option<Transaction>>;

    /// Returns transaction receipt by transaction hash.
    #[method(name = "eth_getTransactionReceipt")]
    async fn transaction_receipt(&self, hash: H256) -> RpcResult<Option<Receipt>>;

    // ########################################################################
    // State
    // ########################################################################

    /// Returns content of the storage at given address.
    #[method(name = "eth_getStorageAt")]
    async fn storage_at(
        &self,
        address: H160,
        index: U256,
        number: Option<BlockNumber>,
    ) -> RpcResult<H256>;

    /// Returns the number of transactions sent from given address at given time (block number).
    #[method(name = "eth_getTransactionCount")]
    async fn transaction_count(
        &self,
        address: H160,
        number: Option<BlockNumber>,
    ) -> RpcResult<U256>;

    /// Returns the code at given address at given time (block number).
    #[method(name = "eth_getCode")]
    async fn code_at(&self, address: H160, number: Option<BlockNumber>) -> RpcResult<Bytes>;

    // ########################################################################
    // Execute
    // ########################################################################

    /// Call contract, returning the output data.
    #[method(name = "eth_call")]
    async fn call(
        &self,
        request: CallRequest,
        number: Option<BlockNumber>,
        state_overrides: Option<BTreeMap<H160, CallStateOverride>>,
    ) -> RpcResult<Bytes>;

    /// Estimate gas needed for execution of given contract.
    #[method(name = "eth_estimateGas")]
    async fn estimate_gas(
        &self,
        request: CallRequest,
        number: Option<BlockNumber>,
    ) -> RpcResult<U256>;

    // ########################################################################
    // Fee
    // ########################################################################

    /// Returns current gas_price.
    #[method(name = "eth_gasPrice")]
    fn gas_price(&self) -> RpcResult<U256>;

    /// Introduced in EIP-1159 for getting information on the appropriate priority fee to use.
    #[method(name = "eth_feeHistory")]
    async fn fee_history(
        &self,
        block_count: U256,
        newest_block: BlockNumber,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory>;

    /// Introduced in EIP-1159, a Geth-specific and simplified priority fee oracle.
    /// Leverages the already existing fee history cache.
    #[method(name = "eth_maxPriorityFeePerGas")]
    fn max_priority_fee_per_gas(&self) -> RpcResult<U256>;

    // ########################################################################
    // Mining
    // ########################################################################

    /// Returns true if client is actively mining new blocks.
    #[method(name = "eth_mining")]
    fn is_mining(&self) -> RpcResult<bool>;

    /// Returns the number of hashes per second that the node is mining with.
    #[method(name = "eth_hashrate")]
    fn hashrate(&self) -> RpcResult<U256>;

    /// Returns the hash of the current block, the seedHash, and the boundary condition to be met.
    #[method(name = "eth_getWork")]
    fn work(&self) -> RpcResult<Work>;

    /// Used for submitting mining hashrate.
    #[method(name = "eth_submitHashrate")]
    fn submit_hashrate(&self, hashrate: U256, id: H256) -> RpcResult<bool>;

    /// Used for submitting a proof-of-work solution.
    #[method(name = "eth_submitWork")]
    fn submit_work(&self, nonce: H64, pow_hash: H256, mix_digest: H256) -> RpcResult<bool>;

    // ########################################################################
    // Submit
    // ########################################################################

    /// Sends transaction; will block waiting for signer to return the
    /// transaction hash.
    #[method(name = "eth_sendTransaction")]
    async fn send_transaction(&self, request: TransactionRequest) -> RpcResult<H256>;

    /// Sends signed transaction, returning its hash.
    #[method(name = "eth_sendRawTransaction")]
    async fn send_raw_transaction(&self, bytes: Bytes) -> RpcResult<H256>;
}

#[async_trait]
impl<B, C, P, CT, BE, A, EC> EthApiServer for Eth<B, C, P, CT, BE, A, EC>
where
    B: BlockT,
    C: CallApiAt<B> + ProvideRuntimeApi<B>,
    C::Api: BlockBuilderApi<B> + ConvertTransactionRuntimeApi<B> + EthereumRuntimeRPCApi<B>,
    C: HeaderBackend<B> + StorageProvider<B, BE> + 'static,
    BE: Backend<B> + 'static,
    P: TransactionPool<Block = B> + 'static,
    CT: ConvertTransaction<<B as BlockT>::Extrinsic> + Send + Sync + 'static,
    A: ChainApi<Block = B> + 'static,
    EC: EthConfig<B, C>,
{
    // ########################################################################
    // Client
    // ########################################################################

    fn protocol_version(&self) -> RpcResult<u64> {
        self.protocol_version()
    }

    fn syncing(&self) -> RpcResult<SyncStatus> {
        self.syncing()
    }

    fn author(&self) -> RpcResult<H160> {
        self.author()
    }

    fn accounts(&self) -> RpcResult<Vec<H160>> {
        self.accounts()
    }

    fn block_number(&self) -> RpcResult<U256> {
        self.block_number()
    }

    fn chain_id(&self) -> RpcResult<Option<U64>> {
        self.chain_id()
    }

    // ########################################################################
    // Block
    // ########################################################################

    async fn block_by_hash(&self, hash: H256, full: bool) -> RpcResult<Option<RichBlock>> {
        self.block_by_hash(hash, full).await
    }

    async fn block_by_number(
        &self,
        number: BlockNumber,
        full: bool,
    ) -> RpcResult<Option<RichBlock>> {
        self.block_by_number(number, full).await
    }

    async fn block_transaction_count_by_hash(&self, hash: H256) -> RpcResult<Option<U256>> {
        self.block_transaction_count_by_hash(hash).await
    }

    async fn block_transaction_count_by_number(
        &self,
        number: BlockNumber,
    ) -> RpcResult<Option<U256>> {
        self.block_transaction_count_by_number(number).await
    }

    fn block_uncles_count_by_hash(&self, hash: H256) -> RpcResult<U256> {
        self.block_uncles_count_by_hash(hash)
    }

    fn block_uncles_count_by_number(&self, number: BlockNumber) -> RpcResult<U256> {
        self.block_uncles_count_by_number(number)
    }

    fn uncle_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        self.uncle_by_block_hash_and_index(hash, index)
    }

    fn uncle_by_block_number_and_index(
        &self,
        number: BlockNumber,
        index: Index,
    ) -> RpcResult<Option<RichBlock>> {
        self.uncle_by_block_number_and_index(number, index)
    }

    // ########################################################################
    // Transaction
    // ########################################################################

    async fn transaction_by_hash(&self, hash: H256) -> RpcResult<Option<Transaction>> {
        self.transaction_by_hash(hash).await
    }

    async fn transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        self.transaction_by_block_hash_and_index(hash, index).await
    }

    async fn transaction_by_block_number_and_index(
        &self,
        number: BlockNumber,
        index: Index,
    ) -> RpcResult<Option<Transaction>> {
        self.transaction_by_block_number_and_index(number, index).await
    }

    async fn transaction_receipt(&self, hash: H256) -> RpcResult<Option<Receipt>> {
        self.transaction_receipt(hash).await
    }

    // ########################################################################
    // State
    // ########################################################################

    async fn storage_at(
        &self,
        address: H160,
        index: U256,
        number: Option<BlockNumber>,
    ) -> RpcResult<H256> {
        self.storage_at(address, index, number).await
    }

    async fn transaction_count(
        &self,
        address: H160,
        number: Option<BlockNumber>,
    ) -> RpcResult<U256> {
        self.transaction_count(address, number).await
    }

    async fn code_at(&self, address: H160, number: Option<BlockNumber>) -> RpcResult<Bytes> {
        self.code_at(address, number).await
    }

    // ########################################################################
    // Execute
    // ########################################################################

    async fn call(
        &self,
        request: CallRequest,
        number: Option<BlockNumber>,
        state_overrides: Option<BTreeMap<H160, CallStateOverride>>,
    ) -> RpcResult<Bytes> {
        self.call(request, number, state_overrides).await
    }

    #[allow(unused_variables)]
    async fn estimate_gas(
        &self,
        request: CallRequest,
        number: Option<BlockNumber>,
    ) -> RpcResult<U256> {
        Ok(U256::zero())
    }

    // ########################################################################
    // Fee
    // ########################################################################

    fn gas_price(&self) -> RpcResult<U256> {
        Ok(U256::one())
    }

    async fn fee_history(
        &self,
        block_count: U256,
        newest_block: BlockNumber,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        self.fee_history(block_count, newest_block, reward_percentiles).await
    }

    fn max_priority_fee_per_gas(&self) -> RpcResult<U256> {
        self.max_priority_fee_per_gas()
    }

    // ########################################################################
    // Mining
    // ########################################################################

    fn is_mining(&self) -> RpcResult<bool> {
        self.is_mining()
    }

    fn hashrate(&self) -> RpcResult<U256> {
        self.hashrate()
    }

    fn work(&self) -> RpcResult<Work> {
        self.work()
    }

    fn submit_hashrate(&self, hashrate: U256, id: H256) -> RpcResult<bool> {
        self.submit_hashrate(hashrate, id)
    }

    fn submit_work(&self, nonce: H64, pow_hash: H256, mix_digest: H256) -> RpcResult<bool> {
        self.submit_work(nonce, pow_hash, mix_digest)
    }

    // ########################################################################
    // Submit
    // ########################################################################

    async fn send_transaction(&self, request: TransactionRequest) -> RpcResult<H256> {
        self.send_transaction(request).await
    }

    async fn send_raw_transaction(&self, bytes: Bytes) -> RpcResult<H256> {
        self.send_raw_transaction(bytes).await
    }
}
