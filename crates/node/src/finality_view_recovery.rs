const RPC_FINALITY_TIMEOUT_VOTE_METHOD: &str = "consensus_v2_timeout_vote";
const RPC_FINALITY_TIMEOUT_HIGH_QC_COMPAT: &str = "consensus-v2-signed-high-qc";
const RPC_FINALITY_TIMEOUT_VOTES_ENCODING: &str = "gzip-base64-chunks-v1";
const RPC_FINALITY_TIMEOUT_VOTE_CHUNK_PREFIX: &str = "proxy_timeout_votes_chunk_";
const RPC_FINALITY_TIMEOUT_VOTE_MAX_CHUNKS: usize = 512;
const RPC_FINALITY_TIMEOUT_VOTE_DECOMPRESSED_MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
struct RpcFinalityViewContext {
    view: u64,
    timeout_certificate_file: Option<PathBuf>,
}

fn rpc_finality_requested_view(
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<u64, (String, String)> {
    rpc_finality_required_u64_param(params, "proxy_consensus_view").map(|view| view.unwrap_or(0))
}

fn require_rpc_finality_view_bound(
    data_dir: &Path,
    block_height: u64,
    view: u64,
) -> Result<(), (String, String)> {
    let genesis = NodeStore::new(data_dir).read_genesis().map_err(|error| {
        (
            "rpc_finality_view_failed".to_string(),
            format!("finality view genesis read failed: {error}"),
        )
    })?;
    if !consensus_v2_active_at(&genesis, block_height) {
        if view == 0 {
            return Ok(());
        }
        return Err((
            "rpc_finality_view_not_active".to_string(),
            format!("consensus v2 is not active at height {block_height}"),
        ));
    }
    let validator_count = active_validator_ids_for_node(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
        .map_err(|error| {
            (
                "rpc_finality_view_failed".to_string(),
                format!("finality view validator lookup failed: {error}"),
            )
        })?
        .len() as u64;
    if view >= validator_count {
        return Err((
            "rpc_finality_view_exhausted".to_string(),
            format!(
                "finality recovery view {view} exceeds the bounded {validator_count}-validator attempt window"
            ),
        ));
    }
    Ok(())
}

fn prepare_rpc_finality_view(
    data_dir: &Path,
    params: &serde_json::Map<String, serde_json::Value>,
    artifact_dir: &Path,
) -> Result<RpcFinalityViewContext, (String, String)> {
    let status = status(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .map_err(|error| {
        (
            "rpc_finality_view_failed".to_string(),
            format!("finality view status failed: {error}"),
        )
    })?;
    let block_height = status.block_height.checked_add(1).ok_or_else(|| {
        (
            "rpc_finality_view_failed".to_string(),
            "finality view block height overflow".to_string(),
        )
    })?;
    let view = rpc_finality_requested_view(params)?;
    require_rpc_finality_view_bound(data_dir, block_height, view)?;

    let supplied_vote_chunks = params
        .keys()
        .any(|key| key.starts_with(RPC_FINALITY_TIMEOUT_VOTE_CHUNK_PREFIX));
    let supplied_vote_envelope = supplied_vote_chunks
        || params.contains_key("proxy_timeout_votes_encoding")
        || params.contains_key("proxy_timeout_votes_chunk_count");
    if view == 0 {
        if supplied_vote_envelope {
            return Err((
                "rpc_protocol_error".to_string(),
                "view 0 finality request must not carry timeout votes".to_string(),
            ));
        }
        return Ok(RpcFinalityViewContext {
            view,
            timeout_certificate_file: None,
        });
    }

    let votes_json = decode_rpc_finality_timeout_vote_chunks(params)?;
    let votes: Vec<BlockTimeoutVoteFile> = serde_json::from_str(&votes_json).map_err(|error| {
        (
            "rpc_protocol_error".to_string(),
            format!("finality timeout vote envelope is invalid: {error}"),
        )
    })?;
    if votes.is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "finality timeout vote envelope must contain at least one vote".to_string(),
        ));
    }
    let validator_count = active_validator_ids_for_node(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
        .map_err(|error| {
            (
                "rpc_finality_view_failed".to_string(),
                format!("finality timeout validator lookup failed: {error}"),
            )
        })?
        .len();
    if votes.len() > validator_count {
        return Err((
            "rpc_protocol_error".to_string(),
            format!(
                "finality timeout vote count {} exceeds validator count {validator_count}",
                votes.len()
            ),
        ));
    }

    std::fs::create_dir_all(artifact_dir).map_err(|error| {
        (
            "rpc_finality_view_failed".to_string(),
            format!(
                "finality recovery artifact directory `{}` create failed: {error}",
                artifact_dir.display()
            ),
        )
    })?;
    let mut vote_files = Vec::with_capacity(votes.len());
    for (index, vote) in votes.iter().enumerate() {
        let vote_file = artifact_dir.join(format!("timeout-vote-{index}.json"));
        let json = serde_json::to_vec_pretty(vote).map_err(|error| {
            (
                "rpc_finality_view_failed".to_string(),
                format!("finality timeout vote serialization failed: {error}"),
            )
        })?;
        std::fs::write(&vote_file, json).map_err(|error| {
            (
                "rpc_finality_view_failed".to_string(),
                format!(
                    "finality timeout vote write `{}` failed: {error}",
                    vote_file.display()
                ),
            )
        })?;
        vote_files.push(vote_file);
    }
    let timeout_certificate_file = artifact_dir.join("timeout-certificate.json");
    aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
        data_dir: data_dir.to_path_buf(),
        verify_block_log: true,
        block_height,
        view: view - 1,
        vote_files,
        certificate_file: timeout_certificate_file.clone(),
    })
    .map_err(|error| {
        (
            "rpc_finality_timeout_certificate_invalid".to_string(),
            format!("finality timeout certificate aggregation failed: {error}"),
        )
    })?;
    Ok(RpcFinalityViewContext {
        view,
        timeout_certificate_file: Some(timeout_certificate_file),
    })
}

