use super::*;
use crate::accounting as accrual;
fn date(year: i32, month: u32, day: u32) -> CurrentDateInfo {
    CurrentDateInfo::new::<Test>(year, month, day).unwrap()
}

fn clock(year: i32, month: u32, day: u32) {
    Timestamp::set_timestamp(
        NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis() as u64,
    );
}

fn timestamp_hook() {
    <Privileges as frame_support::traits::OnTimestampSet<u64>>::on_timestamp_set(Timestamp::get());
}

fn populate_maximum_cached_population() {
    for account in 1000..1000 + accrual::MAX_MEMBERS as u64 {
        VipMembers::<Test>::insert(
            account,
            VipMemberInfo::<Test> {
                start: 0,
                tax_type: PenaltyType::Flat,
                points: 0,
                active_stake: 1000,
            },
        );
        VippMembers::<Test>::insert(
            account,
            VippMemberInfo::<Test> {
                points: 0,
                active_vipp_threshold: (0..64)
                    .map(|item| (account as u32 * 64 + item, if item == 63 { 5 } else { 15 }))
                    .collect(),
            },
        );
    }
}

#[test]
fn paused_accounting_uses_actual_month_for_unbond_penalty() {
    for (anchor, now, expected_penalty) in
        [((2020, 1, 1), (2020, 2, 1), 70), ((2025, 12, 31), (2026, 1, 1), 0)]
    {
        ExtBuilder::default().build_and_execute(|| {
            CurrentDate::<Test>::put(date(anchor.0, anchor.1, anchor.2));
            clock(anchor.0, anchor.1, anchor.2);
            join_vip_and_vipp(10);
            accrual::AccrualReviewRequired::<Test>::put(true);
            clock(now.0, now.1, now.2);
            let old_balance = Balances::balance(&10);
            assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 400));
            assert_eq!(Balances::balance(&10), old_balance - expected_penalty);
            assert_eq!(CurrentDate::<Test>::get(), date(anchor.0, anchor.1, anchor.2));
        });
    }
}

#[test]
fn paused_accounting_uses_actual_quarter_for_declining_unbond_penalty() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(1000);
        assert_ok!(Privileges::become_vip_status(
            RuntimeOrigin::signed(10),
            PenaltyType::Declining
        ));
        accrual::AccrualReviewRequired::<Test>::put(true);
        clock(2020, 10, 1);
        let old_balance = Balances::balance(&10);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 400));
        assert_eq!(Balances::balance(&10), old_balance - 40);
        assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 1));
    });
}

#[test]
fn timestamp_hook_credits_completed_day_once_before_any_user_dispatch() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        clock(2020, 1, 2);
        <Privileges as frame_support::traits::OnTimestampSet<u64>>::on_timestamp_set(
            Timestamp::get(),
        );
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
        assert_eq!(Privileges::vip_members(10).unwrap().points, 20);
        <Privileges as frame_support::traits::OnTimestampSet<u64>>::on_timestamp_set(
            Timestamp::get(),
        );
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
    });
}

#[test]
fn maximum_population_daily_hook_processes_all_sources_once() {
    ExtBuilder::default().build_and_execute(|| {
        populate_maximum_cached_population();
        clock(2020, 1, 2);
        let started = std::time::Instant::now();
        <Privileges as frame_support::traits::OnTimestampSet<u64>>::on_timestamp_set(Timestamp::get());
        let elapsed = started.elapsed();
        assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 2));
        assert_eq!(VipMembers::<Test>::iter().count(), 1024);
        assert_eq!(VippMembers::<Test>::iter().count(), 1024);
        assert!(VipMembers::<Test>::iter().all(|(_, member)| member.points == 20));
        assert!(VippMembers::<Test>::iter().all(|(_, member)| member.points == 950));
        <Privileges as frame_support::traits::OnTimestampSet<u64>>::on_timestamp_set(Timestamp::get());
        assert!(VippMembers::<Test>::iter().all(|(_, member)| member.points == 950));
        println!("native_test_timing scenario=daily_1024_vip_1024_vipp_64_caps_each elapsed_us={} (unit-test host timing, not runtime benchmark)", elapsed.as_micros());
    });
}

