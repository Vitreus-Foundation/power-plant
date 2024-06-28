#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256 (well, actually to 512).
#![recursion_limit = "512"]
#![allow(clippy::identity_op, clippy::new_without_default, clippy::or_fun_call)]
// #![cfg_attr(feature = "runtime-benchmarks", deny(unused_crate_dependencies))]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use frame_support::PalletId;
use pallet_balances::NegativeImbalance;
use polkadot_primitives::{
    runtime_api, slashing, CandidateCommitments, CandidateEvent, CandidateHash,
    CommittedCandidateReceipt, CoreState, DisputeState, ExecutorParams, GroupRotationInfo,
    Id as ParaId, InboundDownwardMessage, InboundHrmpMessage, OccupiedCoreAssumption,
    PersistedValidationData, PvfCheckStatement, ScrapedOnChainVotes, SessionInfo, ValidationCode,
    ValidationCodeHash, ValidatorId, ValidatorIndex, ValidatorSignature, PARACHAIN_KEY_TYPE_ID,
};

use runtime_common::{paras_registrar, paras_sudo_wrapper, prod_or_fast, slots};

use runtime_parachains::{
    configuration as parachains_configuration, disputes as parachains_disputes,
    disputes::slashing as parachains_slashing,
    dmp as parachains_dmp, hrmp as parachains_hrmp, inclusion as parachains_inclusion,
    inclusion::{AggregateMessageOrigin, UmpQueueId},
    initializer as parachains_initializer, origin as parachains_origin, paras as parachains_paras,
    paras_inherent as parachains_paras_inherent,
    runtime_api_impl::v5 as parachains_runtime_api_impl,
    scheduler as parachains_scheduler, session_info as parachains_session_info,
    shared as parachains_shared,
};

use ethereum::{EIP1559Transaction, EIP2930Transaction, LegacyTransaction};
use frame_support::pallet_prelude::{DispatchError, DispatchResult};
use frame_support::traits::tokens::{
    fungible::Inspect as FungibleInspect, fungibles::SwapNative, nonfungibles_v2::Inspect,
    DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence,
};
use frame_support::traits::{
    Currency, EitherOfDiverse, ExistenceRequirement, OnUnbalanced, ProcessMessage,
    ProcessMessageError, SignedImbalance, WithdrawReasons,
};
use orml_traits::GetByKey;
use parity_scale_codec::{Compact, Decode, Encode};
use sp_api::impl_runtime_apis;
use sp_core::{
    crypto::{ByteArray, KeyTypeId},
    OpaqueMetadata, H160, H256, U256,
};
use sp_runtime::traits::{AccountIdConversion, ConvertInto, Keccak256, Zero};
use sp_runtime::{
    create_runtime_str,
    curve::PiecewiseLinear,
    generic, impl_opaque_keys,
    traits::{
        BlakeTwo256, Block as BlockT, DispatchInfoOf, Dispatchable, Extrinsic, Get,
        IdentifyAccount, IdentityLookup, NumberFor, OpaqueKeys, PostDispatchInfoOf,
        SaturatedConversion, UniqueSaturatedInto, Verify,
    },
    transaction_validity::{
        TransactionPriority, TransactionSource, TransactionValidity, TransactionValidityError,
    },
    ApplyExtrinsicResult, ConsensusEngineId, FixedPointNumber, Perbill, Permill,
};
use sp_staking::{EraIndex, SessionIndex};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, prelude::*};
use sp_version::RuntimeVersion;
// Substrate FRAME
use energy_fee_runtime_api::CallRequest;
#[cfg(feature = "with-paritydb-weights")]
use frame_support::weights::constants::ParityDbWeight as RuntimeDbWeight;
#[cfg(feature = "with-rocksdb-weights")]
use frame_support::weights::constants::RocksDbWeight as RuntimeDbWeight;
use frame_support::{
    construct_runtime,
    dispatch::GetDispatchInfo,
    ord_parameter_types, parameter_types,
    traits::{
        fungible::ItemOf, AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, ConstU8,
        ExtrinsicCall, FindAuthor, Hooks, KeyOwnerProofSystem,
    },
    weights::{constants::WEIGHT_REF_TIME_PER_MILLIS, ConstantMultiplier, Weight, WeightMeter},
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy};
use pallet_energy_broker::{ConstantSum, NativeOrAssetId, NativeOrAssetIdConverter};
use pallet_energy_fee::{traits::AssetsBalancesConverter, CallFee, CustomFee, TokenExchange};
use pallet_grandpa::{
    fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};
use pallet_reputation::{ReputationTier, RANKS_PER_TIER, REPUTATION_POINTS_PER_DAY};
use pallet_transaction_payment::{FeeDetails, InclusionFee};
// Frontier
use fp_account::EthereumSignature;
use fp_evm::weight_per_gas;
use fp_rpc::TransactionStatus;
use pallet_ethereum::{Call::transact, PostLogContent, Transaction as EthereumTransaction};
use pallet_evm::{
    Account as EVMAccount, AddressMapping, EnsureAccountId20, FeeCalculator, GasWeightMapping,
    IdentityAddressMapping, Runner,
};
use pallet_nfts::PalletFeatures;
use sp_consensus_beefy::{
    crypto::AuthorityId as BeefyId,
    mmr::{BeefyDataProvider, MmrLeafVersion},
};
use sp_runtime::transaction_validity::InvalidTransaction;

pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;

pub use pallet_energy_generation::StakerStatus;
pub use pallet_nac_managing;
pub use pallet_privileges;
pub use pallet_reputation::ReputationPoint;

// A few exports that help ease life for downstream crates.
pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;

pub use pallet_sudo::Call as SudoCall;
pub use parachains_paras::Call as ParasCall;
pub use paras_sudo_wrapper::Call as ParasSudoWrapperCall;

pub use areas::{CouncilCollective, TechnicalCollective};

mod precompiles;
mod helpers {
    pub mod runner;
}
pub mod areas;
pub mod migrations;
#[cfg(test)]
mod tests;
mod weights;
mod xcm_config;

use precompiles::VitreusPrecompiles;

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

/// Energy of an account.
pub type Energy = Balance;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem;

/// Asset ID.
#[cfg(not(feature = "runtime-benchmarks"))]
pub type AssetId = u128;

#[cfg(feature = "runtime-benchmarks")]
pub type AssetId = u32;

