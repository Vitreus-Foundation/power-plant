//! Access to the stake that the pallet holds in [`Config::StakeCurrency`].

use frame_support::traits::{
    fungible::{BalancedHold, Inspect, InspectHold, MutateHold},
    tokens::{Fortitude, Precision, Preservation},
};
use sp_runtime::DispatchResult;

use crate::{slashing::NegativeImbalanceOf, Config, HoldReason, StakeOf};

/// The balance of `who` that is on hold as stake.
pub fn staked<T: Config>(who: &T::AccountId) -> StakeOf<T> {
    T::StakeCurrency::balance_on_hold(&HoldReason::Staking.into(), who)
}

/// The balance of `who` that can be added to its stake.
///
/// The existential deposit stays free.
pub fn free_to_stake<T: Config>(who: &T::AccountId) -> StakeOf<T> {
    // Frozen funds can be staked: a freeze limits the total balance, and a hold doesn't reduce
    // the total.
    T::StakeCurrency::reducible_balance(who, Preservation::Preserve, Fortitude::Force)
}

/// Sets the stake of `who` to `amount`, holding more of its balance or releasing the difference.
pub fn update_stake<T: Config>(who: &T::AccountId, amount: StakeOf<T>) -> DispatchResult {
    T::StakeCurrency::set_on_hold(&HoldReason::Staking.into(), who, amount)
}

/// Releases the whole stake of `who`.
pub fn kill_stake<T: Config>(who: &T::AccountId) -> DispatchResult {
    T::StakeCurrency::release_all(&HoldReason::Staking.into(), who, Precision::BestEffort)
        .map(|_| ())
}

/// Slashes up to `value` from the stake of `who`.
///
/// Returns the slashed funds and the part of `value` that the stake didn't cover.
pub fn slash<T: Config>(
    who: &T::AccountId,
    value: StakeOf<T>,
) -> (NegativeImbalanceOf<T>, StakeOf<T>) {
    <T::StakeCurrency as BalancedHold<_>>::slash(&HoldReason::Staking.into(), who, value)
}
