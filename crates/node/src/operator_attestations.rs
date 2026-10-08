use super::*;

pub const OPERATOR_CONTROL_ATTESTATION_SCHEMA: &str = "postfiat-operator-control-attestation-v1";
pub const OPERATOR_CONTROL_ATTESTATION_VERIFY_SCHEMA: &str =
    "postfiat-operator-control-attestation-verify-v1";
const OPERATOR_CONTROL_ATTESTATION_SIGNATURE_CONTEXT: &[u8] =
    b"postfiat-l1-v2/operator-control-attestation/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OperatorControlAttestationBody {
    Provider {
        provider_name: String,
        provider_account_fingerprint: String,
        instance_id: String,
        region: String,
        exclusive_control: bool,
    },
    Host {
        host_fingerprint: String,
        host_admin_fingerprint: String,
        exclusive_control: bool,
    },
    Custody {
        key_custody_fingerprint: String,
        storage_boundary: String,
        backup_boundary: String,
        exclusive_control: bool,
    },
}

impl OperatorControlAttestationBody {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Provider { .. } => "provider",
            Self::Host { .. } => "host",
            Self::Custody { .. } => "custody",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorControlAttestation {
    pub schema: String,
    pub validator_id: String,
    pub onboarding_challenge_id: String,
    pub operator: String,
    pub observed_at: String,
    pub body: OperatorControlAttestationBody,
    pub manifest_signing_key_hex: String,
    pub signature_hex: String,
    pub attestation_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorControlAttestationCreateOptions {
    pub master_key_file: PathBuf,
    pub validator_id: String,
    pub onboarding_challenge_id: String,
    pub operator: String,
    pub observed_at: String,
    pub body: OperatorControlAttestationBody,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorControlAttestationVerifyOptions {
    pub attestation_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorControlAttestationVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub attestation_file: String,
    pub attestation_hash: String,
    pub validator_id: String,
    pub onboarding_challenge_id: String,
    pub operator: String,
    pub kind: String,
    pub manifest_signing_key_hex: String,
    pub signature_verified: bool,
    pub exclusive_control: bool,
    pub redaction_checked: bool,
}

#[derive(Serialize)]
struct OperatorControlAttestationSigningPayload<'a> {
    schema: &'a str,
    validator_id: &'a str,
    onboarding_challenge_id: &'a str,
    operator: &'a str,
    observed_at: &'a str,
    body: &'a OperatorControlAttestationBody,
    manifest_signing_key_hex: &'a str,
}

#[derive(Serialize)]
struct OperatorControlAttestationHashPayload<'a> {
    signing_payload: OperatorControlAttestationSigningPayload<'a>,
    signature_hex: &'a str,
}

fn signing_payload(
    attestation: &OperatorControlAttestation,
) -> OperatorControlAttestationSigningPayload<'_> {
    OperatorControlAttestationSigningPayload {
        schema: &attestation.schema,
        validator_id: &attestation.validator_id,
        onboarding_challenge_id: &attestation.onboarding_challenge_id,
        operator: &attestation.operator,
        observed_at: &attestation.observed_at,
        body: &attestation.body,
        manifest_signing_key_hex: &attestation.manifest_signing_key_hex,
    }
}

fn signing_payload_bytes(attestation: &OperatorControlAttestation) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&signing_payload(attestation)).map_err(invalid_data)
}

fn attestation_hash(attestation: &OperatorControlAttestation) -> io::Result<String> {
    let payload = OperatorControlAttestationHashPayload {
        signing_payload: signing_payload(attestation),
        signature_hex: &attestation.signature_hex,
    };
    let encoded = serde_json::to_vec(&payload).map_err(invalid_data)?;
    let mut hasher = Sha256::new();
    hasher.update(b"postfiat.operator_control_attestation.v1\0");
    hasher.update(encoded);
    Ok(bytes_to_hex(hasher.finalize().as_slice()))
}

