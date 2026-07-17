#[derive(Debug, Clone, serde::Serialize)]
struct RpcServeEventRecord {
    schema: String,
    node_id: String,
    request_index: u64,
    peer_addr: String,
    id: String,
    method: String,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    handler_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    handler_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mempool_submit_peer_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mempool_submit_total_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    orchard_batch_create_peer_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    orchard_batch_create_total_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    orchard_batch_create_active_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_class: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RpcServeReport {
    schema: String,
    node_id: String,
    bind_address: String,
    event_log: Option<String>,
    max_requests: u64,
    child_timeout_ms: u64,
    child_isolation: RpcChildIsolationReport,
    request_count: u64,
    ok_count: u64,
    error_count: u64,
    mempool_submit_signed_transfer_count: u64,
    mempool_submit_signed_transfer_finality_count: u64,
    orchard_batch_create_count: u64,
    max_mempool_submit_per_peer: u64,
    max_mempool_submit_total: u64,
    max_orchard_batch_create_per_peer: u64,
    max_orchard_batch_create_total: u64,
    max_orchard_batch_create_concurrent: u64,
    invalid_signature_count: u64,
    duplicate_transaction_count: u64,
    request_too_large_count: u64,
    mempool_submit_rate_limited_count: u64,
    mempool_submit_global_rate_limited_count: u64,
    orchard_batch_create_rate_limited_count: u64,
    orchard_batch_create_global_rate_limited_count: u64,
    orchard_batch_create_concurrency_limited_count: u64,
    orchard_batch_create_not_public_safe_count: u64,
    rpc_child_timeout_count: u64,
    method_not_allowed_count: u64,
    read_only: bool,
    mempool_submit_finality_enabled: bool,
    orchard_batch_create_enabled: bool,
    owned_lane_enabled: bool,
    requests: Vec<RpcServeEventRecord>,
    verified: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RpcChildIsolationReport {
    process_per_request: bool,
    request_file_spooled: bool,
    stdin_null: bool,
    stdout_stderr_piped: bool,
    environment_cleared: bool,
    current_dir_canonical_data_dir: bool,
    timeout_enforced: bool,
}

#[derive(Debug, Clone)]
struct RpcServeOptions {
    data_dir: PathBuf,
    spool_dir: PathBuf,
    ready_file: PathBuf,
    bind_host: String,
    port: u16,
    max_requests: usize,
    timeout_ms: u64,
    child_timeout_ms: u64,
    event_log: Option<PathBuf>,
    allow_mempool_submit: bool,
    allow_mempool_submit_finality: bool,
    allow_orchard_batch_create: bool,
    owned_lane_enabled: bool,
    finality_topology_file: PathBuf,
    finality_key_file: PathBuf,
    finality_proposal_key_file: Option<PathBuf>,
    finality_artifact_root: PathBuf,
    finality_timeout_ms: u64,
    finality_send_retries: usize,
    finality_retry_backoff_ms: u64,
    finality_quorum_early_full_propagation: bool,
    max_mempool_submit_per_peer: u64,
    max_mempool_submit_total: u64,
    max_orchard_batch_create_per_peer: u64,
    max_orchard_batch_create_total: u64,
    max_orchard_batch_create_concurrent: u64,
    keep_alive: bool,
}

struct RpcServeConnectionContext {
    data_dir: PathBuf,
    spool_dir: PathBuf,
    node_id: String,
    peer_addr: String,
    allow_mempool_submit: bool,
    allow_mempool_submit_finality: bool,
    allow_orchard_batch_create: bool,
    owned_lane_enabled: bool,
    owned_certificate_domain: postfiat_types::OwnedCertificateDomain,
    child_timeout_ms: u64,
    finality_topology_file: PathBuf,
    finality_key_file: PathBuf,
    finality_proposal_key_file: Option<PathBuf>,
    finality_artifact_root: PathBuf,
    finality_timeout_ms: u64,
    finality_send_retries: usize,
    finality_retry_backoff_ms: u64,
    finality_quorum_early_full_propagation: bool,
    max_mempool_submit_per_peer: u64,
    max_mempool_submit_total: u64,
    max_orchard_batch_create_per_peer: u64,
    max_orchard_batch_create_total: u64,
    max_orchard_batch_create_concurrent: u64,
    mempool_submit_state: Arc<Mutex<RpcServeMempoolSubmitState>>,
    orchard_batch_create_state: Arc<Mutex<RpcServeMempoolSubmitState>>,
    mempool_mutation_lock: Arc<Mutex<()>>,
    finality_submit_lock: Arc<Mutex<()>>,
    health_cache: Arc<Mutex<RpcServeHealthCache>>,
    fastswap_service: Arc<Mutex<Option<crate::fastswap_service::FastSwapValidatorServiceV1>>>,
    runtime_metrics: Arc<RpcServeRuntimeMetrics>,
}

#[derive(Debug, Default)]
struct RpcServeRuntimeMetrics {
    active_connections: AtomicU64,
    peak_active_connections: AtomicU64,
    accepted_connection_count: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RpcServeRuntimeMetricsSnapshot {
    active_connections: u64,
    active_connection_limit: u64,
    peak_active_connections: u64,
    accepted_connection_count: u64,
}

impl RpcServeRuntimeMetrics {
    fn connection_opened(&self) {
        let active = self
            .active_connections
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        self.accepted_connection_count
            .fetch_add(1, Ordering::Relaxed);
        self.peak_active_connections
            .fetch_max(active, Ordering::AcqRel);
    }

    fn snapshot(&self) -> RpcServeRuntimeMetricsSnapshot {
        RpcServeRuntimeMetricsSnapshot {
            active_connections: self.active_connections.load(Ordering::Acquire),
            active_connection_limit: MAX_RPC_SERVE_ACTIVE_CONNECTIONS as u64,
            peak_active_connections: self.peak_active_connections.load(Ordering::Acquire),
            accepted_connection_count: self.accepted_connection_count.load(Ordering::Relaxed),
        }
    }
}

struct RpcServeActiveConnectionGuard(Arc<RpcServeRuntimeMetrics>);

impl Drop for RpcServeActiveConnectionGuard {
    fn drop(&mut self) {
        self.0.active_connections.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct RpcMempoolSubmitSignedTransferFinalityReport {
    schema: String,
    tx_id: String,
    finality: TxFinalityReport,
    round_report_file: String,
    artifact_dir: String,
    readiness_wait_ms: f64,
    mempool_submit_ms: f64,
    mempool_batch_ms: f64,
    certified_round_ms: f64,
    total_ms: f64,
    certified_sends_deferred: bool,
    round_ok: bool,
}

#[derive(Debug, Default)]
struct RpcServeMempoolSubmitState {
    counts_by_peer: BTreeMap<String, std::collections::VecDeque<Instant>>,
    total_timestamps: std::collections::VecDeque<Instant>,
    active_count: u64,
}

#[derive(Debug, Default)]
struct RpcServeHealthCache {
    status: Option<(RpcServeHealthStamp, StatusReport)>,
    mempool: Option<(RpcServeHealthStamp, postfiat_types::MempoolState)>,
    status_checked_at: Option<Instant>,
    mempool_checked_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RpcServeHealthStamp(Vec<(u64, u128)>);

#[derive(Debug, Clone, Copy)]
struct RpcServeMempoolSubmitCounters {
    peer_count: u64,
    total_count: u64,
    active_count: u64,
}

#[derive(Debug)]
struct RpcServeActiveCounterGuard {
    state: Arc<Mutex<RpcServeMempoolSubmitState>>,
}

static RPC_SERVE_SPOOL_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
const RPC_SERVE_SPOOL_DIR_ATTEMPTS: u32 = 128;
const RPC_ORCHARD_ACTION_SPOOL_MAX_REQUEST_BYTES: u64 = MAX_RPC_REQUEST_BYTES as u64;
const RPC_ORCHARD_ACTION_SPOOL_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const RPC_SERVE_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
const RPC_SERVE_HEALTH_STAMP_MAX_AGE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, serde::Serialize)]
struct RpcServeReadinessReport {
    schema: String,
    ready: bool,
    degraded: bool,
    node_id: String,
    bind_address: String,
    data_dir: String,
    data_dir_readable: bool,
    spool_dir: String,
    spool_probe_ok: bool,
    event_log: Option<String>,
    event_log_writable: bool,
    local_state_loaded: bool,
    listener_bound: bool,
    finality_enabled: bool,
    finality_topology_available: bool,
    finality_key_available: bool,
    telemetry_failure_count: u64,
    last_telemetry_error: Option<String>,
}

impl Drop for RpcServeActiveCounterGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.active_count = state.active_count.saturating_sub(1);
        }
    }
}

fn rpc_finality_required_u64_param(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<Option<u64>, (String, String)> {
    let Some(value) = params.get(key) else {
        return Ok(None);
    };
    if let Some(number) = value.as_u64() {
        return Ok(Some(number));
    }
    Err((
        "rpc_protocol_error".to_string(),
        format!("{key} must be an unsigned integer"),
    ))
}

fn wait_for_rpc_finality_required_parent(
    context: &RpcServeConnectionContext,
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<f64, (String, String)> {
    let Some(required_height) = rpc_finality_required_u64_param(
        params,
        "proxy_required_current_height",
    )? else {
        return Ok(0.0);
    };
    let required_state_root = params
        .get("proxy_required_state_root")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let required_parent_hash = params
        .get("proxy_required_parent_hash")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let timeout_ms = rpc_finality_required_u64_param(params, "proxy_readiness_timeout_ms")?
        .unwrap_or(context.finality_timeout_ms)
        .min(context.finality_timeout_ms.max(1));
    let started = Instant::now();
    let mut last_status_error = None;
    loop {
        let local_status = match status(NodeOptions {
            data_dir: context.data_dir.clone(),
        }) {
            Ok(status) => status,
            Err(error) => {
                let message = format!("finality parent readiness status failed: {error}");
                if monotonic_elapsed_ms(started) >= timeout_ms as f64 {
                    return Err(("rpc_finality_parent_wait_failed".to_string(), message));
                }
                last_status_error = Some(message);
                std::thread::sleep(std::time::Duration::from_millis(20));
                continue;
            }
        };
        if let Some(error) = rpc_finality_parent_stale_error(
            &context.node_id,
            required_height,
            required_parent_hash.as_deref(),
            required_state_root.as_deref(),
            local_status.block_height,
            &local_status.block_tip_hash,
            &local_status.state_root,
        ) {
            return Err(error);
        }
        let height_matches = local_status.block_height == required_height;
        let root_matches = required_state_root
            .as_ref()
            .map(|root| local_status.state_root == *root)
            .unwrap_or(true);
        let parent_hash_matches = required_parent_hash
            .as_ref()
            .map(|hash| local_status.block_tip_hash == *hash)
            .unwrap_or(true);
        if height_matches && root_matches && parent_hash_matches {
            return Ok(monotonic_elapsed_ms(started));
        }
        if monotonic_elapsed_ms(started) >= timeout_ms as f64 {
            let root_requirement = required_state_root
                .as_deref()
                .unwrap_or("<any>");
            let hash_requirement = required_parent_hash
                .as_deref()
                .unwrap_or("<any>");
            let status_error_suffix = last_status_error
                .as_deref()
                .map(|error| format!("; last status error: {error}"))
                .unwrap_or_default();
            return Err((
                "rpc_finality_parent_not_ready".to_string(),
                format!(
                    "finality proposer `{}` did not reach required parent height {} hash {} root {} before {}ms; observed height {} hash {} root {}{}",
                    context.node_id,
                    required_height,
                    hash_requirement,
                    root_requirement,
                    timeout_ms,
                    local_status.block_height,
                    local_status.block_tip_hash,
                    local_status.state_root,
                    status_error_suffix,
                ),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[allow(clippy::too_many_arguments)]
fn rpc_finality_parent_stale_error(
    node_id: &str,
    required_height: u64,
    required_parent_hash: Option<&str>,
    required_state_root: Option<&str>,
    observed_height: u64,
    observed_parent_hash: &str,
    observed_state_root: &str,
) -> Option<(String, String)> {
    if observed_height <= required_height {
        return None;
    }
    Some((
        "rpc_finality_parent_stale".to_string(),
        format!(
            "finality proposer `{node_id}` already advanced past required parent height {required_height} hash {} root {}; observed height {observed_height} hash {observed_parent_hash} root {observed_state_root}",
            required_parent_hash.unwrap_or("<any>"),
            required_state_root.unwrap_or("<any>"),
        ),
    ))
}

fn rpc_finality_proposer_mismatch(
    local_node_id: &str,
    expected_proposer: &str,
    block_height: u64,
    view: u64,
) -> Option<(String, String)> {
    if local_node_id == expected_proposer {
        return None;
    }
    Some((
        "rpc_finality_wrong_proposer".to_string(),
        format!(
            "local validator `{local_node_id}` is not deterministic proposer \
             `{expected_proposer}` for height {block_height} view {view}; \
             retry the signed request at `{expected_proposer}`"
        ),
    ))
}

fn require_rpc_finality_local_proposer(
    context: &RpcServeConnectionContext,
    view: u64,
) -> Result<(), (String, String)> {
    let local_status = status(NodeOptions {
        data_dir: context.data_dir.clone(),
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality proposer status failed: {error}"),
        )
    })?;
    let block_height = local_status.block_height.checked_add(1).ok_or_else(|| {
        (
            "rpc_finality_submit_failed".to_string(),
            "finality proposer height overflow".to_string(),
        )
    })?;
    let proposer = block_proposer(BlockProposerOptions {
        data_dir: context.data_dir.clone(),
        block_height,
        view,
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality proposer lookup failed: {error}"),
        )
    })?;
    match rpc_finality_proposer_mismatch(
        &local_status.node_id,
        &proposer.proposer,
        block_height,
        view,
    ) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn rpc_serve_method_accepts_proxy_parent_wait(method: &str) -> bool {
    matches!(
        method,
        "transfer_fee_quote"
            | "atomic_swap_fee_quote"
            | "asset_fee_quote"
            | "escrow_fee_quote"
            | "nft_fee_quote"
            | "offer_fee_quote"
    )
}

const RPC_OWNED_SIGN_DECOMPRESSED_MAX_BYTES: u64 = 128 * 1024;
const RPC_OWNED_SIGN_ENCODED_MAX_BYTES: usize = 256 * 1024;

fn rpc_owned_sign_order_json(request: &RpcRequest) -> Result<String, String> {
    if let Some(order_json) = request.params.get("order_json").and_then(|value| value.as_str()) {
        if !order_json.is_empty() {
            return Ok(order_json.to_string());
        }
    }
    let encoded = request
        .params
        .get("order_json_gzip_base64")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "requires order_json or order_json_gzip_base64".to_string())?;
    if encoded.len() > RPC_OWNED_SIGN_ENCODED_MAX_BYTES {
        return Err("compressed order exceeds encoded byte cap".to_string());
    }
    let compressed = BASE64_STANDARD
        .decode(encoded)
        .map_err(|error| format!("compressed order base64 decode failed: {error}"))?;
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut limited = (&mut decoder).take(RPC_OWNED_SIGN_DECOMPRESSED_MAX_BYTES + 1);
    let mut order_json = String::new();
    limited
        .read_to_string(&mut order_json)
        .map_err(|error| format!("compressed order gzip decode failed: {error}"))?;
    if order_json.len() as u64 > RPC_OWNED_SIGN_DECOMPRESSED_MAX_BYTES {
        return Err("compressed order exceeds decompressed byte cap".to_string());
    }
    if order_json.is_empty() {
        return Err("compressed order decoded to an empty value".to_string());
    }
    Ok(order_json)
}

fn rpc_owned_json_response(
    id: &str,
    value_json: String,
    serialization_error: &str,
) -> RpcResponse {
    match serde_json::from_str::<serde_json::Value>(&value_json) {
        Ok(value) => success_response(id, &value, vec![]).unwrap_or_else(|_| {
            rpc_serve_error_response(id, "rpc_internal", serialization_error)
        }),
        Err(_) => rpc_serve_error_response(id, "rpc_internal", serialization_error),
    }
}

fn rpc_serve(options: RpcServeOptions) -> Result<RpcServeReport, String> {
    clear_transport_ready_file(&options.ready_file, "rpc serve")?;
    let mut local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("rpc serve status failed: {error}"))?;
    let reconciled_terminal_entries = reconcile_terminal_mempool_entries(&options.data_dir)
        .map_err(|error| format!("rpc serve terminal mempool reconciliation failed: {error}"))?;
    if reconciled_terminal_entries > 0 {
        local_status = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })
        .map_err(|error| format!("rpc serve post-reconciliation status failed: {error}"))?;
    }
    let owned_certificate_domain = owned_certificate_domain(&options.data_dir)
        .map_err(|error| format!("rpc serve FastPay domain failed: {error}"))?;
    validate_rpc_serve_bind_host(&options.bind_host)?;
    prepare_rpc_serve_spool_root(&options.spool_dir)?;
    probe_rpc_serve_spool_root(&options.spool_dir)?;
    let bind_address = socket_address(&options.bind_host, options.port);
    let listener = TcpListener::bind(&bind_address)
        .map_err(|error| format!("rpc serve bind `{bind_address}` failed: {error}"))?;
    let mut event_writer = options
        .event_log
        .as_ref()
        .map(|path| open_transport_event_log(path))
        .transpose()?;
    let finality_topology_available = !options.allow_mempool_submit_finality
        || options.finality_topology_file.is_file();
    let finality_key_available = !options.allow_mempool_submit_finality
        || options.finality_key_file.is_file();
    if !finality_topology_available || !finality_key_available {
        return Err("rpc serve finality configuration files are unavailable".to_string());
    }
    let initial_mempool = mempool_state(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("rpc serve initial mempool load failed: {error}"))?;
    let readiness = Arc::new(Mutex::new(RpcServeReadinessReport {
        schema: "postfiat-rpc-serve-readiness-v1".to_string(),
        ready: true,
        degraded: false,
        node_id: local_status.node_id.clone(),
        bind_address: bind_address.clone(),
        data_dir: options.data_dir.display().to_string(),
        data_dir_readable: true,
        spool_dir: options.spool_dir.display().to_string(),
        spool_probe_ok: true,
        event_log: options.event_log.as_ref().map(|path| path.display().to_string()),
        event_log_writable: true,
        local_state_loaded: true,
        listener_bound: true,
        finality_enabled: options.allow_mempool_submit_finality,
        finality_topology_available,
        finality_key_available,
        telemetry_failure_count: 0,
        last_telemetry_error: None,
    }));
    {
        let readiness = readiness
            .lock()
            .map_err(|_| "rpc serve readiness lock poisoned".to_string())?;
        write_rpc_serve_readiness(&options.ready_file, &readiness)?;
    }

    let mut requests = Vec::with_capacity(options.max_requests);
    let mempool_submit_state = Arc::new(Mutex::new(RpcServeMempoolSubmitState::default()));
    let orchard_batch_create_state = Arc::new(Mutex::new(RpcServeMempoolSubmitState::default()));
    let mempool_mutation_lock = Arc::new(Mutex::new(()));
    let finality_submit_lock = Arc::new(Mutex::new(()));
    let health_cache = Arc::new(Mutex::new(RpcServeHealthCache {
        status: Some((
            rpc_serve_health_stamp(&options.data_dir, true)?,
            local_status.clone(),
        )),
        mempool: Some((
            rpc_serve_health_stamp(&options.data_dir, false)?,
            initial_mempool,
        )),
        status_checked_at: Some(Instant::now()),
        mempool_checked_at: Some(Instant::now()),
    }));
    let fastswap_service = Arc::new(Mutex::new(None));
    let runtime_metrics = Arc::new(RpcServeRuntimeMetrics::default());
    let (event_sender, event_receiver) = mpsc::channel::<RpcServeEventRecord>();
    let mut accepted_count = 0_usize;
    let mut active_connections = 0_usize;
    while accepted_count < options.max_requests {
        while active_connections >= MAX_RPC_SERVE_ACTIVE_CONNECTIONS {
            receive_rpc_serve_event(
                &event_receiver,
                &mut active_connections,
                &mut requests,
                &mut event_writer,
                &options.ready_file,
                &readiness,
            )?;
        }
        drain_rpc_serve_events(
            &event_receiver,
            &mut active_connections,
            &mut requests,
            &mut event_writer,
            &options.ready_file,
            &readiness,
        )?;

        let (mut stream, peer_addr) = listener
            .accept()
            .map_err(|error| format!("rpc serve accept failed: {error}"))?;
        set_stream_timeout(&stream, options.timeout_ms)?;
        accepted_count = accepted_count.saturating_add(1);
        active_connections = active_connections.saturating_add(1);
        runtime_metrics.connection_opened();
        let request_index = accepted_count as u64;
        let context = RpcServeConnectionContext {
            data_dir: options.data_dir.clone(),
            spool_dir: options.spool_dir.clone(),
            node_id: local_status.node_id.clone(),
            peer_addr: peer_addr.ip().to_string(),
            allow_mempool_submit: options.allow_mempool_submit,
            allow_mempool_submit_finality: options.allow_mempool_submit_finality,
            allow_orchard_batch_create: options.allow_orchard_batch_create,
            owned_lane_enabled: options.owned_lane_enabled,
            owned_certificate_domain: owned_certificate_domain.clone(),
            child_timeout_ms: options.child_timeout_ms,
            finality_topology_file: options.finality_topology_file.clone(),
            finality_key_file: options.finality_key_file.clone(),
            finality_proposal_key_file: options.finality_proposal_key_file.clone(),
            finality_artifact_root: options.finality_artifact_root.clone(),
            finality_timeout_ms: options.finality_timeout_ms,
            finality_send_retries: options.finality_send_retries,
            finality_retry_backoff_ms: options.finality_retry_backoff_ms,
            finality_quorum_early_full_propagation: options
                .finality_quorum_early_full_propagation,
            max_mempool_submit_per_peer: options.max_mempool_submit_per_peer,
            max_mempool_submit_total: options.max_mempool_submit_total,
            max_orchard_batch_create_per_peer: options.max_orchard_batch_create_per_peer,
            max_orchard_batch_create_total: options.max_orchard_batch_create_total,
            max_orchard_batch_create_concurrent: options.max_orchard_batch_create_concurrent,
            mempool_submit_state: Arc::clone(&mempool_submit_state),
            orchard_batch_create_state: Arc::clone(&orchard_batch_create_state),
            mempool_mutation_lock: Arc::clone(&mempool_mutation_lock),
            finality_submit_lock: Arc::clone(&finality_submit_lock),
            health_cache: Arc::clone(&health_cache),
            fastswap_service: Arc::clone(&fastswap_service),
            runtime_metrics: Arc::clone(&runtime_metrics),
        };
        let event_sender = event_sender.clone();
        let keep_alive = options.keep_alive;
        thread::spawn(move || {
            let _active_connection_guard =
                RpcServeActiveConnectionGuard(Arc::clone(&context.runtime_metrics));
            let fallback_node_id = context.node_id.clone();
            let fallback_peer_addr = context.peer_addr.clone();
            let mut context = context;
            let mut idx = request_index;
            // Keep one reader for the connection lifetime so pipelined bytes
            // read ahead after a newline are not discarded between requests.
            let mut reader = BufReader::new(&mut stream);
            let last_event = loop {
                let event = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_rpc_serve_connection(&mut reader, idx, &mut context)
                }))
                .unwrap_or_else(|_| {
                    let id = format!("remote-{idx}");
                    let response =
                        rpc_serve_error_response(&id, "rpc_worker_panic", "rpc serve worker panicked");
                    rpc_serve_event(
                        &fallback_node_id,
                        idx,
                        &fallback_peer_addr,
                        id,
                        "invalid",
                        None,
                        None,
                        &response,
                    )
                });
                idx = idx.saturating_add(1);
                if !keep_alive {
                    break event;
                }
                // Observe both read-ahead and new socket bytes without
                // consuming the next frame before the handler sees it.
                match reader.fill_buf() {
                    Ok([]) => break event,
                    Ok(_) => continue,
                    Err(_) => break event,
                }
            };
            let _ = event_sender.send(last_event);
        });
    }
    drop(event_sender);
    while active_connections > 0 {
        receive_rpc_serve_event(
            &event_receiver,
            &mut active_connections,
            &mut requests,
            &mut event_writer,
            &options.ready_file,
            &readiness,
        )?;
    }
    requests.sort_by_key(|request| request.request_index);

