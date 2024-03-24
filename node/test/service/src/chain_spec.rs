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

//! Chain specifications for the test runtime.

use chain_spec::{
    devnet_keys::{authority_keys_from_seed, get_account_id_from_seed},
    Extensions,
};
use sc_chain_spec::{ChainSpec, ChainType, Properties};
use sp_core::ecdsa;
use vitreus_power_plant_runtime::{AccountId, SS58Prefix, WASM_BINARY};

/// The `ChainSpec` parameterized for polkadot test runtime.
pub type PolkadotChainSpec =
    sc_service::GenericChainSpec<vitreus_power_plant_runtime::RuntimeGenesisConfig, Extensions>;

/// Local testnet config (multivalidator Alice + Bob)
pub fn polkadot_local_testnet_config() -> PolkadotChainSpec {
    PolkadotChainSpec::from_genesis(
        "Local Testnet",
        "local_testnet",
        ChainType::Local,
        || polkadot_local_testnet_genesis(),
        vec![],
        None,
        None,
        None,
        Some(properties()),
        Default::default(),
    )
}

/// Local testnet genesis config (multivalidator Alice + Bob)
pub fn polkadot_local_testnet_genesis() -> vitreus_power_plant_runtime::RuntimeGenesisConfig {
    chain_spec::testnet_genesis(
        &WASM_BINARY.expect("WASM not available"),
        get_account_id_from_seed::<ecdsa::Public>("Alice"),
        testnet_accounts(),
        vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
        vec![],
        SS58Prefix::get() as u64,
    )
}

fn testnet_accounts() -> Vec<AccountId> {
    vec![
        get_account_id_from_seed::<ecdsa::Public>("Alice"),
        get_account_id_from_seed::<ecdsa::Public>("Bob"),
        get_account_id_from_seed::<ecdsa::Public>("Charlie"),
        get_account_id_from_seed::<ecdsa::Public>("Dave"),
        get_account_id_from_seed::<ecdsa::Public>("Eve"),
        get_account_id_from_seed::<ecdsa::Public>("Ferdie"),
        get_account_id_from_seed::<ecdsa::Public>("Alice//stash"),
        get_account_id_from_seed::<ecdsa::Public>("Bob//stash"),
        get_account_id_from_seed::<ecdsa::Public>("Charlie//stash"),
        get_account_id_from_seed::<ecdsa::Public>("Dave//stash"),
        get_account_id_from_seed::<ecdsa::Public>("Eve//stash"),
        get_account_id_from_seed::<ecdsa::Public>("Ferdie//stash"),
    ]
}

fn properties() -> Properties {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "VTRS".into());
    properties.insert("tokenDecimals".into(), 18.into());
    properties.insert("ss58Format".into(), SS58Prefix::get().into());
    properties
}

/// Can be called for a `Configuration` to check if it is a configuration for the `Test` network.
pub trait IdentifyVariant {
    /// Returns if this is a configuration for the `Test` network.
    fn is_test(&self) -> bool;
}

impl IdentifyVariant for Box<dyn ChainSpec> {
    fn is_test(&self) -> bool {
        self.id().starts_with("test")
    }
}
