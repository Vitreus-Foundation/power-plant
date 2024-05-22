//! This pallet holds the NAC - NFTs with granted access level of the user.
//! It uses `pallet_nfts` under the hood.
//!
//! It's supposed there is a single collection holding all the NACs. The level is a `u8` value
//! stored in the NAC's metadata and in the NFT's attribute.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use frame_support::{
    pallet_prelude::{BoundedVec, DispatchResult},
    traits::{
        tokens::{
            nonfungibles_v2::{Create, Inspect, InspectEnumerable, Mutate},
            Balance,
        },
        Get, Incrementable, OnNewAccount, Currency,
    },
};
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
pub use pallet::*;
use pallet_claiming::OnClaimHandler;
use pallet_nfts::{CollectionConfig, CollectionSettings, ItemConfig, ItemSettings, MintSettings};
use pallet_reputation::{AccountReputation, ReputationPoint, ReputationRecord, ReputationTier};
use parity_scale_codec::{Encode, MaxEncodedLen, Codec, Decode};
use sp_arithmetic::{FixedPointOperand, Perbill};
use sp_runtime::{
    traits::{BlakeTwo256, Hash, MaybeSerializeDeserialize},
    SaturatedConversion,
};
use sp_std::fmt::Debug;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

type CollectionConfigFor<T> =
    CollectionConfig<<T as Config>::Balance, BlockNumberFor<T>, <T as Config>::CollectionId>;

/// NAC level attribute key in NFT.
const NAC_LEVEL_ATTRIBUTE_KEY: [u8; 3] = [0, 0, 1];

/// Claimed amount attribute key in NFT.
const CLAIM_AMOUNT_ATTRIBUTE_KEY: [u8; 3] = [0, 0, 2];

/// Did the account have VIPP status.
const VIPP_STATUS_EXIST: [u8; 3] = [0, 0, 3];

/// Default NAC level for account.
const DEFAULT_NAC_LEVEL: u8 = 1;

