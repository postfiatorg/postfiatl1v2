use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_node::{init, InitOptions};
use postfiat_rpc_sdk::{
    owned_objects_request, server_info_request, validate_response_kind, verify_bridge_request,
    OwnedObjectsParams, RpcRequest, RpcResponse, RpcResponseKind,
};

fn node_bin() -> &'static str {
    env!("CARGO_BIN_EXE_postfiat-node")
}

fn unique_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "postfiat-fastpay-default-rpc-{}-{nanos}",
        std::process::id()
    ))
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral local address")
        .port()
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if path.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {}", path.display());
}

fn spawn_rpc(data_dir: &Path, ready_file: &Path, port: u16, disabled: bool) -> Child {
    spawn_rpc_serving(data_dir, ready_file, port, disabled, 1)
}

fn spawn_rpc_serving(
    data_dir: &Path,
    ready_file: &Path,
    port: u16,
    disabled: bool,
    max_requests: usize,
) -> Child {
    let mut command = Command::new(node_bin());
    let spool_dir = data_dir.join("rpc-spool");
    let port = port.to_string();
    let max_requests = max_requests.to_string();
    command.args([
        "rpc-serve",
        "--unsafe-devnet-json-storage",
        "--data-dir",
        data_dir.to_str().expect("data directory UTF-8"),
        "--spool-dir",
        spool_dir.to_str().expect("spool directory UTF-8"),
        "--ready-file",
        ready_file.to_str().expect("ready file UTF-8"),
        "--bind-host",
        "127.0.0.1",
        "--port",
        &port,
        "--max-requests",
        &max_requests,
        "--timeout-ms",
        "10000",
        "--child-timeout-ms",
        "10000",
    ]);
    if disabled {
        command.arg("--disable-owned-lane");
    }
    command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn RPC server")
}

fn rpc_call(port: u16, request: &RpcRequest) -> RpcResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect RPC server");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("set RPC timeout");
    serde_json::to_writer(&mut stream, request).expect("write RPC request");
    stream.write_all(b"\n").expect("terminate RPC request");
    stream.flush().expect("flush RPC request");
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .expect("read RPC response");
    serde_json::from_str(&response).expect("decode RPC response")
}

fn assert_server_capability(data_dir: &Path, root: &Path, disabled: bool) {
    let port = free_port();
    let ready = root.join(if disabled {
        "disabled.ready.json"
    } else {
        "default.ready.json"
    });
    let mut child = spawn_rpc(data_dir, &ready, port, disabled);
    wait_for_file(&ready);
    let response = rpc_call(port, &server_info_request("fastpay-capability"));
    assert!(response.ok, "server_info failed: {:?}", response.error);
    let result = response.result.expect("server_info result");
    assert_eq!(result["rpc"]["owned_lane_enabled"], !disabled);
    assert_eq!(result["rpc"]["read_only"], disabled);
    let domain = &result["rpc"]["owned_certificate_domain"];
    assert_eq!(
        domain["schema"],
        postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2
    );
    assert_eq!(domain["chain_id"], "postfiat-fastpay-default-rpc-test");
    assert_eq!(domain["protocol_version"], 1);
    assert_eq!(
        domain["genesis_hash"]
            .as_str()
            .expect("FastPay genesis hash")
            .len(),
        96
    );
    assert_eq!(
        domain["registry_id"]
            .as_str()
            .expect("FastPay registry id")
            .len(),
        96
    );
    let status = child.wait().expect("wait for RPC server");
    assert!(status.success(), "RPC server failed with {status}");
}

#[test]
fn signed_fastpay_rpc_is_enabled_by_default_and_explicitly_disableable() {
    let root = unique_root();
    let data_dir = root.join("node");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-fastpay-default-rpc-test".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize RPC test node");

    assert_server_capability(&data_dir, &root, false);
    assert_server_capability(&data_dir, &root, true);

    fs::remove_dir_all(root).expect("remove RPC test directory");
}

