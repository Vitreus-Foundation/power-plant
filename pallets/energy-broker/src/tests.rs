use crate::{mock::*, *};
use frame_support::{
    assert_noop, assert_ok,
    traits::fungibles::{Inspect, Mutate},
};
use sp_runtime::{DispatchError, TokenError};

const NATIVE_TOKEN: NativeOrAssetId = NativeOrAssetId::Native;
const ENERGY_TOKEN: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());

fn balance(owner: u128) -> u128 {
    <<Test as Config>::Assets>::balance(NATIVE_TOKEN, &owner)
}

fn energy_balance(owner: u128) -> u128 {
    <<Test as Config>::Assets>::balance(ENERGY_TOKEN, &owner)
}

fn get_ed() -> u128 {
    <<Test as Config>::Assets>::minimum_balance(NATIVE_TOKEN)
}

fn get_energy_ed() -> u128 {
    <<Test as Config>::Assets>::minimum_balance(ENERGY_TOKEN)
}

fn get_energy_total_issuance() -> u128 {
    <<Test as Config>::Assets>::total_issuance(ENERGY_TOKEN)
}

fn set_balances(who: u128, balance: u128, energy_balance: u128) {
    <Test as Config>::Assets::set_balance(NATIVE_TOKEN, &who, balance);
    <Test as Config>::Assets::set_balance(ENERGY_TOKEN, &who, energy_balance);
}

#[test]
fn get_amount_works() {
    new_test_ext().execute_with(|| {
        let amount_in = 100000;
        let (amount_out, fee) =
            EnergyBroker::get_amount_out(amount_in, &(NATIVE_TOKEN, ENERGY_TOKEN)).unwrap();

        let (expected_amount_in, expected_fee) =
            EnergyBroker::get_amount_in(amount_out, &(NATIVE_TOKEN, ENERGY_TOKEN)).unwrap();

        assert_eq!(amount_in, expected_amount_in);
        assert_eq!(fee, expected_fee);
    });
}

#[test]
fn can_swap_native_for_exact_energy() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();

        let alice_balance = balance(ALICE);
        let broker_balance = balance(broker_account);

        let alice_energy = energy_balance(ALICE);
        let broker_energy = energy_balance(broker_account);

        let exchange_out = 1000;
        let expect_in = 100;
        let expect_fee = 2;

        assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
            RuntimeOrigin::signed(ALICE),
            (NATIVE_TOKEN, ENERGY_TOKEN),
            exchange_out,
            None,
            true,
        ));

        assert_eq!(balance(ALICE), alice_balance - expect_in - expect_fee);
        assert_eq!(balance(broker_account), broker_balance + expect_in);
        assert_eq!(balance(FeeAccount::get()), get_ed() + expect_fee);

        assert_eq!(energy_balance(ALICE), alice_energy + exchange_out);
        assert_eq!(energy_balance(broker_account), broker_energy - exchange_out);
    });
}

#[test]
fn can_swap_exact_native_for_energy() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();

        let alice_balance = balance(ALICE);
        let broker_balance = balance(broker_account);

        let alice_energy = energy_balance(ALICE);
        let broker_energy = energy_balance(broker_account);

        let exchange_in = 98;
        let expect_out = 980;
        let expect_fee = 2;

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            (NATIVE_TOKEN, ENERGY_TOKEN),
            exchange_in + expect_fee,
            None,
            true,
        ));

        assert_eq!(balance(ALICE), alice_balance - exchange_in - expect_fee);
        assert_eq!(balance(broker_account), broker_balance + exchange_in);
        assert_eq!(balance(FeeAccount::get()), get_ed() + expect_fee);

        assert_eq!(energy_balance(ALICE), alice_energy + expect_out);
        assert_eq!(energy_balance(broker_account), broker_energy - expect_out);
    });
}

