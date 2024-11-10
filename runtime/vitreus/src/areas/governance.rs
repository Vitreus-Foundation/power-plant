use crate::{AccountId, Balance, Balances, BlockNumber, BlockWeights, Bounties, Council, MoreThanHalfCouncil, OriginCaller, Preimage, Runtime, RuntimeCall, RuntimeEvent, RuntimeHoldReason, RuntimeOrigin, Scheduler, TechnicalCommittee, Treasury, TreasuryExtension, DAYS, HOURS, MICRO_VTRS, MILLI_VTRS, MINUTES, NANO_VTRS, PICO_VTRS, UNITS};

use frame_support::traits::fungible::HoldConsideration;
use frame_support::traits::tokens::{PayFromAccount, UnityAssetBalanceConversion};
use frame_support::traits::{Currency, EitherOf, LinearStoragePrice, OnUnbalanced, LockIdentifier};
use frame_support::{parameter_types, traits::EitherOfDiverse, weights::Weight, PalletId};
use frame_system::{EnsureRoot, EnsureWithSuccess};
use pallet_treasury::NegativeImbalanceOf;
use polkadot_runtime_common::prod_or_fast;
use sp_core::ConstU32;
use sp_runtime::traits::{AccountIdConversion, IdentityLookup};
use sp_runtime::{Perbill, Permill};
use static_assertions::const_assert;

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    items as Balance * 200 * NANO_VTRS + (bytes as Balance) * PICO_VTRS
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
    pub CouncilMotionDuration: BlockNumber = prod_or_fast!(7 * DAYS, 5 * MINUTES, "VITREUS_MOTION_DURATION");
    pub const CouncilMaxProposals: u32 = 100;
    pub const CouncilMaxMembers: u32 = 100;
    pub MaxProposalWeight: Weight = Perbill::from_percent(50) * BlockWeights::get().max_block;
}

pub type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilMotionDuration;
    type MaxProposals = CouncilMaxProposals;
    type MaxMembers = CouncilMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type MaxProposalWeight = MaxProposalWeight;
}

parameter_types! {
    pub const CandidacyBond: Balance = 100 * UNITS;
    // 1 storage item created, key size is 32 bytes, value size is 16+16.
    pub const VotingBondBase: Balance = deposit(1, 64);
    // additional data per vote is 32 bytes (account id).
    pub const VotingBondFactor: Balance = deposit(0, 32);
    pub const DesiredMembers: u32 = 13;
    pub const DesiredRunnersUp: u32 = 7;
    pub const TermDuration: BlockNumber = prod_or_fast!(7 * DAYS, 5 * MINUTES);
    pub const MaxCandidates: u32 = 64;
    pub const MaxVoters: u32 = 512;
    pub const MaxVotesPerVoter: u32 = 16;
    pub const ElectionsPhragmenPalletId: LockIdentifier = *b"electphr";
}

// Make sure that there are no more than `MaxMembers` members elected via elections-phragmen.
const_assert!(DesiredMembers::get() <= CouncilMaxMembers::get());

impl pallet_elections_phragmen::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = ElectionsPhragmenPalletId;
    type Currency = Balances;
    type ChangeMembers = Council;
    // NOTE: this implies that council's genesis members cannot be set directly and must come from
    // this module.
    type InitializeMembers = Council;
    type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
    type CandidacyBond = CandidacyBond;
    type VotingBondBase = VotingBondBase;
    type VotingBondFactor = VotingBondFactor;
    type LoserCandidate = ();
    type KickedMember = ();
    type DesiredMembers = DesiredMembers;
    type DesiredRunnersUp = DesiredRunnersUp;
    type TermDuration = TermDuration;
    type MaxCandidates = MaxCandidates;
    type MaxVoters = MaxVoters;
    type MaxVotesPerVoter = MaxVotesPerVoter;
    type WeightInfo = pallet_elections_phragmen::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub TechnicalMotionDuration: BlockNumber = prod_or_fast!(7 * DAYS, 5 * MINUTES, "VITREUS_MOTION_DURATION");
    pub const TechnicalMaxProposals: u32 = 100;
    pub const TechnicalMaxMembers: u32 = 100;
}

pub type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = TechnicalMotionDuration;
    type MaxProposals = TechnicalMaxProposals;
    type MaxProposalWeight = MaxProposalWeight;
    type MaxMembers = TechnicalMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
}

impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AddOrigin = MoreThanHalfCouncil;
    type RemoveOrigin = MoreThanHalfCouncil;
    type SwapOrigin = MoreThanHalfCouncil;
    type ResetOrigin = MoreThanHalfCouncil;
    type PrimeOrigin = MoreThanHalfCouncil;
    type MembershipInitialized = TechnicalCommittee;
    type MembershipChanged = TechnicalCommittee;
    type MaxMembers = TechnicalMaxMembers;
    type WeightInfo = pallet_membership::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: Balance = 10 * MILLI_VTRS;
    pub const ProposalBondMaximum: Balance = 10 * UNITS;
    pub SpendPeriod: BlockNumber = prod_or_fast!(24 * DAYS, 40, "VITREUS_SPEND_PERIOD");
    pub const Burn: Permill = Permill::from_percent(0);
    pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
    pub const PayoutSpendPeriod: BlockNumber = 30 * DAYS;

    // TODO: reconsider
    pub const DataDepositPerByte: Balance = 100 * PICO_VTRS;
    pub const MaxApprovals: u32 = 100;
    pub const MaxAuthorities: u32 = 100_000;
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
    pub const RootSpendOriginMaxAmount: Balance = Balance::MAX;
    pub const CouncilSpendOriginMaxAmount: Balance = Balance::MAX;
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
    type Paymaster = PayFromAccount<Balances, pallet_treasury::TreasuryAccountId<Runtime>>;
    type BalanceConverter = UnityAssetBalanceConversion;
    type PayoutPeriod = PayoutSpendPeriod;
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const SpendThreshold: Permill = Permill::from_percent(10);
    pub const StakingRewardsPalletId: PalletId = PalletId(*b"stknrwrd");
    pub const LiquidityPalletId: PalletId = PalletId(*b"liquidty");
    pub const LiquidityReservesPalletId: PalletId = PalletId(*b"liqresrv");
}

pub struct StakingRewardsSink;

impl OnUnbalanced<NegativeImbalanceOf<Runtime>> for StakingRewardsSink {
    fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<Runtime>) {
        let staking_rewards_address: AccountId =
            StakingRewardsPalletId::get().into_account_truncating();
        Balances::resolve_creating(&staking_rewards_address, amount);
    }
}

impl pallet_treasury_extension::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SpendThreshold = SpendThreshold;
    type OnRecycled = StakingRewardsSink;
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
    type Currency = Balances;
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