    let ok_count = requests.iter().filter(|request| request.ok).count() as u64;
    let request_count = requests.len() as u64;
    let mempool_submit_signed_transfer_count = requests
        .iter()
        .filter(|request| is_mempool_submit_signed_method(&request.method))
        .count() as u64;
    let mempool_submit_signed_transfer_finality_count = requests
        .iter()
        .filter(|request| is_mempool_submit_signed_transfer_finality_method(&request.method))
        .count() as u64;
    let orchard_batch_create_count = requests
        .iter()
        .filter(|request| is_orchard_batch_create_method(&request.method))
        .count() as u64;
    let invalid_signature_count = rpc_serve_error_class_count(&requests, "invalid_signature");
    let duplicate_transaction_count =
        rpc_serve_error_class_count(&requests, "duplicate_transaction");
    let request_too_large_count = rpc_serve_error_class_count(&requests, "request_too_large");
    let mempool_submit_rate_limited_count =
        rpc_serve_error_class_count(&requests, "mempool_submit_rate_limited");
    let mempool_submit_global_rate_limited_count =
        rpc_serve_error_class_count(&requests, "mempool_submit_global_rate_limited");
    let orchard_batch_create_rate_limited_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_rate_limited");
    let orchard_batch_create_global_rate_limited_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_global_rate_limited");
    let orchard_batch_create_concurrency_limited_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_concurrency_limited");
    let orchard_batch_create_not_public_safe_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_not_public_safe");
    let rpc_child_timeout_count = rpc_serve_error_class_count(&requests, "rpc_child_timeout");
    let method_not_allowed_count = rpc_serve_error_class_count(&requests, "method_not_allowed");
    Ok(RpcServeReport {
        schema: "postfiat-rpc-serve-v1".to_string(),
        node_id: local_status.node_id,
        bind_address,
        event_log: options.event_log.map(|path| path.display().to_string()),
        max_requests: options.max_requests as u64,
        child_timeout_ms: options.child_timeout_ms,
        child_isolation: rpc_child_isolation_report(),
        request_count,
        ok_count,
        error_count: request_count.saturating_sub(ok_count),
        mempool_submit_signed_transfer_count,
        mempool_submit_signed_transfer_finality_count,
        orchard_batch_create_count,
        max_mempool_submit_per_peer: options.max_mempool_submit_per_peer,
        max_mempool_submit_total: options.max_mempool_submit_total,
        max_orchard_batch_create_per_peer: options.max_orchard_batch_create_per_peer,
        max_orchard_batch_create_total: options.max_orchard_batch_create_total,
        max_orchard_batch_create_concurrent: options.max_orchard_batch_create_concurrent,
        invalid_signature_count,
        duplicate_transaction_count,
        request_too_large_count,
        mempool_submit_rate_limited_count,
        mempool_submit_global_rate_limited_count,
        orchard_batch_create_rate_limited_count,
        orchard_batch_create_global_rate_limited_count,
        orchard_batch_create_concurrency_limited_count,
        orchard_batch_create_not_public_safe_count,
        rpc_child_timeout_count,
        method_not_allowed_count,
        read_only: !options.allow_mempool_submit
            && !options.allow_mempool_submit_finality
            && !options.allow_orchard_batch_create
            && !options.owned_lane_enabled,
        mempool_submit_finality_enabled: options.allow_mempool_submit_finality,
        orchard_batch_create_enabled: options.allow_orchard_batch_create,
        owned_lane_enabled: options.owned_lane_enabled,
        requests,
        verified: true,
    })
}

fn receive_rpc_serve_event(
    event_receiver: &mpsc::Receiver<RpcServeEventRecord>,
    active_connections: &mut usize,
    requests: &mut Vec<RpcServeEventRecord>,
    event_writer: &mut Option<std::fs::File>,
    ready_file: &Path,
    readiness: &Arc<Mutex<RpcServeReadinessReport>>,
) -> Result<(), String> {
    let event = event_receiver
        .recv()
        .map_err(|error| format!("rpc serve worker channel receive failed: {error}"))?;
    *active_connections = active_connections.saturating_sub(1);
    write_rpc_serve_event_or_degrade(event_writer, &event, ready_file, readiness);
    requests.push(event);
    Ok(())
}

fn drain_rpc_serve_events(
    event_receiver: &mpsc::Receiver<RpcServeEventRecord>,
    active_connections: &mut usize,
    requests: &mut Vec<RpcServeEventRecord>,
    event_writer: &mut Option<std::fs::File>,
    ready_file: &Path,
    readiness: &Arc<Mutex<RpcServeReadinessReport>>,
) -> Result<(), String> {
    while let Ok(event) = event_receiver.try_recv() {
        *active_connections = active_connections.saturating_sub(1);
        write_rpc_serve_event_or_degrade(event_writer, &event, ready_file, readiness);
        requests.push(event);
    }
    Ok(())
}

fn write_rpc_serve_event_or_degrade(
    event_writer: &mut Option<std::fs::File>,
    event: &RpcServeEventRecord,
    ready_file: &Path,
    readiness: &Arc<Mutex<RpcServeReadinessReport>>,
) {
    let Some(writer) = event_writer.as_mut() else {
        return;
    };
    if let Err(error) = write_event_log_line(writer, event) {
        let mut readiness = match readiness.lock() {
            Ok(readiness) => readiness,
            Err(_) => {
                eprintln!("ERROR rpc serve telemetry failed: {error}; readiness lock poisoned");
                *event_writer = None;
                return;
            }
        };
        readiness.degraded = true;
        readiness.event_log_writable = false;
        readiness.telemetry_failure_count = readiness.telemetry_failure_count.saturating_add(1);
        readiness.last_telemetry_error = Some(error.clone());
        *event_writer = None;
        if let Err(ready_error) = write_rpc_serve_readiness(ready_file, &readiness) {
            eprintln!("ERROR rpc serve telemetry failed: {error}; readiness update failed: {ready_error}");
        } else {
            eprintln!("ERROR rpc serve telemetry disabled after write failure: {error}");
        }
    }
}

fn rpc_serve_fastswap_service<'a>(
    context: &'a RpcServeConnectionContext,
) -> std::io::Result<
    std::sync::MutexGuard<'a, Option<crate::fastswap_service::FastSwapValidatorServiceV1>>,
> {
    let mut service = context
        .fastswap_service
        .lock()
        .map_err(|_| std::io::Error::other("FastSwap service lock poisoned"))?;
    if service.is_none() {
        *service = Some(crate::fastswap_service::FastSwapValidatorServiceV1::open(
            &context.data_dir,
            &context.node_id,
        )?);
    }
    service
        .as_mut()
        .ok_or_else(|| std::io::Error::other("FastSwap service initialization failed"))?
        .refresh_canonical(&context.data_dir)?;
    Ok(service)
}

fn parse_fastswap_wire_payload<T: serde::de::DeserializeOwned>(
    value: &str,
    label: &str,
) -> std::io::Result<T> {
    let bytes = if value.starts_with(postfiat_rpc_sdk::FASTSWAP_WIRE_GZIP_BASE64_V2_PREFIX) {
        postfiat_rpc_sdk::decode_fastswap_wire_payload_v2(value).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{label} wire decode failed: {error}"),
            )
        })?
    } else {
        value.as_bytes().to_vec()
    };
    serde_json::from_slice(&bytes).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{label} parse failed: {error}"),
        )
    })
}

fn request_uses_fastswap_wire_v2(request: &postfiat_rpc_sdk::RpcRequest) -> bool {
    request.params.as_object().is_some_and(|params| {
        params.values().any(|value| {
            value.as_str().is_some_and(|value| {
                value.starts_with(postfiat_rpc_sdk::FASTSWAP_WIRE_GZIP_BASE64_V2_PREFIX)
            })
        })
    })
}

fn fastswap_success_response(
    request: &postfiat_rpc_sdk::RpcRequest,
    value: &serde_json::Value,
) -> postfiat_rpc_sdk::RpcResponse {
    let result = if request_uses_fastswap_wire_v2(request) {
        serde_json::to_string(value)
            .map_err(|error| format!("FastSwap response serialization failed: {error}"))
            .and_then(|json| postfiat_rpc_sdk::encode_fastswap_wire_payload_v2(&json))
            .map(serde_json::Value::String)
    } else {
        Ok(value.clone())
    };
    match result {
        Ok(result) => postfiat_rpc_sdk::success_response(&request.id, &result, vec![])
            .unwrap_or_else(|_| {
                rpc_serve_error_response(
                    &request.id,
                    "rpc_internal",
                    "FastSwap response serialization failed",
                )
            }),
        Err(error) => rpc_serve_error_response(&request.id, "rpc_internal", &error),
    }
}

