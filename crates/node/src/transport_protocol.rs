use super::*;

pub(super) fn read_topology_file(path: &PathBuf) -> Result<NetworkTopology, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("topology read `{}` failed: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("topology parse `{}` failed: {error}", path.display()))
}

pub(super) fn read_transport_json_file<T: DeserializeOwned>(
    path: &Path,
    label: &str,
) -> Result<T, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("{label} read `{}` failed: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("{label} parse `{}` failed: {error}", path.display()))
}

pub(super) fn sign_transport_envelope_auth(
    data_dir: &Path,
    topology: &NetworkTopology,
    signer: &str,
    frame: &FramedMessage,
) -> Result<TransportEnvelopeAuth, String> {
    if frame.from != signer {
        return Err(format!(
            "transport auth signer `{signer}` does not match frame sender `{}`",
            frame.from
        ));
    }
    let key_path = data_dir.join(VALIDATOR_KEYS_FILE);
    validate_transport_private_file_permissions(&key_path, "validator key file")?;
    let key_file: ValidatorKeyFile = read_transport_json_file(&key_path, "validator key file")?;
    validate_transport_validator_key_file(&key_file)?;
    let key_record = transport_validator_key_record(&key_file, signer)?;
    let registry = read_transport_validator_registry(data_dir)?;
    let registry_record = transport_validator_registry_record(&registry, signer)?;
    if key_record.algorithm_id != registry_record.algorithm_id
        || key_record.public_key_hex != registry_record.public_key_hex
    {
        return Err(format!(
            "transport auth signer `{signer}` key does not match validator registry"
        ));
    }
    if key_record.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(format!(
            "transport auth signer `{signer}` uses unsupported algorithm `{}`",
            key_record.algorithm_id
        ));
    }
    let private_key = Zeroizing::new(decode_transport_hex(
        "transport auth private key",
        &key_record.private_key_hex,
        None,
    )?);
    let message = transport_auth_message(topology, frame)?;
    let signature = ml_dsa_65_sign_with_context(&private_key, &message, TRANSPORT_AUTH_CONTEXT)
        .map_err(|error| format!("transport auth signing failed: {error}"))?;
    Ok(TransportEnvelopeAuth {
        schema: TRANSPORT_AUTH_SCHEMA.to_string(),
        signer: signer.to_string(),
        algorithm_id: key_record.algorithm_id.clone(),
        public_key_hex: key_record.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    })
}

pub(super) fn validate_transport_envelope_auth(
    auth: Option<&TransportEnvelopeAuth>,
    data_dir: &Path,
    topology: &NetworkTopology,
    frame: &FramedMessage,
) -> Result<(), String> {
    let auth = auth.ok_or_else(|| "transport envelope is missing authentication".to_string())?;
    if auth.schema != TRANSPORT_AUTH_SCHEMA {
        return Err(format!(
            "transport auth schema `{}` is not supported",
            auth.schema
        ));
    }
    if auth.signer != frame.from {
        return Err(format!(
            "transport auth signer `{}` does not match frame sender `{}`",
            auth.signer, frame.from
        ));
    }
    if auth.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(format!(
            "transport auth signer `{}` uses unsupported algorithm `{}`",
            auth.signer, auth.algorithm_id
        ));
    }
    let registry = read_transport_validator_registry(data_dir)?;
    let registry_record = transport_validator_registry_record(&registry, &auth.signer)?;
    if auth.public_key_hex != registry_record.public_key_hex
        || auth.algorithm_id != registry_record.algorithm_id
    {
        return Err(format!(
            "transport auth signer `{}` does not match validator registry",
            auth.signer
        ));
    }
    let public_key = decode_transport_hex(
        "transport auth public key",
        &auth.public_key_hex,
        Some(ML_DSA_65_PUBLIC_KEY_BYTES),
    )?;
    let signature = decode_transport_hex(
        "transport auth signature",
        &auth.signature_hex,
        Some(ML_DSA_65_SIGNATURE_BYTES),
    )?;
    let message = transport_auth_message(topology, frame)?;
    if !ml_dsa_65_verify_with_context(&public_key, &message, &signature, TRANSPORT_AUTH_CONTEXT) {
        return Err(format!(
            "transport auth signature mismatch for `{}`",
            auth.signer
        ));
    }
    Ok(())
}

pub(super) fn transport_auth_message(
    topology: &NetworkTopology,
    frame: &FramedMessage,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&(
        TRANSPORT_AUTH_SCHEMA,
        topology.topology_id.as_str(),
        topology.chain_id.as_str(),
        topology.genesis_hash.as_str(),
        topology.protocol_version,
        frame,
    ))
    .map_err(|error| format!("transport auth message serialization failed: {error}"))
}

pub(super) fn read_transport_validator_registry(
    data_dir: &Path,
) -> Result<ValidatorRegistry, String> {
    read_transport_json_file(
        &data_dir.join(VALIDATOR_REGISTRY_FILE),
        "validator registry",
    )
}

#[cfg(unix)]
pub(super) fn validate_transport_private_file_permissions(
    path: &Path,
    label: &str,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("{label} inspect `{}` failed: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "{label} `{}` is not a regular file",
            path.display()
        ));
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode != 0o600 {
        return Err(format!(
            "{label} `{}` has mode {:o}, expected 600",
            path.display(),
            mode
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn validate_transport_private_file_permissions(
    path: &Path,
    label: &str,
) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("{label} inspect `{}` failed: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "{label} `{}` is not a regular file",
            path.display()
        ));
    }
    Ok(())
}

pub(super) fn validate_transport_validator_key_file(
    key_file: &ValidatorKeyFile,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for record in &key_file.validators {
        if !seen.insert(record.node_id.as_str()) {
            return Err(format!("duplicate validator key `{}`", record.node_id));
        }
        if record.algorithm_id != ML_DSA_65_ALGORITHM {
            return Err(format!(
                "validator key `{}` uses unsupported algorithm `{}`",
                record.node_id, record.algorithm_id
            ));
        }
        let public_key = hex_to_bytes(&record.public_key_hex).map_err(|error| {
            format!(
                "validator key `{}` public key invalid: {error}",
                record.node_id
            )
        })?;
        if public_key.len() != ML_DSA_65_PUBLIC_KEY_BYTES {
            return Err(format!(
                "validator key `{}` public key has {} bytes, expected {}",
                record.node_id,
                public_key.len(),
                ML_DSA_65_PUBLIC_KEY_BYTES
            ));
        }
        let private_key = hex_to_bytes(&record.private_key_hex).map_err(|error| {
            format!(
                "validator key `{}` private key invalid: {error}",
                record.node_id
            )
        })?;
        if private_key.is_empty() {
            return Err(format!(
                "validator key `{}` private key must not be empty",
                record.node_id
            ));
        }
    }
    Ok(())
}

pub(super) fn transport_validator_key_record<'a>(
    key_file: &'a ValidatorKeyFile,
    node_id: &str,
) -> Result<&'a ValidatorKeyRecord, String> {
    key_file
        .validators
        .iter()
        .find(|record| record.node_id == node_id)
        .ok_or_else(|| format!("missing validator key `{node_id}`"))
}

pub(super) fn transport_validator_registry_record<'a>(
    registry: &'a ValidatorRegistry,
    node_id: &str,
) -> Result<&'a postfiat_node::ValidatorRegistryRecord, String> {
    registry
        .validators
        .iter()
        .find(|record| record.node_id == node_id)
        .ok_or_else(|| format!("missing validator registry key `{node_id}`"))
}

pub(super) fn decode_transport_hex(
    label: &str,
    value: &str,
    expected_bytes: Option<usize>,
) -> Result<Vec<u8>, String> {
    if let Some(expected_bytes) = expected_bytes {
        let expected_hex_len = expected_bytes.saturating_mul(2);
        if value.len() != expected_hex_len {
            return Err(format!(
                "{label} has length {}, expected {expected_hex_len}",
                value.len()
            ));
        }
    }
    hex_to_bytes(value).map_err(|error| format!("{label} has invalid hex: {error}"))
}

pub(super) fn read_transport_payload_file(path: &PathBuf) -> Result<String, String> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "transport payload metadata `{}` failed: {error}",
            path.display()
        )
    })?;
    if metadata.len() > MAX_TRANSPORT_FRAME_BYTES {
        return Err(format!(
            "transport payload `{}` is too large: {} bytes",
            path.display(),
            metadata.len()
        ));
    }
    std::fs::read_to_string(path).map_err(|error| {
        format!(
            "transport payload read `{}` failed: {error}",
            path.display()
        )
    })
}

pub(super) fn transport_batch_frame_payload(
    payload_json: &str,
    certificate_json: Option<&str>,
    batch_kind: &str,
) -> Result<Vec<u8>, String> {
    validate_transport_batch_kind(batch_kind)?;
    if let Some(certificate_json) = certificate_json {
        let payload = TransportCertifiedBatchPayload {
            schema: TRANSPORT_CERTIFIED_BATCH_PAYLOAD_SCHEMA,
            batch_kind,
            payload_json,
            certificate_json,
        };
        return serde_json::to_vec(&payload)
            .map_err(|error| format!("certified transport payload serialization failed: {error}"));
    }
    if batch_kind != "transparent" {
        return Err(format!(
            "uncertified transport batch kind `{batch_kind}` is not supported"
        ));
    }
    Ok(payload_json.as_bytes().to_vec())
}

pub(super) fn transport_block_vote_request_payload(
    block_height: u64,
    view: u64,
    batch_kind: &str,
    batch_json: &str,
    proposal_json: &str,
    timeout_certificate_json: Option<&str>,
    consensus_v2: Option<&TransportConsensusV2VoteRequest>,
) -> Result<Vec<u8>, String> {
    let payload = TransportBlockVoteRequestPayload {
        schema: TRANSPORT_BLOCK_VOTE_REQUEST_SCHEMA,
        block_height,
        view,
        batch_kind,
        batch_json,
        proposal_json,
        timeout_certificate_json,
        consensus_v2,
    };
    serde_json::to_vec(&payload).map_err(|error| {
        format!("transport block vote request payload serialization failed: {error}")
    })
}

pub(super) fn read_transport_line(stream: &TcpStream, context: &str) -> Result<String, String> {
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();
    let limit = MAX_TRANSPORT_FRAME_BYTES + 1;
    let read = reader
        .by_ref()
        .take(limit)
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("{context} failed: {error}"))?;
    if read == 0 {
        return Err(format!("{context} returned empty frame"));
    }
    if bytes.len() as u64 > MAX_TRANSPORT_FRAME_BYTES {
        return Err(format!(
            "{context} exceeded {MAX_TRANSPORT_FRAME_BYTES} bytes"
        ));
    }
    if !bytes.ends_with(b"\n") {
        return Err(format!("{context} missing newline terminator"));
    }
    String::from_utf8(bytes).map_err(|error| format!("{context} was not UTF-8: {error}"))
}

pub(super) fn read_rpc_line<R: BufRead>(reader: &mut R, context: &str) -> Result<String, String> {
    let mut bytes = Vec::new();
    let max_line_bytes = MAX_RPC_REQUEST_BYTES as u64 + 1;
    let limit = max_line_bytes + 1;
    let read = reader
        .take(limit)
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("{context} failed: {error}"))?;
    if read == 0 {
        return Err(format!("{context} returned empty frame"));
    }
    if bytes.len() as u64 > max_line_bytes {
        return Err(format!(
            "{context} exceeded {MAX_RPC_REQUEST_BYTES} request bytes"
        ));
    }
    if !bytes.ends_with(b"\n") {
        return Err(format!("{context} missing newline terminator"));
    }
    String::from_utf8(bytes).map_err(|error| format!("{context} was not UTF-8: {error}"))
}

