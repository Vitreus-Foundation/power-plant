use pallet_dex_contracts_api::{ExchangeTokenPair, MomentOf};
use sp_core::{H160, U256};
use sp_runtime::DispatchError;

pub trait Dex<Balance, Moment, AccountId>
where
    Balance: Into<U256>,
    Moment: Into<U256>,
    AccountId: Into<H160>,
{
    fn swap_exact_input_single(
        token_pair: ExchangeTokenPair,
        fee: u32,
        recipient: AccountId,
        deadline: Moment,
        amount_in: Balance,
        amount_out_minimum: Balance,
        sqrt_price_limit_x96: U256,
    ) -> Result<Balance, DispatchError>;

    fn swap_exact_output_single(
        token_pair: ExchangeTokenPair,
        fee: u32,
        recipient: AccountId,
        deadline: Moment,
        amount_out: Balance,
        amount_in_maximum: Balance,
        sqrt_price_limit_x96: U256,
    ) -> Result<Balance, DispatchError>;
}
