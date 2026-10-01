#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::{Decode, Encode};
use sp_runtime::{FixedU128, Percent, Vec};

sp_api::decl_runtime_apis! {
    #[api_version(4)]
    pub trait EnergyBrokerApi<AccountId, AssetKind, Balance>
    where
        AccountId: Encode,
        AssetKind: Decode + Encode,
        Balance: Decode + Encode,
    {
        /// Estimates energy received from a given amount of native currency.
        fn estimate_energy_from_native(amount: Balance) -> Option<Balance>;

        /// Estimates native currency received from a given amount of energy.
        fn estimate_native_from_energy(amount: Balance) -> Option<Balance>;

        /// Quotes the amount of `asset2` resulting from swapping the exact `amount` of `asset1`.
        ///
        /// Does not account for available liquidity. The most `asset2` the path can supply is
        /// [`Self::max_amount_out`].
        fn quote_price_exact_tokens_for_tokens(
            who: Option<AccountId>,
            asset1: AssetKind,
            asset2: AssetKind,
            amount: Balance,
            include_fee: bool,
        ) -> Option<Balance>;

        /// Quotes the amount of `asset1` required to obtain the exact `amount` of `asset2`.
        ///
        /// Does not account for available liquidity. The most `asset2` the path can supply is
        /// [`Self::max_amount_out`].
        fn quote_price_tokens_for_exact_tokens(
            who: Option<AccountId>,
            asset1: AssetKind,
            asset2: AssetKind,
            amount: Balance,
            include_fee: bool,
        ) -> Option<Balance>;

        /// Returns the current exchange rate between energy and native currency.
        fn energy_exchange_rate() -> Option<FixedU128>;

        /// Returns the current warehouse fill level as a percentage.
        fn current_warehouse_level() -> Percent;

        /// Returns the supported swap paths.
        fn paths() -> Vec<(AssetKind, AssetKind)>;

        /// The maximum amount of `asset2` the path `(asset1, asset2)` can currently supply.
        ///
        /// A path not in [`Self::paths`] returns zero. An unbounded path returns
        /// `Balance::max_value()`.
        fn max_amount_out(asset1: AssetKind, asset2: AssetKind) -> Balance;
    }
}
