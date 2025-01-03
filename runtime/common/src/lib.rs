//! Common traits for runtime.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

// TODO: move here custom traits from pallets

pub use sp_staking::{EraIndex, SessionIndex};

/// A trait for executing actions when a new session begins.
#[impl_trait_for_tuples::impl_for_tuples(8)]
pub trait OnSessionChange {
    /// Called at the start of a new session.
    fn on_new_session(index: SessionIndex);
}

/// A trait for handling events when energy is burned.
#[impl_trait_for_tuples::impl_for_tuples(8)]
pub trait OnEnergyBurn<Balance> {
    /// Called whenever energy is burned.
    fn on_energy_burn(amount: Balance);
}

/// A trait for handling events when energy is sold.
#[impl_trait_for_tuples::impl_for_tuples(8)]
pub trait OnEnergySell<Balance> {
    /// Called whenever energy is sold.
    fn on_energy_sell(amount: Balance);
}

/// A trait that determines how much energy should be generated for a given era.
pub trait EraEnergyRateCalculator<Energy> {
    /// Calculates the energy generation for the given era.
    fn calculate(era: EraIndex) -> Option<Energy>;
}

impl<T> EraEnergyRateCalculator<T> for () {
    fn calculate(_era: EraIndex) -> Option<T> {
        None
    }
}

/// A trait for querying staking-related information.
pub trait Staking<Balance> {
    /// Returns the total stake for the specified era.
    fn total_stake(era: EraIndex) -> Balance;
}

/// A trait for querying era and session-related information.
pub trait EraSessionLookup {
    /// Returns the era index corresponding to the given session index.
    fn era_for_session(session_index: SessionIndex) -> Option<EraIndex>;

    /// Returns the half-open range of sessions `[start, end)` for the specified era.
    fn session_range_for_era(era_index: EraIndex) -> Option<(SessionIndex, SessionIndex)>;
}

/// A trait for querying the storage capacity and current energy level of a warehouse.
pub trait Warehouse<Energy> {
    /// Returns the current amount of energy stored in the warehouse.
    fn current_amount() -> Energy;

    /// Returns the maximum capacity of the warehouse for storing energy.
    fn max_capacity() -> Energy;
}
