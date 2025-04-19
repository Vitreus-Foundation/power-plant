#![allow(clippy::collapsible_else_if, unused_parens)]

use super::*;

pub type Permanent = (
    pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
    pallet_energy_generation::migrations::FixCooperatorStake<Runtime>,
);

pub type V0213 =
    (InitTechnicalCommitteeTreasury, pallet_privileges::migration::MigrateToV1<Runtime>);

pub type Unreleased = ();

pub struct InitTechnicalCommitteeTreasury;
impl frame_support::traits::OnRuntimeUpgrade for InitTechnicalCommitteeTreasury {
    fn on_runtime_upgrade() -> Weight {
        use frame_support::traits::{Currency, ExistenceRequirement};

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
