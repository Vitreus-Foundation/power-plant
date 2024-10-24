//! Benchmarking Setup for Reputation Pallet
//!
//! This module provides the benchmarking setup for the Reputation pallet, allowing developers to evaluate the performance of specific pallet functions.
//! It leverages FRAME benchmarking tools to define and run performance tests on key dispatchable functions, ensuring the pallet meets desired efficiency standards.
//!
//! # Features
//! - Defines benchmarks for critical functions, focusing on evaluating their runtime performance.
//! - Uses the `runtime-benchmarks` feature to conditionally compile benchmarking code.
//! - Benchmarks are written using the FRAME benchmarking framework, ensuring consistency and accuracy.
//!
//! # Structure
//! - Contains benchmarking functions encapsulated in the `benchmarks` module.
//! - Uses the `PalletReputation` alias to easily access pallet functions and ensure benchmarking is done on the correct pallet.
//! - Includes various benchmarks such as `force_set_points` to measure the cost of setting reputation points manually.
//!
//! # Usage
//! - Include this file to add benchmarking capabilities to the Reputation pallet, useful for determining and optimizing runtime performance.
//! - Run the benchmarking setup using Substrate's benchmarking CLI tools to generate weight information.
//!
//! # Benchmarks Overview
//! - **force_set_points**: Measures the cost of using the `force_set_points` extrinsic to set reputation points for an account from the Root origin.
//! - **other benchmarks**: Additional benchmarking functions can be added to assess different aspects of the pallet's performance.
//!
//! # Dependencies
//! - Relies on `frame_benchmarking` for benchmarking macros and tools, and `frame_system` for access to origin types and utilities.
//! - Utilizes the `whitelisted_caller` function to simulate caller behavior without affecting benchmark results.
//!
//! # Important Notes
//! - Benchmarks are intended for runtime developers to fine-tune and optimize the pallet's performance.
//! - Ensure benchmarking is run in an isolated environment to avoid inconsistencies caused by external factors.

#![cfg(feature = "runtime-benchmarks")]
use super::*;

#[allow(unused)]
use crate::Pallet as PalletReputation;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn force_set_points() {
        let points = 100.into();
        let account = whitelisted_caller();
        #[extrinsic_call]
        force_set_points(RawOrigin::Root, account, points);
    }

    #[benchmark]
    fn increase_points() {
        let points = 100.into();
        let account: T::AccountId = whitelisted_caller();
        PalletReputation::<T>::update_points(
            RawOrigin::Signed(account.clone()).into(),
            account.clone(),
        )
        .expect("Expected to update whitelisted caller's points");
        #[extrinsic_call]
        increase_points(RawOrigin::Root, account, points);
    }

    #[benchmark]
    fn slash() {
        let points = 100.into();
        let account: T::AccountId = whitelisted_caller();
        PalletReputation::<T>::update_points(
            RawOrigin::Signed(account.clone()).into(),
            account.clone(),
        )
        .expect("Expected to update whitelisted caller's points");
        #[extrinsic_call]
        slash(RawOrigin::Root, account, points);
    }

    #[benchmark]
    fn update_points() {
        let account: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        update_points(RawOrigin::Signed(account.clone()), account.clone());
    }

    impl_benchmark_test_suite!(PalletReputation, crate::mock::new_test_ext(), crate::mock::Test);
}
