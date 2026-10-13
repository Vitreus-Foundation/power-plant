use super::*;
use crate::migration::recovery::*;
use frame_support::traits::tokens::nonfungibles_v2::Mutate;
use sp_runtime::DispatchError;

fn mint_legacy_nft(account: AccountId, item: u32, amount: Balance) {
    assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::mint_into(
        &1,
        &item,
        &account,
        &pallet_nfts::ItemConfig { settings: pallet_nfts::ItemSettings::all_enabled() },
        true,
    ));
    set_claim(item, &amount.encode());
    if let Some((claim, nac)) = NacManaging::get_claim_balance(&account) {
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 2],
            &(claim + amount).encode(),
        ));
    }
}

fn set_claim(item: u32, bytes: &[u8]) {
    assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
        &1,
        &item,
        &[0, 0, 2],
        bytes,
    ));
}

fn scan(nft_limit: u32) -> Progress {
    for _ in 0..100 {
        let progress = step::<Test>(2, nft_limit).unwrap();
        if progress.phase == Phase::AwaitingReview {
            return progress;
        }
    }
    panic!("bounded recovery did not finish");
}

fn historical_state() -> Vec<u8> {
    (
        VipPoints::<Test>::iter().collect::<Vec<_>>(),
        VippPoints::<Test>::iter().collect::<Vec<_>>(),
        Rewards::<Test>::iter().collect::<Vec<_>>(),
    )
        .encode()
}

#[test]
fn recovery_repairs_actual_nft_ids_and_stash_stake_without_changing_points_or_rewards() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        assert_ok!(EnergyGeneration::set_controller(RuntimeOrigin::signed(10), 11));
        let nft = owned_vipp_ids(10)[0];
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        VipMembers::<Test>::mutate(10, |member| {
            let member = member.as_mut().unwrap();
            member.active_stake = 0;
            member.points = 987;
        });
        VippMembers::<Test>::mutate(10, |member| {
            let member = member.as_mut().unwrap();
            member.active_vipp_threshold = vec![(nac, 0)];
            member.points = 654;
        });
        VipPoints::<Test>::insert(2019, 10, 100);
        VippPoints::<Test>::insert(2019, 10, 200);
        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), 2019, 1000, 2000));
        let history = historical_state();
        let date = CurrentDate::<Test>::get();

        assert!(begin::<Test>());
        let progress = scan(64);
        assert_eq!(progress.repaired_accounts, 1);
        assert_eq!(progress.pending_reviews, 1);
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 1000);
        assert_eq!(Privileges::vip_members(10).unwrap().points, 987);
        let vipp = Privileges::vipp_members(10).unwrap();
        assert_eq!(vipp.active_vipp_threshold, vec![(nft, 950)]);
        assert_eq!(vipp.points, 654);
        assert_eq!(historical_state(), history);
        assert_eq!(CurrentDate::<Test>::get(), date);
        let review = CacheRecoveryReviews::<Test>::get(10).unwrap();
        assert_eq!(review.issues, STAKE_CHANGED | NFT_CACHE_CHANGED);
        assert_eq!(review.previous_active_stake, Some(0));
        assert_eq!(review.vip_points, Some(987));
        assert_eq!(review.vipp_points, Some(654));
        assert!(is_frozen::<Test>());
    });
}

#[test]
fn recovery_resumes_from_persisted_cursor_and_respects_account_batch_limit() {
    ExtBuilder::default().build_and_execute(|| {
        for account in 1000..1037 {
            VipMembers::<Test>::insert(
                account,
                VipMemberInfo::<Test> {
                    start: 0,
                    tax_type: PenaltyType::Flat,
                    points: account,
                    active_stake: 777,
                },
            );
        }
        assert!(begin::<Test>());
        let first = step::<Test>(3, 64).unwrap();
        assert_eq!(first.scanned_rows, 3);
        assert_eq!(first.repaired_accounts, 3);
        assert!(!first.cursor.is_empty());
        System::set_block_number(99);
        assert!(!begin::<Test>());
        assert_eq!(CacheRecoveryProgress::<Test>::get().unwrap(), first);
        let next = step::<Test>(3, 64).unwrap();
        assert_eq!(next.scanned_rows, 6);
        assert_eq!(next.repaired_accounts, 6);
        assert_ne!(next.cursor, first.cursor);
        let done = scan(64);
        assert_eq!(done.scanned_rows, 37);
        assert_eq!(done.repaired_accounts, 37);
        assert_eq!(done.pending_reviews, 37);
        for account in 1000..1037 {
            let member = Privileges::vip_members(account).unwrap();
            assert_eq!(member.active_stake, 0);
            assert_eq!(member.points, account);
        }
    });
}

