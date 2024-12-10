// This file is part of Substrate.

// Copyright (C) 2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Claims Pallet
//!
//! A secure token claiming system that enables users to claim tokens using Ethereum-style signatures.
//!
//! ## Overview
//! This pallet enables:
//! - Claiming tokens with cryptographic proof of ownership
//! - Vesting schedule support for claimed tokens
//! - Root-level management of claimable token supply
//! - Integration with existing balance and vesting systems
//!
//! ## Security Model
//! - Uses secp256k1 ECDSA for signature verification
//! - Single-use claims that are removed after successful processing
//! - Protected token supply managed only by root
//! - Vesting protection preventing claiming to accounts that already have schedules
//!
//! ## Components
//! - `claim`: Main extrinsic for users to claim tokens with an Ethereum signature
//! - `mint_tokens_to_claim`: Root operation to add tokens to the claiming pool
//! - `mint_claim`: Root operation to create new claims
//! - Claims are stored in a map of Ethereum addresses to balances
//! - Optional vesting schedules can be configured per claim
//!
//! ## Usage
//! 1. Root mints tokens to claims pool
//! 2. Root creates claims for Ethereum addresses
//! 3. Users sign messages with Ethereum keys
//! 4. Users submit signatures to claim tokens
//! 5. Optional vesting schedules are automatically applied

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::type_complexity)]

use crate::weights::WeightInfo;
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::AllowDeath, VestingSchedule},
    DefaultNoBound, PalletId,
};
use polkadot_primitives::ValidityError;
use serde::{self, Deserialize, Deserializer, Serialize, Serializer};
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
use sp_runtime::traits::{AccountIdConversion, CheckedSub, Saturating};
use sp_std::{vec, vec::Vec};

#[cfg(not(feature = "std"))]
use sp_std::alloc::{format, string::String};

pub use pallet::*;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

/// Pallet ID.
const PALLET_ID: PalletId = PalletId(*b"Claiming");

type CurrencyOf<T, I> = <<T as Config<I>>::VestingSchedule as VestingSchedule<
    <T as frame_system::Config>::AccountId,
>>::Currency;

type BalanceOf<T, I> =
    <CurrencyOf<T, I> as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Handler for when a claim is made.
pub trait OnClaimHandler<AccountId, Balance, ClaimData> {
    /// Handle a claim.
    fn on_claim(who: &AccountId, amount: Balance, data: Option<ClaimData>) -> DispatchResult;
}

impl<AccountId, Balance, ClaimData> OnClaimHandler<AccountId, Balance, ClaimData> for () {
    fn on_claim(_who: &AccountId, _amount: Balance, _data: Option<ClaimData>) -> DispatchResult {
        Ok(())
    }
}

/// An Ethereum address (i.e. 20 bytes, used to represent an Ethereum account).
///
/// This gets serialized to the 0x-prefixed hex representation.
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct EthereumAddress(pub [u8; 20]);

impl sp_std::fmt::Display for EthereumAddress {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        let hex: String = rustc_hex::ToHex::to_hex(&self.0[..]);
        write!(f, "0x{}", hex)
    }
}

impl Serialize for EthereumAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex: String = rustc_hex::ToHex::to_hex(&self.0[..]);
        serializer.serialize_str(&format!("0x{}", hex))
    }
}

impl<'de> Deserialize<'de> for EthereumAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let base_string = String::deserialize(deserializer)?;
        let offset = if base_string.starts_with("0x") { 2 } else { 0 };
        let s = &base_string[offset..];
        if s.len() != 40 {
            Err(serde::de::Error::custom(
                "Bad length of Ethereum address (should be 42 including '0x')",
            ))?;
        }
        let raw: Vec<u8> = rustc_hex::FromHex::from_hex(s)
            .map_err(|e| serde::de::Error::custom(format!("{:?}", e)))?;
        let mut r = Self::default();
        r.0.copy_from_slice(&raw);
        Ok(r)
    }
}

/// An Ethereum signature
#[derive(Encode, Decode, Clone, TypeInfo)]
pub struct EcdsaSignature(pub [u8; 65]);

impl PartialEq for EcdsaSignature {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}

