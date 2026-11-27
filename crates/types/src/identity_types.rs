#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    Human,
    Agent,
    Service,
}

impl IdentityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Service => "service",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityStatus {
    Active,
    Revoked,
}

impl IdentityStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRotateMode {
    Planned,
    Compromised,
}

impl IdentityRotateMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Compromised => "compromised",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityLinkKind {
    ValidatorNode,
    OldChainAccount,
    AgentController,
    CustomNip05,
    EvmAddress,
    SolanaAddress,
}

impl IdentityLinkKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ValidatorNode => "validator_node",
            Self::OldChainAccount => "old_chain_account",
            Self::AgentController => "agent_controller",
            Self::CustomNip05 => "custom_nip05",
            Self::EvmAddress => "evm_address",
            Self::SolanaAddress => "solana_address",
        }
    }

    pub fn from_str_kind(value: &str) -> Result<Self, String> {
        match value {
            "validator_node" => Ok(Self::ValidatorNode),
            "old_chain_account" => Ok(Self::OldChainAccount),
            "agent_controller" => Ok(Self::AgentController),
            "custom_nip05" => Ok(Self::CustomNip05),
            "evm_address" => Ok(Self::EvmAddress),
            "solana_address" => Ok(Self::SolanaAddress),
            _ => Err(format!("unknown identity link kind `{value}`")),
        }
    }

    pub fn is_consensus_verified(&self) -> bool {
        matches!(
            self,
            Self::ValidatorNode | Self::OldChainAccount | Self::AgentController
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityLinkStatus {
    Active,
    PendingReconfirmation,
}

impl IdentityLinkStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::PendingReconfirmation => "pending_reconfirmation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityLinkV1 {
    /// 96 lowercase hex = SHA3-384(IDENTITY_LINK_ID_DOMAIN || account || kind || target || seq).
    pub link_id: String,
    pub kind: IdentityLinkKind,
    pub target: String,
    pub consensus_verified: bool,
    pub status: IdentityLinkStatus,
    pub linked_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_hash: Option<String>,
}

impl IdentityLinkV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("identity_link.link_id", &self.link_id, 96)?;
        validate_text_field("identity_link.target", &self.target)?;
        if let Some(metadata_hash) = &self.metadata_hash {
            validate_lower_hex_len("identity_link.metadata_hash", metadata_hash, 96)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRecordV1 {
    pub account: String,
    /// 64 lowercase hex (32-byte x-only Nostr secp256k1 pubkey).
    pub nostr_pubkey: String,
    /// 1 = legacy old-chain derived v1 npub; 2 = new-chain seed KDF v2 npub.
    pub derivation_version: u8,
    pub key_index: u32,
    pub identity_kind: IdentityKind,
    /// Monotonic identity-local revision counter (starts at 1 on mint).
    pub sequence: u64,
    pub status: IdentityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered_namespace: Option<String>,
    /// Canonical lowercase ASCII `[a-z0-9._-]`, 2..=32 chars, no leading/trailing punctuation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    /// Max 5 `wss://` URLs, each <= 128 bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relays: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_nft_id: Option<String>,
    /// Optional Pillar 2 X-Wing public key (2432 lowercase hex = 1216 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key: Option<String>,
    /// Optional Pillar 5 shielded receive address (`pfpq1...`, <= 128 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pq_receive_address: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<IdentityLinkV1>,
    pub created_at_height: u64,
    pub updated_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_height: Option<u64>,
}

impl IdentityRecordV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("identity.account", &self.account)?;
        validate_lower_hex_len(
            "identity.nostr_pubkey",
            &self.nostr_pubkey,
            IDENTITY_NOSTR_PUBKEY_HEX_LEN,
        )?;
        if self.derivation_version != 1 && self.derivation_version != 2 {
            return Err("identity.derivation_version must be 1 or 2".to_string());
        }
        if self.sequence == 0 {
            return Err("identity.sequence must be nonzero".to_string());
        }
        if self.registered_namespace.is_some() != self.handle.is_some() {
            return Err(
                "identity.registered_namespace and identity.handle must both be set or both absent"
                    .to_string(),
            );
        }
        if let Some(ns) = &self.registered_namespace {
            validate_identity_namespace(ns)?;
        }
        if let Some(handle) = &self.handle {
            validate_identity_handle(handle)?;
        }
        validate_identity_relays(&self.relays)?;
        if let Some(nft_id) = &self.avatar_nft_id {
            validate_lower_hex_len("identity.avatar_nft_id", nft_id, 96)?;
        }
        if let Some(message_key) = &self.message_key {
            validate_lower_hex_len(
                "identity.message_key",
                message_key,
                IDENTITY_XWING_PUBKEY_HEX_LEN,
            )?;
        }
        if let Some(pq_addr) = &self.pq_receive_address {
            validate_identity_pq_receive_address(pq_addr)?;
        }
        if self.links.len() > IDENTITY_MAX_LINKS {
            return Err(format!(
                "identity.links exceeds maximum of {IDENTITY_MAX_LINKS}"
            ));
        }
        for link in &self.links {
            link.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHistoryEventV1 {
    pub sequence: u64,
    pub block_height: u64,
    pub action: String,
    pub detail: String,
}

impl IdentityHistoryEventV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("identity_history.action", &self.action)?;
        validate_optional_text_field("identity_history.detail", &self.detail, 256)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleNamespaceRegistrarV1 {
    pub namespace: String,
    pub hot_attestation_account: String,
    pub cold_signers: [String; 3],
    pub reassignment_timelock_blocks: u64,
    pub active: bool,
    pub registered_at_height: u64,
}

impl HandleNamespaceRegistrarV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_identity_namespace(&self.namespace)?;
        validate_text_field(
            "registrar.hot_attestation_account",
            &self.hot_attestation_account,
        )?;
        let mut unique = BTreeSet::new();
        for signer in &self.cold_signers {
            validate_text_field("registrar.cold_signer", signer)?;
            if !unique.insert(signer.as_str()) {
                return Err("registrar.cold_signers must contain 3 distinct accounts".to_string());
            }
        }
        if self.reassignment_timelock_blocks < IDENTITY_HANDLE_REASSIGN_MIN_TIMELOCK_BLOCKS {
            return Err(format!(
                "registrar.reassignment_timelock_blocks must be >= {IDENTITY_HANDLE_REASSIGN_MIN_TIMELOCK_BLOCKS}"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleReservationV1 {
    pub namespace: String,
    pub handle: String,
    pub commitment_hash: String,
    pub reserved_at_height: u64,
    pub expires_at_height: u64,
}

impl HandleReservationV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_identity_namespace(&self.namespace)?;
        validate_identity_handle(&self.handle)?;
        validate_lower_hex_len(
            "handle_reservation.commitment_hash",
            &self.commitment_hash,
            96,
        )?;
        if self.expires_at_height <= self.reserved_at_height {
            return Err(
                "handle_reservation.expires_at_height must exceed reserved_at_height".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrarCosignatureV1 {
    pub signer_account: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl RegistrarCosignatureV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("registrar_cosig.signer_account", &self.signer_account)?;
        validate_hex_field("registrar_cosig.public_key_hex", &self.public_key_hex)?;
        validate_hex_field("registrar_cosig.signature_hex", &self.signature_hex)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingHandleReassignmentV1 {
    pub reassignment_id: String,
    pub namespace: String,
    pub handle: String,
    pub from_account: String,
    pub to_account: String,
    /// "lost_key" | "abuse_impersonation" | "emergency_compromise"
    pub reason_code: String,
    pub emergency_compromise: bool,
    pub proposed_at_height: u64,
    pub effective_at_height: u64,
    pub approvals: Vec<RegistrarCosignatureV1>,
    /// "pending" | "executed" | "canceled"
    pub state: String,
}

impl PendingHandleReassignmentV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("reassignment.reassignment_id", &self.reassignment_id, 96)?;
        validate_identity_namespace(&self.namespace)?;
        validate_identity_handle(&self.handle)?;
        validate_text_field("reassignment.from_account", &self.from_account)?;
        validate_text_field("reassignment.to_account", &self.to_account)?;
        validate_identity_reassign_reason(&self.reason_code)?;
        if self.approvals.len() < 2 {
            return Err("reassignment requires at least 2 registrar approvals".to_string());
        }
        for approval in &self.approvals {
            approval.validate()?;
        }
        if !matches!(self.state.as_str(), "pending" | "executed" | "canceled") {
            return Err(format!("invalid reassignment state `{}`", self.state));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorIdentityBindingV1 {
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    pub bound_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unbound_at_height: Option<u64>,
}

impl ValidatorIdentityBindingV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("validator_binding.node_id", &self.node_id)?;
        if let Some(account) = &self.account {
            validate_text_field("validator_binding.account", account)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleAttestationV1 {
    pub namespace: String,
    pub handle: String,
    pub account: String,
    pub expires_at_height: u64,
    pub registrar_account: String,
    pub registrar_pubkey_hex: String,
    pub registrar_signature_hex: String,
}

impl HandleAttestationV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_identity_namespace(&self.namespace)?;
        validate_identity_handle(&self.handle)?;
        validate_text_field("handle_attestation.account", &self.account)?;
        validate_text_field(
            "handle_attestation.registrar_account",
            &self.registrar_account,
        )?;
        validate_hex_field(
            "handle_attestation.registrar_pubkey_hex",
            &self.registrar_pubkey_hex,
        )?;
        validate_hex_field(
            "handle_attestation.registrar_signature_hex",
            &self.registrar_signature_hex,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IdentitySetActionV1 {
    Mint {
        nostr_pubkey: String,
        nostr_proof_sig_hex: String,
        derivation_version: u8,
        key_index: u32,
        identity_kind: IdentityKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handle_attestation: Option<HandleAttestationV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reservation_salt_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        relays: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_nft_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pq_receive_address: Option<String>,
    },
    Update {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        relays: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_nft_id: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_key: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pq_receive_address: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handle_attestation: Option<HandleAttestationV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reservation_salt_hex: Option<String>,
    },
    Link {
        kind: IdentityLinkKind,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof_pubkey_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof_sig_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata_hash: Option<String>,
    },
    Unlink {
        link_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        self_unlink_pubkey_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        self_unlink_sig_hex: Option<String>,
    },
    Rotate {
        mode: IdentityRotateMode,
        new_nostr_pubkey: String,
        new_nostr_proof_sig_hex: String,
        derivation_version: u8,
        key_index: u32,
    },
    RotateAccount {
        new_account: String,
        new_account_pubkey_hex: String,
        new_account_sig_hex: String,
    },
    Revoke {
        reason_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySetOperation {
    pub account: String,
    pub payload: IdentitySetActionV1,
}

impl IdentitySetOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("identity_set.account", &self.account)?;
        match &self.payload {
            IdentitySetActionV1::Mint {
                nostr_pubkey,
                nostr_proof_sig_hex,
                derivation_version,
                handle_attestation,
                reservation_salt_hex,
                relays,
                avatar_nft_id,
                message_key,
                pq_receive_address,
                ..
            } => {
                validate_lower_hex_len(
                    "identity_set.nostr_pubkey",
                    nostr_pubkey,
                    IDENTITY_NOSTR_PUBKEY_HEX_LEN,
                )?;
                validate_lower_hex_len(
                    "identity_set.nostr_proof_sig_hex",
                    nostr_proof_sig_hex,
                    IDENTITY_BIP340_SIG_HEX_LEN,
                )?;
                if *derivation_version != 1 && *derivation_version != 2 {
                    return Err("identity_set.derivation_version must be 1 or 2".to_string());
                }
                if let Some(att) = handle_attestation {
                    att.validate()?;
                }
                if let Some(salt) = reservation_salt_hex {
                    validate_hex_field("identity_set.reservation_salt_hex", salt)?;
                }
                validate_identity_relays(relays)?;
                if let Some(nft_id) = avatar_nft_id {
                    validate_lower_hex_len("identity_set.avatar_nft_id", nft_id, 96)?;
                }
                if let Some(mk) = message_key {
                    validate_lower_hex_len(
                        "identity_set.message_key",
                        mk,
                        IDENTITY_XWING_PUBKEY_HEX_LEN,
                    )?;
                }
                if let Some(pq) = pq_receive_address {
                    validate_identity_pq_receive_address(pq)?;
                }
            }
            IdentitySetActionV1::Update {
                relays,
                avatar_nft_id,
                message_key,
                pq_receive_address,
                handle_attestation,
                reservation_salt_hex,
            } => {
                if let Some(relays) = relays {
                    validate_identity_relays(relays)?;
                }
                if let Some(Some(nft_id)) = avatar_nft_id {
                    validate_lower_hex_len("identity_set.avatar_nft_id", nft_id, 96)?;
                }
                if let Some(Some(mk)) = message_key {
                    validate_lower_hex_len(
                        "identity_set.message_key",
                        mk,
                        IDENTITY_XWING_PUBKEY_HEX_LEN,
                    )?;
                }
                if let Some(Some(pq)) = pq_receive_address {
                    validate_identity_pq_receive_address(pq)?;
                }
                if let Some(att) = handle_attestation {
                    att.validate()?;
                }
                if let Some(salt) = reservation_salt_hex {
                    validate_hex_field("identity_set.reservation_salt_hex", salt)?;
                }
            }
            IdentitySetActionV1::Link {
                target,
                proof_pubkey_hex,
                proof_sig_hex,
                metadata_hash,
                ..
            } => {
                validate_text_field("identity_set.link.target", target)?;
                if let Some(pk) = proof_pubkey_hex {
                    validate_hex_field("identity_set.link.proof_pubkey_hex", pk)?;
                }
                if let Some(sig) = proof_sig_hex {
                    validate_hex_field("identity_set.link.proof_sig_hex", sig)?;
                }
                if let Some(mh) = metadata_hash {
                    validate_lower_hex_len("identity_set.link.metadata_hash", mh, 96)?;
                }
            }
            IdentitySetActionV1::Unlink {
                link_id,
                self_unlink_pubkey_hex,
                self_unlink_sig_hex,
            } => {
                validate_lower_hex_len("identity_set.unlink.link_id", link_id, 96)?;
                if let Some(pk) = self_unlink_pubkey_hex {
                    validate_hex_field("identity_set.unlink.self_unlink_pubkey_hex", pk)?;
                }
                if let Some(sig) = self_unlink_sig_hex {
                    validate_hex_field("identity_set.unlink.self_unlink_sig_hex", sig)?;
                }
            }
            IdentitySetActionV1::Rotate {
                new_nostr_pubkey,
                new_nostr_proof_sig_hex,
                derivation_version,
                ..
            } => {
                validate_lower_hex_len(
                    "identity_set.new_nostr_pubkey",
                    new_nostr_pubkey,
                    IDENTITY_NOSTR_PUBKEY_HEX_LEN,
                )?;
                validate_lower_hex_len(
                    "identity_set.new_nostr_proof_sig_hex",
                    new_nostr_proof_sig_hex,
                    IDENTITY_BIP340_SIG_HEX_LEN,
                )?;
                if *derivation_version != 1 && *derivation_version != 2 {
                    return Err("identity_set.derivation_version must be 1 or 2".to_string());
                }
            }
            IdentitySetActionV1::RotateAccount {
                new_account,
                new_account_pubkey_hex,
                new_account_sig_hex,
            } => {
                validate_text_field("identity_set.new_account", new_account)?;
                if new_account == &self.account {
                    return Err("identity_set.new_account must differ from account".to_string());
                }
                validate_hex_field("identity_set.new_account_pubkey_hex", new_account_pubkey_hex)?;
                validate_hex_field("identity_set.new_account_sig_hex", new_account_sig_hex)?;
            }
            IdentitySetActionV1::Revoke { reason_code } => {
                validate_text_field("identity_set.reason_code", reason_code)?;
            }
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRegistrarRegisterOperation {
    pub caller: String,
    pub namespace: String,
    pub hot_attestation_account: String,
    pub cold_signers: [String; 3],
    pub reassignment_timelock_blocks: u64,
}

impl IdentityRegistrarRegisterOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("identity_registrar_register.caller", &self.caller)?;
        let preview = HandleNamespaceRegistrarV1 {
            namespace: self.namespace.clone(),
            hot_attestation_account: self.hot_attestation_account.clone(),
            cold_signers: self.cold_signers.clone(),
            reassignment_timelock_blocks: self.reassignment_timelock_blocks,
            active: true,
            registered_at_height: 0,
        };
        preview.validate()
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReserveOperation {
    pub registrar_account: String,
    pub namespace: String,
    pub handle: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
}

impl IdentityHandleReserveOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field(
            "identity_handle_reserve.registrar_account",
            &self.registrar_account,
        )?;
        validate_identity_namespace(&self.namespace)?;
        validate_identity_handle(&self.handle)?;
        validate_lower_hex_len(
            "identity_handle_reserve.commitment_hash",
            &self.commitment_hash,
            96,
        )?;
        if self.expires_at_height == 0 {
            return Err("identity_handle_reserve.expires_at_height must be nonzero".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReassignProposeOperation {
    pub proposer: String,
    pub namespace: String,
    pub handle: String,
    pub from_account: String,
    pub to_account: String,
    pub reason_code: String,
    pub emergency_compromise: bool,
    pub approvals: Vec<RegistrarCosignatureV1>,
}

impl IdentityHandleReassignProposeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("identity_handle_reassign_propose.proposer", &self.proposer)?;
        validate_identity_namespace(&self.namespace)?;
        validate_identity_handle(&self.handle)?;
        validate_text_field(
            "identity_handle_reassign_propose.from_account",
            &self.from_account,
        )?;
        validate_text_field(
            "identity_handle_reassign_propose.to_account",
            &self.to_account,
        )?;
        if self.from_account == self.to_account {
            return Err(
                "identity_handle_reassign_propose.to_account must differ from from_account"
                    .to_string(),
            );
        }
        validate_identity_reassign_reason(&self.reason_code)?;
        for approval in &self.approvals {
            approval.validate()?;
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReassignExecuteOperation {
    pub executor: String,
    pub reassignment_id: String,
}

impl IdentityHandleReassignExecuteOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("identity_handle_reassign_execute.executor", &self.executor)?;
        validate_lower_hex_len(
            "identity_handle_reassign_execute.reassignment_id",
            &self.reassignment_id,
            96,
        )?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReassignCancelOperation {
    pub caller: String,
    pub reassignment_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<RegistrarCosignatureV1>,
}

impl IdentityHandleReassignCancelOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("identity_handle_reassign_cancel.caller", &self.caller)?;
        validate_lower_hex_len(
            "identity_handle_reassign_cancel.reassignment_id",
            &self.reassignment_id,
            96,
        )?;
        for approval in &self.approvals {
            approval.validate()?;
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub fn validate_identity_handle(handle: &str) -> Result<(), String> {
    let bytes = handle.as_bytes();
    if bytes.len() < IDENTITY_HANDLE_MIN_LEN || bytes.len() > IDENTITY_HANDLE_MAX_LEN {
        return Err(format!(
            "identity handle must be {IDENTITY_HANDLE_MIN_LEN}..={IDENTITY_HANDLE_MAX_LEN} ASCII bytes"
        ));
    }
    if !bytes.iter().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
    }) {
        return Err(
            "identity handle must contain only lowercase ASCII [a-z0-9._-]".to_string(),
        );
    }
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    if !(first.is_ascii_lowercase() || first.is_ascii_digit())
        || !(last.is_ascii_lowercase() || last.is_ascii_digit())
    {
        return Err("identity handle must not start or end with punctuation".to_string());
    }
    Ok(())
}

pub fn validate_identity_namespace(namespace: &str) -> Result<(), String> {
    let bytes = namespace.as_bytes();
    if bytes.is_empty() || bytes.len() > IDENTITY_NAMESPACE_MAX_LEN {
        return Err(format!(
            "identity namespace must be 1..={IDENTITY_NAMESPACE_MAX_LEN} ASCII bytes"
        ));
    }
    if !bytes.iter().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-')
    }) || bytes[0] == b'.'
        || bytes[bytes.len() - 1] == b'.'
    {
        return Err("identity namespace must be a canonical lowercase DNS domain".to_string());
    }
    Ok(())
}

pub fn validate_identity_relays(relays: &[String]) -> Result<(), String> {
    if relays.len() > IDENTITY_MAX_RELAYS {
        return Err(format!(
            "identity relays exceeds maximum of {IDENTITY_MAX_RELAYS}"
        ));
    }
    for relay in relays {
        if relay.len() > IDENTITY_MAX_RELAY_URL_BYTES
            || !relay.starts_with("wss://")
            || relay.len() <= "wss://".len()
            || !relay.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(format!(
                "identity relay must be a valid wss:// URL <= {IDENTITY_MAX_RELAY_URL_BYTES} bytes"
            ));
        }
    }
    Ok(())
}

pub fn validate_identity_pq_receive_address(address: &str) -> Result<(), String> {
    if !address.starts_with("pfpq1")
        || address.len() <= "pfpq1".len()
        || address.len() > 128
        || !address.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(
            "identity pq_receive_address must start with `pfpq1` and be <= 128 ASCII bytes"
                .to_string(),
        );
    }
    Ok(())
}

pub fn validate_identity_reassign_reason(reason: &str) -> Result<(), String> {
    match reason {
        "lost_key" | "abuse_impersonation" | "emergency_compromise" => Ok(()),
        _ => Err(format!("invalid handle reassignment reason_code `{reason}`")),
    }
}

pub fn identity_handle_key(namespace: &str, handle: &str) -> String {
    format!("{namespace}\0{handle}")
}

pub fn identity_link_id(account: &str, kind: IdentityLinkKind, target: &str, seq: u64) -> String {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(account.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(kind.as_str().as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(target.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(&seq.to_be_bytes());
    hash_hex_domain(IDENTITY_LINK_ID_DOMAIN, &preimage)
}

pub fn identity_handle_reservation_commitment(
    handle: &str,
    nostr_pubkey_hex: &str,
    salt_bytes: &[u8],
) -> String {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(handle.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(nostr_pubkey_hex.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(salt_bytes);
    hash_hex_domain(IDENTITY_HANDLE_RESERVE_DOMAIN, &preimage)
}

pub fn identity_reassignment_id(
    namespace: &str,
    handle: &str,
    from_account: &str,
    to_account: &str,
    proposed_at_height: u64,
    tx_sequence: u64,
) -> String {
    let preimage = format!(
        "namespace={namespace}\nhandle={handle}\nfrom_account={from_account}\nto_account={to_account}\nproposed_at_height={proposed_at_height}\ntx_sequence={tx_sequence}\n"
    );
    hash_hex_domain(IDENTITY_HANDLE_REASSIGN_DOMAIN, preimage.as_bytes())
}

pub fn identity_nostr_proof_message(
    op: &str,
    chain_id: &str,
    account: &str,
    tx_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_NOSTR_PROOF_DOMAIN}\nop={op}\nchain_id={chain_id}\naccount={account}\nsequence={tx_sequence}\n"
    )
    .into_bytes()
}

pub fn identity_validator_link_message(
    chain_id: &str,
    account: &str,
    node_id: &str,
    tx_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_VALIDATOR_LINK_DOMAIN}\nchain_id={chain_id}\naccount={account}\nnode_id={node_id}\nsequence={tx_sequence}\n"
    )
    .into_bytes()
}

pub fn identity_old_chain_link_message(
    chain_id: &str,
    account: &str,
    old_chain_account: &str,
    tx_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_OLD_CHAIN_LINK_DOMAIN}\nchain_id={chain_id}\naccount={account}\nold_chain_account={old_chain_account}\nsequence={tx_sequence}\n"
    )
    .into_bytes()
}

pub fn identity_controller_link_message(
    chain_id: &str,
    account: &str,
    controller_account: &str,
    tx_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_CONTROLLER_LINK_DOMAIN}\nchain_id={chain_id}\naccount={account}\ncontroller_account={controller_account}\nsequence={tx_sequence}\n"
    )
    .into_bytes()
}

pub fn identity_unlink_self_message(chain_id: &str, link_id: &str) -> Vec<u8> {
    format!("{IDENTITY_UNLINK_SELF_DOMAIN}\nchain_id={chain_id}\nlink_id={link_id}\n").into_bytes()
}

pub fn identity_rotate_account_message(
    chain_id: &str,
    old_account: &str,
    new_account: &str,
    identity_sequence: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_ROTATE_ACCOUNT_DOMAIN}\nchain_id={chain_id}\nold_account={old_account}\nnew_account={new_account}\nidentity_sequence={identity_sequence}\n"
    )
    .into_bytes()
}

pub fn identity_handle_attestation_message(
    chain_id: &str,
    namespace: &str,
    handle: &str,
    account: &str,
    expires_at_height: u64,
) -> Vec<u8> {
    format!(
        "{IDENTITY_HANDLE_ATTESTATION_DOMAIN}\nchain_id={chain_id}\nnamespace={namespace}\nhandle={handle}\naccount={account}\nexpires_at_height={expires_at_height}\n"
    )
    .into_bytes()
}

pub fn identity_handle_reassign_message(
    chain_id: &str,
    namespace: &str,
    handle: &str,
    from_account: &str,
    to_account: &str,
    reason_code: &str,
    emergency_compromise: bool,
) -> Vec<u8> {
    format!(
        "{IDENTITY_HANDLE_REASSIGN_DOMAIN}\nchain_id={chain_id}\nnamespace={namespace}\nhandle={handle}\nfrom_account={from_account}\nto_account={to_account}\nreason_code={reason_code}\nemergency_compromise={emergency_compromise}\n"
    )
    .into_bytes()
}

pub fn identity_handle_reassign_cancel_message(
    chain_id: &str,
    reassignment_id: &str,
) -> Vec<u8> {
    format!(
        "{IDENTITY_HANDLE_REASSIGN_DOMAIN}/cancel\nchain_id={chain_id}\nreassignment_id={reassignment_id}\n"
    )
    .into_bytes()
}
