# Generating real weights for pallet-vitreus-dex and pallet-launchpad

The two pallets ship placeholder `weights.rs` files (the old hard-coded
constants in the frame-weight-template layout). This is the runbook for
replacing them with measured weights. Do not run it on the development box:
2 vCPU shared with a live node produces numbers that are wrong by an unknown
factor, and wrong weights are worse than placeholders because they look
authoritative.

## 1. Hardware

Weights are only meaningful relative to the hardware the chain is expected
to run on. Polkadot's reference hardware is 8 physical cores at ≥ 3.4 GHz,
32 GB RAM, NVMe; `benchmark machine` (step 4) scores a box against it.

DigitalOcean **CPU-Optimized, dedicated vCPU**, Ubuntu 24.04 x64:

| Size | Use |
|---|---|
| `c-16` (16 vCPU, 32 GB) | recommended — the release build with `runtime-benchmarks` is the slow part (~25–35 min here vs 90+ on c-8); benchmarks themselves are single-threaded |
| `c-8` (8 vCPU, 16 GB) | works; roughly 2× the build time |

Do not use shared-CPU (Basic / Premium) droplets; steal time shows up
directly in the numbers. Nothing else should run on the box while
`benchmark pallet` runs. Budget: ~2 hours on `c-16`.

## 2. Provision and build

```bash
# as root on the droplet
apt update && apt install -y build-essential clang libclang-dev llvm protobuf-compiler \
  pkg-config libssl-dev git curl cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"

git clone git@github.com:Bison1330/vitreusdex-pallet.git power-plant   # or https
cd power-plant
git checkout feature/solver-marketplace
git rev-parse HEAD > /tmp/bench-commit           # record what was measured
rustup show                                       # picks up rust-toolchain.toml (1.83 + wasm32)

# The launchpad is testnet-only, so the benchmarking node must carry the
# testnet native runtime. Both pallets are measured from this one binary.
cargo build --release --locked --features testnet-native,runtime-benchmarks
ls -la target/release/vitreus-power-plant-node
```

`--locked` matters: the checked-in `Cargo.lock` is what the runtime was
verified against.

## 3. Sanity: the benchmark list

```bash
./target/release/vitreus-power-plant-node benchmark pallet --chain dev --list \
  | grep -E '^pallet_(vitreus_dex|launchpad),'
```

Expect 20 lines for `pallet_vitreus_dex` (create_pool … set_solver_bond_amount,
then set_default_fee_routing, set_protocol_fee_recipient,
claim_pool_creator_fees, withdraw_protocol_fees) and 11 for
`pallet_launchpad` (create_launch, buy, buy_crossing, sell, graduate,
claim_creator_fees, set_creator_fee_recipient, set_params,
set_creation_paused, force_seed_into_existing_pool, set_launch_metadata —
ten calls plus `buy_crossing`, the crossing branch of `buy`). If either
pallet is missing, the binary was built without `testnet-native` or
without `runtime-benchmarks`.

## 4. Score the machine

```bash
./target/release/vitreus-power-plant-node benchmark machine --chain dev \
  --disk-duration 30 --allow-fail 2>&1 | tee /tmp/benchmark-machine.txt
# (--allow-fail only turns a below-reference score from an error into a
#  warning so the report still prints; a warning still means stop.)
```

Every row should be ≥ the reference score. A CPU row well below reference
means a shared or throttled box — stop and re-provision. Keep this file; it
goes back with the weights.

## 5. Generate

```bash
COMMON="--chain dev --wasm-execution compiled --steps 50 --repeat 20 --heap-pages 4096 \
        --template .maintain/frame-weight-template.hbs"

./target/release/vitreus-power-plant-node benchmark pallet $COMMON \
  --pallet pallet_vitreus_dex --extrinsic '*' \
  --output pallets/vitreus-dex/src/weights.rs \
  --json-file /tmp/bench-vitreus-dex.json 2>&1 | tee /tmp/bench-vitreus-dex.log

./target/release/vitreus-power-plant-node benchmark pallet $COMMON \
  --pallet pallet_launchpad --extrinsic '*' \
  --output pallets/launchpad/src/weights.rs \
  --json-file /tmp/bench-launchpad.json 2>&1 | tee /tmp/bench-launchpad.log
```

