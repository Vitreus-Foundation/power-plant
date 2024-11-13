# Claiming Pallet

A Substrate pallet that enables secure token claiming mechanisms with support for vesting schedules. This pallet allows users to claim tokens using Ethereum-style signatures, providing a bridge between Ethereum-based token distributions and Substrate-based chains.

## Overview

The Claiming pallet provides functionality for:
- Secure token claiming using cryptographic signatures
- Vesting schedule support for claimed tokens
- Root-level management of claimable token supply
- Integration with existing balance and vesting systems

## Security Features

- **Ethereum Signature Verification**: Uses `secp256k1` for signature verification, compatible with Ethereum's signing methods
- **Single-Use Claims**: Each claim can only be used once and is removed after successful processing
- **Protected Token Supply**: Only root can mint new tokens for claiming
- **Vesting Protection**: Prevents claiming to accounts that already have vesting schedules
- **Total Supply Tracking**: Maintains accurate tracking of total claimable tokens

## Installation

Add this to your runtime's `Cargo.toml`:

```toml
[dependencies]
pallet-claiming = { version = "0.1.0", default-features = false }
```

## Configuration

The pallet has several configurable options through its `Config` trait:

```rust
pub trait Config: frame_system::Config + pallet_balances::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    type Currency: Currency<Self::AccountId>;
    type VestingSchedule: VestingSchedule<Self::AccountId>;
    type OnClaim: OnClaimHandler<Self::AccountId, BalanceOf<Self>>;
    type Prefix: Get<&'static [u8]>;
    type WeightInfo: WeightInfo;
}
```

## Usage

### Genesis Configuration

Configure initial claims and vesting schedules in your chain spec:

```rust
GenesisConfig {
    claims: vec![
        (ethereum_address, amount),
        // ...
    ],
    vesting: vec![
        (ethereum_address, (total_amount, per_block, starting_block)),
        // ...
    ],
}
```

### Extrinsics

1. **Claim Tokens** (`claim`):
   ```rust
   fn claim(origin: OriginFor<T>, ethereum_signature: EcdsaSignature) -> DispatchResult
   ```
    - Called by users to claim their tokens
    - Requires a valid Ethereum signature
    - Automatically handles vesting if configured

2. **Mint Tokens to Claim** (`mint_tokens_to_claim`):
   ```rust
   fn mint_tokens_to_claim(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResult
   ```
    - Root-only call
    - Adds new tokens to the claiming pool

3. **Mint New Claim** (`mint_claim`):
   ```rust
   fn mint_claim(origin: OriginFor<T>, who: EthereumAddress, value: BalanceOf<T>) -> DispatchResult
   ```
    - Root-only call
    - Creates or adds to existing claims

### Events

- `Claimed { account_id, amount }`: Emitted when tokens are successfully claimed
- `TokenMintedToClaim(amount)`: Emitted when new tokens are added to the claiming pool

### Errors

- `NotEnoughTokensForClaim`: Insufficient tokens in the claiming pool
- `InvalidEthereumSignature`: The provided signature is invalid
- `SignerHasNoClaim`: No claim exists for the signing address
- `VestedBalanceExists`: Cannot claim to an account that already has a vesting schedule

## Testing

The pallet includes comprehensive tests covering:
- Basic claiming functionality
- Vesting schedule integration
- Security checks
- Edge cases

Run tests with:
```bash
cargo test
```

## Benchmarking

The pallet includes benchmarking for all extrinsics. Weights are provided for:
- `mint_tokens_to_claim`
- `claim`
- `mint_claim`

## Migration Support

The pallet includes migration tools for transferring claims between addresses:

```rust
TransferClaim<T, Source, Destination>
```

## Security Considerations

1. **Signature Verification**
    - Always verify signatures using the pallet's `eth_recover` function
    - Use the correct message prefix as configured in your runtime

2. **Access Control**
    - Only root can mint new tokens or claims
    - Users can only claim once per valid signature

3. **Economic Security**
    - Track total supply carefully
    - Consider implications of vesting schedules

## Examples

### Claiming Tokens

```rust
// On the user side (simplified):
let message = account_id.using_encoded(to_ascii_hex);
let signature = eth_sign(message);

// Submit claim
claiming.claim(signature)
```

### Adding New Claims (as Root)

```rust
// Add new claimable tokens
claiming.mint_tokens_to_claim(1000);

// Create new claim
claiming.mint_claim(eth_address, 100);
```

## License

Licensed under Apache 2.0 - see [LICENSE](LICENSE) for details