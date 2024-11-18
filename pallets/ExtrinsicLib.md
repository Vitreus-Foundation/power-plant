## Introduction

Welcome to the **Vitreus Extrinsic Library**. This document provides a detailed overview of the extrinsics and pallets available in the Vitreus blockchain ecosystem. Each pallet offers specific functionalities that empower developers and users to interact with the blockchain, whether it's transferring assets, executing governance actions, or engaging with advanced features like cross-chain messaging.

The purpose of this library is to serve as a foundational resource, guiding users through the available extrinsics and how they can be leveraged to build, manage, and scale within Vitreus. With each pallet, we break down the key operations, common use cases, and technical details to ensure a clear understanding of how to effectively utilize these tools in real-world scenarios.

As Vitreus evolves, so too will this library, growing to include new features and updates as the blockchain ecosystem expands. Whether you're a developer looking for technical specifications or a non-developer seeking to understand the capabilities of Vitreus, this library provides the necessary knowledge to navigate and harness the power of the Vitreus blockchain.

---

### Claiming Pallet Documentation

**Version:** 0.1.0  
**Last Updated:** February 2024

## Overview
The Claiming pallet allows users to claim tokens that have been allocated to them, typically tied to an Ethereum address. It verifies the claim using signatures and may mint new tokens for claimants if necessary. This pallet is essential for token distribution and claim processes.

---

### Quick Reference

#### Key Features
- **Claim Tokens**: Allows users to claim tokens allocated to their Ethereum address.
- **Mint Claimable Tokens**: Administrators can mint additional tokens to the pool of claimable assets.
- **Vesting**: Claimed tokens may be subject to a vesting schedule.

#### Common Use Cases
- Claiming tokens after participating in a token sale or airdrop.
- Increasing the pool of claimable tokens for new users.
- Managing vesting schedules for claimed tokens.

---

## For Non-Developers 🌟

### What is the Claiming Pallet?
The Claiming pallet lets users claim tokens that have been set aside for them, typically tied to their Ethereum address. The process verifies the user’s ownership of the address and checks for any applicable vesting schedules before releasing the tokens.

### Key Concepts
- **Claim**: The process of receiving tokens that have been reserved for a specific address.
- **Vesting**: A schedule that gradually releases tokens over time.
- **Minting**: Adding more tokens to the claimable pool by administrators.

### Available Operations

#### Claim Tokens
- **What it does**: Claims tokens that have been allocated to your Ethereum address.
- **When to use it**: Use this when you want to claim your tokens using your Ethereum wallet signature as proof of ownership.
- **Example**: Alice claims her tokens from the token sale using her Ethereum signature.
- **Important to know**: If vesting is in place, your tokens will be gradually released over time.

#### Mint Tokens to Claim
- **What it does**: Mints additional tokens to the pool of claimable tokens.
- **When to use it**: This is an administrative function used when the pool of claimable tokens needs to be increased.
- **Example**: Governance mints more tokens to be claimable after a new token distribution round.
- **Important to know**: Only root or privileged accounts can mint tokens.

---

## For Developers 💻

### Technical Overview
The Claiming pallet provides functionality for claiming tokens using a signature tied to an Ethereum address. It includes administrative functions for minting new tokens into the claimable pool and can also handle vesting schedules for claimed tokens. The process uses Ethereum’s ECDSA signature verification standard.

### Integration Points
This pallet integrates with the runtime’s currency system to handle minting and token distribution. It also relies on signature verification to ensure that only rightful claimants can receive tokens.

### Extrinsics

#### claim(origin, ethereum_signature)
- **Purpose**: Claims tokens that have been allocated to the caller’s Ethereum address.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the claim.
  - `ethereum_signature (EcdsaSignature)`: The signature from the Ethereum address that owns the claim.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Claiming::claim(origin, ethereum_signature)?;
  ```

**Technical Details**:
- Origin must be signed (`ensure_signed`).
- Uses `secp256k1_ecdsa_recover` for signature verification.
- Checks if vesting applies, and if so, applies the vesting schedule.
- Verifies the claim against the `Claims` storage map.
- Clears the claim after a successful claim.
- Emits relevant events.

#### mint_tokens_to_claim(origin, amount)
- **Purpose**: Mints additional tokens into the pool of claimable tokens.
- **Parameters**:
  - `origin (OriginFor<T>)`: The root account minting the tokens.
  - `amount (BalanceOf<T>)`: The amount of tokens to mint.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Claiming::mint_tokens_to_claim(origin, amount)?;
  ```

**Technical Details**:
- Origin must be root (`ensure_root`).
- Uses `Currency::deposit_creating` for minting tokens.
- Updates the total claimable tokens in storage.
- Emits `TokenMintedToClaim` event.

### Security Considerations 🔒
- **Signature Verification**: Ensure that signatures are correctly verified to prevent unauthorized claims.
- **Minting Controls**: Only privileged accounts should be allowed to mint tokens to prevent inflation or misuse.
- **Vesting Schedules**: Ensure vesting schedules are accurately enforced to prevent premature access to tokens.

---

### Best Practices
1. **Validate Signatures**: Make sure signatures are properly validated before releasing tokens to ensure rightful claims.
2. **Secure Minting**: Ensure that only trusted accounts can mint tokens to the claimable pool.
3. **Handle Vesting Carefully**: Ensure that vesting schedules are adhered to, preventing premature access to tokens.

---

### EnergyBroker Pallet Documentation

