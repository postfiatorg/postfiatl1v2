use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_node::{init, InitOptions};
use postfiat_rpc_sdk::{server_info_request, RpcRequest, RpcResponse};

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
    let mut command = Command::new(node_bin());
    let spool_dir = data_dir.join("rpc-spool");
    let port = port.to_string();
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
        "1",
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
