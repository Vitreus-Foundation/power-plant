// Drives pallet-launchpad end to end on a live chain: (optionally) sudo-upgrade
// the runtime, create a launch, buy/sell along the curve, cross the threshold,
// and verify the graduated VitreusDEX pool. Prints every extrinsic hash and the
// events that matter. Dev-chain only: uses the well-known Alith/Baltathar keys.
//
//   node scripts/devchain-launchpad.mjs [--upgrade path/to/runtime.compact.compressed.wasm]
//   RPC=ws://127.0.0.1:9945
import { readFileSync } from 'node:fs';
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { u8aToHex } from '@polkadot/util';

const RPC = process.env.RPC ?? 'ws://127.0.0.1:9945';
const upgradeIdx = process.argv.indexOf('--upgrade');
const wasmPath = upgradeIdx > 0 ? process.argv[upgradeIdx + 1] : null;
const UNIT = 10n ** 18n;

await cryptoWaitReady();
const keyring = new Keyring({ type: 'ethereum' });
const alith = keyring.addFromUri('0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133');
const baltathar = keyring.addFromUri('0x8075991ce870b93a8870eca0c0f91913d12f47948ca0fd25b49c6fa7cdbeee8b');

let api = await ApiPromise.create({ provider: new WsProvider(RPC, 2000), noInitWarn: true });
const log = (...a) => console.log(...a);
const fmt = (v, d = 18n) => { const b = BigInt(v.toString()); const w = b / 10n ** d; const f = (b % 10n ** d).toString().padStart(Number(d), '0').slice(0, 6); return `${w.toLocaleString('en-US')}.${f}`; };

/** Sign, submit, wait for finalization; return {hash, block, events} or throw on dispatch error. */
async function send(label, tx, signer) {
  return new Promise((resolve, reject) => {
    let done = false;
    tx.signAndSend(signer, { nonce: -1 }, (r) => {
      // Resolve on whichever inclusion status arrives first: a node (or a
      // chopsticks fork) can deliver Finalized without a preceding InBlock.
      if (!done && (r.status.isInBlock || r.status.isFinalized)) {
        done = true;
        const blockHash = r.status.isInBlock ? r.status.asInBlock : r.status.asFinalized;
        const evs = r.events.map((e) => ({ section: e.event.section, method: e.event.method, data: e.event.data.toHuman() }));
        const failed = evs.find((e) => e.section === 'system' && e.method === 'ExtrinsicFailed');
        if (failed || r.dispatchError) {
          const de = r.dispatchError;
          const msg = de?.isModule ? (() => { const m = api.registry.findMetaError(de.asModule); return `${m.section}.${m.name}: ${m.docs.join(' ')}`; })() : de?.toString();
          reject(new Error(`${label} failed in ${blockHash.toHex()}: ${msg}`));
        }
        log(`  ${label}: tx ${r.txHash.toHex()} in block ${blockHash.toHex()}`);
        for (const e of evs) if (!['system', 'transactionPayment', 'energyFee', 'balances'].includes(e.section) || e.method === 'ExtrinsicFailed') log(`    event ${e.section}.${e.method} ${JSON.stringify(e.data)}`);
        resolve({ hash: r.txHash.toHex(), block: blockHash.toHex(), events: evs });
      }
      if (r.isError) reject(new Error(`${label}: ${r.status.type}`));
    }).catch(reject);
  });
}

log(`chain ${api.runtimeChain} spec ${api.runtimeVersion.specVersion} | launchpad pallet: ${!!api.tx.launchpad}`);

