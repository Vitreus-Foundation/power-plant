#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256 (well, actually to 512).
#![recursion_limit = "512"]
#![allow(clippy::identity_op, clippy::new_without_default, clippy::or_fun_call)]
// #![cfg_attr(feature = "runtime-benchmarks", deny(unused_crate_dependencies))]

// Make the WASM binary available.
#[cfg(all(feature = "std", feature = "mainnet-runtime"))]
include!(concat!(env!("OUT_DIR"), "/vitreus_power_plant_mainnet_runtime"));
#[cfg(all(feature = "std", feature = "testnet-runtime"))]
include!(concat!(env!("OUT_DIR"), "/vitreus_power_plant_testnet_runtime"));

use fp_account::EthereumSignature;
use frame_support::{
    construct_runtime,
    weights::{constants::WEIGHT_REF_TIME_PER_MILLIS, Weight},
};
use pallet_reputation::REPUTATION_POINTS_PER_DAY;
use polkadot_runtime_common::{auctions, paras_registrar, paras_sudo_wrapper, prod_or_fast, slots};
use polkadot_runtime_parachains::{
    assigner_parachains as parachains_assigner_parachains,
    configuration as parachains_configuration, disputes as parachains_disputes,
    disputes::slashing as parachains_slashing, dmp as parachains_dmp, hrmp as parachains_hrmp,
    inclusion as parachains_inclusion, initializer as parachains_initializer,
    origin as parachains_origin, paras as parachains_paras,
    paras_inherent as parachains_paras_inherent, scheduler as parachains_scheduler,
    session_info as parachains_session_info, shared as parachains_shared,
};
use sp_core::{H160, H256};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys,
    traits::{BlakeTwo256, IdentifyAccount, Verify},
    Perbill, Vec,
};
use sp_version::RuntimeVersion;

pub use pallet_energy_generation::StakerStatus;
pub use pallet_reputation::ReputationPoint;

// A few exports that help ease life for downstream crates.
pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;

pub use pallet_sudo::Call as SudoCall;
pub use parachains_paras::Call as ParasCall;
pub use paras_sudo_wrapper::Call as ParasSudoWrapperCall;

pub use crate::{apis::*, configs::*, impls::*, precompiles::*};

mod apis;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;
mod configs;
mod impls;
mod migrations;
mod precompiles;
#[cfg(all(test, feature = "testnet-runtime"))]
mod tests;
mod weights;

/// Type of block number.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = EthereumSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem;

/// The address format for describing accounts.
pub type Address = AccountId;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
    pallet_energy_fee::CheckEnergyFee<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic =
    fp_self_contained::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra, H160>;

/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;

/// All migrations that will run on the next runtime upgrade.
///
/// This contains the combined migrations of the last 10 releases. It allows to skip runtime
/// upgrades in case governance decides to do so. THE ORDER IS IMPORTANT.
#[rustfmt::skip]
pub type Migrations = (
    migrations::Unreleased,
    migrations::V0213,
    migrations::Permanent,
);

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations,
>;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub grandpa: Grandpa,
            pub babe: Babe,
            pub im_online: ImOnline,
            pub para_validator: Initializer,
            pub para_assignment: ParaSessionInfo,
            pub authority_discovery: AuthorityDiscovery,
            pub beefy: Beefy,
        }
    }
}

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
    sp_consensus_babe::BabeEpochConfiguration {
        c: PRIMARY_PROBABILITY,
        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryVRFSlots,
    };

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("vitreus-power-plant"),
    impl_name: create_runtime_str!("vitreus-power-plant"),
    authoring_version: 1,
    spec_version: 213,
    impl_version: 0,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 4,
    state_version: 1,
};

// Time measurmement primitive
pub type Moment = u64;

pub const MILLISECS_PER_BLOCK: Moment = 6000;
pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;

pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;

// 1 in 4 blocks (on average, not counting collisions) will be primary BABE blocks.
pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);

