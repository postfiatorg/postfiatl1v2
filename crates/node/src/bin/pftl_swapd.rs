use postfiat_crypto_provider::{bytes_to_hex, hash_hex};
use postfiat_node::{
    authorize_and_journal_pftl_swap_intent, build_pftl_swap_quote,
    capture_pftl_swap_state_identity, create_asset_orchard_ingress,
    create_asset_orchard_ingress_batch, create_asset_orchard_private_egress_batch,
    create_shielded_atomic_batch, find_pftl_swap_intent_replay, find_pftl_swap_quote,
    load_pftl_swap_journal, load_pftl_swap_quote_store, record_pftl_swap_stage_timings,
    recover_pftl_swap_journal, revalidate_pftl_swap_quote_for_execution, simulate_shielded_batch,
    store_pftl_swap_quote, transition_pftl_swap_journal_entry, AssetOrchardIngressBatchOptions,
    AssetOrchardIngressCreateOptions, AssetOrchardPrivateEgressBatchOptions, PftlSwapDirection,
    PftlSwapJournalEntry, PftlSwapJournalState, PftlSwapOutputMode, PftlSwapQuoteOptions,
    PftlSwapQuoteRequestV1, PftlSwapQuoteV1, ShieldedAtomicBatchOptions,
    ShieldedBatchSimulateOptions, SignedPftlSwapIntentV1,
};
use postfiat_storage::NodeStore;
use postfiat_types::{ShieldedAction, ShieldedActionBatch};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFAULT_BIND: &str = "127.0.0.1:8798";
const DEFAULT_MAX_BODY_BYTES: usize = 1 << 20;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MAX_CONNECTIONS: usize = 16;
const DEFAULT_MAX_SWAPS_PER_PRINCIPAL_PER_MINUTE: u64 = 12;
const DEFAULT_MAXIMUM_FEE_ATOMS: u64 = 100;
const MAX_RATE_LIMIT_PRINCIPALS: usize = 1_024;
const PRIVATE_NOTE_INDEX_SCHEMA_V1: &str = "postfiat.pftl_swap.private_note_index.v1";
const MAX_PRIVATE_NOTE_INDEX_ENTRIES: usize = 4_096;
const MAX_PRIVATE_STATE_FILE_BYTES: usize = 16 << 20;
const MAX_READY_FILE_BYTES: usize = 1 << 20;
static DAEMON_SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
struct Config {
    data_dir: PathBuf,
    journal_file: PathBuf,
    quote_store_file: PathBuf,
    private_dir: PathBuf,
    batch_dir: PathBuf,
    processed_batch_dir: PathBuf,
    round_driver_ready_file: PathBuf,
    round_driver_max_age: Duration,
    transparent_key_file: PathBuf,
    note_index_file: PathBuf,
    route_id: String,
    controlled_wallet_id: String,
    bind: SocketAddr,
    asset_service_address: SocketAddr,
    asset_service_vault_dir: PathBuf,
    quote_ttl_blocks: u64,
    maximum_fee_atoms: u64,
    issue_ethereum_recipient: String,
    egress_policy_id: String,
    readiness_amount_atoms: u64,
    max_body_bytes: usize,
    max_connections: usize,
    max_swaps_per_principal_per_minute: u64,
    max_requests: Option<u64>,
    request_timeout: Duration,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    target: String,
    body: Vec<u8>,
}

#[derive(Debug)]
struct PreparedSwap {
    batch: ShieldedActionBatch,
    output_note_refs: Vec<String>,
    stage_timings_ns: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PrivateNoteIndexV1 {
    schema: String,
    notes: BTreeMap<String, PrivateNoteRecordV1>,
}

impl Default for PrivateNoteIndexV1 {
    fn default() -> Self {
        Self {
            schema: PRIVATE_NOTE_INDEX_SCHEMA_V1.to_string(),
            notes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PrivateNoteRecordV1 {
    path: PathBuf,
    state: PrivateNoteStateV1,
    swap_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PrivateNoteStateV1 {
    Pending,
    Spendable,
    Egressed,
    Spent,
    Discarded,
}

#[derive(Debug, Clone, Deserialize)]
struct CertifiedRoundDriverReadyV2 {
    schema: String,
    node_id: String,
    batch_kind: String,
    batch_dir: PathBuf,
    processed_dir: PathBuf,
    start_height: u64,
    max_rounds: usize,
    processed_round_count: usize,
    idle_timeout_ms: u64,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    persistent_vote_streams: bool,
    heartbeat_unix_ms: u64,
    local_state: CertifiedRoundDriverLocalState,
    authenticated_peer_count: usize,
    required_remote_peer_count: usize,
    authenticated_quorum: bool,
    shielded_verifier_prewarm: CertifiedRoundDriverVerifierReadiness,
}

#[derive(Debug, Clone, Deserialize)]
struct CertifiedRoundDriverLocalState {
    node_id: String,
    state_root: String,
    block_height: u64,
    block_tip_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CertifiedRoundDriverVerifierReadiness {
    requested: bool,
    asset_orchard_swap_verifier_warm: bool,
    asset_orchard_private_egress_verifier_warm: bool,
}

#[derive(Debug)]
struct RuntimeState {
    config: Config,
    active_connections: AtomicUsize,
    swap_active: AtomicBool,
    private_state_lock: Mutex<()>,
    note_index_lock: Mutex<()>,
    principal_rates: Mutex<BTreeMap<String, PrincipalRateWindow>>,
}

#[derive(Debug, Clone)]
struct PrincipalRateWindow {
    started: Instant,
    requests: u64,
}

struct ConnectionPermit {
    state: Arc<RuntimeState>,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.state.active_connections.fetch_sub(1, Ordering::AcqRel);
    }
}

struct SwapPermit<'a> {
    active: &'a AtomicBool,
}

impl Drop for SwapPermit<'_> {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("pftl-swapd failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    install_signal_handlers()?;
    let config = parse_config()?;
    prepare_private_dir(&config.private_dir)?;
    prepare_private_dir(&config.batch_dir)?;
    prepare_private_dir(&config.processed_batch_dir)?;
    validate_private_file(
        &config.transparent_key_file,
        "controlled transparent wallet key",
    )?;
    if !config.note_index_file.exists() {
        write_private_json(&config.note_index_file, &PrivateNoteIndexV1::default())?;
    }
    load_private_note_index(&config)?;
    recover_pftl_swap_journal(&config.journal_file)?;
    let state = Arc::new(RuntimeState {
        config,
        active_connections: AtomicUsize::new(0),
        swap_active: AtomicBool::new(false),
        private_state_lock: Mutex::new(()),
        note_index_lock: Mutex::new(()),
        principal_rates: Mutex::new(BTreeMap::new()),
    });
    recover_published_outbox(&state)?;
    let listener = TcpListener::bind(state.config.bind)?;
    listener.set_nonblocking(true)?;
    let local = listener.local_addr()?;
    if !local.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "pftl-swapd must bind to a loopback address",
        ));
    }
    println!(
        "{}",
        serde_json::to_string(&json!({
            "schema": "postfiat.pftl_swap.daemon_start.v1",
            "bind": local.to_string(),
            "local_only": true,
            "route_id": state.config.route_id,
            "controlled_wallet_id": state.config.controlled_wallet_id,
        }))
        .map_err(invalid_data)?
    );

    let mut served = 0_u64;
    loop {
        if DAEMON_SHUTDOWN.load(Ordering::Acquire) {
            break;
        }
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                eprintln!("pftl-swapd accept failed: {error}");
                continue;
            }
        };
        if !stream.peer_addr().is_ok_and(|peer| peer.ip().is_loopback()) {
            write_json_response(
                &mut stream,
                403,
                &json!({"ok": false, "error": "non_loopback_peer"}),
            )?;
            continue;
        }
        let Some(permit) = try_acquire_connection(&state) else {
            write_json_response(
                &mut stream,
                503,
                &json!({"ok": false, "error": "connection_capacity_exhausted"}),
            )?;
            continue;
        };
        let child_state = Arc::clone(&state);
        std::thread::Builder::new()
            .name(format!("pftl-swapd-request-{}", served.saturating_add(1)))
            .spawn(move || serve_connection(child_state, stream, permit))?;
        served = served.saturating_add(1);
        if state
            .config
            .max_requests
            .is_some_and(|limit| served >= limit)
        {
            break;
        }
    }
    let deadline = Instant::now() + state.config.request_timeout;
    while state.active_connections.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if state.active_connections.load(Ordering::Acquire) == 0 {
        let _private_state = state
            .private_state_lock
            .lock()
            .map_err(|_| invalid_data("private state lock poisoned"))?;
        recover_pftl_swap_journal(&state.config.journal_file)?;
    }
    Ok(())
}

extern "C" fn pftl_swapd_signal_handler(_: libc::c_int) {
    DAEMON_SHUTDOWN.store(true, Ordering::Release);
}

fn install_signal_handlers() -> io::Result<()> {
    #[cfg(unix)]
    unsafe {
        if libc::signal(
            libc::SIGTERM,
            pftl_swapd_signal_handler as *const () as libc::sighandler_t,
        ) == libc::SIG_ERR
            || libc::signal(
                libc::SIGINT,
                pftl_swapd_signal_handler as *const () as libc::sighandler_t,
            ) == libc::SIG_ERR
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

fn try_acquire_connection(state: &Arc<RuntimeState>) -> Option<ConnectionPermit> {
    let mut current = state.active_connections.load(Ordering::Acquire);
    loop {
        if current >= state.config.max_connections {
            return None;
        }
        match state.active_connections.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                return Some(ConnectionPermit {
                    state: Arc::clone(state),
                });
            }
            Err(observed) => current = observed,
        }
    }
}

