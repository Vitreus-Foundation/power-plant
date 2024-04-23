//! This pallet holds the VIP status of users.
//! TODO: add description to this pallet (Privilege-pallet)

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use frame_support::{RuntimeDebug, pallet_prelude::{BoundedVec, DispatchResult, Decode, TypeInfo, PhantomData}, traits::{tokens::nonfungibles_v2::Inspect, Currency, Get, Incrementable, LockableCurrency}, PalletId};
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

// Wrapper for `T::MAX_ACCOUNT_VIP_SLOTS` to satisfy `trait Get`.
pub struct MaxAccountVipSlotsGet<T>(PhantomData<T>);
impl<T: Config> Get<u32> for MaxAccountVipSlotsGet<T> {
    fn get() -> u32 {
        T::MAX_ACCOUNT_VIP_SLOTS
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::tokens::Balance;
    use frame_system::{ensure_root, ensure_signed};
    use sp_runtime::traits::{CheckedMul, CheckedShl};

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_nfts::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency trait.
        type Currency: LockableCurrency<Self::AccountId>;

        /// Block number into a quarter.
        #[pallet::constant]
        type BlocksPerDays: Get<BlockNumberFor<Self>>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;

        /// Maximum number of VIP slots in a VIP pool an account may have at a given moment.
        const MAX_ACCOUNT_VIP_SLOTS: u32;
    }

    #[pallet::extra_constants]
    impl<T: Config> Pallet<T> {
        #[pallet::constant_name(MaxAccountVipSlots)]
        fn max_account_vip_slots() -> u32 {
            T::MAX_ACCOUNT_VIP_SLOTS
        }
    }

    /// Account payment into VIP pool.
    #[pallet::storage]
    #[pallet::getter(fn get_account_payment)]
    pub type AccountsPayment<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<ContributionInfo<BalanceOf<T>, BlockNumberFor<T>>, MaxAccountVipSlotsGet<T>>
    >;

    /// Accounts payable
    #[pallet::storage]
    #[pallet::getter(fn get_vip_pool)]
    pub type AccountsPayable<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (T::AccountId, u32, u8),
        BalanceOf<T>
    >;