#[test]
fn initialization_chill_before_midnight_timestamp_preserves_the_day_being_closed() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        System::set_block_number(1);
        timestamp_hook();
        System::set_block_number(2);
        // Session/Scheduler initialization still sees the previous block's timestamp.
        assert_ok!(EnergyGeneration::force_chill(RuntimeOrigin::root(), 10));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 0);
        assert_eq!(PreTimestampStake::<Test>::get(10), Some((2, 1000, false)));
        clock(2020, 1, 2);
        timestamp_hook();
        assert_eq!(Privileges::vip_members(10).unwrap().points, 20);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 0);
        System::set_block_number(3);
        clock(2020, 1, 3);
        timestamp_hook();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
    });
}

#[test]
fn repeated_initialization_stake_changes_keep_only_first_snapshot() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        System::set_block_number(1);
        timestamp_hook();
        System::set_block_number(2);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 400));
        assert_ok!(EnergyGeneration::bond_extra(RuntimeOrigin::signed(10), 100));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 700);
        assert_eq!(PreTimestampStake::<Test>::get(10), Some((2, 1000, false)));
        clock(2020, 1, 2);
        timestamp_hook();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
        System::set_block_number(3);
        clock(2020, 1, 3);
        timestamp_hook();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 1650);
    });
}

#[test]
fn same_day_initialization_snapshot_never_leaks_into_tomorrows_accrual() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        System::set_block_number(1);
        timestamp_hook();
        System::set_block_number(2);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 300));
        // The next timestamp is still on January 1; no day is closed in this block.
        clock(2020, 1, 1);
        timestamp_hook();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        System::set_block_number(3);
        clock(2020, 1, 2);
        timestamp_hook();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 700);
    });
}

#[test]
fn vipp_registration_respects_member_capacity() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(1000);
        for account in 10_000..10_000 + crate::accounting::MAX_MEMBERS as u64 {
            VippMembers::<Test>::insert(
                account,
                VippMemberInfo::<Test> { points: 0, active_vipp_threshold: Vec::new() },
            );
        }
        assert_noop!(
            Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat),
            Error::<Test>::AccountingCapacityExceeded
        );
        assert!(owned_vipp_ids(10).is_empty());
        assert!(!VipMembers::<Test>::contains_key(10));
    });
}

#[test]
fn vip_and_nft_dispatches_before_timestamp_are_rejected_until_the_clock_is_known() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        join_vip_and_vipp(100);
        System::set_block_number(1);
        timestamp_hook();
        System::set_block_number(2);
        assert_noop!(
            Privileges::exit_vip(RuntimeOrigin::signed(10)),
            Error::<Test>::AccountingPaused
        );
        assert_noop!(
            Privileges::become_vip_status(RuntimeOrigin::signed(20), PenaltyType::Flat),
            Error::<Test>::AccountingPaused
        );
        assert_noop!(
            NacManaging::transfer_vipp(RuntimeOrigin::signed(10), owned_vipp_ids(10)[0], 100),
            Error::<Test>::AccountingPaused
        );
        clock(2020, 1, 2);
        timestamp_hook();
        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert!(PreTimestampStake::<Test>::get(10).is_none());
    });
}

