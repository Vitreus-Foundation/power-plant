use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};

/// The payout budget of a mock session at `BUDGET_STAKE`, with the default APR and multiplier.
const PAYOUT_BUDGET: u128 = 1_000_000;
/// A 600 s mock session at 10% APR earns 1/525_960 of the stake: 31_557_600 s / 600 s * 10.
const BUDGET_STAKE: u128 = 525_960 * PAYOUT_BUDGET;

#[test]
fn setting_zero_annual_percentage_rate_fails() {
    new_test_ext().execute_with(|| {
        // zero APR makes the denominator of the exchange rate zero
        assert_noop!(
            Pallet::<Test>::set_annual_percentage_rate(RuntimeOrigin::root(), 0),
            Error::<Test>::InvalidAnnualPercentageRate
        );
    });
}

#[test]
fn setting_multiplier_coefficients_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        // 0.02*x + 1, which is 2 at the mock warehouse fill of 50%
        let coefficients = [
            FixedI128::zero(),
            FixedI128::zero(),
            FixedI128::from_rational(2, 100),
            FixedI128::one(),
        ];
        let [a, b, c, d] = coefficients;

        assert_ok!(Pallet::<Test>::set_multiplier_coefficients(RuntimeOrigin::root(), a, b, c, d));
        assert_eq!(MultiplierCoefficients::<Test>::get(), coefficients);
        System::assert_last_event(
            Event::<Test>::MultiplierCoefficientsUpdated { coefficients }.into(),
        );

        assert_eq!(
            Pallet::<Test>::calculate_warehouse_capacity_multiplier(),
            FixedU128::from_u32(2)
        );
    });
}

#[test]
fn setting_overflowing_multiplier_coefficients_fails() {
    new_test_ext().execute_with(|| {
        let zero = FixedI128::zero();

        // a*x^3 fits at the mock warehouse fill of 50%, but overflows at the full warehouse
        let a = FixedI128::saturating_from_integer(1_000_000_000_000_000u64);
        assert_noop!(
            Pallet::<Test>::set_multiplier_coefficients(RuntimeOrigin::root(), a, zero, zero, zero),
            Error::<Test>::InvalidMultiplierCoefficients
        );

        // a*x^3 and b*x^2 fit at the full warehouse, but their sum overflows
        let a = FixedI128::saturating_from_integer(100_000_000_000_000u64);
        let b = FixedI128::saturating_from_integer(10_000_000_000_000_000u64);
        assert_noop!(
            Pallet::<Test>::set_multiplier_coefficients(RuntimeOrigin::root(), a, b, zero, zero),
            Error::<Test>::InvalidMultiplierCoefficients
        );
    });
}

