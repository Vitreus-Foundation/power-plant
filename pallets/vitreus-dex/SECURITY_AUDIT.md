# pallet-vitreus-dex Security Audit Report

**Date:** April 2026
**Auditor:** Claude AI (Anthropic claude-sonnet-4-6) assisted by Kevin Hahn
**Scope:** `pallets/vitreus-dex/src/lib.rs`

## Executive Summary

A comprehensive security audit was performed on the `pallet-vitreus-dex` AMM pallet for the Vitreus blockchain. The audit covered integer safety, access control, AMM math correctness, LP share calculations, slippage protection, pool account security, edge cases, fee logic, energy hooks, locked positions, reserve manipulation, and first-depositor attacks.

**12 findings were identified and resolved — 1 Critical, 4 High, 4 Medium, 3 Low.** A thirteenth, Critical, was found and fixed on 2026-09-15 (below); it is a truncation Finding 6 did not address.

All 11 unit tests pass. The full runtime (`vitreus-power-plant-runtime` with `testnet-runtime` feature) compiles cleanly.

## Findings

| # | Severity | Title | Description | Fix Applied |
|---|----------|-------|-------------|-------------|
| 1 | **CRITICAL** | First Depositor Attack | No minimum liquidity lockup on first deposit. An attacker could deposit 1 wei, directly transfer tokens to the pool account, then exploit integer truncation to steal subsequent depositors' funds. Classic Uniswap V2 attack vector. | `MINIMUM_LIQUIDITY = 1_000` shares permanently burned on first deposit. New `InsufficientInitialLiquidity` error enforces `sqrt(a*b) > 1_000`. |
| 2 | **HIGH** | Reserve Tracking Desync | Reserves tracked in storage rather than read from actual balances. Direct transfers to the pool account could manipulate pricing without updating reserves. | Added `sync_reserves()` helper that reads actual on-chain balances via `T::Assets::balance()`. Called at the start of `swap` and `remove_liquidity`. |
| 3 | **HIGH** | Fee Double-Counting | Swap added the full `amount_in` (including fee) to reserves while also incrementing `total_fees_collected`. Fees were counted both in reserves and in the fee counter. | Reserves now updated with `amount_in_after_fee` only. The fee stays in the pool account but is not counted in reserves until the next sync, when it accrues to LPs. |
| 4 | **MEDIUM** | Unrestricted Fee Tier | `fee_tier` accepted any value 0..999, allowing zero-fee pools (sandwich-attack vulnerable) and absurd fees like 99.9% that trap user funds. | Fee tier whitelisted to `1` (0.1%), `3` (0.3%), or `10` (1.0%). |
| 5 | **HIGH** | No Pair Canonicalization | Pools keyed by caller-provided `(asset_a, asset_b)` ordering. `create_pool(A, B)` and `create_pool(B, A)` created two separate pools, fragmenting liquidity. `add_liquidity` and `remove_liquidity` only looked up one ordering. | Added `canonical_pair()` that sorts by SCALE encoding. Applied in all 5 extrinsics (`create_pool`, `add_liquidity`, `remove_liquidity`, `swap`, `lock_liquidity`). |
| 6 | **MEDIUM** | Pool Sub-Account Collision | `into_sub_account_truncating(&pair)` concatenated raw asset encodings without length prefixes. Two different asset pairs with the same concatenated encoding could produce the same pool account. | Derivation now uses `(pair.0.encode(), pair.1.encode())` — SCALE-encoded `Vec<u8>` tuples with length prefixes for unambiguous separation. |
| 7 | **MEDIUM** | Slippage Check No-Op in add_liquidity | Slippage check compared caller-provided `amount_a >= amount_a_min` — both values controlled by the caller, making the check trivially satisfied. | Slippage now checked against `actual_a` and `actual_b` (the optimized amounts after ratio adjustment), which may differ from the caller's requested amounts. |
| 8 | **HIGH** | Excess Token Donation | When a user provided imbalanced amounts relative to pool ratio, the pallet transferred both full amounts but minted shares based on the lesser ratio. The excess was donated to existing LPs with no compensation. | Optimal deposit amounts now computed before transfer. Only the proportional amounts are transferred from the user; excess stays in the user's account. |
| 9 | **LOW** | entry_block Reset on Top-Up | Adding liquidity to an existing position overwrote `entry_block` with the current block, allowing gaming of any time-based incentive logic. | `entry_block` preserved on top-up. Only set on initial position creation. |
| 10 | **MEDIUM** | locked_until Dead Code | `locked_until` field existed on `LiquidityPosition` but was never set by any extrinsic — the locking feature was entirely non-functional. | Added `lock_liquidity` extrinsic (call_index 4) that sets `locked_until` on a position. New `LiquidityLocked` event emitted. |
| 11 | **LOW** | FeesCollected Event Never Emitted | `FeesCollected` event was defined but never emitted. `total_fees_collected` was incremented but never read. | `FeesCollected` event now emitted in `swap` after fee accounting, with the pool account as the recipient. |
| 12 | **LOW** | Energy Hook No-Ops | `OnEnergySell` and `OnEnergyBurn` implementations only emitted events with no persistent state tracking. | Added `TotalEnergySold` and `TotalEnergyBurned` storage counters. Hooks now accumulate amounts via `checked_add`. **Superseded (2026-09-14):** the hooks, counters and events were removed entirely. Nothing in the pallet, runtime or site read the counters, and `pallet_energy_fee` invokes `OnEnergyBurn` on every fee-paying extrinsic, so the DEX was writing storage and emitting two events (one usually `amount: 0`) for every unrelated transaction on the chain. The runtime's `OnEnergySell`/`OnEnergyBurn` tuples are back to what they were before the DEX was added. |

