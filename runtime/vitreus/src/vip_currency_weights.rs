//! Native-currency observation costs for callers that bypass Balances dispatchables.
//!
//! Counts are upper bounds on AccountStore observations, not unique accounts/keys.
//! A Currency mutation can first upgrade an old balance account and then write it:
//! reserve/unreserve/lock/slash/deposit therefore allow two observations; transfer
//! or repatriation allows four. Runtime NFT item/collection deposits are zero and
//! DustRemoval is (), so the OnNewAccount NAC mint adds no recursive balance write.
//! Each decorator preserves the underlying pallet weight, including nested calls.
//! See docs/vip-vipp-currency-weights.md for source
//! paths, hook exclusions, and validation. These allowances require benchmark
//! calibration; source-count coverage does not certify existing SDK hook bounds.

use crate::Runtime;
use frame_support::{traits::Get, weights::Weight};

/// Cost of the bounded light observer, shared by all native-currency consumers.
/// Uses the common NAC envelope calibrated against the 2026-10-06 mainnet-WASM
/// cases, including full vesting/reserve vectors and both breach-queue branches.
pub(crate) fn observed_accounts<T: frame_system::Config>(observations: u64) -> Weight {
    <pallet_nac_managing::weights::SubstrateWeight<T> as pallet_nac_managing::WeightInfo>::balance_observer()
        .saturating_mul(observations)
}

pub struct VippCurrencyWeights;

macro_rules! currency_weights {
    ($trait:path, $base:ty; $(fn $name:ident($($arg:ident: u32),*) => $observations:expr;)+) => {
        impl $trait for VippCurrencyWeights {
            $(fn $name($($arg: u32),*) -> Weight {
                <$base as $trait>::$name($($arg),*)
                    .saturating_add(observed_accounts::<Runtime>($observations))
            })+
        }
    };
}

// Assets: native deposits/refunds only; asset amounts are not VTRS.
currency_weights!(pallet_assets::WeightInfo, pallet_assets::weights::SubstrateWeight<Runtime>;
    fn create() => 2;
    fn force_create() => 0;
    fn start_destroy() => 0;
    fn destroy_accounts(c: u32) => 2 * u64::from(c);
    fn destroy_approvals(a: u32) => 2 * u64::from(a);
    fn finish_destroy() => 2;
    fn mint() => 0;
    fn burn() => 0;
    fn transfer() => 0;
    fn transfer_keep_alive() => 0;
    fn force_transfer() => 0;
    fn freeze() => 0;
    fn thaw() => 0;
    fn freeze_asset() => 0;
    fn thaw_asset() => 0;
    fn transfer_ownership() => 4;
    fn set_team() => 0;
    fn set_metadata(n: u32, s: u32) => 2;
    fn clear_metadata() => 2;
    fn force_set_metadata(n: u32, s: u32) => 0;
    fn force_clear_metadata() => 2;
    fn force_asset_status() => 0;
    fn approve_transfer() => 2;
    fn transfer_approved() => 2;
    fn cancel_approval() => 2;
    fn force_cancel_approval() => 2;
    fn set_min_balance() => 0;
    fn touch() => 2;
    fn touch_other() => 2;
    fn refund() => 2;
    fn refund_other() => 2;
    fn block() => 0;
);

// NFTs: item/metadata/attribute deposits, price/tips, reserved-deposit repatriation.
currency_weights!(pallet_nfts::WeightInfo, pallet_nfts::weights::SubstrateWeight<Runtime>;
    fn create() => 2;
    fn force_create() => 0;
    fn destroy(m: u32, c: u32, a: u32) => 2 * (u64::from(m) + u64::from(a) + 1);
    fn mint() => 6;
    fn force_mint() => 2;
    fn burn() => 4;
    fn transfer() => 0;
    fn redeposit(i: u32) => 2 * u64::from(i);
    fn lock_item_transfer() => 0;
    fn unlock_item_transfer() => 0;
    fn lock_collection() => 0;
    fn transfer_ownership() => 4;
    fn set_team() => 0;
    fn force_collection_owner() => 4;
    fn force_collection_config() => 0;
    fn lock_item_properties() => 0;
    fn set_attribute() => 4;
    fn force_set_attribute() => 2;
    fn clear_attribute() => 2;
    fn approve_item_attributes() => 0;
    fn cancel_item_attributes_approval(n: u32) => 2;
    fn set_metadata() => 4;
    fn clear_metadata() => 2;
    fn set_collection_metadata() => 2;
    fn clear_collection_metadata() => 2;
    fn approve_transfer() => 0;
    fn cancel_approval() => 0;
    fn clear_all_transfer_approvals() => 0;
    fn set_accept_ownership() => 0;
    fn set_collection_max_supply() => 0;
    fn update_mint_settings() => 0;
    fn set_price() => 0;
    fn buy_item() => 4;
    fn pay_tips(n: u32) => 4 * u64::from(n);
    fn create_swap() => 0;
    fn cancel_swap() => 0;
    fn claim_swap() => 4;
    fn mint_pre_signed(n: u32) => 8 + 4 * u64::from(n);
    fn set_attributes_pre_signed(n: u32) => 4 * u64::from(n);
);

