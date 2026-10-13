use super::*;

#[test]
fn validator_removal_updates_many_delegators_without_erasing_points_or_double_subtraction() {
    ExtBuilder::default().build_and_execute(|| {
        for account in 200..264 {
            assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), account, 2000));
            assert_ok!(EnergyGeneration::bond(
                RuntimeOrigin::signed(account),
                account,
                500,
                pallet_energy_generation::RewardDestination::Stash
            ));
            assert_ok!(EnergyGeneration::cooperate(
                RuntimeOrigin::signed(account),
                vec![(10, 200), (20, 300)]
            ));
            assert_ok!(Privileges::become_vip_status(
                RuntimeOrigin::signed(account),
                PenaltyType::Flat
            ));
        }
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));
        EnergyGeneration::do_remove_validator_from_cooperators_target(&10);
        assert!(EnergyGeneration::do_remove_validator(&10));
        // Repeating either removal cannot subtract the same delegated stake again.
        EnergyGeneration::do_remove_validator_from_cooperators_target(&10);
        assert!(!EnergyGeneration::do_remove_validator(&10));
        for account in 200..264 {
            let member = Privileges::vip_members(account).unwrap();
            assert_eq!(member.active_stake, 300);
            assert_eq!(member.points, 10);
            assert_eq!(member.active_stake, EnergyGeneration::get_active_stake(&account));
        }
    });
}

#[test]
fn removing_validator_collaboration_preserves_validator_solo_stake_despite_stale_links() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(20), PenaltyType::Flat));
        // A legacy collaboration link must not reduce a registered validator's
        // solo stake, even when an old cooperator record is still present.
        pallet_energy_generation::Cooperators::<Test>::insert(
            20,
            pallet_energy_generation::Cooperations::<Test> {
                targets: [(10, 333)]
                    .into_iter()
                    .collect::<std::collections::BTreeMap<_, _>>()
                    .try_into()
                    .unwrap(),
                submitted_in: 0,
                suppressed: false,
            },
        );
        pallet_energy_generation::Collaborations::<Test>::mutate(10, |members| {
            members.as_mut().unwrap().try_insert(20).unwrap();
        });

        assert!(EnergyGeneration::do_remove_validator(&10));

        assert_eq!(Privileges::vip_members(20).unwrap().active_stake, 500);
        assert_eq!(EnergyGeneration::get_active_stake(&20), 500);
    });
}

#[test]
fn removing_one_validator_preserves_inactive_targets_without_counting_them_twice() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(100);
        assert!(EnergyGeneration::do_remove_validator(&10));
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 300);
        // Force-unbond/validator-to-cooperator paths keep the old target as
        // inactive; repeated cleanup must not subtract it from the other stake.
        assert_eq!(EnergyGeneration::cooperators(100).unwrap().targets.get(&10), Some(&200));
        EnergyGeneration::do_remove_validator_from_cooperators_target(&10);
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 300);
        assert!(EnergyGeneration::do_remove_validator(&20));
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 0);
    });
}

#[test]
fn collaborator_limit_rejects_growth_but_allows_existing_delegation_updates() {
    ExtBuilder::default().build_and_execute(|| {
        let full: std::collections::BTreeSet<_> =
            (0..u64::from(pallet_energy_generation::MAX_VALIDATOR_COLLABORATORS)).collect();
        let full: frame_support::storage::bounded_btree_set::BoundedBTreeSet<
            AccountId,
            frame_support::traits::ConstU32<{ u32::MAX }>,
        > = full.try_into().unwrap();
        pallet_energy_generation::Collaborations::<Test>::insert(10, full);
        assert_ok!(EnergyGeneration::do_add_cooperator(
            &100,
            EnergyGeneration::cooperators(100).unwrap()
        ));
        assert_noop!(
            EnergyGeneration::do_add_cooperator(&2000, EnergyGeneration::cooperators(100).unwrap()),
            pallet_energy_generation::Error::<Test>::TooManyCooperators
        );
        assert_eq!(
            EnergyGeneration::collaborations(10).unwrap().len(),
            pallet_energy_generation::MAX_VALIDATOR_COLLABORATORS as usize
        );
    });
}

#[test]
fn vip_exit_rejects_oversized_legacy_fanout_before_any_penalty_or_nft_burn() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let oversized: std::collections::BTreeSet<_> =
            (0..=u64::from(pallet_energy_generation::MAX_VALIDATOR_COLLABORATORS)).collect();
        let oversized: frame_support::storage::bounded_btree_set::BoundedBTreeSet<
            AccountId,
            frame_support::traits::ConstU32<{ u32::MAX }>,
        > = oversized.try_into().unwrap();
        pallet_energy_generation::Collaborations::<Test>::insert(10, oversized);
        assert_noop!(
            Privileges::exit_vip(RuntimeOrigin::signed(10)),
            Error::<Test>::AccountingCapacityExceeded
        );
    });
}
