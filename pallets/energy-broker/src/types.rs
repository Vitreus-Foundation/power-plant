// This file is part of Substrate.

// Copyright (C) 2022 Parity Technologies (UK) Ltd.
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

//! # Asset Conversion Types and Formulas
//!
//! Core type definitions and mathematical formulas for AMM operations.
//!
//! ## Key Components
//!
//! ### Asset Identification
//! - `PoolIdOf`: Pool identifier using asset pair
//! - `PoolInfo`: LP token tracking for pools
//! - `NativeOrAssetId`: Enum for native/non-native assets
//! - `MultiAssetIdConverter`: Asset type conversion traits
//!
//! ### Price Calculation
//! - `Formula` trait: Core AMM math interface
//! - `ConstantSum`: Fixed-rate conversion implementation
//! - Liquidity calculations
//! - Swap amount computation
//! - Fee handling
//!
//! ### Key Operations
//! - LP token amount calculation
//! - Optimal liquidity determination
//! - Input/output amount computation
//! - Price quotes with/without fees
//! - Asset normalization
//!
//! ## Safety Features
//! - Overflow protection
//! - Type-safe asset conversions
//! - Error propagation
//! - Ordering guarantees for pool pairs
//! - Minimum amount validation
//!
//! This module provides the mathematical foundation and type safety for AMM operations while
//! maintaining precision and preventing numerical errors.
//!
use super::*;
use core::marker::PhantomData;
use frame_support::traits::Get;
use sp_std::cmp::Ordering;

use frame_support::traits::tokens::{ConversionFromAssetBalance, ConversionToAssetBalance};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

pub(super) type PoolIdOf<T> = (<T as Config>::MultiAssetId, <T as Config>::MultiAssetId);

/// Stores the lp_token asset id a particular pool has been assigned.
#[derive(Decode, Encode, Default, PartialEq, Eq, MaxEncodedLen, TypeInfo)]
pub struct PoolInfo<PoolAssetId> {
    /// Liquidity pool asset
    pub lp_token: PoolAssetId,
}

/// A trait that converts between a MultiAssetId and either the native currency or an AssetId.
pub trait MultiAssetIdConverter<MultiAssetId, AssetId> {
    /// Returns the MultiAssetId reperesenting the native currency of the chain.
    fn get_native() -> MultiAssetId;

    /// Returns true if the given MultiAssetId is the native currency.
    fn is_native(asset: &MultiAssetId) -> bool;

    /// If it's not native, returns the AssetId for the given MultiAssetId.
    fn try_convert(asset: &MultiAssetId) -> Result<AssetId, ()>;

    /// Wrapps an AssetId as a MultiAssetId.
    fn into_multiasset_id(asset: &AssetId) -> MultiAssetId;
}

/// Benchmark Helper
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetId> {
    /// Returns an asset id from a given integer.
    fn asset_id(asset_id: u32) -> AssetId;
}

#[cfg(feature = "runtime-benchmarks")]
impl<AssetId> BenchmarkHelper<AssetId> for ()
where
    AssetId: From<u32>,
{
    fn asset_id(asset_id: u32) -> AssetId {
        asset_id.into()
    }
}

/// An implementation of MultiAssetId that can be either Native or an asset.
#[derive(Decode, Encode, Default, MaxEncodedLen, TypeInfo, Clone, Copy, Debug)]
pub enum NativeOrAssetId<AssetId>
where
    AssetId: Ord,
{
    /// Native asset. For example, on statemint this would be dot.
    #[default]
    Native,
    /// A non-native asset id.
    Asset(AssetId),
}

impl<AssetId: Ord> Ord for NativeOrAssetId<AssetId> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Native, Self::Native) => Ordering::Equal,
            (Self::Native, Self::Asset(_)) => Ordering::Less,
            (Self::Asset(_), Self::Native) => Ordering::Greater,
            (Self::Asset(id1), Self::Asset(id2)) => <AssetId as Ord>::cmp(id1, id2),
        }
    }
}
impl<AssetId: Ord> PartialOrd for NativeOrAssetId<AssetId> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(<Self as Ord>::cmp(self, other))
    }
}
impl<AssetId: Ord> PartialEq for NativeOrAssetId<AssetId> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl<AssetId: Ord> Eq for NativeOrAssetId<AssetId> {}

