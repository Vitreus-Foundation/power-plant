#![allow(missing_docs)]

use super::*;
use frame_support::{
    migrations::VersionedMigration,
    traits::{Get, StorageVersion, UncheckedOnRuntimeUpgrade},
    weights::Weight,
};

mod v0 {
    use super::*;
    use frame_support::{storage_alias, Twox64Concat};

    #[storage_alias]
    pub type YearVipResults<T: Config> = StorageMap<
        Pallet<T>,
        Twox64Concat,
        i32,
        Vec<(
            <T as frame_system::Config>::AccountId,
            <T as pallet_energy_generation::Config>::StakeBalance,
        )>,
    >;

    #[storage_alias]
    pub type YearVippResults<T: Config> = StorageMap<
        Pallet<T>,
        Twox64Concat,
        i32,
        Vec<(
            <T as frame_system::Config>::AccountId,
            <T as pallet_energy_generation::Config>::StakeBalance,
        )>,
    >;
}

mod v1 {
    use super::v0::{YearVipResults, YearVippResults};
    use super::*;

    pub struct VersionUncheckedMigrateToV1<T>(core::marker::PhantomData<T>);

    impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateToV1<T> {
        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
            let vip = YearVipResults::<T>::iter()
                .flat_map(|(_, results)| results)
                .filter(|(_, points)| !points.is_zero())
                .count();
            let vipp = YearVippResults::<T>::iter()
                .flat_map(|(_, results)| results)
                .filter(|(_, points)| !points.is_zero())
                .count();

            Ok((vip as u32, vipp as u32).encode())
        }

        fn on_runtime_upgrade() -> Weight {
            let mut weight: Weight = Weight::zero();

            for (year, results) in YearVipResults::<T>::drain() {
                let total = results.len();

                let mut migrated = 0;
                for (account, points) in results {
                    if !points.is_zero() {
                        VipPoints::<T>::insert(year as u32, account, points);
                        migrated += 1;
                    }
                }

                log::info!(
                    "Migrated {} out of {} VIP results for the year {}",
                    migrated,
                    total,
                    year
                );

                weight.saturating_accrue(T::DbWeight::get().reads_writes(1, migrated));
            }

            for (year, results) in YearVippResults::<T>::drain() {
                let total = results.len();

                let mut migrated = 0;
                for (account, points) in results {
                    if !points.is_zero() {
                        VippPoints::<T>::insert(year as u32, account, points);
                        migrated += 1;
                    }
                }

                log::info!(
                    "Migrated {} out of {} VIPP results for the year {}",
                    migrated,
                    total,
                    year
                );

                weight.saturating_accrue(T::DbWeight::get().reads_writes(1, migrated));
            }

            weight
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
            let (vip_results_before_upgrade, vipp_results_before_upgrade) =
                <(u32, u32)>::decode(&mut &state[..]).expect("Was properly encoded");

            let vip_results_after_upgrade = VipPoints::<T>::iter().count() as u32;
            let vipp_results_after_upgrade = VippPoints::<T>::iter().count() as u32;

            ensure!(
                vip_results_before_upgrade == vip_results_after_upgrade,
                "Number of VIP results should be the same as the one before the upgrade."
            );
            ensure!(
                vipp_results_before_upgrade == vipp_results_after_upgrade,
                "Number of VIPP results should be the same as the one before the upgrade."
            );

            ensure!(
                YearVipResults::<T>::iter().next() == None,
                "YearVipResults storage should have been removed"
            );
            ensure!(
                YearVippResults::<T>::iter().next() == None,
                "YearVippResults storage should have been removed"
            );

            Ok(())
        }
    }
}

pub type MigrateToV1<T> = VersionedMigration<
    0,
    1,
    v1::VersionUncheckedMigrateToV1<T>,
    Pallet<T>,
    <T as frame_system::Config>::DbWeight,
>;

