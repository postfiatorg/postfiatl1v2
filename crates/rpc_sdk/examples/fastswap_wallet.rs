use postfiat_rpc_sdk::{
    drive_fastswap_three_wave, preview_fastswap, reconcile_fastswap_replication,
    swap_settlement_mode, FastSwapWalletSessionV1, SwapSettlementModeV1, TcpFastSwapTransportV1,
};
use postfiat_types::{FastSwapCommitteeV1, SignedFastSwapIntentV1};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

struct SessionLock {
    path: PathBuf,
}

impl SessionLock {
    fn acquire(session_path: &Path) -> io::Result<Self> {
        let mut name = session_path.as_os_str().to_os_string();
        name.push(".lock");
        let path = PathBuf::from(name);
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        writeln!(file, "{}", std::process::id())?;
        file.sync_all()?;
        Ok(Self { path })
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return Ok(());
    }
    let committee_path = required_value(&args, "--committee")?;
    let settlement_mode = swap_settlement_mode(optional_value(&args, "--settlement"))?;
    if settlement_mode != SwapSettlementModeV1::FastSwapV1 {
        return Err(
            "this FastSwap command requires explicit `--settlement fastswap_v1`; consensus_w6 remains the default"
                .into(),
        );
    }
    let session_path = PathBuf::from(required_value(&args, "--session")?);
    let _session_lock = SessionLock::acquire(&session_path).map_err(|error| {
        format!(
            "cannot exclusively lock wallet session {}: {error}",
            session_path.display()
        )
    })?;
    let timeout_ms = optional_value(&args, "--timeout-ms")
        .unwrap_or("10000")
        .parse::<u64>()?;
    let endpoints = validator_endpoints(&args)?;
    let committee = serde_json::from_slice::<FastSwapCommitteeV1>(&fs::read(committee_path)?)?;
    committee
        .validate()
        .map_err(|error| format!("invalid committee: {error:?}"))?;
    let expected_ids = committee
        .validators
        .iter()
        .map(|validator| validator.validator_id.clone())
        .collect::<BTreeSet<_>>();
    if endpoints.keys().cloned().collect::<BTreeSet<_>>() != expected_ids {
        return Err("--validator endpoints must exactly match the committee roster".into());
    }
    let transport = TcpFastSwapTransportV1::new(endpoints, Duration::from_millis(timeout_ms))
        .map_err(|error| format!("invalid FastSwap transport: {error}"))?;
    let mut session = if session_path.exists() {
        let session = FastSwapWalletSessionV1::from_durable_json(&fs::read(&session_path)?)
            .map_err(|error| format!("invalid wallet session: {error:?}"))?;
        if session.settlement_mode != settlement_mode {
            return Err("durable wallet session settlement mode mismatch".into());
        }
        session
    } else {
        let signed_path = required_value(&args, "--signed-intent")?;
        let signed = serde_json::from_slice::<SignedFastSwapIntentV1>(&fs::read(signed_path)?)
            .map_err(|error| format!("invalid signed FastSwap intent: {error}"))?;
        let effects = preview_fastswap(&signed, &committee, &transport)
            .map_err(|error| format!("FastSwap quorum preview failed: {error:?}"))?;
        let session = FastSwapWalletSessionV1::new(settlement_mode, signed, effects)
            .map_err(|error| format!("FastSwap session creation failed: {error:?}"))?;
        atomic_write(
            &session_path,
            &session
                .to_durable_json()
                .map_err(|error| format!("session encoding failed: {error:?}"))?,
        )?;
        session
    };
    let terminal = drive_fastswap_three_wave(&mut session, &committee, &transport, |current| {
        let bytes = current
            .to_durable_json()
            .map_err(|error| format!("session encoding failed: {error:?}"))?;
        atomic_write(&session_path, &bytes).map_err(|error| error.to_string())
    })
    .map_err(|error| format!("FastSwap settlement failed: {error:?}"))?;
    let replication =
        reconcile_fastswap_replication(&mut session, &committee, &transport, |current| {
            let bytes = current
                .to_durable_json()
                .map_err(|error| format!("session encoding failed: {error:?}"))?;
            atomic_write(&session_path, &bytes).map_err(|error| error.to_string())
        })
        .map_err(|error| format!("FastSwap replication reconciliation failed: {error:?}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat-fastswap-wallet-result-v1",
            "state": session.state,
            "swap_id": hex(&terminal.effects.swap_id.0),
            "receipt": terminal.effects.receipt,
            "lock_signers": terminal.lock_qc.votes.len(),
            "decision_signers": terminal.decision_qc.votes.len(),
            "effects_signers": terminal.effects_qc.votes.len(),
            "replication": replication,
            "timings_ms": session.last_timings,
            "session_file": session_path,
        }))?
    );
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid session path"))?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn validator_endpoints(args: &[String]) -> Result<BTreeMap<String, String>, Box<dyn Error>> {
    let mut endpoints = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--validator" {
            let value = args
                .get(index + 1)
                .ok_or("--validator requires ID=HOST:PORT")?;
            let (id, endpoint) = value
                .split_once('=')
                .ok_or("--validator requires ID=HOST:PORT")?;
            if id.is_empty()
                || endpoint.is_empty()
                || endpoints
                    .insert(id.to_owned(), endpoint.to_owned())
                    .is_some()
            {
                return Err(format!("invalid or duplicate validator endpoint `{value}`").into());
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(endpoints)
}

fn required_value<'a>(args: &'a [String], flag: &str) -> Result<&'a str, Box<dyn Error>> {
    optional_value(args, flag).ok_or_else(|| format!("missing {flag}").into())
}

fn optional_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn print_usage() {
    eprintln!(concat!(
        "usage: cargo run -p postfiat-rpc-sdk --example fastswap_wallet -- ",
        "--settlement fastswap_v1 ",
        "--committee committee.json --session wallet-session.json ",
        "[--signed-intent dual-signed-intent.json] ",
        "--validator validator-0=HOST:PORT ",
        "[--validator validator-N=HOST:PORT ...] [--timeout-ms 10000]"
    ));
}