**Version:** 0.1.0   
**Last Updated:** August 2024

## Overview
The EnergyBroker pallet facilitates decentralized asset management, including liquidity provisioning, token swaps, and asset trading. It handles various aspects of trading pairs, managing liquidity pools, and swapping tokens using set conversion formulas. The pallet supports precise calculations for token amounts during trades and liquidity adjustments.

---

### Quick Reference

#### Key Features
- **Create and Manage Liquidity Pools**: Set up and manage pools of assets for decentralized trading.
- **Token Swaps**: Allows users to swap one token for another within the liquidity pools.
- **Liquidity Adjustments**: Add or remove liquidity from pools, managing market dynamics.
- **Support for Custom Trading Algorithms**: Uses algorithms for asset conversion and swap calculations.

#### Common Use Cases
- Creating a new liquidity pool for a trading pair.
- Swapping tokens between two assets in a decentralized manner.
- Adding or removing liquidity to earn fees or rebalance a pool.
- Calculating token outputs based on set formulas during trades.

---

## For Non-Developers 🌟

### What is the EnergyBroker Pallet?
The EnergyBroker pallet allows users to create and manage liquidity pools, where assets can be traded in a decentralized way. Users can swap tokens, add liquidity to pools to earn fees, or remove liquidity as needed. It uses formulas to calculate how much of each asset should be traded or provided, similar to how a traditional exchange works but without a central authority.

### Key Concepts
- **Liquidity Pool**: A pool of tokens used for decentralized trading.
- **Token Swap**: The process of exchanging one token for another within a liquidity pool.
- **Add/Remove Liquidity**: Contributing or withdrawing tokens from a liquidity pool.
- **Trading Formula**: A mathematical formula that calculates how much of one token is needed to trade for another.

### Available Operations

#### Add Liquidity
- **What it does**: Adds tokens to a liquidity pool, increasing the pool size and enabling more trades.
- **When to use it**: Use this when you want to earn fees by providing liquidity to a trading pair.
- **Example**: Alice adds 100 tokens of Asset A and 200 tokens of Asset B to the A/B liquidity pool.
- **Important to know**: You can earn a portion of the fees generated from trades in the pool.

#### Remove Liquidity
- **What it does**: Withdraws tokens from a liquidity pool, reducing the pool size.
- **When to use it**: Use this when you want to reclaim your tokens from the pool, along with any fees earned.
- **Example**: Bob removes his tokens from the A/B liquidity pool, receiving his contribution plus a share of the fees.
- **Important to know**: Removing liquidity may impact the pool’s balance and trading dynamics.

#### Swap Tokens
- **What it does**: Trades one token for another using a liquidity pool.
- **When to use it**: Use this when you want to exchange one token for another without using a centralized exchange.
- **Example**: Alice swaps 10 tokens of Asset A for 5 tokens of Asset B using the A/B pool.
- **Important to know**: The amount of tokens received depends on the current pool size and market conditions.

---

## For Developers 💻

### Technical Overview
The EnergyBroker pallet manages decentralized asset swaps and liquidity pools. It implements core functionalities like token swaps, liquidity provisioning, and conversion algorithms to ensure the smooth operation of decentralized trading. The pallet supports precise calculations to determine token amounts during trades and liquidity adjustments, enabling users to manage liquidity pools and trade assets effectively.

### Integration Points
This pallet integrates with the system’s asset management and token transfer modules to facilitate trades and liquidity management. It also works with pricing algorithms and custom formulas to calculate the exact token amounts during swaps.

### Extrinsics

#### addLiquidity(origin, asset_a, asset_b, amount_a, amount_b)
- **Purpose**: Adds liquidity to a pool consisting of two assets.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account providing the liquidity.
  - `asset_a (T::AssetId)`: The first asset in the trading pair.
  - `asset_b (T::AssetId)`: The second asset in the trading pair.
  - `amount_a (BalanceOf<T>)`: The amount of the first asset to add.
  - `amount_b (BalanceOf<T>)`: The amount of the second asset to add.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyBroker::addLiquidity(origin, asset_a, asset_b, amount_a, amount_b)?;
  ```

#### removeLiquidity(origin, asset_a, asset_b, liquidity)
- **Purpose**: Removes liquidity from a pool, withdrawing tokens and any earned fees.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account removing liquidity.
  - `asset_a (T::AssetId)`: The first asset in the trading pair.
  - `asset_b (T::AssetId)`: The second asset in the trading pair.
  - `liquidity (BalanceOf<T>)`: The liquidity tokens to burn.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyBroker::removeLiquidity(origin, asset_a, asset_b, liquidity)?;
  ```

