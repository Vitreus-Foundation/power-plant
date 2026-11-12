#!/usr/bin/env bash
# try-runtime checks for the CI workflows. Run from the repository root:
#
#     .github/scripts/try-runtime.sh install DIR
#     .github/scripts/try-runtime.sh build NETWORK
#     .github/scripts/try-runtime.sh check NETWORK [--strict]
#
# `install` puts the try-runtime-cli release these checks are written for into DIR. `build` builds
# the runtime of NETWORK, mainnet or testnet, with the try-runtime feature. `check` runs the
# migrations of that build on the current state of NETWORK, with the try-runtime found on PATH.
# --strict also requires spec_version to be above the one on chain, as in a release PR.
set -euo pipefail

CLI_VERSION=v0.10.1

usage() {
    echo "usage: $0 install DIR | build NETWORK | check NETWORK [--strict]" >&2
    exit 2
}

known_network() {
    case $1 in
        mainnet | testnet) ;;
        *) usage ;;
    esac
}

install_cli() {
    mkdir -p "$1"
    curl -sfL --retry 3 -o "$1/try-runtime" \
        "https://github.com/paritytech/try-runtime-cli/releases/download/$CLI_VERSION/try-runtime-x86_64-unknown-linux-musl"
    chmod +x "$1/try-runtime"
}

build() {
    cargo build --release --locked -p vitreus-power-plant-runtime --features "try-runtime,$1-runtime"
}

check() {
    local network=$1 uri
    case $network in
        mainnet) uri=wss://rpc-mainnet.vtrs.io:443 ;;
        testnet) uri=wss://rpc.testnet.compliq.io:9945 ;;
    esac
    # try-runtime wants spec_version above the one on chain, and only a release PR raises it.
    local version_check=(--disable-spec-version-check)
    if [ "${2:-}" = --strict ]; then version_check=(); fi

    export RUST_LOG=try-runtime::cli=info,remote-ext=info,runtime=debug
    local snap=target/try-runtime/$network.snap
    mkdir -p "${snap%/*}"
    # Only the download talks to the node, so only it is retried: a failed check never is. It takes
    # the last finalized block and logs its hash. Archive nodes keep the state of every block, so
    # `create-snapshot --at <hash>` gets the same state again.
    local attempt
    for attempt in 1 2 3 4 5; do
        try-runtime create-snapshot --uri "$uri" -- "$snap" && break
        if [ "$attempt" -eq 5 ]; then return 1; fi
        sleep 10
    done

    # --disable-mbm-checks: the runtime has no pallet-migrations, so no multi-block migrations.
    # --blocktime: MILLISECS_PER_BLOCK of the runtime.
    try-runtime \
        --runtime "target/release/wbuild/vitreus-power-plant-runtime/vitreus_power_plant_${network}_runtime.compact.compressed.wasm" \
        on-runtime-upgrade "${version_check[@]}" --checks=all --disable-mbm-checks --blocktime 6000 \
        snap --path "$snap"
}

# The arguments are checked before anything is downloaded or built.
case ${1:-} in
    install)
        [ $# -eq 2 ] || usage
        install_cli "$2"
        ;;
    build)
        [ $# -eq 2 ] || usage
        known_network "$2"
        build "$2"
        ;;
    check)
        [ $# -eq 2 ] || { [ $# -eq 3 ] && [ "$3" = --strict ]; } || usage
        known_network "$2"
        check "$2" "${3:-}"
        ;;
    *) usage ;;
esac