/// Staged repair of derived membership caches. This deliberately does not migrate points:
/// determining historic entitlement requires the archive replay and an approved payout policy.
///
/// Runtime integration pauses membership/NFT mutations and accrual during recovery.
/// Staking remains operable and refreshes live caches. Explicit review prevents repaired
/// *current* stake from being applied retroactively to an unverified historical interval.
pub mod recovery {
    use super::*;
    use frame_support::{
        storage::StoragePrefixedMap,
        traits::tokens::nonfungibles_v2::{Inspect, InspectEnumerable},
    };
    use parity_scale_codec::{Decode, DecodeAll, MaxEncodedLen};
    use sp_arithmetic::traits::CheckedAdd;
    use sp_runtime::{DispatchError, RuntimeDebug};

    pub const MAX_ACCOUNTS_PER_STEP: u32 = 16;
    pub const MAX_NFTS_PER_ACCOUNT: u32 = 64;
    pub const MAX_MEMBERS_PER_KIND: u32 = 1024;
    const CLAIM_ATTRIBUTE: [u8; 3] = [0, 0, 2];

    pub const STAKE_CHANGED: u16 = 1;
    pub const NFT_CACHE_CHANGED: u16 = 1 << 1;
    pub const ZERO_VALUE_NFT: u16 = 1 << 2;
    pub const NO_OWNED_NFT: u16 = 1 << 3;
    pub const MALFORMED_ATTRIBUTE: u16 = 1 << 4;
    pub const NFT_LIMIT_EXCEEDED: u16 = 1 << 5;
    pub const INACTIVE_NAC: u16 = 1 << 6;
    pub const NOT_A_VIP: u16 = 1 << 7;
    pub const MISSING_NAC: u16 = 1 << 8;
    pub const CACHE_LIMIT_EXCEEDED: u16 = 1 << 9;
    pub const MALFORMED_MEMBER: u16 = 1 << 10;
    pub const UNDERFUNDED: u16 = 1 << 11;
    pub const MALFORMED_NAC_CLAIM: u16 = 1 << 12;
    pub const THRESHOLD_BELOW_NFTS: u16 = 1 << 13;
    pub const QUARANTINED_BALANCE: u16 = 1 << 14;
    pub const NFT_OWNER_MISMATCH: u16 = 1 << 15;
    pub const UNRESOLVED: u16 = MALFORMED_ATTRIBUTE
        | NFT_LIMIT_EXCEEDED
        | MISSING_NAC
        | CACHE_LIMIT_EXCEEDED
        | MALFORMED_MEMBER
        | UNDERFUNDED
        | MALFORMED_NAC_CLAIM
        | THRESHOLD_BELOW_NFTS
        | QUARANTINED_BALANCE
        | NFT_OWNER_MISMATCH;

    #[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
    pub enum Phase {
        Vip,
        Vipp,
        AwaitingReview,
        Complete,
    }

    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
    pub struct Progress {
        pub phase: Phase,
        /// Last fully processed raw storage key, exclusive when resuming.
        pub cursor: Vec<u8>,
        pub scanned_rows: u32,
        pub vip_rows: u32,
        pub vipp_rows: u32,
        pub repaired_accounts: u32,
        pub pending_reviews: u32,
        /// The old accounting anchor is preserved, never moved to "today" by recovery.
        pub original_date: CurrentDateInfo,
    }

