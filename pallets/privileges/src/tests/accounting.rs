use super::*;
use crate::accounting as accrual;
use crate::migration::recovery;

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

fn run_approved_job() {
    for _ in 0..1000 {
        let job = accrual::ApprovedCatchUp::<Test>::get().unwrap();
        if !Privileges::check_correct_date(&CurrentDate::<Test>::get(), &job.target) {
            return;
        }
        assert_ok!(accrual::step::<Test>());
    }
    panic!("bounded catch-up did not finish");
}

#[test]
fn long_interruption_freezes_points_and_rejects_membership_and_nft_mutations() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        join_vip_and_vipp(100);
        let member = VippMembers::<Test>::get(10).unwrap().encode();
        clock(2022, 1, 1);
        Privileges::update_quarter_info();
        assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 1));
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), member);
        assert_noop!(
            Privileges::become_vip_status(RuntimeOrigin::signed(20), PenaltyType::Flat),
            Error::<Test>::AccountingPaused
        );
        assert_noop!(
            Privileges::exit_vip(RuntimeOrigin::signed(10)),
            Error::<Test>::AccountingPaused
        );
        assert_noop!(
            Privileges::change_penalty_type(RuntimeOrigin::signed(10), PenaltyType::Declining),
            Error::<Test>::AccountingPaused
        );
        assert_noop!(
            NacManaging::transfer_vipp(RuntimeOrigin::signed(10), owned_vipp_ids(10)[0], 100),
            Error::<Test>::AccountingPaused
        );
        assert_err!(accrual::ensure_ready::<Test>(), Error::<Test>::AccountingPaused);
    });
}

#[test]
fn approved_catchup_freezes_sources_while_live_staking_remains_operable() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        clock(2022, 1, 1);
        assert_ok!(accrual::approve::<Test>(date(2022, 1, 1)));
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 800));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 200);
        run_approved_job();
        assert_eq!(Privileges::vipp_points(2020, 10), Some(366 * 950));
        assert_eq!(Privileges::vipp_points(2021, 10), Some(365 * 950));
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_ok!(accrual::finish::<Test>());
        clock(2022, 1, 2);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 200);
    });
}

#[test]
fn catchup_persists_cursor_and_credits_at_most_64_sources_in_one_step() {
    ExtBuilder::default().build_and_execute(|| {
        for account in 1000..1130 {
            VipMembers::<Test>::insert(
                account,
                VipMemberInfo::<Test> {
                    start: 0,
                    tax_type: PenaltyType::Flat,
                    points: 0,
                    active_stake: 1000,
                },
            );
        }
        clock(2020, 1, 2);
        assert_ok!(accrual::approve::<Test>(date(2020, 1, 2)));
        assert_ok!(accrual::step::<Test>());
        assert_eq!(VipMembers::<Test>::iter().filter(|(_, member)| member.points > 0).count(), 64);
        assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 1));
        let cursor = accrual::ApprovedCatchUp::<Test>::get().unwrap().source_cursor;
        assert_eq!(cursor, 64);
        System::set_block_number(99);
        assert_ok!(accrual::step::<Test>());
        assert_eq!(VipMembers::<Test>::iter().filter(|(_, member)| member.points > 0).count(), 128);
        assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 1));
        run_approved_job();
        for (_, member) in VipMembers::<Test>::iter() {
            assert_eq!(member.points, 20);
        }
        assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 2));
        // Completed steps cannot credit the same interval a second time.
        assert_ok!(accrual::step::<Test>());
        assert_eq!(VipMembers::<Test>::iter().map(|(_, member)| member.points).sum::<u64>(), 2600);
    });
}

