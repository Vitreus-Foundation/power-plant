use crate::CallFee;
use frame_support::traits::{
    fungible::{Balanced, Inspect},
    tokens::{Balance, Preservation},
    Get, Imbalance,
};
use sp_arithmetic::{
    per_things::Rounding, rational::MultiplyRational, traits::Zero, ArithmeticError,
};
use sp_runtime::{DispatchError, TokenError};
use sp_std::marker::PhantomData;

type BalanceOf<T, A> = <T as Inspect<A>>::Balance;

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

pub trait TokenExchange<AccountId, SourceToken, TargetToken, TokenBalance, Credit, Debt>
where
    SourceToken: Balanced<AccountId> + Inspect<AccountId, Balance = TokenBalance>,
    TargetToken: Balanced<AccountId> + Inspect<AccountId, Balance = TokenBalance>,
    TokenBalance: Balance,
{
    fn convert_from_input(amount: TokenBalance) -> Option<TokenBalance>;

    fn convert_from_output(amount: TokenBalance) -> Option<TokenBalance>;

    fn exchange_from_input(
        who: &AccountId,
        amount: TokenBalance,
    ) -> Result<TokenBalance, DispatchError> {
        if amount.is_zero() {
            return Ok(BalanceOf::<SourceToken, AccountId>::zero());
        }
        let resulting_amount = Self::convert_from_input(amount)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

        Self::exchange_inner(who, amount, resulting_amount)
    }

    fn exchange_from_output(
        who: &AccountId,
        amount: TokenBalance,
    ) -> Result<TokenBalance, DispatchError> {
        if amount.is_zero() {
            return Ok(BalanceOf::<SourceToken, AccountId>::zero());
        }
        let resulting_amount = Self::convert_from_output(amount)
            .ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

        Self::exchange_inner(who, resulting_amount, amount)
    }

    // TODO: handle dust without errors
    fn exchange_inner(
        who: &AccountId,
        amount_in: TokenBalance,
        amount_out: TokenBalance,
    ) -> Result<TokenBalance, DispatchError> {
        let debt = SourceToken::rescind(amount_in);
        _ = SourceToken::settle(who, debt, Preservation::Protect)
            .map_err(|_| DispatchError::Token(TokenError::BelowMinimum))?
            .drop_zero()
            .map_err(|_| {
                DispatchError::Other("Something went wrong during token withdrawal (dust left)")
            })?;

        let credit = TargetToken::issue(amount_out);
        _ = SourceToken::resolve(who, credit)
            .map_err(|_| DispatchError::Token(TokenError::BelowMinimum))?;

        Ok(amount_out)
    }
}

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
