use super::*;

use frame_support::{
    migrations::VersionedMigration,
    traits::{OnRuntimeUpgrade, UncheckedOnRuntimeUpgrade},
    weights::Weight,
};

use crate::{Config, Pallet};

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

pub mod v16 {
    use super::*;
    use sp_runtime::{FixedPointNumber, FixedU128};

    pub struct VersionUncheckedMigrateV15ToV16<T>(core::marker::PhantomData<T>);
    impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateV15ToV16<T> {
        fn on_runtime_upgrade() -> Weight {
            let mut count = 0;

            ErasEnergyPerStakeCurrency::<T>::translate_values(|rate: EnergyOf<T>| {
                count += 1;
                FixedU128::checked_from_rational(1, rate)
            });

            log!(info, "Upgraded {} records", count);
            T::DbWeight::get().reads_writes(count, count)
        }
    }

    pub type MigrateV15ToV16<T> = VersionedMigration<
        15,
        16,
        VersionUncheckedMigrateV15ToV16<T>,
        Pallet<T>,
        <T as frame_system::Config>::DbWeight,
    >;
}

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

pub struct FixCooperatorStake<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for FixCooperatorStake<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut count = 0;
        let mut fixed = 0;

        for (_, ledger) in Ledger::<T>::iter() {
            if let Some(cooperations) = Cooperators::<T>::get(&ledger.stash) {
                let targets_stake = cooperations.total();
                if targets_stake > ledger.active {
                    Pallet::<T>::adjust_cooperator_targets(&ledger.stash, ledger.active);

                    log!(
                        info,
                        "Fix {:?} stake: active/targets/difference: {:?}/{:?}/{:?}",
                        ledger.stash,
                        ledger.active,
                        targets_stake,
                        targets_stake - ledger.active
                    );

                    fixed += 1;
                }
            }
            count += 1;
        }

        log!(info, "Fixed {} out of {} cooperators", fixed, count);

        T::DbWeight::get().reads_writes(count * 2, fixed)
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_: Vec<u8>) -> Result<(), TryRuntimeError> {
        for (_, ledger) in Ledger::<T>::iter() {
            if let Some(cooperations) = Cooperators::<T>::get(&ledger.stash) {
                frame_support::ensure!(
                    cooperations.total() <= ledger.active,
                    "cooperator targets stake must not exceed active stake"
                );
            }
        }

        Ok(())
    }
}
