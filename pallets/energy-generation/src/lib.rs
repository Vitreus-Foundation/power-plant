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
#![warn(clippy::all)]

//!
//! # Module Overview
//!
//! This module implements several key functionalities for managing staking operations in a
//! Substrate-based blockchain. It includes logic for handling rebonding of stakes, proportional
//! slashing of stakers, and strategies for disabling validators that violate network rules.
//! These functionalities play a critical role in maintaining network security, ensuring fairness,
//! and enforcing network policies regarding stakers and validators.
//!
//! # Key Features and Functions
//!
//! - **Rebonding of Stakes**:
//!   - `rebond(value: StakeOf<T>) -> (Self, StakeOf<T>)`: This function allows stakers to
//!     reallocate part or all of their unlocking funds back into the active balance. It updates
//!     the ledger with the new active balance and the remaining unlocking chunks. This provides
//!     flexibility for stakers who wish to adjust their staking strategy after initiating
//!     withdrawal.
//!
//! - **Proportional Slashing**:
//!   - `slash_stake(slash_amount: StakeOf<T>, slash_era: EraIndex) -> StakeOf<T>`: Implements a
//!     proportional slashing mechanism where a specified amount of stake is deducted from the
//!     staker’s active and/or unlocking balance. Depending on the existence and timing of
//!     unlocking chunks, the function distributes the slash across both active and unlocking
//!     stakes, ensuring that slashing is as fair as possible while covering the entire
//!     penalty amount.
//!
//! - **Validator Disabling Strategy**:
//!   - `disable_limit(validators_len: usize) -> usize`: Computes the maximum number of validators
//!     that can be disabled based on the total active set and a defined disabling factor. This
//!     limit prevents too many validators from being disabled at once, ensuring network stability.
//!   - `decision(offender_stash: &T::AccountId, slash_era: EraIndex, currently_disabled: &[u32]) -> Option<u32>`:
//!     Decides whether a validator should be disabled based on various conditions, including whether
//!     the current disabling limit has been reached and whether the offence occurred in the current
//!     era. This ensures that only current and relevant violations lead to disabling actions.
//!
//! # Access Control and Security
//!
//! - **Controlled Validator Actions**: Disabling validators and slashing stakes are both sensitive
//!   actions that have significant implications for network security and staker confidence. This
//!   module implements checks to ensure these actions are taken only when justified, such as
//!   ensuring offences occurred in the current era and limiting the number of validators that can be
//!   disabled.
//! - **Proportional Slashing**: The `slash_stake` function employs a proportional mechanism to
//!   distribute the penalty across active and unlocking funds. This approach ensures fairness while
//!   minimizing unintended side effects, such as leaving stakers with an unusable "dusted" amount
//!   in their balances.
//!
//! # Developer Notes
//!
//! - **Defensive Programming**: Several functions use defensive programming techniques, such as
//!   `unwrap_or_else` with logging, to prevent unexpected states, like a zero disabling limit or
//!   invalid indices. This ensures robustness and reliability, even when the underlying data might
//!   contain inconsistencies.
//! - **Logging for Transparency**: Throughout the disabling logic, extensive logging is used to
//!   document decisions on validator disabling, including whether the offending validator is in the
//!   active set or if a limit has been reached. These logs are critical for tracing the reasons
//!   behind disabling decisions, which can be useful during governance reviews or audits.
//! - **Conditional Benchmarking**: The module includes benchmarking support functions that allow
//!   developers to test and optimize the staking and disabling mechanisms. These functions are
//!   conditionally compiled (`#[cfg(feature = "runtime-benchmarks")]`) to ensure they do not
//!   impact the production runtime.
//!
//! # Usage Scenarios
//!
//! - **Rebonding Unlocking Funds**: A staker who has previously initiated withdrawal can use the
//!   `rebond` function to move part or all of their unlocking balance back into the active stake,
//!   thereby continuing to participate in staking rewards. This allows for flexible staking
//!   management based on market conditions or changes in personal strategy.
//! - **Validator Accountability**: When a validator commits an offence, the `slash_stake` function
//!   is used to impose a penalty. This penalty can be applied to both active and unlocking stakes,
//!   depending on the availability of funds and the era in which the offence occurred. The `decision`
//!   function in the disabling strategy further ensures that misbehaving validators are appropriately
//!   disabled from the active set.
//! - **Enforcing Disabling Limits**: The `disable_limit` function prevents too many validators from
//!   being disabled at once, which could jeopardize network operations. This is especially important
//!   in cases where multiple validators are found to be violating network policies in quick
//!   succession.
//!
//! # Integration Considerations
//!
//! - **Economic Model**: The slashing and rebonding mechanisms defined in this module are integral
//!   to the blockchain's economic model. Developers must carefully consider the impact of slashing
//!   and rebonding on overall token supply, staking incentives, and validator behavior. Misaligned
//!   penalties could lead to excessive validator exits or reduced participation.
//! - **Customizing Disabling Strategies**: The disabling strategy can be customized by adjusting
//!   the `DISABLING_LIMIT_FACTOR` to change how many validators can be disabled relative to the
//!   active set size. This flexibility allows different networks to enforce validator accountability
//!   according to their governance models and risk tolerance.
//! - **Logging and Monitoring**: Logs generated by disabling decisions and slashing operations should
//!   be monitored by network operators to ensure that validator penalties are being applied fairly
//!   and in accordance with network rules. Misuse or errors in these operations could undermine
//!   confidence in the staking process and lead to governance interventions.
//!
//! # Example Scenario
//!
//! Suppose a validator violates network policies and is identified during the current era. The
//! `slash_stake` function is used to penalize the validator by slashing a specified amount from both
//! their active and unlocking balances. After the penalty is applied, the disabling strategy (`decision`)
//! determines whether the validator should be removed from the active set based on the current disabling
//! limit and the era of the offence. This ensures that the validator is appropriately penalized and
//! potentially disabled if their actions pose a risk to network security and stability.
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]
#![recursion_limit = "256"]