#### swapExactTokensForTokens(origin, amount_in, asset_in, asset_out)
- **Purpose**: Swaps an exact amount of one token for another within a pool.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account initiating the swap.
  - `amount_in (BalanceOf<T>)`: The exact amount of tokens to swap.
  - `asset_in (T::AssetId)`: The asset being swapped.
  - `asset_out (T::AssetId)`: The asset to receive.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyBroker::swapExactTokensForTokens(origin, amount_in, asset_in, asset_out)?;
  ```

### Security Considerations 🔒
- **Liquidity Risk**: Ensure proper liquidity management to avoid imbalances in trading pools.
- **Swap Calculations**: Double-check the correctness of conversion formulas to prevent calculation errors.
- **Access Control**: Restrict access to sensitive liquidity pool operations to avoid misuse.

---

### Best Practices
1. **Monitor Liquidity**: Regularly monitor pool liquidity to maintain healthy trading conditions.
2. **Use Accurate Formulas**: Ensure conversion formulas are accurate to prevent issues during trades.
3. **Provide Liquidity Wisely**: Only add liquidity to pools where you understand the risks and potential rewards.

---

### EnergyFee Pallet Documentation

**Version:** 0.1.0  
**Last Updated:** November 2024

## Overview
The EnergyFee pallet is responsible for managing the network's fee structure by adjusting thresholds and multipliers that control how fees are applied during transactions. This pallet allows administrators to dynamically adjust fees based on network load, block fullness, and other factors, ensuring smooth operation during both peak and off-peak times.

---

### Quick Reference

#### Key Features
- **Update Burned Energy Threshold**: Modify the threshold for burning energy units.
- **Update Block Fullness Threshold**: Adjust the network’s block fullness limit.
- **Set Fee Multiplier**: Control the upper limit of fee multipliers based on network usage.
- **Adjust Base Fee**: Update the base transaction fee for the network.

#### Common Use Cases
- Increasing the energy burn threshold during high-traffic periods to maintain efficiency.
- Reducing block fullness thresholds to manage congestion.
- Setting a higher fee multiplier to prevent spam transactions.
- Updating the base fee to reflect network changes or increased traffic.

---

## For Non-Developers 🌟

### What is the EnergyFee Pallet?
The EnergyFee pallet controls how transaction fees are calculated and applied on the blockchain. It allows network administrators to adjust fees dynamically based on network load, helping prevent congestion during busy times. By managing energy thresholds, fee multipliers, and base fees, the pallet ensures that the network remains stable and efficient.

### Key Concepts
- **Burned Energy Threshold**: The minimum amount of energy that must be burned before a transaction is processed.
- **Block Fullness**: A metric indicating how full a block is, which helps determine if fees should be adjusted.
- **Fee Multiplier**: A scaling factor applied to transaction fees during high-traffic periods.
- **Base Fee**: The minimum fee required for any transaction to be included in a block.

### Available Operations

#### Update Burned Energy Threshold
- **What it does**: Adjusts the threshold for burning energy units before a transaction is processed.
- **When to use it**: Use this when network traffic increases and you need to manage energy consumption more effectively.
- **Example**: Increasing the energy burn threshold during a period of high traffic to maintain network performance.
- **Important to know**: Setting the threshold too high may prevent smaller transactions from being processed.

#### Update Block Fullness Threshold
- **What it does**: Sets the block fullness threshold, which determines when fees are adjusted based on how full a block is.
- **When to use it**: Use this when you need to prevent blocks from becoming too congested during high-traffic periods.
- **Example**: Reducing the block fullness threshold to 70% during a network upgrade.
- **Important to know**: A lower threshold may cause fees to increase more rapidly as blocks fill up.

#### Update Fee Multiplier
- **What it does**: Sets the upper limit for the fee multiplier, which is applied when network traffic reaches certain levels.
- **When to use it**: Use this to prevent spam transactions during high-traffic periods by increasing the cost of transactions.
- **Example**: Setting a fee multiplier of 3x during peak times to reduce spam.
- **Important to know**: Setting the multiplier too high may deter legitimate transactions.

#### Update Base Fee
- **What it does**: Adjusts the base fee required for all transactions.
- **When to use it**: Use this to reflect changes in network conditions or when the cost of maintaining the network increases.
- **Example**: Increasing the base fee to 100 units to manage higher demand.
- **Important to know**: Ensure that the base fee remains accessible for everyday users while still covering network costs.

---

## For Developers 💻

### Technical Overview
The EnergyFee pallet manages key parameters that govern the blockchain's fee structure. It allows administrators to update the burned energy threshold, block fullness threshold, fee multipliers, and base fees to ensure the network operates efficiently under varying conditions. This flexibility helps maintain network stability by adjusting fees based on real-time traffic.

### Integration Points
The EnergyFee pallet interacts with the transaction pool, block production logic, and governance system. It is used to manage how fees are applied to transactions based on block fullness and network congestion, ensuring that resources are allocated efficiently.

### Extrinsics

#### updateBurnedEnergyThreshold(origin, threshold)
- **Purpose**: Adjusts the energy threshold for burning units.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the request.
  - `threshold (BalanceOf<T>)`: The new threshold for energy burning.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyFee::updateBurnedEnergyThreshold(origin, 1000)?;
  ```

#### updateBlockFullnessThreshold(origin, threshold)
- **Purpose**: Updates the block fullness threshold.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the request.
  - `threshold (Perquintill)`: The new block fullness threshold (from 0 to 1_000_000).
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyFee::updateBlockFullnessThreshold(origin, 800_000)?;
  ```

#### updateUpperFeeMultiplier(origin, multiplier)
- **Purpose**: Updates the maximum fee multiplier.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the request.
  - `multiplier (FixedPointNumber)`: The new fee multiplier.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyFee::updateUpperFeeMultiplier(origin, 2_000_000_000)?;
  ```

