#[cfg(test)]
mod transport_batch_payload_tests {
    use super::*;
    use std::io;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    static CONSENSUS_V2_TRANSPORT_TEST_LOCK: Mutex<()> = Mutex::new(());

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

    fn consensus_v2_test_base_port(validator_count: usize) -> u16 {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .subsec_nanos() as u16;
        for offset in 0..512u16 {
            let base = 30_000u16 + ((seed.wrapping_add(offset * 4)) % 20_000);
            let rpc_base = base.saturating_add(100);
            if (0..validator_count).all(|index| {
                let index = index as u16;
                TcpListener::bind(("127.0.0.1", base.saturating_add(index * 2))).is_ok()
                    && TcpListener::bind(("127.0.0.1", rpc_base.saturating_add(index))).is_ok()
            }) {
                return base;
            }
        }
        panic!("could not find four local transport ports");
    }

    fn start_consensus_v2_test_servers(
        data_dirs: &[PathBuf],
        topology_file: &Path,
        proposer_index: usize,
        round_label: &str,
    ) -> Vec<std::thread::JoinHandle<Result<TransportValidatorServeReport, String>>> {
        let mut handles = Vec::new();
        for (index, data_dir) in data_dirs.iter().enumerate() {
            if index == proposer_index {
                continue;
            }
            let data_dir = data_dir.clone();
            let topology_file = topology_file.to_path_buf();
            let vote_dir = data_dir.join(format!("{round_label}-transport-votes"));
            let key_file = data_dir.join(VALIDATOR_KEYS_FILE);
            handles.push(std::thread::spawn(move || {
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
                )
            }));
        }
        let topology_path = topology_file.to_path_buf();
        let topology = read_topology_file(&topology_path).expect("read test topology");
        let expected_addresses = topology
            .peers
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != proposer_index)
            .map(|(_, peer)| (peer.host.clone(), peer.p2p_port))
            .collect::<Vec<_>>();
        let mut ready = false;
        for _ in 0..500 {
            ready = expected_addresses.iter().all(|(host, port)| {
                TcpListener::bind((host.as_str(), *port))
                    .is_err_and(|error| error.kind() == io::ErrorKind::AddrInUse)
            });
            if ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !ready {
            let results = handles
                .into_iter()
                .map(|handle| handle.join())
                .collect::<Vec<_>>();
            panic!("validator services did not bind before the test round: {results:?}");
        }
        handles
    }

    fn run_consensus_v2_transport_round(
        data_dirs: &[PathBuf],
        topology_file: &Path,
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
            proposer_index,
            &format!("h{height}-v{view}"),
        );
        let data_dir = data_dirs[proposer_index].clone();
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
            local_apply_before_certified_send: false,
            defer_certified_sends: false,
            required_parent: None,
        })
        .expect("consensus v2 transport round");
        crate::transport_runtime::clear_transport_vote_stream_pool_for_test()
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

    fn run_activated_consensus_v2_transport_failed_proposer(validator_count: usize) {
        prewarm_shielded_verifier_cache("consensus v2 transport test")
            .expect("prewarm verifier");
        let root = std::env::temp_dir().join(format!(
            "postfiat-consensus-v2-transport-{}-{}",
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
                })
                .expect("init validator");
                data_dir
            })
            .collect::<Vec<_>>();
        let shared_keys = std::fs::read(data_dirs[0].join(VALIDATOR_KEYS_FILE))
            .expect("shared validator keys");
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
        let topology_file = root.join("topology.json");
        let base_port = consensus_v2_test_base_port(validator_count);
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: "postfiat-consensus-v2-transport-test".to_string(),
            validators: validator_count as u32,
            base_port,
            rpc_base_port: Some(base_port.saturating_add(100)),
            hosts: Some(vec!["127.0.0.1".to_string(); validator_count]),
            output_file: topology_file.clone(),
            activation_height: 2,
        })
        .expect("write topology");
        let (_, validators) =
            live_consensus_v2_context(&data_dirs[0]).expect("consensus context");

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
        let recovery_rpc = std::thread::spawn(move || {
            rpc_serve(RpcServeOptions {
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
                owned_lane_enabled: true,
                finality_topology_file: recovery_topology,
                finality_key_file: recovery_data_dir.join(VALIDATOR_KEYS_FILE),
                finality_proposal_key_file: Some(recovery_data_dir.join(VALIDATOR_KEYS_FILE)),
                finality_artifact_root: recovery_data_dir.join("finality-artifacts"),
                finality_timeout_ms: 15_000,
                finality_send_retries: 2,
                finality_retry_backoff_ms: 50,
                finality_quorum_early_full_propagation: false,
                max_mempool_submit_per_peer: 4,
                max_mempool_submit_total: 8,
                max_orchard_batch_create_per_peer: 1,
                max_orchard_batch_create_total: 1,
                max_orchard_batch_create_concurrent: 1,
                keep_alive: false,
            })
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
        crate::transport_runtime::clear_transport_vote_stream_pool_for_test()
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
        let key_file: ValidatorKeyFile =
            serde_json::from_slice(&shared_keys).expect("parse shared validator keys");
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
        let rotated_key_json =
            serde_json::to_string_pretty(&rotated_key_file).expect("rotated key file json");
        for data_dir in &data_dirs {
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
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK.lock().expect("test lock");
        run_activated_consensus_v2_transport_failed_proposer(4);
    }

    #[test]
    fn activated_consensus_v2_transport_survives_failed_view_zero_proposer_n6() {
        let _guard = CONSENSUS_V2_TRANSPORT_TEST_LOCK.lock().expect("test lock");
        run_activated_consensus_v2_transport_failed_proposer(6);
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
        require_unsafe_devnet_json_storage(
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
