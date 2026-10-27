//! VIPP ownership changes must update the entitlement and both NAC thresholds atomically.

use super::*;
use frame_support::{
    ensure,
    traits::tokens::{nonfungibles_v2::Transfer, Locker},
};
use sp_runtime::traits::{CheckedAdd, CheckedSub};
use sp_std::marker::PhantomData;

/// Keep account certificates bound to their account and prevent NFT transfer,
/// sale, swap and burn paths from bypassing VIPP accounting.
/// Other collections retain their existing transfer and burn rules.
pub struct VippNftLocker<T>(PhantomData<T>);

impl<T: Config> Locker<T::CollectionId, T::ItemId> for VippNftLocker<T> {
    fn is_locked(collection: T::CollectionId, item: T::ItemId) -> bool {
        collection == T::NftCollectionId::get()
            || (collection == T::VIPPCollectionId::get()
                && VippMutationItem::<T>::get() != Some(item))
    }
}

impl<T: Config> Pallet<T> {
    /// Allow exactly one trusted NFT mutation and always clear its temporary permission.
    /// A failed operation rolls back the permission together with its NFT writes.
    pub(crate) fn with_vipp_mutation(
        item_id: T::ItemId,
        mutate: impl FnOnce() -> DispatchResult,
    ) -> DispatchResult {
        ensure!(!VippMutationItem::<T>::exists(), Error::<T>::InvalidVippTransfer);
        with_storage_layer(|| {
            VippMutationItem::<T>::put(item_id);
            mutate()?;
            VippMutationItem::<T>::kill();
            Ok(())
        })
    }

    /// Transfer an existing positive entitlement to an eligible VIP member.
    /// Ineligible recipients are rejected before ownership changes; this does not
    /// reactivate an inactive NAC or silently discard either user's entitlement.
    pub fn do_transfer_vipp(
        sender: &T::AccountId,
        item_id: T::ItemId,
        destination: &T::AccountId,
    ) -> DispatchResult {
        let collection = T::VIPPCollectionId::get();
        ensure!(
            T::Nfts::owner(&collection, &item_id).as_ref() == Some(sender),
            Error::<T>::NotVippOwner
        );
        if sender == destination {
            return Ok(());
        }
        ensure!(!Self::is_vipp_inactive(sender), Error::<T>::InvalidVippTransfer);
        ensure!(
            !Self::is_vipp_inactive(destination) && T::OnVIPPChanged::can_receive_vipp(destination),
            Error::<T>::InvalidVippRecipient
        );

        // Both scans are bounded by MaxVippNftsPerAccount. Validate all attributes
        // before changing ownership, including legacy NFTs with malformed values.
        let sender_claims = Self::owned_vipp_claims(sender)?;
        let amount = sender_claims
            .iter()
            .find(|(id, _)| *id == item_id)
            .map(|(_, amount)| *amount)
            .ok_or(Error::<T>::InvalidClaimAttribute)?;
        ensure!(!amount.is_zero(), Error::<T>::InvalidVippTransfer);
        let recipient_claims = Self::owned_vipp_claims(destination)?;
        ensure!(
            recipient_claims.len() < T::MaxVippNftsPerAccount::get() as usize,
            Error::<T>::TooManyVippNfts
        );

        let (sender_claim, sender_nac) = Self::transfer_claim_balance(sender, false)?;
        let (recipient_claim, recipient_nac) = Self::transfer_claim_balance(destination, true)?;
        // Moving one NFT must not leave the other owned NFTs backed by a
        // smaller NAC claim. Legacy transfers can make either side inconsistent.
        for (claims, claim) in
            [(&sender_claims, sender_claim), (&recipient_claims, recipient_claim)]
        {
            let owned_total = claims
                .iter()
                .try_fold(T::Balance::zero(), |total, (_, amount)| total.checked_add(amount))
                .ok_or(Error::<T>::InvalidVippTransfer)?;
            ensure!(owned_total <= claim, Error::<T>::InvalidVippTransfer);
        }
        let remaining_claim =
            sender_claim.checked_sub(&amount).ok_or(Error::<T>::InvalidVippTransfer)?;
        let updated_claim =
            recipient_claim.checked_add(&amount).ok_or(Error::<T>::InvalidVippTransfer)?;
        ensure!(
            Self::threshold_meets_vipp_requirements(sender, remaining_claim)
                && Self::threshold_meets_vipp_requirements(destination, updated_claim),
            Error::<T>::InsufficientVippBalance
        );

        with_storage_layer(|| {
            T::OnVIPPChanged::ensure_accounting_ready()?;
            Self::with_vipp_mutation(item_id, || {
                T::Nfts::transfer(&collection, &item_id, destination)
            })?;
            T::Nfts::set_attribute(
                &T::NftCollectionId::get(),
                &sender_nac,
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                &remaining_claim.encode(),
            )?;
            T::Nfts::set_attribute(
                &T::NftCollectionId::get(),
                &recipient_nac,
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                &updated_claim.encode(),
            )?;
            if T::Nfts::owned_in_collection(&collection, sender).next().is_none() {
                Self::set_inactive_attribute_vipp(sender)?;
            }
            // Completed days were settled before ownership changed. Remove the
            // sender's cap and register the recipient's after moving the backing.
            T::OnVIPPChanged::burn_vipp_nft(sender, item_id)?;
            T::OnVIPPChanged::mint_vipp(destination, Perbill::from_percent(95) * amount, item_id)?;
            // A transfer does not move native funds. If a callback ever changes
            // backing, fail atomically instead of burning unrelated entitlements.
            ensure!(
                Self::threshold_meets_vipp_requirements(sender, remaining_claim)
                    && Self::threshold_meets_vipp_requirements(destination, updated_claim),
                Error::<T>::InsufficientVippBalance
            );
            Ok(())
        })
    }

