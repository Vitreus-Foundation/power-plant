use crate::CallFee;
use frame_support::traits::{
    fungible::{Balanced, Inspect},
    tokens::{
        Balance, ConversionFromAssetBalance, ConversionToAssetBalance, Fortitude, Precision,
        Preservation,
    },
    Get,
};
use pallet_asset_rate::{Config as AssetRateConfig, Error as AssetRateError};
use sp_runtime::{DispatchError, FixedPointNumber};
use sp_std::marker::PhantomData;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type AssetIdOf<T> = <T as AssetRateConfig>::AssetId;
type BalanceOf<T> = <<T as AssetRateConfig>::Currency as Inspect<AccountIdOf<T>>>::Balance;
/// Custom fee calculation for specified scenarios
pub trait CustomFee<RuntimeCall, DispatchInfo, Balance, ConstantFee>
where
    ConstantFee: Get<Balance>,
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: &DispatchInfo,
    ) -> CallFee<Balance>;
}

pub trait TokenExchange<AccountId, SourceToken, TargetToken, TokenBalance>
where
    SourceToken: Balanced<AccountId> + Inspect<AccountId, Balance = TokenBalance>,
    TargetToken: Balanced<AccountId> + Inspect<AccountId, Balance = TokenBalance>,
    TokenBalance: Balance,
{
    /// Calculate the amount of `TargetToken` corresponding to `amount` of `SourceToken`
    fn convert_from_input(amount: TokenBalance) -> Result<TokenBalance, DispatchError>;

    /// Calculate the amount of `SourceToken` corresponding to `amount` of `TargetToken`
    fn convert_from_output(amount: TokenBalance) -> Result<TokenBalance, DispatchError>;

    /// Exchange `SourceToken` -> `TargetToken` based on the `amount` of `SourceToken`
    /// on behalf of user `who`
    fn exchange_from_input(
        who: &AccountId,
        amount: TokenBalance,
    ) -> Result<TokenBalance, DispatchError> {
        if amount.is_zero() {
            return Ok(TokenBalance::zero());
        }
        let resulting_amount = Self::convert_from_input(amount)?;
        Self::exchange_inner(who, amount, resulting_amount)
    }

    /// Exchange `SourceToken` -> `TargetToken` based on the `amount` of `TargetToken`
    /// on behalf of user `who`
    fn exchange_from_output(
        who: &AccountId,
        amount: TokenBalance,
    ) -> Result<TokenBalance, DispatchError> {
        if amount.is_zero() {
            return Ok(TokenBalance::zero());
        }
        let resulting_amount = Self::convert_from_output(amount)?;
        Self::exchange_inner(who, resulting_amount, amount)
    }

    fn exchange_inner(
        who: &AccountId,
        amount_in: TokenBalance,
        amount_out: TokenBalance,
    ) -> Result<TokenBalance, DispatchError> {
        let _ = SourceToken::withdraw(
            who,
            amount_in,
            Precision::Exact,
            Preservation::Protect,
            Fortitude::Polite,
        )?;
        let _ = TargetToken::deposit(who, amount_out, Precision::Exact)?;
        Ok(amount_out)
    }
}

pub struct AssetsBalancesConverter<T, P>(PhantomData<(T, P)>);

impl<T: AssetRateConfig, P> ConversionFromAssetBalance<BalanceOf<T>, AssetIdOf<T>, BalanceOf<T>>
    for AssetsBalancesConverter<T, P>
where
    P: ConversionFromAssetBalance<
        BalanceOf<T>,
        AssetIdOf<T>,
        BalanceOf<T>,
        Error = AssetRateError<T>,
    >,
{
    type Error = AssetRateError<T>;

    fn from_asset_balance(
        balance: BalanceOf<T>,
        asset_id: AssetIdOf<T>,
    ) -> Result<BalanceOf<T>, Self::Error> {
        P::from_asset_balance(balance, asset_id)
    }
}

impl<T: AssetRateConfig, P> ConversionToAssetBalance<BalanceOf<T>, AssetIdOf<T>, BalanceOf<T>>
    for AssetsBalancesConverter<T, P>
where
    P: ConversionFromAssetBalance<
        BalanceOf<T>,
        AssetIdOf<T>,
        BalanceOf<T>,
        Error = AssetRateError<T>,
    >,
{
    type Error = AssetRateError<T>;

    fn to_asset_balance(
        balance: BalanceOf<T>,
        asset_id: AssetIdOf<T>,
    ) -> Result<BalanceOf<T>, Self::Error> {
        let rate = pallet_asset_rate::ConversionRateToNative::<T>::get(asset_id)
            .ok_or(Self::Error::UnknownAssetId)?;
        Ok(rate.saturating_div_int(balance))
    }
}

pub struct NativeExchange<AssetId, SourceToken, TargetToken, Rate, GetAssetId>(
    PhantomData<(AssetId, SourceToken, TargetToken, Rate, GetAssetId)>,
);

impl<AC, AS, TT, ST, B, G, R> TokenExchange<AC, ST, TT, B> for NativeExchange<AS, ST, TT, R, G>
where
    TT: Balanced<AC> + Inspect<AC, Balance = B>,
    ST: Balanced<AC> + Inspect<AC, Balance = B>,
    B: Balance,
    G: Get<AS>,
    R: ConversionFromAssetBalance<B, AS, B> + ConversionToAssetBalance<B, AS, B>,
    <R as ConversionFromAssetBalance<B, AS, B>>::Error: Into<DispatchError>,
    <R as ConversionToAssetBalance<B, AS, B>>::Error: Into<DispatchError>,
{
    fn convert_from_input(amount: B) -> Result<B, DispatchError> {
        let asset_id = G::get();
        R::to_asset_balance(amount, asset_id).map_err(|e| e.into())
    }

    fn convert_from_output(amount: B) -> Result<B, DispatchError> {
        let asset_id = G::get();
        R::from_asset_balance(amount, asset_id).map_err(|e| e.into())
    }
}

// X of S tokens equal R*X of T tokens
// pub struct Exchange<S, T, R>(PhantomData<(S, T, R)>);
//
// impl<A, S, T, R, B> TokenExchange<A, S, T, B> for Exchange<S, T, R>
// where
//     R: Get<(B, B)>,
//     S: Balanced<A> + Inspect<A, Balance = B>,
//     T: Balanced<A> + Inspect<A, Balance = B>,
//     B: Balance,
// {
//     fn convert_from_input(amount: B) -> Option<B> {
//         let (numerator, denominator) = R::get();
//         amount.multiply_rational(numerator, denominator, Rounding::Down)
//     }
//
//     fn convert_from_output(amount: B) -> Option<B> {
//         let (numerator, denominator) = R::get();
//         amount.multiply_rational(denominator, numerator, Rounding::Up)
//     }
// }
