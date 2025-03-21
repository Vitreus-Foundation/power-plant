# Dynamic Energy Pallet – Developer Guide & Extrinsic Parameter Reference

This document provides a comprehensive explanation of the Dynamic Energy pallet, how its extrinsics work, and a detailed reference for the input parameters each extrinsic takes. It is intended for developers and administrators interacting with this Substrate runtime module.

---

## Table of Contents

- [Overview](#overview)
- [Storage Items & Key Concepts](#storage-items--key-concepts)
- [Extrinsics Overview](#extrinsics-overview)
  - [1. force_set_energy_burn](#1-force_set-energy-burn)
  - [2. force_set_energy_sale](#2-force_set-energy-sale)
  - [3. force_set_total_stake](#3-force-set-total-stake)
  - [4. set_generation_rate_smooth_factor](#4-set_generation-rate-smooth-factor)
  - [5. set_exchange_rate_smooth_factor](#5-set-exchange-rate-smooth-factor)
  - [6. set_annual_percentage_rate](#6-set-annual-percentage-rate)
  - [7. set_multiplier_coefficients](#7-set-multiplier-coefficients)
- [Underlying Calculations and Mechanics](#underlying-calculations-and-mechanics)
- [Practical Use Cases](#practical-use-cases)
- [Invoking Extrinsics](#invoking-extrinsics)
- [Conclusion](#conclusion)

---

## Overview

The Dynamic Energy pallet manages on-chain parameters for calculating energy generation and exchange rates dynamically. It does so by:

- Tracking **energy burn** and **energy sale** events during sessions.
- Calculating rates using smoothing functions to blend new measurements with historical data.
- Providing administrative overrides to simulate or correct system behavior.
- Adjusting economic parameters such as the annual percentage rate (APR) and warehouse capacity multiplier coefficients.

---

## Storage Items & Key Concepts

- **Energy & Stake Overrides:**  
  - **EnergyBurnOverride:** Manually override the automatically accumulated energy burn.
  - **EnergySaleOverride:** Manually override the automatically accumulated energy sale.
  - **TotalStakeOverride:** Manually override the total stake used for rate calculations.

- **Rate Parameters & Smoothing Factors:**  
  - **GenerationRateSmoothFactor:** Controls smoothing for energy generation rates.
  - **ExchangeRateSmoothFactor:** Controls smoothing for energy exchange rates.
  - **AnnualPercentageRate (APR):** Expressed in 10ths of a percent (e.g., 100 equals 10%).
  - **MultiplierCoefficients:** An array of four coefficients ([a, b, c, d]) for calculating the warehouse capacity multiplier using a polynomial (`a*x^3 + b*x^2 + c*x + d`).

- **Session Metrics:**  
  - **SessionEnergyBurn & SessionEnergySale:** Accumulate energy burn and sale amounts during a session.
  - **EnergyGeneration:** Stores the computed energy generation value per session.

---

## Extrinsics Overview

All extrinsics require a privileged origin (specified by `T::ManageOrigin`). Below is a description of each extrinsic and the associated input parameters.

### 1. force_set_energy_burn

- **Purpose:**  
  Override the energy burn value for the current session.

- **When to Use:**  
  - **Testing:** Simulate specific energy burn conditions.
  - **Correction:** Adjust a misreported or anomalous energy burn measurement.

- **Input Parameters:**  
  - `amount: Option<EnergyOf<T>>`  
    - **Type:** Option of the pallet’s balance type (alias for `T::Balance`).
    - **Meaning:**  
      - A numeric value representing the energy burned (measured in the unit defined by your runtime).
      - Passing `None` reverts the override so that the pallet uses the accumulated session value.

- **Event Emitted:**  
  - `EnergyBurnForceSet`

---

### 2. force_set_energy_sale

- **Purpose:**  
  Manually override the energy sale value for the current session.

- **When to Use:**  
  - **Testing/Debugging:** Simulate different energy sale scenarios.
  - **Correction:** Adjust for inaccuracies in automatically recorded sales data.

- **Input Parameters:**  
  - `amount: Option<EnergyOf<T>>`  
    - **Type:** Option of the pallet’s balance type.
    - **Meaning:**  
      - A specific value for energy sale amount.
      - Passing `None` disables the override, allowing the pallet to revert to its default behavior.

- **Event Emitted:**  
  - `EnergySaleForceSet`

---

### 3. force_set_total_stake

- **Purpose:**  
  Override the total stake value used in exchange rate calculations.

- **When to Use:**  
  - **Testing:** See how the exchange rate responds to different total stake values.
  - **Correction:** Fix situations where the automatically derived total stake does not meet expected values.

- **Input Parameters:**  
  - `amount: Option<StakeOf<T>>`  
    - **Type:** Option of the stake type (alias for `T::Balance`).
    - **Meaning:**  
      - A numeric value representing the total stake.
      - Passing `None` causes the pallet to fall back to the automatically calculated stake from the staking module.

- **Event Emitted:**  
  - `TotalStakeForceSet`

---

### 4. set_generation_rate_smooth_factor

- **Purpose:**  
  Set the smoothing factor for the energy generation rate.

- **Why It Matters:**  
  Smoothing mitigates sudden changes by blending new data with historical values.
  
- **Input Parameters:**  
  - `value: u32`  
    - **Type:** Unsigned 32-bit integer.
    - **Meaning:**  
      - A value greater than 0.
      - **1:** Disables smoothing (only the new value is used).
      - **>1:** Increases the weight of historical data, reducing volatility in the computed generation rate.

- **Event Emitted:**  
  - `GenerationRateSmoothFactorUpdated`

---

### 5. set_exchange_rate_smooth_factor

- **Purpose:**  
  Set the smoothing factor for the exchange rate calculation.

- **Why It Matters:**  
  This parameter controls how new exchange rate values are blended with previous ones to minimize abrupt changes.

- **Input Parameters:**  
  - `value: u32`  
    - **Type:** Unsigned 32-bit integer.
    - **Meaning:**  
      - Must be greater than 0.
      - Determines the weight given to new versus old exchange rate values; a value of 1 means no smoothing, while higher values increase the influence of past data.

- **Event Emitted:**  
  - `ExchangeRateSmoothFactorUpdated`

---

### 6. set_annual_percentage_rate

- **Purpose:**  
  Update the annual percentage rate (APR) used in exchange rate computations.

- **Why It Matters:**  
  The APR influences the economic incentives and conversion rates between energy metrics and token values.

- **Input Parameters:**  
  - `value: u32`  
    - **Type:** Unsigned 32-bit integer.
    - **Meaning:**  
      - Represents the APR in 10ths of a percent.
      - For example, a value of **100** corresponds to an APR of **10%**.

- **Event Emitted:**  
  - `AnnualPercentageRateUpdated`

---

### 7. set_multiplier_coefficients

- **Purpose:**  
  Update the coefficients for the warehouse capacity multiplier calculation.

- **Why It Matters:**  
  The warehouse capacity multiplier adjusts the exchange rate based on the current utilization of the warehouse's capacity. The polynomial used is:
  multiplier = `ax^3 + bx^2 + c*x + d`

where **x** is the percentage of the warehouse’s capacity currently in use.

- **Input Parameters:**  
- `a: FixedI128`  
- `b: FixedI128`  
- `c: FixedI128`  
- `d: FixedI128`  
  - **Type for Each:** A fixed-point 128-bit integer.
  - **Meaning:**  
    - These coefficients determine the shape of the polynomial.
    - Adjust them to modify how aggressively the multiplier reacts to changes in warehouse capacity (e.g., encouraging higher capacity utilization by altering economic incentives).

- **Event Emitted:**  
- `MultiplierCoefficientsUpdated`

---

## Underlying Calculations and Mechanics

### Energy Generation Calculation

- **Basis:**  
The generation rate is calculated based on the energy burn value.

- **Smoothing:**  
The new generation rate is blended with the previous session’s rate using a weight computed as:
`weight = 2 / (smooth_factor + 1) smoothed_rate = weight * new_rate + (1 - weight) * old_rate`


### Exchange Rate Calculation

- **Formula:**  
The exchange rate is derived from several factors:
- **Numerator:**  
  ```
  energy_sale * constant_factor * SECONDS_IN_YEAR * fixed_divisor
  ```
- **Denominator:**  
  ```
  total_stake * apr * duration * multiplier
  ```
The result is then smoothed similarly to the generation rate.

### Warehouse Capacity Multiplier

- **Calculation:**  
- Compute **x** as:
  ```
  x = (current_amount / max_capacity) * 100
  ```
- Evaluate the polynomial:
  ```
  multiplier = a*x^3 + b*x^2 + c*x + d
  ```
- If the multiplier is non-positive, a warning is logged and it defaults to one.

---

## Practical Use Cases

- **Testing and Simulation:**  
Use the `force_set_` extrinsics to simulate various economic scenarios without relying solely on live data.

- **Adjusting System Responsiveness:**  
Modify the smooth factors to control how reactive the system is to sudden changes in energy metrics.

- **Economic Policy Tuning:**  
Change the APR and multiplier coefficients to reflect new economic models or to adjust network incentives.

- **Debugging:**  
In the event of unexpected behavior, manually overriding key values can help pinpoint issues or stabilize operations until a more permanent solution is applied.

---

## Invoking Extrinsics

These extrinsics can be called via:

- **Substrate UI (Polkadot.js Apps):**  
Navigate to the Dynamic Energy pallet, select an extrinsic, fill in the parameters, and submit the transaction using an account with appropriate privileges.

- **CLI Tools (e.g., Subxt):**  
Construct and submit extrinsics programmatically. For example:
```
// Example using Subxt:
let call = dynamic_energy::calls::ForceSetEnergyBurn { amount: Some(1000) };
api.tx().dynamic_energy().force_set_energy_burn(call).sign_and_submit(&admin_key)?;
```

### Exchange Rate Calculation

- **Formula:**  
The exchange rate is derived from several factors:
- **Numerator:**  
  ```
  energy_sale * constant_factor * SECONDS_IN_YEAR * fixed_divisor
  ```
- **Denominator:**  
  ```
  total_stake * apr * duration * multiplier
  ```
The result is then smoothed similarly to the generation rate.

### Warehouse Capacity Multiplier

- **Calculation:**  
- Compute **x** as:
  ```
  x = (current_amount / max_capacity) * 100
  ```
- Evaluate the polynomial:
  ```
  multiplier = a*x^3 + b*x^2 + c*x + d
  ```
- If the multiplier is non-positive, a warning is logged and it defaults to one.

---

## Practical Use Cases

- **Testing and Simulation:**  
Use the `force_set_` extrinsics to simulate various economic scenarios without relying solely on live data.

- **Adjusting System Responsiveness:**  
Modify the smooth factors to control how reactive the system is to sudden changes in energy metrics.

- **Economic Policy Tuning:**  
Change the APR and multiplier coefficients to reflect new economic models or to adjust network incentives.

- **Debugging:**  
In the event of unexpected behavior, manually overriding key values can help pinpoint issues or stabilize operations until a more permanent solution is applied.

---

### Invoking Extrinsics

These extrinsics can be called via:

- **Substrate UI (Polkadot.js Apps):**  
Navigate to the Dynamic Energy pallet, select an extrinsic, fill in the parameters, and submit the transaction using an account with appropriate privileges.

- **CLI Tools (e.g., Subxt):**  
Construct and submit extrinsics programmatically. For example:
```
// Example using Subxt:
let call = dynamic_energy::calls::ForceSetEnergyBurn { amount: Some(1000) };
api.tx().dynamic_energy().force_set_energy_burn(call).sign_and_submit(&admin_key)?;
```
- **Automated Scripts:**
Integrate these calls into runtime management or governance scripts as part of your network's operational processes.

---

## Conclusion
The Dynamic Energy pallet provides robust tools for managing and fine-tuning on-chain economic parameters. With extrinsics that allow manual overrides, smoothing adjustments, and parameter updates, administrators have precise control over energy generation and exchange rate calculations. This guide details both the functionality and the necessary input parameters to empower developers in testing, simulation, and live adjustments.

For further questions or contributions, please refer to the Substrate documentation and follow the project's contribution guidelines.