`--steps 50 --repeat 20` is the Polkadot convention: 50 points per linear
component (`create_launch` has four, `set_launch_metadata` two), 20 repeats
each. Together the two runs take on the order of 20–40 minutes on `c-16`.
`--output` overwrites the placeholder file in place; the template writes
the same `WeightInfo` trait, `SubstrateWeight<T>` and `()` impls the
placeholders have, so nothing else in the crate changes.

Optional, same session, and recommended by LAUNCHPAD_SPEC §8.2: every
other pallet's weights were generated under the old `AssetId = u32`
switch and measured a narrower type than production. Regenerating them is
the same command per pallet; it is a separate commit.

## 6. Verify before committing

On the droplet, all of these must pass:

```bash
# 1. The generated files compile and every test still passes. Tests that
#    reason about weights (`weights_crossing_buy_refunds_when_not_crossing`)
#    use `<() as WeightInfo>` and are unaffected by the numbers.
cargo test -p pallet-vitreus-dex -p pallet-launchpad
cargo test -p pallet-vitreus-dex -p pallet-launchpad --features runtime-benchmarks
cargo check -p vitreus-power-plant-runtime --features testnet-runtime,runtime-benchmarks
cargo check -p vitreus-power-plant-runtime --features mainnet-runtime

# 2. Every function is present with the right signature.
grep -c 'fn .*-> Weight' pallets/vitreus-dex/src/weights.rs   # 20 in the trait
grep -n 'fn create_launch(n: u32, s: u32, d: u32, u: u32)\|fn set_launch_metadata(d: u32, u: u32)' \
  pallets/launchpad/src/weights.rs
```

Then read the numbers, not just the diff:

- **No zero `ref_time` and no zero `proof_size`** on any function; a zero
  means the benchmark's `#[extrinsic_call]` did not run the path.
- **Ordering holds:** `buy_crossing() > buy()`; `graduate()` is close to
  `buy_crossing()` less a plain buy; `create_launch` at maximum components
  > at minimum; `swap_exact_tokens_for_tokens` (measured on the routed
  branch: three transfers, two counters) > `lock_liquidity()`;
  `set_launch_metadata` grows with `d` and `u`.
- **Components are sane:** the per-byte slopes on `n`, `s`, `d`, `u` should
  be small positive numbers (storage write cost per byte), not large or
  negative. A negative slope means noise dominated — re-run that pallet
  with `--repeat 50`.
- **Reads/writes match the code:** the `// Storage:` comments the template
  emits list every storage item each call touched. `buy_crossing` must show the DEX `Pools`, `LiquidityPositions`,
  `TotalLiquidity` writes (the seed) that `buy` does not;
  `claim_pool_creator_fees` must show `CreatorFeesUnclaimed` read and
  written plus two `System::Account` writes. If a call's list is missing a
  storage item you know it touches, the benchmark setup took a cheaper
  branch than intended.
- **Magnitude vs placeholders:** placeholders are in the 10^7–10^8 ref_time
  range. Measured values 2–10× off in either direction are normal; 100× off
  is a benchmark that measured the wrong thing.

## 7. Copy back and commit

Copy to the repo on the box you commit from:

```
pallets/vitreus-dex/src/weights.rs
pallets/launchpad/src/weights.rs
/tmp/benchmark-machine.txt      -> kept with the raw output (see below)
/tmp/bench-*.json               -> kept with the raw output; hundreds of
                                   thousands of lines, so not in this repo
/tmp/bench-*.log                -> keep locally; not committed
/tmp/bench-commit               -> goes in the commit message
```

The raw `benchmark pallet` JSON and the `benchmark machine` output are the
provenance for a weights.rs; keep them where the PR that ships the weights
can link to them (the contributor's fork, a gist, or a release asset). The
commit that ships weights states the machine, its `benchmark machine`
summary line, steps/repeat, and the measured commit in its body; each
weights.rs already carries the per-extrinsic statistics in its header.
Regenerated weights for other pallets are a second commit.

After it lands: the runtime's `spec_version` must be bumped for the weights
to reach a live chain (weights are compiled into the runtime), and
`benchmark overhead` (block and extrinsic base weights, in
`runtime/vitreus/src/weights/`) is a separate exercise on the same hardware
if those were never measured either.
