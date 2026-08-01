use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

use postfiat_crypto_provider::{bytes_to_hex, hash_hex};
use postfiat_execution::genesis_hash;
use postfiat_node::{
    build_atomic_shielded_action_batch, create_asset_orchard_private_egress,
    create_asset_orchard_private_primary_issue, create_asset_orchard_private_primary_issue_batch,
    create_asset_orchard_private_primary_redeem, create_asset_orchard_private_primary_redeem_batch,
    create_asset_orchard_swap_action, create_shielded_swap_action_batch,
    AssetOrchardPrivateEgressCreateOptions, AssetOrchardPrivatePrimaryIssueBatchOptions,
    AssetOrchardPrivatePrimaryIssueCreateOptions, AssetOrchardPrivatePrimaryRedeemBatchOptions,
    AssetOrchardPrivatePrimaryRedeemCreateOptions, AssetOrchardSwapCreateOptions,
    ShieldedSwapActionBatchOptions,
};
use postfiat_privacy_orchard::{
    asset_orchard_domain_genesis_hash, build_asset_orchard_wallet_note,
    encrypt_asset_orchard_wallet_note, reset_asset_orchard_private_egress_timings,
    take_asset_orchard_private_egress_timings, AssetOrchardPricingClaim,
    AssetOrchardPrivateEgressProvingKey, AssetOrchardPrivateEgressVerifyingKey,
    AssetOrchardSwapProvingKey, AssetOrchardSwapVerifyingKey, AssetTag,
};
use postfiat_storage::NodeStore;
use postfiat_types::ShieldedActionBatch;
use serde::Serialize;
use serde_json::{json, Value};

const MAX_BODY_BYTES: usize = 64 * 1024;
const DEFAULT_BIND: &str = "127.0.0.1:8799";
const NOTE_VAULT_SCHEMA: &str = "postfiat-asset-orchard-local-note-vault-record-v1";
const PREWARM_READY_SCHEMA: &str = "postfiat-asset-orchard-local-service-prewarm-ready-v1";
const MAX_CONNECTIONS: usize = 16;
static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
static PROVER_ACTIVE: AtomicBool = AtomicBool::new(false);

struct ConnectionPermit;

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct ProverPermit;

impl Drop for ProverPermit {
    fn drop(&mut self) {
        PROVER_ACTIVE.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone)]
struct Config {
    bind: SocketAddr,
    data_dir: PathBuf,
    logical_data_dir: Option<PathBuf>,
    vault_dir: PathBuf,
    prewarm_ready_file: PathBuf,
    product_profile_sha256: String,
}

#[derive(Debug, Clone)]
struct PrewarmCircuitState {
    circuit_id: &'static str,
    status: &'static str,
    started_at_unix_ms: Option<u128>,
    completed_at_unix_ms: Option<u128>,
    elapsed_ms: Option<f64>,
    k: Option<u32>,
    params_hash: Option<String>,
    vk_hash: Option<String>,
    error: Option<String>,
    note: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct PrewarmState {
    enabled: bool,
    status: &'static str,
    started_at_unix_ms: u128,
    completed_at_unix_ms: Option<u128>,
    marker_file: PathBuf,
    swap: PrewarmCircuitState,
    private_egress: PrewarmCircuitState,
    ingress_notes: PrewarmCircuitState,
}

static PROVER_WARM_STATE: OnceLock<Mutex<PrewarmState>> = OnceLock::new();

#[derive(Debug)]
struct IngressNoteRequest {
    wallet_address: String,
    asset_id: String,
    amount_atoms: u64,
}

#[derive(Debug)]
struct SwapActionRequest {
    request_id: Option<String>,
    wallet_address: String,
    liquidity_wallet_address: Option<String>,
    from_asset_id: String,
    to_asset_id: String,
    amount_atoms: u64,
    wallet_commitment: Option<String>,
    liquidity_amount_atoms: u64,
    liquidity_commitment: String,
    quote_binding_hash: String,
    quote_expires_at_ms: u128,
    pricing_claim: AssetOrchardPricingClaim,
    input_note_path_a: Option<String>,
    input_note_path_b: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapCircuitOrder {
    WalletIsBase,
    WalletIsQuote,
}

#[derive(Debug)]
struct SwapBatchRequest {
    swap_action_json: String,
}

#[derive(Debug)]
struct AtomicBatchRequest {
    batches: Vec<ShieldedActionBatch>,
}

#[derive(Debug)]
struct PrivateEgressActionRequest {
    wallet_address: String,
    to: String,
    asset_id: String,
    amount_atoms: u64,
    note_commitment: Option<String>,
    input_note_path: Option<String>,
    policy_id: String,
    disclosure_hash: String,
    pending_output_commitments: Vec<String>,
}

#[derive(Debug)]
struct PrivatePrimaryIssueActionRequest {
    request_id: String,
    input_note_path: String,
    route_id: String,
    subscriber: String,
    ethereum_recipient: String,
    reservation_id: String,
    subscription_nonce: String,
    mint_amount_atoms: u64,
    settlement_value_atoms: u64,
    expires_at_height: u64,
    pending_output_commitments: Vec<String>,
}

#[derive(Debug)]
struct PrivatePrimaryRedeemActionRequest {
    request_id: String,
    input_note_path: String,
    route_id: String,
    owner: String,
    settlement_recipient: String,
    nav_amount_atoms: u64,
    settlement_output_atoms: u64,
    expires_at_height: u64,
    pending_output_commitments: Vec<String>,
}

#[derive(Debug, Serialize)]
struct VaultRecordPublic {
    id: String,
    stored: bool,
    schema: &'static str,
}

fn main() -> io::Result<()> {
    let config = parse_config()?;
    if !config.bind.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "asset-orchard local service must bind a loopback address",
        ));
    }
    prepare_private_dir(&config.vault_dir)?;
    configure_rayon_threads_env();
    let listener = TcpListener::bind(config.bind)?;
    start_prover_prewarm(&config)?;
    eprintln!(
        "Asset-Orchard local ingress note service listening on {}",
        config.bind
    );
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if stream
                    .peer_addr()
                    .is_ok_and(|peer| !peer.ip().is_loopback())
                {
                    let _ = write_json_response(
                        &mut stream,
                        403,
                        &json!({ "ok": false, "error": "non_loopback_peer" }),
                    );
                    continue;
                }
                let Some(permit) = try_acquire_connection() else {
                    let _ = write_json_response(
                        &mut stream,
                        503,
                        &json!({ "ok": false, "error": "connection_capacity_exhausted" }),
                    );
                    continue;
                };
                let request_config = config.clone();
                thread::Builder::new()
                    .name("asset-orchard-local-request".to_string())
                    .spawn(move || {
                        let _permit = permit;
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(120)));
                        if let Err(error) = handle_connection(&request_config, &mut stream) {
                            eprintln!("asset-orchard local request failed: {}", error.kind());
                            let _ = write_json_response(
                                &mut stream,
                                500,
                                &json!({ "ok": false, "error": error.kind().to_string() }),
                            );
                        }
                    })?;
            }
            Err(error) => eprintln!("asset-orchard local service accept failed: {error}"),
        }
    }
    Ok(())
}

fn try_acquire_connection() -> Option<ConnectionPermit> {
    let mut current = ACTIVE_CONNECTIONS.load(Ordering::Acquire);
    loop {
        if current >= MAX_CONNECTIONS {
            return None;
        }
        match ACTIVE_CONNECTIONS.compare_exchange_weak(
            current,
            current + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return Some(ConnectionPermit),
            Err(observed) => current = observed,
        }
    }
}

fn try_acquire_prover() -> io::Result<ProverPermit> {
    PROVER_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "resident prover capacity is exhausted",
            )
        })?;
    Ok(ProverPermit)
}

fn parse_config() -> io::Result<Config> {
    let mut bind =
        env::var("ASSET_ORCHARD_LOCAL_SERVICE_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_string());
    let mut data_dir = env::var("POSTFIAT_DATA_DIR")
        .or_else(|_| env::var("NAVSWAP_SHIELDED_INGRESS_DATA_DIR"))
        .ok()
        .map(PathBuf::from);
    let mut vault_dir = env::var("ASSET_ORCHARD_LOCAL_VAULT_DIR")
        .ok()
        .map(PathBuf::from);
    let mut logical_data_dir = env::var("ASSET_ORCHARD_LOGICAL_DATA_DIR")
        .ok()
        .map(PathBuf::from);
    let mut prewarm_ready_file = env::var("ASSET_ORCHARD_PREWARM_READY_FILE")
        .ok()
        .map(PathBuf::from);
    let product_profile_sha256 = env::var("POSTFIAT_ASSET_ORCHARD_PRODUCT_PROFILE_SHA256")
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "POSTFIAT_ASSET_ORCHARD_PRODUCT_PROFILE_SHA256 is required",
            )
        })?;
    if product_profile_sha256.len() != 64
        || !product_profile_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "POSTFIAT_ASSET_ORCHARD_PRODUCT_PROFILE_SHA256 must be 64 hexadecimal characters",
        ));
    }

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                bind = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--bind requires an address")
                })?;
            }
            "--data-dir" => {
                data_dir = Some(PathBuf::from(args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--data-dir requires a path")
                })?));
            }
            "--vault-dir" => {
                vault_dir = Some(PathBuf::from(args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--vault-dir requires a path")
                })?));
            }
            "--logical-data-dir" => {
                logical_data_dir = Some(PathBuf::from(args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--logical-data-dir requires a path",
                    )
                })?));
            }
            "--prewarm-ready-file" => {
                prewarm_ready_file = Some(PathBuf::from(args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--prewarm-ready-file requires a path",
                    )
                })?));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument `{other}`"),
                ));
            }
        }
    }

    let bind: SocketAddr = bind.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid --bind address: {error}"),
        )
    })?;
    if !matches!(bind.ip(), IpAddr::V4(_) | IpAddr::V6(_)) || !bind.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--bind must be a loopback address",
        ));
    }

    let vault_dir = vault_dir.unwrap_or_else(default_vault_dir);
    let prewarm_ready_file =
        prewarm_ready_file.unwrap_or_else(|| vault_dir.join("prewarm-ready.json"));
    if logical_data_dir
        .as_ref()
        .is_some_and(|path| !path.is_absolute())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--logical-data-dir must be an absolute path",
        ));
    }

    Ok(Config {
        bind,
        data_dir: data_dir.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "POSTFIAT_DATA_DIR or --data-dir is required",
            )
        })?,
        logical_data_dir,
        vault_dir,
        prewarm_ready_file,
        product_profile_sha256: product_profile_sha256.to_ascii_lowercase(),
    })
}

fn print_usage() {
    println!(
        "usage: asset-orchard-local-service [--bind 127.0.0.1:8789] --data-dir PATH [--logical-data-dir PATH] [--vault-dir PATH] [--prewarm-ready-file PATH]"
    );
}

fn default_vault_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".local/share/postfiat/asset-orchard-local-vault")
}

fn configure_rayon_threads_env() {
    if env::var_os("RAYON_NUM_THREADS").is_some() {
        return;
    }
    if let Ok(threads) = thread::available_parallelism() {
        env::set_var("RAYON_NUM_THREADS", threads.get().to_string());
    }
}

fn prover_prewarm_enabled() -> bool {
    !matches!(
        env::var("ASSET_ORCHARD_LOCAL_SERVICE_PREWARM")
            .unwrap_or_else(|_| "1".to_string())
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "no" | "off"
    )
}

fn prover_circuit_prewarm_enabled(variable: &str) -> bool {
    prover_prewarm_enabled()
        && !matches!(
            env::var(variable)
                .unwrap_or_else(|_| "1".to_string())
                .to_ascii_lowercase()
                .as_str(),
            "0" | "false" | "no" | "off"
        )
}

fn start_prover_prewarm(config: &Config) -> io::Result<()> {
    if let Some(parent) = config.prewarm_ready_file.parent() {
        prepare_private_dir(parent)?;
    }
    let enabled = prover_prewarm_enabled();
    let swap_enabled = prover_circuit_prewarm_enabled("ASSET_ORCHARD_LOCAL_SERVICE_PREWARM_SWAP");
    let private_egress_enabled =
        prover_circuit_prewarm_enabled("ASSET_ORCHARD_LOCAL_SERVICE_PREWARM_PRIVATE_EGRESS");
    let started_at_unix_ms = unix_ms()?;
    let state = PrewarmState::new_with_circuits(
        enabled,
        swap_enabled,
        private_egress_enabled,
        started_at_unix_ms,
        config.prewarm_ready_file.clone(),
    );
    let _ = PROVER_WARM_STATE.set(Mutex::new(state));
    if !enabled {
        write_prewarm_marker_if_terminal();
        return Ok(());
    }

    if swap_enabled {
        thread::Builder::new()
            .name("asset-orchard-prewarm-swap".to_string())
            .spawn(|| {
                let start = Instant::now();
                let result = prewarm_swap_keys().map_err(|error| error.to_string());
                finish_prewarm_circuit("swap", start, result);
            })?;
    }

    if private_egress_enabled {
        thread::Builder::new()
            .name("asset-orchard-prewarm-private-egress".to_string())
            .spawn(|| {
                let start = Instant::now();
                let result = prewarm_private_egress_keys().map_err(|error| error.to_string());
                finish_prewarm_circuit("private_egress", start, result);
            })?;
    }
    write_prewarm_marker_if_terminal();

    Ok(())
}

fn prewarm_swap_keys() -> Result<(u32, String, String), Box<dyn std::error::Error>> {
    let proving_key = AssetOrchardSwapProvingKey::cached()?;
    let _verifying_key = AssetOrchardSwapVerifyingKey::cached()?;
    let metadata = proving_key.metadata();
    Ok((
        metadata.k,
        metadata.params_hash.clone(),
        metadata.vk_hash.clone(),
    ))
}

fn prewarm_private_egress_keys() -> Result<(u32, String, String), Box<dyn std::error::Error>> {
    let proving_key = AssetOrchardPrivateEgressProvingKey::cached()?;
    let _verifying_key = AssetOrchardPrivateEgressVerifyingKey::cached()?;
    let metadata = proving_key.metadata();
    Ok((
        metadata.k,
        metadata.params_hash.clone(),
        metadata.vk_hash.clone(),
    ))
}

fn finish_prewarm_circuit(
    circuit: &'static str,
    start: Instant,
    result: Result<(u32, String, String), String>,
) {
    let completed_at_unix_ms = unix_ms().ok();
    if let Some(lock) = PROVER_WARM_STATE.get() {
        if let Ok(mut state) = lock.lock() {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            let target = state.circuit_mut(circuit);
            target.completed_at_unix_ms = completed_at_unix_ms;
            target.elapsed_ms = Some(elapsed_ms);
            match result {
                Ok((k, params_hash, vk_hash)) => {
                    target.status = "ready";
                    target.k = Some(k);
                    target.params_hash = Some(params_hash);
                    target.vk_hash = Some(vk_hash);
                    target.error = None;
                }
                Err(error) => {
                    target.status = "error";
                    target.error = Some(error);
                }
            }
            state.recompute_status(completed_at_unix_ms);
        }
    }
    write_prewarm_marker_if_terminal();
}

fn write_prewarm_marker_if_terminal() {
    let Some(lock) = PROVER_WARM_STATE.get() else {
        return;
    };
    let (terminal, marker_file, snapshot) = match lock.lock() {
        Ok(state) => (
            state.is_terminal(),
            state.marker_file.clone(),
            state.to_json(),
        ),
        Err(_) => return,
    };
    if terminal {
        let _ = atomic_write_private_json(&marker_file, &snapshot);
    }
}

