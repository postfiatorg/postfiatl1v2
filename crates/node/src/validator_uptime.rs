use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hex_to_bytes, ml_dsa_65_keygen,
    ml_dsa_65_sign_with_context, ml_dsa_65_validate_public_key, ml_dsa_65_verify_with_context,
    ML_DSA_65_ALGORITHM, ML_DSA_65_PUBLIC_KEY_BYTES, ML_DSA_65_SIGNATURE_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as Sha2Digest, Sha256};

use crate::{set_private_file_permissions, unix_now};

pub const VALIDATOR_UPTIME_FEED_SCHEMA: &str = "postfiat-validator-uptime-v1";
pub const VALIDATOR_UPTIME_FEED_SCHEMA_VERSION: u32 = 1;
pub const VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT: &str = "postfiat-l1-v2/validator-uptime-feed/v1";
pub const OBSERVER_KEYSET_SCHEMA: &str = "postfiat.observer_keyset.v1";
pub const OBSERVER_PRIVATE_KEY_SCHEMA: &str = "postfiat.observer_private_key.v1";
pub const OBSERVER_KEY_PURPOSE: &str = "validator-uptime-observer";

pub const UPTIME_METRIC_VERSION: &str = "latency-fair-round-window-v1";
pub const UPTIME_EVALUATION_HORIZON_DAYS: u32 = 30;
pub const UPTIME_MIN_OBSERVED_DAYS: u32 = 25;
pub const UPTIME_LIVENESS_THRESHOLD_BPS: u32 = 9_500;
pub const UPTIME_GRACE_PERIOD_DAYS: u32 = 14;
pub const UPTIME_RECOVERY_CONSECUTIVE_DAYS: u32 = 3;
pub const UPTIME_STALE_AFTER_SECONDS: u64 = 3_600;
pub const UPTIME_MAX_FUTURE_SKEW_SECONDS: u64 = 60;
pub const UPTIME_STALE_BEHAVIOR: &str = "block_new_bindings_and_credits_preserve_existing_access";

pub const UPTIME_CREDIT_SOURCES: [&str; 4] = [
    "prepare_vote",
    "precommit_vote",
    "timeout_vote",
    "consensus_heartbeat",
];

/// Recursively canonicalize any `serde_json::Value` by sorting all object keys
/// lexicographically and encoding with compact UTF-8 separators (`,` and `:`),
/// matching Python `json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)`.
pub fn canonical_json_bytes(value: &serde_json::Value) -> Result<Vec<u8>, String> {
    let canonical = canonicalize_json_value(value);
    serde_json::to_vec(&canonical)
        .map_err(|error| format!("canonical JSON encoding failed: {error}"))
}

fn canonicalize_json_value(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize_json_value(v));
            }
            let mut out = serde_json::Map::with_capacity(sorted.len());
            for (k, v) in sorted {
                out.insert(k, v);
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize_json_value).collect())
        }
        other => other.clone(),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    bytes_to_hex(&hasher.finalize())
}

