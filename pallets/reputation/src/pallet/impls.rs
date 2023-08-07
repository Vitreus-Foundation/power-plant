//! Implementations for the Reputation pallet.

use crate::{ReputationPoint, ReputationRecord};

use super::pallet::*;
use frame_support::pallet_prelude::*;
use frame_support::traits::{OnKilledAccount, OnNewAccount};

/// Notice that this pallet implements the `OnNewAccount` and `OnKilledAccount` traits from
/// `frame_support`. If you want any account to have associated reputation with it, you need to
/// specify `frame_system::Config` to use this pallet on `OnNewAccount`.
///
/// `OnKilledAccount` is used to to remove orfan data from the store.
impl<T: Config> Pallet<T> {
    /// Updates the points for the time since the last time the account was updated.
    pub fn update_points_for_time() {
        let now = <frame_system::Pallet<T>>::block_number();
        AccountReputation::<T>::translate(|_: T::AccountId, mut old: ReputationRecord<T>| {
            old.update_with_block_number(now);
            Some(old)
        });
    }

    /// Acturally do the slash.
    pub fn do_slash(account: &T::AccountId, points: ReputationPoint) -> DispatchResult {
        let updated = <frame_system::Pallet<T>>::block_number();

        AccountReputation::<T>::try_mutate_exists(account, |value| {
            value
                .as_mut()
                .map(|old| {
                    *old.points = old.points.saturating_sub(*points);
                    old.updated = updated;
                })
                .ok_or(Error::<T>::AccountNotFound)
        })?;

        Self::deposit_event(Event::ReputationSlashed { account: account.clone(), points });

        Ok(())
    }

    /// Add the account if it's not in the storage.
    pub fn add_not_exists(account: &T::AccountId) {
        AccountReputation::<T>::mutate(account, |old| {
            if old.is_none() {
                *old = Some(ReputationRecord::default());
            }
        });
    }

    /// Increase the points for an account by the given amount, creating it if it doesn't exist.
    pub fn increase_creating(account: &T::AccountId, points: ReputationPoint) {
        AccountReputation::<T>::mutate(account, |old| match old {
            Some(rec) => *rec.points += *points,
            None => *old = Some(ReputationRecord::<T>::from(points)),
        });
    }

    /// Acturally increase points.
    pub fn do_increase_points(account: &T::AccountId, points: ReputationPoint) -> DispatchResult {
        let updated = <frame_system::Pallet<T>>::block_number();

        <AccountReputation<T>>::try_mutate_exists(account, |value| {
            value
                .as_mut()
                .map(|old| {
                    *old.points = old.points.saturating_add(*points);
                    old.updated = updated;
                })
                .ok_or(Error::<T>::AccountNotFound)
        })?;

        Self::deposit_event(Event::ReputationIncreased { account: account.clone(), points });

        Ok(())
    }
}

impl<T: Config> OnNewAccount<T::AccountId> for Pallet<T> {
    fn on_new_account(who: &T::AccountId) {
        let now = <frame_system::Pallet<T>>::block_number();
        let new_rep = ReputationRecord::<T>::with_blocknumber(now);
        AccountReputation::<T>::insert(who, new_rep);
    }
}

impl<T: Config> OnKilledAccount<T::AccountId> for Pallet<T> {
    fn on_killed_account(who: &T::AccountId) {
        AccountReputation::<T>::remove(who);
    }
}
