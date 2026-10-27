use super::*;
use crate::mock::*;
use crate::{Error, PenaltyType};
use frame_support::traits::fungible::Inspect;
use frame_support::{assert_err, assert_noop, assert_ok};
use substrate_test_utils::assert_eq_uvec;

#[test]
fn test_data_building() {
    // Valid date creation.
    assert_ok!(CurrentDateInfo::new::<Test>(2020, 1, 1));
    assert_ok!(CurrentDateInfo::new::<Test>(2000, 10, 12));
    assert_ok!(CurrentDateInfo::new::<Test>(2009, 1, 31));

    // Invalid date creation.
    assert_err!(CurrentDateInfo::new::<Test>(2009, 14, 31), Error::<Test>::NotCorrectDate);
}

#[test]
fn test_quarter_calculation() {
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 1, 1).unwrap().current_quarter, 1);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 2, 15).unwrap().current_quarter, 1);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 3, 31).unwrap().current_quarter, 1);

    assert_eq!(CurrentDateInfo::new::<Test>(2020, 4, 1).unwrap().current_quarter, 2);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 5, 15).unwrap().current_quarter, 2);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 6, 30).unwrap().current_quarter, 2);

    assert_eq!(CurrentDateInfo::new::<Test>(2020, 7, 1).unwrap().current_quarter, 3);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 8, 15).unwrap().current_quarter, 3);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 9, 30).unwrap().current_quarter, 3);

    assert_eq!(CurrentDateInfo::new::<Test>(2020, 10, 1).unwrap().current_quarter, 4);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 11, 15).unwrap().current_quarter, 4);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 12, 31).unwrap().current_quarter, 4);
}

#[test]
fn test_days_since_new_year_calculation() {
    // Non-leap year.
    assert_eq!(CurrentDateInfo::new::<Test>(2021, 1, 1).unwrap().days_since_new_year, 0);
    assert_eq!(CurrentDateInfo::new::<Test>(2021, 1, 31).unwrap().days_since_new_year, 30);
    assert_eq!(CurrentDateInfo::new::<Test>(2021, 2, 1).unwrap().days_since_new_year, 31);
    assert_eq!(CurrentDateInfo::new::<Test>(2021, 2, 28).unwrap().days_since_new_year, 58);
    assert_eq!(CurrentDateInfo::new::<Test>(2021, 3, 1).unwrap().days_since_new_year, 59);
    assert_eq!(CurrentDateInfo::new::<Test>(2021, 12, 31).unwrap().days_since_new_year, 364);

    // Leap year.
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 1, 1).unwrap().days_since_new_year, 0);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 1, 31).unwrap().days_since_new_year, 30);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 2, 1).unwrap().days_since_new_year, 31);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 2, 29).unwrap().days_since_new_year, 59);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 3, 1).unwrap().days_since_new_year, 60);
    assert_eq!(CurrentDateInfo::new::<Test>(2020, 12, 31).unwrap().days_since_new_year, 365);
}

#[test]
fn test_adding_days_to_date() {
    // Add days to current date.
    let mut current_date = CurrentDateInfo::new::<Test>(2019, 1, 1).unwrap();
    assert_ok!(current_date.add_days::<Test>(65));

    let eq_data = CurrentDateInfo::new::<Test>(2019, 3, 7).unwrap();
    assert_eq!(current_date, eq_data);

    assert_ok!(current_date.add_days::<Test>(300));
    let eq_data = CurrentDateInfo::new::<Test>(2020, 1, 1).unwrap();
    assert_eq!(current_date, eq_data);

    // Check leap year.
    assert_ok!(current_date.add_days::<Test>(366));
    let eq_data = CurrentDateInfo::new::<Test>(2021, 1, 1).unwrap();
    assert_eq!(current_date, eq_data);
}

#[test]
fn test_update_time() {
    ExtBuilder::default().build_and_execute(|| {
        // Set valid time.
        let initial_data = CurrentDateInfo::new::<Test>(2020, 1, 1).unwrap();
        assert_eq!(Privileges::current_date(), initial_data);

        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 5, 10));

        let first_test_data = CurrentDateInfo::new::<Test>(2020, 5, 10).unwrap();
        assert_eq!(Privileges::current_date(), first_test_data);

        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2022, 1, 30));

        let second_test_data = CurrentDateInfo::new::<Test>(2022, 1, 30).unwrap();
        assert_eq!(Privileges::current_date(), second_test_data);
    })
}

