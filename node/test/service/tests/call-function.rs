// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

use polkadot_test_service::*;
use sp_core::Pair;
use sp_keyring::Sr25519Keyring::{Alice};

// TODO: fix "bad signature"
#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore]
async fn call_function_actually_work() {
    let alice_config =
        node_config(|| {}, tokio::runtime::Handle::current(), Alice, Vec::new(), true);

    let alice = run_validator_node(alice_config, None);

    let function =
		vitreus_power_plant_runtime::RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
			dest: test_accounts::charlie().public().into(),
            value: 1,
        });
	let output = alice.send_extrinsic(function, test_accounts::bob()).await.unwrap();

    let res = output.result;
    let json = serde_json::from_str::<serde_json::Value>(res.as_str()).expect("valid JSON");
    let object = json.as_object().expect("JSON is an object");
    assert!(object.contains_key("jsonrpc"), "key jsonrpc exists");
    let result = object.get("result");
    let result = result.expect("key result exists");
    assert_eq!(result.as_str().map(|x| x.starts_with("0x")), Some(true), "result starts with 0x");
}
