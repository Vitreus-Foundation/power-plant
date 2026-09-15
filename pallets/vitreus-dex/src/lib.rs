//! # Vitreus DEX pallet
//!
//! A native AMM DEX pallet for the Vitreus blockchain. Scaffolded to mirror the
//! structure of `pallet-energy-broker`. Pallet index: 43. PalletId: `vtrs/dex`.
//!
//! Other pallets in the same runtime can create pools and provision liquidity
//! without dispatching extrinsics via the [`PoolManager`] trait (implemented
//! for [`Pallet`]), which wraps the origin-free `do_create_pool`,
//! `do_add_liquidity_for` and `do_lock_liquidity_for` helpers.
//!
//! # Part 3 integration notes (runtime wiring)
//!
//! When wiring this pallet into `runtime/vitreus/src/lib.rs`:
//!
//! 1. **Pallet index 43** — confirmed free between `DynamicEnergy = 42` and
//!    `Proxy = 44`. Use `VitreusDex: pallet_vitreus_dex = 43`.
//!
//! 2. **Energy fee routing** — `pallet_energy_fee` is the runtime's
//!    `OnChargeTransaction`. Add `RuntimeCall::VitreusDex(..)` to the
//!    `CallFee::Regular(Self::custom_fee())` match in
//!    `runtime/vitreus/src/lib.rs` (around the `CustomFee` impl) or leave
//!    it to the default `weight_fee` branch — decide per governance.
//!
//! 3. **`DefaultSolverBondAmount`** — do NOT use the mock value of
//!    `1_000_000_000_000` in production. VTRS has 18 decimals; use
//!    `1_000 * UNITS` (= 10^21) or the post-governance agreed value.
//!
//! 4. **Same-block bid races** — Vitreus runs BABE with rotational block
//!    authorship, and `pallet_energy_fee::pay_priority_fee` is a no-op so
//!    tips can't influence inclusion. Same-block overbidding is
//!    non-deterministic in ordering but economically sound: the current
//!    BABE author's mempool view decides which valid bid lands first;
//!    losers retry next block. No commit-reveal needed.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![allow(clippy::result_unit_err, clippy::too_many_arguments)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod settlement_integration_tests;

pub mod settlement;
pub mod weights;
#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub use weights::WeightInfo;
#[cfg(feature = "runtime-benchmarks")]
pub use benchmarking::BenchmarkHelper;

pub use pallet::*;

use frame_support::{
    dispatch::DispatchResult,
    traits::{
        fungibles::{Balanced, Inspect, Mutate},
        tokens::{
            Balance,
            Fortitude::Polite,
            Preservation::{Expendable, Preserve},
        },
        Contains,
    },
    PalletId,
};
use frame_system::pallet_prelude::BlockNumberFor;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Unsigned;
use sp_runtime::{
    traits::{
        AccountIdConversion, Bounded, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Ensure,
        IntegerSquareRoot, One, Saturating, Zero,
    },
    DispatchError, RuntimeDebug, SaturatedConversion,
};

/// The PalletId used to derive the DEX sovereign account.
pub const PALLET_ID: PalletId = PalletId(*b"vtrs/dex");

/// Denominator for the fee tier, expressed in 10ths of a percent.
pub const FEE_DENOMINATOR: u32 = 1_000;

/// Minimum liquidity permanently locked on first deposit to prevent first-depositor attacks.
pub const MINIMUM_LIQUIDITY: u32 = 1_000;

/// Basis-point denominator for fee routing.
pub const BPS: u32 = 10_000;

/// Upper bound on `protocol_bps + creator_bps` (D4). Equal to the smallest
/// whitelisted fee tier (0.1% = 10 bps) so that every tier can carry the
/// routed slices and the pool always keeps `fee_tier × 10 − routed ≥ 0`.
/// A default that no tier could honour would otherwise fail launchpad
/// graduations at seed time.
pub const MAX_ROUTED_BPS: u16 = 10;

/// D4: how a pool's swap fee is split. Snapshotted into [`PoolInfo`] when the
/// pool is created or seeded and never changed afterwards — the same
/// per-launch immutability the launchpad gives its curve terms. The pool
/// keeps `fee_tier × 10 − protocol_bps − creator_bps` bps.
///
/// Routed slices are always denominated in the native asset (VTRS): taken
/// from the input when VTRS is `asset_in`, from the output when it is
/// `asset_out`. Pools with no native side route nothing.
#[derive(Clone, Copy, Encode, Decode, Default, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct FeeRouting {
    /// Share of every swap sent to the protocol fee recipient, in bps of the swap.
    pub protocol_bps: u16,
    /// Share of every swap accrued for the pool's creator, in bps of the swap.
    pub creator_bps: u16,
}

impl FeeRouting {
    /// `protocol_bps + creator_bps`.
    pub fn routed_bps(&self) -> u16 {
        self.protocol_bps.saturating_add(self.creator_bps)
    }
    /// Whether the split is within [`MAX_ROUTED_BPS`].
    pub fn is_valid(&self) -> bool {
        self.routed_bps() <= MAX_ROUTED_BPS
    }
}

/// D4: resolves who may claim a pool's accrued creator share. The DEX has
/// no notion of a creator; the runtime binds this to the launchpad
/// (`AssetToLaunch → Launches[id].creator_fee_recipient`) so there is one
/// source of truth and `set_creator_fee_recipient` needs no propagation.
/// Consulted only at claim time, never inside a swap.
pub trait CreatorFeeRecipient<AssetKind, AccountId> {
    /// The account entitled to `asset`'s creator share, if any.
    fn creator_fee_recipient(asset: &AssetKind) -> Option<AccountId>;
}

impl<AssetKind, AccountId> CreatorFeeRecipient<AssetKind, AccountId> for () {
    fn creator_fee_recipient(_: &AssetKind) -> Option<AccountId> {
        None
    }
}

/// On-chain record of a trading pair's reserves, fee tier and dedicated sub-account.
///
/// **Quoters: price from the pool account's live balances, not from
/// `reserve_a` / `reserve_b`.** `do_swap` calls `sync_reserves` before it
/// prices, which re-reads the balances, so the stored reserves are only a
/// snapshot as of the last sync: they lag by the pool's share of every fee
/// since (it stays in the account uncounted, Finding 3) and by any direct
/// transfer. A quote computed from the stored fields overstates the output
/// and the chain then fails the swap with `SlippageExceeded` at a tight
/// `amount_out_min`. The frontend's swap page reads balances for this reason.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PoolInfo<Balance, AccountId> {
    /// Reserve of `asset_a` as of the last `sync_reserves`; see the struct docs
    /// before pricing from this.
    pub reserve_a: Balance,
    /// Reserve of `asset_b` as of the last `sync_reserves`; see the struct docs
    /// before pricing from this.
    pub reserve_b: Balance,
    /// Swap fee tier for this pool, expressed in 10ths of a percent.
    pub fee_tier: u32,
    /// Cumulative fees collected over the pool's lifetime.
    pub total_fees_collected: Balance,
    /// Sub-account that physically holds the pool's reserves.
    pub pool_account: AccountId,
    /// D4: fee split snapshotted at creation / seeding. Zero for pools that
    /// predate D4 (migration v1) and for pools with no native side.
    pub routing: FeeRouting,
}

/// On-chain record of a single liquidity provider's position in a pool.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LiquidityPosition<Balance, BlockNumber> {
    /// LP shares owned by the provider.
    pub shares: Balance,
    /// Block at which the position was opened.
    pub entry_block: BlockNumber,
    /// Block until which the position is locked, if any.
    pub locked_until: Option<BlockNumber>,
}

/// In-runtime interface for pallets that need to create DEX pools and seed
/// them with liquidity without going through the extrinsic layer.
///
/// None of these methods perform origin checks — the extrinsics in this
/// pallet gate the same logic on `ManageOrigin` / `ensure_signed`, and an
/// implementing pallet is expected to apply its own authorisation before
/// calling. Implemented for [`Pallet<T>`]; bind it in a dependent pallet's
/// `Config` as
/// `type Dex: PoolManager<Self::AccountId, AssetKind, Balance, BlockNumberFor<Self>>`.
pub trait PoolManager<AccountId, AssetKind, Balance, BlockNumber> {
    /// Whether a pool exists for the (unordered) asset pair.
    fn pool_exists(asset_a: AssetKind, asset_b: AssetKind) -> bool;

    /// Create a pool for the pair. `fee_tier` is in 10ths of a percent and
    /// must be one of the whitelisted tiers (1, 3, 10).
    fn create_pool(asset_a: AssetKind, asset_b: AssetKind, fee_tier: u32) -> DispatchResult;

    /// Add liquidity from `who`'s balances, crediting the LP position to
    /// `who`. Returns the LP shares minted. `amount_a`/`amount_a_min` belong
    /// to `asset_a` and `amount_b`/`amount_b_min` to `asset_b` in whatever
    /// order the caller passes them (D5); the pool's canonical order is an
    /// implementation detail.
    fn add_liquidity_for(
        who: &AccountId,
        asset_a: AssetKind,
        asset_b: AssetKind,
        amount_a: Balance,
        amount_b: Balance,
        amount_a_min: Balance,
        amount_b_min: Balance,
    ) -> Result<Balance, DispatchError>;

    /// Extend the lock on `who`'s LP position in the pool to `lock_until`.
    /// A lock can only ever be extended: passing a block earlier than the
    /// current lock fails with `LockCannotBeShortened`.
    fn lock_liquidity_for(
        who: &AccountId,
        asset_a: AssetKind,
        asset_b: AssetKind,
        lock_until: BlockNumber,
    ) -> DispatchResult;
}

/// The single entry point through which a pool for a *reserved* asset (see
/// [`Config::ReservedAssets`]) can come into existence. Reserved assets are
/// rejected by every other pool-creation path, including `ManageOrigin`.
///
/// Intended for the launchpad pallet's graduation step. The implementation
/// is transactional and, in order:
///
/// 1. requires `asset` to be reserved and `quote` not to be;
/// 2. creates the pool, or adopts an existing pool that has never received
///    liquidity (`TotalLiquidity == 0`); a pool that already holds shares is
///    rejected with `PoolAlreadySeeded`;
/// 3. sweeps any balance the pool's sub-account already holds in either asset
///    to [`Config::ExcessRecipient`], so a donation parked at the (predictable)
///    pool address before seeding cannot be absorbed by `sync_reserves` into
///    the opening price. If the recipient cannot receive the asset the whole
///    seed fails with `ExcessRecipientCannotReceive` — loudly, so a mis-wired
///    recipient is noticed and fixed, after which seeding can be retried;
/// 4. pulls exactly `(amount_quote, amount_asset)` from `who` — quote first, so
///    the pool account has a provider before it receives a possibly
///    non-sufficient asset — as the pool's first deposit;
/// 5. locks `who`'s position until `BlockNumber::max_value()`.
///
/// No origin check is performed; only the launchpad binds this trait and the
/// DEX exposes no extrinsic that reaches it.
pub trait ReservedPoolSeeder<AccountId, AssetKind, Balance, BlockNumber> {
    /// Returns the LP shares credited to `who` (net of `MINIMUM_LIQUIDITY`).
    fn seed_reserved_pool_for(
        who: &AccountId,
        asset: AssetKind,
        quote: AssetKind,
        amount_asset: Balance,
        amount_quote: Balance,
        fee_tier: u32,
    ) -> Result<Balance, DispatchError>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The origin which can manage parameters of this pallet.
        type ManageOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// The type in which the assets for swapping are measured.
        type Balance: Balance;

