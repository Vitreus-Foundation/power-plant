// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//!
//! # Module Overview
//!
//! This module defines the mock runtime for testing the `pallet_nac_managing` in a Substrate-based
//! blockchain. The mock runtime is used to simulate the blockchain environment, allowing developers
//! to create unit tests for the NAC (Nonfungible Asset Certificate) management pallet, which involves
//! NFTs, reputation, and asset handling. By using this controlled environment, developers can validate
//! the behavior of the pallet under different scenarios, ensuring that all functionalities are tested
//! comprehensively.
//!
//! # Key Features and Components
//!
//! - **Mock Runtime Setup**:
//!   - **Constructed Runtime**: The mock runtime (`Test`) is constructed using the `construct_runtime!`
//!     macro, which includes various pallets necessary for testing, such as `frame_system`,
//!     `pallet_balances`, `pallet_assets`, `pallet_nfts`, `pallet_reputation`, and `pallet_nac_managing`.
//!   - **Key Types**: The mock runtime uses simplified data types for easy testing. For example,
//!     `AccountId` and `Balance` are defined as `u64`, providing a straightforward way to handle user
//!     accounts and balances during tests.
//!   - **Initial Configuration**: The constants `INIT_TIMESTAMP` and `BLOCK_TIME` are defined to simulate
//!     a blockchain environment with specific initial settings. This helps ensure consistency across
//!     tests that involve time-dependent behavior.
//!
//! - **Pallet Configuration**:
//!   - **Balances Pallet**: Implements `pallet_balances::Config` for handling currency-related operations,
//!     such as account balances and dust removal. The `ExistentialDeposit` is set to `1` to ensure
//!     accounts remain active as long as they maintain a minimal balance.
//!   - **Assets Pallet**: Configures `pallet_assets::Config` to manage assets, including defining parameters
//!     for deposits (`AssetDeposit`, `AssetAccountDeposit`, `ApprovalDeposit`), metadata storage costs,
//!     and account approval limits.
//!   - **Reputation Pallet**: Implements `pallet_reputation::Config` to manage user reputation, which
//!     integrates with the NAC management system.
//!
//! - **Genesis Storage Setup**:
//!   - **`new_test_ext()` Function**: This function initializes the genesis state for the mock runtime,
//!     providing a clean and consistent starting point for each test. It uses `sp_io::TestExternalities`
//!     to create an in-memory blockchain state, ensuring that tests do not interfere with each other.
//!
//! # Access Control and Security
//!
//! - **Testing Purposes Only**: The mock runtime defined in this module is strictly for testing. It
//!   provides full control over the blockchain state, allowing unrestricted modifications to simulate
//!   various conditions. This level of access would not be suitable for a production environment.
//! - **Root and Signed Origins**: The configuration for the assets pallet (`EnsureRoot` and `EnsureSigned`)
//!   ensures that certain actions, such as creating or managing assets, are limited to authorized
//!   users during testing. This helps simulate the access control that would be present in a live
//!   blockchain environment.
//!
//! # Developer Notes
//!
//! - **Flexible Pallet Integration**: The mock runtime integrates several pallets (`pallet_assets`,
//!   `pallet_nfts`, `pallet_reputation`) alongside `pallet_nac_managing`. This setup provides a
//!   realistic environment where multiple pallets interact, allowing comprehensive testing of
//!   interdependencies and combined functionalities.
//! - **Simplified Account Handling**: Using `u64` for `AccountId` and `Balance` simplifies account
//!   management in tests, making it easier to write, maintain, and understand the unit tests. This
//!   choice is ideal for testing purposes but should be adjusted for production scenarios.
//! - **Default Genesis State**: The use of `frame_system::GenesisConfig::default()` ensures that the
//!   blockchain state starts in a consistent default configuration. This helps prevent issues that
//!   could arise from uninitialized or inconsistent states during testing.
//!
//! # Usage Scenarios
//!
//! - **Unit Testing NAC Management**: This mock runtime is used for unit tests that verify the core
//!   functionalities of the `pallet_nac_managing`. For instance, developers can test minting NFTs,
//!   updating NAC levels, or verifying that reputation adjustments occur correctly based on user
//!   actions.
//! - **Benchmarking and Performance Testing**: The controlled environment provided by the mock runtime
//!   allows for benchmarking and performance testing of the NAC management pallet. Developers can use
//!   this setup to measure the computational load of different extrinsics and optimize performance.
//! - **Simulating Edge Cases**: By using `new_test_ext()`, developers can create custom blockchain
//!   states that simulate edge cases, such as a large number of assets, rapid NFT minting, or complex
//!   interactions between different pallets. This helps ensure that the pallet can handle unexpected
//!   or extreme conditions without issues.
//!
//! # Integration Considerations
//!
//! - **Testing Multiple Pallets Together**: Since the `pallet_nac_managing` interacts with other
//!   pallets like `pallet_assets` and `pallet_reputation`, developers should consider the integration
//!   between these pallets when writing tests. Proper integration ensures that all pallets work
//!   harmoniously, especially in scenarios involving cross-pallet functionality.
//! - **Parameter Adjustments for Realistic Testing**: Parameters such as `AssetDeposit`, `ApprovalDeposit`,
//!   and `ExistentialDeposit` are set to zero or minimal values for testing convenience. When deploying
//!   in production, these parameters should be configured to reflect the economic and security
//!   requirements of the network.
//! - **Role-Based Access Control Testing**: The mock runtime simulates both root and signed origins
//!   (`EnsureRoot` and `EnsureSigned`). Developers should test various role-based scenarios to ensure
//!   that unauthorized users cannot perform restricted actions, which helps prevent misuse in a live
//!   environment.
//!
//! # Example Scenario
//!
//! Suppose a developer needs to verify that a user can mint a new NFT through the NAC management
//! pallet only if they have sufficient permissions and meet the asset requirements. Using `new_test_ext()`,
//! the developer can create a clean testing environment, set up an authorized account (`EnsureRoot`),
//! and simulate the minting process. By checking the resulting events and changes in the on-chain state,
//! the developer can confirm that the pallet correctly enforces access controls, asset requirements,
//! and interaction with other pallets like `pallet_assets` and `pallet_nfts`.
//!


