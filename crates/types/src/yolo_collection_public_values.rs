pub const YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.yolo.collection_public_values.v1";
pub const YOLO_COLLECTION_PUBLIC_VALUES_MAGIC_V1: &[u8; 8] = b"PFTYCOL1";
pub const YOLO_COLLECTION_PUBLIC_VALUES_VERSION_V1: u32 = 1;
pub const YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES: usize = 304;
pub const YOLO_COLLECTION_MAX_SNAPSHOTS_V1: u32 = 64;

/// Fixed-width disclosure-safe output of the YOLO collection SP1 program.
///
/// This proves collection-manifest and commitment consistency only. It does not
/// select options, value a portfolio, authorize an order, or create reserves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloCollectionPublicValuesV1 {
    pub schema: String,
    pub program_sha256: String,
    pub epoch_sha256: String,
    pub methodology_sha256: String,
    pub collector_code_sha256: String,
    pub source_id_sha256: String,
    pub account_application_identity_sha256: String,
    pub commitments_sha256: String,
    pub attested_collection_sha256: String,
    pub normalized_input_root_sha256: String,
    pub snapshot_count: u32,
}

impl YoloCollectionPublicValuesV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1 {
            return Err("YOLO collection public-values schema mismatch".to_string());
        }
        for (field, value) in [
            ("program_sha256", &self.program_sha256),
            ("epoch_sha256", &self.epoch_sha256),
            ("methodology_sha256", &self.methodology_sha256),
            ("collector_code_sha256", &self.collector_code_sha256),
            ("source_id_sha256", &self.source_id_sha256),
            (
                "account_application_identity_sha256",
                &self.account_application_identity_sha256,
            ),
            ("commitments_sha256", &self.commitments_sha256),
            (
                "attested_collection_sha256",
                &self.attested_collection_sha256,
            ),
            (
                "normalized_input_root_sha256",
                &self.normalized_input_root_sha256,
            ),
        ] {
            yolo_validate_lower_hex(field, value, 32)?;
        }
        if self.snapshot_count == 0 || self.snapshot_count > YOLO_COLLECTION_MAX_SNAPSHOTS_V1 {
            return Err(format!(
                "snapshot_count must be in 1..={YOLO_COLLECTION_MAX_SNAPSHOTS_V1}"
            ));
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut output = Vec::with_capacity(YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES);
        output.extend_from_slice(YOLO_COLLECTION_PUBLIC_VALUES_MAGIC_V1);
        output.extend_from_slice(&YOLO_COLLECTION_PUBLIC_VALUES_VERSION_V1.to_be_bytes());
        for value in [
            &self.program_sha256,
            &self.epoch_sha256,
            &self.methodology_sha256,
            &self.collector_code_sha256,
            &self.source_id_sha256,
            &self.account_application_identity_sha256,
            &self.commitments_sha256,
            &self.attested_collection_sha256,
            &self.normalized_input_root_sha256,
        ] {
            yolo_append_hex(&mut output, value)?;
        }
        output.extend_from_slice(&self.snapshot_count.to_be_bytes());
        if output.len() != YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES {
            return Err("YOLO collection public-values length invariant failed".to_string());
        }
        Ok(output)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES {
            return Err(format!(
                "YOLO collection public values must be exactly {YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES} bytes"
            ));
        }
        let mut offset = 0usize;
        let magic = yolo_take(bytes, &mut offset, 8)?;
        if magic != YOLO_COLLECTION_PUBLIC_VALUES_MAGIC_V1 {
            return Err("YOLO collection public-values magic mismatch".to_string());
        }
        let version = u32::from_be_bytes(
            yolo_take(bytes, &mut offset, 4)?
                .try_into()
                .map_err(|_| "YOLO collection version is truncated".to_string())?,
        );
        if version != YOLO_COLLECTION_PUBLIC_VALUES_VERSION_V1 {
            return Err("YOLO collection public-values version mismatch".to_string());
        }
        let mut next_hex = || -> Result<String, String> {
            Ok(yolo_encode_hex(yolo_take(bytes, &mut offset, 32)?))
        };
        let value = Self {
            schema: YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            program_sha256: next_hex()?,
            epoch_sha256: next_hex()?,
            methodology_sha256: next_hex()?,
            collector_code_sha256: next_hex()?,
            source_id_sha256: next_hex()?,
            account_application_identity_sha256: next_hex()?,
            commitments_sha256: next_hex()?,
            attested_collection_sha256: next_hex()?,
            normalized_input_root_sha256: next_hex()?,
            snapshot_count: u32::from_be_bytes(
                yolo_take(bytes, &mut offset, 4)?
                    .try_into()
                    .map_err(|_| "YOLO snapshot count is truncated".to_string())?,
            ),
        };
        if offset != bytes.len() {
            return Err("YOLO collection public values have trailing bytes".to_string());
        }
        value.validate()?;
        Ok(value)
    }
}

fn yolo_validate_lower_hex(field: &str, value: &str, bytes: usize) -> Result<(), String> {
    if value.len() != bytes * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{field} must be exactly {bytes} bytes of lowercase hex"
        ));
    }
    Ok(())
}

fn yolo_append_hex(output: &mut Vec<u8>, value: &str) -> Result<(), String> {
    yolo_validate_lower_hex("YOLO collection digest", value, 32)?;
    for pair in value.as_bytes().chunks_exact(2) {
        output.push((yolo_hex_nibble(pair[0])? << 4) | yolo_hex_nibble(pair[1])?);
    }
    Ok(())
}

fn yolo_encode_hex(bytes: &[u8]) -> String {
    const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(LOWER_HEX[(byte >> 4) as usize] as char);
        encoded.push(LOWER_HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn yolo_hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("YOLO collection digest decode failed".to_string()),
    }
}

fn yolo_take<'a>(bytes: &'a [u8], offset: &mut usize, length: usize) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| "YOLO collection public-values offset overflow".to_string())?;
    let value = bytes
        .get(*offset..end)
        .ok_or_else(|| "YOLO collection public values are truncated".to_string())?;
    *offset = end;
    Ok(value)
}

#[cfg(test)]
mod yolo_collection_public_values_tests {
    use super::*;

    fn values() -> YoloCollectionPublicValuesV1 {
        YoloCollectionPublicValuesV1 {
            schema: YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            program_sha256: "01".repeat(32),
            epoch_sha256: "02".repeat(32),
            methodology_sha256: "03".repeat(32),
            collector_code_sha256: "04".repeat(32),
            source_id_sha256: "05".repeat(32),
            account_application_identity_sha256: "06".repeat(32),
            commitments_sha256: "07".repeat(32),
            attested_collection_sha256: "08".repeat(32),
            normalized_input_root_sha256: "09".repeat(32),
            snapshot_count: 5,
        }
    }

    #[test]
    fn yolo_collection_public_values_round_trip_fixed_width() {
        let value = values();
        let encoded = value.encode().unwrap();
        assert_eq!(encoded.len(), YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES);
        assert_eq!(
            YoloCollectionPublicValuesV1::decode(&encoded).unwrap(),
            value
        );
    }

    #[test]
    fn yolo_collection_public_values_reject_bad_count_and_trailing_data() {
        let mut value = values();
        value.snapshot_count = 0;
        assert!(value.validate().is_err());
        let mut encoded = values().encode().unwrap();
        encoded.push(0);
        assert!(YoloCollectionPublicValuesV1::decode(&encoded).is_err());
    }
}
