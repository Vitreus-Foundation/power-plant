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
use frame_support::{assert_err, assert_ok};
use sp_runtime::{DispatchError::BadOrigin, TokenError::FundsUnavailable};

#[test]
fn assigning_tokens_test() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::assign_token_amount(RuntimeOrigin::root(), 1, 10000));
        assert_err!(Claiming::assign_token_amount(RuntimeOrigin::signed(1), 1, 10000), BadOrigin);
    });
}

#[test]
fn user_has_access_test() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::assign_token_amount(RuntimeOrigin::root(), 1, 10000));
        assert_ok!(Claiming::claim(RuntimeOrigin::signed(1), 2, 5000));
        assert_err!(Claiming::claim(RuntimeOrigin::signed(4), 3, 1000), FundsUnavailable);
        assert_err!(Claiming::claim(RuntimeOrigin::signed(1), 4, 6000), FundsUnavailable);
    });
}
