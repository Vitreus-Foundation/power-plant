//! A slashing implementation for NPoS systems.
//!
//! For the purposes of the economic model, it is easiest to think of each validator as a cooperator
//! which cooperates only its own identity.
//!
//! The act of cooperation signals intent to unify economic identity with the validator - to take
//! part in the rewards of a job well done, and to take part in the punishment of a job done badly.
//!
//! There are 3 main difficulties to account for with slashing in PoR:
//!   - A cooperator can cooperate multiple validators and be slashed via any of them.
//!   - Until slashed, stake is reused from era to era. Cooperating with N coins for E eras in a row
//!     does not mean you have N*E coins to be slashed - you've only ever had N.
//!   - Slashable offences can be found after the fact and out of order.
//!
//! The algorithm implemented in this module tries to balance these 3 difficulties.
//!
//! First, we only slash participants for the _maximum_ slash they receive in some time period,
//! rather than the sum. This ensures a protection from overslashing.
//!
//! Second, we do not want the time period (or "span") that the maximum is computed
//! over to last indefinitely. That would allow participants to begin acting with
//! impunity after some point, fearing no further repercussions. For that reason, we
//! automatically "chill" validators and withdraw a cooperator's cooperation after a slashing event,
//! requiring them to re-enlist voluntarily (acknowledging the slash) and begin a new
//! slashing span.
//!
//! Typically, you will have a single slashing event per slashing span. Only in the case
//! where a validator releases many misbehaviors at once, or goes "back in time" to misbehave in
//! eras that have already passed, would you encounter situations where a slashing span
//! has multiple misbehaviors. However, accounting for such cases is necessary
//! to deter a class of "rage-quit" attacks.
//!
//! Based on research at <https://research.web3.foundation/en/latest/polkadot/slashing/npos.html>

use core::ops::{Add, Mul};

use crate::{
    Config, CooperatorSlashInEra, DisabledValidators, DisablingStrategy, Error, Exposure, Pallet,
    Perbill, SessionInterface, SpanSlash, StakeOf, UnappliedSlash, ValidatorSlashInEra,
};
use frame_support::{
    ensure,
    traits::{tokens::Imbalance, Currency, Defensive, OnUnbalanced},
};
use pallet_reputation::{Reputation, ReputationPoint, ReputationRecord, RANKS_PER_TIER};
use parity_scale_codec::{Decode, Encode, FullCodec, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, One, Saturating, Zero},
    DispatchError, DispatchResult, RuntimeDebug,
};
use sp_staking::EraIndex;
use sp_std::vec::Vec;

/// The proportion of the slashing reward to be paid out on the first slashing detection.
/// This is f_1 in the paper.
const REWARD_F1: Perbill = Perbill::from_percent(50);

/// The index of a slashing span - unique to each stash.
pub type SpanIndex = u32;

pub type SlashEntityOf<T> = SlashEntity<StakeOf<T>>;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub type NegativeImbalanceOf<T> =
    <<T as Config>::StakeCurrency as Currency<AccountIdOf<T>>>::NegativeImbalance;

// A range of start..end eras for a slashing span.
#[derive(Encode, Decode, TypeInfo)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub(crate) struct SlashingSpan {
    pub(crate) index: SpanIndex,
    pub(crate) start: EraIndex,
    pub(crate) length: Option<EraIndex>, // the ongoing slashing span has indeterminate length.
}

impl SlashingSpan {
    fn contains_era(&self, era: EraIndex) -> bool {
        self.start <= era && self.length.map_or(true, |l| self.start + l > era)
    }
}

/// An encoding of all of a staker's slashing spans.
#[derive(Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct SlashingSpans {
    // the index of the current slashing span of the cooperator. different for
    // every stash, resets when the account hits free balance 0.
    span_index: SpanIndex,
    // the start era of the most recent (ongoing) slashing span.
    last_start: EraIndex,
    // the last era at which a non-zero slash occurred.
    last_nonzero_slash: EraIndex,
    // all prior slashing spans' start indices, in reverse order (most recent first)
    // encoded as offsets relative to the slashing span after it.
    prior: Vec<EraIndex>,
}

impl SlashingSpans {
    // creates a new record of slashing spans for a stash, starting at the beginning of the bonding
    // period, relative to now.
    pub(crate) fn new(window_start: EraIndex) -> Self {
        SlashingSpans {
            span_index: 0,
            last_start: window_start,
            // initialize to zero, as this structure is lazily created until
            // the first slash is applied. setting equal to `window_start` would
            // put a time limit on cooperations.
            last_nonzero_slash: 0,
            prior: Vec::new(),
        }
    }

