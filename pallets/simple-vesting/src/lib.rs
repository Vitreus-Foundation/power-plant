//! The pallet provides a simple vesting mechanism using "Reserved" balance.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]
#![warn(missing_docs)]
#![allow(clippy::type_complexity)]

use frame_support::dispatch::DispatchResult;
use frame_support::traits::{Currency, NamedReservableCurrency};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, Bounded, Convert, Saturating, Zero},
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
}

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The currency trait.
        type Currency: NamedReservableCurrency<Self::AccountId, ReserveIdentifier = [u8; 8]>;

        /// Convert the block number into a balance.
        type BlockNumberToBalance: Convert<BlockNumberFor<Self>, BalanceOf<Self>>;
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

                T::Currency::reserve_named(&VESTING_ID, who, locked)
                    .expect("Unable to reserve balance");
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}
}

#[allow(dead_code)]
impl<T: Config> Pallet<T> {
    /// Unlock any vested funds of `who`.
    fn do_vest(who: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        T::Currency::unreserve_named(&VESTING_ID, &who, amount);

        Ok(())
    }
}
