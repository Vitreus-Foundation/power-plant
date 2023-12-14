//! Tests for the module.

use super::{ConfigOp, Event, *};
use crate::slashing::max_slash_amount;
use crate::testing_utils::perbill_signed_sub_abs;
use frame_support::{
    assert_noop, assert_ok, assert_storage_noop, bounded_vec,
    dispatch::{extract_actual_weight, Dispatchable, GetDispatchInfo, WithPostDispatchInfo},
    pallet_prelude::*,
    traits::{Currency, Get, ReservableCurrency},
    weights::Weight,
};
use mock::*;
use pallet_balances::Error as BalancesError;

use pallet_reputation::{ReputationRecord, ReputationTier};
use sp_runtime::{
    assert_eq_error_rate, traits::BadOrigin, FixedPointNumber, FixedU128, Perbill, Percent,
    TokenError,
};
use sp_staking::offence::{DisableStrategy, OffenceDetails};
use sp_std::prelude::*;
use substrate_test_utils::assert_eq_uvec;

#[test]
fn set_staking_configs_works() {
    ExtBuilder::default().build_and_execute(|| {
        // setting works
        assert_ok!(PowerPlant::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Set(1_500),
            ConfigOp::Set(2_000),
            ConfigOp::Set(10),
            ConfigOp::Set(20),
            ConfigOp::Set(Percent::from_percent(75)),
            ConfigOp::Set(Zero::zero())
        ));
        assert_eq!(MinCooperatorBond::<Test>::get(), 1_500);
        assert_eq!(MinValidatorBond::<Test>::get(), 2_000);
        assert_eq!(MaxCooperatorsCount::<Test>::get(), Some(10));
        assert_eq!(MaxValidatorsCount::<Test>::get(), Some(20));
        assert_eq!(ChillThreshold::<Test>::get(), Some(Percent::from_percent(75)));
        assert_eq!(MinCommission::<Test>::get(), Perbill::from_percent(0));

        // noop does nothing
        assert_storage_noop!(assert_ok!(PowerPlant::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop
        )));

        // removing works
        assert_ok!(PowerPlant::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Remove
        ));
        assert_eq!(MinCooperatorBond::<Test>::get(), 0);
        assert_eq!(MinValidatorBond::<Test>::get(), 0);
        assert_eq!(MaxCooperatorsCount::<Test>::get(), None);
        assert_eq!(MaxValidatorsCount::<Test>::get(), None);
        assert_eq!(ChillThreshold::<Test>::get(), None);
        assert_eq!(MinCommission::<Test>::get(), Perbill::from_percent(0));
    });
}

#[test]
fn force_unstake_works() {
    ExtBuilder::default().build_and_execute(|| {
        // Account 11 is stashed and locked, and account 10 is the controller
        assert_eq!(PowerPlant::bonded(11), Some(10));
        // Adds 2 slashing spans
        add_slash(&11);
        // Cant transfer
        assert_noop!(
            Balances::transfer_allow_death(RuntimeOrigin::signed(11), 1, 10),
            TokenError::Frozen,
        );
        // Force unstake requires root.
        assert_noop!(PowerPlant::force_unstake(RuntimeOrigin::signed(11), 11, 2), BadOrigin);
        // Force unstake needs correct number of slashing spans (for weight calculation)
        assert_noop!(
            PowerPlant::force_unstake(RuntimeOrigin::root(), 11, 0),
            Error::<Test>::IncorrectSlashingSpans
        );
        // We now force them to unstake
        assert_ok!(PowerPlant::force_unstake(RuntimeOrigin::root(), 11, 2));
        // No longer bonded.
        assert_eq!(PowerPlant::bonded(11), None);
        // Transfer works.
        assert_ok!(Balances::transfer_allow_death(RuntimeOrigin::signed(11), 1, 10));
    });
}

#[test]
fn kill_stash_works() {
    ExtBuilder::default().build_and_execute(|| {
        // Account 11 is stashed and locked, and account 10 is the controller
        assert_eq!(PowerPlant::bonded(11), Some(10));
        // Adds 2 slashing spans
        add_slash(&11);
        // Only can kill a stash account
        assert_noop!(PowerPlant::kill_stash(&12, 0), Error::<Test>::NotStash);
        // Respects slashing span count
        assert_noop!(PowerPlant::kill_stash(&11, 0), Error::<Test>::IncorrectSlashingSpans);
        // Correct inputs, everything works
        assert_ok!(PowerPlant::kill_stash(&11, 2));
        // No longer bonded.
        assert_eq!(PowerPlant::bonded(11), None);
    });
}

#[test]
fn basic_setup_works() {
    // Verifies initial conditions of mock
    ExtBuilder::default().build_and_execute(|| {
        // Account 11 is stashed and locked, and account 10 is the controller
        assert_eq!(PowerPlant::bonded(11), Some(10));
        // Account 21 is stashed and locked, and account 20 is the controller
        assert_eq!(PowerPlant::bonded(21), Some(20));
        // Account 1 is not a stashed
        assert_eq!(PowerPlant::bonded(1), None);

        // Account 10 controls the stash from account 11, which is 100 * balance_factor units
        assert_eq!(
            PowerPlant::ledger(10).unwrap(),
            StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            }
        );
        // Account 20 controls the stash from account 21, which is 200 * balance_factor units
        assert_eq!(
            PowerPlant::ledger(20),
            Some(StakingLedger {
                stash: 21,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
        // Account 1 does not control any stash
        assert_eq!(PowerPlant::ledger(1), None);

        // ValidatorPrefs are default
        assert_eq_uvec!(
            <Validators<Test>>::iter().collect::<Vec<_>>(),
            vec![
                (31, ValidatorPrefs::default()),
                (21, ValidatorPrefs::default_collaborative()),
                (11, ValidatorPrefs::default_collaborative())
            ]
        );

        assert_eq!(
            PowerPlant::ledger(100),
            Some(StakingLedger {
                stash: 101,
                total: 500,
                active: 500,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.into_iter().collect::<Vec<_>>(),
            vec![(11, 200), (21, 300)],
        );

        assert_eq!(
            PowerPlant::eras_stakers(active_era(), 11),
            Exposure {
                total: 1000 + 200,
                own: 1000,
                others: vec![IndividualExposure { who: 101, value: 200 }]
            },
        );
        assert_eq!(
            PowerPlant::eras_stakers(active_era(), 21),
            Exposure {
                total: 1000 + 300,
                own: 1000,
                others: vec![IndividualExposure { who: 101, value: 300 }]
            },
        );

        assert_eq!(Session::validators(), vec![31, 21, 11]);
        // initial total stake = 1300 + 1200 + 500 (31)
        assert_eq!(PowerPlant::eras_total_stake(active_era()), 3000);

        // The number of validators required.
        assert_eq!(PowerPlant::validator_count(), 2);

        // Initial Era and session
        assert_eq!(active_era(), 0);

        // Account 10 has `balance_factor` free balance
        assert_eq!(Balances::free_balance(10), 1);
        assert_eq!(Balances::free_balance(10), 1);

        // New era is not being forced
        assert_eq!(PowerPlant::force_era(), Forcing::NotForcing);
    });
}

#[test]
fn change_controller_works() {
    ExtBuilder::default().build_and_execute(|| {
        // 10 and 11 are bonded as stash controller.
        assert_eq!(PowerPlant::bonded(11), Some(10));

        // 10 can control 11 who is initially a validator.
        assert_ok!(PowerPlant::chill(RuntimeOrigin::signed(10)));

        // change controller
        assert_ok!(PowerPlant::set_controller(RuntimeOrigin::signed(11), 5));
        assert_eq!(PowerPlant::bonded(11), Some(5));
        mock::start_active_era(1);

        // 10 is no longer in control.
        assert_noop!(
            PowerPlant::validate(RuntimeOrigin::signed(10), ValidatorPrefs::default()),
            Error::<Test>::NotController,
        );
        assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(5), ValidatorPrefs::default()));
    })
}

#[test]
fn rewards_should_work() {
    ExtBuilder::default().cooperate(true).session_per_era(3).build_and_execute(|| {
        let init_balance_11 = Balances::total_balance(&11);
        let init_balance_21 = Balances::total_balance(&21);
        let init_balance_101 = Balances::total_balance(&101);

        // Set payees
        Payee::<Test>::insert(11, RewardDestination::Controller);
        Payee::<Test>::insert(21, RewardDestination::Controller);
        Payee::<Test>::insert(101, RewardDestination::Controller);

        let init_rep_11 = ReputationPallet::reputation(11).unwrap().reputation.points();
        let init_rep_21 = ReputationPallet::reputation(21).unwrap().reputation.points();

        Pallet::<Test>::reward_by_ids(vec![(11, 50.into())]);
        Pallet::<Test>::reward_by_ids(vec![(11, 50.into())]);
        // This is the second validator of the current elected set.
        Pallet::<Test>::reward_by_ids(vec![(21, 50.into())]);

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points() - 100,
            *init_rep_11
        );
        assert_eq!(
            *ReputationPallet::reputation(21).unwrap().reputation.points() - 50,
            *init_rep_21
        );

        // Compute total payout now for whole duration of the session.
        let total_payout_0 = current_total_payout_for_duration(reward_time_per_era());

        start_session(1);
        assert_eq_uvec!(Session::validators(), vec![31, 21, 11]);

        let part_for_10 = FixedU128::from_rational(1000, 3000) * FixedU128::from_float(1.08);
        assert_eq!(controller_stash_reputation_tier(&10), Some(ReputationTier::Trailblazer(1)));
        let part_for_20 = FixedU128::from_rational(1000, 3000) * FixedU128::from_float(1.08);
        assert_eq!(controller_stash_reputation_tier(&20), Some(ReputationTier::Trailblazer(1)));

        let part_for_100_from_10 = Perbill::from_rational::<u32>(200, 3000);
        assert_eq!(controller_stash_reputation_tier(&100), None);
        let part_for_100_from_20 = Perbill::from_rational::<u32>(300, 3000);

        start_session(2);
        start_session(3);

        assert_eq!(active_era(), 1);

        mock::make_all_reward_payment(0);

        assert_eq_error_rate!(
            Assets::balance(VNRG::get(), 10),
            part_for_10.saturating_mul_int(total_payout_0),
            2
        );
        assert_eq_error_rate!(Balances::total_balance(&11), init_balance_11, 2);
        assert_eq_error_rate!(
            Assets::balance(VNRG::get(), 20),
            part_for_20.saturating_mul_int(total_payout_0),
            2
        );
        assert_eq_error_rate!(Balances::total_balance(&21), init_balance_21, 2);
        assert_eq_error_rate!(
            Assets::balance(VNRG::get(), 100),
            part_for_100_from_10 * total_payout_0 + part_for_100_from_20 * total_payout_0,
            2
        );
        assert_eq_error_rate!(Balances::total_balance(&101), init_balance_101, 2);

        assert_eq_uvec!(Session::validators(), vec![31, 21, 11]);
        Pallet::<Test>::reward_by_ids(vec![(11, 1.into())]);

        // Compute total payout now for whole duration as other parameter won't change
        let total_payout_1 = current_total_payout_for_duration(reward_time_per_era());

        mock::start_active_era(2);
        let mut events = mock::staking_events();
        let energy_rate = ErasEnergyPerStakeCurrency::<Test>::get(2).unwrap();
        assert_eq!(
            events.pop().unwrap(),
            Event::EraEnergyPerStakeCurrencySet { era_index: 2, energy_rate }
        );
        mock::make_all_reward_payment(1);

        assert_eq_error_rate!(
            Assets::balance(VNRG::get(), 10),
            part_for_10.saturating_mul_int(total_payout_0 + total_payout_1),
            2
        );
        assert_eq_error_rate!(Balances::total_balance(&11), init_balance_11, 2);
        assert_eq_error_rate!(
            Assets::balance(VNRG::get(), 20),
            part_for_20.saturating_mul_int(total_payout_0 + total_payout_1),
            2
        );
        assert_eq_error_rate!(Balances::total_balance(&21), init_balance_21, 2);
        assert_eq_error_rate!(
            Assets::balance(VNRG::get(), 100),
            part_for_100_from_10 * (total_payout_0 + total_payout_1)
                + part_for_100_from_20 * (total_payout_0 + total_payout_1),
            2
        );
        assert_eq_error_rate!(Balances::total_balance(&101), init_balance_101, 2);
    });
}

#[test]
fn staking_should_work() {
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        // remember + compare this along with the test.
        assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

        // put some money and reputation in account that we'll use.
        for i in 1..5 {
            let _ = Balances::make_free_balance_be(&i, 2000);
            ReputationPallet::force_set_points(
                RuntimeOrigin::root(),
                i,
                ValidatorReputationTier::get().into(),
            )
            .unwrap();
        }

        // --- Block 2:
        start_session(2);
        // add a new candidate for being a validator. account 3 controlled by 4.
        assert_ok!(PowerPlant::bond(
            RuntimeOrigin::signed(3),
            4,
            1500,
            RewardDestination::Controller
        ));
        assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(4), ValidatorPrefs::default()));
        assert_ok!(Session::set_keys(
            RuntimeOrigin::signed(4),
            SessionKeys { other: 4.into() },
            vec![]
        ));

        // No effects will be seen so far.
        assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

        // --- Block 3:
        start_session(3);

        // No effects will be seen so far. Era has not been yet triggered.
        assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

        // --- Block 4: the validators will now be queued.
        start_session(4);
        assert_eq!(active_era(), 1);

        // --- Block 5: the validators are still in queue.
        start_session(5);

        // --- Block 6: the validators will now be changed.
        start_session(6);

        assert_eq_uvec!(validator_controllers(), vec![4, 30, 20, 10]);
        // --- Block 6: Unstake 4 as a validator, freeing up the balance stashed in 3
        // 4 will chill
        PowerPlant::chill(RuntimeOrigin::signed(4)).unwrap();

        // --- Block 7: nothing. 4 is still there.
        start_session(7);
        assert_eq_uvec!(validator_controllers(), vec![4, 30, 20, 10]);

        // --- Block 8:
        start_session(8);

        // --- Block 9: 4 will not be a validator.
        start_session(9);
        assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

        // Note: the stashed value of 4 is still lock
        assert_eq!(
            PowerPlant::ledger(4),
            Some(StakingLedger {
                stash: 3,
                total: 1500,
                active: 1500,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![0],
            })
        );
        // e.g. it cannot reserve more than 500 that it has free from the total 2000
        assert_noop!(Balances::reserve(&3, 501), BalancesError::<Test, _>::LiquidityRestrictions);
        assert_ok!(Balances::reserve(&3, 409));
    });
}

#[test]
fn blocking_and_kicking_works() {
    ExtBuilder::default()
        .minimum_validator_count(1)
        .validator_count(4)
        .cooperate(true)
        .build_and_execute(|| {
            // block validator 10/11
            assert_ok!(PowerPlant::validate(
                RuntimeOrigin::signed(10),
                ValidatorPrefs { collaborative: false, ..Default::default() }
            ));
            // attempt to cooperate from 100/101...
            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(100), vec![(11, 200)]));
            // should have worked since we're already cooperated them
            assert_eq!(
                Cooperators::<Test>::get(101).unwrap().targets.into_iter().collect::<Vec<_>>(),
                vec![(11, 200)]
            );
            // kick the cooperator
            assert_ok!(PowerPlant::kick(RuntimeOrigin::signed(10), vec![101]));
            // should have been kicked now
            assert!(Cooperators::<Test>::get(101).unwrap().targets.is_empty());
            // attempt to cooperate from 100/101...
            assert_noop!(
                PowerPlant::cooperate(RuntimeOrigin::signed(100), vec![(11, 200)]),
                Error::<Test>::BadTarget
            );
        });
}

#[test]
fn less_than_needed_candidates_works() {
    ExtBuilder::default()
        .minimum_validator_count(1)
        .validator_count(4)
        .cooperate(false)
        .build_and_execute(|| {
            assert_eq!(PowerPlant::validator_count(), 4);
            assert_eq!(PowerPlant::minimum_validator_count(), 1);
            assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

            mock::start_active_era(1);

            // Previous set is selected. NO election algorithm is even executed.
            assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

            // But the exposure is updated in a simple way. No external votes exists.
            // This is purely self-vote.
            assert!(ErasStakers::<Test>::iter_prefix_values(active_era())
                .all(|exposure| exposure.others.is_empty()));
        });
}

#[test]
fn no_candidate_emergency_condition() {
    ExtBuilder::default()
        .minimum_validator_count(1)
        .validator_count(15)
        .set_status(41, StakerStatus::Validator)
        .cooperate(false)
        .build_and_execute(|| {
            // initial validators
            assert_eq_uvec!(validator_controllers(), vec![10, 20, 30, 40]);
            let prefs = ValidatorPrefs { commission: Perbill::one(), ..Default::default() };
            Validators::<Test>::insert(11, prefs.clone());

            // set the minimum validator count.
            MinimumValidatorCount::<Test>::put(10);

            // try to chill
            let res = PowerPlant::chill(RuntimeOrigin::signed(10));
            assert_ok!(res);

            let current_era = CurrentEra::<Test>::get();

            // try trigger new era
            mock::run_to_block(20);
            assert_eq!(*staking_events().last().unwrap(), Event::StakingElectionFailed);
            // No new era is created
            assert_eq!(current_era, CurrentEra::<Test>::get());

            // Go to far further session to see if validator have changed
            mock::run_to_block(100);

            // Previous ones are elected. chill is not effective in active era (as era hasn't
            // changed)
            assert_eq_uvec!(validator_controllers(), vec![10, 20, 30, 40]);
            // The chill is still pending.
            assert!(!Validators::<Test>::contains_key(11));
            // No new era is created.
            assert_eq!(current_era, CurrentEra::<Test>::get());
        });
}

