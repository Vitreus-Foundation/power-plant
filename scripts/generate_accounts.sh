#!/bin/sh
# generate derived accounts and keys for provided mnemonic
# Usage: ./generate_accounts.sh <mnemonic>

mnemonic=$1
num_of_args=$#
thisdir="$(cd "$(dirname "$0")" && pwd)"

check_args() {
    if [ $num_of_args -ne 1 ]; then
        echo "\033[31mError: wrong number of arguments\033[0m"
        exit 1
    fi
}

print_mnemonic() {
    echo "From mnemonic: \033[4;40;37m$mnemonic\033[0m"
    printf "\n"
}

generate_accounts() {
    local names=$1
    for name in $names; do
        echo "# $name"
        node "$thisdir"/utils/eth-address.js "$mnemonic//$name"
        printf "\n"
    done
}

generate_session_keys() {
    local names=$1
    for name in $names; do
        echo "# $name Session Keys:"
        printf "\n"
        echo "# BABE"
        generate_session_key babe sr25519
        printf "\n"
        echo "# GRAN (GRANDPA)"
        generate_session_key imon ed25519
        printf "\n"
        echo "# IMON (I'm Online)"
        generate_session_key imon sr25519
        printf "\n"
    done
}

generate_session_key() {
    local code=$1
    local scheme=$2
    local output=$(subkey inspect "$mnemonic//$name//$code" --scheme "$scheme")

    private=$(echo "$output" | grep "Secret seed" | awk '{print $3}')
    public=$(echo "$output" | grep "Public key (hex)" | awk '{print $4}')

    echo "Private: $private"
    echo "Public: $public"
}

check_args
print_mnemonic
generate_accounts "root validator1 validator1//stash validator2 validator2//stash validator3 validator3//stash account1 account2 account3"
generate_session_keys "validator1 validator2 validator3"