fn prover_warm_snapshot(config: &Config) -> Value {
    if let Some(lock) = PROVER_WARM_STATE.get() {
        if let Ok(state) = lock.lock() {
            return state.to_json();
        }
    }
    json!({
        "schema": PREWARM_READY_SCHEMA,
        "enabled": prover_prewarm_enabled(),
        "ready": false,
        "status": "not_started",
        "prewarm_ready_file": config.prewarm_ready_file.display().to_string(),
        "circuits": {
            "swap": PrewarmCircuitState::pending("asset-orchard-swap-v1").to_json(),
            "private_egress": PrewarmCircuitState::pending("asset-orchard-private-egress-v1").to_json(),
            "ingress_notes": PrewarmCircuitState::not_applicable(
                "asset-orchard-ingress-notes",
                "ingress note creation has no separate Halo2 proving key in this implementation"
            ).to_json(),
        },
        "disk_pk_vk_cache": disk_pk_vk_cache_capability(),
        "threading": prover_threading_capability(),
    })
}

fn disk_pk_vk_cache_capability() -> Value {
    json!({
        "supported": false,
        "status": "skipped",
        "reason": "pinned halo2_proofs exposes verifier pinned-assembly serialization but no ProvingKey read/write API; resident service prewarm remains the primary warm path",
        "stale_cache_test_required": false,
    })
}

fn prover_threading_capability() -> Value {
    json!({
        "halo2_multicore_feature": "explicitly_enabled",
        "rayon_num_threads": env::var("RAYON_NUM_THREADS").ok(),
        "available_parallelism": thread::available_parallelism().ok().map(|threads| threads.get()),
    })
}

impl PrewarmCircuitState {
    fn pending(circuit_id: &'static str) -> Self {
        Self {
            circuit_id,
            status: "warming",
            started_at_unix_ms: unix_ms().ok(),
            completed_at_unix_ms: None,
            elapsed_ms: None,
            k: None,
            params_hash: None,
            vk_hash: None,
            error: None,
            note: None,
        }
    }

    fn disabled(circuit_id: &'static str) -> Self {
        Self {
            circuit_id,
            status: "disabled",
            started_at_unix_ms: None,
            completed_at_unix_ms: None,
            elapsed_ms: None,
            k: None,
            params_hash: None,
            vk_hash: None,
            error: None,
            note: Some("prewarm disabled by ASSET_ORCHARD_LOCAL_SERVICE_PREWARM"),
        }
    }

    fn not_applicable(circuit_id: &'static str, note: &'static str) -> Self {
        Self {
            circuit_id,
            status: "not_applicable",
            started_at_unix_ms: None,
            completed_at_unix_ms: None,
            elapsed_ms: None,
            k: None,
            params_hash: None,
            vk_hash: None,
            error: None,
            note: Some(note),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "circuit_id": self.circuit_id,
            "status": self.status,
            "ready": self.status == "ready" || self.status == "not_applicable",
            "started_at_unix_ms": self.started_at_unix_ms.map(|value| value.to_string()),
            "completed_at_unix_ms": self.completed_at_unix_ms.map(|value| value.to_string()),
            "elapsed_ms": self.elapsed_ms,
            "k": self.k,
            "params_hash": self.params_hash,
            "vk_hash": self.vk_hash,
            "error": self.error,
            "note": self.note,
        })
    }
}

impl PrewarmState {
    #[cfg(test)]
    fn new(enabled: bool, started_at_unix_ms: u128, marker_file: PathBuf) -> Self {
        Self::new_with_circuits(enabled, enabled, enabled, started_at_unix_ms, marker_file)
    }

    fn new_with_circuits(
        enabled: bool,
        swap_enabled: bool,
        private_egress_enabled: bool,
        started_at_unix_ms: u128,
        marker_file: PathBuf,
    ) -> Self {
        let (status, swap, private_egress) = if enabled {
            (
                "warming",
                if swap_enabled {
                    PrewarmCircuitState::pending("asset-orchard-swap-v1")
                } else {
                    PrewarmCircuitState::not_applicable(
                        "asset-orchard-swap-v1",
                        "swap prewarm disabled by ASSET_ORCHARD_LOCAL_SERVICE_PREWARM_SWAP",
                    )
                },
                if private_egress_enabled {
                    PrewarmCircuitState::pending("asset-orchard-private-egress-v1")
                } else {
                    PrewarmCircuitState::not_applicable(
                        "asset-orchard-private-egress-v1",
                        "private-egress prewarm disabled by ASSET_ORCHARD_LOCAL_SERVICE_PREWARM_PRIVATE_EGRESS",
                    )
                },
            )
        } else {
            (
                "disabled",
                PrewarmCircuitState::disabled("asset-orchard-swap-v1"),
                PrewarmCircuitState::disabled("asset-orchard-private-egress-v1"),
            )
        };
        let mut state = Self {
            enabled,
            status,
            started_at_unix_ms,
            completed_at_unix_ms: if enabled {
                None
            } else {
                Some(started_at_unix_ms)
            },
            marker_file,
            swap,
            private_egress,
            ingress_notes: PrewarmCircuitState::not_applicable(
                "asset-orchard-ingress-notes",
                "ingress note creation has no separate Halo2 proving key in this implementation",
            ),
        };
        state.recompute_status(Some(started_at_unix_ms));
        state
    }

    fn circuit_mut(&mut self, circuit: &str) -> &mut PrewarmCircuitState {
        match circuit {
            "swap" => &mut self.swap,
            "private_egress" => &mut self.private_egress,
            _ => &mut self.ingress_notes,
        }
    }

    fn recompute_status(&mut self, completed_at_unix_ms: Option<u128>) {
        if self.swap.status == "error" || self.private_egress.status == "error" {
            self.status = "error";
            self.completed_at_unix_ms = completed_at_unix_ms;
        } else if matches!(self.swap.status, "ready" | "not_applicable")
            && matches!(self.private_egress.status, "ready" | "not_applicable")
        {
            self.status = "ready";
            self.completed_at_unix_ms = completed_at_unix_ms;
        } else if self.enabled {
            self.status = "warming";
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self.status, "ready" | "error" | "disabled")
    }

    fn to_json(&self) -> Value {
        json!({
            "schema": PREWARM_READY_SCHEMA,
            "enabled": self.enabled,
            "ready": self.status == "ready",
            "status": self.status,
            "started_at_unix_ms": self.started_at_unix_ms.to_string(),
            "completed_at_unix_ms": self.completed_at_unix_ms.map(|value| value.to_string()),
            "prewarm_ready_file": self.marker_file.display().to_string(),
            "circuits": {
                "swap": self.swap.to_json(),
                "private_egress": self.private_egress.to_json(),
                "ingress_notes": self.ingress_notes.to_json(),
            },
            "disk_pk_vk_cache": disk_pk_vk_cache_capability(),
            "threading": prover_threading_capability(),
        })
    }
}

fn handle_connection(config: &Config, stream: &mut TcpStream) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if request_complete(&buffer)? {
            break;
        }
        if buffer.len() > MAX_BODY_BYTES + 8192 {
            return write_json_response(
                stream,
                413,
                &json!({ "ok": false, "error": "request too large" }),
            );
        }
    }

    let request = parse_http_request(&buffer)?;
    match (request.method.as_str(), request.path.as_str()) {
        ("OPTIONS", _) => write_json_response(stream, 200, &json!({ "ok": true })),
        ("GET", "/asset-orchard/readiness") => {
            write_json_response(stream, 200, &local_readiness(config))
        }
        ("GET", "/asset-orchard/notes") => match list_public_notes(config) {
            Ok(response) => write_json_response(stream, 200, &response),
            Err(error) => write_json_response(stream, 400, &error_response(&error)),
        },
        ("POST", "/asset-orchard/ingress-notes") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            let ingress = parse_ingress_note_request(&body)?;
            match build_and_store_note(config, &ingress) {
                Ok((wallet_note, encrypted_output, vault_record)) => write_json_response(
                    stream,
                    200,
                    &json!({
                        "ok": true,
                        "wallet_note": wallet_note,
                        "encrypted_output": encrypted_output,
                        "vault_record": vault_record,
                    }),
                ),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/swap-actions") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            let swap = parse_swap_action_request(&body)?;
            if reject_if_prover_not_ready(config, stream)? {
                return Ok(());
            }
            let _prover = match try_acquire_prover() {
                Ok(permit) => permit,
                Err(_) => {
                    return write_json_response(
                        stream,
                        503,
                        &json!({ "ok": false, "error": "prover_capacity_exhausted" }),
                    );
                }
            };
            match build_and_store_swap_action(config, &swap) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/swap-batch") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            let swap = parse_swap_batch_request(&body)?;
            match build_swap_batch(config, &swap) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/atomic-batch") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            let atomic = parse_atomic_batch_request(&body)?;
            match build_atomic_batch(config, &atomic) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/swap-finalize") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            match finalize_swap(config, &body) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/private-egress-actions") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            let mut egress = parse_private_egress_action_request(&body)?;
            if let Some(path) = egress.input_note_path.as_deref() {
                egress.input_note_path = Some(
                    resolve_client_note_path(config, path)?
                        .display()
                        .to_string(),
                );
            }
            if reject_if_prover_not_ready(config, stream)? {
                return Ok(());
            }
            let _prover = match try_acquire_prover() {
                Ok(permit) => permit,
                Err(_) => {
                    return write_json_response(
                        stream,
                        503,
                        &json!({ "ok": false, "error": "prover_capacity_exhausted" }),
                    );
                }
            };
            match build_and_store_private_egress_action(config, &egress) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/private-primary-issue-actions") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            let issue = parse_private_primary_issue_action_request(&body)?;
            if reject_if_prover_not_ready(config, stream)? {
                return Ok(());
            }
            let _prover = match try_acquire_prover() {
                Ok(permit) => permit,
                Err(_) => {
                    return write_json_response(
                        stream,
                        503,
                        &json!({ "ok": false, "error": "prover_capacity_exhausted" }),
                    );
                }
            };
            match build_and_store_private_primary_issue_action(config, &issue) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/private-primary-redeem-actions") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            let redeem = parse_private_primary_redeem_action_request(&body)?;
            if reject_if_prover_not_ready(config, stream)? {
                return Ok(());
            }
            let _prover = match try_acquire_prover() {
                Ok(permit) => permit,
                Err(_) => {
                    return write_json_response(
                        stream,
                        503,
                        &json!({ "ok": false, "error": "prover_capacity_exhausted" }),
                    );
                }
            };
            match build_and_store_private_primary_redeem_action(config, &redeem) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        ("POST", "/asset-orchard/private-egress-finalize") => {
            let body: Value = serde_json::from_slice(&request.body).map_err(invalid_json)?;
            if let Some(path) = find_forbidden_private_material(&body, "$") {
                return write_json_response(
                    stream,
                    400,
                    &json!({
                        "ok": false,
                        "error": "forbidden_private_material",
                        "message": format!("request contains forbidden private material at {path}"),
                    }),
                );
            }
            match finalize_private_egress(config, &body) {
                Ok(response) => write_json_response(stream, 200, &response),
                Err(error) => write_json_response(stream, 400, &error_response(&error)),
            }
        }
        _ => write_json_response(stream, 404, &json!({ "ok": false, "error": "not_found" })),
    }
}

fn reject_if_prover_not_ready(config: &Config, stream: &mut TcpStream) -> io::Result<bool> {
    let readiness = local_readiness(config);
    if readiness.get("ready").and_then(Value::as_bool) == Some(true) {
        return Ok(false);
    }
    write_json_response(
        stream,
        503,
        &json!({
            "ok": false,
            "error": "service_not_ready",
            "readiness": readiness,
        }),
    )?;
    Ok(true)
}

fn private_primary_work_dir(config: &Config, operation: &str, request_id: &str) -> PathBuf {
    config
        .vault_dir
        .join("private-primary-work")
        .join(operation)
        .join(request_id)
}

fn resolve_client_note_path(config: &Config, client_path: &str) -> io::Result<PathBuf> {
    let path = Path::new(client_path);
    let Some(logical_root) = config.logical_data_dir.as_ref() else {
        return Ok(path.to_path_buf());
    };
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private note path must be absolute",
        ));
    }
    let relative = path.strip_prefix(logical_root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private note path is outside the logical data directory",
        )
    })?;
    if relative.components().any(|component| {
        !matches!(
            component,
            std::path::Component::Normal(_) | std::path::Component::CurDir
        )
    }) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private note path contains a non-local component",
        ));
    }
    let data_root = config.data_dir.canonicalize()?;
    let resolved = config
        .data_dir
        .join(relative)
        .canonicalize()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                "private note path does not resolve in the mounted data directory",
            )
        })?;
    if !resolved.starts_with(&data_root) || !resolved.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private note path is outside the mounted data directory",
        ));
    }
    Ok(resolved)
}

fn client_output_note_path(config: &Config, local_path: &Path) -> io::Result<String> {
    let Some(logical_root) = config.logical_data_dir.as_ref() else {
        return Ok(local_path.display().to_string());
    };
    let data_root = config.data_dir.canonicalize()?;
    let resolved = local_path.canonicalize()?;
    let relative = resolved.strip_prefix(&data_root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private output note is outside the mounted data directory",
        )
    })?;
    Ok(logical_root.join(relative).display().to_string())
}

fn cached_or_prepare_private_primary_work_dir(
    config: &Config,
    operation: &str,
    request_id: &str,
) -> io::Result<(PathBuf, Option<Value>)> {
    let work_dir = private_primary_work_dir(config, operation, request_id);
    let response_file = work_dir.join("response.json");
    if response_file.exists() {
        let response = serde_json::from_slice(&fs::read(response_file)?).map_err(invalid_json)?;
        return Ok((work_dir, Some(response)));
    }
    if work_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{operation} request `{request_id}` has incomplete prior state; audit it and use a new request_id"
            ),
        ));
    }
    prepare_private_dir(&work_dir)?;
    Ok((work_dir, None))
}

fn build_and_store_private_primary_issue_action(
    config: &Config,
    request: &PrivatePrimaryIssueActionRequest,
) -> io::Result<Value> {
    let (work_dir, cached) =
        cached_or_prepare_private_primary_work_dir(config, "issue", &request.request_id)?;
    if let Some(response) = cached {
        return Ok(response);
    }
    let action_file = work_dir.join("action.json");
    let batch_file = work_dir.join("batch.json");
    let output_note_file = work_dir.join("output-note.json");
    let output_seed_hex = bytes_to_hex(&random_seed()?);

    let total_start = Instant::now();
    reset_asset_orchard_private_egress_timings();
    let input_note_path = resolve_client_note_path(config, &request.input_note_path)?;
    let report = match create_asset_orchard_private_primary_issue(
        AssetOrchardPrivatePrimaryIssueCreateOptions {
            data_dir: config.data_dir.clone(),
            note_file: input_note_path,
            output_note_seed_hex: output_seed_hex,
            output_note_file: output_note_file.clone(),
            route_id: request.route_id.clone(),
            subscriber: request.subscriber.clone(),
            ethereum_recipient: request.ethereum_recipient.clone(),
            reservation_id: request.reservation_id.clone(),
            subscription_nonce: request.subscription_nonce.clone(),
            mint_amount_atoms: request.mint_amount_atoms,
            settlement_value_atoms: request.settlement_value_atoms,
            expires_at_height: request.expires_at_height,
            pending_output_commitments: request.pending_output_commitments.clone(),
            action_file: action_file.clone(),
            overwrite: false,
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            let _ = take_asset_orchard_private_egress_timings();
            return Err(error);
        }
    };
    let proof_timing = private_egress_timing_value()?;
    let batch = create_asset_orchard_private_primary_issue_batch(
        AssetOrchardPrivatePrimaryIssueBatchOptions {
            data_dir: config.data_dir.clone(),
            action_file: action_file.clone(),
            batch_file: batch_file.clone(),
        },
    )?;
    let response = json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-private-primary-issue-action-v1",
        "request_id": request.request_id,
        "action": serde_json::from_slice::<Value>(&fs::read(&action_file)?).map_err(invalid_json)?,
        "batch": batch,
        "verification": report,
        "output_note_path": client_output_note_path(config, &output_note_file)?,
        "timing": {
            "total_ms": total_start.elapsed().as_secs_f64() * 1000.0,
            "proof": proof_timing
        },
        "readiness": local_readiness(config),
    });
    atomic_write_private_json(&work_dir.join("response.json"), &response)?;
    Ok(response)
}

