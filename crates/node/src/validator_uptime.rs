//! The signed validator uptime feed (`postfiat-validator-uptime-v1`).
//!
//! The contract is docs/specs/validator-uptime-feed-v1.md and its reference in
//! code is scripts/validator_uptime_contract.py. This module is the same
//! contract for the node: `postfiat-node top` reads a feed with it, and the
//! observer signs and checks attestations with it. A test verifies the Python
//! signed fixture here, so the two accept and reject the same feeds.

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

use crate::set_private_file_permissions;

pub const VALIDATOR_UPTIME_FEED_SCHEMA: &str = "postfiat-validator-uptime-v1";
pub const VALIDATOR_UPTIME_FEED_SCHEMA_VERSION: u32 = 1;
pub const VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT: &str = "postfiat-l1-v2/validator-uptime-feed/v1";
pub const OBSERVER_KEYSET_SCHEMA: &str = "postfiat.observer_keyset.v1";
pub const OBSERVER_PRIVATE_KEY_SCHEMA: &str = "postfiat.observer_private_key.v1";
pub const OBSERVER_KEY_PURPOSE: &str = "validator-uptime-observer";

pub const UPTIME_METRIC_VERSION: &str = "precommit-qc-observer-brackets-v1";
pub const UPTIME_TIME_BASIS: &str = "observer_brackets";
pub const UPTIME_MAX_OBSERVATION_GAP_SECONDS: u64 = 900;
pub const UPTIME_EVALUATION_HORIZON_DAYS: u32 = 30;
pub const UPTIME_MIN_OBSERVED_DAYS: u32 = 25;
pub const UPTIME_LIVENESS_THRESHOLD_BPS: u32 = 9_500;
pub const UPTIME_GRACE_PERIOD_DAYS: u32 = 14;
pub const UPTIME_RECOVERY_CONSECUTIVE_DAYS: u32 = 3;
pub const UPTIME_STALE_AFTER_SECONDS: u64 = 3_600;
pub const UPTIME_MAX_FUTURE_SKEW_SECONDS: u64 = 60;
pub const UPTIME_OBSERVER_COUNT: usize = 3;
pub const UPTIME_QUORUM_THRESHOLD: usize = 2;
pub const UPTIME_INDETERMINATE_BEHAVIOR: &str =
    "block_new_bindings_and_credits_preserve_existing_access";
const SECONDS_PER_DAY: i64 = 86_400;

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
pub struct UptimeChainV1 {
    pub network: String,
    pub chain_id: String,
    pub genesis_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverKeysetEntryV1 {
    pub observer_id: String,
    pub region: String,
    pub operator_group: String,
    pub public_key_hex: String,
}

/// The three observer keys a consumer pins. Installed out of band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverKeysetV1 {
    pub schema: String,
    pub keyset_id: String,
    pub chain: UptimeChainV1,
    pub threshold: usize,
    pub total_observers: usize,
    pub algorithm_id: String,
    pub signing_context: String,
    pub observers: Vec<ObserverKeysetEntryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptimeQuorumPolicyV1 {
    pub observer_keyset_id: String,
    pub observer_count: usize,
    pub quorum_threshold: usize,
    pub algorithm_id: String,
    pub signing_context: String,
    pub stale_after_seconds: u64,
    pub max_future_skew_seconds: u64,
    pub indeterminate_behavior: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UptimeWindowSpecV1 {
    pub metric_version: String,
    pub time_basis: String,
    pub max_observation_gap_seconds: u64,
    pub evaluation_horizon_days: u32,
    pub min_observed_days: u32,
    pub liveness_threshold_bps: u32,
    pub grace_period_days: u32,
    pub recovery_consecutive_days: u32,
    pub window_start: String,
    pub window_end: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DayObservationV1 {
    pub date: String,
    pub complete: bool,
    pub from_height: Option<u64>,
    pub to_height: Option<u64>,
    pub committed_blocks: u64,
    pub signed_blocks: u64,
    pub participation_bps: Option<u32>,
}

/// One observer's signed statement about one validator. Every field is inside
/// the observer's signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorClaimV1 {
    pub validator_id: String,
    pub ml_dsa_public_key: String,
    pub registry_active: bool,
    pub access_state: String,
    pub access_state_reason: String,
    pub first_active_date: Option<String>,
    pub ramp_in_active: bool,
    pub observed_days: u32,
    pub mean_daily_participation_bps: Option<u32>,
    pub consecutive_passing_days: u32,
    pub liveness_met: bool,
    pub grace_started_at: Option<String>,
    pub grace_expires_at: Option<String>,
    pub recovery_floor_date: Option<String>,
    pub removal_proposal_eligible: bool,
    pub reward_eligible: bool,
    pub finance_hub_access: bool,
    pub research_api_keys_active: bool,
    pub days: Vec<DayObservationV1>,
}

/// The facts an observer holds about a validator. Every other field of a
/// claim follows from these (`derive_claim`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorFactsV1 {
    pub validator_id: String,
    pub ml_dsa_public_key: String,
    pub registry_active: bool,
    pub access_state: String,
    pub first_active_date: Option<String>,
    pub grace_started_at: Option<String>,
    pub recovery_floor_date: Option<String>,
    pub days: Vec<DayObservationV1>,
}

/// A claim copied whole from `source_observer_id`, plus who agrees with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRowV1 {
    #[serde(flatten)]
    pub claim: ValidatorClaimV1,
    pub source_observer_id: String,
    pub agreeing_observer_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverAttestationV1 {
    pub observer_id: String,
    pub region: String,
    pub operator_group: String,
    pub observed_at: String,
    pub committee_epoch: u64,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub registry_root: String,
    pub validator_claims: Vec<ValidatorClaimV1>,
    pub claim_digest_sha256: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

/// Everything of an attestation but the digest, the key and the signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsignedObserverAttestationV1 {
    pub chain: UptimeChainV1,
    pub quorum_policy: UptimeQuorumPolicyV1,
    pub window_spec: UptimeWindowSpecV1,
    pub observed_at: String,
    pub committee_epoch: u64,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub registry_root: String,
    pub validator_facts: Vec<ValidatorFactsV1>,
}

/// The served `validators` array is kept as raw JSON: it is compared, byte for
/// byte in canonical form, with the rows rebuilt from the signed claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorUptimeFeedV1 {
    pub schema: String,
    pub schema_version: u32,
    pub generated_at: String,
    pub chain: UptimeChainV1,
    pub quorum_policy: UptimeQuorumPolicyV1,
    pub window_spec: UptimeWindowSpecV1,
    pub validators: Vec<serde_json::Value>,
    pub observer_attestations: Vec<ObserverAttestationV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UptimeFeedVerificationReport {
    pub verified_observers: Vec<String>,
    /// Observer id to the reason its attestation does not count.
    pub rejected_observers: BTreeMap<String, String>,
    /// Fewer than two attestations count. New bindings and new credit are
    /// blocked; nobody is revoked.
    pub indeterminate: bool,
    pub allow_new_bindings: bool,
    pub allow_credit_grants: bool,
    pub revoke_existing_access: bool,
    pub validators: Vec<ValidatorRowV1>,
    pub validators_without_quorum: Vec<String>,
    /// Observer id to the `observed_at` accepted from it; a consumer stores
    /// this and passes it as `last_accepted` next time.
    pub accepted: BTreeMap<String, String>,
    pub newest_observed_at_unix: Option<i64>,
    pub highest_finalized_height: Option<u64>,
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

// --------------------------------------------------------------------------- time

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


/// Days since 1970-01-01 to `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe as i64 + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn digits(raw: &str) -> Option<i64> {
    (!raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()))
        .then(|| raw.parse().ok())
        .flatten()
}

