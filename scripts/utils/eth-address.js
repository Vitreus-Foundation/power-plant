// generate ethereum key pair from a subkey mnemonic

import { Keyring } from "@polkadot/keyring";
import crypto from "@polkadot/util-crypto";

// TODO: simplify this using crypto primitives
const KEYRING_ETH = new Keyring({ type: "ethereum" });

await (async function() {
    const mnemonic = parseArgs();

    const seed = Buffer.from(crypto.mnemonicToSeed(mnemonic));
    console.log(`Private: 0x${seed.toString("hex")}`);
    console.log(`Address: ${KEYRING_ETH.addFromSeed(seed).address}`);
})();

function parseArgs() {
    if (process.argv.length < 3) {
        console.error("Please provide a mnemonic.");
        process.exit(9);
    }

    return process.argv[2];
}
