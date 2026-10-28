use super::*;
use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature, TransactionV2};
use fp_self_contained::SelfContainedCall;
use frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::{
        fungible::Mutate as _,
        fungibles::{Inspect as _, Mutate as _},
        Hooks,
    },
};
use pallet_energy_fee::DefaultFeeMultiplier;
use sp_runtime::{BuildStorage, FixedU128, Perquintill};

fn alith() -> AccountId {
    AccountId::from(hex_literal::hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac"))
}

fn new_test_ext() -> sp_io::TestExternalities {
    sp_io::TestExternalities::new(
        RuntimeGenesisConfig {
            balances: BalancesConfig { balances: vec![(alith(), 1_000_000 * vtrs::UNITS)] },
            assets: AssetsConfig {
                assets: vec![
                    (VNRG::get(), alith(), false, 1),
                    (SNRG::get(), alith(), false, 1),
                    (LNRG::get(), alith(), false, 1),
                ],
                accounts: vec![
                    (VNRG::get(), alith(), 100_000_000_000_000_000_000),
                    (SNRG::get(), alith(), 100_000_000_000_000_000_000),
                    (LNRG::get(), alith(), 100_000_000_000_000_000_000),
                ],
                ..Default::default()
            },
            nac_managing: NacManagingConfig { accounts: vec![(alith(), 2)], owners: vec![alith()] },
            ..Default::default()
        }
        .build_storage()
        .unwrap(),
    )
}

fn mock_signature() -> TransactionSignature {
    let r = H256([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x02,
    ]);

    TransactionSignature::new(27, r, r).unwrap()
}

const AMOUNT: Balance = 1_000_000_000;

fn asset_balance(asset: &NativeOrAssetId, who: &AccountId) -> Balance {
    NativeAndAssets::balance(asset.clone(), who)
}

fn asset_issuance(asset: &NativeOrAssetId) -> Balance {
    NativeAndAssets::total_issuance(asset.clone())
}

/// Gives the broker VTRS to pay for energy and sets the rates of its SNRG and LNRG paths.
fn fund_broker() {
    Balances::set_balance(&EnergyBroker::account_id(), 1_000 * vtrs::UNITS);
    pallet_dynamic_energy::ExchangeRate::<Runtime>::put(FixedU128::from_u32(1));
    pallet_asset_rate::ConversionRateToNative::<Runtime>::insert(
        SNRG::get(),
        FixedU128::from_u32(1),
    );
}

/// The VTRS the broker can pay out.
fn broker_vtrs() -> Balance {
    Balances::reducible_balance(
        &EnergyBroker::account_id(),
        Preservation::Preserve,
        Fortitude::Polite,
    )
}

#[test]
fn configured_base_extrinsic_weight_is_evm_compatible() {
    let min_ethereum_transaction_weight = WeightPerGas::get() * 21_000;
    let base_extrinsic = BlockWeights::get().get(DispatchClass::Normal).base_extrinsic;
    assert!(base_extrinsic.ref_time() <= min_ethereum_transaction_weight.ref_time());
}

#[test]
fn fee_multiplier_update_works() {
    new_test_ext().execute_with(|| {
        let max_block_weight =
            BlockWeights::get().per_class.get(DispatchClass::Normal).max_total.unwrap();
        let block_weight_a = max_block_weight / 2;
        System::set_block_consumed_resources(block_weight_a, 0);

        TransactionPayment::on_finalize(1);
        assert_eq!(
            TransactionPayment::next_fee_multiplier(),
            DefaultFeeMultiplier::<Runtime>::get()
        );

        let block_fullness_a = Perquintill::from_percent(50) + Perquintill::from_parts(1000000);
        let upper_fee_multiplier = FixedU128::from_rational(2, 1);
        EnergyFee::update_block_fullness_threshold(RuntimeOrigin::root(), block_fullness_a)
            .expect("Expected to set a new block fullness threshold");
        EnergyFee::update_upper_fee_multiplier(RuntimeOrigin::root(), upper_fee_multiplier)
            .expect("Expected to set a new upper fee multiplier");

        TransactionPayment::on_finalize(1);
        assert_eq!(
            TransactionPayment::next_fee_multiplier(),
            DefaultFeeMultiplier::<Runtime>::get()
        );

        let block_fullness_b = Perquintill::from_percent(50);
        EnergyFee::update_block_fullness_threshold(RuntimeOrigin::root(), block_fullness_b)
            .expect("Expected to set a new block fullness threshold");

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), upper_fee_multiplier);

        let call_with_custom_fee =
            RuntimeCall::Balances(BalancesCall::transfer_keep_alive { dest: alith(), value: 1 });
        let updated_custom_fee =
            upper_fee_multiplier.saturating_mul_int(GetConstantEnergyFee::get());

        assert_eq!(
            EnergyFee::dispatch_info_to_fee(&call_with_custom_fee, None, None),
            CallFee::Regular(updated_custom_fee)
        );

        let block_weight_b = max_block_weight / 3;
        System::set_block_consumed_resources(block_weight_b, 0);

        TransactionPayment::on_finalize(1);
        assert_eq!(
            TransactionPayment::next_fee_multiplier(),
            DefaultFeeMultiplier::<Runtime>::get()
        );

        assert_eq!(
            EnergyFee::dispatch_info_to_fee(&call_with_custom_fee, None, None),
            CallFee::Regular(GetConstantEnergyFee::get())
        );
    });
}

