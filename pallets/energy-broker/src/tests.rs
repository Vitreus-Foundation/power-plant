use crate::{mock::*, *};
use frame_support::{
    assert_noop, assert_ok,
    traits::fungibles::{Inspect, Mutate},
};
use sp_runtime::{DispatchError, TokenError};
use vitreus_runtime_common::{OnEnergyBurn, OnSessionChange, Swap};

const NATIVE_TOKEN: NativeOrAssetId = NativeOrAssetId::Native;
const ENERGY_TOKEN: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());
const STATIC_ENERGY_TOKEN: NativeOrAssetId = NativeOrAssetId::WithId(SNRG::get());

fn balance(owner: u128) -> u128 {
    <<Test as Config>::Assets>::balance(NATIVE_TOKEN, &owner)
}

fn energy_balance(owner: u128) -> u128 {
    <<Test as Config>::Assets>::balance(ENERGY_TOKEN, &owner)
}

fn static_energy_balance(owner: u128) -> u128 {
    <<Test as Config>::Assets>::balance(STATIC_ENERGY_TOKEN, &owner)
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
            EnergyBroker::get_amount_out(amount_in, &(NATIVE_TOKEN, ENERGY_TOKEN), true).unwrap();

        let (expected_amount_in, expected_fee) =
            EnergyBroker::get_amount_in(amount_out, &(NATIVE_TOKEN, ENERGY_TOKEN), true).unwrap();

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
            ALICE,
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
            ALICE,
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
            ALICE,
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
            ALICE,
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
            EnergyBroker::get_amount_out(amount_in, &(NATIVE_TOKEN, ENERGY_TOKEN), true).unwrap();

        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                ALICE,
                (NATIVE_TOKEN, ENERGY_TOKEN),
                amount_in,
                Some(amount_out + 1),
                true
            ),
            Error::<Test>::ProvidedMinimumNotSufficientForSwap
        );

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
            (NATIVE_TOKEN, ENERGY_TOKEN),
            amount_in,
            Some(amount_out),
            true
        ));

        // A zero minimum is the same as none.
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
            (NATIVE_TOKEN, ENERGY_TOKEN),
            amount_in,
            Some(0),
            true
        ));
    });
}

#[test]
fn swap_with_amount_in_max_works() {
    new_test_ext().execute_with(|| {
        let amount_out = 200;
        let (amount_in, _) =
            EnergyBroker::get_amount_in(amount_out, &(NATIVE_TOKEN, ENERGY_TOKEN), true).unwrap();

        assert_noop!(
            EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(ALICE),
                ALICE,
                (NATIVE_TOKEN, ENERGY_TOKEN),
                amount_out,
                Some(amount_in - 1),
                true
            ),
            Error::<Test>::ProvidedMaximumNotSufficientForSwap
        );

        // A zero maximum cannot cover any swap.
        assert_noop!(
            EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(ALICE),
                ALICE,
                (NATIVE_TOKEN, ENERGY_TOKEN),
                amount_out,
                Some(0),
                true
            ),
            Error::<Test>::ProvidedMaximumNotSufficientForSwap
        );

        assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
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
            ALICE,
            (ENERGY_TOKEN, NATIVE_TOKEN),
            energy_balance(ALICE),
            None,
            false,
        ));
        assert_eq!(energy_balance(ALICE), 0);

        frame_system::Pallet::<Test>::inc_providers(&ALICE);
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
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
                ALICE,
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
                ALICE,
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
                ALICE,
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
                ALICE,
                (ENERGY_TOKEN, NATIVE_TOKEN),
                liquidity + 1,
                None,
                true
            ),
            Error::<Test>::InsufficientLiquidity
        );
    });
}

/// The mirror of `can_not_swap_without_liquidity`: a conversion mints its output, so the broker
/// holding none of it must not stop the swap. This is why the liquidity check goes through
/// `AssetConverter::reducible_balance` instead of reading the broker's balance directly.
#[test]
fn conversion_does_not_require_liquidity() {
    new_test_ext().execute_with(|| {
        let broker = EnergyBroker::account_id();
        let amount = 1000;

        // Leave the broker with far less energy than the swap must produce.
        <Test as Config>::Assets::set_balance(ENERGY_TOKEN, &broker, get_energy_ed());
        assert!(energy_balance(broker) < amount);

        let alice_energy_before = energy_balance(ALICE);
        let alice_static_before = static_energy_balance(ALICE);

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
            (STATIC_ENERGY_TOKEN, ENERGY_TOKEN),
            amount,
            None,
            false,
        ));

        assert_eq!(static_energy_balance(ALICE), alice_static_before - amount);
        assert_eq!(energy_balance(ALICE), alice_energy_before + amount);
    });
}

