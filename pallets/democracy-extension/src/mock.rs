use super::*;
use crate as pallet_democracy_extension;
use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstBool, ConstU32, ConstU64, EqualPrivilegeOnly},
    weights::Weight,
};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_runtime::{BuildStorage, Perbill};

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Scheduler: pallet_scheduler,
        Democracy: pallet_democracy,
        DemocracyExtension: pallet_democracy_extension,
    }
);

parameter_types! {
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(
            Weight::from_parts(frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
        );
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}
parameter_types! {
    pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

impl pallet_scheduler::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = MaximumSchedulerWeight;
    type ScheduleOrigin = EnsureRoot<u64>;
    type MaxScheduledPerBlock = ConstU32<100>;
    type WeightInfo = ();
    type OriginPrivilegeCmp = EqualPrivilegeOnly;
    type Preimages = ();
}

impl pallet_democracy::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = DemocracyExtension;
    type EnactmentPeriod = ConstU64<2>;
    type LaunchPeriod = ConstU64<2>;
    type VotingPeriod = ConstU64<2>;
    type VoteLockingPeriod = ConstU64<3>;
    type FastTrackVotingPeriod = ConstU64<2>;
    type MinimumDeposit = ConstU64<1>;
    type MaxDeposits = ConstU32<1000>;
    type MaxBlacklisted = ConstU32<5>;
    type SubmitOrigin = EnsureSigned<Self::AccountId>;
    type ExternalOrigin = EnsureSigned<Self::AccountId>;
    type ExternalMajorityOrigin = EnsureSigned<Self::AccountId>;
    type ExternalDefaultOrigin = EnsureSigned<Self::AccountId>;
    type FastTrackOrigin = EnsureSigned<Self::AccountId>;
    type CancellationOrigin = EnsureSigned<Self::AccountId>;
    type BlacklistOrigin = EnsureRoot<u64>;
    type CancelProposalOrigin = EnsureRoot<u64>;
    type VetoOrigin = EnsureSigned<Self::AccountId>;
    type CooloffPeriod = ConstU64<2>;
    type Slash = ();
    type InstantOrigin = EnsureSigned<Self::AccountId>;
    type InstantAllowed = ConstBool<false>;
    type Scheduler = Scheduler;
    type MaxVotes = ConstU32<100>;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = ();
    type MaxProposals = ConstU32<100>;
    type Preimages = ();
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<Self::AccountId>;
    type Currency = Balances;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    pallet_balances::GenesisConfig::<Test> { balances: vec![(1, 2000), (2, 3000)] }
        .assimilate_storage(&mut t)
        .unwrap();

    pallet_democracy::GenesisConfig::<Test>::default()
        .assimilate_storage(&mut t)
        .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