// Vesting: one lock write; vested transfers additionally mutate sender and recipient.
currency_weights!(pallet_vesting::WeightInfo, pallet_vesting::weights::SubstrateWeight<Runtime>;
    fn vest_locked(l: u32, s: u32) => 2;
    fn vest_unlocked(l: u32, s: u32) => 2;
    fn vest_other_locked(l: u32, s: u32) => 2;
    fn vest_other_unlocked(l: u32, s: u32) => 2;
    fn vested_transfer(l: u32, s: u32) => 6;
    fn force_vested_transfer(l: u32, s: u32) => 6;
    fn not_unlocking_merge_schedules(l: u32, s: u32) => 2;
    fn unlocking_merge_schedules(l: u32, s: u32) => 2;
    fn force_remove_vesting_schedule(l: u32, s: u32) => 2;
);

// Preimage: allow lazy legacy unreserve+hold, then one new/drop consideration.
currency_weights!(pallet_preimage::WeightInfo, pallet_preimage::weights::SubstrateWeight<Runtime>;
    fn note_preimage(s: u32) => 6;
    fn note_requested_preimage(s: u32) => 6;
    fn note_no_deposit_preimage(s: u32) => 6;
    fn unnote_preimage() => 6;
    fn unnote_no_deposit_preimage() => 6;
    fn request_preimage() => 6;
    fn request_no_deposit_preimage() => 6;
    fn request_unnoted_preimage() => 6;
    fn request_requested_preimage() => 6;
    fn unrequest_preimage() => 6;
    fn unrequest_unnoted_preimage() => 6;
    fn unrequest_multi_referenced_preimage() => 6;
    fn ensure_updated(n: u32) => 4 * u64::from(n);
);

// Multisig: deposit reserve/unreserve; nested dispatch retains its own weight.
currency_weights!(pallet_multisig::WeightInfo, pallet_multisig::weights::SubstrateWeight<Runtime>;
    fn as_multi_threshold_1(z: u32) => 0;
    fn as_multi_create(s: u32, z: u32) => 2;
    fn as_multi_approve(s: u32, z: u32) => 0;
    fn as_multi_complete(s: u32, z: u32) => 2;
    fn approve_as_multi_create(s: u32) => 2;
    fn approve_as_multi_approve(s: u32) => 0;
    fn cancel_as_multi(s: u32) => 2;
);

// Elections: locks + deposits; candidate slash also resolves imbalance to Treasury.
currency_weights!(pallet_elections_phragmen::WeightInfo, pallet_elections_phragmen::weights::SubstrateWeight<Runtime>;
    fn vote_equal(v: u32) => 4;
    fn vote_more(v: u32) => 4;
    fn vote_less(v: u32) => 4;
    fn remove_voter() => 4;
    fn submit_candidacy(c: u32) => 2;
    fn renounce_candidacy_candidate(c: u32) => 2;
    fn renounce_candidacy_members() => 2;
    fn renounce_candidacy_runners_up() => 2;
    fn remove_member_without_replacement() => 4;
    fn remove_member_with_replacement() => 4;
    fn clean_defunct_voters(v: u32, d: u32) => 4 * u64::from(v);
    fn election_phragmen(c: u32, v: u32, e: u32) => 4 * u64::from(c);
);

