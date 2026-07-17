use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use postfiat_rpc_sdk::{
    decode_mempool_submit_signed_transfer_summary, decode_transfer_fee_quote_summary,
    decode_tx_finality_summary, mempool_submit_signed_transfer_json_request, response_from_json,
    transfer_fee_quote_request, tx_finality_request_from_submit, wallet_identity_from_backup,
    wallet_sign_transfer_from_quote, MempoolSubmitSummary, RpcRequest, RpcResponse,
    TxFinalitySummary, WalletBackupFile, MAX_RPC_REQUEST_BYTES,
};
use serde_json::json;

const MAX_EXAMPLE_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
struct Config {
    quote_addr: String,
    submit_addr: String,
    tx_addr: String,
    backup_file: String,
    recipient: String,
    amount: u64,
    timeout_ms: u64,
    finality_attempts: u32,
    finality_delay_ms: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args(env::args().skip(1).collect())?;
    let timeout = Duration::from_millis(config.timeout_ms);
    let backup = read_wallet_backup_file(&config.backup_file)?;
    let identity = wallet_identity_from_backup(&backup)?;

    let quote_request = transfer_fee_quote_request(
        "sdk-tcp-wallet-quote",
        identity.address.clone(),
        config.recipient.clone(),
        config.amount,
        None,
    );
    let quote_response = send_rpc_request(&config.quote_addr, &quote_request, timeout)?;
    let quote = decode_transfer_fee_quote_summary(&quote_response)?;

    let signed = wallet_sign_transfer_from_quote(&backup, &quote)?;
    let signed_json = serde_json::to_string(&signed)?;
    let submit_request =
        mempool_submit_signed_transfer_json_request("sdk-tcp-wallet-submit", signed_json);
    let submit_response = send_rpc_request(&config.submit_addr, &submit_request, timeout)?;
    let submit = decode_mempool_submit_signed_transfer_summary(&submit_response)?;

    let finality = poll_tx_finality(
        &config.tx_addr,
        &submit,
        timeout,
        config.finality_attempts,
        Duration::from_millis(config.finality_delay_ms),
    )?;

    let summary = json!({
        "schema": "postfiat-rpc-sdk-tcp-wallet-flow-example-v1",
        "from": identity.address,
        "to": config.recipient,
        "amount": config.amount,
        "fee": quote.minimum_fee,
        "tx_id": submit.tx_id,
        "confirmed": finality.accepted,
        "proof_id": finality.proof_id,
        "block_height": finality.block_height,
        "certificate_id": finality.certificate_id
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn poll_tx_finality(
    tx_addr: &str,
    submit: &MempoolSubmitSummary,
    timeout: Duration,
    attempts: u32,
    delay: Duration,
) -> Result<TxFinalitySummary, Box<dyn Error>> {
    let mut last_error = None;
    for attempt in 1..=attempts {
        let tx_request =
            tx_finality_request_from_submit(format!("sdk-tcp-wallet-tx-{attempt}"), submit);
        match send_rpc_request(tx_addr, &tx_request, timeout)
            .and_then(|response| decode_tx_finality_summary(&response).map_err(Into::into))
        {
            Ok(finality) => return Ok(finality),
            Err(error) => {
                last_error = Some(error.to_string());
                if attempt < attempts {
                    thread::sleep(delay);
                }
            }
        }
    }
    Err(format!(
        "tx finality unavailable after {attempts} attempts for {}; last error: {}",
        submit.tx_id,
        last_error.unwrap_or_else(|| "none".to_string())
    )
    .into())
}

fn send_rpc_request(
    addr: &str,
    request: &RpcRequest,
    timeout: Duration,
) -> Result<RpcResponse, Box<dyn Error>> {
    let mut request_json = serde_json::to_string(request)?;
    if request_json.len() > MAX_RPC_REQUEST_BYTES {
        return Err(format!("rpc request exceeds {MAX_RPC_REQUEST_BYTES} bytes").into());
    }
    request_json.push('\n');

    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(request_json.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let response_json = read_bounded_line(&mut reader, MAX_EXAMPLE_RESPONSE_BYTES)?;
    let response = response_from_json(&response_json)?;
    response.validate_protocol()?;
    Ok(response)
}

fn read_bounded_line<R: BufRead>(reader: &mut R, max_bytes: usize) -> io::Result<String> {
    let mut buf = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if buf.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "rpc server closed before sending a response",
                ));
            }
            break;
        }
        let consumed = if let Some(newline_index) = available.iter().position(|byte| *byte == b'\n')
        {
            let end = newline_index + 1;
            if buf.len().saturating_add(end) > max_bytes {
                return Err(response_too_large(max_bytes));
            }
            buf.extend_from_slice(&available[..end]);
            reader.consume(end);
            break;
        } else {
            if buf.len().saturating_add(available.len()) > max_bytes {
                return Err(response_too_large(max_bytes));
            }
            let len = available.len();
            buf.extend_from_slice(available);
            len
        };
        reader.consume(consumed);
    }
    String::from_utf8(buf).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn response_too_large(max_bytes: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("rpc response exceeded {max_bytes} bytes"),
    )
}

