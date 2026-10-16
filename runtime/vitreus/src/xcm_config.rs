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
    parachains_hrmp, parachains_origin, AccountId, AllPalletsWithSystem, Assets, Balance, Balances,
    CouncilCollective, DynamicEnergy, EnergyAsset, EnergyBroker, ParaId, Runtime, RuntimeCall,
    RuntimeEvent, RuntimeOrigin, TransactionPicosecondFee, Treasury, XcmPallet, LNRG, SNRG, VNRG,
};
use frame_support::{
    parameter_types,
    traits::{Contains, ContainsPair, Equals, Everything, Nothing},
    weights::{ConstantMultiplier, Weight},
};
use frame_system::EnsureRoot;
use origin_conversion::SignedToAccountKey20;
use polkadot_runtime_common::{
    paras_registrar, prod_or_fast,
    xcm_sender::{ChildParachainRouter, NoPriceForMessageDelivery},
};
use sp_core::ConstU32;
use sp_runtime::traits::TryConvertInto;
use xcm::latest::prelude::*;
use xcm_builder::{
    AccountKey20Aliases, AllowExplicitUnpaidExecutionFrom, AllowKnownQueryResponses,
    AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom, AsPrefixedGeneralIndex,
    BackingToPlurality, ChildParachainAsNative, ChildParachainConvertsVia, FixedWeightBounds,
    FrameTransactionalProcessor, FungiblesAdapter, IsChildSystemParachain, IsConcrete,
    MatchedConvertedConcreteId, MintLocation, NoChecking, SignedAccountKey20AsNative,
    SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId, UsingComponents,
    WithComputedOrigin, WithUniqueTopic, XcmFeeManagerFromComponents,
};
use xcm_executor::{
    traits::{MatchesFungibles, TransactAsset, WithOriginFilter},
    XcmExecutor,
};

// TODO: use constants from `vitreus-runtime-constants` crate
const RELAY_NETWORK: NetworkId = prod_or_fast!(
    NetworkId::ByGenesis(hex_literal::hex!(
        "4f27ff2e1c714c78b718d11a999774b2f639da713b9481337942997140185cfc"
    )),
    NetworkId::ByGenesis(hex_literal::hex!(
        "c28caa6bf827d357af8ca58ddcefc2aba6d08b2fd29de8acb67becd4ea6c3673"
    ))
);
pub const ASSETS_PALLET_ID: u8 = 5;

parameter_types! {
    pub const TokenLocation: Location = Here.into_location();
    pub const ThisNetwork: NetworkId = RELAY_NETWORK;
    pub UniversalLocation: InteriorLocation = ThisNetwork::get().into();
    pub CheckAccount: AccountId = XcmPallet::check_account();
    pub TreasuryAccount: AccountId = Treasury::account_id();
    pub LocalCheckAccount: (AccountId, MintLocation) = (CheckAccount::get(), MintLocation::Local);
}

parameter_types! {
    pub EnergyTokenLocation: Location = Location::new(
        0,
        [PalletInstance(ASSETS_PALLET_ID), GeneralIndex(VNRG::get())]
    );
    pub StaticEnergyTokenLocation: Location = Location::new(
        0,
        [PalletInstance(ASSETS_PALLET_ID), GeneralIndex(SNRG::get())]
    );
    pub LiquidEnergyTokenLocation: Location = Location::new(
        0,
        [PalletInstance(ASSETS_PALLET_ID), GeneralIndex(LNRG::get())]
    );
}

pub type LocationConverter = (
    // We can convert a child parachain using the standard `AccountId` conversion.
    ChildParachainConvertsVia<ParaId, AccountId>,
    // We can directly alias an `AccountKey20` into a local account.
    AccountKey20Aliases<ThisNetwork, AccountId>,
);

/// Our asset transactor. This is what allows us to interest with the runtime facilities from the point of
/// view of XCM-only concepts like `Location` and `Asset`.
///
/// Ours is only aware of the Balances pallet, which is mapped to `TokenLocation`.
#[allow(deprecated)]
pub type LocalAssetTransactor = xcm_builder::CurrencyAdapter<
    // Use this currency:
    Balances,
    // Use this currency when it is a fungible asset matching the given location or name:
    IsConcrete<TokenLocation>,
    // We can convert the Locations with our converter above:
    LocationConverter,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We track our teleports in/out to keep total issuance correct.
    LocalCheckAccount,
