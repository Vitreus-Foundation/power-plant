//! Benchmarking setup for pallet-Faucet
#![cfg(feature = "runtime-benchmarks")]
use super::*;

#[allow(unused)]
use crate::Pallet as Faucet;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn request_funds() {
        // Unsigned call: `who` is the beneficiary, `amount` must be within `MaxAmount`.
        let amount: T::Balance = 100u32.into();
        let who: T::AccountId = whitelisted_caller();
        #[extrinsic_call]
        request_funds(RawOrigin::None, who.clone(), amount);

        assert!(Requests::<T>::contains_key(&who));
    }

    impl_benchmark_test_suite!(Faucet, crate::mock::new_test_ext(), crate::mock::Test);
}