#[test]
fn can_not_swap_zero_amount() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(ALICE),
                ALICE,
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
                ALICE,
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
                ALICE,
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
                ALICE,
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
                ALICE,
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
fn energy_sale_is_recorded_for_a_real_trade() {
    new_test_ext().execute_with(|| {
        assert_eq!(energy_sold(), 0);

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
            (ENERGY_TOKEN, NATIVE_TOKEN),
            1000,
            None,
            true,
        ));

        assert!(energy_sold() > 0);
    });
}

#[test]
fn energy_sale_is_not_recorded_when_recipient_is_the_broker() {
    new_test_ext().execute_with(|| {
        let broker = EnergyBroker::account_id();
        let amount = 1000;

        let broker_energy_before = energy_balance(broker);
        let capacity_before = EnergyCapacity::<Test>::get();
        assert_eq!(energy_sold(), 0);

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            broker,
            (STATIC_ENERGY_TOKEN, ENERGY_TOKEN),
            amount,
            None,
            false,
        ));

        // The minted output did land on the broker, so the swap really happened ...
        assert_eq!(energy_balance(broker), broker_energy_before + amount);
        // ... and it was still not counted as a sale.
        assert_eq!(energy_sold(), 0);
        assert_eq!(EnergyCapacity::<Test>::get(), capacity_before);

        // Same swap to an ordinary recipient: also not a sale.
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
            (STATIC_ENERGY_TOKEN, ENERGY_TOKEN),
            amount,
            None,
            false,
        ));

        assert_eq!(energy_sold(), 0);
    });
}

#[test]
fn burn_energy_beyond_capacity() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();
        set_balances(broker_account, 1000, INITIAL_ENERGY_CAPACITY.saturating_sub(100));

        let energy_issuance = get_energy_total_issuance();

        let alice_balance = balance(ALICE);
        let alice_energy = energy_balance(ALICE);

        // the broker saves 100 energy and burns 880
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
            (ENERGY_TOKEN, NATIVE_TOKEN),
            1000,
            None,
            true
        ));

        assert_eq!(balance(ALICE), alice_balance + 98);
        assert_eq!(energy_balance(ALICE), alice_energy - 1000);

        assert_eq!(energy_balance(broker_account), INITIAL_ENERGY_CAPACITY);
        assert_eq!(get_energy_total_issuance(), energy_issuance - 880);

        // the broker is full, but an user can swap anyway
        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            ALICE,
            (ENERGY_TOKEN, NATIVE_TOKEN),
            1000,
            None,
            true
        ));

        assert_eq!(balance(ALICE), alice_balance + 98 + 98);
        assert_eq!(energy_balance(ALICE), alice_energy - 1000 - 1000);

        assert_eq!(energy_balance(broker_account), INITIAL_ENERGY_CAPACITY);
        assert_eq!(get_energy_total_issuance(), energy_issuance - 880 - 980);
    });
}

#[test]
fn energy_burn_is_summed_over_sliding_window() {
    new_test_ext().execute_with(|| {
        EnergyBroker::on_energy_burn(100);
        EnergyBroker::on_energy_burn(50);
        EnergyBroker::on_new_session(1);
        EnergyBroker::on_energy_burn(200);
        EnergyBroker::on_new_session(2);

        assert_eq!(EnergyBurn::<Test>::get(0), Some(150));
        assert_eq!(TotalEnergyBurn::<Test>::get(), 350);

        // The window is two sessions, so session 0 drops out when session 3 starts.
        EnergyBroker::on_energy_burn(400);
        EnergyBroker::on_new_session(3);

        assert_eq!(EnergyBurn::<Test>::get(0), None);
        assert_eq!(TotalEnergyBurn::<Test>::get(), 600);
    });
}

#[test]
fn capacity_follows_energy_burn_without_override() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();
        assert_ok!(EnergyBroker::force_set_capacity(RuntimeOrigin::root(), None));

        EnergyBroker::on_energy_burn(300);
        EnergyBroker::on_new_session(1);

        assert_eq!(EnergyCapacity::<Test>::get(), 300);
        assert_eq!(energy_balance(broker_account), 300);
    });
}

#[test]
fn capacity_does_not_drop_to_zero() {
    new_test_ext().execute_with(|| {
        assert_ok!(EnergyBroker::force_set_capacity(RuntimeOrigin::root(), None));

        EnergyBroker::on_energy_burn(300);
        EnergyBroker::on_new_session(1);

        // Nothing is burned for a whole window after that.
        EnergyBroker::on_new_session(2);
        EnergyBroker::on_new_session(3);

        assert_eq!(TotalEnergyBurn::<Test>::get(), 0);
        assert_eq!(EnergyCapacity::<Test>::get(), 300);
    });
}

