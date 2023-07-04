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

//! Implementations for the Staking FRAME Pallet.

use core::cmp::Ordering;

use frame_support::{
    dispatch::WithPostDispatchInfo,
    pallet_prelude::*,
    traits::{
        Currency, DefensiveResult, Get, Imbalance, LockableCurrency, OnUnbalanced, UnixTime,
        WithdrawReasons,
    },
    weights::Weight,
};
use frame_system::RawOrigin;
use pallet_session::historical;
use sp_runtime::{
    traits::{Bounded, Convert, One, SaturatedConversion, Saturating, StaticLookup, Zero},
    Perbill,
};
use sp_staking::{
    offence::{DisableStrategy, OffenceDetails, OnOffenceHandler},
    EraIndex, SessionIndex, Stake, StakingInterface,
};
use sp_std::prelude::*;

use crate::{
    log, slashing, weights::WeightInfo, ActiveEraInfo, BalanceOf, EraPayout, Exposure, ExposureOf,
    Forcing, PositiveImbalanceOf, RewardDestination, SessionInterface, StakingLedger,
};

use super::{pallet::*, STAKING_ID};

impl<T: Config> Pallet<T> {
    /// The total balance that can be slashed from a stash account as of right now.
    pub fn slashable_balance_of(stash: &T::AccountId) -> BalanceOf<T> {
        // Weight note: consider making the stake accessible through stash.
        Self::bonded(stash).and_then(Self::ledger).map(|l| l.active).unwrap_or_default()
    }

    pub(super) fn do_withdraw_unbonded(
        controller: &T::AccountId,
        num_slashing_spans: u32,
    ) -> Result<Weight, DispatchError> {
        let mut ledger = Self::ledger(controller).ok_or(Error::<T>::NotController)?;
        let (stash, old_total) = (ledger.stash.clone(), ledger.total);
        if let Some(current_era) = Self::current_era() {
            ledger = ledger.consolidate_unlocked(current_era)
        }

        let used_weight =
            if ledger.unlocking.is_empty() && ledger.active < T::Currency::minimum_balance() {
                // This account must have called `unbond()` with some value that caused the active
                // portion to fall below existential deposit + will have no more unlocking chunks
                // left. We can now safely remove all staking-related information.
                Self::kill_stash(&stash, num_slashing_spans)?;
                // Remove the lock.
                T::Currency::remove_lock(STAKING_ID, &stash);

                T::WeightInfo::withdraw_unbonded_kill(num_slashing_spans)
            } else {
                // This was the consequence of a partial unbond. just update the ledger and move on.
                Self::update_ledger(controller, &ledger);

                // This is only an update, so we use less overall weight.
                T::WeightInfo::withdraw_unbonded_update(num_slashing_spans)
            };

        // `old_total` should never be less than the new total because
        // `consolidate_unlocked` strictly subtracts balance.
        if ledger.total < old_total {
            // Already checked that this won't overflow by entry condition.
            let value = old_total - ledger.total;
            Self::deposit_event(Event::<T>::Withdrawn { stash, amount: value });
        }

        Ok(used_weight)
    }

