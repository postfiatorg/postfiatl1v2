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

fn node_bin() -> String {
    std::env::var("POSTFIAT_SIGNER_TEST_NODE")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_postfiat-node").to_string())
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

/// The signer refuses guard state below a directory another user could
/// replace (`validator_signer_state_permissions`). Under a `TMPDIR` with such
/// an ancestor these process-level tests cannot run, so they say so and stop.
fn temp_dir_custody_safe() -> bool {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    let Ok(dir) = std::env::temp_dir().canonicalize() else {
        return false;
    };
    let safe = dir.ancestors().all(|path| {
        std::fs::metadata(path).is_ok_and(|m| {
            (m.uid() == uid || m.uid() == 0)
                && (m.mode() & 0o022 == 0 || (m.uid() == 0 && m.mode() & 0o1000 != 0))
        })
    });
    if !safe {
        eprintln!(
            "skipped: TMPDIR {} or a parent is writable by another user; the signer refuses guard state there",
            dir.display()
        );
    }
    safe
}

#[test]
fn rpc_serve_signs_a_finality_vote_through_the_signer_process() {
    if !temp_dir_custody_safe() {
        return;
    }
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
    {
        use std::os::unix::fs::PermissionsExt;
        for directory in [&root, &signer_dir] {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .expect("custody mode");
        }
    }
    let key_file = signer_dir.join(VALIDATOR_KEYS_FILE);
    fs::rename(data_dir.join(VALIDATOR_KEYS_FILE), &key_file).expect("move key");
    let local = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status");
    // A corrupt decoy key file in the data directory: rpc-serve in socket
    // mode must sign without reading it.
    let decoy = data_dir.join(VALIDATOR_KEYS_FILE);
    fs::write(&decoy, b"{ decoy, not a key file").expect("decoy");
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&decoy, fs::Permissions::from_mode(0o600)).expect("decoy mode");
    }

    // SAFETY: geteuid has no preconditions and cannot fail.
    let euid = unsafe { libc::geteuid() };
    let socket = signer_dir.join("validator-0.sock");
    children.0.push(
        Command::new(node_bin())
            .args([
                "validator-signer-serve",
                "--state-dir",
                &path(&signer_dir.join("guard")),
                "--initialize-state",
                "--chain-id",
                CHAIN_ID,
                "--genesis-hash",
                &local.genesis_hash,
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
    assert_eq!(fs::read(&decoy).expect("decoy"), b"{ decoy, not a key file");

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
    // The decoy is still a key file in rpc-serve's data directory.
    assert!(
        reasons.contains("validator_key_copy_in_data_dir"),
        "{reasons}"
    );
    assert_eq!(
        custody["data_dir_key_files"],
        serde_json::json!(["validator_keys.json"])
    );
    assert_eq!(custody["signer_double_sign_protection"], true);

    // Persist through an actual process death and restart, independently of
    // rpc-serve's own vote cache. These are disposable test-chain signatures.
    use postfiat_node::{SocketSigner, ValidatorSigner};
    let context = b"postfiat-l1-v2/block-certificate/v1";
    // The guard signs only for this chain; a far height keeps the slot clear
    // of rpc-serve's own votes.
    let message = serde_json::json!([
        CHAIN_ID,
        local.genesis_hash,
        1,
        1_000_000,
        0,
        "b".repeat(96),
        "validator-0",
        "transparent",
        "c".repeat(96),
        "d".repeat(96),
        [],
        "validator-0",
        true
    ]);
    let encoded = serde_json::to_vec(&message).expect("tuple");
    let signer = SocketSigner::connect(&socket, Duration::from_secs(5)).expect("connect signer");
    let signature = signer
        .sign("validator-0", context, &encoded)
        .expect("first signature");
    children.0[0].kill().expect("kill signer");
    children.0[0].wait().expect("wait signer");
    children.0.push(
        Command::new(node_bin())
            .args([
                "validator-signer-serve",
                "--state-dir",
                &path(&signer_dir.join("guard")),
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
            .expect("restart signer"),
    );
    wait_for("restarted signer", || {
        SocketSigner::connect(&socket, Duration::from_millis(100)).is_ok()
    });
    let signer = SocketSigner::connect(&socket, Duration::from_secs(5)).expect("reconnect signer");
    assert_eq!(
        signature,
        signer
            .sign("validator-0", context, &encoded)
            .expect("idempotent restart retry")
    );
    let mut conflict = message;
    conflict[12] = false.into();
    let error = signer
        .sign(
            "validator-0",
            context,
            &serde_json::to_vec(&conflict).unwrap(),
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("validator_signer_double_sign"),
        "{error}"
    );
    eprintln!("restart_refusal chain={CHAIN_ID} height=1000000 view=0 error={error}");
}

/// The deployment shape with two OS users: the signer runs as user A and
/// rpc-serve as user B, which reaches the `0660` socket through the shared
/// group. Run as root with `POSTFIAT_TWO_USER_SIGNER=<A uid>:<B uid>:<gid>`,
/// where B is a member of the group `gid`:
///
/// ```text
/// sudo POSTFIAT_TWO_USER_SIGNER=1001:1002:1003 cargo test --locked -p postfiat-node \
///   --test validator_signer_process -- --ignored --exact \
///   rpc_serve_under_another_user_signs_through_the_group_socket
/// ```
///
/// The test (as root) creates the chain, gives the data directory to B, the
/// key directory (`0700`) to A and the socket directory (`0750`) to A with
/// group `gid`, starts `validator-signer-serve --socket-group <gid>
/// --allow-peer-uid <B uid>` as A, then `rpc-serve --finality-signer-socket`
/// as B, and asks for a vote and the readiness read.
#[test]
#[ignore = "needs two OS users"]
fn rpc_serve_under_another_user_signs_through_the_group_socket() {
    use std::os::unix::fs::{chown, PermissionsExt};
    use std::os::unix::process::CommandExt;
    let users = std::env::var("POSTFIAT_TWO_USER_SIGNER")
        .expect("POSTFIAT_TWO_USER_SIGNER=<signer uid>:<serving uid>:<shared gid>");
    let ids: Vec<u32> = users
        .split(':')
        .map(|id| id.parse().expect("numeric id"))
        .collect();
    let [signer_uid, serving_uid, gid] = ids[..] else {
        panic!("POSTFIAT_TWO_USER_SIGNER needs three ids");
    };
    // SAFETY: geteuid has no preconditions and cannot fail.
    assert_eq!(unsafe { libc::geteuid() }, 0, "run as root");
    fn chown_tree(path: &Path, uid: u32, gid: u32) {
        chown(path, Some(uid), Some(gid)).expect("chown");
        if path.is_dir() {
            for entry in fs::read_dir(path).expect("read dir") {
                chown_tree(&entry.expect("entry").path(), uid, gid);
            }
        }
    }
    let root = PathBuf::from(format!("/tmp/pf-vsig-two-users-{}", std::process::id()));
    let mut children = Children(Vec::new(), root.clone());
    fs::create_dir_all(&root).expect("root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).expect("root mode");
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
    let key_dir = root.join("signer-keys");
    let socket_dir = root.join("signer-run");
    fs::create_dir_all(&key_dir).expect("key dir");
    fs::create_dir_all(&socket_dir).expect("socket dir");
    let key_file = key_dir.join(VALIDATOR_KEYS_FILE);
    fs::rename(data_dir.join(VALIDATOR_KEYS_FILE), &key_file).expect("move key");
    chown_tree(&key_dir, signer_uid, gid);
    fs::set_permissions(&key_dir, fs::Permissions::from_mode(0o700)).expect("key dir mode");
    chown(&socket_dir, Some(signer_uid), Some(gid)).expect("socket dir owner");
    fs::set_permissions(&socket_dir, fs::Permissions::from_mode(0o750)).expect("socket dir mode");
    let local = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status");
    let serve_dir = root.join("serve");
    fs::create_dir_all(&serve_dir).expect("serve dir");
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve port");
    let port = listener.local_addr().expect("port").port();
    drop(listener);
    let topology = serve_dir.join("topology.json");
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
    chown_tree(&data_dir, serving_uid, gid);
    chown_tree(&serve_dir, serving_uid, gid);

    let socket = socket_dir.join("validator-0.sock");
    children.0.push(
        Command::new(node_bin())
            .args([
                "validator-signer-serve",
                "--state-dir",
                &path(&key_file.parent().expect("key directory").join("guard")),
                "--initialize-state",
                "--chain-id",
                CHAIN_ID,
                "--genesis-hash",
                &local.genesis_hash,
                "--key-file",
                &path(&key_file),
                "--socket",
                &path(&socket),
                "--socket-group",
                &gid.to_string(),
                "--allow-peer-uid",
                &serving_uid.to_string(),
            ])
            .uid(signer_uid)
            .gid(gid)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn signer as user A"),
    );
    wait_for("signer socket", || socket.exists());
    let ready_file = serve_dir.join("rpc.ready.json");
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
                &path(&serve_dir.join("finality")),
            ])
            .uid(serving_uid)
            .gid(gid)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn rpc-serve as user B"),
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
    let readiness = rpc(
        port,
        serde_json::json!({
            "version": "postfiat-local-rpc-v1",
            "id": "readiness",
            "method": "private_readiness",
            "params": {}
        }),
    );
    let custody = &readiness["result"]["custody"];
    assert_eq!(custody["signer_uid"], signer_uid);
    assert_eq!(custody["serving_uid"], serving_uid);
    let reasons = readiness["result"]["reasons"].to_string();
    assert!(!reasons.contains("validator_signer_same_user"), "{reasons}");
    assert!(
        !reasons.contains("validator_signer_unreachable"),
        "{reasons}"
    );
}
