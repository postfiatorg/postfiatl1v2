use super::*;

pub(super) struct TransportBatchInboxFiles {
    pub(super) batch_file: PathBuf,
    pub(super) certificate_file: Option<PathBuf>,
}

pub(super) fn transport_listen(
    data_dir: PathBuf,
    topology_file: PathBuf,
    bind_host: Option<String>,
    max_peers: usize,
    timeout_ms: u64,
) -> Result<TransportListenReport, String> {
    if max_peers == 0 {
        return Err("--max-peers must be positive".to_string());
    }
    let topology = read_topology_file(&topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("transport status failed: {error}"))?;
    let local_peer = topology
        .peer(&local_status.node_id)
        .ok_or_else(|| format!("local node `{}` is not in topology", local_status.node_id))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let bind_host = bind_host.unwrap_or_else(|| local_peer.host.clone());
    validate_controlled_transport_bind_host(&bind_host)?;
    let bind_address = socket_address(&bind_host, local_peer.p2p_port);
    let listener = TcpListener::bind(&bind_address)
        .map_err(|error| format!("transport listen bind `{bind_address}` failed: {error}"))?;

    let mut accepted = Vec::with_capacity(max_peers);
    for _ in 0..max_peers {
        let (mut stream, _) = listener
            .accept()
            .map_err(|error| format!("transport accept failed: {error}"))?;
        set_stream_timeout(&stream, timeout_ms)?;
        let line = read_transport_line(&stream, "transport read")?;
        let hello = parse_transport_hello(&line)?;
        validate_transport_hello(&hello, &topology, &local_status)?;
        let response = transport_hello(&topology, &local_status);
        write_transport_hello(&mut stream, &response)?;
        accepted.push(hello);
    }

    Ok(TransportListenReport {
        schema: "postfiat-transport-listen-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        bind_address,
        accepted,
        verified: true,
    })
}

pub(super) fn transport_dial(
    data_dir: PathBuf,
    topology_file: PathBuf,
    to: String,
    timeout_ms: u64,
) -> Result<TransportDialReport, String> {
    let topology = read_topology_file(&topology_file)?;
    let local_status = status(NodeOptions { data_dir })
        .map_err(|error| format!("transport status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let peer = topology
        .peer(&to)
        .ok_or_else(|| format!("target node `{to}` is not in topology"))?;
    let peer_address = socket_address(&peer.host, peer.p2p_port);
    let mut stream = connect_transport_stream(&peer_address, timeout_ms, "transport dial")?;
    set_stream_timeout(&stream, timeout_ms)?;
    let sent = transport_hello(&topology, &local_status);
    write_transport_hello(&mut stream, &sent)?;

    let line = read_transport_line(&stream, "transport response read")?;
    let received = parse_transport_hello(&line)?;
    validate_transport_hello(&received, &topology, &local_status)?;
    if received.node_id != to {
        return Err(format!(
            "transport response came from `{}`, expected `{to}`",
            received.node_id
        ));
    }

    Ok(TransportDialReport {
        schema: "postfiat-transport-dial-v1".to_string(),
        from: local_status.node_id,
        to,
        topology_id: topology.topology_id,
        peer_address,
        sent,
        received,
        verified: true,
    })
}

pub(super) fn transport_batch_listen(
    data_dir: PathBuf,
    topology_file: PathBuf,
    bind_host: Option<String>,
    max_peers: usize,
    timeout_ms: u64,
) -> Result<TransportBatchListenReport, String> {
    if max_peers == 0 {
        return Err("--max-peers must be positive".to_string());
    }
    prewarm_shielded_verifier_cache("transport batch listen")?;
    let topology = read_topology_file(&topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("transport batch status failed: {error}"))?;
    let local_peer = topology
        .peer(&local_status.node_id)
        .ok_or_else(|| format!("local node `{}` is not in topology", local_status.node_id))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let bind_host = bind_host.unwrap_or_else(|| local_peer.host.clone());
    validate_controlled_transport_bind_host(&bind_host)?;
    let bind_address = socket_address(&bind_host, local_peer.p2p_port);
    let listener = TcpListener::bind(&bind_address)
        .map_err(|error| format!("transport batch listen bind `{bind_address}` failed: {error}"))?;

    let mut accepted = Vec::with_capacity(max_peers);
    for _ in 0..max_peers {
        let (mut stream, _) = listener
            .accept()
            .map_err(|error| format!("transport batch accept failed: {error}"))?;
        set_stream_timeout(&stream, timeout_ms)?;
        let line = read_transport_line(&stream, "transport batch read")?;
        let envelope = parse_transport_batch_envelope(&line)?;
        validate_transport_batch_envelope(&envelope, &topology, &local_status)?;
        validate_transport_envelope_auth(
            envelope.auth.as_ref(),
            &data_dir,
            &topology,
            &envelope.frame,
        )?;
        let inbox = write_transport_batch_payload(&data_dir, &envelope)?;
        let receipts = apply_transport_batch(
            &data_dir,
            &envelope.batch_kind,
            inbox.batch_file,
            inbox.certificate_file,
            None,
        )
        .map_err(|error| format!("transport batch apply failed: {error}"))?;
        let state_after = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .map_err(|error| format!("transport batch post-apply status failed: {error}"))?;
        let ack = transport_batch_ack(
            &topology,
            &local_status.node_id,
            &envelope,
            &state_after,
            &receipts,
        );
        write_json_line(&mut stream, &ack)?;
        accepted.push(ack);
    }

    Ok(TransportBatchListenReport {
        schema: "postfiat-transport-batch-listen-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        bind_address,
        accepted,
        verified: true,
    })
}

pub(super) fn transport_batch_serve(
    data_dir: PathBuf,
    topology_file: PathBuf,
    bind_host: Option<String>,
    max_batches: usize,
    timeout_ms: u64,
    event_log: Option<PathBuf>,
) -> Result<TransportBatchServeReport, String> {
    if max_batches == 0 {
        return Err("--max-batches must be positive".to_string());
    }
    prewarm_shielded_verifier_cache("transport batch service")?;
    let topology = read_topology_file(&topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("transport batch service status failed: {error}"))?;
    let local_peer = topology
        .peer(&local_status.node_id)
        .ok_or_else(|| format!("local node `{}` is not in topology", local_status.node_id))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let bind_host = bind_host.unwrap_or_else(|| local_peer.host.clone());
    validate_controlled_transport_bind_host(&bind_host)?;
    let bind_address = socket_address(&bind_host, local_peer.p2p_port);
    let listener = TcpListener::bind(&bind_address).map_err(|error| {
        format!("transport batch service bind `{bind_address}` failed: {error}")
    })?;
    let mut event_writer = event_log
        .as_ref()
        .map(|path| open_transport_event_log(path))
        .transpose()?;

    let mut accepted = Vec::with_capacity(max_batches);
    let mut rejected = Vec::new();
    let mut batch_index = 0_u64;
    while accepted.len() < max_batches {
        batch_index = batch_index.saturating_add(1);
        let (mut stream, _) = listener
            .accept()
            .map_err(|error| format!("transport batch service accept failed: {error}"))?;
        set_stream_timeout(&stream, timeout_ms)?;
        let ack = match handle_transport_batch_service_connection(
            &mut stream,
            &data_dir,
            &topology,
            &local_status,
        ) {
            Ok(ack) => ack,
            Err(error) => {
                let state_after = status(NodeOptions {
                    data_dir: data_dir.clone(),
                })
                .map_err(|error| {
                    format!("transport batch service rejection status failed: {error}")
                })?;
                let rejection = TransportBatchServeRejection {
                    schema: "postfiat-transport-batch-serve-rejection-v1".to_string(),
                    node_id: local_status.node_id.clone(),
                    topology_id: topology.topology_id.clone(),
                    batch_index,
                    error,
                    state: transport_hello(&topology, &state_after),
                };
                if let Some(writer) = event_writer.as_mut() {
                    let event = TransportBatchServeEvent {
                        schema: "postfiat-transport-batch-serve-event-v1".to_string(),
                        node_id: local_status.node_id.clone(),
                        topology_id: topology.topology_id.clone(),
                        batch_index,
                        outcome: "rejected".to_string(),
                        ack: None,
                        rejection: Some(rejection.clone()),
                    };
                    write_event_log_line(writer, &event)?;
                }
                rejected.push(rejection);
                continue;
            }
        };
        if let Some(writer) = event_writer.as_mut() {
            let event = TransportBatchServeEvent {
                schema: "postfiat-transport-batch-serve-event-v1".to_string(),
                node_id: local_status.node_id.clone(),
                topology_id: topology.topology_id.clone(),
                batch_index,
                outcome: "accepted".to_string(),
                ack: Some(ack.clone()),
                rejection: None,
            };
            write_event_log_line(writer, &event)?;
        }
        accepted.push(ack);
    }

    Ok(TransportBatchServeReport {
        schema: "postfiat-transport-batch-serve-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        bind_address,
        event_log: event_log.map(|path| path.display().to_string()),
        accepted_count: accepted.len() as u64,
        rejected_count: rejected.len() as u64,
        accepted,
        rejected,
        verified: true,
    })
}

fn handle_transport_batch_service_connection(
    stream: &mut TcpStream,
    data_dir: &Path,
    topology: &NetworkTopology,
    local_status: &StatusReport,
) -> Result<TransportBatchAck, String> {
    let line = read_transport_line(stream, "transport batch service read")?;
    handle_transport_batch_service_line(stream, data_dir, topology, local_status, &line)
}

fn handle_transport_batch_service_line(
    stream: &mut TcpStream,
    data_dir: &Path,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    line: &str,
) -> Result<TransportBatchAck, String> {
    let envelope = parse_transport_batch_envelope(line)?;
    validate_transport_batch_envelope(&envelope, topology, local_status)?;
    validate_transport_envelope_auth(envelope.auth.as_ref(), data_dir, topology, &envelope.frame)?;
    let inbox = write_transport_batch_payload(data_dir, &envelope)?;
    let apply_result = apply_transport_batch(
        data_dir,
        &envelope.batch_kind,
        inbox.batch_file,
        inbox.certificate_file,
        None,
    );
    let state_after = status(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .map_err(|error| format!("transport batch service post-apply status failed: {error}"))?;
    let ack = match apply_result {
        Ok(receipts) => transport_batch_ack(
            topology,
            &local_status.node_id,
            &envelope,
            &state_after,
            &receipts,
        ),
        Err(error) if error.contains("already applied") => transport_already_applied_ack(
            data_dir,
            topology,
            &local_status.node_id,
            &envelope,
            &state_after,
        )?,
        Err(error) => return Err(format!("transport batch service apply failed: {error}")),
    };
    write_json_line(stream, &ack)?;
    Ok(ack)
}

pub(super) fn apply_transport_batch(
    data_dir: &Path,
    batch_kind: &str,
    batch_file: PathBuf,
    certificate_file: Option<PathBuf>,
    replay_block_file: Option<PathBuf>,
) -> Result<Vec<Receipt>, String> {
    apply_transport_batch_with_timings(
        data_dir,
        batch_kind,
        batch_file,
        certificate_file,
        replay_block_file,
    )
    .map(|result| result.receipts)
}

pub(super) fn apply_transport_batch_with_timings(
    data_dir: &Path,
    batch_kind: &str,
    batch_file: PathBuf,
    certificate_file: Option<PathBuf>,
    replay_block_file: Option<PathBuf>,
) -> Result<TransportBatchApplyResult, String> {
    let options = ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file,
        certificate_file,
    };
    let result = match batch_kind {
        "transparent" => apply_batch_with_replay(options, replay_block_file).map(|report| {
            TransportBatchApplyResult {
                receipts: report.receipts,
                local_apply_breakdown: Some(report.timings),
            }
        }),
        "governance" => {
            apply_governance_batch_with_replay(options, replay_block_file).map(|receipts| {
                TransportBatchApplyResult {
                    receipts,
                    local_apply_breakdown: None,
                }
            })
        }
        "shielded" => {
            apply_shielded_batch_with_replay(options, replay_block_file).map(|receipts| {
                TransportBatchApplyResult {
                    receipts,
                    local_apply_breakdown: None,
                }
            })
        }
        "bridge" => apply_bridge_batch_with_replay(options, replay_block_file).map(|receipts| {
            TransportBatchApplyResult {
                receipts,
                local_apply_breakdown: None,
            }
        }),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("transport batch kind `{batch_kind}` is not supported for apply"),
        )),
    };
    result.map_err(|error| error.to_string())
}

fn apply_transport_batch_with_verified_certificate_with_timings(
    data_dir: &Path,
    batch_kind: &str,
    batch_file: PathBuf,
    certificate_file: PathBuf,
    verified_certificate: VerifiedBlockCertificateFile,
) -> Result<TransportBatchApplyResult, String> {
    if batch_kind != "transparent" {
        return apply_transport_batch_with_timings(
            data_dir,
            batch_kind,
            batch_file,
            Some(certificate_file),
            None,
        );
    }
    let options = ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file,
        certificate_file: Some(certificate_file),
    };
    apply_batch_with_verified_certificate_with_timings(options, verified_certificate)
        .map(|report| TransportBatchApplyResult {
            receipts: report.receipts,
            local_apply_breakdown: Some(report.timings),
        })
        .map_err(|error| error.to_string())
}

