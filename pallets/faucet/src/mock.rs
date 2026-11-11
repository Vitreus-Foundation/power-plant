use crate as pallet_faucet;
use frame_support::traits::{ConstU16, ConstU32, ConstU64};
use frame_support::{derive_impl, parameter_types};
use frame_system::pallet_prelude::*;
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type AccountId = u32;
pub(crate) type Balance = u32;

// minutes * seconds / 6 seconds per block
pub const BLOCKS_PER_HOUR: BlockNumberFor<Test> = 60 * 60 / 6;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Faucet: pallet_faucet,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type BlockHashCount = ConstU64<250>;
    type AccountData = pallet_balances::AccountData<Balance>;
    type SS58Prefix = ConstU16<42>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<1024>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type ExistentialDeposit = ConstU32<1>;
    type AccountStore = System;
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type RuntimeHoldReason = ();
}

parameter_types! {
    pub const AccumulationPeriod: BlockNumberFor<Test> = BLOCKS_PER_HOUR * 24;
    pub const MaxAmount: Balance = 100;
}

impl pallet_faucet::Config for Test {
    type AccumulationPeriod = AccumulationPeriod;
    type MaxAmount = MaxAmount;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}
