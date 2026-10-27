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

use frame_support::{assert_err, assert_noop, assert_ok};
use parity_scale_codec::Decode;

type BalanceOf<Test> = <Test as pallet_balances::Config>::Balance;

const VANGUARD_1_REPUTATION_POINT: u64 = 7398066;

fn get_claimed(collection_id: CollectionId, item_id: ItemId) -> BalanceOf<Test> {
    let claimed_raw =
        Nfts::system_attribute(&collection_id, Some(&item_id), &CLAIM_AMOUNT_ATTRIBUTE_KEY)
            .unwrap_or_default();
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

        NacManaging::do_mint(item_id, owner).expect("Minting failed");
        NacManaging::update_nft_info(&collection_id, &item_id, nac_level, owner)
            .expect("Error updating nft info");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 0);

        NacManaging::on_claim(&owner, 1000_u64, None).expect("Error on claim");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 1000);

        NacManaging::on_claim(&owner, 1000_u64, None).expect("Error on claim");

        let new_claimed = get_claimed(collection_id, item_id);
        assert_eq!(new_claimed, claimed + 1000);
    });
}

fn prepare_vipp_claim(amount: Balance) {
    let _ = Balances::make_free_balance_be(&1, 10_000);
    assert_ok!(NacManaging::create_collection(&1));
    assert_ok!(NacManaging::create_collection(&1));
    assert_ok!(NacManaging::do_mint(123, 1));
    assert_ok!(NacManaging::on_claim(&1, amount, None));
}

#[test]
fn mint_vipp_nft_should_work() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        let (cap, id) = NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        assert_eq!(cap, 950);
        assert_ne!(id, 123);
        assert_eq!(Nfts::owner(1, id), Some(1));
        assert_eq!(get_claimed(1, id), 1_000);
        System::assert_last_event(Event::<Test>::VippNftMinted { owner: 1, item_id: id }.into());

        System::set_block_number(2);
        assert_eq!(NacManaging::mint_vipp_nft(&1).unwrap(), None);
        assert_eq!(Nfts::owned_in_collection(&1, &1).collect::<Vec<_>>(), vec![id]);
    });
}

#[test]
fn zero_claim_does_not_mint_vipp() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(0);
        assert_eq!(NacManaging::mint_vipp_nft(&1).unwrap(), None);
        assert_eq!(Nfts::owned_in_collection(&1, &1).count(), 0);
    });
}

#[test]
fn mint_vipp_nft_attribute_failure_is_atomic() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        FailAttributeWrite::set(Some((1, CLAIM_AMOUNT_ATTRIBUTE_KEY.to_vec())));
        assert_noop!(
            NacManaging::mint_vipp_nft(&1),
            DispatchError::Other("injected NFT attribute failure")
        );
        assert_eq!(Nfts::owned_in_collection(&1, &1).count(), 0);
    });
}

#[test]
fn mint_vipp_nft_id_collision_is_atomic() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        let (_, id) = NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        // The legacy ID generator returns the same ID twice in one block.
        assert_noop!(
            NacManaging::on_claim(&1, 200u64, None),
            pallet_nfts::Error::<Test>::AlreadyExists
        );
        assert_eq!(get_claimed(1, id), 1_000);
        assert_eq!(get_claimed(0, 123), 1_000);

        System::set_block_number(2);
        assert_ok!(NacManaging::on_claim(&1, 200u64, None));
        assert_eq!(get_claimed(0, 123), 1_200);
        assert_eq!(NacManaging::owned_vipp_claims(&1).unwrap().len(), 2);
    });
}

#[test]
fn on_claim_with_invalid_attribute_fails() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
            &0,
            &123,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &[1]
        ));
        assert_noop!(NacManaging::on_claim(&1, 200u64, None), Error::<Test>::InvalidClaimAttribute);
    });
}

#[test]
fn on_claim_callback_failure_is_atomic() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        System::set_block_number(2);
        FailVippCallback::set(true);
        assert_noop!(
            NacManaging::on_claim(&1, 200u64, None),
            DispatchError::Other("injected VIPP callback failure")
        );
    });
}

#[test]
fn on_claim_at_nft_limit_fails() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(64);
        for id in 0..64 {
            mint_manual_vipp(id, 1);
        }
        assert_noop!(NacManaging::on_claim(&1, 1u64, None), Error::<Test>::TooManyVippNfts);
        assert_eq!(get_claimed(0, 123), 64);
    });
}

fn mint_manual_vipp(id: ItemId, amount: Balance) {
    let config = ItemConfig { settings: ItemSettings::all_enabled() };
    assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::mint_into(&1, &id, &1, &config, true));
    assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
        &1,
        &id,
        &CLAIM_AMOUNT_ATTRIBUTE_KEY,
        &amount.encode(),
    ));
}