fn transport_batch_send(
    data_dir: PathBuf,
    topology_file: PathBuf,
    to: String,
    batch_kind: Option<String>,
    batch_file: PathBuf,
    certificate_file: Option<PathBuf>,
    timeout_ms: u64,
) -> Result<TransportBatchSendReport, String> {
    let topology = read_topology_file(&topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("transport batch status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let peer = topology
        .peer(&to)
        .ok_or_else(|| format!("target node `{to}` is not in topology"))?;
    let batch_kind = batch_kind.unwrap_or_else(default_transport_batch_kind);
    validate_transport_batch_kind(&batch_kind)?;
    if certificate_file.is_none() {
        return Err(
            "transport batch send requires --certificate-file; uncertified service apply is disabled"
                .to_string(),
        );
    }
    let payload_json = read_transport_payload_file(&batch_file)?;
    serde_json::from_str::<serde_json::Value>(&payload_json)
        .map_err(|error| format!("transport batch payload is not valid JSON: {error}"))?;
    let certificate_json = certificate_file
        .as_ref()
        .map(read_transport_payload_file)
        .transpose()?;
    if let Some(certificate_json) = certificate_json.as_ref() {
        serde_json::from_str::<serde_json::Value>(certificate_json)
            .map_err(|error| format!("transport batch certificate is not valid JSON: {error}"))?;
    }
    let framed_payload =
        transport_batch_frame_payload(&payload_json, certificate_json.as_deref(), &batch_kind)?;
    let domain = network_domain_from_topology(&topology);
    let frame = frame_message(
        &domain,
        local_status.node_id.clone(),
        Some(to.clone()),
        TRANSPORT_BATCH_TOPIC,
        &framed_payload,
    )
    .map_err(|error| format!("transport batch frame failed: {error}"))?;
    let auth = sign_transport_envelope_auth(&data_dir, &topology, &local_status.node_id, &frame)?;
    let envelope = TransportBatchEnvelope {
        schema: TRANSPORT_BATCH_SCHEMA.to_string(),
        topology_id: topology.topology_id.clone(),
        batch_kind: batch_kind.clone(),
        frame,
        auth: Some(auth),
        payload_json,
        certificate_json,
    };
    let peer_address = socket_address(&peer.host, peer.p2p_port);
    let mut stream = connect_transport_stream(&peer_address, timeout_ms, "transport batch dial")?;
    set_stream_timeout(&stream, timeout_ms)?;
    write_json_line(&mut stream, &envelope)?;
    let line = read_transport_line(&stream, "transport batch ack read")?;
    let ack = parse_transport_batch_ack(&line)?;
    validate_transport_batch_ack(&ack, &topology, &local_status, &to, &envelope)?;
    let sent = TransportBatchSummary {
        from: local_status.node_id.clone(),
        to: to.clone(),
        batch_kind: batch_kind.clone(),
        message_id: envelope.frame.message_id.clone(),
        payload_hash: envelope.frame.payload_hash.clone(),
        payload_len: envelope.frame.payload_len,
        certificate_attached: envelope.certificate_json.is_some(),
    };

    Ok(TransportBatchSendReport {
        schema: "postfiat-transport-batch-send-v1".to_string(),
        from: local_status.node_id,
        to,
        topology_id: topology.topology_id,
        peer_address,
        attempts: 1,
        max_attempts: 1,
        retry_backoff_ms: 0,
        retry_errors: Vec::new(),
        sent,
        ack,
        verified: true,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transport_batch_send_with_retries(
    data_dir: PathBuf,
    topology_file: PathBuf,
    to: String,
    batch_kind: Option<String>,
    batch_file: PathBuf,
    certificate_file: Option<PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
) -> Result<TransportBatchSendReport, String> {
    if send_retries > MAX_TRANSPORT_SEND_RETRIES {
        return Err(format!(
            "--send-retries must be <= {MAX_TRANSPORT_SEND_RETRIES}"
        ));
    }
    let max_attempts = send_retries
        .checked_add(1)
        .ok_or_else(|| "transport batch send attempt count overflow".to_string())?;
    let mut retry_errors = Vec::with_capacity(send_retries);
    for attempt in 1..=max_attempts {
        match transport_batch_send(
            data_dir.clone(),
            topology_file.clone(),
            to.clone(),
            batch_kind.clone(),
            batch_file.clone(),
            certificate_file.clone(),
            timeout_ms,
        ) {
            Ok(mut report) => {
                report.attempts = attempt as u64;
                report.max_attempts = max_attempts as u64;
                report.retry_backoff_ms = retry_backoff_ms;
                report.retry_errors = retry_errors;
                return Ok(report);
            }
            Err(error) => {
                retry_errors.push(format!("attempt {attempt}: {error}"));
                if attempt == max_attempts {
                    return Err(format!(
                        "transport batch send to `{to}` failed after {max_attempts} attempts: {}",
                        retry_errors.join("; ")
                    ));
                }
                if retry_backoff_ms > 0 {
                    std::thread::sleep(Duration::from_millis(retry_backoff_ms));
                }
            }
        }
    }
    Err("transport batch send retry loop exited unexpectedly".to_string())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transport_block_vote_listen(
    data_dir: PathBuf,
    topology_file: PathBuf,
    key_file: PathBuf,
    vote_dir: PathBuf,
    bind_host: Option<String>,
    max_requests: usize,
    timeout_ms: u64,
    require_signed_proposal: bool,
) -> Result<TransportBlockVoteListenReport, String> {
    if max_requests == 0 {
        return Err("--max-requests must be positive".to_string());
    }
    std::fs::create_dir_all(&vote_dir)
        .map_err(|error| format!("transport block vote directory create failed: {error}"))?;
    let topology = read_topology_file(&topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("transport block vote status failed: {error}"))?;
    let local_peer = topology
        .peer(&local_status.node_id)
        .ok_or_else(|| format!("local node `{}` is not in topology", local_status.node_id))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let bind_host = bind_host.unwrap_or_else(|| local_peer.host.clone());
    validate_controlled_transport_bind_host(&bind_host)?;
    let bind_address = socket_address(&bind_host, local_peer.p2p_port);
    let ready_file = transport_ready_file_from_env(
        TRANSPORT_BLOCK_VOTE_READY_FILE_ENV,
        "transport block vote listen",
    )?;
    if let Some(ready_file) = ready_file.as_ref() {
        clear_transport_ready_file(ready_file, "transport block vote listen")?;
    }
    let mut accepted = Vec::with_capacity(max_requests);
    let (listener, shielded_verifier_prewarm) = transport_startup_after_prewarm(
        || prewarm_shielded_verifier_cache("transport block vote listen"),
        || {
            TcpListener::bind(&bind_address).map_err(|error| {
                format!("transport block vote listen bind `{bind_address}` failed: {error}")
            })
        },
        |shielded_verifier_prewarm| {
            if let Some(ready_file) = ready_file.as_ref() {
                let ready_report = TransportBlockVoteListenReadyReport {
                    schema: "postfiat-transport-block-vote-listen-ready-v1",
                    node_id: &local_status.node_id,
                    topology_id: &topology.topology_id,
                    bind_address: &bind_address,
                    vote_dir: vote_dir.display().to_string(),
                    max_requests,
                    timeout_ms,
                    require_signed_proposal,
                    shielded_verifier_prewarm,
                };
                write_transport_ready_file(
                    &ready_file,
                    &ready_report,
                    "transport block vote listen",
                )?;
            }
            Ok(())
        },
    )?;
    for _ in 0..max_requests {
        let (mut stream, _) = listener
            .accept()
            .map_err(|error| format!("transport block vote accept failed: {error}"))?;
        set_stream_timeout(&stream, timeout_ms)?;
        let response = handle_transport_block_vote_connection(
            &mut stream,
            &data_dir,
            &key_file,
            &vote_dir,
            &topology,
            &local_status,
            require_signed_proposal,
        )?;
        accepted.push(response);
    }

    Ok(TransportBlockVoteListenReport {
        schema: "postfiat-transport-block-vote-listen-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        bind_address,
        require_signed_proposal,
        shielded_verifier_prewarm,
        accepted,
        verified: true,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transport_validator_serve(
    data_dir: PathBuf,
    topology_file: PathBuf,
    key_file: PathBuf,
    vote_dir: PathBuf,
    bind_host: Option<String>,
    max_connections: usize,
    timeout_ms: u64,
    event_log: Option<PathBuf>,
    require_signed_proposal: bool,
) -> Result<TransportValidatorServeReport, String> {
    transport_validator_serve_inner(
        data_dir,
        topology_file,
        key_file,
        vote_dir,
        bind_host,
        max_connections,
        timeout_ms,
        event_log,
        require_signed_proposal,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transport_validator_serve_inner(
    data_dir: PathBuf,
    topology_file: PathBuf,
    key_file: PathBuf,
    vote_dir: PathBuf,
    bind_host: Option<String>,
    max_connections: usize,
    timeout_ms: u64,
    event_log: Option<PathBuf>,
    require_signed_proposal: bool,
    prewarmed_for_test: Option<TransportShieldedVerifierPrewarmReport>,
) -> Result<TransportValidatorServeReport, String> {
    if max_connections == 0 {
        return Err("--max-connections must be positive".to_string());
    }
    std::fs::create_dir_all(&vote_dir)
        .map_err(|error| format!("transport validator vote directory create failed: {error}"))?;
    let topology = read_topology_file(&topology_file)?;
    let local_status = run_once(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("transport validator service status failed: {error}"))?;
    let local_peer = topology
        .peer(&local_status.node_id)
        .ok_or_else(|| format!("local node `{}` is not in topology", local_status.node_id))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let bind_host = bind_host.unwrap_or_else(|| local_peer.host.clone());
    validate_controlled_transport_bind_host(&bind_host)?;
    let bind_address = socket_address(&bind_host, local_peer.p2p_port);
    let event_log_path = event_log.clone();
    let event_writer = event_log
        .as_ref()
        .map(|path| open_transport_event_log(path))
        .transpose()?;
    let ready_file = transport_ready_file_from_env(
        TRANSPORT_VALIDATOR_READY_FILE_ENV,
        "transport validator service",
    )?;
    if let Some(ready_file) = ready_file.as_ref() {
        clear_transport_ready_file(ready_file, "transport validator service")?;
    }
    let event_writer = Arc::new(Mutex::new(event_writer));
    let shared_state = Arc::new(Mutex::new(TransportValidatorServeSharedState::default()));
    let mut handles = Vec::with_capacity(max_connections);
    let (listener, shielded_verifier_prewarm) = transport_startup_after_prewarm(
        || match prewarmed_for_test {
            Some(report) => Ok(report),
            None => prewarm_shielded_verifier_cache("transport validator service"),
        },
        || {
            TcpListener::bind(&bind_address).map_err(|error| {
                format!("transport validator service bind `{bind_address}` failed: {error}")
            })
        },
        |shielded_verifier_prewarm| {
            if let Some(ready_file) = ready_file.as_ref() {
                let ready_report = TransportValidatorServeReadyReport {
                    schema: "postfiat-transport-validator-serve-ready-v1",
                    node_id: &local_status.node_id,
                    topology_id: &topology.topology_id,
                    bind_address: &bind_address,
                    vote_dir: vote_dir.display().to_string(),
                    max_connections,
                    timeout_ms,
                    require_signed_proposal,
                    shielded_verifier_prewarm,
                };
                write_transport_ready_file(
                    &ready_file,
                    &ready_report,
                    "transport validator service",
                )?;
            }
            Ok(())
        },
    )?;
    for connection_index in 1..=max_connections {
        let connection_index = connection_index as u64;
        let (stream, _) = listener
            .accept()
            .map_err(|error| format!("transport validator service accept failed: {error}"))?;
        set_stream_timeout(&stream, timeout_ms)?;
        let data_dir_for_thread = data_dir.clone();
        let key_file_for_thread = key_file.clone();
        let vote_dir_for_thread = vote_dir.clone();
        let topology_for_thread = topology.clone();
        let event_writer_for_thread = Arc::clone(&event_writer);
        let shared_state_for_thread = Arc::clone(&shared_state);
        handles.push(thread::spawn(move || {
            handle_transport_validator_connection(
                stream,
                data_dir_for_thread,
                key_file_for_thread,
                vote_dir_for_thread,
                topology_for_thread,
                event_writer_for_thread,
                shared_state_for_thread,
                connection_index,
                require_signed_proposal,
            )
        }));
    }

    for handle in handles {
        handle
            .join()
            .map_err(|_| "transport validator service worker thread panicked".to_string())??;
    }
    let mut shared_state = shared_state
        .lock()
        .map_err(|_| "transport validator service summary lock poisoned".to_string())?;
    let batch_acks = std::mem::take(&mut shared_state.batch_acks);
    let block_vote_responses = std::mem::take(&mut shared_state.block_vote_responses);
    let rejected = std::mem::take(&mut shared_state.rejected);

    Ok(TransportValidatorServeReport {
        schema: "postfiat-transport-validator-serve-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        bind_address,
        event_log: event_log_path.map(|path| path.display().to_string()),
        require_signed_proposal,
        shielded_verifier_prewarm,
        connection_count: max_connections as u64,
        accepted_batch_count: batch_acks.len() as u64,
        accepted_block_vote_count: block_vote_responses.len() as u64,
        rejected_count: rejected.len() as u64,
        batch_acks,
        block_vote_responses,
        rejected,
        verified: true,
    })
}

fn transport_connection_idle_or_closed(error: &str) -> bool {
    error.contains("returned empty frame")
        || error.contains("Resource temporarily unavailable")
        || error.contains("timed out")
        || error.contains("WouldBlock")
        || error.contains("Connection reset")
        || error.contains("Broken pipe")
}

#[allow(clippy::too_many_arguments)]
fn handle_transport_validator_connection(
    mut stream: TcpStream,
    data_dir: PathBuf,
    key_file: PathBuf,
    vote_dir: PathBuf,
    topology: NetworkTopology,
    event_writer: Arc<Mutex<Option<std::fs::File>>>,
    shared_state: Arc<Mutex<TransportValidatorServeSharedState>>,
    connection_index: u64,
    require_signed_proposal: bool,
) -> Result<(), String> {
    let mut handled_request_count = 0u64;
    loop {
        let local_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .map_err(|error| {
            format!("transport validator service connection status failed: {error}")
        })?;
        validate_status_matches_topology(&local_status, &topology)?;
        let read_start = Instant::now();
        let line = match read_transport_line(&stream, "transport validator service read") {
            Ok(line) => line,
            Err(error) => {
                if handled_request_count > 0 && transport_connection_idle_or_closed(&error) {
                    break;
                }
                let rejection = transport_validator_rejection(
                    &data_dir,
                    &topology,
                    &local_status,
                    connection_index,
                    "unknown",
                    error,
                )?;
                {
                    let mut writer = event_writer.lock().map_err(|_| {
                        "transport validator service event log lock poisoned".to_string()
                    })?;
                    write_transport_validator_event(
                        &mut writer,
                        &local_status,
                        &topology,
                        connection_index,
                        "unknown",
                        None,
                        None,
                        Some(rejection.clone()),
                    )?;
                }
                shared_state
                    .lock()
                    .map_err(|_| "transport validator service state lock poisoned".to_string())?
                    .rejected
                    .push(rejection);
                break;
            }
        };
        handled_request_count = handled_request_count.saturating_add(1);
        let transport_read_ms = monotonic_elapsed_ms(read_start);
        let kind = transport_envelope_schema(&line).unwrap_or_else(|_| "unknown".to_string());
        match kind.as_str() {
            TRANSPORT_BATCH_SCHEMA => match handle_transport_batch_service_line(
                &mut stream,
                &data_dir,
                &topology,
                &local_status,
                &line,
            ) {
                Ok(ack) => {
                    {
                        let mut writer = event_writer.lock().map_err(|_| {
                            "transport validator service event log lock poisoned".to_string()
                        })?;
                        write_transport_validator_event(
                            &mut writer,
                            &local_status,
                            &topology,
                            connection_index,
                            "batch",
                            Some(ack.clone()),
                            None,
                            None,
                        )?;
                    }
                    shared_state
                        .lock()
                        .map_err(|_| "transport validator service state lock poisoned".to_string())?
                        .batch_acks
                        .push(ack);
                }
                Err(error) => {
                    record_transport_validator_rejection(
                        Some(&mut stream),
                        &data_dir,
                        &topology,
                        &local_status,
                        connection_index,
                        "batch",
                        error,
                        &event_writer,
                        &shared_state,
                    )?;
                }
            },
            TRANSPORT_BLOCK_VOTE_REQUEST_SCHEMA => match handle_transport_block_vote_line(
                &mut stream,
                &data_dir,
                &key_file,
                &vote_dir,
                &topology,
                &local_status,
                &line,
                require_signed_proposal,
                transport_read_ms,
            ) {
                Ok(response) => {
                    {
                        let mut writer = event_writer.lock().map_err(|_| {
                            "transport validator service event log lock poisoned".to_string()
                        })?;
                        write_transport_validator_event(
                            &mut writer,
                            &local_status,
                            &topology,
                            connection_index,
                            "block_vote_request",
                            None,
                            Some(response.clone()),
                            None,
                        )?;
                    }
                    shared_state
                        .lock()
                        .map_err(|_| "transport validator service state lock poisoned".to_string())?
                        .block_vote_responses
                        .push(response);
                }
                Err(error) => {
                    record_transport_validator_rejection(
                        Some(&mut stream),
                        &data_dir,
                        &topology,
                        &local_status,
                        connection_index,
                        "block_vote_request",
                        error,
                        &event_writer,
                        &shared_state,
                    )?;
                }
            },
            other => {
                record_transport_validator_rejection(
                    Some(&mut stream),
                    &data_dir,
                    &topology,
                    &local_status,
                    connection_index,
                    other,
                    format!("transport validator service schema `{other}` is not supported"),
                    &event_writer,
                    &shared_state,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_transport_validator_rejection(
    response_stream: Option<&mut TcpStream>,
    data_dir: &Path,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    connection_index: u64,
    kind: &str,
    error: String,
    event_writer: &Arc<Mutex<Option<std::fs::File>>>,
    shared_state: &Arc<Mutex<TransportValidatorServeSharedState>>,
) -> Result<(), String> {
    let rejection = transport_validator_rejection(
        data_dir,
        topology,
        local_status,
        connection_index,
        kind,
        error,
    )?;
    if let Some(stream) = response_stream {
        write_transport_validator_rejection_response(stream, &rejection)?;
    }
    {
        let mut writer = event_writer
            .lock()
            .map_err(|_| "transport validator service event log lock poisoned".to_string())?;
        write_transport_validator_event(
            &mut writer,
            local_status,
            topology,
            connection_index,
            kind,
            None,
            None,
            Some(rejection.clone()),
        )?;
    }
    shared_state
        .lock()
        .map_err(|_| "transport validator service state lock poisoned".to_string())?
        .rejected
        .push(rejection);
    Ok(())
}

fn write_transport_validator_rejection_response(
    stream: &mut TcpStream,
    rejection: &TransportValidatorServeRejection,
) -> Result<(), String> {
    let json = serde_json::to_string(rejection)
        .map_err(|error| format!("transport validator rejection serialization failed: {error}"))?;
    stream
        .write_all(json.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("transport validator rejection response write failed: {error}"))
}

fn handle_transport_block_vote_connection(
    stream: &mut TcpStream,
    data_dir: &Path,
    key_file: &Path,
    vote_dir: &Path,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    require_signed_proposal: bool,
) -> Result<TransportBlockVoteResponse, String> {
    let read_start = Instant::now();
    let line = read_transport_line(stream, "transport block vote request read")?;
    let transport_read_ms = monotonic_elapsed_ms(read_start);
    handle_transport_block_vote_line(
        stream,
        data_dir,
        key_file,
        vote_dir,
        topology,
        local_status,
        &line,
        require_signed_proposal,
        transport_read_ms,
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_transport_block_vote_line(
    stream: &mut TcpStream,
    data_dir: &Path,
    key_file: &Path,
    vote_dir: &Path,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    line: &str,
    require_signed_proposal: bool,
    transport_read_ms: f64,
) -> Result<TransportBlockVoteResponse, String> {
    let total_start = Instant::now();
    let mut timings = TransportBlockVoteHandlingTimingReport {
        schema: "postfiat-transport-block-vote-handling-timings-v1".to_string(),
        total_ms: 0.0,
        transport_read_ms,
        request_parse_ms: 0.0,
        request_validate_ms: 0.0,
        auth_validate_ms: 0.0,
        signed_proposal_policy_ms: 0.0,
        request_dir_ms: 0.0,
        batch_file_write_ms: 0.0,
        proposal_file_write_ms: 0.0,
        vote_creation_ms: 0.0,
        response_build_ms: 0.0,
        response_json_serde_ms: 0.0,
        transport_write_ms: 0.0,
        process_spawn_ms: 0.0,
        block_vote_breakdown: BlockVoteCreationTimingReport::default(),
    };

    let stage_start = Instant::now();
    let envelope = parse_transport_block_vote_request(line)?;
    timings.request_parse_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    validate_transport_block_vote_request(&envelope, topology, local_status)?;
    timings.request_validate_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    validate_transport_envelope_auth(envelope.auth.as_ref(), data_dir, topology, &envelope.frame)?;
    timings.auth_validate_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    validate_signed_proposal_policy(&envelope.proposal_json, require_signed_proposal)?;
    timings.signed_proposal_policy_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    let request_dir = vote_dir.join("requests");
    std::fs::create_dir_all(&request_dir).map_err(|error| {
        format!("transport block vote request directory create failed: {error}")
    })?;
    timings.request_dir_ms = monotonic_elapsed_ms(stage_start);

    let batch_file = request_dir.join(format!("{}.batch.json", envelope.frame.message_id));
    let proposal_file = request_dir.join(format!("{}.proposal.json", envelope.frame.message_id));
    let timeout_certificate_file = envelope.timeout_certificate_json.as_ref().map(|_| {
        request_dir.join(format!(
            "{}.timeout-certificate.json",
            envelope.frame.message_id
        ))
    });
    let vote_file = vote_dir.join(format!(
        "{}.{}.{}.block_vote.json",
        envelope.block_height, envelope.view, local_status.node_id
    ));

    let stage_start = Instant::now();
    std::fs::write(&batch_file, envelope.batch_json.as_bytes())
        .map_err(|error| format!("transport block vote batch write failed: {error}"))?;
    timings.batch_file_write_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    std::fs::write(&proposal_file, envelope.proposal_json.as_bytes())
        .map_err(|error| format!("transport block vote proposal write failed: {error}"))?;
    timings.proposal_file_write_ms = monotonic_elapsed_ms(stage_start);

    if let (Some(path), Some(json)) = (
        timeout_certificate_file.as_ref(),
        envelope.timeout_certificate_json.as_ref(),
    ) {
        serde_json::from_str::<serde_json::Value>(json).map_err(|error| {
            format!("transport block vote timeout certificate is not valid JSON: {error}")
        })?;
        std::fs::write(path, json.as_bytes()).map_err(|error| {
            format!("transport block vote timeout certificate write failed: {error}")
        })?;
    }

    let stage_start = Instant::now();
    let vote_with_timings = create_block_vote_with_timings(BlockVoteOptions {
        data_dir: data_dir.to_path_buf(),
        verify_block_log: false,
        key_file: key_file.to_path_buf(),
        validator_id: Some(local_status.node_id.clone()),
        batch_file: Some(batch_file),
        proposal_file: Some(proposal_file),
        timeout_certificate_file,
        block_height: Some(envelope.block_height),
        vote_file: vote_file.clone(),
    })
    .map_err(|error| format!("transport block vote signing failed: {error}"))?;
    timings.vote_creation_ms = monotonic_elapsed_ms(stage_start);
    timings.block_vote_breakdown = vote_with_timings.timings;

    let consensus_v2_vote = match envelope.consensus_v2.as_ref() {
        None => None,
        Some(request) => {
            let block_proposal: BlockProposalFile = serde_json::from_str(&envelope.proposal_json)
                .map_err(|error| {
                    format!("transport consensus v2 block proposal parse failed: {error}")
                })?;
            verify_consensus_v2_proposal_matches_block(&block_proposal, &request.proposal)
                .map_err(|error| {
                    format!("transport consensus v2 proposal binding failed: {error}")
                })?;
            let vote = match request.phase {
                postfiat_types::ConsensusV2Phase::Prepare => {
                    if request.prepare_qc.is_some() {
                        return Err(
                            "transport consensus v2 prepare request carried prepare QC"
                                .to_string(),
                        );
                    }
                    create_consensus_v2_prepare_vote(
                        data_dir,
                        &request.proposal,
                        request.timeout_certificate.as_ref(),
                        key_file,
                        &local_status.node_id,
                    )
                    .map_err(|error| {
                        format!("transport consensus v2 prepare signing failed: {error}")
                    })?
                }
                postfiat_types::ConsensusV2Phase::Precommit => {
                    let prepare_qc = request.prepare_qc.as_ref().ok_or_else(|| {
                        "transport consensus v2 precommit request omitted prepare QC".to_string()
                    })?;
                    let (domain, validators) = live_consensus_v2_context(data_dir)
                        .map_err(|error| format!("transport consensus v2 context: {error}"))?;
                    let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)
                        .map_err(|error| format!("transport consensus v2 QC graph: {error}"))?;
                    postfiat_ordering_fast::verify_consensus_v2_proposal(
                        &domain,
                        &validators,
                        &request.proposal,
                        request.timeout_certificate.as_ref(),
                        &graph,
                    )
                    .map_err(|error| {
                        format!("transport consensus v2 precommit proposal failed: {error}")
                    })?;
                    create_consensus_v2_precommit_vote(
                        data_dir,
                        prepare_qc,
                        key_file,
                        &local_status.node_id,
                    )
                    .map_err(|error| {
                        format!("transport consensus v2 precommit signing failed: {error}")
                    })?
                }
            };
            Some(vote)
        }
    };

    let stage_start = Instant::now();
    let response = TransportBlockVoteResponse {
        schema: TRANSPORT_BLOCK_VOTE_RESPONSE_SCHEMA.to_string(),
        topology_id: topology.topology_id.clone(),
        from: local_status.node_id.clone(),
        to: envelope.frame.from.clone(),
        message_id: envelope.frame.message_id.clone(),
        payload_hash: envelope.frame.payload_hash.clone(),
        block_height: envelope.block_height,
        view: envelope.view,
        vote_file: vote_file.display().to_string(),
        vote: vote_with_timings.vote,
        consensus_v2_vote,
        state: transport_hello(topology, local_status),
        timings: None,
        verified: true,
    };
    timings.response_build_ms = monotonic_elapsed_ms(stage_start);

    let mut response = response;
    timings.total_ms = transport_read_ms + monotonic_elapsed_ms(total_start);
    response.timings = Some(timings.clone());

    let stage_start = Instant::now();
    serde_json::to_string(&response)
        .map_err(|error| format!("transport block vote response serialization failed: {error}"))?;
    if let Some(response_timings) = response.timings.as_mut() {
        response_timings.response_json_serde_ms = monotonic_elapsed_ms(stage_start);
        response_timings.total_ms = transport_read_ms + monotonic_elapsed_ms(total_start);
    }
    let response_json = serde_json::to_string(&response)
        .map_err(|error| format!("transport block vote response serialization failed: {error}"))?;

    let stage_start = Instant::now();
    stream
        .write_all(response_json.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("transport block vote response write failed: {error}"))?;
    if let Some(response_timings) = response.timings.as_mut() {
        response_timings.transport_write_ms = monotonic_elapsed_ms(stage_start);
        response_timings.total_ms = transport_read_ms + monotonic_elapsed_ms(total_start);
    }
    Ok(response)
}

pub(super) fn transport_block_vote_request(
    options: TransportBlockVoteRequestOptions,
) -> Result<TransportBlockVoteRequestReport, String> {
    let total_start = Instant::now();
    let mut timings = TransportBlockVoteRequestTimingReport {
        schema: "postfiat-transport-block-vote-request-timings-v1".to_string(),
        total_ms: 0.0,
        attempt_loop_ms: 0.0,
        retry_sleep_ms: 0.0,
        topology_read_ms: 0.0,
        status_ms: 0.0,
        peer_lookup_ms: 0.0,
        payload_read_ms: 0.0,
        request_json_serde_ms: 0.0,
        request_frame_ms: 0.0,
        transport_connect_ms: 0.0,
        transport_write_ms: 0.0,
        transport_read_ms: 0.0,
        response_json_serde_ms: 0.0,
        response_validate_ms: 0.0,
        vote_json_serde_ms: 0.0,
        vote_file_write_ms: 0.0,
        remote_handling: None,
    };

    let stage_start = Instant::now();
    let topology = read_topology_file(&options.topology_file)?;
    timings.topology_read_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    let local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("transport block vote request status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;
    timings.status_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    let peer = topology
        .peer(&options.to)
        .ok_or_else(|| format!("target node `{}` is not in topology", options.to))?;
    timings.peer_lookup_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    let batch_json = read_transport_payload_file(&options.batch_file)?;
    serde_json::from_str::<serde_json::Value>(&batch_json)
        .map_err(|error| format!("transport block vote batch is not valid JSON: {error}"))?;
    let proposal_json = read_transport_payload_file(&options.proposal_file)?;
    let proposal = serde_json::from_str::<serde_json::Value>(&proposal_json)
        .map_err(|error| format!("transport block vote proposal is not valid JSON: {error}"))?;
    let timeout_certificate_json = match options.timeout_certificate_file.as_ref() {
        Some(path) => {
            let json = read_transport_payload_file(path)?;
            serde_json::from_str::<serde_json::Value>(&json).map_err(|error| {
                format!("transport block vote timeout certificate is not valid JSON: {error}")
            })?;
            Some(json)
        }
        None => None,
    };
    timings.payload_read_ms = monotonic_elapsed_ms(stage_start);

    let (proposal_height, view) = proposal_height_view(&proposal)?;
    let block_height = options.block_height.ok_or_else(|| {
        "transport block vote request requires --height for proposal-backed votes".to_string()
    })?;
    if proposal_height != block_height {
        return Err(format!(
            "transport block vote request proposal height {proposal_height} does not match --height {block_height}"
        ));
    }
    let batch_kind = options
        .batch_kind
        .unwrap_or_else(|| "transparent".to_string());
    if !is_supported_transport_batch_kind(&batch_kind) {
        return Err(format!(
            "transport block vote request batch kind `{batch_kind}` is not supported"
        ));
    }
    let stage_start = Instant::now();
    let framed_payload = transport_block_vote_request_payload(
        block_height,
        view,
        &batch_kind,
        &batch_json,
        &proposal_json,
        timeout_certificate_json.as_deref(),
        options.consensus_v2.as_ref(),
    )?;
    timings.request_json_serde_ms += monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    let domain = network_domain_from_topology(&topology);
    let frame = frame_message(
        &domain,
        local_status.node_id.clone(),
        Some(options.to.clone()),
        TRANSPORT_BLOCK_VOTE_TOPIC,
        &framed_payload,
    )
    .map_err(|error| format!("transport block vote frame failed: {error}"))?;
    timings.request_frame_ms = monotonic_elapsed_ms(stage_start);

    let envelope = TransportBlockVoteRequestEnvelope {
        schema: TRANSPORT_BLOCK_VOTE_REQUEST_SCHEMA.to_string(),
        topology_id: topology.topology_id.clone(),
        auth: Some(sign_transport_envelope_auth(
            &options.data_dir,
            &topology,
            &local_status.node_id,
            &frame,
        )?),
        frame,
        block_height,
        view,
        batch_kind: batch_kind.clone(),
        batch_json,
        proposal_json,
        timeout_certificate_json,
        consensus_v2: options.consensus_v2,
    };
    let peer_address = socket_address(&peer.host, peer.p2p_port);

    let stage_start = Instant::now();
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|error| format!("transport block vote request serialization failed: {error}"))?;
    timings.request_json_serde_ms += monotonic_elapsed_ms(stage_start);

    let (line, transport_connect_ms, transport_write_ms, transport_read_ms) =
        transport_block_vote_request_exchange(&peer_address, &envelope_json, options.timeout_ms)?;
    timings.transport_connect_ms = transport_connect_ms;
    timings.transport_write_ms = transport_write_ms;
    timings.transport_read_ms = transport_read_ms;

    let stage_start = Instant::now();
    let response = parse_transport_block_vote_response(&line)?;
    timings.response_json_serde_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    validate_transport_block_vote_response(
        &response,
        &topology,
        &local_status,
        &options.to,
        &envelope,
    )?;
    if let Some(vote) = response.consensus_v2_vote.as_ref() {
        let (domain, validators) = live_consensus_v2_context(&options.data_dir)
            .map_err(|error| format!("transport consensus v2 response context: {error}"))?;
        postfiat_ordering_fast::verify_consensus_v2_vote(&domain, &validators, vote)
            .map_err(|error| format!("transport consensus v2 response vote invalid: {error}"))?;
    }
    timings.response_validate_ms = monotonic_elapsed_ms(stage_start);

    if let Some(parent) = options
        .vote_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!("transport block vote output directory create failed: {error}")
        })?;
    }
    let stage_start = Instant::now();
    let vote_json = serde_json::to_string_pretty(&response.vote)
        .map_err(|error| format!("transport block vote serialization failed: {error}"))?;
    timings.vote_json_serde_ms = monotonic_elapsed_ms(stage_start);

    let stage_start = Instant::now();
    std::fs::write(&options.vote_file, vote_json.as_bytes())
        .map_err(|error| format!("transport block vote output write failed: {error}"))?;
    timings.vote_file_write_ms = monotonic_elapsed_ms(stage_start);
    timings.remote_handling = response.timings.clone();
    timings.total_ms = monotonic_elapsed_ms(total_start);
    timings.attempt_loop_ms = timings.total_ms;

    let request = TransportBatchSummary {
        from: local_status.node_id.clone(),
        to: options.to.clone(),
        batch_kind: batch_kind.clone(),
        message_id: envelope.frame.message_id.clone(),
        payload_hash: envelope.frame.payload_hash.clone(),
        payload_len: envelope.frame.payload_len,
        certificate_attached: false,
    };

    Ok(TransportBlockVoteRequestReport {
        schema: "postfiat-transport-block-vote-request-report-v1".to_string(),
        from: local_status.node_id,
        to: options.to,
        topology_id: topology.topology_id,
        peer_address,
        attempts: 1,
        max_attempts: 1,
        retry_backoff_ms: 0,
        retry_errors: Vec::new(),
        request,
        response,
        vote_file: options.vote_file.display().to_string(),
        timings,
        verified: true,
    })
}

fn persistent_vote_streams_enabled() -> bool {
    match std::env::var(TRANSPORT_PERSISTENT_VOTE_STREAMS_ENV) {
        Ok(value) => {
            let value = value.trim().to_ascii_lowercase();
            !(value == "0" || value == "false" || value == "off" || value == "no")
        }
        Err(_) => true,
    }
}

fn transport_vote_stream_pool() -> &'static Mutex<BTreeMap<String, TcpStream>> {
    static POOL: std::sync::OnceLock<Mutex<BTreeMap<String, TcpStream>>> =
        std::sync::OnceLock::new();
    POOL.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[cfg(test)]
pub(super) fn clear_transport_vote_stream_pool_for_test() -> Result<(), String> {
    transport_vote_stream_pool()
        .lock()
        .map_err(|_| "transport vote stream pool lock poisoned".to_string())?
        .clear();
    Ok(())
}

fn write_transport_frame(
    stream: &mut TcpStream,
    frame_json: &str,
    context: &str,
) -> Result<(), String> {
    stream
        .write_all(frame_json.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("{context} write failed: {error}"))
}

fn transport_vote_exchange_on_stream(
    stream: &mut TcpStream,
    envelope_json: &str,
    timeout_ms: u64,
) -> Result<(String, f64, f64), String> {
    set_stream_timeout(stream, timeout_ms)?;
    let write_start = Instant::now();
    write_transport_frame(stream, envelope_json, "transport block vote request")?;
    let transport_write_ms = monotonic_elapsed_ms(write_start);
    let read_start = Instant::now();
    let line = read_transport_line(stream, "transport block vote response read")?;
    let transport_read_ms = monotonic_elapsed_ms(read_start);
    Ok((line, transport_write_ms, transport_read_ms))
}

fn transport_block_vote_request_exchange(
    peer_address: &str,
    envelope_json: &str,
    timeout_ms: u64,
) -> Result<(String, f64, f64, f64), String> {
    let pool_enabled = persistent_vote_streams_enabled();
    if pool_enabled {
        let cached_stream = {
            transport_vote_stream_pool()
                .lock()
                .map_err(|_| "transport vote stream pool lock poisoned".to_string())?
                .remove(peer_address)
        };
        if let Some(mut stream) = cached_stream {
            match transport_vote_exchange_on_stream(&mut stream, envelope_json, timeout_ms) {
                Ok((line, transport_write_ms, transport_read_ms)) => {
                    transport_vote_stream_pool()
                        .lock()
                        .map_err(|_| "transport vote stream pool lock poisoned".to_string())?
                        .insert(peer_address.to_string(), stream);
                    return Ok((line, 0.0, transport_write_ms, transport_read_ms));
                }
                Err(_) => {
                    // Stale pooled streams are expected during rolling deploys and
                    // idle timeout closes. Drop the stream and use the one-shot path.
                }
            }
        }
    }

    let connect_start = Instant::now();
    let mut stream =
        connect_transport_stream(peer_address, timeout_ms, "transport block vote dial")?;
    set_stream_timeout(&stream, timeout_ms)?;
    let transport_connect_ms = monotonic_elapsed_ms(connect_start);
    let (line, transport_write_ms, transport_read_ms) =
        transport_vote_exchange_on_stream(&mut stream, envelope_json, timeout_ms)?;
    if pool_enabled {
        transport_vote_stream_pool()
            .lock()
            .map_err(|_| "transport vote stream pool lock poisoned".to_string())?
            .insert(peer_address.to_string(), stream);
    }
    Ok((
        line,
        transport_connect_ms,
        transport_write_ms,
        transport_read_ms,
    ))
}

#[allow(clippy::too_many_arguments)]
fn transport_block_vote_request_with_retries(
    data_dir: PathBuf,
    topology_file: PathBuf,
    to: String,
    batch_kind: Option<String>,
    batch_file: PathBuf,
    proposal_file: PathBuf,
    timeout_certificate_file: Option<PathBuf>,
    vote_file: PathBuf,
    block_height: Option<u64>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    consensus_v2: Option<TransportConsensusV2VoteRequest>,
) -> Result<TransportBlockVoteRequestReport, String> {
    if send_retries > MAX_TRANSPORT_SEND_RETRIES {
        return Err(format!(
            "--send-retries must be <= {MAX_TRANSPORT_SEND_RETRIES}"
        ));
    }
    let max_attempts = send_retries
        .checked_add(1)
        .ok_or_else(|| "transport block vote request attempt count overflow".to_string())?;
    let mut retry_errors = Vec::with_capacity(send_retries);
    let retry_loop_start = Instant::now();
    let mut retry_sleep_ms_total = 0.0;
    for attempt in 1..=max_attempts {
        match transport_block_vote_request(TransportBlockVoteRequestOptions {
            data_dir: data_dir.clone(),
            topology_file: topology_file.clone(),
            to: to.clone(),
            batch_kind: batch_kind.clone(),
            batch_file: batch_file.clone(),
            proposal_file: proposal_file.clone(),
            timeout_certificate_file: timeout_certificate_file.clone(),
            vote_file: vote_file.clone(),
            block_height,
            timeout_ms,
            consensus_v2: consensus_v2.clone(),
        }) {
            Ok(mut report) => {
                report.attempts = attempt as u64;
                report.max_attempts = max_attempts as u64;
                report.retry_backoff_ms = retry_backoff_ms;
                report.retry_errors = retry_errors;
                report.timings.retry_sleep_ms = retry_sleep_ms_total;
                report.timings.attempt_loop_ms = monotonic_elapsed_ms(retry_loop_start);
                return Ok(report);
            }
            Err(error) => {
                retry_errors.push(format!("attempt {attempt}: {error}"));
                if attempt == max_attempts {
                    return Err(format!(
                        "transport block vote request to `{to}` failed after {max_attempts} attempts: {}",
                        retry_errors.join("; ")
                ));
                }
                if retry_backoff_ms > 0 {
                    let sleep_start = Instant::now();
                    std::thread::sleep(Duration::from_millis(retry_backoff_ms));
                    retry_sleep_ms_total += monotonic_elapsed_ms(sleep_start);
                }
            }
        }
    }
    Err("transport block vote request retry loop exited unexpectedly".to_string())
}

#[allow(clippy::too_many_arguments)]
fn collect_consensus_v2_precommit_votes(
    options: &TransportPeerCertifiedBatchRoundOptions,
    proposal: &BlockProposalFile,
    consensus_v2_proposal: &postfiat_types::ConsensusV2Proposal,
    timeout_certificate: Option<&postfiat_types::ConsensusV2TimeoutCertificate>,
    prepare_qc: &postfiat_types::ConsensusV2QuorumCertificate,
    proposal_file: &Path,
    vote_dir: &Path,
    targets: &[String],
    local_status: &StatusReport,
) -> Result<Vec<postfiat_types::ConsensusV2Vote>, String> {
    let local_vote = create_consensus_v2_precommit_vote(
        &options.data_dir,
        prepare_qc,
        &options.key_file,
        &local_status.node_id,
    )
    .map_err(|error| format!("local consensus v2 precommit failed: {error}"))?;
    let outcomes = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(targets.len());
        for target in targets {
            let target = target.clone();
            let data_dir = options.data_dir.clone();
            let topology_file = options.topology_file.clone();
            let batch_kind = Some(proposal.batch_kind.clone());
            let batch_file = options.batch_file.clone();
            let proposal_file = proposal_file.to_path_buf();
            let timeout_certificate_file = options.timeout_certificate_file.clone();
            let vote_file = vote_dir.join(format!("{target}.precommit.block_vote.json"));
            let block_height = Some(proposal.block_height);
            let timeout_ms = options.timeout_ms;
            let send_retries = options.send_retries;
            let retry_backoff_ms = options.retry_backoff_ms;
            let consensus_v2 = Some(TransportConsensusV2VoteRequest {
                phase: postfiat_types::ConsensusV2Phase::Precommit,
                proposal: consensus_v2_proposal.clone(),
                timeout_certificate: timeout_certificate.cloned(),
                prepare_qc: Some(prepare_qc.clone()),
            });
            handles.push(scope.spawn(move || {
                let result = transport_block_vote_request_with_retries(
                    data_dir,
                    topology_file,
                    target.clone(),
                    batch_kind,
                    batch_file,
                    proposal_file,
                    timeout_certificate_file,
                    vote_file,
                    block_height,
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    consensus_v2,
                );
                (target, result)
            }));
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "consensus v2 precommit worker panicked".to_string())
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    let mut votes = vec![local_vote];
    for (target, result) in outcomes {
        match result {
            Ok(report) => {
                let vote = report.response.consensus_v2_vote.ok_or_else(|| {
                    format!("validator `{target}` omitted consensus v2 precommit vote")
                })?;
                votes.push(vote);
            }
            Err(_error)
                if options.allow_peer_failures || options.quorum_early_full_propagation => {}
            Err(error) => return Err(error),
        }
    }
    Ok(votes)
}

pub(super) fn transport_certified_batch_round(
    options: TransportCertifiedBatchRoundOptions,
) -> Result<TransportCertifiedBatchRoundReport, String> {
    if options.send_retries > MAX_TRANSPORT_SEND_RETRIES {
        return Err(format!(
            "--send-retries must be <= {MAX_TRANSPORT_SEND_RETRIES}"
        ));
    }
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "certified batch round artifact directory create `{}` failed: {error}",
            options.artifact_dir.display()
        )
    })?;
    let proposal_file = options.artifact_dir.join("block-proposal.json");
    let vote_dir = options.artifact_dir.join("votes");
    let certificate_file = options.artifact_dir.join("block-certificate.json");
    let certification = certify_batch_round(BatchCertificateRoundOptions {
        data_dir: options.data_dir.clone(),
        batch_kind: options.batch_kind.clone(),
        batch_file: options.batch_file.clone(),
        validator_key_dir: options.validator_key_dir.clone(),
        vote_dir,
        proposal_file,
        certificate_file: certificate_file.clone(),
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        skip_block_log_verify: options.skip_block_log_verify,
    })
    .map_err(|error| format!("transport certified batch round certification failed: {error}"))?;

    let topology = read_topology_file(&options.topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("transport certified batch round status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let targets = active_transport_targets(
        &options.data_dir,
        &topology,
        &local_status,
        "transport certified batch round",
    )?;

    let mut sends = Vec::with_capacity(targets.len());
    for target in targets {
        let send = transport_batch_send_with_retries(
            options.data_dir.clone(),
            options.topology_file.clone(),
            target,
            Some(certification.batch_kind.clone()),
            options.batch_file.clone(),
            Some(certificate_file.clone()),
            options.timeout_ms,
            options.send_retries,
            options.retry_backoff_ms,
        )?;
        sends.push(send);
    }

    let local_receipts = apply_transport_batch(
        &options.data_dir,
        &certification.batch_kind,
        options.batch_file.clone(),
        Some(certificate_file),
        None,
    )
    .map_err(|error| format!("transport certified batch round local apply failed: {error}"))?;
    let local_state_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| {
        format!("transport certified batch round post-apply status failed: {error}")
    })?;
    validate_status_matches_topology(&local_state_status, &topology)?;
    let local_state = transport_hello(&topology, &local_state_status);
    let local_receipt_count = local_receipts.len() as u64;
    let local_accepted_count = local_receipts
        .iter()
        .filter(|receipt| receipt.accepted)
        .count() as u64;
    let local_rejected_count = local_receipt_count.saturating_sub(local_accepted_count);
    let local_apply_verified = local_receipt_count > 0
        && local_rejected_count == 0
        && local_state.block_height == certification.block_height
        && local_state.block_tip_hash != "genesis";
    let all_sends_verified = sends.iter().all(|send| {
        send.verified
            && send.sent.certificate_attached
            && send.ack.certificate_attached
            && send.sent.message_id == send.ack.message_id
            && send.sent.payload_hash == send.ack.payload_hash
            && send.ack.applied
            && send.ack.rejected_count == 0
            && send.ack.state.block_height == certification.block_height
            && send.ack.state.block_tip_hash != "genesis"
    });
    let retry_send_count = sends.iter().filter(|send| send.attempts > 1).count() as u64;
    let retry_error_count = sends
        .iter()
        .map(|send| send.retry_errors.len() as u64)
        .sum();
    let round_ok = certification.round_ok && all_sends_verified && local_apply_verified;

    Ok(TransportCertifiedBatchRoundReport {
        schema: "postfiat-transport-certified-batch-round-v1".to_string(),
        from: local_status.node_id,
        topology_id: topology.topology_id,
        peer_count: sends.len(),
        batch_file: options.batch_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        certification,
        sends,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        retry_send_count,
        retry_error_count,
        local_receipt_count,
        local_accepted_count,
        local_rejected_count,
        local_apply_verified,
        local_state,
        all_sends_verified,
        round_ok,
    })
}

pub(super) fn transport_peer_certified_batch_round(
    options: TransportPeerCertifiedBatchRoundOptions,
) -> Result<TransportPeerCertifiedBatchRoundReport, String> {
    let round_start = Instant::now();
    if options.defer_certified_sends && !options.local_apply_before_certified_send {
        return Err(
            "--defer-certified-sends requires --local-apply-before-certified-send".to_string(),
        );
    }
    if options.send_retries > MAX_TRANSPORT_SEND_RETRIES {
        return Err(format!(
            "--send-retries must be <= {MAX_TRANSPORT_SEND_RETRIES}"
        ));
    }
    let resumed_delivery = resume_durable_certified_send_outbox(
        &options.data_dir,
        &options.topology_file,
        CERTIFIED_SEND_OUTBOX_MAX_JOBS,
    )?;
    if !resumed_delivery.all_completed {
        return Err(format!(
            "certified delivery outbox still has {} pending job(s); refusing a new proposal",
            resumed_delivery.pending
        ));
    }
    let shielded_verifier_prewarm = prewarm_shielded_verifier_cache("peer certified batch round")?;
    let setup_start = Instant::now();
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "peer certified batch round artifact directory create `{}` failed: {error}",
            options.artifact_dir.display()
        )
    })?;
    let proposal_file = options.artifact_dir.join("block-proposal.json");
    let vote_dir = options.artifact_dir.join("votes");
    let certificate_file = options.artifact_dir.join("block-certificate.json");
    std::fs::create_dir_all(&vote_dir).map_err(|error| {
        format!(
            "peer certified batch round vote directory create `{}` failed: {error}",
            vote_dir.display()
        )
    })?;

    let topology = read_topology_file(&options.topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("peer certified batch round status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let setup_ms = monotonic_elapsed_ms(setup_start);

    let proposal_start = Instant::now();
    let proposal_options = BatchProposalOptions {
        data_dir: options.data_dir.clone(),
        verify_block_log: false,
        batch_kind: options.batch_kind.clone(),
        batch_file: options.batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        key_file: options.proposal_key_file.clone(),
        validator_id: None,
    };
    let proposal_with_timings = match options.required_parent.as_ref() {
        Some(required_parent) => {
            propose_batch_with_required_parent_with_timings(proposal_options, required_parent)
        }
        None => propose_batch_with_timings(proposal_options),
    }
    .map_err(|error| format!("peer certified batch round proposal failed: {error}"))?;
    let proposal_breakdown = proposal_with_timings.timings;
    let proposal = proposal_with_timings.proposal;
    let genesis = NodeStore::new(&options.data_dir)
        .read_genesis()
        .map_err(|error| format!("peer certified batch round genesis failed: {error}"))?;
    let consensus_v2_active = consensus_v2_active_at(&genesis, proposal.block_height);
    let consensus_v2_timeout_certificate = if consensus_v2_active && proposal.view > 0 {
        let path = options.timeout_certificate_file.as_ref().ok_or_else(|| {
            "consensus v2 nonzero view omitted timeout certificate file".to_string()
        })?;
        let certificate = verify_block_timeout_certificate_file(
            BlockTimeoutCertificateVerifyOptions {
                data_dir: options.data_dir.clone(),
                verify_block_log: false,
                certificate_file: path.clone(),
            },
        )
        .map_err(|error| format!("consensus v2 timeout certificate read failed: {error}"))?;
        Some(certificate.consensus_v2_certificate.ok_or_else(|| {
            "consensus v2 timeout certificate file omitted v2 certificate".to_string()
        })?)
    } else {
        None
    };
    let consensus_v2_proposal = if consensus_v2_active {
        let proposal_key_file = options.proposal_key_file.as_ref().ok_or_else(|| {
            "consensus v2 activation requires --proposal-key-file".to_string()
        })?;
        Some(
            create_consensus_v2_proposal_for_block(
                &options.data_dir,
                &proposal,
                consensus_v2_timeout_certificate.as_ref(),
                proposal_key_file,
            )
            .map_err(|error| format!("consensus v2 proposal failed: {error}"))?,
        )
    } else {
        None
    };
    if options.require_signed_proposal && proposal.signature.is_none() {
        return Err(
            "peer certified batch round requires signed proposal; pass --proposal-key-file"
                .to_string(),
        );
    }
    if let Some(block_height) = options.block_height {
        if proposal.block_height != block_height {
            return Err(format!(
                "proposed block height {} does not match --height {block_height}",
                proposal.block_height
            ));
        }
    }
    if options.require_local_proposer && proposal.proposer != local_status.node_id {
        return Err(format!(
            "local validator `{}` is not deterministic proposer `{}` for height {} view {}",
            local_status.node_id, proposal.proposer, proposal.block_height, proposal.view
        ));
    }
    let proposal_ms = monotonic_elapsed_ms(proposal_start);

    let target_selection_start = Instant::now();
    let targets = active_transport_targets(
        &options.data_dir,
        &topology,
        &local_status,
        "peer certified batch round",
    )?;
    let target_peer_count = targets.len();
    let vote_request_quorum = bft_quorum_threshold(target_peer_count + 1)
        .map_err(|error| format!("peer certified batch round quorum failed: {error}"))?;
    let required_remote_vote_count = vote_request_quorum.saturating_sub(1);
    let target_selection_ms = monotonic_elapsed_ms(target_selection_start);
    let local_vote_file = vote_dir.join(format!("{}.block_vote.json", local_status.node_id));
    let local_vote_data_dir = options.data_dir.clone();
    let local_vote_key_file = options.key_file.clone();
    let local_vote_validator_id = local_status.node_id.clone();
    let local_vote_proposal = proposal.clone();
    let local_consensus_v2_proposal = consensus_v2_proposal.clone();
    let local_consensus_v2_timeout_certificate = consensus_v2_timeout_certificate.clone();
    let local_vote_block_height = proposal.block_height;
    let local_vote_output_file = local_vote_file.clone();
    let local_vote_handle = std::thread::spawn(move || {
        let local_vote_start = Instant::now();
        let local_vote =
            create_block_vote_for_verified_proposal(BlockVoteForVerifiedProposalOptions {
                data_dir: local_vote_data_dir.clone(),
                verify_block_log: false,
                key_file: local_vote_key_file.clone(),
                validator_id: Some(local_vote_validator_id.clone()),
                proposal: local_vote_proposal,
                block_height: Some(local_vote_block_height),
                vote_file: local_vote_output_file,
            })
            .map_err(|error| format!("peer certified batch round local vote failed: {error}"))?;
        let consensus_v2_vote = match local_consensus_v2_proposal.as_ref() {
            Some(proposal) => Some(
                create_consensus_v2_prepare_vote(
                    &local_vote_data_dir,
                    proposal,
                    local_consensus_v2_timeout_certificate.as_ref(),
                    &local_vote_key_file,
                    &local_vote_validator_id,
                )
                .map_err(|error| {
                    format!("peer certified batch round local v2 prepare failed: {error}")
                })?,
            ),
            None => None,
        };
        Ok::<_, String>((
            local_vote,
            consensus_v2_vote,
            monotonic_elapsed_ms(local_vote_start),
        ))
    });

    let mut vote_requests = Vec::with_capacity(targets.len());
    let mut remote_vote_files = Vec::with_capacity(targets.len());
    let mut vote_request_failures = Vec::new();
    let mut vote_request_timings = Vec::with_capacity(targets.len());
    let mut unresolved_vote_targets = Vec::new();
    let mut vote_request_quorum_early = false;
    let vote_requests_start = Instant::now();
    let quorum_early_vote_collection =
        options.allow_peer_failures || options.quorum_early_full_propagation;
    let vote_request_outcomes = if quorum_early_vote_collection {
        let (sender, receiver) = mpsc::channel();
        let mut pending = targets.iter().cloned().collect::<BTreeSet<_>>();
        // The current sync transport has no cancellable request primitive. In
        // allowed-failure mode, unresolved workers are bounded by per-peer
        // socket timeouts and are recorded as unresolved instead of blocking
        // the quorum certificate path.
        for target in &targets {
            let target = target.clone();
            let vote_file = vote_dir.join(format!("{target}.block_vote.json"));
            let data_dir = options.data_dir.clone();
            let topology_file = options.topology_file.clone();
            let batch_kind = Some(proposal.batch_kind.clone());
            let batch_file = options.batch_file.clone();
            let proposal_file = proposal_file.clone();
            let timeout_certificate_file = options.timeout_certificate_file.clone();
            let block_height = Some(proposal.block_height);
            let timeout_ms = options.timeout_ms;
            let send_retries = options.send_retries;
            let retry_backoff_ms = options.retry_backoff_ms;
            let consensus_v2 = consensus_v2_proposal.clone().map(|proposal| {
                TransportConsensusV2VoteRequest {
                    phase: postfiat_types::ConsensusV2Phase::Prepare,
                    proposal,
                    timeout_certificate: consensus_v2_timeout_certificate.clone(),
                    prepare_qc: None,
                }
            });
            let sender = sender.clone();
            let _ = std::thread::spawn(move || {
                let vote_request_start = Instant::now();
                let result = transport_block_vote_request_with_retries(
                    data_dir,
                    topology_file,
                    target.clone(),
                    batch_kind,
                    batch_file,
                    proposal_file,
                    timeout_certificate_file,
                    vote_file.clone(),
                    block_height,
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    consensus_v2,
                );
                let _ = sender.send(TransportPeerVoteRequestOutcome {
                    target,
                    vote_file,
                    duration_ms: monotonic_elapsed_ms(vote_request_start),
                    result,
                });
            });
        }
        drop(sender);

        let mut outcomes = Vec::with_capacity(targets.len());
        let mut successful_remote_votes = 0usize;
        while !pending.is_empty() {
            let outcome = receiver
                .recv()
                .map_err(|_| "peer block vote request workers exited before quorum".to_string())?;
            pending.remove(&outcome.target);
            if outcome.result.is_ok() {
                successful_remote_votes = successful_remote_votes.saturating_add(1);
            }
            outcomes.push(outcome);
            if successful_remote_votes >= required_remote_vote_count {
                while let Ok(outcome) = receiver.try_recv() {
                    pending.remove(&outcome.target);
                    outcomes.push(outcome);
                }
                vote_request_quorum_early = !pending.is_empty();
                break;
            }
        }
        unresolved_vote_targets = pending.into_iter().collect();
        outcomes
    } else {
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(targets.len());
            for target in &targets {
                let target = target.clone();
                let vote_file = vote_dir.join(format!("{target}.block_vote.json"));
                let data_dir = options.data_dir.clone();
                let topology_file = options.topology_file.clone();
                let batch_kind = Some(proposal.batch_kind.clone());
                let batch_file = options.batch_file.clone();
                let proposal_file = proposal_file.clone();
                let timeout_certificate_file = options.timeout_certificate_file.clone();
                let block_height = Some(proposal.block_height);
                let timeout_ms = options.timeout_ms;
                let send_retries = options.send_retries;
                let retry_backoff_ms = options.retry_backoff_ms;
                let consensus_v2 = consensus_v2_proposal.clone().map(|proposal| {
                    TransportConsensusV2VoteRequest {
                        phase: postfiat_types::ConsensusV2Phase::Prepare,
                        proposal,
                        timeout_certificate: consensus_v2_timeout_certificate.clone(),
                        prepare_qc: None,
                    }
                });
                handles.push(scope.spawn(move || {
                    let vote_request_start = Instant::now();
                    let result = transport_block_vote_request_with_retries(
                        data_dir,
                        topology_file,
                        target.clone(),
                        batch_kind,
                        batch_file,
                        proposal_file,
                        timeout_certificate_file,
                        vote_file.clone(),
                        block_height,
                        timeout_ms,
                        send_retries,
                        retry_backoff_ms,
                        consensus_v2,
                    );
                    TransportPeerVoteRequestOutcome {
                        target,
                        vote_file,
                        duration_ms: monotonic_elapsed_ms(vote_request_start),
                        result,
                    }
                }));
            }
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .map_err(|_| "peer block vote request thread panicked".to_string())
                })
                .collect::<Result<Vec<_>, String>>()
        })?
    };
    let (local_vote, local_consensus_v2_prepare_vote, local_vote_ms) = local_vote_handle
        .join()
        .map_err(|_| "peer certified batch round local vote thread panicked".to_string())??;
    if local_vote.vote.validator != local_status.node_id {
        return Err(format!(
            "local vote validator `{}` did not match source `{}`",
            local_vote.vote.validator, local_status.node_id
        ));
    }
    let mut consensus_v2_prepare_votes = local_consensus_v2_prepare_vote
        .into_iter()
        .collect::<Vec<_>>();
    for outcome in vote_request_outcomes {
        let request = match outcome.result {
            Ok(request) => request,
            Err(error) => {
                vote_request_timings.push(TransportPeerTargetTimingReport {
                    target: outcome.target.clone(),
                    duration_ms: outcome.duration_ms,
                    result: "failed".to_string(),
                    vote_request_breakdown: None,
                });
                if quorum_early_vote_collection {
                    vote_request_failures.push(TransportPeerFailureReport {
                        to: outcome.target,
                        error,
                    });
                    continue;
                }
                return Err(error);
            }
        };
        if request.response.vote.vote.validator != outcome.target {
            let error = format!(
                "remote vote validator `{}` did not match expected `{}`",
                request.response.vote.vote.validator, outcome.target
            );
            vote_request_timings.push(TransportPeerTargetTimingReport {
                target: outcome.target.clone(),
                duration_ms: outcome.duration_ms,
                result: "invalid".to_string(),
                vote_request_breakdown: Some(request.timings.clone()),
            });
            if quorum_early_vote_collection {
                vote_request_failures.push(TransportPeerFailureReport {
                    to: outcome.target,
                    error,
                });
                continue;
            }
            return Err(error);
        }
        if consensus_v2_active {
            let vote = request
                .response
                .consensus_v2_vote
                .clone()
                .ok_or_else(|| {
                    format!(
                        "remote validator `{}` omitted consensus v2 prepare vote",
                        outcome.target
                    )
                })?;
            consensus_v2_prepare_votes.push(vote);
        }
        vote_request_timings.push(TransportPeerTargetTimingReport {
            target: outcome.target,
            duration_ms: outcome.duration_ms,
            result: "ok".to_string(),
            vote_request_breakdown: Some(request.timings.clone()),
        });
        remote_vote_files.push(outcome.vote_file);
        vote_requests.push(request);
    }
    let vote_requests_ms = monotonic_elapsed_ms(vote_requests_start);
    let mut vote_files = Vec::with_capacity(vote_requests.len() + 1);
    vote_files.push(local_vote_file.clone());
    vote_files.extend(remote_vote_files);

    let certificate_start = Instant::now();
    let mut verified_certificate = aggregate_verified_block_certificate(BlockCertificateOptions {
        data_dir: options.data_dir.clone(),
        verify_block_log: false,
        batch_file: Some(options.batch_file.clone()),
        proposal_file: Some(proposal_file.clone()),
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        block_height: Some(proposal.block_height),
        vote_files: vote_files.clone(),
        certificate_file: certificate_file.clone(),
    })
    .map_err(|error| format!("peer certified batch round certificate failed: {error}"))?;
    if let Some(consensus_v2_proposal) = consensus_v2_proposal.as_ref() {
        let prepare_qc = certify_and_persist_consensus_v2_votes(
            &options.data_dir,
            consensus_v2_proposal.round,
            postfiat_types::ConsensusV2Phase::Prepare,
            Some(consensus_v2_proposal.block.clone()),
            consensus_v2_prepare_votes,
        )
        .map_err(|error| format!("consensus v2 prepare QC failed: {error}"))?;
        let precommit_votes = collect_consensus_v2_precommit_votes(
            &options,
            &proposal,
            consensus_v2_proposal,
            consensus_v2_timeout_certificate.as_ref(),
            &prepare_qc,
            &proposal_file,
            &vote_dir,
            &targets,
            &local_status,
        )?;
        let precommit_qc = certify_and_persist_consensus_v2_votes(
            &options.data_dir,
            consensus_v2_proposal.round,
            postfiat_types::ConsensusV2Phase::Precommit,
            Some(consensus_v2_proposal.block.clone()),
            precommit_votes,
        )
        .map_err(|error| format!("consensus v2 precommit QC failed: {error}"))?;
        let commit = assemble_consensus_v2_commit(
            &options.data_dir,
            &proposal,
            consensus_v2_proposal.clone(),
            consensus_v2_timeout_certificate.clone(),
            prepare_qc,
            precommit_qc,
        )
        .map_err(|error| format!("consensus v2 commit assembly failed: {error}"))?;
        verified_certificate = verified_certificate
            .attach_consensus_v2_commit(&options.data_dir, &proposal, commit)
            .map_err(|error| format!("consensus v2 certificate attachment failed: {error}"))?;
        write_consensus_v2_block_certificate_file(
            &certificate_file,
            verified_certificate.as_block_certificate_file(),
        )
        .map_err(|error| format!("consensus v2 certificate write failed: {error}"))?;
    }
    let certificate = verified_certificate.as_block_certificate_file().clone();
    if certificate.certificate.votes.len() < certificate.certificate.quorum {
        return Err("peer certified batch round certificate below quorum".to_string());
    }
    if !options.allow_peer_failures
        && !options.quorum_early_full_propagation
        && certificate.certificate.votes.len() != certificate.certificate.validators.len()
    {
        return Err("peer certified batch round certificate vote count mismatch".to_string());
    }
    let proposal_hash = certificate.proposal_hash.clone().ok_or_else(|| {
        "peer certified batch round certificate did not carry proposal hash".to_string()
    })?;
    let certification = BatchCertificateRoundReport {
        schema: "postfiat.batch_certificate_round.v1".to_string(),
        chain_id: certificate.chain_id.clone(),
        genesis_hash: certificate.genesis_hash.clone(),
        protocol_version: certificate.protocol_version,
        batch_kind: proposal.batch_kind.clone(),
        batch_id: proposal.batch_id.clone(),
        block_height: proposal.block_height,
        view: proposal.view,
        proposal_hash,
        certificate_id: certificate.certificate_id.clone(),
        validators: certificate.certificate.validators.clone(),
        vote_count: certificate.certificate.votes.len(),
        proposal_file: proposal_file.display().to_string(),
        certificate_file: certificate_file.display().to_string(),
        vote_dir: vote_dir.display().to_string(),
        vote_files: vote_files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        private_key_policy: CertificateRoundPrivateKeyPolicy {
            split_key_files: true,
            private_key_material_redacted: true,
        },
        round_ok: true,
    };
    let certificate_ms = monotonic_elapsed_ms(certificate_start);

    let unresolved_vote_target_set = unresolved_vote_targets
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let certified_send_targets = certified_send_targets_for_round(
        &topology,
        &local_status,
        &targets,
        &unresolved_vote_target_set,
        options.quorum_early_full_propagation,
        options.local_apply_before_certified_send,
    );
    let skipped_certified_send_targets = if options.quorum_early_full_propagation {
        Vec::new()
    } else {
        unresolved_vote_targets.clone()
    };
    let mut durable_job_files = BTreeMap::new();
    let expected_block_hash = if options.local_apply_before_certified_send {
        let block_hash = durable_certified_send_expected_block_hash(
            &topology,
            &proposal,
            &certificate,
        )?;
        if certificate
            .block_hash
            .as_ref()
            .is_some_and(|certificate_hash| certificate_hash != &block_hash)
        {
            return Err(
                "certified block hash conflicts with deterministic proposal evidence".to_string(),
            );
        }
        for target in &certified_send_targets {
            let job_file = enqueue_durable_certified_send_job(
                &options.data_dir,
                &topology,
                &local_status.node_id,
                target,
                &certification.batch_kind,
                certification.block_height,
                &certification.certificate_id,
                &block_hash,
                &proposal.state_root,
                &options.batch_file,
                &certificate_file,
                options.timeout_ms,
                options.send_retries,
                options.retry_backoff_ms,
            )?;
            durable_job_files.insert(target.clone(), job_file);
        }
        Some(block_hash)
    } else {
        None
    };
    let mut local_receipts = None;
    let mut local_state = None;
    let mut local_apply_ms = 0.0;
    let mut post_apply_status_ms = 0.0;
    let mut client_visible_finality_ms = 0.0;
    let mut local_apply_breakdown = None;
    if options.local_apply_before_certified_send {
        let local_apply_start = Instant::now();
        let apply_result = apply_transport_batch_with_verified_certificate_with_timings(
            &options.data_dir,
            &certification.batch_kind,
            options.batch_file.clone(),
            certificate_file.clone(),
            verified_certificate.clone(),
        )
        .map_err(|error| format!("peer certified batch round local apply failed: {error}"))?;
        local_apply_ms = monotonic_elapsed_ms(local_apply_start);
        local_apply_breakdown = apply_result.local_apply_breakdown;

        let post_apply_status_start = Instant::now();
        let local_state_status = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })
        .map_err(|error| format!("peer certified batch round post-apply status failed: {error}"))?;
        validate_status_matches_topology(&local_state_status, &topology)?;
        if expected_block_hash.as_deref() != Some(local_state_status.block_tip_hash.as_str())
            || proposal.state_root != local_state_status.state_root
        {
            return Err(
                "peer certified batch round local apply conflicts with durable expected hash/root"
                    .to_string(),
            );
        }
        local_state = Some(transport_hello(&topology, &local_state_status));
        post_apply_status_ms = monotonic_elapsed_ms(post_apply_status_start);
        client_visible_finality_ms = monotonic_elapsed_ms(round_start);
        local_receipts = Some(apply_result.receipts);
    }
    let mut sends = Vec::with_capacity(targets.len());
    let mut send_failures = Vec::new();
    let mut certified_send_timings = Vec::with_capacity(targets.len());
    let mut deferred_certified_send_jobs = Vec::new();
    let certified_sends_start = Instant::now();
    if options.defer_certified_sends {
        for target in &certified_send_targets {
            let send_start = Instant::now();
            let job_file = durable_job_files.get(target).ok_or_else(|| {
                format!("durable certified send job missing for target `{target}`")
            })?;
            match spawn_deferred_certified_batch_send(
                job_file,
                &options.data_dir,
                &options.topology_file,
            ) {
                Ok(job) => {
                    certified_send_timings.push(TransportPeerTargetTimingReport {
                        target: target.clone(),
                        duration_ms: monotonic_elapsed_ms(send_start),
                        result: "deferred".to_string(),
                        vote_request_breakdown: None,
                    });
                    deferred_certified_send_jobs.push(job);
                }
                Err(error) => {
                    certified_send_timings.push(TransportPeerTargetTimingReport {
                        target: target.clone(),
                        duration_ms: monotonic_elapsed_ms(send_start),
                        result: "failed".to_string(),
                        vote_request_breakdown: None,
                    });
                    send_failures.push(TransportPeerFailureReport {
                        to: target.clone(),
                        error,
                    });
                }
            }
        }
    } else {
        let send_outcomes = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(certified_send_targets.len());
            for target in certified_send_targets.clone() {
                let data_dir = options.data_dir.clone();
                let topology_file = options.topology_file.clone();
                let batch_kind = Some(certification.batch_kind.clone());
                let batch_file = options.batch_file.clone();
                let certificate_file = certificate_file.clone();
                let timeout_ms = options.timeout_ms;
                let send_retries = options.send_retries;
                let retry_backoff_ms = options.retry_backoff_ms;
                let durable_job_file = durable_job_files.get(&target).cloned();
                handles.push(scope.spawn(move || {
                    let send_start = Instant::now();
                    let result = match durable_job_file {
                        Some(job_file) => {
                            send_durable_certified_send_job(&job_file, &data_dir, &topology_file)
                        }
                        None => transport_batch_send_with_retries(
                            data_dir,
                            topology_file,
                            target.clone(),
                            batch_kind,
                            batch_file,
                            Some(certificate_file),
                            timeout_ms,
                            send_retries,
                            retry_backoff_ms,
                        ),
                    };
                    TransportPeerBatchSendOutcome {
                        target,
                        duration_ms: monotonic_elapsed_ms(send_start),
                        result,
                    }
                }));
            }
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .map_err(|_| "certified batch send thread panicked".to_string())
                })
                .collect::<Result<Vec<_>, String>>()
        })?;
        for outcome in send_outcomes {
            match outcome.result {
                Ok(send) => {
                    certified_send_timings.push(TransportPeerTargetTimingReport {
                        target: outcome.target,
                        duration_ms: outcome.duration_ms,
                        result: "ok".to_string(),
                        vote_request_breakdown: None,
                    });
                    sends.push(send);
                }
                Err(error) => {
                    certified_send_timings.push(TransportPeerTargetTimingReport {
                        target: outcome.target.clone(),
                        duration_ms: outcome.duration_ms,
                        result: "failed".to_string(),
                        vote_request_breakdown: None,
                    });
                    if options.allow_peer_failures || options.local_apply_before_certified_send {
                        send_failures.push(TransportPeerFailureReport {
                            to: outcome.target,
                            error,
                        });
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }
    let certified_sends_ms = monotonic_elapsed_ms(certified_sends_start);

    if !options.local_apply_before_certified_send {
        let local_apply_start = Instant::now();
        let apply_result = apply_transport_batch_with_verified_certificate_with_timings(
            &options.data_dir,
            &certification.batch_kind,
            options.batch_file.clone(),
            certificate_file.clone(),
            verified_certificate,
        )
        .map_err(|error| format!("peer certified batch round local apply failed: {error}"))?;
        local_apply_ms = monotonic_elapsed_ms(local_apply_start);
        local_apply_breakdown = apply_result.local_apply_breakdown;

        let post_apply_status_start = Instant::now();
        let local_state_status = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })
        .map_err(|error| format!("peer certified batch round post-apply status failed: {error}"))?;
        validate_status_matches_topology(&local_state_status, &topology)?;
        local_state = Some(transport_hello(&topology, &local_state_status));
        post_apply_status_ms = monotonic_elapsed_ms(post_apply_status_start);
        client_visible_finality_ms = monotonic_elapsed_ms(round_start);
        local_receipts = Some(apply_result.receipts);
    }
    let local_receipts = local_receipts
        .ok_or_else(|| "peer certified batch round did not apply locally".to_string())?;
    let local_state =
        local_state.ok_or_else(|| "peer certified batch round missing local state".to_string())?;
    let local_hot_finality = transport_hot_finality_reports(
        &topology,
        &proposal,
        &certificate,
        &local_receipts,
        &local_state,
    )?;

    let verification_start = Instant::now();
    let local_receipt_count = local_receipts.len() as u64;
    let local_accepted_count = local_receipts
        .iter()
        .filter(|receipt| receipt.accepted)
        .count() as u64;
    let local_rejected_count = local_receipt_count.saturating_sub(local_accepted_count);
    let local_apply_verified = local_receipt_count > 0
        && local_rejected_count == 0
        && local_state.block_height == certification.block_height
        && local_state.block_tip_hash != "genesis";
    let all_vote_requests_verified = vote_requests
        .iter()
        .all(|request| request.verified && request.response.verified);
    let vote_request_failures_allowed = vote_request_failures.is_empty()
        || options.allow_peer_failures
        || options.quorum_early_full_propagation;
    let all_sends_verified = if options.defer_certified_sends {
        false
    } else {
        sends.len() == certified_send_targets.len()
            && sends.iter().all(|send| {
                send.verified
                    && send.sent.certificate_attached
                    && send.ack.certificate_attached
                    && send.sent.message_id == send.ack.message_id
                    && send.sent.payload_hash == send.ack.payload_hash
                    && send.ack.applied
                    && send.ack.rejected_count == 0
                    && send.ack.state.block_height == certification.block_height
                    && send.ack.state.block_tip_hash != "genesis"
            })
    };
    let certified_apply_count = if options.defer_certified_sends {
        1
    } else {
        sends.len().saturating_add(1)
    };
    let send_failures_allowed = send_failures.is_empty()
        || (options.allow_peer_failures && !options.local_apply_before_certified_send);
    let retry_vote_request_count = vote_requests
        .iter()
        .filter(|request| request.attempts > 1)
        .count() as u64;
    let retry_vote_request_error_count = vote_requests
        .iter()
        .map(|request| request.retry_errors.len() as u64)
        .sum();
    let retry_send_count = sends.iter().filter(|send| send.attempts > 1).count() as u64;
    let retry_error_count = sends
        .iter()
        .map(|send| send.retry_errors.len() as u64)
        .sum();
    let remote_vote_count = vote_requests.len() as u64;
    let failed_vote_request_count = vote_request_failures.len() as u64;
    let failed_send_count = send_failures.len() as u64;
    let round_ok = certification.round_ok
        && all_vote_requests_verified
        && vote_request_failures_allowed
        && all_sends_verified
        && send_failures_allowed
        && local_apply_verified
        && certification.vote_count >= certificate.certificate.quorum
        && (options.allow_peer_failures
            || options.quorum_early_full_propagation
            || certification.vote_count == certification.validators.len())
        && (!options.allow_peer_failures
            || certified_apply_count >= certificate.certificate.quorum)
        && (!(options.quorum_early_full_propagation || options.local_apply_before_certified_send)
            || certified_apply_count == certified_send_targets.len().saturating_add(1))
        && remote_vote_count + 1 == certification.vote_count as u64;
    let verification_ms = monotonic_elapsed_ms(verification_start);
    let timings = TransportPeerCertifiedBatchRoundTimingsReport {
        total_ms: monotonic_elapsed_ms(round_start),
        shielded_verifier_prewarm,
        setup_ms,
        proposal_ms,
        target_selection_ms,
        local_vote_ms,
        vote_requests_ms,
        certificate_ms,
        certified_sends_ms,
        local_apply_ms,
        post_apply_status_ms,
        client_visible_finality_ms,
        verification_ms,
        proposal_breakdown: Some(proposal_breakdown),
        local_apply_breakdown,
        vote_request_targets: vote_request_timings,
        certified_send_targets: certified_send_timings,
    };

    Ok(TransportPeerCertifiedBatchRoundReport {
        schema: "postfiat-transport-peer-certified-batch-round-v1".to_string(),
        from: local_status.node_id,
        topology_id: topology.topology_id,
        peer_count: sends.len(),
        target_peer_count,
        batch_file: options.batch_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        proposal_signed: proposal.signature.is_some(),
        proposal_signature_signer: proposal
            .signature
            .as_ref()
            .map(|signature| signature.signer.clone()),
        proposal_proposer: proposal.proposer.clone(),
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        certified_sends_deferred: options.defer_certified_sends,
        deferred_certified_send_jobs,
        certification,
        local_vote_file: local_vote_file.display().to_string(),
        vote_requests,
        vote_request_failures,
        unresolved_vote_targets,
        sends,
        send_failures,
        skipped_certified_send_targets,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        vote_request_quorum,
        required_remote_vote_count,
        vote_request_quorum_early,
        retry_vote_request_count,
        retry_vote_request_error_count,
        retry_send_count,
        retry_error_count,
        remote_vote_count,
        failed_vote_request_count,
        failed_send_count,
        local_receipt_count,
        local_accepted_count,
        local_rejected_count,
        local_hot_finality,
        local_apply_verified,
        local_state,
        timings,
        all_vote_requests_verified,
        all_sends_verified,
        round_ok,
    })
}

pub(super) fn transport_peer_certified_mempool_round(
    options: TransportPeerCertifiedMempoolRoundOptions,
) -> Result<TransportPeerCertifiedMempoolRoundReport, String> {
    if options.max_transactions == 0 {
        return Err("--max-transactions must be greater than zero".to_string());
    }
    if options.signed_transfer_file.is_some() && options.signed_transfer_json.is_some() {
        return Err("use only one of --signed-transfer-file or signed_transfer_json".to_string());
    }
    let supplied_inputs = [
        options.signed_transfer_file.is_some(),
        options.signed_transfer_json.is_some(),
        options.signed_payment_v2_json.is_some(),
        options.signed_asset_transaction_json.is_some(),
        options.signed_atomic_swap_transaction_json.is_some(),
        options.signed_escrow_transaction_json.is_some(),
    ]
    .into_iter()
    .filter(|supplied| *supplied)
    .count();
    if supplied_inputs > 1 {
        return Err(
            "use only one of --signed-transfer-file, signed_transfer_json, signed_payment_v2_json, signed_asset_transaction_json, signed_atomic_swap_transaction_json, or signed_escrow_transaction_json"
                .to_string(),
        );
    }
    if let Some(required_parent) = options.required_parent.as_ref() {
        let observed = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })
        .map_err(|error| format!("peer certified mempool parent status failed: {error}"))?;
        if observed.block_height != required_parent.height
            || observed.block_tip_hash != required_parent.block_hash
            || observed.state_root != required_parent.state_root
        {
            return Err(format!(
                "peer certified mempool required parent mismatch before admission: expected height {} hash {} root {}, observed height {} hash {} root {}",
                required_parent.height,
                required_parent.block_hash,
                required_parent.state_root,
                observed.block_height,
                observed.block_tip_hash,
                observed.state_root,
            ));
        }
    }
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "peer certified mempool round artifact directory create `{}` failed: {error}",
            options.artifact_dir.display()
        )
    })?;
    let mut submitted_tx_id = None;
    let mut mempool_submit_ms = 0.0;
    if let Some(signed_transfer_file) = &options.signed_transfer_file {
        let mempool_submit_start = Instant::now();
        let entry = submit_signed_transfer_to_mempool(SignedTransferSubmitOptions {
            data_dir: options.data_dir.clone(),
            transfer_file: signed_transfer_file.clone(),
        })
        .map_err(|error| {
            format!(
                "peer certified mempool round signed transfer submit `{}` failed: {error}",
                signed_transfer_file.display()
            )
        })?;
        mempool_submit_ms = monotonic_elapsed_ms(mempool_submit_start);
        submitted_tx_id = Some(entry.tx_id);
    }
    if let Some(signed_asset_transaction_json) = &options.signed_asset_transaction_json {
        let mempool_submit_start = Instant::now();
        let entry = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: options.data_dir.clone(),
                signed_asset_transaction_json: signed_asset_transaction_json.clone(),
            },
        )
        .map_err(|error| {
            format!(
                "peer certified mempool round signed asset transaction JSON submit failed: {error}"
            )
        })?;
        mempool_submit_ms = monotonic_elapsed_ms(mempool_submit_start);
        submitted_tx_id = Some(entry.tx_id);
    }
    if let Some(signed_atomic_swap_transaction_json) = &options.signed_atomic_swap_transaction_json {
        let mempool_submit_start = Instant::now();
        let entry = submit_signed_atomic_swap_transaction_json_to_mempool(
            SignedAtomicSwapTransactionJsonSubmitOptions {
                data_dir: options.data_dir.clone(),
                signed_atomic_swap_transaction_json: signed_atomic_swap_transaction_json.clone(),
            },
        )
        .map_err(|error| {
            format!(
                "peer certified mempool round signed atomic swap transaction JSON submit failed: {error}"
            )
        })?;
        mempool_submit_ms = monotonic_elapsed_ms(mempool_submit_start);
        submitted_tx_id = Some(entry.tx_id);
    }
    if let Some(signed_escrow_transaction_json) = &options.signed_escrow_transaction_json {
        let mempool_submit_start = Instant::now();
        let entry = submit_signed_escrow_transaction_json_to_mempool(
            SignedEscrowTransactionJsonSubmitOptions {
                data_dir: options.data_dir.clone(),
                signed_escrow_transaction_json: signed_escrow_transaction_json.clone(),
            },
        )
        .map_err(|error| {
            format!(
                "peer certified mempool round signed escrow transaction JSON submit failed: {error}"
            )
        })?;
        mempool_submit_ms = monotonic_elapsed_ms(mempool_submit_start);
        submitted_tx_id = Some(entry.tx_id);
    }
    if let Some(signed_transfer_json) = &options.signed_transfer_json {
        let mempool_submit_start = Instant::now();
        let entry = submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
            data_dir: options.data_dir.clone(),
            signed_transfer_json: signed_transfer_json.clone(),
        })
        .map_err(|error| {
            format!("peer certified mempool round signed transfer JSON submit failed: {error}")
        })?;
        mempool_submit_ms = monotonic_elapsed_ms(mempool_submit_start);
        submitted_tx_id = Some(entry.tx_id);
    }
    if let Some(signed_payment_v2_json) = &options.signed_payment_v2_json {
        let mempool_submit_start = Instant::now();
        let entry = submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
            data_dir: options.data_dir.clone(),
            signed_payment_v2_json: signed_payment_v2_json.clone(),
        })
        .map_err(|error| {
            format!("peer certified mempool round signed payment_v2 JSON submit failed: {error}")
        })?;
        mempool_submit_ms = monotonic_elapsed_ms(mempool_submit_start);
        submitted_tx_id = Some(entry.tx_id);
    }
    let batch_file = options.artifact_dir.join("mempool-batch.json");
    let mempool_batch_start = Instant::now();
    if options.signed_atomic_swap_transaction_json.is_some() {
        let tx_id = submitted_tx_id.clone().ok_or_else(|| {
            "peer certified mempool round atomic submit did not produce a tx_id".to_string()
        })?;
        create_atomic_swap_mempool_batch_for_tx_id(AtomicSwapTargetBatchOptions {
            data_dir: options.data_dir.clone(),
            batch_file: batch_file.clone(),
            tx_id,
        })
        .map_err(|error| {
            format!("peer certified mempool round target atomic batch create failed: {error}")
        })?;
    } else {
        create_mempool_batch(MempoolBatchOptions {
            data_dir: options.data_dir.clone(),
            batch_file: batch_file.clone(),
            max_transactions: options.max_transactions,
        })
        .map_err(|error| format!("peer certified mempool round batch create failed: {error}"))?;
    }
    let mempool_batch_ms = monotonic_elapsed_ms(mempool_batch_start);

    let round_artifact_dir = options.artifact_dir.join("peer-certified-round");
    let round = transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        batch_kind: Some("transparent".to_string()),
        batch_file: batch_file.clone(),
        key_file: options.key_file,
        proposal_key_file: options.proposal_key_file,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        artifact_dir: round_artifact_dir,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file,
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        required_parent: options.required_parent.clone(),
    })?;
    let local_state = status(NodeOptions {
        data_dir: options.data_dir,
    })
    .map_err(|error| format!("peer certified mempool round status failed: {error}"))?;
    Ok(TransportPeerCertifiedMempoolRoundReport {
        schema: "postfiat-transport-peer-certified-mempool-round-v1".to_string(),
        node_id: local_state.node_id,
        topology_id: round.topology_id.clone(),
        batch_file: batch_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        max_transactions: options.max_transactions,
        signed_transfer_file: options
            .signed_transfer_file
            .as_ref()
            .map(|path| path.display().to_string()),
        signed_transfer_json_supplied: options.signed_transfer_json.is_some(),
        signed_payment_v2_json_supplied: options.signed_payment_v2_json.is_some(),
        signed_asset_transaction_json_supplied: options.signed_asset_transaction_json.is_some(),
        signed_atomic_swap_transaction_json_supplied: options
            .signed_atomic_swap_transaction_json
            .is_some(),
        signed_escrow_transaction_json_supplied: options.signed_escrow_transaction_json.is_some(),
        submitted_tx_id,
        mempool_submit_ms,
        mempool_batch_ms,
        round_ok: round.round_ok,
        round,
    })
}

