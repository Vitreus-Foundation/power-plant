# NAC Managing Pallet

## Overview
The NAC (NFTs with Access Control) Managing pallet implements a sophisticated access control system using NFTs on Substrate-based chains. Each NAC represents a user's access level and associated permissions within the system, combining the uniqueness of NFTs with practical access control mechanics.

## Features
- **Access Level NFTs**: Mint and manage NFTs that represent user access levels
- **VIPP Status**: Special status for qualifying accounts with additional privileges
- **Dynamic Level Updates**: Ability to update access levels based on user activity
- **Automated Threshold Management**: Continuous monitoring of account thresholds
- **Reputation Integration**: Built-in integration with reputation system
- **Claim Management**: Handle user claims with automatic VIPP status updates

## Configuration

### Types
```rust
pub trait Config: frame_system::Config + pallet_reputation::Config + pallet_balances::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    type Nfts: Inspect<Self::AccountId> + Mutate<Self::AccountId> + Create<Self::AccountId>;
    type CollectionId: Parameter + Member + Copy + Default + Incrementable;
    type ItemId: Member + Parameter + MaxEncodedLen + Copy + From<u32>;
    type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    type Currency: LockableCurrency<Self::AccountId>;
    type OnVIPPChanged: OnVippStatusHandler<Self::AccountId, Balance, ItemId>;
    // Collection identifiers
    type NftCollectionId: Get<Self::CollectionId>;
    type VIPPCollectionId: Get<Self::CollectionId>;
}
```

### Constants
- `DEFAULT_NAC_LEVEL`: Initial access level for new accounts (default: 1)
- `NAC_LEVEL_ATTRIBUTE_KEY`: NFT attribute key for storing access level
- `CLAIM_AMOUNT_ATTRIBUTE_KEY`: NFT attribute key for storing claim amounts
- `VIPP_STATUS_EXIST`: NFT attribute key for VIPP status tracking

## Main Functions

### Extrinsics

```rust
// Mint new NAC NFT
fn mint(origin: OriginFor<T>, nac_level: u8, owner: T::AccountId) -> DispatchResult

// Update existing NAC
fn update_nft(
    origin: OriginFor<T>,
    new_nac_level: Option<u8>,
    owner: T::AccountId
) -> DispatchResult

// Check NAC level
fn check_nac_level(origin: OriginFor<T>, owner: T::AccountId) -> DispatchResult
```

### Helper Functions

```rust
// Verify access level
pub fn user_has_access(account_id: T::AccountId, desired_access_level: u8) -> bool

// Get current NAC level
pub fn get_nac_level(account_id: &T::AccountId) -> Option<(u8, T::ItemId)>

// Manage VIPP status
pub fn can_mint_vipp(account: &T::AccountId) -> Option<(T::Balance, T::ItemId)>
```

## Security Considerations

### Access Control
- Only authorized origins (`AdminOrigin`) can mint or update NACs
- Access level verification is atomic and deterministic
- NFT attributes are protected against unauthorized modifications

### Threshold Management
- Automatic monitoring of account balances against thresholds
- Systematic VIPP status revocation when thresholds are breached
- Protected claim amount tracking

### Best Practices
1. Always verify access levels before granting permissions
2. Monitor events for important state changes
3. Implement proper error handling for all operations
4. Regularly audit account thresholds and VIPP status
5. Use appropriate origin checks for administrative functions

## Integration Guide

### Basic Setup
1. Include the pallet in your runtime's `Cargo.toml`:
```toml
[dependencies]
pallet-nac-managing = { version = "0.1.0", default-features = false }
```

2. Implement required traits in your runtime:
```rust
impl pallet_nac_managing::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Nfts = Nfts;
    // ... implement other required types
}
```

### Usage Example
```rust
// Mint new NAC
nac_managing::Pallet::<T>::mint(
    RuntimeOrigin::root(),
    desired_level,
    account_id
)?;

// Check access level
if nac_managing::Pallet::<T>::user_has_access(account_id, required_level) {
    // Grant access to protected resource
}
```

## Events
- `NftMinted`: Emitted when new NAC is created
- `NftUpdated`: Emitted when NAC level is modified
- `UserNacLevel`: Emitted when NAC level is checked
- `VippNftMinted`: Emitted when VIPP status is granted

## Errors
- `NftNotFound`: NAC doesn't exist for the account
- `NftAlreadyExist`: Attempting to mint duplicate NAC
- `NacLevelIsIncorrect`: Invalid access level value

## Testing
The pallet includes comprehensive tests covering:
- Basic minting operations
- Level updates and verification
- VIPP status management
- Threshold checking
- Claim processing

Run tests using:
```bash
cargo test
```

## License
Licensed under Apache License, Version 2.0.

## Contributing
We welcome contributions! Please follow these steps:
1. Fork the repository
2. Create your feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

## Support
For technical support or questions, please open an issue in the repository.