fn handle_rpc_serve_connection(
    reader: &mut BufReader<&mut TcpStream>,
    request_index: u64,
    context: &mut RpcServeConnectionContext,
) -> RpcServeEventRecord {
    let handler_started = Instant::now();
    let line = match read_rpc_line(reader, "rpc serve read") {
        Ok(line) => line,
        Err(error) => {
            let id = format!("remote-{request_index}");
            let error_code = if error.contains("exceeded") {
                "rpc_request_too_large"
            } else {
                "rpc_read_error"
            };
            let response = rpc_serve_error_response(&id, error_code, &error);
            let _ = write_json_line(reader.get_mut(), &response);
            return rpc_serve_event(
                &context.node_id,
                request_index,
                &context.peer_addr,
                id,
                "invalid",
                None,
                None,
                &response,
            );
        }
    };
    if let Err(error) = validate_rpc_serve_request_line(&line) {
        let id = format!("remote-{request_index}");
        let response = rpc_serve_error_response(&id, "rpc_request_too_large", &error);
        let _ = write_json_line(reader.get_mut(), &response);
        return rpc_serve_event(
            &context.node_id,
            request_index,
            &context.peer_addr,
            id,
            "invalid",
            None,
            None,
            &response,
        );
    }

    let request = match serde_json::from_str::<RpcRequest>(&line) {
        Ok(request) => request,
        Err(error) => {
            let id = format!("remote-{request_index}");
            let response = rpc_serve_error_response(
                &id,
                "rpc_parse_error",
                &format!("rpc request parse failed: {error}"),
            );
            let _ = write_json_line(reader.get_mut(), &response);
            return rpc_serve_event(
                &context.node_id,
                request_index,
                &context.peer_addr,
                id,
                "invalid",
                None,
                None,
                &response,
            );
        }
    };

    let id = request.id.clone();
    let method = request.method.clone();
    let mut mempool_submit_counts = None;
    let mut orchard_batch_create_counts = None;
    let mut health_cache_hit = false;
    let mut response = if let Err(error) = request.validate_protocol() {
        rpc_serve_error_response(&id, "rpc_protocol_error", &error.to_string())
    } else if rpc_value_contains_key_material(&request.params) {
        rpc_serve_error_response(
            &id,
            "rpc_protocol_error",
            "remote rpc request contains key material fields",
        )
    } else if !rpc_serve_method_allowed_with_owned_lane(
        &method,
        context.allow_mempool_submit,
        context.allow_mempool_submit_finality,
        context.allow_orchard_batch_create,
        context.owned_lane_enabled,
    ) {
        rpc_serve_error_response(
            &id,
            "rpc_method_not_allowed",
            &format!("rpc method `{method}` is not enabled on the read-only remote server"),
        )
    } else if method == "status" {
        match rpc_serve_cached_status(context) {
            Ok((report, cache_hit)) => {
                health_cache_hit = cache_hit;
                success_response(
                &id,
                &report,
                vec![RpcEvent::new("status", report.node_id.clone(), "status queried")],
            )
            .unwrap_or_else(|error| {
                rpc_serve_error_response(
                    &id,
                    "rpc_internal",
                    &format!("status serialization failed: {error}"),
                )
            })
            }
            Err(error) => rpc_serve_error_response(&id, "rpc_status_failed", &error.to_string()),
        }
    } else if method == "mempool_status" {
        match rpc_serve_cached_mempool(context) {
            Ok((mempool, cache_hit)) => {
                health_cache_hit = cache_hit;
                success_response(
                &id,
                &mempool,
                vec![RpcEvent::new(
                    "mempool_status",
                    mempool.len().to_string(),
                    "mempool queried",
                )],
            )
            .unwrap_or_else(|error| {
                rpc_serve_error_response(
                    &id,
                    "rpc_internal",
                    &format!("mempool serialization failed: {error}"),
                )
            })
            }
            Err(error) => {
                rpc_serve_error_response(&id, "rpc_mempool_status_failed", &error.to_string())
            }
        }
    } else if rpc_serve_method_accepts_proxy_parent_wait(&method)
        && request
            .params
            .as_object()
            .map(|params| params.contains_key("proxy_required_current_height"))
            .unwrap_or(false)
    {
        match request.params.as_object() {
            Some(params) => match wait_for_rpc_finality_required_parent(context, params) {
                Ok(_) => {
                    if method == "transfer_fee_quote" {
                        run_rpc_serve_transfer_fee_quote(&request, context)
                    } else if method == "atomic_swap_fee_quote" {
                        run_rpc_serve_atomic_swap_fee_quote(&request, context)
                    } else {
                        match run_rpc_request_via_child(
                            &context.data_dir,
                            &context.spool_dir,
                            request_index,
                            &line,
                            context.child_timeout_ms,
                        ) {
                            Ok(response) => response,
                            Err(error) => rpc_serve_child_error_response(&id, &error),
                        }
                    }
                }
                Err((code, message)) => rpc_serve_error_response(&id, &code, &message),
            },
            None => rpc_serve_error_response(
                &id,
                "rpc_protocol_error",
                "proxy parent-wait request params must be an object",
            ),
        }
    } else if is_mempool_submit_signed_method(&method) {
        let counters = record_rpc_serve_mempool_submit_attempt(
            &context.mempool_submit_state,
            &context.peer_addr,
        )
        .unwrap_or(RpcServeMempoolSubmitCounters {
            peer_count: u64::MAX,
            total_count: u64::MAX,
            active_count: u64::MAX,
        });
        mempool_submit_counts = Some(counters);
        if counters.total_count > context.max_mempool_submit_total {
            rpc_serve_error_response(
                &id,
                "rpc_mempool_submit_global_rate_limited",
                &format!(
                    "rpc serve exceeded {method} total limit {}",
                    context.max_mempool_submit_total
                ),
            )
        } else if counters.peer_count > context.max_mempool_submit_per_peer {
            rpc_serve_error_response(
                &id,
                "rpc_mempool_submit_rate_limited",
                &format!(
                    "peer `{}` exceeded {method} limit {}",
                    context.peer_addr, context.max_mempool_submit_per_peer
                ),
            )
        } else {
            run_serialized_rpc_mempool_submit(request_index, &request, &line, context)
        }
    } else if method == "transfer_fee_quote" {
        run_rpc_serve_transfer_fee_quote(&request, context)
    } else if method == "atomic_swap_fee_quote" {
        run_rpc_serve_atomic_swap_fee_quote(&request, context)
    } else if is_orchard_batch_create_method(&method) {
        let counters = record_rpc_serve_mempool_submit_attempt(
            &context.orchard_batch_create_state,
            &context.peer_addr,
        )
        .unwrap_or(RpcServeMempoolSubmitCounters {
            peer_count: u64::MAX,
            total_count: u64::MAX,
            active_count: u64::MAX,
        });
        orchard_batch_create_counts = Some(counters);
        if let Err(error) = validate_rpc_serve_orchard_batch_create_request(&request) {
            rpc_serve_error_response(&id, "rpc_orchard_batch_create_not_public_safe", &error)
        } else if counters.total_count > context.max_orchard_batch_create_total {
            rpc_serve_error_response(
                &id,
                "rpc_orchard_batch_create_global_rate_limited",
                &format!(
                    "rpc serve exceeded Orchard batch-create total limit {}",
                    context.max_orchard_batch_create_total
                ),
            )
        } else if counters.peer_count > context.max_orchard_batch_create_per_peer {
            rpc_serve_error_response(
                &id,
                "rpc_orchard_batch_create_rate_limited",
                &format!(
                    "peer `{}` exceeded Orchard batch-create limit {}",
                    context.peer_addr, context.max_orchard_batch_create_per_peer
                ),
            )
        } else {
            match try_acquire_rpc_serve_active_orchard_worker(
                &context.orchard_batch_create_state,
                context.max_orchard_batch_create_concurrent,
            ) {
                Ok((_guard, active_count)) => {
                    if let Some(counts) = orchard_batch_create_counts.as_mut() {
                        counts.active_count = active_count;
                    }
                    match run_rpc_request_via_child(
                        &context.data_dir,
                        &context.spool_dir,
                        request_index,
                        &line,
                        context.child_timeout_ms,
                    ) {
                        Ok(response) => response,
                        Err(error) => rpc_serve_child_error_response(&id, &error),
                    }
                }
                Err(error) => rpc_serve_error_response(
                    &id,
                    "rpc_orchard_batch_create_concurrency_limited",
                    &error,
                ),
            }
        }
    } else if matches!(
        method.as_str(),
        "fastswap_capabilities"
            | "fastswap_preview"
            | "fastswap_prepare"
            | "fastswap_commit"
            | "fastswap_apply"
            | "fastswap_catch_up"
            | "fastswap_status"
            | "fastswap_effects"
            | "fastswap_votes"
            | "fastswap_new_round_vote"
            | "fastswap_propose_round"
            | "fastswap_precommit"
            | "fastswap_commit_round"
            | "fastswap_cancel_apply"
            | "fastlane_exit"
            | "fastswap_checkpoint_status"
            | "fastswap_objects"
            | "fastswap_policy"
            | "fastlane_asset_control_prepare"
            | "fastlane_asset_control_preview"
            | "fastlane_asset_control_apply"
            | "fastlane_asset_control_catch_up"
    ) {
        match rpc_serve_fastswap_service(context) {
            Ok(mut service) => {
                let result: Result<serde_json::Value, std::io::Error> = service
                    .as_mut()
                    .ok_or_else(|| std::io::Error::other("FastSwap service unavailable"))
                    .and_then(|service| match method.as_str() {
                    "fastswap_capabilities" => serde_json::to_value(service.capabilities())
                        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                    "fastswap_preview" => request
                        .params
                        .get("signed_intent_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_preview requires signed_intent_json"))
                        .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::SignedFastSwapIntentV1>(json, "FastSwap signed intent"))
                        .and_then(|signed| service.preview(&signed))
                        .and_then(|preview| serde_json::to_value(preview).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_prepare" => request
                        .params
                        .get("signed_intent_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_prepare requires signed_intent_json"))
                        .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::SignedFastSwapIntentV1>(json, "FastSwap signed intent"))
                        .and_then(|signed| service.prepare(&signed))
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_commit" => request
                        .params
                        .get("lock_qc_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_commit requires lock_qc_json"))
                        .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::FastSwapCertificateV1>(json, "FastSwap LockQC"))
                        .and_then(|certificate| service.commit(&certificate))
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_apply" => request
                        .params
                        .get("decision_qc_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_apply requires decision_qc_json"))
                        .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::FastSwapCertificateV1>(json, "FastSwap DecisionQC"))
                        .and_then(|certificate| {
                            request.params.get("signed_intent_json")
                                .and_then(serde_json::Value::as_str)
                                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_apply requires signed_intent_json"))
                                .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::SignedFastSwapIntentV1>(json, "FastSwap signed intent"))
                                .and_then(|signed| service.apply(&certificate, &signed))
                        })
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_catch_up" => {
                        let parse_certificate = |name: &str| request.params.get(name)
                            .and_then(serde_json::Value::as_str)
                            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("fastswap_catch_up requires {name}")))
                            .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::FastSwapCertificateV1>(json, "FastSwap certificate"));
                        parse_certificate("lock_qc_json").and_then(|lock_qc| {
                            parse_certificate("decision_qc_json").and_then(|decision_qc| {
                                request.params.get("signed_intent_json")
                                    .and_then(serde_json::Value::as_str)
                                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_catch_up requires signed_intent_json"))
                                    .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::SignedFastSwapIntentV1>(json, "FastSwap signed intent"))
                                    .and_then(|signed| service.catch_up_confirm(&lock_qc, &decision_qc, &signed))
                            })
                        })
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                    },
                    "fastswap_status" | "fastswap_effects" => request
                        .params
                        .get("swap_id")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "FastSwap query requires swap_id"))
                        .and_then(crate::fastswap_service::parse_swap_id_hex)
                        .and_then(|swap_id| {
                            if method == "fastswap_status" {
                                serde_json::to_value(service.status(swap_id))
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
                            } else {
                                service.effects(swap_id).and_then(|effects| serde_json::to_value(effects)
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                            }
                        }),
                    "fastswap_votes" => request
                        .params
                        .get("swap_id")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_votes requires swap_id"))
                        .and_then(crate::fastswap_service::parse_swap_id_hex)
                        .and_then(|swap_id| {
                            let phase = request.params.get("phase")
                                .and_then(serde_json::Value::as_str)
                                .and_then(postfiat_rpc_sdk::parse_fastswap_phase)
                                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_votes requires a valid phase"))?;
                            let round = request.params.get("round")
                                .and_then(serde_json::Value::as_u64)
                                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_votes requires round"))?;
                            service.vote_evidence(swap_id, phase, round)
                        })
                        .and_then(|evidence| serde_json::to_value(evidence)
                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_new_round_vote" => request
                        .params
                        .get("swap_id")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_new_round_vote requires swap_id"))
                        .and_then(crate::fastswap_service::parse_swap_id_hex)
                        .and_then(|swap_id| request.params.get("target_round")
                            .and_then(serde_json::Value::as_u64)
                            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_new_round_vote requires target_round"))
                            .and_then(|round| service.new_round_vote(swap_id, round)))
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_propose_round" | "fastswap_precommit" => request
                        .params
                        .get("proposal_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "FastSwap round method requires proposal_json"))
                        .and_then(|json| serde_json::from_str::<postfiat_types::FastSwapProposalV1>(json)
                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastSwap proposal parse failed: {error}"))))
                        .and_then(|proposal| {
                            if method == "fastswap_propose_round" {
                                service.validate_round_proposal(&proposal)?;
                                Ok(serde_json::json!({"valid": true, "round": proposal.round}))
                            } else {
                                service.precommit_round(&proposal).and_then(|vote| serde_json::to_value(vote)
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                            }
                        }),
                    "fastswap_commit_round" => request
                        .params
                        .get("precommit_qc_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_commit_round requires precommit_qc_json"))
                        .and_then(|json| serde_json::from_str::<postfiat_types::FastSwapCertificateV1>(json)
                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastSwap PrecommitQC parse failed: {error}"))))
                        .and_then(|certificate| service.commit_round(&certificate))
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_cancel_apply" => request
                        .params
                        .get("decision_qc_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_cancel_apply requires decision_qc_json"))
                        .and_then(|json| parse_fastswap_wire_payload::<postfiat_types::FastSwapCertificateV1>(json, "FastSwap cancel DecisionQC"))
                        .and_then(|certificate| service.cancel_apply(&certificate))
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastlane_exit" => request
                        .params
                        .get("signed_exit_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_exit requires signed_exit_json"))
                        .and_then(|json| serde_json::from_str::<postfiat_types::SignedFastLaneExitIntentV1>(json)
                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane signed exit parse failed: {error}"))))
                        .and_then(|signed| service.exit(&signed))
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastswap_checkpoint_status" => {
                        let previous = request.params.get("previous_checkpoint_id")
                            .and_then(serde_json::Value::as_str)
                            .map(crate::fastswap_service::parse_checkpoint_id_hex)
                            .transpose();
                        previous.and_then(|previous| {
                            postfiat_storage::NodeStore::new(&context.data_dir)
                                .read_ledger()
                                .and_then(|ledger| service.checkpoint_status(&ledger, previous))
                                .and_then(|status| serde_json::to_value(status)
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                        })
                    },
                    "fastswap_objects" => {
                        let owner = request.params.get("owner_pubkey")
                            .and_then(serde_json::Value::as_str)
                            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastswap_objects requires owner_pubkey"))
                            .and_then(|value| postfiat_crypto_provider::hex_to_bytes(value)
                                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastSwap owner pubkey hex invalid: {error}"))));
                        owner.and_then(|owner| {
                            let asset = request.params.get("asset_id")
                                .and_then(serde_json::Value::as_str)
                                .map(crate::fastswap_service::parse_asset_id_hex)
                                .transpose()?;
                            let cursor_id = request.params.get("cursor_object_id").and_then(serde_json::Value::as_str);
                            let cursor_version = request.params.get("cursor_version").and_then(serde_json::Value::as_u64);
                            let cursor = match (cursor_id, cursor_version) {
                                (None, None) => None,
                                (Some(id), Some(version)) => Some(crate::fastswap_service::parse_object_key(id, version)?),
                                _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "FastSwap cursor id/version must be supplied together")),
                            };
                            let limit = request.params.get("limit").and_then(serde_json::Value::as_u64).unwrap_or(50);
                            let limit: usize = limit.try_into().map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "FastSwap object limit overflow"))?;
                            service.objects(&owner, asset, cursor, limit)
                                .and_then(|response| serde_json::to_value(response)
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                        })
                    },
                    "fastswap_policy" => {
                        let policy_hash = request.params.get("policy_hash")
                            .and_then(serde_json::Value::as_str)
                            .map(crate::fastswap_service::parse_policy_hash_hex)
                            .transpose();
                        policy_hash.and_then(|policy_hash| {
                            let asset_0 = request.params.get("asset_0").and_then(serde_json::Value::as_str)
                                .map(crate::fastswap_service::parse_asset_id_hex).transpose()?;
                            let asset_1 = request.params.get("asset_1").and_then(serde_json::Value::as_str)
                                .map(crate::fastswap_service::parse_asset_id_hex).transpose()?;
                            let pair = match (asset_0, asset_1) {
                                (None, None) => None,
                                (Some(a), Some(b)) => Some((a, b)),
                                _ => return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "FastSwap policy pair requires asset_0 and asset_1")),
                            };
                            service.policy(policy_hash, pair)
                                .and_then(|response| serde_json::to_value(response)
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                        })
                    },
                    "fastlane_asset_control_prepare" => request
                        .params
                        .get("signed_command_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_asset_control_prepare requires signed_command_json"))
                        .and_then(|json| serde_json::from_str::<postfiat_types::SignedFastAssetControlCommandV1>(json)
                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane asset control parse failed: {error}"))))
                        .and_then(|signed| service.asset_control_prepare(&signed))
                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastlane_asset_control_preview" => request
                        .params
                        .get("signed_command_json")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_asset_control_preview requires signed_command_json"))
                        .and_then(|json| serde_json::from_str::<postfiat_types::SignedFastAssetControlCommandV1>(json)
                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane asset control parse failed: {error}"))))
                        .and_then(|signed| service.asset_control_preview(&signed))
                        .and_then(|preview| serde_json::to_value(preview).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))),
                    "fastlane_asset_control_apply" => {
                        let decision_qc = request.params.get("decision_qc_json")
                            .and_then(serde_json::Value::as_str)
                            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_asset_control_apply requires decision_qc_json"))
                            .and_then(|json| serde_json::from_str::<postfiat_types::FastSwapCertificateV1>(json)
                                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane asset control DecisionQC parse failed: {error}"))));
                        decision_qc.and_then(|decision_qc| {
                            request.params.get("signed_command_json")
                                .and_then(serde_json::Value::as_str)
                                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_asset_control_apply requires signed_command_json"))
                                .and_then(|json| serde_json::from_str::<postfiat_types::SignedFastAssetControlCommandV1>(json)
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane asset control parse failed: {error}"))))
                                .and_then(|signed| service.asset_control_apply(&decision_qc, &signed))
                                .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                        })
                    },
                    "fastlane_asset_control_catch_up" => {
                        let lock_qc = request.params.get("lock_qc_json")
                            .and_then(serde_json::Value::as_str)
                            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_asset_control_catch_up requires lock_qc_json"))
                            .and_then(|json| serde_json::from_str::<postfiat_types::FastSwapCertificateV1>(json)
                                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane asset control LockQC parse failed: {error}"))));
                        lock_qc.and_then(|lock_qc| {
                            request.params.get("decision_qc_json")
                                .and_then(serde_json::Value::as_str)
                                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_asset_control_catch_up requires decision_qc_json"))
                                .and_then(|json| serde_json::from_str::<postfiat_types::FastSwapCertificateV1>(json)
                                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane asset control DecisionQC parse failed: {error}"))))
                                .and_then(|decision_qc| {
                                    request.params.get("signed_command_json")
                                        .and_then(serde_json::Value::as_str)
                                        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "fastlane_asset_control_catch_up requires signed_command_json"))
                                        .and_then(|json| serde_json::from_str::<postfiat_types::SignedFastAssetControlCommandV1>(json)
                                            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("FastLane asset control parse failed: {error}"))))
                                        .and_then(|signed| service.asset_control_catch_up(&lock_qc, &decision_qc, &signed))
                                        .and_then(|vote| serde_json::to_value(vote).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)))
                                })
                        })
                    },
                    _ => Err(std::io::Error::other("unsupported FastSwap method")),
                });
                match result {
                    Ok(value) => fastswap_success_response(&request, &value),
                    Err(error) => rpc_serve_error_response(&id, "fastswap_failed", &error.to_string()),
                }
            }
            Err(error) => rpc_serve_error_response(&id, "fastswap_unavailable", &error.to_string()),
        }
    } else if method == "owned_sign_v3" {
        let validator_id = request
            .params
            .get("validator_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if validator_id.is_empty() {
            rpc_serve_error_response(
                &id,
                "rpc_protocol_error",
                "owned_sign_v3 requires validator_id",
            )
        } else {
            match rpc_owned_sign_order_json(&request) {
                Ok(order_json) => match crate::owned_sign_v3(
                    NodeOptions {
                        data_dir: context.data_dir.clone(),
                    },
                    &order_json,
                    validator_id,
                ) {
                    Ok(vote_json) => rpc_owned_json_response(
                        &id,
                        vote_json,
                        "FastPay v3 vote serialization failed",
                    ),
                    Err(error) => rpc_serve_error_response(
                        &id,
                        "owned_sign_v3_failed",
                        &error.to_string(),
                    ),
                },
                Err(error) => rpc_serve_error_response(
                    &id,
                    "rpc_protocol_error",
                    &format!("owned_sign_v3 {error}"),
                ),
            }
        }
    } else if method == "owned_apply_v3" {
        let validator_id = request
            .params
            .get("validator_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let cert_json = request
            .params
            .get("cert_json")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if validator_id.is_empty() || cert_json.is_empty() {
            rpc_serve_error_response(
                &id,
                "rpc_protocol_error",
                "owned_apply_v3 requires validator_id and cert_json",
            )
        } else {
            match crate::owned_apply_v3(
                NodeOptions {
                    data_dir: context.data_dir.clone(),
                },
                cert_json,
                validator_id,
            ) {
                Ok(ack_json) => rpc_owned_json_response(
                    &id,
                    ack_json,
                    "FastPay v3 apply acknowledgement serialization failed",
                ),
                Err(error) => rpc_serve_error_response(
                    &id,
                    "owned_apply_v3_failed",
                    &error.to_string(),
                ),
            }
        }
    } else if method == "owned_unwrap_sign_v3" {
        let validator_id = request
            .params
            .get("validator_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if validator_id.is_empty() {
            rpc_serve_error_response(
                &id,
                "rpc_protocol_error",
                "owned_unwrap_sign_v3 requires validator_id",
            )
        } else {
            match rpc_owned_sign_order_json(&request) {
                Ok(order_json) => match crate::owned_unwrap_sign_v3(
                    NodeOptions {
                        data_dir: context.data_dir.clone(),
                    },
                    &order_json,
                    validator_id,
                ) {
                    Ok(vote_json) => rpc_owned_json_response(
                        &id,
                        vote_json,
                        "FastPay v3 unwrap vote serialization failed",
                    ),
                    Err(error) => rpc_serve_error_response(
                        &id,
                        "owned_unwrap_sign_v3_failed",
                        &error.to_string(),
                    ),
                },
                Err(error) => rpc_serve_error_response(
                    &id,
                    "rpc_protocol_error",
                    &format!("owned_unwrap_sign_v3 {error}"),
                ),
            }
        }
    } else if method == "owned_unwrap_apply_v3" {
        let validator_id = request
            .params
            .get("validator_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let cert_json = request
            .params
            .get("cert_json")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if validator_id.is_empty() || cert_json.is_empty() {
            rpc_serve_error_response(
                &id,
                "rpc_protocol_error",
                "owned_unwrap_apply_v3 requires validator_id and cert_json",
            )
        } else {
            match crate::owned_unwrap_apply_v3(
                NodeOptions {
                    data_dir: context.data_dir.clone(),
                },
                cert_json,
                validator_id,
            ) {
                Ok(ack_json) => rpc_owned_json_response(
                    &id,
                    ack_json,
                    "FastPay v3 unwrap acknowledgement serialization failed",
                ),
                Err(error) => rpc_serve_error_response(
                    &id,
                    "owned_unwrap_apply_v3_failed",
                    &error.to_string(),
                ),
            }
        }
    } else if method == "owned_recovery_capabilities" {
        match crate::owned_recovery_capabilities_v3(NodeOptions {
            data_dir: context.data_dir.clone(),
        }) {
            Ok(capabilities_json) => rpc_owned_json_response(
                &id,
                capabilities_json,
                "FastPay recovery capabilities serialization failed",
            ),
            Err(error) => rpc_serve_error_response(
                &id,
                "owned_recovery_capabilities_failed",
                &error.to_string(),
            ),
        }
    } else if method == "owned_certificate" {
        let selector = request
            .params
            .get("lock_id")
            .or_else(|| request.params.get("certificate_digest"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if selector.is_empty() {
            rpc_serve_error_response(
                &id,
                "rpc_protocol_error",
                "owned_certificate requires lock_id or certificate_digest",
            )
        } else {
            match crate::owned_certificate_v3(
                NodeOptions {
                    data_dir: context.data_dir.clone(),
                },
                selector,
            ) {
                Ok(certificate_json) => rpc_owned_json_response(
                    &id,
                    certificate_json,
                    "FastPay certificate serialization failed",
                ),
                Err(error) => rpc_serve_error_response(
                    &id,
                    "owned_certificate_failed",
                    &error.to_string(),
                ),
            }
        }
    } else if method == "owned_recovery_status" {
        let lock_id = request
            .params
            .get("lock_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if lock_id.is_empty() {
            rpc_serve_error_response(
                &id,
                "rpc_protocol_error",
                "owned_recovery_status requires lock_id",
            )
        } else {
            match crate::owned_recovery_status_v3(
                NodeOptions {
                    data_dir: context.data_dir.clone(),
                },
                lock_id,
            ) {
                Ok(status_json) => rpc_owned_json_response(
                    &id,
                    status_json,
                    "FastPay recovery status serialization failed",
                ),
                Err(error) => rpc_serve_error_response(
                    &id,
                    "owned_recovery_status_failed",
                    &error.to_string(),
                ),
            }
        }
    } else if method == "owned_sign" {
        let validator_id = request
            .params
            .get("validator_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match rpc_owned_sign_order_json(&request) {
        Ok(order_json) => {
            match crate::owned_sign(
                NodeOptions { data_dir: context.data_dir.clone() },
                &order_json,
                validator_id,
            ) {
                Ok(vote_json) => {
                    let vote_value: serde_json::Value =
                        serde_json::from_str(&vote_json)
                            .unwrap_or(serde_json::Value::String(vote_json));
                    success_response(&id, &vote_value, vec![])
                        .unwrap_or_else(|_| rpc_serve_error_response(&id, "rpc_internal", "vote serialization failed"))
                }
                Err(error) => rpc_serve_error_response(
                    &id,
                    "owned_sign_failed",
                    &error.to_string(),
                ),
            }
        }
        Err(error) => rpc_serve_error_response(
            &id,
            "rpc_protocol_error",
            &format!("owned_sign {error}"),
        ),
        }
    } else if method == "owned_apply" {
        let cert_json = request
            .params
            .get("cert_json")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if cert_json.is_empty() {
            rpc_serve_error_response(&id, "rpc_protocol_error", "owned_apply requires cert_json")
        } else {
            match crate::owned_apply_report(NodeOptions { data_dir: context.data_dir.clone() }, cert_json) {
                Ok(report) => success_response(&id, &report, vec![])
                    .unwrap_or_else(|_| {
                        rpc_serve_error_response(&id, "rpc_internal", "owned apply serialization failed")
                    }),
                Err(error) => {
                    rpc_serve_error_response(&id, "owned_apply_failed", &error.to_string())
                }
            }
        }
    } else if method == "owned_unwrap_sign" {
        let validator_id = request
            .params
            .get("validator_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match rpc_owned_sign_order_json(&request) {
        Ok(order_json) => {
            match crate::owned_unwrap_sign(
                NodeOptions { data_dir: context.data_dir.clone() },
                &order_json,
                validator_id,
            ) {
                Ok(vote_json) => {
                    let vote_value: serde_json::Value =
                        serde_json::from_str(&vote_json)
                            .unwrap_or(serde_json::Value::String(vote_json));
                    success_response(&id, &vote_value, vec![])
                        .unwrap_or_else(|_| rpc_serve_error_response(&id, "rpc_internal", "unwrap vote serialization failed"))
                }
                Err(error) => rpc_serve_error_response(
                    &id,
                    "owned_unwrap_sign_failed",
                    &error.to_string(),
                ),
            }
        }
        Err(error) => rpc_serve_error_response(
            &id,
            "rpc_protocol_error",
            &format!("owned_unwrap_sign {error}"),
        ),
        }
    } else if method == "owned_unwrap_apply" {
        let cert_json = request
            .params
            .get("cert_json")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if cert_json.is_empty() {
            rpc_serve_error_response(&id, "rpc_protocol_error", "owned_unwrap_apply requires cert_json")
        } else {
            match crate::owned_unwrap_apply_report(NodeOptions { data_dir: context.data_dir.clone() }, cert_json) {
                Ok(report) => success_response(&id, &report, vec![])
                    .unwrap_or_else(|_| {
                        rpc_serve_error_response(&id, "rpc_internal", "owned unwrap apply serialization failed")
                    }),
                Err(error) => {
                    rpc_serve_error_response(&id, "owned_unwrap_apply_failed", &error.to_string())
                }
            }
        }
    } else if method == "server_info" {
        match run_rpc_request_via_child(
            &context.data_dir,
            &context.spool_dir,
            request_index,
            &line,
            context.child_timeout_ms,
        ) {
            Ok(mut response) => {
                if response.ok {
                    if let Some(result) = response.result.as_mut() {
                        merge_rpc_serve_server_info_capabilities(
                            result,
                            context.allow_mempool_submit,
                            context.allow_mempool_submit_finality,
                            context.allow_orchard_batch_create,
                            context.owned_lane_enabled,
                            &context.owned_certificate_domain,
                            context.max_mempool_submit_per_peer,
                            context.max_mempool_submit_total,
                        );
                    }
                }
                response
            }
            Err(error) => rpc_serve_child_error_response(&id, &error),
        }
    } else {
        match run_rpc_request_via_child(
            &context.data_dir,
            &context.spool_dir,
            request_index,
            &line,
            context.child_timeout_ms,
        ) {
            Ok(response) => response,
            Err(error) => rpc_serve_child_error_response(&id, &error),
        }
    };
    if method == "metrics" && response.ok {
        if let Some(result) = response.result.as_mut() {
            merge_rpc_serve_runtime_metrics(result, context.runtime_metrics.snapshot());
        }
    }
    let _ = write_json_line(reader.get_mut(), &response);
    let mut event = rpc_serve_event(
        &context.node_id,
        request_index,
        &context.peer_addr,
        id,
        method.clone(),
        mempool_submit_counts,
        orchard_batch_create_counts,
        &response,
    );
    event.handler_mode = Some(if matches!(method.as_str(), "status" | "mempool_status") {
        if health_cache_hit {
            "in_process_health_cache".to_string()
        } else {
            "in_process_health_refresh".to_string()
        }
    } else {
        "isolated_or_specialized".to_string()
    });
    event.handler_ms = Some(monotonic_elapsed_ms(handler_started));
    event
}

fn merge_rpc_serve_runtime_metrics(
    result: &mut serde_json::Value,
    snapshot: RpcServeRuntimeMetricsSnapshot,
) {
    let Some(result) = result.as_object_mut() else {
        return;
    };
    let utilization_ppm = if snapshot.active_connection_limit == 0 {
        1_000_000
    } else {
        snapshot
            .active_connections
            .saturating_mul(1_000_000)
            .checked_div(snapshot.active_connection_limit)
            .unwrap_or(1_000_000)
            .min(1_000_000)
    };
    result.insert(
        "rpc".to_string(),
        serde_json::json!({
            "active_connections": snapshot.active_connections,
            "active_connection_limit": snapshot.active_connection_limit,
            "active_connection_utilization_ppm": utilization_ppm,
            "peak_active_connections": snapshot.peak_active_connections,
            "accepted_connection_count": snapshot.accepted_connection_count,
        }),
    );
}

fn merge_rpc_serve_server_info_capabilities(
    result: &mut serde_json::Value,
    allow_mempool_submit: bool,
    allow_mempool_submit_finality: bool,
    allow_orchard_batch_create: bool,
    owned_lane_enabled: bool,
    owned_certificate_domain: &postfiat_types::OwnedCertificateDomain,
    max_mempool_submit_per_peer: u64,
    max_mempool_submit_total: u64,
) {
    let Some(obj) = result.as_object_mut() else {
        return;
    };
    let read_only = !allow_mempool_submit
        && !allow_mempool_submit_finality
        && !allow_orchard_batch_create
        && !owned_lane_enabled;
    let rpc = obj
        .entry("rpc".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let Some(rpc) = rpc.as_object_mut() else {
        return;
    };
    rpc.insert(
        "version".to_string(),
        serde_json::json!(postfiat_rpc_sdk::RPC_VERSION),
    );
    rpc.insert("read_only".to_string(), serde_json::json!(read_only));
    rpc.insert(
        "mempool_submit_enabled".to_string(),
        serde_json::json!(allow_mempool_submit || allow_mempool_submit_finality),
    );
    rpc.insert(
        "mempool_submit_finality_enabled".to_string(),
        serde_json::json!(allow_mempool_submit_finality),
    );
    rpc.insert(
        "mempool_submit_asset_transaction_finality_enabled".to_string(),
        serde_json::json!(allow_mempool_submit_finality),
    );
    rpc.insert(
        "mempool_submit_atomic_swap_finality_enabled".to_string(),
        serde_json::json!(allow_mempool_submit_finality),
    );
    rpc.insert(
        "owned_lane_enabled".to_string(),
        serde_json::json!(owned_lane_enabled),
    );
    rpc.insert(
        "owned_certificate_domain".to_string(),
        serde_json::json!(owned_certificate_domain),
    );
    rpc.insert(
        "max_mempool_submit_per_peer".to_string(),
        serde_json::json!(max_mempool_submit_per_peer),
    );
    rpc.insert(
        "max_mempool_submit_total".to_string(),
        serde_json::json!(max_mempool_submit_total),
    );
    rpc.insert(
        "mempool_submit_rate_limit_window_secs".to_string(),
        serde_json::json!(RPC_SERVE_RATE_LIMIT_WINDOW.as_secs()),
    );
    rpc.insert(
        "health_cache_max_age_ms".to_string(),
        serde_json::json!(RPC_SERVE_HEALTH_STAMP_MAX_AGE.as_millis()),
    );
}

fn rpc_serve_cached_status(
    context: &RpcServeConnectionContext,
) -> Result<(StatusReport, bool), String> {
    if let Ok(cache) = context.health_cache.lock() {
        if cache.status_checked_at.is_some_and(|checked_at| {
            checked_at.elapsed() <= RPC_SERVE_HEALTH_STAMP_MAX_AGE
        }) {
            if let Some((_, report)) = cache.status.as_ref() {
                return Ok((report.clone(), true));
            }
        }
    }
    let current_stamp = rpc_serve_health_stamp(&context.data_dir, true)?;
    if let Ok(mut cache) = context.health_cache.lock() {
        cache.status_checked_at = Some(Instant::now());
        if let Some((stamp, report)) = cache.status.as_ref() {
            if *stamp == current_stamp {
                return Ok((report.clone(), true));
            }
        }
    }
    let report = status(NodeOptions {
        data_dir: context.data_dir.clone(),
    })
    .map_err(|error| format!("rpc status read failed: {error}"))?;
    if let Ok(mut cache) = context.health_cache.lock() {
        cache.status = Some((current_stamp, report.clone()));
    }
    Ok((report, false))
}

fn rpc_serve_cached_mempool(
    context: &RpcServeConnectionContext,
) -> Result<(postfiat_types::MempoolState, bool), String> {
    if let Ok(cache) = context.health_cache.lock() {
        if cache.mempool_checked_at.is_some_and(|checked_at| {
            checked_at.elapsed() <= RPC_SERVE_HEALTH_STAMP_MAX_AGE
        }) {
            if let Some((_, mempool)) = cache.mempool.as_ref() {
                return Ok((mempool.clone(), true));
            }
        }
    }
    let current_stamp = rpc_serve_health_stamp(&context.data_dir, false)?;
    if let Ok(mut cache) = context.health_cache.lock() {
        cache.mempool_checked_at = Some(Instant::now());
        if let Some((stamp, mempool)) = cache.mempool.as_ref() {
            if *stamp == current_stamp {
                return Ok((mempool.clone(), true));
            }
        }
    }
    let mempool = mempool_state(NodeOptions {
        data_dir: context.data_dir.clone(),
    })
    .map_err(|error| format!("rpc mempool status read failed: {error}"))?;
    if let Ok(mut cache) = context.health_cache.lock() {
        cache.mempool = Some((current_stamp, mempool.clone()));
    }
    Ok((mempool, false))
}

fn rpc_serve_health_stamp(data_dir: &Path, include_status_files: bool) -> Result<RpcServeHealthStamp, String> {
    let mut names = vec![postfiat_storage::MEMPOOL_FILE];
    if include_status_files {
        names.extend([
            postfiat_storage::CHAIN_TIP_FILE,
            postfiat_storage::NODE_STATE_FILE,
            postfiat_storage::GOVERNANCE_FILE,
        ]);
    }
    let mut stamp = Vec::with_capacity(names.len());
    for name in names {
        let path = data_dir.join(name);
        let metadata = std::fs::metadata(&path).map_err(|error| {
            rpc_serve_storage_error(&format!("health stamp `{}` metadata", path.display()), error)
        })?;
        let modified = metadata
            .modified()
            .map_err(|error| {
                rpc_serve_storage_error(&format!("health stamp `{}` modified time", path.display()), error)
            })?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("rpc serve health stamp `{}` predates epoch: {error}", path.display()))?
            .as_nanos();
        stamp.push((metadata.len(), modified));
    }
    Ok(RpcServeHealthStamp(stamp))
}

fn record_rpc_serve_mempool_submit_attempt(
    state: &Arc<Mutex<RpcServeMempoolSubmitState>>,
    peer_addr: &str,
) -> Result<RpcServeMempoolSubmitCounters, String> {
    let mut state = state
        .lock()
        .map_err(|_| "rpc serve mempool submit counter lock poisoned".to_string())?;
    let now = Instant::now();
    prune_rpc_serve_rate_limit_window(&mut state, now);
    state.total_timestamps.push_back(now);
    let peer_count = {
        let peer_timestamps = state.counts_by_peer.entry(peer_addr.to_string()).or_default();
        peer_timestamps.push_back(now);
        peer_timestamps.len() as u64
    };
    Ok(RpcServeMempoolSubmitCounters {
        peer_count,
        total_count: state.total_timestamps.len() as u64,
        active_count: state.active_count,
    })
}

fn prune_rpc_serve_rate_limit_window(state: &mut RpcServeMempoolSubmitState, now: Instant) {
    while state
        .total_timestamps
        .front()
        .is_some_and(|timestamp| {
            now.checked_duration_since(*timestamp)
                .is_some_and(|age| age > RPC_SERVE_RATE_LIMIT_WINDOW)
        })
    {
        state.total_timestamps.pop_front();
    }

    state.counts_by_peer.retain(|_, timestamps| {
        while timestamps.front().is_some_and(|timestamp| {
            now.checked_duration_since(*timestamp)
                .is_some_and(|age| age > RPC_SERVE_RATE_LIMIT_WINDOW)
        }) {
            timestamps.pop_front();
        }
        !timestamps.is_empty()
    });
}

fn try_acquire_rpc_serve_active_orchard_worker(
    state: &Arc<Mutex<RpcServeMempoolSubmitState>>,
    max_concurrent: u64,
) -> Result<(RpcServeActiveCounterGuard, u64), String> {
    let mut state_guard = state
        .lock()
        .map_err(|_| "rpc serve Orchard worker counter lock poisoned".to_string())?;
    if state_guard.active_count >= max_concurrent {
        return Err(format!(
            "rpc serve exceeded Orchard batch-create concurrent verifier limit {max_concurrent}"
        ));
    }
    state_guard.active_count = state_guard.active_count.saturating_add(1);
    let active_count = state_guard.active_count;
    drop(state_guard);
    Ok((
        RpcServeActiveCounterGuard {
            state: Arc::clone(state),
        },
        active_count,
    ))
}

fn is_orchard_batch_create_method(method: &str) -> bool {
    matches!(
        method,
        "shield_batch_orchard"
            | "shield_batch_orchard_deposit"
            | "shield_batch_orchard_withdraw"
            | "shield_batch_swap"
    )
}

fn is_mempool_submit_signed_method(method: &str) -> bool {
    matches!(
        method,
        RPC_FINALITY_TIMEOUT_VOTE_METHOD
            | "mempool_submit_signed_transfer"
            | "mempool_submit_signed_transfer_finality"
            | "mempool_submit_signed_payment_v2"
            | "mempool_submit_signed_payment_v2_finality"
            | "mempool_submit_signed_asset_transaction_finality"
            | "mempool_submit_signed_asset_transaction"
            | "mempool_submit_signed_atomic_swap_transaction"
            | "mempool_submit_signed_atomic_swap_transaction_finality"
            | "mempool_submit_fastlane_primary"
            | "mempool_submit_fastlane_primary_finality"
            | "mempool_submit_signed_escrow_transaction"
            | "mempool_submit_signed_escrow_transaction_finality"
            | "mempool_submit_signed_nft_transaction"
            | "mempool_submit_signed_offer_transaction"
            | "shield_batch_finality"
    )
}

/// Finality submit methods share the finality gate and rate-limit counter.
fn is_mempool_submit_signed_transfer_finality_method(method: &str) -> bool {
    matches!(
        method,
        RPC_FINALITY_TIMEOUT_VOTE_METHOD
            | "mempool_submit_signed_transfer_finality"
            | "mempool_submit_signed_payment_v2_finality"
            | "mempool_submit_signed_asset_transaction_finality"
            | "mempool_submit_signed_atomic_swap_transaction_finality"
            | "mempool_submit_fastlane_primary_finality"
            | "mempool_submit_signed_escrow_transaction_finality"
            | "shield_batch_finality"
    )
}

fn run_rpc_serve_shield_batch_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_shield_batch_finality_inner(request_index, request, context) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_shield_batch_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "shield_batch_finality params must be an object".to_string(),
        )
    })?;
    let batch_json = params
        .get("batch_json")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "shield_batch_finality requires inline batch_json".to_string(),
            )
        })?;
    let batch: serde_json::Value = serde_json::from_str(batch_json).map_err(|error| {
        (
            "rpc_protocol_error".to_string(),
            format!("shield_batch_finality batch_json is invalid: {error}"),
        )
    })?;
    let batch_id = batch
        .get("batch_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "shield_batch_finality batch has no batch_id".to_string(),
            )
        })?;
    if batch_id.len() != 96 || !batch_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err((
            "rpc_protocol_error".to_string(),
            "shield_batch_finality batch_id must be 96 hexadecimal characters".to_string(),
        ));
    }
    let actions = batch
        .get("actions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "shield_batch_finality batch has no actions array".to_string(),
            )
        })?;
    if actions.is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "shield_batch_finality batch actions must not be empty".to_string(),
        ));
    }

    let _finality_guard = context.finality_submit_lock.try_lock().map_err(|error| match error {
        std::sync::TryLockError::WouldBlock => (
            "rpc_finality_submit_busy".to_string(),
            "another finality submit is already in progress".to_string(),
        ),
        std::sync::TryLockError::Poisoned(_) => (
            "rpc_finality_submit_lock_poisoned".to_string(),
            "finality submit lock poisoned".to_string(),
        ),
    })?;
    let artifact_dir = context
        .finality_artifact_root
        .join(format!("rpc-shield-batch-finality-{batch_id}"));
    std::fs::create_dir_all(&artifact_dir).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("shield finality artifact directory create failed: {error}"),
        )
    })?;
    let batch_file = artifact_dir.join("shield-batch.json");
    let result_file = artifact_dir.join("result.json");
    if batch_file.is_file() {
        let existing = std::fs::read_to_string(&batch_file).map_err(|error| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("shield finality durable batch read failed: {error}"),
            )
        })?;
        if existing.trim() != batch_json.trim() {
            return Err((
                "rpc_protocol_error".to_string(),
                "shield_batch_finality batch_id was reused with different bytes".to_string(),
            ));
        }
    } else {
        postfiat_storage::atomic_write(&batch_file, format!("{}\n", batch_json.trim()).as_bytes())
            .map_err(|error| {
                (
                    "rpc_finality_submit_failed".to_string(),
                    format!("shield finality durable batch write failed: {error}"),
                )
            })?;
    }
    if result_file.is_file() {
        let result_bytes = std::fs::read(&result_file).map_err(|error| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("shield finality cached result read failed: {error}"),
            )
        })?;
        let result: serde_json::Value = serde_json::from_slice(&result_bytes).map_err(|error| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("shield finality cached result decode failed: {error}"),
            )
        })?;
        return success_response(
            &request.id,
            &result,
            vec![RpcEvent::new(
                "shield_batch_finality",
                batch_id.to_string(),
                "cached certified shield batch returned",
            )],
        )
        .map_err(|error| ("rpc_internal".to_string(), error.to_string()));
    }
    let total_start = Instant::now();
    let readiness_wait_ms = wait_for_rpc_finality_required_parent(context, params)?;
    let finality_view = prepare_rpc_finality_view(&context.data_dir, params, &artifact_dir)?;
    require_rpc_finality_local_proposer(context, finality_view.view)?;
    let round = transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
        data_dir: context.data_dir.clone(),
        topology_file: context.finality_topology_file.clone(),
        batch_kind: Some("shielded".to_string()),
        batch_file: batch_file.clone(),
        key_file: context.finality_key_file.clone(),
        proposal_key_file: context.finality_proposal_key_file.clone(),
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: context.finality_quorum_early_full_propagation,
        artifact_dir: artifact_dir.join("round"),
        block_height: None,
        view: Some(finality_view.view),
        timeout_certificate_file: finality_view.timeout_certificate_file,
        timeout_ms: context.finality_timeout_ms,
        send_retries: context.finality_send_retries,
        retry_backoff_ms: context.finality_retry_backoff_ms,
        local_apply_before_certified_send: true,
        defer_certified_sends: true,
        required_parent: None,
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("shield_batch_finality failed: {error}"),
        )
    })?;
    let deferred_job_count = round.deferred_certified_send_jobs.len();
    if !round.certification.round_ok
        || !round.local_apply_verified
        || !round.all_vote_requests_verified
        || !round.vote_request_failures.is_empty()
        || !round.send_failures.is_empty()
        || deferred_job_count != round.target_peer_count
    {
        return Err((
            "rpc_finality_submit_failed".to_string(),
            format!(
                "shield_batch_finality did not produce verified local finality and one durable send job per peer (jobs {deferred_job_count}, targets {})",
                round.target_peer_count,
            ),
        ));
    }
    let certificate_bytes = std::fs::read(&round.certification.certificate_file).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("shield finality certificate read failed: {error}"),
        )
    })?;
    let certificate: serde_json::Value =
        serde_json::from_slice(&certificate_bytes).map_err(|error| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("shield finality certificate decode failed: {error}"),
            )
        })?;
    let result = serde_json::json!({
        "schema": "postfiat-rpc-shield-batch-finality-v1",
        "batch_id": batch_id,
        "request_index": request_index,
        "readiness_wait_ms": readiness_wait_ms,
        "total_ms": total_start.elapsed().as_secs_f64() * 1000.0,
        "certificate": certificate,
        "round": round,
    });
    let result_bytes = serde_json::to_vec_pretty(&result).map_err(|error| {
        (
            "rpc_internal".to_string(),
            format!("shield finality result serialization failed: {error}"),
        )
    })?;
    postfiat_storage::atomic_write(&result_file, [result_bytes.as_slice(), b"\n"].concat())
        .map_err(|error| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("shield finality result write failed: {error}"),
            )
        })?;
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "shield_batch_finality",
            batch_id.to_string(),
            "shield batch certified with validator-owned proposer key",
        )],
    )
    .map_err(|error| ("rpc_internal".to_string(), error.to_string()))
}

