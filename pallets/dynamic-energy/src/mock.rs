use super::*;
use crate as pallet_dynamic_energy;

use frame_support::{construct_runtime, derive_impl, parameter_types};
use frame_system::EnsureRoot;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) const ENERGY_BURN: u128 = 1000;
pub(crate) const ENERGY_SALE: u128 = 2000;
pub(crate) const TOTAL_STAKE: u128 = 3000;

pub struct MockStaking;

impl Staking<u128> for MockStaking {
    fn total_stake(era: EraIndex) -> u128 {
        1000 + 100 * era as u128
    }
}

impl EraSessionLookup for MockStaking {
    fn active_era() -> Option<EraIndex> {
        None
    }

    fn era_for_session(session_index: SessionIndex) -> Option<EraIndex> {
        Some(session_index / SessionsPerEra::get())
    }

    fn session_range_for_era(era_index: EraIndex) -> Option<(SessionIndex, SessionIndex)> {
        Some((era_index * SessionsPerEra::get(), (era_index + 1) * SessionsPerEra::get()))
    }
}

pub struct MockWarehouse;

impl Warehouse<u128> for MockWarehouse {
    fn current_amount() -> u128 {
        500
    }

    fn max_capacity() -> u128 {
        1000
    }
}

construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Timestamp: pallet_timestamp,
        DynamicEnergy: pallet_dynamic_energy,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

#[derive_impl(pallet_timestamp::config_preludes::TestDefaultConfig)]
impl pallet_timestamp::Config for Test {}

parameter_types! {
    pub SessionsPerEra: u32 = 3;
    pub ExpectedSessionDuration: u32 = 600;
    pub DefaultAnnualPercentageRate: u32 = 100;
    pub DefaultMultiplierCoefficients: [FixedI128; 4] = [
        FixedI128::zero(), FixedI128::zero(), FixedI128::zero(), FixedI128::one()
    ];
}

impl pallet_dynamic_energy::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<u64>;
    type Balance = u128;
    type HigherPrecisionBalance = sp_core::U256;
    type Staking = MockStaking;
    type Warehouse = MockWarehouse;
    type UnixTime = Timestamp;
    type SessionsPerEra = SessionsPerEra;
    type ExpectedSessionDuration = ExpectedSessionDuration;
    type DefaultAnnualPercentageRate = DefaultAnnualPercentageRate;
    type DefaultMultiplierCoefficients = DefaultMultiplierCoefficients;
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_dynamic_energy::GenesisConfig::<Test> {
        energy_burn: ENERGY_BURN,
        energy_sale: ENERGY_SALE,
        total_stake: TOTAL_STAKE,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
