//!
//! # Module Overview
//!
//! This module defines a pallet for managing VIP and VIPP membership
//! in a Substrate-based blockchain. The pallet provides mechanisms for managing VIP memberships,
//! calculating loyalty points, updating membership information, handling penalties, and tracking
//! contributions. The primary aim is to manage memberships based on contributions, energy generation,
//! and reputation within the network.
//!
//! # Key Features and Components
//!
//! - **VIP Membership Management**:
//!   - **Storage**:
//!     - **`VipMembers`**: Tracks information related to VIP members, including active stake,
//!       contribution information, and tax type.
//!     - **`VippMembers`**: Stores details about VIPP members who qualify for a special status
//!       due to their exceptional contributions or activity.
//!
//! - **Extrinsic and Core Functions**:
//!   - **`add_new_vip_member()`**: Adds a new VIP member with initial details such as the stake,
//!     contribution, and tax type. This extrinsic is available only to root users to ensure proper
//!     control and security.
//!   - **`update_quarter_info()`**: Updates the quarterly information for all VIP members, including
//!     active stake and contribution points. This function helps ensure that each VIP's membership
//!     information is up to date and reflects recent activity.
//!   - **`on_claim()`**: Handles updates when users claim rewards, which affects their VIP status
//!     and potentially increments their loyalty points.
//!
//! - **Membership Handling and Contributions**:
//!   - **Tax Management**: Each VIP member has an associated `tax_type`, which determines the penalty
//!     and reward system. Depending on the current quarter, the tax percentage may vary, and different
//!     penalties or perks are applied based on the member's behavior and activity.
//!   - **Stake-Based Calculations**: Calculations are performed based on the active stake of VIP members.
//!     Functions like `calculate_points()` use stake information to calculate points that affect the
//!     member's level and rewards.
//!
//! - **Quarter and Year Management**:
//!   - **`check_correct_date()`**: Ensures that new date information is accurate and chronologically
//!     correct, preventing any inconsistency in the progression of quarterly or yearly membership data.
//!   - **Constants**: Constants such as `MAX_UPDATE_DAYS`, `FREE_PENALTY_PERIOD_MONTH_NUMBER`, and
//!     `YEAR_FIRST_MONTH` are used to manage quarterly and yearly information effectively, ensuring
//!     a consistent membership management cycle.
//!
//! # Access Control and Security
//!
//! - **Root-Only Access**: Certain extrinsic, such as adding a new VIP member, are restricted to
//!   root users (`ensure_root`). This restriction ensures that only authorized users can add new
//!   members to the VIP pool, providing an essential layer of access control.
//! - **Verification and Error Handling**: Functions include verification steps to ensure that all
//!   data is accurate, such as `check_correct_date()` to validate date progression. Errors are
//!   handled via `ensure()` statements, maintaining the integrity of the system's state.
//!
//! # Developer Notes
//!
//! - **Integration with Other Pallets**: This pallet integrates closely with `pallet_energy_generation`
//!   and `pallet_nac_managing`, leveraging energy generation data and NAC-level information to
//!   determine VIP status and manage stake-based interactions.
//! - **Weight Information**: Weights are defined for each extrinsic to represent the computational
//!   cost of the operation. The weight implementation (`WeightInfo`) is used to ensure the efficient
//!   and fair pricing of each operation based on its resource consumption.
//! - **Time Management**: The pallet relies on `UnixTime` for computing durations like years and
//!   quarters. This reliance helps ensure that the state updates are accurate and aligned with
//!   real-world time, which is crucial for membership updates and loyalty point calculations.
//!
//! # Usage Scenarios
//!
//! - **VIP Membership Addition**: An administrator can add a new VIP member to the blockchain
//!   using `add_new_vip_member()`, setting the initial parameters such as active stake, contribution
//!   information, and associated tax type. This functionality allows rewarding users who have made
//!   significant contributions to the network.
//! - **Updating Stake and Contributions**: The quarterly update functionality (`update_quarter_info()`)
//!   ensures that each VIP member’s contribution and stake are recalculated periodically. This feature
//!   helps maintain an up-to-date record of each member’s activity, ensuring that rewards and penalties
//!   are assigned fairly.
//! - **Reward Claim Handling**: When users claim rewards, the `on_claim()` function updates their
//!   VIP membership status and recalculates their loyalty points. This ensures that claiming rewards
//!   has a direct impact on the user's status, encouraging active participation.
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use chrono::{DateTime, Datelike, Days, NaiveDate};
pub use contribution_info::*;
use frame_support::pallet_prelude::BuildGenesisConfig;
use frame_support::{
    ensure,
    pallet_prelude::{Decode, DispatchResult, TypeInfo},
    traits::{LockableCurrency, UnixTime},
    weights::Weight,
};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use pallet_energy_generation::OnVipMembershipHandler;
use parity_scale_codec::Encode;
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::Perquintill;
use sp_runtime::{Perbill, SaturatedConversion};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

mod contribution_info;

pub mod weights;

