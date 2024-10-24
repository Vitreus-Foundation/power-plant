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
//! This Rust module is responsible for calculating the total payout to all validators and their
//! cooperators in a Substrate-based blockchain, based on the nominated proof-of-stake (NPoS) model.
//! The core functionality provided by this module centers around determining staking rewards for
//! each era, based on the overall staking rate, the total supply, and specific inflation targets.
//!
//! # Key Features and Functions
//!
//! - **Payout Calculation**:
//!   - `compute_total_payout(yearly_inflation: &PiecewiseLinear, npos_token_staked: N, total_tokens: N, era_duration: u64) -> (N, N)`:
//!     This function calculates the total payout for an era, considering the yearly inflation rate,
//!     the number of tokens staked by cooperators and validators, the total token supply, and the
//!     duration of the era in milliseconds. It returns two values:
//!     - **Staker Payout**: The reward portion that goes to the stakers.
//!     - **Maximum Payout**: The maximum allowable reward payout for the era.
//!
//! - **Mathematical Model**:
//!   - The function utilizes a `PiecewiseLinear` model to calculate the yearly inflation rate, which
//!     is then adjusted for the length of the staking era. This ensures that payouts are proportional
//!     to the network's inflation goals and staking participation.
//!   - The computation takes into account the staking rate (i.e., the fraction of total tokens that
//!     are staked), enabling dynamic adjustment of rewards to incentivize more or less staking
//!     participation as needed.
//!
//! # Access Control and Security
//!
//! - **Restricted Use**: This module's primary function, `compute_total_payout`, is intended to be
//!   used by the staking pallet internally, typically during each era's payout calculation phase.
//!   It is crucial that this calculation remains consistent to ensure fair reward distribution among
//!   validators and their nominators.
//! - **Fair Reward Distribution**: By basing the payout on the staking rate and adjusting for yearly
//!   inflation, the module ensures that the rewards are fair and sustainable, aligning with the
//!   economic model of the blockchain. This approach helps maintain the desired staking participation
//!   rate without introducing inflationary pressures that could destabilize the token's value.
//!
//! # Developer Notes
//!
//! - **Constants for Time Calculation**: The module uses constants like `MILLISECONDS_PER_YEAR` to
//!   perform the necessary conversion of yearly inflation to the appropriate rate per staking era.
//!   This constant represents the Julian year (365.25 days) and is used to accurately compute
//!   era-based payouts.
//! - **Flexible Token Types**: The function is generic over types implementing `AtLeast32BitUnsigned`,
//!   making it adaptable for use with different types of numerical representations (e.g., `u64`,
//!   `u128`). This flexibility allows it to support various tokenomics and supply sizes across
//!   different blockchain networks.
//! - **Inflation Control**: The use of a piecewise linear curve (`PiecewiseLinear`) for yearly
//!   inflation provides flexibility in adjusting the rewards based on network conditions. This allows
//!   developers to fine-tune inflation rates to incentivize desirable behaviors in the staking
//!   ecosystem, such as increasing validator participation during times of low staking engagement.
//!
//! # Usage Scenarios
//!
//! - **Era Payout Calculation**: The `compute_total_payout` function is typically called at the end of
//!   each era to determine how much reward should be distributed to all validators and their
//!   cooperators. This is crucial for maintaining a predictable and transparent reward system that
//!   validators and stakers can rely on when deciding to stake their tokens.
//! - **Governance Adjustments**: The curve-based inflation model allows the governance body of the
//!   blockchain to adjust the inflation rate dynamically, ensuring that the staking rewards are aligned
//!   with overall economic goals. For instance, the yearly inflation can be increased to encourage
//!   more staking, or reduced if the participation is already high.
//!
//! # Integration Considerations
//!
//! - **Staking Economics**: When integrating this module with a blockchain, it is essential to ensure
//!   that the parameters, such as yearly inflation rates and era duration, are set in accordance with
//!   the chain's economic model. Improper settings may result in over-inflation or under-rewarding
//!   stakers, which can negatively impact the stability and attractiveness of the staking process.
//! - **Adjustable Era Duration**: The payout calculation is affected by the `era_duration`, expressed
//!   in milliseconds. Developers need to ensure that era durations are set consistently across the
//!   staking module, as changes to this value will impact the total rewards calculated for each era.
//! - **Piecewise Linear Inflation**: The `PiecewiseLinear` model for inflation can be customized to
//!   define different segments and corresponding rates. It is advisable to have governance-approved
//!   settings for these curves to avoid manipulation that could favor specific validators or create
//!   unsustainable inflation dynamics.
//!
//! # Example Scenario
//!
//! Suppose a blockchain network aims for a yearly inflation rate that adjusts dynamically based on
//! the staking rate. At the end of an era, the `compute_total_payout` function is invoked to determine
//! the total reward to distribute among validators and their cooperators. The function calculates the
//! payout based on the current staking rate, ensuring that the reward is proportional to the number
//! of tokens staked in relation to the total supply. If staking participation is low, the inflation
//! curve might increase rewards to incentivize more staking, whereas high participation could reduce
//! the rewards to control inflation. This approach helps maintain a balanced and fair staking
//! ecosystem, encouraging participation without over-inflating the token supply.
//!


