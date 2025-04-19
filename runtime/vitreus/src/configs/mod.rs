use fp_evm::weight_per_gas;
use frame_support::{
    derive_impl, ord_parameter_types, parameter_types,
    traits::{
        fungible::{HoldConsideration, ItemOf},
        tokens::{
            imbalance::{ResolveAssetTo, ResolveTo},
            pay::{PayAssetFromAccount, PayFromAccount},
            UnityAssetBalanceConversion, UnityOrOuterConversion,
        },
        AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, ConstU8, EitherOf, EitherOfDiverse,
        Equals, KeyOwnerProofSystem, LinearStoragePrice, WithdrawReasons,
    },
    weights::ConstantMultiplier,
    PalletId,
};
use frame_system::{EnsureRoot, EnsureSignedBy, EnsureWithSuccess};
use pallet_energy_generation::StashOf;
use pallet_ethereum::PostLogContent;
use pallet_evm::{EnsureAccountId20, IdentityAddressMapping};
use pallet_grandpa::AuthorityId as GrandpaId;
pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_nfts::PalletFeatures;
use pallet_reputation::ReputationTier;
use parity_scale_codec::Compact;
use sp_consensus_beefy::{ecdsa_crypto::AuthorityId as BeefyId, mmr::MmrLeafVersion};
use sp_core::{crypto::KeyTypeId, U256};
use sp_runtime::{
    curve::PiecewiseLinear,
    traits::{AccountIdConversion, ConvertInto, IdentityLookup, Keccak256, One, OpaqueKeys, Zero},
    transaction_validity::TransactionPriority,
    FixedI128, Permill,
};
use sp_staking::{EraIndex, SessionIndex};
use sp_std::vec;
use vitreus_runtime_common::NativeEnergyExchange;

#[cfg(feature = "with-paritydb-weights")]
use frame_support::weights::constants::ParityDbWeight as RuntimeDbWeight;
#[cfg(feature = "with-rocksdb-weights")]
use frame_support::weights::constants::RocksDbWeight as RuntimeDbWeight;

use super::*;

pub mod collectives;
pub mod parachains;
pub mod xcm_config;

pub use collectives::{CouncilCollective, TechnicalCollective};

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    items as Balance * 200 * NANO_VTRS + (bytes as Balance) * PICO_VTRS
}

parameter_types! {
    pub const LiquidityPalletId: PalletId = PalletId(*b"liquidty");
    pub const LiquidityReservesPalletId: PalletId = PalletId(*b"liqresrv");
    pub const StakingRewardsPalletId: PalletId = PalletId(*b"stknrwrd");
}

/// Asset ID.
#[cfg(not(feature = "runtime-benchmarks"))]
pub type AssetId = u128;

#[cfg(feature = "runtime-benchmarks")]
pub type AssetId = u32;

/// Energy of an account.
pub type Energy = Balance;

pub type NativeOrAssetId = frame_support::traits::fungible::NativeOrWithId<AssetId>;

pub type NativeAndAssets = frame_support::traits::fungible::UnionOf<
    Balances,
    Assets,
    frame_support::traits::fungible::NativeFromLeft,
    NativeOrAssetId,
    AccountId,
>;

/// Origin for council voting
type MoreThanHalfCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;

parameter_types! {
    pub const VNRG: AssetId = 0; // Energy
    pub const SNRG: AssetId = 1; // Static Energy
    pub const LNRG: AssetId = 2; // Liquid Energy
}

