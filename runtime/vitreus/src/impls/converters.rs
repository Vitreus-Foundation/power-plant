use frame_support::traits::{
    fungibles::{Balanced, Credit},
    tokens::{ConversionFromAssetBalance, ConversionToAssetBalance},
    Equals,
};
use pallet_energy_broker::FixedPathAssetConverter;

use super::*;

type DynamicEnergyConversion =
    pallet_dynamic_energy::DynamicEnergyConversion<Runtime, (Equals<VNRG>, Equals<LNRG>)>;

pub struct NativeToEnergyConverter;
impl FixedPathAssetConverter<Runtime> for NativeToEnergyConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::Native;
    const TARGET: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());

    fn get_amount_out(amount_in: Balance) -> Option<Balance> {
        DynamicEnergyConversion::to_asset_balance(amount_in, Self::TARGET).ok()
    }

    fn get_amount_in(amount_out: Balance) -> Option<Balance> {
        DynamicEnergyConversion::from_asset_balance(amount_out, Self::TARGET).ok()
    }
}

pub struct LiquidEnergyToNativeConverter;
impl FixedPathAssetConverter<Runtime> for LiquidEnergyToNativeConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::WithId(LNRG::get());
    const TARGET: NativeOrAssetId = NativeOrAssetId::Native;

    fn get_amount_out(amount_in: Balance) -> Option<Balance> {
        DynamicEnergyConversion::from_asset_balance(amount_in, Self::SOURCE).ok()
    }

    fn get_amount_in(amount_out: Balance) -> Option<Balance> {
        DynamicEnergyConversion::to_asset_balance(amount_out, Self::SOURCE).ok()
    }

    fn resolve(
        broker: &AccountId,
        credit: Credit<AccountId, NativeAndAssets>,
    ) -> Result<(), Credit<AccountId, NativeAndAssets>> {
        match Assets::deposit(VNRG::get(), broker, credit.peek(), Precision::Exact) {
            Ok(debt) => {
                drop(credit);
                drop(debt);
                Ok(())
            },
            Err(_) => Err(credit),
        }
    }
}

pub struct StaticEnergyToNativeConverter;
impl FixedPathAssetConverter<Runtime> for StaticEnergyToNativeConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::WithId(SNRG::get());
    const TARGET: NativeOrAssetId = NativeOrAssetId::Native;

    fn get_amount_out(amount_in: Balance) -> Option<Balance> {
        AssetRate::from_asset_balance(amount_in, SNRG::get()).ok()
    }

    fn get_amount_in(amount_out: Balance) -> Option<Balance> {
        AssetRate::to_asset_balance(amount_out, SNRG::get()).ok()
    }

    fn resolve(
        _broker: &AccountId,
        credit: Credit<AccountId, NativeAndAssets>,
    ) -> Result<(), Credit<AccountId, NativeAndAssets>> {
        drop(credit);
        Ok(())
    }
}

pub struct LiquidEnergyToEnergyConverter;
impl FixedPathAssetConverter<Runtime> for LiquidEnergyToEnergyConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::WithId(LNRG::get());
    const TARGET: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());

    fn swap_fee() -> Option<u32> {
        Some(0)
    }

    fn get_amount_out(amount_in: Balance) -> Option<Balance> {
        Some(amount_in)
    }

    fn get_amount_in(amount_out: Balance) -> Option<Balance> {
        Some(amount_out)
    }

    fn withdraw(
        _broker: &AccountId,
        value: Balance,
    ) -> Result<Credit<AccountId, NativeAndAssets>, DispatchError> {
        Ok(NativeAndAssets::issue(Self::TARGET, value))
    }

    fn resolve(
        _broker: &AccountId,
        credit: Credit<AccountId, NativeAndAssets>,
    ) -> Result<(), Credit<AccountId, NativeAndAssets>> {
        drop(credit);
        Ok(())
    }
}

pub struct StaticEnergyToEnergyConverter;
impl FixedPathAssetConverter<Runtime> for StaticEnergyToEnergyConverter {
    const SOURCE: NativeOrAssetId = NativeOrAssetId::WithId(SNRG::get());
    const TARGET: NativeOrAssetId = NativeOrAssetId::WithId(VNRG::get());

    fn swap_fee() -> Option<u32> {
        Some(0)
    }

    fn get_amount_out(amount_in: Balance) -> Option<Balance> {
        Some(amount_in)
    }

    fn get_amount_in(amount_out: Balance) -> Option<Balance> {
        Some(amount_out)
    }

    fn withdraw(
        _broker: &AccountId,
        value: Balance,
    ) -> Result<Credit<AccountId, NativeAndAssets>, DispatchError> {
        Ok(NativeAndAssets::issue(Self::TARGET, value))
    }

    fn resolve(
        _broker: &AccountId,
        credit: Credit<AccountId, NativeAndAssets>,
    ) -> Result<(), Credit<AccountId, NativeAndAssets>> {
        drop(credit);
        Ok(())
    }
}
