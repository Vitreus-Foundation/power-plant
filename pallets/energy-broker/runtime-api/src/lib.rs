#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::{Decode, Encode};
use sp_runtime::{FixedU128, Percent, Vec};

sp_api::decl_runtime_apis! {
    #[api_version(5)]
    pub trait EnergyBrokerApi<AccountId, AssetKind, Balance>
    where
        AccountId: Encode,
        AssetKind: Decode + Encode,
        Balance: Decode + Encode,
    {
        /// Estimates energy received from a given amount of native currency.
        ///
        /// Does not account for available liquidity; the limit is [`Self::max_amount_out`].
        fn estimate_energy_from_native(amount: Balance) -> Option<Balance>;

        /// Estimates native currency received from a given amount of energy.
        ///
        /// Does not account for available liquidity; the limit is [`Self::max_amount_out`].
        fn estimate_native_from_energy(amount: Balance) -> Option<Balance>;

        /// Quotes the amount of `asset2` resulting from swapping the exact `amount` of `asset1`.
        ///
        /// Returns `None` when the quoted `asset2` exceeds [`Self::max_amount_out`].
        fn quote_price_exact_tokens_for_tokens(
            who: Option<AccountId>,
            asset1: AssetKind,
            asset2: AssetKind,
            amount: Balance,
            include_fee: bool,
        ) -> Option<Balance>;

        /// Quotes the amount of `asset1` required to obtain the exact `amount` of `asset2`.
        ///
        /// Returns `None` when `amount` exceeds [`Self::max_amount_out`].
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

        /// The maximum amount of `asset2` a swap from `asset1` can currently supply.
        ///
        /// `None` if the swap can't be priced, a path not in [`Self::paths`] included. Otherwise
        /// zero if nothing can be supplied and `Balance::max_value()` if the path is unbounded.
        fn max_amount_out(asset1: AssetKind, asset2: AssetKind) -> Option<Balance>;

        /// The maximum amount of `asset1` a swap to `asset2` can currently take.
        ///
        /// If `include_fee` is `true`, the amount includes the swap fee. `None` if the swap can't
        /// be priced, a path not in [`Self::paths`] included. Otherwise zero if nothing can be
        /// taken and `Balance::max_value()` if there is no limit.
        fn max_amount_in(
            asset1: AssetKind,
            asset2: AssetKind,
            include_fee: bool,
        ) -> Option<Balance>;
    }
}
