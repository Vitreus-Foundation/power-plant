#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
#![allow(clippy::new_without_default, clippy::or_fun_call)]
#![cfg_attr(feature = "runtime-benchmarks", deny(unused_crate_dependencies))]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use parity_scale_codec::{Compact, Decode, Encode};
use sp_api::impl_runtime_apis;
use sp_core::{
    crypto::{ByteArray, KeyTypeId},
    OpaqueMetadata, H160, H256, U256,
};
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
    ApplyExtrinsicResult, ConsensusEngineId, Perbill, Permill,
};
use sp_staking::{EraIndex, SessionIndex};
use sp_std::{marker::PhantomData, prelude::*};
use sp_version::RuntimeVersion;
// Substrate FRAME
#[cfg(feature = "with-paritydb-weights")]
use frame_support::weights::constants::ParityDbWeight as RuntimeDbWeight;
#[cfg(feature = "with-rocksdb-weights")]
use frame_support::weights::constants::RocksDbWeight as RuntimeDbWeight;
use frame_support::{
    construct_runtime,
    dispatch::GetDispatchInfo,
    parameter_types,
    traits::{
        fungible::ItemOf, AsEnsureOriginWithArg, ConstU32, ConstU64, ConstU8, ExtrinsicCall,
        FindAuthor, Hooks, KeyOwnerProofSystem,
    },
    weights::{constants::WEIGHT_REF_TIME_PER_MILLIS, ConstantMultiplier, IdentityFee, Weight},
};
use frame_system::{EnsureRoot, EnsureSigned};
use pallet_energy_fee::{
    traits::{AssetsBalancesConverter, NativeExchange},
    CallFee, CustomFee,
};
use pallet_grandpa::{
    fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};
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
use sp_runtime::transaction_validity::InvalidTransaction;

pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;

pub use pallet_energy_generation::StakerStatus;
pub use pallet_nac_managing;
pub use pallet_reputation::ReputationPoint;

// A few exports that help ease life for downstream crates.
pub use frame_system::Call as SystemCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;

mod precompiles;
mod helpers {
    pub mod runner;
}

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
pub type AssetId = u128;

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
            pub babe: Babe,
            pub grandpa: Grandpa,
            pub im_online: ImOnline,
        }
    }
}

/// The BABE epoch configuration at genesis.
pub const BABE_GENESIS_EPOCH_CONFIG: sp_consensus_babe::BabeEpochConfiguration =
    sp_consensus_babe::BabeEpochConfiguration {
        c: PRIMARY_PROBABILITY,
        allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
    };

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("vitreus-power-plant"),
    impl_name: create_runtime_str!("vitreus-power-plant"),
    authoring_version: 1,
    spec_version: 1,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 1,
};

// Time measurmement primitive
pub type Moment = u64;

pub const MILLISECS_PER_BLOCK: Moment = 3000;
pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;

pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;

// 1 in 4 blocks (on average, not counting collisions) will be primary BABE blocks.
pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);

// NOTE: Currently it is not possible to change the epoch duration after the chain has started.
//       Attempting to do so will brick block production.
pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = 10 * MINUTES;
pub const EPOCH_DURATION_IN_SLOTS: u64 = {
    const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

    (EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
};

// Time is measured by number of blocks.
// 60_000 ms per minute / ms per block
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

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
}

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
    type OnNewAccount = Reputation;
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
    pub const MaxAuthorities: u32 = 100;
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
    type MaxSetIdSessionEntries = ConstU64<0>;

    type KeyOwnerProof = sp_core::Void;
    type EquivocationReportSystem = ();
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
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
    type MaxHolds = ();
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
    #[cfg(feature = "runtime_benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const AccumulationPeriod: BlockNumber = HOURS * 24;
    pub const MaxAmount: Balance = 100 * vtrs::UNITS;
}

