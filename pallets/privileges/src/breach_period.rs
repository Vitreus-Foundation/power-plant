//! Preserve the calendar period of a deferred VIPP threshold breach.
//!
//! A finalization budget may postpone NFT burns across a year boundary.
//! Any historical forfeiture requires explicit review. An initialization breach
//! before the January 1 timestamp belongs to the new year being initialized.
use crate::*;
use frame_support::{dispatch::DispatchClass, traits::tokens::nonfungibles_v2::InspectEnumerable};

/// An invalid clock/overflow requires explicit review, never guessed forfeiture.
const UNRESOLVED_YEAR: u32 = u32::MAX;

fn clock_year<T: Config>() -> u32 {
    accounting::today::<T>()
        .ok()
        .and_then(|date| u32::try_from(date.current_year).ok())
        .unwrap_or(UNRESOLVED_YEAR)
}

/// Record the first breach only for an account that has current VIPP points.
pub fn record<T: Config>(account: &T::AccountId) {
    if !VippMembers::<T>::contains_key(account) || VippBreachYear::<T>::contains_key(account) {
        return;
    }
    let before_timestamp = TimestampSeenAtBlock::<T>::get()
        .is_some_and(|seen| seen != frame_system::Pallet::<T>::block_number());
    if before_timestamp {
        let queued = PreTimestampVippBreaches::<T>::try_mutate(|accounts| {
            if accounts.contains(account) {
                Ok(())
            } else {
                accounts.try_push(account.clone())
            }
        });
        if queued.is_err() {
            VippBreachYear::<T>::insert(account, UNRESOLVED_YEAR);
            AccrualReviewRequired::<T>::put(true);
        }
    } else {
        let year = clock_year::<T>();
        VippBreachYear::<T>::insert(account, year);
        if year == UNRESOLVED_YEAR {
            AccrualReviewRequired::<T>::put(true);
        }
    }
}

/// Called before daily/yearly settlement, once the current block's timestamp is known.
pub fn resolve_timestamp<T: Config>() {
    let count = PreTimestampVippBreaches::<T>::decode_len().unwrap_or_default() as u64;
    frame_system::Pallet::<T>::register_extra_weight_unchecked(
        if count == 0 {
            T::DbWeight::get().reads(1)
        } else {
            // The measured maximum includes queue decoding/encoding and proof,
            // not only the per-account storage operations.
            <T as Config>::WeightInfo::resolve_timestamp_breaches()
        },
        DispatchClass::Mandatory,
    );
    if count == 0 {
        return;
    }
    let year = clock_year::<T>();
    for account in PreTimestampVippBreaches::<T>::take() {
        if !VippBreachYear::<T>::contains_key(&account) {
            VippBreachYear::<T>::insert(&account, year);
        }
    }
    if year == UNRESOLVED_YEAR {
        AccrualReviewRequired::<T>::put(true);
    }
}

/// Called after the complete bounded repair, inside its storage transaction.
/// Partial burns preserve point balances. If losing the last NFT might affect an
/// archived year, first/minimum breach data alone cannot prove the forfeiture date:
/// only an explicit approved historical decision can remove an annual balance.
pub fn settle<T: Config>(account: &T::AccountId) -> DispatchResult {
    ensure!(!PreTimestampVippBreaches::<T>::get().contains(account), Error::<T>::AccountingPaused);
    let Some(year) = VippBreachYear::<T>::get(account) else {
        return Ok(());
    };
    ensure!(year != UNRESOLVED_YEAR, Error::<T>::AccountingPaused);
    let last_nft_gone = T::Nfts::owned_in_collection(&T::VIPPCollectionId::get(), account)
        .next()
        .is_none();
    if last_nft_gone && i64::from(year) < i64::from(CurrentDate::<T>::get().current_year) {
        let approval =
            VippForfeitureApproval::<T>::get(account).ok_or(Error::<T>::AccountingPaused)?;
        if let Some(approved_year) = approval {
            ensure!(
                approved_year >= year
                    && i64::from(approved_year) < i64::from(CurrentDate::<T>::get().current_year),
                Error::<T>::NotCorrectDate
            );
            ensure!(!Rewards::<T>::contains_key(approved_year), Error::<T>::YearRewardsAlreadySet);
            VippPoints::<T>::remove(approved_year, account);
        }
    }
    VippBreachYear::<T>::remove(account);
    VippForfeitureApproval::<T>::remove(account);
    Ok(())
}
