pub const YOLO_TARGET_REGISTER_TRANSACTION_KIND_V1: &str = "yolo_target_register_v1";
pub const YOLO_TARGET_SUBMIT_TRANSACTION_KIND_V1: &str = "yolo_target_submit_v1";
pub const YOLO_TARGET_SP1_GROTH16_VERIFIER_V1: &str = "postfiat-yolo-target-sp1-groth16-v1";
pub const YOLO_TARGET_MAX_PROOF_BYTES_V1: usize = 4096;

/// Immutable caller-approved run expectations. A registration is namespaced by
/// its signer and does not grant asset, reserve, trading or issuance authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloTargetRegisterOperationV1 {
    pub registrant: String,
    pub submitter: String,
    pub program_sha256: String,
    pub sp1_program_vkey: String,
    pub methodology_sha256: String,
    pub parameter_manifest_sha256: String,
    pub collection_manifest_sha256: String,
    pub series_id_sha256: String,
    pub underlier_id_sha256: String,
    pub epoch_sha256: String,
    pub prior_state_sha256: String,
    pub replay_id_sha256: String,
    pub activation_height: u64,
}

impl YoloTargetRegisterOperationV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("yolo registrant", &self.registrant)?;
        validate_text_field("yolo submitter", &self.submitter)?;
        for value in [
            &self.program_sha256,
            &self.methodology_sha256,
            &self.parameter_manifest_sha256,
            &self.collection_manifest_sha256,
            &self.series_id_sha256,
            &self.underlier_id_sha256,
            &self.epoch_sha256,
            &self.prior_state_sha256,
            &self.replay_id_sha256,
        ] {
            yolo_validate_lower_hex("yolo registration digest", value, 32)?;
        }
        let key = self
            .sp1_program_vkey
            .strip_prefix("0x")
            .ok_or_else(|| "yolo vkey requires 0x prefix".to_string())?;
        yolo_validate_lower_hex("yolo program vkey", key, 32)?;
        if self.activation_height == 0 {
            return Err("yolo activation height must be explicit and nonzero".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        // Struct serialization has a fixed field order; all fields are mandatory.
        serde_json::to_vec(self).expect("YOLO registration consists of serializable primitives")
    }

    pub fn registration_id(&self) -> String {
        let encoded = self.signing_bytes();
        let mut hasher = Sha3_384::new();
        hasher.update(b"postfiat.yolo.target_registration.v1");
        hasher.update((encoded.len() as u64).to_be_bytes());
        hasher.update(encoded);
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    pub fn validate_public_values(&self, value: &YoloTargetPublicValuesV1) -> Result<(), String> {
        self.validate()?;
        value.validate()?;
        for (actual, expected) in [
            (&value.program_sha256, &self.program_sha256),
            (&value.methodology_sha256, &self.methodology_sha256),
            (
                &value.parameter_manifest_sha256,
                &self.parameter_manifest_sha256,
            ),
            (
                &value.collection_manifest_sha256,
                &self.collection_manifest_sha256,
            ),
            (&value.series_id_sha256, &self.series_id_sha256),
            (&value.underlier_id_sha256, &self.underlier_id_sha256),
            (&value.epoch_sha256, &self.epoch_sha256),
            (&value.prior_state_sha256, &self.prior_state_sha256),
        ] {
            if actual != expected {
                return Err("target public values differ from registered expectations".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloTargetSubmitOperationV1 {
    pub submitter: String,
    pub registration_id: String,
    pub replay_id_sha256: String,
    pub values: YoloTargetPublicValuesV1,
    pub sp1_proof_bytes: Vec<u8>,
    pub sp1_public_values: Vec<u8>,
}

impl YoloTargetSubmitOperationV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("yolo submitter", &self.submitter)?;
        yolo_validate_lower_hex("yolo registration id", &self.registration_id, 48)?;
        yolo_validate_lower_hex("yolo replay id", &self.replay_id_sha256, 32)?;
        if self.sp1_proof_bytes.is_empty()
            || self.sp1_proof_bytes.len() > YOLO_TARGET_MAX_PROOF_BYTES_V1
        {
            return Err("target proof length outside bound".to_string());
        }
        if self.sp1_public_values.len() != YOLO_TARGET_PUBLIC_VALUES_V1_BYTES {
            return Err("target public values must contain exactly 408 bytes".to_string());
        }
        if self.values.encode()? != self.sp1_public_values {
            return Err("redundant target fields differ from public bytes".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("YOLO submission consists of serializable primitives")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloTargetRegistrationV1 {
    pub registration_id: String,
    pub registered_height: u64,
    pub operation: YoloTargetRegisterOperationV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloTargetReceiptV1 {
    pub registration_id: String,
    pub transaction_hash: String,
    pub inclusion_height: u64,
    pub operation: YoloTargetSubmitOperationV1,
}
