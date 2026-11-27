//! A read replica answers in-process; a validator answers through a worker
//! process. This test runs both against one data directory and compares the
//! replies, so the new path is checked against the old one and not against
//! itself. It also checks what a replica refuses.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_node::block_vote_records::{
    write_block_vote_record, BlockVoteOutcome, BlockVoteRecord, BlockVoteRecordEntry,
    BLOCK_VOTE_RECORD_SCHEMA,
};
use postfiat_node::{faucet_key, init, InitOptions, NodeOptions};
use postfiat_rpc_sdk::{RpcRequest, RpcResponse};

fn node_bin() -> &'static str {
    env!("CARGO_BIN_EXE_postfiat-node")
}

fn unique_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "postfiat-replica-equivalence-{}-{nanos}",
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
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if path.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {}", path.display());
}

#[derive(Debug)]
struct Replica {
    child: Child,
    port: u16,
}

impl Drop for Replica {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_replica(data_dir: &Path, extra: &[&str]) -> Result<Replica, String> {
    let port = free_port();
    let ready = data_dir.join(format!("readiness/replica-{port}.json"));
    let mut command = Command::new(node_bin());
    command.args([
        "rpc-serve",
        "--unsafe-devnet-json-storage",
        "--rpc-profile",
        "public-replica",
        "--data-dir",
        data_dir.to_str().expect("data directory UTF-8"),
        "--spool-dir",
        data_dir.join("rpc-spool").to_str().expect("spool UTF-8"),
        "--ready-file",
        ready.to_str().expect("ready file UTF-8"),
        "--bind-host",
        "127.0.0.1",
        "--port",
        &port.to_string(),
        "--max-requests",
        "100000",
        "--timeout-ms",
        "10000",
        "--child-timeout-ms",
        "10000",
        "--max-replica-lag-blocks",
        "5",
    ]);
    command.args(extra);
    let mut child = command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn replica");
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if ready.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            return Ok(Replica { child, port });
        }
        if let Some(status) = child.try_wait().expect("poll replica") {
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                let _ = std::io::Read::read_to_string(&mut pipe, &mut stderr);
            }
            return Err(format!("replica exited with {status}: {stderr}"));
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err("replica did not become ready".to_string())
}

fn replica_call(port: u16, request: &RpcRequest) -> RpcResponse {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect replica");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("set timeout");
    serde_json::to_writer(&mut stream, request).expect("write request");
    stream.write_all(b"\n").expect("terminate request");
    stream.flush().expect("flush request");
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .expect("read response");
    serde_json::from_str(&response).unwrap_or_else(|error| panic!("decode `{response}`: {error}"))
}

/// The worker path, exactly as `rpc-serve` runs it for a validator.
fn worker_call(data_dir: &Path, request: &RpcRequest) -> Result<RpcResponse, String> {
    let request_file = data_dir.join(format!("worker-request-{}.json", request.id));
    fs::write(
        &request_file,
        serde_json::to_vec(request).expect("request json"),
    )
    .expect("spool");
    let output = Command::new(node_bin())
        .env_clear()
        .current_dir(data_dir)
        .arg("rpc")
        .arg("--request-file")
        .arg(&request_file)
        .arg("--data-dir")
        .arg(data_dir)
        .output()
        .expect("run worker");
    let _ = fs::remove_file(&request_file);
    if output.stdout.is_empty() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("worker reply: {error}"))
}

fn write_source_tip(data_dir: &Path, source_height: u64, local_height: u64) {
    let path = data_dir.join("readiness/replica-source-tip.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("readiness dir");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    fs::write(
        path,
        serde_json::json!({
            "schema": "postfiat-rpc-replica-source-tip-v1",
            "source_height": source_height,
            "local_height": local_height,
            "checked_at_unix": now,
        })
        .to_string(),
    )
    .expect("write source tip");
}

