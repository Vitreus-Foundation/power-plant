#![allow(clippy::type_complexity, clippy::identity_op)]

use hex_literal::hex;
use serde::{Deserialize, Serialize};
// Substrate
use polkadot_primitives::{AssignmentId, AuthorityDiscoveryId, ValidatorId};
use sc_chain_spec::{ChainSpecExtension, ChainType, Properties};
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::ecdsa;
use sp_core::{storage::Storage, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::{FixedU128, Perbill};
use sp_state_machine::BasicExternalities;
// Frontier
use vitreus_power_plant_runtime::{
    opaque, vtrs, AccountId, AssetsConfig, AuthorityDiscoveryConfig, BabeConfig, Balance,
    BalancesConfig, Claiming, ClaimingConfig, ConfigurationConfig, CouncilConfig, EVMChainIdConfig,
    EnableManualSeal, EnergyFeeConfig, EnergyGenerationConfig, ImOnlineConfig, ImOnlineId,
    MaxCooperations, NacManagingConfig, ReputationConfig, ReputationPoint, RuntimeGenesisConfig,
    SS58Prefix, SessionConfig, Signature, SimpleVestingConfig, StakerStatus, SudoConfig,
    SystemConfig, TechnicalCommitteeConfig, BABE_GENESIS_EPOCH_CONFIG,
    COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD, VNRG, WASM_BINARY,
};

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    /// The light sync state.
    ///
    /// This value will be set by the `sync-state rpc` implementation.
    pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig, Extensions>;

/// Specialized `ChainSpec` for development.
pub type DevChainSpec = sc_service::GenericChainSpec<DevGenesisExt, Extensions>;

const INITIAL_ENERGY_BALANCE: Balance = 100_000_000_000_000_000_000u128;
/// 10^9 with 18 decimals
const INITIAL_ENERGY_RATE: FixedU128 = FixedU128::from_inner(1_000_000_000_000_000_000_000_000_000);

/// Min validator stake for user who has NAC level = 1.
const MIN_COMMON_VALIDATOR_BOND: Balance = 1_000_000 * vtrs::UNITS;

/// Min validator stake for user who has NAC level > 1.
const MIN_TRUST_VALIDATOR_BOND: Balance = 1 * vtrs::UNITS;

const MIN_COOPERATOR_BOND: Balance = 1_000_000_000_000_000_000;
const ENERGY_PER_STAKE_CURRENCY: Balance = 19_909_091_036_891;

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
    use tech_addresses::*;

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
                    vec![
                        alith(),
                        baltathar(),
                        charleth(),
                        dorothy(),
                        ethan(),
                        faith(),
                        goliath(),
                        treasury(),
                    ],
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
        Default::default(),
    )
}

pub fn devnet_config() -> ChainSpec {
    use devnet_keys::*;
    use tech_addresses::*;

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
                vec![
                    alith(),
                    baltathar(),
                    charleth(),
                    dorothy(),
                    ethan(),
                    faith(),
                    goliath(),
                    treasury(),
                ],
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
        Default::default(),
    )
}

pub fn localnet_config() -> ChainSpec {
    use devnet_keys::*;
    use tech_addresses::*;

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
                vec![
                    alith(),
                    baltathar(),
                    charleth(),
                    dorothy(),
                    ethan(),
                    faith(),
                    goliath(),
                    treasury(),
                ],
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
        Default::default(),
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
                root(),
                // Pre-funded accounts
                vec![root(), account_1(), account_2(), account_3()],
                // Initial Validators
                vec![validator_1_keys(), validator_2_keys(), validator_3_keys()],
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
        Default::default(),
    )
}