// TODO: fix benchmarks
// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;
#[cfg(any(feature = "runtime-benchmarks", test))]
pub mod testing_utils;

#[cfg(test)]
pub(crate) mod mock;
#[cfg(test)]
mod tests;

pub mod inflation;
pub mod migrations;
pub mod slashing;
pub mod weights;

mod pallet;

use frame_support::{
    defensive,
    traits::{tokens::fungibles::Debt, Currency, Defensive, Get},
    BoundedVec, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use pallet_reputation::Reputation;
use parity_scale_codec::{Decode, Encode, HasCompact, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{Convert, Saturating, StaticLookup, Zero},
    BoundedBTreeMap, Perbill, Perquintill, Rounding, RuntimeDebug,
};
// pub use sp_staking::StakerStatus;
use sp_staking::{
    offence::{Offence, OffenceError, ReportOffence},
    EraIndex, OnStakingUpdate, SessionIndex,
};
use sp_std::{collections::btree_map::BTreeMap, prelude::*};
pub use weights::WeightInfo;

pub use pallet::{pallet::*, *};

pub(crate) const LOG_TARGET: &str = "runtime::staking";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
    ($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
        log::$level!(
            target: $crate::LOG_TARGET,
            concat!("[{:?}] 💸 ", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
        )
    };
}

/// Counter for the number of "reward" points earned by a given validator.
pub type RewardPoint = u32;

/// The balance type of this pallet.
pub type StakeOf<T> = <T as Config>::StakeBalance;

/// The energy type of this pallet.
pub type EnergyOf<T> = <T as pallet_assets::Config>::Balance;

/// Negative imbalance of stake.
pub type StakeNegativeImbalanceOf<T> = <<T as Config>::StakeCurrency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;
/// The energy `Debt`.
pub type EnergyDebtOf<T> =
    Debt<<T as frame_system::Config>::AccountId, pallet_assets::pallet::Pallet<T>>;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

/// Representation of the status of a staker.
#[derive(RuntimeDebug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, TypeInfo)]
pub enum StakerStatus<AccountId, Stake> {
    /// Chilling.
    Idle,
    /// Declaring desire in validate, i.e author blocks.
    Validator,
    /// Declaring desire to cooperate, delegate, or generally approve of the given set of others.
    Cooperator(Vec<(AccountId, Stake)>),
}

/// Information regarding the active era (era in used in session).
#[derive(Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ActiveEraInfo {
    /// Index of era.
    pub index: EraIndex,
    /// Moment of start expressed as millisecond from `$UNIX_EPOCH`.
    ///
    /// Start can be none if start hasn't been set for the era yet,
    /// Start is set on the first on_finalize of the era to guarantee usage of `Time`.
    start: Option<u64>,
}

