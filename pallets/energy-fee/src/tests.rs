//! Tests for the module.

// use frame_support::pallet_prelude::*;
use crate::{mock::*, BurnedEnergy, BurnedEnergyThreshold, CheckEnergyFee, Event};
use frame_support::traits::Hooks;
use frame_support::{dispatch::DispatchInfo, traits::fungible::Inspect};
use frame_system::RawOrigin;
use frame_system::mocking::MockUncheckedExtrinsic;
use frame_system::weights::{SubstrateWeight as SystemWeight, WeightInfo as _};
use pallet_assets::{weights::SubstrateWeight as AssetsWeight, WeightInfo as _};
use pallet_evm::{Config as EVMConfig, GasWeightMapping, OnChargeEVMTransaction};
use pallet_transaction_payment::OnChargeTransaction;
use parity_scale_codec::Encode;
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};
use sp_runtime::{traits::SignedExtension, FixedPointNumber, DispatchError};

type Extrinsic = MockUncheckedExtrinsic<Test>;

const INITIAL_ENERGY_BALANCE: Balance = 1_000_000_000_000;

// TODO: replace numeric constants with named constants and define all of them in mock

#[test]
fn withdraw_fee_with_stock_coefficients_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        System::set_block_number(1);
        let initial_energy_balance: Balance = BalancesVNRG::balance(&ALICE);

        let system_remark_call: RuntimeCall =
            RuntimeCall::System(frame_system::Call::remark { remark: [1u8; 32].to_vec() });

        let dispatch_info: DispatchInfo =
            DispatchInfo { weight: SystemWeight::<Test>::remark(32), ..Default::default() };

        let extrinsic_len: u32 =
            Extrinsic::new_signed(system_remark_call.clone(), ALICE, (), ()).encode().len() as u32;

        let computed_fee = TransactionPayment::compute_fee(extrinsic_len, &dispatch_info, 0);

        assert!(<EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &system_remark_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .is_ok());

        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance.saturating_sub(computed_fee),
        );

        System::assert_has_event(Event::<Test>::EnergyFeePaid { who: ALICE, amount: computed_fee }.into());
    });
}

#[test]
fn withdraw_fee_with_custom_coefficients_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        System::set_block_number(1);
        let initial_energy_balance: Balance = BalancesVNRG::balance(&ALICE);
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

        assert!(<EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &assets_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .is_ok());

        let constant_fee = GetConstantEnergyFee::get();

        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance.saturating_sub(constant_fee),
        );

        assert_eq!(BurnedEnergy::<Test>::get(), constant_fee);
        System::assert_has_event(Event::<Test>::EnergyFeePaid { who: ALICE, amount: constant_fee }.into());
    });
}

#[test]
fn withdraw_zero_fee_during_evm_extrinsic_call_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        System::set_block_number(1);
        let initial_energy_balance: Balance = BalancesVNRG::balance(&ALICE);
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

        assert!(<EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &evm_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .is_ok());

        assert_eq!(BalancesVNRG::balance(&ALICE), initial_energy_balance);
    });
}

#[test]
fn evm_withdraw_fee_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        System::set_block_number(1);
        let initial_energy_balance: Balance = BalancesVNRG::balance(&ALICE);

        // fee equals arbitrary number since we don't take it into account
        assert!(<EnergyFee as OnChargeEVMTransaction<Test>>::withdraw_fee(
            &ALICE.into(),
            1_234_567_890.into(),
        )
        .is_ok());

        let constant_fee = GetConstantEnergyFee::get();
        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance.saturating_sub(constant_fee),
        );

        assert_eq!(BurnedEnergy::<Test>::get(), constant_fee);
        System::assert_has_event(Event::<Test>::EnergyFeePaid { who: ALICE, amount: constant_fee }.into());
    });
}

