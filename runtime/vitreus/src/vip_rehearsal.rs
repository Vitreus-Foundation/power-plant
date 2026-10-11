//! Opt-in rehearsal using exported mainnet SCALE storage and the actual Runtime.
//! It never connects to a node or signs/submits an extrinsic. See the pinned
//! fixture's manifest for excluded state: this is not a full Executive replay.

#![cfg(feature = "mainnet-runtime")]

use super::*;
use frame_support::{
    assert_noop, assert_ok,
    storage::StoragePrefixedMap,
    traits::{tokens::nonfungibles_v2::InspectEnumerable, Currency, Hooks, StorageVersion},
};
use pallet_privileges::{
    migration::recovery::{self, Phase},
    AccrualReviewRequired, CacheRecoveryProgress, CacheRecoveryReviews, CurrentDate, VipMembers,
    VipPoints, VippMembers, VippPoints,
};
use parity_scale_codec::Encode;
use std::{collections::BTreeMap, fmt::Write, path::Path};

fn unhex(value: &str) -> Vec<u8> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    assert_eq!(value.len() % 2, 0, "Malformed snapshot hex");
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).expect("Invalid hex")
        })
        .collect()
}

fn hex(value: &[u8]) -> String {
    let mut output = String::from("0x");
    for byte in value {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn raw_prefix(prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut output = Vec::new();
    let mut cursor = prefix.to_vec();
    if let Some(value) = sp_io::storage::get(prefix) {
        output.push((cursor.clone(), value.to_vec()));
    }
    while let Some(key) = sp_io::storage::next_key(&cursor) {
        if !key.starts_with(prefix) {
            break;
        }
        output.push((key.clone(), sp_io::storage::get(&key).unwrap().to_vec()));
        cursor = key;
    }
    output
}

fn current_points() -> (BTreeMap<AccountId, Balance>, BTreeMap<AccountId, Balance>) {
    (
        VipMembers::<Runtime>::iter()
            .map(|(account, info)| (account, info.points))
            .collect(),
        VippMembers::<Runtime>::iter()
            .map(|(account, info)| (account, info.points))
            .collect(),
    )
}

const ISSUE_NAMES: [&str; 16] = [
    "stake_changed",
    "nft_cache_changed",
    "zero_value_nft",
    "no_owned_nft",
    "malformed_attribute",
    "nft_limit_exceeded",
    "inactive_nac",
    "not_a_vip",
    "missing_nac",
    "cache_limit_exceeded",
    "malformed_member",
    "underfunded",
    "malformed_nac_claim",
    "threshold_below_nfts",
    "quarantined_balance",
    "nft_owner_mismatch",
];

fn paused_operational_boundaries() -> String {
    let members: Vec<_> = VipMembers::<Runtime>::iter_keys().collect();
    let member = *members.first().unwrap();
    assert_noop!(
        Privileges::change_penalty_type(
            RuntimeOrigin::signed(member),
            pallet_privileges::PenaltyType::Flat,
        ),
        pallet_privileges::Error::<Runtime>::AccountingPaused
    );
    assert_noop!(
        Privileges::set_rewards(RuntimeOrigin::root(), 2025, 1, 1),
        pallet_privileges::Error::<Runtime>::AccountingPaused
    );
    assert_noop!(
        Privileges::force_set_vip_points(RuntimeOrigin::root(), 2025, member, 1),
        pallet_privileges::Error::<Runtime>::AccountingPaused
    );

    // Choose a real transfer passing ownership and balance checks: the failure
    // must come from maintenance, not an otherwise invalid request.
    let mut transfer = None;
    'senders: for sender in &members {
        if NacManaging::is_vipp_inactive(sender) {
            continue;
        }
        let Some((sender_claim, _)) = NacManaging::try_get_claim_balance(sender).unwrap() else {
            continue;
        };
        for (item, amount) in NacManaging::owned_vipp_claims(sender).unwrap() {
            if amount == 0
                || amount > sender_claim
                || !NacManaging::threshold_meets_vipp_requirements(sender, sender_claim - amount)
            {
                continue;
            }
            for receiver in &members {
                if sender == receiver || NacManaging::is_vipp_inactive(receiver) {
                    continue;
                }
                let Some((receiver_claim, _)) =
                    NacManaging::try_get_claim_balance(receiver).unwrap()
                else {
                    continue;
                };
                let Some(new_claim) = receiver_claim.checked_add(amount) else {
                    continue;
                };
                if NacManaging::threshold_meets_vipp_requirements(receiver, new_claim) {
                    transfer = Some((*sender, item, *receiver));
                    break 'senders;
                }
            }
        }
    }
    let (sender, item, receiver) = transfer.expect("A financially eligible VIPP transfer fixture");
    assert_noop!(
        NacManaging::transfer_vipp(RuntimeOrigin::signed(sender), item, receiver),
        pallet_privileges::Error::<Runtime>::AccountingPaused
    );

    let rewards =
        pallet_privileges::Rewards::<Runtime>::get(2024).expect("Untouched funded 2024 pool");
    let pool = Privileges::account_id();
    let pool_balance = Balances::free_balance(pool);
    let candidate = VipPoints::<Runtime>::iter_prefix(2024)
        .find_map(|(account, vip_points)| {
            let vipp_points = VippPoints::<Runtime>::get(2024, account).unwrap_or_default();
            if account == pool
                || System::account(account).providers == 0
                || vip_points == 0
                || vip_points > rewards.vip_points
                || vipp_points > rewards.vipp_points
            {
                return None;
            }
            let vip = sp_runtime::Perquintill::from_rational(vip_points, rewards.vip_points)
                * rewards.vip_rewards;
            let vipp = if vipp_points == 0 {
                0
            } else {
                sp_runtime::Perquintill::from_rational(vipp_points, rewards.vipp_points)
                    * rewards.vipp_rewards
            };
            let payout = vip + vipp;
            (payout > 0 && payout < pool_balance).then_some((account, payout))
        })
        .expect("A real funded, unclaimed 2024 entitlement");
    let (claimant, payout) = candidate;
    let claimant_before = Balances::free_balance(claimant);
    // This payout only executes in a disposable local storage transaction. The
    // rehearsal ends with the imported financial bytes unchanged.
    sp_io::storage::start_transaction();
    assert_ok!(Privileges::claim_rewards(RuntimeOrigin::signed(claimant), 2024));
    assert_eq!(Balances::free_balance(claimant), claimant_before + payout);
    assert_eq!(Balances::free_balance(pool), pool_balance - payout);
    assert!(!VipPoints::<Runtime>::contains_key(2024, claimant));
    assert!(!VippPoints::<Runtime>::contains_key(2024, claimant));
    assert!(AccrualReviewRequired::<Runtime>::get());
    sp_io::storage::rollback_transaction();
    assert_eq!(Balances::free_balance(claimant), claimant_before);
    assert_eq!(Balances::free_balance(pool), pool_balance);
    format!(
        "Paused annual funding / forced historical points / VIP penalty change: rejected\nPaused financially eligible VIPP transfer: rejected ({}, item {item}, {})\nExisting funded 2024 claim: succeeds while paused ({}; {payout} native base units), then local transaction rolled back\n",
        hex(&sender.encode()), hex(&receiver.encode()), hex(&claimant.encode()),
    )
}

/// A deliberately ignored test: supplying a fixture is mandatory and normal CI
/// must not accidentally turn a missing or synthetic snapshot into a successful rehearsal.
#[test]
#[ignore = "Set VIPP_REHEARSAL_SNAPSHOT to pinned snapshot.tsv; run with --ignored --nocapture"]
fn pinned_mainnet_cache_recovery_preserves_financial_state() {
    let snapshot_path = std::env::var("VIPP_REHEARSAL_SNAPSHOT")
        .expect("VIPP_REHEARSAL_SNAPSHOT must name the exported mainnet snapshot.tsv");
    let snapshot_path = Path::new(&snapshot_path);
    let bytes = std::fs::read(snapshot_path).expect("Read pinned raw storage fixture");
    let expected_hash = std::fs::read_to_string(snapshot_path.with_file_name("snapshot.sha256"))
        .expect("Read snapshot.sha256 written by the exporter");
    assert_eq!(unhex(expected_hash.trim()), sp_io::hashing::sha2_256(&bytes));
    let mut storage = sp_core::storage::Storage::default();
    for line in std::str::from_utf8(&bytes).unwrap().lines() {
        let (key, value) = line.split_once('\t').expect("key<TAB>value");
        assert!(storage.top.insert(unhex(key), unhex(value)).is_none(), "Duplicate key");
    }
    assert!(storage.top.len() > 1_000, "Fixture must contain real current dependencies");
    let input_key_count = storage.top.len();
    let output_dir = snapshot_path.parent().unwrap();
    sp_io::TestExternalities::new(storage).execute_with(|| {
        let block = System::block_number();
        assert!(block > 12_000_000, "Expected mainnet rather than genesis/mock state");
        assert_eq!(AccountId::default().encode().len(), 20, "Mainnet AccountId20 codec");
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(1));
        assert!(!CacheRecoveryProgress::<Runtime>::exists());
        let original_date = CurrentDate::<Runtime>::get();
        let points_before = current_points();
        assert!(!points_before.0.is_empty());
        assert_eq!(points_before.0.len(), raw_prefix(&VipMembers::<Runtime>::final_prefix()).len(),
            "Every legacy VIP row must decode in the actual Runtime");
        assert_eq!(points_before.1.len(), raw_prefix(&VippMembers::<Runtime>::final_prefix()).len(),
            "Every legacy VIPP row must decode in the actual Runtime");

        let mut protected_prefixes = vec![
            VipPoints::<Runtime>::final_prefix().to_vec(),
            VippPoints::<Runtime>::final_prefix().to_vec(),
            pallet_privileges::Rewards::<Runtime>::final_prefix().to_vec(),
            frame_system::Account::<Runtime>::final_prefix().to_vec(),
        ];
        for pallet in ["Balances", "Nfts", "NacManaging", "EnergyGeneration", "Vesting", "SimpleVesting"] {
            protected_prefixes.push(sp_io::hashing::twox_128(pallet.as_bytes()).to_vec());
        }
        let financial_before: Vec<_> = protected_prefixes.iter().map(|prefix| raw_prefix(prefix)).collect();
        let annual_vip_rows = financial_before[0].len();
        let annual_vipp_rows = financial_before[1].len();
        let funded_years = financial_before[2].len();

        let upgrade_weight = <Privileges as Hooks<BlockNumber>>::on_runtime_upgrade();
        assert!(AccrualReviewRequired::<Runtime>::get());
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(2));
        let initial = CacheRecoveryProgress::<Runtime>::get().unwrap();
        assert_eq!(initial.phase, Phase::Vip);
        assert_eq!(initial.scanned_rows, 0, "Upgrade itself must not sweep members");
        assert_eq!(current_points(), points_before);
        assert_eq!(CurrentDate::<Runtime>::get(), original_date);
        assert_noop!(Privileges::update_time(RuntimeOrigin::root(), 2027, 1, 1),
            pallet_privileges::Error::<Runtime>::AccountingPaused);
        let existing_vip = *points_before.0.keys().next().unwrap();
        assert_noop!(Privileges::exit_vip(RuntimeOrigin::signed(existing_vip)),
            pallet_privileges::Error::<Runtime>::AccountingPaused);
        assert!(Privileges::continue_cache_recovery(RuntimeOrigin::signed(existing_vip), 16).is_err());
        assert!(Privileges::continue_cache_recovery(RuntimeOrigin::root(), 17).is_err());

        let started = std::time::Instant::now();
        let mut steps = 0_u32;
        let mut largest_step = 0_u32;
        loop {
            let previous = CacheRecoveryProgress::<Runtime>::get().unwrap();
            if previous.phase == Phase::AwaitingReview { break; }
            assert_ok!(Privileges::continue_cache_recovery(RuntimeOrigin::root(), 16));
            let next = CacheRecoveryProgress::<Runtime>::get().unwrap();
            let processed = next.scanned_rows - previous.scanned_rows;
            assert!(processed <= 16, "Bounded work per governance call");
            largest_step = largest_step.max(processed);
            steps += 1;
            assert!(steps < 10_000, "Cursor must make finite progress");
            if steps == 1 {
                let checkpoint = next.encode();
                let review_checkpoint: Vec<_> = CacheRecoveryReviews::<Runtime>::iter()
                    .map(|(account, review)| (account, review.encode())).collect();
                <Privileges as Hooks<BlockNumber>>::on_runtime_upgrade();
                assert_eq!(CacheRecoveryProgress::<Runtime>::get().unwrap().encode(), checkpoint,
                    "Repeated upgrade resumes the existing cursor");
                assert_eq!(CacheRecoveryReviews::<Runtime>::iter()
                    .map(|(account, review)| (account, review.encode())).collect::<Vec<_>>(), review_checkpoint);
            }
        }
        let elapsed = started.elapsed();
        let progress = CacheRecoveryProgress::<Runtime>::get().unwrap();
        assert_eq!(progress.vip_rows as usize, points_before.0.len());
        assert_eq!(progress.vipp_rows as usize, VippMembers::<Runtime>::iter_keys().count());
        assert!(progress.pending_reviews > 0, "Known legacy differences require explicit review");
        assert!(Privileges::finish_cache_recovery(RuntimeOrigin::root()).is_err(),
            "Rehearsal must not silently approve unresolved historical decisions");
        assert!(AccrualReviewRequired::<Runtime>::get());
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(2));

        let mut csv = String::from("account,issues,issue_names,unresolved,vip_points_before,vipp_points_before,active_stake_before,active_stake_after,owned_vipp_nfts,cached_nfts_after,total_native,eligible_native,nac_claim\n");
        let mut issue_counts = [0_u32; 16];
        let mut unresolved = 0_u32;
        let mut repeat_candidate = None;
        for (account, review) in CacheRecoveryReviews::<Runtime>::iter() {
            let is_unresolved = review.issues & recovery::UNRESOLVED != 0;
            unresolved += u32::from(is_unresolved);
            let names: Vec<_> = ISSUE_NAMES.iter().enumerate().filter_map(|(bit, name)| {
                if review.issues & (1 << bit) == 0 { return None; }
                issue_counts[bit] += 1;
                Some(*name)
            }).collect();
            let active = VipMembers::<Runtime>::get(account).map(|member| member.active_stake);
            let caps = VippMembers::<Runtime>::get(account).map(|member| member.active_vipp_threshold.len()).unwrap_or_default();
            let owned = <Nfts as InspectEnumerable<AccountId>>::owned_in_collection(&VIPPCollectionId::get(), &account).count();
            let claim = NacManaging::try_get_claim_balance(&account).ok().flatten().map(|(value, _)| value);
            writeln!(csv, "{},{},{},{},{},{},{},{},{},{},{},{},{}", hex(&account.encode()), review.issues,
                names.join("|"), is_unresolved, review.vip_points.map(|v|v.to_string()).unwrap_or_default(),
                review.vipp_points.map(|v|v.to_string()).unwrap_or_default(),
                review.previous_active_stake.map(|v|v.to_string()).unwrap_or_default(),
                active.map(|v|v.to_string()).unwrap_or_default(), owned, caps,
                <Balances as Currency<AccountId>>::total_balance(&account), NacManaging::eligible_vipp_balance(&account),
                claim.map(|v|v.to_string()).unwrap_or_default()).unwrap();
            if !is_unresolved && repeat_candidate.is_none() { repeat_candidate = Some(account); }
        }
        // Replay only the existing derived-cache repair locally. No acknowledgement,
        // burn, balance correction, catch-up source approval or historical decision.
        let repeat = repeat_candidate.expect("At least one unambiguous derived-cache repair");
        let before_retry = current_points();
        assert_ok!(Privileges::retry_cache_recovery(RuntimeOrigin::root(), repeat));
        assert_eq!(current_points(), before_retry);
        assert_eq!(CacheRecoveryProgress::<Runtime>::get().unwrap().pending_reviews, progress.pending_reviews);
        let points_after = current_points();
        assert_eq!(points_after.0, points_before.0);
        for (account, points) in &points_before.1 { assert_eq!(points_after.1.get(account), Some(points)); }
        for (account, points) in &points_after.1 {
            if !points_before.1.contains_key(account) { assert_eq!(*points, 0); }
        }
        assert_eq!(CurrentDate::<Runtime>::get(), original_date);
        let operational_boundaries = paused_operational_boundaries();
        for (prefix, before) in protected_prefixes.iter().zip(&financial_before) {
            assert_eq!(&raw_prefix(prefix), before, "Protected financial state changed: {}", hex(prefix));
        }
        let mut report = format!(
            "Pinned mainnet block: {block}\nActual Runtime spec version: {}\nAccount encoding: 20 bytes\nInput raw storage keys: {input_key_count}\nVIP rows: {}\nVIPP rows after recovery: {}\nRecovery steps: {steps}\nMaximum rows in one step: {largest_step}\nRepaired current caches: {}\nPending reviews (none approved): {}\nUnresolved authoritative cases: {unresolved}\nAnnual VIP rows preserved: {annual_vip_rows}\nAnnual VIPP rows preserved: {annual_vipp_rows}\nReward pool rows preserved: {funded_years}\nCurrent point balances preserved: yes\nAll imported NFT/native balance/NAC/staking/vesting bytes preserved: yes\nCalendar anchor preserved: {original_date:?}\nStorage schema version 2 installed; cache recovery still awaits governance finish: yes\nAccrual remains paused: yes\nIdempotent local retry account: {}\nUpgrade declared weight: {upgrade_weight:?}\nNative rehearsal duration (NOT a benchmark): {elapsed:?}\n",
            VERSION.spec_version, progress.vip_rows, progress.vipp_rows, progress.repaired_accounts, progress.pending_reviews, hex(&repeat.encode()));
        for (name, count) in ISSUE_NAMES.iter().zip(issue_counts) { writeln!(report, "Issue {name}: {count}").unwrap(); }
        report.push_str(&operational_boundaries);
        println!("{report}");
        std::fs::write(output_dir.join("rehearsal-summary.txt"), report).unwrap();
        std::fs::write(output_dir.join("recovery-reviews.csv"), csv).unwrap();
    });
}

