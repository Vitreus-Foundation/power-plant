use super::*;
use crate::mock::*;
use crate::{Error, PenaltyType};
use frame_support::{assert_err, assert_ok};

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

        let vip_points = EnergyGeneration::ledger(10).unwrap().active * 7 / 40;
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

        let vip_points = (200 + 300) / 4;
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
        assert!(!EnergyGeneration::is_user_validator(&20));
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
        let _current_date = Privileges::current_date();

        assert_eq!(Privileges::vip_members(10).unwrap().points, 982);
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

        let assert_year_result = Vec::from([(10, 1169), (20, 524)]);
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year + 1,
            current_date.current_month,
            current_date.current_day
        ));
        assert_eq!(Privileges::year_vip_results(2020).unwrap().len(), 2);
        assert_eq!(Privileges::year_vip_results(2020).unwrap(), assert_year_result);
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

        let mut vip_points = EnergyGeneration::ledger(30).unwrap().active * 7 / 40;
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
        vip_points += (EnergyGeneration::ledger(30).unwrap().active * 7 / 40) / 2;
        assert_eq!(Privileges::vip_members(30).unwrap().points, vip_points);

        assert_eq!(EnergyGeneration::ledger(30).unwrap().active, 1500);
        assert_eq!(System::account(30).data.frozen, 1500);
        assert_eq!(System::account(30).data.free, 2000);
    })
}
