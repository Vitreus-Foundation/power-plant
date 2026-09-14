//! Tests for pallet-launchpad, named as LAUNCHPAD_SPEC.md §6 names them.
//!
//! Property-style tests use an in-test xorshift generator (no `proptest` in
//! the workspace); they are deterministic and run a few thousand operations.

use crate::{
    curve::{self, MAX_TRADE_IN},
    mock::*,
    AssetToLaunch, Curves, Error, Event, LaunchId, LaunchParams, Launches, NextLaunchId, Params, Phase,
};
use frame_support::{assert_noop, assert_ok};
use pallet_vitreus_dex::{LiquidityPositions, PoolInfo, PoolManager, Pools, TotalLiquidity, MINIMUM_LIQUIDITY};
use sp_core::U256;
use sp_runtime::{traits::BadOrigin, DispatchError};
use std::borrow::Borrow;

// ---- constants & helpers ----------------------------------------------

const SELLABLE: u128 = 800_000_000 * UNIT;
const RESERVED: u128 = 200_000_000 * UNIT;
const RESCUE_DELAY: u64 = 100_800;

fn native() -> NativeOrAssetId {
    NativeOrAssetId::Native
}
fn asset_of(id: LaunchId) -> u128 {
    ASSET_BASE + id as u128
}
fn kind(id: LaunchId) -> NativeOrAssetId {
    NativeOrAssetId::WithId(asset_of(id))
}
fn escrow(id: LaunchId) -> Acc {
    Launchpad::escrow_account(id)
}
fn pair(id: LaunchId) -> (NativeOrAssetId, NativeOrAssetId) {
    VitreusDex::canonical_pair(native(), kind(id))
}
fn pool_account(id: LaunchId) -> Acc {
    VitreusDex::pool_account_for(native(), kind(id))
}
fn bv(s: &[u8]) -> frame_support::BoundedVec<u8, frame_support::traits::ConstU32<50>> {
    s.to_vec().try_into().unwrap()
}
fn state(id: LaunchId) -> crate::CurveState<Test> {
    Curves::<Test>::get(id).expect("curve")
}
fn launch(id: LaunchId) -> crate::Launch<Test> {
    Launches::<Test>::get(id).expect("launch")
}
fn vtrs(who: impl Borrow<Acc>) -> u128 {
    Balances::free_balance(who.borrow())
}
fn tok(id: LaunchId, who: impl Borrow<Acc>) -> u128 {
    Assets::balance(asset_of(id), who.borrow())
}
fn origin(who: impl Borrow<Acc>) -> RuntimeOrigin {
    RuntimeOrigin::signed(who.borrow().clone())
}

/// Create a launch with default terms and no initial buy; returns its id.
fn create(creator: impl Borrow<Acc>) -> LaunchId {
    let id = NextLaunchId::<Test>::get();
    assert_ok!(Launchpad::create_launch(origin(creator), bv(b"Meme"), bv(b"MEME"), None, 0, 0, None, None));
    id
}
fn buy(who: impl Borrow<Acc>, id: LaunchId, q: u128) {
    assert_ok!(Launchpad::buy(origin(who.borrow()), id, q, 0));
}
fn sell(who: impl Borrow<Acc>, id: LaunchId, t: u128) {
    assert_ok!(Launchpad::sell(origin(who.borrow()), id, t, 0));
}
/// Buy enough to exhaust the curve. Returns the buyer's VTRS spent.
fn cross(who: impl Borrow<Acc>, id: LaunchId) -> u128 {
    let who = who.borrow();
    let before = vtrs(who);
    buy(who, id, 500_000_000 * UNIT);
    before - vtrs(who)
}
fn terms(id: LaunchId) -> curve::Terms {
    Launchpad::curve_terms(id).unwrap()
}
fn p_end(id: LaunchId) -> (u128, u128) {
    // marginal price at sell-out as a ratio (num, den): (V_q + R) / VT_FLOOR
    let t = terms(id);
    let r = curve::raise_at_sellout(&t, SELLABLE).unwrap();
    (t.virtual_quote + r, t.token_floor)
}
fn deferred_error() -> Option<DispatchError> {
    System::events().iter().rev().find_map(|r| match &r.event {
        RuntimeEvent::Launchpad(Event::GraduationDeferred { error, .. }) => Some(*error),
        _ => None,
    })
}
fn has_event(f: impl Fn(&Event<Test>) -> bool) -> bool {
    System::events().iter().any(|r| matches!(&r.event, RuntimeEvent::Launchpad(e) if f(e)))
}
fn dex_err(e: pallet_vitreus_dex::Error<Test>) -> DispatchError {
    e.into()
}
fn set_params(t: u128, fee: u16, share: u16) {
    assert_ok!(Launchpad::set_params(
        RuntimeOrigin::root(),
        LaunchParams { graduation_target: t, curve_fee_bps: fee, protocol_share_bps: share, pool_fee_tier: 3, creation_fee: CREATION_FEE }
    ));
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// I1, I2, I3 (over the given holders), I5, I6 — the storage-level invariants.
fn check_invariants(id: LaunchId, holders: &[Acc]) {
    let s = state(id);
    let l = launch(id);
    let t = terms(id);
    // I1: escrow VTRS ≥ ED + real_quote + unclaimed (deposits are reserved, not free)
    assert!(
        vtrs(&l.escrow) >= ED + s.real_quote + s.creator_fees_unclaimed,
        "I1: escrow {} < ED + {} + {}",
        vtrs(&l.escrow),
        s.real_quote,
        s.creator_fees_unclaimed
    );
    match s.phase {
        Phase::Trading | Phase::Complete => {
            // I2
            assert!(tok(id, &l.escrow) >= s.tokens_remaining + RESERVED, "I2");
            // I3: holders' tokens == sold
            let held: u128 = holders.iter().map(|h| tok(id, h)).sum();
            assert_eq!(held, SELLABLE - s.tokens_remaining, "I3");
            // I6
            assert_eq!(s.phase == Phase::Complete, s.tokens_remaining == 0 && s.graduated_at.is_none(), "I6");
        },
        Phase::Graduated => {
            assert_eq!(s.tokens_remaining, 0, "I6 graduated");
            assert_eq!(s.real_quote, 0, "I6 graduated real_quote");
            assert!(s.lp_shares > 0, "I6 lp_shares");
        },
    }
    // I5
    let q_max = curve::raise_at_sellout(&t, SELLABLE).unwrap();
    assert!(s.real_quote <= q_max, "I5 real_quote {} > {}", s.real_quote, q_max);
    assert!(s.tokens_remaining <= SELLABLE, "I5 tokens_remaining");
}

// =======================================================================
// Lifecycle
// =======================================================================

#[test]
fn lifecycle_happy_path() {
    new_test_ext().execute_with(|| {
        let treasury_before = vtrs(TREASURY);
        let id = NextLaunchId::<Test>::get();
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"Meme"), bv(b"MEME"), None, 5 * UNIT, 0, None, None));
        let l = launch(id);
        assert_eq!(l.asset_id, asset_of(id));
        assert_eq!(Assets::total_supply(asset_of(id)), 1_000_000_000 * UNIT);
        // creation fee: ED kept free, metadata deposits reserved, the rest to treasury;
        // the initial buy then adds its net + creator fee to the escrow
        let s0 = state(id);
        assert_eq!(vtrs(&l.escrow), ED + s0.real_quote + s0.creator_fees_unclaimed);
        assert_eq!(Balances::reserved_balance(&l.escrow), 100 + 2 * 4 + 2 * 4);
        assert!(tok(id, ALICE) > 0);

        let buyers = [BOB, CHARLIE, DAVE];
        for (i, b) in buyers.iter().cycle().take(10).enumerate() {
            buy(b, id, (i as u128 + 1) * 10 * UNIT);
        }
        for b in buyers.iter() {
            let held = tok(id, b);
            sell(b, id, held / 3);
        }
        check_invariants(id, &[ALICE, BOB, CHARLIE, DAVE]);
        let protocol_paid_before = state(id).protocol_fees_paid;

        let spent = cross(BOB, id);
        assert!(spent > 0);
        let s = state(id);
        assert_eq!(s.phase, Phase::Graduated);
        assert_eq!(s.tokens_remaining, 0);
        assert_eq!(s.real_quote, 0);
        assert!(s.lp_shares > 0);
        assert!(s.protocol_fees_paid > protocol_paid_before);

        // I7: pool opens at the curve's final price within 1e-6.
        let pool = Pools::<Test>::get(pair(id)).unwrap();
        let (quote_seeded, tokens_seeded) = (pool.reserve_a, pool.reserve_b); // canonical: Native first
        assert_eq!(tokens_seeded, RESERVED);
        let (pn, pd) = p_end(id);
        // |quote/tokens − pn/pd| / (pn/pd) ≤ 1e-6  ⇔  |quote·pd − tokens·pn| · 1e6 ≤ tokens·pn
        let lhs = U256::from(quote_seeded) * U256::from(pd);
        let rhs = U256::from(tokens_seeded) * U256::from(pn);
        let diff = if lhs > rhs { lhs - rhs } else { rhs - lhs };
        assert!(diff * U256::from(1_000_000u32) <= rhs, "I7 opening price off by > 1e-6");
        // raised ≈ T
        let t = T_DEFAULT;
        assert!(quote_seeded <= t && t - quote_seeded <= t / 100_000_000, "raise {} vs T {}", quote_seeded, t);

        // Position: escrow, locked forever.
        let pos = LiquidityPositions::<Test>::get(&l.escrow, pair(id)).unwrap();
        assert_eq!(pos.locked_until, Some(u64::MAX));
        assert_eq!(pos.shares, s.lp_shares);

        // Treasury got creation-fee surplus + protocol fees.
        let s = state(id);
        assert_eq!(vtrs(TREASURY) - treasury_before, s.protocol_fees_paid + (CREATION_FEE - ED - (100 + 2 * 4 + 2 * 4)));
        assert!(has_event(|e| matches!(e, Event::Graduated { .. })));
    });
}

