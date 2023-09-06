use hex_literal::hex;
use serde::{Deserialize, Serialize};
// Substrate
use sc_chain_spec::{ChainType, Properties};
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::ecdsa;
use sp_core::{storage::Storage, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::{FixedU128, Perbill};
use sp_state_machine::BasicExternalities;
// Frontier
use vitreus_power_plant_runtime::{
    opaque, vtrs, AccountId, AssetsConfig, BabeConfig, Balance, BalancesConfig, EVMChainIdConfig,
    EnableManualSeal, EnergyFeeConfig, EnergyGenerationConfig, ImOnlineConfig, ImOnlineId,
    MaxCooperations, NacManagingConfig, ReputationConfig, RuntimeGenesisConfig, SS58Prefix,
    SessionConfig, Signature, StakerStatus, SudoConfig, SystemConfig, BABE_GENESIS_EPOCH_CONFIG,
    COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD, VNRG, WASM_BINARY,
};

const INITIAL_NAC_COLLECTION_ID: u32 = 0;

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Specialized `ChainSpec` for development.
pub type DevChainSpec = sc_service::GenericChainSpec<DevGenesisExt>;

const INITIAL_ENERGY_BALANCE: Balance = 100_000_000_000_000_000_000u128;
/// 10^9 with 18 decimals
const INITIAL_ENERGY_RATE: FixedU128 = FixedU128::from_inner(1_000_000_000_000_000_000_000_000_000);

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

pub fn development_config(enable_manual_seal: Option<bool>) -> DevChainSpec {
    use devnet_keys::*;

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
                    vec![alith(), baltathar(), charleth(), dorothy(), ethan(), faith(), goliath()],
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

pub fn devnet_config() -> ChainSpec {
    use devnet_keys::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::from_genesis(
        // Name
        "Devnet",
        // ID
        "devnet",
        ChainType::Custom("Devnet".to_string()),
        move || {
            testnet_genesis(
                wasm_binary,
                // Sudo account
                alith(),
                // Pre-funded accounts
                vec![alith(), baltathar(), charleth(), dorothy(), ethan(), faith(), goliath()],
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
        Some(properties()),
        // Extensions
        None,
    )
}

pub fn localnet_config() -> ChainSpec {
    use devnet_keys::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::from_genesis(
        // Name
        "Localnet",
        // ID
        "localnet",
        ChainType::Local,
        move || {
            testnet_genesis(
                wasm_binary,
                // Sudo account
                alith(),
                // Pre-funded accounts
                vec![alith(), baltathar(), charleth(), dorothy(), ethan(), faith(), goliath()],
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
        Some(properties()),
        // Extensions
        None,
    )
}

pub fn testnet_config() -> ChainSpec {
    use testnet_keys::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::from_genesis(
        // Name
        "Testnet",
        // ID
        "testnet",
        ChainType::Custom("Testnet".to_string()),
        move || {
            testnet_genesis(
                wasm_binary,
                // Sudo account
                stefania(),
                // Pre-funded accounts
                vec![stefania(), galya(), raya(), lyuba()],
                // Initial Validators
                vec![valya_validator_keys(), zina_validator_keys(), nina_validator_keys()],
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
        Some(properties()),
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
    const ENDOWMENT: Balance = 1_000_000 * vtrs::UNITS;
    const STASH: Balance = ENDOWMENT / 1_000_000;
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
        evm: Default::default(),
        ethereum: Default::default(),
        energy_fee: EnergyFeeConfig {
            initial_energy_rate: INITIAL_ENERGY_RATE,
            ..Default::default()
        },
        dynamic_fee: Default::default(),
        base_fee: Default::default(),
        assets: AssetsConfig {
            assets: vec![(VNRG::get(), root_key, true, 1)],
            metadata: vec![(
                VNRG::get(),
                "Energy".as_bytes().to_vec(),
                "VNRG".as_bytes().to_vec(),
                18,
            )],
            accounts: endowed_accounts
                .iter()
                .cloned()
                .map(|account| (VNRG::get(), account, INITIAL_ENERGY_BALANCE))
                .collect(),
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
        nac_managing: NacManagingConfig {
            accounts: endowed_accounts.iter().map(|x| (*x, 1)).collect(),
            collections: vec![(INITIAL_NAC_COLLECTION_ID, root_key)],
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

mod devnet_keys {
    use super::*;

    pub(super) fn alith() -> AccountId {
        AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac"))
    }

    pub(super) fn baltathar() -> AccountId {
        AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0"))
    }

    pub(super) fn charleth() -> AccountId {
        AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc"))
    }

    pub(super) fn dorothy() -> AccountId {
        AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9"))
    }

    pub(super) fn ethan() -> AccountId {
        AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB"))
    }

    pub(super) fn faith() -> AccountId {
        AccountId::from(hex!("C0F0f4ab324C46e55D02D0033343B4Be8A55532d"))
    }

    pub(super) fn goliath() -> AccountId {
        AccountId::from(hex!("7BF369283338E12C90514468aa3868A551AB2929"))
    }

    pub fn authority_keys_from_seed(
        s: &str,
    ) -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
        (
            get_account_id_from_seed::<ecdsa::Public>(&format!("{}//stash", s)),
            get_account_id_from_seed::<ecdsa::Public>(s),
            derive_dev::<BabeId>(s),
            derive_dev::<GrandpaId>(s),
            derive_dev::<ImOnlineId>(s),
        )
    }
    /// Generate a crypto pair.
    pub fn derive_dev<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
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
        AccountPublic::from(derive_dev::<TPublic>(seed)).into_account()
    }
}

mod testnet_keys {
    use super::*;

    pub(super) fn stefania() -> AccountId {
        AccountId::from(hex!("0F92eb04a6c829A850106B7a5EFdEfFA3e12ff9A"))
    }

    pub(super) fn valya() -> AccountId {
        AccountId::from(hex!("6ee40c68188132A8Eb06bA5A3f4A9CFaDf1AF342"))
    }

    pub(super) fn zina() -> AccountId {
        AccountId::from(hex!("416B1da2C7242A796C12c0641676C7E35a04597D"))
    }

    pub(super) fn nina() -> AccountId {
        AccountId::from(hex!("cF994FD2E08eb3e9A584080f3606B1434B5CcADF"))
    }

    pub(super) fn galya() -> AccountId {
        AccountId::from(hex!("675D18406cc184E6CC322a12F5e2b156394E0f5a"))
    }

    pub(super) fn raya() -> AccountId {
        AccountId::from(hex!("D939d4FC9f7777Ac31F2cb419da896e645c47289"))
    }

    pub(super) fn lyuba() -> AccountId {
        AccountId::from(hex!("D2d0C6bc8F13D89C69de3f19bed306dE60Ef2Efc"))
    }

    pub(super) fn valya_validator_keys() -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
        (
            AccountId::from(hex!("7eb27fE7e7eac00F8294D085c2058F6c84DA58E4")), // Stash
            valya(),
            sp_core::sr25519::Public(hex!(
                "164be205541f9afd3a4f2743e53f32f6c708801a8ea4d172a841bbe80fb8896c"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "51fd2b964631392aa13abfa0cb7bcf42ec7cbc215e2cc437d8ca6314b488cf41"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "021f199e2a8b3e25d0a2f078225f48e876f9ae028ab904e803d115e749a01d0c"
            ))
            .into(),
        )
    }

    pub(super) fn zina_validator_keys() -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
        (
            AccountId::from(hex!("a4bc9609540342f2e5429141e610a84117aa4d98")), // Stash
            zina(),
            sp_core::sr25519::Public(hex!(
                "e8bcd01c18e37e45b2c42710ba51845d8b35406fa5a7f5feb99aed0decca4408"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "ca8f73db4af15312b4cbb297cc60279c8478dd32f2ff261f46c7dfdd27f2ff24"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "ca8f73db4af15312b4cbb297cc60279c8478dd32f2ff261f46c7dfdd27f2ff24"
            ))
            .into(),
        )
    }

    pub(super) fn nina_validator_keys() -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
        (
            AccountId::from(hex!("F10713631180c6D825a17650258E8cC8b62161e4")), // Stash
            nina(),
            sp_core::sr25519::Public(hex!(
                "e65beb63ed80dfc2de0d5c0a7e945306eaddfbe8ef69a1186d7299e84d8fff18"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "043d568913ea8c6e8f51bb634b7e6d1545b22506499ca5472bb9e3bc47b5e6d6"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "e8b78cd8499b36e69fe55993565b0453998cfe1ada6e4cbd0d1f43343751fc43"
            ))
            .into(),
        )
    }
}

fn session_keys(babe: BabeId, grandpa: GrandpaId, im_online: ImOnlineId) -> opaque::SessionKeys {
    opaque::SessionKeys { babe, grandpa, im_online }
}

fn properties() -> Properties {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "VTRS".into());
    properties.insert("tokenDecimals".into(), 18.into());
    properties.insert("ss58Format".into(), SS58Prefix::get().into());
    properties
}