#[test]
fn force_set_capacity_works() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            EnergyBroker::force_set_capacity(RuntimeOrigin::signed(ALICE), Some(100)),
            DispatchError::BadOrigin
        );
        assert_noop!(
            EnergyBroker::force_set_capacity(RuntimeOrigin::root(), Some(0)),
            Error::<Test>::ZeroAmount
        );

        // The override takes effect from the next session.
        assert_ok!(EnergyBroker::force_set_capacity(RuntimeOrigin::root(), Some(100)));
        assert_eq!(EnergyCapacity::<Test>::get(), INITIAL_ENERGY_CAPACITY);

        EnergyBroker::on_new_session(1);
        assert_eq!(EnergyCapacity::<Test>::get(), 100);
    });
}

#[test]
fn force_add_liquidity_works() {
    new_test_ext().execute_with(|| {
        let broker_account = EnergyBroker::account_id();
        let alice_energy = energy_balance(ALICE);
        let broker_energy = energy_balance(broker_account);

        assert_noop!(
            EnergyBroker::force_add_liquidity(
                RuntimeOrigin::signed(ALICE),
                ALICE,
                ENERGY_TOKEN,
                100,
                true
            ),
            DispatchError::BadOrigin
        );

        assert_ok!(EnergyBroker::force_add_liquidity(
            RuntimeOrigin::root(),
            ALICE,
            ENERGY_TOKEN,
            100,
            true,
        ));

        assert_eq!(energy_balance(ALICE), alice_energy - 100);
        assert_eq!(energy_balance(broker_account), broker_energy + 100);
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
            ALICE,
            (NATIVE_TOKEN, ENERGY_TOKEN),
            5,
            None,
            true
        ));

        assert_eq!(balance(ALICE), alice_balance_before - 1);
        assert_eq!(energy_balance(ALICE), alice_energy_before + 5);
    });
}

#[test]
fn swap_to_recipient_works() {
    new_test_ext().execute_with(|| {
        let alice_balance = balance(ALICE);

        let exchange_in = 98;
        let expect_out = 980;
        let expect_fee = 2;

        assert_eq!(energy_balance(BOB), 0);

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(ALICE),
            BOB,
            (NATIVE_TOKEN, ENERGY_TOKEN),
            exchange_in + expect_fee,
            None,
            true,
        ));

        assert_eq!(balance(ALICE), alice_balance - exchange_in - expect_fee);
        assert_eq!(energy_balance(BOB), expect_out);
    });
}

#[test]
fn feeless_account_swaps_without_fee() {
    new_test_ext().execute_with(|| {
        let bob_balance = balance(BOB);
        let fee_account_balance = balance(FeeAccount::get());

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(BOB),
            BOB,
            (NATIVE_TOKEN, ENERGY_TOKEN),
            100,
            None,
            true,
        ));
        assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
            RuntimeOrigin::signed(BOB),
            BOB,
            (NATIVE_TOKEN, ENERGY_TOKEN),
            1000,
            None,
            true,
        ));

        assert_eq!(balance(BOB), bob_balance - 200);
        assert_eq!(energy_balance(BOB), 2000);
        assert_eq!(balance(FeeAccount::get()), fee_account_balance);
    });
}

#[test]
fn swap_trait_returns_swapped_amounts() {
    new_test_ext().execute_with(|| {
        let bob_energy = energy_balance(BOB);
        let amount_out = <EnergyBroker as Swap<u128>>::swap_exact_tokens_for_tokens(
            ALICE,
            vec![NATIVE_TOKEN, ENERGY_TOKEN],
            100,
            None,
            BOB,
            true,
        );
        assert_eq!(amount_out, Ok(energy_balance(BOB) - bob_energy));

        let alice_balance = balance(ALICE);
        let amount_in = <EnergyBroker as Swap<u128>>::swap_tokens_for_exact_tokens(
            ALICE,
            vec![NATIVE_TOKEN, ENERGY_TOKEN],
            1000,
            None,
            BOB,
            true,
        );
        assert_eq!(amount_in, Ok(alice_balance - balance(ALICE)));
    });
}

#[test]
fn swap_trait_rejects_invalid_path() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            <EnergyBroker as Swap<u128>>::swap_exact_tokens_for_tokens(
                ALICE,
                vec![NATIVE_TOKEN],
                100,
                None,
                ALICE,
                true
            ),
            Error::<Test>::InvalidPath
        );
    });
}

#[test]
fn swap_trait_rolls_back_a_failed_swap() {
    new_test_ext().execute_with(|| {
        // Without a native balance the recipient cannot hold energy, so the swap fails only after
        // the input is taken and the fee is paid. All of it must be rolled back.
        let recipient = 3;
        assert_noop!(
            <EnergyBroker as Swap<u128>>::swap_exact_tokens_for_tokens(
                ALICE,
                vec![NATIVE_TOKEN, ENERGY_TOKEN],
                100,
                None,
                recipient,
                true
            ),
            Error::<Test>::BelowMinimum
        );
    });
}
