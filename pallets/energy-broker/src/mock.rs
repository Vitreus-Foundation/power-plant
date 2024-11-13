//! Test environment for Energy Broker pallet.

use super::*;
use crate as pallet_energy_broker;

use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU32},
};
use frame_system::EnsureSigned;
use sp_arithmetic::{FixedPointNumber, FixedU128};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

pub const ALICE: u128 = 1;
pub const INITIAL_BALANCE: u128 = 10000;
pub const INITIAL_ENERGY_BALANCE: u128 = 10000;

construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        EnergyBroker: pallet_energy_broker,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = u128;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type ExistentialDeposit = ConstU128<10>;
    type AccountStore = System;
}

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetId = u32;
    type AssetIdParameter = u32;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type AssetDeposit = ConstU128<1>;
    type AssetAccountDeposit = ConstU128<10>;
    type MetadataDepositBase = ConstU128<1>;
    type MetadataDepositPerByte = ConstU128<1>;
    type ApprovalDeposit = ConstU128<1>;
    type StringLimit = ConstU32<50>;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type CallbackHandle = ();
    pallet_assets::runtime_benchmarks_enabled! {
        type BenchmarkHelper = ();
    }
}

pub struct AssetRate;
impl AssetRate {
    const RATE: FixedU128 = FixedU128::from_rational(1, 10);
}

impl EnergyBalanceConverter<u128, NativeOrAssetId> for AssetRate {
    fn asset_to_energy_balance(asset_id: NativeOrAssetId, balance: u128) -> Option<u128> {
        match asset_id {
            NativeOrAssetId::Native => {
                Self::RATE.reciprocal().map(|x| x.saturating_mul_int(balance))
            },
            _ => None,
        }
    }

    fn energy_to_asset_balance(asset_id: NativeOrAssetId, balance: u128) -> Option<u128> {
        match asset_id {
            NativeOrAssetId::Native => Some(Self::RATE.saturating_mul_int(balance)),
            _ => None,
        }
    }
}

pub type NativeOrAssetId = frame_support::traits::fungible::NativeOrWithId<u32>;

type NativeAndAssets = frame_support::traits::fungible::UnionOf<
    Balances,
    Assets,
    frame_support::traits::fungible::NativeFromLeft,
    NativeOrAssetId,
    u128,
>;

parameter_types! {
    pub const NativeAsset: NativeOrAssetId = NativeOrAssetId::Native;
    pub const VNRG: u32 = 1;
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetKind = NativeOrAssetId;
    type Assets = NativeAndAssets;
    type BalanceConverter = AssetRate;
    // means 2%
    type SwapFee = ConstU32<20>;
    type NativeAsset = NativeAsset;
    type EnergyAsset = VNRG;
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(EnergyBroker::account_id(), INITIAL_BALANCE), (ALICE, 1000)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        assets: vec![(VNRG::get(), 42, false, 20)],
        accounts: vec![
            (VNRG::get(), EnergyBroker::account_id(), INITIAL_ENERGY_BALANCE),
            (VNRG::get(), ALICE, 5000),
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
