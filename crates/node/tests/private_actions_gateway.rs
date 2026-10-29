//! Gateway-to-node integration for user-proven shielded actions (W-1).
//!
//! Two real validators (each a transport service plus `rpc-serve
//! --allow-private-action-admission`; the certified round needs a peer) and
//! the real wallet proxy with both as its sources, with no stub:
//! a native session signed in Rust, the private snapshot, a user-built ingress
//! v2 admitted once and applied by the validator's own round, its receipt with
//! finality, and the gateway's refusals. Skipped when Node.js or the proxy's
//! npm dependencies are absent.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hex_to_bytes, ml_dsa_65_keygen, ml_dsa_65_sign,
    ml_dsa_65_sign_with_context_seed, ML_DSA_65_ALGORITHM,
};
use postfiat_execution::{genesis_hash, TRUSTLINE_STATE_EXPANSION_FEE};
use postfiat_network::{local_topology, NetworkDomain};
use postfiat_node::{
    apply_batch, asset_fee_quote, create_asset_orchard_ingress, create_mempool_batch,
    create_transfer_batch, init, submit_signed_asset_transaction_json_to_mempool,
    ApplyBatchOptions, AssetFeeQuoteOptions, AssetOrchardIngressCreateOptions,
    AssetOrchardIngressFile, BatchTransferOptions, DevKeyFile, InitOptions, MempoolBatchOptions,
    SignedAssetTransactionJsonSubmitOptions,
};
use postfiat_storage::NodeStore;
use postfiat_types::{
    AssetCreateOperation, AssetTransactionOperation, GovernanceAmendment, IssuedPaymentOperation,
    ShieldedAction, SignedAssetTransaction, TrustSetOperation, UnsignedAssetTransaction,
    ADDRESS_NAMESPACE, GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT,
};
use serde_json::{json, Value};

const CHAIN_ID: &str = "postfiat-local";
const VALIDATORS: usize = 2;
const ORIGIN: &str = "https://gateway.example.test";
const BROWSER_TOKEN: &str = "integration-browser-token-32-bytes-min-0001";

struct Harness {
    root: PathBuf,
    children: Vec<Child>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        if std::env::var_os("POSTFIAT_KEEP_PRIVATE_ACTIONS_GATEWAY").is_none() {
            let _ = fs::remove_dir_all(&self.root);
        } else {
            eprintln!("preserved harness at {}", self.root.display());
        }
    }
}

fn node_bin() -> &'static str {
    env!("CARGO_BIN_EXE_postfiat-node")
}

fn proxy_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../wallet-proxy")
}

fn proxy_available() -> bool {
    proxy_dir().join("node_modules/ws").is_dir()
        && proxy_dir()
            .join("node_modules/@noble/post-quantum")
            .is_dir()
        && Command::new("node")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local address")
        .port()
}

fn free_base_port() -> u16 {
    for base in (41_000u16..60_000).step_by(16) {
        let listeners: Vec<_> = (0..4)
            .map_while(|offset| TcpListener::bind(("127.0.0.1", base + offset)).ok())
            .collect();
        if listeners.len() == 4 {
            return base;
        }
    }
    panic!("no contiguous local port range available");
}

fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {}", path.display());
}

fn dev_key() -> DevKeyFile {
    let key_pair = ml_dsa_65_keygen().expect("ML-DSA-65 key pair");
    DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: address_from_public_key(&key_pair.public_key),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        private_key_hex: bytes_to_hex(&key_pair.private_key),
    }
}

fn write_private(path: &Path, contents: String) {
    fs::write(path, contents).expect("write private file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("chmod 0600");
    }
}

fn apply(data_dir: &Path, batch: &Path, label: &str) {
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file: batch.to_path_buf(),
        certificate_file: None,
    })
    .unwrap_or_else(|error| panic!("apply {label}: {error}"));
    assert!(
        !receipts.is_empty() && receipts.iter().all(|receipt| receipt.accepted),
        "{label} rejected: {receipts:?}"
    );
}

fn fund(data_dir: &Path, address: &str, label: &str) {
    let batch = data_dir.join(format!("{label}.batch.json"));
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.to_path_buf(),
        key_file: None,
        to: address.to_string(),
        amount: 1_000_000,
        batch_file: batch.clone(),
    })
    .unwrap_or_else(|error| panic!("fund {label}: {error}"));
    apply(data_dir, &batch, label);
}