#[test]
fn validate_self_contained_should_disallow_calls_if_sender_cant_pay_fees() {
    new_test_ext().execute_with(|| {
        // A sender without energy buys it with VTRS. Without a rate, or without VNRG for the broker
        // to sell, the fee can't be quoted, and `InvalidTransaction::Payment` would come from that
        // instead of the VTRS balance check.
        let broker = EnergyBroker::account_id();
        // VNRG is not sufficient, so the broker's account must exist before it can hold any.
        Balances::set_balance(&broker, ExistentialDeposit::get());
        NativeAndAssets::set_balance(
            NativeOrAssetId::WithId(VNRG::get()),
            &broker,
            1_000_000_000_000_000,
        );
        pallet_dynamic_energy::ExchangeRate::<Runtime>::put(FixedU128::from_u32(1));

        assert!(
            <EnergyBroker as QuotePrice>::quote_price_tokens_for_exact_tokens(
                NativeOrAssetId::Native,
                NativeOrAssetId::WithId(VNRG::get()),
                EnergyFee::ethereum_fee(),
                true,
            )
            .is_some(),
            "the EVM fee must be quotable in VTRS",
        );

        let sample_tx = TransactionV2::Legacy(LegacyTransaction {
            nonce: Default::default(),
            gas_price: 1.into(),
            gas_limit: 100_000.into(),
            action: TransactionAction::Call(Default::default()),
            value: Default::default(),
            input: Default::default(),
            signature: mock_signature(),
        });

        let ethereum_call = pallet_ethereum::Call::new_call_variant_transact(sample_tx);
        let runtime_call = RuntimeCall::Ethereum(ethereum_call);
        let dispatch_info = runtime_call.get_dispatch_info();
        let len = 0_usize;

        let alith_h160 = H160::from(alith().0);
        let noname_h160 = Default::default();

        assert!(matches!(
            runtime_call.validate_self_contained(&alith_h160, &dispatch_info, len),
            Some(Ok(..))
        ));

        assert_eq!(
            runtime_call.validate_self_contained(&noname_h160, &dispatch_info, len),
            Some(Err(InvalidTransaction::Payment.into()))
        );

        // Short of energy, alith buys the rest of the fee with VTRS, which needs VNRG for the
        // broker to sell. EVM balances are VNRG, so alith keeps half the fee for Frontier's own
        // balance check.
        NativeAndAssets::set_balance(NativeOrAssetId::WithId(SNRG::get()), &alith(), 0);
        NativeAndAssets::set_balance(NativeOrAssetId::WithId(LNRG::get()), &alith(), 0);
        NativeAndAssets::set_balance(
            NativeOrAssetId::WithId(VNRG::get()),
            &alith(),
            EnergyFee::ethereum_fee() / 2,
        );
        assert!(matches!(
            runtime_call.validate_self_contained(&alith_h160, &dispatch_info, len),
            Some(Ok(..))
        ));

        NativeAndAssets::set_balance(NativeOrAssetId::WithId(VNRG::get()), &broker, 0);
        assert_eq!(
            runtime_call.validate_self_contained(&alith_h160, &dispatch_info, len),
            Some(Err(InvalidTransaction::Payment.into()))
        );
    })
}

#[test]
fn energy_fee_exchange_is_limited_by_broker_vnrg() {
    use vitreus_runtime_common::QuotePriceIntoEnergy;
    // The exchange as the config wires it, so the test checks what energy-fee actually uses.
    type EnergyFeeExchange = <Runtime as pallet_energy_fee::Config>::EnergyExchange;

    new_test_ext().execute_with(|| {
        let energy = NativeOrAssetId::WithId(VNRG::get());
        let path = (NativeOrAssetId::Native, energy.clone());
        let broker = EnergyBroker::account_id();

        // VNRG is not sufficient, so the broker's account must exist before it can hold any.
        Balances::set_balance(&broker, ExistentialDeposit::get());
        NativeAndAssets::set_balance(energy.clone(), &broker, vnrg::UNITS);
        assert!(EnergyFeeExchange::max_amount_out() > 0);
        assert_eq!(EnergyFeeExchange::max_amount_out(), EnergyBroker::max_amount_out(&path));

        // Without VNRG of its own, the broker has none to sell.
        NativeAndAssets::set_balance(energy, &broker, 0);
        assert_eq!(EnergyFeeExchange::max_amount_out(), 0);
    })
}

mod energy_broker {
    use super::*;
    use frame_support::{assert_noop, assert_ok};

    fn energy_sale() -> Balance {
        pallet_dynamic_energy::SessionEnergySale::<Runtime>::get()
    }

    fn energy_payout() -> Balance {
        pallet_dynamic_energy::SessionEnergyPayout::<Runtime>::get()
    }

    /// Sets the payout quota of the session to `factor` payout budgets of `budget` VTRS.
    fn set_payout_quota(budget: Balance, factor: u32) {
        pallet_dynamic_energy::SessionPayoutBudget::<Runtime>::put(budget);
        assert_ok!(DynamicEnergy::set_payout_quota_factor(RuntimeOrigin::root(), Some(factor)));
    }

    /// Converting `source` into VNRG must be quoted one-to-one and cost no swap fee.
    fn assert_conversion_is_one_to_one_and_without_swap_fee(source: AssetId) {
        let input = NativeOrAssetId::WithId(source);
        let vnrg = NativeOrAssetId::WithId(VNRG::get());

        assert_eq!(
            EnergyBroker::swap_fee(&(input.clone(), vnrg.clone())),
            0,
            "conversion must be without swap fee",
        );

        assert_eq!(
            <EnergyBroker as QuotePrice>::quote_price_exact_tokens_for_tokens(
                input.clone(),
                vnrg.clone(),
                AMOUNT,
                true,
            ),
            Some(AMOUNT),
            "conversion must be one-to-one",
        );

        assert_eq!(
            <EnergyBroker as QuotePrice>::quote_price_tokens_for_exact_tokens(
                input, vnrg, AMOUNT, true,
            ),
            Some(AMOUNT),
            "conversion must be one-to-one in reverse",
        );
    }

