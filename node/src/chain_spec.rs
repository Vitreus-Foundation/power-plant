use std::{collections::BTreeMap, str::FromStr};

use hex_literal::hex;
use serde::{Deserialize, Serialize};
// Substrate
use sc_chain_spec::{ChainType, Properties};
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::ecdsa;
use sp_core::{storage::Storage, Pair, Public, H160, U256};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::Perbill;
use sp_state_machine::BasicExternalities;
// Frontier
use vitreus_power_plant_runtime::{
    opaque, AccountId, AssetsConfig, BabeConfig, Balance, BalancesConfig, EVMChainIdConfig,
    EVMConfig, EnableManualSeal, EnergyGenerationConfig, ImOnlineConfig, ImOnlineId,
    MaxCooperations, ReputationConfig, RuntimeGenesisConfig, SS58Prefix, SessionConfig, Signature,
    StakerStatus, SudoConfig, SystemConfig, BABE_GENESIS_EPOCH_CONFIG,
    COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD, VNRG, WASM_BINARY,
};

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Specialized `ChainSpec` for development.
pub type DevChainSpec = sc_service::GenericChainSpec<DevGenesisExt>;

fn alith() -> AccountId {
    AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac"))
}

fn baltathar() -> AccountId {
    AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0"))
}

fn charleth() -> AccountId {
    AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc"))
}

fn dorothy() -> AccountId {
    AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9"))
}

fn ethan() -> AccountId {
    AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB"))
}

fn faith() -> AccountId {
    AccountId::from(hex!("C0F0f4ab324C46e55D02D0033343B4Be8A55532d"))
}

/// Extension for the dev genesis config to support a custom changes to the genesis state.
#[derive(Serialize, Deserialize)]
pub struct DevGenesisExt {
    /// Genesis config.
    genesis_config: RuntimeGenesisConfig,
    /// The flag that if enable manual-seal mode.
    enable_manual_seal: Option<bool>,
}

impl sp_runtime::BuildStorage for DevGenesisExt {
    fn assimilate_storage(&self, storage: &mut Storage) -> Result<(), String> {
        BasicExternalities::execute_with_storage(storage, || {
            if let Some(enable_manual_seal) = &self.enable_manual_seal {
                EnableManualSeal::set(enable_manual_seal);
            }
        });
        self.genesis_config.assimilate_storage(storage)
    }
}

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn session_keys(babe: BabeId, grandpa: GrandpaId, im_online: ImOnlineId) -> opaque::SessionKeys {
    opaque::SessionKeys { babe, grandpa, im_online }
}

/// Generate a Babe authority key.
pub fn authority_keys_from_seed(s: &str) -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
    (
        get_account_id_from_seed::<ecdsa::Public>(&format!("{}//stash", s)),
        get_account_id_from_seed::<ecdsa::Public>(s),
        get_from_seed::<BabeId>(s),
        get_from_seed::<GrandpaId>(s),
        get_from_seed::<ImOnlineId>(s),
    )
}

fn properties() -> Properties {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "VTRS".into());
    properties.insert("tokenDecimals".into(), 18.into());
    properties.insert("ss58Format".into(), SS58Prefix::get().into());
    properties
}

const UNITS: Balance = 1_000_000_000_000_000_000;