impl sp_std::fmt::Debug for EcdsaSignature {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
        write!(f, "EcdsaSignature({:?})", &self.0[..])
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config + pallet_balances::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency mechanism, used for VTRS claiming.
        type Currency: Currency<Self::AccountId>;

        /// The vesting schedule
        type VestingSchedule: VestingSchedule<Self::AccountId, Moment = BlockNumberFor<Self>>;

        /// Additional claim data.
        type ClaimData: Parameter;

        /// Handler for when a claim is made.
        type OnClaim: OnClaimHandler<Self::AccountId, BalanceOf<Self, I>, Self::ClaimData>;

        /// Ethereum message prefix
        #[pallet::constant]
        type Prefix: Get<&'static [u8]>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn claims)]
    pub(super) type Claims<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Identity, EthereumAddress, BalanceOf<T, I>>;

    /// Vesting schedule for a claim.
    /// First balance is the total amount that should be held for vesting.
    /// Second balance is how much should be unlocked per block.
    /// The block number is when the vesting should start.
    #[pallet::storage]
    #[pallet::getter(fn vesting)]
    pub(super) type Vesting<T: Config<I>, I: 'static = ()> = StorageMap<
        _,
        Identity,
        EthereumAddress,
        (BalanceOf<T, I>, BalanceOf<T, I>, BlockNumberFor<T>),
    >;

    /// Additional data for a claim.
    #[pallet::storage]
    pub(super) type ClaimsData<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Identity, EthereumAddress, T::ClaimData>;

    #[pallet::storage]
    #[pallet::getter(fn total)]
    pub(super) type Total<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BalanceOf<T, I>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// Tokens were claimed.
        Claimed {
            /// To whom the tokens were claimed.
            account_id: T::AccountId,
            /// Amount to claim.
            amount: BalanceOf<T, I>,
        },

