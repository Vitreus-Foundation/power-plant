#![allow(clippy::type_complexity)]

use super::*;

/// A trait for exchanging assets along a fixed conversion path.
pub trait FixedPathAssetConverter<T: Config> {
    /// The asset being converted from.
    const SOURCE: T::AssetKind;

    /// The asset being converted to.
    const TARGET: T::AssetKind;

    /// Returns a custom swap fee for the given asset pair, or `None` to use the default fee.
    fn swap_fee() -> Option<u32> {
        None
    }

    /// The rate of the path, `TARGET` per unit of `SOURCE`.
    ///
    /// `None` when the path has no rate, such as before its rate source is set.
    fn conversion_rate() -> Option<FixedU128>;

    /// The maximum amount of the output asset the path can currently supply, or
    /// `Balance::max_value()` if the output is minted and therefore unbounded.
    ///
    /// This is what the pallet checks before it debits anyone, so it must never exceed what
    /// [`Self::withdraw`] can take from the broker. It can be less when the path has a limit of
    /// its own. Otherwise override it together with [`Self::withdraw`].
    fn reducible_balance(broker: &T::AccountId) -> T::Balance {
        T::Assets::reducible_balance(Self::TARGET, broker, Preserve, Polite)
    }

    /// Withdraws balance from the broker account and returns a credit if successful.
    fn withdraw(
        broker: &T::AccountId,
        value: T::Balance,
    ) -> Result<Credit<T::AccountId, T::Assets>, DispatchError> {
        T::Assets::withdraw(Self::TARGET, broker, value, Exact, Preserve, Polite)
    }

    /// Resolves the credit into the broker account.
    fn resolve(
        broker: &T::AccountId,
        credit: Credit<T::AccountId, T::Assets>,
    ) -> Result<(), Credit<T::AccountId, T::Assets>> {
        T::Assets::resolve(broker, credit)
    }
}

/// A trait for converting between asset pairs.
pub trait AssetConverter<T: Config> {
    /// Returns the supported swap paths.
    fn paths() -> Vec<(T::AssetKind, T::AssetKind)>;

    /// Returns a custom swap fee for the given asset pair, or `None` to use the default fee.
    fn swap_fee(path: &(T::AssetKind, T::AssetKind)) -> Option<u32>;

    /// [`FixedPathAssetConverter::conversion_rate`] of the converter for `path`.
    ///
    /// `None` for a path no converter serves.
    fn conversion_rate(path: &(T::AssetKind, T::AssetKind)) -> Option<FixedU128>;

    /// The maximum amount of the output asset the given path can currently supply, never more
    /// than [`Self::withdraw`] can take from the broker.
    fn reducible_balance(path: &(T::AssetKind, T::AssetKind), broker: &T::AccountId) -> T::Balance;

    /// Withdraws balance from the broker account and returns a credit if successful.
    fn withdraw(
        path: &(T::AssetKind, T::AssetKind),
        broker: &T::AccountId,
        value: T::Balance,
    ) -> Result<Credit<T::AccountId, T::Assets>, DispatchError>;

    /// Resolves the credit into the broker account.
    fn resolve(
        path: &(T::AssetKind, T::AssetKind),
        broker: &T::AccountId,
        credit: Credit<T::AccountId, T::Assets>,
    ) -> Result<(), Credit<T::AccountId, T::Assets>>;
}

#[impl_trait_for_tuples::impl_for_tuples(30)]
#[tuple_types_custom_trait_bound(FixedPathAssetConverter<T>)]
impl<T: Config> AssetConverter<T> for Tuple {
    #[allow(clippy::let_and_return, clippy::vec_init_then_push)]
    fn paths() -> Vec<(T::AssetKind, T::AssetKind)> {
        let mut paths = Vec::new();
        for_tuples!( #( paths.push((Tuple::SOURCE, Tuple::TARGET)); )* );
        paths
    }

    fn swap_fee(path: &(T::AssetKind, T::AssetKind)) -> Option<u32> {
        for_tuples!( #(
            if path == &(Tuple::SOURCE, Tuple::TARGET) {
                return Tuple::swap_fee();
            }
        )* );
        None
    }

    fn conversion_rate(path: &(T::AssetKind, T::AssetKind)) -> Option<FixedU128> {
        for_tuples!( #(
            if path == &(Tuple::SOURCE, Tuple::TARGET) {
                return Tuple::conversion_rate();
            }
        )* );
        None
    }

    fn reducible_balance(path: &(T::AssetKind, T::AssetKind), broker: &T::AccountId) -> T::Balance {
        for_tuples!( #(
            if path == &(Tuple::SOURCE, Tuple::TARGET) {
                return Tuple::reducible_balance(broker);
            }
        )* );
        Zero::zero()
    }

    fn withdraw(
        path: &(T::AssetKind, T::AssetKind),
        broker: &T::AccountId,
        value: T::Balance,
    ) -> Result<Credit<T::AccountId, T::Assets>, DispatchError> {
        for_tuples!( #(
            if path == &(Tuple::SOURCE, Tuple::TARGET) {
                return Tuple::withdraw(broker, value);
            }
        )* );
        Err(TokenError::UnknownAsset.into())
    }

    fn resolve(
        path: &(T::AssetKind, T::AssetKind),
        broker: &T::AccountId,
        credit: Credit<T::AccountId, T::Assets>,
    ) -> Result<(), Credit<T::AccountId, T::Assets>> {
        for_tuples!( #(
            if path == &(Tuple::SOURCE, Tuple::TARGET) {
                return Tuple::resolve(broker, credit);
            }
        )* );
        Err(credit)
    }
}
