//! Observe native balance changes without making transfers depend on legacy NFT validity.

use super::*;
use frame_support::traits::StoredMap;
use sp_std::marker::PhantomData;

type AccountDataOf<T> = pallet_balances::AccountData<<T as pallet_balances::Config>::Balance>;

/// Wrap the native currency's account store to notice changes from any pallet,
/// including transfers, slashes, fees and vesting. The underlying write's result
/// is preserved even when legacy VIPP data requires repair.
/// All calling pallets must include the observer cost in their declared weights;
/// charging extra weight after dispatch does not provide admission control.
pub struct VippBalanceObserver<T, Store>(PhantomData<(T, Store)>);

impl<T: Config, Store: StoredMap<T::AccountId, AccountDataOf<T>>>
    StoredMap<T::AccountId, AccountDataOf<T>> for VippBalanceObserver<T, Store>
{
    fn get(who: &T::AccountId) -> AccountDataOf<T> {
        Store::get(who)
    }

    fn try_mutate_exists<R, E: From<DispatchError>>(
        who: &T::AccountId,
        mutate: impl FnOnce(&mut Option<AccountDataOf<T>>) -> Result<R, E>,
    ) -> Result<R, E> {
        T::OnVIPPChanged::before_balance_change(who);
        let result = Store::try_mutate_exists(who, mutate)?;
        Pallet::<T>::observe_balance_change(who);
        Ok(result)
    }
}

impl<T: Config> Pallet<T> {
    /// Accounts with unverified or malformed ownership must not accrue new rewards.
    pub fn is_vipp_quarantined(who: &T::AccountId) -> bool {
        VippAccountingQuarantine::<T>::contains_key(who)
    }

    fn quarantine_vipp_account(who: &T::AccountId) {
        let eligible = Self::eligible_vipp_balance(who);
        VippBreachBalance::<T>::mutate(who, |limit| {
            *limit = Some(limit.unwrap_or(eligible).min(eligible));
        });
        if !VippAccountingQuarantine::<T>::contains_key(who) {
            T::OnVIPPChanged::on_balance_breach(who);
            VippAccountingQuarantine::<T>::insert(who, ());
            Self::deposit_event(Event::<T>::VippAccountingQuarantined { owner: who.clone() });
        }
    }

    fn queue_balance_change(who: &T::AccountId) -> bool {
        PendingVippBalanceChanges::<T>::mutate(|pending| {
            pending.contains(who) || pending.try_push(who.clone()).is_ok()
        })
    }

    fn observe_balance_change(who: &T::AccountId) {
        if ProcessingVippBalanceChange::<T>::get().as_ref() == Some(who) {
            // NFT deposit refunds may reenter the account store. The outer
            // bounded reconciliation rechecks the actual balance after burns.
            return;
        }
        if Self::get_claim_balance(who).is_none()
            && T::Nfts::owned_in_collection(&T::VIPPCollectionId::get(), who).next().is_none()
        {
            return;
        }
        Self::check_observed_balance(who);
    }

    /// Capture a deficit immediately without scanning or burning NFTs. The
    /// underlying extrinsic may roll back, so expensive work belongs exclusively
    /// in finalization, outside nested utility transactions.
    pub fn check_observed_balance(who: &T::AccountId) {
        let _ = Self::queue_balance_change(who);
        Self::capture_balance_breach(who);
    }

    fn capture_balance_breach(who: &T::AccountId) {
        if Self::is_vipp_quarantined(who) {
            Self::quarantine_vipp_account(who);
            return;
        }
        if ProcessingVippBalanceChange::<T>::exists() || VippMutationItem::<T>::exists() {
            return;
        }
        let claim = match Self::try_get_claim_balance(who) {
            Ok(Some((claim, _))) => claim,
            Ok(None) => return,
            Err(_) => {
                Self::quarantine_vipp_account(who);
                return;
            },
        };
        if Self::is_vipp_inactive(who) || Self::threshold_meets_vipp_requirements(who, claim) {
            return;
        }
        Self::quarantine_vipp_account(who);
    }

    /// Reconcile the reserved number of captured deficits during finalization.
    /// This must run outside extrinsic transactions: otherwise rollback could
    /// restore the budget counter while the expensive scans had already run.
    /// Unattempted accounts remain queued in order for the next block. Failed
    /// attempts retain their captured deficit for reviewed repair.
    pub fn finish_balance_changes() {
        if ProcessingVippBalanceChange::<T>::exists() || VippMutationItem::<T>::exists() {
            return;
        }
        let mut deferred = BoundedVec::<T::AccountId, T::MaxPendingVippAccounts>::default();
        for who in PendingVippBalanceChanges::<T>::take() {
            Self::capture_balance_breach(&who);
            if !Self::is_vipp_quarantined(&who) {
                continue;
            }
            let used = VippReconciliationsThisBlock::<T>::get();
            if used >= T::MaxVippReconciliationsPerBlock::get() {
                // Every deferred item came from the same bounded queue, so it
                // fits. Preserve the captured minimum even after a refill.
                let _ = deferred.try_push(who);
                continue;
            }
            VippReconciliationsThisBlock::<T>::put(used.saturating_add(1));
            ProcessingVippBalanceChange::<T>::put(&who);
            let result = T::OnVIPPChanged::ensure_accounting_ready().and_then(|_| {
                with_storage_layer(|| {
                    let limit = VippBreachBalance::<T>::get(&who)
                        .ok_or(Error::<T>::VippAccountingUnavailable)?;
                    Self::repair_balance_with_limit(&who, limit)
                })
            });
            ProcessingVippBalanceChange::<T>::kill();
            if let Err(error) = result {
                log::error!(target: "runtime::nac-managing", "VIPP finalization requires reviewed repair: {:?}", error);
            }
        }
        PendingVippBalanceChanges::<T>::mutate(|pending| {
            // Deposit refunds/callbacks may have queued other accounts while
            // reconciling. Older unattempted work has priority; do not overwrite
            // new observations or add duplicates. On capacity overflow their
            // quarantine/evidence still survives for explicit reviewed repair.
            for who in core::mem::take(pending) {
                if !deferred.contains(&who) {
                    let _ = deferred.try_push(who);
                }
            }
            *pending = deferred;
        });
    }
}
