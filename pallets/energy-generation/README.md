# Energy Generation Pallet

[![Substrate version](https://img.shields.io/badge/Substrate-4.0.0-brightgreen?logo=Parity%20Substrate)](https://substrate.io)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

A Substrate pallet for managing energy generation, staking, and reputation-based validation in a decentralized network.

## Overview

The Energy Generation pallet extends traditional staking mechanisms to incorporate energy generation metrics and reputation-based validation. It manages funds at stake by network maintainers while tying them to energy production capabilities and reputation scores.

### Key Features

- Energy-per-stake currency rate calculation
- Reputation-tied validator selection
- Collaborative staking mechanisms
- NAC (Network Access Control) level integration
- VIP membership handling
- Battery slot capacity management

## Architecture

The pallet is organized into several key components:

```
energy-generation/
├── reward-curve/      # Reward calculation implementations
├── reward-fn/         # Reward distribution functions
├── rpc/               # RPC interface definitions
├── runtime-api/       # Runtime API implementations
└── src/              
    ├── pallet/        # Core pallet implementation
    ├── benchmarking/  # Performance benchmarks
    ├── migrations/    # Storage migrations
    └── slashing/      # Slashing mechanisms
```

## Core Concepts

### Staking Roles

1. **Validators**
  - Run nodes to maintain network
  - Generate energy revenue
  - Earn rewards based on performance and reputation
  - Must maintain minimum reputation tier

2. **Cooperators**
  - Support validators through stake delegation
  - Share in rewards and penalties
  - Must meet minimum bond requirements

### Energy Generation

The pallet tracks energy generation through a unique energy-per-stake currency rate:

```rust
pub trait EnergyRateCalculator<Stake, Energy> {
    fn calculate_energy_rate(
        total_staked: Stake,
        total_issuance: Energy,
        core_nodes_num: u32,
        battery_slot_cap: Energy,
    ) -> Energy;
}
```

### Reputation Integration

Validators and cooperators are evaluated based on:
- Block authoring performance
- Network participation
- Energy generation consistency
- Slashing history

## Configuration

### Key Types

```rust
pub trait Config: frame_system::Config + pallet_assets::Config + pallet_reputation::Config {
    type StakeCurrency: LockableCurrency<Self::AccountId>;
    type StakeBalance: AtLeast32BitUnsigned;
    type EnergyAssetId: Get<Self::AssetId>;
    type BatterySlotCapacity: Get<EnergyOf<Self>>;
    // ... additional configuration types
}
```

### Constants

- `SessionsPerEra`: Number of sessions per era
- `BondingDuration`: Lock period for staked funds
- `MaxCooperations`: Maximum cooperations per cooperator
- `HistoryDepth`: Number of eras to keep in history

## Usage

### Becoming a Validator

```rust
// Bond your stake
pallet_energy_generation::Pallet::<T>::bond(
    origin,
    controller,
    value,
    payee
)?;

// Set validator preferences
pallet_energy_generation::Pallet::<T>::validate(
    origin,
    ValidatorPrefs {
        commission: Perbill::from_percent(5),
        collaborative: true,
        ..Default::default()
    }
)?;
```

### Cooperating

```rust
// Bond stake first
pallet_energy_generation::Pallet::<T>::bond(
    origin,
    controller,
    value,
    payee
)?;

// Choose validators to support
pallet_energy_generation::Pallet::<T>::cooperate(
    origin,
    vec![(validator_stash, stake_amount)]
)?;
```

## Rewards

Rewards are calculated based on:
1. Energy generation rate
2. Reputation tier multipliers
3. Validator commission
4. Cooperation distribution

The reward formula incorporates:
```rust
let reward = base_reward
    .saturating_mul(energy_rate)
    .saturating_mul(reputation_multiplier);
```

## Slashing

Slashing occurs for:
- Offline validators
- Equivocation
- Energy generation manipulation
- Reputation decay

Slashed amounts affect both validators and cooperators proportionally.

## RPC Interface

The pallet exposes RPC methods for querying:
- Current energy per stake currency rate
- Reputation tier additional rewards
- Validator and cooperator statuses

Example:
```rust
#[rpc(server, client)]
pub trait EnergyGenerationApi<BlockHash> {
    #[method(name = "energyGeneration_currentEnergyPerStakeCurrency")]
    fn current_energy_per_stake_currency(&self, at: Option<BlockHash>) -> RpcResult<u128>;
}
```

## Testing

Run the test suite:
```bash
cargo test --package pallet-energy-generation
```

Run benchmarks:
```bash
cargo test --package pallet-energy-generation --features runtime-benchmarks
```

## Security Considerations

1. **Reputation Verification**
  - All reputation changes are verified through multiple validators
  - Reputation cannot be manipulated through stake size alone

2. **Energy Rate Protection**
  - Rates are calculated using verifiable on-chain data
  - Protected against manipulation through minimum/maximum bounds

3. **Slashing Safety**
  - Gradual slashing with warning mechanisms
  - Protected against mass-exit attacks

## Development

### Prerequisites
- Rust 1.70+
- Substrate 4.0.0+
- `wasm32-unknown-unknown` target

### Building
```bash
cargo build --release
```

### Contributing
1. Fork the repository
2. Create your feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## License

Licensed under Apache 2.0 - see [LICENSE](LICENSE)