fn build_and_store_private_primary_redeem_action(
    config: &Config,
    request: &PrivatePrimaryRedeemActionRequest,
) -> io::Result<Value> {
    let (work_dir, cached) =
        cached_or_prepare_private_primary_work_dir(config, "redeem", &request.request_id)?;
    if let Some(response) = cached {
        return Ok(response);
    }
    let action_file = work_dir.join("action.json");
    let batch_file = work_dir.join("batch.json");
    let output_note_file = work_dir.join("output-note.json");
    let output_seed_hex = bytes_to_hex(&random_seed()?);
    let redemption_id = bytes_to_hex(&random_bytes::<48>()?);
    let redemption_nonce = bytes_to_hex(&random_seed()?);

    let total_start = Instant::now();
    reset_asset_orchard_private_egress_timings();
    let input_note_path = resolve_client_note_path(config, &request.input_note_path)?;
    let report = match create_asset_orchard_private_primary_redeem(
        AssetOrchardPrivatePrimaryRedeemCreateOptions {
            data_dir: config.data_dir.clone(),
            note_file: input_note_path,
            output_note_seed_hex: output_seed_hex,
            output_note_file: output_note_file.clone(),
            route_id: request.route_id.clone(),
            owner: request.owner.clone(),
            settlement_recipient: request.settlement_recipient.clone(),
            redemption_id,
            redemption_nonce,
            nav_amount_atoms: request.nav_amount_atoms,
            settlement_output_atoms: request.settlement_output_atoms,
            expires_at_height: request.expires_at_height,
            pending_output_commitments: request.pending_output_commitments.clone(),
            action_file: action_file.clone(),
            overwrite: false,
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            let _ = take_asset_orchard_private_egress_timings();
            return Err(error);
        }
    };
    let proof_timing = private_egress_timing_value()?;
    let batch = create_asset_orchard_private_primary_redeem_batch(
        AssetOrchardPrivatePrimaryRedeemBatchOptions {
            data_dir: config.data_dir.clone(),
            action_file: action_file.clone(),
            batch_file: batch_file.clone(),
        },
    )?;
    let response = json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-private-primary-redeem-action-v1",
        "request_id": request.request_id,
        "action": serde_json::from_slice::<Value>(&fs::read(&action_file)?).map_err(invalid_json)?,
        "batch": batch,
        "verification": report,
        "output_note_path": client_output_note_path(config, &output_note_file)?,
        "timing": {
            "total_ms": total_start.elapsed().as_secs_f64() * 1000.0,
            "proof": proof_timing
        },
        "readiness": local_readiness(config),
    });
    atomic_write_private_json(&work_dir.join("response.json"), &response)?;
    Ok(response)
}

fn build_and_store_note(
    config: &Config,
    request: &IngressNoteRequest,
) -> io::Result<(Value, String, VaultRecordPublic)> {
    let genesis = NodeStore::new(&config.data_dir).read_genesis()?;
    let genesis_hash_hex = genesis_hash(&genesis);
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash_hex)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let seed_hex = bytes_to_hex(&random_seed()?);
    let wallet_note = build_asset_orchard_wallet_note(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
        &request.asset_id,
        request.amount_atoms,
        &seed_hex,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    let encrypted_output = bytes_to_hex(
        &encrypt_asset_orchard_wallet_note(
            &genesis.chain_id,
            genesis_hash_32,
            genesis.protocol_version,
            &wallet_note,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?
        .to_bytes()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?,
    );
    let note_value = serde_json::to_value(&wallet_note)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let output_commitment = note_value
        .get("output_commitment")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "wallet note missing output commitment",
            )
        })?
        .to_string();
    let record_id = output_commitment.clone();
    let record = json!({
        "schema": NOTE_VAULT_SCHEMA,
        "created_at_unix_ms": unix_ms()?,
        "wallet_address": request.wallet_address,
        "asset_id": request.asset_id,
        "amount_atoms": request.amount_atoms,
        "chain_id": genesis.chain_id,
        "genesis_hash": genesis_hash_hex,
        "protocol_version": genesis.protocol_version,
        "wallet_note": note_value,
    });
    let path = config.vault_dir.join(format!("{record_id}.json"));
    atomic_write_private_json(&path, &record)?;
    Ok((
        record.get("wallet_note").cloned().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "vault record missing wallet note",
            )
        })?,
        encrypted_output,
        VaultRecordPublic {
            id: record_id,
            stored: true,
            schema: NOTE_VAULT_SCHEMA,
        },
    ))
}

fn swap_circuit_order(request: &SwapActionRequest) -> io::Result<SwapCircuitOrder> {
    let wallet_tag = AssetTag::derive(&request.from_asset_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    let facility_tag = AssetTag::derive(&request.to_asset_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    let base_tag = AssetTag {
        lo: request.pricing_claim.base_asset_tag_lo,
        hi: request.pricing_claim.base_asset_tag_hi,
    };
    let quote_tag = AssetTag {
        lo: request.pricing_claim.quote_asset_tag_lo,
        hi: request.pricing_claim.quote_asset_tag_hi,
    };
    match (
        wallet_tag == base_tag && facility_tag == quote_tag,
        wallet_tag == quote_tag && facility_tag == base_tag,
    ) {
        (true, false) => Ok(SwapCircuitOrder::WalletIsBase),
        (false, true) => Ok(SwapCircuitOrder::WalletIsQuote),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "swap assets do not match the pricing claim base/quote tags",
        )),
    }
}

fn build_and_store_swap_action(config: &Config, request: &SwapActionRequest) -> io::Result<Value> {
    let request_fingerprint = swap_action_request_fingerprint(request)?;
    let durable_work_dir = request.request_id.as_deref().map(|request_id| {
        config
            .vault_dir
            .join("swap-work")
            .join("by-request")
            .join(request_id)
    });
    if let Some(work_dir) = durable_work_dir.as_ref() {
        let response_file = work_dir.join("response.json");
        if response_file.exists() {
            let response: Value =
                serde_json::from_slice(&fs::read(response_file)?).map_err(invalid_json)?;
            if string_value(&response, "request_fingerprint").as_deref()
                != Some(request_fingerprint.as_str())
            {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "swap request_id is already bound to different immutable request fields",
                ));
            }
            return Ok(response);
        }
        if work_dir.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "swap request `{}` has incomplete prior state; finalize or audit it before retry",
                    request.request_id.as_deref().unwrap_or("")
                ),
            ));
        }
    }
    if request.quote_expires_at_ms <= unix_ms()? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "quote is expired",
        ));
    }

    let (wallet_input, pool_input) = swap_input_records(config, request)?;
    let wallet_input_id = note_record_id(&wallet_input)?;
    let pool_input_id = note_record_id(&pool_input)?;
    if wallet_input_id == pool_input_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wallet and pool input notes must be distinct",
        ));
    }

    let work_dir = if let Some(work_dir) = durable_work_dir {
        prepare_private_dir(&work_dir)?;
        work_dir
    } else {
        let work_dir = config.vault_dir.join("swap-work").join(format!(
            "{}-{}",
            std::process::id(),
            unix_ms()?
        ));
        prepare_private_dir(&work_dir)?;
        work_dir
    };
    let input_a = work_dir.join("input-wallet.json");
    let input_b = work_dir.join("input-pool.json");
    let action_file = work_dir.join("swap-action.json");
    let pricing_claim_file = work_dir.join("pricing-claim.json");
    let output_a = work_dir.join("output-wallet.json");
    let output_b = work_dir.join("output-pool.json");
    atomic_write_private_json(&input_a, wallet_note_value(&wallet_input)?)?;
    atomic_write_private_json(&input_b, wallet_note_value(&pool_input)?)?;
    let pricing_claim_json = serde_json::to_value(&request.pricing_claim).map_err(invalid_json)?;
    atomic_write_private_json(&pricing_claim_file, &pricing_claim_json)?;

    // The circuit requires [base, quote] inputs and returns [quote, base].
    // Wallet-facing `from`/`to` is independent of that canonical order: NAV
    // primary routes historically put the wallet on the quote side, while a
    // pfUSDC -> pNOK FIX puts it on the base side. Derive the ordering from the
    // finalized pricing-claim tags instead of assuming either market shape.
    let (input_note_files, output_note_files) = match swap_circuit_order(request)? {
        SwapCircuitOrder::WalletIsBase => (
            [input_a.clone(), input_b.clone()],
            [output_a.clone(), output_b.clone()],
        ),
        SwapCircuitOrder::WalletIsQuote => (
            [input_b.clone(), input_a.clone()],
            [output_b.clone(), output_a.clone()],
        ),
    };

    reset_asset_orchard_private_egress_timings();
    let report = match create_asset_orchard_swap_action(AssetOrchardSwapCreateOptions {
        data_dir: config.data_dir.clone(),
        input_note_files,
        output_note_seed_hexes: [bytes_to_hex(&random_seed()?), bytes_to_hex(&random_seed()?)],
        pricing_claim_file,
        action_file: action_file.clone(),
        output_note_files,
        overwrite: true,
    }) {
        Ok(report) => report,
        Err(error) => {
            let _ = take_asset_orchard_private_egress_timings();
            return Err(error);
        }
    };
    let timing = private_egress_timing_value()?;
    let action: Value = serde_json::from_slice(&fs::read(&action_file)?).map_err(invalid_json)?;
    let action_bytes = serde_json::to_vec(&action)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let action_json = String::from_utf8(action_bytes.clone())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let swap_id = hash_hex("postfiat.asset_orchard.local_swap_id.v1", &action_bytes);

    let wallet_output_note: Value =
        serde_json::from_slice(&fs::read(&output_a)?).map_err(invalid_json)?;
    let pool_output_note: Value =
        serde_json::from_slice(&fs::read(&output_b)?).map_err(invalid_json)?;
    let wallet_output_id = wallet_note_output_commitment(&wallet_output_note)?;
    let pool_output_id = wallet_note_output_commitment(&pool_output_note)?;
    let now_ms = unix_ms()?;

    let wallet_output_record = json!({
        "schema": NOTE_VAULT_SCHEMA,
        "created_at_unix_ms": now_ms,
        "wallet_address": request.wallet_address,
        "asset_id": request.to_asset_id,
        "amount_atoms": request.liquidity_amount_atoms,
        "state": "pending_swap_output",
        "swap_id": swap_id,
        "quote_binding_hash": request.quote_binding_hash,
        "wallet_note": wallet_output_note,
    });
    let pool_owner = string_value(&pool_input, "wallet_address")
        .unwrap_or_else(|| "controlled_pool_operator".to_string());
    let pool_output_record = json!({
        "schema": NOTE_VAULT_SCHEMA,
        "created_at_unix_ms": now_ms,
        "wallet_address": pool_owner,
        "asset_id": request.from_asset_id,
        "amount_atoms": request.amount_atoms,
        "state": "pending_swap_output",
        "swap_id": swap_id,
        "quote_binding_hash": request.quote_binding_hash,
        "wallet_note": pool_output_note,
    });
    atomic_write_private_json(
        &vault_record_path(config, &wallet_output_id),
        &wallet_output_record,
    )?;
    atomic_write_private_json(
        &vault_record_path(config, &pool_output_id),
        &pool_output_record,
    )?;

    let mut wallet_locked = wallet_input.clone();
    set_record_state(
        &mut wallet_locked,
        "locked_for_swap",
        &swap_id,
        &request.quote_binding_hash,
    )?;
    atomic_write_private_json(&vault_record_path(config, &wallet_input_id), &wallet_locked)?;
    let mut pool_locked = pool_input.clone();
    set_record_state(
        &mut pool_locked,
        "locked_for_swap",
        &swap_id,
        &request.quote_binding_hash,
    )?;
    atomic_write_private_json(&vault_record_path(config, &pool_input_id), &pool_locked)?;

    let pending = json!({
        "schema": "postfiat-asset-orchard-local-swap-pending-v1",
        "created_at_unix_ms": now_ms,
        "swap_id": swap_id,
        "quote_binding_hash": request.quote_binding_hash,
        "quote_expires_at_ms": request.quote_expires_at_ms.to_string(),
        "wallet_address": request.wallet_address,
        "liquidity_wallet_address": request.liquidity_wallet_address,
        "from_asset_id": request.from_asset_id,
        "to_asset_id": request.to_asset_id,
        "amount_atoms": request.amount_atoms,
        "inputs": [
            { "role": "wallet_input", "id": wallet_input_id, "asset_id": request.from_asset_id, "amount_atoms": request.amount_atoms },
            { "role": "pool_input", "id": pool_input_id, "asset_id": request.to_asset_id, "amount_atoms": request.liquidity_amount_atoms }
        ],
        "outputs": [
            { "role": "wallet_output", "id": wallet_output_id, "asset_id": request.to_asset_id, "amount_atoms": request.liquidity_amount_atoms },
            { "role": "pool_output", "id": pool_output_id, "asset_id": request.from_asset_id, "amount_atoms": request.amount_atoms }
        ],
        "action_file": action_file.display().to_string(),
    });
    let swaps_dir = config.vault_dir.join("swaps");
    prepare_private_dir(&swaps_dir)?;
    atomic_write_private_json(&swaps_dir.join(format!("{swap_id}.json")), &pending)?;

    let response = json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-swap-action-v1",
        "request_id": request.request_id,
        "request_fingerprint": request_fingerprint,
        "swap_id": swap_id,
        "action_json": action_json,
        "action_json_bytes": action_bytes.len(),
        "verification": {
            "report_schema": report.schema,
            "pool_id": report.pool_id,
            "anchor": report.anchor,
            "nullifier_count": report.nullifiers.len(),
            "output_count": report.output_commitments.len(),
            "proof_bytes": report.proof_bytes,
            "verified": report.verified,
        },
        "swap_create": report,
        "output_note_files": [
            output_a.display().to_string(),
            output_b.display().to_string()
        ],
        "vault_update": {
            "quote_binding_hash": request.quote_binding_hash,
            "inputs": pending["inputs"].clone(),
            "outputs": pending["outputs"].clone(),
            "wallet_output_commitment": wallet_output_id,
            "pool_output_commitment": pool_output_id,
        },
        "timing": timing,
        "readiness": local_readiness(config),
    });
    atomic_write_private_json(&work_dir.join("response.json"), &response)?;
    Ok(response)
}

