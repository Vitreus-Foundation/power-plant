use super::*;
use frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::{
        fungible::Mutate as _,
        fungibles::{Inspect as _, Mutate as _},
    },
};
use sp_runtime::{BuildStorage, FixedU128};

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
    set_broker_rates(FixedU128::from_u32(1));
}

/// Sets the energy exchange rate and the SNRG rate to `rate`.
fn set_broker_rates(rate: FixedU128) {
    pallet_dynamic_energy::ExchangeRate::<Runtime>::put(rate);
    pallet_asset_rate::ConversionRateToNative::<Runtime>::insert(SNRG::get(), rate);
}

/// The VTRS the broker can pay out.
fn broker_vtrs() -> Balance {
    Balances::reducible_balance(
        &EnergyBroker::account_id(),
        Preservation::Preserve,
        Fortitude::Polite,
    )
}

fn energy_sale() -> Balance {
    pallet_dynamic_energy::SessionEnergySale::<Runtime>::get()
}

fn energy_payout() -> Balance {
    pallet_dynamic_energy::SessionEnergyPayout::<Runtime>::get()
}

/// Sets the payout quota of the session to `quota` VTRS, as a session change does with a factor.
fn set_payout_quota(quota: Balance) {
    pallet_dynamic_energy::SessionPayoutQuota::<Runtime>::put(quota);
}

#[test]
fn configured_base_extrinsic_weight_is_evm_compatible() {
    let min_ethereum_transaction_weight = WeightPerGas::get() * 21_000;
    let base_extrinsic = BlockWeights::get().get(DispatchClass::Normal).base_extrinsic;
    assert!(base_extrinsic.ref_time() <= min_ethereum_transaction_weight.ref_time());
}

