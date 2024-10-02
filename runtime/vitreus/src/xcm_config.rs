// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! XCM configuration for Vitreus.

#![allow(clippy::match_like_matches_macro, clippy::type_complexity)]

use super::{
    parachains_origin, AccountId, AllPalletsWithSystem, Balance, Balances, CouncilCollective, Dmp,
    ParaId, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, TransactionByteFee,
    TransactionPicosecondFee, Treasury, XcmPallet,
};
use frame_support::weights::ConstantMultiplier;
use frame_support::{
    match_types, parameter_types,
    traits::{fungible::Credit, Contains, Everything, IsInVec, Nothing, OnUnbalanced},
    weights::Weight,
};
use frame_system::EnsureRoot;
use origin_conversion::SignedToAccountKey20;
use runtime_common::{
    paras_registrar, prod_or_fast,
    xcm_sender::{ChildParachainRouter, ExponentialPrice},
};
use scale_info::prelude::sync::Arc;
use sp_core::ConstU32;
use sp_runtime::traits::TryConvertInto;
use xcm::latest::prelude::*;
use xcm::opaque::{
    v4::AssetId,
    v4::Junctions::{X1, X2},
    v4::{InteriorLocation, Junction},
};
use xcm::v4::Junctions::Here;
use xcm_builder::{
    AccountKey20Aliases, AllowExplicitUnpaidExecutionFrom, AllowKnownQueryResponses,
    AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom, AsPrefixedGeneralIndex,
    BackingToPlurality, ChildParachainAsNative, ChildParachainConvertsVia,
    CurrencyAdapter as XcmCurrencyAdapter, FixedWeightBounds, FrameTransactionalProcessor,
    FungiblesAdapter, IsChildSystemParachain, IsConcrete, MatchedConvertedConcreteId, MintLocation,
    NoChecking, SignedAccountKey20AsNative, SovereignSignedViaLocation, TakeWeightCredit,
    TrailingSetTopicAsId, UsingComponents, WithComputedOrigin, WithUniqueTopic,
};
use xcm_executor::{
    traits::{TransactAsset, WeightTrader, WithOriginFilter},
    AssetsInHolding, XcmExecutor,
};

// TODO: use constants from `vitreus-runtime-constants` crate
const ASSET_HUB_ID: u32 = 1000;
const BRIDGE_HUB_ID: u32 = 1013;
const RELAY_NETWORK: NetworkId = prod_or_fast!(
    NetworkId::ByGenesis(hex_literal::hex!(
        "4f27ff2e1c714c78b718d11a999774b2f639da713b9481337942997140185cfc"
    )),
    NetworkId::ByGenesis(hex_literal::hex!(
        "c28caa6bf827d357af8ca58ddcefc2aba6d08b2fd29de8acb67becd4ea6c3673"
    ))
);
pub const ETHEREUM_NETWORK: NetworkId =
    prod_or_fast!(NetworkId::Ethereum { chain_id: 1 }, NetworkId::Ethereum { chain_id: 11155111 });
pub const ETHEREUM_VTRS_ADDRESS: [u8; 20] = prod_or_fast!(
    hex_literal::hex!("74950FC112473caba58193c6bF6412a6f1e4d7d2"),
    hex_literal::hex!("27C2E2131DF1310C9bdfAc779316685dB8B1E8bb")
);
pub const ASSETS_PALLET_ID: u8 = 5;
pub const VNRG_ASSET_ID: u128 = 1;

parameter_types! {
    pub const TokenLocation: Location = Here.into_location();
    pub const WrappedTokenLocation: Location = Here.into_location();
    pub const EnergyTokenLocation: Location = Here.into_location();
    pub const ThisNetwork: NetworkId = RELAY_NETWORK;
    pub UniversalLocation: InteriorLocation = ThisNetwork::get().into();
    pub CheckAccount: AccountId = XcmPallet::check_account();
    pub TreasuryAccount: AccountId = Treasury::account_id();
    pub LocalCheckAccount: (AccountId, MintLocation) = (CheckAccount::get(), MintLocation::Local);
}

pub type LocationConverter = (
    // We can convert a child parachain using the standard `AccountId` conversion.
    ChildParachainConvertsVia<ParaId, AccountId>,
    // We can directly alias an `AccountKey20` into a local account.
    AccountKey20Aliases<ThisNetwork, AccountId>,
);

