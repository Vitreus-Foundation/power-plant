//! Runtime wiring for `pallet-launch-treasury` (index 212, `testnet-runtime` only).
//!
//! Validator-backed treasuries for launch tokens: the DEX's and the launchpad's
//! treasury fee slices reach the pallet's vault through `TreasurySink`, the vault
//! stakes them with validators through `energy-generation` by dispatching its
//! extrinsics as `Signed(vault)` (the pallet has no in-runtime staking trait),
//! sells the LNRG it earns through the energy broker, and buys back and burns the
//! launch token. Design: `pallets/LAUNCH_TREASURY_SPEC.md` in
//! `power-plant-experimental`, where the pallet lives.
//!
//! Everything here is what the pallet needs *of this runtime*: the two adapters,
//! the terms, the benchmark helper, and the one-off upgrade hook that funds the
//! vault. Nothing here is pallet code.

use super::*;
// ---- pallet-launch-treasury -------------------------------------------
//
// Validator-backed treasuries (pallets/LAUNCH_TREASURY_SPEC.md). The
// vault stakes through `energy-generation` by dispatching its extrinsics
// as `Signed(vault)` (spec §3.3: the pallet has no in-runtime staking
// trait) and sells LNRG through the energy broker's `Swap`.

use pallet_launch_treasury::{TreasuryExchange, TreasuryStaking, TreasuryTerms};

parameter_types! {
    pub const LaunchTreasuryPalletId: PalletId = PalletId(*b"vtrs/lpt");
    pub LnrgAssetKind: NativeOrAssetId = NativeOrAssetId::WithId(LNRG::get());
    /// Spec §5.1 defaults. `dormancy_blocks` is snapshotted per treasury.
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

/// `energy-generation` for the vault: the stash is its own controller
/// and payee, every call is the pallet's own `pub fn` dispatched with
/// `RawOrigin::Signed(vault)`, and every error is the pallet's own.
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

/// The energy broker for the vault: an LNRG → VTRS quote and sale at
/// the protocol rate, and the broker's own VTRS as the depth a sale is
/// sized under (spec §6.4). The broker sells with `keep_alive` (R8).
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