#### updateBaseFee(origin, base_fee)
- **Purpose**: Adjusts the base fee for all transactions.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the request.
  - `base_fee (BalanceOf<T>)`: The new base fee.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyFee::updateBaseFee(origin, 100)?;
  ```

### Security Considerations 🔒
- **Fee Adjustments**: Ensure that only trusted accounts can update fee parameters to prevent malicious manipulation.
- **Threshold Sensitivity**: Adjust thresholds carefully to avoid disrupting the normal flow of transactions.
- **Multipliers**: Setting too high a multiplier can price out legitimate users, while too low a multiplier may not prevent spam.

---

### Best Practices
1. **Monitor Network Traffic**: Adjust the block fullness threshold and fee multipliers based on real-time network usage to prevent congestion.
2. **Balance Fees**: Ensure base fees are reasonable and accessible, while still preventing spam and covering network costs.
3. **Use Safe Multipliers**: Set fee multipliers that are high enough to deter spam, but not so high that they exclude normal users.

---

### EnergyGeneration Pallet Documentation

**Version:** 0.1.0  
**Last Updated:** November 2024

## Overview
The EnergyGeneration pallet governs the production of energy in the network, the distribution of rewards, and the management of reputations for validators and cooperators. It handles dynamic energy rates, reward calculations, and slashing events for misconduct such as offline behavior or energy manipulation. The pallet ensures fair energy distribution and penalizes validators or cooperators who violate network rules.

---

### Quick Reference

#### Key Features
- **Energy Rate Calculation**: Determine energy rates based on validators' performance and reputation tiers.
- **Reputation Management**: Adjust validator and cooperator reputations to incentivize good behavior.
- **Rewards Distribution**: Calculate and distribute rewards based on energy production, reputation multipliers, and commissions.
- **Slashing Mechanism**: Penalize validators and cooperators for misconduct, such as going offline or manipulating energy rates.

#### Common Use Cases
- Rewarding validators for generating energy based on their reputation and contributions.
- Slashing validators or cooperators for network violations.
- Updating energy rates dynamically as validators generate and contribute energy.

---

## For Non-Developers 🌟

### What is the EnergyGeneration Pallet?
The EnergyGeneration pallet handles the distribution of rewards and penalties related to energy production. Validators earn rewards based on how much energy they generate and their reputation tier. If validators violate the rules (e.g., going offline or manipulating energy rates), they can be penalized. This ensures that the network operates smoothly, with validators being incentivized to generate energy and maintain a positive reputation.

### Key Concepts
- **Energy Rate**: The amount of energy generated per unit of stake.
- **Reputation**: A ranking system that multiplies rewards based on validator performance.
- **Rewards**: Tokens distributed to validators and cooperators based on their energy contributions and reputation.
- **Slashing**: A penalty mechanism that reduces a validator’s or cooperator’s stake for misconduct.

### Available Operations

#### Update Energy Rate
- **What it does**: Adjusts the energy rate for validators, affecting their rewards.
- **When to use it**: Use this to adjust how much energy is produced per stake unit.
- **Example**: Increasing the energy rate to incentivize more validator participation.
- **Important to know**: Higher energy rates result in higher rewards for validators.

#### Adjust Reputation Tier
- **What it does**: Updates the reputation of a validator, which affects their reward multiplier.
- **When to use it**: Use this to incentivize better performance from validators.
- **Example**: Increasing the reputation tier for a validator that has consistently contributed to energy production.
- **Important to know**: Validators with higher reputations receive more rewards for the same amount of energy produced.

#### Apply Slashing
- **What it does**: Penalizes validators or cooperators by reducing their stake.
- **When to use it**: Use this when a validator goes offline or attempts to manipulate the energy system.
- **Example**: Slashing a validator that was caught manipulating their energy production rates.
- **Important to know**: Slashing reduces the amount of tokens held by the validator or cooperator.

---

## For Developers 💻

### Technical Overview
The EnergyGeneration pallet controls how energy is produced and rewarded in the network. It uses algorithms to calculate energy rates, reputation multipliers, and rewards distribution, while also implementing slashing mechanisms for misconduct. It interacts with other runtime modules to ensure validators are properly incentivized and penalized based on their behavior and performance.

### Integration Points
The EnergyGeneration pallet integrates with the staking and reputation modules to adjust energy rates and manage rewards based on validator performance. It also interacts with slashing mechanisms to apply penalties when validators violate the rules.

### Extrinsics

#### updateEnergyRate(origin, new_rate)
- **Purpose**: Adjusts the energy rate for validators, affecting their rewards.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the request.
  - `new_rate (BalanceOf<T>)`: The new energy rate.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyGeneration::updateEnergyRate(origin, 1500)?;
  ```

#### adjustReputation(origin, validator, new_tier)
- **Purpose**: Updates the reputation tier for a validator.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the request.
  - `validator (T::AccountId)`: The validator whose reputation is being updated.
  - `new_tier (ReputationTier)`: The new reputation tier.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyGeneration::adjustReputation(origin, validator, 3)?;
  ```

#### slashValidator(origin, validator, amount)
- **Purpose**: Applies slashing to a validator for misconduct.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the request.
  - `validator (T::AccountId)`: The validator to be slashed.
  - `amount (BalanceOf<T>)`: The amount of tokens to be slashed.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = EnergyGeneration::slashValidator(origin, validator, 500)?;
  ```

### Security Considerations 🔒
- **Energy Manipulation**: Ensure validators cannot manipulate energy production by setting strict controls on how energy rates are adjusted.
- **Reputation Integrity**: Ensure that reputation adjustments are accurate and transparent to avoid favoritism.
- **Slashing Enforcement**: Apply slashing fairly and consistently to prevent misuse of the system.

---