#[test]
fn cooperating_and_rewards_should_work() {
    ExtBuilder::default()
        .cooperate(false)
        .set_status(41, StakerStatus::Validator)
        .set_status(11, StakerStatus::Idle)
        .set_status(31, StakerStatus::Idle)
        .build_and_execute(|| {
            // initial validators.
            assert_eq_uvec!(validator_controllers(), vec![40, 20]);

            // re-validate with 11 and 31.
            assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(10), Default::default()));
            assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(30), Default::default()));

            // Set payee to controller.
            assert_ok!(PowerPlant::set_payee(
                RuntimeOrigin::signed(10),
                RewardDestination::Controller
            ));
            assert_ok!(PowerPlant::set_payee(
                RuntimeOrigin::signed(20),
                RewardDestination::Controller
            ));
            assert_ok!(PowerPlant::set_payee(
                RuntimeOrigin::signed(30),
                RewardDestination::Controller
            ));
            assert_ok!(PowerPlant::set_payee(
                RuntimeOrigin::signed(40),
                RewardDestination::Controller
            ));

            // give the man some money
            let initial_balance = 1000;
            for i in [1, 2, 3, 4, 5, 10, 11, 20, 21].iter() {
                let _ = Balances::make_free_balance_be(i, initial_balance);
                ReputationPallet::add_not_exists(i);
            }

            // bond two account pairs and state interest in cooperation.
            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(1),
                2,
                1000,
                RewardDestination::Controller
            ));

            // 31 doesn't have enough reputation to be collaborative validator, moreover it's Idle
            assert_noop!(
                PowerPlant::cooperate(
                    RuntimeOrigin::signed(2),
                    vec![(11, 300), (21, 200), (31, 500)]
                ),
                Error::<Test>::BadTarget
            );

            // 11 is Idle, so despite it has enough reputation, it's a bad target
            assert_noop!(
                PowerPlant::cooperate(RuntimeOrigin::signed(2), vec![(11, 300), (21, 700)]),
                Error::<Test>::BadTarget
            );

            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(2), vec![(21, 1000)]));

            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(3),
                4,
                1000,
                RewardDestination::Controller
            ));

            // the validator should make himself collaborative after Idle
            assert_ok!(PowerPlant::make_collaborative(RuntimeOrigin::signed(11)));

            // 41 doesn't have enough reputation for collaborative validator
            assert_noop!(
                PowerPlant::cooperate(
                    RuntimeOrigin::signed(4),
                    vec![(21, 200), (11, 200), (41, 600)]
                ),
                Error::<Test>::ReputationTooLow
            );

            ReputationPallet::force_set_points(
                RuntimeOrigin::root(),
                41,
                CollaborativeValidatorReputationTier::get().into(),
            )
            .unwrap();

            // despite it has enought reputation  now, the validator didn't set desire to be a
            // collaborative validator, so it's still an error
            assert_noop!(
                PowerPlant::cooperate(
                    RuntimeOrigin::signed(4),
                    vec![(21, 200), (11, 400), (41, 400)]
                ),
                Error::<Test>::BadTarget
            );

            assert_ok!(PowerPlant::make_collaborative(RuntimeOrigin::signed(41)));
            assert_ok!(PowerPlant::cooperate(
                RuntimeOrigin::signed(4),
                vec![(21, 200), (11, 150), (41, 525)]
            ));

            // the total reward for era 0
            let total_payout_0 = current_total_payout_for_duration(reward_time_per_era());
            PowerPlant::reward_by_ids(vec![(41, 1.into())]);
            PowerPlant::reward_by_ids(vec![(21, 1.into())]);

            mock::start_active_era(1);

            assert_eq_uvec!(validator_controllers(), vec![30, 40, 20, 10]);
            let eras_total_stake = ErasTotalStake::<Test>::get(0);

            let energy_reward_40 =
                calculate_reward(total_payout_0, eras_total_stake, 1000, Percent::from_percent(8));
            assert_eq!(controller_stash_reputation_tier(&40), Some(ReputationTier::Trailblazer(1)));

            let energy_reward_20 =
                calculate_reward(total_payout_0, eras_total_stake, 1000, Percent::from_percent(8));
            assert_eq!(controller_stash_reputation_tier(&20), Some(ReputationTier::Trailblazer(1)));

            // old validators must have already received some rewards.
            let initial_balance_40 = Assets::balance(VNRG::get(), 40);
            let mut initial_balance_20 = Assets::balance(VNRG::get(), 20);
            mock::make_all_reward_payment(0);
            assert_eq!(Assets::balance(VNRG::get(), 40), initial_balance_40 + energy_reward_40);
            assert_eq!(Assets::balance(VNRG::get(), 20), initial_balance_20 + energy_reward_20);
            initial_balance_20 = Assets::balance(VNRG::get(), 20);

            assert_eq!(ErasStakers::<Test>::iter_prefix_values(active_era()).count(), 4);
            assert_eq!(
                PowerPlant::eras_stakers(active_era(), 11),
                Exposure {
                    total: 1000 + 150,
                    own: 1000,
                    others: vec![IndividualExposure { who: 3, value: 150 }]
                },
            );
            assert_eq!(
                PowerPlant::eras_stakers(active_era(), 21),
                Exposure {
                    total: 1000 + 1000 + 200,
                    own: 1000,
                    others: vec![
                        IndividualExposure { who: 1, value: 1000 },
                        IndividualExposure { who: 3, value: 200 },
                    ]
                },
            );
            assert_eq!(
                PowerPlant::eras_stakers(active_era(), 41),
                Exposure {
                    total: 1000 + 525,
                    own: 1000,
                    others: vec![IndividualExposure { who: 3, value: 525 },]
                },
            );

            // the total reward for era 1
            let total_payout_1 = current_total_payout_for_duration(reward_time_per_era());
            PowerPlant::reward_by_ids(vec![(21, 2.into())]);
            PowerPlant::reward_by_ids(vec![(11, 1.into())]);

            mock::start_active_era(2);

            // nothing else will happen, era ends and rewards are paid again, it is expected that
            // cooperators will also be paid. See below

            mock::make_all_reward_payment(1);

            let eras_total_stake = ErasTotalStake::<Test>::get(1);
            // Cooperator 2: staked 1000 on 20, thus 1/1 but the rewards differ due to different
            // reputation bonus
            let energy_reward_2 =
                calculate_reward(total_payout_1, eras_total_stake, 1000, Percent::from_percent(0));
            assert_eq!(controller_stash_reputation_tier(&2), None);
            let energy_reward_20 =
                calculate_reward(total_payout_1, eras_total_stake, 1000, Percent::from_percent(8));
            assert_eq!(controller_stash_reputation_tier(&20), Some(ReputationTier::Trailblazer(1)));
            assert_eq_error_rate!(Assets::balance(VNRG::get(), 2), energy_reward_2, 4,);
            assert_eq_error_rate!(
                Assets::balance(VNRG::get(), 20),
                initial_balance_20 + energy_reward_20,
                3,
            );

            // Cooperator 4: staked 150 on 10, 200 on 20 and 525 on 40
            let energy_reward_4 = calculate_reward(
                total_payout_1,
                eras_total_stake,
                150 + 200 + 525,
                Percent::from_percent(0),
            );
            assert_eq!(controller_stash_reputation_tier(&4), None);

            assert_eq_error_rate!(Assets::balance(VNRG::get(), 4), energy_reward_4, 2,);

            let energy_reward_10 =
                calculate_reward(total_payout_1, eras_total_stake, 1000, Percent::from_percent(8));
            assert_eq!(controller_stash_reputation_tier(&10), Some(ReputationTier::Trailblazer(1)));

            assert_eq_error_rate!(Assets::balance(VNRG::get(), 10), energy_reward_10, 4,);
        });
}

#[test]
fn cooperators_also_get_slashed_pro_rata() {
    ExtBuilder::default().build_and_execute(|| {
        for n in [100, 101] {
            ReputationPallet::add_not_exists(&n);
        }
        mock::start_active_era(1);
        let slash_percent = Perbill::from_percent(5);
        let initial_exposure = PowerPlant::eras_stakers(active_era(), 11);
        // 101 is a cooperator for 11
        assert_eq!(initial_exposure.others.first().unwrap().who, 101);

        // staked values;
        let cooperator_stake = PowerPlant::ledger(100).unwrap().active;
        let cooperator_reputation = ReputationPallet::reputation(101).unwrap().reputation.points();
        let cooperator_balance = balances(&101).0;
        let validator_stake = PowerPlant::ledger(10).unwrap().active;
        let validator_reputation = ReputationPallet::reputation(11).unwrap().reputation.points();
        let validator_balance = balances(&11).0;

        assert!(*cooperator_reputation > 0);
        assert!(*validator_reputation > 0);

        // 11 goes offline
        on_offence_now(
            &[OffenceDetails { offender: (11, initial_exposure.clone()), reporters: vec![] }],
            &[slash_percent],
        );

        // stakes shouldn't be touched
        assert_eq!(PowerPlant::ledger(100).unwrap().active, cooperator_stake);
        assert_eq!(PowerPlant::ledger(10).unwrap().active, validator_stake);
        // as well the balance
        assert_eq!(balances(&101).0, cooperator_balance);
        assert_eq!(balances(&11).0, validator_balance);
        // reputation must have been decreased
        let slashed_validator_reputation =
            ReputationPallet::reputation(11).unwrap().reputation.points();
        let slashed_cooperator_reputation =
            ReputationPallet::reputation(101).unwrap().reputation.points();
        assert!(*slashed_validator_reputation > 0);
        assert!(*slashed_cooperator_reputation > 0);
        assert!(*slashed_validator_reputation < *validator_reputation);
        assert!(*slashed_cooperator_reputation < *cooperator_reputation);

        let validator_slash_p = Perbill::from_percent(100)
            - Perbill::from_rational(*slashed_validator_reputation, *validator_reputation);
        let cooperator_slash_p = Perbill::from_percent(100)
            - Perbill::from_rational(*slashed_cooperator_reputation, *cooperator_reputation);
        assert!(validator_slash_p > Perbill::zero());
        assert!(cooperator_slash_p > Perbill::zero());
        // allow error rate of 0.05%
        assert!(
            perbill_signed_sub_abs(validator_slash_p, cooperator_slash_p)
                < Perbill::from_rational(1u32, 2000)
        );
    });
}

#[test]
fn double_staking_should_fail() {
    // should test (in the same order):
    // * an account already bonded as stash cannot be be stashed again.
    // * an account already bonded as stash cannot cooperate.
    // * an account already bonded as controller can cooperate.
    ExtBuilder::default().build_and_execute(|| {
        let arbitrary_value = 5;
        // 2 = controller, 1 stashed => ok
        assert_ok!(PowerPlant::bond(
            RuntimeOrigin::signed(1),
            2,
            arbitrary_value,
            RewardDestination::default()
        ));
        // 4 = not used so far, 1 stashed => not allowed.
        assert_noop!(
            PowerPlant::bond(
                RuntimeOrigin::signed(1),
                4,
                arbitrary_value,
                RewardDestination::default()
            ),
            Error::<Test>::AlreadyBonded,
        );
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            1,
            CollaborativeValidatorReputationTier::get().into()
        ));
        assert_ok!(PowerPlant::validate(
            RuntimeOrigin::signed(2),
            ValidatorPrefs::default_collaborative()
        ));

        // 1 = stashed => attempting to cooperate should fail.
        assert_noop!(
            PowerPlant::cooperate(RuntimeOrigin::signed(1), vec![(1, 5)]),
            Error::<Test>::NotController
        );
        // 2 = controller  => cooperating should work.
        assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(2), vec![(1, 5)]));
    });
}

#[test]
fn double_controlling_should_fail() {
    // should test (in the same order):
    // * an account already bonded as controller CANNOT be reused as the controller of another
    //   account.
    ExtBuilder::default().build_and_execute(|| {
        let arbitrary_value = 5;
        // 2 = controller, 1 stashed => ok
        assert_ok!(PowerPlant::bond(
            RuntimeOrigin::signed(1),
            2,
            arbitrary_value,
            RewardDestination::default(),
        ));
        // 2 = controller, 3 stashed (Note that 2 is reused.) => no-op
        assert_noop!(
            PowerPlant::bond(
                RuntimeOrigin::signed(3),
                2,
                arbitrary_value,
                RewardDestination::default()
            ),
            Error::<Test>::AlreadyPaired,
        );
    });
}

#[test]
fn session_and_eras_work_simple() {
    ExtBuilder::default().period(1).build_and_execute(|| {
        assert_eq!(active_era(), 0);
        assert_eq!(current_era(), 0);
        assert_eq!(Session::current_index(), 1);
        assert_eq!(System::block_number(), 1);

        // Session 1: this is basically a noop. This has already been started.
        start_session(1);
        assert_eq!(Session::current_index(), 1);
        assert_eq!(active_era(), 0);
        assert_eq!(System::block_number(), 1);

        // Session 2: No change.
        start_session(2);
        assert_eq!(Session::current_index(), 2);
        assert_eq!(active_era(), 0);
        assert_eq!(System::block_number(), 2);

        // Session 3: Era increment.
        start_session(3);
        assert_eq!(Session::current_index(), 3);
        assert_eq!(active_era(), 1);
        assert_eq!(System::block_number(), 3);

        // Session 4: No change.
        start_session(4);
        assert_eq!(Session::current_index(), 4);
        assert_eq!(active_era(), 1);
        assert_eq!(System::block_number(), 4);

        // Session 5: No change.
        start_session(5);
        assert_eq!(Session::current_index(), 5);
        assert_eq!(active_era(), 1);
        assert_eq!(System::block_number(), 5);

        // Session 6: Era increment.
        start_session(6);
        assert_eq!(Session::current_index(), 6);
        assert_eq!(active_era(), 2);
        assert_eq!(System::block_number(), 6);
    });
}

#[test]
fn session_and_eras_work_complex() {
    ExtBuilder::default().period(5).build_and_execute(|| {
        assert_eq!(active_era(), 0);
        assert_eq!(Session::current_index(), 0);
        assert_eq!(System::block_number(), 1);

        start_session(1);
        assert_eq!(Session::current_index(), 1);
        assert_eq!(active_era(), 0);
        assert_eq!(System::block_number(), 5);

        start_session(2);
        assert_eq!(Session::current_index(), 2);
        assert_eq!(active_era(), 0);
        assert_eq!(System::block_number(), 10);

        start_session(3);
        assert_eq!(Session::current_index(), 3);
        assert_eq!(active_era(), 1);
        assert_eq!(System::block_number(), 15);

        start_session(4);
        assert_eq!(Session::current_index(), 4);
        assert_eq!(active_era(), 1);
        assert_eq!(System::block_number(), 20);

        start_session(5);
        assert_eq!(Session::current_index(), 5);
        assert_eq!(active_era(), 1);
        assert_eq!(System::block_number(), 25);

        start_session(6);
        assert_eq!(Session::current_index(), 6);
        assert_eq!(active_era(), 2);
        assert_eq!(System::block_number(), 30);
    });
}

#[test]
fn forcing_new_era_works() {
    ExtBuilder::default().build_and_execute(|| {
        // normal flow of session.
        start_session(1);
        assert_eq!(active_era(), 0);

        start_session(2);
        assert_eq!(active_era(), 0);

        start_session(3);
        assert_eq!(active_era(), 1);

        // no era change.
        PowerPlant::set_force_era(Forcing::ForceNone);

        start_session(4);
        assert_eq!(active_era(), 1);

        start_session(5);
        assert_eq!(active_era(), 1);

        start_session(6);
        assert_eq!(active_era(), 1);

        start_session(7);
        assert_eq!(active_era(), 1);

        // back to normal.
        // this immediately starts a new session.
        PowerPlant::set_force_era(Forcing::NotForcing);

        start_session(8);
        assert_eq!(active_era(), 1);

        start_session(9);
        assert_eq!(active_era(), 2);
        // forceful change
        PowerPlant::set_force_era(Forcing::ForceAlways);

        start_session(10);
        assert_eq!(active_era(), 2);

        start_session(11);
        assert_eq!(active_era(), 3);

        start_session(12);
        assert_eq!(active_era(), 4);

        // just one forceful change
        PowerPlant::set_force_era(Forcing::ForceNew);
        start_session(13);
        assert_eq!(active_era(), 5);
        assert_eq!(ForceEra::<Test>::get(), Forcing::NotForcing);

        start_session(14);
        assert_eq!(active_era(), 6);

        start_session(15);
        assert_eq!(active_era(), 6);
    });
}

#[test]
fn cannot_transfer_staked_balance() {
    // Tests that a stash account cannot transfer funds
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        // Confirm account 11 is stashed
        assert_eq!(PowerPlant::bonded(11), Some(10));
        // Confirm account 11 has some free balance
        assert_eq!(Balances::free_balance(11), 1000);
        // Confirm account 11 (via controller 10) is totally staked
        assert_eq!(PowerPlant::eras_stakers(active_era(), 11).total, 1000);
        // Confirm account 11 cannot transfer as a result
        assert_noop!(
            Balances::transfer_allow_death(RuntimeOrigin::signed(11), 20, 1),
            TokenError::Frozen,
        );

        // Give account 11 extra free balance
        let _ = Balances::make_free_balance_be(&11, 10000);
        // Confirm that account 11 can now transfer some balance
        assert_ok!(Balances::transfer_allow_death(RuntimeOrigin::signed(11), 20, 1));
    });
}

#[test]
fn cannot_transfer_staked_balance_2() {
    // Tests that a stash account cannot transfer funds
    // Same test as above but with 20, and more accurate.
    // 21 has 2000 free balance but 1000 at stake
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        // Confirm account 21 is stashed
        assert_eq!(PowerPlant::bonded(21), Some(20));
        // Confirm account 21 has some free balance
        assert_eq!(Balances::free_balance(21), 2000);
        // Confirm account 21 (via controller 20) is totally staked
        assert_eq!(PowerPlant::eras_stakers(active_era(), 21).total, 1000);
        // Confirm account 21 can transfer at most 1000
        assert_noop!(
            Balances::transfer_allow_death(RuntimeOrigin::signed(21), 20, 1001),
            TokenError::Frozen,
        );
        assert_ok!(Balances::transfer_allow_death(RuntimeOrigin::signed(21), 20, 1000));
    });
}

#[test]
fn cannot_reserve_staked_balance() {
    // Checks that a bonded account cannot reserve balance from free balance
    ExtBuilder::default().build_and_execute(|| {
        // Confirm account 11 is stashed
        assert_eq!(PowerPlant::bonded(11), Some(10));
        // Confirm account 11 has some free balance
        assert_eq!(Balances::free_balance(11), 1000);
        // Confirm account 11 (via controller 10) is totally staked
        assert_eq!(PowerPlant::eras_stakers(active_era(), 11).own, 1000);
        // Confirm account 11 cannot reserve as a result
        assert_noop!(Balances::reserve(&11, 1), BalancesError::<Test, _>::LiquidityRestrictions);

        // Give account 11 extra free balance
        let _ = Balances::make_free_balance_be(&11, 10000);
        // Confirm account 11 can now reserve balance
        assert_ok!(Balances::reserve(&11, 1));
    });
}

#[test]
fn reward_destination_works() {
    // Rewards go to the correct destination as determined in Payee
    ExtBuilder::default().cooperate(false).validator_count(1).build_and_execute(|| {
        // Check that account 11 is a validator
        assert!(Session::validators().contains(&11));
        // Check the balance of the validator account
        assert_eq!(Balances::free_balance(10), 1);
        // Check the balance of the stash account
        assert_eq!(Balances::free_balance(11), 1000);
        // Check how much is at stake
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );

        // Compute total payout now for whole duration as other parameter won't change
        let total_payout_0 = current_total_payout_for_duration(reward_time_per_era());
        Pallet::<Test>::reward_by_ids(vec![(11, 1000.into())]);

        mock::start_active_era(1);
        mock::make_all_reward_payment(0);
        let total_stake = ErasTotalStake::<Test>::get(0);
        let energy_reward_10_0 =
            calculate_reward(total_payout_0, total_stake, 1000, Percent::from_percent(8));
        assert_eq!(controller_stash_reputation_tier(&10), Some(ReputationTier::Trailblazer(1)));
        let controller_balance_0 = Assets::balance(VNRG::get(), 10);

        // check the reward destination
        assert_eq!(PowerPlant::payee(11), RewardDestination::Controller);
        // controller reaceve reward
        assert_eq!(controller_balance_0, energy_reward_10_0);

        // Change RewardDestination to Stash
        Payee::<Test>::insert(11, RewardDestination::Stash);

        // Compute total payout now for whole duration as other parameter won't change
        let total_payout_1 = current_total_payout_for_duration(reward_time_per_era());
        Pallet::<Test>::reward_by_ids(vec![(11, 1000.into())]);

        mock::start_active_era(2);
        mock::make_all_reward_payment(1);

        let total_stake = ErasTotalStake::<Test>::get(1);
        // Stash reputation tier hasn't changed, no check needed
        let energy_reward_10_1 =
            calculate_reward(total_payout_1, total_stake, 1000, Percent::from_percent(8));

        // Check that RewardDestination is Stash
        assert_eq!(PowerPlant::payee(11), RewardDestination::Stash);
        // Check that reward went to the stash account
        assert_eq!(Assets::balance(VNRG::get(), 11), energy_reward_10_1);
        // Record this value
        let recorded_stash_balance = Assets::balance(VNRG::get(), 11);

        // Change RewardDestination to Controller
        Payee::<Test>::insert(11, RewardDestination::Controller);

        // Check controller balance
        assert_eq!(Assets::balance(VNRG::get(), 10), controller_balance_0);

        // Compute total payout now for whole duration as other parameter won't change
        let total_payout_2 = current_total_payout_for_duration(reward_time_per_era());
        Pallet::<Test>::reward_by_ids(vec![(11, 1000.into())]);

        mock::start_active_era(3);
        mock::make_all_reward_payment(2);
        let total_stake = ErasTotalStake::<Test>::get(2);
        // Stash reputation tier hasn't changed, no check needed
        let energy_reward_10_2 =
            calculate_reward(total_payout_2, total_stake, 1000, Percent::from_percent(8));

        // Check that RewardDestination is Controller
        assert_eq!(PowerPlant::payee(11), RewardDestination::Controller);
        // Check that reward went to the controller account
        assert_eq!(Assets::balance(VNRG::get(), 10), energy_reward_10_2 + controller_balance_0);
        // stash balance shouldn't be changed
        assert_eq!(Assets::balance(VNRG::get(), 11), recorded_stash_balance);
    });
}

#[test]
fn validator_payment_prefs_work() {
    // Test that validator preferences are correctly honored
    // Note: unstake threshold is being directly tested in slashing tests.
    // This test will focus on validator payment.
    ExtBuilder::default().build_and_execute(|| {
        // there are other validators, so we need to clear the storage first
        let _ = Validators::<Test>::clear(u32::MAX, None);
        let commission = Perbill::from_percent(40);
        Validators::<Test>::insert(
            11,
            ValidatorPrefs { commission, collaborative: true, ..Default::default() },
        );

        // Reward controller so staked ratio doesn't change.
        Payee::<Test>::insert(11, RewardDestination::Controller);
        Payee::<Test>::insert(101, RewardDestination::Controller);

        mock::start_active_era(1);
        mock::make_all_reward_payment(0);

        let balance_era_1_10 = Assets::balance(VNRG::get(), 10);
        let balance_era_1_100 = Assets::balance(VNRG::get(), 100);

        // Compute total payout now for whole duration as other parameter won't change
        let total_payout_1 = current_total_payout_for_duration(reward_time_per_era());
        let exposure_1 = PowerPlant::eras_stakers(active_era(), 11);
        // PowerPlant::reward_by_ids(vec![(11, 1.into())]);

        mock::start_active_era(2);
        mock::make_all_reward_payment(1);

        let total_stake = ErasTotalStake::<Test>::get(1);
        let ratio = Perbill::from_rational(exposure_1.total, total_stake);
        let total_reward = ratio * total_payout_1;
        let taken_cut = commission * total_reward;
        let shared_cut = total_reward - taken_cut;
        let mut reward_of_10 = shared_cut * exposure_1.own / exposure_1.total + taken_cut;
        // Additional 8% since stash account has a Tralblazer(1) reputation tier
        reward_of_10 = Perbill::from_percent(8) * reward_of_10 + reward_of_10;
        let reward_of_100 = shared_cut * exposure_1.others[0].value / exposure_1.total;
        assert_eq_error_rate!(Assets::balance(VNRG::get(), 10), balance_era_1_10 + reward_of_10, 2);
        assert_eq_error_rate!(
            Assets::balance(VNRG::get(), 100),
            balance_era_1_100 + reward_of_100,
            2
        );
    });
}