/// Reward points of an era. Used to split era total payout between validators.
///
/// This points will be used to reward validators and their respective cooperators.
#[derive(PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct EraRewardPoints<AccountId: Ord> {
    /// Total number of points. Equals the sum of reward points for each validator.
    pub total: RewardPoint,
    /// The reward points earned by a given validator.
    pub individual: BTreeMap<AccountId, RewardPoint>,
}

impl<AccountId: Ord> Default for EraRewardPoints<AccountId> {
    fn default() -> Self {
        EraRewardPoints { total: Default::default(), individual: BTreeMap::new() }
    }
}

/// A destination account for payment.
#[derive(
    PartialEq, Eq, Copy, Clone, Default, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
pub enum RewardDestination<AccountId> {
    /// Pay into the stash account.
    Stash,
    /// Pay into the controller account.
    #[default]
    Controller,
    /// Pay into a specified account.
    Account(AccountId),
    /// Receive no reward.
    None,
}

/// Preference of what happens regarding validation.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, Default, MaxEncodedLen)]
pub struct ValidatorPrefs {
    /// Reward that validator takes up-front; only the rest is split between themselves and
    /// cooperators.
    #[codec(compact)]
    pub commission: Perbill,
    /// Whether or not this validator is accepting cooperations.
    ///
    /// Notice, that to be a collaborative validator it should have reputation tier more than
    /// `Config::CollaborativeValidatorReputationTier`.
    pub collaborative: bool,
    /// The minimum reputation for cooperators.
    pub min_coop_reputation: Reputation,
}

impl ValidatorPrefs {
    /// Same as `default` but with `collaborative` set to `true`
    pub fn default_collaborative() -> Self {
        Self { collaborative: true, ..Default::default() }
    }
}

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct UnlockChunk<Balance: HasCompact + MaxEncodedLen> {
    /// Amount of funds to be unlocked.
    #[codec(compact)]
    value: Balance,
    /// Era number at which point it'll be unlocked.
    #[codec(compact)]
    era: EraIndex,
}

/// The ledger of a (bonded) stash.
#[derive(
    PartialEqNoBound,
    EqNoBound,
    CloneNoBound,
    Encode,
    Decode,
    RuntimeDebugNoBound,
    TypeInfo,
    MaxEncodedLen,
)]
#[scale_info(skip_type_params(T))]
pub struct StakingLedger<T: Config> {
    /// The stash account whose balance is actually locked and at stake.
    pub stash: T::AccountId,
    /// The total amount of the stash's balance that we are currently accounting for.
    /// It's just `active` plus all the `unlocking` balances.
    #[codec(compact)]
    pub total: StakeOf<T>,
    /// The total amount of the stash's balance that will be at stake in any forthcoming
    /// rounds.
    #[codec(compact)]
    pub active: StakeOf<T>,
    /// Any balance that is becoming free, which may eventually be transferred out of the stash
    /// (assuming it doesn't get slashed first). It is assumed that this will be treated as a first
    /// in, first out queue where the new (higher value) eras get pushed on the back.
    pub unlocking: BoundedVec<UnlockChunk<StakeOf<T>>, T::MaxUnlockingChunks>,
    /// List of eras for which the stakers behind a validator have claimed rewards. Only updated
    /// for validators.
    pub claimed_rewards: BoundedVec<EraIndex, T::HistoryDepth>,
}

