use ethereum::{EIP1559Transaction, EIP2930Transaction, LegacyTransaction};
use fp_evm::FeeCalculator;
use frame_support::{
    dispatch::GetDispatchInfo,
    traits::{
        fungible,
        fungible::Inspect,
        tokens::{DepositConsequence, Precision, Provenance, WithdrawConsequence},
        tokens::{Fortitude, Preservation},
        Currency, ExistenceRequirement, FindAuthor, Imbalance, OnUnbalanced, PrivilegeCmp,
        ProcessMessage, ProcessMessageError, SignedImbalance, WithdrawReasons,
    },
    weights::WeightMeter,
};
use pallet_energy_fee::{CallFee, CustomFee};
use pallet_ethereum::{Call::transact, Transaction as EthereumTransaction};
use pallet_evm::AddressMapping;
use pallet_reputation::{ReputationTier, RANKS_PER_TIER};
use pallet_treasury::NegativeImbalanceOf;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_primitives::ValidatorIndex;
use polkadot_runtime_parachains::inclusion::{AggregateMessageOrigin, UmpQueueId};
use sp_consensus_beefy::mmr::BeefyDataProvider;
use sp_core::{ByteArray, U256};
use sp_runtime::{
    traits::{
        AccountIdConversion, Convert, DispatchInfoOf, Dispatchable, Extrinsic, PostDispatchInfoOf,
        Zero,
    },
    transaction_validity::{InvalidTransaction, TransactionValidity, TransactionValidityError},
    ConsensusEngineId, DispatchError, DispatchResult, FixedPointNumber, FixedU64, RuntimeDebug,
    SaturatedConversion, Saturating,
};
use sp_std::{cmp::Ordering, marker::PhantomData};
use vitreus_runtime_common::ExposureMultiplier;

use super::*;

mod converters;
pub use converters::*;

mod runner;

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        public: <Signature as Verify>::Signer,
        account: AccountId,
        nonce: Index,
    ) -> Option<(RuntimeCall, <UncheckedExtrinsic as Extrinsic>::SignaturePayload)> {
        let tip = 0;
        // take the biggest period possible.
        let period =
            BlockHashCount::get().checked_next_power_of_two().map(|c| c / 2).unwrap_or(2) as u64;
        let current_block = System::block_number()
            .saturated_into::<u64>()
            // The `System::block_number` is initialized with `n+1`,
            // so the actual block number is `n`.
            .saturating_sub(1);
        let era = generic::Era::mortal(period, current_block);
        let extra = (
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(era),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
            pallet_energy_fee::CheckEnergyFee::<Runtime>::new(),
        );
        let raw_payload = SignedPayload::new(call, extra)
            .map_err(|e| {
                log::warn!("Unable to create signed payload: {:?}", e);
            })
            .ok()?;
        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
        // let address = AccountIdLookup::unlookup(account);
        let (call, extra, _) = raw_payload.deconstruct();
        Some((call, (account, signature, extra)))
    }
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    RuntimeCall: From<C>,
{
    type Extrinsic = UncheckedExtrinsic;
    type OverarchingCall = RuntimeCall;
}

pub struct ParasProvider;
impl BeefyDataProvider<H256> for ParasProvider {
    fn extra_data() -> H256 {
        let mut para_heads: Vec<(u32, Vec<u8>)> = parachains_paras::Parachains::<Runtime>::get()
            .into_iter()
            .filter_map(|id| {
                parachains_paras::Heads::<Runtime>::get(id).map(|head| (id.into(), head.0))
            })
            .collect();
        para_heads.sort();
        binary_merkle_tree::merkle_root::<mmr::Hashing, _>(
            para_heads.into_iter().map(|pair| pair.encode()),
        )
    }
}

/// Special `RewardValidators` that does nothing ;)
pub struct RewardValidators;
impl parachains_inclusion::RewardValidators for RewardValidators {
    fn reward_backing(_: impl IntoIterator<Item = ValidatorIndex>) {}
    fn reward_bitfields(_: impl IntoIterator<Item = ValidatorIndex>) {}
}

/// Message processor to handle any messages that were enqueued into the `MessageQueue` pallet.
pub struct MessageProcessor;
impl ProcessMessage for MessageProcessor {
    type Origin = AggregateMessageOrigin;

    fn process_message(
        message: &[u8],
        origin: Self::Origin,
        meter: &mut WeightMeter,
        id: &mut [u8; 32],
    ) -> Result<bool, ProcessMessageError> {
        use xcm::latest::Junction;

        let para = match origin {
            AggregateMessageOrigin::Ump(UmpQueueId::Para(para)) => para,
        };
        xcm_builder::ProcessXcmMessage::<
            Junction,
            xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
            RuntimeCall,
        >::process_message(message, Junction::Parachain(para.into()), meter, id)
    }
}

