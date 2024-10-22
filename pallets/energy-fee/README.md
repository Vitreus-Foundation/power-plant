# Energy Fee Pallet

A sophisticated dual-token transaction fee mechanism for Substrate-based blockchains that implements a dynamic fee system using both native (VTRS) and energy (VNRG) tokens.

## Overview

The Energy Fee pallet provides a flexible fee system that:
- Supports dual-token fee payments using both native tokens and energy tokens
- Implements dynamic fee adjustments based on block fullness
- Manages automatic token exchanges for fee payments
- Supports custom fee calculations for specific extrinsics
- Provides compatibility with EVM transactions
- Includes threshold-based fee burning mechanisms

## Features

### Dual-Token Fee System
- Primary fee token (VNRG) used for transaction fee payments
- Automatic conversion from native token (VTRS) when VNRG balance is insufficient
- Configurable exchange rates between VTRS and VNRG

### Dynamic Fee Adjustment
- Block fullness-based fee multiplier
- Configurable upper fee multiplier limit
- Automatic fee scaling based on network congestion
- Base fee management for predictable minimum costs

### Energy Burning Mechanism
- Tracks accumulated burned energy per block
- Configurable burning thresholds
- Automatic threshold validation for transaction inclusion

### Custom Fee Calculations
- Support for pallet-specific fee calculations
- Special handling for EVM transactions
- Extensible trait system for custom fee logic

### EVM Compatibility
- Native support for Ethereum transaction fee mechanisms
- Consistent fee handling across Substrate and EVM transactions
- Configurable gas price calculations

## Installation

To use this pallet, add the following to your runtime's `Cargo.toml`:

```toml
[dependencies]
pallet-energy-fee = { version = "0.1.0", default-features = false }
```

And the following to your runtime's `std` feature:

```toml
std = [
    # ...
    "pallet-energy-fee/std",
]
```

## Configuration

### Runtime Configuration

Implement the pallet's configuration trait for your runtime:

```rust
impl pallet_energy_fee::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<AccountId>;
    type GetConstantFee = GetConstantEnergyFee;
    type CustomFee = EnergyFee;
    type FeeTokenBalanced = BalancesVNRG;
    type MainTokenBalanced = BalancesVTRS;
    type EnergyExchange = EnergyExchange;
    type EnergyAssetId = GetVNRG;
    type MainRecycleDestination = MainBurnDestination;
    type FeeRecycleDestination = FeeBurnDestination;
    type OnWithdrawFee = ();
}
```

### Genesis Configuration

Configure initial parameters in your chain spec:

```rust
parameter_types! {
    pub const GetConstantEnergyFee: Balance = 1_000_000_000;
}

GenesisConfig {
    energy_fee: EnergyFeeConfig {
        initial_energy_rate: FixedU128::from(1_000_000_000),
        ..Default::default()
    },
    // ... other genesis configurations
}
```

## Usage

### Basic Fee Management

```rust
// Update burned energy threshold
EnergyFee::update_burned_energy_threshold(
    RuntimeOrigin::root(),
    new_threshold
)?;

// Update block fullness threshold
EnergyFee::update_block_fullness_threshold(
    RuntimeOrigin::root(),
    new_threshold
)?;

// Update fee multiplier
EnergyFee::update_upper_fee_multiplier(
    RuntimeOrigin::root(),
    new_multiplier
)?;
```

### Custom Fee Implementation

Implement the `CustomFee` trait to define specific fee calculation logic:

```rust
impl CustomFee<RuntimeCall, DispatchInfo, Balance, ConstantFee> for EnergyFee {
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfo>,
        calculated_fee: Option<Balance>,
    ) -> CallFee<Balance> {
        match runtime_call {
            RuntimeCall::BalancesVTRS(..) => CallFee::Regular(Self::custom_fee()),
            RuntimeCall::EVM(..) => CallFee::EVM(Self::custom_fee()),
            _ => CallFee::Regular(Self::weight_fee(
                runtime_call,
                dispatch_info,
                calculated_fee
            )),
        }
    }
}
```

## RPC Interface

The pallet provides RPC methods for fee estimation:

```rust
pub trait EnergyFeeApi<BlockHash, AccountId, Balance, Call> {
    fn estimate_gas(&self, request: CallRequest, at: Option<BlockHash>) -> RpcResult<U256>;
    
    fn estimate_call_fee(
        &self,
        account: AccountId,
        encoded_call: Bytes,
        at: Option<BlockHash>,
    ) -> RpcResult<Option<FeeDetails<Balance>>>;
    
    fn vtrs_to_vnrg_swap_rate(
        &self,
        at: Option<BlockHash>
    ) -> RpcResult<Option<u128>>;
}
```

## Security Considerations

1. **Fee Burning Threshold**
    - Configurable maximum energy burning per block
    - Protection against network spam
    - Automatic transaction filtering based on thresholds

2. **Exchange Rate Protection**
    - Controlled token exchange mechanisms
    - Protection against exchange rate manipulation
    - Reserved balance checks during exchanges

3. **Access Control**
    - Root-only parameter updates
    - Protected fee multiplier adjustments
    - Secure threshold management

## Testing

The pallet includes comprehensive tests covering:
- Fee calculation scenarios
- Token exchange mechanisms
- Threshold management
- EVM compatibility
- Dynamic fee adjustments

Run the test suite:

```bash
cargo test -p pallet-energy-fee
```

## Benchmarking

The pallet provides benchmarks for key operations:

```bash
cargo run --release --features runtime-benchmarks \
    benchmark pallet \
    --chain dev \
    --pallet pallet_energy_fee \
    --extrinsic "*" \
    --steps 50 \
    --repeat 20
```

## License

This pallet is part of the core blockchain implementation and inherits its license terms.

## Contributing

Contributions are welcome! Please ensure you:
1. Write tests for new features
2. Update documentation
3. Follow the existing code style
4. Include benchmark updates for modified operations

## Technical Support

For technical support and discussions:
1. Open issues in the repository
2. Join the developer community
3. Consult the implementation documentation