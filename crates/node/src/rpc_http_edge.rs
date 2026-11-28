//! `postfiat-node rpc-http-edge`: a loopback HTTP front for `rpc-serve`.
//!
//! `rpc-serve` speaks one JSON request per line over TCP. Public clients and
//! TLS proxies speak HTTP. This edge takes `POST /rpc` with one JSON request,
//! checks the method against a fixed list, forwards the request to the
//! loopback `rpc-serve`, and returns its one-line reply as the response body.
//! `GET /healthz` answers 200 only while the node behind it is ready.
//!
//! It holds no key, opens no store, and never binds a public address: TLS and
//! per-address rate limits belong to the proxy in front of it.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The `read_only_public` posture of docs/runbooks/rpc-method-inventory.json,
/// sorted. A replica profile serves exactly these, and so does the edge.
pub const READ_ONLY_PUBLIC_METHODS: &[&str] = &[
    "account",
    "account_assets",
    "account_bridge_holds",
    "account_escrows",
    "account_lines",
    "account_nfts",
    "account_offers",
    "account_tx",
    "account_tx_index_status",
    "archive_window",
    "asset_bridge_holds",
    "asset_fee_quote",
    "asset_info",
    "asset_orchard_action_status",
    "asset_orchard_pricing_claim",
    "asset_orchard_private_egress_binding",
    "atomic_settlement_template",
    "atomic_swap_fee_quote",
    "batch_archive",
    "block_vote_records",
    "blocks",
    "book_offers",
    "bridge_status",
    "escrow_fee_quote",
    "escrow_info",
    "external_lane_info",
    "fastlane_asset_control_preview",
    "fastpay_fee_quote",
    "fastpay_recovery_committee",
    "fastswap_capabilities",
    "fastswap_checkpoint_status",
    "fastswap_effects",
    "fastswap_objects",
    "fastswap_policy",
    "fastswap_preview",
    "fastswap_propose_round",
    "fastswap_status",
    "fastswap_votes",
    "fee",
    "fx_fix_info",
    "fx_fix_list",
    "fx_fix_quote",
    "fx_fix_reservation_info",
    "issuer_assets",
    "issuer_nfts",
    "ledger",
    "manifests",
    "market_ops_status",
    "mempool_status",
    "metrics",
    "nav_reserve_proof_status",
    "navcoin_bridge_claims",
    "navcoin_bridge_packet",
    "navcoin_bridge_receipt_replay",
    "navcoin_bridge_routes",
    "navcoin_bridge_supply_status",
    "nft_fee_quote",
    "nft_info",
    "offer_fee_quote",
    "offer_info",
    "orchard_pool_report",
    "owned_certificate",
    "owned_fence_status",
    "owned_objects",
    "owned_recovery_capabilities",
    "owned_recovery_status",
    "pftl_uniswap_receipt",
    "pfusdc_egress_witness",
    "pfusdc_ingress_preflight",
    "private_account_lines",
    "private_pool_market",
    "private_pool_page",
    "private_pool_receipts",
    "private_pool_snapshot",
    "private_readiness",
    "receipts",
    "server_info",
    "shield_disclose",
    "shield_scan",
    "shield_turnstile",
    "status",
    "transfer_fee_quote",
    "tx",
    "validators",
    "vault_bridge_route",
    "vault_bridge_status",
    "verify_blocks",
    "verify_bridge",
    "verify_mempool",
    "verify_shielded",
    "verify_state",
    "yolo_target_receipt",
];

/// Served by the edge only with `--allow-private-action-admission`.
pub const PRIVATE_ACTION_METHODS: &[&str] = &["private_action_receipt", "private_action_submit"];

pub const MAX_RPC_REQUEST_BYTES: usize = 65_536;
const MAX_HEADER_BYTES: usize = 8_192;
const MAX_UPSTREAM_REPLY_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct RpcHttpEdgeOptions {
    pub listen_host: String,
    pub listen_port: u16,
    pub upstream_host: String,
    pub upstream_port: u16,
    pub allow_private_action_admission: bool,
    pub max_connections: usize,
    pub timeout_ms: u64,
    /// Requests one client address may make per second, and how many it may
    /// make at once after a pause. `0` switches the limit off.
    pub rate_per_second: u32,
    pub burst: u32,
    /// Stop after this many connections; `None` serves until killed.
    pub max_requests: Option<u64>,
}

