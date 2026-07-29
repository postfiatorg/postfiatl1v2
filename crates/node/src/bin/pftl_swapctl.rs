use postfiat_node::{
    sign_pftl_swap_intent_with_key_file, PftlSwapDirection, PftlSwapIntentV1, PftlSwapOutputMode,
    PftlSwapQuoteRequestV1, PftlSwapQuoteV1, SignedPftlSwapIntentV1, PFTL_SWAP_INTENT_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

const RECORD_SCHEMA: &str = "postfiat.pftl_swapctl.private_record.v1";
const PUBLIC_RESULT_SCHEMA: &str = "postfiat.pftl_swapctl.public_result.v1";
const MAX_HTTP_BODY_BYTES: usize = 2 << 20;
const MAX_PRIVATE_FILE_BYTES: usize = 4 << 20;

#[derive(Debug)]
enum Command {
    Execute(ExecuteConfig),
    Resume(ResumeConfig),
}

#[derive(Debug)]
struct ExecuteConfig {
    address: SocketAddr,
    key_file: PathBuf,
    principal: String,
    controlled_wallet_id: String,
    direction: PftlSwapDirection,
    output_mode: PftlSwapOutputMode,
    nav_amount_atoms: u64,
    input_reference: InputReference,
    idempotency_key: String,
    record_file: PathBuf,
    request_timeout: Duration,
    completion_timeout: Duration,
}

#[derive(Debug)]
struct ResumeConfig {
    address: SocketAddr,
    record_file: PathBuf,
    request_timeout: Duration,
    completion_timeout: Duration,
}

#[derive(Debug)]
enum InputReference {
    Literal(String),
    PrivateRecord(PathBuf),
}

#[derive(Debug, Serialize, Deserialize)]
struct PrivateSwapRecord {
    schema: String,
    state: String,
    quote: PftlSwapQuoteV1,
    signed_intent: SignedPftlSwapIntentV1,
    response: Option<Value>,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: Value,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("pftl-swapctl failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    match parse_command()? {
        Command::Execute(config) => execute(config),
        Command::Resume(config) => resume(config),
    }
}

fn execute(config: ExecuteConfig) -> io::Result<()> {
    if config.record_file.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "private record already exists; use `resume` to preserve the signed lineage",
        ));
    }
    let input_reference = resolve_input_reference(&config.input_reference)?;
    let quote_request = PftlSwapQuoteRequestV1 {
        direction: config.direction,
        nav_amount_atoms: config.nav_amount_atoms,
        output_mode: config.output_mode,
    };
    let quote_response = request_json(
        config.address,
        "POST",
        "/v1/quote",
        Some(&serde_json::to_value(quote_request).map_err(invalid_data)?),
        config.request_timeout,
    )?;
    require_success(&quote_response, "quote")?;
    let quote: PftlSwapQuoteV1 = serde_json::from_value(
        quote_response
            .body
            .get("quote")
            .cloned()
            .ok_or_else(|| invalid_data("quote response omitted quote"))?,
    )
    .map_err(invalid_data)?;
    quote.validate()?;
    let intent = PftlSwapIntentV1 {
        schema: PFTL_SWAP_INTENT_SCHEMA_V1.to_string(),
        chain_id: quote.chain_id.clone(),
        genesis_hash: quote.genesis_hash.clone(),
        protocol_version: quote.protocol_version,
        principal: config.principal,
        controlled_wallet_id: config.controlled_wallet_id,
        route_id: quote.route_id.clone(),
        direction: quote.direction,
        output_mode: quote.output_mode,
        input_reference,
        input_amount_atoms: quote.input_amount_atoms,
        minimum_output_amount_atoms: quote.output_amount_atoms,
        maximum_fee_atoms: quote.maximum_fee_atoms,
        quote_id: quote.quote_id.clone(),
        pricing_nav_epoch: quote.pricing_nav_epoch,
        policy_hash: quote.policy_hash.clone(),
        expiry_height: quote.expiry_height,
        idempotency_key: config.idempotency_key,
    };
    let signed_intent = sign_pftl_swap_intent_with_key_file(&config.key_file, intent)?;
    let mut record = PrivateSwapRecord {
        schema: RECORD_SCHEMA.to_string(),
        state: "signed".to_string(),
        quote,
        signed_intent,
        response: None,
    };
    write_private_record(&config.record_file, &record, false)?;
    submit_and_persist(
        config.address,
        config.request_timeout,
        config.completion_timeout,
        &config.record_file,
        &mut record,
    )
}

