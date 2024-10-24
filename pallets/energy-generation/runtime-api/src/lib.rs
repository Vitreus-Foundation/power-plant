///
/// # Script Overview
///
/// Defines a runtime API for the `EnergyGeneration` pallet in a Substrate-based blockchain. It provides functionality to query information related to energy generation and reputation-based rewards directly from the runtime. This runtime API is intended to be exposed to the node for other modules or external clients to interact with.
///
/// # Key Components
///
/// - **Runtime API Declaration**:
///   - The `EnergyGenerationApi` is declared using `sp_api::decl_runtime_apis!`, which is a macro provided by Substrate to define custom runtime APIs.
///   - `fn reputation_tier_additional_reward(tier: ReputationTier) -> Perbill`: This function returns the additional reward (`Perbill`) for a given `ReputationTier`.
///   - `fn current_energy_per_stake_currency() -> u128`: This function returns the current energy value per stake currency as an unsigned 128-bit integer.
///
/// - **Dependencies**:
///   - `pallet_reputation`: This pallet defines the `ReputationTier` type, which is used as an input for querying rewards based on user reputation.
///   - `sp_runtime::Perbill`: Represents a fraction with a fixed denominator (one billion), used for defining precise reward rates or percentages.
///
/// # Developer Notes
///
/// - The `#![cfg_attr(not(feature = "std"), no_std)]` attribute ensures that the code is compatible with both `std` and `no_std` environments, which is necessary for runtime modules to work on a blockchain node.
/// - This runtime API is typically used by the client-side code or other pallets to obtain specific metrics related to energy generation or reward calculations.
/// - The `EnergyGenerationApi` is implemented by the runtime and is used by off-chain components, such as RPC servers or other runtime-dependent queries, to retrieve specific blockchain data.
///
/// # Usage
///
/// - **Integration**: This script is designed to be included in a Substrate runtime, where the `EnergyGenerationApi` provides access to energy generation metrics and reward information based on reputation tiers.
/// - **Extending Functionality**: Developers can add more methods to the `EnergyGenerationApi` to expose additional runtime functionality that is relevant for energy generation, reputation systems, or other related aspects of the blockchain.
///
/// # Example Scenario
///
/// A user with a specific `ReputationTier` can query the `reputation_tier_additional_reward` function to determine the extra rewards they are eligible for based on their reputation level. Similarly, stakeholders can use `current_energy_per_stake_currency` to understand the current energy-to-currency conversion rate in the system.


#![cfg_attr(not(feature = "std"), no_std)]

use pallet_reputation::ReputationTier;
use sp_runtime::Perbill;

sp_api::decl_runtime_apis! {
    pub trait EnergyGenerationApi
    {
        fn reputation_tier_additional_reward(tier: ReputationTier) -> Perbill;

        fn current_energy_per_stake_currency() -> u128;
    }
}