    // update the slashing spans to reflect the start of a new span at the era after `now` returns
    // `true` if a new span was started, `false` otherwise. `false` indicates that internal state is
    // unchanged.
    pub(crate) fn end_span(&mut self, now: EraIndex) -> bool {
        let next_start = now + 1;
        if next_start <= self.last_start {
            return false;
        }

        let last_length = next_start - self.last_start;
        self.prior.insert(0, last_length);
        self.last_start = next_start;
        self.span_index += 1;
        true
    }

    // an iterator over all slashing spans in _reverse_ order - most recent first.
    pub(crate) fn iter(&'_ self) -> impl Iterator<Item = SlashingSpan> + '_ {
        let mut last_start = self.last_start;
        let mut index = self.span_index;
        let last = SlashingSpan { index, start: last_start, length: None };
        let prior = self.prior.iter().cloned().map(move |length| {
            let start = last_start - length;
            last_start = start;
            index -= 1;

            SlashingSpan { index, start, length: Some(length) }
        });

        sp_std::iter::once(last).chain(prior)
    }

    /// Yields the era index where the most recent non-zero slash occurred.
    pub fn last_nonzero_slash(&self) -> EraIndex {
        self.last_nonzero_slash
    }

    // prune the slashing spans against a window, whose start era index is given.
    //
    // If this returns `Some`, then it includes a range start..end of all the span indices which
    // were pruned.
    fn prune(&mut self, window_start: EraIndex) -> Option<(SpanIndex, SpanIndex)> {
        let old_idx = self
            .iter()
            .skip(1) // skip ongoing span.
            .position(|span| span.length.map_or(false, |len| span.start + len <= window_start));

        let earliest_span_index = self.span_index - self.prior.len() as SpanIndex;
        let pruned = match old_idx {
            Some(o) => {
                self.prior.truncate(o);
                let new_earliest = self.span_index - self.prior.len() as SpanIndex;
                Some((earliest_span_index, new_earliest))
            },
            None => None,
        };

        // readjust the ongoing span, if it started before the beginning of the window.
        self.last_start = sp_std::cmp::max(self.last_start, window_start);
        pruned
    }
}

/// A slashing-span record for a particular stash.
#[derive(Encode, Decode, Default, TypeInfo, MaxEncodedLen)]
pub struct SpanRecord<SlashEntity: Encode + Decode + Default + TypeInfo + MaxEncodedLen> {
    slashed: SlashEntity,
    paid_out: SlashEntity,
}

impl<SlashEntity: Encode + Decode + Default + TypeInfo + MaxEncodedLen> SpanRecord<SlashEntity> {
    /// The value of stash balance slashed in this span.
    #[allow(dead_code)]
    #[cfg(test)]
    pub(crate) fn amount(&self) -> &SlashEntity {
        &self.slashed
    }
}

impl<SlashEntity: Encode + Decode + Default + TypeInfo + MaxEncodedLen> SpanRecord<SlashEntity> {
    pub fn new(slashed: SlashEntity, paid_out: SlashEntity) -> Self {
        Self { slashed, paid_out }
    }
}

/// Parameters for performing a slash.
#[derive(Clone)]
pub(crate) struct SlashParams<'a, T: 'a + Config> {
    /// The stash account being slashed.
    pub(crate) stash: &'a T::AccountId,
    /// The proportion of the slash.
    pub(crate) slash: Perbill,
    /// The exposure of the stash and all cooperators.
    pub(crate) exposure: &'a Exposure<T::AccountId, StakeOf<T>>,
    /// The era where the offence occurred.
    pub(crate) slash_era: EraIndex,
    /// The first era in the current bonding period.
    pub(crate) window_start: EraIndex,
    /// The current era.
    pub(crate) now: EraIndex,
    /// The maximum percentage of a slash that ever gets paid out.
    /// This is f_inf in the paper.
    pub(crate) reward_proportion: Perbill,
}

enum UpdateDecision {
    Update,
    NoUpdate,
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Default, Debug, TypeInfo, MaxEncodedLen)]
pub struct SlashEntityPerbill {
    reputation: Perbill,
    stake: Perbill,
}

impl SlashEntityPerbill {
    pub fn new(reputation: Perbill, stake: Perbill) -> Self {
        Self { reputation, stake }
    }
}

