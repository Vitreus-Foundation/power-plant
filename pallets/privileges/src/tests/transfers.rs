use super::*;
use frame_support::traits::tokens::nonfungibles_v2::{Inspect as NftInspect, Mutate};

fn prepare_recipient(account: AccountId, balance: Balance) {
    ensure_recipient_nac(account);
    assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), account, balance));
    assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(account), PenaltyType::Flat));
}

fn ensure_recipient_nac(account: AccountId) {
    if NacManaging::get_nac_level(&account).is_none() {
        assert_ok!(NacManaging::mint(RuntimeOrigin::root(), 2, account));
    }
}

fn transfer(sender: AccountId, item: u32, recipient: AccountId) -> DispatchResult {
    NacManaging::transfer_vipp(RuntimeOrigin::signed(sender), item, recipient)
}

#[test]
fn last_vipp_transfer_moves_threshold_and_wipes_only_sender_current_vipp_points() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        prepare_recipient(20, 2000);
        let item = owned_vipp_ids(10)[0];
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 2));
        VippPoints::<Test>::insert(2019, 10, 77);
        let vip_before = Privileges::vip_members(10).unwrap();

        assert_ok!(transfer(10, item, 20));

        assert_eq!(Nfts::owner(1, item), Some(20));
        assert_eq!(NacManaging::get_claim_balance(&10).unwrap().0, 0);
        assert_eq!(NacManaging::get_claim_balance(&20).unwrap().0, 1000);
        assert!(NacManaging::is_vipp_inactive(&10));
        assert!(Privileges::vipp_members(10).is_none());
        assert_eq!(Privileges::vip_members(10).unwrap(), vip_before);
        assert_eq!(Privileges::vipp_points(2019, 10), Some(77));
        let received = Privileges::vipp_members(20).unwrap();
        assert_eq!(received.active_vipp_threshold, vec![(item, 950)]);
        assert_eq!(received.points, 0);
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 3));
        assert_eq!(Privileges::vipp_members(20).unwrap().points, 500);
        assert!(pallet_nac_managing::VippMutationItem::<Test>::get().is_none());
    });
}

#[test]
fn partial_vipp_transfer_preserves_points_and_existing_recipient_entitlements() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let item = owned_vipp_ids(10)[0];
        System::set_block_number(System::block_number() + 1);
        claim_for_validator(200);
        ensure_recipient_nac(20);
        join_vip_and_vipp(20);
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 20, 3000));
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 2));
        let sender_points = Privileges::vipp_members(10).unwrap().points;
        let recipient_points = Privileges::vipp_members(20).unwrap().points;

        assert_ok!(transfer(10, item, 20));

        assert_eq!(NacManaging::get_claim_balance(&10).unwrap().0, 200);
        assert_eq!(NacManaging::get_claim_balance(&20).unwrap().0, 2000);
        assert!(!NacManaging::is_vipp_inactive(&10));
        let sender = Privileges::vipp_members(10).unwrap();
        assert_eq!(sender.points, sender_points);
        assert_eq!(sender.active_vipp_threshold, vec![(owned_vipp_ids(10)[0], 190)]);
        let recipient = Privileges::vipp_members(20).unwrap();
        assert_eq!(recipient.points, recipient_points);
        assert_eq!(recipient.active_vipp_threshold.len(), 2);
        assert_eq!(
            recipient.active_vipp_threshold.iter().map(|(_, cap)| *cap).sum::<Balance>(),
            1900
        );
    });
}

#[test]
fn self_transfer_is_an_authenticated_noop() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let item = owned_vipp_ids(10)[0];
        frame_support::assert_storage_noop!(assert_ok!(transfer(10, item, 10)));
        assert_noop!(transfer(20, item, 20), pallet_nac_managing::Error::<Test>::NotVippOwner);
    });
}