pub(super) fn transport_peer_certified_batch_loop(
    options: TransportPeerCertifiedBatchLoopOptions,
) -> Result<TransportPeerCertifiedBatchLoopReport, String> {
    if options.max_rounds == 0 {
        return Err("--max-rounds must be positive".to_string());
    }
    if options.start_height == 0 {
        return Err("--start-height must be positive".to_string());
    }
    if options.poll_ms == 0 {
        return Err("--poll-ms must be positive".to_string());
    }
    std::fs::create_dir_all(&options.batch_dir).map_err(|error| {
        format!(
            "peer certified batch loop batch directory create `{}` failed: {error}",
            options.batch_dir.display()
        )
    })?;
    std::fs::create_dir_all(&options.artifact_root).map_err(|error| {
        format!(
            "peer certified batch loop artifact root create `{}` failed: {error}",
            options.artifact_root.display()
        )
    })?;

    let topology = read_topology_file(&options.topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("peer certified batch loop status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let shielded_verifier_prewarm = prewarm_shielded_verifier_cache("peer certified batch loop")?;
    if let Some(ready_file) = std::env::var_os(CERTIFIED_BATCH_LOOP_READY_FILE_ENV) {
        let ready_file = PathBuf::from(ready_file);
        if let Some(parent) = ready_file.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "peer certified batch loop ready file parent `{}` create failed: {error}",
                    parent.display()
                )
            })?;
        }
        let ready_report = TransportPeerCertifiedBatchLoopReadyReport {
            schema: "postfiat-transport-peer-certified-batch-loop-ready-v1",
            node_id: &local_status.node_id,
            topology_id: &topology.topology_id,
            batch_dir: options.batch_dir.display().to_string(),
            artifact_root: options.artifact_root.display().to_string(),
            start_height: options.start_height,
            max_rounds: options.max_rounds,
            shielded_verifier_prewarm: &shielded_verifier_prewarm,
        };
        let json = serde_json::to_vec_pretty(&ready_report).map_err(|error| {
            format!("peer certified batch loop ready file serialization failed: {error}")
        })?;
        std::fs::write(&ready_file, [json.as_slice(), b"\n"].concat()).map_err(|error| {
            format!(
                "peer certified batch loop ready file `{}` write failed: {error}",
                ready_file.display()
            )
        })?;
    }

    let mut processed = BTreeSet::new();
    let mut processed_batch_files = Vec::with_capacity(options.max_rounds);
    let mut archived_batch_files = Vec::with_capacity(options.max_rounds);
    let mut rounds = Vec::with_capacity(options.max_rounds);
    let poll_duration = Duration::from_millis(options.poll_ms);
    let idle_timeout =
        (options.idle_timeout_ms > 0).then(|| Duration::from_millis(options.idle_timeout_ms));
    let mut last_progress = Instant::now();
    while rounds.len() < options.max_rounds {
        let next_batch = list_certified_loop_batch_files(&options.batch_dir)?
            .into_iter()
            .find(|path| !processed.contains(&path.display().to_string()));
        let Some(batch_file) = next_batch else {
            if let Some(idle_timeout) = idle_timeout {
                let elapsed = last_progress.elapsed();
                if elapsed >= idle_timeout {
                    break;
                }
                std::thread::sleep(std::cmp::min(poll_duration, idle_timeout - elapsed));
            } else {
                std::thread::sleep(poll_duration);
            }
            continue;
        };
        let offset = u64::try_from(rounds.len())
            .map_err(|_| "peer certified batch loop round count overflow".to_string())?;
        let block_height = options
            .start_height
            .checked_add(offset)
            .ok_or_else(|| "peer certified batch loop block height overflow".to_string())?;
        let artifact_dir = options.artifact_root.join(format!("round-{block_height}"));
        let round =
            transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                data_dir: options.data_dir.clone(),
                topology_file: options.topology_file.clone(),
                batch_kind: options.batch_kind.clone(),
                batch_file: batch_file.clone(),
                key_file: options.key_file.clone(),
                proposal_key_file: options.proposal_key_file.clone(),
                require_local_proposer: options.require_local_proposer,
                require_signed_proposal: options.require_signed_proposal,
                allow_peer_failures: options.allow_peer_failures,
                quorum_early_full_propagation: options.quorum_early_full_propagation,
                artifact_dir,
                block_height: Some(block_height),
                view: None,
                timeout_certificate_file: None,
                timeout_ms: options.timeout_ms,
                send_retries: options.send_retries,
                retry_backoff_ms: options.retry_backoff_ms,
                local_apply_before_certified_send: options.local_apply_before_certified_send,
                defer_certified_sends: options.defer_certified_sends,
                required_parent: None,
            })?;
        let batch_file_display = batch_file.display().to_string();
        let archived_batch_file = if round.round_ok {
            archive_processed_batch_file(&batch_file, options.processed_dir.as_deref())?
        } else {
            None
        };
        processed.insert(batch_file_display.clone());
        processed_batch_files.push(batch_file_display);
        if let Some(archived_batch_file) = archived_batch_file {
            archived_batch_files.push(archived_batch_file);
        }
        rounds.push(round);
        last_progress = Instant::now();
    }
    let loop_ok = rounds.len() == options.max_rounds && rounds.iter().all(|round| round.round_ok);
    let shutdown_reason = if rounds.len() == options.max_rounds {
        "max_rounds"
    } else if idle_timeout.is_some() {
        "idle_timeout"
    } else {
        "stopped"
    }
    .to_string();

    Ok(TransportPeerCertifiedBatchLoopReport {
        schema: "postfiat-transport-peer-certified-batch-loop-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        batch_dir: options.batch_dir.display().to_string(),
        artifact_root: options.artifact_root.display().to_string(),
        processed_dir: options
            .processed_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        max_rounds: options.max_rounds,
        start_height: options.start_height,
        poll_ms: options.poll_ms,
        idle_timeout_ms: options.idle_timeout_ms,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        shielded_verifier_prewarm,
        processed_round_count: rounds.len(),
        shutdown_reason,
        processed_batch_files,
        archived_batch_files,
        rounds,
        loop_ok,
    })
}

