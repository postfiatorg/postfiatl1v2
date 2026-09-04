//! Deterministic collection-completeness logic for private YOLO option-chain inputs.
//!
//! This module intentionally performs no strike selection, weighting, valuation,
//! trading, or reserve accounting. Nitro attestation is verified outside this
//! guest; its verified aggregate is bound into these public values.

use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};
use postfiat_types::{
    YoloCollectionPublicValuesV1, YOLO_COLLECTION_MAX_SNAPSHOTS_V1,
    YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const YOLO_COLLECTION_WITNESS_SCHEMA_V1: &str = "postfiat.yolo.collection_proof_witness.v1";
pub const YOLO_QUERY_SCHEMA_V1: &str = "postfiat.yolo.option_chain_query.v1";
pub const YOLO_EPOCH_SCHEMA_V2: &str = "postfiat.yolo.collection_epoch.v2";
pub const YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2: &str = "postfiat.yolo.snapshot_commitment.v2";
pub const YOLO_NORMALIZED_INPUT_ROOT_SCHEMA_V2: &str = "postfiat.yolo.normalized_input_root.v2";
pub const YOLO_MAX_COLLECTION_WITNESS_BYTES_V1: usize = 2 * 1024 * 1024;

const COLLECTION_COMMITMENTS_DOMAIN_V1: &str = "postfiat.yolo.collection_commitments.v1";
const SOURCE_ID_DOMAIN_V1: &str = "postfiat.yolo.source_id.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloOptionChainQueryV1 {
    pub schema: String,
    pub symbol: String,
    pub contract_type: String,
    pub strategy: String,
    /// Python's committed query schema encodes this wire parameter as text.
    pub include_underlying_quote: String,
    pub from_date: String,
    pub to_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloCollectionEpochV2 {
    pub schema: String,
    pub epoch_id: String,
    pub methodology_sha256: String,
    pub underlier: String,
    pub source_id: String,
    pub account_application_identity_sha256: String,
    pub collector_code_sha256: String,
    pub prior_epoch_sha256: Option<String>,
    pub required_response_fields: Vec<String>,
    pub query: YoloOptionChainQueryV1,
    pub window_start_utc: String,
    pub window_end_utc: String,
    pub expected_snapshot_count: u32,
    pub snapshot_offsets_seconds: Vec<u64>,
    pub scheduled_at_utc: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloSnapshotCommitmentV2 {
    pub schema: String,
    pub epoch_sha256: String,
    pub sequence: u32,
    pub scheduled_at_utc: String,
    pub observed_at_utc: String,
    pub source_id: String,
    pub http_status: u16,
    pub response_date: Option<String>,
    pub source_payload_sha256: String,
    pub raw_response_sha256: String,
    pub response_coverage_sha256: String,
    pub contracts_sha256: String,
    pub flattened_contract_count: u64,
    pub reported_contract_count: Option<u64>,
    pub previous_snapshot_sha256: Option<String>,
    pub private_snapshot_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YoloCollectionProofWitnessV1 {
    pub schema: String,
    pub program_sha256: String,
    pub attested_collection_sha256: String,
    pub epoch: YoloCollectionEpochV2,
    pub commitments: Vec<YoloSnapshotCommitmentV2>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NormalizedInputRootV2<'a> {
    schema: &'static str,
    epoch_sha256: &'a str,
    snapshots: Vec<NormalizedSnapshotV2<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NormalizedSnapshotV2<'a> {
    sequence: u32,
    source_payload_sha256: &'a str,
    response_coverage_sha256: &'a str,
    contracts_sha256: &'a str,
}

pub fn execute_yolo_collection_proof(
    witness: &YoloCollectionProofWitnessV1,
) -> Result<YoloCollectionPublicValuesV1, String> {
    if witness.schema != YOLO_COLLECTION_WITNESS_SCHEMA_V1 {
        return Err("YOLO collection witness schema mismatch".to_string());
    }
    validate_digest("program_sha256", &witness.program_sha256)?;
    validate_digest(
        "attested_collection_sha256",
        &witness.attested_collection_sha256,
    )?;
    validate_epoch(&witness.epoch)?;
    if witness.commitments.len() != witness.epoch.expected_snapshot_count as usize {
        return Err("YOLO collection is not complete".to_string());
    }

    let epoch_sha256 = domain_sha256(YOLO_EPOCH_SCHEMA_V2, &witness.epoch)?;
    let mut ordered: Vec<&YoloSnapshotCommitmentV2> = witness.commitments.iter().collect();
    ordered.sort_by_key(|commitment| commitment.sequence);
    for (expected_sequence, commitment) in ordered.iter().enumerate() {
        validate_commitment(commitment)?;
        if commitment.sequence as usize != expected_sequence {
            return Err("YOLO snapshot sequences must be complete and unique".to_string());
        }
        if commitment.epoch_sha256 != epoch_sha256 {
            return Err("YOLO snapshot epoch does not match the manifest".to_string());
        }
        if commitment.source_id != witness.epoch.source_id {
            return Err("YOLO snapshot source does not match the manifest".to_string());
        }
        if commitment.scheduled_at_utc != witness.epoch.scheduled_at_utc[expected_sequence] {
            return Err("YOLO snapshot time does not match the committed schedule".to_string());
        }
        if !(200..=299).contains(&commitment.http_status) {
            return Err("YOLO snapshot HTTP response was not successful".to_string());
        }
        if let Some(reported) = commitment.reported_contract_count {
            if reported != commitment.flattened_contract_count {
                return Err("YOLO snapshot contract count does not match".to_string());
            }
        }
        let expected_previous = if expected_sequence == 0 {
            None
        } else {
            Some(domain_sha256(
                YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2,
                ordered[expected_sequence - 1],
            )?)
        };
        if commitment.previous_snapshot_sha256.as_ref() != expected_previous.as_ref() {
            return Err("YOLO snapshot commitment chain is broken".to_string());
        }
    }

    let ordered_values: Vec<&YoloSnapshotCommitmentV2> = ordered.clone();
    let commitments_sha256 = domain_sha256(COLLECTION_COMMITMENTS_DOMAIN_V1, &ordered_values)?;
    let normalized = NormalizedInputRootV2 {
        schema: YOLO_NORMALIZED_INPUT_ROOT_SCHEMA_V2,
        epoch_sha256: &epoch_sha256,
        snapshots: ordered
            .iter()
            .map(|commitment| NormalizedSnapshotV2 {
                sequence: commitment.sequence,
                source_payload_sha256: &commitment.source_payload_sha256,
                response_coverage_sha256: &commitment.response_coverage_sha256,
                contracts_sha256: &commitment.contracts_sha256,
            })
            .collect(),
    };
    let normalized_input_root_sha256 =
        domain_sha256(YOLO_NORMALIZED_INPUT_ROOT_SCHEMA_V2, &normalized)?;

    let public_values = YoloCollectionPublicValuesV1 {
        schema: YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        program_sha256: witness.program_sha256.clone(),
        epoch_sha256,
        methodology_sha256: witness.epoch.methodology_sha256.clone(),
        collector_code_sha256: witness.epoch.collector_code_sha256.clone(),
        source_id_sha256: domain_sha256(SOURCE_ID_DOMAIN_V1, &witness.epoch.source_id)?,
        account_application_identity_sha256: witness
            .epoch
            .account_application_identity_sha256
            .clone(),
        commitments_sha256,
        attested_collection_sha256: witness.attested_collection_sha256.clone(),
        normalized_input_root_sha256,
        snapshot_count: witness.epoch.expected_snapshot_count,
    };
    public_values.validate()?;
    Ok(public_values)
}

fn validate_epoch(epoch: &YoloCollectionEpochV2) -> Result<(), String> {
    if epoch.schema != YOLO_EPOCH_SCHEMA_V2 {
        return Err("YOLO collection epoch schema mismatch".to_string());
    }
    for (name, value) in [
        ("epoch_id", &epoch.epoch_id),
        ("underlier", &epoch.underlier),
        ("source_id", &epoch.source_id),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{name} must not be empty"));
        }
    }
    for (name, value) in [
        ("methodology_sha256", &epoch.methodology_sha256),
        (
            "account_application_identity_sha256",
            &epoch.account_application_identity_sha256,
        ),
        ("collector_code_sha256", &epoch.collector_code_sha256),
    ] {
        validate_digest(name, value)?;
    }
    if let Some(prior) = &epoch.prior_epoch_sha256 {
        validate_digest("prior_epoch_sha256", prior)?;
    }
    validate_query(&epoch.query)?;
    if epoch.underlier != epoch.underlier.trim().to_uppercase()
        || epoch.underlier != epoch.query.symbol
    {
        return Err("YOLO underlier must be canonical and equal the query symbol".to_string());
    }
    if epoch.required_response_fields.is_empty() {
        return Err("YOLO required response fields must not be empty".to_string());
    }
    let required: BTreeSet<&String> = epoch.required_response_fields.iter().collect();
    if required.len() != epoch.required_response_fields.len()
        || required
            .iter()
            .copied()
            .ne(epoch.required_response_fields.iter())
        || epoch
            .required_response_fields
            .iter()
            .any(|field| field.is_empty() || field.chars().any(char::is_whitespace))
    {
        return Err("YOLO required response fields must be sorted, unique, and valid".to_string());
    }
    if epoch.expected_snapshot_count == 0
        || epoch.expected_snapshot_count > YOLO_COLLECTION_MAX_SNAPSHOTS_V1
        || epoch.snapshot_offsets_seconds.len() != epoch.expected_snapshot_count as usize
        || epoch.scheduled_at_utc.len() != epoch.expected_snapshot_count as usize
    {
        return Err("YOLO snapshot schedule count is invalid".to_string());
    }
    let start = parse_python_utc("windowStartUtc", &epoch.window_start_utc)?;
    let end = parse_python_utc("windowEndUtc", &epoch.window_end_utc)?;
    if start > end {
        return Err("YOLO collection window is inverted".to_string());
    }
    let mut previous_offset = None;
    for (index, offset) in epoch.snapshot_offsets_seconds.iter().enumerate() {
        if previous_offset.is_some_and(|previous| previous >= *offset) {
            return Err("YOLO snapshot offsets must be chronological and unique".to_string());
        }
        previous_offset = Some(*offset);
        let seconds = i64::try_from(*offset)
            .map_err(|_| "YOLO snapshot offset exceeds timestamp range".to_string())?;
        let scheduled = start
            .checked_add_signed(Duration::seconds(seconds))
            .ok_or_else(|| "YOLO scheduled timestamp overflows".to_string())?;
        if scheduled > end {
            return Err("YOLO snapshot offset falls outside the window".to_string());
        }
        let supplied = parse_python_utc("scheduledAtUtc", &epoch.scheduled_at_utc[index])?;
        if supplied != scheduled {
            return Err("YOLO scheduled timestamps do not match their offsets".to_string());
        }
    }
    Ok(())
}

fn validate_query(query: &YoloOptionChainQueryV1) -> Result<(), String> {
    if query.schema != YOLO_QUERY_SCHEMA_V1 {
        return Err("YOLO option-chain query schema mismatch".to_string());
    }
    if query.symbol.is_empty()
        || query.symbol != query.symbol.trim().to_uppercase()
        || query.strategy.is_empty()
        || query.strategy != query.strategy.trim().to_uppercase()
        || !matches!(query.contract_type.as_str(), "ALL" | "CALL" | "PUT")
        || !matches!(query.include_underlying_quote.as_str(), "true" | "false")
    {
        return Err("YOLO option-chain query is not canonical".to_string());
    }
    let from = parse_date("fromDate", &query.from_date)?;
    let to = parse_date("toDate", &query.to_date)?;
    if from > to {
        return Err("YOLO option-chain date interval is inverted".to_string());
    }
    if query.strike_count == Some(0) {
        return Err("YOLO strike count must be positive when supplied".to_string());
    }
    Ok(())
}

fn validate_commitment(commitment: &YoloSnapshotCommitmentV2) -> Result<(), String> {
    if commitment.schema != YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2 {
        return Err("YOLO snapshot commitment schema mismatch".to_string());
    }
    for (name, value) in [
        ("epoch_sha256", &commitment.epoch_sha256),
        ("source_payload_sha256", &commitment.source_payload_sha256),
        ("raw_response_sha256", &commitment.raw_response_sha256),
        (
            "response_coverage_sha256",
            &commitment.response_coverage_sha256,
        ),
        ("contracts_sha256", &commitment.contracts_sha256),
        (
            "private_snapshot_sha256",
            &commitment.private_snapshot_sha256,
        ),
    ] {
        validate_digest(name, value)?;
    }
    if let Some(previous) = &commitment.previous_snapshot_sha256 {
        validate_digest("previous_snapshot_sha256", previous)?;
    }
    parse_python_utc("scheduledAtUtc", &commitment.scheduled_at_utc)?;
    parse_python_utc("observedAtUtc", &commitment.observed_at_utc)?;
    if commitment.source_id.trim().is_empty() || !(100..=599).contains(&commitment.http_status) {
        return Err("YOLO snapshot metadata is invalid".to_string());
    }
    Ok(())
}

fn parse_date(name: &str, value: &str) -> Result<NaiveDate, String> {
    let parsed = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("{name} must be a canonical ISO-8601 date"))?;
    if parsed.format("%Y-%m-%d").to_string() != value {
        return Err(format!("{name} must be a canonical ISO-8601 date"));
    }
    Ok(parsed)
}

fn parse_python_utc(name: &str, value: &str) -> Result<DateTime<Utc>, String> {
    if !value.ends_with('Z') {
        return Err(format!("{name} must use canonical UTC Z form"));
    }
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|_| format!("{name} must be an ISO-8601 timestamp"))?
        .with_timezone(&Utc);
    let nanos = parsed.timestamp_subsec_nanos();
    let canonical = if nanos == 0 {
        parsed.to_rfc3339_opts(SecondsFormat::Secs, true)
    } else {
        if nanos % 1_000 != 0 {
            return Err(format!("{name} exceeds Python microsecond precision"));
        }
        parsed.to_rfc3339_opts(SecondsFormat::Micros, true)
    };
    if canonical != value {
        return Err(format!("{name} must use canonical UTC Z form"));
    }
    Ok(parsed)
}

