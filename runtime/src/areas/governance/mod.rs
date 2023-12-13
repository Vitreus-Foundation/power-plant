use crate::{Runtime, RuntimeEvent, RuntimeOrigin, OriginCaller, BlockWeights};
use crate::{
    Balances,
};
use crate::{
    DAYS, MINUTES, HOURS, BlockNumber, AccountId, Balance,
};
use crate::prod_or_fast;

use frame_support::traits::LockIdentifier;
use frame_support::traits::fungible::Balanced;
use frame_support::{parameter_types, traits::EitherOfDiverse, weights::Weight, PalletId};
use frame_system::EnsureRoot;
use sp_core::ConstU32;
use sp_runtime::{Permill, Percent, Perbill};


pub use origins::*;


mod origins;


impl origins::pallet_custom_origins::Config for Runtime {}

// parameter_types! {
// 	pub LaunchPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1, "VITREUS_LAUNCH_PERIOD");
// 	pub VotingPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1 * MINUTES, "VITREUS_VOTING_PERIOD");
// 	pub FastTrackVotingPeriod: BlockNumber = prod_or_fast!(3 * HOURS, 1 * MINUTES, "VITREUS_FAST_TRACK_VOTING_PERIOD");
// 	pub const MinimumDeposit: Balance = 1_000 * NANO_VTRS; // 1e-6 VTRS
// 	pub EnactmentPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1, "VITREUS_ENACTMENT_PERIOD");
// 	pub CooloffPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1, "VITREUS_COOLOFF_PERIOD");
// 	pub const InstantAllowed: bool = true;
// 	pub const MaxVotes: u32 = 100;
// 	pub const MaxProposals: u32 = 100;
// }
//
// impl pallet_democracy::Config for Runtime {
// 	type RuntimeEvent = RuntimeEvent;
// 	type Currency = Balances;
// 	type EnactmentPeriod = EnactmentPeriod;
// 	type VoteLockingPeriod = EnactmentPeriod;
// 	type LaunchPeriod = LaunchPeriod;
// 	type VotingPeriod = VotingPeriod;
// 	type MinimumDeposit = MinimumDeposit;
// 	type SubmitOrigin = frame_system::EnsureSigned<AccountId>;
// 	/// A straight majority of the council can decide what their next motion is.
// 	type ExternalOrigin = EitherOfDiverse<
// 		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
// 		EnsureRoot<AccountId>,
// 	>;
// 	/// A 60% super-majority can have the next scheduled referendum be a straight majority-carries vote.
// 	type ExternalMajorityOrigin = EitherOfDiverse<
// 		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
// 		EnsureRoot<AccountId>,
// 	>;
// 	/// A unanimous council can have the next scheduled referendum be a straight default-carries
// 	/// (NTB) vote.
// 	type ExternalDefaultOrigin = EitherOfDiverse<
// 		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
// 		EnsureRoot<AccountId>,
// 	>;
// 	/// Two thirds of the technical committee can have an `ExternalMajority/ExternalDefault` vote
// 	/// be tabled immediately and with a shorter voting/enactment period.
// 	type FastTrackOrigin = EitherOfDiverse<
// 		pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
// 		EnsureRoot<AccountId>,
// 	>;
// 	type InstantOrigin = EitherOfDiverse<
// 		pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
// 		EnsureRoot<AccountId>,
// 	>;
// 	type InstantAllowed = InstantAllowed;
// 	type FastTrackVotingPeriod = FastTrackVotingPeriod;
// 	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
// 	type CancellationOrigin = EitherOfDiverse<
// 		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
// 		EnsureRoot<AccountId>,
// 	>;
// 	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
// 	// Root must agree.
// 	type CancelProposalOrigin = EitherOfDiverse<
// 		pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
// 		EnsureRoot<AccountId>,
// 	>;
// 	type BlacklistOrigin = EnsureRoot<AccountId>;
// 	// Any single technical committee member may veto a coming council proposal, however they can
// 	// only do it once and it lasts only for the cooloff period.
// 	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
// 	type CooloffPeriod = CooloffPeriod;
// 	type Slash = Treasury;
// 	type Scheduler = Scheduler;
// 	type PalletsOrigin = OriginCaller;
// 	type MaxVotes = MaxVotes;
//     // TODO: add weights
// 	type WeightInfo = ();
// 	type MaxProposals = MaxProposals;
// 	type Preimages = Preimage;
// 	type MaxDeposits = ConstU32<100>;
// 	type MaxBlacklisted = ConstU32<100>;
// }
//
// parameter_types! {
// 	pub CouncilMotionDuration: BlockNumber = prod_or_fast!(7 * DAYS, 2 * MINUTES, "VITREUS_MOTION_DURATION");
// 	pub const CouncilMaxProposals: u32 = 100;
// 	pub const CouncilMaxMembers: u32 = 100;
// 	pub MaxProposalWeight: Weight = Perbill::from_percent(50) * BlockWeights::get().max_block;
// }
//
// pub type CouncilCollective = pallet_collective::Instance1;
// impl pallet_collective::Config<CouncilCollective> for Runtime {
// 	type RuntimeOrigin = RuntimeOrigin;
// 	type Proposal = RuntimeCall;
// 	type RuntimeEvent = RuntimeEvent;
// 	type MotionDuration = CouncilMotionDuration;
// 	type MaxProposals = CouncilMaxProposals;
// 	type MaxMembers = CouncilMaxMembers;
// 	type DefaultVote = pallet_collective::PrimeDefaultVote;
// 	type SetMembersOrigin = EnsureRoot<AccountId>;
//     // TODO: add weights
// 	type WeightInfo = ();
// 	type MaxProposalWeight = MaxProposalWeight;
// }
//
// // TODO: investigate coefficients
// pub const fn deposit(items: u32, bytes: u32) -> Balance {
// 		items as Balance * 200 * NANO_VTRS + (bytes as Balance) * 10 * PICO_VTRS
// 	}
//
// parameter_types! {
// 	pub const CandidacyBond: Balance = 1_000 * NANO_VTRS;
// 	// 1 storage item created, key size is 32 bytes, value size is 16+16.
// 	pub const VotingBondBase: Balance = deposit(1, 64);
// 	// additional data per vote is 32 bytes (account id).
// 	pub const VotingBondFactor: Balance = deposit(0, 32);
// 	/// Weekly council elections; scaling up to monthly eventually.
// 	pub TermDuration: BlockNumber = prod_or_fast!(7 * DAYS, 2 * MINUTES, "VITREUS_TERM_DURATION");
// 	/// 13 members initially, to be increased to 23 eventually.
// 	pub const DesiredMembers: u32 = 13;
// 	pub const DesiredRunnersUp: u32 = 20;
// 	pub const MaxVoters: u32 = 10 * 1000;
// 	pub const MaxVotesPerVoter: u32 = 16;
// 	pub const MaxCandidates: u32 = 1000;
// 	pub const PhragmenElectionPalletId: LockIdentifier = *b"phrelect";
// }
// // TODO: add static_assertions package and uncomment
// // Make sure that there are no more than `MaxMembers` members elected via phragmen.
// // const_assert!(DesiredMembers::get() <= CouncilMaxMembers::get());
//
// parameter_types! {
// 	pub const TechnicalMotionDuration: BlockNumber = 7 * DAYS;
// 	pub const TechnicalMaxProposals: u32 = 100;
// 	pub const TechnicalMaxMembers: u32 = 100;
// }
//
// pub type TechnicalCollective = pallet_collective::Instance2;
// impl pallet_collective::Config<TechnicalCollective> for Runtime {
// 	type RuntimeOrigin = RuntimeOrigin;
// 	type Proposal = RuntimeCall;
// 	type RuntimeEvent = RuntimeEvent;
// 	type MotionDuration = TechnicalMotionDuration;
// 	type MaxProposals = TechnicalMaxProposals;
// 	type MaxMembers = TechnicalMaxMembers;
// 	type DefaultVote = pallet_collective::PrimeDefaultVote;
// 	type SetMembersOrigin = EnsureRoot<AccountId>;
// 	type WeightInfo = (); // TODO: make weights
//     type MaxProposalWeight = MaxProposalWeight;
// }
//
// impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
// 	type RuntimeEvent = RuntimeEvent;
// 	type AddOrigin = EnsureRoot<AccountId>;
// 	type RemoveOrigin = EnsureRoot<AccountId>;
// 	type SwapOrigin = EnsureRoot<AccountId>;
// 	type ResetOrigin = EnsureRoot<AccountId>;
// 	type PrimeOrigin = EnsureRoot<AccountId>;
// 	type MembershipInitialized = TechnicalCommittee;
// 	type MembershipChanged = TechnicalCommittee;
// 	type MaxMembers = TechnicalMaxMembers;
// 	type WeightInfo = (); // TODO: make weights
// }
//
// parameter_types! {
// 	pub const ProposalBond: Permill = Permill::from_percent(5);
// 	pub const ProposalBondMinimum: Balance = 1_000 * NANO_VTRS;
// 	pub const ProposalBondMaximum: Balance = 5_000 * NANO_VTRS;
// 	pub const SpendPeriod: BlockNumber = 24 * DAYS;
// 	pub const Burn: Permill = Permill::from_percent(1);
// 	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry"); // TODO: change
//
// 	pub const TipCountdown: BlockNumber = 1 * DAYS;
// 	pub const TipFindersFee: Percent = Percent::from_percent(20);
// 	pub const TipReportDepositBase: Balance = 10 * NANO_VTRS;
// 	pub const DataDepositPerByte: Balance = 100 * PICO_VTRS;
// 	pub const MaxApprovals: u32 = 100;
// 	pub const MaxAuthorities: u32 = 100_000;
// 	pub const MaxKeys: u32 = 10_000;
// 	pub const MaxPeerInHeartbeats: u32 = 10_000;
// 	pub const RootSpendOriginMaxAmount: Balance = Balance::MAX;
// 	pub const CouncilSpendOriginMaxAmount: Balance = Balance::MAX;
// }
//
// type TreasuryBalance = pallet_treasury::BalanceOf<<Runtime as pallet_treasury::Config>, ()>;
// type TreasuryPositiveImbalace = pallet_treasury::PositiveImbalanceOf<<Runtime as pallet_treasury::Config>, ()>;
// impl pallet_treasury::SpendFunds<<Runtime as pallet_treasury::Config>, ()> for () {
// 	fn spend_funds(
// 		_budget_remaining: &mut TreasuryBalance,
// 		_imbalance: &mut TreasuryPositiveImbalace,
// 		_total_weight: &mut Weight,
// 		_missed_any: &mut bool,
// 	) { }
// }
//
// impl pallet_treasury::Config for Runtime {
// 	type PalletId = TreasuryPalletId;
// 	type Currency = Balances;
// 	type ApproveOrigin = EnsureRoot<AccountId>;
// 	type RejectOrigin = EnsureRoot<AccountId>;
// 	type RuntimeEvent = RuntimeEvent;
// 	type OnSlash = Treasury;
// 	type ProposalBond = ProposalBond;
// 	type ProposalBondMinimum = ProposalBondMinimum;
// 	type ProposalBondMaximum = ProposalBondMaximum;
// 	type SpendPeriod = SpendPeriod;
// 	type Burn = Burn;
// 	type BurnDestination = (); // TODO: investigate bounties pallet
// 	type SpendFunds = ();
// 	type MaxApprovals = MaxApprovals;
// 	type WeightInfo = (); // TODO: add weights
//     // TODO: investigate whether we need custom origin like in polkadot
// 	type SpendOrigin = EnsureRoot<AccountId>;
// }
