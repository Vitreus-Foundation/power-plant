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
    traits::{Currency, Incrementable, Get, tokens::nonfungibles_v2::Inspect},
};
use frame_system::pallet_prelude::{OriginFor, BlockNumberFor};
pub use pallet::*;
use parity_scale_codec::Encode;
use sp_runtime::traits::{BlakeTwo256, Hash, StaticLookup};
use sp_std::prelude::*;
use pallet_nfts::{CollectionSetting, CollectionConfig, CollectionSettings, MintSettings, ItemConfig, ItemSettings};
pub use weights::WeightInfo;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

type BalanceOf<T, I = ()> = <<T as pallet_nfts::Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;
type CollectionConfigFor<T, I = ()> = CollectionConfig<BalanceOf<T, I>, BlockNumberFor<T>, <T as pallet_nfts::Config<I>>::CollectionId>;

/// NAC level attribute key in NFT.
const NAC_LEVEL_ATTRIBUTE_KEY: [u8; 3] = [0, 0, 1];

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_nfts::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Registry for the minted NFTs.
        type Nfts: Inspect<
                Self::AccountId,
                ItemId = <Self as Config>::ItemId,
                CollectionId = <Self as Config>::CollectionId,
            >;

        /// The collection id type.
        type CollectionId: MaybeSerializeDeserialize
                + Parameter
                + Member
                + Copy
                + Default
                + Ord
                + From<<Self as pallet_nfts::Config>::CollectionId>
                + Into<<Self as pallet_nfts::Config>::CollectionId>;

        /// The item id type.
        type ItemId: Member
                + Parameter
                + MaxEncodedLen
                + Copy
                + From<u32>
                + From<<Self as pallet_nfts::Config>::ItemId>
                + Into<<Self as pallet_nfts::Config>::ItemId>;

        /// The origin which may forcibly mint a NFT or otherwise alter privileged
        /// attributes.
        type ForceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;

        /// NFT Collection ID.
        type NFTCollectionId: Get<<Self as Config>::CollectionId>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An item was minted.
        NFTMinted {
            /// Who gets the NAC.
            owner: AccountIdLookupOf<T>,
            /// The NAC unique ID.
            item_id: <T as Config>::ItemId,
        },

        /// User has NAC level.
        UserNacLevel {
            /// NAC level owner.
            owner: T::AccountId,
            /// NAC level value.
            nac_level: u8,
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// NFT wasn't found.
        NftNotFound,
        /// NFT already exist.
        NftAlreadyExist,
    }

    #[pallet::call]

    impl<T: Config> Pallet<T> {
        /// Mint a NFT item of a particular collection.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            data: BoundedVec<u8, T::StringLimit>,
            nac_level: u8,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            let account_id = T::Lookup::lookup(owner.clone())?;
            let item_id = Self::create_unique_item_id(135, &account_id);
            let collection = T::NFTCollectionId::get();

            match Self::get_nac_level(&account_id) {
                Some(_) => return Err(Error::<T>::NftAlreadyExist)?,
                None => (),
            };

            Self::update_nft_info(origin, collection, item_id, data, nac_level, owner)
        }

        /// Update metadata and NAC level.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::update_nft())]
        pub fn update_nft(
            origin: OriginFor<T>,
            new_data: BoundedVec<u8, T::StringLimit>,
            new_nac_level: Option<u8>,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            let account_id = T::Lookup::lookup(owner.clone())?;
            let collection = T::NFTCollectionId::get();

            let mut new_nac_level = new_nac_level.unwrap_or_default();

            let item_id = match Self::get_nac_level(&account_id) {
                Some(value) => {
                    if value.0 == 0 {
                        new_nac_level = value.0;
                    };

                    value.1
                },
                None => return Err(Error::<T>::NftNotFound)?,
            };

            Self::update_nft_info(origin, collection, item_id, new_data, new_nac_level, owner)
        }

        /// Check NAC level status.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::check_nac_level())]
        pub fn check_nac_level(
            origin: OriginFor<T>,
            account_id: T::AccountId,
        ) -> DispatchResult {
            <T as Config>::ForceOrigin::ensure_origin(origin)?;

            let nac_level = Self::get_nac_level(&account_id).ok_or(Error::<T>::NftNotFound)?.0;

            Self::deposit_event(Event::UserNacLevel { nac_level, owner: account_id });

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
                let collection_settings = Pallet::<T>::collection_config_from_disabled_settings();

                pallet_nfts::Pallet::<T>::force_create(
                    frame_system::RawOrigin::Root.into(),
                    T::Lookup::unlookup(owner.clone()),
                    collection_settings
                )
                    .expect("Cannot create collection");

                let collection_id: <T as Config>::CollectionId = <T as pallet_nfts::Config>::CollectionId::initial_value().into();

                for (n, (account, level)) in self.accounts.iter().enumerate() {
                    let metadata = BoundedVec::<u8, T::StringLimit>::try_from(vec![0, *level, 0])
                        .expect("Cannot initialize metadata");
                    Pallet::<T>::update_nft_info(
                        frame_system::RawOrigin::Signed(owner.clone()).into(),
                        collection_id,
                        (n as u32).into(),
                        metadata,
                        *level,
                        T::Lookup::unlookup(account.clone()),
                    )
                    .expect("Cannot mint NAC");
                }
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Mint or update user NFT.
    pub fn update_nft_info(
        origin: OriginFor<T>,
        collection: <T as Config>::CollectionId,
        item: <T as Config>::ItemId,
        data: BoundedVec<u8, T::StringLimit>,
        nac_level: u8,
        owner: AccountIdLookupOf<T>,
    ) -> DispatchResult {
        let item_config = ItemConfig { settings: ItemSettings::all_enabled() };

        pallet_nfts::Pallet::<T>::force_mint(
            origin.clone(),
            collection.into(),
            item.into(),
            owner.clone(),
            item_config,
        )?;

        pallet_nfts::Pallet::<T>::set_metadata(
            origin.clone(),
            collection.into(),
            item.into(),
            data.clone(),
        )?;

        let key = BoundedVec::<u8, T::KeyLimit>::try_from(Vec::from(NAC_LEVEL_ATTRIBUTE_KEY)).unwrap_or_default();
        let mut nac = BoundedVec::<u8, T::ValueLimit>::new();
        let _ = nac.try_push(nac_level);

        pallet_nfts::Pallet::<T>::set_attribute(
            origin,
            collection.into(),
            Some(item.into()),
            pallet_nfts::AttributeNamespace::CollectionOwner,
            key,
            nac,
        )?;

        Self::deposit_event(Event::NFTMinted { owner, item_id: item });

        Ok(())
    }

    /// Generate uniq ItemId using block_number, token_owner and extrinsic_index
    pub fn create_unique_item_id(
        extrinsic_index: u32,
        owner: &T::AccountId,
    ) -> <T as Config>::ItemId {
        let block_number = frame_system::Pallet::<T>::block_number();
        let mut unique_number = Vec::new();

        unique_number.extend_from_slice(&block_number.encode());
        unique_number.extend_from_slice(&extrinsic_index.to_le_bytes());
        unique_number.extend_from_slice(owner.encode().as_ref());

        // Combine the bytes of the hash into a u32 by bitwise OR (|) and left shifts (<<).
        let hash = BlakeTwo256::hash(&unique_number);
        let mut item_id: u32 = 0;
        for i in 0..8 {
            item_id |= (hash[i] as u32) << (i * 8);
        }

        <T as Config>::ItemId::from(item_id)
    }

    /// Create a new collection
    pub fn collection_config_from_disabled_settings() -> CollectionConfigFor<T> {
        CollectionConfig {
            settings: CollectionSettings::from_disabled(CollectionSetting::DepositRequired.into()),
            max_supply: None,
            mint_settings: MintSettings::default(),
        }
    }

    /// Check whether the account has the level.
    pub fn user_has_access(account_id: T::AccountId, desired_access_level: u8) -> bool {
        return match Self::get_nac_level(&account_id) {
            Some(value) => value.0 >= desired_access_level,
            None => false,
        }
    }

    /// Get NAC level.
    pub fn get_nac_level(account_id: &T::AccountId) -> Option<(u8, <T as Config>::ItemId)> {
        let collection_id = T::NFTCollectionId::get();

        if let Some((keys, _)) = pallet_nfts::Account::<T>::iter_prefix((&account_id, &collection_id.into())).next() {
            let item_id = keys;

            let nac_level = T::Nfts::attribute(&collection_id, &item_id.into(), &NAC_LEVEL_ATTRIBUTE_KEY);

            return match nac_level {
                Some(bytes) => {
                    return Some((bytes[0], item_id.into()))
                }
                None => None,
            }
        }

        return None;
    }
}
