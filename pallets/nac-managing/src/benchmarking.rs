//! Integrated VIPP benchmarks. Setup is excluded from the measured block.
use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::Currency;
use frame_system::RawOrigin;

/// An amount safely above the native existential deposit on all supported runtimes.
pub fn unit<T: Config>() -> T::Balance {
    <T as Config>::Currency::minimum_balance()
        .max(1u32.into())
        .saturating_mul(1_000_000u32.into())
}

/// Set a valid claim attribute without invoking an unrelated claim extrinsic.
pub fn set_claim<T: Config>(who: &T::AccountId, raw: T::Balance) -> DispatchResult {
    let nac = Pallet::<T>::get_nac_level(who).ok_or(Error::<T>::NftNotFound)?.1;
    T::Nfts::set_attribute(
        &T::NftCollectionId::get(),
        &nac,
        &CLAIM_AMOUNT_ATTRIBUTE_KEY,
        &raw.encode(),
    )
}

/// Prepare real NFTs and attributes; the callback creates the actual VIP/VIPP cache.
/// This helper exists only in benchmark builds and deliberately avoids claim economics.
pub fn prepare_member<T: Config>(index: u32, nfts: u32) -> Result<T::AccountId, DispatchError> {
    prepare_member_for::<T>(account("vipp-benchmark-member", index, 0), index, nfts)
}