fn resume(config: ResumeConfig) -> io::Result<()> {
    let mut record = read_private_record(&config.record_file)?;
    validate_private_record(&record)?;
    if record.state == "committed" {
        print_public_result(&config.record_file, &record)?;
        return Ok(());
    }
    submit_and_persist(
        config.address,
        config.request_timeout,
        config.completion_timeout,
        &config.record_file,
        &mut record,
    )
}

fn submit_and_persist(
    address: SocketAddr,
    request_timeout: Duration,
    completion_timeout: Duration,
    record_file: &Path,
    record: &mut PrivateSwapRecord,
) -> io::Result<()> {
    validate_private_record(record)?;
    let request = json!({"signed_intent": &record.signed_intent});
    let mut response = request_json(address, "POST", "/v1/swap", Some(&request), request_timeout)?;
    require_success(&response, "swap")?;
    if response.status == 202 {
        let deadline = Instant::now() + completion_timeout;
        loop {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "swap remains pending; rerun `resume` with the same private record",
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
            let target = format!(
                "/v1/status?id={}",
                record.signed_intent.intent.idempotency_key
            );
            let status = request_json(address, "GET", &target, None, request_timeout)?;
            require_success(&status, "status")?;
            match swap_state(&status.body)? {
                "COMMITTED" => {
                    response =
                        request_json(address, "POST", "/v1/swap", Some(&request), request_timeout)?;
                    require_success(&response, "committed swap replay")?;
                    break;
                }
                "REJECTED" | "FAILED_PREPUBLISH" => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "swap reached a terminal failure state",
                    ));
                }
                _ => {}
            }
        }
    }
    if response.status != 200 || swap_state(&response.body)? != "COMMITTED" {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "swap did not return a committed result; rerun `resume`",
        ));
    }
    if record.signed_intent.intent.output_mode == PftlSwapOutputMode::Private {
        let references = response
            .body
            .get("output_note_refs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_data("committed private swap omitted output note references"))?;
        if references.len() != 1 || references[0].as_str().is_none() {
            return Err(invalid_data(
                "committed private swap returned an unexpected output note reference count",
            ));
        }
    }
    record.state = "committed".to_string();
    record.response = Some(response.body);
    write_private_record(record_file, record, true)?;
    print_public_result(record_file, record)
}

