pub const OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3: &str = "postfiat-owned-certificate-domain-v3";
pub const FASTPAY_ORDER_RECOVERY_SCHEMA_V1: &str = "postfiat-fastpay-order-recovery-v1";
pub const FASTPAY_RECOVERY_POLICY_SCHEMA_V1: &str = "postfiat-fastpay-recovery-policy-v1";
pub const FASTPAY_RECOVERY_REVEAL_SCHEMA_V1: &str = "postfiat-fastpay-recovery-reveal-v1";
pub const FASTPAY_RECOVERY_DECISION_REQUEST_SCHEMA_V1: &str =
    "postfiat-fastpay-recovery-decision-request-v1";
pub const FASTPAY_VERSION_FENCE_SCHEMA_V1: &str = "postfiat-fastpay-version-fence-v1";
pub const FASTPAY_APPLY_ACK_SCHEMA_V1: &str = "postfiat-fastpay-apply-ack-v1";
pub const FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1: &str =
    "postfiat-fastpay-recovery-capabilities-v1";
pub const FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V1: &str = "postfiat-fastpay-recovery-committee-v1";
pub const FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V2: &str = "postfiat-fastpay-recovery-committee-v2";

/// Selected by ordered committee installation, never by trying alternative hashes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastPayRecoveryCommitmentVersion {
    V1,
    V2,
}