/// Extrinsic index.
const EXTRINSIC_INDEX: u32 = 135;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::LockableCurrency;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_reputation::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Registry for the minted NFTs.
        type Nfts: Inspect<Self::AccountId, ItemId = Self::ItemId, CollectionId = Self::CollectionId>
            + Mutate<Self::AccountId, ItemConfig>
            + Create<Self::AccountId, CollectionConfigFor<Self>>
            + InspectEnumerable<Self::AccountId>;

        /// The balance type.
        type Balance: Balance
            + MaybeSerializeDeserialize
            + Debug
            + Codec
            + MaxEncodedLen
            + FixedPointOperand;

        /// The collection id type.
        type CollectionId: MaybeSerializeDeserialize
            + Parameter
            + Member
            + Copy
            + Default
            + Ord
            + Incrementable;

        /// The item id type.
        type ItemId: Member + Parameter + MaxEncodedLen + Copy + From<u32>;

        /// The maximum number of bytes that may be used to represent an NFT attribute key.
        type KeyLimit: Get<u32>;

        /// The maximum number of bytes that may be used to represent an NFT attribute value.
        type ValueLimit: Get<u32>;

        /// The origin which may forcibly mint a NFT or otherwise alter privileged
        /// attributes.
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;

        /// The currency.
        type Currency: LockableCurrency<
            Self::AccountId,
            Moment = BlockNumberFor<Self>,
            Balance = Self::Balance,
        >;

        /// NFT Collection ID.
        type NftCollectionId: Get<Self::CollectionId>;

        /// VIPP NFT Collection ID.
        type VIPPCollectionId: Get<Self::CollectionId>;
    }

    /// Temp storage: the information about user NFTs and NAC levels.
    #[pallet::storage]
    pub type UsersNft<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::ItemId, u8), OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// NFT was minted.
        NftMinted {
            /// Who gets the NAC.
            owner: T::AccountId,
            /// The NAC unique ID.
            item_id: T::ItemId,
        },

        /// NFT metadata and attributes were updated.
        NftUpdated {
            /// Whose NFT.
            owner: T::AccountId,
            /// The NAC level.
            nac_level: u8,
        },

        /// User has NAC level.
        UserNacLevel {
            /// NAC level owner.
            owner: T::AccountId,
            /// NAC level value.
            nac_level: u8,
        },

        /// VIPP NFT was minted.
        VippNftMinted {
            /// Who gets the VIPP NFT.
            owner: T::AccountId,
            /// The VIPP NFT unique ID.
            item_id: T::ItemId,
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// NFT wasn't found.
        NftNotFound,
        /// NFT already exist.
        NftAlreadyExist,
        /// NAC level is not correct.
        NacLevelIsIncorrect,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Mint a NFT item of a particular collection.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(origin: OriginFor<T>, nac_level: u8, owner: T::AccountId) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            let collection = T::NftCollectionId::get();

            match Self::get_nac_level(&owner) {
                Some((current_nac_level, item_id)) => {
                    if current_nac_level == nac_level {
                        return Err(Error::<T>::NftAlreadyExist)?;
                    }

                    Self::update_nft_info(&collection, &item_id, nac_level, owner)
                },
                _ => {
                    let item_id = Self::create_unique_item_id(&owner);
                    Self::do_mint(item_id, owner.clone())?;
                    Self::update_nft_info(&collection, &item_id, nac_level, owner)
                },
            }
        }

        /// Update metadata and NAC level.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::update_nft())]
        pub fn update_nft(
            origin: OriginFor<T>,
            new_nac_level: Option<u8>,
            owner: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            let collection = T::NftCollectionId::get();
            let nac_level: u8;

            let item_id = match Self::get_nac_level(&owner) {
                Some(value) => {
                    // Checking whether the NAC level needs to be changed.
                    nac_level = new_nac_level.unwrap_or(value.0);
                    value.1
                },
                None => return Err(Error::<T>::NftNotFound)?,
            };

            Self::update_nft_info(&collection, &item_id, nac_level, owner)
        }

        /// Check NAC level by account_id.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::check_nac_level())]
        pub fn check_nac_level(origin: OriginFor<T>, owner: T::AccountId) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            let nac_level = Self::get_nac_level(&owner).ok_or(Error::<T>::NftNotFound)?.0;
            Self::deposit_event(Event::UserNacLevel { nac_level, owner });
            Ok(())
        }
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// The accounts, who get NACs with values as the second field.
        pub accounts: Vec<(T::AccountId, u8)>,
        /// The accounts, who are collection owners.
        pub owners: Vec<T::AccountId>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for owner in self.owners.iter() {
                Pallet::<T>::create_collection(owner).expect("Cannot create a collection");
                Pallet::<T>::create_collection(owner).expect("Cannot create a collection");

                // Get collection Id.
                let collection_id: T::CollectionId = T::CollectionId::initial_value();

                for (n, (account, level)) in self.accounts.iter().enumerate() {
                    Pallet::<T>::do_mint((n as u32).into(), account.clone())
                        .expect("Cannot mint NFT");

                    Pallet::<T>::update_nft_info(
                        &collection_id,
                        &(n as u32).into(),
                        *level,
                        account.clone(),
                    )
                    .expect("Cannot update NFT.");
                }
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Mint user NFT.
    pub fn do_mint(item_id: T::ItemId, owner: T::AccountId) -> DispatchResult {
        let item_config = ItemConfig { settings: ItemSettings::all_enabled() };
        let collection = T::NftCollectionId::get();

        T::Nfts::mint_into(&collection, &item_id, &owner, &item_config, true)?;

        pallet_reputation::Pallet::<T>::increase_creating(
            &owner,
            ReputationPoint::from(ReputationTier::Vanguard(1)),
        );

        Self::deposit_event(Event::NftMinted { owner, item_id });
        Ok(())
    }

    /// Update user NFT.
    pub fn update_nft_info(
        collection: &T::CollectionId,
        item: &T::ItemId,
        nac_level: u8,
        owner: T::AccountId,
    ) -> DispatchResult {
        let key = BoundedVec::<u8, T::KeyLimit>::try_from(Vec::from(NAC_LEVEL_ATTRIBUTE_KEY))
            .unwrap_or_default();
        let mut nac = BoundedVec::<u8, T::ValueLimit>::new();
        nac.try_push(nac_level).map_err(|_| Error::<T>::NacLevelIsIncorrect)?;

        T::Nfts::set_attribute(collection, item, &key, &nac)?;

        // Temporary solution to save NFT id and NAC level by user.
        UsersNft::<T>::insert(&owner, (&item, &nac_level));

        Self::deposit_event(Event::NftUpdated { owner, nac_level });

        Ok(())
    }

    /// Generate uniq ItemId using block_number, token_owner and extrinsic_index
    pub fn create_unique_item_id(owner: &T::AccountId) -> T::ItemId {
        let block_number = frame_system::Pallet::<T>::block_number();
        let mut unique_number = Vec::new();

        unique_number.extend_from_slice(&block_number.encode());
        unique_number.extend_from_slice(&EXTRINSIC_INDEX.to_le_bytes());
        unique_number.extend_from_slice(owner.encode().as_ref());

        // Combine the bytes of the hash into an u32 by bitwise OR (|) and left shifts (<<).
        let hash = BlakeTwo256::hash(&unique_number);
        let mut item_id: u32 = 0;
        for i in 0..4 {
            item_id |= (hash[i] as u32) << (i * 8);
        }

        T::ItemId::from(item_id)
    }

    /// Create a new collection.
    pub fn create_collection(owner: &T::AccountId) -> DispatchResult {
        let collection_config = CollectionConfig {
            settings: CollectionSettings::all_enabled(),
            max_supply: None,
            mint_settings: MintSettings::default(),
        };

        T::Nfts::create_collection(owner, owner, &collection_config).map(|_| ())
    }

    /// Check whether the account has the level.
    pub fn user_has_access(account_id: T::AccountId, desired_access_level: u8) -> bool {
        match Self::get_nac_level(&account_id) {
            Some(value) => value.0 >= desired_access_level,
            None => false,
        }
    }

    /// Get NAC level.
    pub fn get_nac_level(account_id: &T::AccountId) -> Option<(u8, <T as Config>::ItemId)> {
        let collection_id = T::NftCollectionId::get();

        if let Some(key) = T::Nfts::owned_in_collection(&collection_id, account_id).next() {
            let item_id = key;
            // Get NAC by NFT attribute key.
            let nac_level =
                T::Nfts::system_attribute(&collection_id, &item_id, &NAC_LEVEL_ATTRIBUTE_KEY);

            return match nac_level {
                Some(bytes) => Some((bytes[0], item_id)),
                None => Some((DEFAULT_NAC_LEVEL, item_id)),
            };
        }

        None
    }

    pub fn mint_vipp_nft(account: &T::AccountId) -> Option<(T::Balance, <T as Config>::ItemId)> {
        let claim_balance = Self::get_claim_balance(account);
        if let Some(claim_balance) = claim_balance {
            if Self::threshold_meets_vipp_requirements(account, claim_balance.0) {
                let item_id = Self::create_unique_item_id(account);
                let item_config = ItemConfig { settings: ItemSettings::all_enabled() };
                let collection = T::VIPPCollectionId::get();
                let perbill = Perbill::from_rational(95_u32, 100_u32);

                let result = T::Nfts::mint_into(&collection, &item_id, account, &item_config, true);

                if result.is_ok() {
                    Self::deposit_event(Event::VippNftMinted { owner: account.clone(), item_id });
                    return Some((perbill * claim_balance.0, item_id));
                }

                return None;
            }
        }

        None
    }

    /// can mint VIPP NFT to account.
    pub fn can_mint_vipp(account: &T::AccountId) -> Option<(T::Balance, <T as Config>::ItemId)> {
        let collection_id = T::NftCollectionId::get();
        if let Some(key) = T::Nfts::owned_in_collection(&collection_id, account).next() {
            let item_id = key;
            let vipp_status_exist =
                T::Nfts::system_attribute(&collection_id, &item_id, &VIPP_STATUS_EXIST);


            return match vipp_status_exist {
                Some(_) => { None },
                None => {
                    if Self::get_claim_balance(account).is_some() {
                        return Self::mint_vipp_nft(account);
                    }

                    None
                },
            };
        }

        None
    }

    /// Get user claim balance.
    pub fn get_claim_balance(account_id: &T::AccountId) -> Option<(T::Balance, <T as Config>::ItemId)> {
        let collection_id = T::NftCollectionId::get();

        if let Some(key) = T::Nfts::owned_in_collection(&collection_id, account_id).next() {
            let item_id = key;
            // Get NAC by NFT attribute key.
            let claim_balance =
                T::Nfts::system_attribute(&collection_id, &item_id, &CLAIM_AMOUNT_ATTRIBUTE_KEY);

            return match claim_balance {
                Some(bytes) => {
                    let balance = T::Balance::decode(&mut bytes.as_slice()).unwrap();
                    Some((balance, item_id)) },
                None => None,
            };
        }

        None
    }


    /// Check threshold of account.
    pub fn threshold_meets_vipp_requirements(account: &T::AccountId, claim_balance: T::Balance) -> bool {
        let free_balance = T::Currency::free_balance(account);
        let perbill = Perbill::from_rational(95_u32, 100_u32);

        if free_balance > perbill * claim_balance {
            return true
        }

        false
    }

    /// Check VIPP threshold every transaction.
    pub fn check_account_threshold(account: &T::AccountId) {
        let claim_balance= Self::get_claim_balance(account);

        // match claim_balance {
        //     Some(bytes) => {
        //         let balance = T::Balance::decode(&mut bytes.as_slice()).unwrap();
        //         if !Self::threshold_meets_vipp_requirements(account, balance) {
        //             Self::burn_vipp_nfts(account)
        //         }
        //     },
        //     None => return,
        // }
    }

    /// Burn VIPP status.
    pub fn burn_vipp_nfts(account: &T::AccountId) {

    }
}

