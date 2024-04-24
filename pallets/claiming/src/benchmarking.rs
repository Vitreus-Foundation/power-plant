//! Benchmarking setup for pallet-claiming
#![cfg(feature = "runtime-benchmarks")]
use super::secp_utils::{eth, sig};
use super::*;

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use sp_runtime::traits::Zero;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

pub(crate) fn bob() -> libsecp256k1::SecretKey {
    libsecp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn claim() {
        let who = whitelisted_caller();
        let bob_account = eth(&bob());

        Pallet::<T>::mint_tokens_to_claim(RawOrigin::Root.into(), 150u32.into())
            .expect("Expected to mint tokens to claim");
        assert_eq!(pallet_balances::Pallet::<T>::free_balance(&who), Zero::zero());
        assert_eq!(Pallet::<T>::claims(&bob_account), None);
        Pallet::<T>::mint_claim(RawOrigin::Root.into(), bob_account, 100u32.into())
            .expect("Expected to mint claim");

        #[extrinsic_call]
        _(RawOrigin::Signed(who.clone()), sig::<T>(&bob(), &who.encode(), &[][..]));

        assert_eq!(pallet_balances::Pallet::<T>::free_balance(&who), 100u32.into());
        assert_eq!(Pallet::<T>::total(), 50u32.into());
        assert_last_event::<T>(
            Event::<T>::Claimed { account_id: who, amount: 100u32.into() }.into(),
        );
    }

    #[benchmark]
    fn mint_tokens_to_claim() {
        let claim_account = Pallet::<T>::claim_account_id();
        assert_eq!(pallet_balances::Pallet::<T>::free_balance(&claim_account), Zero::zero());
        assert_eq!(Pallet::<T>::total(), Zero::zero());

        let claim_amount: BalanceOf<T> = 100u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, claim_amount);

        // type conflicts, can't use `claim_amount` here, switching to integer literal
        assert_eq!(pallet_balances::Pallet::<T>::free_balance(&claim_account), 100u32.into());
        assert_eq!(Pallet::<T>::total(), claim_amount);
        assert_last_event::<T>(Event::<T>::TokenMintedToClaim(claim_amount).into());
    }

    #[benchmark]
    fn mint_claim() {
        let bob_account = eth(&bob());
        Pallet::<T>::mint_tokens_to_claim(RawOrigin::Root.into(), 150u32.into())
            .expect("Expected to mint tokens to claim");
        assert_eq!(Pallet::<T>::claims(&bob_account), None);

        let claim_amount: BalanceOf<T> = 100u32.into();

        #[extrinsic_call]
        _(RawOrigin::Root, bob_account.clone(), claim_amount);

        assert_eq!(Pallet::<T>::claims(&bob_account), Some(claim_amount));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
