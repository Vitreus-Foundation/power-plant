use super::*;
use frame_support::traits::Currency;
use pallet_nac_managing::{VippBreachBalance, VippReconciliationsThisBlock};

fn prepare_cooperator() {
    join_vip_and_vipp(100);
    assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 2));
    // A balance breach is dated by UTC, independently of the accounting anchor.
    Timestamp::set_timestamp(
        NaiveDate::from_ymd_opt(2020, 1, 2)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis() as u64,
    );
    VipPoints::<Test>::insert(2019, 100, 111);
    VippPoints::<Test>::insert(2019, 100, 777);
}

fn reduce_eligible_balance_to_949() {
    let balance = <Balances as Currency<AccountId>>::total_balance(&100);
    assert_ok!(Balances::transfer_allow_death(RuntimeOrigin::signed(100), 101, balance - 949));
    <NacManaging as pallet_energy_fee::OnWithdrawFeeHandler<AccountId>>::on_withdraw_fee(&100);
}

#[test]
fn fee_check_after_transfer_updates_real_privileges_cache_and_preserves_previous_year() {
    ExtBuilder::default().build_and_execute(|| {
        prepare_cooperator();
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 500);
        let vip = Privileges::vip_members(100).unwrap();
        reduce_eligible_balance_to_949();
        assert!(!owned_vipp_ids(100).is_empty());
        NacManaging::finish_balance_changes();
        assert!(owned_vipp_ids(100).is_empty());
        assert!(Privileges::vipp_members(100).is_none());
        assert_eq!(Privileges::vip_members(100).unwrap(), vip);
        assert_eq!(Privileges::vip_points(2019, 100), Some(111));
        assert_eq!(Privileges::vipp_points(2019, 100), Some(777));
        assert!(NacManaging::is_vipp_inactive(&100));
        assert!(!NacManaging::is_vipp_quarantined(&100));
    });
}

#[test]
fn quarantine_pauses_new_vipp_points_and_new_pools_but_keeps_funded_previous_year_claimable() {
    ExtBuilder::default().build_and_execute(|| {
        prepare_cooperator();
        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), 2019, 1_000, 2_000));
        VippReconciliationsThisBlock::<Test>::put::<u32>(
            <Test as pallet_nac_managing::Config>::MaxVippReconciliationsPerBlock::get(),
        );
        reduce_eligible_balance_to_949();
        let points = Privileges::vipp_members(100).unwrap().points;
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 3));
        assert_eq!(Privileges::vipp_members(100).unwrap().points, points);
        assert_noop!(
            Privileges::set_rewards(RuntimeOrigin::root(), 2018, 10, 10),
            Error::<Test>::AccountingPaused
        );
        let before = <Balances as Currency<AccountId>>::total_balance(&100);
        assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(100), 2019));
        assert_eq!(<Balances as Currency<AccountId>>::total_balance(&100), before + 3_000);
        assert_eq!(VippBreachBalance::<Test>::get(100), Some(949));
        assert!(NacManaging::is_vipp_quarantined(&100));
    });
}
