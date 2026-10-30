#!/usr/bin/env node
// The host functions a runtime imports, checked against those every node provides. Run from the
// repository root:
//
//     node .github/scripts/host-functions.mjs list WASM
//     node .github/scripts/host-functions.mjs check WASM
//
// WASM is a compact wasm, not the compressed one: srtool's `wasm` output, or a runtime fetched with
// `subwasm get` and unpacked with `subwasm decompress`.
import { readFileSync } from 'node:fs';

// The host functions a runtime may import: every node on mainnet, testnet and stagenet provides
// them. A node that lacks one of the imported host functions can't load the runtime at all, so
// after the upgrade it stops importing blocks.
const SUPPORTED = '.github/supported-host-functions.txt';

function fail(message) {
  console.log(`::error::${message}`);
  process.exit(1);
}

// The host functions `wasm` imports; besides them, it may only import its memory.
function imported(wasm) {
  let module;
  try {
    module = new WebAssembly.Module(readFileSync(wasm));
  } catch (e) {
    fail(`couldn't read the imports of ${wasm}: ${e.message}`);
  }
  const functions = new Set();
  for (const i of WebAssembly.Module.imports(module)) {
    if (i.kind === 'memory' && i.module === 'env' && i.name === 'memory') continue;
    if (i.kind !== 'function' || i.module !== 'env') {
      fail(`${wasm} imports the ${i.kind} ${i.module}.${i.name}, which nodes don't provide`);
    }
    functions.add(i.name);
  }
  return functions;
}

// Prints the host functions `wasm` imports in the format of SUPPORTED.
function list(wasm) {
  console.log([...imported(wasm)].sort().join('\n'));
}

function check(wasm) {
  const supported = new Set(
    readFileSync(SUPPORTED, 'utf8')
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line && !line.startsWith('#')),
  );
  const functions = imported(wasm);
  const missing = [...functions].filter((name) => !supported.has(name)).sort();
  if (missing.length === 0) {
    console.log(`${wasm} imports ${functions.size} host functions, all in ${SUPPORTED}`);
    return;
  }
  for (const name of missing) {
    console.log(`::error::${wasm} imports ${name}, which isn't in ${SUPPORTED}`);
  }
  fail(
    `add a host function to ${SUPPORTED} only once every node on mainnet, testnet and stagenet ` +
      `has it, see "Releases" in README.md`,
  );
}

const [command, wasm] = process.argv.slice(2);
if (command === 'list' && wasm) {
  list(wasm);
} else if (command === 'check' && wasm) {
  check(wasm);
} else {
  fail('usage: node .github/scripts/host-functions.mjs list|check WASM');
}
