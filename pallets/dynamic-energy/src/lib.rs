#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]

use frame_support::traits::{tokens::Balance, Get};
use sp_runtime::{
    traits::{
        AtLeast32BitUnsigned, CheckedConversion, CheckedDiv, CheckedMul, Debug, Ensure, One, Zero,
    },
    FixedI128, FixedPointNumber, FixedU128, Perbill, Saturating,
};

use vitreus_runtime_common::{
    EraEnergyRateCalculator, EraIndex, EraSessionLookup, OnEnergyBurn, OnEnergySell,
    OnSessionChange, SessionIndex, Staking, Warehouse,
};

pub use pallet::*;

pub mod migration;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

const LOG_TARGET: &str = "runtime::dynamic-energy";

const SECONDS_IN_YEAR: u32 = 60 * 60 * 24 * 36525 / 100;

type EnergyOf<T> = <T as Config>::Balance;
type StakeOf<T> = <T as Config>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The origin which can manage parameters of this pallet.
        type ManageOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// The balance type.
        type Balance: Balance;

        /// A type used for calculations concerning the `Balance` type to avoid possible overflows.
        type HigherPrecisionBalance: Debug
            + Ensure
            + From<u32>
            + From<u128>
            + From<EnergyOf<Self>>
            + From<StakeOf<Self>>
            + TryInto<u128>;

        /// The access to staking functionality.
        type Staking: EraSessionLookup + Staking<StakeOf<Self>>;

        /// The access to warehouse functionality.
        type Warehouse: Warehouse<EnergyOf<Self>>;

        /// Number of sessions per era.
        #[pallet::constant]
        type SessionsPerEra: Get<SessionIndex>;

        /// Session duration in seconds.
        #[pallet::constant]
        type ExpectedSessionDuration: Get<u32>;

        /// Default annual percentage rate. Represents 10ths of a percent.
        #[pallet::constant]
        type DefaultAnnualPercentageRate: Get<u32>;

        /// Default coefficients for the warehouse capacity multiplier formula.
        #[pallet::constant]
        type DefaultMultiplierCoefficients: Get<[FixedI128; 4]>;
    }

    /// An override value for the energy burn.
    #[pallet::storage]
    pub type EnergyBurnOverride<T: Config> = StorageValue<_, EnergyOf<T>>;

    /// An override value for the energy sale.
    #[pallet::storage]
    pub type EnergySaleOverride<T: Config> = StorageValue<_, EnergyOf<T>>;

    /// An override value for the total stake.
    #[pallet::storage]
    pub type TotalStakeOverride<T: Config> = StorageValue<_, StakeOf<T>>;

    /// Annual percentage rate. Represents 10ths of a percent.
    #[pallet::storage]
    #[pallet::getter(fn annual_percentage_rate)]
    pub type AnnualPercentageRate<T: Config> =
        StorageValue<_, u32, ValueQuery, T::DefaultAnnualPercentageRate>;

    /// Coefficients for the warehouse capacity multiplier formula.
    #[pallet::storage]
    pub type MultiplierCoefficients<T: Config> =
        StorageValue<_, [FixedI128; 4], ValueQuery, T::DefaultMultiplierCoefficients>;

    /// Smooth factor for generation rate.
    #[pallet::storage]
    pub type GenerationRateSmoothFactor<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultSmoothFactor<T>>;

    /// Smooth factor for exchange rate.
    #[pallet::storage]
    pub type ExchangeRateSmoothFactor<T: Config> =
        StorageValue<_, u32, ValueQuery, DefaultSmoothFactor<T>>;

    /// VNRG/VTRS exchange rate.
    #[pallet::storage]
    #[pallet::getter(fn exchange_rate)]
    pub type ExchangeRate<T: Config> = StorageValue<_, FixedU128>;

    /// The total energy burned during the session.
    #[pallet::storage]
    pub type SessionEnergyBurn<T: Config> = StorageValue<_, EnergyOf<T>, ValueQuery>;

    /// The total energy sold during the session.
    #[pallet::storage]
    pub type SessionEnergySale<T: Config> = StorageValue<_, EnergyOf<T>, ValueQuery>;

    /// Energy generation per session.
    #[pallet::storage]
    pub type EnergyGeneration<T: Config> =
        StorageMap<_, Twox64Concat, SessionIndex, EnergyOf<T>, ValueQuery>;

    /// The default smooth factor.
    /// Set to 1 to ignore past values and disable smoothing.
    #[pallet::type_value]
    pub fn DefaultSmoothFactor<T: Config>() -> u32 {
        1
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The energy burn was forcibly set.
        EnergyBurnForceSet { amount: Option<EnergyOf<T>> },
        /// The energy sale was forcibly set.
        EnergySaleForceSet { amount: Option<EnergyOf<T>> },
        /// The total stake was forcibly set.
        TotalStakeForceSet { amount: Option<StakeOf<T>> },
        /// The smooth factor for generation rate was updated.
        GenerationRateSmoothFactorUpdated { value: u32 },
        /// The smooth factor for exchange rate was updated.
        ExchangeRateSmoothFactorUpdated { value: u32 },
        /// The annual percentage rate was updated.
        AnnualPercentageRateUpdated { value: u32 },
        /// The multiplier coefficients were updated.
        MultiplierCoefficientsUpdated { coefficients: [FixedI128; 4] },
        /// The VNRG/VTRS exchange rate was updated.
        ExchangeRateUpdated { rate: FixedU128 },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Invalid smooth factor value.
        InvalidSmoothFactor,
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// An override value for the energy burn.
        pub energy_burn: EnergyOf<T>,
        /// An override value for the energy sale.
        pub energy_sale: EnergyOf<T>,
        /// An override value for the total stake.
        pub total_stake: StakeOf<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            EnergyBurnOverride::<T>::put(self.energy_burn);
            EnergySaleOverride::<T>::put(self.energy_sale);
            TotalStakeOverride::<T>::put(self.total_stake);
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Override the energy burn value.
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn force_set_energy_burn(
            origin: OriginFor<T>,
            amount: Option<EnergyOf<T>>,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            EnergyBurnOverride::<T>::set(amount);
            Self::deposit_event(Event::EnergyBurnForceSet { amount });

            Ok(())
        }

        /// Override the energy sale value.
        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn force_set_energy_sale(
            origin: OriginFor<T>,
            amount: Option<EnergyOf<T>>,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            EnergySaleOverride::<T>::set(amount);
            Self::deposit_event(Event::EnergySaleForceSet { amount });

            Ok(())
        }

        /// Override the total stake value.
        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn force_set_total_stake(
            origin: OriginFor<T>,
            amount: Option<StakeOf<T>>,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            TotalStakeOverride::<T>::set(amount);
            Self::deposit_event(Event::TotalStakeForceSet { amount });

            Ok(())
        }

        /// Set smooth factor for generation rate.
        #[pallet::call_index(3)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_generation_rate_smooth_factor(
            origin: OriginFor<T>,
            value: u32,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            ensure!(value > 0, Error::<T>::InvalidSmoothFactor);
            GenerationRateSmoothFactor::<T>::set(value);
            Self::deposit_event(Event::GenerationRateSmoothFactorUpdated { value });

            Ok(())
        }

        /// Set smooth factor for exchange rate.
        #[pallet::call_index(4)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_exchange_rate_smooth_factor(origin: OriginFor<T>, value: u32) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            ensure!(value > 0, Error::<T>::InvalidSmoothFactor);
            ExchangeRateSmoothFactor::<T>::set(value);
            Self::deposit_event(Event::ExchangeRateSmoothFactorUpdated { value });

            Ok(())
        }

        /// Set annual percentage rate, the value represents 10ths of a percent.
        #[pallet::call_index(5)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_annual_percentage_rate(origin: OriginFor<T>, value: u32) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            AnnualPercentageRate::<T>::set(value);
            Self::deposit_event(Event::AnnualPercentageRateUpdated { value });

            Ok(())
        }

        /// Set the coefficients for the warehouse capacity multiplier formula (`a*x^3 + b*x^2 + c*x + d`).
        #[pallet::call_index(6)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn set_multiplier_coefficients(
            origin: OriginFor<T>,
            a: FixedI128,
            b: FixedI128,
            c: FixedI128,
            d: FixedI128,
        ) -> DispatchResult {
            T::ManageOrigin::ensure_origin(origin)?;

            let coefficients = [a, b, c, d];

            MultiplierCoefficients::<T>::set(coefficients);
            Self::deposit_event(Event::MultiplierCoefficientsUpdated { coefficients });

            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Calculates warehouse capacity multiplier using `a*x^3+b*x^2+c*x+d` polynomial.
    pub fn calculate_warehouse_capacity_multiplier() -> FixedU128 {
        let x = FixedI128::from_perbill(Perbill::from_rational(
            T::Warehouse::current_amount(),
            T::Warehouse::max_capacity(),
        )) * 100.into(); // convert to percents

        let [a, b, c, d] = MultiplierCoefficients::<T>::get();

        let multiplier = a * x.saturating_pow(3) + b * x.saturating_pow(2) + c * x + d;

        if multiplier > FixedI128::zero() {
            FixedU128::from_inner(multiplier.into_inner() as u128)
        } else {
            log::warn!(target: LOG_TARGET, "Invalid warehouse capacity multiplier");
            FixedU128::one()
        }
    }

    fn update_generation_rate(index: SessionIndex) {
        let rate = EnergyBurnOverride::<T>::get().unwrap_or_else(SessionEnergyBurn::<T>::get);

        log::info!(target: LOG_TARGET, "Calculate generation rate: {:?}", rate);

        let old_rate = index.checked_sub(1).map(EnergyGeneration::<T>::get).unwrap_or_default();

        let rate = Self::smooth_value(rate, old_rate, GenerationRateSmoothFactor::<T>::get());

        EnergyGeneration::<T>::insert(index, rate);
    }

    fn update_exchange_rate(index: SessionIndex) {
        let energy_sale =
            EnergySaleOverride::<T>::get().unwrap_or_else(SessionEnergySale::<T>::get);

        if energy_sale.is_zero() {
            log::trace!(target: LOG_TARGET, "Energy sale is zero; skipping exchange rate update");
            return;
        }

        let total_stake = TotalStakeOverride::<T>::get()
            .or_else(|| {
                index
                    .checked_sub(1)
                    .and_then(T::Staking::era_for_session)
                    .map(T::Staking::total_stake)
            })
            .unwrap_or_default();

        let rate = match Self::calculate_exchange_rate(
            T::HigherPrecisionBalance::from(energy_sale),
            T::HigherPrecisionBalance::from(total_stake),
        ) {
            Some(rate) => {
                log::info!(target: LOG_TARGET, "Calculate exchange rate: {}", rate);
                rate
            },
            None => {
                log::warn!(
                    target: LOG_TARGET,
                    "Failed to calculate exchange rate: energy_sale: {:?}, total_stake: {:?}",
                    energy_sale, total_stake
                );
                return;
            },
        };

        let old_rate = ExchangeRate::<T>::get().unwrap_or_default();

        let rate = FixedU128::from_inner(Self::smooth_value(
            rate.into_inner(),
            old_rate.into_inner(),
            ExchangeRateSmoothFactor::<T>::get(),
        ));

        ExchangeRate::<T>::put(rate);

        Self::deposit_event(Event::ExchangeRateUpdated { rate });
    }

    /// Calculates exchange rate by the following rule:
    /// `energy_sale * seconds_in_year / total_stake * seconds_in_period * APR * capacity_multiplier`
    fn calculate_exchange_rate(
        energy_sale: T::HigherPrecisionBalance,
        total_stake: T::HigherPrecisionBalance,
    ) -> Option<FixedU128> {
        let period = Self::session_duration().into();
        let multiplier = Self::calculate_warehouse_capacity_multiplier().into_inner().into();

        log::trace!(
            target: LOG_TARGET,
            "energy_sale: {:?}, total_stake: {:?}, period: {:?}, multiplier: {:?}",
            energy_sale, total_stake, period, multiplier
        );

        let numerator = energy_sale
            .checked_mul(&1000u32.into())? // APR
            .checked_mul(&SECONDS_IN_YEAR.into())? // period
            .checked_mul(&FixedU128::DIV.into())?; // multiplier

        let denominator = total_stake
            .checked_mul(&Self::annual_percentage_rate().into())?
            .checked_mul(&period)?
            .checked_mul(&multiplier)?;

        let raw = numerator
            .checked_mul(&FixedU128::DIV.into())?
            .checked_div(&denominator)?
            .checked_into()?;

        Some(FixedU128::from_inner(raw))
    }

    // TODO: calculate duration using timestamp
    fn session_duration() -> u32 {
        T::ExpectedSessionDuration::get()
    }

    fn smooth_value<N>(value: N, old_value: N, smooth_factor: u32) -> N
    where
        N: AtLeast32BitUnsigned,
    {
        let weight = Perbill::from_rational(2, smooth_factor.saturating_plus_one());

        Saturating::saturating_add(weight * value, (Perbill::one() - weight) * old_value)
    }
}

