//! Benchmarks for calls that mint VIPP NFTs through the runtime's real callbacks.

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::{
    tokens::nonfungibles_v2::{Inspect, Mutate},
    Currency, VestingSchedule,
};
use pallet_privileges::{PenaltyType, VipMemberInfo, VippMemberInfo};
use sp_runtime::traits::ValidateUnsigned;

pub struct Pallet<T: Config>(frame_system::Pallet<T>);
pub trait Config: frame_system::Config {}
impl Config for Runtime {}

fn limit() -> u32 {
    <Runtime as pallet_nac_managing::Config>::MaxVippNftsPerAccount::get()
}

fn destination() -> AccountId {
    AccountId::from([0x42; 20])
}

fn signature() -> pallet_claiming::EcdsaSignature {
    // Signature of Prefix + the SCALE-encoded destination in lowercase hex.
    pallet_claiming::EcdsaSignature(hex_literal::hex!(
        "c45240d6e76c8e2b8cdb3cdea4d8e7e4cebadc90712f3fb490c37f92e946f0491041662518cf66683f235b7d4c0fb4e5e5ae0132d8cc9b482020944d28c80a931c"
    ))
}

fn setup(owned: u32, claim: u32, vip: bool) -> Result<AccountId, BenchmarkError> {
    let who = destination();
    System::set_block_number(1);
    for collection in [NftCollectionId::get(), VIPPCollectionId::get()] {
        if <Nfts as Inspect<AccountId>>::collection_owner(&collection).is_none() {
            NacManaging::create_collection(&who)?;
        }
    }
    let _ = <Balances as Currency<AccountId>>::make_free_balance_be(&who, 10_000 * vtrs::UNITS);
    if NacManaging::get_nac_level(&who).is_none() {
        NacManaging::mint(RuntimeOrigin::root(), 2, who)?;
    }
    let (_, nac) = NacManaging::get_nac_level(&who).ok_or(BenchmarkError::Stop("NAC missing"))?;
    <Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
        &NftCollectionId::get(),
        &nac,
        &[0, 0, 2],
        &(Balance::from(claim) * vtrs::UNITS).encode(),
    )?;
    let mut caps = Vec::new();
    for id in 0..owned {
        <Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::mint_into(
            &VIPPCollectionId::get(),
            &id,
            &who,
            &pallet_nfts::ItemConfig { settings: pallet_nfts::ItemSettings::all_enabled() },
            true,
        )?;
        <Nfts as Mutate<AccountId, pallet_nfts::ItemConfig>>::set_attribute(
            &VIPPCollectionId::get(),
            &id,
            &[0, 0, 2],
            &vtrs::UNITS.encode(),
        )?;
        caps.push((id, Perbill::from_percent(95) * vtrs::UNITS));
    }
    // A pre-existing cache exercises the bounded membership search for every item.
    pallet_privileges::VippMembers::<Runtime>::insert(
        who,
        VippMemberInfo::<Runtime> { points: 123, active_vipp_threshold: caps },
    );
    pallet_energy_generation::Validators::<Runtime>::insert(
        who,
        pallet_energy_generation::ValidatorPrefs::default(),
    );
    let mut ledger = pallet_energy_generation::StakingLedger::<Runtime>::default_from(who);
    ledger.total = 100 * vtrs::UNITS;
    ledger.active = ledger.total;
    pallet_energy_generation::Ledger::<Runtime>::insert(who, ledger);
    if vip {
        pallet_privileges::VipMembers::<Runtime>::insert(
            who,
            VipMemberInfo::<Runtime> {
                start: 0,
                tax_type: PenaltyType::Flat,
                points: 0,
                active_stake: 100 * vtrs::UNITS,
            },
        );
    }
    Ok(who)
}

