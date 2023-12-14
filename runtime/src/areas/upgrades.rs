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

use crate::areas::governance::CouncilCollective;

parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
        BlockWeights::get().max_block;
    pub const MaxScheduledPerBlock: u32 = 50;
    pub const NoPreimagePostponement: Option<u32> = Some(10);
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
	fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
		if left == right {
			return Some(Ordering::Equal)
		}

		match (left, right) {
			// Root is greater than anything.
			(OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
			// Check which one has more yes votes.
			(
				OriginCaller::Council(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
				OriginCaller::Council(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
			) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
			// For every other origin we don't care, as they are not used for `ScheduleOrigin`.
			_ => None,
		}
	}
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
    // TODO: add weights
    type WeightInfo = ();
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
    type Preimages = Preimage;
}

pub const fn deposit(items: u32, bytes: u32) -> Balance {
    items as Balance * 200 * NANO_VTRS + (bytes as Balance) * 1 * PICO_VTRS
}

parameter_types! {
    pub const PreimageMaxSize: u32 = 4096 * 1024;
    pub const PreimageBaseDeposit: Balance = deposit(2, 64);
    pub const PreimageByteDeposit: Balance = deposit(0, 1);
}

impl pallet_preimage::Config for Runtime {
    // TODO: add weights
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type BaseDeposit = PreimageBaseDeposit;
    type ByteDeposit = PreimageByteDeposit;
}
