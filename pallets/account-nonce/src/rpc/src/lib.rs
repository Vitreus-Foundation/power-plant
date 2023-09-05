use parity_scale_codec::Codec;
use std::fmt::Display;
pub use pallet_account_nonce_runtime_api::NonceApi as NonceRuntimeApi;
use jsonrpsee::{
    core::{Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc(client, server)]
pub trait NonceApi<BlockHash, AccountId, Nonce> {
    #[method(name = "nonce_getValue")]
    fn get_nonce_by_account_id(&self, account_id: AccountId) -> RpcResult<Nonce>;

    #[method(name = "nonce_setValue")]
    fn set_nonce_value(&self, account_id: AccountId, nonce: Nonce) -> RpcResult<bool>;

    #[method(name = "nonce_increment")]
    fn increment(&self, account_id: AccountId) -> RpcResult<bool>;

}

pub struct NoncePallet<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> NoncePallet<C, B> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

pub enum Error {
    DecodeError,
    RuntimeError,
}

impl From<Error> for i32 {
    fn from(e: Error) -> i32 {
        match e {
            Error::RuntimeError => 1,
            Error::DecodeError => 2,
        }
    }
}

impl<C, B, AccountId, Nonce> NonceApiServer<<B as BlockT>::Hash, AccountId, Nonce> for NoncePallet<C, B>
    where
        B: BlockT,
        C: Send + Sync + 'static + ProvideRuntimeApi<B> + HeaderBackend<B>,
        C::Api: NonceRuntimeApi<B, AccountId, Nonce>,
        AccountId: Clone + Display + Codec + Send + 'static,
        Nonce: Clone + Display + Codec + Send + 'static
{
    fn get_nonce_by_account_id(&self, account_id: AccountId) -> RpcResult<Nonce> {
        let api = self.client.runtime_api();
        let at = self.client.info().best_hash;

        api.get_nonce_by_account_id(at, account_id)
            .map_err(runtime_error_into_rpc_err)
    }

    fn set_nonce_value(&self, account_id: AccountId, nonce: Nonce) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        let at = self.client.info().best_hash;

        api.set_nonce_value(at, account_id, nonce)
            .map_err(runtime_error_into_rpc_err)
    }

    fn increment(&self, account_id: AccountId) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        let at = self.client.info().best_hash;

        api.increment(at, account_id)
            .map_err(runtime_error_into_rpc_err)
    }
}

/// Converts a runtime trap into an RPC error.
fn runtime_error_into_rpc_err(err: impl std::fmt::Debug) -> JsonRpseeError {
    CallError::Custom(ErrorObject::owned(
        Error::RuntimeError.into(),
        "Runtime error",
        Some(format!("{:?}", err)),
    ))
        .into()
}