pub const FASTSWAP_SCHEMA_VERSION_V1: u32 = 1;
pub const FASTSWAP_INTENT_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastswap/intent/v1";
pub const FASTSWAP_VOTE_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastswap/vote/v1";
pub const FASTLANE_DEPOSIT_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastlane/deposit/v1";
pub const OWNED_DEPOSIT_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/owned-deposit/v1";
pub const FASTLANE_CHECKPOINT_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastlane/checkpoint/v1";
pub const FASTLANE_CONTROL_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastlane/control/v1";
pub const FASTLANE_ASSET_CONTROL_CONTEXT_V1: &[u8] =
    b"postfiat-l1-v2/fastlane/asset-control/v1";
pub const FASTLANE_EXIT_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastlane/exit/v1";
pub const FASTLANE_EXIT_VOTE_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastlane/exit-vote/v1";
pub const FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1: &str = "fastswap_v1_bootstrap:";
pub const FASTSWAP_MAX_INTENT_BYTES: usize = 128 * 1024;
pub const FASTSWAP_MAX_ASSET_INPUTS_PER_PARTY: usize = 16;
pub const FASTSWAP_MAX_FEE_INPUTS_PER_PARTY: usize = 4;
pub const FASTSWAP_MAX_OUTPUTS: usize = 8;
pub const FASTSWAP_MAX_VALIDATORS: usize = 64;
pub const FASTSWAP_MAX_STRING_BYTES: usize = 4096;
pub const FASTSWAP_ML_DSA_65: &str = "ML-DSA-65";

macro_rules! hash48_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub [u8; 48]);

        impl $name {
            pub const ZERO: Self = Self([0; 48]);
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_bytes(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let bytes = Vec::<u8>::deserialize(deserializer)?;
                let value = bytes
                    .try_into()
                    .map_err(|_| serde::de::Error::custom("expected exactly 48 bytes"))?;
                Ok(Self(value))
            }
        }
    };
}

hash48_newtype!(FastAssetIdV1);
hash48_newtype!(FastAssetRuleHashV1);
hash48_newtype!(FastSwapPolicyHashV1);
hash48_newtype!(FastSwapIntentIdV1);
hash48_newtype!(FastSwapIdV1);
hash48_newtype!(FastSwapEffectsDigestV1);
hash48_newtype!(FastSwapReceiptDigestV1);
hash48_newtype!(FastSwapCertificateDigestV1);
hash48_newtype!(FastSwapCommitteeRootV1);
hash48_newtype!(FastSwapMarketEnvelopeHashV1);
hash48_newtype!(FastSwapRfqHashV1);
hash48_newtype!(FastSwapDepositIdV1);
hash48_newtype!(FastSwapExitClaimIdV1);
hash48_newtype!(FastSwapControlCertificateIdV1);
hash48_newtype!(FastSwapOpaqueHashV1);
hash48_newtype!(FastAssetDefinitionHashV1);
hash48_newtype!(FastHolderPermitIdV1);
hash48_newtype!(FastLaneCheckpointIdV1);
hash48_newtype!(FastLaneExitIdV1);
hash48_newtype!(FastLaneExitEffectsDigestV1);
hash48_newtype!(FastSwapBootstrapIdV1);

