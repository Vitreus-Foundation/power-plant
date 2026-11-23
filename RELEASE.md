# Releasing

A runtime release is a "Runtime version N" PR that does all of this at once:

- moves the `[Unreleased]` entries of CHANGELOG.md into a new `## [N] - YYYY-MM-DD` section right
  below the (now empty) `## [Unreleased]`;
- bumps `spec_version` to N;
- moves the `Unreleased` migrations into `V0<N>`.

Merging a release PR publishes the GitHub release `runtime-vN`. It holds the testnet and mainnet
wasm built with [srtool](https://github.com/paritytech/srtool), their srtool reports, and notes:
the `[N]` section of CHANGELOG.md, each wasm's `spec_name`, `spec_version`, `transaction_version`,
size, blake2-256 and setCode call hashes, and the command that rebuilds it. The release PR shows
the same notes and hashes before the merge, from a dry run that also runs each runtime's
migrations on the current state of its network with
[try-runtime](https://github.com/paritytech/try-runtime-cli). The dry run has to pass: merging
into `main` requires `Release check`.

- Upgrade a network only with the wasm from a release, never with a local build.
- Right before upgrading a network, run the try-runtime workflow by hand on the tag `runtime-vN`:
  the state has changed since the release PR checked the migrations.
- Before upgrading mainnet, run the [e2e tests](e2e-tests/README.md) on a fork of it with the
  release's mainnet wasm: `WASM=<wasm> npm test` in `e2e-tests`.
- To verify a release, check out its tag and run the command from its notes: the hashes must
  match.
- If a release run fails, re-run it, or run Release on `main` by hand. Merging other PRs
  doesn't publish it.
- The release build fails if a runtime imports a host function missing from
  `.github/supported-host-functions.txt`: a node that lacks it can't load the runtime and stops at
  the upgrade. Add one there only once every node on mainnet, testnet and stagenet has it, the
  relay nodes embedded in collators included, in a PR that says which node versions provide it.
- An urgent fix goes into `main` like any other change, and a release PR follows right away.
  The release takes everything merged since the last one, so keep `main` ready to release.

## Node releases

A node release is a PR that bumps `version` in `[workspace.package]` of Cargo.toml, the version
that only the node crates inherit. On merge, CI builds the image with the `testnet-native`
feature, one image for every network, and checks that its binaries start and that the node
reports that version and commit. Then it pushes `vitreusfoundation/power-plant:X.Y.Z` and
`:latest` to Docker Hub and, last, tags the commit `node-vX.Y.Z`.

- If a release run fails, run Node release on `main` by hand. It builds the commit that set the
  version, not the latest one. Merging other PRs doesn't publish it.
- A version with a tag is never published again, and the `node-v*` tags can't be deleted or
  moved.
- Publishing needs the `docker-hub` environment, open only to `main`, with the Docker Hub token in
  the secret `DOCKERHUB_TOKEN` and its Docker ID in the variable `DOCKERHUB_USERNAME`.
