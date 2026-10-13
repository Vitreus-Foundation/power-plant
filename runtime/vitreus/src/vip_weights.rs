//! Additional weight of VIP/VIPP staking, claim and existing fee hooks.
//! No global native AccountStore observer is installed by this focused patch.

use crate::Runtime;
use frame_support::{traits::Get, weights::Weight};

pub struct VipWeights;

/// One fee withdrawal: existing optional native-to-energy swap plus one bounded
/// NAC threshold check. Native balance writes do not invoke additional observers.
pub(crate) fn fee_observation_weight() -> Weight {
    Weight::from_parts(200_000_000, 20_000).saturating_add(
        <pallet_nac_managing::weights::SubstrateWeight<Runtime> as pallet_nac_managing::WeightInfo>::balance_observer(),
    )
}

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

impl pallet_claiming::weights::WeightInfo for VipWeights {
    fn mint_tokens_to_claim() -> Weight {
        <pallet_claiming::weights::SubstrateWeight<Runtime> as pallet_claiming::weights::WeightInfo>::mint_tokens_to_claim()
    }
    fn claim() -> Weight {
        // Both claiming instances use this upper envelope: ordinary claims run the full bounded NAC callback;
        // Kickstart claims can mint one NFT and set one attribute.
        <pallet_claiming::weights::SubstrateWeight<Runtime> as pallet_claiming::weights::WeightInfo>::claim()

            .saturating_add(<pallet_nac_managing::weights::SubstrateWeight<Runtime> as pallet_nac_managing::WeightInfo>::transfer_vipp(
                <Runtime as pallet_nac_managing::Config>::MaxVippNftsPerAccount::get(),
            ))
            .saturating_add(<pallet_nfts::weights::SubstrateWeight<Runtime> as pallet_nfts::WeightInfo>::force_mint())
            .saturating_add(<pallet_nfts::weights::SubstrateWeight<Runtime> as pallet_nfts::WeightInfo>::force_set_attribute().saturating_mul(2))
    }
    fn mint_claim() -> Weight {
        <pallet_claiming::weights::SubstrateWeight<Runtime> as pallet_claiming::weights::WeightInfo>::mint_claim()
    }
}

pub(crate) fn block_weights() -> frame_system::limits::BlockWeights {
    use frame_support::dispatch::DispatchClass;
    let mut weights = frame_system::limits::BlockWeights::with_sensible_defaults(
        crate::MAXIMUM_BLOCK_WEIGHT,
        crate::NORMAL_DISPATCH_RATIO,
    );
    let fee_weight = fee_observation_weight();
    for class in DispatchClass::all() {
        let per_class = weights.per_class.get_mut(*class);
        per_class.base_extrinsic = per_class.base_extrinsic.saturating_add(fee_weight);
        per_class.max_extrinsic =
            per_class.max_extrinsic.map(|limit| limit.saturating_sub(fee_weight));
    }
    weights
}