pub(super) fn transport_certified_batch_loop(
    options: TransportCertifiedBatchLoopOptions,
) -> Result<TransportCertifiedBatchLoopReport, String> {
    if options.max_rounds == 0 {
        return Err("--max-rounds must be positive".to_string());
    }
    if options.start_height == 0 {
        return Err("--start-height must be positive".to_string());
    }
    if options.poll_ms == 0 {
        return Err("--poll-ms must be positive".to_string());
    }
    std::fs::create_dir_all(&options.batch_dir).map_err(|error| {
        format!(
            "certified batch loop batch directory create `{}` failed: {error}",
            options.batch_dir.display()
        )
    })?;
    std::fs::create_dir_all(&options.artifact_root).map_err(|error| {
        format!(
            "certified batch loop artifact root create `{}` failed: {error}",
            options.artifact_root.display()
        )
    })?;

    let topology = read_topology_file(&options.topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("transport certified batch loop status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;

    let mut processed = BTreeSet::new();
    let mut processed_batch_files = Vec::with_capacity(options.max_rounds);
    let mut archived_batch_files = Vec::with_capacity(options.max_rounds);
    let mut rounds = Vec::with_capacity(options.max_rounds);
    let poll_duration = Duration::from_millis(options.poll_ms);
    let idle_timeout =
        (options.idle_timeout_ms > 0).then(|| Duration::from_millis(options.idle_timeout_ms));
    let mut last_progress = Instant::now();
    while rounds.len() < options.max_rounds {
        let next_batch = list_certified_loop_batch_files(&options.batch_dir)?
            .into_iter()
            .find(|path| !processed.contains(&path.display().to_string()));
        let Some(batch_file) = next_batch else {
            if let Some(idle_timeout) = idle_timeout {
                let elapsed = last_progress.elapsed();
                if elapsed >= idle_timeout {
                    break;
                }
                std::thread::sleep(std::cmp::min(poll_duration, idle_timeout - elapsed));
            } else {
                std::thread::sleep(poll_duration);
            }
            continue;
        };
        let offset = u64::try_from(rounds.len())
            .map_err(|_| "certified batch loop round count overflow".to_string())?;
        let block_height = options
            .start_height
            .checked_add(offset)
            .ok_or_else(|| "certified batch loop block height overflow".to_string())?;
        let artifact_dir = options.artifact_root.join(format!("round-{block_height}"));
        let round = transport_certified_batch_round(TransportCertifiedBatchRoundOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            batch_kind: options.batch_kind.clone(),
            batch_file: batch_file.clone(),
            validator_key_dir: options.validator_key_dir.clone(),
            artifact_dir,
            block_height: Some(block_height),
            view: None,
            timeout_certificate_file: None,
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            skip_block_log_verify: false,
        })?;
        let batch_file_display = batch_file.display().to_string();
        let archived_batch_file = if round.round_ok {
            archive_processed_batch_file(&batch_file, options.processed_dir.as_deref())?
        } else {
            None
        };
        processed.insert(batch_file_display.clone());
        processed_batch_files.push(batch_file_display);
        if let Some(archived_batch_file) = archived_batch_file {
            archived_batch_files.push(archived_batch_file);
        }
        rounds.push(round);
        last_progress = Instant::now();
    }
    let loop_ok = rounds.len() == options.max_rounds && rounds.iter().all(|round| round.round_ok);
    let shutdown_reason = if rounds.len() == options.max_rounds {
        "max_rounds"
    } else if idle_timeout.is_some() {
        "idle_timeout"
    } else {
        "stopped"
    }
    .to_string();

    Ok(TransportCertifiedBatchLoopReport {
        schema: "postfiat-transport-certified-batch-loop-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        batch_dir: options.batch_dir.display().to_string(),
        artifact_root: options.artifact_root.display().to_string(),
        processed_dir: options
            .processed_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        max_rounds: options.max_rounds,
        start_height: options.start_height,
        poll_ms: options.poll_ms,
        idle_timeout_ms: options.idle_timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        processed_round_count: rounds.len(),
        shutdown_reason,
        processed_batch_files,
        archived_batch_files,
        rounds,
        loop_ok,
    })
}

pub(super) fn transport_peer_certified_private_egress_loop(
    options: TransportPeerCertifiedPrivateEgressLoopOptions,
) -> Result<TransportPeerCertifiedPrivateEgressLoopReport, String> {
    if options.max_rounds == 0 {
        return Err("--max-rounds must be positive".to_string());
    }
    if options.start_height == 0 {
        return Err("--start-height must be positive".to_string());
    }
    if options.poll_ms == 0 {
        return Err("--poll-ms must be positive".to_string());
    }
    std::fs::create_dir_all(&options.egress_dir).map_err(|error| {
        format!(
            "peer certified private egress loop egress directory create `{}` failed: {error}",
            options.egress_dir.display()
        )
    })?;
    std::fs::create_dir_all(&options.batch_dir).map_err(|error| {
        format!(
            "peer certified private egress loop batch directory create `{}` failed: {error}",
            options.batch_dir.display()
        )
    })?;
    std::fs::create_dir_all(&options.artifact_root).map_err(|error| {
        format!(
            "peer certified private egress loop artifact root create `{}` failed: {error}",
            options.artifact_root.display()
        )
    })?;

    let topology = read_topology_file(&options.topology_file)?;
    let local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("peer certified private egress loop status failed: {error}"))?;
    validate_status_matches_topology(&local_status, &topology)?;
    let shielded_verifier_prewarm =
        prewarm_shielded_verifier_cache("peer certified private egress loop")?;
    if let Some(ready_file) = options.ready_file.as_ref() {
        write_private_egress_loop_ready_file(
            ready_file,
            &local_status.node_id,
            &topology.topology_id,
            &options,
            &shielded_verifier_prewarm,
        )?;
    }

    let mut processed = BTreeSet::new();
    let mut processed_egress_files = Vec::with_capacity(options.max_rounds);
    let mut archived_egress_files = Vec::with_capacity(options.max_rounds);
    let mut processed_batch_files = Vec::with_capacity(options.max_rounds);
    let mut archived_batch_files = Vec::with_capacity(options.max_rounds);
    let mut rounds = Vec::with_capacity(options.max_rounds);
    let poll_duration = Duration::from_millis(options.poll_ms);
    let idle_timeout =
        (options.idle_timeout_ms > 0).then(|| Duration::from_millis(options.idle_timeout_ms));
    let mut last_progress = Instant::now();
    while rounds.len() < options.max_rounds {
        let next_egress = list_private_egress_loop_files(&options.egress_dir)?
            .into_iter()
            .find(|path| !processed.contains(&path.display().to_string()));
        let Some(egress_file) = next_egress else {
            if let Some(idle_timeout) = idle_timeout {
                let elapsed = last_progress.elapsed();
                if elapsed >= idle_timeout {
                    break;
                }
                std::thread::sleep(std::cmp::min(poll_duration, idle_timeout - elapsed));
            } else {
                std::thread::sleep(poll_duration);
            }
            continue;
        };
        let offset = u64::try_from(rounds.len())
            .map_err(|_| "peer certified private egress loop round count overflow".to_string())?;
        let block_height = options.start_height.checked_add(offset).ok_or_else(|| {
            "peer certified private egress loop block height overflow".to_string()
        })?;
        let egress_file_display = egress_file.display().to_string();
        let batch_file =
            private_egress_loop_batch_file(&options.batch_dir, block_height, &egress_file)?;
        if batch_file.exists() {
            return Err(format!(
                "peer certified private egress loop batch file already exists: {}",
                batch_file.display()
            ));
        }
        let batch_wrap_start = Instant::now();
        create_asset_orchard_private_egress_batch(AssetOrchardPrivateEgressBatchOptions {
            data_dir: options.data_dir.clone(),
            egress_file: egress_file.clone(),
            batch_file: batch_file.clone(),
        })
        .map_err(|error| {
            format!(
                "peer certified private egress loop batch wrap `{}` failed: {error}",
                egress_file.display()
            )
        })?;
        let batch_wrap_ms = monotonic_elapsed_ms(batch_wrap_start);
        let batch_file_display = batch_file.display().to_string();
        let artifact_dir = options.artifact_root.join(format!("round-{block_height}"));
        let artifact_dir_display = artifact_dir.display().to_string();
        let round =
            transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                data_dir: options.data_dir.clone(),
                topology_file: options.topology_file.clone(),
                batch_kind: Some("shielded".to_string()),
                batch_file: batch_file.clone(),
                key_file: options.key_file.clone(),
                proposal_key_file: options.proposal_key_file.clone(),
                require_local_proposer: options.require_local_proposer,
                require_signed_proposal: options.require_signed_proposal,
                allow_peer_failures: options.allow_peer_failures,
                quorum_early_full_propagation: options.quorum_early_full_propagation,
                artifact_dir,
                block_height: Some(block_height),
                view: None,
                timeout_certificate_file: None,
                timeout_ms: options.timeout_ms,
                send_retries: options.send_retries,
                retry_backoff_ms: options.retry_backoff_ms,
                local_apply_before_certified_send: options.local_apply_before_certified_send,
                defer_certified_sends: options.defer_certified_sends,
                required_parent: None,
            })?;
        let archived_egress_file = if round.round_ok {
            archive_processed_batch_file(&egress_file, options.processed_egress_dir.as_deref())?
        } else {
            None
        };
        let archived_batch_file = if round.round_ok {
            archive_processed_batch_file(&batch_file, options.processed_batch_dir.as_deref())?
        } else {
            None
        };
        processed.insert(egress_file_display.clone());
        processed_egress_files.push(egress_file_display.clone());
        processed_batch_files.push(batch_file_display.clone());
        if let Some(path) = archived_egress_file.clone() {
            archived_egress_files.push(path);
        }
        if let Some(path) = archived_batch_file.clone() {
            archived_batch_files.push(path);
        }
        rounds.push(TransportPeerCertifiedPrivateEgressLoopRoundReport {
            block_height,
            egress_file: egress_file_display,
            batch_file: batch_file_display,
            artifact_dir: artifact_dir_display,
            batch_wrap_ms,
            archived_egress_file,
            archived_batch_file,
            round,
        });
        last_progress = Instant::now();
    }
    let loop_ok =
        rounds.len() == options.max_rounds && rounds.iter().all(|round| round.round.round_ok);
    let shutdown_reason = if rounds.len() == options.max_rounds {
        "max_rounds"
    } else if idle_timeout.is_some() {
        "idle_timeout"
    } else {
        "stopped"
    }
    .to_string();

    Ok(TransportPeerCertifiedPrivateEgressLoopReport {
        schema: "postfiat-transport-peer-certified-private-egress-loop-v1".to_string(),
        node_id: local_status.node_id,
        topology_id: topology.topology_id,
        egress_dir: options.egress_dir.display().to_string(),
        batch_dir: options.batch_dir.display().to_string(),
        artifact_root: options.artifact_root.display().to_string(),
        ready_file: options
            .ready_file
            .as_ref()
            .map(|path| path.display().to_string()),
        processed_egress_dir: options
            .processed_egress_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        processed_batch_dir: options
            .processed_batch_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        max_rounds: options.max_rounds,
        start_height: options.start_height,
        poll_ms: options.poll_ms,
        idle_timeout_ms: options.idle_timeout_ms,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        shielded_verifier_prewarm,
        processed_round_count: rounds.len(),
        shutdown_reason,
        processed_egress_files,
        archived_egress_files,
        processed_batch_files,
        archived_batch_files,
        rounds,
        loop_ok,
    })
}