    /// Executing a conversion of `source` into VNRG must burn the input and mint the output
    /// one-to-one, leave the caller holding the result, and leave the broker's own holdings alone.
    fn assert_conversion_burns_and_mints(source: AssetId) {
        let input = NativeOrAssetId::WithId(source);
        let vnrg = NativeOrAssetId::WithId(VNRG::get());
        let broker = EnergyBroker::account_id();

        let vnrg_issuance_before = asset_issuance(&vnrg);
        let input_issuance_before = asset_issuance(&input);
        let alith_vnrg_before = asset_balance(&vnrg, &alith());
        let alith_input_before = asset_balance(&input, &alith());
        let broker_vnrg_before = asset_balance(&vnrg, &broker);
        let broker_input_before = asset_balance(&input, &broker);
        let broker_native_before = Balances::free_balance(broker);
        let sale_before = energy_sale();
        let payout_before = energy_payout();

        assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
            RuntimeOrigin::signed(alith()),
            alith(),
            (input.clone(), vnrg.clone()),
            AMOUNT,
            None,
            false,
        ));

        // The input asset is destroyed; VNRG is created, one-to-one.
        assert_eq!(
            asset_issuance(&input),
            input_issuance_before - AMOUNT,
            "the input asset must be burned",
        );
        assert_eq!(asset_issuance(&vnrg), vnrg_issuance_before + AMOUNT, "VNRG must be minted");

        // And the caller is the one holding the result.
        assert_eq!(asset_balance(&input, &alith()), alith_input_before - AMOUNT);
        assert_eq!(asset_balance(&vnrg, &alith()), alith_vnrg_before + AMOUNT);

        // Minted, not drawn: none of the broker's holdings move.
        assert_eq!(asset_balance(&vnrg, &broker), broker_vnrg_before);
        assert_eq!(asset_balance(&input, &broker), broker_input_before);
        assert_eq!(Balances::free_balance(broker), broker_native_before);

        // Converting between energy assets is not a trade, so it must not feed the rate or count
        // against the payout quota.
        assert_eq!(energy_sale(), sale_before);
        assert_eq!(energy_payout(), payout_before);
    }

    #[test]
    fn snrg_conversion_is_one_to_one_and_without_swap_fee() {
        new_test_ext()
            .execute_with(|| assert_conversion_is_one_to_one_and_without_swap_fee(SNRG::get()))
    }

    #[test]
    fn lnrg_conversion_is_one_to_one_and_without_swap_fee() {
        new_test_ext()
            .execute_with(|| assert_conversion_is_one_to_one_and_without_swap_fee(LNRG::get()))
    }

    #[test]
    fn snrg_conversion_burns_and_mints_without_touching_the_broker() {
        new_test_ext().execute_with(|| assert_conversion_burns_and_mints(SNRG::get()))
    }

    #[test]
    fn lnrg_conversion_burns_and_mints_without_touching_the_broker() {
        new_test_ext().execute_with(|| assert_conversion_burns_and_mints(LNRG::get()))
    }

    #[test]
    fn lnrg_conversion_ignores_payout_quota() {
        new_test_ext().execute_with(|| {
            let lnrg_sale = (NativeOrAssetId::WithId(LNRG::get()), NativeOrAssetId::Native);

            // A quota without a payout budget closes the LNRG → VTRS path, but not the conversion.
            assert_ok!(DynamicEnergy::set_payout_quota_factor(RuntimeOrigin::root(), Some(2)));
            assert_eq!(EnergyBroker::max_amount_out(&lnrg_sale), 0);

            assert_conversion_burns_and_mints(LNRG::get());
        })
    }

    #[test]
    fn lnrg_sale_is_limited_by_payout_quota() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            let lnrg = NativeOrAssetId::WithId(LNRG::get());
            let path = (lnrg.clone(), NativeOrAssetId::Native);
            let broker = EnergyBroker::account_id();

            // A quota of two budgets of 10 VTRS, far below the broker's VTRS.
            fund_broker();
            set_payout_quota(10 * vtrs::UNITS, 2);
            assert_eq!(EnergyBroker::max_amount_out(&path), 20 * vtrs::UNITS);

            // At one energy per VTRS and the 1% swap fee, 25 LNRG would be paid 24.75 VTRS.
            let too_much = 25 * vtrs::UNITS;
            assert_eq!(
                <EnergyBroker as QuotePrice>::quote_price_exact_tokens_for_tokens(
                    lnrg.clone(),
                    NativeOrAssetId::Native,
                    too_much,
                    true,
                ),
                None,
            );
            assert_noop!(
                EnergyBroker::swap_exact_tokens_for_tokens(
                    RuntimeOrigin::signed(alith()),
                    alith(),
                    path.clone(),
                    too_much,
                    None,
                    false,
                ),
                pallet_energy_broker::Error::<Runtime>::InsufficientLiquidity
            );

            // A sale within the quota is paid, and its payout counts against the quota.
            let amount_in = 10 * vtrs::UNITS;
            let exchange_in = amount_in / 100 * 99;
            let payout = exchange_in;
            let lnrg_before = asset_balance(&lnrg, &alith());
            let alith_vtrs_before = Balances::free_balance(alith());
            let broker_vtrs_before = Balances::free_balance(broker);

            assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(alith()),
                alith(),
                path.clone(),
                amount_in,
                None,
                false,
            ));

            assert_eq!(asset_balance(&lnrg, &alith()), lnrg_before - amount_in);
            assert_eq!(Balances::free_balance(alith()), alith_vtrs_before + payout);
            assert_eq!(Balances::free_balance(broker), broker_vtrs_before - payout);
            assert_eq!(energy_sale(), exchange_in);
            assert_eq!(energy_payout(), payout);
            assert_eq!(EnergyBroker::max_amount_out(&path), 20 * vtrs::UNITS - payout);
            System::assert_has_event(
                pallet_energy_broker::Event::<Runtime>::SwapExecuted {
                    who: alith(),
                    recipient: alith(),
                    path,
                    amount_in,
                    amount_out: payout,
                }
                .into(),
            );
        })
    }

    #[test]
    fn lnrg_sale_has_no_payout_quota_without_factor() {
        new_test_ext().execute_with(|| {
            let path = (NativeOrAssetId::WithId(LNRG::get()), NativeOrAssetId::Native);

            // Without a factor only the broker's VTRS limit the sale, whatever the payout budget.
            fund_broker();
            pallet_dynamic_energy::SessionPayoutBudget::<Runtime>::put(10 * vtrs::UNITS);
            assert_eq!(EnergyBroker::max_amount_out(&path), broker_vtrs());
        })
    }

    #[test]
    fn snrg_sale_is_not_limited_by_payout_quota() {
        new_test_ext().execute_with(|| {
            let snrg_path = (NativeOrAssetId::WithId(SNRG::get()), NativeOrAssetId::Native);
            let lnrg_path = (NativeOrAssetId::WithId(LNRG::get()), NativeOrAssetId::Native);

            fund_broker();
            Balances::set_balance(&EnergyBroker::account_id(), 50 * vtrs::UNITS);
            set_payout_quota(10 * vtrs::UNITS, 2);

            // The legacy SNRG buyback is not an energy sale: only the broker's VTRS limit it.
            assert_eq!(EnergyBroker::max_amount_out(&snrg_path), broker_vtrs());
            assert_eq!(EnergyBroker::max_amount_out(&lnrg_path), 20 * vtrs::UNITS);

            // It pays out of the same VTRS, though, so it can leave LNRG sales less than the quota.
            assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(alith()),
                alith(),
                snrg_path,
                broker_vtrs() - 5 * vtrs::UNITS,
                None,
                false,
            ));

            assert_eq!(broker_vtrs(), 5 * vtrs::UNITS);
            assert_eq!(EnergyBroker::max_amount_out(&lnrg_path), 5 * vtrs::UNITS);
            assert_eq!(energy_sale(), 0);
            assert_eq!(energy_payout(), 0);

            // An LNRG sale for more than the broker has left fails, though it is within the quota.
            assert_noop!(
                EnergyBroker::swap_tokens_for_exact_tokens(
                    RuntimeOrigin::signed(alith()),
                    alith(),
                    lnrg_path,
                    6 * vtrs::UNITS,
                    None,
                    false,
                ),
                pallet_energy_broker::Error::<Runtime>::InsufficientLiquidity
            );
        })
    }
}

