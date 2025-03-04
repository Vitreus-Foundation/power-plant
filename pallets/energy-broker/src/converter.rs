#![allow(clippy::type_complexity)]

use super::*;

/// A trait for exchanging assets along a fixed conversion path.
pub trait FixedPathAssetConverter<T: Config> {
    /// The asset being converted from.
    const FROM: T::AssetKind;

    /// The asset being converted to.
    const TO: T::AssetKind;

    /// Computes the output amount for a given input.
    fn get_amount_out(amount_in: T::Balance) -> Option<T::Balance>;

    /// Computes the required input amount for a desired output.
    fn get_amount_in(amount_out: T::Balance) -> Option<T::Balance>;

    /// Resolves the credit into the broker account and returns the asset and amount if successful.
    fn resolve_into_broker(
        broker: &T::AccountId,
        credit: Credit<T::AccountId, T::Assets>,
    ) -> Result<(T::AssetKind, T::Balance), Credit<T::AccountId, T::Assets>> {
        let asset = credit.asset();
        let amount = credit.peek();

        T::Assets::resolve(broker, credit).map(|()| (asset, amount))
    }
}

/// A trait for converting between asset pairs.
pub trait AssetConverter<T: Config> {
    /// Returns the supported swap paths.
    fn paths() -> Vec<(T::AssetKind, T::AssetKind)>;

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

    /// Resolves the credit into the broker account and returns the asset and amount if successful.
    fn resolve_into_broker(
        path: &(T::AssetKind, T::AssetKind),
        broker: &T::AccountId,
        credit: Credit<T::AccountId, T::Assets>,
    ) -> Result<(T::AssetKind, T::Balance), Credit<T::AccountId, T::Assets>>;
}

#[impl_trait_for_tuples::impl_for_tuples(30)]
#[tuple_types_custom_trait_bound(FixedPathAssetConverter<T>)]
impl<T: Config> AssetConverter<T> for Tuple {
    fn paths() -> Vec<(T::AssetKind, T::AssetKind)> {
        let mut paths = Vec::new();
        for_tuples!( #( paths.push((Tuple::FROM, Tuple::TO)); )* );
        paths
    }

    fn get_amount_out(
        path: &(T::AssetKind, T::AssetKind),
        amount_in: T::Balance,
    ) -> Option<T::Balance> {
        for_tuples!( #(
            if path == &(Tuple::FROM, Tuple::TO) {
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
            if path == &(Tuple::FROM, Tuple::TO) {
                return Tuple::get_amount_in(amount_out);
            }
        )* );
        None
    }

    fn resolve_into_broker(
        path: &(T::AssetKind, T::AssetKind),
        broker: &T::AccountId,
        credit: Credit<T::AccountId, T::Assets>,
    ) -> Result<(T::AssetKind, T::Balance), Credit<T::AccountId, T::Assets>> {
        for_tuples!( #(
            if path == &(Tuple::FROM, Tuple::TO) {
                return Tuple::resolve_into_broker(broker, credit);
            }
        )* );
        Err(credit)
    }
}