// Both treasuries: proposer refund + beneficiary deposit; settlement/burn destination. SpendFunds hooks charge separately.
currency_weights!(pallet_treasury::WeightInfo, pallet_treasury::weights::SubstrateWeight<Runtime>;
    fn spend_local() => 0;
    fn remove_approval() => 0;
    fn on_initialize_proposals(p: u32) => 4 * u64::from(p) + 4;
    fn spend() => 0;
    fn payout() => 4;
    fn check_status() => 0;
    fn void_spend() => 0;
);

// Bounties: curator/proposer deposits, slash destination, and two claim transfers.
currency_weights!(pallet_bounties::WeightInfo, pallet_bounties::weights::SubstrateWeight<Runtime>;
    fn propose_bounty(d: u32) => 2;
    fn approve_bounty() => 0;
    fn propose_curator() => 0;
    fn unassign_curator() => 4;
    fn accept_curator() => 2;
    fn award_bounty() => 0;
    fn claim_bounty() => 10;
    fn close_bounty_proposed() => 4;
    fn close_bounty_active() => 6;
    fn extend_bounty_expiry() => 0;
    fn spend_funds(b: u32) => 4 * u64::from(b);
);

// Democracy: native adapter locks/deposits/slashes; metadata len() lazily upgrades preimages.
// A successful launch/maturity uses the SDK full-block hook reservation, not these idle weights.
currency_weights!(pallet_democracy::WeightInfo, pallet_democracy::weights::SubstrateWeight<Runtime>;
    fn propose() => 2;
    fn second() => 2;
    fn vote_new() => 2;
    fn vote_existing() => 2;
    fn emergency_cancel() => 0;
    fn blacklist() => 4 * u64::from(<<Runtime as pallet_democracy::Config>::MaxDeposits as Get<u32>>::get());
    fn external_propose() => 0;
    fn external_propose_majority() => 0;
    fn external_propose_default() => 0;
    fn fast_track() => 0;
    fn veto_external() => 0;
    fn cancel_proposal() => 4 * u64::from(<<Runtime as pallet_democracy::Config>::MaxDeposits as Get<u32>>::get());
    fn cancel_referendum() => 0;
    fn on_initialize_base(r: u32) => 0;
    fn on_initialize_base_with_launch_period(r: u32) => 0;
    fn delegate(r: u32) => 2;
    fn undelegate(r: u32) => 0;
    fn clear_public_proposals() => 0;
    fn unlock_remove(r: u32) => 2;
    fn unlock_set(r: u32) => 2;
    fn remove_vote(r: u32) => 0;
    fn remove_other_vote(r: u32) => 0;
    fn set_external_metadata() => 4;
    fn clear_external_metadata() => 0;
    fn set_proposal_metadata() => 4;
    fn clear_proposal_metadata() => 0;
    fn set_referendum_metadata() => 4;
    fn clear_referendum_metadata() => 0;
);

// Proxy: reserve/unreserve proxy and announcement deposits; child dispatch has its own weight.
currency_weights!(pallet_proxy::WeightInfo, pallet_proxy::weights::SubstrateWeight<Runtime>;
    fn proxy(p: u32) => 0;
    fn proxy_announced(a: u32, p: u32) => 2;
    fn remove_announcement(a: u32, p: u32) => 2;
    fn reject_announcement(a: u32, p: u32) => 2;
    fn announce(a: u32, p: u32) => 2;
    fn add_proxy(p: u32) => 2;
    fn remove_proxy(p: u32) => 2;
    fn remove_proxies(p: u32) => 2;
    fn create_pure(p: u32) => 2;
    fn kill_pure(p: u32) => 2;
);

// Scheduler: preimage migration/consideration; matching base additions preserve marginal-weight subtraction.
currency_weights!(pallet_scheduler::WeightInfo, pallet_scheduler::weights::SubstrateWeight<Runtime>;
    fn service_agendas_base() => 0;
    fn service_agenda_base(s: u32) => 0;
    fn service_task_base() => 6;
    fn service_task_fetched(s: u32) => 6;
    fn service_task_named() => 6;
    fn service_task_periodic() => 6;
    fn execute_dispatch_signed() => 0;
    fn execute_dispatch_unsigned() => 0;
    fn schedule(s: u32) => 12;
    fn cancel(s: u32) => 6;
    fn schedule_named(s: u32) => 12;
    fn cancel_named(s: u32) => 6;
    fn schedule_retry(s: u32) => 0;
    fn set_retry() => 0;
    fn set_retry_named() => 0;
    fn cancel_retry() => 0;
    fn cancel_retry_named() => 0;
);

