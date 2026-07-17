use crate::{
    decode_fastswap_wire_result_v2, encode_fastswap_wire_payload_v2, fastswap_capabilities_request,
    response_from_json, FastSwapRpcTransportV1, RpcRequest, RpcResponse,
    FASTSWAP_WIRE_GZIP_BASE64_V2, MAX_RPC_REQUEST_BYTES,
};
use postfiat_types::{FastSwapCapabilitiesV1, FastSwapCommitteeV1};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const MAX_FASTSWAP_RPC_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_FASTSWAP_ENDPOINT_BYTES: usize = 512;

type ConnectionSlot = Arc<Mutex<Option<BufReader<TcpStream>>>>;
type ConnectionPool = BTreeMap<(String, &'static str), ConnectionSlot>;

/// Bounded persistent TCP transport for a long-lived wallet process.
///
/// Connections are isolated by validator and protocol lane so a slow response
/// left outside a quorum-early result cannot head-of-line block the next phase.
/// A failed stream is discarded and is not implicitly replayed: the durable
/// wallet state machine decides whether and when an idempotent request resumes.
#[derive(Clone)]
pub struct TcpFastSwapTransportV1 {
    endpoints: Arc<BTreeMap<String, String>>,
    timeout: Duration,
    connections: Arc<Mutex<ConnectionPool>>,
    compact_payloads: Arc<AtomicBool>,
}

impl TcpFastSwapTransportV1 {
    pub fn new(endpoints: BTreeMap<String, String>, timeout: Duration) -> Result<Self, String> {
        if endpoints.is_empty() || timeout.is_zero() {
            return Err("FastSwap transport requires endpoints and a nonzero timeout".to_owned());
        }
        for (validator_id, endpoint) in &endpoints {
            if validator_id.is_empty()
                || endpoint.is_empty()
                || endpoint.len() > MAX_FASTSWAP_ENDPOINT_BYTES
            {
                return Err("invalid FastSwap validator endpoint".to_owned());
            }
        }
        Ok(Self {
            endpoints: Arc::new(endpoints),
            timeout,
            connections: Arc::new(Mutex::new(BTreeMap::new())),
            compact_payloads: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn validator_ids(&self) -> impl Iterator<Item = &str> {
        self.endpoints.keys().map(String::as_str)
    }

    fn connect(&self, endpoint: &str) -> Result<BufReader<TcpStream>, String> {
        let stream = TcpStream::connect(endpoint)
            .map_err(|error| format!("FastSwap RPC connect failed: {error}"))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .and_then(|_| stream.set_write_timeout(Some(self.timeout)))
            .and_then(|_| stream.set_nodelay(true))
            .map_err(|error| format!("FastSwap RPC socket setup failed: {error}"))?;
        Ok(BufReader::new(stream))
    }

    fn exchange(
        &self,
        mut connection: BufReader<TcpStream>,
        request: &RpcRequest,
    ) -> Result<(BufReader<TcpStream>, RpcResponse), String> {
        let mut bytes = serde_json::to_vec(request)
            .map_err(|error| format!("FastSwap RPC request encoding failed: {error}"))?;
        if bytes.len() > MAX_RPC_REQUEST_BYTES {
            return Err(format!(
                "FastSwap RPC request exceeds {MAX_RPC_REQUEST_BYTES} bytes"
            ));
        }
        bytes.push(b'\n');
        connection
            .get_mut()
            .write_all(&bytes)
            .and_then(|_| connection.get_mut().flush())
            .map_err(|error| format!("FastSwap RPC write failed: {error}"))?;
        let raw = read_bounded_line(&mut connection, MAX_FASTSWAP_RPC_RESPONSE_BYTES)
            .map_err(|error| format!("FastSwap RPC read failed: {error}"))?;
        let mut response = response_from_json(&raw)
            .map_err(|error| format!("FastSwap RPC response decode failed: {error}"))?;
        response
            .validate_protocol()
            .map_err(|error| format!("FastSwap RPC response invalid: {error}"))?;
        if response.id != request.id {
            return Err("FastSwap RPC response id mismatch".to_owned());
        }
        if let Some(encoded) = response
            .result
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .filter(|value| value.starts_with(crate::FASTSWAP_WIRE_GZIP_BASE64_V2_PREFIX))
            .map(str::to_owned)
        {
            response.result = Some(decode_fastswap_wire_result_v2(&encoded)?);
        }
        Ok((connection, response))
    }

    /// Negotiate wire v2 against the exact committee and open every normal
    /// protocol lane before any mutating request. A partial negotiation leaves
    /// compact mode disabled; callers must not start settlement on error.
    pub fn prewarm_fastswap_runtime_v2(
        &self,
        committee: &FastSwapCommitteeV1,
    ) -> Result<(), String> {
        self.compact_payloads.store(false, Ordering::Release);
        let mut capabilities = BTreeMap::new();
        std::thread::scope(|scope| {
            let workers = committee
                .validators
                .iter()
                .map(|validator| {
                    let validator_id = validator.validator_id.clone();
                    let transport = self.clone();
                    scope.spawn(move || {
                        let response = transport.call(
                            &validator_id,
                            &fastswap_capabilities_request(format!(
                                "fastswap-wire-v2-capabilities-{validator_id}"
                            )),
                        )?;
                        let value = response
                            .result_as::<FastSwapCapabilitiesV1>()
                            .map_err(|error| format!("{error:?}"))?;
                        Ok::<_, String>((validator_id, value))
                    })
                })
                .collect::<Vec<_>>();
            for worker in workers {
                let (validator_id, value) = worker
                    .join()
                    .map_err(|_| "FastSwap capability worker panicked".to_owned())??;
                capabilities.insert(validator_id, value);
            }
            Ok::<(), String>(())
        })?;
        for validator in &committee.validators {
            let capability = capabilities
                .get(&validator.validator_id)
                .ok_or_else(|| format!("missing capability from {}", validator.validator_id))?;
            if !capability.enabled
                || capability.committee != committee.domain
                || capability.terminal_receipt_code != "fastswap_applied"
                || !capability
                    .wire_codecs
                    .iter()
                    .any(|codec| codec == FASTSWAP_WIRE_GZIP_BASE64_V2)
            {
                return Err(format!(
                    "{} does not advertise exact FastSwap wire v2 support",
                    validator.validator_id
                ));
            }
        }
        for validator in &committee.validators {
            let endpoint = self
                .endpoints
                .get(&validator.validator_id)
                .ok_or_else(|| format!("missing endpoint for {}", validator.validator_id))?;
            for lane in ["settlement", "catch-up"] {
                let key = (validator.validator_id.clone(), lane);
                let slot = {
                    let mut connections = self
                        .connections
                        .lock()
                        .map_err(|_| "FastSwap connection pool lock poisoned".to_owned())?;
                    connections
                        .entry(key)
                        .or_insert_with(|| Arc::new(Mutex::new(None)))
                        .clone()
                };
                let mut cached = slot
                    .lock()
                    .map_err(|_| "FastSwap connection lane lock poisoned".to_owned())?;
                if cached.is_none() {
                    *cached = Some(self.connect(endpoint)?);
                }
            }
        }
        self.compact_payloads.store(true, Ordering::Release);
        Ok(())
    }

    pub fn compact_payloads_enabled(&self) -> bool {
        self.compact_payloads.load(Ordering::Acquire)
    }
}

impl FastSwapRpcTransportV1 for TcpFastSwapTransportV1 {
    fn call(&self, validator_id: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
        request
            .validate_protocol()
            .map_err(|error| format!("FastSwap RPC request invalid: {error}"))?;
        let endpoint = self
            .endpoints
            .get(validator_id)
            .ok_or_else(|| format!("no endpoint for validator `{validator_id}`"))?;
        let lane =
            if self.compact_payloads_enabled() && is_fastswap_settlement_method(&request.method) {
                "settlement"
            } else {
                protocol_lane(&request.method)
            };
        let key = (validator_id.to_owned(), lane);
        let slot = {
            let mut connections = self
                .connections
                .lock()
                .map_err(|_| "FastSwap connection pool lock poisoned".to_owned())?;
            connections
                .entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(None)))
                .clone()
        };
        // A quorum-early wave may return while one validator's worker is still
        // completing. Serialize only that validator+phase lane so the next
        // wallet operation reuses the same stream instead of opening an
        // unbounded duplicate connection. Other validators and phases remain
        // fully parallel.
        let mut cached = slot
            .lock()
            .map_err(|_| "FastSwap connection lane lock poisoned".to_owned())?;
        let connection = match cached.take() {
            Some(connection) => connection,
            None => self.connect(endpoint)?,
        };
        let (connection, response) = self.exchange(connection, request)?;
        *cached = Some(connection);
        Ok(response)
    }

    fn encode_fastswap_payload(&self, json: &str) -> Result<String, String> {
        if self.compact_payloads_enabled() {
            encode_fastswap_wire_payload_v2(json)
        } else {
            Ok(json.to_owned())
        }
    }
}

fn protocol_lane(method: &str) -> &'static str {
    match method {
        "fastswap_preview" | "fastlane_asset_control_preview" => "preview",
        "fastswap_prepare" | "fastlane_asset_control_prepare" => "prepare",
        "fastswap_commit" => "commit",
        "fastswap_apply" | "fastlane_asset_control_apply" => "apply",
        "fastswap_new_round_vote"
        | "fastswap_propose_round"
        | "fastswap_precommit"
        | "fastswap_commit_round"
        | "fastswap_cancel_apply" => "recovery",
        "fastswap_catch_up" | "fastlane_asset_control_catch_up" => "catch-up",
        "fastlane_exit" => "exit",
        _ => "read",
    }
}

fn is_fastswap_settlement_method(method: &str) -> bool {
    matches!(
        method,
        "fastswap_preview" | "fastswap_prepare" | "fastswap_commit" | "fastswap_apply"
    )
}

fn read_bounded_line<R: BufRead>(reader: &mut R, max_bytes: usize) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "RPC server closed without a complete response",
            ));
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if bytes.len().saturating_add(take) > max_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("RPC response exceeds {max_bytes} bytes"),
            ));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        if bytes.last() == Some(&b'\n') {
            return String::from_utf8(bytes)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{error_response, status_request};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn persistent_transport_reuses_one_lane_connection_and_checks_response_id() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            server_accepts.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream);
            for expected_id in ["read-1", "read-2"] {
                let mut line = String::new();
                reader.read_line(&mut line).expect("read request");
                let request = serde_json::from_str::<RpcRequest>(&line).expect("request JSON");
                assert_eq!(request.id, expected_id);
                let response = error_response(request.id, "test_only", "typed response", vec![]);
                serde_json::to_writer(reader.get_mut(), &response).expect("response JSON");
                reader.get_mut().write_all(b"\n").expect("response newline");
                reader.get_mut().flush().expect("response flush");
            }
        });
        let transport = TcpFastSwapTransportV1::new(
            BTreeMap::from([("validator-0".to_owned(), address.to_string())]),
            Duration::from_secs(2),
        )
        .expect("transport");
        for id in ["read-1", "read-2"] {
            let response = transport
                .call("validator-0", &status_request(id))
                .expect("transport call");
            assert_eq!(response.id, id);
            assert!(!response.ok);
        }
        server.join().expect("server");
        assert_eq!(accepts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_same_lane_calls_wait_for_and_reuse_one_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let accepts = Arc::new(AtomicUsize::new(0));
        let server_accepts = accepts.clone();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            server_accepts.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream);
            for index in 0..2 {
                let mut line = String::new();
                reader.read_line(&mut line).expect("read request");
                let request = serde_json::from_str::<RpcRequest>(&line).expect("request JSON");
                if index == 0 {
                    std::thread::sleep(Duration::from_millis(50));
                }
                let response = error_response(request.id, "test_only", "typed response", vec![]);
                serde_json::to_writer(reader.get_mut(), &response).expect("response JSON");
                reader.get_mut().write_all(b"\n").expect("response newline");
                reader.get_mut().flush().expect("response flush");
            }
        });
        let transport = TcpFastSwapTransportV1::new(
            BTreeMap::from([("validator-0".to_owned(), address.to_string())]),
            Duration::from_secs(2),
        )
        .expect("transport");
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let workers = ["read-a", "read-b"].map(|id| {
            let transport = transport.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                transport.call("validator-0", &status_request(id))
            })
        });
        barrier.wait();
        for worker in workers {
            let response = worker.join().expect("worker").expect("transport call");
            assert!(!response.ok);
        }
        drop(transport);
        server.join().expect("server");
        assert_eq!(accepts.load(Ordering::SeqCst), 1);
    }
}