#[test]
fn bond_extra_works() {
    // Tests that extra `free_balance` in the stash can be added to stake
    // NOTE: this tests only verifies `StakingLedger` for correct updates
    // See `bond_extra_and_withdraw_unbonded_works` for more details and updates on `Exposure`.
    ExtBuilder::default().build_and_execute(|| {
        // Check that account 10 is a validator
        assert!(<Validators<Test>>::contains_key(11));
        // Check that account 10 is bonded to account 11
        assert_eq!(PowerPlant::bonded(11), Some(10));
        // Check how much is at stake
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );

        // Give account 11 some large free balance greater than total
        let _ = Balances::make_free_balance_be(&11, 1000000);

        // Call the bond_extra function from controller, add only 100
        assert_ok!(PowerPlant::bond_extra(RuntimeOrigin::signed(11), 100));
        // There should be 100 more `total` and `active` in the ledger
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000 + 100,
                active: 1000 + 100,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );

        // Call the bond_extra function with a large number, should handle it
        assert_ok!(PowerPlant::bond_extra(RuntimeOrigin::signed(11), Balance::max_value()));
        // The full amount of the funds should now be in the total and active
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000000,
                active: 1000000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
    });
}

#[test]
fn bond_extra_and_withdraw_unbonded_works() {
    //
    // * Should test
    // * Given an account being bonded [and chosen as a validator](not mandatory)
    // * It can add extra funds to the bonded account.
    // * it can unbond a portion of its funds from the stash account.
    // * Once the unbonding period is done, it can actually take the funds out of the stash.
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        // Set payee to controller. avoids confusion
        assert_ok!(PowerPlant::set_payee(RuntimeOrigin::signed(10), RewardDestination::Controller));

        // Give account 11 some large free balance greater than total
        let _ = Balances::make_free_balance_be(&11, 1000000);

        // Initial config should be correct
        assert_eq!(active_era(), 0);

        // check the balance of a validator accounts.
        assert_eq!(Balances::total_balance(&10), 1);

        // confirm that 10 is a normal validator and gets paid at the end of the era.
        mock::start_active_era(1);

        // Initial state of 10
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
        assert_eq!(
            PowerPlant::eras_stakers(active_era(), 11),
            Exposure { total: 1000, own: 1000, others: vec![] }
        );

        // deposit the extra 100 units
        PowerPlant::bond_extra(RuntimeOrigin::signed(11), 100).unwrap();

        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000 + 100,
                active: 1000 + 100,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
        // Exposure is a snapshot! only updated after the next era update.
        assert_ne!(
            PowerPlant::eras_stakers(active_era(), 11),
            Exposure { total: 1000 + 100, own: 1000 + 100, others: vec![] }
        );

        // trigger next era.
        mock::start_active_era(2);
        assert_eq!(active_era(), 2);

        // ledger should be the same.
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000 + 100,
                active: 1000 + 100,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
        // Exposure is now updated.
        assert_eq!(
            PowerPlant::eras_stakers(active_era(), 11),
            Exposure { total: 1000 + 100, own: 1000 + 100, others: vec![] }
        );

        // Unbond almost all of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 1000).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000 + 100,
                active: 100,
                unlocking: bounded_vec![UnlockChunk { value: 1000, era: 2 + 3 }],
                claimed_rewards: bounded_vec![],
            }),
        );

        // Attempting to free the balances now will fail. 2 eras need to pass.
        assert_ok!(PowerPlant::withdraw_unbonded(RuntimeOrigin::signed(10), 0));
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000 + 100,
                active: 100,
                unlocking: bounded_vec![UnlockChunk { value: 1000, era: 2 + 3 }],
                claimed_rewards: bounded_vec![],
            }),
        );

        // trigger next era.
        mock::start_active_era(3);

        // nothing yet
        assert_ok!(PowerPlant::withdraw_unbonded(RuntimeOrigin::signed(10), 0));
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000 + 100,
                active: 100,
                unlocking: bounded_vec![UnlockChunk { value: 1000, era: 2 + 3 }],
                claimed_rewards: bounded_vec![],
            }),
        );

        // trigger next era.
        mock::start_active_era(5);

        assert_ok!(PowerPlant::withdraw_unbonded(RuntimeOrigin::signed(10), 0));
        // Now the value is free and the staking ledger is updated.
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 100,
                active: 100,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            }),
        );
    })
}

#[test]
fn many_unbond_calls_should_work() {
    ExtBuilder::default().build_and_execute(|| {
        let mut current_era = 0;
        // locked at era MaxUnlockingChunks - 1 until 3

        let max_unlocking_chunks = <<Test as Config>::MaxUnlockingChunks as Get<u32>>::get();

        for i in 0..max_unlocking_chunks - 1 {
            // There is only 1 chunk per era, so we need to be in a new era to create a chunk.
            current_era = i;
            mock::start_active_era(current_era);
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(10), 1));
        }

        current_era += 1;
        mock::start_active_era(current_era);

        // This chunk is locked at `current_era` through `current_era + 2` (because
        // `BondingDuration` == 3).
        assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(10), 1));
        assert_eq!(
            PowerPlant::ledger(10).map(|l| l.unlocking.len()).unwrap(),
            <<Test as Config>::MaxUnlockingChunks as Get<u32>>::get() as usize
        );

        // even though the number of unlocked chunks is the same as `MaxUnlockingChunks`,
        // unbonding works as expected.
        for i in current_era..(current_era + max_unlocking_chunks) - 1 {
            // There is only 1 chunk per era, so we need to be in a new era to create a chunk.
            current_era = i;
            mock::start_active_era(current_era);
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(10), 1));
        }

        // only slots within last `BondingDuration` are filled.
        assert_eq!(
            PowerPlant::ledger(10).map(|l| l.unlocking.len()).unwrap(),
            <<Test as Config>::BondingDuration>::get() as usize
        );
    })
}

#[test]
fn auto_withdraw_may_not_unlock_all_chunks() {
    ExtBuilder::default().build_and_execute(|| {
        // set `MaxUnlockingChunks` to a low number to test case when the unbonding period
        // is larger than the number of unlocking chunks available, which may result on a
        // `Error::NoMoreChunks`, even when the auto-withdraw tries to release locked chunks.
        MaxUnlockingChunks::set(1);

        let mut current_era = 0;

        // fills the chunking slots for account
        mock::start_active_era(current_era);
        assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(10), 1));

        current_era += 1;
        mock::start_active_era(current_era);

        // unbonding will fail because i) there are no remaining chunks and ii) no filled chunks
        // can be released because current chunk hasn't stay in the queue for at least
        // `BondingDuration`
        assert_noop!(PowerPlant::unbond(RuntimeOrigin::signed(10), 1), Error::<Test>::NoMoreChunks);

        // fast-forward a few eras for unbond to be successful with implicit withdraw
        current_era += 10;
        mock::start_active_era(current_era);
        assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(10), 1));
    })
}

#[test]
fn rebond_works() {
    //
    // * Should test
    // * Given an account being bonded [and chosen as a validator](not mandatory)
    // * it can unbond a portion of its funds from the stash account.
    // * it can re-bond a portion of the funds scheduled to unlock.
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        // Set payee to controller. avoids confusion
        assert_ok!(PowerPlant::set_payee(RuntimeOrigin::signed(10), RewardDestination::Controller));

        // Give account 11 some large free balance greater than total
        let _ = Balances::make_free_balance_be(&11, 1000000);

        // confirm that 10 is a normal validator and gets paid at the end of the era.
        mock::start_active_era(1);

        // Initial state of 10
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );

        mock::start_active_era(2);
        assert_eq!(active_era(), 2);

        // Try to rebond some funds. We get an error since no fund is unbonded.
        assert_noop!(
            PowerPlant::rebond(RuntimeOrigin::signed(10), 500),
            Error::<Test>::NoUnlockChunk
        );

        // Unbond almost all of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 900).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 100,
                unlocking: bounded_vec![UnlockChunk { value: 900, era: 2 + 3 }],
                claimed_rewards: bounded_vec![],
            })
        );

        // Re-bond all the funds unbonded.
        PowerPlant::rebond(RuntimeOrigin::signed(10), 900).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );

        // Unbond almost all of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 900).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 100,
                unlocking: bounded_vec![UnlockChunk { value: 900, era: 5 }],
                claimed_rewards: bounded_vec![],
            })
        );

        // Re-bond part of the funds unbonded.
        PowerPlant::rebond(RuntimeOrigin::signed(10), 500).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 600,
                unlocking: bounded_vec![UnlockChunk { value: 400, era: 5 }],
                claimed_rewards: bounded_vec![],
            })
        );

        // Re-bond the remainder of the funds unbonded.
        PowerPlant::rebond(RuntimeOrigin::signed(10), 500).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );

        // Unbond parts of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 300).unwrap();
        PowerPlant::unbond(RuntimeOrigin::signed(10), 300).unwrap();
        PowerPlant::unbond(RuntimeOrigin::signed(10), 300).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 100,
                unlocking: bounded_vec![UnlockChunk { value: 900, era: 5 }],
                claimed_rewards: bounded_vec![],
            })
        );

        // Re-bond part of the funds unbonded.
        PowerPlant::rebond(RuntimeOrigin::signed(10), 500).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 600,
                unlocking: bounded_vec![UnlockChunk { value: 400, era: 5 }],
                claimed_rewards: bounded_vec![],
            })
        );
    })
}

#[test]
fn rebond_is_fifo() {
    // Rebond should proceed by reversing the most recent bond operations.
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        // Set payee to controller. avoids confusion
        assert_ok!(PowerPlant::set_payee(RuntimeOrigin::signed(10), RewardDestination::Controller));

        // Give account 11 some large free balance greater than total
        let _ = Balances::make_free_balance_be(&11, 1000000);

        // confirm that 10 is a normal validator and gets paid at the end of the era.
        mock::start_active_era(1);

        // Initial state of 10
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );

        mock::start_active_era(2);

        // Unbond some of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 400).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 600,
                unlocking: bounded_vec![UnlockChunk { value: 400, era: 2 + 3 }],
                claimed_rewards: bounded_vec![],
            })
        );

        mock::start_active_era(3);

        // Unbond more of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 300).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 300,
                unlocking: bounded_vec![
                    UnlockChunk { value: 400, era: 2 + 3 },
                    UnlockChunk { value: 300, era: 3 + 3 },
                ],
                claimed_rewards: bounded_vec![],
            })
        );

        mock::start_active_era(4);

        // Unbond yet more of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 200).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 100,
                unlocking: bounded_vec![
                    UnlockChunk { value: 400, era: 2 + 3 },
                    UnlockChunk { value: 300, era: 3 + 3 },
                    UnlockChunk { value: 200, era: 4 + 3 },
                ],
                claimed_rewards: bounded_vec![],
            })
        );

        // Re-bond half of the unbonding funds.
        PowerPlant::rebond(RuntimeOrigin::signed(10), 400).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 500,
                unlocking: bounded_vec![
                    UnlockChunk { value: 400, era: 2 + 3 },
                    UnlockChunk { value: 100, era: 3 + 3 },
                ],
                claimed_rewards: bounded_vec![],
            })
        );
    })
}

#[test]
fn rebond_emits_right_value_in_event() {
    // When a user calls rebond with more than can be rebonded, things succeed,
    // and the rebond event emits the actual value rebonded.
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        // Set payee to controller. avoids confusion
        assert_ok!(PowerPlant::set_payee(RuntimeOrigin::signed(10), RewardDestination::Controller));

        // Give account 11 some large free balance greater than total
        let _ = Balances::make_free_balance_be(&11, 1000000);

        // confirm that 10 is a normal validator and gets paid at the end of the era.
        mock::start_active_era(1);

        // Unbond almost all of the funds in stash.
        PowerPlant::unbond(RuntimeOrigin::signed(10), 900).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 100,
                unlocking: bounded_vec![UnlockChunk { value: 900, era: 1 + 3 }],
                claimed_rewards: bounded_vec![],
            })
        );

        // Re-bond less than the total
        PowerPlant::rebond(RuntimeOrigin::signed(10), 100).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 200,
                unlocking: bounded_vec![UnlockChunk { value: 800, era: 1 + 3 }],
                claimed_rewards: bounded_vec![],
            })
        );
        // Event emitted should be correct
        assert_eq!(*staking_events().last().unwrap(), Event::Bonded { stash: 11, amount: 100 });

        // Re-bond way more than available
        PowerPlant::rebond(RuntimeOrigin::signed(10), 100_000).unwrap();
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
        // Event emitted should be correct, only 800
        assert_eq!(*staking_events().last().unwrap(), Event::Bonded { stash: 11, amount: 800 });
    });
}

#[test]
fn reward_to_stake_works() {
    ExtBuilder::default()
        .cooperate(false)
        .set_status(31, StakerStatus::Idle)
        .set_status(41, StakerStatus::Idle)
        .set_stake(21, 2000)
        .build_and_execute(|| {
            assert_eq!(PowerPlant::validator_count(), 2);
            // Confirm account 10 and 20 are validators
            assert!(<Validators<Test>>::contains_key(11) && <Validators<Test>>::contains_key(21));

            assert_eq!(PowerPlant::eras_stakers(active_era(), 11).total, 1000);
            assert_eq!(PowerPlant::eras_stakers(active_era(), 21).total, 2000);

            // Give the man some money.
            let _ = Balances::make_free_balance_be(&10, 1000);
            let _ = Balances::make_free_balance_be(&20, 1000);

            // Bypass logic and change current exposure
            ErasStakers::<Test>::insert(0, 21, Exposure { total: 69, own: 69, others: vec![] });
            <Ledger<Test>>::insert(
                20,
                StakingLedger {
                    stash: 21,
                    total: 69,
                    active: 69,
                    unlocking: Default::default(),
                    claimed_rewards: bounded_vec![],
                },
            );

            // Compute total payout now for whole duration as other parameter won't change
            let total_payout_0 = current_total_payout_for_duration(reward_time_per_era());
            let eras_total_stake = PowerPlant::eras_total_stake(active_era());
            let energy_reward_10 =
                calculate_reward(total_payout_0, eras_total_stake, 1000, Percent::from_percent(8));
            assert_eq!(controller_stash_reputation_tier(&10), Some(ReputationTier::Trailblazer(1)));
            let energy_reward_20 =
                calculate_reward(total_payout_0, eras_total_stake, 2000, Percent::from_percent(8));
            assert_eq!(controller_stash_reputation_tier(&20), Some(ReputationTier::Trailblazer(1)));

            // New era --> rewards are paid --> stakes are changed
            mock::start_active_era(1);
            mock::make_all_reward_payment(0);

            assert_eq!(PowerPlant::eras_stakers(active_era(), 11).total, 1000);
            assert_eq!(PowerPlant::eras_stakers(active_era(), 21).total, 69);

            let _10_balance = Assets::balance(VNRG::get(), 10);
            let _20_balance = Assets::balance(VNRG::get(), 20);
            assert_eq_error_rate!(_10_balance, energy_reward_10, 3);
            assert_eq_error_rate!(_20_balance, energy_reward_20, 3);

            // Trigger another new era as the info are frozen before the era start.
            mock::start_active_era(2);

            // -- new infos
            assert_eq!(PowerPlant::eras_stakers(active_era(), 11).total, 1000);
            assert_eq!(PowerPlant::eras_stakers(active_era(), 21).total, 69);
        });
}

#[test]
fn reap_stash_works() {
    ExtBuilder::default()
        .existential_deposit(10)
        .balance_factor(10)
        .build_and_execute(|| {
            // given
            assert_eq!(Balances::free_balance(10), 10);
            assert_eq!(Balances::free_balance(11), 10 * 1000);
            assert_eq!(PowerPlant::bonded(11), Some(10));

            assert!(<Ledger<Test>>::contains_key(10));
            assert!(<Bonded<Test>>::contains_key(11));
            assert!(<Validators<Test>>::contains_key(11));
            assert!(<Payee<Test>>::contains_key(11));

            // stash is not reapable
            assert_noop!(
                PowerPlant::reap_stash(RuntimeOrigin::signed(20), 11, 0),
                Error::<Test>::FundedTarget
            );
            // controller or any other account is not reapable
            assert_noop!(
                PowerPlant::reap_stash(RuntimeOrigin::signed(20), 10, 0),
                Error::<Test>::NotStash
            );

            // no easy way to cause an account to go below ED, we tweak their staking ledger
            // instead.
            Ledger::<Test>::insert(
                10,
                StakingLedger {
                    stash: 11,
                    total: 5,
                    active: 5,
                    unlocking: Default::default(),
                    claimed_rewards: bounded_vec![],
                },
            );

            // reap-able
            assert_ok!(PowerPlant::reap_stash(RuntimeOrigin::signed(20), 11, 0));

            // then
            assert!(!<Ledger<Test>>::contains_key(10));
            assert!(!<Bonded<Test>>::contains_key(11));
            assert!(!<Validators<Test>>::contains_key(11));
            assert!(!<Payee<Test>>::contains_key(11));
        });
}

#[test]
fn switching_roles() {
    // Test that it should be possible to switch between roles (cooperator, validator, idle) with
    // minimal overhead.
    ExtBuilder::default()
        // .add_staker(5, 6, 2000, status)
        .cooperate(false)
        .build_and_execute(|| {
            // Reset reward destination
            for i in &[10, 20] {
                assert_ok!(PowerPlant::set_payee(
                    RuntimeOrigin::signed(*i),
                    RewardDestination::Controller
                ));
            }

            assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

            // put some money in account that we'll use.
            for i in 1..7 {
                let _ = Balances::deposit_creating(&i, 5000);
            }

            ReputationPallet::force_set_points(
                RuntimeOrigin::root(),
                5,
                CollaborativeValidatorReputationTier::get().into(),
            )
            .unwrap();

            // add 2 cooperators
            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(1),
                2,
                2000,
                RewardDestination::Controller
            ));
            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(2), vec![(11, 750)]));

            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(3),
                4,
                500,
                RewardDestination::Controller
            ));
            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(4), vec![(21, 425)]));

            // add a new validator candidate
            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(5),
                6,
                1000,
                RewardDestination::Controller
            ));
            assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(6), ValidatorPrefs::default()));
            assert_ok!(Session::set_keys(
                RuntimeOrigin::signed(6),
                SessionKeys { other: 6.into() },
                vec![]
            ));

            mock::start_active_era(1);

            // with current cooperators 10 and 5 have the most stake
            assert_eq_uvec!(validator_controllers(), vec![6, 30, 20, 10]);

            // 2 decides to be a validator. Consequences:
            // first we need to make it have enough reputation
            assert_ok!(ReputationPallet::force_set_points(
                RuntimeOrigin::root(),
                1,
                ValidatorReputationTier::get().into()
            ));

            assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(2), ValidatorPrefs::default()));
            assert_ok!(Session::set_keys(
                RuntimeOrigin::signed(2),
                SessionKeys { other: 2.into() },
                vec![]
            ));

            mock::start_active_era(2);

            assert_eq_uvec!(validator_controllers(), vec![6, 30, 2, 20, 10]);
        });
}

#[test]
fn wrong_vote_is_moot() {
    // this test is not applicable to our implementation, but still it checks the collaboraitve
    // staking works
    ExtBuilder::default()
        .add_staker(61, 60, 500, StakerStatus::Cooperator(vec![(11, 150), (21, 100)]))
        .build_and_execute(|| {
            // the genesis validators already reflect the above vote, nonetheless start a new era.
            mock::start_active_era(1);

            // new validators
            assert_eq_uvec!(validator_controllers(), vec![30, 20, 10]);

            // our new voter is taken into account
            assert!(PowerPlant::eras_stakers(active_era(), 11).others.iter().any(|i| i.who == 61));
            assert!(PowerPlant::eras_stakers(active_era(), 21).others.iter().any(|i| i.who == 61));
        });
}

#[test]
fn bond_with_no_staked_value() {
    // Behavior when someone bonds with no staked value.
    // Particularly when they votes and the candidate is elected.
    ExtBuilder::default()
        .validator_count(3)
        .existential_deposit(5)
        .balance_factor(5)
        .cooperate(false)
        .minimum_validator_count(1)
        .build_and_execute(|| {
            // Can't bond with 1
            assert_noop!(
                PowerPlant::bond(RuntimeOrigin::signed(1), 2, 1, RewardDestination::Controller),
                Error::<Test>::InsufficientBond,
            );
            // bonded with absolute minimum value possible.
            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(1),
                2,
                5,
                RewardDestination::Controller
            ));
            assert_eq!(Balances::locks(1)[0].amount, 5);

            // unbonding even 1 will cause all to be unbonded.
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(2), 1));
            assert_eq!(
                PowerPlant::ledger(2),
                Some(StakingLedger {
                    stash: 1,
                    active: 0,
                    total: 5,
                    unlocking: bounded_vec![UnlockChunk { value: 5, era: 3 }],
                    claimed_rewards: bounded_vec![],
                })
            );

            mock::start_active_era(1);
            mock::start_active_era(2);

            // not yet removed.
            assert_ok!(PowerPlant::withdraw_unbonded(RuntimeOrigin::signed(2), 0));
            assert!(PowerPlant::ledger(2).is_some());
            assert_eq!(Balances::locks(1)[0].amount, 5);

            mock::start_active_era(3);

            // poof. Account 1 is removed from the staking system.
            assert_ok!(PowerPlant::withdraw_unbonded(RuntimeOrigin::signed(2), 0));
            assert!(PowerPlant::ledger(2).is_none());
            assert_eq!(Balances::locks(1).len(), 0);
        });
}

