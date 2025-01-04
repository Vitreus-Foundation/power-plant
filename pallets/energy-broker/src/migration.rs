//! Energy Broker migrations.

use super::*;
use frame_support::{
    migrations::VersionedMigration, traits::UncheckedOnRuntimeUpgrade, weights::Weight,
};

mod v1 {
    use super::*;

    pub struct VersionUncheckedMigrateToV1<T, Capacity>(core::marker::PhantomData<(T, Capacity)>);

    impl<T: Config, Capacity: Get<T::Balance>> UncheckedOnRuntimeUpgrade
        for VersionUncheckedMigrateToV1<T, Capacity>
    {
        fn on_runtime_upgrade() -> Weight {
            EnergyCapacity::<T>::put(Capacity::get());
            EnergyCapacityOverride::<T>::put(Capacity::get());

            log::info!("Set energy capacity to {:?}", Capacity::get());

            T::DbWeight::get().writes(2)
        }
    }
}

/// Initialize energy capacity.
pub type MigrateToV1<T, Capacity> = VersionedMigration<
    0,
    1,
    v1::VersionUncheckedMigrateToV1<T, Capacity>,
    Pallet<T>,
    <T as frame_system::Config>::DbWeight,
>;