/// Origin for council voting
type MoreThanHalfCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
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
        pub struct OldSessionKeys {
            pub grandpa: Grandpa,
            pub babe: Babe,
            pub im_online: ImOnline,
            pub para_validator: Initializer,
            pub para_assignment: ParaSessionInfo,
            pub authority_discovery: AuthorityDiscovery,
        }
    }

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

    // remove this when removing `OldSessionKeys`
    pub fn transform_session_keys(v: AccountId, old: OldSessionKeys) -> SessionKeys {
        log::info!("Update session keys for {:?}", v);

        SessionKeys {
            grandpa: old.grandpa,
            babe: old.babe,
            im_online: old.im_online,
            para_validator: old.para_validator,
            para_assignment: old.para_assignment,
            authority_discovery: old.authority_discovery,
            beefy: {
                let mut id: BeefyId =
                    sp_application_crypto::ecdsa::Public::from_raw([0u8; 33]).into();
                let id_raw: &mut [u8] = id.as_mut();
                id_raw[13..33].copy_from_slice(&v.0);
                id_raw[0..4].copy_from_slice(b"beef");
                id
            },
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
    spec_version: 115,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
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

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;
    pub const BlockHashCount: BlockNumber = 256;
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights
        ::with_sensible_defaults(MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO);
    pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
        ::max_with_normal_ratio(MAXIMUM_BLOCK_LENGTH, NORMAL_DISPATCH_RATIO);
    pub const SS58Prefix: u16 = 1943;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    /// The basic call filter to use in dispatchable.
    type BaseCallFilter = frame_support::traits::Everything;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = BlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = BlockLength;
    /// The ubiquitous origin type.
    type RuntimeOrigin = RuntimeOrigin;
    /// The aggregated dispatch type that is available for extrinsics.
    type RuntimeCall = RuntimeCall;
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = IdentityLookup<AccountId>;
    type Block = Block;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RuntimeDbWeight;
    /// Version of the runtime.
    type Version = Version;
    /// Converts a module to the index of the module in `construct_runtime!`.
    ///
    /// This type is being generated by `construct_runtime!`.
    type PalletInfo = PalletInfo;
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// What to do if a new account is created.
    type OnNewAccount = NacManaging;
    /// What to do if an account is fully reaped from the system.
    type OnKilledAccount = Reputation;
    /// Weight information for the extrinsics of this pallet.
    type SystemWeightInfo = ();
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    /// The set code logic, just the default since we're not a parachain.
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

parameter_types! {
    // NOTE: Currently it is not possible to change the epoch duration after the chain has started.
    //       Attempting to do so will brick block production.
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
    pub const ReportLongevity: u64 = 24 * 28 * 6 * EpochDuration::get();
        // BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
    pub const MaxAuthorities: u32 = 10_000;
}

impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = Session;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type KeyOwnerProof =
        <Historical as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    type WeightInfo = ();
    type MaxAuthorities = ConstU32<32>;
    type MaxSetIdSessionEntries = ConstU64<168>;

    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
    pub storage EnableManualSeal: bool = false;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

const EXISTENTIAL_DEPOSIT: u128 = 1;

parameter_types! {
    pub const ExistentialDeposit: u128 = EXISTENTIAL_DEPOSIT;
    // For weight estimation, we assume that the most locks on an individual account will be 50.
    // This number may need to be adjusted in the future if this assumption no longer holds true.
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
    pub const MaxFreezes: u32 = 8;
    pub const MaxHolds: u32 = 2;
}

impl pallet_balances::Config for Runtime {
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = ();
    type MaxFreezes = MaxFreezes;
    type RuntimeHoldReason = ();
    type MaxHolds = MaxHolds;
}

parameter_types! {
    pub const AssetDeposit: Balance = 100; // The deposit required to create an asset
    pub const AssetAccountDeposit: Balance = 10;
    pub const ApprovalDeposit: Balance = EXISTENTIAL_DEPOSIT;
    pub const AssetsStringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 100;
    pub const MetadataDepositPerByte: Balance = 2;
}

impl pallet_assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type AssetIdParameter = Compact<AssetId>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type RemoveItemsLimit = ConstU32<500>;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = AssetsStringLimit;
    type Freezer = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_reputation::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

use pallet_energy_generation::{EnergyRateCalculator, StakeOf, StashOf};

pallet_staking_reward_curve::build! {
    const I_NPOS: PiecewiseLinear<'static> = curve!(
        min_inflation: 0_025_000,
        max_inflation: 0_100_000,
        ideal_stake: 0_500_000,
        falloff: 0_050_000,
        max_piece_count: 40,
        test_precision: 0_005_000,
    );
}

impl pallet_session::Config for Runtime {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, EnergyGeneration>;
    type Keys = opaque::SessionKeys;
    type ShouldEndSession = Babe;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = StashOf<Runtime>;
    type NextSessionRotation = Babe;
    type WeightInfo = ();
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = pallet_energy_generation::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_energy_generation::ExposureOf<Runtime>;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type EventHandler = (EnergyGeneration, ImOnline);
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = EnergyGeneration;
}

impl pallet_authority_discovery::Config for Runtime {
    type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        public: <Signature as Verify>::Signer,
        account: AccountId,
        nonce: Index,
    ) -> Option<(RuntimeCall, <UncheckedExtrinsic as Extrinsic>::SignaturePayload)> {
        let tip = 0;
        // take the biggest period possible.
        let period =
            BlockHashCount::get().checked_next_power_of_two().map(|c| c / 2).unwrap_or(2) as u64;
        let current_block = System::block_number()
            .saturated_into::<u64>()
            // The `System::block_number` is initialized with `n+1`,
            // so the actual block number is `n`.
            .saturating_sub(1);
        let era = generic::Era::mortal(period, current_block);
        let extra = (
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(era),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
            pallet_energy_fee::CheckEnergyFee::<Runtime>::new(),
        );
        let raw_payload = SignedPayload::new(call, extra)
            .map_err(|e| {
                log::warn!("Unable to create signed payload: {:?}", e);
            })
            .ok()?;
        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
        // let address = AccountIdLookup::unlookup(account);
        let (call, extra, _) = raw_payload.deconstruct();
        Some((call, (account, signature, extra)))
    }
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = RuntimeCall;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = Babe;
    type ValidatorSet = Historical;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = pallet_im_online::weights::SubstrateWeight<Runtime>;
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
}

parameter_types! {
    pub const BeefySetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

impl pallet_beefy::Config for Runtime {
    type BeefyId = BeefyId;
    type MaxAuthorities = MaxAuthorities;
    type MaxSetIdSessionEntries = BeefySetIdSessionEntries;
    type OnNewValidatorSet = MmrLeaf;
    type WeightInfo = ();
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, BeefyId)>>::Proof;
    type EquivocationReportSystem =
        pallet_beefy::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

mod mmr {
    use super::Runtime;
    pub use pallet_mmr::primitives::*;

    pub type Leaf = <<Runtime as pallet_mmr::Config>::LeafData as LeafDataProvider>::LeafData;
    pub type Hashing = <Runtime as pallet_mmr::Config>::Hashing;
}

impl pallet_mmr::Config for Runtime {
    const INDEXING_PREFIX: &'static [u8] = mmr::INDEXING_PREFIX;
    type Hashing = Keccak256;
    type OnNewRoot = pallet_beefy_mmr::DepositBeefyDigest<Runtime>;
    type WeightInfo = ();
    type LeafData = pallet_beefy_mmr::Pallet<Runtime>;
}

pub struct ParasProvider;
impl BeefyDataProvider<H256> for ParasProvider {
    fn extra_data() -> H256 {
        let mut para_heads: Vec<(u32, Vec<u8>)> = Paras::parachains()
            .into_iter()
            .filter_map(|id| Paras::para_head(id).map(|head| (id.into(), head.0)))
            .collect();
        para_heads.sort();
        binary_merkle_tree::merkle_root::<mmr::Hashing, _>(
            para_heads.into_iter().map(|pair| pair.encode()),
        )
    }
}

impl pallet_beefy_mmr::Config for Runtime {
    type LeafVersion = LeafVersion;
    type BeefyAuthorityToMerkleLeaf = pallet_beefy_mmr::BeefyEcdsaToEthereum;
    type LeafExtra = H256;
    type BeefyDataProvider = ParasProvider;
}

parameter_types! {
    /// Version of the produced MMR leaf.
    ///
    /// The version consists of two parts;
    /// - `major` (3 bits)
    /// - `minor` (5 bits)
    ///
    /// `major` should be updated only if decoding the previous MMR Leaf format from the payload
    /// is not possible (i.e. backward incompatible change).
    /// `minor` should be updated if fields are added to the previous MMR Leaf, which given SCALE
    /// encoding does not prevent old leafs from being decoded.
    ///
    /// Hence we expect `major` to be changed really rarely (think never).
    /// See [`MmrLeafVersion`] type documentation for more details.
    pub LeafVersion: MmrLeafVersion = MmrLeafVersion::new(0, 0);
}

// it takes a month to become a validator from 0
pub const VALIDATOR_REPUTATION_THRESHOLD: ReputationPoint =
    ReputationPoint::new(REPUTATION_POINTS_PER_DAY.0 * 30);
// it takes 2 months to become a collaborative validator from 0
pub const COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD: ReputationPoint =
    ReputationPoint::new(REPUTATION_POINTS_PER_DAY.0 * 30);

parameter_types! {
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &I_NPOS;
    pub const SessionsPerEra: SessionIndex = prod_or_fast!(4, 1);
    pub const BondingDuration: EraIndex = prod_or_fast!(42, 5);
    // TODO: consider removing, since the slash defer feature was removed
    pub const SlashDeferDuration: EraIndex = 0;
    pub const Period: BlockNumber = 5;
    pub const Offset: BlockNumber = 0;
    pub const VNRG: AssetId = 1;
    pub const BatterySlotCapacity: Energy = 100_000_000_000;
    pub const MaxCooperations: u32 = 256;
    pub const HistoryDepth: u32 = 84;
    pub const MaxUnlockingChunks: u32 = 64;
    pub const RewardOnUnbalanceWasCalled: bool = false;
    pub const MaxWinners: u32 = 100;
    // it takes a month to become a validator from 0
    pub const ValidatorReputationTier: ReputationTier = ReputationTier::Vanguard(1);
    // it takes 2 months to become a collaborative validator from 0
    pub const CollaborativeValidatorReputationTier: ReputationTier = ReputationTier::Vanguard(1);
    pub const RewardRemainderUnbalanced: u128 = 0;
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

pub struct EnergyPerStakeCurrency;

impl EnergyRateCalculator<StakeOf<Runtime>, Energy> for EnergyPerStakeCurrency {
    fn calculate_energy_rate(
        _total_staked: StakeOf<Runtime>,
        _total_issuance: Energy,
        _core_nodes_num: u32,
        _battery_slot_cap: Energy,
    ) -> Energy {
        19_909_091_036_891
    }
}

pub struct EnergyPerReputationPoint;

impl EnergyRateCalculator<StakeOf<Runtime>, Energy> for EnergyPerReputationPoint {
    fn calculate_energy_rate(
        _total_staked: StakeOf<Runtime>,
        _total_issuance: Energy,
        _core_nodes_num: u32,
        _battery_slot_cap: Energy,
    ) -> Energy {
        1_000
    }
}

pub struct ReputationTierEnergyRewardAdditionalPercentMapping;

impl GetByKey<ReputationTier, Perbill> for ReputationTierEnergyRewardAdditionalPercentMapping {
    fn get(k: &ReputationTier) -> Perbill {
        match k {
            ReputationTier::Vanguard(2) => Perbill::from_percent(2),
            ReputationTier::Vanguard(3) => Perbill::from_percent(4),
            ReputationTier::Trailblazer(0) => Perbill::from_percent(5),
            ReputationTier::Trailblazer(1) => Perbill::from_percent(8),
            ReputationTier::Trailblazer(2) => Perbill::from_percent(10),
            ReputationTier::Trailblazer(3) => Perbill::from_percent(12),
            ReputationTier::Ultramodern(0) => Perbill::from_percent(13),
            ReputationTier::Ultramodern(1) => Perbill::from_percent(16),
            ReputationTier::Ultramodern(2) => Perbill::from_percent(18),
            ReputationTier::Ultramodern(3) => Perbill::from_percent(20),
            ReputationTier::Ultramodern(rank) => {
                let additional_percentage = rank.saturating_sub(RANKS_PER_TIER);
                Perbill::from_percent(20_u8.saturating_add(additional_percentage).into())
            },
            // includes unhandled cases
            _ => Perbill::zero(),
        }
    }
}

pub struct EnergyGenerationBenchmarkConfig;
impl pallet_energy_generation::BenchmarkingConfig for EnergyGenerationBenchmarkConfig {
    type MaxValidators = ConstU32<1000>;
    type MaxCooperators = ConstU32<1000>;
}

type EnergyGenerationAdminOrigin = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
>;

impl pallet_energy_generation::Config for Runtime {
    type AdminOrigin = EnergyGenerationAdminOrigin;
    type BatterySlotCapacity = BatterySlotCapacity;
    type BenchmarkingConfig = EnergyGenerationBenchmarkConfig;
    type BondingDuration = BondingDuration;
    type CollaborativeValidatorReputationTier = CollaborativeValidatorReputationTier;
    type ValidatorReputationTier = ValidatorReputationTier;
    type EnergyAssetId = VNRG;
    type EnergyPerStakeCurrency = EnergyGeneration;
    type HistoryDepth = HistoryDepth;
    type MaxCooperations = MaxCooperations;
    type MaxCooperatorRewardedPerValidator = ConstU32<128>;
    type MaxUnlockingChunks = MaxUnlockingChunks;
    type NextNewSession = Session;
    type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
    type EventListeners = ();
    type ReputationTierEnergyRewardAdditionalPercentMapping =
        ReputationTierEnergyRewardAdditionalPercentMapping;
    type Reward = ();
    type RewardRemainder = Treasury;
    type RuntimeEvent = RuntimeEvent;
    type SessionInterface = Self;
    type SessionsPerEra = SessionsPerEra;
    type Slash = Treasury;
    type SlashDeferDuration = SlashDeferDuration;
    type StakeBalance = Balance;
    type StakeCurrency = Balances;
    type OnVipMembershipHandler = Privileges;
    type ThisWeightInfo = ();
    type UnixTime = Timestamp;
}

parameter_types! {
    // Setting this to value > 0 would break nac-managing
    pub const CollectionDeposit: Balance = 0;
    // Setting this to value > 0 would break nac-managing
    pub const ItemDeposit: Balance = 0;
    pub const KeyLimit: u32 = 32;
    pub const ValueLimit: u32 = 256;
    pub const ApprovalsLimit: u32 = 20;
    pub const ItemAttributesApprovalsLimit: u32 = 20;
    pub const MaxTips: u32 = 10;
    pub const MaxDeadlineDuration: BlockNumber = 12 * 30 * DAYS;
    pub const MaxAttributesPerCall: u32 = 10;
    pub Features: PalletFeatures = PalletFeatures::all_enabled();
}

type CollectionId = u32;
type ItemId = u32;

impl pallet_nfts::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureRoot<AccountId>;
    type CollectionDeposit = CollectionDeposit;
    type ApprovalsLimit = ();
    type ItemAttributesApprovalsLimit = ();
    type MaxTips = ();
    type MaxDeadlineDuration = ();
    type MaxAttributesPerCall = ();
    type Features = ();
    type OffchainPublic = <Signature as Verify>::Signer;
    type OffchainSignature = Signature;
    type ItemDeposit = ItemDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type AttributeDepositBase = MetadataDepositBase;
    type DepositPerByte = MetadataDepositPerByte;
    type StringLimit = AssetsStringLimit;
    type KeyLimit = KeyLimit;
    type ValueLimit = ValueLimit;
    type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    // TODO: do we want to allow regular users create nfts?
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type Locker = ();
}

parameter_types! {
    pub const NftCollectionId: CollectionId = 0;
    pub const VIPPCollectionId: CollectionId = 1;
}

impl pallet_nac_managing::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Nfts = Nfts;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type AdminOrigin = EnsureRoot<Self::AccountId>;
    type WeightInfo = pallet_nac_managing::weights::SubstrateWeight<Runtime>;
    type Currency = Balances;
    type OnVIPPChanged = Privileges;
    type NftCollectionId = NftCollectionId;
    type VIPPCollectionId = VIPPCollectionId;
}

