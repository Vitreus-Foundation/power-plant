// This file is part of Substrate.

// Copyright (C) 2022 Parity Technologies (UK) Ltd.
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
use crate::{pallet, to_ascii_hex, Config, CurrencyOf, EcdsaSignature, Error, EthereumAddress};
use frame_support::traits::{Currency, ExistenceRequirement, VestingSchedule};
use frame_support::{assert_err, assert_noop, assert_ok};
use hex_literal::hex;
use parity_scale_codec::Encode;
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::TokenError;

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
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_eq!(Balances::free_balance(&42), 100);
        assert_eq!(Vesting::vesting_balance(&42), Some(50));
        assert_eq!(Claiming::total(), 50);
    });
}

#[test]
fn add_claim_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 250));

        assert_noop!(
            Claiming::mint_claim(RuntimeOrigin::signed(42), eth(&bob()), 200, None, None),
            sp_runtime::traits::BadOrigin,
        );
        assert_eq!(Balances::free_balance(42), 0);
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::none(),
                69,
                sig::<Test>(&bob(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim,
        );
        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&bob()), 200, None, None));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            69,
            sig::<Test>(&bob(), &69u64.encode(), &[][..])
        ));
        assert_eq!(Balances::free_balance(&69), 200);
        assert_eq!(Vesting::vesting_balance(&69), None);
        assert_eq!(Claiming::total(), 50);
    });
}

#[test]
fn add_claim_to_existing_claim_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 250));

        assert_eq!(Claiming::claims(&eth(&alice())), Some(100));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&alice()), 50, None, None));
        assert_eq!(Claiming::claims(&eth(&alice())), Some(150));

        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_eq!(Balances::free_balance(&42), 150);
        assert_eq!(Vesting::vesting_balance(&42), Some(50));
        assert_eq!(Claiming::total(), 100);
    });
}

#[test]
fn claiming_more_than_available_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 50));

        assert_eq!(Balances::free_balance(42), 0);
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::none(),
                42,
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
            RuntimeOrigin::none(),
            42,
            sig::<Test>(&alice(), &42u64.encode(), &[][..])
        ));
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&alice(), &42u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim
        );
    });
}

#[test]
fn claiming_while_vested_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        CurrencyOf::<Test>::make_free_balance_be(&69, 1000);
        assert_eq!(Balances::free_balance(69), 1000);
        // A user is already vested
        assert_ok!(<Test as Config>::VestingSchedule::add_vesting_schedule(&69, 1000, 100, 10));

        // They should not be able to claim
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            69,
            sig::<Test>(&alice(), &69u64.encode(), &[][..])
        ),);
    });
}

#[test]
fn non_sender_sig_doesnt_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        assert_eq!(Balances::free_balance(42), 0);
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::none(),
                42,
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
                RuntimeOrigin::none(),
                42,
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

#[test]
fn mint_claim_with_vesting_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 100));

        assert_eq!(Claiming::vesting(&eth(&bob())), None);

        // Создаём клейм с вестингом
        let vesting_schedule = Some((100, 10, 1));
        assert_ok!(Claiming::mint_claim(
            RuntimeOrigin::root(),
            eth(&bob()),
            100,
            vesting_schedule,
            None
        ));

        assert_eq!(Claiming::vesting(&eth(&bob())), vesting_schedule);
    });
}

#[test]
fn mint_claim_with_nft_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 50));

        let nfts_for_alice: Vec<_> = pallet::Nfts::<Test>::iter_prefix(eth(&alice())).collect();
        assert!(nfts_for_alice.is_empty());

        let nft_info = Some((1u32.into(), 1u32.into(), 5));
        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&alice()), 50, None, nft_info));

        assert_eq!(
            Claiming::nfts(&eth(&alice()), nft_info.unwrap().0),
            Some((nft_info.unwrap().1, nft_info.unwrap().2))
        );
    });
}

#[test]
fn mint_claim_with_vesting_and_nft_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 150));

        assert_eq!(Claiming::vesting(&eth(&eve())), None);

        let nfts_for_eve: Vec<_> = pallet::Nfts::<Test>::iter_prefix(eth(&eve())).collect();
        assert!(nfts_for_eve.is_empty());

        let vesting_schedule = Some((100, 20, 1));
        let nft_info = Some((2u32.into(), 2u32.into(), 10));
        assert_ok!(Claiming::mint_claim(
            RuntimeOrigin::root(),
            eth(&eve()),
            100,
            vesting_schedule,
            nft_info
        ));

        assert_eq!(Claiming::vesting(&eth(&eve())), vesting_schedule);
        assert_eq!(
            Claiming::nfts(&eth(&eve()), nft_info.unwrap().0),
            Some((nft_info.unwrap().1, nft_info.unwrap().2))
        );
    });
}

#[test]
fn add_claim_with_vesting_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 300));

        assert_noop!(
            Claiming::mint_claim(
                RuntimeOrigin::signed(42),
                eth(&bob()),
                200,
                Some((50, 10, 1)),
                None
            ),
            sp_runtime::traits::BadOrigin,
        );
        assert_eq!(Balances::free_balance(42), 0);
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::none(),
                69,
                sig::<Test>(&bob(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim,
        );
        assert_ok!(Claiming::mint_claim(
            RuntimeOrigin::root(),
            eth(&bob()),
            200,
            Some((50, 10, 1)),
            None
        ));
        assert_ok!(Claiming::claim(
            RuntimeOrigin::none(),
            69,
            sig::<Test>(&bob(), &69u64.encode(), &[][..])
        ));
        assert_eq!(Balances::free_balance(&69), 200);
        assert_eq!(Vesting::vesting_balance(&69), Some(50));

        // Make sure we can not transfer the vested balance.
        assert_err!(
            <Balances as Currency<_>>::transfer(&69, &80, 180, ExistenceRequirement::AllowDeath),
            TokenError::Frozen,
        );
    });
}

#[test]
fn claim_with_nft_should_work() {
    new_test_ext().execute_with(|| {
        assert_ok!(Claiming::mint_tokens_to_claim(RuntimeOrigin::root(), 300));

        let nfts_for_eve: Vec<_> = pallet::Nfts::<Test>::iter_prefix(eth(&eve())).collect();
        assert!(nfts_for_eve.is_empty());

        let nft_info = Some((3u32.into(), 3u32.into(), 10));

        assert_ok!(Claiming::mint_claim(RuntimeOrigin::root(), eth(&eve()), 100, None, nft_info));

        assert_eq!(
            Claiming::nfts(&eth(&eve()), nft_info.unwrap().0),
            Some((nft_info.unwrap().1, nft_info.unwrap().2))
        );
        assert_noop!(
            Claiming::claim(
                RuntimeOrigin::none(),
                42,
                sig::<Test>(&eve(), &69u64.encode(), &[][..])
            ),
            Error::<Test>::SignerHasNoClaim,
        );
    });
}