pub(super) fn set_stream_timeout(stream: &TcpStream, timeout_ms: u64) -> Result<(), String> {
    stream
        .set_nodelay(true)
        .map_err(|error| format!("transport set TCP_NODELAY failed: {error}"))?;
    let timeout = Duration::from_millis(timeout_ms);
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("transport set read timeout failed: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("transport set write timeout failed: {error}"))
}

pub(super) fn transport_hello(topology: &NetworkTopology, status: &StatusReport) -> TransportHello {
    TransportHello {
        schema: TRANSPORT_HELLO_SCHEMA.to_string(),
        topology_id: topology.topology_id.clone(),
        node_id: status.node_id.clone(),
        chain_id: status.chain_id.clone(),
        genesis_hash: status.genesis_hash.clone(),
        protocol_version: status.protocol_version,
        state_root: status.state_root.clone(),
        block_height: status.block_height,
        block_tip_hash: status.block_tip_hash.clone(),
    }
}

pub(super) fn write_transport_hello(
    stream: &mut TcpStream,
    hello: &TransportHello,
) -> Result<(), String> {
    write_json_line(stream, hello)
}

pub(super) fn write_json_line<T: serde::Serialize>(
    stream: &mut TcpStream,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(value)
        .map_err(|error| format!("transport serialization failed: {error}"))?;
    if json.len() as u64 > MAX_TRANSPORT_FRAME_BYTES {
        return Err(format!(
            "transport frame exceeded {MAX_TRANSPORT_FRAME_BYTES} bytes"
        ));
    }
    stream
        .write_all(json.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .map_err(|error| format!("transport write failed: {error}"))
}

pub(super) fn open_transport_event_log(path: &std::path::Path) -> Result<std::fs::File, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("transport event log directory create failed: {error}"))?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("transport event log open failed: {error}"))
}

pub(super) fn write_event_log_line<T: serde::Serialize>(
    writer: &mut std::fs::File,
    value: &T,
) -> Result<(), String> {
    let json = serde_json::to_string(value)
        .map_err(|error| format!("transport event serialization failed: {error}"))?;
    writer
        .write_all(json.as_bytes())
        .and_then(|_| writer.write_all(b"\n"))
        .and_then(|_| writer.flush())
        .map_err(|error| format!("transport event log write failed: {error}"))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_transport_validator_event(
    event_writer: &mut Option<std::fs::File>,
    local_status: &StatusReport,
    topology: &NetworkTopology,
    connection_index: u64,
    kind: &str,
    batch_ack: Option<TransportBatchAck>,
    block_vote_response: Option<TransportBlockVoteResponse>,
    rejection: Option<TransportValidatorServeRejection>,
) -> Result<(), String> {
    if let Some(writer) = event_writer.as_mut() {
        let outcome = if rejection.is_some() {
            "rejected"
        } else {
            "accepted"
        };
        let event = TransportValidatorServeEvent {
            schema: "postfiat-transport-validator-serve-event-v1".to_string(),
            node_id: local_status.node_id.clone(),
            topology_id: topology.topology_id.clone(),
            connection_index,
            kind: kind.to_string(),
            outcome: outcome.to_string(),
            batch_ack,
            block_vote_response,
            rejection,
        };
        write_event_log_line(writer, &event)?;
    }
    Ok(())
}

pub(super) fn transport_validator_rejection(
    data_dir: &Path,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    connection_index: u64,
    kind: &str,
    error: String,
) -> Result<TransportValidatorServeRejection, String> {
    let state_after = status(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .map_err(|error| format!("transport validator service rejection status failed: {error}"))?;
    Ok(TransportValidatorServeRejection {
        schema: "postfiat-transport-validator-serve-rejection-v1".to_string(),
        node_id: local_status.node_id.clone(),
        topology_id: topology.topology_id.clone(),
        connection_index,
        kind: kind.to_string(),
        error,
        state: transport_hello(topology, &state_after),
    })
}

pub(super) fn transport_envelope_schema(line: &str) -> Result<String, String> {
    if line.trim().is_empty() {
        return Err("transport envelope was empty".to_string());
    }
    let value = serde_json::from_str::<serde_json::Value>(line)
        .map_err(|error| format!("transport envelope parse failed: {error}"))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "transport envelope is missing string schema".to_string())?;
    if schema.trim().is_empty() {
        return Err("transport envelope schema is empty".to_string());
    }
    Ok(schema.to_string())
}

pub(super) fn parse_transport_hello(line: &str) -> Result<TransportHello, String> {
    if line.trim().is_empty() {
        return Err("transport hello was empty".to_string());
    }
    serde_json::from_str(line).map_err(|error| format!("transport hello parse failed: {error}"))
}

pub(super) fn parse_transport_batch_envelope(line: &str) -> Result<TransportBatchEnvelope, String> {
    if line.trim().is_empty() {
        return Err("transport batch envelope was empty".to_string());
    }
    serde_json::from_str(line)
        .map_err(|error| format!("transport batch envelope parse failed: {error}"))
}

pub(super) fn parse_transport_batch_ack(line: &str) -> Result<TransportBatchAck, String> {
    if line.trim().is_empty() {
        return Err("transport batch ack was empty".to_string());
    }
    let schema = transport_envelope_schema(line)?;
    if schema == "postfiat-transport-validator-serve-rejection-v1" {
        let rejection: TransportValidatorServeRejection = serde_json::from_str(line)
            .map_err(|error| format!("transport validator rejection parse failed: {error}"))?;
        return Err(format!(
            "transport batch rejected by `{}` for `{}`: {}",
            rejection.node_id, rejection.kind, rejection.error
        ));
    }
    if schema != TRANSPORT_BATCH_ACK_SCHEMA {
        return Err(format!(
            "transport batch ack schema `{schema}` is not supported"
        ));
    }
    serde_json::from_str(line).map_err(|error| format!("transport batch ack parse failed: {error}"))
}

pub(super) fn parse_transport_block_vote_request(
    line: &str,
) -> Result<TransportBlockVoteRequestEnvelope, String> {
    if line.trim().is_empty() {
        return Err("transport block vote request was empty".to_string());
    }
    serde_json::from_str(line)
        .map_err(|error| format!("transport block vote request parse failed: {error}"))
}

pub(super) fn parse_transport_block_vote_response(
    line: &str,
) -> Result<TransportBlockVoteResponse, String> {
    if line.trim().is_empty() {
        return Err("transport block vote response was empty".to_string());
    }
    let schema = transport_envelope_schema(line)?;
    if schema == "postfiat-transport-validator-serve-rejection-v1" {
        let rejection: TransportValidatorServeRejection = serde_json::from_str(line)
            .map_err(|error| format!("transport validator rejection parse failed: {error}"))?;
        return Err(format!(
            "transport block vote rejected by `{}` for `{}`: {}",
            rejection.node_id, rejection.kind, rejection.error
        ));
    }
    if schema != TRANSPORT_BLOCK_VOTE_RESPONSE_SCHEMA {
        return Err(format!(
            "transport block vote response schema `{schema}` is not supported"
        ));
    }
    serde_json::from_str(line)
        .map_err(|error| format!("transport block vote response parse failed: {error}"))
}

pub(super) fn proposal_height_view(proposal: &serde_json::Value) -> Result<(u64, u64), String> {
    let block_height = proposal
        .get("block_height")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            "transport block vote request proposal is missing numeric block_height".to_string()
        })?;
    let view = proposal
        .get("view")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            "transport block vote request proposal is missing numeric view".to_string()
        })?;
    Ok((block_height, view))
}

pub(super) fn validate_status_matches_topology(
    status: &StatusReport,
    topology: &NetworkTopology,
) -> Result<(), String> {
    if status.chain_id != topology.chain_id
        || status.genesis_hash != topology.genesis_hash
        || status.protocol_version != topology.protocol_version
    {
        return Err(format!(
            "local node `{}` does not match topology domain",
            status.node_id
        ));
    }
    if topology.peer(&status.node_id).is_none() {
        return Err(format!(
            "local node `{}` is not in topology",
            status.node_id
        ));
    }
    Ok(())
}