pub fn development_config(enable_manual_seal: Option<bool>) -> DevChainSpec {
    let wasm_binary = WASM_BINARY.expect("WASM not available");

    DevChainSpec::from_genesis(
        // Name
        "Development",
        // ID
        "dev",
        ChainType::Development,
        move || {
            DevGenesisExt {
                genesis_config: testnet_genesis(
                    wasm_binary,
                    // Sudo account
                    alith(),
                    // Pre-funded accounts
                    vec![alith(), baltathar(), charleth(), dorothy(), ethan(), faith()],
                    // Initial Validators
                    vec![authority_keys_from_seed("Alice")],
                    vec![],
                    // Ethereum chain ID
                    SS58Prefix::get() as u64,
                ),
                enable_manual_seal,
            }
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        // Fork ID
        None,
        // Properties
        Some(properties()),
        // Extensions
        None,
    )
}

pub fn local_testnet_config() -> ChainSpec {
    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::from_genesis(
        // Name
        "Local Testnet",
        // ID
        "local_testnet",
        ChainType::Local,
        move || {
            testnet_genesis(
                wasm_binary,
                // Sudo account
                alith(),
                // Pre-funded accounts
                vec![alith(), baltathar(), charleth(), dorothy(), ethan(), faith()],
                // Initial Validators
                vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
                vec![],
                SS58Prefix::get() as u64,
            )
        },
        // Bootnodes
        vec![],
        // Telemetry
        None,
        // Protocol ID
        None,
        None,
        // Properties
        None,
        // Extensions
        None,
    )
}

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
    wasm_binary: &[u8],
    root_key: AccountId,
    mut endowed_accounts: Vec<AccountId>,
    initial_validators: Vec<(AccountId, AccountId, BabeId, GrandpaId, ImOnlineId)>,
    initial_cooperators: Vec<AccountId>,
    chain_id: u64,
) -> RuntimeGenesisConfig {
    // endow all authorities and cooperators.
    initial_validators
        .iter()
        .map(|x| [&x.0, &x.1])
        .chain(initial_cooperators.iter().map(|x| [x, x]))
        .for_each(|x| {
            for i in x {
                if !endowed_accounts.contains(i) {
                    endowed_accounts.push(*i)
                }
            }
        });

    // stakers: all validators and nominators.
    const ENDOWMENT: Balance = 1_000_000 * UNITS;
    const STASH: Balance = ENDOWMENT / 1000_000;
    let mut rng = rand::thread_rng();
    let stakers = initial_validators
        .iter()
        .map(|x| (x.0, x.1, STASH, StakerStatus::Validator))
        .chain(initial_cooperators.iter().map(|x| {
            use rand::{seq::SliceRandom, Rng};
            let limit = (MaxCooperations::get() as usize).min(initial_validators.len());
            let count = rng.gen::<usize>() % limit;
            let stake = STASH / count as Balance;
            let cooperations = initial_validators
                .as_slice()
                .choose_multiple(&mut rng, count)
                .map(|choice| (choice.0, stake))
                .collect::<Vec<_>>();
            (*x, *x, STASH, StakerStatus::Cooperator(cooperations))
        }))
        .collect::<Vec<_>>();

    RuntimeGenesisConfig {
        // System
        system: SystemConfig {
            // Add Wasm runtime to storage.
            code: wasm_binary.to_vec(),
            ..Default::default()
        },
        sudo: SudoConfig {
            // Assign network admin rights.
            key: Some(root_key),
        },

        // Monetary
        balances: BalancesConfig {
            balances: endowed_accounts.iter().cloned().map(|k| (k, ENDOWMENT)).collect(),
        },
        babe: BabeConfig { epoch_config: Some(BABE_GENESIS_EPOCH_CONFIG), ..Default::default() },
        grandpa: Default::default(),
        transaction_payment: Default::default(),

        // EVM compatibility
        evm_chain_id: EVMChainIdConfig { chain_id, ..Default::default() },
        evm: EVMConfig {
            accounts: {
                let mut map = BTreeMap::new();
                map.insert(
                    // H160 address of Alice dev account
                    // Derived from SS58 (42 prefix) address
                    // SS58: 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
                    // hex: 0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d
                    // Using the full hex key, truncating to the first 20 bytes (the first 40 hex chars)
                    H160::from_str("d43593c715fdd31c61141abd04a99fd6822c8558")
                        .expect("internal H160 is valid; qed"),
                    fp_evm::GenesisAccount {
                        balance: U256::from_str("0xffffffffffffffffffffffffffffffff")
                            .expect("internal U256 is valid; qed"),
                        code: Default::default(),
                        nonce: Default::default(),
                        storage: Default::default(),
                    },
                );
                map.insert(
                    // H160 address of CI test runner account
                    H160::from_str("6be02d1d3665660d22ff9624b7be0551ee1ac91b")
                        .expect("internal H160 is valid; qed"),
                    fp_evm::GenesisAccount {
                        balance: U256::from_str("0xffffffffffffffffffffffffffffffff")
                            .expect("internal U256 is valid; qed"),
                        code: Default::default(),
                        nonce: Default::default(),
                        storage: Default::default(),
                    },
                );
                map.insert(
                    // H160 address for benchmark usage
                    H160::from_str("1000000000000000000000000000000000000001")
                        .expect("internal H160 is valid; qed"),
                    fp_evm::GenesisAccount {
                        nonce: U256::from(1),
                        balance: U256::from(1_000_000_000_000_000_000_000_000u128),
                        storage: Default::default(),
                        code: vec![0x00],
                    },
                );
                map
            },
            ..Default::default()
        },
        ethereum: Default::default(),
        dynamic_fee: Default::default(),
        base_fee: Default::default(),
        assets: AssetsConfig {
            assets: vec![(VNRG::get(), alith(), true, 1)],
            metadata: vec![(
                VNRG::get(),
                "Energy".as_bytes().to_vec(),
                "VNRG".as_bytes().to_vec(),
                18,
            )],
            ..Default::default()
        },
        reputation: ReputationConfig {
            accounts: stakers
                .iter()
                .flat_map(|x| {
                    [
                        (x.0, COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD.into()),
                        (x.1, COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD.into()),
                    ]
                })
                .collect::<Vec<_>>(),
        },
        session: SessionConfig {
            keys: initial_validators
                .iter()
                .map(|x| (x.1, x.0, session_keys(x.2.clone(), x.3.clone(), x.4.clone())))
                .collect::<Vec<_>>(),
        },
        energy_generation: EnergyGenerationConfig {
            validator_count: initial_validators.len() as u32,
            minimum_validator_count: initial_validators.len() as u32,
            invulnerables: initial_validators.iter().map(|x| x.0).collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            stakers,
            ..Default::default()
        },
        im_online: ImOnlineConfig { keys: vec![] },
    }
}
