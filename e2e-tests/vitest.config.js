import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // A fork reads mainnet's state over RPC, so its setup and blocks take seconds.
    hookTimeout: 120_000,
    testTimeout: 120_000,
  },
});