pub fn stagenet_config() -> ChainSpec {
    use devnet_keys::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::from_genesis(
        // Name
        "Stagenet",
        // ID
        "stagenet",
        ChainType::Custom("Stagenet".to_string()),
        move || {
            mainnet_genesis(
                wasm_binary,
                // Initial Validators
                vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
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
        Default::default(),
    )
}

pub fn mainnet_config() -> ChainSpec {
    use mainnet_keys::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::from_genesis(
        // Name
        "Mainnet",
        // ID
        "mainnet",
        ChainType::Live,
        move || {
            mainnet_genesis(
                wasm_binary,
                // Initial Validators
                vec![
                    validator_1_keys(),
                    validator_2_keys(),
                    validator_3_keys(),
                    validator_4_keys(),
                    validator_5_keys(),
                ],
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
        Default::default(),
    )
}

/// Configure initial storage state for FRAME modules.
pub fn testnet_genesis(
    wasm_binary: &[u8],
    root_key: AccountId,
    mut endowed_accounts: Vec<AccountId>,
    initial_validators: Vec<(
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    )>,
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
        claiming: genesis::claiming_config(),
        vesting: Default::default(),
        simple_vesting: Default::default(),
        babe: BabeConfig { epoch_config: Some(BABE_GENESIS_EPOCH_CONFIG), ..Default::default() },
        council: CouncilConfig {
            members: endowed_accounts.iter().cloned().take(3).collect(),
            ..Default::default()
        },
        democracy: Default::default(),
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
        assets: AssetsConfig {
            assets: vec![(VNRG::get(), root_key, false, 1)],
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
            accounts: endowed_accounts.iter().map(|x| (*x, 2)).collect(),
            owners: vec![root_key],
        },
        session: SessionConfig {
            keys: initial_validators
                .iter()
                .map(|x| {
                    (
                        x.1,
                        x.0,
                        session_keys(
                            x.2.clone(),
                            x.3.clone(),
                            x.4.clone(),
                            x.5.clone(),
                            x.6.clone(),
                            x.7.clone(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
        },
        technical_committee: TechnicalCommitteeConfig {
            members: endowed_accounts.iter().cloned().skip(3).take(3).collect(),
            ..Default::default()
        },
        technical_membership: Default::default(),
        treasury: Default::default(),
        energy_generation: EnergyGenerationConfig {
            validator_count: 125,
            minimum_validator_count: initial_validators.len() as u32,
            invulnerables: initial_validators.iter().map(|x| x.0).collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            min_cooperator_bond: MIN_COOPERATOR_BOND,
            min_common_validator_bond: MIN_COMMON_VALIDATOR_BOND,
            min_trust_validator_bond: MIN_TRUST_VALIDATOR_BOND,
            stakers,
            energy_per_stake_currency: ENERGY_PER_STAKE_CURRENCY,
            block_authoring_reward: ReputationPoint(24),
            ..Default::default()
        },
        im_online: ImOnlineConfig { keys: vec![] },
        authority_discovery: AuthorityDiscoveryConfig { keys: vec![], ..Default::default() },
        hrmp: Default::default(),
        configuration: ConfigurationConfig { config: default_parachains_host_configuration() },
        paras: Default::default(),
    }
}

/// Configure initial storage state for FRAME modules.
fn mainnet_genesis(
    wasm_binary: &[u8],
    initial_validators: Vec<(
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    )>,
) -> RuntimeGenesisConfig {
    use mainnet_keys::*;

    let root_key = root();
    let endowed_accounts = [root()];

    const ENDOWMENT: Balance = 1_000 * vtrs::UNITS;
    const STASH: Balance = 1 * vtrs::UNITS;

    let stakers = initial_validators
        .iter()
        .map(|x| (x.0, x.1, STASH, StakerStatus::Validator))
        .collect::<Vec<_>>();

    let claiming_config = genesis::claiming_config();

    let claiming_balance = claiming_config
        .claims
        .iter()
        .fold(15_000_000 * vtrs::UNITS, |total, claim| total.saturating_add(claim.1));

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
            balances: endowed_accounts
                .iter()
                .map(|k| (*k, ENDOWMENT))
                .chain([(Claiming::claim_account_id(), claiming_balance)])
                .chain(initial_validators.iter().map(|x| (x.0, STASH)))
                .chain(genesis::vested_balance())
                .chain(genesis::tech_allocation())
                .collect(),
        },
        claiming: claiming_config,
        vesting: Default::default(),
        simple_vesting: genesis::simple_vesting_config(),
        babe: BabeConfig { epoch_config: Some(BABE_GENESIS_EPOCH_CONFIG), ..Default::default() },
        council: genesis::council_config(),
        democracy: Default::default(),
        grandpa: Default::default(),
        transaction_payment: Default::default(),

        // EVM compatibility
        evm_chain_id: EVMChainIdConfig { chain_id: SS58Prefix::get() as u64, ..Default::default() },
        evm: Default::default(),
        ethereum: Default::default(),
        energy_fee: EnergyFeeConfig {
            initial_energy_rate: INITIAL_ENERGY_RATE,
            ..Default::default()
        },
        assets: AssetsConfig {
            assets: vec![(VNRG::get(), root_key, false, 1)],
            metadata: vec![(
                VNRG::get(),
                "Energy".as_bytes().to_vec(),
                "VNRG".as_bytes().to_vec(),
                18,
            )],
            accounts: vec![],
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
            accounts: initial_validators
                .iter()
                .map(|x| x.1)
                .chain(genesis::vnode_accounts())
                .map(|account| (account, 2))
                .collect(),
            owners: vec![root_key],
        },
        session: SessionConfig {
            keys: initial_validators
                .iter()
                .map(|x| {
                    (
                        x.1,
                        x.0,
                        session_keys(
                            x.2.clone(),
                            x.3.clone(),
                            x.4.clone(),
                            x.5.clone(),
                            x.6.clone(),
                            x.7.clone(),
                        ),
                    )
                })
                .collect::<Vec<_>>(),
        },
        technical_committee: genesis::technical_committee_config(),
        technical_membership: Default::default(),
        treasury: Default::default(),
        energy_generation: EnergyGenerationConfig {
            validator_count: initial_validators.len() as u32,
            minimum_validator_count: initial_validators.len() as u32 - 1,
            invulnerables: initial_validators.iter().map(|x| x.0).collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            min_commission: Perbill::from_percent(20),
            min_cooperator_bond: MIN_COOPERATOR_BOND,
            min_common_validator_bond: MIN_COMMON_VALIDATOR_BOND,
            min_trust_validator_bond: MIN_TRUST_VALIDATOR_BOND,
            stakers,
            disable_collaboration: true,
            energy_per_stake_currency: ENERGY_PER_STAKE_CURRENCY,
            block_authoring_reward: ReputationPoint(24),
            ..Default::default()
        },
        im_online: ImOnlineConfig { keys: vec![] },
        authority_discovery: AuthorityDiscoveryConfig { keys: vec![], ..Default::default() },
        hrmp: Default::default(),
        configuration: ConfigurationConfig { config: default_parachains_host_configuration() },
        paras: Default::default(),
    }
}

pub mod devnet_keys {
    use super::*;

    pub fn alith() -> AccountId {
        AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac"))
    }

    pub fn baltathar() -> AccountId {
        AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0"))
    }

    pub fn charleth() -> AccountId {
        AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc"))
    }

    pub fn dorothy() -> AccountId {
        AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9"))
    }

    pub fn ethan() -> AccountId {
        AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB"))
    }

    pub fn faith() -> AccountId {
        AccountId::from(hex!("C0F0f4ab324C46e55D02D0033343B4Be8A55532d"))
    }

    pub fn goliath() -> AccountId {
        AccountId::from(hex!("7BF369283338E12C90514468aa3868A551AB2929"))
    }

    pub fn authority_keys_from_seed(
        s: &str,
    ) -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            get_account_id_from_seed::<ecdsa::Public>(&format!("{}//stash", s)),
            get_account_id_from_seed::<ecdsa::Public>(s),
            derive_dev::<BabeId>(s),
            derive_dev::<GrandpaId>(s),
            derive_dev::<ImOnlineId>(s),
            derive_dev::<ValidatorId>(s),
            derive_dev::<AssignmentId>(s),
            derive_dev::<AuthorityDiscoveryId>(s),
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

pub mod testnet_keys {
    use super::*;

    pub(super) fn root() -> AccountId {
        AccountId::from(hex!("2F8CF06C0c21CA40eC4006d35C01B92a63d15d66"))
    }

    pub(super) fn validator_1() -> AccountId {
        AccountId::from(hex!("BE2839a4F6fadCdc651151b307568FC8daEB670D"))
    }

    pub(super) fn validator_2() -> AccountId {
        AccountId::from(hex!("3862660d31edcF2e84fB5c551768a84ac7259bfb"))
    }

    pub(super) fn validator_3() -> AccountId {
        AccountId::from(hex!("A4A86AD2cC74A7f289Eb9921CF805e22eB2Bb2BF"))
    }

    pub(super) fn account_1() -> AccountId {
        AccountId::from(hex!("624B523D1d80B7527e4444F5dbBE37A43df8819b"))
    }

    pub(super) fn account_2() -> AccountId {
        AccountId::from(hex!("156C92352EEcA66E54B755D63538C911fF3D6d3E"))
    }

    pub(super) fn account_3() -> AccountId {
        AccountId::from(hex!("E0E337F0753CB3099B17c6Af6D3E7C41e99FF83D"))
    }

    pub(super) fn validator_1_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("784e69Feba8a2FCCc938A722D5a66E9EbfA3A14A")), // Stash
            validator_1(),
            sp_core::sr25519::Public(hex!(
                "f29f3491dc2baf6ffeffd01702a1b5289519c00a229d41544edc357d0355db51"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "275fad28e7f2904a0341b5baa66b40f8941b09a22739a8b141f99b91e0dd9458"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "408300338038bb359afc7f32a0622d3be520988b5a89c3af5af0272e6745de5e"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "f870a88d596c9207b9df17fbc7960ba9f7fa25296fc3d17a844bc7680287011e"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "164b92db1487c67254182e9e231823b662fb39ffdee4e1ffe73559f600bebd25"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "7297d91787b2ec39853efaf1a553b4a5b58f834161a11f03575116e0340ada62"
            ))
            .into(),
        )
    }

    pub(super) fn validator_2_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("309753d1BAc45489B9C4BdDEf28963d862AdCb13")), // Stash
            validator_2(),
            sp_core::sr25519::Public(hex!(
                "80b57a74ddb35163ada69d61022d518cdad36eb63f766a04f9d2db35da28052f"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "a4e37cd11ee58c2a6d529f42b13195295179df0921bf20d9f634145d71e817f1"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "527844f460f369100ca67a1fa084b9a29b71d984cd90479ce5bcd7efb74bde1c"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "7aec7e56d3de6cf85d23d38fea64107523ffeb43e17e27de6899cac625199a3d"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "acc2c4d8acefa119eee9a88a880bc490895c0aeb2a661daeccf2b6fcba30da3f"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "d2d0a556d5526c8114e7312a9d7220869894db1ae01f3ef7696f9f784fc58a4f"
            ))
            .into(),
        )
    }

    pub(super) fn validator_3_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("A6543B65DD9cFA7e324AF616A339D3c1a13fa685")), // Stash
            validator_3(),
            sp_core::sr25519::Public(hex!(
                "c2335d394c89693254fb1a323dc74d9c1a14f43ad3292081b331930f9fa8d072"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "281a3b47515392d492faca42d616fa09e609b5fbbaa98716293ebf5c6d4e6248"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "3e99fe54593eeaf568029ec4989106286fd3384fc9c7b723d0e60bc3c3c02479"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "1eb253fc5186d7ec1bf2d28cc8120d97431745ead18381aca1cff47ebae0a83c"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "0075191c5441c7a2134c234f3ab393866deded809f21a37c9a6025ce26884556"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "5425357e3002c6d2972e803362fe8648156837e70f3951929f13c0b9ba75c93b"
            ))
            .into(),
        )
    }
}

