//! Mock Runtime for Reputation Pallet Testing
//!
//! This module provides a mock runtime environment for testing the Reputation pallet.
//! It includes the necessary configurations, types, and dependencies required to simulate a Substrate-based blockchain runtime, enabling developers to write unit tests for the pallet.
//!
//! # Features
//! - Defines a mock runtime using the `construct_runtime!` macro for testing purposes.
//! - Includes core components like the System pallet to enable a functional blockchain environment.
//! - Uses type aliases to simplify key elements such as blocks and transaction indexes (Nonces).
//!
//! # Structure
//! - Configures the `Test` runtime that includes `System` and `ReputationPallet`.
//! - Defines the necessary constants and traits for the mock runtime, ensuring it behaves consistently with expected blockchain logic.
//! - Utilizes `BlakeTwo256` as the hashing algorithm and `IdentityLookup` for account resolution.
//!
//! # Usage
//! - Import this mock runtime to test the behavior of the Reputation pallet in isolation.
//! - Write unit tests that target the `Test` runtime to validate the pallet's functionality and ensure it handles different scenarios correctly.
//!
//! # Dependencies
//! - Relies on FRAME support for system-level configuration, including `frame_system` and other fundamental pallets.
//! - Uses `sp_runtime` and `sp_core` for essential runtime traits and hashing utilities.
//! - Defines constants like `ConstU16` and `ConstU64` to configure runtime parameters.
//!
//! # Important Notes
//! - The mock runtime is designed specifically for testing and should not be used in production.
//! - Ensure to expand or modify the mock configuration if additional pallets or functionality need to be tested.


use crate as pallet_reputation;
use frame_support::{
    derive_impl,
    traits::{ConstU16, ConstU64},
};
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
/// Index of a transaction in the chain.
pub type Nonce = u32;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        ReputationPallet: pallet_reputation,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type Nonce = Nonce;
    type Block = Block;
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_reputation::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}
