//! VitreusDex / Launchpad: testnet only. Indices 210, 211.

use super::*;

parameter_types! {
    pub const DefaultBidWindowBlocks: BlockNumber = 10;
    pub const DefaultSettlementWindowBlocks: BlockNumber = 5;
    pub const DefaultSolverBondAmount: Balance = 1_000 * UNITS;
}

/// Launchpad token ids. `create_pool` cannot touch this range.
pub const LAUNCHPAD_ASSET_ID_START: u128 = 1u128 << 64;
pub const LAUNCHPAD_ASSET_ID_END: u128 = 1u128 << 65;

pub struct LaunchpadReservedAssets;
impl frame_support::traits::Contains<NativeOrAssetId> for LaunchpadReservedAssets {
    fn contains(asset: &NativeOrAssetId) -> bool {
        match asset {
            NativeOrAssetId::WithId(id) => {
                (LAUNCHPAD_ASSET_ID_START..LAUNCHPAD_ASSET_ID_END).contains(id)
            },
            NativeOrAssetId::Native => false,
        }
    }
}

impl pallet_vitreus_dex::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<AccountId>;
    type Balance = Balance;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetKind = NativeOrAssetId;
    type Assets = NativeAndAssets;
    type NativeAsset = NativeAsset;
    type EnergyAsset = VNRG;
    type ReservedAssets = LaunchpadReservedAssets;
    type ExcessRecipient = xcm_config::TreasuryAccount;
    type DefaultProtocolFeeRecipient = xcm_config::TreasuryAccount;
    type CreatorFeeRecipient = LaunchpadCreators;
    // D9: the treasury slice of every swap in a launch token's pool goes to
    // that launch's vault.
    type TreasurySink = LaunchTreasury;
    type DefaultBidWindowBlocks = DefaultBidWindowBlocks;
    type DefaultSettlementWindowBlocks = DefaultSettlementWindowBlocks;
    type DefaultSolverBondAmount = DefaultSolverBondAmount;
    type WeightInfo = pallet_vitreus_dex::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = DexBenchmarkHelper;
}

pub struct LaunchpadCreators;
impl pallet_vitreus_dex::CreatorFeeRecipient<NativeOrAssetId, AccountId> for LaunchpadCreators {
    fn creator_fee_recipient(asset: &NativeOrAssetId) -> Option<AccountId> {
        match asset {
            NativeOrAssetId::WithId(id) => Launchpad::creator_fee_recipient_for(*id),
            NativeOrAssetId::Native => None,
        }
    }
}

/// Launchpad protocol fees follow the DEX recipient.
pub struct DexProtocolFeeRecipient;
impl frame_support::traits::Get<AccountId> for DexProtocolFeeRecipient {
    fn get() -> AccountId {
        VitreusDex::protocol_fee_recipient()
    }
}

/// Plants a launch record so `claim_pool_creator_fees` hits `LaunchpadCreators`.
#[cfg(feature = "runtime-benchmarks")]
pub struct DexBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl pallet_vitreus_dex::BenchmarkHelper<NativeOrAssetId, AccountId> for DexBenchmarkHelper {
    fn asset_kind(seed: u32) -> NativeOrAssetId {
        NativeOrAssetId::WithId(seed.into())
    }

    fn set_creator(asset: &NativeOrAssetId, who: &AccountId) -> bool {
        let NativeOrAssetId::WithId(asset_id) = asset else { return false };
        let id = pallet_launchpad::NextLaunchId::<Runtime>::get();
        pallet_launchpad::Launches::<Runtime>::insert(
            id,
            pallet_launchpad::Launch::<Runtime> {
                asset_id: *asset_id,
                creator: *who,
                creator_fee_recipient: *who,
                escrow: Launchpad::escrow_account(id),
                created_at: System::block_number(),
                curve: pallet_launchpad::CurveParams {
                    graduation_target: 3 * UNITS,
                    virtual_quote: UNITS,
                    curve_fee_bps: 100,
                    protocol_share_bps: 2_500,
                    treasury_share_bps: 2_500,
                    pool_fee_tier: 3,
                },
                params_hash: Default::default(),
            },
        );
        pallet_launchpad::AssetToLaunch::<Runtime>::insert(*asset_id, id);
        pallet_launchpad::NextLaunchId::<Runtime>::put(id + 1);
        true
    }
}

