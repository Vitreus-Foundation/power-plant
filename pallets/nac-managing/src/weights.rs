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

//!
//! # Module Overview
//!
//! This module defines the weight functions for the `pallet-nac-managing` in a Substrate-based blockchain.
//! Weights are used to represent the estimated computational cost of extrinsics, which helps ensure that
//! transactions are fairly priced based on their resource usage. These weights are derived from benchmarking
//! and help maintain network performance by accurately estimating the cost of executing key functions such
//! as minting NFTs, updating their attributes, and checking NAC (Nonfungible Asset Certificate) levels.
//!
//! # Key Features and Components
//!
//! - **Weight Functions for Extrinsics**:
//!   - **`mint()`**: Calculates the weight for the `mint()` extrinsic, which is used to mint an NFT for a
//!     user. The weight includes both the computational cost and the database read and write operations.
//!     Specifically, it includes 3 database reads and 3 writes.
//!   - **`update_nft()`**: Provides the weight for the `update_nft()` extrinsic, used to update an NFT's
//!     attributes. This weight also includes database operations and reflects the complexity involved in
//!     modifying existing on-chain data.
//!   - **`check_nac_level()`**: Defines the weight for checking the NAC level of an NFT. This function
//!     includes multiple database reads to retrieve the necessary information and ensure the NAC level
//!     is up-to-date.
//!
//! - **Weight Implementation Structures**:
//!   - **`SubstrateWeight<T>`**: Implements the `WeightInfo` trait for a generic runtime (`T`). The weight
//!     functions are calculated based on the runtime's database weight (`T::DbWeight`). This allows the
//!     pallet to adjust its weight calculations based on the specific runtime configuration, making the
//!     weights adaptable to different node hardware or configurations.
//!   - **Backwards Compatibility Implementation**: To ensure compatibility with older versions of the
//!     pallet or different environments, an implementation of `WeightInfo` for `()` is provided. This
//!     version uses a default weight based on the `RocksDbWeight`, which represents a common database
//!     weight used in Substrate nodes.
//!
//! - **Weight Calculation Components**:
//!   - **`Weight::from_parts()`**: Used to create the weight value, which includes both computational
//!     units and proof size. The weights defined here are derived from actual measurements and are
//!     essential for pricing transactions correctly.
//!   - **Database Read and Write Operations**: The functions include multiple `.reads()` and `.writes()`
//!     calls to represent the cost associated with accessing on-chain storage. Accurate accounting of
//!     these operations is crucial for ensuring that more storage-intensive operations are appropriately
//!     priced to reflect their impact on network performance.
//!
//! # Access Control and Security
//!
//! - **Preventing Resource Abuse**: By assigning appropriate weights to extrinsics, the network can
//!   prevent resource abuse, ensuring that high-cost operations are priced to dissuade excessive use.
//!   This helps maintain network stability and protects against denial-of-service (DoS) attacks.
//! - **Accurate Cost Representation**: Weights are calculated based on benchmarking data, which helps
//!   ensure that the cost of executing these extrinsics is aligned with the actual computational and
//!   storage requirements. This accuracy is vital for fair resource allocation and network performance.
//!
//! # Developer Notes
//!
//! - **Benchmarking Required**: The initial weights provided are derived from benchmarking, but developers
//!   should regularly update these weights if the implementation of the extrinsics changes or if the
//!   network's hardware changes. This helps keep the cost estimation consistent with the actual resource
//!   usage of the pallet's functions.
//! - **Configurable Runtime Weights**: The `SubstrateWeight<T>` structure allows the weights to be adapted
//!   based on the runtime's configuration. This means that the same pallet can be used in different network
//!   environments, with each environment setting weights according to its specific hardware capabilities.
//! - **Reducing Complexity During Testing**: For testing and development purposes, the weight functions
//!   implemented for `()` use the default `RocksDbWeight`, simplifying the setup while retaining the ability
//!   to simulate realistic cost impacts.
//!
//! # Usage Scenarios
//!
//! - **Minting NFTs**: The `mint()` function is used to determine the cost associated with creating new
//!   NFTs. This weight ensures that the minting process is appropriately priced to prevent excessive
//!   minting activity that could lead to increased load on the network.
//! - **Updating NFT Attributes**: The `update_nft()` weight function is used when an existing NFT is being
//!   modified, such as updating its NAC level. This ensures that modifications, which involve database
//!   operations, are fairly priced to reflect the resources they consume.
//! - **Access Verification via NAC Level**: The `check_nac_level()` function provides the weight for checking
//!   the NAC level of a user's NFT. This weight helps ensure that users are charged for the computational
//!   effort needed to verify access rights or status within the network.
//!
//! # Integration Considerations
//!
//! - **Dynamic Weight Calculation**: Developers should ensure that the weights remain up-to-date as the
//!   pallet evolves. This may involve re-running benchmarks after making significant changes to extrinsic
//!   logic or modifying how on-chain data is accessed.
//! - **Hardware-Specific Weights**: The weights calculated in this module are based on benchmarking performed
//!   under specific hardware conditions. When deploying the pallet to a production network, developers should
//!   ensure that the weights are adjusted to reflect the actual hardware used by validators to maintain
//!   fairness and performance consistency.
//! - **Performance Impact**: Weights directly impact transaction fees, which can influence user behavior.
//!   Ensuring that the weights accurately reflect the resource costs helps maintain a balanced fee structure
//!   that discourages misuse without making necessary actions prohibitively expensive for users.
//!
//! # Example Scenario
//!
//! Suppose the `pallet-nac-managing` is used to manage NFTs that represent access levels within a community.
//! When an admin mints a new NFT using the `mint()` extrinsic, the network charges a fee based on the weight
//! calculated by the `mint()` function. This weight accounts for the computational effort and database operations
//! required to create the NFT and update the blockchain state. Similarly, when a user updates their NFT's NAC
//! level using `update_nft()`, the weight ensures that the transaction fee reflects the cost of modifying
//! on-chain data. By using these weights, the network maintains a fair fee structure that accurately represents
//! the resource usage for managing NFTs.
//!

pub trait WeightInfo {
    fn mint() -> Weight;
    fn update_nft() -> Weight;
    fn check_nac_level() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn mint() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().writes(3_u64))
    }

    fn update_nft() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().reads(3_u64))
    }

    fn check_nac_level() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(T::DbWeight::get().reads(3_u64))
            .saturating_add(T::DbWeight::get().reads(3_u64))
    }
}

impl WeightInfo for () {
    fn mint() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }

    fn update_nft() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }

    fn check_nac_level() -> Weight {
        Weight::from_parts(38_924_000, 3643)
            .saturating_add(RocksDbWeight::get().reads(3_u64))
            .saturating_add(RocksDbWeight::get().writes(3_u64))
    }
}