#[test]
fn lifecycle_initial_buy_completes_curve() {
    new_test_ext().execute_with(|| {
        let id = NextLaunchId::<Test>::get();
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"Meme"), bv(b"MEME"), None, 100_000 * UNIT, 0, None, None));
        assert_eq!(state(id).phase, Phase::Graduated);
        assert_eq!(tok(id, ALICE), SELLABLE);
        assert!(VitreusDex::pool_exists(native(), kind(id)));
    });
}

#[test]
fn inv_try_state_holds_after_random_sequences() {
    new_test_ext().execute_with(|| {
        let ids: Vec<LaunchId> = (0..3).map(|_| create(ALICE)).collect();
        let holders = [ALICE, BOB, CHARLIE, DAVE];
        let mut rng = Rng(0xDEADBEEFCAFEF00D);
        for step in 0..600 {
            let id = ids[(rng.next() % 3) as usize];
            let who = holders[(rng.next() % 4) as usize].clone();
            let r = rng.next();
            match r % 7 {
                0..=2 => {
                    let amt = (r as u128 % 2_000) * UNIT + 1;
                    let _ = Launchpad::buy(origin(&who), id, amt, 0);
                },
                3 | 4 => {
                    let held = tok(id, &who);
                    if held > 0 {
                        let _ = Launchpad::sell(origin(&who), id, (r as u128 % held).max(1), 0);
                    }
                },
                5 => {
                    // donation to escrow: must be inert
                    assert_ok!(Balances::transfer_allow_death(origin(&who), escrow(id), UNIT));
                },
                _ => {
                    let _ = Launchpad::claim_creator_fees(origin(ALICE), id);
                    if step % 50 == 0 {
                        set_params(T_DEFAULT * 2, 200, 6_000);
                    }
                },
            }
            for id in &ids {
                check_invariants(*id, &holders);
            }
        }
    });
}

// =======================================================================
// FM-01 / FM-02 — pool creation and pre-seed donations
// =======================================================================

#[test]
fn fm01_only_launchpad_can_seed_reserved_pool() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 10 * UNIT);
        // (a) root cannot create the pool for the launch asset
        assert_noop!(
            VitreusDex::create_pool(RuntimeOrigin::root(), kind(id), native(), 3),
            pallet_vitreus_dex::Error::<Test>::ReservedAsset
        );
        // (b) nobody can add liquidity: there is no pool
        assert_noop!(
            VitreusDex::add_liquidity(origin(BOB), native(), kind(id), UNIT, tok(id, BOB), 0, 0),
            pallet_vitreus_dex::Error::<Test>::PoolNotFound
        );
        assert!(!VitreusDex::pool_exists(native(), kind(id)));

        cross(BOB, id);
        assert!(VitreusDex::pool_exists(native(), kind(id)));
        // Post-graduation, ordinary LPs may join through the normal path.
        assert_ok!(VitreusDex::add_liquidity(origin(BOB), native(), kind(id), UNIT, tok(id, BOB), 0, 0));
    });
}

#[test]
fn fm01_seed_into_existing_liquid_pool_is_rejected() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 10 * UNIT);
        // The guard makes this unreachable through any call: plant the pool
        // record directly, then add liquidity at ~10^4× p_end (1 VTRS per 1000 tokens).
        Pools::<Test>::insert(
            pair(id),
            PoolInfo { reserve_a: 0, reserve_b: 0, fee_tier: 3, total_fees_collected: 0, routing: pallet_vitreus_dex::FeeRouting::default(),
                pool_account: pool_account(id) },
        );
        TotalLiquidity::<Test>::insert(pair(id), 0u128);
        assert_ok!(VitreusDex::do_add_liquidity_for(&BOB, native(), kind(id), UNIT, 1_000 * UNIT, 0, 0));

        let escrow_q = vtrs(escrow(id));
        let escrow_t = tok(id, escrow(id));
        let spent = cross(BOB, id); // buy succeeds ...
        assert!(spent > 0);
        assert_eq!(state(id).phase, Phase::Complete); // ... graduation deferred
        assert_eq!(deferred_error(), Some(dex_err(pallet_vitreus_dex::Error::<Test>::PoolAlreadySeeded)));
        assert!(vtrs(escrow(id)) > escrow_q); // the raise is still in escrow
        assert_eq!(tok(id, escrow(id)), RESERVED); // sellable exhausted, reserved still here
        let _ = escrow_t;

        assert_noop!(Launchpad::graduate(origin(CHARLIE), id), dex_err(pallet_vitreus_dex::Error::<Test>::PoolAlreadySeeded));
        assert_noop!(Launchpad::force_seed_into_existing_pool(RuntimeOrigin::root(), id, 100), Error::<Test>::RescueNotDue);
        System::set_block_number(System::block_number() + RESCUE_DELAY);
        assert_noop!(Launchpad::force_seed_into_existing_pool(RuntimeOrigin::root(), id, 100), Error::<Test>::PriceOutOfTolerance);
        assert_eq!(state(id).phase, Phase::Complete);
        assert_eq!(tok(id, escrow(id)), RESERVED);
    });
}

#[test]
fn fm02_prefunded_pool_account_does_not_move_opening_price() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 1_000 * UNIT);
        let pa = pool_account(id);
        // Park native then tokens at the predictable pool address.
        assert_ok!(Balances::transfer_allow_death(origin(BOB), pa.clone(), 5 * UNIT));
        let donated_tokens = tok(id, BOB) / 2;
        assert_ok!(Assets::transfer(origin(BOB), asset_of(id).into(), pa.clone(), donated_tokens));
        let excess_before = (vtrs(EXCESS), tok(id, EXCESS));

        cross(CHARLIE, id);
        assert_eq!(state(id).phase, Phase::Graduated);
        let pool = Pools::<Test>::get(pair(id)).unwrap();
        let raised = pool.reserve_a;
        assert_eq!(pool.reserve_b, RESERVED);
        // Live balances equal the stored amounts: nothing extra in the pool.
        assert_eq!(vtrs(&pa), raised);
        assert_eq!(tok(id, &pa), RESERVED);
        // Donations went to the DEX excess recipient.
        assert_eq!(vtrs(EXCESS) - excess_before.0, 5 * UNIT);
        assert_eq!(tok(id, EXCESS) - excess_before.1, donated_tokens);
        // First swap prices off the stored reserves (sync_reserves finds nothing extra).
        let amount_in = 10 * UNIT;
        let after_fee = amount_in - amount_in * 3 / 1_000;
        let expected = u128::try_from(U256::from(RESERVED) * U256::from(after_fee) / U256::from(raised + after_fee)).unwrap();
        let before = tok(id, DAVE);
        assert_ok!(VitreusDex::swap_exact_tokens_for_tokens(origin(DAVE), native(), kind(id), amount_in, expected, DAVE));
        assert_eq!(tok(id, DAVE) - before, expected);
    });
}

// =======================================================================
// FM-03 / FM-04 — custody
// =======================================================================

