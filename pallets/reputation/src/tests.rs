use crate::{mock::*, Error, Event, ReputationRecord, REPUTATION_POINTS_PER_BLOCK};
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

fn user() -> u64 {
    frame_benchmarking::account("test", 1, 1)
}
