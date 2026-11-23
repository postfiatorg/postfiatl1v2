#[cfg(test)]
mod transport_batch_payload_tests {
    use super::*;
    use std::io;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    static CONSENSUS_V2_TRANSPORT_TEST_LOCK: Mutex<()> = Mutex::new(());
    /// Observed after the STO-07 fix: each serving validator opens its
    /// database once at service start, once per connection's first status and
    /// once per request (8); the proposer opens it 5 times (2 peer-health
    /// reads, the round, 2 harness status reads). Before the fix: 126 and 163.
    const MAX_DATABASE_OPENS_PER_VALIDATOR_ROUND: u64 = 8;

    fn send_consensus_v2_test_rpc(port: u16, request: &RpcRequest) -> RpcResponse {
        let mut stream = (0..500)
            .find_map(|_| match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => Some(stream),
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(10));
                    None
                }
            })
            .expect("connect to consensus-v2 test RPC");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("set consensus-v2 test RPC timeout");
        let mut request_line = serde_json::to_vec(request).expect("serialize test RPC request");
        request_line.push(b'\n');
        stream.write_all(&request_line).expect("write test RPC request");
        let mut response_line = String::new();
        BufReader::new(stream)
            .read_line(&mut response_line)
            .expect("read test RPC response");
        serde_json::from_str(&response_line).expect("parse test RPC response")
    }

    fn reserve_consensus_v2_test_ports(
        validator_count: usize,
    ) -> (u16, Vec<TcpListener>, Vec<TcpListener>) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos() as u16;
        for offset in 0..512u16 {
            let base = 30_000u16 + ((seed.wrapping_add(offset * 4)) % 20_000);
            let rpc_base = base.saturating_add(100);
            let listeners = (0..validator_count)
                .map(|index| TcpListener::bind(("127.0.0.1", base + index as u16 * 2)))
                .collect::<io::Result<Vec<_>>>();
            if let Ok(listeners) = listeners {
                let rpc_listeners = (0..validator_count)
                    .map(|index| TcpListener::bind(("127.0.0.1", rpc_base + index as u16)))
                    .collect::<io::Result<Vec<_>>>();
                if let Ok(rpc_listeners) = rpc_listeners {
                    // Keep every transport and RPC port reserved for the entire scenario,
                    // including while its validator is the local proposer. Otherwise
                    // another test or an outbound connection can claim a later round's port.
                    return (base, listeners, rpc_listeners);
                }
            }
        }
        panic!("could not reserve {validator_count} local transport ports");
    }

    fn start_consensus_v2_test_servers(
        data_dirs: &[PathBuf],
        topology_file: &Path,
        listeners: &[TcpListener],
        excluded_index: usize,
        round_label: &str,
    ) -> Vec<std::thread::JoinHandle<Result<TransportValidatorServeReport, String>>> {
        let mut handles = Vec::new();
        for (index, data_dir) in data_dirs.iter().enumerate() {
            if index == excluded_index {
                continue;
            }
            let data_dir = data_dir.clone();
            let topology_file = topology_file.to_path_buf();
            let vote_dir = data_dir.join(format!("{round_label}-transport-votes"));
            let key_file = data_dir.join(VALIDATOR_KEYS_FILE);
            let listener = listeners[index].try_clone().expect("clone reserved listener");
            handles.push(std::thread::spawn(move || {
                let shutdown_requested = std::sync::atomic::AtomicBool::new(false);
                crate::transport_runtime::transport_validator_serve_inner(
                    data_dir,
                    topology_file,
                    key_file,
                    vote_dir,
                    Some("127.0.0.1".to_string()),
                    2,
                    15_000,
                    None,
                    true,
                    Some(TransportShieldedVerifierPrewarmReport {
                        schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
                        requested: true,
                        total_ms: 0.0,
                        asset_orchard_swap_verifier_warm: true,
                        asset_orchard_swap_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_warm: true,
                        asset_orchard_private_egress_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_breakdown: None,
                    }),
                    Some(listener),
                    &shutdown_requested,
                )
            }));
        }
        // The sockets are already listening before any worker starts. Requests
        // queue until the worker accepts them; authenticated health then awaits a
        // real response. Probing readiness with bind() races the worker's bind,
        // and AddrInUse can also mean TIME_WAIT rather than a listening server.
        handles
    }

    fn run_consensus_v2_transport_round(
        data_dirs: &[PathBuf],
        topology_file: &Path,
        listeners: &[TcpListener],
        proposer_index: usize,
        batch_kind: &str,
        batch_file: PathBuf,
        artifact_dir: PathBuf,
        height: u64,
        view: u64,
        timeout_certificate_file: Option<PathBuf>,
        expected_block_vote_count: u64,
    ) {
        let handles = start_consensus_v2_test_servers(
            data_dirs,
            topology_file,
            listeners,
            proposer_index,
            &format!("h{height}-v{view}"),
        );
        let data_dir = data_dirs[proposer_index].clone();
        let topology =
            read_topology_file(&topology_file.to_path_buf()).expect("read health topology");
        let local_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("read health local status");
        let (authenticated, required) =
            crate::transport_runtime::transport_authenticated_peer_health(
            &data_dir,
            &topology,
            &local_status,
            60_000,
        )
            .expect("authenticated persistent peer health");
        assert_eq!(authenticated, data_dirs.len() - 1);
        assert_eq!(required, data_dirs.len() - 1);
        transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
            data_dir: data_dir.clone(),
            topology_file: topology_file.to_path_buf(),
            batch_kind: Some(batch_kind.to_string()),
            batch_file,
            key_file: data_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
            require_local_proposer: true,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: false,
            artifact_dir,
            block_height: Some(height),
            view: Some(view),
            timeout_certificate_file,
            timeout_ms: 15_000,
            send_retries: 2,
            retry_backoff_ms: 50,
            vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
            local_apply_before_certified_send: false,
            defer_certified_sends: false,
            commit_processed_dir: None,
            required_parent: None,
        })
        .expect("consensus v2 transport round");
        crate::transport_runtime::clear_transport_vote_stream_pool_for_test(topology_file)
            .expect("close persistent vote sessions");
        for handle in handles {
            let report = handle
                .join()
                .expect("validator service thread")
                .expect("validator service");
            assert_eq!(report.connection_count, 2);
            assert_eq!(
                report.accepted_block_vote_count,
                expected_block_vote_count
            );
            assert_eq!(report.accepted_health_count, 1);
            assert_eq!(report.accepted_batch_count, 1);
            assert!(report.rejected.is_empty(), "{:?}", report.rejected);
        }
        let statuses = data_dirs
            .iter()
            .map(|data_dir| status(NodeOptions {
                data_dir: data_dir.clone(),
            }))
            .collect::<io::Result<Vec<_>>>()
            .expect("fleet statuses");
        assert!(statuses.iter().all(|status| status.block_height == height));
        assert!(statuses.windows(2).all(|pair| {
            pair[0].block_tip_hash == pair[1].block_tip_hash
                && pair[0].state_root == pair[1].state_root
        }));
    }

    struct ConsensusV2TransportFleet {
        root: PathBuf,
        data_dirs: Vec<PathBuf>,
        topology_file: PathBuf,
        base_port: u16,
        listeners: Vec<TcpListener>,
        rpc_listeners: Vec<TcpListener>,
        validators: postfiat_ordering_fast::ConsensusV2ValidatorSet,
        key_file: ValidatorKeyFile,
    }

    fn consensus_v2_transport_fleet(
        label: &str,
        validator_count: usize,
        storage_activation_height: Option<u64>,
    ) -> ConsensusV2TransportFleet {
        let root = std::env::temp_dir().join(format!(
            "postfiat-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let data_dirs = (0..validator_count)
            .map(|index| {
                let data_dir = root.join(format!("validator-{index}"));
                init_consensus_v2(InitConsensusV2Options {
                    data_dir: data_dir.clone(),
                    chain_id: "postfiat-consensus-v2-transport-test".to_string(),
                    node_id: format!("validator-{index}"),
                    validator_count: validator_count as u32,
                    activation_height: 2,
                    storage_activation_height,
                })
                .expect("init validator");
                data_dir
            })
            .collect::<Vec<_>>();
        let shared_keys = std::fs::read(data_dirs[0].join(VALIDATOR_KEYS_FILE))
            .expect("shared validator keys");
        let key_file: ValidatorKeyFile =
            serde_json::from_slice(&shared_keys).expect("parse shared validator keys");
        let shared_registry = std::fs::read(data_dirs[0].join(VALIDATOR_REGISTRY_FILE))
            .expect("shared validator registry");
        for data_dir in data_dirs.iter().skip(1) {
            std::fs::write(data_dir.join(VALIDATOR_KEYS_FILE), &shared_keys)
                .expect("stage keys");
            std::fs::write(data_dir.join(VALIDATOR_REGISTRY_FILE), &shared_registry)
                .expect("stage registry");
        }
        let bootstrap_snapshot = root.join("bootstrap-snapshot");
        export_snapshot(SnapshotExportOptions {
            data_dir: data_dirs[0].clone(),
            snapshot_dir: bootstrap_snapshot.clone(),
        })
        .expect("export common initial state");
        for (index, data_dir) in data_dirs.iter().enumerate().skip(1) {
            std::fs::remove_dir_all(data_dir).expect("clear initialized replica before restore");
            import_snapshot(SnapshotImportOptions {
                data_dir: data_dir.clone(),
                snapshot_dir: bootstrap_snapshot.clone(),
                node_id: Some(format!("validator-{index}")),
            })
            .expect("import common initial state");
            std::fs::write(data_dir.join(VALIDATOR_KEYS_FILE), &shared_keys)
                .expect("restore shared validator keys after signer-isolated snapshot");
            #[cfg(unix)]
            std::fs::set_permissions(
                data_dir.join(VALIDATOR_KEYS_FILE),
                std::fs::Permissions::from_mode(0o600),
            )
            .expect("protect restored validator keys");
        }
        for (index, data_dir) in data_dirs.iter().enumerate() {
            let node_id = format!("validator-{index}");
            let local_record = key_file
                .validators
                .iter()
                .find(|record| record.node_id == node_id)
                .expect("local validator key")
                .clone();
            let isolated = serde_json::to_vec_pretty(&ValidatorKeyFile {
                validators: vec![local_record],
            })
            .expect("serialize isolated validator key");
            let key_path = data_dir.join(VALIDATOR_KEYS_FILE);
            std::fs::write(&key_path, isolated).expect("write isolated validator key");
            #[cfg(unix)]
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                .expect("protect isolated validator key");
        }
        let topology_file = root.join("topology.json");
        let (base_port, listeners, rpc_listeners) = reserve_consensus_v2_test_ports(validator_count);
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: "postfiat-consensus-v2-transport-test".to_string(),
            validators: validator_count as u32,
            base_port,
            rpc_base_port: Some(base_port.saturating_add(100)),
            hosts: Some(vec!["127.0.0.1".to_string(); validator_count]),
            output_file: topology_file.clone(),
            activation_height: 2,
            storage_activation_height,
        })
        .expect("write topology");
        let (_, validators) =
            live_consensus_v2_context(&data_dirs[0]).expect("consensus context");
        ConsensusV2TransportFleet {
            root,
            data_dirs,
            topology_file,
            base_port,
            listeners,
            rpc_listeners,
            validators,
            key_file,
        }
    }


    fn run_activated_consensus_v2_transport_failed_proposer(validator_count: usize) {
        prewarm_shielded_verifier_cache("consensus v2 transport test")
            .expect("prewarm verifier");
        let ConsensusV2TransportFleet {
            root,
            data_dirs,
            topology_file,
            base_port,
            listeners,
            rpc_listeners,
            validators,
            key_file,
        } = consensus_v2_transport_fleet("consensus-v2-transport", validator_count, None);

        let first_proposer = postfiat_ordering_fast::leader_for_view(
            &validators.validator_ids(),
            1,
            0,
        )
        .expect("first proposer");
        let first_index = validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &first_proposer)
            .expect("first proposer index");
        let first_batch = root.join("height-1.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dirs[first_index].clone(),
            key_file: Some(data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "a".repeat(40)),
            amount: 1,
            batch_file: first_batch.clone(),
        })
        .expect("first batch");
        run_consensus_v2_transport_round(
            &data_dirs,
            &topology_file,
            &listeners,
            first_index,
            "transparent",
            first_batch,
            root.join("height-1-artifacts"),
            1,
            0,
            None,
            1,
        );

        let timeout_votes = data_dirs
            .iter()
            .enumerate()
            .map(|(index, data_dir)| {
                let path = root.join(format!("validator-{index}.h2-v0.timeout.json"));
                create_block_timeout_vote(BlockTimeoutVoteOptions {
                    data_dir: data_dir.clone(),
                    verify_block_log: true,
                    key_file: data_dir.join(VALIDATOR_KEYS_FILE),
                    validator_id: Some(format!("validator-{index}")),
                    block_height: 2,
                    view: 0,
                    high_qc_id: "consensus-v2-no-high-qc".to_string(),
                    vote_file: path.clone(),
                })
                .expect("timeout vote");
                read_transport_json_file::<BlockTimeoutVoteFile>(&path, "timeout vote")
                    .expect("read timeout vote")
            })
            .collect::<Vec<_>>();
        let recovery_proposer = postfiat_ordering_fast::leader_for_view(
            &validators.validator_ids(),
            2,
            1,
        )
        .expect("recovery proposer");
        let recovery_index = validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &recovery_proposer)
            .expect("recovery proposer index");
        let recovery_batch = root.join("height-2.batch.json");
        let recovery_batch_value = create_transfer_batch(BatchTransferOptions {
            data_dir: data_dirs[recovery_index].clone(),
            key_file: Some(data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "b".repeat(40)),
            amount: 10,
            batch_file: recovery_batch.clone(),
        })
        .expect("recovery batch");
        let signed_transfer_json = serde_json::to_string(
            recovery_batch_value
                .transactions
                .first()
                .expect("recovery signed transfer"),
        )
        .expect("serialize recovery transfer");
        let recovery_handles = start_consensus_v2_test_servers(
            &data_dirs,
            &topology_file,
            &listeners,
            recovery_index,
            "h2-v1-shipping-rpc",
        );
        let recovery_rpc_port = base_port
            .saturating_add(100)
            .saturating_add(recovery_index as u16);
        let recovery_ready_file = root.join("height-2-rpc-ready.json");
        let recovery_data_dir = data_dirs[recovery_index].clone();
        let recovery_topology = topology_file.clone();
        let recovery_ready = recovery_ready_file.clone();
        let recovery_rpc_listener = rpc_listeners[recovery_index]
            .try_clone()
            .expect("clone reserved recovery RPC listener");
        let recovery_rpc = std::thread::spawn(move || {
            rpc_serve_inner(RpcServeOptions {
                data_dir: recovery_data_dir.clone(),
                spool_dir: recovery_data_dir.join("runtime/rpc-spool"),
                ready_file: recovery_ready,
                bind_host: "127.0.0.1".to_string(),
                port: recovery_rpc_port,
                max_requests: 1,
                timeout_ms: 30_000,
                child_timeout_ms: 30_000,
                event_log: None,
                allow_mempool_submit: false,
                allow_mempool_submit_finality: true,
                allow_orchard_batch_create: false,
                allow_private_action_admission: false,
                owned_lane_enabled: true,
                finality_topology_file: recovery_topology,
                finality_key_file: recovery_data_dir.join(VALIDATOR_KEYS_FILE),
                finality_proposal_key_file: Some(recovery_data_dir.join(VALIDATOR_KEYS_FILE)),
                finality_signer_socket: None,
                finality_artifact_root: recovery_data_dir.join("finality-artifacts"),
                finality_timeout_ms: 15_000,
                finality_send_retries: 2,
                finality_retry_backoff_ms: 50,
                finality_vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
                finality_quorum_early_full_propagation: false,
                max_mempool_submit_per_peer: 4,
                max_mempool_submit_total: 8,
                max_orchard_batch_create_per_peer: 1,
                max_orchard_batch_create_total: 1,
                max_orchard_batch_create_concurrent: 1,
                max_child_dispatch_concurrent: 8,
                max_child_dispatch_per_peer: 4,
                keep_alive: false,
            }, Some(recovery_rpc_listener))
            .expect("serve recovery finality RPC")
        });
        for _ in 0..500 {
            if recovery_ready_file.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(recovery_ready_file.is_file(), "recovery RPC did not become ready");
        let parent = status(NodeOptions {
            data_dir: data_dirs[recovery_index].clone(),
        })
        .expect("recovery parent status");
        let mut request = RpcRequest::empty(
            "shipping-view-recovery",
            "mempool_submit_signed_transfer_finality",
        )
        .with_param("signed_transfer_json", &signed_transfer_json)
        .expect("signed recovery transfer")
        .with_param("proxy_required_current_height", 1u64)
        .expect("required recovery height")
        .with_param("proxy_required_parent_hash", &parent.block_tip_hash)
        .expect("required recovery hash")
        .with_param("proxy_required_state_root", &parent.state_root)
        .expect("required recovery root")
        .with_param("proxy_consensus_view", 1u64)
        .expect("recovery view");
        for (key, value) in rpc_finality_timeout_vote_params_for_test(&timeout_votes) {
            request = request
                .with_param(key, value)
                .expect("signed timeout vote chunk");
        }
        let response = send_consensus_v2_test_rpc(recovery_rpc_port, &request);
        assert!(response.ok, "recovery finality response: {:?}", response.error);
        let report = response.result.expect("recovery finality result");
        assert_eq!(
            report["certified_sends_deferred"],
            serde_json::Value::Bool(true),
            "recovery finality report: {report}"
        );
        assert_eq!(
            report["finality"]["confirmed"],
            serde_json::Value::Bool(true),
            "recovery finality report: {report}"
        );
        assert_eq!(
            report["finality"]["receipt"]["accepted"],
            serde_json::Value::Bool(true)
        );
        recovery_rpc
            .join()
            .expect("join recovery RPC");
        crate::transport_runtime::clear_transport_vote_stream_pool_for_test(&topology_file)
            .expect("close recovery persistent sessions");
        for handle in recovery_handles {
            let report = handle
                .join()
                .expect("recovery validator service thread")
                .expect("recovery validator service");
            assert_eq!(report.accepted_block_vote_count, 2);
            assert_eq!(report.accepted_batch_count, 1);
            assert!(report.rejected.is_empty(), "{:?}", report.rejected);
        }
        let recovery_statuses = data_dirs
            .iter()
            .map(|data_dir| status(NodeOptions {
                data_dir: data_dir.clone(),
            }))
            .collect::<io::Result<Vec<_>>>()
            .expect("recovery statuses");
        assert!(recovery_statuses.iter().all(|status| status.block_height == 2));
        assert!(recovery_statuses.windows(2).all(|pair| {
            pair[0].block_tip_hash == pair[1].block_tip_hash
                && pair[0].state_root == pair[1].state_root
        }));
        let governance_proposer = postfiat_ordering_fast::leader_for_view(
            &validators.validator_ids(),
            3,
            0,
        )
        .expect("governance proposer");
        let governance_index = validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &governance_proposer)
            .expect("governance proposer index");
        let unsigned_amendment_file = root.join("height-3-governance.unsigned.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dirs[governance_index].clone(),
            validators: validators.validator_ids(),
            support: validators.validator_ids(),
            kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: unsigned_amendment_file.clone(),
        })
        .expect("create six-node governance proposal");
        let authorization_files = key_file
            .validators
            .iter()
            .map(|key_record| {
                let isolated_key_file =
                    root.join(format!("{}.governance-key.json", key_record.node_id));
                let isolated_key_json = serde_json::to_string_pretty(&ValidatorKeyFile {
                    validators: vec![key_record.clone()],
                })
                .expect("serialize isolated governance key");
                std::fs::write(&isolated_key_file, format!("{isolated_key_json}\n"))
                    .expect("write isolated governance key");
                #[cfg(unix)]
                std::fs::set_permissions(
                    &isolated_key_file,
                    std::fs::Permissions::from_mode(0o600),
                )
                .expect("protect isolated governance key");
                let authorization_file =
                    root.join(format!("{}.governance-authorization.json", key_record.node_id));
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: data_dirs[governance_index].clone(),
                    amendment_file: unsigned_amendment_file.clone(),
                    validator: key_record.node_id.clone(),
                    validator_key_file: isolated_key_file,
                    proposal_slot: 3,
                    expires_at_height: 12,
                    authorization_file: authorization_file.clone(),
                })
                .expect("sign isolated governance authorization");
                authorization_file
            })
            .collect::<Vec<_>>();
        let signed_amendment_file = root.join("height-3-governance.signed.json");
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dirs[governance_index].clone(),
            amendment_file: unsigned_amendment_file,
            authorization_files,
            proposal_slot: 3,
            output_file: signed_amendment_file.clone(),
        })
        .expect("assemble six-node governance amendment");
        let governance_batch = root.join("height-3-governance.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dirs[governance_index].clone(),
            amendment_file: Some(signed_amendment_file),
            registry_update_file: None,
            batch_file: governance_batch.clone(),
        })
        .expect("create six-node governance batch");
        run_consensus_v2_transport_round(
            &data_dirs,
            &topology_file,
            &listeners,
            governance_index,
            "governance",
            governance_batch,
            root.join("height-3-artifacts"),
            3,
            0,
            None,
            2,
        );
        let registry_before_rotation: ValidatorRegistry = serde_json::from_slice(
            &std::fs::read(data_dirs[0].join(VALIDATOR_REGISTRY_FILE))
                .expect("read registry before six-node rotation"),
        )
        .expect("parse registry before six-node rotation");
        let validator_ids = validators.validator_ids();
        let previous_registry_root = validator_registry_root_report(ValidatorRegistryRootOptions {
            data_dir: data_dirs[0].clone(),
            registry_file: None,
            validators: validator_ids.clone(),
        })
        .expect("previous six-node registry root")
        .registry_root;
        let rotation_subject = validator_ids.last().expect("rotation subject").clone();
        let previous_record = registry_before_rotation
            .validators
            .iter()
            .find(|record| record.node_id == rotation_subject)
            .expect("previous rotation record");
        let replacement_key = postfiat_crypto_provider::ml_dsa_65_keygen()
            .expect("replacement validator key");
        let previous_entry = ValidatorRegistryEntry {
            node_id: previous_record.node_id.clone(),
            algorithm_id: previous_record.algorithm_id.clone(),
            public_key_hex: previous_record.public_key_hex.clone(),
            active: true,
        };
        let replacement_public_key_hex = bytes_to_hex(&replacement_key.public_key);
        let replacement_entry = ValidatorRegistryEntry {
            node_id: rotation_subject.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: replacement_public_key_hex.clone(),
            active: true,
        };
        let mut registry_after_rotation = registry_before_rotation.clone();
        let replacement_record = registry_after_rotation
            .validators
            .iter_mut()
            .find(|record| record.node_id == rotation_subject)
            .expect("replacement registry record");
        replacement_record.algorithm_id = ML_DSA_65_ALGORITHM.to_string();
        replacement_record.public_key_hex = replacement_public_key_hex;
        let proposed_registry_file = root.join("rotation.proposed-registry.json");
        let proposed_registry_json =
            serde_json::to_string_pretty(&registry_after_rotation).expect("proposed registry json");
        std::fs::write(
            &proposed_registry_file,
            format!("{proposed_registry_json}\n"),
        )
        .expect("write proposed registry");
        let new_registry_root = validator_registry_root_report(ValidatorRegistryRootOptions {
            data_dir: data_dirs[0].clone(),
            registry_file: Some(proposed_registry_file),
            validators: validator_ids.clone(),
        })
        .expect("new six-node registry root")
        .registry_root;
        assert_ne!(previous_registry_root, new_registry_root);
        let previous_entry_file = root.join("rotation.previous-entry.json");
        let replacement_entry_file = root.join("rotation.replacement-entry.json");
        for (path, entry) in [
            (&previous_entry_file, &previous_entry),
            (&replacement_entry_file, &replacement_entry),
        ] {
            let json = serde_json::to_string_pretty(entry).expect("registry entry json");
            std::fs::write(path, format!("{json}\n")).expect("write registry entry");
        }
        let unsigned_update_file = root.join("height-4-registry-update.unsigned.json");
        create_validator_registry_update(ValidatorRegistryUpdateOptions {
            data_dir: data_dirs[0].clone(),
            validators: validator_ids.clone(),
            support: validator_ids.clone(),
            activation_height: 5,
            previous_registry_root: previous_registry_root.clone(),
            new_registry_root: new_registry_root.clone(),
            previous_validators: validator_ids.clone(),
            new_validators: validator_ids.clone(),
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
            subject_node_id: rotation_subject.clone(),
            previous_record_file: Some(previous_entry_file),
            new_record_file: Some(replacement_entry_file),
            update_file: unsigned_update_file.clone(),
        })
        .expect("create six-node key rotation proposal");
        let rotation_authorization_files = key_file
            .validators
            .iter()
            .map(|key_record| {
                let isolated_key_file =
                    root.join(format!("{}.rotation-key.json", key_record.node_id));
                let key_json = serde_json::to_string_pretty(&ValidatorKeyFile {
                    validators: vec![key_record.clone()],
                })
                .expect("serialize isolated rotation key");
                std::fs::write(&isolated_key_file, format!("{key_json}\n"))
                    .expect("write isolated rotation key");
                #[cfg(unix)]
                std::fs::set_permissions(
                    &isolated_key_file,
                    std::fs::Permissions::from_mode(0o600),
                )
                .expect("protect isolated rotation key");
                let authorization_file =
                    root.join(format!("{}.rotation-authorization.json", key_record.node_id));
                sign_validator_registry_update_authorization(
                    ValidatorRegistryAuthorizationSignOptions {
                        data_dir: data_dirs[0].clone(),
                        update_file: unsigned_update_file.clone(),
                        validator: key_record.node_id.clone(),
                        validator_key_file: isolated_key_file,
                        proposal_slot: 4,
                        expires_at_height: 12,
                        authorization_file: authorization_file.clone(),
                    },
                )
                .expect("old committee signs key rotation");
                authorization_file
            })
            .collect::<Vec<_>>();
        let signed_update_file = root.join("height-4-registry-update.signed.json");
        assemble_signed_validator_registry_update(ValidatorRegistryUpdateAssembleOptions {
            data_dir: data_dirs[0].clone(),
            update_file: unsigned_update_file,
            authorization_files: rotation_authorization_files,
            proposal_slot: 4,
            output_file: signed_update_file.clone(),
        })
        .expect("assemble signed six-node key rotation");
        let rotation_batch = root.join("height-4-registry-update.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dirs[0].clone(),
            amendment_file: None,
            registry_update_file: Some(signed_update_file),
            batch_file: rotation_batch.clone(),
        })
        .expect("create signed six-node rotation batch");
        let rotation_proposer = postfiat_ordering_fast::leader_for_view(&validator_ids, 4, 0)
            .expect("rotation proposer");
        let rotation_index = validator_ids
            .iter()
            .position(|validator| validator == &rotation_proposer)
            .expect("rotation proposer index");
        run_consensus_v2_transport_round(
            &data_dirs,
            &topology_file,
            &listeners,
            rotation_index,
            "governance",
            rotation_batch,
            root.join("height-4-artifacts"),
            4,
            0,
            None,
            2,
        );
        for data_dir in &data_dirs {
            assert_eq!(
                validator_registry_root_report(ValidatorRegistryRootOptions {
                    data_dir: data_dir.clone(),
                    registry_file: None,
                    validators: validator_ids.clone(),
                })
                .expect("preactivation registry root")
                .registry_root,
                previous_registry_root
            );
        }
        let activation_proposer = postfiat_ordering_fast::leader_for_view(&validator_ids, 5, 0)
            .expect("rotation activation proposer");
        let activation_index = validator_ids
            .iter()
            .position(|validator| validator == &activation_proposer)
            .expect("rotation activation proposer index");
        let activation_batch = root.join("height-5-rotation-activation.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dirs[activation_index].clone(),
            key_file: Some(data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "c".repeat(40)),
            amount: 1,
            batch_file: activation_batch.clone(),
        })
        .expect("create rotation activation batch");
        run_consensus_v2_transport_round(
            &data_dirs,
            &topology_file,
            &listeners,
            activation_index,
            "transparent",
            activation_batch,
            root.join("height-5-artifacts"),
            5,
            0,
            None,
            2,
        );
        for data_dir in &data_dirs {
            assert_eq!(
                validator_registry_root_report(ValidatorRegistryRootOptions {
                    data_dir: data_dir.clone(),
                    registry_file: None,
                    validators: validator_ids.clone(),
                })
                .expect("activated registry root")
                .registry_root,
                new_registry_root
            );
            verify_blocks(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("v2 chain replay");
            assert_eq!(
                NodeStore::new(data_dir)
                    .read_governance()
                    .expect("six-node governance")
                    .crypto_policy_version,
                2
            );
        }

        let mut rotated_key_file = key_file.clone();
        let rotated_key_record = rotated_key_file
            .validators
            .iter_mut()
            .find(|record| record.node_id == rotation_subject)
            .expect("rotated validator key record");
        rotated_key_record.algorithm_id = ML_DSA_65_ALGORITHM.to_string();
        rotated_key_record.public_key_hex = bytes_to_hex(&replacement_key.public_key);
        rotated_key_record.private_key_hex = bytes_to_hex(&replacement_key.private_key);
        for (index, data_dir) in data_dirs.iter().enumerate() {
            let node_id = format!("validator-{index}");
            let local_record = rotated_key_file
                .validators
                .iter()
                .find(|record| record.node_id == node_id)
                .expect("rotated local validator key")
                .clone();
            let rotated_key_json = serde_json::to_string_pretty(&ValidatorKeyFile {
                validators: vec![local_record],
            })
            .expect("rotated isolated key file json");
            std::fs::write(
                data_dir.join(VALIDATOR_KEYS_FILE),
                format!("{rotated_key_json}\n"),
            )
            .expect("stage rotated validator key");
            #[cfg(unix)]
            std::fs::set_permissions(
                data_dir.join(VALIDATOR_KEYS_FILE),
                std::fs::Permissions::from_mode(0o600),
            )
            .expect("protect rotated validator keys");
        }
        let (_, rotated_validators) =
            live_consensus_v2_context(&data_dirs[0]).expect("rotated consensus context");
        let post_rotation_proposer = postfiat_ordering_fast::leader_for_view(
            &rotated_validators.validator_ids(),
            6,
            0,
        )
        .expect("post-rotation proposer");
        let post_rotation_index = rotated_validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &post_rotation_proposer)
            .expect("post-rotation proposer index");
        let post_rotation_batch = root.join("height-6-post-rotation.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dirs[post_rotation_index].clone(),
            key_file: Some(data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "d".repeat(40)),
            amount: 1,
            batch_file: post_rotation_batch.clone(),
        })
        .expect("create post-rotation batch");
        run_consensus_v2_transport_round(
            &data_dirs,
            &topology_file,
            &listeners,
            post_rotation_index,
            "transparent",
            post_rotation_batch,
            root.join("height-6-artifacts"),
            6,
            0,
            None,
            2,
        );
        for data_dir in &data_dirs {
            verify_blocks(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("post-rotation v2 chain replay");
        }

        let fresh_backup = postfiat_rpc_sdk::wallet_backup_from_master_seed(
            "postfiat-consensus-v2-transport-test",
            "7e".repeat(32),
            0,
        )
        .expect("fresh wallet backup");
        let fresh_key =
            postfiat_rpc_sdk::derive_wallet_key_pair(&fresh_backup).expect("fresh wallet key");
        let fresh_address =
            postfiat_crypto_provider::address_from_public_key(&fresh_key.public_key);
        assert!(data_dirs.iter().all(|data_dir| NodeStore::new(data_dir)
            .read_ledger()
            .expect("pre-funding ledger")
            .account(&fresh_address)
            .is_none()));
        let funding_proposer = postfiat_ordering_fast::leader_for_view(
            &rotated_validators.validator_ids(),
            7,
            0,
        )
        .expect("fresh wallet funding proposer");
        let funding_index = rotated_validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &funding_proposer)
            .expect("fresh wallet funding proposer index");
        let funding_batch = root.join("height-7-fresh-wallet-funding.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dirs[funding_index].clone(),
            key_file: Some(data_dirs[0].join("faucet_key.json")),
            to: fresh_address.clone(),
            amount: 100,
            batch_file: funding_batch.clone(),
        })
        .expect("create fresh wallet funding batch");
        run_consensus_v2_transport_round(
            &data_dirs,
            &topology_file,
            &listeners,
            funding_index,
            "transparent",
            funding_batch,
            root.join("height-7-artifacts"),
            7,
            0,
            None,
            2,
        );

        let deposit_proposer = postfiat_ordering_fast::leader_for_view(
            &rotated_validators.validator_ids(),
            8,
            0,
        )
        .expect("owned deposit proposer");
        let deposit_index = rotated_validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &deposit_proposer)
            .expect("owned deposit proposer index");
        let deposit_store = NodeStore::new(&data_dirs[deposit_index]);
        let deposit_genesis = deposit_store.read_genesis().expect("deposit genesis");
        let deposit_source_pubkey = fresh_key.public_key.clone();
        let deposit_private_key = fresh_key.private_key;
        let deposit_genesis_hash: [u8; 48] =
            hex_to_bytes(&postfiat_execution::genesis_hash(&deposit_genesis))
            .expect("deposit genesis hash")
            .try_into()
            .expect("deposit genesis hash length");
        let before_deposit = deposit_store.read_ledger().expect("pre-deposit ledger");
        let before_deposit_balance = before_deposit
            .account(&fresh_address)
            .expect("pre-deposit source")
            .balance;
        let before_owned_count = before_deposit.owned_objects.len();
        let deposit = postfiat_types::OwnedDepositV1 {
            domain: postfiat_types::FastSwapChainDomainV1 {
                chain_id: deposit_genesis.chain_id,
                genesis_hash: postfiat_types::FastSwapOpaqueHashV1(deposit_genesis_hash),
                protocol_version: deposit_genesis.protocol_version,
            },
            source_address: fresh_address.clone(),
            source_pubkey: deposit_source_pubkey.clone(),
            sequence: before_deposit
                .account(&fresh_address)
                .expect("deposit source")
                .sequence
                + 1,
            fee_pft: 2,
            destination_owner_pubkey: deposit_source_pubkey,
            asset: "PFT".to_owned(),
            amount_atoms: 40,
            valid_through_height: 20,
            nonce: [0x7d; 32],
        };
        let signed_deposit = postfiat_types::SignedOwnedDepositV1 {
            signature: ml_dsa_65_sign_with_context(
                &deposit_private_key,
                &deposit.signing_bytes().expect("deposit signing bytes"),
                postfiat_types::OWNED_DEPOSIT_CONTEXT_V1,
            )
            .expect("sign owned deposit"),
            algorithm_id: ML_DSA_65_ALGORITHM.to_owned(),
            deposit,
        };
        let deposit_transaction = postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit {
                signed: signed_deposit,
            },
        };
        let deposit_entry = admit_fastlane_primary_to_mempool(
            &data_dirs[deposit_index],
            deposit_transaction,
        )
        .expect("admit owned deposit");
        let deposit_batch = root.join("height-8-owned-deposit.batch.json");
        create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dirs[deposit_index].clone(),
            batch_file: deposit_batch.clone(),
            max_transactions: 1,
        })
        .expect("create owned deposit batch");
        run_consensus_v2_transport_round(
            &data_dirs,
            &topology_file,
            &listeners,
            deposit_index,
            "transparent",
            deposit_batch,
            root.join("height-8-artifacts"),
            8,
            0,
            None,
            2,
        );
        for data_dir in &data_dirs {
            let store = NodeStore::new(data_dir);
            let receipts = store.read_receipts().expect("owned deposit receipts");
            let receipt = receipts
                .iter()
                .find(|receipt| receipt.tx_id == deposit_entry.tx_id)
                .expect("owned deposit receipt");
            assert!(receipt.accepted);
            assert_eq!(receipt.code, "owned_deposit_applied");
            assert_eq!(receipt.fee_burned, 2);
            let ledger = store.read_ledger().expect("post-deposit ledger");
            let account = ledger
                .account(&fresh_address)
                .expect("post-deposit source");
            assert_eq!(account.balance, before_deposit_balance - 42);
            assert_eq!(ledger.owned_objects.len(), before_owned_count + 1);
            let owned = ledger.owned_objects.last().expect("deposited object");
            assert_eq!(owned.value, 40);
            assert_eq!(owned.asset, "PFT");
            assert_eq!(account.balance + owned.value + 2, before_deposit_balance);
            assert!(verify_blocks(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("post-deposit v2 chain replay")
            .verified);
        }
        std::fs::remove_dir_all(root).expect("cleanup transport test");
    }

    #[test]
    fn activated_consensus_v2_transport_survives_failed_view_zero_proposer_n4() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        run_activated_consensus_v2_transport_failed_proposer(4);
    }

    #[test]
    fn activated_consensus_v2_transport_survives_failed_view_zero_proposer_n6() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        run_activated_consensus_v2_transport_failed_proposer(6);
    }

    fn transactional_database_directory(data_dir: &Path) -> PathBuf {
        NodeStore::new(data_dir)
            .transactional_generation_pointer()
            .expect("read generation pointer")
            .map(|pointer| pointer.database_directory)
            .unwrap_or_else(|| data_dir.to_path_buf())
    }

    fn database_opens(database_directories: &[PathBuf]) -> Vec<u64> {
        database_directories
            .iter()
            .map(|directory| {
                postfiat_storage::transactional::transactional_open_counts_for(directory)
                    .database_opens
            })
            .collect()
    }

    /// An rpc-serve-style reader on another thread: a read-only open with no
    /// retry, which redb refuses while any writer handle is open.
    fn assert_database_free_for_rpc_reader(data_dirs: &[PathBuf]) {
        for data_dir in data_dirs {
            let data_dir = data_dir.clone();
            std::thread::spawn(move || {
                let store = NodeStore::new(&data_dir);
                let directory = transactional_database_directory(&data_dir);
                store
                    .open_transactional_store_read_only_at(&directory)
                    .expect("rpc-serve-style reader opens the database between rounds")
                    .meta()
                    .expect("rpc-serve-style reader reads the store meta");
            })
            .join()
            .expect("rpc-serve-style reader thread");
        }
    }

    /// Run one round and return each validator's database opens during it.
    fn transactional_six_validator_round(
        fleet: &ConsensusV2TransportFleet,
        height: u64,
        expected_block_vote_count: u64,
    ) -> Vec<u64> {
        let proposer =
            postfiat_ordering_fast::leader_for_view(&fleet.validators.validator_ids(), height, 0)
                .expect("proposer");
        let proposer_index = fleet
            .validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let batch = fleet.root.join(format!("height-{height}.batch.json"));
        create_transfer_batch(BatchTransferOptions {
            data_dir: fleet.data_dirs[proposer_index].clone(),
            key_file: Some(fleet.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "c".repeat(40)),
            amount: height,
            batch_file: batch.clone(),
        })
        .expect("transfer batch");
        let directories = fleet
            .data_dirs
            .iter()
            .map(|data_dir| transactional_database_directory(data_dir))
            .collect::<Vec<_>>();
        let before = database_opens(&directories);
        run_consensus_v2_transport_round(
            &fleet.data_dirs,
            &fleet.topology_file,
            &fleet.listeners,
            proposer_index,
            "transparent",
            batch,
            fleet.root.join(format!("height-{height}-artifacts")),
            height,
            0,
            None,
            expected_block_vote_count,
        );
        database_opens(&directories)
            .iter()
            .zip(&before)
            .map(|(after, before)| after - before)
            .collect()
    }

    /// STO-07: one consensus round must not reopen each validator's redb
    /// database per store lookup, and rpc-serve must still be able to open it
    /// between rounds.
    #[test]
    fn transactional_six_validator_round_opens_each_database_a_bounded_number_of_times() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("transactional open-count test").expect("prewarm");
        let fleet = consensus_v2_transport_fleet("transactional-open-count", 6, Some(2));
        let _ = transactional_six_validator_round(&fleet, 1, 1);
        // Snapshot import builds no store before activation; build each
        // replica's verified generation at the shared height-1 tip.
        for data_dir in fleet.data_dirs.iter().skip(1) {
            let tip = status(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("replica status");
            rebuild_transactional_storage(StorageMigrationOptions {
                data_dir: data_dir.clone(),
                output_dir: data_dir.join("transactional-generation-v1"),
                expected_tip: tip.block_tip_hash,
                expected_state_root: tip.state_root,
                verify_only: false,
            })
            .expect("build the replica's transactional store");
        }
        let _ = transactional_six_validator_round(&fleet, 2, 2);
        for data_dir in &fleet.data_dirs {
            let storage = status(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("status")
            .storage
            .expect("transactional storage status");
            assert!(storage.transactional_active, "{storage:?}");
        }
        assert_database_free_for_rpc_reader(&fleet.data_dirs);

        let opens = transactional_six_validator_round(&fleet, 3, 2);
        eprintln!("STO-07 database opens per validator for one round: {opens:?}");
        assert_database_free_for_rpc_reader(&fleet.data_dirs);
        assert!(
            opens.iter().all(|count| *count <= MAX_DATABASE_OPENS_PER_VALIDATOR_ROUND),
            "per-validator database opens {opens:?} exceed {MAX_DATABASE_OPENS_PER_VALIDATOR_ROUND}"
        );
        std::fs::remove_dir_all(&fleet.root).expect("cleanup transactional open-count fleet");
    }

    /// Serve exactly `connections` batch connections for one validator, as a
    /// freshly started process would, and fail instead of hanging.
    fn serve_validator_once(
        fleet: &ConsensusV2TransportFleet,
        index: usize,
        connections: usize,
        label: &str,
    ) -> std::thread::JoinHandle<Result<TransportValidatorServeReport, String>> {
        let data_dir = fleet.data_dirs[index].clone();
        let topology_file = fleet.topology_file.clone();
        let vote_dir = data_dir.join(format!("{label}-transport-votes"));
        let key_file = data_dir.join(VALIDATOR_KEYS_FILE);
        let listener = fleet.listeners[index]
            .try_clone()
            .expect("clone reserved listener");
        std::thread::spawn(move || {
            let shutdown_requested = std::sync::atomic::AtomicBool::new(false);
            crate::transport_runtime::transport_validator_serve_inner(
                data_dir,
                topology_file,
                key_file,
                vote_dir,
                Some("127.0.0.1".to_string()),
                connections,
                15_000,
                None,
                true,
                Some(TransportShieldedVerifierPrewarmReport {
                    schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
                    requested: true,
                    total_ms: 0.0,
                    asset_orchard_swap_verifier_warm: true,
                    asset_orchard_swap_verifier_ms: Some(0.0),
                    asset_orchard_private_egress_verifier_warm: true,
                    asset_orchard_private_egress_verifier_ms: Some(0.0),
                    asset_orchard_private_egress_verifier_breakdown: None,
                }),
                Some(listener),
                &shutdown_requested,
            )
        })
    }

    fn join_validator_service(
        handle: std::thread::JoinHandle<Result<TransportValidatorServeReport, String>>,
    ) -> TransportValidatorServeReport {
        let deadline = Instant::now() + Duration::from_secs(120);
        while !handle.is_finished() {
            assert!(Instant::now() < deadline, "validator service did not finish");
            std::thread::sleep(Duration::from_millis(20));
        }
        handle
            .join()
            .expect("validator service thread")
            .expect("validator service")
    }

    fn blocks_with_batch_id(data_dir: &Path, batch_id: &str) -> usize {
        NodeStore::new(data_dir)
            .read_blocks()
            .expect("read block log")
            .blocks
            .iter()
            .filter(|block| block.header.batch_id == batch_id)
            .count()
    }

    /// A certified send whose acknowledgement was lost is redelivered to a
    /// transactional target that already holds the identical block. The
    /// target answers `already_applied`, the sender's job completes, the block
    /// is stored once, and the next proposal is not refused.
    #[test]
    fn certified_send_redelivery_after_a_lost_ack_completes_as_already_applied() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("duplicate certified send test").expect("prewarm");
        let fleet = consensus_v2_transport_fleet("duplicate-certified-send", 4, Some(2));
        let _ = transactional_six_validator_round(&fleet, 1, 1);
        for data_dir in fleet.data_dirs.iter().skip(1) {
            let tip = status(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("replica status");
            rebuild_transactional_storage(StorageMigrationOptions {
                data_dir: data_dir.clone(),
                output_dir: data_dir.join("transactional-generation-v1"),
                expected_tip: tip.block_tip_hash,
                expected_state_root: tip.state_root,
                verify_only: false,
            })
            .expect("build the replica's transactional store");
        }
        let _ = transactional_six_validator_round(&fleet, 2, 2);

        // The next proposer holds a job for block 2 whose acknowledgement from
        // the target never arrived, exactly as the outbox records it.
        let ids = fleet.validators.validator_ids();
        let next_proposer = postfiat_ordering_fast::leader_for_view(&ids, 3, 0).expect("leader");
        let source = ids
            .iter()
            .position(|validator| validator == &next_proposer)
            .expect("source index");
        let target = (source + 1) % fleet.data_dirs.len();
        let source_dir = fleet.data_dirs[source].clone();
        let target_dir = fleet.data_dirs[target].clone();
        let source_status = status(NodeOptions {
            data_dir: source_dir.clone(),
        })
        .expect("source status");
        let target_before = status(NodeOptions {
            data_dir: target_dir.clone(),
        })
        .expect("target status");
        assert_eq!(target_before.block_height, 2);
        let certificate_file = fleet.root.join("height-2-artifacts/block-certificate.json");
        let certificate: BlockCertificateFile = serde_json::from_slice(
            &std::fs::read(&certificate_file).expect("read height-2 certificate"),
        )
        .expect("parse height-2 certificate");
        let topology = read_topology_file(&fleet.topology_file).expect("topology");
        let job_file = enqueue_durable_certified_send_job(
            &source_dir,
            &topology,
            &format!("validator-{source}"),
            &format!("validator-{target}"),
            "transparent",
            2,
            &certificate.certificate_id,
            &source_status.block_tip_hash,
            &source_status.state_root,
            &fleet.root.join("height-2.batch.json"),
            &certificate_file,
            15_000,
            0,
            0,
        )
        .expect("enqueue the redelivery job");
        let mut job = read_durable_certified_send_job(&job_file).expect("read job");
        job.attempt_count = 1;
        job.last_error = Some(
            "transport batch ack read failed: Resource temporarily unavailable (os error 11)"
                .to_string(),
        );
        write_durable_certified_send_job(&job_file, &job).expect("record the lost ack");
        let batch_id = NodeStore::new(&target_dir)
            .read_blocks()
            .expect("target blocks")
            .blocks
            .iter()
            .find(|block| block.header.height == 2)
            .expect("block 2")
            .header
            .batch_id
            .clone();
        assert_eq!(blocks_with_batch_id(&target_dir, &batch_id), 1);

        // The target restarted after applying block 2: a fresh service on the
        // same store answers the redelivery.
        let service = serve_validator_once(&fleet, target, 1, "redelivery");
        let resumed = resume_durable_certified_send_outbox_pending_only(
            &source_dir,
            &fleet.topology_file,
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("resume outbox");
        let served = join_validator_service(service);
        assert!(
            resumed.all_completed,
            "redelivery must complete: {:?}",
            resumed.targets
        );
        assert_eq!(resumed.pending, 0);
        assert_eq!(resumed.targets.len(), 1);
        assert_eq!(resumed.targets[0].result, "completed");
        assert!(resumed.targets[0].already_applied);
        assert_eq!(served.accepted_batch_count, 1);
        assert!(served.batch_acks[0].already_applied);
        let job = read_durable_certified_send_job(&job_file).expect("reread job");
        assert!(job.completed);
        assert!(job.ack.as_ref().is_some_and(|ack| ack.already_applied));

        let target_after = status(NodeOptions {
            data_dir: target_dir.clone(),
        })
        .expect("target status after redelivery");
        assert_eq!(target_after.block_height, 2);
        assert_eq!(target_after.block_tip_hash, target_before.block_tip_hash);
        assert_eq!(target_after.state_root, target_before.state_root);
        assert_eq!(blocks_with_batch_id(&target_dir, &batch_id), 1);

        // Same batch key, different certificate content: never acknowledged.
        let other_target = (target + 1) % fleet.data_dirs.len();
        assert_ne!(other_target, source);
        let mut conflicting = certificate_json(&certificate_file);
        conflicting["certificate"]["registry_root"] = serde_json::json!("77".repeat(48));
        let error = assert_redelivery_is_a_conflict(&fleet, source, other_target, &conflicting);
        // Release 6.4's check names one conflict for every differing field.
        assert!(
            error.contains(&format!(
                "already-applied batch `{batch_id}` conflicts with certified height/hash"
            )),
            "{error}"
        );

        // The completed job no longer holds the proposer back.
        let _ = transactional_six_validator_round(&fleet, 3, 2);
        std::fs::remove_dir_all(&fleet.root).expect("cleanup duplicate certified send fleet");
    }

    fn certificate_json(certificate_file: &Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(certificate_file).expect("read certificate"))
            .expect("certificate json")
    }

    /// Redeliver block 2 from `source` to `target` (which already holds it)
    /// with `certificate`, through the real outbox and a fresh target service.
    /// The job must stay pending with `certified_send_duplicate_conflict`; the
    /// job is then dropped so later rounds are not held back. Returns the error.
    fn assert_redelivery_is_a_conflict(
        fleet: &ConsensusV2TransportFleet,
        source: usize,
        target: usize,
        certificate: &serde_json::Value,
    ) -> String {
        let source_dir = &fleet.data_dirs[source];
        let source_status = status(NodeOptions {
            data_dir: source_dir.clone(),
        })
        .expect("source status");
        let target_before = status(NodeOptions {
            data_dir: fleet.data_dirs[target].clone(),
        })
        .expect("target status");
        let certificate_id = certificate["certificate_id"]
            .as_str()
            .expect("certificate id")
            .to_string();
        let certificate_file = fleet
            .root
            .join(format!("height-2-conflict-{source}-{target}.json"));
        std::fs::write(&certificate_file, certificate.to_string()).expect("write certificate");
        let topology = read_topology_file(&fleet.topology_file).expect("topology");
        let job_file = enqueue_durable_certified_send_job(
            source_dir,
            &topology,
            &format!("validator-{source}"),
            &format!("validator-{target}"),
            "transparent",
            2,
            &certificate_id,
            &source_status.block_tip_hash,
            &source_status.state_root,
            &fleet.root.join("height-2.batch.json"),
            &certificate_file,
            15_000,
            0,
            0,
        )
        .expect("enqueue the conflicting job");
        let service = serve_validator_once(fleet, target, 1, "conflict");
        let conflict = resume_durable_certified_send_outbox_pending_only(
            source_dir,
            &fleet.topology_file,
            CERTIFIED_SEND_OUTBOX_MAX_JOBS,
        )
        .expect("resume conflicting outbox");
        let _ = join_validator_service(service);
        assert!(!conflict.all_completed);
        assert_eq!(conflict.pending, 1);
        let pending = conflict
            .targets
            .iter()
            .find(|report| report.target == format!("validator-{target}"))
            .expect("conflicting target report");
        assert_eq!(pending.result, "pending");
        assert!(!pending.already_applied);
        let error = pending.error.clone().expect("named conflict");
        assert!(
            error.contains(
                "transport batch service apply failed: certified_send_duplicate_conflict: \
                 already-applied batch `"
            ),
            "{error}"
        );
        let job = read_durable_certified_send_job(&job_file).expect("job");
        assert!(!job.completed);
        assert!(job.ack.is_none());
        let target_after = status(NodeOptions {
            data_dir: fleet.data_dirs[target].clone(),
        })
        .expect("target status after conflict");
        assert_eq!(target_after.block_tip_hash, target_before.block_tip_hash);
        assert_eq!(target_after.state_root, target_before.state_root);
        std::fs::remove_dir_all(job_file.parent().expect("conflict job dir"))
            .expect("drop the conflicting job before the next round");
        error
    }

    /// A redelivered block whose consensus-v2 commit is missing or altered is
    /// not the block the target holds, even when the legacy certificate
    /// matches: the job stays pending with `certified_send_duplicate_conflict`.
    #[test]
    fn certified_send_redelivery_with_a_different_consensus_v2_commit_stays_pending() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("duplicate v2 commit test").expect("prewarm");
        let fleet = consensus_v2_transport_fleet("duplicate-v2-commit", 4, Some(2));
        let _ = transactional_six_validator_round(&fleet, 1, 1);
        for data_dir in fleet.data_dirs.iter().skip(1) {
            let tip = status(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("replica status");
            rebuild_transactional_storage(StorageMigrationOptions {
                data_dir: data_dir.clone(),
                output_dir: data_dir.join("transactional-generation-v1"),
                expected_tip: tip.block_tip_hash,
                expected_state_root: tip.state_root,
                verify_only: false,
            })
            .expect("build the replica's transactional store");
        }
        let _ = transactional_six_validator_round(&fleet, 2, 2);
        let ids = fleet.validators.validator_ids();
        let next_proposer = postfiat_ordering_fast::leader_for_view(&ids, 3, 0).expect("leader");
        let source = ids
            .iter()
            .position(|validator| validator == &next_proposer)
            .expect("source index");
        let delivered =
            certificate_json(&fleet.root.join("height-2-artifacts/block-certificate.json"));
        assert!(
            delivered["consensus_v2_commit"].is_object(),
            "block 2 must carry a consensus-v2 commit"
        );
        let batch_id = NodeStore::new(&fleet.data_dirs[source])
            .read_blocks()
            .expect("source blocks")
            .blocks
            .iter()
            .find(|block| block.header.height == 2)
            .expect("block 2")
            .header
            .batch_id
            .clone();
        let conflict =
            format!("already-applied batch `{batch_id}` conflicts with certified height/hash");

        // The consensus-v2 commit removed.
        let target = (source + 1) % fleet.data_dirs.len();
        let mut removed = delivered.clone();
        removed
            .as_object_mut()
            .expect("certificate object")
            .remove("consensus_v2_commit");
        let error = assert_redelivery_is_a_conflict(&fleet, source, target, &removed);
        assert!(error.contains(&conflict), "{error}");

        // The precommit votes emptied.
        let target = (source + 2) % fleet.data_dirs.len();
        let mut emptied = delivered;
        emptied["consensus_v2_commit"]["precommit_qc"]["votes"] = serde_json::json!([]);
        let error = assert_redelivery_is_a_conflict(&fleet, source, target, &emptied);
        assert!(error.contains(&conflict), "{error}");

        // Neither conflict holds the proposer back.
        let _ = transactional_six_validator_round(&fleet, 3, 2);
        std::fs::remove_dir_all(&fleet.root).expect("cleanup duplicate v2 commit fleet");
    }

    #[test]
    fn peer_round_routes_to_remote_proposer_with_host_local_keys() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("remote proposer transport test")
            .expect("prewarm verifier");
        let validator_count = 4usize;
        let root = std::env::temp_dir().join(format!(
            "postfiat-remote-proposer-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let data_dirs = (0..validator_count)
            .map(|index| {
                let data_dir = root.join(format!("validator-{index}"));
                init_consensus_v2(InitConsensusV2Options {
                    data_dir: data_dir.clone(),
                    chain_id: "postfiat-remote-proposer-test".to_string(),
                    node_id: format!("validator-{index}"),
                    validator_count: validator_count as u32,
                    activation_height: 1,
                    storage_activation_height: None,
                })
                .expect("init validator");
                data_dir
            })
            .collect::<Vec<_>>();
        let shared_keys_bytes = std::fs::read(data_dirs[0].join(VALIDATOR_KEYS_FILE))
            .expect("read generated validator keys");
        let shared_keys: ValidatorKeyFile =
            serde_json::from_slice(&shared_keys_bytes).expect("parse generated validator keys");
        let bootstrap_snapshot = root.join("bootstrap-snapshot");
        export_snapshot(SnapshotExportOptions {
            data_dir: data_dirs[0].clone(),
            snapshot_dir: bootstrap_snapshot.clone(),
        })
        .expect("export common state");
        for (index, data_dir) in data_dirs.iter().enumerate().skip(1) {
            std::fs::remove_dir_all(data_dir).expect("clear initialized replica");
            import_snapshot(SnapshotImportOptions {
                data_dir: data_dir.clone(),
                snapshot_dir: bootstrap_snapshot.clone(),
                node_id: Some(format!("validator-{index}")),
            })
            .expect("import common state");
        }
        for (index, data_dir) in data_dirs.iter().enumerate() {
            let node_id = format!("validator-{index}");
            let record = shared_keys
                .validators
                .iter()
                .find(|record| record.node_id == node_id)
                .expect("local validator key")
                .clone();
            let isolated = serde_json::to_vec_pretty(&ValidatorKeyFile {
                validators: vec![record],
            })
            .expect("serialize isolated validator key");
            let key_path = data_dir.join(VALIDATOR_KEYS_FILE);
            std::fs::write(&key_path, isolated).expect("write isolated validator key");
            #[cfg(unix)]
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                .expect("protect isolated validator key");
        }

        let topology_file = root.join("topology.json");
        let (base_port, listeners, _rpc_listeners) = reserve_consensus_v2_test_ports(validator_count);
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: "postfiat-remote-proposer-test".to_string(),
            validators: validator_count as u32,
            base_port,
            rpc_base_port: Some(base_port.saturating_add(100)),
            hosts: Some(vec!["127.0.0.1".to_string(); validator_count]),
            output_file: topology_file.clone(),
            activation_height: 1,
            storage_activation_height: None,
        })
        .expect("write topology");
        let (_, validators) =
            live_consensus_v2_context(&data_dirs[0]).expect("consensus context");
        let proposer =
            postfiat_ordering_fast::leader_for_view(&validators.validator_ids(), 1, 0)
                .expect("height-1 leader");
        let proposer_index = validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let collector_index = (proposer_index + 1) % validator_count;
        let collector_dir = data_dirs[collector_index].clone();
        let batch_file = root.join("height-1.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: collector_dir.clone(),
            key_file: Some(data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "c".repeat(40)),
            amount: 1_000_000,
            batch_file: batch_file.clone(),
        })
        .expect("create remote-proposer batch");

        let handles = start_consensus_v2_test_servers(
            &data_dirs,
            &topology_file,
            &listeners,
            collector_index,
            "remote-proposer-h1",
        );
        let topology = read_topology_file(&topology_file).expect("read topology");
        let local_status = status(NodeOptions {
            data_dir: collector_dir.clone(),
        })
        .expect("collector status");
        transport_authenticated_peer_health(
            &collector_dir,
            &topology,
            &local_status,
            15_000,
        )
        .expect("authenticated peer health");
        let processed_dir = root.join("processed");
        let processed_batch = processed_dir.join(
            batch_file
                .file_name()
                .expect("remote proposer batch file name"),
        );
        let report =
            transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                data_dir: collector_dir.clone(),
                topology_file: topology_file.clone(),
                batch_kind: Some("transparent".to_string()),
                batch_file: batch_file.clone(),
                key_file: collector_dir.join(VALIDATOR_KEYS_FILE),
                proposal_key_file: Some(collector_dir.join(VALIDATOR_KEYS_FILE)),
                require_local_proposer: false,
                require_signed_proposal: true,
                allow_peer_failures: false,
                quorum_early_full_propagation: false,
                artifact_dir: root.join("height-1-artifacts"),
                block_height: Some(1),
                view: Some(0),
                timeout_certificate_file: None,
                timeout_ms: 15_000,
                send_retries: 2,
                retry_backoff_ms: 50,
                vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
                local_apply_before_certified_send: true,
                defer_certified_sends: false,
                commit_processed_dir: Some(processed_dir.clone()),
                required_parent: None,
            })
            .expect("route round to remote proposer");
        let send_summary = report
            .sends
            .iter()
            .map(|send| {
                (
                    send.to.as_str(),
                    send.verified,
                    send.ack.applied,
                    send.ack.already_applied,
                    send.ack.rejected_count,
                    send.ack.state.block_height,
                    send.ack.certificate_attached,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            report.round_ok,
            "local_apply={} all_votes={} all_sends={} send_failures={:?} sends={send_summary:?}",
            report.local_apply_verified,
            report.all_vote_requests_verified,
            report.all_sends_verified,
            report.send_failures,
        );
        assert_eq!(report.proposal_proposer, proposer);
        assert_eq!(
            report.proposal_signature_signer.as_deref(),
            Some(proposer.as_str())
        );
        assert!(
            processed_batch.exists(),
            "local certified apply must publish the processed commit marker"
        );
        assert!(
            batch_file.exists(),
            "synchronous fleet sends retain the outbox source until the round completes"
        );
        let processed_certificate = processed_dir.join("height-1.certificate.json");
        assert!(processed_certificate.exists());
        clear_transport_vote_stream_pool_for_test(&topology_file).expect("close persistent streams");
        for (index, handle) in handles.into_iter().enumerate() {
            let node_index = if index < collector_index {
                index
            } else {
                index + 1
            };
            let service = handle
                .join()
                .expect("validator service thread")
                .expect("validator service");
            assert_eq!(service.accepted_health_count, 1);
            assert_eq!(service.accepted_block_vote_count, 2);
            assert_eq!(service.accepted_batch_count, 1);
            assert_eq!(
                service.accepted_block_proposal_count,
                u64::from(node_index == proposer_index)
            );
            assert!(service.rejected.is_empty(), "{:?}", service.rejected);
        }
        let statuses = data_dirs
            .iter()
            .map(|data_dir| status(NodeOptions {
                data_dir: data_dir.clone(),
            }))
            .collect::<io::Result<Vec<_>>>()
            .expect("fleet statuses");
        assert!(statuses.iter().all(|status| status.block_height == 1));
        assert!(statuses.windows(2).all(|pair| {
            pair[0].block_tip_hash == pair[1].block_tip_hash
                && pair[0].state_root == pair[1].state_root
        }));
        for (index, data_dir) in data_dirs.iter().enumerate() {
            let keys: ValidatorKeyFile = read_transport_json_file(
                &data_dir.join(VALIDATOR_KEYS_FILE),
                "isolated validator key",
            )
            .expect("read isolated validator key");
            assert_eq!(keys.validators.len(), 1);
            assert_eq!(keys.validators[0].node_id, format!("validator-{index}"));
        }
        std::fs::remove_dir_all(root).expect("cleanup remote proposer test");
    }

    /// The 2026-10-23 height-1144 failure: the proposer's own topology entry
    /// is not loopback, and its own validator service cannot serve a certified
    /// send while the round holds the store lease. Here nothing answers at that
    /// address at all. The default round (no `--local-apply-before-certified-send`)
    /// must apply the block through the store it holds and send only to peers.
    #[test]
    fn default_peer_round_applies_locally_without_sending_to_its_own_validator() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("no self-delivery test").expect("prewarm");
        let fleet = consensus_v2_transport_fleet("certified-round-no-self-send", 4, None);
        let proposer =
            postfiat_ordering_fast::leader_for_view(&fleet.validators.validator_ids(), 1, 0)
                .expect("proposer");
        let proposer_index = fleet
            .validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let closed_port = TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .expect("pick a closed port")
            .port();
        let mut topology_json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&fleet.topology_file).expect("read topology"),
        )
        .expect("parse topology");
        let proposer_peer = topology_json["peers"]
            .as_array_mut()
            .expect("topology peers")
            .iter_mut()
            .find(|peer| peer["node_id"] == proposer.as_str())
            .expect("proposer topology entry");
        proposer_peer["host"] = serde_json::json!("0.0.0.0");
        proposer_peer["p2p_port"] = serde_json::json!(closed_port);
        std::fs::write(
            &fleet.topology_file,
            serde_json::to_vec_pretty(&topology_json).expect("serialize topology"),
        )
        .expect("write topology");

        let proposer_dir = fleet.data_dirs[proposer_index].clone();
        let batch = fleet.root.join("height-1.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: proposer_dir.clone(),
            key_file: Some(fleet.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "e".repeat(40)),
            amount: 1_000_000,
            batch_file: batch.clone(),
        })
        .expect("transfer batch");
        let handles = start_consensus_v2_test_servers(
            &fleet.data_dirs,
            &fleet.topology_file,
            &fleet.listeners,
            proposer_index,
            "no-self-send-h1",
        );
        let topology = read_topology_file(&fleet.topology_file).expect("read topology");
        let local_status = status(NodeOptions {
            data_dir: proposer_dir.clone(),
        })
        .expect("proposer status");
        transport_authenticated_peer_health(&proposer_dir, &topology, &local_status, 60_000)
            .expect("authenticated peer health");
        let report =
            transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                data_dir: proposer_dir.clone(),
                topology_file: fleet.topology_file.clone(),
                batch_kind: Some("transparent".to_string()),
                batch_file: batch,
                key_file: proposer_dir.join(VALIDATOR_KEYS_FILE),
                proposal_key_file: Some(proposer_dir.join(VALIDATOR_KEYS_FILE)),
                require_local_proposer: true,
                require_signed_proposal: true,
                allow_peer_failures: false,
                quorum_early_full_propagation: false,
                artifact_dir: fleet.root.join("height-1-artifacts"),
                block_height: Some(1),
                view: Some(0),
                timeout_certificate_file: None,
                timeout_ms: 15_000,
                send_retries: 2,
                retry_backoff_ms: 50,
                vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
                local_apply_before_certified_send: false,
                defer_certified_sends: false,
                commit_processed_dir: None,
                required_parent: None,
            })
            .expect("default round must not depend on a send to its own validator");
        clear_transport_vote_stream_pool_for_test(&fleet.topology_file).expect("close persistent streams");
        for handle in handles {
            let service = handle
                .join()
                .expect("validator service thread")
                .expect("validator service");
            assert_eq!(service.accepted_batch_count, 1);
            assert!(service.rejected.is_empty(), "{:?}", service.rejected);
        }

        assert!(
            report.round_ok,
            "cert_ok={} votes={} validators={} remote={} all_votes={} vote_failures={:?} all_sends={} local_apply={} send_failures={:?}",
            report.certification.round_ok,
            report.certification.vote_count,
            report.certification.validators.len(),
            report.remote_vote_count,
            report.all_vote_requests_verified,
            report.vote_request_failures,
            report.all_sends_verified,
            report.local_apply_verified,
            report.send_failures
        );
        assert!(report.local_apply_verified);
        assert!(report.send_failures.is_empty(), "{:?}", report.send_failures);
        assert_eq!(report.sends.len(), fleet.data_dirs.len() - 1);
        assert!(report.sends.iter().all(|send| send.to != proposer));
        assert!(report
            .timings
            .certified_send_targets
            .iter()
            .all(|target| target.target != proposer));
        let statuses = fleet
            .data_dirs
            .iter()
            .map(|data_dir| status(NodeOptions {
                data_dir: data_dir.clone(),
            }))
            .collect::<io::Result<Vec<_>>>()
            .expect("fleet statuses");
        assert_eq!(statuses[proposer_index].block_height, 1);
        assert!(statuses.windows(2).all(|pair| {
            pair[0].block_tip_hash == pair[1].block_tip_hash
                && pair[0].state_root == pair[1].state_root
        }));
        std::fs::remove_dir_all(&fleet.root).expect("cleanup no self-delivery fleet");
    }

    /// Forwards one validator's transport port and holds back every reply
    /// chunk, so that validator answers its vote request after the quorum.
    fn spawn_delayed_transport_proxy(
        upstream_port: u16,
        delay: Duration,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind delayed proxy");
        let port = listener.local_addr().expect("delayed proxy address").port();
        listener
            .set_nonblocking(true)
            .expect("nonblocking delayed proxy");
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                let client = match listener.accept() {
                    Ok((client, _)) => client,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                client.set_nonblocking(false).expect("blocking proxy client");
                let Ok(upstream) = TcpStream::connect(("127.0.0.1", upstream_port)) else {
                    continue;
                };
                let client_reader = client.try_clone().expect("clone proxy client");
                let upstream_writer = upstream.try_clone().expect("clone proxy upstream");
                std::thread::spawn(move || {
                    let _ = io::copy(&mut &client_reader, &mut &upstream_writer);
                    let _ = upstream_writer.shutdown(std::net::Shutdown::Write);
                });
                std::thread::spawn(move || {
                    let mut upstream = upstream;
                    let mut client = client;
                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        match upstream.read(&mut buffer) {
                            Ok(0) | Err(_) => break,
                            Ok(count) => {
                                std::thread::sleep(delay);
                                if client.write_all(&buffer[..count]).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    let _ = client.shutdown(std::net::Shutdown::Write);
                });
            }
        });
        port
    }

    /// Answers every request on one validator's address with the rejection a
    /// validator service sends when it refuses a vote (same schema and fields
    /// as `transport_validator_rejection`).
    fn spawn_refusing_transport_peer(
        node_id: String,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind refusing peer");
        let port = listener.local_addr().expect("refusing peer address").port();
        listener
            .set_nonblocking(true)
            .expect("nonblocking refusing peer");
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                let stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                stream.set_nonblocking(false).expect("blocking refusing stream");
                let mut request = String::new();
                if BufReader::new(stream.try_clone().expect("clone refusing stream"))
                    .read_line(&mut request)
                    .is_err()
                {
                    continue;
                }
                let rejection = serde_json::json!({
                    "schema": "postfiat-transport-validator-serve-rejection-v1",
                    "node_id": node_id,
                    "topology_id": "refusing-peer",
                    "connection_index": 1,
                    "kind": "block_vote_request",
                    "error": "transport block vote signing failed: conflicting block proposal vote already recorded",
                    "state": {
                        "schema": TRANSPORT_HELLO_SCHEMA,
                        "node_id": node_id,
                        "topology_id": "refusing-peer",
                        "chain_id": "postfiat-consensus-v2-transport-test",
                        "genesis_hash": "refusing-peer",
                        "protocol_version": 1,
                        "state_root": "refusing-peer",
                        "block_height": 0,
                        "block_tip_hash": "genesis"
                    }
                });
                let mut stream = stream;
                let _ = stream.write_all(format!("{rejection}\n").as_bytes());
            }
        });
        port
    }

    /// SCT-08 item 4: the proposer keeps one vote outcome per requested
    /// validator for the finalized height: accepted votes with their ids, a
    /// refusal with its reason class, a validator that never answers, and a
    /// vote that arrives after the quorum.
    #[test]
    fn peer_round_keeps_a_vote_outcome_record_for_every_requested_validator() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("vote record test").expect("prewarm");
        // Ten validators: a quorum of seven still forms with one refusing,
        // one silent and one late validator.
        let fleet = consensus_v2_transport_fleet("certified-round-vote-records", 10, None);
        let ids = fleet.validators.validator_ids();
        let proposer = postfiat_ordering_fast::leader_for_view(&ids, 1, 0).expect("proposer");
        let proposer_index = ids
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let silent_index = (proposer_index + 1) % ids.len();
        let late_index = (proposer_index + 2) % ids.len();
        let refusing_index = (proposer_index + 3) % ids.len();
        let silent = ids[silent_index].clone();
        let late = ids[late_index].clone();
        let refusing = ids[refusing_index].clone();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let refusing_port = spawn_refusing_transport_peer(refusing.clone(), stop.clone());
        let late_port = fleet.listeners[late_index]
            .local_addr()
            .expect("late validator port")
            .port();
        let proxy_port =
            spawn_delayed_transport_proxy(late_port, Duration::from_millis(1_500), stop.clone());
        let closed_port = TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .expect("pick a closed port")
            .port();
        let mut topology_json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&fleet.topology_file).expect("read topology"),
        )
        .expect("parse topology");
        for peer in topology_json["peers"]
            .as_array_mut()
            .expect("topology peers")
            .iter_mut()
        {
            if peer["node_id"] == silent.as_str() {
                peer["p2p_port"] = serde_json::json!(closed_port);
            } else if peer["node_id"] == late.as_str() {
                peer["p2p_port"] = serde_json::json!(proxy_port);
            } else if peer["node_id"] == refusing.as_str() {
                peer["p2p_port"] = serde_json::json!(refusing_port);
            }
        }
        std::fs::write(
            &fleet.topology_file,
            serde_json::to_vec_pretty(&topology_json).expect("serialize topology"),
        )
        .expect("write topology");

        let mut handles = Vec::new();
        for (index, data_dir) in fleet.data_dirs.iter().enumerate() {
            if index == proposer_index || index == silent_index || index == refusing_index {
                continue;
            }
            let data_dir = data_dir.clone();
            let topology_file = fleet.topology_file.clone();
            let listener = fleet.listeners[index]
                .try_clone()
                .expect("clone reserved listener");
            let stop = stop.clone();
            handles.push(std::thread::spawn(move || {
                crate::transport_runtime::transport_validator_serve_inner(
                    data_dir.clone(),
                    topology_file,
                    data_dir.join(VALIDATOR_KEYS_FILE),
                    data_dir.join("vote-record-transport-votes"),
                    Some("127.0.0.1".to_string()),
                    16,
                    15_000,
                    None,
                    true,
                    Some(TransportShieldedVerifierPrewarmReport {
                        schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
                        requested: true,
                        total_ms: 0.0,
                        asset_orchard_swap_verifier_warm: true,
                        asset_orchard_swap_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_warm: true,
                        asset_orchard_private_egress_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_breakdown: None,
                    }),
                    Some(listener),
                    &stop,
                )
            }));
        }

        let proposer_dir = fleet.data_dirs[proposer_index].clone();
        let batch = fleet.root.join("height-1.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: proposer_dir.clone(),
            key_file: Some(fleet.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "f".repeat(40)),
            amount: 1_000_000,
            batch_file: batch.clone(),
        })
        .expect("transfer batch");
        let report =
            transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                data_dir: proposer_dir.clone(),
                topology_file: fleet.topology_file.clone(),
                batch_kind: Some("transparent".to_string()),
                batch_file: batch,
                key_file: proposer_dir.join(VALIDATOR_KEYS_FILE),
                proposal_key_file: Some(proposer_dir.join(VALIDATOR_KEYS_FILE)),
                require_local_proposer: true,
                require_signed_proposal: true,
                allow_peer_failures: true,
                quorum_early_full_propagation: false,
                artifact_dir: fleet.root.join("height-1-artifacts"),
                block_height: Some(1),
                view: Some(0),
                timeout_certificate_file: None,
                timeout_ms: 15_000,
                send_retries: 1,
                retry_backoff_ms: 50,
                vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
                local_apply_before_certified_send: false,
                defer_certified_sends: false,
                commit_processed_dir: None,
                required_parent: None,
            })
            .expect("round with one refusing, one silent and one late validator");
        assert!(report.round_ok, "send_failures={:?}", report.send_failures);
        crate::transport_runtime::wait_for_pending_block_vote_records();
        clear_transport_vote_stream_pool_for_test(&fleet.topology_file).expect("close persistent streams");
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        for handle in handles {
            let _ = handle.join();
        }

        let certificate: BlockCertificateFile = read_transport_json_file(
            Path::new(&report.certification.certificate_file),
            "round certificate",
        )
        .expect("read certificate");
        let proposer_status = status(NodeOptions {
            data_dir: proposer_dir.clone(),
        })
        .expect("proposer status");
        let record = postfiat_node::block_vote_records::read_block_vote_record(&proposer_dir, 1)
            .expect("read vote record")
            .expect("the finalized height has a vote record");
        assert_eq!(record.block_hash, proposer_status.block_tip_hash);
        assert_eq!(record.certificate_id, report.certification.certificate_id);
        assert_eq!(record.proposer, proposer);
        assert_eq!(record.round_deadline_ms, 15_000);
        assert_eq!(
            record
                .entries
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>(),
            ids
        );
        use postfiat_node::block_vote_records::BlockVoteOutcome;
        for entry in &record.entries {
            if entry.validator == silent {
                assert_eq!(entry.outcome, BlockVoteOutcome::NoAnswer, "{entry:?}");
                assert!(entry.vote_id.is_none());
                continue;
            }
            if entry.validator == refusing {
                assert_eq!(entry.outcome, BlockVoteOutcome::Refused, "{entry:?}");
                assert_eq!(
                    entry.refusal_class.as_deref(),
                    Some("conflicting_block_proposal_vote_already_recorded")
                );
                assert!(entry.vote_id.is_none());
                continue;
            }
            assert_eq!(entry.outcome, BlockVoteOutcome::Accepted, "{entry:?}");
            assert_eq!(entry.answered_after_quorum, entry.validator == late, "{entry:?}");
            let certificate_vote = certificate
                .certificate
                .votes
                .iter()
                .find(|vote| vote.validator == entry.validator);
            if entry.validator == late {
                assert!(
                    certificate_vote.is_none(),
                    "a vote after the quorum is not in the certificate"
                );
                assert!(entry.vote_id.as_deref().is_some_and(|id| !id.is_empty()));
            } else {
                assert_eq!(
                    entry.vote_id.as_deref(),
                    certificate_vote.map(|vote| vote.vote_id.as_str()),
                    "{entry:?}"
                );
            }
        }
        std::fs::remove_dir_all(&fleet.root).expect("cleanup vote record fleet");
    }

    /// Accepts connections on one validator's address and never answers, so
    /// every vote request and certified send to it runs into its timeout.
    fn spawn_hanging_transport_peer(stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind hanging peer");
        let port = listener.local_addr().expect("hanging peer address").port();
        listener
            .set_nonblocking(true)
            .expect("nonblocking hanging peer");
        std::thread::spawn(move || {
            let mut held = Vec::new();
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                match listener.accept() {
                    Ok((client, _)) => held.push(client),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        port
    }

    /// VR-01: a vote that arrives after the quorum but inside the round
    /// deadline is recorded as accepted with its vote id and
    /// `answered_after_quorum`, even when the round reaches the record writer
    /// only after the deadline (here: a validator that never answers holds the
    /// round's certified send to its timeout).
    #[test]
    fn peer_round_records_a_vote_after_the_quorum_when_the_writer_runs_at_the_deadline() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("vote record deadline test").expect("prewarm");
        // Seven validators: a quorum of five forms from the proposer and four
        // prompt validators, before the late one and without the hanging one.
        let fleet = consensus_v2_transport_fleet("certified-round-vote-record-deadline", 7, None);
        let ids = fleet.validators.validator_ids();
        let proposer = postfiat_ordering_fast::leader_for_view(&ids, 1, 0).expect("proposer");
        let proposer_index = ids
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let hanging_index = (proposer_index + 1) % ids.len();
        let late_index = (proposer_index + 2) % ids.len();
        let hanging = ids[hanging_index].clone();
        let late = ids[late_index].clone();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let hanging_port = spawn_hanging_transport_peer(stop.clone());
        let late_port = fleet.listeners[late_index]
            .local_addr()
            .expect("late validator port")
            .port();
        let proxy_port =
            spawn_delayed_transport_proxy(late_port, Duration::from_millis(1_000), stop.clone());
        let mut topology_json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&fleet.topology_file).expect("read topology"),
        )
        .expect("parse topology");
        for peer in topology_json["peers"]
            .as_array_mut()
            .expect("topology peers")
            .iter_mut()
        {
            if peer["node_id"] == hanging.as_str() {
                peer["p2p_port"] = serde_json::json!(hanging_port);
            } else if peer["node_id"] == late.as_str() {
                peer["p2p_port"] = serde_json::json!(proxy_port);
            }
        }
        std::fs::write(
            &fleet.topology_file,
            serde_json::to_vec_pretty(&topology_json).expect("serialize topology"),
        )
        .expect("write topology");

        let mut handles = Vec::new();
        for (index, data_dir) in fleet.data_dirs.iter().enumerate() {
            if index == proposer_index || index == hanging_index {
                continue;
            }
            let data_dir = data_dir.clone();
            let topology_file = fleet.topology_file.clone();
            let listener = fleet.listeners[index]
                .try_clone()
                .expect("clone reserved listener");
            let stop = stop.clone();
            handles.push(std::thread::spawn(move || {
                crate::transport_runtime::transport_validator_serve_inner(
                    data_dir.clone(),
                    topology_file,
                    data_dir.join(VALIDATOR_KEYS_FILE),
                    data_dir.join("vote-record-transport-votes"),
                    Some("127.0.0.1".to_string()),
                    16,
                    15_000,
                    None,
                    true,
                    Some(TransportShieldedVerifierPrewarmReport {
                        schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
                        requested: true,
                        total_ms: 0.0,
                        asset_orchard_swap_verifier_warm: true,
                        asset_orchard_swap_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_warm: true,
                        asset_orchard_private_egress_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_breakdown: None,
                    }),
                    Some(listener),
                    &stop,
                )
            }));
        }

        let proposer_dir = fleet.data_dirs[proposer_index].clone();
        let batch = fleet.root.join("height-1.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: proposer_dir.clone(),
            key_file: Some(fleet.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "f".repeat(40)),
            amount: 1_000_000,
            batch_file: batch.clone(),
        })
        .expect("transfer batch");
        let deadline_ms = 4_000;
        let round_start = std::time::Instant::now();
        let report =
            transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                data_dir: proposer_dir.clone(),
                topology_file: fleet.topology_file.clone(),
                batch_kind: Some("transparent".to_string()),
                batch_file: batch,
                key_file: proposer_dir.join(VALIDATOR_KEYS_FILE),
                proposal_key_file: Some(proposer_dir.join(VALIDATOR_KEYS_FILE)),
                require_local_proposer: true,
                require_signed_proposal: true,
                allow_peer_failures: true,
                // Full propagation sends the block to the hanging validator too, as
                // the live faucet rounds do; that send holds the round to its timeout.
                quorum_early_full_propagation: true,
                artifact_dir: fleet.root.join("height-1-artifacts"),
                block_height: Some(1),
                view: Some(0),
                timeout_certificate_file: None,
                timeout_ms: deadline_ms,
                send_retries: 1,
                retry_backoff_ms: 50,
                vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
                local_apply_before_certified_send: false,
                defer_certified_sends: false,
                commit_processed_dir: None,
                required_parent: None,
            })
            .expect("round with one hanging and one late validator");
        // Only the send to the validator that never answers fails.
        assert!(
            report.send_failures.iter().all(|failure| failure.to == hanging),
            "send_failures={:?}",
            report.send_failures
        );
        let round_elapsed = round_start.elapsed();
        crate::transport_runtime::wait_for_pending_block_vote_records();
        clear_transport_vote_stream_pool_for_test(&fleet.topology_file).expect("close persistent streams");
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        for handle in handles {
            let _ = handle.join();
        }
        // The scenario: the round reached the writer only after the deadline.
        assert!(
            round_elapsed >= Duration::from_millis(deadline_ms),
            "the round finished before the deadline ({round_elapsed:?}), so the writer did not run at it"
        );

        let record = postfiat_node::block_vote_records::read_block_vote_record(&proposer_dir, 1)
            .expect("read vote record")
            .expect("the finalized height has a vote record");
        assert_eq!(record.round_deadline_ms, deadline_ms);
        let entry = |validator: &str| {
            record
                .entries
                .iter()
                .find(|entry| entry.validator == validator)
                .cloned()
                .expect("an entry per requested validator")
        };
        let late_entry = entry(&late);
        assert_eq!(
            serde_json::to_value(late_entry.outcome).expect("outcome"),
            serde_json::json!("accepted"),
            "a vote after the quorum and inside the deadline: {late_entry:?}"
        );
        assert!(late_entry.answered_after_quorum, "{late_entry:?}");
        assert!(late_entry.vote_id.as_deref().is_some_and(|id| !id.is_empty()));
        let hanging_entry = entry(&hanging);
        assert_eq!(
            serde_json::to_value(hanging_entry.outcome).expect("outcome"),
            serde_json::json!("no_answer"),
            "a validator that never answered: {hanging_entry:?}"
        );
        assert!(hanging_entry.vote_id.is_none());
        for validator in ids.iter().filter(|v| **v != late && **v != hanging) {
            let entry = entry(validator);
            assert_eq!(
                serde_json::to_value(entry.outcome).expect("outcome"),
                serde_json::json!("accepted"),
                "{entry:?}"
            );
        }
        std::fs::remove_dir_all(&fleet.root).expect("cleanup vote record deadline fleet");
    }

    /// VR-01: an answer the writer still sees but that arrived after the round
    /// deadline is recorded as `late_after_deadline` with its vote id; a
    /// validator with no answer at all stays `no_answer`.
    #[test]
    fn vote_record_marks_an_answer_after_the_deadline_late_after_deadline() {
        use crate::transport_runtime::{
            finish_round_block_vote_record, BlockVoteObservation, RoundBlockVoteRecordInput,
        };
        use postfiat_node::block_vote_records::BlockVoteOutcome;
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-vote-record-after-deadline-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&data_dir).expect("data dir");
        let observation = |outcome, vote_id: Option<&str>, duration_ms| BlockVoteObservation {
            outcome,
            vote_id: vote_id.map(str::to_string),
            refusal_class: None,
            duration_ms,
        };
        let observations = [
            ("validator-1", observation(BlockVoteOutcome::Accepted, Some("vote-1"), 900.0)),
            ("validator-2", observation(BlockVoteOutcome::Accepted, Some("vote-2"), 1_100.0)),
            ("validator-3", observation(BlockVoteOutcome::Accepted, Some("vote-3"), 2_500.0)),
            ("validator-4", observation(BlockVoteOutcome::Accepted, Some("vote-4"), 4_600.0)),
        ]
        .into_iter()
        .map(|(validator, observation)| (validator.to_string(), observation))
        .collect();
        let written = finish_round_block_vote_record(RoundBlockVoteRecordInput {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-vote-record-test".to_string(),
            block_height: 7,
            view: 0,
            block_hash: "b".repeat(96),
            certificate_id: "c".repeat(96),
            proposer: "validator-0".to_string(),
            proposer_vote_id: "vote-0".to_string(),
            quorum: 4,
            required_remote_vote_count: 3,
            requested_peers: (1..=5).map(|index| format!("validator-{index}")).collect(),
            round_deadline_ms: 4_000,
            deadline: std::time::Instant::now(),
            observations,
            late_vote_receiver: None,
        });
        assert_eq!(written, "written");
        let record = postfiat_node::block_vote_records::read_block_vote_record(&data_dir, 7)
            .expect("read record")
            .expect("record written");
        let outcome = |validator: &str| {
            let entry = record
                .entries
                .iter()
                .find(|entry| entry.validator == validator)
                .expect("entry");
            (
                serde_json::to_value(entry.outcome).expect("outcome"),
                entry.vote_id.clone(),
                entry.answered_after_quorum,
            )
        };
        assert_eq!(outcome("validator-2"), (serde_json::json!("accepted"), Some("vote-2".to_string()), false));
        assert_eq!(outcome("validator-3"), (serde_json::json!("accepted"), Some("vote-3".to_string()), false));
        assert_eq!(
            outcome("validator-4"),
            (serde_json::json!("late_after_deadline"), Some("vote-4".to_string()), true),
            "an answer after the deadline that the writer still sees"
        );
        assert_eq!(outcome("validator-5"), (serde_json::json!("no_answer"), None, false));
        std::fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    /// A refusal the writer received after the round deadline is an answer:
    /// it stays `refused` with its class (only a validator with no answer
    /// received is `no_answer`, docs/review/vote-records-vr01-20261027.md).
    #[test]
    fn vote_record_keeps_a_refusal_after_the_deadline_refused() {
        use crate::transport_runtime::{
            finish_round_block_vote_record, BlockVoteObservation, RoundBlockVoteRecordInput,
        };
        use postfiat_node::block_vote_records::BlockVoteOutcome;
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-vote-record-refusal-after-deadline-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&data_dir).expect("data dir");
        let observation = |outcome, vote_id: Option<&str>, refusal: Option<&str>, duration_ms| {
            BlockVoteObservation {
                outcome,
                vote_id: vote_id.map(str::to_string),
                refusal_class: refusal.map(str::to_string),
                duration_ms,
            }
        };
        let observations = [
            ("validator-1", observation(BlockVoteOutcome::Accepted, Some("vote-1"), None, 900.0)),
            ("validator-2", observation(BlockVoteOutcome::Accepted, Some("vote-2"), None, 1_100.0)),
            ("validator-3", observation(BlockVoteOutcome::Accepted, Some("vote-3"), None, 2_500.0)),
            (
                "validator-4",
                observation(BlockVoteOutcome::Refused, None, Some("block_view_is_stale"), 4_600.0),
            ),
            (
                "validator-5",
                observation(BlockVoteOutcome::Refused, None, Some("block_view_is_stale"), 3_200.0),
            ),
        ]
        .into_iter()
        .map(|(validator, observation)| (validator.to_string(), observation))
        .collect();
        let written = finish_round_block_vote_record(RoundBlockVoteRecordInput {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-vote-record-test".to_string(),
            block_height: 8,
            view: 0,
            block_hash: "b".repeat(96),
            certificate_id: "c".repeat(96),
            proposer: "validator-0".to_string(),
            proposer_vote_id: "vote-0".to_string(),
            quorum: 4,
            required_remote_vote_count: 3,
            requested_peers: (1..=6).map(|index| format!("validator-{index}")).collect(),
            round_deadline_ms: 4_000,
            deadline: std::time::Instant::now(),
            observations,
            late_vote_receiver: None,
        });
        assert_eq!(written, "written");
        let record = postfiat_node::block_vote_records::read_block_vote_record(&data_dir, 8)
            .expect("read record")
            .expect("record written");
        let outcome = |validator: &str| {
            let entry = record
                .entries
                .iter()
                .find(|entry| entry.validator == validator)
                .expect("entry");
            (
                serde_json::to_value(entry.outcome).expect("outcome"),
                entry.refusal_class.clone(),
                entry.answered_after_quorum,
            )
        };
        assert_eq!(
            outcome("validator-5"),
            (serde_json::json!("refused"), Some("block_view_is_stale".to_string()), true),
            "a refusal inside the deadline"
        );
        assert_eq!(
            outcome("validator-4"),
            (serde_json::json!("refused"), Some("block_view_is_stale".to_string()), true),
            "a refusal after the deadline that the writer still sees"
        );
        assert_eq!(outcome("validator-6"), (serde_json::json!("no_answer"), None, false));
        std::fs::remove_dir_all(&data_dir).expect("cleanup");
    }

    /// Holds the late validator's prepare replies until the test opens it, or
    /// for at most a given time, and counts the replies passed on.
    #[derive(Default)]
    struct LateReplyGate {
        open: Mutex<bool>,
        opened: std::sync::Condvar,
        passed_on: std::sync::atomic::AtomicUsize,
    }

    impl LateReplyGate {
        fn open(&self) {
            *self
                .open
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
            self.opened.notify_all();
        }

        fn hold(&self, limit: Duration) {
            let open = self
                .open
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            drop(
                self.opened
                    .wait_timeout_while(open, limit, |open| !*open)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
            // Counted before the reply is written, so a round that saw the
            // reply always finds it counted.
            self.passed_on.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn passed_on(&self) -> usize {
            self.passed_on.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    /// Line proxy in front of one validator, modelling validator-2 at height
    /// 1453: its prepare vote reaches the proposer late, and every precommit
    /// request is held and then refused.
    fn spawn_late_refusing_validator_proxy(
        listener: TcpListener,
        upstream: std::net::SocketAddr,
        prepare_gate: std::sync::Arc<LateReplyGate>,
        prepare_hold: Duration,
        precommit_hold: Duration,
        precommit_requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        std::thread::spawn(move || {
            for client in listener.incoming() {
                let Ok(client) = client else { return };
                let precommit_requests = std::sync::Arc::clone(&precommit_requests);
                let prepare_gate = std::sync::Arc::clone(&prepare_gate);
                std::thread::spawn(move || {
                    let mut client_writer = client.try_clone().expect("clone proxy client");
                    let mut client_reader = BufReader::new(client);
                    let mut upstream_stream: Option<(TcpStream, BufReader<TcpStream>)> = None;
                    loop {
                        let mut line = String::new();
                        if client_reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        let request: serde_json::Value =
                            serde_json::from_str(&line).unwrap_or(serde_json::Value::Null);
                        let phase = request
                            .pointer("/consensus_v2/phase")
                            .and_then(serde_json::Value::as_str);
                        if phase == Some("precommit") {
                            precommit_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            std::thread::sleep(precommit_hold);
                            let refusal = serde_json::json!({
                                "schema": "postfiat-transport-validator-rejection-v1",
                                "error": "transport consensus v2 precommit signing failed: consensus v2 precommit vote is not authorized by a newer non-nil prepare QC",
                            });
                            let _ = client_writer.write_all(format!("{refusal}\n").as_bytes());
                            return;
                        }
                        if upstream_stream.is_none() {
                            let stream = TcpStream::connect(upstream).expect("proxy upstream");
                            let reader =
                                BufReader::new(stream.try_clone().expect("clone upstream"));
                            upstream_stream = Some((stream, reader));
                        }
                        let (stream, reader) = upstream_stream.as_mut().expect("upstream");
                        if stream.write_all(line.as_bytes()).is_err() {
                            return;
                        }
                        let mut response = String::new();
                        if reader.read_line(&mut response).unwrap_or(0) == 0 {
                            return;
                        }
                        if phase == Some("prepare") {
                            prepare_gate.hold(prepare_hold);
                        }
                        if client_writer.write_all(response.as_bytes()).is_err() {
                            return;
                        }
                    }
                });
            }
        });
    }

    fn certificate_qc_voters(certificate_file: &Path, qc: &str) -> Vec<String> {
        fn find<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
            match value {
                serde_json::Value::Object(map) => map
                    .get(key)
                    .or_else(|| map.values().find_map(|value| find(value, key))),
                serde_json::Value::Array(items) => items.iter().find_map(|value| find(value, key)),
                _ => None,
            }
        }
        let certificate: serde_json::Value = serde_json::from_slice(
            &std::fs::read(certificate_file).expect("read block certificate"),
        )
        .expect("parse block certificate");
        find(&certificate, qc)
            .and_then(|qc| qc.get("votes"))
            .and_then(serde_json::Value::as_array)
            .expect("QC votes")
            .iter()
            .map(|vote| {
                vote.get("validator")
                    .and_then(serde_json::Value::as_str)
                    .expect("QC vote validator")
                    .to_string()
            })
            .collect()
    }

    #[derive(Clone, Copy)]
    enum ProxyVote {
        /// Forward to the real validator and hold its reply this long.
        Forward(Duration),
        /// Forward a tampered request so the real validator sends an explicit
        /// rejection, then close like the validator does.
        Refuse,
        /// Refuse the first request of the phase like `Refuse` (a peer that is
        /// not ready yet), then forward the requests that follow.
        RefuseFirst,
        /// Refuse like `Refuse` until this long after the phase's first
        /// request (a peer lagging that long), then forward.
        RefuseFor(Duration),
        /// Answer with the validator's permanent refusal: it already voted for
        /// a conflicting proposal at this height.
        RefuseConflicting,
        /// Hold the request this long before the validator sees it (a peer
        /// still busy with the phase), then forward it.
        Hold(Duration),
        /// Forward, then drop the consensus v2 vote from the validator's reply.
        OmitV2Vote,
        /// Forward, then corrupt the consensus v2 vote signature in the reply
        /// (which the transport accepts and the prepare QC would not) and hold
        /// it this long.
        CorruptV2Signature(Duration),
        /// Read the request and never answer.
        Silent,
        /// Hold the request this long, then close without a reply.
        Drop(Duration),
    }

    /// Line proxy in front of one validator, acting per vote phase and
    /// counting vote requests per phase ([prepare, precommit]).
    fn spawn_vote_phase_proxy(
        listener: TcpListener,
        upstream: std::net::SocketAddr,
        prepare: ProxyVote,
        precommit: ProxyVote,
        requests: std::sync::Arc<[std::sync::atomic::AtomicUsize; 2]>,
    ) {
        std::thread::spawn(move || {
            let first_requests = std::sync::Arc::new([
                std::sync::OnceLock::<Instant>::new(),
                std::sync::OnceLock::<Instant>::new(),
            ]);
            for client in listener.incoming() {
                let Ok(client) = client else { return };
                let requests = std::sync::Arc::clone(&requests);
                let first_requests = std::sync::Arc::clone(&first_requests);
                std::thread::spawn(move || {
                    let mut client_writer = client.try_clone().expect("clone proxy client");
                    let mut client_reader = BufReader::new(client);
                    let mut upstream_stream: Option<(TcpStream, BufReader<TcpStream>)> = None;
                    loop {
                        let mut line = String::new();
                        if client_reader.read_line(&mut line).unwrap_or(0) == 0 {
                            return;
                        }
                        let mut request: serde_json::Value =
                            serde_json::from_str(&line).unwrap_or(serde_json::Value::Null);
                        let (phase, configured) = match request
                            .pointer("/consensus_v2/phase")
                            .and_then(serde_json::Value::as_str)
                        {
                            Some("prepare") => (Some(0), prepare),
                            Some("precommit") => (Some(1), precommit),
                            _ => (None, ProxyVote::Forward(Duration::ZERO)),
                        };
                        let earlier = phase.map(|phase| {
                            requests[phase].fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                        });
                        let since_first = phase.map(|phase| {
                            first_requests[phase].get_or_init(Instant::now).elapsed()
                        });
                        let action = match configured {
                            ProxyVote::RefuseFirst if earlier == Some(0) => ProxyVote::Refuse,
                            ProxyVote::RefuseFirst => ProxyVote::Forward(Duration::ZERO),
                            ProxyVote::RefuseFor(lag) if since_first.is_some_and(|since| since < lag) => {
                                ProxyVote::Refuse
                            }
                            ProxyVote::RefuseFor(_) => ProxyVote::Forward(Duration::ZERO),
                            ProxyVote::Hold(hold) => {
                                std::thread::sleep(hold);
                                ProxyVote::Forward(Duration::ZERO)
                            }
                            other => other,
                        };
                        let delay = match action {
                            ProxyVote::Forward(delay) => delay,
                            ProxyVote::OmitV2Vote => Duration::ZERO,
                            ProxyVote::CorruptV2Signature(delay) => delay,
                            ProxyVote::Silent => {
                                std::thread::sleep(Duration::from_secs(120));
                                return;
                            }
                            ProxyVote::Drop(hold) => {
                                std::thread::sleep(hold);
                                return;
                            }
                            ProxyVote::Refuse | ProxyVote::RefuseConflicting => {
                                request["block_height"] = serde_json::json!(1_000_000u64);
                                line = format!("{request}\n");
                                upstream_stream = None;
                                Duration::ZERO
                            }
                            ProxyVote::RefuseFirst | ProxyVote::RefuseFor(_) | ProxyVote::Hold(_) => {
                                unreachable!()
                            }
                        };
                        if upstream_stream.is_none() {
                            let Ok(stream) = TcpStream::connect(upstream) else {
                                return;
                            };
                            let reader =
                                BufReader::new(stream.try_clone().expect("clone upstream"));
                            upstream_stream = Some((stream, reader));
                        }
                        let (stream, reader) = upstream_stream.as_mut().expect("upstream");
                        if stream.write_all(line.as_bytes()).is_err() {
                            return;
                        }
                        let mut response = String::new();
                        if reader.read_line(&mut response).unwrap_or(0) == 0 {
                            return;
                        }
                        if matches!(action, ProxyVote::OmitV2Vote) {
                            let mut reply: serde_json::Value =
                                serde_json::from_str(&response).expect("vote reply json");
                            if let Some(reply) = reply.as_object_mut() {
                                reply.remove("consensus_v2_vote");
                            }
                            response = format!("{reply}\n");
                        }
                        if matches!(action, ProxyVote::CorruptV2Signature(_)) {
                            let mut reply: serde_json::Value =
                                serde_json::from_str(&response).expect("vote reply json");
                            if let Some(signature) = reply
                                .pointer_mut("/consensus_v2_vote/signature/signature_hex")
                            {
                                let hex = signature.as_str().expect("signature hex");
                                let flipped = if hex.starts_with('0') { "1" } else { "0" };
                                *signature = serde_json::json!(format!("{flipped}{}", &hex[1..]));
                            }
                            response = format!("{reply}\n");
                        }
                        if matches!(action, ProxyVote::RefuseConflicting) {
                            let mut reply: serde_json::Value =
                                serde_json::from_str(&response).expect("rejection json");
                            reply["error"] = serde_json::json!(
                                "transport block vote signing failed: conflicting block proposal vote already recorded"
                            );
                            response = format!("{reply}\n");
                        }
                        std::thread::sleep(delay);
                        if client_writer.write_all(response.as_bytes()).is_err() {
                            return;
                        }
                        if matches!(action, ProxyVote::Refuse | ProxyVote::RefuseConflicting) {
                            return;
                        }
                    }
                });
            }
        });
    }

    struct VotePhaseRoundFixture {
        data_dirs: Vec<PathBuf>,
        topology_file: PathBuf,
        proposer_index: usize,
        quorum: usize,
        artifact_dir: PathBuf,
        /// Vote requests seen by each proxied validator, by validator index.
        proxied: Vec<(usize, std::sync::Arc<[std::sync::atomic::AtomicUsize; 2]>)>,
        /// The proposer's reserved transport port; its service is not started.
        proposer_listener: TcpListener,
    }

    impl VotePhaseRoundFixture {
        fn requests(&self, index: usize, phase: usize) -> usize {
            self.proxied
                .iter()
                .find(|(proxied, _)| *proxied == index)
                .expect("proxied validator")
                .1[phase]
                .load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    fn vote_phase_round_fixture(
        tag: &str,
        proxies: &[(usize, ProxyVote, ProxyVote)],
    ) -> VotePhaseRoundFixture {
        vote_phase_round_fixture_of(tag, 6, proxies)
    }

    /// `validator_count` consensus v2 validators (active from height 1) at
    /// height 0 with live transport services; `proxies` puts a phase proxy in
    /// front of the validators that follow the height-1 proposer by the given
    /// offsets.
    fn vote_phase_round_fixture_of(
        tag: &str,
        validator_count: usize,
        proxies: &[(usize, ProxyVote, ProxyVote)],
    ) -> VotePhaseRoundFixture {
        prewarm_shielded_verifier_cache(tag).expect("prewarm verifier");
        let chain_id = format!("postfiat-{tag}-test");
        let root = std::env::temp_dir().join(format!(
            "postfiat-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let data_dirs = (0..validator_count)
            .map(|index| {
                let data_dir = root.join(format!("validator-{index}"));
                init_consensus_v2(InitConsensusV2Options {
                    data_dir: data_dir.clone(),
                    chain_id: chain_id.clone(),
                    node_id: format!("validator-{index}"),
                    validator_count: validator_count as u32,
                    activation_height: 1,
                    storage_activation_height: None,
                })
                .expect("init validator");
                data_dir
            })
            .collect::<Vec<_>>();
        let shared_keys: ValidatorKeyFile = serde_json::from_slice(
            &std::fs::read(data_dirs[0].join(VALIDATOR_KEYS_FILE)).expect("read keys"),
        )
        .expect("parse keys");
        let bootstrap_snapshot = root.join("bootstrap-snapshot");
        export_snapshot(SnapshotExportOptions {
            data_dir: data_dirs[0].clone(),
            snapshot_dir: bootstrap_snapshot.clone(),
        })
        .expect("export common state");
        for (index, data_dir) in data_dirs.iter().enumerate().skip(1) {
            std::fs::remove_dir_all(data_dir).expect("clear initialized replica");
            import_snapshot(SnapshotImportOptions {
                data_dir: data_dir.clone(),
                snapshot_dir: bootstrap_snapshot.clone(),
                node_id: Some(format!("validator-{index}")),
            })
            .expect("import common state");
        }
        for (index, data_dir) in data_dirs.iter().enumerate() {
            let record = shared_keys
                .validators
                .iter()
                .find(|record| record.node_id == format!("validator-{index}"))
                .expect("local validator key")
                .clone();
            let key_path = data_dir.join(VALIDATOR_KEYS_FILE);
            std::fs::write(
                &key_path,
                serde_json::to_vec_pretty(&ValidatorKeyFile {
                    validators: vec![record],
                })
                .expect("serialize isolated key"),
            )
            .expect("write isolated key");
            #[cfg(unix)]
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                .expect("protect isolated key");
        }
        let topology_file = root.join("topology.json");
        let (base_port, mut listeners, _rpc_listeners) =
            reserve_consensus_v2_test_ports(validator_count);
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id,
            validators: validator_count as u32,
            base_port,
            rpc_base_port: Some(base_port.saturating_add(100)),
            hosts: Some(vec!["127.0.0.1".to_string(); validator_count]),
            output_file: topology_file.clone(),
            activation_height: 1,
            storage_activation_height: None,
        })
        .expect("write topology");
        let (_, validators) = live_consensus_v2_context(&data_dirs[0]).expect("context");
        let validator_ids = validators.validator_ids();
        let proposer =
            postfiat_ordering_fast::leader_for_view(&validator_ids, 1, 0).expect("height-1 leader");
        let proposer_index = validator_ids
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let mut proxied = Vec::new();
        for &(offset, prepare, precommit) in proxies {
            let index = (proposer_index + offset) % validator_count;
            let upstream = TcpListener::bind(("127.0.0.1", 0)).expect("proxied upstream port");
            let upstream_addr = upstream.local_addr().expect("proxied upstream addr");
            let topology_listener = std::mem::replace(&mut listeners[index], upstream);
            let requests = std::sync::Arc::new([
                std::sync::atomic::AtomicUsize::new(0),
                std::sync::atomic::AtomicUsize::new(0),
            ]);
            spawn_vote_phase_proxy(
                topology_listener,
                upstream_addr,
                prepare,
                precommit,
                std::sync::Arc::clone(&requests),
            );
            proxied.push((index, requests));
        }
        for (index, data_dir) in data_dirs.iter().enumerate() {
            if index == proposer_index {
                continue;
            }
            spawn_vote_phase_validator(
                data_dir.clone(),
                topology_file.clone(),
                listeners[index]
                    .try_clone()
                    .expect("clone reserved listener"),
            );
        }
        let proposer_listener = listeners[proposer_index]
            .try_clone()
            .expect("clone reserved proposer listener");
        let topology = read_topology_file(&topology_file).expect("read topology");
        let proposer_dir = data_dirs[proposer_index].clone();
        let local_status = status(NodeOptions {
            data_dir: proposer_dir.clone(),
        })
        .expect("proposer status");
        transport_authenticated_peer_health(&proposer_dir, &topology, &local_status, 15_000)
            .expect("authenticated peer health");
        VotePhaseRoundFixture {
            artifact_dir: root.join("height-1-artifacts"),
            data_dirs,
            topology_file,
            proposer_index,
            quorum: validators.quorum,
            proxied,
            proposer_listener,
        }
    }

    /// Transport validator service for one validator of the vote phase fixture.
    fn spawn_vote_phase_validator(data_dir: PathBuf, topology_file: PathBuf, listener: TcpListener) {
        std::thread::spawn(move || {
            let shutdown_requested = std::sync::atomic::AtomicBool::new(false);
            crate::transport_runtime::transport_validator_serve_inner(
                data_dir.clone(),
                topology_file,
                data_dir.join(VALIDATOR_KEYS_FILE),
                data_dir.join("vote-phase-transport-votes"),
                Some("127.0.0.1".to_string()),
                // A refused request closes its connection, and a refusal
                // is retried until the phase deadline.
                1024,
                60_000,
                None,
                true,
                Some(TransportShieldedVerifierPrewarmReport {
                    schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
                    requested: true,
                    total_ms: 0.0,
                    asset_orchard_swap_verifier_warm: true,
                    asset_orchard_swap_verifier_ms: Some(0.0),
                    asset_orchard_private_egress_verifier_warm: true,
                    asset_orchard_private_egress_verifier_ms: Some(0.0),
                    asset_orchard_private_egress_verifier_breakdown: None,
                }),
                Some(listener),
                &shutdown_requested,
            )
        });
    }

    type RoundHandle =
        std::thread::JoinHandle<Result<TransportPeerCertifiedBatchRoundReport, String>>;

    /// Runs the height-1 round under a stand-in for the RPC mempool mutation
    /// lock and waits up to `bound`. Returns the round's result, or `None`
    /// (round thread left detached) when it is still running at `bound`.
    fn run_vote_phase_round(
        fixture: &VotePhaseRoundFixture,
        timeout_ms: u64,
        send_retries: usize,
        retry_backoff_ms: u64,
        quorum_early: bool,
        bound: Duration,
    ) -> (
        Option<Result<TransportPeerCertifiedBatchRoundReport, String>>,
        Duration,
        bool,
    ) {
        run_vote_phase_round_with_sends(
            fixture,
            timeout_ms,
            send_retries,
            retry_backoff_ms,
            quorum_early,
            false,
            PEER_VOTE_PHASE_DEADLINE_MS,
            bound,
        )
    }

    /// `run_vote_phase_round`, choosing whether certified sends are deferred
    /// to durable jobs as on the RPC finality paths, and the phase deadline.
    #[allow(clippy::too_many_arguments)]
    fn run_vote_phase_round_with_sends(
        fixture: &VotePhaseRoundFixture,
        timeout_ms: u64,
        send_retries: usize,
        retry_backoff_ms: u64,
        quorum_early: bool,
        defer_certified_sends: bool,
        vote_phase_deadline_ms: u64,
        bound: Duration,
    ) -> (
        Option<Result<TransportPeerCertifiedBatchRoundReport, String>>,
        Duration,
        bool,
    ) {
        run_vote_phase_round_at(
            fixture,
            &RoundSlot {
                proposer_index: fixture.proposer_index,
                view: 0,
                timeout_certificate_file: None,
                artifact_dir: fixture.artifact_dir.clone(),
                batch_file: None,
            },
            timeout_ms,
            send_retries,
            retry_backoff_ms,
            quorum_early,
            defer_certified_sends,
            vote_phase_deadline_ms,
            bound,
        )
    }

    /// Who proposes the height-1 round, at which view, and where it writes.
    struct RoundSlot {
        proposer_index: usize,
        view: u64,
        timeout_certificate_file: Option<PathBuf>,
        artifact_dir: PathBuf,
        /// A batch to propose instead of a new transfer.
        batch_file: Option<PathBuf>,
    }

    #[allow(clippy::too_many_arguments)]
    fn run_vote_phase_round_at(
        fixture: &VotePhaseRoundFixture,
        slot: &RoundSlot,
        timeout_ms: u64,
        send_retries: usize,
        retry_backoff_ms: u64,
        quorum_early: bool,
        defer_certified_sends: bool,
        vote_phase_deadline_ms: u64,
        bound: Duration,
    ) -> (
        Option<Result<TransportPeerCertifiedBatchRoundReport, String>>,
        Duration,
        bool,
    ) {
        let proposer_dir = fixture.data_dirs[slot.proposer_index].clone();
        let batch_file = match &slot.batch_file {
            Some(batch_file) => batch_file.clone(),
            None => {
                let batch_file = slot.artifact_dir.with_extension("batch.json");
                create_transfer_batch(BatchTransferOptions {
                    data_dir: proposer_dir.clone(),
                    key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
                    to: format!("pf{}", "d".repeat(40)),
                    amount: 1_000_000,
                    batch_file: batch_file.clone(),
                })
                .expect("create batch");
                batch_file
            }
        };
        let options = TransportPeerCertifiedBatchRoundOptions {
            data_dir: proposer_dir.clone(),
            topology_file: fixture.topology_file.clone(),
            batch_kind: Some("transparent".to_string()),
            batch_file,
            key_file: proposer_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: Some(proposer_dir.join(VALIDATOR_KEYS_FILE)),
            require_local_proposer: true,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: quorum_early,
            artifact_dir: slot.artifact_dir.clone(),
            block_height: Some(1),
            view: Some(slot.view),
            timeout_certificate_file: slot.timeout_certificate_file.clone(),
            timeout_ms,
            send_retries,
            retry_backoff_ms,
            vote_phase_deadline_ms,
            local_apply_before_certified_send: true,
            defer_certified_sends,
            commit_processed_dir: None,
            required_parent: None,
        };
        let mempool_mutation_lock = std::sync::Arc::new(Mutex::new(()));
        let lock = std::sync::Arc::clone(&mempool_mutation_lock);
        let round_start = Instant::now();
        let round: RoundHandle = std::thread::spawn(move || {
            let _mutation_guard = lock.try_lock().expect("mempool mutation lock free");
            transport_peer_certified_batch_round(options)
        });
        while !round.is_finished() && round_start.elapsed() < bound {
            std::thread::sleep(Duration::from_millis(50));
        }
        let elapsed = round_start.elapsed();
        let lock_free = mempool_mutation_lock.try_lock().is_ok();
        let result = round
            .is_finished()
            .then(|| round.join().expect("round thread"));
        (result, elapsed, lock_free)
    }

    /// Repository units: --finality-timeout-ms 30000, 16 retries, 250 ms backoff.
    const UNIT_TIMEOUT_MS: u64 = 30_000;
    const UNIT_SEND_RETRIES: usize = 16;
    const UNIT_RETRY_BACKOFF_MS: u64 = 250;

    #[test]
    fn persistently_refused_prepare_votes_are_asked_until_the_phase_deadline() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture = vote_phase_round_fixture(
            "vote-refusal",
            &[
                (1, ProxyVote::Refuse, forward),
                (2, ProxyVote::Refuse, forward),
            ],
        );
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        let refusing = fixture
            .proxied
            .iter()
            .map(|(index, _)| fixture.requests(*index, 0))
            .collect::<Vec<_>>();
        eprintln!(
            "vote refusal: elapsed={elapsed:?} prepare_requests_to_refusing_peers={refusing:?}"
        );
        let error = result
            .expect("round still running")
            .expect_err("two refusals leave the round below quorum");
        assert!(lock_free);
        // A refusal may be a peer that is not ready yet, so it is asked again
        // past the retry budget until the phase deadline ends the round.
        assert!(
            refusing.iter().all(|requests| *requests > UNIT_SEND_RETRIES + 1),
            "a refused prepare vote was not asked past the retry budget: {refusing:?} {error}"
        );
        assert!(elapsed >= Duration::from_secs(19), "phase ended early: {elapsed:?}");
        assert!(!fixture.artifact_dir.join("block-certificate.json").exists());
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// Six validators, quorum 5, one silent: the round needs a lagging peer
    /// that refuses for longer than the retry budget lasts (about 5.5 s).
    #[test]
    fn a_peer_refusing_past_the_retry_budget_is_asked_until_the_deadline_and_the_round_certifies() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture = vote_phase_round_fixture(
            "vote-refuse-8s",
            &[
                (1, ProxyVote::RefuseFor(Duration::from_secs(8)), forward),
                (2, ProxyVote::Silent, forward),
            ],
        );
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        let lagging = fixture.requests(fixture.proxied[0].0, 0);
        eprintln!("refuse 8 s: elapsed={elapsed:?} prepare_requests_to_lagging_peer={lagging}");
        let round = result
            .expect("round still running")
            .expect("a peer that refuses for 8 s and then votes completes the quorum");
        assert!(lock_free);
        assert!(round.certification.round_ok);
        assert!(lagging > UNIT_SEND_RETRIES + 1, "{lagging}");
        assert!(elapsed >= Duration::from_secs(8) && elapsed < Duration::from_secs(19));
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// Two permanent refusals and one silent peer: four remote votes are
    /// needed and at most three remain possible.
    fn assert_round_ends_once_quorum_is_out_of_reach(tag: &str, phase: usize) {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        let failing = |vote: ProxyVote| {
            if phase == 0 {
                (vote, forward)
            } else {
                (forward, vote)
            }
        };
        let proxies = [
            ProxyVote::RefuseConflicting,
            ProxyVote::RefuseConflicting,
            ProxyVote::Silent,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, vote)| {
            let (prepare, precommit) = failing(vote);
            (index + 1, prepare, precommit)
        })
        .collect::<Vec<_>>();
        let fixture = vote_phase_round_fixture(tag, &proxies);
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        eprintln!("{tag}: elapsed={elapsed:?} lock_free={lock_free}");
        result
            .expect("round still running")
            .expect_err("two refusals and a silent peer leave the round below quorum");
        assert!(lock_free);
        assert!(
            elapsed < Duration::from_secs(10),
            "the phase waited {elapsed:?} for a quorum that could no longer form"
        );
        if phase == 0 {
            assert!(!fixture.artifact_dir.join("block-certificate.json").exists());
        }
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    #[test]
    fn a_prepare_phase_ends_once_its_quorum_is_out_of_reach() {
        assert_round_ends_once_quorum_is_out_of_reach("prepare-out-of-reach", 0);
    }

    #[test]
    fn a_precommit_phase_ends_once_its_quorum_is_out_of_reach() {
        assert_round_ends_once_quorum_is_out_of_reach("precommit-out-of-reach", 1);
    }

    #[test]
    fn lagging_peers_that_refuse_a_prepare_once_are_asked_again_and_the_round_certifies() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        // Six validators, quorum 5: the round needs one of the two lagging
        // peers, each of which refuses its first prepare request.
        let fixture = vote_phase_round_fixture(
            "vote-refuse-once",
            &[
                (1, ProxyVote::RefuseFirst, forward),
                (2, ProxyVote::RefuseFirst, forward),
            ],
        );
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        let lagging = fixture
            .proxied
            .iter()
            .map(|(index, _)| fixture.requests(*index, 0))
            .collect::<Vec<_>>();
        eprintln!("refuse once: elapsed={elapsed:?} prepare_requests_to_lagging_peers={lagging:?}");
        let round = result
            .expect("round still running")
            .expect("a peer that refused once and then voted completes the quorum");
        assert!(lock_free);
        assert!(round.certification.round_ok);
        assert!(lagging.iter().any(|requests| *requests > 1));
        assert!(fixture.artifact_dir.join("block-certificate.json").is_file());
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    #[test]
    fn a_peer_still_preparing_is_asked_for_its_precommit_once_its_prepare_vote_lands() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        // Prepare quorum forms without the busy peer (offset 2), but precommit
        // quorum needs it because offset 1 refuses every precommit.
        let fixture = vote_phase_round_fixture(
            "vote-precommit-busy",
            &[
                (1, forward, ProxyVote::Refuse),
                (2, ProxyVote::Hold(Duration::from_secs(4)), forward),
            ],
        );
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        let busy = fixture.proxied[1].0;
        eprintln!(
            "busy precommit: elapsed={elapsed:?} busy_requests=[{}, {}]",
            fixture.requests(busy, 0),
            fixture.requests(busy, 1)
        );
        let round = result
            .expect("round still running")
            .expect("the busy peer precommits once its prepare vote exists");
        assert!(lock_free);
        assert!(round.certification.round_ok);
        let certificate_file = fixture.artifact_dir.join("block-certificate.json");
        assert!(certificate_qc_voters(&certificate_file, "precommit_qc")
            .contains(&format!("validator-{busy}")));
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// The record of an early-quorum v2 round (`fixture` validators, proposer
    /// at `fixture.proposer_index`), after its late answers were collected.
    fn v2_round_vote_record(
        fixture: &VotePhaseRoundFixture,
    ) -> postfiat_node::block_vote_records::BlockVoteRecord {
        crate::transport_runtime::wait_for_pending_block_vote_records();
        postfiat_node::block_vote_records::read_block_vote_record(
            &fixture.data_dirs[fixture.proposer_index],
            1,
        )
        .expect("read vote record")
        .expect("the finalized height has a vote record")
    }

    /// Consensus v2 active at height 1 (the record tests above run before its
    /// activation): a prepare reply that arrives after the prepare quorum is
    /// relayed by the precommit collector to the vote record exactly once.
    #[test]
    fn v2_vote_record_keeps_a_late_prepare_reply_once_and_a_silent_peer_as_no_answer() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use postfiat_node::block_vote_records::BlockVoteOutcome;
        let forward = ProxyVote::Forward(Duration::ZERO);
        // Ten validators, quorum 7: the six fast peers make both quorums.
        let validator_count = 10;
        let fixture = vote_phase_round_fixture_of(
            "v2-vote-record-late",
            validator_count,
            &[
                (1, ProxyVote::Forward(Duration::from_secs(3)), forward),
                (2, ProxyVote::Silent, forward),
            ],
        );
        let (result, _, lock_free) = run_vote_phase_round_with_sends(
            &fixture,
            8_000,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            8_000,
            Duration::from_secs(30),
        );
        let round = result
            .expect("round still running")
            .expect("round certifies without the late and the silent peer");
        assert!(lock_free);
        assert!(round.certification.round_ok);
        let record = v2_round_vote_record(&fixture);
        let name = |offset: usize| {
            format!("validator-{}", (fixture.proposer_index + offset) % validator_count)
        };
        let validators = record
            .entries
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        assert_eq!(validators.len(), validator_count, "{validators:?}");
        assert_eq!(
            validators.iter().collect::<BTreeSet<_>>().len(),
            validator_count,
            "a validator is recorded twice: {validators:?}"
        );
        let entry = |validator: &str| {
            record
                .entries
                .iter()
                .find(|entry| entry.validator == validator)
                .expect("recorded validator")
                .clone()
        };
        let late = entry(&name(1));
        assert_eq!(late.outcome, BlockVoteOutcome::Accepted, "{late:?}");
        assert!(late.answered_after_quorum, "{late:?}");
        assert!(late.vote_id.is_some(), "{late:?}");
        let silent = entry(&name(2));
        assert_eq!(silent.outcome, BlockVoteOutcome::NoAnswer, "{silent:?}");
        assert!(silent.vote_id.is_none());
        let certificate_file = fixture.artifact_dir.join("block-certificate.json");
        for qc in ["prepare_qc", "precommit_qc"] {
            let voters = certificate_qc_voters(&certificate_file, qc);
            assert_eq!(
                voters.iter().collect::<BTreeSet<_>>().len(),
                voters.len(),
                "{qc} counts a validator twice: {voters:?}"
            );
            assert!(voters.len() >= fixture.quorum, "{qc}: {voters:?}");
            assert!(!voters.contains(&name(2)), "{qc}: {voters:?}");
        }
        assert!(!certificate_qc_voters(&certificate_file, "prepare_qc").contains(&name(1)));
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file)
            .expect("close persistent streams");
    }

    /// A prepare reply that arrives but fails the prepare QC checks (here a
    /// bad v2 signature) is recorded as a refused, invalid vote response,
    /// whether it arrives before or after the prepare quorum.
    #[test]
    fn v2_vote_record_keeps_a_rejected_prepare_reply_as_an_invalid_vote_response() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        use postfiat_node::block_vote_records::BlockVoteOutcome;
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture = vote_phase_round_fixture_of(
            "v2-vote-record-rejected",
            10,
            &[
                (1, ProxyVote::CorruptV2Signature(Duration::ZERO), forward),
                (3, ProxyVote::CorruptV2Signature(Duration::from_secs(3)), forward),
            ],
        );
        // No retries, so both rejected replies end well before the record
        // deadline.
        let (result, _, lock_free) = run_vote_phase_round_with_sends(
            &fixture,
            8_000,
            0,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            8_000,
            Duration::from_secs(30),
        );
        let round = result
            .expect("round still running")
            .expect("round certifies without the rejected replies");
        assert!(lock_free);
        assert!(round.certification.round_ok);
        let record = v2_round_vote_record(&fixture);
        let certificate_file = fixture.artifact_dir.join("block-certificate.json");
        for offset in [1, 3] {
            let validator = format!("validator-{}", (fixture.proposer_index + offset) % 10);
            let entry = record
                .entries
                .iter()
                .find(|entry| entry.validator == validator)
                .expect("recorded validator");
            assert_eq!(entry.outcome, BlockVoteOutcome::Refused, "{entry:?}");
            assert_eq!(
                entry.refusal_class.as_deref(),
                Some("invalid_vote_response"),
                "{entry:?}"
            );
            assert!(entry.vote_id.is_none(), "{entry:?}");
            assert!(!certificate_qc_voters(&certificate_file, "prepare_qc").contains(&validator));
        }
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file)
            .expect("close persistent streams");
    }

    #[test]
    fn a_prepare_reply_without_a_consensus_v2_vote_counts_as_a_missing_vote() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        // Offset 1 answers its prepare without a v2 vote; offset 2 answers
        // late, after the four fast replies that would otherwise stop the
        // prepare phase.
        let fixture = vote_phase_round_fixture(
            "vote-omitted-v2",
            &[
                (1, ProxyVote::OmitV2Vote, forward),
                (2, ProxyVote::Forward(Duration::from_secs(3)), forward),
            ],
        );
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        eprintln!("omitted v2 vote: elapsed={elapsed:?}");
        let round = result
            .expect("round still running")
            .expect("a reply without a v2 vote is a missing vote, not a round error");
        assert!(lock_free);
        assert!(round.certification.round_ok);
        let omitting = format!("validator-{}", fixture.proxied[0].0);
        let certificate_file = fixture.artifact_dir.join("block-certificate.json");
        assert!(!certificate_qc_voters(&certificate_file, "prepare_qc").contains(&omitting));
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    #[test]
    fn a_shielded_batch_certified_while_a_peer_refused_is_reported_final_and_cached() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        // A permanent refusal is a recorded failure before the slow fifth
        // vote completes the quorum, as on the RPC finality paths.
        let fixture = vote_phase_round_fixture(
            "vote-refused-final",
            &[
                (1, ProxyVote::RefuseConflicting, forward),
                (2, ProxyVote::Forward(Duration::from_secs(2)), forward),
            ],
        );
        let (result, _, lock_free) = run_vote_phase_round_with_sends(
            &fixture,
            UNIT_TIMEOUT_MS,
            0,
            UNIT_RETRY_BACKOFF_MS,
            true,
            true,
            PEER_VOTE_PHASE_DEADLINE_MS,
            Duration::from_secs(30),
        );
        let round = result
            .expect("round still running")
            .expect("round certifies without the refusing peer");
        assert!(lock_free);
        assert!(round.certification.round_ok);
        assert!(!round.vote_request_failures.is_empty());
        let result_file = fixture.artifact_dir.join("shield-result.json");
        let request = RpcRequest::new("shield-1", "shield_batch_finality", serde_json::json!({}));
        let response = match finish_rpc_serve_shield_batch_finality(
            &request,
            &"a".repeat(96),
            1,
            0.0,
            Instant::now(),
            round,
            &result_file,
            true,
        ) {
            Ok(response) => response,
            Err((code, message)) => panic!("certified shielded batch reported {code}: {message}"),
        };
        assert!(response.ok);
        let cached: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&result_file).expect("cached result"))
                .expect("cached result json");
        let result = response.result.as_ref().expect("finality result");
        for key in ["schema", "batch_id", "certificate"] {
            assert!(cached[key] == result[key], "cached result differs at `{key}`");
        }
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    #[test]
    fn silent_peers_end_the_vote_phase_at_its_deadline_and_release_the_lock() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture = vote_phase_round_fixture(
            "vote-phase-deadline",
            &[
                (1, ProxyVote::Silent, forward),
                (2, ProxyVote::Silent, forward),
            ],
        );
        // Phase deadline 20 s, plus margin for the round's local work.
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(28),
        );
        let silent = fixture
            .proxied
            .iter()
            .map(|(index, _)| fixture.requests(*index, 0))
            .collect::<Vec<_>>();
        eprintln!("vote phase deadline: elapsed={elapsed:?} lock_free={lock_free} prepare_requests_to_silent_peers={silent:?}");
        let result = result.unwrap_or_else(|| {
            panic!("round still holds the mempool lock after {elapsed:?} without prepare quorum")
        });
        result.expect_err("two silent peers leave the round below quorum");
        assert!(lock_free);
        assert!(
            elapsed >= Duration::from_secs(19),
            "phase ended early: {elapsed:?}"
        );
        assert_eq!(
            silent,
            vec![1, 1],
            "silent peers were asked again after the deadline"
        );
        assert!(!fixture.artifact_dir.join("block-certificate.json").exists());
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    #[test]
    fn the_configured_vote_phase_deadline_ends_a_phase_without_quorum() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture = vote_phase_round_fixture(
            "vote-phase-deadline-flag",
            &[
                (1, ProxyVote::Silent, forward),
                (2, ProxyVote::Silent, forward),
            ],
        );
        let (result, elapsed, lock_free) = run_vote_phase_round_with_sends(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            3_000,
            Duration::from_secs(15),
        );
        eprintln!("vote phase deadline 3 s: elapsed={elapsed:?} lock_free={lock_free}");
        result
            .expect("round still running")
            .expect_err("two silent peers leave the round below quorum");
        assert!(lock_free);
        assert!(elapsed >= Duration::from_secs(3) && elapsed < Duration::from_secs(10));
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    #[test]
    fn all_votes_rounds_keep_the_configured_vote_timeout() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = vote_phase_round_fixture(
            "all-votes-timeout",
            &[(
                1,
                ProxyVote::Forward(Duration::from_secs(22)),
                ProxyVote::Forward(Duration::ZERO),
            )],
        );
        let (result, elapsed, _) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            0,
            UNIT_RETRY_BACKOFF_MS,
            false,
            Duration::from_secs(60),
        );
        eprintln!("all-votes timeout: elapsed={elapsed:?}");
        let report = result
            .expect("round still running")
            .expect("a 22 s vote is inside the configured 30 s timeout");
        assert!(report.round_ok, "{:?}", report.send_failures);
        assert!(elapsed >= Duration::from_secs(22));
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// The wallet proxy's rule for moving a failed finality submission to the
    /// next view (wallet-proxy/rpc-routing.js, finalityFailureCanAdvanceView),
    /// applied to the RPC code the node sends for this error; keep it in step.
    fn submission_failure_advances_view(error: &str) -> bool {
        match rpc_finality_failure_code("rpc_finality_submit_failed", error) {
            "rpc_finality_view_spent" => true,
            "rpc_finality_view_must_repropose" => false,
            _ => submission_failure_advances_view_on_release_6_4(error),
        }
    }

    /// The text rule for nodes that send every failure as
    /// `rpc_finality_submit_failed`.
    fn submission_failure_advances_view_on_release_6_4(error: &str) -> bool {
        let error = error.to_ascii_lowercase();
        [
            "vote request",
            "vote workers",
            "quorum",
            "certificate",
            "transport",
            "timed out",
            "connection",
            "view spent",
            "conflicting block proposal vote already recorded",
        ]
        .iter()
        .any(|pattern| error.contains(pattern))
    }

    /// Proposal hashes a validator's vote locks hold at (height, view).
    fn vote_lock_proposal_hashes(data_dir: &Path, height: u64, view: u64) -> Vec<String> {
        let prefix = format!("{height}.{view}.");
        let mut hashes = std::fs::read_dir(data_dir.join("block_proposal_vote_locks"))
            .expect("vote lock directory")
            .map(|entry| entry.expect("vote lock entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".json"))
            })
            .map(|path| {
                let lock: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(path).expect("read vote lock"))
                        .expect("vote lock json");
                lock["proposal_hash"].as_str().expect("proposal hash").to_string()
            })
            .collect::<Vec<_>>();
        hashes.sort();
        hashes
    }

    /// Heights 1701 and 1707: the view-0 proposer of height 1 reserves its
    /// vote lock with the proposal signature, signs its own vote and prepare,
    /// then stops; no peer is asked. It then comes back as a voter. Returns the
    /// proposal hash its view-0 lock holds.
    fn fail_view_0_proposer_after_its_prepare(fixture: &VotePhaseRoundFixture) -> Vec<String> {
        let proposer = fixture.proposer_index;
        let proposer_dir = fixture.data_dirs[proposer].clone();
        let proposer_id = format!("validator-{proposer}");
        let key_file = proposer_dir.join(VALIDATOR_KEYS_FILE);

        // The failed run, as on validator 3: vote lock reserved by the
        // proposal signature, proposal, own vote, own prepare. Then nothing.
        let crashed_dir = fixture.artifact_dir.with_extension("crashed");
        std::fs::create_dir_all(&crashed_dir).expect("crashed run directory");
        let crashed_batch = crashed_dir.join("batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: proposer_dir.clone(),
            key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "e".repeat(40)),
            amount: 7,
            batch_file: crashed_batch.clone(),
        })
        .expect("crashed run batch");
        let unsigned = propose_batch(BatchProposalOptions {
            data_dir: proposer_dir.clone(),
            verify_block_log: false,
            batch_kind: Some("transparent".to_string()),
            batch_file: crashed_batch,
            proposal_file: crashed_dir.join("block-proposal.json"),
            view: Some(0),
            timeout_certificate_file: None,
            key_file: None,
            validator_id: None,
        })
        .expect("crashed run proposal");
        let signed = sign_verified_block_proposal(&proposer_dir, unsigned, &key_file, &proposer_id)
            .expect("crashed run proposal signature");
        let consensus_v2_proposal =
            create_consensus_v2_proposal_for_block(&proposer_dir, &signed, None, &key_file)
                .expect("crashed run consensus v2 proposal");
        create_block_vote_for_verified_proposal(BlockVoteForVerifiedProposalOptions {
            data_dir: proposer_dir.clone(),
            verify_block_log: false,
            key_file: key_file.clone(),
            validator_id: Some(proposer_id.clone()),
            proposal: signed,
            block_height: Some(1),
            vote_file: crashed_dir.join(format!("{proposer_id}.block_vote.json")),
        })
        .expect("crashed run own vote");
        create_consensus_v2_prepare_vote(
            &proposer_dir,
            &consensus_v2_proposal,
            None,
            &key_file,
            &proposer_id,
        )
        .expect("crashed run own prepare");
        let crashed_lock = vote_lock_proposal_hashes(&proposer_dir, 1, 0);
        assert_eq!(crashed_lock.len(), 1, "{crashed_lock:?}");
        // The proposer comes back as a voter.
        spawn_vote_phase_validator(
            proposer_dir.clone(),
            fixture.topology_file.clone(),
            fixture
                .proposer_listener
                .try_clone()
                .expect("clone proposer listener"),
        );

        crashed_lock
    }

    /// Timeout votes for height 1 at `view` from the given validators, as the
    /// RPC's consensus_v2_timeout_vote signs them. Returns the vote files that
    /// were signed and the refusals.
    fn timeout_votes(
        fixture: &VotePhaseRoundFixture,
        validators: &[usize],
        view: u64,
        dir: &Path,
    ) -> (Vec<PathBuf>, Vec<String>) {
        std::fs::create_dir_all(dir).expect("timeout vote directory");
        let mut signed = Vec::new();
        let mut refused = Vec::new();
        for &index in validators {
            let data_dir = &fixture.data_dirs[index];
            let vote_file = dir.join(format!("validator-{index}.h1.v{view}.timeout.json"));
            match create_block_timeout_vote(BlockTimeoutVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: false,
                key_file: data_dir.join(VALIDATOR_KEYS_FILE),
                validator_id: Some(format!("validator-{index}")),
                block_height: 1,
                view,
                high_qc_id: RPC_FINALITY_TIMEOUT_HIGH_QC_COMPAT.to_string(),
                vote_file: vote_file.clone(),
            }) {
                Ok(_) => signed.push(vote_file),
                Err(error) => refused.push(error.to_string()),
            }
        }
        (signed, refused)
    }

    fn height_1_leader(fixture: &VotePhaseRoundFixture, view: u64) -> usize {
        let (_, validators) =
            live_consensus_v2_context(&fixture.data_dirs[0]).expect("consensus context");
        let leader = postfiat_ordering_fast::leader_for_view(&validators.validator_ids(), 1, view)
            .expect("leader");
        validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &leader)
            .expect("leader index")
    }

    /// Timeout certificate for height 1 at `view`, built by the next view's leader.
    fn timeout_certificate(
        fixture: &VotePhaseRoundFixture,
        view: u64,
        vote_files: Vec<PathBuf>,
        dir: &Path,
    ) -> io::Result<PathBuf> {
        let certificate_file = dir.join(format!("h1.v{view}.timeout-certificate.json"));
        aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
            data_dir: fixture.data_dirs[height_1_leader(fixture, view + 1)].clone(),
            verify_block_log: false,
            block_height: 1,
            view,
            vote_files,
            certificate_file: certificate_file.clone(),
        })?;
        Ok(certificate_file)
    }

    /// All six validators hold the same block at height 1.
    fn assert_height_1_final_everywhere(fixture: &VotePhaseRoundFixture) {
        let tips = fixture
            .data_dirs
            .iter()
            .map(|data_dir| {
                let tip = status(NodeOptions {
                    data_dir: data_dir.clone(),
                })
                .expect("validator status");
                (tip.block_height, tip.block_tip_hash)
            })
            .collect::<Vec<_>>();
        assert!(
            tips.iter().all(|tip| tip == &tips[0]) && tips[0].0 == 1,
            "{tips:?}"
        );
    }

    /// Two submissions recover height 1 at the same time and split the timeout
    /// votes (3 and 3, quorum 5); a third gets a certificate but its view-1
    /// attempt ends busy. Every validator has now signed its one view-0 timeout
    /// vote. A fourth submission must still finish the height.
    #[test]
    fn split_and_abandoned_recoveries_leave_the_height_recoverable() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = vote_phase_round_fixture("resumable-recovery", &[]);
        fail_view_0_proposer_after_its_prepare(&fixture);
        let dir = fixture.artifact_dir.with_extension("recoveries");
        // Two concurrent collections, each reaching half the validators.
        let (first, second) = std::thread::scope(|scope| {
            let first = scope.spawn(|| timeout_votes(&fixture, &[0, 2, 4], 0, &dir.join("c1")));
            let second = scope.spawn(|| timeout_votes(&fixture, &[1, 3, 5], 0, &dir.join("c2")));
            (first.join().expect("c1"), second.join().expect("c2"))
        });
        assert_eq!((first.0.len(), second.0.len()), (3, 3), "{first:?} {second:?}");
        assert!(first.0.len() < fixture.quorum && second.0.len() < fixture.quorum);
        // A third collection reaches everyone; its view-1 attempt ends busy.
        let (third, refused) = timeout_votes(&fixture, &[0, 1, 2, 3, 4, 5], 0, &dir.join("c3"));
        assert!(
            third.len() >= fixture.quorum,
            "height 1 is wedged: a later recovery collected {} timeout votes, need {}: {refused:?}",
            third.len(),
            fixture.quorum
        );
        // Each validator answered with the vote it signed the first time.
        for index in 0..6 {
            let earlier = if index % 2 == 0 { &first.0 } else { &second.0 };
            let read = |path: &PathBuf| -> serde_json::Value {
                serde_json::from_slice(&std::fs::read(path).expect("vote file")).expect("vote json")
            };
            let earlier = earlier
                .iter()
                .find(|path| path.to_string_lossy().contains(&format!("validator-{index}.")))
                .expect("earlier vote");
            let again = third
                .iter()
                .find(|path| path.to_string_lossy().contains(&format!("validator-{index}.")))
                .expect("vote again");
            assert_eq!(
                read(earlier)["consensus_v2_vote"],
                read(again)["consensus_v2_vote"],
                "validator-{index} signed a second timeout vote"
            );
        }
        timeout_certificate(&fixture, 0, third, &dir.join("c3")).expect("third certificate");
        // A fourth submission collects again and finishes at view 1.
        let (fourth, refused) = timeout_votes(&fixture, &[0, 1, 2, 3, 4, 5], 0, &dir.join("c4"));
        assert!(fourth.len() >= fixture.quorum, "{refused:?}");
        let certificate =
            timeout_certificate(&fixture, 0, fourth, &dir.join("c4")).expect("fourth certificate");
        let (result, _, _) = run_vote_phase_round_at(
            &fixture,
            &RoundSlot {
                proposer_index: height_1_leader(&fixture, 1),
                view: 1,
                timeout_certificate_file: Some(certificate),
                artifact_dir: dir.join("view-1-round"),
                batch_file: None,
            },
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            PEER_VOTE_PHASE_DEADLINE_MS,
            Duration::from_secs(30),
        );
        let report = result
            .expect("view-1 round still running")
            .expect("view 1 certifies height 1");
        assert!(report.round_ok, "{:?}", report.send_failures);
        assert_height_1_final_everywhere(&fixture);
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// View 0 forms a prepare QC for block A; two precommits arrive after the
    /// phase deadline, so the proposer holds 4 of 5 and view 0 fails while
    /// every validator is locked on A. The timeout certificate carries A's QC,
    /// so view 1 must carry A. A view-1 proposer holding another batch must
    /// stop before it signs anything (keeping its view-1 slot), and then
    /// finish the height by proposing A's batch.
    #[test]
    fn view_1_after_a_view_0_prepare_qc_must_carry_that_block() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let late = ProxyVote::Forward(Duration::from_secs(5));
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture =
            vote_phase_round_fixture("view-1-reproposal", &[(2, forward, late), (3, forward, late)]);
        let (result, _, _) = run_vote_phase_round_with_sends(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            3_000,
            Duration::from_secs(30),
        );
        result
            .expect("view-0 round still running")
            .expect_err("view 0 misses its precommit quorum");
        let block_a = fixture.artifact_dir.with_extension("batch.json");
        spawn_vote_phase_validator(
            fixture.data_dirs[fixture.proposer_index].clone(),
            fixture.topology_file.clone(),
            fixture
                .proposer_listener
                .try_clone()
                .expect("clone proposer listener"),
        );
        let dir = fixture.artifact_dir.with_extension("view-1");
        let (votes, refused) = timeout_votes(&fixture, &[0, 1, 2, 3, 4, 5], 0, &dir);
        assert_eq!(votes.len(), 6, "{refused:?}");
        let certificate = timeout_certificate(&fixture, 0, votes, &dir).expect("certificate");
        let leader = height_1_leader(&fixture, 1);
        let leader_dir = fixture.data_dirs[leader].clone();
        let block_b = dir.join("block-b.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: leader_dir.clone(),
            key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "b".repeat(40)),
            amount: 5,
            batch_file: block_b.clone(),
        })
        .expect("block B batch");
        let round_at = |batch_file: PathBuf, tag: &str| {
            run_vote_phase_round_at(
                &fixture,
                &RoundSlot {
                    proposer_index: leader,
                    view: 1,
                    timeout_certificate_file: Some(certificate.clone()),
                    artifact_dir: dir.join(tag),
                    batch_file: Some(batch_file),
                },
                UNIT_TIMEOUT_MS,
                UNIT_SEND_RETRIES,
                UNIT_RETRY_BACKOFF_MS,
                true,
                false,
                PEER_VOTE_PHASE_DEADLINE_MS,
                Duration::from_secs(40),
            )
            .0
            .expect("view-1 round still running")
        };
        let error = round_at(block_b, "with-b").expect_err("view 1 cannot carry block B");
        assert!(
            error.starts_with("consensus v2 view must re-propose"),
            "view 1 failed only after signing: {error}"
        );
        assert!(
            !submission_failure_advances_view(&error),
            "the proxy would spend every later view on `{error}`, and view 1 could no longer re-propose block A"
        );
        assert!(
            vote_lock_proposal_hashes(&leader_dir, 1, 1).is_empty(),
            "the view-1 proposer spent its view-1 slot on block B"
        );
        let report = round_at(block_a, "with-a").expect("view 1 certifies block A");
        assert!(report.round_ok, "{:?}", report.send_failures);
        assert_height_1_final_everywhere(&fixture);
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// As above, but block B is submitted at a validator that is not the
    /// view-1 proposer and forwards it. The forwarded path must stop before
    /// the view-1 proposer signs too, so view 1 can still carry block A.
    #[test]
    fn a_forwarded_view_1_proposal_must_carry_the_prepare_qc_block() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let late = ProxyVote::Forward(Duration::from_secs(5));
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture = vote_phase_round_fixture(
            "view-1-forwarded-reproposal",
            &[(2, forward, late), (3, forward, late)],
        );
        let (result, _, _) = run_vote_phase_round_with_sends(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            3_000,
            Duration::from_secs(30),
        );
        result
            .expect("view-0 round still running")
            .expect_err("view 0 misses its precommit quorum");
        let block_a = fixture.artifact_dir.with_extension("batch.json");
        spawn_vote_phase_validator(
            fixture.data_dirs[fixture.proposer_index].clone(),
            fixture.topology_file.clone(),
            fixture
                .proposer_listener
                .try_clone()
                .expect("clone proposer listener"),
        );
        let dir = fixture.artifact_dir.with_extension("view-1");
        let (votes, refused) = timeout_votes(&fixture, &[0, 1, 2, 3, 4, 5], 0, &dir);
        assert_eq!(votes.len(), 6, "{refused:?}");
        let certificate = timeout_certificate(&fixture, 0, votes, &dir).expect("certificate");
        let leader = height_1_leader(&fixture, 1);
        let leader_dir = fixture.data_dirs[leader].clone();
        let requester = (0..fixture.data_dirs.len())
            .find(|index| *index != leader && *index != fixture.proposer_index)
            .expect("a validator that is not a proposer");
        let requester_dir = fixture.data_dirs[requester].clone();
        let block_b = dir.join("block-b.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: requester_dir.clone(),
            key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "b".repeat(40)),
            amount: 5,
            batch_file: block_b.clone(),
        })
        .expect("block B batch");
        let error = transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
            data_dir: requester_dir.clone(),
            topology_file: fixture.topology_file.clone(),
            batch_kind: Some("transparent".to_string()),
            batch_file: block_b,
            key_file: requester_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: Some(requester_dir.join(VALIDATOR_KEYS_FILE)),
            require_local_proposer: false,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: true,
            artifact_dir: dir.join("forwarded-b"),
            block_height: Some(1),
            view: Some(1),
            timeout_certificate_file: Some(certificate.clone()),
            timeout_ms: UNIT_TIMEOUT_MS,
            send_retries: 0,
            retry_backoff_ms: UNIT_RETRY_BACKOFF_MS,
            vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
            local_apply_before_certified_send: true,
            defer_certified_sends: false,
            commit_processed_dir: None,
            required_parent: None,
        })
        .expect_err("view 1 cannot carry block B");
        assert!(error.contains("consensus v2 view must re-propose"), "{error}");
        assert!(
            vote_lock_proposal_hashes(&leader_dir, 1, 1).is_empty(),
            "the view-1 proposer spent its view-1 slot on forwarded block B: {error}"
        );
        let (result, _, _) = run_vote_phase_round_at(
            &fixture,
            &RoundSlot {
                proposer_index: leader,
                view: 1,
                timeout_certificate_file: Some(certificate),
                artifact_dir: dir.join("with-a"),
                batch_file: Some(block_a),
            },
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            PEER_VOTE_PHASE_DEADLINE_MS,
            Duration::from_secs(40),
        );
        let report = result
            .expect("view-1 round still running")
            .expect("view 1 certifies block A");
        assert!(report.round_ok, "{:?}", report.send_failures);
        assert_height_1_final_everywhere(&fixture);
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// View 0 forms a prepare QC Q for block A, but every precommit request is
    /// refused, so only the view-0 proposer stores Q (and locks on A). Its
    /// timeout vote names Q, so the timeout certificate does too. The view-1
    /// proposer and every peer must still verify that certificate and finish
    /// height 1 at view 1 with block A.
    #[test]
    fn a_prepare_qc_held_only_by_the_view_0_proposer_still_finishes_at_view_1() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let forward = ProxyVote::Forward(Duration::ZERO);
        // Refused through view 0 (3 s deadline); forwarded again for view 1.
        let refused = ProxyVote::RefuseFor(Duration::from_secs(8));
        // Proxies are placed by offset from the view-0 proposer: every peer.
        let fixture = vote_phase_round_fixture(
            "view-1-sole-qc-holder",
            &(1..6).map(|offset| (offset, forward, refused)).collect::<Vec<_>>(),
        );
        let peers = (0..fixture.data_dirs.len())
            .filter(|index| *index != fixture.proposer_index)
            .collect::<Vec<_>>();
        let (result, _, _) = run_vote_phase_round_with_sends(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            3_000,
            Duration::from_secs(30),
        );
        result
            .expect("view-0 round still running")
            .expect_err("view 0 gets no precommit from any peer");
        let block_a = fixture.artifact_dir.with_extension("batch.json");
        spawn_vote_phase_validator(
            fixture.data_dirs[fixture.proposer_index].clone(),
            fixture.topology_file.clone(),
            fixture
                .proposer_listener
                .try_clone()
                .expect("clone proposer listener"),
        );
        let dir = fixture.artifact_dir.with_extension("view-1");
        let (votes, refused_votes) = timeout_votes(&fixture, &[0, 1, 2, 3, 4, 5], 0, &dir);
        assert_eq!(votes.len(), 6, "{refused_votes:?}");
        let proposer_vote: BlockTimeoutVoteFile = serde_json::from_slice(
            &std::fs::read(
                dir.join(format!("validator-{}.h1.v0.timeout.json", fixture.proposer_index)),
            )
            .expect("proposer timeout vote"),
        )
        .expect("proposer timeout vote json");
        let qc_id = proposer_vote
            .consensus_v2_vote
            .and_then(|vote| vote.high_qc)
            .expect("the view-0 proposer's timeout vote names its prepare QC")
            .certificate_id;
        for &peer in &peers {
            let held = std::fs::read_dir(fixture.data_dirs[peer].join("consensus-v2-qcs"))
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .any(|entry| entry.file_name().to_string_lossy().contains(&qc_id))
                })
                .unwrap_or(false);
            assert!(!held, "validator-{peer} already holds the view-0 prepare QC");
        }
        let leader = height_1_leader(&fixture, 1);
        assert_ne!(leader, fixture.proposer_index);
        let certificate = timeout_certificate(&fixture, 0, votes, &dir)
            .expect("the view-1 proposer builds the timeout certificate");
        let (result, _, _) = run_vote_phase_round_at(
            &fixture,
            &RoundSlot {
                proposer_index: leader,
                view: 1,
                timeout_certificate_file: Some(certificate),
                artifact_dir: dir.join("with-a"),
                batch_file: Some(block_a),
            },
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            PEER_VOTE_PHASE_DEADLINE_MS,
            Duration::from_secs(40),
        );
        let report = result
            .expect("view-1 round still running")
            .expect("view 1 certifies block A");
        assert!(report.round_ok, "{:?}", report.send_failures);
        assert_height_1_final_everywhere(&fixture);
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// The view-0 setup of the test above: view 0 forms a prepare QC Q that
    /// only the view-0 proposer stores. Returns the view-0 timeout votes of
    /// all six validators (the proposer's carries Q), Q's ID, and a validator
    /// that is neither proposer and does not hold Q.
    fn sole_qc_holder_timeout_votes(
        tag: &str,
    ) -> (VotePhaseRoundFixture, PathBuf, Vec<PathBuf>, String, usize) {
        let forward = ProxyVote::Forward(Duration::ZERO);
        let refused = ProxyVote::RefuseFor(Duration::from_secs(8));
        let fixture = vote_phase_round_fixture(
            tag,
            &(1..6).map(|offset| (offset, forward, refused)).collect::<Vec<_>>(),
        );
        let (result, _, _) = run_vote_phase_round_with_sends(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            3_000,
            Duration::from_secs(30),
        );
        result
            .expect("view-0 round still running")
            .expect_err("view 0 gets no precommit from any peer");
        let dir = fixture.artifact_dir.with_extension("view-1");
        let (votes, refused_votes) = timeout_votes(&fixture, &[0, 1, 2, 3, 4, 5], 0, &dir);
        assert_eq!(votes.len(), 6, "{refused_votes:?}");
        let proposer_vote: BlockTimeoutVoteFile = serde_json::from_slice(
            &std::fs::read(&votes[fixture.proposer_index]).expect("proposer timeout vote"),
        )
        .expect("proposer timeout vote json");
        let qc_id = proposer_vote
            .consensus_v2_vote
            .and_then(|vote| vote.high_qc)
            .expect("the view-0 proposer's timeout vote names its prepare QC")
            .certificate_id;
        let leader = height_1_leader(&fixture, 1);
        let peer = (0..fixture.data_dirs.len())
            .find(|index| *index != leader && *index != fixture.proposer_index)
            .expect("a validator that is not a proposer");
        assert!(!holds_qc(&fixture.data_dirs[leader], &qc_id));
        assert!(!holds_qc(&fixture.data_dirs[peer], &qc_id));
        (fixture, dir, votes, qc_id, peer)
    }

    fn holds_qc(data_dir: &Path, qc_id: &str) -> bool {
        std::fs::read_dir(data_dir.join("consensus-v2-qcs"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .any(|entry| entry.file_name().to_string_lossy().contains(qc_id))
            })
            .unwrap_or(false)
    }

    /// A copy of the JSON file `from` at `to`, changed by `edit`.
    fn edited_json(from: &Path, to: PathBuf, edit: impl FnOnce(&mut serde_json::Value)) -> PathBuf {
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(from).expect("json file")).expect("json");
        edit(&mut value);
        std::fs::create_dir_all(to.parent().expect("parent")).expect("edited json directory");
        std::fs::write(&to, serde_json::to_vec_pretty(&value).expect("json bytes"))
            .expect("edited json");
        to
    }

    fn break_signature(signature: &mut serde_json::Value) {
        let hex = signature["signature_hex"].as_str().expect("signature hex");
        let first = if hex.starts_with('0') { "1" } else { "0" };
        signature["signature_hex"] = format!("{first}{}", &hex[1..]).into();
    }

    /// A carried QC is stored only once the timeout vote or certificate that
    /// carries it verifies.
    #[test]
    fn a_carried_qc_is_stored_only_after_its_vote_or_certificate_verifies() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (fixture, dir, votes, qc_id, peer) =
            sole_qc_holder_timeout_votes("carried-qc-stored-after-verification");
        let leader_dir = &fixture.data_dirs[height_1_leader(&fixture, 1)];
        let mut bad_votes = votes.clone();
        bad_votes[fixture.proposer_index] = edited_json(
            &votes[fixture.proposer_index],
            dir.join("bad-vote/proposer.json"),
            |vote| break_signature(&mut vote["consensus_v2_vote"]["signature"]),
        );
        timeout_certificate(&fixture, 0, bad_votes, &dir.join("bad-vote"))
            .expect_err("a timeout vote with a broken signature");
        assert!(
            !holds_qc(leader_dir, &qc_id),
            "the aggregator stored the QC a vote carried before the vote verified"
        );
        let certificate = timeout_certificate(&fixture, 0, votes, &dir).expect("certificate");
        assert!(holds_qc(leader_dir, &qc_id));

        let peer_dir = &fixture.data_dirs[peer];
        let bad_certificate =
            edited_json(&certificate, dir.join("bad-certificate.json"), |certificate| {
                let votes = certificate["consensus_v2_certificate"]["votes"]
                    .as_array_mut()
                    .expect("v2 votes");
                let vote = votes
                    .iter_mut()
                    .find(|vote| !vote["high_qc"].is_null())
                    .expect("a vote naming the QC");
                break_signature(&mut vote["signature"]);
            });
        let verify = |certificate_file: &PathBuf| {
            verify_block_timeout_certificate_file(BlockTimeoutCertificateVerifyOptions {
                data_dir: peer_dir.clone(),
                verify_block_log: false,
                certificate_file: certificate_file.clone(),
            })
        };
        verify(&bad_certificate).expect_err("a certificate with a broken vote signature");
        assert!(
            !holds_qc(peer_dir, &qc_id),
            "a peer stored the QC a certificate carried before the certificate verified"
        );
        verify(&certificate).expect("the genuine certificate");
        assert!(holds_qc(peer_dir, &qc_id));
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// A carried QC that does not verify is ignored: it fails a vote or
    /// certificate only when the receiver does not hold the QC it names.
    #[test]
    fn a_carried_qc_that_does_not_verify_is_ignored_when_the_receiver_holds_the_named_qc() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (fixture, dir, votes, qc_id, peer) =
            sole_qc_holder_timeout_votes("carried-qc-ignored");
        let leader_dir = &fixture.data_dirs[height_1_leader(&fixture, 1)];
        let mut swapped_votes = votes.clone();
        swapped_votes[fixture.proposer_index] = edited_json(
            &votes[fixture.proposer_index],
            dir.join("swapped-vote/proposer.json"),
            |vote| break_signature(&mut vote["consensus_v2_high_qc"]["votes"][0]["signature"]),
        );
        let error = timeout_certificate(&fixture, 0, swapped_votes.clone(), &dir.join("swapped-1"))
            .expect_err("the aggregator does not hold the QC")
            .to_string();
        assert!(error.contains("unresolved"), "{error}");
        timeout_certificate(&fixture, 0, votes.clone(), &dir.join("genuine-1"))
            .expect("certificate");
        assert!(holds_qc(leader_dir, &qc_id));
        timeout_certificate(&fixture, 0, swapped_votes, &dir.join("swapped-2"))
            .expect("the aggregator holds the QC, so the swapped copy is ignored");

        let certificate =
            timeout_certificate(&fixture, 0, votes, &dir).expect("certificate");
        let swapped_certificate =
            edited_json(&certificate, dir.join("swapped-certificate.json"), |certificate| {
                for qc in certificate["consensus_v2_high_qcs"]
                    .as_array_mut()
                    .expect("carried QCs")
                {
                    break_signature(&mut qc["votes"][0]["signature"]);
                }
            });
        let peer_dir = &fixture.data_dirs[peer];
        let verify = |certificate_file: &PathBuf| {
            verify_block_timeout_certificate_file(BlockTimeoutCertificateVerifyOptions {
                data_dir: peer_dir.clone(),
                verify_block_log: false,
                certificate_file: certificate_file.clone(),
            })
        };
        let error = verify(&swapped_certificate)
            .expect_err("the peer does not hold the QC")
            .to_string();
        assert!(error.contains("unresolved"), "{error}");
        verify(&certificate).expect("the genuine certificate");
        verify(&swapped_certificate)
            .expect("the peer holds the QC, so the swapped copy is ignored");
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// The certificate carries every distinct QC its votes name, under a list
    /// field an older node ignores.
    #[test]
    fn a_timeout_certificate_carries_every_qc_its_votes_name() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (fixture, dir, votes, qc_id, _) = sole_qc_holder_timeout_votes("carried-qcs");
        let certificate = timeout_certificate(&fixture, 0, votes, &dir).expect("certificate");
        let certificate: serde_json::Value =
            serde_json::from_slice(&std::fs::read(certificate).expect("certificate"))
                .expect("certificate json");
        let named = certificate["consensus_v2_certificate"]["votes"]
            .as_array()
            .expect("v2 votes")
            .iter()
            .filter_map(|vote| vote["high_qc"]["certificate_id"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let carried = certificate["consensus_v2_high_qcs"]
            .as_array()
            .map(|qcs| {
                qcs.iter()
                    .filter_map(|qc| qc["certificate_id"].as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(named, [qc_id.as_str()].into_iter().collect());
        assert_eq!(carried, named.into_iter().collect::<Vec<_>>(), "{certificate:#}");
        assert!(certificate.get("consensus_v2_high_qc").is_none());
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// The view-1 proposer already signed at view 1 and the timeout
    /// certificate requires block A. The spent view is reported first, so the
    /// proxy moves on to view 2 instead of stopping on view 1.
    #[test]
    fn a_spent_view_is_reported_before_a_required_reproposal() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (fixture, dir, certificate, leader) =
            view_0_prepare_qc_on_every_validator("spent-before-reproposal");
        let leader_dir = fixture.data_dirs[leader].clone();
        let (signed, refused) = timeout_votes(&fixture, &[leader], 1, &dir.join("spent"));
        assert_eq!(signed.len(), 1, "{refused:?}");
        let block_b = dir.join("block-b.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: leader_dir.clone(),
            key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "b".repeat(40)),
            amount: 5,
            batch_file: block_b.clone(),
        })
        .expect("block B batch");
        let (result, _, _) = run_vote_phase_round_at(
            &fixture,
            &RoundSlot {
                proposer_index: leader,
                view: 1,
                timeout_certificate_file: Some(certificate),
                artifact_dir: dir.join("with-b"),
                batch_file: Some(block_b),
            },
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            PEER_VOTE_PHASE_DEADLINE_MS,
            Duration::from_secs(40),
        );
        let error = result
            .expect("view-1 round still running")
            .expect_err("view 1 is spent");
        assert_eq!(
            rpc_finality_failure_code("rpc_finality_submit_failed", &error),
            "rpc_finality_view_spent",
            "{error}"
        );
        assert!(submission_failure_advances_view(&error), "{error}");
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// View 0 forms a prepare QC for block A that every validator locks on,
    /// but misses its precommit quorum. Returns the view-0 timeout
    /// certificate (which names A's QC) and the view-1 proposer.
    fn view_0_prepare_qc_on_every_validator(
        tag: &str,
    ) -> (VotePhaseRoundFixture, PathBuf, PathBuf, usize) {
        let late = ProxyVote::Forward(Duration::from_secs(5));
        let forward = ProxyVote::Forward(Duration::ZERO);
        let fixture = vote_phase_round_fixture(tag, &[(2, forward, late), (3, forward, late)]);
        let (result, _, _) = run_vote_phase_round_with_sends(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            3_000,
            Duration::from_secs(30),
        );
        result
            .expect("view-0 round still running")
            .expect_err("view 0 misses its precommit quorum");
        spawn_vote_phase_validator(
            fixture.data_dirs[fixture.proposer_index].clone(),
            fixture.topology_file.clone(),
            fixture
                .proposer_listener
                .try_clone()
                .expect("clone proposer listener"),
        );
        let dir = fixture.artifact_dir.with_extension("view-1");
        let (votes, refused) = timeout_votes(&fixture, &[0, 1, 2, 3, 4, 5], 0, &dir);
        assert_eq!(votes.len(), 6, "{refused:?}");
        let certificate = timeout_certificate(&fixture, 0, votes, &dir).expect("certificate");
        let leader = height_1_leader(&fixture, 1);
        (fixture, dir, certificate, leader)
    }

    /// A requester that skips the re-proposal check (release 6.4) sends block
    /// B to the view-1 proposer; the forwarded handler refuses it before it
    /// reserves its view-1 lock.
    #[test]
    fn the_forwarded_handler_refuses_a_view_1_proposal_without_the_prepare_qc_block() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (fixture, dir, certificate, leader) =
            view_0_prepare_qc_on_every_validator("forwarded-handler-reproposal");
        let leader_dir = fixture.data_dirs[leader].clone();
        let requester = (0..fixture.data_dirs.len())
            .find(|index| *index != leader && *index != fixture.proposer_index)
            .expect("a validator that is not a proposer");
        let requester_dir = fixture.data_dirs[requester].clone();
        let block_b = dir.join("block-b.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: requester_dir.clone(),
            key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "b".repeat(40)),
            amount: 5,
            batch_file: block_b.clone(),
        })
        .expect("block B batch");
        let local_status = status(NodeOptions {
            data_dir: requester_dir.clone(),
        })
        .expect("requester status");
        let required_parent = RequiredBlockParent {
            height: local_status.block_height,
            block_hash: local_status.block_tip_hash.clone(),
            state_root: local_status.state_root.clone(),
        };
        let expected = propose_batch_with_required_parent_with_timings(
            BatchProposalOptions {
                data_dir: requester_dir.clone(),
                verify_block_log: false,
                batch_kind: Some("transparent".to_string()),
                batch_file: block_b.clone(),
                proposal_file: dir.join("block-b.proposal.json"),
                view: Some(1),
                timeout_certificate_file: Some(certificate.clone()),
                key_file: None,
                validator_id: None,
            },
            &required_parent,
        )
        .expect("unsigned view-1 proposal of block B")
        .proposal;
        assert_eq!(expected.proposer, format!("validator-{leader}"));
        let error = crate::transport_runtime::transport_block_proposal_request(
            &requester_dir,
            &read_topology_file(&fixture.topology_file).expect("topology"),
            &local_status,
            &expected.proposer,
            &expected.batch_kind,
            &std::fs::read_to_string(&block_b).expect("block B batch json"),
            &expected,
            &required_parent,
            Some(&std::fs::read_to_string(&certificate).expect("certificate json")),
            UNIT_TIMEOUT_MS,
        )
        .expect_err("the view-1 proposer must refuse block B");
        assert!(error.contains("consensus v2 view must re-propose"), "{error}");
        assert!(
            vote_lock_proposal_hashes(&leader_dir, 1, 1).is_empty(),
            "the view-1 proposer spent its view-1 slot on forwarded block B: {error}"
        );
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// A proposer that already signed a timeout vote at view 1 cannot finish
    /// view 0 or view 1; the spent-view error names view 1 and asks for view 2.
    #[test]
    fn a_spent_view_names_the_highest_round_the_proposer_signed() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = vote_phase_round_fixture("spent-view-highest", &[]);
        fail_view_0_proposer_after_its_prepare(&fixture);
        let dir = fixture.artifact_dir.with_extension("timeouts");
        let (signed, refused) = timeout_votes(&fixture, &[fixture.proposer_index], 1, &dir);
        assert_eq!(signed.len(), 1, "{refused:?}");
        let (result, _, _) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        let error = result.expect("round still running").expect_err("view 0 is spent");
        assert!(
            error.contains("timeout certificate for view 1 and a proposal at view 2"),
            "{error}"
        );
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// A submission at a validator that is not the proposer asks the spent
    /// proposer to sign over transport; that path reports the spent view too.
    #[test]
    fn a_forwarded_proposal_to_a_spent_proposer_reports_the_spent_view() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = vote_phase_round_fixture("spent-view-forwarded", &[]);
        fail_view_0_proposer_after_its_prepare(&fixture);
        let requester = (fixture.proposer_index + 1) % fixture.data_dirs.len();
        let requester_dir = fixture.data_dirs[requester].clone();
        let artifact_dir = fixture.artifact_dir.with_extension("forwarded");
        let batch_file = artifact_dir.with_extension("batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: requester_dir.clone(),
            key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "c".repeat(40)),
            amount: 3,
            batch_file: batch_file.clone(),
        })
        .expect("forwarded batch");
        let error = transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
            data_dir: requester_dir.clone(),
            topology_file: fixture.topology_file.clone(),
            batch_kind: Some("transparent".to_string()),
            batch_file,
            key_file: requester_dir.join(VALIDATOR_KEYS_FILE),
            proposal_key_file: Some(requester_dir.join(VALIDATOR_KEYS_FILE)),
            require_local_proposer: false,
            require_signed_proposal: true,
            allow_peer_failures: false,
            quorum_early_full_propagation: true,
            artifact_dir,
            block_height: Some(1),
            view: Some(0),
            timeout_certificate_file: None,
            timeout_ms: UNIT_TIMEOUT_MS,
            send_retries: 0,
            retry_backoff_ms: UNIT_RETRY_BACKOFF_MS,
            vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
            local_apply_before_certified_send: true,
            defer_certified_sends: false,
            commit_processed_dir: None,
            required_parent: None,
        })
        .expect_err("the spent proposer cannot sign at view 0");
        assert!(error.contains("consensus v2 view spent"), "{error}");
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// Heights 1701 and 1707 (2026-11-19): the view-0 proposer signed a
    /// proposal, its own vote and its own prepare, then failed before any peer
    /// voted. Every later proposal at that height met its own vote lock, and
    /// nothing moved the height to view 1 until the records were moved aside
    /// by hand.
    #[test]
    fn a_proposer_that_signed_and_failed_does_not_wedge_its_height() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = vote_phase_round_fixture("spent-view", &[]);
        let proposer_dir = fixture.data_dirs[fixture.proposer_index].clone();
        let crashed_lock = fail_view_0_proposer_after_its_prepare(&fixture);

        // A normal submission at height 1 reaches the view-0 proposer.
        let start = Instant::now();
        let (result, _, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        let error = result
            .expect("round still running")
            .expect_err("the proposer cannot sign at view 0 again");
        assert!(lock_free);
        // Asking again changes nothing: the records never expire.
        let (retry, _, _) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        assert_eq!(
            retry.expect("retry still running").expect_err("retry at view 0"),
            error
        );
        assert!(
            submission_failure_advances_view(&error),
            "height 1 never finalizes: the submission failed with `{error}`, which no recovery path treats as a reason to change view"
        );
        // The proposer reports the spent view itself, not a message that only
        // happens to match a broader pattern.
        assert!(error.starts_with("consensus v2 view spent"), "{error}");

        // The proxy's recovery: timeout votes for view 0 from every validator,
        // a timeout certificate, the view-1 proposer.
        let recovery_dir = fixture.artifact_dir.with_extension("view-1");
        std::fs::create_dir_all(&recovery_dir).expect("recovery directory");
        let vote_files = fixture
            .data_dirs
            .iter()
            .enumerate()
            .map(|(index, data_dir)| {
                let vote_file = recovery_dir.join(format!("validator-{index}.timeout.json"));
                create_block_timeout_vote(BlockTimeoutVoteOptions {
                    data_dir: data_dir.clone(),
                    verify_block_log: false,
                    key_file: data_dir.join(VALIDATOR_KEYS_FILE),
                    validator_id: Some(format!("validator-{index}")),
                    block_height: 1,
                    view: 0,
                    high_qc_id: RPC_FINALITY_TIMEOUT_HIGH_QC_COMPAT.to_string(),
                    vote_file: vote_file.clone(),
                })
                .expect("timeout vote");
                vote_file
            })
            .collect::<Vec<_>>();
        let (_, validators) = live_consensus_v2_context(&proposer_dir).expect("context");
        let view_1_proposer =
            postfiat_ordering_fast::leader_for_view(&validators.validator_ids(), 1, 1)
                .expect("view-1 proposer");
        let view_1_index = validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &view_1_proposer)
            .expect("view-1 proposer index");
        let timeout_certificate_file = recovery_dir.join("timeout-certificate.json");
        aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
            data_dir: fixture.data_dirs[view_1_index].clone(),
            verify_block_log: false,
            block_height: 1,
            view: 0,
            vote_files,
            certificate_file: timeout_certificate_file.clone(),
        })
        .expect("timeout certificate");
        let round_dir = recovery_dir.join("round");
        let (result, _, _) = run_vote_phase_round_at(
            &fixture,
            &RoundSlot {
                proposer_index: view_1_index,
                view: 1,
                timeout_certificate_file: Some(timeout_certificate_file),
                artifact_dir: round_dir.clone(),
                batch_file: None,
            },
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            false,
            PEER_VOTE_PHASE_DEADLINE_MS,
            Duration::from_secs(30),
        );
        let report = result
            .expect("view-1 round still running")
            .expect("view 1 certifies height 1");
        assert!(report.round_ok, "{:?}", report.send_failures);
        let elapsed = start.elapsed();

        // One block at height 1 on every validator: the view-1 block.
        let certificate: BlockCertificateFile = read_transport_json_file(
            &round_dir.join("block-certificate.json"),
            "view-1 certificate",
        )
        .expect("view-1 certificate");
        assert_eq!((certificate.view, certificate.proposer.as_str()), (1, view_1_proposer.as_str()));
        assert_ne!(certificate.proposal_hash.as_ref(), crashed_lock.first());
        let tips = fixture
            .data_dirs
            .iter()
            .map(|data_dir| {
                let tip = status(NodeOptions {
                    data_dir: data_dir.clone(),
                })
                .expect("validator status");
                (tip.block_height, tip.block_tip_hash)
            })
            .collect::<Vec<_>>();
        assert!(
            tips.iter().all(|tip| tip == &tips[0]) && tips[0].0 == 1,
            "{tips:?}"
        );
        // The failed proposer's view-0 record is untouched: it never signed
        // a second proposal there.
        assert_eq!(vote_lock_proposal_hashes(&proposer_dir, 1, 0), crashed_lock);
        eprintln!("spent view 0: error=`{error}` recovered_at_view_1_in={elapsed:?}");
        assert!(elapsed < Duration::from_secs(30), "recovery took {elapsed:?}");
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// Early-quorum round (the live finality path) with one validator answering
    /// `late_phase` (0 prepare, 1 precommit) 8 s late and the others at once.
    /// Returns the round time.
    fn round_time_with_one_vote_late(tag: &str, late_phase: Option<usize>) -> Duration {
        let delay = |phase| Duration::from_secs(if late_phase == Some(phase) { 8 } else { 0 });
        let (prepare, precommit) = (ProxyVote::Forward(delay(0)), ProxyVote::Forward(delay(1)));
        let fixture = vote_phase_round_fixture(tag, &[(1, prepare, precommit)]);
        let (result, elapsed, lock_free) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(40),
        );
        let report = result
            .expect("round still running")
            .expect("round certifies");
        eprintln!(
            "{tag}: round_time={elapsed:?} lock_free={lock_free} timings={}",
            serde_json::to_string(&report.timings).expect("round timings json")
        );
        assert!(report.round_ok, "{:?}", report.send_failures);
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
        elapsed
    }

    /// One signed transfer through rpc-serve's finality path with the live
    /// finality flags (30 s timeout, 16 retries, 250 ms backoff, early quorum,
    /// deliveries deferred), six validators over real transport. Reports the
    /// RPC's steps around the certified round and the round's own timers.
    #[test]
    fn finality_rpc_reports_each_step_around_the_certified_round() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = vote_phase_round_fixture("finality-rpc-steps", &[]);
        let proposer_dir = fixture.data_dirs[fixture.proposer_index].clone();
        let batch = create_transfer_batch(BatchTransferOptions {
            data_dir: proposer_dir.clone(),
            key_file: Some(fixture.data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "f".repeat(40)),
            amount: 1_000_000,
            batch_file: fixture.artifact_dir.with_extension("signed.batch.json"),
        })
        .expect("signed transfer");
        let signed_transfer_json =
            serde_json::to_string(batch.transactions.first().expect("signed transfer"))
                .expect("serialize signed transfer");
        let rpc_listener = TcpListener::bind(("127.0.0.1", 0)).expect("RPC port");
        let rpc_port = rpc_listener.local_addr().expect("RPC address").port();
        let ready_file = fixture.artifact_dir.with_extension("rpc-ready.json");
        let rpc = {
            let data_dir = proposer_dir.clone();
            let topology_file = fixture.topology_file.clone();
            let ready_file = ready_file.clone();
            std::thread::spawn(move || {
                rpc_serve_inner(
                    RpcServeOptions {
                        data_dir: data_dir.clone(),
                        spool_dir: data_dir.join("runtime/rpc-spool"),
                        ready_file,
                        bind_host: "127.0.0.1".to_string(),
                        port: rpc_port,
                        max_requests: 1,
                        timeout_ms: 30_000,
                        child_timeout_ms: 30_000,
                        event_log: None,
                        allow_mempool_submit: false,
                        allow_mempool_submit_finality: true,
                        allow_orchard_batch_create: false,
                        allow_private_action_admission: false,
                        owned_lane_enabled: true,
                        finality_topology_file: topology_file,
                        finality_key_file: data_dir.join(VALIDATOR_KEYS_FILE),
                        finality_proposal_key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
                        finality_signer_socket: None,
                        finality_artifact_root: data_dir.join("finality-artifacts"),
                        finality_timeout_ms: 30_000,
                        finality_send_retries: 16,
                        finality_retry_backoff_ms: 250,
                        finality_vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
                        finality_quorum_early_full_propagation: true,
                        max_mempool_submit_per_peer: 4,
                        max_mempool_submit_total: 8,
                        max_orchard_batch_create_per_peer: 1,
                        max_orchard_batch_create_total: 1,
                        max_orchard_batch_create_concurrent: 1,
                        max_child_dispatch_concurrent: 8,
                        max_child_dispatch_per_peer: 4,
                        keep_alive: false,
                    },
                    Some(rpc_listener),
                )
                .expect("serve finality RPC")
            })
        };
        for _ in 0..500 {
            if ready_file.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready_file.is_file(), "finality RPC did not become ready");
        let parent = status(NodeOptions {
            data_dir: proposer_dir.clone(),
        })
        .expect("parent status");
        let request = RpcRequest::empty("finality-rpc-steps", "mempool_submit_signed_transfer_finality")
            .with_param("signed_transfer_json", &signed_transfer_json)
            .expect("signed transfer param")
            .with_param("proxy_required_current_height", parent.block_height)
            .expect("required height")
            .with_param("proxy_required_parent_hash", &parent.block_tip_hash)
            .expect("required hash")
            .with_param("proxy_required_state_root", &parent.state_root)
            .expect("required root");
        let start = Instant::now();
        let response = send_consensus_v2_test_rpc(rpc_port, &request);
        let reply_ms = start.elapsed().as_secs_f64() * 1_000.0;
        assert!(response.ok, "finality RPC failed: {:?}", response.error);
        let result = response.result.expect("finality result");
        assert_eq!(result["finality"]["confirmed"], serde_json::Value::Bool(true));
        assert_eq!(result["certified_sends_deferred"], serde_json::Value::Bool(true));
        let steps = &result["step_timings"];
        for step in ["view_and_proposer_ms", "mempool_round_ms", "round_report_write_ms"] {
            assert!(steps[step].as_f64().is_some(), "missing {step}: {result}");
        }
        // The mempool round takes its node id from the round's post-apply
        // status instead of reading the full status again.
        assert!(steps.get("post_round_status_ms").is_none(), "{steps}");
        // View preparation and the proposer check reuse the status read that
        // matched the required parent.
        assert_eq!(steps["view_and_proposer_status_reads"], 0, "{steps}");
        let round_report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(result["round_report_file"].as_str().expect("round report path"))
                .expect("read round report"),
        )
        .expect("round report json");
        eprintln!(
            "finality rpc steps: client_reply_ms={reply_ms:.1} total_ms={} readiness_wait_ms={} mempool_submit_ms={} mempool_batch_ms={} certified_round_ms={} steps={steps} round_timings={}",
            result["total_ms"],
            result["readiness_wait_ms"],
            result["mempool_submit_ms"],
            result["mempool_batch_ms"],
            result["certified_round_ms"],
            round_report["round"]["timings"],
        );
        rpc.join().expect("finality RPC thread");
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    #[test]
    fn an_early_quorum_round_with_every_vote_on_time_sets_the_baseline() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let elapsed = round_time_with_one_vote_late("on-time-votes", None);
        assert!(elapsed < Duration::from_secs(8), "round took {elapsed:?}");
    }

    #[test]
    fn a_precommit_answered_8s_late_does_not_delay_an_early_quorum_round() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let elapsed = round_time_with_one_vote_late("late-precommit-8s", Some(1));
        assert!(elapsed < Duration::from_secs(8), "round took {elapsed:?}");
    }

    #[test]
    fn a_prepare_answered_8s_late_does_not_delay_an_early_quorum_round() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let elapsed = round_time_with_one_vote_late("late-prepare-8s", Some(0));
        assert!(elapsed < Duration::from_secs(8), "round took {elapsed:?}");
    }

    #[test]
    fn outstanding_precommit_requests_stop_once_the_round_certifies() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fixture = vote_phase_round_fixture(
            "precommit-stop",
            &[(
                1,
                ProxyVote::Forward(Duration::ZERO),
                ProxyVote::Drop(Duration::from_millis(300)),
            )],
        );
        let late_index = fixture.proxied[0].0;
        let (result, elapsed, _) = run_vote_phase_round(
            &fixture,
            UNIT_TIMEOUT_MS,
            UNIT_SEND_RETRIES,
            UNIT_RETRY_BACKOFF_MS,
            true,
            Duration::from_secs(30),
        );
        let report = result
            .expect("round still running")
            .expect("round certifies at precommit quorum");
        assert!(report.round_ok, "{:?}", report.send_failures);
        let precommit_voters = certificate_qc_voters(
            &fixture.artifact_dir.join("block-certificate.json"),
            "precommit_qc",
        );
        assert!(precommit_voters.len() >= fixture.quorum);
        let at_certification = fixture.requests(late_index, 1);
        std::thread::sleep(Duration::from_secs(5));
        let after = fixture.requests(late_index, 1);
        eprintln!(
            "precommit stop: elapsed={elapsed:?} precommit_requests_at_certification={at_certification} after_5s={after}"
        );
        // One attempt in flight at certification may still finish (pooled
        // stream plus its one-shot fallback); nothing new may start.
        assert!(
            after <= at_certification + 2,
            "precommit requests kept retrying after certification: {at_certification} -> {after}"
        );
        clear_transport_vote_stream_pool_for_test(&fixture.topology_file).expect("close persistent streams");
    }

    /// Height 1453 (2026-11-18): the proposer held a precommit quorum (5 of 6)
    /// but waited on one late validator that refused precommit, holding the
    /// mempool mutation lock for the whole wait.
    #[test]
    fn height_1453_round_certifies_at_precommit_quorum_without_late_refusing_peer() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        prewarm_shielded_verifier_cache("height 1453 stall test").expect("prewarm verifier");
        let validator_count = 6usize;
        // The incident's transport timeout was 900 s; 60 s keeps the failing
        // run short while staying far above the late validator's hold.
        let transport_timeout_ms = 60_000u64;
        let precommit_hold = Duration::from_secs(20);
        // The late prepare vote is held until the round has returned, or for
        // at most this long: a round that waits for it returns only after it
        // was passed on, on a runner of any speed. The bound is 5 times the
        // slowest round seen (6 s on a hosted runner).
        let late_prepare_hold = Duration::from_secs(30);
        let late_prepare_gate = std::sync::Arc::new(LateReplyGate::default());
        let root = std::env::temp_dir().join(format!(
            "postfiat-height-1453-stall-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let data_dirs = (0..validator_count)
            .map(|index| {
                let data_dir = root.join(format!("validator-{index}"));
                init_consensus_v2(InitConsensusV2Options {
                    data_dir: data_dir.clone(),
                    chain_id: "postfiat-height-1453-stall-test".to_string(),
                    node_id: format!("validator-{index}"),
                    validator_count: validator_count as u32,
                    activation_height: 1,
                    storage_activation_height: None,
                })
                .expect("init validator");
                data_dir
            })
            .collect::<Vec<_>>();
        let shared_keys: ValidatorKeyFile = serde_json::from_slice(
            &std::fs::read(data_dirs[0].join(VALIDATOR_KEYS_FILE)).expect("read keys"),
        )
        .expect("parse keys");
        let bootstrap_snapshot = root.join("bootstrap-snapshot");
        export_snapshot(SnapshotExportOptions {
            data_dir: data_dirs[0].clone(),
            snapshot_dir: bootstrap_snapshot.clone(),
        })
        .expect("export common state");
        for (index, data_dir) in data_dirs.iter().enumerate().skip(1) {
            std::fs::remove_dir_all(data_dir).expect("clear initialized replica");
            import_snapshot(SnapshotImportOptions {
                data_dir: data_dir.clone(),
                snapshot_dir: bootstrap_snapshot.clone(),
                node_id: Some(format!("validator-{index}")),
            })
            .expect("import common state");
        }
        for (index, data_dir) in data_dirs.iter().enumerate() {
            let record = shared_keys
                .validators
                .iter()
                .find(|record| record.node_id == format!("validator-{index}"))
                .expect("local validator key")
                .clone();
            let key_path = data_dir.join(VALIDATOR_KEYS_FILE);
            std::fs::write(
                &key_path,
                serde_json::to_vec_pretty(&ValidatorKeyFile {
                    validators: vec![record],
                })
                .expect("serialize isolated key"),
            )
            .expect("write isolated key");
            #[cfg(unix)]
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                .expect("protect isolated key");
        }
        let topology_file = root.join("topology.json");
        let (base_port, mut listeners, _rpc_listeners) =
            reserve_consensus_v2_test_ports(validator_count);
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: "postfiat-height-1453-stall-test".to_string(),
            validators: validator_count as u32,
            base_port,
            rpc_base_port: Some(base_port.saturating_add(100)),
            hosts: Some(vec!["127.0.0.1".to_string(); validator_count]),
            output_file: topology_file.clone(),
            activation_height: 1,
            storage_activation_height: None,
        })
        .expect("write topology");
        let (_, validators) = live_consensus_v2_context(&data_dirs[0]).expect("context");
        let validator_ids = validators.validator_ids();
        let proposer =
            postfiat_ordering_fast::leader_for_view(&validator_ids, 1, 0).expect("height-1 leader");
        let proposer_index = validator_ids
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let late_index = (proposer_index + 1) % validator_count;
        let late_validator = format!("validator-{late_index}");

        // The late validator's real service listens on a private port; its
        // topology port is served by the late/refusing proxy.
        let late_upstream = TcpListener::bind(("127.0.0.1", 0)).expect("late upstream port");
        let late_upstream_addr = late_upstream.local_addr().expect("late upstream addr");
        let late_topology_listener = std::mem::replace(&mut listeners[late_index], late_upstream);
        let precommit_requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        spawn_late_refusing_validator_proxy(
            late_topology_listener,
            late_upstream_addr,
            std::sync::Arc::clone(&late_prepare_gate),
            late_prepare_hold,
            precommit_hold,
            std::sync::Arc::clone(&precommit_requests),
        );
        // Detached services: the late validator's connection count depends on
        // how many vote streams the proposer opens, so none is joined.
        for (index, data_dir) in data_dirs.iter().enumerate() {
            if index == proposer_index {
                continue;
            }
            let data_dir = data_dir.clone();
            let topology_file = topology_file.clone();
            let listener = listeners[index]
                .try_clone()
                .expect("clone reserved listener");
            std::thread::spawn(move || {
                let shutdown_requested = std::sync::atomic::AtomicBool::new(false);
                crate::transport_runtime::transport_validator_serve_inner(
                    data_dir.clone(),
                    topology_file,
                    data_dir.join(VALIDATOR_KEYS_FILE),
                    data_dir.join("height-1453-transport-votes"),
                    Some("127.0.0.1".to_string()),
                    16,
                    transport_timeout_ms,
                    None,
                    true,
                    Some(TransportShieldedVerifierPrewarmReport {
                        schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
                        requested: true,
                        total_ms: 0.0,
                        asset_orchard_swap_verifier_warm: true,
                        asset_orchard_swap_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_warm: true,
                        asset_orchard_private_egress_verifier_ms: Some(0.0),
                        asset_orchard_private_egress_verifier_breakdown: None,
                    }),
                    Some(listener),
                    &shutdown_requested,
                )
            });
        }

        let proposer_dir = data_dirs[proposer_index].clone();
        let batch_file = root.join("height-1.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: proposer_dir.clone(),
            key_file: Some(data_dirs[0].join("faucet_key.json")),
            to: format!("pf{}", "d".repeat(40)),
            amount: 1_000_000,
            batch_file: batch_file.clone(),
        })
        .expect("create batch");
        let topology = read_topology_file(&topology_file).expect("read topology");
        let local_status = status(NodeOptions {
            data_dir: proposer_dir.clone(),
        })
        .expect("proposer status");
        transport_authenticated_peer_health(&proposer_dir, &topology, &local_status, 15_000)
            .expect("authenticated peer health");

        // The RPC finality path runs the round under the mempool mutation lock
        // (run_serialized_rpc_mempool_submit); model it with the same try_lock.
        let mempool_mutation_lock = std::sync::Arc::new(Mutex::new(()));
        let artifact_dir = root.join("height-1-artifacts");
        let round_start = Instant::now();
        let round = {
            let lock = std::sync::Arc::clone(&mempool_mutation_lock);
            let options = TransportPeerCertifiedBatchRoundOptions {
                data_dir: proposer_dir.clone(),
                topology_file: topology_file.clone(),
                batch_kind: Some("transparent".to_string()),
                batch_file: batch_file.clone(),
                key_file: proposer_dir.join(VALIDATOR_KEYS_FILE),
                proposal_key_file: Some(proposer_dir.join(VALIDATOR_KEYS_FILE)),
                require_local_proposer: true,
                require_signed_proposal: true,
                allow_peer_failures: false,
                quorum_early_full_propagation: true,
                artifact_dir: artifact_dir.clone(),
                block_height: Some(1),
                view: Some(0),
                timeout_certificate_file: None,
                timeout_ms: transport_timeout_ms,
                send_retries: 1,
                retry_backoff_ms: 50,
                // As at height 1453, no phase deadline ends the wait before
                // the late validator answers: only the quorum may end it.
                vote_phase_deadline_ms: transport_timeout_ms,
                local_apply_before_certified_send: true,
                defer_certified_sends: false,
                commit_processed_dir: None,
                required_parent: None,
            };
            std::thread::spawn(move || {
                let _mutation_guard = lock.try_lock().expect("mempool mutation lock free");
                transport_peer_certified_batch_round(options)
            })
        };
        let mut busy_until_round_end = false;
        while !round.is_finished() && round_start.elapsed() < late_prepare_hold * 4 {
            busy_until_round_end |= matches!(
                mempool_mutation_lock.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        let mempool_lock_free_after = round_start.elapsed();
        let mempool_lock_free = mempool_mutation_lock.try_lock().is_ok();
        let report = round
            .join()
            .expect("round thread")
            .expect("height 1453 round must certify at quorum");
        let round_elapsed = round_start.elapsed();
        let late_prepare_replies_before_return = late_prepare_gate.passed_on();
        let precommit_requests_before_return =
            precommit_requests.load(std::sync::atomic::Ordering::SeqCst);
        late_prepare_gate.open();
        eprintln!(
            "height-1453 repro: round_elapsed={round_elapsed:?} mempool_lock_free_after={mempool_lock_free_after:?} \
             late_prepare_replies_before_return={late_prepare_replies_before_return} \
             precommit_requests_to_late_peer={precommit_requests_before_return} busy_seen={busy_until_round_end} \
             certified_and_applied_ms={:.0}",
            report.timings.client_visible_finality_ms
        );
        // The late validator's prepare reply was still held when the round
        // returned, so the round did not wait for it; nor did it ask the late
        // validator for a precommit, which needs that prepare vote.
        assert_eq!(
            late_prepare_replies_before_return, 0,
            "round took {round_elapsed:?}: it waited on the late peer instead of certifying at precommit quorum"
        );
        assert_eq!(precommit_requests_before_return, 0);
        assert!(
            mempool_lock_free,
            "mempool mutation lock still held {mempool_lock_free_after:?} into the round"
        );
        assert!(
            report.round_ok,
            "round_ok false: {:?}",
            report.send_failures
        );

        let certificate_file = artifact_dir.join("block-certificate.json");
        let prepare_voters = certificate_qc_voters(&certificate_file, "prepare_qc");
        let precommit_voters = certificate_qc_voters(&certificate_file, "precommit_qc");
        assert!(prepare_voters.len() >= validators.quorum);
        assert!(precommit_voters.len() >= validators.quorum);
        assert!(
            !precommit_voters.contains(&late_validator),
            "late validator was counted in the precommit QC: {precommit_voters:?}"
        );

        // The late validator still receives the certified block through the
        // existing certified delivery path.
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let late = status(NodeOptions {
                data_dir: data_dirs[late_index].clone(),
            })
            .expect("late validator status");
            if late.block_height == 1 {
                let proposer_state = status(NodeOptions {
                    data_dir: proposer_dir.clone(),
                })
                .expect("proposer status");
                assert_eq!(late.block_tip_hash, proposer_state.block_tip_hash);
                assert_eq!(late.state_root, proposer_state.state_root);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "late validator never received block 1"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        clear_transport_vote_stream_pool_for_test(&topology_file).expect("close persistent streams");
    }

    #[test]
    fn certified_transport_payload_commits_batch_kind() {
        let payload_json = r#"{"schema":"test-batch","batch_id":"batch-1"}"#;
        let certificate_json = r#"{"schema":"test-certificate","certificate_id":"cert-1"}"#;

        let framed =
            transport_batch_frame_payload(payload_json, Some(certificate_json), "shielded")
                .expect("frame certified payload");
        let payload: serde_json::Value =
            serde_json::from_slice(&framed).expect("decode framed payload");

        assert_eq!(
            payload.get("schema").and_then(serde_json::Value::as_str),
            Some(TRANSPORT_CERTIFIED_BATCH_PAYLOAD_SCHEMA)
        );
        assert_eq!(
            payload
                .get("batch_kind")
                .and_then(serde_json::Value::as_str),
            Some("shielded")
        );
        assert_eq!(
            payload
                .get("payload_json")
                .and_then(serde_json::Value::as_str),
            Some(payload_json)
        );
        assert_eq!(
            payload
                .get("certificate_json")
                .and_then(serde_json::Value::as_str),
            Some(certificate_json)
        );
    }

    #[test]
    fn uncertified_transport_payload_only_allows_transparent_batches() {
        let payload_json = r#"{"schema":"test-batch","batch_id":"batch-1"}"#;

        let transparent =
            transport_batch_frame_payload(payload_json, None, "transparent").expect("frame");
        assert_eq!(transparent, payload_json.as_bytes());

        let error = transport_batch_frame_payload(payload_json, None, "shielded")
            .expect_err("reject uncertified shielded transport");
        assert!(error.contains("uncertified transport batch kind `shielded`"));
    }

    #[test]
    fn plaintext_transport_bind_guard_allows_only_private_hosts_by_default() {
        for host in [
            "127.0.0.1",
            "10.0.0.5",
            "172.16.0.1",
            "192.168.1.2",
            "localhost",
            "::1",
            "fc00::1",
            "fe80::1",
        ] {
            assert!(
                is_private_transport_bind_host(host),
                "{host} should be private"
            );
        }

        for host in ["0.0.0.0", "::", "192.0.2.53", "validator.example.com"] {
            validate_controlled_transport_bind_host_with_override(host, true)
                .expect_err("legacy public-bind opt-in must not bypass the plaintext guard");
        }

        for host in [
            "0.0.0.0",
            "::",
            "*",
            "192.0.2.53",
            "2001:4860:4860::8888",
            "validator.example.com",
        ] {
            assert!(
                !is_private_transport_bind_host(host),
                "{host} must not be accepted as a direct plaintext bind"
            );
        }
    }

    #[test]
    fn validator_service_requires_explicit_plaintext_file_signer_acknowledgement() {
        let error = require_unsafe_devnet_file_signer(&[], "validator service")
            .expect_err("implicit plaintext file-signing service must fail closed");
        assert!(error.contains("--unsafe-devnet-file-signer"), "{error}");
        require_unsafe_devnet_file_signer(
            &["--unsafe-devnet-file-signer".to_string()],
            "validator service",
        )
        .expect("controlled-devnet acknowledgement");
    }

    #[test]
    fn long_running_validator_service_requires_explicit_json_storage_acknowledgement() {
        for command in [
            "transport-validator-serve",
            "transport-block-vote-listen",
        ] {
            let error = run_cli_group_01(
                command,
                &["--unsafe-devnet-file-signer".to_string()],
            )
            .expect_err("implicit JSON consensus storage service must fail closed");
            assert!(
                error.contains("--unsafe-devnet-json-storage"),
                "{command}: {error}"
            );
        }
        require_transactional_or_unsafe_devnet_json_storage(
            &["--unsafe-devnet-json-storage".to_string()],
            "validator service",
        )
        .expect("controlled-devnet storage acknowledgement");

        for command in [
            "rpc-serve",
            "run",
            "transport-certified-batch-loop",
            "transport-peer-certified-batch-loop",
            "transport-peer-certified-private-egress-loop",
        ] {
            let error = run_cli_group_03(command, &[])
                .expect_err("implicit JSON-backed long-running service must fail closed");
            assert!(
                error.contains("--unsafe-devnet-json-storage"),
                "{command}: {error}"
            );
        }
    }

    #[test]
    fn rpc_serve_vote_phase_deadline_is_positive_and_at_most_the_finality_timeout() {
        // A later invalid flag stops rpc-serve right after the deadline is
        // parsed, so an accepted deadline reports that flag instead.
        let parse = |finality_timeout_ms: &str, deadline_ms: Option<&str>| {
            let mut flags = vec![
                "--unsafe-devnet-json-storage",
                "--port",
                "1",
                "--finality-timeout-ms",
                finality_timeout_ms,
            ];
            if let Some(deadline_ms) = deadline_ms {
                flags.extend(["--finality-vote-phase-deadline-ms", deadline_ms]);
            }
            flags.extend(["--max-mempool-submit-per-peer", "0"]);
            let flags = flags.into_iter().map(str::to_string).collect::<Vec<_>>();
            run_cli_group_03("rpc-serve", &flags).expect_err("rpc-serve must not start")
        };
        let accepted = |error: String| error.contains("--max-mempool-submit-per-peer");
        assert!(accepted(parse("30000", None)), "default");
        assert!(accepted(parse("5000", None)), "default above a short finality timeout");
        assert!(accepted(parse("30000", Some("20000"))));
        assert!(accepted(parse("30000", Some("30000"))));
        for deadline_ms in ["0", "-1", "20s", "30001", "600000"] {
            let error = parse("30000", Some(deadline_ms));
            assert!(
                error.contains("--finality-vote-phase-deadline-ms"),
                "{deadline_ms}: {error}"
            );
        }
    }

    #[test]
    fn verified_transactional_storage_does_not_require_legacy_acknowledgement() {
        let root = std::env::temp_dir().join(format!(
            "postfiat-transactional-service-guard-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        init_consensus_v2(InitConsensusV2Options {
            data_dir: root.clone(),
            chain_id: "postfiat-transactional-service-guard-test".to_owned(),
            node_id: "validator-0".to_owned(),
            validator_count: 1,
            activation_height: 1,
            storage_activation_height: Some(1),
        })
        .expect("initialize verified transactional generation");

        require_transactional_or_unsafe_devnet_json_storage(
            &[
                "--data-dir".to_owned(),
                root.to_string_lossy().into_owned(),
            ],
            "validator service",
        )
        .expect("transactional service must not require the legacy acknowledgement");

        std::fs::remove_dir_all(root)
            .expect("remove transactional service guard fixture");
    }

    #[test]
    fn peer_certified_loop_start_height_supports_restart_safe_auto() {
        assert_eq!(
            resolve_peer_certified_start_height_value(None, 482).expect("default auto height"),
            482
        );
        assert_eq!(
            resolve_peer_certified_start_height_value(Some("auto"), 482)
                .expect("explicit auto height"),
            482
        );
        assert_eq!(
            resolve_peer_certified_start_height_value(Some("900"), 482)
                .expect("explicit numeric height"),
            900
        );
        let error = resolve_peer_certified_start_height_value(Some("tip"), 482)
            .expect_err("unknown start-height token must fail closed");
        assert!(error.contains("u64 or `auto`"), "{error}");
    }

    #[test]
    fn rpc_fastpay_is_enabled_by_default_and_only_exactly_disabled() {
        assert!(rpc_owned_lane_enabled(&[]));
        assert!(rpc_owned_lane_enabled(&[
            "--disable-owned-lane=false".to_string()
        ]));
        assert!(!rpc_owned_lane_enabled(&[
            "--disable-owned-lane".to_string()
        ]));
    }

    #[test]
    fn block_vote_request_commits_to_height_and_view() {
        let topology = postfiat_network::local_topology(
            NetworkDomain {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: "a".repeat(96),
                protocol_version: 1,
            },
            2,
            42_000,
        )
        .expect("topology");
        let local_status = StatusReport {
            chain_id: topology.chain_id.clone(),
            genesis_hash: topology.genesis_hash.clone(),
            protocol_version: topology.protocol_version,
            rpc_schema: "postfiat-local-rpc-v1".to_string(),
            build_git_revision: "test-revision".to_string(),
            build_profile: "test".to_string(),
            active_nav_profiles: Vec::new(),
            deployment_manifest_sha256: None,
            deployment_manifest_verified: false,
            deployment_validator_id: None,
            deployment_service_artifacts: Vec::new(),
            deployment_runtime_artifacts: None,
            validator_count: 2,
            node_id: "validator-1".to_string(),
            status: "ok".to_string(),
            last_run_unix: 1,
            state_root: "state-root".to_string(),
            block_height: 6,
            block_tip_hash: "tip".to_string(),
            mempool_pending: 0,
            storage: None,
        };
        let batch_json = r#"{"schema":"test-batch","batch_id":"batch-1"}"#;
        let proposal_json = serde_json::json!({
            "schema": "postfiat.block_proposal.v1",
            "chain_id": topology.chain_id,
            "genesis_hash": topology.genesis_hash,
            "protocol_version": topology.protocol_version,
            "block_height": 7,
            "view": 2,
            "parent_hash": "tip",
            "proposer": "validator-1",
            "batch_kind": "transparent",
            "batch_id": "batch-1",
            "payload_hash": "payload",
            "state_root": "state",
            "receipt_count": 0,
            "receipt_ids": []
        })
        .to_string();
        let payload = transport_block_vote_request_payload(
            7,
            2,
            "transparent",
            batch_json,
            &proposal_json,
            None,
            None,
        )
        .expect("request payload");
        let domain = network_domain_from_topology(&topology);
        let frame = frame_message(
            &domain,
            "validator-0".to_string(),
            Some("validator-1".to_string()),
            TRANSPORT_BLOCK_VOTE_TOPIC,
            &payload,
        )
        .expect("frame");
        let envelope = TransportBlockVoteRequestEnvelope {
            schema: TRANSPORT_BLOCK_VOTE_REQUEST_SCHEMA.to_string(),
            topology_id: topology.topology_id.clone(),
            frame,
            auth: None,
            block_height: 7,
            view: 2,
            batch_kind: "transparent".to_string(),
            batch_json: batch_json.to_string(),
            proposal_json,
            timeout_certificate_json: None,
            consensus_v2: None,
        };

        validate_transport_block_vote_request(&envelope, &topology, &local_status)
            .expect("valid request");

        let timeout_certificate_json = serde_json::json!({
            "schema": "postfiat.block_timeout_certificate.v1",
            "chain_id": topology.chain_id,
            "genesis_hash": topology.genesis_hash,
            "protocol_version": topology.protocol_version,
            "block_height": 7,
            "view": 1,
            "certificate_id": "timeout-cert-view-1"
        })
        .to_string();
        let timeout_payload = transport_block_vote_request_payload(
            7,
            2,
            "transparent",
            batch_json,
            &envelope.proposal_json,
            Some(&timeout_certificate_json),
            None,
        )
        .expect("request payload with timeout certificate");
        let timeout_frame = frame_message(
            &domain,
            "validator-0".to_string(),
            Some("validator-1".to_string()),
            TRANSPORT_BLOCK_VOTE_TOPIC,
            &timeout_payload,
        )
        .expect("timeout frame");
        let envelope_with_timeout = TransportBlockVoteRequestEnvelope {
            frame: timeout_frame,
            timeout_certificate_json: Some(timeout_certificate_json),
            ..envelope.clone()
        };
        validate_transport_block_vote_request(&envelope_with_timeout, &topology, &local_status)
            .expect("valid request with timeout certificate");
        let mut missing_timeout = envelope_with_timeout.clone();
        missing_timeout.timeout_certificate_json = None;
        let timeout_error =
            validate_transport_block_vote_request(&missing_timeout, &topology, &local_status)
                .expect_err("timeout certificate omission must break payload hash");
        assert!(
            timeout_error.contains("payload hash or message id mismatch"),
            "{timeout_error}"
        );

        validate_signed_proposal_policy(&envelope.proposal_json, false)
            .expect("unsigned proposal allowed without policy");
        let policy_error = validate_signed_proposal_policy(&envelope.proposal_json, true)
            .expect_err("unsigned proposal rejected by policy");
        assert!(
            policy_error.contains("requires signed proposal"),
            "{policy_error}"
        );

        let signed_proposal_json = serde_json::json!({
            "schema": "postfiat.block_proposal.v1",
            "chain_id": topology.chain_id,
            "genesis_hash": topology.genesis_hash,
            "protocol_version": topology.protocol_version,
            "block_height": 7,
            "view": 2,
            "parent_hash": "tip",
            "proposer": "validator-1",
            "batch_kind": "transparent",
            "batch_id": "batch-1",
            "payload_hash": "payload",
            "state_root": "state",
            "receipt_count": 0,
            "receipt_ids": [],
            "signature": {
                "signer": "validator-1"
            }
        })
        .to_string();
        validate_signed_proposal_policy(&signed_proposal_json, true)
            .expect("signed proposal satisfies policy");

        let mut wrong_view = envelope;
        wrong_view.view = 1;
        let error = validate_transport_block_vote_request(&wrong_view, &topology, &local_status)
            .expect_err("view mismatch");
        assert!(error.contains("proposal view 2 does not match envelope view 1"));
    }

    #[test]
    fn block_vote_response_reports_validator_rejection() {
        let rejection = serde_json::json!({
            "schema": "postfiat-transport-validator-serve-rejection-v1",
            "node_id": "validator-2",
            "topology_id": "topology-1",
            "connection_index": 7,
            "kind": "block_vote_request",
            "error": "transport block vote signing failed: conflicting block proposal vote already recorded",
            "state": {
                "schema": TRANSPORT_HELLO_SCHEMA,
                "node_id": "validator-2",
                "topology_id": "topology-1",
                "chain_id": "postfiat-local",
                "genesis_hash": "a",
                "protocol_version": 1,
                "state_root": "state",
                "block_height": 6,
                "block_tip_hash": "tip"
            }
        })
        .to_string();

        let error = parse_transport_block_vote_response(&rejection)
            .expect_err("validator rejection should be surfaced");
        assert!(
            error.contains("transport block vote rejected by `validator-2`"),
            "{error}"
        );
        assert!(
            error.contains("conflicting block proposal vote already recorded"),
            "{error}"
        );
    }
}
