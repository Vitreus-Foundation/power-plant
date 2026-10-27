use super::*;
use crate::migration::recovery;
use frame_support::traits::Currency;
use pallet_nac_managing::{VippBreachBalance, VippReconciliationsThisBlock, VippRepairInProgress};

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
fn reviewed_maintenance_repair_replays_captured_deficit_after_refill_and_unblocks_recovery() {
    ExtBuilder::default().build_and_execute(|| {
        prepare_cooperator();
        let nft = owned_vipp_ids(100)[0];
        VippReconciliationsThisBlock::<Test>::put::<u32>(
            <Test as pallet_nac_managing::Config>::MaxVippReconciliationsPerBlock::get(),
        );
        reduce_eligible_balance_to_949();
        assert!(NacManaging::is_vipp_quarantined(&100));
        assert_eq!(VippBreachBalance::<Test>::get(100), Some(949));
        assert_eq!(owned_vipp_ids(100), vec![nft]);
        assert_ok!(Balances::transfer_allow_death(RuntimeOrigin::signed(101), 100, 100));
        assert_eq!(NacManaging::eligible_vipp_balance(&100), 1_049);
        assert_eq!(VippBreachBalance::<Test>::get(100), Some(949));
        assert!(recovery::begin::<Test>());
        for _ in 0..10 {
            if recovery::step::<Test>(2, 64).unwrap().phase == recovery::Phase::AwaitingReview {
                break;
            }
        }
        assert_eq!(
            recovery::CacheRecoveryProgress::<Test>::get().unwrap().phase,
            recovery::Phase::AwaitingReview
        );
        assert_noop!(NacManaging::mint_vipp_nft(&100), Error::<Test>::AccountingPaused,);
        let date = Privileges::current_date();
        assert_ok!(NacManaging::repair_vipp_balance(RuntimeOrigin::root(), 100));
        assert!(owned_vipp_ids(100).is_empty());
        assert!(Privileges::vipp_members(100).is_none());
        assert_eq!(Privileges::current_date(), date);
        assert_eq!(Privileges::vip_points(2019, 100), Some(111));
        assert_eq!(Privileges::vipp_points(2019, 100), Some(777));
        assert!(!NacManaging::is_vipp_quarantined(&100));
        assert!(!VippRepairInProgress::<Test>::get());
        assert_ok!(recovery::retry_account::<Test>(&100, 64));
        assert_ok!(recovery::acknowledge::<Test>(&[100]));
        assert_ok!(recovery::finish::<Test>());
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