mod treasury {
    use super::*;
    use frame_support::assert_ok;
    use vitreus_runtime_common::{QuotePriceFromEnergy, SwapFromEnergy};

    // The treasury's exchanges as its config wires them, so the tests check what it actually uses.
    type TreasuryEnergyExchange = <Runtime as pallet_treasury_extension::Config>::EnergyExchange;
    type TreasuryStaticEnergyExchange =
        <Runtime as pallet_treasury_extension::Config>::StaticEnergyExchange;
    type TreasuryLiquidEnergyExchange =
        <Runtime as pallet_treasury_extension::Config>::LiquidEnergyExchange;

    /// A rate at which the broker's conversions don't round.
    const EXACT_RATE: FixedU128 = FixedU128::from_u32(1);
    /// A rate at which the broker's conversions round down.
    const ROUNDING_RATE: FixedU128 = FixedU128::from_rational(3, 2);
    /// A rate at which a base unit of VTRS at the broker pays for no energy on either path: LNRG
    /// rounds the output down to zero, SNRG would pay out more than the unit. At `ROUNDING_RATE`
    /// the SNRG path still swaps one for one.
    const DUST_RATE: FixedU128 = FixedU128::from_rational(7, 3);

    /// Selling `AMOUNT` of `asset` through `Exchange` must take exactly that much, pay VTRS and
    /// leave the seller's other energy as it was.
    fn assert_sells_for_vtrs<Exchange>(asset: AssetId)
    where
        Exchange: SwapFromEnergy<AccountId, Balance = Balance>,
    {
        fund_broker();
        let other_energy = move || {
            [VNRG::get(), SNRG::get(), LNRG::get()]
                .into_iter()
                .filter(|other| *other != asset)
                .map(|other| asset_balance(&NativeOrAssetId::WithId(other), &alith()))
                .collect::<Vec<_>>()
        };
        let other_energy_before = other_energy();
        let asset = NativeOrAssetId::WithId(asset);
        let asset_before = asset_balance(&asset, &alith());
        let native_before = Balances::free_balance(alith());

        let native_out = Exchange::swap_exact_tokens_for_tokens(alith(), AMOUNT, true)
            .expect("the sale must go through");

        assert!(native_out > 0, "the sale must pay VTRS");
        assert_eq!(asset_balance(&asset, &alith()), asset_before - AMOUNT);
        assert_eq!(Balances::free_balance(alith()), native_before + native_out);
        assert_eq!(other_energy(), other_energy_before, "the sale must not touch other energy");
    }