    pub(super) fn do_payout_stakers(
        validator_stash: T::AccountId,
        era: EraIndex,
    ) -> DispatchResultWithPostInfo {
        // Validate input data
        let current_era = CurrentEra::<T>::get().ok_or_else(|| {
            Error::<T>::InvalidEraToReward
                .with_weight(T::WeightInfo::payout_stakers_alive_staked(0))
        })?;
        let history_depth = T::HistoryDepth::get();
        ensure!(
            era <= current_era && era >= current_era.saturating_sub(history_depth),
            Error::<T>::InvalidEraToReward
                .with_weight(T::WeightInfo::payout_stakers_alive_staked(0))
        );

        // Note: if era has no reward to be claimed, era may be future. better not to update
        // `ledger.claimed_rewards` in this case.
        let era_payout = <ErasValidatorReward<T>>::get(era).ok_or_else(|| {
            Error::<T>::InvalidEraToReward
                .with_weight(T::WeightInfo::payout_stakers_alive_staked(0))
        })?;

        let controller = Self::bonded(&validator_stash).ok_or_else(|| {
            Error::<T>::NotStash.with_weight(T::WeightInfo::payout_stakers_alive_staked(0))
        })?;
        let mut ledger = <Ledger<T>>::get(&controller).ok_or(Error::<T>::NotController)?;

        ledger
            .claimed_rewards
            .retain(|&x| x >= current_era.saturating_sub(history_depth));

        match ledger.claimed_rewards.binary_search(&era) {
            Ok(_) => {
                return Err(Error::<T>::AlreadyClaimed
                    .with_weight(T::WeightInfo::payout_stakers_alive_staked(0)))
            },
            Err(pos) => ledger
                .claimed_rewards
                .try_insert(pos, era)
                // Since we retain era entries in `claimed_rewards` only upto
                // `HistoryDepth`, following bound is always expected to be
                // satisfied.
                .defensive_map_err(|_| Error::<T>::BoundNotMet)?,
        }

        // Input data seems good, no errors allowed after this point

        <Ledger<T>>::insert(&controller, &ledger);

        // Get Era reward points. It has TOTAL and INDIVIDUAL
        // Find the fraction of the era reward that belongs to the validator
        // Take that fraction of the eras rewards to split to nominator and validator
        //
        // Then look at the validator, figure out the proportion of their reward
        // which goes to them and each of their nominators.

        let era_reward_points = <ErasRewardPoints<T>>::get(era);
        let total_reward_points = era_reward_points.total;
        let validator_reward_points = era_reward_points
            .individual
            .get(&ledger.stash)
            .copied()
            .unwrap_or_else(Zero::zero);

        // Nothing to do if they have no reward points.
        if validator_reward_points.is_zero() {
            return Ok(Some(T::WeightInfo::payout_stakers_alive_staked(0)).into());
        }

        // This is the fraction of the total reward that the validator and the
        // nominators will get.
        let validator_total_reward_part =
            Perbill::from_rational(validator_reward_points, total_reward_points);

        let validator_total_payout = validator_total_reward_part * era_payout;

        Self::deposit_event(Event::<T>::PayoutStarted {
            era_index: era,
            validator_stash: ledger.stash.clone(),
        });

        let mut total_imbalance = PositiveImbalanceOf::<T>::zero();
        // We can now make total validator payout:
        if let Some(imbalance) = Self::make_payout(&ledger.stash, validator_total_payout) {
            Self::deposit_event(Event::<T>::Rewarded {
                stash: ledger.stash,
                amount: imbalance.peek(),
            });
            total_imbalance.subsume(imbalance);
        }

        T::Reward::on_unbalanced(total_imbalance);
        Ok(Some(T::WeightInfo::payout_stakers_alive_staked(0)).into())
    }

    /// Update the ledger for a controller.
    ///
    /// This will also update the stash lock.
    pub(crate) fn update_ledger(controller: &T::AccountId, ledger: &StakingLedger<T>) {
        T::Currency::set_lock(STAKING_ID, &ledger.stash, ledger.total, WithdrawReasons::all());
        <Ledger<T>>::insert(controller, ledger);
    }

    /// Chill a stash account.
    pub(crate) fn chill_stash(stash: &T::AccountId) {
        if Self::do_remove_validator(stash) {
            Self::deposit_event(Event::<T>::Chilled { stash: stash.clone() });
        }
    }

    /// Actually make a payment to a staker. This uses the currency's reward function
    /// to pay the right payee for the given staker account.
    fn make_payout(stash: &T::AccountId, amount: BalanceOf<T>) -> Option<PositiveImbalanceOf<T>> {
        let dest = Self::payee(stash);
        match dest {
            RewardDestination::Controller => Self::bonded(stash)
                .map(|controller| T::Currency::deposit_creating(&controller, amount)),
            RewardDestination::Stash => T::Currency::deposit_into_existing(stash, amount).ok(),
            RewardDestination::Staked => Self::bonded(stash)
                .and_then(|c| Self::ledger(&c).map(|l| (c, l)))
                .and_then(|(controller, mut l)| {
                    l.active += amount;
                    l.total += amount;
                    let r = T::Currency::deposit_into_existing(stash, amount).ok();
                    Self::update_ledger(&controller, &l);
                    r
                }),
            RewardDestination::Account(dest_account) => {
                Some(T::Currency::deposit_creating(&dest_account, amount))
            },
            RewardDestination::None => None,
        }
    }

    /// Plan a new session potentially trigger a new era.
    fn new_session(session_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        if let Some(current_era) = Self::current_era() {
            // Initial era has been set.
            let current_era_start_session_index = Self::eras_start_session_index(current_era)
                .unwrap_or_else(|| {
                    frame_support::print("Error: start_session_index must be set for current_era");
                    0
                });

            let era_length = session_index.saturating_sub(current_era_start_session_index); // Must never happen.

            match ForceEra::<T>::get() {
                // Will be set to `NotForcing` again if a new era has been triggered.
                Forcing::ForceNew => (),
                // Short circuit to `try_trigger_new_era`.
                Forcing::ForceAlways => (),
                // Only go to `try_trigger_new_era` if deadline reached.
                Forcing::NotForcing if era_length >= T::SessionsPerEra::get() => (),
                _ => {
                    // Either `Forcing::ForceNone`,
                    // or `Forcing::NotForcing if era_length >= T::SessionsPerEra::get()`.
                    return None;
                },
            }

            // New era.
            let maybe_new_era_validators = Self::try_trigger_new_era(session_index);
            if maybe_new_era_validators.is_some()
                && matches!(ForceEra::<T>::get(), Forcing::ForceNew)
            {
                Self::set_force_era(Forcing::NotForcing);
            }

            maybe_new_era_validators
        } else {
            // Set initial era.
            log!(debug, "Starting the first era.");
            Self::try_trigger_new_era(session_index)
        }
    }