fn swap_action_request_fingerprint(request: &SwapActionRequest) -> io::Result<String> {
    let pricing_claim = serde_json::to_string(&request.pricing_claim).map_err(invalid_json)?;
    let preimage = match request.liquidity_wallet_address.as_deref() {
        Some(liquidity_wallet_address) => format!(
            "request_id={}\nwallet_address={}\nliquidity_wallet_address={}\nfrom_asset_id={}\nto_asset_id={}\namount_atoms={}\nwallet_commitment={}\nliquidity_amount_atoms={}\nliquidity_commitment={}\nquote_binding_hash={}\nquote_expires_at_ms={}\npricing_claim={}\ninput_note_path_a={}\ninput_note_path_b={}\n",
            request.request_id.as_deref().unwrap_or(""),
            request.wallet_address,
            liquidity_wallet_address,
            request.from_asset_id,
            request.to_asset_id,
            request.amount_atoms,
            request.wallet_commitment.as_deref().unwrap_or(""),
            request.liquidity_amount_atoms,
            request.liquidity_commitment,
            request.quote_binding_hash,
            request.quote_expires_at_ms,
            pricing_claim,
            request.input_note_path_a.as_deref().unwrap_or(""),
            request.input_note_path_b.as_deref().unwrap_or(""),
        ),
        // Preserve the exact v1 preimage for in-flight requests created before
        // liquidity ownership became an explicit request field.
        None => format!(
            "request_id={}\nwallet_address={}\nfrom_asset_id={}\nto_asset_id={}\namount_atoms={}\nwallet_commitment={}\nliquidity_amount_atoms={}\nliquidity_commitment={}\nquote_binding_hash={}\nquote_expires_at_ms={}\npricing_claim={}\ninput_note_path_a={}\ninput_note_path_b={}\n",
            request.request_id.as_deref().unwrap_or(""),
            request.wallet_address,
            request.from_asset_id,
            request.to_asset_id,
            request.amount_atoms,
            request.wallet_commitment.as_deref().unwrap_or(""),
            request.liquidity_amount_atoms,
            request.liquidity_commitment,
            request.quote_binding_hash,
            request.quote_expires_at_ms,
            pricing_claim,
            request.input_note_path_a.as_deref().unwrap_or(""),
            request.input_note_path_b.as_deref().unwrap_or(""),
        ),
    };
    Ok(hash_hex(
        "postfiat.asset_orchard.local_swap_request.v1",
        preimage.as_bytes(),
    ))
}

fn swap_input_records(config: &Config, request: &SwapActionRequest) -> io::Result<(Value, Value)> {
    match (
        request.input_note_path_a.as_deref(),
        request.input_note_path_b.as_deref(),
    ) {
        (Some(path_a), Some(path_b)) => Ok((
            local_note_record_from_path(
                config,
                "input_note_path_a",
                path_a,
                Some(&request.wallet_address),
                &request.from_asset_id,
                request.amount_atoms,
                None,
            )?,
            local_note_record_from_path(
                config,
                "input_note_path_b",
                path_b,
                request.liquidity_wallet_address.as_deref(),
                &request.to_asset_id,
                request.liquidity_amount_atoms,
                Some(&request.liquidity_commitment),
            )?,
        )),
        (None, None) => Ok((
            select_vault_note(
                config,
                Some(&request.wallet_address),
                &request.from_asset_id,
                request.amount_atoms,
                request.wallet_commitment.as_deref(),
            )?,
            select_vault_note(
                config,
                request.liquidity_wallet_address.as_deref(),
                &request.to_asset_id,
                request.liquidity_amount_atoms,
                Some(&request.liquidity_commitment),
            )?,
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input_note_path_a and input_note_path_b must be provided together",
        )),
    }
}

fn private_egress_input_record(
    config: &Config,
    request: &PrivateEgressActionRequest,
) -> io::Result<Value> {
    if let Some(path) = request.input_note_path.as_deref() {
        return local_note_record_from_path(
            config,
            "input_note_path",
            path,
            Some(&request.wallet_address),
            &request.asset_id,
            request.amount_atoms,
            request.note_commitment.as_deref(),
        );
    }
    select_vault_note(
        config,
        Some(&request.wallet_address),
        &request.asset_id,
        request.amount_atoms,
        request.note_commitment.as_deref(),
    )
}

fn local_note_record_from_path(
    config: &Config,
    field: &str,
    path: &str,
    wallet_address: Option<&str>,
    asset_id: &str,
    amount_atoms: u64,
    commitment: Option<&str>,
) -> io::Result<Value> {
    let note = read_wallet_note_file(field, path)?;
    let output_commitment =
        validate_wallet_note_file(field, path, &note, asset_id, amount_atoms, commitment)?;
    let record_path = vault_record_path(config, &output_commitment);
    if record_path.exists() {
        let record = read_vault_record(config, &output_commitment).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{field} {path} existing vault record {} cannot be used: {error}",
                    record_path.display()
                ),
            )
        })?;
        ensure_note_record_matches(
            &record,
            wallet_address,
            asset_id,
            amount_atoms,
            &output_commitment,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{field} {path} existing vault record {} cannot be used: {error}",
                    record_path.display()
                ),
            )
        })?;
        return Ok(record);
    }
    let record = json!({
        "schema": NOTE_VAULT_SCHEMA,
        "created_at_unix_ms": unix_ms()?,
        "wallet_address": wallet_address.unwrap_or("controlled_pool_operator"),
        "asset_id": asset_id,
        "amount_atoms": amount_atoms,
        "state": "spendable",
        "source_note_path": path,
        "wallet_note": note,
        "output_commitment": output_commitment,
    });
    ensure_note_record_matches(
        &record,
        wallet_address,
        asset_id,
        amount_atoms,
        &output_commitment,
    )?;
    Ok(record)
}

fn read_wallet_note_file(field: &str, path: &str) -> io::Result<Value> {
    let note_path = PathBuf::from(path);
    let bytes = fs::read(&note_path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} {path} cannot be read: {error}"),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} {path} is not valid note JSON: {error}"),
        )
    })
}

fn validate_wallet_note_file(
    field: &str,
    path: &str,
    note: &Value,
    asset_id: &str,
    amount_atoms: u64,
    commitment: Option<&str>,
) -> io::Result<String> {
    if !note.is_object() {
        return Err(invalid_note_path(
            field,
            path,
            "wallet note must be a JSON object",
        ));
    }
    if string_value(note, "schema").as_deref() != Some("postfiat-asset-orchard-wallet-note-v1") {
        return Err(invalid_note_path(
            field,
            path,
            "wallet note schema mismatch",
        ));
    }
    if string_value(note, "asset_id").as_deref() != Some(asset_id) {
        return Err(invalid_note_path(
            field,
            path,
            "wallet note asset_id mismatch",
        ));
    }
    if note.get("value").and_then(Value::as_u64) != Some(amount_atoms) {
        return Err(invalid_note_path(field, path, "wallet note value mismatch"));
    }
    let output_commitment = wallet_note_output_commitment(note)
        .map_err(|error| invalid_note_path(field, path, &error.to_string()))?;
    if let Some(commitment) = commitment {
        if output_commitment != commitment {
            return Err(invalid_note_path(
                field,
                path,
                "wallet note output_commitment mismatch",
            ));
        }
    }
    Ok(output_commitment)
}

fn invalid_note_path(field: &str, path: &str, message: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{field} {path} schema-invalid note JSON: {message}"),
    )
}

fn private_egress_timing_value() -> io::Result<Value> {
    serde_json::to_value(take_asset_orchard_private_egress_timings())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn build_swap_batch(config: &Config, request: &SwapBatchRequest) -> io::Result<Value> {
    let work_dir = config.vault_dir.join("swap-batch-work").join(format!(
        "{}-{}",
        std::process::id(),
        unix_ms()?
    ));
    prepare_private_dir(&work_dir)?;
    let swap_file = work_dir.join("swap-action.json");
    let batch_file = work_dir.join("batch.json");
    write_private_text_file(&swap_file, &request.swap_action_json)?;

    let batch = create_shielded_swap_action_batch(ShieldedSwapActionBatchOptions {
        data_dir: config.data_dir.clone(),
        swap_file: swap_file.clone(),
        batch_file: batch_file.clone(),
    })?;
    let batch_json = serde_json::to_string_pretty(&batch)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let batch_value = serde_json::to_value(&batch)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-swap-batch-v1",
        "batch": batch_value,
        "batch_json": batch_json,
        "batch_json_bytes": batch_json.len(),
        "batch_file": batch_file.display().to_string(),
        "readiness": local_readiness(config),
    }))
}

fn build_atomic_batch(config: &Config, request: &AtomicBatchRequest) -> io::Result<Value> {
    let store = NodeStore::new(&config.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let chain_tip = store.read_chain_tip()?;
    let execution_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let activation_height = governance
        .shielded_atomic_batch_activation_height()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "shielded atomic batch execution is not governed",
            )
        })?;
    if execution_height < activation_height {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "shielded atomic batch execution activates at height {activation_height}; next height is {execution_height}"
            ),
        ));
    }
    let mut actions = Vec::with_capacity(request.batches.len());
    for batch in &request.batches {
        postfiat_node::verify_shielded_action_batch_id(&genesis, batch)?;
        if batch.atomic {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "nested atomic shielded batches are not allowed",
            ));
        }
        if batch.actions.len() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "each source shielded batch must contain exactly one action",
            ));
        }
        actions.push(batch.actions[0].clone());
    }
    let batch = build_atomic_shielded_action_batch(&genesis, actions)?;
    Ok(json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-atomic-batch-v1",
        "batch": batch,
        "activation_height": activation_height,
        "execution_height": execution_height,
        "readiness": local_readiness(config),
    }))
}

fn finalize_swap(config: &Config, body: &Value) -> io::Result<Value> {
    let swap_id = string_field(body, "swap_id")?;
    let accepted = match body.get("accepted") {
        Some(Value::Bool(value)) => *value,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "accepted boolean is required",
            ))
        }
    };
    if !swap_id.bytes().all(|byte| byte.is_ascii_hexdigit()) || swap_id.len() != 96 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "swap_id must be 48-byte hex",
        ));
    }
    let pending_path = config
        .vault_dir
        .join("swaps")
        .join(format!("{swap_id}.json"));
    let pending: Value = serde_json::from_slice(&fs::read(&pending_path)?).map_err(invalid_json)?;
    let quote_binding_hash = string_field(&pending, "quote_binding_hash")?;
    let final_input_state = if accepted { "spent" } else { "spendable" };
    let final_output_state = if accepted { "spendable" } else { "failed" };

    let mut updated_inputs = Vec::new();
    for item in pending
        .get("inputs")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "pending swap missing inputs"))?
    {
        let id = string_field(item, "id")?;
        let mut record = read_vault_record(config, &id)?;
        set_record_state(
            &mut record,
            final_input_state,
            &swap_id,
            &quote_binding_hash,
        )?;
        atomic_write_private_json(&vault_record_path(config, &id), &record)?;
        updated_inputs.push(public_note_record(&record)?);
    }

    let mut updated_outputs = Vec::new();
    for item in pending
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "pending swap missing outputs"))?
    {
        let id = string_field(item, "id")?;
        let mut record = read_vault_record(config, &id)?;
        set_record_state(
            &mut record,
            final_output_state,
            &swap_id,
            &quote_binding_hash,
        )?;
        atomic_write_private_json(&vault_record_path(config, &id), &record)?;
        updated_outputs.push(public_note_record(&record)?);
    }

    let mut finalized = pending.clone();
    set_record_state(
        &mut finalized,
        if accepted { "certified" } else { "failed" },
        &swap_id,
        &quote_binding_hash,
    )?;
    atomic_write_private_json(&pending_path, &finalized)?;
    Ok(json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-swap-finalize-v1",
        "swap_id": swap_id,
        "accepted": accepted,
        "inputs": updated_inputs,
        "outputs": updated_outputs,
    }))
}

fn build_and_store_private_egress_action(
    config: &Config,
    request: &PrivateEgressActionRequest,
) -> io::Result<Value> {
    let input = private_egress_input_record(config, request)?;
    let input_id = note_record_id(&input)?;

    let work_dir =
        config
            .vault_dir
            .join("egress-work")
            .join(format!("{}-{}", std::process::id(), unix_ms()?));
    prepare_private_dir(&work_dir)?;
    let note_file = match string_value(&input, "source_note_path") {
        Some(path) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() || !path.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "private egress source note handle is unavailable",
                ));
            }
            path
        }
        None => {
            let path = work_dir.join("input-note.json");
            atomic_write_private_json(&path, wallet_note_value(&input)?)?;
            path
        }
    };
    let egress_file = work_dir.join("private-egress.json");

    reset_asset_orchard_private_egress_timings();
    let report = match create_asset_orchard_private_egress(AssetOrchardPrivateEgressCreateOptions {
        data_dir: config.data_dir.clone(),
        note_file: note_file.clone(),
        to: request.to.clone(),
        asset_id: Some(request.asset_id.clone()),
        amount: Some(request.amount_atoms),
        fee: 0,
        policy_id: request.policy_id.clone(),
        disclosure_hash: request.disclosure_hash.clone(),
        pending_output_commitments: request.pending_output_commitments.clone(),
        egress_file: egress_file.clone(),
        overwrite: true,
    }) {
        Ok(report) => report,
        Err(error) => {
            let _ = take_asset_orchard_private_egress_timings();
            return Err(error);
        }
    };
    let timing = private_egress_timing_value()?;
    let egress_bytes = fs::read(&egress_file)?;
    let egress: Value = serde_json::from_slice(&egress_bytes).map_err(invalid_json)?;
    let egress_json = String::from_utf8(egress_bytes.clone())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let egress_id = hash_hex(
        "postfiat.asset_orchard.local_private_egress_id.v1",
        &egress_bytes,
    );
    let now_ms = unix_ms()?;

    let pending = json!({
        "schema": "postfiat-asset-orchard-local-private-egress-pending-v1",
        "created_at_unix_ms": now_ms,
        "egress_id": egress_id,
        "wallet_address": request.wallet_address,
        "to": request.to,
        "asset_id": request.asset_id,
        "amount_atoms": request.amount_atoms,
        "policy_id": request.policy_id,
        "disclosure_hash": request.disclosure_hash,
        "input": {
            "id": input_id,
            "asset_id": request.asset_id,
            "amount_atoms": request.amount_atoms
        },
        "egress_file": egress_file.display().to_string(),
    });
    let egresses_dir = config.vault_dir.join("egresses");
    prepare_private_dir(&egresses_dir)?;
    atomic_write_private_json(&egresses_dir.join(format!("{egress_id}.json")), &pending)?;

    let mut locked = input.clone();
    set_record_egress_state(
        &mut locked,
        "locked_for_egress",
        &egress_id,
        &request.disclosure_hash,
    )?;
    atomic_write_private_json(&vault_record_path(config, &input_id), &locked)?;

    Ok(json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-private-egress-action-v1",
        "egress_id": egress_id,
        "egress_json": egress_json,
        "egress_json_bytes": egress_bytes.len(),
        "verification": {
            "report_schema": report.schema,
            "pool_id": report.pool_id,
            "to": report.to,
            "asset_id": report.asset_id,
            "amount_atoms": report.amount.to_string(),
            "fee": report.fee.to_string(),
            "policy_id": report.policy_id,
            "disclosure_hash": report.disclosure_hash,
            "anchor": report.anchor,
            "nullifier": report.nullifier,
            "exit_binding_hash": report.exit_binding_hash,
            "proof_bytes": report.proof_bytes,
            "verified": report.verified,
            "privacy": report.privacy
        },
        "vault_update": {
            "input": pending["input"].clone(),
            "state": "locked_for_egress",
            "egress_id": egress_id,
            "disclosure_hash": request.disclosure_hash
        },
        "egress": egress,
        "private_egress_report": report,
        "note_file": note_file.display().to_string(),
        "timing": timing,
        "readiness": local_readiness(config),
    }))
}

