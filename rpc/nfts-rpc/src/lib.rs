use jsonrpsee::{
    core::RpcResult,
    proc_macros::rpc,
    types::{ErrorCode, ErrorObject},
};
use parity_scale_codec::{Decode, Encode};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

// Runtime API imports.
pub use nfts_runtime_api::NftsAuxApi as NftsRuntimeApi;

#[rpc(server, client)]
pub trait NftsApi<BlockHash, AccountId, CollectionId, ItemId> {
    #[method(name = "nfts_owned")]
    fn owned(
        &self,
        account: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<(CollectionId, ItemId)>>;

    #[method(name = "nfts_level")]
    fn level(
        &self,
        account: AccountId,
        collection: CollectionId,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<Vec<u8>>>;
}

pub struct Nfts<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> Nfts<C, B> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, CollectionId, ItemId>
    NftsApiServer<<Block as BlockT>::Hash, AccountId, CollectionId, ItemId> for Nfts<C, Block>
where
    Block: BlockT,
    AccountId: Encode,
    CollectionId: Encode + Decode,
    ItemId: Encode + Decode,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: NftsRuntimeApi<Block, AccountId, CollectionId, ItemId>,
{
    fn owned(
        &self,
        account: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Vec<(CollectionId, ItemId)>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        api.owned(at, account).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query owned.",
                Some(e.to_string()),
            )
        })
    }

    fn level(
        &self,
        account: AccountId,
        collection: CollectionId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Option<Vec<u8>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(self.client.info().best_hash);

        api.level(at, account, collection).map_err(|e| {
            ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Unable to query level.",
                Some(e.to_string()),
            )
        })
    }
}
