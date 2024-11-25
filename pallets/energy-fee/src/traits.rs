use crate::CallFee;
use frame_support::ensure;
use frame_support::traits::{
    fungible::{Balanced, Inspect},
    tokens::{Balance, ConversionFromAssetBalance, ConversionToAssetBalance, Precision},
    Get, Imbalance,
};
use frame_support::traits::{Currency, ExistenceRequirement, OnUnbalanced, WithdrawReasons};
use pallet_asset_rate::{Config as AssetRateConfig, Error as AssetRateError};
use sp_runtime::{DispatchError, FixedPointNumber, FixedPointOperand, TokenError};
use sp_std::marker::PhantomData;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type AssetIdOf<T> = <T as AssetRateConfig>::AssetKind;
type BalanceOf<T> = <<T as AssetRateConfig>::Currency as Inspect<AccountIdOf<T>>>::Balance;
/// Custom fee calculation for specified scenarios
pub trait CustomFee<RuntimeCall, DispatchInfo, Balance, ConstantFee>
where
    ConstantFee: Get<Balance>,
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfo>,
        calculated_fee: Option<Balance>,
    ) -> CallFee<Balance>;

    fn custom_fee() -> Balance;

    fn weight_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfo>,
        calculated_fee: Option<Balance>,
    ) -> Balance;

    fn ethereum_fee() -> Balance {
        Self::custom_fee()
    }
}

pub trait TokenExchange<AccountId, SourceToken, TargetToken, SourceTokenRecycleDest, TokenBalance>
where
    SourceToken: Currency<AccountId, Balance = TokenBalance>,
    TargetToken: Balanced<AccountId> + Inspect<AccountId, Balance = TokenBalance>,
    SourceTokenRecycleDest: OnUnbalanced<<SourceToken as Currency<AccountId>>::NegativeImbalance>,
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
        let credit = SourceToken::withdraw(
            who,
            amount_in,
            WithdrawReasons::TRANSACTION_PAYMENT,
            ExistenceRequirement::KeepAlive,
        )?;
        let credit_amount = credit.peek();
        // Regardless of whether the conversion is successful or not, we need to recycle the credit
        SourceTokenRecycleDest::on_unbalanced(credit);

        ensure!(credit_amount == amount_in, DispatchError::Token(TokenError::FundsUnavailable));
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
    BalanceOf<T>: FixedPointOperand,
{
    type Error = DispatchError;

    fn from_asset_balance(
        balance: BalanceOf<T>,
        asset_id: AssetIdOf<T>,
    ) -> Result<BalanceOf<T>, Self::Error> {
        P::from_asset_balance(balance, asset_id).map_err(|e| e.into())
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
    BalanceOf<T>: FixedPointOperand,
{
    type Error = DispatchError;

    fn to_asset_balance(
        balance: BalanceOf<T>,
        asset_id: AssetIdOf<T>,
    ) -> Result<BalanceOf<T>, Self::Error> {
        let rate = pallet_asset_rate::ConversionRateToNative::<T>::get(asset_id)
            .ok_or::<Self::Error>(AssetRateError::<T>::UnknownAssetKind.into())?;
        let result = rate
            .reciprocal()
            .ok_or(DispatchError::Other("Asset rate too low"))?
            .saturating_mul_int(balance);

        Ok(result)
    }
}

pub struct NativeExchange<AssetId, SourceToken, TargetToken, Rate, GetAssetId>(
    PhantomData<(AssetId, SourceToken, TargetToken, Rate, GetAssetId)>,
);

// TODO: rename types
impl<AC, AS, TT, ST, STD, B, G, R> TokenExchange<AC, ST, TT, STD, B>
    for NativeExchange<AS, ST, TT, R, G>
where
    TT: Balanced<AC> + Inspect<AC, Balance = B>,
    ST: Currency<AC, Balance = B>,
    STD: OnUnbalanced<<ST as Currency<AC>>::NegativeImbalance>,
    B: Balance,
    G: Get<AS>,
    R: ConversionFromAssetBalance<B, AS, B, Error = DispatchError>
        + ConversionToAssetBalance<B, AS, B, Error = DispatchError>,
{
    fn convert_from_input(amount: B) -> Result<B, DispatchError> {
        let asset_id = G::get();
        R::to_asset_balance(amount, asset_id)
    }

    fn convert_from_output(amount: B) -> Result<B, DispatchError> {
        let asset_id = G::get();
        R::from_asset_balance(amount, asset_id)
    }
}
