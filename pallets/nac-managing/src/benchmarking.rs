#![cfg(feature = "runtime-benchmarks")]
use super::*;

use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn mint() {
        let nac_level = 5_u8;
        let who: AccountIdOf<T> = whitelisted_caller();
        let item_id = Pallet::<T>::create_unique_item_id(&who);

        Pallet::<T>::create_collection(&who).expect("Expected to create NAC NFT collection");

        #[extrinsic_call]
        _(RawOrigin::Root, nac_level, who.clone());

        assert_eq!(Pallet::<T>::get_nac_level(&who), Some((nac_level, item_id)));
        assert_last_event::<T>(Event::<T>::NftUpdated { owner: who, nac_level }.into());
    }

    #[benchmark]
    fn update_nft() {
        let who: AccountIdOf<T> = whitelisted_caller();
        let initial_nac_level = 1_u8;
        let new_nac_level = 5_u8;
        let item_id = Pallet::<T>::create_unique_item_id(&who);

        Pallet::<T>::create_collection(&who).expect("Expected to create NAC NFT collection");
        Pallet::<T>::mint(RawOrigin::Root.into(), initial_nac_level, who.clone())
            .expect("Expected to mint a NAC NFT");

        #[extrinsic_call]
        _(RawOrigin::Root, Some(new_nac_level), who.clone());

        assert_eq!(Pallet::<T>::get_nac_level(&who), Some((new_nac_level, item_id)));
        assert_last_event::<T>(
            Event::<T>::NftUpdated { owner: who, nac_level: new_nac_level }.into(),
        );
    }

    #[benchmark]
    fn check_nac_level() {
        let owner: AccountIdOf<T> = whitelisted_caller();
        let nac_level = 1_u8;

        Pallet::<T>::create_collection(&owner).expect("Expected to create NAC NFT collection");
        Pallet::<T>::mint(RawOrigin::Root.into(), nac_level, owner.clone())
            .expect("Expected to mint a NAC NFT");

        #[extrinsic_call]
        _(RawOrigin::Root, owner.clone());

        assert_last_event::<T>(Event::UserNacLevel { nac_level, owner }.into());
    }

    // #[benchmark]
    // fn claim() {
    //     let who = whitelisted_caller();
    //     let bob_account = eth(&bob());
    //
    //     Pallet::<T>::mint_tokens_to_claim(RawOrigin::Root.into(), 150u32.into())
    //         .expect("Expected to mint tokens to claim");
    //     assert_eq!(pallet_balances::Pallet::<T>::free_balance(&who), Zero::zero());
    //     assert_eq!(Pallet::<T>::claims(&bob_account), None);
    //     Pallet::<T>::mint_claim(RawOrigin::Root.into(), bob_account, 100u32.into())
    //         .expect("Expected to mint claim");
    //
    //     #[extrinsic_call]
    //     _(RawOrigin::Signed(who.clone()), sig::<T>(&bob(), &who.encode(), &[][..]));
    //
    //     assert_eq!(pallet_balances::Pallet::<T>::free_balance(&who), 100u32.into());
    //     assert_eq!(Pallet::<T>::total(), 50u32.into());
    //     assert_last_event::<T>(
    //         Event::<T>::Claimed { account_id: who, amount: 100u32.into() }.into(),
    //     );
    // }
    //
    // #[benchmark]
    // fn mint_tokens_to_claim() {
    //     let claim_account = Pallet::<T>::claim_account_id();
    //     assert_eq!(pallet_balances::Pallet::<T>::free_balance(&claim_account), Zero::zero());
    //     assert_eq!(Pallet::<T>::total(), Zero::zero());
    //
    //     let claim_amount: BalanceOf<T> = 100u32.into();
    //
    //     #[extrinsic_call]
    //     _(RawOrigin::Root, claim_amount);
    //
    //     // type conflicts, can't use `claim_amount` here, switching to integer literal
    //     assert_eq!(pallet_balances::Pallet::<T>::free_balance(&claim_account), 100u32.into());
    //     assert_eq!(Pallet::<T>::total(), claim_amount);
    //     assert_last_event::<T>(Event::<T>::TokenMintedToClaim(claim_amount).into());
    // }
    //
    // #[benchmark]
    // fn mint_claim() {
    //     let bob_account = eth(&bob());
    //     Pallet::<T>::mint_tokens_to_claim(RawOrigin::Root.into(), 150u32.into())
    //         .expect("Expected to mint tokens to claim");
    //     assert_eq!(Pallet::<T>::claims(&bob_account), None);
    //
    //     let claim_amount: BalanceOf<T> = 100u32.into();
    //
    //     #[extrinsic_call]
    //     _(RawOrigin::Root, bob_account.clone(), claim_amount);
    //
    //     assert_eq!(Pallet::<T>::claims(&bob_account), Some(claim_amount));
    // }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
