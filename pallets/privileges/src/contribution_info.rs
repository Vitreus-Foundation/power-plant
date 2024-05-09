use core::default::Default;
use super::*;

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
}

impl CurrentDateInfo {
    /// Create new Current Date Info.
    pub fn new(
        current_year: i32,
        current_month: u32,
        current_day: u32,
    ) -> CurrentDateInfo {
        CurrentDateInfo {
            current_month,
            current_day,
            current_year,
            current_quarter: Self::check_quarter(current_month),
        }
    }

    /// Check quarter depends on month.
    fn check_quarter(month: u32) -> u8 {
        match month {
            1..=3 => 1,
            4..=6 => 2,
            7..=9 => 3,
            10..=12 => 4,
            _ => 0
        }
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
            PenaltyType::Declining => Perbill::from_percent(30 - (5 * current_quarter as u32))
        }
    }
}

/// Information about VIP member.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct VipMemberInfo<T: pallet_energy_generation::Config> {
    /// Where member started VIP program.
    pub start: u64,
    /// Choosed penalty type.
    pub tax_type: PenaltyType,
    /// Current VIP points.
    pub points: T::StakeBalance,
    /// Current active stake.
    pub active_stake: T::StakeBalance,
}