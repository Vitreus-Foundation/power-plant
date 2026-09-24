#![allow(clippy::collapsible_else_if, unused_parens)]

use super::*;

pub type Permanent = (
    pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
    pallet_energy_generation::migrations::FixCooperatorStake<Runtime>,
);

pub type V0213 =
    (InitTechnicalCommitteeTreasury, pallet_privileges::migration::MigrateToV1<Runtime>);

pub type Unreleased = (SetPrecompileCode,);

pub struct SetPrecompileCode;
impl frame_support::traits::OnRuntimeUpgrade for SetPrecompileCode {
    fn on_runtime_upgrade() -> Weight {
        use sp_core::H160;

        let precompile_address = H160::from_low_u64_be(2048);

        // fe = INVALID opcode — prevents accidental execution as a contract
        let dummy_code: sp_std::vec::Vec<u8> = sp_std::vec![0xfe];

        pallet_evm::AccountCodes::<Runtime>::insert(precompile_address, &dummy_code);

        log::info!(
            "SetPrecompileCode: wrote {} byte(s) to EVM AccountCodes for 0x{:x}",
            dummy_code.len(),
            precompile_address,
        );

        <Runtime as frame_system::Config>::DbWeight::get().writes(1)
    }
}

pub struct InitTechnicalCommitteeTreasury;
impl frame_support::traits::OnRuntimeUpgrade for InitTechnicalCommitteeTreasury {
    fn on_runtime_upgrade() -> Weight {
        if !System::account_exists(&TechnicalCommitteeTreasury::account_id()) {
            let amount = <Balances as Currency<_>>::minimum_balance();
            let res = <Balances as Currency<_>>::transfer(
                &Treasury::account_id(),
                &TechnicalCommitteeTreasury::account_id(),
                amount,
                ExistenceRequirement::KeepAlive,
            );

            match res {
                Ok(_) => {
                    log::info!("Transfer {} tokens from Treasury to TechnicalTreasury", amount)
                },
                Err(_) => log::warn!("Failed to initialize TechnicalTreasury"),
            }
        }

        Weight::zero()
    }
}