    /// Potentially plan a new era.
    ///
    /// Get election result from `T::ElectionProvider`.
    /// In case election result has more than [`MinimumValidatorCount`] validator trigger a new era.
    ///
    /// In case a new era is planned, the new validator set is returned.
    pub(crate) fn try_trigger_new_era(
        start_session_index: SessionIndex,
    ) -> Option<Vec<T::AccountId>> {
        let num_of_validators = Validators::<T>::count();
        if num_of_validators < Self::minimum_validator_count().max(1) {
            // Session will panic if we ever return an empty validator set, thus max(1) ^^.
            match CurrentEra::<T>::get() {
                Some(current_era) if current_era > 0 => log!(
                    warn,
                    "chain does not have enough staking candidates to operate for era {:?} ({} \
                    validators, minimum is {})",
                    CurrentEra::<T>::get().unwrap_or(0),
                    num_of_validators,
                    Self::minimum_validator_count(),
                ),
                None => {
                    // The initial era is allowed to have no exposures.
                    // In this case the SessionManager is expected to choose a sensible validator
                    // set.
                    // TODO: this should be simplified #8911
                    CurrentEra::<T>::put(0);
                    ErasStartSessionIndex::<T>::insert(0, start_session_index);
                },
                _ => (),
            }

            return None;
        }

        Some(Self::trigger_new_era(start_session_index))
    }

    /// Plan a new era.
    ///
    /// * Bump the current era storage (which holds the latest planned era).
    /// * Store start session index for the new planned era.
    /// * Clean old era information.
    /// * Store staking information for the new planned era
    ///
    /// Returns the new validator set.
    pub fn trigger_new_era(start_session_index: SessionIndex) -> Vec<T::AccountId> {
        // Increment or set current era.
        let new_planned_era = CurrentEra::<T>::mutate(|s| {
            *s = Some(s.map(|s| s + 1).unwrap_or(0));
            s.unwrap()
        });
        ErasStartSessionIndex::<T>::insert(new_planned_era, start_session_index);

        // Clean old era information.
        if let Some(old_era) = new_planned_era.checked_sub(T::HistoryDepth::get() + 1) {
            Self::clear_era_information(old_era);
        }

        // Set staking information for the new era.
        Self::store_stakers_info(new_planned_era)
    }

    /// Store staking information for the new planned era
    pub fn store_stakers_info(new_planned_era: EraIndex) -> Vec<T::AccountId> {
        let mut total_stake: BalanceOf<T> = Zero::zero();
        Validators::<T>::iter().for_each(|(stash, _prefs)| {
            let stake = Self::slashable_balance_of(&stash);
            total_stake = total_stake.saturating_add(stake);
            <ErasStakers<T>>::insert(new_planned_era, &stash, Exposure(stake));

            let pref = Self::validators(&stash);
            <ErasValidatorPrefs<T>>::insert(new_planned_era, stash, pref);
        });

        // Insert current era staking information
        <ErasTotalStake<T>>::insert(new_planned_era, total_stake);

        if new_planned_era > 0 {
            log!(
                info,
                "new validator set of size {:?} has been processed for era {:?}",
                Validators::<T>::count(),
                new_planned_era,
            );
        }

        Validators::<T>::iter().map(|(x, _)| x).collect()
    }

    /// Start a session potentially starting an era.
    fn start_session(start_session: SessionIndex) {
        let next_active_era = Self::active_era().map(|e| e.index + 1).unwrap_or(0);
        // This is only `Some` when current era has already progressed to the next era, while the
        // active era is one behind (i.e. in the *last session of the active era*, or *first session
        // of the new current era*, depending on how you look at it).
        if let Some(next_active_era_start_session_index) =
            Self::eras_start_session_index(next_active_era)
        {
            match next_active_era_start_session_index.cmp(&start_session) {
                Ordering::Equal => Self::start_era(start_session),
                Ordering::Less => {
                    // This arm should never happen, but better handle it than to stall the staking
                    // pallet.
                    frame_support::print("Warning: A session appears to have been skipped.");
                    Self::start_era(start_session);
                },
                _ => (),
            }
        }

        // disable all offending validators that have been disabled for the whole era
        for (index, disabled) in <OffendingValidators<T>>::get() {
            if disabled {
                T::SessionInterface::disable_validator(index);
            }
        }
    }