#[test]
fn become_a_vip_as_validator() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(
            Privileges::vip_members(10).unwrap().active_stake,
            EnergyGeneration::ledger(10).unwrap().active
        );
        let current_date = Privileges::current_date();

        // Set next day.
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 1
        ));

        let vip_points = EnergyGeneration::ledger(10).unwrap().active / 50;
        assert_eq!(Privileges::vip_members(10).unwrap().points, vip_points);
    })
}

#[test]
fn become_a_vip_as_cooperator() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(
            RuntimeOrigin::signed(100),
            PenaltyType::Declining,
        ));
        assert_eq!(Privileges::vip_members(100).unwrap().points, 0);
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 200 + 300);
        let current_date = Privileges::current_date();

        // Set next day.
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 1
        ));

        let vip_points = (200 + 300) / 50;
        assert_eq!(Privileges::vip_members(100).unwrap().points, vip_points);
    })
}

#[test]
fn exit_a_vip_as_validator_free_tax_period() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(
            RuntimeOrigin::signed(10),
            PenaltyType::Declining,
        ));

        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 2));

        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(EnergyGeneration::ledger(10).unwrap().active, 1000);
        assert_eq!(System::account(10).data.frozen, 1000);
        assert_eq!(System::account(10).data.free, 1000);
    })
}

#[test]
fn exit_a_vip_as_cooperator_free_tax_period() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(
            RuntimeOrigin::signed(100),
            PenaltyType::Declining,
        ));

        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 1, 2));

        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(100)));
        assert_eq!(Privileges::vip_members(100), None);
        assert_eq!(EnergyGeneration::ledger(100).unwrap().active, 500);
        assert_eq!(System::account(100).data.frozen, 500);
        assert_eq!(System::account(100).data.free, 2000);
    })
}

#[test]
fn exit_a_vip_as_validator_non_free_tax_period() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(
            RuntimeOrigin::signed(10),
            PenaltyType::Declining,
        ));

        // Set free tax period.
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 2, 2));

        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(EnergyGeneration::ledger(10).unwrap().active, 1000 - 1000 / 4);
        assert_eq!(System::account(10).data.frozen, 1000 - 1000 / 4);
        assert_eq!(System::account(10).data.free, 1000 - 1000 / 4);
        assert!(EnergyGeneration::is_user_validator(&10));
    })
}

#[test]
fn exit_a_vip_as_validator_non_free_tax_period_and_kick() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(
            RuntimeOrigin::signed(20),
            PenaltyType::Declining,
        ));

        // Set free tax period.
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 2, 2));

        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(20)));
        assert_eq!(Privileges::vip_members(20), None);
        assert_eq!(EnergyGeneration::ledger(20).unwrap().active, 500 - 500 / 4);
        assert_eq!(System::account(20).data.frozen, 500 - 500 / 4);
        assert_eq!(System::account(20).data.free, 2000 - 500 / 4);
        assert!(EnergyGeneration::is_user_validator(&20));
    })
}

#[test]
fn exit_a_vip_as_cooperator_non_free_tax_period() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(
            RuntimeOrigin::signed(100),
            PenaltyType::Declining,
        ));

        // Set free tax period.
        assert_ok!(Privileges::update_time(RuntimeOrigin::root(), 2020, 2, 2));

        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(100)));
        assert_eq!(Privileges::vip_members(100), None);
        assert_eq!(EnergyGeneration::ledger(100).unwrap().active, 375);
        assert_eq!(System::account(100).data.frozen, 375);
        assert_eq!(System::account(100).data.free, 1875);
    })
}

#[test]
fn test_change_penalty_type() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vip_members(10).unwrap().tax_type, PenaltyType::Flat);
        let current_date = Privileges::current_date();

        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 1
        ));
        assert_ok!(Privileges::change_penalty_type(
            RuntimeOrigin::signed(10),
            PenaltyType::Declining
        ));

        assert_eq!(Privileges::vip_members(10).unwrap().tax_type, PenaltyType::Declining);
    })
}

#[test]
fn test_calculation_vip_points() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(
            Privileges::vip_members(10).unwrap().active_stake,
            EnergyGeneration::ledger(10).unwrap().active
        );
        let current_date = Privileges::current_date();

        // Set next day.
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month + 5,
            current_date.current_day + 10
        ));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 1437);
    })
}