pub fn is_lowercase_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverPrivateKeyFileV1 {
    pub schema: String,
    pub purpose: String,
    pub observer_id: String,
    pub region: String,
    pub operator_group: String,
    pub address: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub private_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverKeysetEntryV1 {
    pub observer_id: String,
    pub region: String,
    pub operator_group: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverKeysetV1 {
    pub schema: String,
    pub keyset_id: String,
    pub observer_count: usize,
    pub quorum_threshold: usize,
    pub algorithm_id: String,
    pub signing_context: String,
    pub observers: Vec<ObserverKeysetEntryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptimeFeedChainV1 {
    pub network: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub committee_epoch: u64,
    pub registry_root: String,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptimeQuorumPolicyV1 {
    pub observer_count: usize,
    pub quorum_threshold: usize,
    pub algorithm_id: String,
    pub signing_context: String,
    pub stale_after_seconds: u64,
    pub max_future_skew_seconds: u64,
    pub stale_behavior: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptimeWindowSpecV1 {
    pub metric_version: String,
    pub credit_sources: Vec<String>,
    pub blocks_per_height_day: u64,
    pub evaluation_horizon_days: u32,
    pub min_observed_days: u32,
    pub liveness_threshold_bps: u32,
    pub grace_period_days: u32,
    pub recovery_consecutive_days: u32,
    pub window_start_height: u64,
    pub window_end_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeightDayObservationV1 {
    pub day_index: u64,
    pub from_height: u64,
    pub to_height: u64,
    pub complete: bool,
    pub eligible_heights: u64,
    pub credited_heights: u64,
    pub precommit_qc_heights: u64,
    pub participation_bps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorQuorumStatusV1 {
    pub validator_id: String,
    pub ml_dsa_public_key: String,
    pub operator_identity_id: Option<String>,
    pub public_rpc_url: Option<String>,
    pub registry_active: bool,
    pub first_active_height: Option<u64>,
    pub ramp_in_active: bool,
    pub observed_height_days: u32,
    pub mean_daily_participation_bps: Option<u32>,
    pub consecutive_passing_days: u32,
    pub liveness_met: bool,
    pub access_state: String,
    pub access_state_reason: String,
    pub grace_started_at: Option<String>,
    pub grace_expires_at: Option<String>,
    pub grace_started_height: Option<u64>,
    pub grace_expires_height: Option<u64>,
    pub recovery_floor_height: Option<u64>,
    pub removal_proposal_eligible: bool,
    pub reward_eligible: bool,
    pub finance_hub_access: bool,
    pub research_api_keys_active: bool,
    pub agreeing_observer_ids: Vec<String>,
    pub height_days: Vec<HeightDayObservationV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverValidatorClaimV1 {
    pub validator_id: String,
    pub ml_dsa_public_key: String,
    pub registry_active: bool,
    pub observed_height_days: u32,
    pub mean_daily_participation_bps: Option<u32>,
    pub consecutive_passing_days: u32,
    pub liveness_met: bool,
    pub access_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverAttestationV1 {
    pub observer_id: String,
    pub region: String,
    pub operator_group: String,
    pub observed_at: String,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub registry_root: String,
    pub validator_claims: Vec<ObserverValidatorClaimV1>,
    pub claim_digest_sha256: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorUptimeFeedV1 {
    pub schema: String,
    pub schema_version: u32,
    pub generated_at: String,
    pub chain: UptimeFeedChainV1,
    pub quorum_policy: UptimeQuorumPolicyV1,
    pub window_spec: UptimeWindowSpecV1,
    pub validators: Vec<ValidatorQuorumStatusV1>,
    pub observer_attestations: Vec<ObserverAttestationV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UptimeFeedVerificationReport {
    pub schema: String,
    pub valid: bool,
    pub feed_stale: bool,
    pub verified_observer_count: usize,
    pub fresh_observer_count: usize,
    pub quorum_threshold: usize,
    pub verified_observer_ids: Vec<String>,
    pub stale_observer_ids: Vec<String>,
    pub oldest_fresh_age_seconds: Option<u64>,
    pub allow_new_bindings: bool,
    pub allow_credit_grants: bool,
    pub revoke_existing_on_stale: bool,
    pub finalized_height: u64,
    pub validator_count: usize,
}

pub fn create_observer_key_file(
    path: &Path,
    observer_id: &str,
    region: &str,
    operator_group: &str,
) -> io::Result<ObserverPrivateKeyFileV1> {
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to overwrite existing observer key file `{}`",
                path.display()
            ),
        ));
    }
    if observer_id.len() < 3 || region.len() < 2 || operator_group.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "observer_id, region, and operator_group must be non-empty valid identifiers",
        ));
    }
    let keypair = ml_dsa_65_keygen()
        .map_err(|error| io::Error::other(format!("ML-DSA-65 keygen failed: {error}")))?;
    let public_key_hex = bytes_to_hex(&keypair.public_key);
    let private_key_hex = bytes_to_hex(&keypair.private_key);
    let address = address_from_public_key(&keypair.public_key);
    let record = ObserverPrivateKeyFileV1 {
        schema: OBSERVER_PRIVATE_KEY_SCHEMA.to_string(),
        purpose: OBSERVER_KEY_PURPOSE.to_string(),
        observer_id: observer_id.to_string(),
        region: region.to_string(),
        operator_group: operator_group.to_string(),
        address,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        private_key_hex,
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(&record)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(path, format!("{json}\n"))?;
    set_private_file_permissions(path)?;
    Ok(record)
}

pub fn observer_claim_canonical_bytes(
    chain: &UptimeFeedChainV1,
    window_spec: &UptimeWindowSpecV1,
    attestation: &ObserverAttestationV1,
) -> Result<Vec<u8>, String> {
    let body = serde_json::json!({
        "chain_id": &chain.chain_id,
        "finalized_block_hash": &attestation.finalized_block_hash,
        "finalized_height": attestation.finalized_height,
        "genesis_hash": &chain.genesis_hash,
        "network": &chain.network,
        "observed_at": &attestation.observed_at,
        "observer_id": &attestation.observer_id,
        "operator_group": &attestation.operator_group,
        "region": &attestation.region,
        "registry_root": &attestation.registry_root,
        "schema": VALIDATOR_UPTIME_FEED_SCHEMA,
        "schema_version": VALIDATOR_UPTIME_FEED_SCHEMA_VERSION,
        "validator_claims": &attestation.validator_claims,
        "window_spec": window_spec,
    });
    canonical_json_bytes(&body)
}

pub fn sign_observer_attestation(
    key: &ObserverPrivateKeyFileV1,
    chain: &UptimeFeedChainV1,
    window_spec: &UptimeWindowSpecV1,
    observed_at: &str,
    mut validator_claims: Vec<ObserverValidatorClaimV1>,
) -> Result<ObserverAttestationV1, String> {
    if key.schema != OBSERVER_PRIVATE_KEY_SCHEMA || key.purpose != OBSERVER_KEY_PURPOSE {
        return Err("invalid observer private key schema or purpose".to_string());
    }
    if key.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(format!(
            "unsupported observer key algorithm `{}`",
            key.algorithm_id
        ));
    }
    validator_claims.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    let private_key_bytes = hex_to_bytes(&key.private_key_hex)
        .map_err(|error| format!("bad private key hex: {error}"))?;
    let mut attestation = ObserverAttestationV1 {
        observer_id: key.observer_id.clone(),
        region: key.region.clone(),
        operator_group: key.operator_group.clone(),
        observed_at: observed_at.to_string(),
        finalized_height: chain.finalized_height,
        finalized_block_hash: chain.finalized_block_hash.clone(),
        registry_root: chain.registry_root.clone(),
        validator_claims,
        claim_digest_sha256: String::new(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key.public_key_hex.clone(),
        signature_hex: String::new(),
    };
    let canonical = observer_claim_canonical_bytes(chain, window_spec, &attestation)?;
    attestation.claim_digest_sha256 = sha256_hex(&canonical);
    let signature = ml_dsa_65_sign_with_context(
        &private_key_bytes,
        &canonical,
        VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT.as_bytes(),
    )
    .map_err(|error| format!("ML-DSA-65 observer signing failed: {error}"))?;
    attestation.signature_hex = bytes_to_hex(&signature);
    Ok(attestation)
}

pub fn validate_observer_keyset(keyset: &ObserverKeysetV1) -> Result<(), String> {
    if keyset.schema != OBSERVER_KEYSET_SCHEMA {
        return Err(format!(
            "unexpected observer keyset schema `{}` (expected `{OBSERVER_KEYSET_SCHEMA}`)",
            keyset.schema
        ));
    }
    if keyset.observer_count != 3 || keyset.quorum_threshold != 2 {
        return Err("observer keyset must require 2-of-3 quorum".to_string());
    }
    if keyset.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(format!(
            "unsupported observer keyset algorithm `{}`",
            keyset.algorithm_id
        ));
    }
    if keyset.signing_context != VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT {
        return Err(format!(
            "unexpected observer keyset signing_context `{}`",
            keyset.signing_context
        ));
    }
    if keyset.observers.len() != keyset.observer_count {
        return Err(format!(
            "observer keyset has {} entries, expected {}",
            keyset.observers.len(),
            keyset.observer_count
        ));
    }
    let mut ids = BTreeSet::new();
    let mut regions = BTreeSet::new();
    let mut groups = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for entry in &keyset.observers {
        if !ids.insert(entry.observer_id.as_str()) {
            return Err(format!(
                "duplicate observer_id `{}` in keyset",
                entry.observer_id
            ));
        }
        if !regions.insert(entry.region.as_str()) {
            return Err(format!("duplicate region `{}` in keyset", entry.region));
        }
        if !groups.insert(entry.operator_group.as_str()) {
            return Err(format!(
                "duplicate operator_group `{}` in keyset",
                entry.operator_group
            ));
        }
        if !is_lowercase_hex_len(&entry.public_key_hex, ML_DSA_65_PUBLIC_KEY_BYTES * 2) {
            return Err(format!(
                "observer `{}` public_key_hex is not valid ML-DSA-65 hex",
                entry.observer_id
            ));
        }
        if !keys.insert(entry.public_key_hex.as_str()) {
            return Err(format!(
                "duplicate public_key_hex in keyset for `{}`",
                entry.observer_id
            ));
        }
        let pk_bytes = hex_to_bytes(&entry.public_key_hex)
            .map_err(|error| format!("observer `{}` bad pubkey hex: {error}", entry.observer_id))?;
        ml_dsa_65_validate_public_key(&pk_bytes).map_err(|error| {
            format!(
                "observer `{}` invalid ML-DSA-65 public key: {error}",
                entry.observer_id
            )
        })?;
    }
    Ok(())
}

/// Parse an RFC 3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SS[.ffffff]Z`) into Unix seconds.
pub fn parse_rfc3339_utc_unix(raw: &str) -> Result<i64, String> {
    let trimmed = raw
        .strip_suffix('Z')
        .ok_or_else(|| format!("timestamp `{raw}` must end with `Z`"))?;
    let (date_part, time_part) = trimmed
        .split_once('T')
        .ok_or_else(|| format!("timestamp `{raw}` missing `T` separator"))?;
    let mut date_fields = date_part.split('-');
    let year: i64 = date_fields
        .next()
        .ok_or_else(|| format!("timestamp `{raw}` missing year"))?
        .parse()
        .map_err(|_| format!("timestamp `{raw}` invalid year"))?;
    let month: u32 = date_fields
        .next()
        .ok_or_else(|| format!("timestamp `{raw}` missing month"))?
        .parse()
        .map_err(|_| format!("timestamp `{raw}` invalid month"))?;
    let day: u32 = date_fields
        .next()
        .ok_or_else(|| format!("timestamp `{raw}` missing day"))?
        .parse()
        .map_err(|_| format!("timestamp `{raw}` invalid day"))?;
    if date_fields.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("timestamp `{raw}` invalid date"));
    }
    let time_whole = time_part.split('.').next().unwrap_or(time_part);
    let mut time_fields = time_whole.split(':');
    let hour: u32 = time_fields
        .next()
        .ok_or_else(|| format!("timestamp `{raw}` missing hour"))?
        .parse()
        .map_err(|_| format!("timestamp `{raw}` invalid hour"))?;
    let minute: u32 = time_fields
        .next()
        .ok_or_else(|| format!("timestamp `{raw}` missing minute"))?
        .parse()
        .map_err(|_| format!("timestamp `{raw}` invalid minute"))?;
    let second: u32 = time_fields
        .next()
        .ok_or_else(|| format!("timestamp `{raw}` missing second"))?
        .parse()
        .map_err(|_| format!("timestamp `{raw}` invalid second"))?;
    if time_fields.next().is_some() || hour > 23 || minute > 59 || second > 59 {
        return Err(format!("timestamp `{raw}` invalid time"));
    }
    let days = days_from_civil(year, month, day)?;
    Ok(days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))
}

fn days_from_civil(year: i64, month: u32, day: u32) -> Result<i64, String> {
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => return Err(format!("invalid month {month}")),
    };
    if day == 0 || day > max_day {
        return Err(format!("invalid calendar day {year}-{month:02}-{day:02}"));
    }
    // Howard Hinnant's civil-from-days inverse algorithm (days since 1970-01-01).
    let y = year - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = u64::from(month);
    let d = u64::from(day);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe as i64 - 719_468)
}