type EnergyAsset = ItemOf<Assets, VNRG, AccountId>;
type StaticEnergyAsset = ItemOf<Assets, SNRG, AccountId>;
type LiquidEnergyAsset = ItemOf<Assets, LNRG, AccountId>;

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
#[derive_impl(frame_system::config_preludes::RelayChainDefaultConfig)]
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
    /// The aggregated RuntimeTask type.
    type RuntimeTask = RuntimeTask;
    /// This stores the number of previous transactions associated with a sender account.
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
    /// The maximum number of consumers allowed on a single account.
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
    type MaxNominators = MaxCooperations;
    type KeyOwnerProof =
        <Historical as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxNominators = MaxCooperations;
    type MaxSetIdSessionEntries = ConstU64<168>;
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;
    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

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
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type MaxFreezes = MaxFreezes;
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
    type RemoveItemsLimit = ConstU32<500>;
    type AssetId = AssetId;
    type AssetIdParameter = Compact<AssetId>;
    type Currency = Balances;
    #[cfg(feature = "mainnet-runtime")]
    type CreateOrigin = frame_system::EnsureNever<AccountId>;
    #[cfg(feature = "testnet-runtime")]
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = AssetsStringLimit;
    type Freezer = AssetsFreezer;
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_assets_freezer::Config for Runtime {
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_reputation::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

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
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = StashOf<Runtime>;
    type ShouldEndSession = Babe;
    type NextSessionRotation = Babe;
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, EnergyGeneration>;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = opaque::SessionKeys;
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
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::MAX;
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
    type RuntimeEvent = RuntimeEvent;
    type ValidatorSet = Historical;
    type NextSessionRotation = Babe;
    type ReportUnresponsiveness = pallet_energy_generation::ChillOnOffence<Runtime, Offences>;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = pallet_im_online::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const BeefySetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

impl pallet_beefy::Config for Runtime {
    type BeefyId = BeefyId;
    type MaxAuthorities = MaxAuthorities;
    type MaxNominators = MaxCooperations;
    type MaxSetIdSessionEntries = BeefySetIdSessionEntries;
    type OnNewValidatorSet = MmrLeaf;
    type WeightInfo = ();
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, BeefyId)>>::Proof;
    type EquivocationReportSystem =
        pallet_beefy::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
    type AncestryHelper = MmrLeaf;
}