/// Our asset transactor. This is what allows us to interest with the runtime facilities from the point of
/// view of XCM-only concepts like `MultiLocation` and `MultiAsset`.
///
/// Ours is only aware of the Balances pallet, which is mapped to `TokenLocation`.
pub type LocalAssetTransactor = XcmCurrencyAdapter<
    // Use this currency:
    Balances,
    // Use this currency when it is a fungible asset matching the given location or name:
    IsConcrete<TokenLocation>,
    // We can convert the MultiLocations with our converter above:
    LocationConverter,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We track our teleports in/out to keep total issuance correct.
    LocalCheckAccount,
>;

parameter_types! {
    pub const DefaultDepositFeePercent: u8 = 1;
    pub storage DepositFeePercent: u8 = DefaultDepositFeePercent::get();

    pub const DefaultWithdrawalFeePercent: u8 = 9;
    pub storage WithdrawalFeePercent: u8 = DefaultWithdrawalFeePercent::get();
}

/// Means for transacting the wrapped VTRS.
pub type WrappedTokenTransactor = currency_adapter::CurrencyAdapterWithFee<
    // Use this currency:
    Balances,
    // Use this currency when it is a fungible asset matching the given location or name:
    IsConcrete<WrappedTokenLocation>,
    // We can convert the MultiLocations with our converter above:
    LocationConverter,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We track our teleports in/out to keep total issuance correct.
    LocalCheckAccount,
    // Fee for wVTRS -> VTRS conversion
    DepositFeePercent,
    // Fee for VTRS -> wVTRS conversion
    WithdrawalFeePercent,
    // Send fee to the treasury
    TreasuryAccount,
>;

parameter_types! {
    pub AssetsPalletLocation: Location = PalletInstance(ASSETS_PALLET_ID).into();
    pub EnergyTokenLocationVec: sp_std::vec::Vec<Location> = [EnergyTokenLocation::get()].into();
}

pub type EnergyTokenConcreteId = MatchedConvertedConcreteId<
    super::AssetId,
    Balance,
    IsInVec<EnergyTokenLocationVec>,
    AsPrefixedGeneralIndex<AssetsPalletLocation, super::AssetId, TryConvertInto>,
    TryConvertInto,
>;

/// Means for transacting VNRG.
pub type EnergyTransactor = FungiblesAdapter<
    // Use this fungibles implementation:
    Assets,
    // Use this currency when it is a fungible asset matching the given location or name:
    EnergyTokenConcreteId,
    // Convert an XCM Location into a local account id:
    LocationConverter,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // Does not check teleports.
    NoChecking,
    // The account to use for tracking teleports.
    CheckAccount,
>;

/// The means that we convert an the XCM message origin location into a local dispatch origin.
type LocalOriginConverter = (
    // A `Signed` origin of the sovereign account that the original location controls.
    SovereignSignedViaLocation<LocationConverter, RuntimeOrigin>,
    // A child parachain, natively expressed, has the `Parachain` origin.
    ChildParachainAsNative<parachains_origin::Origin, RuntimeOrigin>,
    // The AccountKey20 location type can be expressed natively as a `Signed` origin.
    SignedAccountKey20AsNative<ThisNetwork, RuntimeOrigin>,
);

parameter_types! {
    /// The amount of weight an XCM operation takes. This is a safe overestimate.
    pub const BaseXcmWeight: Weight = Weight::from_parts(1_000_000_000, 64 * 1024);
    /// A temporary weight value for each XCM instruction.
    /// NOTE: This should be removed after we account for PoV weights.
    pub const FixedXcmWeight: Weight = Weight::from_parts(1_000_000_000, 0);
    /// Maximum number of instructions in a single XCM fragment. A sanity check against weight
    /// calculations getting too crazy.
    pub const MaxInstructions: u32 = 100;
    /// The asset ID for the asset that we use to pay for message delivery fees.
    pub FeeAssetId: AssetId = AssetId(TokenLocation::get());
    /// The base fee for the message delivery fees.
    pub const BaseDeliveryFee: u128 = 1_000_000_000;
}

pub type PriceForChildParachainDelivery =
    ExponentialPrice<FeeAssetId, BaseDeliveryFee, TransactionByteFee, Dmp>;

/// The XCM router. When we want to send an XCM message, we use this type. It amalgamates all of our
/// individual routers.
pub type XcmRouter = WithUniqueTopic<
    // Only one router so far - use DMP to communicate with child parachains.
    ChildParachainRouter<Runtime, XcmPallet, PriceForChildParachainDelivery>,
