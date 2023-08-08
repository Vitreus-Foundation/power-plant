use crate::weights::*;
use crate::{ReputationPoint, ReputationRecord};
pub use impls::*;
pub use pallet::*;

mod impls;

#[allow(clippy::module_inception)]
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::SaturatedConversion;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Type representing the weight of this pallet
        type WeightInfo: WeightInfo;
    }

    /// Reputation per account storage.
    #[pallet::storage]
    #[pallet::getter(fn reputation)]
    pub type AccountReputation<T: Config> =
        StorageMap<_, Twox64Concat, T::AccountId, ReputationRecord>;

    /// Pallet event type.
    #[pallet::event]
    #[pallet::generate_deposit(pub fn deposit_event)]
    pub enum Event<T: Config> {
        /// Reputation of an account is forcibly updated with the new value. [account, points]
        ReputationSetForcibly { account: T::AccountId, points: ReputationPoint },
        /// Reputation of an account is increased for the number of points. [account, points]
        ReputationIncreased { account: T::AccountId, points: ReputationPoint },
        /// Reputation of an account is slashed for the number of points. [account, points]
        ReputationSlashed { account: T::AccountId, points: ReputationPoint },
        /// Reputation of an account is updated. [account, points]
        ReputationUpdated { account: T::AccountId, points: ReputationPoint },
        /// Failed increase reputation of an account. [account, error, points]
        ReputationIncreaseFailed {
            account: T::AccountId,
            error: DispatchError,
            points: ReputationPoint,
        },
    }

    /// Pallet error type.
    #[pallet::error]
    #[derive(PartialEq, Clone)]
    pub enum Error<T> {
        /// Account not found
        AccountNotFound,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Force set reputation points for an account. Should be called by root.
        ///
        /// The associated account will be inserted in the store if it's not there.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::force_set_points())]
        pub fn force_set_points(
            origin: OriginFor<T>,
            account: T::AccountId,
            points: ReputationPoint,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let updated = <frame_system::Pallet<T>>::block_number().saturated_into();

            <AccountReputation<T>>::insert(&account, ReputationRecord { points, updated });

            Self::deposit_event(Event::ReputationSetForcibly { account, points });

            Ok(())
        }

        /// Increase the points for an account by the given amount. Should be called by root.
        ///
        /// The account should be in the store, otherwise this will return an error.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::increase_points())]
        pub fn increase_points(
            origin: OriginFor<T>,
            account: T::AccountId,
            points: ReputationPoint,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::do_increase_points(&account, points)?;
            Ok(())
        }

        /// Slash the points of an account by the given amount. Should be called by root.
        ///
        /// The account should be in the store, otherwise this will return an error.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::slash())]
        pub fn slash(
            origin: OriginFor<T>,
            account: T::AccountId,
            points: ReputationPoint,
        ) -> DispatchResult {
            ensure_root(origin)?;
            Self::do_slash(&account, points)?;
            Ok(())
        }

        /// Update points for a single account with reputation points for time being in the network.
        /// Can be called by any signed origin.
        ///
        /// If an account is not exists, it will be created with 0 points.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::update_points())]
        pub fn update_points(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number().saturated_into();
            let mut rep = <AccountReputation<T>>::get(&account)
                .unwrap_or_else(|| ReputationRecord::with_blocknumber(now));
            rep.update_with_block_number(now);
            let points = rep.points;

            <AccountReputation<T>>::insert(&account, rep);

            Self::deposit_event(Event::ReputationUpdated { account, points });

            Ok(())
        }
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub accounts: Vec<(T::AccountId, ReputationRecord)>,
    }

    // #[cfg(feature = "std")]
    // impl<T: Config> Default for GenesisConfig<T> {
    //     fn default() -> Self {
    //         GenesisConfig { accounts: Default::default() }
    //     }
    // }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (account, reputation) in &self.accounts {
                AccountReputation::<T>::insert(account, reputation);
            }
        }
    }
}
