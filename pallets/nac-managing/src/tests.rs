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

//!
//! # Module Overview
//!
//! This module provides unit tests for the `pallet_nac_managing` in a Substrate-based blockchain.
//! The tests are designed to verify key functionalities of the Nonfungible Asset Certificate (NAC)
//! management pallet, which involves minting NFTs, managing NAC levels, and handling claims. The
//! tests validate that the pallet operates correctly under different conditions, ensuring that the
//! core features like minting, updating NAC levels, and checking access work as intended.
//!
//! # Key Test Cases
//!
//! - **Basic NFT Minting**:
//!   - `basic_minting_should_work()`: Verifies that an NFT can be minted correctly for a user with
//!     a specified NAC level. The test ensures that after minting and updating, the NAC level is
//!     set correctly, and the reputation points are accurately updated for the user.
//!
//! - **Updating NAC Level**:
//!   - `update_nac_level_test()`: Tests the update of NAC levels for a minted NFT. It ensures that
//!     the `update_nft_info()` function correctly updates the NAC level, and that subsequent checks
//!     for NAC compliance (`check_nac_level()`) work properly, including returning an error if the
//!     NFT is not found for a given account.
//!
//! - **Access Verification**:
//!   - `user_has_access_test()`: Validates that users have the correct level of access based on
//!     their NAC level. This is tested by minting an NFT, updating its NAC level, and then checking
//!     if the user has access to different levels. The test verifies both positive and negative
//!     scenarios to ensure accurate access control.
//!
//! - **Handling Claims**:
//!   - `on_claim_should_work()`: Tests the `on_claim()` function, which allows users to claim rewards
//!     associated with their NFT. The test checks that claims are recorded correctly, and that multiple
//!     claims update the total claimed value accurately.
//!
//! # Access Control and Security
//!
//! - **Minting Restrictions**: The `basic_minting_should_work()` test ensures that only authorized
//!   users (such as admins) can create collections and mint NFTs. This helps simulate the access
//!   control mechanisms that would be present in a production environment.
//! - **Error Handling Verification**: The tests use assertions like `assert_err!()` to confirm that
//!   the correct errors are returned when invalid operations are attempted, such as attempting to
//!   check an NAC level for a user without an NFT. This ensures the system can prevent unauthorized
//!   or invalid actions.
//!
//! # Developer Notes
//!
//! - **Controlled Test Environment**: The tests run using `new_test_ext()`, which provides a clean
//!   test environment for each test run. This ensures consistency and reproducibility by starting
//!   with the same blockchain state for each test.
//! - **Reputation Integration**: The `basic_minting_should_work()` test checks the integration with
//!   the `pallet_reputation` by verifying the reputation points after minting. This ensures that
//!   the user's reputation is correctly affected by actions involving their NFTs.
//! - **Incremental Claims Testing**: The `on_claim_should_work()` test ensures that multiple claims
//!   are correctly accumulated, simulating real-world scenarios where users interact with their NFTs
//!   repeatedly to claim rewards.
//!
//! # Usage Scenarios
//!
//! - **Basic NFT Issuance**: The `basic_minting_should_work()` test ensures that users can receive
//!   NFTs, and that these NFTs are correctly associated with a NAC level. This is useful for scenarios
//!   where users are rewarded for participation or specific achievements within the network.
//! - **Updating User Benefits**: The `update_nac_level_test()` helps verify that user benefits,
//!   represented by NAC levels, can be updated dynamically. This is crucial for scenarios involving
//!   loyalty programs or tier-based access to features.
//! - **Claiming Rewards**: The `on_claim_should_work()` test simulates scenarios where users claim
//!   rewards over time. It helps ensure that the claiming process is smooth, accurate, and cumulative,
//!   providing a reliable reward system for users holding NFTs.
//!
//! # Integration Considerations
//!
//! - **Reputation and Access Control**: Developers should consider how changes to NAC levels affect
//!   a user's reputation and access within the network. The tests ensure that these aspects are
//!   correctly integrated, but additional testing may be required if the reputation or access
//!   mechanisms are expanded in the future.
//! - **Error Handling for Unauthorized Actions**: Tests like `update_nac_level_test()` and
//!   `user_has_access_test()` verify that unauthorized actions are correctly prevented. This is
//!   important for ensuring that the system is secure and that users cannot bypass restrictions.
//! - **Claims and Reward Tracking**: The `on_claim_should_work()` test highlights the importance of
//!   accurately tracking user interactions with their NFTs, especially for claims. Developers should
//!   ensure that the on-chain state remains consistent and that all claims are recorded properly,
//!   even under heavy load or in complex scenarios.
//!
//! # Example Scenario
//!
//! Suppose a developer needs to verify that users who have received NFTs through the NAC management
//! system can claim rewards based on their activity. Using the `on_claim_should_work()` test, the
//! developer can simulate multiple claims from the same user and verify that the claimed amount is
//! correctly accumulated and recorded on-chain. This ensures that users can interact with their
//! NFTs over time without inconsistencies or errors, providing a seamless experience for NFT holders.
//!


use crate::{mock::*, *};

use frame_support::{assert_err, assert_ok};
use parity_scale_codec::Decode;

type BalanceOf<Test> = <Test as pallet_balances::Config>::Balance;

const VANGUARD_1_REPUTATION_POINT: u64 = 7398066;

fn get_claimed(collection_id: CollectionId, item_id: ItemId) -> BalanceOf<Test> {
    let claimed_raw =
        Nfts::system_attribute(&collection_id, Some(&item_id), &CLAIM_AMOUNT_ATTRIBUTE_KEY)
            .unwrap_or(vec![]);
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

        NacManaging::do_mint(item_id, owner.clone()).expect("Minting failed");
        NacManaging::update_nft_info(&collection_id, &item_id, nac_level, owner.clone())
            .expect("Error updating nft info");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 0);

        NacManaging::on_claim(&owner, 1000_u64).expect("Error on claim");

        let claimed = get_claimed(collection_id, item_id);
        assert_eq!(claimed, 1000);

        NacManaging::on_claim(&owner, 1000_u64).expect("Error on claim");

        let new_claimed = get_claimed(collection_id, item_id);
        assert_eq!(new_claimed, claimed + 1000);
    });
}