fn run_rpc_serve_mempool_submit_signed_transfer_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_mempool_submit_signed_transfer_finality_inner(
        request_index,
        request,
        context,
    ) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_mempool_submit_fastlane_primary_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_mempool_submit_fastlane_primary_finality_inner(
        request_index,
        request,
        context,
    ) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_mempool_submit_fastlane_primary_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "mempool_submit_fastlane_primary_finality params must be an object".to_string(),
        )
    })?;
    let transaction_json = params
        .get("fastlane_primary_json")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing or empty fastlane_primary_json".to_string(),
            )
        })?;
    let transaction: postfiat_types::FastLanePrimaryTransactionV1 =
        serde_json::from_str(transaction_json).map_err(|error| {
            (
                "rpc_protocol_error".to_string(),
                format!("FastLane primary JSON parse failed: {error}"),
            )
        })?;
    let _finality_guard = match context.finality_submit_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err((
                "rpc_finality_submit_busy".to_string(),
                "another finality submit is already in progress".to_string(),
            ));
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err((
                "rpc_finality_submit_lock_poisoned".to_string(),
                "finality submit lock poisoned".to_string(),
            ));
        }
    };

    let request_component = transport_artifact_component(&request.id);
    let artifact_dir = context
        .finality_artifact_root
        .join(format!("rpc-finality-{request_index}-{request_component}"));
    let total_start = Instant::now();
    let readiness_wait_ms = wait_for_rpc_finality_required_parent(context, params)?;
    let finality_view = prepare_rpc_finality_view(&context.data_dir, params, &artifact_dir)?;
    require_rpc_finality_local_proposer(context, finality_view.view)?;

    let mempool_submit_start = Instant::now();
    let entry = admit_fastlane_primary_to_mempool(&context.data_dir, transaction).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("mempool_submit_fastlane_primary_finality admission failed: {error}"),
        )
    })?;
    let mempool_submit_ms = monotonic_elapsed_ms(mempool_submit_start);
    let tx_id = entry.tx_id;

    let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
        data_dir: context.data_dir.clone(),
        topology_file: context.finality_topology_file.clone(),
        key_file: context.finality_key_file.clone(),
        proposal_key_file: context.finality_proposal_key_file.clone(),
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: context.finality_quorum_early_full_propagation,
        artifact_dir: artifact_dir.clone(),
        block_height: None,
        view: Some(finality_view.view),
        timeout_certificate_file: finality_view.timeout_certificate_file,
        timeout_ms: context.finality_timeout_ms,
        send_retries: context.finality_send_retries,
        retry_backoff_ms: context.finality_retry_backoff_ms,
        local_apply_before_certified_send: true,
        defer_certified_sends: true,
        required_parent: None,
        max_transactions: 1,
        signed_transfer_file: None,
        signed_transfer_json: None,
        signed_payment_v2_json: None,
        signed_asset_transaction_json: None,
        signed_atomic_swap_transaction_json: None,
        signed_escrow_transaction_json: None,
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("mempool_submit_fastlane_primary_finality failed: {error}"),
        )
    })?;
    let finality = round
        .round
        .local_hot_finality
        .iter()
        .find(|report| {
            report.tx_id == tx_id
                && report.confirmed
                && report.receipt.accepted
                && report.receipt.code == "owned_deposit_applied"
        })
        .cloned()
        .ok_or_else(|| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!(
                    "FastLane primary finality round did not emit owned_deposit_applied for `{tx_id}`"
                ),
            )
        })?;
    let total_ms = monotonic_elapsed_ms(total_start);
    let round_report_file = artifact_dir.join("rpc-finality-round.json");
    let round_json = serde_json::to_string_pretty(&round).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality round report serialization failed: {error}"),
        )
    })?;
    std::fs::write(&round_report_file, round_json).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!(
                "finality round report write `{}` failed: {error}",
                round_report_file.display()
            ),
        )
    })?;
    let result = RpcMempoolSubmitSignedTransferFinalityReport {
        schema: "postfiat-rpc-mempool-submit-fastlane-primary-finality-v1".to_string(),
        tx_id: tx_id.clone(),
        finality,
        round_report_file: round_report_file.display().to_string(),
        artifact_dir: artifact_dir.display().to_string(),
        readiness_wait_ms,
        mempool_submit_ms,
        mempool_batch_ms: round.mempool_batch_ms,
        certified_round_ms: round.round.timings.total_ms,
        total_ms,
        certified_sends_deferred: round.round.certified_sends_deferred,
        round_ok: round.round_ok,
    };
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "mempool_submit_fastlane_primary_finality",
            tx_id,
            "signed FastLane primary transaction finalized",
        )],
    )
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality RPC response serialization failed: {error}"),
        )
    })
}

