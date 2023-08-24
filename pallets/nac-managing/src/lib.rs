#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::BoundedVec;
use frame_support::traits::EnsureOriginWithArg;
use sp_runtime::{traits::StaticLookup, traits::Zero};
use sp_std::prelude::*;
use frame_support::{ensure};
use frame_system::pallet_prelude::OriginFor;
use frame_support::pallet_prelude::DispatchResult;

pub use weights::WeightInfo;
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    /// The module configuration trait.
    pub trait Config: frame_system::Config + pallet_uniques::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The origin which may forcibly create or destroy an item or otherwise alter privileged
        /// attributes.
        type ForceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Standard collection creation is only allowed if the origin attempting it and the
        /// collection are in this set.
        type CreateOrigin: EnsureOriginWithArg<
            Self::RuntimeOrigin,
            Self::CollectionId,
            Success = Self::AccountId,
        >;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    pub type UsersNft<T> = StorageMap<_, Blake2_128Concat, <T as frame_system::Config>::AccountId, <T as pallet_uniques::Config>::ItemId, OptionQuery>;

    #[pallet::storage]
    pub type NftAccessLevels<T> = StorageMap<_, Blake2_128Concat, <T as pallet_uniques::Config>::ItemId, u8, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ///An item was minted
        ItemMinted {
            owner: T::AccountId,
            collection_id: T::CollectionId,
            item_id: T::ItemId,
            metadata: BoundedVec<u8, T::StringLimit>,
            verification_level: u8,
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The user already has a NFT
        TokenAlreadyExists,
    }


    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_collection())]
        pub fn create_collection(
            origin: OriginFor<T>,
            collection: T::CollectionId,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            <T as Config>::ForceOrigin::ensure_origin(origin)?;
            let owner = T::Lookup::lookup(owner)?;

            Self::do_create_collection(
                collection.clone(),
                owner.clone(),
                owner.clone()
            )?;

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            collection: T::CollectionId,
            item: T::ItemId,
            data: BoundedVec<u8, T::StringLimit>,
            verification_level: u8,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            let owner = T::Lookup::lookup(owner)?;

            Self::do_mint(origin, collection, item, data, verification_level, owner)?;

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn do_create_collection(
        collection: T::CollectionId,
        owner: T::AccountId,
        issuer: T::AccountId,
    ) -> DispatchResult {
        let deposit_info = (Zero::zero(), true);

        pallet_uniques::Pallet::<T>::do_create_collection(
            collection.clone(),
            owner.clone(),
            issuer.clone(),
            deposit_info.0,
            deposit_info.1,
            pallet_uniques::Event::Created {
                collection: collection.clone(),
                creator: owner.clone(),
                owner: issuer.clone()
            }
        )
    }

    fn do_mint(
        origin: OriginFor<T>,
        collection: T::CollectionId,
        item: T::ItemId,
        data: BoundedVec<u8, T::StringLimit>,
        verification_level: u8,
        owner: T::AccountId
    ) -> DispatchResult {
        ensure!(!UsersNft::<T>::get(&owner).is_some(), Error::<T>::TokenAlreadyExists);

        pallet_uniques::Pallet::<T>::do_mint(
            collection.clone(),
            item.into(),
            owner.clone(),
            |_details| Ok(())
        )?;

        let is_frozen = true;

        pallet_uniques::Pallet::<T>::set_metadata(
            origin,
            collection.clone(),
            item.clone(),
            data.clone(),
            is_frozen
        )?;

        UsersNft::<T>::insert(&owner, &item);
        NftAccessLevels::<T>::insert(&item, &verification_level);

        Self::deposit_event(Event::ItemMinted {
            owner,
            collection_id: collection,
            item_id: item,
            metadata: data,
            verification_level,
        });

        Ok(())
    }

    pub fn check_level_by_item_id(item_id: T::ItemId) -> u8 {
        NftAccessLevels::<T>::get(item_id).unwrap_or(0)
    }

    pub fn check_level_by_account_id(account_id: T::AccountId) -> u8 {
        let item_id = match UsersNft::<T>::get(account_id) {
            Some(item_id) => item_id,
            None => return 0,
        };

        NftAccessLevels::<T>::get(item_id).unwrap_or(0)
    }
}

