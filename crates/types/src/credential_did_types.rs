#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerSigningAccountV1 {
    pub account: String,
    pub added_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_height: Option<u64>,
    /// Credentials signed by this account with `issued_height <= compromise_cutoff_height`
    /// are treated as invalid immediately by `asset_gate(...)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compromise_cutoff_height: Option<u64>,
}

impl IssuerSigningAccountV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("issuer_signing_account.account", &self.account)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialIssuerV1 {
    /// 96 lowercase hex = SHA3-384(CREDENTIAL_ISSUER_ID_DOMAIN || chain_id || admin || nonce).
    pub issuer_id: String,
    pub admin_account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_admin_account: Option<String>,
    /// Verified off-chain by Explorer & W3C DID resolvers (NEVER DNS inside consensus).
    pub domain: String,
    pub signing_accounts: Vec<IssuerSigningAccountV1>,
    /// Namespaced `<domain>/<type>` strings (e.g. `"vendor.example/kyc.basic"`).
    pub allowed_credential_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendor_dids: Vec<String>,
    /// "active" | "deregistered"
    pub status: String,
    pub created_at_height: u64,
    pub updated_at_height: u64,
}

impl CredentialIssuerV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("credential_issuer.issuer_id", &self.issuer_id, 96)?;
        validate_text_field("credential_issuer.admin_account", &self.admin_account)?;
        if let Some(pending) = &self.pending_admin_account {
            validate_text_field("credential_issuer.pending_admin_account", pending)?;
        }
        validate_issuer_domain(&self.domain)?;
        if self.signing_accounts.is_empty()
            || self.signing_accounts.len() > MAX_ISSUER_SIGNING_ACCOUNTS
        {
            return Err(format!(
                "credential_issuer.signing_accounts must have 1..={MAX_ISSUER_SIGNING_ACCOUNTS} entries"
            ));
        }
        let mut seen_signers = BTreeSet::new();
        for signer in &self.signing_accounts {
            signer.validate()?;
            if !seen_signers.insert(signer.account.as_str()) {
                return Err("duplicate signing_account in credential_issuer".to_string());
            }
        }
        if self.allowed_credential_types.len() > MAX_ISSUER_CREDENTIAL_TYPES {
            return Err(format!(
                "credential_issuer.allowed_credential_types exceeds {MAX_ISSUER_CREDENTIAL_TYPES}"
            ));
        }
        for cred_type in &self.allowed_credential_types {
            validate_namespaced_credential_type(&self.domain, cred_type)?;
        }
        if self.vendor_dids.len() > MAX_ISSUER_VENDOR_DIDS {
            return Err(format!(
                "credential_issuer.vendor_dids exceeds {MAX_ISSUER_VENDOR_DIDS}"
            ));
        }
        for did in &self.vendor_dids {
            validate_text_field("credential_issuer.vendor_did", did)?;
        }
        if !matches!(self.status.as_str(), "active" | "deregistered") {
            return Err(format!("invalid credential_issuer status `{}`", self.status));
        }
        Ok(())
    }

    pub fn has_compromised_signing_account(&self) -> bool {
        self.signing_accounts
            .iter()
            .any(|s| s.compromise_cutoff_height.is_some())
    }

    pub fn is_active_signing_account(&self, account: &str) -> bool {
        self.signing_accounts
            .iter()
            .any(|s| s.account == account && s.revoked_at_height.is_none())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRecordV1 {
    /// 96 lowercase hex = SHA3-384(CREDENTIAL_ID_DOMAIN || subject || issuer_id || credential_type).
    pub credential_id: String,
    pub subject: String,
    pub issuer_id: String,
    pub credential_type: String,
    pub signing_account: String,
    /// 96 lowercase hex salted hash commitment — ZERO PII stored on chain.
    pub commitment_hash: String,
    pub issued_height: u64,
    pub expires_at_height: u64,
    /// "pending" | "accepted" | "revoked"
    pub status: String,
    pub deposit_paid: u64,
    pub deposit_payer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_at_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_height: Option<u64>,
}

impl CredentialRecordV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("credential.credential_id", &self.credential_id, 96)?;
        validate_text_field("credential.subject", &self.subject)?;
        validate_lower_hex_len("credential.issuer_id", &self.issuer_id, 96)?;
        validate_text_field("credential.credential_type", &self.credential_type)?;
        validate_text_field("credential.signing_account", &self.signing_account)?;
        validate_lower_hex_len("credential.commitment_hash", &self.commitment_hash, 96)?;
        validate_text_field("credential.deposit_payer", &self.deposit_payer)?;
        if !matches!(self.status.as_str(), "pending" | "accepted" | "revoked") {
            return Err(format!("invalid credential status `{}`", self.status));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetApprovalMode {
    Either,
    Both,
}

impl AssetApprovalMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Either => "either",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetLapsedExitRule {
    ReturnOnly,
    StrictFreezeClawback,
}

impl AssetLapsedExitRule {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReturnOnly => "return_only",
            Self::StrictFreezeClawback => "strict_freeze_clawback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetWrapperMode {
    PftlOnly,
    PftlPlusEthereumFreeWrapper,
}

impl AssetWrapperMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PftlOnly => "pftl_only",
            Self::PftlPlusEthereumFreeWrapper => "pftl_plus_ethereum_free_wrapper",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRequirementPairV1 {
    pub issuer_id: String,
    pub credential_type: String,
}

impl CredentialRequirementPairV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("credential_pair.issuer_id", &self.issuer_id, 96)?;
        validate_text_field("credential_pair.credential_type", &self.credential_type)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPolicyRuleV1 {
    /// AND-list of required (issuer_id, credential_type) pairs (1..=4).
    pub all_of: Vec<CredentialRequirementPairV1>,
}

impl CredentialPolicyRuleV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.all_of.is_empty() || self.all_of.len() > MAX_CREDENTIAL_POLICY_AND_PAIRS {
            return Err(format!(
                "credential_policy rule all_of must contain 1..={MAX_CREDENTIAL_POLICY_AND_PAIRS} pairs"
            ));
        }
        for pair in &self.all_of {
            pair.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPolicyV1 {
    /// OR-list of AND-rules (0..=8; empty means manual-approval only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub any_of: Vec<CredentialPolicyRuleV1>,
}

impl CredentialPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.any_of.len() > MAX_CREDENTIAL_POLICY_OR_CLAUSES {
            return Err(format!(
                "credential_policy.any_of exceeds {MAX_CREDENTIAL_POLICY_OR_CLAUSES}"
            ));
        }
        for rule in &self.any_of {
            rule.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAssetAccessPolicyV1 {
    pub approval_mode: AssetApprovalMode,
    pub lapsed_exit_rule: AssetLapsedExitRule,
    pub wrapper_mode: AssetWrapperMode,
    pub credential_policy: CredentialPolicyV1,
    pub proposed_at_height: u64,
    pub effective_at_height: u64,
}

impl PendingAssetAccessPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        self.credential_policy.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAccessPolicyV1 {
    pub approval_mode: AssetApprovalMode,
    pub lapsed_exit_rule: AssetLapsedExitRule,
    pub wrapper_mode: AssetWrapperMode,
    pub credential_policy: CredentialPolicyV1,
    pub notice_blocks: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_policy: Option<PendingAssetAccessPolicyV1>,
}

impl AssetAccessPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        self.credential_policy.validate()?;
        if self.notice_blocks < ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS {
            return Err(format!(
                "asset_access_policy.notice_blocks must be >= {ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS}"
            ));
        }
        if let Some(pending) = &self.pending_policy {
            pending.validate()?;
        }
        Ok(())
    }

    pub fn effective_at(
        &self,
        block_height: u64,
    ) -> (
        AssetApprovalMode,
        AssetLapsedExitRule,
        AssetWrapperMode,
        &CredentialPolicyV1,
    ) {
        if let Some(pending) = &self.pending_policy {
            if block_height >= pending.effective_at_height {
                return (
                    pending.approval_mode,
                    pending.lapsed_exit_rule,
                    pending.wrapper_mode,
                    &pending.credential_policy,
                );
            }
        }
        (
            self.approval_mode,
            self.lapsed_exit_rule,
            self.wrapper_mode,
            &self.credential_policy,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerRegisterOperation {
    pub admin_account: String,
    pub domain: String,
    pub signing_accounts: Vec<String>,
    pub allowed_credential_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendor_dids: Vec<String>,
    pub registration_nonce: u64,
}

impl IssuerRegisterOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("issuer_register.admin_account", &self.admin_account)?;
        validate_issuer_domain(&self.domain)?;
        for acct in &self.signing_accounts {
            validate_text_field("issuer_register.signing_account", acct)?;
        }
        for ct in &self.allowed_credential_types {
            validate_text_field("issuer_register.allowed_credential_type", ct)?;
        }
        for did in &self.vendor_dids {
            validate_text_field("issuer_register.vendor_did", did)?;
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerUpdateOperation {
    pub admin_account: String,
    pub issuer_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_signing_accounts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revoke_signing_accounts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compromise_cutoffs: Vec<(String, u64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_credential_types: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_dids: Option<Vec<String>>,
}

impl IssuerUpdateOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("issuer_update.admin_account", &self.admin_account)?;
        validate_lower_hex_len("issuer_update.issuer_id", &self.issuer_id, 96)?;
        if let Some(domain) = &self.domain {
            validate_issuer_domain(domain)?;
        }
        for acct in &self.add_signing_accounts {
            validate_text_field("issuer_update.add_signing_account", acct)?;
        }
        for acct in &self.revoke_signing_accounts {
            validate_text_field("issuer_update.revoke_signing_account", acct)?;
        }
        for (acct, _) in &self.compromise_cutoffs {
            validate_text_field("issuer_update.compromise_cutoff_account", acct)?;
        }
        if let Some(types) = &self.allowed_credential_types {
            for ct in types {
                validate_text_field("issuer_update.allowed_credential_type", ct)?;
            }
        }
        if let Some(dids) = &self.vendor_dids {
            for did in dids {
                validate_text_field("issuer_update.vendor_did", did)?;
            }
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuerAdminTransferAction {
    Propose,
    Accept,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerAdminTransferOperation {
    pub caller: String,
    pub issuer_id: String,
    pub action: IssuerAdminTransferAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_admin_account: Option<String>,
}

impl IssuerAdminTransferOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("issuer_admin_transfer.caller", &self.caller)?;
        validate_lower_hex_len("issuer_admin_transfer.issuer_id", &self.issuer_id, 96)?;
        if self.action == IssuerAdminTransferAction::Propose {
            let Some(new_admin) = &self.new_admin_account else {
                return Err(
                    "issuer_admin_transfer.new_admin_account is required for propose".to_string(),
                );
            };
            validate_text_field("issuer_admin_transfer.new_admin_account", new_admin)?;
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerDeregisterOperation {
    pub admin_account: String,
    pub issuer_id: String,
    pub reason_code: String,
}

impl IssuerDeregisterOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("issuer_deregister.admin_account", &self.admin_account)?;
        validate_lower_hex_len("issuer_deregister.issuer_id", &self.issuer_id, 96)?;
        validate_text_field("issuer_deregister.reason_code", &self.reason_code)?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialCreateOperation {
    pub signing_account: String,
    pub issuer_id: String,
    pub subject: String,
    pub credential_type: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
}

impl CredentialCreateOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("credential_create.signing_account", &self.signing_account)?;
        validate_lower_hex_len("credential_create.issuer_id", &self.issuer_id, 96)?;
        validate_text_field("credential_create.subject", &self.subject)?;
        validate_text_field("credential_create.credential_type", &self.credential_type)?;
        validate_lower_hex_len(
            "credential_create.commitment_hash",
            &self.commitment_hash,
            96,
        )?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAcceptOperation {
    pub subject: String,
    pub credential_id: String,
}

impl CredentialAcceptOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("credential_accept.subject", &self.subject)?;
        validate_lower_hex_len("credential_accept.credential_id", &self.credential_id, 96)?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialCreateAndAcceptOperation {
    pub signing_account: String,
    pub issuer_id: String,
    pub subject: String,
    pub credential_type: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
    pub subject_pubkey_hex: String,
    pub subject_accept_signature_hex: String,
    pub subject_accept_nonce: u64,
}

impl CredentialCreateAndAcceptOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field(
            "credential_create_and_accept.signing_account",
            &self.signing_account,
        )?;
        validate_lower_hex_len("credential_create_and_accept.issuer_id", &self.issuer_id, 96)?;
        validate_text_field("credential_create_and_accept.subject", &self.subject)?;
        validate_text_field(
            "credential_create_and_accept.credential_type",
            &self.credential_type,
        )?;
        validate_lower_hex_len(
            "credential_create_and_accept.commitment_hash",
            &self.commitment_hash,
            96,
        )?;
        validate_hex_field(
            "credential_create_and_accept.subject_pubkey_hex",
            &self.subject_pubkey_hex,
        )?;
        validate_hex_field(
            "credential_create_and_accept.subject_accept_signature_hex",
            &self.subject_accept_signature_hex,
        )?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRevokeOperation {
    pub caller: String,
    pub credential_id: String,
    pub reason_code: String,
}

impl CredentialRevokeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("credential_revoke.caller", &self.caller)?;
        validate_lower_hex_len("credential_revoke.credential_id", &self.credential_id, 96)?;
        validate_text_field("credential_revoke.reason_code", &self.reason_code)?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialDeleteOperation {
    pub caller: String,
    pub credential_id: String,
}

impl CredentialDeleteOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("credential_delete.caller", &self.caller)?;
        validate_lower_hex_len("credential_delete.credential_id", &self.credential_id, 96)?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAccessSetOperation {
    pub issuer: String,
    pub asset_id: String,
    #[serde(default)]
    pub disable_authorization_permanently: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<AssetApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lapsed_exit_rule: Option<AssetLapsedExitRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapper_mode: Option<AssetWrapperMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_policy: Option<CredentialPolicyV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice_blocks: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emergency_remove_issuer_id: Option<String>,
}

impl AssetAccessSetOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("asset_access_set.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "asset_access_set.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if let Some(issuer_id) = &self.emergency_remove_issuer_id {
            validate_lower_hex_len(
                "asset_access_set.emergency_remove_issuer_id",
                issuer_id,
                96,
            )?;
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub fn validate_issuer_domain(domain: &str) -> Result<(), String> {
    let bytes = domain.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        return Err("issuer domain must be 1..=128 ASCII bytes".to_string());
    }
    if !bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
        || bytes[0] == b'.'
        || bytes[bytes.len() - 1] == b'.'
    {
        return Err("issuer domain must be a canonical lowercase DNS domain".to_string());
    }
    Ok(())
}

pub fn validate_namespaced_credential_type(domain: &str, cred_type: &str) -> Result<(), String> {
    let prefix = format!("{domain}/");
    let Some(suffix) = cred_type.strip_prefix(&prefix) else {
        return Err(format!(
            "credential_type `{cred_type}` must be prefixed by `{prefix}`"
        ));
    };
    if suffix.is_empty()
        || cred_type.len() > 192
        || !suffix.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
    {
        return Err(format!(
            "credential_type `{cred_type}` has invalid namespaced suffix"
        ));
    }
    Ok(())
}

pub fn credential_issuer_id(chain_id: &str, admin_account: &str, registration_nonce: u64) -> String {
    let preimage =
        format!("chain_id={chain_id}\nadmin={admin_account}\nnonce={registration_nonce}\n");
    hash_hex_domain(CREDENTIAL_ISSUER_ID_DOMAIN, preimage.as_bytes())
}

pub fn credential_id(subject: &str, issuer_id: &str, credential_type: &str) -> String {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(subject.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(issuer_id.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(credential_type.as_bytes());
    hash_hex_domain(CREDENTIAL_ID_DOMAIN, &preimage)
}

pub fn credential_delegated_accept_preimage(
    chain_id: &str,
    genesis_hash: &str,
    subject: &str,
    issuer_id: &str,
    credential_type: &str,
    commitment_hash: &str,
    expires_at_height: u64,
    subject_accept_nonce: u64,
) -> Vec<u8> {
    format!(
        "{CREDENTIAL_DELEGATED_ACCEPT_DOMAIN}\nchain_id={chain_id}\ngenesis_hash={genesis_hash}\nsubject={subject}\nissuer_id={issuer_id}\ncredential_type={credential_type}\ncommitment_hash={commitment_hash}\nexpires_at_height={expires_at_height}\nsubject_accept_nonce={subject_accept_nonce}\n"
    )
    .into_bytes()
}
