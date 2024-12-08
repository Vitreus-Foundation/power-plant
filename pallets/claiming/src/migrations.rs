use super::*;
use frame_support::traits::OnRuntimeUpgrade;

#[cfg(feature = "try-runtime")]
use sp_runtime::{traits::Zero, Saturating, TryRuntimeError};

/// Migration to transform Claims storage from a single map to a double map.
/// This migration introduces a second key `u16` (presale ID) to the Claims storage.
/// All existing entries will be migrated to the new format, with the presale ID defaulted to `1`.
pub struct MigrateToDoubleMap<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MigrateToDoubleMap<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut weight: Weight = T::DbWeight::get().reads_writes(0, 0);

        // Migrate all entries in the existing Claims storage
        Claims::<T>::translate(|ethereum_address, balance: BalanceOf<T>| {
            let presale_id = 1;

            // Log the migration for each address
            if !<Claims<T>>::contains_key(ethereum_address, presale_id) {
                log::info!(
                    "Migrating claim for EthereumAddress: {:?} with presale_id: {}",
                    ethereum_address,
                    presale_id
                );
            } else {
                log::warn!(
                        "Claim for EthereumAddress: {:?} with presale_id: {} already exists. Skipping migration.",
                        ethereum_address,
                        presale_id
                    );
            }

            weight += T::DbWeight::get().reads_writes(1, 1);

            Some((presale_id, balance))
        });

        weight
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        // Calculate the total balance before migration
        let total = <Claims<T>>::iter_values()
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
        let new_total = <Claims<T>>::iter_values()
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
