//! Bounded threshold reconciliation triggered by fee and claim hooks.
//!
//! Native AccountStore observation is intentionally outside this focused patch.
//! Arbitrary balance changes are checked only when an explicit hook runs.

use super::*;

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

    /// Capture a deficit immediately without scanning or burning NFTs. The
    /// underlying extrinsic may roll back, so expensive work belongs exclusively
    /// in finalization, outside nested utility transactions.
    pub fn check_observed_balance(who: &T::AccountId) {
        // Ordinary fee payers with no VIPP claim/ownership must not consume the
        // bounded queue. Decode failures still need quarantine and review.
        if matches!(Self::try_get_claim_balance(who), Ok(None))
            && !Self::is_vipp_quarantined(who)
            && T::Nfts::owned_in_collection(&T::VIPPCollectionId::get(), who).next().is_none()
        {
            return;
        }
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