impl<T: Config> OnEnergyBurn<EnergyOf<T>> for Pallet<T> {
    fn on_energy_burn(amount: EnergyOf<T>) {
        SessionEnergyBurn::<T>::mutate(|total| total.saturating_accrue(amount));
    }
}

impl<T: Config> OnEnergySell<EnergyOf<T>> for Pallet<T> {
    fn on_energy_sell(amount: EnergyOf<T>) {
        SessionEnergySale::<T>::mutate(|total| total.saturating_accrue(amount));
    }
}

impl<T: Config> OnSessionChange for Pallet<T> {
    fn on_new_session(index: SessionIndex) {
        Self::update_generation_rate(index);
        Self::update_exchange_rate(index);

        SessionEnergyBurn::<T>::put(EnergyOf::<T>::zero());
        SessionEnergySale::<T>::put(EnergyOf::<T>::zero());

        if let Some(old_session) = index.checked_sub(T::SessionsPerEra::get() + 1) {
            EnergyGeneration::<T>::remove(old_session);
        }
    }
}

impl<T: Config> EraEnergyRateCalculator<EnergyOf<T>> for Pallet<T> {
    fn calculate(era: EraIndex) -> Option<EnergyOf<T>> {
        let (start, end) = T::Staking::session_range_for_era(era)?;

        let mut total = EnergyOf::<T>::zero();
        for index in start..end {
            let amount = EnergyGeneration::<T>::get(index);

            log::trace!(target: LOG_TARGET, "Energy generated in session {:?}: {:?}", index, amount);

            total.saturating_accrue(amount);
        }

        log::trace!(target: LOG_TARGET, "Energy generated in era {:?}: {:?}", era, total);

        Some(total)
    }
}
