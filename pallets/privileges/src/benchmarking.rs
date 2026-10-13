//! Worst-case integrated accounting benchmarks, including maximum cache sizes.
use super::*;
use frame_benchmarking::v2::*;
use frame_support::{traits::Get, BoundedBTreeMap, BoundedBTreeSet};
use frame_system::RawOrigin;
use pallet_energy_generation::{Collaborations, Cooperations, Cooperators};

pub(crate) fn populate<T: Config>(members: u32) -> Vec<T::AccountId> {
    let mut accounts = Vec::new();
    for i in 0..members {
        let who = account("accounting-member", i, 0);
        VipMembers::<T>::insert(
            &who,
            VipMemberInfo::<T> {
                start: 0,
                tax_type: PenaltyType::Flat,
                points: 10u32.into(),
                active_stake: 1_000_000u32.into(),
            },
        );
        VippMembers::<T>::insert(
            &who,
            VippMemberInfo::<T> {
                points: 10u32.into(),
                active_vipp_threshold: (0..64).map(|i| (i.into(), 10_000u32.into())).collect(),
            },
        );
        accounts.push(who);
    }
    accounts
}

fn maximum_cooperations<T: Config>(
    who: &T::AccountId,
) -> Result<T::StakeBalance, sp_runtime::DispatchError> {
    let mut followers = sp_std::collections::btree_set::BTreeSet::new();
    followers.insert(who.clone());
    followers.extend((0..1023).map(|i| account("max-coop-follower", i, 0)));
    let followers = BoundedBTreeSet::try_from(followers)
        .map_err(|_| sp_runtime::DispatchError::Other("collaborations bound"))?;
    let value: T::StakeBalance = pallet_nac_managing::benchmarking::unit::<T>().into();
    let mut targets = BoundedBTreeMap::<T::AccountId, T::StakeBalance, T::MaxCooperations>::new();
    for i in 0..T::MaxCooperations::get() {
        let validator = account("max-coop-validator", i, 0);
        Collaborations::<T>::insert(&validator, &followers);
        targets
            .try_insert(validator, value)
            .map_err(|_| sp_runtime::DispatchError::Other("cooperations bound"))?;
    }
    Cooperators::<T>::insert(
        who,
        Cooperations::<T> { targets, submitted_in: 0, suppressed: false },
    );
    Ok(value.saturating_mul(T::MaxCooperations::get().into()))
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn become_vip_status() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(200, 64)?;
        populate::<T>(1023);
        VipMembers::<T>::remove(&who);
        VippMembers::<T>::remove(&who);
        let mut followers = sp_std::collections::btree_set::BTreeSet::new();
        followers.insert(who.clone());
        followers.extend((0..1023).map(|i| account("join-follower", i, 0)));
        let followers = BoundedBTreeSet::try_from(followers)
            .map_err(|_| BenchmarkError::Stop("collaborations bound"))?;
        let mut targets =
            BoundedBTreeMap::<T::AccountId, T::StakeBalance, T::MaxCooperations>::new();
        for i in 0..T::MaxCooperations::get() {
            let validator = account("join-validator", i, 0);
            Collaborations::<T>::insert(&validator, &followers);
            targets
                .try_insert(validator, 1_000u32.into())
                .map_err(|_| BenchmarkError::Stop("cooperations bound"))?;
        }
        Cooperators::<T>::insert(
            &who,
            Cooperations::<T> { targets, submitted_in: 0, suppressed: false },
        );
        #[extrinsic_call]
        _(RawOrigin::Signed(who.clone()), PenaltyType::Flat);
        assert!(VipMembers::<T>::contains_key(&who));
        assert_eq!(VippMembers::<T>::get(&who).unwrap().active_vipp_threshold.len(), 64);
        Ok(())
    }

    /// Initial membership mints the first NFT; this has a different write set
    /// from rebuilding an existing maximum-size ownership cache.
    #[benchmark]
    fn become_vip_new_member() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(209, 0)?;
        populate::<T>(1023);
        VipMembers::<T>::remove(&who);
        VippMembers::<T>::remove(&who);
        pallet_nac_managing::benchmarking::set_claim::<T>(
            &who,
            pallet_nac_managing::benchmarking::unit::<T>(),
        )?;
        maximum_cooperations::<T>(&who)?;
        #[extrinsic_call]
        become_vip_status(RawOrigin::Signed(who.clone()), PenaltyType::Flat);
        assert!(VipMembers::<T>::contains_key(&who));
        assert_eq!(VippMembers::<T>::get(&who).unwrap().active_vipp_threshold.len(), 1);
        Ok(())
    }

    /// A penalized validator is chilled and all 1024 delegation caches are touched.
    /// 1023 of the followers are VIPs, each with a maximum-size target map.
    #[benchmark]
    fn exit_vip() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(201, 64)?;
        let mut followers = populate::<T>(1023);
        followers.push(account("non-vip-follower", 0, 0));
        let stake: T::StakeBalance = pallet_nac_managing::benchmarking::unit::<T>().into();
        let mut targets =
            BoundedBTreeMap::<T::AccountId, T::StakeBalance, T::MaxCooperations>::new();
        targets
            .try_insert(who.clone(), stake)
            .map_err(|_| BenchmarkError::Stop("cooperations bound"))?;
        for i in 1..T::MaxCooperations::get() {
            targets
                .try_insert(account("exit-other-target", i, 0), stake)
                .map_err(|_| BenchmarkError::Stop("cooperations bound"))?;
        }
        for follower in &followers {
            Cooperators::<T>::insert(
                follower,
                Cooperations::<T> { targets: targets.clone(), submitted_in: 0, suppressed: false },
            );
        }
        let followers = BoundedBTreeSet::try_from(
            followers.into_iter().collect::<sp_std::collections::btree_set::BTreeSet<_>>(),
        )
        .map_err(|_| BenchmarkError::Stop("collaborations bound"))?;
        Collaborations::<T>::insert(&who, followers);
        pallet_energy_generation::Validators::<T>::insert(
            &who,
            pallet_energy_generation::ValidatorPrefs::default(),
        );
        pallet_energy_generation::Bonded::<T>::insert(&who, &who);
        let mut ledger = pallet_energy_generation::StakingLedger::<T>::default_from(who.clone());
        ledger.active = stake;
        ledger.total = stake;
        pallet_energy_generation::Ledger::<T>::insert(&who, ledger);
        pallet_energy_generation::CurrentEra::<T>::put(1);
        pallet_energy_generation::MinTrustValidatorBond::<T>::put(stake);
        pallet_energy_generation::MinCommonValidatorBond::<T>::put(stake);
        // Hold the accounted date in a penalized month; no time setter is needed
        // because the measured call uses the pallet's already-settled date.
        CurrentDate::<T>::put(CurrentDateInfo::new::<T>(1970, 2, 1)?);
        #[extrinsic_call]
        _(RawOrigin::Signed(who.clone()));
        assert!(!VipMembers::<T>::contains_key(&who));
        assert!(!pallet_energy_generation::Validators::<T>::contains_key(&who));
        assert!(pallet_nac_managing::Pallet::<T>::owned_vipp_claims(&who)?.is_empty());
        Ok(())
    }

    #[benchmark]
    fn exit_vip_cooperator() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(207, 64)?;
        let stake = maximum_cooperations::<T>(&who)?;
        pallet_energy_generation::Bonded::<T>::insert(&who, &who);
        let mut ledger = pallet_energy_generation::StakingLedger::<T>::default_from(who.clone());
        ledger.active = stake;
        ledger.total = stake;
        pallet_energy_generation::Ledger::<T>::insert(&who, ledger);
        pallet_energy_generation::CurrentEra::<T>::put(1);
        pallet_energy_generation::MinCooperatorBond::<T>::put(T::StakeBalance::from(1u32));
        CurrentDate::<T>::put(CurrentDateInfo::new::<T>(1970, 2, 1)?);
        #[extrinsic_call]
        exit_vip(RawOrigin::Signed(who.clone()));
        assert!(!VipMembers::<T>::contains_key(&who));
        assert!(pallet_nac_managing::Pallet::<T>::owned_vipp_claims(&who)?.is_empty());
        Ok(())
    }

    #[benchmark]
    fn refresh_active_stake() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(208, 64)?;
        let stake = maximum_cooperations::<T>(&who)?;
        #[block]
        {
            <Pallet<T> as OnVipMembershipHandler<T::AccountId, Weight, Perbill>>::update_active_stake(&who);
        }
        assert_eq!(VipMembers::<T>::get(who).unwrap().active_stake, stake);
        Ok(())
    }

    #[benchmark]
    fn change_penalty_type() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(202, 64)?;
        #[extrinsic_call]
        _(RawOrigin::Signed(who.clone()), PenaltyType::Declining);
        assert_eq!(VipMembers::<T>::get(who).unwrap().tax_type, PenaltyType::Declining);
        Ok(())
    }

    #[benchmark]
    fn force_set_vip_points() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(203, 64)?;
        let year = CurrentDate::<T>::get().current_year as u32;
        #[extrinsic_call]
        _(RawOrigin::Root, year, who.clone(), 500u32.into());
        assert_eq!(VipMembers::<T>::get(who).unwrap().points, 500u32.into());
        Ok(())
    }

    #[benchmark]
    fn force_set_vipp_points() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(204, 64)?;
        let year = CurrentDate::<T>::get().current_year as u32;
        #[extrinsic_call]
        _(RawOrigin::Root, year, who.clone(), 500u32.into());
        assert_eq!(VippMembers::<T>::get(who).unwrap().points, 500u32.into());
        Ok(())
    }

    #[benchmark]
    fn set_rewards() -> Result<(), BenchmarkError> {
        let now = crate::accounting::today::<T>()?;
        let year = now.current_year as u32 - 1;
        CurrentDate::<T>::put(now);
        for i in 0..1024 {
            let who: T::AccountId = account("year-points-member", i, 0);
            VipPoints::<T>::insert(year, &who, T::StakeBalance::from(1_000u32));
            VippPoints::<T>::insert(year, who, T::StakeBalance::from(1_000u32));
        }
        #[extrinsic_call]
        _(RawOrigin::Root, year, 1_000u64.into(), 1_000u64.into());
        assert!(Rewards::<T>::contains_key(year));
        Ok(())
    }

    #[benchmark]
    fn claim_rewards() -> Result<(), BenchmarkError> {
        let who = pallet_nac_managing::benchmarking::prepare_member::<T>(205, 64)?;
        let pot = pallet_nac_managing::benchmarking::prepare_member_for::<T>(
            Pallet::<T>::account_id(),
            206,
            64,
        )?;
        // The source pot also has entitlements: paying 98% creates a new breach.
        let amount = <T as Config>::Currency::total_balance(&pot).saturating_mul(49u64.into())
            / 100u64.into();
        let pending: Vec<T::AccountId> = (0..T::MaxPendingVippAccounts::get())
            .map(|i| account("reward-pending", i, 0))
            .collect();
        pallet_nac_managing::PendingVippBalanceChanges::<T>::put(
            frame_support::BoundedVec::<_, T::MaxPendingVippAccounts>::try_from(pending)
                .map_err(|_| BenchmarkError::Stop("pending queue bound"))?,
        );
        VipPoints::<T>::insert(1969, &who, T::StakeBalance::from(100u32));
        VippPoints::<T>::insert(1969, &who, T::StakeBalance::from(100u32));
        Rewards::<T>::insert(
            1969,
            RewardsInfo::<T> {
                vip_points: 100u32.into(),
                vipp_points: 100u32.into(),
                vip_rewards: amount,
                vipp_rewards: amount,
            },
        );
        #[extrinsic_call]
        _(RawOrigin::Signed(who.clone()), 1969);
        assert!(!VipPoints::<T>::contains_key(1969, &who));
        assert!(!VippPoints::<T>::contains_key(1969, who));
        Ok(())
    }

    /// Up to 1024 VIP plus 1024 VIPP records; seven days including annual rollover.
    #[benchmark]
    fn accounting(m: Linear<1, 1024>, d: Linear<1, 7>) -> Result<(), BenchmarkError> {
        let accounts = populate::<T>(m);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        TimestampSeenAtBlock::<T>::put(frame_system::pallet_prelude::BlockNumberFor::<T>::from(
            0u32,
        ));
        let start = CurrentDateInfo::new::<T>(2024, 12, 31)?;
        CurrentDate::<T>::put(&start);
        let mut target = start;
        target.add_days::<T>(d.into())?;
        #[block]
        {
            crate::accounting::advance::<T>(&target, None)?;
        }
        assert_eq!(CurrentDate::<T>::get(), target);
        assert!(VipPoints::<T>::contains_key(2024, &accounts[0]));
        assert!(VippPoints::<T>::contains_key(2024, &accounts[0]));
        Ok(())
    }

    /// Maximum-size preserved source rows can have larger proof than missing rows.
    #[benchmark]
    fn accounting_snapshots() -> Result<(), BenchmarkError> {
        let accounts = populate::<T>(1024);
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        TimestampSeenAtBlock::<T>::put(frame_system::pallet_prelude::BlockNumberFor::<T>::from(
            0u32,
        ));
        for who in &accounts {
            PreTimestampStake::<T>::insert(
                who,
                (
                    frame_system::pallet_prelude::BlockNumberFor::<T>::from(1u32),
                    T::StakeBalance::from(1_000_000u32),
                    false,
                ),
            );
        }
        CurrentDate::<T>::put(CurrentDateInfo::new::<T>(2024, 12, 31)?);
        let target = CurrentDateInfo::new::<T>(2025, 1, 7)?;
        #[block]
        {
            crate::accounting::advance::<T>(&target, None)?;
        }
        assert_eq!(CurrentDate::<T>::get(), target);
        Ok(())
    }

    #[benchmark]
    fn resolve_timestamp_breaches() -> Result<(), BenchmarkError> {
        let accounts: Vec<T::AccountId> =
            (0..1024).map(|i| account("pending-breach", i, 0)).collect();
        PreTimestampVippBreaches::<T>::put(
            frame_support::BoundedVec::<_, frame_support::traits::ConstU32<1024>>::try_from(
                accounts.clone(),
            )
            .unwrap(),
        );
        #[block]
        {
            crate::breach_period::resolve_timestamp::<T>();
        }
        assert!(PreTimestampVippBreaches::<T>::get().is_empty());
        assert!(VippBreachYear::<T>::contains_key(&accounts[0]));
        Ok(())
    }

    /// A full frozen job is decoded/re-encoded even though only 64 VIP sources run.
    #[benchmark]
    fn catch_up_vip() -> Result<(), BenchmarkError> {
        let accounts = populate::<T>(1024);
        let start = CurrentDateInfo::new::<T>(2024, 12, 31)?;
        CurrentDate::<T>::put(&start);
        let mut target = start.clone();
        target.add_days::<T>(366)?;
        let sources: Vec<_> =
            accounts.iter().cloned().map(|who| (who, 1_000_000u32.into())).collect();
        ApprovedCatchUp::<T>::put(crate::accounting::CatchUp::<T> {
            target,
            source_cursor: 0,
            vip: sources.clone().try_into().unwrap(),
            vipp: sources.try_into().unwrap(),
        });
        #[block]
        {
            crate::accounting::step::<T>()?;
        }
        assert_eq!(ApprovedCatchUp::<T>::get().unwrap().source_cursor, 64);
        assert_eq!(CurrentDate::<T>::get(), start);
        Ok(())
    }

    /// Same maximum source blob, measuring the VIPP branch and annual write paths.
    #[benchmark]
    fn catch_up_vipp() -> Result<(), BenchmarkError> {
        let accounts = populate::<T>(1024);
        let start = CurrentDateInfo::new::<T>(2024, 12, 31)?;
        CurrentDate::<T>::put(&start);
        let mut target = start.clone();
        target.add_days::<T>(366)?;
        let sources: Vec<_> =
            accounts.iter().cloned().map(|who| (who, 1_000_000u32.into())).collect();
        ApprovedCatchUp::<T>::put(crate::accounting::CatchUp::<T> {
            target,
            source_cursor: 1024,
            vip: sources.clone().try_into().unwrap(),
            vipp: sources.try_into().unwrap(),
        });
        #[block]
        {
            crate::accounting::step::<T>()?;
        }
        assert_eq!(ApprovedCatchUp::<T>::get().unwrap().source_cursor, 1088);
        assert_eq!(CurrentDate::<T>::get(), start);
        Ok(())
    }

    /// Each account owns 64 NFTs and delegates to every allowed target. Each
    /// collaboration record contains 1024 accounts, exercising maximum decoding.
    #[benchmark]
    fn recovery(a: Linear<1, 16>) -> Result<(), BenchmarkError> {
        let mut accounts = Vec::new();
        for i in 0..a {
            let who = pallet_nac_managing::benchmarking::prepare_member::<T>(100 + i, 64)?;
            // Historical zero-cap corruption must actually be repaired in the
            // measured step; an already-correct cache would omit this write.
            VippMembers::<T>::mutate(&who, |member| {
                for (_, value) in &mut member.as_mut().unwrap().active_vipp_threshold {
                    *value = Zero::zero();
                }
            });
            accounts.push(who);
        }
        let mut followers = accounts.clone();
        followers.extend((a..1024).map(|i| account("recovery-follower", i, 0)));
        let followers = BoundedBTreeSet::try_from(
            followers.into_iter().collect::<sp_std::collections::btree_set::BTreeSet<_>>(),
        )
        .map_err(|_| BenchmarkError::Stop("collaborations bound"))?;
        for (owner_index, who) in accounts.iter().enumerate() {
            // Distinct keys per owner are essential: sharing the target list
            // would undercount both cold reads and storage proof by up to 16x.
            let mut targets =
                BoundedBTreeMap::<T::AccountId, T::StakeBalance, T::MaxCooperations>::new();
            for i in 0..T::MaxCooperations::get() {
                let validator = account("recovery-validator", i, owner_index as u32);
                Collaborations::<T>::insert(&validator, &followers);
                targets
                    .try_insert(validator, 1_000u32.into())
                    .map_err(|_| BenchmarkError::Stop("cooperations bound"))?;
            }
            Cooperators::<T>::insert(
                who,
                Cooperations::<T> { targets, submitted_in: 0, suppressed: false },
            );
        }
        migration::recovery::begin::<T>();
        #[block]
        {
            migration::recovery::step::<T>(a, 64)?;
        }
        assert_eq!(CacheRecoveryProgress::<T>::get().unwrap().repaired_accounts, a);
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, sp_io::TestExternalities::default(), crate::mock::Test);
}
