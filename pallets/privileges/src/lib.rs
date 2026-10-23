//! VIP/VIPP membership, completed UTC days and annual reward pools.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use chrono::{DateTime, Datelike, Days, NaiveDate};
pub use contribution_info::*;
use frame_support::pallet_prelude::BuildGenesisConfig;
use frame_support::{
    ensure,
    pallet_prelude::{Decode, DispatchResult, TypeInfo},
    traits::{Currency, Get, UnixTime},
    weights::Weight,
    PalletId,
};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use pallet_energy_generation::OnVipMembershipHandler;
use parity_scale_codec::Encode;
use sp_arithmetic::traits::{Saturating, Zero};
use sp_arithmetic::Perquintill;
use sp_runtime::{traits::AccountIdConversion, Perbill, SaturatedConversion};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

mod accounting;
mod breach_period;
mod contribution_info;

pub mod migration;

pub mod weights;

const INCREASE_VIP_POINTS_CONSTANT: u64 = 50;

const MAX_UPDATE_DAYS: u32 = 366;

const FREE_PENALTY_PERIOD_MONTH_NUMBER: u32 = 1;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type PointsOf<T> = <T as pallet_energy_generation::Config>::StakeBalance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::UnixTime;
    use frame_system::pallet_prelude::BlockNumberFor;
    use frame_system::{ensure_root, ensure_signed};

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_energy_generation::Config + pallet_nac_managing::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency trait.
        type Currency: Currency<Self::AccountId, Balance: From<u64>>;

        /// Time used for computing year, quarter durations.
        type UnixTime: UnixTime;

        /// The pallet id.
        #[pallet::constant]
        type PalletId: Get<PalletId>;

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
    #[pallet::getter(fn vip_points)]
    pub type VipPoints<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        u32,
        Twox64Concat,
        T::AccountId,
        <T as pallet_energy_generation::Config>::StakeBalance,
    >;

    #[pallet::storage]
    #[pallet::getter(fn vipp_points)]
    pub type VippPoints<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        u32,
        Twox64Concat,
        T::AccountId,
        <T as pallet_energy_generation::Config>::StakeBalance,
    >;

    #[pallet::storage]
    #[pallet::getter(fn rewards)]
    pub type Rewards<T: Config> = StorageMap<_, Twox64Concat, u32, RewardsInfo<T>>;

    #[pallet::storage]
    #[pallet::getter(fn current_date)]
    pub type CurrentDate<T: Config> = StorageValue<_, CurrentDateInfo, ValueQuery>;

    /// Block whose timestamp has been applied; unset only before first use.
    #[pallet::storage]
    pub type TimestampSeenAtBlock<T: Config> = StorageValue<_, BlockNumberFor<T>>;

    /// First stake cached before this block's timestamp, for UTC rollover.
    #[pallet::storage]
    pub type PreTimestampStake<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, (BlockNumberFor<T>, PointsOf<T>, bool)>;

    /// Year of the first unresolved balance breach, retained across rollover.
    #[pallet::storage]
    pub type VippBreachYear<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u32>;

    /// Breaches during initialization whose actual UTC year awaits the inherent.
    #[pallet::storage]
    pub type PreTimestampVippBreaches<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, ConstU32<1024>>, ValueQuery>;

    /// Explicit historical decision for a deferred last-NFT loss. Inner None
    /// means reviewed preservation of all prior years, not absence of approval.
    #[pallet::storage]
    pub type VippForfeitureApproval<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, Option<u32>>;

    /// Automatic accrual is paused until the stale interval is reviewed.
    #[pallet::storage]
    pub type AccrualReviewRequired<T: Config> = StorageValue<_, bool, ValueQuery>;

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
        /// Rewards have been paid to the specified user.
        RewardsPaid {
            /// The account that received the rewards.
            account: T::AccountId,
            /// The amount of VIP rewards.
            vip_rewards: BalanceOf<T>,
            /// The amount of VIPP rewards.
            vipp_rewards: BalanceOf<T>,
        },
        /// VIP points were forcibly updated for a specific account.
        VipPointsForced {
            /// The year for which the points were updated.
            year: u32,
            /// The account whose VIP points were changed.
            account: T::AccountId,
            /// The new VIP points value assigned to the account.
            points: PointsOf<T>,
        },
        /// VIPP points were forcibly updated for a specific account.
        VippPointsForced {
            /// The year for which the points were updated.
            year: u32,
            /// The account whose VIPP points were changed.
            account: T::AccountId,
            /// The new VIPP points value assigned to the account.
            points: PointsOf<T>,
        },
        /// Governance reviewed which, if any, historical VIPP year was forfeited.
        VippForfeitureApproved {
            /// Account with an unresolved balance breach.
            account: T::AccountId,
            /// Historical year to clear, or None to preserve prior years.
            year: Option<u32>,
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
        /// No rewards can be claimed by given account.
        NoRewardsForAccount,
        /// Rewards have already been set for the specified year.
        YearRewardsAlreadySet,
        /// Error indicating insufficient tokens for a claim.
        NotEnoughTokensForClaim,
        /// Account points exceed the frozen pool total or have no valid denominator.
        InvalidRewardPoints,
        /// Accounting is awaiting bounded catch-up or explicit state review.
        AccountingPaused,
        /// A bounded member or NFT processing limit was exceeded.
        AccountingCapacityExceeded,
        /// An existing historical record must never be overwritten by catch-up.
        HistoricalPointsAlreadyExist,
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
                Self::do_set_user_privilege(&who, tax_type)?;
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
            ensure!(!accounting::paused::<T>(), Error::<T>::AccountingPaused);
            let current_date = Self::current_date();
            let new_date = CurrentDateInfo::new::<T>(new_date_year, new_date_month, new_date_day)?;

            if !Self::check_correct_date(&current_date, &new_date) {
                return Err(Error::<T>::NotCorrectDate.into());
            }

            let mut limit = current_date;
            limit.add_days::<T>(accounting::MAX_AUTO_DAYS.into())?;
            ensure!(!Self::check_correct_date(&limit, &new_date), Error::<T>::NotCorrectDate);
            Self::advance_date(&new_date)
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

        /// Force set VIP points.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::force_set_vip_points())]
        pub fn force_set_vip_points(
            origin: OriginFor<T>,
            year: u32,
            account: T::AccountId,
            points: PointsOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            accounting::ensure_ready::<T>()?;

            let current_year = Self::current_date().current_year as u32;

            ensure!(year <= current_year, Error::<T>::NotCorrectDate);
            ensure!(Self::are_rewards_not_set(year), Error::<T>::YearRewardsAlreadySet);

            if year == current_year {
                VipMembers::<T>::try_mutate(&account, |info| {
                    info.as_mut()
                        .ok_or(Error::<T>::AccountHasNotVipStatus)
                        .map(|info| info.points = points)
                })?;
            } else {
                VipPoints::<T>::insert(year, &account, points);
            }

            Self::deposit_event(Event::<T>::VipPointsForced { year, account, points });

            Ok(())
        }

        /// Force set VIPP points.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::force_set_vipp_points())]
        pub fn force_set_vipp_points(
            origin: OriginFor<T>,
            year: u32,
            account: T::AccountId,
            points: PointsOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            accounting::ensure_ready::<T>()?;

            let current_year = Self::current_date().current_year as u32;

            ensure!(year <= current_year, Error::<T>::NotCorrectDate);
            ensure!(Self::are_rewards_not_set(year), Error::<T>::YearRewardsAlreadySet);

            if year == current_year {
                VippMembers::<T>::try_mutate(&account, |info| {
                    info.as_mut()
                        .ok_or(Error::<T>::AccountHasNotVipStatus)
                        .map(|info| info.points = points)
                })?;
            } else {
                VippPoints::<T>::insert(year, &account, points);
            }

            Self::deposit_event(Event::<T>::VippPointsForced { year, account, points });

            Ok(())
        }

        /// Set VIP and VIPP rewards.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::set_rewards())]
        pub fn set_rewards(
            origin: OriginFor<T>,
            year: u32,
            vip_rewards: BalanceOf<T>,
            vipp_rewards: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            accounting::ensure_ready::<T>()?;

            let current_year = Self::current_date().current_year as u32;

            ensure!(year < current_year, Error::<T>::NotCorrectDate);
            ensure!(Self::are_rewards_not_set(year), Error::<T>::YearRewardsAlreadySet);

            ensure!(
                pallet_nac_managing::VippAccountingQuarantine::<T>::iter_keys().next().is_none(),
                Error::<T>::AccountingPaused
            );
            let vip: Vec<_> = VipPoints::<T>::iter_prefix_values(year)
                .take(accounting::MAX_MEMBERS + 1)
                .collect();
            let vipp: Vec<_> = VippPoints::<T>::iter_prefix_values(year)
                .take(accounting::MAX_MEMBERS + 1)
                .collect();
            ensure!(
                vip.len() <= accounting::MAX_MEMBERS && vipp.len() <= accounting::MAX_MEMBERS,
                Error::<T>::AccountingCapacityExceeded
            );
            let vip_points = vip
                .into_iter()
                .fold(PointsOf::<T>::zero(), |total, points| total.saturating_add(points));
            let vipp_points = vipp
                .into_iter()
                .fold(PointsOf::<T>::zero(), |total, points| total.saturating_add(points));

            Rewards::<T>::insert(
                year,
                RewardsInfo { vip_points, vip_rewards, vipp_points, vipp_rewards },
            );

            Ok(())
        }

        /// Make a claim to collect VIP/VIPP rewards.
        ///
        /// Refunds the transaction fees upon successful execution.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
        pub fn claim_rewards(origin: OriginFor<T>, year: u32) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let rewards = Self::rewards(year).ok_or(Error::<T>::NoRewardsForAccount)?;

            let vip_points = VipPoints::<T>::take(year, &who);
            let vipp_points = VippPoints::<T>::take(year, &who);
            ensure!(vip_points.is_some() || vipp_points.is_some(), Error::<T>::NoRewardsForAccount);
            let vip_rewards = Self::reward_share(
                vip_points.unwrap_or_default(),
                rewards.vip_points,
                rewards.vip_rewards,
            )?;
            let vipp_rewards = Self::reward_share(
                vipp_points.unwrap_or_default(),
                rewards.vipp_points,
                rewards.vipp_rewards,
            )?;

            <T as Config>::Currency::transfer(
                &Self::account_id(),
                &who,
                vip_rewards.saturating_add(vipp_rewards),
                frame_support::traits::ExistenceRequirement::AllowDeath,
            )
            .map_err(|_| Error::<T>::NotEnoughTokensForClaim)?;

            Self::deposit_event(Event::<T>::RewardsPaid {
                account: who,
                vip_rewards,
                vipp_rewards,
            });

            Ok(Pays::No.into())
        }
        /// Approve a reviewed historical forfeiture decision before repairing a
        /// breach that spans years. This call itself does not change any points.
        #[pallet::call_index(16)]
        #[pallet::weight(T::DbWeight::get().reads_writes(8, 1))]
        pub fn approve_vipp_forfeiture(
            origin: OriginFor<T>,
            account: T::AccountId,
            year: Option<u32>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let first = VippBreachYear::<T>::get(&account).ok_or(Error::<T>::AccountingPaused)?;
            ensure!(
                pallet_nac_managing::Pallet::<T>::is_vipp_quarantined(&account),
                Error::<T>::AccountingPaused
            );
            let current = Self::current_date().current_year as u32;
            ensure!(first < current, Error::<T>::NotCorrectDate);
            if let Some(year) = year {
                ensure!(year >= first && year < current, Error::<T>::NotCorrectDate);
                ensure!(Self::are_rewards_not_set(year), Error::<T>::YearRewardsAlreadySet);
            }
            VippForfeitureApproval::<T>::insert(&account, year);
            Self::deposit_event(Event::<T>::VippForfeitureApproved { account, year });
            Ok(())
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
    /// Calculate a share without turning a zero denominator into a full pool payout.
    fn reward_share(
        points: PointsOf<T>,
        total_points: PointsOf<T>,
        rewards: BalanceOf<T>,
    ) -> Result<BalanceOf<T>, sp_runtime::DispatchError> {
        if points.is_zero() {
            return Ok(BalanceOf::<T>::zero());
        }
        ensure!(!total_points.is_zero() && points <= total_points, Error::<T>::InvalidRewardPoints);
        Ok(Perquintill::from_rational(points, total_points) * rewards)
    }

    /// The account ID that holds the rewards.
    pub fn account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }

    /// Set user privilege as VIP.
    #[frame_support::transactional]
    fn do_set_user_privilege(account: &T::AccountId, tax_type: PenaltyType) -> DispatchResult {
        // Settle completed days before inserting the new member.
        accounting::ensure_ready::<T>()?;
        let now_as_millis_u64 = <T as Config>::UnixTime::now().as_millis().saturated_into::<u64>();
        accounting::ensure_member_capacity::<T>(account)?;
        let active_stake = pallet_energy_generation::Pallet::<T>::get_active_stake(account);

        let vip_member_info = VipMemberInfo {
            start: now_as_millis_u64,
            tax_type,
            points: <T as pallet_energy_generation::Config>::StakeBalance::default(),
            active_stake,
        };

        VipMembers::<T>::insert(account, vip_member_info);
        Self::do_set_vipp_status(account)
    }

    /// Initialize a new VIP membership from the NFTs actually owned by the account.
    fn do_set_vipp_status(account: &T::AccountId) -> DispatchResult {
        if pallet_nac_managing::Pallet::<T>::is_vipp_inactive(account) {
            VippMembers::<T>::remove(account);
            return Ok(());
        }
        if let Some((amount, item_id)) = pallet_nac_managing::Pallet::<T>::can_mint_vipp(account)? {
            Self::register_vipp_nft(account, amount.into(), item_id)?;
        }
        Self::sync_vipp_membership(account, true)
    }

    /// Register an NFT after its mint or accounted transfer has succeeded.
    fn register_vipp_nft(
        account: &T::AccountId,
        amount: T::StakeBalance,
        item_id: T::ItemId,
    ) -> DispatchResult {
        if !VipMembers::<T>::contains_key(account)
            || pallet_nac_managing::Pallet::<T>::is_vipp_inactive(account)
        {
            return Ok(());
        }
        accounting::ensure_vipp_capacity::<T>(account)?;
        VippMembers::<T>::try_mutate(account, |member| -> DispatchResult {
            let info = member.get_or_insert(VippMemberInfo::<T> {
                points: Default::default(),
                active_vipp_threshold: Vec::new(),
            });
            if let Some((_, value)) =
                info.active_vipp_threshold.iter_mut().find(|(id, _)| *id == item_id)
            {
                *value = amount;
            } else {
                ensure!(
                    info.active_vipp_threshold.len()
                        < <T as pallet_nac_managing::Config>::MaxVippNftsPerAccount::get() as usize,
                    pallet_nac_managing::Error::<T>::TooManyVippNfts
                );
                info.active_vipp_threshold.push((item_id, amount));
            }
            Ok(())
        })?;
        Self::sync_vipp_membership(account, false)
    }

    /// Refresh registered NFT caps without activating unaccounted ownership.
    fn sync_vipp_membership(account: &T::AccountId, reset_points: bool) -> DispatchResult {
        if !VipMembers::<T>::contains_key(account)
            || pallet_nac_managing::Pallet::<T>::is_vipp_inactive(account)
        {
            VippMembers::<T>::remove(account);
            return Ok(());
        }
        let previous = VippMembers::<T>::get(account);
        let retain_points = previous.is_some();
        let registered = previous.map(|member| member.active_vipp_threshold).unwrap_or_default();
        let owned = pallet_nac_managing::Pallet::<T>::owned_vipp_claims(account)?;
        let has_owned_claim = owned.iter().any(|(_, amount)| !amount.is_zero());
        // Ownership alone does not prove that the previous owner's entitlement
        // and the recipient's NAC threshold were updated during a transfer.
        let active_vipp_threshold: Vec<_> = owned
            .into_iter()
            .filter(|(id, amount)| {
                !amount.is_zero() && registered.iter().any(|(registered, _)| registered == id)
            })
            .map(|(item_id, amount)| (item_id, (Perbill::from_percent(95) * amount).into()))
            .collect();
        // Keep accumulated points while unresolved positive NFTs still exist,
        // but give unregistered items no accrual cap until backing is reconciled.
        if active_vipp_threshold.is_empty() && !(retain_points && has_owned_claim) {
            VippMembers::<T>::remove(account);
        } else {
            accounting::ensure_vipp_capacity::<T>(account)?;
            VippMembers::<T>::mutate(account, |member| {
                let points = if reset_points {
                    Default::default()
                } else {
                    member.as_ref().map(|info| info.points).unwrap_or_default()
                };
                *member = Some(VippMemberInfo::<T> { points, active_vipp_threshold });
            });
        }
        Ok(())
    }

    /// Exit VIP.
    #[frame_support::transactional]
    pub fn do_exit_vip(account: &T::AccountId) -> DispatchResult {
        accounting::ensure_ready::<T>()?;
        ensure!(
            !pallet_nac_managing::Pallet::<T>::is_vipp_quarantined(account),
            Error::<T>::AccountingPaused
        );
        // Legacy storage has no hard fan-out bound. Refuse an oversized exit
        // before penalties can trigger a validator-wide cache refresh.
        let key = pallet_energy_generation::Collaborations::<T>::hashed_key_for(account);
        let mut prefix = [0u8; 5];
        if sp_io::storage::read(&key, &mut prefix, 0).is_some() {
            let count = parity_scale_codec::Compact::<u32>::decode(&mut prefix.as_slice())
                .map_err(|_| Error::<T>::AccountingCapacityExceeded)?
                .0;
            ensure!(
                count <= pallet_energy_generation::MAX_VALIDATOR_COLLABORATORS,
                Error::<T>::AccountingCapacityExceeded
            );
        }
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
                PreTimestampStake::<T>::remove(account);
                let had_vipp = VippMembers::<T>::take(account).is_some();
                if had_vipp
                    || !pallet_nac_managing::Pallet::<T>::owned_vipp_claims(account)?.is_empty()
                {
                    while pallet_nac_managing::Pallet::<T>::burn_vipp_nft(account)? {}
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
        accounting::ensure_ready::<T>()?;
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

    fn are_rewards_not_set(year: u32) -> bool {
        !Rewards::<T>::contains_key(year)
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
    fn effective_penalty_date() -> CurrentDateInfo {
        let accounted = Self::current_date();
        match accounting::today::<T>() {
            Ok(today) if Self::check_correct_date(&accounted, &today) => today,
            _ => accounted,
        }
    }

    fn is_penalty_free_period() -> bool {
        let current_date = Self::effective_penalty_date();

        current_date.current_month == FREE_PENALTY_PERIOD_MONTH_NUMBER
    }

    /// Settle up to seven closed UTC days. Longer interruptions require an
    /// explicitly reviewed frozen-source job; partial automatic catch-up is unsafe.
    pub fn update_quarter_info() {
        if !CurrentDate::<T>::exists() || accounting::paused::<T>() {
            return;
        }
        let Ok(new_date) = accounting::today::<T>() else {
            return;
        };
        let current = Self::current_date();
        let mut limit = current.clone();
        if limit.add_days::<T>(accounting::MAX_AUTO_DAYS.into()).is_err()
            || Self::check_correct_date(&limit, &new_date)
        {
            return;
        }
        if Self::check_correct_date(&current, &new_date) {
            if let Err(error) = Self::advance_date(&new_date) {
                accounting::AccrualReviewRequired::<T>::put(true);
                log::error!(target: "runtime::privileges", "VIP date update failed: {:?}", error);
            }
        }
    }

    fn advance_date(new_date: &CurrentDateInfo) -> DispatchResult {
        accounting::advance::<T>(new_date)
    }

    /// Calculate multiplier based on the number of elapsed days in VIP.
    fn calculate_multiplier(elapsed_day: u64) -> Perquintill {
        Perquintill::from_rational(1, INCREASE_VIP_POINTS_CONSTANT + elapsed_day)
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
        T::DbWeight::get().reads(6)
    }

    fn kick_account_from_vip(account: &T::AccountId) -> Weight {
        Self::update_quarter_info();
        accounting::before_stake_change::<T>(account);
        VipMembers::<T>::mutate(account, |vip_info| {
            if let Some(vip_info) = vip_info {
                vip_info.active_stake =
                    <T as pallet_energy_generation::Config>::StakeBalance::default();
            }
        });

        T::DbWeight::get().reads_writes(10, 2)
    }

    fn update_active_stake(account: &T::AccountId) -> Weight {
        // The cache still contains the pre-mutation stake at this point.
        Self::update_quarter_info();
        accounting::before_stake_change::<T>(account);
        VipMembers::<T>::mutate(account, |vip_info| {
            if let Some(vip_info) = vip_info {
                vip_info.active_stake =
                    pallet_energy_generation::Pallet::<T>::get_active_stake(account);
            }
        });
        T::DbWeight::get().reads_writes(270, 2)
    }

    fn remove_validator_delegation(account: &T::AccountId, validator: &T::AccountId) -> Weight {
        Self::update_quarter_info();
        accounting::before_stake_change::<T>(account);
        if !pallet_energy_generation::Pallet::<T>::is_user_validator(account) {
            let removed = pallet_energy_generation::Pallet::<T>::cooperators(account)
                .and_then(|cooperations| cooperations.targets.get(validator).copied())
                .unwrap_or_default();
            VipMembers::<T>::mutate(account, |member| {
                if let Some(member) = member {
                    member.active_stake = member.active_stake.saturating_sub(removed);
                }
            });
        }
        T::DbWeight::get().reads_writes(10, 2)
    }

    fn get_tax_percent(account: &T::AccountId) -> Perbill {
        Self::update_quarter_info();
        let current_date = Self::effective_penalty_date();
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

impl<T: Config, Moment> frame_support::traits::OnTimestampSet<Moment> for Pallet<T> {
    fn on_timestamp_set(_: Moment) {
        frame_system::Pallet::<T>::register_extra_weight_unchecked(
            T::DbWeight::get().reads_writes(6, 1),
            frame_support::dispatch::DispatchClass::Mandatory,
        );
        breach_period::resolve_timestamp::<T>();
        // Timestamp is an inherent, applied before user extrinsics. Charge the
        // bounded scan before dispatch admission, only on a date transition.
        if CurrentDate::<T>::exists() && !accounting::paused::<T>() {
            if let Ok(today) = accounting::today::<T>() {
                let mut limit = Self::current_date();
                if Self::check_correct_date(&limit, &today)
                    && limit.add_days::<T>(accounting::MAX_AUTO_DAYS.into()).is_ok()
                    && !Self::check_correct_date(&limit, &today)
                {
                    frame_system::Pallet::<T>::register_extra_weight_unchecked(
                        <T as Config>::WeightInfo::accounting(2048, 7, 1),
                        frame_support::dispatch::DispatchClass::Mandatory,
                    );
                    Self::update_quarter_info();
                }
            }
        }
        TimestampSeenAtBlock::<T>::put(frame_system::Pallet::<T>::block_number());
    }
}
