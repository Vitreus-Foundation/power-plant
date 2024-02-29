use frame_support::parameter_types;
use pallet_evm_precompile_balances_erc20::{Erc20BalancesPrecompile, Erc20Metadata};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
use pallet_evm_precompileset_assets_erc20::{Erc20AssetsPrecompileSet, IsForeign, IsLocal};
use precompile_utils::precompile_set::*;

pub struct NativeErc20Metadata;

/// ERC20 metadata for the native token.
impl Erc20Metadata for NativeErc20Metadata {
	/// Returns the name of the token.
	fn name() -> &'static str {
		"VTRS token"
	}

	/// Returns the symbol of the token.
	fn symbol() -> &'static str {
		"VTRS"
	}

	/// Returns the decimals places of the token.
	fn decimals() -> u8 {
		18
	}

	/// Must return `true` only if it represents the main native currency of
	/// the network. It must be the currency used in `pallet_evm`.
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

pub type BalancesPrecompileAddress = AddressU64<2050>; 

parameter_types! {
	pub ForeignAssetPrefix: &'static [u8] = FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX;
	pub LocalAssetPrefix: &'static [u8] = LOCAL_ASSET_PRECOMPILE_ADDRESS_PREFIX;
}

type EthereumPrecompilesChecks = (AcceptDelegateCall, CallableByContract, CallableByPrecompile);

#[precompile_utils::precompile_name_from_address]
type VitreusPrecompilesAt<R> = (
	// Ethereum precompiles:
	// We allow DELEGATECALL to stay compliant with Ethereum behavior.
	PrecompileAt<AddressU64<1>, ECRecover, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<2>, Sha256, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<3>, Ripemd160, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<4>, Identity, EthereumPrecompilesChecks>,
	PrecompileAt<AddressU64<5>, Modexp, EthereumPrecompilesChecks>,
	// Non-Moonbeam specific nor Ethereum precompiles :
	PrecompileAt<AddressU64<1024>, Sha3FIPS256, (CallableByContract, CallableByPrecompile)>,
	PrecompileAt<AddressU64<1026>, ECRecoverPublicKey, (CallableByContract, CallableByPrecompile)>,
	PrecompileAt<
		BalancesPrecompileAddress,
		Erc20BalancesPrecompile<R, NativeErc20Metadata>,
		(CallableByContract, CallableByPrecompile),
	>,
);

pub type VitreusPrecompiles<R> = PrecompileSetBuilder<
	R,
	(
		// Skip precompiles if out of range.
		PrecompilesInRangeInclusive<(AddressU64<1>, AddressU64<4095>), VitreusPrecompilesAt<R>>,
		PrecompileSetStartingWith<
			LocalAssetPrefix,
			Erc20AssetsPrecompileSet<R, IsLocal, ()>,
			(CallableByContract, CallableByPrecompile),
		>,
	),
>;
