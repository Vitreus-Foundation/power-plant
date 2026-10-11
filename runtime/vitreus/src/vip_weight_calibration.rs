//! Release-only checks against retained local compiled-WASM benchmark evidence.
use super::*;
use frame_support::traits::Hooks;
use pallet_nac_managing::WeightInfo as _;
use pallet_privileges::WeightInfo as _;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};

type NacWeights = <Runtime as pallet_nac_managing::Config>::WeightInfo;
type VipWeights = <Runtime as pallet_privileges::Config>::WeightInfo;

fn declared_weight(case: &Value) -> Weight {
    let component = |name: &str| case["components"][name].as_u64().unwrap_or_default() as u32;
    match (case["pallet"].as_str().unwrap(), case["benchmark"].as_str().unwrap()) {
        ("pallet_nac_managing", "mint") => NacWeights::mint(),
        ("pallet_nac_managing", "update_nft") => NacWeights::update_nft(),
        ("pallet_nac_managing", "check_nac_level") => NacWeights::check_nac_level(),
        ("pallet_nac_managing", "check_vipp_threshold") => {
            NacWeights::check_vipp_threshold(component("n"))
        },
        ("pallet_nac_managing", "transfer_vipp") => NacWeights::transfer_vipp(component("n")),
        ("pallet_nac_managing", "transfer_new_recipient") => NacWeights::transfer_vipp(64),
        (
            "pallet_nac_managing",
            "observe_new_account"
            | "observe_malformed_claim"
            | "observe_balance_change"
            | "observe_queued_balance_change",
        ) => NacWeights::balance_observer(),
        ("pallet_privileges", "become_vip_status" | "become_vip_new_member") => {
            VipWeights::become_vip_status()
        },
        ("pallet_privileges", "exit_vip" | "exit_vip_cooperator") => VipWeights::exit_vip(),
        ("pallet_privileges", "refresh_active_stake") => VipWeights::refresh_active_stake(),
        ("pallet_privileges", "change_penalty_type") => VipWeights::change_penalty_type(),
        ("pallet_privileges", "force_set_vip_points") => VipWeights::force_set_vip_points(),
        ("pallet_privileges", "force_set_vipp_points") => VipWeights::force_set_vipp_points(),
        ("pallet_privileges", "set_rewards") => VipWeights::set_rewards(),
        ("pallet_privileges", "claim_rewards") => VipWeights::claim_rewards(),
        // FRAME m creates m VIP members AND m VIPP members.
        ("pallet_privileges", "accounting") => {
            VipWeights::accounting(2 * component("m"), component("d"), 1)
        },
        ("pallet_privileges", "accounting_snapshots") => VipWeights::accounting(2048, 7, 1),
        ("pallet_privileges", "catch_up_vip" | "catch_up_vipp") => {
            VipWeights::accounting(64, 366, 2)
        },
        ("pallet_privileges", "resolve_timestamp_breaches") => {
            VipWeights::resolve_timestamp_breaches()
        },
        ("pallet_privileges", "recovery") => VipWeights::recovery(component("a"), 64),
        other => panic!("Unmapped benchmark {other:?}"),
    }
}

#[test]
#[ignore = "requires retained local compiled-WASM benchmark artifacts"]
fn vip_actual_runtime_weights_cover_every_measured_sample() {
    let directory =
        std::env::var_os("VIPP_WEIGHT_REPORT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../investigations/vip-vipp-release-20261006")
            });
    let evidence: Value = serde_json::from_slice(
        &std::fs::read(directory.join("weight-validation-cases.json")).unwrap(),
    )
    .unwrap();
    let db = <Runtime as frame_system::Config>::DbWeight::get();
    let mut rows = Vec::new();
    let mut samples = 0;
    for case in evidence["cases"].as_array().unwrap() {
        let weight = declared_weight(case);
        let database =
            db.reads_writes(case["reads"].as_u64().unwrap(), case["writes"].as_u64().unwrap());
        let mut maximum_required = 0;
        for sample in case["observed_time_ns"].as_array().unwrap() {
            // FRAME exports nanoseconds; Weight ref_time is picoseconds.
            let required = sample
                .as_u64()
                .unwrap()
                .saturating_mul(2_000)
                .saturating_add(database.ref_time());
            assert!(
                weight.ref_time() >= required,
                "CPU/DB underweight: {case}; actual weight {weight:?}; required {required}"
            );
            maximum_required = maximum_required.max(required);
            samples += 1;
        }
        let proof = case["frame_estimated_proof_bytes"]
            .as_u64()
            .unwrap()
            .max(case["maximum_raw_proof_bytes"].as_u64().unwrap());
        assert!(
            weight.proof_size() >= proof,
            "Proof underweight: {case}; actual weight {weight:?}; required {proof}"
        );
        rows.push(json!({
            "pallet": case["pallet"], "benchmark": case["benchmark"], "components": case["components"],
            "declared_ref_time_ps": weight.ref_time(), "required_twice_cpu_plus_db_ps": maximum_required,
            "declared_proof_bytes": weight.proof_size(), "required_proof_bytes": proof,
            "samples": case["observed_time_ns"].as_array().unwrap().len(),
        }));
    }
    let finalizer_reserve = sp_io::TestExternalities::default()
        .execute_with(|| <NacManaging as Hooks<BlockNumber>>::on_initialize(1));
    let mut budgets = BTreeMap::new();
    budgets.insert("native_account_observation", NacWeights::balance_observer());
    budgets.insert("nac_finalization_reserve", finalizer_reserve);
    budgets.insert("daily_accounting_2048_7_1", VipWeights::accounting(2048, 7, 1));
    budgets.insert("timestamp_breach_queue_1024", VipWeights::resolve_timestamp_breaches());
    budgets.insert("recovery_16_64", VipWeights::recovery(16, 64));
    budgets.insert("catchup_64_366_2", VipWeights::accounting(64, 366, 2));
    budgets.insert("vip_exit_maximum", VipWeights::exit_vip());
    let mandatory = finalizer_reserve
        .saturating_add(VipWeights::accounting(2048, 7, 1))
        .saturating_add(VipWeights::resolve_timestamp_breaches())
        .saturating_add(db.reads_writes(6, 1));
    budgets.insert("vip_specific_mandatory_maximum", mandatory);
    let block_limit = BlockWeights::get().max_block;
    assert!(mandatory.all_lte(block_limit), "VIP hooks exceed block limit");
    assert!(
        mandatory.saturating_add(VipWeights::recovery(16, 64)).all_lte(block_limit),
        "Recovery cannot fit after VIP hooks"
    );
    let budget_json: BTreeMap<_, _> = budgets
        .into_iter()
        .map(|(name, weight)| {
            (name, json!({"ref_time_ps":weight.ref_time(), "proof_bytes":weight.proof_size()}))
        })
        .collect();
    let result = json!({
        "status":"passed", "source_sha256":evidence["source_sha256"], "validated_time_samples":samples,
        "cases":rows, "actual_runtime_budget":budget_json,
        "block_limit":{"ref_time_ps":block_limit.ref_time(),"proof_bytes":block_limit.proof_size()},
        "scope":"Actual configured Runtime weights; VIP-specific hook maximum excludes unrelated pallet hooks. Local measurement baseline, not a production hardware certification.",
    });
    std::fs::write(
        directory.join("runtime-weight-validation.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("Verified {samples} raw timings against actual Runtime CPU/DB/proof envelopes");
}
