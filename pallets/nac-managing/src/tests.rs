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


fn get_user_nfts() -> Vec<(u32, u8)> {
    let nfts: Vec<_> = UsersNft::<Test>::iter().map(|(_account_id, (item_id, level))| (item_id, level)).collect();
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
        let first_owner = H160::random();
        assert_ok!(NacManaging::create_collection(RuntimeOrigin::root(), 0, first_owner));
        assert_ok!(NacManaging::mint(RuntimeOrigin::signed(first_owner), 0, 2, BoundedVec::new(), 1_u8, Default::default()));
        assert_eq!(get_user_nfts(), vec![(2, 1)]);

        let second_owner = H160::random();
        assert_ok!(NacManaging::create_collection(RuntimeOrigin::root(), 1, second_owner));
        assert_ok!(NacManaging::mint(RuntimeOrigin::signed(second_owner), 1, 4, BoundedVec::new(), 2_u8, H160::random()));
        assert_eq!(get_user_nfts(), vec![(4, 2), (2, 1)]);
    });
}