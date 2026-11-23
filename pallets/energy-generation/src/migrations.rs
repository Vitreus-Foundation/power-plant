use super::*;

use frame_support::{
    migrations::VersionedMigration,
    traits::{OnRuntimeUpgrade, UncheckedOnRuntimeUpgrade},
    weights::Weight,
};

use crate::{Config, Pallet};

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

/// Moving the stake of every stash from a lock to a hold.
pub mod v17 {
    use super::*;
    use crate::asset;
    use frame_support::traits::{fungible::Inspect, InspectLockableCurrency, LockIdentifier};
    use sp_runtime::Saturating;

    #[cfg(feature = "try-runtime")]
    use parity_scale_codec::{Decode, Encode};

    /// The lock that kept the stake before it moved to a hold.
    pub(crate) const STAKING_ID: LockIdentifier = *b"staking ";

    /// Moves the stake of every stash from the [`STAKING_ID`] lock of `OldCurrency` to a hold.
    ///
    /// The hold leaves the existential deposit free. Where the balance doesn't cover
    /// `ledger.total`, the ledger is cut to what it covers by
    /// [`StakingLedger::update_total_stake`](crate::StakingLedger::update_total_stake).
    pub struct VersionUncheckedMigrateStakeToHold<T, OldCurrency>(
        core::marker::PhantomData<(T, OldCurrency)>,
    );

    impl<T, OldCurrency> UncheckedOnRuntimeUpgrade
        for VersionUncheckedMigrateStakeToHold<T, OldCurrency>
    where
        T: Config,
        OldCurrency: InspectLockableCurrency<T::AccountId>,
    {
        fn on_runtime_upgrade() -> Weight {
            let (mut count, mut cut_count, mut below_minimum) = (0u64, 0u64, 0u64);
            let mut cut_total = StakeOf::<T>::zero();

            for (controller, mut ledger) in Ledger::<T>::iter() {
                count += 1;
                let stash = ledger.stash.clone();

                // `free_to_stake` ignores the lock. Holding before removing the lock keeps the
                // consumer that balances has for a lock or a hold, instead of adding it again.
                let held = ledger.total.min(asset::free_to_stake::<T>(&stash));
                if let Err(error) = asset::update_stake::<T>(&stash, held) {
                    log!(error, "Can't hold the stake of {:?}: {:?}", stash, error);
                    continue;
                }
                OldCurrency::remove_lock(STAKING_ID, &stash);

                if held < ledger.total {
                    let cut = ledger.total.saturating_sub(held);
                    ledger.update_total_stake(held);
                    if let Err(error) = Pallet::<T>::update_ledger(&controller, &ledger) {
                        log!(error, "Can't update the ledger of {:?}: {:?}", stash, error);
                    }
                    Pallet::<T>::adjust_cooperator_targets(&stash, ledger.active);

                    let dust = !ledger.active.is_zero()
                        && ledger.active < T::StakeCurrency::minimum_balance();
                    if dust || ledger.active < Pallet::<T>::min_active_bond(&stash) {
                        below_minimum += 1;
                    }
                    log!(
                        info,
                        "Stake of {:?} cut by {:?} to {:?}, {:?} of it active",
                        stash,
                        cut,
                        ledger.total,
                        ledger.active
                    );
                    cut_count += 1;
                    cut_total.saturating_accrue(cut);
                }
            }

            log!(
                info,
                "Moved {} stakes to holds, cut {} of them by {:?} in total, {} below the minimum \
                 of their role or the existential deposit",
                count,
                cut_count,
                cut_total,
                below_minimum
            );
            T::DbWeight::get().reads_writes(count * 5 + cut_count * 6, count * 3 + cut_count * 4)
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
            Ok((Ledger::<T>::iter().count() as u64).encode())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
            let count =
                u64::decode(&mut &state[..]).map_err(|_| "can't decode the ledger count")?;
            frame_support::ensure!(
                Ledger::<T>::iter().count() as u64 == count,
                "the number of ledgers changed"
            );
            for (_, ledger) in Ledger::<T>::iter() {
                frame_support::ensure!(
                    OldCurrency::balance_locked(STAKING_ID, &ledger.stash).is_zero(),
                    "a stake is still under the lock"
                );
                frame_support::ensure!(
                    asset::staked::<T>(&ledger.stash) == ledger.total,
                    "the stake hold differs from the ledger total"
                );
            }
            Ok(())
        }
    }

