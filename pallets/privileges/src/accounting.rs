//! Bounded UTC accounting and explicitly approved recovery of a stale clock.
//!
//! A frozen source is an administrator-approved assumption for an interval, not
//! evidence of historical stake. Historical reconstruction belongs off chain.
use super::*;
use frame_support::{traits::ConstU32, BoundedVec};
use parity_scale_codec::MaxEncodedLen;
use sp_runtime::DispatchError;

pub(crate) const MAX_AUTO_DAYS: u32 = 7;
pub(crate) const MAX_SOURCES_PER_STEP: usize = 64;
pub(crate) const MAX_MEMBERS: usize = migration::recovery::MAX_MEMBERS_PER_KIND as usize;
type Sources<T> = BoundedVec<(<T as frame_system::Config>::AccountId, PointsOf<T>), ConstU32<1024>>;

#[derive(Encode, Decode, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CatchUp<T: Config> {
    pub target: CurrentDateInfo,
    pub source_cursor: u32,
    pub vip: Sources<T>,
    pub vipp: Sources<T>,
}

pub use crate::{AccrualReviewRequired, ApprovedCatchUp};

pub fn today<T: Config>() -> Result<CurrentDateInfo, DispatchError> {
    let now = i64::try_from(<T as Config>::UnixTime::now().as_secs())
        .ok()
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
        .ok_or(Error::<T>::NotCorrectDate)?;
    CurrentDateInfo::new::<T>(now.year(), now.month(), now.day())
}

pub fn paused<T: Config>() -> bool {
    migration::recovery::is_frozen::<T>()
        || AccrualReviewRequired::<T>::get()
        || ApprovedCatchUp::<T>::exists()
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
        if Pallet::<T>::check_correct_date(&CurrentDate::<T>::get(), &now)
            && !ApprovedCatchUp::<T>::exists()
        {
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
pub fn advance<T: Config>(target: &CurrentDateInfo, frozen: Option<&CatchUp<T>>) -> DispatchResult {
    let mut cursor = CurrentDate::<T>::get();
    ensure!(within_limit::<T>(&cursor, target)?, Error::<T>::AccountingPaused);
    let owned;
    let (vip, vipp) = if let Some(job) = frozen {
        (&job.vip, &job.vipp)
    } else {
        owned = sources::<T>(true)?;
        (&owned.0, &owned.1)
    };
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

/// Root approves using the current bounded sources for this specified interval.
/// Repeat with another approved interval if the real clock advances meanwhile.
pub fn approve<T: Config>(target: CurrentDateInfo) -> DispatchResult {
    ensure!(!migration::recovery::is_frozen::<T>(), Error::<T>::AccountingPaused);
    if let Some(job) = ApprovedCatchUp::<T>::get() {
        ensure!(
            !Pallet::<T>::check_correct_date(&CurrentDate::<T>::get(), &job.target),
            Error::<T>::AccountingPaused
        );
    }
    ensure!(
        Pallet::<T>::check_correct_date(&CurrentDate::<T>::get(), &target),
        Error::<T>::NotCorrectDate
    );
    ensure!(!Pallet::<T>::check_correct_date(&today::<T>()?, &target), Error::<T>::NotCorrectDate);
    let (vip, vipp) = sources::<T>(false)?;
    ApprovedCatchUp::<T>::put(CatchUp::<T> { target, source_cursor: 0, vip, vipp });
    Ok(())
}

#[frame_support::transactional]
pub fn step<T: Config>() -> DispatchResult {
    let mut job = ApprovedCatchUp::<T>::get().ok_or(Error::<T>::AccountingPaused)?;
    ensure!(!migration::recovery::is_frozen::<T>(), Error::<T>::AccountingPaused);
    let anchor = CurrentDate::<T>::get();
    if !Pallet::<T>::check_correct_date(&anchor, &job.target) {
        return Ok(());
    }
    let mut limit = anchor.clone();
    limit.add_days::<T>(MAX_UPDATE_DAYS.into())?;
    let target = if Pallet::<T>::check_correct_date(&limit, &job.target) {
        limit
    } else {
        job.target.clone()
    };
    let start = job.source_cursor as usize;
    let end = (start + MAX_SOURCES_PER_STEP).min(job.vip.len() + job.vipp.len());
    let vip: Vec<_> = job
        .vip
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= start && *i < end)
        .map(|(_, source)| source.clone())
        .collect();
    let vipp: Vec<_> = job
        .vipp
        .iter()
        .enumerate()
        .filter(|(i, _)| *i + job.vip.len() >= start && *i + job.vip.len() < end)
        .map(|(_, source)| source.clone())
        .collect();
    let chunk = CatchUp::<T> {
        target: target.clone(),
        source_cursor: 0,
        vip: vip.try_into().map_err(|_| Error::<T>::AccountingCapacityExceeded)?,
        vipp: vipp.try_into().map_err(|_| Error::<T>::AccountingCapacityExceeded)?,
    };
    advance::<T>(&target, Some(&chunk))?;
    if end == job.vip.len() + job.vipp.len() {
        job.source_cursor = 0;
    } else {
        // Annual rows are already persisted for the processed accounts, but the
        // global clock commits only after every source finished this window.
        CurrentDate::<T>::put(anchor);
        job.source_cursor = end as u32;
    }
    ApprovedCatchUp::<T>::put(job);
    Ok(())
}

/// Do not silently credit the days that passed while administrators reviewed
/// the job. An extended interval needs another explicit source approval.
pub fn finish<T: Config>() -> DispatchResult {
    ensure!(!migration::recovery::is_frozen::<T>(), Error::<T>::AccountingPaused);
    ensure!(
        !Pallet::<T>::check_correct_date(&CurrentDate::<T>::get(), &today::<T>()?),
        Error::<T>::AccountingPaused
    );
    if let Some(job) = ApprovedCatchUp::<T>::get() {
        ensure!(
            !Pallet::<T>::check_correct_date(&CurrentDate::<T>::get(), &job.target),
            Error::<T>::AccountingPaused
        );
    }
    ApprovedCatchUp::<T>::kill();
    AccrualReviewRequired::<T>::kill();
    Ok(())
}