pub struct TreasuryAccountId<R, I: 'static = ()>(sp_std::marker::PhantomData<(R, I)>);
impl<R, I> sp_runtime::traits::TypedGet for TreasuryAccountId<R, I>
where
    R: pallet_treasury::Config<I>,
{
    type Type = <R as frame_system::Config>::AccountId;
    fn get() -> Self::Type {
        <pallet_treasury::Pallet<R, I>>::account_id()
    }
}

pub struct StakingRewardsSink;
impl OnUnbalanced<NegativeImbalanceOf<Runtime>> for StakingRewardsSink {
    fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<Runtime>) {
        let staking_rewards_address: AccountId =
            StakingRewardsPalletId::get().into_account_truncating();
        Balances::resolve_creating(&staking_rewards_address, amount);
    }
}

// We implement CusomFee here since the RuntimeCall defined in construct_runtime! macro
impl CustomFee<RuntimeCall, DispatchInfoOf<RuntimeCall>, Balance, GetConstantEnergyFee>
    for EnergyFee
{
    fn dispatch_info_to_fee(
        runtime_call: &RuntimeCall,
        dispatch_info: Option<&DispatchInfoOf<RuntimeCall>>,
        calculated_fee: Option<Balance>,
    ) -> CallFee<Balance> {
        match runtime_call {
            RuntimeCall::Assets(..)
            | RuntimeCall::AssetRate(..)
            | RuntimeCall::Auctions(..)
            | RuntimeCall::Balances(..)
            | RuntimeCall::Bounties(..)
            | RuntimeCall::DynamicEnergy(..)
            | RuntimeCall::EnergyGeneration(..)
            | RuntimeCall::EnergyBroker(..)
            | RuntimeCall::Nfts(..)
            | RuntimeCall::AtomicSwap(..)
            | RuntimeCall::Claiming(..)
            | RuntimeCall::Kickstart(..)
            | RuntimeCall::Vesting(..)
            | RuntimeCall::NacManaging(..)
            | RuntimeCall::ManualBridge(..)
            | RuntimeCall::Privileges(..)
            | RuntimeCall::Council(..)
            | RuntimeCall::TechnicalCommittee(..)
            | RuntimeCall::TechnicalMembership(..)
            | RuntimeCall::Treasury(..)
            | RuntimeCall::TechnicalCommitteeTreasury(..)
            | RuntimeCall::Democracy(..)
            | RuntimeCall::Elections(..)
            | RuntimeCall::Session(..)
            | RuntimeCall::XcmPallet(..)
            | RuntimeCall::SimpleVesting(..)
            | RuntimeCall::Multisig(..)
            | RuntimeCall::Proxy(..)
            | RuntimeCall::Reputation(..) => CallFee::Regular(Self::custom_fee()),
            RuntimeCall::EVM(..) | RuntimeCall::Ethereum(..) => CallFee::EVM(Self::ethereum_fee()),
            RuntimeCall::Utility(pallet_utility::Call::batch { calls })
            | RuntimeCall::Utility(pallet_utility::Call::batch_all { calls })
            | RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => {
                let resulting_fee = calls
                    .iter()
                    .map(|call| Self::dispatch_info_to_fee(call, None, None))
                    .fold(Balance::zero(), |acc, call_fee| match call_fee {
                        CallFee::Regular(fee) => acc.saturating_add(fee),
                        CallFee::EVM(fee) => acc.saturating_add(fee),
                    })
                    .max(Self::custom_fee());
                CallFee::Regular(resulting_fee)
            },
            RuntimeCall::Utility(pallet_utility::Call::dispatch_as { call, .. })
            | RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. }) => {
                Self::dispatch_info_to_fee(call, None, calculated_fee)
            },
            RuntimeCall::Sudo(..) => CallFee::Regular(0),
            _ => CallFee::Regular(Self::weight_fee(runtime_call, dispatch_info, calculated_fee)),
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
                pallet_transaction_payment::Pallet::<Runtime>::compute_fee(len, info, Zero::zero())
            } else {
                let info = &runtime_call.get_dispatch_info();
                pallet_transaction_payment::Pallet::<Runtime>::compute_fee(len, info, Zero::zero())
            }
        }
    }
}

pub struct FindAuthorTruncated<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for FindAuthorTruncated<F> {
    fn find_author<'a, I>(digests: I) -> Option<H160>
    where
        I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
    {
        if let Some(author_index) = F::find_author(digests) {
            let (public, _) = Babe::authorities()[author_index as usize].clone();
            return Some(H160::from_slice(&public.to_raw_vec()[4..24]));
        }
        None
    }
}