fn deferred_december_breach(two_nfts: bool) {
    CurrentDate::<Test>::put(date(2025, 12, 31));
    clock(2025, 12, 31);
    join_vip_and_vipp(10);
    System::set_block_number(1);
    timestamp_hook();
    if two_nfts {
        System::set_block_number(2);
        timestamp_hook();
        claim_for_validator(1000);
    }
    VippMembers::<Test>::mutate(10, |member| member.as_mut().unwrap().points = 5000);
    VippPoints::<Test>::insert(2024, 10, 777);
    assert_ok!(Balances::force_set_balance(
        RuntimeOrigin::root(),
        10,
        if two_nfts { 1000 } else { 100 }
    ));
    <NacManaging as pallet_energy_fee::OnWithdrawFeeHandler<AccountId>>::on_withdraw_fee(&10);
    assert_eq!(VippBreachYear::<Test>::get(10), Some(2025));
    pallet_nac_managing::VippReconciliationsThisBlock::<Test>::put(2);
    NacManaging::finish_balance_changes();
    assert!(NacManaging::is_vipp_quarantined(&10));
    assert_eq!(owned_vipp_ids(10).len(), if two_nfts { 2 } else { 1 });
    System::set_block_number(3);
    clock(2026, 1, 1);
    timestamp_hook();
    assert_eq!(Privileges::vipp_points(2025, 10), Some(5000));
    System::set_block_number(4);
    clock(2026, 1, 2);
    timestamp_hook();
}

#[test]
fn quarantined_vipp_entitlement_cannot_generate_new_points() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        VippMembers::<Test>::mutate(10, |m| m.as_mut().unwrap().points = 456);
        pallet_nac_managing::VippAccountingQuarantine::<Test>::insert(10, ());
        clock(2020, 1, 2);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 456);
        assert_eq!(Privileges::vip_members(10).unwrap().points, 20);
    });
}

#[test]
fn pre_timestamp_fee_check_preserves_closed_year_before_finalization_burn() {
    ExtBuilder::default().build_and_execute(|| {
        CurrentDate::<Test>::put(date(2025, 12, 31));
        clock(2025, 12, 31);
        join_vip_and_vipp(10);
        System::set_block_number(1);
        timestamp_hook();
        System::set_block_number(2);
        // Explicit fee checks can run before the timestamp, e.g. scheduled EVM calls.
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 100));
        <NacManaging as pallet_energy_fee::OnWithdrawFeeHandler<AccountId>>::on_withdraw_fee(&10);
        assert!(NacManaging::is_vipp_quarantined(&10));
        assert_eq!(PreTimestampStake::<Test>::get(10), Some((2, 1000, false)));
        assert!(VippBreachYear::<Test>::get(10).is_none());
        assert!(PreTimestampVippBreaches::<Test>::get().contains(&10));
        assert_ok!(EnergyGeneration::force_chill(RuntimeOrigin::root(), 10));
        assert_eq!(PreTimestampStake::<Test>::get(10), Some((2, 1000, false)));
        clock(2026, 1, 1);
        timestamp_hook();
        assert_eq!(Privileges::vipp_points(2025, 10), Some(950));
        assert_eq!(Privileges::vip_points(2025, 10), Some(2));
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_eq!(VippBreachYear::<Test>::get(10), Some(2026));
        assert!(NacManaging::is_vipp_quarantined(&10));
        NacManaging::finish_balance_changes();
        assert!(Privileges::vipp_members(10).is_none());
        assert!(owned_vipp_ids(10).is_empty());
        assert_eq!(Privileges::vipp_points(2025, 10), Some(950));
        assert!(VippBreachYear::<Test>::get(10).is_none());
        System::set_block_number(3);
        clock(2026, 1, 2);
        timestamp_hook();
        assert_eq!(Privileges::vipp_points(2025, 10), Some(950));
        assert!(Privileges::vipp_members(10).is_none());
    });
}

