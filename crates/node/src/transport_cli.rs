const TRANSPORT_HELLO_SCHEMA: &str = "postfiat-transport-hello-v1";
const TRANSPORT_BATCH_SCHEMA: &str = "postfiat-transport-batch-v1";
const TRANSPORT_BATCH_ACK_SCHEMA: &str = "postfiat-transport-batch-ack-v1";
const TRANSPORT_CERTIFIED_BATCH_PAYLOAD_SCHEMA: &str =
    "postfiat-transport-certified-batch-payload-v1";
const TRANSPORT_BATCH_TOPIC: &str = "transparent_batch";
const TRANSPORT_BLOCK_VOTE_REQUEST_SCHEMA: &str = "postfiat-transport-block-vote-request-v1";
const TRANSPORT_BLOCK_VOTE_RESPONSE_SCHEMA: &str = "postfiat-transport-block-vote-response-v1";
const TRANSPORT_BLOCK_VOTE_TOPIC: &str = "block_vote_request";
const MAX_TRANSPORT_FRAME_BYTES: u64 = 4 * 1024 * 1024;
const PREWARM_SHIELDED_VERIFIER_ENV: &str = "POSTFIAT_PREWARM_SHIELDED_VERIFIER";
const PREWARM_ASSET_ORCHARD_SWAP_VERIFIER_ENV: &str =
    "POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER";
const PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER_ENV: &str =
    "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER";