### Best Practices
1. **Monitor Energy Rates**: Adjust energy rates periodically to ensure validators are properly incentivized based on current network conditions.
2. **Maintain Reputation**: Regularly update validator reputations to reflect their ongoing performance.
3. **Apply Slashing Fairly**: Ensure slashing is applied consistently and transparently to maintain trust in the system.

---

### nacManaging Pallet Documentation

**Version:** 0.1.0  
**Last Updated:** October 2024

## Overview
The **nacManaging** pallet is responsible for managing Non-Fungible Access Certificates (NACs), which provide special access rights or privileges to users. The pallet includes functionality for minting new NACs, updating NAC levels, and checking the current NAC level of an account. NAC levels can influence the staking, validation, or other capabilities of an account within the network.

---

### Quick Reference

#### Key Features
- **Mint NAC**: Create new Non-Fungible Access Certificates for accounts.
- **Update NAC Level**: Modify the level of an existing NAC for an account.
- **Check NAC Level**: Verify the current NAC level assigned to an account.

#### Common Use Cases
- Minting NACs for new users to grant access to specific features or roles within the network.
- Updating the NAC level for users who have earned higher access or validation privileges.
- Checking the current NAC level to determine a user’s status or eligibility for certain actions.

---

## For Non-Developers 🌟

### What is the nacManaging Pallet?
The **nacManaging** pallet is used to create and manage Non-Fungible Access Certificates (NACs) within the network. NACs give users special privileges or access, similar to how NFTs work, but they are tied to account capabilities and access rights. Administrators can mint new NACs, update existing NAC levels, and check what level an account has.

### Key Concepts
- **NAC (Non-Fungible Access Certificate)**: A certificate tied to an account that gives specific privileges or access rights.
- **NAC Level**: The level of the NAC, which determines the scope of access or rights granted to the account.
- **Minting**: The process of creating a new NAC for an account.
- **Admin Origin**: The permission required to mint or modify NACs, ensuring only authorized actors can manage access certificates.

### Available Operations

#### Mint NAC
- **What it does**: Mints a new NAC for an account, granting it specific access or privileges.
- **When to use it**: Use this to create a new NAC for a user when they join the network or gain new rights.
- **Example**: Alice mints a NAC for Bob, granting him VIP status in the system.
- **Important to know**: Only administrators can mint new NACs.

#### Update NAC Level
- **What it does**: Updates the level of an existing NAC, modifying the account’s access or privileges.
- **When to use it**: Use this when a user’s access needs to be updated, such as when they are promoted or demoted.
- **Example**: Alice updates Bob's NAC to level 2, giving him additional privileges in the network.
- **Important to know**: NAC levels should be validated to ensure appropriate progression.

#### Check NAC Level
- **What it does**: Retrieves the current NAC level for an account.
- **When to use it**: Use this to verify the access level of an account.
- **Example**: Alice checks Bob's NAC level to confirm his eligibility for VIP access.
- **Important to know**: Only administrators can check NAC levels.

---

## For Developers 💻

### Technical Overview
The **nacManaging** pallet enables the creation and management of Non-Fungible Access Certificates (NACs) for user accounts. NACs can have different levels that grant specific privileges, and the pallet includes functionality to mint new NACs, update NAC levels, and verify existing NACs. This pallet ensures that NACs are tied to the account's privileges and influence their interaction with the system.

### Integration Points
The **nacManaging** pallet integrates with account and identity modules to manage access certificates. It interacts with the staking and governance systems, as NAC levels can influence account permissions or roles.

### Extrinsics

#### mint(origin, level, target_account)
- **Purpose**: Mints a new NAC for the specified account.
- **Parameters**:
  - `origin (OriginFor<T>)`: The admin account making the request.
  - `level (u32)`: The level of the NAC to mint.
  - `target_account (T::AccountId)`: The account to which the NAC will be assigned.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = nacManaging::mint(origin, 1, target_account)?;
  ```

#### updateNft(origin, level, target_account)
- **Purpose**: Updates the level of an existing NAC for the specified account.
- **Parameters**:
  - `origin (OriginFor<T>)`: The admin account making the request.
  - `level (Option<u32>)`: The new NAC level (None if unchanged).
  - `target_account (T::AccountId)`: The account whose NAC will be updated.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = nacManaging::updateNft(origin, Some(2), target_account)?;
  ```

