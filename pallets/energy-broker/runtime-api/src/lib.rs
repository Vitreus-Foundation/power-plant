#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::{Decode, Encode};
use sp_runtime::{FixedU128, Percent};

sp_api::decl_runtime_apis! {
    #[api_version(1)]
    pub trait EnergyBrokerApi<Balance>
    where
        Balance: Decode + Encode,
    {
        fn estimate_energy_from_native(amount: Balance) -> Option<Balance>;

        fn estimate_native_from_energy(amount: Balance) -> Option<Balance>;

        fn energy_exchange_rate() -> Option<FixedU128>;

        fn current_warehouse_level() -> Percent;
    }
}