impl pallet_privileges::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type UnixTime = Timestamp;
    type WeightInfo = pallet_privileges::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const TransactionByteFee: Balance = 1;
    pub const TransactionPicosecondFee: Balance = 8;
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = EnergyFee;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = ConstantMultiplier<Balance, TransactionPicosecondFee>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = EnergyFee;
}

impl pallet_asset_rate::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CreateOrigin = MoreThanHalfCouncil;
    type RemoveOrigin = MoreThanHalfCouncil;
    type UpdateOrigin = MoreThanHalfCouncil;
    type AssetId = AssetId;
    type Currency = Balances;
    type Balance = Balance;
    type WeightInfo = pallet_asset_rate::weights::SubstrateWeight<Runtime>;
}

pub type PoolAssetsInstance = pallet_assets::Instance1;
impl pallet_assets::Config<PoolAssetsInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type AssetIdParameter = Compact<AssetId>;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSignedBy<AssetConversionOrigin, AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    // Deposits are zero because creation/admin is limited to Asset Conversion pallet.
    type AssetDeposit = ConstU128<0>;
    type AssetAccountDeposit = ConstU128<0>;
    type MetadataDepositBase = ConstU128<0>;
    type MetadataDepositPerByte = ConstU128<0>;
    type RemoveItemsLimit = ConstU32<500>;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = AssetsStringLimit;
    type Freezer = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const AssetConversionPalletId: PalletId = PalletId(*b"py/ascon");
    pub const AllowMultiAssetPools: bool = false;
    pub const LiquidityWithdrawalFee: Permill = Permill::from_percent(1);
    pub const PoolSetupFee: Balance = 0;
    pub const PoolSwapFee: u32 = 10; // 1%
    pub const MaxSwapPathLength: u32 = 2;
    pub const MintMinLiquidity: Balance = 100;
}

ord_parameter_types! {
    pub const AssetConversionOrigin: AccountId =
        AccountIdConversion::<AccountId>::into_account_truncating(&AssetConversionPalletId::get());
}

type EnergyRate = AssetsBalancesConverter<Runtime, AssetRate>;
type EnergyItem = ItemOf<Assets, VNRG, AccountId>;

impl pallet_energy_broker::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Formula = ConstantSum<EnergyRate>;
    type Currency = Balances;
    type Balance = Balance;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetBalance = Balance;
    type AssetId = AssetId;
    type Assets = Assets;
    type PoolAssetId = AssetId;
    type PoolAssets = PoolAssets;
    type PalletId = AssetConversionPalletId;
    type LPFee = PoolSwapFee;
    type PoolSetupFee = PoolSetupFee;
    type PoolSetupFeeReceiver = AssetConversionOrigin;
    type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
    type AllowMultiAssetPools = AllowMultiAssetPools;
    type MaxSwapPathLength = MaxSwapPathLength;
    type MintMinLiquidity = MintMinLiquidity;
    type MultiAssetId = NativeOrAssetId<AssetId>;
    type MultiAssetIdConverter = NativeOrAssetIdConverter<AssetId>;
    type WeightInfo = pallet_energy_broker::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const GetConstantEnergyFee: Balance = 1_000_000_000;
    pub GetConstantGasLimit: U256 = U256::from(100_000);
    pub EnergyBrokerPalletId: PalletId = PalletId(*b"enrgbrkr");
}

pub struct EnergyBrokerSink;

impl OnUnbalanced<NegativeImbalance<Runtime>> for EnergyBrokerSink {
    fn on_nonzero_unbalanced(amount: NegativeImbalance<Runtime>) {
        let energy_broker_address: AccountId =
            EnergyBrokerPalletId::get().into_account_truncating();
        Balances::resolve_creating(&energy_broker_address, amount);
    }
}

pub struct EnergyBrokerExchange;

