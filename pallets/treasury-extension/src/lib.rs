#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    pallet_prelude::*,
    traits::{Currency, Get, Imbalance, IsType, OnUnbalanced},
};
use pallet_treasury::{BalanceOf, NegativeImbalanceOf, PositiveImbalanceOf};
use sp_arithmetic::{traits::Saturating, Permill};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// Pallet which burns a fraction of the treasury's balance.
    /// In case if treasury didn't spend more than `T::SpendThreshold`, this pallet burns
    /// leftover funds.
    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_treasury::Config<I> {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Funds ratio to be recycled.
        type SpendThreshold: Get<Permill>;
        /// What to do with the recycled funds
        type OnRecycled: OnUnbalanced<NegativeImbalanceOf<Self, I>>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        Recycled { recyled_funds: BalanceOf<T, I> },
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    fn treasury_balance() -> BalanceOf<T, I> {
        pallet_treasury::Pallet::<T, I>::pot()
    }
}

impl<T: Config<I>, I: 'static> pallet_treasury::SpendFunds<T, I> for Pallet<T, I> {
    fn spend_funds(
        budget_remaining: &mut BalanceOf<T, I>,
        imbalance: &mut PositiveImbalanceOf<T, I>,
        total_weight: &mut Weight,
        missed_any: &mut bool,
    ) {
        // Just to make sure that treasury won't burn funds
        *missed_any = true;

        let fraction_for_recycle = T::SpendThreshold::get().mul_ceil(Self::treasury_balance());
        let imbalance_amount = imbalance.peek();

        // imbalance amount is greater than amount for recycle, no need to continue
        if fraction_for_recycle <= imbalance_amount {
            *total_weight += T::DbWeight::get().reads_writes(1, 0);
            return;
        }

        let unrecycled_amount = fraction_for_recycle.saturating_sub(imbalance_amount);
        let (debit, credit) = T::Currency::pair(unrecycled_amount);
        imbalance.subsume(debit);
        T::OnRecycled::on_unbalanced(credit);
        Self::deposit_event(Event::Recycled { recyled_funds: unrecycled_amount });

        *budget_remaining = budget_remaining.saturating_sub(unrecycled_amount);
        // TODO: add weight mutation
        // *total_weight += <T as pallet::Config<I>>::WeightInfo::spend_funds();
    }
}
