//! The validator signer as its own process: `postfiat-node
//! validator-signer-serve` holds the key and `postfiat-node rpc-serve
//! --finality-signer-socket` signs a finality vote through it on a local
//! chain whose data directory holds no key file.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_node::{
    init_consensus_v2, status, write_consensus_v2_topology, InitConsensusV2Options, NodeOptions,
    TopologyConsensusV2Options, VALIDATOR_KEYS_FILE,
};

const CHAIN_ID: &str = "validator-signer-process-test";

struct Children(Vec<Child>, PathBuf);

impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.1);
    }
}

fn node_bin() -> &'static str {
    env!("CARGO_BIN_EXE_postfiat-node")
}

fn path(p: &Path) -> String {
    p.to_str().expect("UTF-8 path").to_string()
}

fn wait_for(what: &str, ready: impl Fn() -> bool) {
    let started = Instant::now();
    while !ready() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{what} not ready"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn rpc(port: u16, request: serde_json::Value) -> serde_json::Value {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect RPC");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .expect("read timeout");
    let mut line = serde_json::to_vec(&request).expect("request");
    line.push(b'\n');
    stream.write_all(&line).expect("write");
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .expect("read");
    serde_json::from_str(&response).expect("response json")
}

#[test]
fn rpc_serve_signs_a_finality_vote_through_the_signer_process() {
    let root = std::env::temp_dir().join(format!(
        "pf-vsig-process-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let mut children = Children(Vec::new(), root.clone());
    let data_dir = root.join("node");
    init_consensus_v2(InitConsensusV2Options {
        data_dir: data_dir.clone(),
        chain_id: CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
        activation_height: 1,
        storage_activation_height: None,
    })
    .expect("init chain");
    // The key leaves the data directory: only the signer process reads it.
    let signer_dir = root.join("signer");
    fs::create_dir_all(&signer_dir).expect("signer dir");
    let key_file = signer_dir.join(VALIDATOR_KEYS_FILE);
    fs::rename(data_dir.join(VALIDATOR_KEYS_FILE), &key_file).expect("move key");
    let local = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status");

    // SAFETY: geteuid has no preconditions and cannot fail.
    let euid = unsafe { libc::geteuid() };
    let socket = signer_dir.join("validator-0.sock");
    children.0.push(
        Command::new(node_bin())
            .args([
                "validator-signer-serve",
                "--key-file",
                &path(&key_file),
                "--socket",
                &path(&socket),
                "--allow-peer-uid",
                &euid.to_string(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn signer"),
    );
    wait_for("signer socket", || socket.exists());

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve port");
    let port = listener.local_addr().expect("port").port();
    drop(listener);
    let topology = root.join("topology.json");
    write_consensus_v2_topology(TopologyConsensusV2Options {
        chain_id: CHAIN_ID.to_string(),
        validators: 4,
        base_port: port.saturating_add(20),
        rpc_base_port: Some(port),
        hosts: Some(vec!["127.0.0.1".to_string(); 4]),
        output_file: topology.clone(),
        activation_height: 1,
        storage_activation_height: None,
    })
    .expect("topology");
    let ready_file = root.join("rpc.ready.json");
    children.0.push(
        Command::new(node_bin())
            .args([
                "rpc-serve",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                &path(&data_dir),
                "--ready-file",
                &path(&ready_file),
                "--port",
                &port.to_string(),
                "--max-requests",
                "100",
                "--allow-mempool-submit-finality",
                "--finality-topology",
                &path(&topology),
                "--finality-signer-socket",
                &path(&socket),
                "--finality-artifact-root",
                &path(&root.join("finality")),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn rpc-serve"),
    );
    wait_for("rpc-serve", || ready_file.is_file());

    let vote = rpc(
        port,
        serde_json::json!({
            "version": "postfiat-local-rpc-v1",
            "id": "timeout-vote",
            "method": "consensus_v2_timeout_vote",
            "params": {
                "block_height": 1,
                "view": 0,
                "proxy_required_current_height": 0,
                "proxy_required_parent_hash": local.block_tip_hash,
                "proxy_required_state_root": local.state_root,
            }
        }),
    );
    assert_eq!(vote["ok"], true, "{vote}");
    assert_eq!(vote["result"]["vote"]["validator"], "validator-0");
    assert_eq!(
        vote["result"]["consensus_v2_vote"]["validator"],
        "validator-0"
    );
    assert!(!data_dir.join(VALIDATOR_KEYS_FILE).exists());

    let readiness = rpc(
        port,
        serde_json::json!({
            "version": "postfiat-local-rpc-v1",
            "id": "readiness",
            "method": "private_readiness",
            "params": {}
        }),
    );
    assert_eq!(readiness["ok"], true, "{readiness}");
    let custody = &readiness["result"]["custody"];
    assert_eq!(custody["validator_signer"], "socket");
    assert_eq!(custody["signer_uid"], euid);
    let reasons = readiness["result"]["reasons"].to_string();
    assert!(reasons.contains("validator_signer_same_user"), "{reasons}");
    assert!(
        !reasons.contains("validator_signer_unsafe_file"),
        "{reasons}"
    );
    assert!(!reasons.contains("validator_key_file_"), "{reasons}");
}
