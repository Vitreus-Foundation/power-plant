#!/bin/bash
# adds session keys to the node
# it's supposed that node is already built and running

num_of_args=$#
envs="$1"
prefix="$2"
rpc_api_endpoint="$3"

check_args() {
    if [ $num_of_args -ne 3 ]; then
        echo "\033[31mError: wrong number of arguments\033[0m"
        usage
        exit 1
    fi
}

usage() {
    echo "Usage: ./add-session-keys.sh <ENVS_FILE> <PREFIX> <RPC_API_ENDPOINT>"
    echo "\t<ENVS_FILE>        contains the environment variables with session keys"
    echo "\t<PREFIX>           the prefix of the environment variables in the envs file"
    echo "\t<RPC_API_ENDPOINT> the URL to connect to the node via RPC"
    printf "\n\033[31m"
    echo "The envs file should contain the variables:"
    echo "\t<PREFIX><PRIVATE, PUBLIC>_<GRAN, BABE, IMON, PARA, ASGN, AUDI, BEEF>"
    printf "\033[0m\n"
}

load_envs() {
    source "$envs"
    
    # check all variables are loaded
    local envs_postfix="PRIVATE_GRAN PUBLIC_GRAN PRIVATE_BABE PUBLIC_BABE PRIVATE_IMON PUBLIC_IMON PRIVATE_PARA PUBLIC_PARA PRIVATE_ASGN PUBLIC_ASGN PRIVATE_AUDI PUBLIC_AUDI PRIVATE_BEEF PUBLIC_BEEF"
    
    for postfix in $envs_postfix; do
        local variable_name="${prefix}${postfix}"

        if [[ -z "${!variable_name}" ]]; then
            echo "\033[31mError: ${variable_name} is not set\033[0m"
            exit 1
        fi
    done
}

check_availability() {
    local retry_count=0
    local max_retries=30
    local retry_interval=5

    while [ $retry_count -lt $max_retries ]; do
        # Use curl to test the connection without making an actual request
        curl --connect-timeout 5 "$rpc_api_endpoint" 2>/dev/null
        
        # Check the exit status of curl
        if [ $? -eq 0 ]; then
            echo "Connected to $rpc_api_endpoint"
            break
        else
            echo "$rpc_api_endpoint is not available. Retrying in $retry_interval seconds..."
            sleep $retry_interval
            ((retry_count++))
        fi
    done
    
    if [ $retry_count -eq $max_retries ]; then
        echo "\033[31mError: Couldn't connect to $rpc_api_endpoint\033[0m"
        kill $$
    fi
}

add_session_keys() {
    local private_gran="${prefix}PRIVATE_GRAN"
    local public_gran="${prefix}PUBLIC_GRAN"

    local private_babe="${prefix}PRIVATE_BABE"
    local public_babe="${prefix}PUBLIC_BABE"

    local private_imon="${prefix}PRIVATE_IMON"
    local public_imon="${prefix}PUBLIC_IMON"

    local private_para="${prefix}PRIVATE_PARA"
    local public_para="${prefix}PUBLIC_PARA"

    local private_asgn="${prefix}PRIVATE_ASGN"
    local public_asgn="${prefix}PUBLIC_ASGN"

    local private_audi="${prefix}PRIVATE_AUDI"
    local public_audi="${prefix}PUBLIC_AUDI"

    local private_beef="${prefix}PRIVATE_BEEF"
    local public_beef="${prefix}PUBLIC_BEEF"

    add_key "gran" "${!private_gran}" "${!public_gran}" "$rpc_api_endpoint"
    add_key "babe" "${!private_babe}" "${!public_babe}" "$rpc_api_endpoint"
    add_key "imon" "${!private_imon}" "${!public_imon}" "$rpc_api_endpoint"
    add_key "para" "${!private_para}" "${!public_para}" "$rpc_api_endpoint"
    add_key "asgn" "${!private_asgn}" "${!public_asgn}" "$rpc_api_endpoint"
    add_key "audi" "${!private_audi}" "${!public_audi}" "$rpc_api_endpoint"
    add_key "beef" "${!private_beef}" "${!public_beef}" "$rpc_api_endpoint"
}

add_key() {
    local key_type="$1"
    local private="$2"
    local public="$3"
    
    local request="{\
        \"jsonrpc\":\"2.0\",\
        \"id\":1,\
        \"method\":\"author_insertKey\",\
        \"params\": [ \"$key_type\", \"$private\", \"$public\" ]\
    }"

    echo "Adding '${key_type}' key to ${rpc_api_endpoint}"
    curl -H "Content-Type: application/json" -d "$request" "$rpc_api_endpoint"
}

check_args
load_envs
check_availability
add_session_keys

echo "Done adding session keys."
