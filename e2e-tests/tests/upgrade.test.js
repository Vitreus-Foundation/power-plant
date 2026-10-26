import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { failedExtrinsics, setupFork } from './helpers.js';

describe.skipIf(!process.env.WASM)('with WASM', () => {
  let fork;

  beforeAll(async () => {
    fork = await setupFork();
  });

  afterAll(async () => {
    await fork?.teardown();
  });

  it('upgraded runtime builds blocks', async () => {
    const { api, dev, upgrade } = fork;
    expect(
      upgrade.to,
      'the migrations run only if spec_version grows; raise it locally for an unreleased build',
    ).toBeGreaterThan(upgrade.from);

    const { query } = await api.at(upgrade.blockHash);
    const lastUpgrade = (await query.system.lastRuntimeUpgrade()).unwrap();
    const version = await api.rpc.state.getRuntimeVersion(upgrade.blockHash);
    expect(lastUpgrade.specVersion.toNumber()).toBe(upgrade.to);
    expect(lastUpgrade.specName.toString()).toBe(version.specName.toString());
    expect(await failedExtrinsics(api, upgrade.blockHash)).toBe(0);

    for (let i = 0; i < 3; i++) {
      const blockHash = await dev.newBlock();
      expect(await failedExtrinsics(api, blockHash)).toBe(0);
    }
    expect((await api.rpc.state.getRuntimeVersion()).specVersion.toNumber()).toBe(upgrade.to);
  });
});
