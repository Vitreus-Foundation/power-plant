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
use frame_support::pallet_prelude::ConstU32;
use frame_support::{assert_err, assert_ok};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;

const VANGUARD_1_REPUTATION_POINT: u64 = 7398066;

fn account(id: u8) -> AccountIdOf<Test> {
    [id; 32].into()
}

#[test]
fn basic_minting_should_work() {
    new_test_ext().execute_with(|| {
        let data = vec![0, 1, 3];
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let metadata = BoundedVec::<u8, ConstU32<50>>::try_from(data).unwrap_or_default();

        assert_ok!(NacManaging::create_collection(account(1)));

        assert_ok!(NacManaging::do_mint(item_id, account(1)));
        assert_ok!(NacManaging::update_nft_info(
            RuntimeOrigin::signed(account(1)),
            collection_id,
            item_id,
            metadata,
            nac_level,
            account(1)
        ));

        assert_eq!(NacManaging::get_nac_level(&account(1)), Some((nac_level, 123)));
        assert_eq!(
            Reputation::reputation(account(1)).unwrap().reputation.points().0,
            VANGUARD_1_REPUTATION_POINT
        );
    });
}

#[test]
fn update_metadata_and_nac_level_test() {
    new_test_ext().execute_with(|| {
        let data = vec![0, 1, 3];
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let metadata = BoundedVec::<u8, ConstU32<50>>::try_from(data).unwrap_or_default();

        assert_ok!(NacManaging::create_collection(account(1)));

        assert_ok!(NacManaging::do_mint(item_id, account(1)));
        assert_eq!(
            Reputation::reputation(account(1)).unwrap().reputation.points().0,
            VANGUARD_1_REPUTATION_POINT
        );
        assert_ok!(NacManaging::update_nft_info(
            RuntimeOrigin::signed(account(1)),
            collection_id,
            item_id,
            metadata.clone(),
            nac_level,
            account(1)
        ));

        assert_eq!(NacManaging::get_nac_level(&account(1)), Some((nac_level, 123)));

        let new_nac_level = 10_u8;
        assert_ok!(NacManaging::update_nft(
            RuntimeOrigin::signed(account(1)),
            metadata.clone(),
            Some(new_nac_level),
            account(1)
        ));
        assert_eq!(NacManaging::get_nac_level(&account(1)), Some((new_nac_level, 123)));

        assert_ok!(NacManaging::update_nft(
            RuntimeOrigin::signed(account(1)),
            metadata,
            None,
            account(1)
        ));
        assert_eq!(NacManaging::get_nac_level(&account(1)), Some((new_nac_level, 123)));
        assert_eq!(
            Reputation::reputation(account(1)).unwrap().reputation.points().0,
            VANGUARD_1_REPUTATION_POINT
        );
    });
}

#[test]
fn check_nac_level_test() {
    new_test_ext().execute_with(|| {
        let data = vec![0, 1, 3];
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let metadata = BoundedVec::<u8, ConstU32<50>>::try_from(data).unwrap_or_default();

        assert_ok!(NacManaging::create_collection(account(1)));

        assert_ok!(NacManaging::do_mint(item_id, account(1)));
        assert_ok!(NacManaging::update_nft_info(
            RuntimeOrigin::signed(account(1)),
            collection_id,
            item_id,
            metadata.clone(),
            nac_level,
            account(1)
        ));

        assert_ok!(NacManaging::check_nac_level(RuntimeOrigin::root(), account(1)));
        assert_err!(
            NacManaging::check_nac_level(RuntimeOrigin::root(), account(2)),
            Error::<Test>::NftNotFound
        );
    });
}

#[test]
fn user_has_access_test() {
    new_test_ext().execute_with(|| {
        let data = vec![0, 1, 3];
        let nac_level = 5_u8;
        let item_id = 123_u32;
        let collection_id = 0_u32;
        let metadata = BoundedVec::<u8, ConstU32<50>>::try_from(data).unwrap_or_default();

        assert_ok!(NacManaging::create_collection(account(1)));

        assert_ok!(NacManaging::do_mint(item_id, account(1)));
        assert_ok!(NacManaging::update_nft_info(
            RuntimeOrigin::signed(account(1)),
            collection_id,
            item_id,
            metadata.clone(),
            nac_level,
            account(1)
        ));

        assert!(NacManaging::user_has_access(account(1), 2));
        assert!(NacManaging::user_has_access(account(1), 5));
        assert!(!NacManaging::user_has_access(account(1), 6));
    });
}