fn serve_connection(state: Arc<RuntimeState>, mut stream: TcpStream, _permit: ConnectionPermit) {
    let result = (|| -> io::Result<()> {
        stream.set_read_timeout(Some(state.config.request_timeout))?;
        stream.set_write_timeout(Some(state.config.request_timeout))?;
        match read_http_request(&mut stream, state.config.max_body_bytes) {
            Ok(request) => {
                if let Err(error) = handle_request(&state, &mut stream, request) {
                    let status = match error.kind() {
                        io::ErrorKind::NotFound => 404,
                        io::ErrorKind::PermissionDenied => 403,
                        io::ErrorKind::WouldBlock
                        | io::ErrorKind::StorageFull
                        | io::ErrorKind::AlreadyExists => 409,
                        _ => 400,
                    };
                    write_json_response(
                        &mut stream,
                        status,
                        &json!({
                            "ok": false,
                            "error": error.kind().to_string(),
                            "message": public_error_message(error.kind()),
                        }),
                    )?;
                }
            }
            Err(_) => {
                write_json_response(
                    &mut stream,
                    400,
                    &json!({"ok": false, "error": "bad_request"}),
                )?;
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("pftl-swapd request transport failed: {}", error.kind());
    }
}

fn handle_request(
    state: &RuntimeState,
    stream: &mut TcpStream,
    request: HttpRequest,
) -> io::Result<()> {
    let config = &state.config;
    match (request.method.as_str(), request.target.as_str()) {
        ("GET", "/v1/ready") => {
            let report = readiness_report(state);
            let status = if report["ready"].as_bool() == Some(true) {
                200
            } else {
                503
            };
            write_json_response(stream, status, &report)
        }
        ("POST", "/v1/quote") => {
            if DAEMON_SHUTDOWN.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "pftl-swapd is shutting down",
                ));
            }
            let request: PftlSwapQuoteRequestV1 =
                serde_json::from_slice(&request.body).map_err(invalid_data)?;
            let quote = build_pftl_swap_quote(PftlSwapQuoteOptions {
                data_dir: config.data_dir.clone(),
                route_id: config.route_id.clone(),
                request,
                quote_ttl_blocks: config.quote_ttl_blocks,
                maximum_fee_atoms: config.maximum_fee_atoms,
            })?;
            let _private_state = state
                .private_state_lock
                .lock()
                .map_err(|_| invalid_data("private state lock poisoned"))?;
            store_pftl_swap_quote(&config.quote_store_file, &quote, quote.quote_height)?;
            write_json_response(stream, 200, &json!({"ok": true, "quote": quote}))
        }
        ("POST", "/v1/swap") => {
            if DAEMON_SHUTDOWN.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "pftl-swapd is shutting down",
                ));
            }
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_data)?;
            let signed_intent: SignedPftlSwapIntentV1 =
                serde_json::from_value(body.get("signed_intent").cloned().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "signed_intent is required")
                })?)
                .map_err(invalid_data)?;
            signed_intent.verify()?;
            let existing = {
                let _private_state = state
                    .private_state_lock
                    .lock()
                    .map_err(|_| invalid_data("private state lock poisoned"))?;
                find_pftl_swap_intent_replay(&config.journal_file, &signed_intent)?
            };
            if let Some(existing) = existing.as_ref() {
                let entry = resolve_published_swap(state, &existing.idempotency_key)?;
                if !matches!(
                    entry.state,
                    PftlSwapJournalState::InterruptedPrepublish
                        | PftlSwapJournalState::FailedPrepublish
                ) {
                    let pending = matches!(
                        entry.state,
                        PftlSwapJournalState::Journaled
                            | PftlSwapJournalState::Proving
                            | PftlSwapJournalState::Prepared
                            | PftlSwapJournalState::Published
                    );
                    return write_json_response(
                        stream,
                        if pending { 202 } else { 200 },
                        &json!({
                            "ok": true,
                            "replayed": true,
                            "swap": public_swap_status(&entry),
                            "output_note_refs": committed_output_note_refs(config, &entry)?,
                        }),
                    );
                }
            }
            if readiness_report(state)["ready"].as_bool() != Some(true) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "pftl-swapd is not ready to accept swaps",
                ));
            }
            let _swap_permit = try_acquire_swap(state)?;
            enforce_principal_rate_limit(state, &signed_intent.intent.principal)?;
            let (quote, entry, replayed) = if let Some(existing) = existing {
                (
                    find_pftl_swap_quote(&config.quote_store_file, &signed_intent.intent.quote_id)?,
                    existing,
                    true,
                )
            } else {
                let _private_state = state
                    .private_state_lock
                    .lock()
                    .map_err(|_| invalid_data("private state lock poisoned"))?;
                let quote =
                    find_pftl_swap_quote(&config.quote_store_file, &signed_intent.intent.quote_id)?;
                let execution_height = quote
                    .quote_height
                    .checked_add(1)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "height overflow"))?;
                let (entry, replayed) = authorize_and_journal_pftl_swap_intent(
                    &config.journal_file,
                    &config.data_dir,
                    &quote,
                    &signed_intent,
                    &config.controlled_wallet_id,
                    execution_height,
                )?;
                (quote, entry, replayed)
            };
            let prepared = match execute_prepublication(
                state,
                &quote,
                &signed_intent,
                &entry.idempotency_key,
                &entry.swap_id,
            ) {
                Ok(prepared) => prepared,
                Err(error) => {
                    let durable = {
                        let _private_state = state
                            .private_state_lock
                            .lock()
                            .map_err(|_| invalid_data("private state lock poisoned"))?;
                        load_pftl_swap_journal(&config.journal_file)?
                            .entries
                            .get(&entry.idempotency_key)
                            .cloned()
                            .ok_or_else(|| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "failed swap journal entry missing",
                                )
                            })?
                    };
                    if durable.state == PftlSwapJournalState::Published {
                        return Err(io::Error::new(
                            io::ErrorKind::WouldBlock,
                            "PFTL swap publication outcome requires recovery",
                        ));
                    }
                    let reason = format!("prepublication_{:?}", error.kind()).to_ascii_lowercase();
                    let next = if error.kind() == io::ErrorKind::WouldBlock {
                        PftlSwapJournalState::InterruptedPrepublish
                    } else {
                        PftlSwapJournalState::FailedPrepublish
                    };
                    let _ = transition_swap_journal(
                        state,
                        &entry.idempotency_key,
                        next,
                        None,
                        None,
                        None,
                        Some(reason),
                    );
                    let _ = discard_pending_output_notes(state, &entry.swap_id);
                    return Err(io::Error::new(
                        error.kind(),
                        "PFTL swap failed before publication",
                    ));
                }
            };
            let mut published = {
                let _private_state = state
                    .private_state_lock
                    .lock()
                    .map_err(|_| invalid_data("private state lock poisoned"))?;
                load_pftl_swap_journal(&config.journal_file)?
                    .entries
                    .get(&entry.idempotency_key)
                    .cloned()
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "published swap journal missing")
                    })?
            };
            let deadline = std::time::Instant::now() + config.request_timeout;
            while published.state == PftlSwapJournalState::Published
                && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(100));
                published = resolve_published_swap(state, &entry.idempotency_key)?;
            }
            let committed = published.state == PftlSwapJournalState::Committed;
            write_json_response(
                stream,
                if committed { 200 } else { 202 },
                &json!({
                    "ok": true,
                    "replayed": replayed,
                    "swap": public_swap_status(&published),
                    "output_note_refs": if committed {
                        prepared.output_note_refs
                    } else {
                        Vec::<String>::new()
                    },
                }),
            )
        }
        ("GET", target) if target.starts_with("/v1/status?") => {
            let id = target
                .strip_prefix("/v1/status?id=")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "status id is required")
                })?;
            let journal = load_pftl_swap_journal(&config.journal_file)?;
            let entry = journal
                .entries
                .get(id)
                .or_else(|| journal.entries.values().find(|entry| entry.swap_id == id))
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PFTL swap is unknown"))?
                .clone();
            let entry = resolve_published_swap(state, &entry.idempotency_key)?;
            write_json_response(
                stream,
                200,
                &json!({"ok": true, "swap": public_swap_status(&entry)}),
            )
        }
        _ => write_json_response(stream, 404, &json!({"ok": false, "error": "not_found"})),
    }
}

fn try_acquire_swap(state: &RuntimeState) -> io::Result<SwapPermit<'_>> {
    state
        .swap_active
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "PFTL swap proving capacity is exhausted",
            )
        })?;
    Ok(SwapPermit {
        active: &state.swap_active,
    })
}

fn enforce_principal_rate_limit(state: &RuntimeState, principal: &str) -> io::Result<()> {
    let mut rates = state
        .principal_rates
        .lock()
        .map_err(|_| invalid_data("principal rate-limit lock poisoned"))?;
    let now = Instant::now();
    rates.retain(|_, window| now.duration_since(window.started) < Duration::from_secs(60));
    if rates.len() >= MAX_RATE_LIMIT_PRINCIPALS && !rates.contains_key(principal) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "principal rate-limit table has reached capacity",
        ));
    }
    let window = rates
        .entry(principal.to_string())
        .or_insert(PrincipalRateWindow {
            started: now,
            requests: 0,
        });
    if window.requests >= state.config.max_swaps_per_principal_per_minute {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "principal swap request rate exceeded",
        ));
    }
    window.requests = window.requests.saturating_add(1);
    Ok(())
}

fn public_error_message(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::NotFound => "requested resource was not found",
        io::ErrorKind::PermissionDenied => "request authorization failed",
        io::ErrorKind::WouldBlock => "request cannot be admitted in the current state",
        io::ErrorKind::StorageFull => "durable service capacity is exhausted",
        io::ErrorKind::AlreadyExists => "request conflicts with durable service state",
        _ => "request validation failed",
    }
}

fn public_swap_status(entry: &PftlSwapJournalEntry) -> Value {
    let transition_elapsed = |from: PftlSwapJournalState, to: PftlSwapJournalState| {
        let start = entry
            .transitions
            .iter()
            .find(|transition| transition.state == from)
            .map(|transition| transition.at_monotonic_ns)
            .unwrap_or(0);
        let end = entry
            .transitions
            .iter()
            .rev()
            .find(|transition| transition.state == to)
            .map(|transition| transition.at_monotonic_ns)
            .unwrap_or(0);
        (start > 0 && end >= start).then_some(end - start)
    };
    json!({
        "swap_id": entry.swap_id,
        "idempotency_key": entry.idempotency_key,
        "quote_id": entry.quote_id,
        "direction": entry.direction,
        "input_amount_atoms": entry.input_amount_atoms,
        "minimum_output_amount_atoms": entry.minimum_output_amount_atoms,
        "state": entry.state,
        "batch_hash": entry.batch_hash,
        "committed_height": entry.committed_height,
        "certificate_ref": entry.certificate_ref,
        "timing": {
            "schema": "postfiat.pftl_swap.public_timing.v1",
            "stages": &entry.timing,
            "accepted_to_committed_ns": transition_elapsed(
                PftlSwapJournalState::Journaled,
                PftlSwapJournalState::Committed,
            ),
            "published_to_committed_ns": transition_elapsed(
                PftlSwapJournalState::Published,
                PftlSwapJournalState::Committed,
            ),
        },
    })
}

