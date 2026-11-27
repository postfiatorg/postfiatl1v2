use std::collections::VecDeque;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use postfiat_rpc_sdk::{RpcRequest, RpcResponse};
use serde::{Deserialize, Serialize};

use crate::release_upgrade::{
    directory_data_size_bytes, fetch_https_bounded, filesystem_free_bytes,
    verify_release_manifest_v3, ReleaseKeysetV1, ReleaseManifestV3, MAX_RELEASE_MANIFEST_BYTES,
    UPGRADE_HEADROOM_BYTES,
};
use crate::unix_now;
use crate::validator_uptime::{
    verify_validator_uptime_feed, ObserverKeysetV1, ValidatorUptimeFeedV1,
};

pub const TOP_SNAPSHOT_SCHEMA: &str = "postfiat-node-top-snapshot-v1";
pub const DEFAULT_TOP_RPC_HOST: &str = "127.0.0.1";
pub const DEFAULT_TOP_RPC_PORT: u16 = 27650;
pub const DEFAULT_TOP_REFRESH_MS: u64 = 2_000;
pub const MIN_TOP_REFRESH_MS: u64 = 500;
pub const MAX_TOP_REFRESH_MS: u64 = 60_000;
pub const DEFAULT_OBSERVER_KEYSET_FILE: &str = "/etc/postfiat/observer-keyset.json";
pub const DEFAULT_TOP_RELEASE_KEYSET_FILE: &str = "/etc/postfiat/release-keyset.json";
pub const MAX_UPTIME_FEED_BYTES: usize = 8 * 1024 * 1024;
const LATENCY_WINDOW_SIZE: usize = 64;

#[derive(Debug, Clone)]
pub struct ValidatorTopOptions {
    pub rpc_host: String,
    pub rpc_port: u16,
    pub allow_remote_rpc: bool,
    pub data_dir: Option<PathBuf>,
    pub uptime_feed_url: Option<String>,
    pub uptime_feed_file: Option<PathBuf>,
    pub observer_keyset_file: PathBuf,
    pub release_manifest_url: Option<String>,
    pub release_manifest_file: Option<PathBuf>,
    pub release_keyset_file: PathBuf,
    pub refresh_ms: u64,
    pub once: bool,
    pub json: bool,
    pub no_color: bool,
    pub rpc_timeout_ms: u64,
    pub now_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopWarning {
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopNodeBanner {
    pub node_id: String,
    pub role: String,
    pub chain_id: String,
    pub git_revision: Option<String>,
    pub status: String,
    pub upgrade_alert: String,
    pub latest_release_id: Option<String>,
    pub activation_height: Option<u64>,
    pub blocks_until_activation: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopHeightPanel {
    pub finalized_height: u64,
    pub view: Option<u64>,
    pub block_tip_prefix: String,
    pub state_root_prefix: String,
    pub committee_epoch: Option<u64>,
    pub height_lag: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopPeersVotesPanel {
    pub configured_peers: usize,
    pub connected_peers: Option<usize>,
    pub quorum_size: usize,
    pub latest_height_day_bps: Option<u32>,
    pub mean_30d_participation_bps: Option<u32>,
    pub last_voted_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopSignerPanel {
    pub signer_mode: String,
    pub signer_reachable: Option<bool>,
    pub signer_double_sign_protection: Option<bool>,
    pub signer_key_matches_registry: Option<bool>,
    pub separate_uid_enforced: Option<bool>,
    pub custody_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopDiskLatencyPanel {
    pub data_dir_bytes: Option<u64>,
    pub fs_free_bytes: Option<u64>,
    pub upgrade_headroom_ok: Option<bool>,
    pub rpc_read_p95_ms: f64,
    pub last_status_ms: f64,
    pub rss_mib: Option<f64>,
    pub history_retention_mode: Option<String>,
    pub mempool_pending: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopRewardEligibilityPanel {
    pub access_state: String,
    pub access_state_reason: Option<String>,
    pub observer_quorum: String,
    pub feed_age_seconds: Option<u64>,
    pub observed_height_days: Option<u32>,
    pub mean_daily_participation_bps: Option<u32>,
    pub grace_remaining_blocks: Option<u64>,
    pub recovery_streak_days: Option<u32>,
    pub finance_hub_access: Option<bool>,
    pub reward_eligible: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatorTopSnapshotV1 {
    pub schema: String,
    pub captured_at_unix: u64,
    pub rpc_endpoint: String,
    pub node: TopNodeBanner,
    pub height: TopHeightPanel,
    pub peers_votes: TopPeersVotesPanel,
    pub signer: TopSignerPanel,
    pub disk_latency: TopDiskLatencyPanel,
    pub reward_eligibility: TopRewardEligibilityPanel,
    pub warnings: Vec<TopWarning>,
}

pub fn validate_top_options(options: &ValidatorTopOptions) -> Result<(), String> {
    if !(MIN_TOP_REFRESH_MS..=MAX_TOP_REFRESH_MS).contains(&options.refresh_ms) {
        return Err(format!(
            "--refresh-ms must be between {MIN_TOP_REFRESH_MS} and {MAX_TOP_REFRESH_MS}"
        ));
    }
    if options.uptime_feed_url.is_some() && options.uptime_feed_file.is_some() {
        return Err("--uptime-feed-url and --uptime-feed-file are mutually exclusive".to_string());
    }
    if options.release_manifest_url.is_some() && options.release_manifest_file.is_some() {
        return Err(
            "--release-manifest-url and --release-manifest-file are mutually exclusive".to_string(),
        );
    }
    if !options.allow_remote_rpc && !is_loopback_host(&options.rpc_host) {
        return Err(format!(
            "refusing non-loopback --rpc-host `{}` without explicit --allow-remote-rpc",
            options.rpc_host
        ));
    }
    Ok(())
}

pub fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let trimmed = host.trim_start_matches('[').trim_end_matches(']');
    trimmed.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn query_rpc_timed(
    host: &str,
    port: u16,
    method: &str,
    timeout_ms: u64,
) -> Result<(serde_json::Value, f64), String> {
    let started = Instant::now();
    let addr_text = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let address = addr_text
        .to_socket_addrs()
        .map_err(|error| format!("resolve `{addr_text}` failed: {error}"))?
        .next()
        .ok_or_else(|| format!("`{addr_text}` resolved to no address"))?;
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("connect `{addr_text}` failed: {error}"))?;
    let req_id = format!("top-{method}");
    let request = RpcRequest::empty(&req_id, method);
    let mut wire = serde_json::to_vec(&request)
        .map_err(|error| format!("encode `{method}` failed: {error}"))?;
    wire.push(b'\n');
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("write timeout failed: {error}"))?;
    stream
        .write_all(&wire)
        .map_err(|error| format!("write `{method}` failed: {error}"))?;
    let _ = stream.shutdown(std::net::Shutdown::Write);

    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(format!("RPC `{method}` timed out after {timeout_ms}ms"));
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|error| format!("read timeout failed: {error}"))?;
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let nl = chunk[..n].iter().position(|b| *b == b'\n');
                raw.extend_from_slice(&chunk[..nl.unwrap_or(n)]);
                if raw.len() > 4 * 1024 * 1024 {
                    return Err(format!("RPC `{method}` response exceeded 4 MiB"));
                }
                if nl.is_some() {
                    break;
                }
            }
            Err(error) => return Err(format!("read `{method}` failed: {error}")),
        }
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let response: RpcResponse = serde_json::from_slice(&raw)
        .map_err(|error| format!("parse `{method}` failed: {error}"))?;
    response
        .validate_protocol()
        .map_err(|error| format!("protocol `{method}` failed: {error}"))?;
    if !response.ok {
        let code = response
            .error
            .as_ref()
            .map_or("rpc_error", |e| e.code.as_str());
        return Err(format!("RPC `{method}` returned ok=false ({code})"));
    }
    let result = response
        .result
        .ok_or_else(|| format!("RPC `{method}` missing result"))?;
    Ok((result, elapsed_ms))
}

fn read_process_rss_mib() -> Option<f64> {
    let raw = fs::read_to_string("/proc/self/status").ok()?;
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: f64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some((kb / 1024.0 * 10.0).round() / 10.0);
        }
    }
    None
}

fn p95_latency_ms(samples: &VecDeque<f64>) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = samples.iter().copied().collect();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let idx = ((sorted.len() * 95).saturating_sub(1)) / 100;
    let val = sorted[idx.min(sorted.len() - 1)];
    (val * 100.0).round() / 100.0
}