use crate::{self as pallet_nac_managing, *};

use frame_support::{
    derive_impl, parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU32, ConstU64},
    weights::constants::RocksDbWeight,
};
use frame_system::{EnsureRoot, EnsureSigned};
use parity_scale_codec::Compact;
use sp_core::H256;
use sp_runtime::traits::BlakeTwo256;
use sp_runtime::{
    testing::{TestSignature, UintAuthorityId},
    traits::IdentityLookup,
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;

/// The AccountId alias in this test module.
pub(crate) type AccountId = u64;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u64;
pub(crate) type Signature = TestSignature;
pub(crate) type AccountPublic = UintAuthorityId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        Nfts: pallet_nfts,
        Reputation: pallet_reputation,
        NacManaging: pallet_nac_managing,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type BaseCallFilter = frame_support::traits::Everything;
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
    type DbWeight = RocksDbWeight;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = NacManaging;
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

parameter_types! {
    pub TestCollectionDeposit:  u64 = 0;
    pub TestItemDeposit:  u64 = 0;
}

pub type CollectionId = u32;
pub type ItemId = u32;

impl pallet_nfts::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type CollectionDeposit = TestCollectionDeposit;
    type ApprovalsLimit = ();
    type ItemAttributesApprovalsLimit = ();
    type MaxTips = ();
    type MaxDeadlineDuration = ();
    type MaxAttributesPerCall = ();
    type Features = ();
    type OffchainPublic = AccountPublic;
    type OffchainSignature = Signature;
    type ItemDeposit = TestItemDeposit;
    type MetadataDepositBase = ConstU64<1>;
    type AttributeDepositBase = ConstU64<1>;
    type DepositPerByte = ConstU64<1>;
    type StringLimit = ConstU32<50>;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = ();
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<Self::AccountId>>;
    type Locker = ();
}

parameter_types! {
    pub const NftCollectionId: CollectionId = 0;
    pub const VIPPCollectionId: CollectionId = 1;
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type Nfts = Nfts;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type NftCollectionId = NftCollectionId;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
    type Currency = Balances;
    type VIPPCollectionId = VIPPCollectionId;
    type OnVIPPChanged = ();
}

parameter_types! {
    pub const AssetDeposit: Balance = 0;
    pub const AssetAccountDeposit: Balance = 0;
    pub const ApprovalDeposit: Balance = 0;
    pub const AssetsStringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 0;
    pub const MetadataDepositPerByte: Balance = 0;
}

pub type AssetId = u32;

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type Currency = Balances;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = AssetsStringLimit;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetIdParameter = Compact<AssetId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type CallbackHandle = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

impl pallet_reputation::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU64<1>;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type MaxFreezes = ();
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
