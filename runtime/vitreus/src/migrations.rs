#![allow(clippy::collapsible_else_if, unused_parens)]

use super::*;

pub type Permanent = (
    pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
    pallet_energy_generation::migrations::FixCooperatorStake<Runtime>,
);

pub type V0213 =
    (InitTechnicalCommitteeTreasury, pallet_privileges::migration::MigrateToV1<Runtime>);

#[cfg(feature = "mainnet-runtime")]
pub type V0214 = ();
#[cfg(feature = "testnet-runtime")]
pub type V0214 = (
    // Funds the launch-treasury vault with the ED once from the Treasury, so the
    // first routed fee isn't withheld. Idempotent.
    pallet_launch_treasury::migrations::FundVault<Runtime, xcm_config::TreasuryAccount>,
);

pub type V0215 = (
    frame_support::migrations::RemovePallet<
        ManualBridgePalletName,
        <Runtime as frame_system::Config>::DbWeight,
    >,
    ClearManualBridgeFeeStorage,
);

pub type Unreleased = (coretime_migration::MigrateToCoretimeWithoutBroker,);

pub struct InitTechnicalCommitteeTreasury;
impl frame_support::traits::OnRuntimeUpgrade for InitTechnicalCommitteeTreasury {
    fn on_runtime_upgrade() -> Weight {
        if !System::account_exists(&TechnicalCommitteeTreasury::account_id()) {
            let amount = <Balances as Currency<_>>::minimum_balance();
            let res = <Balances as Currency<_>>::transfer(
                &Treasury::account_id(),
                &TechnicalCommitteeTreasury::account_id(),
                amount,
                ExistenceRequirement::KeepAlive,
            );

            match res {
                Ok(_) => {
                    log::info!("Transfer {} tokens from Treasury to TechnicalTreasury", amount)
                },
                Err(_) => log::warn!("Failed to initialize TechnicalTreasury"),
            }
        }

        Weight::zero()
    }
}

parameter_types! {
    pub const ManualBridgePalletName: &'static str = "ManualBridge";
}

pub struct ClearManualBridgeFeeStorage;
impl frame_support::traits::OnRuntimeUpgrade for ClearManualBridgeFeeStorage {
    fn on_runtime_upgrade() -> Weight {
        use sp_io::hashing::twox_128;

        // Keys match `parameter_types! { storage DepositFeePercent / WithdrawalFeePercent }`.
        frame_support::storage::unhashed::kill(&twox_128(b":DepositFeePercent:"));
        frame_support::storage::unhashed::kill(&twox_128(b":WithdrawalFeePercent:"));

        <Runtime as frame_system::Config>::DbWeight::get().writes(2)
    }
}

mod coretime_migration {
    use super::*;
    use frame_support::traits::OnRuntimeUpgrade;

    type MigrateToCoretime =
        coretime::migration::MigrateToCoretime<Runtime, (), GetLegacyLeaseImpl>;

    /// `MigrateToCoretime` for a relay chain without a coretime chain: every parachain gets a
    /// core of its own, with no end, and its messages to the broker are not sent. It also stops
    /// on-demand orders, as no core serves the on-demand pool.
    ///
    /// Runs once, so later upgrades keep the on-demand queue size set on chain. The upstream
    /// `post_upgrade` expects a message to the broker, so the try-runtime checks are our own.
    pub struct MigrateToCoretimeWithoutBroker;
    impl OnRuntimeUpgrade for MigrateToCoretimeWithoutBroker {
        fn on_runtime_upgrade() -> Weight {
            let weight = MigrateToCoretime::on_runtime_upgrade();
            // Zero weight means it found its storage and skipped itself.
            if weight.is_zero() {
                return weight;
            }

            parachains_configuration::ActiveConfig::<Runtime>::mutate(|config| {
                config.scheduler_params.on_demand_queue_max_size = 0
            });
            weight.saturating_add(
                <Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1),
            )
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
            let state = MigrateToCoretime::pre_upgrade()?;
            // A pending config would put the old `num_cores` and `on_demand_queue_max_size`
            // back in its session.
            frame_support::ensure!(
                state.is_empty() || PendingConfigs::get().is_empty(),
                "Configuration changes are pending"
            );
            Ok(state)
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
            if state.is_empty() {
                return Ok(());
            }

            let (core_count, _dmp_queue_size): (u32, u32) = Decode::decode(&mut &state[..])
                .map_err(|_| "Failed to decode the pre-upgrade state")?;
            let config = parachains_configuration::ActiveConfig::<Runtime>::get();
            frame_support::ensure!(
                config.scheduler_params.num_cores == core_count,
                "Not every parachain got a core"
            );
            frame_support::ensure!(
                config.scheduler_params.on_demand_queue_max_size == 0,
                "On-demand orders are still accepted"
            );
            Ok(())
        }
    }

    #[cfg(feature = "try-runtime")]
    #[frame_support::storage_alias]
    type PendingConfigs = StorageValue<
        Configuration,
        Vec<(SessionIndex, parachains_configuration::HostConfiguration<BlockNumber>)>,
        frame_support::pallet_prelude::ValueQuery,
    >;

    /// End of a parachain's lease, in blocks, for the messages `MigrateToCoretime` sends to the
    /// broker.
    struct GetLegacyLeaseImpl;
    impl coretime::migration::GetLegacyLease<BlockNumber> for GetLegacyLeaseImpl {
        fn get_parachain_lease_in_blocks(para: ParaId) -> Option<BlockNumber> {
            use polkadot_runtime_common::traits::Leaser;

            let now = System::block_number();
            let lease = slots::Leases::<Runtime>::get(para);
            if lease.is_empty() {
                return None;
            }
            // The lease hasn't started yet.
            if lease.iter().any(Option::is_none) {
                return None;
            }
            let (index, _) = <Slots as Leaser<BlockNumber>>::lease_period_index(now)?;
            // `LeaseOffset` is zero, so periods start at multiples of `LeasePeriod`.
            Some(index.saturating_add(lease.len() as u32).saturating_mul(LeasePeriod::get()))
        }
    }
}