/// Regression for issue #27: `rpc-serve` workers change their working
/// directory to the data directory, so a relative `--data-dir` (and a
/// relative explicit `--spool-dir`) used to make every child-dispatched
/// method fail with "No such file or directory" and answer under the CLI
/// fallback id `local-1` instead of the request id.
#[test]
fn rpc_child_dispatch_works_with_relative_data_and_spool_directories() {
    let root = unique_root();
    let data_dir = root.join("node");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-relative-data-dir-test".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize RPC test node");

    let absolute_data_dir = data_dir.to_str().expect("data directory UTF-8");
    let cases: [(&str, &str, Option<&str>); 3] = [
        ("relative data dir, default spool", "node", None),
        ("relative data dir, relative spool", "node", Some("spool")),
        ("absolute data dir, default spool", absolute_data_dir, None),
    ];
    for (label, data_dir_arg, spool_dir_arg) in cases {
        let port = free_port();
        let ready = data_dir.join("readiness/rpc.ready.json");
        let mut command = Command::new(node_bin());
        command
            .current_dir(&root)
            .args(["rpc-serve", "--unsafe-devnet-json-storage", "--data-dir"])
            .arg(data_dir_arg);
        if let Some(spool_dir_arg) = spool_dir_arg {
            command.args(["--spool-dir", spool_dir_arg]);
        }
        let mut child = command
            .args(["--port", &port.to_string(), "--max-requests", "1"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn RPC server");
        wait_for_file(&ready);
        // server_info is answered by an isolated worker, not in-process.
        let request_id = "relative-data-dir-probe";
        let response = rpc_call(port, &server_info_request(request_id));
        let status = child.wait().expect("wait for RPC server");
        assert!(status.success(), "{label}: RPC server failed with {status}");
        assert_eq!(
            response.id, request_id,
            "{label}: response id must echo the request id"
        );
        assert!(response.ok, "{label}: {:?}", response.error);
        assert_eq!(
            response.result.expect("server_info result")["chain_id"],
            "postfiat-relative-data-dir-test",
            "{label}"
        );
        fs::remove_file(&ready).expect("remove readiness file between runs");
    }

    fs::remove_dir_all(root).expect("remove RPC test directory");
}

/// Regression for issue #160: with no bridge transfer, `verify_bridge`'s
/// report has an empty `latest_transfer_id`. Using it as the event subject
/// made the worker's response fail `rpc-serve`'s protocol check, so the
/// public read always answered `rpc_server_error` "RPC worker failed".
#[test]
fn verify_bridge_answers_over_rpc_serve_without_bridge_transfers() {
    let root = unique_root();
    let data_dir = root.join("node");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-verify-bridge-rpc-test".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize RPC test node");

    let port = free_port();
    let ready = root.join("verify-bridge.ready.json");
    let mut child = spawn_rpc(&data_dir, &ready, port, false);
    wait_for_file(&ready);
    let request_id = "verify-bridge-no-transfers";
    let response = rpc_call(port, &verify_bridge_request(request_id));
    let status = child.wait().expect("wait for RPC server");
    assert!(status.success(), "RPC server failed with {status}");

    assert!(response.ok, "verify_bridge failed: {:?}", response.error);
    assert_eq!(response.id, request_id);
    response
        .validate_protocol()
        .expect("verify_bridge response is protocol valid");
    validate_response_kind(&response, RpcResponseKind::VerifyBridge)
        .expect("verify_bridge result validates");
    let result = response.result.as_ref().expect("verify_bridge result");
    assert_eq!(result["transfer_count"], 0);
    assert_eq!(result["latest_transfer_id"], "");

    fs::remove_dir_all(root).expect("remove RPC test directory");
}

