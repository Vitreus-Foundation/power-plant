# Security policy

## Reporting a vulnerability

Report vulnerabilities through
[GitHub's private vulnerability reporting](https://github.com/Vitreus-Foundation/power-plant/security/advisories/new),
never in a public issue, pull request or chat.

Include:

- the runtime's `spec_version`, if you know it;
- the affected pallet, call, RPC method or node component;
- the steps to reproduce and the impact you expect.

Reports without steps to reproduce, including automated ones, may be closed.

Don't test on mainnet: use a development chain or a fork of mainnet's state, as the
[e2e tests](e2e-tests/README.md) do.

## Scope

In scope: the node, the mainnet runtime and its pallets.

Out of scope:

- testnet and the code that runs only there, such as the experimental pallets and the faucet;
- websites, block explorers and other services around the network.

If the vulnerability is in the Polkadot SDK, also report it to Parity at security@parity.io.

## What happens next

We confirm the report and prepare a fix, which ships with the next release and mainnet upgrade.
Older releases get no backports. Keep the details private until mainnet runs the fix.

There is no bug bounty, and we can't promise a response time.
