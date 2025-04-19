use frame_support::traits::LockIdentifier;
use static_assertions::const_assert;

use super::*;

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
    type MaxMembers = TechnicalMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = pallet_collective::weights::SubstrateWeight<Runtime>;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type MaxProposalWeight = MaxProposalWeight;
}

parameter_types! {
    pub const CandidacyBond: Balance = 1 * UNITS;
    // 1 storage item created, key size is 32 bytes, value size is 16+16.
    pub const VotingBondBase: Balance = deposit(1, 64);
    // additional data per vote is 32 bytes (account id).
    pub const VotingBondFactor: Balance = deposit(0, 32);
    pub const DesiredMembers: u32 = 7;
    pub const DesiredRunnersUp: u32 = 7;
    // TODO: remove `storage` after April of 2025
    pub storage TermDuration: BlockNumber = prod_or_fast!(6 * MONTHS, 10 * MINUTES);
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
    type InitializeMembers = ();
    type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
    type CandidacyBond = CandidacyBond;
    type VotingBondBase = VotingBondBase;
    type VotingBondFactor = VotingBondFactor;
    type LoserCandidate = Treasury;
    type KickedMember = Treasury;
    type DesiredMembers = DesiredMembers;
    type DesiredRunnersUp = DesiredRunnersUp;
    type TermDuration = TermDuration;
    type MaxCandidates = MaxCandidates;
    type MaxVoters = MaxVoters;
    type MaxVotesPerVoter = MaxVotesPerVoter;
    type WeightInfo = pallet_elections_phragmen::weights::SubstrateWeight<Runtime>;
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
