#![cfg_attr(not(feature = "std"), no_std)]

use alloy_primitives::FixedBytes;
use alloy_sol_types::SolConstructor;
pub use pallet::*;
use sp_arithmetic::{traits::EnsureFrom, ArithmeticError};
use sp_core::{RuntimeDebug, H160, U256};
use sp_runtime::{
    traits::{
        AccountIdConversion, Convert, DispatchInfoOf, Get, PostDispatchInfoOf, Saturating, Zero,
    },
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    DispatchError, Perbill, Perquintill,
};

use frame_support::traits::fungible::{Balanced, Inspect};
use frame_support::{PalletId, StorageValue as StorageValueTrait};
use pallet_evm::Runner;

use frame_support::dispatch::{Pays, PostDispatchInfo};
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use sp_runtime::DispatchErrorWithPostInfo;

use fp_evm::{CallInfo, CreateInfo, ExitReason, ExitSucceed};
use frame_support::dispatch::Vec;
use pallet_evm_precompileset_assets_erc20::AccountIdAssetIdConversion;

use pallet_dex_contracts_api::{
    EthCaller, ExchangeTokenPair, FactoryContract, H160ToAlloyAddressConverter, LiquidityAmount,
    MomentOf, PositionManagerContract, QuoterContract, SwapRouterContract, TickBoundaries,
    TokenPair,
};
use pallet_dex_contracts_api::{
    NonfungiblePositionManager, NonfungibleTokenPositionDescriptor, Quoter, SwapRouter,
    UniswapV3Factory,
};
use traits::Dex;

pub mod traits;

#[cfg(test)]
mod tests;

// #[cfg(feature = "runtime-benchmarks")]
// pub mod benchmarking;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

// type BalanceOf<T> = <<T as Config>::Balances as Inspect<AccountIdOf<T>>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::{OptionQuery, *};
    use frame_system::pallet_prelude::*;
    use pallet_dex_contracts_api::MomentOf;
    use pallet_ethereum::Transaction;
    use pallet_evm::EnsureAddressOrigin;

    /// Pallet which implements fee withdrawal traits
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + pallet_evm::Config
        + pallet_ethereum::Config
        + pallet_assets::Config
        + AccountIdAssetIdConversion<Self::AccountId, Self::AssetId>
    {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The PalletId used for deriving factory contract owner address
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// TODO: document
        type BalancesPrecompileAddress: Get<H160>;

        type CurrencyLabel: Get<Vec<u8>>;
    }

    #[pallet::storage]
    pub type FactoryContractAddress<T: Config> = StorageValue<_, H160, OptionQuery>;

    #[pallet::storage]
    pub type RouterContractAddress<T: Config> = StorageValue<_, H160, OptionQuery>;

    #[pallet::storage]
    pub type PositionDescriptorContractAddress<T: Config> = StorageValue<_, H160, OptionQuery>;

    #[pallet::storage]
    pub type PositionManagerContractAddress<T: Config> = StorageValue<_, H160, OptionQuery>;

    #[pallet::storage]
    pub type QuoterContractAddress<T: Config> = StorageValue<_, H160, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Energy fee is paid to execute transaction [who, fee_amount]
        UpperFeeMultiplierUpdated,
        PoolCreated {
            address: H160,
        },
        DepositedLiqudity {
            token_pair: TokenPair,
            amount_a: U256,
            amount_b: U256,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        FailedToCreatePool,
        FailedToEnableFee,
        FailedToAddLiquidity,
        NoContractAddress,
    }

    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        pub factory_bytecode: Vec<u8>,
        pub router_bytecode: Vec<u8>,
        pub position_manager_bytecode: Vec<u8>,
        pub position_descriptor_bytecode: Vec<u8>,
        pub quoter_bytecode: Vec<u8>,
        pub _config: PhantomData<T>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T>
    where
        T::AccountId: Into<H160>,
    {
        fn build(&self) {
            frame_support::log::error!("here!");
            let GenesisConfig {
                factory_bytecode,
                router_bytecode,
                position_manager_bytecode,
                position_descriptor_bytecode,
                quoter_bytecode,
                ..
            } = self;
            let balances_wrapper_address = T::BalancesPrecompileAddress::get();
            let currency_label = T::CurrencyLabel::get();
            // if some contract failed to deploy, then there is no reason to deploy others
            Pallet::<T>::deploy_factory(factory_bytecode.clone())
                .and_then(|factory_address| {
                    Pallet::<T>::deploy_router(
                        router_bytecode.clone(),
                        factory_address,
                        balances_wrapper_address,
                    )
                    .map(|_| factory_address)
                })
                .and_then(|factory_address| {
                    Pallet::<T>::deploy_position_descriptor(
                        position_descriptor_bytecode.clone(),
                        balances_wrapper_address,
                        currency_label,
                    )
                    .map(|position_descriptor_address| {
                        (position_descriptor_address, factory_address)
                    })
                })
                .and_then(|(position_descriptor_address, factory_address)| {
                    Pallet::<T>::deploy_position_manager(
                        position_manager_bytecode.clone(),
                        factory_address,
                        balances_wrapper_address,
                        position_descriptor_address,
                    )
                    .map(|_| factory_address)
                })
                .and_then(|factory_address| {
                    Pallet::<T>::deploy_quoter(
                        quoter_bytecode.clone(),
                        factory_address,
                        balances_wrapper_address,
                    )
                });
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T>
    where
        T::AccountId: Into<H160>,
        MomentOf<T>: Into<U256>,
    {
        #[pallet::call_index(0)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn create_pool(
            origin: OriginFor<T>,
            token_pair: TokenPair,
            fee: u32,
            price: U256,
        ) -> DispatchResult {
            let _ = ensure_root(origin)?;
            let contract = FactoryContract::<T>::new(
                Self::factory_contract_address()?,
                Self::account_id().into(),
            );
            let address = contract
                .create_pool(token_pair, fee, price)
                .map_err::<DispatchError, _>(|_| Error::<T>::FailedToCreatePool.into())?;
            Self::deposit_event(Event::PoolCreated { address });
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn setup_fee(origin: OriginFor<T>, fee: u32, tick_spacing: i32) -> DispatchResult {
            let _ = ensure_root(origin)?;
            let contract = FactoryContract::<T>::new(
                Self::factory_contract_address()?,
                Self::account_id().into(),
            );
            contract
                .enable_fee_amount(fee, tick_spacing)
                .map_err::<DispatchError, _>(|_| Error::<T>::FailedToEnableFee.into())?;
            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::DbWeight::get().writes(1))]
        pub fn mint_liquidity(
            origin: OriginFor<T>,
            token_pair: TokenPair,
            fee: u32,
            tick_boundaries: TickBoundaries,
            desired_amounts: LiquidityAmount,
            recipient: H160,
            deadline: MomentOf<T>,
        ) -> DispatchResult {
            let contract = PositionManagerContract::<T>::new(
                Self::position_manager_contract_address()?,
                Self::account_id().into(),
            );
            let (liquidity, amount_a, amount_b) = contract
                .mint(
                    token_pair.clone(),
                    fee,
                    tick_boundaries,
                    desired_amounts,
                    recipient,
                    deadline,
                )
                .map_err::<DispatchError, _>(|_| Error::<T>::FailedToAddLiquidity.into())?;
            Self::deposit_event(Event::<T>::DepositedLiqudity { token_pair, amount_a, amount_b });
            Ok(().into())
        }
    }
}

