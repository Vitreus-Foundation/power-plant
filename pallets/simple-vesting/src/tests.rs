//! Unit Tests for Simple Vesting Pallet
//!
//! This module provides unit tests for the Simple Vesting pallet, verifying the functionality of vesting-related operations.
//! It utilizes the mock runtime from `mock.rs` to simulate blockchain behavior and ensure that the vesting logic works as intended.
//!
//! # Features
//! - Tests various scenarios for the vesting pallet, such as successful vesting transfers, balance checks, and error conditions.
//! - Uses `assert_ok` and `assert_noop` macros to validate both valid and invalid operations.
//! - Ensures that the pallet correctly manages the locked balances over time based on the vesting schedule.
//!
//! # Structure
//! - Imports the mock runtime and other pallet components including `VestingInfo`, `Error`, and relevant events.
//! - Contains multiple unit tests that exercise key functionalities, focusing on both success paths and error handling.
//! - Uses the `Currency` trait to manipulate balances directly, providing a realistic testing scenario for funds management.
//!
//! # Tests Overview
//! - **vest_works**: Tests the basic vesting functionality where `BOB` receives a vested transfer from `ALICE` and checks the expected balance changes.
//! - **other tests**: Additional tests should cover various vesting-related scenarios, such as edge cases for insufficient balances, incorrect vesting parameters, and vesting expiration.
//!
//! # Usage
//! - Use these tests to verify the correctness of the Simple Vesting pallet, especially after changes or updates to the vesting logic.
//! - Run the tests using `cargo test` in the Substrate development environment.
//!
//! # Dependencies
//! - Uses `frame_support` for testing utilities, including `assert_ok` and `assert_noop`.
//! - Relies on `sp_runtime` for handling errors and dispatch results.
//! - Imports pallet constants and types, such as `VestingInfo`, to ensure consistency with the pallet's implementation.
//!
//! # Important Notes
//! - Ensure that the mock runtime is properly configured before running the tests to avoid misleading outcomes.
//! - Expand the unit tests to include more comprehensive coverage as additional features are added to the vesting pallet.


use crate::{mock::*, Error, Event, VestingInfo};
use frame_support::traits::Currency;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

#[test]
fn vest_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, 2 * ED));
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 10 * ED, per_block: 3 * ED, starting_block: 5 }
        ));

        // Still locked
        System::set_block_number(5);
        assert_ok!(SimpleVesting::vest(RuntimeOrigin::signed(BOB)));
        assert_eq!(Balances::total_balance(&BOB), 12 * ED);
        assert_eq!(Balances::free_balance(&BOB), 2 * ED);
        assert_eq!(Balances::reserved_balance(&BOB), 10 * ED);

        // Unlock first 3 units
        System::set_block_number(6);
        assert_ok!(SimpleVesting::vest(RuntimeOrigin::signed(BOB)));
        assert_eq!(Balances::free_balance(&BOB), 5 * ED);
        assert_eq!(Balances::reserved_balance(&BOB), 7 * ED);
        System::assert_last_event(Event::VestingUpdated { account: BOB, unvested: 7 * ED }.into());

        // Unlock another 6 units
        System::set_block_number(8);
        assert_ok!(SimpleVesting::vest(RuntimeOrigin::signed(BOB)));
        assert_eq!(Balances::free_balance(&BOB), 11 * ED);
        assert_eq!(Balances::reserved_balance(&BOB), 1 * ED);
        System::assert_last_event(Event::VestingUpdated { account: BOB, unvested: 1 * ED }.into());

        // Unlock the rest
        System::set_block_number(9);
        assert_ok!(SimpleVesting::vest(RuntimeOrigin::signed(BOB)));
        assert_eq!(Balances::total_balance(&BOB), 12 * ED);
        assert_eq!(Balances::free_balance(&BOB), 12 * ED);
        assert_eq!(Balances::reserved_balance(&BOB), 0);
        System::assert_last_event(Event::VestingCompleted { account: BOB }.into());

        assert_eq!(Balances::reserves(&BOB), vec![]);
        assert_eq!(SimpleVesting::vesting(BOB), None);
    });
}

#[test]
fn vest_all_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, ED));
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 20 * ED, per_block: 4 * ED, starting_block: 2 }
        ));

        assert_eq!(System::providers(&BOB), 2);

        // Still locked
        System::set_block_number(2);
        assert_ok!(SimpleVesting::vest(RuntimeOrigin::signed(BOB)));
        assert_eq!(Balances::total_balance(&BOB), 21 * ED);
        assert_eq!(Balances::free_balance(&BOB), ED);
        assert_eq!(Balances::reserved_balance(&BOB), 20 * ED);

        // Unlock all
        System::set_block_number(7); // 2 + (20 / 4)
        assert_ok!(SimpleVesting::vest(RuntimeOrigin::signed(BOB)));
        assert_eq!(Balances::total_balance(&BOB), 21 * ED);
        assert_eq!(Balances::free_balance(&BOB), 21 * ED);
        assert_eq!(Balances::reserved_balance(&BOB), 0);
        System::assert_last_event(Event::VestingCompleted { account: BOB }.into());

        assert_eq!(SimpleVesting::vesting(BOB), None);
        assert_eq!(Balances::reserves(&BOB), vec![]);
        assert_eq!(System::providers(&BOB), 1);
    });
}