        /// Wide integer used for every multiply-then-divide in the pool math so
        /// reserve products cannot overflow. Balances are 18-decimal `u128`s, so
        /// a product of two realistic reserves (10^22 × 2·10^26) is ~10^48 and
        /// does not fit `u128`. Bind to `sp_core::U256`, as `pallet_energy_broker`
        /// does. Results are narrowed back with a checked conversion; a value
        /// that does not fit `Balance` is an [`Error::Overflow`], never a
        /// truncation.
        type HigherPrecisionBalance: IntegerSquareRoot
            + One
            + Ensure
            + Unsigned
            + From<u32>
            + From<Self::Balance>
            + TryInto<Self::Balance>;

        /// Type of asset class used to provide liquidity.
        type AssetKind: Parameter + MaxEncodedLen;

        /// Registry of assets utilized for providing liquidity.
        type Assets: Inspect<Self::AccountId, AssetId = Self::AssetKind, Balance = Self::Balance>
            + Mutate<Self::AccountId>
            + Balanced<Self::AccountId>;

        /// Identifier of native asset.
        #[pallet::constant]
        type NativeAsset: Get<Self::AssetKind>;

        /// Identifier of energy asset.
        #[pallet::constant]
        type EnergyAsset: Get<Self::AssetKind>;

        /// Assets for which pools may only be created through
        /// [`ReservedPoolSeeder::seed_reserved_pool_for`]. `do_create_pool`
        /// rejects them for every caller, `ManageOrigin` included. The
        /// runtime binds this to the launchpad's asset-id range.
        type ReservedAssets: Contains<Self::AssetKind>;

        /// Recipient of any balance found in a reserved pool's sub-account
        /// before it is seeded (see [`ReservedPoolSeeder`]).
        type ExcessRecipient: Get<Self::AccountId>;

        /// D4: where routed protocol fees go while [`ProtocolFeeRecipient`]
        /// is unset. The runtime binds this to the runtime Treasury. The
        /// destination is deliberately storage-settable rather than a
        /// constant: the Treasury is not a store (its extension recycles a
        /// fraction of the balance to staking every spend period), so
        /// governance must be able to redirect revenue without a runtime
        /// upgrade.
        type DefaultProtocolFeeRecipient: Get<Self::AccountId>;

        /// D4: who may claim a pool's creator share. `()` means nobody (no
        /// creators; creator slices then fold into the pool at snapshot).
        type CreatorFeeRecipient: CreatorFeeRecipient<Self::AssetKind, Self::AccountId>;

        // ---- Solver marketplace config ----

        /// Initial default for the bid window in blocks. Can be updated at
        /// runtime via `set_bid_window` (gated on `ManageOrigin`).
        #[pallet::constant]
        type DefaultBidWindowBlocks: Get<BlockNumberFor<Self>>;

        /// Initial default for the settlement window in blocks.
        #[pallet::constant]
        type DefaultSettlementWindowBlocks: Get<BlockNumberFor<Self>>;

        /// Initial default for the solver bond amount.
        ///
        /// Denominated in the native-asset sub-unit (VTRS uses 18 decimals
        /// in the production runtime, so 1 VTRS = 10^18 sub-units). The
        /// production runtime should bind this to something on the order of
        /// `1_000 * UNITS` (≈ 10^21 sub-units); the test mock binds it to
        /// `1_000_000_000_000` which is fine for tests but equals only
        /// 10^-6 VTRS in production denominations.
        #[pallet::constant]
        type DefaultSolverBondAmount: Get<Self::Balance>;

        /// Weight information for the extrinsics of this pallet.
        type WeightInfo: WeightInfo;

