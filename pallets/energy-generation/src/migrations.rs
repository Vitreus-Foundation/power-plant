use core::marker::PhantomData;

use super::*;
use crate::slashing::{SlashEntityOf, SlashEntityPerbill, SpanRecord};
use crate::{Config, Pallet};
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use pallet_reputation::ReputationPoint;

pub struct UpdateSlashStorages<T>(PhantomData<T>);

#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
struct OldUnappliedSlash<AccountId> {
    pub validator: AccountId,
    pub own: ReputationPoint,
    pub others: Vec<(AccountId, ReputationPoint)>,
    pub reporters: Vec<AccountId>,
    pub payout: ReputationPoint,
}

#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct OldSpanRecord {
    pub slashed: ReputationPoint,
    pub paid_out: ReputationPoint,
}

impl<T: Config> OnRuntimeUpgrade for UpdateSlashStorages<T> {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() > 13 {
            return Zero::zero();
        }

        let mut weight = T::DbWeight::get().reads(0);

        UnappliedSlashes::<T>::translate::<Vec<OldUnappliedSlash<T::AccountId>>, _>(|_, value| {
            let new_value = value
                .into_iter()
                .map(|old_unapplied_slash| {
                    let OldUnappliedSlash { validator, own, others, reporters, payout } =
                        old_unapplied_slash;
                    let own = SlashEntityOf::<T>::new(own, Zero::zero());
                    let others = others
                        .into_iter()
                        .map(|(account, reputation)| {
                            (account, SlashEntityOf::<T>::new(reputation, Zero::zero()))
                        })
                        .collect();
                    let payout = SlashEntityOf::<T>::new(payout, Zero::zero());
                    UnappliedSlash::new(validator, own, others, reporters, payout)
                })
                .collect();

            weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            Some(new_value)
        });

        ValidatorSlashInEra::<T>::translate::<(Perbill, ReputationPoint), _>(
            |_, _, (ratio, reputation)| {
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
                let slash_ratio = SlashEntityPerbill::new(ratio, Zero::zero());
                Some((slash_ratio, SlashEntityOf::<T>::new(reputation, Zero::zero())))
            },
        );
        CooperatorSlashInEra::<T>::translate::<ReputationPoint, _>(|_, _, reputation| {
            weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            Some(SlashEntityOf::<T>::new(reputation, Zero::zero()))
        });
        SpanSlash::<T>::translate::<OldSpanRecord, _>(|_, old_span_record| {
            weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            let OldSpanRecord { slashed, paid_out } = old_span_record;
            let new_slashed = SlashEntityOf::<T>::new(slashed, Zero::zero());
            let new_paid_out = SlashEntityOf::<T>::new(paid_out, Zero::zero());
            Some(SpanRecord::new(new_slashed, new_paid_out))
        });

        StorageVersion::new(14).put::<Pallet<T>>();
        weight.saturating_add(T::DbWeight::get().writes(1))
    }
}