fn decode_rpc_finality_timeout_vote_chunks(
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, (String, String)> {
    let encoding = params
        .get("proxy_timeout_votes_encoding")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "nonzero finality view requires a timeout vote encoding".to_string(),
            )
        })?;
    if encoding != RPC_FINALITY_TIMEOUT_VOTES_ENCODING {
        return Err((
            "rpc_protocol_error".to_string(),
            format!("unsupported finality timeout vote encoding `{encoding}`"),
        ));
    }
    let chunk_count = rpc_finality_required_u64_param(params, "proxy_timeout_votes_chunk_count")?
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "nonzero finality view requires timeout vote chunks".to_string(),
            )
        })? as usize;
    if chunk_count == 0 || chunk_count > RPC_FINALITY_TIMEOUT_VOTE_MAX_CHUNKS {
        return Err((
            "rpc_protocol_error".to_string(),
            format!(
                "finality timeout vote chunk count {chunk_count} is outside 1..={RPC_FINALITY_TIMEOUT_VOTE_MAX_CHUNKS}"
            ),
        ));
    }
    let actual_chunk_count = params
        .keys()
        .filter(|key| {
            key.strip_prefix(RPC_FINALITY_TIMEOUT_VOTE_CHUNK_PREFIX)
                .is_some_and(|suffix| {
                    suffix.len() == 4 && suffix.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
        .count();
    if actual_chunk_count != chunk_count {
        return Err((
            "rpc_protocol_error".to_string(),
            format!(
                "finality timeout vote envelope declares {chunk_count} chunks but supplies {actual_chunk_count}"
            ),
        ));
    }
    let mut encoded = String::new();
    for index in 0..chunk_count {
        let key = format!("{RPC_FINALITY_TIMEOUT_VOTE_CHUNK_PREFIX}{index:04}");
        let chunk = params
            .get(&key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                (
                    "rpc_protocol_error".to_string(),
                    format!("finality timeout vote envelope is missing chunk {index}"),
                )
            })?;
        encoded.push_str(chunk);
    }
    let compressed = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        encoded.as_bytes(),
    )
    .map_err(|error| {
        (
            "rpc_protocol_error".to_string(),
            format!("finality timeout vote base64 decode failed: {error}"),
        )
    })?;
    let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
    let mut limited = std::io::Read::take(
        &mut decoder,
        RPC_FINALITY_TIMEOUT_VOTE_DECOMPRESSED_MAX_BYTES + 1,
    );
    let mut votes_json = String::new();
    std::io::Read::read_to_string(&mut limited, &mut votes_json).map_err(|error| {
        (
            "rpc_protocol_error".to_string(),
            format!("finality timeout vote gzip decode failed: {error}"),
        )
    })?;
    if votes_json.len() as u64 > RPC_FINALITY_TIMEOUT_VOTE_DECOMPRESSED_MAX_BYTES {
        return Err((
            "rpc_protocol_error".to_string(),
            "finality timeout vote envelope exceeds its decompressed byte cap".to_string(),
        ));
    }
    if votes_json.is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "finality timeout vote envelope decoded to an empty value".to_string(),
        ));
    }
    Ok(votes_json)
}

#[cfg(test)]
fn rpc_finality_timeout_vote_params_for_test(
    votes: &[BlockTimeoutVoteFile],
) -> serde_json::Map<String, serde_json::Value> {
    let votes_json = serde_json::to_vec(votes).expect("serialize timeout votes");
    let mut encoder =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    std::io::Write::write_all(&mut encoder, &votes_json).expect("compress timeout votes");
    let compressed = encoder.finish().expect("finish timeout vote compression");
    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        compressed,
    );
    let chunks = encoded.as_bytes().chunks(4_000).collect::<Vec<_>>();
    let mut params = serde_json::Map::new();
    params.insert(
        "proxy_timeout_votes_encoding".to_string(),
        serde_json::Value::String(RPC_FINALITY_TIMEOUT_VOTES_ENCODING.to_string()),
    );
    params.insert(
        "proxy_timeout_votes_chunk_count".to_string(),
        serde_json::json!(chunks.len()),
    );
    for (index, chunk) in chunks.iter().enumerate() {
        params.insert(
            format!("{RPC_FINALITY_TIMEOUT_VOTE_CHUNK_PREFIX}{index:04}"),
            serde_json::Value::String(String::from_utf8(chunk.to_vec()).expect("base64 chunk")),
        );
    }
    params
}

