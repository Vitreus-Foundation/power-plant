//! Autogenerated weights for pallet_template
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2023-04-06, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `Alexs-MacBook-Pro-2.local`, CPU: `<UNKNOWN>`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ../../target/release/node-template
// benchmark
// pallet
// --chain
// dev
// --pallet
// pallet_template
// --extrinsic
// *
// --steps=50
// --repeat=20
// --execution=wasm
// --wasm-execution=compiled
// --output
// pallets/template/src/weights.rs
// --template
// ../../.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight, RuntimeDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_template.
pub trait WeightInfo {
    fn force_set_points() -> Weight;
    fn increase_points() -> Weight;
    fn slash() -> Weight;
    fn update_points() -> Weight;
}

impl WeightInfo for () {
    
    fn force_set_points() -> Weight {
        RuntimeDbWeight::default().writes(1)
    }

    fn increase_points() -> Weight {
        RuntimeDbWeight::default().writes(1)
    }

    fn slash() -> Weight {
        RuntimeDbWeight::default().writes(1)
    }

    fn update_points() -> Weight {
        RuntimeDbWeight::default().reads_writes(1, 1)
    }
}