#[test]
fn catchup_limits_each_date_window_to_366_days() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        clock(2022, 1, 1);
        assert_ok!(accrual::approve::<Test>(date(2022, 1, 1)));
        assert_ok!(accrual::step::<Test>());
        assert_eq!(CurrentDate::<Test>::get(), date(2021, 1, 1));
        assert_eq!(Privileges::vipp_points(2020, 10), Some(366 * 950));
        assert!(Privileges::vipp_points(2021, 10).is_none());
        assert_err!(accrual::finish::<Test>(), Error::<Test>::AccountingPaused);
        assert_ok!(accrual::step::<Test>());
        assert_eq!(CurrentDate::<Test>::get(), date(2022, 1, 1));
        assert_eq!(Privileges::vipp_points(2021, 10), Some(365 * 950));
    });
}

#[test]
fn catchup_cannot_replace_a_partially_processed_job() {
    ExtBuilder::default().build_and_execute(|| {
        for account in 1000..1070 {
            VipMembers::<Test>::insert(
                account,
                VipMemberInfo::<Test> {
                    start: 0,
                    tax_type: PenaltyType::Flat,
                    points: 0,
                    active_stake: 1000,
                },
            );
        }
        clock(2020, 1, 3);
        assert_ok!(accrual::approve::<Test>(date(2020, 1, 2)));
        assert_ok!(accrual::step::<Test>());
        let job = accrual::ApprovedCatchUp::<Test>::get().unwrap().encode();
        assert_noop!(accrual::approve::<Test>(date(2020, 1, 3)), Error::<Test>::AccountingPaused);
        assert_eq!(accrual::ApprovedCatchUp::<Test>::get().unwrap().encode(), job);
    });
}

#[test]
fn catchup_cannot_overwrite_a_funded_year_or_existing_annual_points() {
    for funded in [false, true] {
        ExtBuilder::default().build_and_execute(|| {
            join_vip_and_vipp(10);
            clock(2021, 1, 1);
            if funded {
                Rewards::<Test>::insert(
                    2020,
                    RewardsInfo::<Test> {
                        vip_points: 100,
                        vip_rewards: 200,
                        vipp_points: 300,
                        vipp_rewards: 400,
                    },
                );
            } else {
                VippPoints::<Test>::insert(2020, 10, 999);
            }
            assert_ok!(accrual::approve::<Test>(date(2021, 1, 1)));
            let expected = if funded {
                Error::<Test>::YearRewardsAlreadySet
            } else {
                Error::<Test>::HistoricalPointsAlreadyExist
            };
            assert_noop!(accrual::step::<Test>(), expected);
            assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 1));
            assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
            assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
            assert!(VipPoints::<Test>::get(2020, 10).is_none());
            if !funded {
                assert_eq!(VippPoints::<Test>::get(2020, 10), Some(999));
            }
        });
    }
}

#[test]
fn catchup_requires_new_approval_for_days_elapsed_during_review() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        clock(2020, 1, 2);
        assert_ok!(accrual::approve::<Test>(date(2020, 1, 2)));
        run_approved_job();
        clock(2020, 1, 3);
        assert_err!(accrual::finish::<Test>(), Error::<Test>::AccountingPaused);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
        assert_ok!(accrual::approve::<Test>(date(2020, 1, 3)));
        run_approved_job();
        assert_ok!(accrual::finish::<Test>());
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 1900);
    });
}

#[test]
fn catchup_rejects_future_target_without_creating_a_job() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        assert_noop!(accrual::approve::<Test>(date(2020, 1, 2)), Error::<Test>::NotCorrectDate);
        assert!(!accrual::ApprovedCatchUp::<Test>::exists());
    });
}