fn read_wallet_backup_file(path: &str) -> Result<WalletBackupFile, Box<dyn Error>> {
    let raw = fs::read_to_string(path)?;
    let backup = serde_json::from_str::<WalletBackupFile>(&raw)?;
    wallet_identity_from_backup(&backup)?;
    Ok(backup)
}

fn parse_args(args: Vec<String>) -> Result<Config, Box<dyn Error>> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        std::process::exit(0);
    }
    let quote_addr = required_value(&args, "--quote-addr")?;
    let submit_addr = required_value(&args, "--submit-addr")?;
    let tx_addr = required_value(&args, "--tx-addr")?;
    let backup_file = required_value(&args, "--backup-file")?;
    let recipient = required_value(&args, "--to")?;
    let amount = required_value(&args, "--amount")?
        .parse::<u64>()
        .map_err(|error| format!("invalid --amount: {error}"))?;
    if amount == 0 {
        return Err("--amount must be greater than zero".into());
    }
    let timeout_ms = optional_value(&args, "--timeout-ms")
        .unwrap_or_else(|| "5000".to_string())
        .parse::<u64>()
        .map_err(|error| format!("invalid --timeout-ms: {error}"))?;
    if timeout_ms == 0 {
        return Err("--timeout-ms must be greater than zero".into());
    }
    let finality_attempts = optional_value(&args, "--finality-attempts")
        .unwrap_or_else(|| "12".to_string())
        .parse::<u32>()
        .map_err(|error| format!("invalid --finality-attempts: {error}"))?;
    if finality_attempts == 0 {
        return Err("--finality-attempts must be greater than zero".into());
    }
    let finality_delay_ms = optional_value(&args, "--finality-delay-ms")
        .unwrap_or_else(|| "1000".to_string())
        .parse::<u64>()
        .map_err(|error| format!("invalid --finality-delay-ms: {error}"))?;
    Ok(Config {
        quote_addr,
        submit_addr,
        tx_addr,
        backup_file,
        recipient,
        amount,
        timeout_ms,
        finality_attempts,
        finality_delay_ms,
    })
}

fn required_value(args: &[String], flag: &str) -> Result<String, Box<dyn Error>> {
    optional_value(args, flag).ok_or_else(|| format!("missing {flag}").into())
}

fn optional_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p postfiat-rpc-sdk --example tcp_wallet_flow -- \\
  --quote-addr HOST:PORT --submit-addr HOST:PORT --tx-addr HOST:PORT \\
  --backup-file wallet.backup.json --to ADDRESS --amount AMOUNT \\
  [--timeout-ms MS] [--finality-attempts N] [--finality-delay-ms MS]"
    );
}