#[test]
fn setting_payout_quota_factor_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(Pallet::<Test>::set_payout_quota_factor(RuntimeOrigin::root(), Some(12)));
        assert_eq!(PayoutQuotaFactor::<Test>::get(), Some(12));
        System::assert_last_event(Event::<Test>::PayoutQuotaFactorSet { value: Some(12) }.into());

        assert_ok!(Pallet::<Test>::set_payout_quota_factor(RuntimeOrigin::root(), None));
        assert_eq!(PayoutQuotaFactor::<Test>::get(), None);
        System::assert_last_event(Event::<Test>::PayoutQuotaFactorSet { value: None }.into());

        assert_noop!(
            Pallet::<Test>::set_payout_quota_factor(RuntimeOrigin::signed(1), Some(12)),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn setting_zero_payout_quota_factor_fails() {
    new_test_ext().execute_with(|| {
        // a quota of zero budgets would stop all energy sales
        assert_noop!(
            Pallet::<Test>::set_payout_quota_factor(RuntimeOrigin::root(), Some(0)),
            Error::<Test>::InvalidPayoutQuotaFactor
        );
    });
}

#[test]
fn calculating_polynomial_works() {
    // on-chain coefficients: -0.000001*x^3 + 0.000147*x^2 - 0.009475*x + 1.228239
    let coefficients = [
        FixedI128::from_inner(-1_000_000_000_000),
        FixedI128::from_inner(147_000_000_000_000),
        FixedI128::from_inner(-9_475_000_000_000_000),
        FixedI128::from_inner(1_228_239_000_000_000_000),
    ];

    assert_eq!(
        Pallet::<Test>::checked_polynomial(coefficients, FixedI128::from_u32(0)),
        Some(FixedI128::from_rational(1_228_239, 1_000_000))
    );
    assert_eq!(
        Pallet::<Test>::checked_polynomial(coefficients, FixedI128::from_u32(50)),
        Some(FixedI128::from_rational(996_989, 1_000_000))
    );
    assert_eq!(
        Pallet::<Test>::checked_polynomial(coefficients, FixedI128::from_u32(100)),
        Some(FixedI128::from_rational(750_739, 1_000_000))
    );
}

#[test]
fn calculating_overflowing_multiplier_works() {
    new_test_ext().execute_with(|| {
        let zero = FixedI128::zero();

        // a*x^3 overflows at the mock warehouse fill of 50%
        MultiplierCoefficients::<Test>::put([FixedI128::from_inner(i128::MAX), zero, zero, zero]);
        assert_eq!(Pallet::<Test>::calculate_warehouse_capacity_multiplier(), FixedU128::one());

        // a*x^3 and b*x^2 fit at the mock warehouse fill of 50%, but their sum overflows
        let a = FixedI128::saturating_from_integer(800_000_000_000_000u64);
        let b = FixedI128::saturating_from_integer(40_000_000_000_000_000u64);
        MultiplierCoefficients::<Test>::put([a, b, zero, zero]);
        assert_eq!(Pallet::<Test>::calculate_warehouse_capacity_multiplier(), FixedU128::one());

        // the session change goes through the same calculation
        start_session(1);
        assert_eq!(Pallet::<Test>::exchange_rate_parameters().multiplier, FixedU128::one());
    });
}

#[test]
fn energy_generation_works() {
    new_test_ext().execute_with(|| {
        start_session(0);
        assert!(EnergyGeneration::<Test>::contains_key(0));

        start_session(1);
        assert!(EnergyGeneration::<Test>::contains_key(1));

        start_session(2);
        assert!(EnergyGeneration::<Test>::contains_key(2));

        let total_generation = EnergyGeneration::<Test>::get(0)
            + EnergyGeneration::<Test>::get(1)
            + EnergyGeneration::<Test>::get(2);

        assert!(total_generation > 0);
        assert_eq!(Pallet::<Test>::calculate(0), Some(total_generation));

        // era 0 data removed once era 1 starts
        start_session(3);
        start_session(4);
        assert!(!EnergyGeneration::<Test>::contains_key(0));
        assert!(!EnergyGeneration::<Test>::contains_key(1));
        assert!(!EnergyGeneration::<Test>::contains_key(2));
        assert!(EnergyGeneration::<Test>::contains_key(3));
        assert!(EnergyGeneration::<Test>::contains_key(4));
    });
}

#[test]
fn calculating_energy_generation_works() {
    new_test_ext().execute_with(|| {
        // eras of 3 sessions; staking calculates an era before the next one starts, and every
        // session generates the energy burn override from genesis
        for era in 0..3 {
            for index in 3 * era..3 * (era + 1) {
                start_session(index);

                // the rate is smoothed with the previous session, which pruning at an era start
                // must not remove first
                let old_rate = (index > 0).then_some(ENERGY_BURN);
                assert_eq!(Pallet::<Test>::generation_rate_parameters().old_rate, old_rate);
            }

            assert_eq!(Pallet::<Test>::calculate(era), Some(3 * ENERGY_BURN));
        }
    });
}

#[test]
fn calculating_long_era_energy_generation_works() {
    new_test_ext().execute_with(|| {
        // era 0 lasts 7 sessions instead of 3, as when elections fail
        EraLength::set(7);

        for index in 0..7 {
            start_session(index);
        }

        // every session generates the energy burn override from genesis
        assert_eq!(Pallet::<Test>::calculate(0), Some(7 * ENERGY_BURN));

        // era 0 data removed
        for index in 7..11 {
            start_session(index);
        }
        assert!((0..7).all(|index| !EnergyGeneration::<Test>::contains_key(index)));
        assert!((7..11).all(EnergyGeneration::<Test>::contains_key));
    });
}

#[test]
fn calculating_exchange_rate_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        TotalStakeOverride::<Test>::put(BUDGET_STAKE);

        // the rate is the energy sold per native unit the stake earns over the session
        start_session(1);
        let rate = FixedU128::from_rational(ENERGY_SALE, PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));
        System::assert_last_event(Event::<Test>::ExchangeRateUpdated { rate }.into());

        // the warehouse capacity multiplier doubles the earnings: 0.02*x + 1 is 2 at the mock
        // warehouse fill of 50%
        let zero = FixedI128::zero();
        let c = FixedI128::from_rational(2, 100);
        MultiplierCoefficients::<Test>::put([zero, zero, c, FixedI128::one()]);

        start_session(2);
        let rate = FixedU128::from_rational(ENERGY_SALE, 2 * PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));

        // so does an APR of 20%
        AnnualPercentageRate::<Test>::put(200);

        start_session(3);
        let rate = FixedU128::from_rational(ENERGY_SALE, 4 * PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));

        // and a session that lasts twice as long as expected, as session 4 is skipped
        start_session(5);
        let rate = FixedU128::from_rational(ENERGY_SALE, 8 * PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));
        System::assert_last_event(Event::<Test>::ExchangeRateUpdated { rate }.into());
    });
}

