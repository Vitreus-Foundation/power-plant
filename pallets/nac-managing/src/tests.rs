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
            .unwrap_or(vec![]);
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

        NacManaging::do_mint(item_id, owner.clone()).expect("Minting failed");
        NacManaging::update_nft_info(&collection_id, &item_id, nac_level, owner.clone())
            .expect("Error updating nft info");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 0);

        NacManaging::on_claim(&owner, 1000_u64).expect("Error on claim");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 1000);

        NacManaging::on_claim(&owner, 1000_u64).expect("Error on claim");

        let new_claimed = get_claimed(collection_id, item_id);
        assert_eq!(new_claimed, claimed + 1000);
    });
}