fn validate_attestation_body(body: &OperatorControlAttestationBody) -> io::Result<bool> {
    match body {
        OperatorControlAttestationBody::Provider {
            provider_name,
            provider_account_fingerprint,
            instance_id,
            region,
            exclusive_control,
        } => {
            validate_manifest_text_field("operator attestation provider name", provider_name)?;
            validate_hex_string(
                "operator attestation provider account fingerprint",
                provider_account_fingerprint,
                Some(64),
            )?;
            validate_manifest_text_field("operator attestation instance id", instance_id)?;
            validate_manifest_text_field("operator attestation region", region)?;
            if !exclusive_control {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "operator provider attestation must assert exclusive control",
                ));
            }
            Ok(*exclusive_control)
        }
        OperatorControlAttestationBody::Host {
            host_fingerprint,
            host_admin_fingerprint,
            exclusive_control,
        } => {
            validate_hex_string(
                "operator attestation host fingerprint",
                host_fingerprint,
                Some(64),
            )?;
            validate_hex_string(
                "operator attestation host admin fingerprint",
                host_admin_fingerprint,
                Some(64),
            )?;
            if !exclusive_control {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "operator host attestation must assert exclusive control",
                ));
            }
            Ok(*exclusive_control)
        }
        OperatorControlAttestationBody::Custody {
            key_custody_fingerprint,
            storage_boundary,
            backup_boundary,
            exclusive_control,
        } => {
            validate_hex_string(
                "operator attestation key custody fingerprint",
                key_custody_fingerprint,
                Some(64),
            )?;
            validate_manifest_text_field(
                "operator attestation custody storage boundary",
                storage_boundary,
            )?;
            validate_manifest_text_field(
                "operator attestation custody backup boundary",
                backup_boundary,
            )?;
            if !exclusive_control {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "operator custody attestation must assert exclusive control",
                ));
            }
            Ok(*exclusive_control)
        }
    }
}

fn validate_attestation_for_signing(attestation: &OperatorControlAttestation) -> io::Result<bool> {
    if attestation.schema != OPERATOR_CONTROL_ATTESTATION_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported operator control attestation schema `{}`",
                attestation.schema
            ),
        ));
    }
    validate_manifest_text_field(
        "operator attestation validator id",
        &attestation.validator_id,
    )?;
    validate_hex_string(
        "operator attestation onboarding challenge id",
        &attestation.onboarding_challenge_id,
        Some(64),
    )?;
    validate_manifest_text_field("operator attestation operator", &attestation.operator)?;
    validate_manifest_text_field("operator attestation observed at", &attestation.observed_at)?;
    if !is_utc_rfc3339_second_timestamp(&attestation.observed_at) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator attestation observed_at must be a UTC RFC3339 second timestamp",
        ));
    }
    decode_ml_dsa_65_public_key_hex(
        "operator attestation manifest signing key",
        &attestation.manifest_signing_key_hex,
    )?;
    validate_attestation_body(&attestation.body)
}

/// Whether `value` is a UTC RFC 3339 timestamp at second resolution in the
/// exact form `YYYY-MM-DDTHH:MM:SSZ`, with a real calendar date and a real
/// time of day. (SWP-05: the earlier check verified shape and digits only, so
/// `2026-99-99T99:99:99Z` was accepted.) Leap seconds are not accepted: the
/// encoding promises second resolution on a 00-59 scale.
fn is_utc_rfc3339_second_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return false;
    }
    let field = |start: usize, len: usize| -> Option<u32> {
        let digits = &bytes[start..start + len];
        if !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        digits.iter().try_fold(0u32, |acc, byte| {
            acc.checked_mul(10)?.checked_add(u32::from(byte - b'0'))
        })
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        field(0, 4),
        field(5, 2),
        field(8, 2),
        field(11, 2),
        field(14, 2),
        field(17, 2),
    ) else {
        return false;
    };
    if !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 {
        return false;
    }
    let leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        _ => 28,
    };
    (1..=days_in_month).contains(&day)
}

fn reject_attestation_private_material(raw: &str) -> io::Result<()> {
    reject_operator_manifest_private_material(raw).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "operator control attestation contains private material marker",
        )
    })
}

pub fn verify_operator_control_attestation_record(
    attestation: &OperatorControlAttestation,
    attestation_file: &Path,
) -> io::Result<OperatorControlAttestationVerifyReport> {
    let exclusive_control = validate_attestation_for_signing(attestation)?;
    let public_key = decode_ml_dsa_65_public_key_hex(
        "operator attestation manifest signing key",
        &attestation.manifest_signing_key_hex,
    )?;
    let signature = decode_ml_dsa_65_signature_hex(
        "operator control attestation signature",
        &attestation.signature_hex,
    )?;
    let payload = signing_payload_bytes(attestation)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &payload,
        &signature,
        OPERATOR_CONTROL_ATTESTATION_SIGNATURE_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator control attestation signature verification failed",
        ));
    }
    validate_hex_string(
        "operator control attestation hash",
        &attestation.attestation_hash,
        Some(64),
    )?;
    if attestation.attestation_hash != attestation_hash(attestation)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator control attestation hash mismatch",
        ));
    }
    Ok(OperatorControlAttestationVerifyReport {
        schema: OPERATOR_CONTROL_ATTESTATION_VERIFY_SCHEMA.to_string(),
        verified: true,
        attestation_file: attestation_file.display().to_string(),
        attestation_hash: attestation.attestation_hash.clone(),
        validator_id: attestation.validator_id.clone(),
        onboarding_challenge_id: attestation.onboarding_challenge_id.clone(),
        operator: attestation.operator.clone(),
        kind: attestation.body.kind().to_string(),
        manifest_signing_key_hex: attestation.manifest_signing_key_hex.clone(),
        signature_verified: true,
        exclusive_control,
        redaction_checked: true,
    })
}