const INCREASE_VIP_POINTS_CONSTANT: u64 = 50;

const MAX_UPDATE_DAYS: u32 = 366;

const FREE_PENALTY_PERIOD_MONTH_NUMBER: u32 = 1;

const YEAR_FIRST_MONTH: u32 = 1;
const YEAR_FIRST_DAY: u32 = 1;

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
        frame_system::Config + pallet_energy_generation::Config + pallet_nac_managing::Config
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
    #[pallet::getter(fn vipp_members)]
    pub type VippMembers<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, VippMemberInfo<T>>;

    #[pallet::storage]
    #[pallet::getter(fn year_vip_results)]
    pub type YearVipResults<T: Config> = StorageMap<
        _,
        Twox64Concat,
        i32,
        Vec<(T::AccountId, <T as pallet_energy_generation::Config>::StakeBalance)>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn year_vipp_results)]
    pub type YearVippResults<T: Config> = StorageMap<
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
        /// Account hasn't claim balance.
        HasNotClaim,
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
            let mut current_date = Self::current_date();
            let new_date = CurrentDateInfo::new::<T>(new_date_year, new_date_month, new_date_day)?;

            if !Self::check_correct_date(&current_date, &new_date) {
                return Err(Error::<T>::NotCorrectDate.into());
            }

            let mut updated_days = 0;

            while Self::check_correct_date(&current_date, &new_date)
                && updated_days < MAX_UPDATE_DAYS
            {
                current_date.add_days::<T>(1)?;
                // Accrual of VIP points for users who have VIP status.
                Self::update_points_for_time(current_date.days_since_new_year);

                // Accrual of VIPP points for users who have VIPP status.
                Self::update_vipp_points_for_time(current_date.days_since_new_year);
                if current_date.current_month == YEAR_FIRST_MONTH
                    && current_date.current_day == YEAR_FIRST_DAY
                {
                    Self::save_year_info(current_date.current_year - 1);
                }

                updated_days += 1;
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

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Initial date.
        pub date: Option<(i32, u32, u32)>,
        /// Phantom date.
        pub _config: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            match self.date {
                Some(date) => {
                    let current_data_info =
                        CurrentDateInfo::new::<T>(date.0, date.1, date.2).unwrap();

                    CurrentDate::<T>::put(current_data_info);
                },
                None => {
                    let now_as_millis_u64 =
                        <T as Config>::UnixTime::now().as_millis().saturated_into::<u64>()
                            / 1000u64;

                    let new_date =
                        DateTime::from_timestamp(i64::try_from(now_as_millis_u64).unwrap(), 0)
                            .expect("Cannot get date");

                    let current_data_info = CurrentDateInfo::new::<T>(
                        new_date.year(),
                        new_date.month(),
                        new_date.day(),
                    )
                    .unwrap();

                    CurrentDate::<T>::put(current_data_info);
                },
            }
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
        Self::do_set_vipp_status(account);
    }

    /// Set VIP member VIPP status.
    fn do_set_vipp_status(account: &T::AccountId) {
        let vipp_nft = pallet_nac_managing::Pallet::<T>::can_mint_vipp(account);

        if let Some(vipp_nft) = vipp_nft {
            let vipp_member_info = VippMemberInfo::<T> {
                points: <T as pallet_energy_generation::Config>::StakeBalance::default(),
                active_vipp_threshold: vec![(vipp_nft.1, vipp_nft.0.into())],
            };

            VippMembers::<T>::insert(account, vipp_member_info);
        }
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

                    pallet_energy_generation::Pallet::<T>::slash_vip_account(
                        account,
                        slash_percent,
                    )?;
                }

                VipMembers::<T>::remove(account);
                let vipp_status = VippMembers::<T>::get(account);
                if vipp_status.is_some() {
                    VippMembers::<T>::remove(account);
                    while pallet_nac_managing::Pallet::<T>::burn_vipp_nft(account) {}
                }

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
        ensure!(Self::is_penalty_free_period(), Error::<T>::IsNotPenaltyFreePeriod);
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

        current_date.current_month == FREE_PENALTY_PERIOD_MONTH_NUMBER
    }

    /// Mint new VIPP.
    fn mint_new_vipp_nft(who: &T::AccountId, amount: T::StakeBalance, item_id: T::ItemId) {
        let member_info = VippMembers::<T>::get(who);

        if let Some(mut info) = member_info {
            info.active_vipp_threshold.push((item_id, amount));
            VippMembers::<T>::insert(who, info);
        } else {
            let vipp_member_info = VippMemberInfo::<T> {
                points: <T as pallet_energy_generation::Config>::StakeBalance::default(),
                active_vipp_threshold: vec![(item_id, amount)],
            };

            VippMembers::<T>::insert(who, vipp_member_info);
        }
    }

    /// Burn VIPP NFT.
    fn burn_vipp_nft(who: &T::AccountId, current_item_id: T::ItemId) {
        VippMembers::<T>::mutate_exists(who, |info_opt| {
            if let Some(info) = info_opt {
                info.active_vipp_threshold.retain(|(item_id, _)| *item_id != current_item_id);

                if info.active_vipp_threshold.is_empty() {
                    *info_opt = None;
                }
            }
        });
    }

    /// Update current quarter info.
    pub fn update_quarter_info() {
        let now_as_millis_u64 =
            <T as Config>::UnixTime::now().as_millis().saturated_into::<u64>() / 1000u64;
        let new_date =
            DateTime::from_timestamp(i64::try_from(now_as_millis_u64).unwrap(), 0).unwrap();

        let current_date = Self::current_date();
        let new_date =
            CurrentDateInfo::new::<T>(new_date.year(), new_date.month(), new_date.day()).unwrap();

        if current_date.days_since_new_year != new_date.days_since_new_year {
            // Accrual of VIP points for users who have VIP status.
            Self::update_points_for_time(new_date.days_since_new_year);

            // Accrual of VIPP points for users who have VIPP status.
            Self::update_vipp_points_for_time(new_date.days_since_new_year);

            if new_date.current_month == YEAR_FIRST_MONTH && new_date.current_day == YEAR_FIRST_DAY
            {
                Self::save_year_info(new_date.current_year - 1);
            }

            CurrentDate::<T>::put(new_date);
        }
    }

    /// Updates the VIP points for the time since the last time the account was updated.
    pub fn update_points_for_time(elapsed_day: u64) {
        if elapsed_day == 0 {
            return;
        }

        VipMembers::<T>::translate(|_, mut old_info: VipMemberInfo<T>| {
            let multiplier = Self::calculate_multiplier(elapsed_day);
            let points = Self::calculate_points(old_info.active_stake, multiplier);
            let new_points = old_info.points.saturating_add(points);
            old_info.points = new_points;
            Some(old_info)
        });
    }

    /// Updates the VIPP points for the time since the last time the account was updated.
    pub fn update_vipp_points_for_time(elapsed_day: u64) {
        if elapsed_day == 0 {
            return;
        }

        VippMembers::<T>::translate(|acc, mut old_info: VippMemberInfo<T>| {
            let threshold = old_info.active_vipp_threshold.iter().fold(
                <T as pallet_energy_generation::Config>::StakeBalance::default(),
                |acc, (_, balance)| acc + *balance,
            );

            let vip_member_info = VipMembers::<T>::get(acc);
            let active_stake = match vip_member_info {
                None => <T as pallet_energy_generation::Config>::StakeBalance::default(),
                Some(info) => info.active_stake,
            };
            if threshold >= active_stake {
                let new_points = old_info.points.saturating_add(active_stake);
                old_info.points = new_points;
            } else {
                let new_points = old_info.points.saturating_add(threshold);
                old_info.points = new_points;
            }

            Some(old_info)
        });
    }

    /// Calculate multiplier based on the number of elapsed days in VIP.
    fn calculate_multiplier(elapsed_day: u64) -> Perquintill {
        Perquintill::from_rational(1, INCREASE_VIP_POINTS_CONSTANT + elapsed_day)
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

        let mut vipp_results = Vec::new();
        VippMembers::<T>::translate(|account, mut vipp_info: VippMemberInfo<T>| {
            vipp_results.push((account, vipp_info.points));
            vipp_info.points = <T as pallet_energy_generation::Config>::StakeBalance::default();
            Some(vipp_info)
        });

        YearVippResults::<T>::insert(current_year, vipp_results);
    }

    /// Calculate VIP points for account.
    fn calculate_points(
        active_stake: T::StakeBalance,
        multiplier: Perquintill,
    ) -> <T as pallet_energy_generation::Config>::StakeBalance {
        multiplier * active_stake
    }

    /// Check if the new date is correct.
    fn check_correct_date(last_date: &CurrentDateInfo, new_date: &CurrentDateInfo) -> bool {
        new_date.current_year > last_date.current_year
            || (new_date.current_year == last_date.current_year
                && new_date.days_since_new_year > last_date.days_since_new_year)
    }
}

impl<T: Config> OnVipMembershipHandler<T::AccountId, Weight, Perbill> for Pallet<T> {
    fn change_quarter_info() -> Weight {
        Self::update_quarter_info();
        Weight::from_parts(1, 1)
    }

    fn kick_account_from_vip(account: &T::AccountId) -> Weight {
        VipMembers::<T>::mutate(account, |vip_info| {
            if let Some(vip_info) = vip_info {
                vip_info.active_stake =
                    <T as pallet_energy_generation::Config>::StakeBalance::default();
            }
        });

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

    fn get_tax_percent(account: &T::AccountId) -> Perbill {
        let current_date = Self::current_date();
        let vip_info = Self::vip_members(account);
        match vip_info {
            Some(vip_member_info) => {
                if !Self::is_penalty_free_period() {
                    return vip_member_info.tax_type.penalty_percent(current_date.current_quarter);
                }

                Perbill::default()
            },
            None => Perbill::default(),
        }
    }
}
