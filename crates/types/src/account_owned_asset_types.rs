#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub address: String,
    pub balance: u64,
    pub sequence: u64,
    pub public_key_hex: Option<String>,
}

/// A FastPay-style owned-value object (UTXO): single owner, single-consumption
/// per version. Spending an object at version v retires v and mints fresh output
/// versions. Part of the consensusless owned-value lane (M2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedObject {
    /// Hex of the 32-byte object id.
    pub id: String,
    pub version: u64,
    /// Hex of the ML-DSA-65 public key authorized to spend this object.
    pub owner_pubkey_hex: String,
    pub value: u64,
    pub asset: String,
}

/// Reference to a consumed input owned object at a specific version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedObjectRef {
    /// Hex of the 32-byte object id.
    pub id: String,
    pub version: u64,
}

/// Specification of a created output owned object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedOutputSpec {
    pub owner_pubkey_hex: String,
    pub value: u64,
    pub asset: String,
}

pub const OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2: &str = "postfiat-owned-certificate-domain-v2";

/// Chain and committee identity covered by every FastPay owner authorization
/// and validator vote. A certificate from another chain, genesis, protocol
/// version, or validator registry must never be replayable here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedCertificateDomain {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_id: String,
}

/// A FastPay owned-value transfer order: consume inputs at their current
/// versions, create outputs, burn a fee. Authorized off-chain by the input
/// owner + a 2f+1 validator certificate (the consensusless fast path); the
/// on-chain execution applies the certified order to `LedgerState.owned_objects`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedTransferOrder {
    pub domain: OwnedCertificateDomain,
    pub inputs: Vec<OwnedObjectRef>,
    pub outputs: Vec<OwnedOutputSpec>,
    pub fee: u64,
    pub nonce: u64,
    /// XRPL-style signed memos (memo_type / memo_format / memo_data, hex,
    /// bounded by MAX_PAYMENT_MEMOS + per-field byte caps). Covered by the
    /// order's signature; metadata only — the apply ignores them; they are
    /// indexed in history like account-lane payment memos.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memos: Vec<PaymentMemo>,
}

/// A validator's signed vote on an owned-transfer order (the consensusless
/// fast-path "lock + sign" step).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedTransferVote {
    pub validator_id: String,
    pub signature_hex: String,
}

/// The consensusless certificate: the order + owner authorization + a quorum of
/// validator votes. Self-authenticating — on-chain apply must verify this (owner
/// auth + >=quorum valid votes) before applying, so a bare submitted order is
/// never trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedTransferCertificate {
    pub order: OwnedTransferOrder,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
    pub votes: Vec<OwnedTransferVote>,
}

/// Owner-authorized transfer submitted before a validator acquires input locks
/// and emits its vote. A validator must never lock from a bare order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOwnedTransferOrder {
    pub order: OwnedTransferOrder,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
}

pub const VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1: &str = "postfiat.vault_bridge.route_profile.v1";
pub const VAULT_BRIDGE_ROUTE_PROFILE_HASH_DOMAIN_V1: &str =
    "postfiat.vault_bridge.route_profile_hash.v1";
pub const VAULT_BRIDGE_ROUTE_BINDING_DOMAIN_V1: &str = "postfiat.vault_bridge.route_binding.v1";
pub const VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED: &str = "independently-observed";
pub const VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN: &str = "receipt-proven";
pub const RETIRED_VAULT_BRIDGE_ADDRESS: &str = "0x1a15e6103d6af4e88924f748e13b829d3948dea9";

/// Complete public route authority stored in replicated state. Signed
/// governance binds its canonical hash, while clients discover this full
/// preimage from the chain rather than supplying it through configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeRouteProfileV1 {
    pub schema: String,
    pub route_id: String,
    pub asset_id: String,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub route_epoch: u32,
    pub verifier_kind: String,
    pub evidence_tier: String,
    /// Verifier-specific policy committed by proof public values. Empty for
    /// observer-quorum routes; exact 32-byte lowercase hex for SP1 routes.
    pub verifier_policy_hash: String,
    /// SP1 program verifying-key hash. Empty for observer-quorum routes.
    pub verifier_program_vkey: String,
    /// Proof encoding selected by this route. Empty for observer routes.
    pub verifier_proof_encoding: String,
    pub max_proof_bytes: u64,
    pub max_public_values_bytes: u64,
    pub max_snapshot_age_blocks: u64,
    pub challenge_window_blocks: u64,
    pub max_epoch_gap_blocks: u64,
    pub settle_deadline_blocks: u64,
    pub min_challenge_bond: u64,
    pub min_attestations: u64,
    pub minimum_confirmations: u64,
    pub activation_height: u64,
    pub expires_at_height: u64,
}

impl VaultBridgeRouteProfileV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1 {
            return Err("vault bridge route profile schema mismatch".to_string());
        }
        if self.route_id.is_empty()
            || self.route_id.len() > 64
            || !self.route_id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(
                "vault bridge route profile route_id must be 1..64 lowercase identifier bytes"
                    .to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_route_profile.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.source_chain_id == 0 {
            return Err("vault bridge route profile source_chain_id must be nonzero".to_string());
        }
        validate_evm_address_text(
            "vault_bridge_route_profile.vault_address",
            &self.vault_address,
        )?;
        validate_evm_address_text(
            "vault_bridge_route_profile.token_address",
            &self.token_address,
        )?;
        if self.vault_address == RETIRED_VAULT_BRIDGE_ADDRESS {
            return Err("vault bridge route profile identifies a retired vault".to_string());
        }
        validate_vault_bridge_runtime_code_hash(
            "vault_bridge_route_profile.vault_runtime_code_hash",
            &self.vault_runtime_code_hash,
        )?;
        validate_vault_bridge_runtime_code_hash(
            "vault_bridge_route_profile.token_runtime_code_hash",
            &self.token_runtime_code_hash,
        )?;
        if self.route_epoch == 0 {
            return Err("vault bridge route profile route_epoch must be nonzero".to_string());
        }
        let expected_tier = match self.verifier_kind.as_str() {
            NAV_PROFILE_VERIFIER_MULTI_FETCH => {
                if self.min_attestations == 0 || self.minimum_confirmations == 0 {
                    return Err("independently observed route profile requires nonzero attestation and confirmation thresholds".to_string());
                }
                if !self.verifier_policy_hash.is_empty()
                    || !self.verifier_program_vkey.is_empty()
                    || !self.verifier_proof_encoding.is_empty()
                    || self.max_proof_bytes != 0
                    || self.max_public_values_bytes != 0
                {
                    return Err(
                        "independently observed route profile must not carry proof-verifier fields"
                            .to_string(),
                    );
                }
                VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED
            }
            NAV_PROFILE_VERIFIER_SP1_GROTH16 => {
                if self.min_attestations != 0 || self.minimum_confirmations != 0 {
                    return Err(
                        "receipt-proven route profile must not require observer attestations or confirmations"
                            .to_string(),
                    );
                }
                validate_lower_hex_len(
                    "vault_bridge_route_profile.verifier_policy_hash",
                    &self.verifier_policy_hash,
                    NAV_SP1_POLICY_HASH_HEX_LEN,
                )?;
                if self.verifier_program_vkey.len() != NAV_SP1_PROGRAM_VKEY_HEX_LEN
                    || !self.verifier_program_vkey.starts_with("0x")
                    || !self.verifier_program_vkey[2..]
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit())
                {
                    return Err("receipt-proven route profile verifier_program_vkey must be a 0x-prefixed 32-byte hex string".to_string());
                }
                if self.verifier_proof_encoding != NAV_SP1_PROOF_ENCODING_GROTH16 {
                    return Err(format!(
                        "receipt-proven route profile verifier_proof_encoding must be {NAV_SP1_PROOF_ENCODING_GROTH16}"
                    ));
                }
                VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN
            }
            _ => return Err("vault bridge route profile verifier_kind is unsupported".to_string()),
        };
        if self.evidence_tier != expected_tier {
            return Err(
                "vault bridge route profile evidence_tier does not match verifier_kind".to_string(),
            );
        }
        if self.max_snapshot_age_blocks == 0
            || self.challenge_window_blocks == 0
            || self.max_epoch_gap_blocks == 0
            || self.settle_deadline_blocks == 0
        {
            return Err("vault bridge route profile timing bounds must be nonzero".to_string());
        }
        if self.activation_height == 0 || self.expires_at_height <= self.activation_height {
            return Err(
                "vault bridge route profile expiry must follow nonzero activation height"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub fn canonical_preimage(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!(
            "schema={}\nroute_id={}\nasset_id={}\nsource_chain_id={}\nvault_address={}\nvault_runtime_code_hash={}\ntoken_address={}\ntoken_runtime_code_hash={}\nroute_epoch={}\nverifier_kind={}\nevidence_tier={}\nverifier_policy_hash={}\nverifier_program_vkey={}\nverifier_proof_encoding={}\nmax_proof_bytes={}\nmax_public_values_bytes={}\nmax_snapshot_age_blocks={}\nchallenge_window_blocks={}\nmax_epoch_gap_blocks={}\nsettle_deadline_blocks={}\nmin_challenge_bond={}\nmin_attestations={}\nminimum_confirmations={}\nactivation_height={}\nexpires_at_height={}\n",
            self.schema,
            self.route_id,
            self.asset_id,
            self.source_chain_id,
            self.vault_address,
            self.vault_runtime_code_hash,
            self.token_address,
            self.token_runtime_code_hash,
            self.route_epoch,
            self.verifier_kind,
            self.evidence_tier,
            self.verifier_policy_hash,
            self.verifier_program_vkey,
            self.verifier_proof_encoding,
            self.max_proof_bytes,
            self.max_public_values_bytes,
            self.max_snapshot_age_blocks,
            self.challenge_window_blocks,
            self.max_epoch_gap_blocks,
            self.settle_deadline_blocks,
            self.min_challenge_bond,
            self.min_attestations,
            self.minimum_confirmations,
            self.activation_height,
            self.expires_at_height,
        ))
    }

    pub fn profile_hash(&self) -> Result<String, String> {
        Ok(hash_hex_domain(
            VAULT_BRIDGE_ROUTE_PROFILE_HASH_DOMAIN_V1,
            self.canonical_preimage()?.as_bytes(),
        ))
    }

    pub fn source_domain(&self) -> String {
        format!(
            "erc20_bridge_vault:{}:{}:{}",
            self.source_chain_id, self.vault_address, self.token_address
        )
    }
}

fn validate_vault_bridge_runtime_code_hash(field: &str, value: &str) -> Result<(), String> {
    let Some(hex) = value.strip_prefix("0x") else {
        return Err(format!(
            "{field} must be a 0x-prefixed lowercase 32-byte hash"
        ));
    };
    validate_lower_hex_len(field, hex, 64)?;
    if hex.bytes().all(|byte| byte == b'0') {
        return Err(format!("{field} must be nonzero"));
    }
    Ok(())
}

/// A FastPay owned-value unwrap order: consume owned objects at their current
/// versions, credit an account address, mint optional owned change, and burn a
/// fee. Authorized by the input owner plus validator quorum, matching the
/// owned-transfer certificate model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedUnwrapOrder {
    pub domain: OwnedCertificateDomain,
    pub inputs: Vec<OwnedObjectRef>,
    pub to_address: String,
    pub amount: u64,
    pub asset: String,
    pub fee: u64,
    pub nonce: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memos: Vec<PaymentMemo>,
}

/// A validator's signed vote on an owned-unwrap order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedUnwrapVote {
    pub validator_id: String,
    pub signature_hex: String,
}

/// The consensusless unwrap certificate: the order + owner authorization + a
/// quorum of validator votes. Apply must verify this before moving value from
/// the owned lane back to an account balance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedUnwrapCertificate {
    pub order: OwnedUnwrapOrder,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
    pub votes: Vec<OwnedUnwrapVote>,
}

/// Owner-authorized unwrap submitted before a validator acquires input locks
/// and emits its vote. It shares the transfer lock domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOwnedUnwrapOrder {
    pub order: OwnedUnwrapOrder,
    pub owner_pubkey_hex: String,
    pub owner_signature_hex: String,
}

