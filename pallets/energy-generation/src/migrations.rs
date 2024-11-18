use super::*;

use frame_support::{
    migrations::VersionedMigration, traits::UncheckedOnRuntimeUpgrade, weights::Weight,
};

use crate::{Config, Pallet};

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
