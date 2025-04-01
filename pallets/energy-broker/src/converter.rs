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

    /// Computes the output amount for a given input.
    fn get_amount_out(amount_in: T::Balance) -> Option<T::Balance>;

    /// Computes the required input amount for a desired output.
    fn get_amount_in(amount_out: T::Balance) -> Option<T::Balance>;

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

    /// Computes the output amount for a given input.
    fn get_amount_out(
        path: &(T::AssetKind, T::AssetKind),
        amount_in: T::Balance,
    ) -> Option<T::Balance>;

    /// Computes the required input amount for a desired output.
    fn get_amount_in(
        path: &(T::AssetKind, T::AssetKind),
        amount_out: T::Balance,
    ) -> Option<T::Balance>;

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

    fn get_amount_out(
        path: &(T::AssetKind, T::AssetKind),
        amount_in: T::Balance,
    ) -> Option<T::Balance> {
        for_tuples!( #(
            if path == &(Tuple::SOURCE, Tuple::TARGET) {
                return Tuple::get_amount_out(amount_in);
            }
        )* );
        None
    }

    fn get_amount_in(
        path: &(T::AssetKind, T::AssetKind),
        amount_out: T::Balance,
    ) -> Option<T::Balance> {
        for_tuples!( #(
            if path == &(Tuple::SOURCE, Tuple::TARGET) {
                return Tuple::get_amount_in(amount_out);
            }
        )* );
        None
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
