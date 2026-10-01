//! Test environment for Energy Broker pallet.

use super::*;
use crate as pallet_energy_broker;

use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{
        tokens::imbalance::ResolveAssetTo, AsEnsureOriginWithArg, ConstU128, ConstU32, Equals,
    },
};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_runtime::{traits::IdentityLookup, BuildStorage, FixedPointNumber, FixedU128};

type Block = frame_system::mocking::MockBlock<Test>;

pub const ALICE: u128 = 1;
pub const BOB: u128 = 2;
pub const INITIAL_BALANCE: u128 = 10000;
pub const INITIAL_ENERGY_BALANCE: u128 = 10000;
pub const INITIAL_ENERGY_CAPACITY: u128 = 2 * INITIAL_ENERGY_BALANCE;

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

pub type NativeOrAssetId = frame_support::traits::fungible::NativeOrWithId<u32>;

type NativeAndAssets = frame_support::traits::fungible::UnionOf<
    Balances,
    Assets,
    frame_support::traits::fungible::NativeFromLeft,
    NativeOrAssetId,
    u128,
>;

parameter_types! {
    pub const VNRG: u32 = 1;
    pub const SNRG: u32 = 2;
    pub const FeeAccount: u128 = 99;
    pub const FeelessAccount: u128 = BOB;
}

const RATE: FixedU128 = FixedU128::from_rational(1, 10);

pub struct MockNativeToEnergyConverter;
impl FixedPathAssetConverter<Test> for MockNativeToEnergyConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::Native;
    const TARGET: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());

    fn get_amount_out(amount_in: u128) -> Option<u128> {
        RATE.reciprocal().map(|x| x.saturating_mul_int(amount_in))
    }

    fn get_amount_in(amount_out: u128) -> Option<u128> {
        Some(RATE.saturating_mul_int(amount_out))
    }
}

pub struct MockEnergyToNativeConverter;
impl FixedPathAssetConverter<Test> for MockEnergyToNativeConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());
    const TARGET: NativeOrAssetId = NativeOrAssetId::Native;

    fn get_amount_out(amount_in: u128) -> Option<u128> {
        Some(RATE.saturating_mul_int(amount_in))
    }

    fn get_amount_in(amount_out: u128) -> Option<u128> {
        RATE.reciprocal().map(|x| x.saturating_mul_int(amount_out))
    }
}

pub struct MockStaticEnergyToEnergyConverter;
impl FixedPathAssetConverter<Test> for MockStaticEnergyToEnergyConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::WithId(SNRG::get());
    const TARGET: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());

    fn swap_fee() -> Option<u32> {
        Some(0)
    }

    fn get_amount_out(amount_in: u128) -> Option<u128> {
        Some(amount_in)
    }

    fn get_amount_in(amount_out: u128) -> Option<u128> {
        Some(amount_out)
    }

    fn reducible_balance(_broker: &u128) -> u128 {
        u128::MAX
    }

    fn withdraw(
        _broker: &u128,
        value: u128,
    ) -> Result<Credit<u128, NativeAndAssets>, DispatchError> {
        Ok(<NativeAndAssets as Balanced<u128>>::issue(Self::TARGET, value))
    }

    fn resolve(
        _broker: &u128,
        credit: Credit<u128, NativeAndAssets>,
    ) -> Result<(), Credit<u128, NativeAndAssets>> {
        drop(credit);
        Ok(())
    }
}

parameter_types! {
    /// Accumulates what `do_swap` reports as an energy sale. Lives in the test externalities, so
    /// every `new_test_ext` starts from zero.
    pub storage RecordedEnergySale: u128 = 0;
}

pub struct RecordEnergySell;
impl OnEnergySell<u128> for RecordEnergySell {
    fn on_energy_sell(amount: u128) {
        RecordedEnergySale::set(&RecordedEnergySale::get().saturating_add(amount));
    }
}

pub(crate) fn energy_sold() -> u128 {
    RecordedEnergySale::get()
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<u128>;
    type Balance = u128;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetKind = NativeOrAssetId;
    type Assets = NativeAndAssets;
    type AssetConverter = (
        MockNativeToEnergyConverter,
        MockEnergyToNativeConverter,
        MockStaticEnergyToEnergyConverter,
    );
    type FeelessAccounts = Equals<FeelessAccount>;
    type SwapFeeTarget = ResolveAssetTo<FeeAccount, Self::Assets>;
    type OnEnergySell = RecordEnergySell;
    type SwapFee = ConstU32<20>; // means 2%
    type EnergyAsset = VNRG;
    type BurnedEnergySessionsCount = ConstU32<2>;
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (EnergyBroker::account_id(), INITIAL_BALANCE),
            (FeeAccount::get(), 10),
            (ALICE, 1000),
            (BOB, 1000),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        assets: vec![(VNRG::get(), 42, false, 20), (SNRG::get(), 42, false, 20)],
        accounts: vec![
            (VNRG::get(), EnergyBroker::account_id(), INITIAL_ENERGY_BALANCE),
            (VNRG::get(), FeeAccount::get(), 20),
            (VNRG::get(), ALICE, 5000),
            (SNRG::get(), ALICE, 5000),
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_energy_broker::GenesisConfig::<Test> { energy_capacity: Some(INITIAL_ENERGY_CAPACITY) }
        .assimilate_storage(&mut t)
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
