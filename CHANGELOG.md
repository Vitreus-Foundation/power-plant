# Changelog

Changelog for the Vitreus runtime.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions are the runtime's `spec_version`, not [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Add the `EnergyExchangeFailed` event to `pallet-treasury-extension`

### Changed

- Refactor the energy exchanges of `pallet-energy-fee` and `pallet-treasury-extension` and add `max_amount_out` to them
- Limit the treasury's energy sales in `pallet-treasury-extension` to the broker's liquidity
- Report the energy sold per asset in the `EnergyExchanged` event of `pallet-treasury-extension`
- Set `SpendPeriod` of the treasury to 1 day and `SpendThreshold` of `pallet-treasury-extension` to 1%, on testnet
- Raise the launchpad's default graduation target on testnet from 3,000 to 4,600,000 VTRS
- Let only Root submit public proposals in `pallet-democracy` and lengthen its periods, on testnet

### Removed

- Remove `pallet-uniques` and other unused dependencies from the runtime
- Remove the unused `NacRunner` and other dead EVM code from the runtime

### Fixed

- Make the treasury's VNRG sales in `pallet-treasury-extension` atomic
- Stop re-creating the treasury's energy accounts every session in `pallet-treasury-extension`

## [215] - 2026-10-19

### Added

- Add `max_amount_out` to the EnergyBroker runtime API to query how much a swap path can supply
- Add a `Launchpad` proxy type on testnet

### Changed

- Improve validation and error reporting in `pallet-energy-broker`
- Make the EnergyBroker `quote_price_*` runtime APIs return `None` when the output exceeds `max_amount_out`
- Make the EnergyFee `estimate_call_fee` runtime API return `None` when the broker cannot sell the VNRG missing for the fee
- Reject a zero value in `set_annual_percentage_rate` of `pallet-dynamic-energy`
- Reject coefficients that can overflow the multiplier in `set_multiplier_coefficients` of `pallet-dynamic-energy`
- Restrict XCM `send` to governance and disable `execute` and reserve transfers
- Drop the delivery fee for outbound XCM to parachains
- Raise the storage deposits of `pallet-proxy`, `pallet-multisig`, `pallet-preimage` and the voting bond of `pallet-elections-phragmen`
- Set `spec_name` to `vitreus-power-plant-testnet` on testnet
- Make successful `pallet-launchpad` buys and sells free (`Pays::No`) at or above 200× one call's energy fee, priced through the energy broker, on testnet
- Require the recipient of `pallet-vitreus-dex` `swap_exact_tokens_for_tokens` to be the signer, on testnet

### Removed

- Remove the `NativeAsset` constant from `pallet-energy-broker`
- Remove the `SessionsPerEra` constant from `pallet-dynamic-energy`
- Remove `pallet-manual-bridge` and clear its storage

### Fixed

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
