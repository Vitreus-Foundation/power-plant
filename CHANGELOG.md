# Changelog

Changelog for the Vitreus runtime.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions are the runtime's `spec_version`, not [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Improve validation and error reporting in `pallet-energy-broker`

### Fixed

- Keep VIPP NFT IDs and reward caps consistent on VIP entry and additional claims; prevent zero-value minting and issuance before VIPP membership or after deactivation
- Burn VIPP NFTs by numeric claim value, mark the last NFT's owner inactive, and roll back NFT/accounting changes on failure
- Refresh VIP active stake after delegation reductions, slashing and validator removal; use stash/controller mappings for VIP stake and exit penalties
- Accrue December 31 before VIP/VIPP year rollover, process skipped dates without dropping days, and settle completed days before membership, stake or NFT cap changes
- Rebuild VIPP caps from owned NFTs during mint/burn callbacks, including legacy records containing NAC IDs or zero caps
- Prevent zero-point reward claims from receiving an entire pool and allow claims with only a VIPP annual record
- Transfer VIPP NFTs atomically with both owners' NAC thresholds and reward caches; block unaccounted NFT transfer, burn, purchase and swap paths
- Include the exact VIPP threshold boundary and exclude both locked and reserved unvested balances from eligibility
- Capture VIPP balance breaches across native currency operations and defer bounded automatic reconciliation to block finalization; retain deficits for reviewed repair even after a refill
- Add staged, reviewable recovery of legacy VIP/VIPP caches without automatically reconstructing historical points
- Bound calendar work, settle UTC dates from the timestamp inherent, and require approved frozen sources for catch-up after interruptions longer than seven days
- Preserve individual daily rounding during bounded catch-up and refuse to overwrite existing annual records or funded reward pools
- Preserve pre-timestamp stake and eligibility through initialization-time slashes, and require reviewed year selection for deferred last-NFT losses spanning calendar years
- Include VIPP observation in declared native balance-call weights so oversized Utility batches are rejected before execution
- Reserve native balance observation, stake refreshes and reputation-triggered delegation removal during slash execution, and include Treasury session exchanges; preserve existing penalty amounts
- Account for native balance observation in currency callers, nested EVM execution and AtomicSwap, while retaining the existing fee formulas
- Observe SimpleVesting eligibility after named reserves are committed so a self-vesting followed by a refill cannot hide a balance breach
- Cap excluded SimpleVesting funds by actual reserves when generic reserve slashes leave stale named-reserve metadata
- Keep unprocessed VIPP balance breaches queued for subsequent blocks instead of requiring manual repair after the per-block limit is reached
- Track installation of the VIP/VIPP storage schema separately from approval of recovered data; keep unresolved accounting paused
- Use the actual UTC month for penalties when point accounting is paused, and update removed-validator stake caches without rescanning every delegation target
- Fix energy sale tracking in `pallet-energy-broker`
- Roll back failed `pallet-energy-broker` swaps made by other pallets

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
