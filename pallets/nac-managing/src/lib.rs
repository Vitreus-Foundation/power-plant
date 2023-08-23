#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{tokens::Locker, EnsureOriginWithArg};
use frame_system::Config as SystemConfig;
use parity_scale_codec::{Decode, Encode};
use sp_runtime::{traits::StaticLookup, ArithmeticError, RuntimeDebug};
use sp_std::prelude::*;

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

mod functions;
mod types;

pub mod weights;

/// A type alias for the account ID type used in the dispatchable functions of this pallet.
type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T, I = ()>(_);

    #[cfg(feature = "runtime-benchmarks")]
    pub trait BenchmarkHelper<CollectionId, ItemId> {
        fn collection(i: u16) -> CollectionId;
        fn item(i: u16) -> ItemId;
    }
    #[cfg(feature = "runtime-benchmarks")]
    impl<CollectionId: From<u16>, ItemId: From<u16>> BenchmarkHelper<CollectionId, ItemId> for () {
        fn collection(i: u16) -> CollectionId {
            i.into()
        }
        fn item(i: u16) -> ItemId {
            i.into()
        }
    }

    #[pallet::config]
    /// The module configuration trait.
    pub trait Config<I: 'static = ()>: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Identifier for the collection of item.
        type CollectionId: Member + Parameter + MaxEncodedLen;

        /// The type used to identify a unique item within a collection.
        type ItemId: Member + Parameter + MaxEncodedLen + Copy;

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

        /// Locker trait to enable Locking mechanism downstream.
        type Locker: Locker<Self::CollectionId, Self::ItemId>;

        /// The maximum length of data stored on-chain.
        #[pallet::constant]
        type StringLimit: Get<u32>;

        #[cfg(feature = "runtime-benchmarks")]
        /// A set of helper functions for benchmarking.
        type Helper: BenchmarkHelper<Self::CollectionId, Self::ItemId>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    /// Details of a collection.
    pub(super) type Collection<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::CollectionId, CollectionDetails<T::AccountId>>;

    #[pallet::storage]
    /// The items held by any given account; set out this way so that items owned by a single
    /// account can be enumerated.
    pub(super) type Account<T: Config<I>, I: 'static = ()> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Blake2_128Concat, T::CollectionId>,
            NMapKey<Blake2_128Concat, T::ItemId>,
        ),
        (),
        OptionQuery,
    >;

    #[pallet::storage]
    pub(super) type Item<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::CollectionId,
        Blake2_128Concat,
        T::ItemId,
        ItemDetails<T::AccountId>,
        OptionQuery,
    >;

    #[pallet::storage]
    pub(super) type ItemMetadataOf<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::CollectionId,
        Blake2_128Concat,
        T::ItemId,
        ItemMetadata<T::StringLimit>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// A `collection` was created.
        CollectionCreated { collection: T::CollectionId, owner: T::AccountId },
        /// An `item` was issued.
        Issued { collection: T::CollectionId, item: T::ItemId, owner: T::AccountId },
        /// Check NAC level
        FoundNacLevel { data: BoundedVec<u8, T::StringLimit>, nac: u8 },
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// The signing account has no permission to do the operation.
        NoPermission,
        /// The given item ID is unknown.
        UnknownCollection,
        /// The item ID has already been used for an item.
        AlreadyExists,
        /// The owner turned out to be different to what was expected.
        WrongOwner,
        /// The item ID is already taken.
        InUse,
        /// The collection ID is already taken.
        CollectionIdInUse,
        /// The given item ID is unknown.
        UnknownItem,
    }

    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Get the owner of the item, if the item exists.
        pub fn owner(collection: T::CollectionId, item: T::ItemId) -> Option<T::AccountId> {
            Item::<T, I>::get(collection, item).map(|i| i.owner)
        }

        /// Get the owner of the collection, if the item exists.
        pub fn collection_owner(collection: T::CollectionId) -> Option<T::AccountId> {
            Collection::<T, I>::get(collection).map(|i| i.owner)
        }
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_collection())]
        pub fn create_collection(
            origin: OriginFor<T>,
            collection: T::CollectionId,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            T::ForceOrigin::ensure_origin(origin)?;
            let owner = T::Lookup::lookup(owner)?;

            Self::do_create_collection(
                collection.clone(),
                owner.clone(),
                owner.clone(),
                Event::CollectionCreated { collection, owner },
            )
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            collection: T::CollectionId,
            item: T::ItemId,
            nac_level: u8,
            data: BoundedVec<u8, T::StringLimit>,
            owner: AccountIdLookupOf<T>,
        ) -> DispatchResult {
            let origin = ensure_signed(origin)?;
            let owner = T::Lookup::lookup(owner)?;

            Self::do_mint(collection, item, nac_level, data, owner, |collection_details| {
                ensure!(collection_details.issuer == origin, Error::<T, I>::NoPermission);
                Ok(())
            })
        }

        #[pallet::call_index(2)]
        #[pallet::weight({0})]
        pub fn check_levels(origin: OriginFor<T>, acc_id: T::AccountId) -> DispatchResult {
            ensure_signed(origin)?;
            Self::check_level(&acc_id)
        }
    }
}