impl pallet_mmr::Config for Runtime {
    const INDEXING_PREFIX: &'static [u8] = mmr::INDEXING_PREFIX;
    type Hashing = Keccak256;
    type OnNewRoot = pallet_beefy_mmr::DepositBeefyDigest<Runtime>;
    type WeightInfo = ();
    type LeafData = pallet_beefy_mmr::Pallet<Runtime>;
    type BlockHashProvider = pallet_mmr::DefaultBlockHashProvider<Runtime>;
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

impl pallet_beefy_mmr::Config for Runtime {
    type LeafVersion = LeafVersion;
    type BeefyAuthorityToMerkleLeaf = pallet_beefy_mmr::BeefyEcdsaToEthereum;
    type LeafExtra = H256;
    type BeefyDataProvider = ParasProvider;
}

parameter_types! {
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &I_NPOS;
    pub const SessionsPerEra: SessionIndex = prod_or_fast!(4, 1);
    pub const BondingDuration: EraIndex = prod_or_fast!(42, 5);
    // TODO: consider removing, since the slash defer feature was removed
    pub const SlashDeferDuration: EraIndex = 0;
    pub const Period: BlockNumber = 5;
    pub const Offset: BlockNumber = 0;
    pub const BatterySlotCapacity: Energy = 100_000_000_000;
    pub const MaxCooperations: u32 = 256;
    pub const HistoryDepth: u32 = 84;
    pub const MaxUnlockingChunks: u32 = 64;
    pub const RewardOnUnbalanceWasCalled: bool = false;
    pub const MaxWinners: u32 = 100;
    // it takes a month to become a validator from 0
    pub const ValidatorReputationTier: ReputationTier = ReputationTier::Vanguard(1);
    // it takes a month to become a collaborative validator from 0
    pub const CollaborativeValidatorReputationTier: ReputationTier = ReputationTier::Vanguard(1);
    pub const RewardRemainderUnbalanced: u128 = 0;
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
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
    type EnergyAssetId = LNRG;
    type EraEnergyRateCalculator = DynamicEnergy;
    type HistoryDepth = HistoryDepth;
    type MaxCooperations = MaxCooperations;
    type MaxCooperatorRewardedPerValidator = ConstU32<128>;
    type MaxUnlockingChunks = MaxUnlockingChunks;
    type NextNewSession = Session;
    type EventListeners = ();
    type SessionChangeListeners = (EnergyBroker, DynamicEnergy, TreasuryExtension);
    type Reward = ();
    type RewardRemainder = Treasury;
    type RuntimeEvent = RuntimeEvent;
    type SessionInterface = Self;
    type SessionsPerEra = SessionsPerEra;
    type DisablingStrategy = pallet_energy_generation::UpToLimitDisablingStrategy;
    type Slash = Treasury;
    type SlashDeferDuration = SlashDeferDuration;
    type StakeBalance = Balance;
    type StakeCurrency = Balances;
    type ValidatorNacLevel = NacManaging;
    type ValidatorExposureMultiplier = ReputationExposureMultiplier;
    type CooperatorExposureMultiplier = ();
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

pub type CollectionId = u32;
pub type ItemId = u32;

impl pallet_nfts::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    #[cfg(feature = "mainnet-runtime")]
    type CreateOrigin = frame_system::EnsureNever<AccountId>;
    #[cfg(feature = "testnet-runtime")]
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
    type Locker = ();
    type CollectionDeposit = CollectionDeposit;
    type ItemDeposit = ItemDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type AttributeDepositBase = MetadataDepositBase;
    type DepositPerByte = MetadataDepositPerByte;
    type StringLimit = AssetsStringLimit;
    type KeyLimit = KeyLimit;
    type ValueLimit = ValueLimit;
    type ApprovalsLimit = ();
    type ItemAttributesApprovalsLimit = ();
    type MaxTips = ();
    type MaxDeadlineDuration = ();
    type MaxAttributesPerCall = ();
    type Features = ();
    type OffchainSignature = Signature;
    type OffchainPublic = <Signature as Verify>::Signer;
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type WeightInfo = pallet_nfts::weights::SubstrateWeight<Runtime>;
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

parameter_types! {
    pub const PrivilegesPalletId: PalletId = PalletId(*b"py/prvlg");
}

impl pallet_privileges::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type UnixTime = Timestamp;
    type PalletId = PrivilegesPalletId;
    type WeightInfo = pallet_privileges::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const TransactionByteFee: Balance = 1;
    pub const TransactionPicosecondFee: Balance = 8;
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = EnergyFee;
    type WeightToFee = ConstantMultiplier<Balance, TransactionPicosecondFee>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = EnergyFee;
    type OperationalFeeMultiplier = ConstU8<5>;
}

impl pallet_asset_rate::Config for Runtime {
    type WeightInfo = pallet_asset_rate::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type CreateOrigin = MoreThanHalfCouncil;
    type RemoveOrigin = MoreThanHalfCouncil;
    type UpdateOrigin = MoreThanHalfCouncil;
    type Currency = Balances;
    type AssetKind = AssetId;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const AssetConversionPalletId: PalletId = PalletId(*b"py/ascon");
    pub const SwapFee: u32 = 10; // 1%
    pub const NativeAsset: NativeOrAssetId = NativeOrAssetId::Native;
    pub const BurnedEnergySessionsCount: u32 = 84 * SessionsPerEra::get();
}

ord_parameter_types! {
    pub const AssetConversionOrigin: AccountId =
        AccountIdConversion::<AccountId>::into_account_truncating(&AssetConversionPalletId::get());
}

pub type PoolAssetsInstance = pallet_assets::Instance1;
impl pallet_assets::Config<PoolAssetsInstance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type RemoveItemsLimit = ConstU32<500>;
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
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = AssetsStringLimit;
    type Freezer = ();
    type Extra = ();
    type CallbackHandle = ();
    type WeightInfo = pallet_assets::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_energy_broker::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<AccountId>;
    type Balance = Balance;
    type HigherPrecisionBalance = sp_core::U256;
    type AssetKind = NativeOrAssetId;
    type Assets = NativeAndAssets;
    type AssetConverter = (
        NativeToEnergyConverter,
        LiquidEnergyToNativeConverter,
        StaticEnergyToNativeConverter,
        LiquidEnergyToEnergyConverter,
        StaticEnergyToEnergyConverter,
    );
    type FeelessAccounts = Equals<xcm_config::TreasuryAccount>;
    type SwapFeeTarget = ResolveAssetTo<pallet_treasury::TreasuryAccountId<Runtime>, Self::Assets>;
    type OnEnergySell = DynamicEnergy;
    type SwapFee = SwapFee;
    type NativeAsset = NativeAsset;
    type EnergyAsset = VNRG;
    type BurnedEnergySessionsCount = BurnedEnergySessionsCount;
}

