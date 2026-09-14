//! Test environment for the Vitreus DEX pallet.

use super::*;
use crate as pallet_vitreus_dex;

use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64},
};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

pub const ALICE: u128 = 1;
pub const BOB: u128 = 2;
pub const CHARLIE: u128 = 3;

pub const USDC_ID: u32 = 1;
pub const VNRG_ID: u32 = 2;
/// Account that receives pre-seed sweeps (stands in for the treasury).
pub const TREASURY: u128 = 99;
/// Asset ids at or above this are "reserved" in the mock (launchpad range).
pub const RESERVED_ASSET_BASE: u32 = 1_000;

construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        VitreusDex: pallet_vitreus_dex,
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
    type ExistentialDeposit = ConstU128<1>;
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
    type ForceOrigin = EnsureRoot<Self::AccountId>;
    type AssetDeposit = ConstU128<0>;
    type AssetAccountDeposit = ConstU128<0>;
    type MetadataDepositBase = ConstU128<0>;
    type MetadataDepositPerByte = ConstU128<0>;
    type ApprovalDeposit = ConstU128<0>;
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
    pub const NativeAsset: NativeOrAssetId = NativeOrAssetId::Native;
    pub const USDC: u32 = USDC_ID;
    pub const VNRG: u32 = VNRG_ID;
    pub EnergyAsset: NativeOrAssetId = NativeOrAssetId::WithId(VNRG_ID);
    pub const ExcessRecipient: u128 = TREASURY;
}

/// D4: the creator on record for a reserved asset in tests (stands in for
/// the launchpad's `creator_fee_recipient`). `LAUNCH_ID` → `CREATOR`;
/// everything else has no creator.
pub const CREATOR: u128 = 77;
pub const LAUNCH_ID: u32 = RESERVED_ASSET_BASE + 1;
pub struct MockCreators;
impl crate::CreatorFeeRecipient<NativeOrAssetId, u128> for MockCreators {
    fn creator_fee_recipient(asset: &NativeOrAssetId) -> Option<u128> {
        match asset {
            NativeOrAssetId::WithId(id) if *id == LAUNCH_ID => Some(CREATOR),
            NativeOrAssetId::WithId(id) => PRIMED_CREATORS.with(|c| c.borrow().get(id).copied()),
            NativeOrAssetId::Native => None,
        }
    }
}

thread_local! {
    /// Creators primed by `MockBenchHelper::set_creator` (benchmarks only).
    static PRIMED_CREATORS: std::cell::RefCell<std::collections::BTreeMap<u32, u128>> =
        std::cell::RefCell::new(std::collections::BTreeMap::new());
}

/// Benchmark helper for the mock: `WithId(seed)` assets, and creators primed
/// through the thread-local `MockCreators` consults.
#[cfg(feature = "runtime-benchmarks")]
pub struct MockBenchHelper;
#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<NativeOrAssetId, u128> for MockBenchHelper {
    fn asset_kind(seed: u32) -> NativeOrAssetId {
        NativeOrAssetId::WithId(seed)
    }
    fn set_creator(asset: &NativeOrAssetId, who: &u128) -> bool {
        match asset {
            NativeOrAssetId::WithId(id) => {
                PRIMED_CREATORS.with(|c| c.borrow_mut().insert(*id, *who));
                true
            },
            NativeOrAssetId::Native => false,
        }
    }
}

/// Mock reserved range: every `WithId(id)` with `id >= RESERVED_ASSET_BASE`.
pub struct ReservedAssets;
impl frame_support::traits::Contains<NativeOrAssetId> for ReservedAssets {
    fn contains(a: &NativeOrAssetId) -> bool {
        matches!(a, NativeOrAssetId::WithId(id) if *id >= RESERVED_ASSET_BASE)
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<u128>;
    type Balance = u128;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetKind = NativeOrAssetId;
    type Assets = NativeAndAssets;
    type NativeAsset = NativeAsset;
    type EnergyAsset = EnergyAsset;
    type ReservedAssets = ReservedAssets;
    type ExcessRecipient = ExcessRecipient;
    type DefaultProtocolFeeRecipient = ExcessRecipient;
    type CreatorFeeRecipient = MockCreators;
    type DefaultBidWindowBlocks = ConstU64<10>;
    type DefaultSettlementWindowBlocks = ConstU64<5>;
    type DefaultSolverBondAmount = ConstU128<1_000_000_000_000>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = MockBenchHelper;
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> {
        // 10× the default solver bond (1e12) so tests can run register +
        // deregister + re-register flows without hitting ED or balance limits.
        balances: vec![
            (ALICE, 10_000_000_000_000),
            (BOB, 10_000_000_000_000),
            (CHARLIE, 10_000_000_000_000),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        assets: vec![(USDC_ID, ALICE, true, 1), (VNRG_ID, ALICE, true, 1)],
        accounts: vec![
            (USDC_ID, ALICE, 1_000_000),
            (USDC_ID, BOB, 1_000_000),
            (USDC_ID, CHARLIE, 1_000_000),
            (VNRG_ID, ALICE, 1_000_000),
            (VNRG_ID, BOB, 1_000_000),
            (VNRG_ID, CHARLIE, 1_000_000),
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