/// `YYYY-MM-DD` to days since 1970-01-01.
pub fn parse_utc_date(raw: &str) -> Result<i64, String> {
    let bad = || format!("`{raw}` is not a UTC date of the form YYYY-MM-DD");
    let parts: Vec<&str> = raw.split('-').collect();
    if raw.len() != 10 || parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 {
        return Err(bad());
    }
    let (year, month, day) = (
        digits(parts[0]).ok_or_else(bad)?,
        digits(parts[1]).ok_or_else(bad)?,
        digits(parts[2]).ok_or_else(bad)?,
    );
    days_from_civil(year, month as u32, day as u32).map_err(|_| bad())
}

pub fn format_utc_date(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `YYYY-MM-DDTHH:MM:SSZ` to Unix seconds. No fraction, no offset.
pub fn parse_utc_timestamp(raw: &str) -> Result<i64, String> {
    let bad = || format!("`{raw}` is not a UTC timestamp of the form YYYY-MM-DDTHH:MM:SSZ");
    if raw.len() != 20 || !raw.ends_with('Z') || raw.as_bytes()[10] != b'T' {
        return Err(bad());
    }
    let days = parse_utc_date(&raw[..10]).map_err(|_| bad())?;
    let time: Vec<&str> = raw[11..19].split(':').collect();
    if time.len() != 3 || time.iter().any(|part| part.len() != 2) {
        return Err(bad());
    }
    let (hour, minute, second) = (
        digits(time[0]).ok_or_else(bad)?,
        digits(time[1]).ok_or_else(bad)?,
        digits(time[2]).ok_or_else(bad)?,
    );
    if hour > 23 || minute > 59 || second > 59 {
        return Err(bad());
    }
    Ok(days * SECONDS_PER_DAY + hour * 3_600 + minute * 60 + second)
}

pub fn format_utc_timestamp(unix: i64) -> String {
    let days = unix.div_euclid(SECONDS_PER_DAY);
    let rest = unix.rem_euclid(SECONDS_PER_DAY);
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        format_utc_date(days),
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// A UTC midnight timestamp to its day.
fn parse_utc_midnight(raw: &str) -> Result<i64, String> {
    let unix = parse_utc_timestamp(raw)?;
    if unix.rem_euclid(SECONDS_PER_DAY) != 0 {
        return Err(format!("`{raw}` is not a UTC midnight"));
    }
    Ok(unix / SECONDS_PER_DAY)
}

// --------------------------------------------------------------------------- one validator row

fn within_days_after(start_date: Option<&String>, window_end_day: i64) -> Result<bool, String> {
    match start_date {
        None => Ok(false),
        Some(date) => {
            Ok(window_end_day < parse_utc_date(date)? + i64::from(UPTIME_EVALUATION_HORIZON_DAYS))
        }
    }
}

/// Complete days with at least one committed block. After a recovery, only
/// days from the floor on.
fn scoring_days<'a>(
    days: &'a [DayObservationV1],
    recovery_floor_date: Option<&String>,
    window_end_day: i64,
) -> Result<Vec<&'a DayObservationV1>, String> {
    let floor = match recovery_floor_date {
        Some(date) if within_days_after(recovery_floor_date, window_end_day)? => {
            Some(parse_utc_date(date)?)
        }
        _ => None,
    };
    let mut scored = Vec::new();
    for day in days {
        if !day.complete || day.committed_blocks == 0 {
            continue;
        }
        if floor.is_some_and(|floor| parse_utc_date(&day.date).is_ok_and(|d| d < floor)) {
            continue;
        }
        scored.push(day);
    }
    Ok(scored)
}

/// Build the full claim from the facts. Mirrors `derive_row` of the reference.
pub fn derive_claim(facts: &ValidatorFactsV1, window_end_day: i64) -> Result<ValidatorClaimV1, String> {
    let scored = scoring_days(&facts.days, facts.recovery_floor_date.as_ref(), window_end_day)?;
    let observed_days = scored.len() as u32;
    let mut sum = 0_u64;
    for day in &scored {
        sum += u64::from(day.participation_bps.ok_or_else(|| {
            format!("{} {}: a scored day has no participation_bps", facts.validator_id, day.date)
        })?);
    }
    let mean = (observed_days > 0).then(|| (sum / u64::from(observed_days)) as u32);
    let streak = scored
        .iter()
        .rev()
        .take_while(|day| day.participation_bps.is_some_and(|bps| bps >= UPTIME_LIVENESS_THRESHOLD_BPS))
        .count() as u32;
    let ramp_in = within_days_after(facts.first_active_date.as_ref(), window_end_day)?;
    let in_recovery = within_days_after(facts.recovery_floor_date.as_ref(), window_end_day)?;
    let liveness_met = facts.registry_active
        && (observed_days == 0 || mean.is_some_and(|mean| mean >= UPTIME_LIVENESS_THRESHOLD_BPS));

    let (reason, hub, reward, removal) = match facts.access_state.as_str() {
        "pending" => ("awaiting_ratification", false, false, false),
        "active" if ramp_in => ("day0_ramp_in", true, true, false),
        "active" if in_recovery => ("recovered_3day_streak", true, true, false),
        "active" if observed_days >= UPTIME_MIN_OBSERVED_DAYS => ("healthy_liveness", true, true, false),
        // Too few days with blocks to judge. Access stays, the credit waits.
        "active" => ("insufficient_observed_days", true, false, false),
        "lapsed" if facts.registry_active => ("below_liveness_threshold", true, false, false),
        "lapsed" => ("removed_from_registry", true, false, false),
        "revoked" if facts.registry_active => ("grace_expired", false, false, true),
        "revoked" => ("removed_from_registry", false, false, false),
        other => return Err(format!("{}: unknown access_state `{other}`", facts.validator_id)),
    };
    let grace_expires_at = match &facts.grace_started_at {
        None => None,
        Some(started) => Some(format_utc_timestamp(
            parse_utc_timestamp(started)? + i64::from(UPTIME_GRACE_PERIOD_DAYS) * SECONDS_PER_DAY,
        )),
    };
    Ok(ValidatorClaimV1 {
        validator_id: facts.validator_id.clone(),
        ml_dsa_public_key: facts.ml_dsa_public_key.clone(),
        registry_active: facts.registry_active,
        access_state: facts.access_state.clone(),
        access_state_reason: reason.to_string(),
        first_active_date: facts.first_active_date.clone(),
        ramp_in_active: ramp_in,
        observed_days,
        mean_daily_participation_bps: mean,
        consecutive_passing_days: streak,
        liveness_met,
        grace_started_at: facts.grace_started_at.clone(),
        grace_expires_at,
        recovery_floor_date: facts.recovery_floor_date.clone(),
        removal_proposal_eligible: removal,
        reward_eligible: reward,
        finance_hub_access: hub,
        research_api_keys_active: hub,
        days: facts.days.clone(),
    })
}