/// The defect class of issue #160 across every public read that takes no
/// parameter: on a fresh node none may build an event from an empty value,
/// which `rpc-serve`'s protocol check would turn into `rpc_server_error`.
#[test]
fn parameterless_public_reads_answer_over_rpc_serve_on_a_fresh_node() {
    const METHODS: &[&str] = &[
        "status",
        "server_info",
        "metrics",
        "ledger",
        "account_tx_index_status",
        "fee",
        "external_lane_info",
        "fx_fix_list",
        "private_readiness",
        "navcoin_bridge_routes",
        "receipts",
        "blocks",
        "validators",
        "manifests",
        "batch_archive",
        "verify_blocks",
        "verify_state",
        "verify_bridge",
        "verify_mempool",
        "verify_shielded",
        "orchard_pool_report",
        "mempool_status",
        "shield_turnstile",
        "bridge_status",
    ];
    let root = unique_root();
    let data_dir = root.join("node");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-fresh-reads-rpc-test".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize RPC test node");

    let port = free_port();
    let ready = root.join("fresh-reads.ready.json");
    let mut child = spawn_rpc_serving(&data_dir, &ready, port, false, METHODS.len());
    wait_for_file(&ready);
    let mut failures = Vec::new();
    for method in METHODS {
        let response = rpc_call(port, &RpcRequest::empty(format!("fresh-{method}"), *method));
        if !response.ok {
            failures.push(format!("{method}: {:?}", response.error));
        } else if let Err(error) = response.validate_protocol() {
            failures.push(format!("{method}: {error}"));
        }
    }
    let status = child.wait().expect("wait for RPC server");
    assert!(status.success(), "RPC server failed with {status}");
    assert!(
        failures.is_empty(),
        "fresh-node reads failed: {failures:#?}"
    );

    fs::remove_dir_all(root).expect("remove RPC test directory");
}

/// Pins the worker-side shape that `rpc-serve` treats as the only recoverable
/// id mismatch: a worker that cannot read its spooled request file answers
/// under the CLI fallback id `local-1` with `ok: false`, no result, and the
/// single `("error", <code>, "rpc request failed")` event. If this shape
/// drifts, the parent's id recovery stops matching it and fails closed.
#[test]
fn rpc_worker_unreadable_request_file_answers_with_fallback_error_shape() {
    let root = unique_root();
    fs::create_dir_all(&root).expect("create RPC test directory");
    let missing_request = root.join("missing-request.json");
    let output = Command::new(node_bin())
        .args(["rpc", "--request-file"])
        .arg(&missing_request)
        .arg("--data-dir")
        .arg(root.join("node"))
        .stdin(Stdio::null())
        .output()
        .expect("run rpc worker");
    assert!(
        !output.status.success(),
        "worker must fail on a missing request file"
    );
    let response: RpcResponse =
        serde_json::from_slice(&output.stdout).expect("worker emits a JSON RPC response");
    response
        .validate_protocol()
        .expect("fallback response is protocol valid");
    assert_eq!(response.id, "local-1");
    assert!(!response.ok);
    assert!(response.result.is_none());
    let error = response.error.as_ref().expect("fallback error");
    assert_eq!(response.events.len(), 1, "{:?}", response.events);
    assert_eq!(response.events[0].event_type, "error");
    assert_eq!(response.events[0].subject, error.code);
    assert_eq!(response.events[0].message, "rpc request failed");

    fs::remove_dir_all(root).expect("remove RPC test directory");
}

