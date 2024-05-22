#![allow(clippy::collapsible_else_if, unused_parens)]

use super::*;
use frame_support::dispatch::RawOrigin;
use frame_support::traits::fungibles::roles::Inspect;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use frame_support::weights::constants::RocksDbWeight;
use hex_literal::hex;
use pallet_assets::WeightInfo;
use pallet_claiming::EthereumAddress;
use pallet_energy_generation::migrations::UpdateSlashStorages;
use pallet_energy_generation::ConfigOp;

pub type V0101 = (FixRewards);
pub type V0103 = (UpdateSlashStorages<Runtime>, TransferClaimFrom0x66C6To0xE621);
pub type V0104 = (SetPoolAssetsStorageVersion, InitEnergyBroker);
pub type V0105 = (FixMinCoopReputation, TransferClaimFrom0x48CfTo0x1206, TransferClaimFrom0x419fTo0x1920);

pub type Unreleased =
    ();

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
                    weight += RocksDbWeight::get().reads_writes(4, 3);
                    if pallet_nac_managing::UsersNft::<Runtime>::contains_key(account.0) {
                        log::info!("User {:?} already has NAC (2 level)", account.0);
                    } else {
                        if NacManaging::mint(RawOrigin::Root.into(), 1, account.0).is_err() {
                            log::warn!("NacManaging::mint call failed for {:?}", account.0);
                        } else {
                            log::info!("Mint NAC (1 level) to {:?}", account.0);
                        }
                    }
                }

                weight += RocksDbWeight::get().reads_writes(2, 0);
            }
        }

        weight
    }
}

parameter_types! {
    pub const ClaimAddress0x66C6: EthereumAddress = EthereumAddress(hex!("66C688840c1c2502c603457B0f916bC73b7a1EEA"));
    pub const ClaimAddress0xE621: EthereumAddress = EthereumAddress(hex!("E6219dc7F606EeD6221d23081e82DC747Adf200d"));
}

pub type TransferClaimFrom0x66C6To0xE621 =
    pallet_claiming::migrations::TransferClaim<Runtime, ClaimAddress0x66C6, ClaimAddress0xE621>;

pub struct SetPoolAssetsStorageVersion;

impl OnRuntimeUpgrade for SetPoolAssetsStorageVersion {
    fn on_runtime_upgrade() -> Weight {
        let storage_version = PoolAssets::on_chain_storage_version();
        if storage_version < 1 {
            StorageVersion::new(1).put::<PoolAssets>();
            log::info!("Set PoolAssets StorageVersion");
        }

        RocksDbWeight::get().reads_writes(1, 1)
    }
}

pub struct InitEnergyBroker;