#[test]
fn delayed_last_nft_burn_requires_review_before_removing_only_the_approved_archived_year() {
    ExtBuilder::default().build_and_execute(|| {
        deferred_december_breach(false);
        let nft = owned_vipp_ids(10);
        assert_noop!(
            NacManaging::repair_vipp_balance(RuntimeOrigin::root(), 10),
            Error::<Test>::AccountingPaused
        );
        assert_eq!(owned_vipp_ids(10), nft);
        assert_eq!(Privileges::vipp_points(2025, 10), Some(5000));
        assert!(NacManaging::is_vipp_quarantined(&10));
        assert_ok!(Privileges::approve_vipp_forfeiture(RuntimeOrigin::root(), 10, Some(2025)));
        assert_ok!(NacManaging::repair_vipp_balance(RuntimeOrigin::root(), 10));
        assert!(Privileges::vipp_points(2025, 10).is_none());
        assert_eq!(Privileges::vipp_points(2024, 10), Some(777));
        assert!(owned_vipp_ids(10).is_empty());
        assert!(Privileges::vipp_members(10).is_none());
        assert!(!NacManaging::is_vipp_quarantined(&10));
        assert!(VippBreachYear::<Test>::get(10).is_none());
        assert!(VippForfeitureApproval::<Test>::get(10).is_none());
    });
}

#[test]
fn delayed_partial_nft_burn_preserves_archived_points_without_forfeiture_approval() {
    ExtBuilder::default().build_and_execute(|| {
        deferred_december_breach(true);
        assert_ok!(NacManaging::repair_vipp_balance(RuntimeOrigin::root(), 10));
        assert_eq!(owned_vipp_ids(10).len(), 1);
        assert_eq!(Privileges::vipp_points(2025, 10), Some(5000));
        assert_eq!(Privileges::vipp_points(2024, 10), Some(777));
        assert!(Privileges::vipp_members(10).is_some());
        assert!(VippBreachYear::<Test>::get(10).is_none());
        assert!(!NacManaging::is_vipp_quarantined(&10));
    });
}

#[test]
fn worsening_breach_in_new_year_does_not_infer_forfeiture_of_the_first_breach_year() {
    ExtBuilder::default().build_and_execute(|| {
        deferred_december_breach(true);
        // December's deficit would lose only one NFT. This later deficit loses both.
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 100));
        <NacManaging as pallet_energy_fee::OnWithdrawFeeHandler<AccountId>>::on_withdraw_fee(&10);
        assert_eq!(VippBreachYear::<Test>::get(10), Some(2025));
        assert_noop!(
            NacManaging::repair_vipp_balance(RuntimeOrigin::root(), 10),
            Error::<Test>::AccountingPaused
        );
        assert_eq!(Privileges::vipp_points(2025, 10), Some(5000));
        assert_ok!(Privileges::approve_vipp_forfeiture(RuntimeOrigin::root(), 10, None));
        assert_ok!(NacManaging::repair_vipp_balance(RuntimeOrigin::root(), 10));
        assert_eq!(Privileges::vipp_points(2025, 10), Some(5000));
        assert_eq!(Privileges::vipp_points(2024, 10), Some(777));
        assert!(Privileges::vipp_members(10).is_none());
        assert!(owned_vipp_ids(10).is_empty());
        assert!(VippForfeitureApproval::<Test>::get(10).is_none());
    });
}

#[test]
fn historical_forfeiture_approval_cannot_change_a_funded_year() {
    ExtBuilder::default().build_and_execute(|| {
        deferred_december_breach(false);
        // Model a legacy externally configured pool; ordinary set_rewards rejects quarantine.
        Rewards::<Test>::insert(
            2025,
            RewardsInfo::<Test> {
                vip_points: 100,
                vip_rewards: 200,
                vipp_points: 5000,
                vipp_rewards: 400,
            },
        );
        let pool = Rewards::<Test>::get(2025).unwrap().encode();
        assert_noop!(
            Privileges::approve_vipp_forfeiture(RuntimeOrigin::root(), 10, Some(2025)),
            Error::<Test>::YearRewardsAlreadySet
        );
        assert_eq!(Rewards::<Test>::get(2025).unwrap().encode(), pool);
        assert_eq!(Privileges::vipp_points(2025, 10), Some(5000));
        assert!(VippForfeitureApproval::<Test>::get(10).is_none());
    });
}
