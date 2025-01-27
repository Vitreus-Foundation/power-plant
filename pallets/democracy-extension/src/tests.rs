use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

#[test]
fn set_electorate_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(DemocracyExtension::total_issuance(), 5000);

        assert_ok!(DemocracyExtension::set_electorate(RuntimeOrigin::root(), 3000));
        assert_eq!(DemocracyExtension::total_issuance(), 3000);
    });
}

#[test]
fn set_zero_electorate_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(DemocracyExtension::set_electorate(RuntimeOrigin::root(), 3000));
        assert_eq!(DemocracyExtension::total_issuance(), 3000);

        assert_ok!(DemocracyExtension::set_electorate(RuntimeOrigin::root(), 0));
        assert_eq!(DemocracyExtension::total_issuance(), Balances::total_issuance());
    });
}

#[test]
fn set_electorate_non_root_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DemocracyExtension::set_electorate(RuntimeOrigin::signed(1), 42),
            DispatchError::BadOrigin
        );
    });
}
