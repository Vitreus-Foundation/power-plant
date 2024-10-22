#![cfg_attr(not(feature = "std"), no_std)]

use ethereum::AccessListItem;
use ethereum_types::{H160, U256};
use parity_scale_codec::{Codec, Decode, Encode};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::prelude::*;

/// Introduced for compatibility with eth_estimateGas RPC schema.
/// Similar to fc_rpc_core::types::Bytes, which does not
/// implement necessary traits
#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct Bytes(Vec<u8>);

impl Bytes {
    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

/// Introduced for compatibility with eth_estimateGas RPC schema.
/// Similar to fc_rpc_core::types::CallRequest, which does not
/// implement necessary traits
#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct CallRequest {
    pub from: Option<H160>,
    pub to: Option<H160>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub gas: Option<U256>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
    pub nonce: Option<U256>,
    pub access_list: Option<Vec<AccessListItem>>,
    pub transaction_type: Option<U256>,
}

#[derive(Copy, Clone, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct FeeDetails<Balance> {
    pub vtrs: Balance,
    pub vnrg: Balance,
}

sp_api::decl_runtime_apis! {
    pub trait EnergyFeeApi<AccountId, Balance, Call>
    where
        AccountId: Codec,
        Balance: Codec,
        Call: Codec,
    {
        fn estimate_gas(request: CallRequest) -> U256;

        fn estimate_call_fee(account: AccountId, call: Call) -> Option<FeeDetails<Balance>>;

        fn vtrs_to_vnrg_swap_rate() -> Option<u128>;
    }
}
