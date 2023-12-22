//! The pallet provides a simple claiming mechanism.
//! Allows claiming tokens immediately on the user's account without additional confirmations.
//! The origin should be signed.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use crate::weights::WeightInfo;
use frame_support::traits::Currency;
use frame_support::traits::ExistenceRequirement::AllowDeath;
use frame_support::{pallet_prelude::MaxEncodedLen, pallet_prelude::*, PalletId};
use scale_info::prelude::vec::Vec;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, CheckedSub};

pub use pallet::*;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

const PALLET_ID: PalletId = PalletId(*b"Claiming");
type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Information about tokens claiming.
#[derive(Clone, Eq, PartialEq, RuntimeDebugNoBound, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
#[codec(mel_bound())]
pub struct ClaimingInfo<T: Config> {
    /// Action of this claim.
    pub amount: BalanceOf<T>,
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
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency mechanism, used for VTRS claiming.
        type Currency: Currency<Self::AccountId>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn claims)]
    pub type Claims<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<ClaimingInfo<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn total)]
    pub(super) type Total<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Tokens was claimed.
        Claimed {
            /// To whom the tokens were claimed.
            account_id: T::AccountId,
            /// Amount to claim.
            amount: BalanceOf<T>,
        },

        /// Tokens was minted to claim.
        TokenMintedToClaim(BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Error indicating insufficient VTRS for a claim.
        NotEnoughTokensForClaim,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim tokens to user account.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::claim())]
        pub fn claim(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            amount: BalanceOf<T>,
            eth_hash: H256,
        ) -> DispatchResult {
            ensure_root(origin)?;

            let claim = ClaimingInfo::<T> { amount, eth_hash };
            Self::process_claim(claim, &account_id)?;

            Self::deposit_event(Event::<T>::Claimed { account_id, amount });
            Ok(())
        }

        /// Mint a new claim to collect VTRS.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::mint_tokens_to_claim())]
        pub fn mint_tokens_to_claim(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;

            T::Currency::deposit_creating(&Self::claim_account_id(), amount);

            <Total<T>>::mutate(|value| *value += amount);
            Self::deposit_event(Event::<T>::TokenMintedToClaim(amount));

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The account ID that holds the VTRS to claim.
    fn claim_account_id() -> T::AccountId {
        PALLET_ID.into_account_truncating()
    }

    /// Claims tokens to account wallet.
    fn process_claim(claim_info: ClaimingInfo<T>, to_acc: &T::AccountId) -> DispatchResult {
        let new_total = Self::total()
            .checked_sub(&claim_info.amount)
            .ok_or(Error::<T>::NotEnoughTokensForClaim)?;

        <T as Config>::Currency::transfer(
            &Self::claim_account_id(),
            to_acc,
            claim_info.amount,
            AllowDeath,
        )?;

        let mut claims = Claims::<T>::get(to_acc);
        claims.push(claim_info.clone());
        Claims::<T>::insert(to_acc, claims);
        <Total<T>>::put(new_total);

        Ok(())
    }
}