/// The same setup for a predetermined account, such as a pallet reward pot.
pub fn prepare_member_for<T: Config>(
    who: T::AccountId,
    index: u32,
    nfts: u32,
) -> Result<T::AccountId, DispatchError> {
    // FRAME enables events at block one before dispatch; settle our fixture at
    // that same block so it is not mistaken for pre-timestamp initialization.
    if frame_system::Pallet::<T>::block_number().is_zero() {
        frame_system::Pallet::<T>::set_block_number(1u32.into());
    }
    let admin: T::AccountId = account("vipp-benchmark-admin", 0, 0);
    let amount = unit::<T>();
    let _ = <T as Config>::Currency::make_free_balance_be(
        &admin,
        amount.saturating_mul(1_000_000u32.into()),
    );
    let config = CollectionConfig {
        settings: CollectionSettings::all_enabled(),
        max_supply: None,
        mint_settings: MintSettings::default(),
    };
    for collection in [T::NftCollectionId::get(), T::VIPPCollectionId::get()] {
        // The runtime chain spec may already contain either reserved collection.
        let _ = T::Nfts::create_collection_with_id(collection, &admin, &admin, &config);
    }
    let _ =
        <T as Config>::Currency::make_free_balance_be(&who, amount.saturating_mul(1_000u32.into()));
    if Pallet::<T>::get_nac_level(&who).is_none() {
        Pallet::<T>::do_mint((1_000_000 + index).into(), who.clone())?;
    }
    let nac = Pallet::<T>::get_nac_level(&who).ok_or(Error::<T>::NftNotFound)?.1;
    T::Nfts::set_attribute(
        &T::NftCollectionId::get(),
        &nac,
        &CLAIM_AMOUNT_ATTRIBUTE_KEY,
        &amount.saturating_mul(nfts.into()).encode(),
    )?;
    for i in 0..nfts {
        let item = (2_000_000 + index * 128 + i).into();
        T::Nfts::mint_into(
            &T::VIPPCollectionId::get(),
            &item,
            &who,
            &ItemConfig { settings: ItemSettings::all_enabled() },
            true,
        )?;
        T::Nfts::set_attribute(
            &T::VIPPCollectionId::get(),
            &item,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &amount.encode(),
        )?;
    }
    T::UnvestedBalance::benchmark_prepare(&who)?;
    T::OnVIPPChanged::benchmark_prepare_member(&who, amount.saturating_mul(128u32.into()))?;
    Ok(who)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn mint() -> Result<(), BenchmarkError> {
        prepare_member::<T>(7, 0)?;
        let who: T::AccountId = account("nac-unfunded-owner", 0, 0);
        #[extrinsic_call]
        _(RawOrigin::Root, 2, who.clone());
        assert_eq!(Pallet::<T>::get_nac_level(&who).unwrap().0, 2);
        Ok(())
    }

    #[benchmark]
    fn update_nft() -> Result<(), BenchmarkError> {
        let who = prepare_member::<T>(8, 0)?;
        #[extrinsic_call]
        _(RawOrigin::Root, Some(2), who.clone());
        assert_eq!(Pallet::<T>::get_nac_level(&who).unwrap().0, 2);
        Ok(())
    }

    #[benchmark]
    fn check_nac_level() -> Result<(), BenchmarkError> {
        let who = prepare_member::<T>(9, 0)?;
        #[extrinsic_call]
        _(RawOrigin::Root, who.clone());
        assert!(Pallet::<T>::get_nac_level(&who).is_some());
        Ok(())
    }

    /// Account creation also invokes NAC minting through System::OnNewAccount.
    #[benchmark]
    fn observe_new_account() -> Result<(), BenchmarkError> {
        #[cfg(test)]
        crate::mock::ObserveVippBalances::set(true);
        prepare_member::<T>(10, 0)?;
        let who: T::AccountId = account("observer-fresh-account", 0, 0);
        let balance = unit::<T>();
        #[block]
        {
            let _ = <T as Config>::Currency::make_free_balance_be(&who, balance);
        }
        assert_eq!(<T as Config>::Currency::free_balance(&who), balance);
        assert!(Pallet::<T>::get_nac_level(&who).is_some());
        Ok(())
    }

    #[benchmark]
    fn observe_malformed_claim() -> Result<(), BenchmarkError> {
        #[cfg(test)]
        crate::mock::ObserveVippBalances::set(true);
        let who = prepare_member::<T>(11, 64)?;
        T::OnVIPPChanged::benchmark_fill_breach_queue(1024);
        let nac = Pallet::<T>::get_nac_level(&who).unwrap().1;
        T::Nfts::set_attribute(
            &T::NftCollectionId::get(),
            &nac,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &[1],
        )?;
        let pending: Vec<T::AccountId> = (0..T::MaxPendingVippAccounts::get())
            .map(|i| account("pending-vipp-malformed", i, 0))
            .collect();
        PendingVippBalanceChanges::<T>::put(
            BoundedVec::<_, T::MaxPendingVippAccounts>::try_from(pending)
                .map_err(|_| BenchmarkError::Stop("pending queue bound"))?,
        );
        frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number().saturating_add(1u32.into()),
        );
        let balance = unit::<T>();
        #[block]
        {
            let _ = <T as Config>::Currency::make_free_balance_be(&who, balance);
        }
        assert!(Pallet::<T>::is_vipp_quarantined(&who));
        Ok(())
    }

    /// Burn every NFT, including the callbacks that rescan remaining ownership.
    #[benchmark]
    fn check_vipp_threshold(n: Linear<1, 64>) -> Result<(), BenchmarkError> {
        let who = prepare_member::<T>(1, n)?;
        // A captured deficit of zero forces all burns, regardless of refunded deposits.
        VippAccountingQuarantine::<T>::insert(&who, ());
        VippBreachBalance::<T>::insert(&who, T::Balance::zero());
        #[extrinsic_call]
        repair_vipp_balance(RawOrigin::Root, who.clone());
        assert!(Pallet::<T>::owned_vipp_claims(&who)?.is_empty());
        assert!(Pallet::<T>::is_vipp_inactive(&who));
        Ok(())
    }

    /// Both ends are populated: the receiver reaches n NFTs after the transfer.
    #[benchmark]
    fn transfer_vipp(n: Linear<1, 64>) -> Result<(), BenchmarkError> {
        let sender = prepare_member::<T>(2, n)?;
        let destination = prepare_member::<T>(3, n.saturating_sub(1))?;
        let item = (2_000_000 + 2 * 128).into();
        #[extrinsic_call]
        _(RawOrigin::Signed(sender.clone()), item, destination.clone());
        assert_eq!(T::Nfts::owner(&T::VIPPCollectionId::get(), &item), Some(destination));
        assert_eq!(Pallet::<T>::owned_vipp_claims(&sender)?.len(), n as usize - 1);
        Ok(())
    }

    /// First NFT for a recipient scans the population bound before adding its cache.
    #[benchmark]
    fn transfer_new_recipient() -> Result<(), BenchmarkError> {
        T::OnVIPPChanged::benchmark_populate_members(1022);
        let sender = prepare_member::<T>(5, 64)?;
        let destination = prepare_member::<T>(6, 0)?;
        let item = (2_000_000 + 5 * 128).into();
        #[extrinsic_call]
        transfer_vipp(RawOrigin::Signed(sender), item, destination.clone());
        assert_eq!(T::Nfts::owner(&T::VIPPCollectionId::get(), &item), Some(destination));
        Ok(())
    }

    /// Actual native AccountStore path, filled pending queue, newly breached member.
    #[benchmark]
    fn observe_balance_change() -> Result<(), BenchmarkError> {
        #[cfg(test)]
        crate::mock::ObserveVippBalances::set(true);
        let who = prepare_member::<T>(4, 64)?;
        T::OnVIPPChanged::benchmark_fill_breach_queue(1024);
        let pending: Vec<T::AccountId> = (0..T::MaxPendingVippAccounts::get())
            .map(|i| account("pending-vipp", i, 0))
            .collect();
        PendingVippBalanceChanges::<T>::put(
            BoundedVec::<_, T::MaxPendingVippAccounts>::try_from(pending)
                .map_err(|_| BenchmarkError::Stop("pending queue bound"))?,
        );
        let low = unit::<T>();
        // Initialization can mutate balances before the timestamp inherent:
        // include creation of the preserved previous-day stake/breach source.
        frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number().saturating_add(1u32.into()),
        );
        #[block]
        {
            let _ = <T as Config>::Currency::make_free_balance_be(&who, low);
        }
        assert_eq!(<T as Config>::Currency::free_balance(&who), low);
        assert!(Pallet::<T>::is_vipp_quarantined(&who));
        Ok(())
    }

    /// Successful append rewrites the full queues, unlike their overflow path.
    #[benchmark]
    fn observe_queued_balance_change() -> Result<(), BenchmarkError> {
        #[cfg(test)]
        crate::mock::ObserveVippBalances::set(true);
        let who = prepare_member::<T>(12, 64)?;
        T::OnVIPPChanged::benchmark_fill_breach_queue(1023);
        let pending: Vec<T::AccountId> = (0..T::MaxPendingVippAccounts::get().saturating_sub(1))
            .map(|i| account("pending-vipp-append", i, 0))
            .collect();
        PendingVippBalanceChanges::<T>::put(
            BoundedVec::<_, T::MaxPendingVippAccounts>::try_from(pending)
                .map_err(|_| BenchmarkError::Stop("pending queue bound"))?,
        );
        frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number().saturating_add(1u32.into()),
        );
        let low = unit::<T>();
        #[block]
        {
            let _ = <T as Config>::Currency::make_free_balance_be(&who, low);
        }
        assert!(Pallet::<T>::is_vipp_quarantined(&who));
        assert_eq!(
            PendingVippBalanceChanges::<T>::get().len(),
            T::MaxPendingVippAccounts::get() as usize
        );
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
