use core::marker::PhantomData;

use super::*;
use crate::slashing::{SlashEntityOf, SlashEntityPerbill, SpanRecord};
use crate::{Config, Pallet};
#[cfg(feature = "try-runtime")]
use frame_support::ensure;
use frame_support::traits::{OnRuntimeUpgrade, StorageVersion};
use pallet_reputation::ReputationPoint;
#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

pub struct UpdateSlashStorages<T>(PhantomData<T>);

#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
struct OldUnappliedSlash<AccountId> {
    pub validator: AccountId,
    pub own: ReputationPoint,
    pub others: Vec<(AccountId, ReputationPoint)>,
    pub reporters: Vec<AccountId>,
    pub payout: ReputationPoint,
}

#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
struct OldSpanRecord {
    pub slashed: ReputationPoint,
    pub paid_out: ReputationPoint,
}

#[derive(Encode, Decode, PartialEq, Eq)]
struct StorageElementsCounter {
    unapplied_slashes: u128,
    validator_slash_in_era: u128,
    cooperator_slash_in_era: u128,
    span_slash: u128,
}

impl<T: Config> OnRuntimeUpgrade for UpdateSlashStorages<T> {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        let storage_version = StorageVersion::get::<Pallet<T>>();
        if storage_version != 13 {
            log::info!("Invalid storage version (current {:?}), skip migration", storage_version);
            return Zero::zero();
        }
        log::info!("Starting EnergyGeneration migration");

        let mut weight = T::DbWeight::get().reads(0);

        UnappliedSlashes::<T>::translate::<Vec<OldUnappliedSlash<T::AccountId>>, _>(|_, value| {
            let new_value = value
                .into_iter()
                .map(|old_unapplied_slash| {
                    let OldUnappliedSlash { validator, own, others, reporters, payout } =
                        old_unapplied_slash;
                    let own = SlashEntityOf::<T>::new(own, Zero::zero());
                    let others = others
                        .into_iter()
                        .map(|(account, reputation)| {
                            (account, SlashEntityOf::<T>::new(reputation, Zero::zero()))
                        })
                        .collect();
                    let payout = SlashEntityOf::<T>::new(payout, Zero::zero());
                    UnappliedSlash::new(validator, own, others, reporters, payout)
                })
                .collect();

            weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            Some(new_value)
        });

        ValidatorSlashInEra::<T>::translate::<(Perbill, ReputationPoint), _>(
            |_, _, (ratio, reputation)| {
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
                let slash_ratio = SlashEntityPerbill::new(ratio, Zero::zero());
                Some((slash_ratio, SlashEntityOf::<T>::new(reputation, Zero::zero())))
            },
        );
        CooperatorSlashInEra::<T>::translate::<ReputationPoint, _>(|_, _, reputation| {
            weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            Some(SlashEntityOf::<T>::new(reputation, Zero::zero()))
        });
        SpanSlash::<T>::translate::<OldSpanRecord, _>(|_, old_span_record| {
            weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            let OldSpanRecord { slashed, paid_out } = old_span_record;
            let new_slashed = SlashEntityOf::<T>::new(slashed, Zero::zero());
            let new_paid_out = SlashEntityOf::<T>::new(paid_out, Zero::zero());
            Some(SpanRecord::new(new_slashed, new_paid_out))
        });

        StorageVersion::new(14).put::<Pallet<T>>();
        weight.saturating_add(T::DbWeight::get().writes(1))
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        let counter = StorageElementsCounter {
            unapplied_slashes: UnappliedSlashes::<T>::iter().count() as u128,
            validator_slash_in_era: ValidatorSlashInEra::<T>::iter().count() as u128,
            cooperator_slash_in_era: CooperatorSlashInEra::<T>::iter().count() as u128,
            span_slash: SpanSlash::<T>::iter().count() as u128,
        };
        Ok(counter.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let new_counter = StorageElementsCounter {
            unapplied_slashes: UnappliedSlashes::<T>::iter().count() as u128,
            validator_slash_in_era: ValidatorSlashInEra::<T>::iter().count() as u128,
            cooperator_slash_in_era: CooperatorSlashInEra::<T>::iter().count() as u128,
            span_slash: SpanSlash::<T>::iter().count() as u128,
        };
        let old_counter = StorageElementsCounter::decode(&mut state.as_slice())
            .map_err(|_| TryRuntimeError::Corruption)?;
        ensure!(old_counter == new_counter, TryRuntimeError::Corruption);
        ensure!(
            Pallet::<T>::on_chain_storage_version() == 14,
            TryRuntimeError::Other("Storage version wasn't updated")
        );
        Ok(())
    }
}
