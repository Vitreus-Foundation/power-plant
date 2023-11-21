use crate as pallet_energy_fee;
use crate::traits::{AssetsBalancesConverter, NativeExchange};
use crate::{CallFee, CustomFee};
use fp_account::AccountId20;

use frame_support::traits::fungible::ItemOf;
use frame_support::weights::{ConstantMultiplier, IdentityFee};
use frame_support::{
    pallet_prelude::Weight,
    parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, Everything},
};
use frame_system::{EnsureRoot, EnsureSigned};
use pallet_ethereum::PostLogContent;
use pallet_evm::{EnsureAccountId20, IdentityAddressMapping};
use parity_scale_codec::Compact;

use sp_arithmetic::FixedU128;
use sp_core::{H256, U256};

use sp_runtime::{
    traits::{BlakeTwo256, DispatchInfoOf, IdentityLookup, Zero},
    BuildStorage, Permill,
};

type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) type AccountId = AccountId20;
pub(crate) type AssetId = u128;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u128;
pub(crate) type BalancesVNRG = ItemOf<Assets, GetVNRG, AccountId>;
pub(crate) type EnergyRate = AssetsBalancesConverter<Test, AssetRate>;

pub(crate) const VNRG: AssetId = 1;
pub(crate) const ALICE: AccountId = AccountId20([1u8; 20]);
pub(crate) const BOB: AccountId = AccountId20([2u8; 20]);

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
        AssetRate: pallet_asset_rate,
        EVMChainId: pallet_evm_chain_id,
        Timestamp: pallet_timestamp,
        Ethereum: pallet_ethereum,
        EVM: pallet_evm,
        BaseFee: pallet_base_fee,
        Sudo: pallet_sudo,
    }
);

parameter_types! {
    pub const GetVNRG: AssetId = VNRG;
    pub const AssetDeposit: Balance = 0;
    pub const AssetAccountDeposit: Balance = 0;
    pub const ApprovalDeposit: Balance = 0;
    pub const AssetsStringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 0;
    pub const MetadataDepositPerByte: Balance = 0;
    pub BlockGasLimit: U256 = U256::from(75_000_000);
    pub const WeightPerGas: Weight = Weight::from_all(1_000_000);
    pub const GetPostLogContent: PostLogContent = PostLogContent::BlockAndTxnHashes;
    pub const GetPrecompilesValue: () = ();
    pub const GetConstantEnergyFee: Balance = 1_000_000_000;
}

impl frame_system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
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

impl pallet_balances::Config for Test {
    type MaxLocks = ConstU32<1024>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = ();
    type MaxFreezes = ();
    type MaxHolds = ();
    type RuntimeHoldReason = ();
}

impl pallet_asset_rate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CreateOrigin = EnsureRoot<AccountId>;
    type RemoveOrigin = EnsureRoot<AccountId>;
    type UpdateOrigin = EnsureRoot<AccountId>;
    type AssetId = AssetId;
    type Currency = BalancesVTRS;
    type Balance = Balance;
    type WeightInfo = ();
}

impl pallet_energy_fee::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManageOrigin = EnsureRoot<AccountId>;
    type GetConstantFee = GetConstantEnergyFee;
    type CustomFee = EnergyFee;
    type FeeTokenBalanced = BalancesVNRG;
    type MainTokenBalanced = BalancesVTRS;
    type EnergyExchange = NativeExchange<AssetId, BalancesVTRS, BalancesVNRG, EnergyRate, GetVNRG>;
    type EnergyAssetId = GetVNRG;
}

impl pallet_timestamp::Config for Test {
    type MinimumPeriod = ConstU64<1000>;
    type Moment = u64;
    type OnTimestampSet = ();
    type WeightInfo = ();
}

impl pallet_evm_chain_id::Config for Test {}

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
    type ChainId = EVMChainId;
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
}

impl CustomFee<RuntimeCall, DispatchInfoOf<RuntimeCall>, Balance, GetConstantEnergyFee>
    for EnergyFee
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        _dispatch_info: &DispatchInfoOf<RuntimeCall>,
    ) -> CallFee<Balance> {
        match runtime_call {
            RuntimeCall::BalancesVTRS(..) | RuntimeCall::Assets(..) => {
                CallFee::Custom(GetConstantEnergyFee::get())
            },
            RuntimeCall::EVM(..) => CallFee::EVM(GetConstantEnergyFee::get()),
            _ => CallFee::Stock,
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
    type FeeMultiplierUpdate = ();
}

impl pallet_sudo::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
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
        balances: vec![(ALICE, VTRS_INITIAL_BALANCE), (BOB, VTRS_INITIAL_BALANCE)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        accounts: vec![(GetVNRG::get(), BOB, 1000)]
            .into_iter()
            .chain(alice_account.into_iter())
            .collect(),
        assets: vec![(GetVNRG::get(), BOB, false, 1)],
        metadata: vec![(GetVNRG::get(), b"VNRG".to_vec(), b"VNRG".to_vec(), 18)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_energy_fee::GenesisConfig::<Test> {
        initial_energy_rate: VNRG_TO_VTRS_RATE,
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_sudo::GenesisConfig::<Test> { key: Some(ALICE) }
        .assimilate_storage(&mut t)
        .unwrap();

    t.into()
}