fn asset_operation(
    data_dir: &Path,
    signer: &DevKeyFile,
    operation: AssetTransactionOperation,
    label: &str,
) {
    let quote = asset_fee_quote(AssetFeeQuoteOptions {
        data_dir: data_dir.to_path_buf(),
        source: signer.address.clone(),
        operation_json: serde_json::to_string(&operation).expect("operation json"),
        sequence: None,
    })
    .unwrap_or_else(|error| panic!("quote {label}: {error}"));
    let unsigned = UnsignedAssetTransaction {
        chain_id: quote.chain_id,
        genesis_hash: quote.genesis_hash,
        protocol_version: quote.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: quote.transaction_kind,
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: quote.source,
        fee: quote.minimum_fee,
        sequence: quote.sequence,
        operation,
    };
    let private_key = hex_to_bytes(&signer.private_key_hex).expect("signer key");
    let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
    let signed = SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: signer.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    };
    submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
        data_dir: data_dir.to_path_buf(),
        signed_asset_transaction_json: serde_json::to_string(&signed).expect("signed json"),
    })
    .unwrap_or_else(|error| panic!("submit {label}: {error}"));
    let batch = data_dir.join(format!("{label}.batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file: batch.clone(),
        max_transactions: 1,
    })
    .unwrap_or_else(|error| panic!("batch {label}: {error}"));
    apply(data_dir, &batch, label);
}

fn activate(data_dir: &Path, kind: &str, label: &str) {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let mut governance = store.read_governance().expect("governance");
    governance.apply(GovernanceAmendment {
        amendment_id: label.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        instance_id: "private-actions-gateway".to_string(),
        proposal_id: label.to_string(),
        certificate_id: label.to_string(),
        proposer: "validator-0".to_string(),
        validators: vec!["validator-0".to_string()],
        quorum: 1,
        kind: kind.to_string(),
        value: 1,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        support: vec!["validator-0".to_string()],
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
    });
    store
        .write_governance(&governance)
        .expect("write governance");
}

fn split_validator_key(data_dir: &Path, validator: &str) -> PathBuf {
    let combined: Value =
        serde_json::from_slice(&fs::read(data_dir.join("validator_keys.json")).expect("keys"))
            .expect("keys json");
    let record = combined["validators"]
        .as_array()
        .expect("validator keys")
        .iter()
        .find(|record| record["node_id"] == validator)
        .expect("validator key")
        .clone();
    let path = data_dir.join(format!("{validator}.validator_keys.json"));
    write_private(&path, format!("{}\n", json!({"validators": [record]})));
    path
}

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create directory");
    for entry in fs::read_dir(source).expect("read directory") {
        let entry = entry.expect("entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("entry type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy file");
        }
    }
}

fn rpc(port: u16, method: &str, params: Value) -> Value {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect node RPC");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .expect("timeout");
    let request = json!({"version": "postfiat-local-rpc-v1", "id": method, "method": method, "params": params});
    stream
        .write_all(format!("{request}\n").as_bytes())
        .expect("write RPC");
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .expect("read RPC");
    serde_json::from_str(&line).expect("RPC json")
}