parameter_types! {
    pub const LaunchpadPalletId: PalletId = PalletId(*b"vtrs/lpd");
    pub const LaunchpadTotalSupply: Balance = 1_000_000_000 * UNITS;
    /// 80 % sold on the curve; the remaining 20 % seeds the pool.
    pub const LaunchpadSellable: Balance = 800_000_000 * UNITS;
    /// Virtual token reserve left at sell-out (16× price multiple).
    pub const LaunchpadVirtualTokenFloor: Balance = 266_666_667 * UNITS;
    pub const LaunchpadAssetBase: AssetId = LAUNCHPAD_ASSET_ID_START;
    pub const LaunchpadMinGraduationTarget: Balance = 3 * UNITS;
    pub const LaunchpadMaxGraduationTarget: Balance = 3_000_000_000 * UNITS;
    /// 2·ED + metadata deposits the escrow pays (base + per-byte × name + symbol).
    pub const LaunchpadMinCreationFee: Balance = 2 * EXISTENTIAL_DEPOSIT + 100 + 2 * 50 * 2;
    pub const LaunchpadRescueDelay: BlockNumber = 7 * DAYS;
    /// Stored verbatim, never validated; the cap is the only bound on the write.
    pub const LaunchpadUriLimit: u32 = 256;
    pub const LaunchpadDescriptionLimit: u32 = 1_024;
    /// Default terms; `set_params` changes them for future launches.
    /// Curve fee split creator 50 / protocol 25 / treasury 25
    /// (LAUNCH_TREASURY_SPEC §2.6); existing launches keep their snapshot.
    pub LaunchpadDefaultParams: pallet_launchpad::LaunchParams<Balance> = pallet_launchpad::LaunchParams {
        graduation_target: 3_000 * UNITS,
        curve_fee_bps: 100,
        protocol_share_bps: 2_500,
        treasury_share_bps: 2_500,
        pool_fee_tier: 3,
        creation_fee: 1 * UNITS,
    };
}

pub struct LaunchpadAssetKind;
impl sp_runtime::traits::Convert<AssetId, NativeOrAssetId> for LaunchpadAssetKind {
    fn convert(id: AssetId) -> NativeOrAssetId {
        NativeOrAssetId::WithId(id)
    }
}

impl pallet_launchpad::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LaunchManageOrigin = EnsureRoot<AccountId>;
    type AssetId = AssetId;
    type Currency = Balances;
    type LaunchAssets = Assets;
    type NativeAssetKind = NativeAsset;
    type IntoAssetKind = LaunchpadAssetKind;
    type Dex = VitreusDex;
    type Treasury = DexProtocolFeeRecipient;
    // L1: the treasury share of every curve fee goes to the launch's vault.
    type CurveTreasurySink = LaunchTreasury;
    type PalletId = LaunchpadPalletId;
    type TotalSupply = LaunchpadTotalSupply;
    type Sellable = LaunchpadSellable;
    type VirtualTokenFloor = LaunchpadVirtualTokenFloor;
    type LaunchAssetBase = LaunchpadAssetBase;
    type MinGraduationTarget = LaunchpadMinGraduationTarget;
    type MaxGraduationTarget = LaunchpadMaxGraduationTarget;
    type MaxCurveFeeBps = frame_support::traits::ConstU16<500>;
    type MinProtocolShareBps = frame_support::traits::ConstU16<5_000>;
    type MinCreationFee = LaunchpadMinCreationFee;
    type RescueDelay = LaunchpadRescueDelay;
    type StringLimit = AssetsStringLimit;
    type UriLimit = LaunchpadUriLimit;
    type DescriptionLimit = LaunchpadDescriptionLimit;
    type DefaultLaunchParams = LaunchpadDefaultParams;
    type BuyHook = ();
    type WeightInfo = pallet_launchpad::weights::SubstrateWeight<Runtime>;
}