    #[pallet::storage]
    #[pallet::getter(fn total_contributors)]
    pub type TotalContributors<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::storage]
    pub type CurrentQuarterInfo<T: Config> = StorageValue<_, CurrentQuarter<T>, ValueQuery>;

    #[pallet::storage]
    pub type QuarterResults<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (u32, u8),
        QuarterResult<T>
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        TokensTransferredToVipPool { account: T::AccountId, amount: BalanceOf<T> },
        VipStatusReceived { account: T::AccountId, contribution_info: ContributionInfo<BalanceOf<T>, BlockNumberFor<T>> /*amount: T::Balance, start_block: BlockNumberFor<T>*/ },
        RewardClaimed { account: T::AccountId, reward_info: BalanceOf<T> },
        PoolBalance { amount: BalanceOf<T> },
        QuarterRevenueSet { year: u32, quarter: u8, amount: BalanceOf<T> },
        TestEventS { vip_pool: u32, revenue: u32, total_term: u32, final_reward: u32, participation: u32, start_participation: u32 }
    }

    #[pallet::error]
    pub enum Error<T> {
        InsufficientError,
        InvalidInputError,
        NoAssociatedClaim,
        NoAssociatedPayment,
        QuarterlyRewardAlreadyClaimed,
        NoQuarterForClaiming,
        VipStatusIsNotCorrect,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            // Check quarter and run the action
            if _now == CurrentQuarterInfo::<T>::get().end_block {
                let _ = Self::calculate_quarterly_results();
            }

            T::DbWeight::get().reads(1)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::become_vip_status())]
        pub fn become_vip_status(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
            tax_type: PenaltyType,
        ) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;


            Self::do_set_user_privilege(who, amount, tax_type)
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult {
            let payee = ensure_signed(origin)?;

            Self::do_claim_reward(payee)
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::set_quarter_revenue())]
        pub fn set_quarter_revenue(origin: OriginFor<T>, year: u32, quarter: u8, amount: BalanceOf<T>) -> DispatchResult {
            ensure_root(origin)?;

            Self::set_revenue(year, quarter, amount)
        }

        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::check_pool_balance())]
        pub fn check_avenue_balance(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;

            let balance = Self::revenue_pool_balance();
            Self::deposit_event(Event::<T>::PoolBalance {
                amount: balance
            });

            Ok(())
        }

        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::check_pool_balance())]
        pub fn check_pool_balance(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;

            let balance = Self::pool_balance();
            Self::deposit_event(Event::<T>::PoolBalance {
                amount: balance
            });

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The account ID that holds the funds (VIP pool).
    pub fn vip_pool_account_id() -> T::AccountId {
        PALLET_ID.into_account_truncating()
    }

    /// The account ID that holds the revenue.
    pub fn revenue_account_id() -> T::AccountId {
        CDI_ACCOUNT_ID.into_account_truncating()
    }

    /// The VIP pool balance.
    pub fn pool_balance() -> BalanceOf<T> {
        <T as Config>::Currency::free_balance(&Self::vip_pool_account_id())
    }

    /// The Revenue pool balance
    pub fn revenue_pool_balance() -> BalanceOf<T> {
        <T as Config>::Currency::free_balance(&Self::revenue_account_id())
    }

    /// Set user privilege as VIP.
    pub fn do_set_user_privilege(
        account: T::AccountId,
        amount: BalanceOf<T>,
        tax_type: PenaltyType
    ) -> DispatchResult {
        let starting_block = <frame_system::Pallet<T>>::block_number();
        let contribution_info = ContributionInfo::new(amount.clone(), starting_block.clone(), tax_type);

        <T as Config>::Currency::transfer(
            &account,
            &PALLET_ID.into_account_truncating(),
            amount.clone(),
            AllowDeath,
        )?;

        AccountsPayment::<T>::try_append(&account, &contribution_info)
            .expect("Too many vesting VIP slots at genesis.");

        Self::deposit_event(Event::<T>::TokensTransferredToVipPool {
            account: account.clone(),
            amount: amount.clone()
        });

        TotalContributors::<T>::set(Self::total_contributors() + 1);

        Self::deposit_event(Event::<T>::VipStatusReceived {
            account: account.clone(),
            contribution_info
        });

        Ok(())
    }

    /// Set revenue.
    pub fn set_revenue(year: u32, quarter: u8, amount: BalanceOf<T>) -> DispatchResult {
        let quarter_info = QuarterResults::<T>::get((&year, &quarter));
        match quarter_info {
            Some(mut info) => {
                info.avenue = amount;
                QuarterResults::<T>::insert((&year, &quarter), info);
            },
            None => return Err(Error::<T>::InvalidInputError)?
        }

        Self::deposit_event(Event::<T>::QuarterRevenueSet {
            year,
            quarter,
            amount
        });

        Ok(())
    }

    /// Claim a reward
    pub fn do_claim_reward(account: T::AccountId) -> DispatchResult {
        let period = Self::check_claim_period()?;

        if AccountsPayable::<T>::get((&account, period.quarter_info.current_year, period.quarter_info.current_quarter)).is_some() {
            return Err(Error::<T>::QuarterlyRewardAlreadyClaimed)?
        }

        let contribution_info = AccountsPayment::<T>::get(&account)
            .ok_or(Error::<T>::NoAssociatedPayment)?;

        let vip_pool = period.pool_balance;
        let revenue = period.avenue;
        let total_term = period.quarter_info.end_block - period.quarter_info.year_first_block;
        let final_reward: BalanceOf<T> = contribution_info.iter().fold(Default::default(), |acc, &item| {
            // Formula: Revenue * [ (UserTokens / Wallet) * Participation (start/end)  ]

            let participation = item.locked();
            let start_participation = item.starting_block;

            let token_participation = Perbill::from_rational(participation, vip_pool);
            let time_participation = Perbill::from_rational(total_term - start_participation, total_term);

            let result = token_participation * time_participation * revenue;

            result
        });

        <T as Config>::Currency::transfer(
            &Self::revenue_account_id(),
            &account,
            final_reward.into(),
            AllowDeath,
        )?;

        AccountsPayable::<T>::insert(
            (account.clone(), period.quarter_info.current_year, period.quarter_info.current_quarter),
            final_reward.clone()
        );

        Self::deposit_event(Event::<T>::RewardClaimed {
            account,
            reward_info: final_reward.into()
        });

        Ok(())
    }

    /// Check user VIP status.
    pub fn has_account_vip_status(account: T::AccountId) -> bool {
        true
    }

    /// Saving quarterly results.
    pub fn calculate_quarterly_results() -> DispatchResult {
        let current_quarter_info = CurrentQuarterInfo::<T>::get();

        let pool_balance = Self::pool_balance();
        QuarterResults::<T>::insert(
            (&current_quarter_info.current_year, &current_quarter_info.current_quarter),
            QuarterResult::new(current_quarter_info.clone(), pool_balance));

        let mut new_quarter_info = current_quarter_info.clone();

        if current_quarter_info.current_quarter == 4 {
            new_quarter_info.current_year += 1;
            new_quarter_info.current_quarter = 1;
            new_quarter_info.year_first_block = current_quarter_info.end_block;
        } else {
            new_quarter_info.current_quarter += 1;
        }

        let days: BlockNumberFor<T> = Self::days_in_quarter(new_quarter_info.current_year, new_quarter_info.current_quarter).into();
        //new_quarter_info.end_block += days.checked_mul(&T::BlocksPerDays::get()).expect("Multiplication overflow: days is too large.");
        new_quarter_info.end_block += 15_u32.into();

        CurrentQuarterInfo::<T>::set(new_quarter_info);
        AccountsPayable::<T>::remove_all(None);

        let claim_period = Self::check_claim_period()?;
        <T as Config>::Currency::make_free_balance_be(
            &Self::revenue_account_id(),
            claim_period.avenue.into(),
        );

        Ok(())
    }

    /// Calculate days in quarter.
    fn days_in_quarter(year: u32, quarter: u8) -> u32 {
        let mut days_per_month: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

        if year & 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            days_per_month[1] = 29;
        }

        let start_month = ((quarter - 1) * 3) as usize;
        let end_month = start_month + 2;

        days_per_month[start_month..=end_month].iter().sum()
    }

    /// Check claim period
    fn check_claim_period() -> Result<QuarterResult<T>, Error<T>> {
        let current_quarter = CurrentQuarterInfo::<T>::get();

        let mut previous_quarter = current_quarter.current_quarter;
        let mut previous_year = current_quarter.current_year;
        if previous_quarter == 2 {
            previous_quarter = 4;
            previous_year -= 1;
        } else if previous_quarter == 1 {
            previous_quarter = 3;
            previous_year -= 1;
        }

        QuarterResults::<T>::get((previous_year, previous_quarter))
                .ok_or(Error::<T>::NoQuarterForClaiming)
    }
}