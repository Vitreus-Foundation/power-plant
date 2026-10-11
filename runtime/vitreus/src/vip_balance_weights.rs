//! Admit native balance calls with the cost of VIPP balance observation included.
//!
//! The SDK balance weights predate our account-store observer. Utility batches
//! sum these weights before execution, so the surcharge must be declared here,
//! rather than added to block consumption after a transfer has already run.
//! Other pallets calling the currency directly need their own benchmark audit.

use core::marker::PhantomData;
use frame_support::weights::Weight;
use pallet_balances::WeightInfo as BalanceWeightInfo;

type SdkWeight<T> = pallet_balances::weights::SubstrateWeight<T>;

/// SDK native balance weights plus a bounded VIPP observer allowance.
/// The shared NAC allowance is calibrated against retained local compiled-WASM
/// benchmarks; it is not a certification of production validator hardware.
pub struct VippBalanceWeights<T>(PhantomData<T>);

impl<T: frame_system::Config> VippBalanceWeights<T> {
    fn observed_accounts(accounts: u64) -> Weight {
        crate::vip_currency_weights::observed_accounts::<T>(accounts)
    }
}

impl<T: frame_system::Config> BalanceWeightInfo for VippBalanceWeights<T> {
    fn transfer_allow_death() -> Weight {
        SdkWeight::<T>::transfer_allow_death().saturating_add(Self::observed_accounts(4))
    }

    fn transfer_keep_alive() -> Weight {
        SdkWeight::<T>::transfer_keep_alive().saturating_add(Self::observed_accounts(4))
    }

    fn force_set_balance_creating() -> Weight {
        SdkWeight::<T>::force_set_balance_creating().saturating_add(Self::observed_accounts(2))
    }

    fn force_set_balance_killing() -> Weight {
        SdkWeight::<T>::force_set_balance_killing().saturating_add(Self::observed_accounts(3))
    }

    fn force_transfer() -> Weight {
        SdkWeight::<T>::force_transfer().saturating_add(Self::observed_accounts(4))
    }

    fn transfer_all() -> Weight {
        SdkWeight::<T>::transfer_all().saturating_add(Self::observed_accounts(4))
    }

    fn force_unreserve() -> Weight {
        SdkWeight::<T>::force_unreserve().saturating_add(Self::observed_accounts(2))
    }

    fn upgrade_accounts(accounts: u32) -> Weight {
        SdkWeight::<T>::upgrade_accounts(accounts)
            .saturating_add(Self::observed_accounts(u64::from(accounts).saturating_mul(2)))
    }

    fn force_adjust_total_issuance() -> Weight {
        // Updating issuance does not mutate an account balance.
        SdkWeight::<T>::force_adjust_total_issuance()
    }

    fn burn_allow_death() -> Weight {
        SdkWeight::<T>::burn_allow_death().saturating_add(Self::observed_accounts(3))
    }

    fn burn_keep_alive() -> Weight {
        SdkWeight::<T>::burn_keep_alive().saturating_add(Self::observed_accounts(2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountId, BlockWeights, Runtime, RuntimeCall};
    use frame_support::dispatch::{DispatchClass, GetDispatchInfo};

    fn normal_block_limit() -> u64 {
        BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap().ref_time()
    }

    #[test]
    fn utility_cannot_admit_a_thousand_native_transfers_at_sdk_only_cost() {
        let transfer = RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive {
            dest: AccountId::from([7u8; 20]),
            value: 1,
        });
        let batch =
            RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![transfer; 1000] });
        let block_limit = normal_block_limit();

        // The pre-observer SDK declaration would admit these transfer costs.
        assert!(
            SdkWeight::<Runtime>::transfer_keep_alive().saturating_mul(1000).ref_time()
                < block_limit
        );
        // The runtime call declaration now rejects the batch before observation.
        assert!(batch.get_dispatch_info().weight.ref_time() > block_limit);
    }

    #[test]
    fn bulk_account_upgrades_reserve_observation_for_each_account() {
        let upgrade = RuntimeCall::Balances(pallet_balances::Call::upgrade_accounts {
            who: vec![AccountId::from([8u8; 20]); 2000],
        });
        assert!(upgrade.get_dispatch_info().weight.ref_time() > normal_block_limit());
    }
}