impl<T: Config> Pallet<T>
where
    T::AccountId: Into<H160>,
{
    pub fn factory_contract_address() -> Result<H160, DispatchError> {
        if let Some(address) = FactoryContractAddress::<T>::get() {
            Ok(address)
        } else {
            Err(Error::<T>::NoContractAddress.into())
        }
    }

    pub fn router_contract_address() -> Result<H160, DispatchError> {
        if let Some(address) = RouterContractAddress::<T>::get() {
            Ok(address)
        } else {
            Err(Error::<T>::NoContractAddress.into())
        }
    }

    pub fn position_manager_contract_address() -> Result<H160, DispatchError> {
        if let Some(address) = PositionManagerContractAddress::<T>::get() {
            Ok(address)
        } else {
            Err(Error::<T>::NoContractAddress.into())
        }
    }

    pub fn quoter_contract_address() -> Result<H160, DispatchError> {
        if let Some(address) = QuoterContractAddress::<T>::get() {
            Ok(address)
        } else {
            Err(Error::<T>::NoContractAddress.into())
        }
    }

    pub fn account_id() -> H160 {
        T::PalletId::get().into_account_truncating()
    }

    pub fn deploy_factory(factory_bytecode: Vec<u8>) -> Option<H160> {
        Self::deploy_contract::<FactoryContractAddress<T>>(
            factory_bytecode,
            Vec::new(),
            "pool factory",
        )
    }

    pub fn deploy_router(
        router_bytecode: Vec<u8>,
        factory_address: H160,
        balances_wrapper_address: H160,
    ) -> Option<H160> {
        let balances_precompile_address =
            H160ToAlloyAddressConverter::convert(balances_wrapper_address);
        let factory_address = H160ToAlloyAddressConverter::convert(factory_address);
        let router_constructor_calldata = SwapRouter::constructorCall {
            _factory: factory_address,
            _WETH9: balances_precompile_address,
        }
        .abi_encode();
        Self::deploy_contract::<RouterContractAddress<T>>(
            router_bytecode,
            router_constructor_calldata,
            "swap router",
        )
    }

    pub fn deploy_position_manager(
        position_manager_bytecode: Vec<u8>,
        factory_address: H160,
        balances_wrapper_address: H160,
        position_descriptor_address: H160,
    ) -> Option<H160> {
        let balances_precompile_address =
            H160ToAlloyAddressConverter::convert(balances_wrapper_address);
        let factory_address = H160ToAlloyAddressConverter::convert(factory_address);
        let position_descriptor_address =
            H160ToAlloyAddressConverter::convert(position_descriptor_address);

        let position_manager_constructor_calldata = NonfungiblePositionManager::constructorCall {
            _factory: factory_address,
            _WETH9: balances_precompile_address,
            _tokenDescriptor_: position_descriptor_address,
        }
        .abi_encode();
        Self::deploy_contract::<PositionManagerContractAddress<T>>(
            position_manager_bytecode,
            position_manager_constructor_calldata,
            "position manager",
        )
    }

    pub fn deploy_position_descriptor(
        position_descriptor_bytecode: Vec<u8>,
        balances_wrapper_address: H160,
        native_currency_label: Vec<u8>,
    ) -> Option<H160> {
        let balances_precompile_address =
            H160ToAlloyAddressConverter::convert(balances_wrapper_address);
        let position_descriptor_constructor_calldata =
            NonfungibleTokenPositionDescriptor::constructorCall {
                _WETH9: balances_precompile_address,
                _nativeCurrencyLabelBytes: FixedBytes::left_padding_from(&native_currency_label),
            }
            .abi_encode();
        Self::deploy_contract::<PositionDescriptorContractAddress<T>>(
            position_descriptor_bytecode,
            position_descriptor_constructor_calldata,
            "position descriptor",
        )
    }

    pub fn deploy_quoter(
        quoter_bytecode: Vec<u8>,
        factory_address: H160,
        balances_wrapper_address: H160,
    ) -> Option<H160> {
        let balances_precompile_address =
            H160ToAlloyAddressConverter::convert(balances_wrapper_address);
        let quoter_constructor_calldata = Quoter::constructorCall {
            _factory: H160ToAlloyAddressConverter::convert(factory_address),
            _WETH9: H160ToAlloyAddressConverter::convert(balances_wrapper_address),
        }
        .abi_encode();
        Self::deploy_contract::<QuoterContractAddress<T>>(
            quoter_bytecode,
            quoter_constructor_calldata,
            "quoter",
        )
    }

    pub fn deploy_contract<Storage: StorageValueTrait<H160>>(
        mut bytecode: Vec<u8>,
        mut calldata: Vec<u8>,
        name: &str,
    ) -> Option<H160> {
        bytecode.append(&mut calldata);
        match EthCaller::<T>::create(Pallet::<T>::account_id(), bytecode, U256::zero()) {
            Ok(create_info)
                if matches!(
                    create_info.exit_reason,
                    ExitReason::Succeed(ExitSucceed::Returned)
                ) =>
            {
                let addr = create_info.value;
                Storage::put(addr.clone());
                Some(addr)
            },
            Ok(create_info) => {
                frame_support::log::error!(
                    "Failed to create {} contract: {:#?}",
                    name,
                    create_info
                );
                None
            },
            Err(e) => {
                frame_support::log::error!("Failed to create {} contract: {:#?}", name, e);
                None
            },
        }
    }
}

