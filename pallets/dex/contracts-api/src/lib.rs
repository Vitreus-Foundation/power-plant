#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;

pub use crate::converters::{
    AlloyAddressToH160Converter, H160ToAlloyAddressConverter, U256ToAlloyU256Converter,
};
use alloy_primitives::{Address, FixedBytes, U256 as SolU256};
use alloy_sol_types::{sol, SolCall, SolConstructor, SolInterface};
use converters::AlloyU256ToU256Converter;
use fp_evm::{CallInfo, Config as EvmConfig, CreateInfo};
use frame_support::traits::Time;
use pallet_evm::{Config, Runner};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{RuntimeDebug, H160, U256};
use sp_runtime::{traits::Convert, DispatchError};
use sp_std::vec::Vec;

mod converters;

sol! {
    contract UniswapV3Factory {
        event OwnerChanged(address indexed oldOwner, address indexed newOwner);
        event PoolCreated(
            address indexed token0,
            address indexed token1,
            uint24 indexed fee,
            int24 tickSpacing,
            address pool
        );
        event FeeAmountEnabled(uint24 indexed fee, int24 indexed tickSpacing);

        mapping(address => mapping(address => mapping(uint24 => address))) public override getPool;
        mapping(uint24 => int24) public override feeAmountTickSpacing;

        function createPool(
            address tokenA,
            address tokenB,
            uint24 fee
        ) external returns (address pool);
        function setOwner(address _owner) external override;
        function enableFeeAmount(uint24 fee, int24 tickSpacing) external;
    }

    contract UniswapV3Pool {
        function initialize(uint160 sqrtPriceX96) external override;
    }

    contract SwapRouter {
        constructor(address _factory, address _WETH9);

        struct ExactInputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
            uint160 sqrtPriceLimitX96;
        }

        /// @notice Swaps `amountIn` of one token for as much as possible of another token
        /// @param params The parameters necessary for the swap, encoded as `ExactInputSingleParams` in calldata
        /// @return amountOut The amount of the received token
        function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);

        struct ExactInputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountIn;
            uint256 amountOutMinimum;
        }

        /// @notice Swaps `amountIn` of one token for as much as possible of another along the specified path
        /// @param params The parameters necessary for the multi-hop swap, encoded as `ExactInputParams` in calldata
        /// @return amountOut The amount of the received token
        function exactInput(ExactInputParams calldata params) external payable returns (uint256 amountOut);

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }

        /// @notice Swaps as little as possible of one token for `amountOut` of another token
        /// @param params The parameters necessary for the swap, encoded as `ExactOutputSingleParams` in calldata
        /// @return amountIn The amount of the input token
        function exactOutputSingle(ExactOutputSingleParams calldata params) external payable returns (uint256 amountIn);

        struct ExactOutputParams {
            bytes path;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
        }

        /// @notice Swaps as little as possible of one token for `amountOut` of another along the specified path (reversed)
        /// @param params The parameters necessary for the multi-hop swap, encoded as `ExactOutputParams` in calldata
        /// @return amountIn The amount of the input token
        function exactOutput(ExactOutputParams calldata params) external payable returns (uint256 amountIn);
    }

    contract NonfungiblePositionManager {
        constructor(
            address _factory,
            address _WETH9,
            address _tokenDescriptor_
        );

        struct MintParams {
            address token0;
            address token1;
            uint24 fee;
            int24 tickLower;
            int24 tickUpper;
            uint256 amount0Desired;
            uint256 amount1Desired;
            uint256 amount0Min;
            uint256 amount1Min;
            address recipient;
            uint256 deadline;
        }

        function mint(MintParams calldata params)
            external
            payable
            returns (
                uint256 tokenId,
                uint128 liquidity,
                uint256 amount0,
                uint256 amount1
            );
    }

    contract NonfungibleTokenPositionDescriptor {
        constructor(address _WETH9, bytes32 _nativeCurrencyLabelBytes);
    }

    contract Quoter {
        constructor(address _factory, address _WETH9);

        function quoteExactInputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountIn,
            uint160 sqrtPriceLimitX96
        ) public override returns (uint256 amountOut);

        function quoteExactOutputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountOut,
            uint160 sqrtPriceLimitX96
        ) public override returns (uint256 amountIn);
    }
}