fn facts_of(claim: &ValidatorClaimV1) -> ValidatorFactsV1 {
    ValidatorFactsV1 {
        validator_id: claim.validator_id.clone(),
        ml_dsa_public_key: claim.ml_dsa_public_key.clone(),
        registry_active: claim.registry_active,
        access_state: claim.access_state.clone(),
        first_active_date: claim.first_active_date.clone(),
        grace_started_at: claim.grace_started_at.clone(),
        recovery_floor_date: claim.recovery_floor_date.clone(),
        days: claim.days.clone(),
    }
}

/// Refuse a signed claim that does not follow from its own facts.
pub fn check_claim(claim: &ValidatorClaimV1, window_start_day: i64, window_end_day: i64) -> Result<(), String> {
    let vid = &claim.validator_id;
    let state = claim.access_state.as_str();
    if !is_lowercase_hex_len(&claim.ml_dsa_public_key, ML_DSA_65_PUBLIC_KEY_BYTES * 2) {
        return Err(format!("{vid}: ml_dsa_public_key is not an ML-DSA-65 public key"));
    }
    if (state == "pending") != claim.first_active_date.is_none() {
        return Err(format!("{vid}: first_active_date is null exactly while pending"));
    }
    if state == "pending" && claim.registry_active {
        return Err(format!("{vid}: a pending validator is not in the registry"));
    }
    if state == "active" && !claim.registry_active {
        return Err(format!("{vid}: an active validator is in the registry"));
    }
    if (state == "lapsed") != claim.grace_started_at.is_some() {
        return Err(format!("{vid}: grace_started_at is set exactly while lapsed"));
    }
    if let Some(started) = &claim.grace_started_at {
        if parse_utc_midnight(started)? > window_end_day {
            return Err(format!("{vid}: grace starts at a UTC midnight, not after the evaluation day"));
        }
    }
    if claim.recovery_floor_date.is_some()
        && (state != "active" || !within_days_after(claim.recovery_floor_date.as_ref(), window_end_day)?)
    {
        return Err(format!("{vid}: recovery_floor_date outlived its 30 days or its active state"));
    }

    let mut expected_day = match &claim.first_active_date {
        Some(first) => parse_utc_date(first)?.max(window_start_day),
        None => window_end_day,
    };
    let mut last_to = 0_u64;
    for day in &claim.days {
        let label = format!("{vid} {}", day.date);
        if parse_utc_date(&day.date)? != expected_day || expected_day >= window_end_day {
            return Err(format!("{vid}: days must list every window day from first_active_date, once, in order"));
        }
        expected_day += 1;
        if day.signed_blocks > day.committed_blocks {
            return Err(format!("{label}: signed_blocks exceeds committed_blocks"));
        }
        match (day.from_height, day.to_height) {
            (None, None) => {
                if day.committed_blocks != 0 {
                    return Err(format!("{label}: blocks without a height range"));
                }
            }
            (Some(from), Some(to)) => {
                if from == 0 || from <= last_to || to < from {
                    return Err(format!("{label}: height ranges must ascend"));
                }
                if day.committed_blocks > to - from + 1 {
                    return Err(format!("{label}: more blocks than heights"));
                }
                last_to = to;
            }
            _ => return Err(format!("{label}: from_height and to_height are set together")),
        }
        let expected_bps = (day.complete && day.committed_blocks > 0)
            .then(|| (day.signed_blocks * 10_000 / day.committed_blocks) as u32);
        if day.participation_bps != expected_bps {
            return Err(format!(
                "{label}: participation_bps is {:?}, expected {expected_bps:?}",
                day.participation_bps
            ));
        }
    }
    if expected_day < window_end_day {
        return Err(format!("{vid}: days must list every window day from first_active_date, once, in order"));
    }

    let derived = derive_claim(&facts_of(claim), window_end_day)?;
    if derived != *claim {
        let (got, want) = (
            serde_json::to_value(claim).unwrap_or_default(),
            serde_json::to_value(&derived).unwrap_or_default(),
        );
        let field = want
            .as_object()
            .and_then(|want| want.iter().find(|(key, value)| got.get(key.as_str()) != Some(value)))
            .map_or_else(|| "a derived field".to_string(), |(key, _)| key.clone());
        return Err(format!("{vid}: {field} is not what the signed facts give"));
    }
    if state == "active" && !claim.liveness_met {
        return Err(format!("{vid}: active without liveness"));
    }
    Ok(())
}

// --------------------------------------------------------------------------- the feed

fn to_value<T: Serialize>(value: &T) -> Result<serde_json::Value, String> {
    serde_json::to_value(value).map_err(|error| format!("serialization failed: {error}"))
}