fn run_rpc_serve_transfer_fee_quote(
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_transfer_fee_quote_inner(request, context) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_transfer_fee_quote_inner(
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "transfer_fee_quote params must be an object".to_string(),
        )
    })?;
    let from = params
        .get("from")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing from".to_string(),
            )
        })?;
    let to = params
        .get("to")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing to".to_string(),
            )
        })?;
    let amount = params
        .get("amount")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing or invalid amount".to_string(),
            )
        })?;
    let sequence = params.get("sequence").and_then(serde_json::Value::as_u64);
    let report = transfer_fee_quote(TransferFeeQuoteOptions {
        data_dir: context.data_dir.clone(),
        from: from.to_string(),
        to: to.to_string(),
        amount,
        sequence,
        memo_type: params
            .get("memo_type")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        memo_format: params
            .get("memo_format")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        memo_data: params
            .get("memo_data")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
    })
    .map_err(|error| {
        (
            "rpc_transfer_fee_quote_failed".to_string(),
            format!("rpc transfer_fee_quote failed: {error}"),
        )
    })?;
    success_response(
        &request.id,
        &report,
        vec![RpcEvent::new(
            "transfer_fee_quote",
            report.from.clone(),
            "transfer fee quoted",
        )],
    )
    .map_err(|error| {
        (
            "rpc_transfer_fee_quote_failed".to_string(),
            format!("transfer_fee_quote response serialization failed: {error}"),
        )
    })
}

fn run_rpc_serve_mempool_submit_signed_transfer_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "mempool_submit_signed_transfer_finality params must be an object".to_string(),
        )
    })?;
    if params.contains_key("transfer_file") {
        return Err((
            "rpc_protocol_error".to_string(),
            "remote finality submit requires signed_transfer_json, not transfer_file".to_string(),
        ));
    }
    let signed_transfer_json = params
        .get("signed_transfer_json")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing signed_transfer_json".to_string(),
            )
        })?;
    if signed_transfer_json.trim().is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "signed_transfer_json must be nonempty".to_string(),
        ));
    }
    let _finality_guard = match context.finality_submit_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err((
                "rpc_finality_submit_busy".to_string(),
                "another finality submit is already in progress".to_string(),
            ));
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err((
                "rpc_finality_submit_lock_poisoned".to_string(),
                "finality submit lock poisoned".to_string(),
            ));
        }
    };

    let request_component = transport_artifact_component(&request.id);
    let artifact_dir = context
        .finality_artifact_root
        .join(format!("rpc-finality-{request_index}-{request_component}"));
    let total_start = Instant::now();
    let readiness_wait_ms = wait_for_rpc_finality_required_parent(context, params)?;
    let finality_view = prepare_rpc_finality_view(&context.data_dir, params, &artifact_dir)?;
    require_rpc_finality_local_proposer(context, finality_view.view)?;
    let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
        data_dir: context.data_dir.clone(),
        topology_file: context.finality_topology_file.clone(),
        key_file: context.finality_key_file.clone(),
        proposal_key_file: context.finality_proposal_key_file.clone(),
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: context.finality_quorum_early_full_propagation,
        artifact_dir: artifact_dir.clone(),
        block_height: None,
        view: Some(finality_view.view),
        timeout_certificate_file: finality_view.timeout_certificate_file,
        timeout_ms: context.finality_timeout_ms,
        send_retries: context.finality_send_retries,
        retry_backoff_ms: context.finality_retry_backoff_ms,
        local_apply_before_certified_send: true,
        defer_certified_sends: true,
        required_parent: None,
        max_transactions: 1,
        signed_transfer_file: None,
        signed_transfer_json: Some(signed_transfer_json.to_string()),
        signed_payment_v2_json: None,
        signed_asset_transaction_json: None,
        signed_atomic_swap_transaction_json: None,
        signed_escrow_transaction_json: None,
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("mempool_submit_signed_transfer_finality failed: {error}"),
        )
    })?;
    let tx_id = round.submitted_tx_id.clone().ok_or_else(|| {
        (
            "rpc_finality_submit_failed".to_string(),
            "finality submit round did not report submitted_tx_id".to_string(),
        )
    })?;
    let finality = round
        .round
        .local_hot_finality
        .iter()
        .find(|report| {
            report.tx_id == tx_id && report.confirmed && report.receipt.accepted
        })
        .cloned()
        .ok_or_else(|| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("finality submit round did not emit accepted hot finality for `{tx_id}`"),
            )
        })?;
    let total_ms = monotonic_elapsed_ms(total_start);
    let round_report_file = artifact_dir.join("rpc-finality-round.json");
    let round_json = serde_json::to_string_pretty(&round).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality round report serialization failed: {error}"),
        )
    })?;
    std::fs::write(&round_report_file, round_json).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!(
                "finality round report write `{}` failed: {error}",
                round_report_file.display()
            ),
        )
    })?;
    let result = RpcMempoolSubmitSignedTransferFinalityReport {
        schema: "postfiat-rpc-mempool-submit-signed-transfer-finality-v1".to_string(),
        tx_id: tx_id.clone(),
        finality,
        round_report_file: round_report_file.display().to_string(),
        artifact_dir: artifact_dir.display().to_string(),
        readiness_wait_ms,
        mempool_submit_ms: round.mempool_submit_ms,
        mempool_batch_ms: round.mempool_batch_ms,
        certified_round_ms: round.round.timings.total_ms,
        total_ms,
        certified_sends_deferred: round.round.certified_sends_deferred,
        round_ok: round.round_ok,
    };
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "mempool_submit_signed_transfer_finality",
            tx_id,
            "externally signed transparent transfer finalized",
        )],
    )
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality RPC response serialization failed: {error}"),
        )
    })
}

