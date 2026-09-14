//! Benchmarks for pallet-vitreus-dex.
//!
//! Assets are created through `fungibles::Create`, the force-create path that
//! does not consult `pallet_assets::CreateOrigin`, so these run unchanged
//! under `mainnet-runtime` (where `CreateOrigin = EnsureNever`). Non-native
//! assets come from [`BenchmarkHelper::asset_kind`]; the runtime binds it to
//! ids far below the launchpad's reserved range.
//!
//! Where an extrinsic has a cheaper and a dearer branch, the dearer one is
//! set up here (see the module docs in `weights.rs`).

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::{settlement::IntentStatus, Pallet as Dex};
use frame_benchmarking::v2::*;
use frame_support::traits::{fungibles::Create, EnsureOrigin, Get};
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedFrom;

/// Supplies non-native asset identifiers for benchmarking. Bound in `Config`
/// under `runtime-benchmarks` only.
pub trait BenchmarkHelper<AssetKind, AccountId> {
    /// A non-native, non-reserved asset kind for `seed`. Distinct seeds must
    /// yield distinct assets.
    fn asset_kind(seed: u32) -> AssetKind;

    /// D4: make `Config::CreatorFeeRecipient` resolve `asset` to `who`, if the
    /// runtime's lookup can be primed (the launchpad-backed runtime plants a
    /// launch record). Returns `false` when it cannot; the
    /// `claim_pool_creator_fees` benchmark then reports `Weightless`.
    fn set_creator(asset: &AssetKind, who: &AccountId) -> bool;
}

impl<AssetId: From<u32> + Ord, AccountId>
    BenchmarkHelper<frame_support::traits::fungible::NativeOrWithId<AssetId>, AccountId> for ()
{
    fn asset_kind(seed: u32) -> frame_support::traits::fungible::NativeOrWithId<AssetId> {
        frame_support::traits::fungible::NativeOrWithId::WithId(seed.into())
    }
    fn set_creator(_: &frame_support::traits::fungible::NativeOrWithId<AssetId>, _: &AccountId) -> bool {
        false
    }
}

const ASSET_A: u32 = 100;
const ASSET_B: u32 = 101;
const FEE_TIER: u32 = 3;

/// 10^24 — comfortably above any runtime's solver bond (≈10^21) and ED.
fn big<T: Config>() -> T::Balance {
    T::Balance::unique_saturated_from(1_000_000_000_000_000_000_000_000u128)
}
/// 10^21 per side of liquidity.
fn liq<T: Config>() -> T::Balance {
    T::Balance::unique_saturated_from(1_000_000_000_000_000_000_000u128)
}
/// 10^18 per trade.
fn trade<T: Config>() -> T::Balance {
    T::Balance::unique_saturated_from(1_000_000_000_000_000_000u128)
}

fn native<T: Config>() -> T::AssetKind {
    T::NativeAsset::get()
}

fn fund_native<T: Config>(who: &T::AccountId) {
    T::Assets::mint_into(native::<T>(), who, big::<T>()).expect("mint native");
}

/// Create asset `seed` (sufficient, min balance 1) and give every holder `big()`.
fn setup_asset<T: Config>(seed: u32, holders: &[&T::AccountId]) -> T::AssetKind
where
    T::Assets: Create<T::AccountId>,
{
    let asset = T::BenchmarkHelper::asset_kind(seed);
    T::Assets::create(asset.clone(), holders[0].clone(), true, One::one()).expect("create asset");
    for h in holders {
        T::Assets::mint_into(asset.clone(), h, big::<T>()).expect("mint asset");
    }
    asset
}

/// Pool (native, ASSET_A) with `liq()` on each side from `provider`.
fn setup_pool<T: Config>(provider: &T::AccountId) -> T::AssetKind
where
    T::Assets: Create<T::AccountId>,
{
    fund_native::<T>(provider);
    let asset = setup_asset::<T>(ASSET_A, &[provider]);
    Dex::<T>::do_create_pool(native::<T>(), asset.clone(), FEE_TIER).expect("create pool");
    Dex::<T>::do_add_liquidity_for(
        provider,
        native::<T>(),
        asset.clone(),
        liq::<T>(),
        liq::<T>(),
        Zero::zero(),
        Zero::zero(),
    )
    .expect("seed liquidity");
    asset
}

fn manage_origin<T: Config>() -> Result<T::RuntimeOrigin, BenchmarkError> {
    T::ManageOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)
}

