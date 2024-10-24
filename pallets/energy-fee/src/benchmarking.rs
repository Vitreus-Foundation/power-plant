//! Benchmarking Module for the Energy Fee Pallet
//!
//! This module provides benchmarking capabilities to measure computational and storage costs
//! of the Energy Fee pallet's dispatchable functions. These benchmarks are crucial for
//! accurate weight calculations and fee estimation.
//!
//! # Benchmarked Functions
//!
//! The following extrinsics are benchmarked:
//! * `update_burned_energy_threshold` - Measuring cost of updating energy burn limits
//! * `update_block_fullness_threshold` - Measuring cost of modifying block fullness parameters
//! * `update_upper_fee_multiplier` - Measuring cost of adjusting fee multiplier bounds
//!
//! # Running Benchmarks
//!
//! ```bash
//! # Run benchmarks for all functions
//! cargo run --release --features runtime-benchmarks \
//!     benchmark pallet \
//!     --chain dev \
//!     --pallet pallet_energy_fee \
//!     --extrinsic "*" \
//!     --steps 50 \
//!     --repeat 20
//! ```
//!
//! # Implementation Notes
//!
//! - All benchmarks use Root origin for administrative functions
//! - Event emission is verified in each benchmark
//! - Uses mock runtime for test suite implementation
//! - Zero/One values used as conservative benchmarking inputs
//!
//! # Usage in Weight Calculation
//!
//! The benchmark results are used to:
//! 1. Set appropriate weights for extrinsics
//! 2. Calculate transaction fees
//! 3. Determine block resource limits
//!
//! # Security Considerations
//!
//! These benchmarks ensure that:
//! - Administrative operations have appropriate weight assignments
//! - System remains stable under parameter adjustments
//! - Resource consumption is accurately measured

#![cfg(feature = "runtime-benchmarks")]
use super::*;

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_runtime::traits::One;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn update_burned_energy_threshold() {
        let new_threshold = BalanceOf::<T>::zero();
        #[extrinsic_call]
        _(RawOrigin::Root, new_threshold);
        assert_last_event::<T>(Event::<T>::BurnedEnergyThresholdUpdated { new_threshold }.into());
    }

    #[benchmark]
    fn update_block_fullness_threshold() {
        let new_threshold = Perquintill::one();
        #[extrinsic_call]
        _(RawOrigin::Root, new_threshold);
        assert_last_event::<T>(Event::<T>::BlockFullnessThresholdUpdated { new_threshold }.into());
    }

    #[benchmark]
    fn update_upper_fee_multiplier() {
        let new_multiplier = Multiplier::one();
        #[extrinsic_call]
        _(RawOrigin::Root, new_multiplier);
        assert_last_event::<T>(Event::<T>::UpperFeeMultiplierUpdated { new_multiplier }.into());
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(0), crate::mock::Test);
}