#[test]
fn recovery_quarantines_malformed_nft_and_retry_keeps_original_review_evidence() {
    use frame_support::traits::{Hooks, StorageVersion};
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nft = owned_vipp_ids(10)[0];
        VippMembers::<Test>::mutate(10, |m| m.as_mut().unwrap().points = 1234);
        let original = VippMembers::<Test>::get(10).unwrap().encode();
        set_claim(nft, &[1]);
        StorageVersion::new(1).put::<Privileges>();
        <Privileges as Hooks<u64>>::on_runtime_upgrade();
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(2));
        scan(64);
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original);
        assert_eq!(CacheRecoveryProgress::<Test>::get().unwrap().repaired_accounts, 0);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, MALFORMED_ATTRIBUTE);
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
        assert!(Privileges::finish_cache_recovery(RuntimeOrigin::root()).is_err());
        assert!(crate::accounting::paused::<Test>());
        set_claim(nft, &1000u64.encode());
        assert_ok!(retry_account::<Test>(&10, 64));
        let review = CacheRecoveryReviews::<Test>::get(10).unwrap();
        assert_eq!(review.issues & UNRESOLVED, 0);
        assert_eq!(review.vipp_points, Some(1234));
        assert_ok!(acknowledge::<Test>(&[10]));
        assert_ok!(finish::<Test>());
        assert!(!is_frozen::<Test>());
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original);
    });
}

#[test]
fn recovery_rejects_trailing_bytes_in_claim_attribute() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let mut bytes = 1000u64.encode();
        bytes.push(0);
        set_claim(owned_vipp_ids(10)[0], &bytes);
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, MALFORMED_ATTRIBUTE);
        assert!(is_frozen::<Test>());
    });
}

#[test]
fn recovery_bounded_nft_scan_preserves_entitlement_until_larger_retry() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        mint_legacy_nft(10, 777, 100);
        let original = VippMembers::<Test>::get(10).unwrap().encode();
        assert!(begin::<Test>());
        scan(1);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, NFT_LIMIT_EXCEEDED);
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original);
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
        assert_ok!(retry_account::<Test>(&10, 2));
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.active_vipp_threshold.iter().map(|(_, cap)| cap).sum::<u64>(), 1045);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues & UNRESOLVED, 0);
    });
}

#[test]
fn recovery_does_not_decode_or_truncate_oversized_legacy_cache() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        VippMembers::<Test>::mutate(10, |member| {
            let member = member.as_mut().unwrap();
            member.points = 123;
            member.active_vipp_threshold = vec![(7, 999); 65];
        });
        let original = VippMembers::<Test>::get(10).unwrap().encode();
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, CACHE_LIMIT_EXCEEDED);
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original);
        assert!(is_frozen::<Test>());
    });
}

#[test]
fn recovery_preserves_ghost_and_orphan_points_but_removes_future_reward_caps() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        VippMembers::<Test>::insert(
            10,
            VippMemberInfo::<Test> { points: 456, active_vipp_threshold: vec![(999, 1000)] },
        );
        VippMembers::<Test>::insert(
            40,
            VippMemberInfo::<Test> { points: 789, active_vipp_threshold: vec![(888, 2000)] },
        );
        assert!(begin::<Test>());
        scan(64);
        for (account, points) in [(10, 456), (40, 789)] {
            let member = Privileges::vipp_members(account).unwrap();
            assert_eq!(member.points, points);
            assert!(member.active_vipp_threshold.is_empty());
            assert_ne!(
                CacheRecoveryReviews::<Test>::get(account).unwrap().issues & NO_OWNED_NFT,
                0
            );
        }
        assert_ne!(CacheRecoveryReviews::<Test>::get(40).unwrap().issues & NOT_A_VIP, 0);
    });
}

