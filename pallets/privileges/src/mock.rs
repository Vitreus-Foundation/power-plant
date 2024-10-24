//!
//! # Module Overview
//!
//! This module defines a mock runtime for testing various Substrate-based pallets, including
//! `pallet_energy_generation`, `pallet_reputation`, `pallet_authorship`, `pallet_privileges`,
//! `pallet_balances`, `pallet_vesting`, `pallet_claiming`, and `pallet_nac_managing`. The mock
//! runtime creates a simulated environment where the interaction between different pallets can
//! be tested without deploying to a live blockchain. This setup is crucial for validating the
//! functionality of each pallet under different scenarios and ensuring that they work correctly
//! when integrated.
//!
//! # Key Features and Components
//!
//! - **Mock Runtime Construction**:
//!   - **`construct_runtime!` Macro**: Uses the `construct_runtime!` macro to create the mock runtime,
//!     `Test`, which includes the necessary pallets such as `System`, `Balances`, `Timestamp`,
//!     `Authorship`, `EnergyGeneration`, and others. This allows developers to simulate real-world
//!     interactions between pallets.
//!   - **Runtime Types and Constants**: The mock runtime defines key types such as `AccountId`,
//!     `Balance`, and constants like `MinVestedTransfer` and `ExistentialDeposit`. These are used
//!     to set up the initial conditions for tests, ensuring that all scenarios are covered effectively.
//!
//! - **Pallet Configuration**:
//!   - **`pallet_balances::Config`**: Implements configuration for the Balances pallet, including
//!     the `ExistentialDeposit` to ensure that accounts with minimal balance remain active.
//!   - **`pallet_vesting::Config`**: Configures the Vesting pallet with parameters for managing
//!     vested funds, ensuring that users cannot withdraw funds before they are fully vested.
//!   - **`pallet_claiming::Config`**: Configures the Claiming pallet, defining how claims are processed,
//!     integrating the `OnClaim` handler from `pallet_nac_managing` to ensure that claiming actions
//!     correctly affect VIP membership and NAC levels.
//!
//! - **Signature and Key Handling**:
//!   - **Ethereum Address Generation**: Functions like `eth()` and `bob()` are used to derive Ethereum
//!     addresses from secret keys using the Keccak-256 hashing algorithm. This is helpful for tests
//!     involving Ethereum compatibility or cross-chain claims.
//!   - **Signature Creation**: The `sig()` function generates an ECDSA signature for given data. This
//!     function is used in tests that require signed transactions or messages to ensure authenticity
//!     and simulate user interactions in a secure manner.
//!
//! - **Test Utility Functions**:
//!   - **`new_test_ext()`**: Initializes the genesis state of the blockchain to provide a clean testing
//!     environment. This ensures that each test starts with the same initial conditions, allowing for
//!     consistent and reproducible test results.
//!   - **Helper Functions for Public Keys and Hex Conversion**: Functions like `public()` and
//!     `to_ascii_hex()` are provided to convert data into public keys or hexadecimal representations,
//!     supporting more advanced testing scenarios involving cryptographic operations.
//!
//! # Access Control and Security
//!
//! - **Root and Signed Origins**: Several pallets are configured to use root or signed origins for
//!   executing certain operations, such as adding new members or modifying balances. This simulates
//!   access control measures that would be in place on a live blockchain, ensuring that only authorized
//!   users can perform critical actions.
//! - **Controlled Test Environment**: The mock runtime operates within a controlled environment, which
//!   means full access to modify the blockchain state. This level of control is only appropriate for
//!   testing and allows developers to simulate scenarios that would be impossible or risky in a live
//!   environment.
//!
//! # Developer Notes
//!
//! - **Integrated Testing of Multiple Pallets**: The mock runtime integrates several pallets into one
//!   cohesive testing environment. Developers can write unit tests that involve interactions between
//!   different pallets, such as claiming rewards, generating energy, and updating VIP memberships.
//! - **Cryptographic Compatibility**: The Ethereum address and signature functions are particularly
//!   useful for testing interactions that involve external chains or off-chain systems. Developers
//!   should ensure that the cryptographic operations match the standards expected by other systems,
//!   such as Ethereum's signature format.
//! - **Testing Vesting and Claiming Logic**: The configurations for the Vesting and Claiming pallets
//!   allow for thorough testing of vesting schedules and reward claiming scenarios. Developers should
//!   use these configurations to ensure that users receive the correct rewards and that their vested
//!   funds are appropriately managed.
//!
//! # Usage Scenarios
//!
//! - **Testing Cross-Pallet Interactions**: The mock runtime setup allows developers to test how
//!   different pallets interact, such as when a user claims rewards (`pallet_claiming`) and this
//!   affects their reputation or VIP membership (`pallet_nac_managing`). These cross-pallet tests
//!   help identify integration issues before deploying to a production environment.
//! - **Simulating Key Management and Signatures**: Functions like `sig()` and `public()` are used to
//!   simulate user interactions that require signed messages. For instance, developers can test how
//!   claims are processed when signed by a particular user's key, ensuring that the system correctly
//!   validates the signature before allowing an action.
//! - **Role-Based Testing**: By configuring certain actions to require root access, developers can
//!   simulate and test scenarios where administrative actions are performed. This helps ensure that
//!   privileged actions are restricted to authorized accounts and cannot be executed by unauthorized
//!   users.
//!
//! # Integration Considerations
//!
//! - **Updating Runtime Configurations**: Developers should keep in mind that the runtime configurations
//!   used in the mock environment are simplified versions. When integrating with a production environment,
//!   parameters like `ExistentialDeposit` and vesting conditions should be adjusted to match the economic
//!   model and security requirements of the live blockchain.
//! - **Testing with Different Account Types**: The use of simplified `AccountId` types (`u64`) in the
//!   mock runtime is convenient for testing but should be adjusted for more complex or production-ready
//!   implementations where stronger typing or more robust identifiers are required.
//! - **Benchmarking Weights and Performance**: The mock runtime provides a controlled environment for
//!   testing performance metrics such as transaction weights. Developers should use this setup to
//!   benchmark the computational cost of different extrinsics, ensuring that weights are properly
//!   configured to reflect real-world resource usage.
//!
//! # Example Scenario
//!
//! Suppose a developer needs to test how users claim rewards through the `pallet_claiming` and how
//! this affects their NAC level and VIP status managed by `pallet_nac_managing`. Using `new_test_ext()`,
//! the developer can initialize a clean testing environment and simulate a user claiming a reward. The
//! signature is generated using `sig()` to ensure authenticity, and the `OnClaim` handler from the NAC
//! managing pallet updates the user's VIP status accordingly. The developer can then validate that the
//! user's NAC level and reputation have been updated as expected and that all cross-pallet interactions
//! occurred without errors.
//!


