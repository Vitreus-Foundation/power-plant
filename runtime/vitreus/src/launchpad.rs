//! VitreusDex / Launchpad / LaunchTreasury: testnet-only runtime wiring.

use super::*;
use pallet_launch_treasury::{TreasuryExchange, TreasuryStaking, TreasuryTerms};

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
    /// Curve fee split creator 50 / protocol 25 / treasury 25; existing
    /// launches keep their snapshot.
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

parameter_types! {
    pub const LaunchTreasuryPalletId: PalletId = PalletId(*b"vtrs/lpt");
    pub LnrgAssetKind: NativeOrAssetId = NativeOrAssetId::WithId(LNRG::get());
    /// `dormancy_blocks` is snapshotted per treasury.
    pub LaunchTreasuryDefaultTerms: TreasuryTerms<Balance, BlockNumber> = TreasuryTerms {
        dormancy_blocks: 90 * DAYS,
        min_stake: 1 * UNITS,
        max_burn_impact_bps: 50,
        min_burn_interval: 10,
        keeper_bounty_bps: 50,
    };
}

pub struct AssetIdOfKind;
impl sp_runtime::traits::Convert<NativeOrAssetId, Option<AssetId>> for AssetIdOfKind {
    fn convert(kind: NativeOrAssetId) -> Option<AssetId> {
        match kind {
            NativeOrAssetId::WithId(id) => Some(id),
            NativeOrAssetId::Native => None,
        }
    }
}

/// Stakes the vault through `energy-generation`. The pallet has no
/// in-runtime staking trait, so every call is the pallet's own `pub fn`
/// dispatched as `RawOrigin::Signed(vault)`. The stash is its own
/// controller and payee, and every error is the pallet's own.
pub struct EnergyGenerationStaking;
impl EnergyGenerationStaking {
    fn ledger(stash: &AccountId) -> Option<pallet_energy_generation::StakingLedger<Runtime>> {
        pallet_energy_generation::Bonded::<Runtime>::get(stash)
            .and_then(pallet_energy_generation::Ledger::<Runtime>::get)
    }
    fn signed(stash: &AccountId) -> RuntimeOrigin {
        frame_system::RawOrigin::Signed(*stash).into()
    }
}

impl TreasuryStaking<AccountId, Balance> for EnergyGenerationStaking {
    fn is_bonded(stash: &AccountId) -> bool {
        pallet_energy_generation::Bonded::<Runtime>::contains_key(stash)
    }
    fn active(stash: &AccountId) -> Balance {
        Self::ledger(stash).map(|l| l.active).unwrap_or_default()
    }
    fn total(stash: &AccountId) -> Balance {
        Self::ledger(stash).map(|l| l.total).unwrap_or_default()
    }
    fn is_cooperating(stash: &AccountId) -> bool {
        pallet_energy_generation::Cooperators::<Runtime>::contains_key(stash)
    }
    fn cooperated(stash: &AccountId) -> Balance {
        pallet_energy_generation::Cooperators::<Runtime>::get(stash)
            .map(|c| c.targets.values().fold(Balance::zero(), |a, s| a.saturating_add(*s)))
            .unwrap_or_default()
    }
    fn min_cooperator_bond() -> Balance {
        pallet_energy_generation::MinCooperatorBond::<Runtime>::get()
    }
    fn current_era() -> u32 {
        pallet_energy_generation::CurrentEra::<Runtime>::get().unwrap_or(0)
    }
    fn bonding_duration() -> u32 {
        BondingDuration::get()
    }
    fn is_cooperable(validator: &AccountId) -> bool {
        // What `cooperate` checks of a new target, so the vault never
        // submits an all-or-nothing call that one bad target would fail.
        pallet_energy_generation::Validators::<Runtime>::contains_key(validator)
            && pallet_energy_generation::Validators::<Runtime>::get(validator).collaborative
            && EnergyGeneration::is_legit_for_collab(validator)
    }
    fn bond(stash: &AccountId, value: Balance) -> DispatchResult {
        EnergyGeneration::bond(
            Self::signed(stash),
            *stash,
            value,
            pallet_energy_generation::RewardDestination::Account(*stash),
        )
    }
    fn bond_extra(stash: &AccountId, value: Balance) -> DispatchResult {
        EnergyGeneration::bond_extra(Self::signed(stash), value)
    }
    fn cooperate(stash: &AccountId, targets: Vec<(AccountId, Balance)>) -> DispatchResult {
        EnergyGeneration::cooperate(Self::signed(stash), targets)
    }
    fn chill(stash: &AccountId) -> DispatchResult {
        EnergyGeneration::chill(Self::signed(stash))
    }
    fn unbond(stash: &AccountId, value: Balance) -> DispatchResult {
        EnergyGeneration::unbond(Self::signed(stash), value)
            .map(|_| ())
            .map_err(|e| e.error)
    }
    fn withdraw_unbonded(stash: &AccountId) -> Result<Balance, DispatchError> {
        let before = Self::total(stash);
        // `num_slashing_spans` is only checked as an upper bound
        // (`IncorrectSlashingSpans` if it is *below* the real count, when
        // the last chunk leaves and the stash is killed) and otherwise
        // feeds the call's post-dispatch weight, which this caller does
        // not use. A span is one slash on the vault; 256 is not reachable.
        EnergyGeneration::withdraw_unbonded(Self::signed(stash), 256).map_err(|e| e.error)?;
        Ok(before.saturating_sub(Self::total(stash)))
    }
}

