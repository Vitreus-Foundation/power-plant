//! Traits and implementations for exchanging native currency and energy.
//!
//! # Available Traits
//! - [`QuotePrice`]: Quotes swaps between asset classes.
//! - [`MaxAmountOut`]: The maximum amount a swap between asset classes can currently supply.
//! - [`QuotePriceIntoEnergy`] and [`QuotePriceFromEnergy`]: Quotes swaps between native currency
//!   and one energy asset, one direction each.
//! - [`SwapIntoEnergy`] and [`SwapFromEnergy`]: Swaps between native currency and one energy
//!   asset, one direction each.
//!
//! Exchanges implement [`Swap`], [`QuotePrice`] and [`MaxAmountOut`]. Pallets use the
//! one-direction traits instead, with `EnergyAsset` bound to the fungible they hold, and don't see
//! the exchange's paths or asset kinds. [`NativeEnergyExchange`] implements the one-direction
//! traits on top of an exchange. `()` implements them as no exchange at all: it quotes nothing,
//! can supply nothing and fails every swap with [`TokenError::Unsupported`].

use frame_support::traits::{
    fungible::ItemOf,
    tokens::{fungibles, Balance},
};
use sp_runtime::{traits::Get, DispatchError, TokenError};
use sp_std::vec;

pub use pallet_asset_conversion::Swap;

