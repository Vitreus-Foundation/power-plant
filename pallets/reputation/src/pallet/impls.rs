//! Implementations for Reputation Pallet (Non-Dispatchables)
//!
//! This module contains core implementations for the Reputation pallet, focusing on non-dispatchable functionality.
//! It extends and complements the pallet by providing the logic needed for maintaining and modifying reputation records.
//!
//! # Features
//! - Implements the `OnNewAccount` and `OnKilledAccount` traits from FRAME support, allowing reputation tracking on account creation and removal.
//! - Provides core functions for managing and modifying reputation points and records.
//! - Converts values using `SaturatedConversion` for safety.
//!
//! # Structure
//! - Implements reputation-related functionality that is invoked internally within the pallet.
//! - Defines how new accounts are initialized with reputation and how reputation is handled when accounts are removed.
//!
//! # Dependencies
//! - Uses FRAME support traits such as `OnNewAccount` and `OnKilledAccount` for handling lifecycle events of accounts.
//! - Relies on `frame_support::pallet_prelude` for prelude items to ensure consistent pallet interaction.
//!
//! # Usage
//! This file should be used to define the core, non-user facing logic of the Reputation pallet.
//! It is automatically utilized by the pallet during account lifecycle events and when updating reputation records.

use crate::{ReputationPoint, ReputationRecord};

use super::pallet::*;
use frame_support::pallet_prelude::*;
use frame_support::traits::{OnKilledAccount, OnNewAccount};
use sp_runtime::SaturatedConversion;

/// Notice that this pallet implements the `OnNewAccount` and `OnKilledAccount` traits from
/// `frame_support`. If you want any account to have associated reputation with it, you need to
/// specify `frame_system::Config` to use this pallet on `OnNewAccount`.
///
/// `OnKilledAccount` is used to to remove orfan data from the store.
impl<T: Config> Pallet<T> {
    /// Updates the points for the time since the last time the account was updated.
    pub fn update_points_for_time() {
        let now = <frame_system::Pallet<T>>::block_number().saturated_into();
        AccountReputation::<T>::translate(|_: T::AccountId, mut old: ReputationRecord| {
            old.update_with_block_number(now);
            Some(old)
        });
    }

    /// Acturally do the slash.
    pub fn do_slash(account: &T::AccountId, points: ReputationPoint) -> DispatchResult {
        let updated = <frame_system::Pallet<T>>::block_number().saturated_into();

        AccountReputation::<T>::try_mutate_exists(account, |value| {
            value
                .as_mut()
                .map(|old| {
                    old.reputation.decrease(points);
                    old.updated = updated;
                })
                .ok_or(Error::<T>::AccountNotFound)
        })?;

        Self::deposit_event(Event::ReputationSlashed { account: account.clone(), points });

        Ok(())
    }

    /// Increase the points for an account by the given amount, creating it if it doesn't exist.
    pub fn increase_creating(account: &T::AccountId, points: ReputationPoint) {
        AccountReputation::<T>::mutate(account, |old| match old {
            Some(rec) => rec.reputation.increase(points),
            None => *old = Some(ReputationRecord::from(points)),
        });
    }

    /// Acturally increase points.
    pub fn do_increase_points(account: &T::AccountId, points: ReputationPoint) -> DispatchResult {
        <AccountReputation<T>>::try_mutate_exists(account, |value| {
            value
                .as_mut()
                .map(|old| {
                    old.reputation.increase(points);
                })
                .ok_or(Error::<T>::AccountNotFound)
        })?;

        Ok(())
    }
}

impl<T: Config> OnNewAccount<T::AccountId> for Pallet<T> {
    fn on_new_account(who: &T::AccountId) {
        if AccountReputation::<T>::contains_key(who) {
            return;
        }

        let now = <frame_system::Pallet<T>>::block_number().saturated_into();
        let new_rep = ReputationRecord::with_blocknumber(now);
        AccountReputation::<T>::insert(who, new_rep);
    }
}

impl<T: Config> OnKilledAccount<T::AccountId> for Pallet<T> {
    fn on_killed_account(who: &T::AccountId) {
        AccountReputation::<T>::remove(who);
    }
}
