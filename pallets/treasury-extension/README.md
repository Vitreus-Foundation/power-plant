# Treasury Extension Pallet

A Substrate pallet that extends the functionality of the default Treasury pallet by implementing automated fund recycling mechanisms. This pallet ensures efficient treasury management by preventing excessive accumulation of unused funds while maintaining economic sustainability.

## Overview

The Treasury Extension pallet introduces a mechanism to recycle (re-allocate) unused treasury funds when spending falls below a configured threshold. Instead of burning unused funds, this pallet allows for their strategic reallocation according to the runtime's configuration.

### Features

- Automated fund recycling based on configurable spending thresholds
- Customizable allocation of recycled funds
- Integration with the existing Treasury pallet's spending mechanism
- Event emission for transparency and tracking
- Configurable weight parameters

## Installation

Add this pallet to your runtime's `Cargo.toml`:

```toml
[dependencies]
pallet-treasury-extension = { workspace = true }
```

## Configuration

### Runtime `Config` Trait Implementation

```rust
impl pallet_treasury_extension::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SpendThreshold = SpendThreshold;
    type OnRecycled = RecyclingDestination;
    type WeightInfo = pallet_treasury_extension::weights::SubstrateWeight<Runtime>;
}
```

### Types

- `RuntimeEvent`: The overarching event type for the runtime
- `SpendThreshold`: A `Permill` value determining the recycling threshold
- `OnRecycled`: Handler for recycled funds (implements `OnUnbalanced`)
- `WeightInfo`: Weight configuration for the pallet's dispatchables

## Usage

### Basic Setup

1. Implement the pallet's configuration trait for your runtime
2. Add the pallet to your `construct_runtime` macro:

```rust
construct_runtime!(
    pub enum Runtime where
        // ...
        Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>},
        TreasuryExtension: pallet_treasury_extension::{Pallet, Event<T>},
    }
);
```

### Configuring the Spending Threshold

Set up a parameter type for the spending threshold:

```rust
parameter_types! {
    pub const SpendThreshold: Permill = Permill::from_percent(50);
}
```

### Handling Recycled Funds

Implement the `OnUnbalanced` trait for your recycling destination:

```rust
pub struct RecyclingDestination;

impl OnUnbalanced<NegativeImbalanceOf<Runtime>> for RecyclingDestination {
    fn on_unbalanced(amount: NegativeImbalanceOf<Runtime>) {
        // Define how recycled funds should be handled
    }
}
```

## Events

The pallet emits the following event:

- `Recycled { recycled_funds: Balance }`: Emitted when funds are recycled, indicating the amount

## Security Considerations

1. **Threshold Configuration**: Choose the `SpendThreshold` carefully to maintain treasury sustainability
2. **Recycling Destination**: Ensure the `OnRecycled` implementation handles funds securely
3. **Integration Testing**: Thoroughly test integration with existing Treasury mechanisms

## Benchmarking

The pallet includes benchmarking for its functions. Run benchmarks using:

```bash
cargo run --release -- benchmark pallet --chain dev --pallet pallet-treasury-extension --extrinsic '*' --steps 50 --repeat 20
```

## License

This pallet follows the same licensing terms as your project's main license.

## Contributing

Contributions are welcome! Please ensure you:

1. Write tests for new functionality
2. Update documentation as needed
3. Follow the existing code style
4. Include benchmarks for new dispatchables

## Example

```rust
// Configure the treasury extension
impl pallet_treasury_extension::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SpendThreshold = SpendThreshold;
    type OnRecycled = Treasury; // Recycle back to treasury
    type WeightInfo = ();
}

// Monitor recycling events
frame_system::Pallet::<Runtime>::events()
    .iter()
    .filter_map(|record| {
        if let RuntimeEvent::TreasuryExtension(Event::Recycled { recycled_funds }) = &record.event {
            Some(recycled_funds)
        } else {
            None
        }
    });
```

## Related Pallets

- `pallet-treasury`: The main Treasury pallet this extends
- `pallet-bounties`: Can be used in conjunction for treasury fund allocation