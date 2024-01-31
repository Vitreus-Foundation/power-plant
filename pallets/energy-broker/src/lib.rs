#![cfg_attr(not(feature = "std"), no_std)]

use core::ops::AddAssign;

use bitmaps::Bitmap;
use frame_support::{
    ensure,
    traits::{
        fungible::{Balanced, Inspect, Mutate},
        tokens::{Fortitude, Precision, Preservation},
    },
    PalletId,
};
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::{
    traits::{EnsureFixedPointNumber, EnsureMul},
    ArithmeticError, FixedPointNumber, FixedPointOperand, FixedU128,
};
use sp_core::{RuntimeDebug, H160, U256};
use sp_runtime::{
    traits::{
        AccountIdConversion, Convert, DispatchInfoOf, Get, One, PostDispatchInfoOf, Saturating,
        Zero,
    },
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    DispatchError, DispatchResult, Perbill, Perquintill,
};

// #[cfg(test)]
// pub(crate) mod mock;

// #[cfg(test)]
// mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

pub type RawTickBitmap = Bitmap<256>;
pub type TickWord = Tick;
pub type Price = U256;

pub const MIN_TICK: Tick = -887272;
pub const MAX_TICK: Tick = -MIN_TICK;

pub const TICK_LOW_SUB: Price = U256([6552757943157144234, 184476617836266586, 0, 0]);
pub const TICK_HIGH_ADD: Price = U256([4998474450511881007, 15793544031827761793, 0, 0]);
pub const LOG_2_10001: Price = U256([11745905768312294533, 13863, 0, 0]);

pub enum SwapDirection {
    SellingA,
    SellingB,
}

pub enum TickLiquidityState {
    Flipped,
    Same,
}

pub enum TickSearchStep {
    Initialized { next_tick: Tick },
    NotInitialized { next_tick: Tick },
}

#[derive(Encode, Decode, TypeInfo, RuntimeDebug, Clone, Default, MaxEncodedLen)]
pub struct TickBitmap([u128; 2]);

impl Into<RawTickBitmap> for TickBitmap {
    fn into(self) -> RawTickBitmap {
        self.0.into()
    }
}

impl From<RawTickBitmap> for TickBitmap {
    fn from(value: RawTickBitmap) -> Self {
        Self(value.into())
    }
}

#[derive(Encode, Decode, TypeInfo, RuntimeDebug, Clone, Default, MaxEncodedLen)]
pub struct Slot<Price: MaxEncodedLen> {
    // original type uint160
    pub sqrt_price: Price,
    pub tick: Tick,
}

pub struct SwapState<Balance, Price> {
    amount_specified_remaining: Balance,
    amount_calculated: Balance,
    sqrt_price: Price,
    tick: Tick,
}

#[derive(Default)]
pub struct StepState<Balance: Default, Price: Default> {
    sqrt_price_start: Price,
    next_tick: Tick,
    sqrt_price_next: Price,
    amount_in: Balance,
    amount_out: Balance,
}

#[derive(
    Encode, Decode, TypeInfo, Clone, Copy, PartialEq, Eq, RuntimeDebug, Default, MaxEncodedLen,
)]
pub enum TickInfo<Balance: MaxEncodedLen> {
    #[default]
    Empty,
    Initialized {
        liquidity: Balance,
    },
}

impl<Balance: AddAssign + MaxEncodedLen> TickInfo<Balance> {
    pub fn update(&mut self, liquidity_delta: Balance) -> TickLiquidityState {
        match self {
            Self::Empty => {
                *self = Self::Initialized { liquidity: liquidity_delta };
                TickLiquidityState::Flipped
            },
            Self::Initialized { ref mut liquidity } => {
                *liquidity += liquidity_delta;
                TickLiquidityState::Same
            },
        }
    }
}

#[derive(Encode, Decode, TypeInfo, RuntimeDebug, Clone, Default, MaxEncodedLen)]
pub struct PositionInfo<Balance: MaxEncodedLen> {
    pub liquidity: Balance,
}

impl<Balance: MaxEncodedLen + AddAssign> PositionInfo<Balance> {
    fn update(&mut self, liquidity_delta: Balance) {
        self.liquidity += liquidity_delta;
    }
}

