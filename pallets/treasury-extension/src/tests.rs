use crate::mock::*;
use crate::Event;
use frame_support::{assert_ok, traits::fungibles::Mutate};
use pallet_treasury::Event as TreasuryEvent;
use sp_runtime::{DispatchError, TokenError};
use vitreus_runtime_common::OnSessionChange;

#[test]
fn spend_funds_should_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let budget_remaining = Treasury::pot();
        let threshold = SpendThreshold::get().mul_ceil(budget_remaining);
        let spent = threshold - 1;
        let left = budget_remaining - threshold;

        Treasury::spend_local(RuntimeOrigin::root(), spent, ALICE)
            .expect("Expected to add and approve treasury spend proposal");
        Treasury::spend_funds();

        // making sure that Treasury hasn't emit Burnt event.
        let events = System::events();
        assert!(!events.iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Treasury(TreasuryEvent::<Test>::Burnt { .. })
        )),);

        System::assert_has_event(Event::<Test>::Recycled { recyled_funds: 1 }.into());
        assert_eq!(Treasury::pot(), left);
    });
}

#[test]
fn ensure_no_recycle_upon_spend_threhsold_exceeding() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let budget_remaining = Treasury::pot();
        let threshold = SpendThreshold::get().mul_ceil(budget_remaining);
        let spent = threshold;
        let left = budget_remaining - threshold;

        Treasury::spend_local(RuntimeOrigin::root(), spent, ALICE)
            .expect("Expected to add and approve treasury spend proposal");
        Treasury::spend_funds();

        // making sure that Treasury hasn't emit Burnt event
        // and TreasuryExtension hasn't emitted Recycled event.
        let events = System::events();
        assert!(!events.iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Treasury(TreasuryEvent::<Test>::Burnt { .. })
        ) | matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(Event::<Test>::Recycled { .. })
        )));

        assert_eq!(Treasury::pot(), left);
    });
}

/// Mints the treasury energy to sell, on top of the minimum balance it keeps.
fn mint_treasury_energy(energy: Balance, static_energy: Balance, liquid_energy: Balance) {
    let treasury = Treasury::account_id();
    assert_ok!(Assets::mint_into(ENERGY, &treasury, ENERGY_MIN_BALANCE + energy));
    assert_ok!(Assets::mint_into(STATIC_ENERGY, &treasury, ENERGY_MIN_BALANCE + static_energy));
    assert_ok!(Assets::mint_into(LIQUID_ENERGY, &treasury, ENERGY_MIN_BALANCE + liquid_energy));
}

#[test]
fn energy_above_minimum_is_sold_and_reported() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        mint_treasury_energy(10, 20, 30);
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        let native = (10 + 20 + 30) * EXCHANGE_RATE;
        System::assert_last_event(
            Event::<Test>::EnergyExchanged {
                energy: 10,
                static_energy: 20,
                liquid_energy: 30,
                native,
            }
            .into(),
        );
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(Event::<Test>::EnergyExchangeFailed { .. })
        )));
        for asset in [ENERGY, STATIC_ENERGY, LIQUID_ENERGY] {
            assert_eq!(Assets::balance(asset, treasury), ENERGY_MIN_BALANCE);
        }
        assert_eq!(Balances::free_balance(treasury), native_before + native);
    });
}

#[test]
fn energy_sale_stays_within_exchange_liquidity() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        mint_treasury_energy(10, 20, 30);
        // The exchanges can pay for 25 energy all together: the 10 Energy, then 15 of the 20
        // Static Energy, and none of the Liquid Energy.
        ExchangeLiquidity::set(&((10 + 15) * EXCHANGE_RATE));
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        let native = (10 + 15) * EXCHANGE_RATE;
        System::assert_last_event(
            Event::<Test>::EnergyExchanged {
                energy: 10,
                static_energy: 15,
                liquid_energy: 0,
                native,
            }
            .into(),
        );
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(Event::<Test>::EnergyExchangeFailed { .. })
        )));
        assert_eq!(Assets::balance(ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Assets::balance(STATIC_ENERGY, treasury), ENERGY_MIN_BALANCE + 5);
        assert_eq!(Assets::balance(LIQUID_ENERGY, treasury), ENERGY_MIN_BALANCE + 30);
        assert_eq!(Balances::free_balance(treasury), native_before + native);
    });
}

#[test]
fn energy_sale_stays_within_exchange_liquidity_despite_rounding_up() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        mint_treasury_energy(10, 20, 30);
        // The exchanges can pay for the 10 Energy and 15 of the 20 Static Energy, with a unit of
        // native currency to spare. For the 31 left after the Energy, the Static Energy exchange
        // quotes 16 Static Energy, rounded up, but they would buy 32.
        ExchangeLiquidity::set(&((10 + 15) * EXCHANGE_RATE + 1));
        RoundingUpExchange::set(&Some(STATIC_ENERGY));
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        let native = (10 + 15) * EXCHANGE_RATE;
        System::assert_last_event(
            Event::<Test>::EnergyExchanged {
                energy: 10,
                static_energy: 15,
                liquid_energy: 0,
                native,
            }
            .into(),
        );
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(Event::<Test>::EnergyExchangeFailed { .. })
        )));
        assert_eq!(Assets::balance(ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Assets::balance(STATIC_ENERGY, treasury), ENERGY_MIN_BALANCE + 5);
        assert_eq!(Assets::balance(LIQUID_ENERGY, treasury), ENERGY_MIN_BALANCE + 30);
        assert_eq!(Balances::free_balance(treasury), native_before + native);
    });
}

