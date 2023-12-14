extern crate alloc;
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
};

pub struct Node {
    name: String,
}

impl Node {
    pub fn new(name: String) -> Self {
        Self { name }
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
        Ok(self.name.clone())
    }
}