impl pallet_faucet::Config for Runtime {
    type AccumulationPeriod = AccumulationPeriod;
    type MaxAmount = MaxAmount;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

use pallet_reputation::REPUTATION_POINTS_PER_DAY;

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
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Runtime, EnergyGeneration>;
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

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
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

// it takes a month to become a validator from 0
pub const VALIDATOR_REPUTATION_THRESHOLD: ReputationPoint =
    ReputationPoint::new(REPUTATION_POINTS_PER_DAY.0 * 30);
// it takes 2 months to become a collaborative validator from 0
pub const COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD: ReputationPoint =
    ReputationPoint::new(REPUTATION_POINTS_PER_DAY.0 * 60);

parameter_types! {
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &I_NPOS;
    pub const SessionsPerEra: SessionIndex = 5;
    pub const BondingDuration: EraIndex = 24 * 28;
    pub const SlashDeferDuration: EraIndex = 24 * 7; // 1/4 the bonding duration.
    pub const Period: BlockNumber = 5;
    pub const Offset: BlockNumber = 0;
    pub const VNRG: AssetId = 1;
    pub const BatterySlotCapacity: Energy = 100_000_000_000;
    pub const MaxCooperations: u32 = 1024;
    pub const HistoryDepth: u32 = 84;
    pub const MaxUnlockingChunks: u32 = 32;
    pub const RewardOnUnbalanceWasCalled: bool = false;
    pub const MaxWinners: u32 = 100;
    // it takes a month to become a validator from 0
    pub const ValidatorReputationThreshold: ReputationPoint = VALIDATOR_REPUTATION_THRESHOLD;
    // it takes 2 months to become a collaborative validator from 0
    pub const CollaborativeValidatorReputationThreshold: ReputationPoint = COLLABORATIVE_VALIDATOR_REPUTATION_THRESHOLD;
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
        1_000_000
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

pub struct EnergyGenerationBenchmarkConfig;
impl pallet_energy_generation::BenchmarkingConfig for EnergyGenerationBenchmarkConfig {
    type MaxValidators = ConstU32<1000>;
    type MaxCooperators = ConstU32<1000>;
}

impl pallet_energy_generation::Config for Runtime {
    type AdminOrigin = EnsureRoot<AccountId>;
    type BatterySlotCapacity = BatterySlotCapacity;
    type BenchmarkingConfig = EnergyGenerationBenchmarkConfig;
    type BondingDuration = BondingDuration;
    type CollaborativeValidatorReputationThreshold = CollaborativeValidatorReputationThreshold;
    type EnergyAssetId = VNRG;
    type EnergyPerReputationPoint = EnergyPerReputationPoint;
    type EnergyPerStakeCurrency = EnergyPerStakeCurrency;
    type HistoryDepth = HistoryDepth;
    type MaxCooperations = MaxCooperations;
    type MaxCooperatorRewardedPerValidator = ConstU32<64>;
    type MaxUnlockingChunks = MaxUnlockingChunks;
    type NextNewSession = Session;
    type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
    type EventListeners = ();
    type Reward = ();
    type RewardRemainder = ();
    type RuntimeEvent = RuntimeEvent;
    type SessionInterface = ();
    type SessionsPerEra = SessionsPerEra;
    type Slash = ();
    type SlashDeferDuration = SlashDeferDuration;
    type StakeBalance = Balance;
    type StakeCurrency = Balances;
    type ThisWeightInfo = ();
    type UnixTime = Timestamp;
    type ValidatorReputationThreshold = ValidatorReputationThreshold;
}

parameter_types! {
    pub const CollectionDeposit: Balance = 100;
    pub const ItemDeposit: Balance = 1;
    pub const KeyLimit: u32 = 32;
    pub const ValueLimit: u32 = 256;
    pub const ApprovalsLimit: u32 = 20;
    pub const ItemAttributesApprovalsLimit: u32 = 20;
    pub const MaxTips: u32 = 10;
    pub const MaxDeadlineDuration: BlockNumber = 12 * 30 * DAYS;
}

type CollectionId = u32;
type ItemId = u32;

impl pallet_uniques::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureRoot<AccountId>;
    type CollectionDeposit = CollectionDeposit;
    type ItemDeposit = ItemDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type AttributeDepositBase = MetadataDepositBase;
    type DepositPerByte = MetadataDepositPerByte;
    type StringLimit = AssetsStringLimit;
    type KeyLimit = KeyLimit;
    type ValueLimit = ValueLimit;
    type WeightInfo = pallet_uniques::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type Locker = ();
}

impl pallet_nac_managing::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type ForceOrigin = frame_system::EnsureRoot<AccountId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type WeightInfo = pallet_nac_managing::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const TransactionByteFee: Balance = 1;
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = EnergyFee;
    type OperationalFeeMultiplier = ConstU8<5>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = ();
}

