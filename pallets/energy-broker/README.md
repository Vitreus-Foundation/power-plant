# Energy Broker Pallet

## Overview

The Energy Broker pallet is a critical component of the Vitreus ecosystem that enables decentralized energy trading through an automated market maker (AMM) system. Built on Substrate's proven asset conversion mechanics, it facilitates trustless exchange between energy assets and native/non-native tokens while ensuring market efficiency and liquidity.

## Features

- **Decentralized Energy Trading**: Enables peer-to-peer energy trading without intermediaries
- **Automated Market Making**: Uses a constant sum formula optimized for energy asset pricing
- **Liquidity Provision**: Allows users to provide liquidity and earn fees
- **Multi-Asset Support**: Handles both native currency and custom energy assets
- **Path-Based Routing**: Supports complex trades through multiple pools
- **Security-First Design**: Built with robust error handling and economic security measures

## Key Components

### Pool Management
- Create energy trading pools with `create_pool`
- Add liquidity through `add_liquidity`
- Remove liquidity via `remove_liquidity`
- Auto-generated pool accounts for custody of assets

### Trading Functions
- `swap_exact_tokens_for_tokens`: Trade exact input amount for variable output
- `swap_tokens_for_exact_tokens`: Trade variable input for exact output amount
- Quote functions for price discovery
- Multi-hop trading support for complex routes

### Security Features
- Withdrawal fee mechanism to prevent manipulation
- Minimum liquidity requirements
- Economic security through pool setup fees
- Slippage protection parameters
- Keep-alive checks for account safety

## Configuration

Key configuration parameters include:

```rust
pub trait Config: frame_system::Config {
    type RuntimeEvent: From<Event<Self>>;
    type Formula: Formula<Self>;            // Pricing formula
    type Currency: InspectFungible<...>;    // Native currency handling
    type Assets: Inspect<...>;              // Energy asset registry
    type PoolAssets: Inspect<...>;          // Liquidity pool tokens
    type LPFee: Get<u32>;                  // Liquidity provider fee
    type PoolSetupFee: Get<Balance>;       // One-time pool creation fee
    type MaxSwapPathLength: Get<u32>;      // Maximum routing path length
}
```

## Usage

### Creating an Energy Trading Pool
```rust
// Create a pool between native token and energy asset
AssetConversion::create_pool(
    Origin::root(),
    alice,
    NativeOrAssetId::Native,
    NativeOrAssetId::Asset(energy_asset_id)
)?;
```

### Adding Liquidity
```rust
// Provide liquidity to enable trading
AssetConversion::add_liquidity(
    Origin::signed(alice),
    NativeOrAssetId::Native,
    energy_asset_id,
    amount_native,
    amount_energy,
    min_native,
    min_energy,
    alice
)?;
```

### Trading Energy
```rust
// Swap exact amount of native token for energy
AssetConversion::swap_exact_tokens_for_tokens(
    Origin::signed(alice),
    vec![native_id, energy_id].try_into()?,
    input_amount,
    minimum_energy_out,
    alice,
    true
)?;
```

## Security Considerations

1. **Liquidity Protection**
    - Minimum liquidity requirements prevent pool drainage
    - Withdrawal fees discourage manipulation
    - Slippage parameters protect traders

2. **Economic Security**
    - Pool setup fees prevent spam
    - LP fees ensure sustainable operation
    - Native currency requirements for stability

3. **Technical Safety**
    - Comprehensive error handling
    - Overflow protection
    - Account existence checks

## Implementation Notes

- Built on proven Substrate patterns
- Optimized for energy trading requirements
- Extensive testing coverage
- Benchmarked for performance
- Runtime API for price discovery

## Testing

The pallet includes extensive tests covering:
- Pool creation and management
- Liquidity provision mechanics
- Trading scenarios
- Error conditions
- Economic security
- Multi-hop routing

## Future Developments

Planned enhancements:
- Oracle integration for external price feeds
- Enhanced analytics and reporting
- Additional trading pair support
- Optimized gas efficiency
- Advanced routing algorithms

## License

Licensed under the Apache License, Version 2.0

## ------------------------------

