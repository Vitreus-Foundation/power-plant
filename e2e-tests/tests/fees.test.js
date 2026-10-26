import { afterAll, beforeAll, expect, it } from 'vitest';
import {
  UNIT,
  VNRG_ASSET_ID,
  alith,
  extrinsicEvents,
  freshAccount,
  mulPermill,
  setupFork,
  treasuryAccount,
} from './helpers.js';

// `FixedU128`, the type of the fee multiplier, has 18 decimals.
const MULTIPLIER_ONE = 10n ** 18n;

let fork;

beforeAll(async () => {
  fork = await setupFork();
});

afterAll(async () => {
  await fork?.teardown();
});

it('signed transfer pays flat fee in VNRG', async () => {
  const { api, dev } = fork;
  const receiver = freshAccount();
  const treasury = treasuryAccount(api);
  const amount = UNIT;

  const state = async (blockHash) => {
    const { query } = await api.at(blockHash);
    const [alithAccount, alithVnrg, receiverAccount, treasuryVnrg, vnrgAsset, burned, nac] =
      await Promise.all([
        query.system.account(alith.address),
        query.assets.account(VNRG_ASSET_ID, alith.address),
        query.system.account(receiver.address),
        query.assets.account(VNRG_ASSET_ID, treasury),
        query.assets.asset(VNRG_ASSET_ID),
        query.energyFee.burnedEnergy(),
        query.nacManaging.usersNft(receiver.address),
      ]);
    return {
      alithVtrs: alithAccount.data.free.toBigInt(),
      alithVnrg: alithVnrg.unwrap().balance.toBigInt(),
      receiverVtrs: receiverAccount.data.free.toBigInt(),
      treasuryVnrg: treasuryVnrg.unwrapOrDefault().balance.toBigInt(),
      vnrgSupply: vnrgAsset.unwrap().supply.toBigInt(),
      burned: burned.toBigInt(),
      receiverNacLevel: nac.isSome ? nac.unwrap()[1].toNumber() : undefined,
    };
  };

  const parentHash = (await api.rpc.chain.getHeader()).hash;
  const { query } = await api.at(parentHash);
  // A Balances call pays the flat fee: the fee multiplier times the base fee, rounded down.
  const multiplier = (await query.transactionPayment.nextFeeMultiplier()).toBigInt();
  const baseFee = (await query.energyFee.baseFee()).toBigInt();
  const fee = (multiplier * baseFee) / MULTIPLIER_ONE;
  // The treasury gets the fee recycling rate of the fee; the rest is burned.
  const recycled = mulPermill(fee, (await query.treasuryExtension.feeRecyclingRate()).toBigInt());
  const before = await state(parentHash);

  const txHash = await api.tx.balances
    .transferKeepAlive(receiver.address, amount)
    .signAndSend(alith);
  const blockHash = await dev.newBlock();
  const events = await extrinsicEvents(api, blockHash, txHash);
  const after = await state(blockHash);

  const event = (kind) => {
    const found = events.find((e) => kind.is(e));
    expect(found, kind.meta.name.toString()).toBeDefined();
    return found.data;
  };
  expect(events.some((e) => api.events.system.ExtrinsicSuccess.is(e))).toBe(true);

  const paid = event(api.events.energyFee.EnergyFeePaid);
  expect(paid.who.toString()).toBe(alith.address);
  expect(paid.amount.toBigInt()).toBe(fee);

  const withdrawn = event(api.events.assets.Withdrawn);
  expect(withdrawn.assetId.toNumber()).toBe(VNRG_ASSET_ID);
  expect(withdrawn.who.toString()).toBe(alith.address);
  expect(withdrawn.amount.toBigInt()).toBe(fee);

  const deposited = event(api.events.assets.Deposited);
  expect(deposited.assetId.toNumber()).toBe(VNRG_ASSET_ID);
  expect(deposited.who.toString()).toBe(treasury);
  expect(deposited.amount.toBigInt()).toBe(recycled);

  const transfer = event(api.events.balances.Transfer);
  expect(transfer.from.toString()).toBe(alith.address);
  expect(transfer.to.toString()).toBe(receiver.address);
  expect(transfer.amount.toBigInt()).toBe(amount);

  expect(event(api.events.system.NewAccount).account.toString()).toBe(receiver.address);
  expect(event(api.events.nacManaging.NftMinted).owner.toString()).toBe(receiver.address);

  // VTRS goes only to the transfer.
  expect(after.alithVtrs).toBe(before.alithVtrs - amount);
  expect(after.receiverVtrs).toBe(amount);
  expect(after.alithVnrg).toBe(before.alithVnrg - fee);
  expect(after.treasuryVnrg).toBe(before.treasuryVnrg + recycled);
  // `BurnedEnergy` starts from zero in every block, and no other fee is paid in this one.
  expect(after.burned).toBe(fee - recycled);
  expect(after.vnrgSupply).toBe(before.vnrgSupply - (fee - recycled));
  // The receiver is created by the transfer, so `OnNewAccount` gives it NAC level 1.
  expect(before.receiverNacLevel).toBeUndefined();
  expect(after.receiverNacLevel).toBe(1);
});
