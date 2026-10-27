use super::*;
use frame_support::traits::{tokens::nonfungibles_v2::Mutate, OnRuntimeUpgrade, StorageVersion};

type Repair = crate::migration::RepairVippCache<Test>;

fn mint_item(account: AccountId, id: u32, claim: Option<Vec<u8>>) {
    assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::mint_into(
        &1,
        &id,
        &account,
        &pallet_nfts::ItemConfig { settings: pallet_nfts::ItemSettings::all_enabled() },
        true,
    ));
    if let Some(claim) = claim {
        assert_ok!(<Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &1,
            &id,
            &[0, 0, 2],
            &claim,
        ));
    }
}

fn set_cache(account: AccountId, points: u64, caps: Vec<(u32, u64)>) {
    VippMembers::<Test>::insert(
        account,
        VippMemberInfo::<Test> { points, active_vipp_threshold: caps },
    );
    StorageVersion::new(1).put::<Privileges>();
}

#[test]
fn cache_repair_rebuilds_owned_caps_and_preserves_empty_members() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(1000);
        mint_item(10, 777, Some(100u64.encode()));
        mint_item(10, 778, Some(0u64.encode()));
        mint_item(10, 779, Some(1u64.encode()));
        mint_item(10, 780, Some(199u64.encode()));
        mint_item(100, 781, Some(500u64.encode()));
        set_cache(10, 123, vec![(0, 950), (777, 1), (777, 1), (778, 0), (999, 77)]);
        set_cache(20, 321, vec![(999, 77)]);

        Repair::on_runtime_upgrade();

        let member = Privileges::vipp_members(10).unwrap();
        let mut caps = member.active_vipp_threshold;
        caps.sort();
        // Match the pallet's existing Perbill rounding, including single base units.
        assert_eq!(caps, vec![(777, 95), (779, 1), (780, 189)]);
        assert_eq!(member.points, 123);
        let empty = Privileges::vipp_members(20).unwrap();
        assert!(empty.active_vipp_threshold.is_empty());
        assert_eq!(empty.points, 321);
        assert!(Privileges::vipp_members(100).is_none());
        assert_eq!(NacManaging::owned_vipp_claims(&10).unwrap().len(), 4);
        assert_eq!(NacManaging::get_claim_balance(&10).unwrap().0, 1000);
    });
}

#[test]
fn cache_repair_rejects_malformed_members_without_partial_writes() {
    ExtBuilder::default().build_and_execute(|| {
        set_cache(10, 123, vec![(0, 950)]);
        let key = VippMembers::<Test>::hashed_key_for(20);
        sp_io::storage::set(&key, &[1]);
        let before = Privileges::vipp_members(10).unwrap();

        Repair::on_runtime_upgrade();

        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(1));
        assert_eq!(Privileges::vipp_members(10).unwrap(), before);
        assert_eq!(sp_io::storage::get(&key).unwrap().as_ref(), &[1]);
        assert!(matches!(
            System::events().last().unwrap().event,
            RuntimeEvent::Privileges(Event::VippCacheMigrationFailed { account: Some(20), .. })
        ));
    });
}

#[test]
fn cache_repair_is_idempotent_after_new_mints() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let (_, nac) = NacManaging::get_nac_level(&10).unwrap();
        set_cache(10, 123, vec![(nac, 950)]);
        Repair::on_runtime_upgrade();
        System::set_block_number(2);
        claim_for_validator(100);
        let before = Privileges::vipp_members(10).unwrap();
        assert_eq!(
            Repair::on_runtime_upgrade(),
            <Test as frame_system::Config>::DbWeight::get().reads(1)
        );
        assert_eq!(Privileges::vipp_members(10).unwrap(), before);
    });
}

#[test]
fn cache_repair_rejects_invalid_attributes_without_partial_writes() {
    let mut trailing = 100u64.encode();
    trailing.push(0);
    for attribute in [None, Some(vec![1]), Some(trailing)] {
        ExtBuilder::default().build_and_execute(|| {
            claim_for_validator(1000);
            mint_item(10, 777, attribute);
            set_cache(10, 123, vec![(0, 950)]);
            set_cache(20, 321, vec![(999, 77)]);
            let before: Vec<_> = VippMembers::<Test>::iter().collect();

            Repair::on_runtime_upgrade();

            assert_eq!(VippMembers::<Test>::iter().collect::<Vec<_>>(), before);
            assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(1));
            System::assert_last_event(
                Event::<Test>::VippCacheMigrationFailed {
                    account: Some(10),
                    error: pallet_nac_managing::Error::<Test>::InvalidClaimAttribute.into(),
                }
                .into(),
            );
        });
    }
}

