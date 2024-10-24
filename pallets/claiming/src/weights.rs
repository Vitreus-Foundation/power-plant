// This file is part of Substrate.

// Copyright (C) 2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


//! # Claims Pallet Weight Configuration
//!
//! Defines the computational and storage weights for claims pallet operations.
//! These weights are crucial for blockchain resource management and fee calculation.
//!
//! ## Weight Calculations
//! Each operation includes:
//! - Base computational weight (in picoseconds)
//! - Proof size weight
//! - Database read operations
//! - Database write operations
//!
//! ## Operations
//! ### Mint Tokens to Claim
//! - Computational: 15,273,000 ps
//! - Storage: 1 read, 2 writes
//! - Used when adding tokens to claiming pool
//!
//! ### Claim
//! - Computational: 38,924,000 ps
//! - Storage: 3 reads, 3 writes
//! - Used for signature verification and token transfer
//!
//! ### Mint Claim
//! - Computational: 38,924,000 ps
//! - Storage: 3 reads, 3 writes
//! - Used when creating new claims
//!
//! ## Implementations
//! - SubstrateWeight<T>: Production implementation using runtime configuration
//! - Unit implementation: Testing implementation using RocksDB weights
//!
//! These weights were benchmarked on reference hardware and include safety margins.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet-claiming`.
pub trait WeightInfo {
    fn mint_tokens_to_claim() -> Weight;
    fn claim() -> Weight;
    fn mint_claim() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn mint_tokens_to_claim() -> Weight {
        Weight::from_parts(15_273_000, 3643)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(2_u64))
    }

    fn claim() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }

    fn mint_claim() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }
}

impl WeightInfo for () {
    fn mint_tokens_to_claim() -> Weight {
        Weight::from_parts(15_273_000, 3643)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(2_u64))
    }

    fn claim() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }

    fn mint_claim() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
}