fn certified_round_batch_file_name(batch_hash: &str) -> String {
    format!("{batch_hash}.batch.json")
}

fn recover_published_outbox(state: &RuntimeState) -> io::Result<()> {
    let config = &state.config;
    let journal = load_pftl_swap_journal(&config.journal_file)?;
    let ordered = NodeStore::new(&config.data_dir).read_ordered_batches()?;
    for entry in journal
        .entries
        .values()
        .filter(|entry| entry.state == PftlSwapJournalState::Published)
    {
        let batch_hash = entry.batch_hash.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "published swap has no batch hash",
            )
        })?;
        if config
            .processed_batch_dir
            .join(certified_round_batch_file_name(batch_hash))
            .exists()
        {
            let _ = resolve_published_swap(state, &entry.idempotency_key)?;
            continue;
        }
        if ordered.contains(batch_hash)
            || config
                .batch_dir
                .join(certified_round_batch_file_name(batch_hash))
                .exists()
        {
            continue;
        }
        let stage = config.batch_dir.join(format!(".{batch_hash}.pending"));
        if stage.exists() {
            fs::rename(
                &stage,
                config
                    .batch_dir
                    .join(certified_round_batch_file_name(batch_hash)),
            )?;
            sync_parent_directory(&stage)?;
            continue;
        }
        let attempts_dir = config.private_dir.join("work").join(&entry.swap_id);
        let mut attempts = fs::read_dir(&attempts_dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        attempts.sort();
        let prepared = attempts
            .into_iter()
            .rev()
            .map(|path| path.join("prepared-batch.json"))
            .find(|path| path.exists())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "published swap has no recoverable prepared batch",
                )
            })?;
        let batch: ShieldedActionBatch =
            serde_json::from_slice(&fs::read(prepared)?).map_err(invalid_data)?;
        if batch.batch_id != *batch_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "recoverable prepared batch identity mismatch",
            ));
        }
        write_private_json(&stage, &batch)?;
        fs::rename(
            &stage,
            config
                .batch_dir
                .join(certified_round_batch_file_name(batch_hash)),
        )?;
        sync_parent_directory(&stage)?;
    }
    Ok(())
}

fn resolve_published_swap(
    state: &RuntimeState,
    idempotency_key: &str,
) -> io::Result<PftlSwapJournalEntry> {
    let config = &state.config;
    let _private_state = state
        .private_state_lock
        .lock()
        .map_err(|_| invalid_data("private state lock poisoned"))?;
    let journal = load_pftl_swap_journal(&config.journal_file)?;
    let entry = journal
        .entries
        .get(idempotency_key)
        .cloned()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "PFTL swap journal entry missing")
        })?;
    if entry.state == PftlSwapJournalState::Committed {
        mark_committed_note_index(state, &entry)?;
        return Ok(entry);
    }
    if entry.state != PftlSwapJournalState::Published {
        return Ok(entry);
    }
    let batch_hash = entry.batch_hash.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "published swap has no batch hash",
        )
    })?;
    let processed = config
        .processed_batch_dir
        .join(certified_round_batch_file_name(batch_hash));
    if !processed.exists() {
        return Ok(entry);
    }
    let processed_batch: ShieldedActionBatch =
        serde_json::from_slice(&fs::read(&processed)?).map_err(invalid_data)?;
    if processed_batch.batch_id != *batch_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "processed swap batch identity mismatch",
        ));
    }
    let store = NodeStore::new(&config.data_dir);
    if !store.read_ordered_batches()?.contains(batch_hash) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "processed swap is not present in local ordered state",
        ));
    }
    let block = store
        .read_blocks()?
        .blocks
        .into_iter()
        .find(|block| block.header.batch_id == *batch_hash)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "processed swap has no local certified block",
            )
        })?;
    let committed = transition_pftl_swap_journal_entry(
        &config.journal_file,
        idempotency_key,
        PftlSwapJournalState::Committed,
        Some(batch_hash.clone()),
        Some(block.header.height),
        Some(block.header.certificate_id),
        None,
    )?;
    mark_committed_note_index(state, &committed)?;
    Ok(committed)
}

fn transition_swap_journal(
    state: &RuntimeState,
    idempotency_key: &str,
    next: PftlSwapJournalState,
    batch_hash: Option<String>,
    committed_height: Option<u64>,
    certificate_ref: Option<String>,
    reason: Option<String>,
) -> io::Result<PftlSwapJournalEntry> {
    let _private_state = state
        .private_state_lock
        .lock()
        .map_err(|_| invalid_data("private state lock poisoned"))?;
    transition_pftl_swap_journal_entry(
        &state.config.journal_file,
        idempotency_key,
        next,
        batch_hash,
        committed_height,
        certificate_ref,
        reason,
    )
}

fn mark_committed_note_index(state: &RuntimeState, entry: &PftlSwapJournalEntry) -> io::Result<()> {
    let config = &state.config;
    let _note_index = state
        .note_index_lock
        .lock()
        .map_err(|_| invalid_data("private note index lock poisoned"))?;
    let mut index = load_private_note_index(config)?;
    let quote = find_pftl_swap_quote(&config.quote_store_file, &entry.quote_id)?;
    for (commitment, record) in &mut index.notes {
        if record.swap_id == entry.swap_id {
            let next = if quote.output_mode == PftlSwapOutputMode::Private {
                PrivateNoteStateV1::Spendable
            } else {
                PrivateNoteStateV1::Egressed
            };
            transition_private_note_state(record, next)?;
        }
        let reference_hash = hash_hex(
            "postfiat.pftl_swap.input_reference.v1",
            commitment.as_bytes(),
        );
        if reference_hash == entry.input_reference_hash {
            transition_private_note_state(record, PrivateNoteStateV1::Spent)?;
        }
    }
    persist_private_note_index(config, &index)
}

fn committed_output_note_refs(
    config: &Config,
    entry: &PftlSwapJournalEntry,
) -> io::Result<Vec<String>> {
    if entry.state != PftlSwapJournalState::Committed {
        return Ok(Vec::new());
    }
    let quote = find_pftl_swap_quote(&config.quote_store_file, &entry.quote_id)?;
    if quote.output_mode != PftlSwapOutputMode::Private {
        return Ok(Vec::new());
    }
    let index = load_private_note_index(config)?;
    let mut references = index
        .notes
        .iter()
        .filter(|(_, record)| {
            record.swap_id == entry.swap_id && record.state == PrivateNoteStateV1::Spendable
        })
        .map(|(commitment, _)| commitment.clone())
        .collect::<Vec<_>>();
    references.sort();
    Ok(references)
}

fn execute_prepublication(
    state: &RuntimeState,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
    idempotency_key: &str,
    swap_id: &str,
) -> io::Result<PreparedSwap> {
    let config = &state.config;
    let prepublication_start = Instant::now();
    let proving = transition_swap_journal(
        state,
        idempotency_key,
        PftlSwapJournalState::Proving,
        None,
        None,
        None,
        None,
    )?;
    let attempt = proving
        .transitions
        .iter()
        .filter(|transition| transition.state == PftlSwapJournalState::Proving)
        .count();
    let request_id = pftl_swap_proving_request_id(swap_id);
    let preflight_start = Instant::now();
    revalidate_pftl_swap_quote_for_execution(&config.data_dir, quote)?;
    let build_identity = capture_pftl_swap_state_identity(&config.data_dir)?;
    let preflight_ns = elapsed_ns(preflight_start);
    let work_dir = config
        .private_dir
        .join("work")
        .join(swap_id)
        .join(format!("attempt-{attempt}"));
    prepare_private_dir(&work_dir)?;
    let mut prepared = prepare_swap(state, quote, signed_intent, swap_id, &request_id, &work_dir)?;
    insert_stage_timing(
        &mut prepared.stage_timings_ns,
        "preflight_state_capture",
        preflight_ns,
    )?;
    if capture_pftl_swap_state_identity(&config.data_dir)? != build_identity {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "PFTL state changed during proof construction; exact batch must be rebuilt",
        ));
    }
    let prepared_batch_file = work_dir.join("prepared-batch.json");
    write_private_json(&prepared_batch_file, &prepared.batch)?;
    let simulation_start = Instant::now();
    let simulation = simulate_shielded_batch(ShieldedBatchSimulateOptions {
        data_dir: config.data_dir.clone(),
        batch_file: prepared_batch_file.clone(),
    })?;
    insert_elapsed_stage(
        &mut prepared.stage_timings_ns,
        "local_atomic_simulation",
        simulation_start,
    )?;
    if !simulation.all_accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "prepared PFTL swap batch failed local atomic simulation",
        ));
    }
    insert_stage_timing(
        &mut prepared.stage_timings_ns,
        "prepublication_total",
        elapsed_ns(prepublication_start),
    )?;
    let attempt_timings = prefix_attempt_timings(attempt, &prepared.stage_timings_ns)?;
    record_swap_timings(state, idempotency_key, &attempt_timings)?;
    transition_swap_journal(
        state,
        idempotency_key,
        PftlSwapJournalState::Prepared,
        Some(prepared.batch.batch_id.clone()),
        None,
        None,
        None,
    )?;

    let final_revalidation_start = Instant::now();
    revalidate_pftl_swap_quote_for_execution(&config.data_dir, quote)?;
    let final_revalidation_ns = elapsed_ns(final_revalidation_start);
    if DAEMON_SHUTDOWN.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "PFTL swap interrupted by daemon shutdown before publication",
        ));
    }
    prepare_private_dir(&config.batch_dir)?;
    let stage = config
        .batch_dir
        .join(format!(".{}.pending", prepared.batch.batch_id));
    let destination = config
        .batch_dir
        .join(certified_round_batch_file_name(&prepared.batch.batch_id));
    let publication_start = Instant::now();
    write_private_json(&stage, &prepared.batch)?;
    transition_swap_journal(
        state,
        idempotency_key,
        PftlSwapJournalState::Published,
        Some(prepared.batch.batch_id.clone()),
        None,
        None,
        None,
    )?;
    if destination.exists() {
        let existing: ShieldedActionBatch =
            serde_json::from_slice(&fs::read(&destination)?).map_err(invalid_data)?;
        if existing != prepared.batch {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "published batch path contains different bytes",
            ));
        }
        fs::remove_file(&stage)?;
    } else {
        fs::rename(&stage, &destination)?;
        sync_parent_directory(&destination)?;
    }
    let mut publication_timings = BTreeMap::new();
    insert_stage_timing(
        &mut publication_timings,
        &format!("attempt_{attempt}_final_policy_revalidation"),
        final_revalidation_ns,
    )?;
    insert_stage_timing(
        &mut publication_timings,
        &format!("attempt_{attempt}_publication_outbox_write"),
        elapsed_ns(publication_start),
    )?;
    record_swap_timings(state, idempotency_key, &publication_timings)?;
    Ok(prepared)
}

