use super::*;
use chain_spec::{
    devnet_config,
    devnet_keys::{alith, baltathar},
};
use ethereum::{TransactionAction, TransactionSignature, TransactionV2};
use fp_self_contained::SelfContainedCall;
use frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::Hooks,
};
use pallet_energy_fee::DefaultFeeMultiplier;
use sp_runtime::{BuildStorage, FixedU128, Perquintill};

pub fn devnet_ext() -> sp_io::TestExternalities {
    sp_io::TestExternalities::new(devnet_config().build_storage().unwrap())
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
    let base_extrinsic = <Runtime as frame_system::Config>::BlockWeights::get()
        .get(frame_support::dispatch::DispatchClass::Normal)
        .base_extrinsic;
    assert!(base_extrinsic.ref_time() <= min_ethereum_transaction_weight.ref_time());
}

#[test]
fn fee_multiplier_update_works() {
    devnet_ext().execute_with(|| {
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

// TODO: add checks for tx execution results (resolve the problem with the nac level intializing)
#[test]
fn runtime_should_allow_ethereum_txs_with_zero_gas_limit() {
    devnet_ext().execute_with(|| {
        let baltathar_h160 = H160::from(baltathar().0);
        let alith_h160 = H160::from(alith().0);

        let amount = 1_000_000_000;

        let sample_tx = TransactionV2::Legacy(LegacyTransaction {
            nonce: Default::default(),
            gas_price: 1.into(),
            gas_limit: 0.into(),
            action: TransactionAction::Call(baltathar_h160),
            value: amount.into(),
            input: Default::default(),
            signature: mock_signature(),
        });

        let ethereum_call = pallet_ethereum::Call::new_call_variant_transact(sample_tx);
        let runtime_call = RuntimeCall::Ethereum(ethereum_call);
        let dispatch_info = runtime_call.get_dispatch_info();
        let len = 0_usize;

        assert!(matches!(
            runtime_call.validate_self_contained(&alith_h160, &dispatch_info, len),
            Some(Ok(..))
        ));

        let uxt = UncheckedExtrinsic::new_unsigned(runtime_call);
        assert!(Executive::apply_extrinsic(uxt).is_ok());
    })
}

#[test]
fn validate_self_contained_should_disallow_calls_if_sender_cant_pay_fees() {
    devnet_ext().execute_with(|| {
        let sample_tx = TransactionV2::Legacy(LegacyTransaction {
            nonce: Default::default(),
            gas_price: 1.into(),
            gas_limit: 0.into(),
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
