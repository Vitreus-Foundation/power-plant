# Substrate Faucet Pallet 🚰

A secure and rate-limited faucet implementation for Substrate-based test networks. This pallet enables controlled distribution of test tokens while preventing abuse through time-based restrictions and amount limits.

## Overview

The Faucet pallet provides a permissionless mechanism for users to request test tokens on networks where real value is not at stake. It implements several security measures to prevent abuse while maintaining simplicity and usability.

### Features

- ⏱️ Time-based rate limiting (24-hour cooldown)
- 💎 Configurable maximum request amount
- 🔒 Account-based tracking
- ⚡ Efficient storage using Blake2 hashing
- 📊 Event emission for tracking and indexing
- ⚖️ Benchmarked weights

## Installation

Add this pallet to your runtime's `Cargo.toml`:

```toml
[dependencies]
pallet-faucet = { version = "0.1.0", default-features = false }
```

### Runtime Configuration

Include the following in your runtime's `construct_runtime!` macro:

```rust
construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        // --snip--
        Faucet: pallet_faucet,
    }
);
```

### Config Trait Implementation

Implement the pallet's configuration trait for your runtime:

```rust
parameter_types! {
    pub const MaxAmount: Balance = 100 * UNITS; // Adjust based on your token decimals
    pub const AccumulationPeriod: BlockNumber = 7200; // 24 hours in blocks (assuming 12s block time)
}

impl pallet_faucet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxAmount = MaxAmount;
    type AccumulationPeriod = AccumulationPeriod;
    type WeightInfo = pallet_faucet::weights::SubstrateWeight<Runtime>;
}
```

## Usage

Users can request funds through a signed extrinsic:

```rust
// Request 50 tokens
Faucet::request_funds(Origin::signed(account_id), 50 * UNITS);
```

### Security Considerations

1. **Rate Limiting**: Users cannot request more than `MaxAmount` tokens within a 24-hour period
2. **Amount Validation**: Individual requests cannot exceed `MaxAmount`
3. **Signed Transactions**: All requests must be signed, preventing unauthorized access
4. **Storage Efficiency**: Uses Blake2 for storage key hashing
5. **No Admin Controls**: Operates autonomously without privileged actions

### Important Notes

- This pallet is intended for **TEST NETWORKS ONLY**
- Do not deploy on production networks or networks with real value
- Configure `MaxAmount` based on your network's token economics
- Adjust `AccumulationPeriod` based on your block time

## Events

The pallet emits the following events:

- `FundsSent { who: T::AccountId, amount: T::Balance }`: Triggered when funds are successfully sent

## Errors

Possible error conditions:

- `AmountTooHigh`: Request exceeds configured `MaxAmount`
- `RequestLimitExceeded`: User has exceeded their 24-hour allocation

## Testing

Run the test suite:

```bash
cargo test
```

### Test Coverage

- ✅ Basic fund requesting
- ✅ Maximum amount validation
- ✅ Time-based restrictions
- ✅ Accumulation period reset
- ✅ Multiple request handling

## Benchmarking

The pallet includes benchmarking for weights calculation:

```bash
cargo build --release --features runtime-benchmarks
./target/release/node-template benchmark pallet \
    --chain dev \
    --execution=wasm \
    --wasm-execution=compiled \
    --pallet pallet_faucet \
    --extrinsic "*" \
    --steps 50 \
    --repeat 20
```

## License

The pallet is part of your Substrate node template. Refer to your project's license terms.

## Contributing

Contributions are welcome! Please ensure:
1. Tests pass
2. New features include tests
3. Documentation is updated
4. Benchmarks are included for new extrinsics

## Security Considerations

While this pallet is designed for test networks, it follows security best practices:

1. Rate limiting prevents drain attacks
2. Storage uses efficient hashing
3. All actions require signed transactions
4. No privileged functions
5. Simple, auditable codebase

## Support

For issues, questions, or contributions, please open an issue in the repository.