fn swap_state(body: &Value) -> io::Result<&str> {
    body.get("swap")
        .and_then(|swap| swap.get("state"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_data("swap response omitted state"))
}

fn require_success(response: &HttpResponse, operation: &str) -> io::Result<()> {
    if (200..300).contains(&response.status)
        && response.body.get("ok").and_then(Value::as_bool) == Some(true)
    {
        return Ok(());
    }
    let code = response
        .body
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let message = response
        .body
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("request failed");
    Err(io::Error::other(format!(
        "{operation} returned HTTP {} ({code}): {message}",
        response.status
    )))
}

fn print_public_result(record_file: &Path, record: &PrivateSwapRecord) -> io::Result<()> {
    let response = record
        .response
        .as_ref()
        .ok_or_else(|| invalid_data("committed private record omitted response"))?;
    let public = json!({
        "schema": PUBLIC_RESULT_SCHEMA,
        "committed": true,
        "record_file": record_file,
        "quote": {
            "quote_id": record.quote.quote_id,
            "route_id": record.quote.route_id,
            "direction": record.quote.direction,
            "output_mode": record.quote.output_mode,
            "nav_amount_atoms": record.quote.nav_amount_atoms,
            "input_asset_id": record.quote.input_asset_id,
            "input_amount_atoms": record.quote.input_amount_atoms,
            "output_asset_id": record.quote.output_asset_id,
            "output_amount_atoms": record.quote.output_amount_atoms,
            "pricing_nav_epoch": record.quote.pricing_nav_epoch,
            "pricing_reserve_packet_hash": record.quote.pricing_reserve_packet_hash,
        },
        "swap": response.get("swap").cloned().unwrap_or(Value::Null),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&public).map_err(invalid_data)?
    );
    Ok(())
}

fn resolve_input_reference(source: &InputReference) -> io::Result<String> {
    match source {
        InputReference::Literal(value) => Ok(value.clone()),
        InputReference::PrivateRecord(path) => {
            let record = read_private_record(path)?;
            validate_private_record(&record)?;
            if record.state != "committed" {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "input private record is not committed",
                ));
            }
            let references = record
                .response
                .as_ref()
                .and_then(|response| response.get("output_note_refs"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    invalid_data("input private record omitted output note references")
                })?;
            if references.len() != 1 {
                return Err(invalid_data(
                    "input private record has an unexpected output note reference count",
                ));
            }
            references[0]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_data("input note reference is not a string"))
        }
    }
}

fn validate_private_record(record: &PrivateSwapRecord) -> io::Result<()> {
    if record.schema != RECORD_SCHEMA || !matches!(record.state.as_str(), "signed" | "committed") {
        return Err(invalid_data("private swap record metadata is invalid"));
    }
    record.quote.validate()?;
    record.signed_intent.verify()?;
    if record.signed_intent.intent.quote_id != record.quote.quote_id {
        return Err(invalid_data(
            "private swap record signed intent does not match quote",
        ));
    }
    if (record.state == "committed") != record.response.is_some() {
        return Err(invalid_data(
            "private swap record state does not match response presence",
        ));
    }
    Ok(())
}

fn read_private_record(path: &Path) -> io::Result<PrivateSwapRecord> {
    validate_private_file(path, "private swap record")?;
    let mut file = File::open(path)?;
    if file.metadata()?.len() > MAX_PRIVATE_FILE_BYTES as u64 {
        return Err(invalid_data("private swap record exceeds size bound"));
    }
    let mut bytes = Zeroizing::new(Vec::new());
    Read::by_ref(&mut file)
        .take(MAX_PRIVATE_FILE_BYTES.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PRIVATE_FILE_BYTES {
        return Err(invalid_data("private swap record exceeds size bound"));
    }
    serde_json::from_slice(&bytes).map_err(invalid_data)
}

fn write_private_record(path: &Path, record: &PrivateSwapRecord, replace: bool) -> io::Result<()> {
    validate_private_record(record)?;
    if path.exists() {
        if !replace {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "private swap record already exists",
            ));
        }
        validate_private_file(path, "private swap record")?;
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    set_private_directory_permissions(parent)?;
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(invalid_data)?
        .as_nanos();
    let temporary = parent.join(format!(".pftl-swapctl-{}-{suffix}.tmp", std::process::id()));
    let mut bytes = Zeroizing::new(serde_json::to_vec_pretty(record).map_err(invalid_data)?);
    bytes.push(b'\n');
    if bytes.len() > MAX_PRIVATE_FILE_BYTES {
        return Err(invalid_data("private swap record exceeds size bound"));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    set_private_file_permissions(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()
}

fn validate_private_file(path: &Path, label: &str) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not a regular file"),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} must have mode 600"),
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private record directory must not be accessible by group or other users",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn request_json(
    address: SocketAddr,
    method: &str,
    target: &str,
    value: Option<&Value>,
    timeout: Duration,
) -> io::Result<HttpResponse> {
    if !address.ip().is_loopback()
        || !matches!(method, "GET" | "POST")
        || !target.starts_with('/')
        || target.len() > 512
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HTTP request target or loopback address is invalid",
        ));
    }
    let body = Zeroizing::new(match value {
        Some(value) => serde_json::to_vec(value).map_err(invalid_data)?,
        None => Vec::new(),
    });
    if body.len() > MAX_HTTP_BODY_BYTES || (method == "GET" && !body.is_empty()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HTTP request body exceeds bounds",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    write!(
        stream,
        "{method} {target} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    stream.flush()?;
    read_http_response(stream)
}

