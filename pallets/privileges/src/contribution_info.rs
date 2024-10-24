//!
//! # Module Overview
//!
//! This module provides the core functionality for managing VIP and VIPP membership contributions
//! within a Substrate-based blockchain. It includes structures and methods for tracking VIP member
//! information, managing penalties, updating membership data, and ensuring the correctness of date
//! information used for contribution calculations.
//!
//! # Key Features and Components
//!
//! - **Current Date Information**:
//!   - **`CurrentDateInfo`**: A struct that tracks the current date, including year, month, day,
//!     quarter, and days since the start of the year. This struct is used to manage quarterly and
//!     yearly membership cycles and ensure consistency across time-based operations.
//!   - **`new()` Function**: Constructs a new `CurrentDateInfo` instance by checking the quarter
//!     and calculating the number of days since the new year. This function ensures that all date
//!     information is accurate and that the appropriate quarter is set.
//!
//! - **VIP and VIPP Membership Management**:
//!   - **`VipMemberInfo`**: A struct representing a VIP member's information, including the start
//!     date, penalty type, points, and active stake. This is used to manage membership benefits and
//!     calculate contributions.
//!   - **`VippMemberInfo`**: A struct representing VIPP member information, including VIP points
//!     and the active VIPP threshold. It is used for members who qualify for the Very Important
//!     Person Protocol (VIPP) status due to exceptional contributions.
//!
//! - **Penalty Handling**:
//!   - **`PenaltyType` Enum**: Defines two types of penalties—`Declining` and `Flat`. The penalty
//!     calculation is based on the current quarter, and the `penalty_percent()` method provides
//!     the correct percentage based on the penalty type and current quarter.
//!
//! - **OnVippStatusHandler Implementation**:
//!   - The module implements `OnVippStatusHandler` for minting and burning VIPP NFTs. This interface
//!     defines how to handle VIPP statuses, ensuring that users who meet or lose their VIPP
//!     qualifications are rewarded or penalized appropriately by minting or burning VIPP NFTs.
//!
//! # Access Control and Security
//!
//! - **Error Handling**: The module includes validation functions (`check_quarter`, `check_days_since_new_year`)
//!   to ensure that all date-related information is accurate. Errors are handled through the `DispatchError`
//!   type to maintain consistency and robustness in the system's state.
//! - **Controlled Membership Updates**: VIP and VIPP membership updates are tightly controlled by
//!   restricting minting and burning actions to specific handlers, ensuring that only authorized
//!   operations affect member statuses.
//!
//! # Developer Notes
//!
//! - **Quarter and Year Calculation**: The `CurrentDateInfo` struct and related functions help manage
//!   the complexity of calculating the current quarter and number of days since the start of the year.
//!   These functions should be reviewed whenever changing the logic that affects how contributions
//!   and penalties are applied to members based on date information.
//! - **Penalty Type Flexibility**: The `PenaltyType` enum provides a flexible mechanism for defining
//!   different types of penalties. Developers can extend this enum to add new penalty types if needed
//!   to support additional membership rules or custom behaviors.
//! - **Handling VIPP Thresholds**: The `VippMemberInfo` struct includes an `active_vipp_threshold`
//!   field, which allows developers to track the specific items and their associated thresholds that
//!   qualify a member for VIPP status. This is essential for determining who is eligible for VIPP
//!   benefits based on their contributions.
//!
//! # Usage Scenarios
//!
//! - **Adding and Updating Date Information**: The `new()` function in `CurrentDateInfo` is used
//!   whenever new membership data needs to be updated based on the current date. It ensures that
//!   the appropriate quarter and day information is always accurate, which is crucial for calculating
//!   contributions and applying penalties or rewards.
//! - **Penalty Application**: The `PenaltyType` enum's `penalty_percent()` function is used to apply
//!   penalties based on the current quarter. This feature helps enforce different penalty schemes,
//!   such as declining penalties over time or a flat rate, depending on the network's rules for
//!   membership maintenance.
//! - **Minting and Burning VIPP NFTs**: The `OnVippStatusHandler` implementation allows for the
//!   automatic minting or burning of VIPP NFTs based on member actions. For example, when a user
//!   reaches a new threshold, the `mint_vipp()` function is called to issue a new VIPP NFT, rewarding
//!   them for their contributions.
//!
//! # Integration Considerations
//!
//! - **Integration with Energy Generation and NAC Pallets**: This module is designed to work with
//!   `pallet_energy_generation` and `pallet_nac_managing`. Developers should ensure that any changes
//!   to these pallets, particularly those affecting stake calculations or NAC level management, are
//!   reflected in this module to maintain consistency in VIP membership data and contributions.
//! - **Date Validation and Synchronization**: Since this pallet uses time-based calculations, it is
//!   critical to synchronize date-related operations with other parts of the blockchain that might
//!   rely on similar timeframes, such as staking or rewards payouts.
//! - **Extending Penalty Types**: The `PenaltyType` enum and its associated calculation logic are
//!   flexible but should be updated cautiously. If new penalty types are added, make sure that the
//!   calculation logic and associated tests are updated to reflect the changes accurately.
//!
//! # Example Scenario
//!
//! Suppose a member qualifies for VIPP status by meeting specific contribution requirements. Using
//! the `mint_vipp()` function defined in the `OnVippStatusHandler` implementation, a new VIPP NFT
//! is minted for that member, representing their status upgrade. The `CurrentDateInfo` struct ensures
//! that the correct quarter and date information is recorded, providing an accurate snapshot of when
//! the member achieved VIPP status. The `PenaltyType` is then used to determine any applicable penalties
//! for maintaining their status, depending on the member's current activity level and contribution history.
//!


