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
    harness.children.push(
        Command::new("node")
            .arg("server.js")
            .current_dir(proxy_dir())
            .env("HOME", &harness.root)
            .env("LISTEN_HOST", "127.0.0.1")
            .env("LISTEN_PORT", proxy_port.to_string())
            .env("ALLOWED_ORIGINS", ORIGIN)
            .env("WALLET_PROXY_API_TOKEN", BROWSER_TOKEN)
            .env("RPC_HOST", "127.0.0.1")
            .env("RPC_PORT", rpc_port.to_string())
            .env(
                "RPC_FLEET",
                rpc_ports
                    .iter()
                    .enumerate()
                    .map(|(index, port)| format!("validator-{index}=127.0.0.1:{port}"))
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .env("NAVSWAP_IDEMPOTENCY_STORE_PATH", "off")
            .env("NAVSWAP_RUN_STORE_PATH", "off")
            .env("WALLET_PROXY_NATIVE_SESSION_ORIGIN", ORIGIN)
            .env("WALLET_PROXY_NATIVE_SESSION_SECRET", secret)
            .env("WALLET_PROXY_NATIVE_SESSION_CHAIN_ID", &genesis.chain_id)
            .env(
                "WALLET_PROXY_NATIVE_SESSION_GENESIS_HASH",
                &genesis_hash_hex,
            )
            .env("WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID", &genesis.chain_id)
            .env(
                "WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH",
                &genesis_hash_hex,
            )
            .env("WALLET_PROXY_PRIVATE_ACTIONS_BASE_ASSET_ID", &asset_id)
            .env("WALLET_PROXY_PRIVATE_ACTIONS_QUOTE_ASSET_ID", &asset_id)
            .env(
                "WALLET_PROXY_PRIVATE_ACTIONS_STORE",
                harness.root.join("gateway-store"),
            )
            .stdout(Stdio::null())
            .stderr(fs::File::create(harness.root.join("proxy.log")).expect("log"))
            .spawn()
            .expect("spawn wallet proxy"),
    );
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
    let ingress_file = harness.root.join("ingress.json");
    create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
        data_dir: data_dir.clone(),
        key_file: holder_key_file.clone(),
        asset_id: asset_id.clone(),
        amount: 9,
        fee: 0,
        note_seed_hex: bytes_to_hex(&[0x51; 32]),
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
    eprintln!(
        "private actions gateway integration took {:?}",
        started.elapsed()
    );
}