// ---- 0. runtime upgrade (only if asked and needed) -------------------------
if (wasmPath) {
  if (api.runtimeVersion.specVersion.toNumber() >= 215) {
    log('already at spec >= 215, skipping upgrade');
  } else {
    const code = u8aToHex(readFileSync(wasmPath));
    log(`upgrading runtime: ${wasmPath} (${(code.length - 2) / 2} bytes) via sudo (${alith.address})`);
    await send('sudo.sudoUncheckedWeight(system.setCode)', api.tx.sudo.sudoUncheckedWeight(api.tx.system.setCode(code), { refTime: 0, proofSize: 0 }), alith);
    await new Promise((r) => setTimeout(r, 8000));
    await api.disconnect();
    api = await ApiPromise.create({ provider: new WsProvider(RPC, 2000), noInitWarn: true });
    log(`after upgrade: spec ${api.runtimeVersion.specVersion} | launchpad pallet: ${!!api.tx.launchpad}`);
  }
}
if (!api.tx.launchpad) { log('no launchpad pallet on chain; stop'); process.exit(1); }

// ---- 1. params & create ---------------------------------------------------
const params = (await api.query.launchpad.params()).toHuman();
log('launch params:', JSON.stringify(params));
const consts = api.consts.launchpad;
log(`constants: supply ${fmt(consts.totalSupply)} sellable ${fmt(consts.sellable)} floor ${fmt(consts.virtualTokenFloor)} assetBase ${consts.launchAssetBase.toString()} pallet index ${api.runtimeMetadata.asLatest.pallets.find(p => p.name.toString() === 'Launchpad').index}`);

const nextId = (await api.query.launchpad.nextLaunchId()).toNumber();
const c = await send('launchpad.create_launch', api.tx.launchpad.createLaunch('Dev Launch', 'DLNCH', null, 0, 0, null, null), baltathar);
const created = c.events.find((e) => e.section === 'launchpad' && e.method === 'LaunchCreated');
const launchId = Number(String(created.data.id).replace(/,/g, ''));
const assetId = BigInt(String(created.data.assetId).replace(/,/g, ''));
log(`  launch id ${launchId} (expected ${nextId}) asset id ${assetId} (= 2^64 + ${assetId - (1n << 64n)})`);
const launch = (await api.query.launchpad.launches(launchId)).unwrap();
const escrow = launch.escrow.toString();
log(`  escrow ${escrow} | asset supply ${fmt(await api.query.assets.asset(assetId).then(a => a.unwrap().supply))} | metadata ${JSON.stringify((await api.query.assets.metadata(assetId)).toHuman())}`);

const state = async () => (await api.query.launchpad.curves(launchId)).unwrap();
const show = async (tag) => { const s = await state(); log(`  [${tag}] phase ${s.phase} realQuote ${fmt(s.realQuote)} VTRS tokensRemaining ${fmt(s.tokensRemaining)} creatorUnclaimed ${fmt(s.creatorFeesUnclaimed)} protocolPaid ${fmt(s.protocolFeesPaid)}`); };
await show('created');

// ---- 2. trade along the curve ---------------------------------------------
const tokBal = async (who) => (await api.query.assets.account(assetId, who)).unwrapOr(null)?.balance.toBigInt() ?? 0n;
for (const [who, name, amt] of [[baltathar, 'baltathar', 10n], [alith, 'alith', 100n], [baltathar, 'baltathar', 500n]]) {
  const before = await tokBal(who.address);
  const r = await send(`launchpad.buy(${amt} VTRS) by ${name}`, api.tx.launchpad.buy(launchId, (amt * UNIT).toString(), 0), who);
  const bought = r.events.find((e) => e.section === 'launchpad' && e.method === 'Bought');
  log(`    got ${fmt((await tokBal(who.address)) - before)} DLNCH (event tokensOut ${bought.data.tokensOut})`);
  await show(`after buy ${amt}`);
}
{
  const held = await tokBal(alith.address);
  const sellAmt = held / 2n;
  const r = await send(`launchpad.sell(${fmt(sellAmt)} DLNCH) by alith`, api.tx.launchpad.sell(launchId, sellAmt.toString(), 0), alith);
  const sold = r.events.find((e) => e.section === 'launchpad' && e.method === 'Sold');
  log(`    quoteOut ${sold.data.quoteOut} fee ${sold.data.fee}`);
  await show('after sell');
}

