//! # Migration Module for Claims Pallet
//!
//! This module handles the migration of storage in the `Claims` pallet from an older format
//! to a new one.

use super::*;
use frame_support::traits::OnRuntimeUpgrade;

#[cfg(feature = "try-runtime")]
use sp_runtime::{traits::Zero, Saturating, TryRuntimeError};

/// Migrate the `Claims` storage from a single map to a double map.
///
/// This function performs the following actions:
/// 1. Iterates over all entries in the old `Claims` storage (a `StorageMap`).
/// 2. Transforms each entry into the new `ClaimsAmount` storage (a `StorageDoubleMap`),
///    assigning a default `presale_id` of `1` to each migrated entry.
/// 3. Updates the storage version to `1` to indicate that the migration is complete.
///
/// # Process
/// - Reads each entry from the old `Claims` storage.
/// - Inserts the entry into the new `ClaimsAmount` storage with an additional key `presale_id`.
/// - Logs the processed keys and values for debugging purposes.
///
/// # Parameters
/// - `T`: The pallet configuration type that implements the `Config` trait.
///
/// # Returns
/// - The weight consumed during the migration, calculated based on the number of storage reads and writes.
///
/// # Storage Changes
/// - `Claims`:
///   - All existing entries are drained (removed) from this storage.
/// - `ClaimsAmount`:
///   - All drained entries are inserted into this new storage with the additional key `presale_id = 1`.
/// - Storage version:
///   - Updated from `0` to `1` to indicate the migration is complete.
///
/// # Logs
/// - Logs the key and value of each migrated entry for debugging and verification.
///
/// # Weight Calculation
/// - The weight is calculated as the sum of:
///   - `1` read and `1` write for each migrated entry.
///   - An additional read and write for updating the storage version.

pub fn migrate_to_double_map<T: Config>() -> Weight {
    let mut reads = 0;
    let mut writes = 0;

    if StorageVersion::get::<Pallet<T>>() == 0 {
        for (key, value) in <Claims<T>>::iter() {
            reads += 1;
            writes += 1;

            log::info!("Processing key: {:?}, value: {:?}", key, value);

            let presale_id: u16 = 1; // Assign a default presale ID
            <ClaimsAmount<T>>::insert(key, presale_id, value);
        }

        let _ = <Claims<T>>::clear(u32::MAX, None);
    }

    // Update storage version to 1
    StorageVersion::new(1).put::<Pallet<T>>();
    log::info!("Migration to double map completed.");

    T::DbWeight::get().reads_writes(reads + 1, writes + 1)
}

/// Migration to transform Claims storage from a single map to a double map.
/// This migration introduces a second key `u16` (presale ID) to the Claims storage.
/// All existing entries will be migrated to the new format, with the presale ID defaulted to `1`.
pub struct MigrateToDoubleMap<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateToDoubleMap<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut weight: Weight = T::DbWeight::get().reads_writes(0, 0);

        weight += migrate_to_double_map::<T>();
        weight
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        // Calculate the total balance before migration
        let total = <ClaimsAmount<T>>::iter_values()
            .fold(BalanceOf::<T>::zero(), |acc, balance| acc.saturating_add(balance));
        log::info!("Pre-upgrade total balance: {:?}", total);

        Ok(total.encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        // Decode the total balance from pre-upgrade
        let old_total: BalanceOf<T> =
            Decode::decode(&mut &state[..]).expect("pre_upgrade provides a valid state; qed");

        // Calculate the total balance after migration
        let new_total = <ClaimsAmount<T>>::iter_values()
            .fold(BalanceOf::<T>::zero(), |acc, balance| acc.saturating_add(balance));

        log::info!("Post-upgrade total balance: {:?}", new_total);

        // Ensure the total balance remains unchanged
        ensure!(
            new_total == old_total,
            "Total balance of claims should not change during migration"
        );

        Ok(())
    }
}
