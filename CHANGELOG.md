# Changelog

Changelog for the Vitreus runtime.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions are the runtime's `spec_version`, not [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Add `max_amount_out` to the EnergyBroker runtime API to query how much a swap path can supply

### Changed

- Improve validation and error reporting in `pallet-energy-broker`
- Set `spec_name` to `vitreus-power-plant-testnet` on testnet
- Restrict XCM `send` to governance and disable `execute` and reserve transfers
- Drop the delivery fee for outbound XCM to parachains
- Make the EnergyBroker `quote_price_*` runtime APIs return `None` when the output exceeds `max_amount_out`
- Make the EnergyFee `estimate_call_fee` runtime API return `None` when the broker cannot sell the VNRG missing for the fee
- Reject a zero value in `set_annual_percentage_rate` of `pallet-dynamic-energy`
- Reject coefficients that can overflow the multiplier in `set_multiplier_coefficients` of `pallet-dynamic-energy`

### Removed

- Remove the `SessionsPerEra` constant from `pallet-dynamic-energy`

### Fixed

- Keep VIPP NFT IDs and reward caps consistent on VIP entry, claims and burns; prevent zero-value minting and roll back failed NFT/accounting changes
- Refresh VIP active stake after delegation reductions, slashing and validator removal, using the correct stash/controller mapping
- Accrue completed UTC days and December 31 before calendar rollover; bound catch-up and require reviewed sources after long interruptions
- Make VIPP transfers atomic across ownership, thresholds and reward caches; prevent unaccounted NFT transfer and burn paths
- Correct VIPP threshold equality and vesting exclusion at explicit checks; defer fee/claim-triggered reconciliation with a bounded per-block budget
- Protect funded reward years and add staged recovery of legacy VIP/VIPP caches without automatically rewriting historical points

- Fix the `QuotePrice` implementation in `pallet-energy-broker` to return `None` when the output exceeds `max_amount_out`
- Fix energy sale tracking in `pallet-energy-broker`
- Roll back failed `pallet-energy-broker` swaps made by other pallets
- Reject EVM transactions in the transaction pool when the broker cannot sell the VNRG missing for their fee
- Fix exchange rate and energy generation edge cases in `pallet-dynamic-energy`

## [214] - 2026-09-25

### Added

- Add energy conversion from LNRG and SNRG to VNRG
- Add `pallet-vitreus-dex` on testnet
- Add `pallet-launchpad` on testnet
- Add `pallet-launch-treasury` on testnet

### Changed

- Refactor `pallet-energy-broker`
- Refactor and update XCM configuration to use VNRG as the fee payment asset
- Set slash defer duration to 6 days

## [213] - 2025-03-23

### Added

- Add new extrinsics to `pallet-privileges`: `force_set_vip_points`, `force_set_vipp_points`, `set_rewards`, `claim_rewards` 
- Add TechnicalCommittee treasury
- Add lower bound for energy generation

### Changed

- Reduce treasury spending limit to 500k VTRS for Council motions
- Lower `ParaDeposit` to 1000 VTRS on testnet
- Raise `LeasePeriod` to 1 week on testnet

## [212] - 2025-03-05

### Added

- Add support for three energy assets
- Add runtime API to retrieve supported swap paths

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
