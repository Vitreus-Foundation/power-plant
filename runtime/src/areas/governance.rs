use core::cmp::Ordering;

use crate::{prod_or_fast, MICRO_VTRS};
use crate::{
    AccountId, Balance, Balances, Preimage, BlockNumber, BlockWeights, OriginCaller,
    Runtime, RuntimeEvent, RuntimeOrigin, RuntimeCall, DAYS, HOURS, MINUTES, NANO_VTRS, PICO_VTRS,
    MoreThanHalfCouncil, TechnicalCommittee, Treasury, Scheduler, Bounties,
};

use frame_support::traits::fungible::Balanced;
use frame_support::traits::{EitherOf, EqualPrivilegeOnly, LockIdentifier, PrivilegeCmp};
use frame_support::{parameter_types, traits::EitherOfDiverse, weights::Weight, PalletId};
use frame_system::{EnsureRoot, EnsureWithSuccess};
use sp_core::ConstU32;
use sp_runtime::{Perbill, Percent, Permill};


parameter_types! {
	pub CouncilMotionDuration: BlockNumber = prod_or_fast!(7 * DAYS, 2 * MINUTES, "VITREUS_MOTION_DURATION");
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
	type MaxProposalWeight = MaxProposalWeight;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
    // TODO: add weights
	type WeightInfo = ();
}

parameter_types! {
	pub const TechnicalMotionDuration: BlockNumber = 7 * DAYS;
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
    // TODO: add weights
	type WeightInfo = ();
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
    // TODO: add weights
	type WeightInfo = ();
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 1 * MICRO_VTRS;
	pub const ProposalBondMaximum: Balance = 5 * MICRO_VTRS;
	pub const SpendPeriod: BlockNumber = 24 * DAYS;
	pub const Burn: Permill = Permill::from_percent(1);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");

	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 10 * NANO_VTRS;
	pub const DataDepositPerByte: Balance = 100 * PICO_VTRS;
	pub const MaxApprovals: u32 = 100;
	pub const MaxAuthorities: u32 = 100_000;
	pub const MaxKeys: u32 = 10_000;
	pub const MaxPeerInHeartbeats: u32 = 10_000;
	pub const MaxPeerDataEncodingSize: u32 = 1_000;
	pub const RootSpendOriginMaxAmount: Balance = Balance::MAX;
	pub const CouncilSpendOriginMaxAmount: Balance = Balance::MAX;
}

type ApproveOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
>;

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type ApproveOrigin = ApproveOrigin;
	type RejectOrigin = MoreThanHalfCouncil;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type SpendFunds = Bounties;
	type MaxApprovals = MaxApprovals;
    // TODO: add weights
	type WeightInfo = ();
	type SpendOrigin = EitherOf<
		frame_system::EnsureRootWithSuccess<AccountId, RootSpendOriginMaxAmount>,
		EnsureWithSuccess<
			pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
			AccountId,
			CouncilSpendOriginMaxAmount,
		>,
	>;
}

parameter_types! {
	pub const BountyDepositBase: Balance = 10 * NANO_VTRS;
	pub const BountyDepositPayoutDelay: BlockNumber = 8 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 90 * DAYS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub const CuratorDepositMin: Balance = 100 * NANO_VTRS;
	pub const CuratorDepositMax: Balance = 2 * MICRO_VTRS;
	pub const BountyValueMinimum: Balance = 100 * NANO_VTRS;
}

impl pallet_bounties::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type CuratorDepositMultiplier = CuratorDepositMultiplier;
	type CuratorDepositMin = CuratorDepositMin;
	type CuratorDepositMax = CuratorDepositMax;
	type BountyValueMinimum = BountyValueMinimum;
	type ChildBountyManager = ();
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
    // TODO: add weights
	type WeightInfo = ();
}

parameter_types! {
	pub LaunchPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1, "VITREUS_LAUNCH_PERIOD");
	pub VotingPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1 * MINUTES, "VITREUS_VOTING_PERIOD");
	pub FastTrackVotingPeriod: BlockNumber = prod_or_fast!(3 * HOURS, 1 * MINUTES, "VITREUS_FAST_TRACK_VOTING_PERIOD");
	pub const MinimumDeposit: Balance = 1 * MICRO_VTRS;
	pub EnactmentPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1, "VITREUS_ENACTMENT_PERIOD");
	pub CooloffPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1, "VITREUS_COOLOFF_PERIOD");
	pub const InstantAllowed: bool = true;
	pub const MaxVotes: u32 = 100;
	pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type VoteLockingPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	type SubmitOrigin = frame_system::EnsureSigned<AccountId>;
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
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = EitherOfDiverse<
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
		EnsureRoot<AccountId>,
	>;
	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	// Root must agree.
	type CancelProposalOrigin = EitherOfDiverse<
		pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
		EnsureRoot<AccountId>,
	>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = MaxVotes;
    // TODO: add weights 
	type WeightInfo = ();
	type MaxProposals = MaxProposals;
	type Preimages = Preimage;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
}