fn read_http_response(stream: TcpStream) -> io::Result<HttpResponse> {
    let mut reader = BufReader::new(stream);
    let first = read_bounded_line(&mut reader, 512, "HTTP response line")?;
    let status = first
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| invalid_data("invalid HTTP response status"))?;
    let mut content_length = None;
    for index in 0..=64 {
        let line = read_bounded_line(&mut reader, 8_192, "HTTP response header")?;
        if line == "\r\n" || line == "\n" {
            break;
        }
        if index == 64 {
            return Err(invalid_data("HTTP response has too many headers"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| invalid_data("invalid HTTP response header"))?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(invalid_data(
                "HTTP transfer-encoding responses are not supported",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .map_err(|_| invalid_data("invalid HTTP content length"))?;
            if content_length.is_some_and(|existing| existing != parsed) {
                return Err(invalid_data("conflicting HTTP content lengths"));
            }
            content_length = Some(parsed);
        }
    }
    let length = content_length.ok_or_else(|| invalid_data("HTTP content length is required"))?;
    if length > MAX_HTTP_BODY_BYTES {
        return Err(invalid_data("HTTP response body exceeds size bound"));
    }
    let mut bytes = Zeroizing::new(vec![0_u8; length]);
    reader.read_exact(&mut bytes)?;
    Ok(HttpResponse {
        status,
        body: serde_json::from_slice(&bytes).map_err(invalid_data)?,
    })
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    maximum_bytes: usize,
    label: &str,
) -> io::Result<String> {
    let mut line = String::new();
    Read::by_ref(reader)
        .take(maximum_bytes.saturating_add(1) as u64)
        .read_line(&mut line)?;
    if line.is_empty() || line.len() > maximum_bytes || !line.ends_with('\n') {
        return Err(invalid_data(format!(
            "{label} is empty, unterminated, or exceeds its bound"
        )));
    }
    Ok(line)
}

fn parse_command() -> io::Result<Command> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage_error)?;
    if matches!(command.as_str(), "-h" | "--help" | "help") {
        println!("{}", usage());
        std::process::exit(0);
    }
    let values = parse_flag_values(args)?;
    match command.as_str() {
        "execute" => parse_execute(values).map(Command::Execute),
        "resume" => parse_resume(values).map(Command::Resume),
        _ => Err(usage_error()),
    }
}

fn parse_flag_values(
    mut args: impl Iterator<Item = String>,
) -> io::Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    while let Some(flag) = args.next() {
        if !flag.starts_with("--") {
            return Err(usage_error());
        }
        let value = args.next().ok_or_else(usage_error)?;
        if value.starts_with("--") || value.len() > 4_096 || values.insert(flag, value).is_some() {
            return Err(usage_error());
        }
    }
    Ok(values)
}

