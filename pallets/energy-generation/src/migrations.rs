//!
//! # Module Overview
//!
//! This module defines migrations for the Substrate-based blockchain's staking pallet. It includes
//! logic to migrate storage formats as part of runtime upgrades, ensuring compatibility and efficiency
//! as new versions are introduced. Specifically, this module focuses on migrating the data structure
//! used for storing information about offending validators, transitioning from an older format to a
//! new, streamlined version.
//!
//! # Key Features and Functions
//!
//! - **Migration from Version 14 to Version 15**:
//!   - The migration targets the `OffendingValidators` storage, which previously stored a vector
//!     of tuples (`Vec<(u32, bool)>`) representing validator indices and a boolean indicating
//!     whether the validator was disabled. The new version simplifies this storage to only store
//!     the indices of disabled validators (`Vec<u32>`).
//!
//! - **Unchecked Migration Implementation**:
//!   - `VersionUncheckedMigrateV14ToV15<T>`: This struct implements the migration logic, including
//!     taking the existing `OffendingValidators` entries, filtering for disabled validators, and
//!     respecting the disabling limit set by the `DefaultDisablingStrategy`. The filtered and
//!     truncated list of disabled validators is then saved to the new storage item (`DisabledValidators`).
//!   - `on_runtime_upgrade() -> Weight`: The main function for performing the migration during a
//!     runtime upgrade. It calculates the necessary weights for reading and writing the migration
//!     data, ensuring that the migration is performed efficiently and does not exceed expected
//!     resource limits.
//!
//! - **Try-Runtime Support**:
//!   - The migration supports `try-runtime` testing, which allows for post-upgrade verification
//!     using the `post_upgrade` function. This ensures that after the migration, the old storage
//!     (`OffendingValidators`) is properly cleaned up, adding an extra layer of safety during
//!     runtime upgrades.
//!
//! # Access Control and Security
//!
//! - **Controlled Upgrade Process**: The migration is implemented as part of a `VersionedMigration`,
//!   which allows for orderly progression from version 14 to version 15. Only trusted developers or
//!   administrators can trigger the migration, ensuring that changes are made in a controlled and
//!   auditable manner.
//! - **Data Integrity Verification**: The `post_upgrade` function, available during `try-runtime`,
//!   ensures that no residual data remains in the old storage format. This prevents inconsistencies
//!   and data corruption, providing confidence that the migration has been successful and complete.
//!
//! # Developer Notes
//!
//! - **Migration Efficiency**: The migration function (`on_runtime_upgrade`) uses truncation to
//!   respect the disabling limit set by `DefaultDisablingStrategy`. This helps ensure that only a
//!   limited number of validators are disabled, maintaining network stability while streamlining
//!   data representation.
//! - **Conditional Compilation for Testing**: The module includes conditional compilation for
//!   runtime testing (`#[cfg(feature = "try-runtime")]`). This ensures that the migration logic
//!   can be thoroughly tested in an isolated environment without affecting the live network.
//! - **Versioned Migration Pattern**: Using a versioned migration approach (`VersionedMigration<14, 15, ...>`),
//!   the module provides a clear and well-documented path for upgrading storage formats, making it
//!   easy to understand and track changes across different versions of the pallet.
//!
//! # Usage Scenarios
//!
//! - **Storage Format Evolution**: As the blockchain evolves, older storage formats may become
//!   inefficient or redundant. This migration helps transition to a new format that is easier to
//!   work with and better suited to the current logic of the pallet, which directly impacts
//!   validator accountability mechanisms.
//! - **Runtime Upgrade Preparation**: When planning a runtime upgrade from version 14 to version 15,
//!   the migration must be included to ensure that all relevant data structures are updated
//!   accordingly. This helps prevent runtime errors due to incompatible storage formats.
//! - **Validator Disabling Strategy Change**: The migration also respects a new disabling strategy
//!   (`UpToLimitDisablingStrategy`), which means that only a limited number of validators can be
//!   disabled. This is particularly useful in preventing excessive validator removal, which could
//!   lead to network instability.
//!
//! # Integration Considerations
//!
//! - **Runtime Weight Calculation**: The migration involves reading from the old storage and writing
//!   to the new one. Proper weight calculations are crucial to ensure the migration does not cause
//!   unexpected runtime performance issues. The `DbWeight` associated with reads and writes helps
//!   mitigate these risks by providing accurate metrics for the migration's impact.
//! - **Testing with `try-runtime`**: The use of `try-runtime` for migration testing ensures that the
//!   changes can be validated in an environment that simulates the live blockchain state. This
//!   allows developers to catch potential issues before deploying the migration to the main network,
//!   ensuring smooth and error-free upgrades.
//! - **Respecting the Disabling Limit**: The truncation of migrated validators based on the disabling
//!   limit (`DefaultDisablingStrategy::disable_limit()`) means that the number of disabled validators
//!   will not exceed a predefined threshold. This is an important safeguard to ensure that the
//!   network continues to function properly without a sudden loss of validators.
//!
//! # Example Scenario
//!
//! Suppose the blockchain governance decides to upgrade the staking pallet to improve the efficiency
//! of managing disabled validators. The old format for storing `OffendingValidators` contained both
//! an index and a boolean flag, which has since been deemed redundant. The migration from version 14
//! to version 15 removes the boolean flag, retaining only the indices of disabled validators. This
//! change reduces storage overhead and simplifies the logic for determining which validators are
//! disabled. During the upgrade, the migration code filters the list of validators to include only
//! those marked as disabled, respects the network's disabling limit, and writes the final list to the
//! new storage format (`DisabledValidators`). Post-upgrade testing ensures that the old storage is
//! properly cleaned, preventing any inconsistencies that could arise from leftover data.
//!


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