# Future Developments: Dynamic Energy Broker System

## Dynamic Rate Mechanism

The Energy Broker pallet is evolving from a static to a dynamic conversion system, introducing several key improvements:

### Automated Market Making
- **Dynamic Exchange Rate**: Replaces fixed 1:0.9 VTRS/gVolt ratio with market-responsive rates
- **Demand-Based Pricing**: Exchange rates adjust based on:
    - Network usage metrics
    - 84-era rolling averages
    - Staking pool dynamics
    - Warehouse capacity utilization

### Warehouse Integration
The future system will introduce a Warehouse mechanism for enhanced stability:
```rust
pub trait WarehouseConfig {
    /// Capacity thresholds that trigger rate adjustments
    type CapacityThresholds: Get<Vec<(Permill, Permill)>>;
    /// Rate multipliers for different capacity levels
    type RateMultipliers: Get<Vec<FixedU128>>;
}
```

### Rate Calculation Formula
The dynamic system will employ sophisticated rate calculations:
```rust
pub type ExchangeRate = (
    // Current rate based on:
    total_staked: Balance,      // Total staked VTRS
    warehouse_capacity: Balance, // Current warehouse capacity
    network_usage: Balance,     // Recent network activity
    era_average: Balance,       // 84-era rolling average
) -> Rate;
```

## Advanced Features

### Adaptive Generation Rate
- Dynamically adjusts gVolt generation based on network demand
- Uses rolling averages to smooth volatility
- Prevents supply/demand mismatches

### Capacity-Based Multipliers
Progressive rate adjustments based on Warehouse capacity:
- Under-capacity (0-45%): +1% to +20% return rate
- Neutral zone (46-55%): Baseline rates
- Over-capacity (56-100%): -1% to -20% return rate

### Economic Security
- **Anti-Gaming Measures**:
    - Rate calculations based on current period metrics
    - Rolling averages to prevent manipulation
    - Smart contract-governed Warehouse operations

## Future Integration Points

### Smart Contract Integration
```rust
pub trait WarehouseControl {
    /// Mint new gVolts under controlled conditions
    fn mint_gvolts(amount: Balance) -> DispatchResult;
    /// Burn excess gVolts when needed
    fn burn_gvolts(amount: Balance) -> DispatchResult;
    /// Monitor and adjust capacity
    fn adjust_capacity(target: Permill) -> DispatchResult;
}
```

### Network Metrics
- Real-time network usage tracking
- Demand forecasting
- Capacity optimization
- Rate stabilization algorithms

## Sustainability Features

### Long-term Stability
- Self-adjusting supply mechanics
- Demand-responsive pricing
- Buffer against market volatility
- Sustainable reward structures

### Scaling Support
- Dynamic capacity adjustment
- Network growth accommodation
- Transaction cost stability
- Performance optimization

## Technical Implementation Notes

### Critical Formulas

The core rate calculation will use:
```rust
pub fn calculate_vtrs_earned(
    staked_vtrs: Balance,
    gvolts_sold: Balance,
    era_average: Balance,
    target_rate: Balance,
) -> Balance {
    // H = A * (Bn / Cn) * (D * (En / F) / Gn-1) * Multiplier
    // Where:
    // A = Total Staked VTRS * APR / 8760 (hourly rate)
    // Bn = Current gVolts sold to Warehouse
    // Cn = 84-era rolling average of gVolts sold
    // D = Target VTRS/second rate
    // En = Current total staked VTRS
    // F = Initial staked VTRS
    // Gn-1 = Previous 84-era rolling average VTRS/second rate
}
```

### Migration Considerations

- Phased rollout approach
- Backwards compatibility period
- Graceful static-to-dynamic transition
- Data migration support

## Future Roadmap Integration

The dynamic Energy Broker system represents a critical component in Vitreus' evolution:
- Q4 2024: Initial dynamic rate implementation
- 2025: Full Warehouse integration
- 2025-2026: Advanced network metrics and optimization
- Post-2026: Enhanced scaling and sustainability features

This evolution ensures the Energy Broker remains central to Vitreus' mission of providing efficient, sustainable, and scalable energy trading infrastructure.