fn http(port: u16, path: &str, body: &Value, token: &str) -> (u16, Value) {
    let payload = body.to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect proxy");
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .expect("timeout");
    let auth = if token.is_empty() {
        String::new()
    } else {
        format!("Authorization: Bearer {token}\r\n")
    };
    write!(
        stream,
        "POST {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nOrigin: {ORIGIN}\r\nContent-Type: application/json\r\n{auth}Content-Length: {}\r\n\r\n{payload}",
        payload.len()
    )
    .expect("write HTTP");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read HTTP");
    let status = response[9..12].parse().expect("status code");
    let body = response.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

fn sha3_hex(domain: &str, bytes: &[u8]) -> String {
    bytes_to_hex(&postfiat_crypto_provider::hash_bytes(domain, bytes))
}

#[test]
fn gateway_admits_a_user_proven_ingress_through_the_real_node() {
    if !proxy_available() {
        eprintln!("skipping: Node.js or wallet-proxy/node_modules is missing (run npm ci in wallet-proxy)");
        return;
    }
    let started = Instant::now();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let mut harness = Harness {
        root: std::env::temp_dir().join(format!(
            "postfiat-private-gateway-{}-{nanos}",
            std::process::id()
        )),
        children: Vec::new(),
    };
    let data_dir = harness.root.join("validator-0");
    fs::create_dir_all(&harness.root).expect("harness root");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: VALIDATORS as u32,
    })
    .expect("init");
    // Governed activations, written before the first block so that every
    // committed state root (and so every finalized read) includes them.
    activate(
        &data_dir,
        GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT,
        "activate-atomic-batches",
    );
    activate(
        &data_dir,
        GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT,
        "activate-pricing-rules",
    );

    // An issued asset held by a user who signs her own burn.
    let issuer = dev_key();
    let holder = dev_key();
    let holder_key_file = harness.root.join("holder.key.json");
    write_private(
        &holder_key_file,
        serde_json::to_string_pretty(&holder).expect("key json"),
    );
    fund(&data_dir, &issuer.address, "fund-issuer");
    fund(&data_dir, &holder.address, "fund-holder");
    asset_operation(
        &data_dir,
        &issuer,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.address.clone(),
            code: "A666".to_string(),
            version: 1,
            precision: 6,
            display_name: "NAVCoin a666".to_string(),
            max_supply: Some(1_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        "asset-create",
    );
    let asset_id = NodeStore::new(&data_dir)
        .read_ledger()
        .expect("ledger")
        .asset_definitions[0]
        .asset_id
        .clone();
    asset_operation(
        &data_dir,
        &holder,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            limit: 100,
            authorized: false,
            frozen: false,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
        "trust-set",
    );
    asset_operation(
        &data_dir,
        &issuer,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: holder.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            amount: 40,
        }),
        "issue",
    );

    // Two validators, each a transport service plus rpc-serve with the admission switch.
    let validator_dirs: Vec<PathBuf> = (0..VALIDATORS)
        .map(|index| harness.root.join(format!("validator-{index}")))
        .collect();
    for (index, dir) in validator_dirs.iter().enumerate().skip(1) {
        copy_dir(&data_dir, dir);
        let store = NodeStore::new(dir);
        let mut state = store.read_node_state().expect("node state");
        state.node_id = format!("validator-{index}");
        store.write_node_state(&state).expect("node identity");
    }
    let genesis = NodeStore::new(&data_dir).read_genesis().expect("genesis");
    let genesis_hash_hex = genesis_hash(&genesis);
    let topology = local_topology(
        NetworkDomain {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash_hex.clone(),
            protocol_version: genesis.protocol_version,
        },
        VALIDATORS as u32,
        free_base_port(),
    )
    .expect("two-validator topology");
    let rpc_ports: Vec<u16> = topology.peers.iter().map(|peer| peer.rpc_port).collect();
    let rpc_port = rpc_ports[0];
    let topology_path = harness.root.join("topology.json");
    fs::write(
        &topology_path,
        serde_json::to_string_pretty(&topology).expect("topology json"),
    )
    .expect("write topology");
    let path = |p: &Path| p.to_str().expect("UTF-8 path").to_string();
    let mut ready_files = Vec::new();
    for (index, dir) in validator_dirs.iter().enumerate() {
        let validator = format!("validator-{index}");
        let key = split_validator_key(dir, &validator);
        let transport_ready = harness
            .root
            .join(format!("{validator}.transport.ready.json"));
        harness.children.push(
            Command::new(node_bin())
                .env("POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE", &transport_ready)
                // Transport readiness requires warm shielded, swap and private-egress verifiers.
                .env("POSTFIAT_PREWARM_SHIELDED_VERIFIER", "1")
                .env("POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER", "1")
                .env(
                    "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER",
                    "1",
                )
                .args([
                    "transport-validator-serve",
                    "--unsafe-devnet-file-signer",
                    "--unsafe-devnet-json-storage",
                    "--data-dir",
                    &path(dir),
                    "--topology",
                    &path(&topology_path),
                    "--key-file",
                    &path(&key),
                    "--vote-dir",
                    &path(&harness.root.join(format!("{validator}.votes"))),
                    "--max-connections",
                    "100",
                    "--timeout-ms",
                    "60000",
                ])
                .stdout(Stdio::null())
                .stderr(
                    fs::File::create(harness.root.join(format!("{validator}.transport.log")))
                        .expect("log"),
                )
                .spawn()
                .expect("spawn transport service"),
        );
        let rpc_ready = harness.root.join(format!("{validator}.rpc.ready.json"));
        harness.children.push(
            Command::new(node_bin())
                .args([
                    "rpc-serve",
                    "--unsafe-devnet-json-storage",
                    "--data-dir",
                    &path(dir),
                    "--spool-dir",
                    &path(&harness.root.join(format!("{validator}.rpc-spool"))),
                    "--ready-file",
                    &path(&rpc_ready),
                    "--port",
                    &rpc_ports[index].to_string(),
                    "--max-requests",
                    "10000",
                    "--timeout-ms",
                    "60000",
                    "--child-timeout-ms",
                    "60000",
                    "--allow-private-action-admission",
                    "--finality-topology",
                    &path(&topology_path),
                    "--finality-key-file",
                    &path(&key),
                    "--finality-artifact-root",
                    &path(&harness.root.join(format!("{validator}.finality"))),
                    "--finality-timeout-ms",
                    "60000",
                    "--finality-send-retries",
                    "2",
                    "--finality-retry-backoff-ms",
                    "25",
                    "--finality-quorum-early-full-propagation",
                    "--keep-alive",
                ])
                .stdout(Stdio::null())
                .stderr(
                    fs::File::create(harness.root.join(format!("{validator}.rpc.log")))
                        .expect("log"),
                )
                .spawn()
                .expect("spawn rpc-serve"),
        );
        ready_files.push(transport_ready);
        ready_files.push(rpc_ready);
    }
    for ready in &ready_files {
        wait_for_file(ready, Duration::from_secs(180));
    }

    // The wallet proxy with both validators as its sources (the default: all of RPC_FLEET).
    let proxy_port = free_port();
    let secret = "integration-native-session-secret-32-bytes-01";
    let proxy_home = harness.root.join("proxy-home");
    let gateway_store = harness.root.join("gateway-store");
    fs::create_dir_all(&proxy_home).expect("proxy home");
    // The proxy's whole environment, every setting at its most permissive value.
    let rpc_fleet = rpc_ports
        .iter()
        .enumerate()
        .map(|(index, port)| format!("validator-{index}=127.0.0.1:{port}"))
        .collect::<Vec<_>>()
        .join(",");
    let proxy_env: Vec<(String, String)> = [
        ("PATH", std::env::var("PATH").unwrap_or_default()),
        ("HOME", path(&proxy_home)),
        ("LISTEN_HOST", "127.0.0.1".to_string()),
        ("LISTEN_PORT", proxy_port.to_string()),
        ("ALLOWED_ORIGINS", ORIGIN.to_string()),
        ("WALLET_PROXY_API_TOKEN", BROWSER_TOKEN.to_string()),
        ("RPC_HOST", "127.0.0.1".to_string()),
        ("RPC_PORT", rpc_port.to_string()),
        ("RPC_FLEET", rpc_fleet),
        ("NAVSWAP_IDEMPOTENCY_STORE_PATH", "off".to_string()),
        ("NAVSWAP_RUN_STORE_PATH", "off".to_string()),
        ("WALLET_PROXY_MAX_HTTP_BODY_BYTES", "16777216".to_string()),
        ("WALLET_PROXY_MUTATION_RATE_LIMIT", "100000".to_string()),
        ("WALLET_PROXY_MUTATION_RATE_WINDOW_MS", "1".to_string()),
        ("WALLET_PROXY_MUTATION_CONCURRENCY", "4096".to_string()),
        ("WALLET_PROXY_NATIVE_SESSION_ORIGIN", ORIGIN.to_string()),
        ("WALLET_PROXY_NATIVE_SESSION_SECRET", secret.to_string()),
        (
            "WALLET_PROXY_NATIVE_SESSION_CHAIN_ID",
            genesis.chain_id.clone(),
        ),
        (
            "WALLET_PROXY_NATIVE_SESSION_GENESIS_HASH",
            genesis_hash_hex.clone(),
        ),
        ("WALLET_PROXY_NATIVE_SESSION_TTL_MS", "3600000".to_string()),
        (
            "WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_LIMIT",
            "100000".to_string(),
        ),
        (
            "WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID",
            genesis.chain_id.clone(),
        ),
        (
            "WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH",
            genesis_hash_hex.clone(),
        ),
        (
            "WALLET_PROXY_PRIVATE_ACTIONS_BASE_ASSET_ID",
            asset_id.clone(),
        ),
        (
            "WALLET_PROXY_PRIVATE_ACTIONS_QUOTE_ASSET_ID",
            asset_id.clone(),
        ),
        ("WALLET_PROXY_PRIVATE_ACTIONS_STORE", path(&gateway_store)),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value))
    .collect();
    let proxy = Command::new("node")
        .arg("server.js")
        .current_dir(proxy_dir())
        .env_clear()
        .envs(proxy_env.iter().map(|(name, value)| (name, value)))
        .stdout(fs::File::create(harness.root.join("proxy.stdout.log")).expect("log"))
        .stderr(fs::File::create(harness.root.join("proxy.stderr.log")).expect("log"))
        .spawn()
        .expect("spawn wallet proxy");
    let proxy_pid = proxy.id();
    harness.children.push(proxy);
    let deadline = Instant::now() + Duration::from_secs(30);
    while TcpStream::connect(("127.0.0.1", proxy_port)).is_err() {
        assert!(Instant::now() < deadline, "wallet proxy did not start");
        thread::sleep(Duration::from_millis(50));
    }

    // A native session opened with a Rust-signed install identity.
    let install = ml_dsa_65_keygen().expect("install key");
    let public_key_hex = bytes_to_hex(&install.public_key);
    let nonce = bytes_to_hex(&nanos.to_le_bytes().repeat(2));
    let issued_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    let message = [
        "postfiat-native-session-open-v1",
        ORIGIN,
        &genesis.chain_id,
        &genesis_hash_hex,
        &public_key_hex,
        &nonce,
        &issued_at_ms.to_string(),
    ]
    .join("\n");
    let signature = ml_dsa_65_sign_with_context_seed(
        &install.private_key,
        message.as_bytes(),
        b"postfiat-l1-v2/native-session/v1",
        &[0x53; 32],
    )
    .expect("sign open");
    let (status, session) = http(
        proxy_port,
        "/api/native-session/v1/open",
        &json!({
            "schema": "postfiat-native-session-open-v1", "algorithm_id": "ML-DSA-65",
            "public_key_hex": public_key_hex, "client_nonce_hex": nonce, "origin": ORIGIN,
            "chain_id": genesis.chain_id, "genesis_hash_hex": genesis_hash_hex,
            "issued_at_ms": issued_at_ms, "signature_hex": bytes_to_hex(&signature),
        }),
        "",
    );
    assert_eq!(status, 200, "native session open: {session}");
    let token = session["token"]
        .as_str()
        .expect("session token")
        .to_string();

    // Snapshot of the empty pool.
    let (status, empty) = http(
        proxy_port,
        "/api/private/v1/snapshot",
        &json!({"address": holder.address}),
        &token,
    );
    assert_eq!(status, 200, "empty snapshot: {empty}");
    assert_eq!(empty["commitments"], json!([]));
    assert_eq!(empty["outputs"], json!([]));
    assert_eq!(empty["complete"]["commitments"], true, "{empty}");
    assert_eq!(empty["complete"]["outputs"], true, "{empty}");
    assert_eq!(empty["domain"]["genesis_hash"], genesis_hash_hex.as_str());

    // A user-built ingress v2: the exact public JSON the wallet would persist.
    let note_seed =
        postfiat_crypto_provider::hash_bytes("gateway-test-note-seed", &nanos.to_le_bytes())[..32]
            .to_vec();
    let ingress_file = harness.root.join("ingress.json");
    create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
        data_dir: data_dir.clone(),
        key_file: holder_key_file.clone(),
        asset_id: asset_id.clone(),
        amount: 9,
        fee: 0,
        note_seed_hex: bytes_to_hex(&note_seed),
        encrypted_output_hex: None,
        ingress_file: ingress_file.clone(),
        note_file: harness.root.join("note.json"),
        overwrite: false,
    })
    .expect("create ingress");
    let file: AssetOrchardIngressFile =
        serde_json::from_slice(&fs::read(&ingress_file).expect("read ingress"))
            .expect("ingress json");
    let output_commitment = file.payload.output_commitment.clone();
    let encrypted_output = file.payload.encrypted_output.clone();
    let action_json = serde_json::to_string(&ShieldedAction::AssetOrchardIngressV2(file.payload))
        .expect("action json");
    let domain = json!([{"chain_id": genesis.chain_id, "genesis_hash": genesis_hash_hex, "protocol_version": genesis.protocol_version}, action_json]);
    let intent_id = sha3_hex("tasknode.private-intent.v1", domain.to_string().as_bytes());
    let body =
        json!({"address": holder.address, "intent_id": intent_id, "action_json": action_json});

    let (status, submitted) = http(proxy_port, "/api/private/v1/submit", &body, &token);
    assert_eq!(status, 202, "submit: {submitted}");
    assert_eq!(submitted["intent_id"], intent_id.as_str());
    assert!(
        ["queued", "dispatched", "accepted"].contains(&submitted["state"].as_str().unwrap_or("")),
        "{submitted}"
    );

    // The validator's own round applies it; the receipt carries finality.
    let deadline = Instant::now() + Duration::from_secs(120);
    let receipt = loop {
        let (status, receipt) = http(proxy_port, "/api/private/v1/receipt", &body, &token);
        assert_eq!(status, 200, "receipt: {receipt}");
        if receipt["state"] == "accepted" {
            break receipt;
        }
        assert!(receipt["state"] != "rejected", "rejected: {receipt}");
        assert!(Instant::now() < deadline, "not accepted in time: {receipt}");
        thread::sleep(Duration::from_millis(500));
    };
    assert_eq!(receipt["accepted"], true);
    assert_eq!(receipt["batch_id"], submitted["batch_id"]);
    let finality = &receipt["finality"];
    assert_eq!(finality["kind"], "validator_quorum_block_certificate_v1");
    assert_eq!(finality["height"], receipt["inclusion_height"]);
    assert_eq!(finality["block_hash"], receipt["block_hash"]);
    let height = receipt["inclusion_height"]
        .as_u64()
        .expect("inclusion height");

    // The new commitment and its ciphertext are on the next snapshot.
    let (status, after) = http(
        proxy_port,
        "/api/private/v1/snapshot",
        &json!({"address": holder.address}),
        &token,
    );
    assert_eq!(status, 200, "snapshot after ingress: {after}");
    assert!(after["snapshot_token"]["height"].as_u64().unwrap() >= height);
    assert_eq!(after["commitments"], json!([output_commitment]));
    assert_eq!(
        after["outputs"][0]["commitment"],
        output_commitment.as_str()
    );
    assert_eq!(after["outputs"][0]["ciphertext"], encrypted_output.as_str());

    // The same body again: the same record, no second round.
    let (status, again) = http(proxy_port, "/api/private/v1/submit", &body, &token);
    assert_eq!(status, 202, "{again}");
    assert_eq!(again["intent_id"], intent_id.as_str());
    thread::sleep(Duration::from_millis(1_500));
    let tip = rpc(rpc_port, "server_info", json!({}));
    assert_eq!(
        tip["result"]["ledger"]["height"].as_u64(),
        Some(height),
        "a second round ran: {tip}"
    );

    // Changed bytes under the intent are refused; an unknown intent stays pending.
    let changed = action_json.replacen("\"amount\":9", "\"amount\":8", 1);
    assert_ne!(changed, action_json);
    let (status, refused) = http(
        proxy_port,
        "/api/private/v1/submit",
        &json!({"address": holder.address, "intent_id": intent_id, "action_json": changed}),
        &token,
    );
    assert_eq!(status, 400, "{refused}");
    assert_eq!(refused["code"], "private_intent_mismatch");
    let unknown_domain = json!([{"chain_id": genesis.chain_id, "genesis_hash": genesis_hash_hex, "protocol_version": genesis.protocol_version}, changed]);
    let unknown = json!({"address": holder.address, "intent_id": sha3_hex("tasknode.private-intent.v1", unknown_domain.to_string().as_bytes()), "action_json": changed});
    let (status, pending) = http(proxy_port, "/api/private/v1/receipt", &unknown, &token);
    assert_eq!(status, 200, "{pending}");
    assert_eq!(pending["state"], "unknown");
    assert_eq!(pending["finality"], Value::Null);
    let proxy_fds_after_flow = open_paths(proxy_pid);

    // S7.2 (3): verification cannot be turned off. A tampered ingress (one hex
    // digit of the burn signature flipped) passes the gateway's shape checks
    // under the most permissive settings, and the node refuses it itself.
    let tampered_seed =
        postfiat_crypto_provider::hash_bytes("gateway-test-tampered-seed", &nanos.to_le_bytes())
            [..32]
            .to_vec();
    let tampered_file = harness.root.join("tampered-ingress.json");
    create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
        data_dir: data_dir.clone(),
        key_file: holder_key_file.clone(),
        asset_id: asset_id.clone(),
        amount: 7,
        fee: 0,
        note_seed_hex: bytes_to_hex(&tampered_seed),
        encrypted_output_hex: None,
        ingress_file: tampered_file.clone(),
        note_file: harness.root.join("tampered-note.json"),
        overwrite: false,
    })
    .expect("create second ingress");
    let mut tampered_payload = serde_json::from_slice::<AssetOrchardIngressFile>(
        &fs::read(&tampered_file).expect("read second ingress"),
    )
    .expect("second ingress json")
    .payload;
    let signature = &mut tampered_payload.burn_transaction.signature_hex;
    let flipped = if signature.starts_with('0') { "1" } else { "0" };
    signature.replace_range(0..1, flipped);
    let tampered_json =
        serde_json::to_string(&ShieldedAction::AssetOrchardIngressV2(tampered_payload))
            .expect("tampered json");
    let tampered_domain = json!([{"chain_id": genesis.chain_id, "genesis_hash": genesis_hash_hex, "protocol_version": genesis.protocol_version}, tampered_json]);
    let tampered_intent = sha3_hex(
        "tasknode.private-intent.v1",
        tampered_domain.to_string().as_bytes(),
    );
    let tampered_body = json!({"address": holder.address, "intent_id": tampered_intent, "action_json": tampered_json});
    let (status, tampered) = http(proxy_port, "/api/private/v1/submit", &tampered_body, &token);
    assert_eq!(status, 202, "condition: tampered submit through the gateway; observed: {status} {tampered}; expected: 202 (the gateway does not verify signatures)");
    assert_eq!(tampered["state"], "queued", "condition: tampered ingress at the node; observed: {tampered}; expected: queued, nothing reserved");
    let (_, tampered_receipt) = http(
        proxy_port,
        "/api/private/v1/receipt",
        &tampered_body,
        &token,
    );
    let node_code = tampered_receipt["code"].as_str().unwrap_or("").to_string();
    assert!(
        !node_code.is_empty() && tampered_receipt["accepted"].is_null() && tampered_receipt["finality"].is_null(),
        "condition: receipt of the tampered intent; observed: {tampered_receipt}; expected: pending with the node's refusal code"
    );
    let tampered_action_hash = tampered["action_hash"]
        .as_str()
        .expect("action hash")
        .to_string();
    for (index, port) in rpc_ports.iter().enumerate() {
        let direct = rpc(
            *port,
            "private_action_submit",
            json!({
                "chain_id": genesis.chain_id, "genesis_hash": genesis_hash_hex, "protocol_version": genesis.protocol_version,
                "pool_id": "asset-orchard-v1", "session_scope": session["principal"], "intent_id": tampered_intent,
                "action_json": tampered_json, "action_hash": tampered_action_hash,
            }),
        );
        assert_eq!(direct["ok"], false, "condition: validator-{index} given the tampered ingress directly; observed: {direct}; expected: refused");
        assert_eq!(direct["error"]["code"].as_str(), Some(node_code.as_str()), "condition: validator-{index} refusal code; observed: {direct}; expected: the code the gateway reported");
        let receipt = rpc(
            *port,
            "private_action_receipt",
            json!({"intent_id": tampered_intent}),
        );
        assert_eq!(receipt["result"]["state"], "pending_unknown", "condition: validator-{index} record of the tampered intent; observed: {receipt}; expected: none");
    }
    thread::sleep(Duration::from_millis(1_500));
    let tip = rpc(rpc_port, "server_info", json!({}));
    assert_eq!(
        tip["result"]["ledger"]["height"].as_u64(),
        Some(height),
        "condition: chain after the tampered submit; observed: {tip}; expected: no new block"
    );
    eprintln!("tampered ingress refused by the node with code {node_code}");

    // S7.2 (2): the gateway process holds no validator key and never opens the
    // validators' data directories.
    let validator_roots: Vec<String> = validator_dirs.iter().map(|dir| path(dir)).collect();
    let validator_secret_hex: Vec<String> = validator_dirs
        .iter()
        .flat_map(|dir| validator_private_keys(dir))
        .collect();
    for (name, value) in &proxy_env {
        let lower = name.to_ascii_lowercase();
        assert!(
            !lower.contains("key_file") && !lower.contains("data_dir") && !lower.contains("private_key"),
            "condition: proxy environment variable names; observed: {name}; expected: no key file, data directory or private key setting"
        );
        assert!(
            !validator_roots.iter().any(|root| value.contains(root.as_str()))
                && !value.contains("validator_keys")
                && !validator_secret_hex.iter().any(|key| value.contains(key.as_str())),
            "condition: proxy environment variable {name}; observed: a validator path or key; expected: none"
        );
    }
    #[cfg(target_os = "linux")]
    {
        let environ = fs::read(format!("/proc/{proxy_pid}/environ")).expect("proxy environ");
        let mut names: Vec<String> = environ
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                String::from_utf8_lossy(entry)
                    .split('=')
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        names.sort();
        let mut expected: Vec<String> = proxy_env.iter().map(|(name, _)| name.clone()).collect();
        expected.sort();
        assert_eq!(names, expected, "condition: the live proxy environment; observed: other variables; expected: exactly the test's list");
        for (label, paths) in [
            ("after the flow", proxy_fds_after_flow),
            ("at the end", open_paths(proxy_pid)),
        ] {
            let inside: Vec<&String> = paths
                .iter()
                .filter(|open| {
                    validator_roots
                        .iter()
                        .any(|root| open.starts_with(root.as_str()))
                })
                .collect();
            assert!(inside.is_empty(), "condition: files the proxy holds open {label}; observed: {} under a validator data directory; expected: none", inside.len());
        }
    }

    // S7.2 (1): no secret in anything the proxy or the nodes wrote.
    let mut secrets: Vec<(&str, Vec<Vec<u8>>)> = vec![
        (
            "install identity secret key",
            byte_forms(&install.private_key),
        ),
        ("native session secret", vec![secret.as_bytes().to_vec()]),
        ("native bearer token", vec![token.as_bytes().to_vec()]),
        (
            "browser relay token",
            vec![BROWSER_TOKEN.as_bytes().to_vec()],
        ),
        ("issuer secret key", hex_forms(&issuer.private_key_hex)),
        ("holder secret key", hex_forms(&holder.private_key_hex)),
        ("note seed", byte_forms(&note_seed)),
        ("note seed", byte_forms(&tampered_seed)),
    ];
    for note in ["note.json", "tampered-note.json"] {
        let opening: Value =
            serde_json::from_slice(&fs::read(harness.root.join(note)).expect("note"))
                .expect("note json");
        for field in ["rho", "psi", "rcm"] {
            let value = opening["note"][field]
                .as_str()
                .expect("opening field")
                .to_string();
            secrets.push(("note opening field", hex_forms(&value)));
        }
    }
    for dir in &validator_dirs {
        for key in validator_private_keys(dir) {
            secrets.push(("validator secret key", hex_forms(&key)));
        }
        secrets.push((
            "validator key file",
            vec![fs::read(dir.join("validator_keys.json")).expect("key file")],
        ));
    }
    let mut scanned = Vec::new();
    for root in [proxy_home.clone(), gateway_store.clone()] {
        collect_files(&root, &mut scanned);
    }
    scanned.push(harness.root.join("proxy.stdout.log"));
    scanned.push(harness.root.join("proxy.stderr.log"));
    for (index, dir) in validator_dirs.iter().enumerate() {
        let mut node_files = Vec::new();
        collect_files(dir, &mut node_files);
        // The key files the test and init placed there are the keys themselves.
        node_files.retain(|file| {
            let name = file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            !name.ends_with("validator_keys.json") && name != "faucet_key.json"
        });
        scanned.extend(node_files);
        for extra in ["finality", "votes", "rpc-spool"] {
            collect_files(
                &harness.root.join(format!("validator-{index}.{extra}")),
                &mut scanned,
            );
        }
        scanned.push(harness.root.join(format!("validator-{index}.rpc.log")));
        scanned.push(
            harness
                .root
                .join(format!("validator-{index}.transport.log")),
        );
    }
    // Positive control: the scanner finds planted secrets in each encoding.
    let control = harness.root.join("scanner-control.bin");
    let mut planted = base64(&install.private_key).into_bytes();
    planted.extend_from_slice(holder.private_key_hex.to_ascii_uppercase().as_bytes());
    planted.extend_from_slice(&note_seed);
    fs::write(&control, &planted).expect("write control");
    let planted_classes: Vec<&str> = secrets
        .iter()
        .filter(|(_, forms)| forms.iter().any(|form| contains(&planted, form)))
        .map(|(class, _)| *class)
        .collect();
    assert!(
        ["install identity secret key", "holder secret key", "note seed"]
            .iter()
            .all(|class| planted_classes.contains(class)),
        "condition: scanner positive control; observed: {planted_classes:?}; expected: all three planted classes found"
    );
    let mut findings = Vec::new();
    for file in &scanned {
        let Ok(bytes) = fs::read(file) else { continue };
        for (class, forms) in &secrets {
            if forms
                .iter()
                .any(|form| !form.is_empty() && contains(&bytes, form))
            {
                findings.push(format!("{}: {class}", file.display()));
            }
        }
    }
    findings.sort();
    findings.dedup();
    assert!(
        findings.is_empty(),
        "condition: {} files the proxy and nodes wrote; observed: secrets in {:?} (values redacted); expected: none",
        scanned.len(),
        findings
    );
    assert!(scanned.len() > 20, "condition: inspection coverage; observed: {} files; expected: the stores, logs and round artifacts", scanned.len());
    eprintln!("secret inspection scanned {} files: clean", scanned.len());
    eprintln!(
        "private actions gateway integration took {:?}",
        started.elapsed()
    );
}