pub fn collect_top_snapshot(
    options: &ValidatorTopOptions,
    latency_samples: &mut VecDeque<f64>,
) -> Result<ValidatorTopSnapshotV1, String> {
    validate_top_options(options)?;
    let now_unix = options.now_unix.unwrap_or_else(unix_now);

    let (status_val, status_ms) = query_rpc_timed(
        &options.rpc_host,
        options.rpc_port,
        "status",
        options.rpc_timeout_ms,
    )?;
    latency_samples.push_back(status_ms);
    while latency_samples.len() > LATENCY_WINDOW_SIZE {
        latency_samples.pop_front();
    }

    let server_info_val = query_rpc_timed(
        &options.rpc_host,
        options.rpc_port,
        "server_info",
        options.rpc_timeout_ms,
    )
    .ok()
    .map(|(v, ms)| {
        latency_samples.push_back(ms);
        while latency_samples.len() > LATENCY_WINDOW_SIZE {
            latency_samples.pop_front();
        }
        v
    });
    let metrics_val = query_rpc_timed(
        &options.rpc_host,
        options.rpc_port,
        "metrics",
        options.rpc_timeout_ms,
    )
    .ok()
    .map(|(v, _)| v);
    let readiness_val = query_rpc_timed(
        &options.rpc_host,
        options.rpc_port,
        "private_readiness",
        options.rpc_timeout_ms,
    )
    .ok()
    .map(|(v, _)| v);
    let validators_val = query_rpc_timed(
        &options.rpc_host,
        options.rpc_port,
        "validators",
        options.rpc_timeout_ms,
    )
    .ok()
    .map(|(v, _)| v);

    let mut warnings = Vec::new();

    let node_id = status_val
        .get("node_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let chain_id = status_val
        .get("chain_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let status_text = status_val
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let git_revision = status_val
        .get("build_git_revision")
        .or_else(|| status_val.get("git_revision"))
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let finalized_height = status_val
        .get("block_height")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let block_tip_prefix: String = status_val
        .get("block_tip_hash")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .chars()
        .take(12)
        .collect();
    let state_root_prefix: String = status_val
        .get("state_root")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .chars()
        .take(12)
        .collect();
    let mempool_pending = status_val
        .get("mempool_pending")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    let role = status_val
        .get("node_role")
        .or_else(|| server_info_val.as_ref().and_then(|s| s.get("node_role")))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("validator")
        .to_string();

    let view = metrics_val
        .as_ref()
        .and_then(|m| m.get("consensus"))
        .and_then(|c| c.get("current_view").or_else(|| c.get("view")))
        .and_then(serde_json::Value::as_u64);
    let committee_epoch = metrics_val
        .as_ref()
        .and_then(|m| m.get("consensus"))
        .and_then(|c| {
            c.get("committee_epoch")
                .or_else(|| c.get("validator_registry_update_count"))
        })
        .and_then(serde_json::Value::as_u64);

    let configured_peers = validators_val
        .as_ref()
        .and_then(|v| v.get("validator_count"))
        .and_then(serde_json::Value::as_u64)
        .or_else(|| {
            status_val
                .get("validator_count")
                .and_then(serde_json::Value::as_u64)
        })
        .unwrap_or(0) as usize;
    let connected_peers = metrics_val
        .as_ref()
        .and_then(|m| m.get("rpc"))
        .and_then(|r| r.get("active_connections"))
        .and_then(serde_json::Value::as_u64)
        .map(|n| n as usize);
    let quorum_size = if configured_peers == 0 {
        0
    } else {
        (configured_peers * 2) / 3 + 1
    };

    // Panel 4: Signer Guard Health from `private_readiness`.
    let custody = readiness_val.as_ref().and_then(|r| r.get("custody"));
    let signer_mode = custody
        .and_then(|c| c.get("validator_signer"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("none")
        .to_string();
    let signer_double_sign_protection = custody
        .and_then(|c| c.get("signer_double_sign_protection"))
        .and_then(serde_json::Value::as_bool);
    let signer_key_matches_registry = custody
        .and_then(|c| c.get("signer_key_matches"))
        .and_then(serde_json::Value::as_bool);
    let serving_uid = custody
        .and_then(|c| c.get("serving_uid"))
        .and_then(serde_json::Value::as_u64);
    let signer_uid = custody
        .and_then(|c| c.get("signer_uid"))
        .and_then(serde_json::Value::as_u64);
    let separate_uid_enforced = match (serving_uid, signer_uid) {
        (Some(s), Some(k)) => Some(s != k),
        _ => None,
    };
    let custody_reasons: Vec<String> = readiness_val
        .as_ref()
        .and_then(|r| r.get("reasons"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let signer_reachable = if signer_mode == "socket" {
        Some(
            !custody_reasons
                .iter()
                .any(|r| r == "validator_signer_unreachable"),
        )
    } else {
        None
    };
    if custody_reasons
        .iter()
        .any(|r| r == "validator_signer_same_user")
    {
        warnings.push(TopWarning {
            code: "SIGNER_SAME_USER".to_string(),
            severity: "warn".to_string(),
            message: "Validator signer socket shares UID with rpc-serve process".to_string(),
        });
    }

    // Panel 5: Read-only disk & memory inspection (never opens redb).
    let (data_dir_bytes, fs_free_bytes, upgrade_headroom_ok) = if let Some(dir) = &options.data_dir
    {
        let data = directory_data_size_bytes(dir).ok();
        let free = filesystem_free_bytes(dir).ok();
        let ok = match (data, free) {
            (Some(d), Some(f)) => {
                let ok = f >= d.saturating_mul(2).saturating_add(UPGRADE_HEADROOM_BYTES);
                if !ok {
                    warnings.push(TopWarning {
                        code: "LOW_DISK_FOR_UPGRADE".to_string(),
                        severity: "warn".to_string(),
                        message: format!(
                            "Free disk ({f}B) is below 2x data-dir ({d}B) + 512 MiB headroom"
                        ),
                    });
                }
                Some(ok)
            }
            _ => None,
        };
        (data, free, ok)
    } else {
        (None, None, None)
    };
    let history_retention_mode = status_val
        .get("storage")
        .and_then(|s| s.get("backend"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    // Panel 1 & 7: Optional 2-of-3 signed release manifest check (fail-soft).
    let (upgrade_alert, latest_release_id, activation_height, blocks_until_activation) =
        check_release_manifest_fail_soft(
            options,
            git_revision.as_deref(),
            finalized_height,
            now_unix,
            &mut warnings,
        );

    // Panel 2, 3, 6 & 7: Optional 2-of-3 signed uptime feed check (fail-soft).
    let (reward_eligibility, height_lag, latest_day_bps, mean_30d_bps, last_voted_height) =
        check_uptime_feed_fail_soft(options, &node_id, finalized_height, now_unix, &mut warnings);

    if height_lag.is_some_and(|lag| lag > 5) {
        warnings.push(TopWarning {
            code: "HEIGHT_LAG_HIGH".to_string(),
            severity: "warn".to_string(),
            message: format!(
                "Node finalized height lags verified observer height by {} blocks",
                height_lag.unwrap_or(0)
            ),
        });
    }

    Ok(ValidatorTopSnapshotV1 {
        schema: TOP_SNAPSHOT_SCHEMA.to_string(),
        captured_at_unix: now_unix,
        rpc_endpoint: format!("{}:{}", options.rpc_host, options.rpc_port),
        node: TopNodeBanner {
            node_id,
            role,
            chain_id,
            git_revision,
            status: status_text,
            upgrade_alert,
            latest_release_id,
            activation_height,
            blocks_until_activation,
        },
        height: TopHeightPanel {
            finalized_height,
            view,
            block_tip_prefix,
            state_root_prefix,
            committee_epoch,
            height_lag,
        },
        peers_votes: TopPeersVotesPanel {
            configured_peers,
            connected_peers,
            quorum_size,
            latest_height_day_bps: latest_day_bps,
            mean_30d_participation_bps: mean_30d_bps,
            last_voted_height,
        },
        signer: TopSignerPanel {
            signer_mode,
            signer_reachable,
            signer_double_sign_protection,
            signer_key_matches_registry,
            separate_uid_enforced,
            custody_reasons,
        },
        disk_latency: TopDiskLatencyPanel {
            data_dir_bytes,
            fs_free_bytes,
            upgrade_headroom_ok,
            rpc_read_p95_ms: p95_latency_ms(latency_samples),
            last_status_ms: (status_ms * 100.0).round() / 100.0,
            rss_mib: read_process_rss_mib(),
            history_retention_mode,
            mempool_pending,
        },
        reward_eligibility,
        warnings,
    })
}

fn check_release_manifest_fail_soft(
    options: &ValidatorTopOptions,
    installed_git_rev: Option<&str>,
    finalized_height: u64,
    now_unix: u64,
    warnings: &mut Vec<TopWarning>,
) -> (String, Option<String>, Option<u64>, Option<u64>) {
    let raw_bytes = if let Some(path) = &options.release_manifest_file {
        match fs::read(path) {
            Ok(b) if b.len() <= MAX_RELEASE_MANIFEST_BYTES => b,
            _ => {
                warnings.push(TopWarning {
                    code: "RELEASE_CHECK_UNAVAILABLE".to_string(),
                    severity: "warn".to_string(),
                    message: format!("Cannot read release manifest `{}`", path.display()),
                });
                return ("UNCHECKED".to_string(), None, None, None);
            }
        }
    } else if let Some(url) = &options.release_manifest_url {
        match fetch_https_bounded(url, MAX_RELEASE_MANIFEST_BYTES) {
            Ok(b) => b,
            Err(error) => {
                warnings.push(TopWarning {
                    code: "RELEASE_CHECK_UNAVAILABLE".to_string(),
                    severity: "warn".to_string(),
                    message: format!("Release channel fetch failed: {error}"),
                });
                return ("UNCHECKED".to_string(), None, None, None);
            }
        }
    } else {
        return ("UNCHECKED".to_string(), None, None, None);
    };

    let keyset: ReleaseKeysetV1 = match fs::read_to_string(&options.release_keyset_file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    {
        Some(k) => k,
        None => {
            warnings.push(TopWarning {
                code: "RELEASE_CHECK_UNAVAILABLE".to_string(),
                severity: "warn".to_string(),
                message: format!(
                    "Release keyset `{}` unavailable or invalid",
                    options.release_keyset_file.display()
                ),
            });
            return ("UNCHECKED".to_string(), None, None, None);
        }
    };

    let manifest: ReleaseManifestV3 = match serde_json::from_slice(&raw_bytes) {
        Ok(m) => m,
        Err(error) => {
            warnings.push(TopWarning {
                code: "RELEASE_CHECK_UNAVAILABLE".to_string(),
                severity: "warn".to_string(),
                message: format!("Release manifest JSON invalid: {error}"),
            });
            return ("UNCHECKED".to_string(), None, None, None);
        }
    };

    let verified = match verify_release_manifest_v3(&manifest, &keyset, now_unix) {
        Ok(v) => v,
        Err(error) => {
            warnings.push(TopWarning {
                code: "RELEASE_CHECK_UNAVAILABLE".to_string(),
                severity: "warn".to_string(),
                message: format!("Release manifest 2-of-3 verification failed: {error}"),
            });
            return ("UNCHECKED".to_string(), None, None, None);
        }
    };

    let blocks_until = verified
        .activation_height
        .map(|h| h.saturating_sub(finalized_height));
    if installed_git_rev.is_some_and(|rev| rev == verified.git_revision) {
        return (
            "UP TO DATE".to_string(),
            Some(verified.release_id),
            verified.activation_height,
            blocks_until,
        );
    }
    if verified.rollout_mode == "activation_height" {
        if let (Some(act_h), Some(rem)) = (verified.activation_height, blocks_until) {
            if verified.advance_warning_blocks.is_some_and(|w| rem <= w) {
                warnings.push(TopWarning {
                    code: "ACTIVATION_IMMINENT".to_string(),
                    severity: "critical".to_string(),
                    message: format!(
                        "Release `{}` activates at height {act_h} ({rem} blocks remaining)",
                        verified.release_id
                    ),
                });
            }
            return (
                format!("ACTIVATION HEIGHT {act_h} IN {rem} BLOCKS"),
                Some(verified.release_id),
                Some(act_h),
                Some(rem),
            );
        }
    }
    (
        "ROLLING UPGRADE AVAILABLE".to_string(),
        Some(verified.release_id),
        verified.activation_height,
        blocks_until,
    )
}

fn check_uptime_feed_fail_soft(
    options: &ValidatorTopOptions,
    node_id: &str,
    local_height: u64,
    now_unix: u64,
    warnings: &mut Vec<TopWarning>,
) -> (
    TopRewardEligibilityPanel,
    Option<u64>,
    Option<u32>,
    Option<u32>,
    Option<u64>,
) {
    let default_panel = |state: &str, quorum: &str| TopRewardEligibilityPanel {
        access_state: state.to_string(),
        access_state_reason: None,
        observer_quorum: quorum.to_string(),
        feed_age_seconds: None,
        observed_height_days: None,
        mean_daily_participation_bps: None,
        grace_remaining_blocks: None,
        recovery_streak_days: None,
        finance_hub_access: None,
        reward_eligible: None,
    };

    let raw_bytes = if let Some(path) = &options.uptime_feed_file {
        match fs::read(path) {
            Ok(b) if b.len() <= MAX_UPTIME_FEED_BYTES => b,
            _ => {
                warnings.push(TopWarning {
                    code: "FEED_UNAVAILABLE".to_string(),
                    severity: "warn".to_string(),
                    message: format!("Cannot read uptime feed `{}`", path.display()),
                });
                return (
                    default_panel("unknown", "unavailable"),
                    None,
                    None,
                    None,
                    None,
                );
            }
        }
    } else if let Some(url) = &options.uptime_feed_url {
        match fetch_https_bounded(url, MAX_UPTIME_FEED_BYTES) {
            Ok(b) => b,
            Err(error) => {
                warnings.push(TopWarning {
                    code: "FEED_UNAVAILABLE".to_string(),
                    severity: "warn".to_string(),
                    message: format!("Uptime feed fetch failed: {error}"),
                });
                return (
                    default_panel("unknown", "unavailable"),
                    None,
                    None,
                    None,
                    None,
                );
            }
        }
    } else {
        return (
            default_panel("not_configured", "none"),
            None,
            None,
            None,
            None,
        );
    };

    let keyset: ObserverKeysetV1 = match fs::read_to_string(&options.observer_keyset_file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    {
        Some(k) => k,
        None => {
            warnings.push(TopWarning {
                code: "FEED_UNAVAILABLE".to_string(),
                severity: "warn".to_string(),
                message: format!(
                    "Observer keyset `{}` unavailable or invalid",
                    options.observer_keyset_file.display()
                ),
            });
            return (
                default_panel("unknown", "unverified"),
                None,
                None,
                None,
                None,
            );
        }
    };

    let feed: ValidatorUptimeFeedV1 = match serde_json::from_slice(&raw_bytes) {
        Ok(f) => f,
        Err(error) => {
            warnings.push(TopWarning {
                code: "FEED_UNAVAILABLE".to_string(),
                severity: "warn".to_string(),
                message: format!("Uptime feed JSON invalid: {error}"),
            });
            return (default_panel("unknown", "invalid"), None, None, None, None);
        }
    };

    let verified = match verify_validator_uptime_feed(&feed, &keyset, Some(now_unix)) {
        Ok(v) => v,
        Err(error) => {
            warnings.push(TopWarning {
                code: "FEED_UNAVAILABLE".to_string(),
                severity: "warn".to_string(),
                message: format!("Uptime feed 2-of-3 verification failed: {error}"),
            });
            return (default_panel("unknown", "invalid"), None, None, None, None);
        }
    };

    let quorum_label = if verified.feed_stale {
        warnings.push(TopWarning {
            code: "FEED_STALE".to_string(),
            severity: "warn".to_string(),
            message: "Observer uptime feed is stale (>3600s); existing access preserved"
                .to_string(),
        });
        "stale".to_string()
    } else {
        format!("{}/3", verified.fresh_observer_count)
    };

    let height_lag = Some(feed.chain.finalized_height.saturating_sub(local_height));
    let Some(row) = feed.validators.iter().find(|v| v.validator_id == node_id) else {
        return (
            default_panel("not_listed", &quorum_label),
            height_lag,
            None,
            None,
            None,
        );
    };

    if row
        .mean_daily_participation_bps
        .is_some_and(|bps| bps < 9_500)
    {
        warnings.push(TopWarning {
            code: "PARTICIPATION_BELOW_95".to_string(),
            severity: "warn".to_string(),
            message: format!(
                "Mean participation {} bps is below 9500 bps (95.00%) threshold",
                row.mean_daily_participation_bps.unwrap_or(0)
            ),
        });
    }

    let latest_day = row.height_days.last();
    let latest_day_bps = latest_day.and_then(|d| d.participation_bps);
    let last_voted_height = latest_day
        .filter(|d| d.credited_heights > 0)
        .map(|d| d.to_height);
    let grace_remaining = row
        .grace_expires_height
        .map(|exp| exp.saturating_sub(feed.chain.finalized_height));

    (
        TopRewardEligibilityPanel {
            access_state: row.access_state.clone(),
            access_state_reason: Some(row.access_state_reason.clone()),
            observer_quorum: quorum_label,
            feed_age_seconds: verified.oldest_fresh_age_seconds,
            observed_height_days: Some(row.observed_height_days),
            mean_daily_participation_bps: row.mean_daily_participation_bps,
            grace_remaining_blocks: grace_remaining,
            recovery_streak_days: Some(row.consecutive_passing_days),
            finance_hub_access: Some(row.finance_hub_access),
            reward_eligible: Some(row.reward_eligible),
        },
        height_lag,
        latest_day_bps,
        row.mean_daily_participation_bps,
        last_voted_height,
    )
}

pub fn render_top_dashboard(
    snapshot: &ValidatorTopSnapshotV1,
    color: bool,
    paused: bool,
    warning_scroll: usize,
) -> String {
    let (bold, green, yellow, red, cyan, reset) = if color {
        (
            "\x1b[1m", "\x1b[32m", "\x1b[33m", "\x1b[31m", "\x1b[36m", "\x1b[0m",
        )
    } else {
        ("", "", "", "", "", "")
    };

    let state_color = match snapshot.reward_eligibility.access_state.as_str() {
        "active" => green,
        "lapsed" => yellow,
        "revoked" => red,
        _ => cyan,
    };

    let mut out = String::new();
    out.push_str(&format!(
        "{bold}{cyan}=== POST FIAT NODE TOP ==={reset}  endpoint={}  {}\n",
        snapshot.rpc_endpoint,
        if paused { "[PAUSED]" } else { "[LIVE]" }
    ));
    out.push_str(&format!(
        "{bold}[1] Node & Release{reset}: id={} role={} chain={} rev={} alert={}\n",
        snapshot.node.node_id,
        snapshot.node.role,
        snapshot.node.chain_id,
        snapshot
            .node
            .git_revision
            .as_deref()
            .map(|r| &r[..r.len().min(12)])
            .unwrap_or("unknown"),
        snapshot.node.upgrade_alert,
    ));
    out.push_str(&format!(
        "{bold}[2] Height & Lag  {reset}: height={} view={} tip={} root={} epoch={} lag={}\n",
        snapshot.height.finalized_height,
        snapshot
            .height
            .view
            .map_or_else(|| "-".to_string(), |v| v.to_string()),
        snapshot.height.block_tip_prefix,
        snapshot.height.state_root_prefix,
        snapshot
            .height
            .committee_epoch
            .map_or_else(|| "-".to_string(), |e| e.to_string()),
        snapshot
            .height
            .height_lag
            .map_or_else(|| "-".to_string(), |l| format!("{l} blk")),
    ));
    out.push_str(&format!(
        "{bold}[3] Peers & Votes {reset}: peers={}/{} quorum={} day_bps={} mean30d_bps={} last_vote_h={}\n",
        snapshot
            .peers_votes
            .connected_peers
            .map_or_else(|| "-".to_string(), |c| c.to_string()),
        snapshot.peers_votes.configured_peers,
        snapshot.peers_votes.quorum_size,
        snapshot
            .peers_votes
            .latest_height_day_bps
            .map_or_else(|| "-".to_string(), |b| format!("{b}")),
        snapshot
            .peers_votes
            .mean_30d_participation_bps
            .map_or_else(|| "-".to_string(), |b| format!("{b}")),
        snapshot
            .peers_votes
            .last_voted_height
            .map_or_else(|| "-".to_string(), |h| h.to_string()),
    ));
    out.push_str(&format!(
        "{bold}[4] Signer Guard  {reset}: mode={} reachable={} double_sign_guard={} key_match={} sep_uid={}\n",
        snapshot.signer.signer_mode,
        opt_bool(snapshot.signer.signer_reachable),
        opt_bool(snapshot.signer.signer_double_sign_protection),
        opt_bool(snapshot.signer.signer_key_matches_registry),
        opt_bool(snapshot.signer.separate_uid_enforced),
    ));
    out.push_str(&format!(
        "{bold}[5] Disk & RPC    {reset}: data={} free={} upgrade_2x_ok={} rpc_p95={:.2}ms rss={}MiB mempool={}\n",
        fmt_bytes(snapshot.disk_latency.data_dir_bytes),
        fmt_bytes(snapshot.disk_latency.fs_free_bytes),
        opt_bool(snapshot.disk_latency.upgrade_headroom_ok),
        snapshot.disk_latency.rpc_read_p95_ms,
        snapshot
            .disk_latency
            .rss_mib
            .map_or_else(|| "-".to_string(), |r| format!("{r:.1}")),
        snapshot.disk_latency.mempool_pending,
    ));
    out.push_str(&format!(
        "{bold}[6] Task Node Elig{reset}: state={state_color}{}{reset} quorum={} age={}s days={}/30 mean_bps={} fh={} reward={} recovery={}/3\n",
        snapshot.reward_eligibility.access_state,
        snapshot.reward_eligibility.observer_quorum,
        snapshot
            .reward_eligibility
            .feed_age_seconds
            .map_or_else(|| "-".to_string(), |a| a.to_string()),
        snapshot
            .reward_eligibility
            .observed_height_days
            .map_or_else(|| "-".to_string(), |d| d.to_string()),
        snapshot
            .reward_eligibility
            .mean_daily_participation_bps
            .map_or_else(|| "-".to_string(), |b| b.to_string()),
        opt_bool(snapshot.reward_eligibility.finance_hub_access),
        opt_bool(snapshot.reward_eligibility.reward_eligible),
        snapshot
            .reward_eligibility
            .recovery_streak_days
            .map_or_else(|| "-".to_string(), |d| d.to_string()),
    ));
    out.push_str(&format!(
        "{bold}[7] Warnings ({})  {reset}:\n",
        snapshot.warnings.len()
    ));
    if snapshot.warnings.is_empty() {
        out.push_str(&format!("  {green}none{reset}\n"));
    } else {
        for warn in snapshot.warnings.iter().skip(warning_scroll).take(6) {
            let w_color = if warn.severity == "critical" {
                red
            } else {
                yellow
            };
            out.push_str(&format!(
                "  {w_color}[{}]{reset} {}\n",
                warn.code, warn.message
            ));
        }
    }
    out.push_str("Keys: [q] quit  [r] refresh  [p] pause  [j/k] scroll warnings\n");
    out
}

fn opt_bool(val: Option<bool>) -> &'static str {
    match val {
        Some(true) => "yes",
        Some(false) => "no",
        None => "-",
    }
}

fn fmt_bytes(bytes: Option<u64>) -> String {
    let Some(b) = bytes else {
        return "-".to_string();
    };
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if b as f64 >= GIB {
        format!("{:.2}GiB", b as f64 / GIB)
    } else if b as f64 >= MIB {
        format!("{:.1}MiB", b as f64 / MIB)
    } else {
        format!("{b}B")
    }
}

#[cfg(unix)]
struct RawTtyGuard {
    orig: libc::termios,
    active: bool,
}

#[cfg(unix)]
impl RawTtyGuard {
    fn enter() -> Self {
        unsafe {
            if libc::isatty(libc::STDIN_FILENO) == 0 {
                return Self {
                    orig: std::mem::zeroed(),
                    active: false,
                };
            }
            let mut orig: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut orig) != 0 {
                return Self {
                    orig,
                    active: false,
                };
            }
            let mut raw = orig;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO);
            raw.c_cc[libc::VMIN] = 0;
            raw.c_cc[libc::VTIME] = 0;
            let active = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) == 0;
            Self { orig, active }
        }
    }

    fn read_key(&self) -> Option<u8> {
        if !self.active {
            return None;
        }
        let mut byte = [0_u8; 1];
        let n = unsafe { libc::read(libc::STDIN_FILENO, byte.as_mut_ptr().cast(), 1) };
        if n == 1 {
            Some(byte[0])
        } else {
            None
        }
    }
}

#[cfg(unix)]
impl Drop for RawTtyGuard {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.orig);
            }
        }
    }
}

pub fn run_validator_top(options: &ValidatorTopOptions) -> Result<(), String> {
    validate_top_options(options)?;
    let mut latency_samples = VecDeque::with_capacity(LATENCY_WINDOW_SIZE);

    #[cfg(unix)]
    let stdout_is_tty = unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 };
    #[cfg(not(unix))]
    let stdout_is_tty = false;
    let use_color = !options.no_color && std::env::var_os("NO_COLOR").is_none() && stdout_is_tty;

    if options.once {
        let snapshot = collect_top_snapshot(options, &mut latency_samples)?;
        if options.json {
            let text = serde_json::to_string_pretty(&snapshot)
                .map_err(|error| format!("top snapshot JSON encode failed: {error}"))?;
            println!("{text}");
        } else {
            print!("{}", render_top_dashboard(&snapshot, use_color, false, 0));
        }
        return Ok(());
    }

    #[cfg(unix)]
    let tty = if !options.json {
        Some(RawTtyGuard::enter())
    } else {
        None
    };

    let mut paused = false;
    let mut warning_scroll = 0_usize;
    let mut last_snapshot: Option<ValidatorTopSnapshotV1> = None;
    let refresh = Duration::from_millis(options.refresh_ms);

    loop {
        if !paused || last_snapshot.is_none() {
            let snapshot = collect_top_snapshot(options, &mut latency_samples)?;
            if options.json {
                let line = serde_json::to_string(&snapshot)
                    .map_err(|error| format!("top NDJSON encode failed: {error}"))?;
                println!("{line}");
                let _ = io::stdout().flush();
            } else {
                if stdout_is_tty {
                    print!("\x1b[2J\x1b[H");
                }
                print!(
                    "{}",
                    render_top_dashboard(&snapshot, use_color, paused, warning_scroll)
                );
                let _ = io::stdout().flush();
            }
            last_snapshot = Some(snapshot);
        }

        let wait_until = Instant::now() + refresh;
        while Instant::now() < wait_until {
            #[cfg(unix)]
            if let Some(key) = tty.as_ref().and_then(RawTtyGuard::read_key) {
                match key {
                    b'q' | b'Q' | 3 => return Ok(()),
                    b'r' | b'R' => break,
                    b'p' | b'P' => {
                        paused = !paused;
                        if let Some(snap) = &last_snapshot {
                            if stdout_is_tty {
                                print!("\x1b[2J\x1b[H");
                            }
                            print!(
                                "{}",
                                render_top_dashboard(snap, use_color, paused, warning_scroll)
                            );
                            let _ = io::stdout().flush();
                        }
                    }
                    b'j' => {
                        if let Some(snap) = &last_snapshot {
                            if warning_scroll + 1 < snap.warnings.len() {
                                warning_scroll += 1;
                            }
                            if stdout_is_tty {
                                print!("\x1b[2J\x1b[H");
                            }
                            print!(
                                "{}",
                                render_top_dashboard(snap, use_color, paused, warning_scroll)
                            );
                            let _ = io::stdout().flush();
                        }
                    }
                    b'k' => {
                        warning_scroll = warning_scroll.saturating_sub(1);
                        if let Some(snap) = &last_snapshot {
                            if stdout_is_tty {
                                print!("\x1b[2J\x1b[H");
                            }
                            print!(
                                "{}",
                                render_top_dashboard(snap, use_color, paused, warning_scroll)
                            );
                            let _ = io::stdout().flush();
                        }
                    }
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    #[test]
    fn top_enforces_loopback_and_refresh_bounds() {
        let mut opts = ValidatorTopOptions {
            rpc_host: "203.0.113.10".to_string(),
            rpc_port: 27650,
            allow_remote_rpc: false,
            data_dir: None,
            uptime_feed_url: None,
            uptime_feed_file: None,
            observer_keyset_file: PathBuf::from(DEFAULT_OBSERVER_KEYSET_FILE),
            release_manifest_url: None,
            release_manifest_file: None,
            release_keyset_file: PathBuf::from(DEFAULT_TOP_RELEASE_KEYSET_FILE),
            refresh_ms: 2_000,
            once: true,
            json: true,
            no_color: true,
            rpc_timeout_ms: 2_000,
            now_unix: Some(1_795_651_500),
        };
        assert!(validate_top_options(&opts).is_err());
        opts.allow_remote_rpc = true;
        assert!(validate_top_options(&opts).is_ok());
        opts.refresh_ms = 100;
        assert!(validate_top_options(&opts).is_err());
    }

    #[test]
    fn top_collects_snapshot_from_mock_rpc_and_fails_soft_on_missing_feed() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind mock rpc");
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            for _ in 0..5 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut line = String::new();
                let _ = BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
                let req: RpcRequest = serde_json::from_str(&line).unwrap();
                let result = match req.method.as_str() {
                    "status" => serde_json::json!({
                        "node_id": "validator-0",
                        "chain_id": "postfiat-testnet-1",
                        "status": "ok",
                        "block_height": 432000,
                        "block_tip_hash": "ab".repeat(48),
                        "state_root": "cd".repeat(48),
                        "validator_count": 6,
                        "mempool_pending": 3,
                        "build_git_revision": "11".repeat(20),
                    }),
                    "server_info" => serde_json::json!({
                        "node_id": "validator-0",
                        "node_role": "validator",
                    }),
                    "metrics" => serde_json::json!({
                        "consensus": { "view": 7, "committee_epoch": 12 },
                        "rpc": { "active_connections": 4 },
                    }),
                    "private_readiness" => serde_json::json!({
                        "ready": true,
                        "reasons": [],
                        "custody": {
                            "validator_signer": "socket",
                            "signer_double_sign_protection": true,
                            "signer_key_matches": true,
                            "serving_uid": 1001,
                            "signer_uid": 1002
                        }
                    }),
                    "validators" => serde_json::json!({
                        "validator_count": 6,
                        "validators": []
                    }),
                    _ => serde_json::json!({}),
                };
                let resp = postfiat_rpc_sdk::success_response(&req.id, &result, vec![]).unwrap();
                let mut wire = serde_json::to_vec(&resp).unwrap();
                wire.extend_from_slice(b"\n");
                let _ = stream.write_all(&wire);
            }
        });

        let opts = ValidatorTopOptions {
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: port,
            allow_remote_rpc: false,
            data_dir: Some(std::env::temp_dir()),
            uptime_feed_url: None,
            uptime_feed_file: Some(PathBuf::from("/nonexistent/postfiat-feed.json")),
            observer_keyset_file: PathBuf::from("/nonexistent/observer-keyset.json"),
            release_manifest_url: None,
            release_manifest_file: None,
            release_keyset_file: PathBuf::from("/nonexistent/release-keyset.json"),
            refresh_ms: 2_000,
            once: true,
            json: true,
            no_color: true,
            rpc_timeout_ms: 2_000,
            now_unix: Some(1_795_651_500),
        };
        let mut samples = VecDeque::new();
        let snap = collect_top_snapshot(&opts, &mut samples).expect("collect snapshot");
        server.join().expect("join mock rpc");

        assert_eq!(snap.schema, TOP_SNAPSHOT_SCHEMA);
        assert_eq!(snap.node.node_id, "validator-0");
        assert_eq!(snap.height.finalized_height, 432_000);
        assert_eq!(snap.height.view, Some(7));
        assert_eq!(snap.peers_votes.configured_peers, 6);
        assert_eq!(snap.peers_votes.quorum_size, 5);
        assert_eq!(snap.signer.signer_mode, "socket");
        assert_eq!(snap.signer.signer_double_sign_protection, Some(true));
        assert_eq!(snap.signer.separate_uid_enforced, Some(true));
        assert!(snap.warnings.iter().any(|w| w.code == "FEED_UNAVAILABLE"));

        let rendered = render_top_dashboard(&snap, false, false, 0);
        assert!(rendered.contains("POST FIAT NODE TOP"));
        assert!(rendered.contains("validator-0"));
        assert!(rendered.contains("FEED_UNAVAILABLE"));
    }
}
