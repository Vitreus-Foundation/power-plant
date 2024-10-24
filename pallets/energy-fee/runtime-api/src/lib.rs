//! # Energy Fee Runtime API Definitions
//!
//! Core types and runtime API traits for energy fee calculations.
//!
//! ## Core Types
//!
//! ### CallRequest
//! Ethereum-compatible call request structure containing:
//! - `from`: Optional sender address
//! - `to`: Optional recipient address
//! - `gas_price`, `max_fee_per_gas`: Gas pricing options
//! - `gas`: Optional gas limit
//! - `value`: Optional transfer value
//! - `data`: Optional call data
//! - `access_list`: Optional EIP-2930 access list
//!
//! ### FeeDetails
//! Fee breakdown containing both token types:
//! - `vtrs`: Native token fee amount
//! - `vnrg`: Energy token fee amount
//!
//! ## Runtime API Methods
//!
//! ### Fee Estimation
//! - `estimate_gas`: Calculate gas cost for EVM calls
//! - `estimate_call_fee`: Calculate total fee for runtime calls
//! - `vtrs_to_vnrg_swap_rate`: Get current token exchange rate
//!
//! ## Implementation Notes
//! - No-std compatible
//! - Implements necessary codec traits
//! - Ethereum type compatibility
//! - Serialization support for RPC
//!
//! This API enables fee estimation and exchange rate queries from the runtime.

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
