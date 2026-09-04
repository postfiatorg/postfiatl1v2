pub const YOLO_TARGET_PUBLIC_VALUES_SCHEMA_V1: &str = "postfiat.yolo.target_public_values.v1";
pub const YOLO_TARGET_PUBLIC_VALUES_MAGIC_V1: &[u8; 8] = b"PFTYTGT1";
pub const YOLO_TARGET_PUBLIC_VALUES_VERSION_V1: u32 = 1;
pub const YOLO_TARGET_PUBLIC_VALUES_V1_BYTES: usize = 408;
pub const YOLO_TARGET_MAX_SNAPSHOTS_V1: u32 = 64;
pub const YOLO_TARGET_MAX_SELECTED_CONTRACTS_V1: u32 = 64;

/// Methodology result encoded in the target proof's consensus public values.
///
/// Codes are append-only. A different meaning requires a new ABI version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum YoloTargetStatusV1 {
    TargetComputed,
    NoReconstitution,
    NoEligibleSuccessor,
    HaltedInstrument,
    UnresolvedCapitalPolicy,
}

impl YoloTargetStatusV1 {
    pub fn code(self) -> u32 {
        match self {
            Self::TargetComputed => 1,
            Self::NoReconstitution => 2,
            Self::NoEligibleSuccessor => 3,
            Self::HaltedInstrument => 4,
            Self::UnresolvedCapitalPolicy => 5,
        }
    }

    pub fn from_code(code: u32) -> Result<Self, String> {
        match code {
            1 => Ok(Self::TargetComputed),
            2 => Ok(Self::NoReconstitution),
            3 => Ok(Self::NoEligibleSuccessor),
            4 => Ok(Self::HaltedInstrument),
            5 => Ok(Self::UnresolvedCapitalPolicy),
            _ => Err("unknown YOLO target status code".to_string()),
        }
    }
}

/// Fixed-width, disclosure-safe output of the YOLO target SP1 program.
///
/// This payload commits to a calculated target and its provenance. It has no
/// transaction semantics and cannot authorize broker orders, reserve claims,
/// issuance, redemption, or capital movement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloTargetPublicValuesV1 {
    pub schema: String,
    pub program_sha256: String,
    pub methodology_sha256: String,
    pub parameter_manifest_sha256: String,
    pub collection_manifest_sha256: String,
    pub series_id_sha256: String,
    pub underlier_id_sha256: String,
    pub epoch_sha256: String,
    pub collection_sha256: String,
    pub aggregate_attestation_sha256: String,
    pub prior_state_sha256: String,
    pub selected_expiration_sha256: String,
    pub target_sha256: String,
    pub status: YoloTargetStatusV1,
    pub snapshot_count: u32,
    pub selected_contract_count: u32,
}

