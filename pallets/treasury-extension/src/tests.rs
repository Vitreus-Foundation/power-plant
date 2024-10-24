//! Unit Tests for Treasury Pallet Extension
//!
//! This module provides unit tests for the Treasury pallet extension, ensuring that all key functions behave as expected in various scenarios.
//! It uses the mock runtime defined in `mock.rs` to simulate blockchain operations, validate spending logic, and ensure proper handling of treasury funds.
//!
//! # Features
//! - Tests different treasury operations, such as spending funds, handling thresholds, and ensuring proper balance deductions.
//! - Uses the `assert_ok` and `expect` macros to verify both success paths and error conditions.
//! - Validates the triggering of events such as spending events within the treasury pallet.
//!
//! # Structure
//! - Imports the mock runtime and all necessary pallet components, including events from both the Treasury and the Treasury extension.
//! - Contains multiple unit tests designed to exercise different aspects of treasury fund management, focusing on both success and failure cases.
//! - Uses runtime origin (`RuntimeOrigin::root()`) to simulate different user permissions and validate access control.
//!
//! # Tests Overview
//! - **spend_funds_should_work**: Tests the functionality of spending funds from the treasury, ensuring that the remaining balance and thresholds are calculated correctly.
//! - **additional tests**: More test cases should be added to cover corner cases such as exceeding the spend threshold, unauthorized access, and incorrect fund allocations.
//!
//! # Usage
//! - Use these tests to ensure that changes to the Treasury pallet extension do not introduce regressions or unintended behavior.
//! - Run the tests using `cargo test` in the Substrate development environment to validate correctness.
//!
//! # Dependencies
//! - Relies on `frame_support` for testing utilities and event handling.
//! - Uses the mock runtime from `mock.rs` to provide a simulated environment for testing the Treasury extension.
//! - Imports events like `TreasuryEvent` to validate that correct events are triggered during operations.
//!
//! # Important Notes
//! - Ensure the mock runtime is properly configured before running tests to prevent misleading results.
//! - Expand the unit tests to maintain comprehensive coverage as additional features or changes are introduced to the treasury logic.
//! - Properly simulate user roles and permissions to ensure robust access control validation.

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

        Treasury::spend_local(RuntimeOrigin::root(), spent, ALICE)
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

        Treasury::spend_local(RuntimeOrigin::root(), spent, ALICE)
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
