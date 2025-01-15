# DynamicEnergy Pallet Documentation (New)

**Version:** 0.1.0  
**Last Updated:** January 2025  

---

## Key Features

- **Dynamic Energy Management**: Adjusts energy rates and exchange parameters based on network conditions.  
- **Smooth Value Calculations**: Implements smoothing mechanisms for generation and exchange rates.  
- **APR and Multiplier Adjustments**: Allows updates to annual percentage rates and multiplier coefficients for energy capacity.  

---

## Common Use Cases

- Dynamically managing energy rates in response to network traffic.  
- Adjusting exchange rates between assets and energy for optimized market conditions.  
- Setting APR values for energy operations across network sessions.  

---

## For Non-Developers 🌟

### What is the DynamicEnergy Pallet?

The **DynamicEnergy** pallet provides mechanisms for managing energy dynamics, including burning, selling, and generating energy. By adjusting parameters such as APR and exchange rates, this pallet ensures energy operations remain aligned with network conditions. It also tracks energy generation and updates exchange rates dynamically.

### Key Concepts

- **Energy Burn Override**: Allows overriding the default energy burned value for recalibration.  
- **Annual Percentage Rate (APR)**: Sets the default rate of energy returns, configurable by administrators.  
- **Smooth Factors**: Control how quickly generation and exchange rates respond to changes.  

---

## Available Operations

### **Force Set Energy Burn**
- **What it does**: Overrides the energy burn value used in network calculations.  
- **When to use it**: Use this to align the burn rate with updated energy dynamics.  
- **Example**: Increasing the burn rate during high-traffic periods for better energy allocation.  
- **Important to know**: Only administrators can execute this operation.  

---

### **Update Exchange Rate**
- **What it does**: Calculates and updates the exchange rate between assets and energy.  
- **When to use it**: Use this to ensure that the exchange rate remains fair and reflects network conditions.  
- **Example**: Adjusting the exchange rate to account for increased energy sales.  
- **Important to know**: Exchange rates directly impact market trades.  

---

## For Developers 💻

### Technical Overview

The **DynamicEnergy** pallet manages energy dynamics through smoothing algorithms, configurable APR, and storage mechanisms. It allows the network to adapt to changing conditions and ensures efficient resource allocation. Developers can use its extrinsics to customize and optimize energy behavior.

### Integration Points

- **Staking Module**: Interfaces with staking to calculate total stakes and align energy dynamics.  
- **Warehouse Module**: Retrieves data for energy capacities and generation rates.  
- **Runtime Event System**: Emits events for updates to energy rates and parameters.  

---

### Extrinsics

#### `force_set_energy_burn(origin, amount)`
- **Purpose**: Overrides the current energy burn value.  
- **Parameters**:  
  - `origin`: The administrative account initiating the override.  
  - `amount`: The new energy burn value.  
- **Returns**: `DispatchResult` indicating success or failure.  
- **Example Usage**:
  ```rust
  let result = DynamicEnergy::force_set_energy_burn(origin, Some(500))?;
  ```

#### `set_annual_percentage_rate(origin, rate)`
- **Purpose**: Updates the APR for energy operations.  
- **Parameters**:  
  - `origin`: The administrative account initiating the update.  
  - `rate`: The new APR value.  
- **Returns**: `DispatchResult` indicating success or failure.  
- **Example Usage**:
  ```rust
  let result = DynamicEnergy::set_annual_percentage_rate(origin, 50)?;
  ```

#### `set_multiplier_coefficients(origin, a, b, c, d)`
- **Purpose**: Updates the coefficients for the capacity multiplier formula.  
- **Parameters**:  
  - `origin`: The administrative account initiating the update.  
  - `a, b, c, d`: The new coefficients for the multiplier formula.  
- **Returns**: `DispatchResult` indicating success or failure.  
- **Example Usage**:
  ```rust
  let result = DynamicEnergy::set_multiplier_coefficients(
      origin, 
      1.into(), 
      2.into(), 
      3.into(), 
      4.into()
  )?;
  ```
