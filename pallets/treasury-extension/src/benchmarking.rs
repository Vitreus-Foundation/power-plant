//! Benchmarking setup for pallet-treasury-extension
#![cfg(feature = "runtime-benchmarks")]
use super::*;

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_runtime::traits::{One, Zero};
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
