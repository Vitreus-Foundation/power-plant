# Vitreus - Layer 0 Blockchain Ecosystem🌿⚡

Vitreus is a  next-generation Layer 0 blockchain platform focused on decentralized energy trading and management, built using Substrate and featuring comprehensive EVM compatibility.

[![Substrate version](https://img.shields.io/badge/Substrate-stable2407-brightgreen?logo=Parity%20Substrate)](https://substrate.io)
[![License](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](LICENSE)

## Overview

Vitreus is a sophisticated blockchain ecosystem designed to revolutionize energy trading and management. Built on Substrate with full EVM compatibility, it combines traditional blockchain capabilities with specialized energy-focused features.

### Core Features

- **Dual-Token System**
    - VTRS (Native Token): Platform governance and staking
    - VNRG (Energy Token): Energy trading and fee payments

- **Energy Trading Infrastructure**
    - Automated Market for energy assets
    - Dynamic exchange rates based on network metrics
    - Warehouse mechanism for supply stability
    - Multi-asset support and path-based routing

- **Advanced Security & Access Control**
    - NFT-based access control system (NAC)
    - Reputation-based validation
    - Tiered VIP system with dynamic privileges
    - Comprehensive slashing mechanisms

- **Economic Features**
    - Dynamic fee calculation system
    - Automated treasury management
    - Flexible vesting schedules
    - Reputation-based rewards

## Architecture

Vitreus consists of several specialized pallets working in harmony:

### Core Pallets

1. **Energy Broker**
    - Decentralized energy trading through AMM
    - Automated price discovery
    - Liquidity provision management

2. **Energy Fee**
    - Dual-token transaction fee mechanism
    - Dynamic fee adjustments
    - Automated token exchanges

3. **Energy Generation**
    - Staking and validation mechanisms
    - Energy-per-stake rate calculation
    - Reputation integration

4. **NAC Managing**
    - NFT-based access control
    - VIPP status management
    - Dynamic level updates

### Supporting Pallets

- **Claiming**: Token distribution and vesting
- **Reputation**: Dynamic scoring system
- **Privileges**: VIP membership management
- **Simple Vesting**: Token lock mechanisms
- **Treasury Extension**: Fund recycling and management
- **Faucet**: Token distribution (it's only supported in testnet)

## Getting Started

### Prerequisites

- Rust 1.74 or later
- `wasm32-unknown-unknown` target
- Node.js (for testing)

### Installation

1. Clone the repository:
```bash
git clone https://github.com/your-org/vitreus-power-plant
cd vitreus-power-plant
```

2. Build the node:
```bash
cargo build --release
```

3. Run the node:
```bash
./target/release/vitreus-power-plant-node --dev
```

### Development Chain Configuration

The development chain comes with pre-funded accounts for testing:

- **Alith (Sudo)**: `0xf24FF3a9CF04c71Dbc94D0b566f7A27B94566cac`
- **Baltathar**: `0x3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0`
- Additional test accounts available in development mode

### Network Configuration

For connecting to the network:
- Chain ID: 1943
- Network Name: vitreus-power-plant
- Currency Symbol: VTRS
- RPC URL: http://localhost:9944/

## Development

### Building for Production

```bash
# Build with specific DB backend
cargo build --release --features=with-rocksdb-weights
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific pallet tests
cargo test -p pallet-energy-broker
```

## Security Considerations

1. **Access Control**
    - Multiple validation layers through NAC system
    - Reputation-based restrictions
    - Tiered privilege system

2. **Economic Security**
    - Dynamic fee mechanisms
    - Slashing for malicious behavior
    - Stake-based validation

3. **Network Stability**
    - Warehouse mechanism for price stability
    - Automated treasury management
    - Progressive rate adjustments

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Style

- Follow Rust standard practices
- Use the provided clippy configuration
- Ensure comprehensive test coverage
- Include benchmarks for new features

## Documentation

- [Pallet Documentation](./pallets/ExtrinsicLib.md)

## License

This project is licensed under the GPL-3.0 License - see the [LICENSE](LICENSE) file for details.

## Support

- Open an issue for bug reports
- Join our [community](https://discord.gg/vitreus)
- Check technical documentation

## Acknowledgments

Built using:
- [Substrate](https://substrate.io/)
- [Frontier](https://github.com/paritytech/frontier)