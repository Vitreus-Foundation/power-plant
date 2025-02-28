use core::marker::PhantomData;

use crate::{self as pallet_energy_fee, FeeCreditOf};
use crate::{CallFee, CustomFee};
use fp_account::AccountId20;

use frame_support::{
    derive_impl,
    dispatch::GetDispatchInfo,
    pallet_prelude::Weight,
    parameter_types,
    traits::{
        fungible::{Balanced, ItemOf, Mutate},
        tokens::{Fortitude, Precision, Preservation},
        AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, Everything, OnUnbalanced,
    },
    weights::{ConstantMultiplier, IdentityFee},
};
use frame_system::{EnsureRoot, EnsureSigned};
use pallet_ethereum::PostLogContent;
use pallet_evm::{EnsureAccountId20, IdentityAddressMapping};
use parity_scale_codec::{Compact, Encode};

use sp_arithmetic::{FixedPointNumber, FixedU128, Perbill, Perquintill};
use sp_core::{Get, H256, U256};

use sp_runtime::{
    traits::{BlakeTwo256, DispatchInfoOf, IdentityLookup, Zero},
    BuildStorage, DispatchError, Permill,
};
use vitreus_runtime_common::{QuotePriceNativeForEnergy, SwapNativeForEnergy};

type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) type AccountId = AccountId20;
pub(crate) type AssetId = u32;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u128;
pub(crate) type BalancesVNRG = ItemOf<Assets, GetVNRG, AccountId>;
pub(crate) type StaticEnergyAsset = ItemOf<Assets, ConstU32<2>, AccountId>;
pub(crate) type LiquidEnergyAsset = ItemOf<Assets, ConstU32<3>, AccountId>;

pub(crate) const VNRG: AssetId = 1;
pub(crate) const ALICE: AccountId = AccountId20([1u8; 20]);
pub(crate) const BOB: AccountId = AccountId20([2u8; 20]);
pub(crate) const FEE_DEST: AccountId = AccountId20([3u8; 20]);

/// 10^9 with 18 decimals
/// 1 VNRG = VNRG_TO_VTRS_RATE VTRS
pub(crate) const VNRG_TO_VTRS_RATE: FixedU128 =
    FixedU128::from_inner(1_000_000_000_000_000_000_000_000_000);
pub(crate) const VTRS_INITIAL_BALANCE: u128 = 2_000_000_000_000_000_000_000_000_000;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        BalancesVTRS: pallet_balances,
        Assets: pallet_assets,
        TransactionPayment: pallet_transaction_payment,
        EnergyFee: pallet_energy_fee,
        Timestamp: pallet_timestamp,
        Ethereum: pallet_ethereum,
        EVM: pallet_evm,
        BaseFee: pallet_base_fee,
    }
);

const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(1_000_000_000, 1_000_000_u64);
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(100);

parameter_types! {
    pub const GetVNRG: AssetId = VNRG;
    pub const AssetDeposit: Balance = 0;
    pub const AssetAccountDeposit: Balance = 0;
    pub const ApprovalDeposit: Balance = 0;
    pub const AssetsStringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 0;
    pub const MetadataDepositPerByte: Balance = 0;
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights
        ::with_sensible_defaults(MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO);
    pub BlockGasLimit: U256 = U256::from(75_000_000);
    pub const WeightPerGas: Weight = Weight::from_all(1_000_000);
    pub const GetPostLogContent: PostLogContent = PostLogContent::BlockAndTxnHashes;
    pub const GetPrecompilesValue: () = ();
    pub const GetConstantEnergyFee: Balance = 1_000_000_000;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = BlockWeights;
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = Nonce;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = ConstU64<250>;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<1024>;
    type MaxReserves = ConstU32<1>;
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ConstU32<1>;
    type RuntimeHoldReason = ();
}

parameter_types! {
    pub const FeeBurnAccount: AccountId = FEE_DEST;
    pub const FeeRecyclingRate: Permill = Permill::from_percent(20);
}

pub struct FeeBurnDestination<GetAccountId: Get<AccountId>>(PhantomData<GetAccountId>);