parameter_types! {
    pub const ExpectedSessionDuration: u32 = EPOCH_DURATION_IN_BLOCKS * SECS_PER_BLOCK as u32;
    pub const AnnualPercentageRate: u32 = 100; // 10%
    pub MultiplierCoefficients: [FixedI128; 4] = [
        FixedI128::zero(), FixedI128::zero(), FixedI128::zero(), FixedI128::one(),
    ];
}

impl pallet_dynamic_energy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<AccountId>;
    type Balance = Balance;
    type HigherPrecisionBalance = sp_core::U256;
    type Staking = EnergyGeneration;
    type Warehouse = EnergyBroker;
    type UnixTime = Timestamp;
    type SessionsPerEra = SessionsPerEra;
    type ExpectedSessionDuration = ExpectedSessionDuration;
    type DefaultAnnualPercentageRate = AnnualPercentageRate;
    type DefaultMultiplierCoefficients = MultiplierCoefficients;
}

parameter_types! {
    pub const GetConstantEnergyFee: Balance = 1_000_000_000;
    pub GetConstantGasLimit: U256 = U256::from(100_000);
}

impl pallet_energy_fee::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = MoreThanHalfCouncil;
    type GetConstantFee = GetConstantEnergyFee;
    type CustomFee = EnergyFee;
    type EnergyAsset = EnergyAsset;
    type StaticEnergyAsset = StaticEnergyAsset;
    type LiquidEnergyAsset = LiquidEnergyAsset;
    type EnergyExchange = NativeEnergyExchange<EnergyBroker, NativeAsset, VNRG>;
    type OnWithdrawFee = NacManaging;
    type OnEnergyBurn = (EnergyBroker, DynamicEnergy);
    type FeeRecyclingRate = TreasuryExtension;
    type FeeRecyclingDestination =
        ResolveTo<pallet_treasury::TreasuryAccountId<Runtime>, Self::EnergyAsset>;
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
    type ClaimData = ();
    type OnClaim = NacManaging;
    type Prefix = Prefix;
    type WeightInfo = ();
}

impl pallet_claiming::Config<pallet_claiming::Instance1> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type VestingSchedule = Vesting;
    type ClaimData = KickstartClaimData;
    type OnClaim = KickstartClaimHandler;
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
    type BlockNumberProvider = System;
    type WeightInfo = pallet_vesting::weights::SubstrateWeight<Runtime>;
    type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
    const MAX_VESTING_SCHEDULES: u32 = 28;
}

impl pallet_simple_vesting::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = ConvertInto;
    type Slash = Treasury;
}

parameter_types! {
    // One storage item; key size 32, value size 8; .
    pub const ProxyDepositBase: Balance = deposit(1, 8);
    // Additional storage item size of 33 bytes.
    pub const ProxyDepositFactor: Balance = deposit(0, 33);
    pub const MaxProxies: u16 = 32;
    pub const AnnouncementDepositBase: Balance = deposit(1, 8);
    pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
    pub const MaxPending: u16 = 32;
}

impl pallet_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ProxyDepositBase;
    type ProxyDepositFactor = ProxyDepositFactor;
    type MaxProxies = MaxProxies;
    type WeightInfo = pallet_proxy::weights::SubstrateWeight<Runtime>;
    type MaxPending = MaxPending;
    type CallHasher = BlakeTwo256;
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = pallet_utility::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        BlockWeights::get().max_block;
    pub const MaxScheduledPerBlock: u32 = 50;
    pub const NoPreimagePostponement: Option<u32> = Some(10);
}

type ScheduleOrigin = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
>;

impl pallet_scheduler::Config for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeEvent = RuntimeEvent;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = ScheduleOrigin;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Runtime>;
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
    type Preimages = Preimage;
}

parameter_types! {
    pub const PreimageMaxSize: u32 = 4096 * 1024;
    pub const PreimageBaseDeposit: Balance = deposit(2, 64);
    pub const PreimageByteDeposit: Balance = deposit(0, 1);
    pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
    type WeightInfo = pallet_preimage::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Consideration = HoldConsideration<
        AccountId,
        Balances,
        PreimageHoldReason,
        LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
    >;
}

