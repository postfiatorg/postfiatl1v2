#[cfg(test)]
mod validator_signer_socket_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn signer_root(label: &str) -> Option<PathBuf> {
        temp_dir_custody_safe().then(|| std::env::temp_dir().join(format!(
            "pf-vsock-{label}-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        )))
    }

    fn euid() -> u32 {
        // SAFETY: geteuid has no preconditions and cannot fail.
        unsafe { libc::geteuid() }
    }

    /// A one-validator consensus-v2 chain whose validator key lives only in
    /// the signer's directory: rpc-serve's data directory holds no key file.
    fn chain_with_key_moved_out(root: &Path) -> (PathBuf, StatusReport) {
        init_consensus_v2(InitConsensusV2Options {
            data_dir: root.join("node"),
            chain_id: "validator-signer-socket-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
            activation_height: 1,
            storage_activation_height: None,
        })
        .expect("init chain");
        let signer_dir = root.join("signer");
        fs::create_dir_all(&signer_dir).expect("signer dir");
        {
            use std::os::unix::fs::PermissionsExt;
            for directory in [root, signer_dir.as_path()] {
                fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).expect("custody mode");
            }
        }
        let key_file = signer_dir.join(VALIDATOR_KEYS_FILE);
        fs::rename(root.join("node").join(VALIDATOR_KEYS_FILE), &key_file).expect("move key");
        let local = status(NodeOptions {
            data_dir: root.join("node"),
        })
        .expect("status");
        (key_file, local)
    }

    fn start_signer(key_file: &Path, socket: &Path, max_connections: Option<usize>) {
        // The guard signs for the test chain beside the signer directory.
        let chain = status(NodeOptions {
            data_dir: key_file.parent().and_then(Path::parent).expect("root").join("node"),
        })
        .expect("status");
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let options = postfiat_node::ValidatorSignerServeOptions {
            state_dir: socket.with_extension(format!("{}.guard", key_file.file_name().unwrap().to_string_lossy())),
            initialize_state: true,
            chain_id: Some(chain.chain_id.clone()),
            genesis_hash: Some(chain.genesis_hash.clone()),
            min_height: None,
            key_file: key_file.to_path_buf(),
            proposal_key_file: None,
            socket: socket.to_path_buf(),
            allowed_peer_uids: vec![euid()],
            socket_group: None,
            deadline_ms: 5_000,
            max_connections,
        };
        thread::spawn(move || {
            postfiat_node::validator_signer_serve(options, |_| ready_tx.send(()).expect("ready"))
        });
        ready_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("signer ready");
    }

    fn socket_serve_options(root: &Path, socket: &Path, port: u16, requests: usize) -> RpcServeOptions {
        let data_dir = root.join("node");
        RpcServeOptions {
            data_dir: data_dir.clone(),
            spool_dir: data_dir.join("runtime/rpc-spool"),
            ready_file: data_dir.join(format!("readiness/rpc-{port}.json")),
            bind_host: "127.0.0.1".to_string(),
            port,
            max_requests: requests,
            timeout_ms: 10_000,
            child_timeout_ms: 10_000,
            event_log: None,
            allow_mempool_submit: false,
            allow_mempool_submit_finality: true,
            allow_orchard_batch_create: false,
            allow_private_action_admission: false,
            owned_lane_enabled: true,
            finality_topology_file: root.join("topology.json"),
            finality_key_file: socket.to_path_buf(),
            finality_proposal_key_file: Some(socket.to_path_buf()),
            finality_signer_socket: Some(socket.to_path_buf()),
            finality_artifact_root: data_dir.join("finality-artifacts"),
            finality_timeout_ms: 10_000,
            finality_send_retries: 0,
            finality_retry_backoff_ms: 0,
            finality_quorum_early_full_propagation: false,
            max_mempool_submit_per_peer: 8,
            max_mempool_submit_total: 32,
            max_orchard_batch_create_per_peer: 2,
            max_orchard_batch_create_total: 8,
            max_orchard_batch_create_concurrent: 1,
            max_child_dispatch_concurrent: 8,
            max_child_dispatch_per_peer: 4,
            keep_alive: false,
        }
    }

    fn write_topology(root: &Path, rpc_port: u16) {
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: "validator-signer-socket-test".to_string(),
            validators: 4,
            base_port: rpc_port.saturating_add(20),
            rpc_base_port: Some(rpc_port),
            hosts: Some(vec!["127.0.0.1".to_string(); 4]),
            output_file: root.join("topology.json"),
            activation_height: 1,
            storage_activation_height: None,
        })
        .expect("topology");
    }

    fn send(port: u16, request: &RpcRequest) -> RpcResponse {
        let mut stream = (0..200)
            .find_map(|_| match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => Some(stream),
                Err(_) => {
                    thread::sleep(Duration::from_millis(10));
                    None
                }
            })
            .expect("connect RPC");
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .expect("read timeout");
        let mut line = serde_json::to_vec(request).expect("request json");
        line.push(b'\n');
        stream.write_all(&line).expect("write");
        let mut response = String::new();
        BufReader::new(stream).read_line(&mut response).expect("read");
        serde_json::from_str(&response).expect("response json")
    }

    fn timeout_vote_request(local: &StatusReport, view: u64) -> RpcRequest {
        RpcRequest::empty("timeout-vote", RPC_FINALITY_TIMEOUT_VOTE_METHOD)
            .with_param("block_height", 1u64)
            .expect("height")
            .with_param("view", view)
            .expect("view")
            .with_param("proxy_required_current_height", 0u64)
            .expect("parent height")
            .with_param("proxy_required_parent_hash", &local.block_tip_hash)
            .expect("parent hash")
            .with_param("proxy_required_state_root", &local.state_root)
            .expect("parent root")
    }

    fn reserved_port() -> (TcpListener, u16) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve port");
        let port = listener.local_addr().expect("port").port();
        (listener, port)
    }

    #[test]
    fn rpc_serve_signs_a_finality_vote_through_the_signer_socket_with_no_key_file() {
        let Some(root) = signer_root("vote") else {
            return;
        };
        let (key_file, local) = chain_with_key_moved_out(&root);
        let socket = root.join("signer/signer.sock");
        start_signer(&key_file, &socket, None);
        let (listener, port) = reserved_port();
        write_topology(&root, port);
        // The first readiness read builds the pinned verifying keys; build
        // them outside the client's read timeout.
        postfiat_node::private_readiness(postfiat_node::PrivateReadinessOptions {
            data_dir: root.join("node"),
            pool_id: None,
            admission_switch: None,
            service: None,
        })
        .expect("build the readiness verifying keys");
        let options = socket_serve_options(&root, &socket, port, 2);
        let server = thread::spawn(move || rpc_serve_inner(options, Some(listener)));
        let response = send(port, &timeout_vote_request(&local, 0));
        assert!(response.ok, "{:?}", response.error);
        // Readiness reports the socket signer and the uid it answered with;
        // in one test process that is this user, so custody is refused as
        // `validator_signer_same_user`, never as an unsafe or missing file.
        let readiness = send(
            port,
            &RpcRequest::new("readiness", "private_readiness", serde_json::json!({})),
        );
        assert!(readiness.ok, "{:?}", readiness.error);
        let report = readiness.result.expect("readiness");
        assert_eq!(report["custody"]["validator_signer"], "socket");
        assert_eq!(report["custody"]["signer_socket"], socket.display().to_string());
        assert_eq!(report["custody"]["signer_uid"], euid());
        assert_eq!(report["custody"]["serving_uid"], euid());
        assert_eq!(report["custody"]["key_files"], serde_json::json!([]));
        let reasons: Vec<String> = report["reasons"]
            .as_array()
            .expect("reasons")
            .iter()
            .filter_map(|reason| reason.as_str().map(str::to_string))
            .collect();
        assert!(reasons.iter().any(|reason| reason == "validator_signer_same_user"), "{reasons:?}");
        for absent in [
            "validator_signer_unsafe_file",
            "validator_signer_unreachable",
            "validator_key_file_missing",
            "validator_key_file_unreadable",
            "validator_key_file_not_owner_only",
        ] {
            assert!(!reasons.iter().any(|reason| reason == absent), "{absent}: {reasons:?}");
        }
        let vote = response
            .result_as::<BlockTimeoutVoteFile>()
            .expect("timeout vote");
        assert_eq!(vote.vote.validator, "validator-0");
        let v2 = vote.consensus_v2_vote.expect("v2 timeout vote");
        assert_eq!(v2.validator, "validator-0");
        server.join().expect("server").expect("rpc serve");
        assert!(!root.join("node").join(VALIDATOR_KEYS_FILE).exists());

        // A signer that stops answering fails the request with a typed
        // refusal; nothing falls back to a key file.
        let gone = root.join("signer/gone.sock");
        start_signer(&key_file, &gone, Some(1));
        let (listener, port) = reserved_port();
        write_topology(&root, port);
        let options = socket_serve_options(&root, &gone, port, 1);
        let server = thread::spawn(move || rpc_serve_inner(options, Some(listener)));
        let response = send(port, &timeout_vote_request(&local, 1));
        assert!(!response.ok);
        let error = response.error.expect("refusal");
        assert!(error.message.contains("validator_signer_unreachable"), "{error:?}");
        server.join().expect("server").expect("rpc serve");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn transport_envelopes_sign_through_the_registered_signer_with_no_key_file() {
        let Some(root) = signer_root("transport") else {
            return;
        };
        let (key_file, _) = chain_with_key_moved_out(&root);
        let data_dir = root.join("node");
        let (_, port) = reserved_port();
        write_topology(&root, port);
        let topology = transport_protocol::read_topology_file(&root.join("topology.json"))
            .expect("topology");
        let frame = postfiat_network::frame_message(
            &transport_protocol::network_domain_from_topology(&topology),
            "validator-0",
            Some("validator-1".to_string()),
            TRANSPORT_BATCH_TOPIC,
            b"payload",
        )
        .expect("frame");
        // Without a registered signer the envelope needs the data
        // directory's key file, which is gone.
        assert!(transport_protocol::sign_transport_envelope_auth(
            &data_dir,
            &topology,
            "validator-0",
            &frame
        )
        .is_err());
        let socket = root.join("signer/transport.sock");
        start_signer(&key_file, &socket, None);
        let registration = postfiat_node::register_validator_signer_socket(
            &socket,
            &data_dir,
            "validator-0",
            Duration::from_secs(5),
        )
        .expect("register signer");
        let auth = transport_protocol::sign_transport_envelope_auth(
            &data_dir,
            &topology,
            "validator-0",
            &frame,
        )
        .expect("sign through the signer");
        transport_protocol::validate_transport_envelope_auth(
            Some(&auth),
            &data_dir,
            &topology,
            &frame,
        )
        .expect("the envelope verifies");
        // A corrupt decoy key file in the data directory and another form of
        // the data directory's path: the envelope still signs through the
        // signer, so nothing read the file, and a key file path inside the
        // data directory is refused.
        fs::write(data_dir.join(VALIDATOR_KEYS_FILE), b"{ decoy, not a key file").expect("decoy");
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(data_dir.join(VALIDATOR_KEYS_FILE), fs::Permissions::from_mode(0o600)).expect("decoy mode");
        }
        let other_form = data_dir.join(".");
        let auth = transport_protocol::sign_transport_envelope_auth(
            &other_form,
            &topology,
            "validator-0",
            &frame,
        )
        .expect("sign through the signer by another path form");
        transport_protocol::validate_transport_envelope_auth(
            Some(&auth),
            &data_dir,
            &topology,
            &frame,
        )
        .expect("the envelope verifies");
        let error = postfiat_node::validator_signer_for(&data_dir.join(VALIDATOR_KEYS_FILE))
            .err()
            .expect("a key file in a socket signer's data directory is refused");
        assert!(
            error.to_string().starts_with("validator_signer_file_refused_in_socket_mode"),
            "{error}"
        );
        drop(registration);
        assert!(postfiat_node::registered_validator_signer_for_data_dir(&data_dir).is_none());
        let _ = fs::remove_dir_all(root);
    }

    /// A key file for `validator-0` holding a key that is not the registry's.
    fn other_key_file(root: &Path, name: &str) -> PathBuf {
        let other = root.join("signer").join(name);
        let pair = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[7u8; 32]);
        fs::write(
            &other,
            serde_json::to_vec_pretty(&serde_json::json!({ "validators": [{
                "node_id": "validator-0",
                "algorithm_id": ML_DSA_65_ALGORITHM,
                "public_key_hex": bytes_to_hex(&pair.public_key),
                "private_key_hex": bytes_to_hex(&pair.private_key),
            }]}))
            .expect("json"),
        )
        .expect("write other key");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&other, fs::Permissions::from_mode(0o600)).expect("mode");
        other
    }

    #[test]
    fn readiness_reports_a_signer_that_holds_another_key() {
        let Some(root) = signer_root("readiness-key") else {
            return;
        };
        let (key_file, _) = chain_with_key_moved_out(&root);
        let other = other_key_file(&root, "readiness-other.json");
        let socket = root.join("signer/swap.sock");
        // The validator's signer answers rpc-serve's startup check and exits.
        start_signer(&key_file, &socket, Some(1));
        let (listener, port) = reserved_port();
        write_topology(&root, port);
        postfiat_node::private_readiness(postfiat_node::PrivateReadinessOptions {
            data_dir: root.join("node"),
            pool_id: None,
            admission_switch: None,
            service: None,
        })
        .expect("build the readiness verifying keys");
        let options = socket_serve_options(&root, &socket, port, 1);
        let server = thread::spawn(move || rpc_serve_inner(options, Some(listener)));
        let started = std::time::Instant::now();
        while postfiat_node::registered_validator_signer_for_data_dir(&root.join("node")).is_none() {
            assert!(started.elapsed() < Duration::from_secs(20), "rpc-serve did not register the signer");
            thread::sleep(Duration::from_millis(20));
        }
        while std::os::unix::net::UnixStream::connect(&socket).is_ok() {
            assert!(started.elapsed() < Duration::from_secs(20), "first signer still serving");
            thread::sleep(Duration::from_millis(20));
        }
        // A signer with another key now answers at the same socket.
        start_signer(&other, &socket, None);
        let readiness = send(
            port,
            &RpcRequest::new("readiness", "private_readiness", serde_json::json!({})),
        );
        assert!(readiness.ok, "{:?}", readiness.error);
        let report = readiness.result.expect("readiness");
        assert_eq!(report["custody"]["signer_key_matches"], false);
        let reasons = report["reasons"].to_string();
        assert!(reasons.contains("validator_signer_key_mismatch"), "{reasons}");
        server.join().expect("server").expect("rpc serve");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rpc_serve_refuses_a_signer_whose_key_is_not_the_validators() {
        let Some(root) = signer_root("mismatch") else {
            return;
        };
        let (_, _) = chain_with_key_moved_out(&root);
        let other = root.join("signer/other.json");
        let pair = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[7u8; 32]);
        fs::write(
            &other,
            serde_json::to_vec_pretty(&serde_json::json!({ "validators": [{
                "node_id": "validator-0",
                "algorithm_id": ML_DSA_65_ALGORITHM,
                "public_key_hex": bytes_to_hex(&pair.public_key),
                "private_key_hex": bytes_to_hex(&pair.private_key),
            }]}))
            .expect("json"),
        )
        .expect("write other key");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&other, fs::Permissions::from_mode(0o600)).expect("mode");
        let socket = root.join("signer/other.sock");
        start_signer(&other, &socket, None);
        let (listener, port) = reserved_port();
        write_topology(&root, port);
        // No request budget: a server that starts returns at once rather
        // than waiting for a client.
        let error = rpc_serve_inner(socket_serve_options(&root, &socket, port, 0), Some(listener))
            .expect_err("a signer with another key must be refused");
        assert!(error.contains("validator_signer_key_mismatch"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rpc_serve_refuses_the_signer_socket_together_with_a_key_file() {
        let error = run_cli(
            [
                "rpc-serve",
                "--unsafe-devnet-json-storage",
                "--port",
                "0",
                "--finality-signer-socket",
                "/nonexistent/signer.sock",
                "--finality-key-file",
                "/nonexistent/validator_keys.json",
            ]
            .map(str::to_string)
            .to_vec(),
        )
        .expect_err("both signers must be refused");
        assert!(error.starts_with("validator_signer_flags_conflict"), "{error}");
    }
}
