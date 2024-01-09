use crate as pallet_treasury_extension;

use frame_support::PalletId;
use frame_support::{
    parameter_types,
    traits::{ConstU128, ConstU32, ConstU64, Everything},
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};

use sp_core::H256;

use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, Permill,
};

// 2 gVTRS
const VTRS_INITIAL_BALANCE: u128 = 2_000_000_000_000_000_000_000_000_000;

type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) type AccountId = u64;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u128;
pub(crate) type BlockNumber = u64;

pub(crate) const ALICE: AccountId = 1;
pub(crate) const BOB: AccountId = 2;

pub(crate) const TREASURY: u64 = 1;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Bounties: pallet_bounties,
        Treasury: pallet_treasury,
        TreasuryExtension: pallet_treasury_extension,
    }
);

impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = Nonce;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
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

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<1024>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type MaxHolds = ();
    type RuntimeHoldReason = ();
}

parameter_types! {
    pub const ProposalBond: Permill = Permill::from_percent(5);
    pub const ProposalBondMinimum: Balance = Balance::MIN;
    pub const ProposalBondMaximum: Balance = Balance::MAX;
    pub const SpendPeriod: BlockNumber = 10;
    pub const Burn: Permill = Permill::from_percent(1);
    pub const TreasuryPalletId: PalletId = PalletId(TREASURY.to_le_bytes());

    pub const DataDepositPerByte: Balance = 1;
    pub const MaxApprovals: u32 = 100;
    pub const MaxAuthorities: u32 = 100_000;
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
    pub const RootSpendOriginMaxAmount: Balance = Balance::MAX;
    pub const CouncilSpendOriginMaxAmount: Balance = Balance::MAX;
}

impl pallet_treasury::Config for Test {
    type PalletId = TreasuryPalletId;
    type Currency = Balances;
    type ApproveOrigin = EnsureRoot<AccountId>;
    type RejectOrigin = EnsureRoot<AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type OnSlash = Treasury;
    type ProposalBond = ProposalBond;
    type ProposalBondMinimum = ProposalBondMinimum;
    type ProposalBondMaximum = ProposalBondMaximum;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = ();
    type SpendFunds = (Bounties, TreasuryExtension);
    type MaxApprovals = MaxApprovals;
    type WeightInfo = ();
    type SpendOrigin = EnsureRootWithSuccess<AccountId, ConstU128<{ Balance::MAX }>>;
}

parameter_types! {
    pub const BountyDepositBase: Balance = 10;
    pub const BountyDepositPayoutDelay: BlockNumber = 8;
    pub const BountyUpdatePeriod: BlockNumber = 90;
    pub const MaximumReasonLength: u32 = 16384;
    pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
    pub const CuratorDepositMin: Balance = 100_000;
    pub const CuratorDepositMax: Balance = 100_000_000;
    pub const BountyValueMinimum: Balance = 100;
}

impl pallet_bounties::Config for Test {
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
    type WeightInfo = ();
}

parameter_types! {
    pub const SpendThreshold: Permill = Permill::from_percent(50);
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type SpendThreshold = SpendThreshold;
    type OnRecycled = ();
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, VTRS_INITIAL_BALANCE),
            (BOB, VTRS_INITIAL_BALANCE),
            (Treasury::account_id(), 1000),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
