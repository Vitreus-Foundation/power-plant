use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};

#[test]
fn can_receive_funds() {
    new_test_ext().execute_and_prove(|| {
        let balance = 100;
        assert_ok!(Faucet::request_funds(RuntimeOrigin::none(), 1, balance));
        assert_eq!(Balances::free_balance(1), balance);
    });
}

#[test]
fn cannot_request_more_than_max_amount() {
    new_test_ext().execute_and_prove(|| {
        let balance = 101;
        assert_noop!(
            Faucet::request_funds(RuntimeOrigin::none(), 1, balance),
            Error::<Test>::AmountTooHigh
        );
    });
}

#[test]
fn cannot_exceed_max_amount_during_period() {
    new_test_ext().execute_and_prove(|| {
        assert_ok!(Faucet::request_funds(RuntimeOrigin::none(), 1, 10));
        assert_eq!(Balances::free_balance(1), 10);

        System::set_block_number(BLOCKS_PER_HOUR * 7);

        assert_ok!(Faucet::request_funds(RuntimeOrigin::none(), 1, 20));
        assert_eq!(Balances::free_balance(1), 30);

        System::set_block_number(BLOCKS_PER_HOUR * 20);

        assert_ok!(Faucet::request_funds(RuntimeOrigin::none(), 1, 50));
        assert_eq!(Balances::free_balance(1), 80);

        System::set_block_number(BLOCKS_PER_HOUR * 23);

        assert_noop!(
            Faucet::request_funds(RuntimeOrigin::none(), 1, 21),
            Error::<Test>::RequestLimitExceeded
        );

        System::set_block_number(BLOCKS_PER_HOUR * 24 - 1);

        assert_noop!(
            Faucet::request_funds(RuntimeOrigin::none(), 1, 21),
            Error::<Test>::RequestLimitExceeded
        );

        System::set_block_number(BLOCKS_PER_HOUR * 24);

        assert_ok!(Faucet::request_funds(RuntimeOrigin::none(), 1, 21));
        assert_eq!(Balances::free_balance(1), 101);
    });
}
