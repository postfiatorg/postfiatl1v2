fn run_cli_group_01(command: &str, flags: &[String]) -> Result<(), String> {
    match command {
        "init" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let node_id = flag_value(flags, "--node-id").unwrap_or(DEFAULT_NODE_ID);
            let validator_count = flag_value(flags, "--validators")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let report = init(InitOptions {
                data_dir: PathBuf::from(data_dir),
                chain_id: chain_id.to_string(),
                node_id: node_id.to_string(),
                validator_count,
            })
            .map_err(|error| format!("init failed: {error}"))?;
            let json = report
                .to_json()
                .map_err(|error| format!("init report serialization failed: {error}"))?;
            print!("{json}");
            Ok(())
        }
        "init-consensus-v2" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let node_id = flag_value(flags, "--node-id").unwrap_or(DEFAULT_NODE_ID);
            let validator_count = flag_value(flags, "--validators")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let activation_height = flag_value(flags, "--activation-height")
                .ok_or("missing --activation-height")?
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let storage_activation_height = flag_value(flags, "--storage-activation-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--storage-activation-height must be a u64".to_string())
                })
                .transpose()?;
            let report = init_consensus_v2(InitConsensusV2Options {
                data_dir: PathBuf::from(data_dir),
                chain_id: chain_id.to_string(),
                node_id: node_id.to_string(),
                validator_count,
                activation_height,
                storage_activation_height,
            })
            .map_err(|error| format!("init-consensus-v2 failed: {error}"))?;
            let json = report
                .to_json()
                .map_err(|error| format!("init report serialization failed: {error}"))?;
            print!("{json}");
            Ok(())
        }
        "topology" => {
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let hosts = flag_value(flags, "--hosts")
                .map(parse_csv_values)
                .transpose()?;
            let validators = match (flag_value(flags, "--validators"), hosts.as_ref()) {
                (Some(value), _) => value
                    .parse::<u32>()
                    .map_err(|_| "--validators must be a u32".to_string())?,
                (None, Some(hosts)) => u32::try_from(hosts.len())
                    .map_err(|_| "--hosts contains too many validators".to_string())?,
                (None, None) => 4,
            };
            let base_port = flag_value(flags, "--base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--base-port must be a u16".to_string())
                })
                .transpose()?
                .unwrap_or(DEFAULT_BASE_PORT);
            let rpc_base_port = flag_value(flags, "--rpc-base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--rpc-base-port must be a u16".to_string())
                })
                .transpose()?;
            let output_file = flag_value(flags, "--output").unwrap_or(DEFAULT_TOPOLOGY_FILE);
            let topology = write_local_topology(TopologyOptions {
                chain_id: chain_id.to_string(),
                validators,
                base_port,
                rpc_base_port,
                hosts,
                output_file: PathBuf::from(output_file),
            })
            .map_err(|error| format!("topology failed: {error}"))?;
            let json = serde_json::to_string_pretty(&topology)
                .map_err(|error| format!("topology serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "topology-consensus-v2" => {
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let hosts = flag_value(flags, "--hosts")
                .map(parse_csv_values)
                .transpose()?;
            let validators = match (flag_value(flags, "--validators"), hosts.as_ref()) {
                (Some(value), _) => value
                    .parse::<u32>()
                    .map_err(|_| "--validators must be a u32".to_string())?,
                (None, Some(hosts)) => u32::try_from(hosts.len())
                    .map_err(|_| "--hosts contains too many validators".to_string())?,
                (None, None) => 4,
            };
            let base_port = flag_value(flags, "--base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--base-port must be a u16".to_string())
                })
                .transpose()?
                .unwrap_or(DEFAULT_BASE_PORT);
            let rpc_base_port = flag_value(flags, "--rpc-base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--rpc-base-port must be a u16".to_string())
                })
                .transpose()?;
            let activation_height = flag_value(flags, "--activation-height")
                .ok_or("missing --activation-height")?
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let storage_activation_height = flag_value(flags, "--storage-activation-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--storage-activation-height must be a u64".to_string())
                })
                .transpose()?;
            let output_file = flag_value(flags, "--output").unwrap_or(DEFAULT_TOPOLOGY_FILE);
            let topology = write_consensus_v2_topology(TopologyConsensusV2Options {
                chain_id: chain_id.to_string(),
                validators,
                base_port,
                rpc_base_port,
                hosts,
                output_file: PathBuf::from(output_file),
                activation_height,
                storage_activation_height,
            })
            .map_err(|error| format!("topology-consensus-v2 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&topology)
                .map_err(|error| format!("topology serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-listen" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let max_peers = flag_value(flags, "--max-peers")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-peers must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let report = transport_listen(
                data_dir,
                PathBuf::from(topology_file),
                bind_host,
                max_peers,
                timeout_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport listen serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-dial" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let report = transport_dial(
                data_dir,
                PathBuf::from(topology_file),
                to.to_string(),
                timeout_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport dial serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-batch-listen" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let max_peers = flag_value(flags, "--max-peers")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-peers must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let report = transport_batch_listen(
                data_dir,
                PathBuf::from(topology_file),
                bind_host,
                max_peers,
                timeout_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport batch listen serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-batch-serve" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let max_batches = flag_value(flags, "--max-batches")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-batches must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let event_log = flag_value(flags, "--event-log").map(PathBuf::from);
            let report = transport_batch_serve(
                data_dir,
                PathBuf::from(topology_file),
                bind_host,
                max_batches,
                timeout_ms,
                event_log,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport batch serve serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-batch-send" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let certificate_file = flag_value(flags, "--certificate-file").map(PathBuf::from);
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let report = transport_batch_send_with_retries(
                data_dir,
                PathBuf::from(topology_file),
                to.to_string(),
                batch_kind,
                PathBuf::from(batch_file),
                certificate_file,
                timeout_ms,
                send_retries,
                retry_backoff_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport batch send serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-block-vote-listen" => {
            require_unsafe_devnet_file_signer(flags, "transport block-vote listener")?;
            require_transactional_or_unsafe_devnet_json_storage(flags, "transport block-vote listener")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let vote_dir = flag_value(flags, "--vote-dir").ok_or("missing --vote-dir")?;
            let max_requests = flag_value(flags, "--max-requests")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-requests must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let report = transport_block_vote_listen(
                data_dir,
                PathBuf::from(topology_file),
                PathBuf::from(key_file),
                PathBuf::from(vote_dir),
                bind_host,
                max_requests,
                timeout_ms,
                require_signed_proposal,
            )?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport block vote listen serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-validator-serve" => {
            require_unsafe_devnet_file_signer(flags, "transport validator service")?;
            require_transactional_or_unsafe_devnet_json_storage(flags, "transport validator service")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let vote_dir = flag_value(flags, "--vote-dir").ok_or("missing --vote-dir")?;
            let max_connections = flag_value(flags, "--max-connections")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-connections must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let event_log = flag_value(flags, "--event-log").map(PathBuf::from);
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let report = transport_validator_serve(
                data_dir,
                PathBuf::from(topology_file),
                PathBuf::from(key_file),
                PathBuf::from(vote_dir),
                bind_host,
                max_connections,
                timeout_ms,
                event_log,
                require_signed_proposal,
            )?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport validator serve serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-block-vote-request" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let proposal_file =
                flag_value(flags, "--proposal-file").ok_or("missing --proposal-file")?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let vote_file = flag_value(flags, "--vote-file").ok_or("missing --vote-file")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let report = transport_block_vote_request(TransportBlockVoteRequestOptions {
                data_dir,
                topology_file: PathBuf::from(topology_file),
                to: to.to_string(),
                batch_kind,
                batch_file: PathBuf::from(batch_file),
                proposal_file: PathBuf::from(proposal_file),
                timeout_certificate_file,
                vote_file: PathBuf::from(vote_file),
                block_height,
                timeout_ms,
                consensus_v2: None,
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport block vote request serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-certified-batch-round" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let validator_key_dir =
                flag_value(flags, "--validator-key-dir").ok_or("missing --validator-key-dir")?;
            let artifact_dir =
                flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let skip_block_log_verify = flags.contains(&"--skip-block-log-verify".to_string());
            let report = transport_certified_batch_round(TransportCertifiedBatchRoundOptions {
                data_dir,
                topology_file: PathBuf::from(topology_file),
                batch_kind,
                batch_file: PathBuf::from(batch_file),
                validator_key_dir: PathBuf::from(validator_key_dir),
                artifact_dir: PathBuf::from(artifact_dir),
                block_height,
                view,
                timeout_certificate_file,
                timeout_ms,
                send_retries,
                retry_backoff_ms,
                skip_block_log_verify,
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport certified batch round serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-peer-certified-batch-round" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let proposal_key_file = flag_value(flags, "--proposal-key-file")
                .map(PathBuf::from)
                .or_else(|| Some(key_file.clone()));
            let require_local_proposer = flag_present(flags, "--require-local-proposer");
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let allow_peer_failures = flag_present(flags, "--allow-peer-failures");
            let quorum_early_full_propagation =
                flag_present(flags, "--quorum-early-full-propagation");
            let local_apply_before_certified_send =
                flag_present(flags, "--local-apply-before-certified-send");
            let defer_certified_sends = flag_present(flags, "--defer-certified-sends");
            let artifact_dir =
                flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let report =
                transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                    data_dir,
                    topology_file: PathBuf::from(topology_file),
                    batch_kind,
                    batch_file: PathBuf::from(batch_file),
                    key_file,
                    proposal_key_file,
                    require_local_proposer,
                    require_signed_proposal,
                    allow_peer_failures,
                    quorum_early_full_propagation,
                    artifact_dir: PathBuf::from(artifact_dir),
                    block_height,
                    view,
                    timeout_certificate_file,
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    local_apply_before_certified_send,
                    defer_certified_sends,
                    commit_processed_dir: None,
                    required_parent: None,
                })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport peer certified batch round serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-peer-certified-mempool-round" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let proposal_key_file = flag_value(flags, "--proposal-key-file")
                .map(PathBuf::from)
                .or_else(|| Some(key_file.clone()));
            let require_local_proposer = flag_present(flags, "--require-local-proposer");
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let allow_peer_failures = flag_present(flags, "--allow-peer-failures");
            let quorum_early_full_propagation =
                flag_present(flags, "--quorum-early-full-propagation");
            let local_apply_before_certified_send =
                flag_present(flags, "--local-apply-before-certified-send");
            let defer_certified_sends = flag_present(flags, "--defer-certified-sends");
            let artifact_dir =
                flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let max_transactions = flag_value(flags, "--max-transactions")
                .unwrap_or("100")
                .parse::<usize>()
                .map_err(|_| "--max-transactions must be a usize".to_string())?;
            let signed_transfer_file = flag_value(flags, "--signed-transfer-file").map(PathBuf::from);
            let signed_transfer_json =
                flag_value(flags, "--signed-transfer-json").map(str::to_string);
            let signed_asset_transaction_json = flag_value(flags, "--signed-asset-transaction-json")
                .map(str::to_string);
            let report =
                transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
                    data_dir,
                    topology_file: PathBuf::from(topology_file),
                    key_file,
                    proposal_key_file,
                    require_local_proposer,
                    require_signed_proposal,
                    allow_peer_failures,
                    quorum_early_full_propagation,
                    artifact_dir: PathBuf::from(artifact_dir),
                    block_height,
                    view,
                    timeout_certificate_file,
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    local_apply_before_certified_send,
                    defer_certified_sends,
                    max_transactions,
                    signed_transfer_file,
                    signed_transfer_json,
                    signed_payment_v2_json: None,
                    signed_asset_transaction_json,
                    signed_atomic_swap_transaction_json: None,
                    signed_escrow_transaction_json: None,
                    required_parent: None,
                })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport peer certified mempool round serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "pftl-submit-certified-asset-ops" | "submit-certified-asset-ops" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file =
                PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
            let key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let proposal_key_file = flag_value(flags, "--proposal-key-file").map(PathBuf::from);
            let artifact_dir =
                PathBuf::from(flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?);
            let ops_file = match (flag_value(flags, "--ops-file"), flag_value(flags, "--bundle")) {
                (Some(_), Some(_)) => {
                    return Err("use only one of --ops-file or --bundle".to_string());
                }
                (Some(ops_file), None) => PathBuf::from(ops_file),
                (None, Some(bundle_dir)) => {
                    let ops_file = artifact_dir.with_extension("certified-ops.request.json");
                    certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
                        bundle_dir: PathBuf::from(bundle_dir),
                        output_file: ops_file.clone(),
                        proposer_key_file: flag_value(flags, "--proposer-key-file")
                            .map(PathBuf::from),
                        attestor_key_file: flag_value(flags, "--attestor-key-file")
                            .map(PathBuf::from),
                        finalizer_key_file: flag_value(flags, "--finalizer-key-file")
                            .map(PathBuf::from),
                        claimer_key_file: flag_value(flags, "--claimer-key-file").map(PathBuf::from),
                        owner_key_file: flag_value(flags, "--owner-key-file").map(PathBuf::from),
                        include_deposit_claim: !flag_present(flags, "--skip-deposit-claim"),
                        overwrite: flag_present(flags, "--overwrite"),
                    })?;
                    ops_file
                }
                (None, None) => return Err("missing --ops-file or --bundle".to_string()),
            };
            let max_transactions = flag_value(flags, "--max-transactions")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--max-transactions must be a usize".to_string())
                })
                .transpose()?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let report = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
                data_dir,
                topology_file,
                key_file,
                proposal_key_file,
                ops_file,
                artifact_dir,
                max_transactions,
                require_local_proposer: flag_present(flags, "--require-local-proposer"),
                require_signed_proposal: !flag_present(flags, "--allow-unsigned-proposal"),
                allow_peer_failures: flag_present(flags, "--allow-peer-failures"),
                quorum_early_full_propagation: flag_present(
                    flags,
                    "--quorum-early-full-propagation",
                ),
                local_apply_before_certified_send: flag_present(
                    flags,
                    "--local-apply-before-certified-send",
                ),
                defer_certified_sends: flag_present(flags, "--defer-certified-sends"),
                block_height,
                view,
                timeout_certificate_file,
                timeout_ms,
                send_retries,
                retry_backoff_ms,
                allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                resume: flag_present(flags, "--resume"),
                overwrite: flag_present(flags, "--overwrite"),
                prepare_only: flag_present(flags, "--prepare-only"),
                batch_only: flag_present(flags, "--batch-only"),
            })?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("certified asset ops serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "pftl-certified-asset-ops-from-bundle" => {
            let bundle_dir =
                PathBuf::from(flag_value(flags, "--bundle").ok_or("missing --bundle")?);
            let output_file =
                PathBuf::from(flag_value(flags, "--output").ok_or("missing --output")?);
            let report = certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
                bundle_dir,
                output_file,
                proposer_key_file: flag_value(flags, "--proposer-key-file").map(PathBuf::from),
                attestor_key_file: flag_value(flags, "--attestor-key-file").map(PathBuf::from),
                finalizer_key_file: flag_value(flags, "--finalizer-key-file").map(PathBuf::from),
                claimer_key_file: flag_value(flags, "--claimer-key-file").map(PathBuf::from),
                owner_key_file: flag_value(flags, "--owner-key-file").map(PathBuf::from),
                include_deposit_claim: !flag_present(flags, "--skip-deposit-claim"),
                overwrite: flag_present(flags, "--overwrite"),
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("certified asset ops bundle adapter serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        _ => unreachable!("run_cli_group_01 dispatch mismatch"),
    }
}
