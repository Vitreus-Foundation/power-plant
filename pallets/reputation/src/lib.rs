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

use core::ops::{Add, Deref, DerefMut};

use libm::{ceil, pow};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::{SaturatedConversion, Zero};

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
pub const REPUTATION_POINTS_PER_BLOCK: ReputationPoint = ReputationPoint(24);

/// The number of reputation points per 24 hours.
///
/// Given that a slot duration is 3000 ms per block (for BABE), we have 60_000 / 3000 blocks per
/// minute, so:
///
/// REPUTATION_POINTS_PER_BLOCK * 20 blocks/minute * 60 minutes * 24 hours
pub const REPUTATION_POINTS_PER_DAY: ReputationPoint =
    ReputationPoint(REPUTATION_POINTS_PER_BLOCK.0 * 10 * 60 * 24);

/// The number of repputation points per 30 days.
pub const REPUTATION_POINTS_PER_MONTH: ReputationPoint =
    ReputationPoint(REPUTATION_POINTS_PER_DAY.0 * 30);

/// The number of repputation points per 12 months.
pub const REPUTATION_POINTS_PER_YEAR: ReputationPoint =
    ReputationPoint(REPUTATION_POINTS_PER_MONTH.0 * 12);

/// `c` in reputation ranking formula.
pub const CURVATURE: f64 = 1.6;

/// `N` in block authoring rewards formula.
pub const NORMAL: f64 = 2.0;

/// We use U3 in formula.
pub const ULTRAMODERN_3_POINTS: ReputationPoint =
    ReputationPoint((REPUTATION_POINTS_PER_YEAR.0 as f64 * NORMAL) as u64);

/// Total ranks per U3.
pub const RANKS_PER_U3: u8 = 9;

/// The number of ranks per tier.
pub const RANKS_PER_TIER: u8 = 3;

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
    pub reputation: Reputation,
    /// When the reputation was updated.
    pub updated: u64,
}

impl ReputationRecord {
    /// Create a new reputation with the given block number.
    pub fn with_blocknumber(updated: u64) -> Self {
        Self { reputation: Default::default(), updated }
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
        self.reputation.increase(reward.into());
        self.updated = block_number;
    }

    /// Calculate reputation points for the range between `start` and `end` blocks.
    pub fn calculate(start: u64, end: u64) -> u64 {
        if end <= start {
            return 0;
        }

        let difference = end - start;
        crate::REPUTATION_POINTS_PER_BLOCK.saturating_mul(difference.saturated_into())
    }
}

impl From<ReputationPoint> for ReputationRecord {
    fn from(points: ReputationPoint) -> Self {
        Self { reputation: points.into(), updated: 0 }
    }
}

#[allow(missing_docs)]
#[derive(
    Clone,
    Debug,
    Decode,
    Default,
    Encode,
    Eq,
    MaxEncodedLen,
    Ord,
    PartialEq,
    PartialOrd,
    TypeInfo,
    serde::Deserialize,
    serde::Serialize,
)]
pub struct Reputation {
    /// The reputation tier.
    tier: Option<ReputationTier>,
    /// The amount of reputation points.
    points: ReputationPoint,
}

impl Reputation {
    /// Increase reputation by the given amount of points.
    pub fn increase(&mut self, points: ReputationPoint) {
        let new_points = self.points.saturating_add(*points).into();
        self.update(new_points);
    }

    /// Decrease reputation by the given amount of points.
    pub fn decrease(&mut self, points: ReputationPoint) {
        let new_points = self.points.saturating_sub(*points).into();
        self.update(new_points);
    }