parameter_types! {
    // One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
    pub const DepositBase: Balance = deposit(1, 88);
    // Additional storage item size of 32 bytes.
    pub const DepositFactor: Balance = deposit(0, 32);
    pub const MaxSignatories: u32 = 100;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = pallet_multisig::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: Balance = 10 * MILLI_VTRS;
    pub const ProposalBondMaximum: Balance = 10 * UNITS;
    pub SpendPeriod: BlockNumber = prod_or_fast!(24 * DAYS, 40, "VITREUS_SPEND_PERIOD");
    pub const Burn: Permill = Permill::from_percent(0);
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const PayoutSpendPeriod: BlockNumber = 30 * DAYS;
    pub const DataDepositPerByte: Balance = 100 * PICO_VTRS;
    pub const MaxApprovals: u32 = 100;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
    pub const RootSpendOriginMaxAmount: Balance = Balance::MAX;
    pub const CouncilSpendOriginMaxAmount: Balance = 500_000 * UNITS;
}

impl pallet_treasury::Config for Runtime {
    type PalletId = TreasuryPalletId;
    type Currency = Balances;
    type RejectOrigin = MoreThanHalfCouncil;
    type RuntimeEvent = RuntimeEvent;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = ();
    type MaxApprovals = MaxApprovals;
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
    type SpendFunds = (Bounties, TreasuryExtension);
    type SpendOrigin = EitherOf<
        frame_system::EnsureRootWithSuccess<AccountId, RootSpendOriginMaxAmount>,
        EnsureWithSuccess<
            pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
            AccountId,
            CouncilSpendOriginMaxAmount,
        >,
    >;
    type AssetKind = ();
    type Beneficiary = AccountId;
    type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
    type Paymaster = PayFromAccount<Balances, TreasuryAccountId<Runtime>>;
    type BalanceConverter = UnityAssetBalanceConversion;
    type PayoutPeriod = PayoutSpendPeriod;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const TechnicalCommitteeTreasuryPalletId: PalletId = PalletId(*b"py/tctsr");
    pub const TechnicalCommitteeSpendOriginMaxAmount: Balance = 1_000_000 * UNITS;
}

pub type TechnicalCommitteeTreasury = pallet_treasury::Instance1;
impl pallet_treasury::Config<TechnicalCommitteeTreasury> for Runtime {
    type Currency = Balances;
    type RejectOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        pallet_collective::EnsureProportionMoreThan<AccountId, TechnicalCollective, 1, 2>,
    >;
    type RuntimeEvent = RuntimeEvent;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type PalletId = TechnicalCommitteeTreasuryPalletId;
    type BurnDestination = ();
    type WeightInfo = pallet_treasury::weights::SubstrateWeight<Runtime>;
    type SpendFunds = ();
    type MaxApprovals = MaxApprovals;
    type SpendOrigin = EitherOf<
        frame_system::EnsureRootWithSuccess<AccountId, RootSpendOriginMaxAmount>,
        EnsureWithSuccess<
            pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
            AccountId,
            TechnicalCommitteeSpendOriginMaxAmount,
        >,
    >;
    type AssetKind = NativeOrAssetId;
    type Beneficiary = AccountId;
    type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
    type Paymaster = PayAssetFromAccount<
        NativeAndAssets,
        TreasuryAccountId<Runtime, TechnicalCommitteeTreasury>,
    >;
    type BalanceConverter = UnityOrOuterConversion<
        Equals<NativeAsset>,
        pallet_dynamic_energy::DynamicEnergyConversion<Runtime, Equals<VNRG>>,
    >;
    type PayoutPeriod = PayoutSpendPeriod;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const SpendThreshold: Permill = Permill::from_percent(10);
    pub storage TreasuryTargetBalance: Balance = 100_000_000 * UNITS;
    pub storage FeeRecyclingBaseRate: Permill = Permill::from_percent(12);
    pub storage FeeRecyclingScalingFactor: Permill = Permill::from_percent(2);
}