fn create_rpc_finality_timeout_vote(
    context: &RpcServeConnectionContext,
    params: &serde_json::Map<String, serde_json::Value>,
    artifact_dir: &Path,
) -> Result<BlockTimeoutVoteFile, (String, String)> {
    let block_height = rpc_finality_required_u64_param(params, "block_height")?.ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "consensus_v2_timeout_vote requires block_height".to_string(),
        )
    })?;
    let view = rpc_finality_required_u64_param(params, "view")?.ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "consensus_v2_timeout_vote requires view".to_string(),
        )
    })?;
    let local = status(NodeOptions {
        data_dir: context.data_dir.clone(),
    })
    .map_err(|error| {
        (
            "rpc_finality_timeout_vote_failed".to_string(),
            format!("finality timeout status failed: {error}"),
        )
    })?;
    let expected_height = local.block_height.checked_add(1).ok_or_else(|| {
        (
            "rpc_finality_timeout_vote_failed".to_string(),
            "finality timeout height overflow".to_string(),
        )
    })?;
    if block_height != expected_height {
        return Err((
            "rpc_finality_timeout_parent_mismatch".to_string(),
            format!(
                "timeout vote height {block_height} does not extend local height {}",
                local.block_height
            ),
        ));
    }
    require_rpc_finality_view_bound(&context.data_dir, block_height, view)?;
    std::fs::create_dir_all(artifact_dir).map_err(|error| {
        (
            "rpc_finality_timeout_vote_failed".to_string(),
            format!(
                "finality timeout artifact directory `{}` create failed: {error}",
                artifact_dir.display()
            ),
        )
    })?;
    let vote_file = artifact_dir.join(format!("{}.h{block_height}.v{view}.json", context.node_id));
    create_block_timeout_vote(BlockTimeoutVoteOptions {
        data_dir: context.data_dir.clone(),
        verify_block_log: true,
        key_file: context.finality_key_file.clone(),
        validator_id: Some(context.node_id.clone()),
        block_height,
        view,
        high_qc_id: RPC_FINALITY_TIMEOUT_HIGH_QC_COMPAT.to_string(),
        vote_file,
    })
    .map_err(|error| {
        (
            "rpc_finality_timeout_vote_failed".to_string(),
            format!("finality timeout vote failed: {error}"),
        )
    })
}

fn run_rpc_serve_consensus_v2_timeout_vote(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    let result = (|| -> Result<RpcResponse, (String, String)> {
        let params = request.params.as_object().ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "consensus_v2_timeout_vote params must be an object".to_string(),
            )
        })?;
        let _finality_guard = context.finality_submit_lock.try_lock().map_err(|error| match error {
            std::sync::TryLockError::WouldBlock => (
                "rpc_finality_submit_busy".to_string(),
                "another finality operation is already in progress".to_string(),
            ),
            std::sync::TryLockError::Poisoned(_) => (
                "rpc_finality_submit_lock_poisoned".to_string(),
                "finality submit lock poisoned".to_string(),
            ),
        })?;
        wait_for_rpc_finality_required_parent(context, params)?;
        let request_component = transport_artifact_component(&request.id);
        let artifact_dir = context.finality_artifact_root.join(format!(
            "rpc-timeout-vote-{request_index}-{request_component}"
        ));
        let vote = create_rpc_finality_timeout_vote(context, params, &artifact_dir)?;
        success_response(
            &request.id,
            &vote,
            vec![RpcEvent::new(
                RPC_FINALITY_TIMEOUT_VOTE_METHOD,
                format!("{}:{}", vote.block_height, vote.view),
                "validator durably signed a consensus-v2 timeout vote",
            )],
        )
        .map_err(|error| {
            (
                "rpc_finality_timeout_vote_failed".to_string(),
                format!("finality timeout response serialization failed: {error}"),
            )
        })
    })();
    match result {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

#[cfg(test)]
mod finality_view_recovery_tests {
    use super::*;

    #[test]
    fn timeout_vote_chunk_envelope_round_trips_and_rejects_truncation() {
        let mut params = rpc_finality_timeout_vote_params_for_test(&[]);
        assert_eq!(
            decode_rpc_finality_timeout_vote_chunks(&params).expect("decode envelope"),
            "[]"
        );

        params.remove("proxy_timeout_votes_chunk_0000");
        let error = decode_rpc_finality_timeout_vote_chunks(&params)
            .expect_err("truncated envelope must fail closed");
        assert_eq!(error.0, "rpc_protocol_error");
        assert!(error.1.contains("declares 1 chunks but supplies 0"));
    }
}