use crate::{self as pallet_privileges, *};
use std::collections::BTreeMap;

use frame_support::{
    derive_impl, ord_parameter_types, parameter_types,
    traits::{
        AsEnsureOriginWithArg, ConstU32, ConstU64, EitherOfDiverse, FindAuthor, Hooks, Imbalance,
        OnUnbalanced, OneSessionHandler, WithdrawReasons,
    },
    weights::constants::RocksDbWeight,
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy};
use pallet_claiming::{EcdsaSignature, EthereumAddress};
use pallet_energy_generation::{
    EnergyDebtOf, EnergyOf, StakeNegativeImbalanceOf, StakeOf, StakerStatus, TestBenchmarkingConfig,
};
use pallet_reputation::{ReputationPoint, ReputationRecord, ReputationTier};
use parity_scale_codec::Compact;
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::traits::BlakeTwo256;
use sp_runtime::{
    curve::PiecewiseLinear,
    testing::{TestSignature, UintAuthorityId},
    traits::{Identity, IdentityLookup, Zero},
    BuildStorage,
};
use sp_staking::{EraIndex, OnStakingUpdate, SessionIndex};
use sp_std::vec;

type Block = frame_system::mocking::MockBlock<Test>;

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;

/// The AccountId alias in this test module.
pub(crate) type AccountId = u64;
pub(crate) type Nonce = u64;
pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u64;
pub(crate) type Signature = TestSignature;
pub(crate) type AccountPublic = UintAuthorityId;

/// Another session handler struct to test on_disabled.
pub struct OtherSessionHandler;

impl sp_runtime::BoundToRuntimeAppPublic for OtherSessionHandler {
    type Public = UintAuthorityId;
}

