# Changelog

Changelog for the Vitreus runtime.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [211] - 2025-02-15

### Added

- Add `pallet-assets-freezer`
- Add `quote_price_exact_tokens_for_tokens` and `quote_price_tokens_for_exact_tokens` to EnergyBroker runtime API

## [210] - 2025-02-11

### Added

- Add dynamic fee recycling rate calculation based on treasury balance
- Refund fees for successful `payout_stakers` calls
- Runtime API to access parameters used for energy generation and exchange rate calculations

### Changed

- Refactor `pallet-energy-fee` to improve structure and maintainability
- Update fee system to recycle a portion of fees instead of burning them entirely

### Removed

- Remove old migrations

### Fixed

- Fix fee refunds for feeless transactions
