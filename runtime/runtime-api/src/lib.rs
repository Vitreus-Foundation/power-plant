#![cfg_attr(not(feature = "std"), no_std)]

use sp_core::{H160, U256};

sp_api::decl_runtime_apis! {
    pub trait UtilityApi
    {
        fn balance(who: H160) -> U256;
    }
}
