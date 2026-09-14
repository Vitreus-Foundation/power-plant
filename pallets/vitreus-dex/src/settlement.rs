//! Solver Marketplace Settlement — types and pure helpers.
//!
//! This module defines the data model and math helpers used by the solver
//! marketplace extrinsics. Extrinsics, storage items, events, and errors are
//! integrated into the main pallet's `lib.rs` in a separate step, because
//! they need access to `T::Assets`, `T::AssetKind`, `T::Balance`, and the
//! pallet's internal swap helper.
//!
//! Economic parameters exposed here are fixed at compile time. Operational
//! parameters (bid window, settlement window, bond amount) are exposed as
//! runtime storage items in the main pallet and set via a governance
//! extrinsic gated on `T::ManageOrigin`.

use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

// ============================================================================
// Compile-time economic constants
// ============================================================================

/// Reputation awarded per successful fill.
pub const REPUTATION_FILL_REWARD: i64 = 1;

/// Reputation deducted per slash event.
pub const REPUTATION_SLASH_PENALTY: i64 = -10;

/// Share of slashed bond paid to the account that triggers the slash.
/// Expressed in basis points of 10_000 (e.g., 1000 = 10%).
pub const SLASHER_REWARD_BPS: u32 = 1_000;

/// Protocol fee on solver profit. Basis points of the profit amount
/// (e.g., 2000 = 20% of profit to protocol treasury).
pub const SOLVER_PROFIT_FEE_BPS: u32 = 2_000;

// ============================================================================
// Intent status
// ============================================================================

/// Lifecycle status of an intent.
#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
pub enum IntentStatus {
    /// Awaiting solver commitment.
    Open,
    /// A solver has committed to fill this intent.
    Committed,
    /// Successfully settled.
    Settled,
    /// User cancelled before any commitment.
    Cancelled,
    /// Deadline passed without settlement; user can reclaim funds.
    Expired,
}

// ============================================================================
// Intent
// ============================================================================

/// A user-submitted trading intent. Generic over account, asset, balance, and
/// block-number types so it can be instantiated against the concrete pallet
/// types (`T::AccountId`, `T::AssetKind`, `T::Balance`, `BlockNumberFor<T>`).
#[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
pub struct Intent<AccountId, AssetKind, Balance, BlockNumber> {
    /// Pallet-scoped monotonically increasing id.
    pub id: u64,
    /// User who submitted the intent; receives the output on settlement.
    pub user: AccountId,
    /// Asset the user is selling.
    pub token_in: AssetKind,
    /// Asset the user wants to receive.
    pub token_out: AssetKind,
    /// Exact amount of `token_in` the user is providing.
    pub amount_in: Balance,
    /// Minimum acceptable output. Commitment must match or exceed this.
    pub min_amount_out: Balance,
    /// Block at or after which the intent is considered expired if unsettled.
    pub deadline: BlockNumber,
    /// Block at which the intent was submitted.
    pub submitted_at: BlockNumber,
    /// Current lifecycle status.
    pub status: IntentStatus,
}

// ============================================================================
// Solver
// ============================================================================

/// Information about a registered solver.
#[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
pub struct SolverInfo<AccountId, Balance, BlockNumber> {
    /// Pallet-scoped monotonically increasing id.
    pub id: u64,
    /// Solver's on-chain account.
    pub account: AccountId,
    /// VTRS bond held in escrow.
    pub bond: Balance,
    /// Net reputation. Increments by `REPUTATION_FILL_REWARD` per successful
    /// fill, decrements by `REPUTATION_SLASH_PENALTY` per slash. May be
    /// negative.
    pub reputation: i64,
    /// Count of intents this solver has successfully settled.
    pub fills_completed: u64,
    /// Count of times this solver has been slashed.
    pub fills_slashed: u64,
    /// Count of currently-committed (not yet settled/slashed) fills. Must be
    /// zero to deregister. Incremented in `commit_fill`, decremented in
    /// `settle_intent` / slash / expire paths.
    pub active_commitments: u32,
    /// Block at which the solver registered.
    pub registered_at: BlockNumber,
    /// False after voluntary deregistration (bond refunded) or a full slash.
    pub active: bool,
}

// ============================================================================
// Fill commitment
// ============================================================================