fn parse_execute(mut values: BTreeMap<String, String>) -> io::Result<ExecuteConfig> {
    let address = loopback_address(take_required(&mut values, "--address")?)?;
    let key_file = PathBuf::from(take_required(&mut values, "--key-file")?);
    let principal = take_required(&mut values, "--principal")?;
    let controlled_wallet_id = take_required(&mut values, "--controlled-wallet-id")?;
    let direction = parse_direction(&take_required(&mut values, "--direction")?)?;
    let output_mode = parse_output_mode(&take_required(&mut values, "--output-mode")?)?;
    let nav_amount_atoms = parse_positive_u64(
        &take_required(&mut values, "--nav-amount-atoms")?,
        "--nav-amount-atoms",
        u64::MAX,
    )?;
    let idempotency_key = take_required(&mut values, "--idempotency-key")?;
    let record_file = PathBuf::from(take_required(&mut values, "--record-file")?);
    let literal = values.remove("--input-reference");
    let record = values.remove("--input-reference-record").map(PathBuf::from);
    let input_reference = match (literal, record) {
        (Some(value), None) => InputReference::Literal(value),
        (None, Some(path)) => InputReference::PrivateRecord(path),
        _ => return Err(usage_error()),
    };
    let request_timeout = parse_duration(&mut values, "--request-timeout-ms", 180_000)?;
    let completion_timeout = parse_duration(&mut values, "--completion-timeout-ms", 300_000)?;
    reject_unknown(values)?;
    Ok(ExecuteConfig {
        address,
        key_file,
        principal,
        controlled_wallet_id,
        direction,
        output_mode,
        nav_amount_atoms,
        input_reference,
        idempotency_key,
        record_file,
        request_timeout,
        completion_timeout,
    })
}

fn parse_resume(mut values: BTreeMap<String, String>) -> io::Result<ResumeConfig> {
    let address = loopback_address(take_required(&mut values, "--address")?)?;
    let record_file = PathBuf::from(take_required(&mut values, "--record-file")?);
    let request_timeout = parse_duration(&mut values, "--request-timeout-ms", 180_000)?;
    let completion_timeout = parse_duration(&mut values, "--completion-timeout-ms", 300_000)?;
    reject_unknown(values)?;
    Ok(ResumeConfig {
        address,
        record_file,
        request_timeout,
        completion_timeout,
    })
}

fn take_required(values: &mut BTreeMap<String, String>, flag: &str) -> io::Result<String> {
    values.remove(flag).ok_or_else(usage_error)
}

fn parse_duration(
    values: &mut BTreeMap<String, String>,
    flag: &str,
    default_ms: u64,
) -> io::Result<Duration> {
    let value = values
        .remove(flag)
        .map(|value| parse_positive_u64(&value, flag, 600_000))
        .transpose()?
        .unwrap_or(default_ms);
    Ok(Duration::from_millis(value))
}

fn parse_positive_u64(value: &str, flag: &str, maximum: u64) -> io::Result<u64> {
    let parsed = value.parse::<u64>().map_err(|_| usage_error())?;
    if parsed == 0 || parsed > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{flag} is outside its allowed range"),
        ));
    }
    Ok(parsed)
}

fn parse_direction(value: &str) -> io::Result<PftlSwapDirection> {
    match value {
        "issue" => Ok(PftlSwapDirection::Issue),
        "redeem" => Ok(PftlSwapDirection::Redeem),
        _ => Err(usage_error()),
    }
}

fn parse_output_mode(value: &str) -> io::Result<PftlSwapOutputMode> {
    match value {
        "private" => Ok(PftlSwapOutputMode::Private),
        "transparent" => Ok(PftlSwapOutputMode::Transparent),
        _ => Err(usage_error()),
    }
}

fn loopback_address(value: String) -> io::Result<SocketAddr> {
    let address = value.parse::<SocketAddr>().map_err(invalid_data)?;
    if !address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "--address must be loopback",
        ));
    }
    Ok(address)
}

fn reject_unknown(values: BTreeMap<String, String>) -> io::Result<()> {
    if values.is_empty() {
        Ok(())
    } else {
        Err(usage_error())
    }
}

fn usage_error() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, usage())
}

fn usage() -> &'static str {
    "usage:
  pftl_swapctl execute --address 127.0.0.1:8798 --key-file PATH --principal PF_ADDRESS --controlled-wallet-id ID --direction issue|redeem --output-mode private|transparent --nav-amount-atoms N (--input-reference REF | --input-reference-record PATH) --idempotency-key ID --record-file PATH [--request-timeout-ms N] [--completion-timeout-ms N]
  pftl_swapctl resume --address 127.0.0.1:8798 --record-file PATH [--request-timeout-ms N] [--completion-timeout-ms N]"
}

fn invalid_data(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
