//! Benchmarking Setup for Treasury Pallet
//!
//! This module provides the benchmarking setup for the Treasury pallet, allowing developers to evaluate the performance of its key functions.
//! It leverages the FRAME benchmarking framework to define and run performance tests, ensuring that Treasury operations meet acceptable performance standards.
//!
//! # Features
//! - Defines benchmarks for critical Treasury functions, such as `spend_funds`.
//! - Uses the `runtime-benchmarks` feature to conditionally compile benchmarking code.
//! - Benchmarks are written using FRAME's benchmarking tools to provide consistent and accurate performance metrics.
//!
//! # Structure
//! - Contains the `benchmarks` module that defines benchmarking functions for the Treasury pallet.
//! - Includes utility functions such as `assert_last_event` to validate the last event triggered by the benchmarking operation.
//! - Focuses on key operations like fund spending, evaluating both computational complexity and resource utilization.
//!
//! # Usage
//! - Include this benchmarking module to assess and improve the runtime performance of the Treasury pallet.
//! - Run the benchmarking suite using Substrate's benchmarking CLI tools to generate weight information.
//!
//! # Benchmarks Overview
//! - **spend_funds**: Benchmarks the process of spending funds from the Treasury, including the impact on budget and remaining funds.
//! - Additional benchmarks may be added to evaluate other Treasury operations, ensuring comprehensive performance coverage.
//!
//! # Dependencies
//! - Relies on `frame_benchmarking` for benchmarking macros and tools, and `pallet_treasury` for Treasury-specific functionality.
//! - Utilizes the `SpendFunds` trait to benchmark fund allocation processes.
//!
//! # Important Notes
//! - Benchmarking results are critical for setting appropriate weights, ensuring that operations are fair and do not overwhelm the blockchain's resources.
//! - Always run benchmarks in an environment that simulates production as closely as possible to ensure realistic weight calculations.
//! - Expand the benchmarking suite as needed to cover additional Treasury pallet functionalities or new features.

#![cfg(feature = "runtime-benchmarks")]
use super::*;

use frame_benchmarking::v2::*;
use pallet_treasury::SpendFunds;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn spend_funds() {
        let budget_remaining = pallet_treasury::Pallet::<T>::pot();
        let threshold = <T as crate::Config>::SpendThreshold::get().mul_ceil(budget_remaining);
        let mut left = budget_remaining;
        let mut imbalance = PositiveImbalanceOf::<T>::zero();
        let mut total_weight = Weight::zero();
        let mut missed_any = false;

        #[block]
        {
            crate::Pallet::<T>::spend_funds(
                &mut left,
                &mut imbalance,
                &mut total_weight,
                &mut missed_any,
            );
        }
        assert_last_event::<T>(Event::<T>::Recycled { recyled_funds: threshold }.into());
        assert_eq!(left, budget_remaining - threshold);
        assert_eq!(imbalance.peek(), threshold);
        assert!(missed_any);
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
