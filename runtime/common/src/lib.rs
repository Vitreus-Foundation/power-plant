//! Common traits for runtime.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

// TODO: move here custom traits from pallets

pub use sp_staking::{EraIndex, SessionIndex};

use sp_runtime::{
    traits::{One, Zero},
    FixedU64, Saturating,
};

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

/// A trait for calculating the exposure multiplier for an account.
pub trait ExposureMultiplier<AccountId> {
    /// Returns the bonus multiplier component for the given account.
    fn bonus_part(account_id: &AccountId) -> FixedU64;

    /// Returns the total exposure multiplier for the given account.
    ///
    /// This includes the bonus component and a base multiplier of 1.
    fn multiplier(account_id: &AccountId) -> FixedU64 {
        FixedU64::one().saturating_add(Self::bonus_part(account_id))
    }
}

#[impl_trait_for_tuples::impl_for_tuples(8)]
impl<AccountId> ExposureMultiplier<AccountId> for Tuple {
    #[allow(clippy::let_and_return)]
    fn bonus_part(account_id: &AccountId) -> FixedU64 {
        let mut total = FixedU64::zero();

        for_tuples!( #(
            total.saturating_accrue(Tuple::bonus_part(account_id));
        )* );

        total
    }
}

/// A trait for querying era and session-related information.
pub trait EraSessionLookup {
    /// Returns the index of the ongoing era.
    fn active_era() -> Option<EraIndex>;

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
