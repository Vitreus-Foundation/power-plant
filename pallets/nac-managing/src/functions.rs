use super::*;
use frame_support::{ensure, BoundedVec};
use sp_runtime::DispatchResult;

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    pub fn do_create_collection(
        collection: T::CollectionId,
        owner: T::AccountId,
        issuer: T::AccountId,
        event: Event<T, I>,
    ) -> DispatchResult {
        ensure!(
            !Collection::<T, I>::contains_key(collection.clone()),
            Error::<T, I>::CollectionIdInUse
        );

        Collection::<T, I>::insert(
            collection.clone(),
            CollectionDetails {
                owner: owner.clone(),
                issuer,
                items: 0,
                item_metadatas: 0,
                is_frozen: true,
            },
        );

        Self::deposit_event(event);
        Ok(())
    }

    pub fn do_mint(
        collection: T::CollectionId,
        item: T::ItemId,
        nac_level: u8,
        data: BoundedVec<u8, T::StringLimit>,
        owner: T::AccountId,
        with_details: impl FnOnce(&CollectionDetailsFor<T>) -> DispatchResult,
    ) -> DispatchResult {
        ensure!(
            !Item::<T, I>::contains_key(collection.clone(), item),
            Error::<T, I>::AlreadyExists
        );

        Collection::<T, I>::try_mutate(
            &collection,
            |maybe_collection_details| -> DispatchResult {
                let collection_details =
                    maybe_collection_details.as_mut().ok_or(Error::<T, I>::UnknownCollection)?;

                with_details(collection_details)?;

                let items =
                    collection_details.items.checked_add(1).ok_or(ArithmeticError::Overflow)?;
                collection_details.items = items;

                let owner = owner.clone();
                Account::<T, I>::insert((&owner, &collection, &item), ());
                let details = ItemDetails { owner, approved: None, is_frozen: true };
                Item::<T, I>::insert(&collection, &item, details);

                let item_metadata = ItemMetadata { data, nac_level, is_frozen: true };
                ItemMetadataOf::<T, I>::insert(&collection, &item, item_metadata);

                Ok(())
            },
        )?;

        Self::deposit_event(Event::Issued { collection, item, owner });
        Ok(())
    }

    pub fn check_level(acc_id: &T::AccountId) -> DispatchResult {
        for ((account_id, collection_id, item_id), _) in Account::<T, I>::iter() {
            if &account_id == acc_id {
                match ItemMetadataOf::<T, I>::get(collection_id, item_id) {
                    Some(metadata) => Self::deposit_event(Event::FoundNacLevel {
                        data: metadata.data,
                        nac: metadata.nac_level,
                    }),
                    None => (),
                }
            }
        }
        Ok(())
    }
}