/// The exact bytes an observer signs. Everything a consumer acts on is in here.
#[allow(clippy::too_many_arguments)]
pub fn observer_claim_canonical_bytes(
    chain: &UptimeChainV1,
    quorum_policy: &UptimeQuorumPolicyV1,
    window_spec: &UptimeWindowSpecV1,
    attestation: &ObserverAttestationV1,
) -> Result<Vec<u8>, String> {
    let body = serde_json::json!({
        "chain": to_value(chain)?,
        "committee_epoch": attestation.committee_epoch,
        "finalized_block_hash": &attestation.finalized_block_hash,
        "finalized_height": attestation.finalized_height,
        "observed_at": &attestation.observed_at,
        "observer_id": &attestation.observer_id,
        "operator_group": &attestation.operator_group,
        "quorum_policy": to_value(quorum_policy)?,
        "region": &attestation.region,
        "registry_root": &attestation.registry_root,
        "schema": VALIDATOR_UPTIME_FEED_SCHEMA,
        "schema_version": VALIDATOR_UPTIME_FEED_SCHEMA_VERSION,
        "validator_claims": to_value(&attestation.validator_claims)?,
        "window_spec": to_value(window_spec)?,
    });
    canonical_json_bytes(&body)
}

fn check_policy_and_window(
    quorum_policy: &UptimeQuorumPolicyV1,
    window_spec: &UptimeWindowSpecV1,
) -> Result<(i64, i64), String> {
    if quorum_policy.observer_count != UPTIME_OBSERVER_COUNT
        || quorum_policy.quorum_threshold != UPTIME_QUORUM_THRESHOLD
        || quorum_policy.algorithm_id != ML_DSA_65_ALGORITHM
        || quorum_policy.signing_context != VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT
        || quorum_policy.stale_after_seconds != UPTIME_STALE_AFTER_SECONDS
        || quorum_policy.max_future_skew_seconds != UPTIME_MAX_FUTURE_SKEW_SECONDS
        || quorum_policy.indeterminate_behavior != UPTIME_INDETERMINATE_BEHAVIOR
    {
        return Err("schema: quorum_policy differs from the fixed policy".to_string());
    }
    if window_spec.metric_version != UPTIME_METRIC_VERSION {
        return Err(format!("schema: unknown metric_version `{}`", window_spec.metric_version));
    }
    if window_spec.time_basis != UPTIME_TIME_BASIS
        || window_spec.max_observation_gap_seconds != UPTIME_MAX_OBSERVATION_GAP_SECONDS
        || window_spec.evaluation_horizon_days != UPTIME_EVALUATION_HORIZON_DAYS
        || window_spec.min_observed_days != UPTIME_MIN_OBSERVED_DAYS
        || window_spec.liveness_threshold_bps != UPTIME_LIVENESS_THRESHOLD_BPS
        || window_spec.grace_period_days != UPTIME_GRACE_PERIOD_DAYS
        || window_spec.recovery_consecutive_days != UPTIME_RECOVERY_CONSECUTIVE_DAYS
    {
        return Err("schema: window_spec differs from the fixed numbers".to_string());
    }
    let start = parse_utc_midnight(&window_spec.window_start)?;
    let end = parse_utc_midnight(&window_spec.window_end)?;
    if end - start != i64::from(UPTIME_EVALUATION_HORIZON_DAYS) {
        return Err("the window is 30 whole UTC days".to_string());
    }
    Ok((start, end))
}

pub fn validate_observer_keyset(keyset: &ObserverKeysetV1) -> Result<(), String> {
    if keyset.schema != OBSERVER_KEYSET_SCHEMA
        || keyset.threshold != UPTIME_QUORUM_THRESHOLD
        || keyset.total_observers != UPTIME_OBSERVER_COUNT
        || keyset.observers.len() != UPTIME_OBSERVER_COUNT
        || keyset.algorithm_id != ML_DSA_65_ALGORITHM
        || keyset.signing_context != VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT
    {
        return Err("observer keyset is not a 2-of-3 ML-DSA-65 postfiat.observer_keyset.v1".to_string());
    }
    let (mut ids, mut regions, mut groups, mut keys) =
        (BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for entry in &keyset.observers {
        if !ids.insert(&entry.observer_id)
            || !regions.insert(&entry.region)
            || !groups.insert(&entry.operator_group)
            || !keys.insert(&entry.public_key_hex)
        {
            return Err(format!(
                "observer keyset: `{}` repeats an id, a region, an operator group or a key",
                entry.observer_id
            ));
        }
        let public_key = hex_to_bytes(&entry.public_key_hex)
            .map_err(|error| format!("observer `{}` bad public key hex: {error}", entry.observer_id))?;
        ml_dsa_65_validate_public_key(&public_key)
            .map_err(|error| format!("observer `{}` invalid ML-DSA-65 public key: {error}", entry.observer_id))?;
    }
    Ok(())
}

/// Why one observer's attestation must not count, or its `observed_at` in Unix seconds.
#[allow(clippy::too_many_arguments)]
fn check_attestation(
    feed: &ValidatorUptimeFeedV1,
    attestation: &ObserverAttestationV1,
    keyset: &ObserverKeysetV1,
    window: (i64, i64),
    now_unix: i64,
    last_accepted: &BTreeMap<String, String>,
) -> Result<i64, String> {
    let pinned = keyset
        .observers
        .iter()
        .find(|entry| entry.observer_id == attestation.observer_id)
        .ok_or("observer is not in the pinned keyset")?;
    if attestation.public_key_hex != pinned.public_key_hex {
        return Err("public_key_hex differs from the pinned keyset".to_string());
    }
    if attestation.region != pinned.region {
        return Err("region differs from the pinned keyset".to_string());
    }
    if attestation.operator_group != pinned.operator_group {
        return Err("operator_group differs from the pinned keyset".to_string());
    }
    if attestation.algorithm_id != ML_DSA_65_ALGORITHM
        || !is_lowercase_hex_len(&attestation.signature_hex, ML_DSA_65_SIGNATURE_BYTES * 2)
    {
        return Err("ML-DSA-65 signature does not verify".to_string());
    }
    let body = observer_claim_canonical_bytes(&feed.chain, &feed.quorum_policy, &feed.window_spec, attestation)?;
    if attestation.claim_digest_sha256 != sha256_hex(&body) {
        return Err("claim_digest_sha256 does not match the claim body".to_string());
    }
    let public_key = hex_to_bytes(&pinned.public_key_hex).map_err(|error| error.to_string())?;
    let signature = hex_to_bytes(&attestation.signature_hex).map_err(|error| error.to_string())?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &body,
        &signature,
        VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT.as_bytes(),
    ) {
        return Err("ML-DSA-65 signature does not verify".to_string());
    }

    let observed_at = parse_utc_timestamp(&attestation.observed_at)?;
    if observed_at - now_unix > UPTIME_MAX_FUTURE_SKEW_SECONDS as i64 {
        return Err("observed_at is in the future".to_string());
    }
    if now_unix - observed_at > UPTIME_STALE_AFTER_SECONDS as i64 {
        return Err("stale".to_string());
    }
    if let Some(previous) = last_accepted.get(&attestation.observer_id) {
        if observed_at < parse_utc_timestamp(previous)? {
            return Err("older than the last attestation accepted from this observer".to_string());
        }
    }
    let (window_start_day, window_end_day) = window;
    let window_end = window_end_day * SECONDS_PER_DAY;
    if observed_at < window_end || observed_at >= window_end + SECONDS_PER_DAY {
        return Err("observed_at is outside the evaluation day of this window".to_string());
    }
    if attestation.finalized_height == 0 || attestation.validator_claims.is_empty() {
        return Err("an attestation names a finalized height and at least one validator".to_string());
    }
    let mut keys = BTreeSet::new();
    for pair in attestation.validator_claims.windows(2) {
        if pair[0].validator_id >= pair[1].validator_id {
            return Err("validator_claims must be sorted by validator_id without repeats".to_string());
        }
    }
    for claim in &attestation.validator_claims {
        if !keys.insert(&claim.ml_dsa_public_key) {
            return Err("two validators share one key".to_string());
        }
        check_claim(claim, window_start_day, window_end_day)?;
    }
    Ok(observed_at)
}

