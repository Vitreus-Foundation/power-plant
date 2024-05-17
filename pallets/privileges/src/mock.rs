use crate::{self as pallet_privileges, *};
use std::collections::BTreeMap;

use frame_support::{
    assert_ok, ord_parameter_types, parameter_types,
    storage::StorageValue,
    traits::{
        AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, Currency, EitherOfDiverse,
        FindAuthor, Get, Hooks, Imbalance, OnUnbalanced, OneSessionHandler,
    },
    weights::constants::RocksDbWeight,
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy};
use orml_traits::GetByKey;
use pallet_energy_generation::{
    CurrentEra, EnergyDebtOf, EnergyOf, ErasEnergyPerStakeCurrency, ErasStakers, ErasTotalStake,
    Ledger, RewardDestination, SessionInterface, StakeNegativeImbalanceOf, StakeOf, StakerStatus,
    TestBenchmarkingConfig, ValidatorPrefs,
};
use pallet_reputation::{ReputationPoint, ReputationRecord, ReputationTier, RANKS_PER_TIER};
use parity_scale_codec::Compact;
use sp_core::H256;
use sp_runtime::traits::BlakeTwo256;
use sp_runtime::{
    curve::PiecewiseLinear,
    testing::{TestSignature, UintAuthorityId},
    traits::{IdentityLookup, Zero},
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
pub(crate) type Balance = u128;
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
        EnergyGeneration: pallet_energy_generation,
        Session: pallet_session,
        Reputation: pallet_reputation,
        Nfts: pallet_nfts,
        Historical: pallet_session::historical,
        NacManaging: pallet_nac_managing,
        Privileges: pallet_privileges,
    }
);

parameter_types! {
    pub static SessionsPerEra: SessionIndex = 3;
    pub static ExistentialDeposit: Balance = 1;
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
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
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
    type MetadataDepositBase = ConstU128<1>;
    type AttributeDepositBase = ConstU128<1>;
    type DepositPerByte = ConstU128<1>;
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
}

impl pallet_nac_managing::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type Nfts = Nfts;
    type Balance = Balance;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type NftCollectionId = NftCollectionId;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
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

impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type MaxHolds = ();
    type MaxFreezes = ();
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
    pub static RewardRemainderUnbalanced: u128 = 0;
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
    ) {
        LedgerSlashPerEra::set((slashed_bonded, slashed_chunks.clone()));
    }
}

