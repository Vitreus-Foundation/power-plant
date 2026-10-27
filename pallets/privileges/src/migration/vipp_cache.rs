use super::*;
use frame_support::traits::{OnRuntimeUpgrade, StorageVersion};
use sp_runtime::{traits::CheckedAdd, DispatchError};

// Bound the one-shot repair independently of the unbounded legacy storage map.
pub(crate) const MAX_CACHE_MEMBERS: usize = 1024;
type Members<T> = Vec<(<T as frame_system::Config>::AccountId, VippMemberInfo<T>)>;
type Failure<T> = (Option<<T as frame_system::Config>::AccountId>, DispatchError);

/// Replace legacy NAC IDs with owned VIPP IDs without changing accumulated points or NFTs.
pub struct RepairVippCache<T>(core::marker::PhantomData<T>);

impl<T: Config> RepairVippCache<T> {
    fn prepare(reads: &mut u64) -> Result<Members<T>, Failure<T>> {
        let mut members = Vec::new();
        let limit = <T as pallet_nac_managing::Config>::MaxVippNftsPerAccount::get();
        let budget = T::BlockWeights::get().max_block.ref_time() / 2;

        for account in VippMembers::<T>::iter_keys().take(MAX_CACHE_MEMBERS + 1) {
            *reads = reads.saturating_add(1);
            // Reserve the full next ownership scan and writes before performing any more reads.
            let next_reads = reads.saturating_add(2 * u64::from(limit) + 6);
            let next_writes = members.len() as u64 + 2;
            if members.len() == MAX_CACHE_MEMBERS
                || T::DbWeight::get().reads_writes(next_reads, next_writes).ref_time() > budget
            {
                return Err((Some(account), Error::<T>::VippCacheMigrationTooLarge.into()));
            }

            // Value iteration skips undecodable entries, which must instead abort the repair.
            *reads = reads.saturating_add(1);
            let mut member = VippMembers::<T>::get(&account)
                .ok_or_else(|| (Some(account.clone()), Error::<T>::InvalidVippCache.into()))?;
            let owned = match pallet_nac_managing::Pallet::<T>::owned_vipp_claims(&account) {
                Ok(owned) => owned,
                Err(error) => {
                    *reads = next_reads;
                    return Err((Some(account), error));
                },
            };
            *reads = reads.saturating_add(2 * owned.len() as u64 + 1);
            let total = owned
                .iter()
                .try_fold(Zero::zero(), |total, (_, claim)| claim.checked_add(&total))
                .ok_or_else(|| (Some(account.clone()), Error::<T>::InvalidVippBacking.into()))?;
            let mut caps = Vec::new();
            for (id, claim) in owned {
                let cap = Perbill::from_percent(95) * claim;
                if !cap.is_zero() {
                    caps.push((id, cap.into()));
                }
            }
            if !total.is_zero() {
                *reads = reads.saturating_add(3);
                let claim = pallet_nac_managing::Pallet::<T>::try_get_claim_balance(&account)
                    .map_err(|error| (Some(account.clone()), error))?;
                if claim.is_none_or(|(claim, _)| claim < total) {
                    return Err((Some(account), Error::<T>::InvalidVippBacking.into()));
                }
            }
            member.active_vipp_threshold = caps;
            members.push((account, member));
        }
        // The final iterator read establishes that no member remains outside the repair.
        *reads = reads.saturating_add(1);
        Ok(members)
    }
}

impl<T: Config> OnRuntimeUpgrade for RepairVippCache<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut reads = 1u64;
        if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
            return T::DbWeight::get().reads(reads);
        }
        match Self::prepare(&mut reads) {
            Ok(members) => {
                // All fallible reads and backing checks finish before the first storage write.
                let writes = members.len() as u64 + 1;
                for (account, member) in members {
                    VippMembers::<T>::insert(account, member);
                }
                StorageVersion::new(2).put::<Pallet<T>>();
                T::DbWeight::get().reads_writes(reads, writes)
            },
            Err((account, error)) => {
                log::error!("VIPP cache repair failed for {:?}: {:?}", account, error);
                Pallet::<T>::deposit_event(Event::VippCacheMigrationFailed { account, error });
                // Event deposition reads/writes System's event count, records and topics.
                T::DbWeight::get().reads_writes(reads.saturating_add(3), 3)
            },
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
            return Ok(Option::<Members<T>>::None.encode());
        }
        let members = Self::prepare(&mut 1u64).map_err(|(_, error)| error)?;
        Ok(Some(members).encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        use parity_scale_codec::DecodeAll;
        let members = Option::<Members<T>>::decode_all(&mut state.as_slice())
            .map_err(|_| "Cannot decode VIPP cache migration state")?;
        let Some(members) = members else { return Ok(()) };
        ensure!(
            StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
            "Cache repair failed"
        );
        ensure!(
            VippMembers::<T>::iter_keys().count() == members.len(),
            "VIPP cache repair changed membership"
        );
        for (account, expected) in members {
            let actual = VippMembers::<T>::get(&account).ok_or("VIPP member was removed")?;
            ensure!(actual.points == expected.points, "VIPP points changed during cache repair");
            ensure!(
                actual.active_vipp_threshold == expected.active_vipp_threshold,
                "VIPP cache differs from owned, backed NFTs"
            );
        }
        Ok(())
    }
}
