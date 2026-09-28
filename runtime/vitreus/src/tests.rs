use super::*;
use ethereum::{TransactionAction, TransactionSignature, TransactionV2};
use fp_self_contained::SelfContainedCall;
use frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::Hooks,
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
        // A sender without energy buys it with VTRS. Without a rate the fee can't be quoted at all,
        // and `InvalidTransaction::Payment` would come from that instead of the VTRS balance check.
        pallet_dynamic_energy::ExchangeRate::<Runtime>::put(FixedU128::from_u32(1));
        assert!(
            <EnergyBroker as QuotePrice>::quote_price_tokens_for_exact_tokens(
                NativeOrAssetId::Native,
                NativeOrAssetId::WithId(VNRG::get()),
                1,
                true,
            )
            .is_some(),
            "VTRS -> VNRG must be quotable",
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
    })
}

mod energy_broker {
    use super::*;
    use frame_support::{assert_ok, traits::fungibles::Inspect as FungiblesInspect};

    const AMOUNT: Balance = 1_000_000_000;

    fn asset_balance(asset: &NativeOrAssetId, who: &AccountId) -> Balance {
        NativeAndAssets::balance(asset.clone(), who)
    }

    fn asset_issuance(asset: &NativeOrAssetId) -> Balance {
        NativeAndAssets::total_issuance(asset.clone())
    }

    fn energy_sale() -> Balance {
        pallet_dynamic_energy::SessionEnergySale::<Runtime>::get()
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
        let broker_native_before = Balances::free_balance(&broker);
        let sale_before = energy_sale();

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
        assert_eq!(Balances::free_balance(&broker), broker_native_before);

        // Converting between energy assets is not a trade, so it must not feed the rate.
        assert_eq!(energy_sale(), sale_before);
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
}