impl TokenExchange<AccountId, Balances, EnergyItem, EnergyBrokerSink, Balance>
    for EnergyBrokerExchange
{
    fn convert_from_input(amount: Balance) -> Result<Balance, DispatchError> {
        EnergyBroker::get_amount_out(
            &amount,
            (&NativeOrAssetId::Native, &NativeOrAssetId::Asset(VNRG::get())),
        )
        .map_err(|e| e.into())
    }

    fn convert_from_output(amount: Balance) -> Result<Balance, DispatchError> {
        EnergyBroker::get_amount_in(
            &amount,
            (&NativeOrAssetId::Native, &NativeOrAssetId::Asset(VNRG::get())),
        )
        .map_err(|e| e.into())
    }

    fn exchange_from_input(who: &AccountId, amount: Balance) -> Result<Balance, DispatchError> {
        EnergyBroker::swap_exact_native_for_tokens(*who, VNRG::get(), amount, None, *who, true)
    }

    fn exchange_from_output(who: &AccountId, amount: Balance) -> Result<Balance, DispatchError> {
        EnergyBroker::swap_native_for_exact_tokens(*who, VNRG::get(), amount, None, *who, true)
    }

    fn exchange_inner(
        _who: &AccountId,
        _amount_in: Balance,
        _amount_out: Balance,
    ) -> Result<Balance, DispatchError> {
        Err(DispatchError::Other("Unimplemented"))
    }
}

impl pallet_energy_fee::Config for Runtime {
    type ManageOrigin = MoreThanHalfCouncil;
    type RuntimeEvent = RuntimeEvent;
    type FeeTokenBalanced = EnergyItem;
    type MainTokenBalanced = Balances;
    type EnergyExchange = EnergyBrokerExchange;
    type GetConstantFee = GetConstantEnergyFee;
    type CustomFee = EnergyFee;
    type EnergyAssetId = VNRG;
    type MainRecycleDestination = EnergyBrokerSink;
    type FeeRecycleDestination = ();
}

parameter_types! {
    pub const ProofLimit: u32 = 2048;
}

impl pallet_atomic_swap::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SwapAction = pallet_atomic_swap::BalanceSwapAction<Self::AccountId, Balances>;
    type ProofLimit = ProofLimit;
}

parameter_types! {
    pub Prefix: &'static [u8] = b"Pay VTRS to the Vitreus:";
}

impl pallet_claiming::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type VestingSchedule = Vesting;
    type OnClaim = NacManaging;
    type Prefix = Prefix;
    type WeightInfo = ();
}

parameter_types! {
    pub const MinVestedTransfer: Balance = 1;
    pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
        WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

impl pallet_vesting::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = ConvertInto;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = pallet_vesting::weights::SubstrateWeight<Runtime>;
    type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
    const MAX_VESTING_SCHEDULES: u32 = 28;
}

impl pallet_simple_vesting::Config for Runtime {
    type Currency = Balances;
    type BlockNumberToBalance = ConvertInto;
}

// We implement CusomFee here since the RuntimeCall defined in construct_runtime! macro
impl CustomFee<RuntimeCall, DispatchInfoOf<RuntimeCall>, Balance, GetConstantEnergyFee>
    for EnergyFee
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfoOf<RuntimeCall>>,
        calculated_fee: Option<Balance>,
    ) -> CallFee<Balance> {
        match runtime_call {
            RuntimeCall::Assets(..)
            | RuntimeCall::AssetRate(..)
            | RuntimeCall::Balances(..)
            | RuntimeCall::Bounties(..)
            | RuntimeCall::EnergyGeneration(..)
            | RuntimeCall::EnergyBroker(..)
            | RuntimeCall::Nfts(..)
            | RuntimeCall::AtomicSwap(..)
            | RuntimeCall::Claiming(..)
            | RuntimeCall::Vesting(..)
            | RuntimeCall::NacManaging(..)
            | RuntimeCall::Privileges(..)
            | RuntimeCall::Council(..)
            | RuntimeCall::TechnicalCommittee(..)
            | RuntimeCall::TechnicalMembership(..)
            | RuntimeCall::Treasury(..)
            | RuntimeCall::Democracy(..)
            | RuntimeCall::Session(..)
            | RuntimeCall::XcmPallet(..)
            | RuntimeCall::Reputation(..) => CallFee::Regular(Self::custom_fee()),
            RuntimeCall::EVM(..) | RuntimeCall::Ethereum(..) => CallFee::EVM(Self::ethereum_fee()),
            RuntimeCall::Utility(pallet_utility::Call::batch { calls })
            | RuntimeCall::Utility(pallet_utility::Call::batch_all { calls })
            | RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => {
                let resulting_fee = calls
                    .iter()
                    .map(|call| Self::dispatch_info_to_fee(call, None, None))
                    .fold(Balance::zero(), |acc, call_fee| match call_fee {
                        CallFee::Regular(fee) => acc.saturating_add(fee),
                        CallFee::EVM(fee) => acc.saturating_add(fee),
                    });
                CallFee::Regular(resulting_fee)
            },
            RuntimeCall::Utility(pallet_utility::Call::dispatch_as { call, .. })
            | RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. }) => {
                Self::dispatch_info_to_fee(call, None, calculated_fee)
            },
            RuntimeCall::Sudo(..) => CallFee::Regular(0),
            _ => CallFee::Regular(Self::weight_fee(runtime_call, dispatch_info, calculated_fee)),
        }
    }

    fn custom_fee() -> Balance {
        let next_multiplier = TransactionPayment::next_fee_multiplier();
        next_multiplier.saturating_mul_int(EnergyFee::base_fee())
    }

    fn weight_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfoOf<RuntimeCall>>,
        calculated_fee: Option<Balance>,
    ) -> Balance {
        if let Some(fee) = calculated_fee {
            fee
        } else {
            let len = runtime_call.encode().len() as u32;
            if let Some(info) = dispatch_info {
                pallet_transaction_payment::Pallet::<Runtime>::compute_fee(len, info, Zero::zero())
            } else {
                let info = &runtime_call.get_dispatch_info();
                pallet_transaction_payment::Pallet::<Runtime>::compute_fee(len, info, Zero::zero())
            }
        }
    }
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

impl pallet_evm_chain_id::Config for Runtime {}

pub struct FindAuthorTruncated<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for FindAuthorTruncated<F> {
    fn find_author<'a, I>(digests: I) -> Option<H160>
    where
        I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
    {
        if let Some(author_index) = F::find_author(digests) {
            let (public, _) = Babe::authorities()[author_index as usize].clone();
            return Some(H160::from_slice(&public.to_raw_vec()[4..24]));
        }
        None
    }
}

pub struct FixedFeeCalculator;
impl FeeCalculator for FixedFeeCalculator {
    fn min_gas_price() -> (U256, Weight) {
        (U256::one(), Weight::zero())
    }
}

const BLOCK_GAS_LIMIT: u64 = 75_000_000;
const MAX_POV_SIZE: u64 = 5 * 1024 * 1024;

parameter_types! {
    pub BlockGasLimit: U256 = U256::from(BLOCK_GAS_LIMIT);
    pub const GasLimitPovSizeRatio: u64 = BLOCK_GAS_LIMIT.saturating_div(MAX_POV_SIZE);
    pub PrecompilesValue: VitreusPrecompiles<Runtime> = VitreusPrecompiles::<_>::new();
    pub WeightPerGas: Weight =
        Weight::from_parts(weight_per_gas(
                BLOCK_GAS_LIMIT, NORMAL_DISPATCH_RATIO, WEIGHT_MILLISECS_PER_BLOCK
                ),
            0,
        );
}

/// Helper struct which mimics some functionality of the Balances pallet.
///
/// Used in pallet_evm for correct work of fee calculation. The only difference between Balances
/// pallet and this struct is the implementation of the reducible balance, due to the fact that tx
/// fee can be paid as in VTRS as in VNRG.
pub struct QuasiBalances;

impl Currency<AccountId> for QuasiBalances {
    type Balance = <Balances as Currency<AccountId>>::Balance;

    type PositiveImbalance = <Balances as Currency<AccountId>>::PositiveImbalance;

    type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

    fn total_balance(who: &AccountId) -> Self::Balance {
        <Balances as Currency<AccountId>>::total_balance(who)
    }

    fn can_slash(who: &AccountId, value: Self::Balance) -> bool {
        <Balances as Currency<AccountId>>::can_slash(who, value)
    }

    fn total_issuance() -> Self::Balance {
        <Balances as Currency<AccountId>>::total_issuance()
    }

    fn minimum_balance() -> Self::Balance {
        <Balances as Currency<AccountId>>::minimum_balance()
    }

    fn burn(amount: Self::Balance) -> Self::PositiveImbalance {
        <Balances as Currency<AccountId>>::burn(amount)
    }

    fn issue(amount: Self::Balance) -> Self::NegativeImbalance {
        <Balances as Currency<AccountId>>::issue(amount)
    }

    fn free_balance(who: &AccountId) -> Self::Balance {
        <Balances as Currency<AccountId>>::free_balance(who)
    }

