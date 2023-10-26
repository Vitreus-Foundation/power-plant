use super::*;
use chain_spec::{devnet_keys::alith, devnet_config};
use frame_support::{
    dispatch::DispatchClass,
    traits::Hooks,
};
use pallet_energy_fee::DefaultFeeMultiplier;
use sp_runtime::{Perquintill, FixedU128, BuildStorage};


pub fn devnet_ext() -> sp_io::TestExternalities {
    sp_io::TestExternalities::new(
        devnet_config().build_storage().unwrap()
    )
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
        let max_block_weight = BlockWeights::get()
            .per_class
            .get(DispatchClass::Normal)
            .max_total
            .unwrap();
        let block_weight_a = max_block_weight/2;
        System::set_block_consumed_resources(block_weight_a, 0);
        // println!("block weight: {:#?}", System::block_weight());
        // println!("del");
        // println!("max block weight: {:#?}", );

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), DefaultFeeMultiplier::<Runtime>::get());

        let block_fullness_a = Perquintill::from_percent(50) + Perquintill::from_parts(1000000);
        let upper_fee_multiplier = FixedU128::from_rational(2, 1);
        EnergyFee::update_block_fullness_threshold(RuntimeOrigin::root(), block_fullness_a).expect("Expected to set a new block fullness threshold");
        EnergyFee::update_upper_fee_multiplier(RuntimeOrigin::root(), upper_fee_multiplier).expect("Expected to set a new upper fee multiplier");

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), DefaultFeeMultiplier::<Runtime>::get());

        let block_fullness_b = Perquintill::from_percent(50);
        EnergyFee::update_block_fullness_threshold(RuntimeOrigin::root(), block_fullness_b).expect("Expected to set a new block fullness threshold");

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), upper_fee_multiplier);

        let call_with_custom_fee = RuntimeCall::Balances(BalancesCall::transfer { dest: alith(), value: 1 });
        let dispatch_info = call_with_custom_fee.get_dispatch_info();
        let updated_custom_fee = upper_fee_multiplier.saturating_mul_int(GetConstantEnergyFee::get());

        assert_eq!(
            EnergyFee::dispatch_info_to_fee(&call_with_custom_fee, &dispatch_info),
            CallFee::Custom(updated_custom_fee)
        );

        let block_weight_b = max_block_weight/3;
        System::set_block_consumed_resources(block_weight_b, 0);

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), DefaultFeeMultiplier::<Runtime>::get());

        assert_eq!(
            EnergyFee::dispatch_info_to_fee(&call_with_custom_fee, &dispatch_info),
            CallFee::Custom(GetConstantEnergyFee::get())
        );        
    });
}