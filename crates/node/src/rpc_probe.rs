// One read-only status exchange per invocation; the caller can choose its own retry policy.
struct RpcProbeResult {
    height: u64,
    tip_prefix: String,
    round_trip_ms: u128,
}

fn rpc_probe(host: &str, port: u16, timeout_ms: u64) -> Result<RpcProbeResult, String> {
    if timeout_ms == 0 {
        return Err("--timeout-ms must be positive".to_string());
    }
    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let address = socket_address(host, port);
    let address = address
        .to_socket_addrs()
        .map_err(|error| format!("connect refused: address resolution failed: {error}"))?
        .next()
        .ok_or_else(|| "connect refused: endpoint has no address".to_string())?;
    let remaining = timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err("connect timeout".to_string());
    }
    let mut stream = TcpStream::connect_timeout(&address, remaining).map_err(|error| {
        if error.kind() == std::io::ErrorKind::TimedOut {
            "connect timeout".to_string()
        } else {
            format!("connect refused: {error}")
        }
    })?;
    let request = RpcRequest::empty("rpc-probe", "status");
    let mut wire =
        serde_json::to_vec(&request).map_err(|error| format!("malformed request: {error}"))?;
    wire.push(b'\n');
    let remaining = timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err("no response within timeout".to_string());
    }
    stream
        .set_write_timeout(Some(remaining))
        .map_err(|error| format!("send failed: {error}"))?;
    stream
        .write_all(&wire)
        .map_err(|error| format!("send failed: {error}"))?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| format!("send failed: {error}"))?;

    // Stop at the first newline; a keep-alive peer need not close its write side.
    let mut response_bytes = Vec::new();
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err("no response within timeout".to_string());
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|error| format!("read failed: {error}"))?;
        let mut chunk = [0_u8; 4096];
        match stream.read(&mut chunk) {
            Ok(0) if response_bytes.is_empty() => {
                return Err("no response: peer closed".to_string())
            }
            Ok(0) => return Err("malformed response: missing newline".to_string()),
            Ok(count) => {
                let newline = chunk[..count].iter().position(|byte| *byte == b'\n');
                response_bytes.extend_from_slice(&chunk[..newline.unwrap_or(count)]);
                if response_bytes.len() > 1_048_576 {
                    return Err("malformed response: response too large".to_string());
                }
                if newline.is_some() {
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                return Err("no response within timeout".to_string());
            }
            Err(error) => return Err(format!("read failed: {error}")),
        }
    }
    let response: RpcResponse = serde_json::from_slice(&response_bytes)
        .map_err(|error| format!("malformed response: {error}"))?;
    response
        .validate_protocol()
        .map_err(|error| format!("malformed response: {error}"))?;
    if response.id != request.id || response.version != request.version {
        return Err("malformed response: id or version mismatch".to_string());
    }
    if !response.ok {
        let code = response
            .error
            .as_ref()
            .map_or("rpc_error", |error| error.code.as_str());
        return Err(format!("ok:false: {code}"));
    }
    let result = response
        .result
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "malformed response: status result must be an object".to_string())?;
    let height = result
        .get("block_height")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "malformed response: missing block_height".to_string())?;
    let tip = result
        .get("block_tip_hash")
        .and_then(serde_json::Value::as_str)
        .filter(|tip| !tip.is_empty())
        .ok_or_else(|| "malformed response: missing block_tip_hash".to_string())?;
    if started.elapsed() > timeout {
        return Err("no response within timeout".to_string());
    }
    Ok(RpcProbeResult {
        height,
        tip_prefix: tip.chars().take(12).collect(),
        round_trip_ms: started.elapsed().as_millis(),
    })
}