impl<GetAccountId: Get<AccountId>> OnUnbalanced<FeeCreditOf<Test>>
    for FeeBurnDestination<GetAccountId>
{
    fn on_nonzero_unbalanced(amount: FeeCreditOf<Test>) {
        let account_id = GetAccountId::get();
        let _ = <BalancesVNRG as Balanced<AccountId>>::resolve(&account_id, amount);
    }
}

pub struct MockEnergyExchange;

impl QuotePriceNativeForEnergy for MockEnergyExchange {
    type Balance = u128;
    type AssetKind = ();
    type NativeAsset = ();
    type EnergyAsset = ();

    fn quote_price_exact_tokens_for_tokens(
        amount: Self::Balance,
        _include_fee: bool,
    ) -> Option<Self::Balance> {
        VNRG_TO_VTRS_RATE.reciprocal().map(|rate| rate.saturating_mul_int(amount))
    }

    fn quote_price_tokens_for_exact_tokens(
        amount: Self::Balance,
        _include_fee: bool,
    ) -> Option<Self::Balance> {
        Some(VNRG_TO_VTRS_RATE.saturating_mul_int(amount))
    }
}

impl SwapNativeForEnergy<AccountId> for MockEnergyExchange {
    type Balance = u128;
    type AssetKind = ();
    type NativeAsset = ();
    type EnergyAsset = ();

    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        let amount_out = Self::quote_price_exact_tokens_for_tokens(amount_in, true)
            .ok_or(DispatchError::Unavailable)?;

        BalancesVTRS::burn_from(
            &who,
            amount_in,
            Preservation::Preserve,
            Precision::Exact,
            Fortitude::Polite,
        )?;
        BalancesVNRG::mint_into(&who, amount_out)?;

        Ok(amount_out)
    }

    fn swap_tokens_for_exact_tokens(
        who: AccountId,
        amount_out: Self::Balance,
        _keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        let amount_in = Self::quote_price_tokens_for_exact_tokens(amount_out, true)
            .ok_or(DispatchError::Unavailable)?;

        BalancesVTRS::burn_from(
            &who,
            amount_in,
            Preservation::Preserve,
            Precision::Exact,
            Fortitude::Polite,
        )?;
        BalancesVNRG::mint_into(&who, amount_out)?;

        Ok(amount_out)
    }
}

impl pallet_energy_fee::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<AccountId>;
    type GetConstantFee = GetConstantEnergyFee;
    type CustomFee = EnergyFee;
    type EnergyAsset = BalancesVNRG;
    type StaticEnergyAsset = StaticEnergyAsset;
    type LiquidEnergyAsset = LiquidEnergyAsset;
    type EnergyExchange = MockEnergyExchange;
    type OnWithdrawFee = ();
    type OnEnergyBurn = ();
    type FeeRecyclingRate = FeeRecyclingRate;
    type FeeRecyclingDestination = FeeBurnDestination<FeeBurnAccount>;
}

impl pallet_timestamp::Config for Test {
    type MinimumPeriod = ConstU64<1000>;
    type Moment = u64;
    type OnTimestampSet = ();
    type WeightInfo = ();
}

impl pallet_ethereum::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
    type PostLogContent = GetPostLogContent;
    type ExtraDataLength = ConstU32<1000>;
}

pub struct BaseFeeThreshold;
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
    fn lower() -> Permill {
        Permill::zero()
    }
    fn ideal() -> Permill {
        Permill::from_parts(500_000)
    }
    fn upper() -> Permill {
        Permill::from_parts(1_000_000)
    }
}

parameter_types! {
    pub DefaultBaseFeePerGas: U256 = U256::from(1_000_000_000);
    pub DefaultElasticity: Permill = Permill::from_parts(125_000);
}

impl pallet_base_fee::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Threshold = BaseFeeThreshold;
    type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
    type DefaultElasticity = DefaultElasticity;
}

