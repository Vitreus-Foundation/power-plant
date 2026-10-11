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

/// Information about VIP and VIPP rewards.
#[derive(Encode, Decode, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct RewardsInfo<T: Config> {
    /// VIP points accumulated during the year.
    pub vip_points: PointsOf<T>,

    /// VIP rewards allocated for the year.
    pub vip_rewards: BalanceOf<T>,

    /// VIPP points accumulated during the year.
    pub vipp_points: PointsOf<T>,

    /// VIPP rewards allocated for the year.
    pub vipp_rewards: BalanceOf<T>,
}

impl<T: Config> OnVippStatusHandler<T::AccountId, T::StakeBalance, T::ItemId> for Pallet<T> {
    #[cfg(feature = "runtime-benchmarks")]
    fn benchmark_fill_breach_queue(members: u32) {
        let accounts: Vec<T::AccountId> = (0..members)
            .map(|i| frame_benchmarking::account("observer-pending-breach", i, 0))
            .collect();
        PreTimestampVippBreaches::<T>::put(
            frame_support::BoundedVec::<_, frame_support::traits::ConstU32<1024>>::try_from(
                accounts,
            )
            .expect("exact configured bound"),
        );
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn benchmark_populate_members(members: u32) {
        crate::benchmarking::populate::<T>(members);
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn benchmark_prepare_member(who: &T::AccountId, stake: T::StakeBalance) -> DispatchResult {
        CurrentDate::<T>::put(accounting::today::<T>()?);
        TimestampSeenAtBlock::<T>::put(frame_system::Pallet::<T>::block_number());
        VipMembers::<T>::insert(
            who,
            VipMemberInfo::<T> {
                start: 0,
                tax_type: PenaltyType::Flat,
                points: Zero::zero(),
                active_stake: stake,
            },
        );
        Self::sync_vipp_membership(who, false)
    }
    fn before_balance_change(who: &T::AccountId) {
        accounting::before_stake_change::<T>(who);
    }

    fn on_balance_breach(who: &T::AccountId) {
        breach_period::record::<T>(who);
    }

    fn reconcile_vipp(who: &T::AccountId) -> DispatchResult {
        Self::sync_vipp_membership(who, false)?;
        breach_period::settle::<T>(who)
    }

    fn ensure_repair_ready() -> DispatchResult {
        if !accounting::ApprovedCatchUp::<T>::exists()
            && migration::recovery::CacheRecoveryProgress::<T>::get()
                .is_some_and(|p| p.phase == migration::recovery::Phase::AwaitingReview)
        {
            return Ok(());
        }
        accounting::ensure_ready::<T>()
    }

    fn ensure_accounting_ready() -> DispatchResult {
        accounting::ensure_ready::<T>()
    }

    fn can_receive_vipp(who: &T::AccountId) -> bool {
        VipMembers::<T>::contains_key(who)
    }

    fn mint_vipp(
        who: &T::AccountId,
        _amount: T::StakeBalance,
        _item_id: T::ItemId,
    ) -> DispatchResult {
        Self::update_quarter_info();
        Self::sync_vipp_membership(who, false)
    }

    fn burn_vipp_nft(who: &T::AccountId, _item_id: T::ItemId) -> DispatchResult {
        Self::update_quarter_info();
        Self::sync_vipp_membership(who, false)
    }
}
