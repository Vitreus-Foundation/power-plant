//! Tests for the module.

// use frame_support::pallet_prelude::*;
use crate::mock::*;
use frame_support::traits::fungible::Mutate;
use frame_support::traits::Currency;
use frame_support::{assert_ok, dispatch::DispatchInfo};
use frame_system::mocking::MockUncheckedExtrinsic;
use frame_system::weights::{SubstrateWeight as SystemWeight, WeightInfo as _};
use pallet_assets::{weights::SubstrateWeight as AssetsWeight, WeightInfo as _};
use pallet_evm::{Config as EVMConfig, GasWeightMapping, OnChargeEVMTransaction};
use pallet_transaction_payment::OnChargeTransaction;
use parity_scale_codec::Encode;

type Extrinsic = MockUncheckedExtrinsic<Test>;

const INITIAL_ENERGY_BALANCE: Balance = 1_000_000_000_000;

#[test]
fn withdraw_fee_with_stock_coefficients_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        let initial_energy_balance: Balance = BalancesVNRG::free_balance(ALICE);

        let system_remark_call: RuntimeCall =
            RuntimeCall::System(frame_system::Call::remark { remark: [1u8; 32].to_vec() });

        let dispatch_info: DispatchInfo =
            DispatchInfo { weight: SystemWeight::<Test>::remark(32), ..Default::default() };

        let extrinsic_len: u32 =
            Extrinsic::new_signed(system_remark_call.clone(), ALICE, (), ()).encode().len() as u32;

        let computed_fee = TransactionPayment::compute_fee(extrinsic_len, &dispatch_info, 0);

        assert_ok!(<EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &system_remark_call,
            &dispatch_info,
            computed_fee,
            0,
        ));

        assert_eq!(
            BalancesVNRG::free_balance(ALICE),
            initial_energy_balance.saturating_sub(computed_fee),
        );
    });
}

#[test]
fn withdraw_fee_with_custom_coefficients_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        let initial_energy_balance: Balance = BalancesVNRG::free_balance(ALICE);
        let transfer_amount: Balance = 1_000_000_000;

        let assets_transfer_call: RuntimeCall =
            RuntimeCall::Assets(pallet_assets::Call::transfer {
                id: VNRG.into(),
                target: BOB,
                amount: transfer_amount,
            });

        let dispatch_info: DispatchInfo =
            DispatchInfo { weight: AssetsWeight::<Test>::transfer(), ..Default::default() };

        // arbitrary number, since it does not influence resulting fee in this case
        let extrinsic_len: u32 = 1000;

        let computed_fee = TransactionPayment::compute_fee(extrinsic_len, &dispatch_info, 0);

        assert_ok!(<EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &assets_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        ));

        let constant_fee = GetConstantEnergyFee::get();

        assert_eq!(
            BalancesVNRG::free_balance(ALICE),
            initial_energy_balance.saturating_sub(constant_fee),
        );
    });
}

#[test]
fn withdraw_zero_fee_during_evm_extrinsic_call_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        let initial_energy_balance: Balance = BalancesVNRG::free_balance(ALICE);
        let transfer_amount: Balance = 1_000_000_000;
        let gas_limit: u64 = 1_000_000;

        let evm_transfer_call: RuntimeCall = RuntimeCall::EVM(pallet_evm::Call::call {
            source: ALICE.into(),
            target: BOB.into(),
            input: vec![],
            value: transfer_amount.into(),
            gas_limit: 1_000_000,
            max_fee_per_gas: 1_000_000u128.into(),
            max_priority_fee_per_gas: None,
            nonce: None,
            access_list: vec![],
        });

        let dispatch_info: DispatchInfo = DispatchInfo {
            weight: <Test as EVMConfig>::GasWeightMapping::gas_to_weight(gas_limit, true),
            ..Default::default()
        };

        // arbitrary number, since it does not influence resulting fee in this case
        let extrinsic_len: u32 = 1000;

        let computed_fee = TransactionPayment::compute_fee(extrinsic_len, &dispatch_info, 0);

        assert_ok!(<EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &evm_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        ));

        assert_eq!(BalancesVNRG::free_balance(ALICE), initial_energy_balance,);
    });
}

#[test]
fn evm_withdraw_fee_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        let initial_energy_balance: Balance = BalancesVNRG::free_balance(ALICE);

        // fee equals arbitrary number since we don't take it into account
        assert_ok!(<EnergyFee as OnChargeEVMTransaction<Test>>::withdraw_fee(
            &ALICE.into(),
            1_234_567_890.into(),
        ));

        let constant_fee = GetConstantEnergyFee::get();
        assert_eq!(
            BalancesVNRG::free_balance(ALICE),
            initial_energy_balance.saturating_sub(constant_fee),
        );
    });
}

#[test]
fn vtrs_exchange_during_withdraw_evm_fee_works() {
    new_test_ext(0).execute_with(|| {
        let initial_vtrs_balance: Balance = BalancesVTRS::free_balance(ALICE);

        let (num, denom) = VTRSEnergyRate::get();

        // fee equals arbitrary number since we don't take it into account
        assert_ok!(<EnergyFee as OnChargeEVMTransaction<Test>>::withdraw_fee(
            &ALICE.into(),
            1_234_567_890.into(),
        ));

        let constant_fee = GetConstantEnergyFee::get();
        let vtrs_fee = constant_fee.saturating_mul(num).saturating_div(denom);
        assert_eq!(
            BalancesVNRG::free_balance(ALICE),
            initial_vtrs_balance.saturating_sub(vtrs_fee),
        );
    });
}