#[test]
fn test_year_end_data_saving() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(
            Privileges::vip_members(10).unwrap().active_stake,
            EnergyGeneration::ledger(10).unwrap().active
        );

        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(20), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(20).unwrap().points, 0);
        assert_eq!(
            Privileges::vip_members(20).unwrap().active_stake,
            EnergyGeneration::ledger(20).unwrap().active
        );

        let current_date = Privileges::current_date();

        let expected_points = Vec::from([(10, 2113), (20, 1042)]);
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year + 1,
            current_date.current_month,
            current_date.current_day
        ));

        assert_eq_uvec!(VipPoints::<Test>::iter_prefix(2020).collect::<Vec<_>>(), expected_points);
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vip_members(20).unwrap().points, 0);
    })
}

#[test]
fn test_year_end_data_saving_vipp_results() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 1000));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200, None, None));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            10,
            sig::<Test>(&bob(), &10u64.encode(), &[][..])
        ));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(Privileges::vipp_members(10), None);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().active_vipp_threshold[0].1, 190);
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 1000);

        let current_date = Privileges::current_date();

        let expected_points = Vec::from([(10, 69350)]);
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year + 1,
            current_date.current_month,
            current_date.current_day
        ));

        assert_eq_uvec!(VippPoints::<Test>::iter_prefix(2020).collect::<Vec<_>>(), expected_points);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
    })
}

#[test]
fn test_upgrade_active_stake_throw_bond_extra() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(30), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(30).unwrap().points, 0);
        assert_eq!(
            Privileges::vip_members(30).unwrap().active_stake,
            EnergyGeneration::ledger(30).unwrap().active
        );
        let current_date = Privileges::current_date();

        // Set next day.
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 1
        ));

        let mut vip_points = EnergyGeneration::ledger(30).unwrap().active / 50;
        assert_eq!(Privileges::vip_members(30).unwrap().points, vip_points);
        assert_eq!(EnergyGeneration::ledger(30).unwrap().active, 500);
        assert_eq!(System::account(30).data.frozen, 500);
        assert_eq!(System::account(30).data.free, 2000);

        assert_ok!(EnergyGeneration::bond_extra(RuntimeOrigin::signed(30), 1000));

        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 2
        ));
        vip_points += EnergyGeneration::ledger(30).unwrap().active / (1 + 50);
        assert_eq!(Privileges::vip_members(30).unwrap().points, vip_points);
        assert_eq!(Privileges::vipp_members(30), None);

        assert_eq!(EnergyGeneration::ledger(30).unwrap().active, 1500);
        assert_eq!(System::account(30).data.frozen, 1500);
        assert_eq!(System::account(30).data.free, 2000);

        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 3
        ));
        vip_points += EnergyGeneration::ledger(30).unwrap().active / (2 + 50);
        assert_eq!(Privileges::vip_members(30).unwrap().points, vip_points);
        assert_eq!(Privileges::vipp_members(30), None);

        assert_eq!(EnergyGeneration::ledger(30).unwrap().active, 1500);
        assert_eq!(System::account(30).data.frozen, 1500);
        assert_eq!(System::account(30).data.free, 2000);
    })
}

#[test]
fn test_upgrade_active_stake_throw_unbond() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(
            Privileges::vip_members(10).unwrap().active_stake,
            EnergyGeneration::ledger(10).unwrap().active
        );
        let current_date = Privileges::current_date();

        // Set next day.
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 1
        ));

        let mut vip_points = EnergyGeneration::ledger(10).unwrap().active / 50;
        assert_eq!(Privileges::vip_members(10).unwrap().points, vip_points);
        assert_eq!(EnergyGeneration::ledger(10).unwrap().active, 1000);
        assert_eq!(System::account(10).data.frozen, 1000);
        assert_eq!(System::account(10).data.free, 1000);

        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 100));
        assert_eq!(
            Privileges::vip_members(10).unwrap().active_stake,
            EnergyGeneration::ledger(10).unwrap().active
        );

        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 2
        ));
        vip_points += EnergyGeneration::ledger(10).unwrap().active / (1 + 50);
        assert_eq!(Privileges::vip_members(10).unwrap().points, vip_points);
        assert_eq!(Privileges::vipp_members(10), None);

        assert_eq!(EnergyGeneration::ledger(10).unwrap().active, 900);
        assert_eq!(System::account(10).data.frozen, 1000);
        assert_eq!(System::account(10).data.free, 1000);

        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 3
        ));
        vip_points += EnergyGeneration::ledger(10).unwrap().active / (2 + 50);
        assert_eq!(Privileges::vip_members(10).unwrap().points, vip_points);
        assert_eq!(Privileges::vipp_members(10), None);

        assert_eq!(EnergyGeneration::ledger(10).unwrap().active, 900);
        assert_eq!(System::account(10).data.frozen, 1000);
        assert_eq!(System::account(10).data.free, 1000);
    })
}