#[test]
fn replica_reads_equal_the_worker_path_and_a_replica_refuses_what_it_must() {
    let root = unique_root();
    let data_dir = root.join("replica");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-replica-equivalence-test".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init data directory");
    let address = faucet_key(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("faucet key")
    .address;

    // A data directory that holds a validator key is not a replica's.
    let key_file = data_dir.join("validator_keys.json");
    if key_file.exists() {
        let refusal = spawn_replica(&data_dir, &[]).expect_err("replica must refuse a key file");
        assert!(
            refusal.contains("rpc_replica_key_material_present"),
            "{refusal}"
        );
        fs::remove_file(&key_file).expect("remove validator key");
    }
    // Transaction submission cannot be switched on for a replica.
    let refusal = spawn_replica(&data_dir, &["--allow-mempool-submit"])
        .expect_err("replica must refuse --allow-mempool-submit");
    assert!(refusal.contains("rpc_replica_write_forbidden"), "{refusal}");

    // One vote record, as a proposer writes it: validator-2 answered after the
    // quorum had closed, validator-3 did not answer.
    let entry = |validator: &str, outcome: BlockVoteOutcome, after_quorum: bool| BlockVoteRecordEntry {
        validator: validator.to_string(),
        vote_id: (outcome != BlockVoteOutcome::NoAnswer).then(|| "ab".repeat(48)),
        outcome,
        refusal_class: None,
        answered_after_quorum: after_quorum,
    };
    write_block_vote_record(
        &data_dir,
        &BlockVoteRecord {
            schema: BLOCK_VOTE_RECORD_SCHEMA.to_string(),
            chain_id: "postfiat-replica-equivalence-test".to_string(),
            block_height: 7,
            view: 0,
            block_hash: "cd".repeat(48),
            certificate_id: "ef".repeat(48),
            proposer: "validator-0".to_string(),
            quorum: 3,
            round_deadline_ms: 30_000,
            entries: vec![
                entry("validator-0", BlockVoteOutcome::Accepted, false),
                entry("validator-1", BlockVoteOutcome::Accepted, false),
                entry("validator-2", BlockVoteOutcome::Accepted, true),
                entry("validator-3", BlockVoteOutcome::NoAnswer, false),
            ],
        },
    )
    .expect("write vote record");

    write_source_tip(&data_dir, 0, 0);
    let replica = spawn_replica(&data_dir, &[]).expect("replica starts");
    wait_for_file(&data_dir.join("readiness/replica-source-tip.json"));

    let status = replica_call(replica.port, &RpcRequest::empty("status", "status"));
    let status = status.result.expect("status result");
    assert_eq!(status["node_role"], "rpc_follower");
    assert_eq!(status["voting_enabled"], false);
    assert_eq!(status["rpc_profile"], "public-replica");
    assert_eq!(status["replica_ready"], true);
    assert_eq!(status["replica_lag_blocks"], 0);

    let j = |value: serde_json::Value| value;
    // Valid, base, out-of-range and malformed inputs for every in-process method.
    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("account", j(serde_json::json!({ "address": address }))),
        (
            "account",
            j(serde_json::json!({ "address": "pf-not-an-address" })),
        ),
        ("account", j(serde_json::json!({}))),
        (
            "account_lines",
            j(serde_json::json!({ "account": address })),
        ),
        (
            "account_lines",
            j(serde_json::json!({ "account": address, "limit": 1 })),
        ),
        ("account_lines", j(serde_json::json!({}))),
        (
            "account_assets",
            j(serde_json::json!({ "account": address })),
        ),
        (
            "account_assets",
            j(serde_json::json!({ "account": address, "asset_id": "none" })),
        ),
        ("account_tx", j(serde_json::json!({ "address": address }))),
        (
            "account_tx",
            j(serde_json::json!({ "address": address, "from_height": 999_999, "limit": 2 })),
        ),
        ("account_tx_index_status", j(serde_json::json!({}))),
        ("validators", j(serde_json::json!({}))),
        ("manifests", j(serde_json::json!({}))),
        ("fee", j(serde_json::json!({}))),
        ("ledger", j(serde_json::json!({}))),
        ("ledger", j(serde_json::json!({ "limit": 1 }))),
        ("blocks", j(serde_json::json!({}))),
        (
            "blocks",
            j(serde_json::json!({ "from_height": 0, "limit": 4 })),
        ),
        (
            "blocks",
            j(serde_json::json!({ "from_height": 999_999, "limit": 4 })),
        ),
        ("receipts", j(serde_json::json!({}))),
        (
            "receipts",
            j(serde_json::json!({ "tx_id": "00", "limit": 1 })),
        ),
        ("tx", j(serde_json::json!({ "tx_id": "00" }))),
        ("tx", j(serde_json::json!({}))),
        (
            "vault_bridge_route",
            j(serde_json::json!({ "asset_id": "none" })),
        ),
        (
            "vault_bridge_status",
            j(serde_json::json!({ "asset_id": "none" })),
        ),
        ("bridge_status", j(serde_json::json!({}))),
        ("block_vote_records", j(serde_json::json!({ "from_height": 1, "limit": 4 }))),
        ("block_vote_records", j(serde_json::json!({ "from_height": 7 }))),
        ("block_vote_records", j(serde_json::json!({ "from_height": 999_999, "limit": 64 }))),
        ("block_vote_records", j(serde_json::json!({}))),
        ("block_vote_records", j(serde_json::json!({ "from_height": 0 }))),
        ("block_vote_records", j(serde_json::json!({ "from_height": 1, "limit": 65 }))),
    ];
    let mut compared = 0;
    for (index, (method, params)) in cases.iter().enumerate() {
        let request = RpcRequest::new(format!("case-{index}"), *method, params.clone());
        let from_replica = replica_call(replica.port, &request);
        let from_worker = worker_call(&data_dir, &request);
        let label = format!("{method} {params}");
        match from_worker {
            Ok(worker) => {
                assert_eq!(
                    from_replica.ok, worker.ok,
                    "{label}: {:?} vs {:?}",
                    from_replica.error, worker.error
                );
                assert_eq!(
                    serde_json::to_string(&from_replica.result).expect("replica result"),
                    serde_json::to_string(&worker.result).expect("worker result"),
                    "{label}: the replica's result differs from the worker path's"
                );
            }
            // The worker prints nothing and exits non-zero on a failed read;
            // `rpc-serve` turns that into an error reply. The replica must fail too.
            Err(worker_error) => assert!(
                !from_replica.ok,
                "{label}: the worker path failed ({worker_error}) but the replica answered"
            ),
        }
        compared += 1;
    }
    assert_eq!(compared, cases.len());

    // `metrics` carries the clock and free disk, so only its shape is compared.
    // `rpc-serve` adds its own `rpc` section on top of the worker's report.
    let request = RpcRequest::empty("metrics", "metrics");
    let from_replica = replica_call(replica.port, &request)
        .result
        .expect("replica metrics");
    let from_worker = worker_call(&data_dir, &request)
        .expect("worker metrics")
        .result
        .expect("worker metrics result");
    let keys = |value: &serde_json::Value| -> Vec<String> {
        value
            .as_object()
            .expect("metrics object")
            .keys()
            .cloned()
            .collect()
    };
    let replica_keys: Vec<String> = keys(&from_replica)
        .into_iter()
        .filter(|key| key != "rpc")
        .collect();
    assert_eq!(replica_keys, keys(&from_worker));

    // A height-pure read is cached, names its height, and still equals the worker path.
    let request = RpcRequest::new(
        "cached",
        "blocks",
        serde_json::json!({ "from_height": 0, "limit": 1 }),
    );
    let first = replica_call(replica.port, &request);
    let second = replica_call(replica.port, &request);
    assert_eq!(first.result, second.result);
    let cache_event = second
        .events
        .iter()
        .find(|event| event.event_type == "read_cache")
        .expect("the second answer comes from the cache and says so");
    assert_eq!(cache_event.subject, status["block_height"].to_string());
    assert!(first
        .events
        .iter()
        .all(|event| event.event_type != "read_cache"));
    // A balance read is never cached.
    let request = RpcRequest::new(
        "uncached",
        "account",
        serde_json::json!({ "address": address }),
    );
    replica_call(replica.port, &request);
    let again = replica_call(replica.port, &request);
    assert!(again
        .events
        .iter()
        .all(|event| event.event_type != "read_cache"));

    // The vote record comes back as written; a height without one says so.
    let reply = replica_call(
        replica.port,
        &RpcRequest::new("records", "block_vote_records", serde_json::json!({ "from_height": 6, "limit": 2 })),
    )
    .result
    .expect("vote records");
    assert_eq!(reply["schema"], "postfiat-block-vote-records-v1");
    assert_eq!(reply["records"][0], serde_json::json!({ "height": 6, "available": false, "record": null }));
    assert_eq!(reply["records"][1]["available"], true);
    let entries = &reply["records"][1]["record"]["entries"];
    assert_eq!(entries[2]["validator"], "validator-2");
    assert_eq!((&entries[2]["outcome"], &entries[2]["answered_after_quorum"]), (&serde_json::json!("accepted"), &serde_json::json!(true)));
    assert_eq!(entries[3]["outcome"], "no_answer");

    // What a replica never serves.
    for method in [
        "mempool_submit_signed_payment_v2",
        "mempool_submit_signed_transfer_finality",
        "private_action_submit",
        "a_method_nobody_listed",
    ] {
        let reply = replica_call(replica.port, &RpcRequest::empty("refused", method));
        assert!(!reply.ok, "{method} must be refused");
        let error = reply.error.expect("refusal");
        assert_eq!(
            error.code, "method_not_served_by_profile",
            "{method}: {}",
            error.message
        );
    }

    // The source moved ahead by more than the limit: reads stop, health still answers.
    write_source_tip(&data_dir, 6, 0);
    let reply = replica_call(replica.port, &RpcRequest::empty("lagging", "validators"));
    assert_eq!(reply.error.expect("lag refusal").code, "replica_lagging");
    let status = replica_call(replica.port, &RpcRequest::empty("status", "status"))
        .result
        .expect("status while lagging");
    assert_eq!(status["replica_ready"], false);
    assert_eq!(status["replica_lag_blocks"], 6);
    // Caught up again.
    write_source_tip(&data_dir, 5, 0);
    assert!(replica_call(replica.port, &RpcRequest::empty("ok", "validators")).ok);

    drop(replica);
    let _ = fs::remove_dir_all(root);
}
