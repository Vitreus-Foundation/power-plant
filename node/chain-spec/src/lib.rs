use hex_literal::hex;
use serde::{Deserialize, Serialize};
// Substrate
use sc_chain_spec::{ChainType, Properties};
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::ecdsa;
use sp_core::{storage::Storage, Pair, Public};
use sp_runtime::traits::{AccountIdConversion, IdentifyAccount, Verify};
use sp_runtime::{FixedU128, Perbill};
use sp_state_machine::BasicExternalities;
// Frontier
use vitreus_power_plant_runtime::{
    opaque, vtrs, AccountId, AssetsConfig, BabeConfig, Balance, BalancesConfig, CouncilConfig,
    EVMChainIdConfig, EnableManualSeal, EnergyFeeConfig, EnergyGenerationConfig, ImOnlineConfig,
    ImOnlineId, MaxCooperations, NacManagingConfig, ReputationConfig, RuntimeGenesisConfig,
    SS58Prefix, SessionConfig, Signature, StakerStatus, SudoConfig, SystemConfig,
    TechnicalCommitteeConfig, BABE_GENESIS_EPOCH_CONFIG,
    COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD, VNRG, WASM_BINARY,
};

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
        dynamic_fee: Default::default(),
        base_fee: Default::default(),
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
            accounts: endowed_accounts.iter().map(|x| (*x, 1)).collect(),
            owners: vec![root_key],
        },
        session: SessionConfig {
            keys: initial_validators
                .iter()
                .map(|x| (x.1, x.0, session_keys(x.2.clone(), x.3.clone(), x.4.clone())))
                .collect::<Vec<_>>(),
        },
        technical_committee: TechnicalCommitteeConfig {
            members: endowed_accounts.iter().cloned().skip(3).take(3).collect(),
            ..Default::default()
        },
        technical_membership: Default::default(),
        treasury: Default::default(),
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

    pub fn treasury() -> AccountId {
        vitreus_power_plant_runtime::areas::TreasuryPalletId::get().into_account_truncating()
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

    pub(super) fn validator_1_keys() -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
        (
            AccountId::from(hex!("784e69Feba8a2FCCc938A722D5a66E9EbfA3A14A")), // Stash
            validator_1(),
            sp_core::sr25519::Public(hex!(
                "f29f3491dc2baf6ffeffd01702a1b5289519c00a229d41544edc357d0355db51"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "488c73604a3da26d8f2547c71869d8a78542b008b55fc50bdea72751e702d142"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "408300338038bb359afc7f32a0622d3be520988b5a89c3af5af0272e6745de5e"
            ))
            .into(),
        )
    }

    pub(super) fn validator_2_keys() -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
        (
            AccountId::from(hex!("309753d1BAc45489B9C4BdDEf28963d862AdCb13")), // Stash
            validator_2(),
            sp_core::sr25519::Public(hex!(
                "80b57a74ddb35163ada69d61022d518cdad36eb63f766a04f9d2db35da28052f"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "735fa995b62b01c3ffc05f752a2fa708a46147dec40af60a7b3d5eeeb67c1415"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "527844f460f369100ca67a1fa084b9a29b71d984cd90479ce5bcd7efb74bde1c"
            ))
            .into(),
        )
    }

    pub(super) fn validator_3_keys() -> (AccountId, AccountId, BabeId, GrandpaId, ImOnlineId) {
        (
            AccountId::from(hex!("A6543B65DD9cFA7e324AF616A339D3c1a13fa685")), // Stash
            validator_3(),
            sp_core::sr25519::Public(hex!(
                "c2335d394c89693254fb1a323dc74d9c1a14f43ad3292081b331930f9fa8d072"
            ))
            .into(),
            sp_core::ed25519::Public(hex!(
                "7290d1a791f03dcc5b789d16b09c3ea586789931167339fba079bdb4c9f64c75"
            ))
            .into(),
            sp_core::sr25519::Public(hex!(
                "3e99fe54593eeaf568029ec4989106286fd3384fc9c7b723d0e60bc3c3c02479"
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