#[test]
fn test_minting_vipp_nft() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 1000));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200, None, None));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            10,
            sig::<Test>(&bob(), &10u64.encode(), &[][..])
        ));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(Privileges::vipp_members(10), None);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().active_vipp_threshold[0].1, 190);
    })
}

#[test]
fn test_calculating_validator_vipp_points() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 1000));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200, None, None));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            10,
            sig::<Test>(&bob(), &10u64.encode(), &[][..])
        ));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(Privileges::vipp_members(10), None);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().active_vipp_threshold[0].1, 190);
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 1000);

        let current_date = Privileges::current_date();
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 1
        ));

        assert_eq!(Privileges::vipp_members(10).unwrap().points, 190);
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(10), 900));
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 2
        ));
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 290);
    })
}

#[test]
fn test_calculating_cooperator_vipp_points() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 1000));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 100, None, None));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            100,
            sig::<Test>(&bob(), &100u64.encode(), &[][..])
        ));
        assert_eq!(Privileges::vip_members(100), None);
        assert_eq!(Privileges::vipp_members(100), None);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(100), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(100).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(100).unwrap().active_vipp_threshold[0].1, 95);
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 500);

        let current_date = Privileges::current_date();
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 1
        ));

        assert_eq!(Privileges::vipp_members(100).unwrap().points, 95);
        assert_ok!(EnergyGeneration::cooperate(RuntimeOrigin::signed(100), vec![(10, 50)]));
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 2
        ));
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 145);

        assert_ok!(EnergyGeneration::chill(RuntimeOrigin::signed(100)));

        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year,
            current_date.current_month,
            current_date.current_day + 3
        ));
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 145);
    })
}

#[test]
fn test_burning_vipp_nft() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 1000));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200, None, None));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            10,
            sig::<Test>(&bob(), &10u64.encode(), &[][..])
        ));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(Privileges::vipp_members(10), None);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().active_vipp_threshold[0].1, 190);

        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(Privileges::vipp_members(10), None);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10), None);
    })
}

#[test]
fn test_from_validator_to_cooperator() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 1000));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200, None, None));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            10,
            sig::<Test>(&bob(), &10u64.encode(), &[][..])
        ));
        assert_eq!(Privileges::vip_members(10), None);
        assert_eq!(Privileges::vipp_members(10), None);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat,));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vipp_members(10).unwrap().active_vipp_threshold[0].1, 190);

        assert_ok!(EnergyGeneration::cooperate(RuntimeOrigin::signed(10), vec![(20, 100)]));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 100);
    })
}

#[test]
fn cannot_set_rewards_before_past_year() {
    ExtBuilder::default().build_and_execute(|| {
        let current_year = Privileges::current_date().current_year as u32;

        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), current_year - 1, 100, 100));
        assert_noop!(
            Privileges::set_rewards(RuntimeOrigin::root(), current_year, 100, 100),
            Error::<Test>::NotCorrectDate
        );
        assert_noop!(
            Privileges::set_rewards(RuntimeOrigin::root(), current_year + 1, 100, 100),
            Error::<Test>::NotCorrectDate
        );
    })
}

#[test]
fn cannot_set_rewards_twice() {
    ExtBuilder::default().build_and_execute(|| {
        let year = Privileges::current_date().current_year as u32 - 1;

        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), year, 100, 100));
        assert_noop!(
            Privileges::set_rewards(RuntimeOrigin::root(), year, 100, 100),
            Error::<Test>::YearRewardsAlreadySet
        );
    })
}