pub(super) fn active_transport_targets(
    data_dir: &Path,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    context: &str,
) -> Result<Vec<String>, String> {
    let active_validators = active_validator_ids_for_node(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .map_err(|error| format!("{context} active validator read failed: {error}"))?;
    if !active_validators
        .iter()
        .any(|validator| validator == &local_status.node_id)
    {
        return Err(format!(
            "{context} local node `{}` is not in active validator set",
            local_status.node_id
        ));
    }
    let mut targets = Vec::with_capacity(active_validators.len().saturating_sub(1));
    for validator in active_validators {
        if topology.peer(&validator).is_none() {
            return Err(format!(
                "{context} active validator `{validator}` is not in topology"
            ));
        }
        if validator != local_status.node_id {
            targets.push(validator);
        }
    }
    if targets.is_empty() {
        return Err(format!("{context} requires at least one active peer"));
    }
    Ok(targets)
}

pub(super) fn certified_send_targets_for_round(
    topology: &NetworkTopology,
    local_status: &StatusReport,
    vote_targets: &[String],
    unresolved_vote_target_set: &BTreeSet<String>,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
) -> Vec<String> {
    let mut targets = if quorum_early_full_propagation || local_apply_before_certified_send {
        vote_targets.to_vec()
    } else {
        vote_targets
            .iter()
            .filter(|target| !unresolved_vote_target_set.contains(*target))
            .cloned()
            .collect::<Vec<_>>()
    };

    if !local_apply_before_certified_send {
        if let Some(local_peer) = topology.peer(&local_status.node_id) {
            if !transport_peer_host_is_loopback(&local_peer.host)
                && !targets.iter().any(|target| target == &local_status.node_id)
            {
                targets.push(local_status.node_id.clone());
            }
        }
    }
    targets
}

pub(super) fn transport_peer_host_is_loopback(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .map(|addr| addr.is_loopback())
        .unwrap_or(false)
}

pub(super) fn validate_transport_hello(
    hello: &TransportHello,
    topology: &NetworkTopology,
    local_status: &StatusReport,
) -> Result<(), String> {
    if hello.schema != TRANSPORT_HELLO_SCHEMA {
        return Err(format!(
            "transport hello schema `{}` is not supported",
            hello.schema
        ));
    }
    if hello.topology_id != topology.topology_id {
        return Err("transport hello topology id mismatch".to_string());
    }
    if topology.peer(&hello.node_id).is_none() {
        return Err(format!(
            "transport hello node `{}` is not in topology",
            hello.node_id
        ));
    }
    if hello.chain_id != local_status.chain_id
        || hello.genesis_hash != local_status.genesis_hash
        || hello.protocol_version != local_status.protocol_version
    {
        return Err(format!(
            "transport hello from `{}` does not match local chain domain",
            hello.node_id
        ));
    }
    Ok(())
}

pub(super) fn validate_transport_batch_envelope(
    envelope: &TransportBatchEnvelope,
    topology: &NetworkTopology,
    local_status: &StatusReport,
) -> Result<(), String> {
    if envelope.schema != TRANSPORT_BATCH_SCHEMA {
        return Err(format!(
            "transport batch schema `{}` is not supported",
            envelope.schema
        ));
    }
    if envelope.topology_id != topology.topology_id {
        return Err("transport batch topology id mismatch".to_string());
    }
    validate_transport_batch_kind(&envelope.batch_kind)?;
    if envelope.certificate_json.is_none() {
        return Err(
            "transport batch envelope is missing certificate; uncertified service apply is disabled"
                .to_string(),
        );
    }
    if envelope.frame.topic != TRANSPORT_BATCH_TOPIC {
        return Err(format!(
            "transport batch topic `{}` is not supported",
            envelope.frame.topic
        ));
    }
    if envelope.frame.to.as_deref() != Some(local_status.node_id.as_str()) {
        return Err(format!(
            "transport batch target mismatch: expected `{}` got `{:?}`",
            local_status.node_id, envelope.frame.to
        ));
    }
    if topology.peer(&envelope.frame.from).is_none() {
        return Err(format!(
            "transport batch sender `{}` is not in topology",
            envelope.frame.from
        ));
    }
    serde_json::from_str::<serde_json::Value>(&envelope.payload_json)
        .map_err(|error| format!("transport batch payload is not valid JSON: {error}"))?;
    if let Some(certificate_json) = envelope.certificate_json.as_ref() {
        serde_json::from_str::<serde_json::Value>(certificate_json)
            .map_err(|error| format!("transport batch certificate is not valid JSON: {error}"))?;
    }
    let domain = network_domain_from_topology(topology);
    let framed_payload = transport_batch_frame_payload(
        &envelope.payload_json,
        envelope.certificate_json.as_deref(),
        &envelope.batch_kind,
    )?;
    if !verify_message_payload(&domain, &envelope.frame, &framed_payload) {
        return Err("transport batch payload hash or message id mismatch".to_string());
    }
    Ok(())
}

pub(super) fn validate_transport_batch_ack(
    ack: &TransportBatchAck,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    to: &str,
    envelope: &TransportBatchEnvelope,
) -> Result<(), String> {
    if ack.schema != TRANSPORT_BATCH_ACK_SCHEMA {
        return Err(format!(
            "transport batch ack schema `{}` is not supported",
            ack.schema
        ));
    }
    if ack.topology_id != topology.topology_id {
        return Err("transport batch ack topology id mismatch".to_string());
    }
    if ack.from != to || ack.to != local_status.node_id {
        return Err("transport batch ack route mismatch".to_string());
    }
    if ack.message_id != envelope.frame.message_id
        || ack.payload_hash != envelope.frame.payload_hash
    {
        return Err("transport batch ack message mismatch".to_string());
    }
    if ack.certificate_attached != envelope.certificate_json.is_some() {
        return Err("transport batch ack certificate attachment mismatch".to_string());
    }
    if !ack.applied || ack.receipt_count == 0 {
        return Err("transport batch ack did not apply payload".to_string());
    }
    validate_transport_hello(&ack.state, topology, local_status)?;
    if ack.state.node_id != to {
        return Err(format!(
            "transport batch ack state came from `{}`, expected `{to}`",
            ack.state.node_id
        ));
    }
    if let Some(certified_state) = ack.certified_state.as_ref() {
        validate_transport_hello(certified_state, topology, local_status)?;
        if certified_state.node_id != to || certified_state.block_height > ack.state.block_height {
            return Err(
                "transport batch ack certified state is not a valid target high-water mark"
                    .to_string(),
            );
        }
    }
    Ok(())
}

pub(super) fn validate_transport_block_vote_request(
    envelope: &TransportBlockVoteRequestEnvelope,
    topology: &NetworkTopology,
    local_status: &StatusReport,
) -> Result<(), String> {
    if envelope.schema != TRANSPORT_BLOCK_VOTE_REQUEST_SCHEMA {
        return Err(format!(
            "transport block vote request schema `{}` is not supported",
            envelope.schema
        ));
    }
    if envelope.topology_id != topology.topology_id {
        return Err("transport block vote request topology id mismatch".to_string());
    }
    if envelope.frame.topic != TRANSPORT_BLOCK_VOTE_TOPIC {
        return Err(format!(
            "transport block vote request topic `{}` is not supported",
            envelope.frame.topic
        ));
    }
    if envelope.frame.to.as_deref() != Some(local_status.node_id.as_str()) {
        return Err(format!(
            "transport block vote request target mismatch: expected `{}` got `{:?}`",
            local_status.node_id, envelope.frame.to
        ));
    }
    if topology.peer(&envelope.frame.from).is_none() {
        return Err(format!(
            "transport block vote request sender `{}` is not in topology",
            envelope.frame.from
        ));
    }
    if envelope.block_height == 0 {
        return Err("transport block vote request block height must be positive".to_string());
    }
    if !is_supported_transport_batch_kind(&envelope.batch_kind) {
        return Err(format!(
            "transport block vote request batch kind `{}` is not supported",
            envelope.batch_kind
        ));
    }
    serde_json::from_str::<serde_json::Value>(&envelope.batch_json).map_err(|error| {
        format!("transport block vote request batch is not valid JSON: {error}")
    })?;
    let proposal =
        serde_json::from_str::<serde_json::Value>(&envelope.proposal_json).map_err(|error| {
            format!("transport block vote request proposal is not valid JSON: {error}")
        })?;
    if let Some(timeout_certificate_json) = envelope.timeout_certificate_json.as_ref() {
        serde_json::from_str::<serde_json::Value>(timeout_certificate_json).map_err(|error| {
            format!("transport block vote request timeout certificate is not valid JSON: {error}")
        })?;
    }
    let (proposal_height, proposal_view) = proposal_height_view(&proposal)?;
    if proposal_height != envelope.block_height {
        return Err(format!(
            "transport block vote request proposal height {proposal_height} does not match envelope height {}",
            envelope.block_height
        ));
    }
    if proposal_view != envelope.view {
        return Err(format!(
            "transport block vote request proposal view {proposal_view} does not match envelope view {}",
            envelope.view
        ));
    }
    let proposal_kind = proposal
        .get("batch_kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "transport block vote request proposal is missing string batch_kind".to_string()
        })?;
    if proposal_kind != envelope.batch_kind {
        return Err(format!(
            "transport block vote request proposal batch kind `{proposal_kind}` does not match envelope kind `{}`",
            envelope.batch_kind
        ));
    }
    if let Some(request) = envelope.consensus_v2.as_ref() {
        if request.proposal.round.height != envelope.block_height
            || request.proposal.round.view != envelope.view
        {
            return Err(
                "transport consensus v2 proposal round does not match envelope".to_string(),
            );
        }
        match request.phase {
            postfiat_types::ConsensusV2Phase::Prepare if request.prepare_qc.is_some() => {
                return Err("transport consensus v2 prepare request carried prepare QC".to_string())
            }
            postfiat_types::ConsensusV2Phase::Precommit if request.prepare_qc.is_none() => {
                return Err(
                    "transport consensus v2 precommit request omitted prepare QC".to_string(),
                )
            }
            _ => {}
        }
    }
    let domain = network_domain_from_topology(topology);
    let framed_payload = transport_block_vote_request_payload(
        envelope.block_height,
        envelope.view,
        &envelope.batch_kind,
        &envelope.batch_json,
        &envelope.proposal_json,
        envelope.timeout_certificate_json.as_deref(),
        envelope.consensus_v2.as_ref(),
    )?;
    if !verify_message_payload(&domain, &envelope.frame, &framed_payload) {
        return Err("transport block vote request payload hash or message id mismatch".to_string());
    }
    Ok(())
}

pub(super) fn validate_signed_proposal_policy(
    proposal_json: &str,
    require_signed: bool,
) -> Result<(), String> {
    if !require_signed {
        return Ok(());
    }
    let proposal = serde_json::from_str::<serde_json::Value>(proposal_json).map_err(|error| {
        format!("transport block vote request proposal is not valid JSON: {error}")
    })?;
    if proposal
        .get("signature")
        .is_some_and(|signature| !signature.is_null())
    {
        return Ok(());
    }
    Err("transport block vote request requires signed proposal".to_string())
}

pub(super) fn validate_transport_block_vote_response(
    response: &TransportBlockVoteResponse,
    topology: &NetworkTopology,
    local_status: &StatusReport,
    to: &str,
    envelope: &TransportBlockVoteRequestEnvelope,
) -> Result<(), String> {
    if response.schema != TRANSPORT_BLOCK_VOTE_RESPONSE_SCHEMA {
        return Err(format!(
            "transport block vote response schema `{}` is not supported",
            response.schema
        ));
    }
    if response.topology_id != topology.topology_id {
        return Err("transport block vote response topology id mismatch".to_string());
    }
    if response.from != to || response.to != local_status.node_id {
        return Err("transport block vote response route mismatch".to_string());
    }
    if response.message_id != envelope.frame.message_id
        || response.payload_hash != envelope.frame.payload_hash
    {
        return Err("transport block vote response message mismatch".to_string());
    }
    if response.block_height != envelope.block_height
        || response.vote.block_height != envelope.block_height
    {
        return Err("transport block vote response height mismatch".to_string());
    }
    if response.view != envelope.view || response.vote.view != envelope.view {
        return Err("transport block vote response view mismatch".to_string());
    }
    if response.vote.chain_id != local_status.chain_id
        || response.vote.genesis_hash != local_status.genesis_hash
        || response.vote.protocol_version != local_status.protocol_version
    {
        return Err("transport block vote response vote domain mismatch".to_string());
    }
    if response.vote.schema != "postfiat.block_vote.v1" {
        return Err(format!(
            "transport block vote response vote schema `{}` is not supported",
            response.vote.schema
        ));
    }
    if response.vote.block_hash.is_some() || response.vote.proposal_hash.is_none() {
        return Err(
            "transport block vote response must contain a proposal vote, not a committed block vote"
                .to_string(),
        );
    }
    if response.vote.vote.validator != to || !response.vote.vote.accept {
        return Err("transport block vote response vote validator mismatch".to_string());
    }
    match (
        envelope.consensus_v2.as_ref(),
        response.consensus_v2_vote.as_ref(),
    ) {
        (None, None) => {}
        (Some(request), Some(vote))
            if vote.validator == to
                && vote.round == request.proposal.round
                && vote.phase == request.phase
                && vote.block.as_ref() == Some(&request.proposal.block) => {}
        (Some(_), Some(_)) => {
            return Err("transport consensus v2 response vote target mismatch".to_string())
        }
        (Some(_), None) => {
            return Err("transport consensus v2 response omitted vote".to_string())
        }
        (None, Some(_)) => {
            return Err("transport response carried unsolicited consensus v2 vote".to_string())
        }
    }
    if response.vote_file.trim().is_empty() {
        return Err("transport block vote response vote file is empty".to_string());
    }
    if !response.verified {
        return Err("transport block vote response was not verified by signer".to_string());
    }
    validate_transport_hello(&response.state, topology, local_status)?;
    if response.state.node_id != to {
        return Err(format!(
            "transport block vote response state came from `{}`, expected `{to}`",
            response.state.node_id
        ));
    }
    Ok(())
}

pub(super) fn default_transport_batch_kind() -> String {
    "transparent".to_string()
}

pub(super) fn validate_transport_batch_kind(batch_kind: &str) -> Result<(), String> {
    if !is_supported_transport_batch_kind(batch_kind) {
        return Err(format!(
            "transport batch kind `{batch_kind}` is not supported"
        ));
    }
    Ok(())
}

pub(super) fn is_supported_transport_batch_kind(batch_kind: &str) -> bool {
    matches!(
        batch_kind,
        "transparent" | "governance" | "shielded" | "bridge"
    )
}

