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
#![warn(missing_docs)]

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

#[allow(missing_docs)]
#[derive(Clone, Debug, Decode, Default, Encode, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub struct Reputation {
    /// The reputation tier.
    tier: Option<ReputationTier>,
    /// The amount of reputation points.
    points: ReputationPoint,
}

impl Reputation {
    /// Update reputation with given points.
    pub fn update(&mut self, new_points: ReputationPoint) {
        self.tier = self.tier_from_points(new_points);
        self.points = new_points;
    }

    fn tier_from_points(&self, new_points: ReputationPoint) -> Option<ReputationTier> {
        self.tier
            .and_then(|tier| match new_points.0 {
                p if p < 2000 => None,
                p if p < 4000 => Some(ReputationTier::VanguardZero),
                p if p <= 60_000 && tier >= ReputationTier::TrailblazerZero => {
                    Some(new_points.into())
                },
                p if p < 250_000 && tier >= ReputationTier::TrailblazerZero => {
                    Some(ReputationTier::TrailblazerZero)
                },
                p if p <= 630_000 && tier >= ReputationTier::UltramodernZero => {
                    Some(new_points.into())
                },
                p if p < 2_000_000 && tier >= ReputationTier::UltramodernZero => {
                    Some(ReputationTier::UltramodernZero)
                },
                _ => Some(new_points.into()),
            })
            .or_else(|| if new_points.0 >= 2000 { Some(new_points.into()) } else { None })
    }

    /// Get the `ReputationTier`.
    pub fn tier(&self) -> Option<ReputationTier> {
        self.tier
    }

    /// Get the reputation points.
    pub fn points(&self) -> ReputationPoint {
        self.points
    }
}

impl From<u64> for Reputation {
    fn from(points: u64) -> Self {
        let points = ReputationPoint(points);
        Self { tier: Some(ReputationTier::from(points)), points }
    }
}

/// The reputation score levels (as per the whitepaper).
#[derive(
    Clone, Copy, Debug, Decode, Encode, Eq, MaxEncodedLen, Ord, PartialEq, PartialOrd, TypeInfo,
)]
pub enum ReputationTier {
    /// This is the lowest level which a user can truly be considered for randomized selection of
    /// validation. This status is lost should a user fall below a score of 2,000.
    VanguardZero,
    /// This is the default level which all users start at, if a users score drops below 4,000, they
    /// are demoted.
    VanguardOne,
    /// This rank is achieved after successfully reaching a score of 50,000.
    VanguardTwo,
    /// This rank is achieved after successfully reaching a score of 125,000.
    VanguardThree,
    /// This rank is the lowest level which a user is considered in this tier. This rank overlaps
    /// with Support 3, but is only accessible during demotion events. This rank is lost should a
    /// user fall to a score of 60,000
    TrailblazerZero,
    /// This rank is achieved after successfully reaching a score of 250,000.
    TrailblazerOne,
    /// This rank is achieved after successfully reaching a score of 580,000.
    TrailblazerTwo,
    /// This rank is achieved after successfully reaching a score of 1,000,000.
    TrailblazerThree,
    /// This is the lowest level which a user is considered in this tier. This rank overlaps with
    /// Relay 3, but is only accessible during demotion events. This rank is lost should a user fall
    /// to a score of 630,000
    UltramodernZero,
    /// This rank is achieved after successfully reaching a score of 2,000,000.
    UltramodernOne,
    /// This rank is achieved after successfully reaching a score of 4,250,000.
    UltramodernTwo,
    /// This rank is achieved after successfully reaching a score of 9,000,000.
    UltramodernThree,
}

impl From<ReputationPoint> for ReputationTier {
    fn from(points: ReputationPoint) -> Self {
        match points.0 {
            p if p < 4000 => Self::VanguardZero,
            p if p < 50_000 => Self::VanguardOne,
            p if p < 125_000 => Self::VanguardTwo,
            p if p < 250_000 => Self::VanguardThree,
            // this one is used in slashes only
            // p if p < 250_000 => Self::TrailblazerZero,
            p if p < 580_000 => Self::TrailblazerOne,
            p if p < 1_000_000 => Self::TrailblazerTwo,
            p if p < 2_000_000 => Self::TrailblazerThree,
            // this one is used in slashes only
            // p if p < 250_000 => Self::UltramodelZero,
            p if p < 4_250_000 => Self::UltramodernOne,
            p if p < 9_000_000 => Self::UltramodernTwo,
            p if p >= 9_000_000 => Self::UltramodernThree,
            _ => unreachable!("Reputation points are always positive"),
        }
    }
}

/// The reputation type has the amount of reputation (called `points`) and when it was updated.
#[derive(
    Clone,
    Encode,
    Decode,
    serde::Deserialize,
    serde::Serialize,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
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
    serde::Deserialize,
    serde::Serialize,
    Encode,
    Decode,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
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
