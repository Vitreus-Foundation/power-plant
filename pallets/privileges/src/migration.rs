#![allow(missing_docs)]

use super::*;
use frame_support::{
    migrations::VersionedMigration,
    traits::{Get, UncheckedOnRuntimeUpgrade},
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
            let vip = YearVipResults::<T>::iter().flat_map(|(_, results)| results).count();
            let vipp = YearVippResults::<T>::iter().flat_map(|(_, results)| results).count();

            Ok((vip as u32, vipp as u32).encode())
        }

        fn on_runtime_upgrade() -> Weight {
            let mut weight: Weight = Weight::zero();

            for (year, results) in YearVipResults::<T>::drain() {
                weight.saturating_accrue(T::DbWeight::get().reads_writes(1, results.len() as u64));

                log::info!("Migrating {} VIP results for the year {}", results.len(), year);

                for (account, points) in results {
                    VipPoints::<T>::insert(year as u32, account, points);
                }
            }

            for (year, results) in YearVippResults::<T>::drain() {
                weight.saturating_accrue(T::DbWeight::get().reads_writes(1, results.len() as u64));

                log::info!("Migrating {} VIPP results for the year {}", results.len(), year);

                for (account, points) in results {
                    VippPoints::<T>::insert(year as u32, account, points);
                }
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