pub mod mainnet_keys {
    use super::*;

    pub(super) fn root() -> AccountId {
        AccountId::from(hex!("cD3a7509cE9869902FA5F8132c686014d1A6ef07"))
    }

    pub(super) fn validator_1_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("f3612fF49FE440e46faAc08C71d141249D71ff12")), // Stash
            AccountId::from(hex!("03a6b4755F58f91731735d5B881054Fe6eCA7cc8")), // Validator
            sp_core::sr25519::Public(hex!(
                "bc35dc7c4bb874005361848b08cc7b5cba87e10391f60f124cc04bf5d34c981c"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "24c7ac8c11718f7056b01f6755c7da3d5d16423243334f22b2d560e658def8eb"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "4c2ff1cedac25109f367826a420e1bd74047ab37aa78132bb42fbf1dd689e876"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "0a803acb3451a91d84c27757c5af3a45b295741df3170853d9f6a8073e202300"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "84a974507ccea0dc46d5beb4287d71665a65d83d47bf9da40b1f62ddaa2b804c"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "864507801e50051b2bb691c7839b2eb66d288f4f6f5d070681f79f7aac8c613f"
            ))
            .into(),
        )
    }

    pub(super) fn validator_2_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("e9De3598e78Ac90d45de4f6B19666B280ec4886b")), // Stash
            AccountId::from(hex!("1d1D7cd4469c5997a665ce219fB3aE6B18FF5E52")), // Validator
            sp_core::sr25519::Public(hex!(
                "74dcc8c2138cd579b8cc80d2de4cca3a37cdb6a4dde08a8b8dd309a0106b9c79"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "370f55c79dab485740ef10881d92ec515332292652f0ad9ed61f3450edc13fc7"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "a25d0e79e61aeef2e7baa9814f1d4744afeecab9d8df9891307500b122752d74"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "2c46c1d5dcdfe94e5a8543ad749a235e3a2285f197798f534669e97140517b34"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "9e7d5d82b966eb206efe40e7e3e8d1b37b4fdb0764c651b32ae207b760ae4e74"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "c47aea1cf8270930ef66afd4a04f14c7baf003bc1993c76a2c17807c019c303d"
            ))
            .into(),
        )
    }

    pub(super) fn validator_3_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("66c93481CbF5F1951C0D5E0685540D4e59C3C454")), // Stash
            AccountId::from(hex!("3Ff5AD852f15a22F5b39DF53C400314C8b8F8ACb")), // Validator
            sp_core::sr25519::Public(hex!(
                "b2d683cc8c70417182d724244c284b22ad47ec61a872bd7873958c4ee757a966"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "42fe4d0b5298eaf62062cef16143eb3458a1f0c7b66b92dfa3cbb21141c4681d"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "14958d70c481dc72925df4a8701743f3069241818bf6a725060bbcce008bea09"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "62c90e97adf3f99cad816b0585e8a64d1604929cc76ff7d23f5edcb825f0980e"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "ae5600a1039bd46927c0927673491df50e2e4787970bf2e948f4aefb65601768"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "e49adf172ddb764215d7ba5a4633dec04991fb3bd9d876f34740f10f439ebf0a"
            ))
            .into(),
        )
    }

    pub(super) fn validator_4_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("29f8AE257C8Ab3607AB7F37215606b52a54849D1")), // Stash
            AccountId::from(hex!("1b2f4c7A4863587987e2083c8c7D023856996116")), // Validator
            sp_core::sr25519::Public(hex!(
                "2ae1ee0f0df43aaa1f071bda8f8f58e849a1024f59bfbe35063bd06a5bd90d57"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "f2a1fef4dc58a00843f38f792b4854ae4d35e641784577df018d1017f69098e4"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "eeb0fc7fcf71296f88a83ccf937fffa6752f7eb3da64a8bb4eab601488c43850"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "4010ccfaa632f6aa8d026174dec6a2b27bd4036d1dc4f92c4c66df78e7af1313"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "a6e02f71a4723f4e671cb4e67bbc60307cdf393406d2690c9a54c66f4e210b75"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "341a2e2fd269d9d56c8b1f5b4bf0de3bf05c3f1b7dab48a40f06e66818dd067e"
            ))
            .into(),
        )
    }

    pub(super) fn validator_5_keys() -> (
        AccountId,
        AccountId,
        BabeId,
        GrandpaId,
        ImOnlineId,
        ValidatorId,
        AssignmentId,
        AuthorityDiscoveryId,
    ) {
        (
            AccountId::from(hex!("8136A3A57a52bA0b8370E46D8c5e3B99f20b4c0a")), // Stash
            AccountId::from(hex!("D913FDf697CA06e3BDa32169C58C0e51d0E38db7")), // Validator
            sp_core::sr25519::Public(hex!(
                "64676f9a84208fa8039ea15ab8235532a1ae11368d583f8ab12be5f2721e6b3b"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "f2616ad5f85d65c2d67e85e3d555f05f1f5dd597d791f289eeb124fd1eaf57c7"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "d08221bccf384c180108c7fd0eccd4976f0306503da336950b7bd8a6f5c0d77e"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "929751cb0df5782a5800d133b1dbd72d6a30f22a4024b8968001fb8d4b896553"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "801067636cb11406d8ec953f0e57b9de3784537fb1f1fd425e27fe2d6e9f2a01"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "60c50d14148a65996718014822b528ca2ad0e75a66cddd07e8b80ea78f223955"
            ))
            .into(),
        )
    }
}

