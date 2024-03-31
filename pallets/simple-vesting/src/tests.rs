use crate::{mock::*, VestingInfo};
use frame_support::assert_ok;
use frame_support::traits::Currency;

#[test]
fn basic_setup_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            SimpleVesting::vesting(alice()),
            Some(VestingInfo { locked: 10, per_block: 1, starting_block: 5 })
        );
        assert_eq!(Balances::total_balance(&alice()), 12);
        assert_eq!(Balances::free_balance(&alice()), 2);
        assert_eq!(Balances::reserved_balance(&alice()), 10);

        assert_eq!(
            SimpleVesting::vesting(bob()),
            Some(VestingInfo { locked: 20, per_block: 4, starting_block: 2 })
        );
        assert_eq!(Balances::total_balance(&bob()), 21);
        assert_eq!(Balances::free_balance(&bob()), 1);
        assert_eq!(Balances::reserved_balance(&bob()), 20);

        assert_ok!(SimpleVesting::do_vest(alice(), 3));
        assert_eq!(Balances::total_balance(&alice()), 12);
        assert_eq!(Balances::free_balance(&alice()), 5);
        assert_eq!(Balances::reserved_balance(&alice()), 7);

        assert_ok!(SimpleVesting::do_vest(bob(), 21));
        assert_eq!(Balances::total_balance(&bob()), 21);
        assert_eq!(Balances::free_balance(&bob()), 21);
        assert_eq!(Balances::reserved_balance(&bob()), 0);
    });
}
