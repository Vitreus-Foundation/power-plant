# Privileges Pallet

A Substrate-based pallet that implements a decentralized VIP membership system with dynamic privileges, rewards, and governance mechanisms.

## Overview

The Privileges pallet enables blockchain networks to implement a merit-based VIP system where validators and cooperators can earn special privileges through active participation and stake commitment. The system includes innovative features like dynamic penalty calculations, points accumulation, and dual-tier membership (VIP and VIPP) while maintaining decentralization principles.

## Features

### VIP Membership
- **Eligibility**: Open to network validators and cooperators
- **Membership Types**:
    - Regular VIP: Base tier membership
    - VIPP (VIP Plus): Enhanced tier with additional benefits
- **Points System**: Dynamic point accumulation based on stake and participation

### Penalty System
Two types of exit penalties to ensure system stability:
- **Flat**: Fixed 17.5% penalty rate
- **Declining**: Variable rate that decreases by 5% each quarter (30% → 25% → 20% → 15%)

### Time-Based Mechanics
- Penalty-free period during the first month of each quarter
- Automatic points calculation based on stake and time
- Year-end rewards distribution based on accumulated points

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
pallet-privileges = { git = "https://github.com/your-org/substrate-node", branch = "main" }
```

## Configuration

Implement the pallet's configuration trait for your runtime:

```rust
impl pallet_privileges::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type UnixTime = Timestamp;
    type WeightInfo = weights::SubstrateWeight<Runtime>;
}
```

## Usage

### Becoming a VIP Member

```rust
// Choose penalty type and become VIP
privileges::Pallet::<T>::become_vip_status(
    origin,
    PenaltyType::Declining,
)?;
```

### Exiting VIP Status

```rust
// Exit VIP status (consider penalty periods)
privileges::Pallet::<T>::exit_vip(origin)?;
```

### Changing Penalty Type

```rust
// Can only be changed during penalty-free periods
privileges::Pallet::<T>::change_penalty_type(
    origin,
    PenaltyType::Flat,
)?;
```

## Key Constants

```rust
const INCREASE_VIP_POINTS_CONSTANT: u64 = 50;
const MAX_UPDATE_DAYS: u32 = 366;
const FREE_PENALTY_PERIOD_MONTH_NUMBER: u32 = 1;
```

## Storage Items

- `VipMembers`: Maps account IDs to VIP member information
- `VippMembers`: Maps account IDs to VIPP member information
- `YearVipResults`: Stores annual VIP points results
- `YearVippResults`: Stores annual VIPP points results
- `CurrentDate`: Tracks current date information for calculations

## Events

- `NewVipMember`: Emitted when a new VIP member joins
- `PenaltyTypeChanged`: Emitted when a member changes their penalty type
- `LeftVip`: Emitted when a member exits VIP status

## Error Types

- `AccountNotLegitForVip`: Account doesn't meet VIP requirements
- `AlreadyVipMember`: Account is already a VIP member
- `AccountHasNotVipStatus`: Account isn't a VIP member
- `IsNotPenaltyFreePeriod`: Action attempted outside penalty-free period
- `NotCorrectDate`: Invalid date parameters
- `HasNotClaim`: Account lacks required claim balance

## Points Calculation

VIP points are calculated using the formula:
```rust
points = stake * multiplier
where multiplier = 1 / (INCREASE_VIP_POINTS_CONSTANT + elapsed_days)
```

## Security Considerations

1. **Stake Protection**: Implements slashing mechanisms to prevent abuse
2. **Time Constraints**: Enforces strategic timing for penalty-free actions
3. **Validation Checks**: Ensures only qualified accounts can become VIP members
4. **Balance Verification**: Validates sufficient stake before membership

## Integration Example

```rust
use pallet_privileges::{self, Config, Error, Event, PenaltyType};

// Implement for your runtime
impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type UnixTime = Timestamp;
    type WeightInfo = ();
}

// Add to construct_runtime
construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        // Other pallets...
        Privileges: pallet_privileges,
    }
);
```

## Testing

The pallet includes comprehensive tests covering:
- VIP membership lifecycle
- Points calculation accuracy
- Penalty system mechanics
- VIPP status handling
- Time-based operations

Run tests using:
```bash
cargo test --package pallet-privileges
```

## Contributing

We welcome contributions! Please follow these steps:

1. Fork the repository
2. Create your feature branch
3. Commit your changes
4. Push to the branch
5. Submit a pull request

## License

This project is licensed under the Apache License 2.0

## Contact

For questions and support:
- Open an issue in the repository
- Join our developer community
- Check our technical documentation