>;

parameter_types! {
    pub AssetsPalletLocation: Location = PalletInstance(ASSETS_PALLET_ID).into();
}

pub type EnergyTokenMatcher = MatchedConvertedConcreteId<
    super::AssetId,
    Balance,
    (
        Equals<EnergyTokenLocation>,
        Equals<StaticEnergyTokenLocation>,
        Equals<LiquidEnergyTokenLocation>,
    ),
    AsPrefixedGeneralIndex<AssetsPalletLocation, super::AssetId, TryConvertInto>,
    TryConvertInto,
>;

/// Means for transacting energy.
pub type EnergyTransactor = energy::EnergyAdapter<
    // Use this fungibles implementation:
    Assets,
    // Use this currency when it is a fungible asset matching the given location or name:
    EnergyTokenMatcher,
    // Convert an XCM Location into a local account id:
    LocationConverter,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // Does not check teleports.
    NoChecking,
    // The account to use for tracking teleports.
    CheckAccount,
    // Track burned energy.
    (DynamicEnergy, EnergyBroker),
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
}

// No delivery fee: the sender pays nothing for delivery, so the only cost is the regular
// transaction fee. Safe only while arbitrary outbound XCM stays restricted to governance (see
// `SendXcmOrigin` / `XcmExecuteFilter` here and `BaseCallFilter` in the runtime); reopening those
// entry points would require pricing delivery again.
pub type PriceForChildParachainDelivery = NoPriceForMessageDelivery<ParaId>;

/// The XCM router. When we want to send an XCM message, we use this type. It amalgamates all of our
/// individual routers.
pub type XcmRouter = WithUniqueTopic<
    // Only one router so far - use DMP to communicate with child parachains.
    ChildParachainRouter<Runtime, XcmPallet, PriceForChildParachainDelivery>,
>;

parameter_types! {
    pub Vnrg: AssetFilter = Wild(AllOf { fun: WildFungible, id: AssetId(EnergyTokenLocation::get()) });
    pub Snrg: AssetFilter = Wild(AllOf { fun: WildFungible, id: AssetId(StaticEnergyTokenLocation::get()) });
    pub Lnrg: AssetFilter = Wild(AllOf { fun: WildFungible, id: AssetId(LiquidEnergyTokenLocation::get()) });
    pub const MaxAssetsIntoHolding: u32 = 64;
}

pub struct OnlyParachains;
impl Contains<Location> for OnlyParachains {
    fn contains(loc: &Location) -> bool {
        matches!(loc.unpack(), (0, [Parachain(_)]))
    }
}

pub struct EnergyForParachains;
impl ContainsPair<Asset, Location> for EnergyForParachains {
    fn contains(asset: &Asset, location: &Location) -> bool {
        log::trace!(target: "xcm::contains", "EnergyForParachains asset: {:?}, location: {:?}", asset, location);

        if !OnlyParachains::contains(location) {
            return false;
        }

        Vnrg::get().matches(asset) || Snrg::get().matches(asset) || Lnrg::get().matches(asset)
    }
}

pub type TrustedTeleporters = (EnergyForParachains,);

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

/// Calls a parachain may dispatch on the relay via XCM `Transact` as its own origin — only the
/// calls gated on a parachain origin: HRMP channel management and the registrar self-service ops
/// (lock/unlock, swap, deregister). Root-only HRMP `force_*`, signed-origin `reserve`, and the
/// unbounded `register` are excluded. All allowed calls are fixed-weight with bounded proof size
/// and cannot lead to another call; everything else is rejected.
pub struct SafeCallFilter;
impl Contains<RuntimeCall> for SafeCallFilter {
    fn contains(call: &RuntimeCall) -> bool {
        #[cfg(feature = "runtime-benchmarks")]
        {
            if matches!(call, RuntimeCall::System(frame_system::Call::remark_with_event { .. })) {
                return true;
            }
        }

        matches!(
            call,
            RuntimeCall::Hrmp(
                parachains_hrmp::Call::hrmp_init_open_channel { .. }
                    | parachains_hrmp::Call::hrmp_accept_open_channel { .. }
                    | parachains_hrmp::Call::hrmp_close_channel { .. }
                    | parachains_hrmp::Call::hrmp_cancel_open_request { .. }
                    | parachains_hrmp::Call::establish_channel_with_system { .. },
            ) | RuntimeCall::Registrar(
                paras_registrar::Call::deregister { .. }
                    | paras_registrar::Call::swap { .. }
                    | paras_registrar::Call::add_lock { .. }
                    | paras_registrar::Call::remove_lock { .. },
            )
        )
    }
}

