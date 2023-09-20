//! The pallet provides a simple claiming mechanism.
//! Allows claiming tokens immediately on the user's account without additional confirmations.
//! The origin should be signed.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use crate::weights::WeightInfo;
use frame_support::{pallet_prelude::MaxEncodedLen, pallet_prelude::*};
use pallet_atomic_swap::SwapAction;
use scale_info::prelude::vec::Vec;
use sp_core::H256;

pub use pallet::*;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

/// Information about tokens claiming.
#[derive(Clone, Eq, PartialEq, RuntimeDebugNoBound, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct ClaimingInfo<T: Config> {
    /// Action of this claim.
    pub action: T::SwapAction,
    /// Ethereum transaction hash.
    pub eth_hash: H256,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_atomic_swap::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    pub type Claims<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<ClaimingInfo<T>>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Tokens was claimed.
        Claimed {
            /// To whom the tokens were claimed.
            account_id: T::AccountId,
            /// Swap information.
            swap_info: ClaimingInfo<T>,
        },

        /// User claims information.
        ClaimsInfo {
            /// About whom the claims information was issued.
            account_id: T::AccountId,
            /// Information about claims: value and ethereum transaction hash.
            claims: Vec<ClaimingInfo<T>>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Error when claiming tokens for a user
        ClaimProcessError,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim tokens to user account.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::claim())]
        pub fn claim(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            action: T::SwapAction,
            eth_hash: H256,
        ) -> DispatchResult {
            let source = ensure_signed(origin)?;

            action.reserve(&source)?;

            let swap = ClaimingInfo::<T> { action, eth_hash };

            Self::process_claim(swap.clone(), &source, &account_id)?;

            Self::deposit_event(Event::<T>::Claimed { account_id, swap_info: swap });

            Ok(())
        }

        /// Get user claims information.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::get_claims_info())]
        pub fn get_claims_info(origin: OriginFor<T>, account_id: T::AccountId) -> DispatchResult {
            ensure_signed(origin)?;

            let claims = Claims::<T>::get(&account_id);

            Self::deposit_event(Event::<T>::ClaimsInfo { account_id, claims });

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Claims tokens to account wallet.
    fn process_claim(
        swap_info: ClaimingInfo<T>,
        source: &T::AccountId,
        to_acc: &T::AccountId,
    ) -> DispatchResult {
        if !swap_info.action.claim(source, to_acc) {
            return Err(Error::<T>::ClaimProcessError)?;
        }

        let mut claims = Claims::<T>::get(to_acc);
        claims.push(swap_info);
        Claims::<T>::insert(to_acc, claims);

        Ok(())
    }
}
