use polkadot_primitives::ValidatorId;
use polkadot_runtime_parachains::configuration::ActiveConfigHrmpChannelSizeAndCapacityRatio;
use sp_runtime::Percent;

use super::*;

impl parachains_origin::Config for Runtime {}

impl parachains_configuration::Config for Runtime {
    type WeightInfo = weights::runtime_parachains_configuration::WeightInfo<Runtime>;
}

impl parachains_shared::Config for Runtime {
    type DisabledValidators = Session;
}

impl parachains_session_info::Config for Runtime {
    type ValidatorSet = Historical;
}

impl parachains_inclusion::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DisputesHandler = ParasDisputes;
    type RewardValidators = RewardValidators;
    type MessageQueue = MessageQueue;
    type WeightInfo = weights::runtime_parachains_inclusion::WeightInfo<Runtime>;
}

parameter_types! {
    pub const ParasUnsignedPriority: TransactionPriority = TransactionPriority::MAX;
}

impl parachains_paras::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type UnsignedPriority = ParasUnsignedPriority;
    type NextSessionRotation = Babe;
    type QueueFootprinter = ParaInclusion;
    type OnNewHead = Registrar;
    type WeightInfo = weights::runtime_parachains_paras::WeightInfo<Runtime>;
    type AssignCoretime = ();
}

impl parachains_dmp::Config for Runtime {}

parameter_types! {
    pub const HrmpChannelSizeAndCapacityWithSystemRatio: Percent = Percent::from_percent(100);
}

impl parachains_hrmp::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type ChannelManager = EnsureRoot<AccountId>;
    type Currency = Balances;
    // Use the `HrmpChannelSizeAndCapacityWithSystemRatio` ratio from the actual active
    // `HostConfiguration` configuration for `hrmp_channel_max_message_size` and
    // `hrmp_channel_max_capacity`.
    type DefaultChannelSizeAndCapacityWithSystem = ActiveConfigHrmpChannelSizeAndCapacityRatio<
        Runtime,
        HrmpChannelSizeAndCapacityWithSystemRatio,
    >;
    type VersionWrapper = XcmPallet;
    type WeightInfo = weights::runtime_parachains_hrmp::WeightInfo<Self>;
}

impl parachains_paras_inherent::Config for Runtime {
    type WeightInfo = weights::runtime_parachains_paras_inherent::WeightInfo<Runtime>;
}

impl parachains_scheduler::Config for Runtime {
    type AssignmentProvider = ParaAssignmentProvider;
}

impl parachains_assigner_parachains::Config for Runtime {}

impl parachains_initializer::Config for Runtime {
    type Randomness = pallet_babe::RandomnessFromOneEpochAgo<Runtime>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type CoretimeOnNewSession = ();
    type WeightInfo = weights::runtime_parachains_initializer::WeightInfo<Runtime>;
}

impl parachains_disputes::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RewardValidators = ();
    type SlashingHandler = parachains_slashing::SlashValidatorsForDisputes<ParasSlashing>;
    type WeightInfo = weights::runtime_parachains_disputes::WeightInfo<Runtime>;
}

impl parachains_slashing::Config for Runtime {
    type KeyOwnerProofSystem = Historical;
    type KeyOwnerProof =
        <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, ValidatorId)>>::Proof;
    type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        ValidatorId,
    )>>::IdentificationTuple;
    type HandleReports = parachains_slashing::SlashingReportHandler<
        Self::KeyOwnerIdentification,
        Offences,
        ReportLongevity,
    >;
    type WeightInfo = weights::runtime_parachains_disputes_slashing::WeightInfo<Runtime>;
    type BenchmarkingConfig = parachains_slashing::BenchConfig<1000>;
}

parameter_types! {
    pub const ParaDeposit: Balance = prod_or_fast!(20_000 * UNITS, 1_000 * UNITS);
    pub const ParaDataByteDeposit: Balance = 2;
}

impl paras_registrar::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type Currency = Balances;
    type OnSwap = Slots;
    type ParaDeposit = ParaDeposit;
    type DataDepositPerByte = ParaDataByteDeposit;
    type WeightInfo = weights::runtime_common_paras_registrar::WeightInfo<Runtime>;
}

parameter_types! {
    pub LeasePeriod: BlockNumber = prod_or_fast!(4 * WEEKS, 1 * WEEKS, "VITREUS_LEASE_PERIOD");
}

impl slots::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type Registrar = Registrar;
    type LeasePeriod = LeasePeriod;
    type LeaseOffset = ();
    type ForceOrigin = EnsureRoot<Self::AccountId>;
    type WeightInfo = weights::runtime_common_slots::WeightInfo<Runtime>;
}

impl paras_sudo_wrapper::Config for Runtime {}

parameter_types! {
    // The average auction is 7 days long, so this will be 70% for ending period.
    // 5 Days = 72000 Blocks @ 6 sec per block
    pub const EndingPeriod: BlockNumber = prod_or_fast!(5 * DAYS, 2 * HOURS);
    // ~ 1000 samples per day -> ~ 20 blocks per sample -> 2 minute samples
    pub const SampleLength: BlockNumber = 2 * MINUTES;
}

impl auctions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Leaser = Slots;
    type Registrar = Registrar;
    type EndingPeriod = EndingPeriod;
    type SampleLength = SampleLength;
    type Randomness = pallet_babe::RandomnessFromOneEpochAgo<Runtime>;
    type InitiateOrigin = MoreThanHalfCouncil;
    type WeightInfo = weights::runtime_common_auctions::WeightInfo<Runtime>;
}