#[test]
fn fm03_no_path_moves_escrow_funds_except_curve_and_seed() {
    new_test_ext().execute_with(|| {
        // No withdraw-shaped dispatchable exists.
        let names = Launchpad::call_names();
        for forbidden in ["force_withdraw", "force_refund", "force_cancel", "force_set_phase", "force_mint", "withdraw", "refund"] {
            assert!(!names.iter().any(|n| n.contains(forbidden)), "found {forbidden}");
        }
        assert_eq!(names.len(), 10, "a new dispatchable was added; extend this test's call list");

        let trading = create(ALICE);
        buy(BOB, trading, 100 * UNIT);
        let graduated = create(ALICE);
        cross(BOB, graduated);

        for id in [trading, graduated] {
            let e = escrow(id);
            let snapshot = || (vtrs(&e), tok(id, &e), Balances::reserved_balance(&e));
            let before = snapshot();
            // Every dispatchable, with root and with a signed origin that is
            // neither creator nor recipient, with arguments that make the
            // fund-moving calls no-ops or errors.
            let calls: Vec<RuntimeCall> = vec![
                RuntimeCall::Launchpad(crate::Call::create_launch { name: bv(b"X"), symbol: bv(b"X"), creator_fee_recipient: None, initial_buy: 0, min_tokens_out: 0, expected_params_hash: None, metadata: None }),
                RuntimeCall::Launchpad(crate::Call::buy { launch_id: id, quote_in: 0, min_tokens_out: 0 }),
                RuntimeCall::Launchpad(crate::Call::sell { launch_id: id, tokens_in: 0, min_quote_out: 0 }),
                RuntimeCall::Launchpad(crate::Call::graduate { launch_id: id }),
                RuntimeCall::Launchpad(crate::Call::claim_creator_fees { launch_id: id }),
                RuntimeCall::Launchpad(crate::Call::set_creator_fee_recipient { launch_id: id, new: CHARLIE }),
                RuntimeCall::Launchpad(crate::Call::set_params { new: Params::<Test>::get() }),
                RuntimeCall::Launchpad(crate::Call::set_creation_paused { paused: false }),
                RuntimeCall::Launchpad(crate::Call::force_seed_into_existing_pool { launch_id: id, max_price_deviation_bps: 10_000 }),
                RuntimeCall::Launchpad(crate::Call::set_launch_metadata { launch_id: id, metadata: meta(b"x", b"y") }),
            ];
            assert_eq!(calls.len(), names.len());
            for call in calls {
                use sp_runtime::traits::Dispatchable;
                for o in [RuntimeOrigin::root(), origin(CHARLIE)] {
                    let _ = call.clone().dispatch(o);
                    assert_eq!(snapshot(), before, "{call:?} moved escrow funds");
                }
            }
            // pallet_assets::force_transfer is admin-gated; the admin is the
            // escrow itself, which has no key. An outsider gets NoPermission.
            assert!(Assets::force_transfer(origin(CHARLIE), asset_of(id).into(), e.clone(), CHARLIE, 1).is_err());
            assert_eq!(snapshot(), before);
        }
    });
}

#[test]
fn fm03_flash_style_complete_then_extract() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(CHARLIE, id, 1_000 * UNIT);
        let start = vtrs(BOB);
        let spent = cross(BOB, id); // completes and seeds in one call
        assert_eq!(state(id).phase, Phase::Graduated);
        assert_noop!(Launchpad::sell(origin(BOB), id, 1, 0), Error::<Test>::WrongPhase);
        assert_noop!(Launchpad::claim_creator_fees(origin(BOB), id), Error::<Test>::NotFeeRecipient);
        assert_noop!(Launchpad::graduate(origin(BOB), id), Error::<Test>::WrongPhase);
        assert_noop!(Launchpad::force_seed_into_existing_pool(RuntimeOrigin::root(), id, 0), Error::<Test>::WrongPhase);
        // The only way back to VTRS is the pool, at p_end with price impact.
        let held = tok(id, BOB);
        assert_ok!(VitreusDex::swap_exact_tokens_for_tokens(origin(BOB), kind(id), native(), held, 0, BOB));
        assert!(vtrs(BOB) < start, "extracted more than spent");
        assert!(start - vtrs(BOB) > spent / 100, "round trip cost < 1% of spend — check fee + impact");
    });
}

#[test]
fn fm04_donation_to_escrow_is_inert() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 900 * UNIT); // ~30 %
        let q_before = Launchpad::quote_buy(id, UNIT).unwrap();
        let s_before = state(id);
        let donated_tokens = tok(id, BOB) / 10;
        assert_ok!(Balances::transfer_allow_death(origin(CHARLIE), escrow(id), 100 * UNIT));
        assert_ok!(Assets::transfer(origin(BOB), asset_of(id).into(), escrow(id), donated_tokens));
        // pricing and tracked state are untouched by the donation
        assert_eq!(Launchpad::quote_buy(id, UNIT).unwrap(), q_before);
        assert_eq!(state(id), s_before);
        // I2 holds as ≥: escrow tokens = tracked + reserved + donation
        assert_eq!(tok(id, escrow(id)), s_before.tokens_remaining + RESERVED + donated_tokens);
        cross(DAVE, id);
        let pool = Pools::<Test>::get(pair(id)).unwrap();
        assert_eq!(pool.reserve_b, RESERVED);
        // Donations never entered the pool: they are still in escrow.
        assert!(vtrs(escrow(id)) >= 100 * UNIT + ED + state(id).creator_fees_unclaimed);
        assert_eq!(tok(id, escrow(id)), donated_tokens);
    });
}

// =======================================================================
// FM-05 / FM-07 — the buy hook
// =======================================================================

#[test]
fn fm05_all_entry_paths_hit_the_hook() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        let calls_before = HOOK_CALLS.with(|c| c.borrow().len());
        let buy_call = |q: u128| RuntimeCall::Launchpad(crate::Call::buy { launch_id: id, quote_in: q, min_tokens_out: 0 });

        // direct
        buy(BOB, id, UNIT);
        // utility.batch_all of 3 buys → 3 hook calls, no aggregation
        assert_ok!(Utility::batch_all(origin(BOB), vec![buy_call(UNIT), buy_call(2 * UNIT), buy_call(3 * UNIT)]));
        // proxy: DAVE acts for CHARLIE → hook sees CHARLIE
        assert_ok!(Proxy::add_proxy(origin(CHARLIE), DAVE, ProxyType::Any, 0));
        assert_ok!(Proxy::proxy(origin(DAVE), CHARLIE, None, Box::new(buy_call(UNIT))));
        // multisig (threshold 1): origin is the multi account
        let multi = Multisig::multi_account_id(&[ALICE, BOB], 1);
        assert_ok!(Balances::transfer_allow_death(origin(ALICE), multi.clone(), 10 * UNIT));
        assert_ok!(Multisig::as_multi_threshold_1(origin(ALICE), vec![BOB], Box::new(buy_call(UNIT))));
        // initial buy inside create_launch
        let id2 = NextLaunchId::<Test>::get();
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"B"), bv(b"B"), None, UNIT, 0, None, None));

        let calls = HOOK_CALLS.with(|c| c.borrow().clone());
        let new = &calls[calls_before..];
        assert_eq!(new.len(), 1 + 3 + 1 + 1 + 1);
        assert_eq!(new[0], (id, 1, 1, BOB, false, UNIT));
        assert_eq!(new[1].5, UNIT);
        assert_eq!(new[2].5, 2 * UNIT);
        assert_eq!(new[3].5, 3 * UNIT);
        assert_eq!(new[4].3, CHARLIE);
        assert_eq!(new[5].3, multi);
        assert_eq!(new[6], (id2, 1, 1, ALICE, true, UNIT));

        // A blacklisted account is rejected on every path.
        HOOK_BLACKLIST.with(|b| *b.borrow_mut() = Some(CHARLIE));
        let err = DispatchError::Other("hook: blacklisted");
        assert_noop!(Launchpad::buy(origin(CHARLIE), id, UNIT, 0), err);
        frame_support::assert_err_ignore_postinfo!(Utility::batch_all(origin(CHARLIE), vec![buy_call(UNIT)]), err);
        // proxy dispatch reports the inner error through an event, not the extrinsic result
        let tokens_before = tok(id, CHARLIE);
        assert_ok!(Proxy::proxy(origin(DAVE), CHARLIE, None, Box::new(buy_call(UNIT))));
        assert_eq!(tok(id, CHARLIE), tokens_before, "proxy path bypassed the hook");
        let proxy_results: Vec<_> = System::events()
            .into_iter()
            .filter_map(|r| match r.event {
                RuntimeEvent::Proxy(pallet_proxy::Event::ProxyExecuted { result }) => Some(result),
                _ => None,
            })
            .collect();
        // the event strips `Other` messages, so match on the variant only
        assert!(matches!(proxy_results.last(), Some(Err(DispatchError::Other(_)))), "proxy results: {proxy_results:?}");
    });
}