impl<Stake> Mul<SlashEntity<Stake>> for SlashEntityPerbill
where
    Stake: AtLeast32BitUnsigned + StorageEssentials + Copy,
{
    type Output = SlashEntity<Stake>;

    fn mul(self, rhs: SlashEntity<Stake>) -> Self::Output {
        SlashEntity::new((self.reputation * *rhs.reputation).into(), self.stake * rhs.stake)
    }
}

impl Add<Self> for SlashEntityPerbill {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self { reputation: self.reputation + rhs.reputation, stake: self.stake + rhs.stake }
    }
}

impl Zero for SlashEntityPerbill {
    fn zero() -> Self {
        Self { reputation: Zero::zero(), stake: Zero::zero() }
    }

    fn is_zero(&self) -> bool {
        self.reputation.is_zero() && self.stake.is_zero()
    }
}

impl SlashEntityPerbill {
    fn from_rational<T: Config>(p: SlashEntityOf<T>, q: SlashEntityOf<T>) -> Self {
        Self {
            reputation: Perbill::from_rational(*p.reputation, *q.reputation),
            stake: Perbill::from_rational(p.stake, q.stake),
        }
    }

    fn compare_with(&self, prior: Self) -> UpdateDecision {
        if prior.reputation.deconstruct() < self.reputation.deconstruct()
            || prior.stake.deconstruct() < self.stake.deconstruct()
        {
            UpdateDecision::Update
        } else {
            UpdateDecision::NoUpdate
        }
    }
}

pub trait StorageEssentials: FullCodec + Clone + TypeInfo + Default + MaxEncodedLen {}

impl<T: FullCodec + Clone + TypeInfo + Default + MaxEncodedLen> StorageEssentials for T {}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, Default, Debug, TypeInfo, MaxEncodedLen)]
pub struct SlashEntity<Stake: StorageEssentials + Copy> {
    pub reputation: ReputationPoint,
    pub stake: Stake,
}

impl<Stake: StorageEssentials + Copy> SlashEntity<Stake> {
    pub fn new(reputation: ReputationPoint, stake: Stake) -> Self {
        Self { reputation, stake }
    }

    // get the maximum possible amount of slash for an account
    pub fn max_slash_amount(reputation: &Reputation, stake: Stake) -> Self {
        let rank = reputation.tier().map(|t| t.rank()).unwrap_or(0);
        // RANKS_PER_TIER + 1 because we want take 0-rank into account
        Self::new(
            reputation
                .points()
                .saturating_sub(*ReputationPoint::from_rank(
                    rank.saturating_sub(RANKS_PER_TIER + 1),
                ))
                .into(),
            stake,
        )
    }
}

impl<Stake> SlashEntity<Stake>
where
    Stake: StorageEssentials + AtLeast32BitUnsigned + Copy,
{
    pub fn is_any_zero(&self) -> bool {
        self.reputation.is_zero() || self.stake.is_zero()
    }
}

impl<Stake> Mul<Perbill> for SlashEntity<Stake>
where
    Stake: AtLeast32BitUnsigned + StorageEssentials + Copy,
{
    type Output = Self;
    fn mul(self, rhs: Perbill) -> Self {
        Self::new((rhs * *self.reputation).into(), rhs * self.stake)
    }
}

impl<Stake> Mul<SlashEntityPerbill> for SlashEntity<Stake>
where
    Stake: AtLeast32BitUnsigned + StorageEssentials + Copy,
{
    type Output = Self;
    fn mul(self, rhs: SlashEntityPerbill) -> Self {
        Self::new((rhs.reputation * *self.reputation).into(), rhs.stake * self.stake)
    }
}

impl<Stake> Mul<Self> for SlashEntity<Stake>
where
    Stake: AtLeast32BitUnsigned + StorageEssentials + Copy,
{
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new((*self.reputation * *rhs.reputation).into(), self.stake * rhs.stake)
    }
}

impl<Stake> Add<Self> for SlashEntity<Stake>
where
    Stake: AtLeast32BitUnsigned + StorageEssentials + Copy,
{
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new((*self.reputation + *rhs.reputation).into(), self.stake + rhs.stake)
    }
}

impl<Stake: AtLeast32BitUnsigned + StorageEssentials + Copy> Zero for SlashEntity<Stake> {
    fn zero() -> Self {
        Self::new(Zero::zero(), Zero::zero())
    }

    fn is_zero(&self) -> bool {
        self.reputation.is_zero() && self.stake.is_zero()
    }
}

impl<Stake: AtLeast32BitUnsigned + StorageEssentials + Copy> One for SlashEntity<Stake> {
    fn one() -> Self {
        Self::new(ReputationPoint(One::one()), One::one())
    }
}

