use crate::{
    mock::*, Error, Event, Reputation, ReputationRecord, ReputationTier,
    REPUTATION_POINTS_PER_BLOCK,
};
use frame_support::{assert_noop, assert_ok};

#[test]
fn can_set_points_forcefuly() {
    new_test_ext().execute_with(|| {
        let block_number = 27;
        System::set_block_number(block_number);
        ReputationPallet::update_points_for_time();
        let points = 42.into();
        let account = user();
        assert_ok!(ReputationPallet::force_set_points(RuntimeOrigin::root(), account, points));
        assert_eq!(
            ReputationPallet::reputation(account),
            Some(ReputationRecord { points, updated: block_number })
        );
        System::assert_last_event(Event::ReputationSetForcibly { account, points }.into());
    });
}

#[test]
fn can_encrease_points() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        ReputationPallet::update_points_for_time();
        let points = 42.into();
        let account = user();

        // until account is updated it's not in the store
        assert_noop!(
            ReputationPallet::increase_points(RuntimeOrigin::root(), account, points),
            Error::<Test>::AccountNotFound
        );

        // update the account points to insert it into the store
        assert_ok!(ReputationPallet::update_points(RuntimeOrigin::signed(account), account));

        // wait for some blocks to get points
        let block_number = 27;
        System::set_block_number(block_number);

        ReputationPallet::update_points_for_time();

        assert_ok!(ReputationPallet::increase_points(RuntimeOrigin::root(), account, points));

        let new_points = *points + (block_number - 1) * *REPUTATION_POINTS_PER_BLOCK;
        assert_eq!(
            ReputationPallet::reputation(account),
            Some(ReputationRecord { points: new_points.into(), updated: block_number })
        );
        System::assert_last_event(Event::ReputationIncreased { account, points }.into());
    });
}

#[test]
fn can_slash() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        ReputationPallet::update_points_for_time();
        let points = 42.into();
        let account = user();

        // until account is updated it's not in the store
        assert_noop!(
            ReputationPallet::slash(RuntimeOrigin::root(), account, points),
            Error::<Test>::AccountNotFound
        );

        // update the account points to insert it into the store
        assert_ok!(ReputationPallet::update_points(RuntimeOrigin::signed(account), account));

        // wait for some blocks to get points
        let block_number = 27;
        System::set_block_number(block_number);

        ReputationPallet::update_points_for_time();

        assert_ok!(ReputationPallet::slash(RuntimeOrigin::root(), account, points));

        let new_points = (block_number - 1) * *REPUTATION_POINTS_PER_BLOCK - *points;
        assert_eq!(
            ReputationPallet::reputation(account),
            Some(ReputationRecord { points: new_points.into(), updated: block_number })
        );
        System::assert_last_event(Event::ReputationSlashed { account, points }.into());
    });
}

#[test]
fn can_update_points_for_account() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        ReputationPallet::update_points_for_time();
        let account = user();

        assert_eq!(ReputationPallet::reputation(account), None);

        // update the account points to insert it into the store
        assert_ok!(ReputationPallet::update_points(RuntimeOrigin::signed(account), account));

        // wait for some blocks to get points
        let block_number = 27;
        System::set_block_number(block_number);

        assert_ok!(ReputationPallet::update_points(RuntimeOrigin::signed(account), account));

        let new_points = (block_number - 1) * *REPUTATION_POINTS_PER_BLOCK;
        assert_eq!(
            ReputationPallet::reputation(account),
            Some(ReputationRecord { points: new_points.into(), updated: block_number })
        );
        System::assert_last_event(
            Event::ReputationUpdated { account, points: new_points.into() }.into(),
        );
    });
}

#[test]
fn tier_correct() {
    let mut reputation = Reputation::default();

    reputation.update(1999.into());
    assert_eq!(reputation.rank, None);
    reputation.update(2000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::VanguardZero));
    reputation.update(4000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::VanguardOne));

    // falling below 2000 should remove the tier
    reputation.update(1999.into());
    assert_eq!(reputation.rank, None);

    reputation.update(60_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::VanguardTwo));

    reputation.update(125_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::VanguardThree));

    reputation.update(250_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::TrailblazerOne));

    // still trailblazer
    reputation.update(125_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::TrailblazerZero));

    reputation.update(60_001.into());
    assert_eq!(reputation.rank, Some(ReputationTier::TrailblazerZero));

    // not anymore
    reputation.update(60_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::VanguardTwo));

    // --
    reputation.update(1_000_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::TrailblazerThree));

    reputation.update(630_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::TrailblazerTwo));

    reputation.update(2_000_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::UltramodernOne));

    reputation.update(2_000_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::UltramodernOne));

    // still ultramodern
    reputation.update(1_000_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::UltramodernZero));

    reputation.update(630_001.into());
    assert_eq!(reputation.rank, Some(ReputationTier::UltramodernZero));

    // not anymore
    reputation.update(630_000.into());
    assert_eq!(reputation.rank, Some(ReputationTier::TrailblazerTwo));

    // --
    reputation.update(u64::MAX.into());
    assert_eq!(reputation.rank, Some(ReputationTier::UltramodernThree));
}

fn user() -> u64 {
    frame_benchmarking::account("test", 1, 1)
}