mod energy_fee {
    use super::*;
    use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature, TransactionV2};
    use fp_self_contained::SelfContainedCall;
    use frame_support::{dispatch::PostDispatchInfo, traits::Hooks};
    use pallet_evm::OnChargeEVMTransaction;
    use pallet_transaction_payment::OnChargeTransaction;
    use sp_runtime::Perquintill;
    use vitreus_runtime_common::QuotePriceIntoEnergy;

    // The fee types as the config wires them, so the tests check what the runtime actually uses.
    type EnergyFeeCustomFee = <Runtime as pallet_energy_fee::Config>::CustomFee;
    type EnergyFeeExchange = <Runtime as pallet_energy_fee::Config>::EnergyExchange;
    type TransactionFeeHandler =
        <Runtime as pallet_transaction_payment::Config>::OnChargeTransaction;
    type EvmFeeHandler = <Runtime as pallet_evm::Config>::OnChargeTransaction;

    fn mock_signature() -> TransactionSignature {
        let r = H256::from_low_u64_be(2);
        TransactionSignature::new(27, r, r).unwrap()
    }

    /// Lets the broker sell `vnrg` base units of VNRG for VTRS at a rate of one.
    fn fund_broker_with_vnrg(vnrg: Balance) {
        let broker = EnergyBroker::account_id();
        // VNRG is not sufficient, so the broker's account must exist before it can hold any.
        Balances::set_balance(&broker, ExistentialDeposit::get());
        NativeAndAssets::set_balance(NativeOrAssetId::WithId(VNRG::get()), &broker, vnrg);
        pallet_dynamic_energy::ExchangeRate::<Runtime>::put(FixedU128::from_u32(1));
    }

    #[test]
    fn block_at_fullness_threshold_raises_flat_fee() {
        new_test_ext().execute_with(|| {
            let upper_fee_multiplier = FixedU128::from_u32(2);
            EnergyFee::update_upper_fee_multiplier(RuntimeOrigin::root(), upper_fee_multiplier)
                .expect("Expected to set a new upper fee multiplier");
            EnergyFee::update_block_fullness_threshold(
                RuntimeOrigin::root(),
                Perquintill::from_percent(50),
            )
            .expect("Expected to set a new block fullness threshold");
            let max_block_weight =
                BlockWeights::get().per_class.get(DispatchClass::Normal).max_total.unwrap();
            System::set_block_consumed_resources(max_block_weight / 2, 0);

            TransactionPayment::on_finalize(1);
            assert_eq!(TransactionPayment::next_fee_multiplier(), upper_fee_multiplier);

            let transfer = RuntimeCall::Balances(BalancesCall::transfer_keep_alive {
                dest: alith(),
                value: 1,
            });
            assert_eq!(
                EnergyFeeCustomFee::dispatch_info_to_fee(&transfer, None, None),
                CallFee::Regular(
                    upper_fee_multiplier.saturating_mul_int(GetConstantEnergyFee::get())
                )
            );
        });
    }

    #[test]
    fn ethereum_transaction_is_invalid_if_sender_cannot_pay() {
        new_test_ext().execute_with(|| {
            // A sender without energy buys it with VTRS. Without a rate, or without VNRG for the
            // broker to sell, the fee can't be quoted, and `InvalidTransaction::Payment` would come
            // from that instead of the VTRS balance check.
            let broker = EnergyBroker::account_id();
            fund_broker_with_vnrg(1_000_000_000_000_000);

            assert!(
                EnergyFeeExchange::quote_price_tokens_for_exact_tokens(
                    EnergyFeeCustomFee::ethereum_fee(),
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
                EnergyFeeCustomFee::ethereum_fee() / 2,
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
        new_test_ext().execute_with(|| {
            let energy = NativeOrAssetId::WithId(VNRG::get());
            let path = (NativeOrAssetId::Native, energy.clone());
            let broker = EnergyBroker::account_id();

            fund_broker_with_vnrg(vnrg::UNITS);
            assert!(EnergyFeeExchange::max_amount_out() > 0);
            assert_eq!(EnergyFeeExchange::max_amount_out(), EnergyBroker::max_amount_out(&path));

            // Without VNRG of its own, the broker has none to sell.
            NativeAndAssets::set_balance(energy, &broker, 0);
            assert_eq!(EnergyFeeExchange::max_amount_out(), 0);
        })
    }

    #[test]
    fn fee_check_buys_nothing_from_broker() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            let energy = NativeOrAssetId::WithId(VNRG::get());
            let broker = EnergyBroker::account_id();
            let fee = GetConstantEnergyFee::get();
            // BOB holds VTRS only, so the broker has to sell the fee to BOB.
            let bob = AccountId::from([7u8; 20]);
            Balances::set_balance(&bob, 1_000 * vtrs::UNITS);

            let call = RuntimeCall::Balances(BalancesCall::transfer_keep_alive {
                dest: alith(),
                value: 1,
            });
            let info = call.get_dispatch_info();
            let computed_fee = TransactionPayment::compute_fee(0, &info, 0);
            let check =
                || TransactionFeeHandler::can_withdraw_fee(&bob, &call, &info, computed_fee, 0);

            // Without VNRG to sell, the check fails, so it does go to the broker.
            fund_broker_with_vnrg(0);
            assert_eq!(check(), Err(InvalidTransaction::Payment.into()));

            NativeAndAssets::set_balance(energy.clone(), &broker, 10 * fee);
            System::reset_events();
            assert_eq!(check(), Ok(()));

            assert_eq!(Balances::free_balance(bob), 1_000 * vtrs::UNITS);
            assert_eq!(asset_balance(&energy, &bob), 0);
            assert_eq!(asset_balance(&energy, &broker), 10 * fee);
            assert!(System::events().is_empty());
        })
    }

    #[test]
    fn sudo_call_pays_weight_fee() {
        new_test_ext().execute_with(|| {
            // The call inside pays the flat fee, so a `sudo` charged like its inner call would too.
            let call = RuntimeCall::Sudo(pallet_sudo::Call::sudo {
                call: Box::new(RuntimeCall::Balances(BalancesCall::transfer_keep_alive {
                    dest: alith(),
                    value: 1,
                })),
            });
            let info = call.get_dispatch_info();
            let fee = TransactionPayment::compute_fee(0, &info, 0);
            assert!(fee > 0);
            assert_ne!(fee, GetConstantEnergyFee::get());
            assert_eq!(
                EnergyFeeCustomFee::dispatch_info_to_fee(&call, Some(&info), Some(fee)),
                CallFee::Regular(fee)
            );

            let wrapped = RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                index: 0,
                call: Box::new(call),
            });
            let wrapped_info = wrapped.get_dispatch_info();
            let wrapped_fee = TransactionPayment::compute_fee(0, &wrapped_info, 0);
            assert_eq!(
                EnergyFeeCustomFee::dispatch_info_to_fee(
                    &wrapped,
                    Some(&wrapped_info),
                    Some(wrapped_fee)
                ),
                CallFee::Regular(wrapped_fee)
            );
        })
    }

    #[test]
    fn custom_fee_picks_fee_by_call() {
        new_test_ext().execute_with(|| {
            let flat = GetConstantEnergyFee::get();
            // Stands for the weight-based fee that `ChargeTransactionPayment` computes.
            let weight_fee = 1_234;
            let fee_of = |call: &RuntimeCall| {
                EnergyFeeCustomFee::dispatch_info_to_fee(
                    call,
                    Some(&call.get_dispatch_info()),
                    Some(weight_fee),
                )
            };
            let transfer = RuntimeCall::Balances(BalancesCall::transfer_keep_alive {
                dest: alith(),
                value: 1,
            });
            let remark = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
            let evm_call = RuntimeCall::EVM(pallet_evm::Call::call {
                source: H160::zero(),
                target: H160::zero(),
                input: vec![],
                value: U256::zero(),
                gas_limit: 21_000,
                max_fee_per_gas: U256::one(),
                max_priority_fee_per_gas: None,
                nonce: None,
                access_list: vec![],
            });

            assert_eq!(fee_of(&transfer), CallFee::Regular(flat));
            assert_eq!(fee_of(&remark), CallFee::Regular(weight_fee));
            assert_eq!(fee_of(&evm_call), CallFee::EVM(EnergyFeeCustomFee::ethereum_fee()));
            assert_eq!(
                fee_of(&RuntimeCall::EVM(pallet_evm::Call::withdraw {
                    address: H160::zero(),
                    value: 1
                })),
                CallFee::Regular(EnergyFeeCustomFee::ethereum_fee())
            );

            // A batch pays for each of its calls, and at least the flat fee. Its calls have
            // different fees, so paying the flat fee per call would not pass.
            let batch = |calls| RuntimeCall::Utility(pallet_utility::Call::batch { calls });
            let remark_fee = TransactionPayment::compute_fee(
                remark.encode().len() as u32,
                &remark.get_dispatch_info(),
                0,
            );
            assert_ne!(remark_fee, flat);
            assert_eq!(
                fee_of(&batch(vec![transfer.clone(), remark.clone()])),
                CallFee::Regular(flat + remark_fee)
            );
            assert_eq!(fee_of(&batch(vec![])), CallFee::Regular(flat));

            // A wrapped call pays what the call inside it pays. For a call charged by weight, that
            // is the fee passed in, which covers the wrapper as well.
            let as_derivative = |call| {
                RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                    index: 0,
                    call: Box::new(call),
                })
            };
            assert_eq!(fee_of(&as_derivative(transfer)), CallFee::Regular(flat));
            assert_eq!(fee_of(&as_derivative(remark)), CallFee::Regular(weight_fee));
            assert_eq!(
                fee_of(&as_derivative(evm_call)),
                CallFee::Regular(EnergyFeeCustomFee::ethereum_fee())
            );
        })
    }

    #[test]
    fn signer_without_energy_buys_fee_from_broker() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            let energy = NativeOrAssetId::WithId(VNRG::get());
            let broker = EnergyBroker::account_id();
            let fee = GetConstantEnergyFee::get();
            fund_broker_with_vnrg(10 * fee);
            // A rate other than one, so a handler that ignores it charges the wrong price.
            pallet_dynamic_energy::ExchangeRate::<Runtime>::put(FixedU128::from_rational(3, 2));
            let bob = AccountId::from([7u8; 20]);
            Balances::set_balance(&bob, 1_000 * vtrs::UNITS);

            let call = RuntimeCall::Balances(BalancesCall::transfer_keep_alive {
                dest: alith(),
                value: 1,
            });
            let info = call.get_dispatch_info();
            let price = EnergyFeeExchange::quote_price_tokens_for_exact_tokens(fee, true)
                .expect("Expected the broker to price the fee");

            let liquidity = <TransactionFeeHandler as OnChargeTransaction<Runtime>>::withdraw_fee(
                &bob,
                &call,
                &info,
                TransactionPayment::compute_fee(0, &info, 0),
                0,
            )
            .expect("Expected to withdraw fee");
            <TransactionFeeHandler as OnChargeTransaction<Runtime>>::correct_and_deposit_fee(
                &bob,
                &info,
                &PostDispatchInfo::default(),
                fee,
                0,
                liquidity,
            )
            .expect("Expected to settle fee");

            assert_eq!(Balances::free_balance(bob), 1_000 * vtrs::UNITS - price);
            assert_eq!(asset_balance(&energy, &bob), 0);
            assert_eq!(asset_balance(&energy, &broker), 9 * fee);
            System::assert_has_event(
                pallet_energy_fee::Event::<Runtime>::EnergyFeePaid { who: bob, amount: fee }.into(),
            );
        })
    }

    #[test]
    fn evm_fee_goes_to_treasury_and_burn_counters() {
        new_test_ext().execute_with(|| {
            System::set_block_number(1);
            let energy = NativeOrAssetId::WithId(VNRG::get());
            let treasury = Treasury::account_id();
            // VNRG is not sufficient, so the treasury's account must exist before it can hold any.
            Balances::set_balance(&treasury, ExistentialDeposit::get());
            pallet_treasury_extension::FeeRecyclingRate::<Runtime>::put(Permill::from_percent(20));
            let fee = EnergyFeeCustomFee::ethereum_fee();
            let initial_energy = asset_balance(&energy, &alith());
            let alith_h160 = H160::from(alith().0);

            // The runner passes the gas cost, which the energy fee doesn't depend on.
            let liquidity = <EvmFeeHandler as OnChargeEVMTransaction<Runtime>>::withdraw_fee(
                &alith_h160,
                U256::from(1_234_567_890u64),
            )
            .expect("Expected to withdraw fee");
            assert!(<EvmFeeHandler as OnChargeEVMTransaction<Runtime>>::correct_and_deposit_fee(
                &alith_h160,
                U256::zero(),
                U256::zero(),
                liquidity,
            )
            .is_none());

            // The treasury gets the recycled share, and the rest reaches the burn counters.
            let recycled = Permill::from_percent(20) * fee;
            assert_eq!(asset_balance(&energy, &alith()), initial_energy - fee);
            assert_eq!(asset_balance(&energy, &treasury), recycled);
            assert_eq!(pallet_energy_broker::SessionEnergyBurn::<Runtime>::get(), fee - recycled);
            assert_eq!(pallet_dynamic_energy::SessionEnergyBurn::<Runtime>::get(), fee - recycled);
            System::assert_has_event(
                pallet_energy_fee::Event::<Runtime>::EnergyFeePaid { who: alith(), amount: fee }
                    .into(),
            );
        })
    }
}

