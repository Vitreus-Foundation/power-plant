//! Mock Runtime for Simple Vesting Pallet Testing
//!
//! This module provides a mock runtime environment specifically for testing the Simple Vesting pallet.
//! It sets up the necessary configurations, types, and constants required to simulate a blockchain runtime for unit testing purposes.
//!
//! # Features
//! - Defines a mock runtime using the `construct_runtime!` macro to include core components like System and the Simple Vesting pallet.
//! - Configures basic runtime types such as `AccountId`, `Balance`, and `Nonce` to simulate account behavior.
//! - Provides constants like `ALICE`, `BOB`, and `ED` to facilitate common testing scenarios.
//!
//! # Structure
//! - Sets up the `Test` runtime that includes the `System` pallet along with the `SimpleVestingPallet`.
//! - Defines key type aliases to match the expected types used in the pallet, ensuring compatibility during tests.
//! - Uses `BlakeTwo256` for hashing and `IdentityLookup` for resolving account identities.
//!
//! # Usage
//! - Import this mock runtime in your unit tests to validate the functionality of the Simple Vesting pallet.
//! - Write test cases that target specific vesting scenarios, such as account setup, vesting progression, and balance checks.
//! - Utilize constants like `ALICE` and `BOB` to create consistent test cases.
//!
//! # Dependencies
//! - Uses `frame_support` and `frame_system` for core blockchain logic and support utilities.
//! - Relies on `sp_runtime` and `sp_core` for runtime traits, hashing, and building storage configurations.
//!
//! # Important Notes
//! - This mock runtime is intended for testing only and should not be used in production environments.
//! - It allows developers to simulate different scenarios and behaviors, ensuring the vesting logic functions correctly in a controlled setup.
//! - Expand the mock runtime as needed to include additional pallets or to accommodate more complex testing requirements.


use crate as pallet_simple_vesting;
use frame_support::{
    derive_impl,
    traits::{ConstU16, ConstU32, ConstU64, Everything},
};
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

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
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

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
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