/// Token bucket per client address. The address is the last entry of
/// `X-Forwarded-For`, which the TLS proxy on the same host appends; a direct
/// loopback caller without the header shares one bucket.
///
/// The boundary this rests on: the edge listens on loopback only, so whoever
/// writes that header is a process on this host. The limit is a budget for
/// remote clients behind the proxy. It is not a defence against a local
/// process, which can name any address and can reach `rpc-serve` without the
/// edge anyway. The proxy must write the header itself and never pass one
/// through from the client (Caddy's default).
#[derive(Debug, Default)]
struct RateLimiter {
    buckets: std::collections::HashMap<String, (f64, std::time::Instant)>,
}

const RATE_LIMITER_MAX_CLIENTS: usize = 50_000;

impl RateLimiter {
    fn allow(
        &mut self,
        client: &str,
        rate_per_second: u32,
        burst: u32,
        now: std::time::Instant,
    ) -> bool {
        if rate_per_second == 0 {
            return true;
        }
        if self.buckets.len() >= RATE_LIMITER_MAX_CLIENTS && !self.buckets.contains_key(client) {
            // Full buckets are idle clients; dropping them loses nothing.
            let rate = f64::from(rate_per_second);
            let full = f64::from(burst);
            self.buckets.retain(|_, (tokens, last)| {
                *tokens + now.duration_since(*last).as_secs_f64() * rate < full
            });
            if self.buckets.len() >= RATE_LIMITER_MAX_CLIENTS {
                return false;
            }
        }
        let (tokens, last) = self
            .buckets
            .entry(client.to_string())
            .or_insert((f64::from(burst), now));
        let refilled = (*tokens
            + now.duration_since(*last).as_secs_f64() * f64::from(rate_per_second))
        .min(f64::from(burst));
        *last = now;
        if refilled >= 1.0 {
            *tokens = refilled - 1.0;
            true
        } else {
            *tokens = refilled;
            false
        }
    }
}

pub fn edge_serves_method(method: &str, allow_private_action_admission: bool) -> bool {
    READ_ONLY_PUBLIC_METHODS.binary_search(&method).is_ok()
        || (allow_private_action_admission && PRIVATE_ACTION_METHODS.contains(&method))
}

fn is_loopback(host: &str) -> bool {
    crate::validator_top::is_loopback_host(host)
}

pub fn run_rpc_http_edge(options: RpcHttpEdgeOptions) -> Result<(), String> {
    run_rpc_http_edge_with_listener(options, None)
}

pub fn run_rpc_http_edge_with_listener(
    options: RpcHttpEdgeOptions,
    listener: Option<TcpListener>,
) -> Result<(), String> {
    if !is_loopback(&options.listen_host) {
        return Err(format!(
            "rpc_http_edge_bind_forbidden: the edge listens on loopback only (got `{}`); put a TLS proxy in front of it",
            options.listen_host
        ));
    }
    if !is_loopback(&options.upstream_host) {
        return Err(format!(
            "rpc_http_edge_upstream_forbidden: the edge forwards to a loopback rpc-serve only (got `{}`)",
            options.upstream_host
        ));
    }
    if options.max_connections == 0 {
        return Err("--max-connections must be at least 1".to_string());
    }
    let listener = match listener {
        Some(listener) => listener,
        None => TcpListener::bind((options.listen_host.as_str(), options.listen_port))
            .map_err(|error| format!("rpc http edge bind failed: {error}"))?,
    };
    let active = Arc::new(AtomicUsize::new(0));
    let limiter = Arc::new(std::sync::Mutex::new(RateLimiter::default()));
    let options = Arc::new(options);
    let mut served = 0_u64;
    let mut workers = Vec::new();
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let timeout = Duration::from_millis(options.timeout_ms);
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        if active.fetch_add(1, Ordering::SeqCst) >= options.max_connections {
            active.fetch_sub(1, Ordering::SeqCst);
            let mut stream = stream;
            write_error(&mut stream, 503, "edge_busy", "too many connections");
        } else {
            let worker_active = Arc::clone(&active);
            let worker_options = Arc::clone(&options);
            let worker_limiter = Arc::clone(&limiter);
            let worker = std::thread::spawn(move || {
                let mut stream = stream;
                handle_connection(&mut stream, &worker_options, &worker_limiter);
                worker_active.fetch_sub(1, Ordering::SeqCst);
            });
            if options.max_requests.is_some() {
                workers.push(worker);
            }
        }
        served += 1;
        if options.max_requests.is_some_and(|max| served >= max) {
            break;
        }
    }
    for worker in workers {
        let _ = worker.join();
    }
    Ok(())
}