impl FastPayRecoveryCommitmentVersion {
    pub fn for_committees(committees: &[FastPayRecoveryCommitteeV1]) -> Result<Self, String> {
        if committees.len() > MAX_FASTPAY_RECOVERY_COMMITTEES {
            return Err("too many FastPay recovery committees".to_string());
        }
        let mut version = Self::V1;
        let mut previous_epoch = 0;
        for committee in committees {
            committee.validate()?;
            let next = committee.commitment_version()?;
            if committee.committee_epoch <= previous_epoch
                || (version == Self::V2 && next == Self::V1)
            {
                return Err(
                    "FastPay committee order or commitment downgrade is invalid".to_string()
                );
            }
            previous_epoch = committee.committee_epoch;
            version = next;
        }
        Ok(version)
    }
}
pub const FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1: &str = "fastpay_recovery_bootstrap_v1:";
pub const FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1: u32 = 1;
pub const FASTPAY_LOCK_ID_DOMAIN_V1: &str = "postfiat.fastpay.lock-id.v1";
pub const FASTPAY_CERTIFICATE_DIGEST_DOMAIN_V1: &str = "postfiat.fastpay.certificate.v1";
pub const MAX_FASTPAY_VALIDITY_BLOCKS: u64 = 10_000;
pub const MAX_FASTPAY_RECOVERY_BLOCKS: u64 = 10_000;
pub const MAX_FASTPAY_RECOVERY_REVEALS: usize = 4_096;
pub const MAX_FASTPAY_VERSION_FENCES: usize = 65_536;
pub const MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK: usize = 64;
pub const MAX_FASTPAY_RECOVERY_COMMITTEES: usize = 64;
pub const MAX_FASTPAY_RECOVERY_VALIDATORS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryPolicyV1 {
    pub schema: String,
    pub activation_height: u64,
    pub max_validity_blocks: u64,
    pub max_recovery_blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryCapabilitiesV1 {
    pub schema: String,
    pub domain: OwnedCertificateDomain,
    pub committee_epoch: u64,
    pub current_height: u64,
    pub validator_count: usize,
    pub quorum: usize,
    pub policy: FastPayRecoveryPolicyV1,
}

impl FastPayRecoveryCapabilitiesV1 {
    pub fn validate(&self) -> Result<(), String> {
        self.policy.validate()?;
        if self.schema != FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1
            || self.domain.schema != OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
            || self.committee_epoch == 0
            || self.validator_count == 0
            || self.quorum == 0
            || self.quorum > self.validator_count
        {
            return Err("FastPay recovery capabilities are invalid".to_string());
        }
        Ok(())
    }
}

impl FastPayRecoveryPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FASTPAY_RECOVERY_POLICY_SCHEMA_V1 {
            return Err("FastPay recovery policy schema mismatch".to_string());
        }
        if self.activation_height == 0
            || self.max_validity_blocks == 0
            || self.max_validity_blocks > MAX_FASTPAY_VALIDITY_BLOCKS
            || self.max_recovery_blocks == 0
            || self.max_recovery_blocks > MAX_FASTPAY_RECOVERY_BLOCKS
        {
            return Err("FastPay recovery policy heights are invalid or unbounded".to_string());
        }
        Ok(())
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = b"postfiat.fastpay.recovery-policy.state.v1\0".to_vec();
        fastpay_commit_text(&mut bytes, &self.schema);
        bytes.extend_from_slice(&self.activation_height.to_be_bytes());
        bytes.extend_from_slice(&self.max_validity_blocks.to_be_bytes());
        bytes.extend_from_slice(&self.max_recovery_blocks.to_be_bytes());
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryValidatorV1 {
    pub validator_id: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryCommitteeV1 {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub committee_epoch: u64,
    pub valid_from_height: u64,
    pub new_orders_through_height: u64,
    pub registry_root: String,
    pub quorum: usize,
    pub validators: Vec<FastPayRecoveryValidatorV1>,
}

impl FastPayRecoveryCommitteeV1 {
    pub fn from_public_keys(
        chain_id: String,
        genesis_hash: String,
        protocol_version: u32,
        committee_epoch: u64,
        valid_from_height: u64,
        new_orders_through_height: u64,
        mut validator_public_keys: Vec<(String, String)>,
    ) -> Result<Self, String> {
        validator_public_keys.sort_by(|left, right| left.0.cmp(&right.0));
        if validator_public_keys
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err("FastPay recovery committee contains a duplicate validator".to_string());
        }
        let validators = validator_public_keys
            .into_iter()
            .map(
                |(validator_id, public_key_hex)| FastPayRecoveryValidatorV1 {
                    validator_id,
                    algorithm_id: FASTSWAP_ML_DSA_65.to_string(),
                    public_key_hex,
                },
            )
            .collect::<Vec<_>>();
        let mut committee = Self {
            schema: FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V2.to_string(),
            chain_id,
            genesis_hash,
            protocol_version,
            committee_epoch,
            valid_from_height,
            new_orders_through_height,
            registry_root: String::new(),
            quorum: Self::expected_quorum(validators.len()).unwrap_or(0),
            validators,
        };
        committee.registry_root = committee.computed_root()?;
        committee.validate()?;
        Ok(committee)
    }

    pub fn expected_quorum(validator_count: usize) -> Option<usize> {
        (validator_count > 0).then(|| validator_count - (validator_count - 1) / 3)
    }

    pub fn commitment_version(&self) -> Result<FastPayRecoveryCommitmentVersion, String> {
        match self.schema.as_str() {
            FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V1 => Ok(FastPayRecoveryCommitmentVersion::V1),
            FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V2 => Ok(FastPayRecoveryCommitmentVersion::V2),
            _ => Err("FastPay recovery committee schema mismatch".to_string()),
        }
    }

    fn root_domain(&self) -> Result<&'static str, String> {
        Ok(match self.commitment_version()? {
            FastPayRecoveryCommitmentVersion::V1 => "postfiat.fastpay.recovery-committee.root.v1",
            FastPayRecoveryCommitmentVersion::V2 => "postfiat.fastpay.recovery-committee.root.v2",
        })
    }

    pub fn root_preimage(&self) -> Result<Vec<u8>, String> {
        let version = self.commitment_version()?;
        if self.validators.is_empty()
            || self.validators.len() > MAX_FASTPAY_RECOVERY_VALIDATORS
            || self.chain_id.is_empty()
            || validate_lower_hex_len("fastpay_committee.genesis_hash", &self.genesis_hash, 96)
                .is_err()
            || self.committee_epoch == 0
            || self.valid_from_height == 0
            || self.new_orders_through_height < self.valid_from_height
        {
            return Err("FastPay recovery committee size or epoch is invalid".to_string());
        }
        let mut bytes = self.root_domain()?.as_bytes().to_vec();
        bytes.push(0);
        fastpay_commit_text(&mut bytes, &self.chain_id);
        fastpay_commit_text(&mut bytes, &self.genesis_hash);
        bytes.extend_from_slice(&self.protocol_version.to_be_bytes());
        bytes.extend_from_slice(&self.committee_epoch.to_be_bytes());
        if version == FastPayRecoveryCommitmentVersion::V2 {
            bytes.extend_from_slice(&self.valid_from_height.to_be_bytes());
            bytes.extend_from_slice(&self.new_orders_through_height.to_be_bytes());
        }
        bytes.extend_from_slice(&(self.validators.len() as u64).to_be_bytes());
        let mut previous = None;
        for validator in &self.validators {
            if validator.validator_id.is_empty()
                || validator.algorithm_id != FASTSWAP_ML_DSA_65
                || previous.is_some_and(|value| value >= validator.validator_id.as_str())
            {
                return Err("FastPay recovery validators are not canonical".to_string());
            }
            validate_lower_hex_max(
                "fastpay_committee.public_key_hex",
                &validator.public_key_hex,
                MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
            )?;
            fastpay_commit_text(&mut bytes, &validator.validator_id);
            fastpay_commit_text(&mut bytes, &validator.algorithm_id);
            fastpay_commit_text(&mut bytes, &validator.public_key_hex);
            previous = Some(validator.validator_id.as_str());
        }
        Ok(bytes)
    }

    pub fn computed_root(&self) -> Result<String, String> {
        Ok(hash_hex_domain(self.root_domain()?, &self.root_preimage()?))
    }

    pub fn validate(&self) -> Result<(), String> {
        self.commitment_version()?;
        if self.quorum != Self::expected_quorum(self.validators.len()).unwrap_or(0)
            || self.registry_root != self.computed_root()?
        {
            return Err("FastPay recovery committee is invalid".to_string());
        }
        Ok(())
    }

    pub fn validator_public_keys(&self) -> Vec<(String, String)> {
        self.validators
            .iter()
            .map(|validator| {
                (
                    validator.validator_id.clone(),
                    validator.public_key_hex.clone(),
                )
            })
            .collect()
    }

    pub fn certificate_domain(&self) -> OwnedCertificateDomain {
        OwnedCertificateDomain {
            schema: OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
            chain_id: self.chain_id.clone(),
            genesis_hash: self.genesis_hash.clone(),
            protocol_version: self.protocol_version,
            registry_id: self.registry_root.clone(),
        }
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = match self.commitment_version()? {
            FastPayRecoveryCommitmentVersion::V1 => {
                b"postfiat.fastpay.recovery-committee.state.v1\0"
            }
            FastPayRecoveryCommitmentVersion::V2 => {
                b"postfiat.fastpay.recovery-committee.state.v2\0"
            }
        }
        .to_vec();
        fastpay_commit_text(&mut bytes, &self.schema);
        bytes.extend_from_slice(&self.root_preimage()?);
        bytes.extend_from_slice(&self.valid_from_height.to_be_bytes());
        bytes.extend_from_slice(&self.new_orders_through_height.to_be_bytes());
        fastpay_commit_text(&mut bytes, &self.registry_root);
        bytes.extend_from_slice(&(self.quorum as u64).to_be_bytes());
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryGovernancePayloadV1 {
    pub policy: FastPayRecoveryPolicyV1,
    pub committee: FastPayRecoveryCommitteeV1,
}

impl FastPayRecoveryGovernancePayloadV1 {
    pub fn payload_bytes(&self) -> Result<Vec<u8>, String> {
        self.policy.validate()?;
        self.committee.validate()?;
        if self.policy.activation_height > self.committee.valid_from_height {
            return Err("FastPay committee cannot activate before the recovery policy".to_string());
        }
        let mut bytes = b"postfiat.fastpay.recovery-governance-bootstrap.v1\0".to_vec();
        let policy = self.policy.state_commitment_bytes()?;
        let committee = self.committee.state_commitment_bytes()?;
        bytes.extend_from_slice(&(policy.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&policy);
        bytes.extend_from_slice(&(committee.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&committee);
        Ok(bytes)
    }

    pub fn payload_id(&self) -> Result<String, String> {
        Ok(hash_hex_domain(
            "postfiat.fastpay.recovery-governance-bootstrap.v1",
            &self.payload_bytes()?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryGovernanceBootstrapV1 {
    pub amendment: GovernanceAmendment,
    pub payload: FastPayRecoveryGovernancePayloadV1,
}

impl FastPayRecoveryGovernanceBootstrapV1 {
    pub fn validate_payload_binding(&self) -> Result<(), String> {
        let validator_ids = self
            .payload
            .committee
            .validators
            .iter()
            .map(|validator| validator.validator_id.as_str())
            .collect::<Vec<_>>();
        if self.amendment.kind
            != format!(
                "{}{}",
                FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1,
                self.payload.payload_id()?
            )
            || self.amendment.value != FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1
            || self.amendment.chain_id != self.payload.committee.chain_id
            || self.amendment.genesis_hash != self.payload.committee.genesis_hash
            || self.amendment.protocol_version != self.payload.committee.protocol_version
            // The amendment commits the complete payload hash immediately. The
            // policy's activation height governs when validators may use the
            // recovery lane; making the amendment itself wait for that height
            // would make a future activation impossible to stage on chain.
            || self.amendment.activation_height != 0
            || self.amendment.validators.iter().map(String::as_str).collect::<Vec<_>>()
                != validator_ids
            || self.amendment.paused
        {
            return Err("FastPay recovery governance payload binding is invalid".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayOrderRecoveryV1 {
    pub schema: String,
    pub committee_epoch: u64,
    /// Domain-separated SHA3-384 digest, lowercase hex without a prefix.
    pub lock_id: String,
    pub valid_from_height: u64,
    pub expires_at_height: u64,
    pub recovery_closes_at_height: u64,
}

impl FastPayOrderRecoveryV1 {
    pub fn validate(&self, policy: &FastPayRecoveryPolicyV1) -> Result<(), String> {
        policy.validate()?;
        if self.schema != FASTPAY_ORDER_RECOVERY_SCHEMA_V1 {
            return Err("FastPay order recovery schema mismatch".to_string());
        }
        if self.committee_epoch == 0 {
            return Err("FastPay recovery committee epoch must be nonzero".to_string());
        }
        validate_lower_hex_len("fastpay.lock_id", &self.lock_id, 96)?;
        if self.valid_from_height == 0
            || self.expires_at_height < self.valid_from_height
            || self.recovery_closes_at_height <= self.expires_at_height
        {
            return Err("FastPay recovery window ordering is invalid".to_string());
        }
        let validity_blocks = self
            .expires_at_height
            .checked_sub(self.valid_from_height)
            .ok_or_else(|| "FastPay validity window underflow".to_string())?;
        let recovery_blocks = self
            .recovery_closes_at_height
            .checked_sub(self.expires_at_height)
            .ok_or_else(|| "FastPay recovery window underflow".to_string())?;
        if validity_blocks > policy.max_validity_blocks
            || recovery_blocks > policy.max_recovery_blocks
        {
            return Err("FastPay order exceeds the governed recovery policy".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastPayOperationKindV1 {
    Transfer,
    Unwrap,
}

impl FastPayOperationKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transfer => "transfer",
            Self::Unwrap => "unwrap",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedTransferOrderV3 {
    pub domain: OwnedCertificateDomain,
    pub recovery: FastPayOrderRecoveryV1,
    pub inputs: Vec<OwnedObjectRef>,
    pub outputs: Vec<OwnedOutputSpec>,
    pub fee: u64,
    pub nonce: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memos: Vec<PaymentMemo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedOwnedTransferOrderV3 {
    pub order: OwnedTransferOrderV3,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedTransferCertificateV3 {
    pub order: OwnedTransferOrderV3,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
    pub votes: Vec<OwnedTransferVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedUnwrapOrderV3 {
    pub domain: OwnedCertificateDomain,
    pub recovery: FastPayOrderRecoveryV1,
    pub inputs: Vec<OwnedObjectRef>,
    pub to_address: String,
    pub amount: u64,
    pub asset: String,
    pub fee: u64,
    pub nonce: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memos: Vec<PaymentMemo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedOwnedUnwrapOrderV3 {
    pub order: OwnedUnwrapOrderV3,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedUnwrapCertificateV3 {
    pub order: OwnedUnwrapOrderV3,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
    pub votes: Vec<OwnedUnwrapVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "certificate", rename_all = "snake_case")]
pub enum FastPayCertificateV1 {
    Transfer(OwnedTransferCertificateV3),
    Unwrap(OwnedUnwrapCertificateV3),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "signed_order", rename_all = "snake_case")]
pub enum FastPaySignedOrderV1 {
    Transfer(SignedOwnedTransferOrderV3),
    Unwrap(SignedOwnedUnwrapOrderV3),
}

impl FastPaySignedOrderV1 {
    pub fn operation(&self) -> FastPayOperationKindV1 {
        match self {
            Self::Transfer(_) => FastPayOperationKindV1::Transfer,
            Self::Unwrap(_) => FastPayOperationKindV1::Unwrap,
        }
    }

    pub fn recovery(&self) -> &FastPayOrderRecoveryV1 {
        match self {
            Self::Transfer(signed) => &signed.order.recovery,
            Self::Unwrap(signed) => &signed.order.recovery,
        }
    }

    pub fn inputs(&self) -> &[OwnedObjectRef] {
        match self {
            Self::Transfer(signed) => &signed.order.inputs,
            Self::Unwrap(signed) => &signed.order.inputs,
        }
    }

    pub fn domain(&self) -> &OwnedCertificateDomain {
        match self {
            Self::Transfer(signed) => &signed.order.domain,
            Self::Unwrap(signed) => &signed.order.domain,
        }
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = b"postfiat.fastpay.signed-order.v1\0".to_vec();
        match self {
            Self::Transfer(signed) => {
                bytes.push(1);
                let preimage = fastpay_transfer_lock_preimage_v1(&signed.order);
                bytes.extend_from_slice(&(preimage.len() as u64).to_be_bytes());
                bytes.extend_from_slice(&preimage);
                fastpay_commit_text(&mut bytes, &signed.order.recovery.lock_id);
                fastpay_commit_text(&mut bytes, &signed.owner_pubkey_hex);
                fastpay_commit_text(&mut bytes, &signed.owner_signature_hex);
            }
            Self::Unwrap(signed) => {
                bytes.push(2);
                let preimage = fastpay_unwrap_lock_preimage_v1(&signed.order);
                bytes.extend_from_slice(&(preimage.len() as u64).to_be_bytes());
                bytes.extend_from_slice(&preimage);
                fastpay_commit_text(&mut bytes, &signed.order.recovery.lock_id);
                fastpay_commit_text(&mut bytes, &signed.owner_pubkey_hex);
                fastpay_commit_text(&mut bytes, &signed.owner_signature_hex);
            }
        }
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryDecisionRequestV1 {
    pub schema: String,
    pub submitted_at_height: u64,
    pub signed_order: FastPaySignedOrderV1,
}

impl FastPayRecoveryDecisionRequestV1 {
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.schema != FASTPAY_RECOVERY_DECISION_REQUEST_SCHEMA_V1
            || self.submitted_at_height == 0
            || self.signed_order.inputs().is_empty()
        {
            return Err("FastPay recovery decision request shape is invalid".to_string());
        }
        validate_lower_hex_len(
            "fastpay_recovery_decision.lock_id",
            &self.signed_order.recovery().lock_id,
            96,
        )
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let mut bytes = b"postfiat.fastpay.recovery-decision-request.v1\0".to_vec();
        fastpay_commit_text(&mut bytes, &self.schema);
        bytes.extend_from_slice(&self.submitted_at_height.to_be_bytes());
        let order = self.signed_order.canonical_bytes()?;
        bytes.extend_from_slice(&(order.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&order);
        Ok(bytes)
    }
}

impl FastPayCertificateV1 {
    pub fn operation(&self) -> FastPayOperationKindV1 {
        match self {
            Self::Transfer(_) => FastPayOperationKindV1::Transfer,
            Self::Unwrap(_) => FastPayOperationKindV1::Unwrap,
        }
    }

    pub fn recovery(&self) -> &FastPayOrderRecoveryV1 {
        match self {
            Self::Transfer(certificate) => &certificate.order.recovery,
            Self::Unwrap(certificate) => &certificate.order.recovery,
        }
    }

    pub fn inputs(&self) -> &[OwnedObjectRef] {
        match self {
            Self::Transfer(certificate) => &certificate.order.inputs,
            Self::Unwrap(certificate) => &certificate.order.inputs,
        }
    }

    pub fn domain(&self) -> &OwnedCertificateDomain {
        match self {
            Self::Transfer(certificate) => &certificate.order.domain,
            Self::Unwrap(certificate) => &certificate.order.domain,
        }
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = b"postfiat.fastpay.certificate-envelope.v1\0".to_vec();
        let (signed_order, votes) = match self {
            Self::Transfer(certificate) => (
                FastPaySignedOrderV1::Transfer(SignedOwnedTransferOrderV3 {
                    order: certificate.order.clone(),
                    owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
                    owner_signature_hex: certificate.owner_signature_hex.clone(),
                }),
                certificate
                    .votes
                    .iter()
                    .map(|vote| (vote.validator_id.as_str(), vote.signature_hex.as_str()))
                    .collect::<Vec<_>>(),
            ),
            Self::Unwrap(certificate) => (
                FastPaySignedOrderV1::Unwrap(SignedOwnedUnwrapOrderV3 {
                    order: certificate.order.clone(),
                    owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
                    owner_signature_hex: certificate.owner_signature_hex.clone(),
                }),
                certificate
                    .votes
                    .iter()
                    .map(|vote| (vote.validator_id.as_str(), vote.signature_hex.as_str()))
                    .collect::<Vec<_>>(),
            ),
        };
        if votes.is_empty() || !votes.windows(2).all(|pair| pair[0].0 < pair[1].0) {
            return Err("FastPay certificate votes are not canonical".to_string());
        }
        let signed = signed_order.canonical_bytes()?;
        bytes.extend_from_slice(&(signed.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&signed);
        bytes.extend_from_slice(&(votes.len() as u64).to_be_bytes());
        for (validator_id, signature_hex) in votes {
            fastpay_commit_text(&mut bytes, validator_id);
            fastpay_commit_text(&mut bytes, signature_hex);
        }
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayApplyAckV1 {
    pub schema: String,
    pub domain: OwnedCertificateDomain,
    pub committee_epoch: u64,
    pub lock_id: String,
    pub order_digest: String,
    pub certificate_digest: String,
    pub terminal_state_digest: String,
    pub validator_id: String,
    pub signature_hex: String,
}

impl FastPayApplyAckV1 {
    pub fn validate_signing_shape(&self) -> Result<(), String> {
        if self.schema != FASTPAY_APPLY_ACK_SCHEMA_V1
            || self.domain.schema != OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
            || self.committee_epoch == 0
            || self.validator_id.is_empty()
        {
            return Err("FastPay apply acknowledgement domain is invalid".to_string());
        }
        for (field, value) in [
            ("fastpay_ack.lock_id", self.lock_id.as_str()),
            ("fastpay_ack.order_digest", self.order_digest.as_str()),
            (
                "fastpay_ack.certificate_digest",
                self.certificate_digest.as_str(),
            ),
            (
                "fastpay_ack.terminal_state_digest",
                self.terminal_state_digest.as_str(),
            ),
        ] {
            validate_lower_hex_len(field, value, 96)?;
        }
        Ok(())
    }

    pub fn validate_shape(&self) -> Result<(), String> {
        self.validate_signing_shape()?;
        if self.signature_hex.is_empty() {
            return Err("FastPay apply acknowledgement signature is empty".to_string());
        }
        validate_lower_hex_max(
            "fastpay_ack.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayRecoveryRevealV1 {
    pub schema: String,
    pub lock_id: String,
    pub order_digest: String,
    pub certificate_digest: String,
    pub revealed_at_height: u64,
    pub certificate: FastPayCertificateV1,
}

impl FastPayRecoveryRevealV1 {
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.schema != FASTPAY_RECOVERY_REVEAL_SCHEMA_V1 || self.revealed_at_height == 0 {
            return Err("FastPay recovery reveal metadata is invalid".to_string());
        }
        for (field, value) in [
            ("fastpay_reveal.lock_id", self.lock_id.as_str()),
            ("fastpay_reveal.order_digest", self.order_digest.as_str()),
            (
                "fastpay_reveal.certificate_digest",
                self.certificate_digest.as_str(),
            ),
        ] {
            validate_lower_hex_len(field, value, 96)?;
        }
        if self.certificate.recovery().lock_id != self.lock_id {
            return Err("FastPay reveal lock ID does not match its certificate".to_string());
        }
        Ok(())
    }

    /// Historical v1 bytes, also retained by the v1 application acknowledgement.
    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V1)
    }

    pub fn state_commitment_bytes_for_version(
        &self,
        version: FastPayRecoveryCommitmentVersion,
    ) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let mut bytes = match version {
            FastPayRecoveryCommitmentVersion::V1 => b"postfiat.fastpay.recovery-reveal.state.v1\0",
            FastPayRecoveryCommitmentVersion::V2 => b"postfiat.fastpay.recovery-reveal.state.v2\0",
        }
        .to_vec();
        fastpay_commit_text(&mut bytes, &self.schema);
        bytes.push(match self.certificate.operation() {
            FastPayOperationKindV1::Transfer => 1,
            FastPayOperationKindV1::Unwrap => 2,
        });
        fastpay_commit_text(&mut bytes, &self.lock_id);
        fastpay_commit_text(&mut bytes, &self.order_digest);
        fastpay_commit_text(&mut bytes, &self.certificate_digest);
        bytes.extend_from_slice(&self.revealed_at_height.to_be_bytes());
        if version == FastPayRecoveryCommitmentVersion::V2 {
            fastpay_commit_certificate(&mut bytes, &self.certificate)?;
        }
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastPayRecoveryDecisionV1 {
    Confirmed {
        order_digest: String,
        certificate_digest: String,
    },
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastPayFenceOriginV1 {
    Consensusless,
    OrderedRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastPayVersionFenceV1 {
    pub schema: String,
    pub operation: FastPayOperationKindV1,
    pub origin: FastPayFenceOriginV1,
    pub committee_epoch: u64,
    pub registry_root: String,
    pub lock_id: String,
    pub inputs: Vec<OwnedObjectRef>,
    pub decision: FastPayRecoveryDecisionV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<FastPayCertificateV1>,
    pub decided_at_height: u64,
    pub next_versions: Vec<OwnedObjectRef>,
}

impl FastPayVersionFenceV1 {
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.schema != FASTPAY_VERSION_FENCE_SCHEMA_V1
            || self.committee_epoch == 0
            || self.decided_at_height == 0
            || self.inputs.is_empty()
            || self.inputs.len() != self.next_versions.len()
        {
            return Err("FastPay version fence shape is invalid".to_string());
        }
        validate_lower_hex_len("fastpay_fence.registry_root", &self.registry_root, 96)?;
        validate_lower_hex_len("fastpay_fence.lock_id", &self.lock_id, 96)?;
        let mut seen = BTreeSet::new();
        for (input, next) in self.inputs.iter().zip(&self.next_versions) {
            if input.id != next.id
                || next.version
                    != input
                        .version
                        .checked_add(1)
                        .ok_or_else(|| "FastPay version fence input version overflow".to_string())?
                || !seen.insert((input.id.as_str(), input.version))
            {
                return Err(
                    "FastPay version fence does not advance each unique input once".to_string(),
                );
            }
        }
        match &self.decision {
            FastPayRecoveryDecisionV1::Confirmed {
                order_digest,
                certificate_digest,
            } => {
                validate_lower_hex_len("fastpay_fence.order_digest", order_digest, 96)?;
                validate_lower_hex_len("fastpay_fence.certificate_digest", certificate_digest, 96)?;
                let certificate = self.certificate.as_ref().ok_or_else(|| {
                    "confirmed FastPay fence must retain its complete certificate".to_string()
                })?;
                if certificate.operation() != self.operation
                    || certificate.recovery().lock_id != self.lock_id
                    || certificate.inputs() != self.inputs
                {
                    return Err(
                        "confirmed FastPay fence certificate does not match its lock".to_string(),
                    );
                }
            }
            FastPayRecoveryDecisionV1::Cancelled => {
                if self.certificate.is_some() {
                    return Err("cancelled FastPay fence must not retain a certificate".to_string());
                }
            }
        }
        Ok(())
    }

    /// Historical v1 bytes, also retained by the v1 application acknowledgement.
    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V1)
    }

    pub fn state_commitment_bytes_for_version(
        &self,
        version: FastPayRecoveryCommitmentVersion,
    ) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let mut bytes = match version {
            FastPayRecoveryCommitmentVersion::V1 => b"postfiat.fastpay.version-fence.state.v1\0",
            FastPayRecoveryCommitmentVersion::V2 => b"postfiat.fastpay.version-fence.state.v2\0",
        }
        .to_vec();
        fastpay_commit_text(&mut bytes, &self.schema);
        bytes.push(match self.operation {
            FastPayOperationKindV1::Transfer => 1,
            FastPayOperationKindV1::Unwrap => 2,
        });
        bytes.push(match self.origin {
            FastPayFenceOriginV1::Consensusless => 1,
            FastPayFenceOriginV1::OrderedRecovery => 2,
        });
        bytes.extend_from_slice(&self.committee_epoch.to_be_bytes());
        fastpay_commit_text(&mut bytes, &self.registry_root);
        fastpay_commit_text(&mut bytes, &self.lock_id);
        bytes.extend_from_slice(&(self.inputs.len() as u64).to_be_bytes());
        for input in &self.inputs {
            fastpay_commit_text(&mut bytes, &input.id);
            bytes.extend_from_slice(&input.version.to_be_bytes());
        }
        match &self.decision {
            FastPayRecoveryDecisionV1::Confirmed {
                order_digest,
                certificate_digest,
            } => {
                bytes.push(1);
                fastpay_commit_text(&mut bytes, order_digest);
                fastpay_commit_text(&mut bytes, certificate_digest);
            }
            FastPayRecoveryDecisionV1::Cancelled => bytes.push(2),
        }
        bytes.extend_from_slice(&self.decided_at_height.to_be_bytes());
        bytes.extend_from_slice(&(self.next_versions.len() as u64).to_be_bytes());
        for next in &self.next_versions {
            fastpay_commit_text(&mut bytes, &next.id);
            bytes.extend_from_slice(&next.version.to_be_bytes());
        }
        if version == FastPayRecoveryCommitmentVersion::V2 {
            if let Some(certificate) = &self.certificate {
                fastpay_commit_certificate(&mut bytes, certificate)?;
            }
        }
        Ok(bytes)
    }
}

fn fastpay_commit_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn fastpay_commit_certificate(
    bytes: &mut Vec<u8>,
    certificate: &FastPayCertificateV1,
) -> Result<(), String> {
    // Historical storage preserves arrival order; certificate identity already
    // orders votes by validator. Normalize the commitment without rewriting state.
    let vote_count = match certificate {
        FastPayCertificateV1::Transfer(value) => value.votes.len(),
        FastPayCertificateV1::Unwrap(value) => value.votes.len(),
    };
    if vote_count > MAX_FASTPAY_RECOVERY_VALIDATORS {
        return Err("FastPay retained certificate exceeds the validator bound".to_string());
    }
    let mut canonical = certificate.clone();
    match &mut canonical {
        FastPayCertificateV1::Transfer(value) => {
            value
                .votes
                .sort_by(|a, b| a.validator_id.cmp(&b.validator_id));
        }
        FastPayCertificateV1::Unwrap(value) => {
            value
                .votes
                .sort_by(|a, b| a.validator_id.cmp(&b.validator_id));
        }
    }
    // The strict encoder still rejects duplicate validators after sorting.
    let encoded = canonical.canonical_bytes()?;
    let length = u64::try_from(encoded.len())
        .map_err(|_| "FastPay retained certificate length exceeds u64".to_string())?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&encoded);
    Ok(())
}

fn fastpay_append_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

pub fn fastpay_transfer_lock_preimage_v1(order: &OwnedTransferOrderV3) -> Vec<u8> {
    let mut bytes = b"postfiat.fastpay.transfer-lock-preimage.v1\0".to_vec();
    fastpay_append_text(&mut bytes, &order.domain.schema);
    fastpay_append_text(&mut bytes, &order.domain.chain_id);
    fastpay_append_text(&mut bytes, &order.domain.genesis_hash);
    bytes.extend_from_slice(&order.domain.protocol_version.to_le_bytes());
    fastpay_append_text(&mut bytes, &order.domain.registry_id);
    bytes.extend_from_slice(&order.recovery.committee_epoch.to_le_bytes());
    bytes.extend_from_slice(&order.recovery.valid_from_height.to_le_bytes());
    bytes.extend_from_slice(&order.recovery.expires_at_height.to_le_bytes());
    bytes.extend_from_slice(&order.recovery.recovery_closes_at_height.to_le_bytes());
    bytes.extend_from_slice(&(order.inputs.len() as u64).to_le_bytes());
    for input in &order.inputs {
        fastpay_append_text(&mut bytes, &input.id);
        bytes.extend_from_slice(&input.version.to_le_bytes());
    }
    bytes.extend_from_slice(&(order.outputs.len() as u64).to_le_bytes());
    for output in &order.outputs {
        fastpay_append_text(&mut bytes, &output.owner_pubkey_hex);
        bytes.extend_from_slice(&output.value.to_le_bytes());
        fastpay_append_text(&mut bytes, &output.asset);
    }
    bytes.extend_from_slice(&order.fee.to_le_bytes());
    bytes.extend_from_slice(&order.nonce.to_le_bytes());
    bytes.extend_from_slice(&(order.memos.len() as u64).to_le_bytes());
    for memo in &order.memos {
        fastpay_append_text(&mut bytes, &memo.memo_type);
        fastpay_append_text(&mut bytes, &memo.memo_format);
        fastpay_append_text(&mut bytes, &memo.memo_data);
    }
    bytes
}

pub fn fastpay_unwrap_lock_preimage_v1(order: &OwnedUnwrapOrderV3) -> Vec<u8> {
    let mut bytes = b"postfiat.fastpay.unwrap-lock-preimage.v1\0".to_vec();
    fastpay_append_text(&mut bytes, &order.domain.schema);
    fastpay_append_text(&mut bytes, &order.domain.chain_id);
    fastpay_append_text(&mut bytes, &order.domain.genesis_hash);
    bytes.extend_from_slice(&order.domain.protocol_version.to_le_bytes());
    fastpay_append_text(&mut bytes, &order.domain.registry_id);
    bytes.extend_from_slice(&order.recovery.committee_epoch.to_le_bytes());
    bytes.extend_from_slice(&order.recovery.valid_from_height.to_le_bytes());
    bytes.extend_from_slice(&order.recovery.expires_at_height.to_le_bytes());
    bytes.extend_from_slice(&order.recovery.recovery_closes_at_height.to_le_bytes());
    bytes.extend_from_slice(&(order.inputs.len() as u64).to_le_bytes());
    for input in &order.inputs {
        fastpay_append_text(&mut bytes, &input.id);
        bytes.extend_from_slice(&input.version.to_le_bytes());
    }
    fastpay_append_text(&mut bytes, &order.to_address);
    bytes.extend_from_slice(&order.amount.to_le_bytes());
    fastpay_append_text(&mut bytes, &order.asset);
    bytes.extend_from_slice(&order.fee.to_le_bytes());
    bytes.extend_from_slice(&order.nonce.to_le_bytes());
    bytes.extend_from_slice(&(order.memos.len() as u64).to_le_bytes());
    for memo in &order.memos {
        fastpay_append_text(&mut bytes, &memo.memo_type);
        fastpay_append_text(&mut bytes, &memo.memo_format);
        fastpay_append_text(&mut bytes, &memo.memo_data);
    }
    bytes
}

pub fn fastpay_transfer_lock_id_v1(order: &OwnedTransferOrderV3) -> String {
    hash_hex_domain(
        FASTPAY_LOCK_ID_DOMAIN_V1,
        &fastpay_transfer_lock_preimage_v1(order),
    )
}

pub fn fastpay_unwrap_lock_id_v1(order: &OwnedUnwrapOrderV3) -> String {
    hash_hex_domain(
        FASTPAY_LOCK_ID_DOMAIN_V1,
        &fastpay_unwrap_lock_preimage_v1(order),
    )
}

#[cfg(test)]
mod fastpay_recovery_type_tests {
    use super::*;

    fn domain() -> OwnedCertificateDomain {
        OwnedCertificateDomain {
            schema: OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
            chain_id: "fastpay-recovery-types".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 3,
            registry_id: "22".repeat(48),
        }
    }

    fn policy() -> FastPayRecoveryPolicyV1 {
        FastPayRecoveryPolicyV1 {
            schema: FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
            activation_height: 90,
            max_validity_blocks: 20,
            max_recovery_blocks: 20,
        }
    }

    fn transfer() -> OwnedTransferOrderV3 {
        let mut order = OwnedTransferOrderV3 {
            domain: domain(),
            recovery: FastPayOrderRecoveryV1 {
                schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
                committee_epoch: 7,
                lock_id: "00".repeat(48),
                valid_from_height: 100,
                expires_at_height: 110,
                recovery_closes_at_height: 120,
            },
            inputs: vec![OwnedObjectRef {
                id: "33".repeat(32),
                version: 9,
            }],
            outputs: vec![OwnedOutputSpec {
                owner_pubkey_hex: "44".repeat(32),
                value: 9,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 8,
            memos: Vec::new(),
        };
        order.recovery.lock_id = fastpay_transfer_lock_id_v1(&order);
        order
    }

    fn retained_certificate() -> FastPayCertificateV1 {
        FastPayCertificateV1::Transfer(OwnedTransferCertificateV3 {
            order: transfer(),
            owner_pubkey_hex: "aa".repeat(32),
            owner_signature_hex: "bb".repeat(32),
            votes: vec![OwnedTransferVote {
                validator_id: "validator-0".to_string(),
                signature_hex: "cc".repeat(32),
            }],
        })
    }

    #[test]
    fn v2_retained_votes_accept_historical_order_and_reject_duplicates() {
        let mut certificate = retained_certificate();
        let FastPayCertificateV1::Transfer(value) = &mut certificate else {
            unreachable!()
        };
        value.votes.push(OwnedTransferVote {
            validator_id: "validator-1".into(),
            signature_hex: "dd".repeat(32),
        });
        let mut expected = Vec::new();
        fastpay_commit_certificate(&mut expected, &certificate).unwrap();
        let FastPayCertificateV1::Transfer(value) = &mut certificate else {
            unreachable!()
        };
        value.votes.reverse();
        let stored = certificate.clone();
        assert!(
            certificate.canonical_bytes().is_err(),
            "wire encoder stays strict"
        );
        let mut actual = Vec::new();
        fastpay_commit_certificate(&mut actual, &certificate).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(certificate, stored, "historical state is unchanged");
        let FastPayCertificateV1::Transfer(value) = &mut certificate else {
            unreachable!()
        };
        value.votes.push(value.votes[0].clone());
        assert!(fastpay_commit_certificate(&mut Vec::new(), &certificate).is_err());
    }

    #[test]
    fn historical_v1_committee_root_and_state_bytes_remain_stable() {
        let mut committee = FastPayRecoveryCommitteeV1::from_public_keys(
            domain().chain_id,
            domain().genesis_hash,
            domain().protocol_version,
            7,
            100,
            120,
            (0..4)
                .map(|index| (format!("validator-{index}"), "aa".repeat(32)))
                .collect(),
        )
        .unwrap();
        committee.schema = FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V1.to_string();
        // Independently reconstructed from the pre-0a1216c3 canonical encoder.
        committee.registry_root = "7994495faefce27215cfae8bbb4fb34fee43c5219da4fab2e693551f4b28fa6bcc9a197982b5f9cfdd1aa2da20399a3d".to_string();
        committee.validate().expect("historical v1 root verifies");
        let before = committee.state_commitment_bytes().unwrap();
        committee.valid_from_height += 1;
        assert_eq!(committee.computed_root().unwrap(), committee.registry_root);
        assert_ne!(
            committee.state_commitment_bytes().unwrap(),
            before,
            "v1 governance/state already committed the admission heights"
        );
        committee.schema = "postfiat-fastpay-recovery-committee-v999".into();
        assert!(
            committee.computed_root().is_err(),
            "unknown encodings fail closed"
        );
    }

    #[test]
    fn recovery_committee_root_binds_both_admission_heights() {
        let committee = FastPayRecoveryCommitteeV1::from_public_keys(
            domain().chain_id,
            domain().genesis_hash,
            domain().protocol_version,
            7,
            100,
            120,
            (0..4)
                .map(|index| (format!("validator-{index}"), "aa".repeat(32)))
                .collect(),
        )
        .expect("valid committee");
        let original_root = committee.computed_root().expect("root");
        for change in 0..2 {
            let mut changed = committee.clone();
            if change == 0 {
                changed.valid_from_height += 1;
            } else {
                changed.new_orders_through_height += 1;
            }
            assert_ne!(
                original_root,
                changed.computed_root().expect("changed root")
            );
            assert!(changed.validate().is_err());
        }
    }

    #[test]
    fn recovery_reveal_commitment_binds_retained_certificate_signatures() {
        let certificate = retained_certificate();
        let reveal = FastPayRecoveryRevealV1 {
            schema: FASTPAY_RECOVERY_REVEAL_SCHEMA_V1.to_string(),
            lock_id: certificate.recovery().lock_id.clone(),
            order_digest: "44".repeat(48),
            certificate_digest: "55".repeat(48),
            revealed_at_height: 120,
            certificate,
        };
        let original = reveal
            .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
            .expect("reveal commitment");
        for change in 0..2 {
            let mut changed = reveal.clone();
            let FastPayCertificateV1::Transfer(certificate) = &mut changed.certificate else {
                unreachable!("fixture is a transfer certificate")
            };
            if change == 0 {
                certificate.owner_signature_hex = "dd".repeat(32);
            } else {
                certificate.votes[0].signature_hex = "ee".repeat(32);
            }
            assert_ne!(
                original,
                changed
                    .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
                    .expect("changed reveal")
            );
        }
    }

    #[test]
    fn confirmed_version_fence_commitment_binds_retained_certificate() {
        let certificate = retained_certificate();
        let input = certificate.inputs()[0].clone();
        let fence = FastPayVersionFenceV1 {
            schema: FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
            operation: FastPayOperationKindV1::Transfer,
            origin: FastPayFenceOriginV1::OrderedRecovery,
            committee_epoch: 7,
            registry_root: "22".repeat(48),
            lock_id: certificate.recovery().lock_id.clone(),
            inputs: vec![input.clone()],
            decision: FastPayRecoveryDecisionV1::Confirmed {
                order_digest: "44".repeat(48),
                certificate_digest: "55".repeat(48),
            },
            certificate: Some(certificate),
            decided_at_height: 120,
            next_versions: vec![OwnedObjectRef {
                id: input.id,
                version: input.version + 1,
            }],
        };
        let original = fence
            .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
            .expect("fence commitment");
        let mut changed = fence.clone();
        let Some(FastPayCertificateV1::Transfer(certificate)) = &mut changed.certificate else {
            unreachable!("fixture is a confirmed transfer certificate")
        };
        certificate.votes[0].signature_hex = "ee".repeat(32);
        assert_ne!(
            original,
            changed
                .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
                .expect("changed fence")
        );
    }

    #[test]
    fn recovery_committee_admission_window_property_changes_root_and_rejects_stale_root() {
        let committee = FastPayRecoveryCommitteeV1::from_public_keys(
            domain().chain_id,
            domain().genesis_hash,
            domain().protocol_version,
            7,
            100,
            120,
            (0..4)
                .map(|index| (format!("validator-{index}"), "aa".repeat(32)))
                .collect(),
        )
        .expect("committee");
        let original = committee.registry_root.clone();
        for delta in 1..=32 {
            for field in 0..2 {
                let mut changed = committee.clone();
                if field == 0 {
                    changed.valid_from_height += delta;
                    changed.new_orders_through_height = changed
                        .new_orders_through_height
                        .max(changed.valid_from_height);
                } else {
                    changed.new_orders_through_height += delta;
                }
                assert_ne!(changed.computed_root().expect("new root"), original);
                assert!(changed.validate().is_err(), "stale root must reject");
                changed.registry_root = changed.computed_root().expect("new root");
                changed.validate().expect("matching root accepts");
            }
        }
        for (from, through) in [(0, 120), (121, 120)] {
            let mut changed = committee.clone();
            changed.valid_from_height = from;
            changed.new_orders_through_height = through;
            assert!(
                changed.computed_root().is_err(),
                "invalid window must reject"
            );
        }
    }

    #[test]
    fn recovery_reveal_certificate_mutation_property_changes_committed_bytes() {
        let certificate = retained_certificate();
        let reveal = FastPayRecoveryRevealV1 {
            schema: FASTPAY_RECOVERY_REVEAL_SCHEMA_V1.to_string(),
            lock_id: certificate.recovery().lock_id.clone(),
            order_digest: "44".repeat(48),
            certificate_digest: "55".repeat(48),
            revealed_at_height: 120,
            certificate,
        };
        let original = reveal
            .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
            .expect("original commitment");
        for mutation in 0..=63_u8 {
            for field in 0..2 {
                let mut changed = reveal.clone();
                let FastPayCertificateV1::Transfer(certificate) = &mut changed.certificate else {
                    unreachable!("transfer fixture")
                };
                let signature = format!("{:02x}", mutation);
                if field == 0 {
                    certificate.owner_signature_hex = signature.repeat(32);
                } else {
                    certificate.votes[0].signature_hex = signature.repeat(32);
                }
                assert_ne!(
                    changed
                        .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
                        .expect("changed commitment"),
                    original,
                    "mutation={mutation} field={field}"
                );
            }
        }
    }

    #[test]
    fn lock_id_commits_every_recovery_and_value_field() {
        let order = transfer();
        order.recovery.validate(&policy()).expect("valid recovery");
        assert_eq!(order.recovery.lock_id, fastpay_transfer_lock_id_v1(&order));
        let baseline = order.recovery.lock_id.clone();
        for mutation in 0..4 {
            let mut changed = order.clone();
            match mutation {
                0 => changed.recovery.committee_epoch += 1,
                1 => changed.recovery.expires_at_height += 1,
                2 => changed.inputs[0].version += 1,
                _ => changed.outputs[0].value += 1,
            }
            assert_ne!(baseline, fastpay_transfer_lock_id_v1(&changed));
        }
    }

    #[test]
    fn policy_and_fence_fail_closed_on_unbounded_or_nonmonotonic_state() {
        let mut order = transfer();
        order.recovery.recovery_closes_at_height = 10_001;
        assert!(order.recovery.validate(&policy()).is_err());

        let input = OwnedObjectRef {
            id: "33".repeat(32),
            version: 9,
        };
        let mut fence = FastPayVersionFenceV1 {
            schema: FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
            operation: FastPayOperationKindV1::Transfer,
            origin: FastPayFenceOriginV1::Consensusless,
            committee_epoch: 7,
            registry_root: "22".repeat(48),
            lock_id: "44".repeat(48),
            inputs: vec![input.clone()],
            decision: FastPayRecoveryDecisionV1::Cancelled,
            certificate: None,
            decided_at_height: 120,
            next_versions: vec![OwnedObjectRef {
                id: input.id.clone(),
                version: 10,
            }],
        };
        fence.validate_shape().expect("valid fence");
        fence.next_versions[0].version = 11;
        assert!(fence.validate_shape().is_err());
    }
}
