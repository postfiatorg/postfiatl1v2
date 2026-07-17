use postfiat_rpc_sdk::{
    drive_fastswap_three_wave, preview_fastswap, reconcile_fastswap_replication,
    FastSwapWalletSessionV1, SwapSettlementModeV1, TcpFastSwapTransportV1,
};
use postfiat_types::{FastSwapCommitteeV1, SignedFastSwapIntentV1};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn flag(args: &[String], name: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|value| value == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn persist(path: &Path, session: &FastSwapWalletSessionV1) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(session).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let committee_path = PathBuf::from(flag(&args, "--committee")?);
    let endpoints_path = PathBuf::from(flag(&args, "--endpoints")?);
    let signed_path = PathBuf::from(flag(&args, "--signed-intent")?);
    let session_path = PathBuf::from(flag(&args, "--session")?);
    let output_path = PathBuf::from(flag(&args, "--output")?);
    if session_path.exists() || output_path.exists() {
        return Err("session/output already exists; refusing a second live drive".to_owned());
    }
    let committee: FastSwapCommitteeV1 = read_json(&committee_path)?;
    committee
        .validate()
        .map_err(|error| format!("invalid committee: {error:?}"))?;
    let endpoints: BTreeMap<String, String> = read_json(&endpoints_path)?;
    let signed: SignedFastSwapIntentV1 = read_json(&signed_path)?;
    let transport = TcpFastSwapTransportV1::new(endpoints, Duration::from_secs(30))?;

    let preview_started = Instant::now();
    let expected_effects = preview_fastswap(&signed, &committee, &transport)
        .map_err(|error| format!("FastSwap preview failed: {error:?}"))?;
    let preview_ms = preview_started.elapsed().as_millis() as u64;
    let mut session =
        FastSwapWalletSessionV1::new(SwapSettlementModeV1::FastSwapV1, signed, expected_effects)
            .map_err(|error| format!("FastSwap session failed: {error:?}"))?;
    persist(&session_path, &session)?;

    let settlement_started = Instant::now();
    let _terminal = drive_fastswap_three_wave(&mut session, &committee, &transport, |current| {
        persist(&session_path, current)
    })
    .map_err(|error| format!("FastSwap drive failed: {error:?}"))?;
    let settlement_ms = settlement_started.elapsed().as_millis() as u64;
    let replication =
        reconcile_fastswap_replication(&mut session, &committee, &transport, |current| {
            persist(&session_path, current)
        })
        .map_err(|error| format!("FastSwap replication failed: {error:?}"))?;
    if !replication.failed.is_empty() || !replication.pending.is_empty() {
        return Err(format!("FastSwap replication incomplete: {replication:?}"));
    }
    let report = serde_json::json!({
        "schema": "postfiat-fastswap-live-driver-v1",
        "preview_ms": preview_ms,
        "settlement_ms": settlement_ms,
        "terminal_verified": true,
        "replication": replication,
        "session": session,
    });
    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("{}: {error}", output_path.display()))?;
    println!(
        "FastSwap accepted: preview={}ms settlement={}ms replication_complete=true",
        preview_ms, settlement_ms
    );
    Ok(())
}
