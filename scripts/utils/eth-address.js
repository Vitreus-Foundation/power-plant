// generate ethereum key pair from a subkey mnemonic

import { pbkdf2Sync } from "node:crypto";
import { Keyring } from "@polkadot/keyring";

const KEYRING_ETH = new Keyring({ type: "ethereum" });

await (async function() {
    const mnemonic = parseArgs();

    const seed = mnemonicToSeed(mnemonic);
    console.log(`Private: 0x${seed.toString("hex")}`);
    console.log(`Address: ${KEYRING_ETH.addFromSeed(seed).address}`);
})();

// The first 32 bytes of the BIP-39 seed of the whole argument, the `//name` suffix included, as
// `mnemonicToSeed` of @polkadot/util-crypto 2.x computed it.
function mnemonicToSeed(mnemonic) {
    return pbkdf2Sync(mnemonic.normalize("NFKD"), "mnemonic", 2048, 32, "sha512");
}

function parseArgs() {
    if (process.argv.length < 3) {
        console.error("Please provide a mnemonic.");
        process.exit(9);
    }

    return process.argv[2];
}