    #[derive(Encode, Decode, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct Review<T: Config> {
        pub issues: u16,
        pub vip_points: Option<T::StakeBalance>,
        pub vipp_points: Option<T::StakeBalance>,
        pub previous_active_stake: Option<T::StakeBalance>,
        /// None when the legacy value exceeded the decoding limit; it stays in storage intact.
        pub previous_vipp_cache_hash: Option<[u8; 32]>,
        pub acknowledged: bool,
    }

    pub use crate::{CacheRecoveryProgress, CacheRecoveryReviews};

    /// Idempotent initialization, including after an interrupted runtime upgrade.
    /// Restarting completed recovery requires an explicit new version/migration.
    pub fn begin<T: Config>() -> bool {
        if CacheRecoveryProgress::<T>::exists() {
            return false;
        }
        CacheRecoveryProgress::<T>::put(Progress {
            phase: Phase::Vip,
            cursor: Vec::new(),
            scanned_rows: 0,
            vip_rows: 0,
            vipp_rows: 0,
            repaired_accounts: 0,
            pending_reviews: 0,
            original_date: CurrentDate::<T>::get(),
        });
        true
    }

    pub fn is_frozen<T: Config>() -> bool {
        CacheRecoveryProgress::<T>::get()
            .map(|progress| progress.phase != Phase::Complete)
            .unwrap_or(false)
    }

    /// A bounded recovery step. NFT overflows are quarantined for review rather than
    /// traversing an unbounded collection or truncating an entitlement silently.
    #[frame_support::transactional]
    pub fn step<T: Config>(account_limit: u32, nft_limit: u32) -> Result<Progress, DispatchError> {
        ensure!(
            account_limit > 0
                && account_limit <= MAX_ACCOUNTS_PER_STEP
                && nft_limit > 0
                && nft_limit <= MAX_NFTS_PER_ACCOUNT,
            DispatchError::Other("Invalid recovery batch limits")
        );
        let mut progress = CacheRecoveryProgress::<T>::get()
            .ok_or(DispatchError::Other("Recovery has not started"))?;
        let mut processed = 0;
        while processed < account_limit {
            let prefix = match progress.phase {
                Phase::Vip => VipMembers::<T>::final_prefix(),
                Phase::Vipp => VippMembers::<T>::final_prefix(),
                Phase::AwaitingReview | Phase::Complete => break,
            };
            let cursor = if progress.cursor.is_empty() { &prefix[..] } else { &progress.cursor };
            let next = sp_io::storage::next_key(cursor).filter(|key| key.starts_with(&prefix));
            let Some(key) = next else {
                progress.phase = match progress.phase {
                    Phase::Vip => Phase::Vipp,
                    _ => Phase::AwaitingReview,
                };
                progress.cursor.clear();
                continue;
            };
            // Both member maps use Twox64Concat. Decode the complete key so corruption
            // stops recovery instead of being silently skipped by a storage iterator.
            let account = key
                .get(40..)
                .and_then(|mut bytes| T::AccountId::decode_all(&mut bytes).ok())
                .ok_or(DispatchError::Other("Malformed recovery account key"))?;
            let canonical_key = match progress.phase {
                Phase::Vip => VipMembers::<T>::hashed_key_for(&account),
                _ => VippMembers::<T>::hashed_key_for(&account),
            };
            ensure!(key == canonical_key, DispatchError::Other("Malformed recovery account hash"));
            if progress.phase == Phase::Vip || !VipMembers::<T>::contains_key(&account) {
                let review = repair_account::<T>(&account, nft_limit)?;
                if review.issues != 0 {
                    if review.issues & (STAKE_CHANGED | NFT_CACHE_CHANGED) != 0 {
                        progress.repaired_accounts = progress.repaired_accounts.saturating_add(1);
                    }
                    progress.pending_reviews = progress.pending_reviews.saturating_add(1);
                    CacheRecoveryReviews::<T>::insert(&account, review);
                }
            }
            progress.scanned_rows = progress.scanned_rows.saturating_add(1);
            match progress.phase {
                Phase::Vip => progress.vip_rows = progress.vip_rows.saturating_add(1),
                Phase::Vipp => progress.vipp_rows = progress.vipp_rows.saturating_add(1),
                Phase::AwaitingReview | Phase::Complete => {
                    return Err(DispatchError::Other("Recovery scan is not active"));
                },
            }
            progress.cursor = key;
            processed += 1;
        }
        CacheRecoveryProgress::<T>::put(&progress);
        Ok(progress)
    }

    /// Re-run one quarantined account after its authoritative state was repaired.
    /// The first review retains the original point/stake evidence.
    #[frame_support::transactional]
    pub fn retry_account<T: Config>(account: &T::AccountId, nft_limit: u32) -> DispatchResult {
        ensure!(
            nft_limit > 0 && nft_limit <= MAX_NFTS_PER_ACCOUNT,
            DispatchError::Other("Invalid recovery NFT limit")
        );
        let mut progress = CacheRecoveryProgress::<T>::get()
            .ok_or(DispatchError::Other("Recovery has not started"))?;
        ensure!(
            progress.phase == Phase::AwaitingReview,
            DispatchError::Other("Recovery scan is not finished")
        );
        let mut previous = CacheRecoveryReviews::<T>::get(account)
            .ok_or(DispatchError::Other("No recovery review for account"))?;
        ensure!(
            !previous.acknowledged,
            DispatchError::Other("Recovery review already acknowledged")
        );
        let latest = repair_account::<T>(account, nft_limit)?;
        let repair_flags = STAKE_CHANGED | NFT_CACHE_CHANGED;
        if previous.issues & repair_flags == 0 && latest.issues & repair_flags != 0 {
            progress.repaired_accounts = progress.repaired_accounts.saturating_add(1);
        }
        previous.issues = (previous.issues & !UNRESOLVED) | latest.issues;
        CacheRecoveryReviews::<T>::insert(account, previous);
        CacheRecoveryProgress::<T>::put(progress);
        Ok(())
    }

    /// Called only by an explicitly authorized governance operation after reviewing
    /// historical point preservation. It never changes any point amount itself.
    #[frame_support::transactional]
    pub fn acknowledge<T: Config>(accounts: &[T::AccountId]) -> DispatchResult {
        ensure!(
            accounts.len() <= MAX_ACCOUNTS_PER_STEP as usize,
            DispatchError::Other("Recovery review batch too large")
        );
        let mut progress = CacheRecoveryProgress::<T>::get()
            .ok_or(DispatchError::Other("Recovery has not started"))?;
        ensure!(
            progress.phase == Phase::AwaitingReview,
            DispatchError::Other("Recovery scan is not finished")
        );
        for account in accounts {
            CacheRecoveryReviews::<T>::try_mutate(account, |review| -> DispatchResult {
                let review = review
                    .as_mut()
                    .ok_or(DispatchError::Other("No recovery review for account"))?;
                ensure!(
                    review.issues & UNRESOLVED == 0,
                    DispatchError::Other("Unresolved authoritative recovery data")
                );
                if !review.acknowledged {
                    review.acknowledged = true;
                    progress.pending_reviews = progress.pending_reviews.saturating_sub(1);
                }
                Ok(())
            })?;
        }
        CacheRecoveryProgress::<T>::put(progress);
        Ok(())
    }

    /// Integration must separately approve the accounting date before releasing accrual.
    pub fn finish<T: Config>() -> DispatchResult {
        CacheRecoveryProgress::<T>::try_mutate(|progress| -> DispatchResult {
            let progress =
                progress.as_mut().ok_or(DispatchError::Other("Recovery has not started"))?;
            ensure!(
                progress.phase == Phase::AwaitingReview && progress.pending_reviews == 0,
                DispatchError::Other("Recovery still requires review")
            );
            ensure!(
                progress.vip_rows <= MAX_MEMBERS_PER_KIND
                    && progress.vipp_rows <= MAX_MEMBERS_PER_KIND,
                DispatchError::Other("Recovered population exceeds supported accounting bound")
            );
            progress.phase = Phase::Complete;
            Ok(())
        })
    }

    fn repair_account<T: Config>(
        account: &T::AccountId,
        nft_limit: u32,
    ) -> Result<Review<T>, DispatchError> {
        let vip_before = VipMembers::<T>::get(account);
        let was_vip = vip_before.is_some();
        let malformed_vip = !was_vip && VipMembers::<T>::contains_key(account);
        let vipp_key = VippMembers::<T>::hashed_key_for(account);
        // Old caches used Vec without a bound. Do not decode an arbitrarily large
        // legacy value into memory as part of a supposedly bounded repair.
        let max_cache_bytes = T::StakeBalance::max_encoded_len()
            + 5
            + MAX_NFTS_PER_ACCOUNT as usize
                * (T::ItemId::max_encoded_len() + T::StakeBalance::max_encoded_len());
        let cache_too_large = sp_io::storage::read(&vipp_key, &mut [], 0)
            .map(|size| size as usize > max_cache_bytes)
            .unwrap_or(false);
        let vipp_before = if cache_too_large { None } else { VippMembers::<T>::get(account) };
        let cache_too_large = cache_too_large
            || vipp_before.as_ref().is_some_and(|member| {
                member.active_vipp_threshold.len() > MAX_NFTS_PER_ACCOUNT as usize
            });
        let malformed_vipp =
            !cache_too_large && vipp_before.is_none() && VippMembers::<T>::contains_key(account);
        let preserved_prefix_points = if cache_too_large || malformed_vipp {
            let mut prefix = vec![0u8; T::StakeBalance::max_encoded_len()];
            sp_io::storage::read(&vipp_key, &mut prefix, 0)
                .and_then(|_| T::StakeBalance::decode(&mut prefix.as_slice()).ok())
        } else {
            None
        };
        let mut review = Review::<T> {
            issues: (if cache_too_large { CACHE_LIMIT_EXCEEDED } else { 0 })
                | (if malformed_vip || malformed_vipp { MALFORMED_MEMBER } else { 0 }),
            vip_points: vip_before.as_ref().map(|info| info.points),
            vipp_points: vipp_before.as_ref().map(|info| info.points).or(preserved_prefix_points),
            previous_active_stake: vip_before.as_ref().map(|info| info.active_stake),
            previous_vipp_cache_hash: (!cache_too_large && !malformed_vipp)
                .then(|| sp_io::hashing::blake2_256(&vipp_before.encode())),
            acknowledged: false,
        };
        if malformed_vip || malformed_vipp {
            return Ok(review);
        }
        if let Some(mut vip) = vip_before {
            let stake = pallet_energy_generation::Pallet::<T>::get_active_stake(account);
            if vip.active_stake != stake {
                vip.active_stake = stake;
                VipMembers::<T>::insert(account, vip);
                review.issues |= STAKE_CHANGED;
            }
        }
        if cache_too_large {
            return Ok(review);
        }
        if pallet_nac_managing::Pallet::<T>::is_vipp_quarantined(account) {
            review.issues |= QUARANTINED_BALANCE;
            return Ok(review);
        }

        let collection = T::VIPPCollectionId::get();
        let mut caps = Vec::new();
        let mut owned_count = 0;
        let mut total_claim = <<T as pallet_nac_managing::Config>::Currency as Currency<
            T::AccountId,
        >>::Balance::zero();
        for item_id in
            T::Nfts::owned_in_collection(&collection, account).take(nft_limit as usize + 1)
        {
            owned_count += 1;
            if owned_count > nft_limit {
                review.issues |= NFT_LIMIT_EXCEEDED;
                return Ok(review);
            }
            if T::Nfts::owner(&collection, &item_id).as_ref() != Some(account) {
                review.issues |= NFT_OWNER_MISMATCH;
                return Ok(review);
            }
            let Some(bytes) =
                T::Nfts::system_attribute(&collection, Some(&item_id), &CLAIM_ATTRIBUTE)
            else {
                review.issues |= MALFORMED_ATTRIBUTE;
                return Ok(review);
            };
            let Ok(amount) = <<T as pallet_nac_managing::Config>::Currency as Currency<
                T::AccountId,
            >>::Balance::decode_all(&mut bytes.as_slice()) else {
                review.issues |= MALFORMED_ATTRIBUTE;
                return Ok(review);
            };
            let Some(new_total) = total_claim.checked_add(&amount) else {
                review.issues |= THRESHOLD_BELOW_NFTS;
                return Ok(review);
            };
            total_claim = new_total;
            if amount.is_zero() {
                review.issues |= ZERO_VALUE_NFT;
            } else {
                caps.push((item_id, (Perbill::from_percent(95) * amount).into()));
            }
        }
        if owned_count == 0 && vipp_before.is_some() {
            review.issues |= NO_OWNED_NFT;
        }
        if !was_vip {
            review.issues |= NOT_A_VIP;
            caps.clear();
        }
        let inactive = pallet_nac_managing::Pallet::<T>::is_vipp_inactive(account);
        if owned_count > 0 {
            let Some((_, nac_id)) = pallet_nac_managing::Pallet::<T>::get_nac_level(account) else {
                review.issues |= MISSING_NAC;
                return Ok(review);
            };
            if !inactive {
                let claim = T::Nfts::system_attribute(
                    &T::NftCollectionId::get(), Some(&nac_id), &CLAIM_ATTRIBUTE,
                ).and_then(|bytes| {
                    <<T as pallet_nac_managing::Config>::Currency as Currency<T::AccountId>>::Balance::decode_all(&mut bytes.as_slice()).ok()
                });
                let Some(claim) = claim else {
                    review.issues |= MALFORMED_NAC_CLAIM;
                    return Ok(review);
                };
                if !pallet_nac_managing::Pallet::<T>::threshold_meets_vipp_requirements(
                    account, claim,
                ) {
                    review.issues |= UNDERFUNDED;
                    return Ok(review);
                }
                if total_claim > claim {
                    review.issues |= THRESHOLD_BELOW_NFTS;
                    return Ok(review);
                }
            }
        }
        if inactive {
            if owned_count > 0 || vipp_before.is_some() {
                review.issues |= INACTIVE_NAC;
            }
            caps.clear();
        }
        let old_caps = vipp_before
            .as_ref()
            .map(|info| info.active_vipp_threshold.as_slice())
            .unwrap_or_default();
        if old_caps != caps.as_slice() {
            let points = vipp_before.as_ref().map(|info| info.points).unwrap_or_default();
            VippMembers::<T>::insert(
                account,
                VippMemberInfo::<T> { points, active_vipp_threshold: caps },
            );
            review.issues |= NFT_CACHE_CHANGED;
        }
        // Check the invariant immediately, including ghost/inactive/zero-NFT members.
        ensure!(
            VipMembers::<T>::get(account).map(|info| info.points) == review.vip_points,
            DispatchError::Other("Recovery changed VIP points")
        );
        if let Some(points) = review.vipp_points {
            ensure!(
                VippMembers::<T>::get(account).map(|info| info.points) == Some(points),
                DispatchError::Other("Recovery changed VIPP points")
            );
        }
        Ok(review)
    }
}

