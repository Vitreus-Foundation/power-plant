use energy_fee_runtime_api::CallRequest;
use fp_rpc::TransactionStatus;
use frame_support::{
    genesis_builder_helper::{build_state, get_preset},
    traits::{
        tokens::{
            fungible::Inspect as FungibleInspect,
            nonfungibles_v2::{Inspect, InspectEnumerable},
            Fortitude, Preservation,
        },
        Currency, ExtrinsicCall, Hooks, KeyOwnerProofSystem,
    },
    weights::{Weight, WeightToFee},
};
use pallet_energy_broker::AssetConverter;
use pallet_energy_fee::CustomFee;
use pallet_ethereum::{Call::transact, Transaction as EthereumTransaction};
use pallet_evm::{Account as EVMAccount, AddressMapping, FeeCalculator, GasWeightMapping, Runner};
use pallet_grandpa::{
    fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};
use pallet_transaction_payment::{FeeDetails, InclusionFee};
use parity_scale_codec::Encode;
use polkadot_primitives::{
    runtime_api, slashing, ApprovalVotingParams, CandidateCommitments, CandidateEvent,
    CandidateHash, CommittedCandidateReceipt, CoreIndex, CoreState, DisputeState, ExecutorParams,
    GroupRotationInfo, Id as ParaId, InboundDownwardMessage, InboundHrmpMessage, NodeFeatures,
    OccupiedCoreAssumption, PersistedValidationData, PvfCheckStatement, ScrapedOnChainVotes,
    SessionInfo, ValidationCode, ValidationCodeHash, ValidatorId, ValidatorIndex,
    ValidatorSignature, PARACHAIN_KEY_TYPE_ID,
};
use polkadot_runtime_parachains::runtime_api_impl::{
    v10 as parachains_runtime_api_impl, vstaging as vstaging_parachains_runtime_api_impl,
};
use sp_api::impl_runtime_apis;
use sp_consensus_beefy::ecdsa_crypto::AuthorityId as BeefyId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160, H256, U256};
use sp_runtime::{
    traits::{Block as BlockT, Get, NumberFor, UniqueSaturatedInto},
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, FixedU128, FixedU64, Percent, Permill,
};
use sp_staking::SessionIndex;
use sp_std::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    prelude::*,
};
use sp_version::RuntimeVersion;
use vitreus_runtime_common::{ExposureMultiplier, QuotePrice, Warehouse};
use xcm::{
    latest::prelude::AssetId as XcmAssetId, VersionedAssetId, VersionedAssets, VersionedLocation,
    VersionedXcm,
};
use xcm_runtime_apis::{
    dry_run::{CallDryRunEffects, Error as XcmDryRunApiError, XcmDryRunEffects},
    fees::Error as XcmPaymentApiError,
};

