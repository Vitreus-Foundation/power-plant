//! Energy Fee Pallet Test Suite
//!
//! Comprehensive test coverage for the Energy Fee pallet's functionality, including fee
//! calculation, token exchange, threshold management, and EVM integration.
//!
//! # Test Categories
//!
//! ## Standard Fee Management
//! - `withdraw_fee_with_stock_coefficients_works`: Base fee withdrawal functionality
//! - `withdraw_fee_with_custom_coefficients_works`: Custom fee calculation paths
//! - `update_base_fee_works`: Base fee modification tests
//!
//! ## EVM Integration
//! - `withdraw_zero_fee_during_evm_extrinsic_call_works`: EVM zero-fee scenarios
//! - `evm_withdraw_fee_works`: EVM fee withdrawal functionality
//! - `vtrs_exchange_during_withdraw_evm_fee_works`: Token exchange in EVM context
//! - `fee_multiplier_works_for_evm`: EVM fee multiplier functionality
//!
//! ## Token Exchange
//! - `vtrs_exchange_during_withdraw_fee_with_stock_coefficients_works`: VTRS/VNRG exchange
//! - `exchange_should_not_withdraw_reserved_balance`: Reserved balance protection
//!
//! ## Threshold Management
//! - `check_burned_energy_threshold_works`: Energy burn limits
//! - `check_sudo_bypass_burned_energy_threshold_works`: Admin override tests
//! - `reset_burned_energy_on_init_works`: Energy reset functionality
//!
//! ## Fee Multiplier
//! - `update_upper_fee_multiplier_works`: Fee multiplier configuration
//! - `fee_multiplier_works`: Dynamic fee adjustment tests
//!
//! # Test Constants
//!
//! ```rust
//! const INITIAL_ENERGY_BALANCE: Balance = 1_000_000_000_000;
//! ```
//!
//! # Test Utilities
//!
//! The test suite uses the mock runtime defined in `mock.rs` and includes:
//! - Block weight simulation
//! - Account balance checking
//! - Event verification
//! - Transaction validation
//!
//! # Security Testing Notes
//!
//! Tests specifically verify:
//! 1. Proper access control for admin functions
//! 2. Protection of reserved balances
//! 3. Correct threshold enforcement
//! 4. Fee calculation accuracy
//! 5. Safe token exchange mechanics

use crate::{mock::*, BurnedEnergy, BurnedEnergyThreshold, CheckEnergyFee, Event, TokenExchange};
use frame_support::{
    dispatch::{DispatchInfo, GetDispatchInfo},
    traits::{
        fungible::Inspect, Hooks, LockIdentifier, LockableCurrency, NamedReservableCurrency,
        WithdrawReasons,
    },
};
use frame_system::{
    mocking::MockUncheckedExtrinsic,
    weights::{SubstrateWeight as SystemWeight, WeightInfo as _},
    RawOrigin,
};
use pallet_assets::{weights::SubstrateWeight as AssetsWeight, WeightInfo as _};
use pallet_evm::{Config as EVMConfig, GasWeightMapping, OnChargeEVMTransaction};
use pallet_transaction_payment::{Multiplier, OnChargeTransaction};
use parity_scale_codec::Encode;
use sp_arithmetic::Perbill;
use sp_runtime::{
    traits::{One, SignedExtension},
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    DispatchError, FixedPointNumber, Perquintill,
};

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

        let withdraw_result = <EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &system_remark_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .expect("Expected to withdraw fee");
        assert!(<EnergyFee as OnChargeTransaction<Test>>::correct_and_deposit_fee(
            &ALICE,
            &dispatch_info,
            &From::from(()),
            0,
            0,
            withdraw_result
        )
        .is_ok());

        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance.saturating_sub(computed_fee),
        );
        assert_eq!(
            BalancesVNRG::balance(&FEE_DEST),
            Perbill::from_rational(2u32, 10u32).mul_floor(computed_fee)
        );

        System::assert_has_event(
            Event::<Test>::EnergyFeePaid { who: ALICE, amount: computed_fee }.into(),
        );
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
        System::assert_has_event(
            Event::<Test>::EnergyFeePaid { who: ALICE, amount: constant_fee }.into(),
        );
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
        let withdraw_result = <EnergyFee as OnChargeEVMTransaction<Test>>::withdraw_fee(
            &ALICE.into(),
            1_234_567_890.into(),
        )
        .expect("Expected to withdraw fee");

        assert!(<EnergyFee as OnChargeEVMTransaction<Test>>::correct_and_deposit_fee(
            &ALICE.into(),
            0.into(),
            0.into(),
            withdraw_result
        )
        .is_none());

        let constant_fee = GetConstantEnergyFee::get();
        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance.saturating_sub(constant_fee),
        );
        assert_eq!(
            BalancesVNRG::balance(&FEE_DEST),
            Perbill::from_rational(2u32, 10u32).mul_floor(constant_fee)
        );

        assert_eq!(BurnedEnergy::<Test>::get(), constant_fee);
        System::assert_has_event(
            Event::<Test>::EnergyFeePaid { who: ALICE, amount: constant_fee }.into(),
        );
    });
}

