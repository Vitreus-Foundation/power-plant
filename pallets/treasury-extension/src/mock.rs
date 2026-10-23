use crate as pallet_treasury_extension;

use frame_support::traits::tokens::{
    Fortitude, PayFromAccount, Precision, Preservation, UnityAssetBalanceConversion,
};
use frame_support::PalletId;
use frame_support::{
    derive_impl, parameter_types,
    traits::{
        fungible::{ItemOf, Mutate as _},
        fungibles::Mutate as _,
        AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, Everything,
    },
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess, EnsureSigned};
use pallet_treasury::TreasuryAccountId;
use sp_core::H256;

use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, DispatchError, Permill, TokenError,
};
use vitreus_runtime_common::{QuotePriceFromEnergy, SwapFromEnergy};

// 2 gVTRS
const VTRS_INITIAL_BALANCE: u128 = 2_000_000_000_000_000_000_000_000_000;

type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) type AccountId = u64;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u128;
pub(crate) type BlockNumber = u64;

pub(crate) const ALICE: AccountId = 1;
pub(crate) const BOB: AccountId = 2;

pub(crate) const TREASURY: u64 = 1;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        Bounties: pallet_bounties,
        Treasury: pallet_treasury,
        TreasuryExtension: pallet_treasury_extension,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
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

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
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
    type RuntimeHoldReason = ();
}

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
impl pallet_assets::Config for Test {
    type Balance = Balance;
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type AssetDeposit = ConstU128<1>;
    type AssetAccountDeposit = ConstU128<10>;
    type MetadataDepositBase = ConstU128<1>;
    type MetadataDepositPerByte = ConstU128<1>;
    type ApprovalDeposit = ConstU128<1>;
    type Freezer = ();
}

parameter_types! {
    pub const SpendPeriod: BlockNumber = 10;
    pub const PayoutPeriod: BlockNumber = 5;
    pub const Burn: Permill = Permill::from_percent(1);
    pub const TreasuryPalletId: PalletId = PalletId(TREASURY.to_le_bytes());

    pub const DataDepositPerByte: Balance = 1;
    pub const MaxApprovals: u32 = 100;
    pub const MaxAuthorities: u32 = 100_000;
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
    pub const RootSpendOriginMaxAmount: Balance = Balance::MAX;
    pub const CouncilSpendOriginMaxAmount: Balance = Balance::MAX;
}

impl pallet_treasury::Config for Test {
    type PalletId = TreasuryPalletId;
    type Currency = Balances;
    type RejectOrigin = EnsureRoot<AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type SpendPeriod = SpendPeriod;
    type Burn = Burn;
    type BurnDestination = ();
    type SpendFunds = (Bounties, TreasuryExtension);
    type MaxApprovals = MaxApprovals;
    type SpendOrigin = EnsureRootWithSuccess<AccountId, ConstU128<{ Balance::MAX }>>;
    type AssetKind = ();
    type Beneficiary = AccountId;
    type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
    type Paymaster = PayFromAccount<Balances, TreasuryAccountId<Test>>;
    type BalanceConverter = UnityAssetBalanceConversion;
    type PayoutPeriod = PayoutPeriod;
    type WeightInfo = ();
}

parameter_types! {
    pub const BountyDepositBase: Balance = 10;
    pub const BountyDepositPayoutDelay: BlockNumber = 8;
    pub const BountyUpdatePeriod: BlockNumber = 90;
    pub const MaximumReasonLength: u32 = 16384;
    pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
    pub const CuratorDepositMin: Balance = 100_000;
    pub const CuratorDepositMax: Balance = 100_000_000;
    pub const BountyValueMinimum: Balance = 100;
}

impl pallet_bounties::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type BountyDepositBase = BountyDepositBase;
    type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
    type BountyUpdatePeriod = BountyUpdatePeriod;
    type CuratorDepositMultiplier = CuratorDepositMultiplier;
    type CuratorDepositMin = CuratorDepositMin;
    type CuratorDepositMax = CuratorDepositMax;
    type BountyValueMinimum = BountyValueMinimum;
    type ChildBountyManager = ();
    type DataDepositPerByte = DataDepositPerByte;
    type MaximumReasonLength = MaximumReasonLength;
    type OnSlash = ();
    type WeightInfo = ();
}

pub(crate) const ENERGY: u32 = 0;
pub(crate) const STATIC_ENERGY: u32 = 1;
pub(crate) const LIQUID_ENERGY: u32 = 2;

