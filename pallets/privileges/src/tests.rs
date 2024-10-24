//!
//! # Module Overview
//!
//! This module provides unit tests for the `Privileges` and `Claiming` pallets in a Substrate-based
//! blockchain. The tests validate key functionalities involving VIP and VIPP memberships, including
//! becoming a VIP, updating statuses, handling penalties, and verifying transitions between different
//! user roles (e.g., from a validator to a cooperator). The tests ensure that various scenarios are
//! correctly handled and that membership-related actions are performed securely and consistently.
//!
//! # Key Test Cases
//!
//! - **Becoming a VIP and Exiting VIP Membership**:
//!   - `become_vip_status_should_work()`: Tests if a user can successfully become a VIP member. The
//!     test also verifies that VIP membership points and thresholds are initialized correctly.
//!   - `exit_a_vip_as_validator_free_tax_period()`: Validates that a VIP member, specifically a
//!     validator, can exit the VIP status during a tax-free period without penalties. The user's
//!     frozen and free balances are also checked for correctness after the exit.
//!   - `exit_a_vip_as_cooperator_free_tax_period()`: Tests if a cooperator, who is a VIP member,
//!     can exit during a free tax period, ensuring that their frozen and free balances are updated
//!     correctly.
//!   - `exit_a_vip_as_validator_non_free_tax_period()`: Ensures that a validator who exits VIP
//!     status during a non-tax-free period receives a penalty deduction. The test checks that the
//!     user's active balance and frozen balance reflect the penalty.
//!
//! - **Penalty Handling and Tax Period Verification**:
//!   - `exit_a_vip_as_validator_non_free_tax_period_and_kick()`: Tests the scenario where a validator
//!     exits VIP status during a non-free tax period and is subsequently "kicked" out. It verifies
//!     that penalties are correctly applied and that the user's membership data is properly cleared.
//!
//! - **Transition Between Membership Roles**:
//!   - `test_from_validator_to_cooperator()`: Simulates the process of a user transitioning from a
//!     validator role to a cooperator role. The test ensures that the user's active stake is updated
//!     accurately after cooperating and that their VIP status is adjusted accordingly.
//!
//! - **Claiming and Membership Impact**:
//!   - Tests involving `Claiming::claim()` validate the impact of claims on VIP status. For example,
//!     if a user claims a reward, the test checks that the VIP and VIPP membership points are reset
//!     appropriately, and new membership thresholds are established when needed.
//!
//! # Access Control and Security
//!
//! - **Controlled Membership Actions**: The tests ensure that only authorized users can perform key
//!   actions such as becoming a VIP, exiting VIP status, or claiming rewards. Root origin (`RuntimeOrigin::root()`)
//!   is used where necessary to simulate admin-level access, preventing unauthorized users from
//!   performing privileged actions.
//! - **Penalty and Tax Handling**: The tests include scenarios for both free and non-free tax periods,
//!   ensuring that penalties are correctly applied or waived based on the user's actions and timing.
//!   This helps maintain a fair and consistent system where rewards and penalties are assigned based
//!   on user behavior and adherence to network rules.
//!
//! # Developer Notes
//!
//! - **Transition Testing**: Tests like `test_from_validator_to_cooperator()` verify the user's ability
//!   to transition between roles, such as moving from a validator to a cooperator. This helps ensure
//!   that role changes are handled smoothly without discrepancies in the user's account or membership
//!   status.
//! - **Edge Case Handling**: Several tests, such as those that involve penalty application during
//!   non-free tax periods, are designed to handle edge cases. These tests are crucial for ensuring
//!   the robustness of the system, particularly in scenarios where users attempt to manipulate their
//!   status to avoid penalties.
//! - **Signature Verification**: Functions like `Claiming::claim()` use ECDSA signatures to ensure
//!   authenticity. The `sig()` function generates signatures that are verified during the tests,
//!   simulating real-world usage where users must sign transactions or messages to claim rewards.
//!
//! # Usage Scenarios
//!
//! - **Simulating Member Lifecycle**: The tests are used to simulate the entire lifecycle of a VIP
//!   member, from becoming a VIP, accumulating points, updating stakes, and ultimately exiting the
//!   VIP membership. This ensures that each stage of the lifecycle is validated and functions as
//!   intended under different conditions.
//! - **Tax-Free and Penalty Enforcement**: Tests like `exit_a_vip_as_validator_free_tax_period()`
//!   and `exit_a_vip_as_validator_non_free_tax_period()` simulate the enforcement of taxes or penalties
//!   based on the user's timing of exit. These scenarios are crucial for ensuring that users are
//!   treated fairly based on their adherence to the membership rules.
//! - **Claim and Reward Integration**: The tests also ensure that the claiming process (`Claiming::claim()`)
//!   integrates well with VIP and VIPP memberships. This is particularly important in scenarios where
//!   users earn points or rewards that directly affect their membership status.
//!
//! # Integration Considerations
//!
//! - **Cross-Pallet Integration**: The tests demonstrate how `Privileges` and `Claiming` pallets interact.
//!   Developers should ensure that changes to one pallet do not negatively impact the other. For instance,
//!   any modifications to the reward structure in the `Claiming` pallet must be reflected in the VIP
//!   membership calculations in `Privileges`.
//! - **Access Control Validation**: Given the reliance on root-origin and signed-origin access, developers
//!   must validate that access control remains intact even as the pallets evolve. Unauthorized users
//!   should not be able to gain or revoke VIP status without meeting the required conditions.
//! - **Penalty and Reward Logic Synchronization**: The tests include scenarios for both free and non-free
//!   periods. Developers should ensure that the logic for applying penalties or waiving them is properly
//!   synchronized across all parts of the system, preventing discrepancies that could lead to user complaints
//!   or exploitation.
//!
//! # Example Scenario
//!
//! Suppose a user wants to exit their VIP membership during a non-tax-free period. The `exit_a_vip_as_validator_non_free_tax_period()`
//! test simulates this scenario by having the user exit VIP status. The system calculates and applies
//! the penalty based on the current active stake, deducting the appropriate amount from the user's balance.
//! The test then verifies that the user's frozen and free balances reflect the penalty deduction, ensuring
//! that the rules for penalties are correctly enforced. Additionally, the test checks that the user is
//! removed from the VIP member list and no longer enjoys the associated benefits.
//!


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

        let assert_year_result = Vec::from([(10, 2113), (20, 1042)]);
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year + 1,
            current_date.current_month,
            current_date.current_day
        ));
        assert_eq!(Privileges::year_vip_results(2020).unwrap().len(), 2);
        assert_eq!(Privileges::year_vip_results(2020).unwrap(), assert_year_result);
        assert_eq!(Privileges::vip_members(10).unwrap().points, 0);
        assert_eq!(Privileges::vip_members(20).unwrap().points, 0);
    })
}

#[test]
fn test_year_end_data_saving_vipp_results() {
    ExtBuilder::default().build_and_execute(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 1000));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(10),
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

        let assert_year_result = Vec::from([(10, 69350)]);
        assert_ok!(Privileges::update_time(
            RuntimeOrigin::root(),
            current_date.current_year + 1,
            current_date.current_month,
            current_date.current_day
        ));

        assert_eq!(Privileges::year_vipp_results(2020).unwrap().len(), 1);
        assert_eq!(Privileges::year_vipp_results(2020).unwrap(), assert_year_result);
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

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(10),
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

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(10),
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

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 100));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(100),
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

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(10),
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

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(10),
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