impl<T: Config> StakingLedger<T> {
    /// Initializes the default object using the given `validator`.
    pub fn default_from(stash: T::AccountId) -> Self {
        Self {
            stash,
            total: Zero::zero(),
            active: Zero::zero(),
            unlocking: Default::default(),
            claimed_rewards: Default::default(),
        }
    }

    /// Remove entries from `unlocking` that are sufficiently old and reduce the
    /// total by the sum of their balances.
    fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
        let mut total = self.total;
        let unlocking: BoundedVec<_, _> = self
            .unlocking
            .into_iter()
            .filter(|chunk| {
                if chunk.era > current_era {
                    true
                } else {
                    total = total.saturating_sub(chunk.value);
                    false
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect(
                "filtering items from a bounded vec always leaves length less than bounds. qed",
            );

        Self {
            stash: self.stash,
            total,
            active: self.active,
            unlocking,
            claimed_rewards: self.claimed_rewards,
        }
    }

    /// Re-bond funds that were scheduled for unlocking.
    ///
    /// Returns the updated ledger, and the amount actually rebonded.
    fn rebond(mut self, value: StakeOf<T>) -> (Self, StakeOf<T>) {
        let mut unlocking_balance = StakeOf::<T>::zero();

        while let Some(last) = self.unlocking.last_mut() {
            if unlocking_balance + last.value <= value {
                unlocking_balance += last.value;
                self.active += last.value;
                self.unlocking.pop();
            } else {
                let diff = value - unlocking_balance;

                unlocking_balance += diff;
                self.active += diff;
                last.value -= diff;
            }

            if unlocking_balance >= value {
                break;
            }
        }

        (self, unlocking_balance)
    }

    /// Slash the staker for a given amount of balance.
    ///
    /// This implements a proportional slashing system, whereby we set our preference to slash as
    /// such:
    ///
    /// - If any unlocking chunks exist that are scheduled to be unlocked at `slash_era +
    ///   bonding_duration` and onwards, the slash is divided equally between the active ledger and
    ///   the unlocking chunks.
    /// - If no such chunks exist, then only the active balance is slashed.
    ///
    /// Note that the above is only a *preference*. If for any reason the active ledger, with or
    /// without some portion of the unlocking chunks that are more justified to be slashed are not
    /// enough, then the slashing will continue and will consume as much of the active and unlocking
    /// chunks as needed.
    ///
    /// This will never slash more than the given amount. If any of the chunks become dusted, the
    /// last chunk is slashed slightly less to compensate. Returns the amount of funds actually
    /// slashed.
    ///
    /// `slash_era` is the era in which the slash (which is being enacted now) actually happened.
    pub fn slash_stake(
        &mut self,
        slash_amount: StakeOf<T>,
        minimum_balance: StakeOf<T>,
        slash_era: EraIndex,
    ) -> StakeOf<T> {
        if slash_amount.is_zero() {
            return Zero::zero();
        }

        use sp_runtime::PerThing as _;
        let mut remaining_slash = slash_amount;
        let pre_slash_total = self.total;

        // for a `slash_era = x`, any chunk that is scheduled to be unlocked at era `x + 28`
        // (assuming 28 is the bonding duration) onwards should be slashed.
        let slashable_chunks_start = slash_era + T::BondingDuration::get();

        // `Some(ratio)` if this is proportional, with `ratio`, `None` otherwise. In both cases, we
        // slash first the active chunk, and then `slash_chunks_priority`.
        let (maybe_proportional, slash_chunks_priority) = {
            if let Some(first_slashable_index) =
                self.unlocking.iter().position(|c| c.era >= slashable_chunks_start)
            {
                // If there exists a chunk who's after the first_slashable_start, then this is a
                // proportional slash, because we want to slash active and these chunks
                // proportionally.

                // The indices of the first chunk after the slash up through the most recent chunk.
                // (The most recent chunk is at greatest from this era)
                let affected_indices = first_slashable_index..self.unlocking.len();
                let unbonding_affected_balance =
                    affected_indices.clone().fold(StakeOf::<T>::zero(), |sum, i| {
                        if let Some(chunk) = self.unlocking.get(i).defensive() {
                            sum.saturating_add(chunk.value)
                        } else {
                            sum
                        }
                    });
                let affected_balance = self.active.saturating_add(unbonding_affected_balance);
                let ratio = Perquintill::from_rational_with_rounding(
                    slash_amount,
                    affected_balance,
                    Rounding::Up,
                )
                .unwrap_or_else(|_| Perquintill::one());
                (
                    Some(ratio),
                    affected_indices.chain((0..first_slashable_index).rev()).collect::<Vec<_>>(),
                )
            } else {
                // We just slash from the last chunk to the most recent one, if need be.
                (None, (0..self.unlocking.len()).rev().collect::<Vec<_>>())
            }
        };

        // Helper to update `target` and the ledgers total after accounting for slashing `target`.
        log!(
            debug,
            "slashing {:?} for era {:?} out of {:?}, priority: {:?}, proportional = {:?}",
            slash_amount,
            slash_era,
            self,
            slash_chunks_priority,
            maybe_proportional,
        );

        let mut slash_out_of = |target: &mut StakeOf<T>, slash_remaining: &mut StakeOf<T>| {
            let mut slash_from_target = if let Some(ratio) = maybe_proportional {
                ratio.mul_ceil(*target)
            } else {
                *slash_remaining
            }
            // this is the total that that the slash target has. We can't slash more than
            // this anyhow!
            .min(*target)
            // this is the total amount that we would have wanted to slash
            // non-proportionally, a proportional slash should never exceed this either!
            .min(*slash_remaining);

            // slash out from *target exactly `slash_from_target`.
            *target -= slash_from_target;
            if *target < minimum_balance {
                // Slash the rest of the target if it's dust. This might cause the last chunk to be
                // slightly under-slashed, by at most `MaxUnlockingChunks * ED`, which is not a big
                // deal.
                slash_from_target =
                    sp_std::mem::replace(target, Zero::zero()).saturating_add(slash_from_target)
            }

            self.total = self.total.saturating_sub(slash_from_target);
            *slash_remaining = slash_remaining.saturating_sub(slash_from_target);
        };

        // If this is *not* a proportional slash, the active will always wiped to 0.
        slash_out_of(&mut self.active, &mut remaining_slash);

        let mut slashed_unlocking = BTreeMap::<_, _>::new();
        for i in slash_chunks_priority {
            if remaining_slash.is_zero() {
                break;
            }

            if let Some(chunk) = self.unlocking.get_mut(i).defensive() {
                slash_out_of(&mut chunk.value, &mut remaining_slash);
                // write the new slashed value of this chunk to the map.
                slashed_unlocking.insert(chunk.era, chunk.value);
            } else {
                break;
            }
        }

        // clean unlocking chunks that are set to zero.
        self.unlocking.retain(|c| !c.value.is_zero());

        let final_slashed_amount = pre_slash_total.saturating_sub(self.total);
        T::EventListeners::on_slash(
            &self.stash,
            self.active,
            &slashed_unlocking,
            final_slashed_amount,
        );
        final_slashed_amount
    }
}

/// A record of the cooperations made by a specific account.
#[derive(
    PartialEqNoBound, EqNoBound, Clone, Encode, Decode, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen,
)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(T))]
pub struct Cooperations<T: Config> {
    /// The targets of cooperation.
    pub targets: BoundedBTreeMap<T::AccountId, StakeOf<T>, T::MaxCooperations>,
    /// The era the cooperations were submitted.
    ///
    /// Except for initial cooperations which are considered submitted at era 0.
    pub submitted_in: EraIndex,
    /// Whether the cooperations have been suppressed. This can happen due to slashing of the
    /// validators, or other events that might invalidate the cooperation.
    ///
    /// NOTE: this for future proofing and is thus far not used.
    pub suppressed: bool,
}