/// A solver's commitment to fill a specific intent. Exists only during the
/// `Committed` status window.
#[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
pub struct FillCommitment<AccountId, Balance, BlockNumber> {
    /// Id of the intent being filled.
    pub intent_id: u64,
    /// Id of the solver making the commitment.
    pub solver_id: u64,
    /// Solver's on-chain account (denormalized from `SolverInfo` for convenience).
    pub solver_account: AccountId,
    /// Amount the solver promises to deliver to the user.
    /// Must be `>= intent.min_amount_out`.
    pub committed_amount_out: Balance,
    /// Block at which the commitment was registered.
    pub committed_at: BlockNumber,
    /// Solver must complete settlement by this block or be slashable.
    pub settle_by: BlockNumber,
}

// ============================================================================
// Pure helpers
// ============================================================================

/// `floor(amount * bps / 10_000)` in wide precision (D1).
///
/// The product is computed in `U256` so an `amount` above ~3.4·10^34 no
/// longer saturates and silently underpays. The result is at most `amount`
/// and therefore always fits `u128`; the fallback branch is unreachable and
/// only exists so this helper can never panic.
fn bps_of_u128(amount: u128, bps: u32) -> u128 {
    let x = sp_core::U256::from(amount) * sp_core::U256::from(bps) / sp_core::U256::from(10_000u32);
    u128::try_from(x).unwrap_or(amount)
}

/// Split a slashed bond between slasher and protocol treasury.
///
/// `slasher_reward_bps` is in basis points of 10_000. Returns
/// `(to_treasury, to_slasher)`.
///
/// Non-panicking. Caller converts to/from the concrete balance type.
pub fn split_slashed_bond_u128(bond: u128, slasher_reward_bps: u32) -> (u128, u128) {
    let slasher_amount = bps_of_u128(bond, slasher_reward_bps);
    let treasury_amount = bond.saturating_sub(slasher_amount);
    (treasury_amount, slasher_amount)
}

/// Split solver profit between solver and protocol treasury.
///
/// `fee_bps` is in basis points of 10_000. Returns `(solver_net, protocol_fee)`.
pub fn split_solver_profit_u128(profit: u128, fee_bps: u32) -> (u128, u128) {
    let protocol_fee = bps_of_u128(profit, fee_bps);
    let solver_net = profit.saturating_sub(protocol_fee);
    (solver_net, protocol_fee)
}

// ============================================================================
// Unit tests for pure helpers
// ============================================================================

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn split_slashed_bond_standard_case() {
        // 10% slasher reward on 1000 bond: 100 to slasher, 900 to treasury.
        let (treasury, slasher) = split_slashed_bond_u128(1_000, 1_000);
        assert_eq!(treasury, 900);
        assert_eq!(slasher, 100);
    }

    #[test]
    fn split_slashed_bond_zero_bond() {
        let (treasury, slasher) = split_slashed_bond_u128(0, 1_000);
        assert_eq!(treasury, 0);
        assert_eq!(slasher, 0);
    }

    #[test]
    fn split_slashed_bond_zero_reward() {
        // 0% slasher reward: everything to treasury.
        let (treasury, slasher) = split_slashed_bond_u128(1_000, 0);
        assert_eq!(treasury, 1_000);
        assert_eq!(slasher, 0);
    }

    #[test]
    fn split_slashed_bond_full_reward() {
        // 100% slasher reward (unusual): everything to slasher.
        let (treasury, slasher) = split_slashed_bond_u128(1_000, 10_000);
        assert_eq!(treasury, 0);
        assert_eq!(slasher, 1_000);
    }

    #[test]
    fn split_solver_profit_standard_case() {
        // 20% fee on 100 profit: 20 fee, 80 to solver.
        let (solver, fee) = split_solver_profit_u128(100, 2_000);
        assert_eq!(solver, 80);
        assert_eq!(fee, 20);
    }

    #[test]
    fn split_solver_profit_zero_profit() {
        let (solver, fee) = split_solver_profit_u128(0, 2_000);
        assert_eq!(solver, 0);
        assert_eq!(fee, 0);
    }

    #[test]
    fn split_solver_profit_rounding_behavior() {
        // 1 unit profit at 20%: integer division gives 0 fee. Solver keeps all 1.
        // Acceptable: dust on small fills goes to the solver, not the protocol.
        let (solver, fee) = split_solver_profit_u128(1, 2_000);
        assert_eq!(solver, 1);
        assert_eq!(fee, 0);
    }

    #[test]
    fn split_slashed_bond_large_values_no_overflow() {
        // 1e30 bond at 10%: saturating arithmetic holds.
        let huge = 1_000_000_000_000_000_000_000_000_000_000u128;
        let (treasury, slasher) = split_slashed_bond_u128(huge, 1_000);
        assert_eq!(treasury + slasher, huge);
        assert_eq!(slasher, huge / 10);
    }
}