fn run_rpc_serve_mempool_submit_signed_payment_v2_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_mempool_submit_signed_payment_v2_finality_inner(
        request_index,
        request,
        context,
    ) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_mempool_submit_signed_payment_v2_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "mempool_submit_signed_payment_v2_finality params must be an object".to_string(),
        )
    })?;
    let signed_payment_v2_json = params
        .get("signed_payment_v2_json")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing signed_payment_v2_json".to_string(),
            )
        })?;
    if signed_payment_v2_json.trim().is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "signed_payment_v2_json must be nonempty".to_string(),
        ));
    }
    let _finality_guard = match context.finality_submit_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err((
                "rpc_finality_submit_busy".to_string(),
                "another finality submit is already in progress".to_string(),
            ));
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err((
                "rpc_finality_submit_lock_poisoned".to_string(),
                "finality submit lock poisoned".to_string(),
            ));
        }
    };

    let request_component = transport_artifact_component(&request.id);
    let artifact_dir = context
        .finality_artifact_root
        .join(format!("rpc-finality-{request_index}-{request_component}"));
    let total_start = Instant::now();
    let readiness_wait_ms = wait_for_rpc_finality_required_parent(context, params)?;
    let finality_view = prepare_rpc_finality_view(&context.data_dir, params, &artifact_dir)?;
    require_rpc_finality_local_proposer(context, finality_view.view)?;
    let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
        data_dir: context.data_dir.clone(),
        topology_file: context.finality_topology_file.clone(),
        key_file: context.finality_key_file.clone(),
        proposal_key_file: context.finality_proposal_key_file.clone(),
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: context.finality_quorum_early_full_propagation,
        artifact_dir: artifact_dir.clone(),
        block_height: None,
        view: Some(finality_view.view),
        timeout_certificate_file: finality_view.timeout_certificate_file,
        timeout_ms: context.finality_timeout_ms,
        send_retries: context.finality_send_retries,
        retry_backoff_ms: context.finality_retry_backoff_ms,
        local_apply_before_certified_send: true,
        defer_certified_sends: true,
        required_parent: None,
        max_transactions: 1,
        signed_transfer_file: None,
        signed_transfer_json: None,
        signed_payment_v2_json: Some(signed_payment_v2_json.to_string()),
        signed_asset_transaction_json: None,
        signed_atomic_swap_transaction_json: None,
        signed_escrow_transaction_json: None,
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("mempool_submit_signed_payment_v2_finality failed: {error}"),
        )
    })?;
    let tx_id = round.submitted_tx_id.clone().ok_or_else(|| {
        (
            "rpc_finality_submit_failed".to_string(),
            "finality submit round did not report submitted_tx_id".to_string(),
        )
    })?;
    let finality = round
        .round
        .local_hot_finality
        .iter()
        .find(|report| report.tx_id == tx_id && report.confirmed && report.receipt.accepted)
        .cloned()
        .ok_or_else(|| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("finality submit round did not emit accepted hot finality for `{tx_id}`"),
            )
        })?;
    let total_ms = monotonic_elapsed_ms(total_start);
    let round_report_file = artifact_dir.join("rpc-finality-round.json");
    let round_json = serde_json::to_string_pretty(&round).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality round report serialization failed: {error}"),
        )
    })?;
    std::fs::write(&round_report_file, round_json).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!(
                "finality round report write `{}` failed: {error}",
                round_report_file.display()
            ),
        )
    })?;
    let result = RpcMempoolSubmitSignedTransferFinalityReport {
        schema: "postfiat-rpc-mempool-submit-signed-payment-v2-finality-v1".to_string(),
        tx_id: tx_id.clone(),
        finality,
        round_report_file: round_report_file.display().to_string(),
        artifact_dir: artifact_dir.display().to_string(),
        readiness_wait_ms,
        mempool_submit_ms: round.mempool_submit_ms,
        mempool_batch_ms: round.mempool_batch_ms,
        certified_round_ms: round.round.timings.total_ms,
        total_ms,
        certified_sends_deferred: round.round.certified_sends_deferred,
        round_ok: round.round_ok,
    };
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "mempool_submit_signed_payment_v2_finality",
            tx_id,
            "externally signed payment v2 finalized",
        )],
    )
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality RPC response serialization failed: {error}"),
        )
    })
}

fn run_rpc_serve_mempool_submit_signed_asset_transaction_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_mempool_submit_signed_asset_transaction_finality_inner(
        request_index,
        request,
        context,
    ) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_mempool_submit_signed_asset_transaction_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "mempool_submit_signed_asset_transaction_finality params must be an object".to_string(),
        )
    })?;
    let signed_asset_transaction_json = params
        .get("signed_asset_transaction_json")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing signed_asset_transaction_json".to_string(),
            )
        })?;
    if signed_asset_transaction_json.trim().is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "signed_asset_transaction_json must be nonempty".to_string(),
        ));
    }
    let _finality_guard = match context.finality_submit_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err((
                "rpc_finality_submit_busy".to_string(),
                "another finality submit is already in progress".to_string(),
            ));
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err((
                "rpc_finality_submit_lock_poisoned".to_string(),
                "finality submit lock poisoned".to_string(),
            ));
        }
    };

    let request_component = transport_artifact_component(&request.id);
    let artifact_dir = context
        .finality_artifact_root
        .join(format!("rpc-finality-{request_index}-{request_component}"));
    let total_start = Instant::now();
    let readiness_wait_ms = wait_for_rpc_finality_required_parent(context, params)?;
    let finality_view = prepare_rpc_finality_view(&context.data_dir, params, &artifact_dir)?;
    require_rpc_finality_local_proposer(context, finality_view.view)?;
    let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
        data_dir: context.data_dir.clone(),
        topology_file: context.finality_topology_file.clone(),
        key_file: context.finality_key_file.clone(),
        proposal_key_file: context.finality_proposal_key_file.clone(),
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: context.finality_quorum_early_full_propagation,
        artifact_dir: artifact_dir.clone(),
        block_height: None,
        view: Some(finality_view.view),
        timeout_certificate_file: finality_view.timeout_certificate_file,
        timeout_ms: context.finality_timeout_ms,
        send_retries: context.finality_send_retries,
        retry_backoff_ms: context.finality_retry_backoff_ms,
        local_apply_before_certified_send: true,
        defer_certified_sends: true,
        required_parent: None,
        max_transactions: 1,
        signed_transfer_file: None,
        signed_transfer_json: None,
        signed_payment_v2_json: None,
        signed_asset_transaction_json: Some(signed_asset_transaction_json.to_string()),
        signed_atomic_swap_transaction_json: None,
        signed_escrow_transaction_json: None,
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("mempool_submit_signed_asset_transaction_finality failed: {error}"),
        )
    })?;
    let tx_id = round.submitted_tx_id.clone().ok_or_else(|| {
        (
            "rpc_finality_submit_failed".to_string(),
            "finality submit round did not report submitted_tx_id".to_string(),
        )
    })?;
    let finality = round
        .round
        .local_hot_finality
        .iter()
        .find(|report| report.tx_id == tx_id && report.confirmed && report.receipt.accepted)
        .cloned()
        .ok_or_else(|| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("finality submit round did not emit accepted hot finality for `{tx_id}`"),
            )
        })?;
    let total_ms = monotonic_elapsed_ms(total_start);
    let round_report_file = artifact_dir.join("rpc-finality-round.json");
    let round_json = serde_json::to_string_pretty(&round).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality round report serialization failed: {error}"),
        )
    })?;
    std::fs::write(&round_report_file, round_json).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!(
                "finality round report write `{}` failed: {error}",
                round_report_file.display()
            ),
        )
    })?;
    let result = RpcMempoolSubmitSignedTransferFinalityReport {
        schema: "postfiat-rpc-mempool-submit-signed-asset-transaction-finality-v1".to_string(),
        tx_id: tx_id.clone(),
        finality,
        round_report_file: round_report_file.display().to_string(),
        artifact_dir: artifact_dir.display().to_string(),
        readiness_wait_ms,
        mempool_submit_ms: round.mempool_submit_ms,
        mempool_batch_ms: round.mempool_batch_ms,
        certified_round_ms: round.round.timings.total_ms,
        total_ms,
        certified_sends_deferred: round.round.certified_sends_deferred,
        round_ok: round.round_ok,
    };
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "mempool_submit_signed_asset_transaction_finality",
            tx_id,
            "externally signed asset transaction finalized",
        )],
    )
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality RPC response serialization failed: {error}"),
        )
    })
}

fn run_rpc_serve_mempool_submit_signed_escrow_transaction_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_mempool_submit_signed_escrow_transaction_finality_inner(
        request_index,
        request,
        context,
    ) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_mempool_submit_signed_escrow_transaction_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "mempool_submit_signed_escrow_transaction_finality params must be an object"
                .to_string(),
        )
    })?;
    let signed_escrow_transaction_json = params
        .get("signed_escrow_transaction_json")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing signed_escrow_transaction_json".to_string(),
            )
        })?;
    if signed_escrow_transaction_json.trim().is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "signed_escrow_transaction_json must be nonempty".to_string(),
        ));
    }
    let _finality_guard = match context.finality_submit_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err((
                "rpc_finality_submit_busy".to_string(),
                "another finality submit is already in progress".to_string(),
            ));
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err((
                "rpc_finality_submit_lock_poisoned".to_string(),
                "finality submit lock poisoned".to_string(),
            ));
        }
    };

    let request_component = transport_artifact_component(&request.id);
    let artifact_dir = context
        .finality_artifact_root
        .join(format!("rpc-finality-{request_index}-{request_component}"));
    let total_start = Instant::now();
    let readiness_wait_ms = wait_for_rpc_finality_required_parent(context, params)?;
    let finality_view = prepare_rpc_finality_view(&context.data_dir, params, &artifact_dir)?;
    require_rpc_finality_local_proposer(context, finality_view.view)?;
    let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
        data_dir: context.data_dir.clone(),
        topology_file: context.finality_topology_file.clone(),
        key_file: context.finality_key_file.clone(),
        proposal_key_file: context.finality_proposal_key_file.clone(),
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: context.finality_quorum_early_full_propagation,
        artifact_dir: artifact_dir.clone(),
        block_height: None,
        view: Some(finality_view.view),
        timeout_certificate_file: finality_view.timeout_certificate_file,
        timeout_ms: context.finality_timeout_ms,
        send_retries: context.finality_send_retries,
        retry_backoff_ms: context.finality_retry_backoff_ms,
        local_apply_before_certified_send: true,
        defer_certified_sends: true,
        required_parent: None,
        max_transactions: 1,
        signed_transfer_file: None,
        signed_transfer_json: None,
        signed_payment_v2_json: None,
        signed_asset_transaction_json: None,
        signed_atomic_swap_transaction_json: None,
        signed_escrow_transaction_json: Some(signed_escrow_transaction_json.to_string()),
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("mempool_submit_signed_escrow_transaction_finality failed: {error}"),
        )
    })?;
    let tx_id = round.submitted_tx_id.clone().ok_or_else(|| {
        (
            "rpc_finality_submit_failed".to_string(),
            "finality submit round did not report submitted_tx_id".to_string(),
        )
    })?;
    let finality = round
        .round
        .local_hot_finality
        .iter()
        .find(|report| report.tx_id == tx_id && report.confirmed && report.receipt.accepted)
        .cloned()
        .ok_or_else(|| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("finality submit round did not emit accepted hot finality for `{tx_id}`"),
            )
        })?;
    let total_ms = monotonic_elapsed_ms(total_start);
    let round_report_file = artifact_dir.join("rpc-finality-round.json");
    let round_json = serde_json::to_string_pretty(&round).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality round report serialization failed: {error}"),
        )
    })?;
    std::fs::write(&round_report_file, round_json).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!(
                "finality round report write `{}` failed: {error}",
                round_report_file.display()
            ),
        )
    })?;
    let result = RpcMempoolSubmitSignedTransferFinalityReport {
        schema: "postfiat-rpc-mempool-submit-signed-escrow-transaction-finality-v1"
            .to_string(),
        tx_id: tx_id.clone(),
        finality,
        round_report_file: round_report_file.display().to_string(),
        artifact_dir: artifact_dir.display().to_string(),
        readiness_wait_ms,
        mempool_submit_ms: round.mempool_submit_ms,
        mempool_batch_ms: round.mempool_batch_ms,
        certified_round_ms: round.round.timings.total_ms,
        total_ms,
        certified_sends_deferred: round.round.certified_sends_deferred,
        round_ok: round.round_ok,
    };
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "mempool_submit_signed_escrow_transaction_finality",
            tx_id,
            "externally signed escrow transaction finalized",
        )],
    )
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality RPC response serialization failed: {error}"),
        )
    })
}

fn validate_rpc_serve_orchard_batch_create_request(request: &RpcRequest) -> Result<(), String> {
    if !is_orchard_batch_create_method(&request.method) {
        return Err(format!(
            "rpc method `{}` is not an Orchard batch-create method",
            request.method
        ));
    }
    let params = request
        .params
        .as_object()
        .ok_or_else(|| "Orchard batch-create params must be an object".to_string())?;
    if params.contains_key("action_file")
        || params.contains_key("deposit_file")
        || params.contains_key("swap_file")
    {
        return Err(
            "remote Orchard batch creation requires inline JSON, not client file paths".to_string(),
        );
    }
    if params.contains_key("batch_file") {
        return Err(
            "remote Orchard batch creation uses server-controlled batch spool paths".to_string(),
        );
    }
    let required_json = match request.method.as_str() {
        "shield_batch_orchard_deposit" => "deposit_json",
        "shield_batch_swap" => "swap_json",
        _ => "action_json",
    };
    if params
        .get(required_json)
        .and_then(serde_json::Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err(format!(
            "remote Orchard batch creation requires nonempty {required_json}"
        ));
    }
    Ok(())
}

fn validate_rpc_serve_request_line(line: &str) -> Result<(), String> {
    let request = line.strip_suffix('\n').unwrap_or(line);
    if request.len() > MAX_RPC_REQUEST_BYTES {
        return Err(format!(
            "rpc request exceeded {MAX_RPC_REQUEST_BYTES} bytes"
        ));
    }
    Ok(())
}

fn run_rpc_request_via_child(
    data_dir: &Path,
    spool_root: &Path,
    request_index: u64,
    request_json: &str,
    child_timeout_ms: u64,
) -> Result<RpcResponse, String> {
    let spool_dir = create_rpc_serve_spool_dir(spool_root, request_index)?;
    let request_file = spool_dir.join("request.json");
    std::fs::write(&request_file, request_json.as_bytes())
        .map_err(|error| {
            let _ = std::fs::remove_dir_all(&spool_dir);
            format!("rpc serve request spool write failed: {error}")
        })?;
    let exe = match resolve_rpc_child_exe() {
        Ok(exe) => exe,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&spool_dir);
            return Err(error);
        }
    };
    let output_result = run_rpc_child_command(
        exe,
        data_dir,
        &request_file,
        Duration::from_millis(child_timeout_ms),
    );
    let cleanup_result = std::fs::remove_dir_all(&spool_dir);
    if let Err(error) = cleanup_result {
        return Err(format!("rpc serve request spool cleanup failed: {error}"));
    }
    let output = output_result?;
    if output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("rpc serve child produced no response: {stderr}"));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("rpc serve child response was not UTF-8: {error}"))?;
    let response: RpcResponse = serde_json::from_str(&stdout)
        .map_err(|error| format!("rpc serve child response parse failed: {error}"))?;
    response
        .validate_protocol()
        .map_err(|error| format!("rpc serve child response protocol failed: {error}"))?;
    Ok(response)
}

fn validate_rpc_serve_bind_host(host: &str) -> Result<(), String> {
    validate_rpc_serve_bind_host_with_override(host, false)
}

fn validate_rpc_serve_bind_host_with_override(
    host: &str,
    legacy_allow_public: bool,
) -> Result<(), String> {
    validate_controlled_transport_bind_host_with_override(host, legacy_allow_public)
        .map_err(|error| format!("rpc serve bind host rejected: {error}"))
}

fn create_rpc_serve_spool_dir(base_dir: &Path, request_index: u64) -> Result<PathBuf, String> {
    let mut last_exists_error = None;
    for attempt in 0..RPC_SERVE_SPOOL_DIR_ATTEMPTS {
        let counter =
            RPC_SERVE_SPOOL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let spool_dir = base_dir.join(format!(
            "postfiat-rpc-serve-{}-{request_index}-{counter}-{nanos}-{attempt}",
            process::id()
        ));
        match create_private_rpc_serve_spool_dir(&spool_dir) {
            Ok(()) => return Ok(spool_dir),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                last_exists_error = Some(error);
            }
            Err(error) => {
                return Err(rpc_serve_storage_error(
                    &format!("request spool dir `{}` create", spool_dir.display()),
                    error,
                ));
            }
        }
    }
    Err(last_exists_error
        .map(|error| format!("rpc serve request spool dir collision exhausted: {error}"))
        .unwrap_or_else(|| "rpc serve request spool dir collision exhausted".to_string()))
}

fn prepare_rpc_serve_spool_root(path: &Path) -> Result<(), String> {
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(format!("rpc serve spool root `{}` must not be a symlink", path.display()));
    }
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(|error| {
            rpc_serve_storage_error(
                &format!("spool root `{}` create", path.display()),
                error,
            )
        })?;
    }
    if !path.is_dir() {
        return Err(format!("rpc serve spool root `{}` is not a directory", path.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|error| format!("rpc serve spool root metadata failed: {error}"))?
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions)
            .map_err(|error| rpc_serve_storage_error("spool root permissions", error))?;
    }
    Ok(())
}

fn probe_rpc_serve_spool_root(path: &Path) -> Result<(), String> {
    let probe_dir = create_rpc_serve_spool_dir(path, 0)?;
    let probe_file = probe_dir.join("storage-probe");
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe_file)
            .map_err(|error| rpc_serve_storage_error("spool probe write", error))?;
        file.write_all(b"postfiat-rpc-spool-probe\n")
            .and_then(|_| file.sync_all())
            .map_err(|error| rpc_serve_storage_error("spool probe sync", error))
    })();
    let cleanup = std::fs::remove_dir_all(&probe_dir)
        .map_err(|error| rpc_serve_storage_error("spool probe cleanup", error));
    result.and(cleanup)
}

fn rpc_serve_storage_error(operation: &str, error: std::io::Error) -> String {
    let class = match error.raw_os_error() {
        Some(28) => "capacity_or_inode_exhausted",
        Some(30) => "read_only_filesystem",
        Some(5) => "io_failure",
        Some(13) => "permission_denied",
        _ => "storage_error",
    };
    format!("rpc serve storage failure [{class}] during {operation}: {error}")
}

fn write_rpc_serve_readiness(
    ready_file: &Path,
    readiness: &RpcServeReadinessReport,
) -> Result<(), String> {
    write_transport_ready_file(ready_file, readiness, "rpc serve")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(ready_file)
            .map_err(|error| format!("rpc serve ready file metadata failed: {error}"))?
            .permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(ready_file, permissions)
            .map_err(|error| format!("rpc serve ready file permissions failed: {error}"))?;
    }
    Ok(())
}

#[cfg(unix)]
fn create_private_rpc_serve_spool_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    std::fs::DirBuilder::new().mode(0o700).create(path)?;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_private_rpc_serve_spool_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir(path)
}