fn pftl_swap_proving_request_id(swap_id: &str) -> String {
    let digest = hash_hex(
        "postfiat.pftl_swap.proving_attempt.v1",
        format!("{swap_id}:1").as_bytes(),
    );
    format!("swap-{}", &digest[..59])
}

fn prepare_swap(
    state: &RuntimeState,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
    swap_id: &str,
    request_id: &str,
    work_dir: &Path,
) -> io::Result<PreparedSwap> {
    match quote.direction {
        PftlSwapDirection::Issue => {
            prepare_issue_swap(state, quote, signed_intent, swap_id, request_id, work_dir)
        }
        PftlSwapDirection::Redeem => {
            prepare_redeem_swap(state, quote, signed_intent, swap_id, request_id, work_dir)
        }
    }
}

fn prepare_issue_swap(
    state: &RuntimeState,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
    swap_id: &str,
    request_id: &str,
    work_dir: &Path,
) -> io::Result<PreparedSwap> {
    let config = &state.config;
    let mut stage_timings_ns = BTreeMap::new();
    let ingress_start = Instant::now();
    let first_attempt_dir = work_dir
        .parent()
        .ok_or_else(|| invalid_data("swap work directory has no attempt parent"))?
        .join("attempt-1");
    let ingress_batch_file = first_attempt_dir.join("ingress-batch.json");
    let request_file = first_attempt_dir.join("primary-request.json");
    let cached_response = load_cached_primary_response(config, "issue", request_id)?;
    let (ingress_batch, ingress_output_commitment, issue_request) = if ingress_batch_file.exists() {
        let ingress_batch: ShieldedActionBatch =
            read_private_json(&ingress_batch_file, "durable issue ingress batch")?;
        let ingress_output_commitment = ingress_output_commitment(&ingress_batch)?.to_string();
        let issue_request = request_file
            .exists()
            .then(|| read_private_json::<Value>(&request_file, "durable primary issue request"))
            .transpose()?;
        (ingress_batch, ingress_output_commitment, issue_request)
    } else {
        if work_dir != first_attempt_dir {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "resumed issue has no durable first-attempt ingress",
            ));
        }
        let ingress_file = first_attempt_dir.join("ingress.json");
        let ingress_note_file = first_attempt_dir.join("ingress-note.json");
        let ingress = create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
            data_dir: config.data_dir.clone(),
            key_file: config.transparent_key_file.clone(),
            asset_id: quote.input_asset_id.clone(),
            amount: quote.input_amount_atoms,
            fee: 0,
            note_seed_hex: random_hex(32)?,
            encrypted_output_hex: None,
            ingress_file: ingress_file.clone(),
            note_file: ingress_note_file.clone(),
            overwrite: false,
        })?;
        if ingress.burn_fee > signed_intent.intent.maximum_fee_atoms {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "controlled-wallet ingress fee exceeds signed maximum",
            ));
        }
        let ingress_batch = create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
            data_dir: config.data_dir.clone(),
            ingress_file,
            batch_file: ingress_batch_file,
        })?;
        let issue_request = json!({
            "request_id": request_id,
            "input_note_path": ingress_note_file.display().to_string(),
            "route_id": quote.route_id,
            "subscriber": signed_intent.intent.principal,
            "ethereum_recipient": config.issue_ethereum_recipient,
            "reservation_id": hash_hex(
                "postfiat.pftl_swap.issue_reservation.v1",
                request_id.as_bytes(),
            ),
            "subscription_nonce": random_hex(32)?,
            "mint_amount_atoms": quote.output_amount_atoms,
            "settlement_value_atoms": quote.input_amount_atoms,
            "expires_at_height": quote.expiry_height,
            "pending_output_commitments": [ingress.output_commitment],
        });
        write_private_json(&request_file, &issue_request)?;
        (
            ingress_batch,
            ingress.output_commitment,
            Some(issue_request),
        )
    };
    insert_elapsed_stage(&mut stage_timings_ns, "ingress_construct", ingress_start)?;
    let primary_start = Instant::now();
    let issue_response = match cached_response {
        Some(response) => response,
        None => post_json(
            config.asset_service_address,
            "/asset-orchard/private-primary-issue-actions",
            &issue_request.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "durable primary issue request is unavailable",
                )
            })?,
            config.max_body_bytes,
            config.request_timeout,
        )?,
    };
    insert_elapsed_stage(
        &mut stage_timings_ns,
        "primary_service_request",
        primary_start,
    )?;
    ensure_service_verified(&issue_response, "private-primary issue")?;
    insert_primary_proof_timings(&mut stage_timings_ns, &issue_response)?;
    let issue_batch: ShieldedActionBatch =
        serde_json::from_value(issue_response["batch"].clone()).map_err(invalid_data)?;
    let output_commitment = service_output_commitment(&issue_response)?;
    let output_note_path = service_output_note_path(config, &issue_response)?;
    index_pending_output_note(state, &output_commitment, &output_note_path, swap_id)?;
    let mut source_batches = vec![ingress_batch, issue_batch];
    let output_note_refs = if quote.output_mode == PftlSwapOutputMode::Transparent {
        let egress_start = Instant::now();
        let egress_batch = build_private_egress_batch(
            config,
            quote,
            signed_intent,
            work_dir,
            &output_note_path,
            &output_commitment,
            vec![ingress_output_commitment, output_commitment.clone()],
        )?;
        insert_elapsed_stage(
            &mut stage_timings_ns,
            "optional_egress_service",
            egress_start,
        )?;
        source_batches.push(egress_batch);
        Vec::new()
    } else {
        vec![output_commitment]
    };
    build_atomic_prepared_swap(
        config,
        work_dir,
        source_batches,
        output_note_refs,
        stage_timings_ns,
    )
}

fn load_cached_primary_response(
    config: &Config,
    direction: &str,
    request_id: &str,
) -> io::Result<Option<Value>> {
    if !matches!(direction, "issue" | "redeem") {
        return Err(invalid_data("resident prover cache direction is invalid"));
    }
    let response_file = config
        .asset_service_vault_dir
        .join("private-primary-work")
        .join(direction)
        .join(request_id)
        .join("response.json");
    if !response_file.exists() {
        return Ok(None);
    }
    let response: Value = read_private_json(&response_file, "resident prover cached response")?;
    if response["request_id"].as_str() != Some(request_id) {
        return Err(invalid_data(
            "resident prover cached response request identity mismatch",
        ));
    }
    ensure_service_verified(&response, "cached private-primary response")?;
    Ok(Some(response))
}

fn ingress_output_commitment(batch: &ShieldedActionBatch) -> io::Result<&str> {
    match batch.actions.as_slice() {
        [ShieldedAction::AssetOrchardIngressV2(action)] => {
            validate_lower_hex(
                &action.output_commitment,
                64,
                "durable ingress output commitment",
            )?;
            Ok(&action.output_commitment)
        }
        _ => Err(invalid_data(
            "durable issue ingress batch has an unexpected action shape",
        )),
    }
}

fn prepare_redeem_swap(
    state: &RuntimeState,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
    swap_id: &str,
    request_id: &str,
    work_dir: &Path,
) -> io::Result<PreparedSwap> {
    let config = &state.config;
    let mut stage_timings_ns = BTreeMap::new();
    let input_resolution_start = Instant::now();
    let input_note_path = resolve_indexed_note(state, &signed_intent.intent.input_reference, true)?;
    insert_elapsed_stage(
        &mut stage_timings_ns,
        "input_note_resolution",
        input_resolution_start,
    )?;
    let primary_start = Instant::now();
    let redeem_response = post_json(
        config.asset_service_address,
        "/asset-orchard/private-primary-redeem-actions",
        &json!({
            "request_id": request_id,
            "input_note_path": input_note_path.display().to_string(),
            "route_id": quote.route_id,
            "owner": signed_intent.intent.principal,
            "settlement_recipient": signed_intent.intent.principal,
            "nav_amount_atoms": quote.input_amount_atoms,
            "settlement_output_atoms": quote.output_amount_atoms,
            "expires_at_height": quote.expiry_height,
            "pending_output_commitments": [],
        }),
        config.max_body_bytes,
        config.request_timeout,
    )?;
    insert_elapsed_stage(
        &mut stage_timings_ns,
        "primary_service_request",
        primary_start,
    )?;
    ensure_service_verified(&redeem_response, "private-primary redeem")?;
    insert_primary_proof_timings(&mut stage_timings_ns, &redeem_response)?;
    let redeem_batch: ShieldedActionBatch =
        serde_json::from_value(redeem_response["batch"].clone()).map_err(invalid_data)?;
    let output_commitment = service_output_commitment(&redeem_response)?;
    let output_note_path = service_output_note_path(config, &redeem_response)?;
    index_pending_output_note(state, &output_commitment, &output_note_path, swap_id)?;
    let mut source_batches = vec![redeem_batch];
    let output_note_refs = if quote.output_mode == PftlSwapOutputMode::Transparent {
        let egress_start = Instant::now();
        let egress_batch = build_private_egress_batch(
            config,
            quote,
            signed_intent,
            work_dir,
            &output_note_path,
            &output_commitment,
            vec![output_commitment.clone()],
        )?;
        insert_elapsed_stage(
            &mut stage_timings_ns,
            "optional_egress_service",
            egress_start,
        )?;
        source_batches.push(egress_batch);
        Vec::new()
    } else {
        vec![output_commitment]
    };
    build_atomic_prepared_swap(
        config,
        work_dir,
        source_batches,
        output_note_refs,
        stage_timings_ns,
    )
}

