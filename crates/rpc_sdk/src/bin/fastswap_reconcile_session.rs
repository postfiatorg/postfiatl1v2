use postfiat_rpc_sdk::{
    reconcile_fastswap_replication, FastSwapWalletSessionV1, TcpFastSwapTransportV1,
};
use postfiat_types::FastSwapCommitteeV1;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes).map_err(|error| format!("{}: {error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("{} -> {}: {error}", temporary.display(), path.display()))
}

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let committee_path = PathBuf::from(flag(&args, "--committee")?);
    let endpoints_path = PathBuf::from(flag(&args, "--endpoints")?);
    let session_path = PathBuf::from(flag(&args, "--session")?);
    let output_path = PathBuf::from(flag(&args, "--output")?);
    let expected_pending = BTreeSet::from([flag(&args, "--expect-pending")?]);
    if output_path.exists() {
        return Err("output already exists; refusing an ambiguous reconciliation".to_owned());
    }

    let committee: FastSwapCommitteeV1 = read_json(&committee_path)?;
    committee
        .validate()
        .map_err(|error| format!("invalid committee: {error:?}"))?;
    let endpoints: BTreeMap<String, String> = read_json(&endpoints_path)?;
    let mut session: FastSwapWalletSessionV1 = read_json(&session_path)?;
    if session.replication_pending != expected_pending {
        return Err(format!(
            "pending set mismatch: expected {expected_pending:?}, found {:?}",
            session.replication_pending
        ));
    }
    let transport = TcpFastSwapTransportV1::new(endpoints, Duration::from_secs(30))?;
    let pending_before = session.replication_pending.clone();
    let report = reconcile_fastswap_replication(&mut session, &committee, &transport, |current| {
        write_json_atomic(&session_path, current)
    })
    .map_err(|error| format!("FastSwap replication failed: {error:?}"))?;
    if !report.failed.is_empty()
        || !report.pending.is_empty()
        || !session.replication_pending.is_empty()
    {
        return Err(format!("FastSwap replication incomplete: {report:?}"));
    }
    let output = serde_json::json!({
        "schema": "postfiat-fastswap-session-reconciliation-v1",
        "pending_before": pending_before,
        "replication": report,
        "session": session,
    });
    write_json_atomic(&output_path, &output)?;
    println!("FastSwap replication reconciled: pending=0 failed=0");
    Ok(())
}
