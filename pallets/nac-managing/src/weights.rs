// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet-nac-managing`.
pub trait WeightInfo {
    /// Full native account observation, also an upper bound for light finalizer checks.
    fn balance_observer() -> Weight;
    fn mint() -> Weight;
    fn update_nft() -> Weight;
    fn check_nac_level() -> Weight;
    fn transfer_vipp(max_nfts: u32) -> Weight;
    fn check_vipp_threshold(max_nfts: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn balance_observer() -> Weight {
        observer_weight(T::DbWeight::get())
    }
    fn check_vipp_threshold(max_nfts: u32) -> Weight {
        threshold_weight(T::DbWeight::get(), max_nfts)
    }
    fn transfer_vipp(max_nfts: u32) -> Weight {
        transfer_weight(T::DbWeight::get(), max_nfts)
    }
    fn mint() -> Weight {
        Weight::from_parts(150_000_000, 12_288)
            .saturating_add(T::DbWeight::get().reads_writes(8, 8))
    }

    fn update_nft() -> Weight {
        Weight::from_parts(80_000_000, 8192)
            .saturating_add(T::DbWeight::get().reads_writes(4, 4))
    }

    fn check_nac_level() -> Weight {
        Weight::from_parts(50_000_000, 8192)
            .saturating_add(T::DbWeight::get().reads_writes(3, 1))
    }
}

impl WeightInfo for () {
    fn balance_observer() -> Weight {
        observer_weight(RocksDbWeight::get())
    }
    fn check_vipp_threshold(max_nfts: u32) -> Weight {
        threshold_weight(RocksDbWeight::get(), max_nfts)
    }
    fn transfer_vipp(max_nfts: u32) -> Weight {
        transfer_weight(RocksDbWeight::get(), max_nfts)
    }
    fn mint() -> Weight {
        Weight::from_parts(150_000_000, 12_288)
            .saturating_add(RocksDbWeight::get().reads_writes(8, 8))
    }

    fn update_nft() -> Weight {
        Weight::from_parts(80_000_000, 8192)
            .saturating_add(RocksDbWeight::get().reads_writes(4, 4))
    }

    fn check_nac_level() -> Weight {
        Weight::from_parts(50_000_000, 8192)
            .saturating_add(RocksDbWeight::get().reads_writes(3, 1))
    }
}

fn observer_weight(db: frame_support::weights::RuntimeDbWeight) -> Weight {
    // Include vesting/reserve reads and bounded queue maintenance.
    Weight::from_parts(800_000_000, 40_960).saturating_add(db.reads_writes(21, 7))
}

// Keep the quadratic CPU allowance: burn callbacks rebuild the remaining cache.
fn threshold_weight(db: frame_support::weights::RuntimeDbWeight, max_nfts: u32) -> Weight {
    let n = u64::from(max_nfts);
    Weight::from_parts(200_000_000, 32_768)
        .saturating_add(Weight::from_parts(400_000_000, 8192).saturating_mul(n))
        .saturating_add(Weight::from_parts(2_000_000, 0).saturating_mul(n.saturating_mul(n)))
        .saturating_add(db.reads_writes(
            32_u64.saturating_add(n.saturating_mul(6)),
            12_u64.saturating_add(n.saturating_mul(6)),
        ))
}

fn transfer_weight(db: frame_support::weights::RuntimeDbWeight, max_nfts: u32) -> Weight {
    let n = u64::from(max_nfts);
    // The recipient can be a new VIPP member: allow scanning the bounded 1024
    // member keys for capacity. Daily accrual is charged separately by the timestamp inherent.
    Weight::from_parts(8_000_000_000, 3_145_728)
        .saturating_add(Weight::from_parts(100_000_000, 8192).saturating_mul(n))
        .saturating_add(db.reads_writes(1100_u64.saturating_add(n.saturating_mul(4)), 16))
}