//! This module expose one function `P_NPoS` (Payout NPoS) or `compute_total_payout` which returns
//! the total payout for the era given the era duration and the staking rate in NPoS.
//! The staking rate in NPoS is the total amount of tokens staked by cooperators and validators,
//! divided by the total token supply.

use sp_runtime::{curve::PiecewiseLinear, traits::AtLeast32BitUnsigned, Perbill};

/// The total payout to all validators (and their cooperators) per era and maximum payout.
///
/// Defined as such:
/// `staker-payout = yearly_inflation(npos_token_staked / total_tokens) * total_tokens /
/// era_per_year` `maximum-payout = max_yearly_inflation * total_tokens / era_per_year`
///
/// `era_duration` is expressed in millisecond.
pub fn compute_total_payout<N>(
    yearly_inflation: &PiecewiseLinear<'static>,
    npos_token_staked: N,
    total_tokens: N,
    era_duration: u64,
) -> (N, N)
where
    N: AtLeast32BitUnsigned + Clone,
{
    // Milliseconds per year for the Julian year (365.25 days).
    const MILLISECONDS_PER_YEAR: u64 = 1000 * 3600 * 24 * 36525 / 100;

    let portion = Perbill::from_rational(era_duration, MILLISECONDS_PER_YEAR);
    let payout = portion
        * yearly_inflation
            .calculate_for_fraction_times_denominator(npos_token_staked, total_tokens.clone());
    let maximum = portion * (yearly_inflation.maximum * total_tokens);
    (payout, maximum)
}

#[cfg(test)]
mod test {
    use sp_runtime::curve::PiecewiseLinear;

    pallet_staking_reward_curve::build! {
        const I_NPOS: PiecewiseLinear<'static> = curve!(
            min_inflation: 0_025_000,
            max_inflation: 0_100_000,
            ideal_stake: 0_500_000,
            falloff: 0_050_000,
            max_piece_count: 40,
            test_precision: 0_005_000,
        );
    }

    #[test]
    fn npos_curve_is_sensible() {
        const YEAR: u64 = 365 * 24 * 60 * 60 * 1000;

        // check maximum inflation.
        // not 10_000 due to rounding error.
        assert_eq!(super::compute_total_payout(&I_NPOS, 0, 100_000u64, YEAR).1, 9_993);

        // super::I_NPOS.calculate_for_fraction_times_denominator(25, 100)
        assert_eq!(super::compute_total_payout(&I_NPOS, 0, 100_000u64, YEAR).0, 2_498);
        assert_eq!(super::compute_total_payout(&I_NPOS, 5_000, 100_000u64, YEAR).0, 3_248);
        assert_eq!(super::compute_total_payout(&I_NPOS, 25_000, 100_000u64, YEAR).0, 6_246);
        assert_eq!(super::compute_total_payout(&I_NPOS, 40_000, 100_000u64, YEAR).0, 8_494);
        assert_eq!(super::compute_total_payout(&I_NPOS, 50_000, 100_000u64, YEAR).0, 9_993);
        assert_eq!(super::compute_total_payout(&I_NPOS, 60_000, 100_000u64, YEAR).0, 4_379);
        assert_eq!(super::compute_total_payout(&I_NPOS, 75_000, 100_000u64, YEAR).0, 2_733);
        assert_eq!(super::compute_total_payout(&I_NPOS, 95_000, 100_000u64, YEAR).0, 2_513);
        assert_eq!(super::compute_total_payout(&I_NPOS, 100_000, 100_000u64, YEAR).0, 2_505);

        const DAY: u64 = 24 * 60 * 60 * 1000;
        assert_eq!(super::compute_total_payout(&I_NPOS, 25_000, 100_000u64, DAY).0, 17);
        assert_eq!(super::compute_total_payout(&I_NPOS, 50_000, 100_000u64, DAY).0, 27);
        assert_eq!(super::compute_total_payout(&I_NPOS, 75_000, 100_000u64, DAY).0, 7);

        const SIX_HOURS: u64 = 6 * 60 * 60 * 1000;
        assert_eq!(super::compute_total_payout(&I_NPOS, 25_000, 100_000u64, SIX_HOURS).0, 4);
        assert_eq!(super::compute_total_payout(&I_NPOS, 50_000, 100_000u64, SIX_HOURS).0, 7);
        assert_eq!(super::compute_total_payout(&I_NPOS, 75_000, 100_000u64, SIX_HOURS).0, 2);

        const HOUR: u64 = 60 * 60 * 1000;
        assert_eq!(
            super::compute_total_payout(
                &I_NPOS,
                2_500_000_000_000_000_000_000_000_000u128,
                5_000_000_000_000_000_000_000_000_000u128,
                HOUR
            )
            .0,
            57_038_500_000_000_000_000_000
        );
    }
}