/// Locations that will not be charged fees in the executor, neither for execution nor delivery.
pub type WaivedLocations = ();
pub type WeightToFee = ConstantMultiplier<Balance, TransactionPicosecondFee>;

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    type AssetTransactor = (LocalAssetTransactor, EnergyTransactor);
    type OriginConverter = LocalOriginConverter;
    type IsReserve = ();
    type IsTeleporter = TrustedTeleporters;
    type UniversalLocation = UniversalLocation;
    type Barrier = Barrier;
    type Weigher = FixedWeightBounds<FixedXcmWeight, RuntimeCall, MaxInstructions>;
    type Trader = UsingComponents<
        WeightToFee,
        EnergyTokenLocation,
        AccountId,
        EnergyAsset,
        energy::OnUnbalancedEnergy<AccountId, EnergyAsset, (DynamicEnergy, EnergyBroker)>,
    >;
    type ResponseHandler = XcmPallet;
    type AssetTrap = XcmPallet;
    type AssetLocker = ();
    type AssetExchanger = ();
    type AssetClaims = XcmPallet;
    type SubscriptionService = XcmPallet;
    type PalletInstancesInfo = AllPalletsWithSystem;
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type FeeManager = XcmFeeManagerFromComponents<
        WaivedLocations,
        energy::EnergyFeeHandler<
            super::AssetId,
            Balance,
            EnergyTokenMatcher,
            (DynamicEnergy, EnergyBroker),
        >,
    >;
    type MessageExporter = ();
    type UniversalAliases = Nothing;
    type CallDispatcher = WithOriginFilter<SafeCallFilter>;
    type SafeCallFilter = SafeCallFilter;
    type Aliasers = Nothing;
    type TransactionalProcessor = FrameTransactionalProcessor;
    type HrmpNewChannelOpenRequestHandler = ();
    type HrmpChannelAcceptedHandler = ();
    type HrmpChannelClosingHandler = ();
    type XcmRecorder = XcmPallet;
}

parameter_types! {
    pub const CouncilBodyId: BodyId = BodyId::Executive;
}

/// Type to convert the council origin to a Plurality `Location` value.
pub type CouncilToPlurality = BackingToPlurality<
    RuntimeOrigin,
    pallet_collective::Origin<Runtime, CouncilCollective>,
    CouncilBodyId,
>;

/// Type to convert an `Origin` type value into a `Location` value which represents an interior location
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
    // Arbitrary `send` is restricted to governance; signed accounts use the transfer extrinsics.
    type SendXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, CouncilToPlurality>;
    type XcmRouter = XcmRouter;
    // Keep the origin converter permissive so the transfer extrinsics work for signed accounts;
    // the `execute` call itself is disabled via `XcmExecuteFilter` below.
    type ExecuteXcmOrigin = xcm_builder::EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmExecuteFilter = Nothing;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Everything;
    // Reserve transfers are disabled: assets move to parachains via teleport only, and no reserve
    // is configured (`IsReserve = ()`).
    type XcmReserveTransferFilter = Nothing;
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
    type AdminOrigin = EnsureRoot<AccountId>;
}

mod energy {
    use frame_support::traits::{
        fungible::{Balanced, Credit, Inspect},
        fungibles, Get, Imbalance, OnUnbalanced,
    };
    use sp_std::marker::PhantomData;
    use vitreus_runtime_common::OnEnergyBurn;
    use xcm::latest::Assets;
    use xcm_builder::{AssetChecking, HandleFee};
    use xcm_executor::traits::{ConvertLocation, FeeReason};

    use super::*;