/// Rows from the verified attestations, and the validators without a quorum.
/// Each row is one observer's signed claim, copied whole: with three agreeing
/// observers the one with the middle score, with two the lower score; ties go
/// to the lower observer id; an unknown score counts as the lowest.
pub fn reconcile_validator_rows(
    attestations: &[&ObserverAttestationV1],
) -> (Vec<ValidatorRowV1>, Vec<String>) {
    let mut by_validator: BTreeMap<&str, Vec<(&str, &ValidatorClaimV1)>> = BTreeMap::new();
    for attestation in attestations {
        for claim in &attestation.validator_claims {
            by_validator
                .entry(claim.validator_id.as_str())
                .or_default()
                .push((attestation.observer_id.as_str(), claim));
        }
    }
    let (mut rows, mut no_quorum) = (Vec::new(), Vec::new());
    for (validator_id, claims) in by_validator {
        let mut groups: Vec<Vec<(&str, &ValidatorClaimV1)>> = Vec::new();
        for (observer_id, claim) in claims {
            let agrees = |other: &ValidatorClaimV1| {
                other.ml_dsa_public_key == claim.ml_dsa_public_key
                    && other.registry_active == claim.registry_active
                    && other.liveness_met == claim.liveness_met
                    && other.access_state == claim.access_state
            };
            match groups.iter_mut().find(|group| agrees(group[0].1)) {
                Some(group) => group.push((observer_id, claim)),
                None => groups.push(vec![(observer_id, claim)]),
            }
        }
        let Some(mut group) = groups.into_iter().max_by_key(Vec::len).filter(|g| g.len() >= UPTIME_QUORUM_THRESHOLD)
        else {
            no_quorum.push(validator_id.to_string());
            continue;
        };
        group.sort_by_key(|(observer_id, claim)| {
            (claim.mean_daily_participation_bps.map_or(-1, i64::from), *observer_id)
        });
        let (source_observer_id, source) = if group.len() == 3 { group[1] } else { group[0] };
        let mut agreeing: Vec<String> = group.iter().map(|(observer_id, _)| (*observer_id).to_string()).collect();
        agreeing.sort();
        rows.push(ValidatorRowV1 {
            claim: source.clone(),
            source_observer_id: source_observer_id.to_string(),
            agreeing_observer_ids: agreeing,
        });
    }
    (rows, no_quorum)
}

/// Section 4 of the spec: how a consumer reads a feed. `Err` means the feed as
/// a whole is refused; a wrong observer is reported in `rejected_observers`.
pub fn verify_validator_uptime_feed(
    feed: &ValidatorUptimeFeedV1,
    keyset: &ObserverKeysetV1,
    now_unix: i64,
    last_accepted: &BTreeMap<String, String>,
) -> Result<UptimeFeedVerificationReport, String> {
    validate_observer_keyset(keyset)?;
    if feed.schema != VALIDATOR_UPTIME_FEED_SCHEMA || feed.schema_version != VALIDATOR_UPTIME_FEED_SCHEMA_VERSION {
        return Err("schema: not a postfiat-validator-uptime-v1 feed".to_string());
    }
    parse_utc_timestamp(&feed.generated_at).map_err(|error| format!("schema: {error}"))?;
    if feed.quorum_policy.observer_keyset_id != keyset.keyset_id || feed.chain != keyset.chain {
        return Err("feed is for another chain or keyset than the pinned keyset".to_string());
    }
    let window = check_policy_and_window(&feed.quorum_policy, &feed.window_spec)?;
    if feed.observer_attestations.is_empty() || feed.observer_attestations.len() > UPTIME_OBSERVER_COUNT {
        return Err("schema: a feed carries one to three attestations".to_string());
    }
    let mut seen = BTreeSet::new();
    for attestation in &feed.observer_attestations {
        if !seen.insert(&attestation.observer_id) {
            return Err(format!("observer {} appears twice", attestation.observer_id));
        }
    }

    let mut verified = Vec::new();
    let mut report = UptimeFeedVerificationReport {
        verified_observers: Vec::new(),
        rejected_observers: BTreeMap::new(),
        indeterminate: true,
        allow_new_bindings: false,
        allow_credit_grants: false,
        revoke_existing_access: false,
        validators: Vec::new(),
        validators_without_quorum: Vec::new(),
        accepted: BTreeMap::new(),
        newest_observed_at_unix: None,
        highest_finalized_height: None,
    };
    for attestation in &feed.observer_attestations {
        match check_attestation(feed, attestation, keyset, window, now_unix, last_accepted) {
            Ok(observed_at) => {
                verified.push(attestation);
                report.accepted.insert(attestation.observer_id.clone(), attestation.observed_at.clone());
                report.newest_observed_at_unix = report.newest_observed_at_unix.max(Some(observed_at));
                report.highest_finalized_height =
                    report.highest_finalized_height.max(Some(attestation.finalized_height));
            }
            Err(reason) => {
                report.rejected_observers.insert(attestation.observer_id.clone(), reason);
            }
        }
    }
    report.verified_observers = verified.iter().map(|a| a.observer_id.clone()).collect();
    report.verified_observers.sort();
    report.indeterminate = verified.len() < UPTIME_QUORUM_THRESHOLD;
    report.allow_new_bindings = !report.indeterminate;
    report.allow_credit_grants = !report.indeterminate;
    if report.indeterminate {
        return Ok(report);
    }
    let (rows, no_quorum) = reconcile_validator_rows(&verified);
    let rebuilt = canonical_json_bytes(&to_value(&rows)?)?;
    let served = canonical_json_bytes(&serde_json::Value::Array(feed.validators.clone()))?;
    if rebuilt != served {
        return Err("the validators array is not what the verified signed claims give".to_string());
    }
    report.validators = rows;
    report.validators_without_quorum = no_quorum;
    Ok(report)
}