macro_rules! impl_saturating_for_slash_entity {
    ($method:ident, $op:ident) => {
        fn $method(self, rhs: Self) -> Self {
            Self::new(
                ReputationPoint((*self.reputation).$op(*rhs.reputation)),
                self.stake.$op(rhs.stake),
            )
        }
    };
}

impl<Stake: Saturating + StorageEssentials + Copy> Saturating for SlashEntity<Stake> {
    impl_saturating_for_slash_entity!(saturating_add, saturating_add);
    impl_saturating_for_slash_entity!(saturating_sub, saturating_sub);
    impl_saturating_for_slash_entity!(saturating_mul, saturating_mul);

    fn saturating_pow(self, exp: usize) -> Self {
        Self::new(
            ReputationPoint((*self.reputation).saturating_pow(exp as u32)),
            self.stake.saturating_pow(exp),
        )
    }
}

/// Computes a slash of a validator and cooperators. It returns an unapplied record to be applied at
/// some later point. Slashing metadata is updated in storage, since unapplied records are only
/// rarely intended to be dropped.
///
/// The pending slash record returned does not have initialized reporters. Those have to be set at a
/// higher level, if any.
pub(crate) fn compute_slash<T: Config>(
    params: SlashParams<T>,
) -> Option<UnappliedSlash<T::AccountId, SlashEntityOf<T>>> {
    let mut reward_payout = Zero::zero();
    let mut val_slashed = Zero::zero();

    // is the slash amount here a maximum for the era?
    let reputation_record = pallet_reputation::AccountReputation::<T>::get(params.stash)
        .unwrap_or(ReputationRecord::with_now::<T>());
    let max_slash =
        SlashEntityOf::<T>::max_slash_amount(&reputation_record.reputation, params.exposure.own);
    // used for calculating real validator loss as denominator.
    let current_values =
        SlashEntityOf::<T>::new(reputation_record.reputation.points(), params.exposure.own);
    let own_slash: SlashEntityOf<T> = max_slash * params.slash;
    if own_slash.is_any_zero() {
        // kick out the validator even if they won't be slashed,
        // as long as the misbehavior is from their most recent slashing span.
        kick_out_if_recent::<T>(params);
        return None;
    }

    let prior_slash_p = ValidatorSlashInEra::<T>::get(params.slash_era, params.stash)
        .map_or(Zero::zero(), |(prior_slash_proportion, _)| prior_slash_proportion);
    let validator_loss = SlashEntityPerbill::from_rational::<T>(own_slash, current_values);

    // compare slash proportions rather than slash values to avoid issues due to rounding
    // error.
    match validator_loss.compare_with(prior_slash_p) {
        UpdateDecision::Update => {
            ValidatorSlashInEra::<T>::insert(
                params.slash_era,
                params.stash,
                (validator_loss, own_slash),
            );
        },
        UpdateDecision::NoUpdate => {
            // we slash based on the max in era - this new event is not the max,
            // so neither the validator or any cooperators will need an update.
            //
            // this does lead to a divergence of our system from the paper, which
            // pays out some reward even if the latest report is not max-in-era.
            // we opt to avoid the cooperator lookups and edits and leave more rewards
            // for more drastic misbehavior.
            return None;
        },
    }

    // apply slash to validator.
    {
        let mut spans = fetch_spans::<T>(
            params.stash,
            params.window_start,
            &mut reward_payout,
            &mut val_slashed,
            params.reward_proportion,
        );

        let target_span = spans.compare_and_update_span_slash(params.slash_era, own_slash);

        if target_span == Some(spans.span_index()) {
            // misbehavior occurred within the current slashing span - end current span.
            // Check <https://github.com/paritytech/polkadot-sdk/issues/2650> for details.
            spans.end_span(params.now);
        }
    }

    add_offending_validator::<T>(&params);

    let mut cooperators_slashed = Vec::new();
    reward_payout = reward_payout
        + slash_cooperators::<T>(
            params.clone(),
            validator_loss,
            prior_slash_p,
            &mut cooperators_slashed,
        );

    Some(UnappliedSlash {
        validator: params.stash.clone(),
        own: val_slashed,
        others: cooperators_slashed,
        reporters: Vec::new(),
        payout: reward_payout,
    })
}

