use super::*;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

pub struct Initialize<T, Staking, EnergyExchangeRate, EnergyBurn, EnergySale, TotalStake>(
    core::marker::PhantomData<(T, Staking, EnergyExchangeRate, EnergyBurn, EnergySale, TotalStake)>,
);

impl<
        T: Config,
        Staking: EraSessionLookup,
        EnergyExchangeRate: Get<FixedU128>,
        EnergyBurn: Get<EnergyOf<T>>,
        EnergySale: Get<EnergyOf<T>>,
        TotalStake: Get<StakeOf<T>>,
    > OnRuntimeUpgrade
    for Initialize<T, Staking, EnergyExchangeRate, EnergyBurn, EnergySale, TotalStake>
{
    fn on_runtime_upgrade() -> Weight {
        if !ExchangeRate::<T>::exists() {
            ExchangeRate::<T>::put(EnergyExchangeRate::get());
            log::info!("ExchangeRate: {}", EnergyExchangeRate::get());

            EnergyBurnOverride::<T>::put(EnergyBurn::get());
            log::info!("EnergyBurnOverride: {:?}", EnergyBurn::get());

            EnergySaleOverride::<T>::put(EnergySale::get());
            log::info!("EnergySaleOverride: {:?}", EnergySale::get());

            TotalStakeOverride::<T>::put(TotalStake::get());
            log::info!("TotalStakeOverride: {:?}", TotalStake::get());

            let mut count = 0;
            let sessions = Staking::active_era().and_then(Staking::session_range_for_era);
            if let Some((start, end)) = sessions {
                for index in start..end {
                    EnergyGeneration::<T>::insert(index, EnergyBurn::get());

                    log::info!("EnergyGeneration[{:?}]: {:?}", index, EnergyBurn::get());

                    count += 1;
                }
            }

            T::DbWeight::get().reads_writes(4, count + 4)
        } else {
            T::DbWeight::get().reads(1)
        }
    }
}
