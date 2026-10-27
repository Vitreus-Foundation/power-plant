//! Runtime weights for bounded VIPP minting paths.
//!
//! Generated from FRAME 42.0.1 Wasm measurements on 2026-10-27, Apple M5 Max.
//! Both mainnet-runtime and testnet-runtime; 64 NFTs; 20 repeats x 2 external repeats.
//! benchmark pallet --chain <instrumented-development-spec> --pallet vipp_minting
//! --extrinsic "*" --steps 2 --repeat 20 --external-repeat 2 --heap-pages 4096
//! --wasm-execution compiled --default-pov-mode max-encoded-len
//! The envelope takes each maximum across both networks and all branches, with
//! 20% execution-time headroom over the slowest raw sample. Database costs and
//! FRAME estimated proof sizes are retained separately. The original call budget
//! is also retained; these measurements cover NFT processing, not all staking states.
//!
//! network | case | max ns | reads | writes | estimated proof bytes
//! mainnet | vip_entry_mint | 1084000 | 143 | 7 | 195089
//! mainnet | vip_entry_no_mint | 743000 | 138 | 2 | 195089
//! mainnet | vip_entry_limit | 726000 | 138 | 0 | 195089
//! mainnet | claim_mint | 836000 | 148 | 15 | 195089
//! mainnet | claim_limit | 452000 | 141 | 0 | 192192
//! testnet | vip_entry_mint | 707000 | 143 | 7 | 195089
//! testnet | vip_entry_no_mint | 1362000 | 138 | 2 | 195089
//! testnet | vip_entry_limit | 403000 | 138 | 0 | 195089
//! testnet | claim_mint | 785000 | 148 | 15 | 195089
//! testnet | claim_limit | 479000 | 141 | 0 | 192192

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};
use pallet_claiming::weights::WeightInfo as ClaimWeightInfo;
use pallet_privileges::weights::{
    SubstrateWeight as PrivilegeWeights, WeightInfo as PrivilegeWeightInfo,
};

pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> PrivilegeWeightInfo for WeightInfo<T> {
    fn become_vip_status() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::become_vip_status()
            .saturating_add(Weight::from_parts(1_634_400_000, 195_089))
            .saturating_add(T::DbWeight::get().reads(143))
            .saturating_add(T::DbWeight::get().writes(7))
    }

    fn set_quarter_revenue() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::set_quarter_revenue()
    }

    fn exit_vip() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::exit_vip()
    }

    fn change_penalty_type() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::change_penalty_type()
    }

    fn force_set_vip_points() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::force_set_vip_points()
    }

    fn force_set_vipp_points() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::force_set_vipp_points()
    }

    fn set_rewards() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::set_rewards()
    }

    fn claim_rewards() -> Weight {
        <PrivilegeWeights<T> as PrivilegeWeightInfo>::claim_rewards()
    }
}

impl<T: frame_system::Config> ClaimWeightInfo for WeightInfo<T> {
    fn claim() -> Weight {
        <() as ClaimWeightInfo>::claim()
            .saturating_add(Weight::from_parts(1_003_200_000, 195_089))
            .saturating_add(T::DbWeight::get().reads(148))
            .saturating_add(T::DbWeight::get().writes(15))
    }

    fn mint_tokens_to_claim() -> Weight {
        <() as ClaimWeightInfo>::mint_tokens_to_claim()
    }

    fn mint_claim() -> Weight {
        <() as ClaimWeightInfo>::mint_claim()
    }
}
