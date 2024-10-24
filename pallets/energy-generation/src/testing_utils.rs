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
//! This module provides a set of utility functions designed to facilitate testing and benchmarking
//! of the staking pallet in a Substrate-based blockchain. These utilities include tools for setting
//! up staking environments, such as creating and funding user accounts, bonding validators and cooperators,
//! and generating randomized solutions for staking. The module also includes various helper functions
//! for managing staking states and interactions during testing.
//!
//! # Key Features and Functions
//!
//! - **Staking Setup Utilities**:
//!   - `clear_validators_and_cooperators<T>()`: This function removes all validators and cooperators
//!     from storage, effectively resetting the staking state. It is useful for setting up a clean
//!     testing environment without remnants of previous test data.
//!   - `create_funded_user<T>(string: &'static str, n: u32, balance_factor: u32) -> T::AccountId`:
//!     Creates a user account with a specified balance, enabling developers to quickly set up accounts
//!     that are ready to participate in staking scenarios.
//!   - `create_stash_controller<T>(n: u32, balance_factor: u32, destination: RewardDestination<T::AccountId>)`:
//!     Creates a stash-controller pair with a defined balance and reward destination. This function is
//!     fundamental for configuring validators and their controllers, enabling precise control over
//!     staking setups in tests.
//!
//! - **Validator and Cooperator Management**:
//!   - `setup_validators_and_cooperators<T>()`: Sets up a specified number of validators and cooperators,
//!     optionally randomizing their stakes. This function is useful for creating complex testing scenarios
//!     with multiple actors interacting in the staking ecosystem.
//!   - `setup_cooperators<T>(cooperators: u32, validators: u32, edge_per_cooperator: usize, randomize_stake: bool)`:
//!     Configures a number of cooperators and randomly selects validators for them to cooperate with,
//!     simulating real-world network interactions where cooperators bond their tokens to different validators.
//!
//! - **Testing Tools**:
//!   - `staking_events() -> Vec<Event<Test>>`: Collects all staking-related events that have been emitted,
//!     allowing testers to verify that the correct events are generated in response to specific staking actions.
//!   - `current_era<T>() -> EraIndex`: Retrieves the current staking era, which is useful for determining
//!     the appropriate timing for slashing, payouts, or other era-dependent actions in tests.
//!   - `perbill_signed_sub_abs(a: Perbill, b: Perbill) -> Perbill`: Calculates the absolute difference
//!     between two `Perbill` values, which is useful for verifying that discrepancies between expected and
//!     actual outcomes in tests are within acceptable limits.
//!
//! # Access Control and Security
//!
//! - **Restricted Use for Testing Only**: The functions in this module are intended for use strictly in
//!   testing environments. They provide low-level access to critical staking data and directly manipulate
//!   on-chain state, which is not suitable for production use.
//! - **Controlled Validator Creation**: When creating validators and cooperators, the setup functions
//!   include specific parameters for bonding and stake distribution. These parameters ensure that test
//!   scenarios accurately simulate real-world staking behaviors, such as bonding requirements and reward
//!   distributions.
//!
//! # Developer Notes
//!
//! - **Randomized Solutions for Testing**: The module includes the ability to generate random stake
//!   distributions (`setup_validators_and_cooperators`). This feature is important for stress-testing the
//!   staking logic under a variety of conditions, including edge cases where stake distributions may be
//!   highly unequal or unpredictable.
//! - **Event Tracking**: By using `staking_events()` and related functions, developers can easily track
//!   which events are triggered by specific actions. This allows for comprehensive validation of the
//!   staking process, ensuring that all actions (e.g., slashing, payouts) generate the expected events
//!   and that these events correctly reflect changes to the on-chain state.
//! - **Configuration Flexibility**: Functions like `create_stash_controller` and `setup_validators_and_cooperators`
//!   offer a high degree of flexibility, allowing developers to specify parameters like balance factors
//!   and reward destinations. This flexibility is crucial for testing various configurations, including
//!   different staking models, reward strategies, and governance decisions.
//!
//! # Usage Scenarios
//!
//! - **Resetting Staking State**: The `clear_validators_and_cooperators` function is essential for
//!   resetting the staking state between tests. This ensures that each test begins with a clean state,
//!   preventing interference from previous test runs and allowing for consistent and reproducible results.
//! - **Simulating Realistic Staking Networks**: By using `setup_validators_and_cooperators`, developers
//!   can create a simulated staking network with multiple validators and cooperators, each with different
//!   stakes and configurations. This is useful for evaluating how changes to the staking pallet affect
//!   network dynamics, such as reward distribution, validator selection, and slashing mechanisms.
//! - **Reward Distribution Testing**: The `calculate_reward` utility can be used to validate the
//!   correctness of reward calculations. By setting up controlled test environments and manipulating
//!   staking parameters, developers can ensure that rewards are distributed fairly and in accordance
//!   with the blockchain's economic model.
//!
//! # Integration Considerations
//!
//! - **Testing Performance Impact**: When integrating these utilities into tests, developers should
//!   consider the performance impact of certain functions, especially those that modify a large number
//!   of validators or cooperators (`setup_validators_and_cooperators`). These functions may require
//!   significant computational resources, which is important to take into account for longer test suites.
//! - **Compatibility with Staking and Governance Pallets**: The functions in this module are designed to
//!   interact closely with the staking pallet and other governance-related pallets. Developers should
//!   ensure that the use of these utilities aligns with the expected behavior of the staking and
//!   governance mechanisms, especially when testing slashing and validator selection processes.
//! - **Edge Case Simulation**: The ability to create custom validators, cooperators, and randomized
//!   stakes makes this module well-suited for simulating edge cases. For example, developers can create
//!   validators with minimal bonds or cooperators that are heavily concentrated on a single validator,
//!   helping to identify potential vulnerabilities or performance issues in the staking logic.
//!
//! # Example Scenario
//!
//! Suppose developers need to test how the staking pallet behaves when a large number of cooperators
//! are bonded to only a few validators, potentially causing concentration risk. Using the utility
//! functions provided by this module, they can create several validators with minimal bond requirements,
//! then set up a significant number of cooperators and have them bond to the few available validators.
//! By tracking staking events and analyzing reward distributions, developers can verify whether the
//! system effectively handles such concentrations or if changes are needed to distribute staking
//! incentives more evenly across the network.
//!

