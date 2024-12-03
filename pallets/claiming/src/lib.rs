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
    traits::{
        tokens::nonfungibles_v2::{Inspect, InspectEnumerable, Mutate},
        Currency,
        ExistenceRequirement::AllowDeath,
        VestingSchedule,
    },
    DefaultNoBound, PalletId,
};
use pallet_nfts::{ItemConfig, ItemSettings};
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

/// NFT level attribute key.
const NFT_LEVEL_ATTRIBUTE_KEY: [u8; 3] = [0, 0, 1];

type CurrencyOf<T> = <<T as Config>::VestingSchedule as VestingSchedule<
    <T as frame_system::Config>::AccountId,
>>::Currency;
type BalanceOf<T> = <CurrencyOf<T> as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type CollectionIdOf<T> =
    <<T as Config>::Nfts as Inspect<<T as frame_system::Config>::AccountId>>::CollectionId;
type ItemIdOf<T> = <<T as Config>::Nfts as Inspect<<T as frame_system::Config>::AccountId>>::ItemId;

/// Handler for when a claim is made.
pub trait OnClaimHandler<AccountId, Balance> {
    /// Handle a claim.
    fn on_claim(who: &AccountId, amount: Balance) -> DispatchResult;
}

impl<AccountId, Balance> OnClaimHandler<AccountId, Balance> for () {
    fn on_claim(_who: &AccountId, _amount: Balance) -> DispatchResult {
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
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Registry for the minted NFTs.
        type Nfts: Inspect<Self::AccountId>
            + Mutate<Self::AccountId, ItemConfig>
            + InspectEnumerable<Self::AccountId>;

        /// The currency mechanism, used for VTRS claiming.
        type Currency: Currency<Self::AccountId>;

        /// The vesting schedule
        type VestingSchedule: VestingSchedule<Self::AccountId, Moment = BlockNumberFor<Self>>;

        /// Handler for when a claim is made.
        type OnClaim: OnClaimHandler<Self::AccountId, BalanceOf<Self>>;

        /// Ethereum message prefix
        #[pallet::constant]
        type Prefix: Get<&'static [u8]>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn claims)]
    pub(super) type Claims<T: Config> = StorageMap<_, Identity, EthereumAddress, BalanceOf<T>>;

    /// Vesting schedule for a claim.
    /// First balance is the total amount that should be held for vesting.
    /// Second balance is how much should be unlocked per block.
    /// The block number is when the vesting should start.
    #[pallet::storage]
    #[pallet::getter(fn vesting)]
    pub(super) type Vesting<T: Config> =
        StorageMap<_, Identity, EthereumAddress, (BalanceOf<T>, BalanceOf<T>, BlockNumberFor<T>)>;

    /// Storage map for NFTs that can be claimed by users.
    /// This maps an Ethereum address and a Collection ID to an NFT represented by its Item ID and level.
    ///
    /// Each entry consists of:
    /// - `(EthereumAddress, CollectionIdOf<T>)`: A tuple containing the Ethereum address
    ///   eligible to claim the NFT and the ID of the NFT collection.
    /// - `(ItemIdOf<T>, u32)`: A tuple containing the Item ID of the NFT and its level,
    ///   where the level represents its rarity or attributes.
    ///
    /// This storage allows associating a specific Ethereum address and NFT collection
    /// with a particular NFT and its properties.
    #[pallet::storage]
    #[pallet::getter(fn nfts)]
    pub(super) type Nfts<T: Config> = StorageDoubleMap<
        _,
        Identity,
        EthereumAddress,
        Identity,
        CollectionIdOf<T>,
        (ItemIdOf<T>, u32),
    >;

    #[pallet::storage]
    #[pallet::getter(fn total)]
    pub(super) type Total<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Tokens were claimed.
        Claimed {
            /// To whom the tokens were claimed.
            account_id: T::AccountId,
            /// Amount to claim.
            amount: BalanceOf<T>,
        },

        /// Tokens were minted to claim.
        TokenMintedToClaim(BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
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
    pub struct GenesisConfig<T: Config> {
        /// Claims
        pub claims: Vec<(EthereumAddress, BalanceOf<T>)>,
        /// Vesting schedule for claims
        pub vesting: Vec<(EthereumAddress, (BalanceOf<T>, BalanceOf<T>, BlockNumberFor<T>))>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            self.claims.iter().for_each(|(address, amount)| {
                assert!(
                    !Claims::<T>::contains_key(address),
                    "duplicate claims in genesis: {}",
                    String::from_utf8(to_ascii_hex(&address.0)).unwrap()
                );
                Claims::<T>::insert(address, amount);
            });
            self.vesting.iter().for_each(|(k, v)| {
                Vesting::<T>::insert(k, v);
            });

            <Total<T>>::put(CurrencyOf::<T>::free_balance(&Pallet::<T>::claim_account_id()));
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Make a claim to collect your VTRS and NFTs.
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
        #[pallet::weight(<T as Config>::WeightInfo::claim())]
        pub fn claim(
            origin: OriginFor<T>,
            dest: T::AccountId,
            ethereum_signature: EcdsaSignature,
        ) -> DispatchResult {
            ensure_none(origin)?;

            let data = dest.using_encoded(to_ascii_hex);
            let signer = Self::eth_recover(&ethereum_signature, &data, &[][..])
                .ok_or(Error::<T>::InvalidEthereumSignature)?;

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
        #[pallet::weight(<T as Config>::WeightInfo::mint_tokens_to_claim())]
        pub fn mint_tokens_to_claim(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;

            CurrencyOf::<T>::deposit_creating(&Self::claim_account_id(), amount);

            <Total<T>>::mutate(|value| *value += amount);
            Self::deposit_event(Event::<T>::TokenMintedToClaim(amount));

            Ok(())
        }

        /// Mint a new claim to collect VTRS tokens and optionally assign an NFT with its collection, ID, and level.
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
        /// - `nft_info`: Optional information about an NFT to be assigned to this claim:
        ///   - `CollectionIdOf<T>`: The ID of the NFT collection.
        ///   - `ItemIdOf<T>`: The unique ID of the NFT item.
        ///   - `u32`: The level of the NFT, representing its attributes or rarity.
        ///
        /// <weight>
        /// The weight of this call is invariant over the input parameters.
        /// We assume the worst case where both vesting and NFT information are being inserted.
        ///
        /// Total Complexity: O(1)
        /// </weight>
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::mint_claim())]
        pub fn mint_claim(
            origin: OriginFor<T>,
            who: EthereumAddress,
            value: BalanceOf<T>,
            vesting_schedule: Option<(BalanceOf<T>, BalanceOf<T>, BlockNumberFor<T>)>,
            nft_info: Option<(CollectionIdOf<T>, ItemIdOf<T>, u32)>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Update the claims storage to include the new value.
            <Claims<T>>::mutate(who, |amount| {
                *amount = Some(amount.unwrap_or_default().saturating_add(value))
            });

            // Insert the vesting schedule if provided.
            if let Some(vs) = vesting_schedule {
                ensure!(!<Vesting<T>>::contains_key(who), Error::<T>::DuplicateVestingSchedule);

                <Vesting<T>>::insert(who, vs);
            }

            // Insert the NFT information if provided.
            if let Some((collection_id, item_id, level)) = nft_info {
                <Nfts<T>>::insert(who, collection_id, (item_id, level));
            }

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

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
                Claims::<T>::contains_key(signer),
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

impl<T: Config> Pallet<T> {
    /// The account ID that holds the VTRS to claim.
    pub fn claim_account_id() -> T::AccountId {
        PALLET_ID.into_account_truncating()
    }

    /// Claims tokens to account wallet.
    fn process_claim(signer: EthereumAddress, dest: T::AccountId) -> DispatchResult {
        let amount = <Claims<T>>::get(signer).ok_or(Error::<T>::SignerHasNoClaim)?;

        let new_total =
            Self::total().checked_sub(&amount).ok_or(Error::<T>::NotEnoughTokensForClaim)?;

        CurrencyOf::<T>::transfer(&Self::claim_account_id(), &dest, amount, AllowDeath)?;

        T::OnClaim::on_claim(&dest, amount)?;

        // Check if this claim should have a vesting schedule.
        if let Some(vs) = Vesting::<T>::get(signer) {
            T::VestingSchedule::add_vesting_schedule(&dest, vs.0, vs.1, vs.2)?;
        }

        // Mint NFT if it exists.
        for (collection_id, (item_id, level)) in Nfts::<T>::iter_prefix(signer) {
            Self::do_mint_nft(&collection_id, &item_id, &level, &dest)?;
        }

        <Total<T>>::put(new_total);
        <Claims<T>>::remove(signer);
        <Vesting<T>>::remove(signer);

        let _ = <Nfts<T>>::clear_prefix(signer, u32::MAX, None);

        Self::deposit_event(Event::<T>::Claimed { account_id: dest, amount });

        Ok(())
    }

    /// Mint user NFT.
    pub fn do_mint_nft(
        collection_id: &CollectionIdOf<T>,
        item_id: &ItemIdOf<T>,
        level: &u32,
        owner: &T::AccountId,
    ) -> DispatchResult {
        let item_config = ItemConfig { settings: ItemSettings::all_enabled() };

        T::Nfts::mint_into(collection_id, item_id, owner, &item_config, true)?;
        T::Nfts::set_attribute(
            collection_id,
            item_id,
            &Vec::from(NFT_LEVEL_ATTRIBUTE_KEY),
            &level.to_le_bytes(),
        )?;

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

/// Migrations
pub mod migrations {
    use super::*;
    use frame_support::traits::OnRuntimeUpgrade;

    #[cfg(feature = "try-runtime")]
    use sp_runtime::{traits::Zero, Saturating, TryRuntimeError};

    /// Tranfers a claim and vesting schedule from `Source` to `Destination`.
    pub struct TransferClaim<T, Source, Destination>(PhantomData<(T, Source, Destination)>);

    impl<T, Source, Destination> OnRuntimeUpgrade for TransferClaim<T, Source, Destination>
    where
        T: Config,
        Source: Get<EthereumAddress>,
        Destination: Get<EthereumAddress>,
    {
        fn on_runtime_upgrade() -> Weight {
            let source = Source::get();
            let destination = Destination::get();

            if !<Claims<T>>::contains_key(destination) {
                if !<Vesting<T>>::contains_key(destination) {
                    if let Some(amount) = <Claims<T>>::take(source) {
                        <Claims<T>>::insert(destination, amount);
                        log::info!("Transfer claim from {source} to {destination}");

                        if let Some(vesting) = <Vesting<T>>::take(source) {
                            <Vesting<T>>::insert(destination, vesting);
                            log::info!("Transfer vesting schedule from {source} to {destination}");
                        }
                    }
                } else {
                    // is that possible?
                    log::warn!("Address {destination} has vesting schedule without a claim, skip migration");
                }
            } else {
                log::info!("Address {destination} already has a claim, skip migration");
            }

            T::DbWeight::get().reads_writes(4, 4)
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
            let total =
                <Claims<T>>::iter_values().fold(BalanceOf::<T>::zero(), |a, i| a.saturating_add(i));
            Ok(total.encode())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
            let old_total: BalanceOf<T> =
                Decode::decode(&mut &state[..]).expect("pre_upgrade provides a valid state; qed");

            let new_total =
                <Claims<T>>::iter_values().fold(BalanceOf::<T>::zero(), |a, i| a.saturating_add(i));

            ensure!(new_total == old_total, "Total balance of claims should not change");
            Ok(())
        }
    }
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
