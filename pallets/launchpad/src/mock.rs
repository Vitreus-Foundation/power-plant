//! Test runtime for pallet-launchpad: the real `pallet_vitreus_dex` (D1–D3),
//! `pallet_assets` with testnet-style signed creation (so FM-14 can squat an
//! id), `pallet_balances`, and utility/proxy/multisig so FM-05 can reach `buy`
//! through every wrapper the runtime offers.

use super::*;
use crate as pallet_launchpad;

use frame_support::{
    construct_runtime, derive_impl, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU16, ConstU32, ConstU64, Contains, InstanceFilter},
    PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};
use std::cell::RefCell;

type Block = frame_system::mocking::MockBlock<Test>;

pub const UNIT: u128 = 1_000_000_000_000_000_000;
pub const ED: u128 = 1_000_000_000_000; // 10^-6 VTRS

/// 32-byte account ids: PalletId sub-accounts must not truncate to a shared
/// prefix ("modl" + "vtrs…" would make every escrow and pool account collide
/// on a u64), and the production chain uses 20-byte ids, so this is the
/// closer model.
pub type Acc = AccountId32;
pub const ALICE: Acc = AccountId32::new([1u8; 32]);
pub const BOB: Acc = AccountId32::new([2u8; 32]);
pub const CHARLIE: Acc = AccountId32::new([3u8; 32]);
pub const DAVE: Acc = AccountId32::new([4u8; 32]);
pub const TREASURY: Acc = AccountId32::new([99u8; 32]);
/// DEX `ExcessRecipient` — deliberately separate from the treasury so a test
/// can make pre-seed sweeps fail (FM-08/FM-11) without breaking fee routing.
pub const EXCESS: Acc = AccountId32::new([98u8; 32]);

/// Launch asset ids start here (spec: 2^64).
pub const ASSET_BASE: u128 = 1u128 << 64;

/// Placeholder governance target `T`; governance-set, value does not affect the math.
pub const T_DEFAULT: u128 = 3_000 * UNIT;
pub const CREATION_FEE: u128 = 10 * ED;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        VitreusDex: pallet_vitreus_dex,
        Utility: pallet_utility,
        Proxy: pallet_proxy,
        Multisig: pallet_multisig,
        Launchpad: pallet_launchpad,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = Acc;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type ExistentialDeposit = ConstU128<ED>;
    type AccountStore = System;
}

parameter_types! {
    pub const AssetDeposit: u128 = 100;
    pub const MetadataDepositBase: u128 = 100;
    pub const MetadataDepositPerByte: u128 = 2;
}

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetId = u128;
    type AssetIdParameter = parity_scale_codec::Compact<u128>;
    type Currency = Balances;
    // Testnet-style: any signed account may create assets, so ids can be squatted (FM-14).
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
    type ForceOrigin = EnsureRoot<Self::AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = ConstU128<10>;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ConstU128<ED>;
    type StringLimit = ConstU32<50>;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type CallbackHandle = ();
    pallet_assets::runtime_benchmarks_enabled! {
        type BenchmarkHelper = AssetsBenchmarkHelper;
    }
}

// `()` only builds `Compact<u32>`; this mock keeps the production `u128` ids.
// Only compiled when `pallet-assets/runtime-benchmarks` is on (feature
// unification can switch it on from a sibling crate in the same `cargo test`).
pallet_assets::runtime_benchmarks_enabled! {
    pub struct AssetsBenchmarkHelper;
    impl pallet_assets::BenchmarkHelper<parity_scale_codec::Compact<u128>> for AssetsBenchmarkHelper {
        fn create_asset_id_parameter(id: u32) -> parity_scale_codec::Compact<u128> {
            parity_scale_codec::Compact(id.into())
        }
    }
}

pub type NativeOrAssetId = frame_support::traits::fungible::NativeOrWithId<u128>;
type NativeAndAssets = frame_support::traits::fungible::UnionOf<
    Balances,
    Assets,
    frame_support::traits::fungible::NativeFromLeft,
    NativeOrAssetId,
    Acc,
