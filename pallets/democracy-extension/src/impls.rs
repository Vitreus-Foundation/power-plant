use super::*;
use frame_support::traits::{
    BalanceStatus, ExistenceRequirement, LockIdentifier, SignedImbalance, WithdrawReasons,
};
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};

impl<T: Config> Currency<T::AccountId> for Pallet<T> {
    type Balance = BalanceOf<T>;
    type PositiveImbalance = <CurrencyOf<T> as Currency<T::AccountId>>::PositiveImbalance;
    type NegativeImbalance = <CurrencyOf<T> as Currency<T::AccountId>>::NegativeImbalance;

    fn total_balance(who: &T::AccountId) -> Self::Balance {
        CurrencyOf::<T>::total_balance(who)
    }

    fn can_slash(who: &T::AccountId, value: Self::Balance) -> bool {
        CurrencyOf::<T>::can_slash(who, value)
    }

    fn total_issuance() -> Self::Balance {
        let amount = Self::electorate();
        if amount.is_zero() {
            return CurrencyOf::<T>::total_issuance();
        }

        amount
    }

    fn active_issuance() -> Self::Balance {
        CurrencyOf::<T>::active_issuance()
    }

    fn deactivate(amount: Self::Balance) {
        CurrencyOf::<T>::deactivate(amount)
    }

    fn reactivate(amount: Self::Balance) {
        CurrencyOf::<T>::reactivate(amount)
    }

    fn minimum_balance() -> Self::Balance {
        CurrencyOf::<T>::minimum_balance()
    }

    fn burn(amount: Self::Balance) -> Self::PositiveImbalance {
        CurrencyOf::<T>::burn(amount)
    }

    fn issue(amount: Self::Balance) -> Self::NegativeImbalance {
        CurrencyOf::<T>::issue(amount)
    }

    fn pair(amount: Self::Balance) -> (Self::PositiveImbalance, Self::NegativeImbalance) {
        CurrencyOf::<T>::pair(amount)
    }

    fn free_balance(who: &T::AccountId) -> Self::Balance {
        CurrencyOf::<T>::free_balance(who)
    }

    fn ensure_can_withdraw(
        who: &T::AccountId,
        amount: Self::Balance,
        reasons: WithdrawReasons,
        new_balance: Self::Balance,
    ) -> DispatchResult {
        CurrencyOf::<T>::ensure_can_withdraw(who, amount, reasons, new_balance)
    }

    fn transfer(
        source: &T::AccountId,
        dest: &T::AccountId,
        value: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        CurrencyOf::<T>::transfer(source, dest, value, existence_requirement)
    }

    fn slash(who: &T::AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        CurrencyOf::<T>::slash(who, value)
    }

    fn deposit_into_existing(
        who: &T::AccountId,
        value: Self::Balance,
    ) -> Result<Self::PositiveImbalance, DispatchError> {
        CurrencyOf::<T>::deposit_into_existing(who, value)
    }

    fn resolve_into_existing(
        who: &T::AccountId,
        value: Self::NegativeImbalance,
    ) -> Result<(), Self::NegativeImbalance> {
        CurrencyOf::<T>::resolve_into_existing(who, value)
    }

    fn deposit_creating(who: &T::AccountId, value: Self::Balance) -> Self::PositiveImbalance {
        CurrencyOf::<T>::deposit_creating(who, value)
    }

    fn resolve_creating(who: &T::AccountId, value: Self::NegativeImbalance) {
        CurrencyOf::<T>::resolve_creating(who, value)
    }

    fn withdraw(
        who: &T::AccountId,
        value: Self::Balance,
        reasons: WithdrawReasons,
        liveness: ExistenceRequirement,
    ) -> Result<Self::NegativeImbalance, DispatchError> {
        CurrencyOf::<T>::withdraw(who, value, reasons, liveness)
    }

    fn settle(
        who: &T::AccountId,
        value: Self::PositiveImbalance,
        reasons: WithdrawReasons,
        liveness: ExistenceRequirement,
    ) -> Result<(), Self::PositiveImbalance> {
        CurrencyOf::<T>::settle(who, value, reasons, liveness)
    }

    fn make_free_balance_be(
        who: &T::AccountId,
        balance: Self::Balance,
    ) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
        CurrencyOf::<T>::make_free_balance_be(who, balance)
    }
}

impl<T: Config> ReservableCurrency<T::AccountId> for Pallet<T> {
    fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
        CurrencyOf::<T>::can_reserve(who, value)
    }

    fn slash_reserved(
        who: &T::AccountId,
        value: Self::Balance,
    ) -> (Self::NegativeImbalance, Self::Balance) {
        CurrencyOf::<T>::slash_reserved(who, value)
    }

    fn reserved_balance(who: &T::AccountId) -> Self::Balance {
        CurrencyOf::<T>::reserved_balance(who)
    }

    fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
        CurrencyOf::<T>::reserve(who, value)
    }

    fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
        CurrencyOf::<T>::unreserve(who, value)
    }

    fn repatriate_reserved(
        slashed: &T::AccountId,
        beneficiary: &T::AccountId,
        value: Self::Balance,
        status: BalanceStatus,
    ) -> Result<Self::Balance, DispatchError> {
        CurrencyOf::<T>::repatriate_reserved(slashed, beneficiary, value, status)
    }
}

impl<T: Config> LockableCurrency<T::AccountId> for Pallet<T> {
    type Moment = <CurrencyOf<T> as LockableCurrency<T::AccountId>>::Moment;
    type MaxLocks = <CurrencyOf<T> as LockableCurrency<T::AccountId>>::MaxLocks;

    fn set_lock(
        id: LockIdentifier,
        who: &T::AccountId,
        amount: Self::Balance,
        reasons: WithdrawReasons,
    ) {
        CurrencyOf::<T>::set_lock(id, who, amount, reasons)
    }

    fn extend_lock(
        id: LockIdentifier,
        who: &T::AccountId,
        amount: Self::Balance,
        reasons: WithdrawReasons,
    ) {
        CurrencyOf::<T>::extend_lock(id, who, amount, reasons)
    }

    fn remove_lock(id: LockIdentifier, who: &T::AccountId) {
        CurrencyOf::<T>::remove_lock(id, who)
    }
}
