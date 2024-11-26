#![allow(clippy::collapsible_else_if, unused_parens)]

use super::*;

pub type Permanent = (pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>);

pub type V0200 = (
    pallet_grandpa::migrations::MigrateV4ToV5<Runtime>,
    pallet_energy_generation::migrations::v15::MigrateV14ToV15<Runtime>,
    polkadot_runtime_parachains::configuration::migration::v7::MigrateToV7<Runtime>,
    polkadot_runtime_parachains::configuration::migration::v8::MigrateToV8<Runtime>,
    polkadot_runtime_parachains::configuration::migration::v9::MigrateToV9<Runtime>,
    polkadot_runtime_parachains::configuration::migration::v10::MigrateToV10<Runtime>,
    polkadot_runtime_parachains::configuration::migration::v11::MigrateToV11<Runtime>,
    polkadot_runtime_parachains::configuration::migration::v12::MigrateToV12<Runtime>,
    polkadot_runtime_parachains::inclusion::migration::MigrateToV1<Runtime>,
    polkadot_runtime_parachains::scheduler::migration::MigrateV0ToV1<Runtime>,
    polkadot_runtime_parachains::scheduler::migration::MigrateV1ToV2<Runtime>,
    polkadot_runtime_common::paras_registrar::migration::MigrateToV1<Runtime, ()>,
);

pub type Unreleased = (energy_broker::MigrateToEnergyBrokerV2);

mod energy_broker {
    use super::*;
    use frame_support::traits::{
        fungibles::{Inspect, Mutate},
        tokens::Preservation::Expendable,
        OnRuntimeUpgrade, StorageVersion,
    };

    #[cfg(feature = "try-runtime")]
    use {
        frame_support::ensure,
        parity_scale_codec::{Decode, Encode},
        sp_std::vec::Vec,
    };

    parameter_types! {
        pub PoolAccount: AccountId = AccountId::from(hex_literal::hex!("465c19fe9C8240B66422A8C9Ef8A23693bf23794"));
        pub const EnergyBroker: &'static str = "EnergyBroker";
        pub const Pools: &'static str = "Pools";
        pub const NextPoolAssetId: &'static str = "NextPoolAssetId";
    }

    pub type MigrateToEnergyBrokerV2 = (
        FixStorageVersion<Runtime>,
        MigrateEnergyBrokerAccount<Runtime, PoolAccount>,
        frame_support::migrations::RemoveStorage<EnergyBroker, Pools, RuntimeDbWeight>,
        frame_support::migrations::RemoveStorage<EnergyBroker, NextPoolAssetId, RuntimeDbWeight>,
    );

    pub struct FixStorageVersion<T>(PhantomData<T>);

    impl<T: pallet_energy_broker::Config> OnRuntimeUpgrade for FixStorageVersion<T> {
        fn on_runtime_upgrade() -> Weight {
            if !StorageVersion::exists::<pallet_energy_broker::Pallet<T>>() {
                StorageVersion::new(0).put::<pallet_energy_broker::Pallet<T>>();
            }

            T::DbWeight::get().reads_writes(1, 1)
        }
    }

    pub struct MigrateEnergyBrokerAccount<T, PoolAccount>(PhantomData<(T, PoolAccount)>);

    impl<T, PoolAccount> OnRuntimeUpgrade for MigrateEnergyBrokerAccount<T, PoolAccount>
    where
        T: pallet_energy_broker::Config,
        PoolAccount: Get<T::AccountId>,
    {
        fn on_runtime_upgrade() -> Weight {
            if !frame_system::Pallet::<T>::account_exists(&PoolAccount::get()) {
                log::warn!("Migration MigrateEnergyBrokerAccount can be removed");
                return T::DbWeight::get().reads_writes(1, 0);
            }

            let broker_account = pallet_energy_broker::Pallet::<T>::account_id();

            let amount = T::Assets::balance(T::NativeAsset::get(), &PoolAccount::get());
            match T::Assets::transfer(
                T::NativeAsset::get(),
                &PoolAccount::get(),
                &broker_account,
                amount,
                Expendable,
            ) {
                Ok(_) => log::info!("Transferred {:?} VTRS", amount),
                Err(err) => log::error!("Failed to transfer VTRS: {:?}", err),
            }

            let amount = T::Assets::balance(T::EnergyAsset::get(), &PoolAccount::get());
            match T::Assets::transfer(
                T::EnergyAsset::get(),
                &PoolAccount::get(),
                &broker_account,
                amount,
                Expendable,
            ) {
                Ok(_) => log::info!("Transferred {:?} VNRG", amount),
                Err(err) => log::error!("Failed to transfer VNRG: {:?}", err),
            }

            if let Err(err) = frame_system::Pallet::<T>::dec_providers(&PoolAccount::get()) {
                log::warn!("Failed to decrement provider counter: {:?}", err);
            }

            T::DbWeight::get().reads_writes(8, 5)
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
            let state = if frame_system::Pallet::<T>::account_exists(&PoolAccount::get()) {
                let native = T::Assets::balance(T::NativeAsset::get(), &PoolAccount::get());
                let energy = T::Assets::balance(T::EnergyAsset::get(), &PoolAccount::get());

                Some((native, energy))
            } else {
                None
            };

            Ok(state.encode())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
            if let Some((native, energy)) = Decode::decode(&mut &state[..]).unwrap() {
                ensure!(
                    T::Assets::balance(T::NativeAsset::get(), &PoolAccount::get()).is_zero(),
                    "VTRS balance of the pool account should be zero"
                );
                ensure!(
                    T::Assets::balance(T::EnergyAsset::get(), &PoolAccount::get()).is_zero(),
                    "VNRG balance of the pool account should be zero"
                );

                let broker_account = pallet_energy_broker::Pallet::<T>::account_id();

                ensure!(
                    T::Assets::balance(T::NativeAsset::get(), &broker_account) == native,
                    "VTRS balance should have been transferred to the broker account"
                );
                ensure!(
                    T::Assets::balance(T::EnergyAsset::get(), &broker_account) == energy,
                    "VNRG balance should have been transferred to the broker account"
                );
            }
            Ok(())
        }
    }
}
