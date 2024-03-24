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
use crate::secp_utils::*;
use crate::{to_ascii_hex, Config, CurrencyOf, EcdsaSignature, Error, EthereumAddress};
use frame_support::traits::{Currency, VestingSchedule};
use frame_support::{assert_err, assert_noop, assert_ok};
use hex_literal::hex;
use parity_scale_codec::Encode;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn mint_tokens_to_claim() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 50));
        assert_eq!(Claiming::total(), 50);

        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));
        assert_eq!(Claiming::total(), 200);

        assert_err!(Claiming::mint_tokens_to_claim(RuntimeOrigin::signed(1), 150), BadOrigin);
    });
}

#[test]
fn basic_setup_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(Claiming::claims(&eth(&alice())), Some(100));
        assert_eq!(Claiming::claims(&eth(&dave())), Some(200));
        assert_eq!(Claiming::claims(&eth(&eve())), Some(300));
        assert_eq!(Claiming::claims(&eth(&frank())), Some(400));
        assert_eq!(Claiming::claims(&EthereumAddress::default()), None);
        assert_eq!(Claiming::vesting(&eth(&alice())), Some((50, 10, 1)));
    });
}

#[test]
fn claiming_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        assert_eq!(Balances::free_balance(42), 0);
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(42),
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_eq!(Balances::free_balance(&42), 100);
        assert_eq!(Vesting::vesting_balance(&42), Some(50));
        assert_eq!(Claiming::total(), 50);
    });
}

#[test]
fn claiming_more_than_available_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 50));

        assert_eq!(Balances::free_balance(42), 0);
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::signed(42),
                sig::<Test>(&alice(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::NotEnoughTokensForClaim
        );
        assert_eq!(Balances::free_balance(&42), 0);
        assert_eq!(Claiming::total(), 50);
    });
}

#[test]
fn double_claiming_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        assert_eq!(Balances::free_balance(42), 0);
        assert_ok!(Claiming::claim(
            RuntimeOrigin::signed(42),
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::signed(42),
                sig::<Test>(&alice(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
    });
}

#[test]
fn claiming_while_vested_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        CurrencyOf::<Test>::make_free_balance_be(&69, 1000);
        assert_eq!(Balances::free_balance(69), 1000);
        // A user is already vested
        assert_ok!(<Test as Config>::VestingSchedule::add_vesting_schedule(&69, 1000, 100, 10));

        // They should not be able to claim
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::signed(69),
                sig::<Test>(&alice(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::VestedBalanceExists,
        );
    });
}

#[test]
fn non_sender_sig_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        assert_eq!(Balances::free_balance(42), 0);
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::signed(42),
                sig::<Test>(&alice(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
    });
}

#[test]
fn non_claimant_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        assert_eq!(Balances::free_balance(42), 0);
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::signed(42),
                sig::<Test>(&bob(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
    });
}

#[test]
fn real_eth_sig_works() {
    new_test_ext().execute_with(|| {
        // "Pay RUSTs to the TEST account:2a00000000000000"
        let sig = hex!["444023e89b67e67c0562ed0305d252a5dd12b2af5ac51d6d3cb69a0b486bc4b3191401802dc29d26d586221f7256cd3329fe82174bdf659baea149a40e1c495d1c"];
        let sig = EcdsaSignature(sig);
        let who = 42u64.using_encoded(to_ascii_hex);
        let signer = Claiming::eth_recover(&sig, &who, &[][..]).unwrap();
        assert_eq!(signer.0, hex!["6d31165d5d932d571f3b44695653b46dcc327e84"]);
    });
}
