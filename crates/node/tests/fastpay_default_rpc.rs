use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_node::{init, InitOptions};
use postfiat_rpc_sdk::{
    server_info_request, validate_response_kind, verify_bridge_request, RpcRequest, RpcResponse,
    RpcResponseKind,
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