impl pallet_treasury_extension::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnergyAsset = EnergyAsset;
    type StaticEnergyAsset = StaticEnergyAsset;
    type LiquidEnergyAsset = LiquidEnergyAsset;
    type StaticEnergyExchange = NativeEnergyExchange<EnergyBroker, NativeAsset, SNRG>;
    type LiquidEnergyExchange = NativeEnergyExchange<EnergyBroker, NativeAsset, LNRG>;
    type SpendThreshold = SpendThreshold;
    type OnRecycled = StakingRewardsSink;
    type TreasuryTargetBalance = TreasuryTargetBalance;
    type FeeRecyclingBaseRate = FeeRecyclingBaseRate;
    type FeeRecyclingScalingFactor = FeeRecyclingScalingFactor;
    type WeightInfo = pallet_treasury_extension::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const BountyDepositBase: Balance = 10 * NANO_VTRS;
    pub BountyDepositPayoutDelay: BlockNumber = prod_or_fast!(8 * DAYS, 6 * MINUTES, "VITREUS_BOUNTY_DELAY");
    pub BountyUpdatePeriod: BlockNumber = prod_or_fast!(90 * DAYS, 40 * MINUTES, "VITREUS_BOUNTY_UPDATE_PERIOD");
    pub const MaximumReasonLength: u32 = 16384;
    pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
    pub const CuratorDepositMin: Balance = 100 * NANO_VTRS;
    pub const CuratorDepositMax: Balance = 2 * MICRO_VTRS;
    pub const BountyValueMinimum: Balance = 100 * NANO_VTRS;
}

impl pallet_bounties::Config for Runtime {
    type BountyDepositBase = BountyDepositBase;
    type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
    type BountyUpdatePeriod = BountyUpdatePeriod;
    type CuratorDepositMultiplier = CuratorDepositMultiplier;
    type CuratorDepositMax = CuratorDepositMax;
    type CuratorDepositMin = CuratorDepositMin;
    type BountyValueMinimum = BountyValueMinimum;
    type DataDepositPerByte = DataDepositPerByte;
    type RuntimeEvent = RuntimeEvent;
    type MaximumReasonLength = MaximumReasonLength;
    type WeightInfo = pallet_bounties::weights::SubstrateWeight<Runtime>;
    type ChildBountyManager = ();
    type OnSlash = Treasury;
}

parameter_types! {
    pub LaunchPeriod: BlockNumber = prod_or_fast!(3 * DAYS, 3 * MINUTES, "VITREUS_LAUNCH_PERIOD");
    pub VotingPeriod: BlockNumber = prod_or_fast!(3 * DAYS, 3 * MINUTES, "VITREUS_VOTING_PERIOD");
    pub FastTrackVotingPeriod: BlockNumber = prod_or_fast!(3 * HOURS, MINUTES, "VITREUS_FAST_TRACK_VOTING_PERIOD");
    pub const MinimumDeposit: Balance = UNITS;
    pub EnactmentPeriod: BlockNumber = prod_or_fast!(3 * DAYS, MINUTES, "VITREUS_ENACTMENT_PERIOD");
    pub CooloffPeriod: BlockNumber = prod_or_fast!(7 * DAYS, MINUTES, "VITREUS_COOLOFF_PERIOD");
    pub const InstantAllowed: bool = true;
    pub const MaxVotes: u32 = 100;
    pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
    type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
    type Scheduler = Scheduler;
    type Preimages = Preimage;
    type Currency = DemocracyExtension;
    type EnactmentPeriod = EnactmentPeriod;
    type LaunchPeriod = LaunchPeriod;
    type VotingPeriod = VotingPeriod;
    type VoteLockingPeriod = EnactmentPeriod;
    type MinimumDeposit = MinimumDeposit;
    type InstantAllowed = InstantAllowed;
    type FastTrackVotingPeriod = FastTrackVotingPeriod;
    type CooloffPeriod = CooloffPeriod;
    type MaxVotes = MaxVotes;
    type MaxProposals = MaxProposals;
    type MaxDeposits = ConstU32<100>;
    type MaxBlacklisted = ConstU32<100>;
    /// A straight majority of the council can decide what their next motion is.
    type ExternalOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
        frame_system::EnsureRoot<AccountId>,
    >;
    /// A 60% super-majority can have the next scheduled referendum be a straight majority-carries vote.
    type ExternalMajorityOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
        frame_system::EnsureRoot<AccountId>,
    >;
    /// A unanimous council can have the next scheduled referendum be a straight default-carries
    /// (NTB) vote.
    type ExternalDefaultOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
        frame_system::EnsureRoot<AccountId>,
    >;
    type SubmitOrigin = frame_system::EnsureSigned<AccountId>;
    /// Two thirds of the technical committee can have an `ExternalMajority/ExternalDefault` vote
    /// be tabled immediately and with a shorter voting/enactment period.
    type FastTrackOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
        frame_system::EnsureRoot<AccountId>,
    >;
    type InstantOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
        frame_system::EnsureRoot<AccountId>,
    >;
    // To cancel a proposal which has been passed, 2/3 of the council must agree to it.
    type CancellationOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
        EnsureRoot<AccountId>,
    >;
    type BlacklistOrigin = EnsureRoot<AccountId>;
    // To cancel a proposal before it has been passed, the technical committee must be unanimous or
    // Root must agree.
    type CancelProposalOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
        EnsureRoot<AccountId>,
    >;
    // Any single technical committee member may veto a coming council proposal, however they can
    // only do it once and it lasts only for the cooloff period.
    type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
    type PalletsOrigin = OriginCaller;
    type Slash = Treasury;
}