pub struct FixedFeeCalculator;
impl FeeCalculator for FixedFeeCalculator {
    fn min_gas_price() -> (U256, Weight) {
        (U256::one(), Weight::zero())
    }
}

#[derive(Clone)]
pub struct TransactionConverter;
impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
    fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        )
    }
}
impl fp_rpc::ConvertTransaction<opaque::UncheckedExtrinsic> for TransactionConverter {
    fn convert_transaction(
        &self,
        transaction: pallet_ethereum::Transaction,
    ) -> opaque::UncheckedExtrinsic {
        let extrinsic = UncheckedExtrinsic::new_unsigned(
            pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
        );
        let encoded = extrinsic.encode();
        opaque::UncheckedExtrinsic::decode(&mut &encoded[..])
            .expect("Encoded extrinsic is always valid")
    }
}

// user doesn't have NAC to dispatch transaction
const ACCESS_RESTRICTED: u8 = u8::MAX;

impl fp_self_contained::SelfContainedCall for RuntimeCall {
    type SignedInfo = H160;

    fn is_self_contained(&self) -> bool {
        match self {
            RuntimeCall::Ethereum(call) => call.is_self_contained(),
            _ => false,
        }
    }

    fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => call.check_self_contained(),
            _ => None,
        }
    }

    fn validate_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<TransactionValidity> {
        match self {
            RuntimeCall::Ethereum(call) => {
                let account_id =
                    <Runtime as pallet_evm::Config>::AddressMapping::into_account_id(*info);

                if let CallFee::EVM(amount) =
                    EnergyFee::dispatch_info_to_fee(self, Some(dispatch_info), None)
                {
                    let (_, fee_vtrs_amount) =
                        if let Some(parts) = EnergyFee::calculate_fee_parts(&account_id, amount) {
                            parts
                        } else {
                            return Some(Err(InvalidTransaction::Payment.into()));
                        };

                    let vtrs_balance = Balances::reducible_balance(
                        &account_id,
                        Preservation::Protect,
                        Fortitude::Polite,
                    );

                    if fee_vtrs_amount > vtrs_balance {
                        return Some(Err(InvalidTransaction::Payment.into()));
                    }
                }

                if !NacManaging::user_has_access(account_id, runner::CALL_ACCESS_LEVEL) {
                    return Some(Err(InvalidTransaction::Custom(ACCESS_RESTRICTED).into()));
                };

                call.validate_self_contained(info, dispatch_info, len)
            },
            _ => None,
        }
    }

    fn pre_dispatch_self_contained(
        &self,
        info: &Self::SignedInfo,
        dispatch_info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Option<Result<(), TransactionValidityError>> {
        match self {
            RuntimeCall::Ethereum(call) => {
                call.pre_dispatch_self_contained(info, dispatch_info, len)
            },
            _ => None,
        }
    }

    fn apply_self_contained(
        self,
        info: Self::SignedInfo,
    ) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
        match self {
            call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) => {
                Some(call.dispatch(RuntimeOrigin::from(
                    pallet_ethereum::RawOrigin::EthereumTransaction(info),
                )))
            },
            _ => None,
        }
    }
}

