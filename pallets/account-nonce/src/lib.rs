//! The pallet provides a temporary account nonce management solution.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use sp_runtime::traits::{Saturating};
use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};

pub use pallet::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::storage]
    #[pallet::getter(fn something)]
    pub type AccountNonce<T> = StorageMap<
        _,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        <T as frame_system::Config>::Nonce,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        SomethingDoing(u32, T::AccountId),
        NonceChanged(T::AccountId, T::Nonce),
        ExistingNonce(T::AccountId, T::Nonce),
    }

    #[pallet::error]
    pub enum Error<T> {
        NoneValue,
        NonceDoesntExist,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn increment(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let sender = ensure_signed(origin)?;

            let current_value = AccountNonce::<T>::get(&sender).unwrap();
            let new_value = current_value.saturating_add(T::Nonce::from(1_u8));

            AccountNonce::<T>::insert(sender,  new_value);

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(0)]
        pub fn edit_nonce(origin: OriginFor<T>, nonce: <T as frame_system::Config>::Nonce) -> DispatchResultWithPostInfo {

            let who = ensure_signed(origin)?;

            AccountNonce::<T>::try_mutate_exists(&who, |acc_nonce| {
                match acc_nonce {
                    Some(nonce_value) => {
                        *nonce_value = nonce.clone();
                        Ok(())
                    },
                    None => Err(Error::<T>::NonceDoesntExist),
                }
            })?;

            Self::deposit_event(Event::NonceChanged(who, nonce));

            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(0)]
        pub fn get_nonce(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let nonce = AccountNonce::<T>::get(&who)
                .ok_or_else(|| Error::<T>::NonceDoesntExist)?;

            Self::deposit_event(Event::ExistingNonce(who, nonce));

            Ok(().into())
        }
    }
}