#[test]
fn claim_rewards_works() {
    ExtBuilder::default().build_and_execute(|| {
        let year = Privileges::current_date().current_year as u32 - 1;

        assert_ok!(Privileges::force_set_vip_points(RuntimeOrigin::root(), year, 1, 100));
        assert_ok!(Privileges::force_set_vipp_points(RuntimeOrigin::root(), year, 1, 200));

        assert_ok!(Privileges::force_set_vip_points(RuntimeOrigin::root(), year, 2, 300));

        assert_ok!(Privileges::set_rewards(RuntimeOrigin::root(), year, 1000, 1000));

        // Account 1 has VIP and VIPP rewards
        assert_eq!(Privileges::vip_points(year, 1), Some(100));
        assert_eq!(Privileges::vipp_points(year, 1), Some(200));

        let initial_balance = Balances::balance(&1);
        let expected_vip_reward = 250; // 100/400 * 1000
        let expected_vipp_reward = 1000;

        assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(1), year));
        assert_eq!(
            Balances::balance(&1),
            initial_balance + expected_vip_reward + expected_vipp_reward
        );
        assert_eq!(Privileges::vip_points(year, 1), None);
        assert_eq!(Privileges::vipp_points(year, 1), None);

        // Account 2 has only VIP rewards
        assert_eq!(Privileges::vip_points(year, 2), Some(300));
        assert_eq!(Privileges::vipp_points(year, 2), None);

        let initial_balance = Balances::balance(&2);
        let expected_vip_reward = 750; // 300/400 * 1000
        let expected_vipp_reward = 0;

        assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(2), year));
        assert_eq!(
            Balances::balance(&2),
            initial_balance + expected_vip_reward + expected_vipp_reward
        );
        assert_eq!(Privileges::vip_points(year, 2), None);
        assert_eq!(Privileges::vipp_points(year, 2), None);

        // No double claiming
        assert_noop!(
            Privileges::claim_rewards(RuntimeOrigin::signed(1), year),
            Error::<Test>::NoRewardsForAccount
        );
        assert_noop!(
            Privileges::claim_rewards(RuntimeOrigin::signed(2), year),
            Error::<Test>::NoRewardsForAccount
        );
    })
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

fn claim_for_validator(amount: Balance) {
    claim_for_account(10, amount);
}

fn claim_for_account(account: AccountId, amount: Balance) {
    assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), amount));
    assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), amount, None, None));
    assert_ok!(Claiming::claim(
        RuntimeOrigin::none(),
        account,
        sig::<Test>(&bob(), &account.encode(), &[][..])
    ));
}

fn owned_vipp_ids(account: AccountId) -> Vec<u32> {
    <Nfts as frame_support::traits::tokens::nonfungibles_v2::InspectEnumerable<AccountId>>::owned_in_collection(&1, &account).collect()
}

fn join_vip_and_vipp(account: AccountId) {
    claim_for_account(account, 1000);
    assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(account), PenaltyType::Flat));
    assert!(Privileges::vipp_members(account).is_some());
}

#[test]
fn test_claims_before_becoming_vip() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(200);
        System::set_block_number(2);
        claim_for_validator(100);
        assert!(owned_vipp_ids(10).is_empty());
        assert!(Privileges::vipp_members(10).is_none());

        System::set_block_number(3);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        let ids = owned_vipp_ids(10);
        assert_eq!(ids.len(), 1);
        assert_eq!(
            Privileges::vipp_members(10).unwrap().active_vipp_threshold,
            vec![(ids[0], 285)]
        );
    });
}

#[test]
fn test_existing_vipp_nft_on_vip_entry() {
    use frame_support::traits::tokens::nonfungibles_v2::Mutate;
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(200);
        // Reproduce the legacy state: a real NFT minted before VIP membership.
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::mint_into(
            &1,
            &777,
            &10,
            &pallet_nfts::ItemConfig { settings: pallet_nfts::ItemSettings::all_enabled() },
            true,
        ));
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &1,
            &777,
            &[0, 0, 2],
            &200u64.encode(),
        ));
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        assert_eq!(owned_vipp_ids(10), vec![777]);
        assert!(Privileges::vipp_members(10).is_none());
    });
}

#[test]
fn test_additional_claim_failure() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(200);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 100));
        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 100, None, None));
        // Minting twice in the same block collides with the existing item ID.
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::none(),
                10,
                sig::<Test>(&bob(), &10u64.encode(), &[][..])
            ),
            pallet_nfts::Error::<Test>::AlreadyExists
        );
        assert_eq!(Claiming::claims(eth(&bob())), Some(100));
        System::set_block_number(2);
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            10,
            sig::<Test>(&bob(), &10u64.encode(), &[][..])
        ));
        assert_eq!(
            Privileges::vipp_members(10)
                .unwrap()
                .active_vipp_threshold
                .iter()
                .map(|(_, cap)| cap)
                .sum::<u64>(),
            285
        );
    });
}

