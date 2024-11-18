//!
//! # Module Overview
//!
//! This Rust module defines a simple faucet pallet for a Substrate-based blockchain. The faucet
//! allows users to request funds using their own accounts, with specific restrictions in place
//! to prevent abuse. This pallet is intended for use in test networks only and provides a
//! mechanism to distribute a limited amount of tokens to users within a given time frame.
//!
//! # Key Features and Functions
//!
//! - **Funds Request Limit**:
//!   - Users can request funds only up to `Config::MaxAmount` within a 24-hour period (`Config::AccumulationPeriod`).
//!     This limit helps ensure that the faucet is used responsibly and prevents any one user from
//!     exhausting available resources.
//!
//! - **Signed Origin Requirement**:
//!   - The pallet requires that requests come from a signed origin (`ensure_signed`). This ensures
//!     that only authenticated accounts can request funds, providing a layer of security to prevent
//!     unauthorized usage.
//!
//! - **Storage and Events**:
//!   - `Requests`: Tracks the funds requested by users along with the timestamp of their last request.
//!     This is used to enforce the accumulation period and limit the amount of funds users can request
//!     over time.
//!   - Events such as `FundsSent` are emitted to record successful fund requests, providing a mechanism
//!     for tracking the faucet’s activity in the blockchain’s event log.
//!
//! # Access Control and Security
//!
//! - **Limited Usage**: Users can only request funds up to a pre-configured maximum (`Config::MaxAmount`)
//!   to prevent draining the faucet. Requests exceeding the limit are rejected with an `AmountTooHigh` error.
//! - **Anti-Spam Mechanism**: The `AccumulationPeriod` setting ensures that users cannot repeatedly request
//!   funds in quick succession, mitigating abuse and preventing spam requests.
//!
//! # Developer Notes
//!
//! - **Test Network Use Only**: This pallet is intended for use on test networks. Its purpose is to
//!   provide developers and testers with a means of accessing tokens for testing purposes without needing
//!   to acquire them from an external source. It is not recommended for use on a production network due
//!   to the risk of abuse.
//! - **Weight Considerations**: The pallet includes a weight definition for the `request_funds` extrinsic
//!   to ensure that it does not disproportionately consume resources. The weight function (`WeightInfo::request_funds()`)
//!   helps keep the operation cost predictable and fair within the network.
//!
//! # Usage Scenarios
//!
//! - **Development and Testing**: The faucet pallet is most useful in development or test environments
//!   where developers need a way to easily distribute tokens to various accounts for testing purposes.
//!   The restrictions on the amount and frequency of requests help simulate a realistic environment
//!   without the risk of exhaustion.
//! - **Simulating Real-World Limits**: By enforcing the `MaxAmount` and `AccumulationPeriod` restrictions,
//!   this pallet can help developers understand how users might interact with a limited token distribution
//!   system. This is useful for simulating scenarios where resources are constrained, providing insights
//!   into user behavior under such conditions.
//!
//! # Integration Considerations
//!
//! - **Runtime Configuration**: The `MaxAmount` and `AccumulationPeriod` are configurable constants.
//!   Developers should set these values according to their specific use case, balancing the need for
//!   accessibility with the need to prevent abuse of the faucet.
//! - **Event Monitoring**: The emitted events, such as `FundsSent`, can be monitored to track usage of
//!   the faucet. This can be helpful for analyzing network activity, detecting unusual patterns, or
//!   ensuring that the faucet is functioning correctly.
//! - **Balance Module Dependency**: This pallet depends on the `pallet_balances` module to create
//!   deposits in user accounts. Proper integration with the balance pallet is crucial for the correct
//!   operation of the faucet.
//!
//! # Example Scenario
//!
//! Suppose a developer is testing a decentralized application (dApp) on a local testnet and needs to
//! provide multiple accounts with tokens to simulate user interactions. The faucet pallet allows each
//! account to request tokens, up to a maximum limit (`MaxAmount`) within a 24-hour period (`AccumulationPeriod`).
//! This ensures that developers can perform the necessary tests without manually issuing tokens for each
//! transaction while preventing a single account from depleting the test network's resources.
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]
#![warn(missing_docs)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Currency;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_balances::Config {
        /// The maximum amount of funds a user can request per once or in total per 24 hours.
        #[pallet::constant]
        type MaxAmount: Get<Self::Balance>;
        /// The period during which the user can't request more than `Config::MaxAmount`.
        #[pallet::constant]
        type AccumulationPeriod: Get<BlockNumberFor<Self>>;
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Type representing the weight of this pallet
        type WeightInfo: WeightInfo;
    }

    /// Requests done by users.
    #[pallet::storage]
    #[pallet::getter(fn requests)]
    pub type Requests<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::Balance, BlockNumberFor<T>), ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The requested funds sent to `who`. [who, amount]
        FundsSent {
            /// The account ID reaceved the funds.
            who: T::AccountId,
            /// The amount of funds.
            amount: T::Balance,
        },
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// Request amount more than `Config::MaxAmount`.
        AmountTooHigh,
        /// More than allowed funds requested during `Config::AccumulationPeriod`.
        RequestLimitExceeded,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Request some funds.
        #[pallet::call_index(0)]
        #[pallet::weight((<T as Config>::WeightInfo::request_funds(), DispatchClass::Normal, Pays::No))]
        pub fn request_funds(
            origin: OriginFor<T>,
            who: T::AccountId,
            amount: T::Balance,
        ) -> DispatchResult {
            ensure_none(origin)?;

            ensure!(amount <= T::MaxAmount::get(), Error::<T>::AmountTooHigh);

            let (balance, timestamp) = Requests::<T>::get(&who);
            let now = frame_system::Pallet::<T>::block_number();
            let period = now - timestamp;

            let (total, now) = if period >= T::AccumulationPeriod::get() {
                (amount, now)
            } else {
                (balance + amount, timestamp)
            };

            ensure!(total <= T::MaxAmount::get(), Error::<T>::RequestLimitExceeded);

            let _ = pallet_balances::Pallet::<T>::deposit_creating(&who, amount);

            Requests::<T>::insert(&who, (total, now));

            Self::deposit_event(Event::FundsSent { who, amount });

            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::request_funds { who, amount } => ValidTransaction::with_tag_prefix("Faucet")
                    .and_provides((who, amount))
                    .propagate(true)
                    .build(),
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}
