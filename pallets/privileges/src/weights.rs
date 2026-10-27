//! Conservative weight allowances for bounded VIP/VIPP work.
#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(missing_docs)]
use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};

pub trait WeightInfo {
    fn accounting(members: u32, days: u32, years: u32) -> Weight;
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

// Account/day work preserves per-day rounding; year segments bound annual writes.
// Snapshot reads are included whether or not a boundary snapshot is present.
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

// Join covers first-NFT minting and 64-NFT cache reconstruction; exit covers a
// validator with 1024 collaborators and a cooperator with 256 targets. UTC
// settlement and native fee observation are charged separately by their hooks.
fn operation_weight(db: frame_support::weights::RuntimeDbWeight, cpu: u64, reads: u64, writes: u64, proof: u64) -> Weight {
    Weight::from_parts(cpu, proof).saturating_add(db.reads_writes(reads, writes))
}