/// The amount of exposure (to slashing) than an individual cooperator has.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct IndividualExposure<AccountId, Balance: HasCompact> {
    /// The stash account of the cooperator in question.
    pub who: AccountId,
    /// Amount of funds exposed.
    #[codec(compact)]
    pub value: Balance,
}

/// A snapshot of the stake backing a single validator in the system.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct Exposure<AccountId, Balance: HasCompact> {
    /// The total balance backing this validator.
    #[codec(compact)]
    pub total: Balance,
    /// The validator's own stash that is exposed.
    #[codec(compact)]
    pub own: Balance,
    /// The portions of cooperators stashes that are exposed.
    pub others: Vec<IndividualExposure<AccountId, Balance>>,
}

impl<AccountId, Balance: Default + HasCompact> Default for Exposure<AccountId, Balance> {
    fn default() -> Self {
        Self { total: Default::default(), own: Default::default(), others: vec![] }
    }
}

/// A pending slash record. The value of the slash has been computed but not applied yet,
/// rather deferred for several eras.
#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct UnappliedSlash<AccountId, SlashEntity> {
    /// The stash ID of the offending validator.
    validator: AccountId,
    /// The validator's own slash.
    own: SlashEntity,
    /// All other slashed stakers and amounts.
    others: Vec<(AccountId, SlashEntity)>,
    /// Reporters of the offence; bounty payout recipients.
    reporters: Vec<AccountId>,
    /// The amount of payout.
    payout: SlashEntity,
}