impl<T: Config> Dex<T::Balance, MomentOf<T>, T::AccountId> for Pallet<T>
where
    T::Balance: Into<U256> + TryFrom<U256>,
    MomentOf<T>: Into<U256>,
    T::AccountId: Into<H160>,
{
    fn swap_exact_input_single(
        token_pair: ExchangeTokenPair,
        fee: u32,
        recipient: T::AccountId,
        deadline: MomentOf<T>,
        amount_in: T::Balance,
        amount_out_minimum: T::Balance,
        sqrt_price_limit_x96: U256,
    ) -> Result<T::Balance, DispatchError> {
        let contract = SwapRouterContract::<T>::new(
            Self::router_contract_address()?,
            Self::account_id().into(),
        );
        contract
            .exact_input_single(
                token_pair,
                fee,
                recipient.into(),
                deadline,
                amount_in.into(),
                amount_out_minimum.into(),
                sqrt_price_limit_x96,
            )
            .map_err::<DispatchError, _>(|_| Error::<T>::FailedToEnableFee.into())?
            .try_into()
            .map_err(|_| ArithmeticError::Overflow.into())
    }

    fn swap_exact_output_single(
        token_pair: ExchangeTokenPair,
        fee: u32,
        recipient: T::AccountId,
        deadline: MomentOf<T>,
        amount_out: T::Balance,
        amount_in_maximum: T::Balance,
        sqrt_price_limit_x96: U256,
    ) -> Result<T::Balance, DispatchError> {
        let contract = SwapRouterContract::<T>::new(
            Self::router_contract_address()?,
            Self::account_id().into(),
        );
        contract
            .exact_output_single(
                token_pair,
                fee,
                recipient.into(),
                deadline,
                amount_out.into(),
                amount_in_maximum.into(),
                sqrt_price_limit_x96,
            )
            .map_err::<DispatchError, _>(|_| Error::<T>::FailedToEnableFee.into())?
            .try_into()
            .map_err(|_| ArithmeticError::Overflow.into())
    }
}
