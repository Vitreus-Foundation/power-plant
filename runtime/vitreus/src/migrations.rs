#![allow(unused_parens)]

use super::*;
use frame_support::dispatch::RawOrigin;
use frame_support::traits::fungibles::roles::Inspect;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::constants::RocksDbWeight;
use pallet_assets::WeightInfo;
use pallet_energy_generation::ConfigOp;

pub type V0101 = ();
pub type Unreleased = (FixRewards);

pub struct FixRewards;

impl OnRuntimeUpgrade for FixRewards {
    fn on_runtime_upgrade() -> Weight {
        let new_energy_per_stake_currency = 19_909_091_036_891;

        let mut weight = RocksDbWeight::get().reads_writes(1, 1);
        if EnergyGeneration::current_energy_per_stake_currency() != Some(1_000_000) {
            log::info!("current_energy_per_stake_currency != 1_000_000, skip migration");
            return weight;
        }
        if EnergyGeneration::set_energy_per_stake_currency(
            RawOrigin::Root.into(),
            new_energy_per_stake_currency,
        )
        .is_err()
        {
            log::warn!(
                "EnergyGeneration::set_energy_per_stake_currency call failed, abort migration"
            );
            return weight;
        }

        let new_min_cooperator_bond = 1_000_000_000_000_000_000;

        log::info!("Change MinCooperatorBond parameters");
        weight += RocksDbWeight::get().reads_writes(1, 1);
        if EnergyGeneration::set_staking_configs(
            RawOrigin::Root.into(),
            ConfigOp::Set(new_min_cooperator_bond),
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
            ConfigOp::Noop,
        )
        .is_err()
        {
            log::warn!("EnergyGeneration::set_staking_configs call failed, abort migration");
            return weight;
        }

        log::info!("Fix current_energy_per_stake_currency");
        pallet_energy_generation::ErasEnergyPerStakeCurrency::<Runtime>::translate::<EraIndex, _>(
            |era, _| {
                weight += RocksDbWeight::get().reads_writes(1, 1);
                log::info!("Fix ErasEnergyPerStakeCurrency for EraIndex {era}");
                Some(new_energy_per_stake_currency)
            },
        );

        weight += RocksDbWeight::get().reads(1);
        if let Some(admin) = Assets::admin(VNRG::get()) {
            for account in frame_system::Account::<Runtime>::iter() {
                if let Some(amount) = Assets::maybe_balance(VNRG::get(), account.0) {
                    let new_amount = amount / 19909091;
                    let burn = amount - new_amount;

                    weight += <Runtime as pallet_assets::Config>::WeightInfo::burn();
                    let res = Assets::burn(
                        RawOrigin::Signed(admin).into(),
                        VNRG::get().into(),
                        account.0,
                        burn,
                    );
                    if res.is_ok() {
                        log::info!(
                            "Change VNRG balance for {:?} from {} to {}",
                            account.0,
                            amount,
                            new_amount
                        );
                    } else {
                        log::warn!("Failed to burn VNRG for {:?}", account.0);
                    }
                }

                if account.1.data.reserved != 0 {
                    weight += RocksDbWeight::get().reads_writes(1, 1);
                    if NacManaging::mint(RawOrigin::Signed(admin).into(), 1, account.0).is_err() {
                        log::warn!("NacManaging::mint call failed, abort migration");
                        return weight;
                    } else {
                        log::info!("Mint NAC (1 level) to {:?}", account.0);
                    }
                }

                weight += RocksDbWeight::get().reads_writes(2, 0);
            }
        }

        weight
    }
}