#### checkNacLevel(origin, target_account)
- **Purpose**: Retrieves the NAC level of the specified account.
- **Parameters**:
  - `origin (OriginFor<T>)`: The admin account making the request.
  - `target_account (T::AccountId)`: The account to check for NAC level.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = nacManaging::checkNacLevel(origin, target_account)?;
  ```

### Security Considerations 🔒
- **Admin Origin Required**: Ensure only authorized administrators have access to mint, update, or check NACs.
- **NAC Validation**: Verify that level changes are appropriate to avoid conflicts in user privileges.
- **Auditability**: Full event emission for minting, updating, or checking NACs ensures proper auditing and tracking of access changes.

---

### Best Practices
1. **Ensure Admin Control**: Limit access to minting and updating NACs to trusted accounts with admin privileges.
2. **Monitor NAC Levels**: Regularly review NAC levels to ensure appropriate access is granted to accounts.
3. **Audit Changes**: Use event logging to keep a record of all NAC minting and updates for auditing purposes.

---

### Privileges Pallet Documentation

**Version:** 1.0  
**Last Updated:** October 2024

## Overview
The **Privileges** pallet is responsible for managing VIP and VIPP memberships in the network. It tracks contributions, calculates loyalty points, manages membership details, and applies penalties or rewards based on the behavior and activity of members. This pallet is vital for maintaining a system of privileges and rewards for key contributors in a decentralized system.

---

### Quick Reference

#### Key Features
- **VIP Membership Management**: Track and manage membership status based on contributions and energy generation.
- **Loyalty Points**: Accumulate points for consistent contribution and participation in the network.
- **Tax and Penalty System**: Apply tax rules and penalties based on activity, with different rules for each quarter.
- **Claim Rewards**: Update membership and points when members claim rewards for their contributions.

#### Common Use Cases
- Adding new VIP members to the network based on their contribution and stake.
- Updating membership details to reflect recent activity and loyalty points.
- Applying penalties or rewards based on user activity and the tax system.
- Allowing members to claim rewards, impacting their status in the system.

---

## For Non-Developers 🌟

### What is the Privileges Pallet?
The **Privileges** pallet manages VIP memberships in a blockchain network. It tracks members' contributions, energy generation, and overall reputation to determine their status. Members earn loyalty points, can face penalties or enjoy perks based on their behavior, and can claim rewards that affect their status in the system. This pallet ensures that the network's key contributors are properly recognized and rewarded.

### Key Concepts
- **VIP Membership**: A special status granted to key contributors in the network, based on their stake, contributions, and activity.
- **Loyalty Points**: Points accumulated by members for consistent participation, which influence their status.
- **Tax System**: Each VIP member has an associated tax type that determines the penalties or perks they receive based on their activity.
- **Claiming Rewards**: Members can claim rewards, and their VIP status or loyalty points will be updated accordingly.

### Available Operations

#### Add New VIP Member
- **What it does**: Adds a new VIP member to the network, specifying details such as their stake and contribution information.
- **When to use it**: Use this when adding a new contributor who meets the criteria for VIP membership.
- **Example**: Alice, a network administrator, adds Bob as a new VIP member based on his significant contributions.
- **Important to know**: This operation is restricted to root users to ensure proper control and security.

#### Update Quarterly Information
- **What it does**: Updates the quarterly contribution and stake information for all VIP members, ensuring their status is up to date.
- **When to use it**: Use this at the end of a quarter to recalculate VIP members' contributions and update their loyalty points.
- **Example**: At the end of Q1, Alice updates the quarterly information for all VIP members to reflect their recent contributions.
- **Important to know**: This ensures that loyalty points and tax information are accurately updated.

#### Claim Rewards
- **What it does**: Updates the membership status and loyalty points when a member claims rewards for their contributions.
- **When to use it**: Use this when a VIP member claims rewards based on their activity and contributions.
- **Example**: Bob claims his rewards, and his VIP status is updated to reflect his increased loyalty points.
- **Important to know**: Claiming rewards can impact a member's VIP status, loyalty points, and tax type.

---

## For Developers 💻

### Technical Overview
The **Privileges** pallet provides a system for managing and tracking VIP memberships within the network. It allows administrators to add members, calculate and update their contributions, manage their loyalty points, and handle tax and penalty systems. The pallet also manages the process of claiming rewards and updating member status based on their activity.

### Integration Points
The **Privileges** pallet integrates with various modules related to staking, reputation, and governance, as it manages the activity and status of key contributors in the network. It calculates membership status based on contributions and energy generation, applying penalties and rewards accordingly.

### Extrinsics

#### addNewVipMember(origin, member_account, stake, contribution_info, tax_type)
- **Purpose**: Adds a new VIP member with the given stake and contribution details.
- **Parameters**:
  - `origin (OriginFor<T>)`: The root account adding the new member.
  - `member_account (T::AccountId)`: The account of the new VIP member.
  - `stake (Balance)`: The initial stake of the VIP member.
  - `contribution_info (ContributionInfo)`: The contribution details of the new member.
  - `tax_type (TaxType)`: The tax type associated with the member.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Privileges::addNewVipMember(origin, member_account, stake, contribution_info, tax_type)?;
  ```

#### updateQuarterInfo(origin)
- **Purpose**: Updates the quarterly stake and contribution information for all VIP members.
- **Parameters**:
  - `origin (OriginFor<T>)`: The root account initiating the update.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Privileges::updateQuarterInfo(origin)?;
  ```

#### onClaim(origin, member_account)
- **Purpose**: Handles the claiming of rewards and updates the member's VIP status.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account making the claim.
  - `member_account (T::AccountId)`: The account of the VIP member claiming rewards.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Privileges::onClaim(origin, member_account)?;
  ```

### Security Considerations 🔒
- **Access Control**: Ensure only root users can add new VIP members or update membership details to avoid abuse.
- **Accurate Calculation**: Ensure that loyalty points and penalties are calculated accurately to reflect member contributions.
- **Claim Validation**: Carefully validate reward claims to ensure they match the member's activity and contribution.

---

### Best Practices
1. **Monitor VIP Membership**: Regularly update and monitor VIP membership status to ensure accurate reward distribution and penalties.
2. **Enforce Access Control**: Restrict access to membership management functions to authorized administrators.
3. **Validate Claims**: Regularly review claims to ensure that members' rewards and status reflect their actual contributions and behavior.

---

### Reputation Pallet Documentation

**Version:** 0.1.0  
**Last Updated:** September 2024