mod tech_addresses {
    use sp_runtime::traits::AccountIdConversion;
    use vitreus_power_plant_runtime::AccountId;

    pub fn treasury() -> AccountId {
        vitreus_power_plant_runtime::areas::TreasuryPalletId::get().into_account_truncating()
    }

    pub fn staking_rewards() -> AccountId {
        vitreus_power_plant_runtime::areas::StakingRewardsPalletId::get().into_account_truncating()
    }

    pub fn liquidity() -> AccountId {
        vitreus_power_plant_runtime::areas::LiquidityPalletId::get().into_account_truncating()
    }

    pub fn liquidity_reserves() -> AccountId {
        vitreus_power_plant_runtime::areas::LiquidityReservesPalletId::get()
            .into_account_truncating()
    }
}

mod genesis {
    use super::*;
    use tech_addresses::*;

    use vitreus_power_plant_runtime::{BlockNumber, ExistentialDeposit, DAYS, MILLI_VTRS};

    const YEARS: BlockNumber = 36525 * (DAYS / 100);
    const MONTHS: BlockNumber = YEARS / 12;

    pub(super) fn tech_allocation() -> Vec<(AccountId, Balance)> {
        const INITIAL_TREASURY_ALLOCATION: Balance = 68_364_887_120 * MILLI_VTRS;
        const INITIAL_LIQUIDITY_ALLOCATION: Balance = 10_000_000 * vtrs::UNITS;
        const INITIAL_LIQUIDITY_RESERVES_ALLOCATION: Balance = 125_000_000 * vtrs::UNITS;
        const INITIAL_STAKING_REWARDS_ALLOCATION: Balance = 170_000_000 * vtrs::UNITS;

        vec![
            (treasury(), INITIAL_TREASURY_ALLOCATION),
            (staking_rewards(), INITIAL_STAKING_REWARDS_ALLOCATION),
            (liquidity(), INITIAL_LIQUIDITY_ALLOCATION),
            (liquidity_reserves(), INITIAL_LIQUIDITY_RESERVES_ALLOCATION),
        ]
    }

