//! The pallet provides a simple faucet. The users can ask for funds using their own account only.
//! The origin should be signed.
//!
//! The user can request only `Config::MaxAmount` per 24 hours.
//!
//! This pallet is only for test networks.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]
#![warn(missing_docs)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Currency;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The maximum amount of funds a user can request per once or in total per 24 hours.
        #[pallet::constant]
        type MaxAmount: Get<Self::Balance>;
        /// The period during which the user can't request more than `Config::MaxAmount`.
        #[pallet::constant]
        type AccumulationPeriod: Get<BlockNumberFor<Self>>;
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Type representing the weight of this pallet
        type WeightInfo: WeightInfo;
    }

    /// Requests done by users.
    #[pallet::storage]
    #[pallet::getter(fn requests)]
    pub type Requests<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::Balance, BlockNumberFor<T>), ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The requested funds sent to `who`. [who, amount]
        FundsSent {
            /// The account ID reaceved the funds.
            who: T::AccountId,
            /// The amount of funds.
            amount: T::Balance,
        },
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// Request amount more than `Config::MaxAmount`.
        AmountTooHigh,
        /// More than allowed funds requested during `Config::AccumulationPeriod`.
        RequestLimitExceeded,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Requset some funds.
        #[pallet::call_index(0)]
        #[pallet::weight(
            (<T as Config>::WeightInfo::request_funds(), DispatchClass::Normal, Pays::No)
            )]
        pub fn request_funds(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(amount <= T::MaxAmount::get(), Error::<T>::AmountTooHigh);

            let (balance, timestamp) = Requests::<T>::get(&who);
            let now = frame_system::Pallet::<T>::block_number();
            let period = now - timestamp;

            let (total, now) = if period >= T::AccumulationPeriod::get() {
                (amount, now)
            } else {
                (balance + amount, timestamp)
            };

            ensure!(total <= T::MaxAmount::get(), Error::<T>::RequestLimitExceeded);

            let _ = pallet_balances::Pallet::<T>::deposit_creating(&who, amount);

            Requests::<T>::insert(&who, (total, now));

            Self::deposit_event(Event::FundsSent { who, amount });

            Ok(())
        }
    }
}