#[test]
fn cache_repair_rejects_unbacked_owned_claims() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(100);
        mint_item(10, 777, Some(101u64.encode()));
        set_cache(10, 123, vec![(0, 95)]);
        let before = Privileges::vipp_members(10).unwrap();

        Repair::on_runtime_upgrade();

        assert_eq!(Privileges::vipp_members(10).unwrap(), before);
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(1));
        System::assert_last_event(
            Event::<Test>::VippCacheMigrationFailed {
                account: Some(10),
                error: Error::<Test>::InvalidVippBacking.into(),
            }
            .into(),
        );
    });
}

#[test]
fn cache_repair_rejects_excess_ownership_without_partial_caps() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(1000);
        for id in 0..65 {
            mint_item(10, id, Some(1u64.encode()));
        }
        set_cache(10, 123, vec![(0, 950)]);
        let before = Privileges::vipp_members(10).unwrap();
        Repair::on_runtime_upgrade();
        assert_eq!(Privileges::vipp_members(10).unwrap(), before);
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(1));
        System::assert_last_event(
            Event::<Test>::VippCacheMigrationFailed {
                account: Some(10),
                error: pallet_nac_managing::Error::<Test>::TooManyVippNfts.into(),
            }
            .into(),
        );
    });
}

#[test]
fn cache_repair_rejects_excess_members_without_partial_writes() {
    ExtBuilder::default().build_and_execute(|| {
        for account in 10_000..11_025 {
            set_cache(account, 123, vec![(0, 95)]);
        }
        Repair::on_runtime_upgrade();
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(1));
        assert!(VippMembers::<Test>::iter_values()
            .all(|m| m.points == 123 && m.active_vipp_threshold == vec![(0, 95)]));
        assert!(matches!(
            System::events().last().unwrap().event,
            RuntimeEvent::Privileges(Event::VippCacheMigrationFailed { .. })
        ));
    });
}

#[test]
fn cache_repair_keeps_mint_and_burn_accounting_consistent() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let old = owned_vipp_ids(10)[0];
        let (_, nac) = NacManaging::get_nac_level(&10).unwrap();
        set_cache(10, 123, vec![(nac, 950)]);
        Repair::on_runtime_upgrade();
        System::set_block_number(2);
        claim_for_validator(100);
        Privileges::burn_vipp_nft(&10, old);
        let member = Privileges::vipp_members(10).unwrap();
        assert_eq!(member.points, 123);
        assert_eq!(member.active_vipp_threshold.len(), 1);
        assert_eq!(member.active_vipp_threshold[0].1, 95);
        assert_ok!(Privileges::exit_vip(RuntimeOrigin::signed(10)));
        assert!(Privileges::vipp_members(10).is_none());
        assert!(owned_vipp_ids(10).is_empty());
    });
}

#[cfg(feature = "try-runtime")]
#[test]
fn cache_repair_checks_preserved_points_and_caps() {
    ExtBuilder::default().build_and_execute(|| {
        join_vip_and_vipp(10);
        let (_, nac) = NacManaging::get_nac_level(&10).unwrap();
        set_cache(10, 123, vec![(nac, 950)]);
        let before = Repair::pre_upgrade().unwrap();
        Repair::on_runtime_upgrade();
        assert_ok!(Repair::post_upgrade(before.clone()));
        VippMembers::<Test>::mutate(10, |m| m.as_mut().unwrap().points += 1);
        assert!(Repair::post_upgrade(before.clone()).is_err());
        VippMembers::<Test>::mutate(10, |m| {
            let m = m.as_mut().unwrap();
            m.points = 123;
            m.active_vipp_threshold.clear();
        });
        assert!(Repair::post_upgrade(before).is_err());
    });
}

#[cfg(feature = "try-runtime")]
#[test]
fn cache_repair_precheck_rejects_unbacked_nfts() {
    ExtBuilder::default().build_and_execute(|| {
        claim_for_validator(100);
        mint_item(10, 777, Some(101u64.encode()));
        set_cache(10, 123, vec![(0, 95)]);
        assert!(Repair::pre_upgrade().is_err());
    });
}