impl<AccountId, SlashEntity: Zero> UnappliedSlash<AccountId, SlashEntity> {
    /// Initializes the default object using the given `validator`.
    pub fn default_from(validator: AccountId) -> Self {
        Self {
            validator,
            own: Zero::zero(),
            others: vec![],
            reporters: vec![],
            payout: Zero::zero(),
        }
    }

    pub fn new(
        validator: AccountId,
        own: SlashEntity,
        others: Vec<(AccountId, SlashEntity)>,
        reporters: Vec<AccountId>,
        payout: SlashEntity,
    ) -> Self {
        Self { validator, own, others, reporters, payout }
    }
}

/// Means for interacting with a specialized version of the `session` trait.
///
/// This is needed because `Staking` sets the `ValidatorIdOf` of the `pallet_session::Config`
pub trait SessionInterface<AccountId> {
    /// Disable the validator at the given index, returns `false` if the validator was already
    /// disabled or the index is out of bounds.
    fn disable_validator(validator_index: u32) -> bool;
    /// Get the validators from session.
    fn validators() -> Vec<AccountId>;
    /// Prune historical session tries up to but not including the given index.
    fn prune_historical_up_to(up_to: SessionIndex);
}

impl<T: Config> SessionInterface<<T as frame_system::Config>::AccountId> for T
where
    T: pallet_session::Config<ValidatorId = <T as frame_system::Config>::AccountId>,
    T: pallet_session::historical::Config<
        FullIdentification = Exposure<<T as frame_system::Config>::AccountId, StakeOf<T>>,
        FullIdentificationOf = ExposureOf<T>,
    >,
    T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Config>::AccountId>,
    T::SessionManager: pallet_session::SessionManager<<T as frame_system::Config>::AccountId>,
    T::ValidatorIdOf: Convert<
        <T as frame_system::Config>::AccountId,
        Option<<T as frame_system::Config>::AccountId>,
    >,
{
    fn disable_validator(validator_index: u32) -> bool {
        <pallet_session::Pallet<T>>::disable_index(validator_index)
    }

    fn validators() -> Vec<<T as frame_system::Config>::AccountId> {
        <pallet_session::Pallet<T>>::validators()
    }

    fn prune_historical_up_to(up_to: SessionIndex) {
        <pallet_session::historical::Pallet<T>>::prune_up_to(up_to);
    }
}