#[test]
fn test_vip_entry_failure() {
    use frame_support::traits::tokens::nonfungibles_v2::Mutate;
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(200);
        let id = NacManaging::create_unique_item_id(&10);
        // An ID collision with an item owned by another account must not affect it.
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::mint_into(
            &1,
            &id,
            &20,
            &pallet_nfts::ItemConfig { settings: pallet_nfts::ItemSettings::all_enabled() },
            true,
        ));
        assert_noop!(
            Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat),
            pallet_nfts::Error::<Test>::AlreadyExists
        );
        assert!(Privileges::vip_members(10).is_none());
        assert!(Privileges::vipp_members(10).is_none());
        assert_eq!(Nfts::owner(1, id), Some(20));
    });
}

#[test]
fn test_vip_entry_with_inactive_nac() {
    use frame_support::traits::tokens::nonfungibles_v2::Mutate;
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(200);
        NacManaging::mint_vipp_nft(&10).unwrap().unwrap();
        let (_, nac_id) = NacManaging::get_nac_level(&10).unwrap();
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &0,
            &nac_id,
            &[0, 0, 3],
            &[1]
        ));
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        assert!(Privileges::vipp_members(10).is_none());
        assert_eq!(owned_vipp_ids(10).len(), 1);
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
fn test_unaccounted_vipp_transfer_does_not_activate_recipients() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        assert_ok!(NacManaging::mint(RuntimeOrigin::root(), 2, 20));
        let item = owned_vipp_ids(10)[0];

        for (sender, recipient) in [(10, 20), (20, 100)] {
            assert_ok!(Nfts::transfer(RuntimeOrigin::signed(sender), 1, item, recipient));
            assert_ok!(Privileges::become_vip_status(
                RuntimeOrigin::signed(recipient),
                PenaltyType::Flat
            ));
            assert!(NacManaging::get_claim_balance(&recipient).is_none());
            assert!(Privileges::vipp_members(recipient).is_none());
        }

        // Moving an NFT without updating its backing must not create additional
        // recipients of points. Ownership repair requires separate reconciliation.
        Privileges::update_vipp_points_for_time(1);
        assert_eq!(VippMembers::<Test>::iter_values().map(|m| m.points).sum::<u64>(), 950);
    });
}

fn advance_test_date(origin: RuntimeOrigin, year: i32, month: u32, day: u32) -> DispatchResult {
    Privileges::update_time(origin, year, month, day)
}

#[test]
fn vipp_failed_exit_rolls_back_penalty_membership_and_points() {
    use frame_support::traits::tokens::nonfungibles_v2::Mutate;
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 2, 1));
        let id = owned_vipp_ids(10)[0];
        // A malformed legacy attribute must fail the exit after the penalty path,
        // with the transaction layer restoring balances, locks and memberships.
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &1,
            &id,
            &[0, 0, 2],
            &[1],
        ));
        assert_noop!(
            Privileges::exit_vip(RuntimeOrigin::signed(10)),
            pallet_nac_managing::Error::<Test>::InvalidClaimAttribute
        );
        assert_eq!(owned_vipp_ids(10), vec![id]);
        assert!(Privileges::vip_members(10).is_some());
        assert!(Privileges::vipp_members(10).is_some());
    });
}

#[test]
fn vipp_last_threshold_burn_clears_current_points_and_prevents_reissuance() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(200);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 190);
        VippPoints::<Test>::insert(2019, 10, 123);

        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 100));
        NacManaging::check_account_threshold(&10);
        assert!(owned_vipp_ids(10).is_empty());
        assert!(Privileges::vipp_members(10).is_none());
        assert_eq!(Privileges::vipp_points(2019, 10), Some(123));
        assert!(Privileges::vip_members(10).is_some());

        System::set_block_number(2);
        claim_for_validator(200);
        System::set_block_number(3);
        claim_for_validator(100);
        assert!(owned_vipp_ids(10).is_empty());
        assert!(Privileges::vipp_members(10).is_none());
    });
}