// TODO: remove this declaration after updating to stable2409
/// Trait providing methods to quote swap prices between asset classes.
///
/// The quoted price is only guaranteed if no other swaps are made after the price is quoted and
/// before the target swap (e.g., the swap is made immediately within the same transaction).
pub trait QuotePrice {
    /// Measurement units of the asset classes for pricing.
    type Balance: Balance;
    /// Type representing the kind of assets for which the price is being quoted.
    type AssetKind;
    /// Quotes the amount of `asset1` required to obtain the exact `amount` of `asset2`.
    ///
    /// If `include_fee` is set to `true`, the price will include the pool's fee.
    /// If the pool does not exist or the swap cannot be made, `None` is returned.
    fn quote_price_tokens_for_exact_tokens(
        asset1: Self::AssetKind,
        asset2: Self::AssetKind,
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;
    /// Quotes the amount of `asset2` resulting from swapping the exact `amount` of `asset1`.
    ///
    /// If `include_fee` is set to `true`, the price will include the pool's fee.
    /// If the pool does not exist or the swap cannot be made, `None` is returned.
    fn quote_price_exact_tokens_for_tokens(
        asset1: Self::AssetKind,
        asset2: Self::AssetKind,
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;
}

/// Trait providing the maximum amount an exchange can currently supply of an asset.
pub trait MaxAmountOut: QuotePrice {
    /// Returns the maximum amount of `asset2` a swap from `asset1` can currently supply.
    ///
    /// Zero if nothing can be supplied, including on an unsupported path, and
    /// `Balance::max_value()` if there is no limit. Quoting or swapping for more fails.
    fn max_amount_out(asset1: Self::AssetKind, asset2: Self::AssetKind) -> Self::Balance;
}

/// Trait providing methods to quote swap prices for exchanging native currency for energy.
pub trait QuotePriceIntoEnergy {
    /// Measurement units of both native currency and energy for pricing.
    type Balance: Balance;
    /// The energy asset being quoted, as a fungible.
    ///
    /// Consumers bind it to the fungible they use, so a runtime cannot give them an exchange for
    /// another asset.
    type EnergyAsset;

    /// Quotes the amount of energy resulting from swapping the exact `amount` of native currency.
    ///
    /// If `include_fee` is set to `true`, the price will include the swap fee.
    /// If the swap cannot be priced, has a zero amount on either side or would supply more than
    /// [`Self::max_amount_out`], `None` is returned.
    fn quote_price_exact_tokens_for_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;

    /// Quotes the amount of native currency required to obtain the exact `amount` of energy.
    ///
    /// If `include_fee` is set to `true`, the price will include the swap fee.
    /// If the swap cannot be priced, has a zero amount on either side or would supply more than
    /// [`Self::max_amount_out`], `None` is returned.
    fn quote_price_tokens_for_exact_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;

    /// Returns the maximum amount of energy a swap can currently supply.
    ///
    /// Zero if nothing can be supplied, `Balance::max_value()` if there is no limit. Quoting or
    /// swapping for more fails.
    fn max_amount_out() -> Self::Balance;
}

/// Trait providing methods to quote swap prices for exchanging energy for native currency.
pub trait QuotePriceFromEnergy {
    /// Measurement units of both native currency and energy for pricing.
    type Balance: Balance;
    /// The energy asset being quoted, as a fungible.
    ///
    /// Consumers bind it to the fungible they use, so a runtime cannot give them an exchange for
    /// another asset.
    type EnergyAsset;

    /// Quotes the amount of native currency resulting from swapping the exact `amount` of energy.
    ///
    /// If `include_fee` is set to `true`, the price will include the swap fee.
    /// If the swap cannot be priced, has a zero amount on either side or would supply more than
    /// [`Self::max_amount_out`], `None` is returned.
    fn quote_price_exact_tokens_for_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;

    /// Quotes the amount of energy required to obtain the exact `amount` of native currency.
    ///
    /// If `include_fee` is set to `true`, the price will include the swap fee.
    /// If the swap cannot be priced, has a zero amount on either side or would supply more than
    /// [`Self::max_amount_out`], `None` is returned.
    fn quote_price_tokens_for_exact_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;

    /// Returns the maximum amount of native currency a swap can currently supply.
    ///
    /// Zero if nothing can be supplied, `Balance::max_value()` if there is no limit. Quoting or
    /// swapping for more fails.
    fn max_amount_out() -> Self::Balance;
}

/// Trait for providing methods to swap native currency for energy.
pub trait SwapIntoEnergy<AccountId> {
    /// Measurement units of both native currency and energy for swapping.
    type Balance: Balance;
    /// The energy asset being swapped, as a fungible.
    ///
    /// Consumers bind it to the fungible they use, so a runtime cannot give them an exchange for
    /// another asset.
    type EnergyAsset;

    /// Swap exactly `amount_in` of native currency for energy.
    ///
    /// Withdraws the native currency from `who`, deposits the energy to `who`, respecting
    /// `keep_alive`.
    ///
    /// If successful, returns the amount of energy acquired for the `amount_in`.
    ///
    /// This operation is expected to be atomic.
    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;

    /// Take native currency and swap some amount for `amount_out` of energy.
    ///
    /// Withdraws the native currency from `who`, deposits the energy to `who`, respecting
    /// `keep_alive`.
    ///
    /// If successful, returns the amount of native currency taken to provide `amount_out`.
    ///
    /// This operation is expected to be atomic.
    fn swap_tokens_for_exact_tokens(
        who: AccountId,
        amount_out: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;
}

/// Trait for providing methods to swap energy for native currency.
pub trait SwapFromEnergy<AccountId> {
    /// Measurement units of both native currency and energy for swapping.
    type Balance: Balance;
    /// The energy asset being swapped, as a fungible.
    ///
    /// Consumers bind it to the fungible they use, so a runtime cannot give them an exchange for
    /// another asset.
    type EnergyAsset;

    /// Swap exactly `amount_in` of energy for native currency.
    ///
    /// Withdraws the energy from `who`, deposits the native currency to `who`, respecting
    /// `keep_alive`.
    ///
    /// If successful, returns the amount of native currency acquired for the `amount_in`.
    ///
    /// This operation is expected to be atomic.
    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;

    /// Take energy and swap some amount for `amount_out` of native currency.
    ///
    /// Withdraws the energy from `who`, deposits the native currency to `who`, respecting
    /// `keep_alive`.
    ///
    /// If successful, returns the amount of energy taken to provide `amount_out`.
    ///
    /// This operation is expected to be atomic.
    fn swap_tokens_for_exact_tokens(
        who: AccountId,
        amount_out: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;
}

/// An exchange for swapping between native currency and energy.
///
/// `EnergyAsset` is the energy asset as a fungible, an [`ItemOf`]; `Exchange` is called with the
/// `ItemOf`'s asset id, converted into the exchange's asset kind.
///
/// It quotes and swaps along the native-to-energy and energy-to-native paths of `Exchange`, so its
/// limits are `Exchange`'s `max_amount_out` and its swaps are as atomic as `Exchange`'s [`Swap`].
pub struct NativeEnergyExchange<Exchange, NativeAsset, EnergyAsset>(
    core::marker::PhantomData<(Exchange, NativeAsset, EnergyAsset)>,
);

impl<AccountId, Exchange, NativeAsset, F, A> QuotePriceIntoEnergy
    for NativeEnergyExchange<Exchange, NativeAsset, ItemOf<F, A, AccountId>>
where
    Exchange: MaxAmountOut,
    NativeAsset: Get<Exchange::AssetKind>,
    F: fungibles::Inspect<AccountId>,
    A: Get<F::AssetId>,
    F::AssetId: Into<Exchange::AssetKind>,
{
    type Balance = Exchange::Balance;
    type EnergyAsset = ItemOf<F, A, AccountId>;

    fn quote_price_exact_tokens_for_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance> {
        Exchange::quote_price_exact_tokens_for_tokens(
            NativeAsset::get(),
            A::get().into(),
            amount,
            include_fee,
        )
    }

    fn quote_price_tokens_for_exact_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance> {
        Exchange::quote_price_tokens_for_exact_tokens(
            NativeAsset::get(),
            A::get().into(),
            amount,
            include_fee,
        )
    }

    fn max_amount_out() -> Self::Balance {
        Exchange::max_amount_out(NativeAsset::get(), A::get().into())
    }
}

impl<AccountId, Exchange, NativeAsset, F, A> QuotePriceFromEnergy
    for NativeEnergyExchange<Exchange, NativeAsset, ItemOf<F, A, AccountId>>
where
    Exchange: MaxAmountOut,
    NativeAsset: Get<Exchange::AssetKind>,
    F: fungibles::Inspect<AccountId>,
    A: Get<F::AssetId>,
    F::AssetId: Into<Exchange::AssetKind>,
{
    type Balance = Exchange::Balance;
    type EnergyAsset = ItemOf<F, A, AccountId>;

    fn quote_price_exact_tokens_for_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance> {
        Exchange::quote_price_exact_tokens_for_tokens(
            A::get().into(),
            NativeAsset::get(),
            amount,
            include_fee,
        )
    }

    fn quote_price_tokens_for_exact_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance> {
        Exchange::quote_price_tokens_for_exact_tokens(
            A::get().into(),
            NativeAsset::get(),
            amount,
            include_fee,
        )
    }

    fn max_amount_out() -> Self::Balance {
        Exchange::max_amount_out(A::get().into(), NativeAsset::get())
    }
}

impl<AccountId, Exchange, NativeAsset, F, A> SwapIntoEnergy<AccountId>
    for NativeEnergyExchange<Exchange, NativeAsset, ItemOf<F, A, AccountId>>
where
    AccountId: Clone,
    Exchange: Swap<AccountId>,
    NativeAsset: Get<Exchange::AssetKind>,
    F: fungibles::Inspect<AccountId>,
    A: Get<F::AssetId>,
    F::AssetId: Into<Exchange::AssetKind>,
{
    type Balance = Exchange::Balance;
    type EnergyAsset = ItemOf<F, A, AccountId>;

    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        let send_to = who.clone();

        Exchange::swap_exact_tokens_for_tokens(
            who,
            vec![NativeAsset::get(), A::get().into()],
            amount_in,
            None,
            send_to,
            keep_alive,
        )
    }

    fn swap_tokens_for_exact_tokens(
        who: AccountId,
        amount_out: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        let send_to = who.clone();

        Exchange::swap_tokens_for_exact_tokens(
            who,
            vec![NativeAsset::get(), A::get().into()],
            amount_out,
            None,
            send_to,
            keep_alive,
        )
    }
}

impl<AccountId, Exchange, NativeAsset, F, A> SwapFromEnergy<AccountId>
    for NativeEnergyExchange<Exchange, NativeAsset, ItemOf<F, A, AccountId>>
where
    AccountId: Clone,
    Exchange: Swap<AccountId>,
    NativeAsset: Get<Exchange::AssetKind>,
    F: fungibles::Inspect<AccountId>,
    A: Get<F::AssetId>,
    F::AssetId: Into<Exchange::AssetKind>,
{
    type Balance = Exchange::Balance;
    type EnergyAsset = ItemOf<F, A, AccountId>;

    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        let send_to = who.clone();

        Exchange::swap_exact_tokens_for_tokens(
            who,
            vec![A::get().into(), NativeAsset::get()],
            amount_in,
            None,
            send_to,
            keep_alive,
        )
    }

    fn swap_tokens_for_exact_tokens(
        who: AccountId,
        amount_out: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        let send_to = who.clone();

        Exchange::swap_tokens_for_exact_tokens(
            who,
            vec![A::get().into(), NativeAsset::get()],
            amount_out,
            None,
            send_to,
            keep_alive,
        )
    }
}

impl QuotePriceIntoEnergy for () {
    type Balance = u32;
    type EnergyAsset = ();

    fn quote_price_exact_tokens_for_tokens(
        _amount: Self::Balance,
        _include_fee: bool,
    ) -> Option<Self::Balance> {
        None
    }

    fn quote_price_tokens_for_exact_tokens(
        _amount: Self::Balance,
        _include_fee: bool,
    ) -> Option<Self::Balance> {
        None
    }

    fn max_amount_out() -> Self::Balance {
        0
    }
}

impl QuotePriceFromEnergy for () {
    type Balance = u32;
    type EnergyAsset = ();

    fn quote_price_exact_tokens_for_tokens(
        _amount: Self::Balance,
        _include_fee: bool,
    ) -> Option<Self::Balance> {
        None
    }

    fn quote_price_tokens_for_exact_tokens(
        _amount: Self::Balance,
        _include_fee: bool,
    ) -> Option<Self::Balance> {
        None
    }

    fn max_amount_out() -> Self::Balance {
        0
    }
}

impl<AccountId> SwapIntoEnergy<AccountId> for () {
    type Balance = u32;
    type EnergyAsset = ();

    fn swap_exact_tokens_for_tokens(
        _who: AccountId,
        _amount_in: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(TokenError::Unsupported.into())
    }

    fn swap_tokens_for_exact_tokens(
        _who: AccountId,
        _amount_out: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(TokenError::Unsupported.into())
    }
}

impl<AccountId> SwapFromEnergy<AccountId> for () {
    type Balance = u32;
    type EnergyAsset = ();

    fn swap_exact_tokens_for_tokens(
        _who: AccountId,
        _amount_in: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(TokenError::Unsupported.into())
    }

    fn swap_tokens_for_exact_tokens(
        _who: AccountId,
        _amount_out: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(TokenError::Unsupported.into())
    }
}
