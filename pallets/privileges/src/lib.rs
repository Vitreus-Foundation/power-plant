//! This pallet holds the VIP status of users.
//! TODO: add description to this pallet (Privilege-pallet)

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use frame_support::{RuntimeDebug, pallet_prelude::{BoundedVec, DispatchResult, Decode, TypeInfo, PhantomData}, traits::{tokens::nonfungibles_v2::Inspect, Currency, Get, Incrementable, LockableCurrency, UnixTime}, PalletId, weights::Weight};
use frame_support::traits::ExistenceRequirement::AllowDeath;
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
pub use pallet::*;
use parity_scale_codec::Encode;
use sp_runtime::SaturatedConversion;
use sp_runtime::traits::{AtLeast32BitUnsigned, Bounded, BlakeTwo256, Hash, StaticLookup, AccountIdConversion, Zero, Block, CheckedMul};
use sp_std::prelude::*;
use sp_arithmetic::*;
pub use weights::WeightInfo;
pub use contribution_info::*;
use pallet_energy_generation::OnVipMembershipHandler;
use chrono::{Datelike, NaiveDateTime};

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;
mod contribution_info;

pub mod weights;

const PALLET_ID: PalletId = PalletId(*b"Privileg");
const CDI_ACCOUNT_ID: PalletId = PalletId(*b"CoDiInAc");

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
    use chrono::NaiveDate;
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::tokens::Balance;
    use frame_support::traits::UnixTime;
    use frame_system::{ensure_root, ensure_signed};
    use sp_runtime::traits::{CheckedMul, CheckedShl};

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_nac_managing::Config + pallet_energy_generation::Config {
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
    pub type VipMembers<T: Config> = StorageMap<
        _,
        Twox64Concat,
        T::AccountId,
        VipMemberInfo
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
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account is not legit for VIP (isn't validator or cooperator).
        AccountNotLegitForVip,
        /// Account already has VIP status.
        AlreadyVipMember,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::become_vip_status())]
        pub fn become_vip_status(
            origin: OriginFor<T>,
            tax_type: PenaltyType,
        ) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;

            // Check VIP requirements.
            ensure!(Self::is_legit_for_vip(&who),Error::<T>::AccountNotLegitForVip);
            ensure!(VipMembers::<T>::contains_key(&who), Error::<T>::AlreadyVipMember);

            Self::do_set_user_privilege(who, tax_type)
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::set_quarter_revenue())]
        pub fn update_time(
            origin: OriginFor<T>,
        ) -> DispatchResult {
            Self::update_quarter_info();
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Set user privilege as VIP.
    fn do_set_user_privilege(
        account: T::AccountId,
        tax_type: PenaltyType
    ) -> DispatchResult {
        let now_as_millis_u64 =  <T as Config>::UnixTime::now().as_millis().saturated_into::<u64>();
        let vip_member_info = VipMemberInfo {
            start: now_as_millis_u64,
            tax_type,
            points: 0
        };

        VipMembers::<T>::insert(&account, vip_member_info);

        Self::deposit_event(Event::<T>::NewVipMember { account });

        Ok(())
    }

    /// Assesses whether a user qualifies as a VIP, and whether they are a validator or a cooperator within the network.
    fn is_legit_for_vip(account: &T::AccountId) -> bool {
        // Check account validator status.
        if pallet_energy_generation::Pallet::<T>::is_user_validator(account) {
            return true;
        }

        // Check account cooperator status.
        return if let Some(cooperation) = pallet_energy_generation::Pallet::<T>::cooperators(account) {
            !cooperation.targets.is_empty()
        } else {
            false
        }
    }

    /// Update current quarter info.
    pub fn update_quarter_info() {
        let now_as_millis_u64 =  <T as Config>::UnixTime::now().as_millis().saturated_into::<u64>() / 1000u64;

        let new_date = NaiveDateTime::from_timestamp(i64::try_from(now_as_millis_u64).unwrap(), 0).date();
        let current_date = Self::current_date();

        // Checking whether the day information needs to be updated.
        if current_date.current_day != new_date.day() {
            // Accrual of VIP points for users who have VIP status.
            Self::update_points_for_time();

            if new_date.month() == 1 && new_date.day() == 1 {
                Self::save_year_info();
            }

            CurrentDate::<T>::put(
                CurrentDateInfo::new(
                    new_date.year(),
                    new_date.month(),
                    new_date.day()
                )
            );
        }
    }

    /// Updates the points for the time since the last time the account was updated.
    pub fn update_points_for_time() {
        VipMembers::<T>::translate(|account: T::AccountId, mut old_info: VipMemberInfo| {
            let points = Self::calculate_points(&account);
            let new_points = old_info.points.saturating_add(points);
            old_info.points = new_points;
            Some(old_info)
        });
    }

    /// Save VIP year information to pay rewards.
    pub fn save_year_info() {

    }

    /// Calculate VIP points for account.
    fn calculate_points(account: &T::AccountId) -> u128 {
        let ledger = pallet_energy_generation::Pallet::<T>::ledger(account);

        0
    }
}

impl<T: Config>
    OnVipMembershipHandler<T::AccountId, Weight>
for Pallet<T> {
    fn change_quarter_info() -> Weight {
        let mut consumed_weight = Weight::from_parts(0, 0);
        Self::update_quarter_info();
        consumed_weight
    }

    fn kick_account_from_vip(account: &T::AccountId) -> Weight {
        todo!()
    }
}