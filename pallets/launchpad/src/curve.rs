//! Constant-product bonding-curve math (LAUNCHPAD_SPEC.md §3).
//!
//! Pure functions over `u128` state with `U256` intermediates. Every division
//! rounds in the pool's favour:
//!
//! * fees round UP (the trader pays more);
//! * tokens out of the curve round DOWN (`Tk_new` rounds up);
//! * quote out of the curve rounds DOWN (`Q_new` rounds up);
//! * on a crossing buy the quote needed for the last tokens rounds UP.
//!
//! Consequently the virtual invariant `k = (V_q + real_quote) × (VT_FLOOR +
//! tokens_remaining)` never decreases across a trade (invariant I4).
//!
//! Virtual reserves:
//! ```text
//!   Q  = V_q + real_quote                 (quote side, includes the phantom V_q)
//!   Tk = VT_FLOOR + tokens_remaining      (token side, includes the virtual floor)
//!   k  = Q × Tk                           (U256 only)
//! ```

use sp_core::U256;

/// Basis-point denominator.
pub const BPS: u128 = 10_000;

/// Largest quote input accepted by a single trade. Keeps `q_in × fee_bps`
/// inside `u128` with a wide margin (10^34 × 10^4 = 10^38 < 3.4·10^38) and is
/// never binding in practice (10^16 VTRS).
pub const MAX_TRADE_IN: u128 = 10_000_000_000_000_000_000_000_000_000_000_000;

/// Snapshot of a launch's pricing terms (the parts of `CurveParams` the math needs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    /// `V_q`, the virtual quote reserve.
    pub virtual_quote: u128,
    /// `VT_FLOOR`, virtual token reserve left when the curve sells out.
    pub token_floor: u128,
    /// Trading fee on the quote leg, in basis points.
    pub fee_bps: u128,
}