// doesn't apply any slash, but kicks out the validator if the misbehavior is from
// the most recent slashing span.
fn kick_out_if_recent<T: Config>(params: SlashParams<T>) {
    // these are not updated by era-span or end-span.
    let mut reward_payout = Zero::zero();
    let mut val_slashed = Zero::zero();
    let mut spans = fetch_spans::<T>(
        params.stash,
        params.window_start,
        &mut reward_payout,
        &mut val_slashed,
        params.reward_proportion,
    );

    if spans.era_span(params.slash_era).map(|s| s.index) == Some(spans.span_index()) {
        // Check https://github.com/paritytech/polkadot-sdk/issues/2650 for details
        spans.end_span(params.now);
    }

    add_offending_validator::<T>(&params);
}

/// Inform the [`DisablingStrategy`] implementation about the new offender and disable the list of
/// validators provided by [`make_disabling_decision`].
fn add_offending_validator<T: Config>(params: &SlashParams<T>) {
    DisabledValidators::<T>::mutate(|disabled| {
        if let Some(offender) =
            T::DisablingStrategy::decision(params.stash, params.slash_era, disabled)
        {
            // Add the validator to `DisabledValidators` and disable it. Do nothing if it is
            // already disabled.
            if let Err(index) = disabled.binary_search_by_key(&offender, |index| *index) {
                disabled.insert(index, offender);
                T::SessionInterface::disable_validator(offender);
            }
        }
    });

    // `DisabledValidators` should be kept sorted
    debug_assert!(DisabledValidators::<T>::get().windows(2).all(|pair| pair[0] < pair[1]));
}

/// Slash cooperators. Accepts general parameters and the prior slash percentage of the validator.
///
/// Returns the amount of reward to pay out.
fn slash_cooperators<T: Config>(
    params: SlashParams<T>,
    validator_loss: SlashEntityPerbill,
    prior_slash_p: SlashEntityPerbill,
    cooperators_slashed: &mut Vec<(T::AccountId, SlashEntityOf<T>)>,
) -> SlashEntityOf<T> {
    let mut reward_payout = Zero::zero();

    cooperators_slashed.reserve(params.exposure.others.len());
    for cooperator in &params.exposure.others {
        let stash = &cooperator.who;
        let mut coop_slashed = Zero::zero();

        // the era slash of a cooperator always grows, if the validator had a new max slash for the
        // era.
        let era_slash = {
            let reputation = pallet_reputation::AccountReputation::<T>::get(stash)
                .map(|r| r.reputation.points())
                .unwrap_or_default();
            let own_slash_prior =
                SlashEntityOf::<T>::new(reputation, cooperator.value) * prior_slash_p;
            let own_slash = SlashEntityOf::<T>::new(reputation, cooperator.value) * validator_loss;
            let own_slash_difference = own_slash.saturating_sub(own_slash_prior);

            let era_slash = CooperatorSlashInEra::<T>::get(params.slash_era, stash)
                .unwrap_or_else(Zero::zero)
                .saturating_add(own_slash_difference);
            CooperatorSlashInEra::<T>::insert(params.slash_era, stash, era_slash);

            era_slash
        };

        // compare the era slash against other eras in the same span.
        {
            let mut spans = fetch_spans::<T>(
                stash,
                params.window_start,
                &mut reward_payout,
                &mut coop_slashed,
                params.reward_proportion,
            );

            let target_span = spans.compare_and_update_span_slash(params.slash_era, era_slash);

            if target_span == Some(spans.span_index()) {
                // end the span, but don't chill the cooperator.
                spans.end_span(params.now);
            }
        }
        cooperators_slashed.push((stash.clone(), coop_slashed));
    }

    reward_payout
}

// helper struct for managing a set of spans we are currently inspecting.
// writes alterations to disk on drop, but only if a slash has been carried out.
//
// NOTE: alterations to slashing metadata should not be done after this is dropped.
// dropping this struct applies any necessary slashes, which can lead to free balance
// being 0, and the account being garbage-collected -- a dead account should get no new
// metadata.
struct InspectingSpans<'a, T: Config + 'a> {
    dirty: bool,
    window_start: EraIndex,
    stash: &'a T::AccountId,
    spans: SlashingSpans,
    paid_out: &'a mut SlashEntityOf<T>,
    slash_of: &'a mut SlashEntityOf<T>,
    reward_proportion: Perbill,
    _marker: sp_std::marker::PhantomData<T>,
}

// fetches the slashing spans record for a stash account, initializing it if necessary.
fn fetch_spans<'a, T: Config + 'a>(
    stash: &'a T::AccountId,
    window_start: EraIndex,
    paid_out: &'a mut SlashEntityOf<T>,
    slash_of: &'a mut SlashEntityOf<T>,
    reward_proportion: Perbill,
) -> InspectingSpans<'a, T> {
    let spans = crate::SlashingSpans::<T>::get(stash).unwrap_or_else(|| {
        let spans = SlashingSpans::new(window_start);
        crate::SlashingSpans::<T>::insert(stash, &spans);
        spans
    });

    InspectingSpans {
        dirty: false,
        window_start,
        stash,
        spans,
        slash_of,
        paid_out,
        reward_proportion,
        _marker: sp_std::marker::PhantomData,
    }
}