>;

parameter_types! {
    pub const NativeAsset: NativeOrAssetId = NativeOrAssetId::Native;
    pub EnergyAsset: NativeOrAssetId = NativeOrAssetId::WithId(0);
    pub const ExcessRecipient: Acc = EXCESS;
}

/// Same rule as the runtime's `LaunchpadReservedAssets`: `[2^64, 2^65)`.
pub struct ReservedAssets;
impl Contains<NativeOrAssetId> for ReservedAssets {
    fn contains(a: &NativeOrAssetId) -> bool {
        matches!(a, NativeOrAssetId::WithId(id) if *id >= ASSET_BASE && *id < (ASSET_BASE << 1))
    }
}

/// D4: the runtime-style adapter that lets the DEX resolve a launch asset's
/// creator fee recipient through the launchpad.
pub struct LaunchpadCreators;
impl pallet_vitreus_dex::CreatorFeeRecipient<NativeOrAssetId, Acc> for LaunchpadCreators {
    fn creator_fee_recipient(asset: &NativeOrAssetId) -> Option<Acc> {
        match asset {
            NativeOrAssetId::WithId(id) => Launchpad::creator_fee_recipient_for(*id),
            NativeOrAssetId::Native => None,
        }
    }
}

impl pallet_vitreus_dex::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<Acc>;
    type Balance = u128;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetKind = NativeOrAssetId;
    type Assets = NativeAndAssets;
    type NativeAsset = NativeAsset;
    type EnergyAsset = EnergyAsset;
    type ReservedAssets = ReservedAssets;
    type ExcessRecipient = ExcessRecipient;
    type DefaultProtocolFeeRecipient = TreasuryAccount;
    type CreatorFeeRecipient = LaunchpadCreators;
    type DefaultBidWindowBlocks = ConstU64<10>;
    type DefaultSettlementWindowBlocks = ConstU64<5>;
    type DefaultSolverBondAmount = ConstU128<1_000_000_000_000>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_utility::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = ();
}

#[derive(
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Default,
)]
pub enum ProxyType {
    #[default]
    Any,
}
impl InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, _c: &RuntimeCall) -> bool {
        true
    }
}

impl pallet_proxy::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ConstU128<ED>;
    type ProxyDepositFactor = ConstU128<ED>;
    type MaxProxies = ConstU32<4>;
    type WeightInfo = ();
    type MaxPending = ConstU32<4>;
    type CallHasher = sp_runtime::traits::BlakeTwo256;
    type AnnouncementDepositBase = ConstU128<ED>;
    type AnnouncementDepositFactor = ConstU128<ED>;
}

impl pallet_multisig::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = ConstU128<ED>;
    type DepositFactor = ConstU128<ED>;
    type MaxSignatories = ConstU32<4>;
    type WeightInfo = ();
}

// ---- launchpad ---------------------------------------------------------

parameter_types! {
    pub const LaunchpadPalletId: PalletId = PalletId(*b"vtrs/lpd");
    pub const TotalSupply: u128 = 1_000_000_000 * UNIT;
    pub const Sellable: u128 = 800_000_000 * UNIT;
    pub const VirtualTokenFloor: u128 = 266_666_667 * UNIT;
    pub const LaunchAssetBase: u128 = ASSET_BASE;
    pub const MinGraduationTarget: u128 = 3 * UNIT;
    pub const MaxGraduationTarget: u128 = 3_000_000_000 * UNIT;
    pub const MinCreationFee: u128 = 2 * ED + 100 + 100 + 2 * 50 * 2; // 2·ED + AssetDeposit + MetadataDepositBase + 2·StringLimit·PerByte
    pub const TreasuryAccount: Acc = TREASURY;
    pub DefaultLaunchParams: LaunchParams<u128> = LaunchParams {
        graduation_target: T_DEFAULT,
        curve_fee_bps: 100,
        protocol_share_bps: 5_000,
        pool_fee_tier: 3,
        creation_fee: CREATION_FEE,
    };
}