const TRANSPORT_PERSISTENT_VOTE_STREAMS_ENV: &str = "POSTFIAT_TRANSPORT_PERSISTENT_VOTE_STREAMS";
const TRANSPORT_VALIDATOR_READY_FILE_ENV: &str = "POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE";
const TRANSPORT_BLOCK_VOTE_READY_FILE_ENV: &str = "POSTFIAT_TRANSPORT_BLOCK_VOTE_READY_FILE";
const CERTIFIED_BATCH_LOOP_READY_FILE_ENV: &str = "POSTFIAT_CERTIFIED_BATCH_LOOP_READY_FILE";
const TRANSPORT_CONNECT_TIMEOUT_MAX_MS: u64 = 10_000;
const CERTIFIED_SEND_JOB_SCHEMA: &str = "postfiat-certified-send-job-v1";
const CERTIFIED_SEND_QUARANTINE_SCHEMA: &str = "postfiat-certified-send-quarantine-v1";
const CERTIFIED_SEND_OUTBOX_DIR: &str = "certified-send-outbox";
const CERTIFIED_SEND_OUTBOX_MAX_JOBS: usize = 1_024;
const CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS: usize = 1_024;
const CERTIFIED_SEND_JOB_MAX_BYTES: u64 = 64 * 1024;
const CERTIFIED_SEND_ERROR_MAX_CHARS: usize = 2_048;
const CERTIFIED_SEND_STAGING_DIR_PREFIX: &str = ".certified-send-stage-v1-";
const CERTIFIED_SEND_COMPLETED_RETENTION_DIR: &str = "completed-retention-v1";
const CERTIFIED_SEND_DISPOSABLE_DIR_MAX_ENTRIES: usize = 16;
static CERTIFIED_SEND_STAGING_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize)]
struct TransportShieldedVerifierPrewarmReport {
    schema: String,
    requested: bool,
    total_ms: f64,
    asset_orchard_swap_verifier_warm: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    asset_orchard_swap_verifier_ms: Option<f64>,
    asset_orchard_private_egress_verifier_warm: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    asset_orchard_private_egress_verifier_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asset_orchard_private_egress_verifier_breakdown:
        Option<postfiat_privacy_orchard::AssetOrchardPrivateEgressTimingReport>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerCertifiedBatchLoopReadyReport<'a> {
    schema: &'static str,
    node_id: &'a str,
    topology_id: &'a str,
    batch_dir: String,
    artifact_root: String,
    start_height: u64,
    max_rounds: usize,
    shielded_verifier_prewarm: &'a TransportShieldedVerifierPrewarmReport,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportValidatorServeReadyReport<'a> {
    schema: &'static str,
    node_id: &'a str,
    topology_id: &'a str,
    bind_address: &'a str,
    vote_dir: String,
    max_connections: usize,
    timeout_ms: u64,
    require_signed_proposal: bool,
    shielded_verifier_prewarm: &'a TransportShieldedVerifierPrewarmReport,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBlockVoteListenReadyReport<'a> {
    schema: &'static str,
    node_id: &'a str,
    topology_id: &'a str,
    bind_address: &'a str,
    vote_dir: String,
    max_requests: usize,
    timeout_ms: u64,
    require_signed_proposal: bool,
    shielded_verifier_prewarm: &'a TransportShieldedVerifierPrewarmReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportHello {
    schema: String,
    topology_id: String,
    node_id: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    state_root: String,
    block_height: u64,
    block_tip_hash: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportListenReport {
    schema: String,
    node_id: String,
    topology_id: String,
    bind_address: String,
    accepted: Vec<TransportHello>,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportDialReport {
    schema: String,
    from: String,
    to: String,
    topology_id: String,
    peer_address: String,
    sent: TransportHello,
    received: TransportHello,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportEnvelopeAuth {
    schema: String,
    signer: String,
    algorithm_id: String,
    public_key_hex: String,
    signature_hex: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportBatchEnvelope {
    schema: String,
    topology_id: String,
    #[serde(default = "default_transport_batch_kind")]
    batch_kind: String,
    frame: FramedMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth: Option<TransportEnvelopeAuth>,
    payload_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    certificate_json: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportCertifiedBatchPayload<'a> {
    schema: &'static str,
    batch_kind: &'a str,
    payload_json: &'a str,
    certificate_json: &'a str,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportBlockVoteRequestEnvelope {
    schema: String,
    topology_id: String,
    frame: FramedMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    auth: Option<TransportEnvelopeAuth>,
    block_height: u64,
    view: u64,
    batch_kind: String,
    batch_json: String,
    proposal_json: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_certificate_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consensus_v2: Option<TransportConsensusV2VoteRequest>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportConsensusV2VoteRequest {
    phase: postfiat_types::ConsensusV2Phase,
    proposal: postfiat_types::ConsensusV2Proposal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_certificate: Option<postfiat_types::ConsensusV2TimeoutCertificate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prepare_qc: Option<postfiat_types::ConsensusV2QuorumCertificate>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBlockVoteRequestPayload<'a> {
    schema: &'static str,
    block_height: u64,
    view: u64,
    batch_kind: &'a str,
    batch_json: &'a str,
    proposal_json: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_certificate_json: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    consensus_v2: Option<&'a TransportConsensusV2VoteRequest>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportBlockVoteHandlingTimingReport {
    schema: String,
    total_ms: f64,
    transport_read_ms: f64,
    request_parse_ms: f64,
    request_validate_ms: f64,
    auth_validate_ms: f64,
    signed_proposal_policy_ms: f64,
    request_dir_ms: f64,
    batch_file_write_ms: f64,
    proposal_file_write_ms: f64,
    vote_creation_ms: f64,
    response_build_ms: f64,
    response_json_serde_ms: f64,
    transport_write_ms: f64,
    process_spawn_ms: f64,
    block_vote_breakdown: BlockVoteCreationTimingReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportBlockVoteResponse {
    schema: String,
    topology_id: String,
    from: String,
    to: String,
    message_id: String,
    payload_hash: String,
    block_height: u64,
    view: u64,
    vote_file: String,
    vote: BlockVoteFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consensus_v2_vote: Option<postfiat_types::ConsensusV2Vote>,
    state: TransportHello,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timings: Option<TransportBlockVoteHandlingTimingReport>,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBlockVoteListenReport {
    schema: String,
    node_id: String,
    topology_id: String,
    bind_address: String,
    require_signed_proposal: bool,
    shielded_verifier_prewarm: TransportShieldedVerifierPrewarmReport,
    accepted: Vec<TransportBlockVoteResponse>,
    verified: bool,
}

#[derive(Debug, Clone)]
struct TransportBlockVoteRequestOptions {
    data_dir: PathBuf,
    topology_file: PathBuf,
    to: String,
    batch_kind: Option<String>,
    batch_file: PathBuf,
    proposal_file: PathBuf,
    timeout_certificate_file: Option<PathBuf>,
    vote_file: PathBuf,
    block_height: Option<u64>,
    timeout_ms: u64,
    consensus_v2: Option<TransportConsensusV2VoteRequest>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBlockVoteRequestReport {
    schema: String,
    from: String,
    to: String,
    topology_id: String,
    peer_address: String,
    attempts: u64,
    max_attempts: u64,
    retry_backoff_ms: u64,
    retry_errors: Vec<String>,
    request: TransportBatchSummary,
    response: TransportBlockVoteResponse,
    vote_file: String,
    timings: TransportBlockVoteRequestTimingReport,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBlockVoteRequestTimingReport {
    schema: String,
    total_ms: f64,
    attempt_loop_ms: f64,
    retry_sleep_ms: f64,
    topology_read_ms: f64,
    status_ms: f64,
    peer_lookup_ms: f64,
    payload_read_ms: f64,
    request_json_serde_ms: f64,
    request_frame_ms: f64,
    transport_connect_ms: f64,
    transport_write_ms: f64,
    transport_read_ms: f64,
    response_json_serde_ms: f64,
    response_validate_ms: f64,
    vote_json_serde_ms: f64,
    vote_file_write_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_handling: Option<TransportBlockVoteHandlingTimingReport>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBatchSummary {
    from: String,
    to: String,
    batch_kind: String,
    message_id: String,
    payload_hash: String,
    payload_len: u64,
    certificate_attached: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportBatchAck {
    schema: String,
    topology_id: String,
    from: String,
    to: String,
    message_id: String,
    payload_hash: String,
    applied: bool,
    #[serde(default)]
    already_applied: bool,
    receipt_count: u64,
    accepted_count: u64,
    rejected_count: u64,
    certificate_attached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    certified_state: Option<TransportHello>,
    state: TransportHello,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBatchListenReport {
    schema: String,
    node_id: String,
    topology_id: String,
    bind_address: String,
    accepted: Vec<TransportBatchAck>,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBatchServeEvent {
    schema: String,
    node_id: String,
    topology_id: String,
    batch_index: u64,
    outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ack: Option<TransportBatchAck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejection: Option<TransportBatchServeRejection>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBatchServeRejection {
    schema: String,
    node_id: String,
    topology_id: String,
    batch_index: u64,
    error: String,
    state: TransportHello,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBatchServeReport {
    schema: String,
    node_id: String,
    topology_id: String,
    bind_address: String,
    event_log: Option<String>,
    accepted_count: u64,
    rejected_count: u64,
    accepted: Vec<TransportBatchAck>,
    rejected: Vec<TransportBatchServeRejection>,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportValidatorServeEvent {
    schema: String,
    node_id: String,
    topology_id: String,
    connection_index: u64,
    kind: String,
    outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_ack: Option<TransportBatchAck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    block_vote_response: Option<TransportBlockVoteResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejection: Option<TransportValidatorServeRejection>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TransportValidatorServeRejection {
    schema: String,
    node_id: String,
    topology_id: String,
    connection_index: u64,
    kind: String,
    error: String,
    state: TransportHello,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportValidatorServeReport {
    schema: String,
    node_id: String,
    topology_id: String,
    bind_address: String,
    event_log: Option<String>,
    require_signed_proposal: bool,
    shielded_verifier_prewarm: TransportShieldedVerifierPrewarmReport,
    connection_count: u64,
    accepted_batch_count: u64,
    accepted_block_vote_count: u64,
    rejected_count: u64,
    batch_acks: Vec<TransportBatchAck>,
    block_vote_responses: Vec<TransportBlockVoteResponse>,
    rejected: Vec<TransportValidatorServeRejection>,
    verified: bool,
}

#[derive(Debug, Default)]
struct TransportValidatorServeSharedState {
    batch_acks: Vec<TransportBatchAck>,
    block_vote_responses: Vec<TransportBlockVoteResponse>,
    rejected: Vec<TransportValidatorServeRejection>,
}

fn prewarm_shielded_verifier_cache(
    context: &str,
) -> Result<TransportShieldedVerifierPrewarmReport, String> {
    let total_start = Instant::now();
    let requested = std::env::var_os(PREWARM_SHIELDED_VERIFIER_ENV).is_some();
    eprintln!("INFO {context} shielded verifier prewarm start requested={requested}");
    let mut report = TransportShieldedVerifierPrewarmReport {
        schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
        requested,
        total_ms: 0.0,
        asset_orchard_swap_verifier_warm: false,
        asset_orchard_swap_verifier_ms: None,
        asset_orchard_private_egress_verifier_warm: false,
        asset_orchard_private_egress_verifier_ms: None,
        asset_orchard_private_egress_verifier_breakdown: None,
    };
    if !requested {
        report.total_ms = monotonic_elapsed_ms(total_start);
        eprintln!(
            "INFO {context} shielded verifier prewarm complete requested={} total_ms={:.3}",
            report.requested, report.total_ms
        );
        return Ok(report);
    }
    if shielded_prewarm_component_enabled(PREWARM_ASSET_ORCHARD_SWAP_VERIFIER_ENV) {
        let stage_start = Instant::now();
        let verifying_key = postfiat_privacy_orchard::AssetOrchardSwapVerifyingKey::cached()
            .map_err(|error| format!("{context} shielded verifier prewarm failed: {error}"))?;
        verifying_key
            .metadata()
            .validate_release_pin()
            .map_err(|error| format!("{context} shielded verifier release pin failed: {error}"))?;
        report.asset_orchard_swap_verifier_ms = Some(monotonic_elapsed_ms(stage_start));
        report.asset_orchard_swap_verifier_warm = true;
    }

    if shielded_prewarm_component_enabled(PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER_ENV) {
        postfiat_privacy_orchard::reset_asset_orchard_private_egress_timings();
        let stage_start = Instant::now();
        let private_egress_verifying_key =
            postfiat_privacy_orchard::AssetOrchardPrivateEgressVerifyingKey::cached().map_err(
                |error| format!("{context} private-egress verifier prewarm failed: {error}"),
            )?;
        private_egress_verifying_key
            .metadata()
            .validate_release_pin()
            .map_err(|error| {
                format!("{context} private-egress verifier release pin failed: {error}")
            })?;
        report.asset_orchard_private_egress_verifier_ms = Some(monotonic_elapsed_ms(stage_start));
        report.asset_orchard_private_egress_verifier_warm = true;
        report.asset_orchard_private_egress_verifier_breakdown = Some(
            postfiat_privacy_orchard::take_asset_orchard_private_egress_timings(),
        );
    }
    report.total_ms = monotonic_elapsed_ms(total_start);
    eprintln!(
        "INFO {context} shielded verifier prewarm complete requested={} total_ms={:.3} swap_warm={} private_egress_warm={}",
        report.requested,
        report.total_ms,
        report.asset_orchard_swap_verifier_warm,
        report.asset_orchard_private_egress_verifier_warm
    );
    Ok(report)
}

fn shielded_prewarm_component_enabled(env_name: &str) -> bool {
    std::env::var(env_name)
        .ok()
        .map(|value| {
            let value = value.trim();
            !(value == "0" || value.eq_ignore_ascii_case("false") || value.eq_ignore_ascii_case("no"))
        })
        .unwrap_or(true)
}

fn transport_startup_after_prewarm<L, P, B, W>(
    prewarm: P,
    bind: B,
    write_ready: W,
) -> Result<(L, TransportShieldedVerifierPrewarmReport), String>
where
    P: FnOnce() -> Result<TransportShieldedVerifierPrewarmReport, String>,
    B: FnOnce() -> Result<L, String>,
    W: FnOnce(&TransportShieldedVerifierPrewarmReport) -> Result<(), String>,
{
    let shielded_verifier_prewarm = prewarm()?;
    if !shielded_verifier_prewarm.requested
        || !shielded_verifier_prewarm.asset_orchard_swap_verifier_warm
        || !shielded_verifier_prewarm.asset_orchard_private_egress_verifier_warm
    {
        return Err(
            "transport readiness requires requested, warm swap and warm private-egress verifiers"
                .to_string(),
        );
    }
    let listener = bind()?;
    write_ready(&shielded_verifier_prewarm)?;
    Ok((listener, shielded_verifier_prewarm))
}

fn write_transport_ready_file<T: serde::Serialize>(
    ready_file: &Path,
    value: &T,
    context: &str,
) -> Result<(), String> {
    if let Some(parent) = ready_file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "{context} ready file parent `{}` create failed: {error}",
                parent.display()
            )
        })?;
    }
    let json = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("{context} ready file serialization failed: {error}"))?;
    postfiat_storage::atomic_write(ready_file, [json.as_slice(), b"\n"].concat())
        .map_err(|error| format!("{context} ready file atomic write failed: {error}"))
}

fn transport_ready_file_from_env(
    env_name: &str,
    context: &str,
) -> Result<Option<PathBuf>, String> {
    let Some(value) = std::env::var_os(env_name) else {
        return Ok(None);
    };
    if value.as_os_str().is_empty() {
        eprintln!("WARN {context} ready file env {env_name} is set but empty; treating as unset");
        return Ok(None);
    }
    Ok(Some(PathBuf::from(value)))
}

fn clear_transport_ready_file(ready_file: &Path, context: &str) -> Result<(), String> {
    match std::fs::remove_file(ready_file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "{context} stale ready file `{}` remove failed: {error}",
            ready_file.display()
        )),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportBatchSendReport {
    schema: String,
    from: String,
    to: String,
    topology_id: String,
    peer_address: String,
    attempts: u64,
    max_attempts: u64,
    retry_backoff_ms: u64,
    retry_errors: Vec<String>,
    sent: TransportBatchSummary,
    ack: TransportBatchAck,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerFailureReport {
    to: String,
    error: String,
}

#[derive(Debug, Clone)]
struct TransportCertifiedBatchRoundOptions {
    data_dir: PathBuf,
    topology_file: PathBuf,
    batch_kind: Option<String>,
    batch_file: PathBuf,
    validator_key_dir: PathBuf,
    artifact_dir: PathBuf,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    skip_block_log_verify: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportCertifiedBatchRoundReport {
    schema: String,
    from: String,
    topology_id: String,
    peer_count: usize,
    batch_file: String,
    artifact_dir: String,
    certification: BatchCertificateRoundReport,
    sends: Vec<TransportBatchSendReport>,
    send_retries: usize,
    retry_backoff_ms: u64,
    retry_send_count: u64,
    retry_error_count: u64,
    local_receipt_count: u64,
    local_accepted_count: u64,
    local_rejected_count: u64,
    local_apply_verified: bool,
    local_state: TransportHello,
    all_sends_verified: bool,
    round_ok: bool,
}

#[derive(Debug, Clone)]
struct TransportPeerCertifiedBatchRoundOptions {
    data_dir: PathBuf,
    topology_file: PathBuf,
    batch_kind: Option<String>,
    batch_file: PathBuf,
    key_file: PathBuf,
    proposal_key_file: Option<PathBuf>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    artifact_dir: PathBuf,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    required_parent: Option<RequiredBlockParent>,
}

#[derive(Debug, Clone)]
struct TransportPeerCertifiedMempoolRoundOptions {
    data_dir: PathBuf,
    topology_file: PathBuf,
    key_file: PathBuf,
    proposal_key_file: Option<PathBuf>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    artifact_dir: PathBuf,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    max_transactions: usize,
    signed_transfer_file: Option<PathBuf>,
    signed_transfer_json: Option<String>,
    signed_payment_v2_json: Option<String>,
    signed_asset_transaction_json: Option<String>,
    signed_atomic_swap_transaction_json: Option<String>,
    signed_escrow_transaction_json: Option<String>,
    required_parent: Option<RequiredBlockParent>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerTargetTimingReport {
    target: String,
    duration_ms: f64,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    vote_request_breakdown: Option<TransportBlockVoteRequestTimingReport>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerCertifiedBatchRoundTimingsReport {
    total_ms: f64,
    shielded_verifier_prewarm: TransportShieldedVerifierPrewarmReport,
    setup_ms: f64,
    proposal_ms: f64,
    target_selection_ms: f64,
    local_vote_ms: f64,
    vote_requests_ms: f64,
    certificate_ms: f64,
    certified_sends_ms: f64,
    local_apply_ms: f64,
    post_apply_status_ms: f64,
    client_visible_finality_ms: f64,
    verification_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal_breakdown: Option<BatchProposalTimingReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_apply_breakdown: Option<ApplyBatchTimingReport>,
    vote_request_targets: Vec<TransportPeerTargetTimingReport>,
    certified_send_targets: Vec<TransportPeerTargetTimingReport>,
}

fn monotonic_elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

struct TransportPeerVoteRequestOutcome {
    target: String,
    vote_file: PathBuf,
    duration_ms: f64,
    result: Result<TransportBlockVoteRequestReport, String>,
}

struct TransportPeerBatchSendOutcome {
    target: String,
    duration_ms: f64,
    result: Result<TransportBatchSendReport, String>,
}

struct TransportBatchApplyResult {
    receipts: Vec<Receipt>,
    local_apply_breakdown: Option<ApplyBatchTimingReport>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerDeferredSendJobReport {
    schema: String,
    job_id: String,
    target: String,
    mode: String,
    pid: u32,
    job_file: String,
    report_file: String,
    stderr_file: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DurableCertifiedSendAck {
    already_applied: bool,
    block_height: u64,
    block_tip_hash: String,
    state_root: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DurableCertifiedSendJob {
    schema: String,
    job_id: String,
    topology_id: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    source: String,
    target: String,
    batch_kind: String,
    block_height: u64,
    certificate_id: String,
    block_hash: String,
    expected_state_root: String,
    batch_file: String,
    batch_hash: String,
    certificate_file: String,
    certificate_hash: String,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    attempt_count: u64,
    completed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ack: Option<DurableCertifiedSendAck>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DurableCertifiedSendQuarantineRecord {
    schema: String,
    job_id: String,
    job_file: String,
    reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DurableCertifiedSendResumeTargetReport {
    job_id: String,
    target: String,
    result: String,
    already_applied: bool,
    attempts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DurableCertifiedSendResumeReport {
    schema: String,
    outbox_dir: String,
    discovered: usize,
    attempted: usize,
    completed: usize,
    pending: usize,
    quarantined: usize,
    targets: Vec<DurableCertifiedSendResumeTargetReport>,
    all_completed: bool,
}

fn transport_artifact_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn certified_send_outbox_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CERTIFIED_SEND_OUTBOX_DIR)
}

fn certified_send_completed_dir(data_dir: &Path) -> PathBuf {
    certified_send_outbox_dir(data_dir).join("completed")
}

fn certified_send_quarantine_dir(data_dir: &Path) -> PathBuf {
    certified_send_outbox_dir(data_dir).join("quarantine")
}

fn certified_send_resolved_quarantine_dir(data_dir: &Path) -> PathBuf {
    certified_send_outbox_dir(data_dir).join("resolved-quarantine")
}

fn certified_send_completed_retention_dir(data_dir: &Path) -> PathBuf {
    certified_send_resolved_quarantine_dir(data_dir).join(CERTIFIED_SEND_COMPLETED_RETENTION_DIR)
}

fn certified_send_job_id_is_canonical(job_id: &str) -> bool {
    job_id.len() == 96
        && job_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn certified_send_staging_dir_job_id(name: &str) -> Option<&str> {
    let suffix = name.strip_prefix(CERTIFIED_SEND_STAGING_DIR_PREFIX)?;
    if suffix.len() != 193 || suffix.as_bytes().get(96) != Some(&b'-') {
        return None;
    }
    let job_id = &suffix[..96];
    let staging_id = &suffix[97..];
    (certified_send_job_id_is_canonical(job_id) && certified_send_job_id_is_canonical(staging_id))
        .then_some(job_id)
}

fn certified_send_atomic_temp_name_is_canonical(name: &str, target: &str) -> bool {
    let prefix = format!(".{target}.");
    let Some(middle) = name
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(".tmp"))
    else {
        return false;
    };
    let mut components = middle.split('.');
    let canonical = (0..4).all(|_| {
        components.next().is_some_and(|component| {
            !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
        })
    });
    canonical && components.next().is_none()
}

fn certified_send_disposable_file_is_canonical(name: &str) -> bool {
    matches!(name, "batch.json" | "certificate.json" | "job.json")
        || ["batch.json", "certificate.json", "job.json"]
            .iter()
            .any(|target| certified_send_atomic_temp_name_is_canonical(name, target))
}

#[cfg(unix)]
fn sync_certified_send_directory(path: &Path, label: &str) -> Result<(), String> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "certified send {label} sync `{}` failed: {error}",
                path.display()
            )
        })
}

#[cfg(not(unix))]
fn sync_certified_send_directory(_path: &Path, _label: &str) -> Result<(), String> {
    Ok(())
}

fn require_certified_send_directory(path: &Path, label: &str) -> Result<bool, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "certified send {label} metadata `{}` failed: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "certified send {label} `{}` must be a non-symlink directory",
            path.display()
        ));
    }
    Ok(true)
}

fn validate_certified_send_disposable_job_dir(
    directory: &Path,
    expected_job_id: &str,
) -> Result<(), String> {
    if !require_certified_send_directory(directory, "disposable job directory")? {
        return Err(format!(
            "certified send disposable job directory `{}` disappeared during cleanup",
            directory.display()
        ));
    }
    let mut entry_count = 0usize;
    for entry in std::fs::read_dir(directory).map_err(|error| {
        format!(
            "certified send disposable job directory read `{}` failed: {error}",
            directory.display()
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "certified send disposable job entry `{}` failed: {error}",
                directory.display()
            )
        })?;
        entry_count = entry_count.saturating_add(1);
        if entry_count > CERTIFIED_SEND_DISPOSABLE_DIR_MAX_ENTRIES {
            return Err(format!(
                "certified send disposable job directory `{}` exceeds the bounded entry count {CERTIFIED_SEND_DISPOSABLE_DIR_MAX_ENTRIES}",
                directory.display()
            ));
        }
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            format!(
                "certified send disposable job entry `{}` is not valid UTF-8",
                entry.path().display()
            )
        })?;
        if !certified_send_disposable_file_is_canonical(name) {
            return Err(format!(
                "certified send disposable job directory `{}` contains unknown entry `{name}`",
                directory.display()
            ));
        }
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(|error| {
            format!(
                "certified send disposable job entry metadata `{}` failed: {error}",
                entry.path().display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "certified send disposable job entry `{}` must be a non-symlink regular file",
                entry.path().display()
            ));
        }
        let size_limit = if name == "job.json" || name.starts_with(".job.json.") {
            CERTIFIED_SEND_JOB_MAX_BYTES
        } else {
            MAX_TRANSPORT_FRAME_BYTES
        };
        if metadata.len() > size_limit {
            return Err(format!(
                "certified send disposable job entry `{}` exceeds its bounded size",
                entry.path().display()
            ));
        }
        if name == "job.json" {
            let job = read_durable_certified_send_job(&entry.path())?;
            if job.job_id != expected_job_id {
                return Err(format!(
                    "certified send disposable job `{}` does not match directory identity `{expected_job_id}`",
                    job.job_id
                ));
            }
        }
    }
    Ok(())
}

fn remove_certified_send_disposable_job_dir(
    directory: &Path,
    expected_job_id: &str,
) -> Result<(), String> {
    validate_certified_send_disposable_job_dir(directory, expected_job_id)?;
    let parent = directory.parent().ok_or_else(|| {
        format!(
            "certified send disposable job directory `{}` has no parent",
            directory.display()
        )
    })?;
    std::fs::remove_dir_all(directory).map_err(|error| {
        format!(
            "certified send disposable job directory remove `{}` failed: {error}",
            directory.display()
        )
    })?;
    sync_certified_send_directory(parent, "disposable job parent")
}

pub(crate) fn cleanup_orphan_certified_send_staging_dirs(data_dir: &Path) -> Result<usize, String> {
    let outbox_dir = certified_send_outbox_dir(data_dir);
    if !require_certified_send_directory(&outbox_dir, "outbox directory")? {
        return Ok(0);
    }
    let mut staging_dirs = Vec::new();
    for entry in std::fs::read_dir(&outbox_dir)
        .map_err(|error| format!("certified send staging cleanup read failed: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("certified send staging entry failed: {error}"))?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            format!(
                "certified send outbox entry `{}` is not valid UTF-8",
                entry.path().display()
            )
        })?;
        if let Some(job_id) = certified_send_staging_dir_job_id(name) {
            staging_dirs.push((entry.path(), job_id.to_string()));
        } else if name.starts_with(CERTIFIED_SEND_STAGING_DIR_PREFIX) {
            return Err(format!(
                "certified send staging entry `{}` has a non-canonical name",
                entry.path().display()
            ));
        } else if matches!(name, "completed" | "quarantine" | "resolved-quarantine")
            || certified_send_job_id_is_canonical(name)
        {
            if !require_certified_send_directory(&entry.path(), "recognized outbox entry")? {
                return Err(format!(
                    "certified send recognized outbox entry `{}` disappeared during staging cleanup",
                    entry.path().display()
                ));
            }
        } else {
            return Err(format!(
                "certified send outbox contains unknown entry `{}`",
                entry.path().display()
            ));
        }
    }
    staging_dirs.sort_by(|left, right| left.0.cmp(&right.0));
    for (directory, job_id) in &staging_dirs {
        remove_certified_send_disposable_job_dir(directory, job_id)?;
    }
    Ok(staging_dirs.len())
}

// A staging directory is never a delivery record. Only the rename below makes
// the complete three-file bundle visible under its canonical job id.
fn next_certified_send_staging_dir(outbox_dir: &Path, job_id: &str) -> Result<PathBuf, String> {
    for attempt in 0_u32..128 {
        let counter =
            CERTIFIED_SEND_STAGING_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let identity = format!(
            "{job_id}\n{}\n{counter}\n{nanos}\n{attempt}\n",
            std::process::id()
        );
        let staging_id = postfiat_crypto_provider::hash_hex(
            "postfiat.certified_send_staging.v1",
            identity.as_bytes(),
        );
        let directory = outbox_dir.join(format!(
            "{CERTIFIED_SEND_STAGING_DIR_PREFIX}{job_id}-{staging_id}"
        ));
        match std::fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "certified send staging directory create `{}` failed: {error}",
                    directory.display()
                ));
            }
        }
    }
    Err("certified send staging directory allocation exhausted".to_string())
}

fn certified_send_quarantine_job_dir(data_dir: &Path, job_id: &str) -> PathBuf {
    certified_send_quarantine_dir(data_dir).join(job_id)
}

fn certified_send_quarantine_record_file(data_dir: &Path, job_id: &str) -> PathBuf {
    certified_send_quarantine_job_dir(data_dir, job_id)
        .join("quarantine.json")
}

fn quarantine_durable_certified_send_job(
    data_dir: &Path,
    job_id: &str,
    job_file: &Path,
    reason: &str,
) -> Result<PathBuf, String> {
    let record_file = certified_send_quarantine_record_file(data_dir, job_id);
    let record_dir = certified_send_quarantine_job_dir(data_dir, job_id);
    if record_dir.exists() {
        return Ok(record_file);
    }
    let quarantine_dir = certified_send_quarantine_dir(data_dir);
    std::fs::create_dir_all(&quarantine_dir).map_err(|error| {
        format!(
            "certified send quarantine root create `{}` failed: {error}",
            quarantine_dir.display()
        )
    })?;
    std::fs::create_dir_all(&record_dir).map_err(|error| {
        format!(
            "certified send quarantine directory create `{}` failed: {error}",
            record_dir.display()
        )
    })?;
    #[cfg(unix)]
    {
        std::fs::File::open(certified_send_outbox_dir(data_dir))
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("certified send outbox quarantine sync failed: {error}"))?;
        std::fs::File::open(&quarantine_dir)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("certified send quarantine root sync failed: {error}"))?;
    }
    let record = DurableCertifiedSendQuarantineRecord {
        schema: CERTIFIED_SEND_QUARANTINE_SCHEMA.to_string(),
        job_id: job_id.to_string(),
        job_file: job_file.display().to_string(),
        reason: truncate_certified_send_error(reason.to_string()),
    };
    let bytes = serde_json::to_vec_pretty(&record)
        .map_err(|error| format!("certified send quarantine serialization failed: {error}"))?;
    postfiat_storage::atomic_write(&record_file, bytes)
        .map_err(|error| format!("certified send quarantine atomic write failed: {error}"))?;
    #[cfg(unix)]
    std::fs::File::open(record_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("certified send quarantine directory sync failed: {error}"))?;
    Ok(record_file)
}

fn ensure_durable_certified_send_job_not_quarantined(
    data_dir: &Path,
    job_id: &str,
) -> Result<(), String> {
    let record_file = certified_send_quarantine_record_file(data_dir, job_id);
    if certified_send_quarantine_job_dir(data_dir, job_id).exists() {
        return Err(format!(
            "certified send job `{job_id}` is quarantined at `{}`",
            record_file.display()
        ));
    }
    Ok(())
}

fn rotated_topology_ack_quarantine_is_recoverable(
    job_file: &Path,
    job: &DurableCertifiedSendJob,
    topology: &NetworkTopology,
    record: &DurableCertifiedSendQuarantineRecord,
) -> bool {
    topology.topology_id != job.topology_id
        && record.job_id == job.job_id
        && Path::new(&record.job_file) == job_file
        && record.reason.starts_with(&format!(
            "certified send ack from `{}` conflicts: expected height/hash/root",
            job.target
        ))
}

fn resolve_rotated_topology_ack_quarantine(
    data_dir: &Path,
    job_id: &str,
) -> Result<(), String> {
    let source = certified_send_quarantine_job_dir(data_dir, job_id);
    if !source.exists() {
        return Ok(());
    }
    let root = certified_send_resolved_quarantine_dir(data_dir);
    std::fs::create_dir_all(&root).map_err(|error| {
        format!(
            "certified send resolved-quarantine root create `{}` failed: {error}",
            root.display()
        )
    })?;
    let destination = root.join(job_id);
    if destination.exists() {
        return Err(format!(
            "certified send resolved-quarantine record already exists at `{}`",
            destination.display()
        ));
    }
    std::fs::rename(&source, &destination).map_err(|error| {
        format!(
            "certified send quarantine resolve `{}` -> `{}` failed: {error}",
            source.display(),
            destination.display()
        )
    })?;
    #[cfg(unix)]
    {
        std::fs::File::open(certified_send_quarantine_dir(data_dir))
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("certified send quarantine resolve sync failed: {error}"))?;
        std::fs::File::open(&root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!("certified send resolved-quarantine sync failed: {error}")
            })?;
    }
    Ok(())
}

fn read_durable_certified_send_quarantine_record(
    path: &Path,
) -> Result<DurableCertifiedSendQuarantineRecord, String> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "certified send quarantine metadata `{}` failed: {error}",
            path.display()
        )
    })?;
    if metadata.len() > CERTIFIED_SEND_JOB_MAX_BYTES {
        return Err(format!(
            "certified send quarantine `{}` exceeded {CERTIFIED_SEND_JOB_MAX_BYTES} bytes",
            path.display()
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "certified send quarantine read `{}` failed: {error}",
            path.display()
        )
    })?;
    let record: DurableCertifiedSendQuarantineRecord = serde_json::from_slice(&bytes).map_err(
        |error| {
            format!(
                "certified send quarantine parse `{}` failed: {error}",
                path.display()
            )
        },
    )?;
    if record.schema != CERTIFIED_SEND_QUARANTINE_SCHEMA || record.job_id.trim().is_empty() {
        return Err("certified send quarantine schema or job id is invalid".to_string());
    }
    let directory_job_id = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .ok_or_else(|| "certified send quarantine path has no job id".to_string())?;
    if record.job_id != directory_job_id {
        return Err("certified send quarantine job id does not match its directory".to_string());
    }
    Ok(record)
}