fn write_private_egress_loop_ready_file(
    ready_file: &Path,
    node_id: &str,
    topology_id: &str,
    options: &TransportPeerCertifiedPrivateEgressLoopOptions,
    shielded_verifier_prewarm: &TransportShieldedVerifierPrewarmReport,
) -> Result<(), String> {
    if let Some(parent) = ready_file.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "peer certified private egress loop ready file parent create `{}` failed: {error}",
                parent.display()
            )
        })?;
    }
    let report = serde_json::json!({
        "schema": "postfiat-transport-peer-certified-private-egress-loop-ready-v1",
        "node_id": node_id,
        "topology_id": topology_id,
        "egress_dir": options.egress_dir.display().to_string(),
        "batch_dir": options.batch_dir.display().to_string(),
        "artifact_root": options.artifact_root.display().to_string(),
        "max_rounds": options.max_rounds,
        "start_height": options.start_height,
        "poll_ms": options.poll_ms,
        "idle_timeout_ms": options.idle_timeout_ms,
        "shielded_verifier_prewarm": shielded_verifier_prewarm,
        "ready": true,
    });
    let json = serde_json::to_string_pretty(&report).map_err(|error| {
        format!("peer certified private egress loop ready file serialization failed: {error}")
    })?;
    std::fs::write(ready_file, format!("{json}\n")).map_err(|error| {
        format!(
            "peer certified private egress loop ready file `{}` write failed: {error}",
            ready_file.display()
        )
    })
}

