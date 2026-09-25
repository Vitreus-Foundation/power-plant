#![allow(clippy::collapsible_else_if, unused_parens)]

use super::*;

pub type Permanent = (
    pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
    pallet_energy_generation::migrations::FixCooperatorStake<Runtime>,
);

pub type V0213 =
    (InitTechnicalCommitteeTreasury, pallet_privileges::migration::MigrateToV1<Runtime>);

#[cfg(feature = "mainnet-runtime")]
pub type Unreleased = ();
#[cfg(feature = "testnet-runtime")]
pub type Unreleased = (
    // Funds the launch-treasury vault with the ED once from the Treasury, so the
    // first routed fee isn't withheld. Idempotent.
    pallet_launch_treasury::migrations::FundVault<Runtime, xcm_config::TreasuryAccount>,
);

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
