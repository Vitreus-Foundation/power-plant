import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { fetchConfig } from '@acala-network/chopsticks';
import { setupContextWithConfig } from '@acala-network/chopsticks-testing';
import { Keyring } from '@polkadot/api';

// VTRS and VNRG have 18 decimals.
export const UNIT = 10n ** 18n;
export const VNRG_ASSET_ID = 0;

const keyring = new Keyring({ type: 'ethereum' });

// Moonbeam's well-known development account; `configs/mainnet.yml` gives it VTRS and VNRG.
export const alith = keyring.addFromUri(
  '0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133',
);

// A random account, so it doesn't exist on chain yet.
export const freshAccount = () => keyring.addFromSeed(randomBytes(32));

// A fork with the config of `npm run fork`, on a free port.
export async function setupFork() {
  const config = await fetchConfig(
    fileURLToPath(new URL('../configs/mainnet.yml', import.meta.url)),
  );
  return setupContextWithConfig({ ...config, port: 0 });
}

// The treasury's account: `modl` and its pallet id, padded with zeros.
export function treasuryAccount(api) {
  const account = new Uint8Array(20);
  account.set(new TextEncoder().encode('modl'));
  account.set(api.consts.treasury.palletId.toU8a(), 4);
  return api.createType('AccountId20', account).toString();
}

// The events of one extrinsic of the block.
export async function extrinsicEvents(api, blockHash, txHash) {
  const block = await api.rpc.chain.getBlock(blockHash);
  const index = block.block.extrinsics.findIndex((extrinsic) => extrinsic.hash.eq(txHash));
  if (index < 0) {
    throw new Error(`Extrinsic ${txHash} isn't in block ${blockHash}`);
  }
  const events = await (await api.at(blockHash)).query.system.events();
  return events
    .filter(({ phase }) => phase.isApplyExtrinsic && phase.asApplyExtrinsic.eq(index))
    .map(({ event }) => event);
}

// `Permill * Balance` in Substrate: rounded to the nearest, ties down.
export function mulPermill(amount, parts) {
  const product = amount * parts;
  const quotient = product / 1_000_000n;
  return (product % 1_000_000n) * 2n > 1_000_000n ? quotient + 1n : quotient;
}
