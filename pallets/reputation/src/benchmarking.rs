//! Benchmarking setup for pallet-template
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
        let _ = PalletReputation::<T>::update_points(RawOrigin::Root.into(), account.clone());
        #[extrinsic_call]
        increase_points(RawOrigin::Root, account, points);
    }

    #[benchmark]
    fn slash() {
        let points = 100.into();
        let account: T::AccountId = whitelisted_caller();
        let _ = PalletReputation::<T>::update_points(RawOrigin::Root.into(), account.clone());
        #[extrinsic_call]
        slash(RawOrigin::Root, account, points);
    }

    #[benchmark]
    fn update_points() {
        let account: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        update_points(RawOrigin::Root, account.clone());
    }

    impl_benchmark_test_suite!(PalletReputation, crate::mock::new_test_ext(), crate::mock::Test);
}
