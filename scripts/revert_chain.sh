#!/bin/bash

LOG_TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
LOG_FILE="log_$LOG_TIMESTAMP.txt"

FLAG_FILE="/tmp/node/revert-chain-18.txt"

if [ -e "$FLAG_FILE" ]; then
    echo "Flag file already exists, operation will be skipped." | tee -a "$LOG_FILE"
    exit 0
fi

./target/release/vitreus-power-plant-node revert --base-path /tmp/node --chain ./target/release/vitreus-power-plant-mainnet.json 1024 &>> "$LOG_FILE"

if [ $? -eq 0 ]; then
    echo "revert-chain successfully completed." | tee -a "$LOG_FILE"
    touch "$FLAG_FILE"
else
    echo "Error while executing revert-chain." | tee -a "$LOG_FILE"
    exit 0
fi
