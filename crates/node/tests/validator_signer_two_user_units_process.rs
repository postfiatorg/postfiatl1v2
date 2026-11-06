//! The two-user units on a local chain: `deployment-validator-units-stage
//! --signer-shape two-user` writes the signer, rpc and transport units,
//! `systemd-analyze verify` checks them when it exists, and the three
//! processes start from the units' own command lines under one OS user. The
//! key lives only in the signer's directory, so readiness reports the shared
//! user and no key copy in rpc-serve's data directory.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_node::{
    init_consensus_v2, stage_deployment_validator_units, status, write_consensus_v2_topology,
    DeploymentSignerShape, DeploymentValidatorUnitsStageOptions, InitConsensusV2Options,
    NodeOptions, TopologyConsensusV2Options, VALIDATOR_KEYS_FILE,
};

const CHAIN_ID: &str = "validator-signer-two-user-units-test";
const RELEASE: &str = "release-two-user-1";

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

fn wait_for(what: &str, children: &mut Children, ready: impl Fn() -> bool) {
    let started = Instant::now();
    while !ready() {
        for index in 0..children.0.len() {
            let child = &mut children.0[index];
            if let Some(exit) = child.try_wait().expect("child status") {
                let logs = ["transport.stderr", "rpc.stderr"].map(|name| {
                    let log = fs::read_to_string(children.1.join(name)).unwrap_or_default();
                    log.lines()
                        .find(|line| line.starts_with("error"))
                        .unwrap_or_default()
                        .to_string()
                });
                panic!("a process exited before {what} was ready: {exit}: {logs:?}");
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
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
/// replace, so the test that starts it is ignored by default and needs a
/// `TMPDIR` no other user can write through: run it with `-- --ignored` (CI
/// does); under an unsafe `TMPDIR` it fails saying so rather than pass
/// without running.
fn require_temp_dir_custody_safe() {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    let dir = std::env::temp_dir()
        .canonicalize()
        .expect("canonical TMPDIR");
    let safe = dir.ancestors().all(|path| {
        fs::metadata(path).is_ok_and(|m| {
            (m.uid() == uid || m.uid() == 0)
                && (m.mode() & 0o022 == 0 || (m.uid() == 0 && m.mode() & 0o1000 != 0))
        })
    });
    assert!(
        safe,
        "TMPDIR {} or a parent is writable by another user; the signer refuses guard state there",
        dir.display()
    );
}

/// A port `p` with `p` (validator-0's RPC) and `p + 20` (its transport port)
/// both free when probed, at most five tries. Another process can still take
/// one before the child binds; the child then exits and `wait_for` fails with
/// its log line.
fn free_port_pair() -> u16 {
    for _ in 0..5 {
        let first = TcpListener::bind(("127.0.0.1", 0)).expect("probe port");
        let port = first.local_addr().expect("port").port();
        if port < u16::MAX - 20 && TcpListener::bind(("127.0.0.1", port + 20)).is_ok() {
            return port;
        }
    }
    panic!("no free port pair on loopback after five tries");
}

/// The unit's `ExecStart=` arguments with the packet's paths mapped onto the
/// test root, and its `EnvironmentFile=` read and mapped the same way.
fn exec_start(unit: &str, map: &[(String, String)]) -> Vec<String> {
    let line = unit
        .lines()
        .find_map(|line| line.strip_prefix("ExecStart="))
        .expect("ExecStart");
    let mut line = line.to_string();
    for (from, to) in map {
        line = line.replace(from, to);
    }
    line.split_whitespace().map(str::to_string).collect()
}

fn environment(file: &Path, map: &[(String, String)]) -> BTreeMap<String, String> {
    let mut text = fs::read_to_string(file).expect("environment file");
    for (from, to) in map {
        text = text.replace(from, to);
    }
    text.lines()
        .filter_map(|line| line.split_once('='))
        // The signed deployment manifest and its record belong to the host's
        // release check (`ExecStartPre`), not to the signing path.
        .filter(|(key, _)| !key.contains("DEPLOYMENT"))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
#[ignore = "starts the signer, which needs a TMPDIR no other user can write; run with --ignored"]
fn two_user_units_start_the_signer_rpc_and_transport_without_a_data_dir_key() {
    require_temp_dir_custody_safe();
    let root = std::env::temp_dir().join(format!(
        "pf-vsig-units-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let mut children = Children(Vec::new(), root.clone());
    // The packet stage takes the controlled testnet's six validators.
    let data_dir = root.join("node");
    init_consensus_v2(InitConsensusV2Options {
        data_dir: data_dir.clone(),
        chain_id: CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 6,
        activation_height: 1,
        storage_activation_height: None,
    })
    .expect("init chain");
    let signer_dir = root.join("signer");
    let log_dir = root.join("log");
    for directory in [&signer_dir, &log_dir, &root.join("inputs")] {
        fs::create_dir_all(directory).expect("dir");
    }
    {
        use std::os::unix::fs::PermissionsExt;
        for directory in [&root, &signer_dir] {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .expect("custody mode");
        }
    }
    // The key moves: only the signer's directory holds it.
    fs::rename(
        data_dir.join(VALIDATOR_KEYS_FILE),
        signer_dir.join(VALIDATOR_KEYS_FILE),
    )
    .expect("move key");
    let local = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status");

    let port = free_port_pair();
    let topology = root.join("inputs/topology.json");
    write_consensus_v2_topology(TopologyConsensusV2Options {
        chain_id: CHAIN_ID.to_string(),
        validators: 6,
        base_port: port.saturating_add(20),
        rpc_base_port: Some(port),
        hosts: Some(vec!["127.0.0.1".to_string(); 6]),
        output_file: topology.clone(),
        activation_height: 1,
        storage_activation_height: None,
    })
    .expect("topology");
    let inputs = root.join("inputs");
    fs::copy(node_bin(), inputs.join("postfiat-node")).expect("stage binary input");
    for name in ["swap.metadata.json", "private-egress.metadata.json"] {
        fs::write(inputs.join(name), b"{}").expect("metadata input");
    }
    let staged = root.join("packet");
    let report = stage_deployment_validator_units(DeploymentValidatorUnitsStageOptions {
        release_id: RELEASE.to_string(),
        topology_file: topology.clone(),
        binary_file: inputs.join("postfiat-node"),
        swap_circuit_metadata_file: inputs.join("swap.metadata.json"),
        private_egress_circuit_metadata_file: inputs.join("private-egress.metadata.json"),
        output_dir: staged.clone(),
        signer_shape: DeploymentSignerShape::TwoUser,
    })
    .expect("stage two-user units");
    let row = &report.validators[0];
    let signer_file = row.signer_unit_file.clone().expect("signer unit");
    let units = [&signer_file, &row.rpc_unit_file, &row.transport_unit_file];
    let text = |file: &Path| fs::read_to_string(file).expect("unit");
    let (signer_unit, rpc_unit, transport_unit) = (
        text(&signer_file),
        text(&row.rpc_unit_file),
        text(&row.transport_unit_file),
    );
    for unit in [&rpc_unit, &transport_unit] {
        assert!(unit.contains("SupplementaryGroups=postfiat-signer-clients"));
        assert!(unit.contains("Requires=postfiat-validator-0-signer.service"));
        assert!(!unit.contains("validator_keys.json"), "{unit}");
    }

    // The packet's paths, mapped onto this root; one OS user, so the socket
    // group is this user's group and the allowed peer uid is this uid.
    // SAFETY: geteuid and getegid have no preconditions and cannot fail.
    let (euid, egid) = unsafe { (libc::geteuid(), libc::getegid()) };
    let release_dir = format!("/etc/postfiat/releases/{RELEASE}");
    let map = vec![
        (
            format!("/opt/postfiat/releases/{RELEASE}/postfiat-node"),
            node_bin(),
        ),
        (format!("{release_dir}/topology.json"), path(&topology)),
        (
            "/var/lib/postfiat-signer/validator-0".to_string(),
            path(&signer_dir),
        ),
        (
            "/run/postfiat-signer/validator-0.sock".to_string(),
            path(&signer_dir.join("validator-0.sock")),
        ),
        ("/var/lib/postfiat/validator-0".to_string(), path(&data_dir)),
        ("/var/log/postfiat/validator-0".to_string(), path(&log_dir)),
        (
            "--socket-group postfiat-signer-clients".to_string(),
            format!("--socket-group {egid}"),
        ),
        (
            "${POSTFIAT_SIGNER_ALLOW_PEER_UID}".to_string(),
            euid.to_string(),
        ),
    ];
    // `systemd-analyze verify`, when it exists, on the units as written with
    // only the packet's paths mapped onto this root, so the binary and the
    // directories they name exist on this host.
    match Command::new("systemd-analyze").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let unit_dir = root.join("units");
            fs::create_dir_all(&unit_dir).expect("unit dir");
            let mapped = units
                .iter()
                .map(|unit| {
                    let mut text = fs::read_to_string(unit).expect("unit");
                    for (from, to) in &map {
                        text = text.replace(from, to);
                    }
                    let file = unit_dir.join(unit.file_name().expect("unit name"));
                    fs::write(&file, text).expect("mapped unit");
                    path(&file)
                })
                .collect::<Vec<_>>();
            let verify = Command::new("systemd-analyze")
                // Only the three units count: errors in the host's own units
                // that they reference (network-online.target) do not.
                .args(["verify", "--man=no", "--recursive-errors=no"])
                .args(&mapped)
                .output()
                .expect("systemd-analyze verify");
            let stderr = String::from_utf8_lossy(&verify.stderr);
            eprintln!("systemd-analyze verify: {} {stderr}", verify.status);
            assert!(verify.status.success(), "{stderr}");
        }
        _ => eprintln!("skipped systemd-analyze verify: the binary is not installed"),
    }
    let socket = signer_dir.join("validator-0.sock");
    let mut signer_args = exec_start(&signer_unit, &map);
    assert_eq!(signer_args[1], "validator-signer-serve");
    // The operator initializes the guard once; the unit never does.
    signer_args.push("--initialize-state".to_string());
    assert!(signer_args.contains(&local.genesis_hash), "{signer_args:?}");
    children.0.push(
        Command::new(&signer_args[0])
            .args(&signer_args[1..])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn signer"),
    );
    wait_for("signer socket", &mut children, || socket.exists());

    let transport_args = exec_start(&transport_unit, &map);
    assert!(transport_args.contains(&"--finality-signer-socket".to_string()));
    let transport_env = environment(&row.transport_environment_file, &map);
    let transport_ready = PathBuf::from(&transport_env["POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE"]);
    fs::create_dir_all(transport_ready.parent().expect("ready dir")).expect("ready dir");
    children.0.push(
        Command::new(&transport_args[0])
            .args(&transport_args[1..])
            .envs(&transport_env)
            .stdout(Stdio::null())
            .stderr(fs::File::create(root.join("transport.stderr")).expect("stderr"))
            .spawn()
            .expect("spawn transport-validator-serve"),
    );
    // Its ready file follows the signer check, the verifier prewarm and bind.
    wait_for("transport-validator-serve", &mut children, || {
        transport_ready.is_file()
    });

    let rpc_args = exec_start(&rpc_unit, &map);
    assert!(rpc_args.contains(&"--finality-signer-socket".to_string()));
    fs::create_dir_all(data_dir.join("finality-artifacts")).expect("artifact dir");
    let rpc_ready = data_dir.join("readiness/rpc.ready.json");
    children.0.push(
        Command::new(&rpc_args[0])
            .args(&rpc_args[1..])
            .envs(environment(&row.rpc_environment_file, &map))
            .stdout(Stdio::null())
            .stderr(fs::File::create(root.join("rpc.stderr")).expect("stderr"))
            .spawn()
            .expect("spawn rpc-serve"),
    );
    wait_for("rpc-serve", &mut children, || rpc_ready.is_file());

    let rpc_port = rpc_args
        .iter()
        .position(|arg| arg == "--port")
        .map(|index| rpc_args[index + 1].parse::<u16>().expect("port"))
        .expect("--port");
    let readiness = rpc(
        rpc_port,
        serde_json::json!({
            "version": "postfiat-local-rpc-v1",
            "id": "readiness",
            "method": "private_readiness",
            "params": {}
        }),
    );
    assert_eq!(readiness["ok"], true, "{readiness}");
    let custody = &readiness["result"]["custody"];
    assert_eq!(custody["validator_signer"], "socket", "{readiness}");
    assert_eq!(custody["signer_uid"], euid);
    let reasons = readiness["result"]["reasons"].to_string();
    assert!(reasons.contains("validator_signer_same_user"), "{reasons}");
    assert!(
        !reasons.contains("validator_key_copy_in_data_dir"),
        "{reasons}"
    );
    assert!(!data_dir.join(VALIDATOR_KEYS_FILE).exists());
    assert_eq!(custody["data_dir_key_files"], serde_json::json!([]));
    eprintln!("readiness reasons={reasons}");
}

/// `--finality-signer-socket` replaces the key file: both together are
/// refused before anything else is read.
#[test]
fn transport_validator_serve_refuses_a_key_file_beside_the_signer_socket() {
    for extra in [
        &["--key-file", "/nonexistent/validator_keys.json"][..],
        &["--unsafe-devnet-file-signer"][..],
    ] {
        let output = Command::new(node_bin())
            .args([
                "transport-validator-serve",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                "/nonexistent/node",
                "--topology",
                "/nonexistent/topology.json",
                "--vote-dir",
                "/nonexistent/votes",
                "--finality-signer-socket",
                "/nonexistent/validator-0.sock",
            ])
            .args(extra)
            .output()
            .expect("run transport-validator-serve");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("validator_signer_flags_conflict"),
            "{extra:?}: {stderr}"
        );
    }
}

/// Registering the signer reads the node id and writes nothing: with no
/// signer answering, the service refuses and the node state is untouched.
#[test]
fn transport_validator_serve_writes_nothing_before_the_signer_answers() {
    let root = std::env::temp_dir().join(format!(
        "pf-vsig-units-state-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
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
    let state_file = data_dir.join("node_state.json");
    let before = fs::read(&state_file).expect("node state");
    let output = Command::new(node_bin())
        .args([
            "transport-validator-serve",
            "--unsafe-devnet-json-storage",
            "--data-dir",
            &path(&data_dir),
            "--topology",
            &path(&root.join("topology.json")),
            "--vote-dir",
            &path(&root.join("votes")),
            "--finality-signer-socket",
            &path(&root.join("absent.sock")),
        ])
        .output()
        .expect("run transport-validator-serve");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("validator_signer_unreachable"), "{stderr}");
    assert_eq!(
        fs::read(&state_file).expect("node state"),
        before,
        "node state written"
    );
    let _ = fs::remove_dir_all(root);
}