impl OneSessionHandler<AccountId> for OtherSessionHandler {
    type Key = UintAuthorityId;

    fn on_genesis_session<'a, I: 'a>(_: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_new_session<'a, I: 'a>(_: bool, _: I, _: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_disabled(_validator_index: u32) {}
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Assets: pallet_assets,
        Timestamp: pallet_timestamp,
        Authorship: pallet_authorship,
        Balances: pallet_balances,
        Claiming: pallet_claiming,
        EnergyGeneration: pallet_energy_generation,
        Session: pallet_session,
        Reputation: pallet_reputation,
        Nfts: pallet_nfts,
        Historical: pallet_session::historical,
        NacManaging: pallet_nac_managing,
        Privileges: pallet_privileges,
        Vesting: pallet_vesting,
    }
);

parameter_types! {
    pub static SessionsPerEra: SessionIndex = 3;
    pub static ExistentialDeposit: Balance = 10;
    pub static SlashDeferDuration: EraIndex = 0;
    pub static Period: BlockNumber = 5;
    pub static Offset: BlockNumber = 0;
}

/// Author of block is always 11
pub struct Author11;
impl FindAuthor<AccountId> for Author11 {
    fn find_author<'a, I>(_digests: I) -> Option<AccountId>
    where
        I: 'a + IntoIterator<Item = (frame_support::ConsensusEngineId, &'a [u8])>,
    {
        Some(11)
    }
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = Nonce;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type BlockHashCount = ConstU64<250>;
    type DbWeight = RocksDbWeight;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = NacManaging;
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

parameter_types! {
    pub TestCollectionDeposit:  u64 = 0;
    pub TestItemDeposit:  u64 = 0;
}

type CollectionId = u32;
type ItemId = u32;

impl pallet_nfts::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type CollectionDeposit = TestCollectionDeposit;
    type ApprovalsLimit = ();
    type ItemAttributesApprovalsLimit = ();
    type MaxTips = ();
    type MaxDeadlineDuration = ();
    type MaxAttributesPerCall = ();
    type Features = ();
    type OffchainPublic = AccountPublic;
    type OffchainSignature = Signature;
    type ItemDeposit = TestItemDeposit;
    type MetadataDepositBase = ConstU64<1>;
    type AttributeDepositBase = ConstU64<1>;
    type DepositPerByte = ConstU64<1>;
    type StringLimit = ConstU32<50>;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<Self::AccountId>>;
    type Locker = ();
}

impl pallet_timestamp::Config for Test {
    type MinimumPeriod = ConstU64<1000>;
    type Moment = u64;
    type OnTimestampSet = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const NftCollectionId: CollectionId = 0;
    pub const VIPPCollectionId: CollectionId = 1;
}

impl pallet_nac_managing::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type Nfts = Nfts;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type NftCollectionId = NftCollectionId;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
    type Currency = Balances;
    type VIPPCollectionId = VIPPCollectionId;
    type OnVIPPChanged = Privileges;
}

parameter_types! {
    pub const AssetDeposit: Balance = 0;
    pub const AssetAccountDeposit: Balance = 0;
    pub const ApprovalDeposit: Balance = 0;
    pub const AssetsStringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 0;
    pub const MetadataDepositPerByte: Balance = 0;
}

pub type AssetId = u32;

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = AssetsStringLimit;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetIdParameter = Compact<AssetId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type CallbackHandle = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub other: OtherSessionHandler,
    }
}

impl pallet_session::Config for Test {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Test, EnergyGeneration>;
    type Keys = SessionKeys;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (OtherSessionHandler,);
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_energy_generation::StashOf<Test>;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

impl pallet_session::historical::Config for Test {
    type FullIdentification = pallet_energy_generation::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_energy_generation::ExposureOf<Test>;
}

impl pallet_reputation::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl pallet_authorship::Config for Test {
    type FindAuthor = Author11;
    type EventHandler = pallet_energy_generation::Pallet<Test>;
}

impl pallet_privileges::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type UnixTime = Timestamp;
    type WeightInfo = ();
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU64<1>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type MaxFreezes = ();
}

parameter_types! {
    pub const MinVestedTransfer: u64 = 1;
    pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
        WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

impl pallet_vesting::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = Identity;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = ();
    type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
    type BlockNumberProvider = System;
    const MAX_VESTING_SCHEDULES: u32 = 28;
}