fn finalize_private_egress(config: &Config, body: &Value) -> io::Result<Value> {
    let egress_id = string_field(body, "egress_id")?;
    let accepted = match body.get("accepted") {
        Some(Value::Bool(value)) => *value,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "accepted boolean is required",
            ))
        }
    };
    if !egress_id.bytes().all(|byte| byte.is_ascii_hexdigit()) || egress_id.len() != 96 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "egress_id must be 48-byte hex",
        ));
    }
    let pending_path = config
        .vault_dir
        .join("egresses")
        .join(format!("{egress_id}.json"));
    let pending: Value = serde_json::from_slice(&fs::read(&pending_path)?).map_err(invalid_json)?;
    let disclosure_hash = string_field(&pending, "disclosure_hash")?;
    let input = pending
        .get("input")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "pending egress missing input")
        })?;
    let id = input.get("id").and_then(Value::as_str).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "pending egress input id missing",
        )
    })?;

    let mut record = read_vault_record(config, id)?;
    set_record_egress_state(
        &mut record,
        if accepted { "egressed" } else { "spendable" },
        &egress_id,
        &disclosure_hash,
    )?;
    atomic_write_private_json(&vault_record_path(config, id), &record)?;

    let mut finalized = pending.clone();
    set_record_egress_state(
        &mut finalized,
        if accepted { "certified" } else { "failed" },
        &egress_id,
        &disclosure_hash,
    )?;
    atomic_write_private_json(&pending_path, &finalized)?;
    Ok(json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-private-egress-finalize-v1",
        "egress_id": egress_id,
        "accepted": accepted,
        "input": public_note_record(&record)?,
    }))
}

fn random_seed() -> io::Result<[u8; 32]> {
    random_bytes::<32>()
}

fn random_bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut file = fs::File::open("/dev/urandom")?;
    let mut bytes = [0u8; N];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn parse_ingress_note_request(body: &Value) -> io::Result<IngressNoteRequest> {
    let wallet_address = string_field(body, "wallet_address")?;
    let asset_id = string_field(body, "asset_id")?.to_ascii_lowercase();
    if asset_id.len() != 96 || !asset_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "asset_id must be 48-byte hex",
        ));
    }
    let amount_atoms = match body.get("amount_atoms") {
        Some(Value::String(value)) => value
            .parse::<u64>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "amount_atoms must be u64"))?,
        Some(Value::Number(value)) => value.as_u64().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "amount_atoms must be u64")
        })?,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "amount_atoms is required",
            ))
        }
    };
    if amount_atoms == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "amount_atoms must be nonzero",
        ));
    }
    Ok(IngressNoteRequest {
        wallet_address,
        asset_id,
        amount_atoms,
    })
}

fn parse_swap_action_request(body: &Value) -> io::Result<SwapActionRequest> {
    let request_id = body
        .get("request_id")
        .map(|_| request_id_field(body))
        .transpose()?;
    let wallet_address = string_field(body, "wallet_address")?;
    let liquidity_wallet_address = body
        .get("liquidity_wallet_address")
        .map(|_| string_field(body, "liquidity_wallet_address"))
        .transpose()?;
    let from_asset_id = hex_field(body, "from_asset_id", 96)?;
    let to_asset_id = hex_field(body, "to_asset_id", 96)?;
    let amount_atoms = u64_field(body, "amount_atoms")?;
    let wallet_commitment = body
        .get("wallet_commitment")
        .map(|_| hex_field(body, "wallet_commitment", 64))
        .transpose()?;
    let liquidity_amount_atoms = match body.get("liquidity_amount_atoms") {
        Some(_) => u64_field(body, "liquidity_amount_atoms")?,
        None => amount_atoms,
    };
    let liquidity_commitment = hex_field(body, "liquidity_commitment", 64)?;
    let quote_binding_hash = hex_field(body, "quote_binding_hash", 64)?;
    let quote_expires_at_ms = u128_field(body, "quote_expires_at_ms")?;
    let pricing_claim: AssetOrchardPricingClaim = serde_json::from_value(
        body.get("pricing_claim")
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing pricing_claim"))?,
    )
    .map_err(invalid_json)?;
    pricing_claim
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    let input_note_path_a = string_value(body, "input_note_path_a");
    let input_note_path_b = string_value(body, "input_note_path_b");
    if from_asset_id == to_asset_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "from_asset_id and to_asset_id must differ",
        ));
    }
    if liquidity_amount_atoms == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "liquidity_amount_atoms must be nonzero",
        ));
    }
    Ok(SwapActionRequest {
        request_id,
        wallet_address,
        liquidity_wallet_address,
        from_asset_id,
        to_asset_id,
        amount_atoms,
        wallet_commitment,
        liquidity_amount_atoms,
        liquidity_commitment,
        quote_binding_hash,
        quote_expires_at_ms,
        pricing_claim,
        input_note_path_a,
        input_note_path_b,
    })
}

fn parse_swap_batch_request(body: &Value) -> io::Result<SwapBatchRequest> {
    let swap_action_json = string_field(body, "swap_action_json")?;
    if swap_action_json.len() > 8 * 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "swap_action_json is too large",
        ));
    }
    let parsed: Value = serde_json::from_str(&swap_action_json).map_err(invalid_json)?;
    if let Some(path) = find_forbidden_swap_action_private_material(&parsed, "$") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("swap action contains forbidden private material at {path}"),
        ));
    }
    Ok(SwapBatchRequest { swap_action_json })
}

fn parse_atomic_batch_request(body: &Value) -> io::Result<AtomicBatchRequest> {
    let batches = body
        .get("batches")
        .and_then(Value::as_array)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "batches array is required"))?;
    if batches.is_empty() || batches.len() > 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "batches must contain between one and three source batches",
        ));
    }
    let batches = batches
        .iter()
        .cloned()
        .map(|value| serde_json::from_value(value).map_err(invalid_json))
        .collect::<io::Result<Vec<ShieldedActionBatch>>>()?;
    Ok(AtomicBatchRequest { batches })
}

fn parse_private_egress_action_request(body: &Value) -> io::Result<PrivateEgressActionRequest> {
    if body.get("disclosure_ack").and_then(Value::as_bool) != Some(true) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "disclosure_ack=true is required before private egress",
        ));
    }
    let wallet_address = string_field(body, "wallet_address")?;
    let to = string_value(body, "to")
        .or_else(|| string_value(body, "destination"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "to is required"))?;
    let asset_id = hex_field(body, "asset_id", 96)?;
    let amount_atoms = u64_field(body, "amount_atoms")?;
    let note_commitment = string_value(body, "note_commitment")
        .or_else(|| string_value(body, "output_commitment"))
        .map(|value| value.to_ascii_lowercase())
        .map(|value| {
            if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                Ok(value)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "note_commitment must be 32-byte hex",
                ))
            }
        })
        .transpose()?;
    let input_note_path = string_value(body, "input_note_path");
    let policy_id = string_field(body, "policy_id")?;
    let disclosure_hash = hex_field(body, "disclosure_hash", 64)?;
    Ok(PrivateEgressActionRequest {
        wallet_address,
        to,
        asset_id,
        amount_atoms,
        note_commitment,
        input_note_path,
        policy_id,
        disclosure_hash,
        pending_output_commitments: pending_output_commitments(body)?,
    })
}

fn parse_private_primary_issue_action_request(
    body: &Value,
) -> io::Result<PrivatePrimaryIssueActionRequest> {
    Ok(PrivatePrimaryIssueActionRequest {
        request_id: request_id_field(body)?,
        input_note_path: string_field(body, "input_note_path")?,
        route_id: string_field(body, "route_id")?,
        subscriber: string_field(body, "subscriber")?,
        ethereum_recipient: string_field(body, "ethereum_recipient")?,
        reservation_id: hex_field(body, "reservation_id", 96)?,
        subscription_nonce: hex_field(body, "subscription_nonce", 64)?,
        mint_amount_atoms: u64_field(body, "mint_amount_atoms")?,
        settlement_value_atoms: u64_field(body, "settlement_value_atoms")?,
        expires_at_height: u64_field(body, "expires_at_height")?,
        pending_output_commitments: pending_output_commitments(body)?,
    })
}

fn parse_private_primary_redeem_action_request(
    body: &Value,
) -> io::Result<PrivatePrimaryRedeemActionRequest> {
    Ok(PrivatePrimaryRedeemActionRequest {
        request_id: request_id_field(body)?,
        input_note_path: string_field(body, "input_note_path")?,
        route_id: string_field(body, "route_id")?,
        owner: string_field(body, "owner")?,
        settlement_recipient: string_field(body, "settlement_recipient")?,
        nav_amount_atoms: u64_field(body, "nav_amount_atoms")?,
        settlement_output_atoms: u64_field(body, "settlement_output_atoms")?,
        expires_at_height: u64_field(body, "expires_at_height")?,
        pending_output_commitments: pending_output_commitments(body)?,
    })
}

fn pending_output_commitments(body: &Value) -> io::Result<Vec<String>> {
    let Some(values) = body.get("pending_output_commitments") else {
        return Ok(Vec::new());
    };
    let values = values.as_array().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "pending_output_commitments must be an array",
        )
    })?;
    if values.len() > 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pending_output_commitments supports at most two values",
        ));
    }
    values
        .iter()
        .map(|value| {
            let value = value.as_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "pending output commitment must be a string",
                )
            })?;
            if value.len() != 64
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "pending output commitment must be 32-byte lowercase hex",
                ));
            }
            Ok(value.to_string())
        })
        .collect()
}

fn request_id_field(body: &Value) -> io::Result<String> {
    let value = string_field(body, "request_id")?;
    if value.len() <= 64
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || (index > 0 && byte == b'-')
        })
    {
        Ok(value)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "request_id must match [a-z0-9][a-z0-9-]{0,63}",
        ))
    }
}

fn string_field(body: &Value, field: &str) -> io::Result<String> {
    body.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("{field} is required")))
}

fn string_value(body: &Value, field: &str) -> Option<String> {
    body.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn hex_field(body: &Value, field: &str, len: usize) -> io::Result<String> {
    let value = string_field(body, field)?.to_ascii_lowercase();
    if value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(value)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} must be {len} lowercase hex characters"),
        ))
    }
}

fn u64_field(body: &Value, field: &str) -> io::Result<u64> {
    let value = match body.get(field) {
        Some(Value::String(value)) => value.parse::<u64>().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("{field} must be u64"))
        })?,
        Some(Value::Number(value)) => value.as_u64().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("{field} must be u64"))
        })?,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{field} is required"),
            ))
        }
    };
    if value == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} must be nonzero"),
        ));
    }
    Ok(value)
}

fn u128_field(body: &Value, field: &str) -> io::Result<u128> {
    match body.get(field) {
        Some(Value::String(value)) => value.parse::<u128>().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("{field} must be u128"))
        }),
        Some(Value::Number(value)) => value.as_u64().map(u128::from).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, format!("{field} must be u128"))
        }),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} is required"),
        )),
    }
}

fn vault_record_path(config: &Config, id: &str) -> PathBuf {
    config.vault_dir.join(format!("{id}.json"))
}

fn read_vault_record(config: &Config, id: &str) -> io::Result<Value> {
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "note id must be 32-byte hex",
        ));
    }
    serde_json::from_slice(&fs::read(vault_record_path(config, id))?).map_err(invalid_json)
}

fn select_vault_note(
    config: &Config,
    wallet_address: Option<&str>,
    asset_id: &str,
    amount_atoms: u64,
    commitment: Option<&str>,
) -> io::Result<Value> {
    if let Some(commitment) = commitment {
        let record = read_vault_record(config, commitment)?;
        ensure_note_record_matches(&record, wallet_address, asset_id, amount_atoms, commitment)?;
        return Ok(record);
    }
    let mut entries = fs::read_dir(&config.vault_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    entries.sort();
    let mut candidates = Vec::new();
    for path in entries {
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let Some(id) = string_value(&record, "asset_id") else {
            continue;
        };
        if id != asset_id {
            continue;
        }
        if record.get("amount_atoms").and_then(Value::as_u64) != Some(amount_atoms) {
            continue;
        }
        if let Some(wallet_address) = wallet_address {
            if string_value(&record, "wallet_address").as_deref() != Some(wallet_address) {
                continue;
            }
        }
        if matches!(
            string_value(&record, "state").as_deref(),
            Some("spent" | "locked_for_swap" | "locked_for_egress" | "egressed" | "failed")
        ) {
            continue;
        }
        let output_commitment = note_record_id(&record)?;
        ensure_note_record_matches(
            &record,
            wallet_address,
            asset_id,
            amount_atoms,
            &output_commitment,
        )?;
        candidates.push((record_created_at_ms(&record), path, record));
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    if let Some((_, _, record)) = candidates.into_iter().next() {
        return Ok(record);
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("no spendable local note for asset {asset_id} amount {amount_atoms}"),
    ))
}

fn record_created_at_ms(record: &Value) -> u128 {
    record
        .get("created_at_unix_ms")
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64().map(u128::from),
            Value::String(text) => text.parse::<u128>().ok(),
            _ => None,
        })
        .unwrap_or(0)
}

fn ensure_note_record_matches(
    record: &Value,
    wallet_address: Option<&str>,
    asset_id: &str,
    amount_atoms: u64,
    commitment: &str,
) -> io::Result<()> {
    if string_value(record, "schema").as_deref() != Some(NOTE_VAULT_SCHEMA) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local note record schema mismatch",
        ));
    }
    if string_value(record, "asset_id").as_deref() != Some(asset_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local note asset_id mismatch",
        ));
    }
    if record.get("amount_atoms").and_then(Value::as_u64) != Some(amount_atoms) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local note amount mismatch",
        ));
    }
    if let Some(wallet_address) = wallet_address {
        if string_value(record, "wallet_address").as_deref() != Some(wallet_address) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "local note wallet owner mismatch",
            ));
        }
    }
    if matches!(
        string_value(record, "state").as_deref(),
        Some("spent" | "locked_for_swap" | "locked_for_egress" | "egressed" | "failed")
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local note is not spendable",
        ));
    }
    let note_commitment = note_record_id(record)?;
    if note_commitment != commitment {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "local note commitment mismatch",
        ));
    }
    Ok(())
}

fn note_record_id(record: &Value) -> io::Result<String> {
    let note = wallet_note_value(record)?;
    wallet_note_output_commitment(note)
}

fn wallet_note_value(record: &Value) -> io::Result<&Value> {
    record.get("wallet_note").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "local record missing wallet_note",
        )
    })
}

fn wallet_note_output_commitment(note: &Value) -> io::Result<String> {
    let output = string_field(note, "output_commitment")?;
    if output.len() == 64 && output.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(output)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet note output commitment must be 32-byte hex",
        ))
    }
}