fn build_private_egress_batch(
    config: &Config,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
    work_dir: &Path,
    output_note_path: &Path,
    output_commitment: &str,
    pending_output_commitments: Vec<String>,
) -> io::Result<ShieldedActionBatch> {
    let disclosure_hash = hash_hex(
        "postfiat.pftl_swap.transparent_egress.v1",
        quote.quote_id.as_bytes(),
    );
    let response = post_json(
        config.asset_service_address,
        "/asset-orchard/private-egress-actions",
        &json!({
            "disclosure_ack": true,
            "wallet_address": signed_intent.intent.principal,
            "to": signed_intent.intent.principal,
            "asset_id": quote.output_asset_id,
            "amount_atoms": quote.output_amount_atoms,
            "note_commitment": output_commitment,
            "input_note_path": output_note_path.display().to_string(),
            "policy_id": config.egress_policy_id,
            "disclosure_hash": &disclosure_hash[..64],
            "pending_output_commitments": pending_output_commitments,
        }),
        config.max_body_bytes,
        config.request_timeout,
    )?;
    ensure_service_verified(&response, "private egress")?;
    let egress_file = work_dir.join("private-egress.json");
    write_private_json(&egress_file, &response["egress"])?;
    create_asset_orchard_private_egress_batch(AssetOrchardPrivateEgressBatchOptions {
        data_dir: config.data_dir.clone(),
        egress_file,
        batch_file: work_dir.join("private-egress-batch.json"),
    })
}

fn build_atomic_prepared_swap(
    config: &Config,
    work_dir: &Path,
    source_batches: Vec<ShieldedActionBatch>,
    output_note_refs: Vec<String>,
    mut stage_timings_ns: BTreeMap<String, u64>,
) -> io::Result<PreparedSwap> {
    let assembly_start = Instant::now();
    let mut source_batch_files = Vec::with_capacity(source_batches.len());
    for (index, batch) in source_batches.iter().enumerate() {
        let path = work_dir.join(format!("source-batch-{index}.json"));
        write_private_json(&path, batch)?;
        source_batch_files.push(path);
    }
    let batch = create_shielded_atomic_batch(ShieldedAtomicBatchOptions {
        data_dir: config.data_dir.clone(),
        source_batch_files,
        batch_file: work_dir.join("atomic-batch.json"),
    })?;
    insert_elapsed_stage(&mut stage_timings_ns, "atomic_assembly", assembly_start)?;
    Ok(PreparedSwap {
        batch,
        output_note_refs,
        stage_timings_ns,
    })
}

fn insert_elapsed_stage(
    stages: &mut BTreeMap<String, u64>,
    name: &str,
    started: Instant,
) -> io::Result<()> {
    insert_stage_timing(stages, name, elapsed_ns(started))
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos())
        .unwrap_or(u64::MAX)
        .max(1)
}

fn insert_stage_timing(
    stages: &mut BTreeMap<String, u64>,
    name: &str,
    elapsed_ns: u64,
) -> io::Result<()> {
    if stages.insert(name.to_string(), elapsed_ns.max(1)).is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "duplicate PFTL swap timing stage",
        ));
    }
    Ok(())
}

fn prefix_attempt_timings(
    attempt: usize,
    stages: &BTreeMap<String, u64>,
) -> io::Result<BTreeMap<String, u64>> {
    let mut prefixed = BTreeMap::new();
    for (stage, elapsed_ns) in stages {
        insert_stage_timing(
            &mut prefixed,
            &format!("attempt_{attempt}_{stage}"),
            *elapsed_ns,
        )?;
    }
    Ok(prefixed)
}

fn record_swap_timings(
    state: &RuntimeState,
    idempotency_key: &str,
    stages: &BTreeMap<String, u64>,
) -> io::Result<PftlSwapJournalEntry> {
    let _private_state = state
        .private_state_lock
        .lock()
        .map_err(|_| invalid_data("private state lock poisoned"))?;
    record_pftl_swap_stage_timings(&state.config.journal_file, idempotency_key, stages)
}

fn insert_primary_proof_timings(
    stages: &mut BTreeMap<String, u64>,
    response: &Value,
) -> io::Result<()> {
    let timing = response["verification"]
        .get("proof_timing")
        .and_then(Value::as_object)
        .filter(|timing| {
            timing.get("schema").and_then(Value::as_str)
                == Some("postfiat.asset_orchard.private_primary_proof_timing.v1")
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "resident prover response lacks private-primary proof timing",
            )
        })?;
    for (source, target) in [
        ("witness_preparation_ns", "primary_witness"),
        ("output_validity_proof_action_ns", "primary_output_validity"),
        ("binding_and_outer_circuit_ns", "primary_binding_circuit"),
        ("outer_proving_key_ns", "primary_outer_proving_key"),
        ("outer_proof_generation_ns", "primary_outer_proof"),
        (
            "action_assembly_and_authorization_ns",
            "primary_assembly_authorization",
        ),
        ("total_ns", "primary_proof_dag_total"),
    ] {
        let elapsed_ns = timing.get(source).and_then(Value::as_u64).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "resident prover response has invalid private-primary timing",
            )
        })?;
        insert_stage_timing(stages, target, elapsed_ns)?;
    }
    Ok(())
}

fn ensure_service_verified(response: &Value, label: &str) -> io::Result<()> {
    if response["ok"].as_bool() != Some(true)
        || response["verification"]["verified"].as_bool() != Some(true)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("resident prover did not verify {label}"),
        ));
    }
    Ok(())
}

fn service_output_commitment(response: &Value) -> io::Result<String> {
    response["verification"]["output_commitment"]
        .as_str()
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        .map(str::to_string)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "resident prover response has invalid output commitment",
            )
        })
}

fn service_output_note_path(config: &Config, response: &Value) -> io::Result<PathBuf> {
    let path = response["output_note_path"]
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 4_096)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "resident prover response has invalid private output handle",
            )
        })?;
    canonical_note_path(config, &path, "resident prover output handle")
}

fn index_pending_output_note(
    state: &RuntimeState,
    commitment: &str,
    note_path: &Path,
    swap_id: &str,
) -> io::Result<()> {
    let config = &state.config;
    let _note_index = state
        .note_index_lock
        .lock()
        .map_err(|_| invalid_data("private note index lock poisoned"))?;
    validate_lower_hex(commitment, 64, "private output commitment")?;
    validate_lower_hex(swap_id, 96, "private note swap id")?;
    let mut index = load_private_note_index(config)?;
    let record = PrivateNoteRecordV1 {
        path: note_path.to_path_buf(),
        state: PrivateNoteStateV1::Pending,
        swap_id: swap_id.to_string(),
    };
    match index.notes.get_mut(commitment) {
        Some(existing)
            if existing.path == record.path
                && existing.swap_id == record.swap_id
                && existing.state == PrivateNoteStateV1::Discarded =>
        {
            transition_private_note_state(existing, PrivateNoteStateV1::Pending)?;
        }
        Some(existing) if existing != &record => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "private note commitment is already indexed differently",
            ));
        }
        _ => {}
    }
    if index.notes.len() >= MAX_PRIVATE_NOTE_INDEX_ENTRIES && !index.notes.contains_key(commitment)
    {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "private note index has reached its bounded capacity",
        ));
    }
    index.notes.entry(commitment.to_string()).or_insert(record);
    persist_private_note_index(config, &index)
}

fn discard_pending_output_notes(state: &RuntimeState, swap_id: &str) -> io::Result<()> {
    let config = &state.config;
    let _note_index = state
        .note_index_lock
        .lock()
        .map_err(|_| invalid_data("private note index lock poisoned"))?;
    validate_lower_hex(swap_id, 96, "private note swap id")?;
    let mut index = load_private_note_index(config)?;
    for record in index.notes.values_mut() {
        if record.swap_id == swap_id && record.state == PrivateNoteStateV1::Pending {
            transition_private_note_state(record, PrivateNoteStateV1::Discarded)?;
        }
    }
    persist_private_note_index(config, &index)
}

fn resolve_indexed_note(
    state: &RuntimeState,
    commitment: &str,
    require_spendable: bool,
) -> io::Result<PathBuf> {
    let config = &state.config;
    let _note_index = state
        .note_index_lock
        .lock()
        .map_err(|_| invalid_data("private note index lock poisoned"))?;
    if commitment.len() != 64
        || !commitment
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private input reference must be a 32-byte commitment",
        ));
    }
    let index = load_private_note_index(config)?;
    let record = index.notes.get(commitment).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "private input reference is unknown",
        )
    })?;
    if require_spendable && record.state != PrivateNoteStateV1::Spendable {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "private input reference is not committed and spendable",
        ));
    }
    canonical_note_path(config, &record.path, "private input reference")
}

fn load_private_note_index(config: &Config) -> io::Result<PrivateNoteIndexV1> {
    if !config.note_index_file.exists() {
        return Ok(PrivateNoteIndexV1::default());
    }
    validate_private_file(&config.note_index_file, "private note index")?;
    let bytes = read_bounded_file(
        &config.note_index_file,
        MAX_PRIVATE_STATE_FILE_BYTES,
        "private note index",
    )?;
    let index: PrivateNoteIndexV1 = serde_json::from_slice(&bytes).map_err(invalid_data)?;
    validate_private_note_index(config, &index)?;
    Ok(index)
}

fn persist_private_note_index(config: &Config, index: &PrivateNoteIndexV1) -> io::Result<()> {
    validate_private_note_index(config, index)?;
    write_private_json(&config.note_index_file, index)
}

fn validate_private_note_index(config: &Config, index: &PrivateNoteIndexV1) -> io::Result<()> {
    if index.schema != PRIVATE_NOTE_INDEX_SCHEMA_V1
        || index.notes.len() > MAX_PRIVATE_NOTE_INDEX_ENTRIES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "private note index schema or capacity is invalid",
        ));
    }
    for (commitment, record) in &index.notes {
        validate_lower_hex(commitment, 64, "private note commitment")?;
        validate_lower_hex(&record.swap_id, 96, "private note swap id")?;
        let path_text = record.path.to_string_lossy();
        if !record.path.is_absolute()
            || path_text.is_empty()
            || path_text.len() > 4_096
            || record
                .path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "private note index contains an invalid path",
            ));
        }
        if matches!(
            record.state,
            PrivateNoteStateV1::Pending | PrivateNoteStateV1::Spendable
        ) {
            canonical_note_path(config, &record.path, "active private note index record")?;
        }
    }
    Ok(())
}