    pub(super) fn vested_balance() -> impl Iterator<Item = (AccountId, Balance)> {
        let vesting = include!(concat!(env!("OUT_DIR"), "/vesting.rs"));
        vesting.into_iter().map(|(account, amount, _, _)| (account, amount))
    }

    pub(super) fn vnode_accounts() -> impl Iterator<Item = AccountId> {
        let vnode = include!(concat!(env!("OUT_DIR"), "/vnode.rs"));
        vnode.into_iter()
    }

    pub(super) fn claiming_config() -> ClaimingConfig {
        let mut config = ClaimingConfig {
            claims: include!(concat!(env!("OUT_DIR"), "/claiming_claims.rs")),
            vesting: vec![],
        };

        // address, amount in milliVTRS, vesting start/period in years
        let claims = vec![
            (
                hex!("3e743911188753601C688F42510d7d9fF34bfEFf"),
                375083500000000000000000,
                Some((None, 1, 1)),
            ),
            (
                hex!("2902213Ae1122D9D23c41AaC3961Da8d4dcb8588"),
                629210000000000000000,
                Some((None, 1, 1)),
            ),
            (
                hex!("Da67BB5318003a8Cd5D68cC2Fc042958ed4262F2"),
                26000000000000000000000,
                Some((None, 1, 1)),
            ),
            (
                hex!("E5b8524a2613472972cA7Ea11c6Fa2DA65379C2b"),
                1100000000000000000000,
                Some((None, 1, 1)),
            ),
            (
                hex!("cEcb9661f49255d7f814a49018Bc74069Cc0AD45"),
                260000000000000000000000,
                Some((None, 1, 1)),
            ),
            (
                hex!("fb8B24C9072A93BC3F6A5aF7C3F55a0655Eee509"),
                1360000000000000000000,
                Some((None, 1, 1)),
            ),
            (
                hex!("Dc5419Ce5633a3608b1d19F26377D84BD8b0168f"),
                22046888888888800000000 + 2040000000000000000000,
                Some((Some(2040000000000000000000), 1, 1)),
            ),
            (hex!("5b7d4c4b7243bfad283472c1ff3a4fb1949cb309"), 60627000000000000000000, None),
            (hex!("21ECD0192945a534EA5faf594f1a5aDa6CBAD4C0"), 160353820000000000000000, None),
            (hex!("205Be1AD81b62E49ed9D34E97cb52F31D3644A04"), 100540000000000000000000, None),
            (hex!("Ab404525918C62F7A751Db4096f8Bb04E4D12309"), 10000000000000000000000, None),
        ];

        for (address, amount, vesting) in claims {
            let address = pallet_claiming::EthereumAddress(address);

            config.claims.push((address, amount));

            if let Some((vesting_amount, start, period)) = vesting {
                let start = start * YEARS;
                let period = period * YEARS;

                let vesting_amount = vesting_amount.unwrap_or(amount);
                let amount_per_block = vesting_amount / period as u128;

                config.vesting.push((address, (vesting_amount, amount_per_block, start)));
            }
        }

        config
    }

