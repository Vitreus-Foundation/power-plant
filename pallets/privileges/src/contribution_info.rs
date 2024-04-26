use core::default::Default;
use super::*;

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, TypeInfo)]
pub struct CurrentDateInfo {
    pub current_year: i32,
    pub current_month: u32,
    pub current_day: u32,
    pub current_quarter: u8,
}

impl Default for CurrentDateInfo
{
    fn default() -> Self {
        CurrentDateInfo {
            current_year: 0,
            current_month: 0,
            current_day: 0,
            current_quarter: 0
        }
    }
}

impl CurrentDateInfo {
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

#[derive(Copy, Clone, Encode, Decode, RuntimeDebug, Eq, PartialEq, TypeInfo)]
pub enum PenaltyType {
    Declining,
    Flat,
}

impl PenaltyType {
    pub fn penalty_percent(&self, current_quarter: u8) -> Perbill {
        match self {
            PenaltyType::Flat => Perbill::from_rational(7_u32, 40_u32),
            PenaltyType::Declining => Perbill::from_percent(30 - (5 * current_quarter as u32))
        }
    }
}

#[derive(Copy, Clone, Encode, Decode, RuntimeDebug, Eq, PartialEq, TypeInfo)]
pub enum StakingStatus {
    Validator,
    Cooperator,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct VipMemberInfo {
    pub start: u64,
    pub tax_type: PenaltyType,
    pub points: u128,
    pub staking_status: StakingStatus,
}