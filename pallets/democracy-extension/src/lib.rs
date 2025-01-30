#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{Currency, LockableCurrency, ReservableCurrency};
use pallet_democracy::{ReferendumIndex, ReferendumInfo, ReferendumInfoOf, VoteThreshold};
use sp_runtime::{
    traits::{IntegerSquareRoot, One},
    FixedPointNumber, FixedU128, Percent, Saturating,
};

pub use pallet::*;

mod impls;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

type CurrencyOf<T> = <T as Config>::Currency;
type BalanceOf<T> = <CurrencyOf<T> as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_democracy::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The origin which can manage parameters of this pallet.
        type ManageOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Currency type for this pallet.
        type Currency: ReservableCurrency<Self::AccountId>
            + LockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The electorate value was set.
        ElectorateSet { value: BalanceOf<T> },
    }

    #[pallet::storage]
    #[pallet::getter(fn electorate)]
    pub type Electorate<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set the electorate value.
        /// If the value is zero, `Currency::total_issuance()` will be used as the electorate.
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_electorate(origin: OriginFor<T>, value: BalanceOf<T>) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            Electorate::<T>::put(value);
            Self::deposit_event(Event::ElectorateSet { value });

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    pub fn threshold(ref_index: ReferendumIndex) -> Option<Percent> {
        let status = ReferendumInfoOf::<T>::get(ref_index).and_then(|info| match info {
            ReferendumInfo::Ongoing(status) => Some(status),
            ReferendumInfo::Finished { .. } => None,
        })?;

        let turnout = FixedU128::checked_from_rational(
            status.tally.turnout.integer_sqrt(),
            <T as pallet_democracy::Config>::Currency::total_issuance().integer_sqrt(),
        )?;

        let threshold = match status.threshold {
            VoteThreshold::SuperMajorityApprove => FixedU128::one() / turnout.saturating_plus_one(),
            VoteThreshold::SuperMajorityAgainst => turnout / turnout.saturating_plus_one(),
            VoteThreshold::SimpleMajority => FixedU128::from_rational(1, 2),
        };

        Some(Percent::from_rational(threshold.into_inner(), FixedU128::DIV))
    }
}