    pub(super) fn simple_vesting_config() -> SimpleVestingConfig {
        let vesting = include!(concat!(env!("OUT_DIR"), "/vesting.rs"));

        let vesting = vesting
            .into_iter()
            .map(|(address, _, start, period)| {
                (address, start * MONTHS, period * MONTHS, ExistentialDeposit::get())
            })
            .collect();

        SimpleVestingConfig { vesting }
    }

    pub(super) fn council_config() -> CouncilConfig {
        CouncilConfig {
            members: vec![
                AccountId::from(hex!("5f53d6893b5ca9b80a98e66f16f966e8c7d0b29c")),
                AccountId::from(hex!("51C7b0F6Ec0b45b1Fa644Aa1c7560d45D7506c91")),
                AccountId::from(hex!("56fef4e5500eb9b8546c722f22e437e26d45f33c")),
                AccountId::from(hex!("2A27793bDe97A121050093285a6D75a6EbCD9Cf1")),
                AccountId::from(hex!("d3a58ebcbab95c9950aa45d143eb4ca241ff4563")),
                AccountId::from(hex!("e6a32e4b99f2c40f1999db38456e8b1a4d4a9884")),
                AccountId::from(hex!("7bD754C60e252ac4Ea5E583EA8F4ee34Bf7Cb3DC")),
                AccountId::from(hex!("6096991707a190f97e2cb9146fabba23f5fc8cdd")),
                AccountId::from(hex!("93518a144178d7cea1421c7a754b1493ad47f439")),
            ],
            ..Default::default()
        }
    }