/// `owned_objects` over rpc-serve carries `node_id` and `inventory_revision`
/// only when asked, and the revision only where the
/// owned-lane gate refuses legacy (non-v3) certified applies: those change
/// objects without a FastPay fence, so the revision would not move. Where
/// they are allowed (the default lane) the page says why it is absent.
#[test]
fn owned_objects_revision_only_where_the_owned_lane_refuses_legacy_applies() {
    let root = unique_root();
    let data_dir = root.join("node");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-fastpay-default-rpc-test".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize RPC test node");
    let owner = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[83; 32]);
    let request = owned_objects_request(
        "owned-objects-revision",
        OwnedObjectsParams {
            owner_public_key_hex: postfiat_crypto_provider::bytes_to_hex(&owner.public_key),
            asset: None,
            limit: None,
            cursor: None,
            omit_owner_keys: false,
            include_inventory_revision: true,
        },
    );
    let mut default_request = request.clone();
    default_request
        .params
        .as_object_mut()
        .expect("params")
        .remove("include_inventory_revision");
    for disabled in [false, true] {
        let port = free_port();
        let ready = root.join(format!("owned-objects-{disabled}.ready.json"));
        let mut child = spawn_rpc_serving(&data_dir, &ready, port, disabled, 2);
        wait_for_file(&ready);
        // Without the parameter the page has main's shape: none of the fields.
        let default = rpc_call(port, &default_request);
        assert!(default.ok, "owned_objects failed: {:?}", default.error);
        let default = default.result.expect("owned_objects result");
        for field in [
            "node_id",
            "inventory_revision",
            "inventory_revision_unavailable",
        ] {
            assert!(
                default.get(field).is_none(),
                "{field} in a default page: {default}"
            );
        }
        let response = rpc_call(port, &request);
        assert!(response.ok, "owned_objects failed: {:?}", response.error);
        validate_response_kind(&response, RpcResponseKind::OwnedObjects)
            .expect("owned_objects response validates");
        let result = response.result.expect("owned_objects result");
        assert_eq!(result["node_id"], "validator-0");
        if disabled {
            let revision = &result["inventory_revision"];
            assert_eq!(revision["finalized_height"], result["height"]);
            assert_eq!(revision["fastpay_sequence"], 0);
            assert_eq!(revision["incarnation"].as_str().map(str::len), Some(32));
            assert!(result.get("inventory_revision_unavailable").is_none());
        } else {
            assert!(result.get("inventory_revision").is_none(), "{result}");
            assert_eq!(
                result["inventory_revision_unavailable"],
                "legacy_owned_applies_enabled"
            );
        }
        let status = child.wait().expect("wait for RPC server");
        assert!(status.success(), "RPC server failed with {status}");
    }

    fs::remove_dir_all(root).expect("remove RPC test directory");
}

/// The test process plays the validator and holds the real redb writer handle.
/// The separate production rpc-serve process starts its own isolated workers.
#[cfg(target_os = "linux")]
#[test]
fn isolated_receipts_and_status_answer_during_a_validator_commit() {
    use postfiat_node::{
        apply_batch, create_transfer_batch, init_consensus_v2, validator_receipt_reader,
        ApplyBatchOptions, BatchTransferOptions, InitConsensusV2Options,
    };
    use postfiat_rpc_sdk::{receipts_request, status_request};
    let root = unique_root();
    let data_dir = root.join("node");
    let chain_id = "rpc-commit-window";
    init_consensus_v2(InitConsensusV2Options {
        data_dir: data_dir.clone(),
        chain_id: chain_id.into(),
        node_id: "validator-0".into(),
        validator_count: 1,
        activation_height: 1000,
        storage_activation_height: Some(1),
    })
    .unwrap();
    let backup =
        postfiat_rpc_sdk::wallet_backup_from_master_seed(chain_id, "ab".repeat(32), 0).unwrap();
    let key = postfiat_rpc_sdk::derive_wallet_key_pair(&backup).unwrap();
    let to = postfiat_crypto_provider::address_from_public_key(&key.public_key);
    let commit = || {
        let batch_file = root.join("transfer.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: to.clone(),
            amount: 250_000,
            batch_file: batch_file.clone(),
        })
        .unwrap();
        let rows = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].accepted);
        rows[0].clone()
    };
    commit();
    let port = free_port();
    let ready = root.join("rpc.ready.json");
    let mut rpc = spawn_rpc_serving(&data_dir, &ready, port, false, 5);
    wait_for_file(&ready);
    let _reader = validator_receipt_reader::Service::start(&data_dir).unwrap();
    let lease = postfiat_storage::NodeStore::new(&data_dir)
        .transactional_store_lease()
        .unwrap()
        .unwrap();
    let started = Instant::now();
    let latest = commit();
    // Wait beyond the full production deadline, leaving time for the typed
    // status fallback within its 30-second maximum age.
    thread::sleep(Duration::from_secs(21).saturating_sub(started.elapsed()));
    let previous = rpc_call(port, &status_request("status-during-lease"));
    assert!(previous.ok, "{:?}", previous.error);
    assert_eq!(previous.result.unwrap()["block_height"], 1);

    let lookup = |id: &str, tx: Option<&str>| {
        let reply = rpc_call(port, &receipts_request(id, tx, Some(1)));
        assert!(reply.ok, "{:?}", reply.error);
        assert!(reply.events.iter().any(|event| event
            .message
            .starts_with("receipt snapshot at height 2 block ")));
        reply.result.unwrap()
    };
    let expected = serde_json::json!([latest]);
    assert_eq!(lookup("first-ever-lookup", Some(&latest.tx_id)), expected);
    assert_eq!(lookup("latest", None), expected);
    assert_eq!(
        lookup("unknown", Some("missing-transaction")),
        serde_json::json!([])
    );
    assert!(started.elapsed() >= Duration::from_secs(20));
    drop(lease);
    let refreshed = rpc_call(port, &status_request("status-after-lease"));
    assert!(refreshed.ok, "{:?}", refreshed.error);
    assert_eq!(refreshed.result.unwrap()["block_height"], 2);
    assert!(rpc.wait().unwrap().success());
    drop(_reader);
    fs::remove_dir_all(root).unwrap();
}