pub struct IntoAssetKind;
impl sp_runtime::traits::Convert<u128, NativeOrAssetId> for IntoAssetKind {
    fn convert(id: u128) -> NativeOrAssetId {
        NativeOrAssetId::WithId(id)
    }
}

// Recording buy hook for FM-05 / FM-07. Behaviour is switched per test via
// thread-local state; the default is the v1 pass-through.
/// `(launch_id, launch_created_at, now, who, is_creator, quote_in)` as the hook saw it.
pub type HookCall = (LaunchId, u64, u64, Acc, bool, u128);

thread_local! {
    pub static HOOK_CALLS: RefCell<Vec<HookCall>> = const { RefCell::new(Vec::new()) };
    pub static HOOK_BLACKLIST: RefCell<Option<Acc>> = const { RefCell::new(None) };
    pub static HOOK_REJECT_CREATION_BLOCK: RefCell<bool> = const { RefCell::new(false) };
}

pub struct RecordingHook;
impl OnCurveBuy<Acc, u128, u64> for RecordingHook {
    fn on_buy(
        launch_id: LaunchId,
        launch_created_at: u64,
        now: u64,
        who: &Acc,
        is_creator: bool,
        quote_in: u128,
    ) -> Result<(u128, u128), DispatchError> {
        HOOK_CALLS.with(|c| c.borrow_mut().push((launch_id, launch_created_at, now, who.clone(), is_creator, quote_in)));
        if HOOK_BLACKLIST.with(|b| b.borrow().as_ref() == Some(who)) {
            return Err(DispatchError::Other("hook: blacklisted"));
        }
        if HOOK_REJECT_CREATION_BLOCK.with(|r| *r.borrow()) && now == launch_created_at && !is_creator {
            return Err(DispatchError::Other("hook: not in creation block"));
        }
        Ok((quote_in, 0))
    }
}

impl pallet_launchpad::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type LaunchManageOrigin = EnsureRoot<Acc>;
    type AssetId = u128;
    type Currency = Balances;
    type LaunchAssets = Assets;
    type NativeAssetKind = NativeAsset;
    type IntoAssetKind = IntoAssetKind;
    type Dex = VitreusDex;
    type Treasury = TreasuryAccount;
    type PalletId = LaunchpadPalletId;
    type TotalSupply = TotalSupply;
    type Sellable = Sellable;
    type VirtualTokenFloor = VirtualTokenFloor;
    type LaunchAssetBase = LaunchAssetBase;
    type MinGraduationTarget = MinGraduationTarget;
    type MaxGraduationTarget = MaxGraduationTarget;
    type MaxCurveFeeBps = ConstU16<500>;
    type MinProtocolShareBps = ConstU16<5_000>;
    type MinCreationFee = MinCreationFee;
    type RescueDelay = ConstU64<100_800>;
    type StringLimit = ConstU32<50>;
    type UriLimit = ConstU32<256>;
    type DescriptionLimit = ConstU32<1024>;
    type DefaultLaunchParams = DefaultLaunchParams;
    type BuyHook = RecordingHook;
    type WeightInfo = ();
}

/// 10^9 VTRS each for the actors; enough to cross any in-bounds curve.
pub const RICH: u128 = 1_000_000_000 * UNIT;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, RICH),
            (BOB, RICH),
            (CHARLIE, RICH),
            (DAVE, RICH),
            (TREASURY, ED),
            (EXCESS, ED),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        HOOK_CALLS.with(|c| c.borrow_mut().clear());
        HOOK_BLACKLIST.with(|b| *b.borrow_mut() = None);
        HOOK_REJECT_CREATION_BLOCK.with(|r| *r.borrow_mut() = false);
    });
    ext
}