/// Sells the vault's LNRG through the energy broker's `Swap`: an
/// LNRG → VTRS quote and sale at the protocol rate, sized under the
/// broker's own VTRS. The broker sells with `keep_alive`.
pub struct EnergyBrokerExchange;
impl TreasuryExchange<AccountId, Balance> for EnergyBrokerExchange {
    fn quote(lnrg: Balance) -> Option<Balance> {
        <EnergyBroker as QuotePrice>::quote_price_exact_tokens_for_tokens(
            LnrgAssetKind::get(),
            NativeOrAssetId::Native,
            lnrg,
            true,
        )
    }
    fn depth() -> Balance {
        use frame_support::traits::{
            fungible::Inspect as _,
            tokens::{Fortitude::Polite, Preservation::Preserve},
        };
        Balances::reducible_balance(&EnergyBroker::account_id(), Preserve, Polite)
    }
    fn sell(who: &AccountId, lnrg: Balance, min_native: Balance) -> Result<Balance, DispatchError> {
        <EnergyBroker as vitreus_runtime_common::Swap<AccountId>>::swap_exact_tokens_for_tokens(
            *who,
            vec![LnrgAssetKind::get(), NativeOrAssetId::Native],
            lnrg,
            Some(min_native),
            *who,
            true,
        )
    }
}

impl pallet_launch_treasury::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TreasuryManageOrigin = EnsureRoot<AccountId>;
    type Staking = EnergyGenerationStaking;
    type Exchange = EnergyBrokerExchange;
    type LnrgAsset = LnrgAssetKind;
    type AssetIdOf = AssetIdOfKind;
    type PalletId = LaunchTreasuryPalletId;
    type MaxTargets = frame_support::traits::ConstU32<16>;
    type MaxUnlockingChunks = MaxUnlockingChunks;
    type DefaultTerms = LaunchTreasuryDefaultTerms;
    type WeightInfo = pallet_launch_treasury::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = LaunchTreasuryBenchmarkHelper;
}