impl<'a, T: 'a + Config> InspectingSpans<'a, T> {
    fn span_index(&self) -> SpanIndex {
        self.spans.span_index
    }

    fn end_span(&mut self, now: EraIndex) {
        self.dirty = self.spans.end_span(now) || self.dirty;
    }

    // add some value to the slash of the staker.
    // invariant: the staker is being slashed for non-zero value here
    // although `amount` may be zero, as it is only a difference.
    fn add_slash(&mut self, amount: SlashEntityOf<T>, slash_era: EraIndex) {
        *self.slash_of = *self.slash_of + amount;
        self.spans.last_nonzero_slash = sp_std::cmp::max(self.spans.last_nonzero_slash, slash_era);
    }

    // find the span index of the given era, if covered.
    fn era_span(&self, era: EraIndex) -> Option<SlashingSpan> {
        self.spans.iter().find(|span| span.contains_era(era))
    }

    // compares the slash in an era to the overall current span slash.
    // if it's higher, applies the difference of the slashes and then updates the span on disk.
    //
    // returns the span index of the era where the slash occurred, if any.
    fn compare_and_update_span_slash(
        &mut self,
        slash_era: EraIndex,
        slash: SlashEntityOf<T>,
    ) -> Option<SpanIndex> {
        let target_span = self.era_span(slash_era)?;
        let span_slash_key = (self.stash.clone(), target_span.index);
        let mut span_record = SpanSlash::<T>::get(&span_slash_key);
        let mut changed = false;

        let slash_difference = slash.saturating_add(One::one()).saturating_sub(span_record.slashed);
        let reward = match slash_difference {
            mut difference @ SlashEntity { reputation, stake, .. }
                if *reputation > 1 && stake > One::one() =>
            {
                difference = difference.saturating_sub(One::one());
                span_record.slashed = slash;
                let reward = (slash * self.reward_proportion).saturating_sub(span_record.paid_out)
                    * REWARD_F1;
                self.add_slash(difference, slash_era);
                changed = true;

                reward
            },
            // All parts of slash are less than `span_record.slashed`
            SlashEntity { reputation: ReputationPoint(0), stake, .. } if stake.is_zero() => {
                Zero::zero()
            },
            // Case where the part of the slash equals `span_record.slashed` and the other part
            // is less than `span_record.slashed`
            _ => (slash * self.reward_proportion).saturating_sub(span_record.paid_out) * REWARD_F1,
        };

        if !reward.is_zero() {
            changed = true;
            span_record.paid_out = span_record.paid_out + reward;
            *self.paid_out = *self.paid_out + reward;
        }

        if changed {
            self.dirty = true;
            SpanSlash::<T>::insert(&span_slash_key, &span_record);
        }

        Some(target_span.index)
    }
}

impl<'a, T: 'a + Config> Drop for InspectingSpans<'a, T> {
    fn drop(&mut self) {
        // only update on disk if we slashed this account.
        if !self.dirty {
            return;
        }

        if let Some((start, end)) = self.spans.prune(self.window_start) {
            for span_index in start..end {
                SpanSlash::<T>::remove(&(self.stash.clone(), span_index));
            }
        }

        crate::SlashingSpans::<T>::insert(self.stash, &self.spans);
    }
}

/// Clear slashing metadata for an obsolete era.
pub(crate) fn clear_era_metadata<T: Config>(obsolete_era: EraIndex) {
    #[allow(deprecated)]
    ValidatorSlashInEra::<T>::remove_prefix(obsolete_era, None);
    #[allow(deprecated)]
    CooperatorSlashInEra::<T>::remove_prefix(obsolete_era, None);
}

/// Clear slashing metadata for a dead account.
pub(crate) fn clear_stash_metadata<T: Config>(
    stash: &T::AccountId,
    num_slashing_spans: u32,
) -> DispatchResult {
    let spans = match crate::SlashingSpans::<T>::get(stash) {
        None => return Ok(()),
        Some(s) => s,
    };

    ensure!(
        num_slashing_spans as usize >= spans.iter().count(),
        Error::<T>::IncorrectSlashingSpans
    );

    crate::SlashingSpans::<T>::remove(stash);

    // kill slashing-span metadata for account.
    //
    // this can only happen while the account is staked _if_ they are completely slashed.
    // in that case, they may re-bond, but it would count again as span 0. Further ancient
    // slashes would slash into this new bond, since metadata has now been cleared.
    for span in spans.iter() {
        SpanSlash::<T>::remove(&(stash.clone(), span.index));
    }

    Ok(())
}

