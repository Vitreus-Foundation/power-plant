extern crate alloc;
use alloc::sync::Arc;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
};
use std::collections::BTreeMap;
use std::marker::PhantomData;

use sc_client_api::backend::{Backend, StorageProvider};
use sc_transaction_pool::{ChainApi, Pool};
use sc_transaction_pool_api::{InPoolTransaction, TransactionPool};
use sp_api::{ApiRef, CallApiAt, Core, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, Header};

pub struct Node;

impl Node {
    pub fn new() -> Self {
        Self
    }
}

#[rpc(server)]
#[async_trait]
pub trait NodeApi {
    /// Returns balance of the given account.
    #[method(name = "node_name")]
    async fn name(&self) -> RpcResult<String>;
}

#[async_trait]
impl NodeApiServer for Node {
    async fn name(&self) -> RpcResult<String> {
        Ok("vitreus-default-node-name".to_owned())
    }
}