fn transition_private_note_state(
    record: &mut PrivateNoteRecordV1,
    next: PrivateNoteStateV1,
) -> io::Result<()> {
    use PrivateNoteStateV1::*;
    if record.state == next {
        return Ok(());
    }
    if !matches!(
        (record.state, next),
        (Pending, Spendable | Egressed | Discarded) | (Spendable, Spent) | (Discarded, Pending)
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid private note index state transition",
        ));
    }
    record.state = next;
    Ok(())
}

fn canonical_note_path(config: &Config, path: &Path, label: &str) -> io::Result<PathBuf> {
    let canonical = path.canonicalize().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("{label} does not resolve to a private note file"),
        )
    })?;
    let vault = config.asset_service_vault_dir.canonicalize()?;
    if !canonical.starts_with(&vault) || !canonical.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} is outside the configured resident-prover vault"),
        ));
    }
    validate_private_file(&canonical, label)?;
    Ok(canonical)
}

fn validate_lower_hex(value: &str, length: usize, label: &str) -> io::Result<()> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} has invalid canonical encoding"),
        ));
    }
    Ok(())
}

fn readiness_report(state: &RuntimeState) -> Value {
    let config = &state.config;
    let journal = load_pftl_swap_journal(&config.journal_file);
    let quotes = load_pftl_swap_quote_store(&config.quote_store_file);
    let issue = build_pftl_swap_quote(PftlSwapQuoteOptions {
        data_dir: config.data_dir.clone(),
        route_id: config.route_id.clone(),
        request: PftlSwapQuoteRequestV1 {
            direction: PftlSwapDirection::Issue,
            nav_amount_atoms: config.readiness_amount_atoms,
            output_mode: PftlSwapOutputMode::Private,
        },
        quote_ttl_blocks: config.quote_ttl_blocks,
        maximum_fee_atoms: config.maximum_fee_atoms,
    });
    let redeem = build_pftl_swap_quote(PftlSwapQuoteOptions {
        data_dir: config.data_dir.clone(),
        route_id: config.route_id.clone(),
        request: PftlSwapQuoteRequestV1 {
            direction: PftlSwapDirection::Redeem,
            nav_amount_atoms: config.readiness_amount_atoms,
            output_mode: PftlSwapOutputMode::Private,
        },
        quote_ttl_blocks: config.quote_ttl_blocks,
        maximum_fee_atoms: config.maximum_fee_atoms,
    });
    let asset_service = get_json(
        config.asset_service_address,
        "/asset-orchard/readiness",
        config.max_body_bytes,
        config.request_timeout,
    );
    let expected_height = issue
        .as_ref()
        .ok()
        .map(|quote| quote.quote_height)
        .or_else(|| redeem.as_ref().ok().map(|quote| quote.quote_height));
    let expected_state_root = issue
        .as_ref()
        .ok()
        .map(|quote| quote.state_root.as_str())
        .or_else(|| redeem.as_ref().ok().map(|quote| quote.state_root.as_str()));
    let expected_block_id = issue
        .as_ref()
        .ok()
        .map(|quote| quote.quote_block_id.as_str())
        .or_else(|| {
            redeem
                .as_ref()
                .ok()
                .map(|quote| quote.quote_block_id.as_str())
        });
    let asset_service_matches = asset_service.as_ref().ok().is_some_and(|report| {
        report["ready"].as_bool() == Some(true)
            && report["mirror"]["height"].as_u64() == expected_height
            && report["mirror"]["state_root"].as_str() == expected_state_root
            && report["capabilities"]["private_primary_proof_timing_schema"].as_str()
                == Some("postfiat.asset_orchard.private_primary_proof_timing.v1")
            && report["capabilities"]["private_primary_proof_schedule"].as_str()
                == Some("output_validity_then_outer_primary")
    });
    let public_asset_service = asset_service
        .as_ref()
        .ok()
        .map(public_asset_service_readiness);
    let round_driver = match (expected_height, expected_state_root, expected_block_id) {
        (Some(height), Some(state_root), Some(block_id)) => {
            certified_round_driver_readiness(config, height, state_root, block_id)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "current PFTL state is unavailable for round-driver readiness",
        )),
    };
    let active_swaps = journal
        .as_ref()
        .ok()
        .map(|journal| {
            journal
                .entries
                .values()
                .filter(|entry| {
                    matches!(
                        entry.state,
                        PftlSwapJournalState::Journaled
                            | PftlSwapJournalState::Proving
                            | PftlSwapJournalState::Prepared
                            | PftlSwapJournalState::Published
                    )
                })
                .count()
        })
        .unwrap_or(usize::MAX);
    let runtime_swap_active = state.swap_active.load(Ordering::Acquire);
    let shutting_down = DAEMON_SHUTDOWN.load(Ordering::Acquire);
    let ready = journal.is_ok()
        && quotes.is_ok()
        && issue.is_ok()
        && redeem.is_ok()
        && active_swaps == 0
        && !runtime_swap_active
        && !shutting_down
        && asset_service_matches
        && round_driver.is_ok();
    json!({
        "ok": true,
        "schema": "postfiat.pftl_swap.readiness.v1",
        "ready": ready,
        "local_only": true,
        "route_id": config.route_id,
        "controlled_wallet_id": config.controlled_wallet_id,
        "checks": {
            "journal": result_status(&journal),
            "active_swaps": {
                "ok": active_swaps == 0 && !runtime_swap_active,
                "durable_count": active_swaps,
                "runtime_active": runtime_swap_active,
                "limit": 1,
            },
            "admission": {
                "ok": !shutting_down,
                "shutting_down": shutting_down,
            },
            "quote_store": result_status(&quotes),
            "issue_quote": result_status(&issue),
            "redeem_quote": result_status(&redeem),
            "asset_service": {
                "ok": asset_service.is_ok(),
                "mirror_matches": asset_service_matches,
                "report": public_asset_service,
            },
            "certified_round_driver": match &round_driver {
                Ok(report) => report.clone(),
                Err(error) => json!({"ok": false, "error": error.kind().to_string()}),
            },
        }
    })
}

fn certified_round_driver_readiness(
    config: &Config,
    expected_height: u64,
    expected_state_root: &str,
    expected_block_id: &str,
) -> io::Result<Value> {
    let bytes = read_bounded_file(
        &config.round_driver_ready_file,
        MAX_READY_FILE_BYTES,
        "certified round-driver ready file",
    )?;
    let report: CertifiedRoundDriverReadyV2 =
        serde_json::from_slice(&bytes).map_err(invalid_data)?;
    let now_ms: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| invalid_data("system clock is before Unix epoch"))?
        .as_millis()
        .try_into()
        .map_err(invalid_data)?;
    if report.heartbeat_unix_ms > now_ms.saturating_add(1_000) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "certified round-driver heartbeat is in the future",
        ));
    }
    let age_ms = now_ms.saturating_sub(report.heartbeat_unix_ms);
    let max_age_ms: u64 = config
        .round_driver_max_age
        .as_millis()
        .try_into()
        .map_err(invalid_data)?;
    let batch_dir = report.batch_dir.canonicalize()?;
    let expected_batch_dir = config.batch_dir.canonicalize()?;
    let processed_dir = report.processed_dir.canonicalize()?;
    let expected_processed_dir = config.processed_batch_dir.canonicalize()?;
    let next_round_height = report
        .start_height
        .checked_add(u64::try_from(report.processed_round_count).map_err(invalid_data)?)
        .ok_or_else(|| invalid_data("certified round-driver height overflow"))?;
    let expected_next_height = expected_height
        .checked_add(1)
        .ok_or_else(|| invalid_data("PFTL height overflow"))?;
    if report.schema != "postfiat-transport-peer-certified-batch-loop-ready-v2"
        || report.node_id != report.local_state.node_id
        || report.batch_kind != "shielded"
        || batch_dir != expected_batch_dir
        || processed_dir != expected_processed_dir
        || report.max_rounds < 100
        || report.processed_round_count >= report.max_rounds
        || next_round_height != expected_next_height
        || report.idle_timeout_ms != 0
        || !report.require_signed_proposal
        || report.allow_peer_failures
        || report.quorum_early_full_propagation
        || report.local_apply_before_certified_send
        || report.defer_certified_sends
        || !report.persistent_vote_streams
        || age_ms > max_age_ms
        || report.local_state.block_height != expected_height
        || report.local_state.state_root != expected_state_root
        || report.local_state.block_tip_hash != expected_block_id
        || report.authenticated_peer_count != 5
        || report.required_remote_peer_count != 5
        || !report.authenticated_quorum
        || !report.shielded_verifier_prewarm.requested
        || !report
            .shielded_verifier_prewarm
            .asset_orchard_swap_verifier_warm
        || !report
            .shielded_verifier_prewarm
            .asset_orchard_private_egress_verifier_warm
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "certified round-driver report does not satisfy fail-closed swap policy",
        ));
    }
    Ok(json!({
        "ok": true,
        "schema": report.schema,
        "node_id": report.node_id,
        "heartbeat_age_ms": age_ms,
        "local_height": report.local_state.block_height,
        "next_round_height": next_round_height,
        "authenticated_peer_count": report.authenticated_peer_count,
        "required_remote_peer_count": report.required_remote_peer_count,
        "persistent_vote_streams": report.persistent_vote_streams,
    }))
}