    fn ensure_can_withdraw(
        who: &AccountId,
        _amount: Self::Balance,
        reasons: WithdrawReasons,
        new_balance: Self::Balance,
    ) -> DispatchResult {
        <Balances as Currency<AccountId>>::ensure_can_withdraw(who, _amount, reasons, new_balance)
    }

    fn transfer(
        source: &AccountId,
        dest: &AccountId,
        value: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        <Balances as Currency<AccountId>>::transfer(source, dest, value, existence_requirement)
    }

    fn slash(who: &AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        <Balances as Currency<AccountId>>::slash(who, value)
    }

    fn deposit_into_existing(
        who: &AccountId,
        value: Self::Balance,
    ) -> Result<Self::PositiveImbalance, DispatchError> {
        <Balances as Currency<AccountId>>::deposit_into_existing(who, value)
    }

    fn deposit_creating(who: &AccountId, value: Self::Balance) -> Self::PositiveImbalance {
        <Balances as Currency<AccountId>>::deposit_creating(who, value)
    }

    fn withdraw(
        who: &AccountId,
        value: Self::Balance,
        reasons: WithdrawReasons,
        liveness: ExistenceRequirement,
    ) -> Result<Self::NegativeImbalance, DispatchError> {
        <Balances as Currency<AccountId>>::withdraw(who, value, reasons, liveness)
    }

    fn make_free_balance_be(
        who: &AccountId,
        balance: Self::Balance,
    ) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
        <Balances as Currency<AccountId>>::make_free_balance_be(who, balance)
    }
}

impl FungibleInspect<AccountId> for QuasiBalances {
    type Balance = <Balances as FungibleInspect<AccountId>>::Balance;

    fn total_issuance() -> Self::Balance {
        <Balances as FungibleInspect<AccountId>>::total_issuance()
    }

    fn minimum_balance() -> Self::Balance {
        <Balances as FungibleInspect<AccountId>>::minimum_balance()
    }

    fn total_balance(who: &AccountId) -> Self::Balance {
        <Balances as FungibleInspect<AccountId>>::total_balance(who)
    }

    fn balance(who: &AccountId) -> Self::Balance {
        <Balances as FungibleInspect<AccountId>>::balance(who)
    }

    fn reducible_balance(
        _who: &AccountId,
        _preservation: Preservation,
        _force: Fortitude,
    ) -> Self::Balance {
        1_000_000_000_000_000_000_000
    }

    fn can_deposit(
        who: &AccountId,
        amount: Self::Balance,
        provenance: Provenance,
    ) -> DepositConsequence {
        <Balances as FungibleInspect<AccountId>>::can_deposit(who, amount, provenance)
    }

    fn can_withdraw(who: &AccountId, amount: Self::Balance) -> WithdrawConsequence<Self::Balance> {
        <Balances as FungibleInspect<AccountId>>::can_withdraw(who, amount)
    }
}

impl pallet_evm::Config for Runtime {
    type AddressMapping = IdentityAddressMapping;
    type BlockGasLimit = BlockGasLimit;
    type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
    type CallOrigin = EnsureAccountId20;
    type ChainId = EVMChainId;
    type Currency = QuasiBalances;
    type Runner = helpers::runner::NacRunner<Self>;
    type RuntimeEvent = RuntimeEvent;
    type WeightPerGas = WeightPerGas;
    type WithdrawOrigin = EnsureAccountId20;
    type OnCreate = ();
    type Timestamp = Timestamp;
    type FeeCalculator = FixedFeeCalculator;
    type FindAuthor = FindAuthorTruncated<Babe>;
    type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type OnChargeTransaction = EnergyFee;
    type PrecompilesType = VitreusPrecompiles<Self>;
    type PrecompilesValue = PrecompilesValue;
    type WeightInfo = pallet_evm::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const PostBlockAndTxnHashes: PostLogContent = PostLogContent::BlockAndTxnHashes;
}

impl pallet_ethereum::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
    type PostLogContent = PostBlockAndTxnHashes;
    type ExtraDataLength = ConstU32<30>;
}

parameter_types! {
    pub DefaultElasticity: Permill = Permill::from_parts(1_000_000);
}

impl pallet_hotfix_sufficients::Config for Runtime {
    type AddressMapping = IdentityAddressMapping;
    type WeightInfo = pallet_hotfix_sufficients::weights::SubstrateWeight<Runtime>;
}

impl parachains_origin::Config for Runtime {}

impl parachains_configuration::Config for Runtime {
    type WeightInfo = weights::runtime_parachains_configuration::WeightInfo<Runtime>;
}

impl parachains_shared::Config for Runtime {}

impl parachains_session_info::Config for Runtime {
    type ValidatorSet = Historical;
}

/// Special `RewardValidators` that does nothing ;)
pub struct RewardValidators;
impl runtime_parachains::inclusion::RewardValidators for RewardValidators {
    fn reward_backing(_: impl IntoIterator<Item = ValidatorIndex>) {}
    fn reward_bitfields(_: impl IntoIterator<Item = ValidatorIndex>) {}
}

impl parachains_inclusion::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DisputesHandler = ParasDisputes;
    type RewardValidators = RewardValidators;
    type MessageQueue = MessageQueue;
    type WeightInfo = weights::runtime_parachains_inclusion::WeightInfo<Runtime>;
}

parameter_types! {
    pub const ParasUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

impl parachains_paras::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = weights::runtime_parachains_paras::WeightInfo<Runtime>;
    type UnsignedPriority = ParasUnsignedPriority;
    type QueueFootprinter = ParaInclusion;
    type NextSessionRotation = Babe;
}

parameter_types! {
    /// Amount of weight that can be spent per block to service messages.
    ///
    /// # WARNING
    ///
    /// This is not a good value for para-chains since the `Scheduler` already uses up to 80% block weight.
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(20) * BlockWeights::get().max_block;
    pub const MessageQueueHeapSize: u32 = 65_536;
    pub const MessageQueueMaxStale: u32 = 8;
}

/// Message processor to handle any messages that were enqueued into the `MessageQueue` pallet.
pub struct MessageProcessor;
impl ProcessMessage for MessageProcessor {
    type Origin = AggregateMessageOrigin;

    fn process_message(
        message: &[u8],
        origin: Self::Origin,
        meter: &mut WeightMeter,
        id: &mut [u8; 32],
    ) -> Result<bool, ProcessMessageError> {
        use xcm::latest::Junction;

        let para = match origin {
            AggregateMessageOrigin::Ump(UmpQueueId::Para(para)) => para,
        };
        xcm_builder::ProcessXcmMessage::<
            Junction,
            xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
            RuntimeCall,
        >::process_message(message, Junction::Parachain(para.into()), meter, id)
    }
}

impl pallet_message_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Size = u32;
    type HeapSize = MessageQueueHeapSize;
    type MaxStale = MessageQueueMaxStale;
    type ServiceWeight = MessageQueueServiceWeight;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type MessageProcessor = MessageProcessor;
    #[cfg(feature = "runtime-benchmarks")]
    type MessageProcessor =
        pallet_message_queue::mock_helpers::NoopMessageProcessor<AggregateMessageOrigin>;
    type QueueChangeHandler = ParaInclusion;
    type QueuePausedQuery = ();
    type WeightInfo = pallet_message_queue::weights::SubstrateWeight<Runtime>;
}

impl parachains_dmp::Config for Runtime {}

impl parachains_hrmp::Config for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type WeightInfo = weights::runtime_parachains_hrmp::WeightInfo<Self>;
}

impl parachains_paras_inherent::Config for Runtime {
    type WeightInfo = weights::runtime_parachains_paras_inherent::WeightInfo<Runtime>;
}

impl parachains_scheduler::Config for Runtime {}

impl parachains_initializer::Config for Runtime {
    type Randomness = pallet_babe::RandomnessFromOneEpochAgo<Runtime>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type WeightInfo = weights::runtime_parachains_initializer::WeightInfo<Runtime>;
}

impl parachains_disputes::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RewardValidators = ();
    type SlashingHandler = parachains_slashing::SlashValidatorsForDisputes<ParasSlashing>;
    type WeightInfo = weights::runtime_parachains_disputes::WeightInfo<Runtime>;
}

impl parachains_slashing::Config for Runtime {
    type KeyOwnerProofSystem = Historical;
    type KeyOwnerProof =
        <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, ValidatorId)>>::Proof;
    type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        ValidatorId,
    )>>::IdentificationTuple;
    type HandleReports = parachains_slashing::SlashingReportHandler<
        Self::KeyOwnerIdentification,
        Offences,
        ReportLongevity,
    >;
    type WeightInfo = weights::runtime_parachains_disputes_slashing::WeightInfo<Runtime>;
    type BenchmarkingConfig = parachains_slashing::BenchConfig<1000>;
}

parameter_types! {
    pub const ParaDeposit: Balance = 20000 * UNITS;
    pub const ParaDataByteDeposit: Balance = 2;
}