#[test]
fn can_swap_energy_for_exact_native() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();

        let alice_balance = balance(ALICE);
        let broker_balance = balance(broker_account);

        let alice_energy = energy_balance(ALICE);
        let broker_energy = energy_balance(broker_account);

        let exchange_out = 100;
        let expect_in = 1000;
        let expect_fee = 20;

        assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
            RuntimeOrigin::signed(ALICE),
            (ENERGY_TOKEN, NATIVE_TOKEN),
            exchange_out,
            None,
            true,
        ));

        assert_eq!(balance(ALICE), alice_balance + exchange_out);
        assert_eq!(balance(broker_account), broker_balance - exchange_out);

        assert_eq!(energy_balance(ALICE), alice_energy - expect_in - expect_fee);
        assert_eq!(energy_balance(broker_account), broker_energy + expect_in);
        assert_eq!(energy_balance(FeeAccount::get()), get_energy_ed() + expect_fee);
    });
}

#[test]
fn can_swap_exact_energy_for_native() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();

        let alice_balance = balance(ALICE);
        let broker_balance = balance(broker_account);

        let alice_energy = energy_balance(ALICE);
        let broker_energy = energy_balance(broker_account);

        let exchange_in = 980;
        let expect_out = 98;
        let expect_fee = 20;

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            (ENERGY_TOKEN, NATIVE_TOKEN),
            exchange_in + expect_fee,
            None,
            true,
        ));

        assert_eq!(balance(ALICE), alice_balance + expect_out);
        assert_eq!(balance(broker_account), broker_balance - expect_out);

        assert_eq!(energy_balance(ALICE), alice_energy - exchange_in - expect_fee);
        assert_eq!(energy_balance(broker_account), broker_energy + exchange_in);
        assert_eq!(energy_balance(FeeAccount::get()), get_energy_ed() + expect_fee);
    });
}

#[test]
fn swap_with_amount_out_min_works() {
    new_test_ext().execute_with(|| {
        let amount_in = 200;
        let (amount_out, _) =
            EnergyBroker::get_amount_out(amount_in, &(NATIVE_TOKEN, ENERGY_TOKEN)).unwrap();

        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                (NATIVE_TOKEN, ENERGY_TOKEN),
                amount_in,
                Some(amount_out + 1),
                true
            ),
            Error::<Test>::ProvidedMinimumNotSufficientForSwap
        );

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            (NATIVE_TOKEN, ENERGY_TOKEN),
            amount_in,
            Some(amount_out),
            true
        ));
    });
}

#[test]
fn swap_with_amount_in_max_works() {
    new_test_ext().execute_with(|| {
        let amount_out = 200;
        let (amount_in, _) =
            EnergyBroker::get_amount_in(amount_out, &(NATIVE_TOKEN, ENERGY_TOKEN)).unwrap();

        assert_noop!(
            EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(ALICE),
                (NATIVE_TOKEN, ENERGY_TOKEN),
                amount_out,
                Some(amount_in - 1),
                true
            ),
            Error::<Test>::ProvidedMaximumNotSufficientForSwap
        );

        assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
            RuntimeOrigin::signed(ALICE),
            (NATIVE_TOKEN, ENERGY_TOKEN),
            amount_out,
            Some(amount_in),
            true
        ));
    });
}

#[test]
fn swap_without_keep_alive_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            (ENERGY_TOKEN, NATIVE_TOKEN),
            energy_balance(ALICE),
            None,
            false,
        ));
        assert_eq!(energy_balance(ALICE), 0);

        frame_system::Pallet::<Test>::inc_providers(&ALICE);
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            (NATIVE_TOKEN, ENERGY_TOKEN),
            balance(ALICE),
            None,
            false,
        ));

        assert_eq!(balance(ALICE), 0);
    });
}