    /// With more `asset_id` than the broker can pay for at `rate`, the treasury's session hook must
    /// sell as much as the broker can pay out, report the sale and keep the rest.
    fn assert_sells_within_broker_vtrs(asset_id: AssetId, rate: FixedU128) {
        let treasury = Treasury::account_id();
        let lnrg = NativeOrAssetId::WithId(LNRG::get());

        System::set_block_number(1);
        fund_broker();
        pallet_dynamic_energy::ExchangeRate::<Runtime>::put(rate);
        pallet_asset_rate::ConversionRateToNative::<Runtime>::insert(SNRG::get(), rate);
        let broker_vtrs_before = broker_vtrs();

        // More energy than the broker can pay for.
        Balances::set_balance(&treasury, ExistentialDeposit::get());
        assert_ok!(Assets::mint_into(asset_id, &treasury, 2 * broker_vtrs_before));
        let asset = NativeOrAssetId::WithId(asset_id);
        let asset_before = asset_balance(&asset, &treasury);
        let native_before = Balances::free_balance(treasury);
        let lnrg_issuance_before = asset_issuance(&lnrg);

        <TreasuryExtension as vitreus_runtime_common::OnSessionChange>::on_new_session(1);

        let sold = asset_before - asset_balance(&asset, &treasury);
        let received = Balances::free_balance(treasury) - native_before;
        assert!(sold > 0 && sold < asset_before, "the treasury must sell some energy, not all");
        // Rounding can leave the broker dust, but must never make it pay more than it can.
        assert!(received > 0 && received <= broker_vtrs_before);
        assert_eq!(broker_vtrs(), broker_vtrs_before - received);
        if rate == EXACT_RATE {
            assert_eq!(received, broker_vtrs_before, "without rounding the broker pays out all");
            assert_eq!(sold, broker_vtrs_before);
        }

        let sold_of = |id| if id == asset_id { sold } else { 0 };
        System::assert_last_event(
            pallet_treasury_extension::Event::<Runtime>::EnergyExchanged {
                energy: sold_of(VNRG::get()),
                static_energy: sold_of(SNRG::get()),
                liquid_energy: sold_of(LNRG::get()),
                native: received,
            }
            .into(),
        );
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(
                pallet_treasury_extension::Event::EnergyExchangeFailed { .. }
            )
        )));

        if asset_id == VNRG::get() {
            // VNRG is sold as LNRG that only passes through the treasury.
            assert_eq!(asset_issuance(&lnrg), lnrg_issuance_before);
        }
    }

    /// With only a base unit of VTRS left at the broker, too little to pay for any energy, the
    /// treasury's session hook must sell no `asset` and report nothing: dust is not a failed sale.
    fn assert_skips_on_broker_vtrs_dust(asset: AssetId) {
        let treasury = Treasury::account_id();

        System::set_block_number(1);
        pallet_dynamic_energy::ExchangeRate::<Runtime>::put(DUST_RATE);
        pallet_asset_rate::ConversionRateToNative::<Runtime>::insert(SNRG::get(), DUST_RATE);
        Balances::set_balance(&EnergyBroker::account_id(), ExistentialDeposit::get() + 1);

        Balances::set_balance(&treasury, ExistentialDeposit::get());
        assert_ok!(Assets::mint_into(asset, &treasury, AMOUNT));
        let asset = NativeOrAssetId::WithId(asset);
        let asset_before = asset_balance(&asset, &treasury);
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as vitreus_runtime_common::OnSessionChange>::on_new_session(1);

        assert!(!System::events()
            .iter()
            .any(|record| matches!(record.event, RuntimeEvent::TreasuryExtension(_))));
        assert_eq!(asset_balance(&asset, &treasury), asset_before);
        assert_eq!(Balances::free_balance(treasury), native_before);
    }

    /// With only a base unit of `asset` above its minimum balance in the treasury, too little to
    /// buy any VTRS, the treasury's session hook must sell nothing and report nothing: dust is not
    /// a failed sale. Energy has such dust on either path when its base unit is worth less than
    /// one of VTRS.
    fn assert_skips_treasury_dust(asset: AssetId) {
        let treasury = Treasury::account_id();

        System::set_block_number(1);
        fund_broker();
        // Energy is worth two thirds of VTRS on both paths: the LNRG rate counts energy per VTRS,
        // the SNRG rate VTRS per energy.
        pallet_dynamic_energy::ExchangeRate::<Runtime>::put(ROUNDING_RATE);
        pallet_asset_rate::ConversionRateToNative::<Runtime>::insert(
            SNRG::get(),
            FixedU128::from_rational(2, 3),
        );

        Balances::set_balance(&treasury, ExistentialDeposit::get());
        let min_balance =
            <Assets as frame_support::traits::fungibles::Inspect<AccountId>>::minimum_balance(
                asset,
            );
        assert_ok!(Assets::mint_into(asset, &treasury, min_balance + 1));
        let asset = NativeOrAssetId::WithId(asset);
        let asset_before = asset_balance(&asset, &treasury);
        let native_before = Balances::free_balance(treasury);

        <TreasuryExtension as vitreus_runtime_common::OnSessionChange>::on_new_session(1);

        assert!(!System::events()
            .iter()
            .any(|record| matches!(record.event, RuntimeEvent::TreasuryExtension(_))));
        assert_eq!(asset_balance(&asset, &treasury), asset_before);
        assert_eq!(Balances::free_balance(treasury), native_before);
    }

    /// When the broker has no VTRS to pay, selling through `Exchange` must fail and leave the
    /// seller's energy and VTRS, and the energy issuance, as they were.
    fn assert_fails_without_broker_vtrs<Exchange>()
    where
        Exchange: SwapFromEnergy<AccountId, Balance = Balance>,
    {
        fund_broker();
        // With only its existential deposit left, the broker has no VTRS to pay out.
        Balances::set_balance(&EnergyBroker::account_id(), ExistentialDeposit::get());

        let energy = || {
            [VNRG::get(), SNRG::get(), LNRG::get()].map(|asset| {
                let asset = NativeOrAssetId::WithId(asset);
                (asset_balance(&asset, &alith()), asset_issuance(&asset))
            })
        };
        let energy_before = energy();
        let native_before = Balances::free_balance(alith());

        Exchange::swap_exact_tokens_for_tokens(alith(), AMOUNT, true)
            .expect_err("the sale must fail without VTRS to pay");

        assert_eq!(energy(), energy_before, "a failed sale must not touch energy");
        assert_eq!(Balances::free_balance(alith()), native_before);
    }

    #[test]
    fn vnrg_exchange_sells_for_vtrs() {
        new_test_ext().execute_with(|| assert_sells_for_vtrs::<TreasuryEnergyExchange>(VNRG::get()))
    }

    #[test]
    fn snrg_exchange_sells_for_vtrs() {
        new_test_ext()
            .execute_with(|| assert_sells_for_vtrs::<TreasuryStaticEnergyExchange>(SNRG::get()))
    }

    #[test]
    fn lnrg_exchange_sells_for_vtrs() {
        new_test_ext()
            .execute_with(|| assert_sells_for_vtrs::<TreasuryLiquidEnergyExchange>(LNRG::get()))
    }

    #[test]
    fn new_session_sells_vnrg_within_broker_vtrs() {
        new_test_ext().execute_with(|| assert_sells_within_broker_vtrs(VNRG::get(), EXACT_RATE))
    }

    #[test]
    fn new_session_sells_snrg_within_broker_vtrs() {
        new_test_ext().execute_with(|| assert_sells_within_broker_vtrs(SNRG::get(), EXACT_RATE))
    }

    #[test]
    fn new_session_sells_lnrg_within_broker_vtrs() {
        new_test_ext().execute_with(|| assert_sells_within_broker_vtrs(LNRG::get(), EXACT_RATE))
    }

    #[test]
    fn new_session_sells_vnrg_within_broker_vtrs_despite_rounding() {
        new_test_ext().execute_with(|| assert_sells_within_broker_vtrs(VNRG::get(), ROUNDING_RATE))
    }

    #[test]
    fn new_session_sells_snrg_within_broker_vtrs_despite_rounding() {
        new_test_ext().execute_with(|| assert_sells_within_broker_vtrs(SNRG::get(), ROUNDING_RATE))
    }

    #[test]
    fn new_session_sells_lnrg_within_broker_vtrs_despite_rounding() {
        new_test_ext().execute_with(|| assert_sells_within_broker_vtrs(LNRG::get(), ROUNDING_RATE))
    }

    #[test]
    fn new_session_skips_vnrg_on_broker_vtrs_dust() {
        new_test_ext().execute_with(|| assert_skips_on_broker_vtrs_dust(VNRG::get()))
    }

    #[test]
    fn new_session_skips_snrg_on_broker_vtrs_dust() {
        new_test_ext().execute_with(|| assert_skips_on_broker_vtrs_dust(SNRG::get()))
    }

    #[test]
    fn new_session_skips_lnrg_on_broker_vtrs_dust() {
        new_test_ext().execute_with(|| assert_skips_on_broker_vtrs_dust(LNRG::get()))
    }

    #[test]
    fn new_session_skips_treasury_vnrg_dust() {
        new_test_ext().execute_with(|| assert_skips_treasury_dust(VNRG::get()))
    }

    #[test]
    fn new_session_skips_treasury_snrg_dust() {
        new_test_ext().execute_with(|| assert_skips_treasury_dust(SNRG::get()))
    }

    #[test]
    fn new_session_skips_treasury_lnrg_dust() {
        new_test_ext().execute_with(|| assert_skips_treasury_dust(LNRG::get()))
    }

    #[test]
    fn lnrg_exchange_is_limited_by_broker_vtrs() {
        new_test_ext().execute_with(|| {
            let path = (NativeOrAssetId::WithId(LNRG::get()), NativeOrAssetId::Native);

            fund_broker();
            assert!(TreasuryLiquidEnergyExchange::max_amount_out() > 0);
            assert_eq!(
                TreasuryLiquidEnergyExchange::max_amount_out(),
                EnergyBroker::max_amount_out(&path),
            );

            // With only its existential deposit left, the broker has no VTRS to pay out.
            Balances::set_balance(&EnergyBroker::account_id(), ExistentialDeposit::get());
            assert_eq!(TreasuryLiquidEnergyExchange::max_amount_out(), 0);
        })
    }

    #[test]
    fn vnrg_exchange_fails_without_broker_vtrs() {
        new_test_ext().execute_with(assert_fails_without_broker_vtrs::<TreasuryEnergyExchange>)
    }

    #[test]
    fn snrg_exchange_fails_without_broker_vtrs() {
        new_test_ext()
            .execute_with(assert_fails_without_broker_vtrs::<TreasuryStaticEnergyExchange>)
    }

    #[test]
    fn lnrg_exchange_fails_without_broker_vtrs() {
        new_test_ext()
            .execute_with(assert_fails_without_broker_vtrs::<TreasuryLiquidEnergyExchange>)
    }
}