fn register<T: Config>(who: &T::AccountId) -> u64 {
    fund_native::<T>(who);
    let id = NextSolverId::<T>::get();
    Dex::<T>::register_solver(RawOrigin::Signed(who.clone()).into()).expect("register");
    id
}

/// Intent from `user`: `trade()` of `asset` for native, `min_out` = 1,
/// deadline far in the future. Returns the intent id.
fn submit<T: Config>(user: &T::AccountId, asset: T::AssetKind) -> u64 {
    let id = NextIntentId::<T>::get();
    let deadline = frame_system::Pallet::<T>::block_number() + 1_000u32.into();
    Dex::<T>::submit_intent(
        RawOrigin::Signed(user.clone()).into(),
        asset,
        native::<T>(),
        trade::<T>(),
        One::one(),
        deadline,
    )
    .expect("submit intent");
    id
}

fn advance_blocks<T: Config>(n: BlockNumberFor<T>) {
    let now = frame_system::Pallet::<T>::block_number();
    frame_system::Pallet::<T>::set_block_number(now + n);
}

#[benchmarks(where T::Assets: Create<T::AccountId>)]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn create_pool() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let a = setup_asset::<T>(ASSET_A, &[&caller]);
        let b = setup_asset::<T>(ASSET_B, &[&caller]);
        let origin = manage_origin::<T>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, a.clone(), b.clone(), FEE_TIER);

        assert!(Dex::<T>::pool_exists(a, b));
        Ok(())
    }

    /// Dearer branch: the provider already holds a position in the pool.
    #[benchmark]
    fn add_liquidity() {
        let caller: T::AccountId = whitelisted_caller();
        let asset = setup_pool::<T>(&caller);
        let pair = Dex::<T>::canonical_pair(native::<T>(), asset.clone());
        let before = LiquidityPositions::<T>::get(&caller, &pair).expect("position").shares;

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller.clone()),
            native::<T>(),
            asset,
            liq::<T>(),
            liq::<T>(),
            Zero::zero(),
            Zero::zero(),
        );

        assert!(LiquidityPositions::<T>::get(&caller, &pair).expect("position").shares > before);
    }

    #[benchmark]
    fn remove_liquidity() {
        let caller: T::AccountId = whitelisted_caller();
        let asset = setup_pool::<T>(&caller);
        let pair = Dex::<T>::canonical_pair(native::<T>(), asset.clone());
        let shares = LiquidityPositions::<T>::get(&caller, &pair).expect("position").shares;
        let half = shares / 2u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), native::<T>(), asset, half, Zero::zero(), Zero::zero());

        assert_eq!(LiquidityPositions::<T>::get(&caller, &pair).expect("position").shares, shares - half);
    }

    #[benchmark]
    fn swap_exact_tokens_for_tokens() {
        let provider: T::AccountId = account("provider", 0, 0);
        let asset = setup_pool::<T>(&provider);
        // D4: the dearer branch — a pool that routes both slices, so the swap
        // also transfers to the fee escrow and bumps both counters.
        let pair = Dex::<T>::canonical_pair(native::<T>(), asset.clone());
        Pools::<T>::mutate(&pair, |p| {
            if let Some(p) = p {
                p.routing = FeeRouting { protocol_bps: 5, creator_bps: 5 };
            }
        });
        let caller: T::AccountId = whitelisted_caller();
        fund_native::<T>(&caller);
        T::Assets::mint_into(asset.clone(), &caller, big::<T>()).expect("mint");
        let before = T::Assets::balance(native::<T>(), &caller);

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller.clone()),
            asset,
            native::<T>(),
            trade::<T>(),
            One::one(),
            caller.clone(),
        );

        assert!(T::Assets::balance(native::<T>(), &caller) > before);
        assert!(!CreatorFeesUnclaimed::<T>::get(&pair).is_zero());
        assert!(!ProtocolFeesUnclaimed::<T>::get().is_zero());
    }

    #[benchmark]
    fn lock_liquidity() {
        let caller: T::AccountId = whitelisted_caller();
        let asset = setup_pool::<T>(&caller);
        let pair = Dex::<T>::canonical_pair(native::<T>(), asset.clone());
        let until = frame_system::Pallet::<T>::block_number() + 100u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), native::<T>(), asset, until);

        assert_eq!(LiquidityPositions::<T>::get(&caller, &pair).expect("position").locked_until, Some(until));
    }

    /// Dearer branch: the caller has a prior, inactive registration.
    #[benchmark]
    fn register_solver() {
        let caller: T::AccountId = whitelisted_caller();
        register::<T>(&caller);
        Dex::<T>::deregister_solver(RawOrigin::Signed(caller.clone()).into()).expect("deregister");
        let next = NextSolverId::<T>::get();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()));

        assert_eq!(SolverAccountToId::<T>::get(&caller), Some(next));
        assert!(Solvers::<T>::get(next).expect("solver").active);
    }

    #[benchmark]
    fn deregister_solver() {
        let caller: T::AccountId = whitelisted_caller();
        let id = register::<T>(&caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()));

        assert!(!Solvers::<T>::get(id).expect("solver").active);
    }

    #[benchmark]
    fn submit_intent() {
        let caller: T::AccountId = whitelisted_caller();
        fund_native::<T>(&caller);
        let asset = setup_asset::<T>(ASSET_A, &[&caller]);
        let id = NextIntentId::<T>::get();
        let deadline = frame_system::Pallet::<T>::block_number() + 1_000u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), asset, native::<T>(), trade::<T>(), One::one(), deadline);

        assert_eq!(Intents::<T>::get(id).expect("intent").status, IntentStatus::Open);
    }

    #[benchmark]
    fn cancel_intent() {
        let caller: T::AccountId = whitelisted_caller();
        fund_native::<T>(&caller);
        let asset = setup_asset::<T>(ASSET_A, &[&caller]);
        let id = submit::<T>(&caller, asset);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id);

        assert_eq!(Intents::<T>::get(id).expect("intent").status, IntentStatus::Cancelled);
    }

    /// Dearer branch: a prior commitment from another solver is displaced.
    #[benchmark]
    fn commit_fill() {
        let user: T::AccountId = account("user", 0, 0);
        fund_native::<T>(&user);
        let asset = setup_asset::<T>(ASSET_A, &[&user]);
        let id = submit::<T>(&user, asset);

        let rival: T::AccountId = account("rival", 0, 0);
        register::<T>(&rival);
        Dex::<T>::commit_fill(RawOrigin::Signed(rival).into(), id, One::one()).expect("rival commit");

        let caller: T::AccountId = whitelisted_caller();
        let solver_id = register::<T>(&caller);
        let bid: T::Balance = 2u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id, bid);

        let c = FillCommitments::<T>::get(id).expect("commitment");
        assert_eq!(c.solver_id, solver_id);
        assert_eq!(c.committed_amount_out, bid);
    }

    /// Dearer branch: non-zero profit, so user, treasury and solver are all paid.
    #[benchmark]
    fn settle_intent() {
        let provider: T::AccountId = account("provider", 0, 0);
        let asset = setup_pool::<T>(&provider);

        let user: T::AccountId = account("user", 0, 0);
        fund_native::<T>(&user);
        T::Assets::mint_into(asset.clone(), &user, big::<T>()).expect("mint");
        let id = submit::<T>(&user, asset);

        let caller: T::AccountId = whitelisted_caller();
        let solver_id = register::<T>(&caller);
        Dex::<T>::commit_fill(RawOrigin::Signed(caller.clone()).into(), id, One::one()).expect("commit");
        let before = T::Assets::balance(native::<T>(), &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id);

        assert_eq!(Intents::<T>::get(id).expect("intent").status, IntentStatus::Settled);
        assert!(T::Assets::balance(native::<T>(), &caller) > before);
        assert_eq!(Solvers::<T>::get(solver_id).expect("solver").fills_completed, 1);
    }

    /// Dearer branch: bond split pays both treasury and slasher, and the user is refunded.
    #[benchmark]
    fn slash_solver() {
        let user: T::AccountId = account("user", 0, 0);
        fund_native::<T>(&user);
        let asset = setup_asset::<T>(ASSET_A, &[&user]);
        let id = submit::<T>(&user, asset);

        let solver: T::AccountId = account("solver", 0, 0);
        let solver_id = register::<T>(&solver);
        Dex::<T>::commit_fill(RawOrigin::Signed(solver).into(), id, One::one()).expect("commit");
        advance_blocks::<T>(Dex::<T>::current_settlement_window() + One::one());

        let caller: T::AccountId = whitelisted_caller();
        fund_native::<T>(&caller);
        let before = T::Assets::balance(native::<T>(), &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id);

        assert_eq!(Intents::<T>::get(id).expect("intent").status, IntentStatus::Expired);
        assert!(!Solvers::<T>::get(solver_id).expect("solver").active);
        assert!(T::Assets::balance(native::<T>(), &caller) > before);
    }

    #[benchmark]
    fn refund_expired_intent() {
        let caller: T::AccountId = whitelisted_caller();
        fund_native::<T>(&caller);
        let asset = setup_asset::<T>(ASSET_A, &[&caller]);
        let id = submit::<T>(&caller, asset);
        advance_blocks::<T>(1_000u32.into());

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id);

        assert_eq!(Intents::<T>::get(id).expect("intent").status, IntentStatus::Expired);
        assert!(IntentEscrowBalances::<T>::get(id).is_none());
    }

    #[benchmark]
    fn set_bid_window() -> Result<(), BenchmarkError> {
        let origin = manage_origin::<T>()?;
        let v: BlockNumberFor<T> = 42u32.into();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, v);

        assert_eq!(BidWindowBlocks::<T>::get(), Some(v));
        Ok(())
    }

    #[benchmark]
    fn set_settlement_window() -> Result<(), BenchmarkError> {
        let origin = manage_origin::<T>()?;
        let v: BlockNumberFor<T> = 42u32.into();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, v);

        assert_eq!(SettlementWindowBlocks::<T>::get(), Some(v));
        Ok(())
    }

    #[benchmark]
    fn set_solver_bond_amount() -> Result<(), BenchmarkError> {
        let origin = manage_origin::<T>()?;
        let v = trade::<T>();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, v);

        assert_eq!(SolverBondAmount::<T>::get(), Some(v));
        Ok(())
    }

    // ---- D4 ---------------------------------------------------------------

    #[benchmark]
    fn set_default_fee_routing() -> Result<(), BenchmarkError> {
        let origin = manage_origin::<T>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, 5, 5);

        assert_eq!(DefaultFeeRouting::<T>::get(), FeeRouting { protocol_bps: 5, creator_bps: 5 });
        Ok(())
    }

    #[benchmark]
    fn set_protocol_fee_recipient() -> Result<(), BenchmarkError> {
        let origin = manage_origin::<T>()?;
        let recipient: T::AccountId = account("recipient", 0, 0);

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, Some(recipient.clone()));

        assert_eq!(ProtocolFeeRecipient::<T>::get(), Some(recipient));
        Ok(())
    }

    /// Accrued creator fees sit in the escrow with a counter; the claim's
    /// cost does not depend on how they got there, so they are planted.
    #[benchmark]
    fn claim_pool_creator_fees() -> Result<(), BenchmarkError> {
        let provider: T::AccountId = account("provider", 0, 0);
        let asset = setup_pool::<T>(&provider);
        let caller: T::AccountId = whitelisted_caller();
        if !T::BenchmarkHelper::set_creator(&asset, &caller) {
            return Err(BenchmarkError::Weightless);
        }
        let pair = Dex::<T>::canonical_pair(native::<T>(), asset.clone());
        let escrow = Dex::<T>::fee_escrow_account();
        T::Assets::mint_into(native::<T>(), &escrow, big::<T>()).expect("fund escrow");
        CreatorFeesUnclaimed::<T>::insert(&pair, trade::<T>());
        let before = T::Assets::balance(native::<T>(), &caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), asset);

        assert_eq!(T::Assets::balance(native::<T>(), &caller), before + trade::<T>());
        assert!(CreatorFeesUnclaimed::<T>::get(&pair).is_zero());
        Ok(())
    }

    #[benchmark]
    fn withdraw_protocol_fees() {
        let caller: T::AccountId = whitelisted_caller();
        let escrow = Dex::<T>::fee_escrow_account();
        T::Assets::mint_into(native::<T>(), &escrow, big::<T>()).expect("fund escrow");
        ProtocolFeesUnclaimed::<T>::put(trade::<T>());
        let recipient = Dex::<T>::protocol_fee_recipient();
        let before = T::Assets::balance(native::<T>(), &recipient);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller));

        assert_eq!(T::Assets::balance(native::<T>(), &recipient), before + trade::<T>());
        assert!(ProtocolFeesUnclaimed::<T>::get().is_zero());
    }

    impl_benchmark_test_suite!(Dex, crate::mock::new_test_ext(), crate::mock::Test);
}
