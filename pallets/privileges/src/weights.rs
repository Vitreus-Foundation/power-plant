//! Conservative envelopes calibrated from compiled-WASM FRAME measurements.
//!
//! Local baseline: Apple M5 Max, Rust 1.84.1, 2026-10-06. Reproducible commands,
//! raw samples, generated regressions and immutable runtime hashes are retained
//! in investigations/vip-vipp-release-20261006. CPU allowances exceed twice
//! every observed maximum; traced unique IO and FRAME proof estimates are
//! bounded separately. This is a local baseline, not a production hardware claim.
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(missing_docs)]
use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};

pub trait WeightInfo {
    fn accounting(members: u32, days: u32, years: u32) -> Weight;
    fn recovery(accounts: u32, nfts: u32) -> Weight;
    fn set_quarter_revenue() -> Weight;
    fn become_vip_status() -> Weight;
    fn exit_vip() -> Weight;
    fn change_penalty_type() -> Weight;
    fn force_set_vip_points() -> Weight;
    fn force_set_vipp_points() -> Weight;
    fn set_rewards() -> Weight;
    fn claim_rewards() -> Weight;
    fn refresh_active_stake() -> Weight;
    fn resolve_timestamp_breaches() -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn accounting(members: u32, days: u32, years: u32) -> Weight {
        accounting_weight(T::DbWeight::get(), members, days, years)
    }
    fn recovery(accounts: u32, nfts: u32) -> Weight {
        recovery_weight(T::DbWeight::get(), accounts, nfts)
    }
    fn set_quarter_revenue() -> Weight {
        Self::accounting(2048, 7, 1)
    }
    fn become_vip_status() -> Weight {
        operation_weight(T::DbWeight::get(), 25_000_000_000, 2456, 8, 8_388_608)
    }
    fn exit_vip() -> Weight {
        operation_weight(T::DbWeight::get(), 90_000_000_000, 3500, 1430, 13_631_488)
    }
    fn change_penalty_type() -> Weight {
        operation_weight(T::DbWeight::get(), 150_000_000, 12, 3, 8_192)
    }
    fn force_set_vip_points() -> Weight {
        operation_weight(T::DbWeight::get(), 150_000_000, 12, 3, 8_192)
    }
    fn force_set_vipp_points() -> Weight {
        operation_weight(T::DbWeight::get(), 150_000_000, 12, 3, 8_192)
    }
    fn set_rewards() -> Weight {
        operation_weight(T::DbWeight::get(), 10_000_000_000, 2064, 2, 3_145_728)
    }
    fn claim_rewards() -> Weight {
        operation_weight(T::DbWeight::get(), 1_000_000_000, 32, 10, 40_960)
    }
    fn refresh_active_stake() -> Weight {
        operation_weight(T::DbWeight::get(), 16_000_000_000, 272, 2, 6_291_456)
    }
    fn resolve_timestamp_breaches() -> Weight {
        operation_weight(T::DbWeight::get(), 4_000_000_000, 1032, 1030, 3_145_728)
    }
}

impl WeightInfo for () {
    fn accounting(members: u32, days: u32, years: u32) -> Weight {
        accounting_weight(RocksDbWeight::get(), members, days, years)
    }
    fn recovery(accounts: u32, nfts: u32) -> Weight {
        recovery_weight(RocksDbWeight::get(), accounts, nfts)
    }
    fn set_quarter_revenue() -> Weight {
        Self::accounting(2048, 7, 1)
    }
    fn become_vip_status() -> Weight {
        operation_weight(RocksDbWeight::get(), 25_000_000_000, 2456, 8, 8_388_608)
    }
    fn exit_vip() -> Weight {
        operation_weight(RocksDbWeight::get(), 90_000_000_000, 3500, 1430, 13_631_488)
    }
    fn change_penalty_type() -> Weight {
        operation_weight(RocksDbWeight::get(), 150_000_000, 12, 3, 8_192)
    }
    fn force_set_vip_points() -> Weight {
        operation_weight(RocksDbWeight::get(), 150_000_000, 12, 3, 8_192)
    }
    fn force_set_vipp_points() -> Weight {
        operation_weight(RocksDbWeight::get(), 150_000_000, 12, 3, 8_192)
    }
    fn set_rewards() -> Weight {
        operation_weight(RocksDbWeight::get(), 10_000_000_000, 2064, 2, 3_145_728)
    }
    fn claim_rewards() -> Weight {
        operation_weight(RocksDbWeight::get(), 1_000_000_000, 32, 10, 40_960)
    }
    fn refresh_active_stake() -> Weight {
        operation_weight(RocksDbWeight::get(), 16_000_000_000, 272, 2, 6_291_456)
    }
    fn resolve_timestamp_breaches() -> Weight {
        operation_weight(RocksDbWeight::get(), 4_000_000_000, 1032, 1030, 3_145_728)
    }
}

// Measured branches: 2048 combined members / 7 days / one closed year, present
// and absent snapshots, and 64 frozen sources / 366 days / two closed years.
// The m*d term covers the per-account daily rounding loop during catch-up.
fn accounting_weight(db: frame_support::weights::RuntimeDbWeight, members: u32, days: u32, years: u32) -> Weight {
    let m = u64::from(members);
    let d = u64::from(days);
    let y = u64::from(years);
    Weight::from_parts(1_000_000_000, 131_072)
        .saturating_add(Weight::from_parts(25_000_000, 8192).saturating_mul(m))
        .saturating_add(Weight::from_parts(100_000, 0).saturating_mul(m.saturating_mul(d)))
        .saturating_add(Weight::from_parts(10_000_000, 0).saturating_mul(d))
        .saturating_add(db.reads_writes(
            8_u64.saturating_add(m.saturating_mul(3)),
            4_u64.saturating_add(m.saturating_mul(1_u64.saturating_add(y))),
        ))
}

// Each account: 64 real NFTs with a corrupt zero-cap cache, 256 DISTINCT targets,
// and 1024 collaborators per target. Measured proof at 16 accounts: 84.5 MB;
// FRAME estimate: 94.6 MB. Allow 6 MiB/account plus a 1 MiB base. The NFT argument
// remains in the interface; all calls charge the supported 64-NFT maximum so
// smaller requested limits never omit legacy cache decoding or proof work.
fn recovery_weight(db: frame_support::weights::RuntimeDbWeight, accounts: u32, _nfts: u32) -> Weight {
    let a = u64::from(accounts);
    Weight::from_parts(2_000_000_000, 1_048_576)
        .saturating_add(Weight::from_parts(20_000_000_000, 6_291_456).saturating_mul(a))
        .saturating_add(db.reads_writes(
            8_u64.saturating_add(a.saturating_mul(500)),
            4_u64.saturating_add(a.saturating_mul(8)),
        ))
}

// Join covers first-NFT minting and 64-NFT cache reconstruction; exit covers a
// validator with 1024 collaborators and a cooperator with 256 targets. UTC
// settlement and native fee observation are charged separately by their hooks.
fn operation_weight(db: frame_support::weights::RuntimeDbWeight, cpu: u64, reads: u64, writes: u64, proof: u64) -> Weight {
    Weight::from_parts(cpu, proof).saturating_add(db.reads_writes(reads, writes))
}