pub fn read_operator_control_attestation_file(
    path: &Path,
) -> io::Result<OperatorControlAttestation> {
    let raw = read_bounded_json_text_file(path, "operator control attestation")?;
    reject_attestation_private_material(&raw)?;
    serde_json::from_str(&raw).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "failed to parse operator control attestation `{}`: {error}",
                path.display()
            ),
        )
    })
}

pub fn verify_operator_control_attestation(
    options: OperatorControlAttestationVerifyOptions,
) -> io::Result<OperatorControlAttestationVerifyReport> {
    let attestation = read_operator_control_attestation_file(&options.attestation_file)?;
    verify_operator_control_attestation_record(&attestation, &options.attestation_file)
}

pub fn create_operator_control_attestation(
    options: OperatorControlAttestationCreateOptions,
) -> io::Result<OperatorControlAttestation> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "operator control attestation",
    )?;
    validate_private_file_permissions(
        &options.master_key_file,
        "operator control attestation master key",
    )?;
    let master_key = read_key_file(&options.master_key_file)?;
    let mut attestation = OperatorControlAttestation {
        schema: OPERATOR_CONTROL_ATTESTATION_SCHEMA.to_string(),
        validator_id: options.validator_id,
        onboarding_challenge_id: options.onboarding_challenge_id,
        operator: options.operator,
        observed_at: options.observed_at,
        body: options.body,
        manifest_signing_key_hex: master_key.public_key_hex,
        signature_hex: String::new(),
        attestation_hash: String::new(),
    };
    validate_attestation_for_signing(&attestation)?;
    let private_key =
        Zeroizing::new(hex_to_bytes(&master_key.private_key_hex).map_err(invalid_data)?);
    let payload = signing_payload_bytes(&attestation)?;
    let signature = ml_dsa_65_sign_with_context(
        &private_key,
        &payload,
        OPERATOR_CONTROL_ATTESTATION_SIGNATURE_CONTEXT,
    )
    .map_err(invalid_data)?;
    attestation.signature_hex = bytes_to_hex(&signature);
    attestation.attestation_hash = attestation_hash(&attestation)?;
    verify_operator_control_attestation_record(&attestation, &options.output_file)?;
    let json = serde_json::to_string_pretty(&attestation).map_err(invalid_data)?;
    reject_attestation_private_material(&json)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    Ok(attestation)
}

#[cfg(test)]
mod tests {
    use super::is_utc_rfc3339_second_timestamp;

    #[test]
    fn observed_at_accepts_real_utc_second_timestamps() {
        for value in [
            "2026-10-08T20:52:37Z",
            "2024-02-29T00:00:00Z", // leap year
            "2000-02-29T23:59:59Z", // divisible by 400: leap year
            "1999-12-31T23:59:59Z",
            "2026-01-31T00:00:00Z",
            "2026-04-30T12:30:00Z",
        ] {
            assert!(is_utc_rfc3339_second_timestamp(value), "{value}");
        }
    }

    #[test]
    fn observed_at_rejects_impossible_calendar_and_time_values() {
        // SWP-05: shape and digits were the only checks before.
        for value in [
            "2026-99-99T99:99:99Z",
            "2026-00-10T00:00:00Z", // month 0
            "2026-13-10T00:00:00Z", // month 13
            "2026-10-00T00:00:00Z", // day 0
            "2026-10-32T00:00:00Z", // day 32
            "2026-04-31T00:00:00Z", // April has 30 days
            "2026-02-29T00:00:00Z", // not a leap year
            "2100-02-29T00:00:00Z", // divisible by 100, not 400: not a leap year
            "2026-10-08T24:00:00Z", // hour 24
            "2026-10-08T00:60:00Z", // minute 60
            "2026-10-08T00:00:60Z", // second 60 (no leap seconds)
        ] {
            assert!(!is_utc_rfc3339_second_timestamp(value), "{value}");
        }
    }

    #[test]
    fn observed_at_rejects_wrong_shape() {
        for value in [
            "",
            "2026-10-08T20:52:37",      // no Z
            "2026-10-08T20:52:37.000Z", // fractional seconds
            "2026-10-08 20:52:37Z",     // space separator
            "2026-10-08T20:52:37+00:00",
            "2026-1-08T20:52:37Z",
            "２026-10-08T20:52:37Z", // non-ASCII digit
            "2026-10-08T20:52:3xZ",
        ] {
            assert!(!is_utc_rfc3339_second_timestamp(value), "{value}");
        }
    }
}
