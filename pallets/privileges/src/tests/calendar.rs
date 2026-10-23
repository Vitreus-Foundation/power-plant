use super::*;

fn set_clock(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) {
    let millis = NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_opt(hour, minute, second)
        .unwrap()
        .and_utc()
        .timestamp_millis() as u64;
    Timestamp::set_timestamp(millis);
}

fn start_on(year: i32, month: u32, day: u32) {
    CurrentDate::<Test>::put(CurrentDateInfo::new::<Test>(year, month, day).unwrap());
    set_clock(year, month, day, 12, 0, 0);
    join_vip_and_vipp(10);
}

#[test]
fn rollover_includes_december_31_before_saving_points() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 12, 31);
        VipMembers::<Test>::mutate(10, |m| m.as_mut().unwrap().points = 1234);
        VippMembers::<Test>::mutate(10, |m| m.as_mut().unwrap().points = 5678);
        // Audit: blocks 8936542 -> 8936543, 2026-01-01 01:25:24.001 UTC.
        // Amounts are normalized; the historical trigger and state transition are preserved.
        set_clock(2026, 1, 1, 1, 25, 24);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vip_points(2025, 10), Some(1236));
        assert_eq!(Privileges::vipp_points(2025, 10), Some(5678 + 950));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
    });
}

#[test]
fn complete_year_has_365_or_366_vipp_accruals() {
    for (year, days) in [(2024, 366), (2025, 365)] {
        ExtBuilder::default().build_and_execute(|| {
            start_on(year, 1, 1);
            assert_ok!(advance_test_date(RuntimeOrigin::root(), year + 1, 1, 1));
            assert_eq!(Privileges::vipp_points(year as u32, 10), Some(days * 950));
            assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        });
    }
}

#[test]
fn automatic_update_catches_up_all_days_across_february() {
    for (year, days) in [(2024, 2), (2025, 1)] {
        ExtBuilder::default().build_and_execute(|| {
            start_on(year, 2, 28);
            set_clock(year, 3, 1, 0, 0, 0);
            Privileges::update_quarter_info();
            assert_eq!(Privileges::vipp_members(10).unwrap().points, days * 950);
        });
    }
}

#[test]
fn automatic_update_crossing_january_1_splits_points_between_years() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 12, 30);
        set_clock(2026, 1, 2, 0, 0, 0);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vip_points(2025, 10), Some(4));
        assert_eq!(Privileges::vipp_points(2025, 10), Some(1900));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 20);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
    });
}

#[test]
fn same_day_in_a_new_year_requires_review_instead_of_reusing_old_stakes() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 1, 1);
        set_clock(2026, 1, 1, 0, 0, 0);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::current_date().current_year, 2025);
        assert_eq!(Privileges::vipp_points(2025, 10), None);
    });
}

#[test]
fn repeated_and_backwards_clock_updates_cannot_credit_again_or_reopen_a_year() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 12, 31);
        set_clock(2026, 1, 1, 0, 0, 0);
        Privileges::update_quarter_info();
        let vip = Privileges::vip_points(2025, 10);
        let vipp = Privileges::vipp_points(2025, 10);
        Privileges::update_quarter_info();
        set_clock(2025, 12, 31, 23, 59, 59);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::current_date(), CurrentDateInfo::new::<Test>(2026, 1, 1).unwrap());
        set_clock(2026, 1, 1, 1, 0, 0);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vip_points(2025, 10), vip);
        assert_eq!(Privileges::vipp_points(2025, 10), vipp);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
    });
}

#[test]
fn manual_update_rejects_a_gap_over_the_processing_limit_atomically() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 1, 1);
        assert_noop!(
            Privileges::update_time(RuntimeOrigin::root(), 2027, 1, 1),
            Error::<Test>::NotCorrectDate
        );
    });
}

#[test]
fn automatic_long_gap_does_not_partially_credit_from_mutable_current_stakes() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 1, 1);
        set_clock(2027, 1, 1, 0, 0, 0);
        Privileges::update_quarter_info();
        Privileges::update_quarter_info();
        assert_eq!(Privileges::current_date(), CurrentDateInfo::new::<Test>(2025, 1, 1).unwrap());
        assert_eq!(Privileges::vipp_points(2025, 10), None);
        assert_eq!(Privileges::vipp_points(2026, 10), None);
    });
}

#[test]
fn joining_after_midnight_does_not_receive_the_previous_day() {
    ExtBuilder::default().build_and_execute(|| {
        CurrentDate::<Test>::put(CurrentDateInfo::new::<Test>(2025, 3, 4).unwrap());
        set_clock(2025, 3, 5, 0, 1, 0);
        join_vip_and_vipp(10);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        set_clock(2025, 3, 6, 0, 0, 0);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
    });
}

#[test]
fn stake_change_after_midnight_settles_the_previous_day_with_the_old_stake() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 1, 1);
        set_clock(2025, 1, 2, 0, 1, 0);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 800));
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 200);
        set_clock(2025, 1, 3, 0, 0, 0);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 1150);
    });
}

