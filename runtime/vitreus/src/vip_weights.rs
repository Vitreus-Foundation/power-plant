//! Weight allowance for VIP staking callbacks.
use crate::Runtime;
use frame_support::weights::Weight;
pub struct VipWeights;
impl pallet_energy_generation::WeightInfo for VipWeights {
    fn stake_refresh_hook() -> Weight {
        stake_refresh_weight(1)
    }

    fn chill_hook(collaborators: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::chill()
            .saturating_add(stake_refresh_weight(2))
            .saturating_add(validator_removal_weight_for(u64::from(collaborators)))
    }

    fn bond() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::bond()

            .saturating_add(stake_refresh_weight(1))
    }
    fn bond_extra() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::bond_extra()

            .saturating_add(stake_refresh_weight(1))
    }
    fn unbond() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::unbond()

            .saturating_add(stake_refresh_weight(1))
    }
    fn withdraw_unbonded_update(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::withdraw_unbonded_update(s)

            .saturating_add(stake_refresh_weight(1))
    }
    fn withdraw_unbonded_kill(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::withdraw_unbonded_kill(s)

            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn validate() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::validate()

            .saturating_add(stake_refresh_weight(2))
    }
    fn kick(k: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::kick(k)

            .saturating_add(stake_refresh_weight(u64::from(k).saturating_add(1)))
    }
    fn cooperate(n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::cooperate(n)

            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn chill() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::chill()

            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn set_payee() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_payee()
    }
    fn set_controller() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_controller()
    }
    fn set_validator_count() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_validator_count()
    }
    fn force_no_eras() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_no_eras()
    }
    fn force_new_era() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_new_era()
    }
    fn force_new_era_always() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_new_era_always()
    }
    fn set_invulnerables(v: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_invulnerables(v)
    }
    fn force_unstake(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_unstake(s)

            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn cancel_deferred_slash(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::cancel_deferred_slash(s)
    }
    fn payout_stakers_dead_controller(n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::payout_stakers_dead_controller(n)
    }
    fn payout_stakers_alive_staked(n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::payout_stakers_alive_staked(n)
    }
    fn rebond(l: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::rebond(l)

            .saturating_add(stake_refresh_weight(1))
    }
    fn reap_stash(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::reap_stash(s)

            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn new_era(v: u32, n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::new_era(v,n)
    }
    fn get_npos_voters(v: u32, n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::get_npos_voters(v,n)
    }
    fn get_npos_targets(v: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::get_npos_targets(v)
    }
    fn set_staking_configs_all_set() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_staking_configs_all_set()
    }
    fn set_staking_configs_all_remove() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_staking_configs_all_remove()
    }
    fn chill_other() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::chill_other()

            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn force_apply_min_commission() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_apply_min_commission()
    }
    fn set_min_commission() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_min_commission()
    }
    fn make_collaborative() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::make_collaborative()
    }
    fn force_chill() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_chill()

            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
}

fn stake_refresh_weight(refreshes: u64) -> Weight {
    Weight::from_parts(16_000_000_000, 6_291_456)
        .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(272, 2))
        .saturating_add(Weight::from_parts(0, 16 * 1024))
        .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(8, 2))
        .saturating_mul(refreshes)
}

fn validator_removal_weight() -> Weight {
    validator_removal_weight_for(u64::from(pallet_energy_generation::MAX_VALIDATOR_COLLABORATORS))
}

fn validator_removal_weight_for(accounts: u64) -> Weight {
    Weight::from_parts(5_000_000, 32_768)
        .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(26, 4))
        .saturating_mul(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockWeights, RuntimeCall};
    use frame_support::dispatch::{DispatchClass, GetDispatchInfo};

    fn normal_limit() -> Weight {
        BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap()
    }

    #[test]
    fn staking_hook_uses_the_witnessed_collaboration_count() {
        use pallet_energy_generation::WeightInfo;
        let refresh = VipWeights::stake_refresh_hook();
        let small = VipWeights::chill_hook(10);
        let full = VipWeights::chill_hook(pallet_energy_generation::MAX_VALIDATOR_COLLABORATORS);
        assert!(full.ref_time() > small.ref_time());
        assert_eq!(full, VipWeights::chill());
        assert!(full.all_gte(refresh.saturating_mul(2)));
        assert!(full.ref_time() < normal_limit().ref_time());
        let double_chill = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![RuntimeCall::EnergyGeneration(pallet_energy_generation::Call::chill {}); 2],
        });
        assert!(double_chill.get_dispatch_info().weight.ref_time() > normal_limit().ref_time());
    }
}