#[test]
fn calculating_smoothed_exchange_rate_works() {
    new_test_ext().execute_with(|| {
        TotalStakeOverride::<Test>::put(BUDGET_STAKE);

        start_session(1);
        let old_rate = FixedU128::from_rational(ENERGY_SALE, PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(old_rate));

        // a smooth factor of 3 gives the new value a weight of 2/(3+1), a half; twice the sale
        // gives a new value of twice the old rate
        ExchangeRateSmoothFactor::<Test>::put(3);
        EnergySaleOverride::<Test>::put(2 * ENERGY_SALE);

        start_session(2);
        let new_rate = FixedU128::from_rational(2 * ENERGY_SALE, PAYOUT_BUDGET);
        let rate = FixedU128::from_rational(3 * ENERGY_SALE, 2 * PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));

        let params = Pallet::<Test>::exchange_rate_parameters();
        assert_eq!(params.new_rate, Some(new_rate));
        assert_eq!(params.old_rate, Some(old_rate));
        assert_eq!(params.smooth_factor, 3);
    });
}

#[test]
fn calculating_exchange_rate_without_stake_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        TotalStakeOverride::<Test>::put(BUDGET_STAKE);
        start_session(1);
        let rate = FixedU128::from_rational(ENERGY_SALE, PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));

        // with no stake there is nothing earned to divide by, so the rate stays as it was
        TotalStakeOverride::<Test>::put(0);
        System::reset_events();

        start_session(2);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));
        assert_eq!(Pallet::<Test>::exchange_rate_parameters().new_rate, None);
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::DynamicEnergy(Event::ExchangeRateUpdated { .. })
        )));
    });
}

#[test]
fn calculating_zero_exchange_rate_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        let unit = 10u128.pow(18);
        EnergySaleOverride::<Test>::kill();
        TotalStakeOverride::<Test>::put(100_000_000 * unit);

        Pallet::<Test>::on_energy_sell(10_000 * unit, 0);
        start_session(1);
        let rate = Pallet::<Test>::exchange_rate().unwrap();
        assert!(!rate.is_zero());

        // at this stake, a sale of 1 gives a rate that rounds down to zero, which must not
        // replace the previous one
        Pallet::<Test>::on_energy_sell(1, 0);
        start_session(2);
        assert_eq!(Pallet::<Test>::exchange_rate(), Some(rate));
        System::assert_last_event(Event::<Test>::ExchangeRateUpdated { rate }.into());
    });
}

#[test]
fn calculating_payout_budget_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        TotalStakeOverride::<Test>::put(BUDGET_STAKE);

        start_session(1);
        assert_eq!(SessionPayoutBudget::<Test>::get(), Some(PAYOUT_BUDGET));

        // 0.02*x + 1, which is 2 at the mock warehouse fill of 50%
        let zero = FixedI128::zero();
        let c = FixedI128::from_rational(2, 100);
        MultiplierCoefficients::<Test>::put([zero, zero, c, FixedI128::one()]);

        start_session(2);
        assert_eq!(SessionPayoutBudget::<Test>::get(), Some(2 * PAYOUT_BUDGET));

        let unavailable: RuntimeEvent = Event::<Test>::SessionPayoutBudgetUnavailable.into();
        assert!(System::events().iter().all(|record| record.event != unavailable));
    });
}

