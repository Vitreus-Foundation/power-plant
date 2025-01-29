#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::{Decode, Encode};
use sp_runtime::{FixedU128, Percent};

sp_api::decl_runtime_apis! {
    #[api_version(1)]
    pub trait EnergyBrokerApi<Balance>
    where
        Balance: Decode + Encode,
    {
        /// Estimates energy received from a given amount of native currency.
        fn estimate_energy_from_native(amount: Balance) -> Option<Balance>;

        /// Estimates native currency received from a given amount of energy.
        fn estimate_native_from_energy(amount: Balance) -> Option<Balance>;

        /// Returns the current exchange rate between energy and native currency.
        fn energy_exchange_rate() -> Option<FixedU128>;

        /// Returns the current warehouse fill level as a percentage.
        fn current_warehouse_level() -> Percent;
    }
}
