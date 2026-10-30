// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
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

//! Tests for Nac-managing pallet.

use crate::{mock::*, *};

use frame_support::{assert_err, assert_ok};
use parity_scale_codec::Decode;

type BalanceOf<Test> = <Test as pallet_balances::Config>::Balance;

const VANGUARD_1_REPUTATION_POINT: u64 = 7398066;

fn get_claimed(collection_id: CollectionId, item_id: ItemId) -> BalanceOf<Test> {
    let claimed_raw =
        Nfts::system_attribute(&collection_id, Some(&item_id), &CLAIM_AMOUNT_ATTRIBUTE_KEY)
            .unwrap_or_default();
    BalanceOf::<Test>::decode(&mut claimed_raw.as_slice()).unwrap_or(BalanceOf::<Test>::default())
}

#[test]
fn basic_minting_should_work() {
    new_test_ext().execute_with(|| {
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let account = 1_u64;

        assert_ok!(NacManaging::create_collection(&account));

        assert_ok!(NacManaging::do_mint(item_id, account));
        assert_ok!(NacManaging::update_nft_info(&collection_id, &item_id, nac_level, account));

        assert_eq!(NacManaging::get_nac_level(&account), Some((nac_level, 123)));
        assert_eq!(
            Reputation::reputation(1).unwrap().reputation.points().0,
            VANGUARD_1_REPUTATION_POINT
        );
    });
}

#[test]
fn new_account_gets_nac_with_free_item_id() {
    new_test_ext().execute_with(|| {
        let account = 3_u64;
        let collection = NftCollectionId::get();

        assert_ok!(NacManaging::create_collection(&1));
        let taken = NacManaging::create_unique_item_id(&collection, &account).expect("id is free");
        assert_ok!(NacManaging::do_mint(taken, 2));

        NacManaging::on_new_account(&account);

        let (item, _) = UsersNft::<Test>::get(account).expect("NAC is minted");
        assert_ne!(item, taken);
        assert_eq!(NacManaging::get_nac_level(&account), Some((DEFAULT_NAC_LEVEL, item)));
        System::assert_has_event(
            crate::Event::<Test>::NftMinted { owner: account, item_id: item }.into(),
        );
    });
}

#[test]
fn failed_nac_mint_for_new_account_is_reported() {
    new_test_ext().execute_with(|| {
        let account = 3_u64;

        // The NAC collection doesn't exist, so minting fails.
        NacManaging::on_new_account(&account);

        assert_eq!(UsersNft::<Test>::get(account), None);
        System::assert_has_event(
            crate::Event::<Test>::NftMintFailed {
                owner: account,
                error: pallet_nfts::Error::<Test>::UnknownCollection.into(),
            }
            .into(),
        );
    });
}

#[test]
fn nac_mint_fails_without_free_item_id() {
    new_test_ext().execute_with(|| {
        let account = 3_u64;
        let collection = NftCollectionId::get();

        assert_ok!(NacManaging::create_collection(&1));
        // Each call returns the next candidate, since the previous ones are taken.
        for _ in 0..MAX_ITEM_ID_ATTEMPTS {
            let item =
                NacManaging::create_unique_item_id(&collection, &account).expect("id is free");
            assert_ok!(NacManaging::do_mint(item, 2));
        }

        assert_err!(
            NacManaging::mint(RuntimeOrigin::root(), DEFAULT_NAC_LEVEL, account),
            Error::<Test>::NoFreeItemId
        );
    });
}

#[test]
fn update_nac_level_test() {
    new_test_ext().execute_with(|| {
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let account = 1_u64;

        assert_ok!(NacManaging::create_collection(&account));

        assert_ok!(NacManaging::do_mint(item_id, account));
        assert_eq!(
            Reputation::reputation(account).unwrap().reputation.points().0,
            VANGUARD_1_REPUTATION_POINT
        );
        assert_ok!(NacManaging::update_nft_info(&collection_id, &item_id, nac_level, account));

        assert_eq!(NacManaging::get_nac_level(&account), Some((nac_level, 123)));

        let new_nac_level = 10_u8;
        assert_ok!(NacManaging::update_nft(RuntimeOrigin::root(), Some(new_nac_level), account));
        assert_eq!(NacManaging::get_nac_level(&account), Some((new_nac_level, 123)));

        assert_ok!(NacManaging::update_nft(RuntimeOrigin::root(), None, account));
        assert_eq!(NacManaging::get_nac_level(&account), Some((new_nac_level, 123)));
        assert_eq!(
            Reputation::reputation(account).unwrap().reputation.points().0,
            VANGUARD_1_REPUTATION_POINT
        );
    });
}

#[test]
fn check_nac_level_test() {
    new_test_ext().execute_with(|| {
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let account = 1_u64;
        let second_account = 2_u64;

        assert_ok!(NacManaging::create_collection(&account));

        assert_ok!(NacManaging::do_mint(item_id, account));
        assert_ok!(NacManaging::update_nft_info(&collection_id, &item_id, nac_level, account));

        assert_ok!(NacManaging::check_nac_level(RuntimeOrigin::root(), account));
        assert_err!(
            NacManaging::check_nac_level(RuntimeOrigin::root(), second_account),
            Error::<Test>::NftNotFound
        );
    });
}

#[test]
fn user_has_access_test() {
    new_test_ext().execute_with(|| {
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let account = 1_u64;

        assert_ok!(NacManaging::create_collection(&account));

        assert_ok!(NacManaging::do_mint(item_id, account));
        assert_ok!(NacManaging::update_nft_info(&collection_id, &item_id, nac_level, account));

        assert!(NacManaging::user_has_access(account, 2));
        assert!(NacManaging::user_has_access(account, 5));
        assert!(!NacManaging::user_has_access(account, 6));
    });
}

#[test]
fn on_claim_should_work() {
    new_test_ext().execute_with(|| {
        let nac_level = 0_u8;
        let owner = 1_u64;
        let item_id = 123_u32;
        let collection_id = NftCollectionId::get();

        assert_ok!(NacManaging::create_collection(&owner));

        NacManaging::do_mint(item_id, owner).expect("Minting failed");
        NacManaging::update_nft_info(&collection_id, &item_id, nac_level, owner)
            .expect("Error updating nft info");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 0);

        NacManaging::on_claim(&owner, 1000_u64, None).expect("Error on claim");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 1000);

        NacManaging::on_claim(&owner, 1000_u64, None).expect("Error on claim");

        let new_claimed = get_claimed(collection_id, item_id);
        assert_eq!(new_claimed, claimed + 1000);
    });
}

#[test]
fn account_gets_two_vipp_nfts_in_one_block() {
    new_test_ext().execute_with(|| {
        let collection = VIPPCollectionId::get();
        let collection_owner = 1_u64;
        let account = 2_u64;

        assert_ok!(NacManaging::create_collection(&collection_owner));
        assert_ok!(NacManaging::create_collection(&collection_owner));
        // Creating the account mints its NAC, and the balance keeps it above the VIPP threshold.
        assert_ok!(Balances::force_set_balance(RuntimeOrigin::root(), account, 10_000));

        // The first claim only records the amount, each later claim mints a VIPP NFT.
        for _ in 0..3 {
            assert_ok!(NacManaging::on_claim(&account, 1000_u64, None));
        }

        let mut claims: Vec<BalanceOf<Test>> = Nfts::owned_in_collection(&collection, &account)
            .map(|item| get_claimed(collection, item))
            .collect();
        claims.sort();
        assert_eq!(claims, vec![1000, 2000]);
    });
}
