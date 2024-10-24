//!
//! # Module Overview
//!
//! This module defines a mock runtime for testing the faucet pallet in a Substrate-based blockchain.
//! It provides the necessary configurations, types, and parameters to create a controlled environment
//! for simulating the behavior of the faucet, including requesting funds, managing accounts, and
//! interacting with the balances pallet. This mock setup is used for both unit tests and benchmarking
//! to ensure that the faucet behaves correctly in various scenarios.
//!
//! # Key Features and Components
//!
//! - **Mock Runtime Definition**:
//!   - The `construct_runtime!` macro is used to create a mock runtime (`Test`) that includes the
//!     `frame_system`, `pallet_balances`, and `pallet_faucet`. This mock runtime allows developers
//!     to simulate the blockchain environment needed to test the faucet pallet.
//!
//! - **Key Types and Constants**:
//!   - **AccountId**: Defined as `u32` for simplicity in testing. This type represents user accounts
//!     within the mock runtime.
//!   - **Balance**: Also defined as `u32`, it represents token balances, allowing for easy setup and
//!     manipulation of account balances during tests.
//!   - **BLOCKS_PER_HOUR**: A constant used to calculate the number of blocks representing one hour,
//!     assuming a block time of 6 seconds. This value is used to define the accumulation period for
//!     the faucet, which limits the frequency of fund requests.
//!
//! - **Configuration for System and Balances Pallets**:
//!   - **System Configuration**: The mock runtime configuration for the `frame_system` pallet includes
//!     various types, such as `AccountId`, `Nonce`, and `Hash`. The `BaseCallFilter` is set to `Everything`,
//!     allowing all calls to be executed without restrictions, which is suitable for testing purposes.
//!   - **Balances Configuration**: The `pallet_balances` configuration includes settings for `MaxLocks`,
//!     `Balance`, and `ExistentialDeposit`. The `ExistentialDeposit` is set to `1`, ensuring that accounts
//!     remain active as long as they have a balance greater than or equal to `1`.
//!
//! - **Faucet Configuration**:
//!   - The faucet pallet is configured with parameters like `AccumulationPeriod` (24 hours) and `MaxAmount`
//!     (100 tokens). These parameters define the maximum amount of funds that can be requested and the
//!     interval between requests, ensuring that the faucet is used responsibly during tests.
//!
//! - **Genesis Storage Setup**:
//!   - `new_test_ext()`: This function creates an instance of `sp_io::TestExternalities`, which represents
//!     the genesis state of the blockchain. It is used to initialize the testing environment, ensuring
//!     that each test starts with a clean and consistent state.
//!
//! # Access Control and Security
//!
//! - **Testing Only**: This module is intended strictly for testing purposes. It provides unrestricted
//!   access to modify the runtime state, which would not be suitable for a production environment.
//!   The `BaseCallFilter` is set to allow all calls, and the existential deposit is minimal to simplify
//!   account creation and testing.
//! - **Controlled Environment**: The mock runtime provides a controlled environment where developers
//!   can test the faucet without the risks associated with real blockchain interactions, such as
//!   unauthorized access or token misuse.
//!
//! # Developer Notes
//!
//! - **Flexible Testing Configuration**: The mock runtime setup provides flexibility for developers
//!   to adjust parameters like `MaxAmount` and `AccumulationPeriod`. This allows for testing different
//!   scenarios, such as increased fund limits or shorter accumulation periods, to evaluate the behavior
//!   of the faucet under different conditions.
//! - **Simple Account and Balance Types**: Using `u32` for `AccountId` and `Balance` simplifies the
//!   process of creating and managing accounts in tests, reducing complexity while ensuring that all
//!   key functionalities are covered.
//! - **Genesis State Consistency**: The use of `new_test_ext()` ensures that each test begins with
//!   the same genesis state, providing consistency across test cases and making it easier to reproduce
//!   and diagnose issues.
//!
//! # Usage Scenarios
//!
//! - **Unit Testing the Faucet Pallet**: This mock runtime is used to write unit tests for the faucet
//!   pallet, allowing developers to verify that requests for funds are processed correctly, that the
//!   accumulation period is enforced, and that users cannot request more than the allowed amount.
//! - **Benchmarking Faucet Functions**: The mock runtime is also used for benchmarking to measure the
//!   performance of the `request_funds` extrinsic. By providing a controlled environment, developers
//!   can accurately determine the computational cost of the faucet operations.
//! - **Simulating Edge Cases**: The mock setup allows developers to simulate various edge cases, such
//!   as multiple users requesting funds simultaneously, exceeding the maximum allowed amount, or
//!   requesting funds at the boundary of the accumulation period. These tests help ensure that the
//!   faucet behaves as expected under all possible conditions.
//!
//! # Integration Considerations
//!
//! - **Parameter Adjustments**: Developers integrating the faucet pallet into their own blockchains
//!   should adjust the parameters (`MaxAmount`, `AccumulationPeriod`, etc.) to align with their specific
//!   requirements. The values used in the mock runtime are intended for testing and may not be suitable
//!   for production use.
//! - **Compatibility with Balances Pallet**: The faucet relies on the `pallet_balances` module to
//!   distribute funds. Proper integration requires ensuring that the balances pallet is configured
//!   correctly and that all dependencies are met in the runtime.
//! - **Testing and Validation**: Before deploying the faucet pallet in a live environment, developers
//!   should thoroughly test all scenarios using the mock runtime. This helps identify any potential
//!   vulnerabilities or misconfigurations that could lead to abuse or unintended behavior in production.
//!
//! # Example Scenario
//!
//! Suppose a developer needs to verify that the faucet pallet correctly enforces the accumulation period,
//! preventing users from requesting funds more than once in a 24-hour period. Using the `new_test_ext()`
//! function, the developer can create a clean test environment and simulate multiple fund requests from
//! the same account. By checking the state of the `Requests` storage and the emitted events, the developer
//! can confirm that the accumulation period is respected, ensuring the correct behavior of the faucet.
//!


use crate as pallet_faucet;
use frame_support::traits::{ConstU16, ConstU32, ConstU64, Everything};
use frame_support::{derive_impl, parameter_types};
use frame_system::pallet_prelude::*;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;
pub(crate) type AccountId = u32;
pub(crate) type Balance = u32;
type Nonce = u32;

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
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = Nonce;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
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
    type MaxLocks = ConstU32<1024>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU32<1>;
    type AccountStore = System;
    type WeightInfo = ();
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