        /// Supplies non-native asset identifiers for benchmarks.
        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: BenchmarkHelper<Self::AssetKind, Self::AccountId>;
    }

    /// All known pools keyed by their canonical ordered asset pair.
    #[pallet::storage]
    pub type Pools<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        (T::AssetKind, T::AssetKind),
        PoolInfo<T::Balance, T::AccountId>,
    >;

    /// Per-provider liquidity positions keyed by account and pool.
    #[pallet::storage]
    pub type LiquidityPositions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        (T::AssetKind, T::AssetKind),
        LiquidityPosition<T::Balance, BlockNumberFor<T>>,
    >;

    /// Total LP shares outstanding for each pool.
    #[pallet::storage]
    pub type TotalLiquidity<T: Config> =
        StorageMap<_, Blake2_128Concat, (T::AssetKind, T::AssetKind), T::Balance>;

    /// D4: fee split applied to pools created or seeded from now on. Changing
    /// it never touches an existing pool (each pool carries its own snapshot).
    #[pallet::storage]
    pub type DefaultFeeRouting<T: Config> = StorageValue<_, FeeRouting, ValueQuery>;

    /// D4: where protocol fees are paid on `withdraw_protocol_fees`. `None`
    /// means `T::DefaultProtocolFeeRecipient` (the runtime Treasury).
    #[pallet::storage]
    pub type ProtocolFeeRecipient<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// D4: native-asset creator fees accrued per pool, held in the fee
    /// escrow sub-account until `claim_pool_creator_fees`.
    #[pallet::storage]
    pub type CreatorFeesUnclaimed<T: Config> =
        StorageMap<_, Blake2_128Concat, (T::AssetKind, T::AssetKind), T::Balance, ValueQuery>;

    /// D4: native-asset protocol fees accrued across all pools, held in the
    /// fee escrow sub-account until `withdraw_protocol_fees`.
    #[pallet::storage]
    pub type ProtocolFeesUnclaimed<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

    // ---- Settlement: governance-adjustable parameters ----

    /// Number of blocks during which solvers may bid on an open intent.
    /// `None` means fall back to `T::DefaultBidWindowBlocks`.
    #[pallet::storage]
    pub type BidWindowBlocks<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    /// Number of blocks a committed solver has to settle before becoming
    /// slashable. `None` means fall back to `T::DefaultSettlementWindowBlocks`.
    #[pallet::storage]
    pub type SettlementWindowBlocks<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    /// Amount of VTRS a solver must bond to register.
    /// `None` means fall back to `T::DefaultSolverBondAmount`.
    #[pallet::storage]
    pub type SolverBondAmount<T: Config> = StorageValue<_, T::Balance, OptionQuery>;

    // ---- Settlement: id counters ----

    /// Monotonic id for the next submitted intent.
    #[pallet::storage]
    pub type NextIntentId<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// Monotonic id for the next registered solver.
    #[pallet::storage]
    pub type NextSolverId<T: Config> = StorageValue<_, u64, ValueQuery>;

    // ---- Settlement: data maps ----

    /// All intents by id.
    #[pallet::storage]
    pub type Intents<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        crate::settlement::Intent<T::AccountId, T::AssetKind, T::Balance, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Index from solver account to solver id, for uniqueness enforcement
    /// and fast lookup at register time.
    #[pallet::storage]
    pub type SolverAccountToId<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u64,
        OptionQuery,
    >;

    /// All solvers by id.
    #[pallet::storage]
    pub type Solvers<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        crate::settlement::SolverInfo<T::AccountId, T::Balance, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Active commitments keyed by intent id. Removed on settle/cancel/slash.
    #[pallet::storage]
    pub type FillCommitments<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        crate::settlement::FillCommitment<T::AccountId, T::Balance, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Amount held in the shared intent escrow per-intent. Tracks how much
    /// `token_in` the escrow account owes back to each intent owner.
    /// Insert on `submit_intent`, remove on settle / cancel / refund.
    #[pallet::storage]
    pub type IntentEscrowBalances<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        (T::AssetKind, T::Balance),
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new pool has been created.
        PoolCreated {
            /// First asset in the pair.
            asset_a: T::AssetKind,
            /// Second asset in the pair.
            asset_b: T::AssetKind,
            /// Fee tier for the new pool (10ths of a percent).
            fee_tier: u32,
        },
        /// Liquidity has been added to a pool.
        LiquidityAdded {
            /// Account providing the liquidity.
            provider: T::AccountId,
            /// First asset in the pair.
            asset_a: T::AssetKind,
            /// Second asset in the pair.
            asset_b: T::AssetKind,
            /// Amount of `asset_a` deposited.
            amount_a: T::Balance,
            /// Amount of `asset_b` deposited.
            amount_b: T::Balance,
            /// LP shares minted to the provider.
            shares_minted: T::Balance,
        },
        /// Liquidity has been removed from a pool.
        LiquidityRemoved {
            /// Account withdrawing the liquidity.
            provider: T::AccountId,
            /// First asset in the pair.
            asset_a: T::AssetKind,
            /// Second asset in the pair.
            asset_b: T::AssetKind,
            /// Amount of `asset_a` returned.
            amount_a: T::Balance,
            /// Amount of `asset_b` returned.
            amount_b: T::Balance,
            /// LP shares burned from the provider.
            shares_burned: T::Balance,
        },
        /// A swap has been executed against a pool.
        SwapExecuted {
            /// Originator of the swap.
            who: T::AccountId,
            /// Input asset.
            asset_in: T::AssetKind,
            /// Output asset.
            asset_out: T::AssetKind,
            /// Amount of `asset_in` consumed.
            amount_in: T::Balance,
            /// Amount of `asset_out` produced.
            amount_out: T::Balance,
            /// Fee charged on this swap.
            fee: T::Balance,
        },
        /// Accumulated fees have been collected from a pool.
        FeesCollected {
            /// Pool whose fees were collected.
            pool: (T::AssetKind, T::AssetKind),
            /// Total fee amount paid out.
            amount: T::Balance,
            /// Beneficiary of the collected fees.
            recipient: T::AccountId,
        },
        /// D4: routed slices of a swap fee were moved to the fee escrow.
        FeesRouted {
            /// Pool the swap ran against (canonical order).
            pool: (T::AssetKind, T::AssetKind),
            /// Native amount accrued for the protocol.
            protocol: T::Balance,
            /// Native amount accrued for the pool's creator.
            creator: T::Balance,
        },
        /// D4: a creator claimed their accrued share for a pool.
        CreatorFeesClaimed {
            /// The pool (canonical order).
            pool: (T::AssetKind, T::AssetKind),
            /// Who was paid.
            recipient: T::AccountId,
            /// Native amount paid.
            amount: T::Balance,
        },
        /// D4: accrued protocol fees were paid to the current recipient.
        ProtocolFeesWithdrawn {
            /// Who was paid.
            recipient: T::AccountId,
            /// Native amount paid.
            amount: T::Balance,
        },
        /// D4: governance changed the split for pools created from now on.
        DefaultFeeRoutingSet {
            /// The new default.
            routing: FeeRouting,
        },
        /// D4: governance changed where protocol fees are paid.
        ProtocolFeeRecipientSet {
            /// The new recipient; `None` restores the runtime default.
            recipient: Option<T::AccountId>,
        },
        /// A liquidity position has been locked until a given block.
        LiquidityLocked {
            /// The account that locked the position.
            who: T::AccountId,
            /// The pool pair.
            pool: (T::AssetKind, T::AssetKind),
            /// The block until which the position is locked.
            locked_until: BlockNumberFor<T>,
        },
        /// A balance found in a reserved pool's sub-account before seeding was
        /// moved to `ExcessRecipient` so it could not affect the opening price.
        PreSeedBalanceSwept {
            /// The pool pair (canonical order).
            pool: (T::AssetKind, T::AssetKind),
            /// The asset that was swept.
            asset: T::AssetKind,
            /// Amount moved out of the pool sub-account.
            amount: T::Balance,
            /// Where it went.
            to: T::AccountId,
        },
        /// A reserved-asset pool was seeded through `ReservedPoolSeeder` and
        /// its first position locked permanently.
        ReservedPoolSeeded {
            /// Account whose balances funded the seed and who owns the locked position.
            who: T::AccountId,
            /// The reserved asset.
            asset: T::AssetKind,
            /// The quote asset.
            quote: T::AssetKind,
            /// Reserved-asset amount deposited.
            amount_asset: T::Balance,
            /// Quote amount deposited.
            amount_quote: T::Balance,
            /// LP shares credited to `who`.
            shares: T::Balance,
        },

        // ---- Solver marketplace events ----

        /// A new solver registered and posted a bond.
        SolverRegistered {
            /// Id assigned to the new solver.
            solver_id: u64,
            /// Solver's on-chain account.
            account: T::AccountId,
            /// Amount of bond posted.
            bond: T::Balance,
        },

        /// A solver voluntarily deregistered; bond refunded.
        SolverDeregistered {
            /// Id of the solver.
            solver_id: u64,
            /// Solver's on-chain account.
            account: T::AccountId,
            /// Amount refunded from escrow.
            bond_refunded: T::Balance,
        },

        /// A new intent was submitted.
        IntentSubmitted {
            /// Id assigned to the new intent.
            intent_id: u64,
            /// Submitting user.
            user: T::AccountId,
            /// Input asset.
            token_in: T::AssetKind,
            /// Desired output asset.
            token_out: T::AssetKind,
            /// Amount of input provided.
            amount_in: T::Balance,
            /// Minimum acceptable output.
            min_amount_out: T::Balance,
            /// Block by which the intent must settle or be refundable.
            deadline: BlockNumberFor<T>,
        },

        /// A user cancelled an intent before any commitment.
        IntentCancelled {
            /// Id of the cancelled intent.
            intent_id: u64,
            /// User who cancelled.
            user: T::AccountId,
        },

        /// A solver committed to fill an intent.
        FillCommitted {
            /// Intent being filled.
            intent_id: u64,
            /// Solver making the commitment.
            solver_id: u64,
            /// Amount the solver promises to deliver to the user.
            committed_amount_out: T::Balance,
            /// Deadline for settlement.
            settle_by: BlockNumberFor<T>,
        },

        /// An intent was successfully settled.
        IntentSettled {
            /// Intent that was settled.
            intent_id: u64,
            /// Solver that settled it.
            solver_id: u64,
            /// User who submitted the intent.
            user: T::AccountId,
            /// Amount delivered to the user.
            amount_out_to_user: T::Balance,
            /// Solver's net profit after protocol fee.
            solver_net_profit: T::Balance,
            /// Protocol fee taken from solver profit.
            protocol_fee: T::Balance,
        },

        /// A solver was slashed for failing to settle within the window.
        SolverSlashed {
            /// Solver that was slashed.
            solver_id: u64,
            /// Intent whose non-settlement triggered the slash.
            intent_id: u64,
            /// Total amount slashed from the solver's bond.
            slashed_amount: T::Balance,
            /// Portion sent to the protocol treasury.
            to_treasury: T::Balance,
            /// Portion awarded to the slasher.
            to_slasher: T::Balance,
            /// Account that triggered the slash.
            slasher: T::AccountId,
        },

        /// An expired intent was refunded to its owner.
        IntentRefunded {
            /// Intent that was refunded.
            intent_id: u64,
            /// User who was refunded.
            user: T::AccountId,
            /// Amount of `token_in` returned to the user.
            amount_refunded: T::Balance,
        },

        /// Governance updated the bid window.
        BidWindowUpdated {
            /// New bid window value (in blocks).
            new_value: BlockNumberFor<T>,
        },

        /// Governance updated the settlement window.
        SettlementWindowUpdated {
            /// New settlement window value (in blocks).
            new_value: BlockNumberFor<T>,
        },

        /// Governance updated the solver bond amount.
        SolverBondAmountUpdated {
            /// New solver bond amount.
            new_value: T::Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// A pool already exists for this asset pair.
        PoolAlreadyExists,
        /// No pool exists for this asset pair.
        PoolNotFound,
        /// The pool does not hold enough liquidity to satisfy the operation.
        InsufficientLiquidity,
        /// The caller does not own enough LP shares.
        InsufficientShares,
        /// Calculated output falls outside the caller's slippage bounds.
        SlippageExceeded,
        /// The position or pool is currently locked.
        PoolLocked,
        /// The provided fee tier is not accepted.
        InvalidFeeTier,
        /// Amount can't be zero.
        ZeroAmount,
        /// An overflow happened.
        Overflow,
        /// Initial liquidity deposit is too small to exceed MINIMUM_LIQUIDITY.
        InsufficientInitialLiquidity,
        /// The pair contains a reserved asset; such pools can only be created
        /// through `ReservedPoolSeeder::seed_reserved_pool_for`.
        ReservedAsset,
        /// `seed_reserved_pool_for` was called for an asset that is not
        /// reserved, or with a reserved asset as the quote.
        NotReservedAsset,
        /// The reserved pool already holds liquidity and cannot be seeded again.
        PoolAlreadySeeded,
        /// A liquidity lock can be extended but never shortened.
        LockCannotBeShortened,
        /// A pre-seed balance in the pool sub-account could not be delivered to
        /// `ExcessRecipient` (for example, the recipient has no provider and the
        /// asset is not sufficient). Fix the recipient and retry the seed.
        ExcessRecipientCannotReceive,
        /// D4: `protocol_bps + creator_bps` exceeds `MAX_ROUTED_BPS`.
        InvalidFeeRouting,
        /// D4: no creator is known for this asset, so it has no creator share.
        NoCreatorForAsset,
        /// D4: the caller is not the asset's creator fee recipient.
        NotCreatorFeeRecipient,

        // ---- Solver marketplace errors ----
        /// Caller is not a registered solver.
        SolverNotRegistered,
        /// Account has already registered as a solver.
        SolverAlreadyRegistered,
        /// Solver exists but is not currently active.
        SolverNotActive,
        /// Caller does not hold enough VTRS to post the bond.
        InsufficientBondFunds,
        /// No intent with the given id.
        IntentNotFound,
        /// Operation requires the intent to be in `Open` status.
        IntentNotOpen,
        /// Operation requires the intent to be in `Committed` status.
        IntentNotCommitted,
        /// Intent's deadline has passed.
        IntentExpired,
        /// Caller is not the intent's original submitter.
        NotIntentOwner,
        /// Bid window has closed for this intent.
        BidWindowClosed,
        /// Committed amount out is below the intent's minimum.
        BelowMinAmountOut,
        /// Incoming bid is not strictly better than the existing one.
        BidNotBetter,
        /// Caller is not the solver that committed to this intent.
        NotCommittedSolver,
        /// Settlement deadline has already passed.
        SettlementWindowPassed,
        /// Settlement deadline has not yet passed (slashing requires it to).
        SettlementWindowNotPassed,
        /// Solver still has active commitments; cannot deregister yet.
        ActiveCommitmentsExist,
        /// Deadline is in the past or not far enough in the future.
        InvalidDeadline,
        /// Deadline has not yet passed; refund is not yet available.
        DeadlineNotPassed,
        /// Amount parameter is zero or otherwise invalid.
        InvalidAmount,
        /// Swap output did not meet the user's slippage bound.
        SlippageProtectionFailed,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create a new AMM pool for the given asset pair and fee tier.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_pool())]
        pub fn create_pool(
            origin: OriginFor<T>,
            asset_a: T::AssetKind,
            asset_b: T::AssetKind,
            fee_tier: u32,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;
            Self::do_create_pool(asset_a, asset_b, fee_tier)
        }

        /// Add liquidity to an existing pool.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::add_liquidity())]
        pub fn add_liquidity(
            origin: OriginFor<T>,
            asset_a: T::AssetKind,
            asset_b: T::AssetKind,
            amount_a: T::Balance,
            amount_b: T::Balance,
            amount_a_min: T::Balance,
            amount_b_min: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_add_liquidity_for(
                &who,
                asset_a,
                asset_b,
                amount_a,
                amount_b,
                amount_a_min,
                amount_b_min,
            )?;
            Ok(())
        }

        /// Remove liquidity from an existing pool.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::remove_liquidity())]
        pub fn remove_liquidity(
            origin: OriginFor<T>,
            asset_a: T::AssetKind,
            asset_b: T::AssetKind,
            shares: T::Balance,
            amount_a_min: T::Balance,
            amount_b_min: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(shares > Zero::zero(), Error::<T>::ZeroAmount);

            // Finding 5 + D5: canonicalize the pair AND the minimums, which the
            // caller gave in (asset_a, asset_b) order.
            let (pair, amount_a_min, amount_b_min) =
                Self::canonical_pair_with(asset_a, asset_b, amount_a_min, amount_b_min);
            let mut pool = Pools::<T>::get(&pair).ok_or(Error::<T>::PoolNotFound)?;

            // Finding 2: sync reserves from actual balances before computing withdrawal.
            Self::sync_reserves(&pair, &mut pool);

            let total_shares =
                TotalLiquidity::<T>::get(&pair).ok_or(Error::<T>::PoolNotFound)?;
            ensure!(!total_shares.is_zero(), Error::<T>::InsufficientLiquidity);

            let mut position = LiquidityPositions::<T>::get(&who, &pair)
                .ok_or(Error::<T>::InsufficientShares)?;
            ensure!(position.shares >= shares, Error::<T>::InsufficientShares);

            // Finding 10: enforce lock check.
            if let Some(until) = position.locked_until {
                let current = frame_system::Pallet::<T>::block_number();
                ensure!(current >= until, Error::<T>::PoolLocked);
            }

            // D1: wide precision; floor rounds the withdrawal DOWN, in the pool's favour.
            let amount_a = Self::mul_div_floor(shares, pool.reserve_a, total_shares)?;
            let amount_b = Self::mul_div_floor(shares, pool.reserve_b, total_shares)?;

            ensure!(amount_a >= amount_a_min, Error::<T>::SlippageExceeded);
            ensure!(amount_b >= amount_b_min, Error::<T>::SlippageExceeded);

            T::Assets::transfer(
                pair.0.clone(),
                &pool.pool_account,
                &who,
                amount_a,
                Expendable,
            )?;
            T::Assets::transfer(
                pair.1.clone(),
                &pool.pool_account,
                &who,
                amount_b,
                Expendable,
            )?;

            pool.reserve_a = pool
                .reserve_a
                .checked_sub(&amount_a)
                .ok_or(Error::<T>::InsufficientLiquidity)?;
            pool.reserve_b = pool
                .reserve_b
                .checked_sub(&amount_b)
                .ok_or(Error::<T>::InsufficientLiquidity)?;
            Pools::<T>::insert(&pair, pool);

            let new_total =
                total_shares.checked_sub(&shares).ok_or(Error::<T>::Overflow)?;
            TotalLiquidity::<T>::insert(&pair, new_total);

            position.shares =
                position.shares.checked_sub(&shares).ok_or(Error::<T>::Overflow)?;
            if position.shares.is_zero() {
                LiquidityPositions::<T>::remove(&who, &pair);
            } else {
                LiquidityPositions::<T>::insert(&who, &pair, position);
            }

            Self::deposit_event(Event::LiquidityRemoved {
                provider: who,
                asset_a: pair.0,
                asset_b: pair.1,
                amount_a,
                amount_b,
                shares_burned: shares,
            });
            Ok(())
        }

        /// Swap an exact amount of `asset_in` for as much `asset_out` as the pool yields.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::swap_exact_tokens_for_tokens())]
        pub fn swap_exact_tokens_for_tokens(
            origin: OriginFor<T>,
            asset_in: T::AssetKind,
            asset_out: T::AssetKind,
            amount_in: T::Balance,
            amount_out_min: T::Balance,
            recipient: T::AccountId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_swap(&who, asset_in, asset_out, amount_in, amount_out_min, &recipient)?;
            Ok(())
        }

        /// Lock a liquidity position until a given block, or extend an
        /// existing lock. A lock can never be shortened (D3): passing a block
        /// earlier than the current lock fails with `LockCannotBeShortened`.
        ///
        /// Finding 10: activates the previously dead `locked_until` field.
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::lock_liquidity())]
        pub fn lock_liquidity(
            origin: OriginFor<T>,
            asset_a: T::AssetKind,
            asset_b: T::AssetKind,
            lock_until: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::do_lock_liquidity_for(&who, asset_a, asset_b, lock_until)
        }

        // ---- Solver marketplace: solver lifecycle ----

        /// Register as a solver. Transfers the required bond from the caller
        /// to a per-solver escrow sub-account derived from the assigned id.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::register_solver())]
        pub fn register_solver(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Reject duplicate active registration. If a prior registration
            // exists but is inactive (deregistered / slashed), allow a fresh
            // registration with a new solver_id.
            if let Some(existing_id) = SolverAccountToId::<T>::get(&who) {
                if let Some(solver) = Solvers::<T>::get(existing_id) {
                    if solver.active {
                        return Err(Error::<T>::SolverAlreadyRegistered.into());
                    }
                }
            }

            let bond_amount = Self::current_solver_bond();
            let native_asset = T::NativeAsset::get();

            let solver_id = NextSolverId::<T>::get();
            let escrow = Self::solver_escrow_account(solver_id);

            T::Assets::transfer(
                native_asset,
                &who,
                &escrow,
                bond_amount,
                Preserve,
            )
            .map_err(|_| Error::<T>::InsufficientBondFunds)?;

            let now = frame_system::Pallet::<T>::block_number();
            let info = crate::settlement::SolverInfo {
                id: solver_id,
                account: who.clone(),
                bond: bond_amount,
                reputation: 0,
                fills_completed: 0,
                fills_slashed: 0,
                active_commitments: 0,
                registered_at: now,
                active: true,
            };

            Solvers::<T>::insert(solver_id, info);
            SolverAccountToId::<T>::insert(&who, solver_id);
            NextSolverId::<T>::put(solver_id.saturating_add(1));

            Self::deposit_event(Event::SolverRegistered {
                solver_id,
                account: who,
                bond: bond_amount,
            });

            Ok(())
        }

        /// Voluntarily deregister as a solver. Refunds the full bond.
        /// Fails if the solver has any active (committed but unsettled) fills.
        #[pallet::call_index(6)]
        #[pallet::weight(<T as Config>::WeightInfo::deregister_solver())]
        pub fn deregister_solver(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let solver_id = SolverAccountToId::<T>::get(&who)
                .ok_or(Error::<T>::SolverNotRegistered)?;
            let mut solver = Solvers::<T>::get(solver_id)
                .ok_or(Error::<T>::SolverNotRegistered)?;

            ensure!(solver.active, Error::<T>::SolverNotActive);
            ensure!(
                solver.active_commitments == 0,
                Error::<T>::ActiveCommitmentsExist,
            );

            let escrow = Self::solver_escrow_account(solver_id);
            let native_asset = T::NativeAsset::get();
            let bond = solver.bond;

            T::Assets::transfer(
                native_asset,
                &escrow,
                &who,
                bond,
                Expendable,
            )
            .map_err(|_| Error::<T>::InsufficientBondFunds)?;

            solver.active = false;
            solver.bond = Zero::zero();
            Solvers::<T>::insert(solver_id, solver);

            Self::deposit_event(Event::SolverDeregistered {
                solver_id,
                account: who,
                bond_refunded: bond,
            });

            Ok(())
        }

        // ---- Solver marketplace: intent lifecycle ----

        /// Submit a trading intent. Escrows `amount_in` of `token_in` to the
        /// shared intent escrow account. Solvers may bid during the next
        /// `current_bid_window()` blocks.
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::submit_intent())]
        pub fn submit_intent(
            origin: OriginFor<T>,
            token_in: T::AssetKind,
            token_out: T::AssetKind,
            amount_in: T::Balance,
            min_amount_out: T::Balance,
            deadline: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(!amount_in.is_zero(), Error::<T>::InvalidAmount);
            ensure!(token_in != token_out, Error::<T>::InvalidAmount);

            let now = frame_system::Pallet::<T>::block_number();
            ensure!(deadline > now, Error::<T>::InvalidDeadline);

            let escrow = Self::intent_escrow_account();
            T::Assets::transfer(
                token_in.clone(),
                &who,
                &escrow,
                amount_in,
                Preserve,
            )
            .map_err(|_| Error::<T>::InvalidAmount)?;

            let intent_id = NextIntentId::<T>::get();

            let intent = crate::settlement::Intent {
                id: intent_id,
                user: who.clone(),
                token_in: token_in.clone(),
                token_out: token_out.clone(),
                amount_in,
                min_amount_out,
                deadline,
                submitted_at: now,
                status: crate::settlement::IntentStatus::Open,
            };

            Intents::<T>::insert(intent_id, intent);
            IntentEscrowBalances::<T>::insert(intent_id, (token_in.clone(), amount_in));
            NextIntentId::<T>::put(intent_id.saturating_add(1));

            Self::deposit_event(Event::IntentSubmitted {
                intent_id,
                user: who,
                token_in,
                token_out,
                amount_in,
                min_amount_out,
                deadline,
            });

            Ok(())
        }

        /// Cancel an open intent. Only callable by the intent owner. Refunds
        /// `amount_in` of `token_in`. Only succeeds while the intent is
        /// still `Open` (no solver has committed).
        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::cancel_intent())]
        pub fn cancel_intent(origin: OriginFor<T>, intent_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut intent = Intents::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;

            ensure!(intent.user == who, Error::<T>::NotIntentOwner);
            ensure!(
                intent.status == crate::settlement::IntentStatus::Open,
                Error::<T>::IntentNotOpen,
            );

            // Use the recorded escrow balance (defense in depth — future
            // changes could adjust escrow mid-life).
            let (escrow_asset, escrow_amount) = IntentEscrowBalances::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;

            let escrow = Self::intent_escrow_account();
            T::Assets::transfer(
                escrow_asset,
                &escrow,
                &who,
                escrow_amount,
                Expendable,
            )
            .map_err(|_| Error::<T>::InvalidAmount)?;

            intent.status = crate::settlement::IntentStatus::Cancelled;
            Intents::<T>::insert(intent_id, intent);
            IntentEscrowBalances::<T>::remove(intent_id);

            Self::deposit_event(Event::IntentCancelled {
                intent_id,
                user: who,
            });

            Ok(())
        }

        // ---- Solver marketplace: fill lifecycle ----

        /// Commit to fill an open intent at `committed_amount_out`. Must be
        /// `>=` the intent's `min_amount_out`; if a prior commitment exists,
        /// the new bid must strictly exceed it (strict-replace overbidding).
        ///
        /// Must be called within the bid window
        /// (`intent.submitted_at + current_bid_window()`). Sets intent
        /// status to `Committed` and increments the solver's
        /// `active_commitments` counter. The solver's bond stays in the
        /// per-solver escrow regardless of commitment state; slashing on
        /// failed settlement draws against it.
        #[pallet::call_index(9)]
        #[pallet::weight(<T as Config>::WeightInfo::commit_fill())]
        pub fn commit_fill(
            origin: OriginFor<T>,
            intent_id: u64,
            committed_amount_out: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let solver_id = SolverAccountToId::<T>::get(&who)
                .ok_or(Error::<T>::SolverNotRegistered)?;
            let mut solver = Solvers::<T>::get(solver_id)
                .ok_or(Error::<T>::SolverNotRegistered)?;
            ensure!(solver.active, Error::<T>::SolverNotActive);

            let mut intent = Intents::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;
            ensure!(
                intent.status == crate::settlement::IntentStatus::Open
                    || intent.status == crate::settlement::IntentStatus::Committed,
                Error::<T>::IntentNotOpen,
            );

            ensure!(
                committed_amount_out >= intent.min_amount_out,
                Error::<T>::BelowMinAmountOut,
            );

            let now = frame_system::Pallet::<T>::block_number();
            let bid_deadline = intent.submitted_at.saturating_add(Self::current_bid_window());
            ensure!(now <= bid_deadline, Error::<T>::BidWindowClosed);

            ensure!(now < intent.deadline, Error::<T>::IntentExpired);

            // Strict-replace overbidding. Decrement the displaced solver's
            // active_commitments; missing lookup is treated as no-op since
            // the new commit is still valid.
            if let Some(prior) = FillCommitments::<T>::get(intent_id) {
                ensure!(
                    committed_amount_out > prior.committed_amount_out,
                    Error::<T>::BidNotBetter,
                );
                if let Some(mut displaced) = Solvers::<T>::get(prior.solver_id) {
                    displaced.active_commitments =
                        displaced.active_commitments.saturating_sub(1);
                    Solvers::<T>::insert(prior.solver_id, displaced);
                }
            }

            let settle_by = now.saturating_add(Self::current_settlement_window());
            let commitment = crate::settlement::FillCommitment {
                intent_id,
                solver_id,
                solver_account: who.clone(),
                committed_amount_out,
                committed_at: now,
                settle_by,
            };
            FillCommitments::<T>::insert(intent_id, commitment);

            solver.active_commitments = solver.active_commitments.saturating_add(1);
            Solvers::<T>::insert(solver_id, solver);

            intent.status = crate::settlement::IntentStatus::Committed;
            Intents::<T>::insert(intent_id, intent);

            Self::deposit_event(Event::FillCommitted {
                intent_id,
                solver_id,
                committed_amount_out,
                settle_by,
            });

            Ok(())
        }

        /// Settle a committed intent by executing the swap and distributing
        /// slippage capture.
        ///
        /// Only callable by the committed solver, within the settlement
        /// window. Calls `do_swap` with the solver's `committed_amount_out`
        /// as the minimum output; if the AMM can't deliver at least that,
        /// the whole call rolls back and the commitment remains (solver can
        /// retry or be slashed after the window).
        ///
        /// On success: user gets exactly `committed_amount_out`, treasury
        /// gets the protocol fee share of profit, solver pockets the rest.
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::settle_intent())]
        pub fn settle_intent(origin: OriginFor<T>, intent_id: u64) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let commitment = FillCommitments::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotCommitted)?;
            ensure!(
                commitment.solver_account == who,
                Error::<T>::NotCommittedSolver,
            );

            let now = frame_system::Pallet::<T>::block_number();
            ensure!(
                now <= commitment.settle_by,
                Error::<T>::SettlementWindowPassed,
            );

            let mut intent = Intents::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;
            ensure!(
                intent.status == crate::settlement::IntentStatus::Committed,
                Error::<T>::IntentNotCommitted,
            );

            let intent_escrow = Self::intent_escrow_account();
            let treasury = Self::protocol_treasury_account();

            // Execute swap through do_swap; AMM output lands back in intent_escrow.
            let actual_out = Self::do_swap(
                &intent_escrow,
                intent.token_in.clone(),
                intent.token_out.clone(),
                intent.amount_in,
                commitment.committed_amount_out,
                &intent_escrow,
            )?;

            // Slippage capture: do_swap guarantees actual_out >= committed_amount_out.
            let profit = actual_out.saturating_sub(commitment.committed_amount_out);

            let profit_u128: u128 = profit.saturated_into::<u128>();
            let (net_profit_u128, protocol_fee_u128) =
                crate::settlement::split_solver_profit_u128(
                    profit_u128,
                    crate::settlement::SOLVER_PROFIT_FEE_BPS,
                );
            let net_profit: T::Balance = net_profit_u128.saturated_into();
            let protocol_fee: T::Balance = protocol_fee_u128.saturated_into();

            // Deliver committed amount to user.
            T::Assets::transfer(
                intent.token_out.clone(),
                &intent_escrow,
                &intent.user,
                commitment.committed_amount_out,
                Expendable,
            )
            .map_err(|_| Error::<T>::InvalidAmount)?;

            if !protocol_fee.is_zero() {
                T::Assets::transfer(
                    intent.token_out.clone(),
                    &intent_escrow,
                    &treasury,
                    protocol_fee,
                    Expendable,
                )
                .map_err(|_| Error::<T>::InvalidAmount)?;
            }

            if !net_profit.is_zero() {
                T::Assets::transfer(
                    intent.token_out.clone(),
                    &intent_escrow,
                    &who,
                    net_profit,
                    Expendable,
                )
                .map_err(|_| Error::<T>::InvalidAmount)?;
            }

            IntentEscrowBalances::<T>::remove(intent_id);
            intent.status = crate::settlement::IntentStatus::Settled;
            Intents::<T>::insert(intent_id, intent.clone());
            FillCommitments::<T>::remove(intent_id);

            if let Some(mut solver) = Solvers::<T>::get(commitment.solver_id) {
                solver.reputation = solver
                    .reputation
                    .saturating_add(crate::settlement::REPUTATION_FILL_REWARD);
                solver.fills_completed = solver.fills_completed.saturating_add(1);
                solver.active_commitments = solver.active_commitments.saturating_sub(1);
                Solvers::<T>::insert(commitment.solver_id, solver);
            }

            Self::deposit_event(Event::IntentSettled {
                intent_id,
                solver_id: commitment.solver_id,
                user: intent.user,
                amount_out_to_user: commitment.committed_amount_out,
                solver_net_profit: net_profit,
                protocol_fee,
            });

            Ok(())
        }

        /// Slash a solver that failed to settle within the settlement window.
        /// Permissionless — anyone can call once `settle_by` has passed. The
        /// caller receives `SLASHER_REWARD_BPS` of the bond; the rest goes
        /// to the protocol treasury. The user's intent is refunded.
        ///
        /// After slashing, the solver is marked inactive and must re-register
        /// with a fresh bond to participate again.
        #[pallet::call_index(11)]
        #[pallet::weight(<T as Config>::WeightInfo::slash_solver())]
        pub fn slash_solver(origin: OriginFor<T>, intent_id: u64) -> DispatchResult {
            let slasher = ensure_signed(origin)?;

            let commitment = FillCommitments::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotCommitted)?;

            let now = frame_system::Pallet::<T>::block_number();
            ensure!(
                now > commitment.settle_by,
                Error::<T>::SettlementWindowNotPassed,
            );

            let mut intent = Intents::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;
            ensure!(
                intent.status == crate::settlement::IntentStatus::Committed,
                Error::<T>::IntentNotCommitted,
            );

            let mut solver = Solvers::<T>::get(commitment.solver_id)
                .ok_or(Error::<T>::SolverNotRegistered)?;
            let bond = solver.bond;

            let bond_u128: u128 = bond.saturated_into::<u128>();
            let (to_treasury_u128, to_slasher_u128) =
                crate::settlement::split_slashed_bond_u128(
                    bond_u128,
                    crate::settlement::SLASHER_REWARD_BPS,
                );
            let to_treasury: T::Balance = to_treasury_u128.saturated_into();
            let to_slasher: T::Balance = to_slasher_u128.saturated_into();

            let solver_escrow = Self::solver_escrow_account(commitment.solver_id);
            let treasury = Self::protocol_treasury_account();
            let native_asset = T::NativeAsset::get();

            if !to_treasury.is_zero() {
                T::Assets::transfer(
                    native_asset.clone(),
                    &solver_escrow,
                    &treasury,
                    to_treasury,
                    Expendable,
                )
                .map_err(|_| Error::<T>::InsufficientBondFunds)?;
            }

            if !to_slasher.is_zero() {
                T::Assets::transfer(
                    native_asset.clone(),
                    &solver_escrow,
                    &slasher,
                    to_slasher,
                    Expendable,
                )
                .map_err(|_| Error::<T>::InsufficientBondFunds)?;
            }

            // Refund the user.
            let (escrow_asset, escrow_amount) = IntentEscrowBalances::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;
            let intent_escrow = Self::intent_escrow_account();

            T::Assets::transfer(
                escrow_asset,
                &intent_escrow,
                &intent.user,
                escrow_amount,
                Expendable,
            )
            .map_err(|_| Error::<T>::InvalidAmount)?;

            IntentEscrowBalances::<T>::remove(intent_id);
            intent.status = crate::settlement::IntentStatus::Expired;
            Intents::<T>::insert(intent_id, intent.clone());
            FillCommitments::<T>::remove(intent_id);

            solver.bond = Zero::zero();
            solver.reputation = solver
                .reputation
                .saturating_add(crate::settlement::REPUTATION_SLASH_PENALTY);
            solver.fills_slashed = solver.fills_slashed.saturating_add(1);
            solver.active_commitments = solver.active_commitments.saturating_sub(1);
            solver.active = false;
            Solvers::<T>::insert(commitment.solver_id, solver);

            Self::deposit_event(Event::SolverSlashed {
                solver_id: commitment.solver_id,
                intent_id,
                slashed_amount: bond,
                to_treasury,
                to_slasher,
                slasher,
            });

            Self::deposit_event(Event::IntentRefunded {
                intent_id,
                user: intent.user,
                amount_refunded: escrow_amount,
            });

            Ok(())
        }

        /// Refund an intent that expired without ever being committed.
        /// Callable by the intent owner once the deadline has passed, only
        /// while the intent is still `Open`. Intents that were committed but
        /// not settled go through `slash_solver` instead.
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::refund_expired_intent())]
        pub fn refund_expired_intent(
            origin: OriginFor<T>,
            intent_id: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let mut intent = Intents::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;

            ensure!(intent.user == who, Error::<T>::NotIntentOwner);
            ensure!(
                intent.status == crate::settlement::IntentStatus::Open,
                Error::<T>::IntentNotOpen,
            );

            let now = frame_system::Pallet::<T>::block_number();
            ensure!(now >= intent.deadline, Error::<T>::DeadlineNotPassed);

            let (escrow_asset, escrow_amount) = IntentEscrowBalances::<T>::get(intent_id)
                .ok_or(Error::<T>::IntentNotFound)?;
            let intent_escrow = Self::intent_escrow_account();

            T::Assets::transfer(
                escrow_asset,
                &intent_escrow,
                &who,
                escrow_amount,
                Expendable,
            )
            .map_err(|_| Error::<T>::InvalidAmount)?;

            IntentEscrowBalances::<T>::remove(intent_id);
            intent.status = crate::settlement::IntentStatus::Expired;
            Intents::<T>::insert(intent_id, intent);

            Self::deposit_event(Event::IntentRefunded {
                intent_id,
                user: who,
                amount_refunded: escrow_amount,
            });

            Ok(())
        }

        // ---- Solver marketplace: governance setters ----

        /// Update the bid window (in blocks). Gated on `ManageOrigin`.
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::set_bid_window())]
        pub fn set_bid_window(
            origin: OriginFor<T>,
            new_value: BlockNumberFor<T>,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;
            ensure!(!new_value.is_zero(), Error::<T>::InvalidAmount);
            BidWindowBlocks::<T>::put(new_value);
            Self::deposit_event(Event::BidWindowUpdated { new_value });
            Ok(())
        }

        /// Update the settlement window (in blocks). Gated on `ManageOrigin`.
        #[pallet::call_index(14)]
        #[pallet::weight(<T as Config>::WeightInfo::set_settlement_window())]
        pub fn set_settlement_window(
            origin: OriginFor<T>,
            new_value: BlockNumberFor<T>,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;
            ensure!(!new_value.is_zero(), Error::<T>::InvalidAmount);
            SettlementWindowBlocks::<T>::put(new_value);
            Self::deposit_event(Event::SettlementWindowUpdated { new_value });
            Ok(())
        }

        /// Update the required solver bond amount. Gated on `ManageOrigin`.
        /// Does not retroactively affect solvers already bonded at the prior
        /// amount; only applies to new registrations.
        #[pallet::call_index(15)]
        #[pallet::weight(<T as Config>::WeightInfo::set_solver_bond_amount())]
        pub fn set_solver_bond_amount(
            origin: OriginFor<T>,
            new_value: T::Balance,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;
            ensure!(!new_value.is_zero(), Error::<T>::InvalidAmount);
            SolverBondAmount::<T>::put(new_value);
            Self::deposit_event(Event::SolverBondAmountUpdated { new_value });
            Ok(())
        }

        // ---- D4: fee routing ------------------------------------------------

        /// D4: set the fee split for pools created or seeded from now on.
        /// Gated on `ManageOrigin`. Never changes an existing pool's split.
        /// `protocol_bps + creator_bps` must be `≤ MAX_ROUTED_BPS`.
        #[pallet::call_index(16)]
        #[pallet::weight(<T as Config>::WeightInfo::set_default_fee_routing())]
        pub fn set_default_fee_routing(
            origin: OriginFor<T>,
            protocol_bps: u16,
            creator_bps: u16,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;
            let routing = FeeRouting { protocol_bps, creator_bps };
            ensure!(routing.is_valid(), Error::<T>::InvalidFeeRouting);
            DefaultFeeRouting::<T>::put(routing);
            Self::deposit_event(Event::DefaultFeeRoutingSet { routing });
            Ok(())
        }

        /// D4: set where protocol fees are paid; `None` restores the runtime
        /// default (the Treasury). Gated on `ManageOrigin`. Takes effect on the
        /// next `withdraw_protocol_fees`; fees already accrued follow it.
        #[pallet::call_index(17)]
        #[pallet::weight(<T as Config>::WeightInfo::set_protocol_fee_recipient())]
        pub fn set_protocol_fee_recipient(
            origin: OriginFor<T>,
            recipient: Option<T::AccountId>,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;
            match &recipient {
                Some(r) => ProtocolFeeRecipient::<T>::put(r.clone()),
                None => ProtocolFeeRecipient::<T>::kill(),
            }
            Self::deposit_event(Event::ProtocolFeeRecipientSet { recipient });
            Ok(())
        }

        /// D4: pay the accrued creator share of `asset`'s native-quoted pool
        /// to its creator fee recipient. Only that recipient, as resolved by
        /// `T::CreatorFeeRecipient` at call time, may call.
        #[pallet::call_index(18)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_pool_creator_fees())]
        pub fn claim_pool_creator_fees(origin: OriginFor<T>, asset: T::AssetKind) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let recipient = T::CreatorFeeRecipient::creator_fee_recipient(&asset)
                .ok_or(Error::<T>::NoCreatorForAsset)?;
            ensure!(who == recipient, Error::<T>::NotCreatorFeeRecipient);
            let pair = Self::canonical_pair(asset, T::NativeAsset::get());
            let amount = CreatorFeesUnclaimed::<T>::take(&pair);
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            T::Assets::transfer(
                T::NativeAsset::get(),
                &Self::fee_escrow_account(),
                &who,
                amount,
                Expendable,
            )?;
            Self::deposit_event(Event::CreatorFeesClaimed { pool: pair, recipient: who, amount });
            Ok(())
        }

        /// D4: pay every accrued protocol fee to the current protocol fee
        /// recipient. Permissionless: revenue is pulled, so a recipient that
        /// cannot receive can never block a swap.
        #[pallet::call_index(19)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_protocol_fees())]
        pub fn withdraw_protocol_fees(origin: OriginFor<T>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let amount = ProtocolFeesUnclaimed::<T>::take();
            ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
            let recipient = Self::protocol_fee_recipient();
            T::Assets::transfer(
                T::NativeAsset::get(),
                &Self::fee_escrow_account(),
                &recipient,
                amount,
                Expendable,
            )?;
            Self::deposit_event(Event::ProtocolFeesWithdrawn { recipient, amount });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// The account ID of the Vitreus DEX.
        pub fn account_id() -> T::AccountId {
            AccountIdConversion::<T::AccountId>::into_account_truncating(&PALLET_ID)
        }

        /// D4: the sub-account that holds routed fees until they are claimed
        /// or withdrawn. Separate from every pool account so `sync_reserves`
        /// never counts accrued fees as depth.
        pub fn fee_escrow_account() -> T::AccountId {
            PALLET_ID.into_sub_account_truncating(b"fees")
        }

        /// D4: where protocol fees are paid right now — the storage override
        /// or the runtime default. Also read by the launchpad's `Treasury`
        /// binding so all protocol revenue lands in one place.
        pub fn protocol_fee_recipient() -> T::AccountId {
            ProtocolFeeRecipient::<T>::get().unwrap_or_else(T::DefaultProtocolFeeRecipient::get)
        }

        /// D4: the split a pool gets at creation. Snapshotted from the live
        /// default; a pool with no native side can route nothing and gets
        /// zero; `with_creator = false` (plain `create_pool`) folds the
        /// creator slice into the pool since nobody could claim it.
        fn routing_for_new_pool(pair: &(T::AssetKind, T::AssetKind), with_creator: bool) -> FeeRouting {
            let native = T::NativeAsset::get().encode();
            if pair.0.encode() != native && pair.1.encode() != native {
                return FeeRouting::default();
            }
            let d = DefaultFeeRouting::<T>::get();
            FeeRouting { protocol_bps: d.protocol_bps, creator_bps: if with_creator { d.creator_bps } else { 0 } }
        }

        /// `floor(amount × bps / BPS)`.
        fn bps_of(amount: T::Balance, bps: u16) -> Result<T::Balance, Error<T>> {
            if bps == 0 {
                return Ok(Zero::zero());
            }
            Self::mul_div_floor(amount, (bps as u32).into(), BPS.into())
        }

        /// Finding 5: canonicalize a pair so that the lexicographically smaller
        /// encoded asset comes first. Prevents duplicate pools for (A,B) vs (B,A).
        pub fn canonical_pair(
            a: T::AssetKind,
            b: T::AssetKind,
        ) -> (T::AssetKind, T::AssetKind) {
            if a.encode() <= b.encode() {
                (a, b)
            } else {
                (b, a)
            }
        }

        /// D5: canonicalize a pair together with two per-asset values given in
        /// the caller's order (amounts, minimums). Returns the canonical pair
        /// and the values in that same order, so `.1` always belongs to `.0`'s
        /// asset. Every entry point that accepts `(asset_a, asset_b, x_a, x_b)`
        /// must go through this rather than binding `x_a` to `pair.0` blindly.
        pub fn canonical_pair_with<V>(
            a: T::AssetKind,
            b: T::AssetKind,
            x_a: V,
            x_b: V,
        ) -> ((T::AssetKind, T::AssetKind), V, V) {
            if a.encode() <= b.encode() {
                ((a, b), x_a, x_b)
            } else {
                ((b, a), x_b, x_a)
            }
        }

        /// Finding 2: sync pool reserves from actual on-chain asset balances.
        /// Absorbs any direct transfers or previously uncounted fees into the
        /// reserve tracking so the AMM math operates on accurate figures.
        /// Runs at the top of `do_swap`, which is why an off-chain quote must
        /// start from the same balances (see [`PoolInfo`]).
        fn sync_reserves(
            pair: &(T::AssetKind, T::AssetKind),
            pool: &mut PoolInfo<T::Balance, T::AccountId>,
        ) {
            pool.reserve_a = T::Assets::balance(pair.0.clone(), &pool.pool_account);
            pool.reserve_b = T::Assets::balance(pair.1.clone(), &pool.pool_account);
        }

        // ---- Wide-precision arithmetic helpers (D1) --------------------------
        //
        // All pool math that multiplies two `Balance`s goes through these. The
        // rounding direction of every division is the same as the original
        // `u128` code (floor), so nothing here changes which side a rounding
        // favours; it only removes the overflow.

        /// Lift a `Balance` into the wide type.
        fn hp(b: T::Balance) -> T::HigherPrecisionBalance {
            T::HigherPrecisionBalance::from(b)
        }

        /// Narrow a wide value back to `Balance`, failing with `Overflow` rather
        /// than truncating.
        fn narrow(x: T::HigherPrecisionBalance) -> Result<T::Balance, Error<T>> {
            x.try_into().map_err(|_| Error::<T>::Overflow)
        }

        /// `floor(a * b / c)` computed in wide precision. Errors with `Overflow`
        /// if the product overflows the wide type or the result does not fit
        /// `Balance`, and with `InsufficientLiquidity` if `c == 0`.
        fn mul_div_floor(
            a: T::Balance,
            b: T::Balance,
            c: T::Balance,
        ) -> Result<T::Balance, Error<T>> {
            let q = Self::hp(a)
                .checked_mul(&Self::hp(b))
                .ok_or(Error::<T>::Overflow)?
                .checked_div(&Self::hp(c))
                .ok_or(Error::<T>::InsufficientLiquidity)?;
            Self::narrow(q)
        }

        /// `floor(sqrt(a * b))` computed in wide precision.
        fn sqrt_of_product(a: T::Balance, b: T::Balance) -> Result<T::Balance, Error<T>> {
            let p = Self::hp(a).checked_mul(&Self::hp(b)).ok_or(Error::<T>::Overflow)?;
            Self::narrow(p.integer_sqrt())
        }

        /// Whether a pool exists for the (unordered) asset pair.
        pub fn pool_exists(asset_a: T::AssetKind, asset_b: T::AssetKind) -> bool {
            Pools::<T>::contains_key(Self::canonical_pair(asset_a, asset_b))
        }

        /// Create a new AMM pool for the given asset pair and fee tier.
        ///
        /// Origin-free body of the `create_pool` extrinsic. The extrinsic gates
        /// this on `ManageOrigin`; in-runtime callers (see [`PoolManager`]) are
        /// trusted to apply their own authorisation before calling.
        pub fn do_create_pool(
            asset_a: T::AssetKind,
            asset_b: T::AssetKind,
            fee_tier: u32,
        ) -> DispatchResult {
            // D2: a reserved asset can only get a pool through
            // `seed_reserved_pool_for`. No caller is exempt.
            ensure!(
                !T::ReservedAssets::contains(&asset_a) && !T::ReservedAssets::contains(&asset_b),
                Error::<T>::ReservedAsset
            );
            Self::ensure_valid_fee_tier(fee_tier)?;

            // Finding 5: canonicalize pair to prevent duplicate pools.
            let pair = Self::canonical_pair(asset_a, asset_b);
            ensure!(!Pools::<T>::contains_key(&pair), Error::<T>::PoolAlreadyExists);

            // D4: no creator can exist for a governance-created pool.
            let routing = Self::routing_for_new_pool(&pair, false);
            Self::insert_new_pool(&pair, fee_tier, routing);
            Ok(())
        }

        /// Finding 4: whitelist allowed fee tiers (0.1%, 0.3%, 1.0%).
        fn ensure_valid_fee_tier(fee_tier: u32) -> DispatchResult {
            ensure!(
                fee_tier == 1 || fee_tier == 3 || fee_tier == 10,
                Error::<T>::InvalidFeeTier
            );
            Ok(())
        }

        /// The sub-account that holds a pool's reserves, for the (unordered) pair.
        ///
        /// Deterministic in the pair, so it can be computed before the pool
        /// exists — which is exactly why `seed_reserved_pool_for` sweeps it.
        ///
        /// D8 (Finding 13): the seed is `blake2_256` of the pair key, not the
        /// key itself. `into_sub_account_truncating` keeps only the first
        /// `size_of::<AccountId>()` bytes of `"modl" ++ PalletId ++ seed`;
        /// with the raw key as the seed an AccountId20 runtime kept eight
        /// bytes of it — `04 00 44 01` and the low four bytes of the second
        /// asset id for a native pair, so chain asset `n` and launch asset
        /// `2^64 + n` shared one pool account, and for a `(WithId, WithId)`
        /// pair the second asset never featured at all. Eight bytes of a
        /// hash do not collide. (Finding 6's length prefixes addressed
        /// ambiguity *within* the key; the key was then truncated anyway.)
        pub fn pool_account_for(asset_a: T::AssetKind, asset_b: T::AssetKind) -> T::AccountId {
            let pair = Self::canonical_pair(asset_a, asset_b);
            let pair_key = (pair.0.encode(), pair.1.encode());
            let seed = sp_io::hashing::blake2_256(&pair_key.encode());
            PALLET_ID.into_sub_account_truncating(seed)
        }

        /// Write a fresh, empty pool record for an already-canonical `pair`.
        /// Callers must have checked that no pool exists.
        fn insert_new_pool(pair: &(T::AssetKind, T::AssetKind), fee_tier: u32, routing: FeeRouting) {
            let pool = PoolInfo {
                reserve_a: Zero::zero(),
                reserve_b: Zero::zero(),
                fee_tier,
                total_fees_collected: Zero::zero(),
                pool_account: Self::pool_account_for(pair.0.clone(), pair.1.clone()),
                routing,
            };

            Pools::<T>::insert(pair, pool);
            TotalLiquidity::<T>::insert(pair, T::Balance::zero());

            Self::deposit_event(Event::PoolCreated {
                asset_a: pair.0.clone(),
                asset_b: pair.1.clone(),
                fee_tier,
            });
        }

        /// Body of [`ReservedPoolSeeder::seed_reserved_pool_for`]; see the
        /// trait docs for the contract. Transactional: any failure leaves no
        /// trace, including the sweep.
        #[frame_support::transactional]
        pub fn do_seed_reserved_pool_for(
            who: &T::AccountId,
            asset: T::AssetKind,
            quote: T::AssetKind,
            amount_asset: T::Balance,
            amount_quote: T::Balance,
            fee_tier: u32,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(
                amount_asset > Zero::zero() && amount_quote > Zero::zero(),
                Error::<T>::ZeroAmount
            );
            // Only reserved assets come through here, and only against a
            // non-reserved quote, so this path cannot be used to mint arbitrary
            // permanently-locked pools around `ManageOrigin`.
            ensure!(T::ReservedAssets::contains(&asset), Error::<T>::NotReservedAsset);
            ensure!(!T::ReservedAssets::contains(&quote), Error::<T>::NotReservedAsset);
            ensure!(asset.encode() != quote.encode(), Error::<T>::NotReservedAsset);
            Self::ensure_valid_fee_tier(fee_tier)?;

            // 1. Create, or adopt an empty pool.
            let pair = Self::canonical_pair(asset.clone(), quote.clone());
            match Pools::<T>::get(&pair) {
                // D4: a seeded pool has a creator (the launch's fee recipient),
                // so it snapshots the full default split. An adopted empty
                // record keeps whatever split it carries.
                None => Self::insert_new_pool(&pair, fee_tier, Self::routing_for_new_pool(&pair, true)),
                Some(existing) => {
                    let shares = TotalLiquidity::<T>::get(&pair).unwrap_or_else(Zero::zero);
                    ensure!(shares.is_zero(), Error::<T>::PoolAlreadySeeded);
                    ensure!(existing.fee_tier == fee_tier, Error::<T>::InvalidFeeTier);
                },
            }
            let mut pool = Pools::<T>::get(&pair).ok_or(Error::<T>::PoolNotFound)?;
            ensure!(
                pool.reserve_a.is_zero() && pool.reserve_b.is_zero(),
                Error::<T>::PoolAlreadySeeded
            );

            // 2. Sweep whatever already sits in the pool sub-account (FM-02).
            //    The reserved asset goes first: for a non-sufficient asset the
            //    pool account's asset balance holds a consumer reference on its
            //    native account, which must be released before the native
            //    balance can be swept to zero.
            let excess_to = T::ExcessRecipient::get();
            let sweep_order = if T::ReservedAssets::contains(&pair.0) {
                [pair.0.clone(), pair.1.clone()]
            } else {
                [pair.1.clone(), pair.0.clone()]
            };
            for swept in sweep_order {
                let held = T::Assets::reducible_balance(swept.clone(), &pool.pool_account, Expendable, Polite);
                if held.is_zero() {
                    continue;
                }
                // Deliver to the recipient. If it cannot take the asset (for
                // example a treasury with no provider cannot hold a
                // non-sufficient asset) fail the seed with a distinct error
                // rather than burning: a silent burn would make a mis-wired
                // recipient look like correct operation, whereas a loud failure
                // is fixed by re-wiring and retrying. The attempt runs in its
                // own storage layer so the underlying error leaves nothing
                // half-applied before the outer transactional rollback.
                frame_support::storage::with_storage_layer(|| {
                    T::Assets::transfer(swept.clone(), &pool.pool_account, &excess_to, held, Expendable)
                })
                .map_err(|_| Error::<T>::ExcessRecipientCannotReceive)?;
                Self::deposit_event(Event::PreSeedBalanceSwept {
                    pool: pair.clone(),
                    asset: swept,
                    amount: held,
                    to: excess_to.clone(),
                });
            }

            // 3. First deposit, with the amounts mapped onto the canonical
            //    slots. Quote is transferred FIRST regardless of canonical
            //    order: the pool account may have just been emptied (or never
            //    existed) and needs a provider before it can hold a
            //    non-sufficient asset. Do not rely on `canonical_pair` putting
            //    the native asset first.
            let asset_is_a = pair.0.encode() == asset.encode();
            let (amount_a, amount_b) =
                if asset_is_a { (amount_asset, amount_quote) } else { (amount_quote, amount_asset) };

            // Finding 1: first deposit — burn MINIMUM_LIQUIDITY shares permanently.
            let raw_shares = Self::sqrt_of_product(amount_a, amount_b)?;
            let min_liq: T::Balance = MINIMUM_LIQUIDITY.into();
            ensure!(raw_shares > min_liq, Error::<T>::InsufficientInitialLiquidity);
            let shares_to_mint = raw_shares.checked_sub(&min_liq).ok_or(Error::<T>::Overflow)?;
            ensure!(shares_to_mint > Zero::zero(), Error::<T>::ZeroAmount);

            T::Assets::transfer(quote.clone(), who, &pool.pool_account, amount_quote, Expendable)?;
            T::Assets::transfer(asset.clone(), who, &pool.pool_account, amount_asset, Expendable)?;

            pool.reserve_a = amount_a;
            pool.reserve_b = amount_b;
            Pools::<T>::insert(&pair, &pool);
            TotalLiquidity::<T>::insert(&pair, raw_shares);

            // 4. Position, locked forever. `who` never has an existing position
            //    here (the pool had no shares), so this is an insert.
            let current_block = frame_system::Pallet::<T>::block_number();
            let forever: BlockNumberFor<T> = Bounded::max_value();
            LiquidityPositions::<T>::insert(
                who,
                &pair,
                LiquidityPosition {
                    shares: shares_to_mint,
                    entry_block: current_block,
                    locked_until: Some(forever),
                },
            );

            Self::deposit_event(Event::LiquidityAdded {
                provider: who.clone(),
                asset_a: pair.0.clone(),
                asset_b: pair.1.clone(),
                amount_a,
                amount_b,
                shares_minted: shares_to_mint,
            });
            Self::deposit_event(Event::LiquidityLocked {
                who: who.clone(),
                pool: pair,
                locked_until: forever,
            });
            Self::deposit_event(Event::ReservedPoolSeeded {
                who: who.clone(),
                asset,
                quote,
                amount_asset,
                amount_quote,
                shares: shares_to_mint,
            });
            Ok(shares_to_mint)
        }

        /// Add liquidity to an existing pool on behalf of `who`.
        ///
        /// Body of the `add_liquidity` extrinsic with the signer resolved by the
        /// caller. Tokens are pulled from `who`, and the LP position is credited
        /// to `who`. Returns the LP shares minted to `who` (excluding the
        /// permanently locked `MINIMUM_LIQUIDITY` on a first deposit).
        pub fn do_add_liquidity_for(
            who: &T::AccountId,
            asset_a: T::AssetKind,
            asset_b: T::AssetKind,
            amount_a: T::Balance,
            amount_b: T::Balance,
            amount_a_min: T::Balance,
            amount_b_min: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(
                amount_a > Zero::zero() && amount_b > Zero::zero(),
                Error::<T>::ZeroAmount
            );

            // Finding 5 + D5: canonicalize the pair AND the amounts/minimums,
            // which the caller gave in (asset_a, asset_b) order. From here on
            // `amount_a` belongs to `pair.0` and `amount_b` to `pair.1`.
            let (pair, (amount_a, amount_a_min), (amount_b, amount_b_min)) =
                Self::canonical_pair_with(asset_a, asset_b, (amount_a, amount_a_min), (amount_b, amount_b_min));
            let mut pool = Pools::<T>::get(&pair).ok_or(Error::<T>::PoolNotFound)?;

            // D6: price the deposit against what the pool actually holds. The
            // pool's share of every swap fee sits in the account uncounted
            // until the next sync (Finding 3); pricing against the stale
            // recorded reserves minted a depositor shares against a smaller
            // pool than the one `remove_liquidity` (which syncs) pays out of,
            // so swap → add → remove skimmed existing LPs' fees.
            Self::sync_reserves(&pair, &mut pool);

            let total_shares =
                TotalLiquidity::<T>::get(&pair).unwrap_or_else(T::Balance::zero);

            // Determine actual deposit amounts and shares to mint.
            let (actual_a, actual_b, total_new_shares, shares_to_mint) = if total_shares.is_zero()
            {
                // Finding 1: first deposit — burn MINIMUM_LIQUIDITY shares permanently.
                // D1: wide precision; integer sqrt rounds DOWN, in the pool's favour.
                let raw_shares = Self::sqrt_of_product(amount_a, amount_b)?;
                let min_liq: T::Balance = MINIMUM_LIQUIDITY.into();
                ensure!(raw_shares > min_liq, Error::<T>::InsufficientInitialLiquidity);
                let shares_to_mint = raw_shares
                    .checked_sub(&min_liq)
                    .ok_or(Error::<T>::Overflow)?;
                // total includes the locked minimum; user only receives the remainder.
                (amount_a, amount_b, raw_shares, shares_to_mint)
            } else {
                // Finding 8: calculate optimal amounts — don't donate excess tokens.
                // D1: wide precision; floor rounds the matched amount DOWN so the
                // depositor never over-contributes relative to the pool ratio.
                let optimal_b = Self::mul_div_floor(amount_a, pool.reserve_b, pool.reserve_a)?;

                let (actual_a, actual_b) = if optimal_b <= amount_b {
                    (amount_a, optimal_b)
                } else {
                    let optimal_a =
                        Self::mul_div_floor(amount_b, pool.reserve_a, pool.reserve_b)?;
                    (optimal_a, amount_b)
                };

                // D1: floor rounds minted shares DOWN, in the pool's favour.
                let share_a = Self::mul_div_floor(actual_a, total_shares, pool.reserve_a)?;
                let share_b = Self::mul_div_floor(actual_b, total_shares, pool.reserve_b)?;
                let shares = if share_a < share_b { share_a } else { share_b };

                (actual_a, actual_b, shares, shares)
            };

            ensure!(shares_to_mint > Zero::zero(), Error::<T>::ZeroAmount);

            // Finding 7: enforce slippage on the actual (possibly adjusted) amounts.
            ensure!(actual_a >= amount_a_min, Error::<T>::SlippageExceeded);
            ensure!(actual_b >= amount_b_min, Error::<T>::SlippageExceeded);

            T::Assets::transfer(
                pair.0.clone(),
                who,
                &pool.pool_account,
                actual_a,
                Expendable,
            )?;
            T::Assets::transfer(
                pair.1.clone(),
                who,
                &pool.pool_account,
                actual_b,
                Expendable,
            )?;

            pool.reserve_a =
                pool.reserve_a.checked_add(&actual_a).ok_or(Error::<T>::Overflow)?;
            pool.reserve_b =
                pool.reserve_b.checked_add(&actual_b).ok_or(Error::<T>::Overflow)?;
            Pools::<T>::insert(&pair, &pool);

            let new_total =
                total_shares.checked_add(&total_new_shares).ok_or(Error::<T>::Overflow)?;
            TotalLiquidity::<T>::insert(&pair, new_total);

            let current_block = frame_system::Pallet::<T>::block_number();
            LiquidityPositions::<T>::try_mutate(
                who,
                &pair,
                |maybe_pos| -> DispatchResult {
                    match maybe_pos {
                        Some(pos) => {
                            // Finding 9: keep original entry_block on top-up.
                            pos.shares = pos
                                .shares
                                .checked_add(&shares_to_mint)
                                .ok_or(Error::<T>::Overflow)?;
                        },
                        None => {
                            *maybe_pos = Some(LiquidityPosition {
                                shares: shares_to_mint,
                                entry_block: current_block,
                                locked_until: None,
                            });
                        },
                    }
                    Ok(())
                },
            )?;

            Self::deposit_event(Event::LiquidityAdded {
                provider: who.clone(),
                asset_a: pair.0,
                asset_b: pair.1,
                amount_a: actual_a,
                amount_b: actual_b,
                shares_minted: shares_to_mint,
            });
            Ok(shares_to_mint)
        }

        /// Extend the lock on `who`'s liquidity position in the given pool to
        /// `lock_until`.
        ///
        /// Body of the `lock_liquidity` extrinsic with the signer resolved by
        /// the caller. Fails with `InsufficientShares` if `who` has no position.
        /// D3: a lock is monotone — re-locking to the same block is a no-op,
        /// an earlier block fails with `LockCannotBeShortened`, so a position
        /// locked to `BlockNumber::max_value()` stays locked for good.
        pub fn do_lock_liquidity_for(
            who: &T::AccountId,
            asset_a: T::AssetKind,
            asset_b: T::AssetKind,
            lock_until: BlockNumberFor<T>,
        ) -> DispatchResult {
            let pair = Self::canonical_pair(asset_a, asset_b);

            LiquidityPositions::<T>::try_mutate(
                who,
                &pair,
                |maybe_pos| -> DispatchResult {
                    let pos = maybe_pos.as_mut().ok_or(Error::<T>::InsufficientShares)?;
                    if let Some(existing) = pos.locked_until {
                        ensure!(lock_until >= existing, Error::<T>::LockCannotBeShortened);
                    }
                    pos.locked_until = Some(lock_until);
                    Ok(())
                },
            )?;

            Self::deposit_event(Event::LiquidityLocked {
                who: who.clone(),
                pool: pair,
                locked_until: lock_until,
            });

            Ok(())
        }

        /// Execute a swap on behalf of `who`, depositing output to `recipient`.
        ///
        /// Returns the actual `amount_out` transferred to `recipient`. Callers use
        /// this for slippage-capture economics (e.g., solver marketplace).
        ///
        /// Behaviorally identical to the body of `swap_exact_tokens_for_tokens`:
        /// loads the pool, syncs reserves, computes constant-product output, applies
        /// fee, performs transfers, updates storage, emits `SwapExecuted` and
        /// `FeesCollected`.
        pub(crate) fn do_swap(
            who: &T::AccountId,
            asset_in: T::AssetKind,
            asset_out: T::AssetKind,
            amount_in: T::Balance,
            amount_out_min: T::Balance,
            recipient: &T::AccountId,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(amount_in > Zero::zero(), Error::<T>::ZeroAmount);

            // Finding 5: canonicalize pair for lookup.
            let pair = Self::canonical_pair(asset_in.clone(), asset_out.clone());
            let mut pool = Pools::<T>::get(&pair).ok_or(Error::<T>::PoolNotFound)?;

            // Finding 2: sync reserves from actual on-chain balances.
            Self::sync_reserves(&pair, &mut pool);

            let flipped = pair.0.encode() != asset_in.encode();
            let (reserve_in, reserve_out) = if flipped {
                (pool.reserve_b, pool.reserve_a)
            } else {
                (pool.reserve_a, pool.reserve_b)
            };

            ensure!(
                !reserve_in.is_zero() && !reserve_out.is_zero(),
                Error::<T>::InsufficientLiquidity
            );

            // ---- D4: fee split ----------------------------------------------
            //
            // The tier is the total fee (0.1% / 0.3% / 1.0%). The routed part
            // (`protocol_bps + creator_bps`, snapshotted per pool) is always
            // taken in the native asset; the pool keeps the rest:
            //
            //   native in : fee   = tier of input (as before)
            //               routed slices come out of that fee
            //               reserves get input − fee (unchanged output)
            //   native out: input fee = (tier − routed) of input, stays in pool
            //               output is priced on that; routed slices come
            //               out of the gross output, trader gets the net
            //   neither   : nothing is routed (fee stays in the pool)
            //
            // Every division floors, in the pool's favour; k never decreases.
            let native = T::NativeAsset::get();
            let native_in = asset_in.encode() == native.encode();
            let native_out = asset_out.encode() == native.encode();
            let routing = if native_in || native_out { pool.routing } else { FeeRouting::default() };
            let tier_bps: u16 = (pool.fee_tier as u16).saturating_mul(10);
            let pool_bps = tier_bps.saturating_sub(routing.routed_bps());

            let fee_tier_bal: T::Balance = pool.fee_tier.into();
            let denominator_bal: T::Balance = FEE_DENOMINATOR.into();

            // Fee taken from the input. With native in (or nothing routed)
            // this is the full tier, computed exactly as before D4.
            let fee = if native_out && routing.routed_bps() > 0 {
                Self::bps_of(amount_in, pool_bps)?
            } else {
                // D1: wide precision. Fee keeps its pre-existing floor (rounds
                // the fee DOWN by < 1 unit). Direction unchanged by this change.
                Self::mul_div_floor(amount_in, fee_tier_bal, denominator_bal)?
            };
            let amount_in_after_fee =
                amount_in.checked_sub(&fee).ok_or(Error::<T>::Overflow)?;

            // D1: constant-product output in wide precision; floor rounds
            // amount_out DOWN, in the pool's favour, so k never decreases.
            let denom = reserve_in
                .checked_add(&amount_in_after_fee)
                .ok_or(Error::<T>::Overflow)?;
            let amount_out_gross = Self::mul_div_floor(reserve_out, amount_in_after_fee, denom)?;

            // Routed slices, always native.
            let (protocol, creator) = if native_in {
                (Self::bps_of(amount_in, routing.protocol_bps)?, Self::bps_of(amount_in, routing.creator_bps)?)
            } else if native_out {
                (
                    Self::bps_of(amount_out_gross, routing.protocol_bps)?,
                    Self::bps_of(amount_out_gross, routing.creator_bps)?,
                )
            } else {
                (Zero::zero(), Zero::zero())
            };
            let routed = protocol.checked_add(&creator).ok_or(Error::<T>::Overflow)?;
            // With native in the routed part is a sub-slice of `fee` (bounded
            // by MAX_ROUTED_BPS ≤ tier); with native out it comes off the
            // gross output.
            let amount_out = if native_out {
                amount_out_gross.checked_sub(&routed).ok_or(Error::<T>::Overflow)?
            } else {
                amount_out_gross
            };

            ensure!(amount_out >= amount_out_min, Error::<T>::SlippageExceeded);
            ensure!(amount_out_gross < reserve_out, Error::<T>::InsufficientLiquidity);

            T::Assets::transfer(
                asset_in.clone(),
                who,
                &pool.pool_account,
                amount_in,
                Expendable,
            )?;
            T::Assets::transfer(
                asset_out.clone(),
                &pool.pool_account,
                recipient,
                amount_out,
                Expendable,
            )?;
            // D4: move the routed slices out of the pool account into the fee
            // escrow, so neither reserves nor `sync_reserves` ever see them.
            if !routed.is_zero() {
                T::Assets::transfer(
                    native.clone(),
                    &pool.pool_account,
                    &Self::fee_escrow_account(),
                    routed,
                    Expendable,
                )?;
                if !protocol.is_zero() {
                    ProtocolFeesUnclaimed::<T>::mutate(|t| *t = t.saturating_add(protocol));
                }
                if !creator.is_zero() {
                    CreatorFeesUnclaimed::<T>::mutate(&pair, |t| *t = t.saturating_add(creator));
                }
            }

            // Finding 3: only add amount_in_after_fee to reserves; the pool's
            // share of the fee stays in the pool account but is not counted in
            // reserves until the next sync. The output side drops by the gross
            // amount (net to the trader + routed slices).
            if flipped {
                pool.reserve_b = pool
                    .reserve_b
                    .checked_add(&amount_in_after_fee)
                    .ok_or(Error::<T>::Overflow)?;
                pool.reserve_a = pool
                    .reserve_a
                    .checked_sub(&amount_out_gross)
                    .ok_or(Error::<T>::InsufficientLiquidity)?;
            } else {
                pool.reserve_a = pool
                    .reserve_a
                    .checked_add(&amount_in_after_fee)
                    .ok_or(Error::<T>::Overflow)?;
                pool.reserve_b = pool
                    .reserve_b
                    .checked_sub(&amount_out_gross)
                    .ok_or(Error::<T>::InsufficientLiquidity)?;
            }
            pool.total_fees_collected = pool
                .total_fees_collected
                .checked_add(&fee)
                .ok_or(Error::<T>::Overflow)?;

            let pool_account_for_event = pool.pool_account.clone();
            Pools::<T>::insert(&pair, pool);

            Self::deposit_event(Event::SwapExecuted {
                who: who.clone(),
                asset_in,
                asset_out,
                amount_in,
                amount_out,
                fee,
            });

            // Finding 11: emit FeesCollected event.
            Self::deposit_event(Event::FeesCollected {
                pool: pair.clone(),
                amount: fee,
                recipient: pool_account_for_event,
            });
            if !routed.is_zero() {
                Self::deposit_event(Event::FeesRouted { pool: pair, protocol, creator });
            }

            Ok(amount_out)
        }

        // ====================================================================
        // Settlement helpers
        // ====================================================================

        /// Current bid window. Uses storage value if set, otherwise the
        /// genesis default from `T::DefaultBidWindowBlocks`.
        pub(crate) fn current_bid_window() -> BlockNumberFor<T> {
            BidWindowBlocks::<T>::get().unwrap_or_else(T::DefaultBidWindowBlocks::get)
        }

        /// Current settlement window.
        pub(crate) fn current_settlement_window() -> BlockNumberFor<T> {
            SettlementWindowBlocks::<T>::get()
                .unwrap_or_else(T::DefaultSettlementWindowBlocks::get)
        }

        /// Current solver bond amount.
        pub(crate) fn current_solver_bond() -> T::Balance {
            SolverBondAmount::<T>::get().unwrap_or_else(T::DefaultSolverBondAmount::get)
        }

        /// Derive the escrow account holding a specific solver's bond.
        ///
        /// Deterministic function of `solver_id`. Each solver gets a unique
        /// sub-account so slashing and refunds cannot mix funds. The
        /// `solver_id.to_le_bytes()` occupy the front of the seed so that
        /// they survive truncation on shorter `AccountId` types (e.g., u128
        /// in tests); the `"slvr"` tag sits after them and is visible on
        /// 32-byte accounts.
        pub(crate) fn solver_escrow_account(solver_id: u64) -> T::AccountId {
            let mut seed = [0u8; 12];
            seed[..8].copy_from_slice(&solver_id.to_le_bytes());
            seed[8..].copy_from_slice(b"slvr");
            PALLET_ID.into_sub_account_truncating(seed)
        }

        /// Derive the shared escrow account holding all pending intent
        /// `token_in` balances. Per-intent accounting lives in the `Intents`
        /// storage map.
        pub(crate) fn intent_escrow_account() -> T::AccountId {
            PALLET_ID.into_sub_account_truncating(b"intents")
        }

        /// Derive the protocol fee treasury account (where the protocol's
        /// share of solver profits accrues). Downstream distribution happens
        /// off this account via a separate sweep extrinsic in a later phase.
        pub(crate) fn protocol_treasury_account() -> T::AccountId {
            PALLET_ID.into_sub_account_truncating(b"fee_trsy")
        }
    }
}