use crate::{Pallet as Staking, *};
use frame_benchmarking::account;
use frame_system::RawOrigin;
use rand_chacha::{
    rand_core::{RngCore, SeedableRng},
    ChaChaRng,
};
use sp_io::hashing::blake2_256;

use frame_support::{pallet_prelude::*, traits::Currency};
use sp_runtime::{traits::StaticLookup, Perbill};
use sp_std::prelude::*;

const SEED: u32 = 0;

/// This function removes all validators and cooperators from storage.
pub fn clear_validators_and_cooperators<T: Config>() {
    #[allow(deprecated)]
    Validators::<T>::remove_all();

    // whenever we touch cooperators counter we should update `T::VoterList` as well.
    #[allow(deprecated)]
    Cooperators::<T>::remove_all();

    let _ = Collaborations::<T>::clear(u32::MAX, None);
}

/// Grab a funded user.
pub fn create_funded_user<T: Config>(
    string: &'static str,
    n: u32,
    balance_factor: u32,
) -> T::AccountId {
    let user = account(string, n, SEED);
    let balance = T::Currency::minimum_balance() * balance_factor.into();
    let _ = T::Currency::make_free_balance_be(&user, balance);
    user
}

/// Grab a funded user with max Balance.
pub fn create_funded_user_with_balance<T: Config>(
    string: &'static str,
    n: u32,
    balance: StakeOf<T>,
) -> T::AccountId {
    let user = account(string, n, SEED);
    let _ = T::StakeCurrency::make_free_balance_be(&user, balance);
    user
}

/// Create a stash and controller pair.
pub fn create_stash_controller<T: Config>(
    n: u32,
    balance_factor: u32,
    destination: RewardDestination<T::AccountId>,
) -> Result<(T::AccountId, T::AccountId, T::StakeBalance), &'static str> {
    let stash = create_funded_user::<T>("stash", n, balance_factor);
    let controller = create_funded_user::<T>("controller", n, balance_factor);
    let controller_lookup = T::Lookup::unlookup(controller.clone());
    let amount = T::StakeCurrency::minimum_balance() * (balance_factor / 10).max(1).into();
    Staking::<T>::bond(
        RawOrigin::Signed(stash.clone()).into(),
        controller_lookup,
        amount,
        destination,
    )?;
    Ok((stash, controller, amount))
}

/// Create a stash and controller pair with fixed balance.
pub fn create_stash_controller_with_balance<T: Config>(
    n: u32,
    balance: crate::StakeOf<T>,
    destination: RewardDestination<T::AccountId>,
) -> Result<(T::AccountId, T::AccountId), &'static str> {
    let stash = create_funded_user_with_balance::<T>("stash", n, balance);
    let controller = create_funded_user_with_balance::<T>("controller", n, balance);
    let controller_lookup = T::Lookup::unlookup(controller.clone());

    Staking::<T>::bond(
        RawOrigin::Signed(stash.clone()).into(),
        controller_lookup,
        balance,
        destination,
    )?;
    Ok((stash, controller))
}

/// Create a stash and controller pair, where the controller is dead, and payouts go to controller.
/// This is used to test worst case payout scenarios.
pub fn create_stash_and_dead_controller<T: Config>(
    n: u32,
    balance_factor: u32,
    destination: RewardDestination<T::AccountId>,
) -> Result<(T::AccountId, T::AccountId), &'static str> {
    let stash = create_funded_user::<T>("stash", n, balance_factor);
    // controller has no funds
    let controller = create_funded_user::<T>("controller", n, 0);
    let controller_lookup = T::Lookup::unlookup(controller.clone());
    let amount = T::StakeCurrency::minimum_balance() * (balance_factor / 10).max(1).into();
    Staking::<T>::bond(
        RawOrigin::Signed(stash.clone()).into(),
        controller_lookup,
        amount,
        destination,
    )?;
    Ok((stash, controller))
}

