//! Exchanges the runtime's pallets use to buy and sell energy for native tokens.

// TODO: move the energy broker's config and converters here from lib.rs.

use crate::{EnergyAsset, EnergyBroker, LiquidEnergyAsset, NativeAsset, StaticEnergyAsset};
use vitreus_runtime_common::NativeEnergyExchange;

/// Buys VNRG for VTRS through the energy broker.
pub type NativeToEnergyExchange = NativeEnergyExchange<EnergyBroker, NativeAsset, EnergyAsset>;

/// Sells SNRG for VTRS through the energy broker.
pub type StaticEnergyToNativeExchange =
    NativeEnergyExchange<EnergyBroker, NativeAsset, StaticEnergyAsset>;

/// Sells LNRG for VTRS through the energy broker.
pub type LiquidEnergyToNativeExchange =
    NativeEnergyExchange<EnergyBroker, NativeAsset, LiquidEnergyAsset>;
