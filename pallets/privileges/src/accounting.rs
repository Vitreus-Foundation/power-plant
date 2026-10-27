//! Bounded settlement of completed UTC days and annual point snapshots.
use super::*;
use frame_support::{traits::ConstU32, BoundedVec};
use parity_scale_codec::MaxEncodedLen;
use sp_runtime::DispatchError;

pub(crate) const MAX_AUTO_DAYS: u32 = 7;
pub(crate) const MAX_MEMBERS: usize = 1024;
type Sources<T> = BoundedVec<(<T as frame_system::Config>::AccountId, PointsOf<T>), ConstU32<1024>>;

pub use crate::AccrualReviewRequired;

pub fn today<T: Config>() -> Result<CurrentDateInfo, DispatchError> {
    let now = i64::try_from(<T as Config>::UnixTime::now().as_secs())
        .ok()
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
        .ok_or(Error::<T>::NotCorrectDate)?;
    CurrentDateInfo::new::<T>(now.year(), now.month(), now.day())
}

pub fn paused<T: Config>() -> bool {
    AccrualReviewRequired::<T>::get()
}

/// Initialization hooks run before the timestamp inherent. Once the first
/// timestamp was observed, a different block number identifies that interval.
fn before_timestamp<T: Config>() -> bool {
    TimestampSeenAtBlock::<T>::get()
        .is_some_and(|seen| seen != frame_system::Pallet::<T>::block_number())
}

pub fn ensure_ready<T: Config>() -> DispatchResult {
    ensure!(!before_timestamp::<T>(), Error::<T>::AccountingPaused);
    ensure!(!paused::<T>(), Error::<T>::AccountingPaused);
    Pallet::<T>::update_quarter_info();
    if CurrentDate::<T>::exists() {
        ensure!(
            !Pallet::<T>::check_correct_date(&CurrentDate::<T>::get(), &today::<T>()?),
            Error::<T>::AccountingPaused
        );
    }
    Ok(())
}

/// Freeze further automatic use of the old interval if staking had to change
/// while accounting was behind. Staking itself must remain operable.
pub fn before_stake_change<T: Config>(account: &T::AccountId) {
    if !CurrentDate::<T>::exists() {
        return;
    }
    // Preserve the first stake before session rotations/slashes in this block.
    // Later callbacks can update live caches without rewriting yesterday's source.
    if before_timestamp::<T>() {
        let block = frame_system::Pallet::<T>::block_number();
        if PreTimestampStake::<T>::get(account).is_none_or(|(saved, _, _)| saved != block) {
            if let Some(member) = VipMembers::<T>::get(account) {
                PreTimestampStake::<T>::insert(
                    account,
                    (
                        block,
                        member.active_stake,
                        pallet_nac_managing::Pallet::<T>::is_vipp_quarantined(account),
                    ),
                );
            }
        }
    }
    if let Ok(now) = today::<T>() {
        if Pallet::<T>::check_correct_date(&CurrentDate::<T>::get(), &now) {
            AccrualReviewRequired::<T>::put(true);
        }
    }
}

pub fn ensure_member_capacity<T: Config>(account: &T::AccountId) -> DispatchResult {
    if !VipMembers::<T>::contains_key(account) {
        ensure!(
            VipMembers::<T>::iter_keys().take(MAX_MEMBERS).count() < MAX_MEMBERS,
            Error::<T>::AccountingCapacityExceeded
        );
    }
    Ok(())
}

pub fn ensure_vipp_capacity<T: Config>(account: &T::AccountId) -> DispatchResult {
    if !VippMembers::<T>::contains_key(account) {
        ensure!(
            VippMembers::<T>::iter_keys().take(MAX_MEMBERS).count() < MAX_MEMBERS,
            Error::<T>::AccountingCapacityExceeded
        );
    }
    Ok(())
}

pub fn within_limit<T: Config>(
    from: &CurrentDateInfo,
    to: &CurrentDateInfo,
) -> Result<bool, DispatchError> {
    let mut limit = from.clone();
    limit.add_days::<T>(MAX_UPDATE_DAYS.into())?;
    Ok(!Pallet::<T>::check_correct_date(&limit, to))
}