#[test]
fn vtrs_exchange_during_withdraw_evm_fee_works() {
    new_test_ext(0).execute_with(|| {
        System::set_block_number(1);
        let initial_vtrs_balance: Balance = BalancesVTRS::balance(&ALICE);
        let initial_vtrs_recycle_dest_balance: Balance = BalancesVTRS::balance(&MAIN_DEST);

        // fee equals arbitrary number since we don't take it into account
        assert!(<EnergyFee as OnChargeEVMTransaction<Test>>::withdraw_fee(
            &ALICE.into(),
            1_234_567_890.into(),
        )
        .is_ok());

        let constant_fee = GetConstantEnergyFee::get();
        let vtrs_fee = VNRG_TO_VTRS_RATE
            .checked_mul_int(constant_fee)
            .expect("Expected to calculate missing fee in VTRS");
        assert_eq!(BalancesVTRS::balance(&ALICE), initial_vtrs_balance - vtrs_fee);
        assert_eq!(BalancesVNRG::balance(&ALICE), 0);
        assert_eq!(BalancesVTRS::balance(&MAIN_DEST), initial_vtrs_recycle_dest_balance + vtrs_fee);

        System::assert_has_event(
            Event::<Test>::EnergyFeePaid { who: ALICE, amount: GetConstantEnergyFee::get() }.into(),
        );
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
            .checked_mul_int(computed_fee)
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
fn check_sudo_bypass_burned_energy_threshold_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        BurnedEnergyThreshold::<Test>::put(0);
        let transfer_amount: Balance = 1_000_000_000;
        let assets_transfer_call: RuntimeCall =
            RuntimeCall::Assets(pallet_assets::Call::transfer {
                id: VNRG.into(),
                target: BOB,
                amount: transfer_amount,
            });
        let sudo_assets_transfer_call: RuntimeCall =
            RuntimeCall::Sudo(pallet_sudo::Call::sudo { call: Box::new(assets_transfer_call) });
        let dispatch_info: DispatchInfo = sudo_assets_transfer_call.get_dispatch_info();
        let extrinsic_len: usize = 1000;

        let extension: CheckEnergyFee<Test> = CheckEnergyFee::new();
        assert!(extension
            .clone()
            .pre_dispatch(&ALICE, &sudo_assets_transfer_call, &dispatch_info, extrinsic_len)
            .is_ok());
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
        EnergyFee::update_burned_energy_threshold(RawOrigin::Root.into(), new_threshold)
            .expect("Expected to set a new burned energy threshold");

        System::assert_last_event(
            Event::<Test>::BurnedEnergyThresholdUpdated { new_threshold }.into(),
        );

        assert_eq!(EnergyFee::burned_energy_threshold(), Some(new_threshold));
    });
}

