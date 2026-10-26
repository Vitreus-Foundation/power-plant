# E2E tests

Tests of the runtime on a fork of mainnet's live state, made with
[Chopsticks](https://github.com/AcalaNetwork/chopsticks). Chopsticks mocks the consensus and the
parachain inherent and has no Ethereum JSON-RPC, so the node isn't tested here: consensus, p2p,
parachain blocks and the EVM RPC are checked on a local network and the live networks.

Requires Node.js 22 or newer.

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