fn durable_certified_send_job_id(
    topology_id: &str,
    source: &str,
    target: &str,
    block_height: u64,
    certificate_id: &str,
    block_hash: &str,
) -> String {
    let identity = format!(
        "{topology_id}\n{source}\n{target}\n{block_height}\n{certificate_id}\n{block_hash}\n"
    );
    postfiat_crypto_provider::hash_hex("postfiat.certified_send_job.v1", identity.as_bytes())
}

fn durable_certified_send_expected_block_hash(
    topology: &NetworkTopology,
    proposal: &BlockProposalFile,
    certificate: &BlockCertificateFile,
) -> Result<String, String> {
    if let Some(commit) = certificate.consensus_v2_commit.as_ref() {
        if commit.proposal.block.height != proposal.block_height
            || commit.proposal.block.state_root != proposal.state_root
        {
            return Err(
                "consensus v2 certified send commit does not match block proposal".to_string(),
            );
        }
        return Ok(commit.proposal.block.block_id.clone());
    }
    let encoded = serde_json::to_vec(&(
        topology.chain_id.as_str(),
        topology.genesis_hash.as_str(),
        topology.protocol_version,
        proposal.block_height,
        proposal.view,
        proposal.parent_hash.as_str(),
        proposal.proposer.as_str(),
        proposal.batch_kind.as_str(),
        proposal.batch_id.as_str(),
        proposal.state_root.as_str(),
        proposal.receipt_ids.as_slice(),
        certificate.certificate_id.as_str(),
    ))
    .map_err(|error| format!("certified send expected block hash encoding failed: {error}"))?;
    Ok(postfiat_crypto_provider::hash_hex(
        "postfiat.block.v1",
        &encoded,
    ))
}

#[allow(clippy::too_many_arguments)]
fn durable_certified_send_job_matches_request(
    job: &DurableCertifiedSendJob,
    topology: &NetworkTopology,
    source: &str,
    target: &str,
    batch_kind: &str,
    block_height: u64,
    certificate_id: &str,
    block_hash: &str,
    expected_state_root: &str,
    batch_hash: &str,
    certificate_hash: &str,
) -> bool {
    job.topology_id == topology.topology_id
        && job.chain_id == topology.chain_id
        && job.genesis_hash == topology.genesis_hash
        && job.protocol_version == topology.protocol_version
        && job.source == source
        && job.target == target
        && job.batch_kind == batch_kind
        && job.block_height == block_height
        && job.certificate_id == certificate_id
        && job.block_hash == block_hash
        && job.expected_state_root == expected_state_root
        && job.batch_hash == batch_hash
        && job.certificate_hash == certificate_hash
}

