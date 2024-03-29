use crate::mock::*;
use crate::Event;
use pallet_treasury::Event as TreasuryEvent;

#[test]
fn spend_funds_should_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let budget_remaining = Treasury::pot();
        let threshold = SpendThreshold::get().mul_ceil(budget_remaining);
        let spent = threshold - 1;
        let left = budget_remaining - threshold;

        Treasury::spend(RuntimeOrigin::root(), spent, ALICE)
            .expect("Expected to add and approve treasury spend proposal");
        Treasury::spend_funds();

        // making sure that Treasury hasn't emit Burnt event.
        let events = System::events();
        assert!(!events.iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Treasury(TreasuryEvent::<Test>::Burnt { .. })
        )),);

        System::assert_has_event(Event::<Test>::Recycled { recyled_funds: 1 }.into());
        assert_eq!(Treasury::pot(), left);
    });
}

#[test]
fn ensure_no_recycle_upon_spend_threhsold_exceeding() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let budget_remaining = Treasury::pot();
        let threshold = SpendThreshold::get().mul_ceil(budget_remaining);
        let spent = threshold;
        let left = budget_remaining - threshold;

        Treasury::spend(RuntimeOrigin::root(), spent, ALICE)
            .expect("Expected to add and approve treasury spend proposal");
        Treasury::spend_funds();

        // making sure that Treasury hasn't emit Burnt event
        // and TreasuryExtension hasn't emitted Recycled event.
        let events = System::events();
        assert!(!events.iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Treasury(TreasuryEvent::<Test>::Burnt { .. })
        ) | matches!(
            record.event,
            RuntimeEvent::TreasuryExtension(Event::<Test>::Recycled { .. })
        )));

        assert_eq!(Treasury::pot(), left);
    });
}
