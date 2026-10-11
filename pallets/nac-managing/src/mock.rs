// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Test environment for 'pallet-nac-managing'.

#![allow(missing_docs)]

use crate::{self as pallet_nac_managing, *};

use frame_support::{
    derive_impl, parameter_types,
    traits::{
        AsEnsureOriginWithArg, ConstU32, ConstU64, NamedReservableCurrency, StoredMap,
        VestingSchedule, WithdrawReasons,
    },
    weights::constants::RocksDbWeight,
};
use frame_system::{EnsureRoot, EnsureSigned};
use parity_scale_codec::Compact;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Identity};
use sp_runtime::{
    testing::{TestSignature, UintAuthorityId},
    traits::IdentityLookup,
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;

/// The AccountId alias in this test module.
pub(crate) type AccountId = u64;
pub(crate) type Nonce = u64;
pub(crate) type Balance = u64;
pub(crate) type Signature = TestSignature;
pub(crate) type AccountPublic = UintAuthorityId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        Nfts: pallet_nfts,
        Reputation: pallet_reputation,
        NacManaging: pallet_nac_managing,
        Vesting: pallet_vesting,
        SimpleVesting: pallet_simple_vesting,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type BaseCallFilter = frame_support::traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = Nonce;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type BlockHashCount = ConstU64<250>;
    type DbWeight = RocksDbWeight;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = NacManaging;
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

parameter_types! {
    pub static TestCollectionDeposit: u64 = 0;
    pub static TestItemDeposit: u64 = 0;
    pub static FailAttributeWrite: Option<(u32, Vec<u8>)> = None;
    pub static FailVippCallback: bool = false;
    pub static FailVippCallbackOnItem: Option<ItemId> = None;
    pub static ObserveVippBalances: bool = false;
    pub static VippCallbackCalls: u32 = 0;
    pub static QueueDuringVippCallback: Option<AccountId> = None;
}

pub type CollectionId = u32;
pub type ItemId = u32;

impl pallet_nfts::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type Currency = Balances;
    type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type CollectionDeposit = TestCollectionDeposit;
    type ApprovalsLimit = ();
    type ItemAttributesApprovalsLimit = ();
    type MaxTips = ();
    type MaxDeadlineDuration = ();
    type MaxAttributesPerCall = ();
    type Features = ();
    type OffchainPublic = AccountPublic;
    type OffchainSignature = Signature;
    type ItemDeposit = TestItemDeposit;
    type MetadataDepositBase = ConstU64<1>;
    type AttributeDepositBase = ConstU64<1>;
    type DepositPerByte = ConstU64<1>;
    type StringLimit = ConstU32<50>;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type Helper = TestNftBenchmarkHelper;
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<Self::AccountId>>;
    type Locker = ();
}

parameter_types! {
    pub const NftCollectionId: CollectionId = 0;
    pub const VIPPCollectionId: CollectionId = 1;
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type Nfts = TestNfts;
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    type NftCollectionId = NftCollectionId;
    type KeyLimit = ConstU32<50>;
    type ValueLimit = ConstU32<50>;
    type WeightInfo = ();
    type Currency = Balances;
    type VIPPCollectionId = VIPPCollectionId;
    type OnVIPPChanged = TestVippCallback;
    type UnvestedBalance = TestUnvestedBalance;
    type MaxVippNftsPerAccount = ConstU32<64>;
    type MaxPendingVippAccounts = ConstU32<128>;
    type MaxVippReconciliationsPerBlock = ConstU32<2>;
}

pub struct TestUnvestedBalance;

impl UnvestedBalance<AccountId, Balance> for TestUnvestedBalance {
    fn unvested_balance(who: &AccountId) -> Balance {
        let standard = Vesting::vesting_balance(who).unwrap_or_default();
        let reserved = SimpleVesting::vesting(who)
            .map(|schedule| schedule.locked_at::<Identity>(System::block_number()))
            .unwrap_or_default()
            .min(Balances::reserved_balance_named(b"vesting ", who))
            .min(Balances::reserved_balance(who));
        standard.saturating_add(reserved)
    }
}