## Overview
The **Reputation** pallet manages user reputation scores, which reflect the reliability, activity, and contributions of users within the network. This pallet allows the network to assign reputation points based on user actions and behaviors, such as fulfilling tasks, participating in governance, or contributing to the network's overall health. Reputation scores are used to grant or restrict access to certain privileges or roles within the system.

---

### Quick Reference

#### Key Features
- **Assign Reputation Scores**: Award or deduct reputation points based on user activity.
- **Track User Contributions**: Monitor how users contribute to the network, such as through voting or task completion.
- **Manage Reputation Levels**: Define thresholds for various reputation levels that unlock specific privileges or roles.
- **Apply Penalties**: Automatically reduce reputation scores for negative behavior or rule violations.

#### Common Use Cases
- Rewarding users with higher reputation points for completing tasks or making positive contributions.
- Deducing points for users who break rules or exhibit negative behavior in the network.
- Tracking user activity and contributions over time to determine their reputation score.
- Using reputation levels to grant access to specific roles or privileges in the network.

---

## For Non-Developers 🌟

### What is the Reputation Pallet?
The **Reputation** pallet is used to assign, track, and manage reputation scores for users in the network. Reputation represents how much a user has contributed and how reliable they are. Higher reputation scores can unlock new privileges or roles, while low reputation can result in penalties. This is similar to how some online platforms reward users for good behavior and punish negative actions.

### Key Concepts
- **Reputation Score**: A numerical value that represents how trustworthy and reliable a user is within the network.
- **Reputation Level**: The category or rank a user falls into based on their reputation score (e.g., "Trusted", "Newbie", "Penalized").
- **Contributions**: Tasks, votes, or other actions that users perform in the network that impact their reputation score.
- **Penalties**: Negative actions or rule violations that result in a loss of reputation points.

### Available Operations

#### Assign Reputation Points
- **What it does**: Awards reputation points to users based on their contributions or activity.
- **When to use it**: Use this to reward users who have positively contributed to the network.
- **Example**: Alice gains reputation points after voting in a governance proposal.
- **Important to know**: Reputation points help users unlock new roles and privileges in the network.

#### Deduct Reputation Points
- **What it does**: Deducts reputation points from users who exhibit negative behavior or violate network rules.
- **When to use it**: Use this when users need to be penalized for bad behavior.
- **Example**: Bob loses reputation points after attempting to spam the network.
- **Important to know**: Low reputation scores can restrict access to certain features or roles.

#### Check Reputation Level
- **What it does**: Retrieves the current reputation level of a user based on their score.
- **When to use it**: Use this when you need to determine what privileges or roles a user has access to.
- **Example**: Alice checks her reputation level to see if she qualifies for a new governance role.
- **Important to know**: Reputation levels unlock specific privileges and are based on a user’s total reputation score.

---

## For Developers 💻

### Technical Overview
The **Reputation** pallet allows the network to track and manage user reputation by assigning scores based on actions and behavior. It integrates with governance and other task-oriented modules to award reputation points for positive contributions or deduct points for negative actions. Reputation levels are determined based on pre-set thresholds, which grant access to specific roles or privileges.

### Integration Points
The **Reputation** pallet integrates with governance, task management, and penalty systems to automatically adjust user reputation scores. It ensures that a user’s activity is properly monitored and reflected in their reputation level, which impacts their access to various network features.

### Extrinsics

#### assignReputationPoints(origin, user_account, points)
- **Purpose**: Awards reputation points to a user based on their contributions.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account assigning the reputation points.
  - `user_account (T::AccountId)`: The account of the user receiving the points.
  - `points (i32)`: The number of reputation points being awarded (can be positive or negative).
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Reputation::assignReputationPoints(origin, user_account, 100)?;
  ```

#### deductReputationPoints(origin, user_account, points)
- **Purpose**: Deducts reputation points from a user for negative actions.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account initiating the deduction.
  - `user_account (T::AccountId)`: The account of the user being penalized.
  - `points (i32)`: The number of reputation points to be deducted.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Reputation::deductReputationPoints(origin, user_account, -50)?;
  ```