mod energy_broker {
    use super::*;
    use frame_support::{assert_noop, assert_ok};

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
            let lnrg_path = (NativeOrAssetId::WithId(LNRG::get()), NativeOrAssetId::Native);

            fund_broker();
            assert_eq!(EnergyBroker::max_amount_out(&lnrg_path), broker_vtrs());

            // A zero quota closes the LNRG → VTRS path, but not the conversion.
            set_payout_quota(0);
            assert_eq!(EnergyBroker::max_amount_out(&lnrg_path), 0);

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

            // A quota of 20 VTRS, far below the broker's VTRS.
            fund_broker();
            set_payout_quota(20 * vtrs::UNITS);
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
    fn snrg_buyback_is_not_limited_by_payout_quota() {
        new_test_ext().execute_with(|| {
            let snrg_path = (NativeOrAssetId::WithId(SNRG::get()), NativeOrAssetId::Native);
            let lnrg_path = (NativeOrAssetId::WithId(LNRG::get()), NativeOrAssetId::Native);

            fund_broker();
            Balances::set_balance(&EnergyBroker::account_id(), 50 * vtrs::UNITS);
            set_payout_quota(20 * vtrs::UNITS);

            // The legacy SNRG buyback is not an energy sale: only the broker's VTRS limit it.
            assert_eq!(EnergyBroker::max_amount_out(&snrg_path), broker_vtrs());
            assert_eq!(EnergyBroker::max_amount_out(&lnrg_path), 20 * vtrs::UNITS);

            // It pays out of the same VTRS, though, so it can leave LNRG sales less than the quota.
            let snrg = NativeOrAssetId::WithId(SNRG::get());
            let paid_out = broker_vtrs() - 5 * vtrs::UNITS;
            let snrg_in = EnergyBroker::get_amount_in(paid_out, &snrg_path, true)
                .expect("the SNRG path must price the sale");
            let snrg_before = asset_balance(&snrg, &alith());
            let alith_vtrs_before = Balances::free_balance(alith());

            assert_ok!(EnergyBroker::swap_tokens_for_exact_tokens(
                RuntimeOrigin::signed(alith()),
                alith(),
                snrg_path,
                paid_out,
                None,
                false,
            ));

            assert_eq!(asset_balance(&snrg, &alith()), snrg_before - snrg_in);
            assert_eq!(Balances::free_balance(alith()), alith_vtrs_before + paid_out);
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

    /// Changes the session through the listeners as the runtime wires them, in their order.
    fn change_session(index: SessionIndex) {
        <<Runtime as pallet_energy_generation::Config>::SessionChangeListeners as
            vitreus_runtime_common::OnSessionChange>::on_new_session(index);
    }

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
        set_broker_rates(rate);
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
        set_broker_rates(DUST_RATE);
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
    fn new_session_sells_lnrg_within_payout_quota() {
        new_test_ext().execute_with(|| {
            let treasury = Treasury::account_id();
            let lnrg = NativeOrAssetId::WithId(LNRG::get());

            System::set_block_number(1);
            fund_broker();
            // A quota of 20 VTRS, of which a user's sale takes 9.9 VTRS.
            set_payout_quota(20 * vtrs::UNITS);
            assert_ok!(EnergyBroker::swap_exact_tokens_for_tokens(
                RuntimeOrigin::signed(alith()),
                alith(),
                (lnrg.clone(), NativeOrAssetId::Native),
                10 * vtrs::UNITS,
                None,
                false,
            ));
            let leftover = 20 * vtrs::UNITS - energy_payout();

            // More LNRG than the leftover pays for.
            Balances::set_balance(&treasury, ExistentialDeposit::get());
            assert_ok!(Assets::mint_into(LNRG::get(), &treasury, 2 * leftover));
            let lnrg_before = asset_balance(&lnrg, &treasury);
            let native_before = Balances::free_balance(treasury);

            change_session(1);

            // The treasury sells what is left of the closing session's quota, at one energy per
            // VTRS and without a fee, and keeps the rest.
            assert_eq!(asset_balance(&lnrg, &treasury), lnrg_before - leftover);
            assert_eq!(Balances::free_balance(treasury), native_before + leftover);
            System::assert_has_event(
                pallet_treasury_extension::Event::<Runtime>::EnergyExchanged {
                    energy: 0,
                    static_energy: 0,
                    liquid_energy: leftover,
                    native: leftover,
                }
                .into(),
            );
        })
    }

    #[test]
    fn new_session_counts_treasury_sale_towards_closing_session() {
        new_test_ext().execute_with(|| {
            let treasury = Treasury::account_id();
            let lnrg = NativeOrAssetId::WithId(LNRG::get());

            // No payout quota: the order alone decides where the treasury's sale counts. The rate
            // is calculated from the session's own sales, and from a stake whose payout budget
            // over an expected session is whole on both networks.
            System::set_block_number(1);
            fund_broker();
            pallet_dynamic_energy::EnergySaleOverride::<Runtime>::kill();
            let stake = 525_960 * 1_000_000_000;
            pallet_dynamic_energy::TotalStakeOverride::<Runtime>::put(stake);
            Balances::set_balance(&treasury, ExistentialDeposit::get());
            assert_ok!(Assets::mint_into(LNRG::get(), &treasury, AMOUNT));
            let lnrg_before = asset_balance(&lnrg, &treasury);
            let native_before = Balances::free_balance(treasury);

            change_session(1);

            // At one energy per VTRS and without a fee, the treasury is paid what it sells.
            let sold = lnrg_before - asset_balance(&lnrg, &treasury);
            assert!(sold > 0, "the treasury must sell its LNRG");
            assert_eq!(Balances::free_balance(treasury), native_before + sold);
            System::assert_has_event(
                pallet_treasury_extension::Event::<Runtime>::EnergyExchanged {
                    energy: 0,
                    static_energy: 0,
                    liquid_energy: sold,
                    native: sold,
                }
                .into(),
            );

            // The treasury sells before dynamic-energy closes the session, so the new rate is its
            // sale per native unit of the payout budget: 10% APR over the expected session ...
            let budget = stake * u128::from(ExpectedSessionDuration::get()) / (10 * 31_557_600);
            let rate = FixedU128::from_inner(sold * FixedU128::DIV / budget);
            assert_eq!(pallet_dynamic_energy::ExchangeRate::<Runtime>::get(), Some(rate));
            System::assert_has_event(
                pallet_dynamic_energy::Event::<Runtime>::ExchangeRateUpdated { rate }.into(),
            );
            // ... and the new session starts with no sales.
            assert_eq!(energy_sale(), 0);
            assert_eq!(energy_payout(), 0);
        })
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

mod offences {
    use super::*;
    use frame_support::assert_ok;
    use parachains_slashing::HandleReports;
    use polkadot_primitives::slashing::{DisputesTimeSlot, SlashingOffenceKind};
    use sp_core::{ecdsa, ed25519, sr25519, Pair};
    use sp_runtime::{traits::Convert, Perbill};
    use sp_staking::offence::{Offence, OffenceError, ReportOffence};

    /// A validator as offences identify it: its stash and its exposure.
    type Offender = pallet_session::historical::IdentificationTuple<Runtime>;

    /// Number of validators that the genesis starts with.
    const VALIDATOR_COUNT: u8 = 4;
    /// Stake that each validator bonds at genesis.
    const STAKE: Balance = 1_000 * vtrs::UNITS;
    /// Share that BABE, GRANDPA and BEEFY slash for one equivocator out of `VALIDATOR_COUNT`.
    ///
    /// They slash (3k / n)² for k equivocators out of n, here (3 · 1 / 4)² = 9/16.
    const EQUIVOCATION_FRACTION: Perbill = Perbill::from_parts(562_500_000);

    fn stash(validator: u8) -> AccountId {
        AccountId::from([validator; 20])
    }

    fn controller(validator: u8) -> AccountId {
        AccountId::from([validator + 100; 20])
    }

    fn session_keys(validator: u8) -> opaque::SessionKeys {
        let seed = format!("//{validator}");
        let sr25519_key = sr25519::Pair::from_string(&seed, None).unwrap().public();
        opaque::SessionKeys {
            grandpa: ed25519::Pair::from_string(&seed, None).unwrap().public().into(),
            babe: sr25519_key.into(),
            im_online: sr25519_key.into(),
            para_validator: sr25519_key.into(),
            para_assignment: sr25519_key.into(),
            authority_discovery: sr25519_key.into(),
            beefy: ecdsa::Pair::from_string(&seed, None).unwrap().public().into(),
        }
    }

    /// Runtime genesis with `VALIDATOR_COUNT` validators bonded and validating, at block 1.
    ///
    /// pallet-session builds its genesis before energy-generation, so era 0 starts before anyone
    /// has bonded and has no exposures. Offences in era 0 disable validators but slash nothing.
    fn new_test_ext_with_validators() -> sp_io::TestExternalities {
        let validators = 1..=VALIDATOR_COUNT;
        let mut ext = sp_io::TestExternalities::new(
            RuntimeGenesisConfig {
                balances: BalancesConfig {
                    balances: validators.clone().map(|v| (stash(v), STAKE)).collect(),
                },
                reputation: ReputationConfig {
                    accounts: validators
                        .clone()
                        .map(|v| (stash(v), VALIDATOR_REPUTATION_THRESHOLD.into()))
                        .collect(),
                },
                session: SessionConfig {
                    keys: validators
                        .clone()
                        .map(|v| (controller(v), stash(v), session_keys(v)))
                        .collect(),
                },
                energy_generation: EnergyGenerationConfig {
                    stakers: validators
                        .map(|v| (stash(v), controller(v), STAKE, StakerStatus::Validator))
                        .collect(),
                    ..Default::default()
                },
                ..Default::default()
            }
            .build_storage()
            .unwrap(),
        );
        // frame-system records no events at block 0.
        ext.execute_with(|| System::set_block_number(1));
        ext
    }

    /// `validator` with its exposure in the active era.
    fn offender(validator: u8) -> Offender {
        let exposure =
            <Runtime as pallet_session::historical::Config>::FullIdentificationOf::convert(stash(
                validator,
            ))
            .unwrap();
        (stash(validator), exposure)
    }

    /// A GRANDPA equivocation by `validator` in `round` of session 0.
    fn grandpa_equivocation(
        validator: u8,
        round: u64,
    ) -> pallet_grandpa::EquivocationOffence<Offender> {
        pallet_grandpa::EquivocationOffence {
            time_slot: pallet_grandpa::TimeSlot { set_id: 0, round },
            session_index: 0,
            validator_set_count: VALIDATOR_COUNT.into(),
            offender: offender(validator),
        }
    }

    /// Stashes of the validators that pallet-session has disabled.
    fn disabled_stashes() -> Vec<AccountId> {
        let validators = Session::validators();
        Session::disabled_validators()
            .into_iter()
            .map(|index| validators[index as usize])
            .collect()
    }

    /// Reports an equivocation straight to pallet-offences, where the source's report system sends
    /// it once the proofs check out.
    fn report_equivocation<O>(offence: O) -> Result<(), OffenceError>
    where
        O: Offence<Offender>,
    {
        <Offences as ReportOffence<_, _, _>>::report_offence(vec![], offence)
    }

    /// Reports a lost dispute through the handler that the runtime's config wires in.
    fn report_lost_dispute(
        offence: parachains_slashing::SlashingOffence<
            <Runtime as parachains_slashing::Config>::KeyOwnerIdentification,
        >,
    ) -> Result<(), OffenceError> {
        type Handler = <Runtime as parachains_slashing::Config>::HandleReports;
        <Handler as HandleReports<Runtime>>::report_offence(offence)
    }

    /// Reports unresponsive validators through the reporter that the runtime's config wires in.
    fn report_unresponsiveness(
        offence: pallet_im_online::UnresponsivenessOffence<Offender>,
    ) -> Result<(), OffenceError> {
        type Reporter = <Runtime as pallet_im_online::Config>::ReportUnresponsiveness;
        <Reporter as ReportOffence<_, _, _>>::report_offence(vec![], offence)
    }

    /// Checks that energy-generation got an offence by `validator` in era 0 at `fraction`.
    fn assert_slash_reported(validator: AccountId, fraction: Perbill) {
        System::assert_has_event(
            pallet_energy_generation::Event::<Runtime>::SlashReported {
                validator,
                fraction,
                slash_era: 0,
            }
            .into(),
        );
    }

    /// Reports `offence` through `report` and checks that energy-generation got it at `fraction`
    /// and that pallet-session disabled the offender.
    fn assert_offence_disables_offender<O>(
        report: impl FnOnce(O) -> Result<(), OffenceError>,
        offence: O,
        fraction: Perbill,
    ) where
        O: Offence<Offender>,
    {
        let (offender, _) = offence.offenders()[0].clone();
        let timeslot = offence.time_slot().encode();

        assert_ok!(report(offence));

        assert_slash_reported(offender, fraction);
        System::assert_has_event(pallet_offences::Event::Offence { kind: O::ID, timeslot }.into());
        assert_eq!(disabled_stashes(), vec![offender]);
    }

    #[test]
    fn babe_equivocation_disables_offender() {
        new_test_ext_with_validators().execute_with(|| {
            assert_offence_disables_offender(
                report_equivocation,
                pallet_babe::EquivocationOffence {
                    slot: 1.into(),
                    session_index: 0,
                    validator_set_count: VALIDATOR_COUNT.into(),
                    offender: offender(1),
                },
                EQUIVOCATION_FRACTION,
            )
        });
    }

    #[test]
    fn grandpa_equivocation_disables_offender() {
        new_test_ext_with_validators().execute_with(|| {
            assert_offence_disables_offender(
                report_equivocation,
                grandpa_equivocation(1, 1),
                EQUIVOCATION_FRACTION,
            )
        });
    }

    #[test]
    fn beefy_equivocation_disables_offender() {
        new_test_ext_with_validators().execute_with(|| {
            assert_offence_disables_offender(
                report_equivocation,
                pallet_beefy::EquivocationOffence {
                    time_slot: pallet_beefy::TimeSlot::<BlockNumber> { set_id: 0, round: 1 },
                    session_index: 0,
                    validator_set_count: VALIDATOR_COUNT.into(),
                    offender: offender(1),
                },
                EQUIVOCATION_FRACTION,
            )
        });
    }

    #[test]
    fn lost_dispute_disables_offender() {
        new_test_ext_with_validators().execute_with(|| {
            assert_offence_disables_offender(
                report_lost_dispute,
                parachains_slashing::SlashingOffence {
                    validator_set_count: VALIDATOR_COUNT.into(),
                    time_slot: DisputesTimeSlot::new(0, CandidateHash(H256::repeat_byte(1))),
                    offenders: vec![offender(1)],
                    // What the private `SlashingOffence::new` sets for `ForInvalid`.
                    slash_fraction: Perbill::one(),
                    kind: SlashingOffenceKind::ForInvalid,
                },
                Perbill::one(),
            )
        });
    }

    #[test]
    fn unresponsiveness_chills_and_disables_offender() {
        new_test_ext_with_validators().execute_with(|| {
            assert!(pallet_energy_generation::Validators::<Runtime>::contains_key(stash(1)));

            assert_offence_disables_offender(
                report_unresponsiveness,
                pallet_im_online::UnresponsivenessOffence {
                    session_index: 0,
                    validator_set_count: VALIDATOR_COUNT.into(),
                    offenders: vec![offender(1)],
                },
                // im-online slashes nothing while at most n / 10 + 1 of n validators are offline.
                Perbill::zero(),
            );

            System::assert_has_event(
                pallet_energy_generation::Event::<Runtime>::Chilled { stash: stash(1) }.into(),
            );
            assert!(!pallet_energy_generation::Validators::<Runtime>::contains_key(stash(1)));
        });
    }

    #[test]
    fn offender_beyond_disabling_limit_stays_enabled() {
        new_test_ext_with_validators().execute_with(|| {
            // Energy-generation disables at most (n - 1) / 3 of n validators: one out of four.
            assert_ok!(report_equivocation(grandpa_equivocation(1, 1)));
            assert_ok!(report_equivocation(grandpa_equivocation(2, 2)));

            assert_slash_reported(stash(2), EQUIVOCATION_FRACTION);
            assert_eq!(disabled_stashes(), vec![stash(1)]);
        });
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

/// Launchpad free-trade wiring: the free-trade fee is the broker's price for one launchpad call.
#[cfg(feature = "testnet-runtime")]
mod launchpad_free_trades {
    use super::*;

    fn one_call_energy() -> Balance {
        <EnergyFee as CustomFee<
            RuntimeCall,
            DispatchInfoOf<RuntimeCall>,
            Balance,
            GetConstantEnergyFee,
        >>::custom_fee()
    }

    #[test]
    fn free_trade_fee_is_the_broker_price_of_one_call_and_200x_at_the_default_split() {
        new_test_ext().execute_with(|| {
            // A launchpad call is charged the flat fee, which is what the free-trade fee prices.
            let call = RuntimeCall::Launchpad(pallet_launchpad::Call::buy {
                launch_id: 0,
                quote_in: 1,
                min_tokens_out: 0,
            });
            assert_eq!(
                <Runtime as pallet_energy_fee::Config>::CustomFee::dispatch_info_to_fee(
                    &call,
                    Some(&call.get_dispatch_info()),
                    None,
                ),
                CallFee::Regular(one_call_energy())
            );

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
}