impl FastAssetIdV1 {
    pub fn native_pft() -> Self {
        Self(hash48(b"postfiat.fastlane.asset.native_pft.v1", b"PFT"))
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct FastObjectIdV1(pub [u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastSwapCodecError {
    UnexpectedEnd,
    TrailingBytes,
    InvalidSchema(u32),
    InvalidBoolean(u8),
    InvalidEnum(&'static str, u8),
    InvalidUtf8,
    LengthExceeded(&'static str),
    NonCanonical(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FastObjectKeyV1 {
    pub object_id: FastObjectIdV1,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapChainDomainV1 {
    pub chain_id: String,
    pub genesis_hash: FastSwapOpaqueHashV1,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapCommitteeDomainV1 {
    pub chain: FastSwapChainDomainV1,
    pub fastswap_schema_version: u32,
    pub committee_epoch: u64,
    pub committee_root: FastSwapCommitteeRootV1,
    pub validator_count: u16,
    pub quorum: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapValidatorV1 {
    pub validator_id: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapCommitteeV1 {
    pub domain: FastSwapCommitteeDomainV1,
    pub validators: Vec<FastSwapValidatorV1>,
}

impl FastSwapCommitteeV1 {
    pub fn computed_root(&self) -> Result<FastSwapCommitteeRootV1, FastSwapCodecError> {
        if self.validators.len() != usize::from(self.domain.validator_count)
            || !self
                .validators
                .windows(2)
                .all(|pair| pair[0].validator_id < pair[1].validator_id)
        {
            return Err(FastSwapCodecError::NonCanonical("validator roster"));
        }
        let mut bytes = Vec::new();
        for validator in &self.validators {
            append_len_bytes(&mut bytes, validator.validator_id.as_bytes())?;
            append_len_bytes(&mut bytes, &validator.public_key)?;
        }
        Ok(FastSwapCommitteeRootV1(hash48(
            b"postfiat.fastswap.committee.v1",
            &bytes,
        )))
    }

    pub fn validate(&self) -> Result<(), FastSwapCodecError> {
        self.domain.validate()?;
        if self.computed_root()? != self.domain.committee_root {
            return Err(FastSwapCodecError::NonCanonical("committee root"));
        }
        Ok(())
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPCOMMITTEE")?;
        encode_domain(&mut encoder, &self.domain)?;
        encoder.u16(len_u16(self.validators.len(), "committee validators")?);
        for validator in &self.validators {
            encoder.string(&validator.validator_id)?;
            encoder.bytes(&validator.public_key)?;
        }
        Ok(encoder.finish())
    }
}

impl FastSwapCommitteeDomainV1 {
    pub fn validate(&self) -> Result<(), FastSwapCodecError> {
        if self.fastswap_schema_version != FASTSWAP_SCHEMA_VERSION_V1 {
            return Err(FastSwapCodecError::InvalidSchema(
                self.fastswap_schema_version,
            ));
        }
        if self.chain.chain_id.is_empty()
            || self.chain.chain_id.len() > FASTSWAP_MAX_STRING_BYTES
        {
            return Err(FastSwapCodecError::LengthExceeded("chain_id"));
        }
        let n = usize::from(self.validator_count);
        let expected = (2 * n) / 3 + 1;
        if !(4..=FASTSWAP_MAX_VALIDATORS).contains(&n)
            || usize::from(self.quorum) != expected
        {
            return Err(FastSwapCodecError::NonCanonical("committee quorum"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastAssetControlStateV1 {
    Spendable,
    Frozen {
        control_certificate_id: FastSwapControlCertificateIdV1,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastObjectOriginV1 {
    Deposit {
        deposit_id: FastSwapDepositIdV1,
    },
    FastSwapOutput {
        swap_id: FastSwapIdV1,
        output_index: u16,
    },
    FastPaymentOutput {
        certificate_id: FastSwapCertificateDigestV1,
        output_index: u16,
    },
    Change {
        operation_id: FastSwapIdV1,
        output_index: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastAssetObjectV1 {
    pub key: FastObjectKeyV1,
    pub owner_pubkey: Vec<u8>,
    pub asset_id: FastAssetIdV1,
    pub asset_rule_hash: FastAssetRuleHashV1,
    pub amount_atoms: u64,
    pub control_state: FastAssetControlStateV1,
    pub origin: FastObjectOriginV1,
}

impl FastAssetObjectV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTASSETOBJECT")?;
        encode_object(&mut encoder, self)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastAssetRuleV1 {
    pub asset_id: FastAssetIdV1,
    pub asset_definition_hash: FastAssetDefinitionHashV1,
    pub issuer_address: String,
    pub issuer_control_pubkey: Vec<u8>,
    pub requires_authorization: bool,
    pub freeze_enabled: bool,
    pub clawback_enabled: bool,
    pub fast_lane_enabled: bool,
    pub valid_from_height: u64,
    pub valid_through_height: u64,
}

impl FastAssetRuleV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTASSETRULE")?;
        encoder.fixed(&self.asset_id.0)?;
        encoder.fixed(&self.asset_definition_hash.0)?;
        encoder.string(&self.issuer_address)?;
        encoder.bytes(&self.issuer_control_pubkey)?;
        encoder.boolean(self.requires_authorization);
        encoder.boolean(self.freeze_enabled);
        encoder.boolean(self.clawback_enabled);
        encoder.boolean(self.fast_lane_enabled);
        encoder.u64(self.valid_from_height);
        encoder.u64(self.valid_through_height);
        Ok(encoder.finish())
    }

    pub fn rule_hash(&self) -> Result<FastAssetRuleHashV1, FastSwapCodecError> {
        Ok(FastAssetRuleHashV1(hash48(
            b"postfiat.fastlane.asset_rule.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastHolderPermitV1 {
    pub permit_id: FastHolderPermitIdV1,
    pub asset_id: FastAssetIdV1,
    pub owner_pubkey: Vec<u8>,
    pub valid_from_height: u64,
    pub valid_through_height: u64,
    pub consensus_receipt_digest: FastSwapOpaqueHashV1,
}

impl FastHolderPermitV1 {
    pub fn computed_id(&self) -> Result<FastHolderPermitIdV1, FastSwapCodecError> {
        if self.owner_pubkey.is_empty()
            || self.owner_pubkey.len() > FASTSWAP_MAX_INTENT_BYTES
            || self.valid_from_height > self.valid_through_height
        {
            return Err(FastSwapCodecError::NonCanonical("holder permit"));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTHOLDERPERMIT")?;
        encoder.fixed(&self.asset_id.0)?;
        encoder.bytes(&self.owner_pubkey)?;
        encoder.u64(self.valid_from_height);
        encoder.u64(self.valid_through_height);
        encoder.fixed(&self.consensus_receipt_digest.0)?;
        Ok(FastHolderPermitIdV1(hash48(
            b"postfiat.fastlane.holder_permit.v1",
            &encoder.finish(),
        )))
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        if self.computed_id()? != self.permit_id {
            return Err(FastSwapCodecError::NonCanonical("holder permit id"));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTHOLDERPERMITSTATE")?;
        encoder.fixed(&self.permit_id.0)?;
        encoder.fixed(&self.asset_id.0)?;
        encoder.bytes(&self.owner_pubkey)?;
        encoder.u64(self.valid_from_height);
        encoder.u64(self.valid_through_height);
        encoder.fixed(&self.consensus_receipt_digest.0)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastAssetControlActionV1 {
    Freeze,
    Unfreeze,
    Clawback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastAssetControlCommandV1 {
    pub domain: FastSwapCommitteeDomainV1,
    pub action: FastAssetControlActionV1,
    pub input: FastObjectKeyV1,
    pub issuer_address: String,
    pub issuer_control_pubkey: Vec<u8>,
    pub expires_at_height: u64,
    pub nonce: [u8; 32],
}

impl FastAssetControlCommandV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.domain.validate()?;
        if self.input.version == 0
            || self.issuer_address.is_empty()
            || self.issuer_address.len() > FASTSWAP_MAX_STRING_BYTES
            || self.issuer_control_pubkey.is_empty()
            || self.issuer_control_pubkey.len() > FASTSWAP_MAX_INTENT_BYTES
            || self.expires_at_height == 0
        {
            return Err(FastSwapCodecError::NonCanonical("asset control command"));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTASSETCONTROL")?;
        encode_domain(&mut encoder, &self.domain)?;
        encoder.u8(match self.action {
            FastAssetControlActionV1::Freeze => 1,
            FastAssetControlActionV1::Unfreeze => 2,
            FastAssetControlActionV1::Clawback => 3,
        });
        encoder.key(self.input)?;
        encoder.string(&self.issuer_address)?;
        encoder.bytes(&self.issuer_control_pubkey)?;
        encoder.u64(self.expires_at_height);
        encoder.fixed(&self.nonce)?;
        Ok(encoder.finish())
    }

    pub fn operation_id(&self) -> Result<FastSwapIdV1, FastSwapCodecError> {
        Ok(FastSwapIdV1(hash48(
            b"postfiat.fastlane.asset_control.operation_id.v1",
            &self.canonical_bytes()?,
        )))
    }

    pub fn intent_id(&self) -> Result<FastSwapIntentIdV1, FastSwapCodecError> {
        Ok(FastSwapIntentIdV1(hash48(
            b"postfiat.fastlane.asset_control.intent_id.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedFastAssetControlCommandV1 {
    pub command: FastAssetControlCommandV1,
    pub algorithm_id: String,
    pub signature: Vec<u8>,
}

impl SignedFastAssetControlCommandV1 {
    pub fn operation_id(&self) -> Result<FastSwapIdV1, FastSwapCodecError> {
        self.command.operation_id()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneReserveBalanceV1 {
    pub asset_id: FastAssetIdV1,
    #[serde(with = "fastlane_u128_hex_serde")]
    pub amount_atoms: u128,
}

impl FastLaneReserveBalanceV1 {
    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANERESERVE")?;
        encoder.fixed(&self.asset_id.0)?;
        encoder.u128(self.amount_atoms);
        Ok(encoder.finish())
    }
}

mod fastlane_u128_hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{value:032x}"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom(
                "expected a 32-character hexadecimal FastLane amount",
            ));
        }
        u128::from_str_radix(&value, 16).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneDepositV1 {
    pub domain: FastSwapChainDomainV1,
    pub source_address: String,
    pub source_pubkey: Vec<u8>,
    pub sequence: u64,
    pub fee_pft: u64,
    pub destination_owner_pubkey: Vec<u8>,
    pub destination_holder_permit_id: Option<FastHolderPermitIdV1>,
    pub asset_id: FastAssetIdV1,
    pub asset_rule_hash: FastAssetRuleHashV1,
    pub amount_atoms: u64,
    pub nonce: [u8; 32],
}

impl FastLaneDepositV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEDEPOSIT")?;
        encoder.string(&self.domain.chain_id)?;
        encoder.fixed(&self.domain.genesis_hash.0)?;
        encoder.u32(self.domain.protocol_version);
        encoder.string(&self.source_address)?;
        encoder.bytes(&self.source_pubkey)?;
        encoder.u64(self.sequence);
        encoder.u64(self.fee_pft);
        encoder.bytes(&self.destination_owner_pubkey)?;
        match self.destination_holder_permit_id {
            Some(permit_id) => {
                encoder.u8(1);
                encoder.fixed(&permit_id.0)?;
            }
            None => encoder.u8(0),
        }
        encoder.fixed(&self.asset_id.0)?;
        encoder.fixed(&self.asset_rule_hash.0)?;
        encoder.u64(self.amount_atoms);
        encoder.fixed(&self.nonce)?;
        Ok(encoder.finish())
    }

    pub fn deposit_id(&self) -> Result<FastSwapDepositIdV1, FastSwapCodecError> {
        Ok(FastSwapDepositIdV1(hash48(
            b"postfiat.fastlane.deposit_id.v1",
            &self.signing_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedFastLaneDepositV1 {
    pub deposit: FastLaneDepositV1,
    pub algorithm_id: String,
    pub signature: Vec<u8>,
}

/// Consensus-ordered account-to-FastPay deposit. Unlike the retired
/// `wrap_owned` RPC, every debit field is authorized by the source account and
/// the resulting object ID is derived deterministically from this envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedDepositV1 {
    pub domain: FastSwapChainDomainV1,
    pub source_address: String,
    pub source_pubkey: Vec<u8>,
    pub sequence: u64,
    pub fee_pft: u64,
    pub destination_owner_pubkey: Vec<u8>,
    pub asset: String,
    pub amount_atoms: u64,
    pub valid_through_height: u64,
    pub nonce: [u8; 32],
}

impl OwnedDepositV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFOWNEDDEPOSIT")?;
        encoder.string(&self.domain.chain_id)?;
        encoder.fixed(&self.domain.genesis_hash.0)?;
        encoder.u32(self.domain.protocol_version);
        encoder.string(&self.source_address)?;
        encoder.bytes(&self.source_pubkey)?;
        encoder.u64(self.sequence);
        encoder.u64(self.fee_pft);
        encoder.bytes(&self.destination_owner_pubkey)?;
        encoder.string(&self.asset)?;
        encoder.u64(self.amount_atoms);
        encoder.u64(self.valid_through_height);
        encoder.fixed(&self.nonce)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOwnedDepositV1 {
    pub deposit: OwnedDepositV1,
    pub algorithm_id: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneDepositReceiptV1 {
    pub deposit_id: FastSwapDepositIdV1,
    pub accepted: bool,
    pub code: String,
    pub destination_owner_pubkey: Vec<u8>,
    pub asset_id: FastAssetIdV1,
    pub asset_rule_hash: FastAssetRuleHashV1,
    pub amount_atoms: u64,
    pub initial_object_key: FastObjectKeyV1,
}

impl FastLaneDepositReceiptV1 {
    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEDEPOSITRECEIPT")?;
        encoder.fixed(&self.deposit_id.0)?;
        encoder.boolean(self.accepted);
        encoder.string(&self.code)?;
        encoder.bytes(&self.destination_owner_pubkey)?;
        encoder.fixed(&self.asset_id.0)?;
        encoder.fixed(&self.asset_rule_hash.0)?;
        encoder.u64(self.amount_atoms);
        encoder.key(self.initial_object_key)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneExitClaimV1 {
    pub exit_claim_id: FastSwapExitClaimIdV1,
    pub committee: FastSwapCommitteeDomainV1,
    pub owner_pubkey: Vec<u8>,
    pub destination_address: String,
    pub asset_id: FastAssetIdV1,
    pub asset_rule_hash: FastAssetRuleHashV1,
    pub amount_atoms: u64,
}

impl FastLaneExitClaimV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEEXITCLAIM")?;
        encoder.fixed(&self.exit_claim_id.0)?;
        encode_domain(&mut encoder, &self.committee)?;
        encoder.bytes(&self.owner_pubkey)?;
        encoder.string(&self.destination_address)?;
        encoder.fixed(&self.asset_id.0)?;
        encoder.fixed(&self.asset_rule_hash.0)?;
        encoder.u64(self.amount_atoms);
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedFastLaneRedeemV1 {
    pub claim: FastLaneExitClaimV1,
    pub exit_effects_qc: FastLaneExitCertificateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneExitIntentV1 {
    pub committee: FastSwapCommitteeDomainV1,
    pub owner_address: String,
    pub owner_pubkey: Vec<u8>,
    pub inputs: Vec<FastObjectKeyV1>,
    pub asset_id: FastAssetIdV1,
    pub asset_rule_hash: FastAssetRuleHashV1,
    pub amount_atoms: u64,
    pub destination_address: String,
    pub nonce: [u8; 32],
}

impl FastLaneExitIntentV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        if self.inputs.is_empty() || !strictly_sorted_unique(&self.inputs) {
            return Err(FastSwapCodecError::NonCanonical("exit inputs"));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEEXIT")?;
        encode_domain(&mut encoder, &self.committee)?;
        encoder.string(&self.owner_address)?;
        encoder.bytes(&self.owner_pubkey)?;
        encoder.keys(&self.inputs)?;
        encoder.fixed(&self.asset_id.0)?;
        encoder.fixed(&self.asset_rule_hash.0)?;
        encoder.u64(self.amount_atoms);
        encoder.string(&self.destination_address)?;
        encoder.fixed(&self.nonce)?;
        Ok(encoder.finish())
    }

    pub fn exit_id(&self) -> Result<FastLaneExitIdV1, FastSwapCodecError> {
        Ok(FastLaneExitIdV1(hash48(
            b"postfiat.fastlane.exit_id.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedFastLaneExitIntentV1 {
    pub intent: FastLaneExitIntentV1,
    pub algorithm_id: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneExitEffectsV1 {
    pub exit_id: FastLaneExitIdV1,
    pub consumed: Vec<FastObjectKeyV1>,
    pub claim: FastLaneExitClaimV1,
}

impl FastLaneExitEffectsV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        if self.consumed.is_empty() || !strictly_sorted_unique(&self.consumed) {
            return Err(FastSwapCodecError::NonCanonical("exit effects inputs"));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEEXITEFFECT")?;
        encoder.fixed(&self.exit_id.0)?;
        encoder.keys(&self.consumed)?;
        encoder.bytes(&self.claim.canonical_bytes()?)?;
        Ok(encoder.finish())
    }

    pub fn digest(&self) -> Result<FastLaneExitEffectsDigestV1, FastSwapCodecError> {
        Ok(FastLaneExitEffectsDigestV1(hash48(
            b"postfiat.fastlane.exit_effects.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneExitVoteV1 {
    pub committee: FastSwapCommitteeDomainV1,
    pub exit_id: FastLaneExitIdV1,
    pub effects_digest: FastLaneExitEffectsDigestV1,
    pub validator_id: String,
    pub signature: Vec<u8>,
}

impl FastLaneExitVoteV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEEXITVOTE")?;
        encode_domain(&mut encoder, &self.committee)?;
        encoder.fixed(&self.exit_id.0)?;
        encoder.fixed(&self.effects_digest.0)?;
        encoder.string(&self.validator_id)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneExitCertificateV1 {
    pub effects: FastLaneExitEffectsV1,
    pub votes: Vec<FastLaneExitVoteV1>,
}

impl FastLaneExitCertificateV1 {
    pub fn validate_canonical_order(&self) -> Result<(), FastSwapCodecError> {
        if self.votes.is_empty()
            || self.votes.len() > FASTSWAP_MAX_VALIDATORS
            || !self
                .votes
                .windows(2)
                .all(|pair| pair[0].validator_id < pair[1].validator_id)
        {
            return Err(FastSwapCodecError::NonCanonical("exit vote order"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneRedeemReceiptV1 {
    pub exit_claim_id: FastSwapExitClaimIdV1,
    pub accepted: bool,
    pub code: String,
    pub destination_address: String,
    pub asset_id: FastAssetIdV1,
    pub amount_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FastLanePrimaryOperationV1 {
    Deposit { signed: SignedFastLaneDepositV1 },
    OwnedDeposit { signed: SignedOwnedDepositV1 },
    Redeem { signed: SignedFastLaneRedeemV1 },
    AnchorCheckpoint {
        certificate: FastLaneCheckpointCertificateV1,
    },
    Control {
        certificate: FastLaneControlCertificateV1,
    },
    FastPayRecoveryReveal {
        certificate: FastPayCertificateV1,
    },
    FastPayRecoveryDecision {
        request: FastPayRecoveryDecisionRequestV1,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FastLaneControlActionV1 {
    RegisterAssetRule { rule: FastAssetRuleV1 },
    RegisterHolderPermit { permit: FastHolderPermitV1 },
    RegisterPolicy { policy: FastSwapPolicySnapshotV1 },
    StopPrepare { fence: FastLanePrepareFenceV1 },
    ActivateCommittee {
        committee: FastSwapCommitteeV1,
        final_checkpoint: FastLaneCheckpointCertificateV1,
    },
    ActivateProtocol {
        activation_height: u64,
    },
}

impl FastLaneControlActionV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANECONTROL")?;
        match self {
            Self::RegisterAssetRule { rule } => {
                encoder.u8(1);
                encoder.fixed(&rule.rule_hash()?.0)?;
            }
            Self::RegisterPolicy { policy } => {
                policy.validate()?;
                encoder.u8(2);
                encoder.fixed(&policy.policy_hash.0)?;
            }
            Self::StopPrepare { fence } => {
                encoder.u8(3);
                encoder.u64(fence.committee_epoch);
                encoder.u64(fence.policy_epoch);
                encoder.u64(fence.finalized_primary_height);
            }
            Self::ActivateCommittee {
                committee,
                final_checkpoint,
            } => {
                committee.validate()?;
                encoder.u8(4);
                encode_domain(&mut encoder, &committee.domain)?;
                encoder.fixed(&committee.domain.committee_root.0)?;
                let checkpoint = final_checkpoint
                    .votes
                    .first()
                    .ok_or(FastSwapCodecError::NonCanonical("empty final checkpoint"))?;
                encoder.fixed(&checkpoint.checkpoint.checkpoint_id()?.0)?;
            }
            Self::ActivateProtocol { activation_height } => {
                if *activation_height == 0 {
                    return Err(FastSwapCodecError::NonCanonical("activation height"));
                }
                encoder.u8(5);
                encoder.u64(*activation_height);
            }
            Self::RegisterHolderPermit { permit } => {
                if permit.computed_id()? != permit.permit_id {
                    return Err(FastSwapCodecError::NonCanonical("holder permit id"));
                }
                encoder.u8(6);
                encoder.fixed(&permit.permit_id.0)?;
            }
        }
        Ok(encoder.finish())
    }

    pub fn digest(&self) -> Result<FastSwapOpaqueHashV1, FastSwapCodecError> {
        Ok(FastSwapOpaqueHashV1(hash48(
            b"postfiat.fastlane.control_action.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneControlVoteV1 {
    pub committee: FastSwapCommitteeDomainV1,
    pub action_digest: FastSwapOpaqueHashV1,
    pub validator_id: String,
    pub signature: Vec<u8>,
}

impl FastLaneControlVoteV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANECONTROLVOTE")?;
        encode_domain(&mut encoder, &self.committee)?;
        encoder.fixed(&self.action_digest.0)?;
        encoder.string(&self.validator_id)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneControlCertificateV1 {
    pub action: FastLaneControlActionV1,
    pub votes: Vec<FastLaneControlVoteV1>,
}

impl FastLaneControlCertificateV1 {
    pub fn validate_canonical_order(&self) -> Result<(), FastSwapCodecError> {
        if self.votes.is_empty()
            || self.votes.len() > FASTSWAP_MAX_VALIDATORS
            || !self
                .votes
                .windows(2)
                .all(|pair| pair[0].validator_id < pair[1].validator_id)
        {
            return Err(FastSwapCodecError::NonCanonical("control vote order"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLanePrimaryTransactionV1 {
    pub operation: FastLanePrimaryOperationV1,
}

impl FastLanePrimaryTransactionV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEPRIMARY")?;
        match &self.operation {
            FastLanePrimaryOperationV1::Deposit { signed } => {
                encoder.u8(1);
                encoder.bytes(&signed.deposit.signing_bytes()?)?;
                encoder.string(&signed.algorithm_id)?;
                encoder.bytes(&signed.signature)?;
            }
            FastLanePrimaryOperationV1::Redeem { signed } => {
                encoder.u8(2);
                encoder.bytes(&signed.claim.canonical_bytes()?)?;
                encode_exit_certificate(&mut encoder, &signed.exit_effects_qc)?;
            }
            FastLanePrimaryOperationV1::AnchorCheckpoint { certificate } => {
                encoder.u8(3);
                encode_checkpoint_certificate(&mut encoder, certificate)?;
            }
            FastLanePrimaryOperationV1::Control { certificate } => {
                encoder.u8(4);
                encoder.bytes(&certificate.action.canonical_bytes()?)?;
                certificate.validate_canonical_order()?;
                encoder.u16(len_u16(certificate.votes.len(), "control votes")?);
                for vote in &certificate.votes {
                    encoder.bytes(&vote.signing_bytes()?)?;
                    encoder.bytes(&vote.signature)?;
                }
            }
            FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
                encoder.u8(5);
                encoder.bytes(&signed.deposit.signing_bytes()?)?;
                encoder.string(&signed.algorithm_id)?;
                encoder.bytes(&signed.signature)?;
            }
            FastLanePrimaryOperationV1::FastPayRecoveryReveal { certificate } => {
                encoder.u8(6);
                encoder.bytes(&certificate.canonical_bytes().map_err(|_| {
                    FastSwapCodecError::NonCanonical("FastPay recovery certificate")
                })?)?;
            }
            FastLanePrimaryOperationV1::FastPayRecoveryDecision { request } => {
                encoder.u8(7);
                encoder.bytes(&request.canonical_bytes().map_err(|_| {
                    FastSwapCodecError::NonCanonical("FastPay recovery decision")
                })?)?;
            }
        }
        Ok(encoder.finish())
    }

    pub fn tx_id(&self) -> Result<FastSwapOpaqueHashV1, FastSwapCodecError> {
        Ok(FastSwapOpaqueHashV1(hash48(
            b"postfiat.fastlane.primary_transaction.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapPartyV1 {
    pub owner_address: String,
    pub owner_pubkey: Vec<u8>,
    pub offered_asset_id: FastAssetIdV1,
    pub offered_asset_rule_hash: FastAssetRuleHashV1,
    pub offered_amount: u64,
    pub receives_asset_id: FastAssetIdV1,
    pub receives_asset_rule_hash: FastAssetRuleHashV1,
    pub receives_holder_permit_id: Option<FastHolderPermitIdV1>,
    pub receives_amount: u64,
    pub asset_inputs: Vec<FastObjectKeyV1>,
    pub fee_inputs: Vec<FastObjectKeyV1>,
    pub asset_change: u64,
    pub fee_change: u64,
    pub fee_burn_pft: u64,
}

impl FastSwapPartyV1 {
    pub fn canonical_order_key(&self) -> (FastAssetIdV1, &str, &[u8]) {
        (
            self.offered_asset_id,
            self.owner_address.as_str(),
            self.owner_pubkey.as_slice(),
        )
    }

    fn validate_bounds(&self) -> Result<(), FastSwapCodecError> {
        if self.owner_address.is_empty()
            || self.owner_address.len() > FASTSWAP_MAX_STRING_BYTES
            || self.owner_pubkey.is_empty()
            || self.owner_pubkey.len() > FASTSWAP_MAX_INTENT_BYTES
        {
            return Err(FastSwapCodecError::LengthExceeded("party identity"));
        }
        if self.asset_inputs.len() > FASTSWAP_MAX_ASSET_INPUTS_PER_PARTY {
            return Err(FastSwapCodecError::LengthExceeded("asset_inputs"));
        }
        if self.fee_inputs.len() > FASTSWAP_MAX_FEE_INPUTS_PER_PARTY {
            return Err(FastSwapCodecError::LengthExceeded("fee_inputs"));
        }
        if !strictly_sorted_unique(&self.asset_inputs)
            || !strictly_sorted_unique(&self.fee_inputs)
        {
            return Err(FastSwapCodecError::NonCanonical("input order"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapIntentV1 {
    pub domain: FastSwapCommitteeDomainV1,
    pub policy_hash: FastSwapPolicyHashV1,
    pub rfq_hash: FastSwapRfqHashV1,
    pub market_envelope_hash: FastSwapMarketEnvelopeHashV1,
    pub nav_epoch: u64,
    pub expires_at_height: u64,
    pub nonce: [u8; 32],
    pub party_0: FastSwapPartyV1,
    pub party_1: FastSwapPartyV1,
}

impl FastSwapIntentV1 {
    pub fn validate_canonical_shape(&self) -> Result<(), FastSwapCodecError> {
        self.domain.validate()?;
        self.party_0.validate_bounds()?;
        self.party_1.validate_bounds()?;
        if self.party_0.canonical_order_key() >= self.party_1.canonical_order_key() {
            return Err(FastSwapCodecError::NonCanonical("party order"));
        }
        if self.party_0.offered_asset_id == self.party_1.offered_asset_id
            || self.party_0.owner_pubkey == self.party_1.owner_pubkey
        {
            return Err(FastSwapCodecError::NonCanonical("party distinction"));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.validate_canonical_shape()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPINTENT")?;
        encode_domain(&mut encoder, &self.domain)?;
        encoder.fixed(&self.policy_hash.0)?;
        encoder.fixed(&self.rfq_hash.0)?;
        encoder.fixed(&self.market_envelope_hash.0)?;
        encoder.u64(self.nav_epoch);
        encoder.u64(self.expires_at_height);
        encoder.fixed(&self.nonce)?;
        encode_party(&mut encoder, &self.party_0)?;
        encode_party(&mut encoder, &self.party_1)?;
        let bytes = encoder.finish();
        if bytes.len() > FASTSWAP_MAX_INTENT_BYTES {
            return Err(FastSwapCodecError::LengthExceeded("intent"));
        }
        Ok(bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, FastSwapCodecError> {
        if bytes.len() > FASTSWAP_MAX_INTENT_BYTES {
            return Err(FastSwapCodecError::LengthExceeded("intent"));
        }
        let mut decoder = Decoder::new(bytes);
        decoder.expect_fixed(b"PFFASTSWAPINTENT")?;
        let domain = decode_domain(&mut decoder)?;
        let policy_hash = FastSwapPolicyHashV1(decoder.array()?);
        let rfq_hash = FastSwapRfqHashV1(decoder.array()?);
        let market_envelope_hash = FastSwapMarketEnvelopeHashV1(decoder.array()?);
        let nav_epoch = decoder.u64()?;
        let expires_at_height = decoder.u64()?;
        let nonce = decoder.array()?;
        let party_0 = decode_party(&mut decoder)?;
        let party_1 = decode_party(&mut decoder)?;
        decoder.finish()?;
        let value = Self {
            domain,
            policy_hash,
            rfq_hash,
            market_envelope_hash,
            nav_epoch,
            expires_at_height,
            nonce,
            party_0,
            party_1,
        };
        value.validate_canonical_shape()?;
        if value.canonical_bytes()?.as_slice() != bytes {
            return Err(FastSwapCodecError::NonCanonical("intent re-encode"));
        }
        Ok(value)
    }

    pub fn intent_id(&self) -> Result<FastSwapIntentIdV1, FastSwapCodecError> {
        Ok(FastSwapIntentIdV1(hash48(
            b"postfiat.fastswap.intent_id.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapAuthorizationV1 {
    pub role: u8,
    pub algorithm_id: String,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedFastSwapIntentV1 {
    pub intent: FastSwapIntentV1,
    pub authorization_0: FastSwapAuthorizationV1,
    pub authorization_1: FastSwapAuthorizationV1,
}

impl SignedFastSwapIntentV1 {
    pub fn swap_id(&self) -> Result<FastSwapIdV1, FastSwapCodecError> {
        let mut bytes = Vec::with_capacity(48 + self.authorization_0.public_key.len()
            + self.authorization_1.public_key.len());
        bytes.extend_from_slice(&self.intent.intent_id()?.0);
        append_len_bytes(&mut bytes, &self.authorization_0.public_key)?;
        append_len_bytes(&mut bytes, &self.authorization_1.public_key)?;
        Ok(FastSwapIdV1(hash48(
            b"postfiat.fastswap.swap_id.v1",
            &bytes,
        )))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastSwapDecisionV1 {
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastSwapPhaseV1 {
    Precommit,
    Commit,
    Effects,
    NewRound,
    CancelApply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastAssetAmountV1 {
    pub asset_id: FastAssetIdV1,
    pub amount_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapReceiptV1 {
    pub swap_id: FastSwapIdV1,
    pub accepted: bool,
    pub code: String,
    pub consumed_count: u16,
    pub created_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapEffectsV1 {
    pub domain: FastSwapCommitteeDomainV1,
    pub swap_id: FastSwapIdV1,
    pub policy_hash: FastSwapPolicyHashV1,
    pub decision: FastSwapDecisionV1,
    pub consumed: Vec<FastObjectKeyV1>,
    pub created: Vec<FastAssetObjectV1>,
    pub fee_burns: Vec<FastAssetAmountV1>,
    pub receipt: FastSwapReceiptV1,
}

impl FastSwapEffectsV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        if self.decision != FastSwapDecisionV1::Confirm
            || !strictly_sorted_unique(&self.consumed)
            || self.created.len() > FASTSWAP_MAX_OUTPUTS
        {
            return Err(FastSwapCodecError::NonCanonical("effects shape"));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPEFFECT")?;
        encode_domain(&mut encoder, &self.domain)?;
        encoder.fixed(&self.swap_id.0)?;
        encoder.fixed(&self.policy_hash.0)?;
        encoder.u8(1);
        encoder.keys(&self.consumed)?;
        encoder.u16(len_u16(self.created.len(), "created")?);
        for object in &self.created {
            encode_object(&mut encoder, object)?;
        }
        encoder.u16(len_u16(self.fee_burns.len(), "fee_burns")?);
        for burn in &self.fee_burns {
            encoder.fixed(&burn.asset_id.0)?;
            encoder.u64(burn.amount_atoms);
        }
        encoder.fixed(&self.receipt.swap_id.0)?;
        encoder.boolean(self.receipt.accepted);
        encoder.string(&self.receipt.code)?;
        encoder.u16(self.receipt.consumed_count);
        encoder.u16(self.receipt.created_count);
        Ok(encoder.finish())
    }

    pub fn digest(&self) -> Result<FastSwapEffectsDigestV1, FastSwapCodecError> {
        Ok(FastSwapEffectsDigestV1(hash48(
            b"postfiat.fastswap.effects.v1",
            &self.canonical_bytes()?,
        )))
    }
}

impl FastSwapReceiptV1 {
    pub fn digest(&self) -> Result<FastSwapReceiptDigestV1, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPRECEIPT")?;
        encoder.fixed(&self.swap_id.0)?;
        encoder.boolean(self.accepted);
        encoder.string(&self.code)?;
        encoder.u16(self.consumed_count);
        encoder.u16(self.created_count);
        Ok(FastSwapReceiptDigestV1(hash48(
            b"postfiat.fastswap.receipt.v1",
            &encoder.finish(),
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapVoteV1 {
    pub domain: FastSwapCommitteeDomainV1,
    pub swap_id: FastSwapIdV1,
    pub phase: FastSwapPhaseV1,
    pub round: u64,
    pub decision: Option<FastSwapDecisionV1>,
    pub justification_digest: Option<FastSwapCertificateDigestV1>,
    pub effects_digest: FastSwapEffectsDigestV1,
    pub receipt_digest: Option<FastSwapReceiptDigestV1>,
    pub validator_id: String,
    pub signature: Vec<u8>,
}

impl FastSwapVoteV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.domain.validate()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPVOTE")?;
        encode_domain(&mut encoder, &self.domain)?;
        encoder.fixed(&self.swap_id.0)?;
        encoder.u8(phase_code(self.phase));
        encoder.u64(self.round);
        encoder.option_u8(self.decision.map(decision_code));
        encoder.option_hash(self.justification_digest.map(|value| value.0))?;
        encoder.fixed(&self.effects_digest.0)?;
        encoder.option_hash(self.receipt_digest.map(|value| value.0))?;
        encoder.string(&self.validator_id)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapCertificateV1 {
    pub votes: Vec<FastSwapVoteV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapEquivocationEvidenceV1 {
    pub first: FastSwapVoteV1,
    pub second: FastSwapVoteV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapNewRoundVoteV1 {
    pub domain: FastSwapCommitteeDomainV1,
    pub swap_id: FastSwapIdV1,
    pub target_round: u64,
    pub highest_voted_round: u64,
    pub locked_round: Option<u64>,
    pub locked_value: Option<FastSwapDecisionV1>,
    pub locked_certificate_digest: Option<FastSwapCertificateDigestV1>,
    pub terminal_decision: Option<FastSwapDecisionV1>,
    pub terminal_certificate_digest: Option<FastSwapCertificateDigestV1>,
    pub effects_digest: FastSwapEffectsDigestV1,
    pub validator_id: String,
    pub signature: Vec<u8>,
}

impl FastSwapNewRoundVoteV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.domain.validate()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPNEWROUND")?;
        encode_domain(&mut encoder, &self.domain)?;
        encoder.fixed(&self.swap_id.0)?;
        encoder.u64(self.target_round);
        encoder.u64(self.highest_voted_round);
        encoder.option_u64(self.locked_round);
        encoder.option_u8(self.locked_value.map(decision_code));
        encoder.option_hash(self.locked_certificate_digest.map(|value| value.0))?;
        encoder.option_u8(self.terminal_decision.map(decision_code));
        encoder.option_hash(self.terminal_certificate_digest.map(|value| value.0))?;
        encoder.fixed(&self.effects_digest.0)?;
        encoder.string(&self.validator_id)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapNewRoundCertificateV1 {
    pub votes: Vec<FastSwapNewRoundVoteV1>,
}

impl FastSwapNewRoundCertificateV1 {
    pub fn validate_canonical_order(&self) -> Result<(), FastSwapCodecError> {
        if self.votes.is_empty()
            || self.votes.len() > FASTSWAP_MAX_VALIDATORS
            || !self
                .votes
                .windows(2)
                .all(|pair| pair[0].validator_id < pair[1].validator_id)
        {
            return Err(FastSwapCodecError::NonCanonical(
                "new-round certificate vote order",
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<FastSwapCertificateDigestV1, FastSwapCodecError> {
        self.validate_canonical_order()?;
        let mut bytes = Vec::new();
        for vote in &self.votes {
            append_len_bytes(&mut bytes, &vote.signing_bytes()?)?;
            append_len_bytes(&mut bytes, &vote.signature)?;
        }
        Ok(FastSwapCertificateDigestV1(hash48(
            b"postfiat.fastswap.new_round_certificate.v1",
            &bytes,
        )))
    }
}

impl FastSwapCertificateV1 {
    pub fn validate_canonical_order(&self) -> Result<(), FastSwapCodecError> {
        if self.votes.is_empty() || self.votes.len() > FASTSWAP_MAX_VALIDATORS {
            return Err(FastSwapCodecError::LengthExceeded("certificate votes"));
        }
        if !self
            .votes
            .windows(2)
            .all(|pair| pair[0].validator_id < pair[1].validator_id)
        {
            return Err(FastSwapCodecError::NonCanonical("certificate vote order"));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<FastSwapCertificateDigestV1, FastSwapCodecError> {
        self.validate_canonical_order()?;
        let mut bytes = Vec::new();
        for vote in &self.votes {
            append_len_bytes(&mut bytes, &vote.signing_bytes()?)?;
            append_len_bytes(&mut bytes, &vote.signature)?;
        }
        Ok(FastSwapCertificateDigestV1(hash48(
            b"postfiat.fastswap.certificate.v1",
            &bytes,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapProposalV1 {
    pub domain: FastSwapCommitteeDomainV1,
    pub swap_id: FastSwapIdV1,
    pub round: u64,
    pub decision: FastSwapDecisionV1,
    pub effects_digest: FastSwapEffectsDigestV1,
    pub leader_id: String,
    pub new_round_qc: Option<FastSwapNewRoundCertificateV1>,
    pub justification: Option<FastSwapCertificateV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapPolicySnapshotV1 {
    pub domain: FastSwapChainDomainV1,
    pub policy_epoch: u64,
    pub policy_hash: FastSwapPolicyHashV1,
    pub pair_asset_0: FastAssetIdV1,
    pub pair_asset_1: FastAssetIdV1,
    pub asset_rule_hash_0: FastAssetRuleHashV1,
    pub asset_rule_hash_1: FastAssetRuleHashV1,
    pub price_numerator: u128,
    pub price_denominator: u128,
    pub rounding: FastSwapQuoteRoundingV1,
    pub nav_epoch: u64,
    pub market_envelope_hash: FastSwapMarketEnvelopeHashV1,
    pub valid_from_height: u64,
    pub valid_through_height: u64,
    pub fee_schedule_hash: FastSwapOpaqueHashV1,
    pub max_inputs_per_party: u16,
    pub max_outputs: u16,
    pub paused: bool,
}

impl FastSwapPolicySnapshotV1 {
    pub fn computed_hash(&self) -> Result<FastSwapPolicyHashV1, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPPOLICY")?;
        encoder.string(&self.domain.chain_id)?;
        encoder.fixed(&self.domain.genesis_hash.0)?;
        encoder.u32(self.domain.protocol_version);
        encoder.u64(self.policy_epoch);
        encoder.fixed(&self.pair_asset_0.0)?;
        encoder.fixed(&self.pair_asset_1.0)?;
        encoder.fixed(&self.asset_rule_hash_0.0)?;
        encoder.fixed(&self.asset_rule_hash_1.0)?;
        encoder.u128(self.price_numerator);
        encoder.u128(self.price_denominator);
        encoder.u8(match self.rounding {
            FastSwapQuoteRoundingV1::Exact => 1,
            FastSwapQuoteRoundingV1::Down => 2,
        });
        encoder.u64(self.nav_epoch);
        encoder.fixed(&self.market_envelope_hash.0)?;
        encoder.u64(self.valid_from_height);
        encoder.u64(self.valid_through_height);
        encoder.fixed(&self.fee_schedule_hash.0)?;
        encoder.u16(self.max_inputs_per_party);
        encoder.u16(self.max_outputs);
        encoder.boolean(self.paused);
        Ok(FastSwapPolicyHashV1(hash48(
            b"postfiat.fastswap.policy.v1",
            &encoder.finish(),
        )))
    }

    pub fn validate(&self) -> Result<(), FastSwapCodecError> {
        if self.domain.chain_id.is_empty()
            || self.domain.chain_id.len() > FASTSWAP_MAX_STRING_BYTES
            || self.pair_asset_0 >= self.pair_asset_1
            || self.price_denominator == 0
            || self.valid_from_height > self.valid_through_height
            || self.max_inputs_per_party == 0
            || usize::from(self.max_inputs_per_party) > FASTSWAP_MAX_ASSET_INPUTS_PER_PARTY
            || self.max_outputs == 0
            || usize::from(self.max_outputs) > FASTSWAP_MAX_OUTPUTS
            || self.computed_hash()? != self.policy_hash
        {
            return Err(FastSwapCodecError::NonCanonical("policy snapshot"));
        }
        Ok(())
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPPOLICYSTATE")?;
        encoder.fixed(&self.policy_hash.0)?;
        encoder.string(&self.domain.chain_id)?;
        encoder.fixed(&self.domain.genesis_hash.0)?;
        encoder.u32(self.domain.protocol_version);
        encoder.u64(self.policy_epoch);
        encoder.fixed(&self.pair_asset_0.0)?;
        encoder.fixed(&self.pair_asset_1.0)?;
        encoder.fixed(&self.asset_rule_hash_0.0)?;
        encoder.fixed(&self.asset_rule_hash_1.0)?;
        encoder.u128(self.price_numerator);
        encoder.u128(self.price_denominator);
        encoder.u8(match self.rounding {
            FastSwapQuoteRoundingV1::Exact => 1,
            FastSwapQuoteRoundingV1::Down => 2,
        });
        encoder.u64(self.nav_epoch);
        encoder.fixed(&self.market_envelope_hash.0)?;
        encoder.u64(self.valid_from_height);
        encoder.u64(self.valid_through_height);
        encoder.fixed(&self.fee_schedule_hash.0)?;
        encoder.u16(self.max_inputs_per_party);
        encoder.u16(self.max_outputs);
        encoder.boolean(self.paused);
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapGovernanceBootstrapPayloadV1 {
    pub committee: FastSwapCommitteeV1,
    pub asset_rules: Vec<FastAssetRuleV1>,
    pub policies: Vec<FastSwapPolicySnapshotV1>,
    pub activation_height: u64,
}

impl FastSwapGovernanceBootstrapPayloadV1 {
    pub fn validate_payload(&self) -> Result<(), FastSwapCodecError> {
        self.committee.validate()?;
        if self.committee.domain.committee_epoch != 1
            || self.activation_height == 0
            || self.asset_rules.len() > FASTSWAP_MAX_OUTPUTS * 8
            || self.policies.len() > FASTSWAP_MAX_OUTPUTS * 8
        {
            return Err(FastSwapCodecError::NonCanonical(
                "FastSwap governance bootstrap bounds",
            ));
        }
        let rule_hashes = self
            .asset_rules
            .iter()
            .map(|rule| {
                if !rule.fast_lane_enabled || rule.valid_from_height > rule.valid_through_height {
                    return Err(FastSwapCodecError::NonCanonical("bootstrap asset rule"));
                }
                rule.rule_hash()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !strictly_sorted_unique(&rule_hashes) {
            return Err(FastSwapCodecError::NonCanonical(
                "bootstrap asset rule order",
            ));
        }
        for policy in &self.policies {
            policy.validate()?;
            if policy.domain != self.committee.domain.chain {
                return Err(FastSwapCodecError::NonCanonical(
                    "bootstrap policy chain domain",
                ));
            }
        }
        if !self.policies.windows(2).all(|pair| {
            (pair[0].policy_epoch, pair[0].policy_hash)
                < (pair[1].policy_epoch, pair[1].policy_hash)
        }) {
            return Err(FastSwapCodecError::NonCanonical(
                "bootstrap policy order",
            ));
        }
        Ok(())
    }

    pub fn payload_canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.validate_payload()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTSWAPBOOTSTRAP")?;
        encode_domain(&mut encoder, &self.committee.domain)?;
        encoder.u16(len_u16(self.asset_rules.len(), "bootstrap asset rules")?);
        for rule in &self.asset_rules {
            encoder.fixed(&rule.rule_hash()?.0)?;
        }
        encoder.u16(len_u16(self.policies.len(), "bootstrap policies")?);
        for policy in &self.policies {
            encoder.fixed(&policy.policy_hash.0)?;
        }
        encoder.u64(self.activation_height);
        Ok(encoder.finish())
    }

    pub fn bootstrap_id(&self) -> Result<FastSwapBootstrapIdV1, FastSwapCodecError> {
        Ok(FastSwapBootstrapIdV1(hash48(
            b"postfiat.fastswap.governance_bootstrap.v1",
            &self.payload_canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapGovernanceBootstrapV1 {
    pub amendment: GovernanceAmendment,
    pub payload: FastSwapGovernanceBootstrapPayloadV1,
}

impl FastSwapGovernanceBootstrapV1 {
    pub fn validate_payload(&self) -> Result<(), FastSwapCodecError> {
        self.payload.validate_payload()
    }

    pub fn payload_canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.payload.payload_canonical_bytes()
    }

    pub fn bootstrap_id(&self) -> Result<FastSwapBootstrapIdV1, FastSwapCodecError> {
        self.payload.bootstrap_id()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastSwapQuoteRoundingV1 {
    Exact,
    Down,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapReservationV1 {
    pub swap_id: FastSwapIdV1,
    pub intent_id: FastSwapIntentIdV1,
    pub effects_digest: FastSwapEffectsDigestV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapTerminalTombstoneV1 {
    pub swap_id: FastSwapIdV1,
    pub decision: FastSwapDecisionV1,
    pub decision_certificate_digest: FastSwapCertificateDigestV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastSwapLocalStatusV1 {
    Prepared,
    DecisionLocked,
    DecidedConfirm,
    Applied,
    DecidedCancel,
    Cancelled,
    Superseded,
    Checkpointed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapRecordV1 {
    pub swap_id: FastSwapIdV1,
    pub intent_id: FastSwapIntentIdV1,
    pub effects_digest: FastSwapEffectsDigestV1,
    pub expires_at_height: u64,
    pub status: FastSwapLocalStatusV1,
    pub highest_precommit_round: u64,
    pub highest_new_round_vote: u64,
    pub decision_lock_round: Option<u64>,
    pub decision_lock_value: Option<FastSwapDecisionV1>,
    pub lock_certificate_digest: Option<FastSwapCertificateDigestV1>,
    pub decision_certificate_digest: Option<FastSwapCertificateDigestV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneStateV1 {
    pub schema_version: u32,
    pub committee: FastSwapCommitteeDomainV1,
    pub objects: BTreeMap<FastObjectKeyV1, FastAssetObjectV1>,
    pub reservations: BTreeMap<FastObjectKeyV1, FastSwapReservationV1>,
    pub swaps: BTreeMap<FastSwapIdV1, FastSwapRecordV1>,
    pub imported_deposits: BTreeSet<FastSwapDepositIdV1>,
    pub exit_claims: BTreeMap<FastSwapExitClaimIdV1, FastLaneExitClaimV1>,
    pub terminal_tombstones: BTreeMap<FastSwapIdV1, FastSwapTerminalTombstoneV1>,
    pub asset_rules: BTreeMap<FastAssetRuleHashV1, FastAssetRuleV1>,
    pub holder_permits: BTreeMap<FastHolderPermitIdV1, FastHolderPermitV1>,
    pub policy_snapshots: BTreeMap<FastSwapPolicyHashV1, FastSwapPolicySnapshotV1>,
    pub prepare_fences: BTreeMap<u64, FastLanePrepareFenceV1>,
    pub pending_fee_burns: BTreeMap<FastAssetIdV1, u128>,
    #[serde(default)]
    pub anchored_checkpoints: BTreeSet<FastLaneCheckpointIdV1>,
}

impl FastLaneStateV1 {
    pub fn empty(committee: FastSwapCommitteeDomainV1) -> Self {
        Self {
            schema_version: FASTSWAP_SCHEMA_VERSION_V1,
            committee,
            objects: BTreeMap::new(),
            reservations: BTreeMap::new(),
            swaps: BTreeMap::new(),
            imported_deposits: BTreeSet::new(),
            exit_claims: BTreeMap::new(),
            terminal_tombstones: BTreeMap::new(),
            asset_rules: BTreeMap::new(),
            holder_permits: BTreeMap::new(),
            policy_snapshots: BTreeMap::new(),
            prepare_fences: BTreeMap::new(),
            pending_fee_burns: BTreeMap::new(),
            anchored_checkpoints: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLanePrepareFenceV1 {
    pub committee_epoch: u64,
    pub policy_epoch: u64,
    pub finalized_primary_height: u64,
}

impl FastLanePrepareFenceV1 {
    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANEPREPAREFENCE")?;
        encoder.u64(self.committee_epoch);
        encoder.u64(self.policy_epoch);
        encoder.u64(self.finalized_primary_height);
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapCapabilitiesV1 {
    pub schema: String,
    pub enabled: bool,
    pub committee: FastSwapCommitteeDomainV1,
    pub phases: Vec<FastSwapPhaseV1>,
    pub terminal_receipt_code: String,
    #[serde(default)]
    pub wire_codecs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapStatusResponseV1 {
    pub schema: String,
    pub swap_id: FastSwapIdV1,
    pub record: Option<FastSwapRecordV1>,
    pub terminal_tombstone: Option<FastSwapTerminalTombstoneV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapEffectsResponseV1 {
    pub schema: String,
    pub swap_id: FastSwapIdV1,
    pub effects: Option<FastSwapEffectsV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapVoteEvidenceResponseV1 {
    pub schema: String,
    pub validator_id: String,
    pub swap_id: FastSwapIdV1,
    pub phase: FastSwapPhaseV1,
    pub round: u64,
    pub vote: Option<FastSwapVoteV1>,
    pub new_round_vote: Option<FastSwapNewRoundVoteV1>,
    #[serde(default)]
    pub certificate: Option<FastSwapCertificateV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapPreviewResponseV1 {
    pub schema: String,
    pub validator_id: String,
    pub committee: FastSwapCommitteeDomainV1,
    pub effects: FastSwapEffectsV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapObjectsResponseV1 {
    pub schema: String,
    pub validator_id: String,
    pub committee: FastSwapCommitteeDomainV1,
    pub objects: Vec<FastAssetObjectV1>,
    pub next_cursor: Option<FastObjectKeyV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapPolicyResponseV1 {
    pub schema: String,
    pub validator_id: String,
    pub policy: Option<FastSwapPolicySnapshotV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneCheckpointV1 {
    pub previous_checkpoint_id: Option<FastLaneCheckpointIdV1>,
    pub committee: FastSwapCommitteeDomainV1,
    pub live_object_root: FastSwapOpaqueHashV1,
    pub live_object_totals: Vec<FastLaneReserveBalanceV1>,
    pub exit_claim_root: FastSwapOpaqueHashV1,
    pub exit_claim_totals: Vec<FastLaneReserveBalanceV1>,
    pub pending_fee_burn_totals: Vec<FastLaneReserveBalanceV1>,
    pub terminal_root: FastSwapOpaqueHashV1,
    pub highest_wal_sequence: u64,
    pub active_policy_hashes: Vec<FastSwapPolicyHashV1>,
    pub imported_deposit_root: FastSwapOpaqueHashV1,
    pub redeemed_exit_claim_root: FastSwapOpaqueHashV1,
    pub drain_ready: bool,
    pub fenced_policy_epochs: Vec<u64>,
}

impl FastLaneCheckpointV1 {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        if !self
            .live_object_totals
            .windows(2)
            .all(|pair| pair[0].asset_id < pair[1].asset_id)
            || !self
                .exit_claim_totals
                .windows(2)
                .all(|pair| pair[0].asset_id < pair[1].asset_id)
            || !self
                .pending_fee_burn_totals
                .windows(2)
                .all(|pair| pair[0].asset_id < pair[1].asset_id)
            || !strictly_sorted_unique(&self.active_policy_hashes)
            || !strictly_sorted_unique(&self.fenced_policy_epochs)
        {
            return Err(FastSwapCodecError::NonCanonical("checkpoint ordering"));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANECHECKPOINT")?;
        encoder.option_hash(self.previous_checkpoint_id.map(|value| value.0))?;
        encode_domain(&mut encoder, &self.committee)?;
        encoder.fixed(&self.live_object_root.0)?;
        encode_asset_totals(&mut encoder, &self.live_object_totals)?;
        encoder.fixed(&self.exit_claim_root.0)?;
        encode_asset_totals(&mut encoder, &self.exit_claim_totals)?;
        encode_asset_totals(&mut encoder, &self.pending_fee_burn_totals)?;
        encoder.fixed(&self.terminal_root.0)?;
        encoder.u64(self.highest_wal_sequence);
        encoder.u16(len_u16(self.active_policy_hashes.len(), "policy hashes")?);
        for hash in &self.active_policy_hashes {
            encoder.fixed(&hash.0)?;
        }
        encoder.fixed(&self.imported_deposit_root.0)?;
        encoder.fixed(&self.redeemed_exit_claim_root.0)?;
        encoder.boolean(self.drain_ready);
        encoder.u16(len_u16(self.fenced_policy_epochs.len(), "fenced policy epochs")?);
        for epoch in &self.fenced_policy_epochs {
            encoder.u64(*epoch);
        }
        Ok(encoder.finish())
    }

    pub fn checkpoint_id(&self) -> Result<FastLaneCheckpointIdV1, FastSwapCodecError> {
        Ok(FastLaneCheckpointIdV1(hash48(
            b"postfiat.fastlane.checkpoint.v1",
            &self.canonical_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneCheckpointVoteV1 {
    pub checkpoint: FastLaneCheckpointV1,
    pub validator_id: String,
    pub signature: Vec<u8>,
}

impl FastLaneCheckpointVoteV1 {
    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        let mut bytes = self.checkpoint.canonical_bytes()?;
        append_len_bytes(&mut bytes, self.validator_id.as_bytes())?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneCheckpointCertificateV1 {
    pub votes: Vec<FastLaneCheckpointVoteV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLaneCheckpointStatusV1 {
    pub schema: String,
    pub checkpoint: FastLaneCheckpointV1,
    pub vote: FastLaneCheckpointVoteV1,
    pub drain_ready: bool,
    pub rotation_ready: bool,
}

impl FastLaneCheckpointCertificateV1 {
    pub fn validate_canonical_order(&self) -> Result<(), FastSwapCodecError> {
        if self.votes.is_empty()
            || self.votes.len() > FASTSWAP_MAX_VALIDATORS
            || !self
                .votes
                .windows(2)
                .all(|pair| pair[0].validator_id < pair[1].validator_id)
        {
            return Err(FastSwapCodecError::NonCanonical("checkpoint vote order"));
        }
        Ok(())
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.validate_canonical_order()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFFASTLANECHECKPOINTCERT")?;
        encoder.u16(len_u16(self.votes.len(), "checkpoint votes")?);
        for vote in &self.votes {
            encoder.bytes(&vote.signing_bytes()?)?;
            encoder.bytes(&vote.signature)?;
        }
        Ok(encoder.finish())
    }
}

fn strictly_sorted_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn hash48(domain: &[u8], bytes: &[u8]) -> [u8; 48] {
    let mut hasher = Sha3_384::new();
    hasher.update(domain);
    hasher.update([0_u8]);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn len_u16(value: usize, field: &'static str) -> Result<u16, FastSwapCodecError> {
    value
        .try_into()
        .map_err(|_| FastSwapCodecError::LengthExceeded(field))
}

fn append_len_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), FastSwapCodecError> {
    let len: u32 = bytes
        .len()
        .try_into()
        .map_err(|_| FastSwapCodecError::LengthExceeded("bytes"))?;
    output.extend_from_slice(&len.to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn encode_asset_totals(
    encoder: &mut Encoder,
    totals: &[FastLaneReserveBalanceV1],
) -> Result<(), FastSwapCodecError> {
    encoder.u16(len_u16(totals.len(), "asset totals")?);
    for total in totals {
        encoder.fixed(&total.asset_id.0)?;
        encoder.u128(total.amount_atoms);
    }
    Ok(())
}

fn encode_exit_certificate(
    encoder: &mut Encoder,
    certificate: &FastLaneExitCertificateV1,
) -> Result<(), FastSwapCodecError> {
    certificate.validate_canonical_order()?;
    encoder.bytes(&certificate.effects.canonical_bytes()?)?;
    encoder.u16(len_u16(certificate.votes.len(), "exit votes")?);
    for vote in &certificate.votes {
        encoder.bytes(&vote.signing_bytes()?)?;
        encoder.bytes(&vote.signature)?;
    }
    Ok(())
}

fn encode_checkpoint_certificate(
    encoder: &mut Encoder,
    certificate: &FastLaneCheckpointCertificateV1,
) -> Result<(), FastSwapCodecError> {
    certificate.validate_canonical_order()?;
    encoder.u16(len_u16(certificate.votes.len(), "checkpoint votes")?);
    for vote in &certificate.votes {
        encoder.bytes(&vote.signing_bytes()?)?;
        encoder.bytes(&vote.signature)?;
    }
    Ok(())
}

fn decision_code(value: FastSwapDecisionV1) -> u8 {
    match value {
        FastSwapDecisionV1::Confirm => 1,
        FastSwapDecisionV1::Cancel => 2,
    }
}

fn phase_code(value: FastSwapPhaseV1) -> u8 {
    match value {
        FastSwapPhaseV1::Precommit => 1,
        FastSwapPhaseV1::Commit => 2,
        FastSwapPhaseV1::Effects => 3,
        FastSwapPhaseV1::NewRound => 4,
        FastSwapPhaseV1::CancelApply => 5,
    }
}

struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
    fn fixed(&mut self, value: &[u8]) -> Result<(), FastSwapCodecError> {
        if self.bytes.len().saturating_add(value.len()) > FASTSWAP_MAX_INTENT_BYTES {
            return Err(FastSwapCodecError::LengthExceeded("canonical bytes"));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }
    fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }
    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    fn u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), FastSwapCodecError> {
        let length: u32 = value
            .len()
            .try_into()
            .map_err(|_| FastSwapCodecError::LengthExceeded("bytes"))?;
        self.u32(length);
        self.fixed(value)
    }
    fn string(&mut self, value: &str) -> Result<(), FastSwapCodecError> {
        if value.len() > FASTSWAP_MAX_STRING_BYTES {
            return Err(FastSwapCodecError::LengthExceeded("string"));
        }
        self.bytes(value.as_bytes())
    }
    fn key(&mut self, key: FastObjectKeyV1) -> Result<(), FastSwapCodecError> {
        self.fixed(&key.object_id.0)?;
        self.u64(key.version);
        Ok(())
    }
    fn keys(&mut self, keys: &[FastObjectKeyV1]) -> Result<(), FastSwapCodecError> {
        self.u16(len_u16(keys.len(), "keys")?);
        for key in keys {
            self.key(*key)?;
        }
        Ok(())
    }
    fn option_u8(&mut self, value: Option<u8>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.u8(value);
            }
            None => self.u8(0),
        }
    }
    fn option_u64(&mut self, value: Option<u64>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.u64(value);
            }
            None => self.u8(0),
        }
    }
    fn option_hash(&mut self, value: Option<[u8; 48]>) -> Result<(), FastSwapCodecError> {
        match value {
            Some(value) => {
                self.u8(1);
                self.fixed(&value)?;
            }
            None => self.u8(0),
        }
        Ok(())
    }
}

struct Decoder<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Decoder<'bytes> {
    fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, length: usize) -> Result<&'bytes [u8], FastSwapCodecError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(FastSwapCodecError::UnexpectedEnd)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(FastSwapCodecError::UnexpectedEnd)?;
        self.offset = end;
        Ok(value)
    }
    fn expect_fixed(&mut self, expected: &[u8]) -> Result<(), FastSwapCodecError> {
        if self.take(expected.len())? != expected {
            return Err(FastSwapCodecError::NonCanonical("domain tag"));
        }
        Ok(())
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], FastSwapCodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FastSwapCodecError::UnexpectedEnd)
    }
    fn u8(&mut self) -> Result<u8, FastSwapCodecError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, FastSwapCodecError> {
        Ok(u16::from_be_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, FastSwapCodecError> {
        Ok(u32::from_be_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, FastSwapCodecError> {
        Ok(u64::from_be_bytes(self.array()?))
    }
    fn bytes(&mut self, maximum: usize) -> Result<Vec<u8>, FastSwapCodecError> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| FastSwapCodecError::LengthExceeded("bytes"))?;
        if length > maximum {
            return Err(FastSwapCodecError::LengthExceeded("bytes"));
        }
        Ok(self.take(length)?.to_vec())
    }
    fn string(&mut self, maximum: usize) -> Result<String, FastSwapCodecError> {
        String::from_utf8(self.bytes(maximum)?).map_err(|_| FastSwapCodecError::InvalidUtf8)
    }
    fn key(&mut self) -> Result<FastObjectKeyV1, FastSwapCodecError> {
        Ok(FastObjectKeyV1 {
            object_id: FastObjectIdV1(self.array()?),
            version: self.u64()?,
        })
    }
    fn keys(&mut self, maximum: usize) -> Result<Vec<FastObjectKeyV1>, FastSwapCodecError> {
        let length = usize::from(self.u16()?);
        if length > maximum {
            return Err(FastSwapCodecError::LengthExceeded("keys"));
        }
        (0..length).map(|_| self.key()).collect()
    }
    fn finish(self) -> Result<(), FastSwapCodecError> {
        if self.offset != self.bytes.len() {
            return Err(FastSwapCodecError::TrailingBytes);
        }
        Ok(())
    }
}

fn encode_domain(
    encoder: &mut Encoder,
    domain: &FastSwapCommitteeDomainV1,
) -> Result<(), FastSwapCodecError> {
    domain.validate()?;
    encoder.string(&domain.chain.chain_id)?;
    encoder.fixed(&domain.chain.genesis_hash.0)?;
    encoder.u32(domain.chain.protocol_version);
    encoder.u32(domain.fastswap_schema_version);
    encoder.u64(domain.committee_epoch);
    encoder.fixed(&domain.committee_root.0)?;
    encoder.u16(domain.validator_count);
    encoder.u16(domain.quorum);
    Ok(())
}

fn decode_domain(
    decoder: &mut Decoder<'_>,
) -> Result<FastSwapCommitteeDomainV1, FastSwapCodecError> {
    let domain = FastSwapCommitteeDomainV1 {
        chain: FastSwapChainDomainV1 {
            chain_id: decoder.string(FASTSWAP_MAX_STRING_BYTES)?,
            genesis_hash: FastSwapOpaqueHashV1(decoder.array()?),
            protocol_version: decoder.u32()?,
        },
        fastswap_schema_version: decoder.u32()?,
        committee_epoch: decoder.u64()?,
        committee_root: FastSwapCommitteeRootV1(decoder.array()?),
        validator_count: decoder.u16()?,
        quorum: decoder.u16()?,
    };
    domain.validate()?;
    Ok(domain)
}

fn encode_party(
    encoder: &mut Encoder,
    party: &FastSwapPartyV1,
) -> Result<(), FastSwapCodecError> {
    party.validate_bounds()?;
    encoder.string(&party.owner_address)?;
    encoder.bytes(&party.owner_pubkey)?;
    encoder.fixed(&party.offered_asset_id.0)?;
    encoder.fixed(&party.offered_asset_rule_hash.0)?;
    encoder.u64(party.offered_amount);
    encoder.fixed(&party.receives_asset_id.0)?;
    encoder.fixed(&party.receives_asset_rule_hash.0)?;
    encoder.option_hash(party.receives_holder_permit_id.map(|value| value.0))?;
    encoder.u64(party.receives_amount);
    encoder.keys(&party.asset_inputs)?;
    encoder.keys(&party.fee_inputs)?;
    encoder.u64(party.asset_change);
    encoder.u64(party.fee_change);
    encoder.u64(party.fee_burn_pft);
    Ok(())
}

fn decode_party(decoder: &mut Decoder<'_>) -> Result<FastSwapPartyV1, FastSwapCodecError> {
    let owner_address = decoder.string(FASTSWAP_MAX_STRING_BYTES)?;
    let owner_pubkey = decoder.bytes(FASTSWAP_MAX_INTENT_BYTES)?;
    let offered_asset_id = FastAssetIdV1(decoder.array()?);
    let offered_asset_rule_hash = FastAssetRuleHashV1(decoder.array()?);
    let offered_amount = decoder.u64()?;
    let receives_asset_id = FastAssetIdV1(decoder.array()?);
    let receives_asset_rule_hash = FastAssetRuleHashV1(decoder.array()?);
    let receives_holder_permit_id = match decoder.u8()? {
        0 => None,
        1 => Some(FastHolderPermitIdV1(decoder.array()?)),
        value => return Err(FastSwapCodecError::InvalidBoolean(value)),
    };
    let value = FastSwapPartyV1 {
        owner_address,
        owner_pubkey,
        offered_asset_id,
        offered_asset_rule_hash,
        offered_amount,
        receives_asset_id,
        receives_asset_rule_hash,
        receives_holder_permit_id,
        receives_amount: decoder.u64()?,
        asset_inputs: decoder.keys(FASTSWAP_MAX_ASSET_INPUTS_PER_PARTY)?,
        fee_inputs: decoder.keys(FASTSWAP_MAX_FEE_INPUTS_PER_PARTY)?,
        asset_change: decoder.u64()?,
        fee_change: decoder.u64()?,
        fee_burn_pft: decoder.u64()?,
    };
    value.validate_bounds()?;
    Ok(value)
}

fn encode_object(
    encoder: &mut Encoder,
    object: &FastAssetObjectV1,
) -> Result<(), FastSwapCodecError> {
    encoder.key(object.key)?;
    encoder.bytes(&object.owner_pubkey)?;
    encoder.fixed(&object.asset_id.0)?;
    encoder.fixed(&object.asset_rule_hash.0)?;
    encoder.u64(object.amount_atoms);
    match object.control_state {
        FastAssetControlStateV1::Spendable => encoder.u8(0),
        FastAssetControlStateV1::Frozen {
            control_certificate_id,
        } => {
            encoder.u8(1);
            encoder.fixed(&control_certificate_id.0)?;
        }
    }
    match object.origin {
        FastObjectOriginV1::Deposit { deposit_id } => {
            encoder.u8(0);
            encoder.fixed(&deposit_id.0)?;
        }
        FastObjectOriginV1::FastSwapOutput {
            swap_id,
            output_index,
        } => {
            encoder.u8(1);
            encoder.fixed(&swap_id.0)?;
            encoder.u16(output_index);
        }
        FastObjectOriginV1::FastPaymentOutput {
            certificate_id,
            output_index,
        } => {
            encoder.u8(2);
            encoder.fixed(&certificate_id.0)?;
            encoder.u16(output_index);
        }
        FastObjectOriginV1::Change {
            operation_id,
            output_index,
        } => {
            encoder.u8(3);
            encoder.fixed(&operation_id.0)?;
            encoder.u16(output_index);
        }
    }
    Ok(())
}

#[cfg(test)]
mod fastswap_types_tests {
    use super::*;

    fn key(byte: u8) -> FastObjectKeyV1 {
        FastObjectKeyV1 {
            object_id: FastObjectIdV1([byte; 32]),
            version: 1,
        }
    }

    fn party(owner: &str, pubkey: u8, offered: u8, receives: u8) -> FastSwapPartyV1 {
        FastSwapPartyV1 {
            owner_address: owner.to_owned(),
            owner_pubkey: vec![pubkey; 64],
            offered_asset_id: FastAssetIdV1([offered; 48]),
            offered_asset_rule_hash: FastAssetRuleHashV1([offered + 10; 48]),
            offered_amount: 8,
            receives_asset_id: FastAssetIdV1([receives; 48]),
            receives_asset_rule_hash: FastAssetRuleHashV1([receives + 10; 48]),
            receives_holder_permit_id: None,
            receives_amount: 1,
            asset_inputs: vec![key(pubkey)],
            fee_inputs: vec![key(pubkey + 20)],
            asset_change: 2,
            fee_change: 9,
            fee_burn_pft: 1,
        }
    }

    fn intent() -> FastSwapIntentV1 {
        let mut first = party("pf-a", 1, 1, 2);
        let mut second = party("pf-b", 2, 2, 1);
        second.offered_amount = first.receives_amount;
        second.receives_amount = first.offered_amount;
        first.receives_asset_rule_hash = second.offered_asset_rule_hash;
        second.receives_asset_rule_hash = first.offered_asset_rule_hash;
        FastSwapIntentV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: "postfiat-test".to_owned(),
                    genesis_hash: FastSwapOpaqueHashV1([3; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: 1,
                committee_epoch: 7,
                committee_root: FastSwapCommitteeRootV1([4; 48]),
                validator_count: 6,
                quorum: 5,
            },
            policy_hash: FastSwapPolicyHashV1([5; 48]),
            rfq_hash: FastSwapRfqHashV1([6; 48]),
            market_envelope_hash: FastSwapMarketEnvelopeHashV1([7; 48]),
            nav_epoch: 59,
            expires_at_height: 100,
            nonce: [8; 32],
            party_0: first,
            party_1: second,
        }
    }

    #[test]
    fn committee_domain_requires_exact_bft_quorum_for_every_supported_size() {
        for validator_count in 4usize..=FASTSWAP_MAX_VALIDATORS {
            let mut domain = intent().domain;
            domain.validator_count = u16::try_from(validator_count).expect("validator count");
            domain.quorum = u16::try_from((2 * validator_count) / 3 + 1).expect("quorum");
            assert_eq!(domain.validate(), Ok(()));

            let mut below = domain.clone();
            below.quorum -= 1;
            assert_eq!(
                below.validate(),
                Err(FastSwapCodecError::NonCanonical("committee quorum"))
            );

            let mut above = domain;
            above.quorum += 1;
            assert_eq!(
                above.validate(),
                Err(FastSwapCodecError::NonCanonical("committee quorum"))
            );
        }

        for validator_count in [0u16, 1, 2, 3, 65, u16::MAX] {
            let mut domain = intent().domain;
            domain.validator_count = validator_count;
            domain.quorum = 1;
            assert_eq!(
                domain.validate(),
                Err(FastSwapCodecError::NonCanonical("committee quorum"))
            );
        }
    }

    #[test]
    fn intent_encoding_is_canonical_and_rejects_trailing_bytes() {
        let intent = intent();
        let encoded = intent.canonical_bytes().expect("canonical intent");
        assert_eq!(
            FastSwapIntentV1::decode_canonical(&encoded).expect("decode"),
            intent
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            FastSwapIntentV1::decode_canonical(&trailing),
            Err(FastSwapCodecError::TrailingBytes)
        );
    }

    #[test]
    fn fastswap_intent_conformance_vector_is_frozen() {
        let intent = intent();
        let encoded = intent.canonical_bytes().expect("canonical intent");
        assert_eq!(encoded.len(), 1127);
        assert_eq!(
            hex(&intent.intent_id().expect("id").0),
            "b66cf7d768f3cb0a39f278ab1332dad09f6b4951efb40f45aa72c9cab37f3c2d5a5c41291834ac39ac7c57034370bf17"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(DIGITS[(byte >> 4) as usize] as char);
            output.push(DIGITS[(byte & 0x0f) as usize] as char);
        }
        output
    }

    #[test]
    fn intent_hash_changes_for_every_economic_domain_class() {
        let baseline = intent();
        let baseline_id = baseline.intent_id().expect("id");
        let mut cases = Vec::new();
        let mut changed = baseline.clone();
        changed.expires_at_height += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.party_0.offered_amount += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.party_1.receives_amount += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.policy_hash.0[0] ^= 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.domain.committee_epoch += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.party_0.asset_inputs[0].version += 1;
        cases.push(changed);
        for changed in cases {
            assert_ne!(changed.intent_id().expect("changed id"), baseline_id);
        }
    }

    #[test]
    fn owned_deposit_encoding_binds_every_authorization_and_economic_field() {
        let baseline = OwnedDepositV1 {
            domain: FastSwapChainDomainV1 {
                chain_id: "postfiat-owned-deposit-vector".to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([81; 48]),
                protocol_version: 1,
            },
            source_address: "pf-source".to_owned(),
            source_pubkey: vec![82; 64],
            sequence: 7,
            fee_pft: 2,
            destination_owner_pubkey: vec![83; 64],
            asset: "PFT".to_owned(),
            amount_atoms: 40,
            valid_through_height: 100,
            nonce: [84; 32],
        };
        let baseline_bytes = baseline.signing_bytes().expect("baseline bytes");
        assert!(baseline_bytes.starts_with(b"PFOWNEDDEPOSIT"));

        let mut cases = Vec::new();
        let mut changed = baseline.clone();
        changed.domain.chain_id.push('x');
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.domain.genesis_hash.0[0] ^= 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.domain.protocol_version += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.source_address.push('x');
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.source_pubkey[0] ^= 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.sequence += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.fee_pft += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.destination_owner_pubkey[0] ^= 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.asset.push('x');
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.amount_atoms += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.valid_through_height += 1;
        cases.push(changed);
        let mut changed = baseline.clone();
        changed.nonce[0] ^= 1;
        cases.push(changed);

        for changed in cases {
            assert_ne!(
                changed.signing_bytes().expect("changed bytes"),
                baseline_bytes
            );
        }

        let transaction = FastLanePrimaryTransactionV1 {
            operation: FastLanePrimaryOperationV1::OwnedDeposit {
                signed: SignedOwnedDepositV1 {
                    deposit: baseline,
                    algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
                    signature: vec![85; 96],
                },
            },
        };
        let transaction_bytes = transaction.canonical_bytes().expect("transaction bytes");
        assert!(transaction_bytes.starts_with(b"PFFASTLANEPRIMARY"));
        assert_eq!(transaction_bytes[b"PFFASTLANEPRIMARY".len()], 5);
    }

    #[test]
    fn duplicate_or_unsorted_inputs_fail_closed() {
        let mut duplicate = intent();
        duplicate.party_0.asset_inputs.push(duplicate.party_0.asset_inputs[0]);
        assert_eq!(
            duplicate.canonical_bytes(),
            Err(FastSwapCodecError::NonCanonical("input order"))
        );
    }
}
