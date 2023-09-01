use crate::CallFee;
use frame_support::traits::{
    fungible::{Balanced, Inspect},
    tokens::{Balance, Fortitude, Precision, Preservation},
    Get,
};
use sp_arithmetic::{per_things::Rounding, ArithmeticError};
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;

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
    fn convert_from_input(amount: TokenBalance) -> Option<TokenBalance>;

    /// Calculate the amount of `SourceToken` corresponding to `amount` of `TargetToken`
    fn convert_from_output(amount: TokenBalance) -> Option<TokenBalance>;

    /// Exchange `SourceToken` -> `TargetToken` based on the `amount` of `SourceToken`
    /// on behalf of user `who`
    fn exchange_from_input(
        who: &AccountId,
        amount: TokenBalance,
    ) -> Result<TokenBalance, DispatchError> {
        if amount.is_zero() {
            return Ok(TokenBalance::zero());
        }
        let resulting_amount = Self::convert_from_input(amount)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;
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
        let resulting_amount = Self::convert_from_output(amount)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;
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

/// X of S tokens equal R*X of T tokens
pub struct Exchange<S, T, R>(PhantomData<(S, T, R)>);

impl<A, S, T, R, B> TokenExchange<A, S, T, B> for Exchange<S, T, R>
where
    R: Get<(B, B)>,
    S: Balanced<A> + Inspect<A, Balance = B>,
    T: Balanced<A> + Inspect<A, Balance = B>,
    B: Balance,
{
    fn convert_from_input(amount: B) -> Option<B> {
        let (numerator, denominator) = R::get();
        amount.multiply_rational(numerator, denominator, Rounding::Down)
    }

    fn convert_from_output(amount: B) -> Option<B> {
        let (numerator, denominator) = R::get();
        amount.multiply_rational(denominator, numerator, Rounding::Up)
    }
}
