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
use crate::Error;
use frame_support::{assert_err, assert_ok};
use sp_core::H256;
use sp_runtime::DispatchError::BadOrigin;

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;

fn account(id: u32) -> AccountIdOf<Test> {
    id.into()
}

#[test]
fn mint_tokens_to_claim() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 50));
        assert_eq!(Claiming::total(), 50);

        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));
        assert_eq!(Claiming::total(), 200);

        assert_err!(
            Claiming::mint_tokens_to_claim(RuntimeOrigin::signed(account(1)), 150),
            BadOrigin
        );
    });
}

#[test]
fn claim_tokens() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 200));
        assert_eq!(Claiming::total(), 200);

        let first_test_account_id = account(10);
        assert_ok!(Claiming::claim(
            RuntimeOrigin::root(),
            first_test_account_id,
            50,
            H256::random()
        ));
        assert_eq!(Claiming::total(), 150);
        assert_eq!(Balances::free_balance(first_test_account_id), 50);

        assert_ok!(Claiming::claim(
            RuntimeOrigin::root(),
            first_test_account_id,
            150,
            H256::random()
        ));
        assert_eq!(Claiming::total(), 0);
        assert_eq!(Balances::free_balance(first_test_account_id), 200);

        assert_err!(
            Claiming::claim(RuntimeOrigin::root(), first_test_account_id, 50, H256::random()),
            Error::<Test>::NotEnoughTokensForClaim
        );

        assert_eq!(Claiming::claims(first_test_account_id).len(), 2);
    });
}