#[allow(clippy::too_many_arguments)]
fn enqueue_durable_certified_send_job(
    data_dir: &Path,
    topology: &NetworkTopology,
    source: &str,
    target: &str,
    batch_kind: &str,
    block_height: u64,
    certificate_id: &str,
    block_hash: &str,
    expected_state_root: &str,
    batch_file: &Path,
    certificate_file: &Path,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
) -> Result<PathBuf, String> {
    if send_retries > MAX_TRANSPORT_SEND_RETRIES {
        return Err(format!(
            "certified send retries must be <= {MAX_TRANSPORT_SEND_RETRIES}"
        ));
    }
    if block_height == 0
        || certificate_id.trim().is_empty()
        || block_hash.trim().is_empty()
        || expected_state_root.trim().is_empty()
    {
        return Err("certified send job identity or expected state is incomplete".to_string());
    }
    if topology.peer(source).is_none() || topology.peer(target).is_none() || source == target {
        return Err("certified send job source/target is not a distinct topology route".to_string());
    }

    let outbox_dir = certified_send_outbox_dir(data_dir);
    let outbox_was_present = outbox_dir.exists();
    std::fs::create_dir_all(&outbox_dir).map_err(|error| {
        format!(
            "certified send outbox create `{}` failed: {error}",
            outbox_dir.display()
        )
    })?;
    if !outbox_was_present {
        sync_certified_send_directory(data_dir, "data directory")?;
    }
    cleanup_orphan_certified_send_staging_dirs(data_dir)?;
    let job_id = durable_certified_send_job_id(
        &topology.topology_id,
        source,
        target,
        block_height,
        certificate_id,
        block_hash,
    );
    ensure_durable_certified_send_job_not_quarantined(data_dir, &job_id)?;

    let batch_bytes = std::fs::read(batch_file).map_err(|error| {
        format!(
            "certified send batch read `{}` failed: {error}",
            batch_file.display()
        )
    })?;
    let certificate_bytes = std::fs::read(certificate_file).map_err(|error| {
        format!(
            "certified send certificate read `{}` failed: {error}",
            certificate_file.display()
        )
    })?;
    if batch_bytes.len() as u64 > MAX_TRANSPORT_FRAME_BYTES {
        return Err(format!(
            "certified send batch exceeded {MAX_TRANSPORT_FRAME_BYTES} bytes"
        ));
    }
    if certificate_bytes.len() as u64 > MAX_TRANSPORT_FRAME_BYTES {
        return Err(format!(
            "certified send certificate exceeded {MAX_TRANSPORT_FRAME_BYTES} bytes"
        ));
    }
    let batch_hash = postfiat_crypto_provider::hash_hex(
        "postfiat.certified_send_job.batch.v1",
        &batch_bytes,
    );
    let certificate_hash = postfiat_crypto_provider::hash_hex(
        "postfiat.certified_send_job.certificate.v1",
        &certificate_bytes,
    );

    let job_dir = outbox_dir.join(&job_id);
    let job_file = job_dir.join("job.json");
    if job_file.exists() {
        let existing = read_durable_certified_send_job(&job_file)?;
        if let Err(detail) = validate_durable_certified_send_payloads(&job_file, &existing) {
            let error = format!("certified send active job `{job_id}` is corrupt: {detail}");
            quarantine_durable_certified_send_job(data_dir, &job_id, &job_file, &error)?;
            return Err(error);
        }
        if durable_certified_send_job_matches_request(
            &existing,
            topology,
            source,
            target,
            batch_kind,
            block_height,
            certificate_id,
            block_hash,
            expected_state_root,
            &batch_hash,
            &certificate_hash,
        ) {
            return Ok(job_file);
        }
        let error = format!(
            "certified send job `{job_id}` conflicts with its persisted active record"
        );
        quarantine_durable_certified_send_job(data_dir, &job_id, &job_file, &error)?;
        return Err(error);
    }

    let completed_job_file = certified_send_completed_dir(data_dir)
        .join(&job_id)
        .join("job.json");
    if completed_job_file.exists() {
        let existing = read_durable_certified_send_job(&completed_job_file)?;
        let valid_tombstone = validate_completed_durable_certified_send_job(
            &completed_job_file,
            &existing,
        );
        let matching_request = durable_certified_send_job_matches_request(
            &existing,
            topology,
            source,
            target,
            batch_kind,
            block_height,
            certificate_id,
            block_hash,
            expected_state_root,
            &batch_hash,
            &certificate_hash,
        );
        if valid_tombstone.is_ok() && matching_request {
            return Ok(completed_job_file);
        }
        let detail = valid_tombstone
            .err()
            .unwrap_or_else(|| "enqueue request does not match tombstone".to_string());
        let error = format!("certified send tombstone `{job_id}` conflicts: {detail}");
        quarantine_durable_certified_send_job(
            data_dir,
            &job_id,
            &completed_job_file,
            &error,
        )?;
        return Err(error);
    }

    let queued_jobs = std::fs::read_dir(&outbox_dir)
        .map_err(|error| format!("certified send outbox read failed: {error}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("job.json").is_file())
        .count();
    if queued_jobs >= CERTIFIED_SEND_OUTBOX_MAX_JOBS {
        return Err(format!(
            "certified send outbox reached bounded capacity {CERTIFIED_SEND_OUTBOX_MAX_JOBS}"
        ));
    }

    let durable_batch_file = job_dir.join("batch.json");
    let durable_certificate_file = job_dir.join("certificate.json");
    let job = DurableCertifiedSendJob {
        schema: CERTIFIED_SEND_JOB_SCHEMA.to_string(),
        job_id: job_id.clone(),
        topology_id: topology.topology_id.clone(),
        chain_id: topology.chain_id.clone(),
        genesis_hash: topology.genesis_hash.clone(),
        protocol_version: topology.protocol_version,
        source: source.to_string(),
        target: target.to_string(),
        batch_kind: batch_kind.to_string(),
        block_height,
        certificate_id: certificate_id.to_string(),
        block_hash: block_hash.to_string(),
        expected_state_root: expected_state_root.to_string(),
        batch_file: durable_batch_file.display().to_string(),
        batch_hash,
        certificate_file: durable_certificate_file.display().to_string(),
        certificate_hash,
        timeout_ms,
        send_retries,
        retry_backoff_ms,
        attempt_count: 0,
        completed: false,
        last_error: None,
        ack: None,
    };

    let staging_dir = next_certified_send_staging_dir(&outbox_dir, &job_id)?;
    let staging_batch_file = staging_dir.join("batch.json");
    let staging_certificate_file = staging_dir.join("certificate.json");
    let staging_job_file = staging_dir.join("job.json");
    postfiat_storage::atomic_write(&staging_batch_file, &batch_bytes)
        .map_err(|error| format!("certified send durable staging batch write failed: {error}"))?;
    postfiat_storage::atomic_write(&staging_certificate_file, &certificate_bytes).map_err(
        |error| format!("certified send durable staging certificate write failed: {error}"),
    )?;
    write_durable_certified_send_job(&staging_job_file, &job)?;
    sync_certified_send_directory(&staging_dir, "staging directory")?;
    if job_dir.exists() {
        return Err(format!(
            "certified send job `{job_id}` appeared while its complete staging directory was being published"
        ));
    }
    std::fs::rename(&staging_dir, &job_dir).map_err(|error| {
        format!(
            "certified send staging publish `{}` -> `{}` failed: {error}",
            staging_dir.display(),
            job_dir.display()
        )
    })?;
    sync_certified_send_directory(&outbox_dir, "outbox directory after job publish")?;
    let published = read_durable_certified_send_job(&job_file)?;
    validate_durable_certified_send_payloads(&job_file, &published)?;
    Ok(job_file)
}

fn read_durable_certified_send_job(path: &Path) -> Result<DurableCertifiedSendJob, String> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "certified send job metadata `{}` failed: {error}",
            path.display()
        )
    })?;
    if metadata.len() > CERTIFIED_SEND_JOB_MAX_BYTES {
        return Err(format!(
            "certified send job `{}` exceeded {CERTIFIED_SEND_JOB_MAX_BYTES} bytes",
            path.display()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("certified send job read `{}` failed: {error}", path.display()))?;
    let job: DurableCertifiedSendJob = serde_json::from_slice(&bytes)
        .map_err(|error| format!("certified send job parse `{}` failed: {error}", path.display()))?;
    if job.schema != CERTIFIED_SEND_JOB_SCHEMA {
        return Err(format!(
            "certified send job schema `{}` is not supported",
            job.schema
        ));
    }
    let expected_job_id = durable_certified_send_job_id(
        &job.topology_id,
        &job.source,
        &job.target,
        job.block_height,
        &job.certificate_id,
        &job.block_hash,
    );
    if job.job_id != expected_job_id {
        return Err("certified send job id does not match canonical identity".to_string());
    }
    Ok(job)
}

fn write_durable_certified_send_job(
    path: &Path,
    job: &DurableCertifiedSendJob,
) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(job)
        .map_err(|error| format!("certified send job serialization failed: {error}"))?;
    if json.len() as u64 > CERTIFIED_SEND_JOB_MAX_BYTES {
        return Err(format!(
            "certified send job exceeded {CERTIFIED_SEND_JOB_MAX_BYTES} bytes"
        ));
    }
    postfiat_storage::atomic_write(path, json)
        .map_err(|error| format!("certified send job atomic write failed: {error}"))
}

fn cleanup_certified_send_completed_retention_dir(data_dir: &Path) -> Result<usize, String> {
    let retention_dir = certified_send_completed_retention_dir(data_dir);
    if !require_certified_send_directory(&retention_dir, "completed retention directory")? {
        return Ok(0);
    }
    let mut retired = Vec::new();
    for entry in std::fs::read_dir(&retention_dir)
        .map_err(|error| format!("certified send completed retention read failed: {error}"))?
    {
        let entry = entry
            .map_err(|error| format!("certified send completed retention entry failed: {error}"))?;
        let name = entry.file_name();
        let job_id = name.to_str().ok_or_else(|| {
            format!(
                "certified send completed retention entry `{}` is not valid UTF-8",
                entry.path().display()
            )
        })?;
        if !certified_send_job_id_is_canonical(job_id) {
            return Err(format!(
                "certified send completed retention entry `{}` has a non-canonical name",
                entry.path().display()
            ));
        }
        retired.push((entry.path(), job_id.to_string()));
    }
    retired.sort_by(|left, right| left.0.cmp(&right.0));
    for (directory, job_id) in &retired {
        remove_certified_send_disposable_job_dir(directory, job_id)?;
    }
    Ok(retired.len())
}

fn validated_completed_durable_certified_send_jobs(
    data_dir: &Path,
) -> Result<Vec<(PathBuf, DurableCertifiedSendJob)>, String> {
    let completed_dir = certified_send_completed_dir(data_dir);
    if !require_certified_send_directory(&completed_dir, "completed directory")? {
        return Ok(Vec::new());
    }
    let mut completed = Vec::new();
    for entry in std::fs::read_dir(&completed_dir)
        .map_err(|error| format!("certified send completed retention scan failed: {error}"))?
    {
        let entry = entry
            .map_err(|error| format!("certified send completed retention entry failed: {error}"))?;
        let name = entry.file_name();
        let job_id = name.to_str().ok_or_else(|| {
            format!(
                "certified send completed entry `{}` is not valid UTF-8",
                entry.path().display()
            )
        })?;
        if !certified_send_job_id_is_canonical(job_id) {
            return Err(format!(
                "certified send completed entry `{}` has a non-canonical name",
                entry.path().display()
            ));
        }
        if !require_certified_send_directory(&entry.path(), "completed job directory")? {
            return Err(format!(
                "certified send completed job directory `{}` disappeared during retention scan",
                entry.path().display()
            ));
        }
        let job_file = entry.path().join("job.json");
        let job_metadata = std::fs::symlink_metadata(&job_file).map_err(|error| {
            format!(
                "certified send completed job metadata `{}` failed: {error}",
                job_file.display()
            )
        })?;
        if job_metadata.file_type().is_symlink() || !job_metadata.is_file() {
            return Err(format!(
                "certified send completed job `{}` must be a non-symlink regular file",
                job_file.display()
            ));
        }
        let job = read_durable_certified_send_job(&job_file)?;
        if job.job_id != job_id {
            return Err(format!(
                "certified send completed directory `{job_id}` conflicts with job id `{}`",
                job.job_id
            ));
        }
        validate_completed_durable_certified_send_job(&job_file, &job)?;
        completed.push((entry.path(), job));
    }
    completed.sort_by(|(_, left), (_, right)| {
        left.block_height
            .cmp(&right.block_height)
            .then_with(|| left.job_id.cmp(&right.job_id))
    });
    Ok(completed)
}

fn prune_completed_durable_certified_send_jobs(
    data_dir: &Path,
    max_tombstones: usize,
) -> Result<usize, String> {
    cleanup_certified_send_completed_retention_dir(data_dir)?;
    let completed = validated_completed_durable_certified_send_jobs(data_dir)?;
    let prune_count = completed.len().saturating_sub(max_tombstones);
    if prune_count == 0 {
        return Ok(0);
    }

    let outbox_dir = certified_send_outbox_dir(data_dir);
    let resolved_dir = certified_send_resolved_quarantine_dir(data_dir);
    let retention_dir = certified_send_completed_retention_dir(data_dir);
    std::fs::create_dir_all(&retention_dir).map_err(|error| {
        format!(
            "certified send completed retention directory create `{}` failed: {error}",
            retention_dir.display()
        )
    })?;
    sync_certified_send_directory(&outbox_dir, "outbox directory after retention create")?;
    sync_certified_send_directory(
        &resolved_dir,
        "resolved quarantine directory after retention create",
    )?;
    let completed_dir = certified_send_completed_dir(data_dir);
    for (source, job) in completed.into_iter().take(prune_count) {
        let destination = retention_dir.join(&job.job_id);
        if destination.exists() {
            return Err(format!(
                "certified send completed retention destination `{}` already exists",
                destination.display()
            ));
        }
        // Move out of the scanner-visible completed set atomically before
        // recursive deletion. A crash during deletion leaves only a canonical
        // disposable directory in the scanner-ignored retention root.
        std::fs::rename(&source, &destination).map_err(|error| {
            format!(
                "certified send completed retention move `{}` -> `{}` failed: {error}",
                source.display(),
                destination.display()
            )
        })?;
        sync_certified_send_directory(&completed_dir, "completed directory after retention move")?;
        sync_certified_send_directory(&retention_dir, "completed retention directory after move")?;
        remove_certified_send_disposable_job_dir(&destination, &job.job_id)?;
    }
    Ok(prune_count)
}

fn compact_completed_durable_certified_send_jobs(data_dir: &Path) -> Result<usize, String> {
    cleanup_orphan_certified_send_staging_dirs(data_dir)?;
    let outbox_dir = certified_send_outbox_dir(data_dir);
    if !outbox_dir.exists() {
        return Ok(0);
    }
    cleanup_certified_send_completed_retention_dir(data_dir)?;
    validated_completed_durable_certified_send_jobs(data_dir)?;
    let completed_dir = outbox_dir.join("completed");
    std::fs::create_dir_all(&completed_dir)
        .map_err(|error| format!("certified send completed directory create failed: {error}"))?;
    let mut job_dirs = std::fs::read_dir(&outbox_dir)
        .map_err(|error| format!("certified send outbox compaction read failed: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path != &completed_dir && path.join("job.json").is_file())
        .collect::<Vec<_>>();
    job_dirs.sort();
    let mut compacted = 0usize;
    for job_dir in job_dirs {
        let job_file = job_dir.join("job.json");
        let job = read_durable_certified_send_job(&job_file)?;
        if certified_send_quarantine_job_dir(data_dir, &job.job_id).exists() {
            continue;
        }
        if !job.completed {
            continue;
        }
        if let Err(detail) = validate_completed_durable_certified_send_job(&job_file, &job) {
            let error = format!(
                "certified send completed job `{}` is invalid before compaction: {detail}",
                job.job_id
            );
            quarantine_durable_certified_send_job(
                data_dir,
                &job.job_id,
                &job_file,
                &error,
            )?;
            return Err(error);
        }
        let destination = completed_dir.join(&job.job_id);
        if destination.exists() {
            let error = format!(
                "certified send completed record `{}` already exists",
                destination.display()
            );
            quarantine_durable_certified_send_job(
                data_dir,
                &job.job_id,
                &job_file,
                &error,
            )?;
            return Err(error);
        }
        std::fs::rename(&job_dir, &destination).map_err(|error| {
            format!(
                "certified send completed job move `{}` failed: {error}",
                job.job_id
            )
        })?;
        compacted = compacted.saturating_add(1);
    }
    #[cfg(unix)]
    {
        std::fs::File::open(&outbox_dir)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("certified send outbox directory sync failed: {error}"))?;
        std::fs::File::open(&completed_dir)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("certified send completed directory sync failed: {error}"))?;
    }
    prune_completed_durable_certified_send_jobs(
        data_dir,
        CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS,
    )?;
    Ok(compacted)
}