impl OnRuntimeUpgrade for InitEnergyBroker {
    fn on_runtime_upgrade() -> Weight {
        use pallet_asset_rate::WeightInfo as AssetRateWeightInfo;
        use pallet_balances::WeightInfo as BalancesWeightInfo;
        use pallet_energy_broker::WeightInfo as EnergyBrokerWeightInfo;

        let pool_id =
            EnergyBroker::get_pool_id(NativeOrAssetId::Native, NativeOrAssetId::Asset(VNRG::get()));

        let mut weight = RocksDbWeight::get().reads(1);
        if pallet_energy_broker::Pools::<Runtime>::contains_key(pool_id) {
            log::info!("Liquidity pool VTRS/VNRG already exists, skip migration");
            return weight;
        }

        weight += RocksDbWeight::get().reads(1);
        let sudo_address = match Sudo::key() {
            Some(account) => account,
            None => {
                log::warn!("Failed to get sudo account, abort migration");
                return weight;
            },
        };
        let energy_broker_address = EnergyBrokerPalletId::get().into_account_truncating();
        let treasury_address = areas::TreasuryPalletId::get().into_account_truncating();

        let rate = sp_runtime::FixedU128::from_inner(1_111_111_111_111_111_111_111_111_111);

        weight += <Runtime as pallet_asset_rate::Config>::WeightInfo::update();
        if AssetRate::update(RuntimeOrigin::root(), VNRG::get(), rate).is_err() {
            log::warn!("AssetRate::update call failed");
            return weight;
        }
        log::info!("Set gVolt rate to {}", rate);

        weight += <Runtime as pallet_energy_broker::Config>::WeightInfo::create_pool();
        if EnergyBroker::create_pool(
            RuntimeOrigin::root(),
            sudo_address,
            NativeOrAssetId::Native,
            NativeOrAssetId::Asset(VNRG::get()),
        )
        .is_err()
        {
            log::warn!("EnergyBroker::create_pool call failed");
            return weight;
        }
        log::info!("Create liquidity pool VTRS/VNRG");

        weight += <Runtime as pallet_balances::Config>::WeightInfo::transfer_keep_alive();
        if Balances::transfer_keep_alive(
            RuntimeOrigin::signed(sudo_address),
            treasury_address,
            100 * UNITS,
        )
        .is_err()
        {
            log::warn!("Balances::transfer_keep_alive call failed");
            return weight;
        }
        log::info!("Transfer 100 VTRS from sudo to treasury");

        weight += RocksDbWeight::get().reads_writes(2, 2);
        if Assets::mint_into(VNRG::get(), &sudo_address, 100_000_000_000).is_err() {
            log::warn!("Assets::mint_into call failed");
            return weight;
        };
        log::info!("Mint 100 VNRG to sudo account");

        weight += RocksDbWeight::get().reads(1);
        let vtrs_amount = Balances::free_balance(energy_broker_address);
        let vnrg_amount = 1;

        weight += <Runtime as pallet_balances::Config>::WeightInfo::transfer_allow_death();
        if Balances::transfer_allow_death(
            RuntimeOrigin::signed(energy_broker_address),
            sudo_address,
            vtrs_amount,
        )
        .is_err()
        {
            log::warn!("Balances::transfer_allow_death call failed");
            return weight;
        }
        log::info!("Transfer {vtrs_amount} VTRS units from energy broker to sudo address");

        weight += <Runtime as pallet_energy_broker::Config>::WeightInfo::add_liquidity();
        if EnergyBroker::add_liquidity(
            RuntimeOrigin::signed(sudo_address),
            NativeOrAssetId::Native,
            NativeOrAssetId::Asset(VNRG::get()),
            vtrs_amount,
            vnrg_amount,
            0,
            0,
            treasury_address,
        )
        .is_err()
        {
            log::warn!("EnergyBroker::add_liquidity call failed");
            return weight;
        }
        log::info!(
            "Add {vtrs_amount} VTRS units and {vnrg_amount} VNRG units from sudo to the pool"
        );

        weight
    }
}

pub struct FixMinCoopReputation;

impl OnRuntimeUpgrade for FixMinCoopReputation {
    fn on_runtime_upgrade() -> Weight {
        use pallet_energy_generation::ValidatorPrefs;

        pallet_energy_generation::Validators::<Runtime>::translate(
            |account, mut prefs: ValidatorPrefs| {
                if let Some(tier) = prefs.min_coop_reputation.tier() {
                    let points = ReputationPoint::from_rank(tier.rank());

                    if points != prefs.min_coop_reputation.points() {
                        let old_min_coop_reputation = prefs.min_coop_reputation.clone();

                        prefs.min_coop_reputation.update(points);

                        log::info!(
                            "{:?}: fix min_coop_reputation from {:?} to {:?}",
                            account,
                            old_min_coop_reputation,
                            prefs.min_coop_reputation,
                        );
                    }
                }
                Some(prefs)
            },
        );

        let count = pallet_energy_generation::Validators::<Runtime>::count() as u64;
        RocksDbWeight::get().reads_writes(count, count)
    }
}

parameter_types! {
    pub const ClaimAddress0x48Cf: EthereumAddress = EthereumAddress(hex!("48Cf646cEbd1D6035cb148d4a639BFb1A5118ed9"));
    pub const ClaimAddress0x1206: EthereumAddress = EthereumAddress(hex!("12066b03B34e4321B87626496577ba94bff6fC0C"));
    pub const ClaimAddress0x419f: EthereumAddress = EthereumAddress(hex!("419ff94Fc4309Ce3E0Ccb40782F2B0e5Bd21b158"));
    pub const ClaimAddress0x1920: EthereumAddress = EthereumAddress(hex!("1920503F35a4014ad197e1AD4e310A69BF01dB34"));

}

pub type TransferClaimFrom0x48CfTo0x1206 =
    pallet_claiming::migrations::TransferClaim<Runtime, ClaimAddress0x48Cf, ClaimAddress0x1206>;

pub type TransferClaimFrom0x419fTo0x1920 =
    pallet_claiming::migrations::TransferClaim<Runtime, ClaimAddress0x419f, ClaimAddress0x1920>;