#[test]
fn bond_with_little_staked_value_bounded() {
    ExtBuilder::default()
        .validator_count(3)
        .cooperate(false)
        .minimum_validator_count(1)
        .build_and_execute(|| {
            // setup
            assert_ok!(PowerPlant::chill(RuntimeOrigin::signed(30)));
            assert_ok!(PowerPlant::set_payee(
                RuntimeOrigin::signed(10),
                RewardDestination::Controller
            ));
            let init_balance_2 = Assets::balance(VNRG::get(), 2);
            let init_balance_10 = Assets::balance(VNRG::get(), 10);

            // set enought reputation for the stash account
            assert_ok!(ReputationPallet::force_set_points(
                RuntimeOrigin::root(),
                1,
                ValidatorReputationTier::get().into()
            ));

            // Stingy validator.
            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(1),
                2,
                1,
                RewardDestination::Controller
            ));
            assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(2), ValidatorPrefs::default()));
            assert_ok!(Session::set_keys(
                RuntimeOrigin::signed(2),
                SessionKeys { other: 2.into() },
                vec![]
            ));

            // 1 era worth of reward. BUT, we set the timestamp after on_initialize, so outdated by
            // one block.
            let total_payout_0 = current_total_payout_for_duration(reward_time_per_era());

            reward_all_elected();
            mock::start_active_era(1);
            mock::make_all_reward_payment(0);

            // 2 is elected.
            assert_eq_uvec!(validator_controllers(), vec![20, 10, 2]);
            assert_eq!(PowerPlant::eras_stakers(active_era(), 2).total, 0);

            // Account 10 reward check
            let total_stake = ErasTotalStake::<Test>::get(0);
            let bonded = PowerPlant::ledger(10).unwrap();

            // ensuring that the energy reward for account 10 stash is calculated according to their tier
            assert_eq!(controller_stash_reputation_tier(&10), Some(ReputationTier::Trailblazer(1)),);
            let energy_reward_10_0 =
                calculate_reward(total_payout_0, total_stake, bonded.total, Percent::from_percent(8));

            assert!(!Assets::balance(VNRG::get(), 10).is_zero());

            // Old ones are rewarded.
            assert_eq_error_rate!(
                Assets::balance(VNRG::get(), 10),
                init_balance_10 + energy_reward_10_0,
                1
            );
            // no rewards paid to 2. This was initial election.
            assert_eq!(Assets::balance(VNRG::get(), 2), init_balance_2);

            // reward era 2
            let total_payout_1 = current_total_payout_for_duration(reward_time_per_era());
            reward_all_elected();
            mock::start_active_era(2);
            mock::make_all_reward_payment(1);

            assert_eq_uvec!(validator_controllers(), vec![20, 10, 2]);
            assert_eq!(PowerPlant::eras_stakers(active_era(), 2).total, 0);

            let total_stake = ErasTotalStake::<Test>::get(1);
            let bonded = PowerPlant::ledger(2).unwrap();
            let energy_reward_2 =
                calculate_reward(total_payout_1, total_stake, bonded.total, Percent::from_percent(0));
            assert_eq!(controller_stash_reputation_tier(&2), Some(ReputationTier::Vanguard(1)),);

            let bonded = PowerPlant::ledger(10).unwrap();
            let energy_reward_10_1 =
                calculate_reward(total_payout_1, total_stake, bonded.total, Percent::from_percent(8));
            assert_eq!(controller_stash_reputation_tier(&10), Some(ReputationTier::Trailblazer(1)),);


            assert!(!Assets::balance(VNRG::get(), 2).is_zero());
            assert!(!Assets::balance(VNRG::get(), 10).is_zero());

            // 2 is now rewarded.
            assert_eq_error_rate!(
                Assets::balance(VNRG::get(), 2),
                init_balance_2 + energy_reward_2,
                1
            );
            assert_eq_error_rate!(
                Assets::balance(VNRG::get(), 10),
                init_balance_10 + energy_reward_10_0 + energy_reward_10_1,
                3,
            );
        });
}

#[test]
fn reward_validator_slashing_validator_does_not_overflow() {
    ExtBuilder::default().build_and_execute(|| {
        let stake = u64::MAX as Balance * 2;
        let reward_slash = u64::MAX as Balance * 2;

        // Assert multiplication overflows in balance arithmetic.
        assert!(stake.checked_mul(reward_slash).is_none());

        // Set staker
        let _ = Balances::make_free_balance_be(&11, stake);

        let exposure = Exposure::<AccountId, Balance> { total: stake, own: stake, others: vec![] };

        // Check reward
        ErasStakers::<Test>::insert(0, 11, &exposure);
        ErasStakersClipped::<Test>::insert(0, 11, exposure);
        ErasEnergyPerStakeCurrency::<Test>::insert(0, stake);
        mock::start_active_era(1);
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 0));
        assert_eq!(Assets::balance(VNRG::get(), 10), Balance::max_value());

        // Set staker
        let _ = Balances::make_free_balance_be(&11, stake);
        let _ = Balances::make_free_balance_be(&2, stake);

        // only slashes out of bonded stake are applied. without this line, it is 0.
        PowerPlant::bond(RuntimeOrigin::signed(2), 20000, stake - 1, RewardDestination::default())
            .unwrap();
        // Override exposure of 11
        ErasStakers::<Test>::insert(
            0,
            11,
            Exposure {
                total: stake,
                own: 1,
                others: vec![IndividualExposure { who: 2, value: stake - 1 }],
            },
        );

        let reputation = ReputationPallet::reputation(11).unwrap().reputation.points();

        // Check slashing
        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(100)],
        );

        let slash = *max_slash_amount(&reputation.into());

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            *reputation - slash
        );
    })
}

#[test]
fn reward_from_authorship_event_handler_works() {
    ExtBuilder::default().build_and_execute(|| {
        use pallet_authorship::EventHandler;

        assert_eq!(<pallet_authorship::Pallet<Test>>::author(), Some(11));

        let init_reputation_11 = ReputationPallet::reputation(11).unwrap().reputation.points();
        let validator_count = <Test as crate::Config>::SessionInterface::validators().len();
        let reputation_reward =
            (pallet_reputation::NORMAL * *pallet_reputation::REPUTATION_POINTS_PER_BLOCK as f64
                - *pallet_reputation::REPUTATION_POINTS_PER_BLOCK as f64) as u64
                * validator_count as u64;

        Pallet::<Test>::note_author(11);
        Pallet::<Test>::note_author(11);

        // Not mandatory but must be coherent with rewards
        assert_eq_uvec!(Session::validators(), vec![31, 11, 21]);

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            *init_reputation_11 + reputation_reward * 2
        );
    })
}

#[test]
fn era_is_always_same_length() {
    // This ensures that the sessions is always of the same length if there is no forcing no
    // session changes.
    ExtBuilder::default().build_and_execute(|| {
        let session_per_era = <SessionsPerEra as Get<SessionIndex>>::get();

        mock::start_active_era(1);
        assert_eq!(PowerPlant::eras_start_session_index(current_era()).unwrap(), session_per_era);

        mock::start_active_era(2);
        assert_eq!(
            PowerPlant::eras_start_session_index(current_era()).unwrap(),
            session_per_era * 2u32
        );

        let session = Session::current_index();
        PowerPlant::set_force_era(Forcing::ForceNew);
        advance_session();
        advance_session();
        assert_eq!(current_era(), 3);
        assert_eq!(PowerPlant::eras_start_session_index(current_era()).unwrap(), session + 2);

        mock::start_active_era(4);
        assert_eq!(
            PowerPlant::eras_start_session_index(current_era()).unwrap(),
            session + 2u32 + session_per_era
        );
    });
}

#[test]
fn offence_forces_new_era() {
    ExtBuilder::default().build_and_execute(|| {
        on_offence_now(
            &[
                OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                },
                OffenceDetails {
                    offender: (31, PowerPlant::eras_stakers(active_era(), 31)),
                    reporters: vec![],
                },
            ],
            &[Perbill::from_percent(5), Perbill::from_percent(5)],
        );

        assert_eq!(PowerPlant::force_era(), Forcing::ForceNew);
    });
}

#[test]
fn offence_ensures_new_era_without_clobbering() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(PowerPlant::force_new_era_always(RuntimeOrigin::root()));
        assert_eq!(PowerPlant::force_era(), Forcing::ForceAlways);

        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(5)],
        );

        assert_eq!(PowerPlant::force_era(), Forcing::ForceAlways);
    });
}

#[test]
fn offence_deselects_validator_even_when_slash_is_zero() {
    ExtBuilder::default().build_and_execute(|| {
        assert!(Session::validators().contains(&11));
        assert!(Session::validators().contains(&21));
        assert!(<Validators<Test>>::contains_key(11));
        assert!(<Validators<Test>>::contains_key(21));

        on_offence_now(
            &[
                OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                },
                OffenceDetails {
                    offender: (21, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                },
            ],
            &[Perbill::from_percent(0), Perbill::from_percent(0)],
        );

        assert_eq!(PowerPlant::force_era(), Forcing::ForceNew);
        assert!(!<Validators<Test>>::contains_key(11));
        assert!(!<Validators<Test>>::contains_key(21));

        mock::start_active_era(1);

        assert!(!Session::validators().contains(&11));
        assert!(!Session::validators().contains(&21));
        assert!(!<Validators<Test>>::contains_key(11));
        assert!(!<Validators<Test>>::contains_key(21));
    });
}

#[test]
fn slashing_performed_according_exposure() {
    // This test checks that slashing is performed according the exposure (or more precisely,
    // historical exposure), not the current balance.
    //
    // above was for the original test, but our implementation is different. we have slashing per
    // ratio to some validator reputation threshold.
    ExtBuilder::default().build_and_execute(|| {
        assert_eq!(PowerPlant::eras_stakers(active_era(), 11).own, 1000);
        let init_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();

        // Handle an offence with a historical exposure.
        on_offence_now(
            &[OffenceDetails {
                offender: (11, Exposure { total: 500, own: 500, others: vec![] }),
                reporters: vec![],
            }],
            &[Perbill::from_percent(50)],
        );

        let slash = *max_slash_amount(&init_reputation_11.into()) / 2;

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            init_reputation_11 - slash
        );
    });
}

#[test]
fn slash_in_old_span_does_not_deselect() {
    ExtBuilder::default().build_and_execute(|| {
        // Mutate reputation of the stashes, so that they won't be chilled after 95% slash
        let new_rep = Reputation::from(ReputationTier::Trailblazer(2));
        pallet_reputation::AccountReputation::<Test>::mutate(11, |record| {
            record.get_or_insert(ReputationRecord::with_blocknumber(0)).reputation =
                new_rep.clone();
        });
        pallet_reputation::AccountReputation::<Test>::mutate(21, |record| {
            record.get_or_insert(ReputationRecord::with_blocknumber(0)).reputation =
                new_rep.clone();
        });

        mock::start_active_era(1);

        assert!(<Validators<Test>>::contains_key(11));
        assert!(Session::validators().contains(&11));
        assert!(<Validators<Test>>::contains_key(21));
        assert!(Session::validators().contains(&21));

        on_offence_now(
            &[
                OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                },
                OffenceDetails {
                    offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                    reporters: vec![],
                },
            ],
            &[Perbill::from_percent(0), Perbill::from_percent(0)],
        );

        assert_eq!(PowerPlant::force_era(), Forcing::ForceNew);
        assert!(!<Validators<Test>>::contains_key(11));
        assert!(!<Validators<Test>>::contains_key(21));

        mock::start_active_era(2);

        PowerPlant::validate(RuntimeOrigin::signed(10), Default::default()).unwrap();
        PowerPlant::validate(RuntimeOrigin::signed(20), Default::default()).unwrap();
        assert_eq!(PowerPlant::force_era(), Forcing::NotForcing);
        assert!(<Validators<Test>>::contains_key(11));
        assert!(!Session::validators().contains(&11));
        assert!(<Validators<Test>>::contains_key(21));
        assert!(!Session::validators().contains(&21));

        mock::start_active_era(3);

        // this staker is in a new slashing span now, having re-registered after
        // their prior slash.

        on_offence_in_era(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(0)],
            1,
            DisableStrategy::WhenSlashed,
        );
        on_offence_in_era(
            &[OffenceDetails {
                offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(0)],
            1,
            DisableStrategy::WhenSlashed,
        );

        // the validator doesn't get chilled again
        assert!(Validators::<Test>::iter().any(|(stash, _)| stash == 11));
        assert!(Validators::<Test>::iter().any(|(stash, _)| stash == 21));

        // but we are still forcing a new era
        assert_eq!(PowerPlant::force_era(), Forcing::ForceNew);

        on_offence_in_era(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![],
            }],
            // NOTE: A 100% slash here would clean up the account, causing de-registration.
            &[Perbill::from_percent(95)],
            1,
            DisableStrategy::WhenSlashed,
        );
        on_offence_in_era(
            &[OffenceDetails {
                offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                reporters: vec![],
            }],
            // NOTE: A 100% slash here would clean up the account, causing de-registration.
            &[Perbill::from_percent(95)],
            1,
            DisableStrategy::WhenSlashed,
        );

        // the validator doesn't get chilled again
        assert!(Validators::<Test>::iter().any(|(stash, _)| stash == 11));
        assert!(Validators::<Test>::iter().any(|(stash, _)| stash == 21));

        // but it's disabled
        assert!(is_disabled(10));
        assert!(is_disabled(20));
        // and we are still forcing a new era
        assert_eq!(PowerPlant::force_era(), Forcing::ForceNew);
    });
}

#[test]
fn reporters_receive_their_slice() {
    ExtBuilder::default().build_and_execute(|| {
        let initial_reputation_1 = *ReputationPallet::reputation(1).unwrap().reputation.points();
        let initial_reputation_2 = *ReputationPallet::reputation(2).unwrap().reputation.points();

        let offender_before = *ReputationPallet::reputation(11).unwrap().reputation.points();
        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![1, 2],
            }],
            &[Perbill::from_percent(50)],
        );
        let offender_after = *ReputationPallet::reputation(11).unwrap().reputation.points();

        // F1 * slash * reward_proportion / num_of_reporters * energy_per_reputation
        let slash = offender_before - offender_after;
        let reward = slash / 2 / 10; // F! (50%) and reward prop (10%)
        let reward_each = reward / 2; // split between reporters
        assert!(!reward_each.is_zero());
        assert_eq!(
            *ReputationPallet::reputation(1).unwrap().reputation.points(),
            initial_reputation_1 + reward_each
        );
        assert_eq!(
            *ReputationPallet::reputation(2).unwrap().reputation.points(),
            initial_reputation_2 + reward_each
        );
    });
}

#[test]
fn subsequent_reports_in_same_span_pay_out_less() {
    // This test verifies that the reporters of the offence receive their slice from the slashed
    // amount, but less and less if they submit multiple reports in one span.
    ExtBuilder::default().build_and_execute(|| {
        let before_offence = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reputation_1 = *ReputationPallet::reputation(1).unwrap().reputation.points();
        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![1],
            }],
            &[Perbill::from_percent(20)],
        );
        let after_offence = *ReputationPallet::reputation(11).unwrap().reputation.points();

        // F1 * slash * reward_proportion * energy_per_reputation
        let slash = before_offence - after_offence;
        let reward = slash / 2 / 10; // F1 (50%) and reward prop (10%)
        assert_eq_error_rate!(
            *ReputationPallet::reputation(1).unwrap().reputation.points(),
            initial_reputation_1 + reward,
            2
        );

        let before_offence = *ReputationPallet::reputation(11).unwrap().reputation.points();
        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![1],
            }],
            &[Perbill::from_percent(50)],
        );
        let after_offence = *ReputationPallet::reputation(11).unwrap().reputation.points();
        //
        let prior_payout = reward;
        // F1 * slash * reward_proportion * energy_per_reputation
        let slash = before_offence - after_offence;
        let reward = slash / 2 / 10; // F1 (50%) and reward prop (10%)

        assert_eq_error_rate!(
            *ReputationPallet::reputation(1).unwrap().reputation.points(),
            initial_reputation_1 + reward + prior_payout + prior_payout / 2,
            2
        );
    });
}

#[test]
fn invulnerables_are_not_slashed() {
    // For invulnerable validators no slashing is performed.
    ExtBuilder::default().invulnerables(vec![11]).build_and_execute(|| {
        assert_eq!(Balances::free_balance(11), 1000);
        assert_eq!(Balances::free_balance(21), 2000);

        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let exposure_21 = PowerPlant::eras_stakers(active_era(), 21);
        let initial_reputation_21 = *ReputationPallet::reputation(21).unwrap().reputation.points();

        // make cooperators reputation non-zero value
        for other in &exposure_21.others {
            assert_ok!(ReputationPallet::force_set_points(
                RuntimeOrigin::root(),
                other.who,
                ValidatorReputationTier::get().into(),
            ));
        }

        let cooperator_reputations: Vec<_> = exposure_21
            .others
            .iter()
            .map(|o| *ReputationPallet::reputation(o.who).unwrap().reputation.points())
            .collect();

        on_offence_now(
            &[
                OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                },
                OffenceDetails {
                    offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                    reporters: vec![],
                },
            ],
            &[Perbill::from_percent(50), Perbill::from_percent(20)],
        );

        // The validator 11 hasn't been slashed, but 21 has been.
        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11
        );

        let slash_21 = *max_slash_amount(&initial_reputation_21.into()) * 2 / 10;
        let affter_slash_reputation_21 = initial_reputation_21 - slash_21;
        assert_eq!(
            *ReputationPallet::reputation(21).unwrap().reputation.points(),
            affter_slash_reputation_21
        );

        let slash_prop = Perbill::from_rational(affter_slash_reputation_21, initial_reputation_21);

        // ensure that cooperators were slashed as well.
        for (initial_reputation, other) in
            cooperator_reputations.into_iter().zip(exposure_21.others)
        {
            assert_eq!(
                *ReputationPallet::reputation(other.who).unwrap().reputation.points(),
                slash_prop * initial_reputation
            );
        }
    });
}

#[test]
fn dont_slash_if_fraction_is_zero() {
    // Don't slash if the fraction is zero.
    ExtBuilder::default().build_and_execute(|| {
        let initial_reptutation_11 = ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reptutation_21 = ReputationPallet::reputation(21).unwrap().reputation.points();

        on_offence_now(
            &[
                OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                },
                OffenceDetails {
                    offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                    reporters: vec![],
                },
            ],
            &[Perbill::from_percent(0), Perbill::from_percent(0)],
        );

        // The validator hasn't been slashed. The new era is not forced.
        assert_eq!(
            ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reptutation_11
        );
        assert_eq!(
            ReputationPallet::reputation(21).unwrap().reputation.points(),
            initial_reptutation_21
        );
        assert_eq!(PowerPlant::force_era(), Forcing::ForceNew);
    });
}

#[ignore]
#[test]
fn only_slash_for_max_in_era() {
    // multiple slashes within one era are only applied if it is more than any previous slash in the
    // same era.
    ExtBuilder::default().build_and_execute(|| {
        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reputation_21 = *ReputationPallet::reputation(21).unwrap().reputation.points();

        on_offence_now(
            &[
                OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                },
                OffenceDetails {
                    offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                    reporters: vec![],
                },
            ],
            &[Perbill::from_percent(50), Perbill::from_percent(50)],
        );

        let slash_21 = *max_slash_amount(&initial_reputation_21.into()) / 2;

        // The validator has been slashed and has been force-chilled.
        let affter_slash_reputation_21 =
            *ReputationPallet::reputation(21).unwrap().reputation.points();
        assert_eq_error_rate!(affter_slash_reputation_21, initial_reputation_21 - slash_21, 1);
        assert_eq!(PowerPlant::force_era(), Forcing::ForceNew);

        on_offence_now(
            &[OffenceDetails {
                offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(25)],
        );

        // The validator has not been slashed additionally.
        assert_eq!(
            *ReputationPallet::reputation(21).unwrap().reputation.points(),
            affter_slash_reputation_21
        );
        // let mut slash_11 = *max_slash_amount(&initial_reputation_11.into()) / 2;
        let slash_11 = *max_slash_amount(&initial_reputation_11.into()) * 6 / 10;

        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(60)],
        );

        // The validator got slashed 10% more.
        assert_eq_error_rate!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 - slash_11,
            1
        );
    })
}