>;

parameter_types! {
    pub const Vtrs: AssetFilter = Wild(AllOf { fun: WildFungible, id: xcm::v4::AssetId(TokenLocation::get()) });
    pub const WrappedVtrs: AssetFilter = Wild(AllOf { fun: WildFungible, id: xcm::v4::AssetId(WrappedTokenLocation::get()) });
    pub AssetHub: Location = Parachain(ASSET_HUB_ID).into_location();
    pub BridgeHub: Location = Parachain(BRIDGE_HUB_ID).into_location();
    pub VtrsForAssetHub: (AssetFilter, Location) = (Vtrs::get(), AssetHub::get());
    pub VtrsForBridgeHub: (AssetFilter, Location) = (Vtrs::get(), BridgeHub::get());
    pub WrappedVtrsForAssetHub: (AssetFilter, Location) = (WrappedVtrs::get(), AssetHub::get());
    pub const MaxAssetsIntoHolding: u32 = 64;
}
pub type TrustedTeleporters = (
    xcm_builder::Case<VtrsForAssetHub>,
    xcm_builder::Case<VtrsForBridgeHub>,
    xcm_builder::Case<WrappedVtrsForAssetHub>,
);

match_types! {
    pub type OnlyParachains: impl Contains<Location> = {
        Location { parents: 0, interior: Here }
    };
}

/// The barriers one of which must be passed for an XCM message to be executed.
pub type Barrier = TrailingSetTopicAsId<(
    // Weight that is paid for may be consumed.
    TakeWeightCredit,
    // Expected responses are OK.
    AllowKnownQueryResponses<XcmPallet>,
    WithComputedOrigin<
        (
            // If the message is one that immediately attemps to pay for execution, then allow it.
            AllowTopLevelPaidExecutionFrom<Everything>,
            // Messages coming from system parachains need not pay for execution.
            AllowExplicitUnpaidExecutionFrom<IsChildSystemParachain<ParaId>>,
            // Subscriptions for version tracking are OK.
            AllowSubscriptionsFrom<OnlyParachains>,
        ),
        UniversalLocation,
        ConstU32<8>,
    >,
)>;

/// A call filter for the XCM Transact instruction. This is a temporary measure until we
/// properly account for proof size weights.
///
/// Calls that are allowed through this filter must:
/// 1. Have a fixed weight;
/// 2. Cannot lead to another call being made;
/// 3. Have a defined proof size weight, e.g. no unbounded vecs in call parameters.
pub struct SafeCallFilter;
impl Contains<RuntimeCall> for SafeCallFilter {
    fn contains(call: &RuntimeCall) -> bool {
        #[cfg(feature = "runtime-benchmarks")]
        {
            if matches!(call, RuntimeCall::System(frame_system::Call::remark_with_event { .. })) {
                return true;
            }
        }

        match call {
            RuntimeCall::System(
                frame_system::Call::kill_prefix { .. } | frame_system::Call::set_heap_pages { .. },
            )
            | RuntimeCall::Babe(..)
            | RuntimeCall::Timestamp(..)
            | RuntimeCall::Balances(..)
            | RuntimeCall::Session(pallet_session::Call::purge_keys { .. })
            | RuntimeCall::Grandpa(..)
            | RuntimeCall::ImOnline(..)
            | RuntimeCall::Democracy(
                pallet_democracy::Call::second { .. }
                | pallet_democracy::Call::vote { .. }
                | pallet_democracy::Call::emergency_cancel { .. }
                | pallet_democracy::Call::fast_track { .. }
                | pallet_democracy::Call::veto_external { .. }
                | pallet_democracy::Call::cancel_referendum { .. }
                | pallet_democracy::Call::delegate { .. }
                | pallet_democracy::Call::undelegate { .. }
                | pallet_democracy::Call::clear_public_proposals { .. }
                | pallet_democracy::Call::unlock { .. }
                | pallet_democracy::Call::remove_vote { .. }
                | pallet_democracy::Call::remove_other_vote { .. }
                | pallet_democracy::Call::blacklist { .. }
                | pallet_democracy::Call::cancel_proposal { .. },
            )
            | RuntimeCall::Council(
                pallet_collective::Call::vote { .. }
                | pallet_collective::Call::disapprove_proposal { .. }
                | pallet_collective::Call::close { .. },
            )
            | RuntimeCall::TechnicalCommittee(
                pallet_collective::Call::vote { .. }
                | pallet_collective::Call::disapprove_proposal { .. }
                | pallet_collective::Call::close { .. },
            )
            | RuntimeCall::TechnicalMembership(
                pallet_membership::Call::add_member { .. }
                | pallet_membership::Call::remove_member { .. }
                | pallet_membership::Call::swap_member { .. }
                | pallet_membership::Call::change_key { .. }
                | pallet_membership::Call::set_prime { .. }
                | pallet_membership::Call::clear_prime { .. },
            )
            | RuntimeCall::Treasury(..)
            | RuntimeCall::Utility(pallet_utility::Call::as_derivative { .. })
            | RuntimeCall::Vesting(..)
            | RuntimeCall::Bounties(
                pallet_bounties::Call::propose_bounty { .. }
                | pallet_bounties::Call::approve_bounty { .. }
                | pallet_bounties::Call::propose_curator { .. }
                | pallet_bounties::Call::unassign_curator { .. }
                | pallet_bounties::Call::accept_curator { .. }
                | pallet_bounties::Call::award_bounty { .. }
                | pallet_bounties::Call::claim_bounty { .. }
                | pallet_bounties::Call::close_bounty { .. },
            )
            | RuntimeCall::Hrmp(..)
            | RuntimeCall::Registrar(
                paras_registrar::Call::deregister { .. }
                | paras_registrar::Call::swap { .. }
                | paras_registrar::Call::remove_lock { .. }
                | paras_registrar::Call::reserve { .. }
                | paras_registrar::Call::add_lock { .. },
            )
            | RuntimeCall::XcmPallet(pallet_xcm::Call::limited_reserve_transfer_assets {
                ..
            }) => true,
            _ => false,
        }
    }
}