parameter_types! {
    pub Prefix: &'static [u8] = b"Pay RUSTs to the TEST account:";
}

impl pallet_claiming::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type VestingSchedule = Vesting;
    type OnClaim = NacManaging;
    type Prefix = Prefix;
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

parameter_types! {
    pub const BondingDuration: EraIndex = 3;
    pub const RewardCurve: &'static PiecewiseLinear<'static> = &I_NPOS;
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(75);
}

parameter_types! {
    static StakingEventsIndex: usize = 0;
}
ord_parameter_types! {
    pub const One: u64 = 1;
}

type EnsureOneOrRoot = EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<One, AccountId>>;

parameter_types! {
    pub static RewardRemainderUnbalanced: u64 = 0;
}

pub struct RewardRemainderMock;

impl OnUnbalanced<StakeNegativeImbalanceOf<Test>> for RewardRemainderMock {
    fn on_nonzero_unbalanced(amount: StakeNegativeImbalanceOf<Test>) {
        RewardRemainderUnbalanced::mutate(|v| {
            *v += amount.peek();
        });
        drop(amount);
    }
}

pub struct MockReward;
impl OnUnbalanced<EnergyDebtOf<Test>> for MockReward {
    fn on_unbalanced(_: EnergyDebtOf<Test>) {
        RewardOnUnbalanceWasCalled::set(true);
    }
}

pub struct EventListenerMock;
impl OnStakingUpdate<AccountId, Balance> for EventListenerMock {
    fn on_slash(
        _pool_account: &AccountId,
        slashed_bonded: Balance,
        slashed_chunks: &BTreeMap<EraIndex, Balance>,
        _slashed_total: Balance,
    ) {
        LedgerSlashPerEra::set((slashed_bonded, slashed_chunks.clone()));
    }
}

parameter_types! {
    pub const VNRG: AssetId = 1;
    pub static BatterySlotCapacity: EnergyOf<Test> = EnergyOf::<Test>::from(100_000_000_000u64);
    pub static MaxCooperations: u32 = 16;
    pub static HistoryDepth: u32 = 80;
    pub static MaxUnlockingChunks: u32 = 32;
    pub static RewardOnUnbalanceWasCalled: bool = false;
    pub static LedgerSlashPerEra: (StakeOf<Test>, BTreeMap<EraIndex, StakeOf<Test>>) = (Zero::zero(), BTreeMap::new());
    pub static MaxWinners: u32 = 100;
    pub static ValidatorReputationTier: ReputationTier = ReputationTier::Vanguard(1);
    pub static CollaborativeValidatorReputationTier: ReputationTier = ReputationTier::Vanguard(1);
}

impl pallet_energy_generation::Config for Test {
    type StakeCurrency = Balances;
    type StakeBalance = <Self as pallet_balances::Config>::Balance;
    type EnergyAssetId = VNRG;
    type BatterySlotCapacity = BatterySlotCapacity;
    type UnixTime = Timestamp;
    type MaxCooperations = MaxCooperations;
    type HistoryDepth = HistoryDepth;
    type RewardRemainder = RewardRemainderMock;
    type RuntimeEvent = RuntimeEvent;
    type Slash = ();
    type Reward = MockReward;
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type AdminOrigin = EnsureOneOrRoot;
    type SessionInterface = Self;
    type EnergyPerStakeCurrency = EnergyGeneration;
    type NextNewSession = Session;
    type MaxCooperatorRewardedPerValidator = ConstU32<64>;
    type MaxUnlockingChunks = MaxUnlockingChunks;
    type EventListeners = EventListenerMock;
    type DisablingStrategy = pallet_energy_generation::UpToLimitDisablingStrategy<3>;
    type ValidatorReputationTier = ValidatorReputationTier;
    type CollaborativeValidatorReputationTier = CollaborativeValidatorReputationTier;
    type ReputationTierEnergyRewardAdditionalPercentMapping = ();
    type ValidatorNacLevel = ();
    type OnVipMembershipHandler = Privileges;
    type BenchmarkingConfig = TestBenchmarkingConfig;
    type ThisWeightInfo = ();
}

pub enum CooperateSelector {
    CooperateWithDefault,
    CooperateWith(Vec<(AccountId, Balance)>),
    NoCooperate,
}