#[test]
fn swap_when_existential_deposit_would_cause_reaping_but_keep_alive_set() {
    new_test_ext().execute_with(|| {
        let liquidity = 100;

        set_balances(ALICE, liquidity + get_ed(), liquidity + get_energy_ed());

        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                (NATIVE_TOKEN, ENERGY_TOKEN),
                liquidity + 1,
                None,
                true
            ),
            DispatchError::Token(TokenError::NotExpendable)
        );

        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                (ENERGY_TOKEN, NATIVE_TOKEN),
                liquidity + 1,
                None,
                true
            ),
            DispatchError::Token(TokenError::NotExpendable)
        );
    });
}

#[test]
fn can_not_swap_without_liquidity() {
    new_test_ext().execute_with(|| {
        let liquidity = 100;

        set_balances(EnergyBroker::account_id(), liquidity + get_ed(), liquidity + get_energy_ed());

        assert_noop!(
            EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(ALICE),
                (NATIVE_TOKEN, ENERGY_TOKEN),
                liquidity + 1,
                None,
                true
            ),
            Error::<Test>::InsufficientLiquidity
        );

        assert_noop!(
            EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(ALICE),
                (ENERGY_TOKEN, NATIVE_TOKEN),
                liquidity + 1,
                None,
                true
            ),
            Error::<Test>::InsufficientLiquidity
        );
    });
}

#[test]
fn can_not_swap_zero_amount() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                (NATIVE_TOKEN, ENERGY_TOKEN),
                0,
                None,
                true
            ),
            Error::<Test>::ZeroAmount
        );

        assert_noop!(
            EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(ALICE),
                (NATIVE_TOKEN, ENERGY_TOKEN),
                0,
                None,
                true
            ),
            Error::<Test>::ZeroAmount
        );

        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                (ENERGY_TOKEN, NATIVE_TOKEN),
                0,
                None,
                true
            ),
            Error::<Test>::ZeroAmount
        );

        assert_noop!(
            EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(ALICE),
                (ENERGY_TOKEN, NATIVE_TOKEN),
                0,
                None,
                true
            ),
            Error::<Test>::ZeroAmount
        );

        // amount_out = 0
        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                (ENERGY_TOKEN, NATIVE_TOKEN),
                1,
                None,
                true
            ),
            Error::<Test>::ZeroAmount
        );
    });
}

#[test]
fn burn_energy_beyond_capacity() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();
        set_balances(broker_account, 1000, EnergyCapacity::get().saturating_sub(100));

        let energy_issuance = get_energy_total_issuance();

        let alice_balance = balance(ALICE);
        let alice_energy = energy_balance(ALICE);

        // the broker saves 100 energy and burns 880
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            (ENERGY_TOKEN, NATIVE_TOKEN),
            1000,
            None,
            true
        ));

        assert_eq!(balance(ALICE), alice_balance + 98);
        assert_eq!(energy_balance(ALICE), alice_energy - 1000);

        assert_eq!(energy_balance(broker_account), EnergyCapacity::get());
        assert_eq!(get_energy_total_issuance(), energy_issuance - 880);

        // the broker is full, but an user can swap anyway
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            (ENERGY_TOKEN, NATIVE_TOKEN),
            1000,
            None,
            true
        ));

        assert_eq!(balance(ALICE), alice_balance + 98 + 98);
        assert_eq!(energy_balance(ALICE), alice_energy - 1000 - 1000);

        assert_eq!(energy_balance(broker_account), EnergyCapacity::get());
        assert_eq!(get_energy_total_issuance(), energy_issuance - 880 - 980);
    });
}

#[test]
fn swap_tokens_for_exact_tokens_works_for_low_amount_out() {
    new_test_ext().execute_with(|| {
        let alice_balance_before = balance(ALICE);
        let alice_energy_before = energy_balance(ALICE);

        // amount_in = 1, even though 5 / 10 = 0
        assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
            RuntimeOrigin::signed(ALICE),
            (NATIVE_TOKEN, ENERGY_TOKEN),
            5,
            None,
            true
        ));

        assert_eq!(balance(ALICE), alice_balance_before - 1);
        assert_eq!(energy_balance(ALICE), alice_energy_before + 5);
    });
}
