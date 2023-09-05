#![cfg_attr(not(feature = "std"), no_std)]

sp_api::decl_runtime_apis! {
    pub trait NonceApi<AccountId, Nonce> where
    AccountId: parity_scale_codec::Codec,
    Nonce: parity_scale_codec::Codec
    {
        fn get_nonce_by_account_id(account_id: AccountId) -> Nonce;
        fn set_nonce_value(account_id: AccountId, nonce: Nonce) -> bool;
        fn increment(account_id: AccountId) -> bool;
    }
}