impl pallet_democracy_extension::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
        frame_system::EnsureRoot<AccountId>,
    >;
    type Currency = Balances;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

impl pallet_evm_chain_id::Config for Runtime {}

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

parameter_types! {
    pub SuicideQuickClearLimit: u32 = 0;
}

impl pallet_evm::Config for Runtime {
    type FeeCalculator = FixedFeeCalculator;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type WeightPerGas = WeightPerGas;
    type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
    type CallOrigin = EnsureAccountId20;
    type WithdrawOrigin = EnsureAccountId20;
    type AddressMapping = IdentityAddressMapping;
    type Currency = CurrencyAdapter<EnergyAsset>;
    type RuntimeEvent = RuntimeEvent;
    type PrecompilesType = VitreusPrecompiles<Self>;
    type PrecompilesValue = PrecompilesValue;
    type ChainId = EVMChainId;
    type BlockGasLimit = BlockGasLimit;
    type Runner = pallet_evm::runner::stack::Runner<Self>;
    type OnChargeTransaction = EnergyFee;
    type OnCreate = ();
    type FindAuthor = FindAuthorTruncated<Babe>;
    type GasLimitPovSizeRatio = GasLimitPovSizeRatio;
    type SuicideQuickClearLimit = SuicideQuickClearLimit;
    type Timestamp = Timestamp;
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

parameter_types! {
    /// Amount of weight that can be spent per block to service messages.
    ///
    /// # WARNING
    ///
    /// This is not a good value for para-chains since the `Scheduler` already uses up to 80% block weight.
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(20) * BlockWeights::get().max_block;
    pub MessageQueueIdleServiceWeight: Weight = Perbill::from_percent(20) * BlockWeights::get().max_block;
    pub const MessageQueueHeapSize: u32 = 65_536;
    pub const MessageQueueMaxStale: u32 = 8;
}

impl pallet_message_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Size = u32;
    type HeapSize = MessageQueueHeapSize;
    type MaxStale = MessageQueueMaxStale;
    type ServiceWeight = MessageQueueServiceWeight;
    type IdleMaxServiceWeight = MessageQueueIdleServiceWeight;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type MessageProcessor = MessageProcessor;
    #[cfg(feature = "runtime-benchmarks")]
    type MessageProcessor =
        pallet_message_queue::mock_helpers::NoopMessageProcessor<AggregateMessageOrigin>;
    type QueueChangeHandler = ParaInclusion;
    type QueuePausedQuery = ();
    type WeightInfo = pallet_message_queue::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const FaucetMaxAmount: Balance = 1000 * UNITS;
    pub const FaucetAccumulationPeriod: BlockNumber = 1 * DAYS;
}

#[cfg(feature = "testnet-runtime")]
impl pallet_faucet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxAmount = FaucetMaxAmount;
    type AccumulationPeriod = FaucetAccumulationPeriod;
    type WeightInfo = pallet_faucet::weights::SubstrateWeight<Runtime>;
}

impl pallet_manual_bridge::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PayoutOrigin =
        pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>;
    type BridgeAccount = xcm_config::CheckAccount;
    type FeeReceiverAccount = xcm_config::TreasuryAccount;
    type DepositFeePercent = xcm_config::DepositFeePercent;
    type WithdrawalFeePercent = xcm_config::WithdrawalFeePercent;
}