/// Paths the process holds open (Linux `/proc/<pid>/fd`).
fn open_paths(pid: u32) -> Vec<String> {
    let Ok(entries) = fs::read_dir(format!("/proc/{pid}/fd")) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| fs::read_link(entry.ok()?.path()).ok())
        .map(|target| target.to_string_lossy().into_owned())
        .collect()
}

fn validator_private_keys(dir: &Path) -> Vec<String> {
    let keys: Value =
        serde_json::from_slice(&fs::read(dir.join("validator_keys.json")).expect("keys"))
            .expect("keys json");
    keys["validators"]
        .as_array()
        .expect("validator keys")
        .iter()
        .filter_map(|record| record["private_key_hex"].as_str().map(str::to_string))
        .collect()
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(ALPHABET[((n >> shift) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Raw bytes, lower and upper hex, and base64 of a secret.
fn byte_forms(bytes: &[u8]) -> Vec<Vec<u8>> {
    let hex = bytes_to_hex(bytes);
    vec![
        bytes.to_vec(),
        hex.clone().into_bytes(),
        hex.to_ascii_uppercase().into_bytes(),
        base64(bytes).into_bytes(),
        base64(bytes).trim_end_matches('=').as_bytes().to_vec(),
    ]
}

fn hex_forms(hex: &str) -> Vec<Vec<u8>> {
    let bytes = hex_to_bytes(hex).expect("secret hex");
    byte_forms(&bytes)
}

/// S7.2 (3): no switch on the private action path turns verification off.
#[test]
fn private_action_path_has_no_verification_switch() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![
        repo.join("wallet-proxy/private-actions.js"),
        repo.join("wallet-proxy/native-session.js"),
        repo.join("wallet-proxy/server.js"),
        repo.join("crates/node/src/asset_orchard_pricing_reads.rs"),
        repo.join("crates/node/src/rpc_serve_runtime.rs"),
        repo.join("crates/node/src/main_parts/cli_dispatch.rs"),
        repo.join("crates/node/src/main_parts/cli_dispatch_parts/group_03.rs"),
    ];
    for entry in fs::read_dir(repo.join("crates/node/src")).expect("node sources") {
        let path = entry.expect("entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with("private_") && name.ends_with(".rs") {
            files.push(path);
        }
    }
    let upper = [
        "SKIP_VERIF",
        "NO_VERIF",
        "DISABLE_VERIF",
        "UNSAFE",
        "ALLOW_INVALID",
        "INSECURE",
        "BYPASS",
    ];
    let lower = [
        "skip-verif",
        "no-verif",
        "disable-verif",
        "unsafe",
        "allow-invalid",
        "insecure",
        "bypass",
    ];
    // Documented test-only switches on this path; none disables verification.
    let documented: [&str; 0] = [];
    let mut found = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("source file");
        for token in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
            let env_name = token.len() > 3
                && token
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                && upper.iter().any(|word| token.contains(word));
            let flag = token.starts_with("--") && lower.iter().any(|word| token.contains(word));
            if (env_name || flag) && !documented.contains(&token) {
                found.push(format!(
                    "{}: {token}",
                    file.strip_prefix(&repo).unwrap_or(file).display()
                ));
            }
        }
    }
    // The gateway's private action module names no key file or validator data directory.
    let gateway =
        fs::read_to_string(repo.join("wallet-proxy/private-actions.js")).expect("gateway source");
    for marker in [
        "validator_keys",
        "key_file",
        "--key-file",
        "KEY_FILE",
        "DATA_DIR",
        "--data-dir",
    ] {
        if gateway.contains(marker) {
            found.push(format!("wallet-proxy/private-actions.js: {marker}"));
        }
    }
    found.sort();
    found.dedup();
    assert!(
        found.is_empty(),
        "condition: environment variables and CLI flags on the private action path ({} files); observed: {found:?}; expected: none that could disable verification",
        files.len()
    );
}