/// apply the slash to a stash account saturating at 0.
pub fn do_slash<T: Config>(
    stash: &T::AccountId,
    mut value: SlashEntityOf<T>,
    reward_payout: &mut StakeOf<T>,
    slashed_imbalance: &mut NegativeImbalanceOf<T>,
    slash_era: EraIndex,
) -> DispatchResult {
    let controller = <Pallet<T>>::bonded(stash)
        .defensive()
        .ok_or::<DispatchError>(Error::<T>::NotController.into())?;

    let mut ledger = <Pallet<T>>::ledger(&controller)
        .ok_or::<DispatchError>(Error::<T>::NotController.into())?;

    let stake_value =
        ledger.slash_stake(value.stake, T::StakeCurrency::minimum_balance(), slash_era);

    if !stake_value.is_zero() {
        let (imbalance, missing) = T::StakeCurrency::slash(stash, stake_value);
        slashed_imbalance.subsume(imbalance);

        if !missing.is_zero() {
            // deduct overslash from the reward payout
            *reward_payout = reward_payout.saturating_sub(missing);
        }

        <Pallet<T>>::update_ledger(&controller, &ledger);
    }
    value.stake = stake_value;
    <pallet_reputation::Pallet<T>>::do_slash(stash, value.reputation)?;

    // trigger the event
    <Pallet<T>>::deposit_event(super::Event::<T>::Slashed { staker: stash.clone(), amount: value });

    Ok(())
}

/// Apply a previously-unapplied slash.
pub(crate) fn apply_slash<T: Config>(
    unapplied_slash: UnappliedSlash<T::AccountId, SlashEntityOf<T>>,
    slash_era: EraIndex,
) -> DispatchResult {
    let mut stake_reward_payout = unapplied_slash.payout.stake;
    let mut slashed_imbalance = NegativeImbalanceOf::<T>::zero();

    do_slash::<T>(
        &unapplied_slash.validator,
        unapplied_slash.own,
        &mut stake_reward_payout,
        &mut slashed_imbalance,
        slash_era,
    )?;
    Pallet::<T>::check_reputation_validator(&unapplied_slash.validator);
    Pallet::<T>::try_check_reputation_collab(&unapplied_slash.validator);

    for &(ref cooperator, cooperator_slash) in &unapplied_slash.others {
        do_slash::<T>(
            cooperator,
            cooperator_slash,
            &mut stake_reward_payout,
            &mut slashed_imbalance,
            slash_era,
        )?;
        Pallet::<T>::check_reputation_cooperator(&unapplied_slash.validator, cooperator);
    }

    let reward_payout = SlashEntity::new(unapplied_slash.payout.reputation, stake_reward_payout);

    pay_reporters::<T>(reward_payout, slashed_imbalance, &unapplied_slash.reporters)
}

