#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v1::{
    account, benchmarks_instance_pallet, whitelist_account, whitelisted_caller, BenchmarkError,
};
use frame_support::{
    dispatch::UnfilteredDispatchable,
    traits::{EnsureOrigin, Get},
    BoundedVec,
};
use frame_system::RawOrigin as SystemOrigin;
use sp_runtime::traits::Bounded;
use sp_std::prelude::*;

use crate::Pallet as Nac;

const SEED: u32 = 0;

fn create_collection<T: Config<I>, I: 'static>(
) -> (T::CollectionId, T::AccountId, AccountIdLookupOf<T>) {
    let caller: T::AccountId = whitelisted_caller();
    let caller_lookup = T::Lookup::unlookup(caller.clone());
    let collection = T::Helper::collection(0);

    assert!(Nac::<T, I>::create_collection(
        SystemOrigin::Root.into(),
        collection.clone(),
        caller_lookup.clone()
    ).is_ok());

    (collection, caller, caller_lookup)
}

fn mint_item<T: Config<I>, I: 'static>(
    index: u16,
) -> (T::ItemId, T::AccountId, AccountIdLookupOf<T>) {
    let caller = Collection::<T, I>::get(T::Helper::collection(0)).unwrap().admin;
    if caller != whitelisted_caller() {
        whitelist_account!(caller);
    }
    let caller_lookup = T::Lookup::unlookup(caller.clone());
    let item = T::Helper::item(index);
    let nac_level = 1_u8;
    assert!(Uniques::<T, I>::mint(
        SystemOrigin::Signed(caller.clone()).into(),
        T::Helper::collection(0),
        item,
        nac_level,
        None,
        caller_lookup.clone(),
    ).is_ok());
    (item, caller, caller_lookup)
}