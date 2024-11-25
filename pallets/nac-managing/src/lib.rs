//!
//! # Module Overview
//!
//! This Rust module defines a pallet for managing NFTs and VIPP (Very Important Person Protocol) NFTs
//! on a Substrate-based blockchain. The pallet allows minting NFTs, updating their attributes, managing
//! NAC (Nonfungible Asset Certificate) levels, and minting special VIPP NFTs. The primary aim is to
//! provide a mechanism for creating and managing user-owned NFTs, including the issuance of special
//! status tokens (VIPP NFTs) based on specific actions.
//!
//! # Key Features and Components
//!
//! - **NFT Minting and Management**:
//!   - The pallet allows for minting NFTs (`mint()`) for users with specified NAC levels and supports
//!     updating existing NFTs to reflect changes in their attributes, such as increasing the NAC level.
//!     The minting process is managed by an authorized origin (typically an admin).
//!   - **NAC Level Management**: Each user-owned NFT is associated with a NAC level (`u8`). This level
//!     is used to manage and track different tiers or benefits for NFT holders.
//!
//! - **VIPP NFT Minting**:
//!   - VIPP NFTs are special tokens issued to users when specific conditions are met (e.g., upon
//!     claiming rewards). The pallet includes a `mint_vipp_nft()` function that is triggered during
//!     certain operations, providing an additional level of recognition to users.
//!
//! - **Storage Items**:
//!   - **`UsersNft`**: Tracks the NFT details for each user, including their NAC level.
//!   - **Events**: Several events are defined, such as `NftMinted`, `NftUpdated`, and `VippNftMinted`,
//!     which provide information about the actions taken within the pallet, such as minting or updating
//!     NFTs.
//!
//! - **Extrinsics**:
//!   - **`mint()`**: Allows an admin to mint an NFT for a user, specifying the NAC level and owner.
//!     This function ensures that the user meets the requirements and that the NFT is not already
//!     minted for that user.
//!   - **On-Chain Attribute Updates**: The `on_claim()` function allows updating the NFT's attributes
//!     based on user actions, such as claims, which may lead to minting a VIPP NFT.
//!
//! # Access Control and Security
//!
//! - **Admin Origin Requirement**: The `mint()` extrinsic can only be called by an authorized origin
//!   (`T::AdminOrigin::ensure_origin(origin)`), ensuring that only authorized accounts can mint NFTs.
//! - **Error Handling**: The pallet includes custom error types (`Error<T>`) for scenarios such as
//!   attempting to mint an already existing NFT (`NftAlreadyExist`) or updating an invalid NAC level
//!   (`NacLevelIsIncorrect`). This helps maintain the integrity of the pallet's operations.
//! - **Controlled Attribute Changes**: The attribute change process for NFTs is controlled through
//!   the on-chain storage and verified by events, ensuring consistency and preventing unauthorized
//!   modifications.
//!
//! # Developer Notes
//!
//! - **Default NAC Level**: A default NAC level (`DEFAULT_NAC_LEVEL`) is assigned to newly minted NFTs.
//!   This ensures that all NFTs start with a baseline value that can be incremented as needed through
//!   the pallet's functions.
//! - **Flexible NFT Handling**: The pallet allows for multiple interactions with NFTs, including minting,
//!   updating, and minting VIPP NFTs based on user actions. Developers can extend these functionalities
//!   to suit specific business requirements or add new NFT attributes as needed.
//! - **Hooks for Claims and Withdrawals**: The pallet integrates hooks (`on_claim()` and `on_withdraw_fee()`)
//!   that allow for seamless interaction with other pallets or components of the blockchain. For example,
//!   the `on_claim()` function allows minting VIPP NFTs upon a user's successful claim, integrating
//!   additional utility into the existing reward or staking system.
//!
//! # Usage Scenarios
//!
//! - **Minting NFTs for Special Users**: The `mint()` extrinsic is used by an admin to issue NFTs to
//!   specific users, assigning an initial NAC level. This can be used in scenarios such as rewarding
//!   early adopters or providing exclusive access to certain features of the network.
//! - **NAC Level Upgrades**: Users with existing NFTs can have their NAC levels updated based on
//!   specific achievements or milestones. This can be achieved through on-chain functions that
//!   validate user actions and update the NFT's attributes accordingly.
//! - **VIPP Recognition**: Users who meet specific conditions, such as reaching a certain claim amount,
//!   may be rewarded with a VIPP NFT. This helps create an additional incentive for user engagement
//!   and can be used as a status symbol within the community.
//!
//! # Integration Considerations
//!
//! - **Event-Driven Architecture**: The pallet emits several events (`NftMinted`, `NftUpdated`, `VippNftMinted`)
//!   that can be listened to by other modules or off-chain systems. These events are crucial for
//!   creating an event-driven system that reacts to user actions and provides real-time feedback to
//!   the blockchain users.
//! - **OnChain Attribute Integration**: Developers integrating this pallet should consider how the
//!   on-chain attributes (`CLAIM_AMOUNT_ATTRIBUTE_KEY`) interact with the broader system, particularly
//!   when managing rewards, claims, or other actions that may affect NFT ownership or status.
//! - **Administrative Control**: The pallet assumes administrative control for minting NFTs, which means
//!   that the governance model should clearly define who has the authority to execute these functions.
//!   Proper role assignment and verification mechanisms should be in place to avoid misuse or
//!   unauthorized minting of NFTs.
//!
//! # Example Scenario
//!
//! Suppose an admin wants to reward active users of the blockchain by minting special NFTs for them.
//! The admin uses the `mint()` function to issue NFTs, setting an initial NAC level based on each user's
//! engagement. Users can then increase their NAC levels by participating in network activities, such as
//! staking or claiming rewards. When a user's claim amount exceeds a certain threshold, a VIPP NFT is
//! minted for them, recognizing their contributions. The pallet tracks each of these actions, and events
//! like `NftMinted` and `VippNftMinted` are emitted, allowing other network participants to see the
//! status and actions of their peers.
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use frame_support::{
    pallet_prelude::{BoundedVec, DispatchResult},
    traits::{
        tokens::nonfungibles_v2::{Create, Inspect, InspectEnumerable, Mutate},
        Currency, Get, Incrementable, OnNewAccount,
    },
};
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
pub use pallet::*;
use pallet_claiming::OnClaimHandler;
use pallet_energy_fee::OnWithdrawFeeHandler;
use pallet_nfts::{CollectionConfig, CollectionSettings, ItemConfig, ItemSettings, MintSettings};
use pallet_reputation::{AccountReputation, ReputationPoint, ReputationRecord, ReputationTier};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::Perbill;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::{
    traits::{BlakeTwo256, Hash, MaybeSerializeDeserialize},
    SaturatedConversion,
};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