#[test]
fn vipp_burn_selects_the_smallest_decoded_claim() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(257);
        mint_manual_vipp(700, 256);
        mint_manual_vipp(701, 1);

        // Little-endian SCALE bytes order 256 before 1, unlike their numeric values.
        assert_ok!(NacManaging::burn_vipp_nft(&1));
        assert_eq!(Nfts::owner(1, 700), Some(1));
        assert_eq!(Nfts::owner(1, 701), None);
        assert_eq!(get_claimed(0, 123), 256);
        assert!(!NacManaging::is_vipp_inactive(&1));
    });
}

#[test]
fn vipp_burn_rolls_back_if_nac_threshold_update_fails() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        let (_, id) = NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        FailAttributeWrite::set(Some((0, CLAIM_AMOUNT_ATTRIBUTE_KEY.to_vec())));
        assert_noop!(
            NacManaging::burn_vipp_nft(&1),
            DispatchError::Other("injected NFT attribute failure")
        );
        assert_eq!(Nfts::owner(1, id), Some(1));
        assert_eq!(get_claimed(0, 123), 1_000);
    });
}

#[test]
fn vipp_last_burn_marks_nac_inactive_and_cannot_be_reissued() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        assert!(NacManaging::mint_vipp_nft(&1).unwrap().is_some());
        assert_eq!(NacManaging::burn_vipp_nft(&1), Ok(true));
        assert_eq!(Nfts::system_attribute(&0, Some(&123), &VIPP_STATUS_EXIST), Some(vec![1]));
        System::set_block_number(2);
        assert_ok!(NacManaging::on_claim(&1, 1_000u64, None));
        assert_eq!(NacManaging::can_mint_vipp(&1).unwrap(), None);
    });
}

#[test]
fn vipp_burn_rolls_back_if_marking_the_nac_inactive_fails() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        let (_, id) = NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        FailAttributeWrite::set(Some((0, VIPP_STATUS_EXIST.to_vec())));
        assert_noop!(
            NacManaging::burn_vipp_nft(&1),
            DispatchError::Other("injected NFT attribute failure")
        );
        assert_eq!(Nfts::owner(1, id), Some(1));
        assert_eq!(get_claimed(0, 123), 1_000);
    });
}

#[test]
fn vipp_malformed_claim_is_not_treated_as_zero_or_burned() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        let (_, id) = NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
            &1,
            &id,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &[1]
        ));
        assert_noop!(NacManaging::burn_vipp_nft(&1), Error::<Test>::InvalidClaimAttribute);
        assert_eq!(Nfts::owner(1, id), Some(1));
    });
}

#[test]
fn failed_burn_callback_rolls_back_nft_threshold_and_inactive_flag_together() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        FailVippCallback::set(true);
        assert_noop!(
            NacManaging::burn_vipp_nft(&1),
            DispatchError::Other("injected VIPP callback failure")
        );
    });
}

#[test]
fn threshold_reconciliation_rolls_back_earlier_burns_if_a_later_callback_fails() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(100);
        NacManaging::mint_vipp_nft(&1).unwrap().unwrap();
        System::set_block_number(2);
        assert_ok!(NacManaging::on_claim(&1, 200u64, None));
        let owned = NacManaging::owned_vipp_claims(&1).unwrap();
        let second_id = owned.iter().find(|(_, amount)| *amount == 200).unwrap().0;
        let _ = Balances::make_free_balance_be(&1, 50);
        FailVippCallbackOnItem::set(Some(second_id));
        assert_noop!(
            NacManaging::try_check_account_threshold(&1),
            DispatchError::Other("injected VIPP callback failure")
        );
        assert_eq!(NacManaging::owned_vipp_claims(&1).unwrap(), owned);
        assert_eq!(get_claimed(0, 123), 300);
        assert!(!NacManaging::is_vipp_inactive(&1));
    });
}

#[test]
fn oversized_legacy_ownership_is_not_partially_burned() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        for id in 0..65 {
            mint_manual_vipp(id, 1);
        }
        let _ = Balances::make_free_balance_be(&1, 1);
        assert_noop!(NacManaging::try_check_account_threshold(&1), Error::<Test>::TooManyVippNfts);
        assert_eq!(Nfts::owned_in_collection(&1, &1).count(), 65);
        assert_noop!(NacManaging::burn_vipp_nft(&1), Error::<Test>::TooManyVippNfts);
    });
}

#[test]
fn legacy_zero_value_nfts_do_not_make_threshold_reconciliation_loop_forever() {
    new_test_ext().execute_with(|| {
        prepare_vipp_claim(1_000);
        mint_manual_vipp(0, 0);
        mint_manual_vipp(1, 1_000);
        let _ = Balances::make_free_balance_be(&1, 949);
        assert_ok!(NacManaging::try_check_account_threshold(&1));
        assert_eq!(Nfts::owned_in_collection(&1, &1).count(), 0);
        assert!(NacManaging::is_vipp_inactive(&1));
    });
}