pub fn reconcile_quorum_bps(scores: &[Option<u32>]) -> Result<Option<u32>, String> {
    let mut non_null: Vec<u32> = scores.iter().copied().flatten().collect();
    if non_null.is_empty() {
        return Ok(None);
    }
    non_null.sort_unstable();
    match non_null.len() {
        3 => Ok(Some(non_null[1])),
        2 => Ok(Some(non_null[0])),
        other => Err(format!("quorum requires 2 or 3 scores, got {other}")),
    }
}

/// Verify a `postfiat-validator-uptime-v1` feed against a pinned 3-observer
/// `postfiat.observer_keyset.v1` trust file (2-of-3 ML-DSA-65 signatures,
/// canonical SHA-256 digests, freshness window, and quorum reconciliation).
pub fn verify_validator_uptime_feed(
    feed: &ValidatorUptimeFeedV1,
    keyset: &ObserverKeysetV1,
    now_unix: Option<u64>,
) -> Result<UptimeFeedVerificationReport, String> {
    validate_observer_keyset(keyset)?;
    let now_i64 = i64::try_from(now_unix.unwrap_or_else(unix_now))
        .map_err(|_| "current timestamp overflow".to_string())?;

    if feed.schema != VALIDATOR_UPTIME_FEED_SCHEMA
        || feed.schema_version != VALIDATOR_UPTIME_FEED_SCHEMA_VERSION
    {
        return Err(format!(
            "unexpected uptime feed schema `{}` v{}",
            feed.schema, feed.schema_version
        ));
    }
    parse_rfc3339_utc_unix(&feed.generated_at)?;

    if !matches!(
        feed.chain.network.as_str(),
        "l1-devnet" | "l1-testnet" | "l1-mainnet"
    ) {
        return Err(format!("unsupported network `{}`", feed.chain.network));
    }
    if feed.chain.finalized_height == 0
        || !is_lowercase_hex_len(&feed.chain.genesis_hash, 96)
        || !is_lowercase_hex_len(&feed.chain.registry_root, 96)
        || !is_lowercase_hex_len(&feed.chain.finalized_block_hash, 96)
    {
        return Err("invalid chain identity fields in uptime feed".to_string());
    }

    let qp = &feed.quorum_policy;
    if qp.observer_count != 3
        || qp.quorum_threshold != 2
        || qp.algorithm_id != ML_DSA_65_ALGORITHM
        || qp.signing_context != VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT
        || qp.stale_after_seconds != UPTIME_STALE_AFTER_SECONDS
        || qp.max_future_skew_seconds != UPTIME_MAX_FUTURE_SKEW_SECONDS
        || qp.stale_behavior != UPTIME_STALE_BEHAVIOR
    {
        return Err("quorum_policy violates frozen Contract C2 constants".to_string());
    }

    let ws = &feed.window_spec;
    let credit_set: BTreeSet<&str> = ws.credit_sources.iter().map(String::as_str).collect();
    let expected_credits: BTreeSet<&str> = UPTIME_CREDIT_SOURCES.iter().copied().collect();
    if ws.metric_version != UPTIME_METRIC_VERSION
        || ws.credit_sources.len() != 4
        || credit_set != expected_credits
        || ws.blocks_per_height_day == 0
        || ws.evaluation_horizon_days != UPTIME_EVALUATION_HORIZON_DAYS
        || ws.min_observed_days != UPTIME_MIN_OBSERVED_DAYS
        || ws.liveness_threshold_bps != UPTIME_LIVENESS_THRESHOLD_BPS
        || ws.grace_period_days != UPTIME_GRACE_PERIOD_DAYS
        || ws.recovery_consecutive_days != UPTIME_RECOVERY_CONSECUTIVE_DAYS
        || ws.window_start_height == 0
        || ws.window_end_height < ws.window_start_height
        || ws.window_end_height > feed.chain.finalized_height
    {
        return Err("window_spec violates frozen Contract C2 constants".to_string());
    }

    if !(2..=3).contains(&feed.observer_attestations.len()) {
        return Err(format!(
            "observer_attestations requires 2 or 3 entries, got {}",
            feed.observer_attestations.len()
        ));
    }

    let pinned_by_id: BTreeMap<&str, &ObserverKeysetEntryV1> = keyset
        .observers
        .iter()
        .map(|entry| (entry.observer_id.as_str(), entry))
        .collect();

    let mut seen_ids = BTreeSet::new();
    let mut seen_regions = BTreeSet::new();
    let mut seen_groups = BTreeSet::new();
    let mut seen_keys = BTreeSet::new();
    let mut verified_ids = Vec::new();
    let mut stale_ids = Vec::new();
    let mut verified_map: BTreeMap<&str, &ObserverAttestationV1> = BTreeMap::new();
    let mut oldest_fresh_age: Option<u64> = None;

    for att in &feed.observer_attestations {
        let Some(pinned) = pinned_by_id.get(att.observer_id.as_str()) else {
            return Err(format!(
                "observer `{}` is not in pinned observer keyset",
                att.observer_id
            ));
        };
        if att.region != pinned.region
            || att.operator_group != pinned.operator_group
            || att.public_key_hex != pinned.public_key_hex
        {
            return Err(format!(
                "observer `{}` metadata or public key disagrees with pinned keyset",
                att.observer_id
            ));
        }
        if !seen_ids.insert(att.observer_id.as_str())
            || !seen_regions.insert(att.region.as_str())
            || !seen_groups.insert(att.operator_group.as_str())
            || !seen_keys.insert(att.public_key_hex.as_str())
        {
            return Err(format!(
                "duplicate observer identity/region/group/key for `{}`",
                att.observer_id
            ));
        }
        if att.algorithm_id != ML_DSA_65_ALGORITHM
            || !is_lowercase_hex_len(&att.signature_hex, ML_DSA_65_SIGNATURE_BYTES * 2)
            || !is_lowercase_hex_len(&att.claim_digest_sha256, 64)
        {
            return Err(format!(
                "observer `{}` has invalid signature or digest encoding",
                att.observer_id
            ));
        }
        if att.finalized_height != feed.chain.finalized_height
            || att.finalized_block_hash != feed.chain.finalized_block_hash
            || att.registry_root != feed.chain.registry_root
        {
            return Err(format!(
                "observer `{}` chain identity disagrees with feed.chain",
                att.observer_id
            ));
        }
        if att.validator_claims.is_empty() {
            return Err(format!(
                "observer `{}` has empty validator_claims",
                att.observer_id
            ));
        }
        let mut prev_vid: Option<&str> = None;
        let mut claim_pks = BTreeSet::new();
        for claim in &att.validator_claims {
            if let Some(prev) = prev_vid {
                if claim.validator_id.as_str() <= prev {
                    return Err(format!(
                        "observer `{}` validator_claims not strictly sorted",
                        att.observer_id
                    ));
                }
            }
            prev_vid = Some(claim.validator_id.as_str());
            if !is_lowercase_hex_len(&claim.ml_dsa_public_key, ML_DSA_65_PUBLIC_KEY_BYTES * 2)
                || !claim_pks.insert(claim.ml_dsa_public_key.as_str())
            {
                return Err(format!(
                    "observer `{}` has invalid or duplicate validator public key",
                    att.observer_id
                ));
            }
        }

        let canonical = observer_claim_canonical_bytes(&feed.chain, &feed.window_spec, att)?;
        let expected_digest = sha256_hex(&canonical);
        if att.claim_digest_sha256 != expected_digest {
            return Err(format!(
                "observer `{}` claim_digest_sha256 mismatch",
                att.observer_id
            ));
        }
        let pk_bytes = hex_to_bytes(&att.public_key_hex)
            .map_err(|error| format!("observer `{}` bad pubkey hex: {error}", att.observer_id))?;
        let sig_bytes = hex_to_bytes(&att.signature_hex)
            .map_err(|error| format!("observer `{}` bad sig hex: {error}", att.observer_id))?;
        if !ml_dsa_65_verify_with_context(
            &pk_bytes,
            &canonical,
            &sig_bytes,
            VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT.as_bytes(),
        ) {
            return Err(format!(
                "observer `{}` ML-DSA-65 signature verification failed",
                att.observer_id
            ));
        }

        let observed_unix = parse_rfc3339_utc_unix(&att.observed_at)?;
        let future_skew = observed_unix - now_i64;
        if future_skew > UPTIME_MAX_FUTURE_SKEW_SECONDS as i64 {
            return Err(format!(
                "observer `{}` timestamp exceeds max future skew ({future_skew}s)",
                att.observer_id
            ));
        }
        let age = (now_i64 - observed_unix).max(0) as u64;
        verified_ids.push(att.observer_id.clone());
        verified_map.insert(att.observer_id.as_str(), att);
        if age > UPTIME_STALE_AFTER_SECONDS {
            stale_ids.push(att.observer_id.clone());
        } else {
            oldest_fresh_age = Some(oldest_fresh_age.map_or(age, |prev| prev.max(age)));
        }
    }

    if verified_ids.len() < keyset.quorum_threshold {
        return Err(format!(
            "insufficient verified observers: {} < {}",
            verified_ids.len(),
            keyset.quorum_threshold
        ));
    }

    let fresh_count = verified_ids.len() - stale_ids.len();
    let feed_stale = fresh_count < keyset.quorum_threshold;

    // Validate validator rows and 2-of-3 quorum reconciliation.
    if feed.validators.is_empty() {
        return Err("validators array must not be empty".to_string());
    }
    let mut prev_vid: Option<&str> = None;
    let mut val_pks = BTreeSet::new();
    let bpd = feed.window_spec.blocks_per_height_day;
    for v in &feed.validators {
        if let Some(prev) = prev_vid {
            if v.validator_id.as_str() <= prev {
                return Err("validators array must be strictly sorted".to_string());
            }
        }
        prev_vid = Some(v.validator_id.as_str());
        if !is_lowercase_hex_len(&v.ml_dsa_public_key, ML_DSA_65_PUBLIC_KEY_BYTES * 2)
            || !val_pks.insert(v.ml_dsa_public_key.as_str())
        {
            return Err(format!(
                "validator `{}` has invalid or duplicate ml_dsa_public_key",
                v.validator_id
            ));
        }
        validate_validator_access_entitlements(v, bpd)?;

        let mut agreeing_unique = BTreeSet::new();
        let mut scores = Vec::new();
        for oid in &v.agreeing_observer_ids {
            if !agreeing_unique.insert(oid.as_str()) {
                return Err(format!(
                    "validator `{}` has duplicate agreeing observer `{oid}`",
                    v.validator_id
                ));
            }
            let Some(att) = verified_map.get(oid.as_str()) else {
                return Err(format!(
                    "validator `{}` references unverified observer `{oid}`",
                    v.validator_id
                ));
            };
            let Some(claim) = att
                .validator_claims
                .iter()
                .find(|c| c.validator_id == v.validator_id)
            else {
                return Err(format!(
                    "observer `{oid}` missing claim for validator `{}`",
                    v.validator_id
                ));
            };
            if claim.ml_dsa_public_key != v.ml_dsa_public_key
                || claim.registry_active != v.registry_active
                || claim.liveness_met != v.liveness_met
                || claim.access_state != v.access_state
            {
                return Err(format!(
                    "observer `{oid}` disagrees with validator `{}` quorum row",
                    v.validator_id
                ));
            }
            scores.push(claim.mean_daily_participation_bps);
        }
        if agreeing_unique.len() < keyset.quorum_threshold {
            return Err(format!(
                "validator `{}` has fewer than {} agreeing observers",
                v.validator_id, keyset.quorum_threshold
            ));
        }
        let expected_mean = reconcile_quorum_bps(&scores)?;
        if v.mean_daily_participation_bps != expected_mean {
            return Err(format!(
                "validator `{}` mean_daily_participation_bps {:?} != quorum {:?}",
                v.validator_id, v.mean_daily_participation_bps, expected_mean
            ));
        }
    }

    Ok(UptimeFeedVerificationReport {
        schema: "postfiat-validator-uptime-verification-v1".to_string(),
        valid: true,
        feed_stale,
        verified_observer_count: verified_ids.len(),
        fresh_observer_count: fresh_count,
        quorum_threshold: keyset.quorum_threshold,
        verified_observer_ids: verified_ids,
        stale_observer_ids: stale_ids,
        oldest_fresh_age_seconds: oldest_fresh_age,
        allow_new_bindings: !feed_stale,
        allow_credit_grants: !feed_stale,
        revoke_existing_on_stale: false,
        finalized_height: feed.chain.finalized_height,
        validator_count: feed.validators.len(),
    })
}