fn sources<T: Config>(
    close_pre_timestamp_day: bool,
) -> Result<(Sources<T>, Sources<T>), DispatchError> {
    let use_snapshot = close_pre_timestamp_day && before_timestamp::<T>();
    let block = frame_system::Pallet::<T>::block_number();
    let stake_at_boundary = |who: &T::AccountId, live: PointsOf<T>| {
        if use_snapshot {
            if let Some((saved_block, stake, _)) = PreTimestampStake::<T>::get(who) {
                if saved_block == block {
                    return stake;
                }
            }
        }
        live
    };
    let vip: Vec<_> = VipMembers::<T>::iter()
        .take(MAX_MEMBERS + 1)
        .map(|(who, info)| {
            let stake = stake_at_boundary(&who, info.active_stake);
            (who, stake)
        })
        .collect();
    ensure!(vip.len() <= MAX_MEMBERS, Error::<T>::AccountingCapacityExceeded);
    let mut vipp = Vec::new();
    for who in VippMembers::<T>::iter_keys().take(MAX_MEMBERS + 1) {
        let key = VippMembers::<T>::hashed_key_for(&who);
        let limit = T::StakeBalance::max_encoded_len()
            + 5
            + 64 * (T::ItemId::max_encoded_len() + T::StakeBalance::max_encoded_len());
        ensure!(
            sp_io::storage::read(&key, &mut [], 0).is_some_and(|size| size as usize <= limit),
            Error::<T>::AccountingCapacityExceeded
        );
        let info = VippMembers::<T>::get(&who).ok_or(Error::<T>::AccountingPaused)?;
        ensure!(info.active_vipp_threshold.len() <= 64, Error::<T>::AccountingCapacityExceeded);
        let live_stake = VipMembers::<T>::get(&who).map(|m| m.active_stake).unwrap_or_default();
        let cap = info
            .active_vipp_threshold
            .iter()
            .fold(PointsOf::<T>::zero(), |total, (_, value)| total.saturating_add(*value));
        let snapshot = if use_snapshot {
            PreTimestampStake::<T>::get(&who).filter(|(saved, _, _)| *saved == block)
        } else {
            None
        };
        let (stake, quarantined) =
            snapshot.map(|(_, stake, quarantined)| (stake, quarantined)).unwrap_or_else(|| {
                (live_stake, pallet_nac_managing::Pallet::<T>::is_vipp_quarantined(&who))
            });
        let eligible = if quarantined { PointsOf::<T>::zero() } else { stake.min(cap) };
        vipp.push((who, eligible));
    }
    ensure!(vipp.len() <= MAX_MEMBERS, Error::<T>::AccountingCapacityExceeded);
    Ok((
        vip.try_into().map_err(|_| Error::<T>::AccountingCapacityExceeded)?,
        vipp.try_into().map_err(|_| Error::<T>::AccountingCapacityExceeded)?,
    ))
}

/// Preserve each day's rounding but write each account at most once per year
/// segment. Work is bounded by 2 * 1024 members and 366 completed days.
#[frame_support::transactional]
pub fn advance<T: Config>(target: &CurrentDateInfo) -> DispatchResult {
    let mut cursor = CurrentDate::<T>::get();
    ensure!(within_limit::<T>(&cursor, target)?, Error::<T>::AccountingPaused);
    let (vip, vipp) = sources::<T>(true)?;
    let (vip, vipp) = (&vip, &vipp);
    // A 366-day window can close two years (e.g. Dec 31 of a common year).
    let mut segments: Vec<(u32, Vec<Perquintill>, bool)> = Vec::new();
    while Pallet::<T>::check_correct_date(&cursor, target) {
        let year = cursor.current_year;
        ensure!(year >= 0, Error::<T>::NotCorrectDate);
        let mut multipliers = Vec::new();
        while cursor.current_year == year && Pallet::<T>::check_correct_date(&cursor, target) {
            multipliers.push(Pallet::<T>::calculate_multiplier(cursor.days_since_new_year + 1));
            cursor.add_days::<T>(1)?;
        }
        let closed = cursor.current_year != year;
        if closed {
            ensure!(!Rewards::<T>::contains_key(year as u32), Error::<T>::YearRewardsAlreadySet);
        }
        segments.push((year as u32, multipliers, closed));
    }
    for (who, stake) in vip {
        let mut member = VipMembers::<T>::get(who).ok_or(Error::<T>::AccountingPaused)?;
        for (year, multipliers, closed) in &segments {
            for multiplier in multipliers {
                member.points = member
                    .points
                    .saturating_add(Pallet::<T>::calculate_points(*stake, *multiplier));
            }
            if *closed {
                ensure!(
                    !VipPoints::<T>::contains_key(year, who),
                    Error::<T>::HistoricalPointsAlreadyExist
                );
                VipPoints::<T>::insert(year, who, member.points);
                member.points.set_zero();
            }
        }
        VipMembers::<T>::insert(who, member);
    }
    for (who, daily) in vipp {
        let mut member = VippMembers::<T>::get(who).ok_or(Error::<T>::AccountingPaused)?;
        for (year, multipliers, closed) in &segments {
            for _ in multipliers {
                member.points = member.points.saturating_add(*daily);
            }
            if *closed {
                ensure!(
                    !VippPoints::<T>::contains_key(year, who),
                    Error::<T>::HistoricalPointsAlreadyExist
                );
                VippPoints::<T>::insert(year, who, member.points);
                member.points.set_zero();
            }
        }
        VippMembers::<T>::insert(who, member);
    }
    CurrentDate::<T>::put(cursor);
    Ok(())
}
