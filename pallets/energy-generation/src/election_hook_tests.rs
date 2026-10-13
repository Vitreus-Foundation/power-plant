//! Election-time eligibility removals must consume the mandatory hook budget.

use crate::{mock::*, Collaborations, Config, Validators, WeightInfo};
use frame_support::dispatch::DispatchClass;
use pallet_reputation::{AccountReputation, ReputationRecord};

fn charged_election_weight() -> frame_support::weights::Weight {
    // Default fixture also contains non-collaborative validator31, whose empty
    // collaboration removal is independently charged during each election.
    <Test as frame_system::Config>::DbWeight::get()
        .reads(2)
        .saturating_add(<<Test as Config>::ThisWeightInfo as WeightInfo>::chill_hook(1))
        .saturating_add(<<Test as Config>::ThisWeightInfo as WeightInfo>::chill_hook(0))
}

#[test]
fn election_chill_reserves_budget_and_deactivates_delegation() {
    ExtBuilder::default().build_and_execute(|| {
        assert_eq!(Collaborations::<Test>::get(11).unwrap().len(), 1);
        assert_eq!(PowerPlant::get_active_stake(&101), 500);
        AccountReputation::<Test>::remove(11);
        let before = *System::block_weight().get(DispatchClass::Mandatory);
        assert!(PowerPlant::try_trigger_new_era(12).is_some());
        assert!(!Validators::<Test>::contains_key(11));
        assert!(!Collaborations::<Test>::contains_key(11));
        assert_eq!(PowerPlant::get_active_stake(&101), 300);
        assert_eq!(
            *System::block_weight().get(DispatchClass::Mandatory),
            before.saturating_add(charged_election_weight())
        );
    });
}

#[test]
fn election_collaboration_loss_reserves_budget_without_removing_validator() {
    ExtBuilder::default().build_and_execute(|| {
        AccountReputation::<Test>::insert(
            11,
            ReputationRecord {
                reputation: <Test as Config>::ValidatorReputationTier::get().into(),
                updated: 0,
            },
        );
        let before = *System::block_weight().get(DispatchClass::Mandatory);
        assert!(PowerPlant::try_trigger_new_era(12).is_some());
        assert!(Validators::<Test>::contains_key(11));
        assert!(!Validators::<Test>::get(11).collaborative);
        assert!(!Collaborations::<Test>::contains_key(11));
        assert_eq!(PowerPlant::get_active_stake(&101), 300);
        assert_eq!(
            *System::block_weight().get(DispatchClass::Mandatory),
            before.saturating_add(charged_election_weight())
        );
    });
}