impl<AccountId> SessionInterface<AccountId> for () {
    fn disable_validator(_: u32) -> bool {
        true
    }
    fn validators() -> Vec<AccountId> {
        Vec::new()
    }
    fn prune_historical_up_to(_: SessionIndex) {}
}

/// Handler for determining the energy demand on the current era.
pub trait EnergyRateCalculator<Stake, Energy> {
    /// Determine the energy demand for this era.
    fn calculate_energy_rate(
        total_staked: Stake,
        total_issuance: Energy,
        core_nodes_num: u32,
        battery_slot_cap: Energy,
    ) -> Energy;
}

pub trait OnVipMembershipHandler<T, Res, Perbill> {
    /// Change quarter info.
    fn change_quarter_info() -> Res;

    /// Kick account from VIP members.
    fn kick_account_from_vip(account: &T) -> Res;

    /// Update active stake by VIP member.
    fn update_active_stake(account: &T) -> Res;

    /// Get tax percent of account.
    fn get_tax_percent(account: &T) -> Perbill;
}

/// Mode of era-forcing.
#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    Encode,
    Decode,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
)]
pub enum Forcing {
    /// Not forcing anything - just let whatever happen.
    #[default]
    NotForcing,
    /// Force a new era, then reset to `NotForcing` as soon as it is done.
    /// Note that this will force to trigger an election until a new era is triggered, if the
    /// election failed, the next session end will trigger a new election again, until success.
    ForceNew,
    /// Avoid a new era indefinitely.
    ForceNone,
    /// Force a new era at the end of all sessions indefinitely.
    ForceAlways,
}

/// A `Convert` implementation that finds the stash of the given controller account,
/// if any.
pub struct StashOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Convert<T::AccountId, Option<T::AccountId>> for StashOf<T> {
    fn convert(controller: T::AccountId) -> Option<T::AccountId> {
        <Pallet<T>>::ledger(&controller).map(|l| l.stash)
    }
}

/// A typed conversion from stash account ID to the active exposure of cooperators
/// on that account.
///
/// Active exposure is the exposure of the validator set currently validating, i.e. in
/// `active_era`. It can differ from the latest planned exposure in `current_era`.
pub struct ExposureOf<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Convert<T::AccountId, Option<Exposure<T::AccountId, StakeOf<T>>>>
    for ExposureOf<T>
{
    fn convert(validator: T::AccountId) -> Option<Exposure<T::AccountId, StakeOf<T>>> {
        <Pallet<T>>::active_era()
            .map(|active_era| <Pallet<T>>::eras_stakers(active_era.index, &validator))
    }
}

/// Filter historical offences out and only allow those from the bonding period.
pub struct FilterHistoricalOffences<T, R> {
    _inner: sp_std::marker::PhantomData<(T, R)>,
}

impl<T, Reporter, Offender, R, O> ReportOffence<Reporter, Offender, O>
    for FilterHistoricalOffences<Pallet<T>, R>
