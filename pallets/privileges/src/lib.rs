//! This pallet holds the VIP status of users.
//! TODO: add description to this pallet (Privilege-pallet)

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use chrono::{DateTime, Datelike, NaiveDate};
pub use contribution_info::*;
use frame_support::{
    ensure,
    pallet_prelude::{Decode, DispatchResult, TypeInfo},
    traits::{LockableCurrency, UnixTime},
    weights::Weight,
    RuntimeDebug,
};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use pallet_energy_generation::OnVipMembershipHandler;
use parity_scale_codec::Encode;
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::*;
use sp_runtime::SaturatedConversion;
use sp_std::prelude::*;
pub use weights::WeightInfo;

mod contribution_info;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::UnixTime;
    use frame_system::{ensure_root, ensure_signed};

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_nac_managing::Config + pallet_energy_generation::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency trait.
        type Currency: LockableCurrency<Self::AccountId>;

        /// Time used for computing year, quarter durations.
        type UnixTime: UnixTime;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn vip_members)]
    pub type VipMembers<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, VipMemberInfo<T>>;

    #[pallet::storage]
    #[pallet::getter(fn year_vip_results)]
    pub type YearVipResults<T: Config> = StorageMap<
        _,
        Twox64Concat,
        i32,
        Vec<(T::AccountId, <T as pallet_energy_generation::Config>::StakeBalance)>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn current_date)]
    pub type CurrentDate<T: Config> = StorageValue<_, CurrentDateInfo, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New VIP member was added.
        NewVipMember {
            /// Who becomes VIP status.
            account: T::AccountId,
        },
        /// Penalty type was successfully changed.
        PenaltyTypeChanged {
            /// Who changes penalty type.
            account: T::AccountId,
            /// New penalty type.
            new_penalty_type: PenaltyType,
        },
        /// The user has left the VIP.
        LeftVip {
            /// Who has left the VIP.
            account: T::AccountId,
            /// Penalty of this user.
            penalty: Perbill,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account is not legit for VIP (isn't validator or cooperator).
        AccountNotLegitForVip,
        /// Account already has VIP status.
        AlreadyVipMember,
        /// Account hasn't VIP status.
        AccountHasNotVipStatus,
        /// Currently is not a penalty-free period.
        IsNotPenaltyFreePeriod,
        /// Not correct date to set.
        NotCorrectDate,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Become a VIP status.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::become_vip_status())]
        pub fn become_vip_status(origin: OriginFor<T>, tax_type: PenaltyType) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;
            ensure!(!VipMembers::<T>::contains_key(&who), Error::<T>::AlreadyVipMember);

            if Self::is_legit_for_vip(&who) {
                Self::do_set_user_privilege(&who, tax_type);
                Self::deposit_event(Event::<T>::NewVipMember { account: who });
                Ok(())
            } else {
                Err(Error::<T>::AccountNotLegitForVip.into())
            }
        }

        /// Update current date.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::set_quarter_revenue())]
        pub fn update_time(
            origin: OriginFor<T>,
            new_date_year: i32,
            new_date_month: u32,
            new_date_day: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let current_date = Self::current_date();
            let new_date = CurrentDateInfo::new(new_date_year, new_date_month, new_date_day);

            let current_date_naive = NaiveDate::from_ymd_opt(
                current_date.current_year,
                current_date.current_month,
                current_date.current_day,
            )
            .ok_or(Error::<T>::NotCorrectDate)?;
            let new_date_naive =
                NaiveDate::from_ymd_opt(new_date_year, new_date_month, new_date_day)
                    .ok_or(Error::<T>::NotCorrectDate)?;

            let days_since_new_year = (new_date_naive - current_date_naive).num_days() as u64;

            // Accrual of VIP points for users who have VIP status.
            Self::update_points_for_time(days_since_new_year, new_date.current_quarter);

            if new_date.current_month == 1 && new_date.current_day == 1 {
                Self::save_year_info(new_date.current_year - 1);
            }
            CurrentDate::<T>::put(new_date);

            Ok(())
        }

        /// Exit VIP.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::exit_vip())]
        pub fn exit_vip(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;

            Self::do_exit_vip(&who)
        }

        /// Change penalty type.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::change_penalty_type())]
        pub fn change_penalty_type(
            origin: OriginFor<T>,
            new_tax_type: PenaltyType,
        ) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;

            Self::do_change_penalty_type(&who, new_tax_type)
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Set user privilege as VIP.
    fn do_set_user_privilege(account: &T::AccountId, tax_type: PenaltyType) {
        let now_as_millis_u64 = <T as Config>::UnixTime::now().as_millis().saturated_into::<u64>();
        let active_stake = pallet_energy_generation::Pallet::<T>::get_active_stake(account);

        let vip_member_info = VipMemberInfo {
            start: now_as_millis_u64,
            tax_type,
            points: <T as pallet_energy_generation::Config>::StakeBalance::default(),
            active_stake,
        };

        VipMembers::<T>::insert(account, vip_member_info);
    }

    /// Exit VIP.
    pub fn do_exit_vip(account: &T::AccountId) -> DispatchResult {
        let current_date = Self::current_date();
        let vip_info = Self::vip_members(account);
        match vip_info {
            Some(vip_member_info) => {
                let mut penalty_percent = Perbill::default();
                if !Self::is_penalty_free_period() {
                    let slash_percent =
                        vip_member_info.tax_type.penalty_percent(current_date.current_quarter);
                    penalty_percent = slash_percent;
                }

                VipMembers::<T>::remove(account);
                Self::deposit_event(Event::<T>::LeftVip {
                    account: account.clone(),
                    penalty: penalty_percent,
                });
                Ok(())
            },
            None => Err(Error::<T>::AccountHasNotVipStatus.into()),
        }
    }

    /// Change penalty type.
    pub fn do_change_penalty_type(
        account: &T::AccountId,
        new_penalty_type: PenaltyType,
    ) -> DispatchResult {
        ensure!(!Self::is_penalty_free_period(), Error::<T>::IsNotPenaltyFreePeriod);
        VipMembers::<T>::try_mutate::<_, _, Error<T>, _>(account, |vip_config| {
            if let Some(vip) = vip_config {
                vip.tax_type = new_penalty_type;
                Ok(())
            } else {
                Err(Error::<T>::AccountHasNotVipStatus)
            }
        })?;

        Self::deposit_event(Event::<T>::PenaltyTypeChanged {
            account: account.clone(),
            new_penalty_type,
        });
        Ok(())
    }

    /// Assesses whether a user qualifies as a VIP, and whether they are a validator or a cooperator within the network.
    fn is_legit_for_vip(account: &T::AccountId) -> bool {
        // Check account validator status.
        if pallet_energy_generation::Pallet::<T>::is_user_validator(account) {
            return true;
        }

        // Check account cooperator status.
        if let Some(cooperation) = pallet_energy_generation::Pallet::<T>::cooperators(account) {
            !cooperation.targets.is_empty()
        } else {
            false
        }
    }

    /// Is now penalty-free period.
    fn is_penalty_free_period() -> bool {
        let current_date = Self::current_date();

        current_date.current_month == 1
    }

    /// Update current quarter info.
    pub fn update_quarter_info() {
        let now_as_millis_u64 =
            <T as Config>::UnixTime::now().as_millis().saturated_into::<u64>() / 1000u64;

        let new_date =
            DateTime::from_timestamp(i64::try_from(now_as_millis_u64).unwrap(), 0).unwrap();
        let new_date_naive =
            NaiveDate::from_ymd_opt(new_date.year(), new_date.month(), new_date.day()).unwrap();

        let start_date = NaiveDate::from_ymd_opt(new_date.year(), 1, 1).unwrap();

        let days_since_new_year = (start_date - new_date_naive).num_days() as u64;
        let current_date = Self::current_date();

        // Checking whether the day information needs to be updated.
        if current_date.current_day != new_date.day() {
            let current_data_info =
                CurrentDateInfo::new(new_date.year(), new_date.month(), new_date.day());
            // Accrual of VIP points for users who have VIP status.
            Self::update_points_for_time(days_since_new_year, current_data_info.current_quarter);

            if new_date.month() == 1 && new_date.day() == 1 {
                Self::save_year_info(new_date.year() - 1);
            }

            CurrentDate::<T>::put(current_data_info);
        }
    }

    /// Updates the points for the time since the last time the account was updated.
    pub fn update_points_for_time(days_since_new_year: u64, current_quarter: u8) {
        VipMembers::<T>::translate(|_, mut old_info: VipMemberInfo<T>| {
            let multiplier = Self::calculate_multiplier(old_info.tax_type, current_quarter);
            let points =
                Self::calculate_points(days_since_new_year, old_info.active_stake, multiplier);
            let new_points = old_info.points.saturating_add(points);
            old_info.points = new_points;
            Some(old_info)
        });
    }

    /// Calculate multiplier that differs depending on penalty type.
    fn calculate_multiplier(penalty_type: PenaltyType, current_quarter: u8) -> Perbill {
        match penalty_type {
            PenaltyType::Flat => Perbill::from_rational(7_u32, 40_u32),
            PenaltyType::Declining => Perbill::from_percent(30 - current_quarter as u32 * 5),
        }
    }

    /// Save VIP year information to pay rewards.
    pub fn save_year_info(current_year: i32) {
        let mut results = Vec::new();
        VipMembers::<T>::translate(|account, mut vip_info: VipMemberInfo<T>| {
            results.push((account, vip_info.points));
            vip_info.points = <T as pallet_energy_generation::Config>::StakeBalance::default();
            Some(vip_info)
        });

        YearVipResults::<T>::insert(current_year, results);
    }

    /// Calculate VIP points for account.
    fn calculate_points(
        days_since_new_year: u64,
        active_stake: T::StakeBalance,
        multiplier: Perbill,
    ) -> <T as pallet_energy_generation::Config>::StakeBalance {
        (multiplier * active_stake) / days_since_new_year.into()
    }
}

impl<T: Config> OnVipMembershipHandler<T::AccountId, Weight> for Pallet<T> {
    fn change_quarter_info() -> Weight {
        Self::update_quarter_info();
        Weight::from_parts(1, 1)
    }

    fn kick_account_from_vip(account: &T::AccountId) -> Weight {
        let _ = Self::do_exit_vip(account);
        Weight::from_parts(1, 1)
    }

    fn update_active_stake(account: &T::AccountId) -> Weight {
        VipMembers::<T>::mutate(account, |vip_info| {
            if let Some(vip_info) = vip_info {
                vip_info.active_stake =
                    pallet_energy_generation::Pallet::<T>::get_active_stake(account);
            }
        });
        Weight::from_parts(1, 2)
    }
}
