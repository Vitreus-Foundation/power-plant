use super::*;
use ethereum::{TransactionAction, TransactionSignature, TransactionV2};
use fp_self_contained::SelfContainedCall;
use frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::{fungible::Mutate as _, fungibles::Mutate as _, Hooks},
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

mod energy_broker {
    use super::*;
    use frame_support::{assert_ok, traits::fungibles::Inspect as FungiblesInspect};
    use vitreus_runtime_common::SwapEnergyForNative;

    const AMOUNT: Balance = 1_000_000_000;

    // The treasury's exchanges as its config wires them, so the tests check what it actually uses.
    type TreasuryStaticEnergyExchange =
        <Runtime as pallet_treasury_extension::Config>::StaticEnergyExchange;
    type TreasuryLiquidEnergyExchange =
        <Runtime as pallet_treasury_extension::Config>::LiquidEnergyExchange;

    fn asset_balance(asset: &NativeOrAssetId, who: &AccountId) -> Balance {
        NativeAndAssets::balance(asset.clone(), who)
    }

    fn asset_issuance(asset: &NativeOrAssetId) -> Balance {
        NativeAndAssets::total_issuance(asset.clone())
    }

    fn energy_sale() -> Balance {
        pallet_dynamic_energy::SessionEnergySale::<Runtime>::get()
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

        // Converting between energy assets is not a trade, so it must not feed the rate.
        assert_eq!(energy_sale(), sale_before);
    }

    /// Selling `AMOUNT` of `asset` through `Exchange` must take exactly that much and pay VTRS.
    fn assert_sells_for_vtrs<Exchange>(asset: AssetId)
    where
        Exchange: SwapEnergyForNative<AccountId, Balance = Balance>,
    {
        fund_broker();
        let asset = NativeOrAssetId::WithId(asset);
        let asset_before = asset_balance(&asset, &alith());
        let native_before = Balances::free_balance(alith());

        let native_out = Exchange::swap_exact_tokens_for_tokens(alith(), AMOUNT, true)
            .expect("the sale must go through");

        assert!(native_out > 0, "the sale must pay VTRS");
        assert_eq!(asset_balance(&asset, &alith()), asset_before - AMOUNT);
        assert_eq!(Balances::free_balance(alith()), native_before + native_out);
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
    fn treasury_sells_snrg_for_vtrs() {
        new_test_ext()
            .execute_with(|| assert_sells_for_vtrs::<TreasuryStaticEnergyExchange>(SNRG::get()))
    }

    #[test]
    fn treasury_sells_lnrg_for_vtrs() {
        new_test_ext()
            .execute_with(|| assert_sells_for_vtrs::<TreasuryLiquidEnergyExchange>(LNRG::get()))
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