#[test]
fn garbage_collection_after_slashing() {
    // ensures that `SlashingSpans` and `SpanSlash` of an account is removed after reaping.
    ExtBuilder::default()
        .existential_deposit(2)
        .balance_factor(2)
        .build_and_execute(|| {
            let initial_reputation_11 =
                *ReputationPallet::reputation(11).unwrap().reputation.points();

            on_offence_now(
                &[OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                }],
                &[Perbill::from_percent(10)],
            );

            let slash_11 = *max_slash_amount(&initial_reputation_11.into()) / 10;

            assert_eq_error_rate!(
                *ReputationPallet::reputation(11).unwrap().reputation.points(),
                initial_reputation_11 - slash_11,
                1
            );
            assert!(SlashingSpans::<Test>::get(11).is_some());
            assert_eq_error_rate!(**SpanSlash::<Test>::get((11, 0)).amount(), slash_11, 1);

            let reputation_11 = ReputationPallet::reputation(11).unwrap().reputation;

            on_offence_now(
                &[OffenceDetails {
                    offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                    reporters: vec![],
                }],
                &[Perbill::from_percent(100)],
            );

            let slash_11 = *max_slash_amount(&reputation_11);

            // validator and cooperator slash in era are garbage-collected by era change,
            // so we don't test those here.

            assert_eq_error_rate!(
                *ReputationPallet::reputation(11).unwrap().reputation.points(),
                initial_reputation_11 - slash_11,
                1
            );

            // we need to slash the stake to be able to reap the stash
            let _ = Balances::slash(&11, Balances::total_balance(&11));

            Ledger::<Test>::insert(
                10,
                StakingLedger {
                    stash: 11,
                    total: 0,
                    active: 0,
                    unlocking: Default::default(),
                    claimed_rewards: bounded_vec![],
                },
            );

            assert_eq!(Balances::free_balance(11), 2);
            assert_eq!(Balances::total_balance(&11), 2);

            let slashing_spans = SlashingSpans::<Test>::get(11).unwrap();
            assert_eq!(slashing_spans.iter().count(), 2);

            // reap_stash respects num_slashing_spans so that weight is accurate
            // we can't reap it if it has funds, so we remove them first
            assert_noop!(
                PowerPlant::reap_stash(RuntimeOrigin::signed(20), 11, 0),
                Error::<Test>::IncorrectSlashingSpans
            );
            assert_ok!(PowerPlant::reap_stash(RuntimeOrigin::signed(20), 11, 2));

            assert!(SlashingSpans::<Test>::get(11).is_none());
            assert_eq!(SpanSlash::<Test>::get((11, 0)).amount(), &0.into());
        })
}

#[test]
fn garbage_collection_on_window_pruning() {
    // ensures that `ValidatorSlashInEra` and `CooperatorSlashInEra` are cleared after
    // `BondingDuration`.
    ExtBuilder::default().build_and_execute(|| {
        mock::start_active_era(1);

        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        assert!(initial_reputation_11 > 0);
        let initial_reputation_101 = initial_reputation_11;
        let now = active_era();

        // let's make the reputation of the cooperator equal to the validator's stash (just make it
        // to be non-zero)
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            101,
            initial_reputation_101.into()
        ));

        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(now, 11)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(10)],
        );

        let slash_11 = *max_slash_amount(&initial_reputation_11.into()) / 10;
        let slash_101 = *max_slash_amount(&initial_reputation_101.into()) / 10;

        assert_eq_error_rate!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 - slash_11,
            1
        );
        assert_eq_error_rate!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 - slash_101,
            1
        );

        assert!(ValidatorSlashInEra::<Test>::get(now, 11).is_some());
        assert!(CooperatorSlashInEra::<Test>::get(now, 101).is_some());

        // + 1 because we have to exit the bonding window.
        for era in (0..(BondingDuration::get() + 1)).map(|offset| offset + now + 1) {
            assert!(ValidatorSlashInEra::<Test>::get(now, 11).is_some());
            assert!(CooperatorSlashInEra::<Test>::get(now, 101).is_some());

            mock::start_active_era(era);
        }

        assert!(ValidatorSlashInEra::<Test>::get(now, 11).is_none());
        assert!(CooperatorSlashInEra::<Test>::get(now, 101).is_none());
    })
}

#[test]
fn slashes_are_summed_across_spans() {
    ExtBuilder::default().build_and_execute(|| {
        mock::start_active_era(1);
        mock::start_active_era(2);
        mock::start_active_era(3);

        let initial_reputation_21 = *ReputationPallet::reputation(21).unwrap().reputation.points();

        let get_span = |account| SlashingSpans::<Test>::get(account).unwrap();

        on_offence_now(
            &[OffenceDetails {
                offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(10)],
        );

        let reputation_after_slash = *ReputationPallet::reputation(21).unwrap().reputation.points();

        let expected_spans = vec![
            slashing::SlashingSpan { index: 1, start: 4, length: None },
            slashing::SlashingSpan { index: 0, start: 0, length: Some(4) },
        ];

        let slash_21 = *max_slash_amount(&initial_reputation_21.into()) / 10;
        assert_eq!(get_span(21).iter().collect::<Vec<_>>(), expected_spans);
        assert_eq_error_rate!(reputation_after_slash, initial_reputation_21 - slash_21, 2);

        // 21 has been force-chilled. re-signal intent to validate.
        PowerPlant::validate(RuntimeOrigin::signed(20), Default::default()).unwrap();

        mock::start_active_era(4);
        let before_slash_21 = *ReputationPallet::reputation(21).unwrap().reputation.points();

        on_offence_now(
            &[OffenceDetails {
                offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(10)],
        );

        let expected_spans = vec![
            slashing::SlashingSpan { index: 2, start: 5, length: None },
            slashing::SlashingSpan { index: 1, start: 4, length: Some(1) },
            slashing::SlashingSpan { index: 0, start: 0, length: Some(4) },
        ];

        let slash_21 = *max_slash_amount(&before_slash_21.into()) / 10;
        assert_eq!(get_span(21).iter().collect::<Vec<_>>(), expected_spans);
        assert_eq_error_rate!(
            *ReputationPallet::reputation(21).unwrap().reputation.points(),
            reputation_after_slash - slash_21 + reputation_per_era(),
            2
        );
    });
}

#[test]
fn deferred_slashes_are_deferred() {
    ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
        mock::start_active_era(1);

        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reputation_101 = initial_reputation_11;
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            101,
            initial_reputation_101.into()
        ));

        System::reset_events();

        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(10)],
        );

        let slash_11 = *max_slash_amount(&initial_reputation_11.into()) / 10;
        let slash_101 = *max_slash_amount(&initial_reputation_101.into()) / 10;

        // cooperations are not removed regardless of the deferring.
        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
            vec![&11, &21]
        );

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101
        );

        mock::start_active_era(2);

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 + reputation_per_era()
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 + reputation_per_era()
        );

        mock::start_active_era(3);

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 + reputation_per_era() * 2
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 + reputation_per_era() * 2
        );

        // at the start of era 4, slashes from era 1 are processed,
        // after being deferred for at least 2 full eras.
        mock::start_active_era(4);

        let rep_rewards = reputation_per_sessions(2) + reputation_per_era() * 2;

        assert_eq_error_rate!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            (initial_reputation_11 + rep_rewards) - slash_11,
            1
        );
        assert_eq_error_rate!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            (initial_reputation_101 + rep_rewards) - slash_101,
            1
        );

        assert!(matches!(
            staking_events_since_last_call().as_slice(),
            &[
                Event::Chilled { stash: 11 },
                Event::SlashReported { validator: 11, slash_era: 1, .. },
                Event::StakersElected,
                ..,
                Event::Slashed { staker: 11, amount: ReputationPoint(3399277,) },
                Event::Slashed { staker: 101, amount: ReputationPoint(3399277,) },
            ]
        ));
    })
}

#[test]
fn retroactive_deferred_slashes_two_eras_before() {
    ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
        assert_eq!(BondingDuration::get(), 3);

        mock::start_active_era(1);
        let exposure_11_at_era_1 = PowerPlant::eras_stakers(active_era(), 11);

        mock::start_active_era(3);

        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
            &[&11, &21]
        );

        System::reset_events();
        on_offence_in_era(
            &[OffenceDetails { offender: (11, exposure_11_at_era_1), reporters: vec![] }],
            &[Perbill::from_percent(10)],
            1, // should be deferred for two full eras, and applied at the beginning of era 4.
            DisableStrategy::Never,
        );

        mock::start_active_era(4);

        assert!(matches!(
            staking_events_since_last_call().as_slice(),
            &[
                Event::Chilled { stash: 11 },
                Event::SlashReported { validator: 11, slash_era: 1, .. },
                ..,
                Event::Slashed { staker: 11, amount: ReputationPoint(3399313) },
                Event::Slashed { staker: 101, amount: ReputationPoint(54) },
            ]
        ));
    })
}

#[test]
fn retroactive_deferred_slashes_one_before() {
    ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
        assert_eq!(BondingDuration::get(), 3);

        mock::start_active_era(1);
        let exposure_11_at_era_1 = PowerPlant::eras_stakers(active_era(), 11);

        // unbond at slash era.
        mock::start_active_era(2);
        assert_ok!(PowerPlant::chill(RuntimeOrigin::signed(10)));
        assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(10), 100));

        mock::start_active_era(3);
        System::reset_events();

        let reputation_before_slash =
            *ReputationPallet::reputation(11).unwrap().reputation.points();
        on_offence_in_era(
            &[OffenceDetails { offender: (11, exposure_11_at_era_1), reporters: vec![] }],
            &[Perbill::from_percent(10)],
            2, // should be deferred for two full eras, and applied at the beginning of era 5.
            DisableStrategy::Never,
        );

        mock::start_active_era(4);

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            reputation_before_slash + reputation_per_era()
        );
        // slash happens after the next line.

        mock::start_active_era(5);
        assert!(matches!(
            staking_events_since_last_call().as_slice(),
            &[
                Event::SlashReported { validator: 11, slash_era: 2, .. },
                ..,
                Event::Slashed { staker: 11, amount: ReputationPoint(3399313) },
                Event::Slashed { staker: 101, amount: ReputationPoint(54) },
            ]
        ));
    })
}

#[test]
fn staker_cannot_bail_deferred_slash() {
    // as long as SlashDeferDuration is less than BondingDuration, this should not be possible.
    ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
        mock::start_active_era(1);

        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reputation_101 = initial_reputation_11;
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            101,
            initial_reputation_101.into()
        ));

        on_offence_now(
            &[OffenceDetails {
                offender: (11, PowerPlant::eras_stakers(active_era(), 11)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(10)],
        );

        let slash_11 = *max_slash_amount(&initial_reputation_11.into()) / 10;
        let slash_101 = *max_slash_amount(&initial_reputation_101.into()) / 10;

        // now we chill
        assert_ok!(PowerPlant::chill(RuntimeOrigin::signed(100)));
        assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(100), 500));

        assert_eq!(PowerPlant::current_era().unwrap(), 1);
        assert_eq!(active_era(), 1);

        // no slash yet.
        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101
        );

        // no slash yet.
        mock::start_active_era(2);
        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 + reputation_per_era()
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 + reputation_per_era()
        );
        assert_eq!(PowerPlant::current_era().unwrap(), 2);
        assert_eq!(active_era(), 2);

        // no slash yet.
        mock::start_active_era(3);
        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 + reputation_per_era() * 2
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 + reputation_per_era() * 2
        );
        assert_eq!(PowerPlant::current_era().unwrap(), 3);
        assert_eq!(active_era(), 3);

        // and cannot yet unbond:
        assert_storage_noop!(assert!(
            PowerPlant::withdraw_unbonded(RuntimeOrigin::signed(100), 0).is_ok()
        ));
        assert_eq!(
            Ledger::<Test>::get(100).unwrap().unlocking.into_inner(),
            vec![UnlockChunk { era: 4u32, value: 500 as Balance }],
        );

        // at the start of era 4, slashes from era 1 are processed,
        // after being deferred for at least 2 full eras.
        mock::start_active_era(4);

        assert_eq_error_rate!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 + reputation_per_era() * 3
                - reputation_per_sessions(1)
                - slash_11,
            1
        );
        assert_eq_error_rate!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 + reputation_per_era() * 3
                - reputation_per_sessions(1)
                - slash_101,
            1
        );

        // and the leftover of the funds can now be unbonded.
    })
}

#[test]
fn remove_deferred() {
    ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
        mock::start_active_era(1);

        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reputation_101 = initial_reputation_11;
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            101,
            initial_reputation_101.into()
        ));

        let exposure = PowerPlant::eras_stakers(active_era(), 11);

        // deferred to start of era 4.
        on_offence_now(
            &[OffenceDetails { offender: (11, exposure.clone()), reporters: vec![] }],
            &[Perbill::from_percent(10)],
        );

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101
        );

        mock::start_active_era(2);

        // reported later, but deferred to start of era 4 as well.
        System::reset_events();
        on_offence_in_era(
            &[OffenceDetails { offender: (11, exposure.clone()), reporters: vec![] }],
            &[Perbill::from_percent(15)],
            1,
            DisableStrategy::WhenSlashed,
        );

        // fails if empty
        assert_noop!(
            PowerPlant::cancel_deferred_slash(RuntimeOrigin::root(), 1, vec![]),
            Error::<Test>::EmptyTargets
        );

        // cancel one of them.
        assert_ok!(PowerPlant::cancel_deferred_slash(RuntimeOrigin::root(), 4, vec![0]));

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 + reputation_per_era()
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 + reputation_per_era()
        );

        mock::start_active_era(3);

        assert_eq!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 + reputation_per_era() * 2
        );
        assert_eq!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 + reputation_per_era() * 2
        );

        // at the start of era 4, slashes from era 1 are processed,
        // after being deferred for at least 2 full eras.
        mock::start_active_era(4);

        // the first slash for 10% was cancelled, but the 15% one not.
        assert!(matches!(
            staking_events_since_last_call().as_slice(),
            &[
                Event::SlashReported { validator: 11, slash_era: 1, .. },
                ..,
                Event::Slashed { staker: 11, amount: ReputationPoint(1699665) },
                Event::Slashed { staker: 101, amount: ReputationPoint(1699647) },
            ]
        ));
    })
}

#[test]
fn remove_multi_deferred() {
    ExtBuilder::default().slash_defer_duration(2).build_and_execute(|| {
        mock::start_active_era(1);

        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reputation_101 = initial_reputation_11;
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            101,
            initial_reputation_101.into()
        ));

        let exposure = PowerPlant::eras_stakers(active_era(), 11);

        assert_eq!(Session::validators(), [31, 21, 11]);

        on_offence_now(
            &[OffenceDetails { offender: (11, exposure.clone()), reporters: vec![] }],
            &[Perbill::from_percent(10)],
        );

        on_offence_now(
            &[OffenceDetails {
                offender: (21, PowerPlant::eras_stakers(active_era(), 21)),
                reporters: vec![],
            }],
            &[Perbill::from_percent(10)],
        );

        on_offence_now(
            &[OffenceDetails { offender: (11, exposure.clone()), reporters: vec![] }],
            &[Perbill::from_percent(25)],
        );

        // not a validator
        on_offence_now(
            &[OffenceDetails { offender: (42, exposure.clone()), reporters: vec![] }],
            &[Perbill::from_percent(25)],
        );

        // not a validator
        on_offence_now(
            &[OffenceDetails { offender: (69, exposure.clone()), reporters: vec![] }],
            &[Perbill::from_percent(25)],
        );

        // 11, 21, 11
        assert_eq!(UnappliedSlashes::<Test>::get(4).len(), 3);

        // fails if list is not sorted
        assert_noop!(
            PowerPlant::cancel_deferred_slash(RuntimeOrigin::root(), 1, vec![2, 0]),
            Error::<Test>::NotSortedAndUnique
        );
        // fails if list is not unique
        assert_noop!(
            PowerPlant::cancel_deferred_slash(RuntimeOrigin::root(), 1, vec![2, 2]),
            Error::<Test>::NotSortedAndUnique
        );
        // fails if bad index
        assert_noop!(
            PowerPlant::cancel_deferred_slash(RuntimeOrigin::root(), 1, vec![1, 2, 3, 4, 5]),
            Error::<Test>::InvalidSlashIndex
        );

        assert_ok!(PowerPlant::cancel_deferred_slash(RuntimeOrigin::root(), 4, vec![0, 2]));

        let slashes = UnappliedSlashes::<Test>::get(4);
        assert_eq!(slashes.len(), 1);
        assert_eq!(slashes[0].validator, 21);
    })
}

#[test]
fn slash_kicks_validators_not_cooperators_and_disables_cooperator_for_kicked_validator() {
    ExtBuilder::default().build_and_execute(|| {
        mock::start_active_era(1);
        assert_eq_uvec!(Session::validators(), vec![31, 21, 11]);

        let initial_reputation_11 = *ReputationPallet::reputation(11).unwrap().reputation.points();
        let initial_reputation_101 = initial_reputation_11;
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            101,
            initial_reputation_101.into()
        ));

        // 100 has approval for 11 as of now
        assert!(PowerPlant::cooperators(101).unwrap().targets.contains_key(&11));

        // 11 and 21 both have the support of 100
        let exposure_11 = PowerPlant::eras_stakers(active_era(), 11);
        let exposure_21 = PowerPlant::eras_stakers(active_era(), 21);

        assert_eq!(exposure_11.total, 1000 + 200);
        assert_eq!(exposure_21.total, 1000 + 300);

        on_offence_now(
            &[OffenceDetails { offender: (11, exposure_11.clone()), reporters: vec![] }],
            &[Perbill::from_percent(10)],
        );

        assert!(matches!(
            staking_events_since_last_call().as_slice(),
            &[
                ..,
                Event::Chilled { stash: 11 },
                Event::SlashReported { validator: 11, slash_era: 1, .. },
                Event::Slashed { staker: 11, amount: ReputationPoint(3399277) },
                Event::Slashed { staker: 101, amount: ReputationPoint(3399277) },
            ]
        ));

        // post-slash balance
        let slash_11 = *max_slash_amount(&initial_reputation_11.into()) / 10;
        assert_eq_error_rate!(
            *ReputationPallet::reputation(11).unwrap().reputation.points(),
            initial_reputation_11 - slash_11,
            2
        );
        let slash_101 = *max_slash_amount(&initial_reputation_101.into()) / 10;
        assert_eq_error_rate!(
            *ReputationPallet::reputation(101).unwrap().reputation.points(),
            initial_reputation_101 - slash_101,
            2
        );

        // check that validator was chilled.
        assert!(Validators::<Test>::iter().all(|(stash, _)| stash != 11));

        // actually re-bond the slashed validator
        assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(10), Default::default()));
    });
}

#[test]
fn non_slashable_offence_doesnt_disable_validator() {
    ExtBuilder::default().build_and_execute(|| {
        mock::start_active_era(1);
        assert_eq_uvec!(Session::validators(), vec![31, 21, 11]);

        let exposure_11 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 11);
        let exposure_21 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 21);

        // offence with no slash associated
        on_offence_now(
            &[OffenceDetails { offender: (11, exposure_11.clone()), reporters: vec![] }],
            &[Perbill::zero()],
        );

        // it does NOT affect the cooperator.
        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
            vec![&11, &21]
        );

        // offence that slashes 25% of reputation
        on_offence_now(
            &[OffenceDetails { offender: (21, exposure_21.clone()), reporters: vec![] }],
            &[Perbill::from_percent(25)],
        );

        // it DOES NOT affect the cooperator.
        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
            vec![&11, &21]
        );

        assert!(matches!(
            staking_events_since_last_call().as_slice(),
            &[
                ..,
                Event::Chilled { stash: 11 },
                Event::SlashReported { validator: 11, slash_era: 1, .. },
                Event::Chilled { stash: 21 },
                Event::ForceEra { mode: Forcing::ForceNew },
                Event::SlashReported { validator: 21, slash_era: 1, .. },
                Event::Slashed { staker: 21, amount: ReputationPoint(8498191) },
                Event::Slashed { staker: 101, amount: ReputationPoint(45) },
            ]
        ));

        // the offence for validator 10 wasn't slashable so it wasn't disabled
        assert!(!is_disabled(10));
        // whereas validator 20 gets disabled
        assert!(is_disabled(20));
    });
}