/// Mutable curve state the math reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub real_quote: u128,
    pub tokens_remaining: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuyQuote {
    /// Tokens delivered to the buyer.
    pub tokens_out: u128,
    /// Quote taken from the buyer, gross (net + fee). `<= q_in`.
    pub quote_used: u128,
    /// Quote absorbed by the curve (`real_quote += quote_net_used`).
    pub quote_net_used: u128,
    /// Fee, on top of `quote_net_used`.
    pub fee: u128,
    /// True when this buy exhausted the sellable allocation (graduation trigger).
    pub crossed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SellQuote {
    /// Quote leaving the curve (`real_quote -= quote_gross`).
    pub quote_gross: u128,
    /// Fee taken out of `quote_gross`.
    pub fee: u128,
    /// Quote paid to the seller (`quote_gross - fee`).
    pub quote_out: u128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathError {
    ZeroAmount,
    /// A `u128` bound was exceeded (input above `MAX_TRADE_IN`, or a result that
    /// should always fit did not — the latter indicates a broken invariant).
    Overflow,
    /// The trade would deliver nothing to the trader.
    Unquotable,
    /// Curve state does not satisfy the invariants the math relies on.
    BadState,
}

#[inline]
fn ceil_div(n: U256, d: U256) -> U256 {
    // d > 0 guaranteed by every caller.
    (n + d - U256::one()) / d
}

#[inline]
fn to_u128(x: U256) -> Result<u128, MathError> {
    u128::try_from(x).map_err(|_| MathError::Overflow)
}

/// `ceil(amount × fee_bps / BPS)` — fee rounds UP.
#[inline]
fn fee_on(amount: u128, fee_bps: u128) -> Result<u128, MathError> {
    if fee_bps == 0 {
        return Ok(0);
    }
    let n = amount.checked_mul(fee_bps).ok_or(MathError::Overflow)?;
    Ok(n.div_ceil(BPS))
}

/// Virtual reserves and their product. Returns `BadState` if the state's
/// sums do not fit (they always do for in-bounds parameters).
#[inline]
fn reserves(t: &Terms, s: &State) -> Result<(u128, u128, U256), MathError> {
    let q = t.virtual_quote.checked_add(s.real_quote).ok_or(MathError::BadState)?;
    let tk = t.token_floor.checked_add(s.tokens_remaining).ok_or(MathError::BadState)?;
    if q == 0 || tk == 0 {
        return Err(MathError::BadState);
    }
    Ok((q, tk, U256::from(q) * U256::from(tk)))
}

/// Quote for an exact-quote-in buy (§3.3). Does not mutate state.
pub fn quote_buy(t: &Terms, s: &State, q_in: u128) -> Result<BuyQuote, MathError> {
    if q_in == 0 {
        return Err(MathError::ZeroAmount);
    }
    if q_in > MAX_TRADE_IN {
        return Err(MathError::Overflow);
    }
    if s.tokens_remaining == 0 {
        // Curve is complete; the pallet never calls this in that phase.
        return Err(MathError::BadState);
    }
    if t.fee_bps >= BPS {
        return Err(MathError::BadState);
    }

    // 1–2. fee rounds UP; net is what reaches the curve.
    let fee = fee_on(q_in, t.fee_bps)?;
    let q_net = q_in.checked_sub(fee).ok_or(MathError::Overflow)?;
    if q_net == 0 {
        return Err(MathError::Unquotable);
    }

    // 3. virtual reserves
    let (q, tk, k) = reserves(t, s)?;

    // 4. Tk_new rounds UP so fewer tokens leave; guard against rounding above Tk.
    let denom = U256::from(q.checked_add(q_net).ok_or(MathError::Overflow)?);
    let tk_new = to_u128(ceil_div(k, denom))?.min(tk);

    // 5. tokens out
    let t_out = tk - tk_new;
    if t_out == 0 {
        return Err(MathError::Unquotable);
    }

    // 6. normal fill vs. crossing (partial) fill
    if t_out < s.tokens_remaining {
        return Ok(BuyQuote { tokens_out: t_out, quote_used: q_in, quote_net_used: q_net, fee, crossed: false });
    }

    // Crossing: deliver exactly what is left; charge exactly what the curve
    // needs to reach VT_FLOOR (rounded UP), plus a fee on that amount that is
    // the inverse of the normal fee rounding (also UP).
    let q_end = to_u128(ceil_div(k, U256::from(t.token_floor)))?;
    let quote_net_used = q_end.checked_sub(q).ok_or(MathError::BadState)?;
    if quote_net_used == 0 {
        return Err(MathError::Unquotable);
    }
    // fee' = ceil(net × f / (BPS − f))
    let fee_c = if t.fee_bps == 0 {
        0
    } else {
        let n = quote_net_used.checked_mul(t.fee_bps).ok_or(MathError::Overflow)?;
        let d = BPS - t.fee_bps;
        n.div_ceil(d)
    };
    let quote_used = quote_net_used.checked_add(fee_c).ok_or(MathError::Overflow)?;
    // By construction (see spec §3.3) quote_used ≤ q_in; a violation means the
    // branch condition and the rounding disagree, which must never happen.
    if quote_used > q_in {
        return Err(MathError::Overflow);
    }
    Ok(BuyQuote {
        tokens_out: s.tokens_remaining,
        quote_used,
        quote_net_used,
        fee: fee_c,
        crossed: true,
    })
}

/// Quote for an exact-tokens-in sell (§3.4). Does not mutate state.
/// `t_in` must not exceed the tokens the curve has sold (`sellable − tokens_remaining`);
/// the pallet checks that before calling.
pub fn quote_sell(t: &Terms, s: &State, t_in: u128) -> Result<SellQuote, MathError> {
    if t_in == 0 {
        return Err(MathError::ZeroAmount);
    }
    if t.fee_bps >= BPS {
        return Err(MathError::BadState);
    }
    let (q, tk, k) = reserves(t, s)?;

    // Q_new rounds UP so less quote leaves; never below the phantom reserve.
    let denom = U256::from(tk.checked_add(t_in).ok_or(MathError::Overflow)?);
    let q_new = to_u128(ceil_div(k, denom))?.max(t.virtual_quote);

    let quote_gross = q.checked_sub(q_new).ok_or(MathError::BadState)?;
    if quote_gross == 0 {
        return Err(MathError::Unquotable);
    }
    // The curve can only pay out what it really holds (I5 guarantees this;
    // a violation is a broken invariant, not a user error).
    if quote_gross > s.real_quote {
        return Err(MathError::BadState);
    }
    let fee = fee_on(quote_gross, t.fee_bps)?;
    let quote_out = quote_gross.checked_sub(fee).ok_or(MathError::Overflow)?;
    if quote_out == 0 {
        return Err(MathError::Unquotable);
    }
    Ok(SellQuote { quote_gross, fee, quote_out })
}

/// `k` for a state — exposed for invariant checks in tests and `try_state`.
pub fn invariant_k(t: &Terms, s: &State) -> Option<U256> {
    reserves(t, s).ok().map(|(_, _, k)| k)
}

/// Quote raised when the curve sells out: `ceil(V_q·V_t / VT_FLOOR) − V_q` (§3.5).
pub fn raise_at_sellout(t: &Terms, sellable: u128) -> Option<u128> {
    let vt = t.token_floor.checked_add(sellable)?;
    let k = U256::from(t.virtual_quote) * U256::from(vt);
    let q_end = u128::try_from(ceil_div(k, U256::from(t.token_floor))).ok()?;
    q_end.checked_sub(t.virtual_quote)
}

#[cfg(test)]
mod unit {
    use super::*;

    const UNIT: u128 = 1_000_000_000_000_000_000;
    fn terms() -> Terms {
        // T = 3000 VTRS → V_q = 1000 VTRS; VT_FLOOR = 266_666_667 tokens; fee 1%.
        Terms { virtual_quote: 1_000 * UNIT, token_floor: 266_666_667 * UNIT, fee_bps: 100 }
    }
    const SELLABLE: u128 = 800_000_000 * UNIT;

    #[test]
    fn buy_then_sell_back_never_pays_more_than_taken() {
        let t = terms();
        let mut s = State { real_quote: 0, tokens_remaining: SELLABLE };
        let b = quote_buy(&t, &s, 10 * UNIT).unwrap();
        s.real_quote += b.quote_net_used;
        s.tokens_remaining -= b.tokens_out;
        let sl = quote_sell(&t, &s, b.tokens_out).unwrap();
        assert!(sl.quote_gross <= b.quote_net_used);
        assert!(sl.quote_out < 10 * UNIT);
    }

    #[test]
    fn crossing_charges_no_more_than_offered_and_raise_matches_target() {
        let t = terms();
        let s = State { real_quote: 0, tokens_remaining: SELLABLE };
        let b = quote_buy(&t, &s, 1_000_000 * UNIT).unwrap();
        assert!(b.crossed);
        assert_eq!(b.tokens_out, SELLABLE);
        assert!(b.quote_used <= 1_000_000 * UNIT);
        let r = raise_at_sellout(&t, SELLABLE).unwrap();
        assert_eq!(b.quote_net_used, r);
        // R = V_q·(V_t/VT_FLOOR − 1) and V_t/VT_FLOOR = 4 − 1/266_666_667, so R
        // undershoots 3·V_q by 1.25e-9 relative (the integer rounding of VT_FLOOR).
        let target = 3_000 * UNIT;
        let short = target - r;
        assert!(short * 1_000_000_000 <= target * 2, "raise {} vs target {} (short {})", r, target, short);
        assert!(short * 1_000_000_000 >= target, "raise {} vs target {} (short {})", r, target, short);
    }

    #[test]
    fn k_never_decreases_over_a_random_walk() {
        let t = terms();
        let mut s = State { real_quote: 0, tokens_remaining: SELLABLE };
        let mut x: u64 = 0x9E3779B97F4A7C15;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for _ in 0..5_000 {
            let k0 = invariant_k(&t, &s).unwrap();
            let r = next();
            if r % 3 != 0 || s.tokens_remaining == SELLABLE {
                let amt = match r % 5 {
                    0 => 1,
                    1 => (r % 1_000) as u128 + 1,
                    2 => (r as u128 % 50) * UNIT + 1,
                    _ => (r as u128 % 5_000) * UNIT,
                };
                match quote_buy(&t, &s, amt) {
                    Ok(b) => {
                        s.real_quote += b.quote_net_used;
                        s.tokens_remaining -= b.tokens_out;
                        if b.crossed {
                            assert_eq!(s.tokens_remaining, 0);
                            break;
                        }
                    },
                    Err(MathError::Unquotable) | Err(MathError::ZeroAmount) => {},
                    Err(e) => panic!("buy {amt}: {e:?}"),
                }
            } else {
                let sold = SELLABLE - s.tokens_remaining;
                if sold == 0 {
                    continue;
                }
                let amt = match r % 4 {
                    0 => 1,
                    1 => sold,
                    _ => (r as u128 % sold).max(1),
                };
                match quote_sell(&t, &s, amt) {
                    Ok(sl) => {
                        s.real_quote -= sl.quote_gross;
                        s.tokens_remaining += amt;
                    },
                    Err(MathError::Unquotable) => {},
                    Err(e) => panic!("sell {amt}: {e:?}"),
                }
            }
            let k1 = invariant_k(&t, &s).unwrap();
            assert!(k1 >= k0, "k decreased");
        }
    }
}