mod xcm_tests {
    use super::*;
    use frame_support::{assert_err_ignore_postinfo, assert_noop};
    use sp_runtime::{traits::Dispatchable, DispatchError};
    use xcm::latest::prelude::{
        Asset, AssetId, ClearOrigin, ExecuteXcm, Fungible, Here, Location, OriginKind, Parachain,
        ReceiveTeleportedAsset, Transact, Weight, WeightLimit, Xcm, XcmError,
    };
    use xcm::{VersionedAssets, VersionedLocation, VersionedXcm};
    use xcm_executor::{traits::TransferType, XcmExecutor};

    // A child parachain origin that `EnergyForParachains` trusts as a teleporter.
    fn parachain() -> Location {
        Location::new(0, [Parachain(2000)])
    }

    fn energy_asset(amount: u128) -> Asset {
        Asset { id: AssetId(crate::xcm_config::EnergyTokenLocation::get()), fun: Fungible(amount) }
    }

    /// Inbound energy teleports must be rejected explicitly with `UntrustedTeleportLocation`,
    /// not silently masked as `AssetNotFound`. See `EnergyAdapter::can_check_in` for why energy
    /// is one-way.
    #[test]
    fn inbound_energy_teleport_is_rejected() {
        new_test_ext().execute_with(|| {
            let message =
                Xcm(vec![ReceiveTeleportedAsset(vec![energy_asset(1_000_000_000)].into())]);
            let mut hash = [0u8; 32];

            let outcome = XcmExecutor::<crate::xcm_config::XcmConfig>::prepare_and_execute(
                parachain(),
                message,
                &mut hash,
                Weight::MAX,
                Weight::MAX,
            );

            assert_eq!(outcome.ensure_complete(), Err(XcmError::UntrustedTeleportLocation));
        });
    }