fn run_rpc_child_command(
    exe: PathBuf,
    data_dir: &Path,
    request_file: &Path,
    timeout: Duration,
) -> Result<Output, String> {
    let child_data_dir = std::fs::canonicalize(data_dir).map_err(|error| {
        format!(
            "rpc serve child data dir canonicalization failed for `{}`: {error}",
            data_dir.display()
        )
    })?;
    let child = Command::new(exe)
        .env_clear()
        .current_dir(&child_data_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("rpc")
        .arg("--request-file")
        .arg(request_file)
        .arg("--data-dir")
        .arg(&child_data_dir)
        .spawn()
        .map_err(|error| format!("rpc serve child execution failed: {error}"))?;
    wait_for_child_output_with_timeout(child, timeout)
}

fn rpc_child_isolation_report() -> RpcChildIsolationReport {
    RpcChildIsolationReport {
        process_per_request: true,
        request_file_spooled: true,
        stdin_null: true,
        stdout_stderr_piped: true,
        environment_cleared: true,
        current_dir_canonical_data_dir: true,
        timeout_enforced: true,
    }
}

fn wait_for_child_output_with_timeout(
    mut child: Child,
    timeout: Duration,
) -> Result<Output, String> {
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "rpc serve child stdout was not piped".to_string())?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| "rpc serve child stderr was not piped".to_string())?;
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let start = Instant::now();
    let poll_interval = Duration::from_millis(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = join_rpc_child_pipe_reader(stdout_reader, "stdout")?;
                let stderr = join_rpc_child_pipe_reader(stderr_reader, "stderr")?;
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let status = child.wait().map_err(|error| {
                        format!("rpc serve child timeout cleanup failed: {error}")
                    })?;
                    let _stdout = join_rpc_child_pipe_reader(stdout_reader, "stdout")?;
                    let stderr = join_rpc_child_pipe_reader(stderr_reader, "stderr")?;
                    let stderr_text = String::from_utf8_lossy(&stderr);
                    let detail = if stderr_text.trim().is_empty() {
                        String::new()
                    } else {
                        format!(": {}", stderr_text.trim())
                    };
                    let status_detail = if status.success() {
                        String::new()
                    } else {
                        format!(" with status {status}")
                    };
                    return Err(format!(
                        "rpc serve child timed out after {} ms{status_detail}{detail}",
                        timeout.as_millis()
                    ));
                }
                let remaining = timeout.saturating_sub(start.elapsed());
                std::thread::sleep(std::cmp::min(poll_interval, remaining));
            }
            Err(error) => return Err(format!("rpc serve child wait failed: {error}")),
        }
    }
}

fn join_rpc_child_pipe_reader(
    handle: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
    stream_name: &str,
) -> Result<Vec<u8>, String> {
    handle
        .join()
        .map_err(|_| format!("rpc serve child {stream_name} reader panicked"))?
        .map_err(|error| format!("rpc serve child {stream_name} read failed: {error}"))
}

fn resolve_rpc_child_exe() -> Result<PathBuf, String> {
    let current_exe =
        env::current_exe().map_err(|error| format!("rpc serve exe lookup failed: {error}"))?;
    let arg0 = env::args_os().next().map(PathBuf::from);
    let cwd =
        env::current_dir().map_err(|error| format!("rpc serve cwd lookup failed: {error}"))?;
    Ok(resolve_rpc_child_exe_from(current_exe, arg0, &cwd))
}

fn resolve_rpc_child_exe_from(current_exe: PathBuf, arg0: Option<PathBuf>, cwd: &Path) -> PathBuf {
    if current_exe.is_file() {
        return current_exe;
    }
    let Some(arg0) = arg0 else {
        return current_exe;
    };
    if arg0.is_absolute() {
        return arg0;
    }
    let cwd_candidate = cwd.join(&arg0);
    if cwd_candidate.is_file() || arg0.components().count() > 1 {
        return cwd_candidate;
    }
    arg0
}

#[cfg(test)]
fn rpc_serve_method_allowed(
    method: &str,
    allow_mempool_submit: bool,
    allow_mempool_submit_finality: bool,
    allow_orchard_batch_create: bool,
) -> bool {
    rpc_serve_method_allowed_with_owned_lane(
        method,
        allow_mempool_submit,
        allow_mempool_submit_finality,
        allow_orchard_batch_create,
        true,
    )
}

fn rpc_serve_method_allowed_with_owned_lane(
    method: &str,
    allow_mempool_submit: bool,
    allow_mempool_submit_finality: bool,
    allow_orchard_batch_create: bool,
    owned_lane_enabled: bool,
) -> bool {
    matches!(
        method,
        "status"
            | "server_info"
            | "metrics"
            | "ledger"
            | "account"
            | "account_tx"
            | "account_tx_index_status"
            | "fee"
            | "transfer_fee_quote"
            | "atomic_swap_fee_quote"
            | "asset_fee_quote"
            | "escrow_fee_quote"
            | "nft_fee_quote"
            | "offer_fee_quote"
            | "atomic_settlement_template"
            | "offer_info"
            | "account_offers"
            | "book_offers"
            | "asset_info"
            | "account_lines"
            | "account_assets"
            | "owned_objects"
            | "issuer_assets"
            | "market_ops_status"
            | "vault_bridge_status"
            | "vault_bridge_route"
            | "navcoin_bridge_routes"
            | "navcoin_bridge_packet"
            | "navcoin_bridge_claims"
            | "navcoin_bridge_supply_status"
            | "navcoin_bridge_receipt_replay"
            | "escrow_info"
            | "account_escrows"
            | "nft_info"
            | "account_nfts"
            | "issuer_nfts"
            | "receipts"
            | "tx"
            | "blocks"
            | "validators"
            | "manifests"
            | "batch_archive"
            | "archive_window"
            | "verify_blocks"
            | "verify_state"
            | "verify_bridge"
            | "verify_mempool"
            | "verify_shielded"
            | "orchard_pool_report"
            | "mempool_status"
            | "shield_scan"
            | "shield_disclose"
            | "shield_turnstile"
            | "bridge_status"
            | "fastswap_capabilities"
            | "fastswap_preview"
            | "fastswap_prepare"
            | "fastswap_commit"
            | "fastswap_apply"
            | "fastswap_catch_up"
            | "fastswap_status"
            | "fastswap_effects"
            | "fastswap_votes"
            | "fastswap_new_round_vote"
            | "fastswap_propose_round"
            | "fastswap_precommit"
            | "fastswap_commit_round"
            | "fastswap_cancel_apply"
            | "fastlane_exit"
            | "fastswap_checkpoint_status"
            | "fastswap_objects"
            | "fastswap_policy"
            | "fastlane_asset_control_prepare"
            | "fastlane_asset_control_preview"
            | "fastlane_asset_control_apply"
            | "fastlane_asset_control_catch_up"
            | "owned_recovery_capabilities"
            | "owned_certificate"
            | "owned_recovery_status"
    ) || (allow_mempool_submit
        && is_mempool_submit_signed_method(method)
        && !is_mempool_submit_signed_transfer_finality_method(method))
        || (allow_mempool_submit_finality
            && is_mempool_submit_signed_transfer_finality_method(method))
        || (allow_orchard_batch_create && is_orchard_batch_create_method(method))
        || (owned_lane_enabled
            && matches!(
                method,
                "owned_sign"
                    | "owned_apply"
                    | "owned_unwrap_sign"
                    | "owned_unwrap_apply"
                    | "owned_sign_v3"
                    | "owned_apply_v3"
                    | "owned_unwrap_sign_v3"
                    | "owned_unwrap_apply_v3"
            ))
}

#[cfg(test)]
mod remote_method_policy_tests {
    use super::{rpc_serve_method_allowed, rpc_serve_method_allowed_with_owned_lane};

    #[test]
    fn unsigned_owned_lane_mutations_are_never_remote_methods() {
        for method in ["wrap_owned", "unwrap_owned"] {
            assert!(!rpc_serve_method_allowed(method, false, false, false));
            assert!(!rpc_serve_method_allowed(method, true, true, true));
        }
        assert!(rpc_serve_method_allowed(
            "mempool_submit_fastlane_primary",
            true,
            false,
            false
        ));
    }

    #[test]
    fn owned_lane_is_enabled_by_default_and_has_an_emergency_disable_boundary() {
        for method in [
            "owned_sign",
            "owned_apply",
            "owned_unwrap_sign",
            "owned_unwrap_apply",
            "owned_sign_v3",
            "owned_apply_v3",
            "owned_unwrap_sign_v3",
            "owned_unwrap_apply_v3",
        ] {
            assert!(rpc_serve_method_allowed(method, false, false, false));
            assert!(!rpc_serve_method_allowed_with_owned_lane(
                method, false, false, false, false
            ));
        }
        for method in [
            "owned_recovery_capabilities",
            "owned_certificate",
            "owned_recovery_status",
        ] {
            assert!(rpc_serve_method_allowed_with_owned_lane(
                method, false, false, false, false
            ));
        }
    }
}

#[derive(Debug, Clone)]
struct RpcCatchUpOptions {
    data_dir: PathBuf,
    source_host: String,
    source_rpc_port: u16,
    work_dir: PathBuf,
    max_blocks: usize,
    timeout_ms: u64,
}

#[derive(Debug, Clone)]
enum RpcCatchUpAuditMode {
    Full,
    CertifiedDelta {
        expected_height: u64,
        expected_block_hash: String,
        expected_state_root: String,
    },
}

#[derive(Debug, Clone)]
struct ArchiveWindowBackfillOptions {
    data_dir: PathBuf,
    source_host: String,
    source_rpc_port: u16,
    work_dir: PathBuf,
    from_height: u64,
    to_height: u64,
    archive_uri: Option<String>,
    timeout_ms: u64,
    overwrite: bool,
}

struct RpcCatchUpPreparedBlock {
    block: BlockRecord,
    block_file: PathBuf,
    batch_file: PathBuf,
    certificate_file: PathBuf,
    certificate: BlockCertificateFile,
}

fn archive_window_backfill(
    options: ArchiveWindowBackfillOptions,
) -> Result<serde_json::Value, String> {
    std::fs::create_dir_all(&options.work_dir)
        .map_err(|error| format!("archive window backfill work dir create failed: {error}"))?;
    let local_before = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("archive window backfill local status failed: {error}"))?;
    let rpc_options = RpcCatchUpOptions {
        data_dir: options.data_dir.clone(),
        source_host: options.source_host.clone(),
        source_rpc_port: options.source_rpc_port,
        work_dir: options.work_dir.clone(),
        max_blocks: DEFAULT_RPC_CATCH_UP_MAX_BLOCKS,
        timeout_ms: options.timeout_ms,
    };
    let source_status: StatusReport = rpc_catch_up_call(
        &rpc_options,
        "status",
        serde_json::json!({}),
        "archive-window-backfill-source-status",
    )?;
    validate_rpc_catch_up_domain(&local_before, &source_status)?;

    let mut params = serde_json::json!({
        "from_height": options.from_height,
        "to_height": options.to_height,
    });
    if let Some(archive_uri) = options.archive_uri.as_ref() {
        params["archive_uri"] = serde_json::Value::String(archive_uri.clone());
    }
    let bundle: HistoryArchiveWindowBundle = rpc_catch_up_call(
        &rpc_options,
        "archive_window",
        params,
        "archive-window-backfill-window",
    )?;
    if bundle.schema != "postfiat-history-archive-window-v1" {
        return Err(format!(
            "archive window backfill source returned unsupported bundle schema `{}`",
            bundle.schema
        ));
    }
    if bundle.proof.from_height != options.from_height
        || bundle.proof.to_height != options.to_height
    {
        return Err(format!(
            "archive window backfill source returned range {}-{} instead of {}-{}",
            bundle.proof.from_height,
            bundle.proof.to_height,
            options.from_height,
            options.to_height
        ));
    }

    let bundle_file = options.work_dir.join(format!(
        "archive-window-{}-{}.json",
        options.from_height, options.to_height
    ));
    if bundle_file.exists() && !options.overwrite {
        return Err(format!(
            "archive window backfill bundle file `{}` already exists; use --overwrite",
            bundle_file.display()
        ));
    }
    let bundle_json = serde_json::to_string_pretty(&bundle)
        .map_err(|error| format!("archive window backfill bundle serialization failed: {error}"))?;
    std::fs::write(&bundle_file, bundle_json.as_bytes())
        .map_err(|error| format!("archive window backfill bundle write failed: {error}"))?;

    let import_report = import_history_archive_window(HistoryArchiveWindowImportOptions {
        data_dir: options.data_dir.clone(),
        bundle_file: bundle_file.clone(),
        overwrite: options.overwrite,
    })
    .map_err(|error| format!("archive window backfill import failed: {error}"))?;
    let local_after = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("archive window backfill post-import status failed: {error}"))?;
    let history_after = history_status(HistoryOptions::with_defaults(options.data_dir))
        .map_err(|error| format!("archive window backfill history status failed: {error}"))?;
    let backfill_status = if import_report.imported {
        "backfilled"
    } else {
        "already_available"
    };
    let archive_file = import_report.archive_file.clone();
    let archived_window_count = import_report.archived_window_count;
    let imported = import_report.imported;
    let bundle_hash = bundle.bundle_hash.clone();

    Ok(serde_json::json!({
        "schema": "postfiat-archive-window-backfill-v1",
        "status": backfill_status,
        "source_node": source_status.node_id,
        "local_node": local_before.node_id,
        "chain_id": local_before.chain_id,
        "genesis_hash": local_before.genesis_hash,
        "protocol_version": local_before.protocol_version,
        "from_height": options.from_height,
        "to_height": options.to_height,
        "local_height_before": local_before.block_height,
        "source_height": source_status.block_height,
        "source_block_tip": source_status.block_tip_hash,
        "source_state_root": source_status.state_root,
        "bundle_file": bundle_file.display().to_string(),
        "bundle_hash": bundle_hash,
        "imported": imported,
        "archive_file": archive_file,
        "archived_window_count": archived_window_count,
        "import": import_report,
        "local_height_after": local_after.block_height,
        "local_block_tip_after": local_after.block_tip_hash,
        "local_state_root_after": local_after.state_root,
        "local_history_after": history_after
    }))
}

fn rpc_catch_up(options: RpcCatchUpOptions) -> Result<serde_json::Value, String> {
    rpc_catch_up_with_audit(options, RpcCatchUpAuditMode::Full)
}

fn rpc_catch_up_certified_delta(
    options: RpcCatchUpOptions,
    expected_height: u64,
    expected_block_hash: String,
    expected_state_root: String,
) -> Result<serde_json::Value, String> {
    rpc_catch_up_with_audit(
        options,
        RpcCatchUpAuditMode::CertifiedDelta {
            expected_height,
            expected_block_hash,
            expected_state_root,
        },
    )
}