        /// Tokens were minted to claim.
        TokenMintedToClaim(BalanceOf<T, I>),
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// Error indicating insufficient VTRS for a claim.
        NotEnoughTokensForClaim,
        /// Invalid Ethereum signature.
        InvalidEthereumSignature,
        /// Ethereum address has no claim.
        SignerHasNoClaim,
        /// The account already has an existing claim with a vesting schedule.
        DuplicateVestingSchedule,
    }

    #[pallet::genesis_config]
    #[derive(DefaultNoBound)]
    pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
        /// Claims
        pub claims: Vec<(EthereumAddress, BalanceOf<T, I>)>,
        /// Vesting schedule for claims
        pub vesting: Vec<(EthereumAddress, (BalanceOf<T, I>, BalanceOf<T, I>, BlockNumberFor<T>))>,
    }

    #[pallet::genesis_build]
    impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
        fn build(&self) {
            self.claims.iter().for_each(|(address, amount)| {
                assert!(
                    !Claims::<T, I>::contains_key(address),
                    "duplicate claims in genesis: {}",
                    String::from_utf8(to_ascii_hex(&address.0)).unwrap()
                );
                Claims::<T, I>::insert(address, amount);
            });
            self.vesting.iter().for_each(|(k, v)| {
                Vesting::<T, I>::insert(k, v);
            });

            <Total<T, I>>::put(CurrencyOf::<T, I>::free_balance(
                &Pallet::<T, I>::claim_account_id(),
            ));
        }
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Make a claim to collect your reward.
        ///
        /// The dispatch origin for this call must be _None_.
        ///
        /// Unsigned Validation:
        /// A call to claim is deemed valid if the signature provided matches
        /// the expected signed message of:
        ///
        /// > Ethereum Signed Message:
        /// > (configured prefix string)(address)
        ///
        /// and `address` matches the `dest` account.
        ///
        /// Parameters:
        /// - `dest`: The destination account to payout the claim.
        /// - `ethereum_signature`: The signature of an ethereum signed message matching the format
        ///   described above.
        ///
        /// <weight>
        /// The weight of this call is invariant over the input parameters.
        /// Weight includes logic to validate unsigned `claim` call.
        ///
        /// Total Complexity: O(1)
        /// </weight>
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::claim())]
        pub fn claim(
            origin: OriginFor<T>,
            dest: T::AccountId,
            ethereum_signature: EcdsaSignature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let data = dest.using_encoded(to_ascii_hex);
            let signer = Self::eth_recover(&ethereum_signature, &data, &[][..])
                .ok_or(Error::<T, I>::InvalidEthereumSignature)?;

            Self::process_claim(signer, dest)?;

            Ok(())
        }

        /// Mint tokens to the claim account for future claims.
        ///
        /// The dispatch origin for this call must be _Root_.
        ///
        /// This function adds the specified amount of VTRS tokens to the claim account,
        /// increasing the total pool of tokens available for claims.
        ///
        /// Parameters:
        /// - `amount`: The amount of VTRS tokens to be added to the claim account.
        ///
        /// Emits:
        /// - `TokenMintedToClaim`: Upon successfully minting the tokens to the claim account.
        ///
        /// <weight>
        /// The weight of this call is invariant over the input parameters.
        /// Total Complexity: O(1)
        /// </weight>
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::mint_tokens_to_claim())]
        pub fn mint_tokens_to_claim(
            origin: OriginFor<T>,
            amount: BalanceOf<T, I>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            CurrencyOf::<T, I>::deposit_creating(&Self::claim_account_id(), amount);

            <Total<T, I>>::mutate(|value| *value += amount);
            Self::deposit_event(Event::<T, I>::TokenMintedToClaim(amount));

            Ok(())
        }

        /// Mint a new claim to collect VTRS tokens.
        ///
        /// The dispatch origin for this call must be _Root_.
        ///
        /// Parameters:
        /// - `who`: The Ethereum address eligible to collect this claim.
        /// - `value`: The amount of VTRS tokens that will be claimable.
        /// - `vesting_schedule`: An optional vesting schedule for these tokens,
        ///   consisting of:
        ///   - `BalanceOf<T>`: Total amount to be vested.
        ///   - `BalanceOf<T>`: Per-block unlock amount.
        ///   - `BlockNumberFor<T>`: The starting block of the vesting period.
        /// - `data`: Optional information assigned to this claim.
        ///
        /// <weight>
        /// The weight of this call is invariant over the input parameters.
        /// We assume the worst case where both vesting and claim data are being inserted.
        ///
        /// Total Complexity: O(1)
        /// </weight>
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::mint_claim())]
        pub fn mint_claim(
            origin: OriginFor<T>,
            who: EthereumAddress,
            value: BalanceOf<T, I>,
            vesting_schedule: Option<(BalanceOf<T, I>, BalanceOf<T, I>, BlockNumberFor<T>)>,
            data: Option<T::ClaimData>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Update the claims storage to include the new value.
            <Claims<T, I>>::mutate(who, |amount| {
                *amount = Some(amount.unwrap_or_default().saturating_add(value))
            });

            // Insert the vesting schedule if provided.
            if let Some(vs) = vesting_schedule {
                ensure!(
                    !<Vesting<T, I>>::contains_key(who),
                    Error::<T, I>::DuplicateVestingSchedule
                );

                <Vesting<T, I>>::insert(who, vs);
            }

            // Set the additional claim data.
            <ClaimsData<T, I>>::set(who, data);

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config<I>, I: 'static> ValidateUnsigned for Pallet<T, I> {
        type Call = Call<T, I>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            const PRIORITY: u64 = 100;

            let maybe_signer = match call {
                Call::claim { dest, ethereum_signature } => {
                    let data = dest.using_encoded(to_ascii_hex);
                    Self::eth_recover(ethereum_signature, &data, &[][..])
                },
                _ => return Err(InvalidTransaction::Call.into()),
            };

            let signer = maybe_signer.ok_or(InvalidTransaction::Custom(
                ValidityError::InvalidEthereumSignature.into(),
            ))?;

            ensure!(
                Claims::<T, I>::contains_key(signer),
                InvalidTransaction::Custom(ValidityError::SignerHasNoClaim.into())
            );

            Ok(ValidTransaction {
                priority: PRIORITY,
                requires: vec![],
                provides: vec![("claiming", signer).encode()],
                longevity: TransactionLongevity::max_value(),
                propagate: true,
            })
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    /// The account ID that holds the VTRS to claim.
    pub fn claim_account_id() -> T::AccountId {
        PALLET_ID.into_account_truncating()
    }

    /// Claims tokens to account wallet.
    fn process_claim(signer: EthereumAddress, dest: T::AccountId) -> DispatchResult {
        let amount = <Claims<T, I>>::get(signer).ok_or(Error::<T, I>::SignerHasNoClaim)?;

        let new_total = Self::total()
            .checked_sub(&amount)
            .ok_or(Error::<T, I>::NotEnoughTokensForClaim)?;

        CurrencyOf::<T, I>::transfer(&Self::claim_account_id(), &dest, amount, AllowDeath)?;

        // Check if this claim should have a vesting schedule.
        if let Some(vs) = Vesting::<T, I>::get(signer) {
            T::VestingSchedule::add_vesting_schedule(&dest, vs.0, vs.1, vs.2)?;
        }

        let data = ClaimsData::<T, I>::take(signer);
        T::OnClaim::on_claim(&dest, amount, data)?;

        <Total<T, I>>::put(new_total);
        <Claims<T, I>>::remove(signer);
        <Vesting<T, I>>::remove(signer);

        Self::deposit_event(Event::<T, I>::Claimed { account_id: dest, amount });

        Ok(())
    }

    /// Constructs the message that Ethereum RPC's `personal_sign` and `eth_sign` would sign.
    fn ethereum_signable_message(what: &[u8], extra: &[u8]) -> Vec<u8> {
        let prefix = T::Prefix::get();
        let mut l = prefix.len() + what.len() + extra.len();
        let mut rev = Vec::new();
        while l > 0 {
            rev.push(b'0' + (l % 10) as u8);
            l /= 10;
        }
        let mut v = b"\x19Ethereum Signed Message:\n".to_vec();
        v.extend(rev.into_iter().rev());
        v.extend_from_slice(prefix);
        v.extend_from_slice(what);
        v.extend_from_slice(extra);
        v
    }

    /// Attempts to recover the Ethereum address from a message signature signed by using
    /// the Ethereum RPC's `personal_sign` and `eth_sign`.
    fn eth_recover(s: &EcdsaSignature, what: &[u8], extra: &[u8]) -> Option<EthereumAddress> {
        let msg = keccak_256(&Self::ethereum_signable_message(what, extra));
        let mut res = EthereumAddress::default();
        res.0
            .copy_from_slice(&keccak_256(&secp256k1_ecdsa_recover(&s.0, &msg).ok()?[..])[12..]);
        Some(res)
    }
}