    /// Update reputation with given points.
    pub fn update(&mut self, new_points: ReputationPoint) {
        self.tier = ReputationTier::with_rank_relative_to(&self.tier, new_points);
        self.points = new_points;
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

impl From<ReputationTier> for Reputation {
    fn from(tier: ReputationTier) -> Self {
        Self { tier: Some(tier), points: ReputationPoint::from_rank(tier.rank()) }
    }
}

impl From<ReputationPoint> for Reputation {
    fn from(points: ReputationPoint) -> Self {
        Self { tier: ReputationTier::try_from_rank(points.rank()), points }
    }
}

impl From<u64> for Reputation {
    fn from(points: u64) -> Self {
        let points = ReputationPoint(points);
        Self { tier: ReputationTier::try_from_rank(points.rank()), points }
    }
}

/// The reputation score levels (as per the research).
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    Encode,
    Eq,
    MaxEncodedLen,
    Ord,
    PartialEq,
    PartialOrd,
    TypeInfo,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum ReputationTier {
    Vanguard(u8),
    Trailblazer(u8),
    Ultramodern(u8),
}

impl ReputationTier {
    /// Init tier from rank.
    ///
    /// If the rank is 0, returns `None`.
    pub fn try_from_rank(rank: u8) -> Option<Self> {
        if rank == 0 {
            return None;
        }

        Some(match rank {
            r if r <= RANKS_PER_TIER => Self::Vanguard(rank),
            r if r > RANKS_PER_TIER && r <= RANKS_PER_TIER * 2 => {
                Self::Trailblazer(rank - RANKS_PER_TIER)
            },
            _ => Self::Ultramodern(rank - RANKS_PER_TIER * 2),
        })
    }

    /// Get the rank.
    pub fn rank(&self) -> u8 {
        let offset = self.tier_index().saturating_mul(RANKS_PER_TIER);
        self.relative_rank().saturating_add(offset)
    }

    /// Get the rank relative to the tier (i.e. Vanguard, Trailblazer or Ultramodern)
    pub fn relative_rank(&self) -> u8 {
        match self {
            Self::Vanguard(rank) => *rank,
            Self::Trailblazer(rank) => *rank,
            Self::Ultramodern(rank) => *rank,
        }
    }

    /// Vanguard - 0, Trailblazer - 1, Ultramodern - 2
    pub fn tier_index(&self) -> u8 {
        match self {
            Self::Vanguard(_) => 0,
            Self::Trailblazer(_) => 1,
            Self::Ultramodern(_) => 2,
        }
    }

    /// Init tier with rank relative to the given tier.
    ///
    /// If tier felt lower than **Vanguard 1**, it'll return `None`.
    pub fn with_rank_relative_to(
        relative_to: &Option<Self>,
        new_points: ReputationPoint,
    ) -> Option<Self> {
        let new_rank = new_points.rank();

        if new_rank == 0 {
            return None;
        }

        let relative_to = match relative_to {
            Some(r) => {
                if new_rank == r.rank() {
                    return *relative_to;
                }
                r
            },
            None => return Self::try_from_rank(new_rank),
        };

        let lower_index = relative_to.tier_index().saturating_sub(1);
        let middle_rank = ceil(RANKS_PER_TIER as f64 / 2.0) as u8;
        let zero_threshold = ReputationPoint::from_rank(lower_index + middle_rank);

        if relative_to.relative_rank() == 0 && new_points <= zero_threshold {
            if lower_index == 0 && lower_index == relative_to.tier_index() {
                return None;
            }

            return Self::try_from_rank(lower_index + middle_rank);
        }

        if new_rank < relative_to.rank() {
            let first_rank_points =
                ReputationPoint::from_rank(relative_to.tier_index() * RANKS_PER_TIER + 1);

            if new_points < first_rank_points && new_points > zero_threshold {
                return Some(Self::with_zero_rank(relative_to.tier_index()));
            }
        }

        Self::try_from_rank(new_rank)
    }

    /// Init tier with zero rank.
    ///
    /// The argument is the index of the tier (Vanguard, Trailblazer or Ultramodern).
    pub fn with_zero_rank(tier_index: u8) -> Self {
        match tier_index {
            0 => Self::Vanguard(0),
            1 => Self::Trailblazer(0),
            2 => Self::Ultramodern(0),
            _ => unreachable!("There are only 3 tiers"),
        }
    }
}

/// The reputation points type.
#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    Default,
    Encode,
    Eq,
    MaxEncodedLen,
    Ord,
    PartialEq,
    PartialOrd,
    TypeInfo,
    serde::Deserialize,
    serde::Serialize,
)]
#[scale_info(skip_type_params(T))]
pub struct ReputationPoint(pub u64);

impl ReputationPoint {
    /// Create new reputation points.
    pub const fn new(points: u64) -> Self {
        Self(points)
    }

    /// Init reputation points from rank.
    pub fn from_rank(rank: u8) -> Self {
        ReputationPoint(
            (ULTRAMODERN_3_POINTS.0 as f64
                * pow(f64::from(rank) / f64::from(RANKS_PER_U3), CURVATURE)) as u64,
        )
    }

    /// The corresponding reputation rank.
    pub fn rank(&self) -> u8 {
        let res = libm::pow(self.0 as f64 / ULTRAMODERN_3_POINTS.0 as f64, 1. / CURVATURE)
            * RANKS_PER_U3 as f64;
        let resulting_rank = ceil(res) as u8;
        if &ReputationPoint::from_rank(resulting_rank) > self {
            resulting_rank.saturating_sub(1)
        } else {
            resulting_rank
        }
    }
}

impl From<ReputationTier> for ReputationPoint {
    fn from(tier: ReputationTier) -> Self {
        Self::from_rank(tier.rank())
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

impl Add for ReputationPoint {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Zero for ReputationPoint {
    fn zero() -> Self {
        Self(0)
    }
    fn is_zero(&self) -> bool {
        self.0 == 0
    }
}
