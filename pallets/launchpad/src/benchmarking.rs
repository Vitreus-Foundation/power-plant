//! Benchmarks for pallet-launchpad (§6.3).
//!
//! Everything is derived from the runtime's own constants and live `Params`
//! so the same code benchmarks any configuration. Amounts are computed from
//! the curve: the crossing cost is `raise_at_sellout` plus fee, so a buy of
//! `2 × graduation_target` always crosses and `graduation_target / 10` never
//! does.
//!
//! The seed-bearing paths (`buy_crossing`, `graduate`) are set up at their
//! worst case: no pool exists yet (creation, not adoption), and the pool's
//! sub-account already holds both the quote and the launch token, so
//! `seed_reserved_pool_for` performs both pre-seed sweeps. The excess
//! recipient is funded so it can receive the swept launch token.
//!
//! `graduate` and `force_seed_into_existing_pool` need a launch that is
//! `Complete` but not `Graduated`. On a live chain that state only arises
//! when the deferred seed fails; here it is written directly (curve
//! exhausted, escrow funded with the raise), which reproduces exactly the
//! storage the real path leaves behind.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as Launchpad;
use frame_benchmarking::v2::*;
use frame_support::{traits::EnsureOrigin, BoundedVec};
use frame_system::RawOrigin;

type DexOf<T> = pallet_vitreus_dex::Pallet<T>;

fn bounded<T: Config>(len: u32, byte: u8) -> BoundedVec<u8, T::StringLimit> {
    sp_std::vec![byte; len as usize].try_into().expect("len <= StringLimit")
}

fn target<T: Config>() -> u128 {
    Params::<T>::get().graduation_target.into()
}

/// Enough for creation fees, a crossing buy and change.
fn rich<T: Config>() -> BalanceOf<T> {
    let fee: u128 = Params::<T>::get().creation_fee.into();
    (target::<T>().saturating_mul(10).saturating_add(fee)).into()
}

fn fund<T: Config>(who: &T::AccountId, amount: BalanceOf<T>) {
    T::Currency::set_balance(who, amount);
}

fn funded<T: Config>(name: &'static str) -> T::AccountId {
    let who: T::AccountId = account(name, 0, 0);
    fund::<T>(&who, rich::<T>());
    who
}

fn crossing_quote<T: Config>() -> BalanceOf<T> {
    target::<T>().saturating_mul(2).into()
}

fn small_quote<T: Config>() -> BalanceOf<T> {
    (target::<T>() / 10).into()
}

fn manage_origin<T: Config>() -> Result<T::RuntimeOrigin, BenchmarkError> {
    T::LaunchManageOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)
}

/// Create a launch with 1-byte name/symbol and no initial buy.
fn create<T: Config>(creator: &T::AccountId) -> LaunchId {
    let id = NextLaunchId::<T>::get();
    Launchpad::<T>::create_launch(
        RawOrigin::Signed(creator.clone()).into(),
        bounded::<T>(1, b'N'),
        bounded::<T>(1, b'S'),
        None,
        Zero::zero(),
        Zero::zero(),
        None,
        None,
    )
    .expect("create launch");
    id
}

/// Metadata with a `d`-byte description and four `u`-byte URIs — the two
/// dimensions its storage write varies with.
fn metadata<T: Config>(d: u32, u: u32) -> LaunchMetadataOf<T> {
    let uri = |byte: u8| BoundedVec::try_from(sp_std::vec![byte; u as usize]).expect("u ≤ UriLimit");
    LaunchMetadata {
        image: uri(b'i'),
        description: BoundedVec::try_from(sp_std::vec![b'd'; d as usize]).expect("d ≤ DescriptionLimit"),
        website: uri(b'w'),
        twitter: uri(b'x'),
        telegram: uri(b't'),
    }
}

fn asset_kind<T: Config>(id: LaunchId) -> AssetKindOf<T> {
    T::IntoAssetKind::convert(Launches::<T>::get(id).expect("launch").asset_id)
}

/// Park both assets on the (predictable) pool sub-account so the seed has to
/// sweep them, and make sure the excess recipient can take the launch token.
/// Returns the recipient's balances before the sweep for [`assert_swept`].
fn arm_worst_case_seed<T: Config>(id: LaunchId) -> (BalanceOf<T>, BalanceOf<T>) {
    let launch = Launches::<T>::get(id).expect("launch");
    let pool = DexOf::<T>::pool_account_for(T::NativeAssetKind::get(), asset_kind::<T>(id));
    fund::<T>(&pool, small_quote::<T>());
    T::LaunchAssets::mint_into(launch.asset_id, &pool, small_quote::<T>()).expect("mint to pool");
    let excess = <T as pallet_vitreus_dex::Config>::ExcessRecipient::get();
    fund::<T>(&excess, rich::<T>());
    (T::Currency::balance(&excess), T::LaunchAssets::balance(launch.asset_id, &excess))
}