// NOTE: Currently it is not possible to change the epoch duration after the chain has started.
//       Attempting to do so will brick block production.
pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = prod_or_fast!(60 * MINUTES, 10 * MINUTES);
pub const EPOCH_DURATION_IN_SLOTS: u64 = {
    const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

    (EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
};

// Time is measured by number of blocks.
// 60_000 ms per minute / ms per block
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;
pub const WEEKS: BlockNumber = DAYS * 7;
pub const MONTHS: BlockNumber = YEARS / 12;
pub const YEARS: BlockNumber = 36525 * (DAYS / 100);

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> sp_version::NativeVersion {
    sp_version::NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2000ms of compute with a 3 second average block time.
pub const WEIGHT_MILLISECS_PER_BLOCK: u64 = 2000;
pub const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(WEIGHT_MILLISECS_PER_BLOCK * WEIGHT_REF_TIME_PER_MILLIS, u64::MAX);
// 5 mb
pub const MAXIMUM_BLOCK_LENGTH: u32 = 5 * 1024 * 1024;

pub mod vtrs {
    use super::*;
    pub const UNITS: Balance = 1_000_000_000_000_000_000;
    pub const FEMTO_VTRS: Balance = 1_000;
    pub const PICO_VTRS: Balance = 1_000 * FEMTO_VTRS;
    pub const NANO_VTRS: Balance = 1_000 * PICO_VTRS;
    pub const MICRO_VTRS: Balance = 1_000 * NANO_VTRS;
    pub const MILLI_VTRS: Balance = 1_000 * MICRO_VTRS;
}
pub use vtrs::*;

pub mod vnrg {
    use super::*;
    pub const UNITS: Balance = 1_000_000_000_000_000_000;
}

const EXISTENTIAL_DEPOSIT: u128 = 100 * MICRO_VTRS;

// it takes a month to become a validator from 0
pub const VALIDATOR_REPUTATION_THRESHOLD: ReputationPoint =
    ReputationPoint::new(REPUTATION_POINTS_PER_DAY.0 * 30);
// it takes a month to become a collaborative validator from 0
pub const COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD: ReputationPoint =
    ReputationPoint::new(REPUTATION_POINTS_PER_DAY.0 * 30);

const BLOCK_GAS_LIMIT: u64 = 75_000_000;
const MAX_POV_SIZE: u64 = 5 * 1024 * 1024;

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
    pub enum Runtime {
        System: frame_system = 0,
        Timestamp: pallet_timestamp = 1,
        Babe: pallet_babe = 2,
        Grandpa: pallet_grandpa = 3,
        Balances: pallet_balances = 4,
        Assets: pallet_assets = 5,
        AssetRate: pallet_asset_rate = 6,
        TransactionPayment: pallet_transaction_payment = 7,
        Sudo: pallet_sudo = 8,
        PoolAssets: pallet_assets::<Instance1> = 9,
        AssetsFreezer: pallet_assets_freezer = 10,

        EVM: pallet_evm = 15,
        EVMChainId: pallet_evm_chain_id = 16,
        Ethereum: pallet_ethereum = 17,
        HotfixSufficients: pallet_hotfix_sufficients = 18,
        Nfts: pallet_nfts = 19,
        Reputation: pallet_reputation = 20,
        AtomicSwap: pallet_atomic_swap = 21,
        Claiming: pallet_claiming = 22,
        Vesting: pallet_vesting = 23,
        SimpleVesting: pallet_simple_vesting = 24,
        Kickstart: pallet_claiming::<Instance1> = 27,

        // Authorship must be before session in order to note author in the correct session and era
        // for im-online and staking.
        Authorship: pallet_authorship = 30,
        ImOnline: pallet_im_online = 31,
        NacManaging: pallet_nac_managing = 32,
        EnergyFee: pallet_energy_fee = 33,
        Offences: pallet_offences = 34,
        Session: pallet_session = 35,
        Utility: pallet_utility = 36,
        Historical: pallet_session::historical = 37,
        AuthorityDiscovery: pallet_authority_discovery = 38,
        EnergyGeneration: pallet_energy_generation = 39,
        EnergyBroker: pallet_energy_broker = 40,
        Privileges: pallet_privileges = 41,
        DynamicEnergy: pallet_dynamic_energy = 42,
        Proxy: pallet_proxy = 44,

        // Governance-related pallets
        Scheduler: pallet_scheduler = 45,
        Preimage: pallet_preimage = 46,
        Council: pallet_collective::<Instance1> = 47,
        TechnicalCommittee: pallet_collective::<Instance2> = 48,
        TechnicalMembership: pallet_membership::<Instance1> = 49,
        Treasury: pallet_treasury = 50,
        TreasuryExtension: pallet_treasury_extension = 51,
        Bounties: pallet_bounties = 52,
        Democracy: pallet_democracy = 53,
        Elections: pallet_elections_phragmen = 54,
        Multisig: pallet_multisig = 55,
        DemocracyExtension: pallet_democracy_extension = 56,
        TechnicalCommitteeTreasury: pallet_treasury::<Instance1> = 58,

        // Parachains pallets
        ParachainsOrigin: parachains_origin::{Pallet, Origin} = 60,
        Configuration: parachains_configuration::{Pallet, Call, Storage, Config<T>} = 61,
        ParasShared: parachains_shared::{Pallet, Call, Storage} = 62,
        ParaInclusion: parachains_inclusion::{Pallet, Call, Storage, Event<T>} = 63,
        ParaInherent: parachains_paras_inherent::{Pallet, Call, Storage, Inherent} = 64,
        ParaScheduler: parachains_scheduler::{Pallet, Storage} = 65,
        Paras: parachains_paras::{Pallet, Call, Storage, Event, Config<T>, ValidateUnsigned} = 66,
        Initializer: parachains_initializer::{Pallet, Call, Storage} = 67,
        Dmp: parachains_dmp::{Pallet, Storage} = 68,
        Hrmp: parachains_hrmp::{Pallet, Call, Storage, Event<T>, Config<T>} = 70,
        ParaSessionInfo: parachains_session_info::{Pallet, Storage} = 71,
        ParasDisputes: parachains_disputes::{Pallet, Call, Storage, Event<T>} = 72,
        ParasSlashing: parachains_slashing::{Pallet, Call, Storage, ValidateUnsigned} = 73,
        ParaAssignmentProvider: parachains_assigner_parachains = 74,

        // Parachain Onboarding Pallets. Start indices at 80 to leave room.
        Registrar: paras_registrar::{Pallet, Call, Storage, Event<T>} = 80,
        Slots: slots::{Pallet, Call, Storage, Event<T>} = 81,
        ParasSudoWrapper: paras_sudo_wrapper::{Pallet, Call} = 82,
        Auctions: auctions = 83,

        // Pallet for sending XCM.
        XcmPallet: pallet_xcm::{Pallet, Call, Storage, Event<T>, Origin, Config<T>} = 99,

        // Generalized message queue
        MessageQueue: pallet_message_queue::{Pallet, Call, Storage, Event<T>} = 100,

        // BEEFY Bridges support.
        Beefy: pallet_beefy::{Pallet, Call, Storage, Config<T>, ValidateUnsigned} = 200,
        // MMR leaf construction must be after session in order to have a leaf's next_auth_set
        // refer to block<N>. See https://github.com/polkadot-fellows/runtimes/issues/160 for details.
        Mmr: pallet_mmr = 201,
        MmrLeaf: pallet_beefy_mmr = 202,

        #[cfg(feature = "testnet-runtime")]
        Faucet: pallet_faucet = 240,

        ManualBridge: pallet_manual_bridge = 245,
    }
);

mod mmr {
    use super::Runtime;
    pub use pallet_mmr::primitives::*;

    pub type Leaf = <<Runtime as pallet_mmr::Config>::LeafData as LeafDataProvider>::LeafData;
    pub type Hashing = <Runtime as pallet_mmr::Config>::Hashing;
}