use super::*;
use core::default::Default;
use pallet_nac_managing::OnVippStatusHandler;
use sp_core::RuntimeDebug;
use sp_runtime::DispatchError;

/// Current date info.
#[derive(Clone, Encode, Decode, Default, RuntimeDebug, PartialEq, TypeInfo)]
pub struct CurrentDateInfo {
    /// Current year.
    pub current_year: i32,
    /// Current month.
    pub current_month: u32,
    /// Current day.
    pub current_day: u32,
    /// Current quarter.
    pub current_quarter: u8,
    /// Days since new year.
    pub days_since_new_year: u64,
}

impl CurrentDateInfo {
    /// Create new Current Date Info.
    pub fn new<T: Config>(
        current_year: i32,
        current_month: u32,
        current_day: u32,
    ) -> Result<Self, DispatchError> {
        let current_quarter =
            Self::check_quarter(current_month).ok_or(Error::<T>::NotCorrectDate)?;
        let days_since_new_year =
            Self::check_days_since_new_year::<T>(current_year, current_month, current_day)?;

        let current_date_info = CurrentDateInfo {
            current_year,
            current_month,
            current_day,
            current_quarter,
            days_since_new_year,
        };
        Ok(current_date_info)
    }

    /// Check quarter depends on month.
    fn check_quarter(month: u32) -> Option<u8> {
        match month {
            1..=3 => Some(1),
            4..=6 => Some(2),
            7..=9 => Some(3),
            10..=12 => Some(4),
            _ => None,
        }
    }

    /// Check days since new year.
    fn check_days_since_new_year<T: Config>(
        current_year: i32,
        current_month: u32,
        current_day: u32,
    ) -> Result<u64, DispatchError> {
        let year_first_day =
            NaiveDate::from_ymd_opt(current_year, 1, 1).ok_or(Error::<T>::NotCorrectDate)?;
        let new_date_naive = NaiveDate::from_ymd_opt(current_year, current_month, current_day)
            .ok_or(Error::<T>::NotCorrectDate)?;

        let days_since_new_year = (new_date_naive - year_first_day).num_days() as u64;
        Ok(days_since_new_year)
    }

    /// Add days to date.
    pub fn add_days<T: Config>(&mut self, days_num: u64) -> Result<(), DispatchError> {
        let mut current_date_naive =
            NaiveDate::from_ymd_opt(self.current_year, self.current_month, self.current_day)
                .ok_or(Error::<T>::NotCorrectDate)?;
        current_date_naive = current_date_naive
            .checked_add_days(Days::new(days_num))
            .ok_or(Error::<T>::NotCorrectDate)?;

        self.current_day = current_date_naive.day();
        self.current_month = current_date_naive.month();
        self.current_year = current_date_naive.year();
        self.current_quarter =
            Self::check_quarter(self.current_month).ok_or(Error::<T>::NotCorrectDate)?;
        self.days_since_new_year = Self::check_days_since_new_year::<T>(
            self.current_year,
            self.current_month,
            self.current_day,
        )?;

        Ok(())
    }
}

/// Enum of penalty types.
#[derive(Copy, Clone, Encode, Decode, RuntimeDebug, Eq, PartialEq, TypeInfo)]
pub enum PenaltyType {
    /// Declining type of penalty.
    Declining,
    /// Flat type of penalty.
    Flat,
}

impl PenaltyType {
    /// Calculate percent of penalty depending on current quarter.
    pub fn penalty_percent(&self, current_quarter: u8) -> Perbill {
        match self {
            PenaltyType::Flat => Perbill::from_rational(7_u32, 40_u32),
            PenaltyType::Declining => Perbill::from_percent(30 - (5 * current_quarter as u32)),
        }
    }
}

/// Information about VIP member.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct VipMemberInfo<T: pallet_energy_generation::Config> {
    /// Where member started VIP program.
    pub start: u64,
    /// Chose penalty type.
    pub tax_type: PenaltyType,
    /// Current VIP points.
    pub points: T::StakeBalance,
    /// Current active stake.
    pub active_stake: T::StakeBalance,
}

/// Information about VIPP member.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct VippMemberInfo<T: Config> {
    /// Current VIP points.
    pub points: T::StakeBalance,
    /// Current VIPP threshold.
    pub active_vipp_threshold: Vec<(T::ItemId, T::StakeBalance)>,
}

impl<T: Config> OnVippStatusHandler<T::AccountId, T::StakeBalance, T::ItemId> for Pallet<T> {
    fn mint_vipp(who: &T::AccountId, amount: T::StakeBalance, item_id: T::ItemId) {
        Self::mint_new_vipp_nft(who, amount, item_id);
    }

    fn burn_vipp_nft(who: &T::AccountId, item_id: T::ItemId) {
        Self::burn_vipp_nft(who, item_id);
    }
}
