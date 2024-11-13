# Reputation Pallet

A Substrate-based pallet that implements a dynamic reputation system for blockchain networks. This pallet provides a mechanism to evaluate and track user behavior through a tiered reputation scoring system.

## Overview

The Reputation pallet implements a sophisticated scoring system that:
- Awards reputation points to users over time (basic rewards per block)
- Implements a three-tier reputation system: Vanguard, Trailblazer, and Ultramodern
- Supports reputation slashing and manual adjustments
- Provides automatic rank calculation and tier progression
- Implements hooks for account creation and removal

## Key Features

- **Time-based Reputation**: Users earn reputation points automatically over time
- **Tiered System**:
    - Vanguard (Tier 1)
    - Trailblazer (Tier 2)
    - Ultramodern (Tier 3)
- **Flexible Point Management**:
    - Automatic point accrual
    - Manual point adjustment
    - Slashing mechanism
    - Force-set capabilities for governance
- **Account Lifecycle Integration**:
    - Automatic reputation initialization for new accounts
    - Clean-up on account removal

## Constants

```rust
REPUTATION_POINTS_PER_BLOCK: u64 = 24
REPUTATION_POINTS_PER_DAY: u64 = 24 * 10 * 60 * 24  // Based on 6000ms block time
REPUTATION_POINTS_PER_MONTH: u64 = REPUTATION_POINTS_PER_DAY * 30
REPUTATION_POINTS_PER_YEAR: u64 = REPUTATION_POINTS_PER_MONTH * 12
RANKS_PER_TIER: u8 = 3
RANKS_PER_U3: u8 = 9
```

## Installation

Add this to your runtime's `Cargo.toml`:

```toml
[dependencies]
pallet-reputation = { version = "0.1.0", default-features = false }

[features]
default = ["std"]
std = [
    "pallet-reputation/std",
    # ... other std features
]
```

## Runtime Configuration

```rust
impl pallet_reputation::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = pallet_reputation::weights::WeightInfo;
}
```

## Storage

The pallet uses the following storage items:

- `AccountReputation`: Maps account IDs to reputation records
    - Key: `T::AccountId`
    - Value: `ReputationRecord`

## Extrinsics

### Permissionless Calls
- `update_points(account: T::AccountId)`: Update reputation points for an account

### Root/Governance Calls
- `force_set_points(account: T::AccountId, points: ReputationPoint)`: Force set reputation points
- `increase_points(account: T::AccountId, points: ReputationPoint)`: Increase reputation points
- `slash(account: T::AccountId, points: ReputationPoint)`: Slash reputation points
- `force_reset_points()`: Reset all accounts to Vanguard tier 1

## Events

```rust
ReputationSetForcibly { account: T::AccountId, points: ReputationPoint }
ReputationIncreased { account: T::AccountId, points: ReputationPoint }
ReputationSlashed { account: T::AccountId, points: ReputationPoint }
ReputationUpdated { account: T::AccountId, points: ReputationPoint }
ReputationIncreaseFailed { account: T::AccountId, error: DispatchError, points: ReputationPoint }
ReputationResetForcibly { points: ReputationPoint }
```

## Usage Examples

### Basic Runtime Integration

```rust
use pallet_reputation;

parameter_types! {
    pub const ExampleParameter: u32 = 100;
}

impl pallet_reputation::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system,
        Reputation: pallet_reputation,
    }
);
```

### Updating Account Reputation

```rust
use pallet_reputation::{Call as ReputationCall};

// Update reputation for an account
let account = 0x1234...;
let call = ReputationCall::update_points { account: account.clone() };
let origin = frame_system::RawOrigin::Signed(account).into();
call.dispatch(origin)?;
```

## Genesis Configuration

You can configure initial reputation states in your chain spec:

```rust
GenesisConfig {
reputation: ReputationConfig {
accounts: vec![
    (account_id, ReputationRecord::new(points, block_number)),
],
},
}
```

## Security Considerations

1. Root-only operations:
    - Force setting points
    - Increasing points
    - Slashing points
    - Force resetting all points

2. Time-based calculations:
    - Points are calculated based on block numbers
    - Ensure proper block time configuration

3. Account management:
    - Implements `OnNewAccount` and `OnKilledAccount`
    - Proper cleanup of reputation data

## Testing

Run the test suite:

```bash
cargo test
```

For benchmark tests:

```bash
cargo test --features runtime-benchmarks
```

## License

This pallet is part of the project codebase and follows the project's licensing terms.