#[derive(Encode, Decode, TypeInfo, RuntimeDebug, Clone, Default, MaxEncodedLen)]
pub struct PositionId<AccountId: MaxEncodedLen> {
    pub who: AccountId,
    pub lower_tick: Tick,
    pub upper_tick: Tick,
}

// // info stored for each initialized individual tick
// struct Info {
//     // the total position liquidity that references this tick
//     pub liquidity_gross: u128,
//     // amount of net liquidity added (subtracted) when tick is crossed from left to right (right to left),
//     pub liquidityNet: u128;
//     // fee growth per unit of liquidity on the _other_ side of this tick (relative to the current tick)
//     // only has relative meaning, not absolute — the value depends on when the tick is initialized
//     uint256 feeGrowthOutside0X128;
//     uint256 feeGrowthOutside1X128;
//     // the cumulative tick value on the other side of the tick
//     int56 tickCumulativeOutside;
//     // the seconds per unit of liquidity on the _other_ side of this tick (relative to the current tick)
//     // only has relative meaning, not absolute — the value depends on when the tick is initialized
//     uint160 secondsPerLiquidityOutsideX128;
//     // the seconds spent on the other side of the tick (relative to the current tick)
//     // only has relative meaning, not absolute — the value depends on when the tick is initialized
//     uint32 secondsOutside;
//     // true iff the tick is initialized, i.e. the value is exactly equivalent to the expression liquidityGross != 0
//     // these 8 bits are set to prevent fresh sstores when crossing newly initialized ticks
//     bool initialized;
// }

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

type BalanceOf<T> = <<T as Config>::TokenA as Inspect<AccountIdOf<T>>>::Balance;

type Tick = i32;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{pallet_prelude::*, traits::tokens::Balance, Twox64Concat};
    use frame_system::pallet_prelude::*;
    use sp_arithmetic::FixedPointOperand;

    /// Pallet which implements fee withdrawal traits
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_evm::Config + pallet_ethereum::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The first of the two tokens of the pool
        type TokenA: Balanced<Self::AccountId> + Mutate<Self::AccountId>;

        /// The second of the two tokens of the pool
        type TokenB: Balanced<Self::AccountId, Balance = BalanceOf<Self>> + Mutate<Self::AccountId>;

        type Price: FixedPointNumber;

        type PalletId: Get<PalletId>;

        type Precision: Get<Price>;
        // type MaxLiquidityPerTick: Get<Self::Balance>;
    }

    #[pallet::storage]
    pub type Liquidity<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    pub type CurrentSlot<T: Config> = StorageValue<_, Slot, ValueQuery>;

    #[pallet::storage]
    pub type Ticks<T: Config> =
        StorageMap<_, Twox64Concat, Tick, TickInfo<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    pub type Positions<T: Config> = StorageMap<
        _,
        Twox64Concat,
        PositionId<AccountIdOf<T>>,
        PositionInfo<BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type TickBitmaps<T: Config> = StorageMap<_, Twox64Concat, TickWord, TickBitmap, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Energy fee is paid to execute transaction [who, fee_amount]
        UpperFeeMultiplierUpdated { sas: BalanceOf<T> },
        Mint {
            who: T::AccountId,
            lower_tick: Tick,
            upper_tick: Tick,
            liquidity_amount: BalanceOf<T>,
            amount_a: BalanceOf<T>,
            amount_b: BalanceOf<T>,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidTickRange,
        ZeroLiquidity,
        ZeroPrice,
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub sqrt_price: Price,
        pub tick: Tick,
        pub _config: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            let slot = Slot { sqrt_price: self.sqrt_price, tick: self.tick };
            CurrentSlot::<T>::put(slot);
        }
    }
}