#[test]
fn slashing_independent_of_disabling_validator() {
    ExtBuilder::default().build_and_execute(|| {
        mock::start_active_era(1);
        assert_eq_uvec!(Session::validators(), vec![31, 21, 11]);

        let exposure_11 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 11);
        let exposure_21 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 21);

        let now = PowerPlant::active_era().unwrap().index;

        // offence with no slash associated, BUT disabling
        on_offence_in_era(
            &[OffenceDetails { offender: (11, exposure_11.clone()), reporters: vec![] }],
            &[Perbill::zero()],
            now,
            DisableStrategy::Always,
        );

        // cooperation remains untouched.
        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
            vec![&11, &21]
        );

        // offence that slashes 25% of the bond, BUT not disabling
        on_offence_in_era(
            &[OffenceDetails { offender: (21, exposure_21.clone()), reporters: vec![] }],
            &[Perbill::from_percent(25)],
            now,
            DisableStrategy::Never,
        );

        // cooperation remains untouched.
        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
            vec![&11, &21]
        );

        assert!(matches!(
            staking_events_since_last_call().as_slice(),
            &[
                ..,
                Event::Chilled { stash: 11 },
                Event::SlashReported { validator: 11, slash_era: 1, .. },
                Event::Chilled { stash: 21 },
                Event::ForceEra { mode: Forcing::ForceNew },
                Event::SlashReported { validator: 21, slash_era: 1, .. },
                Event::Slashed { staker: 21, amount: ReputationPoint(8498191) },
                Event::Slashed { staker: 101, amount: ReputationPoint(45) },
            ]
        ));

        // the offence for validator 10 was explicitly disabled
        assert!(is_disabled(10));
        // whereas validator 20 is explicitly not disabled
        assert!(!is_disabled(20));
    });
}

#[test]
fn offence_threshold_triggers_new_era() {
    ExtBuilder::default()
        .validator_count(4)
        .set_status(41, StakerStatus::Validator)
        .build_and_execute(|| {
            mock::start_active_era(1);
            assert_eq_uvec!(Session::validators(), vec![41, 31, 21, 11]);

            assert_eq!(
                <Test as Config>::OffendingValidatorsThreshold::get(),
                Perbill::from_percent(75),
            );

            // we have 4 validators and an offending validator threshold of 75%,
            // once the third validator commits an offence a new era should be forced

            let exposure_11 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 11);
            let exposure_21 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 21);
            let exposure_31 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 31);

            on_offence_now(
                &[OffenceDetails { offender: (11, exposure_11.clone()), reporters: vec![] }],
                &[Perbill::zero()],
            );

            assert_eq!(ForceEra::<Test>::get(), Forcing::NotForcing);

            on_offence_now(
                &[OffenceDetails { offender: (21, exposure_21.clone()), reporters: vec![] }],
                &[Perbill::zero()],
            );

            assert_eq!(ForceEra::<Test>::get(), Forcing::NotForcing);

            on_offence_now(
                &[OffenceDetails { offender: (31, exposure_31.clone()), reporters: vec![] }],
                &[Perbill::zero()],
            );

            assert_eq!(ForceEra::<Test>::get(), Forcing::ForceNew);
        });
}

#[test]
fn disabled_validators_are_kept_disabled_for_whole_era() {
    ExtBuilder::default()
        .validator_count(4)
        .set_status(41, StakerStatus::Validator)
        .build_and_execute(|| {
            mock::start_active_era(1);
            assert_eq_uvec!(Session::validators(), vec![41, 31, 21, 11]);
            assert_eq!(<Test as Config>::SessionsPerEra::get(), 3);

            let exposure_11 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 11);
            let exposure_21 = PowerPlant::eras_stakers(PowerPlant::active_era().unwrap().index, 21);

            on_offence_now(
                &[OffenceDetails { offender: (11, exposure_11.clone()), reporters: vec![] }],
                &[Perbill::zero()],
            );

            on_offence_now(
                &[OffenceDetails { offender: (21, exposure_21.clone()), reporters: vec![] }],
                &[Perbill::from_percent(25)],
            );

            // cooperations are not updated.
            assert_eq!(
                PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
                vec![&11, &21]
            );

            // validator 10 should not be disabled since the offence wasn't slashable
            assert!(!is_disabled(10));
            // validator 20 gets disabled since it got slashed
            assert!(is_disabled(20));

            advance_session();

            // disabled validators should carry-on through all sessions in the era
            assert!(!is_disabled(10));
            assert!(is_disabled(20));

            // validator 10 should now get disabled
            on_offence_now(
                &[OffenceDetails { offender: (11, exposure_11.clone()), reporters: vec![] }],
                &[Perbill::from_percent(25)],
            );

            // cooperations are not updated.
            assert_eq!(
                PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
                vec![&11, &21]
            );

            advance_session();

            // and both are disabled in the last session of the era
            assert!(is_disabled(10));
            assert!(is_disabled(20));

            mock::start_active_era(2);

            // when a new era starts disabled validators get cleared
            assert!(!is_disabled(10));
            assert!(!is_disabled(20));
        });
}

#[test]
fn claim_reward_at_the_last_era_and_no_double_claim_and_invalid_claim() {
    // should check that:
    // * rewards get paid until history_depth for both validators and cooperators
    // * an invalid era to claim doesn't update last_reward
    // * double claim of one era fails
    ExtBuilder::default().cooperate(true).build_and_execute(|| {
        // Consumed weight for all payout_stakers dispatches that fail
        let err_weight = <Test as Config>::ThisWeightInfo::payout_stakers_alive_staked(0);

        // Check state
        Payee::<Test>::insert(11, RewardDestination::Controller);
        Payee::<Test>::insert(101, RewardDestination::Controller);

        Pallet::<Test>::reward_by_ids(vec![(11, 1.into())]);

        mock::start_active_era(1);

        Pallet::<Test>::reward_by_ids(vec![(11, 1.into())]);

        mock::start_active_era(2);

        Pallet::<Test>::reward_by_ids(vec![(11, 1.into())]);

        mock::start_active_era(HistoryDepth::get() + 1);

        // This is the latest planned era in staking, not the active era
        let current_era = PowerPlant::current_era().unwrap();

        // Last kept is 1:
        assert!(current_era - HistoryDepth::get() == 1);
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 0),
            // Fail: Era out of history
            Error::<Test>::InvalidEraToReward.with_weight(err_weight)
        );
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 1));
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 2));
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 2),
            // Fail: Double claim
            Error::<Test>::AlreadyClaimed.with_weight(err_weight)
        );
    });
}

#[test]
fn zero_slash_keeps_cooperators() {
    ExtBuilder::default().build_and_execute(|| {
        mock::start_active_era(1);

        assert_eq!(Balances::free_balance(11), 1000);

        let exposure = PowerPlant::eras_stakers(active_era(), 11);
        assert_eq!(Balances::free_balance(101), 2000);

        on_offence_now(
            &[OffenceDetails { offender: (11, exposure.clone()), reporters: vec![] }],
            &[Perbill::from_percent(0)],
        );

        // 11 is still removed..
        assert!(Validators::<Test>::iter().all(|(stash, _)| stash != 11));
        // but their cooperations are kept.
        assert_eq!(
            PowerPlant::cooperators(101).unwrap().targets.keys().collect::<Vec<_>>(),
            vec![&11, &21]
        );
    });
}

#[test]
fn six_session_delay() {
    ExtBuilder::default().initialize_first_session(false).build_and_execute(|| {
        use pallet_session::SessionManager;

        let val_set = Session::validators();
        let init_session = Session::current_index();
        let init_active_era = active_era();

        // pallet-session is delaying session by one, thus the next session to plan is +2.
        assert_eq!(<PowerPlant as SessionManager<_>>::new_session(init_session + 2), None);
        assert_eq!(
            <PowerPlant as SessionManager<_>>::new_session(init_session + 3),
            Some(val_set.clone())
        );
        assert_eq!(<PowerPlant as SessionManager<_>>::new_session(init_session + 4), None);
        assert_eq!(<PowerPlant as SessionManager<_>>::new_session(init_session + 5), None);
        assert_eq!(
            <PowerPlant as SessionManager<_>>::new_session(init_session + 6),
            Some(val_set.clone())
        );

        <PowerPlant as SessionManager<_>>::end_session(init_session);
        <PowerPlant as SessionManager<_>>::start_session(init_session + 1);
        assert_eq!(active_era(), init_active_era);

        <PowerPlant as SessionManager<_>>::end_session(init_session + 1);
        <PowerPlant as SessionManager<_>>::start_session(init_session + 2);
        assert_eq!(active_era(), init_active_era);

        // Reward current era
        PowerPlant::reward_by_ids(vec![(11, 1.into())]);

        // New active era is triggered here.
        <PowerPlant as SessionManager<_>>::end_session(init_session + 2);
        <PowerPlant as SessionManager<_>>::start_session(init_session + 3);
        assert_eq!(active_era(), init_active_era + 1);

        <PowerPlant as SessionManager<_>>::end_session(init_session + 3);
        <PowerPlant as SessionManager<_>>::start_session(init_session + 4);
        assert_eq!(active_era(), init_active_era + 1);

        <PowerPlant as SessionManager<_>>::end_session(init_session + 4);
        <PowerPlant as SessionManager<_>>::start_session(init_session + 5);
        assert_eq!(active_era(), init_active_era + 1);

        // Reward current era
        PowerPlant::reward_by_ids(vec![(21, 2.into())]);

        // New active era is triggered here.
        <PowerPlant as SessionManager<_>>::end_session(init_session + 5);
        <PowerPlant as SessionManager<_>>::start_session(init_session + 6);
        assert_eq!(active_era(), init_active_era + 2);
    });
}

#[test]
fn test_max_cooperator_rewarded_per_validator_and_cant_steal_someone_else_reward() {
    ExtBuilder::default().validator_count(0).build_and_execute(|| {
        make_validator(1010, 1011, 1000);
        mock::start_active_era(1);
        let max_rewarded: u32 =
            <<Test as Config>::MaxCooperatorRewardedPerValidator as Get<_>>::get();
        let cooperators_num = max_rewarded + 3;

        for i in 0..cooperators_num {
            let stash = 10_000 + i as AccountId;
            let controller = 20_000 + i as AccountId;
            let balance = 10_000 + i as Balance;
            Balances::make_free_balance_be(&stash, balance);
            assert_ok!(Assets::mint(RuntimeOrigin::signed(1), VNRG::get().into(), stash, balance));
            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(stash),
                controller,
                balance,
                RewardDestination::Stash
            ));
            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(controller), vec![(1011, 100)]));
        }
        mock::start_active_era(2);

        let exposure = PowerPlant::eras_stakers(2, 1011);

        assert_eq!(exposure.others.len() as u32, cooperators_num);

        Pallet::<Test>::reward_by_ids(vec![(1011, 1.into())]);
        // compute and ensure the reward amount is greater than zero.
        let _ = current_total_payout_for_duration(reward_time_per_era());

        mock::start_active_era(3);
        mock::make_all_reward_payment(2);

        let energy_rate = ErasEnergyPerStakeCurrency::<Test>::get(1).unwrap();
        let total_payout_10 = exposure.total * energy_rate;
        let cooperator_part = Perbill::from_rational(100, exposure.total);
        let cooperator_reward = cooperator_part * total_payout_10;
        mock::start_active_era(4);

        for i in 0..cooperators_num {
            let stash = 10_000 + i as AccountId;
            let balance = 10_000 + i as Balance;

            if i < max_rewarded {
                assert_eq!(Assets::balance(VNRG::get(), stash), balance + cooperator_reward);
            } else {
                assert_eq!(Assets::balance(VNRG::get(), stash), balance);
            }
        }
    });
}

#[test]
fn test_payout_stakers() {
    // Test that payout_stakers work in general, including that only the top
    // `T::MaxCooperatorRewardedPerValidator` cooperators are rewarded.
    ExtBuilder::default().has_stakers(false).build_and_execute(|| {
        let balance = 1000;
        // Track the exposure of the validator and all cooperators.
        // let mut total_exposure = balance;
        // Track the exposure of the validator and the cooperators that will get paid out.
        // let mut payout_exposure = balance;
        // Create a validator:
        make_validator(10, 11, balance);
        assert_eq!(Validators::<Test>::count(), 1);

        // Create cooperators, targeting stash of validators
        for i in 0..100 {
            let bond_amount = balance + i as Balance;
            bond_cooperator(1000 + i, 100 + i, bond_amount, vec![(11, bond_amount)]);
        }

        mock::start_active_era(1);
        let exposure = PowerPlant::eras_stakers(1, 11);
        // adding additional 8%, since validator have a Trailblazer(1) reputation tier
        let mut payout_part =
            FixedU128::from_rational(exposure.own, exposure.total) * FixedU128::from_float(1.08);

        for coop in &exposure.others[36..] {
            // only top value coops are rewarded
            let coop_part = Perbill::from_rational(coop.value, exposure.total);
            payout_part = payout_part + coop_part.into();
        }

        PowerPlant::reward_by_ids(vec![(11, 1.into())]);
        // compute and ensure the reward amount is greater than zero.
        let payout = current_total_payout_for_duration(reward_time_per_era());
        let actual_paid_out = payout_part.saturating_mul_int(payout);

        mock::start_active_era(2);

        let pre_payout_total_issuance = Assets::total_supply(VNRG::get());
        RewardOnUnbalanceWasCalled::set(false);
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 1));
        assert_eq_error_rate!(
            Assets::total_supply(VNRG::get()),
            pre_payout_total_issuance + actual_paid_out,
            45
        );
        assert!(RewardOnUnbalanceWasCalled::get());

        // Top 64 cooperators of validator 11 automatically paid out, including the validator
        // Validator payout goes to controller.
        assert!(Assets::balance(VNRG::get(), 10) > 0);
        for i in 36..100 {
            assert!(Assets::balance(VNRG::get(), 100 + i) > 0);
        }
        // The bottom 36 do not
        for i in 0..36 {
            assert_eq!(Assets::balance(VNRG::get(), 100 + i), 0);
        }

        // We track rewards in `claimed_rewards` vec
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![1]
            })
        );

        for i in 3..16 {
            PowerPlant::reward_by_ids(vec![(11, 1.into())]);

            // compute and ensure the reward amount is greater than zero.
            let payout = current_total_payout_for_duration(reward_time_per_era());
            let actual_paid_out = payout_part.saturating_mul_int(payout);

            let pre_payout_total_issuance = Assets::total_supply(VNRG::get());

            mock::start_active_era(i);
            RewardOnUnbalanceWasCalled::set(false);
            assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, i - 1));
            assert_eq_error_rate!(
                Assets::total_supply(VNRG::get()),
                pre_payout_total_issuance + actual_paid_out,
                45
            );
            assert!(RewardOnUnbalanceWasCalled::get());
        }

        // We track rewards in `claimed_rewards` vec
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: (1..=14).collect::<Vec<_>>().try_into().unwrap()
            })
        );

        let last_era = 99;
        let history_depth = HistoryDepth::get();
        let expected_last_reward_era = last_era - 1;
        let expected_start_reward_era = last_era - history_depth;
        for i in 16..=last_era {
            PowerPlant::reward_by_ids(vec![(11, 1.into())]);
            // compute and ensure the reward amount is greater than zero.
            let _ = current_total_payout_for_duration(reward_time_per_era());
            mock::start_active_era(i);
        }

        // We clean it up as history passes
        assert_ok!(PowerPlant::payout_stakers(
            RuntimeOrigin::signed(1337),
            11,
            expected_start_reward_era
        ));
        assert_ok!(PowerPlant::payout_stakers(
            RuntimeOrigin::signed(1337),
            11,
            expected_last_reward_era
        ));
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![expected_start_reward_era, expected_last_reward_era]
            })
        );

        // Out of order claims works.
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 69));
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 23));
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 42));
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![
                    expected_start_reward_era,
                    23,
                    42,
                    69,
                    expected_last_reward_era
                ]
            })
        );
    });
}

#[test]
fn payout_stakers_handles_basic_errors() {
    // Here we will test payouts handle all errors.
    ExtBuilder::default().has_stakers(false).build_and_execute(|| {
        // Consumed weight for all payout_stakers dispatches that fail
        let err_weight = <Test as Config>::ThisWeightInfo::payout_stakers_alive_staked(0);

        // Same setup as the test above
        let balance = 1000;
        make_validator(10, 11, balance);

        // Create cooperators, targeting stash
        for i in 0..100 {
            let bond = balance + i as Balance;
            bond_cooperator(1000 + i, 100 + i, bond, vec![(11, bond)]);
        }

        mock::start_active_era(1);

        // compute and ensure the reward amount is greater than zero.
        let _ = current_total_payout_for_duration(reward_time_per_era());

        mock::start_active_era(2);

        // Wrong Era, too big
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 2),
            Error::<Test>::InvalidEraToReward.with_weight(err_weight)
        );
        // Wrong Staker
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 10, 1),
            Error::<Test>::NotStash.with_weight(err_weight)
        );

        let last_era = 99;
        for i in 3..=last_era {
            // compute and ensure the reward amount is greater than zero.
            let _ = current_total_payout_for_duration(reward_time_per_era());
            mock::start_active_era(i);
        }

        let history_depth = HistoryDepth::get();
        let expected_last_reward_era = last_era - 1;
        let expected_start_reward_era = last_era - history_depth;

        // We are at era last_era=99. Given history_depth=80, we should be able
        // to payout era starting from expected_start_reward_era=19 through
        // expected_last_reward_era=98 (80 total eras), but not 18 or 99.
        assert_noop!(
            PowerPlant::payout_stakers(
                RuntimeOrigin::signed(1337),
                11,
                expected_start_reward_era - 1
            ),
            Error::<Test>::InvalidEraToReward.with_weight(err_weight)
        );
        assert_noop!(
            PowerPlant::payout_stakers(
                RuntimeOrigin::signed(1337),
                11,
                expected_last_reward_era + 1
            ),
            Error::<Test>::InvalidEraToReward.with_weight(err_weight)
        );
        assert_ok!(PowerPlant::payout_stakers(
            RuntimeOrigin::signed(1337),
            11,
            expected_start_reward_era
        ));
        assert_ok!(PowerPlant::payout_stakers(
            RuntimeOrigin::signed(1337),
            11,
            expected_last_reward_era
        ));

        // Can't claim again
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, expected_start_reward_era),
            Error::<Test>::AlreadyClaimed.with_weight(err_weight)
        );
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, expected_last_reward_era),
            Error::<Test>::AlreadyClaimed.with_weight(err_weight)
        );
    });
}

#[test]
fn payout_stakers_handles_weight_refund() {
    // Note: this test relies on the assumption that `payout_stakers_alive_staked` is solely used by
    // `payout_stakers` to calculate the weight of each payout op.
    ExtBuilder::default().has_stakers(false).build_and_execute(|| {
        let max_coop_rewarded =
            <<Test as Config>::MaxCooperatorRewardedPerValidator as Get<_>>::get();
        // Make sure the configured value is meaningful for our use.
        assert!(max_coop_rewarded >= 4);
        let half_max_coop_rewarded = max_coop_rewarded / 2;
        // Sanity check our max and half max cooperator quantities.
        assert!(half_max_coop_rewarded > 0);
        assert!(max_coop_rewarded > half_max_coop_rewarded);

        let max_coop_rewarded_weight =
            <Test as Config>::ThisWeightInfo::payout_stakers_alive_staked(max_coop_rewarded);
        let half_max_coop_rewarded_weight =
            <Test as Config>::ThisWeightInfo::payout_stakers_alive_staked(half_max_coop_rewarded);
        let zero_coop_payouts_weight =
            <Test as Config>::ThisWeightInfo::payout_stakers_alive_staked(0);
        assert!(zero_coop_payouts_weight.any_gt(Weight::zero()));
        assert!(half_max_coop_rewarded_weight.any_gt(zero_coop_payouts_weight));
        assert!(max_coop_rewarded_weight.any_gt(half_max_coop_rewarded_weight));

        let balance = 1000;
        make_validator(10, 11, balance);

        // Era 1
        start_active_era(1);

        // Reward just the validator.
        PowerPlant::reward_by_ids(vec![(11, 1.into())]);

        // Add some `half_max_nom_rewarded` cooperators who will start backing the validator in the
        // next era.
        for i in 0..half_max_coop_rewarded {
            bond_cooperator(
                (1000 + i).into(),
                (100 + i).into(),
                balance + i as Balance,
                vec![(11, 100)],
            );
        }

        // Era 2
        start_active_era(2);

        // Collect payouts when there are no cooperators
        let call =
            TestCall::PowerPlant(StakingCall::payout_stakers { validator_stash: 11, era: 1 });
        let info = call.get_dispatch_info();
        let result = call.dispatch(RuntimeOrigin::signed(20));
        assert_ok!(result);
        assert_eq!(extract_actual_weight(&result, &info), zero_coop_payouts_weight);

        // The validator is not rewarded in this era; so there will be zero payouts to claim for
        // this era.

        // Era 3
        start_active_era(3);

        // Collect payouts for an era where the validator did not receive any points.
        let call =
            TestCall::PowerPlant(StakingCall::payout_stakers { validator_stash: 11, era: 2 });
        let info = call.get_dispatch_info();
        let result = call.dispatch(RuntimeOrigin::signed(20));
        assert_ok!(result);
        assert_eq!(extract_actual_weight(&result, &info), half_max_coop_rewarded_weight);

        // Reward the validator and its cooperators.
        PowerPlant::reward_by_ids(vec![(11, 1.into())]);

        // Era 4
        start_active_era(4);

        // Collect payouts when the validator has `half_max_nom_rewarded` cooperators.
        let call =
            TestCall::PowerPlant(StakingCall::payout_stakers { validator_stash: 11, era: 3 });
        let info = call.get_dispatch_info();
        let result = call.dispatch(RuntimeOrigin::signed(20));
        assert_ok!(result);
        assert_eq!(extract_actual_weight(&result, &info), half_max_coop_rewarded_weight);

        // Add enough cooperators so that we are at the limit. They will be active cooperators
        // in the next era.
        for i in half_max_coop_rewarded..max_coop_rewarded {
            bond_cooperator(
                (1000 + i).into(),
                (100 + i).into(),
                balance + i as Balance,
                vec![(11, 100)],
            );
        }

        // Era 5
        start_active_era(5);
        // We now have `max_nom_rewarded` cooperators actively cooperating our validator.

        // Reward the validator so we can collect for everyone in the next era.
        PowerPlant::reward_by_ids(vec![(11, 1.into())]);

        // Era 6
        start_active_era(6);

        // Collect payouts when the validator had `half_max_coop_rewarded` cooperators.
        let call =
            TestCall::PowerPlant(StakingCall::payout_stakers { validator_stash: 11, era: 5 });
        let info = call.get_dispatch_info();
        let result = call.dispatch(RuntimeOrigin::signed(20));
        assert_ok!(result);
        assert_eq!(extract_actual_weight(&result, &info), max_coop_rewarded_weight);

        // Try and collect payouts for an era that has already been collected.
        let call =
            TestCall::PowerPlant(StakingCall::payout_stakers { validator_stash: 11, era: 5 });
        let info = call.get_dispatch_info();
        let result = call.dispatch(RuntimeOrigin::signed(20));
        assert!(result.is_err());
        // When there is an error the consumed weight == weight when there are 0 cooperator payouts.
        assert_eq!(extract_actual_weight(&result, &info), zero_coop_payouts_weight);
    });
}