impl<T: Config> PoolManager<T::AccountId, T::AssetKind, T::Balance, BlockNumberFor<T>>
    for Pallet<T>
{
    fn pool_exists(asset_a: T::AssetKind, asset_b: T::AssetKind) -> bool {
        Self::pool_exists(asset_a, asset_b)
    }

    fn create_pool(asset_a: T::AssetKind, asset_b: T::AssetKind, fee_tier: u32) -> DispatchResult {
        Self::do_create_pool(asset_a, asset_b, fee_tier)
    }

    fn add_liquidity_for(
        who: &T::AccountId,
        asset_a: T::AssetKind,
        asset_b: T::AssetKind,
        amount_a: T::Balance,
        amount_b: T::Balance,
        amount_a_min: T::Balance,
        amount_b_min: T::Balance,
    ) -> Result<T::Balance, DispatchError> {
        Self::do_add_liquidity_for(
            who,
            asset_a,
            asset_b,
            amount_a,
            amount_b,
            amount_a_min,
            amount_b_min,
        )
    }

    fn lock_liquidity_for(
        who: &T::AccountId,
        asset_a: T::AssetKind,
        asset_b: T::AssetKind,
        lock_until: BlockNumberFor<T>,
    ) -> DispatchResult {
        Self::do_lock_liquidity_for(who, asset_a, asset_b, lock_until)
    }
}

impl<T: Config> ReservedPoolSeeder<T::AccountId, T::AssetKind, T::Balance, BlockNumberFor<T>>
    for Pallet<T>
{
    fn seed_reserved_pool_for(
        who: &T::AccountId,
        asset: T::AssetKind,
        quote: T::AssetKind,
        amount_asset: T::Balance,
        amount_quote: T::Balance,
        fee_tier: u32,
    ) -> Result<T::Balance, DispatchError> {
        Self::do_seed_reserved_pool_for(who, asset, quote, amount_asset, amount_quote, fee_tier)
    }
}
