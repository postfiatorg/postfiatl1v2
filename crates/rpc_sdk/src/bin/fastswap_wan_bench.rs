use postfiat_crypto_provider::address_from_public_key;
use postfiat_rpc_sdk::{
    derive_wallet_key_pair, drive_fastswap_three_wave, fastswap_effects_request,
    fastswap_status_request, preview_fastswap, reconcile_fastswap_replication,
    wallet_dual_sign_fastswap_intent, FastSwapRpcTransportV1, FastSwapWalletSessionV1,
    SwapSettlementModeV1, TcpFastSwapTransportV1, WalletBackupFile,
};
use postfiat_types::{
    FastAssetObjectV1, FastSwapCommitteeV1, FastSwapEffectsResponseV1, FastSwapIntentV1,
    FastSwapLocalStatusV1, FastSwapStatusResponseV1,
};
use serde::Serialize;
use sha3::{Digest, Sha3_384};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn flag(args: &[String], name: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|value| value == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|value| value == name)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn persist_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    File::open(path.parent().ok_or("evidence path has no parent")?)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn append_jsonl<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut file, value).map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;
    file.sync_data().map_err(|error| error.to_string())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(DIGITS[usize::from(byte >> 4)] as char);
        value.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    value
}

fn benchmark_nonce(run_id: &str, row: usize, prior_swap_id: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_384::new();
    hasher.update(b"postfiat.fastswap.wan_benchmark.nonce.v1");
    hasher.update(run_id.as_bytes());
    hasher.update((row as u64).to_be_bytes());
    hasher.update(prior_swap_id);
    let digest = hasher.finalize();
    let mut nonce = [0u8; 32];
    nonce.copy_from_slice(&digest[..32]);
    nonce
}

fn object_for_asset<'a>(
    objects: &'a [FastAssetObjectV1],
    asset: &postfiat_types::FastAssetIdV1,
) -> Result<&'a FastAssetObjectV1, String> {
    let matching = objects
        .iter()
        .filter(|object| &object.asset_id == asset)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(format!(
            "expected exactly one current object for asset {}, found {}",
            hex(&asset.0),
            matching.len()
        ));
    }
    Ok(matching[0])
}

fn next_intent(
    template: &FastSwapIntentV1,
    current_objects: &[FastAssetObjectV1],
    expires_at_height: u64,
    nonce: [u8; 32],
) -> Result<FastSwapIntentV1, String> {
    let mut intent = template.clone();
    intent.expires_at_height = expires_at_height;
    intent.nonce = nonce;
    let party_0_input = object_for_asset(current_objects, &intent.party_0.offered_asset_id)?;
    let party_1_input = object_for_asset(current_objects, &intent.party_1.offered_asset_id)?;
    if party_0_input.amount_atoms != intent.party_0.offered_amount
        || party_1_input.amount_atoms != intent.party_1.offered_amount
        || party_0_input.asset_rule_hash != intent.party_0.offered_asset_rule_hash
        || party_1_input.asset_rule_hash != intent.party_1.offered_asset_rule_hash
    {
        return Err("current FastSwap objects do not exactly match template economics".to_owned());
    }
    intent.party_0.owner_pubkey = party_0_input.owner_pubkey.clone();
    intent.party_0.owner_address = address_from_public_key(&party_0_input.owner_pubkey);
    intent.party_0.asset_inputs = vec![party_0_input.key];
    intent.party_1.owner_pubkey = party_1_input.owner_pubkey.clone();
    intent.party_1.owner_address = address_from_public_key(&party_1_input.owner_pubkey);
    intent.party_1.asset_inputs = vec![party_1_input.key];
    intent
        .validate_canonical_shape()
        .map_err(|error| format!("constructed non-canonical intent: {error:?}"))?;
    Ok(intent)
}

