use super::*;

#[test]
fn last_nft_burn_clears_legacy_nac_id_cache_and_stops_ghost_points() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nac_id = NacManaging::get_nac_level(&10).unwrap().1;
        assert_ne!(owned_vipp_ids(10), vec![nac_id]);
        VippMembers::<Test>::mutate(10, |m| {
            let m = m.as_mut().unwrap();
            m.active_vipp_threshold = vec![(nac_id, 950)];
            m.points = 12345;
        });
        VippPoints::<Test>::insert(2019, 10, 456);
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 100));
        NacManaging::check_account_threshold(&10);
        assert!(owned_vipp_ids(10).is_empty());
        assert!(Privileges::vipp_members(10).is_none());
        assert_eq!(Privileges::vipp_points(2019, 10), Some(456));
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 2));
        assert!(Privileges::vipp_members(10).is_none());
    });
}

#[test]
fn test_partial_burn_with_unregistered_nft() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(256);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        let retained_id = owned_vipp_ids(10)[0];
        System::set_block_number(2);
        claim_for_validator(1);
        let nac_id = NacManaging::get_nac_level(&10).unwrap().1;
        VippMembers::<Test>::mutate(10, |m| {
            let m = m.as_mut().unwrap();
            m.active_vipp_threshold = vec![(nac_id, 243), (nac_id, 1)];
            m.points = 5678;
        });
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 243));
        NacManaging::check_account_threshold(&10);
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, 5678);
        assert!(member.active_vipp_threshold.is_empty());
        assert_eq!(owned_vipp_ids(10), vec![retained_id]);
    });
}

#[test]
fn test_additional_claim_with_unregistered_nft() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let unregistered = owned_vipp_ids(10)[0];
        VippMembers::<Test>::mutate(10, |m| {
            let m = m.as_mut().unwrap();
            m.active_vipp_threshold = vec![(0, 0)];
            m.points = 123;
        });
        System::set_block_number(2);
        claim_for_validator(100);
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, 123);
        assert_eq!(member.active_vipp_threshold.len(), 1);
        assert_eq!(member.active_vipp_threshold[0].1, 95);
        assert_ne!(member.active_vipp_threshold[0].0, unregistered);
        assert_eq!(owned_vipp_ids(10).len(), 2);
    });
}

#[test]
fn claim_rewards_with_zero_points_works() {
    ExtBuilder::default().build_and_execute(|| {
        VipPoints::<Test>::insert(2019, 10, 0);
        VippPoints::<Test>::insert(2019, 10, 0);
        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), 2019, 1000, 2000));
        let balance = Balances::balance(&10);
        assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019));
        assert_eq!(Balances::balance(&10), balance);
        assert_noop!(
            Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019),
            Error::<Test>::NoRewardsForAccount
        );
    });
}

#[test]
fn claim_rewards_with_vipp_points_only_works() {
    ExtBuilder::default().build_and_execute(|| {
        // The v1 migration omits zero VIP rows. Corrections can also create VIPP-only rows.
        VippPoints::<Test>::insert(2019, 10, 100);
        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), 2019, 0, 2000));
        let balance = Balances::balance(&10);
        assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019));
        assert_eq!(Balances::balance(&10), balance + 2000);
        assert_noop!(
            Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019),
            Error::<Test>::NoRewardsForAccount
        );
    });
}

#[test]
fn claim_rewards_after_failed_transfer_works() {
    ExtBuilder::default().build_and_execute(|| {
        VipPoints::<Test>::insert(2019, 10, 100);
        VippPoints::<Test>::insert(2019, 10, 200);
        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), 2019, 1000, 2000));
        assert_ok!(Balances::force_set_balance(
            RuntimeOrigin::root(),
            Privileges::account_id(),
            10
        ));
        assert_noop!(
            Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019),
            Error::<Test>::NotEnoughTokensForClaim
        );
        assert_eq!(Privileges::vip_points(2019, 10), Some(100));
        assert_eq!(Privileges::vipp_points(2019, 10), Some(200));
        assert_ok!(Balances::force_set_balance(
            RuntimeOrigin::root(),
            Privileges::account_id(),
            3010
        ));
        assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019));
    });
}

#[test]
fn cannot_claim_rewards_with_invalid_total() {
    for total in [0, 50] {
        ExtBuilder::default().build_and_execute(|| {
            VipPoints::<Test>::insert(2019, 10, 100);
            Rewards::<Test>::insert(
                2019,
                RewardsInfo::<Test> {
                    vip_points: total,
                    vip_rewards: 1000,
                    vipp_points: 0,
                    vipp_rewards: 0,
                },
            );
            assert_noop!(
                Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019),
                Error::<Test>::InvalidRewardPoints
            );
        });
    }
}

#[test]
fn cannot_change_points_after_setting_rewards() {
    ExtBuilder::default().build_and_execute(|| {
        VipPoints::<Test>::insert(2019, 10, 100);
        VippPoints::<Test>::insert(2019, 10, 200);
        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), 2019, 1000, 2000));
        for claimed in [false, true] {
            if claimed {
                assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(10), 2019));
            }
            assert_noop!(
                Privileges::force_set_vip_points(RuntimeOrigin::root(), 2019, 10, 500),
                Error::<Test>::YearRewardsAlreadySet
            );
            assert_noop!(
                Privileges::force_set_vipp_points(RuntimeOrigin::root(), 2019, 10, 500),
                Error::<Test>::YearRewardsAlreadySet
            );
        }
    });
}

#[test]
fn summing_legacy_caps_cannot_overflow_and_exceed_active_stake() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        VippMembers::<Test>::mutate(10, |m| {
            m.as_mut().unwrap().active_vipp_threshold = vec![(123, u64::MAX), (456, 1)];
        });
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 2));
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 1000);
    });
}
