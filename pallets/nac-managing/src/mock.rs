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
    traits::{AsEnsureOriginWithArg, ConstU32, ConstU64},
    weights::constants::RocksDbWeight,
};
use frame_system::{EnsureRoot, EnsureSigned};
use parity_scale_codec::Compact;
use sp_core::H256;
use sp_runtime::traits::BlakeTwo256;
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
    pub TestCollectionDeposit: u64 = 0;
    pub TestItemDeposit: u64 = 0;
    pub static FailAttributeWrite: Option<(u32, Vec<u8>)> = None;
    pub static FailVippCallback: bool = false;
}

pub type CollectionId = u32;
pub type ItemId = u32;

// `()` implements the nfts helper only for `MultiSigner` and `MultiSignature`, not for the test types.
// Compiled when `pallet-nfts` has `runtime-benchmarks`, which another crate in the build can switch on.
pallet_nfts::runtime_benchmarks_enabled! {
    pub struct NftsBenchmarkHelper;

    impl pallet_nfts::BenchmarkHelper<CollectionId, ItemId, AccountPublic, AccountId, Signature>
        for NftsBenchmarkHelper
    {
        fn collection(i: u16) -> CollectionId {
            i.into()
        }

        fn item(i: u16) -> ItemId {
            i.into()
        }

        fn signer() -> (AccountPublic, AccountId) {
            (UintAuthorityId(1), 1)
        }

        fn sign(signer: &AccountPublic, message: &[u8]) -> Signature {
            TestSignature(signer.0, message.to_vec())
        }
    }
}

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
    pallet_nfts::runtime_benchmarks_enabled! {
        type Helper = NftsBenchmarkHelper;
    }
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
    type MaxVippNftsPerAccount = ConstU32<64>;
}

pub struct TestVippCallback;

impl crate::OnVippStatusHandler<AccountId, Balance, ItemId> for TestVippCallback {
    fn mint_vipp(
        _who: &AccountId,
        _amount: Balance,
        _item_id: ItemId,
    ) -> sp_runtime::DispatchResult {
        if FailVippCallback::get() {
            return Err(sp_runtime::DispatchError::Other("injected VIPP callback failure"));
        }
        Ok(())
    }

    fn burn_vipp_nft(_who: &AccountId, _item_id: ItemId) {}
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
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxLocks = ConstU32<10>;
    type MaxReserves = ConstU32<10>;
    type MaxFreezes = ();
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
    FailAttributeWrite::set(None);
    FailVippCallback::set(false);
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
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