    pub struct EnergyAdapter<
        Assets,
        Matcher,
        AccountIdConverter,
        AccountId,
        CheckAsset,
        CheckingAccount,
        OnBurn,
    >(
        PhantomData<(
            Assets,
            Matcher,
            AccountIdConverter,
            AccountId,
            CheckAsset,
            CheckingAccount,
            OnBurn,
        )>,
    );
    impl<
            Assets: fungibles::Mutate<AccountId>,
            Matcher: MatchesFungibles<Assets::AssetId, Assets::Balance>,
            AccountIdConverter: ConvertLocation<AccountId>,
            AccountId: Eq + Clone, /* can't get away without it since Currency is generic over it. */
            CheckAsset: AssetChecking<Assets::AssetId>,
            CheckingAccount: Get<AccountId>,
            OnBurn: OnEnergyBurn<Assets::Balance>,
        > TransactAsset
        for EnergyAdapter<
            Assets,
            Matcher,
            AccountIdConverter,
            AccountId,
            CheckAsset,
            CheckingAccount,
            OnBurn,
        >
    {
        fn can_check_in(_origin: &Location, what: &Asset, _context: &XcmContext) -> XcmResult {
            // The relay is the sole issuer of energy; accepting a teleport in would let a
            // parachain mint relay-side energy outside its economy. Reject energy explicitly,
            // and defer any other asset so the transactor tuple can try the next adapter.
            if Matcher::matches_fungibles(what).is_ok() {
                return Err(XcmError::UntrustedTeleportLocation);
            }
            Err(XcmError::AssetNotFound)
        }

        fn check_in(_origin: &Location, _what: &Asset, _context: &XcmContext) {}

        fn can_check_out(dest: &Location, what: &Asset, context: &XcmContext) -> XcmResult {
            FungiblesAdapter::<
                Assets,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckAsset,
                CheckingAccount,
            >::can_check_out(dest, what, context)
        }

        fn check_out(dest: &Location, what: &Asset, context: &XcmContext) {
            FungiblesAdapter::<
                Assets,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckAsset,
                CheckingAccount,
            >::check_out(dest, what, context);

            if let Ok((_, amount)) = Matcher::matches_fungibles(what) {
                OnBurn::on_energy_burn(amount);
            }
        }

        fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
            FungiblesAdapter::<
                Assets,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckAsset,
                CheckingAccount,
            >::deposit_asset(what, who, context)
        }

        fn withdraw_asset(
            what: &Asset,
            who: &Location,
            maybe_context: Option<&XcmContext>,
        ) -> Result<xcm_executor::AssetsInHolding, XcmError> {
            FungiblesAdapter::<
                Assets,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckAsset,
                CheckingAccount,
            >::withdraw_asset(what, who, maybe_context)
        }

        fn internal_transfer_asset(
            what: &Asset,
            from: &Location,
            to: &Location,
            context: &XcmContext,
        ) -> Result<xcm_executor::AssetsInHolding, XcmError> {
            FungiblesAdapter::<
                Assets,
                Matcher,
                AccountIdConverter,
                AccountId,
                CheckAsset,
                CheckingAccount,
            >::internal_transfer_asset(what, from, to, context)
        }
    }

    pub struct EnergyFeeHandler<AssetId, Balance, Matcher, OnBurn>(
        PhantomData<(AssetId, Balance, Matcher, OnBurn)>,
    );
    impl<
            AssetId,
            Balance,
            Matcher: MatchesFungibles<AssetId, Balance>,
            OnBurn: OnEnergyBurn<Balance>,
        > HandleFee for EnergyFeeHandler<AssetId, Balance, Matcher, OnBurn>
    {
        fn handle_fee(fee: Assets, _context: Option<&XcmContext>, _reason: FeeReason) -> Assets {
            let mut assets = fee.into_inner();

            assets.retain(|asset| {
                if let Ok((_, amount)) = Matcher::matches_fungibles(asset) {
                    OnBurn::on_energy_burn(amount);
                    false
                } else {
                    true
                }
            });

            Assets::from_sorted_and_deduplicated_skip_checks(assets)
        }
    }

    pub struct OnUnbalancedEnergy<AccountId, Fungible, OnBurn>(
        PhantomData<(AccountId, Fungible, OnBurn)>,
    );
    impl<
            AccountId,
            Fungible: Balanced<AccountId> + Inspect<AccountId>,
            OnBurn: OnEnergyBurn<Fungible::Balance>,
        > OnUnbalanced<Credit<AccountId, Fungible>>
        for OnUnbalancedEnergy<AccountId, Fungible, OnBurn>
    {
        fn on_nonzero_unbalanced(credit: Credit<AccountId, Fungible>) {
            OnBurn::on_energy_burn(credit.peek());
            drop(credit);
        }
    }
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