#[test]
fn recovery_reports_zero_nfts_without_burning_them_or_erasing_existing_points() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let real_id = owned_vipp_ids(10)[0];
        mint_legacy_nft(10, 777, 0);
        VippMembers::<Test>::mutate(10, |member| {
            let member = member.as_mut().unwrap();
            member.points = 456;
            member.active_vipp_threshold = vec![(777, 0)];
        });
        assert!(begin::<Test>());
        scan(64);
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, 456);
        assert_eq!(member.active_vipp_threshold, vec![(real_id, 950)]);
        assert_eq!(owned_vipp_ids(10).len(), 2);
        assert_ne!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues & ZERO_VALUE_NFT, 0);
    });
}

#[test]
fn recovery_inactive_nac_keeps_nft_and_points_for_review_without_reactivating_cap() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 3],
            &[1],
        ));
        VippMembers::<Test>::mutate(10, |member| member.as_mut().unwrap().points = 456);
        assert!(begin::<Test>());
        scan(64);
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, 456);
        assert!(member.active_vipp_threshold.is_empty());
        assert_eq!(owned_vipp_ids(10).len(), 1);
        assert!(NacManaging::is_vipp_inactive(&10));
        assert_ne!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues & INACTIVE_NAC, 0);
    });
}

#[test]
fn recovery_finish_requires_explicit_review_and_is_idempotent_after_completion() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        VipMembers::<Test>::mutate(10, |member| member.as_mut().unwrap().active_stake = 0);
        assert!(begin::<Test>());
        assert_err!(finish::<Test>(), DispatchError::Other("Recovery still requires review"));
        scan(64);
        assert_err!(finish::<Test>(), DispatchError::Other("Recovery still requires review"));
        assert_ok!(acknowledge::<Test>(&[10, 10]));
        assert_ok!(acknowledge::<Test>(&[10]));
        assert_eq!(CacheRecoveryProgress::<Test>::get().unwrap().pending_reviews, 0);
        assert_ok!(finish::<Test>());
        let done = CacheRecoveryProgress::<Test>::get().unwrap();
        let member = Privileges::vip_members(10).unwrap().encode();
        assert!(!begin::<Test>());
        assert_eq!(step::<Test>(16, 64).unwrap(), done);
        assert_eq!(Privileges::vip_members(10).unwrap().encode(), member);
        assert!(!is_frozen::<Test>());
    });
}

#[test]
fn recovery_rejects_zero_or_unbounded_work_limits_without_advancing_cursor() {
    ExtBuilder::default().build_and_execute(|| {
        assert!(begin::<Test>());
        let original = CacheRecoveryProgress::<Test>::get().unwrap();
        for (accounts, nfts) in [(0, 64), (17, 64), (16, 0), (16, 65)] {
            assert_err!(
                step::<Test>(accounts, nfts),
                DispatchError::Other("Invalid recovery batch limits")
            );
            assert_eq!(CacheRecoveryProgress::<Test>::get().unwrap(), original);
        }
    });
}

#[test]
fn recovery_never_overwrites_points_in_a_malformed_member_record() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        VippMembers::<Test>::mutate(10, |member| member.as_mut().unwrap().points = 456);
        let key = VippMembers::<Test>::hashed_key_for(10);
        let mut bytes = VippMembers::<Test>::get(10).unwrap().encode();
        bytes.pop();
        sp_io::storage::set(&key, &bytes);
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(sp_io::storage::get(&key).unwrap().as_ref(), bytes.as_slice());
        let review = CacheRecoveryReviews::<Test>::get(10).unwrap();
        assert_eq!(review.issues, MALFORMED_MEMBER);
        assert_eq!(review.vipp_points, Some(456));
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
    });
}

#[test]
fn recovery_counts_population_in_batches_and_refuses_unsafe_release_above_limit() {
    ExtBuilder::default().build_and_execute(|| {
        for account in 1000..1000 + u64::from(MAX_MEMBERS_PER_KIND) + 1 {
            VipMembers::<Test>::insert(
                account,
                VipMemberInfo::<Test> {
                    start: 0,
                    tax_type: PenaltyType::Flat,
                    points: account,
                    active_stake: 0,
                },
            );
        }
        assert!(begin::<Test>());
        assert_eq!(CacheRecoveryProgress::<Test>::get().unwrap().vip_rows, 0);
        for _ in 0..100 {
            let progress = step::<Test>(16, 64).unwrap();
            if progress.phase == Phase::AwaitingReview {
                assert_eq!(progress.vip_rows, MAX_MEMBERS_PER_KIND + 1);
                assert_eq!(progress.vipp_rows, 0);
                assert_eq!(progress.pending_reviews, 0);
                assert_err!(
                    finish::<Test>(),
                    DispatchError::Other("Recovered population exceeds supported accounting bound")
                );
                assert!(is_frozen::<Test>());
                return;
            }
        }
        panic!("population scan did not finish");
    });
}