impl pallet_asset_rate::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CreateOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = EnsureRoot<AccountId>;
    type UpdateOrigin = EnsureRoot<AccountId>;
    type AssetId = AssetId;
    type Currency = Balances;
    type Balance = Balance;
    type WeightInfo = pallet_asset_rate::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type AssetKindFactory = ();
}

parameter_types! {
    pub const GetConstantEnergyFee: Balance = 1_000_000_000;
}

type EnergyItem = ItemOf<Assets, VNRG, AccountId>;
type EnergyRate = AssetsBalancesConverter<Runtime, AssetRate>;
type EnergyExchange = NativeExchange<AssetId, Balances, EnergyItem, EnergyRate, VNRG>;

impl pallet_energy_fee::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type FeeTokenBalanced = EnergyItem;
    type MainTokenBalanced = Balances;
    type EnergyExchange = EnergyExchange;
    type GetConstantFee = GetConstantEnergyFee;
    type CustomFee = EnergyFee;
    type EnergyAssetId = VNRG;
}

impl pallet_claiming::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AdminOrigin = EnsureRoot<AccountId>;
    type Currency = Balances;
    type WeightInfo = ();
}

// We implement CusomFee here since the RuntimeCall defined in construct_runtime! macro
impl CustomFee<RuntimeCall, DispatchInfoOf<RuntimeCall>, Balance, GetConstantEnergyFee>
    for EnergyFee
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        _dispatch_info: &DispatchInfoOf<RuntimeCall>,
    ) -> CallFee<Balance> {
        match runtime_call {
            RuntimeCall::Balances(..)
            | RuntimeCall::Assets(..)
            | RuntimeCall::Uniques(..)
            | RuntimeCall::Reputation(..)
            | RuntimeCall::EnergyGeneration(..) => CallFee::Custom(GetConstantEnergyFee::get()),
            RuntimeCall::EVM(..) | RuntimeCall::Ethereum(..) => {
                CallFee::EVM(GetConstantEnergyFee::get())
            },
            _ => CallFee::Stock,
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

impl pallet_evm::Config for Runtime {
    type AddressMapping = IdentityAddressMapping;
    type BlockGasLimit = BlockGasLimit;
    type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
    type CallOrigin = EnsureAccountId20;
    type ChainId = EVMChainId;
    type Currency = Balances;
    type Runner = helpers::runner::NacRunner<Self>;
    type RuntimeEvent = RuntimeEvent;
    type WeightPerGas = WeightPerGas;
    type WithdrawOrigin = EnsureAccountId20;
    type OnCreate = ();
    type Timestamp = Timestamp;
    type FeeCalculator = BaseFee;
    type FindAuthor = FindAuthorTruncated<Babe>;
    type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type OnChargeTransaction = EnergyFee; //EVMCurrencyAdapter<Balances, ()>;
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
    // as we have constant fee, we set it to 1
    pub BoundDivision: U256 = U256::from(1);
}

impl pallet_dynamic_fee::Config for Runtime {
    type MinGasPriceBoundDivisor = BoundDivision;
}

parameter_types! {
    // the minimum amount of gas that a transaction must pay to be included in a block
    pub DefaultBaseFeePerGas: U256 = U256::from(GetConstantEnergyFee::get());
    pub DefaultElasticity: Permill = Permill::from_parts(1_000_000);
}

pub struct BaseFeeThreshold;
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
    fn lower() -> Permill {
        Permill::from_parts(1_000_000)
    }
    fn ideal() -> Permill {
        Permill::from_parts(1_000_000)
    }
    fn upper() -> Permill {
        Permill::from_parts(1_000_000)
    }
}

impl pallet_base_fee::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Threshold = BaseFeeThreshold;
    type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
    type DefaultElasticity = DefaultElasticity;
}