#[derive(
    Default,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Encode,
    Decode,
    RuntimeDebug,
    MaxEncodedLen,
    scale_info::TypeInfo,
)]
pub enum ProxyType {
    #[default]
    Any = 0,
    Staking = 1,
}
impl frame_support::traits::InstanceFilter<RuntimeCall> for ProxyType {
    fn filter(&self, c: &RuntimeCall) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::Staking => {
                matches!(
                    c,
                    RuntimeCall::EnergyGeneration(..)
                        | RuntimeCall::Session(..)
                        | RuntimeCall::Utility(..)
                )
            },
        }
    }
    fn is_superset(&self, o: &Self) -> bool {
        match (self, o) {
            (x, y) if x == y => true,
            (ProxyType::Any, _) => true,
            (_, ProxyType::Any) => false,
            _ => false,
        }
    }
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;
impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
    fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
        if left == right {
            return Some(Ordering::Equal);
        }

        match (left, right) {
            // Root is greater than anything.
            (OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
            // Check which one has more yes votes.
            (
                OriginCaller::Council(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
                OriginCaller::Council(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
            ) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
            // For every other origin we don't care, as they are not used for `ScheduleOrigin`.
            _ => None,
        }
    }
}

pub struct ReputationExposureMultiplier;
impl Convert<&ReputationTier, FixedU64> for ReputationExposureMultiplier {
    fn convert(k: &ReputationTier) -> FixedU64 {
        match k {
            ReputationTier::Vanguard(2) => FixedU64::from_rational(2, 100),
            ReputationTier::Vanguard(3) => FixedU64::from_rational(4, 100),
            ReputationTier::Trailblazer(0) => FixedU64::from_rational(5, 100),
            ReputationTier::Trailblazer(1) => FixedU64::from_rational(8, 100),
            ReputationTier::Trailblazer(2) => FixedU64::from_rational(10, 100),
            ReputationTier::Trailblazer(3) => FixedU64::from_rational(12, 100),
            ReputationTier::Ultramodern(0) => FixedU64::from_rational(13, 100),
            ReputationTier::Ultramodern(1) => FixedU64::from_rational(16, 100),
            ReputationTier::Ultramodern(2) => FixedU64::from_rational(18, 100),
            ReputationTier::Ultramodern(3) => FixedU64::from_rational(20, 100),
            ReputationTier::Ultramodern(rank) => {
                let additional_percentage = rank.saturating_sub(RANKS_PER_TIER);
                FixedU64::from_rational(20_u8.saturating_add(additional_percentage).into(), 100)
            },
            // includes unhandled cases
            _ => FixedU64::zero(),
        }
    }
}
impl ExposureMultiplier<AccountId> for ReputationExposureMultiplier {
    fn bonus_part(account_id: &AccountId) -> FixedU64 {
        Reputation::reputation(account_id)
            .and_then(|record| record.reputation.tier())
            .map(|tier| Self::convert(&tier))
            .unwrap_or_default()
    }
}

#[derive(Decode, Encode, Clone, PartialEq, Eq, Debug, scale_info::TypeInfo)]
pub struct KickstartClaimData {
    collection_id: u32,
    item_id: u32,
    level: u32,
}

pub struct KickstartClaimHandler;
impl pallet_claiming::OnClaimHandler<AccountId, Balance, KickstartClaimData>
    for KickstartClaimHandler
{
    fn on_claim(
        who: &AccountId,
        _amount: Balance,
        data: Option<KickstartClaimData>,
    ) -> DispatchResult {
        use frame_support::traits::nonfungibles_v2::Mutate;
        use pallet_nfts::{ItemConfig, ItemSettings};

        const NFT_LEVEL_ATTRIBUTE_KEY: [u8; 3] = [0, 0, 1];

        if let Some(KickstartClaimData { collection_id, item_id, level }) = data {
            let item_config = ItemConfig { settings: ItemSettings::all_enabled() };

            <Nfts as Mutate<AccountId, ItemConfig>>::mint_into(
                &collection_id,
                &item_id,
                who,
                &item_config,
                true,
            )?;
            <Nfts as Mutate<AccountId, ItemConfig>>::set_attribute(
                &collection_id,
                &item_id,
                &Vec::from(NFT_LEVEL_ATTRIBUTE_KEY),
                &level.to_le_bytes(),
            )?;
        }

        Ok(())
    }
}

pub struct CurrencyAdapter<T>(core::marker::PhantomData<T>);

impl<T> Currency<AccountId> for CurrencyAdapter<T>
where
    T: fungible::Inspect<AccountId> + fungible::Balanced<AccountId> + fungible::Mutate<AccountId>,
{
    type Balance = <T as fungible::Inspect<AccountId>>::Balance;
    type PositiveImbalance = fungible::Debt<AccountId, T>;
    type NegativeImbalance = fungible::Credit<AccountId, T>;

    fn total_balance(who: &AccountId) -> Self::Balance {
        T::total_balance(who)
    }

    fn can_slash(who: &AccountId, value: Self::Balance) -> bool {
        if value.is_zero() {
            return true;
        }
        Self::free_balance(who) >= value
    }

    fn total_issuance() -> Self::Balance {
        T::total_issuance()
    }

    fn minimum_balance() -> Self::Balance {
        T::minimum_balance()
    }

    fn burn(amount: Self::Balance) -> Self::PositiveImbalance {
        if amount.is_zero() {
            return Self::PositiveImbalance::zero();
        }
        T::rescind(amount)
    }

    fn issue(amount: Self::Balance) -> Self::NegativeImbalance {
        if amount.is_zero() {
            return Self::NegativeImbalance::zero();
        }
        T::issue(amount)
    }

    fn free_balance(who: &AccountId) -> Self::Balance {
        T::reducible_balance(who, Preservation::Preserve, Fortitude::Polite)
    }

    fn ensure_can_withdraw(
        who: &AccountId,
        amount: Self::Balance,
        _reasons: WithdrawReasons,
        _new_balance: Self::Balance,
    ) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        T::can_withdraw(who, amount).into_result(true).map(|_| ())
    }

    fn transfer(
        source: &AccountId,
        dest: &AccountId,
        value: Self::Balance,
        existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        if value.is_zero() {
            return Ok(());
        }

        let preservation = match existence_requirement {
            ExistenceRequirement::KeepAlive => Preservation::Preserve,
            ExistenceRequirement::AllowDeath => Preservation::Expendable,
        };
        T::transfer(source, dest, value, preservation).map(|_| ())
    }

    fn slash(who: &AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        if value.is_zero() {
            return (Self::NegativeImbalance::zero(), Zero::zero());
        }

        let imbalance = T::withdraw(
            who,
            value,
            Precision::BestEffort,
            Preservation::Preserve,
            Fortitude::Force,
        )
        .unwrap_or_else(|_| Self::NegativeImbalance::zero());

        let remaining = value.saturating_sub(imbalance.peek());

        (imbalance, remaining)
    }

    fn deposit_into_existing(
        who: &AccountId,
        value: Self::Balance,
    ) -> Result<Self::PositiveImbalance, DispatchError> {
        if value.is_zero() {
            return Ok(Self::PositiveImbalance::zero());
        }
        T::deposit(who, value, Precision::Exact)
    }

    fn deposit_creating(who: &AccountId, value: Self::Balance) -> Self::PositiveImbalance {
        if value.is_zero() {
            return Self::PositiveImbalance::zero();
        }
        T::deposit(who, value, Precision::Exact).unwrap_or_else(|_| Self::PositiveImbalance::zero())
    }

    fn withdraw(
        who: &AccountId,
        value: Self::Balance,
        _reasons: WithdrawReasons,
        liveness: ExistenceRequirement,
    ) -> Result<Self::NegativeImbalance, DispatchError> {
        if value.is_zero() {
            return Ok(Self::NegativeImbalance::zero());
        }

        let preservation = match liveness {
            ExistenceRequirement::KeepAlive => Preservation::Preserve,
            ExistenceRequirement::AllowDeath => Preservation::Expendable,
        };
        T::withdraw(who, value, Precision::Exact, preservation, Fortitude::Polite)
    }

    fn make_free_balance_be(
        who: &AccountId,
        balance: Self::Balance,
    ) -> SignedImbalance<Self::Balance, Self::PositiveImbalance> {
        T::set_balance(who, balance);
        SignedImbalance::Positive(Self::PositiveImbalance::zero())
    }
}