fn validate_validator_access_entitlements(
    v: &ValidatorQuorumStatusV1,
    bpd: u64,
) -> Result<(), String> {
    match v.access_state.as_str() {
        "pending" => {
            if v.registry_active
                || v.first_active_height.is_some()
                || v.ramp_in_active
                || v.liveness_met
                || v.access_state_reason != "awaiting_ratification"
                || v.finance_hub_access
                || v.research_api_keys_active
                || v.reward_eligible
                || v.removal_proposal_eligible
            {
                return Err(format!(
                    "validator `{}` pending state violates Contract C2 entitlements",
                    v.validator_id
                ));
            }
        }
        "active" => {
            if !v.registry_active
                || v.first_active_height.is_none()
                || !v.liveness_met
                || !matches!(
                    v.access_state_reason.as_str(),
                    "day0_ramp_in" | "healthy_liveness" | "recovered_3day_streak"
                )
                || v.grace_started_at.is_some()
                || v.grace_expires_at.is_some()
                || v.grace_started_height.is_some()
                || v.grace_expires_height.is_some()
                || !v.finance_hub_access
                || !v.research_api_keys_active
                || !v.reward_eligible
                || v.removal_proposal_eligible
            {
                return Err(format!(
                    "validator `{}` active state violates Contract C2 entitlements",
                    v.validator_id
                ));
            }
        }
        "lapsed" => {
            let (Some(g_start), Some(g_end), Some(g_start_ts), Some(g_end_ts)) = (
                v.grace_started_height,
                v.grace_expires_height,
                v.grace_started_at.as_deref(),
                v.grace_expires_at.as_deref(),
            ) else {
                return Err(format!(
                    "validator `{}` lapsed state missing grace bounds",
                    v.validator_id
                ));
            };
            let expected_blocks = u64::from(UPTIME_GRACE_PERIOD_DAYS) * bpd;
            if g_end.saturating_sub(g_start) != expected_blocks {
                return Err(format!(
                    "validator `{}` grace height span != 14 height-days",
                    v.validator_id
                ));
            }
            let start_u = parse_rfc3339_utc_unix(g_start_ts)?;
            let end_u = parse_rfc3339_utc_unix(g_end_ts)?;
            if end_u - start_u != i64::from(UPTIME_GRACE_PERIOD_DAYS) * 86_400 {
                return Err(format!(
                    "validator `{}` grace timestamp span != 14 days",
                    v.validator_id
                ));
            }
            if v.first_active_height.is_none()
                || v.liveness_met
                || !matches!(
                    v.access_state_reason.as_str(),
                    "below_liveness_threshold"
                        | "insufficient_observed_days"
                        | "removed_from_registry"
                )
                || !v.finance_hub_access
                || !v.research_api_keys_active
                || v.reward_eligible
                || v.removal_proposal_eligible
            {
                return Err(format!(
                    "validator `{}` lapsed state violates Contract C2 entitlements",
                    v.validator_id
                ));
            }
        }
        "revoked" => {
            if v.first_active_height.is_none()
                || v.liveness_met
                || !matches!(
                    v.access_state_reason.as_str(),
                    "grace_expired" | "removed_from_registry"
                )
                || v.grace_started_at.is_some()
                || v.grace_expires_at.is_some()
                || v.grace_started_height.is_some()
                || v.grace_expires_height.is_some()
                || v.finance_hub_access
                || v.research_api_keys_active
                || v.reward_eligible
            {
                return Err(format!(
                    "validator `{}` revoked state violates Contract C2 entitlements",
                    v.validator_id
                ));
            }
        }
        other => {
            return Err(format!(
                "validator `{}` has invalid access_state `{other}`",
                v.validator_id
            ));
        }
    }

    let mut prev_day: Option<u64> = None;
    for day in &v.height_days {
        if let Some(prev) = prev_day {
            if day.day_index <= prev {
                return Err(format!(
                    "validator `{}` height_days not strictly ascending",
                    v.validator_id
                ));
            }
        }
        prev_day = Some(day.day_index);
        if day.from_height != day.day_index * bpd + 1 || day.to_height != (day.day_index + 1) * bpd
        {
            return Err(format!(
                "validator `{}` height-day {} bounds mismatch",
                v.validator_id, day.day_index
            ));
        }
        if day.precommit_qc_heights > day.credited_heights
            || day.credited_heights > day.eligible_heights
        {
            return Err(format!(
                "validator `{}` height-day {} counts out of range",
                v.validator_id, day.day_index
            ));
        }
        if day.complete && day.eligible_heights > 0 {
            let expected =
                ((day.credited_heights as u128 * 10_000) / day.eligible_heights as u128) as u32;
            if day.participation_bps != Some(expected) {
                return Err(format!(
                    "validator `{}` height-day {} participation_bps mismatch",
                    v.validator_id, day.day_index
                ));
            }
        } else if day.participation_bps.is_some() {
            return Err(format!(
                "validator `{}` incomplete height-day {} must have null participation_bps",
                v.validator_id, day.day_index
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_chain() -> UptimeFeedChainV1 {
        UptimeFeedChainV1 {
            network: "l1-testnet".to_string(),
            chain_id: "postfiat-testnet-1".to_string(),
            genesis_hash: "aa".repeat(48),
            committee_epoch: 12,
            registry_root: "bb".repeat(48),
            finalized_height: 432_000,
            finalized_block_hash: "cc".repeat(48),
        }
    }

    fn sample_window() -> UptimeWindowSpecV1 {
        UptimeWindowSpecV1 {
            metric_version: UPTIME_METRIC_VERSION.to_string(),
            credit_sources: UPTIME_CREDIT_SOURCES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            blocks_per_height_day: 14_400,
            evaluation_horizon_days: 30,
            min_observed_days: 25,
            liveness_threshold_bps: 9_500,
            grace_period_days: 14,
            recovery_consecutive_days: 3,
            window_start_height: 1,
            window_end_height: 432_000,
        }
    }

    fn make_observer(id: &str, region: &str, group: &str) -> ObserverPrivateKeyFileV1 {
        let kp = ml_dsa_65_keygen().expect("keygen");
        ObserverPrivateKeyFileV1 {
            schema: OBSERVER_PRIVATE_KEY_SCHEMA.to_string(),
            purpose: OBSERVER_KEY_PURPOSE.to_string(),
            observer_id: id.to_string(),
            region: region.to_string(),
            operator_group: group.to_string(),
            address: address_from_public_key(&kp.public_key),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&kp.public_key),
            private_key_hex: bytes_to_hex(&kp.private_key),
        }
    }

    #[test]
    fn signed_three_observer_feed_verifies_and_detects_tampering_and_staleness() {
        let obs1 = make_observer("observer-us-east-1", "us-east", "obs-a");
        let obs2 = make_observer("observer-eu-central-2", "eu-central", "obs-b");
        let obs3 = make_observer("observer-ap-northeast-3", "ap-northeast", "obs-c");
        let keyset = ObserverKeysetV1 {
            schema: OBSERVER_KEYSET_SCHEMA.to_string(),
            keyset_id: "test-obs-keyset".to_string(),
            observer_count: 3,
            quorum_threshold: 2,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            signing_context: VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT.to_string(),
            observers: [&obs1, &obs2, &obs3]
                .iter()
                .map(|o| ObserverKeysetEntryV1 {
                    observer_id: o.observer_id.clone(),
                    region: o.region.clone(),
                    operator_group: o.operator_group.clone(),
                    public_key_hex: o.public_key_hex.clone(),
                })
                .collect(),
        };
        let val_pk = bytes_to_hex(&ml_dsa_65_keygen().expect("val keygen").public_key);
        let chain = sample_chain();
        let window = sample_window();
        let observed_at = "2026-11-27T01:40:00Z";
        let make_claim = |bps: u32| {
            vec![ObserverValidatorClaimV1 {
                validator_id: "validator-0".to_string(),
                ml_dsa_public_key: val_pk.clone(),
                registry_active: true,
                observed_height_days: 30,
                mean_daily_participation_bps: Some(bps),
                consecutive_passing_days: 30,
                liveness_met: true,
                access_state: "active".to_string(),
            }]
        };
        let att1 =
            sign_observer_attestation(&obs1, &chain, &window, observed_at, make_claim(9_910))
                .expect("sign obs1");
        let att2 =
            sign_observer_attestation(&obs2, &chain, &window, observed_at, make_claim(9_900))
                .expect("sign obs2");
        let att3 =
            sign_observer_attestation(&obs3, &chain, &window, observed_at, make_claim(9_890))
                .expect("sign obs3");

        let feed = ValidatorUptimeFeedV1 {
            schema: VALIDATOR_UPTIME_FEED_SCHEMA.to_string(),
            schema_version: VALIDATOR_UPTIME_FEED_SCHEMA_VERSION,
            generated_at: "2026-11-27T01:40:10Z".to_string(),
            chain,
            quorum_policy: UptimeQuorumPolicyV1 {
                observer_count: 3,
                quorum_threshold: 2,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                signing_context: VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT.to_string(),
                stale_after_seconds: UPTIME_STALE_AFTER_SECONDS,
                max_future_skew_seconds: UPTIME_MAX_FUTURE_SKEW_SECONDS,
                stale_behavior: UPTIME_STALE_BEHAVIOR.to_string(),
            },
            window_spec: window,
            validators: vec![ValidatorQuorumStatusV1 {
                validator_id: "validator-0".to_string(),
                ml_dsa_public_key: val_pk,
                operator_identity_id: Some("hive:op-alpha-01".to_string()),
                public_rpc_url: Some("https://rpc-alpha.example.postfiat.org".to_string()),
                registry_active: true,
                first_active_height: Some(1),
                ramp_in_active: false,
                observed_height_days: 30,
                mean_daily_participation_bps: Some(9_900),
                consecutive_passing_days: 30,
                liveness_met: true,
                access_state: "active".to_string(),
                access_state_reason: "healthy_liveness".to_string(),
                grace_started_at: None,
                grace_expires_at: None,
                grace_started_height: None,
                grace_expires_height: None,
                recovery_floor_height: None,
                removal_proposal_eligible: false,
                reward_eligible: true,
                finance_hub_access: true,
                research_api_keys_active: true,
                agreeing_observer_ids: vec![
                    obs1.observer_id.clone(),
                    obs2.observer_id.clone(),
                    obs3.observer_id.clone(),
                ],
                height_days: vec![HeightDayObservationV1 {
                    day_index: 29,
                    from_height: 417_601,
                    to_height: 432_000,
                    complete: true,
                    eligible_heights: 14_400,
                    credited_heights: 14_256,
                    precommit_qc_heights: 13_950,
                    participation_bps: Some(9_900),
                }],
            }],
            observer_attestations: vec![att1, att2, att3],
        };

        let now = parse_rfc3339_utc_unix("2026-11-27T01:40:30Z").unwrap() as u64;
        let report = verify_validator_uptime_feed(&feed, &keyset, Some(now)).expect("fresh valid");
        assert!(report.valid);
        assert!(!report.feed_stale);
        assert_eq!(report.fresh_observer_count, 3);
        assert!(report.allow_new_bindings);
        assert!(report.allow_credit_grants);
        assert!(!report.revoke_existing_on_stale);

        // Stale (>3600s later): valid signature, marked stale, never revokes.
        let stale = verify_validator_uptime_feed(&feed, &keyset, Some(now + 3_700))
            .expect("stale still structurally valid");
        assert!(stale.feed_stale);
        assert!(!stale.allow_new_bindings);
        assert!(!stale.allow_credit_grants);
        assert!(!stale.revoke_existing_on_stale);

        // Tampered signature must fail closed.
        let mut tampered = feed.clone();
        let flipped = if tampered.observer_attestations[0]
            .signature_hex
            .starts_with('0')
        {
            "1"
        } else {
            "0"
        };
        tampered.observer_attestations[0]
            .signature_hex
            .replace_range(0..1, flipped);
        assert!(verify_validator_uptime_feed(&tampered, &keyset, Some(now)).is_err());

        // 2-of-3 degraded quorum uses conservative min(9910, 9900) = 9900.
        let mut degraded = feed;
        degraded.observer_attestations.pop();
        degraded.validators[0].agreeing_observer_ids.pop();
        let degraded_report =
            verify_validator_uptime_feed(&degraded, &keyset, Some(now)).expect("2-of-3 valid");
        assert_eq!(degraded_report.fresh_observer_count, 2);
        assert!(!degraded_report.feed_stale);
    }
}