impl paras_registrar::Config for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type OnSwap = Slots;
    type ParaDeposit = ParaDeposit;
    type DataDepositPerByte = ParaDataByteDeposit;
    type WeightInfo = weights::runtime_common_paras_registrar::WeightInfo<Runtime>;
}

parameter_types! {
    pub LeasePeriod: BlockNumber = prod_or_fast!(1 * DAYS, 4 * WEEKS, "VITREUS_LEASE_PERIOD");
}

impl slots::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Registrar = Registrar;
    type LeasePeriod = LeasePeriod;
    type LeaseOffset = ();
    type ForceOrigin = EnsureRoot<Self::AccountId>;
    type WeightInfo = weights::runtime_common_slots::WeightInfo<Runtime>;
}

impl paras_sudo_wrapper::Config for Runtime {}

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

        // Governance-related pallets
        Scheduler: pallet_scheduler = 45,
        Preimage: pallet_preimage = 46,
        Council: pallet_collective::<Instance1> = 47,
        TechnicalCommittee: pallet_collective::<Instance2> = 48,
        TechnicalMembership: pallet_membership::<Instance1> = 49,
        Treasury: pallet_treasury = 50,
        TreasuryExtension: pallet_treasury_extension::{Pallet, Event<T>} = 51,
        Bounties: pallet_bounties = 52,
        Democracy: pallet_democracy = 53,

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

        // Parachain Onboarding Pallets. Start indices at 80 to leave room.
        Registrar: paras_registrar::{Pallet, Call, Storage, Event<T>} = 80,
        Slots: slots::{Pallet, Call, Storage, Event<T>} = 81,
        ParasSudoWrapper: paras_sudo_wrapper::{Pallet, Call} = 82,

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
    }
);

#[derive(Clone)]
pub struct TransactionConverter;

impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
    fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        )
    }
}