/// Converts between a MultiAssetId and an AssetId
/// (or the native currency).
pub struct NativeOrAssetIdConverter<AssetId> {
    _phantom: PhantomData<AssetId>,
}

impl<AssetId: Ord + Clone> MultiAssetIdConverter<NativeOrAssetId<AssetId>, AssetId>
    for NativeOrAssetIdConverter<AssetId>
{
    fn get_native() -> NativeOrAssetId<AssetId> {
        NativeOrAssetId::Native
    }

    fn is_native(asset: &NativeOrAssetId<AssetId>) -> bool {
        *asset == Self::get_native()
    }

    fn try_convert(asset: &NativeOrAssetId<AssetId>) -> Result<AssetId, ()> {
        match asset {
            NativeOrAssetId::Asset(asset) => Ok(asset.clone()),
            NativeOrAssetId::Native => Err(()),
        }
    }

    fn into_multiasset_id(asset: &AssetId) -> NativeOrAssetId<AssetId> {
        NativeOrAssetId::Asset((*asset).clone())
    }
}

/// A trait that contains conversion logic.
pub trait Formula<T: Config> {
    /// Calculates the optimal liquidity amount.
    fn get_liquidity_amount(
        desired: (T::AssetBalance, T::AssetBalance),
        min: (T::AssetBalance, T::AssetBalance),
        reserve: (T::AssetBalance, T::AssetBalance),
    ) -> Result<(T::AssetBalance, T::AssetBalance), Error<T>>;

    /// Calculates LP tokens amount.
    fn get_lp_token_amount(
        liquidity: (T::AssetBalance, T::AssetBalance),
        reserve: (T::AssetBalance, T::AssetBalance),
        path: (&T::MultiAssetId, &T::MultiAssetId),
        total_supply: T::AssetBalance,
    ) -> Result<T::HigherPrecisionBalance, Error<T>>;

    /// Calculates the output amount including the swap fee.
    fn get_amount_out(
        amount_in: T::HigherPrecisionBalance,
        path: (&T::MultiAssetId, &T::MultiAssetId),
    ) -> Result<T::HigherPrecisionBalance, Error<T>>;

    /// Calculates the input amount including the swap fee.
    fn get_amount_in(
        amount_out: T::HigherPrecisionBalance,
        path: (&T::MultiAssetId, &T::MultiAssetId),
    ) -> Result<T::HigherPrecisionBalance, Error<T>>;

    /// Calculates the output amount ignoring the swap fee.
    fn quote(
        amount: T::HigherPrecisionBalance,
        path: (&T::MultiAssetId, &T::MultiAssetId),
    ) -> Result<T::HigherPrecisionBalance, Error<T>>;
}

/// Fixed rate conversion.
pub struct ConstantSum<R>(PhantomData<R>);