/// Install recovery guards without changing points or processing legacy members.
pub struct MigrateToV2<T>(core::marker::PhantomData<T>);

impl<T: Config> frame_support::traits::OnRuntimeUpgrade for MigrateToV2<T> {
    fn on_runtime_upgrade() -> Weight {
        let target_version = StorageVersion::new(2);
        if StorageVersion::get::<Pallet<T>>() < target_version {
            if !TimestampSeenAtBlock::<T>::exists() {
                TimestampSeenAtBlock::<T>::put(
                    frame_system::Pallet::<T>::block_number()
                        .saturating_sub(sp_runtime::traits::One::one()),
                );
            }
            let initialized = migration::recovery::begin::<T>();
            accounting::AccrualReviewRequired::<T>::put(true);
            // This version identifies the installed storage schema, not approval
            // of legacy balances. Recovery progress and the accrual review flag
            // independently keep financial operations paused until governance
            // finishes the bounded repair and approves the calendar. Future
            // upgrades must preserve those guards even when the schema is current.
            if initialized || CacheRecoveryProgress::<T>::get().is_some() {
                target_version.put::<Pallet<T>>();
            } else {
                // Do not certify a schema transition when an existing recovery
                // record cannot decode. The explicit review flag still fails closed.
                log::error!("Privileges recovery state is invalid; schema upgrade remains pending");
            }
            return T::DbWeight::get().reads_writes(6, 4);
        }
        T::DbWeight::get().reads(1)
    }
}
