import { afterAll, beforeAll, expect, it } from 'vitest';
import {
  UNIT,
  VNRG_ASSET_ID,
  alith,
  baltathar,
  brokerAccount,
  dispatchErrors,
  extrinsicEvents,
  freshAccount,
  mulPermill,
  setStorage,
  setupFork,
  treasuryAccount,
  vnrgOf,
  vtrsOf,
} from './helpers.js';

// `FixedU128`, the type of the fee multiplier, has 18 decimals.
const MULTIPLIER_ONE = 10n ** 18n;

// The session hooks move the amounts the tests expect: the treasury swaps its energy, the broker
// burns its VNRG above capacity, and the energy price changes.
const NEW_SESSION = 'the block starts a new session, whose hooks move the amounts; re-run';

let fork;

beforeAll(async () => {
  fork = await setupFork();
});

afterAll(async () => {
  await fork?.teardown();
});

// A Balances call pays the flat fee: the fee multiplier times the base fee, rounded down. The
// treasury gets the fee recycling rate of the fee; the rest is burned.
async function flatFee(query) {
  const multiplier = (await query.transactionPayment.nextFeeMultiplier()).toBigInt();
  const baseFee = (await query.energyFee.baseFee()).toBigInt();
  const fee = (multiplier * baseFee) / MULTIPLIER_ONE;
  const recycled = mulPermill(fee, (await query.treasuryExtension.feeRecyclingRate()).toBigInt());
  return { fee, recycled };
}

function findEvent(events, kind) {
  const found = events.find((event) => kind.is(event));
  expect(found, kind.meta.name.toString()).toBeDefined();
  return found.data;
}

it('signed transfer pays flat fee in VNRG', async () => {
  const { api, dev } = fork;
  const receiver = freshAccount();
  const treasury = treasuryAccount(api);
  const amount = UNIT;

  const state = async (blockHash) => {
    const { query } = await api.at(blockHash);
    const nac = await query.nacManaging.usersNft(receiver.address);
    return {
      session: (await query.session.currentIndex()).toNumber(),
      senderVtrs: await vtrsOf(query, alith.address),
      senderVnrg: await vnrgOf(query, alith.address),
      receiverVtrs: await vtrsOf(query, receiver.address),
      treasuryVnrg: (await vnrgOf(query, treasury)) ?? 0n,
      vnrgSupply: (await query.assets.asset(VNRG_ASSET_ID)).unwrap().supply.toBigInt(),
      burned: (await query.energyFee.burnedEnergy()).toBigInt(),
      receiverNacLevel: nac.isSome ? nac.unwrap()[1].toNumber() : undefined,
    };
  };

  const parentHash = (await api.rpc.chain.getHeader()).hash;
  const { fee, recycled } = await flatFee((await api.at(parentHash)).query);
  const before = await state(parentHash);

  const txHash = await api.tx.balances
    .transferKeepAlive(receiver.address, amount)
    .signAndSend(alith);
  const blockHash = await dev.newBlock();
  const events = await extrinsicEvents(api, blockHash, txHash);
  const after = await state(blockHash);

  expect(after.session, NEW_SESSION).toBe(before.session);
  expect(dispatchErrors(api, events)).toEqual([]);

  const paid = findEvent(events, api.events.energyFee.EnergyFeePaid);
  expect(paid.who.toString()).toBe(alith.address);
  expect(paid.amount.toBigInt()).toBe(fee);

  const withdrawn = findEvent(events, api.events.assets.Withdrawn);
  expect(withdrawn.assetId.toNumber()).toBe(VNRG_ASSET_ID);
  expect(withdrawn.who.toString()).toBe(alith.address);
  expect(withdrawn.amount.toBigInt()).toBe(fee);

  const deposited = findEvent(events, api.events.assets.Deposited);
  expect(deposited.assetId.toNumber()).toBe(VNRG_ASSET_ID);
  expect(deposited.who.toString()).toBe(treasury);
  expect(deposited.amount.toBigInt()).toBe(recycled);

  const transfer = findEvent(events, api.events.balances.Transfer);
  expect(transfer.from.toString()).toBe(alith.address);
  expect(transfer.to.toString()).toBe(receiver.address);
  expect(transfer.amount.toBigInt()).toBe(amount);

  expect(findEvent(events, api.events.system.NewAccount).account.toString()).toBe(
    receiver.address,
  );
  expect(findEvent(events, api.events.nacManaging.NftMinted).owner.toString()).toBe(
    receiver.address,
  );

  // VTRS goes only to the transfer.
  expect(after.senderVtrs).toBe(before.senderVtrs - amount);
  expect(after.receiverVtrs).toBe(amount);
  expect(after.senderVnrg).toBe(before.senderVnrg - fee);
  expect(after.treasuryVnrg).toBe(before.treasuryVnrg + recycled);
  // `BurnedEnergy` starts from zero in every block, and no other fee is paid in this one.
  expect(after.burned).toBe(fee - recycled);
  expect(after.vnrgSupply).toBe(before.vnrgSupply - (fee - recycled));
  // The receiver is created by the transfer, so `OnNewAccount` gives it NAC level 1.
  expect(before.receiverNacLevel).toBeUndefined();
  expect(after.receiverNacLevel).toBe(1);
});

