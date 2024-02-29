use super::*;
use crate::mock::*;

mod mock;
mod precompiles;

#[test]
fn deposit_wvtrs_should_work() {
    new_test_ext(0).execute_with(|| {
        let balance = BalancesVTRS::free_balance(&ALICE);
        assert_eq!(BalancesVTRS::free_balance(&ALICE), VTRS_INITIAL_BALANCE);
        EnergyBroker::deposit(RuntimeOrigin::signed(ALICE), ALICE.into(), 1000.into()).expect("Expected to deposit wvtrs");
        assert_eq!(BalancesVTRS::free_balance(&ALICE), balance - 1000);
        assert_eq!(BalancesVTRS::free_balance(&CONTRACT), 1000);
        EnergyBroker::withdraw(RuntimeOrigin::signed(ALICE), ALICE.into(), 1000.into()).expect("Expected to withdraw wvtrs");
        assert_eq!(BalancesVTRS::free_balance(&ALICE), balance);
        assert_eq!(BalancesVTRS::free_balance(&CONTRACT), 0);
        
    });
}