#[test]
fn calculating_zero_payout_budget_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        TotalStakeOverride::<Test>::put(BUDGET_STAKE);
        start_session(1);
        assert_eq!(SessionPayoutBudget::<Test>::get(), Some(PAYOUT_BUDGET));

        // with no stake there is no payout budget: it is reported, and the old one is not kept
        TotalStakeOverride::<Test>::put(0);
        start_session(2);
        assert_eq!(SessionPayoutBudget::<Test>::get(), None);
        System::assert_last_event(Event::<Test>::SessionPayoutBudgetUnavailable.into());
    });
}

#[test]
fn calculating_sold_energy_works() {
    new_test_ext().execute_with(|| {
        start_session(0);
        assert_eq!(SessionEnergySale::<Test>::get(), 0);

        Pallet::<Test>::on_energy_sell(500, 0);
        Pallet::<Test>::on_energy_sell(1000, 0);
        assert_eq!(SessionEnergySale::<Test>::get(), 1500);

        start_session(1);
        assert_eq!(SessionEnergySale::<Test>::get(), 0);
    });
}

#[test]
fn calculating_energy_payout_works() {
    new_test_ext().execute_with(|| {
        start_session(0);
        assert_eq!(SessionEnergyPayout::<Test>::get(), 0);

        Pallet::<Test>::on_energy_sell(500, 50);
        Pallet::<Test>::on_energy_sell(1000, 100);
        assert_eq!(SessionEnergySale::<Test>::get(), 1500);
        assert_eq!(SessionEnergyPayout::<Test>::get(), 150);

        start_session(1);
        assert_eq!(SessionEnergyPayout::<Test>::get(), 0);
    });
}

#[test]
fn calculating_remaining_payout_quota_works() {
    new_test_ext().execute_with(|| {
        TotalStakeOverride::<Test>::put(BUDGET_STAKE);
        start_session(1);

        // no quota without a factor
        assert_eq!(Pallet::<Test>::remaining_payout_quota(), None);

        // two budgets, less what the energy sold during the session was paid
        PayoutQuotaFactor::<Test>::put(2);
        assert_eq!(Pallet::<Test>::remaining_payout_quota(), Some(2 * PAYOUT_BUDGET));

        let paid = PAYOUT_BUDGET / 2;
        Pallet::<Test>::on_energy_sell(500, paid);
        assert_eq!(Pallet::<Test>::remaining_payout_quota(), Some(2 * PAYOUT_BUDGET - paid));

        // a new factor applies at once
        PayoutQuotaFactor::<Test>::put(3);
        assert_eq!(Pallet::<Test>::remaining_payout_quota(), Some(3 * PAYOUT_BUDGET - paid));

        // a payout above the quota leaves nothing
        Pallet::<Test>::on_energy_sell(500, 3 * PAYOUT_BUDGET);
        assert_eq!(Pallet::<Test>::remaining_payout_quota(), Some(0));

        // a new session opens a new quota
        start_session(2);
        assert_eq!(Pallet::<Test>::remaining_payout_quota(), Some(3 * PAYOUT_BUDGET));
    });
}

#[test]
fn calculating_remaining_payout_quota_without_budget_works() {
    new_test_ext().execute_with(|| {
        PayoutQuotaFactor::<Test>::put(2);

        // with no stake there is no payout budget, so nothing can be paid out
        TotalStakeOverride::<Test>::put(0);
        start_session(1);

        assert_eq!(SessionPayoutBudget::<Test>::get(), None);
        assert_eq!(Pallet::<Test>::remaining_payout_quota(), Some(0));
    });
}

#[test]
fn calculating_burned_energy_works() {
    new_test_ext().execute_with(|| {
        start_session(0);
        assert_eq!(SessionEnergyBurn::<Test>::get(), 0);

        Pallet::<Test>::on_energy_burn(500);
        Pallet::<Test>::on_energy_burn(1000);
        assert_eq!(SessionEnergyBurn::<Test>::get(), 1500);

        start_session(1);
        assert_eq!(SessionEnergyBurn::<Test>::get(), 0);
    });
}