parameter_types! {
    pub const MinVestedTransfer: Balance = 1;
    pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
        WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

impl pallet_vesting::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = Identity;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = ();
    type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
    type BlockNumberProvider = System;
    const MAX_VESTING_SCHEDULES: u32 = 28;
}

impl pallet_simple_vesting::Config for Test {
    type NativeCurrencyObserverWeight = ();
    type OnVestingChanged = TestBalanceObserver;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type BlockNumberToBalance = Identity;
    type Slash = ();
}

pub struct TestVippCallback;

impl crate::OnVippStatusHandler<AccountId, Balance, ItemId> for TestVippCallback {
    fn can_receive_vipp(_who: &AccountId) -> bool {
        true
    }
    fn mint_vipp(
        _who: &AccountId,
        _amount: Balance,
        _item_id: ItemId,
    ) -> sp_runtime::DispatchResult {
        VippCallbackCalls::set(VippCallbackCalls::get().saturating_add(1));
        if FailVippCallback::get() || FailVippCallbackOnItem::get() == Some(_item_id) {
            return Err(sp_runtime::DispatchError::Other("injected VIPP callback failure"));
        }
        if let Some(account) = QueueDuringVippCallback::get() {
            QueueDuringVippCallback::set(None);
            NacManaging::check_observed_balance(&account);
        }
        Ok(())
    }

