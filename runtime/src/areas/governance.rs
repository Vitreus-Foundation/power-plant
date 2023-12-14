use core::cmp::Ordering;

use crate::prod_or_fast;
use crate::{
    AccountId, Balance, Balances, Preimage, BlockNumber, BlockWeights, OriginCaller,
    Runtime, RuntimeEvent, RuntimeOrigin, RuntimeCall, DAYS, HOURS, MINUTES, NANO_VTRS, PICO_VTRS,
};

use frame_support::traits::fungible::Balanced;
use frame_support::traits::{EitherOf, EqualPrivilegeOnly, LockIdentifier, PrivilegeCmp};
use frame_support::{parameter_types, traits::EitherOfDiverse, weights::Weight, PalletId};
use frame_system::EnsureRoot;
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