#[test]
fn fm07_hook_receives_block_numbers_not_time() {
    new_test_ext().execute_with(|| {
        HOOK_REJECT_CREATION_BLOCK.with(|r| *r.borrow_mut() = true);
        System::set_block_number(5);
        // creator's atomic buy is exempt (is_creator == true)
        let id = NextLaunchId::<Test>::get();
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"M"), bv(b"M"), None, UNIT, 0, None, None));
        assert_eq!(launch(id).created_at, 5);
        // a non-creator in the creation block is rejected
        assert_noop!(Launchpad::buy(origin(BOB), id, UNIT, 0), DispatchError::Other("hook: not in creation block"));
        // the creator's later buy in the same block passes
        buy(ALICE, id, UNIT);
        // next block: everyone passes
        System::set_block_number(6);
        buy(BOB, id, UNIT);
        let calls = HOOK_CALLS.with(|c| c.borrow().clone());
        assert!(calls.iter().all(|c| c.0 == id && c.1 == 5));
        assert_eq!(calls.iter().map(|c| c.2).collect::<Vec<_>>(), vec![5, 5, 5, 6]);
        // There is no timestamp in this runtime at all; the hook signature only
        // carries block numbers (typed as BlockNumberFor<T>).
        assert!(!Launchpad::call_names().iter().any(|n| n.contains("time")));
    });
}

// =======================================================================
// FM-06 — LP permanence
// =======================================================================

#[test]
fn fm06_no_creator_lp_and_position_is_permanent() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        cross(BOB, id);
        let e = escrow(id);
        let p = pair(id);
        assert_noop!(
            VitreusDex::remove_liquidity(origin(ALICE), native(), kind(id), 1, 0, 0),
            pallet_vitreus_dex::Error::<Test>::InsufficientShares
        );
        // Even a signature from the escrow (impossible on chain: no key) is blocked by the lock.
        assert_noop!(
            VitreusDex::remove_liquidity(origin(&e), native(), kind(id), 1, 0, 0),
            pallet_vitreus_dex::Error::<Test>::PoolLocked
        );
        assert_noop!(
            <VitreusDex as PoolManager<Acc, NativeOrAssetId, u128, u64>>::lock_liquidity_for(&e, native(), kind(id), System::block_number()),
            pallet_vitreus_dex::Error::<Test>::LockCannotBeShortened
        );
        let positions: Vec<_> = LiquidityPositions::<Test>::iter().filter(|(_, pp, _)| *pp == p).collect();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].0, e);
        assert_eq!(positions[0].2.locked_until, Some(u64::MAX));
        System::set_block_number(u64::MAX - 1);
        assert_noop!(
            VitreusDex::remove_liquidity(origin(&e), native(), kind(id), 1, 0, 0),
            pallet_vitreus_dex::Error::<Test>::PoolLocked
        );
    });
}

// =======================================================================
// FM-08 — crossing buy, partial fill, deferred seed
// =======================================================================

/// Make the next seed fail: drain the DEX excess recipient so a pre-seed token
/// donation at the pool address cannot be delivered (ExcessRecipientCannotReceive).
fn arm_seed_failure(id: LaunchId, donor: impl Borrow<Acc>) {
    let donor = donor.borrow();
    assert_ok!(Balances::transfer_allow_death(origin(donor), pool_account(id), UNIT));
    assert_ok!(Assets::transfer(origin(donor), asset_of(id).into(), pool_account(id), 1_000 * UNIT));
    assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), EXCESS, 0));
}
fn disarm_seed_failure() {
    assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), EXCESS, ED));
}

#[test]
fn fm08_crossing_buy_partial_fill_and_deferred_seed() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 2_000 * UNIT);
        // leave ~1M tokens on the curve
        let s = state(id);
        let q_to_leave_1m = {
            // buy until tokens_remaining ≈ 1_000_000 tokens by quoting the exact crossing cost and backing off
            let t = terms(id);
            let target_remaining = 1_000_000 * UNIT;
            // Q_end for remaining=target: k / (VT_FLOOR + target) − Q
            let q = t.virtual_quote + s.real_quote;
            let tk = t.token_floor + s.tokens_remaining;
            let k = U256::from(q) * U256::from(tk);
            let q_new = u128::try_from(k / U256::from(t.token_floor + target_remaining)).unwrap();
            let net = q_new - q;
            net * 10_000 / (10_000 - 100) // gross of 1% fee
        };
        buy(CHARLIE, id, q_to_leave_1m);
        let remaining = state(id).tokens_remaining;
        assert!(remaining > 900_000 * UNIT && remaining < 1_100_000 * UNIT, "remaining {remaining}");

        arm_seed_failure(id, BOB);
        let quote = Launchpad::quote_buy(id, 3 * q_to_leave_1m).unwrap();
        assert!(quote.crossed);
        let before = vtrs(DAVE);
        let tokens_before = tok(id, DAVE);
        assert_ok!(Launchpad::buy(origin(DAVE), id, 3 * q_to_leave_1m, 0));
        // exactly the remaining tokens, charged exactly net + fee, remainder untouched
        assert_eq!(tok(id, DAVE) - tokens_before, remaining);
        assert_eq!(before - vtrs(DAVE), quote.quote_used);
        assert!(quote.quote_used < 3 * q_to_leave_1m);
        assert_eq!(quote.quote_used, quote.quote_net_used + quote.fee);

        // buy stood, seed deferred
        let s = state(id);
        assert_eq!(s.phase, Phase::Complete);
        assert_eq!(s.tokens_remaining, 0);
        assert!(s.real_quote > 0);
        assert_eq!(deferred_error(), Some(dex_err(pallet_vitreus_dex::Error::<Test>::ExcessRecipientCannotReceive)));
        assert!(!VitreusDex::pool_exists(native(), kind(id)));
        assert_noop!(Launchpad::sell(origin(DAVE), id, 1, 0), Error::<Test>::WrongPhase);
        assert_noop!(Launchpad::buy(origin(DAVE), id, UNIT, 0), Error::<Test>::WrongPhase);
        assert_noop!(Launchpad::graduate(origin(CHARLIE), id), dex_err(pallet_vitreus_dex::Error::<Test>::ExcessRecipientCannotReceive));

        // unforce → anyone graduates; the pool holds exactly the stored amounts
        disarm_seed_failure();
        let raised = state(id).real_quote;
        assert_ok!(Launchpad::graduate(origin(CHARLIE), id));
        assert_eq!(state(id).phase, Phase::Graduated);
        let pool = Pools::<Test>::get(pair(id)).unwrap();
        assert_eq!((pool.reserve_a, pool.reserve_b), (raised, RESERVED));
        assert_eq!(vtrs(pool_account(id)), raised);
        assert_eq!(tok(id, pool_account(id)), RESERVED);
    });
}

#[test]
fn fm08_min_tokens_out_respected_on_partial_fill() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 2_000 * UNIT);
        let remaining = state(id).tokens_remaining;
        let s_before = state(id);
        assert_noop!(
            Launchpad::buy(origin(CHARLIE), id, 500_000_000 * UNIT, remaining + 1),
            Error::<Test>::SlippageExceeded
        );
        assert_eq!(state(id), s_before);
        assert_ok!(Launchpad::buy(origin(DAVE), id, 500_000_000 * UNIT, 0));
        assert_eq!(state(id).phase, Phase::Graduated);
    });
}

// =======================================================================
// FM-09 — rounding
// =======================================================================

