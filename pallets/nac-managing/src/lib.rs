//! This pallet holds the NAC - NFTs with granted access level of the user.
//! It uses `pallet_uniques` under the hood.
//!
//! It's supposed there is a single collection holding all the NACs. The level is a `u8` value
//! stored in the NAC's metadata.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use frame_support::{
    pallet_prelude::{BoundedVec, DispatchResult},
    traits::EnsureOriginWithArg,
};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use parity_scale_codec::Encode;
use sp_runtime::traits::{BlakeTwo256, Hash};
use sp_runtime::{traits::StaticLookup, traits::Zero};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

/// NAC level index in NFT metadata.
const NAC_LEVEL_INDEX: usize = 1;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_uniques::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The collection id type.
        type CollectionId: MaybeSerializeDeserialize
            + Parameter
            + Member
            + Copy
            + Default
            + Ord
            + Into<<Self as pallet_uniques::Config>::CollectionId>;

        /// The item id type.
        type ItemId: Member
            + Parameter
            + MaxEncodedLen
            + Copy
            + From<u64>
            + Into<<Self as pallet_uniques::Config>::ItemId>;

        /// The origin which may forcibly create or destroy an item or otherwise alter privileged
        /// attributes.
        type ForceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Standard collection creation is only allowed if the origin attempting it and the
        /// collection are in this set.
        type CreateOrigin: EnsureOriginWithArg<
            Self::RuntimeOrigin,
            <Self as Config>::CollectionId,
            Success = Self::AccountId,
        >;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;
    }

    /// The information about user NFTs and NAC levels.
    #[pallet::storage]
    pub type UsersNft<T> = StorageMap<
        _,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        (<T as Config>::ItemId, u8),
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An item was minted.
        NacLevelSet {
            /// Who gets the NAC
            owner: T::AccountId,
            /// The NAC unique ID
            item_id: <T as Config>::ItemId,
            /// The access level
            verification_level: u8,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The user already has a NFT.
        TokenAlreadyExists,
        /// The user hasn't permissions to transaction in EVM.
        NoPermissions,
        /// Invalid metadata passed
        InvalidMetadata,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create a collection.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_collection())]
        pub fn create_collection(
            origin: OriginFor<T>,
            collection: <T as Config>::CollectionId,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            <T as Config>::ForceOrigin::ensure_origin(origin)?;

            let owner = T::Lookup::lookup(owner)?;

            Self::do_create_collection(collection, owner.clone(), owner)
        }

        /// Mint NAC.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            collection: <T as Config>::CollectionId,
            data: BoundedVec<u8, T::StringLimit>,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            let owner = T::Lookup::lookup(owner)?;

            let item_id = Self::create_unique_item_id(123, &owner);

            Self::do_mint(origin, collection, item_id, data, owner)
        }
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// The accounts, who get NACs with values as the second field.
        pub accounts: Vec<(T::AccountId, u8)>,
        /// The initial collections. The first field is the collection ID and the second is the
        /// owner ID.
        pub collections: Vec<(<T as Config>::CollectionId, T::AccountId)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (collection, owner) in self.collections.iter() {
                Pallet::<T>::do_create_collection(*collection, owner.clone(), owner.clone())
                    .expect("Cannot create collection");

                for (n, (account, level)) in self.accounts.iter().enumerate() {
                    let metadata = BoundedVec::<u8, T::StringLimit>::try_from(vec![0, *level, 0])
                        .expect("Cannot initialize metadata");
                    Pallet::<T>::do_mint(
                        frame_system::RawOrigin::Signed(owner.clone()).into(),
                        *collection,
                        (n as u64).into(),
                        metadata,
                        account.clone(),
                    )
                    .expect("Cannot mint NAC");
                }
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Non-dispatchable `Self::creat_collection`.
    pub fn do_create_collection(
        collection: <T as Config>::CollectionId,
        owner: T::AccountId,
        issuer: T::AccountId,
    ) -> DispatchResult {
        let deposit_info = (Zero::zero(), true);

        let collection = collection.into();

        pallet_uniques::Pallet::<T>::do_create_collection(
            collection.clone(),
            owner.clone(),
            issuer.clone(),
            deposit_info.0,
            deposit_info.1,
            pallet_uniques::Event::Created { collection, creator: owner, owner: issuer },
        )
    }

    /// Non-dispatchable `Self::mint`.
    pub fn do_mint(
        origin: OriginFor<T>,
        collection: <T as Config>::CollectionId,
        item: <T as Config>::ItemId,
        data: BoundedVec<u8, T::StringLimit>,
        owner: T::AccountId,
    ) -> DispatchResult {
        pallet_uniques::Pallet::<T>::do_mint(
            collection.into().clone(),
            item.into(),
            owner.clone(),
            |_details| Ok(()),
        )?;

        let is_frozen = true;

        pallet_uniques::Pallet::<T>::set_metadata(
            origin,
            collection.into(),
            item.into(),
            data.clone(),
            is_frozen,
        )?;

        let verification_level: u8 =
            *data.get(NAC_LEVEL_INDEX).ok_or(Error::<T>::InvalidMetadata)?;
        UsersNft::<T>::insert(&owner, (&item, &verification_level));

        Self::deposit_event(Event::NacLevelSet { owner, item_id: item, verification_level });

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

        let hash = BlakeTwo256::hash(&unique_number);
        let mut item_id: u64 = 0;
        for i in 0..8 {
            item_id |= (hash[i] as u64) << (i * 8);
        }

        <T as Config>::ItemId::from(item_id)
    }

    /// Check whether the account has the level.
    pub fn user_has_access(account_id: T::AccountId, desired_access_level: u8) -> bool {
        match UsersNft::<T>::get(account_id) {
            Some(nft) => nft.1 >= desired_access_level,
            None => false,
        }
    }
}