fn prepare_claim(owned: u32) -> Result<AccountId, BenchmarkError> {
    let who = setup(owned, owned, true)?;
    let _ = <Balances as Currency<AccountId>>::make_free_balance_be(
        &Claiming::claim_account_id(),
        10 * vtrs::UNITS,
    );
    for _ in 1..<Runtime as pallet_vesting::Config>::MAX_VESTING_SCHEDULES {
        <Vesting as VestingSchedule<AccountId>>::add_vesting_schedule(&who, vtrs::UNITS, 1, 1)?;
    }
    Claiming::mint_claim(
        RuntimeOrigin::root(),
        pallet_claiming::EthereumAddress(hex_literal::hex!(
            "1a642f0e3c3af545e7acbd38b07251b3990914f1"
        )),
        vtrs::UNITS,
        Some((vtrs::UNITS, 1, 1)),
        Some(()),
    )?;
    Ok(who)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn vip_entry_mint() -> Result<(), BenchmarkError> {
        let who = setup(limit() - 1, limit(), false)?;
        #[block]
        {
            Privileges::become_vip_status(RuntimeOrigin::signed(who), PenaltyType::Flat)?;
        }
        let member = Privileges::vipp_members(who).ok_or(BenchmarkError::Stop("VIPP missing"))?;
        assert_eq!(member.active_vipp_threshold.len(), limit() as usize);
        assert_eq!(member.points, 0);
        Ok(())
    }

    #[benchmark]
    fn vip_entry_no_mint() -> Result<(), BenchmarkError> {
        let who = setup(limit(), limit(), false)?;
        #[block]
        {
            Privileges::become_vip_status(RuntimeOrigin::signed(who), PenaltyType::Flat)?;
        }
        let member = Privileges::vipp_members(who).ok_or(BenchmarkError::Stop("VIPP missing"))?;
        assert_eq!(member.active_vipp_threshold.len(), limit() as usize);
        assert_eq!(member.points, 0);
        Ok(())
    }

    #[benchmark]
    fn vip_entry_limit() -> Result<(), BenchmarkError> {
        let who = setup(limit(), limit() + 1, false)?;
        #[block]
        {
            assert_eq!(
                Privileges::become_vip_status(RuntimeOrigin::signed(who), PenaltyType::Flat),
                Err(pallet_nac_managing::Error::<Runtime>::TooManyVippNfts.into())
            );
        }
        assert!(Privileges::vip_members(who).is_none());
        Ok(())
    }

    #[benchmark]
    fn claim_mint() -> Result<(), BenchmarkError> {
        let who = prepare_claim(limit() - 1)?;
        let call =
            pallet_claiming::Call::<Runtime>::claim { dest: who, ethereum_signature: signature() };
        #[block]
        {
            assert!(Claiming::validate_unsigned(TransactionSource::InBlock, &call).is_ok());
            Claiming::claim(RuntimeOrigin::none(), who, signature())?;
        }
        let member = Privileges::vipp_members(who).ok_or(BenchmarkError::Stop("VIPP missing"))?;
        assert_eq!(member.active_vipp_threshold.len(), limit() as usize);
        assert_eq!(member.points, 123);
        Ok(())
    }

    #[benchmark]
    fn claim_limit() -> Result<(), BenchmarkError> {
        let who = prepare_claim(limit())?;
        let call =
            pallet_claiming::Call::<Runtime>::claim { dest: who, ethereum_signature: signature() };
        #[block]
        {
            assert!(Claiming::validate_unsigned(TransactionSource::InBlock, &call).is_ok());
            assert_eq!(
                Claiming::claim(RuntimeOrigin::none(), who, signature()),
                Err(pallet_nac_managing::Error::<Runtime>::TooManyVippNfts.into())
            );
        }
        assert_eq!(NacManaging::owned_vipp_claims(&who)?.len(), limit() as usize);
        Ok(())
    }

    impl_benchmark_test_suite!(
        Pallet,
        sp_io::TestExternalities::new(RuntimeGenesisConfig::default().build_storage().unwrap()),
        Runtime
    );
}

#[cfg(test)]
use sp_runtime::BuildStorage;