    /// Only governance may `send` arbitrary XCM. Regression guard for `SendXcmOrigin`.
    #[test]
    fn signed_cannot_send_xcm() {
        new_test_ext().execute_with(|| {
            let dest = VersionedLocation::from(Location::new(0, [Parachain(2000)]));
            let message = VersionedXcm::from(Xcm::<()>(vec![ClearOrigin]));
            assert_noop!(
                XcmPallet::send(RuntimeOrigin::signed(alith()), Box::new(dest), Box::new(message)),
                DispatchError::BadOrigin,
            );
        });
    }

    /// `execute` of arbitrary XCM is disabled via `XcmExecuteFilter = Nothing`.
    /// Regression guard for the filter.
    #[test]
    fn signed_cannot_execute_xcm() {
        new_test_ext().execute_with(|| {
            let message = VersionedXcm::from(Xcm::<RuntimeCall>(vec![ClearOrigin]));
            assert_err_ignore_postinfo!(
                XcmPallet::execute(
                    RuntimeOrigin::signed(alith()),
                    Box::new(message),
                    Weight::zero(),
                ),
                pallet_xcm::Error::<Runtime>::Filtered,
            );
        });
    }

    /// `transfer_assets_using_type_and_then` is not gated by `XcmExecuteFilter`, so it is blocked
    /// at `BaseCallFilter`. Regression guard.
    #[test]
    fn signed_cannot_transfer_assets_using_type_and_then() {
        new_test_ext().execute_with(|| {
            let call =
                RuntimeCall::XcmPallet(pallet_xcm::Call::transfer_assets_using_type_and_then {
                    dest: Box::new(VersionedLocation::from(Location::new(0, [Parachain(2000)]))),
                    assets: Box::new(xcm::latest::Assets::new().into()),
                    assets_transfer_type: Box::new(TransferType::Teleport),
                    remote_fees_id: Box::new(AssetId(Here.into_location()).into()),
                    fees_transfer_type: Box::new(TransferType::Teleport),
                    custom_xcm_on_dest: Box::new(VersionedXcm::from(Xcm::<()>(vec![]))),
                    weight_limit: WeightLimit::Unlimited,
                });
            assert_err_ignore_postinfo!(
                call.dispatch(RuntimeOrigin::signed(alith())),
                frame_system::Error::<Runtime>::CallFiltered,
            );
        });
    }

    /// Outbound delivery to child parachains is free: `price_for_delivery` returns no assets, so the
    /// sender pays nothing for delivery. Regression guard for `PriceForChildParachainDelivery`.
    #[test]
    fn child_parachain_delivery_is_free() {
        use polkadot_runtime_common::xcm_sender::PriceForMessageDelivery;
        type Price = crate::xcm_config::PriceForChildParachainDelivery;
        new_test_ext().execute_with(|| {
            let message = Xcm::<()>(vec![ClearOrigin]);
            assert!(Price::price_for_delivery(2000.into(), &message).is_none());
        });
    }

    /// Reserve transfers are disabled via `XcmReserveTransferFilter = Nothing`. Regression guard.
    #[test]
    fn signed_cannot_reserve_transfer() {
        new_test_ext().execute_with(|| {
            let dest = Box::new(VersionedLocation::from(Location::new(0, [Parachain(2000)])));
            let beneficiary =
                Box::new(VersionedLocation::from(Location::new(0, [Parachain(2000)])));
            let assets = Box::new(VersionedAssets::from(xcm::latest::Assets::new()));
            assert_noop!(
                XcmPallet::limited_reserve_transfer_assets(
                    RuntimeOrigin::signed(alith()),
                    dest,
                    beneficiary,
                    assets,
                    0,
                    WeightLimit::Unlimited,
                ),
                pallet_xcm::Error::<Runtime>::Filtered,
            );
        });
    }

    /// Only whitelisted parachain-origin calls (HRMP, registrar) may be dispatched via `Transact`;
    /// anything else is rejected with `NoPermission`. Regression guard for `SafeCallFilter` — the
    /// call used here (a reserve transfer) is deliberately not on the whitelist.
    #[test]
    fn transact_is_rejected() {
        use sp_runtime::codec::Encode;
        new_test_ext().execute_with(|| {
            let call = RuntimeCall::XcmPallet(pallet_xcm::Call::limited_reserve_transfer_assets {
                dest: Box::new(VersionedLocation::from(Location::new(0, [Parachain(2000)]))),
                beneficiary: Box::new(VersionedLocation::from(Location::new(0, [Parachain(2000)]))),
                assets: Box::new(VersionedAssets::from(xcm::latest::Assets::new())),
                fee_asset_item: 0,
                weight_limit: WeightLimit::Unlimited,
            });
            let message = Xcm(vec![Transact {
                origin_kind: OriginKind::SovereignAccount,
                require_weight_at_most: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
                call: call.encode().into(),
            }]);
            let mut hash = [0u8; 32];

            let outcome = XcmExecutor::<crate::xcm_config::XcmConfig>::prepare_and_execute(
                parachain(),
                message,
                &mut hash,
                Weight::MAX,
                Weight::MAX,
            );

            assert_eq!(outcome.ensure_complete(), Err(XcmError::NoPermission));
        });
    }
}

/// Launchpad free-trade wiring: the free-trade fee is the broker's price for
/// one launchpad call, and a `Pays::No` refund lands with a VTRS-only signer
/// as VNRG — so their next call does not touch the broker.
#[cfg(feature = "testnet-runtime")]
mod launchpad_free_trades {
    use super::*;
    use frame_support::{
        dispatch::{Pays, PostDispatchInfo},
        traits::fungibles::{Inspect, Mutate},
    };
    use pallet_transaction_payment::OnChargeTransaction;

    fn one_call_energy() -> Balance {
        <EnergyFee as CustomFee<
            RuntimeCall,
            DispatchInfoOf<RuntimeCall>,
            Balance,
            GetConstantEnergyFee,
        >>::custom_fee()
    }
    fn vnrg(who: &AccountId) -> Balance {
        <Assets as Inspect<AccountId>>::balance(VNRG::get(), who)
    }