#[test]
fn vest_not_vesting_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(SimpleVesting::vest(RuntimeOrigin::signed(ALICE)), Error::<Test>::NotVesting);
    });
}

#[test]
fn force_vested_transfer_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, ED));
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));

        assert_eq!(
            SimpleVesting::vesting(BOB),
            Some(VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 })
        );
        assert_eq!(System::providers(&BOB), 2);
        assert_eq!(Balances::free_balance(BOB), ED);
        assert_eq!(Balances::reserved_balance(BOB), 3 * ED);
    });
}

#[test]
fn force_vested_transfer_creating_account_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));
        assert_eq!(
            SimpleVesting::vesting(BOB),
            Some(VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 })
        );
        assert_eq!(System::providers(&BOB), 1);
        assert_eq!(Balances::free_balance(BOB), 0);
        assert_eq!(Balances::reserved_balance(BOB), 3 * ED);
    });
}

#[test]
fn force_vested_transfer_non_root_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            SimpleVesting::force_vested_transfer(
                Some(ALICE).into(),
                ALICE,
                BOB,
                VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn force_vested_transfer_invalid_schedule_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            SimpleVesting::force_vested_transfer(
                RuntimeOrigin::root(),
                ALICE,
                BOB,
                VestingInfo { locked: 0, per_block: 10, starting_block: 20 }
            ),
            Error::<Test>::InvalidScheduleParams,
        );

        assert_noop!(
            SimpleVesting::force_vested_transfer(
                RuntimeOrigin::root(),
                ALICE,
                BOB,
                VestingInfo { locked: 3 * ED, per_block: 0, starting_block: 20 }
            ),
            Error::<Test>::InvalidScheduleParams,
        );
    });
}

#[test]
fn force_vested_transfer_already_vesting_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));
        assert_noop!(
            SimpleVesting::force_vested_transfer(
                RuntimeOrigin::root(),
                ALICE,
                BOB,
                VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
            ),
            Error::<Test>::AlreadyVesting,
        );
    });
}

#[test]
fn force_remove_vesting_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, ED));
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));

        assert_ok!(SimpleVesting::force_remove_vesting(RuntimeOrigin::root(), BOB));

        assert_eq!(System::providers(&BOB), 1);
        assert_eq!(Balances::free_balance(&BOB), ED);
        assert_eq!(Balances::reserved_balance(&BOB), 0);
        assert_eq!(SimpleVesting::vesting(BOB), None);
    });
}

#[test]
fn force_remove_vesting_after_vested_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), BOB, ED));
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: ED, starting_block: 20 }
        ));

        System::set_block_number(21);
        assert_ok!(SimpleVesting::vest(RuntimeOrigin::signed(BOB)));
        assert_eq!(Balances::free_balance(&BOB), 2 * ED);
        assert_eq!(Balances::reserved_balance(&BOB), 2 * ED);

        assert_ok!(SimpleVesting::force_remove_vesting(RuntimeOrigin::root(), BOB));

        assert_eq!(Balances::free_balance(&BOB), 2 * ED);
        assert_eq!(Balances::reserved_balance(&BOB), 0);
        assert_eq!(SimpleVesting::vesting(BOB), None);
    });
}

#[test]
fn force_remove_vesting_without_free_balance_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));

        assert_ok!(SimpleVesting::force_remove_vesting(RuntimeOrigin::root(), BOB));

        assert_eq!(System::providers(&BOB), 0);
        assert_eq!(Balances::free_balance(&BOB), 0);
        assert_eq!(Balances::reserved_balance(&BOB), 0);
        assert_eq!(SimpleVesting::vesting(BOB), None);
    });
}

#[test]
fn force_remove_vesting_non_root_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));
        assert_noop!(
            SimpleVesting::force_remove_vesting(Some(ALICE).into(), BOB),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn force_remove_vesting_dec_providers_correctly_fails() {
    new_test_ext().execute_with(|| {
        assert_ok!(SimpleVesting::force_vested_transfer(
            RuntimeOrigin::root(),
            ALICE,
            BOB,
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));

        // increment consumers, so `dec_providers` fails
        assert_ok!(frame_system::Pallet::<Test>::inc_consumers(&BOB));

        assert_noop!(
            SimpleVesting::force_remove_vesting(RuntimeOrigin::root(), BOB),
            DispatchError::ConsumerRemaining
        );
    });
}

#[test]
fn force_remove_vesting_not_vesting_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            SimpleVesting::force_remove_vesting(RuntimeOrigin::root(), BOB),
            Error::<Test>::NotVesting
        );
    });
}
