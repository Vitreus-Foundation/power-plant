# Vitreus - Layer 0 Blockchain Ecosystem🌿⚡

[![CI](https://github.com/Vitreus-Foundation/power-plant/actions/workflows/ci.yml/badge.svg?branch=main&event=push)](https://github.com/Vitreus-Foundation/power-plant/actions/workflows/ci.yml?query=branch%3Amain)
[![Release](https://img.shields.io/github/v/release/Vitreus-Foundation/power-plant)](https://github.com/Vitreus-Foundation/power-plant/releases/latest)
[![Polkadot SDK version](https://img.shields.io/badge/dynamic/toml?url=https%3A%2F%2Fraw.githubusercontent.com%2FVitreus-Foundation%2Fpower-plant%2Fmain%2FCargo.toml&query=%24.workspace.dependencies%5B%27frame-support%27%5D.branch&label=Polkadot%20SDK&logo=Parity%20Substrate&color=brightgreen)](https://github.com/paritytech/polkadot-sdk)
[![License](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](LICENSE)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.gg/vitreus-dao-official-1050540395559403590)

Vitreus is a relay chain built on the [Polkadot SDK](https://github.com/paritytech/polkadot-sdk),
with EVM from [Frontier](https://github.com/polkadot-evm/frontier). This repository holds its
node and runtime. One runtime crate builds two networks, testnet and mainnet.

VTRS is the native token. [Transaction fees](pallets/energy-fee) are paid in the energy token
VNRG, which the [energy broker](pallets/energy-broker) sells for VTRS. VNRG is also the native
currency of the EVM, used for balances, transfers and gas.

## Getting started

### Prerequisites

- The Rust toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml), with its
  `wasm32-unknown-unknown` target
- The system packages from the
  [Polkadot SDK install guide](https://docs.polkadot.com/parachains/install-polkadot-sdk/); on
  Debian or Ubuntu:
  ```bash
  sudo apt install git clang curl libssl-dev llvm libclang-dev libudev-dev make protobuf-compiler
  ```
- Node.js, for the [e2e tests](e2e-tests/README.md)

### Build and run

```bash
git clone https://github.com/Vitreus-Foundation/power-plant
cd power-plant
cargo build --release --features testnet-native --locked
./target/release/vitreus-power-plant-node --dev
```

`--dev` starts a single-node development chain with the testnet runtime. For the mainnet runtime,
build with `--features mainnet-native` instead.

### Development accounts

The development chain comes with pre-funded accounts for testing:

- **Alith (sudo)**: `0xf24FF3a9CF04c71Dbc94D0b566f7A27B94566cac`
- **Baltathar**: `0x3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0`
- **Charleth**: `0x798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc`
- **Dorothy**: `0x773539d4Ac0e786233D90A233654ccEE26a613D9`
- **Ethan**: `0xFf64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB`
- **Faith**: `0xC0F0f4ab324C46e55D02D0033343B4Be8A55532d`
- **Goliath**: `0x7BF369283338E12C90514468aa3868A551AB2929`

These are Moonbeam's development accounts, and their private keys are public: use them only on
development chains. To use Alith in MetaMask, import its key
`0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133`.

### Connecting apps and wallets

[Polkadot.js Apps](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2Flocalhost%3A9944) and other
Substrate apps connect to `ws://localhost:9944`.

To add the development chain to MetaMask:

- Chain ID: 19434
- Network Name: vitreus-power-plant
- Currency Symbol: VNRG
- RPC URL: http://localhost:9944/

For mainnet, turn on Vitreus in [Talisman](https://talisman.xyz)'s network settings, or add it to
MetaMask from [chainlist.org](https://chainlist.org/chain/1943).

## Tests

```bash
# All tests, once per network
cargo test --features testnet-native --workspace --locked
cargo test --features mainnet-native --workspace --locked

# One pallet's tests
cargo test -p pallet-energy-broker --locked
```

The [e2e tests](e2e-tests/README.md) check the runtime on a fork of mainnet's live state.
In `e2e-tests`, run `npm ci` once, then `npm test`.

## Contributing

- [CONTRIBUTING.md](CONTRIBUTING.md): CHANGELOG entries and migrations in regular PRs
- [RELEASE.md](RELEASE.md): runtime releases and network upgrades

## Documentation

Browse the pallets and docs of the live runtimes in subxt-explorer:

- [Mainnet](https://paritytech.github.io/subxt-explorer/#/?config=url_wss%3A%2F%2Frpc-mainnet.vtrs.io)
- [Testnet](https://paritytech.github.io/subxt-explorer/#/?config=url_wss%3A%2F%2Frpc.testnet.compliq.io%3A9945)

## License

Licensed under [GPL-3.0](LICENSE).

## Support

- Bugs: [GitHub issues](https://github.com/Vitreus-Foundation/power-plant/issues)
- Vulnerabilities: [SECURITY.md](SECURITY.md), not public issues
- Questions: [Discord](https://discord.gg/vitreus-dao-official-1050540395559403590)