impl Account {
    pub fn new(address: impl Into<String>, balance: u64, public_key_hex: Option<String>) -> Self {
        Self {
            address: address.into(),
            balance,
            sequence: 0,
            public_key_hex,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetDefinition {
    pub asset_id: String,
    pub issuer: String,
    pub code: String,
    pub version: u32,
    pub precision: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_supply: Option<u64>,
    #[serde(default)]
    pub requires_authorization: bool,
    #[serde(default)]
    pub freeze_enabled: bool,
    #[serde(default)]
    pub clawback_enabled: bool,
}

impl AssetDefinition {
    pub fn new(
        chain_id: &str,
        issuer: impl Into<String>,
        code: impl Into<String>,
        version: u32,
        precision: u8,
    ) -> Result<Self, String> {
        let issuer = issuer.into();
        let code = code.into();
        let asset_id = issued_asset_id(chain_id, &issuer, &code, version)?;
        let asset = Self {
            asset_id,
            issuer,
            code,
            version,
            precision,
            display_name: String::new(),
            max_supply: None,
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
        };
        asset.validate_for_chain(chain_id)?;
        Ok(asset)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("asset.asset_id", &self.asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
        validate_text_field("asset.issuer", &self.issuer)?;
        validate_issued_asset_code(&self.code)?;
        if self.version == 0 {
            return Err("asset.version must be nonzero".to_string());
        }
        if self.precision > MAX_ISSUED_ASSET_PRECISION {
            return Err(format!(
                "asset.precision must not exceed {MAX_ISSUED_ASSET_PRECISION}"
            ));
        }
        validate_optional_text_field(
            "asset.display_name",
            &self.display_name,
            MAX_ISSUED_ASSET_DISPLAY_NAME_BYTES,
        )?;
        if self.max_supply == Some(0) {
            return Err("asset.max_supply must be nonzero when present".to_string());
        }
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected_asset_id = issued_asset_id(chain_id, &self.issuer, &self.code, self.version)?;
        if self.asset_id != expected_asset_id {
            return Err(
                "asset.asset_id does not match chain, issuer, code, and version".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustLine {
    pub trustline_id: String,
    pub account: String,
    pub issuer: String,
    pub asset_id: String,
    pub limit: u64,
    #[serde(default)]
    pub balance: u64,
    #[serde(default)]
    pub authorized: bool,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub reserve_paid: u64,
}

impl TrustLine {
    pub fn new(
        account: impl Into<String>,
        issuer: impl Into<String>,
        asset_id: impl Into<String>,
        limit: u64,
        reserve_paid: u64,
    ) -> Result<Self, String> {
        let account = account.into();
        let issuer = issuer.into();
        let asset_id = asset_id.into();
        let trustline_id = trustline_id(&account, &issuer, &asset_id)?;
        let line = Self {
            trustline_id,
            account,
            issuer,
            asset_id,
            limit,
            balance: 0,
            authorized: false,
            frozen: false,
            reserve_paid,
        };
        line.validate()?;
        Ok(line)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "trustline.trustline_id",
            &self.trustline_id,
            TRUSTLINE_ID_HEX_LEN,
        )?;
        validate_text_field("trustline.account", &self.account)?;
        validate_text_field("trustline.issuer", &self.issuer)?;
        if self.account == self.issuer {
            return Err("trustline.account must differ from trustline.issuer".to_string());
        }
        validate_lower_hex_len(
            "trustline.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.limit == 0 {
            return Err("trustline.limit must be nonzero".to_string());
        }
        if self.balance > self.limit {
            return Err("trustline.balance must not exceed trustline.limit".to_string());
        }
        let expected_trustline_id = trustline_id(&self.account, &self.issuer, &self.asset_id)?;
        if self.trustline_id != expected_trustline_id {
            return Err(
                "trustline.trustline_id does not match account, issuer, and asset".to_string(),
            );
        }
        Ok(())
    }
}

pub fn issued_asset_id(
    chain_id: &str,
    issuer: &str,
    code: &str,
    version: u32,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_text_field("asset.issuer", issuer)?;
    validate_issued_asset_code(code)?;
    if version == 0 {
        return Err("asset.version must be nonzero".to_string());
    }
    let preimage = format!(
        "chain_id={chain_id}\nissuer={issuer}\ncode_bytes={}\ncode={code}\nversion={version}\n",
        code.len()
    );
    Ok(hash_hex_domain(ISSUED_ASSET_ID_DOMAIN, preimage.as_bytes()))
}

pub fn trustline_id(account: &str, issuer: &str, asset_id: &str) -> Result<String, String> {
    validate_text_field("trustline.account", account)?;
    validate_text_field("trustline.issuer", issuer)?;
    if account == issuer {
        return Err("trustline.account must differ from trustline.issuer".to_string());
    }
    validate_lower_hex_len("trustline.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    let preimage = format!("account={account}\nissuer={issuer}\nasset_id={asset_id}\n");
    Ok(hash_hex_domain(TRUSTLINE_ID_DOMAIN, preimage.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavTrackedAsset {
    pub asset_id: String,
    pub issuer: String,
    pub reserve_operator: String,
    pub proof_profile: String,
    pub valuation_unit: String,
    pub redemption_account: String,
    #[serde(default)]
    pub finalized_epoch: u64,
    #[serde(default)]
    pub nav_per_unit: u64,
    #[serde(default)]
    pub circulating_supply: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub finalized_reserve_packet_hash: String,
    #[serde(default)]
    pub halted: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub halt_reason: String,
    /// Block height at which the active reserve packet finalized; input to
    /// the deadman switch. 0 means no packet has finalized under a
    /// height-aware profile.
    #[serde(default)]
    pub finalized_at_height: u64,
}

impl NavTrackedAsset {
    pub fn new(
        asset_id: impl Into<String>,
        issuer: impl Into<String>,
        reserve_operator: impl Into<String>,
        proof_profile: impl Into<String>,
        valuation_unit: impl Into<String>,
        redemption_account: impl Into<String>,
    ) -> Result<Self, String> {
        let asset = Self {
            asset_id: asset_id.into(),
            issuer: issuer.into(),
            reserve_operator: reserve_operator.into(),
            proof_profile: proof_profile.into(),
            valuation_unit: valuation_unit.into(),
            redemption_account: redemption_account.into(),
            finalized_epoch: 0,
            nav_per_unit: 0,
            circulating_supply: 0,
            finalized_reserve_packet_hash: String::new(),
            halted: false,
            halt_reason: String::new(),
            finalized_at_height: 0,
        };
        asset.validate()?;
        Ok(asset)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "nav_asset.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_text_field("nav_asset.issuer", &self.issuer)?;
        validate_text_field("nav_asset.reserve_operator", &self.reserve_operator)?;
        validate_text_field("nav_asset.proof_profile", &self.proof_profile)?;
        validate_text_field("nav_asset.valuation_unit", &self.valuation_unit)?;
        validate_text_field("nav_asset.redemption_account", &self.redemption_account)?;
        if !self.finalized_reserve_packet_hash.is_empty() {
            validate_lower_hex_len(
                "nav_asset.finalized_reserve_packet_hash",
                &self.finalized_reserve_packet_hash,
                NAV_RESERVE_PACKET_ID_HEX_LEN,
            )?;
        }
        validate_optional_text_field("nav_asset.halt_reason", &self.halt_reason, 128)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavReservePacket {
    pub packet_id: String,
    pub asset_id: String,
    pub issuer: String,
    pub submitter: String,
    pub epoch: u64,
    pub nav_per_unit: u64,
    pub circulating_supply: u64,
    pub verified_net_assets: u64,
    pub proof_profile: String,
    pub source_root: String,
    pub attestor_root: String,
    pub reserve_packet_hash: String,
    #[serde(default)]
    pub state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub challenge_hash: String,
    /// Block height at which this packet was submitted; input to the
    /// challenge-window and staleness checks. 0 for legacy packets.
    #[serde(default)]
    pub submitted_at_height: u64,
    /// For ledger-transparent profiles: the on-ledger accounts whose
    /// native balances back this packet. Consensus recomputes their sum
    /// and requires it to equal verified_net_assets at submit time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reserve_accounts: Vec<String>,
    /// Bonded challenge bookkeeping. Empty/zero when unchallenged.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub challenger: String,
    #[serde(default)]
    pub challenge_bond: u64,
    /// Observer attestations for multi-fetch profiles. One per attestor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attestations: Vec<NavReserveAttestation>,
    /// Groth16 proof calldata for sp1-groth16 profiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sp1_proof_bytes: Vec<u8>,
    /// SP1 public-values blob committed by the Groth16 proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sp1_public_values: Vec<u8>,
}

/// A registered observation operator. Registration is open but
/// identity-bearing: a domain (verified out-of-band, VHS-style), an
/// optional escrowed bond, and a registration height. Multi-fetch
/// attestations are only accepted from registered attestors, which is
/// what makes min_attestations Sybil-resistant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavAttestor {
    pub address: String,
    pub domain: String,
    #[serde(default)]
    pub bond: u64,
    #[serde(default)]
    pub registered_at_height: u64,
}

impl NavAttestor {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_attestor.address", &self.address)?;
        validate_text_field("nav_attestor.domain", &self.domain)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavReserveAttestation {
    pub attestor: String,
    pub pass: bool,
    pub observation_root: String,
    #[serde(default)]
    pub attested_at_height: u64,
}

impl NavReserveAttestation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_attestation.attestor", &self.attestor)?;
        validate_lower_hex_len(
            "nav_attestation.observation_root",
            &self.observation_root,
            96,
        )?;
        Ok(())
    }
}

impl NavReservePacket {
    pub fn new(
        asset_id: impl Into<String>,
        issuer: impl Into<String>,
        submitter: impl Into<String>,
        epoch: u64,
        nav_per_unit: u64,
        circulating_supply: u64,
        verified_net_assets: u64,
        proof_profile: impl Into<String>,
        source_root: impl Into<String>,
        attestor_root: impl Into<String>,
        reserve_packet_hash: impl Into<String>,
    ) -> Result<Self, String> {
        let asset_id = asset_id.into();
        let reserve_packet_hash = reserve_packet_hash.into();
        let packet_id = nav_reserve_packet_id(&asset_id, epoch, &reserve_packet_hash)?;
        let packet = Self {
            packet_id,
            asset_id,
            issuer: issuer.into(),
            submitter: submitter.into(),
            epoch,
            nav_per_unit,
            circulating_supply,
            verified_net_assets,
            proof_profile: proof_profile.into(),
            source_root: source_root.into(),
            attestor_root: attestor_root.into(),
            reserve_packet_hash,
            state: NAV_RESERVE_STATE_SUBMITTED.to_string(),
            challenge_hash: String::new(),
            submitted_at_height: 0,
            reserve_accounts: Vec::new(),
            challenger: String::new(),
            challenge_bond: 0,
            attestations: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        };
        packet.validate()?;
        Ok(packet)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "nav_reserve.packet_id",
            &self.packet_id,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "nav_reserve.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_text_field("nav_reserve.issuer", &self.issuer)?;
        validate_text_field("nav_reserve.submitter", &self.submitter)?;
        if self.epoch == 0 {
            return Err("nav_reserve.epoch must be nonzero".to_string());
        }
        if self.nav_per_unit == 0 {
            return Err("nav_reserve.nav_per_unit must be nonzero".to_string());
        }
        validate_text_field("nav_reserve.proof_profile", &self.proof_profile)?;
        validate_lower_hex_len("nav_reserve.source_root", &self.source_root, 96)?;
        validate_lower_hex_len("nav_reserve.attestor_root", &self.attestor_root, 96)?;
        validate_lower_hex_len(
            "nav_reserve.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        validate_nav_reserve_state(&self.state)?;
        if !self.challenge_hash.is_empty() {
            validate_lower_hex_len("nav_reserve.challenge_hash", &self.challenge_hash, 96)?;
        }
        if self.reserve_accounts.len() > MAX_NAV_RESERVE_ACCOUNTS {
            return Err(format!(
                "nav_reserve.reserve_accounts exceeds maximum of {MAX_NAV_RESERVE_ACCOUNTS}"
            ));
        }
        for account in &self.reserve_accounts {
            validate_text_field("nav_reserve.reserve_accounts entry", account)?;
        }
        if !self.challenger.is_empty() {
            validate_text_field("nav_reserve.challenger", &self.challenger)?;
        }
        if self.attestations.len() > MAX_NAV_ATTESTATIONS_PER_PACKET {
            return Err(format!(
                "nav_reserve.attestations exceeds maximum of {MAX_NAV_ATTESTATIONS_PER_PACKET}"
            ));
        }
        for attestation in &self.attestations {
            attestation.validate()?;
        }
        let expected_packet_id =
            nav_reserve_packet_id(&self.asset_id, self.epoch, &self.reserve_packet_hash)?;
        if self.packet_id != expected_packet_id {
            return Err(
                "nav_reserve.packet_id does not match asset, epoch, and reserve packet hash"
                    .to_string(),
            );
        }
        if self.sp1_proof_bytes.len() > DEFAULT_MAX_NAV_SP1_PROOF_BYTES as usize {
            return Err(format!(
                "nav_reserve.sp1_proof_bytes exceeds maximum of {DEFAULT_MAX_NAV_SP1_PROOF_BYTES}"
            ));
        }
        if self.sp1_public_values.len() > DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES as usize {
            return Err(format!(
                "nav_reserve.sp1_public_values exceeds maximum of {DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES}"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositEvidence {
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub depositor: String,
    pub pftl_recipient: String,
    pub pftl_recipient_hash: String,
    pub amount_atoms: u64,
    pub nonce: String,
    /// Keccak-256 commitment to the exact governed route-profile hash and
    /// epoch selected by the depositor. Empty is accepted only for decoding
    /// historical v1 deposits; governed live ingress requires this field.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub route_binding: String,
    pub deposit_id: String,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
}

impl VaultBridgeDepositEvidence {
    pub fn source_domain(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX,
            self.source_chain_id,
            self.vault_address,
            self.token_address
        )
    }

    pub fn source_asset_ref(&self) -> String {
        format!("erc20:{}:{}", self.source_chain_id, self.token_address)
    }

    pub fn source_tx_or_attestation(&self) -> String {
        format!(
            "{}:{}",
            VAULT_BRIDGE_DEPOSIT_SOURCE_TX_PREFIX, self.deposit_id
        )
    }

    pub fn finality_ref(&self) -> String {
        format!(
            "evm_log:{}:{}:{}:{}",
            self.source_chain_id, self.block_hash, self.tx_hash, self.log_index
        )
    }

    pub fn vault_id(&self) -> String {
        format!(
            "evm:{}:{}:{}",
            self.source_chain_id, self.vault_address, self.token_address
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.source_chain_id == 0 {
            return Err("vault_bridge_deposit.source_chain_id must be nonzero".to_string());
        }
        validate_evm_address_text("vault_bridge_deposit.vault_address", &self.vault_address)?;
        validate_evm_address_text("vault_bridge_deposit.token_address", &self.token_address)?;
        validate_evm_address_text("vault_bridge_deposit.depositor", &self.depositor)?;
        validate_text_field("vault_bridge_deposit.pftl_recipient", &self.pftl_recipient)?;
        validate_lower_hex_len(
            "vault_bridge_deposit.pftl_recipient_hash",
            &self.pftl_recipient_hash,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_deposit.amount_atoms must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_deposit.nonce",
            &self.nonce,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        if !self.route_binding.is_empty() {
            validate_lower_hex_len(
                "vault_bridge_deposit.route_binding",
                &self.route_binding,
                VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
            )?;
            if self.route_binding.bytes().all(|byte| byte == b'0') {
                return Err("vault_bridge_deposit.route_binding must be nonzero".to_string());
            }
        }
        validate_lower_hex_len(
            "vault_bridge_deposit.deposit_id",
            &self.deposit_id,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit.block_hash",
            &self.block_hash,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit.tx_hash",
            &self.tx_hash,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        let expected_recipient_hash = vault_bridge_pftl_recipient_hash(&self.pftl_recipient)?;
        if self.pftl_recipient_hash != expected_recipient_hash {
            return Err(
                "vault_bridge_deposit.pftl_recipient_hash does not match pftl_recipient"
                    .to_string(),
            );
        }
        let expected_deposit_id = vault_bridge_deposit_id(self)?;
        if self.deposit_id != expected_deposit_id {
            return Err(
                "vault_bridge_deposit.deposit_id does not match ERC20BridgeVault depositId preimage"
                    .to_string(),
            );
        }
        validate_text_field("vault_bridge_deposit.source_domain", &self.source_domain())?;
        validate_text_field(
            "vault_bridge_deposit.source_tx_or_attestation",
            &self.source_tx_or_attestation(),
        )?;
        validate_text_field("vault_bridge_deposit.finality_ref", &self.finality_ref())?;
        validate_text_field("vault_bridge_deposit.vault_id", &self.vault_id())?;
        Ok(())
    }

    fn canonical_preimage(&self) -> String {
        let mut preimage = format!(
            "source_chain_id={}\nvault_address={}\ntoken_address={}\ndepositor={}\npftl_recipient_bytes={}\npftl_recipient={}\npftl_recipient_hash={}\namount_atoms={}\nnonce={}\n",
            self.source_chain_id,
            self.vault_address,
            self.token_address,
            self.depositor,
            self.pftl_recipient.len(),
            self.pftl_recipient,
            self.pftl_recipient_hash,
            self.amount_atoms,
            self.nonce,
        );
        if !self.route_binding.is_empty() {
            preimage.push_str(&format!("route_binding={}\n", self.route_binding));
        }
        preimage.push_str(&format!(
            "deposit_id={}\nblock_hash={}\ntx_hash={}\nlog_index={}\nsource_domain={}\nsource_tx_or_attestation={}\nfinality_ref={}\nvault_id={}\n",
            self.deposit_id,
            self.block_hash,
            self.tx_hash,
            self.log_index,
            self.source_domain(),
            self.source_tx_or_attestation(),
            self.finality_ref(),
            self.vault_id(),
        ));
        preimage
    }

    pub(crate) fn append_signing_bytes(&self, bytes: &mut Vec<u8>, prefix: &str) {
        bytes.extend_from_slice(
            format!(
                "{prefix}.source_chain_id={}\n{prefix}.vault_address={}\n{prefix}.token_address={}\n{prefix}.depositor={}\n{prefix}.pftl_recipient_bytes={}\n{prefix}.pftl_recipient={}\n{prefix}.pftl_recipient_hash={}\n{prefix}.amount_atoms={}\n{prefix}.nonce={}\n",
                self.source_chain_id,
                self.vault_address,
                self.token_address,
                self.depositor,
                self.pftl_recipient.len(),
                self.pftl_recipient,
                self.pftl_recipient_hash,
                self.amount_atoms,
                self.nonce,
            )
            .as_bytes(),
        );
        if !self.route_binding.is_empty() {
            bytes.extend_from_slice(
                format!("{prefix}.route_binding={}\n", self.route_binding).as_bytes(),
            );
        }
        bytes.extend_from_slice(
            format!(
                "{prefix}.deposit_id={}\n{prefix}.block_hash={}\n{prefix}.tx_hash={}\n{prefix}.log_index={}\n",
                self.deposit_id,
                self.block_hash,
                self.tx_hash,
                self.log_index,
            )
            .as_bytes(),
        );
    }
}

pub fn vault_bridge_pftl_recipient_hash(pftl_recipient: &str) -> Result<String, String> {
    validate_text_field("vault_bridge_deposit.pftl_recipient", pftl_recipient)?;
    let mut hasher = Keccak256::new();
    hasher.update(pftl_recipient.as_bytes());
    let digest = hasher.finalize();
    Ok(bytes_to_lower_hex(&digest))
}

/// Produce the EVM-sized commitment carried by a v2 vault deposit. The
/// 48-byte SHA3-384 profile hash remains the governance identity; this
/// 32-byte Keccak commitment makes that identity and epoch an ABI-native,
/// wallet-signed deposit argument.
pub fn vault_bridge_route_binding(profile_hash: &str, route_epoch: u32) -> Result<String, String> {
    validate_lower_hex_len(
        "vault_bridge_route_binding.profile_hash",
        profile_hash,
        VAULT_BRIDGE_HEX_HASH_LEN,
    )?;
    if route_epoch == 0 {
        return Err("vault_bridge_route_binding.route_epoch must be nonzero".to_string());
    }
    let profile_hash_bytes = decode_lower_hex_exact(
        "vault_bridge_route_binding.profile_hash",
        profile_hash,
        VAULT_BRIDGE_HEX_HASH_LEN / 2,
    )?;
    let mut hasher = Keccak256::new();
    hasher.update(VAULT_BRIDGE_ROUTE_BINDING_DOMAIN_V1.as_bytes());
    hasher.update([0]);
    hasher.update(profile_hash_bytes);
    hasher.update(route_epoch.to_be_bytes());
    Ok(bytes_to_lower_hex(&hasher.finalize()))
}

pub fn vault_bridge_deposit_id(evidence: &VaultBridgeDepositEvidence) -> Result<String, String> {
    if evidence.source_chain_id == 0 {
        return Err("vault_bridge_deposit.source_chain_id must be nonzero".to_string());
    }
    validate_evm_address_text(
        "vault_bridge_deposit.vault_address",
        &evidence.vault_address,
    )?;
    validate_evm_address_text(
        "vault_bridge_deposit.token_address",
        &evidence.token_address,
    )?;
    validate_evm_address_text("vault_bridge_deposit.depositor", &evidence.depositor)?;
    validate_lower_hex_len(
        "vault_bridge_deposit.pftl_recipient_hash",
        &evidence.pftl_recipient_hash,
        VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
    )?;
    validate_lower_hex_len(
        "vault_bridge_deposit.nonce",
        &evidence.nonce,
        VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
    )?;
    if !evidence.route_binding.is_empty() {
        validate_lower_hex_len(
            "vault_bridge_deposit.route_binding",
            &evidence.route_binding,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
    }

    let vault = decode_evm_address_20(
        "vault_bridge_deposit.vault_address",
        &evidence.vault_address,
    )?;
    let token = decode_evm_address_20(
        "vault_bridge_deposit.token_address",
        &evidence.token_address,
    )?;
    let depositor = decode_evm_address_20("vault_bridge_deposit.depositor", &evidence.depositor)?;
    let recipient_hash = decode_lower_hex_exact(
        "vault_bridge_deposit.pftl_recipient_hash",
        &evidence.pftl_recipient_hash,
        32,
    )?;
    let nonce = decode_lower_hex_exact("vault_bridge_deposit.nonce", &evidence.nonce, 32)?;

    let route_binding = if evidence.route_binding.is_empty() {
        None
    } else {
        Some(decode_lower_hex_exact(
            "vault_bridge_deposit.route_binding",
            &evidence.route_binding,
            32,
        )?)
    };
    let mut abi = Vec::new();
    append_abi_u256_u64(
        &mut abi,
        if route_binding.is_some() {
            9 * 32
        } else {
            8 * 32
        },
    );
    append_abi_u256_u64(&mut abi, evidence.source_chain_id);
    append_abi_address(&mut abi, &vault);
    append_abi_address(&mut abi, &token);
    append_abi_address(&mut abi, &depositor);
    append_abi_u256_u64(&mut abi, evidence.amount_atoms);
    abi.extend_from_slice(&recipient_hash);
    abi.extend_from_slice(&nonce);
    if let Some(route_binding) = route_binding {
        abi.extend_from_slice(&route_binding);
        append_abi_string_tail(&mut abi, "postfiat.erc20_bridge.deposit.v2");
    } else {
        append_abi_string_tail(&mut abi, "postfiat.erc20_bridge.deposit.v1");
    }

    let mut hasher = Keccak256::new();
    hasher.update(&abi);
    let digest = hasher.finalize();
    Ok(bytes_to_lower_hex(&digest))
}

pub fn vault_bridge_deposit_evidence_root(
    evidence: &VaultBridgeDepositEvidence,
) -> Result<String, String> {
    evidence.validate()?;
    Ok(hash_hex_domain(
        VAULT_BRIDGE_DEPOSIT_EVIDENCE_ROOT_DOMAIN,
        evidence.canonical_preimage().as_bytes(),
    ))
}

pub fn vault_bridge_deposit_public_values_hash(
    evidence: &VaultBridgeDepositEvidence,
    evidence_root: &str,
    policy_hash: &str,
) -> Result<String, String> {
    evidence.validate()?;
    validate_lower_hex_len(
        "vault_bridge_deposit_public_values.evidence_root",
        evidence_root,
        VAULT_BRIDGE_HEX_HASH_LEN,
    )?;
    let expected_root = vault_bridge_deposit_evidence_root(evidence)?;
    if evidence_root != expected_root {
        return Err("vault_bridge_deposit_public_values.evidence_root mismatch".to_string());
    }
    validate_vault_bridge_policy_hash(
        "vault_bridge_deposit_public_values.policy_hash",
        policy_hash,
    )?;
    let mut preimage = format!(
        "evidence_root={evidence_root}\nsource_chain_id={}\nvault_address={}\ntoken_address={}\ndepositor={}\npftl_recipient_bytes={}\npftl_recipient={}\npftl_recipient_hash={}\namount_atoms={}\nnonce={}\n",
        evidence.source_chain_id,
        evidence.vault_address,
        evidence.token_address,
        evidence.depositor,
        evidence.pftl_recipient.len(),
        evidence.pftl_recipient,
        evidence.pftl_recipient_hash,
        evidence.amount_atoms,
        evidence.nonce,
    );
    if !evidence.route_binding.is_empty() {
        preimage.push_str(&format!("route_binding={}\n", evidence.route_binding));
    }
    preimage.push_str(&format!(
        "deposit_id={}\nblock_hash={}\ntx_hash={}\nlog_index={}\nsource_domain={}\nsource_tx_or_attestation={}\nfinality_ref={}\nvault_id={}\npolicy_hash={policy_hash}\n",
        evidence.deposit_id,
        evidence.block_hash,
        evidence.tx_hash,
        evidence.log_index,
        evidence.source_domain(),
        evidence.source_tx_or_attestation(),
        evidence.finality_ref(),
        evidence.vault_id(),
    ));
    Ok(hash_hex_domain(
        VAULT_BRIDGE_DEPOSIT_PUBLIC_VALUES_DOMAIN,
        preimage.as_bytes(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositObservation {
    pub tx_exists: bool,
    pub receipt_status: u64,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub depositor: String,
    pub amount_atoms: u64,
    pub deposit_id: String,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub confirmation_depth: u64,
}

impl VaultBridgeDepositObservation {
    pub fn success_for_evidence(
        evidence: &VaultBridgeDepositEvidence,
        confirmation_depth: u64,
    ) -> Self {
        Self {
            tx_exists: true,
            receipt_status: 1,
            source_chain_id: evidence.source_chain_id,
            vault_address: evidence.vault_address.clone(),
            token_address: evidence.token_address.clone(),
            depositor: evidence.depositor.clone(),
            amount_atoms: evidence.amount_atoms,
            deposit_id: evidence.deposit_id.clone(),
            block_hash: evidence.block_hash.clone(),
            tx_hash: evidence.tx_hash.clone(),
            log_index: evidence.log_index,
            confirmation_depth,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.source_chain_id == 0 {
            return Err(
                "vault_bridge_deposit_observation.source_chain_id must be nonzero".to_string(),
            );
        }
        validate_evm_address_text(
            "vault_bridge_deposit_observation.vault_address",
            &self.vault_address,
        )?;
        validate_evm_address_text(
            "vault_bridge_deposit_observation.token_address",
            &self.token_address,
        )?;
        validate_evm_address_text(
            "vault_bridge_deposit_observation.depositor",
            &self.depositor,
        )?;
        if self.amount_atoms == 0 {
            return Err(
                "vault_bridge_deposit_observation.amount_atoms must be nonzero".to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_deposit_observation.deposit_id",
            &self.deposit_id,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_observation.block_hash",
            &self.block_hash,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_observation.tx_hash",
            &self.tx_hash,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        Ok(())
    }

    fn canonical_preimage(&self) -> String {
        format!(
            "tx_exists={}\nreceipt_status={}\nsource_chain_id={}\nvault_address={}\ntoken_address={}\ndepositor={}\namount_atoms={}\ndeposit_id={}\nblock_hash={}\ntx_hash={}\nlog_index={}\nconfirmation_depth={}\n",
            self.tx_exists,
            self.receipt_status,
            self.source_chain_id,
            self.vault_address,
            self.token_address,
            self.depositor,
            self.amount_atoms,
            self.deposit_id,
            self.block_hash,
            self.tx_hash,
            self.log_index,
            self.confirmation_depth,
        )
    }

    pub fn append_signing_bytes(&self, bytes: &mut Vec<u8>, prefix: &str) {
        bytes.extend_from_slice(
            format!(
                "{prefix}.tx_exists={}\n{prefix}.receipt_status={}\n{prefix}.source_chain_id={}\n{prefix}.vault_address={}\n{prefix}.token_address={}\n{prefix}.depositor={}\n{prefix}.amount_atoms={}\n{prefix}.deposit_id={}\n{prefix}.block_hash={}\n{prefix}.tx_hash={}\n{prefix}.log_index={}\n{prefix}.confirmation_depth={}\n",
                self.tx_exists,
                self.receipt_status,
                self.source_chain_id,
                self.vault_address,
                self.token_address,
                self.depositor,
                self.amount_atoms,
                self.deposit_id,
                self.block_hash,
                self.tx_hash,
                self.log_index,
                self.confirmation_depth,
            )
            .as_bytes(),
        );
    }
}

pub fn vault_bridge_deposit_observation_root(
    observation: &VaultBridgeDepositObservation,
) -> Result<String, String> {
    observation.validate()?;
    Ok(hash_hex_domain(
        VAULT_BRIDGE_DEPOSIT_OBSERVATION_ROOT_DOMAIN,
        observation.canonical_preimage().as_bytes(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositAttestation {
    pub attestor: String,
    pub pass: bool,
    pub observation_root: String,
    pub attested_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<VaultBridgeDepositObservation>,
}

impl VaultBridgeDepositAttestation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_deposit_attestation.attestor", &self.attestor)?;
        validate_lower_hex_len(
            "vault_bridge_deposit_attestation.observation_root",
            &self.observation_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        if self.attested_at_height == 0 {
            return Err(
                "vault_bridge_deposit_attestation.attested_at_height must be nonzero".to_string(),
            );
        }
        if let Some(observation) = &self.observation {
            observation.validate()?;
            let expected_root = vault_bridge_deposit_observation_root(observation)?;
            if self.observation_root != expected_root {
                return Err(
                    "vault_bridge_deposit_attestation.observation_root does not match observation"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositRecord {
    pub asset_id: String,
    pub evidence_root: String,
    pub evidence: VaultBridgeDepositEvidence,
    pub policy_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_proof_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_proof_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_public_values_hash: String,
    pub proposer: String,
    pub status: String,
    pub submitted_at_height: u64,
    pub finalized_at_height: u64,
    pub expires_at_height: u64,
    pub challenger: String,
    pub challenge_hash: String,
    pub challenge_bond: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attestations: Vec<VaultBridgeDepositAttestation>,
}

impl VaultBridgeDepositRecord {
    pub fn new(
        asset_id: impl Into<String>,
        evidence_root: impl Into<String>,
        evidence: VaultBridgeDepositEvidence,
        policy_hash: impl Into<String>,
        source_proof_kind: impl Into<String>,
        source_proof_hash: impl Into<String>,
        source_public_values_hash: impl Into<String>,
        proposer: impl Into<String>,
        submitted_at_height: u64,
        expires_at_height: u64,
    ) -> Result<Self, String> {
        let record = Self {
            asset_id: asset_id.into(),
            evidence_root: evidence_root.into(),
            evidence,
            policy_hash: policy_hash.into(),
            source_proof_kind: source_proof_kind.into(),
            source_proof_hash: source_proof_hash.into(),
            source_public_values_hash: source_public_values_hash.into(),
            proposer: proposer.into(),
            status: VAULT_BRIDGE_DEPOSIT_STATUS_PENDING.to_string(),
            submitted_at_height,
            finalized_at_height: 0,
            expires_at_height,
            challenger: String::new(),
            challenge_hash: String::new(),
            challenge_bond: 0,
            attestations: Vec::new(),
        };
        record.validate()?;
        Ok(record)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "vault_bridge_deposit_record.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_record.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        self.evidence.validate()?;
        let expected_root = vault_bridge_deposit_evidence_root(&self.evidence)?;
        if self.evidence_root != expected_root {
            return Err(
                "vault_bridge_deposit_record.evidence_root does not match evidence".to_string(),
            );
        }
        validate_vault_bridge_policy_hash(
            "vault_bridge_deposit_record.policy_hash",
            &self.policy_hash,
        )?;
        validate_vault_bridge_deposit_source_proof_fields(
            "vault_bridge_deposit_record",
            &self.source_proof_kind,
            &self.source_proof_hash,
            &self.source_public_values_hash,
        )?;
        validate_text_field("vault_bridge_deposit_record.proposer", &self.proposer)?;
        validate_vault_bridge_deposit_status(&self.status)?;
        if self.attestations.len() > MAX_NAV_ATTESTATIONS_PER_PACKET {
            return Err(format!(
                "vault_bridge_deposit_record.attestations exceeds maximum of {MAX_NAV_ATTESTATIONS_PER_PACKET}"
            ));
        }
        let mut attestors = BTreeSet::new();
        for attestation in &self.attestations {
            attestation.validate()?;
            if !attestors.insert(attestation.attestor.clone()) {
                return Err(
                    "vault_bridge_deposit_record has duplicate attestor attestations".to_string(),
                );
            }
        }
        if self.submitted_at_height == 0 {
            return Err(
                "vault_bridge_deposit_record.submitted_at_height must be nonzero".to_string(),
            );
        }
        if self.expires_at_height == 0 || self.expires_at_height <= self.submitted_at_height {
            return Err(
                "vault_bridge_deposit_record.expires_at_height must be greater than submitted_at_height"
                    .to_string(),
            );
        }
        match self.status.as_str() {
            VAULT_BRIDGE_DEPOSIT_STATUS_PENDING => {
                if self.finalized_at_height != 0 {
                    return Err(
                        "pending vault_bridge bridge deposit must not have finalized_at_height"
                            .to_string(),
                    );
                }
                if !self.challenger.is_empty()
                    || !self.challenge_hash.is_empty()
                    || self.challenge_bond != 0
                {
                    return Err(
                        "pending vault_bridge bridge deposit must not carry challenge fields"
                            .to_string(),
                    );
                }
            }
            VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED => {
                if self.finalized_at_height != 0 {
                    return Err(
                        "challenged vault_bridge bridge deposit must not have finalized_at_height"
                            .to_string(),
                    );
                }
                validate_text_field("vault_bridge_deposit_record.challenger", &self.challenger)?;
                validate_lower_hex_len(
                    "vault_bridge_deposit_record.challenge_hash",
                    &self.challenge_hash,
                    VAULT_BRIDGE_HEX_HASH_LEN,
                )?;
            }
            VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED => {
                if self.finalized_at_height == 0 {
                    return Err(
                        "finalized vault_bridge bridge deposit requires finalized_at_height"
                            .to_string(),
                    );
                }
                if self.finalized_at_height < self.submitted_at_height {
                    return Err(
                        "vault_bridge bridge deposit finalized_at_height precedes submitted_at_height"
                            .to_string(),
                    );
                }
                if !self.challenger.is_empty()
                    || !self.challenge_hash.is_empty()
                    || self.challenge_bond != 0
                {
                    return Err(
                        "finalized vault_bridge bridge deposit must not carry challenge fields"
                            .to_string(),
                    );
                }
            }
            _ => unreachable!("validated status"),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeReceipt {
    pub receipt_id: String,
    pub asset_id: String,
    pub source_domain: String,
    pub source_asset: String,
    pub claim_type: String,
    pub amount_atoms: u64,
    pub source_tx_or_attestation: String,
    pub finality_ref: String,
    pub vault_id: String,
    pub policy_hash: String,
    pub haircut_bps: u64,
    pub counted_value_atoms: u64,
    pub allocated_value_atoms: u64,
    pub bucket_id: String,
    pub status: String,
    pub created_at_height: u64,
    pub finalized_at_height: u64,
    pub counted_at_height: u64,
    pub expires_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_deposit_evidence: Option<VaultBridgeDepositEvidence>,
}

impl VaultBridgeReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: &str,
        asset_id: impl Into<String>,
        source_domain: impl Into<String>,
        source_asset: impl Into<String>,
        claim_type: impl Into<String>,
        amount_atoms: u64,
        source_tx_or_attestation: impl Into<String>,
        finality_ref: impl Into<String>,
        vault_id: impl Into<String>,
        policy_hash: impl Into<String>,
        created_at_height: u64,
        expires_at_height: u64,
        bridge_deposit_evidence: Option<VaultBridgeDepositEvidence>,
    ) -> Result<Self, String> {
        let asset_id = asset_id.into();
        let source_domain = source_domain.into();
        let source_tx_or_attestation = source_tx_or_attestation.into();
        let finality_ref = finality_ref.into();
        let policy_hash = policy_hash.into();
        let receipt_id = vault_bridge_receipt_id(
            chain_id,
            &asset_id,
            &source_domain,
            &source_tx_or_attestation,
            &finality_ref,
            amount_atoms,
            &policy_hash,
        )?;
        let bucket_id = vault_bridge_bucket_id(&asset_id, &source_domain, &policy_hash)?;
        let receipt = Self {
            receipt_id,
            asset_id,
            source_domain,
            source_asset: source_asset.into(),
            claim_type: claim_type.into(),
            amount_atoms,
            source_tx_or_attestation,
            finality_ref,
            vault_id: vault_id.into(),
            policy_hash,
            haircut_bps: 0,
            counted_value_atoms: 0,
            allocated_value_atoms: 0,
            bucket_id,
            status: VAULT_BRIDGE_RECEIPT_STATUS_PENDING.to_string(),
            created_at_height,
            finalized_at_height: 0,
            counted_at_height: 0,
            expires_at_height,
            bridge_deposit_evidence,
        };
        receipt.validate_for_chain(chain_id)?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "vault_bridge_receipt.receipt_id",
            &self.receipt_id,
            VAULT_BRIDGE_RECEIPT_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_receipt.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_text_field("vault_bridge_receipt.source_domain", &self.source_domain)?;
        validate_text_field("vault_bridge_receipt.source_asset", &self.source_asset)?;
        validate_text_field("vault_bridge_receipt.claim_type", &self.claim_type)?;
        validate_vault_bridge_receipt_bridge_deposit_fields(
            &self.claim_type,
            &self.source_domain,
            &self.source_asset,
            self.amount_atoms,
            &self.source_tx_or_attestation,
            &self.finality_ref,
            &self.vault_id,
            self.bridge_deposit_evidence.as_ref(),
        )?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_receipt.amount_atoms must be nonzero".to_string());
        }
        validate_text_field(
            "vault_bridge_receipt.source_tx_or_attestation",
            &self.source_tx_or_attestation,
        )?;
        validate_text_field("vault_bridge_receipt.finality_ref", &self.finality_ref)?;
        validate_text_field("vault_bridge_receipt.vault_id", &self.vault_id)?;
        validate_vault_bridge_policy_hash("vault_bridge_receipt.policy_hash", &self.policy_hash)?;
        if self.haircut_bps > 10_000 {
            return Err("vault_bridge_receipt.haircut_bps exceeds 10000".to_string());
        }
        if self.counted_value_atoms > self.amount_atoms {
            return Err(
                "vault_bridge_receipt.counted_value_atoms must not exceed amount_atoms".to_string(),
            );
        }
        if self.allocated_value_atoms > self.counted_value_atoms {
            return Err(
                "vault_bridge_receipt.allocated_value_atoms must not exceed counted_value_atoms"
                    .to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_receipt.bucket_id",
            &self.bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        validate_vault_bridge_receipt_status(&self.status)?;
        let expected_bucket_id =
            vault_bridge_bucket_id(&self.asset_id, &self.source_domain, &self.policy_hash)?;
        if self.bucket_id != expected_bucket_id {
            return Err(
                "vault_bridge_receipt.bucket_id does not match asset, source domain, and policy hash"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected_receipt_id = vault_bridge_receipt_id(
            chain_id,
            &self.asset_id,
            &self.source_domain,
            &self.source_tx_or_attestation,
            &self.finality_ref,
            self.amount_atoms,
            &self.policy_hash,
        )?;
        if self.receipt_id != expected_receipt_id {
            return Err("vault_bridge_receipt.receipt_id does not match source fields".to_string());
        }
        Ok(())
    }

    pub fn available_counted_value(&self) -> Result<u64, String> {
        self.counted_value_atoms
            .checked_sub(self.allocated_value_atoms)
            .ok_or_else(|| "vault_bridge_receipt allocated value exceeds counted value".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeBucketState {
    pub asset_id: String,
    pub bucket_id: String,
    pub source_domain: String,
    pub policy_hash: String,
    pub gross_receipt_atoms: u64,
    pub counted_value_atoms: u64,
    pub outstanding_vault_bridge_atoms: u64,
    pub nav_subscription_allocations_atoms: u64,
    pub redemption_queue_atoms: u64,
    pub other_allocations_atoms: u64,
    pub impairment_factor_bps: u64,
    pub status: String,
    pub last_packet_epoch: u64,
    pub last_updated_height: u64,
}

impl VaultBridgeBucketState {
    pub fn new(
        asset_id: impl Into<String>,
        source_domain: impl Into<String>,
        policy_hash: impl Into<String>,
        last_updated_height: u64,
    ) -> Result<Self, String> {
        let asset_id = asset_id.into();
        let source_domain = source_domain.into();
        let policy_hash = policy_hash.into();
        let bucket_id = vault_bridge_bucket_id(&asset_id, &source_domain, &policy_hash)?;
        let bucket = Self {
            asset_id,
            bucket_id,
            source_domain,
            policy_hash,
            gross_receipt_atoms: 0,
            counted_value_atoms: 0,
            outstanding_vault_bridge_atoms: 0,
            nav_subscription_allocations_atoms: 0,
            redemption_queue_atoms: 0,
            other_allocations_atoms: 0,
            impairment_factor_bps: 10_000,
            status: VAULT_BRIDGE_BUCKET_STATUS_ACTIVE.to_string(),
            last_packet_epoch: 0,
            last_updated_height,
        };
        bucket.validate()?;
        Ok(bucket)
    }

    pub fn allocated_atoms(&self) -> Result<u64, String> {
        self.outstanding_vault_bridge_atoms
            .checked_add(self.nav_subscription_allocations_atoms)
            .and_then(|value| value.checked_add(self.redemption_queue_atoms))
            .and_then(|value| value.checked_add(self.other_allocations_atoms))
            .ok_or_else(|| "vault_bridge_bucket allocated atoms overflow".to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "vault_bridge_bucket.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_bucket.bucket_id",
            &self.bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        validate_text_field("vault_bridge_bucket.source_domain", &self.source_domain)?;
        validate_vault_bridge_policy_hash("vault_bridge_bucket.policy_hash", &self.policy_hash)?;
        if self.impairment_factor_bps > 10_000 {
            return Err("vault_bridge_bucket.impairment_factor_bps exceeds 10000".to_string());
        }
        validate_vault_bridge_bucket_status(&self.status)?;
        let expected_bucket_id =
            vault_bridge_bucket_id(&self.asset_id, &self.source_domain, &self.policy_hash)?;
        if self.bucket_id != expected_bucket_id {
            return Err(
                "vault_bridge_bucket.bucket_id does not match asset, source domain, and policy hash"
                    .to_string(),
            );
        }
        let allocated = self.allocated_atoms()?;
        let expected_impairment_factor_bps =
            vault_bridge_bucket_factor_bps(self.counted_value_atoms, allocated)?;
        if self.impairment_factor_bps != expected_impairment_factor_bps {
            return Err(
                "vault_bridge_bucket.impairment_factor_bps does not match counted value and allocated claims"
                    .to_string(),
            );
        }
        if allocated > self.counted_value_atoms
            && self.status != VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED
            && self.status != VAULT_BRIDGE_BUCKET_STATUS_PAUSED
        {
            return Err(
                "vault_bridge_bucket allocated atoms may exceed counted_value_atoms only when impaired or paused"
                    .to_string(),
            );
        }
        Ok(())
    }
}

fn vault_bridge_bucket_factor_bps(
    counted_value_atoms: u64,
    claim_atoms: u64,
) -> Result<u64, String> {
    if claim_atoms == 0 {
        return Ok(10_000);
    }
    let raw = (counted_value_atoms as u128)
        .checked_mul(10_000)
        .ok_or_else(|| "vault_bridge_bucket factor multiplication overflow".to_string())?
        / claim_atoms as u128;
    Ok(raw.min(10_000) as u64)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeAllocation {
    pub allocation_id: String,
    pub receipt_id: String,
    pub asset_id: String,
    pub bucket_id: String,
    pub amount_atoms: u64,
    pub purpose: String,
    pub consumer_id: String,
    pub created_at_height: u64,
    pub retired_at_height: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub released_atoms: u64,
}

impl VaultBridgeAllocation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: &str,
        receipt_id: impl Into<String>,
        asset_id: impl Into<String>,
        bucket_id: impl Into<String>,
        amount_atoms: u64,
        purpose: impl Into<String>,
        consumer_id: impl Into<String>,
        created_at_height: u64,
    ) -> Result<Self, String> {
        let receipt_id = receipt_id.into();
        let asset_id = asset_id.into();
        let bucket_id = bucket_id.into();
        let purpose = purpose.into();
        let consumer_id = consumer_id.into();
        let allocation_id = vault_bridge_allocation_id(
            chain_id,
            &receipt_id,
            &asset_id,
            &bucket_id,
            amount_atoms,
            &purpose,
            &consumer_id,
        )?;
        let allocation = Self {
            allocation_id,
            receipt_id,
            asset_id,
            bucket_id,
            amount_atoms,
            purpose,
            consumer_id,
            created_at_height,
            retired_at_height: 0,
            released_atoms: 0,
        };
        allocation.validate_for_chain(chain_id)?;
        Ok(allocation)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "vault_bridge_allocation.allocation_id",
            &self.allocation_id,
            VAULT_BRIDGE_ALLOCATION_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_allocation.receipt_id",
            &self.receipt_id,
            VAULT_BRIDGE_RECEIPT_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_allocation.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_allocation.bucket_id",
            &self.bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_allocation.amount_atoms must be nonzero".to_string());
        }
        if self.released_atoms > self.amount_atoms {
            return Err("vault_bridge_allocation.released_atoms exceeds amount_atoms".to_string());
        }
        validate_vault_bridge_allocation_purpose(&self.purpose)?;
        validate_text_field("vault_bridge_allocation.consumer_id", &self.consumer_id)?;
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected_allocation_id = vault_bridge_allocation_id(
            chain_id,
            &self.receipt_id,
            &self.asset_id,
            &self.bucket_id,
            self.amount_atoms,
            &self.purpose,
            &self.consumer_id,
        )?;
        if self.allocation_id != expected_allocation_id {
            return Err(
                "vault_bridge_allocation.allocation_id does not match allocation fields"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalPacket {
    pub pftl_chain_id: u64,
    #[serde(default)]
    pub source_chain_id: u64,
    #[serde(default)]
    pub vault_address: String,
    #[serde(default)]
    pub token_address: String,
    pub vault_bridge_asset_id: String,
    pub burn_tx_id: String,
    pub withdrawal_id: String,
    pub recipient: String,
    pub amount_atoms: u64,
    pub source_bucket_id: String,
    pub destination_hash: String,
    pub finalized_height: u64,
    pub evidence_root: String,
}

impl VaultBridgeWithdrawalPacket {
    pub fn is_legacy_domainless(&self) -> bool {
        self.source_chain_id == 0 && self.vault_address.is_empty() && self.token_address.is_empty()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.pftl_chain_id == 0 {
            return Err("vault_bridge_withdrawal_packet.pftl_chain_id must be nonzero".to_string());
        }
        if self.source_chain_id == 0 {
            return Err(
                "vault_bridge_withdrawal_packet.source_chain_id must be nonzero".to_string(),
            );
        }
        validate_evm_address_text(
            "vault_bridge_withdrawal_packet.vault_address",
            &self.vault_address,
        )?;
        validate_evm_address_text(
            "vault_bridge_withdrawal_packet.token_address",
            &self.token_address,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.vault_bridge_asset_id",
            &self.vault_bridge_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.burn_tx_id",
            &self.burn_tx_id,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.withdrawal_id",
            &self.withdrawal_id,
            VAULT_BRIDGE_REDEMPTION_ID_HEX_LEN,
        )?;
        validate_evm_address_text("vault_bridge_withdrawal_packet.recipient", &self.recipient)?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_withdrawal_packet.amount_atoms must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.source_bucket_id",
            &self.source_bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.destination_hash",
            &self.destination_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        if self.finalized_height == 0 {
            return Err(
                "vault_bridge_withdrawal_packet.finalized_height must be nonzero".to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        Ok(())
    }

    fn validate_legacy_domainless_state_record(&self) -> Result<(), String> {
        if !self.is_legacy_domainless() {
            return Err(
                "vault_bridge_withdrawal_packet legacy state record must omit all domain fields"
                    .to_string(),
            );
        }
        if self.pftl_chain_id == 0 {
            return Err("vault_bridge_withdrawal_packet.pftl_chain_id must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.vault_bridge_asset_id",
            &self.vault_bridge_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.burn_tx_id",
            &self.burn_tx_id,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.withdrawal_id",
            &self.withdrawal_id,
            VAULT_BRIDGE_REDEMPTION_ID_HEX_LEN,
        )?;
        validate_evm_address_text("vault_bridge_withdrawal_packet.recipient", &self.recipient)?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_withdrawal_packet.amount_atoms must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.source_bucket_id",
            &self.source_bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.destination_hash",
            &self.destination_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        if self.finalized_height == 0 {
            return Err(
                "vault_bridge_withdrawal_packet.finalized_height must be nonzero".to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_withdrawal_packet.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        Ok(())
    }

    fn canonical_preimage(&self) -> String {
        format!(
            "pftl_chain_id={}\nsource_chain_id={}\nvault_address={}\ntoken_address={}\nvault_bridge_asset_id={}\nburn_tx_id={}\nwithdrawal_id={}\nrecipient={}\namount_atoms={}\nsource_bucket_id={}\ndestination_hash={}\nfinalized_height={}\nevidence_root={}\n",
            self.pftl_chain_id,
            self.source_chain_id,
            self.vault_address,
            self.token_address,
            self.vault_bridge_asset_id,
            self.burn_tx_id,
            self.withdrawal_id,
            self.recipient,
            self.amount_atoms,
            self.source_bucket_id,
            self.destination_hash,
            self.finalized_height,
            self.evidence_root,
        )
    }

    fn legacy_domainless_preimage(&self) -> String {
        format!(
            "pftl_chain_id={}\nvault_bridge_asset_id={}\nburn_tx_id={}\nwithdrawal_id={}\nrecipient={}\namount_atoms={}\nsource_bucket_id={}\ndestination_hash={}\nfinalized_height={}\nevidence_root={}\n",
            self.pftl_chain_id,
            self.vault_bridge_asset_id,
            self.burn_tx_id,
            self.withdrawal_id,
            self.recipient,
            self.amount_atoms,
            self.source_bucket_id,
            self.destination_hash,
            self.finalized_height,
            self.evidence_root,
        )
    }
}

pub fn vault_bridge_withdrawal_packet_hash(
    packet: &VaultBridgeWithdrawalPacket,
) -> Result<String, String> {
    packet.validate()?;
    Ok(hash_hex_domain(
        VAULT_BRIDGE_WITHDRAWAL_PACKET_HASH_DOMAIN,
        packet.canonical_preimage().as_bytes(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalExecutionObservation {
    pub tx_exists: bool,
    pub receipt_status: u64,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub recipient: String,
    pub amount_atoms: u64,
    pub withdrawal_id: String,
    pub withdrawal_packet_hash: String,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub confirmation_depth: u64,
}

impl VaultBridgeWithdrawalExecutionObservation {
    pub fn success_for_packet(
        packet: &VaultBridgeWithdrawalPacket,
        withdrawal_packet_hash: impl Into<String>,
        settlement_tx_hash: impl Into<String>,
        settlement_block_hash: impl Into<String>,
        log_index: u64,
        confirmation_depth: u64,
    ) -> Self {
        Self {
            tx_exists: true,
            receipt_status: 1,
            source_chain_id: packet.source_chain_id,
            vault_address: packet.vault_address.clone(),
            token_address: packet.token_address.clone(),
            recipient: packet.recipient.clone(),
            amount_atoms: packet.amount_atoms,
            withdrawal_id: packet.withdrawal_id.clone(),
            withdrawal_packet_hash: withdrawal_packet_hash.into(),
            block_hash: settlement_block_hash.into(),
            tx_hash: settlement_tx_hash.into(),
            log_index,
            confirmation_depth,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.source_chain_id == 0 {
            return Err(
                "vault_bridge_withdrawal_observation.source_chain_id must be nonzero".to_string(),
            );
        }
        validate_evm_address_text(
            "vault_bridge_withdrawal_observation.vault_address",
            &self.vault_address,
        )?;
        validate_evm_address_text(
            "vault_bridge_withdrawal_observation.token_address",
            &self.token_address,
        )?;
        validate_evm_address_text(
            "vault_bridge_withdrawal_observation.recipient",
            &self.recipient,
        )?;
        if self.amount_atoms == 0 {
            return Err(
                "vault_bridge_withdrawal_observation.amount_atoms must be nonzero".to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_withdrawal_observation.withdrawal_id",
            &self.withdrawal_id,
            VAULT_BRIDGE_REDEMPTION_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_observation.withdrawal_packet_hash",
            &self.withdrawal_packet_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_observation.block_hash",
            &self.block_hash,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_observation.tx_hash",
            &self.tx_hash,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        Ok(())
    }

    fn canonical_preimage(&self) -> String {
        format!(
            "tx_exists={}\nreceipt_status={}\nsource_chain_id={}\nvault_address={}\ntoken_address={}\nrecipient={}\namount_atoms={}\nwithdrawal_id={}\nwithdrawal_packet_hash={}\nblock_hash={}\ntx_hash={}\nlog_index={}\nconfirmation_depth={}\n",
            self.tx_exists,
            self.receipt_status,
            self.source_chain_id,
            self.vault_address,
            self.token_address,
            self.recipient,
            self.amount_atoms,
            self.withdrawal_id,
            self.withdrawal_packet_hash,
            self.block_hash,
            self.tx_hash,
            self.log_index,
            self.confirmation_depth,
        )
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        self.canonical_preimage().into_bytes()
    }

    pub fn append_signing_bytes(&self, bytes: &mut Vec<u8>, prefix: &str) {
        bytes.extend_from_slice(
            format!(
                "{prefix}.tx_exists={}\n{prefix}.receipt_status={}\n{prefix}.source_chain_id={}\n{prefix}.vault_address={}\n{prefix}.token_address={}\n{prefix}.recipient={}\n{prefix}.amount_atoms={}\n{prefix}.withdrawal_id={}\n{prefix}.withdrawal_packet_hash={}\n{prefix}.block_hash={}\n{prefix}.tx_hash={}\n{prefix}.log_index={}\n{prefix}.confirmation_depth={}\n",
                self.tx_exists,
                self.receipt_status,
                self.source_chain_id,
                self.vault_address,
                self.token_address,
                self.recipient,
                self.amount_atoms,
                self.withdrawal_id,
                self.withdrawal_packet_hash,
                self.block_hash,
                self.tx_hash,
                self.log_index,
                self.confirmation_depth,
            )
            .as_bytes(),
        );
    }
}

pub fn vault_bridge_withdrawal_execution_observation_root(
    observation: &VaultBridgeWithdrawalExecutionObservation,
) -> Result<String, String> {
    observation.validate()?;
    Ok(hash_hex_domain(
        VAULT_BRIDGE_WITHDRAWAL_OBSERVATION_ROOT_DOMAIN,
        observation.canonical_preimage().as_bytes(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalExecutionAttestation {
    pub attestor: String,
    pub observation_root: String,
    pub signature_hex: String,
    pub observation: VaultBridgeWithdrawalExecutionObservation,
}

impl VaultBridgeWithdrawalExecutionAttestation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field(
            "vault_bridge_withdrawal_attestation.attestor",
            &self.attestor,
        )?;
        validate_lower_hex_len(
            "vault_bridge_withdrawal_attestation.observation_root",
            &self.observation_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_max(
            "vault_bridge_withdrawal_attestation.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )?;
        self.observation.validate()?;
        let expected_root = vault_bridge_withdrawal_execution_observation_root(&self.observation)?;
        if self.observation_root != expected_root {
            return Err(
                "vault_bridge_withdrawal_attestation.observation_root does not match observation"
                    .to_string(),
            );
        }
        Ok(())
    }
}

pub fn vault_bridge_withdrawal_packet_legacy_domainless_hash(
    packet: &VaultBridgeWithdrawalPacket,
) -> Result<String, String> {
    packet.validate_legacy_domainless_state_record()?;
    Ok(hash_hex_domain(
        VAULT_BRIDGE_WITHDRAWAL_PACKET_HASH_DOMAIN,
        packet.legacy_domainless_preimage().as_bytes(),
    ))
}

pub fn vault_bridge_withdrawal_packet_legacy_domainless_evm_digest(
    packet: &VaultBridgeWithdrawalPacket,
) -> Result<String, String> {
    packet.validate_legacy_domainless_state_record()?;
    let asset_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.vault_bridge_asset_id",
        &packet.vault_bridge_asset_id,
        48,
    )?;
    let burn_tx_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.burn_tx_id",
        &packet.burn_tx_id,
        48,
    )?;
    let withdrawal_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.withdrawal_id",
        &packet.withdrawal_id,
        48,
    )?;
    let recipient = decode_evm_address_20(
        "vault_bridge_withdrawal_packet.recipient",
        &packet.recipient,
    )?;
    let source_bucket_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.source_bucket_id",
        &packet.source_bucket_id,
        48,
    )?;
    let destination_hash = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.destination_hash",
        &packet.destination_hash,
        48,
    )?;
    let evidence_root = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.evidence_root",
        &packet.evidence_root,
        48,
    )?;

    let head_words = 11usize;
    let head_len = head_words.checked_mul(32).ok_or_else(|| {
        "vault_bridge_withdrawal_packet legacy ABI head length overflow".to_string()
    })?;
    let mut head = Vec::with_capacity(head_len);
    let mut tail = Vec::new();
    append_abi_dynamic_bytes(
        &mut head,
        &mut tail,
        head_len,
        "postfiat.erc20_bridge.withdrawal_packet.v1".as_bytes(),
    )?;
    append_abi_u256_u64(&mut head, packet.pftl_chain_id);
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &asset_id)?;
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &burn_tx_id)?;
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &withdrawal_id)?;
    append_abi_address(&mut head, &recipient);
    append_abi_u256_u64(&mut head, packet.amount_atoms);
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &source_bucket_id)?;
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &destination_hash)?;
    append_abi_u256_u64(&mut head, packet.finalized_height);
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &evidence_root)?;

    let mut abi = head;
    abi.extend_from_slice(&tail);
    let mut hasher = Keccak256::new();
    hasher.update(&abi);
    let digest = hasher.finalize();
    Ok(bytes_to_lower_hex(&digest))
}

pub fn vault_bridge_withdrawal_packet_evm_digest(
    packet: &VaultBridgeWithdrawalPacket,
) -> Result<String, String> {
    packet.validate()?;
    let asset_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.vault_bridge_asset_id",
        &packet.vault_bridge_asset_id,
        48,
    )?;
    let vault_address = decode_evm_address_20(
        "vault_bridge_withdrawal_packet.vault_address",
        &packet.vault_address,
    )?;
    let token_address = decode_evm_address_20(
        "vault_bridge_withdrawal_packet.token_address",
        &packet.token_address,
    )?;
    let burn_tx_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.burn_tx_id",
        &packet.burn_tx_id,
        48,
    )?;
    let withdrawal_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.withdrawal_id",
        &packet.withdrawal_id,
        48,
    )?;
    let recipient = decode_evm_address_20(
        "vault_bridge_withdrawal_packet.recipient",
        &packet.recipient,
    )?;
    let source_bucket_id = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.source_bucket_id",
        &packet.source_bucket_id,
        48,
    )?;
    let destination_hash = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.destination_hash",
        &packet.destination_hash,
        48,
    )?;
    let evidence_root = decode_lower_hex_exact(
        "vault_bridge_withdrawal_packet.evidence_root",
        &packet.evidence_root,
        48,
    )?;

    let mut domain_abi = Vec::with_capacity(4 * 32);
    append_abi_u256_u64(&mut domain_abi, packet.pftl_chain_id);
    append_abi_u256_u64(&mut domain_abi, packet.source_chain_id);
    append_abi_address(&mut domain_abi, &vault_address);
    append_abi_address(&mut domain_abi, &token_address);
    let mut hasher = Keccak256::new();
    hasher.update(&domain_abi);
    let domain_hash = hasher.finalize();

    let payload_head_words = 9usize;
    let payload_head_len = payload_head_words.checked_mul(32).ok_or_else(|| {
        "vault_bridge_withdrawal_packet payload ABI head length overflow".to_string()
    })?;
    let mut payload_head = Vec::with_capacity(payload_head_len);
    let mut payload_tail = Vec::new();
    append_abi_dynamic_bytes(
        &mut payload_head,
        &mut payload_tail,
        payload_head_len,
        &asset_id,
    )?;
    append_abi_dynamic_bytes(
        &mut payload_head,
        &mut payload_tail,
        payload_head_len,
        &burn_tx_id,
    )?;
    append_abi_dynamic_bytes(
        &mut payload_head,
        &mut payload_tail,
        payload_head_len,
        &withdrawal_id,
    )?;
    append_abi_address(&mut payload_head, &recipient);
    append_abi_u256_u64(&mut payload_head, packet.amount_atoms);
    append_abi_dynamic_bytes(
        &mut payload_head,
        &mut payload_tail,
        payload_head_len,
        &source_bucket_id,
    )?;
    append_abi_dynamic_bytes(
        &mut payload_head,
        &mut payload_tail,
        payload_head_len,
        &destination_hash,
    )?;
    append_abi_u256_u64(&mut payload_head, packet.finalized_height);
    append_abi_dynamic_bytes(
        &mut payload_head,
        &mut payload_tail,
        payload_head_len,
        &evidence_root,
    )?;
    let mut payload_abi = payload_head;
    payload_abi.extend_from_slice(&payload_tail);
    let mut hasher = Keccak256::new();
    hasher.update(&payload_abi);
    let payload_hash = hasher.finalize();

    let final_head_words = 3usize;
    let final_head_len = final_head_words.checked_mul(32).ok_or_else(|| {
        "vault_bridge_withdrawal_packet final ABI head length overflow".to_string()
    })?;
    let mut final_head = Vec::with_capacity(final_head_len);
    let mut final_tail = Vec::new();
    append_abi_dynamic_bytes(
        &mut final_head,
        &mut final_tail,
        final_head_len,
        "postfiat.erc20_bridge.withdrawal_packet.v2".as_bytes(),
    )?;
    append_abi_bytes32(&mut final_head, domain_hash.as_slice())?;
    append_abi_bytes32(&mut final_head, payload_hash.as_slice())?;

    let mut abi = final_head;
    abi.extend_from_slice(&final_tail);
    let mut hasher = Keccak256::new();
    hasher.update(&abi);
    let digest = hasher.finalize();
    Ok(bytes_to_lower_hex(&digest))
}

pub fn vault_bridge_destination_ref_hash(destination_ref: &str) -> Result<String, String> {
    validate_text_field("vault_bridge_destination_ref", destination_ref)?;
    Ok(hash_hex_domain(
        "postfiat.vault_bridge.destination_ref_hash.v1",
        format!(
            "destination_ref_bytes={}\ndestination_ref={destination_ref}\n",
            destination_ref.len()
        )
        .as_bytes(),
    ))
}

pub fn vault_bridge_redemption_evidence_root(
    chain_id: &str,
    burn_tx_id: &str,
    owner: &str,
    issuer: &str,
    asset_id: &str,
    bucket_id: &str,
    owner_sequence: u64,
    amount_atoms: u64,
    epoch: u64,
    reserve_packet_hash: &str,
    destination_ref: &str,
    created_at_height: u64,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_lower_hex_len(
        "vault_bridge_redemption_evidence.burn_tx_id",
        burn_tx_id,
        VAULT_BRIDGE_HEX_HASH_LEN,
    )?;
    validate_text_field("vault_bridge_redemption_evidence.owner", owner)?;
    validate_text_field("vault_bridge_redemption_evidence.issuer", issuer)?;
    validate_lower_hex_len(
        "vault_bridge_redemption_evidence.asset_id",
        asset_id,
        ISSUED_ASSET_ID_HEX_LEN,
    )?;
    validate_lower_hex_len(
        "vault_bridge_redemption_evidence.bucket_id",
        bucket_id,
        VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
    )?;
    if owner_sequence == 0 {
        return Err("vault_bridge_redemption_evidence.owner_sequence must be nonzero".to_string());
    }
    if amount_atoms == 0 {
        return Err("vault_bridge_redemption_evidence.amount_atoms must be nonzero".to_string());
    }
    if epoch == 0 {
        return Err("vault_bridge_redemption_evidence.epoch must be nonzero".to_string());
    }
    validate_lower_hex_len(
        "vault_bridge_redemption_evidence.reserve_packet_hash",
        reserve_packet_hash,
        NAV_RESERVE_PACKET_ID_HEX_LEN,
    )?;
    validate_text_field(
        "vault_bridge_redemption_evidence.destination_ref",
        destination_ref,
    )?;
    if created_at_height == 0 {
        return Err(
            "vault_bridge_redemption_evidence.created_at_height must be nonzero".to_string(),
        );
    }
    let preimage = format!(
        "chain_id={chain_id}\nburn_tx_id={burn_tx_id}\nowner={owner}\nissuer={issuer}\nasset_id={asset_id}\nbucket_id={bucket_id}\nowner_sequence={owner_sequence}\namount_atoms={amount_atoms}\nepoch={epoch}\nreserve_packet_hash={reserve_packet_hash}\ndestination_ref_bytes={}\ndestination_ref={destination_ref}\ncreated_at_height={created_at_height}\n",
        destination_ref.len()
    );
    Ok(hash_hex_domain(
        "postfiat.vault_bridge.redemption_evidence_root.v1",
        preimage.as_bytes(),
    ))
}

pub fn pftl_chain_numeric_id(chain_id: &str) -> Result<u64, String> {
    validate_chain_id(chain_id)?;
    let mut hasher = Sha3_256::new();
    hasher.update(b"postfiat.pftl_chain_numeric_id.v1");
    hasher.update([0u8]);
    hasher.update(chain_id.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_be_bytes(bytes);
    if value == 0 {
        return Err("pftl_chain_numeric_id unexpectedly resolved to zero".to_string());
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeRedemption {
    pub redemption_id: String,
    pub owner: String,
    pub owner_sequence: u64,
    pub issuer: String,
    pub asset_id: String,
    pub bucket_id: String,
    pub amount_atoms: u64,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    pub destination_ref: String,
    pub settled_atoms: u64,
    pub state: String,
    pub created_at_height: u64,
    pub settlement_receipt_hash: String,
    pub withdrawal_packet: VaultBridgeWithdrawalPacket,
    pub withdrawal_packet_hash: String,
    pub withdrawal_packet_evm_digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withdrawal_observations: Vec<VaultBridgeWithdrawalExecutionAttestation>,
}

impl VaultBridgeRedemption {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: &str,
        owner: impl Into<String>,
        issuer: impl Into<String>,
        asset_id: impl Into<String>,
        bucket_id: impl Into<String>,
        source_domain: impl Into<String>,
        owner_sequence: u64,
        amount_atoms: u64,
        epoch: u64,
        reserve_packet_hash: impl Into<String>,
        destination_ref: impl Into<String>,
        burn_tx_id: impl Into<String>,
        created_at_height: u64,
    ) -> Result<Self, String> {
        let owner = owner.into();
        let issuer = issuer.into();
        let asset_id = asset_id.into();
        let bucket_id = bucket_id.into();
        let source_domain = source_domain.into();
        let reserve_packet_hash = reserve_packet_hash.into();
        let destination_ref = destination_ref.into();
        let burn_tx_id = burn_tx_id.into();
        let (source_chain_id, vault_address, token_address) =
            vault_bridge_evm_source_domain_parts(&source_domain)?;
        let destination_chain_id =
            vault_bridge_evm_chain_id_from_destination_ref(&destination_ref)?;
        if source_chain_id != destination_chain_id {
            return Err(
                "vault_bridge_redemption source domain chain id must match destination_ref chain id"
                    .to_string(),
            );
        }
        let redemption_id =
            vault_bridge_redemption_id(chain_id, &owner, &asset_id, owner_sequence)?;
        let recipient = vault_bridge_evm_recipient_from_destination_ref(&destination_ref)?;
        let destination_hash = vault_bridge_destination_ref_hash(&destination_ref)?;
        let evidence_root = vault_bridge_redemption_evidence_root(
            chain_id,
            &burn_tx_id,
            &owner,
            &issuer,
            &asset_id,
            &bucket_id,
            owner_sequence,
            amount_atoms,
            epoch,
            &reserve_packet_hash,
            &destination_ref,
            created_at_height,
        )?;
        let withdrawal_packet = VaultBridgeWithdrawalPacket {
            pftl_chain_id: pftl_chain_numeric_id(chain_id)?,
            source_chain_id,
            vault_address,
            token_address,
            vault_bridge_asset_id: asset_id.clone(),
            burn_tx_id,
            withdrawal_id: redemption_id.clone(),
            recipient,
            amount_atoms,
            source_bucket_id: bucket_id.clone(),
            destination_hash,
            finalized_height: created_at_height,
            evidence_root,
        };
        let withdrawal_packet_hash = vault_bridge_withdrawal_packet_hash(&withdrawal_packet)?;
        let withdrawal_packet_evm_digest =
            vault_bridge_withdrawal_packet_evm_digest(&withdrawal_packet)?;
        let redemption = Self {
            redemption_id,
            owner,
            owner_sequence,
            issuer,
            asset_id,
            bucket_id,
            amount_atoms,
            epoch,
            reserve_packet_hash,
            destination_ref,
            settled_atoms: 0,
            state: VAULT_BRIDGE_REDEMPTION_STATE_PENDING.to_string(),
            created_at_height,
            settlement_receipt_hash: String::new(),
            withdrawal_packet,
            withdrawal_packet_hash,
            withdrawal_packet_evm_digest,
            withdrawal_observations: Vec::new(),
        };
        redemption.validate_for_chain(chain_id)?;
        Ok(redemption)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "vault_bridge_redemption.redemption_id",
            &self.redemption_id,
            VAULT_BRIDGE_REDEMPTION_ID_HEX_LEN,
        )?;
        validate_text_field("vault_bridge_redemption.owner", &self.owner)?;
        if self.owner_sequence == 0 {
            return Err("vault_bridge_redemption.owner_sequence must be nonzero".to_string());
        }
        validate_text_field("vault_bridge_redemption.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "vault_bridge_redemption.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_redemption.bucket_id",
            &self.bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_redemption.amount_atoms must be nonzero".to_string());
        }
        if self.epoch == 0 {
            return Err("vault_bridge_redemption.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_redemption.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        validate_text_field(
            "vault_bridge_redemption.destination_ref",
            &self.destination_ref,
        )?;
        if self.settled_atoms > self.amount_atoms {
            return Err("vault_bridge_redemption.settled_atoms exceeds amount_atoms".to_string());
        }
        validate_vault_bridge_redemption_state(&self.state)?;
        if self.state == VAULT_BRIDGE_REDEMPTION_STATE_SETTLED
            && self.settled_atoms != self.amount_atoms
        {
            return Err(
                "vault_bridge_redemption settled state requires settled_atoms == amount_atoms"
                    .to_string(),
            );
        }
        if !self.settlement_receipt_hash.is_empty() {
            validate_lower_hex_len(
                "vault_bridge_redemption.settlement_receipt_hash",
                &self.settlement_receipt_hash,
                96,
            )?;
        }
        let legacy_domainless = self.withdrawal_packet.is_legacy_domainless();
        if legacy_domainless {
            self.withdrawal_packet
                .validate_legacy_domainless_state_record()?;
        } else {
            self.withdrawal_packet.validate()?;
        }
        validate_lower_hex_len(
            "vault_bridge_redemption.withdrawal_packet_hash",
            &self.withdrawal_packet_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_redemption.withdrawal_packet_evm_digest",
            &self.withdrawal_packet_evm_digest,
            VAULT_BRIDGE_EVM_BYTES32_HEX_LEN,
        )?;
        if self.withdrawal_packet.vault_bridge_asset_id != self.asset_id {
            return Err("vault_bridge_redemption withdrawal packet asset_id mismatch".to_string());
        }
        if self.withdrawal_packet.withdrawal_id != self.redemption_id {
            return Err(
                "vault_bridge_redemption withdrawal packet withdrawal_id mismatch".to_string(),
            );
        }
        if self.withdrawal_packet.amount_atoms != self.amount_atoms {
            return Err("vault_bridge_redemption withdrawal packet amount mismatch".to_string());
        }
        if self.withdrawal_packet.source_bucket_id != self.bucket_id {
            return Err("vault_bridge_redemption withdrawal packet bucket mismatch".to_string());
        }
        if self.withdrawal_packet.finalized_height != self.created_at_height {
            return Err(
                "vault_bridge_redemption withdrawal packet finalized height mismatch".to_string(),
            );
        }
        let expected_recipient =
            vault_bridge_evm_recipient_from_destination_ref(&self.destination_ref)?;
        if self.withdrawal_packet.recipient != expected_recipient {
            return Err("vault_bridge_redemption withdrawal packet recipient mismatch".to_string());
        }
        let expected_destination_hash = vault_bridge_destination_ref_hash(&self.destination_ref)?;
        if self.withdrawal_packet.destination_hash != expected_destination_hash {
            return Err(
                "vault_bridge_redemption withdrawal packet destination_hash mismatch".to_string(),
            );
        }
        if !legacy_domainless {
            let expected_packet_hash =
                vault_bridge_withdrawal_packet_hash(&self.withdrawal_packet)?;
            if self.withdrawal_packet_hash != expected_packet_hash {
                return Err("vault_bridge_redemption withdrawal_packet_hash mismatch".to_string());
            }
            let expected_evm_digest =
                vault_bridge_withdrawal_packet_evm_digest(&self.withdrawal_packet)?;
            if self.withdrawal_packet_evm_digest != expected_evm_digest {
                return Err(
                    "vault_bridge_redemption withdrawal_packet_evm_digest mismatch".to_string(),
                );
            }
        }
        let mut observer_roots = BTreeSet::new();
        for attestation in &self.withdrawal_observations {
            attestation.validate()?;
            let key = format!("{}:{}", attestation.attestor, attestation.observation_root);
            if !observer_roots.insert(key) {
                return Err(
                    "vault_bridge_redemption has duplicate withdrawal observation attestations"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected =
            vault_bridge_redemption_id(chain_id, &self.owner, &self.asset_id, self.owner_sequence)?;
        if self.redemption_id != expected {
            return Err(
                "vault_bridge_redemption.redemption_id does not match chain, owner, asset, and sequence"
                    .to_string(),
            );
        }
        let expected_chain_id = pftl_chain_numeric_id(chain_id)?;
        if self.withdrawal_packet.pftl_chain_id != expected_chain_id {
            return Err(
                "vault_bridge_redemption withdrawal packet PFTL chain id mismatch".to_string(),
            );
        }
        let expected_evidence_root = vault_bridge_redemption_evidence_root(
            chain_id,
            &self.withdrawal_packet.burn_tx_id,
            &self.owner,
            &self.issuer,
            &self.asset_id,
            &self.bucket_id,
            self.owner_sequence,
            self.amount_atoms,
            self.epoch,
            &self.reserve_packet_hash,
            &self.destination_ref,
            self.created_at_height,
        )?;
        if self.withdrawal_packet.evidence_root != expected_evidence_root {
            return Err(
                "vault_bridge_redemption withdrawal packet evidence_root mismatch".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavRedemption {
    pub redemption_id: String,
    pub owner: String,
    pub owner_sequence: u64,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
    pub epoch: u64,
    pub nav_per_unit: u64,
    #[serde(
        default = "nav_redemption_default_unit_scale",
        skip_serializing_if = "nav_redemption_unit_scale_is_default"
    )]
    pub unit_scale: u128,
    pub reserve_packet_hash: String,
    pub redemption_claim: u64,
    #[serde(default)]
    pub state: String,
    /// Block height at which the redemption claim was created; input to the
    /// settlement-deadline check. 0 for legacy claims.
    #[serde(default)]
    pub created_at_height: u64,
    /// Hash of the off-chain settlement receipt posted via
    /// nav_redeem_settle. Empty while pending.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub settlement_receipt_hash: String,
}

fn nav_redemption_default_unit_scale() -> u128 {
    1
}

fn nav_redemption_unit_scale_is_default(unit_scale: &u128) -> bool {
    *unit_scale == nav_redemption_default_unit_scale()
}

impl NavRedemption {
    pub fn new(
        chain_id: &str,
        owner: impl Into<String>,
        issuer: impl Into<String>,
        asset_id: impl Into<String>,
        owner_sequence: u64,
        amount: u64,
        epoch: u64,
        nav_per_unit: u64,
        reserve_packet_hash: impl Into<String>,
    ) -> Result<Self, String> {
        Self::new_with_unit_scale(
            chain_id,
            owner,
            issuer,
            asset_id,
            owner_sequence,
            amount,
            epoch,
            nav_per_unit,
            1,
            reserve_packet_hash,
        )
    }

    pub fn new_with_unit_scale(
        chain_id: &str,
        owner: impl Into<String>,
        issuer: impl Into<String>,
        asset_id: impl Into<String>,
        owner_sequence: u64,
        amount: u64,
        epoch: u64,
        nav_per_unit: u64,
        unit_scale: u128,
        reserve_packet_hash: impl Into<String>,
    ) -> Result<Self, String> {
        let owner = owner.into();
        let asset_id = asset_id.into();
        let redemption_id = nav_redemption_id(chain_id, &owner, &asset_id, owner_sequence)?;
        let redemption_claim = nav_amount_claim_ceil(amount, nav_per_unit, unit_scale)
            .map_err(|error| format!("nav_redemption claim invalid: {error}"))?;
        let redemption = Self {
            redemption_id,
            owner,
            owner_sequence,
            issuer: issuer.into(),
            asset_id,
            amount,
            epoch,
            nav_per_unit,
            unit_scale,
            reserve_packet_hash: reserve_packet_hash.into(),
            redemption_claim,
            state: NAV_REDEMPTION_STATE_PENDING.to_string(),
            created_at_height: 0,
            settlement_receipt_hash: String::new(),
        };
        redemption.validate_for_chain(chain_id)?;
        Ok(redemption)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "nav_redemption.redemption_id",
            &self.redemption_id,
            NAV_REDEMPTION_ID_HEX_LEN,
        )?;
        validate_text_field("nav_redemption.owner", &self.owner)?;
        if self.owner_sequence == 0 {
            return Err("nav_redemption.owner_sequence must be nonzero".to_string());
        }
        validate_text_field("nav_redemption.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "nav_redemption.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err("nav_redemption.amount must be nonzero".to_string());
        }
        if self.epoch == 0 {
            return Err("nav_redemption.epoch must be nonzero".to_string());
        }
        if self.nav_per_unit == 0 {
            return Err("nav_redemption.nav_per_unit must be nonzero".to_string());
        }
        if self.unit_scale == 0 {
            return Err("nav_redemption.unit_scale must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "nav_redemption.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        validate_nav_redemption_state(&self.state)?;
        if !self.settlement_receipt_hash.is_empty() {
            validate_lower_hex_len(
                "nav_redemption.settlement_receipt_hash",
                &self.settlement_receipt_hash,
                96,
            )?;
        }
        let expected_claim = nav_amount_claim_ceil(self.amount, self.nav_per_unit, self.unit_scale)
            .map_err(|error| format!("nav_redemption claim invalid: {error}"))?;
        if self.redemption_claim != expected_claim {
            return Err(
                "nav_redemption.redemption_claim must equal scaled amount * nav_per_unit"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected_id =
            nav_redemption_id(chain_id, &self.owner, &self.asset_id, self.owner_sequence)?;
        if self.redemption_id != expected_id {
            return Err(
                "nav_redemption.redemption_id does not match chain, owner, asset, and sequence"
                    .to_string(),
            );
        }
        Ok(())
    }
}

/// A governance-registered proof profile: the protocol-level definition of
/// what counts as reserve evidence for a NAV asset, and the timing rules
/// (in blocks) that consensus enforces around it. Profiles are
/// content-addressed and immutable: `profile_id` is a hash of the profile
/// parameters, so identical parameters always resolve to the same id and a
/// registered profile can never be quietly altered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavProofProfile {
    pub profile_id: String,
    pub registered_by: String,
    pub verifier_kind: String,
    /// Which source class observers consult: "ledger" for on-ledger
    /// reserves, or an external venue identifier (e.g.
    /// "hyperliquid-testnet") for multi-fetch profiles.
    #[serde(default)]
    pub source_class: String,
    /// Max blocks between packet submission and finalization before the
    /// packet is considered stale and unfinalizable. 0 disables the bound.
    #[serde(default)]
    pub max_snapshot_age_blocks: u64,
    /// Min blocks between submission and finalization (the bonded-challenge
    /// window). 0 allows immediate finalization.
    #[serde(default)]
    pub challenge_window_blocks: u64,
    /// Deadman switch: max blocks a finalized packet stays live for
    /// mint/redeem. 0 disables the bound.
    #[serde(default)]
    pub max_epoch_gap_blocks: u64,
    /// Max blocks a redemption may stay unsettled before minting is
    /// blocked for the asset. 0 disables the bound.
    #[serde(default)]
    pub settle_deadline_blocks: u64,
    /// Minimum bond (native units) a challenger must escrow.
    #[serde(default)]
    pub min_challenge_bond: u64,
    /// For multi-fetch profiles: distinct pass attestations required
    /// before the packet may finalize. 0 for non-attested kinds.
    #[serde(default)]
    pub min_attestations: u64,
    /// For multi-fetch profiles: the relative tolerance (basis points)
    /// within which an observer's own observation must match the packet's
    /// verified_net_assets to merit a pass verdict. Registered on-chain so
    /// the comparison rule is itself a content-addressed fact. 0 = exact.
    #[serde(default)]
    pub tolerance_bp: u64,
    /// For vault-bridge multi-fetch profiles: minimum source-chain
    /// confirmations an observer must attest before a deposit credit or
    /// withdrawal settlement can finalize. 0 preserves legacy non-EVM
    /// multi-fetch semantics.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub bridge_observer_min_confirmations: u64,
    /// Hash of the valuation policy observers enforce: leg enumeration,
    /// mark sources, haircuts, and strategy invariants (hedge bands,
    /// margin ratios). Empty for profiles whose source is self-marking.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub valuation_policy_hash: String,
    /// Exact governed vault-route profile hash for bridge-backed assets. This
    /// is separate from an SP1 proof's 32-byte valuation-policy hash.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vault_bridge_route_policy_hash: String,
    /// SP1 program verifying key hash (`vk.bytes32()`), e.g. `0x004d1cd3…`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sp1_program_vkey: String,
    /// Proof encoding accepted by this profile (today: `groth16`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sp1_proof_encoding: String,
    /// Max Groth16 proof bytes accepted at submit. 0 = protocol default.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub max_proof_bytes: u64,
    /// Max SP1 public-values bytes accepted at submit. 0 = protocol default.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub max_public_values_bytes: u64,
}

/// PFTL-finalized authorization packet for bounded NAVCoin market operations.
///
/// The hash preimage intentionally does not use serde or JSON. Bridge-facing
/// hashes must be stable across languages and clients, so `envelope_hash` uses
/// the explicit field order from the NAVCoin collateralization spec and encodes
/// integer values as fixed-width big-endian words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsEnvelope {
    pub encoding_version: u32,
    pub chain_id: u64,
    pub adapter_address: [u8; 20],
    pub vault_address: [u8; 20],
    pub mint_controller_address: [u8; 20],
    pub asset_id: [u8; 32],
    pub epoch: u64,
    pub program_id: [u8; 32],
    pub policy_hash: [u8; 32],
    pub parameter_hash: [u8; 32],
    pub reserve_packet_hash: [u8; 32],
    pub supply_packet_hash: [u8; 32],
    pub evidence_root: [u8; 32],
    pub previous_market_state_hash: [u8; 32],
    pub venue_id: [u8; 32],
    pub pool_config_hash: [u8; 32],
    pub hook_code_hash: [u8; 32],
    pub nav_floor_usd_e8: u128,
    pub valid_global_supply_atoms: u128,
    pub verified_net_assets_usd_e8: u128,
    pub funded_alignment_reserve_usd_e8: u128,
    pub required_alignment_reserve_usd_e8: u128,
    pub max_reserve_deploy_usd_e8: u128,
    pub max_mint_atoms: u128,
    pub discount_trigger_bps: u32,
    pub premium_trigger_bps: u32,
    pub data_window_start: u64,
    pub data_window_end: u64,
    pub valid_after: u64,
    pub expires_at: u64,
    pub cooldown_seconds: u64,
    pub nonce: [u8; 32],
}

impl MarketOpsEnvelope {
    /// SHA3-384 over every consensus-relevant envelope field.
    pub fn envelope_hash(&self) -> [u8; 48] {
        let mut hasher = Sha3_384::new();

        hash_u32(&mut hasher, self.encoding_version);
        hash_u64(&mut hasher, self.chain_id);
        hasher.update(self.adapter_address);
        hasher.update(self.vault_address);
        hasher.update(self.mint_controller_address);
        hasher.update(self.asset_id);
        hash_u64(&mut hasher, self.epoch);
        hasher.update(self.program_id);
        hasher.update(self.policy_hash);
        hasher.update(self.parameter_hash);
        hasher.update(self.reserve_packet_hash);
        hasher.update(self.supply_packet_hash);
        hasher.update(self.evidence_root);
        hasher.update(self.previous_market_state_hash);
        hasher.update(self.venue_id);
        hasher.update(self.pool_config_hash);
        hasher.update(self.hook_code_hash);
        hash_uint256_from_u128(&mut hasher, self.nav_floor_usd_e8);
        hash_uint256_from_u128(&mut hasher, self.valid_global_supply_atoms);
        hash_uint256_from_u128(&mut hasher, self.verified_net_assets_usd_e8);
        hash_uint256_from_u128(&mut hasher, self.funded_alignment_reserve_usd_e8);
        hash_uint256_from_u128(&mut hasher, self.required_alignment_reserve_usd_e8);
        hash_uint256_from_u128(&mut hasher, self.max_reserve_deploy_usd_e8);
        hash_uint256_from_u128(&mut hasher, self.max_mint_atoms);
        hash_u32(&mut hasher, self.discount_trigger_bps);
        hash_u32(&mut hasher, self.premium_trigger_bps);
        hash_u64(&mut hasher, self.data_window_start);
        hash_u64(&mut hasher, self.data_window_end);
        hash_u64(&mut hasher, self.valid_after);
        hash_u64(&mut hasher, self.expires_at);
        hash_u64(&mut hasher, self.cooldown_seconds);
        hasher.update(self.nonce);

        let digest = hasher.finalize();
        let mut output = [0u8; 48];
        output.copy_from_slice(&digest);
        output
    }

    pub fn validate_basic(&self) -> Result<(), String> {
        if self.encoding_version == 0 {
            return Err("market_ops_envelope.encoding_version must be nonzero".to_string());
        }
        if self.chain_id == 0 {
            return Err("market_ops_envelope.chain_id must be nonzero".to_string());
        }
        if self.epoch == 0 {
            return Err("market_ops_envelope.epoch must be nonzero".to_string());
        }
        if is_zero_20(&self.adapter_address) {
            return Err("market_ops_envelope.adapter_address must be nonzero".to_string());
        }
        if is_zero_20(&self.vault_address) {
            return Err("market_ops_envelope.vault_address must be nonzero".to_string());
        }
        if is_zero_20(&self.mint_controller_address) {
            return Err("market_ops_envelope.mint_controller_address must be nonzero".to_string());
        }
        if is_zero_32(&self.asset_id) {
            return Err("market_ops_envelope.asset_id must be nonzero".to_string());
        }
        if is_zero_32(&self.program_id) {
            return Err("market_ops_envelope.program_id must be nonzero".to_string());
        }
        if is_zero_32(&self.policy_hash) {
            return Err("market_ops_envelope.policy_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.parameter_hash) {
            return Err("market_ops_envelope.parameter_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.reserve_packet_hash) {
            return Err("market_ops_envelope.reserve_packet_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.supply_packet_hash) {
            return Err("market_ops_envelope.supply_packet_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.evidence_root) {
            return Err("market_ops_envelope.evidence_root must be nonzero".to_string());
        }
        if is_zero_32(&self.venue_id) {
            return Err("market_ops_envelope.venue_id must be nonzero".to_string());
        }
        if is_zero_32(&self.pool_config_hash) {
            return Err("market_ops_envelope.pool_config_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.hook_code_hash) {
            return Err("market_ops_envelope.hook_code_hash must be nonzero".to_string());
        }
        if self.nav_floor_usd_e8 == 0 {
            return Err("market_ops_envelope.nav_floor_usd_e8 must be nonzero".to_string());
        }
        if self.valid_global_supply_atoms == 0 {
            return Err(
                "market_ops_envelope.valid_global_supply_atoms must be nonzero".to_string(),
            );
        }
        if self.verified_net_assets_usd_e8 == 0 {
            return Err(
                "market_ops_envelope.verified_net_assets_usd_e8 must be nonzero".to_string(),
            );
        }
        if self.discount_trigger_bps > 10_000 {
            return Err("market_ops_envelope.discount_trigger_bps exceeds 10000".to_string());
        }
        if self.premium_trigger_bps > 10_000 {
            return Err("market_ops_envelope.premium_trigger_bps exceeds 10000".to_string());
        }
        if self.data_window_start >= self.data_window_end {
            return Err(
                "market_ops_envelope.data_window_start must be before data_window_end".to_string(),
            );
        }
        if self.valid_after > self.expires_at {
            return Err("market_ops_envelope.valid_after must be <= expires_at".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsPolicyRegistration {
    pub program_id: [u8; 32],
    pub policy_hash: [u8; 32],
    pub parameter_hash: [u8; 32],
    pub venue_id: [u8; 32],
    pub pool_config_hash: [u8; 32],
    pub hook_code_hash: [u8; 32],
    #[serde(default)]
    pub activation_epoch: u64,
    #[serde(default)]
    pub deactivation_epoch: u64,
}

impl MarketOpsPolicyRegistration {
    pub fn validate(&self) -> Result<(), String> {
        if is_zero_32(&self.program_id) {
            return Err("market_ops_policy.program_id must be nonzero".to_string());
        }
        if is_zero_32(&self.policy_hash) {
            return Err("market_ops_policy.policy_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.parameter_hash) {
            return Err("market_ops_policy.parameter_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.venue_id) {
            return Err("market_ops_policy.venue_id must be nonzero".to_string());
        }
        if is_zero_32(&self.pool_config_hash) {
            return Err("market_ops_policy.pool_config_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.hook_code_hash) {
            return Err("market_ops_policy.hook_code_hash must be nonzero".to_string());
        }
        if self.deactivation_epoch != 0 && self.deactivation_epoch <= self.activation_epoch {
            return Err(
                "market_ops_policy.deactivation_epoch must be zero or greater than activation_epoch"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub fn accepts(&self, envelope: &MarketOpsEnvelope) -> bool {
        self.program_id == envelope.program_id
            && self.policy_hash == envelope.policy_hash
            && self.parameter_hash == envelope.parameter_hash
            && self.venue_id == envelope.venue_id
            && self.pool_config_hash == envelope.pool_config_hash
            && self.hook_code_hash == envelope.hook_code_hash
            && envelope.epoch >= self.activation_epoch
            && (self.deactivation_epoch == 0 || envelope.epoch < self.deactivation_epoch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsVenueObservation {
    pub dt_seconds: u64,
    pub price_usd_e8: u128,
    pub volume_usd_e8: u128,
}

impl MarketOpsVenueObservation {
    pub fn validate(&self) -> Result<(), String> {
        if self.dt_seconds == 0 {
            return Err("market_ops_observation.dt_seconds must be nonzero".to_string());
        }
        if self.price_usd_e8 == 0 {
            return Err("market_ops_observation.price_usd_e8 must be nonzero".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsEvmEvidenceBundle {
    pub encoding_version: u32,
    pub chain_id: u64,
    pub venue_id: [u8; 32],
    pub pool_id: [u8; 32],
    pub pool_manager: [u8; 20],
    pub hook_address: [u8; 20],
    pub pool_config_hash: [u8; 32],
    pub hook_code_hash: [u8; 32],
    pub headers: Vec<MarketOpsEvmHeaderEvidence>,
    pub receipts: Vec<MarketOpsEvmReceiptEvidence>,
    pub logs: Vec<MarketOpsEvmLogEvidence>,
    pub hook_checkpoints: Vec<MarketOpsHookCheckpointEvidence>,
    pub pool_states: Vec<MarketOpsEvmPoolStateEvidence>,
}

impl MarketOpsEvmEvidenceBundle {
    pub fn validate(&self) -> Result<(), String> {
        if self.encoding_version == 0 {
            return Err("market_ops_evm_evidence.encoding_version must be nonzero".to_string());
        }
        if self.chain_id == 0 {
            return Err("market_ops_evm_evidence.chain_id must be nonzero".to_string());
        }
        if is_zero_32(&self.venue_id) {
            return Err("market_ops_evm_evidence.venue_id must be nonzero".to_string());
        }
        if is_zero_32(&self.pool_id) {
            return Err("market_ops_evm_evidence.pool_id must be nonzero".to_string());
        }
        if is_zero_20(&self.pool_manager) {
            return Err("market_ops_evm_evidence.pool_manager must be nonzero".to_string());
        }
        if is_zero_20(&self.hook_address) {
            return Err("market_ops_evm_evidence.hook_address must be nonzero".to_string());
        }
        if is_zero_32(&self.pool_config_hash) {
            return Err("market_ops_evm_evidence.pool_config_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.hook_code_hash) {
            return Err("market_ops_evm_evidence.hook_code_hash must be nonzero".to_string());
        }
        validate_nonempty_bounded(
            "market_ops_evm_evidence.headers",
            self.headers.len(),
            MAX_MARKET_OPS_EVM_HEADERS,
        )?;
        validate_nonempty_bounded(
            "market_ops_evm_evidence.receipts",
            self.receipts.len(),
            MAX_MARKET_OPS_EVM_RECEIPTS,
        )?;
        validate_nonempty_bounded(
            "market_ops_evm_evidence.logs",
            self.logs.len(),
            MAX_MARKET_OPS_EVM_LOGS,
        )?;
        validate_nonempty_bounded(
            "market_ops_evm_evidence.hook_checkpoints",
            self.hook_checkpoints.len(),
            MAX_MARKET_OPS_EVM_CHECKPOINTS,
        )?;
        validate_nonempty_bounded(
            "market_ops_evm_evidence.pool_states",
            self.pool_states.len(),
            MAX_MARKET_OPS_EVM_POOL_STATES,
        )?;

        let mut previous_header_block = 0;
        for header in &self.headers {
            header.validate()?;
            if header.block_number <= previous_header_block {
                return Err(
                    "market_ops_evm_evidence.headers must be strictly ordered by block_number"
                        .to_string(),
                );
            }
            previous_header_block = header.block_number;
        }

        let mut previous_receipt_key: Option<(u64, u32)> = None;
        for receipt in &self.receipts {
            receipt.validate()?;
            let key = (receipt.block_number, receipt.transaction_index);
            if previous_receipt_key.is_some_and(|previous| key <= previous) {
                return Err(
                    "market_ops_evm_evidence.receipts must be strictly ordered by block_number,transaction_index"
                        .to_string(),
                );
            }
            previous_receipt_key = Some(key);
        }

        let mut previous_log_key: Option<(u64, u32, u32)> = None;
        for log in &self.logs {
            log.validate()?;
            let key = (log.block_number, log.transaction_index, log.log_index);
            if previous_log_key.is_some_and(|previous| key <= previous) {
                return Err(
                    "market_ops_evm_evidence.logs must be strictly ordered by block_number,transaction_index,log_index"
                        .to_string(),
                );
            }
            previous_log_key = Some(key);
        }

        let mut previous_checkpoint_key: Option<(u64, u32)> = None;
        for checkpoint in &self.hook_checkpoints {
            checkpoint.validate()?;
            if checkpoint.pool_id != self.pool_id {
                return Err("market_ops_evm_evidence.hook_checkpoint pool_id mismatch".to_string());
            }
            let key = (checkpoint.block_number, checkpoint.log_index);
            if previous_checkpoint_key.is_some_and(|previous| key <= previous) {
                return Err(
                    "market_ops_evm_evidence.hook_checkpoints must be strictly ordered by block_number,log_index"
                        .to_string(),
                );
            }
            previous_checkpoint_key = Some(key);
        }

        let mut previous_pool_state_key: Option<(u64, u64)> = None;
        for pool_state in &self.pool_states {
            pool_state.validate()?;
            let key = (pool_state.block_number, pool_state.observation_sequence);
            if previous_pool_state_key.is_some_and(|previous| key <= previous) {
                return Err(
                    "market_ops_evm_evidence.pool_states must be strictly ordered by block_number,observation_sequence"
                        .to_string(),
                );
            }
            previous_pool_state_key = Some(key);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsEvmHeaderEvidence {
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub timestamp: u64,
}

impl MarketOpsEvmHeaderEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.block_number == 0 {
            return Err("market_ops_evm_header.block_number must be nonzero".to_string());
        }
        if is_zero_32(&self.block_hash) {
            return Err("market_ops_evm_header.block_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.parent_hash) {
            return Err("market_ops_evm_header.parent_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.state_root) {
            return Err("market_ops_evm_header.state_root must be nonzero".to_string());
        }
        if is_zero_32(&self.receipts_root) {
            return Err("market_ops_evm_header.receipts_root must be nonzero".to_string());
        }
        if self.timestamp == 0 {
            return Err("market_ops_evm_header.timestamp must be nonzero".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsEvmReceiptEvidence {
    pub block_number: u64,
    pub transaction_index: u32,
    pub receipt_hash: [u8; 32],
    pub status: bool,
    pub logs_root: [u8; 32],
}

impl MarketOpsEvmReceiptEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.block_number == 0 {
            return Err("market_ops_evm_receipt.block_number must be nonzero".to_string());
        }
        if is_zero_32(&self.receipt_hash) {
            return Err("market_ops_evm_receipt.receipt_hash must be nonzero".to_string());
        }
        if is_zero_32(&self.logs_root) {
            return Err("market_ops_evm_receipt.logs_root must be nonzero".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsEvmLogEvidence {
    pub block_number: u64,
    pub transaction_index: u32,
    pub log_index: u32,
    pub address: [u8; 20],
    pub topics: Vec<[u8; 32]>,
    pub data_hash: [u8; 32],
}

impl MarketOpsEvmLogEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.block_number == 0 {
            return Err("market_ops_evm_log.block_number must be nonzero".to_string());
        }
        if is_zero_20(&self.address) {
            return Err("market_ops_evm_log.address must be nonzero".to_string());
        }
        if self.topics.is_empty() {
            return Err("market_ops_evm_log.topics must be nonempty".to_string());
        }
        if self.topics.len() > MAX_MARKET_OPS_EVM_LOG_TOPICS {
            return Err(format!(
                "market_ops_evm_log.topics exceeds maximum of {MAX_MARKET_OPS_EVM_LOG_TOPICS}"
            ));
        }
        if is_zero_32(&self.topics[0]) {
            return Err("market_ops_evm_log.topic0 must be nonzero".to_string());
        }
        if is_zero_32(&self.data_hash) {
            return Err("market_ops_evm_log.data_hash must be nonzero".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsHookCheckpointEvidence {
    pub block_number: u64,
    pub log_index: u32,
    pub pool_id: [u8; 32],
    pub checkpoint_count: u64,
    pub swap_count: u64,
    pub depth_count: u64,
    pub swap_root: [u8; 32],
    pub depth_root: [u8; 32],
    pub pftl_state_hash: [u8; 32],
}

impl MarketOpsHookCheckpointEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.block_number == 0 {
            return Err("market_ops_hook_checkpoint.block_number must be nonzero".to_string());
        }
        if is_zero_32(&self.pool_id) {
            return Err("market_ops_hook_checkpoint.pool_id must be nonzero".to_string());
        }
        if self.checkpoint_count == 0 {
            return Err("market_ops_hook_checkpoint.checkpoint_count must be nonzero".to_string());
        }
        if self.swap_count == 0 && self.depth_count == 0 {
            return Err(
                "market_ops_hook_checkpoint must commit to at least one observation".to_string(),
            );
        }
        if is_zero_32(&self.pftl_state_hash) {
            return Err("market_ops_hook_checkpoint.pftl_state_hash must be nonzero".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsEvmPoolStateEvidence {
    pub block_number: u64,
    pub observation_sequence: u64,
    pub timestamp: u64,
    pub dt_seconds: u64,
    pub checkpoint_count: u64,
    pub price_usd_e8: u128,
    pub volume_usd_e8: u128,
    pub zero_for_one: bool,
    pub fee_bps: u32,
    pub liquidity: u128,
    pub base_reserve_atoms: u128,
    pub quote_reserve_usd_e8: u128,
    pub replayable: bool,
}

impl MarketOpsEvmPoolStateEvidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.block_number == 0 {
            return Err("market_ops_evm_pool_state.block_number must be nonzero".to_string());
        }
        if self.observation_sequence == 0 {
            return Err(
                "market_ops_evm_pool_state.observation_sequence must be nonzero".to_string(),
            );
        }
        if self.timestamp == 0 {
            return Err("market_ops_evm_pool_state.timestamp must be nonzero".to_string());
        }
        if self.dt_seconds == 0 {
            return Err("market_ops_evm_pool_state.dt_seconds must be nonzero".to_string());
        }
        if self.checkpoint_count == 0 {
            return Err("market_ops_evm_pool_state.checkpoint_count must be nonzero".to_string());
        }
        if self.price_usd_e8 == 0 {
            return Err("market_ops_evm_pool_state.price_usd_e8 must be nonzero".to_string());
        }
        if self.fee_bps > 10_000 {
            return Err("market_ops_evm_pool_state.fee_bps exceeds 10000".to_string());
        }
        if self.replayable {
            if self.liquidity == 0 {
                return Err(
                    "market_ops_evm_pool_state.liquidity must be nonzero when replayable"
                        .to_string(),
                );
            }
            if self.base_reserve_atoms == 0 {
                return Err(
                    "market_ops_evm_pool_state.base_reserve_atoms must be nonzero when replayable"
                        .to_string(),
                );
            }
            if self.quote_reserve_usd_e8 == 0 {
                return Err(
                    "market_ops_evm_pool_state.quote_reserve_usd_e8 must be nonzero when replayable"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsAlignmentParams {
    pub policy_min_usd_e8: u128,
    pub min_alignment_bps: u32,
    pub stress_repeat_factor_14d: u128,
    pub stress_repeat_factor_90d: u128,
    pub stale_epochs_allowed: u128,
    pub max_decay_per_epoch_bps: u32,
}

impl MarketOpsAlignmentParams {
    pub fn validate(&self) -> Result<(), String> {
        if self.min_alignment_bps > 10_000 {
            return Err("market_ops_alignment.min_alignment_bps exceeds 10000".to_string());
        }
        if self.max_decay_per_epoch_bps > 10_000 {
            return Err("market_ops_alignment.max_decay_per_epoch_bps exceeds 10000".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsReserveDeployLimits {
    pub available_alignment_reserve_usd_e8: u128,
    pub venue_policy_cap_usd_e8: u128,
    pub depth_limited_cap_usd_e8: u128,
    pub cooldown_limited_cap_usd_e8: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsMintLimits {
    pub policy_max_mint_atoms: u128,
    pub venue_bid_depth_atoms: u128,
    pub cooldown_mint_atoms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsPolicyInputs {
    pub unit_scale: u128,
    pub floor_factor_bps: u32,
    pub alignment_params: MarketOpsAlignmentParams,
    pub previous_required_alignment_reserve_usd_e8: u128,
    pub cost_to_restore_14d_usd_e8: Vec<u128>,
    pub cost_to_restore_90d_usd_e8: Vec<u128>,
    pub reserve_limits: MarketOpsReserveDeployLimits,
    pub mint_limits: MarketOpsMintLimits,
    pub discount_observations: Vec<MarketOpsVenueObservation>,
    pub premium_observations: Vec<MarketOpsVenueObservation>,
}

impl MarketOpsPolicyInputs {
    pub fn validate(&self) -> Result<(), String> {
        if self.unit_scale == 0 {
            return Err("market_ops_policy_inputs.unit_scale must be nonzero".to_string());
        }
        if self.floor_factor_bps > 10_000 {
            return Err("market_ops_policy_inputs.floor_factor_bps exceeds 10000".to_string());
        }
        self.alignment_params.validate()?;
        if self.cost_to_restore_14d_usd_e8.is_empty() {
            return Err(
                "market_ops_policy_inputs.cost_to_restore_14d_usd_e8 must be nonempty".to_string(),
            );
        }
        if self.cost_to_restore_90d_usd_e8.is_empty() {
            return Err(
                "market_ops_policy_inputs.cost_to_restore_90d_usd_e8 must be nonempty".to_string(),
            );
        }
        if self.cost_to_restore_14d_usd_e8.len() > MAX_MARKET_OPS_COST_SAMPLES {
            return Err(format!(
                "market_ops_policy_inputs.cost_to_restore_14d_usd_e8 exceeds maximum of {MAX_MARKET_OPS_COST_SAMPLES}"
            ));
        }
        if self.cost_to_restore_90d_usd_e8.len() > MAX_MARKET_OPS_COST_SAMPLES {
            return Err(format!(
                "market_ops_policy_inputs.cost_to_restore_90d_usd_e8 exceeds maximum of {MAX_MARKET_OPS_COST_SAMPLES}"
            ));
        }
        if self.discount_observations.len() > MAX_MARKET_OPS_OBSERVATIONS {
            return Err(format!(
                "market_ops_policy_inputs.discount_observations exceeds maximum of {MAX_MARKET_OPS_OBSERVATIONS}"
            ));
        }
        if self.premium_observations.len() > MAX_MARKET_OPS_OBSERVATIONS {
            return Err(format!(
                "market_ops_policy_inputs.premium_observations exceeds maximum of {MAX_MARKET_OPS_OBSERVATIONS}"
            ));
        }
        for observation in &self.discount_observations {
            observation.validate()?;
        }
        for observation in &self.premium_observations {
            observation.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizedMarketOpsEnvelope {
    pub asset_id: String,
    pub epoch: u64,
    pub envelope_hash: String,
    pub envelope: MarketOpsEnvelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_inputs: Option<MarketOpsPolicyInputs>,
    #[serde(default)]
    pub finalized_at_height: u64,
}

impl FinalizedMarketOpsEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "market_ops_finalized.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.epoch == 0 {
            return Err("market_ops_finalized.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "market_ops_finalized.envelope_hash",
            &self.envelope_hash,
            96,
        )?;
        self.envelope.validate_basic()?;
        if self.envelope.epoch != self.epoch {
            return Err("market_ops_finalized.epoch must match envelope.epoch".to_string());
        }
        if self.envelope.asset_id != market_ops_asset_id(&self.asset_id)? {
            return Err(
                "market_ops_finalized.envelope.asset_id must match ledger asset_id".to_string(),
            );
        }
        if self.envelope_hash != bytes_to_lower_hex(&self.envelope.envelope_hash()) {
            return Err(
                "market_ops_finalized.envelope_hash must match envelope.envelope_hash".to_string(),
            );
        }
        if let Some(policy_inputs) = &self.policy_inputs {
            policy_inputs.validate()?;
        }
        Ok(())
    }
}
