use super::*;

use frame_support::{
    traits::{Get, GetStorageVersion, StorageVersion},
    weights::Weight,
};
use frame_system::pallet_prelude::BlockNumberFor;
use log::{info, warn};

const LOG_TARGET: &str = "simple-vesting";

pub(crate) fn migrate_to_v1<T: Config>() -> Weight {
    let onchain_version = Pallet::<T>::on_chain_storage_version();
    if onchain_version < 1 {
        info!(target: LOG_TARGET, "Fix genesis vesting");

        let mut count = 0;

        Vesting::<T>::translate::<VestingInfo<BalanceOf<T>, BlockNumberFor<T>>, _>(
            |who, mut schedule| {
                frame_system::Pallet::<T>::inc_providers(&who);

                let amount = BalanceOf::<T>::from(1u8);
                match T::Currency::reserve_named(&VESTING_ID, &who, amount) {
                    Ok(_) => {
                        schedule.locked.saturating_accrue(amount);

                        let free = T::Currency::free_balance(&who);
                        let reserved = T::Currency::reserved_balance_named(&VESTING_ID, &who);
                        info!(target: LOG_TARGET, "{:?}: fix reserved balance, free: {:?}, reserved: {:?}", who, free, reserved);
                    },
                    Err(err) => {
                        warn!(target: LOG_TARGET, "{:?}: unable to fix reserved balance: {:?}", who, err);
                    },
                }

                count += 1;

                Some(schedule)
            },
        );

        info!(target: LOG_TARGET, "Update {} items", count);

        StorageVersion::new(1).put::<Pallet<T>>();

        T::DbWeight::get().reads_writes(5 * count as u64, 3 * count as u64)
    } else {
        info!(target: LOG_TARGET, "Skip migration to v1");
        Weight::zero()
    }
}