fn validate_durable_certified_send_payloads(
    job_file: &Path,
    job: &DurableCertifiedSendJob,
) -> Result<(), String> {
    let job_dir = job_file
        .parent()
        .ok_or_else(|| "certified send job file has no parent directory".to_string())?;
    let batch_file = job_dir.join("batch.json");
    let certificate_file = job_dir.join("certificate.json");
    let legacy_job_dir = job_dir
        .parent()
        .filter(|parent| parent.file_name().and_then(|value| value.to_str()) == Some("completed"))
        .and_then(Path::parent)
        .map(|outbox_dir| outbox_dir.join(&job.job_id));
    let stored_batch_file = Path::new(&job.batch_file);
    let stored_certificate_file = Path::new(&job.certificate_file);
    let batch_path_valid = stored_batch_file == batch_file
        || legacy_job_dir
            .as_ref()
            .is_some_and(|legacy| stored_batch_file == legacy.join("batch.json"));
    let certificate_path_valid = stored_certificate_file == certificate_file
        || legacy_job_dir
            .as_ref()
            .is_some_and(|legacy| stored_certificate_file == legacy.join("certificate.json"));
    if !batch_path_valid || !certificate_path_valid {
        return Err("certified send durable payload path is not canonical".to_string());
    }
    let batch_bytes = std::fs::read(&batch_file)
        .map_err(|error| format!("certified send durable batch read failed: {error}"))?;
    let certificate_bytes = std::fs::read(&certificate_file)
        .map_err(|error| format!("certified send durable certificate read failed: {error}"))?;
    if batch_bytes.len() as u64 > MAX_TRANSPORT_FRAME_BYTES
        || certificate_bytes.len() as u64 > MAX_TRANSPORT_FRAME_BYTES
    {
        return Err("certified send durable payload exceeded transport bound".to_string());
    }
    let batch_hash = postfiat_crypto_provider::hash_hex(
        "postfiat.certified_send_job.batch.v1",
        &batch_bytes,
    );
    let certificate_hash = postfiat_crypto_provider::hash_hex(
        "postfiat.certified_send_job.certificate.v1",
        &certificate_bytes,
    );
    if batch_hash != job.batch_hash || certificate_hash != job.certificate_hash {
        return Err("certified send durable payload hash mismatch".to_string());
    }
    Ok(())
}

fn validate_completed_durable_certified_send_job(
    job_file: &Path,
    job: &DurableCertifiedSendJob,
) -> Result<(), String> {
    if !job.completed {
        return Err("certified send tombstone is not marked complete".to_string());
    }
    let ack = job
        .ack
        .as_ref()
        .ok_or_else(|| "certified send tombstone is missing its acknowledgement".to_string())?;
    if ack.block_height != job.block_height
        || ack.block_tip_hash != job.block_hash
        || ack.state_root != job.expected_state_root
    {
        return Err(
            "certified send tombstone acknowledgement conflicts with height/hash/root"
                .to_string(),
        );
    }
    validate_durable_certified_send_payloads(job_file, job)
}

fn truncate_certified_send_error(error: String) -> String {
    error.chars().take(CERTIFIED_SEND_ERROR_MAX_CHARS).collect()
}

fn complete_durable_certified_send_job(
    data_dir: &Path,
    job_file: &Path,
    job: &mut DurableCertifiedSendJob,
    ack: DurableCertifiedSendAck,
) -> Result<(), String> {
    if ack.block_height != job.block_height
        || ack.block_tip_hash != job.block_hash
        || ack.state_root != job.expected_state_root
    {
        let error = format!(
            "certified send ack from `{}` conflicts: expected height/hash/root {}/{}/{}, observed {}/{}/{}",
            job.target,
            job.block_height,
            job.block_hash,
            job.expected_state_root,
            ack.block_height,
            ack.block_tip_hash,
            ack.state_root
        );
        quarantine_durable_certified_send_job(data_dir, &job.job_id, job_file, &error)?;
        job.last_error = Some(truncate_certified_send_error(error.clone()));
        write_durable_certified_send_job(job_file, job)?;
        return Err(error);
    }
    job.completed = true;
    job.last_error = None;
    job.ack = Some(ack);
    write_durable_certified_send_job(job_file, job)
}

fn completed_durable_certified_send_report(
    job_file: &Path,
    job: &DurableCertifiedSendJob,
    topology: &NetworkTopology,
) -> Result<TransportBatchSendReport, String> {
    validate_completed_durable_certified_send_job(job_file, job)?;
    let peer = topology
        .peer(&job.target)
        .ok_or_else(|| format!("certified send tombstone target `{}` is not a peer", job.target))?;
    let ack = job
        .ack
        .as_ref()
        .ok_or_else(|| "certified send tombstone acknowledgement is missing".to_string())?;
    let payload_len = std::fs::metadata(
        job_file
            .parent()
            .ok_or_else(|| "certified send tombstone has no parent directory".to_string())?
            .join("batch.json"),
    )
    .map_err(|error| format!("certified send tombstone batch metadata failed: {error}"))?
    .len();
    let message_id = job.job_id.clone();
    let payload_hash = job.batch_hash.clone();
    Ok(TransportBatchSendReport {
        schema: "postfiat-transport-batch-send-v1".to_string(),
        from: job.source.clone(),
        to: job.target.clone(),
        topology_id: job.topology_id.clone(),
        peer_address: socket_address(&peer.host, peer.p2p_port),
        attempts: 0,
        max_attempts: 0,
        retry_backoff_ms: 0,
        retry_errors: Vec::new(),
        sent: TransportBatchSummary {
            from: job.source.clone(),
            to: job.target.clone(),
            batch_kind: job.batch_kind.clone(),
            message_id: message_id.clone(),
            payload_hash: payload_hash.clone(),
            payload_len,
            certificate_attached: true,
        },
        ack: TransportBatchAck {
            schema: TRANSPORT_BATCH_ACK_SCHEMA.to_string(),
            topology_id: job.topology_id.clone(),
            from: job.target.clone(),
            to: job.source.clone(),
            message_id,
            payload_hash,
            applied: true,
            already_applied: true,
            receipt_count: 0,
            accepted_count: 0,
            rejected_count: 0,
            certificate_attached: true,
            certified_state: Some(TransportHello {
                schema: TRANSPORT_HELLO_SCHEMA.to_string(),
                topology_id: job.topology_id.clone(),
                node_id: job.target.clone(),
                chain_id: job.chain_id.clone(),
                genesis_hash: job.genesis_hash.clone(),
                protocol_version: job.protocol_version,
                state_root: ack.state_root.clone(),
                block_height: ack.block_height,
                block_tip_hash: ack.block_tip_hash.clone(),
            }),
            state: TransportHello {
                schema: TRANSPORT_HELLO_SCHEMA.to_string(),
                topology_id: job.topology_id.clone(),
                node_id: job.target.clone(),
                chain_id: job.chain_id.clone(),
                genesis_hash: job.genesis_hash.clone(),
                protocol_version: job.protocol_version,
                state_root: ack.state_root.clone(),
                block_height: ack.block_height,
                block_tip_hash: ack.block_tip_hash.clone(),
            },
        },
        verified: true,
    })
}

fn ensure_durable_certified_send_local_block(
    data_dir: &Path,
    job_file: &Path,
    job: &DurableCertifiedSendJob,
    local_status: &StatusReport,
) -> Result<StatusReport, String> {
    let mut current = local_status.clone();
    if current.block_height < job.block_height {
        if current.block_height.saturating_add(1) != job.block_height {
            return Err(format!(
                "certified send source is at height {}, more than one block behind job height {}",
                current.block_height, job.block_height
            ));
        }
        let job_dir = job_file
            .parent()
            .ok_or_else(|| "certified send job has no parent directory".to_string())?;
        let batch_file = job_dir.join("batch.json");
        let certificate_file = job_dir.join("certificate.json");
        if job.batch_kind == "transparent" {
            apply_batch_with_expected_commit_identity(
                ApplyBatchOptions {
                    data_dir: data_dir.to_path_buf(),
                    batch_file,
                    certificate_file: Some(certificate_file),
                },
                &ExpectedBatchCommitIdentity {
                    block_height: job.block_height,
                    block_hash: job.block_hash.clone(),
                    state_root: job.expected_state_root.clone(),
                    certificate_id: job.certificate_id.clone(),
                },
            )
            .map_err(|error| format!("certified send local recovery apply failed: {error}"))?;
        } else {
            transport_runtime::apply_transport_batch_with_timings(
                data_dir,
                &job.batch_kind,
                batch_file,
                Some(certificate_file),
                None,
            )
            .map_err(|error| format!("certified send local recovery apply failed: {error}"))?;
        }
        current = status(NodeOptions {
            data_dir: data_dir.to_path_buf(),
        })
        .map_err(|error| format!("certified send local recovery status failed: {error}"))?;
    }

    if current.block_height == job.block_height {
        if current.block_tip_hash != job.block_hash
            || current.state_root != job.expected_state_root
        {
            return Err(format!(
                "certified send source height {} conflicts with expected hash/root",
                job.block_height
            ));
        }
        return Ok(current);
    }
    if current.block_height < job.block_height {
        return Err(format!(
            "certified send source remained at height {} below job height {}",
            current.block_height, job.block_height
        ));
    }

    let blocks = postfiat_storage::NodeStore::new(data_dir)
        .read_blocks()
        .map_err(|error| format!("certified send source block history read failed: {error}"))?;
    let block = blocks
        .blocks
        .iter()
        .find(|block| block.header.height == job.block_height)
        .ok_or_else(|| {
            format!(
                "certified send source block {} is unavailable for high-water verification",
                job.block_height
            )
        })?;
    if block.header.block_hash != job.block_hash
        || block.header.state_root != job.expected_state_root
    {
        return Err(format!(
            "certified send source history at height {} conflicts with expected hash/root",
            job.block_height
        ));
    }
    Ok(current)
}