#[test]
fn fm09_rounding_always_favours_pool() {
    new_test_ext().execute_with(|| {
        let configs = [(3 * UNIT, 0u16), (3_000 * UNIT, 1), (3_000 * UNIT, 100), (1_000_000 * UNIT, 500), (3_000_000_000 * UNIT, 100)];
        let mut rng = Rng(0x1234_5678_9ABC_DEF0);
        let holders = [BOB, CHARLIE, DAVE];
        for (t_target, fee) in configs {
            set_params(t_target, fee, 5_000);
            let id = create(ALICE);
            let t = terms(id);
            let mut ops = 0;
            while ops < 400 {
                ops += 1;
                let who = holders[(rng.next() % 3) as usize].clone();
                let r = rng.next();
                let s0 = state(id);
                if s0.phase != Phase::Trading {
                    break;
                }
                let k0 = Launchpad::invariant_k(id).unwrap();
                let q0 = t.virtual_quote + s0.real_quote;
                if r % 3 != 0 || s0.tokens_remaining == SELLABLE {
                    let amt = match r % 6 {
                        0 => 1,
                        1 => 2,
                        2 => 3,
                        3 => 10u128.pow((r % 30) as u32),
                        4 => (r as u128 % 1_000) * UNIT + 7,
                        _ => vtrs(&who) - ED, // near the balance cap
                    };
                    let quote = Launchpad::quote_buy(id, amt);
                    let res = Launchpad::buy(origin(&who), id, amt, 0).map(|_| ()).map_err(|e| e.error);
                    match (quote, res) {
                        (Ok(q), Ok(())) => {
                            let s1 = state(id);
                            // I13: buyer never gets more than the constant-product price:
                            // t_out · (Q + q_net) ≤ q_net · Tk  (⇔ k does not decrease)
                            let tk0 = t.token_floor + s0.tokens_remaining;
                            let lhs = U256::from(q.tokens_out) * U256::from(q0 + q.quote_net_used);
                            let rhs = U256::from(q.quote_net_used) * U256::from(tk0);
                            assert!(lhs <= rhs, "I13 buy");
                            if q.crossed {
                                // graduated in the same call: real_quote moved to the pool
                                assert_ne!(s1.phase, Phase::Trading);
                            } else {
                                assert_eq!(s1.real_quote, s0.real_quote + q.quote_net_used);
                                assert_eq!(s1.tokens_remaining, s0.tokens_remaining - q.tokens_out);
                            }
                        },
                        (Err(e), Err(e2)) => {
                            assert_eq!(e, e2);
                            assert_eq!(state(id), s0, "failed trade changed state");
                            // Unquotable (rounds to nothing), Overflow (> MAX_TRADE_IN), or
                            // ZeroAmount (a drained buyer's balance − ED == 0).
                            assert!(
                                e == Error::<Test>::Unquotable.into()
                                    || e == Error::<Test>::ArithmeticOverflow.into()
                                    || e == Error::<Test>::ZeroAmount.into(),
                                "unexpected {e:?}"
                            );
                        },
                        (Ok(_), Err(DispatchError::Token(_))) => {
                            // a pure quote does not check the buyer's balance; the
                            // transfer fails (FundsUnavailable / Frozen at the ED edge)
                            assert_eq!(state(id), s0, "failed trade changed state");
                        },
                        (q, r) => panic!("quote/dispatch disagree: {q:?} vs {r:?}"),
                    }
                } else {
                    let held = tok(id, &who);
                    if held == 0 {
                        continue;
                    }
                    let amt = match r % 4 {
                        0 => 1,
                        1 => held,
                        _ => (r as u128 % held).max(1),
                    };
                    let quote = Launchpad::quote_sell(id, amt);
                    let res = Launchpad::sell(origin(&who), id, amt, 0);
                    match (quote, res) {
                        (Ok(q), Ok(())) => {
                            // I13 sell: q_gross · (Tk + t_in) ≤ t_in · Q
                            let tk0 = t.token_floor + s0.tokens_remaining;
                            let lhs = U256::from(q.quote_gross) * U256::from(tk0 + amt);
                            let rhs = U256::from(amt) * U256::from(q0);
                            assert!(lhs <= rhs, "I13 sell");
                        },
                        (Err(e), Err(e2)) => {
                            assert_eq!(e, e2);
                            assert_eq!(state(id), s0);
                            assert_eq!(e, Error::<Test>::Unquotable.into());
                        },
                        (q, r) => panic!("quote/dispatch disagree: {q:?} vs {r:?}"),
                    }
                }
                if state(id).phase == Phase::Trading {
                    let k1 = Launchpad::invariant_k(id).unwrap();
                    assert!(k1 >= k0, "I4: k decreased (T={t_target}, fee={fee})");
                }
                check_invariants(id, &[ALICE, BOB, CHARLIE, DAVE]);
            }
            // Full sell-back by everyone: escrow keeps ≥ ED + unclaimed; real_quote is the retained rounding.
            if state(id).phase == Phase::Trading {
                for who in holders.iter() {
                    let held = tok(id, who);
                    if held > 0 {
                        let _ = Launchpad::sell(origin(who), id, held, 0);
                    }
                }
                let s = state(id);
                assert!(vtrs(escrow(id)) >= ED + s.real_quote + s.creator_fees_unclaimed);
            }
        }
    });
}

#[test]
fn fm09_one_unit_edges() {
    new_test_ext().execute_with(|| {
        for t_target in [3 * UNIT, 3_000_000_000 * UNIT] {
            set_params(t_target, 100, 5_000);
            let id = create(ALICE);
            // A 1-unit buy is consumed entirely by the (rounded-up) fee: nothing
            // reaches the curve, so it is Unquotable rather than free tokens.
            assert_noop!(Launchpad::buy(origin(BOB), id, 1, 0), Error::<Test>::Unquotable);
            // 100 units: fee 1, 99 to the curve, > 0 tokens at either bound.
            buy(BOB, id, 100);
            assert!(tok(id, BOB) > 0);
            buy(BOB, id, UNIT);
            assert_noop!(Launchpad::sell(origin(BOB), id, 1, 0), Error::<Test>::Unquotable);
            assert_noop!(Launchpad::buy(origin(BOB), id, MAX_TRADE_IN + 1, 0), Error::<Test>::ArithmeticOverflow);
            assert_noop!(Launchpad::buy(origin(BOB), id, u128::MAX, 0), Error::<Test>::ArithmeticOverflow);
            assert_noop!(Launchpad::buy(origin(BOB), id, 0, 0), Error::<Test>::ZeroAmount);
            assert_noop!(Launchpad::sell(origin(BOB), id, 0, 0), Error::<Test>::ZeroAmount);
            assert_noop!(Launchpad::sell(origin(BOB), id, tok(id, BOB) + 1, 0), Error::<Test>::SellExceedsSold);
        }
    });
}

#[test]
fn fm09_no_u128_product_of_reserves() {
    // Lint: reserve products live in U256 only.
    let lib = include_str!("lib.rs");
    let curve = include_str!("curve.rs");
    assert!(!lib.contains("checked_mul("), "lib.rs multiplies balances directly");
    for name in ["real_quote", "tokens_remaining", "virtual_quote", "token_floor", "q.", "tk."] {
        assert!(!curve.contains(&format!("{name}checked_mul")), "{name} multiplied in u128");
        assert!(!curve.contains(&format!("{name} *")), "{name} multiplied in u128");
    }
    assert!(curve.contains("U256::from(q) * U256::from(tk)"));
    // the only u128 products in curve.rs are amount × fee_bps
    let products: Vec<&str> = curve.lines().filter(|l| l.contains(".checked_mul(")).collect();
    assert!(products.iter().all(|l| l.contains("fee_bps")), "{products:?}");
}

// =======================================================================
// FM-10 — parameter snapshotting
// =======================================================================

#[test]
fn fm10_params_change_does_not_touch_live_launch() {
    new_test_ext().execute_with(|| {
        set_params(30 * UNIT, 100, 5_000);
        let hash_before = Launchpad::current_params_hash();
        let l = create(ALICE);
        assert_eq!(launch(l).params_hash, hash_before);

        set_params(3_000 * UNIT, 500, 10_000);
        let hash_after = Launchpad::current_params_hash();
        assert_ne!(hash_after, hash_before);

        // L still charges 1 % and splits 50/50
        let q = Launchpad::quote_buy(l, UNIT).unwrap();
        assert_eq!(q.fee, UNIT / 100);
        buy(BOB, l, UNIT);
        assert_eq!(state(l).creator_fees_unclaimed, UNIT / 200);
        // L graduates at ≈ 30 VTRS
        cross(BOB, l);
        let raised = Pools::<Test>::get(pair(l)).unwrap().reserve_a;
        assert!(raised <= 30 * UNIT && 30 * UNIT - raised < UNIT / 1_000_000);

        // M uses the new terms
        let m = create(ALICE);
        assert_eq!(launch(m).curve.curve_fee_bps, 500);
        assert_eq!(launch(m).curve.protocol_share_bps, 10_000);
        assert_eq!(launch(m).curve.graduation_target, 3_000 * UNIT);
        assert_eq!(launch(m).params_hash, hash_after);

        assert_noop!(
            Launchpad::create_launch(origin(ALICE), bv(b"N"), bv(b"N"), None, 0, 0, Some(hash_before), None),
            Error::<Test>::ParamsMismatch
        );
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"N"), bv(b"N"), None, 0, 0, Some(hash_after), None));
    });
}

#[test]
fn fm10_params_bounds() {
    new_test_ext().execute_with(|| {
        let ok = Params::<Test>::get();
        let try_set = |p: LaunchParams<u128>| Launchpad::set_params(RuntimeOrigin::root(), p);
        let oob = || Error::<Test>::ParamsOutOfBounds;
        assert_noop!(try_set(LaunchParams { graduation_target: 3 * UNIT - 1, .. ok.clone() }), oob());
        assert_ok!(try_set(LaunchParams { graduation_target: 3 * UNIT, ..ok.clone() }));
        assert_noop!(try_set(LaunchParams { graduation_target: 3_000_000_000 * UNIT + 1, .. ok.clone() }), oob());
        assert_ok!(try_set(LaunchParams { graduation_target: 3_000_000_000 * UNIT, ..ok.clone() }));
        assert_noop!(try_set(LaunchParams { curve_fee_bps: 501, .. ok.clone() }), oob());
        assert_ok!(try_set(LaunchParams { curve_fee_bps: 500, ..ok.clone() }));
        assert_noop!(try_set(LaunchParams { protocol_share_bps: 4_999, .. ok.clone() }), oob());
        assert_ok!(try_set(LaunchParams { protocol_share_bps: 5_000, ..ok.clone() }));
        assert_noop!(try_set(LaunchParams { protocol_share_bps: 10_001, .. ok.clone() }), oob());
        assert_ok!(try_set(LaunchParams { protocol_share_bps: 10_000, ..ok.clone() }));
        assert_noop!(try_set(LaunchParams { pool_fee_tier: 2, .. ok.clone() }), oob());
        assert_ok!(try_set(LaunchParams { pool_fee_tier: 10, ..ok.clone() }));
        let min_fee = MinCreationFee::get();
        assert_noop!(try_set(LaunchParams { creation_fee: min_fee - 1, .. ok.clone() }), oob());
        assert_ok!(try_set(LaunchParams { creation_fee: min_fee, ..ok.clone() }));
        assert_noop!(Launchpad::set_params(origin(ALICE), ok.clone()), BadOrigin);
        assert_noop!(Launchpad::set_creation_paused(origin(ALICE), true), BadOrigin);
        assert_ok!(Launchpad::set_creation_paused(RuntimeOrigin::root(), true));
        assert_noop!(
            Launchpad::create_launch(origin(ALICE), bv(b"N"), bv(b"N"), None, 0, 0, None, None),
            Error::<Test>::CreationPaused
        );
    });
}