impl pallet_hotfix_sufficients::Config for Runtime {
    type AddressMapping = IdentityAddressMapping;
    type WeightInfo = pallet_hotfix_sufficients::weights::SubstrateWeight<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Babe: pallet_babe,
        Grandpa: pallet_grandpa,
        Balances: pallet_balances,
        Faucet: pallet_faucet,
        Assets: pallet_assets,
        AssetRate: pallet_asset_rate,
        TransactionPayment: pallet_transaction_payment,
        Sudo: pallet_sudo,
        BaseFee: pallet_base_fee,
        DynamicFee: pallet_dynamic_fee,
        EVM: pallet_evm,
        EVMChainId: pallet_evm_chain_id,
        Ethereum: pallet_ethereum,
        HotfixSufficients: pallet_hotfix_sufficients,
        Uniques: pallet_uniques,
        Reputation: pallet_reputation,
        Claiming: pallet_claiming,
        // Authorship must be before session in order to note author in the correct session and era
        // for im-online and staking.
        Authorship: pallet_authorship,
        ImOnline: pallet_im_online,
        EnergyGeneration: pallet_energy_generation,
        EnergyFee: pallet_energy_fee,
        Offences: pallet_offences,
        Session: pallet_session,
        Utility: pallet_utility,
        Historical: pallet_session::historical,
        NacManaging: pallet_nac_managing,
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
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic =
    fp_self_contained::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra, H160>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;

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

                if !NacManaging::user_has_access(account_id, helpers::runner::CALL_ACCESS_LEVEL) {
                    return Some(Err(InvalidTransaction::Custom(ACCESS_RESTRICTED).into()));
                };

                call.validate_self_contained(info, dispatch_info, len)
            },
            _ => None,
        }
    }

    fn pre_dispatch_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<Result<(), TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => {
                call.pre_dispatch_self_contained(info, dispatch_info, len)
            },
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
            Some(pallet_base_fee::Elasticity::<Runtime>::get())
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
            let dispatch_info = <<Block as BlockT>::Extrinsic as GetDispatchInfo>::get_dispatch_info(&uxt);
            let custom_fee = EnergyFee::dispatch_info_to_fee(uxt.call(), &dispatch_info);
            let mut runtime_dispatch_info = TransactionPayment::query_info(uxt, len);

            if let CallFee::Custom(custom_fee) | CallFee::EVM(custom_fee) = custom_fee {
                runtime_dispatch_info.partial_fee = custom_fee;
            }
            runtime_dispatch_info
        }

        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> FeeDetails<Balance> {
            let dispatch_info = <<Block as BlockT>::Extrinsic as GetDispatchInfo>::get_dispatch_info(&uxt);
            let custom_fee = EnergyFee::dispatch_info_to_fee(uxt.call(), &dispatch_info);

            let fee_details = TransactionPayment::query_fee_details(uxt, len);

            match (custom_fee, fee_details) {
                (
                    CallFee::Custom(custom_fee),
                    FeeDetails {
                        inclusion_fee: Some(_),
                        tip
                }) => FeeDetails {
                    inclusion_fee: Some(InclusionFee{
                        base_fee: custom_fee,
                        len_fee: 0,
                        adjusted_weight_fee: 0,
                    }),
                    tip
                },
                (_, fee_details) => fee_details
            }

        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
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
            _equivocation_proof: fg_primitives::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            _key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            None
        }

        fn generate_key_ownership_proof(
            _set_id: fg_primitives::SetId,
            _authority_id: GrandpaId,
        ) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
            // NOTE: this is the only implementation possible since we've
            // defined our key owner proof type as a bottom type (i.e. a type
            // with no values).
            None
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
            use pallet_hotfix_sufficients::Pallet as PalletHotfixSufficients;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);
            list_benchmark!(list, extra, pallet_hotfix_sufficients, PalletHotfixSufficients::<Runtime>);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};
            use pallet_evm::Pallet as PalletEvmBench;
            use pallet_hotfix_sufficients::Pallet as PalletHotfixSufficients;
            impl frame_system_benchmarking::Config for Runtime {}

            let whitelist: Vec<TrackedStorageKey> = vec![];

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);

            add_benchmark!(params, batches, pallet_evm, PalletEvmBench::<Runtime>);
            add_benchmark!(params, batches, pallet_hotfix_sufficients, PalletHotfixSufficients::<Runtime>);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Runtime, WeightPerGas};
    #[test]
    fn configured_base_extrinsic_weight_is_evm_compatible() {
        let min_ethereum_transaction_weight = WeightPerGas::get() * 21_000;
        let base_extrinsic = <Runtime as frame_system::Config>::BlockWeights::get()
            .get(frame_support::dispatch::DispatchClass::Normal)
            .base_extrinsic;
        assert!(base_extrinsic.ref_time() <= min_ethereum_transaction_weight.ref_time());
    }
}