pub(super) fn transport_batch_ack(
    topology: &NetworkTopology,
    from: &str,
    envelope: &TransportBatchEnvelope,
    state_after: &StatusReport,
    receipts: &[postfiat_types::Receipt],
) -> TransportBatchAck {
    let accepted_count = receipts.iter().filter(|receipt| receipt.accepted).count() as u64;
    let receipt_count = receipts.len() as u64;
    let state = transport_hello(topology, state_after);
    TransportBatchAck {
        schema: TRANSPORT_BATCH_ACK_SCHEMA.to_string(),
        topology_id: topology.topology_id.clone(),
        from: from.to_string(),
        to: envelope.frame.from.clone(),
        message_id: envelope.frame.message_id.clone(),
        payload_hash: envelope.frame.payload_hash.clone(),
        applied: true,
        already_applied: false,
        receipt_count,
        accepted_count,
        rejected_count: receipt_count.saturating_sub(accepted_count),
        certificate_attached: envelope.certificate_json.is_some(),
        certified_state: Some(state.clone()),
        state,
    }
}

pub(super) fn transport_already_applied_ack(
    data_dir: &Path,
    topology: &NetworkTopology,
    from: &str,
    envelope: &TransportBatchEnvelope,
    state_after: &StatusReport,
) -> Result<TransportBatchAck, String> {
    let payload: serde_json::Value = serde_json::from_str(&envelope.payload_json)
        .map_err(|error| format!("already-applied batch payload parse failed: {error}"))?;
    let batch_id = payload
        .get("batch_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "already-applied batch payload lacks batch_id".to_string())?;
    let certificate_json = envelope
        .certificate_json
        .as_deref()
        .ok_or_else(|| "already-applied batch requires a certificate".to_string())?;
    let certificate: BlockCertificateFile = serde_json::from_str(certificate_json)
        .map_err(|error| format!("already-applied certificate parse failed: {error}"))?;
    let blocks = postfiat_storage::NodeStore::new(data_dir)
        .read_blocks()
        .map_err(|error| format!("already-applied block log read failed: {error}"))?;
    let block = blocks
        .blocks
        .iter()
        .find(|block| block.header.batch_id == batch_id)
        .ok_or_else(|| format!("already-applied batch `{batch_id}` is absent from block log"))?;
    if block.header.height != certificate.block_height
        || block.header.certificate_id != certificate.certificate_id
        || certificate
            .block_hash
            .as_deref()
            .is_some_and(|hash| hash != block.header.block_hash)
    {
        return Err(format!(
            "already-applied batch `{batch_id}` conflicts with certified height/hash"
        ));
    }
    if state_after.block_height == block.header.height
        && (state_after.state_root != block.header.state_root
            || state_after.block_tip_hash != block.header.block_hash)
    {
        return Err(format!(
            "already-applied batch `{batch_id}` conflicts with current height/root"
        ));
    }
    let receipt_count = block.receipt_ids.len() as u64;
    if receipt_count == 0 {
        return Err(format!(
            "already-applied batch `{batch_id}` has no recorded receipts"
        ));
    }
    let certified_state = TransportHello {
        schema: TRANSPORT_HELLO_SCHEMA.to_string(),
        topology_id: topology.topology_id.clone(),
        node_id: from.to_string(),
        chain_id: state_after.chain_id.clone(),
        genesis_hash: state_after.genesis_hash.clone(),
        protocol_version: state_after.protocol_version,
        state_root: block.header.state_root.clone(),
        block_height: block.header.height,
        block_tip_hash: block.header.block_hash.clone(),
    };
    Ok(TransportBatchAck {
        schema: TRANSPORT_BATCH_ACK_SCHEMA.to_string(),
        topology_id: topology.topology_id.clone(),
        from: from.to_string(),
        to: envelope.frame.from.clone(),
        message_id: envelope.frame.message_id.clone(),
        payload_hash: envelope.frame.payload_hash.clone(),
        applied: true,
        already_applied: true,
        receipt_count,
        accepted_count: receipt_count,
        rejected_count: 0,
        certificate_attached: true,
        certified_state: Some(certified_state),
        state: transport_hello(topology, state_after),
    })
}

pub(super) fn transport_hot_finality_reports(
    topology: &NetworkTopology,
    proposal: &BlockProposalFile,
    certificate: &BlockCertificateFile,
    receipts: &[Receipt],
    local_state: &TransportHello,
) -> Result<Vec<TxFinalityReport>, String> {
    let receipt_ids = receipts
        .iter()
        .map(|receipt| receipt.tx_id.clone())
        .collect::<Vec<_>>();
    if receipt_ids != proposal.receipt_ids {
        return Err("transport hot finality receipt ids do not match proposal".to_string());
    }
    if local_state.block_tip_hash == "genesis" {
        return Err("transport hot finality local state missing block tip hash".to_string());
    }
    let block = BlockRecord {
        header: BlockHeader {
            height: proposal.block_height,
            view: proposal.view,
            parent_hash: proposal.parent_hash.clone(),
            proposer: proposal.proposer.clone(),
            batch_kind: proposal.batch_kind.clone(),
            batch_id: proposal.batch_id.clone(),
            state_root: proposal.state_root.clone(),
            bridge_exit_root: proposal.bridge_exit_root.clone(),
            receipt_count: proposal.receipt_count,
            certificate_id: certificate.certificate_id.clone(),
            certificate: certificate.certificate.clone(),
            consensus_v2_commit: certificate.consensus_v2_commit.clone(),
            block_hash: local_state.block_tip_hash.clone(),
        },
        receipt_ids,
        fastpay_pre_state_effects: proposal.fastpay_pre_state_effects.clone(),
    };

    receipts
        .iter()
        .enumerate()
        .map(|(receipt_index, receipt)| {
            let receipt_index = receipt_index as u64;
            let proof_id =
                transport_hot_tx_finality_proof_id(topology, receipt, receipt_index, &block)?;
            Ok(TxFinalityReport {
                schema: "postfiat-tx-finality-v1".to_string(),
                proof_id,
                chain_id: topology.chain_id.clone(),
                genesis_hash: topology.genesis_hash.clone(),
                protocol_version: topology.protocol_version,
                tx_id: receipt.tx_id.clone(),
                confirmed: true,
                verification_mode: "selected-block-hot-path".to_string(),
                receipt: receipt.clone(),
                receipt_index,
                receipt_count: block.receipt_ids.len() as u64,
                block: block.clone(),
                block_log_verified: false,
                block_count: local_state.block_height,
                tip_hash: local_state.block_tip_hash.clone(),
                tip_state_root: local_state.state_root.clone(),
            })
        })
        .collect()
}

pub(super) fn transport_hot_tx_finality_proof_id(
    topology: &NetworkTopology,
    receipt: &Receipt,
    receipt_index: u64,
    block: &BlockRecord,
) -> Result<String, String> {
    if block.fastpay_pre_state_effects.is_empty() {
        let encoded = serde_json::to_vec(&(
            topology.chain_id.as_str(),
            topology.genesis_hash.as_str(),
            topology.protocol_version,
            receipt,
            receipt_index,
            &block.header,
            &block.receipt_ids,
        ))
        .map_err(|error| format!("transport hot finality proof encode failed: {error}"))?;
        return Ok(hash_hex("postfiat.tx_finality.v1", &encoded));
    }
    let encoded = serde_json::to_vec(&(
        topology.chain_id.as_str(),
        topology.genesis_hash.as_str(),
        topology.protocol_version,
        receipt,
        receipt_index,
        &block.header,
        &block.receipt_ids,
        &block.fastpay_pre_state_effects,
    ))
    .map_err(|error| format!("transport hot finality proof encode failed: {error}"))?;
    Ok(hash_hex("postfiat.tx_finality.v2", &encoded))
}

pub(super) fn write_transport_batch_payload(
    data_dir: &std::path::Path,
    envelope: &TransportBatchEnvelope,
) -> Result<TransportBatchInboxFiles, String> {
    let inbox_dir = data_dir.join("transport_inbox");
    std::fs::create_dir_all(&inbox_dir)
        .map_err(|error| format!("transport inbox create failed: {error}"))?;
    let batch_file = inbox_dir.join(format!("{}.batch.json", envelope.frame.message_id));
    std::fs::write(&batch_file, envelope.payload_json.as_bytes())
        .map_err(|error| format!("transport batch payload write failed: {error}"))?;
    let certificate_file = if let Some(certificate_json) = envelope.certificate_json.as_ref() {
        let certificate_file =
            inbox_dir.join(format!("{}.certificate.json", envelope.frame.message_id));
        std::fs::write(&certificate_file, certificate_json.as_bytes())
            .map_err(|error| format!("transport batch certificate write failed: {error}"))?;
        Some(certificate_file)
    } else {
        None
    };
    Ok(TransportBatchInboxFiles {
        batch_file,
        certificate_file,
    })
}

pub(super) fn network_domain_from_topology(topology: &NetworkTopology) -> NetworkDomain {
    NetworkDomain {
        chain_id: topology.chain_id.clone(),
        genesis_hash: topology.genesis_hash.clone(),
        protocol_version: topology.protocol_version,
    }
}

pub(super) fn socket_address(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

pub(super) fn connect_transport_stream(
    peer_address: &str,
    timeout_ms: u64,
    context: &str,
) -> Result<TcpStream, String> {
    let addresses = peer_address
        .to_socket_addrs()
        .map_err(|error| format!("{context} `{peer_address}` resolve failed: {error}"))?
        .collect::<Vec<SocketAddr>>();
    if addresses.is_empty() {
        return Err(format!(
            "{context} `{peer_address}` resolved no socket addresses"
        ));
    }
    let connect_timeout_ms = timeout_ms.min(TRANSPORT_CONNECT_TIMEOUT_MAX_MS);
    let timeout = Duration::from_millis(connect_timeout_ms);
    let mut errors = Vec::with_capacity(addresses.len());
    for address in addresses {
        match TcpStream::connect_timeout(&address, timeout) {
            Ok(stream) => return Ok(stream),
            Err(error) => errors.push(format!("{address}: {error}")),
        }
    }
    Err(format!(
        "{context} `{peer_address}` failed after {connect_timeout_ms}ms connect timeout: {}",
        errors.join("; ")
    ))
}

pub(super) fn validate_controlled_transport_bind_host(host: &str) -> Result<(), String> {
    validate_controlled_transport_bind_host_with_override(host, false)
}

pub(super) fn validate_controlled_transport_bind_host_with_override(
    host: &str,
    _legacy_allow_public: bool,
) -> Result<(), String> {
    if is_private_transport_bind_host(host) {
        return Ok(());
    }
    Err(format!(
        "transport listener host `{host}` would expose plaintext unauthenticated traffic; direct public and wildcard binds are disabled, so bind to loopback behind an authenticated TLS edge or to a private overlay address"
    ))
}

pub(super) fn is_private_transport_bind_host(host: &str) -> bool {
    let normalized = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if normalized == "localhost" {
        return true;
    }
    if normalized.is_empty() || normalized == "*" {
        return false;
    }
    match normalized.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(address)) => {
            address.is_loopback() || address.is_private() || address.is_link_local()
        }
        Ok(std::net::IpAddr::V6(address)) => {
            address.is_loopback()
                || is_ipv6_unique_local(&address)
                || is_ipv6_unicast_link_local(&address)
        }
        Err(_) => false,
    }
}

pub(super) fn is_ipv6_unique_local(address: &std::net::Ipv6Addr) -> bool {
    (address.segments()[0] & 0xfe00) == 0xfc00
}

