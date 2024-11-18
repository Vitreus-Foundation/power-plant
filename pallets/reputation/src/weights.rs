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

#![allow(missing_docs)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![cfg_attr(rustfmt, rustfmt_skip)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight, RuntimeDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_template.
pub trait WeightInfo {
    fn force_set_points() -> Weight;
    fn increase_points() -> Weight;
    fn slash() -> Weight;
    fn update_points() -> Weight;
    fn force_reset_points() -> Weight;
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

    fn force_reset_points() -> Weight {
        RuntimeDbWeight::default().reads_writes(500, 500)
    }
}