struct HttpRequest {
    method: String,
    path: String,
    client: String,
    body: Vec<u8>,
}

enum ReadError {
    Status(u16, &'static str, &'static str),
    Closed,
}

/// Read one request. The whole of it, head and body, must arrive within
/// `budget`: the per-read socket timeout alone would let a client that sends
/// one byte at a time hold a connection for hours. Reads are buffered; the
/// edge serves one request per connection, so nothing read ahead is lost.
fn read_http_request(stream: &mut TcpStream, budget: Duration) -> Result<HttpRequest, ReadError> {
    let deadline = std::time::Instant::now() + budget;
    let too_slow = ReadError::Status(408, "request_timeout", "the request did not arrive in time");
    let mut reader = std::io::BufReader::with_capacity(MAX_HEADER_BYTES, &*stream);
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if std::time::Instant::now() >= deadline {
            return Err(too_slow);
        }
        match reader.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return Err(ReadError::Closed),
        }
        if head.len() > MAX_HEADER_BYTES {
            return Err(ReadError::Status(
                431,
                "headers_too_large",
                "request headers exceed 8 KiB",
            ));
        }
    }
    let head = String::from_utf8(head)
        .map_err(|_| ReadError::Status(400, "bad_request", "request head is not UTF-8"))?;
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(path), Some(version)) = (
        request_line.next(),
        request_line.next(),
        request_line.next(),
    ) else {
        return Err(ReadError::Status(
            400,
            "bad_request",
            "malformed request line",
        ));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(ReadError::Status(400, "bad_request", "HTTP/1.x only"));
    }
    let mut content_length: Option<usize> = None;
    let mut client = "direct".to_string();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name == "transfer-encoding" {
            return Err(ReadError::Status(
                411,
                "length_required",
                "send Content-Length, not a transfer encoding",
            ));
        }
        if name == "x-forwarded-for" {
            if let Some(last) = value
                .rsplit(',')
                .next()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                client = last.chars().take(64).collect();
            }
        }
        if name == "content-length" {
            let parsed = value.trim().parse::<usize>().map_err(|_| {
                ReadError::Status(400, "bad_request", "Content-Length is not a number")
            })?;
            if content_length.is_some_and(|previous| previous != parsed) {
                return Err(ReadError::Status(
                    400,
                    "bad_request",
                    "conflicting Content-Length",
                ));
            }
            content_length = Some(parsed);
        }
    }
    let length = content_length.unwrap_or(0);
    if length > MAX_RPC_REQUEST_BYTES {
        return Err(ReadError::Status(
            413,
            "request_too_large",
            "request body exceeds 64 KiB",
        ));
    }
    let mut body = vec![0_u8; length];
    let mut filled = 0;
    while filled < length {
        if std::time::Instant::now() >= deadline {
            return Err(too_slow);
        }
        match reader.read(&mut body[filled..]) {
            Ok(0) | Err(_) => return Err(ReadError::Closed),
            Ok(count) => filled += count,
        }
    }
    Ok(HttpRequest {
        method: method.to_string(),
        path: path.split('?').next().unwrap_or_default().to_string(),
        client,
        body,
    })
}

fn write_response(stream: &mut TcpStream, status: u16, body: &[u8]) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        411 => "Length Required",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn write_error(stream: &mut TcpStream, status: u16, code: &str, message: &str) {
    let body = serde_json::json!({ "ok": false, "error": { "code": code, "message": message } });
    write_response(stream, status, body.to_string().as_bytes());
}