    #[test]
    fn free_trade_fee_is_the_broker_price_of_one_call_and_200x_at_the_default_split() {
        new_test_ext().execute_with(|| {
            // No energy rate yet: nothing can be free.
            pallet_dynamic_energy::ExchangeRate::<Runtime>::kill();
            assert_eq!(launchpad::LaunchpadFreeTradeFee::get(), Balance::MAX);

            pallet_dynamic_energy::ExchangeRate::<Runtime>::put(FixedU128::from_u32(1));
            let (vtrs, vnrg) = (NativeOrAssetId::Native, NativeOrAssetId::WithId(VNRG::get()));
            let quote = || {
                <EnergyBroker as QuotePrice>::quote_price_tokens_for_exact_tokens(
                    vtrs.clone(),
                    vnrg.clone(),
                    one_call_energy(),
                    true,
                )
            };
            let with_fee =
                EnergyBroker::get_amount_in(one_call_energy(), &(vtrs.clone(), vnrg.clone()), true)
                    .unwrap();
            let fee = launchpad::LaunchpadFreeTradeFee::get();
            assert_eq!(fee, with_fee, "what a VTRS-only signer pays, broker fee included");

            // The fee is a price only and does not depend on the broker's inventory: this
            // broker holds no VNRG, so `QuotePrice` refuses the quote, and the fee is unchanged.
            assert_eq!(quote(), None);
            // With the VNRG to sell, `QuotePrice` gives the same price. The broker keeps VNRG's
            // minimum balance back, so it needs more than the one call it sells.
            let broker = EnergyBroker::account_id();
            Balances::set_balance(&broker, ExistentialDeposit::get());
            NativeAndAssets::set_balance(vnrg.clone(), &broker, 2 * one_call_energy());
            assert_eq!(quote(), Some(fee));
            assert_eq!(launchpad::LaunchpadFreeTradeFee::get(), fee);

            let p = launchpad::LaunchpadDefaultParams::get();
            let curve = pallet_launchpad::CurveParams {
                graduation_target: p.graduation_target,
                virtual_quote: p.graduation_target / 3,
                curve_fee_bps: p.curve_fee_bps,
                protocol_share_bps: p.protocol_share_bps,
                treasury_share_bps: p.treasury_share_bps,
                pool_fee_tier: p.pool_fee_tier,
            };
            // 100 bps, 25 % protocol + 25 % treasury → 200×.
            assert_eq!(
                pallet_launchpad::Pallet::<Runtime>::free_trade_threshold(&curve),
                Some(fee * 200)
            );
        });
    }

    #[test]
    fn a_pays_no_refund_is_kept_by_a_vtrs_only_signer() {
        new_test_ext().execute_with(|| {
            pallet_dynamic_energy::ExchangeRate::<Runtime>::put(FixedU128::from_u32(1));
            let broker = EnergyBroker::account_id();
            let _ = Balances::deposit_creating(&broker, 1_000 * vtrs::UNITS);
            <Assets as Mutate<AccountId>>::mint_into(
                VNRG::get(),
                &broker,
                1_000 * one_call_energy(),
            )
            .unwrap();
            let bob = AccountId::from([7u8; 20]);
            let _ = Balances::deposit_creating(&bob, 1_000 * vtrs::UNITS);

            let call = RuntimeCall::Launchpad(pallet_launchpad::Call::buy {
                launch_id: 0,
                quote_in: 1,
                min_tokens_out: 0,
            });
            let info = call.get_dispatch_info();
            let fee = match EnergyFee::dispatch_info_to_fee(&call, Some(&info), None) {
                CallFee::Regular(f) => f,
                CallFee::EVM(_) => unreachable!("launchpad calls pay the custom fee"),
            };
            assert_eq!(fee, one_call_energy());
            let free = PostDispatchInfo { actual_weight: None, pays_fee: Pays::No };
            let paid = PostDispatchInfo { actual_weight: None, pays_fee: Pays::Yes };

            // 1st call: bob holds no energy, so the fee is bought from the broker;
            // the Pays::No refund comes back to bob as VNRG.
            let broker0 = vnrg(&broker);
            let vtrs0 = Balances::free_balance(bob);
            let liq = <EnergyFee as OnChargeTransaction<Runtime>>::withdraw_fee(
                &bob, &call, &info, fee, 0,
            )
            .unwrap();
            assert_eq!(vnrg(&broker), broker0 - fee, "one broker purchase");
            assert!(Balances::free_balance(bob) < vtrs0, "paid in VTRS");
            <EnergyFee as OnChargeTransaction<Runtime>>::correct_and_deposit_fee(
                &bob, &info, &free, fee, 0, liq,
            )
            .unwrap();
            assert_eq!(vnrg(&bob), fee, "the refund is bob's to keep");

            // 2nd call, also free: paid from bob's VNRG, the broker is untouched.
            let vtrs1 = Balances::free_balance(bob);
            let liq = <EnergyFee as OnChargeTransaction<Runtime>>::withdraw_fee(
                &bob, &call, &info, fee, 0,
            )
            .unwrap();
            assert_eq!(vnrg(&broker), broker0 - fee);
            assert_eq!(Balances::free_balance(bob), vtrs1);
            <EnergyFee as OnChargeTransaction<Runtime>>::correct_and_deposit_fee(
                &bob, &info, &free, fee, 0, liq,
            )
            .unwrap();
            assert_eq!(vnrg(&bob), fee);

            // A call that pays (a failed or below-threshold trade) spends it.
            let liq = <EnergyFee as OnChargeTransaction<Runtime>>::withdraw_fee(
                &bob, &call, &info, fee, 0,
            )
            .unwrap();
            <EnergyFee as OnChargeTransaction<Runtime>>::correct_and_deposit_fee(
                &bob, &info, &paid, fee, 0, liq,
            )
            .unwrap();
            assert_eq!(vnrg(&bob), 0);
            assert_eq!(vnrg(&broker), broker0 - fee, "still one broker purchase in total");
        });
    }
}
