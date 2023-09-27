//! Benchmarking setup for pallet-energy-fee
#![cfg(feature = "runtime-benchmarks")]
use super::*;

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

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

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(0), crate::mock::Test);
}