use super::*;
use frame_support::{
    pallet_prelude::{BoundedVec, MaxEncodedLen},
    traits::Get,
};
use scale_info::TypeInfo;

/// A type alias representing the details of a collection.
pub(super) type CollectionDetailsFor<T> = CollectionDetails<<T as SystemConfig>::AccountId>;

/// Information concerning the ownership of a collection.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct CollectionDetails<AccountId> {
    /// Can change `issuer` account.
    pub(super) owner: AccountId,
    /// Can mint tokens.
    pub(super) issuer: AccountId,
    /// The total number of outstanding items of this collection.
    pub(super) items: u32,
    /// The total number of outstanding item metadata of this collection.
    pub(super) item_metadatas: u32,
    /// Whether the collection is frozen for non-admin transfers.
    pub(super) is_frozen: bool,
}

/// Information concerning the ownership of a single unique item.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]
pub struct ItemDetails<AccountId> {
    /// The owner of this item.
    pub(super) owner: AccountId,
    /// The approved transferrer of this item, if one is set.
    pub(super) approved: Option<AccountId>,
    /// Whether the item can be transferred or not.
    pub(super) is_frozen: bool,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(StringLimit))]
pub struct ItemMetadata<StringLimit: Get<u32>> {
    /// General information concerning this item. Limited in length by `StringLimit`.
    pub(super) data: BoundedVec<u8, StringLimit>,
    /// User NAC Level
    pub(super) nac_level: u8,
    /// Whether the item metadata may be changed by a non Force origin.
    pub(super) is_frozen: bool,
}