pub(super) fn is_ipv6_unicast_link_local(address: &std::net::Ipv6Addr) -> bool {
    (address.segments()[0] & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod transport_cli_tests {
    use super::*;

    fn test_status(node_id: &str) -> StatusReport {
        StatusReport {
            chain_id: "postfiat-wan-devnet".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 1,
            rpc_schema: "postfiat-local-rpc-v1".to_string(),
            build_git_revision: "test-revision".to_string(),
            build_profile: "test".to_string(),
            active_nav_profiles: Vec::new(),
            deployment_manifest_sha256: None,
            deployment_validator_id: None,
            deployment_service_artifacts: Vec::new(),
            deployment_runtime_artifacts: None,
            validator_count: 3,
            node_id: node_id.to_string(),
            status: "running".to_string(),
            last_run_unix: 0,
            state_root: "22".repeat(48),
            block_height: 10,
            block_tip_hash: "33".repeat(48),
            mempool_pending: 0,
        }
    }

    fn test_topology(local_host: &str) -> NetworkTopology {
        NetworkTopology {
            topology_id: "test-topology".to_string(),
            chain_id: "postfiat-wan-devnet".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 1,
            peers: vec![
                postfiat_network::PeerInfo {
                    node_id: "validator-0".to_string(),
                    host: local_host.to_string(),
                    p2p_port: 26650,
                    rpc_port: 27650,
                    p2p_address: format!("/ip4/{local_host}/tcp/26650"),
                },
                postfiat_network::PeerInfo {
                    node_id: "validator-1".to_string(),
                    host: "203.0.113.11".to_string(),
                    p2p_port: 26651,
                    rpc_port: 27651,
                    p2p_address: "/ip4/203.0.113.11/tcp/26651".to_string(),
                },
                postfiat_network::PeerInfo {
                    node_id: "validator-2".to_string(),
                    host: "203.0.113.12".to_string(),
                    p2p_port: 26652,
                    rpc_port: 27652,
                    p2p_address: "/ip4/203.0.113.12/tcp/26652".to_string(),
                },
            ],
        }
    }

    fn test_prewarm_report() -> TransportShieldedVerifierPrewarmReport {
        TransportShieldedVerifierPrewarmReport {
            schema: "postfiat-transport-shielded-verifier-prewarm-v1".to_string(),
            requested: true,
            total_ms: 123.0,
            asset_orchard_swap_verifier_warm: true,
            asset_orchard_swap_verifier_ms: Some(120.0),
            asset_orchard_private_egress_verifier_warm: true,
            asset_orchard_private_egress_verifier_ms: Some(3.0),
            asset_orchard_private_egress_verifier_breakdown: None,
        }
    }

    fn unique_transport_test_ready_file(test_name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock must be after epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "postfiat-{test_name}-{}-{nanos}",
                std::process::id()
            ))
            .join("ready.json")
    }

    #[test]
    fn transport_startup_after_prewarm_blocks_bind_until_prewarm_ready() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{mpsc, Arc};

        let (event_tx, event_rx) = mpsc::channel::<&'static str>();
        let (release_prewarm_tx, release_prewarm_rx) = mpsc::channel::<()>();
        let prewarm_done = Arc::new(AtomicBool::new(false));
        let bind_done = Arc::new(AtomicBool::new(false));

        let prewarm_done_for_prewarm = Arc::clone(&prewarm_done);
        let prewarm_done_for_bind = Arc::clone(&prewarm_done);
        let prewarm_done_for_ready = Arc::clone(&prewarm_done);
        let bind_done_for_bind = Arc::clone(&bind_done);
        let bind_done_for_ready = Arc::clone(&bind_done);
        let prewarm_event_tx = event_tx.clone();
        let bind_event_tx = event_tx.clone();
        let ready_event_tx = event_tx;

        let handle = std::thread::spawn(move || {
            transport_startup_after_prewarm(
                || {
                    prewarm_event_tx.send("prewarm-started").unwrap();
                    release_prewarm_rx.recv().unwrap();
                    prewarm_done_for_prewarm.store(true, Ordering::SeqCst);
                    prewarm_event_tx.send("prewarm-done").unwrap();
                    Ok(test_prewarm_report())
                },
                || {
                    assert!(prewarm_done_for_bind.load(Ordering::SeqCst));
                    bind_done_for_bind.store(true, Ordering::SeqCst);
                    bind_event_tx.send("bind").unwrap();
                    Ok("listener")
                },
                |shielded_verifier_prewarm| {
                    assert!(shielded_verifier_prewarm.requested);
                    assert!(prewarm_done_for_ready.load(Ordering::SeqCst));
                    assert!(bind_done_for_ready.load(Ordering::SeqCst));
                    ready_event_tx.send("ready").unwrap();
                    Ok(())
                },
            )
        });

        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("prewarm must start"),
            "prewarm-started"
        );
        assert!(
            event_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "bind or ready marker ran before forced-slow prewarm completed"
        );
        release_prewarm_tx.send(()).unwrap();
        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("prewarm must finish"),
            "prewarm-done"
        );
        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("bind must run after prewarm"),
            "bind"
        );
        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("ready marker must write after bind"),
            "ready"
        );
        let (listener, report) = handle
            .join()
            .expect("startup thread must not panic")
            .unwrap();
        assert_eq!(listener, "listener");
        assert!(report.requested);
    }

    #[test]
    fn transport_startup_fails_closed_on_partial_warmup_or_oom() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let partial_bind_called = Arc::new(AtomicBool::new(false));
        let partial_ready_called = Arc::new(AtomicBool::new(false));
        let partial_bind_for_closure = Arc::clone(&partial_bind_called);
        let partial_ready_for_closure = Arc::clone(&partial_ready_called);
        let mut partial = test_prewarm_report();
        partial.asset_orchard_private_egress_verifier_warm = false;
        let partial_error = transport_startup_after_prewarm(
            || Ok(partial),
            || {
                partial_bind_for_closure.store(true, Ordering::SeqCst);
                Ok("listener")
            },
            |_| {
                partial_ready_for_closure.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .expect_err("partial verifier warmup must fail before bind");
        assert!(partial_error.contains("warm swap and warm private-egress"));
        assert!(!partial_bind_called.load(Ordering::SeqCst));
        assert!(!partial_ready_called.load(Ordering::SeqCst));

        let oom_bind_called = Arc::new(AtomicBool::new(false));
        let oom_ready_called = Arc::new(AtomicBool::new(false));
        let oom_bind_for_closure = Arc::clone(&oom_bind_called);
        let oom_ready_for_closure = Arc::clone(&oom_ready_called);
        let oom_error = transport_startup_after_prewarm(
            || Err("verifier prewarm failed: out of memory".to_string()),
            || {
                oom_bind_for_closure.store(true, Ordering::SeqCst);
                Ok("listener")
            },
            |_| {
                oom_ready_for_closure.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .expect_err("OOM must fail before bind");
        assert!(oom_error.contains("out of memory"));
        assert!(!oom_bind_called.load(Ordering::SeqCst));
        assert!(!oom_ready_called.load(Ordering::SeqCst));
    }

    #[test]
    fn transport_startup_clears_stale_ready_file_until_new_ready_after_bind() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{mpsc, Arc};

        let ready_file = unique_transport_test_ready_file("startup-clears-stale-ready-file");
        let ready_dir = ready_file
            .parent()
            .expect("ready file must have parent")
            .to_path_buf();
        std::fs::create_dir_all(&ready_dir).expect("test ready dir create");
        std::fs::write(&ready_file, b"stale\n").expect("stale ready file write");
        assert!(ready_file.exists());

        clear_transport_ready_file(&ready_file, "test transport startup")
            .expect("stale ready file must clear before prewarm");
        assert!(
            !ready_file.exists(),
            "stale ready file must be absent before prewarm starts"
        );

        let (event_tx, event_rx) = mpsc::channel::<&'static str>();
        let (release_prewarm_tx, release_prewarm_rx) = mpsc::channel::<()>();
        let prewarm_done = Arc::new(AtomicBool::new(false));
        let bind_done = Arc::new(AtomicBool::new(false));

        let prewarm_ready_file = ready_file.clone();
        let ready_file_for_ready = ready_file.clone();
        let prewarm_done_for_prewarm = Arc::clone(&prewarm_done);
        let prewarm_done_for_bind = Arc::clone(&prewarm_done);
        let prewarm_done_for_ready = Arc::clone(&prewarm_done);
        let bind_done_for_bind = Arc::clone(&bind_done);
        let bind_done_for_ready = Arc::clone(&bind_done);
        let prewarm_event_tx = event_tx.clone();
        let bind_event_tx = event_tx.clone();
        let ready_event_tx = event_tx;

        let handle = std::thread::spawn(move || {
            transport_startup_after_prewarm(
                || {
                    assert!(
                        !prewarm_ready_file.exists(),
                        "stale ready file must not pass gate during prewarm"
                    );
                    prewarm_event_tx.send("prewarm-started").unwrap();
                    release_prewarm_rx.recv().unwrap();
                    prewarm_done_for_prewarm.store(true, Ordering::SeqCst);
                    prewarm_event_tx.send("prewarm-done").unwrap();
                    Ok(test_prewarm_report())
                },
                || {
                    assert!(prewarm_done_for_bind.load(Ordering::SeqCst));
                    bind_done_for_bind.store(true, Ordering::SeqCst);
                    bind_event_tx.send("bind").unwrap();
                    Ok("listener")
                },
                |shielded_verifier_prewarm| {
                    assert!(shielded_verifier_prewarm.requested);
                    assert!(prewarm_done_for_ready.load(Ordering::SeqCst));
                    assert!(bind_done_for_ready.load(Ordering::SeqCst));
                    let ready_report = serde_json::json!({
                        "schema": "postfiat-test-transport-ready-v1",
                        "shielded_verifier_prewarm": shielded_verifier_prewarm,
                    });
                    write_transport_ready_file(
                        &ready_file_for_ready,
                        &ready_report,
                        "test transport startup",
                    )?;
                    ready_event_tx.send("ready").unwrap();
                    Ok(())
                },
            )
        });

        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("prewarm must start"),
            "prewarm-started"
        );
        assert!(
            !ready_file.exists(),
            "ready file must remain absent while forced-slow prewarm is blocked"
        );
        assert!(
            event_rx.recv_timeout(Duration::from_millis(100)).is_err(),
            "bind or ready marker ran before forced-slow prewarm completed"
        );
        release_prewarm_tx.send(()).unwrap();
        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("prewarm must finish"),
            "prewarm-done"
        );
        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("bind must run after prewarm"),
            "bind"
        );
        assert!(
            !ready_file.exists(),
            "ready file must not reappear before ready write"
        );
        assert_eq!(
            event_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("ready marker must write after bind"),
            "ready"
        );
        let (listener, report) = handle
            .join()
            .expect("startup thread must not panic")
            .unwrap();
        assert_eq!(listener, "listener");
        assert!(report.requested);
        let ready_json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&ready_file).expect("ready file must exist after bind"),
        )
        .expect("ready file must be valid JSON");
        assert_eq!(
            ready_json
                .get("schema")
                .and_then(|value| value.as_str())
                .expect("ready schema"),
            "postfiat-test-transport-ready-v1"
        );
        let _ = std::fs::remove_dir_all(&ready_dir);
    }

    #[test]
    fn certified_send_targets_include_non_loopback_local_public_peer() {
        let topology = test_topology("198.51.100.10");
        let status = test_status("validator-0");
        let vote_targets = vec!["validator-1".to_string(), "validator-2".to_string()];
        let unresolved = BTreeSet::new();

        let targets = certified_send_targets_for_round(
            &topology,
            &status,
            &vote_targets,
            &unresolved,
            false,
            false,
        );

        assert_eq!(
            targets,
            vec![
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-0".to_string()
            ]
        );
    }

    #[test]
    fn certified_send_targets_skip_non_loopback_local_after_local_apply() {
        let topology = test_topology("198.51.100.10");
        let status = test_status("validator-0");
        let vote_targets = vec!["validator-1".to_string(), "validator-2".to_string()];
        let unresolved = BTreeSet::new();

        let targets = certified_send_targets_for_round(
            &topology,
            &status,
            &vote_targets,
            &unresolved,
            false,
            true,
        );

        assert_eq!(
            targets,
            vec!["validator-1".to_string(), "validator-2".to_string()]
        );
    }

    #[test]
    fn certified_send_targets_do_not_duplicate_loopback_local_peer() {
        let topology = test_topology("127.0.0.1");
        let status = test_status("validator-0");
        let vote_targets = vec!["validator-1".to_string(), "validator-2".to_string()];
        let unresolved = BTreeSet::new();

        let targets = certified_send_targets_for_round(
            &topology,
            &status,
            &vote_targets,
            &unresolved,
            false,
            false,
        );

        assert_eq!(
            targets,
            vec!["validator-1".to_string(), "validator-2".to_string()]
        );
    }

    #[test]
    fn certified_send_targets_keep_unresolved_vote_target_skipped() {
        let topology = test_topology("198.51.100.10");
        let status = test_status("validator-0");
        let vote_targets = vec!["validator-1".to_string(), "validator-2".to_string()];
        let unresolved = BTreeSet::from(["validator-2".to_string()]);

        let targets = certified_send_targets_for_round(
            &topology,
            &status,
            &vote_targets,
            &unresolved,
            false,
            false,
        );

        assert_eq!(
            targets,
            vec!["validator-1".to_string(), "validator-0".to_string()]
        );
    }

    #[test]
    fn certified_send_targets_include_unresolved_under_all_delivery_policy() {
        let topology = test_topology("198.51.100.10");
        let status = test_status("validator-0");
        let vote_targets = vec!["validator-1".to_string(), "validator-2".to_string()];
        let unresolved = BTreeSet::from(["validator-2".to_string()]);

        let targets = certified_send_targets_for_round(
            &topology,
            &status,
            &vote_targets,
            &unresolved,
            false,
            true,
        );

        assert_eq!(targets, vote_targets);
    }

    #[test]
    fn transport_batch_ack_reports_validator_rejection() {
        let topology = test_topology("127.0.0.1");
        let status = test_status("validator-0");
        let rejection = serde_json::json!({
            "schema": "postfiat-transport-validator-serve-rejection-v1",
            "node_id": "validator-0",
            "topology_id": topology.topology_id,
            "connection_index": 7,
            "kind": "batch",
            "error": "transport batch service apply failed: external block certificate height 1348 does not match block 1344",
            "state": transport_hello(&topology, &status)
        });

        let error = parse_transport_batch_ack(&rejection.to_string())
            .expect_err("validator rejection must not be parsed as a batch ack");

        assert!(
            error.contains("transport batch rejected by `validator-0` for `batch`"),
            "{error}"
        );
        assert!(
            error.contains("external block certificate height 1348 does not match block 1344"),
            "{error}"
        );
    }

    fn already_applied_test_fixture(
        certificate_height: u64,
    ) -> (
        PathBuf,
        NetworkTopology,
        StatusReport,
        TransportBatchEnvelope,
    ) {
        let root = unique_transport_test_ready_file("already-applied").with_extension("data");
        std::fs::create_dir_all(&root).expect("create already-applied test root");
        let blocks: postfiat_types::BlockLog = serde_json::from_value(serde_json::json!({
            "blocks": [{
                "header": {
                    "height": 10,
                    "view": 0,
                    "parent_hash": "parent",
                    "proposer": "validator-0",
                    "batch_kind": "transparent",
                    "batch_id": "batch-1",
                    "state_root": "22".repeat(48),
                    "receipt_count": 1,
                    "certificate_id": "certificate-1",
                    "certificate": {"validators": ["validator-0", "validator-1", "validator-2"], "quorum": 3, "votes": []},
                    "block_hash": "33".repeat(48)
                },
                "receipt_ids": ["receipt-1"]
            }]
        }))
        .expect("parse block log fixture");
        postfiat_storage::NodeStore::new(&root)
            .write_blocks(&blocks)
            .expect("write block log fixture");
        let certificate = serde_json::json!({
            "schema": "postfiat-block-certificate-v1",
            "chain_id": "postfiat-wan-devnet",
            "genesis_hash": "11".repeat(48),
            "protocol_version": 1,
            "block_height": certificate_height,
            "view": 0,
            "proposer": "validator-0",
            "block_hash": "33".repeat(48),
            "certificate_id": "certificate-1",
            "certificate": {"validators": ["validator-0", "validator-1", "validator-2"], "quorum": 3, "votes": []}
        });
        let envelope = TransportBatchEnvelope {
            schema: TRANSPORT_BATCH_SCHEMA.to_string(),
            topology_id: "test-topology".to_string(),
            batch_kind: "transparent".to_string(),
            frame: FramedMessage {
                message_id: "message-1".to_string(),
                from: "validator-1".to_string(),
                to: Some("validator-0".to_string()),
                topic: "batch".to_string(),
                payload_hash: "payload-hash".to_string(),
                payload_len: 0,
            },
            auth: None,
            payload_json: serde_json::json!({"batch_id": "batch-1"}).to_string(),
            certificate_json: Some(certificate.to_string()),
        };
        (
            root,
            test_topology("127.0.0.1"),
            test_status("validator-0"),
            envelope,
        )
    }

    #[test]
    fn transport_duplicate_certified_batch_returns_typed_idempotent_ack() {
        let (root, topology, status, envelope) = already_applied_test_fixture(10);
        let ack =
            transport_already_applied_ack(&root, &topology, "validator-0", &envelope, &status)
                .expect("matching duplicate is idempotent");
        assert!(ack.applied);
        assert!(ack.already_applied);
        assert_eq!(ack.receipt_count, 1);
        assert_eq!(ack.certified_state.as_ref().unwrap().block_height, 10);
        std::fs::remove_dir_all(root).expect("cleanup idempotent ack fixture");
    }

    #[test]
    fn transport_duplicate_certified_batch_reports_historical_identity_at_high_water() {
        let (root, topology, mut status, envelope) = already_applied_test_fixture(10);
        status.block_height = 12;
        status.block_tip_hash = "44".repeat(48);
        status.state_root = "55".repeat(48);
        let ack =
            transport_already_applied_ack(&root, &topology, "validator-0", &envelope, &status)
                .expect("historical duplicate must return a typed high-water acknowledgement");
        validate_transport_batch_ack(
            &ack,
            &topology,
            &test_status("validator-1"),
            "validator-0",
            &envelope,
        )
        .expect("sender must verify the typed historical identity");
        assert_eq!(ack.state.block_height, 12);
        let certified = ack.certified_state.expect("historical identity");
        assert_eq!(certified.block_height, 10);
        assert_eq!(certified.block_tip_hash, "33".repeat(48));
        assert_eq!(certified.state_root, "22".repeat(48));
        std::fs::remove_dir_all(root).expect("cleanup high-water ack fixture");
    }

    #[test]
    fn transport_duplicate_certified_batch_rejects_conflicting_height() {
        let (root, topology, status, envelope) = already_applied_test_fixture(11);
        let error =
            transport_already_applied_ack(&root, &topology, "validator-0", &envelope, &status)
                .expect_err("conflicting duplicate must fail");
        assert!(
            error.contains("conflicts with certified height/hash"),
            "{error}"
        );
        std::fs::remove_dir_all(root).expect("cleanup conflicting ack fixture");
    }

    #[test]
    fn deferred_certified_send_never_detaches_a_child_process() {
        let root = unique_transport_test_ready_file("deferred-thread").with_extension("data");
        std::fs::create_dir_all(&root).expect("create deferred send root");
        let topology = root.join("missing-topology.json");
        let batch = root.join("batch.json");
        let certificate = root.join("certificate.json");
        std::fs::write(&batch, b"{}\n").expect("write batch fixture");
        std::fs::write(&certificate, b"{}\n").expect("write certificate fixture");
        let network = test_topology("127.0.0.1");
        let job_file = enqueue_durable_certified_send_job(
            &root,
            &network,
            "validator-0",
            "validator-1",
            "transparent",
            10,
            "certificate-1",
            &"33".repeat(48),
            &"22".repeat(48),
            &batch,
            &certificate,
            1,
            0,
            0,
        )
        .expect("persist durable send job");
        let report = spawn_deferred_certified_batch_send(&job_file, &root, &topology)
            .expect("spawn supervised deferred thread");
        assert_eq!(report.mode, "durable-outbox-plus-in-process-worker");
        assert_eq!(report.pid, std::process::id());
        assert_eq!(report.job_file, job_file.display().to_string());
        let stderr = PathBuf::from(&report.stderr_file);
        for _ in 0..100 {
            if stderr.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(stderr.exists(), "deferred thread must report its failure");
        let persisted = read_durable_certified_send_job(&job_file).expect("read durable job");
        assert!(!persisted.completed);
        std::fs::remove_dir_all(root).expect("cleanup deferred send root");
    }

    #[test]
    fn certified_send_job_is_atomic_bounded_and_idempotent() {
        let root = unique_transport_test_ready_file("durable-job").with_extension("data");
        std::fs::create_dir_all(&root).expect("create durable job root");
        let batch = root.join("batch-source.json");
        let certificate = root.join("certificate-source.json");
        std::fs::write(&batch, b"{\"batch\":1}\n").expect("write batch fixture");
        std::fs::write(&certificate, b"{\"certificate\":1}\n").expect("write certificate fixture");
        let topology = test_topology("127.0.0.1");
        let enqueue = || {
            enqueue_durable_certified_send_job(
                &root,
                &topology,
                "validator-0",
                "validator-1",
                "transparent",
                10,
                "certificate-1",
                &"33".repeat(48),
                &"22".repeat(48),
                &batch,
                &certificate,
                50,
                2,
                5,
            )
        };
        let first = enqueue().expect("first durable enqueue");
        let second = enqueue().expect("idempotent durable enqueue");
        assert_eq!(first, second);
        let job = read_durable_certified_send_job(&first).expect("read durable job");
        assert_eq!(job.block_height, 10);
        assert_eq!(job.target, "validator-1");
        assert!(!job.completed);
        validate_durable_certified_send_payloads(&first, &job).expect("validate durable payloads");

        std::fs::write(&job.batch_file, b"tampered\n").expect("tamper durable batch");
        let error = validate_durable_certified_send_payloads(&first, &job)
            .expect_err("tampered durable batch must fail");
        assert!(error.contains("hash mismatch"), "{error}");
        std::fs::remove_dir_all(root).expect("cleanup durable job root");
    }

    #[test]
    fn certified_send_job_survives_topology_rotation_with_same_chain_and_identities() {
        let root =
            unique_transport_test_ready_file("durable-topology-rotation").with_extension("data");
        std::fs::create_dir_all(&root).expect("create topology rotation root");
        let batch = root.join("batch-source.json");
        let certificate = root.join("certificate-source.json");
        std::fs::write(&batch, b"{\"batch\":1}\n").expect("write rotation batch");
        std::fs::write(&certificate, b"{\"certificate\":1}\n").expect("write rotation certificate");
        let old_topology = test_topology("127.0.0.1");
        let job_file = enqueue_durable_certified_send_job(
            &root,
            &old_topology,
            "validator-0",
            "validator-1",
            "governance",
            10,
            "certificate-rotation",
            &"33".repeat(48),
            &"22".repeat(48),
            &batch,
            &certificate,
            50,
            2,
            5,
        )
        .expect("enqueue pre-rotation job");
        let job = read_durable_certified_send_job(&job_file).expect("read pre-rotation job");
        let mut current_topology = old_topology.clone();
        current_topology.topology_id = "rotated-topology".to_string();
        current_topology.peers[1].host = "198.51.100.44".to_string();
        current_topology.peers[1].p2p_address = "/ip4/198.51.100.44/tcp/26651".to_string();

        validate_durable_certified_send_current_deployment(
            &job,
            &current_topology,
            &test_status("validator-0"),
        )
        .expect("same-chain job must resume through a signed topology rotation");

        let mut wrong_chain = current_topology.clone();
        wrong_chain.chain_id = "other-chain".to_string();
        let wrong_chain_error = validate_durable_certified_send_current_deployment(
            &job,
            &wrong_chain,
            &test_status("validator-0"),
        )
        .expect_err("cross-chain replay must remain rejected");
        assert!(wrong_chain_error.contains("deployment identity"));

        let rotated_topology = current_topology.clone();
        let mut missing_target = current_topology;
        missing_target
            .peers
            .retain(|peer| peer.node_id != "validator-1");
        let missing_target_error = validate_durable_certified_send_current_deployment(
            &job,
            &missing_target,
            &test_status("validator-0"),
        )
        .expect_err("removed target identity must remain rejected");
        assert!(missing_target_error.contains("absent from the current topology"));

        let reason = format!(
            "certified send ack from `validator-1` conflicts: expected height/hash/root 10/{}/{}",
            "33".repeat(48),
            "22".repeat(48)
        );
        let record_file =
            quarantine_durable_certified_send_job(&root, &job.job_id, &job_file, &reason)
                .expect("persist pre-fix high-water quarantine");
        let record = read_durable_certified_send_quarantine_record(&record_file)
            .expect("read high-water quarantine");
        assert!(rotated_topology_ack_quarantine_is_recoverable(
            &job_file,
            &job,
            &rotated_topology,
            &record,
        ));
        assert!(!rotated_topology_ack_quarantine_is_recoverable(
            &job_file,
            &job,
            &old_topology,
            &record,
        ));
        resolve_rotated_topology_ack_quarantine(&root, &job.job_id)
            .expect("archive resolved high-water quarantine");
        assert!(!certified_send_quarantine_job_dir(&root, &job.job_id).exists());
        assert!(certified_send_resolved_quarantine_dir(&root)
            .join(&job.job_id)
            .join("quarantine.json")
            .is_file());
        std::fs::remove_dir_all(root).expect("cleanup topology rotation root");
    }

    #[test]
    fn certified_send_resume_orders_same_target_jobs_by_height() {
        let root = unique_transport_test_ready_file("durable-height-order").with_extension("data");
        std::fs::create_dir_all(&root).expect("create height order root");
        let batch = root.join("batch-source.json");
        let certificate = root.join("certificate-source.json");
        std::fs::write(&batch, b"{\"batch\":1}\n").expect("write height order batch");
        std::fs::write(&certificate, b"{\"certificate\":1}\n")
            .expect("write height order certificate");
        let topology = test_topology("127.0.0.1");
        let low_certificate_id = "certificate-height-10";
        let low_block_hash = "33".repeat(48);
        let low_job_id = durable_certified_send_job_id(
            &topology.topology_id,
            "validator-0",
            "validator-1",
            10,
            low_certificate_id,
            &low_block_hash,
        );
        let (high_certificate_id, high_job_id) = (0_u64..10_000)
            .find_map(|suffix| {
                let certificate_id = format!("certificate-height-11-{suffix}");
                let job_id = durable_certified_send_job_id(
                    &topology.topology_id,
                    "validator-0",
                    "validator-1",
                    11,
                    &certificate_id,
                    &"44".repeat(48),
                );
                (job_id < low_job_id).then_some((certificate_id, job_id))
            })
            .expect("find adversarial reverse lexical job id");
        assert!(high_job_id < low_job_id);

        for (height, certificate_id, block_hash, state_root) in [
            (
                10,
                low_certificate_id.to_string(),
                low_block_hash,
                "22".repeat(48),
            ),
            (11, high_certificate_id, "44".repeat(48), "55".repeat(48)),
        ] {
            enqueue_durable_certified_send_job(
                &root,
                &topology,
                "validator-0",
                "validator-1",
                "transparent",
                height,
                &certificate_id,
                &block_hash,
                &state_root,
                &batch,
                &certificate,
                50,
                2,
                5,
            )
            .expect("enqueue height-ordered job");
        }

        let mut job_files = std::fs::read_dir(certified_send_outbox_dir(&root))
            .expect("read height order outbox")
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("job.json"))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        job_files.sort();
        let mut jobs = job_files
            .into_iter()
            .map(|job_file| {
                let job =
                    read_durable_certified_send_job(&job_file).expect("read height order job");
                (job_file, job)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            jobs.iter()
                .map(|(_, job)| job.block_height)
                .collect::<Vec<_>>(),
            vec![11, 10],
            "hash order must reproduce the adversarial causal inversion"
        );
        sort_durable_certified_send_jobs_for_resume(&mut jobs);
        assert_eq!(
            jobs.iter()
                .map(|(_, job)| job.block_height)
                .collect::<Vec<_>>(),
            vec![10, 11]
        );
        std::fs::remove_dir_all(root).expect("cleanup height order root");
    }

    #[test]
    fn certified_send_derives_fresh_block_hash_before_local_apply() {
        fn copy_dir_all(source: &Path, destination: &Path) {
            std::fs::create_dir_all(destination).expect("create copied node directory");
            for entry in std::fs::read_dir(source).expect("read copied node directory") {
                let entry = entry.expect("read copied node entry");
                let destination_path = destination.join(entry.file_name());
                if entry.path().is_dir() {
                    copy_dir_all(&entry.path(), &destination_path);
                } else {
                    std::fs::copy(entry.path(), destination_path).expect("copy node file");
                }
            }
        }

        let root =
            unique_transport_test_ready_file("durable-derived-block-hash").with_extension("data");
        let data_dir = root.join("node");
        let topology_file = root.join("topology.json");
        let batch_file = root.join("batch.json");
        let artifact_dir = root.join("artifacts");
        let validator_key_dir = root.join("validator-keys");
        let recovery_data_dir = root.join("recovery-node");
        std::fs::create_dir_all(&root).expect("create derived block hash root");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            node_id: DEFAULT_NODE_ID.to_string(),
            validator_count: 1,
        })
        .expect("init derived block hash node");
        write_local_topology(TopologyOptions {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            validators: 1,
            base_port: 45_500,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
        })
        .expect("write derived block hash topology");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pf1111111111111111111111111111111111111111".to_string(),
            amount: 100,
            batch_file: batch_file.clone(),
        })
        .expect("create derived block hash batch");
        std::fs::create_dir_all(&validator_key_dir).expect("create split validator key dir");
        std::fs::copy(
            data_dir.join(VALIDATOR_KEYS_FILE),
            validator_key_dir.join("validator-0.validator_keys.json"),
        )
        .expect("copy split validator key");
        std::fs::create_dir_all(&artifact_dir).expect("create derived hash artifacts");
        certify_batch_round(BatchCertificateRoundOptions {
            data_dir: data_dir.clone(),
            batch_kind: Some("transparent".to_string()),
            batch_file: batch_file.clone(),
            validator_key_dir,
            vote_dir: artifact_dir.join("votes"),
            proposal_file: artifact_dir.join("block-proposal.json"),
            certificate_file: artifact_dir.join("block-certificate.json"),
            block_height: Some(1),
            view: None,
            timeout_certificate_file: None,
            skip_block_log_verify: false,
        })
        .expect("fresh null-block-hash certificate");
        let certificate: BlockCertificateFile = serde_json::from_slice(
            &std::fs::read(artifact_dir.join("block-certificate.json"))
                .expect("read fresh block certificate"),
        )
        .expect("parse fresh block certificate");
        assert!(certificate.block_hash.is_none());
        let proposal: BlockProposalFile = serde_json::from_slice(
            &std::fs::read(artifact_dir.join("block-proposal.json"))
                .expect("read fresh block proposal"),
        )
        .expect("parse fresh block proposal");
        let topology = read_topology_file(&topology_file).expect("read derived hash topology");
        let expected = durable_certified_send_expected_block_hash(
            &topology,
            &proposal,
            &certificate,
        )
        .expect("derive expected block hash");
        copy_dir_all(&data_dir, &recovery_data_dir);
        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: Some(artifact_dir.join("block-certificate.json")),
        })
        .expect("apply fresh null-block-hash certificate");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{:?}", receipts[0]);
        let final_status = status(NodeOptions { data_dir }).expect("derived hash final status");
        assert_eq!(final_status.block_height, 1);
        assert_eq!(final_status.block_tip_hash, expected);
        assert_eq!(final_status.state_root, proposal.state_root);

        let job_id = durable_certified_send_job_id(
            &topology.topology_id,
            DEFAULT_NODE_ID,
            "validator-1",
            1,
            &certificate.certificate_id,
            &final_status.block_tip_hash,
        );
        let job_dir = certified_send_outbox_dir(&recovery_data_dir).join(&job_id);
        std::fs::create_dir_all(&job_dir).expect("create recovery outbox job");
        let durable_batch_file = job_dir.join("batch.json");
        let durable_certificate_file = job_dir.join("certificate.json");
        std::fs::copy(root.join("batch.json"), &durable_batch_file).expect("copy recovery batch");
        std::fs::copy(
            artifact_dir.join("block-certificate.json"),
            &durable_certificate_file,
        )
        .expect("copy recovery certificate");
        let batch_bytes = std::fs::read(&durable_batch_file).expect("read recovery batch");
        let certificate_bytes =
            std::fs::read(&durable_certificate_file).expect("read recovery certificate");
        let recovery_job = DurableCertifiedSendJob {
            schema: CERTIFIED_SEND_JOB_SCHEMA.to_string(),
            job_id,
            topology_id: topology.topology_id.clone(),
            chain_id: topology.chain_id.clone(),
            genesis_hash: topology.genesis_hash.clone(),
            protocol_version: topology.protocol_version,
            source: DEFAULT_NODE_ID.to_string(),
            target: "validator-1".to_string(),
            batch_kind: "transparent".to_string(),
            block_height: 1,
            certificate_id: certificate.certificate_id,
            block_hash: final_status.block_tip_hash.clone(),
            expected_state_root: final_status.state_root.clone(),
            batch_file: durable_batch_file.display().to_string(),
            batch_hash: postfiat_crypto_provider::hash_hex(
                "postfiat.certified_send_job.batch.v1",
                &batch_bytes,
            ),
            certificate_file: durable_certificate_file.display().to_string(),
            certificate_hash: postfiat_crypto_provider::hash_hex(
                "postfiat.certified_send_job.certificate.v1",
                &certificate_bytes,
            ),
            timeout_ms: 5_000,
            send_retries: 0,
            retry_backoff_ms: 0,
            attempt_count: 0,
            completed: false,
            last_error: None,
            ack: None,
        };
        let job_file = job_dir.join("job.json");
        write_durable_certified_send_job(&job_file, &recovery_job).expect("write recovery job");
        let recovery_before = status(NodeOptions {
            data_dir: recovery_data_dir.clone(),
        })
        .expect("pre-recovery status");
        assert_eq!(recovery_before.block_height, 0);
        let recovery_after = ensure_durable_certified_send_local_block(
            &recovery_data_dir,
            &job_file,
            &recovery_job,
            &recovery_before,
        )
        .expect("recover local apply from durable job");
        assert_eq!(recovery_after.block_height, 1);
        assert_eq!(recovery_after.block_tip_hash, final_status.block_tip_hash);
        assert_eq!(recovery_after.state_root, final_status.state_root);
        let recovery_replay = ensure_durable_certified_send_local_block(
            &recovery_data_dir,
            &job_file,
            &recovery_job,
            &recovery_after,
        )
        .expect("local recovery replay is idempotent");
        assert_eq!(recovery_replay.block_height, 1);
        std::fs::remove_dir_all(root).expect("cleanup derived block hash root");
    }

    #[test]
    fn certified_send_completed_tombstone_is_high_water_and_conflicts_quarantine() {
        let root = unique_transport_test_ready_file("durable-tombstone").with_extension("data");
        std::fs::create_dir_all(&root).expect("create tombstone root");
        let batch = root.join("batch-source.json");
        let certificate = root.join("certificate-source.json");
        std::fs::write(&batch, b"{\"batch\":1}\n").expect("write tombstone batch");
        std::fs::write(&certificate, b"{\"certificate\":1}\n")
            .expect("write tombstone certificate");
        let topology = test_topology("127.0.0.1");
        let enqueue = |expected_state_root: &str| {
            enqueue_durable_certified_send_job(
                &root,
                &topology,
                "validator-0",
                "validator-1",
                "transparent",
                10,
                "certificate-1",
                &"33".repeat(48),
                expected_state_root,
                &batch,
                &certificate,
                50,
                2,
                5,
            )
        };
        let active_job_file = enqueue(&"22".repeat(48)).expect("enqueue tombstone job");
        let mut job =
            read_durable_certified_send_job(&active_job_file).expect("read tombstone job");
        complete_durable_certified_send_job(
            &root,
            &active_job_file,
            &mut job,
            DurableCertifiedSendAck {
                already_applied: false,
                block_height: 10,
                block_tip_hash: "33".repeat(48),
                state_root: "22".repeat(48),
            },
        )
        .expect("complete tombstone job");
        assert_eq!(
            compact_completed_durable_certified_send_jobs(&root).expect("compact tombstone job"),
            1
        );

        let completed_job_file = certified_send_completed_dir(&root)
            .join(&job.job_id)
            .join("job.json");
        assert!(completed_job_file.is_file());
        assert!(!active_job_file.exists());
        let completed =
            read_durable_certified_send_job(&completed_job_file).expect("read completed tombstone");
        validate_completed_durable_certified_send_job(&completed_job_file, &completed)
            .expect("validate moved tombstone payloads");

        let replay_job_file = enqueue(&"22".repeat(48)).expect("replay completed tombstone");
        assert_eq!(replay_job_file, completed_job_file);
        assert!(
            !active_job_file.exists(),
            "replay must not recreate active job"
        );
        let replay_report =
            completed_durable_certified_send_report(&completed_job_file, &completed, &topology)
                .expect("typed completed replay report");
        assert!(replay_report.ack.already_applied);
        assert_eq!(replay_report.attempts, 0);

        let error =
            enqueue(&"44".repeat(48)).expect_err("conflicting tombstone replay must fail closed");
        assert!(error.contains("tombstone"), "{error}");
        assert!(certified_send_quarantine_record_file(&root, &job.job_id).is_file());
        assert!(completed_job_file.is_file(), "tombstone must be preserved");
        let resume =
            resume_durable_certified_send_outbox(&root, &root.join("missing-topology.json"), 1)
                .expect("standalone tombstone quarantine report");
        assert_eq!(resume.discovered, 1);
        assert_eq!(resume.quarantined, 1);
        assert_eq!(resume.pending, 1);
        assert!(!resume.all_completed);
        assert_eq!(resume.targets[0].result, "quarantined");
        std::fs::remove_dir_all(root).expect("cleanup tombstone root");
    }

    #[test]
    fn certified_send_conflicting_ack_is_quarantined_before_retry() {
        let root = unique_transport_test_ready_file("durable-ack-conflict").with_extension("data");
        std::fs::create_dir_all(&root).expect("create ack conflict root");
        let batch = root.join("batch-source.json");
        let certificate = root.join("certificate-source.json");
        std::fs::write(&batch, b"{\"batch\":1}\n").expect("write ack conflict batch");
        std::fs::write(&certificate, b"{\"certificate\":1}\n")
            .expect("write ack conflict certificate");
        let topology = test_topology("127.0.0.1");
        let job_file = enqueue_durable_certified_send_job(
            &root,
            &topology,
            "validator-0",
            "validator-1",
            "transparent",
            10,
            "certificate-1",
            &"33".repeat(48),
            &"22".repeat(48),
            &batch,
            &certificate,
            50,
            2,
            5,
        )
        .expect("enqueue ack conflict job");
        let mut job = read_durable_certified_send_job(&job_file).expect("read ack conflict job");
        let error = complete_durable_certified_send_job(
            &root,
            &job_file,
            &mut job,
            DurableCertifiedSendAck {
                already_applied: false,
                block_height: 10,
                block_tip_hash: "33".repeat(48),
                state_root: "44".repeat(48),
            },
        )
        .expect_err("conflicting acknowledgement must fail");
        assert!(error.contains("observed"), "{error}");
        let quarantine_file = certified_send_quarantine_record_file(&root, &job.job_id);
        let quarantine: DurableCertifiedSendQuarantineRecord = serde_json::from_slice(
            &std::fs::read(&quarantine_file).expect("read quarantine record"),
        )
        .expect("parse quarantine record");
        assert_eq!(quarantine.schema, CERTIFIED_SEND_QUARANTINE_SCHEMA);
        assert_eq!(quarantine.job_id, job.job_id);
        let persisted = read_durable_certified_send_job(&job_file).expect("read quarantined job");
        assert!(!persisted.completed);
        assert!(persisted.last_error.is_some());

        let retry_error =
            send_durable_certified_send_job(&job_file, &root, &root.join("missing-topology.json"))
                .expect_err("quarantined job must not retry");
        assert!(retry_error.contains("is quarantined"), "{retry_error}");
        let resume =
            resume_durable_certified_send_outbox(&root, &root.join("missing-topology.json"), 1)
                .expect("quarantined resume report");
        assert_eq!(resume.attempted, 0);
        assert_eq!(resume.pending, 1);
        assert_eq!(resume.quarantined, 1);
        assert_eq!(resume.targets[0].result, "quarantined");
        std::fs::remove_dir_all(root).expect("cleanup ack conflict root");
    }

    #[test]
    fn certified_send_tampered_tombstone_is_quarantined() {
        let root =
            unique_transport_test_ready_file("durable-tombstone-tamper").with_extension("data");
        std::fs::create_dir_all(&root).expect("create tombstone tamper root");
        let batch = root.join("batch-source.json");
        let certificate = root.join("certificate-source.json");
        std::fs::write(&batch, b"{\"batch\":1}\n").expect("write tombstone tamper batch");
        std::fs::write(&certificate, b"{\"certificate\":1}\n")
            .expect("write tombstone tamper certificate");
        let topology = test_topology("127.0.0.1");
        let enqueue = || {
            enqueue_durable_certified_send_job(
                &root,
                &topology,
                "validator-0",
                "validator-1",
                "transparent",
                10,
                "certificate-1",
                &"33".repeat(48),
                &"22".repeat(48),
                &batch,
                &certificate,
                50,
                2,
                5,
            )
        };
        let active_job_file = enqueue().expect("enqueue tombstone tamper job");
        let mut job =
            read_durable_certified_send_job(&active_job_file).expect("read tombstone tamper job");
        complete_durable_certified_send_job(
            &root,
            &active_job_file,
            &mut job,
            DurableCertifiedSendAck {
                already_applied: false,
                block_height: 10,
                block_tip_hash: "33".repeat(48),
                state_root: "22".repeat(48),
            },
        )
        .expect("complete tombstone tamper job");
        compact_completed_durable_certified_send_jobs(&root).expect("compact tombstone tamper job");
        let completed_job_file = certified_send_completed_dir(&root)
            .join(&job.job_id)
            .join("job.json");
        let mut tampered = read_durable_certified_send_job(&completed_job_file)
            .expect("read tombstone before tamper");
        tampered
            .ack
            .as_mut()
            .expect("tombstone acknowledgement")
            .state_root = "44".repeat(48);
        write_durable_certified_send_job(&completed_job_file, &tampered)
            .expect("write tampered tombstone");

        let error = enqueue().expect_err("tampered tombstone must fail closed");
        assert!(error.contains("acknowledgement conflicts"), "{error}");
        assert!(certified_send_quarantine_record_file(&root, &job.job_id).is_file());
        assert!(
            completed_job_file.is_file(),
            "tampered evidence must be preserved"
        );
        std::fs::remove_dir_all(root).expect("cleanup tombstone tamper root");
    }

    #[test]
    fn certified_send_job_rejects_corrupt_and_oversized_records() {
        let root = unique_transport_test_ready_file("durable-corrupt").with_extension("data");
        std::fs::create_dir_all(&root).expect("create corrupt job root");
        let corrupt = root.join("corrupt.json");
        std::fs::write(&corrupt, b"not-json\n").expect("write corrupt job");
        assert!(read_durable_certified_send_job(&corrupt).is_err());

        let oversized = root.join("oversized.json");
        std::fs::write(
            &oversized,
            vec![b'x'; CERTIFIED_SEND_JOB_MAX_BYTES as usize + 1],
        )
        .expect("write oversized job");
        let error = read_durable_certified_send_job(&oversized)
            .expect_err("oversized job must fail before parsing");
        assert!(error.contains("exceeded"), "{error}");
        std::fs::remove_dir_all(root).expect("cleanup corrupt job root");
    }

    #[test]
    fn deterministic_six_node_delivery_failures_converge() {
        #[derive(Clone, Copy)]
        enum Fault {
            None,
            LostAck,
            DuplicateSend,
            Delayed,
            CrashAfterEnqueue,
            CrashDuringSend,
            TwoBlocksBehind,
        }

        fn converge(fault: Fault) -> ([u64; 6], u64) {
            let target_height = 12;
            let mut heights = [11, 11, 11, 11, 11, 11];
            if matches!(fault, Fault::TwoBlocksBehind) {
                heights[5] = 10;
            }
            heights[0] = target_height;
            let mut durable_pending = [false; 6];
            for pending in durable_pending.iter_mut().skip(1) {
                *pending = true;
            }
            let mut attempts = 0u64;
            for pass in 0..4 {
                for validator in 1..6 {
                    if !durable_pending[validator] {
                        continue;
                    }
                    attempts = attempts.saturating_add(1);
                    let injected = match fault {
                        Fault::LostAck => pass == 0 && validator == 1,
                        Fault::Delayed => pass < 2 && validator == 2,
                        Fault::CrashAfterEnqueue => pass == 0,
                        Fault::CrashDuringSend => pass == 0 && validator == 3,
                        _ => false,
                    };
                    if injected {
                        continue;
                    }
                    heights[validator] = target_height;
                    if matches!(fault, Fault::DuplicateSend) && pass == 0 && validator == 4 {
                        // The first apply succeeded but its acknowledgement was
                        // replayed. The second delivery is typed idempotent.
                        continue;
                    }
                    durable_pending[validator] = false;
                }
                if durable_pending.iter().skip(1).all(|pending| !pending) {
                    break;
                }
            }
            assert!(durable_pending.iter().skip(1).all(|pending| !pending));
            (heights, attempts)
        }

        let faults = [
            Fault::None,
            Fault::LostAck,
            Fault::DuplicateSend,
            Fault::Delayed,
            Fault::CrashAfterEnqueue,
            Fault::CrashDuringSend,
            Fault::TwoBlocksBehind,
        ];
        for fault in faults {
            let (heights, attempts) = converge(fault);
            assert_eq!(heights, [12; 6]);
            assert!(attempts >= 5);
        }
    }
}