    /// End a session potentially ending an era.
    fn end_session(session_index: SessionIndex) {
        if let Some(active_era) = Self::active_era() {
            if let Some(next_active_era_start_session_index) =
                Self::eras_start_session_index(active_era.index + 1)
            {
                if next_active_era_start_session_index == session_index + 1 {
                    Self::end_era(active_era, session_index);
                }
            }
        }
    }

    /// Start a new era. It does:
    ///
    /// * Increment `active_era.index`,
    /// * reset `active_era.start`,
    /// * update `BondedEras` and apply slashes.
    fn start_era(start_session: SessionIndex) {
        let active_era = ActiveEra::<T>::mutate(|active_era| {
            let new_index = active_era.as_ref().map(|info| info.index + 1).unwrap_or(0);
            *active_era = Some(ActiveEraInfo {
                index: new_index,
                // Set new active era start in next `on_finalize`. To guarantee usage of `Time`
                start: None,
            });
            new_index
        });

        let bonding_duration = T::BondingDuration::get();

        BondedEras::<T>::mutate(|bonded| {
            bonded.push((active_era, start_session));

            if active_era > bonding_duration {
                let first_kept = active_era - bonding_duration;

                // Prune out everything that's from before the first-kept index.
                let n_to_prune =
                    bonded.iter().take_while(|&&(era_idx, _)| era_idx < first_kept).count();

                // Kill slashing metadata.
                for (pruned_era, _) in bonded.drain(..n_to_prune) {
                    slashing::clear_era_metadata::<T>(pruned_era);
                }

                if let Some(&(_, first_session)) = bonded.first() {
                    T::SessionInterface::prune_historical_up_to(first_session);
                }
            }
        });

        Self::apply_unapplied_slashes(active_era);
    }

    /// Compute payout for era.
    fn end_era(active_era: ActiveEraInfo, _session_index: SessionIndex) {
        // Note: active_era_start can be None if end era is called during genesis config.
        if let Some(active_era_start) = active_era.start {
            let now_as_millis_u64 = T::UnixTime::now().as_millis().saturated_into::<u64>();

            let era_duration = (now_as_millis_u64 - active_era_start).saturated_into::<u64>();
            let staked = Self::eras_total_stake(active_era.index);
            let issuance = T::Currency::total_issuance();
            let (validator_payout, remainder) =
                T::EraPayout::era_payout(staked, issuance, era_duration);

            Self::deposit_event(Event::<T>::EraPaid {
                era_index: active_era.index,
                validator_payout,
                remainder,
            });

            // Set ending era reward.
            <ErasValidatorReward<T>>::insert(active_era.index, validator_payout);
            T::RewardRemainder::on_unbalanced(T::Currency::issue(remainder));

            // Clear offending validators.
            <OffendingValidators<T>>::kill();
        }
    }

    /// Remove all associated data of a stash account from the staking system.
    ///
    /// Assumes storage is upgraded before calling.
    ///
    /// This is called:
    /// - after a `withdraw_unbonded()` call that frees all of a stash's bonded balance.
    /// - through `reap_stash()` if the balance has fallen to zero (through slashing).
    pub(crate) fn kill_stash(stash: &T::AccountId, num_slashing_spans: u32) -> DispatchResult {
        let controller = <Bonded<T>>::get(stash).ok_or(Error::<T>::NotStash)?;

        slashing::clear_stash_metadata::<T>(stash, num_slashing_spans)?;

        <Bonded<T>>::remove(stash);
        <Ledger<T>>::remove(&controller);

        <Payee<T>>::remove(stash);
        Self::do_remove_validator(stash);

        frame_system::Pallet::<T>::dec_consumers(stash);

        Ok(())
    }

    /// Clear all era information for given era.
    pub(crate) fn clear_era_information(era_index: EraIndex) {
        #[allow(deprecated)]
        <ErasStakers<T>>::remove_prefix(era_index, None);
        #[allow(deprecated)]
        <ErasStakersClipped<T>>::remove_prefix(era_index, None);
        #[allow(deprecated)]
        <ErasValidatorPrefs<T>>::remove_prefix(era_index, None);
        <ErasValidatorReward<T>>::remove(era_index);
        <ErasRewardPoints<T>>::remove(era_index);
        <ErasTotalStake<T>>::remove(era_index);
        ErasStartSessionIndex::<T>::remove(era_index);
    }

