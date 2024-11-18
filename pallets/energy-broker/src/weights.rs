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

// Executed Command:
// ./target/production/substrate
// benchmark
// pallet
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=pallet_asset_conversion
// --no-storage-info
// --no-median-slopes
// --no-min-squares
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output=./frame/asset-conversion/src/weights.rs
// --header=./HEADER-APACHE2
// --template=./.maintain/frame-weight-template.hbs

//! # Asset Conversion Weight Configuration
//!
//! Auto-generated weights for the asset conversion pallet's extrinsics, based on benchmarking.
//!
//! ## Core Operations and Weights
//!
//! ### Pool Management
//! - Create Pool: ~132.5M ps, 8 reads, 8 writes
//! - Add Liquidity: ~157.9M ps, 8 reads, 7 writes
//! - Remove Liquidity: ~143.6M ps, 7 reads, 6 writes
//!
//! ### Trading Operations
//! - Swap Exact Tokens: ~221.6M ps, 10 reads, 10 writes
//! - Swap For Exact Tokens: ~217.3M ps, 10 reads, 10 writes
//!
//! ## Benchmark Details
//! - Hardware: Intel Xeon CPU @ 2.60GHz
//! - Steps: 50
//! - Repeat: 20
//! - Execution: Wasm Compiled
//! - DB Cache: 1024MB
//! - Max Map Size: 1,000,000
//!
//! All weights include both computational time (ps) and storage proof sizes.
//! Storage operations tracked include:
//! - Pool management
//! - Asset balances
//! - Account data
//! - System parameters
//!
//! Weights are critical for proper fee calculation and resource usage.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_asset_conversion.
pub trait WeightInfo {
	fn create_pool() -> Weight;
	fn add_liquidity() -> Weight;
	fn remove_liquidity() -> Weight;
	fn swap_exact_tokens_for_tokens() -> Weight;
	fn swap_tokens_for_exact_tokens() -> Weight;
}

/// Weights for pallet_asset_conversion using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: AssetConversion Pools (r:1 w:1)
	/// Proof: AssetConversion Pools (max_values: None, max_size: Some(30), added: 2505, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:1 w:1)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:1 w:1)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: AssetConversion NextPoolAssetId (r:1 w:1)
	/// Proof: AssetConversion NextPoolAssetId (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: PoolAssets Asset (r:1 w:1)
	/// Proof: PoolAssets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: PoolAssets Account (r:1 w:1)
	/// Proof: PoolAssets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn create_pool() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `729`
		//  Estimated: `6196`
		// Minimum execution time: 129_741_000 picoseconds.
		Weight::from_parts(132_516_000, 6196)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(8_u64))
	}
	/// Storage: AssetConversion Pools (r:1 w:0)
	/// Proof: AssetConversion Pools (max_values: None, max_size: Some(30), added: 2505, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:1 w:1)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:2 w:2)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: PoolAssets Asset (r:1 w:1)
	/// Proof: PoolAssets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: PoolAssets Account (r:2 w:2)
	/// Proof: PoolAssets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn add_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1382`
		//  Estimated: `6208`
		// Minimum execution time: 154_821_000 picoseconds.
		Weight::from_parts(157_855_000, 6208)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(7_u64))
	}
	/// Storage: AssetConversion Pools (r:1 w:0)
	/// Proof: AssetConversion Pools (max_values: None, max_size: Some(30), added: 2505, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:1 w:1)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:2 w:2)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: PoolAssets Asset (r:1 w:1)
	/// Proof: PoolAssets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: PoolAssets Account (r:1 w:1)
	/// Proof: PoolAssets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn remove_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1371`
		//  Estimated: `6208`
		// Minimum execution time: 139_490_000 picoseconds.
		Weight::from_parts(143_626_000, 6208)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(6_u64))
	}
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:3 w:3)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:6 w:6)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn swap_exact_tokens_for_tokens() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1732`
		//  Estimated: `16644`
		// Minimum execution time: 212_868_000 picoseconds.
		Weight::from_parts(221_638_000, 16644)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(10_u64))
	}
	/// Storage: Assets Asset (r:3 w:3)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:6 w:6)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn swap_tokens_for_exact_tokens() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1732`
		//  Estimated: `16644`
		// Minimum execution time: 211_746_000 picoseconds.
		Weight::from_parts(217_322_000, 16644)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(10_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: AssetConversion Pools (r:1 w:1)
	/// Proof: AssetConversion Pools (max_values: None, max_size: Some(30), added: 2505, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:1 w:1)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:1 w:1)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: AssetConversion NextPoolAssetId (r:1 w:1)
	/// Proof: AssetConversion NextPoolAssetId (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	/// Storage: PoolAssets Asset (r:1 w:1)
	/// Proof: PoolAssets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: PoolAssets Account (r:1 w:1)
	/// Proof: PoolAssets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn create_pool() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `729`
		//  Estimated: `6196`
		// Minimum execution time: 129_741_000 picoseconds.
		Weight::from_parts(132_516_000, 6196)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
	}
	/// Storage: AssetConversion Pools (r:1 w:0)
	/// Proof: AssetConversion Pools (max_values: None, max_size: Some(30), added: 2505, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:1 w:1)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:2 w:2)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: PoolAssets Asset (r:1 w:1)
	/// Proof: PoolAssets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: PoolAssets Account (r:2 w:2)
	/// Proof: PoolAssets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn add_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1382`
		//  Estimated: `6208`
		// Minimum execution time: 154_821_000 picoseconds.
		Weight::from_parts(157_855_000, 6208)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
	/// Storage: AssetConversion Pools (r:1 w:0)
	/// Proof: AssetConversion Pools (max_values: None, max_size: Some(30), added: 2505, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:1 w:1)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:2 w:2)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: PoolAssets Asset (r:1 w:1)
	/// Proof: PoolAssets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: PoolAssets Account (r:1 w:1)
	/// Proof: PoolAssets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn remove_liquidity() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1371`
		//  Estimated: `6208`
		// Minimum execution time: 139_490_000 picoseconds.
		Weight::from_parts(143_626_000, 6208)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(6_u64))
	}
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Assets Asset (r:3 w:3)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:6 w:6)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	fn swap_exact_tokens_for_tokens() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1732`
		//  Estimated: `16644`
		// Minimum execution time: 212_868_000 picoseconds.
		Weight::from_parts(221_638_000, 16644)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(10_u64))
	}
	/// Storage: Assets Asset (r:3 w:3)
	/// Proof: Assets Asset (max_values: None, max_size: Some(210), added: 2685, mode: MaxEncodedLen)
	/// Storage: Assets Account (r:6 w:6)
	/// Proof: Assets Account (max_values: None, max_size: Some(134), added: 2609, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn swap_tokens_for_exact_tokens() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1732`
		//  Estimated: `16644`
		// Minimum execution time: 211_746_000 picoseconds.
		Weight::from_parts(217_322_000, 16644)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(10_u64))
	}
}