impl pallet_evm::Config for Test {
    type AddressMapping = IdentityAddressMapping;
    type BlockGasLimit = BlockGasLimit;
    type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
    type CallOrigin = EnsureAccountId20;
    type ChainId = ConstU64<42>;
    type Currency = BalancesVTRS;
    type Runner = pallet_evm::runner::stack::Runner<Self>;
    type RuntimeEvent = RuntimeEvent;
    type WeightPerGas = WeightPerGas;
    type WithdrawOrigin = EnsureAccountId20;
    type OnCreate = ();
    type Timestamp = Timestamp;
    type FeeCalculator = BaseFee;
    type FindAuthor = ();
    type GasLimitPovSizeRatio = ConstU64<1000>;
    type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
    type OnChargeTransaction = EnergyFee; //EVMCurrencyAdapter<Balances, ()>;
    type PrecompilesType = ();
    type PrecompilesValue = GetPrecompilesValue;
    type WeightInfo = pallet_evm::weights::SubstrateWeight<Test>;
    type SuicideQuickClearLimit = ();
}

impl CustomFee<RuntimeCall, DispatchInfoOf<RuntimeCall>, Balance, GetConstantEnergyFee>
    for EnergyFee
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfoOf<RuntimeCall>>,
        calculated_fee: Option<Balance>,
    ) -> CallFee<Balance> {
        match runtime_call {
            RuntimeCall::BalancesVTRS(..) | RuntimeCall::Assets(..) => {
                CallFee::Regular(Self::custom_fee())
            },
            RuntimeCall::EVM(..) => CallFee::EVM(Self::custom_fee()),
            _ => {
                let fee = Self::weight_fee(runtime_call, dispatch_info, calculated_fee);
                CallFee::Regular(fee)
            },
        }
    }

    fn custom_fee() -> Balance {
        let next_multiplier = TransactionPayment::next_fee_multiplier();
        next_multiplier.saturating_mul_int(EnergyFee::base_fee())
    }

    fn weight_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfoOf<RuntimeCall>>,
        calculated_fee: Option<Balance>,
    ) -> Balance {
        if let Some(fee) = calculated_fee {
            fee
        } else {
            let len = runtime_call.encode().len() as u32;
            if let Some(info) = dispatch_info {
                pallet_transaction_payment::Pallet::<Test>::compute_fee(len, info, Zero::zero())
            } else {
                let info = &runtime_call.get_dispatch_info();
                pallet_transaction_payment::Pallet::<Test>::compute_fee(len, info, Zero::zero())
            }
        }
    }
}

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type Currency = BalancesVTRS;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = AssetDeposit;
    type AssetAccountDeposit = AssetAccountDeposit;
    type MetadataDepositBase = MetadataDepositBase;
    type MetadataDepositPerByte = MetadataDepositPerByte;
    type ApprovalDeposit = ApprovalDeposit;
    type StringLimit = AssetsStringLimit;
    type Freezer = ();
    type Extra = ();
    type WeightInfo = ();
    type RemoveItemsLimit = ConstU32<1000>;
    type AssetIdParameter = Compact<AssetId>;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type CallbackHandle = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

parameter_types! {
    pub const TransactionByteFee: Balance = 1;
}

impl pallet_transaction_payment::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = EnergyFee;
    type OperationalFeeMultiplier = ();
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = EnergyFee;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext(energy_balance: Balance) -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    let alice_account = if !energy_balance.is_zero() {
        vec![(GetVNRG::get(), ALICE, energy_balance)]
    } else {
        vec![]
    };

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, VTRS_INITIAL_BALANCE),
            (BOB, VTRS_INITIAL_BALANCE),
            // required for account creation
            (FEE_DEST, 1),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        accounts: vec![(GetVNRG::get(), BOB, 1000)].into_iter().chain(alice_account).collect(),
        assets: vec![(GetVNRG::get(), BOB, false, 1)],
        metadata: vec![(GetVNRG::get(), b"VNRG".to_vec(), b"VNRG".to_vec(), 18)],
        next_asset_id: None,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

pub(crate) fn calculate_block_weight_based_on_threshold(threshold: Perquintill) -> Weight {
    let max_block_weight = <Test as frame_system::Config>::BlockWeights::get().max_block;
    let (ref_time, proof_size) = (
        threshold.mul_ceil(max_block_weight.ref_time()),
        threshold.mul_ceil(max_block_weight.proof_size()),
    );
    Weight::from_parts(ref_time, proof_size)
}