// =======================================================================
// FM-11 — seeding can never strand funds
// =======================================================================

#[test]
fn fm11_seed_overflow_is_impossible() {
    new_test_ext().execute_with(|| {
        set_params(3_000_000_000 * UNIT, 100, 5_000); // T = MaxGraduationTarget = 3·10^27 base units
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), ALICE, 10_000_000_000 * UNIT));
        let id = create(ALICE);
        assert_ok!(Launchpad::buy(origin(ALICE), id, 5_000_000_000 * UNIT, 0));
        // pre-D1 the DEX's first deposit (isqrt(3e27 × 2e26)) overflowed u128
        assert_eq!(state(id).phase, Phase::Graduated);
        let pool = Pools::<Test>::get(pair(id)).unwrap();
        assert!(pool.reserve_a > 2_999_000_000 * UNIT);
        assert_eq!(pool.reserve_b, RESERVED);
        assert!(TotalLiquidity::<Test>::get(pair(id)).unwrap() > u128::from(MINIMUM_LIQUIDITY));
    });
}

#[test]
fn fm11_every_complete_state_has_a_forward_path() {
    // (a) pool exists WITH liquidity → governance rescue after the delay
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 10 * UNIT);
        Pools::<Test>::insert(
            pair(id),
            PoolInfo { reserve_a: 0, reserve_b: 0, fee_tier: 3, total_fees_collected: 0, routing: pallet_vitreus_dex::FeeRouting::default(),
                pool_account: pool_account(id) },
        );
        TotalLiquidity::<Test>::insert(pair(id), 0u128);
        // liquidity at roughly p_end so the rescue is within tolerance
        let (pn, pd) = p_end(id);
        let tokens = tok(id, BOB);
        let quote = u128::try_from(U256::from(tokens) * U256::from(pn) / U256::from(pd)).unwrap();
        assert_ok!(VitreusDex::do_add_liquidity_for(&BOB, native(), kind(id), quote, tokens, 0, 0));
        cross(CHARLIE, id);
        assert_eq!(state(id).phase, Phase::Complete);
        let raised = state(id).real_quote;
        let e = escrow(id);
        let treasury_before = vtrs(TREASURY);
        System::set_block_number(System::block_number() + RESCUE_DELAY);
        assert_ok!(Launchpad::force_seed_into_existing_pool(RuntimeOrigin::root(), id, 100));
        let s = state(id);
        assert_eq!(s.phase, Phase::Graduated);
        assert!(s.lp_shares > 0);
        // funds left escrow only to the pool or the treasury
        assert_eq!(vtrs(&e), ED + s.creator_fees_unclaimed);
        assert_eq!(tok(id, &e), 0);
        let pool = Pools::<Test>::get(pair(id)).unwrap();
        let to_pool_q = pool.reserve_a - quote;
        let to_treasury_q = vtrs(TREASURY) - treasury_before;
        assert_eq!(to_pool_q + to_treasury_q, raised);
        let pos = LiquidityPositions::<Test>::get(&e, pair(id)).unwrap();
        assert_eq!(pos.locked_until, Some(u64::MAX));
    });
    // (b) pool record exists with ZERO liquidity → the crossing buy adopts it
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        Pools::<Test>::insert(
            pair(id),
            PoolInfo { reserve_a: 0, reserve_b: 0, fee_tier: 3, total_fees_collected: 0, routing: pallet_vitreus_dex::FeeRouting::default(),
                pool_account: pool_account(id) },
        );
        TotalLiquidity::<Test>::insert(pair(id), 0u128);
        cross(BOB, id);
        assert_eq!(state(id).phase, Phase::Graduated);
        assert_eq!(Pools::<Test>::get(pair(id)).unwrap().reserve_b, RESERVED);
    });
    // (c) excess recipient cannot receive → deferred, fixed, permissionless retry
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 10 * UNIT);
        arm_seed_failure(id, BOB);
        cross(CHARLIE, id);
        assert_eq!(state(id).phase, Phase::Complete);
        assert_noop!(Launchpad::graduate(origin(DAVE), id), dex_err(pallet_vitreus_dex::Error::<Test>::ExcessRecipientCannotReceive));
        disarm_seed_failure();
        assert_ok!(Launchpad::graduate(origin(DAVE), id));
        assert_eq!(state(id).phase, Phase::Graduated);
    });
}

#[test]
fn fm11_create_preflight_rejects_unseedable() {
    new_test_ext().execute_with(|| {
        // The DEX minimum is a crate constant; the guard is exercised directly.
        assert_noop!(Launchpad::ensure_seedable(1, 1), Error::<Test>::Unseedable);
        assert_noop!(Launchpad::ensure_seedable(1_000, 1_000), Error::<Test>::Unseedable); // isqrt = 1000, not > 1000
        assert_ok!(Launchpad::ensure_seedable(1_001, 1_001));
        assert_ok!(Launchpad::ensure_seedable(MinGraduationTarget::get(), RESERVED));
        // and with in-bounds params create_launch can never trip it
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"N"), bv(b"N"), None, 0, 0, None, None));
    });
}

// =======================================================================
// FM-12 / FM-13 — economics
// =======================================================================

#[test]
fn fm12_wash_trading_is_net_negative() {
    new_test_ext().execute_with(|| {
        for share in [5_000u16, 7_500, 10_000] {
            for fee in [1u16, 100, 500] {
                set_params(T_DEFAULT, fee, share);
                let id = create(ALICE);
                let start = vtrs(ALICE);
                let mut fees_paid = 0u128;
                for _ in 0..20 {
                    let q = Launchpad::quote_buy(id, 100 * UNIT).unwrap();
                    fees_paid += q.fee;
                    buy(ALICE, id, 100 * UNIT);
                    let held = tok(id, ALICE);
                    let sq = Launchpad::quote_sell(id, held).unwrap();
                    fees_paid += sq.fee;
                    sell(ALICE, id, held);
                }
                let claimed = state(id).creator_fees_unclaimed;
                if claimed > 0 {
                    assert_ok!(Launchpad::claim_creator_fees(origin(ALICE), id));
                }
                assert!(claimed < fees_paid, "share={share} fee={fee}: claimed {claimed} ≥ paid {fees_paid}");
                assert!(vtrs(ALICE) < start, "share={share} fee={fee}: wash trading was profitable");
            }
        }
    });
}

#[test]
fn fm13_dump_model_exposes_concentration() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        // CHARLIE accumulates ~30 % of the sellable supply, then the curve completes.
        while tok(id, CHARLIE) < 3 * SELLABLE / 10 {
            buy(CHARLIE, id, 50 * UNIT);
        }
        cross(BOB, id);
        let pool = Pools::<Test>::get(pair(id)).unwrap();
        let (rq, rt) = (pool.reserve_a, pool.reserve_b);
        let held = tok(id, CHARLIE);
        // constant-product prediction with the pool's 0.3 % fee
        let after_fee = held - held * 3 / 1_000;
        let predicted = u128::try_from(U256::from(rq) * U256::from(after_fee) / U256::from(rt + after_fee)).unwrap();
        let before = vtrs(CHARLIE);
        assert_ok!(VitreusDex::swap_exact_tokens_for_tokens(origin(CHARLIE), kind(id), native(), held, 0, CHARLIE));
        assert_eq!(vtrs(CHARLIE) - before, predicted);
        // The number the frontend's concentration warning must reproduce:
        // a 30 % holder's dump takes more than half the pool's quote.
        assert!(predicted * 2 > rq, "impact smaller than modelled: {predicted} of {rq}");
    });
}

// =======================================================================
// FM-14 — asset ids
// =======================================================================

