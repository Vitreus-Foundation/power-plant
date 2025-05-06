#![allow(dead_code, unused_imports, clippy::type_complexity, clippy::identity_op)]

use hex_literal::hex;
use serde::{Deserialize, Serialize};
// Substrate
use polkadot_primitives::{AssignmentId, AuthorityDiscoveryId, ValidatorId};
use sc_chain_spec::{ChainSpecExtension, ChainType, Properties};
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_beefy::ecdsa_crypto::AuthorityId as BeefyId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::ecdsa;
use sp_core::{Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::{FixedU128, Perbill};

// Frontier

use crate::tech_addresses::treasury;
use vitreus_power_plant_runtime::{
    opaque, vtrs, AccountId, AssetsConfig, AuthorityDiscoveryConfig, BabeConfig, Balance,
    BalancesConfig, Claiming, ClaimingConfig, ConfigurationConfig, CouncilConfig,
    DynamicEnergyConfig, EVMChainIdConfig, EnergyBrokerConfig, EnergyGenerationConfig,
    ImOnlineConfig, ImOnlineId, MaxCooperations, NacManagingConfig, PrivilegesConfig,
    ReputationConfig, ReputationPoint, RuntimeGenesisConfig, SS58Prefix, SessionConfig, Signature,
    SimpleVestingConfig, StakerStatus, SudoConfig, SystemConfig, TechnicalCommitteeConfig,
    BABE_GENESIS_EPOCH_CONFIG, COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD, LNRG, SNRG, VNRG,
    WASM_BINARY,
};

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    /// Block numbers with known hashes.
    pub fork_blocks: sc_client_api::ForkBlocks<polkadot_primitives::Block>,
    /// Known bad block hashes.
    pub bad_blocks: sc_client_api::BadBlocks<polkadot_primitives::Block>,
    /// The light sync state.
    ///
    /// This value will be set by the `sync-state rpc` implementation.
    pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<Extensions>;

const INITIAL_ENERGY_BALANCE: Balance = 100_000_000_000_000_000_000u128;
/// 1 VTRS = 0.9 gVolt => 1.11111... VTRS = 1 gVolt
const INITIAL_ENERGY_RATE: FixedU128 = FixedU128::from_inner(1_111_111_111_111_111_111_111_111_111);

/// Min validator stake for user who has NAC level = 1.
const MIN_COMMON_VALIDATOR_BOND: Balance = 1_000_000 * vtrs::UNITS;

/// Min validator stake for user who has NAC level > 1.
const MIN_TRUST_VALIDATOR_BOND: Balance = 1 * vtrs::UNITS;

const MIN_COOPERATOR_BOND: Balance = 1_000_000_000_000_000_000;
const ENERGY_PER_STAKE_CURRENCY: Balance = 19_909_091_036_891;

#[cfg(feature = "testnet-native")]
pub fn development_config() -> ChainSpec {
    use devnet_keys::*;
    use tech_addresses::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::builder(wasm_binary, Default::default())
        .with_name("Development")
        .with_id("dev")
        .with_chain_type(ChainType::Development)
        .with_properties(properties())
        .with_genesis_config(
            serde_json::to_value(testnet_genesis(
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
            ))
            .expect("Invalid genesis config"),
        )
        .build()
}

#[cfg(feature = "testnet-native")]
pub fn devnet_config() -> ChainSpec {
    use devnet_keys::*;
    use tech_addresses::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::builder(wasm_binary, Default::default())
        .with_name("Devnet")
        .with_id("devnet")
        .with_chain_type(ChainType::Custom("Devnet".to_string()))
        .with_properties(properties())
        .with_genesis_config(
            serde_json::to_value(testnet_genesis(
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
            ))
            .unwrap(),
        )
        .build()
}

#[cfg(feature = "testnet-native")]
pub fn localnet_config() -> ChainSpec {
    use devnet_keys::*;
    use tech_addresses::*;

    let wasm_binary = WASM_BINARY.expect("WASM not available");

    ChainSpec::builder(wasm_binary, Default::default())
        .with_name("Localnet")
        .with_id("localnet")
        .with_chain_type(ChainType::Local)
        .with_properties(properties())
        .with_genesis_config(
            serde_json::to_value(testnet_genesis(
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
            ))
            .unwrap(),
        )
        .build()
}

pub fn testnet_config() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(
        &include_bytes!("../chain-specs/vitreus-power-plant-testnet.json")[..],
    )
}

pub fn stagenet_config() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(
        &include_bytes!("../chain-specs/vitreus-power-plant-stagenet.json")[..],
    )
}

pub fn mainnet_config() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(
        &include_bytes!("../chain-specs/vitreus-power-plant-mainnet.json")[..],
    )
}