    /// [`VersionUncheckedMigrateStakeToHold`] that runs once, from storage version 16 to 17.
    pub type MigrateStakeToHold<T, OldCurrency> = VersionedMigration<
        16,
        17,
        VersionUncheckedMigrateStakeToHold<T, OldCurrency>,
        Pallet<T>,
        <T as frame_system::Config>::DbWeight,
    >;
}

pub mod v16 {
    use super::*;
    use sp_runtime::{FixedPointNumber, FixedU128};

    pub struct VersionUncheckedMigrateV15ToV16<T>(core::marker::PhantomData<T>);
    impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateV15ToV16<T> {
        fn on_runtime_upgrade() -> Weight {
            let mut count = 0;

            ErasEnergyPerStakeCurrency::<T>::translate_values(|rate: EnergyOf<T>| {
                count += 1;
                FixedU128::checked_from_rational(1, rate)
            });

            log!(info, "Upgraded {} records", count);
            T::DbWeight::get().reads_writes(count, count)
        }
    }

    pub type MigrateV15ToV16<T> = VersionedMigration<
        15,
        16,
        VersionUncheckedMigrateV15ToV16<T>,
        Pallet<T>,
        <T as frame_system::Config>::DbWeight,
    >;
}

/// Migrating `OffendingValidators` from `Vec<(u32, bool)>` to `Vec<u32>`
pub mod v15 {
    use super::*;

    type DefaultDisablingStrategy = UpToLimitDisablingStrategy;

    pub struct VersionUncheckedMigrateV14ToV15<T>(core::marker::PhantomData<T>);
    impl<T: Config> UncheckedOnRuntimeUpgrade for VersionUncheckedMigrateV14ToV15<T> {
        fn on_runtime_upgrade() -> Weight {
            let mut migrated = v14::OffendingValidators::<T>::take()
                .into_iter()
                .filter(|p| p.1) // take only disabled validators
                .map(|p| p.0)
                .collect::<Vec<_>>();

            // Respect disabling limit
            migrated.truncate(DefaultDisablingStrategy::disable_limit(
                T::SessionInterface::validators().len(),
            ));

            DisabledValidators::<T>::set(migrated);

            log!(info, "v15 applied successfully.");
            T::DbWeight::get().reads_writes(1, 1)
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
            frame_support::ensure!(
                v14::OffendingValidators::<T>::decode_len().is_none(),
                "OffendingValidators is not empty after the migration"
            );
            Ok(())
        }
    }

    pub type MigrateV14ToV15<T> = VersionedMigration<
        14,
        15,
        VersionUncheckedMigrateV14ToV15<T>,
        Pallet<T>,
        <T as frame_system::Config>::DbWeight,
    >;
}

pub mod v14 {
    use super::*;
    use frame_support::pallet_prelude::ValueQuery;

    #[frame_support::storage_alias]
    pub(crate) type OffendingValidators<T: Config> =
        StorageValue<Pallet<T>, Vec<(u32, bool)>, ValueQuery>;
}

pub struct FixCooperatorStake<T>(core::marker::PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for FixCooperatorStake<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut count = 0;
        let mut fixed = 0;

        for (_, ledger) in Ledger::<T>::iter() {
            if let Some(cooperations) = Cooperators::<T>::get(&ledger.stash) {
                let targets_stake = cooperations.total();
                if targets_stake > ledger.active {
                    Pallet::<T>::adjust_cooperator_targets(&ledger.stash, ledger.active);

                    log!(
                        info,
                        "Fix {:?} stake: active/targets/difference: {:?}/{:?}/{:?}",
                        ledger.stash,
                        ledger.active,
                        targets_stake,
                        targets_stake - ledger.active
                    );

                    fixed += 1;
                }
            }
            count += 1;
        }

        log!(info, "Fixed {} out of {} cooperators", fixed, count);

        T::DbWeight::get().reads_writes(count * 2, fixed)
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_: Vec<u8>) -> Result<(), TryRuntimeError> {
        for (_, ledger) in Ledger::<T>::iter() {
            if let Some(cooperations) = Cooperators::<T>::get(&ledger.stash) {
                frame_support::ensure!(
                    cooperations.total() <= ledger.active,
                    "cooperator targets stake must not exceed active stake"
                );
            }
        }

        Ok(())
    }
}