fn public_asset_service_readiness(report: &Value) -> Value {
    let circuits = report["prover_warm"]["circuits"]
        .as_object()
        .map(|circuits| {
            circuits
                .iter()
                .map(|(name, circuit)| {
                    (
                        name.clone(),
                        json!({
                            "circuit_id": circuit["circuit_id"],
                            "status": circuit["status"],
                            "ready": circuit["ready"],
                            "k": circuit["k"],
                            "params_hash": circuit["params_hash"],
                            "vk_hash": circuit["vk_hash"],
                        }),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
        })
        .unwrap_or_default();
    json!({
        "ready": report["ready"],
        "local_only": report["local_only"],
        "pool_id": report["pool_id"],
        "capabilities": {
            "private_primary_proof_timing_schema":
                report["capabilities"]["private_primary_proof_timing_schema"],
            "private_primary_proof_schedule":
                report["capabilities"]["private_primary_proof_schedule"],
        },
        "mirror": {
            "height": report["mirror"]["height"],
            "state_root": report["mirror"]["state_root"],
        },
        "prover_warm": {
            "ready": report["prover_warm"]["ready"],
            "status": report["prover_warm"]["status"],
            "circuits": circuits,
        },
        "prover_capacity": report["prover_capacity"],
    })
}

fn result_status<T>(result: &io::Result<T>) -> Value {
    match result {
        Ok(_) => json!({"ok": true}),
        Err(error) => json!({"ok": false, "error": error.kind().to_string()}),
    }
}

fn get_json(
    address: SocketAddr,
    path: &str,
    max_body_bytes: usize,
    timeout: Duration,
) -> io::Result<Value> {
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let response = read_http_response(&mut stream, max_body_bytes)?;
    serde_json::from_slice(&response).map_err(invalid_data)
}

fn post_json(
    address: SocketAddr,
    path: &str,
    value: &Value,
    max_body_bytes: usize,
    timeout: Duration,
) -> io::Result<Value> {
    let body = serde_json::to_vec(value).map_err(invalid_data)?;
    if body.len() > max_body_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "internal resident-prover request exceeds configured bound",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    stream.flush()?;
    let response = read_http_response(&mut stream, max_body_bytes)?;
    serde_json::from_slice(&response).map_err(invalid_data)
}

fn prepare_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn validate_private_file(path: &Path, label: &str) -> io::Result<()> {
    let metadata = fs::metadata(path).map_err(|error| {
        io::Error::new(error.kind(), format!("{label} is unavailable: {error}"))
    })?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not a regular file"),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} must not be accessible by group or other users"),
            ));
        }
    }
    Ok(())
}

fn read_bounded_file(path: &Path, maximum_bytes: usize, label: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() > maximum_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} exceeds its configured size bound"),
        ));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(maximum_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} exceeds its configured size bound"),
        ));
    }
    Ok(bytes)
}

fn read_private_json<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> io::Result<T> {
    validate_private_file(path, label)?;
    let bytes = read_bounded_file(path, MAX_PRIVATE_STATE_FILE_BYTES, label)?;
    serde_json::from_slice(&bytes).map_err(invalid_data)
}

fn write_private_json(path: &Path, value: &impl serde::Serialize) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        prepare_private_dir(parent)?;
    }
    let mut bytes = serde_json::to_vec_pretty(value).map_err(invalid_data)?;
    bytes.push(b'\n');
    let temporary = path.with_extension(format!("tmp-{}", random_hex(8)?));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_mode(0o600);
        file.set_permissions(permissions)?;
    }
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    sync_parent_directory(path)
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn random_hex(bytes: usize) -> io::Result<String> {
    if bytes == 0 || bytes > 4_096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "random byte request is outside bounds",
        ));
    }
    let mut value = vec![0_u8; bytes];
    File::open("/dev/urandom")?.read_exact(&mut value)?;
    Ok(bytes_to_hex(&value))
}

fn read_http_request(stream: &mut TcpStream, max_body_bytes: usize) -> io::Result<HttpRequest> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let first = read_bounded_http_line(&mut reader, 512, "HTTP request line")?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();
    let version = parts.next().unwrap_or_default();
    if !matches!(method.as_str(), "GET" | "POST")
        || target.is_empty()
        || target.len() > 512
        || version != "HTTP/1.1"
        || parts.next().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid HTTP request line",
        ));
    }
    let mut content_length = None;
    for header_index in 0..=64 {
        let line = read_bounded_http_line(&mut reader, 8_192, "HTTP request header")?;
        if line == "\r\n" || line == "\n" {
            break;
        }
        if header_index == 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HTTP request has too many headers",
            ));
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid HTTP request header")
        })?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "HTTP transfer-encoding is not supported",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value.trim().parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid content-length")
            })?;
            if content_length.is_some_and(|existing| existing != parsed) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "conflicting HTTP content-length headers",
                ));
            }
            content_length = Some(parsed);
        }
    }
    let content_length = content_length.unwrap_or(0);
    if content_length > max_body_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HTTP request body exceeds configured bound",
        ));
    }
    if method == "GET" && content_length != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HTTP GET requests must not carry a body",
        ));
    }
    let mut body = vec![0_u8; content_length];
    reader.read_exact(&mut body)?;
    Ok(HttpRequest {
        method,
        target,
        body,
    })
}

fn read_http_response(stream: &mut TcpStream, max_body_bytes: usize) -> io::Result<Vec<u8>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let first = read_bounded_http_line(&mut reader, 512, "HTTP response line")?;
    let status = first
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP response"))?;
    let mut content_length = None;
    for header_index in 0..=64 {
        let line = read_bounded_http_line(&mut reader, 8_192, "HTTP response header")?;
        if line == "\r\n" || line == "\n" {
            break;
        }
        if header_index == 64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP response has too many headers",
            ));
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP response header")
        })?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP dependency transfer-encoding is not supported",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value.trim().parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP content-length")
            })?;
            if content_length.is_some_and(|existing| existing != parsed) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "conflicting HTTP response content-length headers",
                ));
            }
            content_length = Some(parsed);
        }
    }
    let length = content_length.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP content-length is required",
        )
    })?;
    if length > max_body_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "HTTP response body exceeds configured bound",
        ));
    }
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    if !(200..300).contains(&status) {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("HTTP dependency returned status {status}"),
        ));
    }
    Ok(body)
}

fn read_bounded_http_line<R: BufRead>(
    reader: &mut R,
    maximum_bytes: usize,
    label: &str,
) -> io::Result<String> {
    let mut line = String::new();
    let mut limited = Read::by_ref(reader).take(maximum_bytes.saturating_add(1) as u64);
    limited.read_line(&mut line)?;
    if line.is_empty() || line.len() > maximum_bytes || !line.ends_with('\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is empty, unterminated, or exceeds its bound"),
        ));
    }
    Ok(line)
}

fn write_json_response(stream: &mut TcpStream, status: u16, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value).map_err(invalid_data)?;
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    stream.flush()
}

fn parse_config() -> io::Result<Config> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let value = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    let data_dir =
        PathBuf::from(value("--data-dir").ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "--data-dir is required")
        })?);
    let private_dir = value("--private-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("pftl-swapd"));
    let batch_dir = value("--batch-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| private_dir.join("batch-outbox"));
    let processed_batch_dir = value("--processed-batch-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| private_dir.join("processed-batches"));
    let round_driver_ready_file = value("--round-driver-ready-file")
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "--round-driver-ready-file is required",
            )
        })?;
    let transparent_key_file = value("--transparent-key-file")
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "--transparent-key-file is required",
            )
        })?;
    let bind = value("--bind")
        .unwrap_or_else(|| DEFAULT_BIND.to_string())
        .parse::<SocketAddr>()
        .map_err(invalid_data)?;
    if !bind.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "--bind must be a loopback address",
        ));
    }
    let asset_service_address = value("--asset-service-address")
        .unwrap_or_else(|| "127.0.0.1:8799".to_string())
        .parse::<SocketAddr>()
        .map_err(invalid_data)?;
    if !asset_service_address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "--asset-service-address must be loopback",
        ));
    }
    let asset_service_vault_dir = value("--asset-service-vault-dir")
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "--asset-service-vault-dir is required",
            )
        })?;
    let issue_ethereum_recipient = value("--issue-ethereum-recipient").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--issue-ethereum-recipient is required by the current v1 issue circuit",
        )
    })?;
    if issue_ethereum_recipient.len() != 42
        || !issue_ethereum_recipient.starts_with("0x")
        || !issue_ethereum_recipient[2..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--issue-ethereum-recipient must be a lowercase EVM address",
        ));
    }
    let egress_policy_id = value("--egress-policy-id").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--egress-policy-id is required",
        )
    })?;
    Ok(Config {
        data_dir,
        journal_file: private_dir.join("journal.json"),
        quote_store_file: private_dir.join("quotes.json"),
        note_index_file: private_dir.join("private-note-index.json"),
        private_dir,
        batch_dir,
        processed_batch_dir,
        round_driver_ready_file,
        round_driver_max_age: Duration::from_millis(parse_bounded_u64(
            value("--round-driver-max-age-ms"),
            5_000,
            1,
            30_000,
            "--round-driver-max-age-ms",
        )?),
        transparent_key_file,
        route_id: value("--route-id")
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "--route-id is required"))?,
        controlled_wallet_id: value("--controlled-wallet-id").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "--controlled-wallet-id is required",
            )
        })?,
        bind,
        asset_service_address,
        asset_service_vault_dir,
        quote_ttl_blocks: parse_bounded_u64(
            value("--quote-ttl-blocks"),
            2,
            1,
            20,
            "--quote-ttl-blocks",
        )?,
        maximum_fee_atoms: parse_bounded_u64(
            value("--maximum-fee-atoms"),
            DEFAULT_MAXIMUM_FEE_ATOMS,
            1,
            1_000_000,
            "--maximum-fee-atoms",
        )?,
        issue_ethereum_recipient,
        egress_policy_id,
        readiness_amount_atoms: parse_u64(
            value("--readiness-amount-atoms"),
            1_000_000,
            "--readiness-amount-atoms",
        )?,
        max_body_bytes: usize::try_from(parse_bounded_u64(
            value("--max-body-bytes"),
            DEFAULT_MAX_BODY_BYTES as u64,
            1,
            4 << 20,
            "--max-body-bytes",
        )?)
        .map_err(invalid_data)?,
        max_connections: usize::try_from(parse_bounded_u64(
            value("--max-connections"),
            DEFAULT_MAX_CONNECTIONS as u64,
            1,
            64,
            "--max-connections",
        )?)
        .map_err(invalid_data)?,
        max_swaps_per_principal_per_minute: parse_bounded_u64(
            value("--max-swaps-per-principal-per-minute"),
            DEFAULT_MAX_SWAPS_PER_PRINCIPAL_PER_MINUTE,
            1,
            600,
            "--max-swaps-per-principal-per-minute",
        )?,
        max_requests: value("--max-requests")
            .map(|value| parse_bounded_u64(Some(value), 1, 1, 1_000_000_000, "--max-requests"))
            .transpose()?,
        request_timeout: Duration::from_millis(parse_bounded_u64(
            value("--request-timeout-ms"),
            DEFAULT_REQUEST_TIMEOUT_MS,
            1_000,
            600_000,
            "--request-timeout-ms",
        )?),
    })
}