impl<T: fungible::Inspect<AccountId>> fungible::Inspect<AccountId> for CurrencyAdapter<T> {
    type Balance = T::Balance;

    fn total_issuance() -> Self::Balance {
        T::total_issuance()
    }

    fn minimum_balance() -> Self::Balance {
        T::minimum_balance()
    }

    fn total_balance(who: &AccountId) -> Self::Balance {
        T::total_balance(who)
    }

    fn balance(who: &AccountId) -> Self::Balance {
        T::balance(who)
    }

    fn reducible_balance(
        who: &AccountId,
        preservation: Preservation,
        force: Fortitude,
    ) -> Self::Balance {
        T::reducible_balance(who, preservation, force)
    }

    fn can_deposit(
        who: &AccountId,
        amount: Self::Balance,
        provenance: Provenance,
    ) -> DepositConsequence {
        T::can_deposit(who, amount, provenance)
    }

    fn can_withdraw(who: &AccountId, amount: Self::Balance) -> WithdrawConsequence<Self::Balance> {
        T::can_withdraw(who, amount)
    }
}

#[allow(dead_code)]
fn transact_with_new_gas_limit(
    transact_call: pallet_ethereum::Call<Runtime>,
) -> pallet_ethereum::Call<Runtime> {
    match transact_call {
        transact { transaction } => {
            let transaction = match transaction {
                EthereumTransaction::Legacy(tx) => EthereumTransaction::Legacy(LegacyTransaction {
                    gas_limit: GetConstantGasLimit::get(),
                    ..tx
                }),
                EthereumTransaction::EIP1559(tx) => {
                    EthereumTransaction::EIP1559(EIP1559Transaction {
                        gas_limit: GetConstantGasLimit::get(),
                        ..tx
                    })
                },
                EthereumTransaction::EIP2930(tx) => {
                    EthereumTransaction::EIP2930(EIP2930Transaction {
                        gas_limit: GetConstantGasLimit::get(),
                        ..tx
                    })
                },
            };
            pallet_ethereum::Call::new_call_variant_transact(transaction)
        },
        _ => transact_call,
    }
}