/// A local hook-budget fork, using all trie nodes from the verified CLI snapshot.
/// It does not manufacture heartbeat transactions or approve the pending recovery.
#[test]
#[ignore = "Set VIPP_REHEARSAL_FULL_SNAPSHOT to the pinned full .snap file"]
fn pinned_mainnet_session_hook_budget_while_recovery_is_paused() {
    use frame_support::dispatch::{DispatchClass, GetDispatchInfo};
    use parity_scale_codec::{Compact, DecodeAll};
    use sp_runtime::traits::Header as _;
    type CliSnapshot = (
        Compact<u16>,
        sp_core::storage::StateVersion,
        Vec<(Vec<u8>, (Vec<u8>, i32))>,
        Hash,
        Header,
    );

    let path = std::env::var("VIPP_REHEARSAL_FULL_SNAPSHOT").expect("full snapshot path");
    let path = Path::new(&path);
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        hex(&sp_io::hashing::sha2_256(&bytes)),
        "0x7258b2cb78c060bde39e045ca2df56131f12f080498953b51a87c235f5d035cb"
    );
    let (version, state_version, trie_nodes, root, header) =
        CliSnapshot::decode_all(&mut bytes.as_slice()).expect("CLI snapshot v4 SCALE layout");
    assert_eq!(version.0, 4);
    assert_eq!(root, *header.state_root());
    let mut ext = sp_io::TestExternalities::from_raw_snapshot(trie_nodes, root, state_version);
    ext.execute_with(|| {
        let session = Session::current_index();
        let validators = Session::validators();
        let queued = pallet_session::QueuedKeys::<Runtime>::get().len();
        let invulnerables = pallet_energy_generation::Invulnerables::<Runtime>::get();
        let offline: Vec<_> = validators.iter().enumerate().filter_map(|(index, account)| {
            let online = pallet_im_online::ReceivedHeartbeats::<Runtime>::contains_key(session, index as u32)
                || pallet_im_online::AuthoredBlocks::<Runtime>::get(session, account) != 0;
            (!online && !invulnerables.contains(account)).then_some(*account)
        }).collect();
        let collaborators: Vec<_> = offline.iter().map(|account| {
            pallet_energy_generation::Collaborations::<Runtime>::get(account).map(|set| set.len()).unwrap_or(0)
        }).collect();
        let now = Timestamp::get();
        let current_slot = u64::from(pallet_babe::CurrentSlot::<Runtime>::get());
        let next_slot = u64::from(pallet_babe::GenesisSlot::<Runtime>::get())
            + (pallet_babe::EpochIndex::<Runtime>::get() + 1) * EpochDuration::get();
        assert!(next_slot > current_slot);
        let next_moment = next_slot * SLOT_DURATION;
        let annual_before = (raw_prefix(&VipPoints::<Runtime>::final_prefix()),
            raw_prefix(&VippPoints::<Runtime>::final_prefix()),
            raw_prefix(&pallet_privileges::Rewards::<Runtime>::final_prefix()));
        let points_before = current_points();

        <Privileges as Hooks<BlockNumber>>::on_runtime_upgrade();
        assert!(AccrualReviewRequired::<Runtime>::get());
        let next_block = System::block_number() + 1;

        // Advance only this disposable fork to the next epoch boundary. Calling
        // rotate_session directly isolates the real session handlers from BABE
        // author selection and avoids pretending this is a consensus-valid block.
        let pre_digest = sp_consensus_babe::digests::PreDigest::SecondaryPlain(
            sp_consensus_babe::digests::SecondaryPlainPreDigest {
                authority_index: 0,
                slot: sp_consensus_babe::Slot::from(next_slot),
            },
        );
        let digest = sp_runtime::Digest {
            logs: vec![sp_runtime::DigestItem::PreRuntime(
                sp_consensus_babe::BABE_ENGINE_ID, pre_digest.encode(),
            )],
        };
        System::initialize(&next_block, &header.hash(), &digest);
        let nac_reserve = <NacManaging as Hooks<BlockNumber>>::on_initialize(next_block);
        System::register_extra_weight_unchecked(nac_reserve, DispatchClass::Mandatory);
        let babe_reserve = <Babe as Hooks<BlockNumber>>::on_initialize(next_block);
        System::register_extra_weight_unchecked(babe_reserve, DispatchClass::Mandatory);
        let before_rotation = *System::block_weight().get(DispatchClass::Mandatory);
        let started = std::time::Instant::now();
        Session::rotate_session();
        let rotation_elapsed = started.elapsed();
        assert_eq!(Session::current_index(), session + 1);
        let rotation_registered = System::block_weight().get(DispatchClass::Mandatory)
            .saturating_sub(before_rotation);

        let timestamp_call = RuntimeCall::Timestamp(pallet_timestamp::Call::set { now: next_moment });
        let timestamp_dispatch = timestamp_call.get_dispatch_info().weight
            .saturating_add(BlockWeights::get().get(DispatchClass::Mandatory).base_extrinsic);
        System::register_extra_weight_unchecked(timestamp_dispatch, DispatchClass::Mandatory);
        assert_ok!(Timestamp::set(RuntimeOrigin::none(), next_moment));
        let queued_balance_checks = pallet_nac_managing::PendingVippBalanceChanges::<Runtime>::get().len();
        <NacManaging as Hooks<BlockNumber>>::on_finalize(next_block);
        let supplemental = *System::block_weight().get(DispatchClass::Mandatory);
        // The SDK session hook itself returns the entire block ceiling on a
        // rotation. Keep this pre-existing reservation separate from the extras.
        let session_base = BlockWeights::get().max_block;
        let registered_total = session_base.saturating_add(supplemental);
        let active_day_envelope = <<Runtime as pallet_privileges::Config>::WeightInfo as
            pallet_privileges::weights::WeightInfo>::accounting(2048, 7, 1);

        assert!(AccrualReviewRequired::<Runtime>::get());
        assert_eq!(StorageVersion::get::<Privileges>(), StorageVersion::new(2));
        assert_eq!(current_points(), points_before, "Paused fork must not accrue or forfeit points");
        assert_eq!((raw_prefix(&VipPoints::<Runtime>::final_prefix()),
            raw_prefix(&VippPoints::<Runtime>::final_prefix()),
            raw_prefix(&pallet_privileges::Rewards::<Runtime>::final_prefix())), annual_before);

        let report = format!(
            "Pinned full snapshot block: {}\nSession index before rotation: {session}\nCurrent validators: {}\nQueued validators: {queued}\nCurrent slot: {current_slot}\nNext session slot: {next_slot}\nPinned timestamp milliseconds: {now}\nNext session timestamp milliseconds: {next_moment}\nSlots until next session: {}\nOffline at the pin, excluding invulnerables: {}\nTheir incoming collaboration counts: {collaborators:?}\nNAC full per-block reserve: {nac_reserve:?}\nReal session handlers extra registered weight: {rotation_registered:?}\nTimestamp declared dispatch plus base: {timestamp_dispatch:?}\nPending NAC balance checks after rotation: {queued_balance_checks}\nAll supplemental registered weight, paused scenario: {supplemental:?}\nExisting SDK full-block session reservation: {session_base:?}\nCombined registered weight including existing reservation: {registered_total:?}\nAdditional theoretical active UTC-close envelope (max2048 members/7days/1year): {active_day_envelope:?}\nCombined supplemental weight plus active-day envelope: {:?}\nNative session hook wall time (not a benchmark): {rotation_elapsed:?}\nFinancial annual/current points and reward pools preserved: yes\nNo recovery review or historical policy approved: yes\nLimitations: offline set is a pin-time scenario, not a forecast of future heartbeats; direct hook fork is not consensus-valid block production; active-day envelope is arithmetic only; declared weights are not physical runtime measurements; existing SDK session reservation already consumes the full block limit before other mandatory work.\n",
            header.number(), validators.len(), next_slot - current_slot, offline.len(),
            supplemental.saturating_add(active_day_envelope));
        println!("{report}");
        std::fs::write(path.parent().unwrap().join("session-hook-budget.txt"), report).unwrap();
    });
}