#[test]
fn accounting_population_and_cache_bounds_fail_without_partial_points() {
    for oversized_cache in [false, true] {
        ExtBuilder::default().build_and_execute(|| {
            join_vip_and_vipp(10);
            if oversized_cache {
                VippMembers::<Test>::mutate(10, |m| {
                    m.as_mut().unwrap().active_vipp_threshold = vec![(0, 1); 65]
                });
            } else {
                for account in 1000..2024 {
                    VipMembers::<Test>::insert(
                        account,
                        VipMemberInfo::<Test> {
                            start: 0,
                            tax_type: PenaltyType::Flat,
                            points: 0,
                            active_stake: 1000,
                        },
                    );
                }
            }
            clock(2020, 1, 2);
            assert_noop!(
                accrual::approve::<Test>(date(2020, 1, 2)),
                Error::<Test>::AccountingCapacityExceeded
            );
            assert!(!accrual::ApprovedCatchUp::<Test>::exists());
            assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
            assert_eq!(CurrentDate::<Test>::get(), date(2020, 1, 1));
        });
    }
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
fn cache_recovery_does_not_move_old_date_or_automatically_approve_repaired_sources() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let old_date = CurrentDate::<Test>::get();
        assert!(recovery::begin::<Test>());
        accrual::AccrualReviewRequired::<Test>::put(true);
        clock(2020, 1, 2);
        Privileges::update_quarter_info();
        assert_eq!(CurrentDate::<Test>::get(), old_date);
        assert_err!(accrual::approve::<Test>(date(2020, 1, 2)), Error::<Test>::AccountingPaused);
        for _ in 0..10 {
            if recovery::step::<Test>(16, 64).unwrap().phase == recovery::Phase::AwaitingReview {
                break;
            }
        }
        assert_ok!(recovery::finish::<Test>());
        assert!(accrual::paused::<Test>());
        Privileges::update_quarter_info();
        assert_eq!(CurrentDate::<Test>::get(), old_date);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_ok!(accrual::approve::<Test>(date(2020, 1, 2)));
        run_approved_job();
        assert_ok!(accrual::finish::<Test>());
        assert!(!accrual::paused::<Test>());
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
    });
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
fn catchup_annual_conflict_preserves_prior_chunks_and_retries_only_unfinished_sources() {
    ExtBuilder::default().build_and_execute(|| {
        for account in 1000..1070 {
            VipMembers::<Test>::insert(
                account,
                VipMemberInfo::<Test> {
                    start: 0,
                    tax_type: PenaltyType::Flat,
                    points: 0,
                    active_stake: 1000,
                },
            );
        }
        clock(2021, 1, 1);
        assert_ok!(accrual::approve::<Test>(date(2021, 1, 1)));
        let job = accrual::ApprovedCatchUp::<Test>::get().unwrap();
        let first = job.vip[0].0;
        let conflicting = job.vip[65].0;
        VipPoints::<Test>::insert(2020, conflicting, 999);
        assert_ok!(accrual::step::<Test>());
        let first_saved = VipPoints::<Test>::get(2020, first).unwrap();
        assert_eq!(accrual::ApprovedCatchUp::<Test>::get().unwrap().source_cursor, 64);
        assert_noop!(accrual::step::<Test>(), Error::<Test>::HistoricalPointsAlreadyExist);
        assert_eq!(accrual::ApprovedCatchUp::<Test>::get().unwrap().source_cursor, 64);
        assert_eq!(VipPoints::<Test>::get(2020, first), Some(first_saved));
        assert_eq!(VipPoints::<Test>::get(2020, conflicting), Some(999));
        // Model an explicitly approved correction of the conflicting fixture row.
        VipPoints::<Test>::remove(2020, conflicting);
        assert_ok!(accrual::step::<Test>());
        assert_eq!(VipPoints::<Test>::get(2020, first), Some(first_saved));
        assert_eq!(VipPoints::<Test>::get(2020, conflicting), Some(first_saved));
        assert_eq!(CurrentDate::<Test>::get(), date(2021, 1, 1));
    });
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
fn maximum_population_catchup_step_is_limited_to_64_sources_for_365_days() {
    ExtBuilder::default().build_and_execute(|| {
        populate_maximum_cached_population();
        CurrentDate::<Test>::put(date(2021, 1, 1));
        clock(2022, 1, 1);
        assert_ok!(accrual::approve::<Test>(date(2022, 1, 1)));
        let started = std::time::Instant::now();
        assert_ok!(accrual::step::<Test>());
        let elapsed = started.elapsed();
        assert_eq!(CurrentDate::<Test>::get(), date(2021, 1, 1));
        assert_eq!(VipPoints::<Test>::iter_prefix(2021).count(), 64);
        assert_eq!(VippPoints::<Test>::iter_prefix(2021).count(), 0);
        assert_eq!(accrual::ApprovedCatchUp::<Test>::get().unwrap().source_cursor, 64);
        assert!(VippMembers::<Test>::iter().all(|(_, member)| member.points == 0));
        println!("native_test_timing scenario=approved_64_of_2048_sources_365_days elapsed_us={} (unit-test host timing, not runtime benchmark)", elapsed.as_micros());
    });
}

#[test]
fn runtime_upgrade_installs_schema_but_preserves_state_and_pauses_until_reviewed_recovery() {
    use frame_support::traits::{Hooks, StorageVersion};
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        VipMembers::<Test>::mutate(10, |member| {
            let member = member.as_mut().unwrap();
            member.points = 123;
            member.active_stake = 0;
        });
        VippMembers::<Test>::mutate(10, |member| member.as_mut().unwrap().points = 456);
        VipPoints::<Test>::insert(2019, 10, 789);
        let original_vip = VipMembers::<Test>::get(10).unwrap().encode();
        let original_vipp = VippMembers::<Test>::get(10).unwrap().encode();
        let original_date = CurrentDate::<Test>::get();
        StorageVersion::new(1).put::<Privileges>();

        <Privileges as Hooks<u64>>::on_runtime_upgrade();
        let initial = recovery::CacheRecoveryProgress::<Test>::get().unwrap();
        assert_eq!(initial.scanned_rows, 0);
        assert!(initial.cursor.is_empty());
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(2));
        assert_eq!(VipMembers::<Test>::get(10).unwrap().encode(), original_vip);
        assert_eq!(VippMembers::<Test>::get(10).unwrap().encode(), original_vipp);
        assert_eq!(CurrentDate::<Test>::get(), original_date);
        assert!(accrual::AccrualReviewRequired::<Test>::get());
        assert_noop!(
            Privileges::exit_vip(RuntimeOrigin::signed(10)),
            Error::<Test>::AccountingPaused
        );

        assert_ok!(Privileges::continue_cache_recovery(RuntimeOrigin::root(), 1));
        let progress = recovery::CacheRecoveryProgress::<Test>::get().unwrap();
        let review = recovery::CacheRecoveryReviews::<Test>::get(10).unwrap().encode();
        assert_eq!(progress.scanned_rows, 1);
        <Privileges as Hooks<u64>>::on_runtime_upgrade();
        assert_eq!(recovery::CacheRecoveryProgress::<Test>::get().unwrap(), progress);
        assert_eq!(recovery::CacheRecoveryReviews::<Test>::get(10).unwrap().encode(), review);
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(2));

        assert_ok!(Privileges::continue_cache_recovery(RuntimeOrigin::root(), 16));
        assert_noop!(
            Privileges::finish_cache_recovery(RuntimeOrigin::root()),
            sp_runtime::DispatchError::Other("Recovery still requires review")
        );
        assert_ok!(Privileges::acknowledge_cache_recovery(
            RuntimeOrigin::root(),
            vec![10].try_into().unwrap()
        ));
        assert_ok!(Privileges::finish_cache_recovery(RuntimeOrigin::root()));
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(2));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 123);
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 1000);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 456);
        assert_eq!(Privileges::vip_points(2019, 10), Some(789));
        assert_eq!(CurrentDate::<Test>::get(), original_date);
        assert!(accrual::paused::<Test>());
        assert_ok!(Privileges::finish_catch_up(RuntimeOrigin::root()));
        <Privileges as Hooks<u64>>::on_runtime_upgrade();
        assert!(!accrual::paused::<Test>());
    });
}

