// This file is part of Substrate.

// Copyright (C) 2019-2022 Parity Technologies (UK) Ltd.
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

//! # Asset Conversion Mock Test Environment
//!
//! Test configuration and mock runtime for the Asset Conversion (AMM) pallet.
//!
//! ## Runtime Configuration
//!
//! ### Core Components
//! - System: Basic substrate framework
//! - Balances: Native currency handling
//! - Assets: Non-native token management (Instance1)
//! - PoolAssets: LP token management (Instance2)
//! - AssetConversion: Main AMM functionality
//!
//! ### Key Parameters
//! - PalletId: py/ascon
//! - LP Fee: 2%
//! - Pool Setup Fee: 100 units
//! - Min Liquidity: 100 units
//! - Max Swap Path: 4 hops
//! - Withdrawal Fee: Configurable (0% default)
//!
//! ### Test Accounts
//! Pre-funded accounts for testing:
//! - Account 1: 10,000 units
//! - Account 2: 20,000 units
//! - Account 3: 30,000 units
//! - Account 4: 40,000 units
//!
//! ### Asset Rates
//! Implements fixed 1:2 conversion ratio between native/non-native assets for
//! predictable test scenarios.
//!
//! ## Usage
//! ```rust
//! let mut ext = new_test_ext();
//! ext.execute_with(|| {
//!     // Test code here
//! });
//! ```

use super::*;
use crate as pallet_asset_conversion;

use frame_support::{
    construct_runtime, derive_impl,
    instances::{Instance1, Instance2},
    ord_parameter_types, parameter_types,
    traits::{
        tokens::{ConversionFromAssetBalance, ConversionToAssetBalance},
        AsEnsureOriginWithArg, ConstU128, ConstU32,
    },
    PalletId,
};
use frame_system::{EnsureSigned, EnsureSignedBy};
use sp_arithmetic::{FixedPointNumber, FixedU128, Permill};
use sp_runtime::{
    traits::{AccountIdConversion, IdentityLookup},
    BuildStorage,
};
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets::<Instance1>,
        PoolAssets: pallet_assets::<Instance2>,
        AssetConversion: pallet_asset_conversion,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type AccountId = u128;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type ExistentialDeposit = ConstU128<10>;
    type AccountStore = System;
}

impl pallet_assets::Config<Instance1> for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetId = u32;
    type AssetIdParameter = u32;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type AssetDeposit = ConstU128<1>;
    type AssetAccountDeposit = ConstU128<10>;
    type MetadataDepositBase = ConstU128<1>;
    type MetadataDepositPerByte = ConstU128<1>;
    type ApprovalDeposit = ConstU128<1>;
    type StringLimit = ConstU32<50>;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type CallbackHandle = ();
    pallet_assets::runtime_benchmarks_enabled! {
        type BenchmarkHelper = ();
    }
}

impl pallet_assets::Config<Instance2> for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = u128;
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetId = u32;
    type AssetIdParameter = u32;
    type Currency = Balances;
    type CreateOrigin =
        AsEnsureOriginWithArg<EnsureSignedBy<AssetConversionOrigin, Self::AccountId>>;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type AssetDeposit = ConstU128<0>;
    type AssetAccountDeposit = ConstU128<0>;
    type MetadataDepositBase = ConstU128<0>;
    type MetadataDepositPerByte = ConstU128<0>;
    type ApprovalDeposit = ConstU128<0>;
    type StringLimit = ConstU32<50>;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type CallbackHandle = ();
    pallet_assets::runtime_benchmarks_enabled! {
        type BenchmarkHelper = ();
    }
}

parameter_types! {
    pub const AssetConversionPalletId: PalletId = PalletId(*b"py/ascon");
    pub storage AllowMultiAssetPools: bool = true;
    pub storage LiquidityWithdrawalFee: Permill = Permill::from_percent(0); // should be non-zero if AllowMultiAssetPools is true, otherwise can be zero
}

ord_parameter_types! {
    pub const AssetConversionOrigin: u128 = AccountIdConversion::<u128>::into_account_truncating(&AssetConversionPalletId::get());
}

pub struct AssetRate;
impl AssetRate {
    const RATE: FixedU128 = FixedU128::from_rational(1, 2);
}

impl ConversionFromAssetBalance<u128, u32, u128> for AssetRate {
    type Error = DispatchError;

    fn from_asset_balance(balance: u128, _asset_id: u32) -> Result<u128, Self::Error> {
        Ok(Self::RATE.saturating_mul_int(balance))
    }
}

impl ConversionToAssetBalance<u128, u32, u128> for AssetRate {
    type Error = DispatchError;

    fn to_asset_balance(balance: u128, _asset_id: u32) -> Result<u128, Self::Error> {
        let result = Self::RATE
            .reciprocal()
            .ok_or(DispatchError::Other("Asset rate too low"))?
            .saturating_mul_int(balance);

        Ok(result)
    }
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Formula = ConstantSum<AssetRate>;
    type Currency = Balances;
    type AssetBalance = <Self as pallet_balances::Config>::Balance;
    type AssetId = u32;
    type PoolAssetId = u32;
    type Assets = Assets;
    type PoolAssets = PoolAssets;
    type PalletId = AssetConversionPalletId;
    type WeightInfo = ();
    type LPFee = ConstU32<20>; // means 2%
    type PoolSetupFee = ConstU128<100>; // should be more or equal to the existential deposit
    type PoolSetupFeeReceiver = AssetConversionOrigin;
    type LiquidityWithdrawalFee = LiquidityWithdrawalFee;
    type AllowMultiAssetPools = AllowMultiAssetPools;
    type MaxSwapPathLength = ConstU32<4>;
    type MintMinLiquidity = ConstU128<100>; // 100 is good enough when the main currency has 12 decimals.

    type Balance = u128;
    type HigherPrecisionBalance = sp_core::U256;

    type MultiAssetId = NativeOrAssetId<u32>;
    type MultiAssetIdConverter = NativeOrAssetIdConverter<u32>;

    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(1, 10000), (2, 20000), (3, 30000), (4, 40000)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
