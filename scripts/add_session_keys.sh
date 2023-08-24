#!/bin/sh
# add session keys to a node's keyring
# Usage: add_session_keys.sh <rpc-api-endpoint> <keys-dir>

rpc_api_endpoint=$1
keys_dir=$2
num_of_args=$#

check_args() {
    if [ $num_of_args -ne 2 ]; then
        echo "\033[31mError: wrong number of arguments\033[0"
        exit 1
    fi
}

perform_request() {
    for file in $keys_dir/*; do
        curl -H "Content-Type: application/json" -d @$file $rpc_api_endpoint
        echo
    done
}

main() {
    check_args
    perform_request
    echo "Done"
}

main
