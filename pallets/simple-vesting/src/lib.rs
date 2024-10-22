//! The pallet provides a simple vesting mechanism using "Reserved" balance.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)]

use frame_support::dispatch::DispatchResult;
use frame_support::traits::{
    Currency, ExistenceRequirement, NamedReservableCurrency, OnUnbalanced,
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, Bounded, Convert, One, Saturating, Zero},
    RuntimeDebug,
};
use sp_std::vec::Vec;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

const VESTING_ID: [u8; 8] = *b"vesting ";

/// Struct to encode the vesting schedule of an individual account.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct VestingInfo<Balance, BlockNumber> {
    /// Locked amount at genesis.
    locked: Balance,
    /// Amount that gets unlocked every block after `starting_block`.
    per_block: Balance,
    /// Starting block for unlocking(vesting).
    starting_block: BlockNumber,
}

impl<Balance, BlockNumber> VestingInfo<Balance, BlockNumber>
where
    Balance: AtLeast32BitUnsigned + Copy,
    BlockNumber: AtLeast32BitUnsigned + Copy + Bounded,
{
    /// Instantiate a new `VestingInfo`.
    pub fn new(
        locked: Balance,
        per_block: Balance,
        starting_block: BlockNumber,
    ) -> VestingInfo<Balance, BlockNumber> {
        VestingInfo { locked, per_block, starting_block }
    }

    /// Validate parameters for `VestingInfo`.
    pub fn is_valid(&self) -> bool {
        !self.locked.is_zero() && !self.per_block.is_zero()
    }

    /// Amount locked at block `n`.
    pub fn locked_at<BlockNumberToBalance: Convert<BlockNumber, Balance>>(
        &self,
        n: BlockNumber,
    ) -> Balance {
        // Number of blocks that count toward vesting;
        // saturating to 0 when n < starting_block.
        let vested_block_count = n.saturating_sub(self.starting_block);
        let vested_block_count = BlockNumberToBalance::convert(vested_block_count);
        // Return amount that is still locked in vesting.
        vested_block_count
            .checked_mul(&self.per_block.max(One::one()))
            .map(|to_unlock| self.locked.saturating_sub(to_unlock))
            .unwrap_or(Zero::zero())
    }
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use super::*;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The currency trait.
        type Currency: NamedReservableCurrency<Self::AccountId, ReserveIdentifier = [u8; 8]>;
        /// Convert the block number into a balance.
        type BlockNumberToBalance: Convert<BlockNumberFor<Self>, BalanceOf<Self>>;
        /// Handler for the unbalanced reduction when removing vesting.
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
    }

    /// Information regarding the vesting of a given account.
    #[pallet::storage]
    #[pallet::getter(fn vesting)]
    pub type Vesting<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, VestingInfo<BalanceOf<T>, BlockNumberFor<T>>>;

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// Vesting schedule: who, begin, length, free balance
        pub vesting: Vec<(T::AccountId, BlockNumberFor<T>, BlockNumberFor<T>, BalanceOf<T>)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            // Generate initial vesting configuration
            // * who - Account which we are generating vesting configuration for
            // * begin - Block when the account will start to vest
            // * length - Number of blocks from `begin` until fully vested
            // * liquid - Number of units which can be spent before vesting begins
            for &(ref who, begin, length, liquid) in self.vesting.iter() {
                let balance = T::Currency::free_balance(who);
                assert!(!balance.is_zero(), "Currencies must be init'd before vesting");
                // Total genesis `balance` minus `liquid` equals funds locked for vesting
                let locked = balance.saturating_sub(liquid);
                let length_as_balance = T::BlockNumberToBalance::convert(length);
                let per_block = locked / length_as_balance.max(sp_runtime::traits::One::one());
                let vesting_info = VestingInfo::new(locked, per_block, begin);
                if !vesting_info.is_valid() {
                    panic!("Invalid VestingInfo params at genesis")
                };

                Vesting::<T>::insert(who, vesting_info);

                frame_system::Pallet::<T>::inc_providers(who);

                T::Currency::reserve_named(&VESTING_ID, who, locked)
                    .expect("Unable to reserve balance");
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new vesting was created.
        VestingCreated {
            /// The account.
            account: T::AccountId,
            /// Amount locked.
            amount: BalanceOf<T>,
        },
        /// The amount vested has been updated. This could indicate a change in funds available.
        /// The balance given is the amount which is left unvested (and thus locked).
        VestingUpdated {
            /// The account.
            account: T::AccountId,
            /// Amount locked
            unvested: BalanceOf<T>,
        },
        /// An account has become fully vested.
        VestingCompleted {
            /// The account.
            account: T::AccountId,
        },
        /// A vesting was removed.
        VestingRemoved {
            /// The account.
            account: T::AccountId,
            /// Amount slashed.
            amount: BalanceOf<T>,
        },
    }

    /// Error for the vesting pallet.
    #[pallet::error]
    pub enum Error<T> {
        /// The account given is not vesting.
        NotVesting,
        /// The account is already vesting.
        AlreadyVesting,
        /// Failed to create a new schedule because some parameter was invalid.
        InvalidScheduleParams,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Unlock any vested funds of the sender account.
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(3, 3))]
        pub fn vest(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::do_vest(who)
        }

        /// Force a vested transfer.
        ///
        /// The dispatch origin for this call must be _Root_.
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 4))]
        pub fn force_vested_transfer(
            origin: OriginFor<T>,
            source: T::AccountId,
            dest: T::AccountId,
            schedule: VestingInfo<BalanceOf<T>, BlockNumberFor<T>>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(schedule.is_valid(), Error::<T>::InvalidScheduleParams);
            ensure!(!Vesting::<T>::contains_key(&dest), Error::<T>::AlreadyVesting);

            T::Currency::transfer(
                &source,
                &dest,
                schedule.locked,
                ExistenceRequirement::KeepAlive,
            )?;

            Vesting::<T>::insert(&dest, schedule);
            frame_system::Pallet::<T>::inc_providers(&dest);

            T::Currency::reserve_named(&VESTING_ID, &dest, schedule.locked)?;

            Self::deposit_event(Event::<T>::VestingCreated {
                account: dest,
                amount: schedule.locked,
            });

            Ok(())
        }

        /// Force remove a vesting schedule and slash locked balance.
        ///
        /// The dispatch origin for this call must be _Root_.
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 4))]
        pub fn force_remove_vesting(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;

            let schedule = Vesting::<T>::take(&who).ok_or(Error::<T>::NotVesting)?;

            let (imbalance, unslashed) =
                T::Currency::slash_reserved_named(&VESTING_ID, &who, schedule.locked);
            let slashed = schedule.locked.saturating_sub(unslashed);

            T::Slash::on_unbalanced(imbalance);
            frame_system::Pallet::<T>::dec_providers(&who)?;

            Self::deposit_event(Event::<T>::VestingRemoved { account: who, amount: slashed });

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Unlock any vested funds of `who`.
    fn do_vest(who: T::AccountId) -> DispatchResult {
        let vesting = Self::vesting(&who).ok_or(Error::<T>::NotVesting)?;

        let now = <frame_system::Pallet<T>>::block_number();
        let locked = vesting.locked_at::<T::BlockNumberToBalance>(now);

        T::Currency::ensure_reserved_named(&VESTING_ID, &who, locked)?;

        if locked.is_zero() {
            Vesting::<T>::remove(&who);
            frame_system::Pallet::<T>::dec_providers(&who)?;
            Self::deposit_event(Event::<T>::VestingCompleted { account: who });
        } else {
            Self::deposit_event(Event::<T>::VestingUpdated { account: who, unvested: locked });
        }

        Ok(())
    }
}