/// One committed transfer in a transactional consensus-v2 node, as in the commit-window test.
#[cfg(target_os = "linux")]
fn committed_transactional_node(chain_id: &str) -> (PathBuf, PathBuf) {
    use postfiat_node::{
        apply_batch, create_transfer_batch, init_consensus_v2, ApplyBatchOptions,
        BatchTransferOptions, InitConsensusV2Options,
    };
    let root = unique_root();
    let data_dir = root.join("node");
    init_consensus_v2(InitConsensusV2Options {
        data_dir: data_dir.clone(),
        chain_id: chain_id.into(),
        node_id: "validator-0".into(),
        validator_count: 1,
        activation_height: 1000,
        storage_activation_height: Some(1),
    })
    .unwrap();
    let backup =
        postfiat_rpc_sdk::wallet_backup_from_master_seed(chain_id, "ab".repeat(32), 0).unwrap();
    let key = postfiat_rpc_sdk::derive_wallet_key_pair(&backup).unwrap();
    let batch_file = root.join("transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: postfiat_crypto_provider::address_from_public_key(&key.public_key),
        amount: 250_000,
        batch_file: batch_file.clone(),
    })
    .unwrap();
    let rows = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .unwrap();
    assert!(rows[0].accepted);
    (root, data_dir)
}

/// The receipt reader takes its snapshot from the validator's handle. If the
/// validator's operation ends during that read, the database must stay open
/// until the snapshot is dropped, or redb releases the file lock under it.
#[cfg(target_os = "linux")]
#[test]
fn held_receipt_snapshot_keeps_the_store_open_after_the_validator_releases_it() {
    use postfiat_storage::transactional::transactional_open_counts_for;
    let (root, data_dir) = committed_transactional_node("snapshot-hold");
    let store = postfiat_storage::NodeStore::new(&data_dir);
    let validator_handle = store.transactional_store_lease().unwrap().unwrap();
    let snapshot = store
        .held_transactional_read_snapshot()
        .unwrap()
        .expect("the validator holds the store");
    let before = transactional_open_counts_for(&data_dir);
    drop(validator_handle);
    assert_eq!(
        transactional_open_counts_for(&data_dir).database_closes,
        before.database_closes,
        "the database closed while a snapshot read was in progress"
    );
    let rows = snapshot
        .receipts_tail(None, 1, 1 << 20, Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert_eq!(rows.len(), 1);
    drop(snapshot);
    assert_eq!(
        transactional_open_counts_for(&data_dir).database_closes,
        before.database_closes + 1
    );
    fs::remove_dir_all(root).unwrap();
}