fn stake_refresh_weight(refreshes: u64) -> Weight {
    <<Runtime as pallet_privileges::Config>::WeightInfo as pallet_privileges::WeightInfo>::refresh_active_stake()
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

/// Preserve the configured gas conversion and include fee-observer work for each
/// EVM execution, including calls nested inside utility. The SDK mapping subtracts
/// base_extrinsic; remove only its original SDK portion so our outer fee reserve
/// is not cancelled. The reverse conversion is unchanged (precompile gas pricing).
pub struct VippGasWeightMapping;
impl pallet_evm::GasWeightMapping for VippGasWeightMapping {
    fn gas_to_weight(gas: u64, without_base_weight: bool) -> Weight {
        let mut weight = pallet_evm::FixedGasWeightMapping::<Runtime>::gas_to_weight(gas, false);
        let fee = fee_observation_weight();
        if without_base_weight {
            let original_base = crate::BlockWeights::get()
                .get(frame_support::dispatch::DispatchClass::Normal)
                .base_extrinsic
                .saturating_sub(fee);
            weight = weight.saturating_sub(original_base);
        }
        weight.saturating_add(fee)
    }

    fn weight_to_gas(weight: Weight) -> u64 {
        pallet_evm::FixedGasWeightMapping::<Runtime>::weight_to_gas(weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountId, BlockWeights, RuntimeCall, System};
    use frame_support::{
        dispatch::{DispatchClass, DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo},
        weights::Weight,
    };
    use pallet_evm::GasWeightMapping;
    use sp_runtime::{traits::SignedExtension, BuildStorage};

    fn normal_limit() -> Weight {
        BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap()
    }

    #[test]
    fn free_or_refunded_calls_keep_the_outer_fee_observation_reserve() {
        let storage = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
        sp_io::TestExternalities::new(storage).execute_with(|| {
            let info = DispatchInfo {
                weight: Weight::from_parts(1_000_000, 0),
                class: DispatchClass::Normal,
                pays_fee: Pays::No,
            };
            frame_system::CheckWeight::<Runtime>::do_pre_dispatch(&info, 0).unwrap();
            let post = PostDispatchInfo { actual_weight: Some(Weight::zero()), pays_fee: Pays::No };
            <frame_system::CheckWeight<Runtime> as SignedExtension>::post_dispatch(
                Some(()),
                &info,
                &post,
                0,
                &Ok(()),
            )
            .unwrap();
            let consumed = *System::block_weight().get(DispatchClass::Normal);
            assert_eq!(consumed, BlockWeights::get().get(DispatchClass::Normal).base_extrinsic);
            assert!(consumed.all_gte(fee_observation_weight()));
        });
    }

    #[test]
    fn fee_reserve_preserves_old_block_ceiling_and_single_call_admission() {
        let old = frame_system::limits::BlockWeights::with_sensible_defaults(
            crate::MAXIMUM_BLOCK_WEIGHT,
            crate::NORMAL_DISPATCH_RATIO,
        );
        let new = BlockWeights::get();
        assert_eq!(new.max_block, old.max_block);
        assert_eq!(new.base_block, old.base_block);
        for class in DispatchClass::non_mandatory() {
            let (old, new) = (old.get(*class), new.get(*class));
            assert_eq!(new.max_total, old.max_total);
            assert_eq!(new.reserved, old.reserved);
            assert_eq!(
                new.base_extrinsic,
                old.base_extrinsic.saturating_add(fee_observation_weight())
            );
            assert_eq!(
                new.max_extrinsic.unwrap().saturating_add(new.base_extrinsic),
                old.max_extrinsic.unwrap().saturating_add(old.base_extrinsic),
            );
        }
        assert!(new.validate().is_ok());
    }

    #[test]
    fn staking_hook_uses_the_witnessed_collaboration_count() {
        use pallet_energy_generation::WeightInfo;
        let refresh = VipWeights::stake_refresh_hook();
        let measured = <<Runtime as pallet_privileges::Config>::WeightInfo as pallet_privileges::WeightInfo>::refresh_active_stake();
        let early_snapshot = Weight::from_parts(0, 16 * 1024)
            .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(8, 2));
        assert_eq!(refresh, measured.saturating_add(early_snapshot));
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

    #[test]
    fn evm_mapping_preserves_linear_gas_cost_and_reserves_each_inner_fee() {
        let original = frame_system::limits::BlockWeights::with_sensible_defaults(
            crate::MAXIMUM_BLOCK_WEIGHT,
            crate::NORMAL_DISPATCH_RATIO,
        )
        .get(DispatchClass::Normal)
        .base_extrinsic;
        for gas in [0, 21_000, 100_000, 1_000_000] {
            let sdk = pallet_evm::FixedGasWeightMapping::<Runtime>::gas_to_weight(gas, false);
            assert_eq!(
                VippGasWeightMapping::gas_to_weight(gas, false),
                sdk.saturating_add(fee_observation_weight())
            );
            assert_eq!(
                VippGasWeightMapping::gas_to_weight(gas, true),
                sdk.saturating_sub(original).saturating_add(fee_observation_weight())
            );
            assert_eq!(
                VippGasWeightMapping::weight_to_gas(sdk),
                pallet_evm::FixedGasWeightMapping::<Runtime>::weight_to_gas(sdk)
            );
        }
        let call = RuntimeCall::EVM(pallet_evm::Call::call {
            source: sp_core::H160::repeat_byte(1),
            target: sp_core::H160::repeat_byte(2),
            input: vec![],
            value: 0.into(),
            gas_limit: 21_000,
            max_fee_per_gas: 1.into(),
            max_priority_fee_per_gas: None,
            nonce: None,
            access_list: vec![],
        });
        let batch =
            RuntimeCall::Utility(pallet_utility::Call::force_batch { calls: vec![call; 1000] });
        assert!(batch.get_dispatch_info().weight.ref_time() > normal_limit().ref_time());
    }

    #[test]
    fn failed_evm_runner_keeps_the_fee_work_in_actual_weight() {
        use pallet_evm::Runner;
        let storage = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
        sp_io::TestExternalities::new(storage).execute_with(|| {
            // No energy assets or native liquidity: fee preparation fails. The
            // same error path is used if conversion performs work before failing.
            let result = <Runtime as pallet_evm::Config>::Runner::call(
                sp_core::H160::repeat_byte(1),
                sp_core::H160::repeat_byte(2),
                vec![],
                0.into(),
                21_000,
                Some(1.into()),
                None,
                None,
                vec![],
                true,
                false,
                None,
                None,
                <Runtime as pallet_evm::Config>::config(),
            );
            let error = result.expect_err("fee cannot be funded in an empty state");
            assert!(error.weight.all_gte(fee_observation_weight()));
        });
    }

    #[test]
    fn evm_and_ethereum_native_fee_conversion_preserve_the_post_dispatch_reserve() {
        use frame_support::traits::fungible::{Inspect, Mutate};
        use sp_runtime::traits::One;
        let source = AccountId::from([1; 20]);
        let mut storage =
            frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (source, 1_000_000_000_000_000_000),
                (crate::EnergyBroker::account_id(), 1_000_000_000_000_000_000),
                (crate::Treasury::account_id(), 1_000_000_000_000_000_000),
            ],
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        pallet_assets::GenesisConfig::<Runtime> {
            assets: vec![(0, source, true, 1)],
            accounts: vec![
                (0, source, 1_000_000),
                (0, crate::EnergyBroker::account_id(), 10_000_000_000),
            ],
            ..Default::default()
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        sp_io::TestExternalities::new(storage).execute_with(|| {
            System::set_block_number(1);
            pallet_dynamic_energy::ExchangeRate::<Runtime>::put(sp_runtime::FixedU128::one());
            let before = crate::Balances::balance(&source);
            // Validation sees sufficient gas-price energy, but the fixed energy
            // fee is larger. It must buy the difference using real native VTRS.
            let post = crate::EVM::call(
                crate::RuntimeOrigin::signed(source),
                source.into(),
                sp_core::H160::repeat_byte(2),
                vec![],
                0.into(),
                21_000,
                1.into(),
                None,
                None,
                vec![],
            )
            .expect("the configured broker funds the fixed energy fee");
            assert!(crate::Balances::balance(&source) < before);
            assert_eq!(post.pays_fee, Pays::No);
            assert_eq!(
                post.actual_weight.unwrap().ref_time(),
                VippGasWeightMapping::gas_to_weight(21_000, true).ref_time(),
            );
            assert!(post.actual_weight.unwrap().ref_time() >= fee_observation_weight().ref_time());

            // Ethereum's already-validated dispatch path has a separate
            // post-dispatch implementation; verify its actual refund too.
            crate::EnergyAsset::mint_into(&source, 1_000_000).unwrap();
            let transaction = ethereum::TransactionV2::Legacy(ethereum::LegacyTransaction {
                nonce: 1.into(),
                gas_price: 1.into(),
                gas_limit: 21_000.into(),
                action: ethereum::TransactionAction::Call(sp_core::H160::repeat_byte(2)),
                value: 0.into(),
                input: vec![],
                signature: ethereum::TransactionSignature::new(
                    27,
                    sp_core::H256::repeat_byte(1),
                    sp_core::H256::repeat_byte(2),
                )
                .unwrap(),
            });
            let call = RuntimeCall::Ethereum(pallet_ethereum::Call::transact {
                transaction: transaction.clone(),
            });
            assert_eq!(
                call.get_dispatch_info().weight,
                VippGasWeightMapping::gas_to_weight(21_000, true),
            );
            let before = crate::Balances::balance(&source);
            let post = crate::Ethereum::transact(
                pallet_ethereum::RawOrigin::EthereumTransaction(source.into()).into(),
                transaction,
            )
            .expect("already-validated Ethereum dispatch funds the fixed fee via the broker");
            assert!(crate::Balances::balance(&source) < before);
            assert_eq!(post.pays_fee, Pays::No);
            assert_eq!(
                post.actual_weight.unwrap().ref_time(),
                VippGasWeightMapping::gas_to_weight(21_000, true).ref_time(),
            );
        });
    }
}