#[test]
fn update_block_fulness_threshold_works() {
    new_test_ext(0).execute_with(|| {
        System::set_block_number(1);
        assert_eq!(EnergyFee::block_fullness_threshold(), Perquintill::one());
        let new_threshold = Perquintill::from_parts(1_234_567_890);
        assert_eq!(
            EnergyFee::update_block_fullness_threshold(
                RawOrigin::Signed(ALICE).into(),
                new_threshold
            ),
            Err(DispatchError::BadOrigin.into())
        );
        EnergyFee::update_block_fullness_threshold(RawOrigin::Root.into(), new_threshold)
            .expect("Expected to set a new block fullness threshold");

        System::assert_last_event(
            Event::<Test>::BlockFullnessThresholdUpdated { new_threshold }.into(),
        );

        assert_eq!(EnergyFee::block_fullness_threshold(), new_threshold);
    });
}

#[test]
fn update_upper_fee_multiplier_works() {
    new_test_ext(0).execute_with(|| {
        System::set_block_number(1);
        assert_eq!(EnergyFee::upper_fee_multiplier(), Multiplier::one());
        let new_multiplier = Multiplier::from(1_234_567_890);
        assert_eq!(
            EnergyFee::update_upper_fee_multiplier(RawOrigin::Signed(ALICE).into(), new_multiplier),
            Err(DispatchError::BadOrigin.into())
        );
        EnergyFee::update_upper_fee_multiplier(RawOrigin::Root.into(), new_multiplier)
            .expect("Expected to set a upper fee multiplier");

        System::assert_last_event(
            Event::<Test>::UpperFeeMultiplierUpdated { new_multiplier }.into(),
        );

        assert_eq!(EnergyFee::upper_fee_multiplier(), new_multiplier);
    });
}

#[test]
fn fee_multiplier_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        System::set_block_number(1);
        let initial_energy_balance: Balance = BalancesVNRG::balance(&ALICE);
        let transfer_amount: Balance = 1_000_000_000;

        let initial_fee_multiplier = TransactionPayment::next_fee_multiplier();
        assert_eq!(initial_fee_multiplier, Multiplier::one());

        let assets_transfer_call: RuntimeCall =
            RuntimeCall::Assets(pallet_assets::Call::transfer {
                id: VNRG.into(),
                target: BOB,
                amount: transfer_amount,
            });

        let dispatch_info: DispatchInfo = assets_transfer_call.get_dispatch_info();
        let extrinsic_len: u32 = 1000;
        let computed_fee = TransactionPayment::compute_fee(extrinsic_len, &dispatch_info, 0);

        <EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &assets_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .expect("Expected to withdraw fee");

        let constant_fee_1 = initial_fee_multiplier.saturating_mul_int(GetConstantEnergyFee::get());

        assert_eq!(BalancesVNRG::balance(&ALICE), initial_energy_balance - constant_fee_1,);

        let new_multiplier = Multiplier::from(2);
        EnergyFee::update_upper_fee_multiplier(RawOrigin::Root.into(), new_multiplier)
            .expect("Expected to set a upper fee multiplier");

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), initial_fee_multiplier);

        // Update block fullness threshold
        let block_fullness_threshold = Perquintill::from_percent(50);
        EnergyFee::update_block_fullness_threshold(RuntimeOrigin::root(), block_fullness_threshold)
            .expect("Expected to update block fullness threshold");

        // Set block weight to be equal to the block weight threshold
        let mock_block_weight = calculate_block_weight_based_on_threshold(block_fullness_threshold);
        System::set_block_consumed_resources(mock_block_weight, 0);

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), new_multiplier);

        let constant_fee_2 = new_multiplier.saturating_mul_int(GetConstantEnergyFee::get());

        <EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &assets_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .expect("Expected to withdraw fee");
        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance - constant_fee_1 - constant_fee_2,
        );
    });
}