#[test]
fn exiting_on_january_1_preserves_the_finished_year_and_clears_the_current_year() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 12, 31);
        set_clock(2026, 1, 1, 0, 1, 0);
        let balance = Balances::balance(&10);
        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert_eq!(Balances::balance(&10), balance);
        assert_eq!(Privileges::vip_points(2025, 10), Some(2));
        assert_eq!(Privileges::vipp_points(2025, 10), Some(950));
        assert!(Privileges::vip_members(10).is_none());
        assert!(Privileges::vipp_members(10).is_none());
    });
}

#[test]
fn first_february_unbond_uses_the_new_month_before_the_next_era() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 1, 31);
        set_clock(2025, 2, 1, 0, 1, 0);
        let balance = Balances::balance(&10);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 400));
        assert_eq!(Balances::balance(&10), balance - 70);
    });
}

#[test]
fn additional_claim_after_midnight_does_not_increase_the_previous_days_cap() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 1, 1);
        set_clock(2025, 1, 2, 0, 1, 0);
        System::set_block_number(2);
        claim_for_validator(100);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
        set_clock(2025, 1, 3, 0, 0, 0);
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 1950);
    });
}

#[test]
fn last_nft_burn_on_january_1_preserves_the_previous_year() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 12, 31);
        set_clock(2026, 1, 1, 0, 1, 0);
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 100));
        NacManaging::check_account_threshold(&10);
        assert_eq!(Privileges::vipp_points(2025, 10), Some(950));
        assert!(Privileges::vipp_members(10).is_none());
        assert!(Privileges::vip_members(10).is_some());
    });
}

#[test]
fn last_second_entry_earns_its_day_but_midnight_entry_does_not_earn_yesterday() {
    for (day, hour, minute, second, expected) in [(1, 23, 59, 59, 950), (2, 0, 0, 0, 0)] {
        ExtBuilder::default().build_and_execute(|| {
            CurrentDate::<Test>::put(CurrentDateInfo::new::<Test>(2025, 3, 1).unwrap());
            set_clock(2025, 3, day, hour, minute, second);
            join_vip_and_vipp(10);
            set_clock(2025, 3, 2, 0, 0, 0);
            Privileges::update_quarter_info();
            assert_eq!(Privileges::vipp_members(10).unwrap().points, expected);
        });
    }
}

#[test]
fn multiple_stake_changes_in_one_day_use_the_final_stake_once() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 1, 1);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 400));
        assert_ok!(EnergyGeneration::bond_extra(RuntimeOrigin::signed(10), 100));
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        set_clock(2025, 1, 2, 0, 0, 0);
        Privileges::update_quarter_info();
        Privileges::update_quarter_info();
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 700);
    });
}

#[test]
fn calendar_fix_preserves_the_existing_daily_vip_denominator() {
    ExtBuilder::default().balance_factor(1000).build_and_execute(|| {
        start_on(2025, 1, 1);
        set_clock(2025, 1, 2, 0, 0, 0);
        Privileges::update_quarter_info();
        // 1,000,000 / 51 rounds to 19,608. Using 50 would silently change the formula.
        assert_eq!(Privileges::vip_members(10).unwrap().points, 19608);
    });
}

#[test]
fn declining_penalty_tracks_utc_quarters_without_waiting_for_an_era() {
    for (month, tax) in [(4, 80), (7, 60), (10, 40)] {
        ExtBuilder::default().build_and_execute(|| {
            start_on(2025, 1, 1);
            assert_ok!(Privileges::change_penalty_type(
                RuntimeOrigin::signed(10),
                PenaltyType::Declining
            ));
            set_clock(2025, month, 1, 0, 1, 0);
            let balance = Balances::balance(&10);
            assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 400));
            assert_eq!(Balances::balance(&10), balance - tax);
        });
    }
}

#[test]
fn setting_rewards_on_january_1_includes_the_completed_year() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 12, 31);
        set_clock(2026, 1, 1, 0, 1, 0);
        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), 2025, 1000, 2000));
        let rewards = Privileges::rewards(2025).unwrap();
        assert_eq!(rewards.vip_points, 2);
        assert_eq!(rewards.vipp_points, 950);
    });
}

#[test]
fn january_1_point_correction_targets_the_previous_year_instead_of_current_members() {
    ExtBuilder::default().build_and_execute(|| {
        start_on(2025, 12, 31);
        set_clock(2026, 1, 1, 0, 1, 0);
        assert_ok!(Privileges::force_set_vip_points(RuntimeOrigin::root(), 2025, 10, 123));
        assert_ok!(Privileges::force_set_vipp_points(RuntimeOrigin::root(), 2025, 10, 456));
        assert_eq!(Privileges::vip_points(2025, 10), Some(123));
        assert_eq!(Privileges::vipp_points(2025, 10), Some(456));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
    });
}