where
    T: Config,
    R: ReportOffence<Reporter, Offender, O>,
    O: Offence<Offender>,
{
    fn report_offence(reporters: Vec<Reporter>, offence: O) -> Result<(), OffenceError> {
        // Disallow any slashing from before the current bonding period.
        let offence_session = offence.session_index();
        let bonded_eras = BondedEras::<T>::get();

        if bonded_eras.first().filter(|(_, start)| offence_session >= *start).is_some() {
            R::report_offence(reporters, offence)
        } else {
            <Pallet<T>>::deposit_event(Event::<T>::OldSlashingReportDiscarded {
                session_index: offence_session,
            });
            Ok(())
        }
    }

    fn is_known_offence(offenders: &[Offender], time_slot: &O::TimeSlot) -> bool {
        R::is_known_offence(offenders, time_slot)
    }
}

/// Configurations of the benchmarking of the pallet.
pub trait BenchmarkingConfig {
    /// The maximum number of validators to use.
    type MaxValidators: Get<u32>;
    /// The maximum number of cooperators to use.
    type MaxCooperators: Get<u32>;
}

/// A mock benchmarking config for pallet-staking.
///
/// Should only be used for testing.
#[cfg(feature = "std")]
pub struct TestBenchmarkingConfig;

#[cfg(feature = "std")]
impl BenchmarkingConfig for TestBenchmarkingConfig {
    type MaxValidators = frame_support::traits::ConstU32<100>;
    type MaxCooperators = frame_support::traits::ConstU32<100>;
}

/// Controls validator disabling
pub trait DisablingStrategy<T: Config> {
    /// Make a disabling decision. Returns the index of the validator to disable or `None` if no new
    /// validator should be disabled.
    fn decision(
        offender_stash: &T::AccountId,
        slash_era: EraIndex,
        currently_disabled: &[u32],
    ) -> Option<u32>;
}

/// Implementation of [`DisablingStrategy`] which disables validators from the active set up to a
/// threshold. `DISABLING_LIMIT_FACTOR` is the factor of the maximum disabled validators in the
/// active set. E.g. setting this value to `3` means no more than 1/3 of the validators in the
/// active set can be disabled in an era.
/// By default a factor of 3 is used which is the byzantine threshold.
pub struct UpToLimitDisablingStrategy<const DISABLING_LIMIT_FACTOR: usize = 3>;

impl<const DISABLING_LIMIT_FACTOR: usize> UpToLimitDisablingStrategy<DISABLING_LIMIT_FACTOR> {
    /// Disabling limit calculated from the total number of validators in the active set. When
    /// reached no more validators will be disabled.
    pub fn disable_limit(validators_len: usize) -> usize {
        validators_len
            .saturating_sub(1)
            .checked_div(DISABLING_LIMIT_FACTOR)
            .unwrap_or_else(|| {
                defensive!("DISABLING_LIMIT_FACTOR should not be 0");
                0
            })
    }
}

impl<T: Config, const DISABLING_LIMIT_FACTOR: usize> DisablingStrategy<T>
    for UpToLimitDisablingStrategy<DISABLING_LIMIT_FACTOR>
{
    fn decision(
        offender_stash: &T::AccountId,
        slash_era: EraIndex,
        currently_disabled: &[u32],
    ) -> Option<u32> {
        let active_set = T::SessionInterface::validators();

        // We don't disable more than the limit
        if currently_disabled.len() >= Self::disable_limit(active_set.len()) {
            log!(
                debug,
                "Won't disable: reached disabling limit {:?}",
                Self::disable_limit(active_set.len())
            );
            return None;
        }

        // We don't disable for offences in previous eras
        if ActiveEra::<T>::get().map(|e| e.index).unwrap_or_default() > slash_era {
            log!(
                debug,
                "Won't disable: current_era {:?} > slash_era {:?}",
                Pallet::<T>::current_era().unwrap_or_default(),
                slash_era
            );
            return None;
        }

        let offender_idx = if let Some(idx) = active_set.iter().position(|i| i == offender_stash) {
            idx as u32
        } else {
            log!(debug, "Won't disable: offender not in active set",);
            return None;
        };

        log!(debug, "Will disable {:?}", offender_idx);

        Some(offender_idx)
    }
}