pub struct DummyWeightTrader;
impl WeightTrader for DummyWeightTrader {
    fn new() -> Self {
        DummyWeightTrader
    }

    fn buy_weight(
        &mut self,
        _weight: Weight,
        _payment: AssetsInHolding,
        _context: &XcmContext,
    ) -> Result<AssetsInHolding, XcmError> {
        Ok(AssetsInHolding::default())
    }
}

pub struct DummyAssetTransactor;
impl TransactAsset for DummyAssetTransactor {
    fn deposit_asset(_what: &Asset, _who: &Location, _context: Option<&XcmContext>) -> XcmResult {
        Ok(())
    }

    fn withdraw_asset(
        _what: &Asset,
        _who: &Location,
        _maybe_context: Option<&XcmContext>,
    ) -> Result<AssetsInHolding, XcmError> {
        let asset: Assets = (Parent, 100_000).into();
        Ok(asset.into())
    }
}

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    type AssetTransactor = DummyAssetTransactor;
    type OriginConverter = LocalOriginConverter;
    type IsReserve = ();
    type IsTeleporter = TrustedTeleporters;
    type UniversalLocation = UniversalLocation;
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<FixedXcmWeight, RuntimeCall, MaxInstructions>;
    type Trader = DummyWeightTrader;
    type ResponseHandler = XcmPallet;
    type AssetTrap = XcmPallet;
    type AssetLocker = ();
    type AssetExchanger = ();
    type AssetClaims = XcmPallet;
    type SubscriptionService = XcmPallet;
    type PalletInstancesInfo = AllPalletsWithSystem;
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type FeeManager = ();
    type MessageExporter = ();
    type UniversalAliases = Nothing;
    type CallDispatcher = WithOriginFilter<SafeCallFilter>;
    type SafeCallFilter = SafeCallFilter;
    type Aliasers = Nothing;
    type TransactionalProcessor = FrameTransactionalProcessor;
    type HrmpChannelAcceptedHandler = ();
    type HrmpChannelClosingHandler = ();
    type HrmpNewChannelOpenRequestHandler = ();
    type XcmRecorder = ();
}

parameter_types! {
    pub const CouncilBodyId: BodyId = BodyId::Executive;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
    pub ReachableDest: Option<MultiLocation> = Some(Parachain(1000).into());
}

/// Type to convert the council origin to a Plurality `MultiLocation` value.
pub type CouncilToPlurality = BackingToPlurality<
    RuntimeOrigin,
    pallet_collective::Origin<Runtime, CouncilCollective>,
    CouncilBodyId,
>;