/// Converts the given binary data into ASCII-encoded hex. It will be twice the length.
fn to_ascii_hex(data: &[u8]) -> Vec<u8> {
    let mut r = Vec::with_capacity(data.len() * 2);
    let mut push_nibble = |n| r.push(if n < 10 { b'0' + n } else { b'a' - 10 + n });
    for &b in data.iter() {
        push_nibble(b / 16);
        push_nibble(b % 16);
    }
    r
}

#[cfg(test)]
mod secp_utils {
    use super::*;

    pub fn public(secret: &libsecp256k1::SecretKey) -> libsecp256k1::PublicKey {
        libsecp256k1::PublicKey::from_secret_key(secret)
    }

    pub fn eth(secret: &libsecp256k1::SecretKey) -> EthereumAddress {
        let mut res = EthereumAddress::default();
        res.0.copy_from_slice(&keccak_256(&public(secret).serialize()[1..65])[12..]);
        res
    }

    pub fn sig<T: Config>(
        secret: &libsecp256k1::SecretKey,
        what: &[u8],
        extra: &[u8],
    ) -> EcdsaSignature {
        let msg = keccak_256(&<super::Pallet<T>>::ethereum_signable_message(
            &to_ascii_hex(what)[..],
            extra,
        ));
        let (sig, recovery_id) = libsecp256k1::sign(&libsecp256k1::Message::parse(&msg), secret);
        let mut r = [0u8; 65];
        r[0..64].copy_from_slice(&sig.serialize()[..]);
        r[64] = recovery_id.serialize();
        EcdsaSignature(r)
    }
}
