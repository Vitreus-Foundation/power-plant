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
use frame_support::assert_ok;
use frame_support::pallet_prelude::ConstU32;

fn get_user_nfts() -> Vec<(u32, u8)> {
    let nfts: Vec<_> = UsersNft::<Test>::iter()
        .map(|(_account_id, (item_id, level))| (item_id, level))
        .collect();
    nfts
}

#[test]
fn basic_setup_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(get_user_nfts(), vec![]);
    });
}

#[test]
fn basic_minting_should_work() {
    new_test_ext().execute_with(|| {
        let data: Vec<u8> = vec![0, 1, 3];
        let metadata = BoundedVec::<u8, ConstU32<50>>::try_from(data.clone()).unwrap_or_default();

        assert_ok!(NacManaging::create_collection(RuntimeOrigin::root(), 0, 1));
        assert_ok!(NacManaging::mint(RuntimeOrigin::signed(1), 0, 1, metadata, 4));
        assert_eq!(get_user_nfts(), vec![(1, 1)]);

        let data: Vec<u8> = vec![3, 2, 0];
        let metadata = BoundedVec::<u8, ConstU32<50>>::try_from(data.clone()).unwrap_or_default();

        assert_ok!(NacManaging::mint(RuntimeOrigin::signed(1), 0, 4, metadata, 5));
        assert_eq!(get_user_nfts(), vec![(1, 1), (4, 2)]);
    });
}
