#![cfg_attr(not(feature = "std"), no_std)]

use pallet_reputation::ReputationTier;
use sp_runtime::Perbill;

sp_api::decl_runtime_apis! {
    pub trait EnergyGenerationApi
    {
        fn reputation_tier_additional_reward(tier: ReputationTier) -> Perbill;

        fn current_energy_per_stake_currency() -> u128;
    }
}
