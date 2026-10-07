use frame_support::traits::{
    fungible::ItemOf,
    tokens::{fungibles, Balance},
};
use sp_runtime::{traits::Get, DispatchError};
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

/// Trait providing methods to quote swap prices for exchanging native currency for energy.
pub trait QuotePriceNativeForEnergy {
    /// Measurement units of the asset classes for pricing.
    type Balance: Balance;
    /// Type representing the kind of assets for which the price is being quoted.
    type AssetKind;
    /// A constant representing the native asset kind.
    type NativeAsset: Get<Self::AssetKind>;
    /// The energy asset being quoted, as a fungible.
    type EnergyAsset;

    /// Quotes the amount of energy resulting from swapping the exact `amount` of native currency.
    fn quote_price_exact_tokens_for_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;

    /// Quotes the amount of native currency required to obtain the exact `amount` of energy.
    fn quote_price_tokens_for_exact_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;
}

/// Trait providing methods to quote swap prices for exchanging energy for native currency.
pub trait QuotePriceEnergyForNative {
    /// Measurement units of the asset classes for pricing.
    type Balance: Balance;
    /// Type representing the kind of assets for which the price is being quoted.
    type AssetKind;
    /// A constant representing the native asset kind.
    type NativeAsset: Get<Self::AssetKind>;
    /// The energy asset being quoted, as a fungible.
    type EnergyAsset;

    /// Quotes the amount of native currency resulting from swapping the exact `amount` of energy.
    fn quote_price_exact_tokens_for_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;

    /// Quotes the amount of energy required to obtain the exact `amount` of native currency.
    fn quote_price_tokens_for_exact_tokens(
        amount: Self::Balance,
        include_fee: bool,
    ) -> Option<Self::Balance>;
}

/// A trait for swapping native currency for energy.
pub trait SwapNativeForEnergy<AccountId> {
    /// The type in which the assets for swapping are measured.
    type Balance: Balance;
    /// Type representing the kind of assets for swapping.
    type AssetKind;
    /// A constant representing the native asset kind.
    type NativeAsset: Get<Self::AssetKind>;
    /// The energy asset being swapped, as a fungible.
    type EnergyAsset;

    /// Swaps an exact amount of native currency for energy.
    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;

    /// Swaps native currency for an exact amount of energy.
    fn swap_tokens_for_exact_tokens(
        who: AccountId,
        amount_out: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;
}

/// A trait for swapping energy for native currency.
pub trait SwapEnergyForNative<AccountId> {
    /// The type in which the assets for swapping are measured.
    type Balance: Balance;
    /// Type representing the kind of assets for swapping.
    type AssetKind;
    /// A constant representing the native asset kind.
    type NativeAsset: Get<Self::AssetKind>;
    /// The energy asset being swapped, as a fungible.
    type EnergyAsset;

    /// Swaps an exact amount of energy for native currency.
    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;

    /// Swaps energy for an exact amount of native currency.
    fn swap_tokens_for_exact_tokens(
        who: AccountId,
        amount_out: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError>;
}

/// An exchange for swapping between native tokens and energy.
///
/// `EnergyAsset` is the energy asset as a fungible, an [`ItemOf`]; `Exchange` is called with the
/// `ItemOf`'s asset id, converted into the exchange's asset kind.
pub struct NativeEnergyExchange<Exchange, NativeAsset, EnergyAsset>(
    core::marker::PhantomData<(Exchange, NativeAsset, EnergyAsset)>,
);

impl<AccountId, Exchange, NativeAsset, F, A> QuotePriceNativeForEnergy
    for NativeEnergyExchange<Exchange, NativeAsset, ItemOf<F, A, AccountId>>
where
    Exchange: QuotePrice,
    NativeAsset: Get<Exchange::AssetKind>,
    F: fungibles::Inspect<AccountId>,
    A: Get<F::AssetId>,
    F::AssetId: Into<Exchange::AssetKind>,
{
    type Balance = Exchange::Balance;
    type AssetKind = Exchange::AssetKind;
    type NativeAsset = NativeAsset;
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
}

impl<AccountId, Exchange, NativeAsset, F, A> QuotePriceEnergyForNative
    for NativeEnergyExchange<Exchange, NativeAsset, ItemOf<F, A, AccountId>>
where
    Exchange: QuotePrice,
    NativeAsset: Get<Exchange::AssetKind>,
    F: fungibles::Inspect<AccountId>,
    A: Get<F::AssetId>,
    F::AssetId: Into<Exchange::AssetKind>,
{
    type Balance = Exchange::Balance;
    type AssetKind = Exchange::AssetKind;
    type NativeAsset = NativeAsset;
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
}

impl<AccountId, Exchange, NativeAsset, F, A> SwapNativeForEnergy<AccountId>
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
    type AssetKind = Exchange::AssetKind;
    type NativeAsset = NativeAsset;
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

impl<AccountId, Exchange, NativeAsset, F, A> SwapEnergyForNative<AccountId>
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
    type AssetKind = Exchange::AssetKind;
    type NativeAsset = NativeAsset;
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

impl QuotePriceNativeForEnergy for () {
    type Balance = u32;
    type AssetKind = ();
    type NativeAsset = ();
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
}

impl QuotePriceEnergyForNative for () {
    type Balance = u32;
    type AssetKind = ();
    type NativeAsset = ();
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
}

impl<AccountId> SwapNativeForEnergy<AccountId> for () {
    type Balance = u32;
    type AssetKind = ();
    type NativeAsset = ();
    type EnergyAsset = ();

    fn swap_exact_tokens_for_tokens(
        _who: AccountId,
        _amount_in: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(DispatchError::Other("not implemented"))
    }

    fn swap_tokens_for_exact_tokens(
        _who: AccountId,
        _amount_out: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(DispatchError::Other("not implemented"))
    }
}

impl<AccountId> SwapEnergyForNative<AccountId> for () {
    type Balance = u32;
    type AssetKind = ();
    type NativeAsset = ();
    type EnergyAsset = ();

    fn swap_exact_tokens_for_tokens(
        _who: AccountId,
        _amount_in: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(DispatchError::Other("not implemented"))
    }

    fn swap_tokens_for_exact_tokens(
        _who: AccountId,
        _amount_out: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        Err(DispatchError::Other("not implemented"))
    }
}
