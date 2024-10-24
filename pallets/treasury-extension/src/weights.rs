
//! Autogenerated weights for `pallet_treasury_extension`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2024-01-09, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `qwertys-MacBook-Pro.local`, CPU: `<UNKNOWN>`
//! EXECUTION: ``, WASM-EXECUTION: `Compiled`, CHAIN: `Some("dev")`, DB CACHE: 1024
//! Autogenerated Weights for Treasury Pallet Extension
//!
//! This file provides autogenerated weights for the Treasury pallet extension using the Substrate Benchmark CLI.
//! These weights are essential for calculating the computational cost of different operations within the Treasury pallet, ensuring that runtime performance remains balanced and resource-efficient.
//!
//! # Features
//! - Generated using Substrate Benchmark CLI version 4.0.0-dev.
//! - Provides weights for critical Treasury functions, detailing the cost associated with each operation.
//! - Includes benchmark details such as the number of steps, repetitions, and worst-case map sizes.
//!
//! # Structure
//! - Contains autogenerated functions that return the weight values for specific Treasury pallet operations.
//! - Each weight function is designed to provide an accurate estimate of the resources required for the associated operation.
//! - Benchmark data such as execution environment and chain type is included for reference.
//!
//! # Usage
//! - Integrate these weights into the Treasury pallet to ensure accurate cost calculation for dispatchable functions.
//! - Weights are used in the runtime to adjust fees and ensure fair resource allocation across operations.
//!
//! # Dependencies
//! - Relies on the Substrate benchmarking framework to generate realistic weights for Treasury operations.
//! - Uses the configuration specified during benchmarking to simulate runtime conditions.
//!
//! # Important Notes
//! - This file is autogenerated, and any manual changes will be overwritten when weights are regenerated.
//! - Always re-run benchmarks after modifying any core logic in the Treasury pallet to maintain accurate weight values.
//! - Benchmarking should be conducted in an environment that closely resembles production to ensure the accuracy of the generated weights.
//! - The weights are highly dependent on the chain configuration, so adjustments may be needed if the runtime or execution settings change.

// Executed Command:
// target/debug/vitreus-power-plant-node
// benchmark
// pallet
// --chain
// dev
// --wasm-execution=compiled
// --pallet
// pallet-treasury-extension
// --extrinsic
// *
// --steps
// 50
// --repeat
// 20
// --output
// pallets/treasury-extension/src/weights.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

pub trait WeightInfo {
    fn spend_funds() -> Weight;
}

/// Weight functions for `pallet_treasury_extension`.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: `System::Account` (r:1 w:0)
	/// Proof: `System::Account` (`max_values`: None, `max_size`: Some(116), added: 2591, mode: `MaxEncodedLen`)
	/// Storage: `Balances::TotalIssuance` (r:1 w:1)
	/// Proof: `Balances::TotalIssuance` (`max_values`: Some(1), `max_size`: Some(16), added: 511, mode: `MaxEncodedLen`)
	/// Storage: `System::Number` (r:1 w:0)
	/// Proof: `System::Number` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `System::ExecutionPhase` (r:1 w:0)
	/// Proof: `System::ExecutionPhase` (`max_values`: Some(1), `max_size`: Some(5), added: 500, mode: `MaxEncodedLen`)
	/// Storage: `System::EventCount` (r:1 w:1)
	/// Proof: `System::EventCount` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `System::Events` (r:1 w:1)
	/// Proof: `System::Events` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	fn spend_funds() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `301`
		//  Estimated: `3581`
		// Minimum execution time: 314_000_000 picoseconds.
		Weight::from_parts(321_000_000, 0)
			.saturating_add(Weight::from_parts(0, 3581))
			.saturating_add(T::DbWeight::get().reads(6))
			.saturating_add(T::DbWeight::get().writes(3))
	}
}

impl WeightInfo for () {
    fn spend_funds() -> Weight {
		Weight::from_parts(321_000_000, 0)
			.saturating_add(Weight::from_parts(0, 3581))
			.saturating_add(RocksDbWeight::get().reads(6))
			.saturating_add(RocksDbWeight::get().writes(3))
    }
}
