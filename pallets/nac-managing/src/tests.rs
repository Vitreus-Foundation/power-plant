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

fn items() -> Vec<(u64, u32, u32)> {
    let mut r: Vec<_> = Account::<Test>::iter().map(|x| x.0).collect();
    r.sort();
    let mut s: Vec<_> = Item::<Test>::iter().map(|x| (x.2.owner, x.0, x.1)).collect();
    s.sort();
    assert_eq!(r, s);
    for collection in Item::<Test>::iter()
        .map(|x| x.0)
        .scan(None, |s, item| {
            if s.map_or(false, |last| last == item) {
                *s = Some(item);
                Some(None)
            } else {
                Some(Some(item))
            }
        })
        .flatten()
    {
        let details = Collection::<Test>::get(collection).unwrap();
        let items = Item::<Test>::iter_prefix(collection).count() as u32;
        assert_eq!(details.items, items);
    }
    r
}

fn collections() -> Vec<(u64, u32)> {
    let mut s: Vec<_> = Collection::<Test>::iter().map(|x| (x.1.owner, x.0)).collect();
    s.sort();
    s
}

#[test]
fn basic_setup_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(items(), vec![]);
    });
}

#[test]
fn basic_minting_should_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(NacManaging::create_collection(RuntimeOrigin::root(), 0, 1));
        assert_eq!(collections(), vec![(1, 0)]);
        assert_ok!(NacManaging::mint(RuntimeOrigin::signed(1), 0, 42, 1_u8, Default::default(), 1));
        assert_eq!(items(), vec![(1, 0, 42)]);

        assert_ok!(NacManaging::create_collection(RuntimeOrigin::root(), 1, 2));
        assert_eq!(collections(), vec![(1, 0), (2, 1)]);
        assert_ok!(NacManaging::mint(RuntimeOrigin::signed(2), 1, 69, 1_u8, Default::default(), 1));
        assert_eq!(items(), vec![(1, 0, 42), (1, 1, 69)]);
    });
}

#[test]
fn mint_should_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(NacManaging::create_collection(RuntimeOrigin::root(), 0, 1));
        assert_ok!(NacManaging::mint(RuntimeOrigin::signed(1), 0, 42, 1_u8, Default::default(), 1));
        assert_eq!(NacManaging::owner(0, 42).unwrap(), 1);
        assert_eq!(collections(), vec![(1, 0)]);
        assert_eq!(items(), vec![(1, 0, 42)]);
    });
}