impl fp_rpc::ConvertTransaction<opaque::UncheckedExtrinsic> for TransactionConverter {
    fn convert_transaction(
        &self,
        transaction: pallet_ethereum::Transaction,
    ) -> opaque::UncheckedExtrinsic {
        let extrinsic = UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        );
        let encoded = extrinsic.encode();
        opaque::UncheckedExtrinsic::decode(&mut &encoded[..])
            .expect("Encoded extrinsic is always valid")
    }
}

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
pub type Migrations = (
    migrations::V0101,
    migrations::V0103,
    migrations::V0104,
    migrations::V0108,
    migrations::V0112,
    migrations::Unreleased,
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

fn transact_with_new_gas_limit(
    transact_call: pallet_ethereum::Call<Runtime>,
) -> pallet_ethereum::Call<Runtime> {
    match transact_call {
        transact { transaction } => {
            let transaction = match transaction {
                EthereumTransaction::Legacy(tx) => EthereumTransaction::Legacy(LegacyTransaction {
                    gas_limit: GetConstantGasLimit::get(),
                    ..tx
                }),
                EthereumTransaction::EIP1559(tx) => {
                    EthereumTransaction::EIP1559(EIP1559Transaction {
                        gas_limit: GetConstantGasLimit::get(),
                        ..tx
                    })
                },
                EthereumTransaction::EIP2930(tx) => {
                    EthereumTransaction::EIP2930(EIP2930Transaction {
                        gas_limit: GetConstantGasLimit::get(),
                        ..tx
                    })
                },
            };
            pallet_ethereum::Call::new_call_variant_transact(transaction)
        },
        _ => transact_call,
    }
}

// user doesn't have NAC to dispatch transaction
const ACCESS_RESTRICTED: u8 = u8::MAX;

impl fp_self_contained::SelfContainedCall for RuntimeCall {
    type SignedInfo = H160;

    fn is_self_contained(&self) -> bool {
        match self {
            RuntimeCall::Ethereum(call) => call.is_self_contained(),
            _ => false,
        }
    }

    fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => call.check_self_contained(),
            _ => None,
        }
    }

    // TODO: get rid of cloning the call
    fn validate_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<TransactionValidity> {
        match self {
            RuntimeCall::Ethereum(call) => {
                let account_id =
                    <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(*info);

                if let CallFee::EVM(amount) =
                    EnergyFee::dispatch_info_to_fee(self, Some(dispatch_info), None)
                {
                    let (_, fee_vtrs_amount) =
                        if let Ok(parts) = EnergyFee::calculate_fee_parts(&account_id, amount) {
                            parts
                        } else {
                            return Some(Err(InvalidTransaction::Payment.into()));
                        };

                    let vtrs_balance = Balances::reducible_balance(
                        &account_id,
                        Preservation::Protect,
                        Fortitude::Polite,
                    );

                    if fee_vtrs_amount > vtrs_balance {
                        return Some(Err(InvalidTransaction::Payment.into()));
                    }
                }

                if !NacManaging::user_has_access(account_id, helpers::runner::CALL_ACCESS_LEVEL) {
                    return Some(Err(InvalidTransaction::Custom(ACCESS_RESTRICTED).into()));
                };

                transact_with_new_gas_limit(call.clone()).validate_self_contained(
                    info,
                    dispatch_info,
                    len,
                )
            },
            _ => None,
        }
    }

    // TODO: get rid of cloning the call
    fn pre_dispatch_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<Result<(), TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => transact_with_new_gas_limit(call.clone())
                .pre_dispatch_self_contained(info, dispatch_info, len),
            _ => None,
        }
    }

    fn apply_self_contained(
        self,
        info: Self::SignedInfo,
    ) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
        match self {
            call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) => {
                Some(call.dispatch(RuntimeOrigin::from(
                    pallet_ethereum::RawOrigin::EthereumTransaction(info),
                )))
            },
            _ => None,
        }
    }
}

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    define_benchmarks!([pallet_evm, EVM]);
}

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
        fn account_nonce(account: AccountId) -> Index {
            System::account_nonce(account)
        }
    }

    impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
        fn chain_id() -> u64 {
            <Runtime as pallet_evm::Config>::ChainId::get()
        }

        fn account_basic(address: H160) -> EVMAccount {
            let (account, _) = pallet_evm::Pallet::<Runtime>::account_basic(&address);
            account
        }

        fn gas_price() -> U256 {
            let (gas_price, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
            gas_price
        }

        fn account_code_at(address: H160) -> Vec<u8> {
            pallet_evm::AccountCodes::<Runtime>::get(address)
        }

        fn author() -> H160 {
            <pallet_evm::Pallet<Runtime>>::find_author()
        }

        fn storage_at(address: H160, index: U256) -> H256 {
            let mut tmp = [0u8; 32];
            index.to_big_endian(&mut tmp);
            pallet_evm::AccountStorages::<Runtime>::get(address, H256::from_slice(&tmp[..]))
        }

        fn call(
            from: H160,
            to: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };

            let is_transactional = false;
            let validate = true;
            let evm_config = config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config());

            let mut estimated_transaction_len = data.len() +
                20 + // to
                20 + // from
                32 + // value
                32 + // gas_limit
                32 + // nonce
                1 + // TransactionAction
                8 + // chain id
                65; // signature

            if max_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if max_priority_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if access_list.is_some() {
                estimated_transaction_len += access_list.encoded_size();
            }

            let gas_limit = gas_limit.min(u64::MAX.into()).low_u64();
            let without_base_extrinsic_weight = true;

            let (weight_limit, proof_size_base_cost) =
                match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    gas_limit,
                    without_base_extrinsic_weight
                ) {
                    weight_limit if weight_limit.proof_size() > 0 => {
                        (Some(weight_limit), Some(estimated_transaction_len as u64))
                    }
                    _ => (None, None),
                };

            <Runtime as pallet_evm::Config>::Runner::call(
                from,
                to,
                data,
                value,
                gas_limit.unique_saturated_into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                is_transactional,
                validate,
                weight_limit,
                proof_size_base_cost,
                evm_config,
            ).map_err(|err| err.error.into())
        }

        fn create(
            from: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };

            let is_transactional = false;
            let validate = true;
            let evm_config = config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config());

            let mut estimated_transaction_len = data.len() +
                20 + // from
                32 + // value
                32 + // gas_limit
                32 + // nonce
                1 + // TransactionAction
                8 + // chain id
                65; // signature

            if max_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if max_priority_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if access_list.is_some() {
                estimated_transaction_len += access_list.encoded_size();
            }

            let gas_limit = if gas_limit > U256::from(u64::MAX) {
                u64::MAX
            } else {
                gas_limit.low_u64()
            };
            let without_base_extrinsic_weight = true;

            let (weight_limit, proof_size_base_cost) =
                match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    gas_limit,
                    without_base_extrinsic_weight
                ) {
                    weight_limit if weight_limit.proof_size() > 0 => {
                        (Some(weight_limit), Some(estimated_transaction_len as u64))
                    }
                    _ => (None, None),
                };

            <Runtime as pallet_evm::Config>::Runner::create(
                from,
                data,
                value,
                gas_limit.unique_saturated_into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                is_transactional,
                validate,
                weight_limit,
                proof_size_base_cost,
                evm_config,
            ).map_err(|err| err.error.into())
        }

        fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
            pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
        }

        fn current_block() -> Option<pallet_ethereum::Block> {
            pallet_ethereum::CurrentBlock::<Runtime>::get()
        }

        fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
            pallet_ethereum::CurrentReceipts::<Runtime>::get()
        }

        fn current_all() -> (
            Option<pallet_ethereum::Block>,
            Option<Vec<pallet_ethereum::Receipt>>,
            Option<Vec<TransactionStatus>>
        ) {
            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentReceipts::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
            )
        }

        fn extrinsic_filter(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> Vec<EthereumTransaction> {
            xts.into_iter().filter_map(|xt| match xt.0.function {
                RuntimeCall::Ethereum(transact { transaction }) => Some(transaction),
                _ => None
            }).collect::<Vec<EthereumTransaction>>()
        }

        fn elasticity() -> Option<Permill> {
            Some(DefaultElasticity::get())
        }

        fn gas_limit_multiplier_support() {}

        fn pending_block(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> (Option<pallet_ethereum::Block>, Option<Vec<TransactionStatus>>) {
            for ext in xts.into_iter() {
                let _ = Executive::apply_extrinsic(ext);
            }

            Ethereum::on_finalize(System::block_number() + 1);

            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
            )
        }
    }

    impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
        fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
            UncheckedExtrinsic::new_unsigned(
                pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
            )
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
        Block,
        Balance,
    > for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            let fee = EnergyFee::dispatch_info_to_fee(uxt.call(), None, None);
            let mut runtime_dispatch_info = TransactionPayment::query_info(uxt, len);

            runtime_dispatch_info.partial_fee = fee.into_inner();
            runtime_dispatch_info
        }

        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> FeeDetails<Balance> {
            let fee = EnergyFee::dispatch_info_to_fee(uxt.call(), None, None).into_inner();
            let fee_details = TransactionPayment::query_fee_details(uxt, len);

            match fee_details {
                FeeDetails {
                    inclusion_fee: Some(InclusionFee { base_fee, len_fee, .. }),
                    tip
                } => FeeDetails {
                    inclusion_fee: Some(InclusionFee{
                        base_fee,
                        len_fee,
                        adjusted_weight_fee: fee,
                    }),
                    tip
                },
                fee_details => fee_details
            }

        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_beefy_mmr::BeefyMmrApi<Block, Hash> for RuntimeApi {
        fn authority_set_proof() -> sp_consensus_beefy::mmr::BeefyAuthoritySet<Hash> {
            MmrLeaf::authority_set_proof()
        }

        fn next_authority_set_proof() -> sp_consensus_beefy::mmr::BeefyNextAuthoritySet<Hash> {
            MmrLeaf::next_authority_set_proof()
        }
    }

    impl pallet_nfts_runtime_api::NftsApi<Block, AccountId, u32, u32> for Runtime {
        fn owner(collection: u32, item: u32) -> Option<AccountId> {
            <Nfts as Inspect<AccountId>>::owner(&collection, &item)
        }

        fn collection_owner(collection: u32) -> Option<AccountId> {
            <Nfts as Inspect<AccountId>>::collection_owner(&collection)
        }

        fn attribute(
            collection: u32,
            item: u32,
            key: Vec<u8>,
        ) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::attribute(&collection, &item, &key)
        }

        fn custom_attribute(
            account: AccountId,
            collection: u32,
            item: u32,
            key: Vec<u8>,
        ) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::custom_attribute(
                &account,
                &collection,
                &item,
                &key,
            )
        }

        fn system_attribute(
            collection: u32,
            item: u32,
            key: Vec<u8>,
        ) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::system_attribute(&collection, &item, &key)
        }

        fn collection_attribute(collection: u32, key: Vec<u8>) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::collection_attribute(&collection, &key)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(),
                epoch_length: EpochDuration::get(),
                c: epoch_config.c,
                authorities: Babe::authorities().to_vec(),
                randomness: Babe::randomness(),
                allowed_slots: epoch_config.allowed_slots,
            }
        }

        fn current_epoch_start() -> sp_consensus_babe::Slot {
            Babe::current_epoch_start()
        }

        fn current_epoch() -> sp_consensus_babe::Epoch {
            Babe::current_epoch()
        }

        fn next_epoch() -> sp_consensus_babe::Epoch {
            Babe::next_epoch()
        }

        fn generate_key_ownership_proof(
            _slot: sp_consensus_babe::Slot,
            authority_id: sp_consensus_babe::AuthorityId,
        ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {

            Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
            key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Babe::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> fg_primitives::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: fg_primitives::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Grandpa::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: fg_primitives::SetId,
            authority_id: GrandpaId,
        ) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
            use parity_scale_codec::Encode;

            Historical::prove((fg_primitives::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(fg_primitives::OpaqueKeyOwnershipProof::new)
        }
    }

    impl energy_fee_runtime_api::EnergyFeeApi<Block> for Runtime {
        fn estimate_gas(request: CallRequest) -> U256 {
            let CallRequest {
                from,
                to,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                gas,
                value,
                data,
                nonce,
                access_list,
                ..
            } = request;
            let call = match data {
                Some(data) => {
                    let from = from.unwrap_or_default();
                    let to = to.unwrap_or_default();
                    let value = value.unwrap_or_else(U256::zero);
                    let gas_limit = gas.unwrap_or_else(|| U256::from(21000)).low_u64(); // default gas limit to 21000
                    let max_fee_per_gas = max_fee_per_gas.unwrap_or_else(U256::zero);
                    let access_list = access_list.unwrap_or_default();
                    let access_list_converted = access_list.into_iter()
                        .map(|item| (item.address, item.storage_keys))
                        .collect();

                    RuntimeCall::EVM(pallet_evm::Call::call {
                        source: from,
                        target: to,
                        input: data.into_inner(),
                        value,
                        gas_limit,
                        max_fee_per_gas,
                        max_priority_fee_per_gas,
                        nonce,
                        access_list: access_list_converted,
                    })
                },
                None => {
                    match (from, to, value) {
                        (_, Some(to), Some(value)) => {
                            let value_converted = Balance::from(value.low_u128());  // Adjust this conversion as necessary

                            RuntimeCall::Balances(pallet_balances::Call::transfer {
                                dest: to.into(),
                                value: value_converted,
                            })
                        },
                        _ => return GetConstantEnergyFee::get().into(),
                    }
                }
            };

            EnergyFee::dispatch_info_to_fee(&call, None, None).into_inner().into()
        }

        fn vtrs_to_vnrg_swap_rate() -> Option<u128> {
            EnergyBroker::quote_price_exact_tokens_for_tokens(
                NativeOrAssetId::Native,
                NativeOrAssetId::Asset(VNRG::get()),
                UNITS,
                true
            )
        }
    }

    impl pallet_energy_broker::AssetConversionApi<
        Block,
        Balance,
        Balance,
        NativeOrAssetId<AssetId>
    > for Runtime {
        fn quote_price_tokens_for_exact_tokens(
            asset1: NativeOrAssetId<AssetId>,
            asset2: NativeOrAssetId<AssetId>,
            amount: Balance,
            include_fee: bool,
        ) -> Option<Balance> {
            EnergyBroker::quote_price_tokens_for_exact_tokens(asset1, asset2, amount, include_fee)
        }

        fn quote_price_exact_tokens_for_tokens(
            asset1: NativeOrAssetId<AssetId>,
            asset2: NativeOrAssetId<AssetId>,
            amount: Balance,
            include_fee: bool,
        ) -> Option<Balance> {
            EnergyBroker::quote_price_exact_tokens_for_tokens(asset1, asset2, amount, include_fee)
        }

        fn get_reserves(
            asset1: NativeOrAssetId<AssetId>,
            asset2: NativeOrAssetId<AssetId>,
        ) -> Option<(Balance, Balance)> {
            EnergyBroker::get_reserves(&asset1, &asset2).ok()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use pallet_treasury_extension::Pallet as PalletTreasuryExtension;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);
            list_benchmark!(list, extra, pallet_treasury_extension, PalletTreasuryExtension::<Runtime>);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};
            use pallet_treasury_extension::Pallet as PalletTreasuryExtension;
            impl frame_system_benchmarking::Config for Runtime {}

            let whitelist: Vec<TrackedStorageKey> = vec![];

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);

            add_benchmark!(params, batches, pallet_treasury_extension, PalletTreasuryExtension::<Runtime>);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }

    impl vitreus_utility_runtime_api::UtilityApi<Block> for Runtime {
        fn balance(who: H160) -> U256 {
            let account_id = <Self as pallet_evm::Config>::AddressMapping::into_account_id(who);
            Balances::reducible_balance(&account_id, Preservation::Preserve, Fortitude::Polite).into()
        }
    }

    impl energy_generation_runtime_api::EnergyGenerationApi<Block> for Runtime {
        fn reputation_tier_additional_reward(tier: ReputationTier) -> Perbill {
            ReputationTierEnergyRewardAdditionalPercentMapping::get(&tier)
        }

        fn current_energy_per_stake_currency() -> u128 {
            EnergyGeneration::active_era()
                .and_then(|era| EnergyGeneration::eras_energy_per_stake_cur(era.index))
                .unwrap_or(0)
        }
    }

    impl runtime_api::ParachainHost<Block, Hash, BlockNumber> for Runtime {
        fn validators() -> Vec<ValidatorId> {
            parachains_runtime_api_impl::validators::<Runtime>()
        }

        fn validator_groups() -> (Vec<Vec<ValidatorIndex>>, GroupRotationInfo<BlockNumber>) {
            parachains_runtime_api_impl::validator_groups::<Runtime>()
        }

        fn availability_cores() -> Vec<CoreState<Hash, BlockNumber>> {
            parachains_runtime_api_impl::availability_cores::<Runtime>()
        }

        fn persisted_validation_data(para_id: ParaId, assumption: OccupiedCoreAssumption)
            -> Option<PersistedValidationData<Hash, BlockNumber>> {
            parachains_runtime_api_impl::persisted_validation_data::<Runtime>(para_id, assumption)
        }

        fn assumed_validation_data(
            para_id: ParaId,
            expected_persisted_validation_data_hash: Hash,
        ) -> Option<(PersistedValidationData<Hash, BlockNumber>, ValidationCodeHash)> {
            parachains_runtime_api_impl::assumed_validation_data::<Runtime>(
                para_id,
                expected_persisted_validation_data_hash,
            )
        }

        fn check_validation_outputs(
            para_id: ParaId,
            outputs: CandidateCommitments,
        ) -> bool {
            parachains_runtime_api_impl::check_validation_outputs::<Runtime>(para_id, outputs)
        }

        fn session_index_for_child() -> SessionIndex {
            parachains_runtime_api_impl::session_index_for_child::<Runtime>()
        }

        fn validation_code(para_id: ParaId, assumption: OccupiedCoreAssumption)
            -> Option<ValidationCode> {
            parachains_runtime_api_impl::validation_code::<Runtime>(para_id, assumption)
        }

        fn candidate_pending_availability(para_id: ParaId) -> Option<CommittedCandidateReceipt<Hash>> {
            parachains_runtime_api_impl::candidate_pending_availability::<Runtime>(para_id)
        }

        fn candidate_events() -> Vec<CandidateEvent<Hash>> {
            parachains_runtime_api_impl::candidate_events::<Runtime, _>(|ev| {
                match ev {
                    RuntimeEvent::ParaInclusion(ev) => {
                        Some(ev)
                    }
                    _ => None,
                }
            })
        }

        fn session_info(index: SessionIndex) -> Option<SessionInfo> {
            parachains_runtime_api_impl::session_info::<Runtime>(index)
        }

        fn session_executor_params(session_index: SessionIndex) -> Option<ExecutorParams> {
            parachains_runtime_api_impl::session_executor_params::<Runtime>(session_index)
        }

        fn dmq_contents(recipient: ParaId) -> Vec<InboundDownwardMessage<BlockNumber>> {
            parachains_runtime_api_impl::dmq_contents::<Runtime>(recipient)
        }

        fn inbound_hrmp_channels_contents(
            recipient: ParaId
        ) -> BTreeMap<ParaId, Vec<InboundHrmpMessage<BlockNumber>>> {
            parachains_runtime_api_impl::inbound_hrmp_channels_contents::<Runtime>(recipient)
        }

        fn validation_code_by_hash(hash: ValidationCodeHash) -> Option<ValidationCode> {
            parachains_runtime_api_impl::validation_code_by_hash::<Runtime>(hash)
        }

        fn on_chain_votes() -> Option<ScrapedOnChainVotes<Hash>> {
            parachains_runtime_api_impl::on_chain_votes::<Runtime>()
        }

        fn submit_pvf_check_statement(
            stmt: PvfCheckStatement,
            signature: ValidatorSignature,
        ) {
            parachains_runtime_api_impl::submit_pvf_check_statement::<Runtime>(stmt, signature)
        }

        fn pvfs_require_precheck() -> Vec<ValidationCodeHash> {
            parachains_runtime_api_impl::pvfs_require_precheck::<Runtime>()
        }

        fn validation_code_hash(para_id: ParaId, assumption: OccupiedCoreAssumption)
            -> Option<ValidationCodeHash>
        {
            parachains_runtime_api_impl::validation_code_hash::<Runtime>(para_id, assumption)
        }

        fn disputes() -> Vec<(SessionIndex, CandidateHash, DisputeState<BlockNumber>)> {
            parachains_runtime_api_impl::get_session_disputes::<Runtime>()
        }

        fn unapplied_slashes(
        ) -> Vec<(SessionIndex, CandidateHash, slashing::PendingSlashes)> {
            parachains_runtime_api_impl::unapplied_slashes::<Runtime>()
        }

        fn key_ownership_proof(
            validator_id: ValidatorId,
        ) -> Option<slashing::OpaqueKeyOwnershipProof> {
            use parity_scale_codec::Encode;

            Historical::prove((PARACHAIN_KEY_TYPE_ID, validator_id))
                .map(|p| p.encode())
                .map(slashing::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_dispute_lost(
            dispute_proof: slashing::DisputeProof,
            key_ownership_proof: slashing::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            parachains_runtime_api_impl::submit_unsigned_slashing_report::<Runtime>(
                dispute_proof,
                key_ownership_proof,
            )
        }
    }

    impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
        fn authorities() -> Vec<sp_authority_discovery::AuthorityId> {
            parachains_runtime_api_impl::relevant_authority_ids::<Runtime>()
        }
    }

    impl sp_consensus_beefy::BeefyApi<Block> for Runtime {
        fn beefy_genesis() -> Option<BlockNumber> {
             Beefy::genesis_block()
        }

        fn validator_set() -> Option<sp_consensus_beefy::ValidatorSet<sp_consensus_beefy::crypto::AuthorityId>> {
            Beefy::validator_set()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_beefy::EquivocationProof<
                BlockNumber,
                sp_consensus_beefy::crypto::AuthorityId,
                sp_consensus_beefy::crypto::Signature,
            >,
            key_owner_proof: sp_consensus_beefy::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Beefy::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_beefy::ValidatorSetId,
            authority_id: sp_consensus_beefy::crypto::AuthorityId,
        ) -> Option<sp_consensus_beefy::OpaqueKeyOwnershipProof> {
             use parity_scale_codec::Encode;

            Historical::prove((sp_consensus_beefy::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_beefy::OpaqueKeyOwnershipProof::new)
        }
    }

    impl sp_mmr_primitives::MmrApi<Block, Hash, BlockNumber> for Runtime {
        fn mmr_root() -> Result<Hash, sp_mmr_primitives::Error> {
            Ok(Mmr::mmr_root())
        }

        fn mmr_leaf_count() -> Result<sp_mmr_primitives::LeafIndex, sp_mmr_primitives::Error> {
            Ok(Mmr::mmr_leaves())
        }

        fn generate_proof(
            block_numbers: Vec<BlockNumber>,
            best_known_block_number: Option<BlockNumber>,
        ) -> Result<(Vec<sp_mmr_primitives::EncodableOpaqueLeaf>, sp_mmr_primitives::Proof<Hash>), sp_mmr_primitives::Error> {
             Mmr::generate_proof(block_numbers, best_known_block_number).map(
                |(leaves, proof)| {
                    (
                        leaves
                            .into_iter()
                            .map(|leaf| mmr::EncodableOpaqueLeaf::from_leaf(&leaf))
                            .collect(),
                        proof,
                    )
                },
            )
        }

        fn verify_proof(leaves: Vec<sp_mmr_primitives::EncodableOpaqueLeaf>, proof: sp_mmr_primitives::Proof<Hash>)
            -> Result<(), sp_mmr_primitives::Error>
        {
             let leaves = leaves.into_iter().map(|leaf|
                leaf.into_opaque_leaf()
                .try_decode()
                .ok_or(mmr::Error::Verify)).collect::<Result<Vec<mmr::Leaf>, mmr::Error>>()?;
            Mmr::verify_leaves(leaves, proof)
        }

        fn verify_proof_stateless(
            root: Hash,
            leaves: Vec<sp_mmr_primitives::EncodableOpaqueLeaf>,
            proof: sp_mmr_primitives::Proof<Hash>
        ) -> Result<(), sp_mmr_primitives::Error> {
            let nodes = leaves.into_iter().map(|leaf|mmr::DataOrHash::Data(leaf.into_opaque_leaf())).collect();
            pallet_mmr::verify_leaves_proof::<mmr::Hashing, _>(root, nodes, proof)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            log::info!("try-runtime::on_runtime_upgrade");
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, BlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect,
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
        }
    }
}