type CollectionConfigFor<T> = CollectionConfig<
    <T as pallet_balances::Config>::Balance,
    BlockNumberFor<T>,
    <T as Config>::CollectionId,
>;

/// NAC level attribute key in NFT.
const NAC_LEVEL_ATTRIBUTE_KEY: [u8; 3] = [0, 0, 1];

/// Claimed amount attribute key in NFT.
const CLAIM_AMOUNT_ATTRIBUTE_KEY: [u8; 3] = [0, 0, 2];

/// Did the account have VIPP status.
const VIPP_STATUS_EXIST: [u8; 3] = [0, 0, 3];

/// Default NAC level for account.
const DEFAULT_NAC_LEVEL: u8 = 1;

/// Extrinsic index.
const EXTRINSIC_INDEX: u32 = 135;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::LockableCurrency;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config + pallet_reputation::Config + pallet_balances::Config
    {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Registry for the minted NFTs.
        type Nfts: Inspect<Self::AccountId, ItemId = Self::ItemId, CollectionId = Self::CollectionId>
            + Mutate<Self::AccountId, ItemConfig>
            + Create<Self::AccountId, CollectionConfigFor<Self>>
            + InspectEnumerable<Self::AccountId>;

        /// The collection id type.
        type CollectionId: MaybeSerializeDeserialize
            + Parameter
            + Member
            + Copy
            + Default
            + Ord
            + Incrementable;

        /// The item id type.
        type ItemId: Member + Parameter + MaxEncodedLen + Copy + From<u32>;

        /// The maximum number of bytes that may be used to represent an NFT attribute key.
        type KeyLimit: Get<u32>;

        /// The maximum number of bytes that may be used to represent an NFT attribute value.
        type ValueLimit: Get<u32>;

        /// The origin which may forcibly mint a NFT or otherwise alter privileged
        /// attributes.
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Weight information for extrinsic.
        type WeightInfo: WeightInfo;

        /// The currency.
        type Currency: LockableCurrency<
            Self::AccountId,
            Moment = BlockNumberFor<Self>,
            Balance = <Self as pallet_balances::Config>::Balance,
        >;

        /// Handler for VIPP members.
        type OnVIPPChanged: OnVippStatusHandler<
            Self::AccountId,
            <Self as pallet_balances::Config>::Balance,
            Self::ItemId,
        >;

        /// NFT Collection ID.
        type NftCollectionId: Get<Self::CollectionId>;

        /// VIPP NFT Collection ID.
        type VIPPCollectionId: Get<Self::CollectionId>;
    }

    /// Temp storage: the information about user NFTs and NAC levels.
    #[pallet::storage]
    pub type UsersNft<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (T::ItemId, u8), OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// NFT was minted.
        NftMinted {
            /// Who gets the NAC.
            owner: T::AccountId,
            /// The NAC unique ID.
            item_id: T::ItemId,
        },

        /// NFT metadata and attributes were updated.
        NftUpdated {
            /// Whose NFT.
            owner: T::AccountId,
            /// The NAC level.
            nac_level: u8,
        },

        /// User has NAC level.
        UserNacLevel {
            /// NAC level owner.
            owner: T::AccountId,
            /// NAC level value.
            nac_level: u8,
        },

        /// VIPP NFT was minted.
        VippNftMinted {
            /// Who gets the VIPP NFT.
            owner: T::AccountId,
            /// The VIPP NFT unique ID.
            item_id: T::ItemId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// NFT wasn't found.
        NftNotFound,
        /// NFT already exist.
        NftAlreadyExist,
        /// NAC level is not correct.
        NacLevelIsIncorrect,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Mint a NFT item of a particular collection.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(origin: OriginFor<T>, nac_level: u8, owner: T::AccountId) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            let collection = T::NftCollectionId::get();

            match Self::get_nac_level(&owner) {
                Some((current_nac_level, item_id)) => {
                    if current_nac_level == nac_level {
                        return Err(Error::<T>::NftAlreadyExist)?;
                    }

                    Self::update_nft_info(&collection, &item_id, nac_level, owner)
                },
                _ => {
                    let item_id = Self::create_unique_item_id(&owner);
                    Self::do_mint(item_id, owner.clone())?;
                    Self::update_nft_info(&collection, &item_id, nac_level, owner)
                },
            }
        }

        /// Update metadata and NAC level.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::update_nft())]
        pub fn update_nft(
            origin: OriginFor<T>,
            new_nac_level: Option<u8>,
            owner: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;

            let collection = T::NftCollectionId::get();
            let nac_level: u8;

            let item_id = match Self::get_nac_level(&owner) {
                Some(value) => {
                    // Checking whether the NAC level needs to be changed.
                    nac_level = new_nac_level.unwrap_or(value.0);
                    value.1
                },
                None => return Err(Error::<T>::NftNotFound)?,
            };

            Self::update_nft_info(&collection, &item_id, nac_level, owner)
        }

        /// Check NAC level by account_id.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::check_nac_level())]
        pub fn check_nac_level(origin: OriginFor<T>, owner: T::AccountId) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            let nac_level = Self::get_nac_level(&owner).ok_or(Error::<T>::NftNotFound)?.0;
            Self::deposit_event(Event::UserNacLevel { nac_level, owner });
            Ok(())
        }
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// The accounts, who get NACs with values as the second field.
        pub accounts: Vec<(T::AccountId, u8)>,
        /// The accounts, who are collection owners.
        pub owners: Vec<T::AccountId>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for owner in self.owners.iter() {
                Pallet::<T>::create_collection(owner).expect("Cannot create a collection");
                Pallet::<T>::create_collection(owner).expect("Cannot create a collection");

                // Get collection Id.
                let collection_id: T::CollectionId =
                    T::CollectionId::initial_value().unwrap_or_default();

                for (n, (account, level)) in self.accounts.iter().enumerate() {
                    Pallet::<T>::do_mint((n as u32).into(), account.clone())
                        .expect("Cannot mint NFT");

                    Pallet::<T>::update_nft_info(
                        &collection_id,
                        &(n as u32).into(),
                        *level,
                        account.clone(),
                    )
                    .expect("Cannot update NFT.");
                }
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Mint user NFT.
    pub fn do_mint(item_id: T::ItemId, owner: T::AccountId) -> DispatchResult {
        let item_config = ItemConfig { settings: ItemSettings::all_enabled() };
        let collection = T::NftCollectionId::get();

        T::Nfts::mint_into(&collection, &item_id, &owner, &item_config, true)?;

        pallet_reputation::Pallet::<T>::increase_creating(
            &owner,
            ReputationPoint::from(ReputationTier::Vanguard(1)),
        );

        Self::deposit_event(Event::NftMinted { owner, item_id });
        Ok(())
    }

    /// Update user NFT.
    pub fn update_nft_info(
        collection: &T::CollectionId,
        item: &T::ItemId,
        nac_level: u8,
        owner: T::AccountId,
    ) -> DispatchResult {
        let key = BoundedVec::<u8, T::KeyLimit>::try_from(Vec::from(NAC_LEVEL_ATTRIBUTE_KEY))
            .unwrap_or_default();
        let mut nac = BoundedVec::<u8, T::ValueLimit>::new();
        nac.try_push(nac_level).map_err(|_| Error::<T>::NacLevelIsIncorrect)?;

        T::Nfts::set_attribute(collection, item, &key, &nac)?;

        // Temporary solution to save NFT id and NAC level by user.
        UsersNft::<T>::insert(&owner, (&item, &nac_level));

        Self::deposit_event(Event::NftUpdated { owner, nac_level });

        Ok(())
    }

    /// Generate uniq ItemId using block_number, token_owner and extrinsic_index
    pub fn create_unique_item_id(owner: &T::AccountId) -> T::ItemId {
        let block_number = frame_system::Pallet::<T>::block_number();
        let mut unique_number = Vec::new();

        unique_number.extend_from_slice(&block_number.encode());
        unique_number.extend_from_slice(&EXTRINSIC_INDEX.to_le_bytes());
        unique_number.extend_from_slice(owner.encode().as_ref());

        // Combine the bytes of the hash into an u32 by bitwise OR (|) and left shifts (<<).
        let hash = BlakeTwo256::hash(&unique_number);
        let mut item_id: u32 = 0;
        for i in 0..4 {
            item_id |= (hash[i] as u32) << (i * 8);
        }

        T::ItemId::from(item_id)
    }

    /// Create a new collection.
    pub fn create_collection(owner: &T::AccountId) -> DispatchResult {
        let collection_config = CollectionConfig {
            settings: CollectionSettings::all_enabled(),
            max_supply: None,
            mint_settings: MintSettings::default(),
        };

        T::Nfts::create_collection(owner, owner, &collection_config).map(|_| ())
    }

    /// Check whether the account has the level.
    pub fn user_has_access(account_id: T::AccountId, desired_access_level: u8) -> bool {
        match Self::get_nac_level(&account_id) {
            Some(value) => value.0 >= desired_access_level,
            None => false,
        }
    }

    /// Get NAC level.
    pub fn get_nac_level(account_id: &T::AccountId) -> Option<(u8, <T as Config>::ItemId)> {
        let collection_id = T::NftCollectionId::get();

        if let Some(key) = T::Nfts::owned_in_collection(&collection_id, account_id).next() {
            let item_id = key;
            // Get NAC by NFT attribute key.
            let nac_level =
                T::Nfts::system_attribute(&collection_id, Some(&item_id), &NAC_LEVEL_ATTRIBUTE_KEY);

            return match nac_level {
                Some(bytes) => Some((bytes[0], item_id)),
                None => Some((DEFAULT_NAC_LEVEL, item_id)),
            };
        }

        None
    }

    /// Mint VIPP nft to account.
    pub fn mint_vipp_nft(account: &T::AccountId) -> Option<(T::Balance, <T as Config>::ItemId)> {
        let claim_balance = Self::get_claim_balance(account);
        let active_vipp_amount = Self::get_active_vipp_amount(account);

        if let Some(claim_balance) = claim_balance {
            if Self::threshold_meets_vipp_requirements(account, claim_balance.0)
                && active_vipp_amount <= claim_balance.0
            {
                let item_id = Self::create_unique_item_id(account);
                let item_config = ItemConfig { settings: ItemSettings::all_enabled() };
                let collection = T::VIPPCollectionId::get();
                let perbill = Perbill::from_rational(95_u32, 100_u32);

                let result = T::Nfts::mint_into(&collection, &item_id, account, &item_config, true);

                let second_result = T::Nfts::set_attribute(
                    &collection,
                    &item_id,
                    &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                    &(claim_balance.0 - active_vipp_amount).encode(),
                );

                let item_id = match Self::get_nac_level(account) {
                    Some(value) => value.1,
                    None => return None,
                };

                if result.is_ok() && second_result.is_ok() {
                    Self::deposit_event(Event::VippNftMinted { owner: account.clone(), item_id });
                    return Some((perbill * (claim_balance.0 - active_vipp_amount), item_id));
                }

                return None;
            }
        }

        None
    }

    /// can mint VIPP NFT to account.
    pub fn can_mint_vipp(account: &T::AccountId) -> Option<(T::Balance, <T as Config>::ItemId)> {
        let collection_id = T::NftCollectionId::get();
        if let Some(key) = T::Nfts::owned_in_collection(&collection_id, account).next() {
            let item_id = key;
            let vipp_status_exist =
                T::Nfts::system_attribute(&collection_id, Some(&item_id), &VIPP_STATUS_EXIST);

            return match vipp_status_exist {
                Some(_) => None,
                None => {
                    if Self::get_claim_balance(account).is_some() {
                        return Self::mint_vipp_nft(account);
                    }

                    None
                },
            };
        }

        None
    }

    /// Get user claim balance.
    pub fn get_claim_balance(
        account_id: &T::AccountId,
    ) -> Option<(T::Balance, <T as Config>::ItemId)> {
        let collection_id = T::NftCollectionId::get();

        if let Some(key) = T::Nfts::owned_in_collection(&collection_id, account_id).next() {
            let item_id = key;
            // Get claim amount by NFT attribute key.
            let claim_balance = T::Nfts::system_attribute(
                &collection_id,
                Some(&item_id),
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            );

            return match claim_balance {
                Some(bytes) => match T::Balance::decode(&mut bytes.as_slice()) {
                    Ok(balance) => Some((balance, item_id)),
                    _ => None,
                },
                None => None,
            };
        }

        None
    }

    /// Check threshold of account.
    pub fn threshold_meets_vipp_requirements(
        account: &T::AccountId,
        claim_balance: <T as pallet_balances::Config>::Balance,
    ) -> bool {
        let free_balance = T::Currency::total_balance(account);
        let perbill = Perbill::from_rational(95_u32, 100_u32);

        if free_balance > perbill * claim_balance {
            return true;
        }

        false
    }

    /// Check VIPP threshold every transaction.
    pub fn check_account_threshold(account: &T::AccountId) {
        while let Some(bytes) = Self::get_claim_balance(account) {
            if !Self::threshold_meets_vipp_requirements(account, bytes.0) {
                if !Self::burn_vipp_nft(account) {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Burn VIPP NFT (return true if the VIPP was burned).
    pub fn burn_vipp_nft(account: &T::AccountId) -> bool {
        let collection_id = T::VIPPCollectionId::get();

        let mut lowest_claim_value = None;

        // Find the VIPP NFT with lowest claim value.
        for key in T::Nfts::owned_in_collection(&collection_id, account) {
            let item_id = key;

            if let Some(claim_value) = T::Nfts::system_attribute(
                &collection_id,
                Some(&item_id),
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            ) {
                match lowest_claim_value {
                    Some((min_claim, _)) if claim_value < min_claim => {
                        lowest_claim_value = Some((claim_value, item_id))
                    },
                    None => lowest_claim_value = Some((claim_value, item_id)),
                    _ => {},
                }
            }
        }

        // Burn NFT.
        if let Some((amount, item_id)) = lowest_claim_value {
            // Burn VIPP NFT.
            let _ = T::Nfts::burn(&collection_id, &item_id, Some(account));
            T::OnVIPPChanged::burn_vipp_nft(account, item_id);
            // Decrease threshold.
            let _ = Self::decrease_thrsehold(
                account,
                T::Balance::decode(&mut amount.as_slice()).unwrap_or(T::Balance::zero()),
            );
            true
        } else {
            Self::set_inactive_attribute_vipp(account);
            false
        }
    }

    /// Set VIPP inactive status for NAC.
    fn set_inactive_attribute_vipp(account: &T::AccountId) {
        let collection = T::NftCollectionId::get();

        let item_id = match Self::get_nac_level(account) {
            Some(value) => value.1,
            None => {
                return;
            },
        };

        let key = BoundedVec::<u8, T::KeyLimit>::try_from(Vec::from(VIPP_STATUS_EXIST))
            .unwrap_or_default();
        let mut is_exist = BoundedVec::<u8, T::ValueLimit>::new();
        let _ = is_exist.try_push(1);
        let _ = T::Nfts::set_attribute(&collection, &item_id, &key, &is_exist);
    }

    /// Decrease threshold of NAC claim value.
    fn decrease_thrsehold(account: &T::AccountId, amount: T::Balance) -> DispatchResult {
        let collection = T::NftCollectionId::get();
        let item = T::Nfts::owned_in_collection(&collection, account)
            .next()
            .ok_or(Error::<T>::NftNotFound)?;

        let claimed_raw =
            T::Nfts::system_attribute(&collection, Some(&item), &CLAIM_AMOUNT_ATTRIBUTE_KEY)
                .unwrap_or(vec![]);
        let currently_claimed =
            T::Balance::decode(&mut claimed_raw.as_slice()).unwrap_or(T::Balance::zero());

        let updated_claimed = currently_claimed.saturating_sub(amount);

        T::Nfts::set_attribute(
            &collection,
            &item,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &updated_claimed.encode(),
        )
    }

    /// Get amount of active VIPP NFT.
    fn get_active_vipp_amount(account: &T::AccountId) -> T::Balance {
        let collection_id = T::VIPPCollectionId::get();
        let mut total_sum = T::Balance::zero();

        for key in T::Nfts::owned_in_collection(&collection_id, account) {
            let item_id = key;

            if let Some(claim_value_bytes) = T::Nfts::system_attribute(
                &collection_id,
                Some(&item_id),
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            ) {
                if let Ok(claim_value) = T::Balance::decode(&mut &claim_value_bytes[..]) {
                    total_sum += claim_value;
                }
            }
        }

        total_sum
    }
}

impl<T: Config> Convert<&T::AccountId, Option<u8>> for Pallet<T> {
    fn convert(who: &T::AccountId) -> Option<u8> {
        Pallet::<T>::get_nac_level(who).map(|(level, _)| level)
    }
}

impl<T: Config> OnNewAccount<T::AccountId> for Pallet<T> {
    fn on_new_account(who: &T::AccountId) {
        if AccountReputation::<T>::contains_key(who) {
            return;
        }
        // Add reputation points to account.
        let now = <frame_system::Pallet<T>>::block_number().saturated_into();
        let new_rep = ReputationRecord::with_blocknumber(now);
        AccountReputation::<T>::insert(who, new_rep);

        // Add default NAC NFT to account.
        let item_id = Self::create_unique_item_id(who);
        let nac_minting_result = Self::do_mint(item_id, who.clone());
        if nac_minting_result.is_ok() {
            UsersNft::<T>::insert(who, (&item_id, &DEFAULT_NAC_LEVEL));
        }
    }
}

impl<T: Config, Balance> OnClaimHandler<T::AccountId, Balance> for Pallet<T>
where
    Balance: frame_support::traits::tokens::Balance,
{
    fn on_claim(who: &T::AccountId, amount: Balance) -> DispatchResult {
        let collection = T::NftCollectionId::get();
        let item = T::Nfts::owned_in_collection(&collection, who)
            .next()
            .ok_or(Error::<T>::NftNotFound)?;

        let claimed_raw =
            T::Nfts::system_attribute(&collection, Some(&item), &CLAIM_AMOUNT_ATTRIBUTE_KEY)
                .unwrap_or(vec![]);
        let currently_claimed =
            Balance::decode(&mut claimed_raw.as_slice()).unwrap_or(Balance::zero());

        let updated_claimed = currently_claimed.saturating_add(amount);

        T::Nfts::set_attribute(
            &collection,
            &item,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &updated_claimed.encode(),
        )?;

        if currently_claimed != Balance::zero() {
            let nft = Self::mint_vipp_nft(who);
            if let Some(nft) = nft {
                T::OnVIPPChanged::mint_vipp(who, nft.0, nft.1);
            }
        }

        Ok(())
    }
}

impl<T: Config> OnWithdrawFeeHandler<T::AccountId> for Pallet<T> {
    fn on_withdraw_fee(who: &T::AccountId) {
        Pallet::<T>::check_account_threshold(who);
    }
}

/// Handler for updating, burning VIPP status.
pub trait OnVippStatusHandler<AccountId, Balance, ItemId> {
    /// Handle a minting new VIPP NFT.
    fn mint_vipp(who: &AccountId, _amount: Balance, item_id: ItemId);

    /// Burning VIPP NFT.
    fn burn_vipp_nft(who: &AccountId, item_id: ItemId);
}

impl<AccountId, Balance, ItemId> OnVippStatusHandler<AccountId, Balance, ItemId> for () {
    fn mint_vipp(_who: &AccountId, _amount: Balance, _item_id: ItemId) {}
    fn burn_vipp_nft(_who: &AccountId, _item_id: ItemId) {}
}