#[test]
fn vipp_exit_burns_owned_nfts_even_if_the_cached_member_is_missing() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(200);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        VippMembers::<Test>::remove(10);
        assert_eq!(owned_vipp_ids(10).len(), 1);
        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert!(owned_vipp_ids(10).is_empty());
        assert!(NacManaging::is_vipp_inactive(&10));
    });
}

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
        assert_ok!(NacManaging::burn_vipp_nft(&10));
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, 5678);
        assert!(member.active_vipp_threshold.is_empty());
        assert_eq!(owned_vipp_ids(10), vec![retained_id]);
    });
}

#[test]
fn vip_stake_cache_uses_reduced_delegations_after_unbonding() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(100);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 500);

        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(100), 100));
        assert_eq!(
            EnergyGeneration::cooperators(100).unwrap().targets.values().sum::<Balance>(),
            400
        );
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 400);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 3));
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 900);
        // Preserve the existing Perquintill rounding: 500/51 -> 10, 400/52 -> 8.
        assert_eq!(Privileges::vip_members(100).unwrap().points, 18);
    });
}

#[test]
fn vip_stake_cache_tracks_each_validator_chill_without_wiping_points_or_nfts() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(100);
        let ids = owned_vipp_ids(100);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));

        assert_ok!(EnergyGeneration::chill(RuntimeOrigin::signed(10)));
        assert_eq!(EnergyGeneration::get_active_stake(&100), 300);
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 300);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 3));
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 800);

        assert_ok!(EnergyGeneration::chill(RuntimeOrigin::signed(20)));
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 0);
        let vip_points = Privileges::vip_members(100).unwrap().points;
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 4));
        assert_eq!(Privileges::vipp_members(100).unwrap().points, 800);
        assert_eq!(Privileges::vip_members(100).unwrap().points, vip_points);
        assert_eq!(owned_vipp_ids(100), ids);
    });
}

#[test]
fn vip_stake_cache_tracks_forced_validator_removal() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(100);
        assert_ok!(EnergyGeneration::force_chill(RuntimeOrigin::root(), 10));
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 300);
        assert_ok!(EnergyGeneration::force_unstake(RuntimeOrigin::root(), 20, 0));
        assert_eq!(EnergyGeneration::get_active_stake(&100), 0);
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 0);
    });
}

#[test]
fn vip_stake_cache_tracks_validator_chilled_by_a_vip_exit_penalty() {
    ExtBuilder::default().min_common_validator_bond(500).build_and_execute(|| {
        join_vip_and_vipp(100);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(20), PenaltyType::Flat));
        let ids = owned_vipp_ids(100);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 2, 1));
        let points = Privileges::vipp_members(100).unwrap().points;
        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(20)));
        assert!(!EnergyGeneration::is_user_validator(&20));
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 200);
        assert_eq!(Privileges::vipp_members(100).unwrap().points, points);
        assert_eq!(owned_vipp_ids(100), ids);
    });
}

#[test]
fn vip_stake_cache_tracks_cooperator_becoming_a_validator_without_resetting_points() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(100);
        assert_ok!(EnergyGeneration::bond_extra(RuntimeOrigin::signed(100), 100));
        // The extra solo stake is inactive while this account is a cooperator.
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 500);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));
        let points = Privileges::vip_members(100).unwrap().points;
        let vipp = Privileges::vipp_members(100).unwrap();
        let ids = owned_vipp_ids(100);

        assert_ok!(EnergyGeneration::validate(RuntimeOrigin::signed(100), Default::default()));
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 600);
        assert_eq!(Privileges::vip_members(100).unwrap().points, points);
        assert_eq!(Privileges::vipp_members(100), Some(vipp));
        assert_eq!(owned_vipp_ids(100), ids);
    });
}

#[test]
fn vip_stake_cache_tracks_validator_switching_to_cooperation() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        join_vip_and_vipp(100);
        let ids = owned_vipp_ids(10);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));
        let vip_points = Privileges::vip_members(10).unwrap().points;
        let vipp_points = Privileges::vipp_members(10).unwrap().points;

        assert_ok!(EnergyGeneration::cooperate(RuntimeOrigin::signed(10), vec![(20, 700)]));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 700);
        assert_eq!(EnergyGeneration::get_active_stake(&100), 300);
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 300);
        assert_eq!(Privileges::vip_members(10).unwrap().points, vip_points);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, vipp_points);
        assert_eq!(owned_vipp_ids(10), ids);
    });
}

