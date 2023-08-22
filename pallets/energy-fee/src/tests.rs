//! Tests for the module.

// use frame_support::pallet_prelude::*;
use crate::mock::*;
use crate::Config;
use frame_support::{
    assert_ok,
    dispatch::DispatchInfo,
    traits::Get,
    weights::{Weight, WeightToFee},
};
use frame_system::mocking::MockUncheckedExtrinsic;
use frame_system::weights::{SubstrateWeight as SystemWeight, WeightInfo as _};
use pallet_assets::{weights::SubstrateWeight as AssetsWeight, WeightInfo as _};
use pallet_transaction_payment::OnChargeTransaction;
use parity_scale_codec::Encode;

type Extrinsic = MockUncheckedExtrinsic<Test>;

#[test]
fn weight_to_fee_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            EnergyFee::weight_to_fee(&Weight::from_parts(100_000_000_000, 0)),
            1_000_000_000,
        );
        assert_eq!(EnergyFee::weight_to_fee(&Weight::from_parts(72_000_000, 0)), 1_000_000_000,);
        assert_eq!(
            EnergyFee::weight_to_fee(&Weight::from_parts(210_200_000_000, 0)),
            1_000_000_000,
        );
    });
}

#[test]
fn withdraw_fee_with_stock_coefficients_works() {
    new_test_ext().execute_with(|| {
        let initial_energy_balance: Balance = Assets::balance(VNRG, ALICE);

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
            Assets::balance(VNRG, ALICE),
            initial_energy_balance.saturating_sub(computed_fee),
        );
    });
}

#[test]
fn withdraw_fee_with_custom_coefficients_works() {
    new_test_ext().execute_with(|| {
        let initial_energy_balance: Balance = Assets::balance(VNRG, ALICE);
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

        let constant_fee = <Test as Config>::GetConstantEnergyFee::get();

        assert_eq!(
            Assets::balance(VNRG, ALICE),
            initial_energy_balance.saturating_sub(constant_fee),
        );
    });
}