fn send_durable_certified_send_job(
    job_file: &Path,
    data_dir: &Path,
    topology_file: &Path,
) -> Result<TransportBatchSendReport, String> {
    let mut job = read_durable_certified_send_job(job_file)?;
    let topology_result = read_topology_file(&topology_file.to_path_buf());
    let quarantine_record_file =
        certified_send_quarantine_record_file(data_dir, &job.job_id);
    if quarantine_record_file.is_file() {
        let record = read_durable_certified_send_quarantine_record(&quarantine_record_file)?;
        let recoverable = topology_result.as_ref().ok().is_some_and(|topology| {
            rotated_topology_ack_quarantine_is_recoverable(job_file, &job, topology, &record)
        });
        if !recoverable {
            ensure_durable_certified_send_job_not_quarantined(data_dir, &job.job_id)?;
        }
    }
    let topology = topology_result?;
    if let Err(detail) = validate_durable_certified_send_payloads(job_file, &job) {
        let error = format!(
            "certified send job `{}` has invalid durable payloads: {detail}",
            job.job_id
        );
        quarantine_durable_certified_send_job(data_dir, &job.job_id, job_file, &error)?;
        return Err(error);
    }
    let local_status = status(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .map_err(|error| format!("certified send outbox local status failed: {error}"))?;
    validate_durable_certified_send_current_deployment(&job, &topology, &local_status)?;
    if job.completed {
        return completed_durable_certified_send_report(job_file, &job, &topology);
    }
    if let Err(error) = ensure_durable_certified_send_local_block(
        data_dir,
        job_file,
        &job,
        &local_status,
    ) {
        job.last_error = Some(truncate_certified_send_error(error.clone()));
        write_durable_certified_send_job(job_file, &job)?;
        return Err(error);
    }

    job.attempt_count = job.attempt_count.saturating_add(1);
    job.last_error = None;
    write_durable_certified_send_job(job_file, &job)?;
    let job_dir = job_file
        .parent()
        .ok_or_else(|| "certified send job has no parent directory".to_string())?;
    let result = transport_batch_send_with_retries(
        data_dir.to_path_buf(),
        topology_file.to_path_buf(),
        job.target.clone(),
        Some(job.batch_kind.clone()),
        job_dir.join("batch.json"),
        Some(job_dir.join("certificate.json")),
        job.timeout_ms,
        job.send_retries,
        job.retry_backoff_ms,
    );
    match result {
        Ok(report) => {
            let certified_state = report
                .ack
                .certified_state
                .as_ref()
                .unwrap_or(&report.ack.state);
            let ack = DurableCertifiedSendAck {
                already_applied: report.ack.already_applied,
                block_height: certified_state.block_height,
                block_tip_hash: certified_state.block_tip_hash.clone(),
                state_root: certified_state.state_root.clone(),
            };
            complete_durable_certified_send_job(data_dir, job_file, &mut job, ack)?;
            resolve_rotated_topology_ack_quarantine(data_dir, &job.job_id)?;
            Ok(report)
        }
        Err(error) => {
            job.last_error = Some(truncate_certified_send_error(error.clone()));
            write_durable_certified_send_job(job_file, &job)?;
            Err(error)
        }
    }
}

fn validate_durable_certified_send_current_deployment(
    job: &DurableCertifiedSendJob,
    topology: &NetworkTopology,
    local_status: &StatusReport,
) -> Result<(), String> {
    if topology.chain_id != job.chain_id
        || topology.genesis_hash != job.genesis_hash
        || topology.protocol_version != job.protocol_version
        || local_status.node_id != job.source
        || local_status.chain_id != job.chain_id
        || local_status.genesis_hash != job.genesis_hash
        || local_status.protocol_version != job.protocol_version
    {
        return Err(format!(
            "certified send job `{}` does not match local deployment identity",
            job.job_id
        ));
    }
    if topology.peer(&job.source).is_none() || topology.peer(&job.target).is_none() {
        return Err(format!(
            "certified send job `{}` source or target is absent from the current topology",
            job.job_id
        ));
    }
    Ok(())
}

fn sort_durable_certified_send_jobs_for_resume(
    jobs: &mut [(PathBuf, DurableCertifiedSendJob)],
) {
    jobs.sort_by(|(_, left), (_, right)| {
        left.target
            .cmp(&right.target)
            .then_with(|| left.block_height.cmp(&right.block_height))
            .then_with(|| left.job_id.cmp(&right.job_id))
    });
}

fn resume_durable_certified_send_outbox(
    data_dir: &Path,
    topology_file: &Path,
    max_jobs: usize,
) -> Result<DurableCertifiedSendResumeReport, String> {
    if max_jobs == 0 || max_jobs > CERTIFIED_SEND_OUTBOX_MAX_JOBS {
        return Err(format!(
            "certified send resume max jobs must be between 1 and {CERTIFIED_SEND_OUTBOX_MAX_JOBS}"
        ));
    }
    compact_completed_durable_certified_send_jobs(data_dir)?;
    let outbox_dir = certified_send_outbox_dir(data_dir);
    if !outbox_dir.exists() {
        return Ok(DurableCertifiedSendResumeReport {
            schema: "postfiat-certified-send-outbox-resume-v1".to_string(),
            outbox_dir: outbox_dir.display().to_string(),
            discovered: 0,
            attempted: 0,
            completed: 0,
            pending: 0,
            quarantined: 0,
            targets: Vec::new(),
            all_completed: true,
        });
    }
    let mut job_files = std::fs::read_dir(&outbox_dir)
        .map_err(|error| format!("certified send outbox read failed: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("job.json"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    job_files.sort();
    let mut jobs = Vec::with_capacity(job_files.len());
    for job_file in job_files {
        let job = read_durable_certified_send_job(&job_file)?;
        jobs.push((job_file, job));
    }
    sort_durable_certified_send_jobs_for_resume(&mut jobs);
    if jobs.len() > CERTIFIED_SEND_OUTBOX_MAX_JOBS {
        return Err(format!(
            "certified send outbox contains {} active jobs, exceeding bound {CERTIFIED_SEND_OUTBOX_MAX_JOBS}",
            jobs.len()
        ));
    }
    let current_topology = read_topology_file(&topology_file.to_path_buf()).ok();

    let quarantine_dir = certified_send_quarantine_dir(data_dir);
    let mut quarantine_entries = if quarantine_dir.exists() {
        std::fs::read_dir(&quarantine_dir)
            .map_err(|error| format!("certified send quarantine read failed: {error}"))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.path())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    quarantine_entries.sort();
    if quarantine_entries.len() > CERTIFIED_SEND_OUTBOX_MAX_JOBS {
        return Err(format!(
            "certified send outbox contains {} quarantine records, exceeding bound {CERTIFIED_SEND_OUTBOX_MAX_JOBS}",
            quarantine_entries.len()
        ));
    }

    let mut discovered_job_ids = jobs
        .iter()
        .map(|(_, job)| job.job_id.clone())
        .collect::<BTreeSet<_>>();
    let mut quarantined_job_ids = BTreeSet::new();
    let mut attempted = 0usize;
    let mut completed = 0usize;
    let mut pending = 0usize;
    let mut quarantined = 0usize;
    let mut targets = Vec::new();
    for quarantine_entry in quarantine_entries {
        let Some(job_id) = quarantine_entry
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        discovered_job_ids.insert(job_id.clone());
        let record_file = quarantine_entry.join("quarantine.json");
        let record = read_durable_certified_send_quarantine_record(&record_file);
        let active_job = jobs
            .iter()
            .find(|(_, job)| job.job_id == job_id);
        let recoverable = active_job
            .zip(record.as_ref().ok())
            .zip(current_topology.as_ref())
            .is_some_and(|(((job_file, job), record), current_topology)| {
                rotated_topology_ack_quarantine_is_recoverable(
                    job_file,
                    job,
                    &current_topology,
                    record,
                )
            });
        if recoverable {
            continue;
        }
        quarantined_job_ids.insert(job_id.clone());
        pending = pending.saturating_add(1);
        quarantined = quarantined.saturating_add(1);
        let target = active_job
            .map(|(_, job)| job.target.clone())
            .or_else(|| {
                record.as_ref().ok().and_then(|record| {
                    read_durable_certified_send_job(Path::new(&record.job_file))
                        .ok()
                        .map(|job| job.target)
                })
            })
            .unwrap_or_else(|| "unknown".to_string());
        let error = match record {
            Ok(record) => format!(
                "{}; quarantine record `{}`",
                record.reason,
                record_file.display()
            ),
            Err(error) => format!(
                "certified send quarantine marker `{}` is incomplete or corrupt: {error}",
                quarantine_entry.display()
            ),
        };
        targets.push(DurableCertifiedSendResumeTargetReport {
            job_id,
            target,
            result: "quarantined".to_string(),
            already_applied: false,
            attempts: 0,
            error: Some(truncate_certified_send_error(error)),
        });
    }

    for (job_file, job) in jobs {
        if quarantined_job_ids.contains(&job.job_id) {
            continue;
        }
        if job.completed {
            completed = completed.saturating_add(1);
            continue;
        }
        if attempted >= max_jobs {
            pending = pending.saturating_add(1);
            continue;
        }
        attempted = attempted.saturating_add(1);
        match send_durable_certified_send_job(&job_file, data_dir, topology_file) {
            Ok(report) => {
                completed = completed.saturating_add(1);
                targets.push(DurableCertifiedSendResumeTargetReport {
                    job_id: job.job_id,
                    target: job.target,
                    result: "completed".to_string(),
                    already_applied: report.ack.already_applied,
                    attempts: report.attempts,
                    error: None,
                });
            }
            Err(error) => {
                pending = pending.saturating_add(1);
                targets.push(DurableCertifiedSendResumeTargetReport {
                    job_id: job.job_id,
                    target: job.target,
                    result: "pending".to_string(),
                    already_applied: false,
                    attempts: 0,
                    error: Some(truncate_certified_send_error(error)),
                });
            }
        }
    }
    Ok(DurableCertifiedSendResumeReport {
        schema: "postfiat-certified-send-outbox-resume-v1".to_string(),
        outbox_dir: outbox_dir.display().to_string(),
        discovered: discovered_job_ids.len(),
        attempted,
        completed,
        pending,
        quarantined,
        all_completed: pending == 0,
        targets,
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn_deferred_certified_batch_send(
    job_file: &Path,
    data_dir: &Path,
    topology_file: &Path,
) -> Result<TransportPeerDeferredSendJobReport, String> {
    let job = read_durable_certified_send_job(job_file)?;
    let send_dir = job_file
        .parent()
        .ok_or_else(|| "certified send job has no parent directory".to_string())?;
    let target = job.target.as_str();
    let target_component = transport_artifact_component(target);
    let report_file = send_dir.join(format!("{target_component}.send.json"));
    let stderr_file = send_dir.join(format!("{target_component}.send.err"));
    let data_dir = data_dir.to_path_buf();
    let topology_file = topology_file.to_path_buf();
    let job_file_owned = job_file.to_path_buf();
    let report_file_for_thread = report_file.clone();
    let stderr_file_for_thread = stderr_file.clone();
    std::thread::Builder::new()
        .name(format!("certified-send-{target_component}"))
        .spawn(move || {
            let result = send_durable_certified_send_job(
                &job_file_owned,
                &data_dir,
                &topology_file,
            );
            match result {
                Ok(report) => match serde_json::to_string_pretty(&report) {
                    Ok(json) => {
                        if let Err(error) = std::fs::write(&report_file_for_thread, json) {
                            let _ = std::fs::write(
                                &stderr_file_for_thread,
                                format!("deferred certified send report write failed: {error}\n"),
                            );
                        }
                    }
                    Err(error) => {
                        let _ = std::fs::write(
                            &stderr_file_for_thread,
                            format!("deferred certified send report serialization failed: {error}\n"),
                        );
                    }
                },
                Err(error) => {
                    let _ = std::fs::write(&stderr_file_for_thread, format!("{error}\n"));
                }
            }
        })
        .map_err(|error| {
            format!("deferred certified send thread spawn to `{target}` failed: {error}")
        })?;
    Ok(TransportPeerDeferredSendJobReport {
        schema: "postfiat-deferred-certified-batch-send-job-v2".to_string(),
        job_id: job.job_id,
        target: target.to_string(),
        mode: "durable-outbox-plus-in-process-worker".to_string(),
        pid: std::process::id(),
        job_file: job_file.display().to_string(),
        report_file: report_file.display().to_string(),
        stderr_file: stderr_file.display().to_string(),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerCertifiedBatchRoundReport {
    schema: String,
    from: String,
    topology_id: String,
    peer_count: usize,
    target_peer_count: usize,
    batch_file: String,
    artifact_dir: String,
    proposal_signed: bool,
    proposal_signature_signer: Option<String>,
    proposal_proposer: String,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    certified_sends_deferred: bool,
    deferred_certified_send_jobs: Vec<TransportPeerDeferredSendJobReport>,
    certification: BatchCertificateRoundReport,
    local_vote_file: String,
    vote_requests: Vec<TransportBlockVoteRequestReport>,
    vote_request_failures: Vec<TransportPeerFailureReport>,
    unresolved_vote_targets: Vec<String>,
    sends: Vec<TransportBatchSendReport>,
    send_failures: Vec<TransportPeerFailureReport>,
    skipped_certified_send_targets: Vec<String>,
    send_retries: usize,
    retry_backoff_ms: u64,
    vote_request_quorum: usize,
    required_remote_vote_count: usize,
    vote_request_quorum_early: bool,
    retry_vote_request_count: u64,
    retry_vote_request_error_count: u64,
    retry_send_count: u64,
    retry_error_count: u64,
    remote_vote_count: u64,
    failed_vote_request_count: u64,
    failed_send_count: u64,
    local_receipt_count: u64,
    local_accepted_count: u64,
    local_rejected_count: u64,
    local_hot_finality: Vec<TxFinalityReport>,
    local_apply_verified: bool,
    local_state: TransportHello,
    timings: TransportPeerCertifiedBatchRoundTimingsReport,
    all_vote_requests_verified: bool,
    all_sends_verified: bool,
    round_ok: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerCertifiedMempoolRoundReport {
    schema: String,
    node_id: String,
    topology_id: String,
    batch_file: String,
    artifact_dir: String,
    max_transactions: usize,
    signed_transfer_file: Option<String>,
    signed_transfer_json_supplied: bool,
    signed_payment_v2_json_supplied: bool,
    signed_asset_transaction_json_supplied: bool,
    signed_atomic_swap_transaction_json_supplied: bool,
    signed_escrow_transaction_json_supplied: bool,
    submitted_tx_id: Option<String>,
    mempool_submit_ms: f64,
    mempool_batch_ms: f64,
    round: TransportPeerCertifiedBatchRoundReport,
    round_ok: bool,
}

#[derive(Debug, Clone)]
struct TxLatencyBenchmarkOptions {
    base_dir: PathBuf,
    topology_file: PathBuf,
    wallet_key_file: PathBuf,
    wallet_address: String,
    recipient: String,
    amount: u64,
    validators: usize,
    rounds: usize,
    vote_policy: String,
    artifact_root: PathBuf,
    report_file: PathBuf,
    iterations_file: Option<PathBuf>,
    build_mode: String,
    generated_utc: Option<String>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TxLatencyBenchmarkIterationReport {
    iteration: usize,
    source_node: String,
    tx_id: String,
    block_height: u64,
    block_hash: String,
    certificate_id: String,
    vote_policy: String,
    validators: usize,
    quorum: usize,
    vote_count: usize,
    quote_ms: f64,
    wallet_sign_ms: f64,
    mempool_submit_ms: f64,
    mempool_batch_ms: f64,
    wallet_to_finality_ms: f64,
    admitted_to_finality_ms: f64,
    consensus_round_ms: f64,
    round_function_return_ms: f64,
    certified_sends_ms: f64,
    local_apply_ms: f64,
    write_commit_ms: f64,
    refresh_account_tx_index_ms: f64,
    receipt_accepted: bool,
    finality_confirmed: bool,
    round_ok: bool,
    all_vote_requests_verified: bool,
    all_sends_verified: bool,
    round_timings: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TxLatencyStatsReport {
    count: usize,
    min_ms: Option<f64>,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    p99_ms: Option<f64>,
    max_ms: Option<f64>,
    mean_ms: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TxLatencyBenchmarkReport {
    schema: String,
    generated_utc: String,
    status: String,
    config: serde_json::Value,
    latency: serde_json::Value,
    checks: serde_json::Value,
    not_measured: Vec<String>,
    final_state: serde_json::Value,
    iterations_file: Option<String>,
    iterations: Vec<TxLatencyBenchmarkIterationReport>,
}

fn tx_latency_percentile(values: &[f64], pct: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|left, right| left.total_cmp(right));
    let rank = ((pct / 100.0) * ordered.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(ordered.len().saturating_sub(1));
    ordered.get(index).copied()
}

fn tx_latency_stats(values: &[f64]) -> TxLatencyStatsReport {
    if values.is_empty() {
        return TxLatencyStatsReport {
            count: 0,
            min_ms: None,
            p50_ms: None,
            p95_ms: None,
            p99_ms: None,
            max_ms: None,
            mean_ms: None,
        };
    }
    TxLatencyStatsReport {
        count: values.len(),
        min_ms: values.iter().copied().reduce(f64::min),
        p50_ms: tx_latency_percentile(values, 50.0),
        p95_ms: tx_latency_percentile(values, 95.0),
        p99_ms: tx_latency_percentile(values, 99.0),
        max_ms: values.iter().copied().reduce(f64::max),
        mean_ms: Some(values.iter().sum::<f64>() / values.len() as f64),
    }
}

fn tx_latency_generated_utc() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix_seconds:{seconds}")
}

fn tx_latency_validator_data_dir(base_dir: &Path, node_id: &str) -> PathBuf {
    base_dir.join(node_id)
}

fn tx_latency_collect_statuses(
    base_dir: &Path,
    validators: usize,
) -> Result<Vec<postfiat_types::StatusReport>, String> {
    let mut statuses = Vec::with_capacity(validators);
    for index in 0..validators {
        let node_id = format!("validator-{index}");
        let report = status(NodeOptions {
            data_dir: tx_latency_validator_data_dir(base_dir, &node_id),
        })
        .map_err(|error| format!("status for `{node_id}` failed: {error}"))?;
        statuses.push(report);
    }
    Ok(statuses)
}

fn tx_latency_statuses_at_height(
    statuses: &[postfiat_types::StatusReport],
    expected_height: u64,
) -> bool {
    statuses
        .iter()
        .all(|status| status.block_height == expected_height)
}

fn tx_latency_write_json_line(
    path: &Path,
    value: &TxLatencyBenchmarkIterationReport,
) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("iterations file `{}` open failed: {error}", path.display()))?;
    let line = serde_json::to_string(value)
        .map_err(|error| format!("iteration JSON serialization failed: {error}"))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("iterations file `{}` write failed: {error}", path.display()))
}

fn tx_latency_nested_f64(value: &serde_json::Value, path: &[&str]) -> f64 {
    let mut cursor = value;
    for key in path {
        let Some(next) = cursor.get(*key) else {
            return 0.0;
        };
        cursor = next;
    }
    cursor.as_f64().unwrap_or_default()
}

fn tx_latency_benchmark(options: TxLatencyBenchmarkOptions) -> Result<TxLatencyBenchmarkReport, String> {
    if options.validators < 4 {
        return Err("--validators must be at least 4".to_string());
    }
    if options.rounds == 0 {
        return Err("--rounds must be positive".to_string());
    }
    if options.amount == 0 {
        return Err("--amount must be positive".to_string());
    }
    let quorum_fast = match options.vote_policy.as_str() {
        "full" => false,
        "quorum-fast" => true,
        other => {
            return Err(format!(
                "--vote-policy must be full or quorum-fast, got `{other}`"
            ));
        }
    };
    if options.send_retries > MAX_TRANSPORT_SEND_RETRIES {
        return Err(format!(
            "--send-retries must be <= {MAX_TRANSPORT_SEND_RETRIES}"
        ));
    }
    if options.defer_certified_sends && !options.local_apply_before_certified_send {
        return Err(
            "--defer-certified-sends requires --local-apply-before-certified-send".to_string(),
        );
    }
    std::fs::create_dir_all(&options.artifact_root).map_err(|error| {
        format!(
            "tx latency benchmark artifact root `{}` create failed: {error}",
            options.artifact_root.display()
        )
    })?;
    if let Some(iterations_file) = &options.iterations_file {
        if let Some(parent) = iterations_file.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "tx latency benchmark iterations parent `{}` create failed: {error}",
                    parent.display()
                )
            })?;
        }
        std::fs::write(iterations_file, "").map_err(|error| {
            format!(
                "tx latency benchmark iterations file `{}` truncate failed: {error}",
                iterations_file.display()
            )
        })?;
    }

    let initial_statuses = tx_latency_collect_statuses(&options.base_dir, options.validators)?;
    let mut current_height = initial_statuses
        .first()
        .map(|status| status.block_height)
        .ok_or_else(|| "no validator statuses collected".to_string())?;
    if !tx_latency_statuses_at_height(&initial_statuses, current_height) {
        return Err("validators are not at one starting height".to_string());
    }

    let mut iterations = Vec::with_capacity(options.rounds);
    for iteration in 1..=options.rounds {
        let next_height = current_height
            .checked_add(1)
            .ok_or_else(|| "block height overflow".to_string())?;
        let proposer = block_proposer(BlockProposerOptions {
            data_dir: tx_latency_validator_data_dir(&options.base_dir, "validator-0"),
            block_height: next_height,
            view: 0,
        })
        .map_err(|error| format!("block proposer for height {next_height} failed: {error}"))?;
        let source_node = proposer.proposer;
        let source_data_dir = tx_latency_validator_data_dir(&options.base_dir, &source_node);
        let key_file = source_data_dir.join(VALIDATOR_KEYS_FILE);
        let artifact_dir = options.artifact_root.join(format!("round-{iteration:06}"));

        let quote_start = Instant::now();
        let quote = transfer_fee_quote(TransferFeeQuoteOptions {
            data_dir: source_data_dir.clone(),
            from: options.wallet_address.clone(),
            to: options.recipient.clone(),
            amount: options.amount,
            sequence: None,
            memo_type: None,
            memo_format: None,
            memo_data: None,
        })
        .map_err(|error| format!("transfer fee quote for round {iteration} failed: {error}"))?;
        let quote_ms = monotonic_elapsed_ms(quote_start);

        let sign_start = Instant::now();
        let signed = wallet_sign_transfer(WalletSignTransferOptions {
            key_file: options.wallet_key_file.clone(),
            chain_id: quote.chain_id.clone(),
            genesis_hash: quote.genesis_hash.clone(),
            protocol_version: quote.protocol_version,
            to: quote.to.clone(),
            amount: quote.amount,
            fee: quote.minimum_fee,
            sequence: quote.sequence,
        })
        .map_err(|error| format!("wallet sign transfer for round {iteration} failed: {error}"))?;
        let wallet_sign_ms = monotonic_elapsed_ms(sign_start);
        let signed_transfer_json = serde_json::to_string(&signed)
            .map_err(|error| format!("signed transfer JSON serialization failed: {error}"))?;

        let round_return_start = Instant::now();
        let mempool_round = transport_peer_certified_mempool_round(
            TransportPeerCertifiedMempoolRoundOptions {
                data_dir: source_data_dir.clone(),
                topology_file: options.topology_file.clone(),
                key_file: key_file.clone(),
                proposal_key_file: Some(key_file),
                require_local_proposer: true,
                require_signed_proposal: true,
                allow_peer_failures: false,
                quorum_early_full_propagation: quorum_fast,
                artifact_dir,
                block_height: Some(next_height),
                view: Some(0),
                timeout_certificate_file: None,
                timeout_ms: options.timeout_ms,
                send_retries: options.send_retries,
                retry_backoff_ms: options.retry_backoff_ms,
                local_apply_before_certified_send: options.local_apply_before_certified_send,
                defer_certified_sends: options.defer_certified_sends,
                max_transactions: 1,
                signed_transfer_file: None,
                signed_transfer_json: Some(signed_transfer_json),
                signed_payment_v2_json: None,
                signed_asset_transaction_json: None,
                signed_atomic_swap_transaction_json: None,
                signed_escrow_transaction_json: None,
                required_parent: None,
            },
        )?;
        let round_function_return_ms = monotonic_elapsed_ms(round_return_start);
        let tx_id = mempool_round
            .submitted_tx_id
            .clone()
            .ok_or_else(|| format!("round {iteration} did not submit a transaction"))?;
        let finality = mempool_round
            .round
            .local_hot_finality
            .iter()
            .find(|finality| finality.tx_id == tx_id)
            .ok_or_else(|| format!("round {iteration} missing hot finality for `{tx_id}`"))?;
        let receipt_accepted = finality.receipt.accepted;
        let finality_confirmed = finality.confirmed;
        let round_timings = serde_json::to_value(&mempool_round.round.timings)
            .map_err(|error| format!("round timings JSON serialization failed: {error}"))?;
        let consensus_round_ms = mempool_round.round.timings.client_visible_finality_ms;
        let admitted_to_finality_ms =
            mempool_round.mempool_batch_ms + consensus_round_ms;
        let wallet_to_finality_ms =
            wallet_sign_ms + mempool_round.mempool_submit_ms + admitted_to_finality_ms;
        let local_apply_breakdown = round_timings
            .get("local_apply_breakdown")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let write_commit_ms = tx_latency_nested_f64(
            &local_apply_breakdown,
            &["write_commit_ms"],
        );
        let refresh_account_tx_index_ms = tx_latency_nested_f64(
            &local_apply_breakdown,
            &["write_commit_breakdown", "refresh_account_tx_index_ms"],
        );
        let iteration_report = TxLatencyBenchmarkIterationReport {
            iteration,
            source_node: source_node.clone(),
            tx_id: tx_id.clone(),
            block_height: finality.block.header.height,
            block_hash: finality.block.header.block_hash.clone(),
            certificate_id: finality.block.header.certificate_id.clone(),
            vote_policy: options.vote_policy.clone(),
            validators: options.validators,
            quorum: finality.block.header.certificate.quorum as usize,
            vote_count: mempool_round.round.certification.vote_count,
            quote_ms,
            wallet_sign_ms,
            mempool_submit_ms: mempool_round.mempool_submit_ms,
            mempool_batch_ms: mempool_round.mempool_batch_ms,
            wallet_to_finality_ms,
            admitted_to_finality_ms,
            consensus_round_ms,
            round_function_return_ms,
            certified_sends_ms: mempool_round.round.timings.certified_sends_ms,
            local_apply_ms: mempool_round.round.timings.local_apply_ms,
            write_commit_ms,
            refresh_account_tx_index_ms,
            receipt_accepted,
            finality_confirmed,
            round_ok: mempool_round.round_ok,
            all_vote_requests_verified: mempool_round.round.all_vote_requests_verified,
            all_sends_verified: mempool_round.round.all_sends_verified,
            round_timings,
        };
        if let Some(iterations_file) = &options.iterations_file {
            tx_latency_write_json_line(iterations_file, &iteration_report)?;
        }
        iterations.push(iteration_report);

        current_height = next_height;
        let statuses = tx_latency_collect_statuses(&options.base_dir, options.validators)?;
        if !tx_latency_statuses_at_height(&statuses, current_height) {
            return Err(format!(
                "validators did not converge at height {current_height} after round {iteration}"
            ));
        }
    }

    let final_statuses = tx_latency_collect_statuses(&options.base_dir, options.validators)?;
    let mut final_state_reports = Vec::with_capacity(options.validators);
    for index in 0..options.validators {
        let node_id = format!("validator-{index}");
        let report = verify_state(NodeOptions {
            data_dir: tx_latency_validator_data_dir(&options.base_dir, &node_id),
        })
        .map_err(|error| format!("verify-state for `{node_id}` failed: {error}"))?;
        final_state_reports.push(report);
    }

    let wallet_to_finality_values = iterations
        .iter()
        .map(|iteration| iteration.wallet_to_finality_ms)
        .collect::<Vec<_>>();
    let admitted_to_finality_values = iterations
        .iter()
        .map(|iteration| iteration.admitted_to_finality_ms)
        .collect::<Vec<_>>();
    let consensus_round_values = iterations
        .iter()
        .map(|iteration| iteration.consensus_round_ms)
        .collect::<Vec<_>>();
    let refresh_index_values = iterations
        .iter()
        .map(|iteration| iteration.refresh_account_tx_index_ms)
        .collect::<Vec<_>>();

    let mut tx_ids = BTreeSet::new();
    let no_duplicate_receipts = iterations
        .iter()
        .all(|iteration| tx_ids.insert(iteration.tx_id.clone()));
    let final_heights = final_statuses
        .iter()
        .map(|status| status.block_height)
        .collect::<BTreeSet<_>>();
    let final_tips = final_statuses
        .iter()
        .map(|status| status.block_tip_hash.clone())
        .collect::<BTreeSet<_>>();
    let final_roots = final_statuses
        .iter()
        .map(|status| status.state_root.clone())
        .collect::<BTreeSet<_>>();
    let final_height_matches_rounds = final_heights.len() == 1
        && final_heights
            .first()
            .is_some_and(|height| *height == current_height);
    let state_verified_after_run = final_state_reports
        .iter()
        .all(|report| report.verified);
    let all_transactions_final = iterations
        .iter()
        .all(|iteration| iteration.finality_confirmed);
    let all_receipts_accepted = iterations
        .iter()
        .all(|iteration| iteration.receipt_accepted);
    let all_rounds_ok = iterations.iter().all(|iteration| iteration.round_ok);
    let all_vote_policies_match = iterations
        .iter()
        .all(|iteration| iteration.vote_policy == options.vote_policy);
    let account_history_index_not_in_synchronous_finality = refresh_index_values
        .iter()
        .all(|value| *value == 0.0);
    let converged = final_heights.len() == 1 && final_tips.len() == 1 && final_roots.len() == 1;
    let benchmark_ok = iterations.len() == options.rounds
        && all_transactions_final
        && all_receipts_accepted
        && no_duplicate_receipts
        && final_height_matches_rounds
        && state_verified_after_run
        && all_rounds_ok
        && all_vote_policies_match
        && account_history_index_not_in_synchronous_finality
        && converged;

    let report = TxLatencyBenchmarkReport {
        schema: "postfiat-real-transaction-latency-benchmark-v1".to_string(),
        generated_utc: options.generated_utc.unwrap_or_else(tx_latency_generated_utc),
        status: if benchmark_ok { "passed" } else { "failed" }.to_string(),
        config: serde_json::json!({
            "validators": options.validators,
            "rounds": options.rounds,
            "mode": "wallet-to-finality",
            "vote_policy": options.vote_policy,
            "transport": "local-loopback-persistent-validator-services",
            "build_mode": options.build_mode,
            "local_apply_before_certified_send": options.local_apply_before_certified_send,
            "defer_certified_sends": options.defer_certified_sends,
            "timeout_ms": options.timeout_ms,
            "send_retries": options.send_retries,
            "retry_backoff_ms": options.retry_backoff_ms,
            "base_dir": options.base_dir.display().to_string(),
            "topology_file": options.topology_file.display().to_string(),
            "artifact_root": options.artifact_root.display().to_string(),
            "wallet_address": options.wallet_address,
            "recipient": options.recipient,
            "amount": options.amount
        }),
        latency: serde_json::json!({
            "wallet_to_finality_ms": tx_latency_stats(&wallet_to_finality_values),
            "admitted_to_finality_ms": tx_latency_stats(&admitted_to_finality_values),
            "consensus_round_ms": tx_latency_stats(&consensus_round_values),
            "refresh_account_tx_index_ms": tx_latency_stats(&refresh_index_values)
        }),
        checks: serde_json::json!({
            "iteration_count_matches_rounds": iterations.len() == options.rounds,
            "all_transactions_final": all_transactions_final,
            "all_receipts_accepted": all_receipts_accepted,
            "no_duplicate_receipts": no_duplicate_receipts,
            "final_height_matches_rounds": final_height_matches_rounds,
            "state_verified_after_run": state_verified_after_run,
            "all_rounds_ok": all_rounds_ok,
            "all_vote_policies_match": all_vote_policies_match,
            "account_history_index_not_in_synchronous_finality": account_history_index_not_in_synchronous_finality,
            "converged": converged
        }),
        not_measured: vec![
            "public WAN latency".to_string(),
            "public RPC load".to_string(),
            "evidence packet assembly".to_string(),
            "full history replay inside timed path".to_string(),
            "account-history query cache catch-up latency".to_string(),
        ],
        final_state: serde_json::json!({
            "height": final_heights.first().copied(),
            "state_root": final_roots.first().cloned(),
            "block_tip_hash": final_tips.first().cloned(),
            "state_verification_count": final_state_reports.len()
        }),
        iterations_file: options
            .iterations_file
            .as_ref()
            .map(|path| path.display().to_string()),
        iterations,
    };
    if let Some(parent) = options.report_file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "tx latency benchmark report parent `{}` create failed: {error}",
                parent.display()
            )
        })?;
    }
    let report_json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("tx latency benchmark report serialization failed: {error}"))?;
    std::fs::write(&options.report_file, format!("{report_json}\n")).map_err(|error| {
        format!(
            "tx latency benchmark report `{}` write failed: {error}",
            options.report_file.display()
        )
    })?;
    Ok(report)
}

