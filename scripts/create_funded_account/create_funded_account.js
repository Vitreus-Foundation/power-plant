// this script creates a new account with the provided amount of funds.
import { Keyring, ApiPromise, WsProvider } from "@polkadot/api"
import { ethers } from "ethers";

await (async function main() {
    const [url, amount] = await parseArgs();
    const api = await initApi(url);
    const keyring = new Keyring({ type: "ethereum" })
    // Alith is root, so we can force set balance with it
    const alith = keyring
        .addFromUri("0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133")
    const { address, privateKey } = createAccount();
    console.log("Account created.");
    console.log("Public key: ", address);
    console.log("Secret key: ", privateKey);

    await sendFunds(api, alith, address, amount);

    console.log("Funds sent.");

    api.disconnect();
})();


function parseArgs() {
    if (process.argv.length < 4) {
        console.error("Usage: node create_funded_account.js <ws-rpc-api-url> <amount>");
        return;
    }

    return [process.argv[2], parseFloat(process.argv[3])];
}

async function initApi(url) {
    const wsProvider = new WsProvider(url)

    return await ApiPromise.create({
        provider: wsProvider,
        initWasm: false,
        types: {
            // mapping the actual specified address format
            AccountId: "EthereumAccountId",
            Address: "AccountId",
            Balance: "u128",
            RefCount: "u8",
            LookupSource: "AccountId",
            Account: {
                nonce: "U256",
                balance: "u128",
            },
            EthTransaction: "LegacyTransaction",
            DispatchErrorModule: "DispatchErrorModuleU8",
            EthereumSignature: {
                r: "H256",
                s: "H256",
                v: "U8",
            },
            ExtrinsicSignature: "EthereumSignature",
            TxPoolResultContent: {
                pending: "HashMap<H160, HashMap<U256, PoolTransaction>>",
                queued: "HashMap<H160, HashMap<U256, PoolTransaction>>",
            },
            TxPoolResultInspect: {
                pending: "HashMap<H160, HashMap<U256, Summary>>",
                queued: "HashMap<H160, HashMap<U256, Summary>>",
            },
            TxPoolResultStatus: {
                pending: "U256",
                queued: "U256",
            },
            Summary: "Bytes",
            PoolTransaction: {
                hash: "H256",
                nonce: "U256",
                blockHash: "Option<H256>",
                blockNumber: "Option<U256>",
                from: "H160",
                to: "Option<H160>",
                value: "U256",
                gasPrice: "U256",
                gas: "U256",
                input: "Bytes",
            },
        }
    })
}

function createAccount() {
    const wallet = ethers.Wallet.createRandom();
    const address = wallet.address;
    const privateKey = wallet.privateKey;

    return { address, privateKey };
}

async function sendFunds(api, root, to, amount) {
    await api.tx.sudo.sudo(api.tx.balances.forceSetBalance(to, amount)).signAndSend(root);
}
