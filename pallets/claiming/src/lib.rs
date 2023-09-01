//! This pallet holds the NAC - NFTs with granted access level of the user.
//! It uses `pallet_uniques` under the hood.
//!
//! It's supposed there is a single collection holding all the NACs. The level is a `u8` value
//! stored in the NAC's metadata.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use frame_support::{
        dispatch::DispatchResultWithPostInfo,
        pallet_prelude::*,
        traits::{Currency, EnsureOrigin, ExistenceRequirement::KeepAlive, ReservableCurrency},
    };
use frame_system::ensure_root;
use crate::weights::WeightInfo;

pub use pallet::*;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The origin which can manage less critical staking parameters that does not require root.
        ///
        /// Supported action: (1) assign token amount.
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// The currency mechanism, used for amount transferring.
        type Currency: ReservableCurrency<Self::AccountId>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Tokens was claimed.
        Claimed {
            /// To whom the tokens were claimed.
            account_id: T::AccountId,
            /// Number of tokens.
            amount: <T as pallet_balances::Config>::Balance,
        },

        /// Tokens was assigned to account (by root).
        TokenAssigned {
            /// To whom the tokens were assigned.
            account_id: T::AccountId,
            /// Number of tokens.
            amount: <T as pallet_balances::Config>::Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Error when transferring tokens for a user
        ClaimProcessError,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Assign tokens to account.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::assign_token_amount())]
        pub fn assign_token_amount(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            T::AdminOrigin::try_origin(origin).map(|_| ()).or_else(ensure_root)?;

            let _ =
                <pallet_balances::Pallet<T> as Currency<_>>::deposit_creating(&account_id, amount);


            Self::deposit_event(Event::TokenAssigned { account_id, amount });

            Ok(().into())
        }

        /// Claim tokens to user account.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::claim())]
        pub fn claim(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            amount: T::Balance,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Self::process_claim(who, &account_id, amount)?;
            Self::deposit_event(Event::Claimed { account_id, amount });

            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Claims tokens to account wallet.
    fn process_claim(
        who: T::AccountId,
        account_id: &T::AccountId,
        amount: T::Balance,
    ) -> DispatchResult {
        <pallet_balances::Pallet<T> as Currency<_>>::transfer(
            &who,
            account_id,
            amount,
            KeepAlive,
        )?;

        Ok(())
    }
}