fn rpc_catch_up_with_audit(
    options: RpcCatchUpOptions,
    audit_mode: RpcCatchUpAuditMode,
) -> Result<serde_json::Value, String> {
    if options.max_blocks == 0 || options.max_blocks > DEFAULT_RPC_CATCH_UP_MAX_BLOCKS {
        return Err(format!(
            "rpc catch-up --max-blocks must be between 1 and {DEFAULT_RPC_CATCH_UP_MAX_BLOCKS}"
        ));
    }
    rpc_catch_up_validate_work_dir(&options.work_dir)?;
    let local_before = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("rpc catch-up local status failed: {error}"))?;
    let source_status: StatusReport = rpc_catch_up_call(
        &options,
        "status",
        serde_json::json!({}),
        "rpc-catch-up-source-status",
    )?;
    validate_rpc_catch_up_domain(&local_before, &source_status)?;

    let local_height = local_before.block_height;
    let source_height = source_status.block_height;
    if source_height <= local_height {
        if matches!(audit_mode, RpcCatchUpAuditMode::CertifiedDelta { .. }) {
            return Err(format!(
                "rpc catch-up certified delta requires a new block: local height {local_height}, source height {source_height}"
            ));
        }
        let verification = rpc_catch_up_verify_completed_state(&options.data_dir)?;
        return Ok(serde_json::json!({
            "schema": "postfiat-rpc-catch-up-v1",
            "status": "up_to_date",
            "source_node": source_status.node_id,
            "local_node": local_before.node_id,
            "local_height_before": local_height,
            "source_height": source_height,
            "applied_count": 0,
            "applied": [],
            "local_height_after": local_height,
            "local_block_tip_after": local_before.block_tip_hash,
            "local_state_root_after": local_before.state_root,
            "audit_mode": "full",
            "full_verification_count": 1,
            "verification": verification,
        }));
    }

    let max_catch_up_height = local_height
        .checked_add(options.max_blocks as u64)
        .ok_or_else(|| "rpc catch-up max height overflow".to_string())?;
    let catch_up_to = match &audit_mode {
        RpcCatchUpAuditMode::Full => std::cmp::min(source_height, max_catch_up_height),
        RpcCatchUpAuditMode::CertifiedDelta {
            expected_height, ..
        } => {
            if *expected_height <= local_height {
                return Err(format!(
                    "rpc catch-up certified delta expected height {expected_height} must be above local height {local_height}"
                ));
            }
            if *expected_height > source_height {
                return Err(format!(
                    "rpc catch-up certified delta expected height {expected_height} exceeds source height {source_height}"
                ));
            }
            if *expected_height > max_catch_up_height {
                return Err(format!(
                    "rpc catch-up certified delta expected height {expected_height} exceeds bounded catch-up height {max_catch_up_height}"
                ));
            }
            *expected_height
        }
    };
    let prepared =
        rpc_catch_up_preflight_blocks(&options, &local_before, &source_status, catch_up_to)?;
    let preflight_file = options.work_dir.join("rpc-catch-up-preflight.json");
    let preflight_report = serde_json::json!({
        "schema": "postfiat-rpc-catch-up-preflight-v1",
        "status": "ready",
        "source_node": source_status.node_id,
        "local_node": local_before.node_id,
        "local_height_before": local_height,
        "source_height": source_height,
        "catch_up_to": catch_up_to,
        "prepared_count": prepared.len(),
        "work_dir": options.work_dir.display().to_string(),
        "prepared": prepared.iter().map(|prepared| {
            serde_json::json!({
                "height": prepared.block.header.height,
                "batch_kind": prepared.block.header.batch_kind,
                "batch_id": prepared.block.header.batch_id,
                "block_hash": prepared.block.header.block_hash,
                "state_root": prepared.block.header.state_root,
                "certificate_id": prepared.certificate.certificate_id,
                "certificate_vote_count": prepared.certificate.certificate.votes.len(),
                "certificate_quorum": prepared.certificate.certificate.quorum,
                "block_file": prepared.block_file.display().to_string(),
                "batch_file": prepared.batch_file.display().to_string(),
                "certificate_file": prepared.certificate_file.display().to_string(),
            })
        }).collect::<Vec<_>>(),
    });
    write_json_file(&preflight_file, &preflight_report)
        .map_err(|error| format!("rpc catch-up preflight report write failed: {error}"))?;

    let mut expected_parent = local_before.block_tip_hash.clone();
    let mut applied = Vec::with_capacity(prepared.len());
    for prepared_block in prepared {
        let block = prepared_block.block;
        if block.header.parent_hash != expected_parent {
            return Err(format!(
                "rpc catch-up block {} parent hash mismatch",
                block.header.height
            ));
        }
        let receipts = apply_transport_batch(
            &options.data_dir,
            &block.header.batch_kind,
            prepared_block.batch_file,
            Some(prepared_block.certificate_file),
            Some(prepared_block.block_file),
        )?;
        let status_after = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })
        .map_err(|error| format!("rpc catch-up post-apply status failed: {error}"))?;
        if status_after.block_height != block.header.height {
            return Err(format!(
                "rpc catch-up local height {} does not match applied block {}",
                status_after.block_height, block.header.height
            ));
        }
        if status_after.block_tip_hash != block.header.block_hash {
            return Err(format!(
                "rpc catch-up local tip does not match block {} hash",
                block.header.height
            ));
        }
        if status_after.state_root != block.header.state_root {
            return Err(format!(
                "rpc catch-up local state root does not match block {}",
                block.header.height
            ));
        }
        let accepted_receipts = receipts.iter().filter(|receipt| receipt.accepted).count();
        let rejected_receipts = receipts.len().saturating_sub(accepted_receipts);
        applied.push(serde_json::json!({
            "height": block.header.height,
            "batch_kind": block.header.batch_kind,
            "batch_id": block.header.batch_id,
            "block_hash": block.header.block_hash,
            "state_root": block.header.state_root,
            "certificate_id": prepared_block.certificate.certificate_id,
            "certificate_vote_count": prepared_block.certificate.certificate.votes.len(),
            "certificate_quorum": prepared_block.certificate.certificate.quorum,
            "receipt_count": receipts.len(),
            "accepted_receipts": accepted_receipts,
            "rejected_receipts": rejected_receipts,
        }));
        expected_parent = status_after.block_tip_hash;
    }

    let local_after = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("rpc catch-up final status failed: {error}"))?;
    let (audit_mode_label, full_verification_count, verification) = match &audit_mode {
        RpcCatchUpAuditMode::Full => (
            "full",
            1usize,
            rpc_catch_up_verify_completed_state(&options.data_dir)?,
        ),
        RpcCatchUpAuditMode::CertifiedDelta {
            expected_height,
            expected_block_hash,
            expected_state_root,
        } => {
            if local_after.block_height != *expected_height {
                return Err(format!(
                    "rpc catch-up certified delta local height {} does not match expected height {expected_height}",
                    local_after.block_height
                ));
            }
            if local_after.block_tip_hash != *expected_block_hash {
                return Err(
                    "rpc catch-up certified delta local tip does not match expected block hash"
                        .to_string(),
                );
            }
            if local_after.state_root != *expected_state_root {
                return Err(
                    "rpc catch-up certified delta local state root does not match expected state root"
                        .to_string(),
                );
            }
            (
                "certified_delta",
                0usize,
                serde_json::json!({
                    "schema": "postfiat-rpc-catch-up-deferred-audit-v1",
                    "status": "deferred",
                    "terminal_full_audit_required": true,
                    "certified_continuity_verified": true,
                    "expected_height": expected_height,
                    "expected_block_hash": expected_block_hash,
                    "expected_state_root": expected_state_root,
                }),
            )
        }
    };
    Ok(serde_json::json!({
        "schema": "postfiat-rpc-catch-up-v1",
        "status": "caught_up",
        "source_node": source_status.node_id,
        "local_node": local_before.node_id,
        "local_height_before": local_height,
        "source_height": source_height,
        "catch_up_to": catch_up_to,
        "applied_count": applied.len(),
        "applied": applied,
        "local_height_after": local_after.block_height,
        "local_block_tip_after": local_after.block_tip_hash,
        "local_state_root_after": local_after.state_root,
        "source_block_tip": source_status.block_tip_hash,
        "source_state_root": source_status.state_root,
        "preflight_file": preflight_file.display().to_string(),
        "more_available": source_height > catch_up_to,
        "audit_mode": audit_mode_label,
        "full_verification_count": full_verification_count,
        "verification": verification,
    }))
}

fn rpc_catch_up_verify_completed_state(
    data_dir: &Path,
) -> Result<serde_json::Value, String> {
    let verification = verify_state(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .map_err(|error| format!("rpc catch-up final state verification failed: {error}"))?;
    if !verification.verified {
        return Err("rpc catch-up final state verification did not pass".to_string());
    }
    serde_json::to_value(verification)
        .map_err(|error| format!("rpc catch-up final verification serialization failed: {error}"))
}

fn rpc_catch_up_validate_work_dir(work_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(work_dir)
        .map_err(|error| format!("rpc catch-up work dir create failed: {error}"))?;
    let probe = work_dir.join(".rpc-catch-up-preflight-write-check");
    std::fs::write(&probe, b"postfiat-rpc-catch-up-preflight\n")
        .map_err(|error| format!("rpc catch-up work dir write check failed: {error}"))?;
    std::fs::remove_file(&probe)
        .map_err(|error| format!("rpc catch-up work dir write check cleanup failed: {error}"))
}

fn rpc_catch_up_preflight_blocks(
    options: &RpcCatchUpOptions,
    local_before: &StatusReport,
    source_status: &StatusReport,
    catch_up_to: u64,
) -> Result<Vec<RpcCatchUpPreparedBlock>, String> {
    let local_height = local_before.block_height;
    let missing_count = (catch_up_to - local_height) as usize;
    let blocks: Vec<BlockRecord> = rpc_catch_up_call(
        options,
        "blocks",
        serde_json::json!({
            "from_height": local_height + 1,
            "limit": missing_count,
        }),
        "rpc-catch-up-blocks",
    )?;
    let mut blocks_by_height = std::collections::BTreeMap::<u64, BlockRecord>::new();
    for block in blocks {
        blocks_by_height.insert(block.header.height, block);
    }

    let mut expected_parent = local_before.block_tip_hash.clone();
    let mut prepared = Vec::with_capacity(missing_count);
    for height in (local_height + 1)..=catch_up_to {
        let block = blocks_by_height.remove(&height).ok_or_else(|| {
            format!("rpc catch-up source did not return block height {height}")
        })?;
        if block.header.parent_hash != expected_parent {
            return Err(format!(
                "rpc catch-up preflight block {} parent hash mismatch",
                block.header.height
            ));
        }
        let archives: Vec<BatchArchiveEntry> = rpc_catch_up_call(
            options,
            "batch_archive",
            serde_json::json!({
                "batch_kind": block.header.batch_kind,
                "batch_id": block.header.batch_id,
                "limit": 1,
            }),
            &format!("rpc-catch-up-archive-{height}"),
        )?;
        if archives.len() != 1 {
            return Err(format!(
                "rpc catch-up preflight expected one archive entry for block {height}, got {}",
                archives.len()
            ));
        }
        let archive = archives.into_iter().next().expect("archive length checked");
        if archive.batch_kind != block.header.batch_kind
            || archive.batch_id != block.header.batch_id
        {
            return Err(format!(
                "rpc catch-up preflight archive entry does not match block {} batch coordinates",
                block.header.height
            ));
        }
        let expected_payload_hash = rpc_catch_up_archive_payload_hash(
            source_status,
            &archive.batch_kind,
            &archive.batch_id,
            &archive.payload_json,
        )?;
        if archive.payload_hash != expected_payload_hash {
            return Err(format!(
                "rpc catch-up preflight archive payload hash mismatch for block {}",
                block.header.height
            ));
        }

        let height_dir = options.work_dir.join(format!("height-{height}"));
        std::fs::create_dir_all(&height_dir)
            .map_err(|error| format!("rpc catch-up height dir create failed: {error}"))?;
        let block_file = height_dir.join("block.json");
        let batch_file = height_dir.join("batch.json");
        let certificate_file = height_dir.join("block-certificate.json");
        let block_json = serde_json::to_string_pretty(&block)
            .map_err(|error| format!("rpc catch-up block serialization failed: {error}"))?;
        std::fs::write(&block_file, block_json.as_bytes())
            .map_err(|error| format!("rpc catch-up block write failed: {error}"))?;
        std::fs::write(&batch_file, archive.payload_json.as_bytes())
            .map_err(|error| format!("rpc catch-up batch write failed: {error}"))?;

        let certificate = postfiat_node::reconstruct_block_certificate_from_archive(
            BlockCertificateFromArchiveOptions {
                data_dir: options.data_dir.clone(),
                block_file: block_file.clone(),
                batch_file: batch_file.clone(),
                certificate_file: certificate_file.clone(),
            },
        )
        .map_err(|error| {
            format!(
                "rpc catch-up preflight certificate reconstruction failed for block {}: {error}",
                block.header.height
            )
        })?;
        expected_parent = block.header.block_hash.clone();
        prepared.push(RpcCatchUpPreparedBlock {
            block,
            block_file,
            batch_file,
            certificate_file,
            certificate,
        });
    }
    Ok(prepared)
}

fn rpc_catch_up_archive_payload_hash(
    status: &StatusReport,
    batch_kind: &str,
    batch_id: &str,
    payload_json: &str,
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        status.chain_id.as_str(),
        status.genesis_hash.as_str(),
        status.protocol_version,
        batch_kind,
        batch_id,
        payload_json,
    ))
    .map_err(|error| format!("rpc catch-up archive payload hash encode failed: {error}"))?;
    Ok(hash_hex("postfiat.batch_archive_payload.v1", &encoded))
}

fn rpc_catch_up_call<T: DeserializeOwned>(
    options: &RpcCatchUpOptions,
    method: &str,
    params: serde_json::Value,
    id: &str,
) -> Result<T, String> {
    let request = RpcRequest::new(id, method, params);
    let mut stream = TcpStream::connect((options.source_host.as_str(), options.source_rpc_port))
        .map_err(|error| {
            format!(
                "rpc catch-up connect to {}:{} failed: {error}",
                options.source_host, options.source_rpc_port
            )
        })?;
    set_stream_timeout(&stream, options.timeout_ms)?;
    write_json_line(&mut stream, &request)?;
    let line = read_transport_line(&stream, "rpc catch-up response read")?;
    let response: RpcResponse = serde_json::from_str(&line)
        .map_err(|error| format!("rpc catch-up response parse failed: {error}"))?;
    response
        .validate_protocol()
        .map_err(|error| format!("rpc catch-up response protocol failed: {error}"))?;
    if response.id != id {
        return Err(format!(
            "rpc catch-up response id `{}` did not match `{id}`",
            response.id
        ));
    }
    response
        .result_as::<T>()
        .map_err(|error| format!("rpc catch-up {method} failed: {error}"))
}

fn validate_rpc_catch_up_domain(local: &StatusReport, source: &StatusReport) -> Result<(), String> {
    if local.chain_id != source.chain_id
        || local.genesis_hash != source.genesis_hash
        || local.protocol_version != source.protocol_version
    {
        return Err("rpc catch-up source chain domain does not match local validator".to_string());
    }
    if local.validator_count != source.validator_count {
        return Err(
            "rpc catch-up source validator count does not match local validator".to_string(),
        );
    }
    Ok(())
}

fn rpc_value_contains_key_material(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, value)| {
            matches!(
                key.as_str(),
                "private_key_hex"
                    | "public_key_hex"
                    | "master_seed_hex"
                    | "spending_key_hex"
                    | "full_viewing_key_hex"
                    | "rseed"
            ) || rpc_value_contains_key_material(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(rpc_value_contains_key_material),
        _ => false,
    }
}

fn rpc_serve_error_response(id: &str, code: &str, message: &str) -> RpcResponse {
    let public_message = rpc_serve_public_error_message(code, message);
    error_response(
        id,
        code,
        public_message,
        vec![RpcEvent::new(
            "rpc_serve",
            code,
            "remote rpc request rejected",
        )],
    )
}

fn rpc_serve_public_error_message<'a>(code: &str, message: &'a str) -> &'a str {
    match code {
        "rpc_internal" => "internal RPC processing failed",
        "rpc_server_error" => "RPC worker failed",
        "rpc_child_timeout" => "RPC worker timed out",
        "rpc_read_error" => "RPC request read failed",
        "rpc_status_failed" => "status query failed",
        "rpc_mempool_status_failed" => "mempool status query failed",
        "fastswap_unavailable" => "FastSwap service unavailable",
        _ if rpc_error_message_contains_internal_path(message) => "request failed",
        _ => message,
    }
}

fn rpc_error_message_contains_internal_path(message: &str) -> bool {
    message.starts_with('/')
        || message.contains(" `/")
        || message.contains(" '/")
        || message.contains(" \"/")
        || message.contains(" /home/")
        || message.contains(" /var/")
        || message.contains(" /tmp/")
        || message.as_bytes().windows(3).any(|window| {
            window[0].is_ascii_alphabetic() && window[1] == b':' && window[2] == b'\\'
        })
}

fn rpc_serve_child_error_response(id: &str, error: &str) -> RpcResponse {
    let code = if error.contains("rpc serve child timed out") {
        "rpc_child_timeout"
    } else {
        "rpc_server_error"
    };
    rpc_serve_error_response(id, code, error)
}

#[allow(clippy::too_many_arguments)]
fn rpc_serve_event(
    node_id: &str,
    request_index: u64,
    peer_addr: &str,
    id: String,
    method: impl Into<String>,
    mempool_submit_counts: Option<RpcServeMempoolSubmitCounters>,
    orchard_batch_create_counts: Option<RpcServeMempoolSubmitCounters>,
    response: &RpcResponse,
) -> RpcServeEventRecord {
    let method = method.into();
    let error_class = rpc_serve_error_class(&method, response);
    RpcServeEventRecord {
        schema: "postfiat-rpc-serve-event-v1".to_string(),
        node_id: node_id.to_string(),
        request_index,
        peer_addr: peer_addr.to_string(),
        id,
        method,
        ok: response.ok,
        handler_mode: None,
        handler_ms: None,
        mempool_submit_peer_count: mempool_submit_counts.map(|counts| counts.peer_count),
        mempool_submit_total_count: mempool_submit_counts.map(|counts| counts.total_count),
        orchard_batch_create_peer_count: orchard_batch_create_counts
            .map(|counts| counts.peer_count),
        orchard_batch_create_total_count: orchard_batch_create_counts
            .map(|counts| counts.total_count),
        orchard_batch_create_active_count: orchard_batch_create_counts
            .map(|counts| counts.active_count),
        error_code: response.error.as_ref().map(|error| error.code.clone()),
        error_class,
    }
}

fn rpc_serve_error_class_count(requests: &[RpcServeEventRecord], error_class: &str) -> u64 {
    requests
        .iter()
        .filter(|request| request.error_class.as_deref() == Some(error_class))
        .count() as u64
}

fn rpc_serve_error_class(method: &str, response: &RpcResponse) -> Option<String> {
    if response.ok {
        return None;
    }
    let error = response.error.as_ref()?;
    if error.code == "rpc_method_not_allowed" {
        return Some("method_not_allowed".to_string());
    }
    if error.code == "rpc_mempool_submit_rate_limited" {
        return Some("mempool_submit_rate_limited".to_string());
    }
    if error.code == "rpc_mempool_submit_global_rate_limited" {
        return Some("mempool_submit_global_rate_limited".to_string());
    }
    if error.code == "rpc_orchard_batch_create_rate_limited" {
        return Some("orchard_batch_create_rate_limited".to_string());
    }
    if error.code == "rpc_orchard_batch_create_global_rate_limited" {
        return Some("orchard_batch_create_global_rate_limited".to_string());
    }
    if error.code == "rpc_orchard_batch_create_concurrency_limited" {
        return Some("orchard_batch_create_concurrency_limited".to_string());
    }
    if error.code == "rpc_finality_submit_busy" {
        return Some("finality_submit_busy".to_string());
    }
    if error.code == "rpc_finality_submit_lock_poisoned" {
        return Some("finality_submit_lock_poisoned".to_string());
    }
    if error.code == "rpc_mempool_mutation_busy" {
        return Some("mempool_mutation_busy".to_string());
    }
    if error.code == "rpc_mempool_mutation_lock_poisoned" {
        return Some("mempool_mutation_lock_poisoned".to_string());
    }
    if error.code == "rpc_finality_parent_stale" {
        return Some("finality_parent_stale".to_string());
    }
    if error.code == "rpc_finality_parent_not_ready" {
        return Some("finality_parent_not_ready".to_string());
    }
    if error.code == "rpc_finality_parent_wait_failed" {
        return Some("finality_parent_wait_failed".to_string());
    }
    if error.code == "rpc_finality_wrong_proposer" {
        return Some("finality_wrong_proposer".to_string());
    }
    if error.code == "rpc_finality_submit_failed" {
        return Some("finality_submit_failed".to_string());
    }
    if error.code == "rpc_orchard_batch_create_not_public_safe" {
        return Some("orchard_batch_create_not_public_safe".to_string());
    }
    if error.code == "rpc_child_timeout" {
        return Some("rpc_child_timeout".to_string());
    }
    if error.code == "rpc_request_too_large" {
        return Some("request_too_large".to_string());
    }
    if error.code == "rpc_parse_error" {
        return Some("parse_error".to_string());
    }
    if error.code == "rpc_protocol_error" {
        return Some("protocol_error".to_string());
    }
    if is_mempool_submit_signed_method(method) {
        if error.message.contains("bad_signature") {
            return Some("invalid_signature".to_string());
        }
        if error.message.contains("already pending") {
            return Some("duplicate_transaction".to_string());
        }
        return if method == "mempool_submit_signed_payment_v2" {
            Some("invalid_signed_payment_v2".to_string())
        } else if matches!(
            method,
            "mempool_submit_signed_asset_transaction"
                | "mempool_submit_signed_asset_transaction_finality"
        ) {
            Some("invalid_signed_asset_transaction".to_string())
        } else if matches!(
            method,
            "mempool_submit_signed_atomic_swap_transaction"
                | "mempool_submit_signed_atomic_swap_transaction_finality"
        ) {
            Some("invalid_signed_atomic_swap_transaction".to_string())
        } else if matches!(
            method,
            "mempool_submit_signed_escrow_transaction"
                | "mempool_submit_signed_escrow_transaction_finality"
        ) {
            Some("invalid_signed_escrow_transaction".to_string())
        } else if method == "mempool_submit_signed_nft_transaction" {
            Some("invalid_signed_nft_transaction".to_string())
        } else {
            Some("invalid_signed_transfer".to_string())
        };
    }
    Some(error.code.clone())
}
