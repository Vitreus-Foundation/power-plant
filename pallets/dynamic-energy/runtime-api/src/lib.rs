#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::FixedU128;

sp_api::decl_runtime_apis! {
    #[api_version(1)]
    pub trait DynamicEnergyApi {
        /// Returns the VNRG/VTRS exchange rate.
        fn exchange_rate() -> Option<FixedU128>;

        /// Calculates the warehouse capacity multiplier.
        fn calculate_warehouse_capacity_multiplier() -> FixedU128;
    }
}
