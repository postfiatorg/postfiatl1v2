use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Instant;

use postfiat_consensus_cobalt::{build_canonical_unl_trust_graph, CobaltDomain, TrustGraph};
use postfiat_crypto_provider::{
    bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_sign_with_context,
    ml_dsa_65_verify_with_context,
};
use postfiat_node::cobalt_shadow::{
    build_signed_protocol_transcript_for_graph_extending, CobaltShadowHistoryEntry,
    CobaltShadowHistoryRange, CobaltShadowIdentity, CobaltShadowLimits,
    CobaltShadowProtocolTranscript, CobaltShadowService, CobaltShadowSignedHistoryRange,
    COBALT_SHADOW_SIGNED_HISTORY_RANGE_SCHEMA,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const MANIFEST_SCHEMA: &str = "postfiat-cobalt-adversarial-e3-campaign-manifest-v1";
const REPORT_SCHEMA: &str = "postfiat-cobalt-adversarial-e3-campaign-v1";
const SUMMARY_SCHEMA: &str = "postfiat-cobalt-adversarial-e3-summary-v1";
const SIGNED_EVIDENCE_SCHEMA: &str = "postfiat-cobalt-adversarial-e3-signed-evidence-v1";
const HISTORY_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/cobalt-shadow/history-range/v1";
const HISTORY_FILE: &str = "protocol-history.jsonl";
const PRIVATE_FILE: &str = "signer-private.json";
const STATE_FILE: &str = "state.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvidenceSource {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LiveBinding {
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    registry_root: String,
    recorded_trust_graph_root: String,
    clone_graph_version: u64,
    clone_activation_height: u64,
    clone_previous_trust_graph_root: Option<String>,
    validators: Vec<String>,
    quorum: usize,
    activation_source: EvidenceSource,
    state_source: EvidenceSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignManifest {
    schema: String,
    campaign_id: String,
    frozen_at: String,
    live_binding: LiveBinding,
    source_files: Vec<EvidenceSource>,
    history_entry_count: usize,
    tamper_cases: Vec<String>,
    forged_catch_up_cases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceAudit {
    live_registry_root: String,
    recorded_live_trust_graph_root: String,
    clone_trust_graph_root: String,
    live_registry_root_exact_match: bool,
    activation_source_sha256: String,
    state_source_sha256: String,
    source_files_verified: usize,
    validator_count: usize,
    quorum: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedEvidence {
    schema: String,
    attack: String,
    sender: String,
    sender_public_key_hex: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    range_hash: String,
    statement_hash: String,
    signature_hex: String,
    signature_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaseResult {
    case_id: String,
    validator: String,
    category: String,
    attack: String,
    expected_reason_code: String,
    rejection_reason: String,
    detected_before_rejoin: bool,
    durable_state_mutated: bool,
    journal_sha256_before: String,
    journal_sha256_after: String,
    state_hash_before: String,
    state_hash_after: String,
    signed_evidence: Option<SignedEvidence>,
    elapsed_micros: u64,
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecoveryResult {
    validator: String,
    first_peer: String,
    second_peer: String,
    interrupted_after_sequence: u64,
    final_sequence: u64,
    final_registry_root: String,
    final_trust_graph_root: String,
    honest_history_sha256: String,
    restored_history_sha256: String,
    byte_identical: bool,
    restart_succeeded: bool,
    no_manual_repair: bool,
    elapsed_micros: u64,
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct ClassificationRow {
    case_id: String,
    category: String,
    attack: String,
    expected_reason_code: String,
    detected_before_rejoin: bool,
    durable_state_mutated: bool,
    signed_evidence_verified: bool,
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct RecoveryClassification {
    validator: String,
    interrupted_after_sequence: u64,
    final_sequence: u64,
    byte_identical: bool,
    restart_succeeded: bool,
    no_manual_repair: bool,
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CampaignSummary {
    schema: String,
    manifest_sha256: String,
    source_revision: String,
    validator_count: usize,
    tamper_case_count: usize,
    forged_catch_up_case_count: usize,
    recovery_case_count: usize,
    rejected_case_count: usize,
    durable_mutation_count: usize,
    signed_evidence_count: usize,
    signed_evidence_verified: bool,
    byte_identical_recovery_count: usize,
    manual_repair_action_count: usize,
    classification_sha256: String,
    summary_only: bool,
    pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignReport {
    schema: String,
    summary: CampaignSummary,
    source_audit: SourceAudit,
    cases: Vec<CaseResult>,
    recoveries: Vec<RecoveryResult>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn sha256_file(path: &Path) -> io::Result<String> {
    fs::read(path).map(|bytes| sha256_bytes(&bytes))
}

fn hash_serialized<T: Serialize>(domain: &str, value: &T) -> io::Result<String> {
    serde_json::to_vec(value)
        .map(|bytes| hash_hex(domain, &bytes))
        .map_err(io::Error::other)
}

fn history_entry_hash(entry: &CobaltShadowHistoryEntry) -> io::Result<String> {
    hash_serialized(
        "postfiat.cobalt.shadow.history.entry.v1",
        &(
            &entry.schema,
            entry.sequence,
            entry.round,
            &entry.parent_entry_hash,
            &entry.transcript_hash,
            &entry.decision,
            &entry.transcript,
        ),
    )
}

fn history_range_hash(range: &CobaltShadowHistoryRange) -> io::Result<String> {
    hash_serialized(
        "postfiat.cobalt.shadow.history.range.v1",
        &(
            &range.schema,
            &range.registry_root,
            &range.trust_graph_root,
            range.start_sequence,
            range.end_sequence,
            &range.entries,
        ),
    )
}

fn signed_statement_hash(signed: &CobaltShadowSignedHistoryRange) -> io::Result<String> {
    hash_serialized(
        "postfiat.cobalt.shadow.signed-history-range.v1",
        &(
            &signed.schema,
            &signed.sender,
            &signed.chain_id,
            &signed.genesis_hash,
            signed.protocol_version,
            &signed.range.range_hash,
        ),
    )
}

fn reseal_range(range: &mut CobaltShadowHistoryRange, changed_index: usize) -> io::Result<()> {
    for index in changed_index..range.entries.len() {
        if index > 0 {
            range.entries[index].parent_entry_hash = range.entries[index - 1].entry_hash.clone();
        }
        let transcript_hash = hash_serialized(
            "postfiat.cobalt.shadow.protocol-transcript.v1",
            &range.entries[index].transcript,
        )?;
        range.entries[index].transcript_hash = transcript_hash.clone();
        range.entries[index].decision.transcript_hash = transcript_hash;
        range.entries[index].entry_hash = history_entry_hash(&range.entries[index])?;
    }
    range.range_hash = history_range_hash(range)?;
    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        return Err(invalid(format!(
            "refusing to overwrite clone {}",
            destination.display()
        )));
    }
    fs::create_dir_all(destination)?;
    fs::set_permissions(destination, fs::metadata(source)?.permissions())?;
    for item in fs::read_dir(source)? {
        let item = item?;
        let target = destination.join(item.file_name());
        if item.file_type()?.is_dir() {
            copy_dir(&item.path(), &target)?;
        } else {
            fs::copy(item.path(), &target)?;
            fs::set_permissions(&target, item.metadata()?.permissions())?;
        }
    }
    Ok(())
}

fn read_private_key(data_dir: &Path) -> io::Result<Vec<u8>> {
    let value: Value = serde_json::from_slice(&fs::read(data_dir.join(PRIVATE_FILE))?)
        .map_err(io::Error::other)?;
    let private_key = value
        .get("private_key_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("temporary clone signer has no private key"))?;
    hex_to_bytes(private_key).map_err(io::Error::other)
}

fn sign_forged_range(
    source: &CobaltShadowService,
    mut signed: CobaltShadowSignedHistoryRange,
) -> io::Result<CobaltShadowSignedHistoryRange> {
    signed.statement_hash = signed_statement_hash(&signed)?;
    let private_key = read_private_key(source.data_dir())?;
    signed.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &private_key,
            signed.statement_hash.as_bytes(),
            HISTORY_SIGNATURE_CONTEXT,
        )
        .map_err(io::Error::other)?,
    );
    Ok(signed)
}

fn signed_evidence(
    attack: &str,
    signed: &CobaltShadowSignedHistoryRange,
    public_key_hex: &str,
) -> io::Result<SignedEvidence> {
    let public_key = hex_to_bytes(public_key_hex).map_err(io::Error::other)?;
    let signature = hex_to_bytes(&signed.signature_hex).map_err(io::Error::other)?;
    let expected_statement = signed_statement_hash(signed)?;
    let signature_verified = expected_statement == signed.statement_hash
        && ml_dsa_65_verify_with_context(
            &public_key,
            signed.statement_hash.as_bytes(),
            &signature,
            HISTORY_SIGNATURE_CONTEXT,
        );
    Ok(SignedEvidence {
        schema: SIGNED_EVIDENCE_SCHEMA.to_string(),
        attack: attack.to_string(),
        sender: signed.sender.clone(),
        sender_public_key_hex: public_key_hex.to_string(),
        chain_id: signed.chain_id.clone(),
        genesis_hash: signed.genesis_hash.clone(),
        protocol_version: signed.protocol_version,
        range_hash: signed.range.range_hash.clone(),
        statement_hash: signed.statement_hash.clone(),
        signature_hex: signed.signature_hex.clone(),
        signature_verified,
    })
}

fn verify_evidence(evidence: &SignedEvidence) -> io::Result<bool> {
    if evidence.schema != SIGNED_EVIDENCE_SCHEMA {
        return Ok(false);
    }
    let statement_hash = hash_serialized(
        "postfiat.cobalt.shadow.signed-history-range.v1",
        &(
            COBALT_SHADOW_SIGNED_HISTORY_RANGE_SCHEMA,
            &evidence.sender,
            &evidence.chain_id,
            &evidence.genesis_hash,
            evidence.protocol_version,
            &evidence.range_hash,
        ),
    )?;
    let public_key = hex_to_bytes(&evidence.sender_public_key_hex).map_err(io::Error::other)?;
    let signature = hex_to_bytes(&evidence.signature_hex).map_err(io::Error::other)?;
    Ok(statement_hash == evidence.statement_hash
        && evidence.signature_verified
        && ml_dsa_65_verify_with_context(
            &public_key,
            evidence.statement_hash.as_bytes(),
            &signature,
            HISTORY_SIGNATURE_CONTEXT,
        ))
}

fn mutate_history(path: &Path, attack: &str) -> io::Result<()> {
    let mut bytes = fs::read(path)?;
    match attack {
        "truncated" => {
            bytes
                .pop()
                .ok_or_else(|| invalid("cannot truncate empty history"))?;
        }
        "padded" => bytes.extend_from_slice(b"{}\n"),
        "reordered" => {
            let mut lines = bytes
                .split_inclusive(|byte| *byte == b'\n')
                .map(Vec::from)
                .collect::<Vec<_>>();
            if lines.len() < 2 {
                return Err(invalid("reorder needs at least two history entries"));
            }
            lines.swap(0, 1);
            bytes = lines.concat();
        }
        "one_entry_modified" => {
            let mut lines = bytes
                .split_inclusive(|byte| *byte == b'\n')
                .map(Vec::from)
                .collect::<Vec<_>>();
            let line = lines
                .get_mut(1)
                .ok_or_else(|| invalid("modification needs two history entries"))?;
            let mut entry: Value = serde_json::from_slice(line).map_err(io::Error::other)?;
            let round = entry
                .get("round")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid("history entry has no round"))?;
            entry["round"] = Value::from(round.saturating_add(1));
            *line = serde_json::to_vec(&entry).map_err(io::Error::other)?;
            line.push(b'\n');
            bytes = lines.concat();
        }
        _ => return Err(invalid(format!("unknown durable tamper {attack}"))),
    }
    fs::write(path, bytes)
}

fn expected_tamper_reason(attack: &str) -> &'static str {
    match attack {
        "truncated" => "journal_truncated",
        "padded" => "journal_record_invalid",
        "reordered" | "one_entry_modified" => "journal_chain_mismatch",
        _ => "unknown",
    }
}

fn tamper_reason_matches(attack: &str, error: &str) -> bool {
    match attack {
        "truncated" => error.contains("truncated"),
        "padded" => error.contains("missing field"),
        "reordered" | "one_entry_modified" => {
            error.contains("persisted protocol history chain mismatch")
        }
        _ => false,
    }
}

fn run_tamper_case(
    work_root: &Path,
    full_dir: &Path,
    validator: &str,
    attack: &str,
) -> io::Result<CaseResult> {
    let started = Instant::now();
    let case_id = format!("{validator}-durable-{attack}");
    let clone = work_root.join("cases").join(&case_id);
    copy_dir(full_dir, &clone)?;
    let journal = clone.join(HISTORY_FILE);
    mutate_history(&journal, attack)?;
    let journal_before = sha256_file(&journal)?;
    let state_before = sha256_file(&clone.join(STATE_FILE))?;
    let error = CobaltShadowService::open(&clone)
        .err()
        .ok_or_else(|| invalid(format!("{case_id} unexpectedly rejoined")))?
        .to_string();
    let journal_after = sha256_file(&journal)?;
    let state_after = sha256_file(&clone.join(STATE_FILE))?;
    let durable_state_mutated = journal_before != journal_after || state_before != state_after;
    let detected = tamper_reason_matches(attack, &error);
    Ok(CaseResult {
        case_id,
        validator: validator.to_string(),
        category: "durable_restart".to_string(),
        attack: attack.to_string(),
        expected_reason_code: expected_tamper_reason(attack).to_string(),
        rejection_reason: error,
        detected_before_rejoin: detected,
        durable_state_mutated,
        journal_sha256_before: journal_before,
        journal_sha256_after: journal_after,
        state_hash_before: state_before,
        state_hash_after: state_after,
        signed_evidence: None,
        elapsed_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
        ok: detected && !durable_state_mutated,
    })
}

fn run_forged_case(
    work_root: &Path,
    partial_dir: &Path,
    target_validator: &str,
    source: &CobaltShadowService,
    latest: &CobaltShadowProtocolTranscript,
    attack: &str,
) -> io::Result<CaseResult> {
    let started = Instant::now();
    let case_id = format!("{target_validator}-catch-up-{attack}");
    let clone = work_root.join("cases").join(&case_id);
    copy_dir(partial_dir, &clone)?;
    let mut target = CobaltShadowService::open(&clone)?;
    let journal_before = sha256_file(&clone.join(HISTORY_FILE))?;
    let state_before = target.state().state_hash.clone();

    let signed = if attack == "omitted_latest_update" {
        let gap = target
            .commit_protocol_transcript(latest)
            .expect_err("latest transcript must announce a gap");
        if !gap.to_string().contains("catch_up_required") {
            return Err(invalid(
                "latest transcript did not create a named catch-up gap",
            ));
        }
        source.signed_history_range(2, 1)?
    } else {
        let mut forged = source.signed_history_range(2, 2)?;
        match attack {
            "fabricated_transition" => {
                forged.range.entries[0]
                    .transcript
                    .ratification
                    .activation_height = forged.range.entries[0]
                    .transcript
                    .ratification
                    .activation_height
                    .saturating_add(17);
            }
            "wrong_root_certificate" => {
                forged.range.entries[0]
                    .transcript
                    .ratification
                    .registry_root = "ff".repeat(48);
            }
            _ => return Err(invalid(format!("unknown forged catch-up case {attack}"))),
        }
        reseal_range(&mut forged.range, 0)?;
        sign_forged_range(source, forged)?
    };

    let evidence = signed_evidence(attack, &signed, &source.state().public_key_hex)?;
    let error = target
        .catch_up_signed_history(&signed)
        .err()
        .ok_or_else(|| invalid(format!("{case_id} unexpectedly accepted")))?
        .to_string();
    let journal_after = sha256_file(&clone.join(HISTORY_FILE))?;
    let state_after = target.state().state_hash.clone();
    let durable_state_mutated = journal_before != journal_after || state_before != state_after;
    let expected_reason_code = match attack {
        "fabricated_transition" => "transition_proof_mismatch",
        "wrong_root_certificate" => "certificate_root_mismatch",
        "omitted_latest_update" => "required_latest_update_omitted",
        _ => "unknown",
    };
    let named_reason = match attack {
        "fabricated_transition" => error.contains("Cobalt protocol validation failed"),
        "wrong_root_certificate" => error.contains("Cobalt protocol validation failed"),
        "omitted_latest_update" => error.contains("omits required latest update"),
        _ => false,
    };
    let ok = named_reason && !durable_state_mutated && evidence.signature_verified;
    Ok(CaseResult {
        case_id,
        validator: target_validator.to_string(),
        category: "forged_catch_up".to_string(),
        attack: attack.to_string(),
        expected_reason_code: expected_reason_code.to_string(),
        rejection_reason: error,
        detected_before_rejoin: named_reason,
        durable_state_mutated,
        journal_sha256_before: journal_before,
        journal_sha256_after: journal_after,
        state_hash_before: state_before,
        state_hash_after: state_after,
        signed_evidence: Some(evidence),
        elapsed_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
        ok,
    })
}

fn run_recovery_case(
    work_root: &Path,
    partial_dir: &Path,
    validator: &str,
    first_peer: &CobaltShadowService,
    second_peer: &CobaltShadowService,
    honest_history_sha256: &str,
    graph: &TrustGraph,
) -> io::Result<RecoveryResult> {
    let started = Instant::now();
    let clone = work_root
        .join("recoveries")
        .join(format!("{validator}-interrupted"));
    copy_dir(partial_dir, &clone)?;
    let mut target = CobaltShadowService::open(&clone)?;
    let first = first_peer.signed_history_range(2, 1)?;
    let after_first = target.catch_up_signed_history(&first)?;
    if after_first.contiguous_sequence != 2 {
        return Err(invalid("first recovery peer did not advance to sequence 2"));
    }
    drop(target);

    let mut restarted = CobaltShadowService::open(&clone)?;
    let second = second_peer.signed_history_range(3, 2)?;
    let final_status = restarted.catch_up_signed_history(&second)?;
    drop(restarted);
    let inspected = CobaltShadowService::inspect(&clone)?;
    let restored_history_sha256 = sha256_file(&clone.join(HISTORY_FILE))?;
    let byte_identical = restored_history_sha256 == honest_history_sha256;
    let restart_succeeded = inspected.contiguous_sequence == 4
        && inspected.history_head == final_status.history_head
        && inspected.governance_digest == final_status.governance_digest;
    let roots_match = inspected.registry_root == graph.registry_root
        && inspected.trust_graph_root == graph.trust_graph_root;
    let ok = byte_identical && restart_succeeded && roots_match;
    Ok(RecoveryResult {
        validator: validator.to_string(),
        first_peer: first_peer.state().identity.node_id.clone(),
        second_peer: second_peer.state().identity.node_id.clone(),
        interrupted_after_sequence: 2,
        final_sequence: inspected.contiguous_sequence,
        final_registry_root: inspected.registry_root,
        final_trust_graph_root: inspected.trust_graph_root,
        honest_history_sha256: honest_history_sha256.to_string(),
        restored_history_sha256,
        byte_identical,
        restart_succeeded,
        no_manual_repair: true,
        elapsed_micros: started.elapsed().as_micros().try_into().unwrap_or(u64::MAX),
        ok,
    })
}

fn source_audit(manifest: &CampaignManifest) -> io::Result<(SourceAudit, TrustGraph)> {
    let binding = &manifest.live_binding;
    let domain = CobaltDomain {
        chain_id: binding.chain_id.clone(),
        genesis_hash: binding.genesis_hash.clone(),
        protocol_version: binding.protocol_version,
    };
    let graph = build_canonical_unl_trust_graph(
        &domain,
        binding.clone_graph_version,
        binding.registry_root.clone(),
        binding.clone_activation_height,
        binding.clone_previous_trust_graph_root.clone(),
        binding.validators.clone(),
        binding.quorum,
    )
    .map_err(invalid)?;
    let activation_source_sha256 = sha256_file(Path::new(&binding.activation_source.path))?;
    let state_source_sha256 = sha256_file(Path::new(&binding.state_source.path))?;
    if activation_source_sha256 != binding.activation_source.sha256
        || state_source_sha256 != binding.state_source.sha256
    {
        return Err(invalid("live binding source hash mismatch"));
    }
    for source in &manifest.source_files {
        if sha256_file(Path::new(&source.path))? != source.sha256 {
            return Err(invalid(format!("source hash mismatch: {}", source.path)));
        }
    }
    let registry_exact = graph.registry_root == binding.registry_root;
    if !registry_exact {
        return Err(invalid(
            "disposable clone is not bound to the live registry root",
        ));
    }
    Ok((
        SourceAudit {
            live_registry_root: binding.registry_root.clone(),
            recorded_live_trust_graph_root: binding.recorded_trust_graph_root.clone(),
            clone_trust_graph_root: graph.trust_graph_root.clone(),
            live_registry_root_exact_match: registry_exact,
            activation_source_sha256,
            state_source_sha256,
            source_files_verified: manifest.source_files.len(),
            validator_count: binding.validators.len(),
            quorum: binding.quorum,
        },
        graph,
    ))
}

fn classification_hash(cases: &[CaseResult], recoveries: &[RecoveryResult]) -> io::Result<String> {
    let mut case_rows = cases
        .iter()
        .map(|case| ClassificationRow {
            case_id: case.case_id.clone(),
            category: case.category.clone(),
            attack: case.attack.clone(),
            expected_reason_code: case.expected_reason_code.clone(),
            detected_before_rejoin: case.detected_before_rejoin,
            durable_state_mutated: case.durable_state_mutated,
            signed_evidence_verified: case
                .signed_evidence
                .as_ref()
                .is_none_or(|evidence| evidence.signature_verified),
            ok: case.ok,
        })
        .collect::<Vec<_>>();
    case_rows.sort();
    let mut recovery_rows = recoveries
        .iter()
        .map(|recovery| RecoveryClassification {
            validator: recovery.validator.clone(),
            interrupted_after_sequence: recovery.interrupted_after_sequence,
            final_sequence: recovery.final_sequence,
            byte_identical: recovery.byte_identical,
            restart_succeeded: recovery.restart_succeeded,
            no_manual_repair: recovery.no_manual_repair,
            ok: recovery.ok,
        })
        .collect::<Vec<_>>();
    recovery_rows.sort();
    let encoded = serde_json::to_vec(&(case_rows, recovery_rows)).map_err(io::Error::other)?;
    Ok(sha256_bytes(&encoded))
}

fn run_campaign(
    manifest_path: &Path,
    output_path: &Path,
    work_root: &Path,
    source_revision: String,
    summary_only: bool,
) -> io::Result<()> {
    let manifest_bytes = fs::read(manifest_path)?;
    let manifest: CampaignManifest =
        serde_json::from_slice(&manifest_bytes).map_err(io::Error::other)?;
    if manifest.schema != MANIFEST_SCHEMA
        || manifest.live_binding.validators.len() != 6
        || manifest.history_entry_count != 4
    {
        return Err(invalid("unsupported E3 campaign manifest"));
    }
    let unique_validators = manifest
        .live_binding
        .validators
        .iter()
        .collect::<BTreeSet<_>>();
    if unique_validators.len() != 6 {
        return Err(invalid("E3 validators are not unique"));
    }
    if work_root.exists() {
        return Err(invalid("E3 work root already exists"));
    }
    fs::create_dir_all(work_root)?;

    let result = (|| {
        let (source_audit, graph) = source_audit(&manifest)?;
        let base_root = work_root.join("fleet");
        fs::create_dir_all(&base_root)?;
        let mut fleet = manifest
            .live_binding
            .validators
            .iter()
            .map(|validator| {
                CobaltShadowService::initialize(
                    base_root.join(validator),
                    CobaltShadowIdentity {
                        node_id: validator.clone(),
                        chain_id: manifest.live_binding.chain_id.clone(),
                        genesis_hash: manifest.live_binding.genesis_hash.clone(),
                        protocol_version: manifest.live_binding.protocol_version,
                    },
                    CobaltShadowLimits::default(),
                )
            })
            .collect::<io::Result<Vec<_>>>()?;

        let partial_root = work_root.join("partial");
        fs::create_dir_all(&partial_root)?;
        let mut transcripts = Vec::new();
        for index in 0..manifest.history_entry_count {
            let round = 1001 + index as u64;
            let payload_hash = hash_hex(
                "postfiat.cobalt.adversarial.e3.payload.v1",
                format!("entry-{index}").as_bytes(),
            );
            let transcript = build_signed_protocol_transcript_for_graph_extending(
                &mut fleet,
                &graph,
                round,
                payload_hash,
                transcripts
                    .last()
                    .map(|previous: &CobaltShadowProtocolTranscript| &previous.ratification),
            )?;
            for service in &mut fleet {
                service.commit_protocol_transcript(&transcript)?;
            }
            transcripts.push(transcript);
            if index == 0 {
                for validator in &manifest.live_binding.validators {
                    copy_dir(&base_root.join(validator), &partial_root.join(validator))?;
                }
            }
        }

        let honest_history = fs::read(
            base_root
                .join(&manifest.live_binding.validators[0])
                .join(HISTORY_FILE),
        )?;
        for validator in &manifest.live_binding.validators {
            if fs::read(base_root.join(validator).join(HISTORY_FILE))? != honest_history {
                return Err(invalid("honest validator histories are not byte-identical"));
            }
        }
        let honest_history_sha256 = sha256_bytes(&honest_history);
        let latest = transcripts
            .last()
            .ok_or_else(|| invalid("E3 generated no transcripts"))?;

        let mut cases = Vec::new();
        for validator in &manifest.live_binding.validators {
            for attack in &manifest.tamper_cases {
                cases.push(run_tamper_case(
                    work_root,
                    &base_root.join(validator),
                    validator,
                    attack,
                )?);
            }
            let target_index = manifest
                .live_binding
                .validators
                .iter()
                .position(|candidate| candidate == validator)
                .ok_or_else(|| invalid("target validator missing"))?;
            let source_index = (target_index + 1) % fleet.len();
            for attack in &manifest.forged_catch_up_cases {
                cases.push(run_forged_case(
                    work_root,
                    &partial_root.join(validator),
                    validator,
                    &fleet[source_index],
                    latest,
                    attack,
                )?);
            }
        }

        let mut recoveries = Vec::new();
        for (target_index, validator) in manifest.live_binding.validators.iter().enumerate() {
            let first_peer = &fleet[(target_index + 1) % fleet.len()];
            let second_peer = &fleet[(target_index + 2) % fleet.len()];
            recoveries.push(run_recovery_case(
                work_root,
                &partial_root.join(validator),
                validator,
                first_peer,
                second_peer,
                &honest_history_sha256,
                &graph,
            )?);
        }

        let tamper_case_count = cases
            .iter()
            .filter(|case| case.category == "durable_restart")
            .count();
        let forged_catch_up_case_count = cases
            .iter()
            .filter(|case| case.category == "forged_catch_up")
            .count();
        let rejected_case_count = cases
            .iter()
            .filter(|case| case.detected_before_rejoin)
            .count();
        let durable_mutation_count = cases
            .iter()
            .filter(|case| case.durable_state_mutated)
            .count();
        let evidence = cases
            .iter()
            .filter_map(|case| case.signed_evidence.as_ref())
            .collect::<Vec<_>>();
        let signed_evidence_verified = evidence.iter().all(|item| item.signature_verified);
        let byte_identical_recovery_count = recoveries
            .iter()
            .filter(|recovery| recovery.byte_identical)
            .count();
        let pass = cases.iter().all(|case| case.ok)
            && recoveries.iter().all(|recovery| recovery.ok)
            && tamper_case_count == 24
            && forged_catch_up_case_count == 18
            && evidence.len() == 18
            && durable_mutation_count == 0
            && byte_identical_recovery_count == 6;
        let summary = CampaignSummary {
            schema: SUMMARY_SCHEMA.to_string(),
            manifest_sha256: sha256_bytes(&manifest_bytes),
            source_revision,
            validator_count: manifest.live_binding.validators.len(),
            tamper_case_count,
            forged_catch_up_case_count,
            recovery_case_count: recoveries.len(),
            rejected_case_count,
            durable_mutation_count,
            signed_evidence_count: evidence.len(),
            signed_evidence_verified,
            byte_identical_recovery_count,
            manual_repair_action_count: 0,
            classification_sha256: classification_hash(&cases, &recoveries)?,
            summary_only,
            pass,
        };
        let report = CampaignReport {
            schema: REPORT_SCHEMA.to_string(),
            summary,
            source_audit,
            cases: if summary_only { Vec::new() } else { cases },
            recoveries: if summary_only { Vec::new() } else { recoveries },
        };
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            output_path,
            serde_json::to_vec_pretty(&report).map_err(io::Error::other)?,
        )?;
        if !report.summary.pass {
            return Err(invalid("E3 campaign gate failed"));
        }
        Ok(())
    })();

    let cleanup = fs::remove_dir_all(work_root);
    result.and(cleanup)
}

fn verify_report(manifest_path: &Path, report_path: &Path) -> io::Result<()> {
    let manifest_bytes = fs::read(manifest_path)?;
    let manifest: CampaignManifest =
        serde_json::from_slice(&manifest_bytes).map_err(io::Error::other)?;
    let report: CampaignReport =
        serde_json::from_slice(&fs::read(report_path)?).map_err(io::Error::other)?;
    if report.schema != REPORT_SCHEMA
        || report.summary.schema != SUMMARY_SCHEMA
        || report.summary.manifest_sha256 != sha256_bytes(&manifest_bytes)
        || !report.summary.pass
        || report.summary.summary_only
        || report.cases.len() != 42
        || report.recoveries.len() != 6
        || report.summary.tamper_case_count != 24
        || report.summary.forged_catch_up_case_count != 18
        || report.summary.recovery_case_count != 6
        || report.summary.durable_mutation_count != 0
        || report.summary.manual_repair_action_count != 0
        || report.summary.classification_sha256
            != classification_hash(&report.cases, &report.recoveries)?
    {
        return Err(invalid("E3 report summary or classification mismatch"));
    }
    if report.cases.iter().any(|case| !case.ok)
        || report.recoveries.iter().any(|recovery| !recovery.ok)
    {
        return Err(invalid("E3 report contains a failed case"));
    }
    for evidence in report
        .cases
        .iter()
        .filter_map(|case| case.signed_evidence.as_ref())
    {
        if !verify_evidence(evidence)? {
            return Err(invalid("E3 signed evidence verification failed"));
        }
    }
    if serde_json::to_string(&report)
        .map_err(io::Error::other)?
        .contains("private_key")
    {
        return Err(invalid("E3 report contains private key material"));
    }
    let (audit, _) = source_audit(&manifest)?;
    if audit != report.source_audit {
        return Err(invalid("E3 source audit mismatch"));
    }
    println!(
        "verified E3: {} rejected attacks, {} byte-identical recoveries, classification {}",
        report.summary.rejected_case_count,
        report.summary.byte_identical_recovery_count,
        report.summary.classification_sha256
    );
    Ok(())
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  postfiat-cobalt-e3-harness run <manifest> <output> <work-root> [--summary-only]\n  postfiat-cobalt-e3-harness verify <manifest> <report>"
    );
    std::process::exit(2);
}

fn main() -> io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        Some("run") if (args.len() == 5 || args.len() == 6) => {
            let summary_only = args.get(5).is_some_and(|flag| flag == "--summary-only");
            if args.len() == 6 && !summary_only {
                usage();
            }
            let source_revision = env::var("COBALT_E3_SOURCE_REVISION").map_err(|_| {
                invalid("set COBALT_E3_SOURCE_REVISION to the frozen source commit")
            })?;
            run_campaign(
                Path::new(&args[2]),
                Path::new(&args[3]),
                Path::new(&args[4]),
                source_revision,
                summary_only,
            )
        }
        Some("verify") if args.len() == 4 => {
            verify_report(Path::new(&args[2]), Path::new(&args[3]))
        }
        _ => usage(),
    }
}