parameter_types! {
    pub const VNRG: AssetId = 1;
    pub static BatterySlotCapacity: EnergyOf<Test> = EnergyOf::<Test>::from(100_000_000_000u128);
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
    type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
    type MaxUnlockingChunks = MaxUnlockingChunks;
    type EventListeners = EventListenerMock;
    type ValidatorReputationTier = ValidatorReputationTier;
    type CollaborativeValidatorReputationTier = CollaborativeValidatorReputationTier;
    type ReputationTierEnergyRewardAdditionalPercentMapping =
        ReputationTierEnergyRewardAdditionalPercentMapping;
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
            energy_per_stake_currency: 1_000_000u128,
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
            // replace any of the status if needed.
            self.status.into_iter().for_each(|(stash, status)| {
                let (_, _, _, ref mut prev_status) = stakers
                    .iter_mut()
                    .find(|s| s.0 == stash)
                    .expect("set_status staker should exist; qed");
                *prev_status = status;
            });
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
            min_common_validator_bond: 500,
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

pub(crate) fn active_era() -> EraIndex {
    EnergyGeneration::active_era().unwrap().index
}

pub(crate) fn current_era() -> EraIndex {
    EnergyGeneration::current_era().unwrap()
}

pub(crate) fn bond(stash: AccountId, ctrl: AccountId, val: Balance) {
    let _ = Balances::make_free_balance_be(&stash, val);
    let _ = Balances::make_free_balance_be(&ctrl, val);
    assert_ok!(EnergyGeneration::bond(
        RuntimeOrigin::signed(stash),
        ctrl,
        val,
        RewardDestination::Controller
    ));
}

pub(crate) fn bond_cooperator(
    stash: AccountId,
    ctrl: AccountId,
    val: Balance,
    target: Vec<(AccountId, Balance)>,
) {
    bond(stash, ctrl, val);
    assert_ok!(EnergyGeneration::cooperate(RuntimeOrigin::signed(ctrl), target));
}

/// Progress to the given block, triggering session and era changes as we progress.
///
/// This will finalize the previous block, initialize up to the given block, essentially simulating
/// a block import/propose process where we first initialize the block, then execute some stuff (not
/// in the function), and then finalize the block.
pub(crate) fn run_to_block(n: BlockNumber) {
    EnergyGeneration::on_finalize(System::block_number());
    for b in (System::block_number() + 1)..=n {
        System::set_block_number(b);
        Session::on_initialize(b);
        <EnergyGeneration as Hooks<u64>>::on_initialize(b);
        Timestamp::set_timestamp(System::block_number() * BLOCK_TIME + INIT_TIMESTAMP);
        if b != n {
            EnergyGeneration::on_finalize(System::block_number());
        }
    }
}

/// Progresses from the current block number (whatever that may be) to the `P * session_index + 1`.
pub(crate) fn start_session(session_index: SessionIndex) {
    let end: u64 = if Offset::get().is_zero() {
        (session_index as u64) * Period::get()
    } else {
        Offset::get() + (session_index.saturating_sub(1) as u64) * Period::get()
    };
    run_to_block(end);
    // session must have progressed properly.
    assert_eq!(
        Session::current_index(),
        session_index,
        "current session index = {}, expected = {}",
        Session::current_index(),
        session_index,
    );
}

/// Go one session forward.
pub(crate) fn advance_session() {
    let current_index = Session::current_index();
    start_session(current_index + 1);
}

/// Progress until the given era.
pub(crate) fn start_active_era(era_index: EraIndex) {
    start_session(era_index * <SessionsPerEra as Get<u32>>::get());
    assert_eq!(active_era(), era_index);
    // One way or another, current_era must have changed before the active era, so they must match
    // at this point.
    assert_eq!(current_era(), active_era());
}

pub(crate) fn current_total_payout_for_duration(duration: u64) -> Balance {
    let num_blocks = duration / BLOCK_TIME;
    let era_index = CurrentEra::<Test>::get().unwrap_or_default();
    let rate = ErasEnergyPerStakeCurrency::<Test>::get(era_index).unwrap_or_default();
    let total_stake = ErasTotalStake::<Test>::get(era_index);
    let era_blocks = Period::get() * SessionsPerEra::get() as u64 - 1;
    let ratio = Perbill::from_rational(num_blocks, era_blocks);
    let payout = ratio * rate * total_stake;

    assert!(payout > 0);
    payout
}

pub(crate) fn make_validator(controller: AccountId, stash: AccountId, balance: Balance) {
    assert_ok!(Reputation::force_set_points(
        RuntimeOrigin::root(),
        controller,
        CollaborativeValidatorReputationTier::get().into()
    ));

    assert_ok!(Reputation::force_set_points(
        RuntimeOrigin::root(),
        stash,
        CollaborativeValidatorReputationTier::get().into()
    ));

    bond(stash, controller, balance);

    assert_ok!(EnergyGeneration::validate(
        RuntimeOrigin::signed(controller),
        ValidatorPrefs::default_collaborative()
    ));

    assert_ok!(Session::set_keys(
        RuntimeOrigin::signed(controller),
        SessionKeys { other: controller.into() },
        vec![]
    ));
}

/// Time it takes to finish a session.
///
/// Note, if you see `time_per_session() - BLOCK_TIME`, it is fine. This is because we set the
/// timestamp after on_initialize, so the timestamp is always one block old.
pub(crate) fn time_per_session() -> u64 {
    Period::get() * BLOCK_TIME
}

// reputation reward points each account receive per session
pub(crate) fn reputation_per_sessions(num: u64) -> u64 {
    Period::get() * num * *pallet_reputation::REPUTATION_POINTS_PER_BLOCK
}

pub(crate) fn reputation_per_era() -> u64 {
    reputation_per_sessions(SessionsPerEra::get() as u64)
}

/// Time it takes to finish an era.
///
/// Note, if you see `time_per_era() - BLOCK_TIME`, it is fine. This is because we set the
/// timestamp after on_initialize, so the timestamp is always one block old.
pub(crate) fn time_per_era() -> u64 {
    time_per_session() * SessionsPerEra::get() as u64
}

/// Time that will be calculated for the reward per era.
pub(crate) fn reward_time_per_era() -> u64 {
    time_per_era() - BLOCK_TIME
}

pub(crate) fn reward_all_elected() {
    let rewards = <Test as pallet_energy_generation::Config>::SessionInterface::validators()
        .into_iter()
        .map(|v| (v, 1.into()));

    <pallet_energy_generation::Pallet<Test>>::reward_by_ids(rewards)
}

pub(crate) fn validator_controllers() -> Vec<AccountId> {
    Session::validators()
        .into_iter()
        .map(|s| EnergyGeneration::bonded(s).expect("no controller for validator"))
        .collect()
}

/// Make all validator and cooperator request their payment
pub(crate) fn make_all_reward_payment(era: EraIndex) {
    let validators: Vec<_> = ErasStakers::<Test>::iter()
        .filter_map(|(e, validator, _)| if e == era { Some(validator) } else { None })
        .collect();

    // reward validators
    for validator_controller in validators.iter().filter_map(EnergyGeneration::bonded) {
        let ledger = <Ledger<Test>>::get(validator_controller).unwrap();
        assert_ok!(EnergyGeneration::payout_stakers(
            RuntimeOrigin::signed(1337),
            ledger.stash,
            era
        ));
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

pub(crate) fn balances(who: &AccountId) -> (Balance, Balance) {
    (Balances::free_balance(who), Balances::reserved_balance(who))
}