#[test]
fn recovery_missing_nac_does_not_infer_eligibility_or_overwrite_entitlement() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        // NACs cannot normally be burned. Simulate a damaged legacy owner index.
        pallet_nfts::Account::<Test>::remove((10, 0, nac));
        let original = VippMembers::<Test>::get(10).unwrap().encode();
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, MISSING_NAC);
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original);
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
    });
}

#[test]
fn recovery_review_batch_rolls_back_if_any_account_is_unresolved() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        join_vip_and_vipp(100);
        VipMembers::<Test>::mutate(10, |member| member.as_mut().unwrap().active_stake = 0);
        set_claim(owned_vipp_ids(100)[0], &[1]);
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(CacheRecoveryProgress::<Test>::get().unwrap().pending_reviews, 2);
        assert_err!(
            acknowledge::<Test>(&[10, 100]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
        assert!(!CacheRecoveryReviews::<Test>::get(10).unwrap().acknowledged);
        assert_eq!(CacheRecoveryProgress::<Test>::get().unwrap().pending_reviews, 2);
    });
}

#[test]
fn recovery_quarantines_underfunding_without_burning_and_retries_after_explicit_repair() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 2],
            &100_000u64.encode(),
        ));
        let original = VippMembers::<Test>::get(10).unwrap().encode();
        let original_ids = owned_vipp_ids(10);
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, UNDERFUNDED);
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original);
        assert_eq!(owned_vipp_ids(10), original_ids);
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
        // Model an explicitly reviewed NAC correction; migration itself never writes NACs.
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 2],
            &1000u64.encode(),
        ));
        assert_ok!(retry_account::<Test>(&10, 64));
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues & UNRESOLVED, 0);
        assert_ok!(acknowledge::<Test>(&[10]));
        assert_eq!(owned_vipp_ids(10), original_ids);
    });
}

#[test]
fn recovery_quarantines_malformed_nac_claim_and_threshold_below_owned_nfts() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nac = NacManaging::get_nac_level(&10).unwrap().1;
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 2],
            &[1],
        ));
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, MALFORMED_NAC_CLAIM);
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac,
            &[0, 0, 2],
            &1u64.encode(),
        ));
        assert_ok!(retry_account::<Test>(&10, 64));
        assert_eq!(
            CacheRecoveryReviews::<Test>::get(10).unwrap().issues & UNRESOLVED,
            THRESHOLD_BELOW_NFTS
        );
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
    });
}

#[test]
fn recovery_cannot_release_quarantined_historical_breach_after_balance_refill() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        pallet_nac_managing::VippAccountingQuarantine::<Test>::insert(10, ());
        // A healthy current balance is not proof that an earlier breach was reconciled.
        assert!(NacManaging::threshold_meets_vipp_requirements(&10, 1000));
        assert!(begin::<Test>());
        scan(64);
        assert_eq!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues, QUARANTINED_BALANCE);
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
        assert_ok!(retry_account::<Test>(&10, 64));
        assert_ne!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues & QUARANTINED_BALANCE, 0);
        assert!(NacManaging::is_vipp_quarantined(&10));
    });
}

#[test]
fn recovery_refuses_a_damaged_owner_index_pointing_at_someone_elses_nft() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        join_vip_and_vipp(100);
        let foreign = owned_vipp_ids(100)[0];
        pallet_nfts::Account::<Test>::insert((10, 1, foreign), ());
        let original = VippMembers::<Test>::get(10).unwrap().encode();
        assert!(begin::<Test>());
        scan(64);
        assert_ne!(CacheRecoveryReviews::<Test>::get(10).unwrap().issues & NFT_OWNER_MISMATCH, 0);
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original);
        assert_err!(
            acknowledge::<Test>(&[10]),
            DispatchError::Other("Unresolved authoritative recovery data")
        );
    });
}
