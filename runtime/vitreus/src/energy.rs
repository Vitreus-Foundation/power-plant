//! Exchanges the runtime's pallets use to buy and sell energy for native tokens.

// TODO: move the energy broker's config and converters here from lib.rs.

use crate::{
    AccountId, Balance, EnergyAsset, EnergyBroker, LiquidEnergyAsset, NativeAsset,
    StaticEnergyAsset,
};
use frame_support::{
    storage::with_storage_layer,
    traits::{
        fungible::Mutate,
        tokens::{
            Fortitude::Polite,
            Precision::Exact,
            Preservation::{Expendable, Preserve},
        },
    },
};
use sp_runtime::{DispatchError, TokenError};
use vitreus_runtime_common::{NativeEnergyExchange, QuotePriceFromEnergy, SwapFromEnergy};

/// Buys VNRG for VTRS through the energy broker.
pub type NativeToEnergyExchange = NativeEnergyExchange<EnergyBroker, NativeAsset, EnergyAsset>;

/// Sells SNRG for VTRS through the energy broker.
pub type StaticEnergyToNativeExchange =
    NativeEnergyExchange<EnergyBroker, NativeAsset, StaticEnergyAsset>;

/// Sells LNRG for VTRS through the energy broker.
pub type LiquidEnergyToNativeExchange =
    NativeEnergyExchange<EnergyBroker, NativeAsset, LiquidEnergyAsset>;

/// Sells VNRG for VTRS through the energy broker.
///
/// The broker has no VNRG → VTRS path, so the VNRG is converted 1:1 into LNRG and sold as LNRG.
/// A failed sale rolls the conversion back.
pub struct EnergyToNativeExchange;

impl QuotePriceFromEnergy for EnergyToNativeExchange {
    type Balance = Balance;
    type EnergyAsset = EnergyAsset;

    fn quote_price_exact_tokens_for_tokens(amount: Balance, include_fee: bool) -> Option<Balance> {
        LiquidEnergyToNativeExchange::quote_price_exact_tokens_for_tokens(amount, include_fee)
    }

    fn quote_price_tokens_for_exact_tokens(amount: Balance, include_fee: bool) -> Option<Balance> {
        LiquidEnergyToNativeExchange::quote_price_tokens_for_exact_tokens(amount, include_fee)
    }

    fn max_amount_out() -> Balance {
        LiquidEnergyToNativeExchange::max_amount_out()
    }
}

impl SwapFromEnergy<AccountId> for EnergyToNativeExchange {
    type Balance = Balance;
    type EnergyAsset = EnergyAsset;

    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Balance,
        keep_alive: bool,
    ) -> Result<Balance, DispatchError> {
        with_storage_layer(|| {
            let preservation = if keep_alive { Preserve } else { Expendable };
            EnergyAsset::burn_from(&who, amount_in, preservation, Exact, Polite)?;
            LiquidEnergyAsset::mint_into(&who, amount_in)?;

            // The LNRG only passes through `who`, so sell all of it regardless of `keep_alive`;
            // any LNRG `who` already had is left untouched.
            LiquidEnergyToNativeExchange::swap_exact_tokens_for_tokens(who, amount_in, false)
        })
    }

    /// Not supported: nothing sells VNRG for an exact amount of VTRS yet.
    fn swap_tokens_for_exact_tokens(
        _who: AccountId,
        _amount_out: Balance,
        _keep_alive: bool,
    ) -> Result<Balance, DispatchError> {
        Err(TokenError::Unsupported.into())
    }
}
