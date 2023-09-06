#!/bin/sh
# runs localnet

node=./target/debug/vitreus-power-plant-node

cargo build

$node --chain localnet --force-authoring --rpc-cors=all --alice --tmp --node-key 0000000000000000000000000000000000000000000000000000000000000001 &
    $node --chain localnet --force-authoring --rpc-cors=all --bob --tmp --port 30334 --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