#[test]
fn fm14_asset_id_squatting() {
    new_test_ext().execute_with(|| {
        let next = NextLaunchId::<Test>::get();
        assert_ok!(Assets::create(origin(BOB), asset_of(next).into(), BOB, 1));
        assert_noop!(
            Launchpad::create_launch(origin(ALICE), bv(b"N"), bv(b"N"), None, 0, 0, None, None),
            Error::<Test>::AssetIdTaken
        );
        assert_eq!(NextLaunchId::<Test>::get(), next);
        assert!(Launches::<Test>::get(next).is_none());
        // Nothing bumps NextLaunchId except a successful create.
        assert!(!Launchpad::call_names().iter().any(|n| n.contains("next") || n.contains("skip")));
    });
    new_test_ext().execute_with(|| {
        let next = NextLaunchId::<Test>::get();
        assert_ok!(Assets::create(origin(BOB), asset_of(next + 1).into(), BOB, 1));
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"N"), bv(b"N"), None, 0, 0, None, None));
        assert_noop!(
            Launchpad::create_launch(origin(ALICE), bv(b"N"), bv(b"N"), None, 0, 0, None, None),
            Error::<Test>::AssetIdTaken
        );
    });
}

#[test]
fn fm14_asset_id_range_never_reused() {
    new_test_ext().execute_with(|| {
        let mut rng = Rng(42);
        let mut created = Vec::new();
        for _ in 0..25 {
            let who = [ALICE, BOB, CHARLIE][(rng.next() % 3) as usize].clone();
            let id = create(&who);
            created.push(id);
            if rng.next() % 2 == 0 {
                buy(&who, id, UNIT);
            }
        }
        assert_eq!(created, (0..25).collect::<Vec<_>>());
        for id in created {
            assert_eq!(launch(id).asset_id, ASSET_BASE + id as u128);
            assert_eq!(AssetToLaunch::<Test>::get(asset_of(id)), Some(id));
        }
        assert_eq!(AssetToLaunch::<Test>::iter().count(), 25);
        assert_eq!(Launches::<Test>::iter().count(), 25);
    });
}

// =======================================================================
// FM-15 / FM-16
// =======================================================================

#[test]
fn fm15_escrow_survives_full_sellback_and_claims() {
    let run = |creation_fee: u128| {
        new_test_ext().execute_with(|| {
            assert_ok!(Launchpad::set_params(
                RuntimeOrigin::root(),
                LaunchParams { creation_fee, ..Params::<Test>::get() }
            ));
            let id = create(ALICE);
            let e = escrow(id);
            buy(BOB, id, 100 * UNIT);
            buy(CHARLIE, id, 50 * UNIT);
            sell(BOB, id, tok(id, BOB));
            sell(CHARLIE, id, tok(id, CHARLIE));
            assert_ok!(Launchpad::claim_creator_fees(origin(ALICE), id));
            assert!(frame_system::Pallet::<Test>::account_exists(&e));
            assert!(vtrs(&e) >= ED);
            assert_eq!(Balances::reserved_balance(&e), 100 + 2 * 4 + 2 * 4); // metadata base + per-byte; Create::create reserves no AssetDeposit
            assert_eq!(tok(id, &e), 1_000_000_000 * UNIT);
            buy(DAVE, id, UNIT);
            assert!(tok(id, DAVE) > 0);
        });
    };
    run(CREATION_FEE);
    run(MinCreationFee::get());
}

#[test]
fn fm16_name_symbol_not_enforced_on_chain() {
    new_test_ext().execute_with(|| {
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"Same"), bv(b"SAME"), None, 0, 0, None, None));
        assert_ok!(Launchpad::create_launch(origin(BOB), bv(b"Same"), bv(b"SAME"), None, 0, 0, None, None));
        assert_noop!(
            Launchpad::create_launch(origin(BOB), bv(b""), bv(b"X"), None, 0, 0, None, None),
            Error::<Test>::InvalidMetadata
        );
    });
}

// =======================================================================
// Smaller behaviours the spec fixes
// =======================================================================

#[test]
fn creator_fee_recipient_is_self_managed() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        buy(BOB, id, 100 * UNIT);
        assert_noop!(Launchpad::set_creator_fee_recipient(origin(BOB), id, BOB), Error::<Test>::NotFeeRecipient);
        assert_noop!(Launchpad::set_creator_fee_recipient(RuntimeOrigin::root(), id, BOB), BadOrigin);
        assert_ok!(Launchpad::set_creator_fee_recipient(origin(ALICE), id, CHARLIE));
        assert_noop!(Launchpad::claim_creator_fees(origin(ALICE), id), Error::<Test>::NotFeeRecipient);
        let unclaimed = state(id).creator_fees_unclaimed;
        let before = vtrs(CHARLIE);
        assert_ok!(Launchpad::claim_creator_fees(origin(CHARLIE), id));
        assert_eq!(vtrs(CHARLIE) - before, unclaimed);
        assert_noop!(Launchpad::claim_creator_fees(origin(CHARLIE), id), Error::<T>::ZeroAmount);
    });
}

type T = Test;

// ---- §6.2 weights ------------------------------------------------------

#[test]
fn weights_crossing_buy_refunds_when_not_crossing() {
    use crate::weights::WeightInfo as W;
    use frame_support::dispatch::GetDispatchInfo;
    new_test_ext().execute_with(|| {
        let id = create(ALICE);

        // Pre-dispatch, `buy` is always charged the crossing path.
        let call: RuntimeCall = crate::Call::<Test>::buy { launch_id: id, quote_in: 1, min_tokens_out: 0 }.into();
        assert_eq!(call.get_dispatch_info().weight, <() as W>::buy_crossing());
        assert!(<() as W>::buy_crossing().all_gt(<() as W>::buy()));

        // A buy that leaves tokens on the curve refunds down to `buy()`.
        let post = Launchpad::buy(origin(BOB), id, 2_000 * UNIT, 0).unwrap();
        assert_eq!(post.actual_weight, Some(<() as W>::buy()));
        assert_eq!(state(id).phase, Phase::Trading);

        // A crossing buy keeps the full charge.
        let post = Launchpad::buy(origin(BOB), id, 500_000_000 * UNIT, 0).unwrap();
        assert_eq!(post.actual_weight, None);
        assert_eq!(state(id).phase, Phase::Graduated);

        // `create_launch` with a non-crossing initial buy: charged create + crossing, refunded to create + buy.
        let call: RuntimeCall = crate::Call::<Test>::create_launch {
            name: bv(b"Meme"), symbol: bv(b"MEME"), creator_fee_recipient: None,
            initial_buy: 2_000 * UNIT, min_tokens_out: 0, expected_params_hash: None, metadata: None,
        }.into();
        assert_eq!(call.get_dispatch_info().weight, <() as W>::create_launch(4, 4, 0, 0).saturating_add(<() as W>::buy_crossing()));
        let post = Launchpad::create_launch(origin(CHARLIE), bv(b"Meme"), bv(b"MEME"), None, 2_000 * UNIT, 0, None, None).unwrap();
        assert_eq!(post.actual_weight, Some(<() as W>::create_launch(4, 4, 0, 0).saturating_add(<() as W>::buy())));

        // Without an initial buy nothing extra is charged and nothing is refunded.
        let call: RuntimeCall = crate::Call::<Test>::create_launch {
            name: bv(b"Meme"), symbol: bv(b"MEME"), creator_fee_recipient: None,
            initial_buy: 0, min_tokens_out: 0, expected_params_hash: None, metadata: None,
        }.into();
        assert_eq!(call.get_dispatch_info().weight, <() as W>::create_launch(4, 4, 0, 0));
        let post = Launchpad::create_launch(origin(CHARLIE), bv(b"Meme"), bv(b"MEME"), None, 0, 0, None, None).unwrap();
        assert_eq!(post.actual_weight, None);
    });
}

// ---- D4: post-graduation fee routing through the DEX -------------------
//
// The DEX snapshots the governance split into the pool it seeds at
// graduation and routes the VTRS slices into its fee escrow; the creator's
// share is claimable by whoever the launch currently names as
// `creator_fee_recipient`, resolved through the runtime adapter at claim
// time. Protocol fees are pulled to the shared recipient.