    /// Apply previously-unapplied slashes on the beginning of a new era, after a delay.
    fn apply_unapplied_slashes(active_era: EraIndex) {
        let era_slashes = UnappliedSlashes::<T>::take(active_era);
        log!(
            debug,
            "found {} slashes scheduled to be executed in era {:?}",
            era_slashes.len(),
            active_era,
        );
        for slash in era_slashes {
            let slash_era = active_era.saturating_sub(T::SlashDeferDuration::get());
            slashing::apply_slash::<T>(slash, slash_era);
        }
    }

    /// Add reward points to validators using their stash account ID.
    ///
    /// Validators are keyed by stash account ID and must be in the current elected set.
    ///
    /// For each element in the iterator the given number of points in u32 is added to the
    /// validator, thus duplicates are handled.
    ///
    /// At the end of the era each the total payout will be distributed among validator
    /// relatively to their points.
    ///
    /// COMPLEXITY: Complexity is `number_of_validator_to_reward x current_elected_len`.
    pub fn reward_by_ids(validators_points: impl IntoIterator<Item = (T::AccountId, u32)>) {
        if let Some(active_era) = Self::active_era() {
            <ErasRewardPoints<T>>::mutate(active_era.index, |era_rewards| {
                for (validator, points) in validators_points.into_iter() {
                    *era_rewards.individual.entry(validator).or_default() += points;
                    era_rewards.total += points;
                }
            });
        }
    }

    /// Helper to set a new `ForceEra` mode.
    pub(crate) fn set_force_era(mode: Forcing) {
        log!(info, "Setting force era mode {:?}.", mode);
        ForceEra::<T>::put(mode);
        Self::deposit_event(Event::<T>::ForceEra { mode });
    }