    fn burn_vipp_nft(_who: &AccountId, _item_id: ItemId) -> sp_runtime::DispatchResult {
        Self::mint_vipp(_who, 0, _item_id)
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

pub type AssetId = u32;

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

impl pallet_reputation::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU64<1>;
    type AccountStore = TestBalanceObserver;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxLocks = ConstU32<10>;
    type MaxReserves = ConstU32<10>;
    type MaxFreezes = ();
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    TestCollectionDeposit::set(0);
    TestItemDeposit::set(0);
    FailAttributeWrite::set(None);
    FailVippCallback::set(false);
    FailVippCallbackOnItem::set(None);
    ObserveVippBalances::set(false);
    VippCallbackCalls::set(0);
    QueueDuringVippCallback::set(None);
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

pub struct TestBalanceObserver;

impl pallet_simple_vesting::OnVestingChanged<AccountId> for TestBalanceObserver {
    fn on_vesting_changed(who: &AccountId) {
        if ObserveVippBalances::get() {
            NacManaging::check_observed_balance(who);
        }
    }
}

impl StoredMap<AccountId, pallet_balances::AccountData<Balance>> for TestBalanceObserver {
    fn get(who: &AccountId) -> pallet_balances::AccountData<Balance> {
        <System as StoredMap<AccountId, pallet_balances::AccountData<Balance>>>::get(who)
    }

    fn try_mutate_exists<R, E: From<DispatchError>>(
        who: &AccountId,
        mutate: impl FnOnce(&mut Option<pallet_balances::AccountData<Balance>>) -> Result<R, E>,
    ) -> Result<R, E> {
        if ObserveVippBalances::get() {
            <crate::balance_observer::VippBalanceObserver<Test, System> as StoredMap<
                AccountId,
                pallet_balances::AccountData<Balance>,
            >>::try_mutate_exists(who, mutate)
        } else {
            <System as StoredMap<AccountId, pallet_balances::AccountData<Balance>>>::try_mutate_exists(who, mutate)
        }
    }
}

// Delegate to the real NFT pallet, with failure injection only at the attribute
// write boundary. Reads and storage decoding remain identical to production.
pub struct TestNfts;

impl Inspect<AccountId> for TestNfts {
    type CollectionId = CollectionId;
    type ItemId = ItemId;
    fn owner(collection: &CollectionId, item: &ItemId) -> Option<AccountId> {
        Nfts::owner(*collection, *item)
    }
    fn system_attribute(
        collection: &CollectionId,
        item: Option<&ItemId>,
        key: &[u8],
    ) -> Option<Vec<u8>> {
        Nfts::system_attribute(collection, item, key)
    }
}

impl InspectEnumerable<AccountId> for TestNfts {
    type CollectionsIterator = <Nfts as InspectEnumerable<AccountId>>::CollectionsIterator;
    type ItemsIterator = <Nfts as InspectEnumerable<AccountId>>::ItemsIterator;
    type OwnedIterator = <Nfts as InspectEnumerable<AccountId>>::OwnedIterator;
    type OwnedInCollectionIterator =
        <Nfts as InspectEnumerable<AccountId>>::OwnedInCollectionIterator;
    fn collections() -> Self::CollectionsIterator {
        Nfts::collections()
    }
    fn items(collection: &CollectionId) -> Self::ItemsIterator {
        Nfts::items(collection)
    }
    fn owned(who: &AccountId) -> Self::OwnedIterator {
        Nfts::owned(who)
    }
    fn owned_in_collection(
        collection: &CollectionId,
        who: &AccountId,
    ) -> Self::OwnedInCollectionIterator {
        Nfts::owned_in_collection(collection, who)
    }
}

impl Create<AccountId, CollectionConfigFor<Test>> for TestNfts {
    fn create_collection(
        who: &AccountId,
        admin: &AccountId,
        config: &CollectionConfigFor<Test>,
    ) -> Result<CollectionId, DispatchError> {
        <Nfts as Create<AccountId, CollectionConfigFor<Test>>>::create_collection(
            who, admin, config,
        )
    }
    fn create_collection_with_id(
        collection: CollectionId,
        who: &AccountId,
        admin: &AccountId,
        config: &CollectionConfigFor<Test>,
    ) -> DispatchResult {
        <Nfts as Create<AccountId, CollectionConfigFor<Test>>>::create_collection_with_id(
            collection, who, admin, config,
        )
    }
}

impl Mutate<AccountId, ItemConfig> for TestNfts {
    fn mint_into(
        collection: &CollectionId,
        item: &ItemId,
        who: &AccountId,
        config: &ItemConfig,
        deposit_collection_owner: bool,
    ) -> DispatchResult {
        <Nfts as Mutate<AccountId, ItemConfig>>::mint_into(
            collection,
            item,
            who,
            config,
            deposit_collection_owner,
        )
    }
    fn burn(collection: &CollectionId, item: &ItemId, owner: Option<&AccountId>) -> DispatchResult {
        <Nfts as Mutate<AccountId, ItemConfig>>::burn(collection, item, owner)
    }
    fn set_attribute(
        collection: &CollectionId,
        item: &ItemId,
        key: &[u8],
        value: &[u8],
    ) -> DispatchResult {
        if FailAttributeWrite::get() == Some((*collection, key.to_vec())) {
            return Err(DispatchError::Other("injected NFT attribute failure"));
        }
        <Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(collection, item, key, value)
    }
}

impl Transfer<AccountId> for TestNfts {
    fn transfer(
        collection: &CollectionId,
        item: &ItemId,
        destination: &AccountId,
    ) -> DispatchResult {
        <Nfts as Transfer<AccountId>>::transfer(collection, item, destination)
    }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct TestNftBenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl pallet_nfts::BenchmarkHelper<u32, u32, UintAuthorityId, u64, TestSignature>
    for TestNftBenchmarkHelper
{
    fn collection(i: u16) -> u32 {
        i.into()
    }
    fn item(i: u16) -> u32 {
        i.into()
    }
    fn signer() -> (UintAuthorityId, u64) {
        (UintAuthorityId(9000), 9000)
    }
    fn sign(signer: &UintAuthorityId, message: &[u8]) -> TestSignature {
        TestSignature(signer.0, message.to_vec())
    }
}
