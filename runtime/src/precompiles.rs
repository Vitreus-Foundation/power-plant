use pallet_evm_precompile_balances_erc20::{Erc20BalancesPrecompile, Erc20Metadata};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
use pallet_evm_precompileset_assets_erc20::{Erc20AssetsPrecompileSet, IsForeign, IsLocal};
use precompile_utils::precompile_set::*;
use frame_support::parameter_types;

use crate::asset_config::{ForeignAssetInstance, LocalAssetInstance};

type EthereumPrecompilesChecks = (AcceptDelegateCall, CallableByContract, CallableByPrecompile);

pub struct NativeErc20Metadata;

impl Erc20Metadata for NativeErc20Metadata {
    fn name() -> &'static str {
        "Vitreus Power Plant Token"
    }

    fn symbol() -> &'static str {
        "VTRS"
    }

    fn decimals() -> u8 {
        18
    }

    fn is_native_currency() -> bool {
        true
    }
}

/// The asset precompile address prefix. Addresses that match against this prefix will be routed
/// to Erc20AssetsPrecompileSet being marked as foreign
pub const FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[255u8; 4];
/// The asset precompile address prefix. Addresses that match against this prefix will be routed
/// to Erc20AssetsPrecompileSet being marked as local
pub const LOCAL_ASSET_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[255u8, 255u8, 255u8, 254u8];

parameter_types! {
    pub ForeignAssetPrefix: &'static [u8] = FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX;
    pub LocalAssetPrefix: &'static [u8] = LOCAL_ASSET_PRECOMPILE_ADDRESS_PREFIX;
}

#[precompile_utils::precompile_name_from_address]
type VitreusPrecompilesAt<R> = (
    // Ethereum precompiles:
    // We allow DELEGATECALL to stay compliant with Ethereum behavior.
    PrecompileAt<AddressU64<1>, ECRecover, EthereumPrecompilesChecks>,
    PrecompileAt<AddressU64<2>, Sha256, EthereumPrecompilesChecks>,
    PrecompileAt<AddressU64<3>, Ripemd160, EthereumPrecompilesChecks>,
    PrecompileAt<AddressU64<4>, Identity, EthereumPrecompilesChecks>,
    PrecompileAt<AddressU64<5>, Modexp, EthereumPrecompilesChecks>,
    // Non-Vitreus specific nor Ethereum precompiles :
    PrecompileAt<AddressU64<1024>, Sha3FIPS256, (CallableByContract, CallableByPrecompile)>,
    PrecompileAt<AddressU64<1025>, ECRecoverPublicKey, (CallableByContract, CallableByPrecompile)>,
    // Vitreus specific precompiles:
    PrecompileAt<
        AddressU64<2050>,
        Erc20BalancesPrecompile<R, NativeErc20Metadata>,
        (CallableByContract, CallableByPrecompile),
    >,
);

/// The PrecompileSet installed in the Vitreus runtime.
/// We include the nine Istanbul precompiles
/// (https://github.com/ethereum/go-ethereum/blob/3c46f557/core/vm/contracts.go#L69)
/// as well as a special precompile for dispatching Substrate extrinsics
/// The following distribution has been decided for the precompiles
/// 0-1023: Ethereum Mainnet Precompiles
/// 1024-2047 Precompiles that are not in Ethereum Mainnet but are neither Vitreus specific
/// 2048-4095 Vitreus specific precompiles
pub type VitreusPrecompiles<R> = PrecompileSetBuilder<
    R,
    (
        // Skip precompiles if out of range.
        PrecompilesInRangeInclusive<(AddressU64<1>, AddressU64<4095>), VitreusPrecompilesAt<R>>,
        // Prefixed precompile sets (XC20)
        PrecompileSetStartingWith<
            ForeignAssetPrefix,
            Erc20AssetsPrecompileSet<R, IsForeign, ForeignAssetInstance>,
            (CallableByContract, CallableByPrecompile),
        >,
        PrecompileSetStartingWith<
            LocalAssetPrefix,
            Erc20AssetsPrecompileSet<R, IsLocal, LocalAssetInstance>,
            (CallableByContract, CallableByPrecompile),
        >,
    ),
>;