/// Configure initial storage state for FRAME modules.
#[cfg(feature = "testnet-native")]
pub fn testnet_genesis(
    _wasm_binary: &[u8],
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
        BeefyId,
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
            // code: wasm_binary.to_vec(),
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
        claiming: Default::default(),
        kickstart: Default::default(),
        vesting: Default::default(),
        simple_vesting: Default::default(),
        babe: BabeConfig { epoch_config: BABE_GENESIS_EPOCH_CONFIG, ..Default::default() },
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
        energy_broker: EnergyBrokerConfig { energy_capacity: 1_000_000_000_000 },
        assets: AssetsConfig {
            assets: vec![
                (VNRG::get(), root_key, false, 1),
                (SNRG::get(), root_key, false, 1),
                (LNRG::get(), root_key, false, 1),
            ],
            metadata: vec![
                (VNRG::get(), "Energy".as_bytes().to_vec(), "VNRG".as_bytes().to_vec(), 18),
                (SNRG::get(), "Static Energy".as_bytes().to_vec(), "SNRG".as_bytes().to_vec(), 18),
                (LNRG::get(), "Liquid Energy".as_bytes().to_vec(), "LNRG".as_bytes().to_vec(), 18),
            ],
            accounts: endowed_accounts
                .iter()
                .cloned()
                .map(|account| (VNRG::get(), account, INITIAL_ENERGY_BALANCE))
                .collect(),
            next_asset_id: Default::default(),
        },
        pool_assets: Default::default(),
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
        privileges: PrivilegesConfig { date: Some((2024, 5, 15)), ..Default::default() },
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
                            x.8.clone(),
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
        technical_committee_treasury: Default::default(),
        elections: Default::default(),
        dynamic_energy: DynamicEnergyConfig {
            energy_burn: 10_000_000_000,
            energy_sale: 10_000_000_000,
            total_stake: stakers.iter().map(|x| x.2).sum(),
        },
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
        xcm_pallet: Default::default(),
        beefy: Default::default(),
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
        BeefyId,
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
            derive_dev::<BeefyId>(s),
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
        BeefyId,
    ) {
        (
            AccountId::from(hex!("784e69Feba8a2FCCc938A722D5a66E9EbfA3A14A")), // Stash
            validator_1(),
            sp_core::sr25519::Public::from_raw(hex!(
                "f29f3491dc2baf6ffeffd01702a1b5289519c00a229d41544edc357d0355db51"
            ))
            .into(),
            sp_core::ed25519::Public::from_raw(hex!(
                "275fad28e7f2904a0341b5baa66b40f8941b09a22739a8b141f99b91e0dd9458"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "408300338038bb359afc7f32a0622d3be520988b5a89c3af5af0272e6745de5e"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "f870a88d596c9207b9df17fbc7960ba9f7fa25296fc3d17a844bc7680287011e"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "164b92db1487c67254182e9e231823b662fb39ffdee4e1ffe73559f600bebd25"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "7297d91787b2ec39853efaf1a553b4a5b58f834161a11f03575116e0340ada62"
            ))
            .into(),
            sp_core::ecdsa::Public::from_raw(hex!(
                "023a9999783ffc163ade9e2ac21a33c546b5bc7a4622641860f0f869a97f1e78d0"
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
        BeefyId,
    ) {
        (
            AccountId::from(hex!("309753d1BAc45489B9C4BdDEf28963d862AdCb13")), // Stash
            validator_2(),
            sp_core::sr25519::Public::from_raw(hex!(
                "80b57a74ddb35163ada69d61022d518cdad36eb63f766a04f9d2db35da28052f"
            ))
            .into(),
            sp_core::ed25519::Public::from_raw(hex!(
                "a4e37cd11ee58c2a6d529f42b13195295179df0921bf20d9f634145d71e817f1"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "527844f460f369100ca67a1fa084b9a29b71d984cd90479ce5bcd7efb74bde1c"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "7aec7e56d3de6cf85d23d38fea64107523ffeb43e17e27de6899cac625199a3d"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "acc2c4d8acefa119eee9a88a880bc490895c0aeb2a661daeccf2b6fcba30da3f"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "d2d0a556d5526c8114e7312a9d7220869894db1ae01f3ef7696f9f784fc58a4f"
            ))
            .into(),
            sp_core::ecdsa::Public::from_raw(hex!(
                "0247fc249f19a0a751d379c56e5f92be6f16942312f0b9fa8ecd09636e68c5d5e7"
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
        BeefyId,
    ) {
        (
            AccountId::from(hex!("A6543B65DD9cFA7e324AF616A339D3c1a13fa685")), // Stash
            validator_3(),
            sp_core::sr25519::Public::from_raw(hex!(
                "c2335d394c89693254fb1a323dc74d9c1a14f43ad3292081b331930f9fa8d072"
            ))
            .into(),
            sp_core::ed25519::Public::from_raw(hex!(
                "281a3b47515392d492faca42d616fa09e609b5fbbaa98716293ebf5c6d4e6248"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "3e99fe54593eeaf568029ec4989106286fd3384fc9c7b723d0e60bc3c3c02479"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "1eb253fc5186d7ec1bf2d28cc8120d97431745ead18381aca1cff47ebae0a83c"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "0075191c5441c7a2134c234f3ab393866deded809f21a37c9a6025ce26884556"
            ))
            .into(),
            sp_core::sr25519::Public::from_raw(hex!(
                "5425357e3002c6d2972e803362fe8648156837e70f3951929f13c0b9ba75c93b"
            ))
            .into(),
            sp_core::ecdsa::Public::from_raw(hex!(
                "03117d68a11002855b405c67d164e41a7244bb5fb5eced2d081f8a04458e1ee11d"
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
}

fn session_keys(
    babe: BabeId,
    grandpa: GrandpaId,
    im_online: ImOnlineId,
    para_validator: ValidatorId,
    para_assignment: AssignmentId,
    authority_discovery: AuthorityDiscoveryId,
    beefy: BeefyId,
) -> opaque::SessionKeys {
    opaque::SessionKeys {
        grandpa,
        babe,
        im_online,
        para_validator,
        para_assignment,
        authority_discovery,
        beefy,
    }
}

fn properties() -> Properties {
    let mut properties = Properties::new();
    properties.insert("isEthereum".into(), true.into());
    properties.insert("ss58Format".into(), SS58Prefix::get().into());
    properties.insert("tokenSymbol".into(), "VTRS".into());
    properties.insert("tokenDecimals".into(), 18.into());
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
        hrmp_channel_max_message_size: 1024 * 1024,
        hrmp_max_parachain_outbound_channels: 4,
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
