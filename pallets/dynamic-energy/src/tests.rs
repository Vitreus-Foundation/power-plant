use crate::{mock::*, *};

#[test]
fn energy_generation_works() {
    new_test_ext().execute_with(|| {
        Pallet::<Test>::on_new_session(0);
        assert!(EnergyGeneration::<Test>::contains_key(0));

        Pallet::<Test>::on_new_session(1);
        assert!(EnergyGeneration::<Test>::contains_key(1));

        Pallet::<Test>::on_new_session(2);
        assert!(EnergyGeneration::<Test>::contains_key(2));

        let total_generation = EnergyGeneration::<Test>::get(0)
            + EnergyGeneration::<Test>::get(1)
            + EnergyGeneration::<Test>::get(2);

        assert!(total_generation > 0);
        assert_eq!(Pallet::<Test>::calculate(0), Some(total_generation));

        // old data removed
        Pallet::<Test>::on_new_session(3);
        Pallet::<Test>::on_new_session(4);
        assert!(!EnergyGeneration::<Test>::contains_key(0));
        assert!(EnergyGeneration::<Test>::contains_key(1));
        assert!(EnergyGeneration::<Test>::contains_key(2));
        assert!(EnergyGeneration::<Test>::contains_key(3));
        assert!(EnergyGeneration::<Test>::contains_key(4));
    });
}

#[test]
fn calculating_sold_energy_works() {
    new_test_ext().execute_with(|| {
        Pallet::<Test>::on_new_session(0);
        assert_eq!(SessionEnergySale::<Test>::get(), 0);

        Pallet::<Test>::on_energy_sell(500);
        Pallet::<Test>::on_energy_sell(1000);
        assert_eq!(SessionEnergySale::<Test>::get(), 1500);

        Pallet::<Test>::on_new_session(1);
        assert_eq!(SessionEnergySale::<Test>::get(), 0);
    });
}

#[test]
fn calculating_burned_energy_works() {
    new_test_ext().execute_with(|| {
        Pallet::<Test>::on_new_session(0);
        assert_eq!(SessionEnergyBurn::<Test>::get(), 0);

        Pallet::<Test>::on_energy_burn(500);
        Pallet::<Test>::on_energy_burn(1000);
        assert_eq!(SessionEnergyBurn::<Test>::get(), 1500);

        Pallet::<Test>::on_new_session(1);
        assert_eq!(SessionEnergyBurn::<Test>::get(), 0);
    });
}
