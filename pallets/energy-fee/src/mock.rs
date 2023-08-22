use crate as pallet_energy_fee;
use crate::CustomFee;
use frame_support::weights::{ConstantMultiplier, IdentityFee, WeightToFee};
use frame_support::{
    parameter_types,
    traits::{AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, Everything},
};
use frame_system::{EnsureRoot, EnsureSigned};
use parity_scale_codec::Compact;

use sp_core::H256;

use sp_runtime::{
    traits::{BlakeTwo256, DispatchInfoOf, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

/// The AccountId alias in this test module.
pub(crate) type AccountId = u64;
pub(crate) type AssetId = u128;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u128;

pub(crate) const VNRG: AssetId = 1;
pub(crate) const ALICE: AccountId = 1;
pub(crate) const BOB: AccountId = 2;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        TransactionPayment: pallet_transaction_payment,
        EnergyFee: pallet_energy_fee,
    }
);

parameter_types! {
    pub const GetVNRG: AssetId = VNRG;
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

impl pallet_energy_fee::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balanced = Assets;
    type GetEnergyAssetId = GetVNRG;
    type GetConstantEnergyFee = ConstU128<1_000_000_000>;
    type CustomFee = EnergyFee;
}

// We implement CusomFee here since the RuntimeCall defined in construct_runtime! macro
impl CustomFee<RuntimeCall, DispatchInfoOf<RuntimeCall>, Balance> for EnergyFee {
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
    ) -> Option<Balance> {
        match runtime_call {
            RuntimeCall::Balances(..) | RuntimeCall::Assets(..) => {
                Some(<Self as WeightToFee>::weight_to_fee(&dispatch_info.weight))
            },
            _ => None,
        }
    }
}

parameter_types! {
    pub const AssetDeposit: Balance = 0;
    pub const AssetAccountDeposit: Balance = 0;
    pub const ApprovalDeposit: Balance = 0;
    pub const AssetsStringLimit: u32 = 50;
    pub const MetadataDepositBase: Balance = 0;
    pub const MetadataDepositPerByte: Balance = 0;
}

impl pallet_assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type AssetId = AssetId;
    type Currency = Balances;
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
    type FeeMultiplierUpdate = ();
}
// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_assets::GenesisConfig::<Test> {
        assets: vec![(VNRG, ALICE, true, 1)],
        metadata: vec![(VNRG, b"VNRG".to_vec(), b"VNRG".to_vec(), 18)],
        accounts: vec![(VNRG, ALICE, 1_000_000_000_000)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
