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
    ensure,
    pallet_prelude::{BoundedVec, DispatchResult},
    storage::with_storage_layer,
    traits::{
        tokens::nonfungibles_v2::{Create, Inspect, InspectEnumerable, Mutate, Transfer},
        Currency, Get, Incrementable, OnNewAccount,
    },
};
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
pub use pallet::*;
use pallet_claiming::OnClaimHandler;
use pallet_energy_fee::OnWithdrawFeeHandler;
use pallet_nfts::{CollectionConfig, CollectionSettings, ItemConfig, ItemSettings, MintSettings};
use pallet_reputation::{AccountReputation, ReputationPoint, ReputationRecord, ReputationTier};
use parity_scale_codec::{DecodeAll, Encode, MaxEncodedLen};
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::Perbill;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::{
    traits::{BlakeTwo256, Hash, MaybeSerializeDeserialize},
    DispatchError, SaturatedConversion,
};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
mod threshold_checks;
pub mod transfers;
pub mod weights;

type CollectionConfigFor<T> = CollectionConfig<
    <T as pallet_balances::Config>::Balance,
    BlockNumberFor<T>,
    <T as Config>::CollectionId,
>;

type VippMintFor<T> = Option<(<T as pallet_balances::Config>::Balance, <T as Config>::ItemId)>;
type NacClaimFor<T> = VippMintFor<T>;
type VippClaimsFor<T> = Vec<(<T as Config>::ItemId, <T as pallet_balances::Config>::Balance)>;

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
    use frame_system::ensure_signed;

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
            + InspectEnumerable<Self::AccountId>
            + Transfer<Self::AccountId>;

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

        /// Currently unvested native funds, excluded from VIPP eligibility.
        type UnvestedBalance: UnvestedBalance<Self::AccountId, Self::Balance>;

        /// Bound for all per-account VIPP ownership scans and mutations.
        #[pallet::constant]
        type MaxVippNftsPerAccount: Get<u32>;

        /// Bound for accounts whose final balance state is checked at block finalization.
        #[pallet::constant]
        type MaxPendingVippAccounts: Get<u32>;

        /// Maximum expensive automatic reconciliations reserved in each block.
        #[pallet::constant]
        type MaxVippReconciliationsPerBlock: Get<u32>;

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

    /// Item temporarily unlocked by an atomic VIPP accounting operation.
    #[pallet::storage]
    pub type VippMutationItem<T: Config> = StorageValue<_, T::ItemId, OptionQuery>;

    /// Accounts requiring a final check once related balance metadata is committed.
    #[pallet::storage]
    pub type PendingVippBalanceChanges<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, T::MaxPendingVippAccounts>, ValueQuery>;

    /// Account currently being reconciled, preventing deposit-refund recursion.
    #[pallet::storage]
    pub type ProcessingVippBalanceChange<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Accounts requiring reconciliation before reward accounting may resume.
    #[pallet::storage]
    pub type VippAccountingQuarantine<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

    /// Lowest eligible balance observed while an account requires reconciliation.
    #[pallet::storage]
    pub type VippBreachBalance<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, OptionQuery>;

    /// Narrow permission for an explicitly approved root balance reconciliation.
    #[pallet::storage]
    pub type VippRepairInProgress<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Expensive automatic checks already executed in this block.
    #[pallet::storage]
    pub type VippReconciliationsThisBlock<T: Config> = StorageValue<_, u32, ValueQuery>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            VippReconciliationsThisBlock::<T>::kill();
            let pending = u64::from(T::MaxPendingVippAccounts::get());
            let reconciliations = u64::from(T::MaxVippReconciliationsPerBlock::get());
            // Finalization can perform a light check for every queued account,
            // even after its heavy-reconciliation budget has been exhausted.
            // The complete observer envelope also covers these lighter checks.
            <T as Config>::WeightInfo::check_vipp_threshold(T::MaxVippNftsPerAccount::get())
                .saturating_mul(reconciliations)
                .saturating_add(
                    <T as Config>::WeightInfo::balance_observer().saturating_mul(pending),
                )
                // Queue take/merge and guards; quarantine/budget reads per account;
                // accounting-ready guards and processing markers per repair.
                .saturating_add(
                    T::DbWeight::get().reads_writes(
                        4 + 2 * pending + 16 * reconciliations,
                        3 + 3 * reconciliations,
                    ),
                )
        }

        fn on_finalize(_now: BlockNumberFor<T>) {
            Self::finish_balance_changes();
        }
    }

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

        /// Accounting could not be safely reconciled; native funds remain usable.
        VippAccountingQuarantined {
            /// Account requiring review and repair before further accrual.
            owner: T::AccountId,
        },

        /// A reviewed account was reconciled against its recorded balance breach.
        VippBalanceRepaired {
            /// Account whose actual NFT state and threshold were reconciled.
            owner: T::AccountId,
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
        /// A VIPP or NAC claim attribute is missing or cannot be decoded.
        InvalidClaimAttribute,
        /// Ownership exceeds the bounded VIPP accounting limit.
        TooManyVippNfts,
        /// The account does not own the VIPP item.
        NotVippOwner,
        /// The destination cannot receive VIPP membership.
        InvalidVippRecipient,
        /// The recipient cannot back the increased VIPP threshold.
        InsufficientVippBalance,
        /// The transfer would produce inconsistent VIPP accounting.
        InvalidVippTransfer,
        /// Accounting requires repair before VIPP ownership can change.
        VippAccountingUnavailable,
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

        /// Transfer a VIPP NFT together with its threshold and reward accounting.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::transfer_vipp(T::MaxVippNftsPerAccount::get()))]
        pub fn transfer_vipp(
            origin: OriginFor<T>,
            item_id: T::ItemId,
            destination: T::AccountId,
        ) -> DispatchResult {
            Self::do_transfer_vipp(&ensure_signed(origin)?, item_id, &destination)
        }

        /// Reconcile a quarantined account using its captured deficit, even if
        /// funds were subsequently restored. This can burn VIPP NFTs.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::check_vipp_threshold(T::MaxVippNftsPerAccount::get()))]
        #[frame_support::transactional]
        pub fn repair_vipp_balance(origin: OriginFor<T>, owner: T::AccountId) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            T::OnVIPPChanged::ensure_repair_ready()?;
            ensure!(!VippRepairInProgress::<T>::get(), Error::<T>::VippAccountingUnavailable);
            let limit = if Self::is_vipp_quarantined(&owner) {
                VippBreachBalance::<T>::get(&owner).ok_or(Error::<T>::VippAccountingUnavailable)?
            } else {
                Self::eligible_vipp_balance(&owner)
            };
            VippRepairInProgress::<T>::put(true);
            Self::repair_balance_with_limit(&owner, limit)?;
            VippRepairInProgress::<T>::kill();
            Self::deposit_event(Event::<T>::VippBalanceRepaired { owner });
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
                Some(bytes) => bytes.first().copied().map(|level| (level, item_id)),
                None => Some((DEFAULT_NAC_LEVEL, item_id)),
            };
        }

        None
    }

    /// Mint a positive, previously unrepresented claim amount as a VIPP NFT.
    /// Returns the 95% reward cap and the actual VIPP item ID.
    pub fn mint_vipp_nft(account: &T::AccountId) -> Result<VippMintFor<T>, DispatchError> {
        T::OnVIPPChanged::ensure_accounting_ready()?;
        ensure!(!Self::is_vipp_quarantined(account), Error::<T>::VippAccountingUnavailable);
        let Some((claim_balance, nac_id)) = Self::try_get_claim_balance(account)? else {
            return Ok(None);
        };
        if T::Nfts::system_attribute(&T::NftCollectionId::get(), Some(&nac_id), &VIPP_STATUS_EXIST)
            .is_some()
        {
            return Ok(None);
        }
        let owned = Self::owned_vipp_claims(account)?;
        let active_vipp_amount = owned
            .iter()
            .fold(T::Balance::zero(), |total, (_, amount)| total.saturating_add(*amount));
        if active_vipp_amount >= claim_balance
            || !Self::threshold_meets_vipp_requirements(account, claim_balance)
        {
            return Ok(None);
        }
        ensure!(
            owned.len() < T::MaxVippNftsPerAccount::get() as usize,
            Error::<T>::TooManyVippNfts
        );
        let amount = claim_balance - active_vipp_amount;
        let item_id = Self::create_unique_item_id(account);
        let collection = T::VIPPCollectionId::get();
        let item_config = ItemConfig { settings: ItemSettings::all_enabled() };

        // Never write an attribute after a failed mint (e.g. an ID collision), and
        // never leave an NFT behind if its claim attribute cannot be written.
        with_storage_layer(|| {
            T::Nfts::mint_into(&collection, &item_id, account, &item_config, true)?;
            T::Nfts::set_attribute(
                &collection,
                &item_id,
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                &amount.encode(),
            )?;
            Self::deposit_event(Event::VippNftMinted { owner: account.clone(), item_id });
            Ok(Some((Perbill::from_percent(95) * amount, item_id)))
        })
    }

    /// Mint a VIPP NFT when the NAC is eligible and has a positive uncovered claim.
    pub fn can_mint_vipp(account: &T::AccountId) -> Result<VippMintFor<T>, DispatchError> {
        Self::mint_vipp_nft(account)
    }

    /// Whether the NAC marks VIPP eligibility as inactive.
    pub fn is_vipp_inactive(account: &T::AccountId) -> bool {
        Self::get_nac_level(account).is_some_and(|(_, item_id)| {
            T::Nfts::system_attribute(
                &T::NftCollectionId::get(),
                Some(&item_id),
                &VIPP_STATUS_EXIST,
            )
            .is_some()
        })
    }

    /// Actual owned VIPP IDs and their raw claim amounts, before the 95% factor.
    /// Malformed attributes must not silently become zero-value entitlements.
    pub fn owned_vipp_claims(account: &T::AccountId) -> Result<VippClaimsFor<T>, DispatchError> {
        let collection = T::VIPPCollectionId::get();
        let items: Vec<_> = T::Nfts::owned_in_collection(&collection, account)
            .take(T::MaxVippNftsPerAccount::get().saturating_add(1) as usize)
            .collect();
        ensure!(
            items.len() <= T::MaxVippNftsPerAccount::get() as usize,
            Error::<T>::TooManyVippNfts
        );
        items
            .into_iter()
            .map(|item_id| {
                let bytes = T::Nfts::system_attribute(
                    &collection,
                    Some(&item_id),
                    &CLAIM_AMOUNT_ATTRIBUTE_KEY,
                )
                .ok_or(Error::<T>::InvalidClaimAttribute)?;
                let amount = T::Balance::decode_all(&mut bytes.as_slice())
                    .map_err(|_| Error::<T>::InvalidClaimAttribute)?;
                Ok((item_id, amount))
            })
            .collect()
    }

    /// Get user claim balance.
    pub fn get_claim_balance(
        account_id: &T::AccountId,
    ) -> Option<(T::Balance, <T as Config>::ItemId)> {
        Self::try_get_claim_balance(account_id).ok().flatten()
    }

    /// Read the NAC claim without treating malformed accounting as no claim.
    pub fn try_get_claim_balance(
        account_id: &T::AccountId,
    ) -> Result<NacClaimFor<T>, DispatchError> {
        let collection_id = T::NftCollectionId::get();

        if let Some(key) = T::Nfts::owned_in_collection(&collection_id, account_id).next() {
            let item_id = key;
            // Get claim amount by NFT attribute key.
            let claim_balance = T::Nfts::system_attribute(
                &collection_id,
                Some(&item_id),
                &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            );

            return claim_balance
                .map(|bytes| {
                    T::Balance::decode_all(&mut bytes.as_slice())
                        .map(|balance| (balance, item_id))
                        .map_err(|_| Error::<T>::InvalidClaimAttribute.into())
                })
                .transpose();
        }

        Ok(None)
    }

    /// Total native funds excluding unvested funds. Locks and reserves are
    /// subsets of the total balance and must not be counted a second time.
    pub fn eligible_vipp_balance(account: &T::AccountId) -> T::Balance {
        T::Currency::total_balance(account)
            .saturating_sub(T::UnvestedBalance::unvested_balance(account))
    }

    /// Check the inclusive 95% threshold against eligible native funds.
    pub fn threshold_meets_vipp_requirements(
        account: &T::AccountId,
        claim_balance: <T as pallet_balances::Config>::Balance,
    ) -> bool {
        Self::eligible_vipp_balance(account) >= Perbill::from_percent(95) * claim_balance
    }

    /// Check VIPP threshold when a fee is withdrawn.
    pub fn check_account_threshold(account: &T::AccountId) {
        if let Err(error) = Self::try_check_account_threshold(account) {
            log::error!(target: "runtime::nac-managing", "VIPP threshold update failed: {:?}", error);
        }
    }

    /// Reconcile a threshold atomically, with work bounded by the NFT limit.
    /// Malformed or oversized legacy ownership is left unchanged for repair.
    #[frame_support::transactional]
    pub fn try_check_account_threshold(account: &T::AccountId) -> DispatchResult {
        Self::ensure_vipp_reconciliation_ready()?;
        Self::check_threshold_with_limit(account, None)
    }

    fn check_threshold_with_limit(
        account: &T::AccountId,
        balance_limit: Option<T::Balance>,
    ) -> DispatchResult {
        let Some((claim, _)) = Self::try_get_claim_balance(account)? else {
            return Ok(());
        };
        let meets_threshold = |claim| {
            let balance = Self::eligible_vipp_balance(account);
            balance.min(balance_limit.unwrap_or(balance)) >= Perbill::from_percent(95) * claim
        };
        if Self::is_vipp_inactive(account) || meets_threshold(claim) {
            return Ok(());
        }
        let mut owned = Self::owned_vipp_claims(account)?;
        owned.sort_by_key(|(_, amount)| *amount);
        if owned.is_empty() {
            return Self::set_inactive_attribute_vipp(account);
        }
        let mut remaining_claim = claim;
        for (item_id, amount) in owned {
            if meets_threshold(remaining_claim) {
                break;
            }
            Self::burn_vipp_item(account, item_id, amount)?;
            remaining_claim = remaining_claim.saturating_sub(amount);
        }
        Ok(())
    }

    /// Burn the smallest VIPP NFT by decoded claim amount, atomically with its accounting.
    pub fn burn_vipp_nft(account: &T::AccountId) -> Result<bool, DispatchError> {
        Self::ensure_vipp_reconciliation_ready()?;
        let smallest =
            Self::owned_vipp_claims(account)?.into_iter().min_by_key(|(_, amount)| *amount);
        with_storage_layer(|| {
            if let Some((item_id, amount)) = smallest {
                Self::burn_vipp_item(account, item_id, amount)?;
                Ok(true)
            } else {
                Self::set_inactive_attribute_vipp(account)?;
                Ok(false)
            }
        })
    }

    fn burn_vipp_item(
        account: &T::AccountId,
        item_id: T::ItemId,
        amount: T::Balance,
    ) -> DispatchResult {
        let collection = T::VIPPCollectionId::get();
        Self::with_vipp_mutation(item_id, || T::Nfts::burn(&collection, &item_id, Some(account)))?;
        Self::decrease_thrsehold(account, amount)?;
        if T::Nfts::owned_in_collection(&collection, account).next().is_none() {
            Self::set_inactive_attribute_vipp(account)?;
        }
        T::OnVIPPChanged::burn_vipp_nft(account, item_id)
    }

    /// Permit only the scoped root repair to reconcile balances during maintenance.
    pub fn ensure_vipp_reconciliation_ready() -> DispatchResult {
        if VippRepairInProgress::<T>::get() {
            Ok(())
        } else {
            T::OnVIPPChanged::ensure_accounting_ready()
        }
    }

    fn repair_balance_with_limit(account: &T::AccountId, limit: T::Balance) -> DispatchResult {
        let owned = Self::owned_vipp_claims(account)?;
        ensure!(
            owned.is_empty() || Self::try_get_claim_balance(account)?.is_some(),
            Error::<T>::InvalidClaimAttribute
        );
        Self::check_threshold_with_limit(account, Some(limit))?;
        VippAccountingQuarantine::<T>::remove(account);
        VippBreachBalance::<T>::remove(account);
        T::OnVIPPChanged::reconcile_vipp(account)
    }

    /// Mark the NAC inactive after the last VIPP NFT is removed.
    fn set_inactive_attribute_vipp(account: &T::AccountId) -> DispatchResult {
        let (_, item_id) = Self::get_nac_level(account).ok_or(Error::<T>::NftNotFound)?;
        T::Nfts::set_attribute(&T::NftCollectionId::get(), &item_id, &VIPP_STATUS_EXIST, &[1])
    }

    /// Decrease threshold of NAC claim value.
    fn decrease_thrsehold(account: &T::AccountId, amount: T::Balance) -> DispatchResult {
        let collection = T::NftCollectionId::get();
        let item = T::Nfts::owned_in_collection(&collection, account)
            .next()
            .ok_or(Error::<T>::NftNotFound)?;

        let claimed_raw =
            T::Nfts::system_attribute(&collection, Some(&item), &CLAIM_AMOUNT_ATTRIBUTE_KEY)
                .ok_or(Error::<T>::InvalidClaimAttribute)?;
        let currently_claimed = T::Balance::decode_all(&mut claimed_raw.as_slice())
            .map_err(|_| Error::<T>::InvalidClaimAttribute)?;

        let updated_claimed = currently_claimed.saturating_sub(amount);

        T::Nfts::set_attribute(
            &collection,
            &item,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &updated_claimed.encode(),
        )
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

impl<T: Config, Balance> OnClaimHandler<T::AccountId, Balance, ()> for Pallet<T>
where
    Balance: frame_support::traits::tokens::Balance,
{
    #[frame_support::transactional]
    fn on_claim(who: &T::AccountId, amount: Balance, _data: Option<()>) -> DispatchResult {
        T::OnVIPPChanged::ensure_accounting_ready()?;
        ensure!(!Self::is_vipp_quarantined(who), Error::<T>::VippAccountingUnavailable);
        let collection = T::NftCollectionId::get();
        let item = T::Nfts::owned_in_collection(&collection, who)
            .next()
            .ok_or(Error::<T>::NftNotFound)?;

        let currently_claimed = match T::Nfts::system_attribute(
            &collection,
            Some(&item),
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
        ) {
            Some(bytes) => Balance::decode_all(&mut bytes.as_slice())
                .map_err(|_| Error::<T>::InvalidClaimAttribute)?,
            None => Balance::zero(),
        };

        let updated_claimed = currently_claimed.saturating_add(amount);

        T::Nfts::set_attribute(
            &collection,
            &item,
            &CLAIM_AMOUNT_ATTRIBUTE_KEY,
            &updated_claimed.encode(),
        )?;

        // An additional claim extends existing VIPP ownership. A claim on its own
        // does not establish VIPP membership before the user joins VIP.
        if T::Nfts::owned_in_collection(&T::VIPPCollectionId::get(), who).next().is_some() {
            if let Some(nft) = Self::can_mint_vipp(who)? {
                T::OnVIPPChanged::mint_vipp(who, nft.0, nft.1)?;
            }
        }

        // Check the new threshold after the claim has changed native balances
        // and vesting schedules; an earlier fee check cannot see this state.
        Self::check_observed_balance(who);

        Ok(())
    }
}

impl<T: Config> OnWithdrawFeeHandler<T::AccountId> for Pallet<T> {
    fn on_withdraw_fee(who: &T::AccountId) {
        Pallet::<T>::check_observed_balance(who);
    }
}

/// Handler for updating, burning VIPP status.
pub trait OnVippStatusHandler<AccountId, Balance, ItemId> {
    /// Populate the integrated membership cache in benchmark-only builds.
    #[cfg(feature = "runtime-benchmarks")]
    fn benchmark_prepare_member(_who: &AccountId, _stake: Balance) -> DispatchResult {
        Ok(())
    }
    /// Populate unrelated cache rows to measure membership-capacity scans.
    #[cfg(feature = "runtime-benchmarks")]
    fn benchmark_populate_members(_members: u32) {}
    /// Exercise the bounded initialization-breach queue at capacity.
    #[cfg(feature = "runtime-benchmarks")]
    fn benchmark_fill_breach_queue(_members: u32) {}
    /// Remember when an unresolved breach began, including across year rollover.
    fn on_balance_breach(_who: &AccountId) {}

    /// Whether an explicitly approved root repair can proceed in the current phase.
    fn ensure_repair_ready() -> DispatchResult {
        Self::ensure_accounting_ready()
    }
    /// Refresh verified ownership after a bounded accounting repair.
    fn reconcile_vipp(_who: &AccountId) -> DispatchResult {
        Ok(())
    }
    /// Settle elapsed periods or reject mutation while an explicit repair is active.
    fn ensure_accounting_ready() -> DispatchResult {
        Ok(())
    }
    /// Whether the account may receive a VIPP NFT through an accounted transfer.
    fn can_receive_vipp(_who: &AccountId) -> bool {
        false
    }
    /// Handle a minting new VIPP NFT.
    fn mint_vipp(who: &AccountId, _amount: Balance, item_id: ItemId) -> DispatchResult;

    /// Burning VIPP NFT.
    fn burn_vipp_nft(who: &AccountId, item_id: ItemId) -> DispatchResult;
}

/// Native funds still subject to vesting at the current block.
pub trait UnvestedBalance<AccountId, Balance> {
    /// Return unvested funds once, without adding overlapping locks or reserves.
    fn unvested_balance(who: &AccountId) -> Balance;

    /// Populate the runtime's maximum vesting state for integrated observer measurements.
    #[cfg(feature = "runtime-benchmarks")]
    fn benchmark_prepare(_who: &AccountId) -> DispatchResult {
        Ok(())
    }
}

impl<AccountId, Balance: Zero> UnvestedBalance<AccountId, Balance> for () {
    fn unvested_balance(_who: &AccountId) -> Balance {
        Balance::zero()
    }
}

impl<AccountId, Balance, ItemId> OnVippStatusHandler<AccountId, Balance, ItemId> for () {
    fn mint_vipp(_who: &AccountId, _amount: Balance, _item_id: ItemId) -> DispatchResult {
        Ok(())
    }
    fn burn_vipp_nft(_who: &AccountId, _item_id: ItemId) -> DispatchResult {
        Ok(())
    }
}