/// The minimum balance of each energy asset.
pub(crate) const ENERGY_MIN_BALANCE: Balance = 1;

/// Native currency the mock exchanges pay per unit of energy.
pub(crate) const EXCHANGE_RATE: Balance = 2;

parameter_types! {
    pub const SpendThreshold: Permill = Permill::from_percent(50);
    /// Native currency the mock exchanges can still pay out, all together.
    pub storage ExchangeLiquidity: Balance = Balance::MAX;
    /// The energy asset whose exchange fails every swap, if any.
    pub storage FailingExchange: Option<u32> = None;
    /// The energy asset whose exchange has no price for it and quotes nothing, if any.
    pub storage UnpricedExchange: Option<u32> = None;
}

/// Sells the energy asset `ID` for native currency at `EXCHANGE_RATE`, minting what it pays out of
/// `ExchangeLiquidity`. It fails every swap while `FailingExchange` names the asset, and quotes
/// nothing while `UnpricedExchange` does.
pub struct MockExchange<const ID: u32>;

impl<const ID: u32> QuotePriceFromEnergy for MockExchange<ID> {
    type Balance = Balance;
    type EnergyAsset = ItemOf<Assets, ConstU32<ID>, AccountId>;

    fn quote_price_exact_tokens_for_tokens(amount: Balance, _: bool) -> Option<Balance> {
        if UnpricedExchange::get() == Some(ID) {
            return None;
        }
        amount.checked_mul(EXCHANGE_RATE)
    }

    fn quote_price_tokens_for_exact_tokens(amount: Balance, _: bool) -> Option<Balance> {
        if UnpricedExchange::get() == Some(ID) {
            return None;
        }
        Some(amount / EXCHANGE_RATE)
    }

    fn max_amount_out() -> Balance {
        ExchangeLiquidity::get()
    }
}

impl<const ID: u32> SwapFromEnergy<AccountId> for MockExchange<ID> {
    type Balance = Balance;
    type EnergyAsset = ItemOf<Assets, ConstU32<ID>, AccountId>;

    fn swap_exact_tokens_for_tokens(
        who: AccountId,
        amount_in: Balance,
        keep_alive: bool,
    ) -> Result<Balance, DispatchError> {
        if FailingExchange::get() == Some(ID) {
            return Err(TokenError::FundsUnavailable.into());
        }

        // Like a real exchange, a swap for more than it can pay fails as a whole.
        let amount_out = amount_in * EXCHANGE_RATE;
        let liquidity_left = ExchangeLiquidity::get()
            .checked_sub(amount_out)
            .ok_or(TokenError::FundsUnavailable)?;

        let preservation =
            if keep_alive { Preservation::Preserve } else { Preservation::Expendable };
        Assets::burn_from(ID, &who, amount_in, preservation, Precision::Exact, Fortitude::Polite)?;

        ExchangeLiquidity::set(&liquidity_left);
        Balances::mint_into(&who, amount_out)?;
        Ok(amount_out)
    }

    fn swap_tokens_for_exact_tokens(
        _who: AccountId,
        _amount_out: Balance,
        _keep_alive: bool,
    ) -> Result<Balance, DispatchError> {
        Err(TokenError::Unsupported.into())
    }
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type EnergyAsset = ItemOf<Assets, ConstU32<ENERGY>, AccountId>;
    type StaticEnergyAsset = ItemOf<Assets, ConstU32<STATIC_ENERGY>, AccountId>;
    type LiquidEnergyAsset = ItemOf<Assets, ConstU32<LIQUID_ENERGY>, AccountId>;
    type EnergyExchange = MockExchange<ENERGY>;
    type StaticEnergyExchange = MockExchange<STATIC_ENERGY>;
    type LiquidEnergyExchange = MockExchange<LIQUID_ENERGY>;
    type SpendThreshold = SpendThreshold;
    type OnRecycled = ();
    type TreasuryTargetBalance = ();
    type FeeRecyclingBaseRate = ();
    type FeeRecyclingScalingFactor = ();
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![
            (ALICE, VTRS_INITIAL_BALANCE),
            (BOB, VTRS_INITIAL_BALANCE),
            (Treasury::account_id(), 1000),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_assets::GenesisConfig::<Test> {
        assets: vec![
            (ENERGY, ALICE, false, ENERGY_MIN_BALANCE),
            (STATIC_ENERGY, ALICE, false, ENERGY_MIN_BALANCE),
            (LIQUID_ENERGY, ALICE, false, ENERGY_MIN_BALANCE),
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