// ---- 3. cross the threshold -----------------------------------------------
const s0 = await state();
const pair = [{ Native: null }, { WithId: assetId.toString() }];
log(`pool exists before crossing: ${(await api.query.vitreusDex.pools(pair)).isSome}`);
const cr = await send('launchpad.buy(500000 VTRS, crossing) by baltathar', api.tx.launchpad.buy(launchId, (500_000n * UNIT).toString(), 0), baltathar);
const names = cr.events.map((e) => `${e.section}.${e.method}`);
log(`  crossing events: ${names.filter((n) => n.startsWith('launchpad') || n.startsWith('vitreusDex') || n.startsWith('assets')).join(', ')}`);
const deferred = cr.events.find((e) => e.section === 'launchpad' && e.method === 'GraduationDeferred');
if (deferred) { log(`  GRADUATION DEFERRED: ${JSON.stringify(deferred.data)}`); }
await show('after crossing');

// ---- 4. verify the pool ---------------------------------------------------
const poolOpt = await api.query.vitreusDex.pools(pair);
if (!poolOpt.isSome) { log('NO POOL — graduation did not seed'); process.exit(2); }
const pool = poolOpt.unwrap();
const poolAcct = pool.poolAccount.toString();
const total = (await api.query.vitreusDex.totalLiquidity(pair)).unwrap();
const pos = (await api.query.vitreusDex.liquidityPositions(escrow, pair)).unwrap();
const s1 = await state();
log(`pool (Native, ${assetId}): reserveA ${fmt(pool.reserveA)} VTRS reserveB ${fmt(pool.reserveB)} DLNCH feeTier ${pool.feeTier} account ${poolAcct}`);
log(`  pool account balances: ${fmt((await api.query.system.account(poolAcct)).data.free)} VTRS, ${fmt(await tokBal(poolAcct))} DLNCH`);
log(`  totalLiquidity ${total.toString()} | escrow position shares ${pos.shares.toString()} lockedUntil ${pos.lockedUntil.toString()} | curve lpShares ${s1.lpShares.toString()}`);
log(`  escrow after: ${fmt((await api.query.system.account(escrow)).data.free)} VTRS free, ${fmt(await tokBal(escrow))} DLNCH`);
// opening price vs curve end price: (V_q + R)/VT_FLOOR
const vq = launch.curve.virtualQuote.toBigInt();
const floor = consts.virtualTokenFloor.toBigInt();
const R = pool.reserveA.toBigInt();
const lhs = R * floor; const rhs = pool.reserveB.toBigInt() * (vq + R);
const diff = lhs > rhs ? lhs - rhs : rhs - lhs;
log(`  opening price vs p_end: relative diff ${(Number(diff * 10n ** 12n / rhs) / 1e12).toExponential(2)} (I7 requires <= 1e-6)`);
log(`  raised ${fmt(R)} VTRS vs T ${fmt(launch.curve.graduationTarget)} | pre-crossing realQuote ${fmt(s0.realQuote)}`);

// ---- 5. the pool trades ---------------------------------------------------
const before = await tokBal(alith.address);
const sw = await send('vitreusDex.swap 1 VTRS -> DLNCH by alith', api.tx.vitreusDex.swapExactTokensForTokens({ Native: null }, { WithId: assetId.toString() }, UNIT.toString(), 0, alith.address), alith);
log(`    received ${fmt((await tokBal(alith.address)) - before)} DLNCH`);
const pool2 = (await api.query.vitreusDex.pools(pair)).unwrap();
log(`  pool after swap: reserveA ${fmt(pool2.reserveA)} reserveB ${fmt(pool2.reserveB)} feesCollected ${pool2.totalFeesCollected.toString()}`);
// creator claims fees
const claim = await send('launchpad.claim_creator_fees by baltathar', api.tx.launchpad.claimCreatorFees(launchId), baltathar);
log('done');
process.exit(0);