#[test]
fn vtrs_exchange_during_withdraw_evm_fee_works() {
    new_test_ext(2).execute_with(|| {
        System::set_block_number(1);
        let initial_vtrs_balance: Balance = BalancesVTRS::balance(&ALICE);

        // fee equals arbitrary number since we don't take it into account
        assert!(<EnergyFee as OnChargeEVMTransaction<Test>>::withdraw_fee(
            &ALICE.into(),
            1_234_567_890.into(),
        )
        .is_ok());

        let constant_fee = GetConstantEnergyFee::get() - 1;
        let vtrs_fee = VNRG_TO_VTRS_RATE
            .checked_mul_int(constant_fee)
            .expect("Expected to calculate missing fee in VTRS");
        assert_eq!(BalancesVTRS::balance(&ALICE), initial_vtrs_balance.saturating_sub(vtrs_fee),);
        assert_eq!(BalancesVNRG::balance(&ALICE), 1,);
        System::assert_has_event(Event::<Test>::EnergyFeePaid { who: ALICE, amount: GetConstantEnergyFee::get() }.into());
    });
}

#[test]
fn vtrs_exchange_during_withdraw_fee_with_stock_coefficients_works() {
    new_test_ext(0).execute_with(|| {
        let initial_vtrs_balance: Balance = BalancesVTRS::balance(&ALICE);

        let system_remark_call: RuntimeCall =
            RuntimeCall::System(frame_system::Call::remark { remark: [1u8; 32].to_vec() });

        let dispatch_info: DispatchInfo =
            DispatchInfo { weight: SystemWeight::<Test>::remark(32), ..Default::default() };

        let extrinsic_len: u32 =
            Extrinsic::new_signed(system_remark_call.clone(), ALICE, (), ()).encode().len() as u32;

        let computed_fee = TransactionPayment::compute_fee(extrinsic_len, &dispatch_info, 0);

        assert!(<EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &system_remark_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .is_ok());

        // We add 1 since VNRG is sufficient and account must have existential
        // balance
        let vtrs_fee = VNRG_TO_VTRS_RATE
            .checked_mul_int(computed_fee + 1)
            .expect("Expected to calculate missing fee in VTRS");

        assert_eq!(BalancesVTRS::balance(&ALICE), initial_vtrs_balance.saturating_sub(vtrs_fee),);
    });
}

#[test]
fn check_burned_energy_threshold_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        let transfer_amount: Balance = 1_000_000_000;
        let assets_transfer_call: RuntimeCall =
            RuntimeCall::Assets(pallet_assets::Call::transfer {
                id: VNRG.into(),
                target: BOB,
                amount: transfer_amount,
            });
        let dispatch_info: DispatchInfo =
            DispatchInfo { weight: AssetsWeight::<Test>::transfer(), ..Default::default() };
        let extrinsic_len: usize = 1000;

        let extension: CheckEnergyFee<Test> = CheckEnergyFee::new();
        assert!(extension
            .clone()
            .pre_dispatch(&ALICE, &assets_transfer_call, &dispatch_info, extrinsic_len)
            .is_ok());

        BurnedEnergyThreshold::<Test>::put(1_000_000_001);
        assert!(extension
            .clone()
            .pre_dispatch(&ALICE, &assets_transfer_call, &dispatch_info, extrinsic_len)
            .is_ok());

        BurnedEnergyThreshold::<Test>::put(999_999_999);
        assert_eq!(
            extension.pre_dispatch(&ALICE, &assets_transfer_call, &dispatch_info, extrinsic_len),
            Err(TransactionValidityError::Invalid(InvalidTransaction::ExhaustsResources))
        );
    });
}

#[test]
fn reset_burned_energy_on_init_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        BurnedEnergy::<Test>::put(1_234_567_890);
        EnergyFee::on_initialize(1);
        assert_eq!(EnergyFee::burned_energy(), 0);
    });
}

#[test]
fn update_burned_energy_threshold_works() {
    new_test_ext(0).execute_with(|| {
        System::set_block_number(1);
        assert_eq!(EnergyFee::burned_energy_threshold(), None);
        let new_threshold = 1_234_567_890;
        assert_eq!(
            EnergyFee::update_burned_energy_threshold(
                RawOrigin::Signed(ALICE).into(),
                new_threshold
            ),
            Err(DispatchError::BadOrigin.into())
        );
        EnergyFee::update_burned_energy_threshold(
            RawOrigin::Root.into(),
            new_threshold
        ).expect("Expected to set a new burned energy threshold");

        System::assert_last_event(Event::<Test>::BurnedEnergyThresholdUpdated { new_threshold }.into());

        assert_eq!(EnergyFee::burned_energy_threshold(), Some(new_threshold));
    });
}