/// Sign one observer's attestation. The claims are derived from the facts
/// here, so an observer cannot sign a row that does not follow from them.
pub fn sign_observer_attestation(
    key: &ObserverPrivateKeyFileV1,
    unsigned: &UnsignedObserverAttestationV1,
) -> Result<ObserverAttestationV1, String> {
    if key.schema != OBSERVER_PRIVATE_KEY_SCHEMA || key.purpose != OBSERVER_KEY_PURPOSE {
        return Err("not an observer key file".to_string());
    }
    let (window_start_day, window_end_day) =
        check_policy_and_window(&unsigned.quorum_policy, &unsigned.window_spec)?;
    let mut validator_claims = Vec::new();
    for facts in &unsigned.validator_facts {
        let claim = derive_claim(facts, window_end_day)?;
        check_claim(&claim, window_start_day, window_end_day)?;
        validator_claims.push(claim);
    }
    validator_claims.sort_by(|a, b| a.validator_id.cmp(&b.validator_id));
    let mut attestation = ObserverAttestationV1 {
        observer_id: key.observer_id.clone(),
        region: key.region.clone(),
        operator_group: key.operator_group.clone(),
        observed_at: unsigned.observed_at.clone(),
        committee_epoch: unsigned.committee_epoch,
        finalized_height: unsigned.finalized_height,
        finalized_block_hash: unsigned.finalized_block_hash.clone(),
        registry_root: unsigned.registry_root.clone(),
        validator_claims,
        claim_digest_sha256: String::new(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key.public_key_hex.clone(),
        signature_hex: String::new(),
    };
    parse_utc_timestamp(&attestation.observed_at)?;
    let body = observer_claim_canonical_bytes(
        &unsigned.chain,
        &unsigned.quorum_policy,
        &unsigned.window_spec,
        &attestation,
    )?;
    let private_key = hex_to_bytes(&key.private_key_hex)
        .map_err(|error| format!("observer private key hex is invalid: {error}"))?;
    let signature = ml_dsa_65_sign_with_context(
        &private_key,
        &body,
        VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT.as_bytes(),
    )
    .map_err(|error| format!("observer ML-DSA-65 sign failed: {error}"))?;
    attestation.claim_digest_sha256 = sha256_hex(&body);
    attestation.signature_hex = bytes_to_hex(&signature);
    Ok(attestation)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_NOW: &str = "2026-11-27T01:40:40Z";

    fn fixture() -> (ValidatorUptimeFeedV1, ObserverKeysetV1, i64) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/specs/schemas");
        let feed = serde_json::from_slice(
            &fs::read(dir.join("postfiat-validator-uptime-v1.fixture.json")).expect("feed fixture"),
        )
        .expect("the fixture is a feed");
        let keyset = serde_json::from_slice(
            &fs::read(dir.join("postfiat-observer-keyset-v1.fixture.json")).expect("keyset fixture"),
        )
        .expect("the fixture is a keyset");
        (feed, keyset, parse_utc_timestamp(FIXTURE_NOW).expect("now"))
    }

    fn verify(feed: &ValidatorUptimeFeedV1, keyset: &ObserverKeysetV1, now: i64) -> Result<UptimeFeedVerificationReport, String> {
        verify_validator_uptime_feed(feed, keyset, now, &BTreeMap::new())
    }

    fn rows_from(feed: &ValidatorUptimeFeedV1, skip: &str) -> Vec<serde_json::Value> {
        let kept: Vec<&ObserverAttestationV1> = feed
            .observer_attestations
            .iter()
            .filter(|a| a.observer_id != skip)
            .collect();
        let (rows, _) = reconcile_validator_rows(&kept);
        rows.iter().map(|row| serde_json::to_value(row).expect("row")).collect()
    }

    /// The fixture is signed by the Python reference with another ML-DSA-65
    /// implementation. Verifying it here is the cross-language check: same
    /// canonical bytes, same context string, same accept.
    #[test]
    fn the_python_signed_fixture_verifies_and_reads_the_same() {
        let (feed, keyset, now) = fixture();
        let report = verify(&feed, &keyset, now).expect("the fixture verifies");
        assert_eq!(
            report.verified_observers,
            ["observer-ap-northeast-3", "observer-eu-central-2", "observer-us-east-1"]
        );
        assert!(!report.indeterminate && report.allow_new_bindings && report.allow_credit_grants);
        assert!(report.rejected_observers.is_empty() && report.validators_without_quorum.is_empty());
        let states: Vec<(&str, &str, &str)> = report
            .validators
            .iter()
            .map(|row| {
                (
                    row.claim.validator_id.as_str(),
                    row.claim.access_state.as_str(),
                    row.claim.access_state_reason.as_str(),
                )
            })
            .collect();
        assert_eq!(
            states,
            [
                ("validator-0", "active", "healthy_liveness"),
                ("validator-1", "active", "day0_ramp_in"),
                ("validator-2", "active", "recovered_3day_streak"),
                ("validator-3", "lapsed", "below_liveness_threshold"),
                ("validator-4", "revoked", "grace_expired"),
                ("validator-5", "pending", "awaiting_ratification"),
            ]
        );
        assert_eq!(report.highest_finalized_height, Some(6_600));
        // Rebuilding the rows here gives the served array byte for byte.
        let rebuilt = rows_from(&feed, "nobody");
        assert_eq!(rebuilt, feed.validators);
    }

    #[test]
    fn one_wrong_observer_is_dropped_and_two_make_the_feed_indeterminate() {
        let (feed, keyset, now) = fixture();
        let first = feed.observer_attestations[0].observer_id.clone();
        let cases: Vec<(&str, Box<dyn Fn(&mut ObserverAttestationV1)>)> = vec![
            ("signature does not verify", Box::new(|a| a.signature_hex = "00".repeat(ML_DSA_65_SIGNATURE_BYTES))),
            ("claim_digest_sha256", Box::new(|a| a.claim_digest_sha256 = "0".repeat(64))),
            ("region differs", Box::new(|a| a.region = "eu-west".to_string())),
            ("claim_digest_sha256", Box::new(|a| a.validator_claims[3].access_state = "active".to_string())),
            ("claim_digest_sha256", Box::new(|a| a.validator_claims[3].removal_proposal_eligible = true)),
            ("claim_digest_sha256", Box::new(|a| a.validator_claims[3].grace_started_at = Some("2026-11-26T00:00:00Z".to_string()))),
            ("public_key_hex differs", Box::new(|a| a.public_key_hex = "ab".repeat(ML_DSA_65_PUBLIC_KEY_BYTES))),
        ];
        for (reason, mutate) in cases {
            let mut bad = feed.clone();
            mutate(&mut bad.observer_attestations[0]);
            // Served as is, the rows still name the wrong observer: refused.
            assert!(verify(&bad, &keyset, now).is_err(), "{reason}");
            // Rebuilt from the two good observers, the feed stands.
            bad.validators = rows_from(&bad, &first);
            let report = verify(&bad, &keyset, now).unwrap_or_else(|error| panic!("{reason}: {error}"));
            assert_eq!(report.verified_observers.len(), 2, "{reason}");
            assert!(report.rejected_observers[&first].contains(reason), "{reason}: {:?}", report.rejected_observers);
            assert!(!report.indeterminate && report.validators.len() == 6);
        }
        // A swapped signature.
        let mut swapped = feed.clone();
        swapped.observer_attestations[0].signature_hex = feed.observer_attestations[1].signature_hex.clone();
        swapped.validators = rows_from(&swapped, &first);
        assert!(verify(&swapped, &keyset, now).expect("two remain").rejected_observers[&first].contains("signature"));

        // Two wrong observers: indeterminate. New grants stop, nobody is revoked.
        let mut two_bad = feed.clone();
        for attestation in two_bad.observer_attestations.iter_mut().take(2) {
            attestation.signature_hex = "00".repeat(ML_DSA_65_SIGNATURE_BYTES);
        }
        let report = verify(&two_bad, &keyset, now).expect("indeterminate is not an error");
        assert!(report.indeterminate && !report.allow_new_bindings && !report.allow_credit_grants);
        assert!(!report.revoke_existing_access && report.validators.is_empty());
    }

    #[test]
    fn served_rows_time_and_schema() {
        let (feed, keyset, now) = fixture();
        // Every field of a served row is covered: change one and the feed is refused.
        for (field, value) in [
            ("removal_proposal_eligible", serde_json::json!(true)),
            ("grace_started_at", serde_json::json!("2026-11-26T00:00:00Z")),
            ("consecutive_passing_days", serde_json::json!(3)),
            ("finance_hub_access", serde_json::json!(false)),
            ("source_observer_id", serde_json::json!("observer-us-east-1")),
            ("an_added_field", serde_json::json!(1)),
        ] {
            let mut bad = feed.clone();
            bad.validators[3][field] = value;
            assert!(verify(&bad, &keyset, now).is_err(), "{field}");
        }
        let mut dropped = feed.clone();
        dropped.validators.pop();
        assert!(verify(&dropped, &keyset, now).is_err());

        // Stale, and an older feed replacing a newer one.
        let stale = verify(&feed, &keyset, now + 3_700).expect("stale is not an error");
        assert!(stale.indeterminate && stale.rejected_observers.values().all(|r| r == "stale"));
        let accepted = verify(&feed, &keyset, now).expect("ok").accepted;
        assert!(!verify_validator_uptime_feed(&feed, &keyset, now, &accepted).expect("same").indeterminate);
        let newer: BTreeMap<String, String> = accepted
            .iter()
            .map(|(id, at)| (id.clone(), format_utc_timestamp(parse_utc_timestamp(at).expect("ts") + 5)))
            .collect();
        let replay = verify_validator_uptime_feed(&feed, &keyset, now, &newer).expect("replay");
        assert!(replay.indeterminate && replay.rejected_observers.values().all(|r| r.contains("older than")));

        // The fixed numbers cannot be moved, and an unknown metric is refused.
        let mut bad = feed.clone();
        bad.window_spec.liveness_threshold_bps = 9_400;
        assert!(verify(&bad, &keyset, now).is_err());
        let mut bad = feed.clone();
        bad.window_spec.metric_version = "latency-fair-round-window-v1".to_string();
        assert!(verify(&bad, &keyset, now).expect_err("metric").contains("metric_version"));
        let mut bad = feed.clone();
        bad.quorum_policy.quorum_threshold = 1;
        assert!(verify(&bad, &keyset, now).is_err());
        let mut twice = feed.clone();
        twice.observer_attestations[1] = feed.observer_attestations[0].clone();
        assert!(verify(&twice, &keyset, now).is_err());
        let mut other_chain = keyset.clone();
        other_chain.chain.chain_id = "postfiat-other-1".to_string();
        assert!(verify(&feed, &other_chain, now).is_err());
        // An undeclared field does not parse.
        let mut raw = serde_json::to_value(&feed).expect("value");
        raw["undeclared_extra"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ValidatorUptimeFeedV1>(raw).is_err());
    }

    #[test]
    fn a_claim_must_follow_from_its_own_facts() {
        let (feed, _, _) = fixture();
        let start = parse_utc_date("2026-10-28").expect("start");
        let end = parse_utc_date("2026-11-27").expect("end");
        let claims = &feed.observer_attestations[0].validator_claims;
        for claim in claims {
            check_claim(claim, start, end).expect("fixture claims are consistent");
            assert_eq!(&derive_claim(&facts_of(claim), end).expect("derive"), claim);
        }
        let cases: Vec<(&str, usize, Box<dyn Fn(&mut ValidatorClaimV1)>)> = vec![
            ("finance_hub_access", 4, Box::new(|c| c.finance_hub_access = true)),
            ("research_api_keys_active", 4, Box::new(|c| c.research_api_keys_active = true)),
            ("reward_eligible", 3, Box::new(|c| c.reward_eligible = true)),
            ("mean_daily_participation_bps", 0, Box::new(|c| c.mean_daily_participation_bps = Some(10_000))),
            ("participation_bps", 0, Box::new(|c| c.days[0].participation_bps = Some(10_000))),
            ("days must list every window day", 0, Box::new(|c| { c.days.remove(5); })),
            ("grace_expires_at", 3, Box::new(|c| c.grace_expires_at = Some("2026-12-31T00:00:00Z".to_string()))),
            ("days must list every window day", 1, Box::new(|c| c.first_active_date = Some("2026-10-01".to_string()))),
            ("signed_blocks exceeds", 0, Box::new(|c| c.days[0].signed_blocks = 10_000)),
            ("liveness_met", 3, Box::new(|c| c.liveness_met = true)),
            ("first_active_date is null exactly while pending", 5, Box::new(|c| c.first_active_date = Some("2026-11-01".to_string()))),
        ];
        for (reason, index, mutate) in cases {
            let mut claim = claims[index].clone();
            mutate(&mut claim);
            let error = check_claim(&claim, start, end).expect_err(reason);
            assert!(error.contains(reason), "{reason}: {error}");
        }
    }

    #[test]
    fn an_attestation_signed_here_is_read_here_and_disagreement_is_settled_by_two() {
        let dir = std::env::temp_dir().join(format!("postfiat-uptime-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (template, _, _) = fixture();
        let chain = template.chain.clone();
        let mut quorum_policy = template.quorum_policy.clone();
        quorum_policy.observer_keyset_id = "postfiat-observer-keyset-rust-test".to_string();
        let observers = [("observer-a-1", "us-east", "group-a"), ("observer-b-2", "eu-central", "group-b"), ("observer-c-3", "ap-northeast", "group-c")];
        let keys: Vec<ObserverPrivateKeyFileV1> = observers
            .iter()
            .map(|(id, region, group)| {
                create_observer_key_file(&dir.join(format!("{id}.json")), id, region, group).expect("key")
            })
            .collect();
        let keyset = ObserverKeysetV1 {
            schema: OBSERVER_KEYSET_SCHEMA.to_string(),
            keyset_id: quorum_policy.observer_keyset_id.clone(),
            chain: chain.clone(),
            threshold: 2,
            total_observers: 3,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            signing_context: VALIDATOR_UPTIME_FEED_SIGNING_CONTEXT.to_string(),
            observers: keys
                .iter()
                .map(|key| ObserverKeysetEntryV1 {
                    observer_id: key.observer_id.clone(),
                    region: key.region.clone(),
                    operator_group: key.operator_group.clone(),
                    public_key_hex: key.public_key_hex.clone(),
                })
                .collect(),
        };
        validate_observer_keyset(&keyset).expect("keyset");
        // The facts of the fixture's six validators, signed again by keys made here.
        let facts: Vec<ValidatorFactsV1> =
            template.observer_attestations[0].validator_claims.iter().map(facts_of).collect();
        let unsigned = |facts: Vec<ValidatorFactsV1>| UnsignedObserverAttestationV1 {
            chain: chain.clone(),
            quorum_policy: quorum_policy.clone(),
            window_spec: template.window_spec.clone(),
            observed_at: "2026-11-27T01:39:00Z".to_string(),
            committee_epoch: 12,
            finalized_height: 6_600,
            finalized_block_hash: template.observer_attestations[0].finalized_block_hash.clone(),
            registry_root: template.observer_attestations[0].registry_root.clone(),
            validator_facts: facts,
        };
        // Observer A says validator-3 is already revoked; B and C say lapsed.
        let mut dissent = facts.clone();
        dissent[3].access_state = "revoked".to_string();
        dissent[3].grace_started_at = None;
        let attestations = vec![
            sign_observer_attestation(&keys[0], &unsigned(dissent)).expect("sign a"),
            sign_observer_attestation(&keys[1], &unsigned(facts.clone())).expect("sign b"),
            sign_observer_attestation(&keys[2], &unsigned(facts.clone())).expect("sign c"),
        ];
        let refs: Vec<&ObserverAttestationV1> = attestations.iter().collect();
        let (rows, no_quorum) = reconcile_validator_rows(&refs);
        assert!(no_quorum.is_empty());
        let mut feed = ValidatorUptimeFeedV1 {
            schema: VALIDATOR_UPTIME_FEED_SCHEMA.to_string(),
            schema_version: 1,
            generated_at: "2026-11-27T01:40:00Z".to_string(),
            chain: chain.clone(),
            quorum_policy: quorum_policy.clone(),
            window_spec: template.window_spec.clone(),
            validators: rows.iter().map(|row| serde_json::to_value(row).expect("row")).collect(),
            observer_attestations: attestations,
        };
        let now = parse_utc_timestamp(FIXTURE_NOW).expect("now");
        let report = verify(&feed, &keyset, now).expect("a feed signed here verifies here");
        assert_eq!(report.verified_observers.len(), 3);
        let row = &report.validators[3];
        assert_eq!(row.claim.access_state, "lapsed");
        assert_eq!(row.agreeing_observer_ids, ["observer-b-2", "observer-c-3"]);
        assert_eq!(report.validators[0].agreeing_observer_ids.len(), 3);

        // With only A and B, validator-3 has no quorum; the other five are unaffected.
        feed.observer_attestations.truncate(2);
        let refs: Vec<&ObserverAttestationV1> = feed.observer_attestations.iter().collect();
        let (rows, no_quorum) = reconcile_validator_rows(&refs);
        assert_eq!(no_quorum, ["validator-3"]);
        feed.validators = rows.iter().map(|row| serde_json::to_value(row).expect("row")).collect();
        let report = verify(&feed, &keyset, now).expect("two observers");
        assert_eq!(report.validators_without_quorum, ["validator-3"]);
        assert_eq!(report.validators.len(), 5);

        // An observer cannot sign facts that contradict themselves.
        let mut nonsense = facts;
        nonsense[0].access_state = "pending".to_string();
        assert!(sign_observer_attestation(&keys[0], &unsigned(nonsense)).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dates_round_trip() {
        for raw in ["1970-01-01", "2000-02-29", "2026-11-27", "2026-12-31", "2027-01-01", "2100-03-01"] {
            assert_eq!(format_utc_date(parse_utc_date(raw).expect(raw)), raw);
        }
        assert_eq!(parse_utc_date("1970-01-02"), Ok(1));
        for bad in ["2026-02-30", "2026-13-01", "26-11-27", "2026-11-27T", "2026/11/27", "2100-02-29"] {
            assert!(parse_utc_date(bad).is_err(), "{bad}");
        }
        let unix = parse_utc_timestamp("2026-11-27T01:40:40Z").expect("timestamp");
        assert_eq!(format_utc_timestamp(unix), "2026-11-27T01:40:40Z");
        assert_eq!(format_utc_timestamp(parse_utc_timestamp("2026-11-24T00:00:00Z").expect("ts") + 14 * 86_400), "2026-12-08T00:00:00Z");
        for bad in ["2026-11-27T01:40:40.5Z", "2026-11-27T01:40:40+00:00", "2026-11-27 01:40:40Z", "2026-11-27T24:00:00Z"] {
            assert!(parse_utc_timestamp(bad).is_err(), "{bad}");
        }
        assert!(parse_utc_midnight("2026-11-27T00:00:01Z").is_err());
    }
}