const DEFAULT_MAX_FEE_PER_GAS: Option<U256> = Some(U256([0, 0, 0, 10_000]));
const DEFAULT_IS_TRANSACTIONAL_FLAG: bool = true;
const DEFAULT_VALIDATE_FLAG: bool = false;

pub type MomentOf<T> = <<T as Config>::Timestamp as Time>::Moment;

pub struct EthCaller<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> EthCaller<T> {
    pub fn call(
        source: H160,
        target: H160,
        amount: U256,
        data: Vec<u8>,
    ) -> Result<CallInfo, DispatchError> {
        T::Runner::call(
            source,
            target,
            data,
            amount,
            Default::default(),
            DEFAULT_MAX_FEE_PER_GAS,
            Default::default(),
            Default::default(),
            Default::default(),
            DEFAULT_IS_TRANSACTIONAL_FLAG,
            DEFAULT_VALIDATE_FLAG,
            Default::default(),
            Default::default(),
            <T as pallet_evm::Config>::config(),
        )
        .map_err(|e| e.error.into())
    }

    pub fn create(source: H160, init: Vec<u8>, amount: U256) -> Result<CreateInfo, DispatchError> {
        T::Runner::create(
            source,
            init,
            amount,
            Default::default(),
            DEFAULT_MAX_FEE_PER_GAS,
            Default::default(),
            Default::default(),
            Default::default(),
            DEFAULT_IS_TRANSACTIONAL_FLAG,
            DEFAULT_VALIDATE_FLAG,
            Default::default(),
            Default::default(),
            <T as pallet_evm::Config>::config(),
        )
        .map_err(|e| e.error.into())
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct LiquidityAmount {
    amount_a_desired: U256,
    amount_b_desired: U256,
    amount_a_min: U256,
    amount_b_min: U256,
}

struct AlloyLiquidityAmount {
    amount_a_desired: SolU256,
    amount_b_desired: SolU256,
    amount_a_min: SolU256,
    amount_b_min: SolU256,
}

impl From<LiquidityAmount> for AlloyLiquidityAmount {
    fn from(value: LiquidityAmount) -> Self {
        let LiquidityAmount { amount_a_desired, amount_b_desired, amount_a_min, amount_b_min } =
            value;
        Self {
            amount_a_desired: U256ToAlloyU256Converter::convert(amount_a_desired),
            amount_b_desired: U256ToAlloyU256Converter::convert(amount_b_desired),
            amount_a_min: U256ToAlloyU256Converter::convert(amount_a_min),
            amount_b_min: U256ToAlloyU256Converter::convert(amount_b_min),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct TokenPair {
    token_a: H160,
    token_b: H160,
}

struct AlloyTokenPair {
    token_a: Address,
    token_b: Address,
}

impl From<TokenPair> for AlloyTokenPair {
    fn from(value: TokenPair) -> Self {
        let TokenPair { token_a, token_b } = value;
        Self {
            token_a: H160ToAlloyAddressConverter::convert(token_a),
            token_b: H160ToAlloyAddressConverter::convert(token_b),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct ExchangeTokenPair {
    token_in: H160,
    token_out: H160,
}

pub struct AlloyExchangeTokenPair {
    token_in: Address,
    token_out: Address,
}

impl From<ExchangeTokenPair> for AlloyExchangeTokenPair {
    fn from(value: ExchangeTokenPair) -> Self {
        let ExchangeTokenPair { token_in, token_out } = value;
        Self {
            token_in: H160ToAlloyAddressConverter::convert(token_in),
            token_out: H160ToAlloyAddressConverter::convert(token_out),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct TickBoundaries {
    tick_lower: i32,
    tick_upper: i32,
}

pub struct FactoryContract<T>(H160, H160, PhantomData<T>);

impl<T: Config> FactoryContract<T> {
    pub fn new(address: H160, source: H160) -> Self {
        FactoryContract(address, source, PhantomData)
    }

    pub fn address(&self) -> H160 {
        self.0
    }

    pub fn source(&self) -> H160 {
        self.1
    }

    pub fn get_pool(&self, token_pair: TokenPair, fee: u32) -> Result<H160, DispatchError> {
        let AlloyTokenPair { token_a, token_b } = token_pair.into();
        let data = UniswapV3Factory::getPoolCall::new((token_a, token_b, fee.into())).abi_encode();

        let raw_pool_address =
            EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
                .map(|v| v.value)?;
        let pool_address =
            UniswapV3Factory::getPoolCall::abi_decode_returns(&raw_pool_address, true)
                .map_err(|_| DispatchError::Other("Decoding error"))?
                ._0;

        Ok(AlloyAddressToH160Converter::convert(pool_address))
    }

    pub fn get_fee_amount_tick_spacing(&self, fee: u32) -> Result<i32, DispatchError> {
        let data = UniswapV3Factory::feeAmountTickSpacingCall::new((fee.into(),)).abi_encode();

        let raw_tick_spacing =
            EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
                .map(|v| v.value)?;
        let tick_spacing =
            UniswapV3Factory::feeAmountTickSpacingCall::abi_decode_returns(&raw_tick_spacing, true)
                .map_err(|_| DispatchError::Other("Decoding error"))?
                ._0;

        Ok(tick_spacing.into())
    }

    pub fn create_pool(
        &self,
        token_pair: TokenPair,
        fee: u32,
        price: U256,
    ) -> Result<H160, DispatchError> {
        let AlloyTokenPair { token_a, token_b } = token_pair.into();
        let data = UniswapV3Factory::UniswapV3FactoryCalls::createPool(
            UniswapV3Factory::createPoolCall::new((token_a, token_b, fee.into())),
        )
        .abi_encode();
        let raw_pool_address =
            EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
                .map(|v| v.value)?;
        let pool_address =
            UniswapV3Factory::createPoolCall::abi_decode_returns(&raw_pool_address, true)
                .map_err(|_| DispatchError::Other("Decoding error"))?
                .pool;

        let data = UniswapV3Pool::UniswapV3PoolCalls::initialize(
            UniswapV3Pool::initializeCall::new((U256ToAlloyU256Converter::convert(price),)),
        )
        .abi_encode();

        let converted_pool_address = AlloyAddressToH160Converter::convert(pool_address);

        EthCaller::<T>::call(self.source(), converted_pool_address, U256::zero(), data)?;

        Ok(converted_pool_address)
    }

    pub fn enable_fee_amount(&self, fee: u32, tick_spacing: i32) -> Result<(), DispatchError> {
        let data = UniswapV3Factory::UniswapV3FactoryCalls::enableFeeAmount(
            UniswapV3Factory::enableFeeAmountCall::new((fee.into(), tick_spacing.into())),
        )
        .abi_encode();
        EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data).map(|_| ())
    }

    pub fn set_owner(&self, owner: H160) -> Result<(), DispatchError> {
        let data = UniswapV3Factory::UniswapV3FactoryCalls::setOwner(
            UniswapV3Factory::setOwnerCall::new((H160ToAlloyAddressConverter::convert(owner),)),
        )
        .abi_encode();
        EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data).map(|_| ())
    }
}

pub struct SwapRouterContract<T>(H160, H160, PhantomData<T>);

impl<T: Config> SwapRouterContract<T>
where
    MomentOf<T>: Into<U256>,
{
    pub fn new(address: H160, source: H160) -> Self {
        SwapRouterContract(address, source, PhantomData)
    }

    pub fn address(&self) -> H160 {
        self.0
    }

    pub fn source(&self) -> H160 {
        self.1
    }

    pub fn exact_input_single(
        &self,
        token_pair: ExchangeTokenPair,
        fee: u32,
        recipient: H160,
        deadline: MomentOf<T>,
        amount_in: U256,
        amount_out_minimum: U256,
        sqrt_price_limit_x96: U256,
    ) -> Result<U256, DispatchError> {
        let AlloyExchangeTokenPair { token_in, token_out } = token_pair.into();
        let params = SwapRouter::ExactInputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee: fee.into(),
            recipient: H160ToAlloyAddressConverter::convert(recipient),
            deadline: U256ToAlloyU256Converter::convert(deadline.into()),
            amountIn: U256ToAlloyU256Converter::convert(amount_in),
            amountOutMinimum: U256ToAlloyU256Converter::convert(amount_out_minimum),
            sqrtPriceLimitX96: U256ToAlloyU256Converter::convert(sqrt_price_limit_x96),
        };
        let data = SwapRouter::exactInputSingleCall::new((params,)).abi_encode();
        let raw_amount_out =
            EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
                .map(|v| v.value)?;
        let amount_out =
            SwapRouter::exactInputSingleCall::abi_decode_returns(&raw_amount_out, true)
                .map_err(|_| DispatchError::Other("Decoding error"))?
                .amountOut;

        Ok(AlloyU256ToU256Converter::convert(amount_out))
    }

    pub fn exact_output_single(
        &self,
        token_pair: ExchangeTokenPair,
        fee: u32,
        recipient: H160,
        deadline: MomentOf<T>,
        amount_out: U256,
        amount_in_maximum: U256,
        sqrt_price_limit_x96: U256,
    ) -> Result<U256, DispatchError> {
        let AlloyExchangeTokenPair { token_in, token_out } = token_pair.into();
        let params = SwapRouter::ExactOutputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee: fee.into(),
            recipient: H160ToAlloyAddressConverter::convert(recipient),
            deadline: U256ToAlloyU256Converter::convert(deadline.into()),
            amountOut: U256ToAlloyU256Converter::convert(amount_out),
            amountInMaximum: U256ToAlloyU256Converter::convert(amount_in_maximum),
            sqrtPriceLimitX96: U256ToAlloyU256Converter::convert(sqrt_price_limit_x96),
        };
        let data = SwapRouter::exactOutputSingleCall::new((params,)).abi_encode();
        let raw_amount_in = EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
            .map(|v| v.value)?;
        let amount_out =
            SwapRouter::exactOutputSingleCall::abi_decode_returns(&raw_amount_in, true)
                .map_err(|_| DispatchError::Other("Decoding error"))?
                .amountIn;

        Ok(AlloyU256ToU256Converter::convert(amount_out))
    }
}

pub struct PositionManagerContract<T>(H160, H160, PhantomData<T>);

impl<T: Config> PositionManagerContract<T>
where
    MomentOf<T>: Into<U256>,
{
    pub fn new(address: H160, source: H160) -> Self {
        PositionManagerContract(address, source, PhantomData)
    }

    pub fn address(&self) -> H160 {
        self.0
    }

    pub fn source(&self) -> H160 {
        self.1
    }

    pub fn mint(
        &self,
        token_pair: TokenPair,
        fee: u32,
        tick_boundaries: TickBoundaries,
        desired_amounts: LiquidityAmount,
        recipient: H160,
        deadline: MomentOf<T>,
    ) -> Result<(u128, U256, U256), DispatchError> {
        let AlloyTokenPair { token_a, token_b } = token_pair.into();
        let TickBoundaries { tick_lower, tick_upper } = tick_boundaries;
        let AlloyLiquidityAmount { amount_a_desired, amount_b_desired, amount_a_min, amount_b_min } =
            desired_amounts.into();
        let params = NonfungiblePositionManager::MintParams {
            token0: token_a,
            token1: token_b,
            fee: fee.into(),
            tickLower: tick_lower.into(),
            tickUpper: tick_upper.into(),
            amount0Desired: amount_a_desired,
            amount1Desired: amount_b_desired,
            amount0Min: amount_a_min,
            amount1Min: amount_b_min,
            recipient: H160ToAlloyAddressConverter::convert(recipient),
            deadline: U256ToAlloyU256Converter::convert(deadline.into()),
        };

        let data = NonfungiblePositionManager::NonfungiblePositionManagerCalls::mint(
            NonfungiblePositionManager::mintCall { params },
        )
        .abi_encode();

        let raw_returns = EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
            .map(|v| v.value)?;
        let NonfungiblePositionManager::mintReturn { liquidity, amount0, amount1, .. } =
            NonfungiblePositionManager::mintCall::abi_decode_returns(&raw_returns, true)
                .map_err(|_| DispatchError::Other("Decoding error"))?;

        Ok((
            liquidity.into(),
            AlloyU256ToU256Converter::convert(amount0),
            AlloyU256ToU256Converter::convert(amount1),
        ))
    }
}

pub struct QuoterContract<T>(H160, H160, PhantomData<T>);

impl<T: Config> QuoterContract<T> {
    pub fn new(address: H160, source: H160) -> Self {
        QuoterContract(address, source, PhantomData)
    }

    pub fn address(&self) -> H160 {
        self.0
    }

    pub fn source(&self) -> H160 {
        self.1
    }

    pub fn quote_exact_input_single(
        &self,
        token_pair: ExchangeTokenPair,
        fee: u32,
        amount_in: U256,
        sqrt_price_limit_x96: U256,
    ) -> Result<U256, DispatchError> {
        let AlloyExchangeTokenPair { token_in, token_out } = token_pair.into();
        let data = Quoter::QuoterCalls::quoteExactInputSingle(Quoter::quoteExactInputSingleCall {
            tokenIn: token_in,
            tokenOut: token_out,
            fee,
            amountIn: U256ToAlloyU256Converter::convert(amount_in),
            sqrtPriceLimitX96: U256ToAlloyU256Converter::convert(sqrt_price_limit_x96),
        })
        .abi_encode();
        let raw_returns = EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
            .map(|v| v.value)?;
        let amount_out = Quoter::quoteExactInputSingleCall::abi_decode_returns(&raw_returns, true)
            .map_err(|_| DispatchError::Other("Decoding error"))?
            .amountOut;

        Ok(AlloyU256ToU256Converter::convert(amount_out))
    }

    pub fn quote_exact_output_single(
        &self,
        token_pair: ExchangeTokenPair,
        fee: u32,
        amount_out: U256,
        sqrt_price_limit_x96: U256,
    ) -> Result<U256, DispatchError> {
        let AlloyExchangeTokenPair { token_in, token_out } = token_pair.into();
        let data =
            Quoter::QuoterCalls::quoteExactOutputSingle(Quoter::quoteExactOutputSingleCall {
                tokenIn: token_in,
                tokenOut: token_out,
                fee,
                amountOut: U256ToAlloyU256Converter::convert(amount_out),
                sqrtPriceLimitX96: U256ToAlloyU256Converter::convert(sqrt_price_limit_x96),
            })
            .abi_encode();
        let raw_returns = EthCaller::<T>::call(self.source(), self.address(), U256::zero(), data)
            .map(|v| v.value)?;
        let amount_in = Quoter::quoteExactOutputSingleCall::abi_decode_returns(&raw_returns, true)
            .map_err(|_| DispatchError::Other("Decoding error"))?
            .amountIn;

        Ok(AlloyU256ToU256Converter::convert(amount_in))
    }
}
