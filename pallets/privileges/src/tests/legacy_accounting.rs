use super::*;

#[test]
fn last_nft_burn_clears_legacy_nac_id_cache_and_stops_ghost_points() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let nac_id = NacManaging::get_nac_level(&10).unwrap().1;
        assert_ne!(owned_vipp_ids(10), vec![nac_id]);
        // Mainnet audit: all 558 cached members referenced at least one non-owned ID.
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
fn partial_burn_rebuilds_legacy_ids_and_caps_without_wiping_points() {
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
        assert_eq!(member.active_vipp_threshold, vec![(retained_id, 243)]);
    });
}

#[test]
fn additional_claim_rebuilds_a_legacy_zero_cap_from_owned_nfts() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        VippMembers::<Test>::mutate(10, |m| {
            let m = m.as_mut().unwrap();
            m.active_vipp_threshold = vec![(0, 0)];
            m.points = 123;
        });
        System::set_block_number(2);
        claim_for_validator(100);
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, 123);
        assert_eq!(member.active_vipp_threshold.iter().map(|(_, cap)| cap).sum::<u64>(), 1045);
        let mut cached_ids: Vec<_> =
            member.active_vipp_threshold.iter().map(|(id, _)| *id).collect();
        cached_ids.sort();
        let mut owned = owned_vipp_ids(10);
        owned.sort();
        assert_eq!(cached_ids, owned);
    });
}

#[test]
fn zero_points_cannot_claim_the_entire_zero_denominator_pool() {
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
fn vipp_only_year_can_be_claimed_without_a_vip_row_and_only_once() {
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
fn failed_reward_transfer_keeps_both_point_records_for_retry() {
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
fn invalid_frozen_pool_total_cannot_pay_or_consume_account_points() {
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
fn finalized_reward_pools_reject_point_corrections_before_and_after_claim() {
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
