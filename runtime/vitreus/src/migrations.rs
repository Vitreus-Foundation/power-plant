#![allow(clippy::collapsible_else_if, unused_parens)]

use super::*;

pub type Permanent = (
    pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
    pallet_energy_generation::migrations::FixCooperatorStake<Runtime>,
);

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

pub type Unreleased = (claiming::RemoveStorage);

mod claiming {
    use frame_support::parameter_types;

    parameter_types! {
        pub const Claiming: &'static str = "Claiming";
        pub const Nfts: &'static str = "Nfts";
        pub const Total: &'static str = "Total";
    }

    pub type RemoveStorage = (
        frame_support::migrations::RemoveStorage<
            Claiming,
            Nfts,
            frame_support::weights::constants::RocksDbWeight,
        >,
        frame_support::migrations::RemoveStorage<
            Claiming,
            Total,
            frame_support::weights::constants::RocksDbWeight,
        >,
    );
}