#[test]
fn bond_during_era_correctly_populates_claimed_rewards() {
    ExtBuilder::default().has_stakers(false).build_and_execute(|| {
        // Era = None
        make_validator(8, 9, 1000);
        assert_eq!(
            PowerPlant::ledger(8),
            Some(StakingLedger {
                stash: 9,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: bounded_vec![],
            })
        );
        mock::start_active_era(5);
        make_validator(10, 11, 1000);
        assert_eq!(
            PowerPlant::ledger(10),
            Some(StakingLedger {
                stash: 11,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: (0..5).collect::<Vec<_>>().try_into().unwrap(),
            })
        );

        // make sure only era upto history depth is stored
        let current_era = 99;
        let last_reward_era = 99 - HistoryDepth::get();
        mock::start_active_era(current_era);
        make_validator(12, 13, 1000);
        assert_eq!(
            PowerPlant::ledger(12),
            Some(StakingLedger {
                stash: 13,
                total: 1000,
                active: 1000,
                unlocking: Default::default(),
                claimed_rewards: (last_reward_era..current_era)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            })
        );
    });
}

#[test]
fn payout_creates_controller() {
    ExtBuilder::default().has_stakers(false).build_and_execute(|| {
        let balance = 1000;
        // Create a validator:
        make_validator(10, 11, balance);

        // Create a stash/controller pair
        bond_cooperator(1234, 1337, 100, vec![(11, 100)]);

        // kill controller
        assert_ok!(Balances::transfer_allow_death(RuntimeOrigin::signed(1337), 1234, 100));
        assert_eq!(Balances::free_balance(1337), 0);

        mock::start_active_era(1);
        // compute and ensure the reward amount is greater than zero.
        let _ = current_total_payout_for_duration(reward_time_per_era());
        mock::start_active_era(2);
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 1));

        // Controller is created
        assert!(Assets::balance(VNRG::get(), 1337) > 0);
    })
}

#[test]
fn payout_to_any_account_works() {
    ExtBuilder::default().has_stakers(false).build_and_execute(|| {
        let balance = 1000;
        // Create a validator:
        make_validator(10, 11, balance); // Default(64)

        // Create a stash/controller pair
        bond_cooperator(1234, 1337, 100, vec![(11, 100)]);

        // Update payout location
        assert_ok!(PowerPlant::set_payee(
            RuntimeOrigin::signed(1337),
            RewardDestination::Account(42)
        ));

        // Reward Destination account doesn't exist
        assert_eq!(Balances::free_balance(42), 0);

        mock::start_active_era(1);
        // compute and ensure the reward amount is greater than zero.
        let _ = current_total_payout_for_duration(reward_time_per_era());
        mock::start_active_era(2);
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(1337), 11, 1));

        // Payment is successful
        assert!(Assets::balance(VNRG::get(), 42) > 0);
    })
}

#[test]
fn session_buffering_with_offset() {
    // similar to live-chains, have some offset for the first session
    ExtBuilder::default()
        .offset(2)
        .period(5)
        .session_per_era(5)
        .build_and_execute(|| {
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 0);

            start_session(1);
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 1);
            assert_eq!(System::block_number(), 2);

            start_session(2);
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 2);
            assert_eq!(System::block_number(), 7);

            start_session(3);
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 3);
            assert_eq!(System::block_number(), 12);

            // active era is lagging behind by one session, because of how session module works.
            start_session(4);
            assert_eq!(current_era(), 1);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 4);
            assert_eq!(System::block_number(), 17);

            start_session(5);
            assert_eq!(current_era(), 1);
            assert_eq!(active_era(), 1);
            assert_eq!(Session::current_index(), 5);
            assert_eq!(System::block_number(), 22);

            // go all the way to active 2.
            start_active_era(2);
            assert_eq!(current_era(), 2);
            assert_eq!(active_era(), 2);
            assert_eq!(Session::current_index(), 10);
        });
}

#[test]
fn session_buffering_no_offset() {
    // no offset, first session starts immediately
    ExtBuilder::default()
        .offset(0)
        .period(5)
        .session_per_era(5)
        .build_and_execute(|| {
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 0);

            start_session(1);
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 1);
            assert_eq!(System::block_number(), 5);

            start_session(2);
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 2);
            assert_eq!(System::block_number(), 10);

            start_session(3);
            assert_eq!(current_era(), 0);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 3);
            assert_eq!(System::block_number(), 15);

            // active era is lagging behind by one session, because of how session module works.
            start_session(4);
            assert_eq!(current_era(), 1);
            assert_eq!(active_era(), 0);
            assert_eq!(Session::current_index(), 4);
            assert_eq!(System::block_number(), 20);

            start_session(5);
            assert_eq!(current_era(), 1);
            assert_eq!(active_era(), 1);
            assert_eq!(Session::current_index(), 5);
            assert_eq!(System::block_number(), 25);

            // go all the way to active 2.
            start_active_era(2);
            assert_eq!(current_era(), 2);
            assert_eq!(active_era(), 2);
            assert_eq!(Session::current_index(), 10);
        });
}

#[test]
fn cannot_rebond_to_lower_than_ed() {
    ExtBuilder::default()
        .existential_deposit(10)
        .balance_factor(10)
        .build_and_execute(|| {
            // initial stuff.
            assert_eq!(
                PowerPlant::ledger(20).unwrap(),
                StakingLedger {
                    stash: 21,
                    total: 10 * 1000,
                    active: 10 * 1000,
                    unlocking: Default::default(),
                    claimed_rewards: bounded_vec![],
                }
            );

            // unbond all of it. must be chilled first.
            assert_ok!(PowerPlant::chill(RuntimeOrigin::signed(20)));
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(20), 10 * 1000));
            assert_eq!(
                PowerPlant::ledger(20).unwrap(),
                StakingLedger {
                    stash: 21,
                    total: 10 * 1000,
                    active: 0,
                    unlocking: bounded_vec![UnlockChunk { value: 10 * 1000, era: 3 }],
                    claimed_rewards: bounded_vec![],
                }
            );

            // now bond a wee bit more
            assert_noop!(
                PowerPlant::rebond(RuntimeOrigin::signed(20), 5),
                Error::<Test>::InsufficientBond
            );
        })
}

#[test]
fn cannot_bond_extra_to_lower_than_ed() {
    ExtBuilder::default()
        .existential_deposit(10)
        .balance_factor(10)
        .build_and_execute(|| {
            // initial stuff.
            assert_eq!(
                PowerPlant::ledger(20).unwrap(),
                StakingLedger {
                    stash: 21,
                    total: 10 * 1000,
                    active: 10 * 1000,
                    unlocking: Default::default(),
                    claimed_rewards: bounded_vec![],
                }
            );

            // unbond all of it. must be chilled first.
            assert_ok!(PowerPlant::chill(RuntimeOrigin::signed(20)));
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(20), 10 * 1000));
            assert_eq!(
                PowerPlant::ledger(20).unwrap(),
                StakingLedger {
                    stash: 21,
                    total: 10 * 1000,
                    active: 0,
                    unlocking: bounded_vec![UnlockChunk { value: 10 * 1000, era: 3 }],
                    claimed_rewards: bounded_vec![],
                }
            );

            // now bond a wee bit more
            assert_noop!(
                PowerPlant::bond_extra(RuntimeOrigin::signed(21), 5),
                Error::<Test>::InsufficientBond,
            );
        })
}

#[test]
fn do_not_die_when_active_is_ed() {
    let ed = 10;
    ExtBuilder::default()
        .existential_deposit(ed)
        .balance_factor(ed)
        .build_and_execute(|| {
            // given
            assert_eq!(
                PowerPlant::ledger(20).unwrap(),
                StakingLedger {
                    stash: 21,
                    total: 1000 * ed,
                    active: 1000 * ed,
                    unlocking: Default::default(),
                    claimed_rewards: bounded_vec![],
                }
            );

            // when unbond all of it except ed.
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(20), 999 * ed));
            start_active_era(3);
            assert_ok!(PowerPlant::withdraw_unbonded(RuntimeOrigin::signed(20), 100));

            // then
            assert_eq!(
                PowerPlant::ledger(20).unwrap(),
                StakingLedger {
                    stash: 21,
                    total: ed,
                    active: ed,
                    unlocking: Default::default(),
                    claimed_rewards: bounded_vec![],
                }
            );
        })
}

#[test]
fn on_finalize_weight_is_nonzero() {
    ExtBuilder::default().build_and_execute(|| {
        let on_finalize_weight = <Test as frame_system::Config>::DbWeight::get().reads(1);
        assert!(<PowerPlant as Hooks<u64>>::on_initialize(1).all_gte(on_finalize_weight));
    })
}

#[test]
fn min_bond_checks_work() {
    ExtBuilder::default()
        .existential_deposit(100)
        .balance_factor(100)
        .min_cooperator_bond(1_000)
        .min_validator_bond(1_500)
        .build_and_execute(|| {
            // 500 is not enough for any role
            assert_ok!(PowerPlant::bond(
                RuntimeOrigin::signed(3),
                4,
                500,
                RewardDestination::Controller
            ));
            assert_noop!(
                PowerPlant::cooperate(RuntimeOrigin::signed(4), vec![(11, 100)]),
                Error::<Test>::InsufficientBond
            );

            assert_ok!(ReputationPallet::force_set_points(
                RuntimeOrigin::root(),
                3,
                CollaborativeValidatorReputationTier::get().into()
            ));

            assert_noop!(
                PowerPlant::validate(RuntimeOrigin::signed(4), ValidatorPrefs::default()),
                Error::<Test>::InsufficientBond,
            );

            // 1000 is enough for cooperator
            assert_ok!(PowerPlant::bond_extra(RuntimeOrigin::signed(3), 500));
            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(4), vec![(11, 10)]));
            assert_noop!(
                PowerPlant::validate(RuntimeOrigin::signed(4), ValidatorPrefs::default()),
                Error::<Test>::InsufficientBond,
            );

            // 1500 is enough for validator
            assert_ok!(PowerPlant::bond_extra(RuntimeOrigin::signed(3), 500));
            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(4), vec![(11, 100)]));
            assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(4), ValidatorPrefs::default()));

            // Can't unbond anything as validator
            assert_noop!(
                PowerPlant::unbond(RuntimeOrigin::signed(4), 500),
                Error::<Test>::InsufficientBond
            );

            // Once they are a cooperator, they can unbond 500
            assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(4), vec![(11, 100)]));
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(4), 500));
            assert_noop!(
                PowerPlant::unbond(RuntimeOrigin::signed(4), 500),
                Error::<Test>::InsufficientBond
            );

            // Once they are chilled they can unbond everything
            assert_ok!(PowerPlant::chill(RuntimeOrigin::signed(4)));
            assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(4), 1000));
        })
}

#[test]
fn chill_other_works() {
    ExtBuilder::default()
        .existential_deposit(100)
        .balance_factor(100)
        .min_cooperator_bond(1_000)
        .min_validator_bond(1_500)
        .build_and_execute(|| {
            let initial_validators = Validators::<Test>::count();
            let initial_cooperators = Cooperators::<Test>::count();
            for i in 0..15 {
                let a = 4 * i;
                let b = 4 * i + 1;
                let c = 4 * i + 2;
                let d = 4 * i + 3;
                Balances::make_free_balance_be(&a, 100_000);
                Balances::make_free_balance_be(&b, 100_000);
                Balances::make_free_balance_be(&c, 100_000);
                Balances::make_free_balance_be(&d, 100_000);

                // Cooperator
                assert_ok!(PowerPlant::bond(
                    RuntimeOrigin::signed(a),
                    b,
                    1000,
                    RewardDestination::Controller
                ));
                assert_ok!(PowerPlant::cooperate(RuntimeOrigin::signed(b), vec![(11, 100)]));

                // Validator
                assert_ok!(PowerPlant::bond(
                    RuntimeOrigin::signed(c),
                    d,
                    1500,
                    RewardDestination::Controller
                ));
                assert_ok!(ReputationPallet::force_set_points(
                    RuntimeOrigin::root(),
                    c,
                    ValidatorReputationTier::get().into(),
                ));
                assert_ok!(PowerPlant::validate(
                    RuntimeOrigin::signed(d),
                    ValidatorPrefs::default()
                ));
            }

            // To chill other users, we need to:
            // * Set a minimum bond amount
            // * Set a limit
            // * Set a threshold
            //
            // If any of these are missing, we do not have enough information to allow the
            // `chill_other` to succeed from one user to another.

            // Can't chill these users
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 11),
                Error::<Test>::CannotChillOther
            );
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 3),
                Error::<Test>::CannotChillOther
            );

            // Change the minimum bond... but no limits.
            assert_ok!(PowerPlant::set_staking_configs(
                RuntimeOrigin::root(),
                ConfigOp::Set(1_500),
                ConfigOp::Set(2_000),
                ConfigOp::Remove,
                ConfigOp::Remove,
                ConfigOp::Remove,
                ConfigOp::Remove
            ));

            // Still can't chill these users
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 11),
                Error::<Test>::CannotChillOther
            );
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 3),
                Error::<Test>::CannotChillOther
            );

            // Add limits, but no threshold
            assert_ok!(PowerPlant::set_staking_configs(
                RuntimeOrigin::root(),
                ConfigOp::Noop,
                ConfigOp::Noop,
                ConfigOp::Set(10),
                ConfigOp::Set(10),
                ConfigOp::Noop,
                ConfigOp::Noop
            ));

            // Still can't chill these users
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 11),
                Error::<Test>::CannotChillOther
            );
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 3),
                Error::<Test>::CannotChillOther
            );

            // Add threshold, but no limits
            assert_ok!(PowerPlant::set_staking_configs(
                RuntimeOrigin::root(),
                ConfigOp::Noop,
                ConfigOp::Noop,
                ConfigOp::Remove,
                ConfigOp::Remove,
                ConfigOp::Noop,
                ConfigOp::Noop
            ));

            // Still can't chill these users
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 11),
                Error::<Test>::CannotChillOther
            );
            assert_noop!(
                PowerPlant::chill_other(RuntimeOrigin::signed(1337), 3),
                Error::<Test>::CannotChillOther
            );

            // Add threshold and limits
            assert_ok!(PowerPlant::set_staking_configs(
                RuntimeOrigin::root(),
                ConfigOp::Noop,
                ConfigOp::Noop,
                ConfigOp::Set(10),
                ConfigOp::Set(10),
                ConfigOp::Set(Percent::from_percent(75)),
                ConfigOp::Noop
            ));

            // 16 people total because tests start with 2 active one
            assert_eq!(Cooperators::<Test>::count(), 15 + initial_cooperators);
            assert_eq!(Validators::<Test>::count(), 15 + initial_validators); // 18

            // Users can now be chilled down to 7 people, so we try to remove 9 of them (starting
            // with 16)
            for i in 11..15 {
                let b = 4 * i + 1;
                let d = 4 * i + 3;
                assert_ok!(PowerPlant::chill_other(RuntimeOrigin::signed(1337), b));
                assert_ok!(PowerPlant::chill_other(RuntimeOrigin::signed(1337), d));
            }

            // chill a validator. Limit is reached, chill-able.
            assert_eq!(Validators::<Test>::count(), 14);
            assert_ok!(PowerPlant::chill_other(RuntimeOrigin::signed(1337), 3));
        })
}

#[test]
fn capped_stakers_works() {
    ExtBuilder::default().build_and_execute(|| {
        let validator_count = Validators::<Test>::count();
        assert_eq!(validator_count, 3);
        let cooperator_count = Cooperators::<Test>::count();
        assert_eq!(cooperator_count, 1);

        // Change the maximums
        let max = 10;
        assert_ok!(PowerPlant::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Set(10),
            ConfigOp::Set(10),
            ConfigOp::Set(max),
            ConfigOp::Set(max),
            ConfigOp::Remove,
            ConfigOp::Remove,
        ));

        let validators_offset = 10_000_000;
        // can create `max - validator_count` validators
        let mut some_existing_validator = AccountId::default();
        for i in 0..max - validator_count {
            let controller = i as AccountId * 2 + validators_offset;
            let stash = controller + 1;
            make_validator(controller, stash, 1000);
            some_existing_validator = controller;
        }

        // but no more
        bond(some_existing_validator + 2, some_existing_validator + 1, 1000);
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            some_existing_validator + 1,
            CollaborativeValidatorReputationTier::get().into()
        ));
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            some_existing_validator + 2,
            CollaborativeValidatorReputationTier::get().into()
        ));

        assert_noop!(
            PowerPlant::validate(
                RuntimeOrigin::signed(some_existing_validator + 1),
                ValidatorPrefs::default()
            ),
            Error::<Test>::TooManyValidators,
        );

        let cooperators_offset = 20_000_000;
        // same with cooperators
        let mut some_existing_cooperator = AccountId::default();
        for i in 0..max - cooperator_count {
            let controller = i as AccountId * 2 + cooperators_offset;
            let stash = i as AccountId * 2 + 1 + cooperators_offset;
            bond_cooperator(stash, controller, 1000, vec![(11, 100)]);
            some_existing_cooperator = controller;
        }

        // one more is too many
        bond(some_existing_cooperator + 2, some_existing_cooperator + 1, 1000);
        assert_noop!(
            PowerPlant::cooperate(
                RuntimeOrigin::signed(some_existing_cooperator + 1),
                vec![(11, 100)]
            ),
            Error::<Test>::TooManyCooperators
        );

        // Re-cooperate works fine
        assert_ok!(PowerPlant::cooperate(
            RuntimeOrigin::signed(some_existing_cooperator),
            vec![(11, 100)]
        ));
        // Re-validate works fine
        assert_ok!(PowerPlant::validate(
            RuntimeOrigin::signed(some_existing_validator),
            ValidatorPrefs::default()
        ));

        // No problem when we set to `None` again
        assert_ok!(PowerPlant::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Noop,
            ConfigOp::Noop,
        ));
        assert_ok!(PowerPlant::cooperate(
            RuntimeOrigin::signed(some_existing_cooperator),
            vec![(11, 100)]
        ));
        assert_ok!(PowerPlant::validate(
            RuntimeOrigin::signed(some_existing_validator),
            ValidatorPrefs::default()
        ));
    })
}

#[test]
fn min_commission_works() {
    ExtBuilder::default().build_and_execute(|| {
        // account 10 controls the stash from account 11
        assert_ok!(PowerPlant::validate(
            RuntimeOrigin::signed(10),
            ValidatorPrefs {
                commission: Perbill::from_percent(5),
                min_coop_reputation: 0.into(),
                collaborative: false
            }
        ));

        // event emitted should be correct
        assert_eq!(
            *staking_events().last().unwrap(),
            Event::ValidatorPrefsSet {
                stash: 11,
                prefs: ValidatorPrefs {
                    commission: Perbill::from_percent(5),
                    min_coop_reputation: 0.into(),
                    collaborative: false
                }
            }
        );

        assert_ok!(PowerPlant::set_staking_configs(
            RuntimeOrigin::root(),
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Remove,
            ConfigOp::Set(Perbill::from_percent(10)),
        ));

        // can't make it less than 10 now
        assert_noop!(
            PowerPlant::validate(
                RuntimeOrigin::signed(10),
                ValidatorPrefs {
                    commission: Perbill::from_percent(5),
                    min_coop_reputation: 0.into(),
                    collaborative: false
                }
            ),
            Error::<Test>::CommissionTooLow
        );

        // can only change to higher.
        assert_ok!(PowerPlant::validate(
            RuntimeOrigin::signed(10),
            ValidatorPrefs {
                commission: Perbill::from_percent(10),
                min_coop_reputation: 0.into(),
                collaborative: false
            }
        ));

        assert_ok!(PowerPlant::validate(
            RuntimeOrigin::signed(10),
            ValidatorPrefs {
                commission: Perbill::from_percent(15),
                min_coop_reputation: 0.into(),
                collaborative: false
            }
        ));
    })
}