impl<T: Config> OnNewAccount<T::AccountId> for Pallet<T> {
    fn on_new_account(who: &T::AccountId) {
        if AccountReputation::<T>::contains_key(who) {
            return;
        }
        // Add reputation points to account.
        let now = <frame_system::Pallet<T>>::block_number().saturated_into();
        let new_rep = ReputationRecord::with_blocknumber(now);
        AccountReputation::<T>::insert(who, new_rep);

        // Add default NAC NFT to account.
        let item_id = Self::create_unique_item_id(who);
        let nac_minting_result = Self::do_mint(item_id, who.clone());
        if nac_minting_result.is_ok() {
            UsersNft::<T>::insert(who, (&item_id, &DEFAULT_NAC_LEVEL));
        }
    }
}

impl<T: Config, Balance> OnClaimHandler<T::AccountId, Balance> for Pallet<T>
where
    Balance: frame_support::traits::tokens::Balance,
{
    fn on_claim(who: &T::AccountId, amount: Balance) -> DispatchResult {
        let collection = T::NftCollectionId::get();
        let item = T::Nfts::owned_in_collection(&collection, who)
            .next()
            .ok_or(Error::<T>::NftNotFound)?;

        let claimed_raw =
            T::Nfts::system_attribute(&collection, &item, &CLAIM_AMOUNT_ATTRIBUTE_KEY)
                .unwrap_or(vec![]);
        let currently_claimed =
            Balance::decode(&mut claimed_raw.as_slice()).unwrap_or(Balance::zero());

        let updated_claimed = currently_claimed.saturating_add(amount);

        T::Nfts::set_attribute(
            &collection,
            &item,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &updated_claimed.encode(),
        )
    }
}

