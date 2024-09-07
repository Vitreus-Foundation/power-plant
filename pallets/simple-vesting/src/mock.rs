use crate as pallet_simple_vesting;
use frame_support::traits::{ConstU16, ConstU32, ConstU64, Everything};
use sp_core::H256;
use sp_runtime::traits::Identity;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
type AccountId = u32;
type Balance = u64;
type Nonce = u32;

pub(crate) const ED: Balance = 100;
pub(crate) const ALICE: AccountId = 1;
pub(crate) const BOB: AccountId = 2;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        SimpleVesting: pallet_simple_vesting,
    }
);

impl frame_system::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type BaseCallFilter = Everything;
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
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU64<ED>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxLocks = ConstU32<1024>;
    type MaxReserves = ConstU32<1024>;
    type MaxHolds = ();
    type MaxFreezes = ();
}

impl pallet_simple_vesting::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = Identity;
    type Slash = ();
}

// Build genesis storage according to the mock runtime.
pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> { balances: vec![(ALICE, 100 * ED)] }
        .assimilate_storage(&mut t)
        .unwrap();
    pallet_simple_vesting::GenesisConfig::<Test> { vesting: vec![] }
        .assimilate_storage(&mut t)
        .unwrap();

    t.into()
}