/// create `max` validators.
pub fn create_validators<T: Config>(
    max: u32,
    balance_factor: u32,
) -> Result<Vec<AccountIdLookupOf<T>>, &'static str> {
    create_validators_with_seed::<T>(max, balance_factor, 0)
}

/// create `max` validators, with a seed to help unintentional prevent account collisions.
pub fn create_validators_with_seed<T: Config>(
    max: u32,
    balance_factor: u32,
    seed: u32,
) -> Result<Vec<AccountIdLookupOf<T>>, &'static str> {
    let mut validators: Vec<AccountIdLookupOf<T>> = Vec::with_capacity(max as usize);
    for i in 0..max {
        let (stash, controller, _) =
            create_stash_controller::<T>(i + seed, balance_factor, RewardDestination::Stash)?;
        let validator_prefs =
            ValidatorPrefs { commission: Perbill::from_percent(50), ..Default::default() };
        Staking::<T>::validate(RawOrigin::Signed(controller).into(), validator_prefs)?;
        let stash_lookup = T::Lookup::unlookup(stash);
        validators.push(stash_lookup);
    }
    Ok(validators)
}

/// This function generates validators and cooperators who are randomly cooperating
/// `edge_per_cooperator` random validators (until `to_cooperate` if provided).
///
/// NOTE: This function will remove any existing validators or cooperators to ensure
/// we are working with a clean state.
///
/// Parameters:
/// - `validators`: number of bonded validators
/// - `cooperators`: number of bonded cooperators.
/// - `edge_per_cooperator`: number of edge (vote) per cooperator.
/// - `randomize_stake`: whether to randomize the stakes.
/// - `to_cooperate`: if `Some(n)`, only the first `n` bonded validator are voted upon. Else, all of
///   them are considered and `edge_per_cooperator` random validators are voted for.
///
/// Return the validators chosen to be cooperated.
pub fn create_validators_with_cooperators_for_era<T: Config>(
    validators: u32,
    cooperators: u32,
    edge_per_cooperator: usize,
    randomize_stake: bool,
    to_cooperate: Option<u32>,
) -> Result<Vec<AccountIdLookupOf<T>>, &'static str> {
    clear_validators_and_cooperators::<T>();

    let mut validators_stash: Vec<AccountIdLookupOf<T>> = Vec::with_capacity(validators as usize);
    let mut rng = ChaChaRng::from_seed(SEED.using_encoded(blake2_256));

    // Create validators
    for i in 0..validators {
        let balance_factor = if randomize_stake { rng.next_u32() % 255 + 10 } else { 100u32 };
        let (v_stash, v_controller, _) =
            create_stash_controller::<T>(i, balance_factor, RewardDestination::Stash)?;
        let validator_prefs =
            ValidatorPrefs { commission: Perbill::from_percent(50), ..Default::default() };
        Staking::<T>::validate(RawOrigin::Signed(v_controller.clone()).into(), validator_prefs)?;
        let stash_lookup = T::Lookup::unlookup(v_stash.clone());
        validators_stash.push(stash_lookup.clone());
    }

    let to_cooperate = to_cooperate.unwrap_or(validators_stash.len() as u32) as usize;
    let validator_chosen = validators_stash[0..to_cooperate].to_vec();

    // Create cooperators
    for j in 0..cooperators {
        let balance_factor = if randomize_stake { rng.next_u32() % 255 + 10 } else { 100u32 };
        let (_n_stash, n_controller, amount) =
            create_stash_controller::<T>(u32::MAX - j, balance_factor, RewardDestination::Stash)?;

        // Have them randomly validate
        let mut available_validators = validator_chosen.clone();
        let mut selected_validators: Vec<(AccountIdLookupOf<T>, StakeOf<T>)> =
            Vec::with_capacity(edge_per_cooperator);
        let num = validators.min(edge_per_cooperator as u32);
        let stake = amount / num.into();

        for _ in 0..num {
            let selected = rng.next_u32() as usize % available_validators.len();
            let validator = available_validators.remove(selected);
            selected_validators.push((validator, stake));
        }
        Staking::<T>::cooperate(
            RawOrigin::Signed(n_controller.clone()).into(),
            selected_validators,
        )?;
    }

    ValidatorCount::<T>::put(validators);

    Ok(validator_chosen)
}

/// get the current era.
pub fn current_era<T: Config>() -> EraIndex {
    <Pallet<T>>::current_era().unwrap_or(0)
}

/// make signed subtraction and return absolute value
pub fn perbill_signed_sub_abs(a: Perbill, b: Perbill) -> Perbill {
    if a >= b {
        a - b
    } else {
        b - a
    }
}
