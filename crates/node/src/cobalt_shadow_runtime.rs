//! Bounded socket service for the non-authoritative Cobalt shadow runtime.

use std::fs;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use postfiat_crypto_provider::hash_hex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cobalt_shadow::{
    build_signed_protocol_transcript, CobaltShadowHistoryRange, CobaltShadowIdentity,
    CobaltShadowLimits, CobaltShadowMessage, CobaltShadowProtocolDecision,
    CobaltShadowProtocolTranscript, CobaltShadowService, CobaltShadowState, CobaltShadowStatus,
};

pub const COBALT_SHADOW_RPC_SCHEMA: &str = "postfiat-cobalt-shadow-rpc-v1";
pub const COBALT_SHADOW_PROBE_SCHEMA: &str = "postfiat-cobalt-shadow-probe-v1";
pub const COBALT_SHADOW_NETWORK_DRILL_SCHEMA: &str = "postfiat-cobalt-shadow-network-drill-v1";
pub const MAX_RPC_FRAME_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum CobaltShadowRpcRequest {
    Probe,
    Snapshot,
    Replay,
    HistoryRange {
        start_sequence: u64,
        limit: usize,
    },
    VerifyHistoryRange {
        range: Box<CobaltShadowHistoryRange>,
    },
    CatchUp {
        range: Box<CobaltShadowHistoryRange>,
    },
    Submit {
        message: CobaltShadowMessage,
    },
    Process,
    Commit {
        transcript: Box<CobaltShadowProtocolTranscript>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CobaltShadowRpcResponse {
    pub schema: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowResourcePosture {
    pub state_bytes: u64,
    pub private_state_bytes: u64,
    pub history_bytes: u64,
    pub frame_limit_bytes: usize,
    pub queue_limit_messages: usize,
    pub message_limit_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowProbe {
    pub schema: String,
    pub node_id: String,
    pub peer_health: String,
    pub configured_peers: usize,
    pub queue_health: String,
    pub queue_depth: usize,
    pub registry_root: String,
    pub trust_graph_root: String,
    pub ratification_locks: usize,
    pub protocol_decisions: usize,
    pub protocol_high_watermark: u64,
    pub protocol_signer_high_watermark: u64,
    pub contiguous_sequence: u64,
    pub history_head: Option<String>,
    pub missing_ranges: Vec<crate::cobalt_shadow::CobaltShadowMissingRange>,
    pub catch_up_status: String,
    pub certificate_signer_count: usize,
    pub replay_posture: String,
    pub messages_received: u64,
    pub bytes_received: u64,
    pub messages_transmitted: u64,
    pub bytes_transmitted: u64,
    pub protocol_validation_micros: u64,
    pub stage_validation_micros: std::collections::BTreeMap<String, u64>,
    pub resource_posture: CobaltShadowResourcePosture,
    pub live_authority: bool,
    pub controls_block_consensus: bool,
    pub status: CobaltShadowStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowNetworkDrillReport {
    pub schema: String,
    pub status: String,
    pub ok: bool,
    pub scope: String,
    pub socket_count: usize,
    pub signed_message_count: usize,
    pub registry_root: String,
    pub trust_graph_root: String,
    pub ratification_id: String,
    pub transcript_hash: String,
    pub converged: bool,
    pub restart_equivalent: bool,
    pub tampered_signature_rejected: bool,
    pub oversized_frame_rejected: bool,
    pub live_authority_remained_disabled: bool,
    pub probes: Vec<CobaltShadowProbe>,
    pub replay: Vec<Vec<CobaltShadowProtocolDecision>>,
}

pub fn serve_listener(
    mut service: CobaltShadowService,
    listener: TcpListener,
    allow_shutdown: bool,
) -> io::Result<CobaltShadowService> {
    for incoming in listener.incoming() {
        let mut stream = incoming?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        let request = match read_request(&mut stream) {
            Ok(request) => request,
            Err(error) => {
                write_response(&mut stream, error_response(&error))?;
                continue;
            }
        };
        let shutdown = matches!(request, CobaltShadowRpcRequest::Shutdown);
        let response = if shutdown && !allow_shutdown {
            error_response(&invalid("remote shutdown is disabled"))
        } else {
            handle_request(&mut service, request)
        };
        write_response(&mut stream, response)?;
        if shutdown && allow_shutdown {
            break;
        }
    }
    Ok(service)
}

pub fn request(endpoint: SocketAddr, request: &CobaltShadowRpcRequest) -> io::Result<Value> {
    let encoded = serde_json::to_vec(request).map_err(json_error)?;
    if encoded.len() > MAX_RPC_FRAME_BYTES {
        return Err(invalid("RPC request exceeds frame bound"));
    }
    let mut stream = TcpStream::connect_timeout(&endpoint, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(&encoded)?;
    stream.shutdown(Shutdown::Write)?;
    let mut response_bytes = Vec::new();
    stream
        .take((MAX_RPC_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut response_bytes)?;
    if response_bytes.len() > MAX_RPC_FRAME_BYTES {
        return Err(invalid("RPC response exceeds frame bound"));
    }
    let response: CobaltShadowRpcResponse =
        serde_json::from_slice(&response_bytes).map_err(json_error)?;
    if !response.ok {
        return Err(invalid(
            response
                .error
                .unwrap_or_else(|| "RPC request failed".to_string()),
        ));
    }
    response
        .result
        .ok_or_else(|| invalid("RPC response did not contain a result"))
}

pub fn probe_service(service: &CobaltShadowService) -> io::Result<CobaltShadowProbe> {
    let status = service.status();
    let data_dir = service.data_dir();
    let state_bytes = fs::metadata(data_dir.join("state.json"))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let private_state_bytes = fs::metadata(data_dir.join("signer-private.json"))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let history_bytes = fs::metadata(data_dir.join("protocol-history.jsonl"))
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    Ok(CobaltShadowProbe {
        schema: COBALT_SHADOW_PROBE_SCHEMA.to_string(),
        node_id: status.node_id.clone(),
        peer_health: if status.peer_count >= 3 && !status.registry_root.is_empty() {
            "healthy"
        } else {
            "unbound"
        }
        .to_string(),
        configured_peers: status.peer_count,
        queue_health: if status.transport_healthy {
            "healthy"
        } else {
            "saturated"
        }
        .to_string(),
        queue_depth: status.queue_depth,
        registry_root: status.registry_root.clone(),
        trust_graph_root: status.trust_graph_root.clone(),
        ratification_locks: status.ratification_lock_count,
        protocol_decisions: status.protocol_decision_count,
        protocol_high_watermark: status.protocol_high_watermark,
        protocol_signer_high_watermark: status.protocol_signer_high_watermark,
        contiguous_sequence: status.contiguous_sequence,
        history_head: status.history_head.clone(),
        missing_ranges: status.missing_ranges.clone(),
        catch_up_status: status.catch_up_status.clone(),
        certificate_signer_count: status.certificate_signer_count,
        replay_posture: if status.ratification_lock_count == status.protocol_decision_count
            && status.protocol_decision_count == status.contiguous_sequence as usize
        {
            "consistent"
        } else {
            "inconsistent"
        }
        .to_string(),
        messages_received: status.messages_received,
        bytes_received: status.bytes_received,
        messages_transmitted: status.messages_transmitted,
        bytes_transmitted: status.bytes_transmitted,
        protocol_validation_micros: status.protocol_validation_micros,
        stage_validation_micros: status.stage_validation_micros.clone(),
        resource_posture: CobaltShadowResourcePosture {
            state_bytes,
            private_state_bytes,
            history_bytes,
            frame_limit_bytes: MAX_RPC_FRAME_BYTES,
            queue_limit_messages: service.state().limits.max_queue_messages,
            message_limit_bytes: service.state().limits.max_message_bytes,
        },
        live_authority: status.live_authority,
        controls_block_consensus: status.controls_block_consensus,
        status,
    })
}

pub fn run_cobalt_shadow_network_drill(
    root: impl Into<PathBuf>,
) -> io::Result<CobaltShadowNetworkDrillReport> {
    let root = root.into();
    fs::create_dir_all(&root)?;
    let mut fleet = (0..3)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                CobaltShadowIdentity {
                    node_id: format!("validator-{index}"),
                    chain_id: "postfiat-cobalt-shadow-socket-drill".to_string(),
                    genesis_hash: "02".repeat(48),
                    protocol_version: 1,
                },
                CobaltShadowLimits::default(),
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    let transcript = build_signed_protocol_transcript(
        &mut fleet,
        11,
        hash_hex(
            "postfiat.cobalt.shadow.network-drill.payload.v1",
            b"inert-validator-governance-proposal",
        ),
    )?;
    let mut listeners = Vec::new();
    let mut endpoints = Vec::new();
    for _ in 0..fleet.len() {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        endpoints.push(listener.local_addr()?);
        listeners.push(listener);
    }
    let handles = fleet
        .into_iter()
        .zip(listeners)
        .map(|(service, listener)| thread::spawn(move || serve_listener(service, listener, true)))
        .collect::<Vec<_>>();

    let mut decisions = Vec::new();
    for endpoint in &endpoints {
        let value = request(
            *endpoint,
            &CobaltShadowRpcRequest::Commit {
                transcript: Box::new(transcript.clone()),
            },
        )?;
        decisions.push(
            serde_json::from_value::<CobaltShadowProtocolDecision>(value).map_err(json_error)?,
        );
    }

    let mut tampered = transcript.clone();
    let replacement = if tampered.rbc_echoes[0].signature_hex.starts_with("00") {
        "01"
    } else {
        "00"
    };
    tampered.rbc_echoes[0]
        .signature_hex
        .replace_range(0..2, replacement);
    let tampered_signature_rejected = request(
        endpoints[0],
        &CobaltShadowRpcRequest::Commit {
            transcript: Box::new(tampered),
        },
    )
    .is_err();

    let oversized_frame_rejected = send_oversized_frame(endpoints[1])?;

    let mut probes = Vec::new();
    let mut replay = Vec::new();
    for endpoint in &endpoints {
        probes.push(
            serde_json::from_value::<CobaltShadowProbe>(request(
                *endpoint,
                &CobaltShadowRpcRequest::Probe,
            )?)
            .map_err(json_error)?,
        );
        let replay_value = request(*endpoint, &CobaltShadowRpcRequest::Replay)?;
        replay.push(
            serde_json::from_value::<Vec<CobaltShadowProtocolDecision>>(replay_value)
                .map_err(json_error)?,
        );
        let snapshot = request(*endpoint, &CobaltShadowRpcRequest::Snapshot)?;
        let state: CobaltShadowState = serde_json::from_value(snapshot).map_err(json_error)?;
        if state.protocol_decisions.len() != 1 {
            return Err(invalid("network drill snapshot lost protocol decision"));
        }
    }
    for endpoint in &endpoints {
        request(*endpoint, &CobaltShadowRpcRequest::Shutdown)?;
    }
    let stopped = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .map_err(|_| invalid("Cobalt shadow server thread panicked"))?
        })
        .collect::<io::Result<Vec<_>>>()?;

    let converged = decisions.windows(2).all(|pair| pair[0] == pair[1])
        && replay.windows(2).all(|pair| pair[0] == pair[1]);
    let expected = decisions[0].clone();
    drop(stopped);
    let mut restarted_replay = Vec::new();
    for index in 0..3 {
        let mut service = CobaltShadowService::open(root.join(format!("validator-{index}")))?;
        restarted_replay.push(service.replay_protocol_state()?);
    }
    let restart_equivalent = restarted_replay
        .iter()
        .all(|rows| rows.as_slice() == [expected.clone()]);
    let live_authority_remained_disabled = probes
        .iter()
        .all(|probe| !probe.live_authority && !probe.controls_block_consensus);
    let ok = converged
        && restart_equivalent
        && tampered_signature_rejected
        && oversized_frame_rejected
        && live_authority_remained_disabled;
    Ok(CobaltShadowNetworkDrillReport {
        schema: COBALT_SHADOW_NETWORK_DRILL_SCHEMA.to_string(),
        status: if ok { "passed" } else { "failed" }.to_string(),
        ok,
        scope: "local-loopback-sockets-no-live-authority".to_string(),
        socket_count: endpoints.len(),
        signed_message_count: expected.signed_message_count,
        registry_root: expected.registry_root.clone(),
        trust_graph_root: expected.trust_graph_root.clone(),
        ratification_id: expected.ratification_id.clone(),
        transcript_hash: expected.transcript_hash.clone(),
        converged,
        restart_equivalent,
        tampered_signature_rejected,
        oversized_frame_rejected,
        live_authority_remained_disabled,
        probes,
        replay,
    })
}

fn handle_request(
    service: &mut CobaltShadowService,
    request: CobaltShadowRpcRequest,
) -> CobaltShadowRpcResponse {
    let result = match request {
        CobaltShadowRpcRequest::Probe => {
            probe_service(service).and_then(|probe| serde_json::to_value(probe).map_err(json_error))
        }
        CobaltShadowRpcRequest::Snapshot => {
            serde_json::to_value(service.state()).map_err(json_error)
        }
        CobaltShadowRpcRequest::Replay => service
            .replay_protocol_state()
            .and_then(|records| serde_json::to_value(records).map_err(json_error)),
        CobaltShadowRpcRequest::HistoryRange {
            start_sequence,
            limit,
        } => service
            .history_range(start_sequence, limit)
            .and_then(|range| serde_json::to_value(range).map_err(json_error)),
        CobaltShadowRpcRequest::VerifyHistoryRange { range } => {
            service.verify_history_range(&range).and_then(|()| {
                serde_json::to_value(json!({
                    "verified": true,
                    "start_sequence": range.start_sequence,
                    "end_sequence": range.end_sequence,
                    "range_hash": range.range_hash,
                }))
                .map_err(json_error)
            })
        }
        CobaltShadowRpcRequest::CatchUp { range } => service
            .catch_up_history(&range)
            .and_then(|status| serde_json::to_value(status).map_err(json_error)),
        CobaltShadowRpcRequest::Submit { message } => service
            .receive(message)
            .and_then(|outcome| serde_json::to_value(outcome).map_err(json_error)),
        CobaltShadowRpcRequest::Process => service.process_all().and_then(|processed| {
            serde_json::to_value(json!({"processed": processed})).map_err(json_error)
        }),
        CobaltShadowRpcRequest::Commit { transcript } => service
            .commit_protocol_transcript(&transcript)
            .and_then(|decision| serde_json::to_value(decision).map_err(json_error)),
        CobaltShadowRpcRequest::Shutdown => Ok(json!({"stopping": true})),
    };
    match result {
        Ok(value) => CobaltShadowRpcResponse {
            schema: COBALT_SHADOW_RPC_SCHEMA.to_string(),
            ok: true,
            result: Some(value),
            error: None,
        },
        Err(error) => error_response(&error),
    }
}

fn read_request(stream: &mut TcpStream) -> io::Result<CobaltShadowRpcRequest> {
    let mut bytes = Vec::new();
    stream
        .take((MAX_RPC_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RPC_FRAME_BYTES {
        return Err(invalid("RPC request exceeds frame bound"));
    }
    serde_json::from_slice(&bytes).map_err(json_error)
}

fn write_response(stream: &mut TcpStream, response: CobaltShadowRpcResponse) -> io::Result<()> {
    let encoded = serde_json::to_vec(&response).map_err(json_error)?;
    if encoded.len() > MAX_RPC_FRAME_BYTES {
        return Err(invalid("RPC response exceeds frame bound"));
    }
    stream.write_all(&encoded)
}

fn error_response(error: &io::Error) -> CobaltShadowRpcResponse {
    CobaltShadowRpcResponse {
        schema: COBALT_SHADOW_RPC_SCHEMA.to_string(),
        ok: false,
        result: None,
        error: Some(error.to_string()),
    }
}

fn send_oversized_frame(endpoint: SocketAddr) -> io::Result<bool> {
    let mut stream = TcpStream::connect_timeout(&endpoint, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(&vec![b'x'; MAX_RPC_FRAME_BYTES + 1])?;
    stream.shutdown(Shutdown::Write)?;
    let mut response_bytes = Vec::new();
    stream
        .take((MAX_RPC_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut response_bytes)?;
    let response: CobaltShadowRpcResponse =
        serde_json::from_slice(&response_bytes).map_err(json_error)?;
    Ok(!response.ok
        && response
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("frame bound"))
}

pub fn validate_listen_address(address: SocketAddr, allow_private_network: bool) -> io::Result<()> {
    if address.ip().is_loopback() {
        return Ok(());
    }
    if allow_private_network && is_private(address.ip()) {
        return Ok(());
    }
    Err(invalid(
        "listen address must be loopback or an explicitly allowed private-network address",
    ))
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

pub fn parse_endpoint(value: &str) -> io::Result<SocketAddr> {
    value
        .parse()
        .map_err(|_| invalid("endpoint must be an IP socket address such as 127.0.0.1:9700"))
}

pub fn read_transcript(path: &Path) -> io::Result<CobaltShadowProtocolTranscript> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_RPC_FRAME_BYTES as u64 {
        return Err(invalid("transcript file exceeds frame bound"));
    }
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(json_error)
}

fn json_error(error: serde_json::Error) -> io::Error {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("postfiat-cobalt-shadow-network-{nonce}"))
    }

    #[test]
    fn local_network_drill_runs_real_signed_protocol_over_three_sockets() {
        let root = test_dir();
        let report = run_cobalt_shadow_network_drill(&root).expect("network drill");
        assert!(report.ok, "{report:#?}");
        assert_eq!(report.socket_count, 3);
        assert_eq!(report.signed_message_count, 25);
        assert!(report.converged);
        assert!(report.restart_equivalent);
        assert!(report.tampered_signature_rejected);
        assert!(report.oversized_frame_rejected);
        assert!(report.live_authority_remained_disabled);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn public_listener_requires_explicit_private_network_and_rejects_public_ip() {
        assert!(
            validate_listen_address("127.0.0.1:9700".parse().expect("loopback"), false).is_ok()
        );
        assert!(validate_listen_address("10.1.2.3:9700".parse().expect("private"), false).is_err());
        assert!(validate_listen_address("10.1.2.3:9700".parse().expect("private"), true).is_ok());
        assert!(validate_listen_address("8.8.8.8:9700".parse().expect("public"), true).is_err());
    }

    #[test]
    fn sidecar_policy_is_bounded_and_not_coupled_to_validator_lifecycle() {
        let unit = include_str!("../../../systemd/postfiat-cobalt-shadow.service.example");
        assert!(unit.contains("User=postfiat-cobalt"));
        assert!(unit.contains("InaccessiblePaths=/var/lib/postfiat"));
        assert!(unit.contains("MemoryMax=128M"));
        assert!(unit.contains("TasksMax=32"));
        assert!(unit.contains("CapabilityBoundingSet="));
        for forbidden in [
            "Requires=postfiat-node",
            "PartOf=postfiat-node",
            "BindsTo=postfiat-node",
        ] {
            assert!(!unit.contains(forbidden));
        }
    }
}