#[test]
fn d4_graduated_pool_routes_fees_and_the_launch_recipient_claims_them() {
    use pallet_vitreus_dex::{CreatorFeesUnclaimed, FeeRouting, ProtocolFeesUnclaimed};

    new_test_ext().execute_with(|| {
        assert_ok!(VitreusDex::set_default_fee_routing(RuntimeOrigin::root(), 5, 5));
        let id = create(ALICE);
        cross(BOB, id);
        assert_eq!(state(id).phase, Phase::Graduated);
        assert_eq!(
            Pools::<Test>::get(pair(id)).unwrap().routing,
            FeeRouting { protocol_bps: 5, creator_bps: 5 },
            "the seed snapshots the split in force at graduation"
        );

        // A DEX swap on the graduated pool routes 5 + 5 bps of the VTRS leg.
        let amount_in = 10 * UNIT;
        let slice = amount_in * 5 / 10_000;
        assert_ok!(VitreusDex::swap_exact_tokens_for_tokens(origin(BOB), native(), kind(id), amount_in, 0, BOB.clone()));
        assert_eq!(CreatorFeesUnclaimed::<Test>::get(pair(id)), slice);
        assert_eq!(ProtocolFeesUnclaimed::<Test>::get(), slice);
        assert_eq!(vtrs(VitreusDex::fee_escrow_account()), 2 * slice);

        // Only the launch's creator fee recipient can claim.
        assert_noop!(
            VitreusDex::claim_pool_creator_fees(origin(BOB), kind(id)),
            dex_err(pallet_vitreus_dex::Error::<Test>::NotCreatorFeeRecipient)
        );
        let before = vtrs(ALICE);
        assert_ok!(VitreusDex::claim_pool_creator_fees(origin(ALICE), kind(id)));
        assert_eq!(vtrs(ALICE) - before, slice);
        assert_eq!(CreatorFeesUnclaimed::<Test>::get(pair(id)), 0);

        // Handing the launch's fee stream to DAVE moves the DEX claim right
        // with it — no propagation, the DEX asks the launchpad at claim time.
        assert_ok!(Launchpad::set_creator_fee_recipient(origin(ALICE), id, DAVE));
        assert_ok!(VitreusDex::swap_exact_tokens_for_tokens(origin(BOB), native(), kind(id), amount_in, 0, BOB.clone()));
        assert_noop!(
            VitreusDex::claim_pool_creator_fees(origin(ALICE), kind(id)),
            dex_err(pallet_vitreus_dex::Error::<Test>::NotCreatorFeeRecipient)
        );
        let before = vtrs(DAVE);
        assert_ok!(VitreusDex::claim_pool_creator_fees(origin(DAVE), kind(id)));
        assert_eq!(vtrs(DAVE) - before, slice);

        // Protocol fees go to the shared recipient — the same account the
        // launchpad's curve fees already land in — when anyone pulls them.
        assert_eq!(VitreusDex::protocol_fee_recipient(), TREASURY);
        let treasury_before = vtrs(TREASURY);
        assert_ok!(VitreusDex::withdraw_protocol_fees(origin(CHARLIE)));
        assert_eq!(vtrs(TREASURY) - treasury_before, 2 * slice);
        assert_eq!(vtrs(VitreusDex::fee_escrow_account()), 0);
    });
}

#[test]
fn d4_creator_fee_recipient_lookup_is_none_for_unknown_assets() {
    new_test_ext().execute_with(|| {
        assert_eq!(Launchpad::creator_fee_recipient_for(asset_of(0)), None);
        let id = create(ALICE);
        assert_eq!(Launchpad::creator_fee_recipient_for(asset_of(id)), Some(ALICE));
        assert_ok!(Launchpad::set_creator_fee_recipient(origin(ALICE), id, BOB));
        assert_eq!(Launchpad::creator_fee_recipient_for(asset_of(id)), Some(BOB));
        assert_eq!(Launchpad::creator_fee_recipient_for(asset_of(id) + 1), None);
    });
}

// ---- §2.9 launch metadata ----------------------------------------------
//
// Presentation data lives in `Metadata`, apart from the launch record, and
// is mutable by the current creator fee recipient. Nothing about it is
// validated on chain (fm16 applies to every field, not just name/symbol).

fn meta(image: &[u8], description: &[u8]) -> crate::LaunchMetadataOf<Test> {
    let uri = |b: &[u8]| frame_support::BoundedVec::try_from(b.to_vec()).unwrap();
    crate::LaunchMetadata {
        image: uri(image),
        description: frame_support::BoundedVec::try_from(description.to_vec()).unwrap(),
        website: uri(b"https://example.com"),
        twitter: uri(b"@example"),
        telegram: uri(b"t.me/example"),
    }
}

#[test]
fn metadata_is_optional_and_stored_apart_from_the_launch_record() {
    new_test_ext().execute_with(|| {
        // None: no record, no event.
        let a = create(ALICE);
        assert!(crate::Metadata::<Test>::get(a).is_none());
        assert!(!has_event(|e| matches!(e, Event::LaunchMetadataSet { .. })));

        // Some: stored verbatim, the launch record itself is unchanged in shape.
        let b = NextLaunchId::<Test>::get();
        let m = meta(b"ipfs://Qm/logo.png", b"A meme.");
        assert_ok!(Launchpad::create_launch(origin(ALICE), bv(b"Meme"), bv(b"MEME"), None, 0, 0, None, Some(m.clone())));
        assert_eq!(crate::Metadata::<Test>::get(b), Some(m));
        assert!(has_event(|e| matches!(e, Event::LaunchMetadataSet { launch_id } if *launch_id == b)));
        assert_eq!(launch(b).creator, ALICE);

        // Nothing is validated: bytes that are not a URI or a handle are accepted as given.
        let c = NextLaunchId::<Test>::get();
        let junk = meta(b"not a uri \x00\xff", b"<script>alert(1)</script>");
        assert_ok!(Launchpad::create_launch(origin(BOB), bv(b"J"), bv(b"J"), None, 0, 0, None, Some(junk.clone())));
        assert_eq!(crate::Metadata::<Test>::get(c), Some(junk));
    });
}

#[test]
fn set_launch_metadata_follows_the_creator_fee_recipient() {
    new_test_ext().execute_with(|| {
        let id = create(ALICE);
        let m1 = meta(b"https://a/1.png", b"one");
        let m2 = meta(b"https://a/2.png", b"two");

        assert_noop!(Launchpad::set_launch_metadata(origin(BOB), id, m1.clone()), Error::<Test>::NotFeeRecipient);
        assert_noop!(Launchpad::set_launch_metadata(RuntimeOrigin::root(), id, m1.clone()), BadOrigin);
        assert_noop!(Launchpad::set_launch_metadata(origin(ALICE), 999, m1.clone()), Error::<Test>::LaunchNotFound);

        // The creator sets it when none was given at creation, and replaces it whole.
        assert_ok!(Launchpad::set_launch_metadata(origin(ALICE), id, m1.clone()));
        assert_eq!(crate::Metadata::<Test>::get(id), Some(m1.clone()));
        assert_ok!(Launchpad::set_launch_metadata(origin(ALICE), id, m2.clone()));
        assert_eq!(crate::Metadata::<Test>::get(id), Some(m2.clone()));

        // Handing the fee stream to CHARLIE hands metadata authority with it.
        assert_ok!(Launchpad::set_creator_fee_recipient(origin(ALICE), id, CHARLIE));
        assert_noop!(Launchpad::set_launch_metadata(origin(ALICE), id, m1.clone()), Error::<Test>::NotFeeRecipient);
        assert_ok!(Launchpad::set_launch_metadata(origin(CHARLIE), id, m1.clone()));
        assert_eq!(crate::Metadata::<Test>::get(id), Some(m1.clone()));

        // Still allowed after graduation.
        cross(BOB, id);
        assert_eq!(state(id).phase, Phase::Graduated);
        assert_ok!(Launchpad::set_launch_metadata(origin(CHARLIE), id, m2.clone()));
        assert_eq!(crate::Metadata::<Test>::get(id), Some(m2));
    });
}

#[test]
fn metadata_fields_are_bounded() {
    use frame_support::{traits::Get, BoundedVec};
    type UriLimit = <Test as crate::Config>::UriLimit;
    type DescriptionLimit = <Test as crate::Config>::DescriptionLimit;
    type Uri = BoundedVec<u8, UriLimit>;
    type Desc = BoundedVec<u8, DescriptionLimit>;
    let u = <UriLimit as Get<u32>>::get() as usize;
    let d = <DescriptionLimit as Get<u32>>::get() as usize;
    assert!(Uri::try_from(vec![b'x'; u]).is_ok());
    assert!(Uri::try_from(vec![b'x'; u + 1]).is_err());
    assert!(Desc::try_from(vec![b'x'; d]).is_ok());
    assert!(Desc::try_from(vec![b'x'; d + 1]).is_err());
    // The weight dimensions see the longest URI and the description length.
    let m = crate::LaunchMetadata::<UriLimit, DescriptionLimit> {
        image: Uri::try_from(vec![b'i'; 3]).unwrap(),
        description: Desc::try_from(vec![b'd'; 7]).unwrap(),
        website: Uri::try_from(vec![b'w'; 5]).unwrap(),
        twitter: Uri::try_from(Vec::<u8>::new()).unwrap(),
        telegram: Uri::try_from(vec![b't'; 4]).unwrap(),
    };
    assert_eq!(m.dims(), (7, 5));
}
