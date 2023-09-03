import { WebSocketProvider, Wallet } from 'ethers';
import assert from 'assert';


const API = new WebSocketProvider("ws://127.0.0.1:9944");
const GAS_PRICE = BigInt("1000000000");

await (async function main() {
    const accounts = initAccounts();
    let times = parseArgs();
    const timeout = 9000; // 9 seconds (3 blocks)

    while (times > 0) {
        const id = setTimeout(() => {
            console.error("Timeout");
            process.exit(1);
        }, timeout);

        console.log(`\nRemaining ${times} transactions...\n`);
        const tx = await sendRandomTransaction(accounts);
        await tx.tx.wait();

        clearTimeout(id);

        const [snbal, rnbal] = [
            await getBalance(tx.sender.address),
            await getBalance(tx.receiver.address)
        ];
        assert.strictEqual(snbal, tx.sbal - tx.value);
        assert.strictEqual(rnbal, tx.rbal + tx.value);
        times--;
    }
    console.log("\n\nAll done");
    process.exit(0);
})().catch((e) => { console.error(e); process.exit(1) });

function parseArgs() {
    if (process.argv.length < 3) {
        console.error("Usage: node eth_transactions.js <number of transactions>");
        process.exit(1);
    }

    return parseInt(process.argv[2]);
}

async function sendRandomTransaction(accounts) {
    let [sname, sender] = randomAccount(accounts);
    let value = await randomValueToSend(sender.address);
    while (value === 0) {
        [sname, sender] = randomAccount(accounts);
        value = await randomValueToSend(sender.address);
    }

    let [rname, receiver] = randomAccount(accounts);
    while (sender.address === receiver.address) {
        [rname, receiver] = randomAccount(accounts);
    }

    const [sbal, rbal] = [await getBalance(sender.address), await getBalance(receiver.address)];

    console.log(`Initial balances: ${sname} ${sbal}, ${rname} ${rbal}`);
    console.log(`Sending ${value} from ${sname} to ${rname}`);
    return {
        sender, receiver, sbal, rbal, value, tx: await sender.sendTransaction({
            to: receiver.address,
            gasPrice: GAS_PRICE, // Gas price in wei
            gasLimit: "65000000",
            value,
        })
    };
}

function initAccounts() {
    let initial = {};
    return [
        ["Alith", "0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133"],
        ["Baltathar", "0x8075991ce870b93a8870eca0c0f91913d12f47948ca0fd25b49c6fa7cdbeee8b"],
        ["Charleth", "0x0b6e18cafb6ed99687ec547bd28139cafdd2bffe70e6b688025de6b445aa5c5b"],
        ["Dorothy", "0x39539ab1876910bbf3a223d84a29e28f1cb4e2e456503e7e91ed39b2e7223d68"],
        ["Ethan", "0x7dce9bc8babb68fec1409be38c8e1a52650206a7ed90ff956ae8a6d15eeaaef4"],
        ["Faith", "0xb9d2ea9a615f3165812e8d44de0d24da9bbd164b65c4f0573e1ce2c8dbd9c8df"],
        ["Goliath", "0x96b8a38e12e1a31dee1eab2fffdf9d9990045f5b37e44d8cc27766ef294acf18"],
        // the next 3 doesn't have NAC
        ["Heath", "0x0d6dcaaef49272a5411896be8ad16c01c35d6f8c18873387b71fbc734759b0ab"],
        ["Ida", "0x4c42532034540267bf568198ccec4cb822a025da542861fcb146a5fab6433ff8"],
        ["Judith", "0x94c49300a58d576011096bcb006aa06f5a91b34b4383891e8029c21dc39fbb8b"],
    ].reduce((acc, [key, val]) => {
        acc[key] = new Wallet(val).connect(API);
        return acc;
    }, initial)
}

function randomAccount(accounts) {
    const keys = Object.keys(accounts);
    const randomIndex = Math.floor(Math.random() * keys.length);
    const key = keys[randomIndex];
    return [key, accounts[key]];
}

async function randomValueToSend(address) {
    const balance = await getBalance(address);
    let tries_num = 100;
    while (tries_num > 0) {
        // 10%-55% of the user's balance
        const precision = 1000000000;
        const prop = BigInt(Math.floor((Math.random() * 0.45 + 0.1) * precision));
        const res = balance / BigInt(precision) * prop;
        if (res > GAS_PRICE) {
            return res;
        }
        tries_num--;
    }

    return 0;
}

async function getBalance(address) {
    return API.getBalance(address);
}