impl YoloTargetPublicValuesV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != YOLO_TARGET_PUBLIC_VALUES_SCHEMA_V1 {
            return Err("YOLO target public-values schema mismatch".to_string());
        }
        for (field, value) in [
            ("program_sha256", &self.program_sha256),
            ("methodology_sha256", &self.methodology_sha256),
            ("parameter_manifest_sha256", &self.parameter_manifest_sha256),
            (
                "collection_manifest_sha256",
                &self.collection_manifest_sha256,
            ),
            ("series_id_sha256", &self.series_id_sha256),
            ("underlier_id_sha256", &self.underlier_id_sha256),
            ("epoch_sha256", &self.epoch_sha256),
            ("collection_sha256", &self.collection_sha256),
            (
                "aggregate_attestation_sha256",
                &self.aggregate_attestation_sha256,
            ),
            ("prior_state_sha256", &self.prior_state_sha256),
            (
                "selected_expiration_sha256",
                &self.selected_expiration_sha256,
            ),
            ("target_sha256", &self.target_sha256),
        ] {
            yolo_validate_lower_hex(field, value, 32)?;
        }
        if self.snapshot_count == 0 || self.snapshot_count > YOLO_TARGET_MAX_SNAPSHOTS_V1 {
            return Err(format!(
                "snapshot_count must be in 1..={YOLO_TARGET_MAX_SNAPSHOTS_V1}"
            ));
        }
        if self.selected_contract_count > YOLO_TARGET_MAX_SELECTED_CONTRACTS_V1 {
            return Err(format!(
                "selected_contract_count must be in 0..={YOLO_TARGET_MAX_SELECTED_CONTRACTS_V1}"
            ));
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut output = Vec::with_capacity(YOLO_TARGET_PUBLIC_VALUES_V1_BYTES);
        output.extend_from_slice(YOLO_TARGET_PUBLIC_VALUES_MAGIC_V1);
        output.extend_from_slice(&YOLO_TARGET_PUBLIC_VALUES_VERSION_V1.to_be_bytes());
        for value in [
            &self.program_sha256,
            &self.methodology_sha256,
            &self.parameter_manifest_sha256,
            &self.collection_manifest_sha256,
            &self.series_id_sha256,
            &self.underlier_id_sha256,
            &self.epoch_sha256,
            &self.collection_sha256,
            &self.aggregate_attestation_sha256,
            &self.prior_state_sha256,
            &self.selected_expiration_sha256,
            &self.target_sha256,
        ] {
            yolo_append_hex(&mut output, value)?;
        }
        output.extend_from_slice(&self.status.code().to_be_bytes());
        output.extend_from_slice(&self.snapshot_count.to_be_bytes());
        output.extend_from_slice(&self.selected_contract_count.to_be_bytes());
        if output.len() != YOLO_TARGET_PUBLIC_VALUES_V1_BYTES {
            return Err("YOLO target public-values length invariant failed".to_string());
        }
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != YOLO_TARGET_PUBLIC_VALUES_V1_BYTES {
            return Err(format!(
                "YOLO target public values must be exactly {YOLO_TARGET_PUBLIC_VALUES_V1_BYTES} bytes"
            ));
        }
        let mut offset = 0usize;
        let magic = yolo_take(bytes, &mut offset, 8)?;
        if magic != YOLO_TARGET_PUBLIC_VALUES_MAGIC_V1 {
            return Err("YOLO target public-values magic mismatch".to_string());
        }
        let version = u32::from_be_bytes(
            yolo_take(bytes, &mut offset, 4)?
                .try_into()
                .map_err(|_| "YOLO target version is truncated".to_string())?,
        );
        if version != YOLO_TARGET_PUBLIC_VALUES_VERSION_V1 {
            return Err("YOLO target public-values version mismatch".to_string());
        }
        let mut next_hex = || -> Result<String, String> {
            Ok(yolo_encode_hex(yolo_take(bytes, &mut offset, 32)?))
        };
        let value = Self {
            schema: YOLO_TARGET_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            program_sha256: next_hex()?,
            methodology_sha256: next_hex()?,
            parameter_manifest_sha256: next_hex()?,
            collection_manifest_sha256: next_hex()?,
            series_id_sha256: next_hex()?,
            underlier_id_sha256: next_hex()?,
            epoch_sha256: next_hex()?,
            collection_sha256: next_hex()?,
            aggregate_attestation_sha256: next_hex()?,
            prior_state_sha256: next_hex()?,
            selected_expiration_sha256: next_hex()?,
            target_sha256: next_hex()?,
            status: YoloTargetStatusV1::from_code(u32::from_be_bytes(
                yolo_take(bytes, &mut offset, 4)?
                    .try_into()
                    .map_err(|_| "YOLO target status is truncated".to_string())?,
            ))?,
            snapshot_count: u32::from_be_bytes(
                yolo_take(bytes, &mut offset, 4)?
                    .try_into()
                    .map_err(|_| "YOLO target snapshot count is truncated".to_string())?,
            ),
            selected_contract_count: u32::from_be_bytes(
                yolo_take(bytes, &mut offset, 4)?
                    .try_into()
                    .map_err(|_| "YOLO target selected count is truncated".to_string())?,
            ),
        };
        if offset != bytes.len() {
            return Err("YOLO target public values have trailing bytes".to_string());
        }
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod yolo_target_public_values_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn values() -> YoloTargetPublicValuesV1 {
        YoloTargetPublicValuesV1 {
            schema: YOLO_TARGET_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            program_sha256: "01".repeat(32),
            methodology_sha256: "02".repeat(32),
            parameter_manifest_sha256: "03".repeat(32),
            collection_manifest_sha256: "04".repeat(32),
            series_id_sha256: "05".repeat(32),
            underlier_id_sha256: "06".repeat(32),
            epoch_sha256: "07".repeat(32),
            collection_sha256: "08".repeat(32),
            aggregate_attestation_sha256: "09".repeat(32),
            prior_state_sha256: "0a".repeat(32),
            selected_expiration_sha256: "0b".repeat(32),
            target_sha256: "0c".repeat(32),
            status: YoloTargetStatusV1::TargetComputed,
            snapshot_count: 5,
            selected_contract_count: 5,
        }
    }

    #[test]
    fn yolo_target_public_values_match_cross_language_golden_vector() {
        let value = values();
        let encoded = value.encode().unwrap();
        assert_eq!(encoded.len(), YOLO_TARGET_PUBLIC_VALUES_V1_BYTES);
        assert_eq!(
            yolo_encode_hex(&Sha256::digest(&encoded)),
            "010e760e6c12211dfdd30842fdacd943644dc0aab412f897258a9469d9426a8e"
        );
        assert_eq!(YoloTargetPublicValuesV1::decode(&encoded), Ok(value));
    }

    #[test]
    fn yolo_target_public_values_reject_malformed_values() {
        let encoded = values().encode().unwrap();
        for malformed in [&encoded[..407], &encoded[..12]] {
            assert!(YoloTargetPublicValuesV1::decode(malformed).is_err());
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(YoloTargetPublicValuesV1::decode(&trailing).is_err());

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert!(YoloTargetPublicValuesV1::decode(&wrong_magic).is_err());

        let mut wrong_version = encoded.clone();
        wrong_version[11] = 2;
        assert!(YoloTargetPublicValuesV1::decode(&wrong_version).is_err());

        let mut wrong_status = encoded.clone();
        wrong_status[399] = 99;
        assert!(YoloTargetPublicValuesV1::decode(&wrong_status).is_err());

        let mut zero_snapshots = values();
        zero_snapshots.snapshot_count = 0;
        assert!(zero_snapshots.validate().is_err());

        let mut too_many_selected = values();
        too_many_selected.selected_contract_count = YOLO_TARGET_MAX_SELECTED_CONTRACTS_V1 + 1;
        assert!(too_many_selected.validate().is_err());
    }

    #[test]
    fn yolo_target_public_values_have_no_trading_or_private_fields() {
        let json = serde_json::to_string(&values()).unwrap();
        for forbidden in [
            "occ_symbol",
            "strike",
            "bid",
            "ask",
            "premium",
            "quantity",
            "account_number",
            "credential",
            "order",
        ] {
            assert!(!json.contains(forbidden));
        }
    }
}