fn backup_for_owner<'a>(
    owner_pubkey: &[u8],
    backups: &'a [(Vec<u8>, WalletBackupFile)],
) -> Result<&'a WalletBackupFile, String> {
    backups
        .iter()
        .find(|(public_key, _)| public_key == owner_pubkey)
        .map(|(_, backup)| backup)
        .ok_or_else(|| "no configured wallet backup owns the current FastSwap object".to_owned())
}

fn assert_conserved(
    intent: &FastSwapIntentV1,
    created: &[FastAssetObjectV1],
) -> Result<(), String> {
    let mut expected = BTreeMap::new();
    *expected
        .entry(intent.party_0.offered_asset_id)
        .or_insert(0u64) += intent.party_0.offered_amount;
    *expected
        .entry(intent.party_1.offered_asset_id)
        .or_insert(0u64) += intent.party_1.offered_amount;
    let mut actual = BTreeMap::new();
    for object in created {
        *actual.entry(object.asset_id).or_insert(0u64) += object.amount_atoms;
    }
    if actual != expected {
        return Err("FastSwap asset totals were not atom-for-atom conserved".to_owned());
    }
    Ok(())
}

fn percentile(sorted: &[u64], numerator: usize, denominator: usize) -> u64 {
    let index = (sorted.len() * numerator)
        .div_ceil(denominator)
        .saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

#[derive(Debug, Serialize)]
struct RowEvidence {
    schema: &'static str,
    run_id: String,
    row: usize,
    cold: bool,
    swap_id: String,
    sign_ms: u64,
    preview_ms: u64,
    prepare_qc_ms: u64,
    decision_qc_ms: u64,
    effects_qc_ms: u64,
    terminal_verify_ms: u64,
    settlement_ms: u64,
    operation_ms: u64,
    lock_votes: usize,
    decision_votes: usize,
    effects_votes: usize,
    receipt_accepted: bool,
    receipt_code: String,
    exact_six: bool,
    conserved: bool,
}

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let committee: FastSwapCommitteeV1 = read_json(Path::new(&flag(&args, "--committee")?))?;
    committee
        .validate()
        .map_err(|error| format!("invalid committee: {error:?}"))?;
    let endpoints: BTreeMap<String, String> = read_json(Path::new(&flag(&args, "--endpoints")?))?;
    let template_session: FastSwapWalletSessionV1 =
        read_json(Path::new(&flag(&args, "--template-session")?))?;
    let backup_a: WalletBackupFile = read_json(Path::new(&flag(&args, "--owner-a-backup")?))?;
    let backup_b: WalletBackupFile = read_json(Path::new(&flag(&args, "--owner-b-backup")?))?;
    let operations = flag(&args, "--operations")?
        .parse::<usize>()
        .map_err(|_| "invalid --operations".to_owned())?;
    if operations < 2 {
        return Err("--operations must include at least one cold and one warm row".to_owned());
    }
    let expires_at_height = flag(&args, "--expires-at-height")?
        .parse::<u64>()
        .map_err(|_| "invalid --expires-at-height".to_owned())?;
    let policy_valid_through_height = flag(&args, "--policy-valid-through-height")?
        .parse::<u64>()
        .map_err(|_| "invalid --policy-valid-through-height".to_owned())?;
    let run_id = flag(&args, "--run-id")?;
    let evidence_dir = PathBuf::from(flag(&args, "--evidence-dir")?);
    if evidence_dir.exists() {
        return Err(format!(
            "evidence directory already exists: {}",
            evidence_dir.display()
        ));
    }
    fs::create_dir_all(&evidence_dir).map_err(|error| error.to_string())?;

    let backups = vec![
        (
            derive_wallet_key_pair(&backup_a)
                .map_err(|error| error.to_string())?
                .public_key,
            backup_a,
        ),
        (
            derive_wallet_key_pair(&backup_b)
                .map_err(|error| error.to_string())?
                .public_key,
            backup_b,
        ),
    ];
    if backups[0].0 == backups[1].0 {
        return Err("benchmark owner backups are not distinct".to_owned());
    }
    let template = template_session.signed_intent.intent.clone();
    if expires_at_height > policy_valid_through_height
        || expires_at_height < template.expires_at_height
    {
        return Err("expiry is outside the certified live policy window".to_owned());
    }
    let mut current_objects = template_session.expected_effects.created.clone();
    if current_objects.len() != 2 {
        return Err("template terminal effects must contain exactly two outputs".to_owned());
    }
    let mut prior_swap_id = template_session.expected_effects.swap_id.0.to_vec();
    let transport = TcpFastSwapTransportV1::new(endpoints, Duration::from_secs(30))?;
    let runtime_prewarm_started = Instant::now();
    if has_flag(&args, "--wire-v2") {
        transport.prewarm_fastswap_runtime_v2(&committee)?;
    }
    let runtime_prewarm_ms = runtime_prewarm_started.elapsed().as_millis() as u64;
    let rows_path = evidence_dir.join("rows.jsonl");
    let mut rows = Vec::with_capacity(operations);

    for row in 0..operations {
        let operation_started = Instant::now();
        let intent = next_intent(
            &template,
            &current_objects,
            expires_at_height,
            benchmark_nonce(&run_id, row, &prior_swap_id),
        )?;
        let owner_0 = backup_for_owner(&intent.party_0.owner_pubkey, &backups)?;
        let owner_1 = backup_for_owner(&intent.party_1.owner_pubkey, &backups)?;
        let sign_started = Instant::now();
        let signed = wallet_dual_sign_fastswap_intent(owner_0, owner_1, intent.clone())
            .map_err(|error| format!("row {row} dual signing failed: {error}"))?;
        let sign_ms = sign_started.elapsed().as_millis() as u64;
        let preview_started = Instant::now();
        let expected = preview_fastswap(&signed, &committee, &transport)
            .map_err(|error| format!("row {row} preview failed: {error:?}"))?;
        let preview_ms = preview_started.elapsed().as_millis() as u64;
        assert_conserved(&intent, &expected.created)?;
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            signed.clone(),
            expected.clone(),
        )
        .map_err(|error| format!("row {row} session failed: {error:?}"))?;
        let session_path = evidence_dir.join(format!("row-{row:03}-session.json"));
        persist_json(&session_path, &session)?;
        let settlement_started = Instant::now();
        let terminal = drive_fastswap_three_wave(&mut session, &committee, &transport, |current| {
            persist_json(&session_path, current)
        })
        .map_err(|error| format!("row {row} settlement failed: {error:?}"))?;
        let settlement_ms = settlement_started.elapsed().as_millis() as u64;
        if !terminal.effects.receipt.accepted
            || terminal.effects.receipt.code != "fastswap_applied"
            || terminal.lock_qc.votes.len() < usize::from(committee.domain.quorum)
            || terminal.decision_qc.votes.len() < usize::from(committee.domain.quorum)
            || terminal.effects_qc.votes.len() < usize::from(committee.domain.quorum)
        {
            return Err(format!("row {row} terminal correctness gate failed"));
        }
        assert_conserved(&intent, &terminal.effects.created)?;
        let replication =
            reconcile_fastswap_replication(&mut session, &committee, &transport, |current| {
                persist_json(&session_path, current)
            })
            .map_err(|error| format!("row {row} replication failed: {error:?}"))?;
        if !replication.failed.is_empty() || !replication.pending.is_empty() {
            return Err(format!(
                "row {row} exact-six replication incomplete: {replication:?}"
            ));
        }
        let swap_id = hex(&signed.swap_id().map_err(|error| format!("{error:?}"))?.0);
        let mut applied = BTreeSet::new();
        for validator in &committee.validators {
            let status = transport
                .call(
                    &validator.validator_id,
                    &fastswap_status_request(
                        format!("wan-bench-status-{row}-{}", validator.validator_id),
                        swap_id.clone(),
                    ),
                )
                .map_err(|error| format!("row {row} status {}: {error}", validator.validator_id))?
                .result_as::<FastSwapStatusResponseV1>()
                .map_err(|error| format!("row {row} status decode: {error:?}"))?;
            if status.record.as_ref().map(|record| record.status)
                != Some(FastSwapLocalStatusV1::Applied)
            {
                return Err(format!(
                    "row {row} {} is not applied",
                    validator.validator_id
                ));
            }
            let effects = transport
                .call(
                    &validator.validator_id,
                    &fastswap_effects_request(
                        format!("wan-bench-effects-{row}-{}", validator.validator_id),
                        swap_id.clone(),
                    ),
                )
                .map_err(|error| format!("row {row} effects {}: {error}", validator.validator_id))?
                .result_as::<FastSwapEffectsResponseV1>()
                .map_err(|error| format!("row {row} effects decode: {error:?}"))?;
            if effects.effects.as_ref() != Some(&expected) {
                return Err(format!(
                    "row {row} {} effects mismatch",
                    validator.validator_id
                ));
            }
            applied.insert(validator.validator_id.clone());
        }
        if applied.len() != committee.validators.len() {
            return Err(format!("row {row} exact-six audit incomplete"));
        }
        let timings = session
            .last_timings
            .clone()
            .ok_or_else(|| format!("row {row} missing stage timings"))?;
        let evidence = RowEvidence {
            schema: "postfiat-fastswap-wan-benchmark-row-v1",
            run_id: run_id.clone(),
            row,
            cold: row == 0,
            swap_id,
            sign_ms,
            preview_ms,
            prepare_qc_ms: timings.prepare_qc_ms,
            decision_qc_ms: timings.decision_qc_ms,
            effects_qc_ms: timings.effects_qc_ms,
            terminal_verify_ms: timings.terminal_verify_ms,
            settlement_ms,
            operation_ms: operation_started.elapsed().as_millis() as u64,
            lock_votes: terminal.lock_qc.votes.len(),
            decision_votes: terminal.decision_qc.votes.len(),
            effects_votes: terminal.effects_qc.votes.len(),
            receipt_accepted: true,
            receipt_code: terminal.effects.receipt.code.clone(),
            exact_six: true,
            conserved: true,
        };
        append_jsonl(&rows_path, &evidence)?;
        println!(
            "row={row} cold={} sign={}ms preview={}ms prepare={}ms decision={}ms effects={}ms settlement={}ms exact_six=true conserved=true",
            row == 0,
            sign_ms,
            preview_ms,
            timings.prepare_qc_ms,
            timings.decision_qc_ms,
            timings.effects_qc_ms,
            settlement_ms,
        );
        prior_swap_id = terminal.effects.swap_id.0.to_vec();
        current_objects = terminal.effects.created;
        rows.push(evidence);
    }

    let mut warm_settlement = rows
        .iter()
        .filter(|row| !row.cold)
        .map(|row| row.settlement_ms)
        .collect::<Vec<_>>();
    warm_settlement.sort_unstable();
    let summary = serde_json::json!({
        "schema": "postfiat-fastswap-wan-benchmark-summary-v1",
        "run_id": run_id,
        "operations": operations,
        "cold_settlement_ms": rows[0].settlement_ms,
        "wire_v2": transport.compact_payloads_enabled(),
        "runtime_prewarm_ms": runtime_prewarm_ms,
        "warm_operations": warm_settlement.len(),
        "warm_settlement_p50_ms": percentile(&warm_settlement, 50, 100),
        "warm_settlement_p95_ms": percentile(&warm_settlement, 95, 100),
        "warm_settlement_p99_ms": percentile(&warm_settlement, 99, 100),
        "accepted": rows.iter().all(|row| row.receipt_accepted && row.receipt_code == "fastswap_applied"),
        "exact_six": rows.iter().all(|row| row.exact_six),
        "conserved": rows.iter().all(|row| row.conserved),
    });
    persist_json(&evidence_dir.join("SUMMARY.json"), &summary)?;
    println!(
        "{}",
        serde_json::to_string(&summary).map_err(|error| error.to_string())?
    );
    Ok(())
}
