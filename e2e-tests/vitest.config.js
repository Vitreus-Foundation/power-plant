import { defineConfig } from 'vitest/config';

// A fork reads mainnet's state over RPC: its setup and blocks take seconds, and with `WASM` the
// setup also builds the upgrade block, which fetches every key the migrations read: minutes.
const timeout = 15 * 60 * 1000;

export default defineConfig({
  test: {
    hookTimeout: timeout,
    testTimeout: timeout,
    // Shows the setup's warnings, such as a missing `WASM`: when vitest detects an AI agent, it
    // otherwise hides the console output of passed tests.
    silent: false,
  },
});