/// Apply a reward payout to some reporters, paying the rewards out of the slashed imbalance.
fn pay_reporters<T: Config>(
    reward_payout: SlashEntityOf<T>,
    slashed_imbalance: NegativeImbalanceOf<T>,
    reporters: &[T::AccountId],
) -> DispatchResult {
    if reward_payout.is_zero() || reporters.is_empty() {
        T::Slash::on_unbalanced(slashed_imbalance);
        return Ok(());
    }

    let (reputation_reward, stake_reward): (ReputationPoint, StakeOf<T>) = {
        let SlashEntity { reputation, stake } = reward_payout;
        (reputation, stake.min(slashed_imbalance.peek()))
    };
    let (mut stake_reward, mut value_slashed) = slashed_imbalance.split(stake_reward);

    let prop = Perbill::from_rational(1, reporters.len() as u64);
    let reputation_per_reporter: ReputationPoint = (prop * *reputation_reward).into();
    let stake_per_reporter = prop * stake_reward.peek();
    for reporter in reporters {
        pallet_reputation::Pallet::<T>::increase_creating(reporter, reputation_per_reporter);

        let (reporter_reward, rest) = stake_reward.split(stake_per_reporter);
        stake_reward = rest;

        // this cancels out the reporter reward imbalance internally, leading
        // to no change in total issuance.
        T::StakeCurrency::resolve_creating(reporter, reporter_reward);
    }

    value_slashed.subsume(stake_reward); // remainder of reward division remains.
    T::Slash::on_unbalanced(value_slashed);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_contains_era() {
        // unbounded end
        let span = SlashingSpan { index: 0, start: 1000, length: None };
        assert!(!span.contains_era(0));
        assert!(!span.contains_era(999));

        assert!(span.contains_era(1000));
        assert!(span.contains_era(1001));
        assert!(span.contains_era(10000));

        // bounded end - non-inclusive range.
        let span = SlashingSpan { index: 0, start: 1000, length: Some(10) };
        assert!(!span.contains_era(0));
        assert!(!span.contains_era(999));

        assert!(span.contains_era(1000));
        assert!(span.contains_era(1001));
        assert!(span.contains_era(1009));
        assert!(!span.contains_era(1010));
        assert!(!span.contains_era(1011));
    }

    #[test]
    fn single_slashing_span() {
        let spans = SlashingSpans {
            span_index: 0,
            last_start: 1000,
            last_nonzero_slash: 0,
            prior: Vec::new(),
        };

        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![SlashingSpan { index: 0, start: 1000, length: None }],
        );
    }

    #[test]
    fn many_prior_spans() {
        let spans = SlashingSpans {
            span_index: 10,
            last_start: 1000,
            last_nonzero_slash: 0,
            prior: vec![10, 9, 8, 10],
        };

        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![
                SlashingSpan { index: 10, start: 1000, length: None },
                SlashingSpan { index: 9, start: 990, length: Some(10) },
                SlashingSpan { index: 8, start: 981, length: Some(9) },
                SlashingSpan { index: 7, start: 973, length: Some(8) },
                SlashingSpan { index: 6, start: 963, length: Some(10) },
            ],
        )
    }

    #[test]
    fn pruning_spans() {
        let mut spans = SlashingSpans {
            span_index: 10,
            last_start: 1000,
            last_nonzero_slash: 0,
            prior: vec![10, 9, 8, 10],
        };

        assert_eq!(spans.prune(981), Some((6, 8)));
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![
                SlashingSpan { index: 10, start: 1000, length: None },
                SlashingSpan { index: 9, start: 990, length: Some(10) },
                SlashingSpan { index: 8, start: 981, length: Some(9) },
            ],
        );

        assert_eq!(spans.prune(982), None);
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![
                SlashingSpan { index: 10, start: 1000, length: None },
                SlashingSpan { index: 9, start: 990, length: Some(10) },
                SlashingSpan { index: 8, start: 981, length: Some(9) },
            ],
        );

        assert_eq!(spans.prune(989), None);
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![
                SlashingSpan { index: 10, start: 1000, length: None },
                SlashingSpan { index: 9, start: 990, length: Some(10) },
                SlashingSpan { index: 8, start: 981, length: Some(9) },
            ],
        );

        assert_eq!(spans.prune(1000), Some((8, 10)));
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![SlashingSpan { index: 10, start: 1000, length: None },],
        );

        assert_eq!(spans.prune(2000), None);
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![SlashingSpan { index: 10, start: 2000, length: None },],
        );

        // now all in one shot.
        let mut spans = SlashingSpans {
            span_index: 10,
            last_start: 1000,
            last_nonzero_slash: 0,
            prior: vec![10, 9, 8, 10],
        };
        assert_eq!(spans.prune(2000), Some((6, 10)));
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![SlashingSpan { index: 10, start: 2000, length: None },],
        );
    }

    #[test]
    fn ending_span() {
        let mut spans = SlashingSpans {
            span_index: 1,
            last_start: 10,
            last_nonzero_slash: 0,
            prior: Vec::new(),
        };

        assert!(spans.end_span(10));

        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![
                SlashingSpan { index: 2, start: 11, length: None },
                SlashingSpan { index: 1, start: 10, length: Some(1) },
            ],
        );

        assert!(spans.end_span(15));
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![
                SlashingSpan { index: 3, start: 16, length: None },
                SlashingSpan { index: 2, start: 11, length: Some(5) },
                SlashingSpan { index: 1, start: 10, length: Some(1) },
            ],
        );

        // does nothing if not a valid end.
        assert!(!spans.end_span(15));
        assert_eq!(
            spans.iter().collect::<Vec<_>>(),
            vec![
                SlashingSpan { index: 3, start: 16, length: None },
                SlashingSpan { index: 2, start: 11, length: Some(5) },
                SlashingSpan { index: 1, start: 10, length: Some(1) },
            ],
        );
    }
}
