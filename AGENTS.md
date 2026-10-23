# AGENTS.md

Guidance for coding agents in this repository.

Vitreus power-plant: a Substrate-based relay chain with EVM (Frontier). VTRS is the native token; transaction fees are paid in the energy token VNRG. One runtime crate builds two networks, testnet and mainnet.

## Layout

- `runtime/vitreus` (`vitreus-power-plant-runtime`) is the runtime; `runtime/common` (`vitreus-runtime-common`) holds the traits shared by pallets and the runtime.
- `pallets/*` are our pallets. Experimental pallets live in the power-plant-experimental repo and come in as git dependencies that share one rev. Change them there, then bump that rev for all of them here.

## Networks

- Runtime features `testnet-runtime` and `mainnet-runtime`; node features `testnet-native` and `mainnet-native`. Each network has its own `spec_name`.
- Network-specific code goes behind its own network's feature, `#[cfg(feature = "testnet-runtime")]` or `#[cfg(feature = "mainnet-runtime")]`, never behind `not(...)` of the other. Everything else must build and pass tests on both networks.

## Commands

CI runs these once with `testnet-native` and once with `mainnet-native`; before pushing, do the same, one feature per run. The CI workflow in `.github/workflows/` is the source of truth. Always pass `--locked`.

```sh
cargo fmt -- --check
SKIP_WASM_BUILD=1 cargo clippy --locked --features testnet-native --workspace --all-targets -- --deny warnings
cargo build --locked --features testnet-native
cargo test --features testnet-native --workspace --locked
```

A full build is slow. Prefix `check` and `clippy` with `SKIP_WASM_BUILD=1` to skip the wasm build. While iterating, test one crate (`cargo test -p pallet-energy-broker --locked`) or the runtime (`cargo test -p vitreus-power-plant-runtime --features testnet-runtime --locked`).

## Dependencies

- Polkadot SDK and Frontier crates come from the sources in the root `Cargo.toml`. Take new crates from the same source as the existing entries: cargo unifies a git dependency only when the source strings match.

## Runtime code

- Runtime and pallet code must not panic. Avoid `unwrap`; restructure the code, or use `expect` with a proof that it cannot fail, ending in `; qed`. Tests are exempt.
- Runtime and pallet crates are `no_std`. Add dependencies with `default-features = false` and propagate the `std`, `runtime-benchmarks` and `try-runtime` features to them.
- Work in hooks (`on_initialize`, `on_finalize`, `on_idle`) is bounded per block, and its weight is accounted for.
- Don't turn a failure into "nothing to do": an `Option` or `Result` that means "can't" doesn't become a default (`unwrap_or_default`, `unwrap_or(0)`). A hook that can't return an error reports it in an event.
- Calls are charged by the `CustomFee` impl for `EnergyFee` in `runtime/vitreus`: pallets listed in its flat-fee arm pay the flat fee, and anything not listed falls through to the weight-based fee. When adding a pallet or changing how calls are charged, choose on purpose.
- A new pallet with benchmarks goes into `define_benchmarks!` for each network it runs on: there is one list per network.
- Pallet and call indices are never reused: a removed pallet or call leaves its index free, and a removed pallet stays as a comment in `construct_runtime!`. An existing call keeps its purpose: a call that does something else is a new call at a new index. Otherwise calls stored on chain or signed earlier decode as the new ones.
- A fix of runtime or pallet behaviour comes with a test that fails without it. Write that test first and see it fail, then make the fix without changing the test. Put new tests next to the related ones. Name new tests as a short statement of what they check, subject first and without articles (`failing_swap_is_reported`); rename existing tests only in a PR of their own.
- Test a pallet's own logic in the pallet's tests, against its mock. Runtime tests are for what depends on the runtime: config wiring, the real types behind config items (exchanges, fee handlers) and pallets working together.

## Code comments

- Comment what the code can't say: invariants, non-obvious reasons, units of values (base units or whole tokens, basis points, blocks or milliseconds), edge cases. Arguments for a design go in the PR description.
- Don't reference spec, design, review or audit documents (their sections such as `§x.y`, their decision or finding IDs, or the files themselves), files outside the repository, or this repo's PRs and issues, in comments or test names. History lives in git. A link to an issue in a dependency, such as the Polkadot SDK or Frontier, that a workaround depends on is fine.
- No market values, dates or snapshots, whether of a live network (rates, balances, prices) or of a local measurement (machine, run date). State the relation the code relies on; numbers for scale go in the PR description. Generated files, such as benchmarked weights, are exempt.
- Docs on shared traits in `runtime/common` describe the contract, not one implementation's assumptions.

## CHANGELOG and versions

The flow is in README.md. These are the parts agents get wrong:

- A PR that changes the runtime or a pallet adds entries under `## [Unreleased]` in `CHANGELOG.md`, runtime refactors included. Each entry starts with a verb and names the pallet or API it changes. Related small changes can share an entry. A PR that changes neither, such as a CI or docs change, ticks "Does not require a CHANGELOG entry" in its description, or the CHANGELOG check fails.
- Add new entries at the end of their subsection. Don't reorder or rewrite entries added by other PRs; on a rebase conflict, keep both sides' entries, yours last. Only a release PR may reorder, merge and reword them, keeping every change covered.
- By merge time, a PR's entries describe its final diff: when the PR changes, update its entries instead of adding lines for follow-up commits.
- Don't change `spec_version` or `transaction_version`; only a release PR does. Storage migrations go to `Unreleased` in `runtime/vitreus/src/migrations.rs`; a migration for one network goes behind its feature.

## Pull requests

- A PR covers one topic: a fix or a set of related fixes in one place, one feature, or one refactor, together with the code it forces to change, such as callers, runtime wiring and runtime APIs.
- Everything else goes to its own PR: unrelated fixes, refactors, renames, reformatting, dependency bumps and cleanups. Report what you noticed instead of fixing it, so it can become an issue; mention it in the PR description only if reviewers need it to understand this PR. Changes to several pallets and the runtime for different reasons are several PRs. Split a large topic into a series of PRs that each build and pass tests, and discuss runtime-wide changes in an issue first.
- `Cargo.lock` changes only for the dependencies the PR changes.
- Don't add entries for your own tools or local files (IDE, agents, notes) to `.gitignore`; use `.git/info/exclude`.
- If the PR changes what users or integrators see (RPC, EVM semantics, fees, XCM, events), describe the impact in the PR description.
- The title becomes the squash commit subject: imperative ("Fix …", "Add …", "Make …") without a `type(scope):` prefix, without spec references, and specific about what the whole PR changes. If it can't be, the PR is too wide.