pub struct ExtBuilder {
    cooperate: CooperateSelector,
    validator_count: u32,
    minimum_validator_count: u32,
    invulnerables: Vec<AccountId>,
    has_stakers: bool,
    initialize_first_session: bool,
    pub min_cooperator_bond: Balance,
    min_common_validator_bond: Balance,
    min_trust_validator_bond: Balance,
    balance_factor: Balance,
    status: BTreeMap<AccountId, StakerStatus<AccountId, Balance>>,
    stakes: BTreeMap<AccountId, Balance>,
    stakers: Vec<(AccountId, AccountId, Balance, StakerStatus<AccountId, Balance>)>,
    energy_per_stake_currency: Balance,
    block_authoring_reward: ReputationPoint,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            cooperate: CooperateSelector::CooperateWithDefault,
            validator_count: 2,
            minimum_validator_count: 0,
            balance_factor: 1,
            invulnerables: vec![],
            has_stakers: true,
            initialize_first_session: true,
            min_cooperator_bond: ExistentialDeposit::get(),
            min_common_validator_bond: ExistentialDeposit::get(),
            min_trust_validator_bond: ExistentialDeposit::get(),
            status: Default::default(),
            stakes: Default::default(),
            stakers: Default::default(),
            energy_per_stake_currency: 1_000_000u64,
            block_authoring_reward: ReputationPoint(12),
        }
    }
}