/// Type to convert an `Origin` type value into a `MultiLocation` value which represents an interior location
/// of this chain.
pub type LocalOriginToLocation = (
    // We allow an origin from the Collective pallet to be used in XCM as a corresponding Plurality of the
    // `Unit` body.
    CouncilToPlurality,
    // And a usual Signed origin to be used in XCM as a corresponding AccountKey20
    SignedToAccountKey20<RuntimeOrigin, AccountId, ThisNetwork>,
);
impl pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmRouter = XcmRouter;
    // Anyone can execute XCM messages locally.
    type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmExecuteFilter = Everything;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Everything;
    // Anyone is able to use reserve transfers regardless of who they are and what they want to
    // transfer.
    type XcmReserveTransferFilter = Everything;
    type Weigher = FixedWeightBounds<BaseXcmWeight, RuntimeCall, MaxInstructions>;
    type UniversalLocation = UniversalLocation;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type Currency = Balances;
    type CurrencyMatcher = IsConcrete<TokenLocation>;
    type TrustedLockers = ();
    type SovereignAccountOf = LocationConverter;
    type MaxLockers = ConstU32<8>;
    type MaxRemoteLockConsumers = ConstU32<0>;
    type RemoteLockConsumerIdentifier = ();
    type WeightInfo = crate::weights::pallet_xcm::WeightInfo<Runtime>;
    #[cfg(feature = "runtime-benchmarks")]
    type ReachableDest = ReachableDest;
    type AdminOrigin = EnsureRoot<AccountId>;
}

mod origin_conversion {
    use super::*;
    use frame_support::traits::{Get, OriginTrait};
    use frame_system::RawOrigin;
    use sp_runtime::traits::TryConvert;
    use sp_std::marker::PhantomData;

    pub struct SignedToAccountKey20<RuntimeOrigin, AccountId, Network>(
        PhantomData<(RuntimeOrigin, AccountId, Network)>,
    );
    impl<
            RuntimeOrigin: OriginTrait + Clone,
            AccountId: Into<[u8; 20]>,
            Network: Get<Option<NetworkId>>,
        > TryConvert<RuntimeOrigin, Location>
        for SignedToAccountKey20<RuntimeOrigin, AccountId, Network>
    where
        RuntimeOrigin::PalletsOrigin: From<RawOrigin<AccountId>>
            + TryInto<RawOrigin<AccountId>, Error = RuntimeOrigin::PalletsOrigin>,
    {
        fn try_convert(o: RuntimeOrigin) -> Result<Location, RuntimeOrigin> {
            o.try_with_caller(|caller| match caller.try_into() {
                Ok(RawOrigin::Signed(who)) => {
                    Ok(Junction::AccountKey20 { network: Network::get(), key: who.into() }.into())
                },
                Ok(other) => Err(other.into()),
                Err(other) => Err(other),
            })
        }
    }
}

mod currency_adapter {
    use crate::{
        xcm_config::{Asset, AssetsInHolding, Location},
        Assets,
    };
    use frame_support::traits::ExistenceRequirement::KeepAlive;
    use frame_support::traits::{Get, WithdrawReasons};
    use sp_runtime::traits::CheckedSub;
    use sp_runtime::{Percent, Saturating};
    use sp_std::marker::PhantomData;
    use xcm::latest::{Error as XcmError, Result, XcmContext};
    use xcm_builder::{CurrencyAdapter, MintLocation};
    use xcm_executor::traits::{ConvertLocation, MatchesFungible, TransactAsset};

    /// Asset transaction errors.
    enum Error {
        /// The given asset is not handled. (According to [`XcmError::AssetNotFound`])
        AssetNotHandled,
        /// `MultiLocation` to `AccountId` conversion failed.
        AccountIdConversionFailed,
    }

    impl From<Error> for XcmError {
        fn from(e: Error) -> Self {
            use XcmError::FailedToTransactAsset;
            match e {
                Error::AssetNotHandled => XcmError::AssetNotFound,
                Error::AccountIdConversionFailed => {
                    FailedToTransactAsset("AccountIdConversionFailed")
                },
            }
        }
    }

    pub struct CurrencyAdapterWithFee<
        Currency,
        Matcher,
        AccountIdConverter,
        AccountId,
        CheckedAccount,
        DepositFeePercent,
        WithdrawalFeePercent,
        FeeReceiverAccount,
    >(
        PhantomData<(
            Currency,
            Matcher,
            AccountIdConverter,
            AccountId,
            CheckedAccount,
            DepositFeePercent,
            WithdrawalFeePercent,
            FeeReceiverAccount,
        )>,
    );