#[test]
fn fee_multiplier_works_for_evm() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        System::set_block_number(1);
        let initial_energy_balance: Balance = BalancesVNRG::balance(&ALICE);

        let new_multiplier = Multiplier::from(2);
        EnergyFee::update_upper_fee_multiplier(RawOrigin::Root.into(), new_multiplier)
            .expect("Expected to set a upper fee multiplier");

        // Update block fullness threshold
        let block_fullness_threshold = Perquintill::from_percent(50);
        EnergyFee::update_block_fullness_threshold(RuntimeOrigin::root(), block_fullness_threshold)
            .expect("Expected to update block fullness threshold");

        // Set block weight to be equal to the block weight threshold
        let mock_block_weight = calculate_block_weight_based_on_threshold(block_fullness_threshold);
        System::set_block_consumed_resources(mock_block_weight, 0);

        TransactionPayment::on_finalize(1);
        assert_eq!(TransactionPayment::next_fee_multiplier(), new_multiplier);

        assert!(<EnergyFee as OnChargeEVMTransaction<Test>>::withdraw_fee(
            &ALICE.into(),
            1_234_567_890.into(),
        )
        .is_ok());

        let constant_fee = new_multiplier.saturating_mul_int(GetConstantEnergyFee::get());
        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance.saturating_sub(constant_fee),
        );
        assert_eq!(BalancesVNRG::balance(&ALICE), initial_energy_balance - constant_fee,);
    });
}

#[test]
fn update_base_fee_works() {
    new_test_ext(INITIAL_ENERGY_BALANCE).execute_with(|| {
        let initial_energy_balance: Balance = BalancesVNRG::balance(&ALICE);
        let transfer_amount: Balance = 1_000_000_000;

        let assets_transfer_call: RuntimeCall =
            RuntimeCall::Assets(pallet_assets::Call::transfer {
                id: VNRG.into(),
                target: BOB,
                amount: transfer_amount,
            });

        let dispatch_info: DispatchInfo = assets_transfer_call.get_dispatch_info();
        let extrinsic_len: u32 = 1000;
        let computed_fee = TransactionPayment::compute_fee(extrinsic_len, &dispatch_info, 0);

        <EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &assets_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .expect("Expected to withdraw fee");

        let constant_fee_1 = GetConstantEnergyFee::get();
        assert_eq!(BalancesVNRG::balance(&ALICE), initial_energy_balance - constant_fee_1,);

        let constant_fee_2 = 100;
        EnergyFee::update_base_fee(RuntimeOrigin::root(), constant_fee_2)
            .expect("Expected to set a new base fee");

        <EnergyFee as OnChargeTransaction<Test>>::withdraw_fee(
            &ALICE,
            &assets_transfer_call,
            &dispatch_info,
            computed_fee,
            0,
        )
        .expect("Expected to withdraw fee");

        assert_eq!(
            BalancesVNRG::balance(&ALICE),
            initial_energy_balance - constant_fee_1 - constant_fee_2,
        );
    });
}

#[test]
fn exchange_should_not_withdraw_reserved_balance() {
    new_test_ext(0).execute_with(|| {
        assert_eq!(BalancesVTRS::free_balance(&ALICE), VTRS_INITIAL_BALANCE);
        let exchange_amount = 10;
        let freeze_amount = 100;

        const VESTING_ID: [u8; 8] = *b"vesting ";
        const STAKING_ID: LockIdentifier = *b"staking ";
        BalancesVTRS::reserve_named(
            &VESTING_ID,
            &ALICE,
            VTRS_INITIAL_BALANCE - exchange_amount - freeze_amount,
        )
        .expect("Expected to reserve VTRS");
        BalancesVTRS::set_lock(STAKING_ID, &ALICE, freeze_amount, WithdrawReasons::all());

        assert!(<EnergyExchange as TokenExchange<
            AccountId,
            BalancesVTRS,
            BalancesVNRG,
            MainBurnDestination<MainBurnAccount>,
            Balance,
        >>::exchange_inner(&ALICE, exchange_amount + 1, 1)
        .is_err());

        assert!(<EnergyExchange as TokenExchange<
            AccountId,
            BalancesVTRS,
            BalancesVNRG,
            MainBurnDestination<MainBurnAccount>,
            Balance,
        >>::exchange_inner(&ALICE, exchange_amount, 1)
        .is_ok());
    })
}
