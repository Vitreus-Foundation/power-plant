use pallet_evm_precompile_balances_erc20::{Erc20BalancesPrecompile, Erc20Metadata};
use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};

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
    PrecompileAt<AddressU64<6>, Bn128Add, EthereumPrecompilesChecks>,
    PrecompileAt<AddressU64<7>, Bn128Mul, EthereumPrecompilesChecks>,
    PrecompileAt<AddressU64<8>, Bn128Pairing, EthereumPrecompilesChecks>,
    PrecompileAt<AddressU64<9>, Blake2F, EthereumPrecompilesChecks>,
    // Non-Vitreus specific nor Ethereum precompiles :
    PrecompileAt<AddressU64<1024>, Sha3FIPS256, (CallableByContract, CallableByPrecompile)>,
    PrecompileAt<AddressU64<1025>, ECRecoverPublicKey, (CallableByContract, CallableByPrecompile)>,
    // Vitreus specific precompiles:
    // NOTE: DELEGATECALL is intentionally NOT accepted for this stateful custom precompile.
    // A precompile reachable via DELEGATECALL runs with `context.address` = the calling
    // contract, which lets any contract emit spoofed Transfer/Approval events for the native
    // token and is the class of issue Moonbeam disabled for custom precompiles after a critical
    // bounty (they only re-enabled DELEGATECALL for the standard Ethereum precompiles). The
    // bridge does not need it: the router calls this precompile via normal CALL for payouts and
    // reaches it through the NativeProxy `transferFrom` (also a normal CALL) for pull-ins.
    PrecompileAt<
        AddressU64<2048>,
        Erc20BalancesPrecompile<R, NativeErc20Metadata>,
        (CallableByContract, CallableByPrecompile),
    >,
);

/// The PrecompileSet installed in the Vitreus runtime.
/// The following distribution has been decided for the precompiles
/// 0-1023: Ethereum Mainnet Precompiles
/// 1024-2047 Precompiles that are not in Ethereum Mainnet but are neither Vitreus specific
/// 2048-4095 Vitreus specific precompiles
pub type VitreusPrecompiles<R> = PrecompileSetBuilder<
    R,
    (
        // Skip precompiles if out of range.
        PrecompilesInRangeInclusive<(AddressU64<1>, AddressU64<4095>), VitreusPrecompilesAt<R>>,
    ),
>;