    pub(super) fn technical_committee_config() -> TechnicalCommitteeConfig {
        TechnicalCommitteeConfig {
            members: vec![
                AccountId::from(hex!("B91De9Bdb5A04ecD5f1ddb875E96e39926F7AFE1")),
                AccountId::from(hex!("0646fc56f259b366a6b05b38c128275b06a68b41")),
            ],
            ..Default::default()
        }
    }
}

fn session_keys(
    babe: BabeId,
    grandpa: GrandpaId,
    im_online: ImOnlineId,
    para_validator: ValidatorId,
    para_assignment: AssignmentId,
    authority_discovery: AuthorityDiscoveryId,
) -> opaque::SessionKeys {
    opaque::SessionKeys {
        grandpa,
        babe,
        im_online,
        para_validator,
        para_assignment,
        authority_discovery,
    }
}

fn properties() -> Properties {
    let mut properties = Properties::new();
    properties.insert("tokenSymbol".into(), "VTRS".into());
    properties.insert("tokenDecimals".into(), 18.into());
    properties.insert("ss58Format".into(), SS58Prefix::get().into());
    properties
}

fn default_parachains_host_configuration(
) -> polkadot_runtime_parachains::configuration::HostConfiguration<polkadot_primitives::BlockNumber>
{
    use polkadot_primitives::{MAX_CODE_SIZE, MAX_POV_SIZE};

    polkadot_runtime_parachains::configuration::HostConfiguration {
        validation_upgrade_cooldown: 2u32,
        validation_upgrade_delay: 2,
        code_retention_period: 1200,
        max_code_size: MAX_CODE_SIZE,
        max_pov_size: MAX_POV_SIZE,
        max_head_data_size: 32 * 1024,
        group_rotation_frequency: 20,
        chain_availability_period: 4,
        thread_availability_period: 4,
        max_upward_queue_count: 8,
        max_upward_queue_size: 1024 * 1024,
        max_downward_message_size: 1024 * 1024,
        max_upward_message_size: 50 * 1024,
        max_upward_message_num_per_candidate: 5,
        hrmp_sender_deposit: 0,
        hrmp_recipient_deposit: 0,
        hrmp_channel_max_capacity: 8,
        hrmp_channel_max_total_size: 8 * 1024,
        hrmp_max_parachain_inbound_channels: 4,
        hrmp_max_parathread_inbound_channels: 4,
        hrmp_channel_max_message_size: 1024 * 1024,
        hrmp_max_parachain_outbound_channels: 4,
        hrmp_max_parathread_outbound_channels: 4,
        hrmp_max_message_num_per_candidate: 5,
        dispute_period: 6,
        no_show_slots: 2,
        n_delay_tranches: 25,
        needed_approvals: 2,
        relay_vrf_modulo_samples: 2,
        zeroth_delay_tranche_width: 0,
        minimum_validation_upgrade_delay: 5,
        ..Default::default()
    }
}
