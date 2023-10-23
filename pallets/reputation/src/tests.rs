use crate::{
    mock::*, Error, Event, Reputation, ReputationPoint, ReputationRecord, ReputationTier,
    RANKS_PER_TIER, REPUTATION_POINTS_PER_BLOCK,
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
    use ReputationTier::*;

    let mut reputation = Reputation::default();

    reputation.update(0.into());
    assert_eq!(reputation.tier, None);

    for n in 1..=RANKS_PER_TIER {
        // Vanguard
        reputation.update(ReputationPoint::from_rank(n));
        assert_eq!(reputation.tier, Some(Vanguard(n)));
        reputation.update((*ReputationPoint::from_rank(n) - 1).into());
        if n == 1 {
            assert_eq!(reputation.tier, None);
        } else {
            assert_eq!(reputation.tier, Some(Vanguard(n - 1)));
        }
        reputation.update(0.into());

        // Trailblazer
        reputation.update(ReputationPoint::from_rank(RANKS_PER_TIER + n));
        assert_eq!(reputation.tier, Some(Trailblazer(n)));
        reputation.update((*ReputationPoint::from_rank(RANKS_PER_TIER + n) - 1).into());
        if n == 1 {
            assert_eq!(reputation.tier, Some(Vanguard(RANKS_PER_TIER)));
        } else {
            assert_eq!(reputation.tier, Some(Trailblazer(n - 1)));
        }
        reputation.update(0.into());

        // Ultramodern
        reputation.update(ReputationPoint::from_rank(RANKS_PER_TIER * 2 + n));
        assert_eq!(reputation.tier, Some(Ultramodern(n)));
        reputation.update((*ReputationPoint::from_rank(RANKS_PER_TIER * 2 + n) - 1).into());
        if n == 1 {
            assert_eq!(reputation.tier, Some(Trailblazer(RANKS_PER_TIER)));
        } else {
            assert_eq!(reputation.tier, Some(Ultramodern(n - 1)));
        }
        reputation.update(0.into());
    }

    reputation.update(u64::MAX.into());
    assert_eq!(reputation.tier, Some(Ultramodern(u8::MAX - RANKS_PER_TIER * 2)));
}

#[test]
fn zero_tiers_work() {
    todo!();
}

fn user() -> u64 {
    frame_benchmarking::account("test", 1, 1)
}