    fn transfer_claim_balance(
        account: &T::AccountId,
        allow_missing_claim: bool,
    ) -> Result<(T::Balance, T::ItemId), DispatchError> {
        let (_, nac) = Self::get_nac_level(account).ok_or(Error::<T>::NftNotFound)?;
        let amount = match T::Nfts::system_attribute(
            &T::NftCollectionId::get(),
            Some(&nac),
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
        ) {
            Some(bytes) => T::Balance::decode_all(&mut bytes.as_slice())
                .map_err(|_| Error::<T>::InvalidClaimAttribute)?,
            None if allow_missing_claim => T::Balance::zero(),
            None => return Err(Error::<T>::InvalidClaimAttribute.into()),
        };
        Ok((amount, nac))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::{assert_noop, assert_ok};

    fn prepare_transfer() -> ItemId {
        let _ = Balances::make_free_balance_be(&1, 10_000);
        let _ = Balances::make_free_balance_be(&2, 10_000);
        assert_ok!(NacManaging::create_collection(&1));
        assert_ok!(NacManaging::create_collection(&1));
        assert_ok!(NacManaging::do_mint(123, 1));
        assert_ok!(NacManaging::do_mint(124, 2));
        assert_ok!(NacManaging::on_claim(&1, 1000u64, None));
        NacManaging::mint_vipp_nft(&1).unwrap().unwrap().1
    }

    #[test]
    fn transfer_rolls_back_ownership_when_a_nac_attribute_write_fails() {
        new_test_ext().execute_with(|| {
            let item = prepare_transfer();
            FailAttributeWrite::set(Some((0, CLAIM_AMOUNT_ATTRIBUTE_KEY.to_vec())));
            assert_noop!(
                NacManaging::transfer_vipp(RuntimeOrigin::signed(1), item, 2),
                DispatchError::Other("injected NFT attribute failure")
            );
            assert_eq!(Nfts::owner(1, item), Some(1));
            assert_eq!(NacManaging::get_claim_balance(&1).unwrap().0, 1000);
            assert!(!NacManaging::is_vipp_inactive(&1));
            assert!(VippMutationItem::<Test>::get().is_none());
        });
    }

    #[test]
    fn transfer_rolls_back_both_thresholds_and_inactive_flag_when_callback_fails() {
        new_test_ext().execute_with(|| {
            let item = prepare_transfer();
            FailVippCallback::set(true);
            assert_noop!(
                NacManaging::transfer_vipp(RuntimeOrigin::signed(1), item, 2),
                DispatchError::Other("injected VIPP callback failure")
            );
            assert_eq!(Nfts::owner(1, item), Some(1));
            assert!(NacManaging::get_claim_balance(&2).is_none());
            assert!(!NacManaging::is_vipp_inactive(&1));
            assert!(VippMutationItem::<Test>::get().is_none());
        });
    }

    #[test]
    fn transfer_rejects_a_full_recipient_before_scanning_or_moving_more_nfts() {
        new_test_ext().execute_with(|| {
            let item = prepare_transfer();
            for id in 0..64 {
                assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::mint_into(
                    &1,
                    &id,
                    &2,
                    &ItemConfig::default(),
                    true
                ));
                assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
                    &1,
                    &id,
                    &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                    &1u64.encode()
                ));
            }
            assert_noop!(
                NacManaging::transfer_vipp(RuntimeOrigin::signed(1), item, 2),
                Error::<Test>::TooManyVippNfts
            );
        });
    }

    #[test]
    fn transfer_rejects_threshold_overflow_without_saturation() {
        new_test_ext().execute_with(|| {
            let item = prepare_transfer();
            assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
                &0,
                &124,
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                &Balance::MAX.encode()
            ));
            assert_noop!(
                NacManaging::transfer_vipp(RuntimeOrigin::signed(1), item, 2),
                Error::<Test>::InvalidVippTransfer
            );
        });
    }

    #[test]
    fn transfer_rejects_a_legacy_zero_value_nft_without_creating_membership() {
        new_test_ext().execute_with(|| {
            let item = prepare_transfer();
            assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
                &1,
                &item,
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                &0u64.encode()
            ));
            assert_noop!(
                NacManaging::transfer_vipp(RuntimeOrigin::signed(1), item, 2),
                Error::<Test>::InvalidVippTransfer
            );
        });
    }

    #[test]
    fn transfer_does_not_treat_malformed_recipient_threshold_as_zero() {
        for malformed in [vec![1], [0u64.encode(), vec![1]].concat()] {
            new_test_ext().execute_with(|| {
                let item = prepare_transfer();
                assert_ok!(<Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
                    &0,
                    &124,
                    &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                    &malformed
                ));
                assert_noop!(
                    NacManaging::transfer_vipp(RuntimeOrigin::signed(1), item, 2),
                    Error::<Test>::InvalidClaimAttribute
                );
            });
        }
    }
}