/// Both sweeps ran: the pool account holds exactly what the seed deposited
/// (had the parked balances been absorbed instead, `sync_reserves` would
/// have counted them and the pool would hold more), and the excess
/// recipient gained at least the parked amounts.
///
/// The recipient side is a lower bound on purpose. In the real runtime
/// `ExcessRecipient` and the launchpad's `Treasury` are the same account,
/// so a crossing buy also pays its protocol fee share there before this
/// runs; an exact equality held in the mock (where they are separate
/// accounts) and trapped the `buy_crossing` benchmark on the testnet
/// runtime.
fn assert_swept<T: Config>(id: LaunchId, before: (BalanceOf<T>, BalanceOf<T>)) {
    let launch = Launches::<T>::get(id).expect("launch");
    let pool = DexOf::<T>::pool_account_for(T::NativeAssetKind::get(), asset_kind::<T>(id));
    let terms = Launchpad::<T>::curve_terms(id).expect("terms");
    let raise: BalanceOf<T> = curve::raise_at_sellout(&terms, T::Sellable::get().into()).expect("raise").into();
    assert_eq!(T::Currency::balance(&pool), raise, "pool holds the raised VTRS and nothing parked");
    assert_eq!(
        T::LaunchAssets::balance(launch.asset_id, &pool),
        Launchpad::<T>::reserved(),
        "pool holds the reserved supply and nothing parked"
    );
    let excess = <T as pallet_vitreus_dex::Config>::ExcessRecipient::get();
    assert!(T::Currency::balance(&excess) >= before.0 + small_quote::<T>());
    assert!(T::LaunchAssets::balance(launch.asset_id, &excess) >= before.1 + small_quote::<T>());
}