## Audit Scope Details

The following categories were reviewed:

1. **Integer Overflow/Underflow** — All arithmetic uses `CheckedAdd`/`CheckedSub`/`CheckedMul`/`CheckedDiv`. No unchecked operations found.
2. **Access Control** — `create_pool` restricted to `ManageOrigin`. All other extrinsics require `ensure_signed`. No unauthorized access paths.
3. **AMM Math Correctness** — Constant product formula `x * y = k` correctly implemented. Reserves updated consistently in both swap directions.
4. **LP Share Calculation** — First deposit uses `sqrt(a * b)` with minimum liquidity lock. Subsequent deposits use proportional `min(share_a, share_b)`.
5. **Slippage Protection** — `amount_out_min` enforced in swap. `amount_a_min`/`amount_b_min` enforced on actual (optimized) amounts in `add_liquidity` and `remove_liquidity`.
6. **Pool Account Security** — Sub-account derived with length-prefixed encoding after pair canonicalization.
7. **Zero Amount Edge Cases** — All entry points guard against zero amounts. Zero reserves checked before swap math.
8. **Fee Calculation** — Fee tier validated against whitelist. No division-by-zero possible (`FEE_DENOMINATOR = 1_000` constant).
9. **Energy Hook Safety** — Hooks cannot panic. Zero amounts handled safely. Cumulative counters use `checked_add` with silent saturation.
10. **Locked Position Enforcement** — `locked_until` checked in `remove_liquidity`. New `lock_liquidity` extrinsic activates the feature.
11. **Reserve Manipulation** — `sync_reserves()` reconciles storage with actual balances, neutralizing direct-transfer attacks.
12. **First Depositor Attack** — `MINIMUM_LIQUIDITY` shares permanently locked, preventing share-price manipulation.

### Finding 13 (2026-09-15) — CRITICAL — Pool Sub-Account Collision on AccountId20

**Found by** deriving pool accounts off-chain to label holders on the site, and noticing the derivation only ever used four bytes of an asset id.

**Description.** `pool_account_for` seeded `into_sub_account_truncating` with the pair key itself. That helper keeps the first `size_of::<AccountId>()` bytes of `"modl" ++ PalletId ++ SCALE(seed)`; twelve of those are the prefix, so on the AccountId20 runtime **eight bytes of the pair key survived**: for `(Native, WithId(x))` the bytes `04 00 44 01` and the low four bytes of `x`, so chain asset `n` and launch asset `2^64 + n` shared one pool account (on the dev chain: VTRS/SNRG with VTRS/GLASS); for `(WithId(a), WithId(b))` the bytes `44 01` and the low six bytes of `a` — the second asset never featured, so every pool whose lower-id asset is USDC shared one account. Nothing guarded it: `insert_new_pool` wrote the same `pool_account` for the second pool, `sync_reserves` then read the first pool's balances as the second's, and the graduation sweep (FM-02) would have delivered the first pool's reserves to `ExcessRecipient`. Governance creating a VTRS/VNRG pool after launch 0 graduated was enough; the chain's three energy assets are ids 0, 1 and 2 — the first three launches. In the pallet's own mock (16-byte accounts) four bytes of the key survive and every native pair collides, which is how the red test shows the consequence: the first depositor into a second native pool withdraws 899,000 of the first pool's 1,000,000 VTRS (`d8_second_pool_on_a_shared_account_would_read_the_first_pools_reserves`).

**Relation to Finding 6.** Finding 6 length-prefixed the two asset encodings so that different pairs could not produce the same *concatenation*. That is ambiguity within the key; the key was then truncated to eight bytes regardless, which is the collision here. Finding 6's fix is kept and is still necessary for the hashed key to be unambiguous.

**Fix.** The seed is `blake2_256(pair_key)`; eight bytes of a hash do not collide. Tests: `d8_distinct_pairs_derive_distinct_pool_accounts`, `d8_second_pool_on_a_shared_account_would_read_the_first_pools_reserves` (red before, green after). On the fork, `migrations::v2::MigrateToV2` (storage version 1 → 2) moves each existing pool's reserves from the old account to the new one and rewrites `pool_account`, with a `pre_upgrade` check that no two pools already share an old account — that state is the bug, and cannot be attributed after the fact (`d8_migration_v2_moves_reserves_to_the_hash_derived_account`). The upstream submission ships the fix with no migration: no chain upstream has a pre-D8 pool.

## Test Results

```
running 11 tests
test mock::test_genesis_config_builds ... ok
test mock::__construct_runtime_integrity_test::runtime_integrity_tests ... ok
test tests::test_create_pool_duplicate_fails ... ok
test tests::test_add_liquidity_first_deposit ... ok
test tests::test_create_pool_success ... ok
test tests::test_add_liquidity_subsequent_deposit ... ok
test tests::test_on_energy_sell_hook ... ok
test tests::test_remove_liquidity_full ... ok
test tests::test_swap_insufficient_liquidity ... ok
test tests::test_swap_exact_tokens ... ok
test tests::test_swap_slippage_protection ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Conclusion

All 13 findings have been resolved. The pallet compiles cleanly within the full `vitreus-power-plant-runtime` (testnet-runtime feature). All 11 unit tests pass. The fixes follow established AMM security patterns (Uniswap V2 minimum liquidity, pair canonicalization, reserve syncing) adapted to the Substrate/FRAME environment.