fn set_record_state(
    record: &mut Value,
    state: &str,
    swap_id: &str,
    quote_binding_hash: &str,
) -> io::Result<()> {
    let Some(map) = record.as_object_mut() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "record must be an object",
        ));
    };
    map.insert("state".to_string(), Value::String(state.to_string()));
    map.insert(
        "updated_at_unix_ms".to_string(),
        Value::String(unix_ms()?.to_string()),
    );
    map.insert("swap_id".to_string(), Value::String(swap_id.to_string()));
    map.insert(
        "quote_binding_hash".to_string(),
        Value::String(quote_binding_hash.to_string()),
    );
    Ok(())
}

fn set_record_egress_state(
    record: &mut Value,
    state: &str,
    egress_id: &str,
    disclosure_hash: &str,
) -> io::Result<()> {
    let Some(map) = record.as_object_mut() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "record must be an object",
        ));
    };
    map.insert("state".to_string(), Value::String(state.to_string()));
    map.insert(
        "updated_at_unix_ms".to_string(),
        Value::String(unix_ms()?.to_string()),
    );
    map.insert(
        "egress_id".to_string(),
        Value::String(egress_id.to_string()),
    );
    map.insert(
        "disclosure_hash".to_string(),
        Value::String(disclosure_hash.to_string()),
    );
    Ok(())
}

fn public_note_record(record: &Value) -> io::Result<Value> {
    Ok(json!({
        "id": note_record_id(record)?,
        "wallet_address": string_value(record, "wallet_address"),
        "asset_id": string_value(record, "asset_id"),
        "amount_atoms": record.get("amount_atoms").and_then(Value::as_u64),
        "state": string_value(record, "state"),
        "swap_id": string_value(record, "swap_id"),
        "quote_binding_hash": string_value(record, "quote_binding_hash"),
        "egress_id": string_value(record, "egress_id"),
        "disclosure_hash": string_value(record, "disclosure_hash"),
    }))
}

fn list_public_notes(config: &Config) -> io::Result<Value> {
    let mut notes = Vec::new();
    let mut entries = fs::read_dir(&config.vault_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        if string_value(&record, "schema").as_deref() != Some(NOTE_VAULT_SCHEMA) {
            continue;
        }
        if let Ok(public) = public_note_record(&record) {
            notes.push(public);
        }
    }
    Ok(json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-note-list-v1",
        "notes": notes,
        "readiness": local_readiness(config),
    }))
}

fn local_readiness(config: &Config) -> Value {
    let mirror = match NodeStore::new(&config.data_dir).read_chain_tip() {
        Ok(tip) => json!({
            "height": tip.height,
            "state_root": tip.state_root,
        }),
        Err(error) => json!({
            "height": null,
            "state_root": null,
            "error": error.kind().to_string(),
        }),
    };
    let mirror_ready = mirror.get("height").and_then(Value::as_u64).is_some()
        && mirror.get("state_root").and_then(Value::as_str).is_some();
    let prover_warm = prover_warm_snapshot(config);
    let prover_ready = prover_warm.get("ready").and_then(Value::as_bool) == Some(true)
        || prover_warm.get("enabled").and_then(Value::as_bool) == Some(false);
    let prover_active = PROVER_ACTIVE.load(Ordering::Acquire);
    let ready = mirror_ready && prover_ready;
    json!({
        "ok": ready,
        "ready": ready,
        "local_only": true,
        "service": "asset-orchard-local-service",
        "bind": config.bind.to_string(),
        "product_profile_sha256": config.product_profile_sha256,
        "mirror": mirror,
        "pool_id": "asset-orchard-v1",
        "circuit_id": "asset-orchard-swap-v1",
        "k": 15,
        "vault_schema": NOTE_VAULT_SCHEMA,
        "capabilities": {
            "private_primary_proof_timing_schema":
                "postfiat.asset_orchard.private_primary_proof_timing.v1",
            "private_primary_proof_schedule":
                "output_validity_then_outer_primary",
        },
        "prover_warm": prover_warm,
        "prover_capacity": {
            "active": prover_active,
            "limit": 1,
            "available": !prover_active,
        },
        "connections": {
            "active": ACTIVE_CONNECTIONS.load(Ordering::Acquire),
            "limit": MAX_CONNECTIONS,
        },
        "operations": {
            "ingress_notes": "/asset-orchard/ingress-notes",
            "swap_actions": "/asset-orchard/swap-actions",
            "swap_batch": "/asset-orchard/swap-batch",
            "atomic_batch": "/asset-orchard/atomic-batch",
            "private_primary_issue_actions": "/asset-orchard/private-primary-issue-actions",
            "private_primary_redeem_actions": "/asset-orchard/private-primary-redeem-actions",
            "private_egress_actions": "/asset-orchard/private-egress-actions",
            "private_egress_finalize": "/asset-orchard/private-egress-finalize",
            "notes": "/asset-orchard/notes"
        },
    })
}

fn find_forbidden_private_material(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                if forbidden_key(key) {
                    return Some(child_path);
                }
                if let Some(hit) = find_forbidden_private_material(child, &child_path) {
                    return Some(hit);
                }
            }
            None
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                if let Some(hit) =
                    find_forbidden_private_material(child, &format!("{path}[{index}]"))
                {
                    return Some(hit);
                }
            }
            None
        }
        _ => None,
    }
}

fn forbidden_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "backup"
            | "backup_json"
            | "decrypted_backup"
            | "key_file"
            | "note_file"
            | "note_files"
            | "note_opening"
            | "note_openings"
            | "passphrase"
            | "private_key"
            | "secret_key"
            | "seed"
            | "seed_hex"
            | "seed_phrase"
            | "spend_authority"
            | "spend_authorization_key"
            | "spend_key"
            | "spending_key"
    ) || (normalized.starts_with("spend_") && normalized != "spend_authorization_signature")
}

fn find_forbidden_swap_action_private_material(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                if forbidden_swap_action_key(key) {
                    return Some(child_path);
                }
                if let Some(hit) = find_forbidden_swap_action_private_material(child, &child_path) {
                    return Some(hit);
                }
            }
            None
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                if let Some(hit) =
                    find_forbidden_swap_action_private_material(child, &format!("{path}[{index}]"))
                {
                    return Some(hit);
                }
            }
            None
        }
        _ => None,
    }
}

fn forbidden_swap_action_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "backup"
            | "backup_json"
            | "decrypted_backup"
            | "input_note"
            | "input_notes"
            | "key_file"
            | "memo"
            | "merkle_path"
            | "note"
            | "note_file"
            | "note_files"
            | "note_opening"
            | "note_openings"
            | "output_note"
            | "output_notes"
            | "passphrase"
            | "private_key"
            | "rho"
            | "rseed"
            | "secret_key"
            | "seed"
            | "seed_hex"
            | "seed_phrase"
            | "spend_authority"
            | "spend_authorization_key"
            | "spend_key"
            | "spending_key"
    )
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn parse_http_request(buffer: &[u8]) -> io::Result<HttpRequest> {
    let header_end = find_header_end(buffer)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "incomplete HTTP request"))?;
    let header = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid HTTP header"))?;
    let mut lines = header.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing request line"))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or_default().to_string();
    let path = request_parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid content-length")
                })?;
            }
        }
    }
    if content_length > MAX_BODY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "request body too large",
        ));
    }
    let body_start = header_end + 4;
    if buffer.len() < body_start + content_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "incomplete HTTP request body",
        ));
    }
    Ok(HttpRequest {
        method,
        path,
        body: buffer[body_start..body_start + content_length].to_vec(),
    })
}

fn request_complete(buffer: &[u8]) -> io::Result<bool> {
    let Some(header_end) = find_header_end(buffer) else {
        return Ok(false);
    };
    let header = std::str::from_utf8(&buffer[..header_end])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid HTTP header"))?;
    let mut content_length = 0usize;
    for line in header.split("\r\n").skip(1) {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid content-length")
                })?;
            }
        }
    }
    Ok(buffer.len() >= header_end + 4 + content_length)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn invalid_json(error: serde_json::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("invalid JSON: {error}"),
    )
}

fn error_response(error: &io::Error) -> Value {
    let code = match error.kind() {
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => "invalid_request",
        io::ErrorKind::NotFound => "not_found",
        io::ErrorKind::AlreadyExists => "conflict",
        io::ErrorKind::PermissionDenied => "forbidden",
        io::ErrorKind::WouldBlock => "temporarily_unavailable",
        io::ErrorKind::StorageFull => "capacity_exhausted",
        _ => "internal_error",
    };
    json!({
        "ok": false,
        "error": code,
    })
}

fn prepare_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    set_private_dir_permissions(path)
}

fn atomic_write_private_json(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        prepare_private_dir(parent)?;
    }
    let temp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        set_private_file_permissions(&temp)?;
        let json = serde_json::to_vec_pretty(value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        file.write_all(&json)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(temp, path)?;
    set_private_file_permissions(path)
}

fn write_private_text_file(path: &Path, value: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        prepare_private_dir(parent)?;
    }
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    set_private_file_permissions(path)?;
    file.write_all(value.trim().as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn unix_ms() -> io::Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_millis())
}