#[test]
fn force_apply_min_commission_works() {
    let prefs = |c| ValidatorPrefs {
        commission: Perbill::from_percent(c),
        min_coop_reputation: 0.into(),
        collaborative: true,
    };
    let validators = || Validators::<Test>::iter().collect::<Vec<_>>();
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            31,
            CollaborativeValidatorReputationTier::get().into()
        ));
        assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(30), prefs(10)));
        assert_ok!(ReputationPallet::force_set_points(
            RuntimeOrigin::root(),
            21,
            CollaborativeValidatorReputationTier::get().into()
        ));
        assert_ok!(PowerPlant::validate(RuntimeOrigin::signed(20), prefs(5)));

        // Given
        assert_eq!(validators(), vec![(31, prefs(10)), (21, prefs(5)), (11, prefs(0))]);
        MinCommission::<Test>::set(Perbill::from_percent(5));

        // When applying to a commission greater than min
        assert_ok!(PowerPlant::force_apply_min_commission(RuntimeOrigin::signed(1), 31));
        // Then the commission is not changed
        assert_eq!(validators(), vec![(31, prefs(10)), (21, prefs(5)), (11, prefs(0))]);

        // When applying to a commission that is equal to min
        assert_ok!(PowerPlant::force_apply_min_commission(RuntimeOrigin::signed(1), 21));
        // Then the commission is not changed
        assert_eq!(validators(), vec![(31, prefs(10)), (21, prefs(5)), (11, prefs(0))]);

        // When applying to a commission that is less than the min
        assert_ok!(PowerPlant::force_apply_min_commission(RuntimeOrigin::signed(1), 11));
        // Then the commission is bumped to the min
        assert_eq!(validators(), vec![(31, prefs(10)), (21, prefs(5)), (11, prefs(5))]);

        // When applying commission to a validator that doesn't exist then storage is not altered
        assert_noop!(
            PowerPlant::force_apply_min_commission(RuntimeOrigin::signed(1), 420),
            Error::<Test>::NotStash
        );
    });
}

#[test]
fn proportional_slash_stop_slashing_if_remaining_zero() {
    let c = |era, value| UnlockChunk::<Balance> { era, value };
    // Given
    let mut ledger = StakingLedger::<Test> {
        stash: 123,
        total: 40,
        active: 20,
        // we have some chunks, but they are not affected.
        unlocking: bounded_vec![c(1, 10), c(2, 10)],
        claimed_rewards: bounded_vec![],
    };

    assert_eq!(BondingDuration::get(), 3);

    // should not slash more than the amount requested, by accidentally slashing the first chunk.
    assert_eq!(ledger.slash_stake(18, 1, 0), 18);
}

#[test]
fn proportional_ledger_slash_works() {
    let c = |era, value| UnlockChunk::<Balance> { era, value };
    // Given
    let mut ledger = StakingLedger::<Test> {
        stash: 123,
        total: 10,
        active: 10,
        unlocking: bounded_vec![],
        claimed_rewards: bounded_vec![],
    };
    assert_eq!(BondingDuration::get(), 3);

    // When we slash a ledger with no unlocking chunks
    assert_eq!(ledger.slash_stake(5, 1, 0), 5);
    // Then
    assert_eq!(ledger.total, 5);
    assert_eq!(ledger.active, 5);
    assert_eq!(LedgerSlashPerEra::get().0, 5);
    assert_eq!(LedgerSlashPerEra::get().1, Default::default());

    // When we slash a ledger with no unlocking chunks and the slash amount is greater then the
    // total
    assert_eq!(ledger.slash_stake(11, 1, 0), 5);
    // Then
    assert_eq!(ledger.total, 0);
    assert_eq!(ledger.active, 0);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(LedgerSlashPerEra::get().1, Default::default());

    // Given
    ledger.unlocking = bounded_vec![c(4, 10), c(5, 10)];
    ledger.total = 2 * 10;
    ledger.active = 0;
    // When all the chunks overlap with the slash eras
    assert_eq!(ledger.slash_stake(20, 0, 0), 20);
    // Then
    assert_eq!(ledger.unlocking, vec![]);
    assert_eq!(ledger.total, 0);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(LedgerSlashPerEra::get().1, BTreeMap::from([(4, 0), (5, 0)]));

    // Given
    ledger.unlocking = bounded_vec![c(4, 100), c(5, 100), c(6, 100), c(7, 100)];
    ledger.total = 4 * 100;
    ledger.active = 0;
    // When the first 2 chunks don't overlap with the affected range of unlock eras.
    assert_eq!(ledger.slash_stake(140, 0, 3), 140);
    // Then
    assert_eq!(ledger.unlocking, vec![c(4, 100), c(5, 100), c(6, 30), c(7, 30)]);
    assert_eq!(ledger.total, 4 * 100 - 140);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(LedgerSlashPerEra::get().1, BTreeMap::from([(6, 30), (7, 30)]));

    // Given
    ledger.unlocking = bounded_vec![c(4, 100), c(5, 100), c(6, 100), c(7, 100)];
    ledger.total = 4 * 100;
    ledger.active = 0;
    // When the first 2 chunks don't overlap with the affected range of unlock eras.
    assert_eq!(ledger.slash_stake(15, 0, 3), 15);
    // Then
    assert_eq!(ledger.unlocking, vec![c(4, 100), c(5, 100), c(6, 100 - 8), c(7, 100 - 7)]);
    assert_eq!(ledger.total, 4 * 100 - 15);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(LedgerSlashPerEra::get().1, BTreeMap::from([(6, 92), (7, 93)]));

    // Given
    ledger.unlocking = bounded_vec![c(4, 40), c(5, 100), c(6, 10), c(7, 250)];
    ledger.active = 500;
    // 900
    ledger.total = 40 + 10 + 100 + 250 + 500;
    // When we have a partial slash that touches all chunks
    assert_eq!(ledger.slash_stake(900 / 2, 0, 0), 450);
    // Then
    assert_eq!(ledger.active, 500 / 2);
    assert_eq!(ledger.unlocking, vec![c(4, 40 / 2), c(5, 100 / 2), c(6, 10 / 2), c(7, 250 / 2)]);
    assert_eq!(ledger.total, 900 / 2);
    assert_eq!(LedgerSlashPerEra::get().0, 500 / 2);
    assert_eq!(
        LedgerSlashPerEra::get().1,
        BTreeMap::from([(4, 40 / 2), (5, 100 / 2), (6, 10 / 2), (7, 250 / 2)])
    );

    // slash 1/4th with not chunk.
    ledger.unlocking = bounded_vec![];
    ledger.active = 500;
    ledger.total = 500;
    // When we have a partial slash that touches all chunks
    assert_eq!(ledger.slash_stake(500 / 4, 0, 0), 500 / 4);
    // Then
    assert_eq!(ledger.active, 3 * 500 / 4);
    assert_eq!(ledger.unlocking, vec![]);
    assert_eq!(ledger.total, ledger.active);
    assert_eq!(LedgerSlashPerEra::get().0, 3 * 500 / 4);
    assert_eq!(LedgerSlashPerEra::get().1, Default::default());

    // Given we have the same as above,
    ledger.unlocking = bounded_vec![c(4, 40), c(5, 100), c(6, 10), c(7, 250)];
    ledger.active = 500;
    ledger.total = 40 + 10 + 100 + 250 + 500; // 900
    assert_eq!(ledger.total, 900);
    // When we have a higher min balance
    assert_eq!(
        ledger.slash_stake(
            900 / 2,
            25, /* min balance - chunks with era 0 & 2 will be slashed to <=25, causing it to
                 * get swept */
            0
        ),
        450
    );
    assert_eq!(ledger.active, 500 / 2);
    // the last chunk was not slashed 50% like all the rest, because some other earlier chunks got
    // dusted.
    assert_eq!(ledger.unlocking, vec![c(5, 100 / 2), c(7, 150)]);
    assert_eq!(ledger.total, 900 / 2);
    assert_eq!(LedgerSlashPerEra::get().0, 500 / 2);
    assert_eq!(
        LedgerSlashPerEra::get().1,
        BTreeMap::from([(4, 0), (5, 100 / 2), (6, 0), (7, 150)])
    );

    // Given
    // slash order --------------------NA--------2----------0----------1----
    ledger.unlocking = bounded_vec![c(4, 40), c(5, 100), c(6, 10), c(7, 250)];
    ledger.active = 500;
    ledger.total = 40 + 10 + 100 + 250 + 500; // 900
    assert_eq!(
        ledger.slash_stake(
            500 + 10 + 250 + 100 / 2, // active + era 6 + era 7 + era 5 / 2
            0,
            3 /* slash era 6 first, so the affected parts are era 6, era 7 and
               * ledge.active. This will cause the affected to go to zero, and then we will
               * start slashing older chunks */
        ),
        500 + 250 + 10 + 100 / 2
    );
    // Then
    assert_eq!(ledger.active, 0);
    assert_eq!(ledger.unlocking, vec![c(4, 40), c(5, 100 / 2)]);
    assert_eq!(ledger.total, 90);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(LedgerSlashPerEra::get().1, BTreeMap::from([(5, 100 / 2), (6, 0), (7, 0)]));

    // Given
    // iteration order------------------NA---------2----------0----------1----
    ledger.unlocking = bounded_vec![c(4, 100), c(5, 100), c(6, 100), c(7, 100)];
    ledger.active = 100;
    ledger.total = 5 * 100;
    // When
    assert_eq!(
        ledger.slash_stake(
            351, // active + era 6 + era 7 + era 5 / 2 + 1
            50,  // min balance - everything slashed below 50 will get dusted
            3    /* slash era 3+3 first, so the affected parts are era 6, era 7 and
                  * ledge.active. This will cause the affected to go to zero, and then we will
                  * start slashing older chunks */
        ),
        400
    );
    // Then
    assert_eq!(ledger.active, 0);
    assert_eq!(ledger.unlocking, vec![c(4, 100)]);
    assert_eq!(ledger.total, 100);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(LedgerSlashPerEra::get().1, BTreeMap::from([(5, 0), (6, 0), (7, 0)]));

    // Tests for saturating arithmetic

    // Given
    let slash = u64::MAX as Balance * 2;
    // The value of the other parts of ledger that will get slashed
    let value = slash - (10 * 4);

    ledger.active = 10;
    ledger.unlocking = bounded_vec![c(4, 10), c(5, 10), c(6, 10), c(7, value)];
    ledger.total = value + 40;
    // When
    let slash_amount = ledger.slash_stake(slash, 0, 0);
    assert_eq_error_rate!(slash_amount, slash, 5);
    // Then
    assert_eq!(ledger.active, 0); // slash of 9
    assert_eq!(ledger.unlocking, vec![]);
    assert_eq!(ledger.total, 0);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(LedgerSlashPerEra::get().1, BTreeMap::from([(4, 0), (5, 0), (6, 0), (7, 0)]));

    // Given
    use sp_runtime::PerThing as _;
    let slash = u64::MAX as Balance * 2;
    let value = u64::MAX as Balance * 2;
    let unit = 100;
    // slash * value that will saturate
    assert!(slash.checked_mul(value).is_none());
    // but slash * unit won't.
    assert!(slash.checked_mul(unit).is_some());
    ledger.unlocking = bounded_vec![c(4, unit), c(5, value), c(6, unit), c(7, unit)];
    //--------------------------------------note value^^^
    ledger.active = unit;
    ledger.total = unit * 4 + value;
    // When
    assert_eq!(ledger.slash_stake(slash, 0, 0), slash);
    // Then
    // The amount slashed out of `unit`
    let affected_balance = value + unit * 4;
    let ratio =
        Perquintill::from_rational_with_rounding(slash, affected_balance, Rounding::Up).unwrap();
    // `unit` after the slash is applied
    let unit_slashed = {
        let unit_slash = ratio.mul_ceil(unit);
        unit - unit_slash
    };
    let value_slashed = {
        let value_slash = ratio.mul_ceil(value);
        value - value_slash
    };
    assert_eq!(ledger.active, unit_slashed);
    assert_eq!(ledger.unlocking, vec![c(5, value_slashed), c(7, 32)]);
    assert_eq!(ledger.total, value_slashed + 32);
    assert_eq!(LedgerSlashPerEra::get().0, 0);
    assert_eq!(
        LedgerSlashPerEra::get().1,
        BTreeMap::from([(4, 0), (5, value_slashed), (6, 0), (7, 32)])
    );
}

#[test]
fn pre_bonding_era_cannot_be_claimed() {
    // Verifies initial conditions of mock
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        let history_depth = HistoryDepth::get();
        // jump to some era above history_depth
        let mut current_era = history_depth + 10;
        let last_reward_era = current_era - 1;
        let start_reward_era = current_era - history_depth;

        // put some money in stash=3 and controller=4.
        for i in 3..5 {
            let _ = Balances::make_free_balance_be(&i, 2000);
        }

        mock::start_active_era(current_era);

        // add a new candidate for being a validator. account 3 controlled by 4.
        assert_ok!(PowerPlant::bond(
            RuntimeOrigin::signed(3),
            4,
            1500,
            RewardDestination::Controller
        ));

        let claimed_rewards: BoundedVec<_, _> =
            (start_reward_era..=last_reward_era).collect::<Vec<_>>().try_into().unwrap();
        assert_eq!(
            PowerPlant::ledger(4).unwrap(),
            StakingLedger {
                stash: 3,
                total: 1500,
                active: 1500,
                unlocking: Default::default(),
                claimed_rewards,
            }
        );

        // start next era
        current_era += 1;
        mock::start_active_era(current_era);

        // claiming reward for last era in which validator was active works
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(4), 3, current_era - 1));

        // consumed weight for all payout_stakers dispatches that fail
        let err_weight = <Test as Config>::ThisWeightInfo::payout_stakers_alive_staked(0);
        // cannot claim rewards for an era before bonding occured as it is
        // already marked as claimed.
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(4), 3, current_era - 2),
            Error::<Test>::AlreadyClaimed.with_weight(err_weight)
        );

        // decoding will fail now since PowerPlant Ledger is in corrupt state
        HistoryDepth::set(history_depth - 1);
        assert_eq!(PowerPlant::ledger(4), None);

        // make sure stakers still cannot claim rewards that they are not meant to
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(4), 3, current_era - 2),
            Error::<Test>::NotController
        );

        // fix the corrupted state for post conditions check
        HistoryDepth::set(history_depth);
    });
}

#[test]
fn reducing_history_depth_abrupt() {
    // Verifies initial conditions of mock
    ExtBuilder::default().cooperate(false).build_and_execute(|| {
        let original_history_depth = HistoryDepth::get();
        let mut current_era = original_history_depth + 10;
        let last_reward_era = current_era - 1;
        let start_reward_era = current_era - original_history_depth;

        // put some money in (stash, controller)=(3,4),(5,6).
        for i in 3..7 {
            let _ = Balances::make_free_balance_be(&i, 2000);
        }

        // start current era
        mock::start_active_era(current_era);

        // add a new candidate for being a staker. account 3 controlled by 4.
        assert_ok!(PowerPlant::bond(
            RuntimeOrigin::signed(3),
            4,
            1500,
            RewardDestination::Controller
        ));

        // all previous era before the bonding action should be marked as
        // claimed.
        let claimed_rewards: BoundedVec<_, _> =
            (start_reward_era..=last_reward_era).collect::<Vec<_>>().try_into().unwrap();
        assert_eq!(
            PowerPlant::ledger(4).unwrap(),
            StakingLedger {
                stash: 3,
                total: 1500,
                active: 1500,
                unlocking: Default::default(),
                claimed_rewards,
            }
        );

        // next era
        current_era += 1;
        mock::start_active_era(current_era);

        // claiming reward for last era in which validator was active works
        assert_ok!(PowerPlant::payout_stakers(RuntimeOrigin::signed(4), 3, current_era - 1));

        // next era
        current_era += 1;
        mock::start_active_era(current_era);

        // history_depth reduced without migration
        let history_depth = original_history_depth - 1;
        HistoryDepth::set(history_depth);
        // claiming reward does not work anymore
        assert_noop!(
            PowerPlant::payout_stakers(RuntimeOrigin::signed(4), 3, current_era - 1),
            Error::<Test>::NotController
        );

        // new stakers can still bond
        assert_ok!(PowerPlant::bond(
            RuntimeOrigin::signed(5),
            6,
            1200,
            RewardDestination::Controller
        ));

        // new staking ledgers created will be bounded by the current history depth
        let last_reward_era = current_era - 1;
        let start_reward_era = current_era - history_depth;
        let claimed_rewards: BoundedVec<_, _> =
            (start_reward_era..=last_reward_era).collect::<Vec<_>>().try_into().unwrap();
        assert_eq!(
            PowerPlant::ledger(6).unwrap(),
            StakingLedger {
                stash: 5,
                total: 1200,
                active: 1200,
                unlocking: Default::default(),
                claimed_rewards,
            }
        );

        // fix the corrupted state for post conditions check
        HistoryDepth::set(original_history_depth);
    });
}

#[test]
fn reducing_max_unlocking_chunks_abrupt() {
    // Concern is on validators only
    // By Default 11, 10 are stash and ctrl and 21,20
    ExtBuilder::default().build_and_execute(|| {
        // given a staker at era=10 and MaxUnlockChunks set to 2
        MaxUnlockingChunks::set(2);
        start_active_era(10);
        assert_ok!(PowerPlant::bond(RuntimeOrigin::signed(3), 4, 300, RewardDestination::Stash));
        assert!(matches!(PowerPlant::ledger(4), Some(_)));

        // when staker unbonds
        assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(4), 20));

        // then an unlocking chunk is added at `current_era + bonding_duration`
        // => 10 + 3 = 13
        let expected_unlocking: BoundedVec<UnlockChunk<Balance>, MaxUnlockingChunks> =
            bounded_vec![UnlockChunk { value: 20 as Balance, era: 13 as EraIndex }];
        assert!(matches!(PowerPlant::ledger(4),
			Some(StakingLedger {
				unlocking,
				..
			}) if unlocking==expected_unlocking));

        // when staker unbonds at next era
        start_active_era(11);
        assert_ok!(PowerPlant::unbond(RuntimeOrigin::signed(4), 50));
        // then another unlock chunk is added
        let expected_unlocking: BoundedVec<UnlockChunk<Balance>, MaxUnlockingChunks> =
            bounded_vec![UnlockChunk { value: 20, era: 13 }, UnlockChunk { value: 50, era: 14 }];
        assert!(matches!(PowerPlant::ledger(4),
			Some(StakingLedger {
				unlocking,
				..
			}) if unlocking==expected_unlocking));

        // when staker unbonds further
        start_active_era(12);
        // then further unbonding not possible
        assert_noop!(PowerPlant::unbond(RuntimeOrigin::signed(4), 20), Error::<Test>::NoMoreChunks);

        // when max unlocking chunks is reduced abruptly to a low value
        MaxUnlockingChunks::set(1);
        // then unbond, rebond ops are blocked with ledger in corrupt state
        assert_noop!(
            PowerPlant::unbond(RuntimeOrigin::signed(4), 20),
            Error::<Test>::NotController
        );
        assert_noop!(
            PowerPlant::rebond(RuntimeOrigin::signed(4), 100),
            Error::<Test>::NotController
        );

        // reset the ledger corruption
        MaxUnlockingChunks::set(2);
    })
}

#[test]
fn set_min_commission_works_with_admin_origin() {
    ExtBuilder::default().build_and_execute(|| {
        // no minimum commission set initially
        assert_eq!(MinCommission::<Test>::get(), Zero::zero());

        // root can set min commission
        assert_ok!(PowerPlant::set_min_commission(
            RuntimeOrigin::root(),
            Perbill::from_percent(10)
        ));

        assert_eq!(MinCommission::<Test>::get(), Perbill::from_percent(10));

        // Non privileged origin can not set min_commission
        assert_noop!(
            PowerPlant::set_min_commission(RuntimeOrigin::signed(2), Perbill::from_percent(15)),
            BadOrigin
        );

        // Admin Origin can set min commission
        assert_ok!(PowerPlant::set_min_commission(
            RuntimeOrigin::signed(1),
            Perbill::from_percent(15),
        ));

        // setting commission below min_commission fails
        assert_noop!(
            PowerPlant::validate(
                RuntimeOrigin::signed(10),
                ValidatorPrefs {
                    commission: Perbill::from_percent(14),
                    min_coop_reputation: 0.into(),
                    collaborative: false
                }
            ),
            Error::<Test>::CommissionTooLow
        );

        // setting commission >= min_commission works
        assert_ok!(PowerPlant::validate(
            RuntimeOrigin::signed(10),
            ValidatorPrefs {
                commission: Perbill::from_percent(15),
                min_coop_reputation: 0.into(),
                collaborative: false
            }
        ));
    })
}
