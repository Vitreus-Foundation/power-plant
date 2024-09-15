use core::marker::PhantomData;

use super::*;
use crate::slashing::{SlashEntityOf, SlashEntityPerbill, SpanRecord};
use crate::{Config, Pallet};
use frame_support::{
    migrations::VersionedMigration,
    traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion, UncheckedOnRuntimeUpgrade},
    weights::Weight,
};
use pallet_reputation::ReputationPoint;

#[cfg(feature = "try-runtime")]
use frame_support::ensure;
#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

/// Migrating `OffendingValidators` from `Vec<(u32, bool)>` to `Vec<u32>`
pub mod v15 {
    use super::*;

    type DefaultDisablingStrategy = UpToLimitDisablingStrategy;

    pub struct VersionUncheckedMigrateV14ToV15<T>(core::marker::PhantomData<T>);
    impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateV14ToV15<T> {
        fn on_runtime_upgrade() -> Weight {
            let mut migrated = v14::OffendingValidators::<T>::take()
                .into_iter()
                .filter(|p| p.1) // take only disabled validators
                .map(|p| p.0)
                .collect::<Vec<_>>();

            // Respect disabling limit
            migrated.truncate(DefaultDisablingStrategy::disable_limit(
                T::SessionInterface::validators().len(),
            ));

            DisabledValidators::<T>::set(migrated);

            log!(info, "v15 applied successfully.");
            T::DbWeight::get().reads_writes(1, 1)
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
            frame_support::ensure!(
                v14::OffendingValidators::<T>::decode_len().is_none(),
                "OffendingValidators is not empty after the migration"
            );
            Ok(())
        }
    }

    pub type MigrateV14ToV15<T> = VersionedMigration<
        14,
        15,
        VersionUncheckedMigrateV14ToV15<T>,
        Pallet<T>,
        <T as frame_system::Config>::DbWeight,
    >;
}

pub mod v14 {
    use super::*;
    use frame_support::pallet_prelude::ValueQuery;

    #[frame_support::storage_alias]
    pub(crate) type OffendingValidators<T: Config> =
        StorageValue<Pallet<T>, Vec<(u32, bool)>, ValueQuery>;
}

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
        let storage_version = Pallet::<T>::on_chain_storage_version();
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
            unapplied_slashes: UnappliedSlashes::<T>::iter_keys().count() as u128,
            validator_slash_in_era: ValidatorSlashInEra::<T>::iter_keys().count() as u128,
            cooperator_slash_in_era: CooperatorSlashInEra::<T>::iter_keys().count() as u128,
            span_slash: SpanSlash::<T>::iter_keys().count() as u128,
        };
        Ok(counter.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        let new_counter = StorageElementsCounter {
            unapplied_slashes: UnappliedSlashes::<T>::iter_keys().count() as u128,
            validator_slash_in_era: ValidatorSlashInEra::<T>::iter_keys().count() as u128,
            cooperator_slash_in_era: CooperatorSlashInEra::<T>::iter_keys().count() as u128,
            span_slash: SpanSlash::<T>::iter_keys().count() as u128,
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