fn write_json_response(stream: &mut TcpStream, status: u16, body: &Value) -> io::Result<()> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let body = serde_json::to_vec(body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write!(
        stream,
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Private-Network: true\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_prewarm_state_is_machine_readable_and_terminal() {
        let state = PrewarmState::new(
            false,
            123,
            PathBuf::from("/tmp/postfiat-prewarm-ready.json"),
        );
        let value = state.to_json();

        assert_eq!(value["schema"], PREWARM_READY_SCHEMA);
        assert_eq!(value["enabled"], false);
        assert_eq!(value["ready"], false);
        assert_eq!(value["status"], "disabled");
        assert_eq!(value["circuits"]["swap"]["status"], "disabled");
        assert_eq!(value["circuits"]["private_egress"]["status"], "disabled");
        assert_eq!(
            value["circuits"]["ingress_notes"]["status"],
            "not_applicable"
        );
        assert_eq!(value["disk_pk_vk_cache"]["supported"], false);
        assert!(state.is_terminal());
    }

    #[test]
    fn ready_prewarm_state_exposes_prover_warm_capability() {
        let mut state =
            PrewarmState::new(true, 123, PathBuf::from("/tmp/postfiat-prewarm-ready.json"));
        state.swap.status = "ready";
        state.swap.k = Some(15);
        state.swap.params_hash = Some("swap-params".to_string());
        state.swap.vk_hash = Some("swap-vk".to_string());
        state.private_egress.status = "ready";
        state.private_egress.k = Some(15);
        state.private_egress.params_hash = Some("egress-params".to_string());
        state.private_egress.vk_hash = Some("egress-vk".to_string());
        state.recompute_status(Some(456));

        let value = state.to_json();
        assert_eq!(value["ready"], true);
        assert_eq!(value["status"], "ready");
        assert_eq!(value["circuits"]["swap"]["ready"], true);
        assert_eq!(value["circuits"]["private_egress"]["ready"], true);
        assert_eq!(
            value["threading"]["halo2_multicore_feature"],
            "explicitly_enabled"
        );
    }

    #[test]
    fn selective_prewarm_can_ready_only_the_private_primary_key() {
        let mut state = PrewarmState::new_with_circuits(
            true,
            false,
            true,
            123,
            PathBuf::from("/tmp/postfiat-prewarm-ready.json"),
        );
        assert_eq!(state.status, "warming");
        assert_eq!(state.swap.status, "not_applicable");
        assert_eq!(state.private_egress.status, "warming");

        state.private_egress.status = "ready";
        state.private_egress.k = Some(15);
        state.recompute_status(Some(456));
        let value = state.to_json();
        assert_eq!(value["ready"], true);
        assert_eq!(value["circuits"]["swap"]["ready"], true);
        assert_eq!(value["circuits"]["private_egress"]["ready"], true);
    }

    #[test]
    fn private_primary_requests_are_public_only_and_path_safe() {
        let issue = parse_private_primary_issue_action_request(&json!({
            "request_id": "run-42-primary-issue",
            "input_note_path": "/var/lib/postfiat/private/input-note.json",
            "route_id": "route-v1",
            "subscriber": "pfsubscriber",
            "ethereum_recipient": "0x1111111111111111111111111111111111111111",
            "reservation_id": "a".repeat(96),
            "subscription_nonce": "b".repeat(64),
            "mint_amount_atoms": "1000000",
            "settlement_value_atoms": "905538",
            "expires_at_height": "900",
        }))
        .unwrap();
        assert_eq!(issue.request_id, "run-42-primary-issue");
        assert_eq!(issue.mint_amount_atoms, 1_000_000);

        let redeem = parse_private_primary_redeem_action_request(&json!({
            "request_id": "run-42-primary-redeem",
            "input_note_path": "/var/lib/postfiat/private/input-note.json",
            "route_id": "route-v1",
            "owner": "pfowner",
            "settlement_recipient": "pfrecipient",
            "nav_amount_atoms": 1000000,
            "settlement_output_atoms": 900580,
            "expires_at_height": 901,
        }))
        .unwrap();
        assert_eq!(redeem.request_id, "run-42-primary-redeem");
        assert_eq!(redeem.settlement_output_atoms, 900_580);

        let unsafe_id = json!({
            "request_id": "../escape",
            "input_note_path": "/var/lib/postfiat/private/input-note.json",
            "route_id": "route-v1",
            "owner": "pfowner",
            "settlement_recipient": "pfrecipient",
            "nav_amount_atoms": 1,
            "settlement_output_atoms": 1,
            "expires_at_height": 2,
        });
        assert!(parse_private_primary_redeem_action_request(&unsafe_id)
            .unwrap_err()
            .to_string()
            .contains("request_id must match"));
    }

    #[test]
    fn private_primary_cached_response_is_idempotent() {
        let root = asset_orchard_local_service_test_dir("primary_idempotent");
        let config = asset_orchard_local_service_test_config(&root);
        let work_dir = private_primary_work_dir(&config, "issue", "cached-issue");
        prepare_private_dir(&work_dir).unwrap();
        let expected = json!({
            "ok": true,
            "schema": "postfiat-asset-orchard-local-private-primary-issue-action-v1",
            "request_id": "cached-issue",
        });
        atomic_write_private_json(&work_dir.join("response.json"), &expected).unwrap();

        let (_, cached) =
            cached_or_prepare_private_primary_work_dir(&config, "issue", "cached-issue").unwrap();
        assert_eq!(cached, Some(expected));

        fs::remove_dir_all(root).expect("cleanup private-primary idempotency test");
    }

    #[test]
    fn swap_batch_request_allows_spend_authorization_signatures_but_not_note_openings() {
        let request = json!({
            "swap_action_json": serde_json::to_string(&json!({
                "schema": "postfiat-asset-orchard-swap-action-v1",
                "pool_id": "asset-orchard-v1",
                "spend_authorization_signatures": ["aa", "bb"]
            })).unwrap(),
        });
        let parsed = parse_swap_batch_request(&request).unwrap();
        assert!(parsed
            .swap_action_json
            .contains("spend_authorization_signatures"));

        let rejected = json!({
            "swap_action_json": serde_json::to_string(&json!({
                "schema": "postfiat-asset-orchard-swap-action-v1",
                "pool_id": "asset-orchard-v1",
                "note_opening": "secret"
            })).unwrap(),
        });
        let error = parse_swap_batch_request(&rejected).unwrap_err();
        assert!(error
            .to_string()
            .contains("forbidden private material at $.note_opening"));
    }

    #[test]
    fn atomic_batch_request_is_bounded_and_preserves_source_actions() {
        let source = ShieldedActionBatch::new(
            "source-batch",
            vec![postfiat_types::ShieldedAction::Mint(
                postfiat_types::ShieldMintAction {
                    owner: "historical-owner".to_string(),
                    asset_id: "asset".to_string(),
                    amount: 1,
                    memo: String::new(),
                },
            )],
        );
        let parsed = parse_atomic_batch_request(&json!({
            "batches": [source.clone()],
        }))
        .expect("parse bounded atomic batch request");
        assert_eq!(parsed.batches.len(), 1);
        assert_eq!(parsed.batches[0].actions.len(), 1);

        let empty = parse_atomic_batch_request(&json!({ "batches": [] }))
            .expect_err("empty atomic request must fail");
        assert!(empty.to_string().contains("between one and three"));
        let too_many = parse_atomic_batch_request(&json!({
            "batches": [source.clone(), source.clone(), source.clone(), source],
        }))
        .expect_err("oversized atomic request must fail");
        assert!(too_many.to_string().contains("between one and three"));
    }

    #[test]
    fn pending_output_commitments_parser_is_canonical_and_bounded() {
        let parsed = pending_output_commitments(&json!({
            "pending_output_commitments": ["11".repeat(32), "22".repeat(32)],
        }))
        .expect("parse bounded pending commitments");
        assert_eq!(parsed, vec!["11".repeat(32), "22".repeat(32)]);

        let absent = pending_output_commitments(&json!({}))
            .expect("missing pending commitments defaults empty");
        assert!(absent.is_empty());

        let oversized = pending_output_commitments(&json!({
            "pending_output_commitments": [
                "11".repeat(32),
                "22".repeat(32),
                "33".repeat(32),
            ],
        }))
        .expect_err("oversized pending commitments must fail");
        assert!(oversized.to_string().contains("at most two"));

        let uppercase = pending_output_commitments(&json!({
            "pending_output_commitments": ["AA".repeat(32)],
        }))
        .expect_err("uppercase commitment must fail");
        assert!(uppercase.to_string().contains("lowercase hex"));
    }

    fn asset_orchard_local_service_test_dir(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "asset_orchard_local_service_{name}_{}_{}",
            std::process::id(),
            unix_ms().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn asset_orchard_local_service_test_config(root: &Path) -> Config {
        let data_dir = root.join("data");
        let vault_dir = root.join("vault");
        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&vault_dir).unwrap();
        Config {
            bind: "127.0.0.1:0".parse().unwrap(),
            data_dir,
            logical_data_dir: None,
            vault_dir,
            prewarm_ready_file: root.join("prewarm-ready.json"),
            product_profile_sha256: "a".repeat(64),
        }
    }

    #[test]
    fn logical_data_dir_maps_private_note_handles_without_expanding_scope() {
        let root = asset_orchard_local_service_test_dir("logical_data_dir");
        let mut config = asset_orchard_local_service_test_config(&root);
        config.logical_data_dir = Some(PathBuf::from("/var/lib/postfiat/validator-2"));
        let notes = config.data_dir.join("asset-orchard-local-vault");
        fs::create_dir_all(&notes).unwrap();
        let note = notes.join("note.json");
        atomic_write_private_json(&note, &json!({"schema": "test"})).unwrap();

        let resolved = resolve_client_note_path(
            &config,
            "/var/lib/postfiat/validator-2/asset-orchard-local-vault/note.json",
        )
        .unwrap();
        assert_eq!(resolved, note.canonicalize().unwrap());
        assert_eq!(
            client_output_note_path(&config, &note).unwrap(),
            "/var/lib/postfiat/validator-2/asset-orchard-local-vault/note.json"
        );

        let outside = resolve_client_note_path(&config, "/var/lib/postfiat/validator-1/note.json")
            .unwrap_err();
        assert_eq!(outside.kind(), io::ErrorKind::PermissionDenied);

        let traversal = resolve_client_note_path(
            &config,
            "/var/lib/postfiat/validator-2/../validator-1/note.json",
        )
        .unwrap_err();
        assert_eq!(traversal.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn asset_orchard_readiness_binds_profile_and_mirror_identity() {
        let root = asset_orchard_local_service_test_dir("readiness_identity");
        let config = asset_orchard_local_service_test_config(&root);
        NodeStore::new(&config.data_dir)
            .write_chain_tip(&postfiat_types::ChainTipState {
                schema: "postfiat-chain-tip-v1".to_string(),
                chain_id: "identity-test".to_string(),
                genesis_hash: "g".repeat(96),
                protocol_version: 1,
                height: 843,
                block_hash: "b".repeat(96),
                state_root: "r".repeat(96),
                ordered_batch_count: 0,
                receipt_count: 0,
                history_base_height: 0,
            })
            .unwrap();

        let readiness = local_readiness(&config);

        assert_eq!(readiness["product_profile_sha256"], "a".repeat(64));
        assert_eq!(readiness["mirror"]["height"], 843);
        assert_eq!(readiness["mirror"]["state_root"], "r".repeat(96));
        assert!(readiness["ready"].is_boolean());
    }

    #[test]
    fn asset_orchard_local_service_ingress_returns_real_ciphertext_separate_from_note() {
        let root = asset_orchard_local_service_test_dir("ingress_ciphertext");
        let config = asset_orchard_local_service_test_config(&root);
        postfiat_node::init(postfiat_node::InitOptions {
            data_dir: config.data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("initialize local ingress service state");
        let request = IngressNoteRequest {
            wallet_address: "pfwallet".to_string(),
            asset_id: "ab".repeat(48),
            amount_atoms: 7,
        };

        let (wallet_note, encrypted_output, vault_record) =
            build_and_store_note(&config, &request).expect("build encrypted ingress note");

        assert!(wallet_note.get("note").is_some());
        assert!(encrypted_output.starts_with("5046414f454e4331"));
        assert_ne!(
            encrypted_output,
            bytes_to_hex(
                format!(
                    "asset_orchard_wallet_ingress:{}:{}:{}",
                    request.asset_id,
                    request.amount_atoms,
                    wallet_note["output_commitment"].as_str().unwrap()
                )
                .as_bytes()
            )
        );
        assert!(vault_record.stored);
        assert!(config
            .vault_dir
            .join(format!("{}.json", vault_record.id))
            .is_file());

        fs::remove_dir_all(root).expect("cleanup ingress ciphertext test");
    }

    #[test]
    fn asset_orchard_local_service_http_rejects_forbidden_private_material() {
        let root = asset_orchard_local_service_test_dir("http_forbidden_material");
        let config = asset_orchard_local_service_test_config(&root);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_connection(&config, &mut stream).unwrap();
        });
        let body = r#"{"note_opening":{"value":1}}"#;
        let mut stream = TcpStream::connect(addr).unwrap();
        write!(
            stream,
            "POST /asset-orchard/private-egress-actions HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        server.join().unwrap();

        assert!(response.contains("HTTP/1.1 400 Bad Request"));
        assert!(response.contains("forbidden_private_material"));
        assert!(response.contains("$.note_opening"));
    }

    fn asset_orchard_local_service_note(
        asset_id: &str,
        amount_atoms: u64,
        commitment: &str,
    ) -> Value {
        json!({
            "schema": "postfiat-asset-orchard-wallet-note-v1",
            "pool_id": "asset-orchard-v1",
            "asset_id": asset_id,
            "value": amount_atoms,
            "output_commitment": commitment,
        })
    }

    fn asset_orchard_local_service_record(
        wallet_address: &str,
        asset_id: &str,
        amount_atoms: u64,
        commitment: &str,
        state: &str,
    ) -> Value {
        json!({
            "schema": NOTE_VAULT_SCHEMA,
            "created_at_unix_ms": 1u64,
            "wallet_address": wallet_address,
            "asset_id": asset_id,
            "amount_atoms": amount_atoms,
            "state": state,
            "wallet_note": asset_orchard_local_service_note(asset_id, amount_atoms, commitment),
        })
    }

    fn asset_orchard_local_service_swap_body(
        path_a: Option<&Path>,
        path_b: Option<&Path>,
    ) -> Value {
        let base_tag = postfiat_privacy_orchard::AssetTag::derive(&"b".repeat(96)).unwrap();
        let quote_tag = postfiat_privacy_orchard::AssetTag::derive(&"a".repeat(96)).unwrap();
        let mut body = json!({
            "wallet_address": "pfwallet",
            "from_asset_id": "a".repeat(96),
            "to_asset_id": "b".repeat(96),
            "amount_atoms": 42,
            "liquidity_commitment": "2".repeat(64),
            "quote_binding_hash": "3".repeat(64),
            "quote_expires_at_ms": (unix_ms().unwrap() + 60_000).to_string(),
            "pricing_claim": {
                "nav_epoch": 59,
                "reserve_packet_hash": "c".repeat(96),
                "ratio_numerator": 42,
                "ratio_denominator": 42,
                "mode": "at_nav_with_band",
                "band_bps": 0,
                "base_asset_tag_lo": format!("{:032x}", base_tag.lo),
                "base_asset_tag_hi": format!("{:032x}", base_tag.hi),
                "quote_asset_tag_lo": format!("{:032x}", quote_tag.lo),
                "quote_asset_tag_hi": format!("{:032x}", quote_tag.hi),
            },
        });
        if let Some(path) = path_a {
            body["input_note_path_a"] = Value::String(path.display().to_string());
        }
        if let Some(path) = path_b {
            body["input_note_path_b"] = Value::String(path.display().to_string());
        }
        body
    }

    fn asset_orchard_local_service_egress_body(path: Option<&Path>) -> Value {
        let mut body = json!({
            "wallet_address": "pfwallet",
            "to": "0x0000000000000000000000000000000000000001",
            "asset_id": "a".repeat(96),
            "amount_atoms": 42,
            "note_commitment": "1".repeat(64),
            "policy_id": "policy",
            "disclosure_hash": "4".repeat(64),
            "disclosure_ack": true,
        });
        if let Some(path) = path {
            body["input_note_path"] = Value::String(path.display().to_string());
        }
        body
    }

    #[test]
    fn asset_orchard_local_service_swap_with_valid_note_paths_loads_inputs() {
        let root = asset_orchard_local_service_test_dir("swap_valid_paths");
        let config = asset_orchard_local_service_test_config(&root);
        let note_a = root.join("note-a.json");
        let note_b = root.join("note-b.json");
        atomic_write_private_json(
            &note_a,
            &asset_orchard_local_service_note(&"a".repeat(96), 42, &"1".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 42, &"2".repeat(64)),
        )
        .unwrap();
        let wallet_record = asset_orchard_local_service_record(
            "pfwallet",
            &"a".repeat(96),
            42,
            &"1".repeat(64),
            "spendable",
        );
        let pool_record = asset_orchard_local_service_record(
            "controlled_pool_operator",
            &"b".repeat(96),
            42,
            &"2".repeat(64),
            "spendable",
        );
        atomic_write_private_json(&vault_record_path(&config, &"1".repeat(64)), &wallet_record)
            .unwrap();
        atomic_write_private_json(&vault_record_path(&config, &"2".repeat(64)), &pool_record)
            .unwrap();
        let request = parse_swap_action_request(&asset_orchard_local_service_swap_body(
            Some(&note_a),
            Some(&note_b),
        ))
        .unwrap();
        let vault_request =
            parse_swap_action_request(&asset_orchard_local_service_swap_body(None, None)).unwrap();

        let (wallet, pool) = swap_input_records(&config, &request).unwrap();
        let (vault_wallet, vault_pool) = swap_input_records(&config, &vault_request).unwrap();

        assert_eq!(note_record_id(&wallet).unwrap(), "1".repeat(64));
        assert_eq!(note_record_id(&pool).unwrap(), "2".repeat(64));
        assert_eq!(wallet, vault_wallet);
        assert_eq!(pool, vault_pool);
    }

    #[test]
    fn asset_orchard_local_service_swap_binds_imported_liquidity_wallet() {
        let root = asset_orchard_local_service_test_dir("swap_liquidity_wallet");
        let config = asset_orchard_local_service_test_config(&root);
        let note_a = root.join("note-a.json");
        let note_b = root.join("note-b.json");
        atomic_write_private_json(
            &note_a,
            &asset_orchard_local_service_note(&"a".repeat(96), 42, &"1".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 42, &"2".repeat(64)),
        )
        .unwrap();
        let mut body = asset_orchard_local_service_swap_body(Some(&note_a), Some(&note_b));
        body["liquidity_wallet_address"] = Value::String("pffacility".to_string());
        let request = parse_swap_action_request(&body).unwrap();

        let (_, liquidity) = swap_input_records(&config, &request).unwrap();

        assert_eq!(
            liquidity.get("wallet_address").and_then(Value::as_str),
            Some("pffacility")
        );
    }

    #[test]
    fn asset_orchard_local_service_swap_accepts_distinct_pool_note_amount() {
        let root = asset_orchard_local_service_test_dir("swap_distinct_pool_amount");
        let config = asset_orchard_local_service_test_config(&root);
        let note_a = root.join("note-a.json");
        let note_b = root.join("note-b.json");
        atomic_write_private_json(
            &note_a,
            &asset_orchard_local_service_note(&"a".repeat(96), 30_000_000, &"1".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 5, &"2".repeat(64)),
        )
        .unwrap();
        let mut body = asset_orchard_local_service_swap_body(Some(&note_a), Some(&note_b));
        body["amount_atoms"] = Value::from(30_000_000_u64);
        body["liquidity_amount_atoms"] = Value::from(5_u64);

        let request = parse_swap_action_request(&body).unwrap();
        let (wallet, pool) = swap_input_records(&config, &request).unwrap();

        assert_eq!(
            wallet.get("amount_atoms").and_then(Value::as_u64),
            Some(30_000_000)
        );
        assert_eq!(pool.get("amount_atoms").and_then(Value::as_u64), Some(5));
    }

    #[test]
    fn asset_orchard_local_service_swap_orders_both_fix_directions_from_asset_tags() {
        let reverse =
            parse_swap_action_request(&asset_orchard_local_service_swap_body(None, None)).unwrap();
        assert_eq!(
            swap_circuit_order(&reverse).unwrap(),
            SwapCircuitOrder::WalletIsQuote
        );

        let mut forward_body = asset_orchard_local_service_swap_body(None, None);
        let wallet_base = AssetTag::derive(&"a".repeat(96)).unwrap();
        let facility_quote = AssetTag::derive(&"b".repeat(96)).unwrap();
        forward_body["pricing_claim"]["base_asset_tag_lo"] =
            Value::String(format!("{:032x}", wallet_base.lo));
        forward_body["pricing_claim"]["base_asset_tag_hi"] =
            Value::String(format!("{:032x}", wallet_base.hi));
        forward_body["pricing_claim"]["quote_asset_tag_lo"] =
            Value::String(format!("{:032x}", facility_quote.lo));
        forward_body["pricing_claim"]["quote_asset_tag_hi"] =
            Value::String(format!("{:032x}", facility_quote.hi));
        let forward = parse_swap_action_request(&forward_body).unwrap();
        assert_eq!(
            swap_circuit_order(&forward).unwrap(),
            SwapCircuitOrder::WalletIsBase
        );
    }

    #[test]
    fn asset_orchard_local_service_swap_rejects_pricing_tags_for_another_pair() {
        let mut body = asset_orchard_local_service_swap_body(None, None);
        let unrelated = AssetTag::derive(&"c".repeat(96)).unwrap();
        body["pricing_claim"]["base_asset_tag_lo"] =
            Value::String(format!("{:032x}", unrelated.lo));
        body["pricing_claim"]["base_asset_tag_hi"] =
            Value::String(format!("{:032x}", unrelated.hi));
        let request = parse_swap_action_request(&body).unwrap();

        let error = swap_circuit_order(&request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error
            .to_string()
            .contains("do not match the pricing claim base/quote tags"));
    }

    #[test]
    fn asset_orchard_local_service_swap_request_id_is_bounded_and_fingerprint_bound() {
        let mut body = asset_orchard_local_service_swap_body(None, None);
        body["request_id"] = Value::String("pnok-fix-run-01".to_string());
        let request = parse_swap_action_request(&body).unwrap();
        assert_eq!(request.request_id.as_deref(), Some("pnok-fix-run-01"));
        let fingerprint = swap_action_request_fingerprint(&request).unwrap();
        assert_eq!(fingerprint.len(), 96);

        body["amount_atoms"] = Value::from(43_u64);
        let changed = parse_swap_action_request(&body).unwrap();
        assert_ne!(
            fingerprint,
            swap_action_request_fingerprint(&changed).unwrap()
        );

        body["amount_atoms"] = Value::from(42_u64);
        body["liquidity_wallet_address"] = Value::String("pffacility".to_string());
        let changed_liquidity_wallet = parse_swap_action_request(&body).unwrap();
        assert_ne!(
            fingerprint,
            swap_action_request_fingerprint(&changed_liquidity_wallet).unwrap()
        );

        body["request_id"] = Value::String("../escape".to_string());
        let error = parse_swap_action_request(&body).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("request_id must match"));
    }

    #[test]
    fn asset_orchard_local_service_swap_retry_recovers_cached_response_before_note_selection() {
        let root = asset_orchard_local_service_test_dir("swap_cached_response");
        let config = asset_orchard_local_service_test_config(&root);
        let mut body = asset_orchard_local_service_swap_body(None, None);
        body["request_id"] = Value::String("pnok-fix-recovery-01".to_string());
        let request = parse_swap_action_request(&body).unwrap();
        let fingerprint = swap_action_request_fingerprint(&request).unwrap();
        let work_dir = config
            .vault_dir
            .join("swap-work/by-request/pnok-fix-recovery-01");
        let cached = json!({
            "ok": true,
            "schema": "postfiat-asset-orchard-local-swap-action-v1",
            "request_id": "pnok-fix-recovery-01",
            "request_fingerprint": fingerprint,
            "swap_id": "a".repeat(96),
        });
        atomic_write_private_json(&work_dir.join("response.json"), &cached).unwrap();

        let recovered = build_and_store_swap_action(&config, &request).unwrap();
        assert_eq!(recovered, cached);

        body["amount_atoms"] = Value::from(43_u64);
        let changed = parse_swap_action_request(&body).unwrap();
        let error = build_and_store_swap_action(&config, &changed).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(error
            .to_string()
            .contains("different immutable request fields"));
    }

    #[test]
    fn asset_orchard_local_service_swap_missing_note_path_fails_cleanly() {
        let root = asset_orchard_local_service_test_dir("swap_missing_path");
        let config = asset_orchard_local_service_test_config(&root);
        let note_b = root.join("note-b.json");
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 42, &"2".repeat(64)),
        )
        .unwrap();
        let missing = root.join("missing-note-a.json");
        let request = parse_swap_action_request(&asset_orchard_local_service_swap_body(
            Some(&missing),
            Some(&note_b),
        ))
        .unwrap();

        let error = swap_input_records(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("input_note_path_a"));
        assert!(error.to_string().contains("missing-note-a.json"));
    }

    #[test]
    fn asset_orchard_local_service_swap_invalid_note_json_path_fails_cleanly() {
        let root = asset_orchard_local_service_test_dir("swap_invalid_json");
        let config = asset_orchard_local_service_test_config(&root);
        let note_a = root.join("note-a.json");
        let note_b = root.join("note-b.json");
        fs::write(&note_a, "{not json").unwrap();
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 42, &"2".repeat(64)),
        )
        .unwrap();
        let request = parse_swap_action_request(&asset_orchard_local_service_swap_body(
            Some(&note_a),
            Some(&note_b),
        ))
        .unwrap();

        let error = swap_input_records(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("not valid note JSON"));
    }

    #[test]
    fn asset_orchard_local_service_swap_schema_invalid_note_path_fails_cleanly() {
        let root = asset_orchard_local_service_test_dir("swap_schema_invalid");
        let config = asset_orchard_local_service_test_config(&root);
        let note_a = root.join("note-a.json");
        let note_b = root.join("note-b.json");
        atomic_write_private_json(&note_a, &json!({"schema": "wrong"})).unwrap();
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 42, &"2".repeat(64)),
        )
        .unwrap();
        let request = parse_swap_action_request(&asset_orchard_local_service_swap_body(
            Some(&note_a),
            Some(&note_b),
        ))
        .unwrap();

        let error = swap_input_records(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("schema-invalid note JSON"));
    }

    #[test]
    fn asset_orchard_local_service_swap_same_path_locked_for_swap_is_rejected() {
        let root = asset_orchard_local_service_test_dir("swap_locked_replay");
        let config = asset_orchard_local_service_test_config(&root);
        let note_a = root.join("note-a.json");
        let note_b = root.join("note-b.json");
        atomic_write_private_json(
            &note_a,
            &asset_orchard_local_service_note(&"a".repeat(96), 42, &"1".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 42, &"2".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &vault_record_path(&config, &"1".repeat(64)),
            &asset_orchard_local_service_record(
                "pfwallet",
                &"a".repeat(96),
                42,
                &"1".repeat(64),
                "locked_for_swap",
            ),
        )
        .unwrap();
        let request = parse_swap_action_request(&asset_orchard_local_service_swap_body(
            Some(&note_a),
            Some(&note_b),
        ))
        .unwrap();

        let error = swap_input_records(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("input_note_path_a"));
        assert!(error.to_string().contains("note-a.json"));
        assert!(error.to_string().contains("existing vault record"));
        assert!(error.to_string().contains("local note is not spendable"));
    }

    #[test]
    fn asset_orchard_local_service_swap_same_path_spent_is_rejected() {
        let root = asset_orchard_local_service_test_dir("swap_spent_replay");
        let config = asset_orchard_local_service_test_config(&root);
        let note_a = root.join("note-a.json");
        let note_b = root.join("note-b.json");
        atomic_write_private_json(
            &note_a,
            &asset_orchard_local_service_note(&"a".repeat(96), 42, &"1".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &note_b,
            &asset_orchard_local_service_note(&"b".repeat(96), 42, &"2".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &vault_record_path(&config, &"1".repeat(64)),
            &asset_orchard_local_service_record(
                "pfwallet",
                &"a".repeat(96),
                42,
                &"1".repeat(64),
                "spent",
            ),
        )
        .unwrap();
        let request = parse_swap_action_request(&asset_orchard_local_service_swap_body(
            Some(&note_a),
            Some(&note_b),
        ))
        .unwrap();

        let error = swap_input_records(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("input_note_path_a"));
        assert!(error.to_string().contains("note-a.json"));
        assert!(error.to_string().contains("existing vault record"));
        assert!(error.to_string().contains("local note is not spendable"));
    }

    #[test]
    fn asset_orchard_local_service_egress_with_valid_note_path_loads_input() {
        let root = asset_orchard_local_service_test_dir("egress_valid_path");
        let config = asset_orchard_local_service_test_config(&root);
        let note = root.join("note.json");
        atomic_write_private_json(
            &note,
            &asset_orchard_local_service_note(&"a".repeat(96), 42, &"1".repeat(64)),
        )
        .unwrap();
        let record = asset_orchard_local_service_record(
            "pfwallet",
            &"a".repeat(96),
            42,
            &"1".repeat(64),
            "spendable",
        );
        atomic_write_private_json(&vault_record_path(&config, &"1".repeat(64)), &record).unwrap();
        let request = parse_private_egress_action_request(
            &asset_orchard_local_service_egress_body(Some(&note)),
        )
        .unwrap();
        let vault_request =
            parse_private_egress_action_request(&asset_orchard_local_service_egress_body(None))
                .unwrap();

        let input = private_egress_input_record(&config, &request).unwrap();
        let vault_input = private_egress_input_record(&config, &vault_request).unwrap();

        assert_eq!(note_record_id(&input).unwrap(), "1".repeat(64));
        assert_eq!(input, vault_input);
    }

    #[test]
    fn asset_orchard_local_service_egress_missing_note_path_fails_cleanly() {
        let root = asset_orchard_local_service_test_dir("egress_missing_path");
        let config = asset_orchard_local_service_test_config(&root);
        let missing = root.join("missing-note.json");
        let request = parse_private_egress_action_request(
            &asset_orchard_local_service_egress_body(Some(&missing)),
        )
        .unwrap();

        let error = private_egress_input_record(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("input_note_path"));
        assert!(error.to_string().contains("missing-note.json"));
    }

    #[test]
    fn asset_orchard_local_service_egress_invalid_note_json_path_fails_cleanly() {
        let root = asset_orchard_local_service_test_dir("egress_invalid_json");
        let config = asset_orchard_local_service_test_config(&root);
        let note = root.join("note.json");
        fs::write(&note, "{not json").unwrap();
        let request = parse_private_egress_action_request(
            &asset_orchard_local_service_egress_body(Some(&note)),
        )
        .unwrap();

        let error = private_egress_input_record(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("not valid note JSON"));
    }

    #[test]
    fn asset_orchard_local_service_egress_schema_invalid_note_path_fails_cleanly() {
        let root = asset_orchard_local_service_test_dir("egress_schema_invalid");
        let config = asset_orchard_local_service_test_config(&root);
        let note = root.join("note.json");
        atomic_write_private_json(&note, &json!({"schema": "wrong"})).unwrap();
        let request = parse_private_egress_action_request(
            &asset_orchard_local_service_egress_body(Some(&note)),
        )
        .unwrap();

        let error = private_egress_input_record(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("schema-invalid note JSON"));
    }

    #[test]
    fn asset_orchard_local_service_egress_same_path_locked_for_egress_is_rejected() {
        let root = asset_orchard_local_service_test_dir("egress_locked_replay");
        let config = asset_orchard_local_service_test_config(&root);
        let note = root.join("note.json");
        atomic_write_private_json(
            &note,
            &asset_orchard_local_service_note(&"a".repeat(96), 42, &"1".repeat(64)),
        )
        .unwrap();
        atomic_write_private_json(
            &vault_record_path(&config, &"1".repeat(64)),
            &asset_orchard_local_service_record(
                "pfwallet",
                &"a".repeat(96),
                42,
                &"1".repeat(64),
                "locked_for_egress",
            ),
        )
        .unwrap();
        let request = parse_private_egress_action_request(
            &asset_orchard_local_service_egress_body(Some(&note)),
        )
        .unwrap();

        let error = private_egress_input_record(&config, &request).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("input_note_path"));
        assert!(error.to_string().contains("note.json"));
        assert!(error.to_string().contains("existing vault record"));
        assert!(error.to_string().contains("local note is not spendable"));
    }

    #[test]
    fn asset_orchard_local_service_egress_existing_vault_record_mismatches_name_path() {
        let cases = vec![
            (
                "asset",
                asset_orchard_local_service_record(
                    "pfwallet",
                    &"c".repeat(96),
                    42,
                    &"1".repeat(64),
                    "spendable",
                ),
                "local note asset_id mismatch",
            ),
            (
                "amount",
                asset_orchard_local_service_record(
                    "pfwallet",
                    &"a".repeat(96),
                    41,
                    &"1".repeat(64),
                    "spendable",
                ),
                "local note amount mismatch",
            ),
            (
                "wallet",
                asset_orchard_local_service_record(
                    "other-wallet",
                    &"a".repeat(96),
                    42,
                    &"1".repeat(64),
                    "spendable",
                ),
                "local note wallet owner mismatch",
            ),
            (
                "commitment",
                asset_orchard_local_service_record(
                    "pfwallet",
                    &"a".repeat(96),
                    42,
                    &"9".repeat(64),
                    "spendable",
                ),
                "local note commitment mismatch",
            ),
        ];

        for (case, record, expected) in cases {
            let root = asset_orchard_local_service_test_dir(&format!(
                "egress_existing_vault_record_mismatch_{case}"
            ));
            let config = asset_orchard_local_service_test_config(&root);
            let note = root.join("note.json");
            atomic_write_private_json(
                &note,
                &asset_orchard_local_service_note(&"a".repeat(96), 42, &"1".repeat(64)),
            )
            .unwrap();
            atomic_write_private_json(&vault_record_path(&config, &"1".repeat(64)), &record)
                .unwrap();
            let request = parse_private_egress_action_request(
                &asset_orchard_local_service_egress_body(Some(&note)),
            )
            .unwrap();

            let error = private_egress_input_record(&config, &request).unwrap_err();

            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("input_note_path"));
            assert!(error.to_string().contains("note.json"));
            assert!(error.to_string().contains("existing vault record"));
            assert!(error.to_string().contains(expected));
        }
    }

    #[test]
    fn asset_orchard_local_service_egress_without_note_path_uses_vault_fallback() {
        let root = asset_orchard_local_service_test_dir("egress_vault_fallback");
        let config = asset_orchard_local_service_test_config(&root);
        let record = asset_orchard_local_service_record(
            "pfwallet",
            &"a".repeat(96),
            42,
            &"1".repeat(64),
            "spendable",
        );
        atomic_write_private_json(&vault_record_path(&config, &"1".repeat(64)), &record).unwrap();
        let request =
            parse_private_egress_action_request(&asset_orchard_local_service_egress_body(None))
                .unwrap();

        let input = private_egress_input_record(&config, &request).unwrap();

        assert_eq!(note_record_id(&input).unwrap(), "1".repeat(64));
    }

    #[test]
    fn asset_orchard_local_service_swap_without_note_paths_uses_vault_fallback() {
        let root = asset_orchard_local_service_test_dir("swap_vault_fallback");
        let config = asset_orchard_local_service_test_config(&root);
        let wallet_record = asset_orchard_local_service_record(
            "pfwallet",
            &"a".repeat(96),
            42,
            &"1".repeat(64),
            "spendable",
        );
        let pool_record = asset_orchard_local_service_record(
            "controlled_pool_operator",
            &"b".repeat(96),
            42,
            &"2".repeat(64),
            "spendable",
        );
        atomic_write_private_json(&vault_record_path(&config, &"1".repeat(64)), &wallet_record)
            .unwrap();
        atomic_write_private_json(&vault_record_path(&config, &"2".repeat(64)), &pool_record)
            .unwrap();
        atomic_write_private_json(
            &vault_record_path(&config, &"3".repeat(64)),
            &asset_orchard_local_service_record(
                "pfwallet",
                &"a".repeat(96),
                42,
                &"3".repeat(64),
                "spendable",
            ),
        )
        .unwrap();
        let mut body = asset_orchard_local_service_swap_body(None, None);
        body["wallet_commitment"] = Value::String("1".repeat(64));
        let request = parse_swap_action_request(&body).unwrap();

        let (wallet, pool) = swap_input_records(&config, &request).unwrap();

        assert_eq!(note_record_id(&wallet).unwrap(), "1".repeat(64));
        assert_eq!(note_record_id(&pool).unwrap(), "2".repeat(64));
    }
}
