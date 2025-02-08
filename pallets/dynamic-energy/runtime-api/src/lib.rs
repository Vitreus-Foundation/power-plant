#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::Decode;
use sp_runtime::FixedU128;

pub use pallet_dynamic_energy::{ExchangeRateParameters, GenerationRateParameters};

sp_api::decl_runtime_apis! {
    #[api_version(2)]
    pub trait DynamicEnergyApi<Balance>
    where
        Balance: Decode
    {
        /// Returns the VNRG/VTRS exchange rate.
        fn exchange_rate() -> Option<FixedU128>;

        /// Calculates the warehouse capacity multiplier.
        fn calculate_warehouse_capacity_multiplier() -> FixedU128;

        /// Returns the parameters used to calculate generation rate.
        fn generation_rate_parameters() -> GenerationRateParameters<Balance>;

        /// Returns the parameters used to calculate exchange rate.
        fn exchange_rate_parameters() -> ExchangeRateParameters<Balance>;
    }
}
