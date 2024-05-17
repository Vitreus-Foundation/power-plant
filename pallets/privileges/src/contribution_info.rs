use super::*;
use core::default::Default;
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
    /// Choosed penalty type.
    pub tax_type: PenaltyType,
    /// Current VIP points.
    pub points: T::StakeBalance,
    /// Current active stake.
    pub active_stake: T::StakeBalance,
}