impl ExtBuilder {
    pub fn existential_deposit(self, existential_deposit: Balance) -> Self {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow_mut() = existential_deposit);
        self
    }
    pub fn cooperate(mut self, cooperate: CooperateSelector) -> Self {
        self.cooperate = cooperate;
        self
    }
    pub fn no_cooperate(mut self) -> Self {
        self.cooperate = CooperateSelector::NoCooperate;
        self
    }
    pub fn default_cooperate(mut self) -> Self {
        self.cooperate = CooperateSelector::CooperateWithDefault;
        self
    }
    pub fn validator_count(mut self, count: u32) -> Self {
        self.validator_count = count;
        self
    }
    pub fn minimum_validator_count(mut self, count: u32) -> Self {
        self.minimum_validator_count = count;
        self
    }
    pub fn slash_defer_duration(self, eras: EraIndex) -> Self {
        SLASH_DEFER_DURATION.with(|v| *v.borrow_mut() = eras);
        self
    }
    pub fn invulnerables(mut self, invulnerables: Vec<AccountId>) -> Self {
        self.invulnerables = invulnerables;
        self
    }
    pub fn session_per_era(self, length: SessionIndex) -> Self {
        SESSIONS_PER_ERA.with(|v| *v.borrow_mut() = length);
        self
    }
    pub fn period(self, length: BlockNumber) -> Self {
        PERIOD.with(|v| *v.borrow_mut() = length);
        self
    }
    pub fn has_stakers(mut self, has: bool) -> Self {
        self.has_stakers = has;
        self
    }
    pub fn initialize_first_session(mut self, init: bool) -> Self {
        self.initialize_first_session = init;
        self
    }
    pub fn offset(self, offset: BlockNumber) -> Self {
        OFFSET.with(|v| *v.borrow_mut() = offset);
        self
    }
    pub fn min_cooperator_bond(mut self, amount: Balance) -> Self {
        self.min_cooperator_bond = amount;
        self
    }
    pub fn min_common_validator_bond(mut self, amount: Balance) -> Self {
        self.min_common_validator_bond = amount;
        self
    }
    pub fn min_trust_validator_bond(mut self, amount: Balance) -> Self {
        self.min_trust_validator_bond = amount;
        self
    }
    pub fn set_status(mut self, who: AccountId, status: StakerStatus<AccountId, Balance>) -> Self {
        self.status.insert(who, status);
        self
    }
    pub fn set_stake(mut self, who: AccountId, stake: Balance) -> Self {
        self.stakes.insert(who, stake);
        self
    }
    pub fn add_staker(
        mut self,
        stash: AccountId,
        ctrl: AccountId,
        stake: Balance,
        status: StakerStatus<AccountId, Balance>,
    ) -> Self {
        self.stakers.push((stash, ctrl, stake, status));
        self
    }
    pub fn balance_factor(mut self, factor: Balance) -> Self {
        self.balance_factor = factor;
        self
    }
    fn build(self) -> sp_io::TestExternalities {
        sp_tracing::try_init_simple();
        let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

        let _ = pallet_assets::GenesisConfig::<Test> {
            assets: vec![(VNRG::get(), 1, true, 1)],
            ..Default::default()
        }
        .assimilate_storage(&mut storage);

        let validator_reputation = ReputationRecord {
            reputation: <Test as pallet_energy_generation::Config>::ValidatorReputationTier::get()
                .into(),
            updated: 0,
        };
        let collab_validator_rep = ReputationRecord {
            reputation:
            <Test as pallet_energy_generation::Config>::CollaborativeValidatorReputationTier::get().into(),
            updated: 0,
        };
        let _ = pallet_reputation::GenesisConfig::<Test> {
            accounts: vec![
                // collaborative validators
                (10, collab_validator_rep.clone()),
                (20, collab_validator_rep.clone()),
                // simple validators
                (30, validator_reputation.clone()),
                (40, validator_reputation.clone()),
                // cooperators
                (100, validator_reputation.clone()),
            ],
        }
        .assimilate_storage(&mut storage);

        let _ = pallet_nac_managing::GenesisConfig::<Test> {
            owners: vec![1],
            accounts: vec![(10, 2), (100, 2)],
        }
        .assimilate_storage(&mut storage);

        let _ = pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (1, 10 * self.balance_factor),
                (2, 20 * self.balance_factor),
                (3, 300 * self.balance_factor),
                (4, 400 * self.balance_factor),
                // controllers
                (10, self.balance_factor * 1000),
                (20, self.balance_factor * 2000),
                (30, self.balance_factor * 2000),
                (40, self.balance_factor * 2000),
                (50, self.balance_factor * 2000),
                // stashes
                (11, self.balance_factor * 1000),
                (21, self.balance_factor * 2000),
                (31, self.balance_factor * 2000),
                (41, self.balance_factor * 2000),
                (51, self.balance_factor * 2000),
                // optional cooperator
                (100, self.balance_factor * 2000),
                (101, self.balance_factor * 2000),
                // aux accounts
                (60, self.balance_factor),
                (61, self.balance_factor * 2000),
                (70, self.balance_factor),
                (71, self.balance_factor * 2000),
                (80, self.balance_factor),
                (81, self.balance_factor * 2000),
                // This allows us to have a total_payout different from 0.
                (999, 1_000_000_000_000),
            ],
        }
        .assimilate_storage(&mut storage);

        let mut stakers = vec![];
        if self.has_stakers {
            stakers = vec![
                // (stash, ctrl, stake, status)
                // these two will be elected in the default test where we elect 2.
                (10, 10, self.balance_factor * 1000, StakerStatus::Validator),
                (20, 20, self.balance_factor * 500, StakerStatus::Validator),
                // a loser validator
                (30, 30, self.balance_factor * 500, StakerStatus::Validator),
                // an idle validator
                (40, 40, self.balance_factor * 1000, StakerStatus::Idle),
            ];
            // optionally add a cooperator
            match self.cooperate {
                CooperateSelector::CooperateWithDefault => stakers.push((
                    100,
                    100,
                    self.balance_factor * 500,
                    StakerStatus::Cooperator(vec![(10, 200), (20, 300)]),
                )),
                CooperateSelector::CooperateWith(target) => stakers.push((
                    100,
                    100,
                    self.balance_factor * 500,
                    StakerStatus::Cooperator(target),
                )),
                CooperateSelector::NoCooperate => (),
            }
            // replaced any of the stakes if needed.
            self.stakes.into_iter().for_each(|(stash, stake)| {
                let (_, _, ref mut prev_stake, _) = stakers
                    .iter_mut()
                    .find(|s| s.0 == stash)
                    .expect("set_stake staker should exits; qed.");
                *prev_stake = stake;
            });
            // extend stakers if needed.
            stakers.extend(self.stakers)
        }

        let _ = pallet_energy_generation::GenesisConfig::<Test> {
            stakers: stakers.clone(),
            validator_count: self.validator_count,
            minimum_validator_count: self.minimum_validator_count,
            invulnerables: self.invulnerables,
            slash_reward_fraction: Perbill::from_percent(10),
            min_cooperator_bond: self.min_cooperator_bond,
            min_common_validator_bond: self.min_common_validator_bond,
            min_trust_validator_bond: self.min_trust_validator_bond,
            energy_per_stake_currency: self.energy_per_stake_currency,
            block_authoring_reward: self.block_authoring_reward,
            ..Default::default()
        }
        .assimilate_storage(&mut storage);

        let _ = pallet_privileges::GenesisConfig::<Test> {
            date: Some((2020, 1, 1)),
            ..Default::default()
        }
        .assimilate_storage(&mut storage);

        let _ = pallet_session::GenesisConfig::<Test> {
            keys: if self.has_stakers {
                // set the keys for the first session.
                stakers
                    .into_iter()
                    .map(|(id, ..)| (id, id, SessionKeys { other: id.into() }))
                    .collect()
            } else {
                // set some dummy validators in genesis.
                (0..self.validator_count as u64)
                    .map(|id| (id, id, SessionKeys { other: id.into() }))
                    .collect()
            },
        }
        .assimilate_storage(&mut storage);

        let mut ext = sp_io::TestExternalities::from(storage);

        if self.initialize_first_session {
            // We consider all test to start after timestamp is initialized This must be ensured by
            // having `timestamp::on_initialize` called before `staking::on_initialize`. Also, if
            // session length is 1, then it is already triggered.
            ext.execute_with(|| {
                System::set_block_number(1);
                Session::on_initialize(1);
                <EnergyGeneration as Hooks<u64>>::on_initialize(1);
                Timestamp::set_timestamp(INIT_TIMESTAMP);
            });
        }

        ext
    }
    pub fn build_and_execute(self, test: impl FnOnce()) {
        sp_tracing::try_init_simple();
        let mut ext = self.build();
        ext.execute_with(test);
        ext.execute_with(|| {
            // EnergyGeneration::do_try_state(System::block_number()).unwrap();
        });
    }
}