fn validate_digest(name: &str, value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{name} must be a lowercase SHA-256 digest"));
    }
    Ok(())
}

fn domain_sha256<T: Serialize>(domain: &str, value: &T) -> Result<String, String> {
    if domain.is_empty() || domain.as_bytes().contains(&0) {
        return Err("YOLO commitment domain is invalid".to_string());
    }
    let value = serde_json::to_value(value)
        .map_err(|_| "YOLO canonical JSON serialization failed".to_string())?;
    let mut canonical = Vec::new();
    write_canonical_json(&value, &mut canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(canonical);
    Ok(hex::encode(hasher.finalize()))
}

fn write_canonical_json(value: &Value, output: &mut Vec<u8>) -> Result<(), String> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(text) => output.extend_from_slice(
            serde_json::to_string(text)
                .map_err(|_| "YOLO JSON string serialization failed".to_string())?
                .as_bytes(),
        ),
        Value::Array(items) => {
            output.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical_json(item, output)?;
            }
            output.push(b']');
        }
        Value::Object(object) => {
            output.push(b'{');
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                output.extend_from_slice(
                    serde_json::to_string(key)
                        .map_err(|_| "YOLO JSON key serialization failed".to_string())?
                        .as_bytes(),
                );
                output.push(b':');
                write_canonical_json(&object[*key], output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch() -> YoloCollectionEpochV2 {
        YoloCollectionEpochV2 {
            schema: YOLO_EPOCH_SCHEMA_V2.to_string(),
            epoch_id: "hood-20260904-a".to_string(),
            methodology_sha256: "aa".repeat(32),
            underlier: "HOOD".to_string(),
            source_id: "schwab-trader-api-individual".to_string(),
            account_application_identity_sha256: "bb".repeat(32),
            collector_code_sha256: "cc".repeat(32),
            prior_epoch_sha256: None,
            required_response_fields: vec!["callExpDateMap".to_string(), "symbol".to_string()],
            query: YoloOptionChainQueryV1 {
                schema: YOLO_QUERY_SCHEMA_V1.to_string(),
                symbol: "HOOD".to_string(),
                contract_type: "ALL".to_string(),
                strategy: "SINGLE".to_string(),
                include_underlying_quote: "true".to_string(),
                from_date: "2026-09-04".to_string(),
                to_date: "2026-10-16".to_string(),
                strike_count: None,
            },
            window_start_utc: "2026-09-04T14:00:00Z".to_string(),
            window_end_utc: "2026-09-04T14:01:00Z".to_string(),
            expected_snapshot_count: 2,
            snapshot_offsets_seconds: vec![0, 60],
            scheduled_at_utc: vec![
                "2026-09-04T14:00:00Z".to_string(),
                "2026-09-04T14:01:00Z".to_string(),
            ],
        }
    }

    fn commitment(epoch_sha256: &str, sequence: u32) -> YoloSnapshotCommitmentV2 {
        YoloSnapshotCommitmentV2 {
            schema: YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2.to_string(),
            epoch_sha256: epoch_sha256.to_string(),
            sequence,
            scheduled_at_utc: format!("2026-09-04T14:0{sequence}:00Z"),
            observed_at_utc: format!("2026-09-04T14:0{sequence}:01Z"),
            source_id: "schwab-trader-api-individual".to_string(),
            http_status: 200,
            response_date: None,
            source_payload_sha256: "11".repeat(32),
            raw_response_sha256: "22".repeat(32),
            response_coverage_sha256: "33".repeat(32),
            contracts_sha256: "44".repeat(32),
            flattened_contract_count: 3,
            reported_contract_count: Some(3),
            previous_snapshot_sha256: None,
            private_snapshot_sha256: "55".repeat(32),
        }
    }

    fn witness() -> YoloCollectionProofWitnessV1 {
        let epoch = epoch();
        let epoch_sha256 = domain_sha256(YOLO_EPOCH_SCHEMA_V2, &epoch).unwrap();
        let first = commitment(&epoch_sha256, 0);
        let mut second = commitment(&epoch_sha256, 1);
        second.previous_snapshot_sha256 =
            Some(domain_sha256(YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2, &first).unwrap());
        YoloCollectionProofWitnessV1 {
            schema: YOLO_COLLECTION_WITNESS_SCHEMA_V1.to_string(),
            program_sha256: "66".repeat(32),
            attested_collection_sha256: "77".repeat(32),
            epoch,
            commitments: vec![second, first],
        }
    }

    #[test]
    fn executes_complete_collection_without_private_payloads() {
        let values = execute_yolo_collection_proof(&witness()).unwrap();
        assert_eq!(values.snapshot_count, 2);
        assert_eq!(values.methodology_sha256, "aa".repeat(32));
        assert_eq!(values.encode().unwrap().len(), 304);
        let serialized = serde_json::to_string(&values).unwrap();
        assert!(!serialized.contains("callExpDateMap"));
    }

    #[test]
    fn rejects_broken_chain_and_unapproved_extra_schedule() {
        let mut broken = witness();
        broken.commitments[0].previous_snapshot_sha256 = Some("88".repeat(32));
        assert!(execute_yolo_collection_proof(&broken).is_err());

        let mut extra = witness();
        extra.epoch.expected_snapshot_count = 3;
        assert!(execute_yolo_collection_proof(&extra).is_err());
    }

    #[test]
    fn canonical_json_sorts_keys_and_preserves_utf8() {
        let digest = domain_sha256("test", &serde_json::json!({"z": "é", "a": 1})).unwrap();
        let expected = Sha256::digest("test\0{\"a\":1,\"z\":\"é\"}".as_bytes());
        assert_eq!(digest, hex::encode(expected));
    }

    #[test]
    fn matches_python_collector_golden_vector() {
        let epoch = YoloCollectionEpochV2 {
            schema: YOLO_EPOCH_SCHEMA_V2.to_string(),
            epoch_id: "hood-20260904-a".to_string(),
            methodology_sha256: "aa".repeat(32),
            underlier: "HOOD".to_string(),
            source_id: "schwab-trader-api-individual".to_string(),
            account_application_identity_sha256: "bb".repeat(32),
            collector_code_sha256: "cc".repeat(32),
            prior_epoch_sha256: None,
            required_response_fields: vec![
                "callExpDateMap".to_string(),
                "numberOfContracts".to_string(),
                "putExpDateMap".to_string(),
                "symbol".to_string(),
            ],
            query: YoloOptionChainQueryV1 {
                schema: YOLO_QUERY_SCHEMA_V1.to_string(),
                symbol: "HOOD".to_string(),
                contract_type: "ALL".to_string(),
                strategy: "SINGLE".to_string(),
                include_underlying_quote: "true".to_string(),
                from_date: "2026-09-04".to_string(),
                to_date: "2026-10-16".to_string(),
                strike_count: None,
            },
            window_start_utc: "2026-09-04T14:00:00Z".to_string(),
            window_end_utc: "2026-09-04T14:01:00Z".to_string(),
            expected_snapshot_count: 2,
            snapshot_offsets_seconds: vec![0, 60],
            scheduled_at_utc: vec![
                "2026-09-04T14:00:00Z".to_string(),
                "2026-09-04T14:01:00Z".to_string(),
            ],
        };
        let epoch_sha256 = "47b0c73713509d70a6eb5fc6f5199505af837769102c49c55c6dc04ae42c6ac4";
        assert_eq!(
            domain_sha256(YOLO_EPOCH_SCHEMA_V2, &epoch).unwrap(),
            epoch_sha256
        );
        let first = YoloSnapshotCommitmentV2 {
            schema: YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2.to_string(),
            epoch_sha256: epoch_sha256.to_string(),
            sequence: 0,
            scheduled_at_utc: "2026-09-04T14:00:00Z".to_string(),
            observed_at_utc: "2026-09-04T14:00:01Z".to_string(),
            source_id: "schwab-trader-api-individual".to_string(),
            http_status: 200,
            response_date: Some("Fri, 04 Sep 2026 14:00:00 GMT".to_string()),
            source_payload_sha256:
                "23b03e7b6dfbffd4de99dabab81f51a04e8fccc65c8308aa5d9c89bd06f09fe7".to_string(),
            raw_response_sha256: "81208ea190f2dfc90ce02de51b8c14d12be74cce504a23b1bf2683291b7aaa50"
                .to_string(),
            response_coverage_sha256:
                "1382a559b3c7e542e8fcf7676d6a814c8069b75e6d84f0fa237ab6983535493f".to_string(),
            contracts_sha256: "93b6ad82ef89af376011e244358502073e8571e7e5fc5598217d82d2a0d69183"
                .to_string(),
            flattened_contract_count: 3,
            reported_contract_count: Some(3),
            previous_snapshot_sha256: None,
            private_snapshot_sha256:
                "a15202ee9b4e31221b966dbbb10f05a7ec26baefadf5cf1d8b612eeb11f1da75".to_string(),
        };
        assert_eq!(
            domain_sha256(YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2, &first).unwrap(),
            "7e73b8bb639e67e94fc45666e545abf59c750acb38dac6dc8eaa2ed593fa2347"
        );
        let mut second = first.clone();
        second.sequence = 1;
        second.scheduled_at_utc = "2026-09-04T14:01:00Z".to_string();
        second.previous_snapshot_sha256 =
            Some("7e73b8bb639e67e94fc45666e545abf59c750acb38dac6dc8eaa2ed593fa2347".to_string());
        second.private_snapshot_sha256 =
            "f12c819c5556272c989cb9c74d07e05b1244d4242d57fbf35f567087d7bce1c1".to_string();
        let values = execute_yolo_collection_proof(&YoloCollectionProofWitnessV1 {
            schema: YOLO_COLLECTION_WITNESS_SCHEMA_V1.to_string(),
            program_sha256: "ee".repeat(32),
            attested_collection_sha256: "ff".repeat(32),
            epoch,
            commitments: vec![second, first],
        })
        .unwrap();
        assert_eq!(
            values.commitments_sha256,
            "284c34e182dfb3940ed909ff2a73d75fc1e3f099de4ece4a52e4e81cfb216e8a"
        );
        assert_eq!(
            values.normalized_input_root_sha256,
            "5a4666278c3bc790a72d428f475856ed8a0dc1df6c328dd08a75f094b5a4dbde"
        );
        assert_eq!(
            values.source_id_sha256,
            "38818578bf56386b9ed859437a7f495775a47e6f7f7b267a7f31508fbfc48cb8"
        );
    }
}