#[test]
fn vipp_transfer_requires_the_actual_owner_and_signed_origin() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        prepare_recipient(20, 2000);
        let item = owned_vipp_ids(10)[0];
        assert_noop!(transfer(100, item, 20), pallet_nac_managing::Error::<Test>::NotVippOwner);
        assert_noop!(
            NacManaging::transfer_vipp(RuntimeOrigin::root(), item, 20),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn vipp_transfer_rejects_non_vip_and_inactive_recipients_without_forfeiture() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let item = owned_vipp_ids(10)[0];
        assert_noop!(
            transfer(10, item, 20),
            pallet_nac_managing::Error::<Test>::InvalidVippRecipient
        );
        prepare_recipient(20, 2000);
        let nac = NacManaging::get_nac_level(&20).unwrap().1;
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 3],
            &[1]
        ));
        assert_noop!(
            transfer(10, item, 20),
            pallet_nac_managing::Error::<Test>::InvalidVippRecipient
        );
        assert_eq!(owned_vipp_ids(10), vec![item]);
    });
}

#[test]
fn inactive_source_cannot_reactivate_a_stale_entitlement_by_transferring_it() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        prepare_recipient(20, 2000);
        let item = owned_vipp_ids(10)[0];
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 3],
            &[1]
        ));
        assert_noop!(
            transfer(10, item, 20),
            pallet_nac_managing::Error::<Test>::InvalidVippTransfer
        );
    });
}

#[test]
fn quarantined_entitlements_cannot_move_until_reconciled() {
    for quarantined in [10, 20] {
        ExtBuilder::default().build_and_execute(|| {
            join_vip_and_vipp(10);
            prepare_recipient(20, 2000);
            let item = owned_vipp_ids(10)[0];
            pallet_nac_managing::VippAccountingQuarantine::<Test>::insert(quarantined, ());
            assert_noop!(
                transfer(10, item, 20),
                pallet_nac_managing::Error::<Test>::VippAccountingUnavailable
            );
        });
    }
}

#[test]
fn vipp_transfer_checks_the_entire_recipient_threshold_with_equality_allowed() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        ensure_recipient_nac(20);
        join_vip_and_vipp(20);
        let item = owned_vipp_ids(10)[0];
        // The incoming cap is 950 but the recipient already has a cap of 950.
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 20, 1899));
        assert_noop!(
            transfer(10, item, 20),
            pallet_nac_managing::Error::<Test>::InsufficientVippBalance
        );
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 20, 1900));
        assert_ok!(transfer(10, item, 20));
        assert_eq!(NacManaging::get_claim_balance(&20).unwrap().0, 2000);
    });
}

#[test]
fn vipp_transfer_excludes_unvested_funds_and_accepts_them_as_they_vest() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        prepare_recipient(20, 1050);
        let item = owned_vipp_ids(10)[0];
        assert_ok!(
            <Vesting as frame_support::traits::VestingSchedule<AccountId>>::add_vesting_schedule(
                &20,
                101,
                1,
                System::block_number()
            )
        );
        assert_noop!(
            transfer(10, item, 20),
            pallet_nac_managing::Error::<Test>::InsufficientVippBalance
        );
        // No explicit vest call is needed to count the newly vested unit.
        System::set_block_number(System::block_number() + 1);
        assert_ok!(transfer(10, item, 20));
    });
}

#[test]
fn ordinary_nft_transfer_and_burn_cannot_bypass_vipp_accounting() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let item = owned_vipp_ids(10)[0];
        assert_noop!(
            Nfts::transfer(RuntimeOrigin::signed(10), 1, item, 20),
            pallet_nfts::Error::<Test>::ItemLocked
        );
        assert_noop!(
            Nfts::burn(RuntimeOrigin::signed(10), 1, item),
            pallet_nfts::Error::<Test>::ItemLocked
        );
        assert_noop!(
            Nfts::burn(RuntimeOrigin::root(), 1, item),
            pallet_nfts::Error::<Test>::ItemLocked
        );
        assert_noop!(
            <Nfts as frame_support::traits::tokens::nonfungibles_v2::Transfer<AccountId>>::transfer(
                &1, &item, &20
            ),
            pallet_nfts::Error::<Test>::ItemLocked
        );
    });
}

#[test]
fn nft_purchase_cannot_bypass_vipp_accounting_or_charge_the_buyer() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        prepare_recipient(20, 2000);
        let item = owned_vipp_ids(10)[0];
        assert_ok!(Nfts::set_price(RuntimeOrigin::signed(10), 1, item, Some(100), None));
        assert_noop!(
            Nfts::buy_item(RuntimeOrigin::signed(20), 1, item, 100),
            pallet_nfts::Error::<Test>::ItemLocked
        );
    });
}

