#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use parity_scale_codec::{Decode, Encode};

sp_api::decl_runtime_apis! {
    pub trait NftsAuxApi<AccountId, CollectionId, ItemId>
    where
        AccountId: Encode,
        CollectionId: Encode + Decode,
        ItemId: Encode + Decode,
    {
        /// Returns the items of all collections owned by `account`.
        fn owned(account: AccountId) -> Vec<(CollectionId, ItemId)>;

        /// Returns the level of the first item of `collection` owned by `account`.
        fn level(account: AccountId, collection: CollectionId) -> Option<Vec<u8>>;
    }
}