impl<T: Config> Pallet<T>
where
    BalanceOf<T>: FixedPointOperand + TryFrom<Price> + Into<Price> + Copy,
{
    pub fn account_id() -> T::AccountId {
        T::PalletId::get().into_account_truncating()
    }

    // pls God forgive me for what I am about to do
    pub fn price_from_tick(tick: Tick) -> Price {
        let abs_tick: Price = if tick < 0 { (-tick).into() } else { tick.into() };

        let mut ratio: Price = if abs_tick & 0x1.into() != 0.into() { 
            Price::from_str_radix("0xfffcb933bd6fad37aa2d162d1a594001", 16).unwrap()
        } else {
            Price::from_str_radix("0x100000000000000000000000000000000", 16).unwrap()
        };
        if abs_tick & 0x2.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xfff97272373d413259a46990580e213a", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x4.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xfff2e50f5f656932ef12357cf3c7fdcc", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x8.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xffe5caca7e10e4e61c3624eaa0941cd0", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x10.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xffcb9843d60f6159c9db58835c926644", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x20.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xff973b41fa98c081472e6896dfb254c0", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x40.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xff2ea16466c96a3843ec78b326b52861", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x80.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xfe5dee046a99a2a811c461f1969c3053", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x100.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xfcbe86c7900a88aedcffc83b479aa3a4", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x200.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xf987a7253ac413176f2b074cf7815e54", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x400.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xf3392b0822b70005940c7a398e4b70f3", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x800.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xe7159475a2c29b7443b29c7fa6e889d9", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x1000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xd097f3bdfd2022b8845ad8f792aa5825", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x2000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0xa9f746462d870fdf8a65dc1f90e061e5", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x4000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0x70d869a156d2a1b890bb3df62baf32f7", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x8000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0x31be135f97d08fd981231505542fcfa6", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x10000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0x9aa508b5b7a84e1c677de54f3e99bc9", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x20000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0x5d6af8dedb81196699c329225ee604", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x40000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0x2216e584f5fa1ea926041bedfe98", 16).unwrap()) >> 128;
        }
        if abs_tick & 0x80000.into() != 0.into() {
            ratio = (ratio * Price::from_str_radix("0x48a170391f7dc42444e8fa2", 16).unwrap()) >> 128;
        }

        if tick > 0 {
            ratio = Price::MAX / ratio;
        }

        // this divides by 1<<32 rounding up to go from a Q128.128 to a Q128.96.
        // we then downcast because we know the result always fits within 160 bits due to our tick input constraint
        // we round up in the division so `price_from_tick` of the output price is always consistent
        (ratio >> 32) + if ratio % (1.into() << 32) == 0.into() { 0.into() } else { 1.into() }
    }

    fn log_int(mut num: Price) -> Price {
        let mut msb = Price::zero();
        for power in (0..=7).rev() {
            let shift = Price::from(1 << power);
            let comp = (Price::from(1) << shift) - Price::from(1);
            if num > comp {
                msb |= shift;
                num >>= shift;
            }
        }
        msb
    }

    fn log_frac(mut num: Price) -> Price {
        let mut log = Price::zero();
        for power in (50..=63).rev() {
            num = (num*num) >> 127;
            let shift = num >> 128;
            log |= shift << power;
            num >>= shift;
        }
        log
    }

    fn tick_from_price(price: Price) -> Tick {
        // changing precision to 128
        let ratio = price << 32;

        let msb = Self::log_int(ratio);

        let (r, mut log_2, log_2_sign) = if msb >= 128.into() {
            // ratio > 1.0
            (ratio >> (msb - Price::from(127)), (msb - Price::from(128)) << 64, true)
        } else {
            (ratio << (Price::from(127) - msb), (Price::from(128) - msb) << 64, false)
        };

        log_2 |= Self::log_frac(r);

        let log_sqrt10001 = log_2 * LOG_2_10001; // 128.128 number


        let (tick_lo, tick_hi) = if log_2_sign {
            let tick_lo = if log_sqrt10001 >= TICK_LOW_SUB {
                ((log_sqrt10001 - TICK_LOW_SUB) >> 128).low_u32() as Tick
            } else {
                ((TICK_LOW_SUB - log_sqrt10001) >> 128).low_u32() as Tick * (-1)
            };
            let tick_hi = ((TICK_HIGH_ADD + log_sqrt10001) >> 128).low_u32() as Tick;
            (tick_lo, tick_hi)
        } else {
            let tick_lo = ((TICK_LOW_SUB + log_sqrt10001) >> 128).low_u32() as Tick * (-1);
            let tick_hi = if log_sqrt10001 > TICK_HIGH_ADD {
                ((log_sqrt10001 - TICK_HIGH_ADD) >> 128).low_u32() as Tick * (-1)
            } else {
                ((TICK_HIGH_ADD - log_sqrt10001) >> 128).low_u32() as Tick
            };
            (tick_lo, tick_hi)
        };
        
        if tick_lo == tick_hi {
            tick_lo
        } else {
            let sqrt_ratio_x96 = Self::price_from_tick(tick_hi);
            if sqrt_ratio_x96 <= price {
                tick_hi
            } else {
                tick_lo
            }
        }
    }

    // amount is liquidity
    pub fn do_mint(
        who: &T::AccountId,
        lower_tick: Tick,
        upper_tick: Tick,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        ensure!(
            lower_tick < upper_tick && lower_tick >= MIN_TICK && upper_tick <= MAX_TICK,
            Error::<T>::InvalidTickRange
        );
        ensure!(amount > BalanceOf::<T>::zero(), Error::<T>::ZeroLiquidity);

        let tick_state_lower = Ticks::<T>::mutate(lower_tick, |tick_info| tick_info.update(amount));
        let tick_state_upper = Ticks::<T>::mutate(upper_tick, |tick_info| tick_info.update(amount));

        if let TickLiquidityState::Flipped = tick_state_lower {
            Self::do_flip_tick(lower_tick);
        }

        if let TickLiquidityState::Flipped = tick_state_upper {
            Self::do_flip_tick(upper_tick);
        }

        let position_id = PositionId { who: who.clone(), lower_tick, upper_tick };
        Positions::<T>::mutate(&position_id, |position_info| position_info.update(amount));

        let current_price = CurrentSlot::<T>::get().sqrt_price;
        let amount_a = Self::calculate_amount_a_delta(
            current_price,
            Self::price_from_tick(upper_tick),
            amount,
        );
        let amount_b = Self::calculate_amount_b_delta(
            current_price,
            Self::price_from_tick(lower_tick),
            amount,
        );

        // let balance_a_before = if amount_a > BalanceOf::<T>::zero() {
        //     Self::token_a_balance()
        // } else {
        //     BalanceOf::<T>::zero()
        // };
        //
        // let balance_b_before = if amount_b > BalanceOf::<T>::zero() {
        //     Self::token_b_balance()
        // } else {
        //     BalanceOf::<T>::zero()
        // };

        Self::deposit_liquidity(&who, amount_a, amount_b)?;
        
        Liquidity::<T>::mutate(|liquidity| *liquidity += amount);
        Ok(())
    }

    // TODO: need to round up values
    fn calculate_amount_a_delta(
        price_a: Price,
        price_b: Price,
        liquidity: BalanceOf<T>,
    ) -> Result<BalanceOf<T>, DispatchError> {
        // avoiding possible underflow
        if price_a > price_b {
            let (price_b, price_a) = (price_a, price_b);
        }
        ensure!(price_a > Price::zero(), Error::<T>::ZeroPrice);

        let precision = T::Precision::get();

        // L*(price_b - price_a)/(price_a*price_b)
        price_b
            .saturating_sub(price_a)
            .checked_mul(precision)
            .ok_or(ArithmeticError::Overflow.into())?
            .checked_div(price_a)
            .ok_or(ArithmeticError::Underflow.into())?
            .checked_mul(liquidity.into())
            .ok_or(ArithmeticError::Overflow.into())?
            .checked_div(price_b)
            .ok_or(ArithmeticError::Underflow.into())?
            .try_into()
            .map_err(|_| ArithmeticError::Overflow.into())
    }

    fn calculate_amount_b_delta(
        price_a: Price,
        price_b: Price,
        liquidity: BalanceOf<T>,
    ) -> Result<BalanceOf<T>, DispatchError> {
        // avoiding possible underflow
        if price_a > price_b {
            let (price_b, price_a) = (price_a, price_b);
        }
        ensure!(price_a > Price::zero(), Error::<T>::ZeroPrice);

        let precision = T::Precision::get();

        // L*(price_a - price_b)
        price_b
            .saturating_sub(price_a)
            .checked_mul(liquidity.into())
            .ok_or(ArithmeticError::Overflow.into())?
            .checked_mul(precision)
            .ok_or(ArithmeticError::Overflow.into())?
            .try_into()
            .map_err(|_| ArithmeticError::Overflow.into())
    }

    pub fn do_swap(who: &T::AccountId, swap_direction: SwapDirection, amount: BalanceOf<T>) {
        let current_slot = CurrentSlot::<T>::get();
        let mut state = SwapState {
            amount_specified_remaining: amount,
            amount_calculated: BalanceOf::<T>::zero(),
            sqrt_price: current_slot.sqrt_price,
            tick: current_slot.tick,
        };
        let liquidity = Liquidity::<T>::get();

        while state.amount_specified_remaining > BalanceOf::<T>::zero() {
            let next_tick =
                Self::next_initialized_tick_within_one_word(state.tick, swap_direction);
            let sqrt_price_next = Self::price_from_tick(next_tick);
            let step =
                StepState { sqrt_price_start: state.sqrt_price, ..Default::default() };
            (state.sqrt_price, step.amount_in, step.amount_out) =
                Self::compute_swap_step(state.sqrt_price, step.sqrt_price_next, liquidity, state.amount_specified_remaining)?;
            state.amount_specified_remaining -= step.amount_in;
            state.amount_calculated += step.amount_out;
            state.tick = Self::tick_from_price(state.sqrt_price);
        }

        CurrentSlot::<T>::mutate(|current_slot| {
            if state.tick != current_slot.tick {
                *current_slot.tick = state.tick;
                *current_slot.sqrt_price = state.sqrt_price;
            }
        });

        let (amount0, amount1) = 
    }

    pub fn compute_swap_step(
        sqrt_price_current: Price,
        sqrt_price_target: Price,
        liquidity: BalanceOf<T>,
        amount_remaining: BalanceOf<T>,
    ) -> Result<(Price, BalanceOf<T>, BalanceOf<T>), DispatchError> {
        let swap_direction = if sqrt_price_current >= sqrt_price_target {
            SwapDirection::SellingA
        } else {
            SwapDirection::SellingB
        };

        let sqrt_price_next = Self::calculate_next_sqrt_price_from_input(
            sqrt_price_current,
            liquidity,
            amount_remaining,
            swap_direction,
        );

        let mut amount_in =
            Self::calculate_amount_a_delta(sqrt_price_current, sqrt_price_next, liquidity)?;
        let mut amount_out =
            Self::calculate_amount_b_delta(sqrt_price_current, sqrt_price_next, liquidity)?;

        (amount_in, amount_out) = match swap_direction {
            SwapDirection::SellingA => (amount_in, amount_out),
            SwapDirection::SellingB => (amount_out, amount_in),
        };

        Ok((sqrt_price_next, amount_in, amount_out))
    }

    // TODO: add emergency formula in case SellingA overflows
    pub fn calculate_next_sqrt_price_from_input(
        sqrt_price: Price,
        liquidity: BalanceOf<T>,
        amount_in: BalanceOf<T>,
        swap_direction: SwapDirection,
    ) -> Result<Price, DispatchError> {
        let precision = T::Precision::get();
        match swap_direction {
            // (liq * q96 * sqrtp_cur) // (liq * q96 + amount_in * sqrtp_cur)
            SwapDirection::SellingA => {
                let numerator = sqrt_price
                    .checked_mul(precision)
                    .ok_or(ArithmeticError::Overflow.into())?
                    .checked_mul(liquidity.into())
                    .ok_or(ArithmeticError::Overflow.into())?;

                let denominator_1 = sqrt_price
                    .checked_mul(amount_in.into())
                    .ok_or(ArithmeticError::Overflow.into())?;

                let denominator_2 = precision
                    .checked_mul(liquidity.into())
                    .ok_or(ArithmeticError::Overflow.into())?;

                let denominator = denominator_1
                    .checked_add(denominator_2)
                    .ok_or(ArithmeticError::Overflow.into())?;

                numerator.checked_div(denominator).ok_or(ArithmeticError::Overflow.into())
            },
            // sqrtp_cur + (amount_in * q96) // liq
            SwapDirection::SellingB => precision
                .checked_mul(amount_in.into())
                .ok_or(ArithmeticError::Overflow.into())?
                .checked_div(liquidity.into())
                .ok_or(ArithmeticError::Overflow.into())?
                .checked_add(sqrt_price)
                .ok_or(ArithmeticError::Overflow.into()),
        }
    }

    pub fn do_swap_a_for_b(who: &T::AccountId, amount: BalanceOf<T>) {
        todo!()
    }

    fn deposit_liquidity(
        who: &AccountIdOf<T>,
        amount_a: BalanceOf<T>,
        amount_b: BalanceOf<T>,
    ) -> DispatchResult {
        let preservation = Preservation::Protect;

        let pool_account = &Self::account_id();

        T::TokenA::transfer(who, pool_account, amount_a, preservation)?;
        T::TokenB::transfer(who, pool_account, amount_b, preservation)?;
        Ok(())
    }

    pub fn token_a_balance() -> BalanceOf<T> {
        T::TokenA::reducible_balance(&Self::account_id(), Preservation::Protect, Fortitude::Force)
    }

    pub fn token_b_balance() -> BalanceOf<T> {
        T::TokenB::reducible_balance(&Self::account_id(), Preservation::Protect, Fortitude::Force)
    }

    pub fn position(tick: Tick) -> (TickWord, u8) {
        let word_pos = tick >> 8;
        let bit_pos = (tick % 256) as u8;
        return (word_pos, bit_pos);
    }

    fn do_flip_tick(tick: Tick) {
        let (word_pos, bit_pos) = Self::position(tick);
        let mut mask = RawTickBitmap::new();
        mask.set(bit_pos as usize, true);

        TickBitmaps::<T>::mutate(word_pos, |prev_bitmap| {
            let mut prev_raw_bitmap: RawTickBitmap = prev_bitmap.clone().into();
            prev_raw_bitmap ^= mask;
            *prev_bitmap = prev_raw_bitmap.into();
        });
    }

    fn next_initialized_tick_within_one_word(tick: Tick, lte: SwapDirection) -> TickSearchStep {
        match lte {
            SwapDirection::SellingA => {
                let (word_pos, bit_pos) = Self::position(tick);

                // all the 1s at or to the right of the current bit_pos
                let mask = RawTickBitmap::mask(bit_pos as usize + 1);
                let tick_bitmap = TickBitmaps::<T>::get(word_pos).into();
                let masked = mask & tick_bitmap;
                match masked.first_index() {
                    None => TickSearchStep::NotInitialized {
                        next_tick: tick.saturating_sub(bit_pos as Tick),
                    },
                    Some(index) => {
                        let offset = (bit_pos as usize).saturating_sub(index) as Tick;
                        TickSearchStep::Initialized { next_tick: tick.saturating_sub(offset) }
                    },
                }
            },
            SwapDirection::SellingB => {
                // start from the word of the next tick, since the current tick state doesn't matter
                let (word_pos, bit_pos) = Self::position(tick + 1);

                // all the 1s at or to the left of the bitPos
                let mask = !RawTickBitmap::mask(bit_pos as usize);
                let tick_bitmap = TickBitmaps::<T>::get(word_pos).into();
                let masked = mask & tick_bitmap;
                match masked.last_index() {
                    None => {
                        let offset = u8::MAX.saturating_sub(bit_pos).saturating_add(1) as Tick;
                        TickSearchStep::NotInitialized {
                            next_tick: tick.saturating_add(bit_pos as Tick),
                        }
                    },
                    Some(index) => {
                        let offset =
                            index.saturating_sub(bit_pos as usize).saturating_add(1) as Tick;
                        TickSearchStep::Initialized { next_tick: tick.saturating_add(offset) }
                    },
                }
            },
        }
    }
    // fn liquidity_from_dx(dx: BalanceOf<T>, amount_b: BalanceOf<T>) -> FixedU128 {
    //
    // }
    //
    // pub fn initial_liquidity_from_reserves(amount_x: BalanceOf<T>, amount_y: BalanceOf<T>) -> FixedU128
}