#[test]
fn nothing_to_sell_emits_no_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(
                Event::<Test>::EnergyExchanged { .. } | Event::<Test>::EnergyExchangeFailed { .. }
            )
        )));
    });
}

#[test]
fn empty_exchange_emits_no_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        mint_treasury_energy(10, 20, 30);
        ExchangeLiquidity::set(&0);
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(
                Event::<Test>::EnergyExchanged { .. } | Event::<Test>::EnergyExchangeFailed { .. }
            )
        )));
        assert_eq!(Assets::balance(ENERGY, treasury), ENERGY_MIN_BALANCE + 10);
        assert_eq!(Assets::balance(STATIC_ENERGY, treasury), ENERGY_MIN_BALANCE + 20);
        assert_eq!(Assets::balance(LIQUID_ENERGY, treasury), ENERGY_MIN_BALANCE + 30);
        assert_eq!(Balances::free_balance(treasury), native_before);
    });
}

#[test]
fn exchange_dust_emits_no_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        mint_treasury_energy(10, 20, 30);
        // The exchanges have less native currency than a unit of energy buys.
        ExchangeLiquidity::set(&(EXCHANGE_RATE - 1));
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(
                Event::<Test>::EnergyExchanged { .. } | Event::<Test>::EnergyExchangeFailed { .. }
            )
        )));
        assert_eq!(Assets::balance(ENERGY, treasury), ENERGY_MIN_BALANCE + 10);
        assert_eq!(Assets::balance(STATIC_ENERGY, treasury), ENERGY_MIN_BALANCE + 20);
        assert_eq!(Assets::balance(LIQUID_ENERGY, treasury), ENERGY_MIN_BALANCE + 30);
        assert_eq!(Balances::free_balance(treasury), native_before);
    });
}

#[test]
fn treasury_dust_emits_no_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        // A unit of Static Energy, which buys less than a unit of native currency here.
        mint_treasury_energy(0, 1, 0);
        CheapEnergyExchange::set(&Some(STATIC_ENERGY));
        // Plenty, but not unlimited: this exchange's quote for an unlimited payout would overflow.
        ExchangeLiquidity::set(&(100 * EXCHANGE_RATE));
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(
                Event::<Test>::EnergyExchanged { .. } | Event::<Test>::EnergyExchangeFailed { .. }
            )
        )));
        assert_eq!(Assets::balance(ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Assets::balance(STATIC_ENERGY, treasury), ENERGY_MIN_BALANCE + 1);
        assert_eq!(Assets::balance(LIQUID_ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Balances::free_balance(treasury), native_before);
    });
}

#[test]
fn failing_swap_is_reported() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        mint_treasury_energy(10, 20, 30);
        FailingExchange::set(&Some(STATIC_ENERGY));
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        System::assert_has_event(
            Event::<Test>::EnergyExchangeFailed {
                energy: None,
                static_energy: Some(TokenError::FundsUnavailable.into()),
                liquid_energy: None,
            }
            .into(),
        );
        let native = (10 + 30) * EXCHANGE_RATE;
        System::assert_last_event(
            Event::<Test>::EnergyExchanged {
                energy: 10,
                static_energy: 0,
                liquid_energy: 30,
                native,
            }
            .into(),
        );
        assert_eq!(Assets::balance(ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Assets::balance(STATIC_ENERGY, treasury), ENERGY_MIN_BALANCE + 20);
        assert_eq!(Assets::balance(LIQUID_ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Balances::free_balance(treasury), native_before + native);
    });
}

#[test]
fn unpriced_energy_is_reported_as_failed() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let treasury = Treasury::account_id();
        mint_treasury_energy(10, 20, 30);
        UnpricedExchange::set(&Some(STATIC_ENERGY));
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as OnSessionChange>::on_new_session(1);

        System::assert_has_event(
            Event::<Test>::EnergyExchangeFailed {
                energy: None,
                static_energy: Some(DispatchError::Unavailable),
                liquid_energy: None,
            }
            .into(),
        );
        let native = (10 + 30) * EXCHANGE_RATE;
        System::assert_last_event(
            Event::<Test>::EnergyExchanged {
                energy: 10,
                static_energy: 0,
                liquid_energy: 30,
                native,
            }
            .into(),
        );
        assert_eq!(Assets::balance(ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Assets::balance(STATIC_ENERGY, treasury), ENERGY_MIN_BALANCE + 20);
        assert_eq!(Assets::balance(LIQUID_ENERGY, treasury), ENERGY_MIN_BALANCE);
        assert_eq!(Balances::free_balance(treasury), native_before + native);
    });
}