/// Write the storage a crossing buy whose seed was deferred leaves behind:
/// curve exhausted, `Complete`, escrow holding the raise.
fn complete_without_seed<T: Config>(id: LaunchId) {
    let launch = Launches::<T>::get(id).expect("launch");
    let terms = Launchpad::<T>::curve_terms(id).expect("terms");
    let raise = curve::raise_at_sellout(&terms, T::Sellable::get().into()).expect("raise");
    let now = frame_system::Pallet::<T>::block_number();
    Curves::<T>::mutate(id, |maybe| {
        let c = maybe.as_mut().expect("curve");
        c.phase = Phase::Complete;
        c.completed_at = Some(now);
        c.tokens_remaining = Zero::zero();
        c.real_quote = raise.into();
    });
    let held: u128 = T::Currency::balance(&launch.escrow).into();
    fund::<T>(&launch.escrow, (held + raise).into());
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn create_launch(
        n: Linear<1, { T::StringLimit::get() }>,
        s: Linear<1, { T::StringLimit::get() }>,
        d: Linear<0, { T::DescriptionLimit::get() }>,
        u: Linear<0, { T::UriLimit::get() }>,
    ) {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());
        let id = NextLaunchId::<T>::get();

        #[extrinsic_call]
        _(
            RawOrigin::Signed(caller.clone()),
            bounded::<T>(n, b'N'),
            bounded::<T>(s, b'S'),
            None,
            Zero::zero(),
            Zero::zero(),
            None,
            Some(metadata::<T>(d, u)),
        );

        assert_eq!(Curves::<T>::get(id).expect("curve").phase, Phase::Trading);
        assert_eq!(NextLaunchId::<T>::get(), id + 1);
        assert_eq!(Metadata::<T>::get(id).expect("metadata").dims(), (d, u));
    }

    #[benchmark]
    fn buy() {
        let creator = funded::<T>("creator");
        let id = create::<T>(&creator);
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());
        let asset = Launches::<T>::get(id).expect("launch").asset_id;

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id, small_quote::<T>(), Zero::zero());

        assert_eq!(Curves::<T>::get(id).expect("curve").phase, Phase::Trading);
        assert!(!T::LaunchAssets::balance(asset, &caller).is_zero());
    }

    /// Partial fill + pool creation + two-asset sweep + seed + permanent lock.
    #[benchmark]
    fn buy_crossing() {
        let creator = funded::<T>("creator");
        let id = create::<T>(&creator);
        let swept = arm_worst_case_seed::<T>(id);
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());

        #[extrinsic_call]
        buy(RawOrigin::Signed(caller.clone()), id, crossing_quote::<T>(), Zero::zero());

        let c = Curves::<T>::get(id).expect("curve");
        assert_eq!(c.phase, Phase::Graduated);
        assert!(!c.lp_shares.is_zero());
        assert_swept::<T>(id, swept);
    }

    #[benchmark]
    fn sell() {
        let creator = funded::<T>("creator");
        let id = create::<T>(&creator);
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());
        Launchpad::<T>::buy(RawOrigin::Signed(caller.clone()).into(), id, small_quote::<T>(), Zero::zero())
            .expect("buy");
        let asset = Launches::<T>::get(id).expect("launch").asset_id;
        let held = T::LaunchAssets::balance(asset, &caller);
        let half = held / 2u32.into();

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id, half, Zero::zero());

        assert_eq!(T::LaunchAssets::balance(asset, &caller), held - half);
    }

    /// Deferred seed on a `Complete` launch, worst-case sweep.
    #[benchmark]
    fn graduate() {
        let creator = funded::<T>("creator");
        let id = create::<T>(&creator);
        complete_without_seed::<T>(id);
        let swept = arm_worst_case_seed::<T>(id);
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id);

        let c = Curves::<T>::get(id).expect("curve");
        assert_eq!(c.phase, Phase::Graduated);
        assert!(c.real_quote.is_zero());
        assert_swept::<T>(id, swept);
    }

    #[benchmark]
    fn claim_creator_fees() -> Result<(), BenchmarkError> {
        // Make sure a buy accrues a non-zero creator share.
        if T::MaxCurveFeeBps::get() == 0 || T::MinProtocolShareBps::get() >= BPS {
            return Err(BenchmarkError::Skip);
        }
        let mut p = Params::<T>::get();
        p.curve_fee_bps = T::MaxCurveFeeBps::get();
        p.protocol_share_bps = T::MinProtocolShareBps::get();
        Launchpad::<T>::set_params(manage_origin::<T>()?, p).expect("set params");

        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());
        let id = create::<T>(&caller);
        let buyer = funded::<T>("buyer");
        Launchpad::<T>::buy(RawOrigin::Signed(buyer).into(), id, small_quote::<T>(), Zero::zero()).expect("buy");
        let owed = Curves::<T>::get(id).expect("curve").creator_fees_unclaimed;
        assert!(!owed.is_zero());
        let before = T::Currency::balance(&caller);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id);

        assert!(Curves::<T>::get(id).expect("curve").creator_fees_unclaimed.is_zero());
        assert_eq!(T::Currency::balance(&caller), before + owed);
        Ok(())
    }

    #[benchmark]
    fn set_launch_metadata(d: Linear<0, { T::DescriptionLimit::get() }>, u: Linear<0, { T::UriLimit::get() }>) {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());
        let id = create::<T>(&caller);
        // Replace an existing record (a write over the largest possible one).
        Metadata::<T>::insert(id, metadata::<T>(T::DescriptionLimit::get(), T::UriLimit::get()));

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id, metadata::<T>(d, u));

        assert_eq!(Metadata::<T>::get(id).expect("metadata").dims(), (d, u));
    }

    #[benchmark]
    fn set_creator_fee_recipient() {
        let caller: T::AccountId = whitelisted_caller();
        fund::<T>(&caller, rich::<T>());
        let id = create::<T>(&caller);
        let new: T::AccountId = account("recipient", 0, 0);

        #[extrinsic_call]
        _(RawOrigin::Signed(caller.clone()), id, new.clone());

        assert_eq!(Launches::<T>::get(id).expect("launch").creator_fee_recipient, new);
    }

    #[benchmark]
    fn set_params() -> Result<(), BenchmarkError> {
        let origin = manage_origin::<T>()?;
        let mut p = Params::<T>::get();
        p.graduation_target = T::MaxGraduationTarget::get();

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, p.clone());

        assert_eq!(Params::<T>::get(), p);
        Ok(())
    }

    #[benchmark]
    fn set_creation_paused() -> Result<(), BenchmarkError> {
        let origin = manage_origin::<T>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, true);

        assert!(CreationPaused::<T>::get());
        Ok(())
    }

    /// Rescue into a pool that already holds liquidity. That pool cannot come
    /// from any DEX call (D2); it is seeded here through the seeder directly,
    /// standing in for the runtime-level bypass the rescue exists for.
    #[benchmark]
    fn force_seed_into_existing_pool() -> Result<(), BenchmarkError> {
        let creator = funded::<T>("creator");
        let id = create::<T>(&creator);
        complete_without_seed::<T>(id);

        let launch = Launches::<T>::get(id).expect("launch");
        let whale = funded::<T>("whale");
        T::LaunchAssets::mint_into(launch.asset_id, &whale, small_quote::<T>()).expect("mint");
        DexOf::<T>::do_seed_reserved_pool_for(
            &whale,
            asset_kind::<T>(id),
            T::NativeAssetKind::get(),
            small_quote::<T>(),
            small_quote::<T>(),
            launch.curve.pool_fee_tier,
        )
        .expect("pre-existing pool");

        let now = frame_system::Pallet::<T>::block_number();
        frame_system::Pallet::<T>::set_block_number(now + T::RescueDelay::get());
        let origin = manage_origin::<T>()?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, id, BPS);

        let c = Curves::<T>::get(id).expect("curve");
        assert_eq!(c.phase, Phase::Graduated);
        assert!(!c.lp_shares.is_zero());
        Ok(())
    }

    impl_benchmark_test_suite!(Launchpad, crate::mock::new_test_ext(), crate::mock::Test);
}