#[test]
fn runtime_upgrade_does_not_certify_an_undecodable_recovery_guard() {
    use frame_support::traits::{Hooks, StorageVersion};
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let before = VipMembers::<Test>::get(10).unwrap().encode();
        StorageVersion::new(1).put::<Privileges>();
        sp_io::storage::set(&recovery::CacheRecoveryProgress::<Test>::hashed_key(), &[255]);

        <Privileges as Hooks<u64>>::on_runtime_upgrade();

        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(1));
        assert!(accrual::AccrualReviewRequired::<Test>::get());
        assert_noop!(
            Privileges::exit_vip(RuntimeOrigin::signed(10)),
            Error::<Test>::AccountingPaused
        );
        assert_eq!(VipMembers::<Test>::get(10).unwrap().encode(), before);
        assert!(Privileges::finish_cache_recovery(RuntimeOrigin::root()).is_err());
    });
}

#[test]
fn maintenance_wrappers_require_root_and_reject_unbounded_or_unreviewed_work() {
    use sp_runtime::DispatchError;
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        assert_noop!(
            Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 9),
            Error::<Test>::NotCorrectDate
        );
        assert_noop!(
            Privileges::begin_cache_recovery(RuntimeOrigin::signed(10)),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Privileges::continue_cache_recovery(RuntimeOrigin::signed(10), 16),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Privileges::retry_cache_recovery(RuntimeOrigin::signed(10), 10),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Privileges::acknowledge_cache_recovery(
                RuntimeOrigin::signed(10),
                vec![10].try_into().unwrap()
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Privileges::finish_cache_recovery(RuntimeOrigin::signed(10)),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Privileges::approve_catch_up(RuntimeOrigin::signed(10), 2020, 1, 2),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Privileges::continue_catch_up(RuntimeOrigin::signed(10)),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Privileges::finish_catch_up(RuntimeOrigin::signed(10)),
            DispatchError::BadOrigin
        );
        assert_ok!(Privileges::begin_cache_recovery(RuntimeOrigin::root()));
        assert_noop!(
            Privileges::continue_cache_recovery(RuntimeOrigin::root(), 17),
            DispatchError::Other("Invalid recovery batch limits")
        );
        assert_noop!(
            Privileges::continue_cache_recovery(RuntimeOrigin::root(), 0),
            DispatchError::Other("Invalid recovery batch limits")
        );
        assert_eq!(recovery::CacheRecoveryProgress::<Test>::get().unwrap().scanned_rows, 0);
        assert_ok!(Privileges::continue_cache_recovery(RuntimeOrigin::root(), 16));
        assert_eq!(recovery::CacheRecoveryProgress::<Test>::get().unwrap().scanned_rows, 2);
    });
}