use super::{
    mmr, opaque, xcm_config, AccountId, Babe, Balance, Balances, Beefy, Block, BlockNumber,
    CollectionId, DefaultElasticity, DemocracyExtension, DynamicEnergy, EnergyBroker, EnergyFee,
    EnergyGeneration, EpochDuration, Ethereum, Executive, GetConstantEnergyFee, Grandpa, Hash,
    Historical, Index, InherentDataExt, ItemId, Mmr, MmrLeaf, NativeOrAssetId, Nfts, OriginCaller,
    Runtime, RuntimeCall, RuntimeEvent, RuntimeGenesisConfig, System, TransactionPayment,
    UncheckedExtrinsic, XcmPallet, BABE_GENESIS_EPOCH_CONFIG, LNRG, VERSION, VNRG,
};

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl xcm_runtime_apis::fees::XcmPaymentApi<Block> for Runtime {
        fn query_acceptable_payment_assets(xcm_version: xcm::Version) -> Result<Vec<VersionedAssetId>, XcmPaymentApiError> {
            let acceptable_assets = vec![XcmAssetId(xcm_config::TokenLocation::get())];
            XcmPallet::query_acceptable_payment_assets(xcm_version, acceptable_assets)
        }

        fn query_weight_to_asset_fee(weight: Weight, asset: VersionedAssetId) -> Result<u128, XcmPaymentApiError> {
            match asset.try_as::<XcmAssetId>() {
                Ok(asset_id) if asset_id.0 == xcm_config::TokenLocation::get() => {
                    // for native token
                    Ok(xcm_config::WeightToFee::weight_to_fee(&weight))
                },
                Ok(asset_id) => {
                    log::trace!(target: "xcm::xcm_runtime_api", "query_weight_to_asset_fee - unhandled asset_id: {asset_id:?}!");
                    Err(XcmPaymentApiError::AssetNotFound)
                },
                Err(_) => {
                    log::trace!(target: "xcm::xcm_runtime_api", "query_weight_to_asset_fee - failed to convert asset: {asset:?}!");
                    Err(XcmPaymentApiError::VersionedConversionFailed)
                }
            }
        }

        fn query_xcm_weight(message: VersionedXcm<()>) -> Result<Weight, XcmPaymentApiError> {
            XcmPallet::query_xcm_weight(message)
        }

        fn query_delivery_fees(destination: VersionedLocation, message: VersionedXcm<()>) -> Result<VersionedAssets, XcmPaymentApiError> {
            XcmPallet::query_delivery_fees(destination, message)
        }
    }

    impl xcm_runtime_apis::dry_run::DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller> for Runtime {
        fn dry_run_call(origin: OriginCaller, call: RuntimeCall) -> Result<CallDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
            XcmPallet::dry_run_call::<Runtime, xcm_config::XcmRouter, OriginCaller, RuntimeCall>(origin, call)
        }

        fn dry_run_xcm(origin_location: VersionedLocation, xcm: VersionedXcm<RuntimeCall>) -> Result<XcmDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
            XcmPallet::dry_run_xcm::<Runtime, xcm_config::XcmRouter, RuntimeCall, xcm_config::XcmConfig>(origin_location, xcm)
        }
    }

    impl xcm_runtime_apis::conversions::LocationToAccountApi<Block, AccountId> for Runtime {
        fn convert_location(location: VersionedLocation) -> Result<
            AccountId,
            xcm_runtime_apis::conversions::Error
        > {
            xcm_runtime_apis::conversions::LocationToAccountHelper::<
                AccountId,
                xcm_config::LocationConverter,
            >::convert_location(location)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
        fn account_nonce(account: AccountId) -> Index {
            System::account_nonce(account)
        }
    }

    impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
        fn chain_id() -> u64 {
            <Runtime as pallet_evm::Config>::ChainId::get()
        }

        fn account_basic(address: H160) -> EVMAccount {
            let (account, _) = pallet_evm::Pallet::<Runtime>::account_basic(&address);
            account
        }

        fn gas_price() -> U256 {
            let (gas_price, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
            gas_price
        }

        fn account_code_at(address: H160) -> Vec<u8> {
            pallet_evm::AccountCodes::<Runtime>::get(address)
        }

        fn author() -> H160 {
            <pallet_evm::Pallet<Runtime>>::find_author()
        }

        fn storage_at(address: H160, index: U256) -> H256 {
            let mut tmp = [0u8; 32];
            index.to_big_endian(&mut tmp);
            pallet_evm::AccountStorages::<Runtime>::get(address, H256::from_slice(&tmp[..]))
        }

        fn call(
            from: H160,
            to: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };

            let is_transactional = false;
            let validate = true;
            let evm_config = config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config());

            let mut estimated_transaction_len = data.len() +
                20 + // to
                20 + // from
                32 + // value
                32 + // gas_limit
                32 + // nonce
                1 + // TransactionAction
                8 + // chain id
                65; // signature

            if max_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if max_priority_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if access_list.is_some() {
                estimated_transaction_len += access_list.encoded_size();
            }

            let gas_limit = gas_limit.min(u64::MAX.into()).low_u64();
            let without_base_extrinsic_weight = true;

            let (weight_limit, proof_size_base_cost) =
                match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    gas_limit,
                    without_base_extrinsic_weight
                ) {
                    weight_limit if weight_limit.proof_size() > 0 => {
                        (Some(weight_limit), Some(estimated_transaction_len as u64))
                    }
                    _ => (None, None),
                };

            <Runtime as pallet_evm::Config>::Runner::call(
                from,
                to,
                data,
                value,
                gas_limit.unique_saturated_into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                is_transactional,
                validate,
                weight_limit,
                proof_size_base_cost,
                evm_config,
            ).map_err(|err| err.error.into())
        }

        fn create(
            from: H160,
            data: Vec<u8>,
            value: U256,
            gas_limit: U256,
            max_fee_per_gas: Option<U256>,
            max_priority_fee_per_gas: Option<U256>,
            nonce: Option<U256>,
            estimate: bool,
            access_list: Option<Vec<(H160, Vec<H256>)>>,
        ) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
            let config = if estimate {
                let mut config = <Runtime as pallet_evm::Config>::config().clone();
                config.estimate = true;
                Some(config)
            } else {
                None
            };

            let is_transactional = false;
            let validate = true;
            let evm_config = config.as_ref().unwrap_or(<Runtime as pallet_evm::Config>::config());

            let mut estimated_transaction_len = data.len() +
                20 + // from
                32 + // value
                32 + // gas_limit
                32 + // nonce
                1 + // TransactionAction
                8 + // chain id
                65; // signature

            if max_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if max_priority_fee_per_gas.is_some() {
                estimated_transaction_len += 32;
            }
            if access_list.is_some() {
                estimated_transaction_len += access_list.encoded_size();
            }

            let gas_limit = if gas_limit > U256::from(u64::MAX) {
                u64::MAX
            } else {
                gas_limit.low_u64()
            };
            let without_base_extrinsic_weight = true;

            let (weight_limit, proof_size_base_cost) =
                match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
                    gas_limit,
                    without_base_extrinsic_weight
                ) {
                    weight_limit if weight_limit.proof_size() > 0 => {
                        (Some(weight_limit), Some(estimated_transaction_len as u64))
                    }
                    _ => (None, None),
                };

            <Runtime as pallet_evm::Config>::Runner::create(
                from,
                data,
                value,
                gas_limit.unique_saturated_into(),
                max_fee_per_gas,
                max_priority_fee_per_gas,
                nonce,
                access_list.unwrap_or_default(),
                is_transactional,
                validate,
                weight_limit,
                proof_size_base_cost,
                evm_config,
            ).map_err(|err| err.error.into())
        }

        fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
            pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
        }

        fn current_block() -> Option<pallet_ethereum::Block> {
            pallet_ethereum::CurrentBlock::<Runtime>::get()
        }

        fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
            pallet_ethereum::CurrentReceipts::<Runtime>::get()
        }

        fn current_all() -> (
            Option<pallet_ethereum::Block>,
            Option<Vec<pallet_ethereum::Receipt>>,
            Option<Vec<TransactionStatus>>
        ) {
            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentReceipts::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
            )
        }

        fn extrinsic_filter(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> Vec<EthereumTransaction> {
            xts.into_iter().filter_map(|xt| match xt.0.function {
                RuntimeCall::Ethereum(transact { transaction }) => Some(transaction),
                _ => None
            }).collect::<Vec<EthereumTransaction>>()
        }

        fn elasticity() -> Option<Permill> {
            Some(DefaultElasticity::get())
        }

        fn gas_limit_multiplier_support() {}

        fn pending_block(
            xts: Vec<<Block as BlockT>::Extrinsic>,
        ) -> (Option<pallet_ethereum::Block>, Option<Vec<TransactionStatus>>) {
            for ext in xts.into_iter() {
                let _ = Executive::apply_extrinsic(ext);
            }

            Ethereum::on_finalize(System::block_number() + 1);

            (
                pallet_ethereum::CurrentBlock::<Runtime>::get(),
                pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
            )
        }

        fn initialize_pending_block(header: &<Block as BlockT>::Header) {
            let _ = Executive::initialize_block(header);
        }
    }

    impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
        fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
            UncheckedExtrinsic::new_unsigned(
                pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
            )
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
        Block,
        Balance,
    > for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            let fee = EnergyFee::dispatch_info_to_fee(uxt.call(), None, None);
            let mut runtime_dispatch_info = TransactionPayment::query_info(uxt, len);

            runtime_dispatch_info.partial_fee = fee.into_inner();
            runtime_dispatch_info
        }

        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> FeeDetails<Balance> {
            let fee = EnergyFee::dispatch_info_to_fee(uxt.call(), None, None).into_inner();
            let fee_details = TransactionPayment::query_fee_details(uxt, len);

            match fee_details {
                FeeDetails {
                    inclusion_fee: Some(InclusionFee { base_fee, len_fee, .. }),
                    tip
                } => FeeDetails {
                    inclusion_fee: Some(InclusionFee{
                        base_fee,
                        len_fee,
                        adjusted_weight_fee: fee,
                    }),
                    tip
                },
                fee_details => fee_details
            }

        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_beefy_mmr::BeefyMmrApi<Block, Hash> for RuntimeApi {
        fn authority_set_proof() -> sp_consensus_beefy::mmr::BeefyAuthoritySet<Hash> {
            MmrLeaf::authority_set_proof()
        }

        fn next_authority_set_proof() -> sp_consensus_beefy::mmr::BeefyNextAuthoritySet<Hash> {
            MmrLeaf::next_authority_set_proof()
        }
    }

    impl pallet_nfts_runtime_api::NftsApi<Block, AccountId, CollectionId, ItemId> for Runtime {
        fn owner(collection: CollectionId, item: ItemId) -> Option<AccountId> {
            <Nfts as Inspect<AccountId>>::owner(&collection, &item)
        }

        fn collection_owner(collection: CollectionId) -> Option<AccountId> {
            <Nfts as Inspect<AccountId>>::collection_owner(&collection)
        }

        fn attribute(
            collection: CollectionId,
            item: ItemId,
            key: Vec<u8>,
        ) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::attribute(&collection, &item, &key)
        }

        fn custom_attribute(
            account: AccountId,
            collection: CollectionId,
            item: ItemId,
            key: Vec<u8>,
        ) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::custom_attribute(
                &account,
                &collection,
                &item,
                &key,
            )
        }

        fn system_attribute(
            collection: CollectionId,
            item: Option<ItemId>,
            key: Vec<u8>,
        ) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::system_attribute(&collection, item.as_ref(), &key)
        }

        fn collection_attribute(collection: CollectionId, key: Vec<u8>) -> Option<Vec<u8>> {
            <Nfts as Inspect<AccountId>>::collection_attribute(&collection, &key)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            let epoch_config = Babe::epoch_config().unwrap_or(BABE_GENESIS_EPOCH_CONFIG);
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(),
                epoch_length: EpochDuration::get(),
                c: epoch_config.c,
                authorities: Babe::authorities().to_vec(),
                randomness: Babe::randomness(),
                allowed_slots: epoch_config.allowed_slots,
            }
        }

        fn current_epoch_start() -> sp_consensus_babe::Slot {
            Babe::current_epoch_start()
        }

        fn current_epoch() -> sp_consensus_babe::Epoch {
            Babe::current_epoch()
        }

        fn next_epoch() -> sp_consensus_babe::Epoch {
            Babe::next_epoch()
        }

        fn generate_key_ownership_proof(
            _slot: sp_consensus_babe::Slot,
            authority_id: sp_consensus_babe::AuthorityId,
        ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {

            Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
            key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Babe::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> fg_primitives::SetId {
            Grandpa::current_set_id()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: fg_primitives::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Grandpa::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: fg_primitives::SetId,
            authority_id: GrandpaId,
        ) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
            Historical::prove((fg_primitives::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(fg_primitives::OpaqueKeyOwnershipProof::new)
        }
    }

    impl dynamic_energy_runtime_api::DynamicEnergyApi<Block, Balance> for Runtime {
        fn exchange_rate() -> Option<FixedU128> {
            DynamicEnergy::exchange_rate()
        }

        fn calculate_warehouse_capacity_multiplier() -> FixedU128 {
            DynamicEnergy::calculate_warehouse_capacity_multiplier()
        }


        fn generation_rate_parameters() -> dynamic_energy_runtime_api::GenerationRateParameters<Balance> {
            DynamicEnergy::generation_rate_parameters()
        }

        fn exchange_rate_parameters() -> dynamic_energy_runtime_api::ExchangeRateParameters<Balance> {
            DynamicEnergy::exchange_rate_parameters()
        }
    }

    impl energy_broker_runtime_api::EnergyBrokerApi<Block, AccountId, NativeOrAssetId, Balance> for Runtime {
        fn estimate_energy_from_native(amount: Balance) -> Option<Balance> {
            Self::quote_price_exact_tokens_for_tokens(
                None,
                NativeOrAssetId::Native,
                NativeOrAssetId::WithId(VNRG::get()),
                amount,
                true,
            )
        }

        fn estimate_native_from_energy(amount: Balance) -> Option<Balance> {
            Self::quote_price_exact_tokens_for_tokens(
                None,
                NativeOrAssetId::WithId(LNRG::get()),
                NativeOrAssetId::Native,
                amount,
                true,
            )
        }

        fn quote_price_exact_tokens_for_tokens(
            _who: Option<AccountId>,
            asset1: NativeOrAssetId,
            asset2: NativeOrAssetId,
            amount: Balance,
            include_fee: bool,
        ) -> Option<Balance> {
            EnergyBroker::quote_price_exact_tokens_for_tokens(asset1, asset2, amount, include_fee)
        }

        fn quote_price_tokens_for_exact_tokens(
            _who: Option<AccountId>,
            asset1: NativeOrAssetId,
            asset2: NativeOrAssetId,
            amount: Balance,
            include_fee: bool,
        ) -> Option<Balance> {
            EnergyBroker::quote_price_tokens_for_exact_tokens(asset1, asset2, amount, include_fee)
        }

        fn energy_exchange_rate() -> Option<FixedU128> {
            DynamicEnergy::exchange_rate()
        }

        fn current_warehouse_level() -> Percent {
            Percent::from_rational(EnergyBroker::current_amount(), EnergyBroker::max_capacity())
        }

        fn paths() -> Vec<(NativeOrAssetId, NativeOrAssetId)> {
            <Runtime as pallet_energy_broker::Config>::AssetConverter::paths()
        }
    }

    impl energy_fee_runtime_api::EnergyFeeApi<Block, AccountId, Balance, RuntimeCall> for Runtime {
        fn estimate_gas(request: CallRequest) -> U256 {
            let CallRequest {
                from,
                to,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                gas,
                value,
                data,
                nonce,
                access_list,
                ..
            } = request;
            let call = match data {
                Some(data) => {
                    let from = from.unwrap_or_default();
                    let to = to.unwrap_or_default();
                    let value = value.unwrap_or_else(U256::zero);
                    let gas_limit = gas.unwrap_or_else(|| U256::from(21000)).low_u64(); // default gas limit to 21000
                    let max_fee_per_gas = max_fee_per_gas.unwrap_or_else(U256::zero);
                    let access_list = access_list.unwrap_or_default();
                    let access_list_converted = access_list.into_iter()
                        .map(|item| (item.address, item.storage_keys))
                        .collect();

                    RuntimeCall::EVM(pallet_evm::Call::call {
                        source: from,
                        target: to,
                        input: data.into_inner(),
                        value,
                        gas_limit,
                        max_fee_per_gas,
                        max_priority_fee_per_gas,
                        nonce,
                        access_list: access_list_converted,
                    })
                },
                None => {
                    match (from, to, value) {
                        (_, Some(to), Some(value)) => {
                            let value_converted = Balance::from(value.low_u128());  // Adjust this conversion as necessary

                            RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
                                dest: to.into(),
                                value: value_converted,
                            })
                        },
                        _ => return GetConstantEnergyFee::get().into(),
                    }
                }
            };

            EnergyFee::dispatch_info_to_fee(&call, None, None).into_inner().into()
        }

        fn estimate_call_fee(account: AccountId, call: RuntimeCall) -> Option<energy_fee_runtime_api::FeeDetails<Balance>> {
            let fee = EnergyFee::dispatch_info_to_fee(&call, None, None).into_inner();
            EnergyFee::calculate_fee_parts(&account, fee).map(|fees| energy_fee_runtime_api::FeeDetails {
                vtrs: fees.1,
                vnrg: fees.0,
            })
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use pallet_treasury_extension::Pallet as PalletTreasuryExtension;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);
            list_benchmark!(list, extra, pallet_treasury_extension, PalletTreasuryExtension::<Runtime>);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};
            use pallet_treasury_extension::Pallet as PalletTreasuryExtension;
            impl frame_system_benchmarking::Config for Runtime {}

            let whitelist: Vec<TrackedStorageKey> = vec![];

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);

            add_benchmark!(params, batches, pallet_treasury_extension, PalletTreasuryExtension::<Runtime>);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }

    impl vitreus_utility_runtime_api::UtilityApi<Block> for Runtime {
        fn balance(who: H160) -> U256 {
            let account_id = <Self as pallet_evm::Config>::AddressMapping::into_account_id(who);
            Balances::reducible_balance(&account_id, Preservation::Preserve, Fortitude::Polite).into()
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            vec![]
        }
    }

    impl energy_generation_runtime_api::EnergyGenerationApi<Block, AccountId> for Runtime {
        fn energy_reward_per_stake() -> FixedU128 {
            EnergyGeneration::active_era()
                .and_then(|era| era.index.checked_sub(1))
                .and_then(EnergyGeneration::eras_energy_per_stake_currency)
                .unwrap_or_default()
        }

        fn validator_exposure_multiplier(account: AccountId) -> FixedU64 {
            <Self as pallet_energy_generation::Config>::ValidatorExposureMultiplier::multiplier(&account)
        }

        fn cooperator_exposure_multiplier(account: AccountId) -> FixedU64 {
            <Self as pallet_energy_generation::Config>::CooperatorExposureMultiplier::multiplier(&account)
        }
    }

    impl governance_runtime_api::GovernanceApi<Block, Balance> for Runtime {
        fn electorate() -> Balance {
            <Self as pallet_democracy::Config>::Currency::total_issuance()
        }

        fn threshold(referendum_index: u32) -> Option<Percent> {
            DemocracyExtension::threshold(referendum_index)
        }
    }

    impl nfts_runtime_api::NftsAuxApi<Block, AccountId, CollectionId, ItemId> for Runtime {
        fn owned(account: AccountId) -> Vec<(CollectionId, ItemId)> {
            <Nfts as InspectEnumerable<AccountId>>::owned(&account).collect()
        }

        fn level(account: AccountId, collection: CollectionId) -> Option<Vec<u8>> {
            <Nfts as InspectEnumerable<AccountId>>::owned_in_collection(&collection, &account)
                .next()
                .and_then(|item| <Nfts as Inspect<AccountId>>::system_attribute(&collection, Some(&item), &[0, 0, 1]))
        }
    }

    #[api_version(11)]
    impl runtime_api::ParachainHost<Block> for Runtime {
        fn validators() -> Vec<ValidatorId> {
            parachains_runtime_api_impl::validators::<Runtime>()
        }

        fn validator_groups() -> (Vec<Vec<ValidatorIndex>>, GroupRotationInfo<BlockNumber>) {
            parachains_runtime_api_impl::validator_groups::<Runtime>()
        }

        fn availability_cores() -> Vec<CoreState<Hash, BlockNumber>> {
            parachains_runtime_api_impl::availability_cores::<Runtime>()
        }

        fn persisted_validation_data(para_id: ParaId, assumption: OccupiedCoreAssumption)
            -> Option<PersistedValidationData<Hash, BlockNumber>> {
            parachains_runtime_api_impl::persisted_validation_data::<Runtime>(para_id, assumption)
        }

        fn assumed_validation_data(
            para_id: ParaId,
            expected_persisted_validation_data_hash: Hash,
        ) -> Option<(PersistedValidationData<Hash, BlockNumber>, ValidationCodeHash)> {
            parachains_runtime_api_impl::assumed_validation_data::<Runtime>(
                para_id,
                expected_persisted_validation_data_hash,
            )
        }

        fn check_validation_outputs(
            para_id: ParaId,
            outputs: CandidateCommitments,
        ) -> bool {
            parachains_runtime_api_impl::check_validation_outputs::<Runtime>(para_id, outputs)
        }

        fn session_index_for_child() -> SessionIndex {
            parachains_runtime_api_impl::session_index_for_child::<Runtime>()
        }

        fn validation_code(para_id: ParaId, assumption: OccupiedCoreAssumption)
            -> Option<ValidationCode> {
            parachains_runtime_api_impl::validation_code::<Runtime>(para_id, assumption)
        }

        fn candidate_pending_availability(para_id: ParaId) -> Option<CommittedCandidateReceipt<Hash>> {
            #[allow(deprecated)]
            parachains_runtime_api_impl::candidate_pending_availability::<Runtime>(para_id)
        }

        fn candidate_events() -> Vec<CandidateEvent<Hash>> {
            parachains_runtime_api_impl::candidate_events::<Runtime, _>(|ev| {
                match ev {
                    RuntimeEvent::ParaInclusion(ev) => {
                        Some(ev)
                    }
                    _ => None,
                }
            })
        }

        fn session_info(index: SessionIndex) -> Option<SessionInfo> {
            parachains_runtime_api_impl::session_info::<Runtime>(index)
        }

        fn session_executor_params(session_index: SessionIndex) -> Option<ExecutorParams> {
            parachains_runtime_api_impl::session_executor_params::<Runtime>(session_index)
        }

        fn dmq_contents(recipient: ParaId) -> Vec<InboundDownwardMessage<BlockNumber>> {
            parachains_runtime_api_impl::dmq_contents::<Runtime>(recipient)
        }

        fn inbound_hrmp_channels_contents(
            recipient: ParaId
        ) -> BTreeMap<ParaId, Vec<InboundHrmpMessage<BlockNumber>>> {
            parachains_runtime_api_impl::inbound_hrmp_channels_contents::<Runtime>(recipient)
        }

        fn validation_code_by_hash(hash: ValidationCodeHash) -> Option<ValidationCode> {
            parachains_runtime_api_impl::validation_code_by_hash::<Runtime>(hash)
        }

        fn on_chain_votes() -> Option<ScrapedOnChainVotes<Hash>> {
            parachains_runtime_api_impl::on_chain_votes::<Runtime>()
        }

        fn submit_pvf_check_statement(
            stmt: PvfCheckStatement,
            signature: ValidatorSignature,
        ) {
            parachains_runtime_api_impl::submit_pvf_check_statement::<Runtime>(stmt, signature)
        }

        fn pvfs_require_precheck() -> Vec<ValidationCodeHash> {
            parachains_runtime_api_impl::pvfs_require_precheck::<Runtime>()
        }

        fn validation_code_hash(para_id: ParaId, assumption: OccupiedCoreAssumption)
            -> Option<ValidationCodeHash>
        {
            parachains_runtime_api_impl::validation_code_hash::<Runtime>(para_id, assumption)
        }

        fn disputes() -> Vec<(SessionIndex, CandidateHash, DisputeState<BlockNumber>)> {
            parachains_runtime_api_impl::get_session_disputes::<Runtime>()
        }

        fn unapplied_slashes(
        ) -> Vec<(SessionIndex, CandidateHash, slashing::PendingSlashes)> {
            parachains_runtime_api_impl::unapplied_slashes::<Runtime>()
        }

        fn key_ownership_proof(
            validator_id: ValidatorId,
        ) -> Option<slashing::OpaqueKeyOwnershipProof> {
            Historical::prove((PARACHAIN_KEY_TYPE_ID, validator_id))
                .map(|p| p.encode())
                .map(slashing::OpaqueKeyOwnershipProof::new)
        }

        fn submit_report_dispute_lost(
            dispute_proof: slashing::DisputeProof,
            key_ownership_proof: slashing::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            parachains_runtime_api_impl::submit_unsigned_slashing_report::<Runtime>(
                dispute_proof,
                key_ownership_proof,
            )
        }

        fn minimum_backing_votes() -> u32 {
            parachains_runtime_api_impl::minimum_backing_votes::<Runtime>()
        }

        fn para_backing_state(para_id: ParaId) -> Option<polkadot_primitives::async_backing::BackingState> {
            parachains_runtime_api_impl::backing_state::<Runtime>(para_id)
        }

        fn async_backing_params() -> polkadot_primitives::AsyncBackingParams {
            parachains_runtime_api_impl::async_backing_params::<Runtime>()
        }

        fn approval_voting_params() -> ApprovalVotingParams {
            parachains_runtime_api_impl::approval_voting_params::<Runtime>()
        }

        fn disabled_validators() -> Vec<ValidatorIndex> {
            parachains_runtime_api_impl::disabled_validators::<Runtime>()
        }

        fn node_features() -> NodeFeatures {
            parachains_runtime_api_impl::node_features::<Runtime>()
        }

        fn claim_queue() -> BTreeMap<CoreIndex, VecDeque<ParaId>> {
            vstaging_parachains_runtime_api_impl::claim_queue::<Runtime>()
        }

        fn candidates_pending_availability(para_id: ParaId) -> Vec<CommittedCandidateReceipt<Hash>> {
            vstaging_parachains_runtime_api_impl::candidates_pending_availability::<Runtime>(para_id)
        }
    }

    impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
        fn authorities() -> Vec<sp_authority_discovery::AuthorityId> {
            parachains_runtime_api_impl::relevant_authority_ids::<Runtime>()
        }
    }

    #[api_version(4)]
    impl sp_consensus_beefy::BeefyApi<Block, BeefyId> for Runtime {
        fn beefy_genesis() -> Option<BlockNumber> {
            pallet_beefy::GenesisBlock::<Runtime>::get()
        }

        fn validator_set() -> Option<sp_consensus_beefy::ValidatorSet<BeefyId>> {
            Beefy::validator_set()
        }

        fn submit_report_double_voting_unsigned_extrinsic(
            equivocation_proof: sp_consensus_beefy::DoubleVotingProof<
                BlockNumber,
                BeefyId,
                sp_consensus_beefy::ecdsa_crypto::Signature,
            >,
            key_owner_proof: sp_consensus_beefy::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Beefy::submit_unsigned_double_voting_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: sp_consensus_beefy::ValidatorSetId,
            authority_id: BeefyId,
        ) -> Option<sp_consensus_beefy::OpaqueKeyOwnershipProof> {
            Historical::prove((sp_consensus_beefy::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_consensus_beefy::OpaqueKeyOwnershipProof::new)
        }
    }

    #[api_version(2)]
    impl sp_mmr_primitives::MmrApi<Block, Hash, BlockNumber> for Runtime {
        fn mmr_root() -> Result<Hash, sp_mmr_primitives::Error> {
            Ok(Mmr::mmr_root())
        }

        fn mmr_leaf_count() -> Result<sp_mmr_primitives::LeafIndex, sp_mmr_primitives::Error> {
            Ok(Mmr::mmr_leaves())
        }

        fn generate_proof(
            block_numbers: Vec<BlockNumber>,
            best_known_block_number: Option<BlockNumber>,
        ) -> Result<(Vec<sp_mmr_primitives::EncodableOpaqueLeaf>, sp_mmr_primitives::LeafProof<Hash>), sp_mmr_primitives::Error> {
             Mmr::generate_proof(block_numbers, best_known_block_number).map(
                |(leaves, proof)| {
                    (
                        leaves
                            .into_iter()
                            .map(|leaf| mmr::EncodableOpaqueLeaf::from_leaf(&leaf))
                            .collect(),
                        proof,
                    )
                },
            )
        }

        fn verify_proof(leaves: Vec<sp_mmr_primitives::EncodableOpaqueLeaf>, proof: sp_mmr_primitives::LeafProof<Hash>)
            -> Result<(), sp_mmr_primitives::Error>
        {
             let leaves = leaves.into_iter().map(|leaf|
                leaf.into_opaque_leaf()
                .try_decode()
                .ok_or(mmr::Error::Verify)).collect::<Result<Vec<mmr::Leaf>, mmr::Error>>()?;
            Mmr::verify_leaves(leaves, proof)
        }

        fn verify_proof_stateless(
            root: Hash,
            leaves: Vec<sp_mmr_primitives::EncodableOpaqueLeaf>,
            proof: sp_mmr_primitives::LeafProof<Hash>
        ) -> Result<(), sp_mmr_primitives::Error> {
            let nodes = leaves.into_iter().map(|leaf|mmr::DataOrHash::Data(leaf.into_opaque_leaf())).collect();
            pallet_mmr::verify_leaves_proof::<mmr::Hashing, _>(root, nodes, proof)
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            log::info!("try-runtime::on_runtime_upgrade");
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, super::BlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect,
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
        }
    }
}
