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

//! Tests for claiming pallet.

use crate::mock::*;
use frame_support::assert_ok;
use pallet_atomic_swap::BalanceSwapAction;
use sp_core::H256;

const A: u64 = 1;
const B: u64 = 2;

#[test]
fn user_has_access_test() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(A),
            B,
            BalanceSwapAction::new(50),
            H256::random()
        ));
        assert_eq!(Balances::free_balance(A), 50);
        assert_eq!(Balances::free_balance(B), 250);
    });
}