fn archive_processed_batch_file(
    batch_file: &Path,
    processed_dir: Option<&Path>,
) -> Result<Option<String>, String> {
    let Some(processed_dir) = processed_dir else {
        return Ok(None);
    };
    std::fs::create_dir_all(processed_dir).map_err(|error| {
        format!(
            "certified batch loop processed directory create `{}` failed: {error}",
            processed_dir.display()
        )
    })?;
    let file_name = batch_file
        .file_name()
        .ok_or_else(|| {
            format!(
                "certified batch loop batch file `{}` has no file name",
                batch_file.display()
            )
        })?
        .to_os_string();
    let archived_path = processed_dir.join(file_name);
    if archived_path.exists() {
        return Err(format!(
            "certified batch loop processed file already exists: {}",
            archived_path.display()
        ));
    }
    std::fs::rename(batch_file, &archived_path).map_err(|error| {
        format!(
            "certified batch loop archive `{}` to `{}` failed: {error}",
            batch_file.display(),
            archived_path.display()
        )
    })?;
    Ok(Some(archived_path.display().to_string()))
}

fn private_egress_loop_batch_file(
    batch_dir: &Path,
    block_height: u64,
    egress_file: &Path,
) -> Result<PathBuf, String> {
    let component = egress_file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(transport_artifact_component)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "private-egress".to_string());
    Ok(batch_dir.join(format!(
        "round-{block_height}.{component}.private-egress.batch.json"
    )))
}

fn list_private_egress_loop_files(egress_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut egress_files = Vec::new();
    for entry in std::fs::read_dir(egress_dir).map_err(|error| {
        format!(
            "private egress loop directory read `{}` failed: {error}",
            egress_dir.display()
        )
    })? {
        let entry =
            entry.map_err(|error| format!("private egress loop entry read failed: {error}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.ends_with(".json") && !file_name.ends_with(".batch.json") {
            egress_files.push(path);
        }
    }
    egress_files.sort_by_key(|path| path.display().to_string());
    Ok(egress_files)
}

fn list_certified_loop_batch_files(batch_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut batch_files = Vec::new();
    for entry in std::fs::read_dir(batch_dir).map_err(|error| {
        format!(
            "certified batch loop directory read `{}` failed: {error}",
            batch_dir.display()
        )
    })? {
        let entry =
            entry.map_err(|error| format!("certified batch loop entry read failed: {error}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.ends_with(".batch.json") {
            batch_files.push(path);
        }
    }
    batch_files.sort_by_key(|path| path.display().to_string());
    Ok(batch_files)
}
