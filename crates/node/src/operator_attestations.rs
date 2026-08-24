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
    let observed = attestation.observed_at.as_bytes();
    if observed.len() != 20
        || observed[4] != b'-'
        || observed[7] != b'-'
        || observed[10] != b'T'
        || observed[13] != b':'
        || observed[16] != b':'
        || observed[19] != b'Z'
        || observed.iter().enumerate().any(|(index, byte)| {
            !matches!(index, 4 | 7 | 10 | 13 | 16 | 19) && !byte.is_ascii_digit()
        })
    {
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
