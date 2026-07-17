use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use postfiat_crypto_provider::{bytes_to_hex, hash_hex};
use postfiat_execution::genesis_hash;
use postfiat_node::{
    create_asset_orchard_private_egress, create_asset_orchard_swap_action,
    create_shielded_swap_action_batch, AssetOrchardPrivateEgressCreateOptions,
    AssetOrchardSwapCreateOptions, ShieldedSwapActionBatchOptions,
};
use postfiat_privacy_orchard::{
    asset_orchard_domain_genesis_hash, build_asset_orchard_wallet_note,
    encrypt_asset_orchard_wallet_note, reset_asset_orchard_private_egress_timings,
    take_asset_orchard_private_egress_timings, AssetOrchardPricingClaim,
    AssetOrchardPrivateEgressProvingKey, AssetOrchardPrivateEgressVerifyingKey,
    AssetOrchardSwapProvingKey, AssetOrchardSwapVerifyingKey,
};
use postfiat_storage::NodeStore;
use serde::Serialize;
use serde_json::{json, Value};

const MAX_BODY_BYTES: usize = 64 * 1024;
const DEFAULT_BIND: &str = "127.0.0.1:8789";
const NOTE_VAULT_SCHEMA: &str = "postfiat-asset-orchard-local-note-vault-record-v1";
const PREWARM_READY_SCHEMA: &str = "postfiat-asset-orchard-local-service-prewarm-ready-v1";

#[derive(Debug, Clone)]
struct Config {
    bind: SocketAddr,
    data_dir: PathBuf,
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
    wallet_address: String,
    from_asset_id: String,
    to_asset_id: String,
    amount_atoms: u64,
    liquidity_amount_atoms: u64,
    liquidity_commitment: String,
    quote_binding_hash: String,
    quote_expires_at_ms: u128,
    pricing_claim: AssetOrchardPricingClaim,
    input_note_path_a: Option<String>,
    input_note_path_b: Option<String>,
}

#[derive(Debug)]
struct SwapBatchRequest {
    swap_action_json: String,
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
                if let Err(error) = handle_connection(&config, &mut stream) {
                    let _ = write_json_response(
                        &mut stream,
                        500,
                        &json!({ "ok": false, "error": error.to_string() }),
                    );
                }
            }
            Err(error) => eprintln!("asset-orchard local service accept failed: {error}"),
        }
    }
    Ok(())
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
    let mut prewarm_ready_file = env::var("ASSET_ORCHARD_PREWARM_READY_FILE")
        .ok()
        .map(PathBuf::from);
    let product_profile_sha256 = env::var("STAKEHUB_PRODUCT_PROFILE_SHA256").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "STAKEHUB_PRODUCT_PROFILE_SHA256 is required",
        )
    })?;
    if product_profile_sha256.len() != 64
        || !product_profile_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "STAKEHUB_PRODUCT_PROFILE_SHA256 must be 64 hexadecimal characters",
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

    Ok(Config {
        bind,
        data_dir: data_dir.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "POSTFIAT_DATA_DIR or --data-dir is required",
            )
        })?,
        vault_dir,
        prewarm_ready_file,
        product_profile_sha256: product_profile_sha256.to_ascii_lowercase(),
    })
}

