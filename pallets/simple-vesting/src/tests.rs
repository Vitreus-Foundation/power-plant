use crate::{mock::*, Error, VestingInfo};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

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
            VestingInfo { locked: 3 * ED, per_block: 10, starting_block: 20 }
        ));

        assert_ok!(SimpleVesting::do_vest(BOB, ED));
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