    /// Ensures that at the end of the current session there will be a new era.
    pub(crate) fn ensure_new_era() {
        match ForceEra::<T>::get() {
            Forcing::ForceAlways | Forcing::ForceNew => (),
            _ => Self::set_force_era(Forcing::ForceNew),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub fn add_era_stakers(
        current_era: EraIndex,
        stash: T::AccountId,
        exposure: Exposure<BalanceOf<T>>,
    ) {
        <ErasStakers<T>>::insert(&current_era, &stash, &exposure);
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub fn set_slash_reward_fraction(fraction: Perbill) {
        SlashRewardFraction::<T>::put(fraction);
    }

    /// This function will remove a validator from the `Validators` storage map.
    ///
    /// Returns true if `who` was removed from `Validators`, otherwise false.
    ///
    /// NOTE: you must ALWAYS use this function to remove a validator from the system. Any access to
    /// `Validators` or `VoterList` outside of this function is almost certainly
    /// wrong.
    pub fn do_remove_validator(who: &T::AccountId) -> bool {
        if Validators::<T>::contains_key(who) {
            Validators::<T>::remove(who);
            true
        } else {
            false
        }
    }
}

/// In this implementation `new_session(session)` must be called before `end_session(session-1)`
/// i.e. the new session must be planned before the ending of the previous session.
///
/// Once the first new_session is planned, all session must start and then end in order, though
/// some session can lag in between the newest session planned and the latest session started.
impl<T: Config> pallet_session::SessionManager<T::AccountId> for Pallet<T> {
    fn new_session(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        log!(trace, "planning new session {}", new_index);
        CurrentPlannedSession::<T>::put(new_index);
        Self::new_session(new_index)
    }
    fn new_session_genesis(new_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        log!(trace, "planning new session {} at genesis", new_index);
        CurrentPlannedSession::<T>::put(new_index);
        Self::new_session(new_index)
    }
    fn start_session(start_index: SessionIndex) {
        log!(trace, "starting session {}", start_index);
        Self::start_session(start_index)
    }
    fn end_session(end_index: SessionIndex) {
        log!(trace, "ending session {}", end_index);
        Self::end_session(end_index)
    }
}

impl<T: Config> historical::SessionManager<T::AccountId, Exposure<BalanceOf<T>>> for Pallet<T> {
    fn new_session(new_index: SessionIndex) -> Option<Vec<(T::AccountId, Exposure<BalanceOf<T>>)>> {
        <Self as pallet_session::SessionManager<_>>::new_session(new_index).map(|validators| {
            let current_era = Self::current_era()
                // Must be some as a new era has been created.
                .unwrap_or(0);

            validators
                .into_iter()
                .map(|v| {
                    let exposure = Self::eras_stakers(current_era, &v);
                    (v, exposure)
                })
                .collect()
        })
    }
    fn new_session_genesis(
        new_index: SessionIndex,
    ) -> Option<Vec<(T::AccountId, Exposure<BalanceOf<T>>)>> {
        <Self as pallet_session::SessionManager<_>>::new_session_genesis(new_index).map(
            |validators| {
                let current_era = Self::current_era()
                    // Must be some as a new era has been created.
                    .unwrap_or(0);

                validators
                    .into_iter()
                    .map(|v| {
                        let exposure = Self::eras_stakers(current_era, &v);
                        (v, exposure)
                    })
                    .collect()
            },
        )
    }
    fn start_session(start_index: SessionIndex) {
        <Self as pallet_session::SessionManager<_>>::start_session(start_index)
    }
    fn end_session(end_index: SessionIndex) {
        <Self as pallet_session::SessionManager<_>>::end_session(end_index)
    }
}

/// Add reward points to block authors:
/// * 20 points to the block producer for producing a (non-uncle) block,
impl<T> pallet_authorship::EventHandler<T::AccountId, T::BlockNumber> for Pallet<T>
where
    T: Config + pallet_authorship::Config + pallet_session::Config,
{
    fn note_author(author: T::AccountId) {
        Self::reward_by_ids(vec![(author, 20)])
    }
}

/// This is intended to be used with `FilterHistoricalOffences`.
impl<T: Config>
    OnOffenceHandler<T::AccountId, pallet_session::historical::IdentificationTuple<T>, Weight>
    for Pallet<T>
where
    T: pallet_session::Config<ValidatorId = <T as frame_system::Config>::AccountId>,
    T: pallet_session::historical::Config<
        FullIdentification = Exposure<BalanceOf<T>>,
        FullIdentificationOf = ExposureOf<T>,
    >,
    T::SessionHandler: pallet_session::SessionHandler<<T as frame_system::Config>::AccountId>,
    T::SessionManager: pallet_session::SessionManager<<T as frame_system::Config>::AccountId>,
    T::ValidatorIdOf: Convert<
        <T as frame_system::Config>::AccountId,
        Option<<T as frame_system::Config>::AccountId>,
    >,
{
    fn on_offence(
        offenders: &[OffenceDetails<
            T::AccountId,
            pallet_session::historical::IdentificationTuple<T>,
        >],
        slash_fraction: &[Perbill],
        slash_session: SessionIndex,
        disable_strategy: DisableStrategy,
    ) -> Weight {
        let reward_proportion = SlashRewardFraction::<T>::get();
        let mut consumed_weight = Weight::from_parts(0, 0);
        let mut add_db_reads_writes = |reads, writes| {
            consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
        };

        let active_era = {
            let active_era = Self::active_era();
            add_db_reads_writes(1, 0);
            if active_era.is_none() {
                // This offence need not be re-submitted.
                return consumed_weight;
            }
            active_era.expect("value checked not to be `None`; qed").index
        };
        let active_era_start_session_index = Self::eras_start_session_index(active_era)
            .unwrap_or_else(|| {
                frame_support::print("Error: start_session_index must be set for current_era");
                0
            });
        add_db_reads_writes(1, 0);

        let window_start = active_era.saturating_sub(T::BondingDuration::get());

        // Fast path for active-era report - most likely.
        // `slash_session` cannot be in a future active era. It must be in `active_era` or before.
        let slash_era = if slash_session >= active_era_start_session_index {
            active_era
        } else {
            let eras = BondedEras::<T>::get();
            add_db_reads_writes(1, 0);

            // Reverse because it's more likely to find reports from recent eras.
            match eras.iter().rev().find(|&(_, sesh)| sesh <= &slash_session) {
                Some((slash_era, _)) => *slash_era,
                // Before bonding period. defensive - should be filtered out.
                None => return consumed_weight,
            }
        };

        add_db_reads_writes(1, 1);

        let slash_defer_duration = T::SlashDeferDuration::get();

        let invulnerables = Self::invulnerables();
        add_db_reads_writes(1, 0);

        for (details, slash_fraction) in offenders.iter().zip(slash_fraction) {
            let (stash, exposure) = &details.offender;

            // Skip if the validator is invulnerable.
            if invulnerables.contains(stash) {
                continue;
            }

            let unapplied = slashing::compute_slash::<T>(slashing::SlashParams {
                stash,
                slash: *slash_fraction,
                exposure,
                slash_era,
                window_start,
                now: active_era,
                reward_proportion,
                disable_strategy,
            });

            Self::deposit_event(Event::<T>::SlashReported {
                validator: stash.clone(),
                fraction: *slash_fraction,
                slash_era,
            });

            if let Some(mut unapplied) = unapplied {
                let reporters_len = details.reporters.len() as u64;

                {
                    let upper_bound = 1 /* Validator/NominatorSlashInEra */ + 2 /* fetch_spans */;
                    add_db_reads_writes(upper_bound, upper_bound);
                }
                unapplied.reporters = details.reporters.clone();
                if slash_defer_duration == 0 {
                    // Apply right away.
                    slashing::apply_slash::<T>(unapplied, slash_era);
                    {
                        let slash_cost = (6, 5);
                        let reward_cost = (2, 2);
                        add_db_reads_writes(
                            slash_cost.0 + reward_cost.0 * reporters_len,
                            slash_cost.1 + reward_cost.1 * reporters_len,
                        );
                    }
                } else {
                    // Defer to end of some `slash_defer_duration` from now.
                    log!(
                        debug,
                        "deferring slash of {:?}% happened in {:?} (reported in {:?}) to {:?}",
                        slash_fraction,
                        slash_era,
                        active_era,
                        slash_era + slash_defer_duration + 1,
                    );
                    UnappliedSlashes::<T>::mutate(
                        slash_era.saturating_add(slash_defer_duration).saturating_add(One::one()),
                        move |for_later| for_later.push(unapplied),
                    );
                    add_db_reads_writes(1, 1);
                }
            } else {
                add_db_reads_writes(4 /* fetch_spans */, 5 /* kick_out_if_recent */)
            }
        }

        consumed_weight
    }
}

// /// A simple sorted list implementation that does not require any additional pallets. Note, this
// /// does not provide validators in sorted order. If you desire nominators in a sorted order take
// /// a look at [`pallet-bags-list`].
// pub struct UseValidatorsMap<T>(sp_std::marker::PhantomData<T>);
// impl<T: Config> SortedListProvider<T::AccountId> for UseValidatorsMap<T> {
//     type Score = BalanceOf<T>;
//     type Error = ();
//
//     /// Returns iterator over voter list, which can have `take` called on it.
//     fn iter() -> Box<dyn Iterator<Item = T::AccountId>> {
//         Box::new(Validators::<T>::iter().map(|(v, _)| v))
//     }
//     fn iter_from(
//         start: &T::AccountId,
//     ) -> Result<Box<dyn Iterator<Item = T::AccountId>>, Self::Error> {
//         if Validators::<T>::contains_key(start) {
//             let start_key = Validators::<T>::hashed_key_for(start);
//             Ok(Box::new(Validators::<T>::iter_from(start_key).map(|(n, _)| n)))
//         } else {
//             Err(())
//         }
//     }
//     fn count() -> u32 {
//         Validators::<T>::count()
//     }
//     fn contains(id: &T::AccountId) -> bool {
//         Validators::<T>::contains_key(id)
//     }
//     fn on_insert(_: T::AccountId, _weight: Self::Score) -> Result<(), Self::Error> {
//         // nothing to do on insert.
//         Ok(())
//     }
//     fn get_score(id: &T::AccountId) -> Result<Self::Score, Self::Error> {
//         Ok(Pallet::<T>::weight_of(id).into())
//     }
//     fn on_update(_: &T::AccountId, _weight: Self::Score) -> Result<(), Self::Error> {
//         // nothing to do on update.
//         Ok(())
//     }
//     fn on_remove(_: &T::AccountId) -> Result<(), Self::Error> {
//         // nothing to do on remove.
//         Ok(())
//     }
//     fn unsafe_regenerate(
//         _: impl IntoIterator<Item = T::AccountId>,
//         _: Box<dyn Fn(&T::AccountId) -> Self::Score>,
//     ) -> u32 {
//         // nothing to do upon regenerate.
//         0
//     }
//     #[cfg(feature = "try-runtime")]
//     fn try_state() -> Result<(), &'static str> {
//         Ok(())
//     }
//
//     fn unsafe_clear() {
//         #[allow(deprecated)]
//         Validators::<T>::remove_all();
//     }
//
//     #[cfg(feature = "runtime-benchmarks")]
//     fn score_update_worst_case(_who: &T::AccountId, _is_increase: bool) -> Self::Score {
//         unimplemented!()
//     }
// }
//
// NOTE: in this entire impl block, the assumption is that `who` is a stash account.
impl<T: Config> StakingInterface for Pallet<T> {
    type AccountId = T::AccountId;
    type Balance = BalanceOf<T>;

    fn minimum_nominator_bond() -> Self::Balance {
        Self::Balance::max_value()
    }

    fn minimum_validator_bond() -> Self::Balance {
        MinValidatorBond::<T>::get()
    }

    fn desired_validator_count() -> u32 {
        ValidatorCount::<T>::get()
    }

    fn election_ongoing() -> bool {
        false
    }

    fn force_unstake(who: Self::AccountId) -> sp_runtime::DispatchResult {
        let num_slashing_spans = Self::slashing_spans(&who).map_or(0, |s| s.iter().count() as u32);
        Self::force_unstake(RawOrigin::Root.into(), who, num_slashing_spans)
    }

    fn stash_by_ctrl(controller: &Self::AccountId) -> Result<Self::AccountId, DispatchError> {
        Self::ledger(controller)
            .map(|l| l.stash)
            .ok_or(Error::<T>::NotController.into())
    }

    fn is_exposed_in_era(who: &Self::AccountId, era: &EraIndex) -> bool {
        ErasStakers::<T>::iter_prefix(era).any(|(validator, _)| validator == *who)
    }

    fn bonding_duration() -> EraIndex {
        T::BondingDuration::get()
    }

    fn current_era() -> EraIndex {
        Self::current_era().unwrap_or(Zero::zero())
    }

    fn stake(who: &Self::AccountId) -> Result<Stake<BalanceOf<T>>, DispatchError> {
        Self::bonded(who)
            .and_then(|c| Self::ledger(c))
            .map(|l| Stake { total: l.total, active: l.active })
            .ok_or(Error::<T>::NotStash.into())
    }

    fn bond_extra(who: &Self::AccountId, extra: Self::Balance) -> DispatchResult {
        Self::bond_extra(RawOrigin::Signed(who.clone()).into(), extra)
    }

    fn unbond(who: &Self::AccountId, value: Self::Balance) -> DispatchResult {
        let ctrl = Self::bonded(who).ok_or(Error::<T>::NotStash)?;
        Self::unbond(RawOrigin::Signed(ctrl).into(), value)
            .map_err(|with_post| with_post.error)
            .map(|_| ())
    }

    fn chill(who: &Self::AccountId) -> DispatchResult {
        // defensive-only: any account bonded via this interface has the stash set as the
        // controller, but we have to be sure. Same comment anywhere else that we read this.
        let ctrl = Self::bonded(who).ok_or(Error::<T>::NotStash)?;
        Self::chill(RawOrigin::Signed(ctrl).into())
    }

    fn withdraw_unbonded(
        who: Self::AccountId,
        num_slashing_spans: u32,
    ) -> Result<bool, DispatchError> {
        let ctrl = Self::bonded(who).ok_or(Error::<T>::NotStash)?;
        Self::withdraw_unbonded(RawOrigin::Signed(ctrl.clone()).into(), num_slashing_spans)
            .map(|_| !Ledger::<T>::contains_key(&ctrl))
            .map_err(|with_post| with_post.error)
    }

    fn bond(
        who: &Self::AccountId,
        value: Self::Balance,
        payee: &Self::AccountId,
    ) -> DispatchResult {
        Self::bond(
            RawOrigin::Signed(who.clone()).into(),
            T::Lookup::unlookup(who.clone()),
            value,
            RewardDestination::Account(payee.clone()),
        )
    }

    fn nominate(_who: &Self::AccountId, _validators: Vec<Self::AccountId>) -> DispatchResult {
        Ok(())
    }

    fn status(
        who: &Self::AccountId,
    ) -> Result<sp_staking::StakerStatus<Self::AccountId>, DispatchError> {
        let is_bonded = Self::bonded(who).is_some();
        if !is_bonded {
            return Err(Error::<T>::NotStash.into());
        }

        use sp_staking::StakerStatus;
        if Validators::<T>::contains_key(who) {
            Ok(StakerStatus::Validator)
        } else {
            Ok(StakerStatus::Idle)
        }
    }

    sp_staking::runtime_benchmarks_enabled! {
        fn nominations(who: &Self::AccountId) -> Option<Vec<T::AccountId>> {
            None
        }

        fn set_current_era(era: EraIndex) {
            CurrentEra::<T>::put(era);
        }
    }
}

#[cfg(any(test, feature = "try-runtime"))]
impl<T: Config> Pallet<T> {
    pub(crate) fn do_try_state(_: crate::pallet::BlockNumberFor<T>) -> Result<(), &'static str> {
        Self::check_ledgers()
    }

    fn check_ledgers() -> Result<(), &'static str> {
        Bonded::<T>::iter().try_for_each(|(_, ctrl)| Self::ensure_ledger_consistent(ctrl))
    }

    #[allow(unused)]
    fn ensure_is_stash(who: &T::AccountId) -> Result<(), &'static str> {
        ensure!(Self::bonded(who).is_some(), "Not a stash.");
        Ok(())
    }

    fn ensure_ledger_consistent(ctrl: T::AccountId) -> Result<(), &'static str> {
        // ensures ledger.total == ledger.active + sum(ledger.unlocking).
        let ledger = Self::ledger(ctrl.clone()).ok_or("Not a controller.")?;
        let real_total: BalanceOf<T> =
            ledger.unlocking.iter().fold(ledger.active, |a, c| a + c.value);
        ensure!(real_total == ledger.total, "ledger.total corrupt");

        if !(ledger.active >= T::Currency::minimum_balance() || ledger.active.is_zero()) {
            log!(warn, "ledger.active less than ED: {:?}, {:?}", ctrl, ledger)
        }

        Ok(())
    }
}
