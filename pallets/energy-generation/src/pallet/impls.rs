//! Implementations for the Staking FRAME Pallet.

use core::cmp::Ordering;

use frame_support::{
    dispatch::WithPostDispatchInfo,
    pallet_prelude::*,
    storage::bounded_btree_set::BoundedBTreeSet,
    traits::{
        tokens::{fungibles::Balanced, Precision},
        Currency, DefensiveResult, Get, LockableCurrency, OnUnbalanced, WithdrawReasons,
    },
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::GetByKey;
use scale_info::prelude::*;

use pallet_reputation::{ReputationPoint, ReputationRecord};
use pallet_session::historical;
use sp_runtime::{
    traits::{Convert, One, Saturating, Zero},
    Perbill,
};
use sp_staking::{
    offence::{DisableStrategy, OffenceDetails, OnOffenceHandler},
    EraIndex, SessionIndex,
};
use sp_std::prelude::*;

use crate::{
    log, slashing, weights::WeightInfo, ActiveEraInfo, Cooperations, EnergyDebtOf, EnergyOf,
    EnergyRateCalculator, Exposure, ExposureOf, Forcing, IndividualExposure, RewardDestination,
    SessionInterface, StakeOf, StakingLedger, ValidatorPrefs,
};

use super::{pallet::*, STAKING_ID};

impl<T: Config> Pallet<T> {
    /// Checks if the account has enough reputation to be a validator.
    pub fn is_legit_for_validator(stash: &T::AccountId) -> bool {
        match pallet_reputation::AccountReputation::<T>::get(stash) {
            Some(record) => record
                .reputation
                .tier()
                .map(|tier| tier >= T::ValidatorReputationTier::get())
                .unwrap_or(false),
            None => false,
        }
    }

    /// Minimum stake to be a validator depends on NAC level.
    pub fn min_bond_for_validator(stash: &T::AccountId) -> StakeOf<T> {
        match pallet_nac_managing::Pallet::<T>::get_nac_level(stash) {
            Some((level, _)) => {
                if level > 1 {
                    MinTrustValidatorBond::<T>::get()
                } else {
                    MinCommonValidatorBond::<T>::get()
                }
            },
            None => MinCommonValidatorBond::<T>::get(),
        }
    }

    /// Check if the account has enough reputation for collaborative staking.
    pub fn is_legit_for_collab(stash: &T::AccountId) -> bool {
        match pallet_reputation::AccountReputation::<T>::get(stash) {
            Some(record) => record
                .reputation
                .tier()
                .map(|tier| tier >= T::CollaborativeValidatorReputationTier::get())
                .unwrap_or(false),
            None => false,
        }
    }

    pub(crate) fn check_reputation_validator(acc: &T::AccountId) {
        if !Self::is_legit_for_validator(acc) {
            Self::chill_stash(acc);
        }
    }

    pub(crate) fn try_check_reputation_collab(acc: &T::AccountId) {
        let prefs = Self::validators(acc);

        if prefs.collaborative && !Self::is_legit_for_collab(acc) {
            Self::chill_stash(acc);
        }
    }

    pub(crate) fn check_reputation_cooperator(validator: &T::AccountId, cooperator: &T::AccountId) {
        let prefs = Self::validators(validator);
        let record = pallet_reputation::AccountReputation::<T>::get(cooperator)
            .unwrap_or_else(ReputationRecord::with_now::<T>);

        if prefs.min_coop_reputation > record.reputation {
            Self::chill_stash(cooperator);
        }
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
            if ledger.unlocking.is_empty() && ledger.active < T::StakeCurrency::minimum_balance() {
                // This account must have called `unbond()` with some value that caused the active
                // portion to fall below existential deposit + will have no more unlocking chunks
                // left. We can now safely remove all staking-related information.
                Self::kill_stash(&stash, num_slashing_spans)?;
                // Remove the lock.
                T::StakeCurrency::remove_lock(STAKING_ID, &stash);

                T::ThisWeightInfo::withdraw_unbonded_kill(num_slashing_spans)
            } else {
                // This was the consequence of a partial unbond. just update the ledger and move on.
                Self::update_ledger(controller, &ledger);

                // This is only an update, so we use less overall weight.
                T::ThisWeightInfo::withdraw_unbonded_update(num_slashing_spans)
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

    /// In the original `pallet_staking` this method applies points to calculate rewards when the
    /// era is finished.
    ///
    /// But we use repuation instead of points. Also, it's not per era, but for the whole time.
    pub fn reward_by_ids(
        validators_points: impl IntoIterator<Item = (T::AccountId, ReputationPoint)>,
    ) {
        for (validator, points) in validators_points.into_iter() {
            pallet_reputation::Pallet::<T>::increase_creating(&validator, points);
        }
    }

    pub(super) fn do_payout_stakers(
        validator_stash: T::AccountId,
        era: EraIndex,
    ) -> DispatchResultWithPostInfo {
        // Validate input data
        let active_era = Self::active_era().map(|v| v.index).unwrap_or_default();
        ensure!(
            era != active_era,
            Error::<T>::InvalidEraToReward
                .with_weight(T::ThisWeightInfo::payout_stakers_alive_staked(0))
        );

        let current_era = CurrentEra::<T>::get().ok_or_else(|| {
            Error::<T>::InvalidEraToReward
                .with_weight(T::ThisWeightInfo::payout_stakers_alive_staked(0))
        })?;
        let history_depth = T::HistoryDepth::get();
        ensure!(
            era <= current_era && era >= current_era.saturating_sub(history_depth),
            Error::<T>::InvalidEraToReward
                .with_weight(T::ThisWeightInfo::payout_stakers_alive_staked(0))
        );

        // Note: if era has no reward to be claimed, era may be future. better not to update
        // `ledger.claimed_rewards` in this case.
        let era_energy_rate = <ErasEnergyPerStakeCurrency<T>>::get(era).ok_or_else(|| {
            Error::<T>::InvalidEraToReward
                .with_weight(T::ThisWeightInfo::payout_stakers_alive_staked(0))
        })?;

        let controller = Self::bonded(&validator_stash).ok_or_else(|| {
            Error::<T>::NotStash.with_weight(T::ThisWeightInfo::payout_stakers_alive_staked(0))
        })?;
        let mut ledger = <Ledger<T>>::get(&controller).ok_or(Error::<T>::NotController)?;

        ledger
            .claimed_rewards
            .retain(|&x| x >= current_era.saturating_sub(history_depth));

        match ledger.claimed_rewards.binary_search(&era) {
            Ok(_) => {
                return Err(Error::<T>::AlreadyClaimed
                    .with_weight(T::ThisWeightInfo::payout_stakers_alive_staked(0)))
            },
            Err(pos) => ledger
                .claimed_rewards
                .try_insert(pos, era)
                // Since we retain era entries in `claimed_rewards` only upto
                // `HistoryDepth`, following bound is always expected to be
                // satisfied.
                .defensive_map_err(|_| Error::<T>::BoundNotMet)?,
        }

        let exposure = <ErasStakersClipped<T>>::get(era, &ledger.stash);

        // Input data seems good, no errors allowed after this point

        <Ledger<T>>::insert(&controller, &ledger);

        let validator_total_payout = exposure.total.into() / era_energy_rate;

        let validator_prefs = Self::eras_validator_prefs(era, &validator_stash);
        // Validator first gets a cut off the top.
        let validator_commission = validator_prefs.commission;
        let validator_commission_payout = validator_commission * validator_total_payout;

        let validator_leftover_payout = validator_total_payout - validator_commission_payout;
        // Now let's calculate how this is split to the validator.
        let validator_exposure_part = Perbill::from_rational(exposure.own, exposure.total);
        let validator_staking_payout = validator_exposure_part * validator_leftover_payout;

        Self::deposit_event(Event::<T>::PayoutStarted {
            era_index: era,
            validator_stash: ledger.stash.clone(),
        });

        let mut total_imbalance = EnergyDebtOf::<T>::zero(T::EnergyAssetId::get());
        // We can now make total validator payout:
        if let Some(imbalance) =
            Self::make_payout(&ledger.stash, validator_staking_payout + validator_commission_payout)
        {
            Self::deposit_event(Event::<T>::Rewarded {
                stash: ledger.stash,
                amount: imbalance.peek(),
            });
            total_imbalance.subsume(imbalance).unwrap_or_default();
        }

        // Track the number of payout ops to cooperators. Note:
        // `WeightInfo::payout_stakers_alive_staked` always assumes at least a validator is paid
        // out, so we do not need to count their payout op.
        let mut cooperator_payout_count: u32 = 0;

        // Lets now calculate how this is split to the cooperators.
        // Reward only the clipped exposures. Note this is not necessarily sorted.
        for cooperator in exposure.others.iter() {
            let cooperator_exposure_part = Perbill::from_rational(cooperator.value, exposure.total);

            let cooperator_reward: EnergyOf<T> =
                cooperator_exposure_part * validator_leftover_payout;
            // We can now make cooperator payout:
            if let Some(imbalance) = Self::make_payout(&cooperator.who, cooperator_reward) {
                // Note: this logic does not count payouts for `RewardDestination::None`.
                cooperator_payout_count += 1;
                let e = Event::<T>::Rewarded {
                    stash: cooperator.who.clone(),
                    amount: imbalance.peek(),
                };
                Self::deposit_event(e);
                total_imbalance.subsume(imbalance).unwrap_or_default();
            }
        }

        T::Reward::on_unbalanced(total_imbalance);
        debug_assert!(cooperator_payout_count <= T::MaxCooperatorRewardedPerValidator::get());
        Ok(Some(T::ThisWeightInfo::payout_stakers_alive_staked(cooperator_payout_count)).into())
    }

    /// Actually make a payment to a staker. This uses the currency's reward function
    /// to pay the right payee for the given staker account.
    fn make_payout(stash: &T::AccountId, amount: EnergyOf<T>) -> Option<EnergyDebtOf<T>> {
        let dest = Self::payee(stash);
        let asset_id = T::EnergyAssetId::get();
        let amount = Self::calculate_energy_reward_multiplier(stash)
            .mul_floor(amount)
            .saturating_add(amount);

        match dest {
            RewardDestination::Controller => Self::bonded(stash).and_then(|controller| {
                pallet_assets::Pallet::<T>::deposit(asset_id, &controller, amount, Precision::Exact)
                    .ok()
            }),

            RewardDestination::Stash => {
                pallet_assets::Pallet::<T>::deposit(asset_id, stash, amount, Precision::Exact).ok()
            },

            RewardDestination::Account(dest_account) => pallet_assets::Pallet::<T>::deposit(
                asset_id,
                &dest_account,
                amount,
                Precision::Exact,
            )
            .ok(),

            RewardDestination::None => None,
        }
    }

    /// Update the ledger for a controller.
    ///
    /// This will also update the stash lock.
    pub(crate) fn update_ledger(controller: &T::AccountId, ledger: &StakingLedger<T>) {
        T::StakeCurrency::set_lock(STAKING_ID, &ledger.stash, ledger.total, WithdrawReasons::all());
        <Ledger<T>>::insert(controller, ledger);
    }

    /// Chill a stash account.
    pub(crate) fn chill_stash(stash: &T::AccountId) {
        let chilled_as_validator = Self::do_remove_validator(stash);
        let chilled_as_cooperator = Self::do_remove_cooperator(stash);
        if chilled_as_validator || chilled_as_cooperator {
            Self::deposit_event(Event::<T>::Chilled { stash: stash.clone() });
        }
    }

    /// Plan a new session potentially trigger a new era.
    fn new_session(session_index: SessionIndex) -> Option<Vec<T::AccountId>> {
        // In any case we update reputation per each session.
        // TODO: replace with an associated type in Config
        pallet_reputation::Pallet::<T>::update_points_for_time();

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

    /// Start a session potentially starting an era.
    fn start_session(start_session: SessionIndex) -> DispatchResult {
        let next_active_era = Self::active_era().map(|e| e.index + 1).unwrap_or(0);
        // This is only `Some` when current era has already progressed to the next era, while the
        // active era is one behind (i.e. in the *last session of the active era*, or *first session
        // of the new current era*, depending on how you look at it).
        if let Some(next_active_era_start_session_index) =
            Self::eras_start_session_index(next_active_era)
        {
            match next_active_era_start_session_index.cmp(&start_session) {
                Ordering::Equal => {
                    Self::start_era(start_session)?;
                },
                Ordering::Less => {
                    // This arm should never happen, but better handle it than to stall the staking
                    // pallet.
                    frame_support::print("Warning: A session appears to have been skipped.");
                    Self::start_era(start_session)?;
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

        Ok(())
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
    /// * Calculate energy rate per bonded currency for active era
    /// * reset `active_era.start`,
    /// * update `BondedEras` and apply slashes.
    fn start_era(start_session: SessionIndex) -> DispatchResult {
        let active_era = ActiveEra::<T>::mutate(|active_era| {
            let new_index = active_era.as_ref().map(|info| info.index + 1).unwrap_or(0);
            *active_era = Some(ActiveEraInfo {
                index: new_index,
                // Set new active era start in next `on_finalize`. To guarantee usage of `Time`
                start: None,
            });
            new_index
        });

        Self::store_energy_rate(active_era);

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

        Self::apply_unapplied_slashes(active_era)
    }

    /// Compute energy demand rate
    fn store_energy_rate(era_index: EraIndex) {
        let staked = Self::eras_total_stake(era_index);
        let issuance = pallet_assets::Pallet::<T>::total_supply(T::EnergyAssetId::get());
        let core_nodes_num = Self::core_nodes_count();
        let battery_slot_cap = T::BatterySlotCapacity::get();

        let energy_per_stake_currency = T::EnergyPerStakeCurrency::calculate_energy_rate(
            staked,
            issuance,
            core_nodes_num,
            battery_slot_cap,
        );

        <ErasEnergyPerStakeCurrency<T>>::insert(era_index, energy_per_stake_currency);
        Self::deposit_event(Event::<T>::EraEnergyPerStakeCurrencySet {
            era_index,
            energy_rate: energy_per_stake_currency,
        });
    }

    fn end_era(_active_era: ActiveEraInfo, _session_index: SessionIndex) {
        // Clear offending validators.
        <OffendingValidators<T>>::kill();
    }

    /// Plan a new era.
    ///
    /// * Bump the current era storage (which holds the latest planned era).
    /// * Store start session index for the new planned era.
    /// * Clean old era information.
    /// * Store staking information for the new planned era
    ///
    /// Returns the new validator set.
    #[allow(clippy::type_complexity)]
    pub fn trigger_new_era(
        start_session_index: SessionIndex,
        exposures: Vec<(T::AccountId, Exposure<T::AccountId, StakeOf<T>>)>,
    ) -> Vec<T::AccountId> {
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
        Self::store_stakers_info(exposures, new_planned_era)
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
        let exposures = Self::ellect_and_collect_exposures();
        if (exposures.len() as u32) < Self::minimum_validator_count().max(1) {
            // Session will panic if we ever return an empty validator set, thus max(1) ^^.
            match CurrentEra::<T>::get() {
                Some(current_era) if current_era > 0 => log!(
                    warn,
                    "chain does not have enough staking candidates to operate for era {:?} ({} \
                    elected, minimum is {})",
                    current_era,
                    exposures.len(),
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

            Self::deposit_event(Event::StakingElectionFailed);
            return None;
        }

        Self::deposit_event(Event::StakersElected);
        Some(Self::trigger_new_era(start_session_index, exposures))
    }

    /// Process the output of the election.
    ///
    /// Store staking information for the new planned era
    #[allow(clippy::type_complexity)]
    pub fn store_stakers_info(
        mut exposures: Vec<(T::AccountId, Exposure<T::AccountId, StakeOf<T>>)>,
        new_planned_era: EraIndex,
    ) -> Vec<T::AccountId> {
        let max_validators = Self::validator_count().max(1) as usize;

        // Get validators with max total stake, invulnerables validators are always elected
        if exposures.len() > max_validators {
            let invulnerables = Self::invulnerables();

            exposures.select_nth_unstable_by(max_validators, |a, b| {
                // If `a` < `b`, then validator `a` will be elected
                match (invulnerables.contains(&a.0), invulnerables.contains(&b.0)) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => a.1.total.cmp(&b.1.total).reverse(),
                }
            });
        }
        let elected_stashes: Vec<_> =
            exposures.iter().take(max_validators).map(|(x, _)| x.clone()).collect();

        // Populate stakers, exposures, and the snapshot of validator prefs.
        let mut total_stake: StakeOf<T> = Zero::zero();
        exposures.into_iter().for_each(|(stash, exposure)| {
            total_stake = total_stake.saturating_add(exposure.total);
            <ErasStakers<T>>::insert(new_planned_era, &stash, &exposure);

            let mut exposure_clipped = exposure;
            let clipped_max_len = T::MaxCooperatorRewardedPerValidator::get() as usize;
            if exposure_clipped.others.len() > clipped_max_len {
                exposure_clipped.others.sort_by(|a, b| a.value.cmp(&b.value).reverse());
                exposure_clipped.others.truncate(clipped_max_len);
            }
            <ErasStakersClipped<T>>::insert(new_planned_era, &stash, exposure_clipped);
        });

        // Insert current era staking information
        <ErasTotalStake<T>>::insert(new_planned_era, total_stake);

        // Collect the pref of all winners.
        for stash in &elected_stashes {
            let pref = Self::validators(stash);
            <ErasValidatorPrefs<T>>::insert(new_planned_era, stash, pref);
        }

        if new_planned_era > 0 {
            log!(
                info,
                "new validator set of size {:?} has been processed for era {:?}",
                elected_stashes.len(),
                new_planned_era,
            );
        }

        elected_stashes
    }

    /// Ellect validators and collect them into a [`Exposure`].
    #[allow(clippy::type_complexity)]
    fn ellect_and_collect_exposures() -> Vec<(T::AccountId, Exposure<T::AccountId, StakeOf<T>>)> {
        Self::ellect_validators();

        Validators::<T>::iter()
            .map(|(validator, prefs)| {
                let controller = Self::bonded(&validator).unwrap();
                // Build `struct exposure` from `support`.
                let own: StakeOf<T> = Self::ledger(&controller).unwrap().active;
                let others = match Collaborations::<T>::get(&validator) {
                    Some(coops) => coops
                        .iter()
                        .cloned()
                        .filter_map(|who| {
                            match Self::cooperators(&who)
                                .and_then(|collab| collab.targets.get(&validator).cloned())
                            {
                                Some(value) => {
                                    let record = pallet_reputation::Pallet::<T>::reputation(&who)
                                        .unwrap_or_else(ReputationRecord::with_now::<T>);
                                    if record.reputation >= prefs.min_coop_reputation {
                                        Some(IndividualExposure { who, value })
                                    } else {
                                        None
                                    }
                                },
                                None => None,
                            }
                        })
                        .collect(),
                    None => Vec::new(),
                };
                let total = own
                    + others
                        .iter()
                        .fold(Zero::zero(), |acc: StakeOf<T>, x| acc.saturating_add(x.value));

                let exposure = Exposure { own, others, total };

                (validator, exposure)
            })
            .collect()
    }

    // filter out illegit validators
    fn ellect_validators() {
        // filter out by min reputation for validator
        let (legit, should_chill): (Vec<_>, Vec<_>) =
            Validators::<T>::iter().partition(|(acc, _)| Self::is_legit_for_validator(acc));
        // if validators don't have enough reputation to be validators, we force chill them
        // in this keys they would need to call Self::validate again, and their reputation will be
        // checked. So at the very list they will be blocked until the next era
        should_chill.iter().for_each(|(acc, _)| {
            Self::chill_stash(acc);
        });

        for (acc, _) in legit {
            if !Self::is_legit_for_collab(&acc) {
                Validators::<T>::mutate(&acc, |prefs| {
                    prefs.collaborative = false;
                });
                // remove it from collaborations (if any)
                //
                // this could be a case, when a validator lost so much reputation, that they can't
                // be a collaborative validator anymore.
                Collaborations::<T>::remove(&acc);
            }
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
        let controller = Self::bonded(stash).ok_or(Error::<T>::NotStash)?;

        slashing::clear_stash_metadata::<T>(stash, num_slashing_spans)?;

        <Bonded<T>>::remove(stash);
        <Ledger<T>>::remove(&controller);

        <Payee<T>>::remove(stash);
        Self::do_remove_validator(stash);
        Self::do_remove_cooperator(stash);

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
        <ErasEnergyPerStakeCurrency<T>>::remove(era_index);
        <ErasTotalStake<T>>::remove(era_index);
        ErasStartSessionIndex::<T>::remove(era_index);
    }

    /// Apply previously-unapplied slashes on the beginning of a new era, after a delay.
    fn apply_unapplied_slashes(active_era: EraIndex) -> DispatchResult {
        let era_slashes = UnappliedSlashes::<T>::take(active_era);
        log!(
            debug,
            "found {} slashes scheduled to be executed in era {:?}",
            era_slashes.len(),
            active_era,
        );
        for slash in era_slashes {
            slashing::apply_slash::<T>(slash)?;
        }

        Ok(())
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
        exposure: Exposure<T::AccountId, StakeOf<T>>,
    ) {
        <ErasStakers<T>>::insert(&current_era, &stash, &exposure);
    }

    #[cfg(feature = "runtime-benchmarks")]
    pub fn set_slash_reward_fraction(fraction: Perbill) {
        SlashRewardFraction::<T>::put(fraction);
    }

    /// This function will add a cooperator to the `Cooperators` and `Collaborations`.
    ///
    /// If the cooperator already exists, their cooperations will be updated.
    ///
    /// NOTE: you must ALWAYS use this function to add cooperator or update their targets. Any access
    /// to `Cooperators` or `Collaborations` outside of this function is almost certainly
    /// wrong.
    pub fn do_add_cooperator(who: &T::AccountId, cooperations: Cooperations<T>) -> DispatchResult {
        for target in &cooperations.targets {
            if !Collaborations::<T>::contains_key(target.0) {
                let mut set = BoundedBTreeSet::new();
                set.try_insert(who.clone()).map_err(|_| Error::<T>::TooManyCooperators)?;
                Collaborations::<T>::insert(target.0, set);
                continue;
            }

            Collaborations::<T>::try_mutate(target.0, |set| {
                set.as_mut()
                    .unwrap()
                    .try_insert(who.clone())
                    .map_err(|_| Error::<T>::TooManyCooperators)
            })?;
        }
        Cooperators::<T>::insert(who, cooperations);

        Ok(())
    }

    /// This function will remove a cooperator from the `Cooperators` storage map,
    /// and `VoterList`.
    ///
    /// Returns true if `who` was removed from `Cooperators`, otherwise false.
    ///
    /// NOTE: you must ALWAYS use this function to remove a cooperator from the system. Any access to
    /// `Cooperators` or `VoterList` outside of this function is almost certainly
    /// wrong.
    pub fn do_remove_cooperator(who: &T::AccountId) -> bool {
        match Cooperators::<T>::get(who) {
            Some(cooperations) => {
                for target in cooperations.targets {
                    Collaborations::<T>::mutate(target.0, |set| {
                        if let Some(set) = set {
                            set.remove(who);
                        }
                    });
                }
                Cooperators::<T>::remove(who);
                true
            },
            None => false,
        }
    }

    /// This function will add a validator to the `Validators` storage map.
    ///
    /// If the validator already exists, their preferences will be updated.
    ///
    /// NOTE: you must ALWAYS use this function to add a validator to the system. Any access to
    /// `Validators` or `VoterList` outside of this function is almost certainly
    /// wrong.
    pub fn do_add_validator(who: &T::AccountId, prefs: ValidatorPrefs) {
        Validators::<T>::insert(who, prefs);
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
            Collaborations::<T>::remove(who);
            true
        } else {
            false
        }
    }

    // TODO: get rid of floating point types.
    pub fn calculate_block_authoring_reward() -> ReputationPoint {
        let active_validators_count = T::SessionInterface::validators().len();
        let reward = Self::block_authoring_reward().saturating_mul(active_validators_count as u64);

        ReputationPoint(reward)
    }

    // TODO: make coefficients a runtime parameter.
    pub fn calculate_energy_reward_multiplier(stash: &T::AccountId) -> Perbill {
        let reputation = if let Some(record) = pallet_reputation::AccountReputation::<T>::get(stash)
        {
            record.reputation
        } else {
            return Perbill::zero();
        };

        if let Some(tier) = reputation.tier() {
            T::ReputationTierEnergyRewardAdditionalPercentMapping::get(&tier)
        } else {
            Perbill::zero()
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
        if let Err(e) = Self::start_session(start_index) {
            log!(error, "failed to start session {}: {:?}", start_index, e);
        }
    }
    fn end_session(end_index: SessionIndex) {
        log!(trace, "ending session {}", end_index);
        Self::end_session(end_index)
    }
}

impl<T: Config> historical::SessionManager<T::AccountId, Exposure<T::AccountId, StakeOf<T>>>
    for Pallet<T>
{
    fn new_session(
        new_index: SessionIndex,
    ) -> Option<Vec<(T::AccountId, Exposure<T::AccountId, StakeOf<T>>)>> {
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
    ) -> Option<Vec<(T::AccountId, Exposure<T::AccountId, StakeOf<T>>)>> {
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

/// This is intended to be used with `FilterHistoricalOffences`.
impl<T: Config>
    OnOffenceHandler<T::AccountId, pallet_session::historical::IdentificationTuple<T>, Weight>
    for Pallet<T>
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
                let cooperators_len = unapplied.others.len() as u64;
                let reporters_len = details.reporters.len() as u64;

                {
                    let upper_bound = 1 /* Validator/CooperatorSlashInEra */ + 2 /* fetch_spans */;
                    let rw = upper_bound + cooperators_len * upper_bound;
                    add_db_reads_writes(rw, rw);
                }
                unapplied.reporters = details.reporters.clone();
                if slash_defer_duration == 0 {
                    // Apply right away.
                    if let Err(e) = slashing::apply_slash::<T>(unapplied) {
                        frame_support::print(format!("failed to apply slash: {:?}", e).as_str());
                    }
                    {
                        let slash_cost = (6, 5);
                        let reward_cost = (2, 2);
                        add_db_reads_writes(
                            (1 + cooperators_len) * slash_cost.0 + reward_cost.0 * reporters_len,
                            (1 + cooperators_len) * slash_cost.1 + reward_cost.1 * reporters_len,
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

/// Add reputation points to block authors:
/// + REPUTATION_POINTS_PER_DAY to the block producer for producing a (non-uncle) block,
impl<T> pallet_authorship::EventHandler<T::AccountId, BlockNumberFor<T>> for Pallet<T>
where
    T: Config + pallet_authorship::Config + pallet_session::Config,
{
    fn note_author(author: T::AccountId) {
        let reward = Self::calculate_block_authoring_reward();
        if let Err(e) = <pallet_reputation::Pallet<T>>::do_increase_points(&author, reward) {
            pallet_reputation::Pallet::<T>::deposit_event(
                pallet_reputation::Event::<T>::ReputationIncreaseFailed {
                    account: author,
                    error: e,
                    points: reward,
                },
            );
        }
    }
}

impl<T: Config> EnergyRateCalculator<StakeOf<T>, EnergyOf<T>> for Pallet<T> {
    fn calculate_energy_rate(
        _total_staked: StakeOf<T>,
        _total_issuance: EnergyOf<T>,
        _core_nodes_num: u32,
        _battery_slot_cap: EnergyOf<T>,
    ) -> EnergyOf<T> {
        Pallet::<T>::current_energy_per_stake_currency().unwrap_or(EnergyOf::<T>::zero())
    }
}

#[cfg(any(test, feature = "try-runtime"))]
impl<T: Config> Pallet<T> {
    pub(crate) fn do_try_state(
        _: frame_system::pallet_prelude::BlockNumberFor<T>,
    ) -> Result<(), &'static str> {
        Self::check_cooperators()?;
        Self::check_exposures()?;
        Self::check_ledgers()
    }

    fn check_ledgers() -> Result<(), &'static str> {
        Bonded::<T>::iter().try_for_each(|(_, ctrl)| Self::ensure_ledger_consistent(ctrl))
    }

    fn check_exposures() -> Result<(), &'static str> {
        // a check per validator to ensure the exposure struct is always sane.
        let era = Self::active_era().unwrap().index;
        ErasStakers::<T>::iter_prefix_values(era).try_for_each(|expo| {
            ensure!(
                expo.total
                    == expo.own
                        + expo.others.iter().map(|e| e.value).fold(Zero::zero(), |acc, x| acc + x),
                "wrong total exposure.",
            );
            Ok(())
        })
    }

    fn check_cooperators() -> Result<(), &'static str> {
        // a check per cooperator to ensure their entire stake is correctly distributed. Will only
        // kick-in if the cooperation was submitted before the current era.
        let era = Self::active_era().unwrap().index;
        <Cooperators<T>>::iter()
            .filter_map(
                |(cooperator, cooperation)| {
                    if cooperation.submitted_in < era {
                        Some(cooperator)
                    } else {
                        None
                    }
                },
            )
            .try_for_each(|cooperator| {
                // must be bonded.
                Self::ensure_is_stash(&cooperator)?;
                let mut sum = StakeOf::<T>::zero();
                T::SessionInterface::validators()
                    .iter()
                    .map(|v| Self::eras_stakers(era, v))
                    .try_for_each(|e| {
                        let individual =
                            e.others.iter().filter(|e| e.who == cooperator).collect::<Vec<_>>();
                        let len = individual.len();
                        match len {
                            0 => { /* not supporting this validator at all. */ },
                            1 => sum += individual[0].value,
                            _ => return Err("cooperator cannot back a validator more than once."),
                        };
                        Ok(())
                    })
            })
    }

    fn ensure_is_stash(who: &T::AccountId) -> Result<(), &'static str> {
        ensure!(Self::bonded(who).is_some(), "Not a stash.");
        Ok(())
    }

    fn ensure_ledger_consistent(ctrl: T::AccountId) -> Result<(), &'static str> {
        // ensures ledger.total == ledger.active + sum(ledger.unlocking).
        let ledger = Self::ledger(ctrl.clone()).ok_or("Not a controller.")?;
        let real_total: StakeOf<T> =
            ledger.unlocking.iter().fold(ledger.active, |a, c| a + c.value);
        ensure!(real_total == ledger.total, "ledger.total corrupt");

        if !(ledger.active >= T::StakeCurrency::minimum_balance() || ledger.active.is_zero()) {
            log!(warn, "ledger.active less than ED: {:?}, {:?}", ctrl, ledger)
        }

        Ok(())
    }
}