it('VTRS-only sender buys fee energy from broker', async () => {
  const { api, dev } = fork;
  const receiver = freshAccount();
  const treasury = treasuryAccount(api);
  const broker = brokerAccount(api);
  const amount = UNIT;
  // The broker sells the energy from its own VNRG; give it enough, whatever it holds on chain.
  await setStorage(fork, {
    Assets: {
      Account: [
        [[VNRG_ASSET_ID, broker], { balance: (100n * UNIT).toString(), reason: 'Consumer' }],
      ],
    },
  });

  const state = async (blockHash) => {
    const { query } = await api.at(blockHash);
    return {
      session: (await query.session.currentIndex()).toNumber(),
      senderVtrs: await vtrsOf(query, baltathar.address),
      senderVnrg: await vnrgOf(query, baltathar.address),
      receiverVtrs: await vtrsOf(query, receiver.address),
      brokerVtrs: await vtrsOf(query, broker),
      brokerVnrg: await vnrgOf(query, broker),
      treasuryVtrs: await vtrsOf(query, treasury),
      treasuryVnrg: (await vnrgOf(query, treasury)) ?? 0n,
      vnrgSupply: (await query.assets.asset(VNRG_ASSET_ID)).unwrap().supply.toBigInt(),
      burned: (await query.energyFee.burnedEnergy()).toBigInt(),
    };
  };

  const parentHash = (await api.rpc.chain.getHeader()).hash;
  const parent = await api.at(parentHash);
  const { fee, recycled } = await flatFee(parent.query);
  // The VTRS the broker charges for the fee energy, with or without its swap fee.
  const quote = async (includeFee) => {
    const price = await parent.call.energyBrokerApi.quotePriceTokensForExactTokens(
      null,
      'Native',
      { WithId: VNRG_ASSET_ID },
      fee,
      includeFee,
    );
    expect(price.isSome, 'the broker quotes the fee energy').toBe(true);
    return price.unwrap().toBigInt();
  };
  const price = await quote(true);
  const swapFee = price - (await quote(false));
  const before = await state(parentHash);

  const txHash = await api.tx.balances
    .transferKeepAlive(receiver.address, amount)
    .signAndSend(baltathar);
  const blockHash = await dev.newBlock();
  const events = await extrinsicEvents(api, blockHash, txHash);
  const after = await state(blockHash);

  expect(after.session, NEW_SESSION).toBe(before.session);
  expect(dispatchErrors(api, events)).toEqual([]);

  const swap = findEvent(events, api.events.energyBroker.SwapExecuted);
  expect(swap.who.toString()).toBe(baltathar.address);
  expect(swap.recipient.toString()).toBe(baltathar.address);
  expect(swap.path[0].isNative).toBe(true);
  expect(swap.path[1].asWithId.toNumber()).toBe(VNRG_ASSET_ID);
  expect(swap.amountIn.toBigInt()).toBe(price);
  expect(swap.amountOut.toBigInt()).toBe(fee);

  const paid = findEvent(events, api.events.energyFee.EnergyFeePaid);
  expect(paid.who.toString()).toBe(baltathar.address);
  expect(paid.amount.toBigInt()).toBe(fee);

  const transfer = findEvent(events, api.events.balances.Transfer);
  expect(transfer.from.toString()).toBe(baltathar.address);
  expect(transfer.to.toString()).toBe(receiver.address);
  expect(transfer.amount.toBigInt()).toBe(amount);

  // Baltathar pays the transfer and the energy in VTRS and ends without VNRG: it buys only the fee.
  expect(after.senderVtrs).toBe(before.senderVtrs - amount - price);
  expect(before.senderVnrg).toBeUndefined();
  expect(after.senderVnrg).toBeUndefined();
  expect(after.receiverVtrs).toBe(amount);
  // The broker keeps the price without the swap fee and gives away the fee energy.
  expect(after.brokerVtrs).toBe(before.brokerVtrs + price - swapFee);
  expect(after.brokerVnrg).toBe(before.brokerVnrg - fee);
  // The treasury gets the swap fee in VTRS and the recycled part of the fee in VNRG.
  expect(after.treasuryVtrs).toBe(before.treasuryVtrs + swapFee);
  expect(after.treasuryVnrg).toBe(before.treasuryVnrg + recycled);
  expect(after.burned).toBe(fee - recycled);
  expect(after.vnrgSupply).toBe(before.vnrgSupply - (fee - recycled));
});