// EnergyGeneration: update_ledger lock, slash and Treasury resolution. Session/offence work has separate hook admission.
impl pallet_energy_generation::WeightInfo for VippCurrencyWeights {
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
            .saturating_add(observed_accounts::<Runtime>(2))
            .saturating_add(stake_refresh_weight(1))
    }
    fn bond_extra() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::bond_extra()
            .saturating_add(observed_accounts::<Runtime>(2))
            .saturating_add(stake_refresh_weight(1))
    }
    fn unbond() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::unbond()
            .saturating_add(observed_accounts::<Runtime>(6))
            .saturating_add(stake_refresh_weight(1))
    }
    fn withdraw_unbonded_update(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::withdraw_unbonded_update(s)
            .saturating_add(observed_accounts::<Runtime>(2))
            .saturating_add(stake_refresh_weight(1))
    }
    fn withdraw_unbonded_kill(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::withdraw_unbonded_kill(s)
            .saturating_add(observed_accounts::<Runtime>(2))
            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn validate() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::validate()
            .saturating_add(observed_accounts::<Runtime>(0))
            .saturating_add(stake_refresh_weight(2))
    }
    fn kick(k: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::kick(k)
            .saturating_add(observed_accounts::<Runtime>(0))
            .saturating_add(stake_refresh_weight(u64::from(k).saturating_add(1)))
    }
    fn cooperate(n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::cooperate(n)
            .saturating_add(observed_accounts::<Runtime>(0))
            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn chill() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::chill()
            .saturating_add(observed_accounts::<Runtime>(0))
            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn set_payee() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_payee()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn set_controller() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_controller()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn set_validator_count() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_validator_count()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn force_no_eras() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_no_eras()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn force_new_era() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_new_era()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn force_new_era_always() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_new_era_always()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn set_invulnerables(v: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_invulnerables(v)
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn force_unstake(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_unstake(s)
            .saturating_add(observed_accounts::<Runtime>(2))
            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn cancel_deferred_slash(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::cancel_deferred_slash(s)
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn payout_stakers_dead_controller(n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::payout_stakers_dead_controller(n)
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn payout_stakers_alive_staked(n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::payout_stakers_alive_staked(n)
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn rebond(l: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::rebond(l)
            .saturating_add(observed_accounts::<Runtime>(2))
            .saturating_add(stake_refresh_weight(1))
    }
    fn reap_stash(s: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::reap_stash(s)
            .saturating_add(observed_accounts::<Runtime>(2))
            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn new_era(v: u32, n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::new_era(v,n)
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn get_npos_voters(v: u32, n: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::get_npos_voters(v,n)
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn get_npos_targets(v: u32) -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::get_npos_targets(v)
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn set_staking_configs_all_set() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_staking_configs_all_set()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn set_staking_configs_all_remove() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_staking_configs_all_remove()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn chill_other() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::chill_other()
            .saturating_add(observed_accounts::<Runtime>(0))
            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
    fn force_apply_min_commission() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_apply_min_commission()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn set_min_commission() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::set_min_commission()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn make_collaborative() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::make_collaborative()
            .saturating_add(observed_accounts::<Runtime>(0))
    }
    fn force_chill() -> Weight {
        <pallet_energy_generation::weights::SubstrateWeight<Runtime> as pallet_energy_generation::WeightInfo>::force_chill()
            .saturating_add(observed_accounts::<Runtime>(0))
            .saturating_add(stake_refresh_weight(2).saturating_add(validator_removal_weight()))
    }
}

// TreasuryExtension spend_funds resolves native recycling credit. Session swaps charge separately.
currency_weights!(pallet_treasury_extension::WeightInfo, pallet_treasury_extension::weights::SubstrateWeight<Runtime>;
    fn spend_funds() => 2;
);

/// Used by local pallets with inline weight declarations and by slash hooks.
pub struct NativeCurrencyObserverWeight;
impl Get<Weight> for NativeCurrencyObserverWeight {
    fn get() -> Weight {
        observed_accounts::<Runtime>(1)
    }
}

/// One broker swap: native sender withdrawal and two native credit resolutions.
pub struct EnergyExchangeWeight;
impl Get<Weight> for EnergyExchangeWeight {
    fn get() -> Weight {
        Weight::from_parts(200_000_000, 20_000).saturating_add(observed_accounts::<Runtime>(6))
    }
}

/// One outer fee charge: optional native swap (6) and the explicit NAC fee
/// callback (1). Asset deposits are not refunded by ordinary energy transfers.
/// Keep this in the base extrinsic weight so post-dispatch call refunds cannot
/// refund the work done by the transaction-payment extension.
pub(crate) fn fee_observation_weight() -> Weight {
    Weight::from_parts(200_000_000, 20_000).saturating_add(observed_accounts::<Runtime>(7))
}

// Registrar: reserve para id, adjust deposit, and refund on deregistration.
currency_weights!(polkadot_runtime_common::paras_registrar::WeightInfo, crate::weights::runtime_common_paras_registrar::WeightInfo<Runtime>;
    fn reserve() => 2;
    fn register() => 2;
    fn force_register() => 4;
    fn deregister() => 2;
    fn swap() => 0;
    fn schedule_code_upgrade(b: u32) => 0;
    fn set_current_head(b: u32) => 0;
);

// HRMP: channel reserve/refund; removal refunds both channel ends. Open processing onlymovesmetadata.
currency_weights!(polkadot_runtime_parachains::hrmp::WeightInfo, crate::weights::runtime_parachains_hrmp::WeightInfo<Runtime>;
    fn hrmp_init_open_channel() => 2;
    fn hrmp_accept_open_channel() => 2;
    fn hrmp_close_channel() => 0;
    fn force_clean_hrmp(i: u32, e: u32) => 4 * (u64::from(i) + u64::from(e));
    fn force_process_hrmp_open(c: u32) => 0;
    fn force_process_hrmp_close(c: u32) => 4 * u64::from(c);
    fn hrmp_cancel_open_request(c: u32) => 2;
    fn clean_open_channel_requests(c: u32) => 4 * u64::from(c);
    fn force_open_hrmp_channel(c: u32) => 6;
    fn establish_system_channel() => 0;
    fn poke_channel_deposits() => 4;
    fn establish_channel_with_system() => 4;
);

impl pallet_claiming::weights::WeightInfo for VippCurrencyWeights {
    fn mint_tokens_to_claim() -> Weight {
        <pallet_claiming::weights::SubstrateWeight<Runtime> as pallet_claiming::weights::WeightInfo>::mint_tokens_to_claim()
            .saturating_add(observed_accounts::<Runtime>(2))
    }
    fn claim() -> Weight {
        // Native transfer (4) + vesting lock (2). Both claiming instances use this
        // upper envelope: ordinary claims run the full bounded NAC callback;
        // Kickstart claims can mint one NFT and set one attribute.
        <pallet_claiming::weights::SubstrateWeight<Runtime> as pallet_claiming::weights::WeightInfo>::claim()
            .saturating_add(observed_accounts::<Runtime>(6))
            .saturating_add(<pallet_nac_managing::weights::SubstrateWeight<Runtime> as pallet_nac_managing::WeightInfo>::transfer_vipp(
                <Runtime as pallet_nac_managing::Config>::MaxVippNftsPerAccount::get(),
            ))
            .saturating_add(<Self as pallet_nfts::WeightInfo>::force_mint())
            .saturating_add(<Self as pallet_nfts::WeightInfo>::force_set_attribute().saturating_mul(2))
    }
    fn mint_claim() -> Weight {
        <pallet_claiming::weights::SubstrateWeight<Runtime> as pallet_claiming::weights::WeightInfo>::mint_claim()
    }
}

#[cfg(feature = "testnet-runtime")]
impl pallet_faucet::WeightInfo for VippCurrencyWeights {
    fn request_funds() -> Weight {
        <pallet_faucet::weights::SubstrateWeight<Runtime> as pallet_faucet::WeightInfo>::request_funds()
            .saturating_add(observed_accounts::<Runtime>(2))
    }
}

/// Both StaticEnergy -> native and LiquidEnergy -> native swaps run on a session.
pub struct TreasuryEnergyExchangesWeight;
impl Get<Weight> for TreasuryEnergyExchangesWeight {
    fn get() -> Weight {
        EnergyExchangeWeight::get().saturating_mul(2)
    }
}

/// Preserve SDK block limits and base execution cost, reserving outer fee work
/// before dispatch. Adjust the single-call maximum by the same allowance.
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

/// The Native and WrappedToken transactors both use VTRS balances. A native
/// transfer allows four observations. A wrapped transfer falls back to withdraw
/// (2), withdrawal-fee transfer (4), and two deposit credits (4): ten more.
/// These are the only two asset IDs matching a native Currency transactor;
/// ordinary energy transfers do not refund their native asset-account deposits.
/// Teleport check-account writes are cheaper than this combined transfer bound.
/// Transact additionally carries the dispatched RuntimeCall weight.
pub(crate) fn xcm_observation_weight() -> Weight {
    observed_accounts::<Runtime>(14)
}

/// Measured refresh of all 256 targets with 1024 collaborators each. The benchmark
/// runs after the timestamp. Initialization can additionally create the first
/// PreTimestampStake snapshot and mark a stale date for review: reserve their
/// reads, two writes and proof paths as well, without reducing the measured cost.
fn stake_refresh_weight(refreshes: u64) -> Weight {
    <<Runtime as pallet_privileges::Config>::WeightInfo as pallet_privileges::WeightInfo>::refresh_active_stake()
        .saturating_add(Weight::from_parts(0, 16 * 1024))
        .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(8, 2))
        .saturating_mul(refreshes)
}

/// Each removed delegation uses the stored target amount, without rescanning all
/// validator sets. Include the outer cooperation-target deletion as well as the
/// bounded before-timestamp snapshot and VIP cache callback per collaborator.
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

    fn nft_purchase() -> RuntimeCall {
        RuntimeCall::Nfts(pallet_nfts::Call::buy_item { collection: 1, item: 1, bid_price: 1 })
    }

    #[test]
    fn all_utility_batch_modes_reserve_direct_currency_work_before_execution() {
        let cases = [
            RuntimeCall::SimpleVesting(pallet_simple_vesting::Call::vest {}),
            RuntimeCall::ManualBridge(pallet_manual_bridge::Call::transfer {
                dest: pallet_manual_bridge::EthereumAddress([8; 20]),
                amount: 1,
            }),
            RuntimeCall::EnergyBroker(pallet_energy_broker::Call::swap_exact_tokens_for_tokens {
                recipient: AccountId::from([8; 20]),
                path: (crate::NativeOrAssetId::Native, crate::NativeOrAssetId::WithId(0)),
                amount_in: 1,
                amount_out_min: None,
                keep_alive: true,
            }),
            RuntimeCall::Democracy(pallet_democracy::Call::second { proposal: 1 }),
            RuntimeCall::Proxy(pallet_proxy::Call::add_proxy {
                delegate: AccountId::from([7; 20]),
                proxy_type: crate::ProxyType::Any,
                delay: 0,
            }),
            nft_purchase(),
        ];
        for call in cases {
            let calls = vec![call; 1000];
            let batches = [
                RuntimeCall::Utility(pallet_utility::Call::batch { calls: calls.clone() }),
                RuntimeCall::Utility(pallet_utility::Call::batch_all { calls: calls.clone() }),
                RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }),
            ];
            for batch in batches {
                assert!(batch.get_dispatch_info().weight.ref_time() > normal_limit().ref_time());
            }
        }
    }

    #[test]
    fn repeated_nft_purchases_are_not_admitted_at_the_old_sdk_price() {
        let old =
            <pallet_nfts::weights::SubstrateWeight<Runtime> as pallet_nfts::WeightInfo>::buy_item();
        assert!(old.saturating_mul(1000).ref_time() < normal_limit().ref_time());
        let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![nft_purchase(); 1000],
        });
        let storage = frame_system::GenesisConfig::<Runtime>::default().build_storage().unwrap();
        sp_io::TestExternalities::new(storage).execute_with(|| {
            // Admission rejects the work, before any potentially rolled-back currency writes.
            assert!(frame_system::CheckWeight::<Runtime>::do_pre_dispatch(
                &batch.get_dispatch_info(),
                0,
            )
            .is_err());
        });
    }

    #[test]
    fn nesting_through_proxy_and_derivative_preserves_currency_weight() {
        let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![nft_purchase(); 1000],
        });
        let derivative = RuntimeCall::Utility(pallet_utility::Call::as_derivative {
            index: 0,
            call: Box::new(batch),
        });
        let proxy = RuntimeCall::Proxy(pallet_proxy::Call::proxy {
            real: AccountId::from([7; 20]),
            force_proxy_type: None,
            call: Box::new(derivative),
        });
        assert!(proxy.get_dispatch_info().weight.ref_time() > normal_limit().ref_time());
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
        let refresh = VippCurrencyWeights::stake_refresh_hook();
        let measured = <<Runtime as pallet_privileges::Config>::WeightInfo as pallet_privileges::WeightInfo>::refresh_active_stake();
        let early_snapshot = Weight::from_parts(0, 16 * 1024)
            .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(8, 2));
        assert_eq!(refresh, measured.saturating_add(early_snapshot));
        let small = VippCurrencyWeights::chill_hook(10);
        let full =
            VippCurrencyWeights::chill_hook(pallet_energy_generation::MAX_VALIDATOR_COLLABORATORS);
        assert!(full.ref_time() > small.ref_time());
        assert_eq!(full, VippCurrencyWeights::chill());
        assert!(full.all_gte(refresh.saturating_mul(2)));
        assert!(full.ref_time() < normal_limit().ref_time());
        let double_chill = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![RuntimeCall::EnergyGeneration(pallet_energy_generation::Call::chill {}); 2],
        });
        assert!(double_chill.get_dispatch_info().weight.ref_time() > normal_limit().ref_time());
    }

    #[test]
    fn erc20_native_transfer_uses_the_active_stack_runner_and_block_gas_limit() {
        assert_eq!(
            core::any::TypeId::of::<<Runtime as pallet_evm::Config>::Runner>(),
            core::any::TypeId::of::<pallet_evm::runner::stack::Runner<Runtime>>(),
        );
        let native_weight = <<Runtime as pallet_balances::Config>::WeightInfo as pallet_balances::WeightInfo>::transfer_allow_death();
        let gas = <Runtime as pallet_evm::Config>::GasWeightMapping::weight_to_gas(native_weight);
        let limit = <Runtime as pallet_evm::Config>::BlockGasLimit::get().as_u64();
        assert!(gas < limit);
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

    #[test]
    fn atomic_swap_mutations_reserve_observer_work_in_every_batch_mode() {
        let action = pallet_atomic_swap::BalanceSwapAction::<AccountId, crate::Balances>::new(1);
        let calls = [
            RuntimeCall::AtomicSwap(pallet_atomic_swap::Call::create_swap {
                target: AccountId::from([8; 20]),
                hashed_proof: [7; 32],
                action: action.clone(),
                duration: 10,
            }),
            RuntimeCall::AtomicSwap(pallet_atomic_swap::Call::claim_swap {
                proof: vec![7; 32],
                action,
            }),
            RuntimeCall::AtomicSwap(pallet_atomic_swap::Call::cancel_swap {
                target: AccountId::from([8; 20]),
                hashed_proof: [7; 32],
            }),
        ];
        for call in calls {
            let repeated = vec![call; 1000];
            let batches = [
                RuntimeCall::Utility(pallet_utility::Call::batch { calls: repeated.clone() }),
                RuntimeCall::Utility(pallet_utility::Call::batch_all { calls: repeated.clone() }),
                RuntimeCall::Utility(pallet_utility::Call::force_batch { calls: repeated }),
            ];
            for batch in batches {
                assert!(batch.get_dispatch_info().weight.ref_time() > normal_limit().ref_time());
            }
        }
    }

    #[test]
    fn both_xcm_weighers_include_native_and_asset_deposit_observation() {
        use xcm::latest::{Asset, Instruction, Location, Xcm};
        use xcm_executor::traits::WeightBounds;
        let native = Asset::from((Location::here(), 1u128));
        let mut message = Xcm::<RuntimeCall>(vec![Instruction::WithdrawAsset(native.into())]);
        let execution =
            <<crate::xcm_config::XcmConfig as xcm_executor::Config>::Weigher as WeightBounds<
                RuntimeCall,
            >>::weight(&mut message)
            .unwrap();
        let local =
            <<Runtime as pallet_xcm::Config>::Weigher as WeightBounds<RuntimeCall>>::weight(
                &mut message,
            )
            .unwrap();
        assert!(execution.all_gte(xcm_observation_weight()));
        assert!(local.all_gte(xcm_observation_weight()));
    }
}