#[derive(Debug, Clone)]
struct TransportPeerCertifiedBatchLoopOptions {
    data_dir: PathBuf,
    topology_file: PathBuf,
    batch_kind: Option<String>,
    batch_dir: PathBuf,
    key_file: PathBuf,
    proposal_key_file: Option<PathBuf>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    artifact_root: PathBuf,
    processed_dir: Option<PathBuf>,
    max_rounds: usize,
    start_height: u64,
    poll_ms: u64,
    timeout_ms: u64,
    idle_timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerCertifiedBatchLoopReport {
    schema: String,
    node_id: String,
    topology_id: String,
    batch_dir: String,
    artifact_root: String,
    processed_dir: Option<String>,
    max_rounds: usize,
    start_height: u64,
    poll_ms: u64,
    idle_timeout_ms: u64,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    send_retries: usize,
    retry_backoff_ms: u64,
    shielded_verifier_prewarm: TransportShieldedVerifierPrewarmReport,
    processed_round_count: usize,
    shutdown_reason: String,
    processed_batch_files: Vec<String>,
    archived_batch_files: Vec<String>,
    rounds: Vec<TransportPeerCertifiedBatchRoundReport>,
    loop_ok: bool,
}

#[derive(Debug, Clone)]
struct TransportPeerCertifiedPrivateEgressLoopOptions {
    data_dir: PathBuf,
    topology_file: PathBuf,
    egress_dir: PathBuf,
    batch_dir: PathBuf,
    key_file: PathBuf,
    proposal_key_file: Option<PathBuf>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    artifact_root: PathBuf,
    ready_file: Option<PathBuf>,
    processed_egress_dir: Option<PathBuf>,
    processed_batch_dir: Option<PathBuf>,
    max_rounds: usize,
    start_height: u64,
    poll_ms: u64,
    timeout_ms: u64,
    idle_timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerCertifiedPrivateEgressLoopRoundReport {
    block_height: u64,
    egress_file: String,
    batch_file: String,
    artifact_dir: String,
    batch_wrap_ms: f64,
    archived_egress_file: Option<String>,
    archived_batch_file: Option<String>,
    round: TransportPeerCertifiedBatchRoundReport,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportPeerCertifiedPrivateEgressLoopReport {
    schema: String,
    node_id: String,
    topology_id: String,
    egress_dir: String,
    batch_dir: String,
    artifact_root: String,
    ready_file: Option<String>,
    processed_egress_dir: Option<String>,
    processed_batch_dir: Option<String>,
    max_rounds: usize,
    start_height: u64,
    poll_ms: u64,
    idle_timeout_ms: u64,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    send_retries: usize,
    retry_backoff_ms: u64,
    shielded_verifier_prewarm: TransportShieldedVerifierPrewarmReport,
    processed_round_count: usize,
    shutdown_reason: String,
    processed_egress_files: Vec<String>,
    archived_egress_files: Vec<String>,
    processed_batch_files: Vec<String>,
    archived_batch_files: Vec<String>,
    rounds: Vec<TransportPeerCertifiedPrivateEgressLoopRoundReport>,
    loop_ok: bool,
}

#[derive(Debug, Clone)]
struct TransportCertifiedBatchLoopOptions {
    data_dir: PathBuf,
    topology_file: PathBuf,
    batch_kind: Option<String>,
    batch_dir: PathBuf,
    validator_key_dir: PathBuf,
    artifact_root: PathBuf,
    processed_dir: Option<PathBuf>,
    max_rounds: usize,
    start_height: u64,
    poll_ms: u64,
    timeout_ms: u64,
    idle_timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TransportCertifiedBatchLoopReport {
    schema: String,
    node_id: String,
    topology_id: String,
    batch_dir: String,
    artifact_root: String,
    processed_dir: Option<String>,
    max_rounds: usize,
    start_height: u64,
    poll_ms: u64,
    idle_timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    processed_round_count: usize,
    shutdown_reason: String,
    processed_batch_files: Vec<String>,
    archived_batch_files: Vec<String>,
    rounds: Vec<TransportCertifiedBatchRoundReport>,
    loop_ok: bool,
}


mod transport_protocol;
mod transport_runtime;
use transport_protocol::*;
use transport_runtime::*;

#[cfg(test)]
mod certified_send_durability_tests {
    use super::*;

    fn unique_certified_send_test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "postfiat-certified-send-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn certified_send_test_topology() -> NetworkTopology {
        NetworkTopology {
            topology_id: "certified-send-durability-test".to_string(),
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 1,
            peers: vec![
                postfiat_network::PeerInfo {
                    node_id: "validator-0".to_string(),
                    host: "127.0.0.1".to_string(),
                    p2p_port: 26_650,
                    rpc_port: 27_650,
                    p2p_address: "127.0.0.1:26650".to_string(),
                },
                postfiat_network::PeerInfo {
                    node_id: "validator-1".to_string(),
                    host: "127.0.0.1".to_string(),
                    p2p_port: 26_651,
                    rpc_port: 27_651,
                    p2p_address: "127.0.0.1:26651".to_string(),
                },
            ],
        }
    }

    fn certified_send_test_payloads(root: &Path) -> (PathBuf, PathBuf) {
        let input = root.join("input");
        std::fs::create_dir_all(&input).expect("create input directory");
        let batch = input.join("batch.json");
        let certificate = input.join("certificate.json");
        std::fs::write(&batch, b"{\"batch\":true}\n").expect("write batch payload");
        std::fs::write(&certificate, b"{\"certificate\":true}\n")
            .expect("write certificate payload");
        (batch, certificate)
    }

    fn enqueue_certified_send_test_job(
        data_dir: &Path,
        topology: &NetworkTopology,
        batch: &Path,
        certificate: &Path,
        height: u64,
    ) -> PathBuf {
        enqueue_durable_certified_send_job(
            data_dir,
            topology,
            "validator-0",
            "validator-1",
            "transparent",
            height,
            &format!("certificate-{height}"),
            &format!("{:096x}", height),
            &format!("{:096x}", height.saturating_add(100)),
            batch,
            certificate,
            1_000,
            1,
            10,
        )
        .expect("enqueue durable test job")
    }

    fn complete_certified_send_test_job(job_file: &Path) {
        let mut job = read_durable_certified_send_job(job_file).expect("read test job");
        job.completed = true;
        job.ack = Some(DurableCertifiedSendAck {
            already_applied: false,
            block_height: job.block_height,
            block_tip_hash: job.block_hash.clone(),
            state_root: job.expected_state_root.clone(),
        });
        write_durable_certified_send_job(job_file, &job).expect("complete test job");
    }

    #[test]
    fn certified_send_enqueue_publishes_only_a_complete_job_directory() {
        let root = unique_certified_send_test_dir("complete-publish");
        let data_dir = root.join("node");
        let topology = certified_send_test_topology();
        let (batch, certificate) = certified_send_test_payloads(&root);

        let job_file =
            enqueue_certified_send_test_job(&data_dir, &topology, &batch, &certificate, 7);
        let job = read_durable_certified_send_job(&job_file).expect("read published job");
        validate_durable_certified_send_payloads(&job_file, &job)
            .expect("validate published payloads");
        let job_dir = job_file.parent().expect("published job parent");
        assert!(job_dir.join("batch.json").is_file());
        assert!(job_dir.join("certificate.json").is_file());
        assert!(job_dir.join("job.json").is_file());
        assert_eq!(
            cleanup_orphan_certified_send_staging_dirs(&data_dir)
                .expect("clean staging after publication"),
            0
        );
        assert!(std::fs::read_dir(certified_send_outbox_dir(&data_dir))
            .expect("read outbox")
            .filter_map(Result::ok)
            .all(|entry| entry
                .file_name()
                .to_str()
                .is_none_or(|name| !name.starts_with(CERTIFIED_SEND_STAGING_DIR_PREFIX))));

        std::fs::remove_dir_all(root).expect("cleanup complete-publish test");
    }

    #[test]
    fn certified_send_staging_cleanup_removes_only_canonical_owned_entries() {
        let root = unique_certified_send_test_dir("staging-cleanup");
        let data_dir = root.join("node");
        let outbox = certified_send_outbox_dir(&data_dir);
        std::fs::create_dir_all(&outbox).expect("create outbox");
        let job_id = "11".repeat(48);
        let staging_id = "22".repeat(48);
        let staging = outbox.join(format!(
            "{CERTIFIED_SEND_STAGING_DIR_PREFIX}{job_id}-{staging_id}"
        ));
        std::fs::create_dir(&staging).expect("create canonical staging");
        std::fs::write(staging.join("batch.json"), b"partial but owned")
            .expect("write canonical staging payload");

        assert_eq!(
            cleanup_orphan_certified_send_staging_dirs(&data_dir).expect("clean canonical staging"),
            1
        );
        assert!(!staging.exists());

        let unrelated = outbox.join("operator-evidence");
        std::fs::create_dir(&unrelated).expect("create unrelated directory");
        let error = cleanup_orphan_certified_send_staging_dirs(&data_dir)
            .expect_err("unknown outbox entry must fail closed");
        assert!(error.contains("unknown entry"));
        assert!(unrelated.is_dir(), "unrelated directory must be preserved");
        std::fs::remove_dir(&unrelated).expect("remove unrelated test directory");

        let corrupt = outbox.join(format!(
            "{CERTIFIED_SEND_STAGING_DIR_PREFIX}{job_id}-{}",
            "33".repeat(48)
        ));
        std::fs::create_dir(&corrupt).expect("create corrupt staging");
        std::fs::write(corrupt.join("unknown.bin"), b"do not delete")
            .expect("write unknown staging entry");
        let error = cleanup_orphan_certified_send_staging_dirs(&data_dir)
            .expect_err("unknown staging content must fail closed");
        assert!(error.contains("unknown entry"));
        assert!(corrupt.is_dir(), "corrupt staging evidence must remain");

        std::fs::remove_dir_all(root).expect("cleanup staging-cleanup test");
    }

    #[test]
    fn certified_send_completed_retention_is_bounded_and_fails_closed() {
        let root = unique_certified_send_test_dir("completed-retention");
        let data_dir = root.join("node");
        let topology = certified_send_test_topology();
        let (batch, certificate) = certified_send_test_payloads(&root);
        for height in 1..=3 {
            let job_file =
                enqueue_certified_send_test_job(&data_dir, &topology, &batch, &certificate, height);
            complete_certified_send_test_job(&job_file);
        }
        assert_eq!(
            compact_completed_durable_certified_send_jobs(&data_dir)
                .expect("compact completed jobs"),
            3
        );
        let completed_dir = certified_send_completed_dir(&data_dir);
        let unknown = completed_dir.join("operator-evidence");
        std::fs::create_dir(&unknown).expect("create unknown completed entry");
        let error = prune_completed_durable_certified_send_jobs(&data_dir, 2)
            .expect_err("unknown completed entry must fail closed");
        assert!(error.contains("non-canonical name"));
        assert_eq!(
            std::fs::read_dir(&completed_dir)
                .expect("read completed after fail closed")
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name() != "operator-evidence")
                .count(),
            3,
            "validation failure must not prune any tombstone"
        );
        std::fs::remove_dir(&unknown).expect("remove unknown test entry");

        assert_eq!(
            prune_completed_durable_certified_send_jobs(&data_dir, 2)
                .expect("prune validated completed jobs"),
            1
        );
        let completed = validated_completed_durable_certified_send_jobs(&data_dir)
            .expect("validate retained completed jobs");
        assert_eq!(
            completed
                .iter()
                .map(|(_, job)| job.block_height)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        let interrupted_source = completed[0].0.clone();
        let interrupted_job_id = completed[0].1.job_id.clone();
        let retention_dir = certified_send_completed_retention_dir(&data_dir);
        std::fs::create_dir_all(&retention_dir).expect("create interrupted retention directory");
        let interrupted_destination = retention_dir.join(&interrupted_job_id);
        std::fs::rename(&interrupted_source, &interrupted_destination)
            .expect("simulate crash after retention rename");
        assert_eq!(
            cleanup_certified_send_completed_retention_dir(&data_dir)
                .expect("finish interrupted retention cleanup"),
            1
        );
        assert!(!interrupted_destination.exists());

        std::fs::remove_dir_all(root).expect("cleanup completed-retention test");
    }
}