#### checkReputationLevel(origin, user_account)
- **Purpose**: Retrieves the reputation level of the specified user.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account initiating the check.
  - `user_account (T::AccountId)`: The account of the user whose reputation is being checked.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = Reputation::checkReputationLevel(origin, user_account)?;
  ```

### Security Considerations 🔒
- **Accurate Scoring**: Ensure that reputation points are awarded and deducted fairly based on actual user behavior.
- **Threshold Validation**: Regularly review reputation thresholds to maintain fairness in how privileges are unlocked or lost.
- **Access Control**: Only allow trusted actors to assign or deduct reputation points to prevent abuse.

---

### Best Practices
1. **Monitor User Behavior**: Regularly track user activity to ensure that reputation scores accurately reflect contributions and penalties.
2. **Fair Penalties**: Ensure penalties for negative behavior are proportionate to the offense and communicated clearly to users.
3. **Update Reputation Regularly**: Consistently update user reputation scores to maintain an accurate reflection of their activity in the network.

---

### SimpleVesting Pallet Documentation

**Version:** 0.1.0  
**Last Updated:** October 2024

## Overview
The **SimpleVesting** pallet manages token vesting schedules, ensuring that locked tokens are gradually released over time. It allows for the creation of vesting schedules, forced vesting transfers, and the removal of vesting schedules. This ensures that tokens are unlocked in a controlled manner, which is often required for early contributors, investors, or other participants in token distributions.

---

### Quick Reference

#### Key Features
- **Vesting Schedules**: Create a schedule that gradually unlocks tokens over a specific period.
- **Force Vested Transfers**: Allow administrators to create vesting schedules by transferring vested tokens from one account to another.
- **Remove Vesting**: Force the removal of a vesting schedule, slashing any locked tokens if necessary.

#### Common Use Cases
- Setting up vesting schedules for contributors or team members to unlock their tokens gradually over time.
- Enforcing vesting schedules by transferring locked tokens from one account to another.
- Removing vesting schedules for users who need to have their vesting plan terminated, with possible slashing of locked tokens.

---

## For Non-Developers 🌟

### What is the SimpleVesting Pallet?
The **SimpleVesting** pallet helps manage the release of tokens over time. Vesting schedules ensure that tokens are gradually unlocked according to a preset schedule, making sure users cannot access all their tokens immediately. It’s typically used for early contributors, investors, or team members who receive tokens but need to wait for them to unlock over a specific time frame.

### Key Concepts
- **Vesting Schedule**: A plan that specifies how and when tokens will be unlocked over time.
- **Force Vested Transfer**: A way for administrators to move vested tokens from one account to another, applying a vesting schedule.
- **Remove Vesting**: The process of terminating a vesting schedule and slashing locked tokens if required.

### Available Operations

#### Vest Tokens
- **What it does**: Unlocks tokens for an account according to its vesting schedule.
- **When to use it**: Use this when a user has tokens locked under a vesting schedule and is eligible to receive some of them.
- **Example**: Alice's tokens are gradually released each block, and she can claim the unlocked amount after the vesting schedule completes.
- **Important to know**: The unlockable amount depends on the vesting schedule and the number of blocks that have passed.

#### Force Vested Transfer
- **What it does**: Transfers tokens from one account to another and applies a vesting schedule to the recipient.
- **When to use it**: Use this to set up a vesting schedule for a recipient who is receiving vested tokens.
- **Example**: Alice (an administrator) forces a vested transfer of 1,000 tokens from Bob to Charlie, with a schedule that unlocks 100 tokens per block.
- **Important to know**: This operation is restricted to root users to prevent unauthorized actions.

#### Remove Vesting Schedule
- **What it does**: Removes the vesting schedule from a user and slashes any remaining locked tokens.
- **When to use it**: Use this when a vesting schedule needs to be forcibly removed, typically due to non-compliance or other issues.
- **Example**: Alice (an administrator) removes the vesting schedule from Bob, slashing the remaining locked tokens.
- **Important to know**: This action can permanently remove access to the locked tokens, and only root users can execute it.

---

## For Developers 💻

### Technical Overview
The **SimpleVesting** pallet manages the creation, execution, and removal of vesting schedules. It enables the gradual unlocking of tokens over a predefined period based on block numbers. Administrators can use the pallet to create vesting schedules for specific users, enforce token locks, and manage the release of tokens. The pallet also allows forced vesting transfers and the removal of vesting schedules if necessary.

### Integration Points
The **SimpleVesting** pallet integrates with staking, governance, and other modules that may need to enforce token locks and controlled releases. It interacts with the runtime to calculate and release tokens based on the current block number.

### Extrinsics

#### vest(origin)
- **Purpose**: Unlocks tokens for an account according to the vesting schedule.
- **Parameters**:
  - `origin (OriginFor<T>)`: The account claiming the unlocked tokens.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = SimpleVesting::vest(RuntimeOrigin::signed(user_account))?;
  ```

#### forceVestedTransfer(origin, source, dest, schedule)
- **Purpose**: Forces a transfer of tokens from one account to another and applies a vesting schedule to the recipient.
- **Parameters**:
  - `origin (OriginFor<T>)`: The root account initiating the transfer.
  - `source (T::AccountId)`: The account from which the tokens are transferred.
  - `dest (T::AccountId)`: The recipient of the vested tokens.
  - `schedule (VestingInfo)`: The vesting schedule that defines how tokens will unlock.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = SimpleVesting::forceVestedTransfer(origin, source_account, dest_account, vesting_schedule)?;
  ```

#### forceRemoveVesting(origin, target)
- **Purpose**: Removes a vesting schedule from the target account and slashes any locked tokens.
- **Parameters**:
  - `origin (OriginFor<T>)`: The root account initiating the removal.
  - `target (T::AccountId)`: The account whose vesting schedule is being removed.
- **Returns**: `DispatchResult` indicating success or failure.
- **Example Usage**:
  ```rust
  let result = SimpleVesting::forceRemoveVesting(origin, target_account)?;
  ```

### Security Considerations 🔒
- **Enforcing Vesting**: Ensure that vesting schedules are properly enforced, especially for critical tokens.
- **Root Access**: Only allow trusted administrators to perform force vested transfers or remove vesting schedules.
- **Accurate Calculation**: Regularly check that vesting schedules are correctly applied and that unlockable amounts are calculated accurately.

---

### Best Practices
1. **Regularly Monitor Vesting**: Ensure that vesting schedules are accurately tracked and tokens are unlocked at the appropriate times.
2. **Protect Root Access**: Restrict access to force vested transfers and vesting removal to authorized users only.
3. **Validate Unlocking**: Regularly validate that tokens are unlocking according to the vesting schedule to avoid errors.