impl<T, R> Formula<T> for ConstantSum<R>
where
    T: Config,
    R: ConversionFromAssetBalance<T::AssetBalance, T::AssetId, T::Balance>,
    R: ConversionToAssetBalance<T::Balance, T::AssetId, T::AssetBalance>,
{
    fn get_liquidity_amount(
        desired: (T::AssetBalance, T::AssetBalance),
        _min: (T::AssetBalance, T::AssetBalance),
        _reserve: (T::AssetBalance, T::AssetBalance),
    ) -> Result<(T::AssetBalance, T::AssetBalance), Error<T>> {
        ensure!(
            desired.0 > Zero::zero() || desired.1 > Zero::zero(),
            Error::<T>::WrongDesiredAmount
        );

        Ok(desired)
    }

    fn get_lp_token_amount(
        liquidity: (T::AssetBalance, T::AssetBalance),
        reserve: (T::AssetBalance, T::AssetBalance),
        path: (&T::MultiAssetId, &T::MultiAssetId),
        total_supply: T::AssetBalance,
    ) -> Result<T::HigherPrecisionBalance, Error<T>> {
        let total_liquidity = Self::normalize_assets(liquidity.0, liquidity.1, path)?;

        if !total_supply.is_zero() {
            let total_reserve = Self::normalize_assets(reserve.0, reserve.1, path)?;

            total_liquidity
                .checked_mul(&T::HigherPrecisionBalance::from(total_supply))
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&total_reserve)
                .ok_or(Error::<T>::Overflow)
        } else {
            Ok(total_liquidity)
        }
    }

    fn get_amount_out(
        amount_in: T::HigherPrecisionBalance,
        path: (&T::MultiAssetId, &T::MultiAssetId),
    ) -> Result<T::HigherPrecisionBalance, Error<T>> {
        let amount_in = amount_in
            .checked_mul(&(T::HigherPrecisionBalance::from(1000u32) - (T::LPFee::get().into())))
            .ok_or(Error::<T>::Overflow)?
            .checked_div(&T::HigherPrecisionBalance::from(1000u32))
            .ok_or(Error::<T>::Overflow)?;

        Self::quote(amount_in, path)
    }

    fn get_amount_in(
        amount_out: T::HigherPrecisionBalance,
        path: (&T::MultiAssetId, &T::MultiAssetId),
    ) -> Result<T::HigherPrecisionBalance, Error<T>> {
        let amount_in = Self::quote(amount_out, (path.1, path.0))?;
        let amount_in = amount_in
            .checked_mul(&T::HigherPrecisionBalance::from(1000u32))
            .ok_or(Error::<T>::Overflow)?
            .checked_div(&(T::HigherPrecisionBalance::from(1000u32) - (T::LPFee::get().into())))
            .ok_or(Error::<T>::Overflow)?;

        Ok(amount_in)
    }

    fn quote(
        amount: T::HigherPrecisionBalance,
        path: (&T::MultiAssetId, &T::MultiAssetId),
    ) -> Result<T::HigherPrecisionBalance, Error<T>> {
        match (
            T::MultiAssetIdConverter::try_convert(path.0),
            T::MultiAssetIdConverter::try_convert(path.1),
        ) {
            (Ok(asset), Err(_)) => Self::from_asset_balance(amount, asset),
            (Err(_), Ok(asset)) => Self::to_asset_balance(amount, asset),
            _ => Err(Error::<T>::PoolMustContainNativeCurrency),
        }
    }
}

impl<R> ConstantSum<R> {
    fn normalize_assets<T>(
        amount1: T::AssetBalance,
        amount2: T::AssetBalance,
        path: (&T::MultiAssetId, &T::MultiAssetId),
    ) -> Result<T::HigherPrecisionBalance, Error<T>>
    where
        T: Config,
        R: ConversionFromAssetBalance<T::AssetBalance, T::AssetId, T::Balance>,
    {
        let asset1 = path.0;
        let asset2 = path.1;
        let amount1 = T::HigherPrecisionBalance::from(amount1);
        let amount2 = T::HigherPrecisionBalance::from(amount2);

        let total = match (
            T::MultiAssetIdConverter::try_convert(asset1),
            T::MultiAssetIdConverter::try_convert(asset2),
        ) {
            (Ok(asset), Err(_)) => Self::from_asset_balance(amount1, asset)?.checked_add(&amount2),
            (Err(_), Ok(asset)) => Self::from_asset_balance(amount2, asset)?.checked_add(&amount1),
            _ => return Err(Error::<T>::PoolNotFound),
        };
        total.ok_or(Error::<T>::Overflow)
    }

    fn from_asset_balance<T>(
        amount: T::HigherPrecisionBalance,
        asset: T::AssetId,
    ) -> Result<T::HigherPrecisionBalance, Error<T>>
    where
        T: Config,
        R: ConversionFromAssetBalance<T::AssetBalance, T::AssetId, T::Balance>,
    {
        let amount = amount.try_into().map_err(|_| Error::<T>::Overflow)?;
        let amount = R::from_asset_balance(amount, asset).map_err(|_| Error::<T>::PoolNotFound)?;
        Ok(T::HigherPrecisionBalance::from(amount))
    }

    fn to_asset_balance<T>(
        amount: T::HigherPrecisionBalance,
        asset: T::AssetId,
    ) -> Result<T::HigherPrecisionBalance, Error<T>>
    where
        T: Config,
        R: ConversionToAssetBalance<T::Balance, T::AssetId, T::AssetBalance>,
    {
        let amount = amount.try_into().map_err(|_| Error::<T>::Overflow)?;
        let amount = R::to_asset_balance(amount, asset).map_err(|_| Error::<T>::PoolNotFound)?;
        Ok(T::HigherPrecisionBalance::from(amount))
    }
}
