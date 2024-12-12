#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{
    tokens::imbalance::Imbalance, Currency, ExistenceRequirement, WithdrawReasons,
};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{traits::Saturating, Percent, RuntimeDebug};

pub use pallet::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// An Ethereum address (i.e. 20 bytes, used to represent an Ethereum account).
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct EthereumAddress(pub [u8; 20]);

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency trait.
        type Currency: Currency<Self::AccountId>;

        /// The origin which may withdraw funds from `BridgeAccount`.
        type PayoutOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        #[pallet::constant]
        type BridgeAccount: Get<Self::AccountId>;

        #[pallet::constant]
        type FeeReceiverAccount: Get<Self::AccountId>;

        #[pallet::constant]
        type DepositFeePercent: Get<u8>;

        #[pallet::constant]
        type WithdrawalFeePercent: Get<u8>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Some amount was transferred to Ethereum address.
        Transfer { from: T::AccountId, to: EthereumAddress, amount: BalanceOf<T> },
        /// A payment happened.
        Paid { to: T::AccountId, amount: BalanceOf<T> },
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().reads_writes(3, 3))]
        pub fn transfer(
            origin: OriginFor<T>,
            dest: EthereumAddress,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let fee_percent = Percent::from_percent(T::WithdrawalFeePercent::get());
            let fee_amount = fee_percent.mul_floor(amount);

            let mut imbalance = T::Currency::withdraw(
                &who,
                amount.saturating_add(fee_amount),
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )?;

            let fee = imbalance.extract(fee_amount);
            let amount = imbalance.peek();

            T::Currency::resolve_creating(&T::BridgeAccount::get(), imbalance);
            T::Currency::resolve_creating(&T::FeeReceiverAccount::get(), fee);

            Self::deposit_event(Event::Transfer { from: who, to: dest, amount });

            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().reads_writes(4, 4))]
        pub fn payout(
            origin: OriginFor<T>,
            dest: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            T::PayoutOrigin::ensure_origin(origin)?;

            let fee_percent = Percent::from_percent(T::DepositFeePercent::get());
            let fee_amount = fee_percent.mul_floor(amount);

            let mut imbalance = T::Currency::withdraw(
                &T::BridgeAccount::get(),
                amount,
                WithdrawReasons::TRANSFER,
                ExistenceRequirement::KeepAlive,
            )?;

            let fee = imbalance.extract(fee_amount);
            let amount = imbalance.peek();

            T::Currency::resolve_creating(&dest, imbalance);
            T::Currency::resolve_creating(&T::FeeReceiverAccount::get(), fee);

            Self::deposit_event(Event::Paid { to: dest, amount });

            Ok(())
        }
    }
}
