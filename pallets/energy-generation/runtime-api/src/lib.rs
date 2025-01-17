#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::Encode;
use sp_runtime::{FixedU128, FixedU64};

sp_api::decl_runtime_apis! {
    #[api_version(2)]
    pub trait EnergyGenerationApi<AccountId>
    where
        AccountId: Encode
    {
        /// Returns the energy reward per stake for the most recent era.
        fn energy_reward_per_stake() -> FixedU128;

        /// Returns the exposure multiplier for a validator's role.
        fn validator_exposure_multiplier(account: AccountId) -> FixedU64;

        /// Returns the exposure multiplier for a cooperator's role.
        fn cooperator_exposure_multiplier(account: AccountId) -> FixedU64;
    }
}
