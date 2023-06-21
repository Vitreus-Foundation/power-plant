import {
    ALITH,
    BALTHAR,
    ERC20_BALANCES_CONTRACT_ADDR,
    callContract,
    encodeInput,
    initApi,
} from "./common.js"
import assert from 'node:assert/strict'

const API = await initApi()

async function run() {
    await testTransfer()
}

async function testTransfer() {
    const { data: { free: amount } } = await API.query.system.account(ALITH.address)
    assert.equal(amount.toHex(), "0x00000000000000001000000000000000")

    const input = encodeInput(
        "transfer(address,uint256)",
        ["address", "uint256"],
        [BALTHAR.address, "0x00000000000000001000000000000000"]
    )

    await callContract(API, ALITH, ERC20_BALANCES_CONTRACT_ADDR, input, async (result) => {
        if (result.status.isFinalized) {
            const { data: { free: amount } } = await API.query.system.account(ALITH.address)
            // including gas and log fee
            assert.equal(amount.toHex(), "0x00000000000000000ffffffff93a0479")
        }
    })
}

run().catch(console.error).then(() => process.exit(0))