fn timestamp_hook() {
    <Privileges as frame_support::traits::OnTimestampSet<u64>>::on_timestamp_set(Timestamp::get());
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

#[test]
fn explicitly_approved_sources_use_live_stake_instead_of_initialization_snapshot() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        System::set_block_number(1);
        timestamp_hook();
        System::set_block_number(2);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 300));
        assert_eq!(PreTimestampStake::<Test>::get(10), Some((2, 1000, false)));
        clock(2020, 1, 2);
        // Approvals deliberately freeze the reviewed live source, even if a shadow exists.
        assert_ok!(accrual::approve::<Test>(date(2020, 1, 2)));
        let job = accrual::ApprovedCatchUp::<Test>::get().unwrap();
        assert_eq!(job.vip.as_slice(), &[(10, 700)]);
        assert_eq!(job.vipp.as_slice(), &[(10, 700)]);
    });
}

#[test]
fn initialization_balance_breach_preserves_closed_year_before_finalization_burn() {
    ExtBuilder::default().build_and_execute(|| {
        ObserveVippBalances::set(true);
        CurrentDate::<Test>::put(date(2025, 12, 31));
        clock(2025, 12, 31);
        join_vip_and_vipp(10);
        System::set_block_number(1);
        timestamp_hook();
        System::set_block_number(2);
        // Simulate an initialization slash: native balance changes before staking callbacks.
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 100));
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

fn deferred_december_breach(two_nfts: bool) {
    ObserveVippBalances::set(true);
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
