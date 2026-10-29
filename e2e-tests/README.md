# E2E tests

Tests of the runtime on a fork of mainnet's live state, made with
[Chopsticks](https://github.com/AcalaNetwork/chopsticks). Chopsticks mocks the consensus and the
parachain inherent and has no Ethereum JSON-RPC, so the node isn't tested here: consensus, p2p,
parachain blocks and the EVM RPC are checked on a local network and the live networks.

Requires Node.js 22.12 or newer.

## Fork mainnet

```sh
npm ci
npm run fork
```

Chopsticks forks the latest finalized block and reads its state lazily, key by key, from
`wss://rpc-mainnet.vtrs.io:443`. The fork serves Substrate RPC over WebSocket and HTTP on
`127.0.0.1:8000` (the `PORT` environment variable overrides `--port`) and builds a block only on
`dev_newBlock`:

```sh
curl -s -H 'Content-Type: application/json' \
  -d '{"id":1,"jsonrpc":"2.0","method":"dev_newBlock","params":[{"count":1}]}' \
  http://127.0.0.1:8000
```

Flags after `--` override `configs/mainnet.yml`, for example
`npm run fork -- --block <number> --wasm-override <wasm>`:

- `--wasm-override` runs the fork on another wasm. The first block built on it runs the migrations
  only if the wasm's `spec_version` is higher than the one in `System.LastRuntimeUpgrade`.
- That first block can take minutes: Chopsticks fetches every key the migrations read over RPC.

## Tests

```sh
npm test
```

Each test file forks the latest finalized block with `configs/mainnet.yml` on a free port, so the
tests need the RPC endpoint but not a running `npm run fork`. The config writes the accounts the
tests use into storage, on the manual fork too: the well-known development accounts Alith
(`0xf24FF3a9CF04c71Dbc94D0b566f7A27B94566cac`) with VTRS and VNRG, and Baltathar
(`0x3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0`) with VTRS only, whose fees the broker covers. Such
an account skips `OnNewAccount` and gets no NAC, while one created by a transfer gets NAC level 1.
A test that needs more, such as the broker's liquidity, sets it itself. The tests take the expected
amounts from the forked state, such as the fee multiplier, the base fee and the broker's quote, so
they don't depend on live balances.

Without `WASM`, the tests run on the runtime on chain, so they check the tests themselves rather
than a build, and the setup warns about it. To run them on another wasm, such as a release's
`vitreus_power_plant_mainnet_runtime-vN.compact.compressed.wasm`:

```sh
WASM=<wasm> npm test
```

The setup of each test file then builds the upgrade block first, so the tests run on the migrated
state; that block takes minutes. The migrations run only if the wasm's `spec_version` is higher than
the one on chain: otherwise the setup warns, and `upgraded runtime builds blocks`, which runs only
with `WASM`, fails. To test an unreleased build, raise its `spec_version` locally.