    impl<
            Currency: frame_support::traits::Currency<AccountId>,
            Matcher: MatchesFungible<Currency::Balance>,
            AccountIdConverter: ConvertLocation<AccountId>,
            AccountId: Clone, // can't get away without it since Currency is generic over it.
            CheckedAccount: Get<Option<(AccountId, MintLocation)>>,
            DepositFeePercent: Get<u8>,
            WithdrawalFeePercent: Get<u8>,
            FeeReceiverAccount: Get<AccountId>,
        > TransactAsset
        for CurrencyAdapterWithFee<
            Currency,
            Matcher,
            AccountIdConverter,
            AccountId,
            CheckedAccount,
            DepositFeePercent,
            WithdrawalFeePercent,
            FeeReceiverAccount,
        >
    {
        fn can_check_in(origin: &Location, what: &Asset, context: &XcmContext) -> Result {
            CurrencyAdapter::<
                Currency,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckedAccount,
            >::can_check_in(origin, what, context)
        }

        fn check_in(origin: &Location, what: &Asset, context: &XcmContext) {
            CurrencyAdapter::<
                Currency,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckedAccount,
            >::check_in(origin, what, context)
        }

        fn can_check_out(dest: &Location, what: &Asset, context: &XcmContext) -> Result {
            CurrencyAdapter::<
                Currency,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckedAccount,
            >::can_check_out(dest, what, context)
        }

        fn check_out(dest: &Location, what: &Asset, context: &XcmContext) {
            CurrencyAdapter::<
                Currency,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckedAccount,
            >::check_out(dest, what, context)
        }

        fn deposit_asset(what: &Asset, who: &Location, _context: Option<&XcmContext>) -> Result {
            log::trace!(target: "xcm::currency_adapter_with_fee", "deposit_asset what: {:?}, who: {:?}", what, who);
            // Check we handle this asset.
            let amount = Matcher::matches_fungible(what).ok_or(Error::AssetNotHandled)?;

            let who = AccountIdConverter::convert_location(who)
                .ok_or(Error::AccountIdConversionFailed)?;

            let fee_percent = Percent::from_percent(DepositFeePercent::get());
            let fee_amount = fee_percent.mul_floor(amount);
            log::trace!(target: "xcm::currency_adapter_with_fee", "deposit_asset fee: {:?}", fee_amount);

            let _imbalance = Currency::deposit_creating(&who, amount.saturating_sub(fee_amount));
            let _imbalance = Currency::deposit_creating(&FeeReceiverAccount::get(), fee_amount);
            Ok(())
        }

        fn withdraw_asset(
            what: &Asset,
            who: &Location,
            _maybe_context: Option<&XcmContext>,
        ) -> sp_std::result::Result<AssetsInHolding, XcmError> {
            log::trace!(target: "xcm::currency_adapter_with_fee", "withdraw_asset what: {:?}, who: {:?}", what, who);
            // Check we handle this asset.
            let amount = Matcher::matches_fungible(what).ok_or(Error::AssetNotHandled)?;
            let who = AccountIdConverter::convert_location(who)
                .ok_or(Error::AccountIdConversionFailed)?;

            let fee_percent = Percent::from_percent(WithdrawalFeePercent::get());
            let fee_amount = fee_percent.mul_floor(amount);
            log::trace!(target: "xcm::currency_adapter_with_fee", "withdraw_asset fee: {:?}", fee_amount);

            let amount_with_fee = amount.saturating_add(fee_amount);
            let new_balance = Currency::free_balance(&who)
                .checked_sub(&amount_with_fee)
                .ok_or(XcmError::NotWithdrawable)?;
            Currency::ensure_can_withdraw(
                &who,
                amount_with_fee,
                WithdrawReasons::TRANSFER,
                new_balance,
            )
            .map_err(|_| XcmError::NotWithdrawable)?;

            Currency::withdraw(&who, amount, WithdrawReasons::TRANSFER, KeepAlive)
                .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

            Currency::transfer(&who, &FeeReceiverAccount::get(), fee_amount, KeepAlive)
                .map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

            Ok(what.clone().into())
        }

        fn internal_transfer_asset(
            asset: &Asset,
            from: &Location,
            to: &Location,
            context: &XcmContext,
        ) -> sp_std::result::Result<AssetsInHolding, XcmError> {
            CurrencyAdapter::<
                Currency,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckedAccount,
            >::internal_transfer_asset(asset, from, to, context)
        }
    }
}
