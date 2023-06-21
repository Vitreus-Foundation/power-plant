import { Keyring, ApiPromise, WsProvider } from "@polkadot/api"
import eip55 from "eip55"
import { encodeFunctionSignature, encodeParameters } from 'web3-eth-abi';

export async function initApi() {
    const wsProvider = new WsProvider("ws://127.0.0.1:9944")

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

function numToAddress(num) {
    const addressHex = num.toString(16) // Convert number to hexadecimal string
    // Pad the string to a length of 40 characters
    const addressHexPadded = addressHex.padStart(40, '0')

    return eip55.encode(addressHexPadded)
}

export function encodeInput(fnSelector, types, params) {
    const fnSignature = encodeFunctionSignature(fnSelector)
    const pars = encodeParameters(types, params)
    return fnSignature + pars.slice(2)
}

export async function callContract(api, alice, contractAddress, input, onResult) {
    const transaction = api.tx.evm.call(
        alice.address,
        contractAddress,
        input,
        0,
        1000000,
        1000000,
        null,
        null,
        []
    )
    const unsub = await transaction.signAndSend(alice, async (result) => {
        await onResult(result)
        if (result.status.isFinalized) unsub()
    })
}

const keyring = new Keyring({ type: "ethereum" })

export const ALITH = keyring
    .addFromUri("0x5fb92d6e98884f76de468fa3f6278f8807c48bebc13595d45af5bdc4da702133")
export const BALTHAR = keyring
    .addFromUri("0x8075991ce870b93a8870eca0c0f91913d12f47948ca0fd25b49c6fa7cdbeee8b")
export const CHARLETH = keyring
    .addFromUri("0x0b6e18cafb6ed99687ec547bd28139cafdd2bffe70e6b688025de6b445aa5c5b")

export const ERC20_BALANCES_CONTRACT_ADDR = numToAddress(2050)