#[test]
fn nft_swap_rolls_back_the_first_leg_when_vipp_second_leg_is_locked() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let item = owned_vipp_ids(10)[0];
        assert_ok!(NacManaging::create_collection(&10));
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::mint_into(
            &2,
            &123,
            &20,
            &Default::default(),
            true
        ));
        assert_ok!(Nfts::create_swap(RuntimeOrigin::signed(10), 1, item, 2, Some(123), None, 0));
        assert_noop!(
            Nfts::claim_swap(RuntimeOrigin::signed(20), 2, 123, 1, item, None),
            pallet_nfts::Error::<Test>::ItemLocked
        );
        assert_eq!(Nfts::owner(2, 123), Some(20));
        assert_eq!(Nfts::owner(1, item), Some(10));
    });
}

#[test]
fn vipp_locker_does_not_change_unrelated_collections() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(NacManaging::create_collection(&10));
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::mint_into(
            &2,
            &123,
            &10,
            &Default::default(),
            true
        ));
        assert_ok!(Nfts::transfer(RuntimeOrigin::signed(10), 2, 123, 20));
        assert_ok!(Nfts::burn(RuntimeOrigin::signed(20), 2, 123));
        assert!(<Nfts as NftInspect<AccountId>>::owner(&2, &123).is_none());
    });
}

#[test]
fn nac_certificate_cannot_move_or_disappear_outside_accounting() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        assert_noop!(
            Nfts::transfer(RuntimeOrigin::signed(10), 0, nac, 20),
            pallet_nfts::Error::<Test>::ItemLocked
        );
        assert_noop!(
            Nfts::burn(RuntimeOrigin::signed(10), 0, nac),
            pallet_nfts::Error::<Test>::ItemLocked
        );
        assert_noop!(
            Nfts::burn(RuntimeOrigin::root(), 0, nac),
            pallet_nfts::Error::<Test>::ItemLocked
        );
    });
}

#[test]
fn failed_nft_transfer_rolls_back_guard_and_accounting() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        prepare_recipient(20, 2000);
        let item = owned_vipp_ids(10)[0];
        // An independent NFT restriction fails after the temporary guard opens.
        assert_ok!(<Nfts as frame_support::traits::tokens::nonfungibles_v2::Transfer<AccountId>>::disable_transfer(&1, &item));
        assert_noop!(transfer(10, item, 20), pallet_nfts::Error::<Test>::ItemLocked);
        assert!(pallet_nac_managing::VippMutationItem::<Test>::get().is_none());
    });
}

#[test]
fn vipp_transfer_rejects_corrupt_source_threshold_instead_of_creating_entitlement() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        prepare_recipient(20, 2000);
        let item = owned_vipp_ids(10)[0];
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 2],
            &999u64.encode()
        ));
        assert_noop!(
            transfer(10, item, 20),
            pallet_nac_managing::Error::<Test>::InvalidVippTransfer
        );
    });
}

#[test]
fn january_1_transfer_saves_december_31_for_the_previous_owner() {
    ExtBuilder::default().build_and_execute(|| {
        let clock = |year, month, day| {
            Timestamp::set_timestamp(
                NaiveDate::from_ymd_opt(year, month, day)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp_millis() as u64,
            );
        };
        CurrentDate::<Test>::put(CurrentDateInfo::new::<Test>(2025, 12, 31).unwrap());
        clock(2025, 12, 31);
        join_vip_and_vipp(10);
        prepare_recipient(20, 2000);
        let item = owned_vipp_ids(10)[0];
        VippMembers::<Test>::mutate(10, |m| m.as_mut().unwrap().points = 100);
        clock(2026, 1, 1);

        assert_ok!(transfer(10, item, 20));

        assert_eq!(Privileges::vipp_points(2025, 10), Some(1050));
        assert!(Privileges::vipp_members(10).is_none());
        assert_eq!(Privileges::vipp_members(20).unwrap().points, 0);
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2026, 1, 2));
        assert_eq!(Privileges::vipp_members(20).unwrap().points, 500);
    });
}