fn parse_u64(value: Option<String>, default: u64, label: &str) -> io::Result<u64> {
    parse_bounded_u64(value, default, 1, u64::MAX, label)
}

fn parse_bounded_u64(
    value: Option<String>,
    default: u64,
    minimum: u64,
    maximum: u64,
    label: &str,
) -> io::Result<u64> {
    let value = value
        .map(|value| value.parse::<u64>().map_err(invalid_data))
        .transpose()?
        .unwrap_or(default);
    if value < minimum || value > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be between {minimum} and {maximum}"),
        ));
    }
    Ok(value)
}

fn invalid_data(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Shutdown;

    fn test_root(label: &str) -> PathBuf {
        let suffix = hash_hex(
            "postfiat.pftl_swapd.test_root.v1",
            format!(
                "{label}:{}:{:?}",
                std::process::id(),
                std::time::SystemTime::now()
            )
            .as_bytes(),
        );
        std::env::temp_dir().join(format!("postfiat-swapd-{label}-{}", &suffix[..16]))
    }

    fn test_config(label: &str) -> Config {
        let root = test_root(label);
        let data_dir = root.join("data");
        let private_dir = root.join("private");
        let batch_dir = private_dir.join("outbox");
        let processed_batch_dir = private_dir.join("processed");
        let vault_dir = private_dir.join("vault");
        for path in [
            &data_dir,
            &private_dir,
            &batch_dir,
            &processed_batch_dir,
            &vault_dir,
        ] {
            prepare_private_dir(path).expect("prepare test directory");
        }
        let transparent_key_file = private_dir.join("transparent-key.json");
        write_private_json(&transparent_key_file, &json!({"test": true})).expect("write test key");
        Config {
            data_dir,
            journal_file: private_dir.join("journal.json"),
            quote_store_file: private_dir.join("quotes.json"),
            private_dir: private_dir.clone(),
            batch_dir,
            processed_batch_dir,
            round_driver_ready_file: private_dir.join("round-driver-ready.json"),
            round_driver_max_age: Duration::from_secs(5),
            transparent_key_file,
            note_index_file: private_dir.join("notes.json"),
            route_id: "route-1".to_string(),
            controlled_wallet_id: "wallet-1".to_string(),
            bind: "127.0.0.1:8798".parse().expect("test bind"),
            asset_service_address: "127.0.0.1:8799".parse().expect("test prover"),
            asset_service_vault_dir: vault_dir,
            quote_ttl_blocks: 2,
            maximum_fee_atoms: 100,
            issue_ethereum_recipient: "0x1111111111111111111111111111111111111111".to_string(),
            egress_policy_id: "egress-1".to_string(),
            readiness_amount_atoms: 1_000_000,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_connections: 4,
            max_swaps_per_principal_per_minute: 2,
            max_requests: None,
            request_timeout: Duration::from_secs(5),
        }
    }

    fn tcp_stream_with_input(bytes: &[u8]) -> TcpStream {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("test listener address");
        let mut client = TcpStream::connect(address).expect("connect test client");
        client.write_all(bytes).expect("write test request");
        client
            .shutdown(Shutdown::Write)
            .expect("shutdown test request writer");
        listener.accept().expect("accept test connection").0
    }

    #[test]
    fn prover_request_id_matches_resident_service_bound() {
        let swap_id = "ab".repeat(48);
        let request_id = pftl_swap_proving_request_id(&swap_id);
        assert_eq!(request_id.len(), 64);
        assert!(request_id.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (index > 0 && byte == b'-')
        }));
        assert_eq!(request_id, pftl_swap_proving_request_id(&swap_id));
        assert_ne!(request_id, pftl_swap_proving_request_id(&"cd".repeat(48)));
    }

    #[test]
    fn private_note_state_machine_is_fail_closed() {
        let mut record = PrivateNoteRecordV1 {
            path: PathBuf::from("/test/note.json"),
            state: PrivateNoteStateV1::Pending,
            swap_id: "11".repeat(48),
        };
        transition_private_note_state(&mut record, PrivateNoteStateV1::Spendable)
            .expect("pending note becomes spendable");
        transition_private_note_state(&mut record, PrivateNoteStateV1::Spent)
            .expect("spendable note becomes spent");
        assert_eq!(
            transition_private_note_state(&mut record, PrivateNoteStateV1::Pending)
                .expect_err("spent note cannot be resurrected")
                .kind(),
            io::ErrorKind::InvalidData,
        );
        let mut interrupted = PrivateNoteRecordV1 {
            path: PathBuf::from("/test/interrupted-note.json"),
            state: PrivateNoteStateV1::Pending,
            swap_id: "22".repeat(48),
        };
        transition_private_note_state(&mut interrupted, PrivateNoteStateV1::Discarded)
            .expect("interrupted note is discarded");
        transition_private_note_state(&mut interrupted, PrivateNoteStateV1::Pending)
            .expect("same prepublication lineage can revive its note");
    }

    #[test]
    fn http_parser_enforces_line_header_and_body_bounds() {
        let mut valid = tcp_stream_with_input(b"GET /v1/ready HTTP/1.1\r\nHost: localhost\r\n\r\n");
        let request = read_http_request(&mut valid, 1_024).expect("parse bounded GET");
        assert_eq!(request.method, "GET");
        assert_eq!(request.target, "/v1/ready");

        let mut conflicting = tcp_stream_with_input(
            b"POST /v1/quote HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\n{}",
        );
        assert_eq!(
            read_http_request(&mut conflicting, 1_024)
                .expect_err("conflicting content lengths must fail")
                .kind(),
            io::ErrorKind::InvalidInput,
        );

        let oversized_line = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(600));
        let mut oversized = tcp_stream_with_input(oversized_line.as_bytes());
        assert_eq!(
            read_http_request(&mut oversized, 1_024)
                .expect_err("oversized request line must fail")
                .kind(),
            io::ErrorKind::InvalidInput,
        );
    }

    #[test]
    fn certified_round_driver_readiness_rejects_degraded_or_stale_reports() {
        let config = test_config("round-ready");
        let height = 41;
        let state_root = "22".repeat(48);
        let block_id = "33".repeat(48);
        let now_ms: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock")
            .as_millis()
            .try_into()
            .expect("test clock fits");
        let mut report = json!({
            "schema": "postfiat-transport-peer-certified-batch-loop-ready-v2",
            "node_id": "validator-2",
            "topology_id": "controlled-six",
            "batch_kind": "shielded",
            "batch_dir": config.batch_dir,
            "processed_dir": config.processed_batch_dir,
            "artifact_root": config.private_dir.join("round-artifacts"),
            "start_height": height + 1,
            "max_rounds": 100,
            "processed_round_count": 0,
            "poll_ms": 100,
            "idle_timeout_ms": 0,
            "require_local_proposer": false,
            "require_signed_proposal": true,
            "allow_peer_failures": false,
            "quorum_early_full_propagation": false,
            "local_apply_before_certified_send": false,
            "defer_certified_sends": false,
            "persistent_vote_streams": true,
            "heartbeat_unix_ms": now_ms,
            "local_state": {
                "schema": "postfiat-transport-hello-v1",
                "topology_id": "controlled-six",
                "node_id": "validator-2",
                "chain_id": "postfiat-test",
                "genesis_hash": "44".repeat(48),
                "protocol_version": 1,
                "state_root": state_root,
                "block_height": height,
                "block_tip_hash": block_id,
            },
            "authenticated_peer_count": 5,
            "required_remote_peer_count": 5,
            "authenticated_quorum": true,
            "shielded_verifier_prewarm": {
                "requested": true,
                "asset_orchard_swap_verifier_warm": true,
                "asset_orchard_private_egress_verifier_warm": true,
            }
        });
        write_private_json(&config.round_driver_ready_file, &report)
            .expect("write healthy ready report");
        assert!(certified_round_driver_readiness(&config, height, &state_root, &block_id).is_ok());

        report["allow_peer_failures"] = Value::Bool(true);
        write_private_json(&config.round_driver_ready_file, &report)
            .expect("write degraded ready report");
        assert_eq!(
            certified_round_driver_readiness(&config, height, &state_root, &block_id)
                .expect_err("degraded round driver must fail")
                .kind(),
            io::ErrorKind::WouldBlock,
        );

        report["allow_peer_failures"] = Value::Bool(false);
        report["heartbeat_unix_ms"] = Value::from(now_ms.saturating_sub(6_000));
        write_private_json(&config.round_driver_ready_file, &report)
            .expect("write stale ready report");
        assert_eq!(
            certified_round_driver_readiness(&config, height, &state_root, &block_id)
                .expect_err("stale round driver must fail")
                .kind(),
            io::ErrorKind::WouldBlock,
        );
        let _ = fs::remove_dir_all(
            config
                .private_dir
                .parent()
                .expect("test private dir has root"),
        );
    }

    #[test]
    fn resident_swap_batches_use_certified_round_driver_suffix() {
        let batch_hash = "ab".repeat(48);
        assert_eq!(
            certified_round_batch_file_name(&batch_hash),
            format!("{batch_hash}.batch.json")
        );
    }

    #[test]
    fn principal_rate_limit_is_bounded_and_exact() {
        let config = test_config("rate-limit");
        let state = RuntimeState {
            config,
            active_connections: AtomicUsize::new(0),
            swap_active: AtomicBool::new(false),
            private_state_lock: Mutex::new(()),
            note_index_lock: Mutex::new(()),
            principal_rates: Mutex::new(BTreeMap::new()),
        };
        let principal = "pf1111111111111111111111111111111111111111";
        enforce_principal_rate_limit(&state, principal).expect("first request");
        enforce_principal_rate_limit(&state, principal).expect("second request");
        assert_eq!(
            enforce_principal_rate_limit(&state, principal)
                .expect_err("third request exceeds bound")
                .kind(),
            io::ErrorKind::WouldBlock,
        );
        let _ = fs::remove_dir_all(
            state
                .config
                .private_dir
                .parent()
                .expect("test private dir has root"),
        );
    }
}
