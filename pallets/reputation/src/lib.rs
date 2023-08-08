//! Reputation pallet provides users behavior evaluation. Any user gets basic reputation reward per
//! block.
//!
//! The frequency of rewards/reputation updates is meant to be done by the pallet, which uses the
//! reputation. To calculate the reputation for all users you should call
//! `Pallet::update_points_for_time`. It's not an extrinsic, but it's cost operation because of the
//! iteration via accounts, so don't call it very often.
//!
//! Reputation is measured in `points`. The `points` can't be transfered, sold or bought. And you
//! should avoid any mechanism for points movement between accounts, because as you get reputation
//! per time, you could simply accumulate reputation between different accounts and get <N of
//! accounts>x points rewards.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]

use core::ops::{Deref, DerefMut};

use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::SaturatedConversion;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod pallet;
pub mod weights;

/// The number of reputation points per block is the basic amount of reputation, which is used to
/// calculate everything else.
pub const REPUTATION_POINTS_PER_BLOCK: ReputationPoint = ReputationPoint(90);
/// The number of reputation points per 24 hours.
///
/// Given that a slot duration is 3000 ms per block (for BABE), we have 60_000 / 3000 blocks per
/// minute, so:
///
/// REPUTATION_POINTS_PER_BLOCK * 20 blocks/minute * 60 minutes * 24 hours
pub const REPUTATION_POINTS_PER_DAY: ReputationPoint =
    ReputationPoint(REPUTATION_POINTS_PER_BLOCK.0 * 10 * 60 * 24);

/// The reputation type has the amount of reputation (called `points`) and when it was updated.
#[derive(
    Clone,
    Encode,
    Decode,
    frame_support::Deserialize,
    frame_support::Serialize,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
// #[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(test, derive(Debug))]
#[scale_info(skip_type_params(T))]
// we use `T: Config`, instead of `T: UniqueSaturatedInfo`, because `UniqueSaturationInto` would
// require `Config` anyway.
pub struct ReputationRecord {
    /// The amount of reputation.
    pub points: ReputationPoint,
    /// When the reputation was updated.
    pub updated: u64,
}

impl ReputationRecord {
    /// Create a new reputation with the given block number.
    pub fn with_blocknumber(updated: u64) -> Self {
        Self { points: ReputationPoint(0), updated }
    }

    /// Create a new reputation with the current block number.
    ///
    /// Shouldn't be called outside of externalities context.
    pub fn with_now<T: pallet::Config>() -> Self {
        Self::with_blocknumber(frame_system::Pallet::<T>::block_number().saturated_into())
    }

    /// Update the reputation points for the range between `Self::updated` and `block_number`.
    pub fn update_with_block_number(&mut self, block_number: u64) {
        let reward = Self::calculate(self.updated, block_number);
        *self.points = self.points.saturating_add(reward);
        self.updated = block_number;
    }

    /// Calculate reputation points for the range between `start` and `end` blocks.
    pub fn calculate(start: u64, end: u64) -> u64 {
        if end < start {
            return 0;
        }

        let difference = end - start;
        crate::REPUTATION_POINTS_PER_BLOCK.saturating_mul(difference.saturated_into())
    }
}

impl From<ReputationPoint> for ReputationRecord {
    fn from(points: ReputationPoint) -> Self {
        Self { points, updated: 0 }
    }
}

/// The reputation points type.
#[derive(
    Clone,
    Debug,
    Default,
    Copy,
    frame_support::Deserialize,
    frame_support::Serialize,
    Encode,
    Decode,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
// #[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[scale_info(skip_type_params(T))]
pub struct ReputationPoint(pub u64);

impl ReputationPoint {
    /// Create new reputation points.
    pub const fn new(points: u64) -> Self {
        Self(points)
    }
}

impl From<u64> for ReputationPoint {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl Deref for ReputationPoint {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ReputationPoint {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<u64> for ReputationPoint {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}
