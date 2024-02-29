use crate as pallet_dex;
use ethereum_types::H160;
use fp_account::AccountId20;
use fp_evm::GenesisAccount;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::fungible::ItemOf;
use frame_support::weights::{ConstantMultiplier, IdentityFee};
use frame_support::{
    pallet_prelude::Weight,
    parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, Everything},
};
use frame_system::{EnsureRoot, EnsureSigned};
use pallet_ethereum::PostLogContent;
use pallet_evm::{EnsureAccountId20, IdentityAddressMapping, OnChargeEVMTransaction};
use pallet_evm_precompileset_assets_erc20::AccountIdAssetIdConversion;
use parity_scale_codec::{Compact, Encode};
use sp_std::collections::btree_map::BTreeMap;

use sp_arithmetic::{FixedPointNumber, FixedU128, Perbill, Perquintill};
use sp_core::{H256, U256};

use sp_runtime::{
    traits::{BlakeTwo256, DispatchInfoOf, IdentityLookup, Zero},
    BuildStorage, Permill,
};

use super::precompiles::{LOCAL_ASSET_PRECOMPILE_ADDRESS_PREFIX, VitreusPrecompiles, BalancesPrecompileAddress};

type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) type AccountId = AccountId20;
pub(crate) type AssetId = u32;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u128;
pub(crate) type BalancesVNRG = ItemOf<Assets, GetVNRG, AccountId>;

pub(crate) const VNRG: AssetId = 1;
pub(crate) const ALICE: AccountId = AccountId20([1u8; 20]);
pub(crate) const BOB: AccountId = AccountId20([2u8; 20]);
pub(crate) static CONTRACT: AccountId = H160::from_low_u64_be(4096).into();

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
        EVMChainId: pallet_evm_chain_id,
        Ethereum: pallet_ethereum,
        EVM: pallet_evm,
        BaseFee: pallet_base_fee,
        Dex: pallet_dex,
        Timestamp: pallet_timestamp,
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
    pub const GetPrecompilesValue: VitreusPrecompiles<Test> = VitreusPrecompiles::<Test>::new();
    pub const GetConstantEnergyFee: Balance = 1_000_000_000;
}

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

pub struct ZeroEVMFee;

impl OnChargeEVMTransaction<Test> for ZeroEVMFee {
    type LiquidityInfo = ();

    fn withdraw_fee(
        _who: &H160,
        _fee: U256,
    ) -> Result<Self::LiquidityInfo, pallet_evm::Error<Test>> {
        Ok(())
    }

    fn correct_and_deposit_fee(
        _who: &H160,
        _corrected_fee: U256,
        _base_fee: U256,
        _already_withdrawn: Self::LiquidityInfo,
    ) -> Self::LiquidityInfo {
        ()
    }

    fn pay_priority_fee(_tip: Self::LiquidityInfo) {}
}

// Instruct how to go from an H160 to an AssetID
// We just take the lowest 128 bits
impl AccountIdAssetIdConversion<AccountId, AssetId> for Test {
	/// The way to convert an account to assetId is by ensuring that the prefix is 0XFFFFFFFF
	/// and by taking the lowest 128 bits as the assetId
	fn account_to_asset_id(account: AccountId) -> Option<(Vec<u8>, AssetId)> {
		let h160_account: H160 = account.into();
		let mut data = [0u8; 16];
		let (prefix_part, id_part) = h160_account.as_fixed_bytes().split_at(4);
		if prefix_part == LOCAL_ASSET_PRECOMPILE_ADDRESS_PREFIX
		{
			data.copy_from_slice(id_part);
			let asset_id: AssetId = u128::from_be_bytes(data).into();
			Some((prefix_part.to_vec(), asset_id))
		} else {
			None
		}
	}

	// The opposite conversion
	fn asset_id_to_account(prefix: &[u8], asset_id: AssetId) -> AccountId {
		let mut data = [0u8; 20];
		data[0..4].copy_from_slice(prefix);
		data[4..20].copy_from_slice(&asset_id.to_be_bytes());
		AccountId::from(data)
	}
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
    type OnChargeTransaction = ZeroEVMFee;
    type PrecompilesType = VitreusPrecompiles<Test>;
    type PrecompilesValue = GetPrecompilesValue;
    type WeightInfo = pallet_evm::weights::SubstrateWeight<Test>;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<1000>;
    type WeightInfo = ();
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
    pub const EnergyBrokerPalletId: PalletId = PalletId(*b"py/brokr"); 
    pub CurrencyLabel: Vec<u8> = b"VTRS".to_vec();
}

impl pallet_dex::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = EnergyBrokerPalletId;
    type BalancesPrecompileAddress = BalancesPrecompileAddress;
    type CurrencyLabel = CurrencyLabel;
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
        accounts: vec![(GetVNRG::get(), BOB, 1000)].into_iter().chain(alice_account).collect(),
        assets: vec![(GetVNRG::get(), BOB, false, 1)],
        metadata: vec![(GetVNRG::get(), b"VNRG".to_vec(), b"VNRG".to_vec(), 18)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_evm::GenesisConfig::<Test> {
        accounts: BTreeMap::from_iter(
            vec![(
                CONTRACT.into(),
                GenesisAccount {
                    nonce: 0.into(),
                    balance: 0.into(),
                    storage: Default::default(),
                    code: include_bytes!("../../../node/chain-spec/nft-descriptor.bin").to_vec(),
                },
            )]
            .into_iter(),
        ),
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_dex::GenesisConfig::<Test> {
        factory_bytecode: include_bytes!("../../../node/chain-spec/v3factory.bin").to_vec(),
        router_bytecode: include_bytes!("../../../node/chain-spec/v3router.bin").to_vec(),
        position_manager_bytecode: include_bytes!("../../../node/chain-spec/v3position-manager.bin").to_vec(),
        position_descriptor_bytecode: include_bytes!("../../../node/chain-spec/v3position-descriptor.bin").to_vec(),
        quoter_bytecode: include_bytes!("../../../node/chain-spec/v3quoter.bin").to_vec(),
    }
    .assimilate_storage(&mut t)
    .unwrap();


    t.into()
}