#[test]
fn vip_stake_cache_uses_stash_ledger_when_joining_with_a_separate_controller() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(EnergyGeneration::set_controller(RuntimeOrigin::signed(10), 11));
        join_vip_and_vipp(10);
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 1000);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));
        assert_eq!(Privileges::vip_members(10).unwrap().points, 20);
        assert_eq!(Privileges::vipp_members(10).unwrap().points, 950);
    });
}

#[test]
fn vip_stake_cache_updates_stash_after_controller_changes() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        assert_ok!(EnergyGeneration::set_controller(RuntimeOrigin::signed(10), 11));
        assert_ok!(EnergyGeneration::unbond(RuntimeOrigin::signed(11), 100));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 900);
        assert_ok!(EnergyGeneration::rebond(RuntimeOrigin::signed(11), 50));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 950);
        assert_ok!(EnergyGeneration::bond_extra(RuntimeOrigin::signed(10), 100));
        assert_eq!(Privileges::vip_members(10).unwrap().active_stake, 1050);
        assert!(Privileges::vip_members(11).is_none());
    });
}

#[test]
fn vip_exit_penalizes_the_stash_with_a_separate_controller() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        assert_ok!(EnergyGeneration::set_controller(RuntimeOrigin::signed(10), 11));
        let controller_balance = Balances::free_balance(11);
        let stash_balance = Balances::free_balance(10);
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 2, 1));

        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert_eq!(EnergyGeneration::ledger(11).unwrap().active, 825);
        assert!(EnergyGeneration::ledger(10).is_none());
        assert_eq!(Balances::free_balance(10), stash_balance - 175);
        assert_eq!(Balances::free_balance(11), controller_balance);
        assert!(owned_vipp_ids(10).is_empty());
        assert!(Privileges::vip_members(10).is_none());
    });
}

#[test]
fn vip_stake_cache_updates_after_cooperator_slashing() {
    use frame_support::traits::Imbalance;
    use pallet_energy_generation::slashing::{self, SlashEntity};
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(100);
        let mut imbalance = slashing::NegativeImbalanceOf::<Test>::zero();
        assert_ok!(slashing::do_slash::<Test>(
            &100,
            SlashEntity { stake: 100, reputation: Default::default() },
            &mut 0,
            &mut imbalance,
            0,
        ));
        assert_eq!(EnergyGeneration::ledger(100).unwrap().active, 400);
        assert_eq!(
            EnergyGeneration::cooperators(100).unwrap().targets.values().sum::<Balance>(),
            400
        );
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 400);
    });
}

#[test]
fn vip_stake_cache_updates_when_validator_loses_collaborative_eligibility() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(100);
        // Keep validators eligible for solo staking while raising the collaboration tier.
        CollaborativeValidatorReputationTier::set(pallet_reputation::ReputationTier::Vanguard(2));
        pallet_energy_generation::ForceEra::<Test>::put(
            pallet_energy_generation::Forcing::ForceNew,
        );
        <EnergyGeneration as pallet_session::SessionManager<AccountId>>::new_session(1);
        CollaborativeValidatorReputationTier::set(pallet_reputation::ReputationTier::Vanguard(1));
        assert!(EnergyGeneration::is_user_validator(&10));
        assert!(EnergyGeneration::collaborations(10).is_none());
        assert_eq!(EnergyGeneration::get_active_stake(&100), 0);
        assert_eq!(Privileges::vip_members(100).unwrap().active_stake, 0);
    });
}

#[test]
fn vipp_threshold_burn_selects_numeric_minimum_and_preserves_remaining_points() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(256);
        assert_ok!(Privileges::become_vip_status(RuntimeOrigin::signed(10), PenaltyType::Flat));
        let original_id = owned_vipp_ids(10)[0];
        assert_ok!(advance_test_date(RuntimeOrigin::root(), 2020, 1, 2));
        let original_points = Privileges::vipp_members(10).unwrap().points;
        System::set_block_number(2);
        claim_for_validator(1);
        assert_eq!(owned_vipp_ids(10).len(), 2);

        // 243 is below floor(95% of 257) = 244 and exactly covers floor(95% of 256).
        // SCALE(256) starts with 0, whereas SCALE(1) starts with 1: byte ordering is wrong.
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), 10, 243));
        NacManaging::check_account_threshold(&10);
        assert_eq!(owned_vipp_ids(10), vec![original_id]);
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, original_points);
        assert_eq!(member.active_vipp_threshold, vec![(original_id, 243)]);
    });
}