/// What the treasury benchmarks need of `energy-generation`,
/// `pallet-reputation` and `pallet-dynamic-energy`: validators the vault
/// may cooperate with, the cooperator's own reputation, an era clock they
/// can move, and an LNRG rate to quote against.
#[cfg(feature = "runtime-benchmarks")]
pub struct LaunchTreasuryBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl pallet_launch_treasury::BenchmarkHelper<AccountId> for LaunchTreasuryBenchmarkHelper {
    fn cooperable_validator(i: u32) -> AccountId {
        use frame_support::traits::fungible::Mutate as _;
        use pallet_energy_generation::{RewardDestination, ValidatorPrefs};
        let who: AccountId = frame_benchmarking::account("treasury-target", i, 0);
        if pallet_energy_generation::Validators::<Runtime>::contains_key(&who) {
            return who;
        }
        // Bond the larger validator minimum, whatever NAC level the
        // account is given, with the same again free for fees.
        let bond = pallet_energy_generation::MinCommonValidatorBond::<Runtime>::get()
            .max(pallet_energy_generation::MinTrustValidatorBond::<Runtime>::get())
            .max(UNITS);
        Balances::set_balance(&who, bond.saturating_mul(2));
        // Above both the validator and the collaborative tier, whatever
        // `OnNewAccount` granted.
        Reputation::force_set_points(
            frame_system::RawOrigin::Root.into(),
            who.clone(),
            pallet_reputation::ReputationPoint::from(ReputationTier::Trailblazer(1)),
        )
        .expect("root sets reputation");
        EnergyGeneration::bond(
            frame_system::RawOrigin::Signed(who.clone()).into(),
            who.clone(),
            bond,
            RewardDestination::Stash,
        )
        .expect("bond validator");
        EnergyGeneration::validate(
            frame_system::RawOrigin::Signed(who.clone()).into(),
            ValidatorPrefs {
                commission: Perbill::zero(),
                collaborative: true,
                ..Default::default()
            },
        )
        .expect("validate");
        assert!(Self::is_cooperable(&who), "benchmark validator passes the pre-flight filter");
        who
    }
    fn clear_cooperator_gate(vault: &AccountId) {
        // `validate` pins every validator's `min_coop_reputation` at
        // Vanguard(1); give the vault the same tier again explicitly.
        Reputation::force_set_points(
            frame_system::RawOrigin::Root.into(),
            vault.clone(),
            pallet_reputation::ReputationPoint::from(ReputationTier::Vanguard(1)),
        )
        .expect("root sets reputation");
    }
    fn set_current_era(era: u32) {
        pallet_energy_generation::CurrentEra::<Runtime>::put(era);
    }
    fn prepare_exchange() {
        // `DynamicEnergy::ExchangeRate` is `None` until the first session
        // change computes it from the genesis overrides; run that hook.
        <DynamicEnergy as vitreus_runtime_common::OnSessionChange>::on_new_session(1);
        assert!(DynamicEnergy::exchange_rate().is_some(), "benchmark genesis yields an LNRG rate");
    }
    fn set_exchange_depth(native: Balance) {
        use frame_support::traits::fungible::Mutate as _;
        Balances::set_balance(
            &EnergyBroker::account_id(),
            native.saturating_add(ExistentialDeposit::get()),
        );
    }
}
#[cfg(feature = "runtime-benchmarks")]
impl LaunchTreasuryBenchmarkHelper {
    fn is_cooperable(v: &AccountId) -> bool {
        <EnergyGenerationStaking as TreasuryStaking<AccountId, Balance>>::is_cooperable(v)
    }
}

/// Funds the vault with its existential deposit once, from the
/// Treasury, so `OnNewAccount` starts its reputation record at the
/// upgrade block (spec §2.2, §7.4) rather than at the first fee.
pub struct FundLaunchTreasuryVault;
impl frame_support::traits::OnRuntimeUpgrade for FundLaunchTreasuryVault {
    fn on_runtime_upgrade() -> Weight {
        let vault = LaunchTreasury::vault();
        if frame_system::Pallet::<Runtime>::providers(&vault) > 0 {
            // Already holds at least its ED: nothing for the first fee
            // to withhold (`VaultFunded`, pallet §9.6).
            pallet_launch_treasury::VaultFunded::<Runtime>::put(true);
            return <Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1);
        }
        let ed = <Runtime as pallet_balances::Config>::ExistentialDeposit::get();
        let res = <Balances as frame_support::traits::fungible::Mutate<AccountId>>::transfer(
            &xcm_config::TreasuryAccount::get(),
            &vault,
            ed,
            frame_support::traits::tokens::Preservation::Preserve,
        );
        log::info!(target: "runtime::launch-treasury", "vault funded: {:?}", res.map(|_| ()));
        if res.is_ok() {
            pallet_launch_treasury::VaultFunded::<Runtime>::put(true);
        }
        <Runtime as frame_system::Config>::DbWeight::get().reads_writes(3, 4)
    }
}