#[macro_export]
macro_rules! assert_session_era {
    ($session:expr, $era:expr) => {
        assert_eq!(
            Session::current_index(),
            $session,
            "wrong session {} != {}",
            Session::current_index(),
            $session,
        );
        assert_eq!(
            Staking::current_era().unwrap(),
            $era,
            "wrong current era {} != {}",
            Staking::current_era().unwrap(),
            $era,
        );
    };
}

pub(crate) fn bob() -> libsecp256k1::SecretKey {
    libsecp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

pub fn eth(secret: &libsecp256k1::SecretKey) -> EthereumAddress {
    let mut res = EthereumAddress::default();
    res.0.copy_from_slice(&keccak_256(&public(secret).serialize()[1..65])[12..]);
    res
}
pub fn public(secret: &libsecp256k1::SecretKey) -> libsecp256k1::PublicKey {
    libsecp256k1::PublicKey::from_secret_key(secret)
}

pub fn sig<T: pallet_claiming::Config>(
    secret: &libsecp256k1::SecretKey,
    what: &[u8],
    extra: &[u8],
) -> EcdsaSignature {
    let msg = keccak_256(&ethereum_signable_message(&to_ascii_hex(what)[..], extra));
    let (sig, recovery_id) = libsecp256k1::sign(&libsecp256k1::Message::parse(&msg), secret);
    let mut r = [0u8; 65];
    r[0..64].copy_from_slice(&sig.serialize()[..]);
    r[64] = recovery_id.serialize();
    EcdsaSignature(r)
}

fn ethereum_signable_message(what: &[u8], extra: &[u8]) -> Vec<u8> {
    let prefix = <Test as pallet_claiming::Config>::Prefix::get();
    let mut l = prefix.len() + what.len() + extra.len();
    let mut rev = Vec::new();
    while l > 0 {
        rev.push(b'0' + (l % 10) as u8);
        l /= 10;
    }
    let mut v = b"\x19Ethereum Signed Message:\n".to_vec();
    v.extend(rev.into_iter().rev());
    v.extend_from_slice(prefix);
    v.extend_from_slice(what);
    v.extend_from_slice(extra);
    v
}

fn to_ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(data.len() * 2);
    let mut push_nibble = |n| r.push(if n < 10 { b'0' + n } else { b'a' - 10 + n });
    for &b in data.iter() {
        push_nibble(b / 16);
        push_nibble(b % 16);
    }
    r
}