fn print_usage() {
    println!(
        "usage: asset-orchard-local-service [--bind 127.0.0.1:8789] --data-dir PATH [--vault-dir PATH] [--prewarm-ready-file PATH]"
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

fn start_prover_prewarm(config: &Config) -> io::Result<()> {
    if let Some(parent) = config.prewarm_ready_file.parent() {
        prepare_private_dir(parent)?;
    }
    let enabled = prover_prewarm_enabled();
    let started_at_unix_ms = unix_ms()?;
    let state = PrewarmState::new(
        enabled,
        started_at_unix_ms,
        config.prewarm_ready_file.clone(),
    );
    let _ = PROVER_WARM_STATE.set(Mutex::new(state));
    if !enabled {
        write_prewarm_marker_if_terminal();
        return Ok(());
    }

    thread::Builder::new()
        .name("asset-orchard-prewarm-swap".to_string())
        .spawn(|| {
            let start = Instant::now();
            let result = prewarm_swap_keys().map_err(|error| error.to_string());
            finish_prewarm_circuit("swap", start, result);
        })?;

    thread::Builder::new()
        .name("asset-orchard-prewarm-private-egress".to_string())
        .spawn(|| {
            let start = Instant::now();
            let result = prewarm_private_egress_keys().map_err(|error| error.to_string());
            finish_prewarm_circuit("private_egress", start, result);
        })?;

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
    fn new(enabled: bool, started_at_unix_ms: u128, marker_file: PathBuf) -> Self {
        let (status, swap, private_egress) = if enabled {
            (
                "warming",
                PrewarmCircuitState::pending("asset-orchard-swap-v1"),
                PrewarmCircuitState::pending("asset-orchard-private-egress-v1"),
            )
        } else {
            (
                "disabled",
                PrewarmCircuitState::disabled("asset-orchard-swap-v1"),
                PrewarmCircuitState::disabled("asset-orchard-private-egress-v1"),
            )
        };
        Self {
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
        }
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
        } else if self.swap.status == "ready" && self.private_egress.status == "ready" {
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
            Err(error) => write_json_response(
                stream,
                400,
                &json!({ "ok": false, "error": error.to_string() }),
            ),
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
                Err(error) => write_json_response(
                    stream,
                    400,
                    &json!({ "ok": false, "error": error.to_string() }),
                ),
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
                Err(error) => write_json_response(
                    stream,
                    400,
                    &json!({ "ok": false, "error": error.to_string() }),
                ),
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
                Err(error) => write_json_response(
                    stream,
                    400,
                    &json!({ "ok": false, "error": error.to_string() }),
                ),
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
            let egress = parse_private_egress_action_request(&body)?;
            match build_and_store_private_egress_action(config, &egress) {
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
                Err(error) => write_json_response(
                    stream,
                    400,
                    &json!({ "ok": false, "error": error.to_string() }),
                ),
            }
        }
        _ => write_json_response(stream, 404, &json!({ "ok": false, "error": "not_found" })),
    }
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

fn build_and_store_swap_action(config: &Config, request: &SwapActionRequest) -> io::Result<Value> {
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

    let work_dir =
        config
            .vault_dir
            .join("swap-work")
            .join(format!("{}-{}", std::process::id(), unix_ms()?));
    prepare_private_dir(&work_dir)?;
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

    reset_asset_orchard_private_egress_timings();
    let report = match create_asset_orchard_swap_action(AssetOrchardSwapCreateOptions {
        data_dir: config.data_dir.clone(),
        // The pricing-bound circuit's canonical private order is NAV base,
        // then settlement quote. The service API remains wallet-from/pool-to,
        // so the pool input must be presented first to the circuit.
        input_note_files: [input_b, input_a],
        output_note_seed_hexes: [bytes_to_hex(&random_seed()?), bytes_to_hex(&random_seed()?)],
        pricing_claim_file,
        action_file: action_file.clone(),
        // build_asset_orchard_swap_action swaps the two ordered inputs. With
        // pool/base first, output[1] is the wallet's acquired NAV note.
        output_note_files: [output_b.clone(), output_a.clone()],
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

    Ok(json!({
        "ok": true,
        "schema": "postfiat-asset-orchard-local-swap-action-v1",
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
    }))
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
                None,
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
                None,
            )?,
            select_vault_note(
                config,
                None,
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
    let note_file = work_dir.join("input-note.json");
    let egress_file = work_dir.join("private-egress.json");
    atomic_write_private_json(&note_file, wallet_note_value(&input)?)?;

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
    let mut file = fs::File::open("/dev/urandom")?;
    let mut seed = [0u8; 32];
    file.read_exact(&mut seed)?;
    Ok(seed)
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
    let wallet_address = string_field(body, "wallet_address")?;
    let from_asset_id = hex_field(body, "from_asset_id", 96)?;
    let to_asset_id = hex_field(body, "to_asset_id", 96)?;
    let amount_atoms = u64_field(body, "amount_atoms")?;
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
        wallet_address,
        from_asset_id,
        to_asset_id,
        amount_atoms,
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
    })
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
            "error": error.to_string(),
        }),
    };
    let mirror_ready = mirror.get("height").and_then(Value::as_u64).is_some()
        && mirror.get("state_root").and_then(Value::as_str).is_some();
    json!({
        "ok": mirror_ready,
        "ready": mirror_ready,
        "local_only": true,
        "service": "asset-orchard-local-service",
        "bind": config.bind.to_string(),
        "product_profile_sha256": config.product_profile_sha256,
        "mirror": mirror,
        "pool_id": "asset-orchard-v1",
        "circuit_id": "asset-orchard-swap-v1",
        "k": 15,
        "vault_schema": NOTE_VAULT_SCHEMA,
        "prover_warm": prover_warm_snapshot(config),
        "operations": {
            "ingress_notes": "/asset-orchard/ingress-notes",
            "swap_actions": "/asset-orchard/swap-actions",
            "swap_batch": "/asset-orchard/swap-batch",
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
    ) || normalized.starts_with("spend_")
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
    let message = error.to_string();
    json!({
        "ok": false,
        "error": message.clone(),
        "message": message,
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
            vault_dir,
            prewarm_ready_file: root.join("prewarm-ready.json"),
            product_profile_sha256: "a".repeat(64),
        }
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
        assert_eq!(readiness["ready"], true);
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
        let request =
            parse_swap_action_request(&asset_orchard_local_service_swap_body(None, None)).unwrap();

        let (wallet, pool) = swap_input_records(&config, &request).unwrap();

        assert_eq!(note_record_id(&wallet).unwrap(), "1".repeat(64));
        assert_eq!(note_record_id(&pool).unwrap(), "2".repeat(64));
    }
}