/// One request line to the loopback `rpc-serve`, one reply line back.
fn forward(options: &RpcHttpEdgeOptions, request_line: &str) -> Result<String, String> {
    let timeout = Duration::from_millis(options.timeout_ms);
    let mut upstream = TcpStream::connect((options.upstream_host.as_str(), options.upstream_port))
        .map_err(|error| format!("connect: {error}"))?;
    let _ = upstream.set_read_timeout(Some(timeout));
    let _ = upstream.set_write_timeout(Some(timeout));
    upstream
        .write_all(request_line.as_bytes())
        .and_then(|()| upstream.write_all(b"\n"))
        .and_then(|()| upstream.flush())
        .map_err(|error| format!("write: {error}"))?;
    let mut reply = String::new();
    BufReader::new(upstream.take(MAX_UPSTREAM_REPLY_BYTES))
        .read_line(&mut reply)
        .map_err(|error| format!("read: {error}"))?;
    if !reply.ends_with('\n') {
        return Err("the node closed the connection before a full reply".to_string());
    }
    Ok(reply.trim_end().to_string())
}

fn handle_connection(
    stream: &mut TcpStream,
    options: &RpcHttpEdgeOptions,
    limiter: &std::sync::Mutex<RateLimiter>,
) {
    let request = match read_http_request(stream, Duration::from_millis(options.timeout_ms)) {
        Ok(request) => request,
        Err(ReadError::Status(status, code, message)) => {
            return write_error(stream, status, code, message)
        }
        Err(ReadError::Closed) => return,
    };
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/healthz") => {
            let probe = r#"{"version":"postfiat-local-rpc-v1","id":"edge-healthz","method":"status","params":{}}"#;
            let ready = forward(options, probe)
                .ok()
                .and_then(|reply| serde_json::from_str::<serde_json::Value>(&reply).ok())
                .is_some_and(|reply| {
                    reply["ok"] == true && reply["result"]["replica_ready"] != false
                });
            let body = serde_json::json!({ "ready": ready }).to_string();
            write_response(stream, if ready { 200 } else { 503 }, body.as_bytes());
        }
        ("POST", "/rpc") => {
            let allowed = limiter.lock().map_or(true, |mut limiter| {
                limiter.allow(
                    &request.client,
                    options.rate_per_second,
                    options.burst,
                    std::time::Instant::now(),
                )
            });
            if !allowed {
                return write_error(
                    stream,
                    429,
                    "rate_limited",
                    "too many requests from this address",
                );
            }
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&request.body) else {
                return write_error(stream, 400, "bad_request", "the body is not JSON");
            };
            let Some(method) = value.get("method").and_then(serde_json::Value::as_str) else {
                return write_error(
                    stream,
                    400,
                    "bad_request",
                    "the body is not one RPC request object",
                );
            };
            if !edge_serves_method(method, options.allow_private_action_admission) {
                return write_error(
                    stream,
                    403,
                    "method_not_served_by_profile",
                    &format!("rpc method `{method}` is not served at this endpoint"),
                );
            }
            // Re-serialized, so the forwarded request is exactly one line.
            match forward(options, &value.to_string()) {
                Ok(reply) => write_response(stream, 200, reply.as_bytes()),
                Err(error) => write_error(stream, 502, "upstream_unavailable", &error),
            }
        }
        (_, "/rpc") | (_, "/healthz") => write_error(
            stream,
            405,
            "method_not_allowed",
            "POST /rpc or GET /healthz",
        ),
        _ => write_error(stream, 404, "not_found", "POST /rpc or GET /healthz"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Shutdown;

    fn options(upstream_port: u16) -> RpcHttpEdgeOptions {
        RpcHttpEdgeOptions {
            listen_host: "127.0.0.1".to_string(),
            listen_port: 0,
            upstream_host: "127.0.0.1".to_string(),
            upstream_port,
            allow_private_action_admission: false,
            max_connections: 8,
            timeout_ms: 5_000,
            rate_per_second: 0,
            burst: 0,
            max_requests: None,
        }
    }

    /// A stand-in `rpc-serve`: answers every line with a fixed reply and records what it got.
    fn fake_node(reply: &'static str) -> (u16, std::sync::mpsc::Receiver<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut line = String::new();
                if BufReader::new(stream.try_clone().expect("clone"))
                    .read_line(&mut line)
                    .is_ok()
                {
                    let _ = sender.send(line);
                    let _ = stream.write_all(reply.as_bytes());
                    let _ = stream.write_all(b"\n");
                }
            }
        });
        (port, receiver)
    }

    fn start_edge(mut edge: RpcHttpEdgeOptions, requests: u64) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let port = listener.local_addr().expect("addr").port();
        edge.max_requests = Some(requests);
        std::thread::spawn(move || run_rpc_http_edge_with_listener(edge, Some(listener)));
        port
    }

    fn http(port: u16, raw: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.write_all(raw.as_bytes()).expect("write");
        let _ = stream.shutdown(Shutdown::Write);
        let mut reply = String::new();
        stream.read_to_string(&mut reply).expect("read");
        let status = reply
            .split(' ')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let body = reply
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or_default()
            .to_string();
        (status, body)
    }

    fn post(port: u16, body: &str) -> (u16, String) {
        http(
            port,
            &format!(
                "POST /rpc HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
    }

    #[test]
    fn method_list_is_sorted_and_is_the_inventory_read_only_posture() {
        let mut sorted = READ_ONLY_PUBLIC_METHODS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted, READ_ONLY_PUBLIC_METHODS,
            "sorted, no repeats: binary_search depends on it"
        );
        let inventory_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/runbooks/rpc-method-inventory.json");
        let inventory: serde_json::Value =
            serde_json::from_slice(&std::fs::read(inventory_path).expect("inventory"))
                .expect("json");
        let mut expected: Vec<&str> = inventory["methods"]
            .as_array()
            .expect("methods")
            .iter()
            .filter(|method| method["posture"] == "read_only_public")
            .map(|method| method["method"].as_str().expect("name"))
            .collect();
        expected.sort_unstable();
        assert_eq!(READ_ONLY_PUBLIC_METHODS, expected.as_slice());
        let private: Vec<&str> = inventory["methods"]
            .as_array()
            .expect("methods")
            .iter()
            .filter(|method| method["posture"] == "private_action_gated")
            .map(|method| method["method"].as_str().expect("name"))
            .collect();
        assert_eq!(PRIVATE_ACTION_METHODS, private.as_slice());
    }

    #[test]
    fn edge_forwards_one_line_and_refuses_everything_else() {
        let (node_port, received) = fake_node(r#"{"ok":true,"result":{"replica_ready":true}}"#);
        let port = start_edge(options(node_port), 9);

        // A pretty-printed request reaches the node as exactly one line.
        let (status, body) = post(
            port,
            "{\n  \"id\": \"a\",\n  \"method\": \"status\",\n  \"params\": {}\n}",
        );
        assert_eq!(
            (status, body.as_str()),
            (200, r#"{"ok":true,"result":{"replica_ready":true}}"#)
        );
        let line = received.recv().expect("forwarded");
        assert_eq!(line.matches('\n').count(), 1);
        assert!(line.contains("\"method\":\"status\""), "{line}");

        // State-changing, operator-only and unknown methods never reach the node.
        for method in [
            "mempool_submit_signed_payment_v2",
            "private_action_submit",
            "nobody_listed_this",
        ] {
            let (status, body) = post(
                port,
                &format!(r#"{{"id":"b","method":"{method}","params":{{}}}}"#),
            );
            assert_eq!(status, 403, "{method}");
            assert!(body.contains("method_not_served_by_profile"), "{body}");
        }
        assert!(
            received.try_recv().is_err(),
            "a refused method must not be forwarded"
        );

        assert_eq!(post(port, "not json").0, 400);
        assert_eq!(post(port, "[1,2]").0, 400);
        assert_eq!(http(port, "GET /rpc HTTP/1.1\r\nHost: x\r\n\r\n").0, 405);
        assert_eq!(http(port, "GET /other HTTP/1.1\r\nHost: x\r\n\r\n").0, 404);
        let big = format!(
            "POST /rpc HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
            MAX_RPC_REQUEST_BYTES + 1
        );
        assert_eq!(http(port, &big).0, 413);
    }

    #[test]
    fn admission_is_an_explicit_switch_and_health_follows_the_node() {
        let (node_port, received) = fake_node(r#"{"ok":true,"result":{"replica_ready":false}}"#);
        let mut edge = options(node_port);
        edge.allow_private_action_admission = true;
        let port = start_edge(edge, 3);
        let (status, _) = post(
            port,
            r#"{"id":"c","method":"private_action_submit","params":{}}"#,
        );
        assert_eq!(status, 200);
        assert!(received
            .recv()
            .expect("forwarded")
            .contains("private_action_submit"));
        // Still no transaction submission.
        assert_eq!(
            post(
                port,
                r#"{"id":"d","method":"mempool_submit_signed_payment_v2","params":{}}"#
            )
            .0,
            403
        );
        // The node says it is behind: the proxy in front must stop routing to it.
        let (status, body) = http(port, "GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n");
        assert_eq!((status, body.as_str()), (503, r#"{"ready":false}"#));

        // A dead node is a 502 for a request and a 503 for health.
        let dead = TcpListener::bind(("127.0.0.1", 0))
            .expect("bind")
            .local_addr()
            .expect("addr")
            .port();
        let port = start_edge(options(dead), 2);
        assert_eq!(
            post(port, r#"{"id":"e","method":"status","params":{}}"#).0,
            502
        );
        assert_eq!(
            http(port, "GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n").0,
            503
        );
    }

    #[test]
    fn each_client_address_has_its_own_request_budget() {
        let start = std::time::Instant::now();
        let mut limiter = RateLimiter::default();
        // A burst of 3, then refused; another address is unaffected.
        for _ in 0..3 {
            assert!(limiter.allow("203.0.113.7", 2, 3, start));
        }
        assert!(!limiter.allow("203.0.113.7", 2, 3, start));
        assert!(limiter.allow("203.0.113.8", 2, 3, start));
        // Half a second later one request has been earned back, not two.
        let later = start + Duration::from_millis(500);
        assert!(limiter.allow("203.0.113.7", 2, 3, later));
        assert!(!limiter.allow("203.0.113.7", 2, 3, later));
        // The budget never grows past the burst.
        let much_later = start + Duration::from_secs(3600);
        for _ in 0..3 {
            assert!(limiter.allow("203.0.113.7", 2, 3, much_later));
        }
        assert!(!limiter.allow("203.0.113.7", 2, 3, much_later));
        // Rate 0 switches the limit off.
        assert!((0..100).all(|_| limiter.allow("x", 0, 0, start)));

        // Over HTTP: the address is the last X-Forwarded-For entry, the one the local proxy wrote.
        let (node_port, _received) = fake_node(r#"{"ok":true,"result":{}}"#);
        let mut edge = options(node_port);
        edge.rate_per_second = 1;
        edge.burst = 1;
        let port = start_edge(edge, 3);
        let call = |forwarded: &str| {
            let body = r#"{"id":"r","method":"status","params":{}}"#;
            http(
                port,
                &format!(
                    "POST /rpc HTTP/1.1\r\nHost: x\r\nX-Forwarded-For: {forwarded}\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                ),
            )
            .0
        };
        assert_eq!(call("198.51.100.1, 203.0.113.7"), 200);
        assert_eq!(
            call("198.51.100.99, 203.0.113.7"),
            429,
            "a spoofed first entry buys nothing"
        );
        assert_eq!(call("203.0.113.8"), 200);
    }

    #[test]
    fn a_request_that_trickles_in_is_cut_off_at_the_budget() {
        let (node_port, received) = fake_node(r#"{"ok":true,"result":{}}"#);
        let mut edge = options(node_port);
        edge.timeout_ms = 400;
        let port = start_edge(edge, 1);
        // One header byte every 100 ms: each read is well inside the socket
        // timeout, and the request as a whole never arrives.
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let started = std::time::Instant::now();
        let mut cut_off = false;
        for byte in b"POST /rpc HTTP/1.1\r\nHost: x\r\nX-Slow: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" {
            if stream.write_all(&[*byte]).is_err() {
                cut_off = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
            if started.elapsed() > Duration::from_secs(3) {
                break;
            }
        }
        let mut reply = String::new();
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.read_to_string(&mut reply);
        assert!(
            cut_off || reply.contains("408") || reply.contains("request_timeout"),
            "the edge kept reading after its budget: {reply:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        assert!(received.try_recv().is_err(), "nothing reaches the node");
    }

    #[test]
    fn edge_binds_and_forwards_on_loopback_only() {
        let mut public = options(1);
        public.listen_host = "0.0.0.0".to_string();
        assert!(run_rpc_http_edge(public)
            .expect_err("public bind")
            .contains("rpc_http_edge_bind_forbidden"));
        let mut remote = options(1);
        remote.upstream_host = "10.0.0.1".to_string();
        assert!(run_rpc_http_edge(remote)
            .expect_err("remote upstream")
            .contains("rpc_http_edge_upstream_forbidden"));
    }
}
