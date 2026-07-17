#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedTransfer {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address_namespace: String,
    pub transaction_kind: String,
    pub signature_algorithm_id: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub sequence: u64,
}

impl UnsignedTransfer {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("transfer.chain_id", &self.chain_id)?;
        validate_lower_hex_len("transfer.genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err("transfer protocol_version must be nonzero".to_string());
        }
        validate_text_field("transfer.address_namespace", &self.address_namespace)?;
        validate_text_field("transfer.transaction_kind", &self.transaction_kind)?;
        validate_text_field(
            "transfer.signature_algorithm_id",
            &self.signature_algorithm_id,
        )?;
        validate_text_field("transfer.from", &self.from)?;
        validate_text_field("transfer.to", &self.to)?;
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "postfiat.transfer.v1\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id={}\nfrom={}\nto={}\namount={}\nfee={}\nsequence={}\n",
            self.chain_id,
            self.genesis_hash,
            self.protocol_version,
            self.address_namespace,
            self.transaction_kind,
            self.signature_algorithm_id,
            self.from,
            self.to,
            self.amount,
            self.fee,
            self.sequence
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedTransfer {
    pub unsigned: UnsignedTransfer,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl SignedTransfer {
    pub fn validate(&self) -> Result<(), String> {
        self.unsigned.validate()?;
        validate_text_field("transfer.algorithm_id", &self.algorithm_id)?;
        validate_lower_hex_max(
            "transfer.public_key_hex",
            &self.public_key_hex,
            MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
        )?;
        validate_lower_hex_max(
            "transfer.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentMemo {
    #[serde(default)]
    pub memo_type: String,
    #[serde(default)]
    pub memo_format: String,
    #[serde(default)]
    pub memo_data: String,
}

impl PaymentMemo {
    pub fn byte_len(&self) -> usize {
        hex_encoded_byte_len(&self.memo_type)
            .saturating_add(hex_encoded_byte_len(&self.memo_format))
            .saturating_add(hex_encoded_byte_len(&self.memo_data))
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_optional_lower_hex_max(
            "payment.memo_type",
            &self.memo_type,
            MAX_PAYMENT_MEMO_TYPE_BYTES,
        )?;
        validate_optional_lower_hex_max(
            "payment.memo_format",
            &self.memo_format,
            MAX_PAYMENT_MEMO_FORMAT_BYTES,
        )?;
        validate_optional_lower_hex_max(
            "payment.memo_data",
            &self.memo_data,
            MAX_PAYMENT_MEMO_DATA_BYTES,
        )?;
        if self.memo_type.is_empty() && self.memo_format.is_empty() && self.memo_data.is_empty() {
            return Err("payment memo must contain at least one field".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedPaymentV2 {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address_namespace: String,
    pub transaction_kind: String,
    pub signature_algorithm_id: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub sequence: u64,
    #[serde(default)]
    pub memos: Vec<PaymentMemo>,
}

impl UnsignedPaymentV2 {
    pub fn memo_bytes(&self) -> usize {
        self.memos
            .iter()
            .map(PaymentMemo::byte_len)
            .fold(0usize, usize::saturating_add)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("payment.chain_id", &self.chain_id)?;
        validate_lower_hex_len("payment.genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err("payment protocol_version must be nonzero".to_string());
        }
        validate_text_field("payment.address_namespace", &self.address_namespace)?;
        validate_text_field("payment.transaction_kind", &self.transaction_kind)?;
        if self.transaction_kind != PAYMENT_V2_TRANSACTION_KIND {
            return Err(format!(
                "payment transaction_kind must be `{PAYMENT_V2_TRANSACTION_KIND}`"
            ));
        }
        validate_text_field(
            "payment.signature_algorithm_id",
            &self.signature_algorithm_id,
        )?;
        validate_text_field("payment.from", &self.from)?;
        validate_text_field("payment.to", &self.to)?;
        if self.amount == 0 {
            return Err("payment amount must be nonzero".to_string());
        }
        if self.memos.len() > MAX_PAYMENT_MEMOS {
            return Err(format!(
                "payment memos must not exceed {MAX_PAYMENT_MEMOS} entries"
            ));
        }
        for memo in &self.memos {
            memo.validate()?;
        }
        let memo_bytes = self.memo_bytes();
        if memo_bytes > MAX_PAYMENT_MEMO_TOTAL_BYTES {
            return Err(format!(
                "payment memo bytes must not exceed {MAX_PAYMENT_MEMO_TOTAL_BYTES}"
            ));
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "postfiat.payment.v2\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id={}\nfrom={}\nto={}\namount={}\nfee={}\nsequence={}\nmemo_count={}\n",
            self.chain_id,
            self.genesis_hash,
            self.protocol_version,
            self.address_namespace,
            self.transaction_kind,
            self.signature_algorithm_id,
            self.from,
            self.to,
            self.amount,
            self.fee,
            self.sequence,
            self.memos.len()
        )
        .into_bytes();
        for (index, memo) in self.memos.iter().enumerate() {
            out.extend_from_slice(
                format!(
                    "memo[{index}].type_bytes={}\nmemo[{index}].type={}\nmemo[{index}].format_bytes={}\nmemo[{index}].format={}\nmemo[{index}].data_bytes={}\nmemo[{index}].data={}\n",
                    hex_encoded_byte_len(&memo.memo_type),
                    memo.memo_type,
                    hex_encoded_byte_len(&memo.memo_format),
                    memo.memo_format,
                    hex_encoded_byte_len(&memo.memo_data),
                    memo.memo_data
                )
                .as_bytes(),
            );
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedPaymentV2 {
    pub unsigned: UnsignedPaymentV2,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl SignedPaymentV2 {
    pub fn validate(&self) -> Result<(), String> {
        self.unsigned.validate()?;
        validate_text_field("payment.algorithm_id", &self.algorithm_id)?;
        validate_lower_hex_max(
            "payment.public_key_hex",
            &self.public_key_hex,
            MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
        )?;
        validate_lower_hex_max(
            "payment.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )?;
        Ok(())
    }

    pub fn tx_id_preimage_bytes(&self) -> Vec<u8> {
        let mut bytes = self.unsigned.signing_bytes();
        bytes.extend_from_slice(b"algorithm=");
        bytes.extend_from_slice(self.algorithm_id.as_bytes());
        bytes.extend_from_slice(b"\npublic_key=");
        bytes.extend_from_slice(self.public_key_hex.as_bytes());
        bytes.extend_from_slice(b"\nsignature=");
        bytes.extend_from_slice(self.signature_hex.as_bytes());
        bytes.extend_from_slice(b"\n");
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetCreateOperation {
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

impl AssetCreateOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("asset_create.issuer", &self.issuer)?;
        validate_issued_asset_code(&self.code)?;
        if self.version == 0 {
            return Err("asset_create.version must be nonzero".to_string());
        }
        if self.precision > MAX_ISSUED_ASSET_PRECISION {
            return Err(format!(
                "asset_create.precision must not exceed {MAX_ISSUED_ASSET_PRECISION}"
            ));
        }
        validate_optional_text_field(
            "asset_create.display_name",
            &self.display_name,
            MAX_ISSUED_ASSET_DISPLAY_NAME_BYTES,
        )?;
        if self.max_supply == Some(0) {
            return Err("asset_create.max_supply must be nonzero when present".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "issuer={}\ncode_bytes={}\ncode={}\nversion={}\nprecision={}\ndisplay_name_bytes={}\ndisplay_name={}\nmax_supply_present={}\nmax_supply={}\nrequires_authorization={}\nfreeze_enabled={}\nclawback_enabled={}\n",
            self.issuer,
            self.code.len(),
            self.code,
            self.version,
            self.precision,
            self.display_name.len(),
            self.display_name,
            self.max_supply.is_some(),
            self.max_supply.unwrap_or_default(),
            self.requires_authorization,
            self.freeze_enabled,
            self.clawback_enabled
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustSetOperation {
    pub account: String,
    pub issuer: String,
    pub asset_id: String,
    pub limit: u64,
    #[serde(default)]
    pub authorized: bool,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub reserve_paid: u64,
}

impl TrustSetOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("trust_set.account", &self.account)?;
        validate_text_field("trust_set.issuer", &self.issuer)?;
        if self.account == self.issuer {
            return Err("trust_set.account must differ from trust_set.issuer".to_string());
        }
        validate_lower_hex_len(
            "trust_set.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.limit == 0 {
            return Err("trust_set.limit must be nonzero".to_string());
        }
        trustline_id(&self.account, &self.issuer, &self.asset_id)?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "account={}\nissuer={}\nasset_id={}\nlimit={}\nauthorized={}\nfrozen={}\nreserve_paid={}\n",
            self.account,
            self.issuer,
            self.asset_id,
            self.limit,
            self.authorized,
            self.frozen,
            self.reserve_paid
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuedPaymentOperation {
    pub from: String,
    pub to: String,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
}

impl IssuedPaymentOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("issued_payment.from", &self.from)?;
        validate_text_field("issued_payment.to", &self.to)?;
        validate_text_field("issued_payment.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "issued_payment.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err("issued_payment.amount must be nonzero".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "from={}\nto={}\nissuer={}\nasset_id={}\namount={}\n",
            self.from, self.to, self.issuer, self.asset_id, self.amount
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBurnOperation {
    pub owner: String,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
}

impl AssetBurnOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("asset_burn.owner", &self.owner)?;
        validate_text_field("asset_burn.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "asset_burn.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err("asset_burn.amount must be nonzero".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "owner={}\nissuer={}\nasset_id={}\namount={}\n",
            self.owner, self.issuer, self.asset_id, self.amount
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetClawbackOperation {
    pub owner: String,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
}

impl AssetClawbackOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("asset_clawback.owner", &self.owner)?;
        validate_text_field("asset_clawback.issuer", &self.issuer)?;
        if self.owner == self.issuer {
            return Err("asset_clawback.owner must differ from asset_clawback.issuer".to_string());
        }
        validate_lower_hex_len(
            "asset_clawback.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err("asset_clawback.amount must be nonzero".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "owner={}\nissuer={}\nasset_id={}\namount={}\n",
            self.owner, self.issuer, self.asset_id, self.amount
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavAssetRegisterOperation {
    pub issuer: String,
    pub asset_id: String,
    pub reserve_operator: String,
    pub proof_profile: String,
    pub valuation_unit: String,
    pub redemption_account: String,
}

impl NavAssetRegisterOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_asset_register.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "nav_asset_register.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_text_field(
            "nav_asset_register.reserve_operator",
            &self.reserve_operator,
        )?;
        validate_text_field("nav_asset_register.proof_profile", &self.proof_profile)?;
        validate_text_field("nav_asset_register.valuation_unit", &self.valuation_unit)?;
        validate_text_field(
            "nav_asset_register.redemption_account",
            &self.redemption_account,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "issuer={}\nasset_id={}\nreserve_operator={}\nproof_profile={}\nvaluation_unit={}\nredemption_account={}\n",
            self.issuer,
            self.asset_id,
            self.reserve_operator,
            self.proof_profile,
            self.valuation_unit,
            self.redemption_account
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavReserveSubmitOperation {
    pub issuer: String,
    pub submitter: String,
    pub asset_id: String,
    pub epoch: u64,
    pub nav_per_unit: u64,
    pub circulating_supply: u64,
    pub verified_net_assets: u64,
    pub proof_profile: String,
    pub source_root: String,
    pub attestor_root: String,
    pub reserve_packet_hash: String,
    /// For ledger-transparent profiles: on-ledger accounts whose native
    /// balances back this packet. Consensus verifies their sum equals
    /// verified_net_assets at execution time.
    #[serde(default)]
    pub reserve_accounts: Vec<String>,
    /// Groth16 proof calldata for sp1-groth16 profiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sp1_proof_bytes: Vec<u8>,
    /// SP1 public-values blob committed by the Groth16 proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sp1_public_values: Vec<u8>,
}

impl NavReserveSubmitOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_reserve_submit.issuer", &self.issuer)?;
        validate_text_field("nav_reserve_submit.submitter", &self.submitter)?;
        validate_lower_hex_len(
            "nav_reserve_submit.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.epoch == 0 {
            return Err("nav_reserve_submit.epoch must be nonzero".to_string());
        }
        if self.nav_per_unit == 0 {
            return Err("nav_reserve_submit.nav_per_unit must be nonzero".to_string());
        }
        validate_text_field("nav_reserve_submit.proof_profile", &self.proof_profile)?;
        validate_lower_hex_len("nav_reserve_submit.source_root", &self.source_root, 96)?;
        validate_lower_hex_len("nav_reserve_submit.attestor_root", &self.attestor_root, 96)?;
        validate_lower_hex_len(
            "nav_reserve_submit.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        nav_reserve_packet_id(&self.asset_id, self.epoch, &self.reserve_packet_hash)?;
        if self.reserve_accounts.len() > MAX_NAV_RESERVE_ACCOUNTS {
            return Err(format!(
                "nav_reserve_submit.reserve_accounts exceeds maximum of {MAX_NAV_RESERVE_ACCOUNTS}"
            ));
        }
        for account in &self.reserve_accounts {
            validate_text_field("nav_reserve_submit.reserve_accounts entry", account)?;
        }
        if self.sp1_proof_bytes.len() > DEFAULT_MAX_NAV_SP1_PROOF_BYTES as usize {
            return Err(format!(
                "nav_reserve_submit.sp1_proof_bytes exceeds maximum of {DEFAULT_MAX_NAV_SP1_PROOF_BYTES}"
            ));
        }
        if self.sp1_public_values.len() > DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES as usize {
            return Err(format!(
                "nav_reserve_submit.sp1_public_values exceeds maximum of {DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES}"
            ));
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "issuer={}\nsubmitter={}\nasset_id={}\nepoch={}\nnav_per_unit={}\ncirculating_supply={}\nverified_net_assets={}\nproof_profile={}\nsource_root={}\nattestor_root={}\nreserve_packet_hash={}\nreserve_accounts={}\nsp1_proof_bytes={}\nsp1_public_values={}\n",
            self.issuer,
            self.submitter,
            self.asset_id,
            self.epoch,
            self.nav_per_unit,
            self.circulating_supply,
            self.verified_net_assets,
            self.proof_profile,
            self.source_root,
            self.attestor_root,
            self.reserve_packet_hash,
            self.reserve_accounts.join(","),
            bytes_to_lower_hex(&self.sp1_proof_bytes),
            bytes_to_lower_hex(&self.sp1_public_values),
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavReserveChallengeOperation {
    pub challenger: String,
    pub asset_id: String,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    pub challenge_hash: String,
    /// Bond escrowed by the challenger; must meet the asset profile's
    /// min_challenge_bond. Resolved deterministically at finalization.
    #[serde(default)]
    pub bond: u64,
}

impl NavReserveChallengeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_reserve_challenge.challenger", &self.challenger)?;
        validate_lower_hex_len(
            "nav_reserve_challenge.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.epoch == 0 {
            return Err("nav_reserve_challenge.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "nav_reserve_challenge.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "nav_reserve_challenge.challenge_hash",
            &self.challenge_hash,
            96,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "challenger={}\nasset_id={}\nepoch={}\nreserve_packet_hash={}\nchallenge_hash={}\nbond={}\n",
            self.challenger,
            self.asset_id,
            self.epoch,
            self.reserve_packet_hash,
            self.challenge_hash,
            self.bond
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavEpochFinalizeOperation {
    pub issuer: String,
    pub asset_id: String,
    pub epoch: u64,
    pub reserve_packet_hash: String,
}

impl NavEpochFinalizeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_epoch_finalize.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "nav_epoch_finalize.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.epoch == 0 {
            return Err("nav_epoch_finalize.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "nav_epoch_finalize.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "issuer={}\nasset_id={}\nepoch={}\nreserve_packet_hash={}\n",
            self.issuer, self.asset_id, self.epoch, self.reserve_packet_hash
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsFinalizeOperation {
    pub issuer: String,
    pub asset_id: String,
    pub envelope_hash: String,
    pub envelope: MarketOpsEnvelope,
    pub policy_inputs: MarketOpsPolicyInputs,
}

impl MarketOpsFinalizeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("market_ops_finalize.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "market_ops_finalize.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len("market_ops_finalize.envelope_hash", &self.envelope_hash, 96)?;
        self.envelope.validate_basic()?;
        self.policy_inputs.validate()?;
        if self.envelope.asset_id != market_ops_asset_id(&self.asset_id)? {
            return Err(
                "market_ops_finalize.envelope.asset_id must match operation asset_id".to_string(),
            );
        }
        if self.envelope_hash != bytes_to_lower_hex(&self.envelope.envelope_hash()) {
            return Err(
                "market_ops_finalize.envelope_hash must match envelope.envelope_hash".to_string(),
            );
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "issuer={}\nasset_id={}\nenvelope_hash={}\n",
            self.issuer, self.asset_id, self.envelope_hash
        )
        .into_bytes();
        append_market_ops_envelope_signing_bytes(&mut bytes, &self.envelope);
        append_market_ops_policy_inputs_signing_bytes(&mut bytes, &self.policy_inputs);
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsPolicyRegisterOperation {
    pub issuer: String,
    pub asset_id: String,
    pub policy: MarketOpsPolicyRegistration,
}

impl MarketOpsPolicyRegisterOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("market_ops_policy_register.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "market_ops_policy_register.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        self.policy.validate()?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes =
            format!("issuer={}\nasset_id={}\n", self.issuer, self.asset_id).into_bytes();
        append_market_ops_policy_registration_signing_bytes(&mut bytes, &self.policy);
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavMintAtNavOperation {
    pub issuer: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    #[serde(default)]
    pub settlement_asset_id: String,
    #[serde(default)]
    pub settlement_bucket_id: String,
    #[serde(default)]
    pub settlement_allocation_id: String,
    #[serde(default)]
    pub settlement_amount_atoms: u64,
}

impl NavMintAtNavOperation {
    pub fn has_vault_bridge_settlement(&self) -> bool {
        !self.settlement_asset_id.is_empty()
            || !self.settlement_bucket_id.is_empty()
            || !self.settlement_allocation_id.is_empty()
            || self.settlement_amount_atoms != 0
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_mint_at_nav.issuer", &self.issuer)?;
        validate_text_field("nav_mint_at_nav.to", &self.to)?;
        if self.issuer == self.to {
            return Err("nav_mint_at_nav.to must differ from issuer".to_string());
        }
        validate_lower_hex_len(
            "nav_mint_at_nav.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err("nav_mint_at_nav.amount must be nonzero".to_string());
        }
        if self.epoch == 0 {
            return Err("nav_mint_at_nav.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "nav_mint_at_nav.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        if self.has_vault_bridge_settlement() {
            validate_lower_hex_len(
                "nav_mint_at_nav.settlement_asset_id",
                &self.settlement_asset_id,
                ISSUED_ASSET_ID_HEX_LEN,
            )?;
            if self.settlement_asset_id == self.asset_id {
                return Err(
                    "nav_mint_at_nav.settlement_asset_id must differ from asset_id".to_string(),
                );
            }
            validate_lower_hex_len(
                "nav_mint_at_nav.settlement_bucket_id",
                &self.settlement_bucket_id,
                VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
            )?;
            validate_lower_hex_len(
                "nav_mint_at_nav.settlement_allocation_id",
                &self.settlement_allocation_id,
                VAULT_BRIDGE_ALLOCATION_ID_HEX_LEN,
            )?;
            if self.settlement_amount_atoms == 0 {
                return Err("nav_mint_at_nav.settlement_amount_atoms must be nonzero when settlement is present".to_string());
            }
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "issuer={}\nto={}\nasset_id={}\namount={}\nepoch={}\nreserve_packet_hash={}\n",
            self.issuer, self.to, self.asset_id, self.amount, self.epoch, self.reserve_packet_hash
        )
        .into_bytes();
        if self.has_vault_bridge_settlement() {
            bytes.extend_from_slice(
                format!(
                    "settlement_asset_id={}\nsettlement_bucket_id={}\nsettlement_allocation_id={}\nsettlement_amount_atoms={}\n",
                    self.settlement_asset_id,
                    self.settlement_bucket_id,
                    self.settlement_allocation_id,
                    self.settlement_amount_atoms
                )
                .as_bytes(),
            );
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavRedeemAtNavOperation {
    pub owner: String,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
    pub epoch: u64,
    pub reserve_packet_hash: String,
}

impl NavRedeemAtNavOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_redeem_at_nav.owner", &self.owner)?;
        validate_text_field("nav_redeem_at_nav.issuer", &self.issuer)?;
        if self.owner == self.issuer {
            return Err("nav_redeem_at_nav.owner must differ from issuer".to_string());
        }
        validate_lower_hex_len(
            "nav_redeem_at_nav.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err("nav_redeem_at_nav.amount must be nonzero".to_string());
        }
        if self.epoch == 0 {
            return Err("nav_redeem_at_nav.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "nav_redeem_at_nav.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "owner={}\nissuer={}\nasset_id={}\namount={}\nepoch={}\nreserve_packet_hash={}\n",
            self.owner,
            self.issuer,
            self.asset_id,
            self.amount,
            self.epoch,
            self.reserve_packet_hash
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavHaltOperation {
    pub issuer: String,
    pub asset_id: String,
    pub halted: bool,
    pub reason: String,
}

impl NavHaltOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_halt.issuer", &self.issuer)?;
        validate_lower_hex_len("nav_halt.asset_id", &self.asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
        validate_optional_text_field("nav_halt.reason", &self.reason, 128)?;
        if self.halted && self.reason.is_empty() {
            return Err("nav_halt.reason must be nonempty when halting".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "issuer={}\nasset_id={}\nhalted={}\nreason={}\n",
            self.issuer, self.asset_id, self.halted, self.reason
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavProfileRegisterOperation {
    pub registrant: String,
    pub verifier_kind: String,
    #[serde(default)]
    pub source_class: String,
    #[serde(default)]
    pub max_snapshot_age_blocks: u64,
    #[serde(default)]
    pub challenge_window_blocks: u64,
    #[serde(default)]
    pub max_epoch_gap_blocks: u64,
    #[serde(default)]
    pub settle_deadline_blocks: u64,
    #[serde(default)]
    pub min_challenge_bond: u64,
    #[serde(default)]
    pub min_attestations: u64,
    #[serde(default)]
    pub tolerance_bp: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub bridge_observer_min_confirmations: u64,
    #[serde(default)]
    pub valuation_policy_hash: String,
    /// Exact governed vault-route profile hash. This remains separate from
    /// the verifier-specific valuation policy committed by SP1 public values.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vault_bridge_route_policy_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sp1_program_vkey: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sp1_proof_encoding: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub max_proof_bytes: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub max_public_values_bytes: u64,
}

impl NavProfileRegisterOperation {
    pub fn effective_source_class(&self) -> &str {
        if self.source_class.is_empty() {
            NAV_PROFILE_SOURCE_CLASS_LEDGER
        } else {
            &self.source_class
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_profile_register.registrant", &self.registrant)?;
        let profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
            &self.registrant,
            &self.verifier_kind,
            self.effective_source_class(),
            self.max_snapshot_age_blocks,
            self.challenge_window_blocks,
            self.max_epoch_gap_blocks,
            self.settle_deadline_blocks,
            self.min_challenge_bond,
            self.min_attestations,
            self.tolerance_bp,
            self.bridge_observer_min_confirmations,
            &self.valuation_policy_hash,
            &self.sp1_program_vkey,
            &self.sp1_proof_encoding,
            self.max_proof_bytes,
            self.max_public_values_bytes,
        )?;
        if !self.vault_bridge_route_policy_hash.is_empty() {
            profile.with_vault_bridge_route_policy_hash(
                self.vault_bridge_route_policy_hash.clone(),
            )?;
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "registrant={}\nverifier_kind={}\nsource_class={}\nmax_snapshot_age_blocks={}\nchallenge_window_blocks={}\nmax_epoch_gap_blocks={}\nsettle_deadline_blocks={}\nmin_challenge_bond={}\nmin_attestations={}\ntolerance_bp={}\nvaluation_policy_hash={}\n",
            self.registrant,
            self.verifier_kind,
            self.source_class,
            self.max_snapshot_age_blocks,
            self.challenge_window_blocks,
            self.max_epoch_gap_blocks,
            self.settle_deadline_blocks,
            self.min_challenge_bond,
            self.min_attestations,
            self.tolerance_bp,
            self.valuation_policy_hash,
        );
        if self.bridge_observer_min_confirmations != 0 {
            out.push_str(&format!(
                "bridge_observer_min_confirmations={}\n",
                self.bridge_observer_min_confirmations
            ));
        }
        if !self.vault_bridge_route_policy_hash.is_empty() {
            out.push_str(&format!(
                "vault_bridge_route_policy_hash={}\n",
                self.vault_bridge_route_policy_hash
            ));
        }
        if !self.sp1_program_vkey.is_empty() {
            out.push_str(&format!("sp1_program_vkey={}\n", self.sp1_program_vkey));
        }
        if !self.sp1_proof_encoding.is_empty() {
            out.push_str(&format!("sp1_proof_encoding={}\n", self.sp1_proof_encoding));
        }
        if self.max_proof_bytes != 0 {
            out.push_str(&format!("max_proof_bytes={}\n", self.max_proof_bytes));
        }
        if self.max_public_values_bytes != 0 {
            out.push_str(&format!(
                "max_public_values_bytes={}\n",
                self.max_public_values_bytes
            ));
        }
        out.into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavAttestorRegisterOperation {
    pub attestor: String,
    pub domain: String,
    #[serde(default)]
    pub bond: u64,
}

impl NavAttestorRegisterOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_attestor_register.attestor", &self.attestor)?;
        validate_text_field("nav_attestor_register.domain", &self.domain)?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "attestor={}\ndomain={}\nbond={}\n",
            self.attestor, self.domain, self.bond
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavReserveAttestOperation {
    pub attestor: String,
    pub asset_id: String,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    pub pass: bool,
    pub observation_root: String,
}

impl NavReserveAttestOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_reserve_attest.attestor", &self.attestor)?;
        validate_lower_hex_len(
            "nav_reserve_attest.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.epoch == 0 {
            return Err("nav_reserve_attest.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "nav_reserve_attest.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "nav_reserve_attest.observation_root",
            &self.observation_root,
            96,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "attestor={}\nasset_id={}\nepoch={}\nreserve_packet_hash={}\npass={}\nobservation_root={}\n",
            self.attestor,
            self.asset_id,
            self.epoch,
            self.reserve_packet_hash,
            self.pass,
            self.observation_root
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavRedeemSettleOperation {
    pub issuer: String,
    pub asset_id: String,
    pub redemption_id: String,
    pub settlement_receipt_hash: String,
    #[serde(default)]
    pub settlement_asset_id: String,
    #[serde(default)]
    pub settlement_bucket_id: String,
    #[serde(default)]
    pub settlement_allocation_id: String,
    #[serde(default)]
    pub settlement_amount_atoms: u64,
}

impl NavRedeemSettleOperation {
    pub fn has_vault_bridge_settlement(&self) -> bool {
        !self.settlement_asset_id.is_empty()
            || !self.settlement_bucket_id.is_empty()
            || !self.settlement_allocation_id.is_empty()
            || self.settlement_amount_atoms != 0
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nav_redeem_settle.issuer", &self.issuer)?;
        validate_lower_hex_len(
            "nav_redeem_settle.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "nav_redeem_settle.redemption_id",
            &self.redemption_id,
            NAV_REDEMPTION_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "nav_redeem_settle.settlement_receipt_hash",
            &self.settlement_receipt_hash,
            96,
        )?;
        if self.has_vault_bridge_settlement() {
            validate_lower_hex_len(
                "nav_redeem_settle.settlement_asset_id",
                &self.settlement_asset_id,
                ISSUED_ASSET_ID_HEX_LEN,
            )?;
            if self.settlement_asset_id == self.asset_id {
                return Err(
                    "nav_redeem_settle.settlement_asset_id must differ from asset_id".to_string(),
                );
            }
            validate_lower_hex_len(
                "nav_redeem_settle.settlement_bucket_id",
                &self.settlement_bucket_id,
                VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
            )?;
            validate_lower_hex_len(
                "nav_redeem_settle.settlement_allocation_id",
                &self.settlement_allocation_id,
                VAULT_BRIDGE_ALLOCATION_ID_HEX_LEN,
            )?;
            if self.settlement_amount_atoms == 0 {
                return Err("nav_redeem_settle.settlement_amount_atoms must be nonzero when settlement is present".to_string());
            }
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "issuer={}\nasset_id={}\nredemption_id={}\nsettlement_receipt_hash={}\n",
            self.issuer, self.asset_id, self.redemption_id, self.settlement_receipt_hash
        )
        .into_bytes();
        if self.has_vault_bridge_settlement() {
            bytes.extend_from_slice(
                format!(
                    "settlement_asset_id={}\nsettlement_bucket_id={}\nsettlement_allocation_id={}\nsettlement_amount_atoms={}\n",
                    self.settlement_asset_id,
                    self.settlement_bucket_id,
                    self.settlement_allocation_id,
                    self.settlement_amount_atoms
                )
                .as_bytes(),
            );
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositProposeOperation {
    pub proposer: String,
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
    pub expires_at_height: u64,
}

impl VaultBridgeDepositProposeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_deposit_propose.proposer", &self.proposer)?;
        validate_lower_hex_len(
            "vault_bridge_deposit_propose.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_propose.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        self.evidence.validate()?;
        validate_vault_bridge_policy_hash(
            "vault_bridge_deposit_propose.policy_hash",
            &self.policy_hash,
        )?;
        validate_vault_bridge_deposit_source_proof_fields(
            "vault_bridge_deposit_propose",
            &self.source_proof_kind,
            &self.source_proof_hash,
            &self.source_public_values_hash,
        )?;
        if self.expires_at_height == 0 {
            return Err(
                "vault_bridge_deposit_propose.expires_at_height must be nonzero".to_string(),
            );
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "proposer={}\nasset_id={}\nevidence_root={}\npolicy_hash={}\nsource_proof_kind={}\nsource_proof_hash={}\nsource_public_values_hash={}\nexpires_at_height={}\n",
            self.proposer,
            self.asset_id,
            self.evidence_root,
            self.policy_hash,
            self.source_proof_kind,
            self.source_proof_hash,
            self.source_public_values_hash,
            self.expires_at_height
        )
        .into_bytes();
        self.evidence.append_signing_bytes(&mut bytes, "evidence");
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositChallengeOperation {
    pub challenger: String,
    pub asset_id: String,
    pub evidence_root: String,
    pub challenge_hash: String,
    pub bond: u64,
}

impl VaultBridgeDepositChallengeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field(
            "vault_bridge_deposit_challenge.challenger",
            &self.challenger,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_challenge.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_challenge.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_challenge.challenge_hash",
            &self.challenge_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "challenger={}\nasset_id={}\nevidence_root={}\nchallenge_hash={}\nbond={}\n",
            self.challenger, self.asset_id, self.evidence_root, self.challenge_hash, self.bond
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositAttestOperation {
    pub attestor: String,
    pub asset_id: String,
    pub evidence_root: String,
    pub pass: bool,
    pub observation_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<VaultBridgeDepositObservation>,
}

impl VaultBridgeDepositAttestOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_deposit_attest.attestor", &self.attestor)?;
        validate_lower_hex_len(
            "vault_bridge_deposit_attest.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_attest.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_attest.observation_root",
            &self.observation_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        if let Some(observation) = &self.observation {
            observation.validate()?;
            let expected_root = vault_bridge_deposit_observation_root(observation)?;
            if self.observation_root != expected_root {
                return Err(
                    "vault_bridge_deposit_attest.observation_root does not match observation"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "attestor={}\nasset_id={}\nevidence_root={}\npass={}\nobservation_root={}\n",
            self.attestor, self.asset_id, self.evidence_root, self.pass, self.observation_root
        )
        .into_bytes();
        if let Some(observation) = &self.observation {
            observation.append_signing_bytes(&mut bytes, "observation");
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositFinalizeOperation {
    pub finalizer: String,
    pub asset_id: String,
    pub evidence_root: String,
}

impl VaultBridgeDepositFinalizeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_deposit_finalize.finalizer", &self.finalizer)?;
        validate_lower_hex_len(
            "vault_bridge_deposit_finalize.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_finalize.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "finalizer={}\nasset_id={}\nevidence_root={}\n",
            self.finalizer, self.asset_id, self.evidence_root
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositClaimOperation {
    pub claimer: String,
    pub asset_id: String,
    pub evidence_root: String,
    pub policy_hash: String,
    pub recipient: String,
    pub amount_atoms: u64,
}

impl VaultBridgeDepositClaimOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_deposit_claim.claimer", &self.claimer)?;
        validate_lower_hex_len(
            "vault_bridge_deposit_claim.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_deposit_claim.evidence_root",
            &self.evidence_root,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_vault_bridge_policy_hash(
            "vault_bridge_deposit_claim.policy_hash",
            &self.policy_hash,
        )?;
        validate_text_field("vault_bridge_deposit_claim.recipient", &self.recipient)?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_deposit_claim.amount_atoms must be nonzero".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "claimer={}\nasset_id={}\nevidence_root={}\npolicy_hash={}\nrecipient={}\namount_atoms={}\n",
            self.claimer,
            self.asset_id,
            self.evidence_root,
            self.policy_hash,
            self.recipient,
            self.amount_atoms
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeReceiptSubmitOperation {
    pub operator: String,
    pub asset_id: String,
    pub source_domain: String,
    pub source_asset: String,
    pub claim_type: String,
    pub amount_atoms: u64,
    pub source_tx_or_attestation: String,
    pub finality_ref: String,
    pub vault_id: String,
    pub policy_hash: String,
    pub expires_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_deposit_evidence: Option<VaultBridgeDepositEvidence>,
}

impl VaultBridgeReceiptSubmitOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_receipt_submit.operator", &self.operator)?;
        validate_lower_hex_len(
            "vault_bridge_receipt_submit.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_text_field(
            "vault_bridge_receipt_submit.source_domain",
            &self.source_domain,
        )?;
        validate_text_field(
            "vault_bridge_receipt_submit.source_asset",
            &self.source_asset,
        )?;
        validate_text_field("vault_bridge_receipt_submit.claim_type", &self.claim_type)?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_receipt_submit.amount_atoms must be nonzero".to_string());
        }
        validate_text_field(
            "vault_bridge_receipt_submit.source_tx_or_attestation",
            &self.source_tx_or_attestation,
        )?;
        validate_text_field(
            "vault_bridge_receipt_submit.finality_ref",
            &self.finality_ref,
        )?;
        validate_text_field("vault_bridge_receipt_submit.vault_id", &self.vault_id)?;
        validate_vault_bridge_policy_hash(
            "vault_bridge_receipt_submit.policy_hash",
            &self.policy_hash,
        )?;
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
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "operator={}\nasset_id={}\nsource_domain_bytes={}\nsource_domain={}\nsource_asset={}\nclaim_type={}\namount_atoms={}\nsource_tx_or_attestation_bytes={}\nsource_tx_or_attestation={}\nfinality_ref_bytes={}\nfinality_ref={}\nvault_id={}\npolicy_hash={}\nexpires_at_height={}\n",
            self.operator,
            self.asset_id,
            self.source_domain.len(),
            self.source_domain,
            self.source_asset,
            self.claim_type,
            self.amount_atoms,
            self.source_tx_or_attestation.len(),
            self.source_tx_or_attestation,
            self.finality_ref.len(),
            self.finality_ref,
            self.vault_id,
            self.policy_hash,
            self.expires_at_height
        )
        .into_bytes();
        bytes.extend_from_slice(
            format!(
                "bridge_deposit_evidence_present={}\n",
                self.bridge_deposit_evidence.is_some()
            )
            .as_bytes(),
        );
        if let Some(evidence) = &self.bridge_deposit_evidence {
            evidence.append_signing_bytes(&mut bytes, "bridge_deposit_evidence");
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeReceiptCountOperation {
    pub operator: String,
    pub asset_id: String,
    pub receipt_id: String,
    pub haircut_bps: u64,
    pub counted_value_atoms: u64,
    pub evidence_root: String,
    pub policy_hash: String,
}

impl VaultBridgeReceiptCountOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_receipt_count.operator", &self.operator)?;
        validate_lower_hex_len(
            "vault_bridge_receipt_count.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_receipt_count.receipt_id",
            &self.receipt_id,
            VAULT_BRIDGE_RECEIPT_ID_HEX_LEN,
        )?;
        if self.haircut_bps > 10_000 {
            return Err("vault_bridge_receipt_count.haircut_bps exceeds 10000".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_receipt_count.evidence_root",
            &self.evidence_root,
            96,
        )?;
        validate_vault_bridge_policy_hash(
            "vault_bridge_receipt_count.policy_hash",
            &self.policy_hash,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "operator={}\nasset_id={}\nreceipt_id={}\nhaircut_bps={}\ncounted_value_atoms={}\nevidence_root={}\npolicy_hash={}\n",
            self.operator,
            self.asset_id,
            self.receipt_id,
            self.haircut_bps,
            self.counted_value_atoms,
            self.evidence_root,
            self.policy_hash
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeMintFromReceiptsOperation {
    pub issuer: String,
    pub to: String,
    pub asset_id: String,
    pub bucket_id: String,
    pub amount_atoms: u64,
    pub receipt_ids: Vec<String>,
    pub epoch: u64,
    pub reserve_packet_hash: String,
}

impl VaultBridgeMintFromReceiptsOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_mint_from_receipts.issuer", &self.issuer)?;
        validate_text_field("vault_bridge_mint_from_receipts.to", &self.to)?;
        if self.issuer == self.to {
            return Err("vault_bridge_mint_from_receipts.to must differ from issuer".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_mint_from_receipts.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_mint_from_receipts.bucket_id",
            &self.bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_mint_from_receipts.amount_atoms must be nonzero".to_string());
        }
        if self.receipt_ids.is_empty() {
            return Err("vault_bridge_mint_from_receipts.receipt_ids must be nonempty".to_string());
        }
        if self.receipt_ids.len() > MAX_VAULT_BRIDGE_MINT_RECEIPTS {
            return Err(format!(
                "vault_bridge_mint_from_receipts.receipt_ids exceeds maximum of {MAX_VAULT_BRIDGE_MINT_RECEIPTS}"
            ));
        }
        for receipt_id in &self.receipt_ids {
            validate_lower_hex_len(
                "vault_bridge_mint_from_receipts.receipt_id",
                receipt_id,
                VAULT_BRIDGE_RECEIPT_ID_HEX_LEN,
            )?;
        }
        if self.epoch == 0 {
            return Err("vault_bridge_mint_from_receipts.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_mint_from_receipts.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "issuer={}\nto={}\nasset_id={}\nbucket_id={}\namount_atoms={}\nreceipt_count={}\nepoch={}\nreserve_packet_hash={}\n",
            self.issuer,
            self.to,
            self.asset_id,
            self.bucket_id,
            self.amount_atoms,
            self.receipt_ids.len(),
            self.epoch,
            self.reserve_packet_hash
        )
        .into_bytes();
        for (index, receipt_id) in self.receipt_ids.iter().enumerate() {
            bytes.extend_from_slice(format!("receipt[{index}]={receipt_id}\n").as_bytes());
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeBurnToRedeemOperation {
    pub owner: String,
    pub issuer: String,
    pub asset_id: String,
    pub bucket_id: String,
    pub amount_atoms: u64,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    pub destination_ref: String,
}

impl VaultBridgeBurnToRedeemOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_burn_to_redeem.owner", &self.owner)?;
        validate_text_field("vault_bridge_burn_to_redeem.issuer", &self.issuer)?;
        if self.owner == self.issuer {
            return Err("vault_bridge_burn_to_redeem.owner must differ from issuer".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_burn_to_redeem.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_burn_to_redeem.bucket_id",
            &self.bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        if self.amount_atoms == 0 {
            return Err("vault_bridge_burn_to_redeem.amount_atoms must be nonzero".to_string());
        }
        if self.epoch == 0 {
            return Err("vault_bridge_burn_to_redeem.epoch must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "vault_bridge_burn_to_redeem.reserve_packet_hash",
            &self.reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        validate_text_field(
            "vault_bridge_burn_to_redeem.destination_ref",
            &self.destination_ref,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "owner={}\nissuer={}\nasset_id={}\nbucket_id={}\namount_atoms={}\nepoch={}\nreserve_packet_hash={}\ndestination_ref_bytes={}\ndestination_ref={}\n",
            self.owner,
            self.issuer,
            self.asset_id,
            self.bucket_id,
            self.amount_atoms,
            self.epoch,
            self.reserve_packet_hash,
            self.destination_ref.len(),
            self.destination_ref,
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeRedeemSettleOperation {
    pub issuer_or_redemption_account: String,
    pub asset_id: String,
    pub redemption_id: String,
    pub settlement_receipt_hash: String,
    pub settled_atoms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withdrawal_observations: Vec<VaultBridgeWithdrawalExecutionAttestation>,
}

impl VaultBridgeRedeemSettleOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field(
            "vault_bridge_redeem_settle.issuer_or_redemption_account",
            &self.issuer_or_redemption_account,
        )?;
        validate_lower_hex_len(
            "vault_bridge_redeem_settle.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_redeem_settle.redemption_id",
            &self.redemption_id,
            VAULT_BRIDGE_REDEMPTION_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_redeem_settle.settlement_receipt_hash",
            &self.settlement_receipt_hash,
            96,
        )?;
        if self.settled_atoms == 0 {
            return Err("vault_bridge_redeem_settle.settled_atoms must be nonzero".to_string());
        }
        if self.withdrawal_observations.len() > MAX_NAV_ATTESTATIONS_PER_PACKET {
            return Err(format!(
                "vault_bridge_redeem_settle.withdrawal_observations exceeds maximum of {MAX_NAV_ATTESTATIONS_PER_PACKET}"
            ));
        }
        let mut attestors = BTreeSet::new();
        for attestation in &self.withdrawal_observations {
            attestation.validate()?;
            if !attestors.insert(attestation.attestor.clone()) {
                return Err(
                    "vault_bridge_redeem_settle has duplicate withdrawal observation attestors"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "issuer_or_redemption_account={}\nasset_id={}\nredemption_id={}\nsettlement_receipt_hash={}\nsettled_atoms={}\n",
            self.issuer_or_redemption_account,
            self.asset_id,
            self.redemption_id,
            self.settlement_receipt_hash,
            self.settled_atoms
        )
        .into_bytes();
        for (index, attestation) in self.withdrawal_observations.iter().enumerate() {
            bytes.extend_from_slice(
                format!(
                    "withdrawal_observation[{index}].attestor={}\nwithdrawal_observation[{index}].observation_root={}\nwithdrawal_observation[{index}].signature_hex={}\n",
                    attestation.attestor,
                    attestation.observation_root,
                    attestation.signature_hex,
                )
                .as_bytes(),
            );
            attestation.observation.append_signing_bytes(
                &mut bytes,
                &format!("withdrawal_observation[{index}].observation"),
            );
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeBucketImpairOperation {
    pub operator: String,
    pub asset_id: String,
    pub bucket_id: String,
    pub updated_counted_value_atoms: u64,
    pub impairment_factor_bps: u64,
    pub reason_hash: String,
    pub policy_hash: String,
}

impl VaultBridgeBucketImpairOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("vault_bridge_bucket_impair.operator", &self.operator)?;
        validate_lower_hex_len(
            "vault_bridge_bucket_impair.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_bucket_impair.bucket_id",
            &self.bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        if self.impairment_factor_bps > 10_000 {
            return Err(
                "vault_bridge_bucket_impair.impairment_factor_bps exceeds 10000".to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_bucket_impair.reason_hash",
            &self.reason_hash,
            96,
        )?;
        validate_vault_bridge_policy_hash(
            "vault_bridge_bucket_impair.policy_hash",
            &self.policy_hash,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "operator={}\nasset_id={}\nbucket_id={}\nupdated_counted_value_atoms={}\nimpairment_factor_bps={}\nreason_hash={}\npolicy_hash={}\n",
            self.operator,
            self.asset_id,
            self.bucket_id,
            self.updated_counted_value_atoms,
            self.impairment_factor_bps,
            self.reason_hash,
            self.policy_hash
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeNavSubscriptionAllocateOperation {
    pub operator: String,
    pub nav_asset_id: String,
    pub settlement_asset_id: String,
    pub settlement_bucket_id: String,
    pub settlement_receipt_id: String,
    pub settlement_amount_atoms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consume_supply_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consume_supply_allocation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nav_recipient: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
}

impl VaultBridgeNavSubscriptionAllocateOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field(
            "vault_bridge_nav_subscription_allocate.operator",
            &self.operator,
        )?;
        validate_lower_hex_len(
            "vault_bridge_nav_subscription_allocate.nav_asset_id",
            &self.nav_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_nav_subscription_allocate.settlement_asset_id",
            &self.settlement_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.settlement_asset_id == self.nav_asset_id {
            return Err(
                "vault_bridge_nav_subscription_allocate.settlement_asset_id must differ from nav_asset_id"
                    .to_string(),
            );
        }
        validate_lower_hex_len(
            "vault_bridge_nav_subscription_allocate.settlement_bucket_id",
            &self.settlement_bucket_id,
            VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "vault_bridge_nav_subscription_allocate.settlement_receipt_id",
            &self.settlement_receipt_id,
            VAULT_BRIDGE_RECEIPT_ID_HEX_LEN,
        )?;
        if self.settlement_amount_atoms == 0 {
            return Err(
                "vault_bridge_nav_subscription_allocate.settlement_amount_atoms must be nonzero"
                    .to_string(),
            );
        }
        match (
            self.consume_supply_owner.as_ref(),
            self.consume_supply_allocation_id.as_ref(),
            self.nav_recipient.as_ref(),
        ) {
            (None, None, None) => {}
            (Some(owner), Some(allocation_id), Some(nav_recipient)) => {
                validate_text_field(
                    "vault_bridge_nav_subscription_allocate.consume_supply_owner",
                    owner,
                )?;
                validate_lower_hex_len(
                    "vault_bridge_nav_subscription_allocate.consume_supply_allocation_id",
                    allocation_id,
                    VAULT_BRIDGE_ALLOCATION_ID_HEX_LEN,
                )?;
                validate_text_field(
                    "vault_bridge_nav_subscription_allocate.nav_recipient",
                    nav_recipient,
                )?;
                if owner != nav_recipient {
                    return Err(
                        "vault_bridge_nav_subscription_allocate.consume_supply_owner must match nav_recipient"
                            .to_string(),
                    );
                }
            }
            _ => {
                return Err(
                    "vault_bridge_nav_subscription_allocate consume-supply fields must be all present or all absent"
                        .to_string(),
                );
            }
        }
        if let Some(subscription_id) = self.subscription_id.as_ref() {
            validate_text_field(
                "vault_bridge_nav_subscription_allocate.subscription_id",
                subscription_id,
            )?;
            if self.nav_recipient.is_none() {
                return Err(
                    "vault_bridge_nav_subscription_allocate.subscription_id requires nav_recipient"
                        .to_string(),
                );
            }
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "operator={}\nnav_asset_id={}\nsettlement_asset_id={}\nsettlement_bucket_id={}\nsettlement_receipt_id={}\nsettlement_amount_atoms={}\n",
            self.operator,
            self.nav_asset_id,
            self.settlement_asset_id,
            self.settlement_bucket_id,
            self.settlement_receipt_id,
            self.settlement_amount_atoms
        )
        .into_bytes();
        if let (Some(owner), Some(allocation_id), Some(nav_recipient)) = (
            self.consume_supply_owner.as_ref(),
            self.consume_supply_allocation_id.as_ref(),
            self.nav_recipient.as_ref(),
        ) {
            bytes.extend_from_slice(
                format!(
                    "consume_supply_owner={owner}\nconsume_supply_allocation_id={allocation_id}\nnav_recipient={nav_recipient}\n"
                )
                .as_bytes(),
            );
        }
        if let Some(subscription_id) = self.subscription_id.as_ref() {
            bytes.extend_from_slice(format!("subscription_id={subscription_id}\n").as_bytes());
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapRouteInitOperation {
    pub operator: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub route_trust_class: String,
    pub native_nav_asset_id: String,
    pub settlement_asset_id: String,
    pub handoff_controller: String,
    pub settlement_adapter: String,
    pub wrapped_navcoin_token: String,
    pub ethereum_chain_id: u64,
    pub route_supply_cap_atoms: u64,
    pub packet_notional_cap_atoms: u64,
    pub latest_finalized_nav_epoch: u64,
    pub return_finality_blocks: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethereum_verification_policy: Option<EthereumRouteVerificationPolicyV1>,
}

impl PftlUniswapRouteInitOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("pftl_uniswap_route_init.operator", &self.operator)?;
        validate_text_field("pftl_uniswap_route_init.route_id", &self.route_id)?;
        validate_lower_hex_len(
            "pftl_uniswap_route_init.route_config_digest",
            &self.route_config_digest,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_text_field(
            "pftl_uniswap_route_init.route_trust_class",
            &self.route_trust_class,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_route_init.native_nav_asset_id",
            &self.native_nav_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_route_init.settlement_asset_id",
            &self.settlement_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.native_nav_asset_id == self.settlement_asset_id {
            return Err(
                "pftl_uniswap_route_init native and settlement assets must differ".to_string(),
            );
        }
        validate_evm_address_text(
            "pftl_uniswap_route_init.handoff_controller",
            &self.handoff_controller,
        )?;
        validate_evm_address_text(
            "pftl_uniswap_route_init.settlement_adapter",
            &self.settlement_adapter,
        )?;
        validate_evm_address_text(
            "pftl_uniswap_route_init.wrapped_navcoin_token",
            &self.wrapped_navcoin_token,
        )?;
        if self.ethereum_chain_id == 0
            || self.route_supply_cap_atoms == 0
            || self.packet_notional_cap_atoms == 0
            || self.return_finality_blocks == 0
        {
            return Err(
                "pftl_uniswap_route_init chain, cap, and finality fields must be nonzero"
                    .to_string(),
            );
        }
        if let Some(policy) = &self.ethereum_verification_policy {
            policy.validate().map_err(|error| {
                format!("pftl_uniswap_route_init Ethereum verification policy: {error:?}")
            })?;
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "operator={}\nroute_id={}\nroute_family={}\nroute_config_digest={}\nroute_trust_class={}\nnative_nav_asset_id={}\nsettlement_asset_id={}\nhandoff_controller={}\nsettlement_adapter={}\nwrapped_navcoin_token={}\nethereum_chain_id={}\nroute_supply_cap_atoms={}\npacket_notional_cap_atoms={}\nlatest_finalized_nav_epoch={}\nreturn_finality_blocks={}\n",
            self.operator,
            self.route_id,
            PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT,
            self.route_config_digest,
            self.route_trust_class,
            self.native_nav_asset_id,
            self.settlement_asset_id,
            self.handoff_controller,
            self.settlement_adapter,
            self.wrapped_navcoin_token,
            self.ethereum_chain_id,
            self.route_supply_cap_atoms,
            self.packet_notional_cap_atoms,
            self.latest_finalized_nav_epoch,
            self.return_finality_blocks
        )
        .into_bytes();
        if let Some(policy) = &self.ethereum_verification_policy {
            bytes.extend_from_slice(
                format!(
                    "ethereum_verification_policy_commitment={}\n",
                    bytes_to_hex(&hash48(b"postfiat.ethereum.route-verification-policy.v1", &[
                        policy.authority_epoch.to_be_bytes().as_slice(),
                        policy.committee_root.0.as_slice(),
                        policy.minimum_confirmations.to_be_bytes().as_slice(),
                        policy.handoff_controller_code_hash.as_slice(),
                        policy.wrapped_navcoin_code_hash.as_slice(),
                    ].concat()))
                )
                .as_bytes(),
            );
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapPrimarySubscribeOperation {
    pub subscriber: String,
    pub route_id: String,
    pub settlement_asset_id: String,
    pub subscription_nonce: String,
    pub settlement_value_atoms: u64,
    pub nav_price_settlement_atoms_per_nav_atom: u64,
    pub pricing_nav_epoch: u64,
    pub pricing_reserve_packet_hash: String,
}

impl PftlUniswapPrimarySubscribeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field(
            "pftl_uniswap_primary_subscribe.subscriber",
            &self.subscriber,
        )?;
        validate_text_field("pftl_uniswap_primary_subscribe.route_id", &self.route_id)?;
        validate_lower_hex_len(
            "pftl_uniswap_primary_subscribe.settlement_asset_id",
            &self.settlement_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_primary_subscribe.subscription_nonce",
            &self.subscription_nonce,
            64,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_primary_subscribe.pricing_reserve_packet_hash",
            &self.pricing_reserve_packet_hash,
            NAV_RESERVE_PACKET_ID_HEX_LEN,
        )?;
        if self.settlement_value_atoms == 0
            || self.nav_price_settlement_atoms_per_nav_atom == 0
            || self.pricing_nav_epoch == 0
        {
            return Err(
                "pftl_uniswap_primary_subscribe amount, price, and epoch must be nonzero"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "subscriber={}\nroute_id={}\nsettlement_asset_id={}\nsubscription_nonce={}\nsettlement_value_atoms={}\nnav_price_settlement_atoms_per_nav_atom={}\npricing_nav_epoch={}\npricing_reserve_packet_hash={}\n",
            self.subscriber,
            self.route_id,
            self.settlement_asset_id,
            self.subscription_nonce,
            self.settlement_value_atoms,
            self.nav_price_settlement_atoms_per_nav_atom,
            self.pricing_nav_epoch,
            self.pricing_reserve_packet_hash
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapExportDebitOperation {
    pub owner: String,
    pub route_id: String,
    pub packet_hash: String,
    pub export_nonce: String,
    pub ethereum_recipient: String,
    pub amount_atoms: u64,
    pub destination_deadline_seconds: u64,
    pub refund_delay_blocks: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethereum_packet_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethereum_packet_schema_version: Option<u32>,
}

impl PftlUniswapExportDebitOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("pftl_uniswap_export_debit.owner", &self.owner)?;
        validate_text_field("pftl_uniswap_export_debit.route_id", &self.route_id)?;
        validate_lower_hex_len(
            "pftl_uniswap_export_debit.packet_hash",
            &self.packet_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_export_debit.export_nonce",
            &self.export_nonce,
            64,
        )?;
        if let Some(packet_digest) = &self.ethereum_packet_digest {
            validate_lower_hex_len(
                "pftl_uniswap_export_debit.ethereum_packet_digest",
                packet_digest,
                64,
            )?;
        }
        if self.ethereum_packet_schema_version.is_some_and(|version| version == 0) {
            return Err(
                "pftl_uniswap_export_debit.ethereum_packet_schema_version must be nonzero"
                    .to_string(),
            );
        }
        validate_evm_address_text(
            "pftl_uniswap_export_debit.ethereum_recipient",
            &self.ethereum_recipient,
        )?;
        if self.amount_atoms == 0
            || self.destination_deadline_seconds == 0
            || self.refund_delay_blocks == 0
        {
            return Err(
                "pftl_uniswap_export_debit amount, deadline, and refund delay must be nonzero"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "owner={}\nroute_id={}\npacket_hash={}\nexport_nonce={}\nethereum_recipient={}\namount_atoms={}\ndestination_deadline_seconds={}\nrefund_delay_blocks={}\n",
            self.owner,
            self.route_id,
            self.packet_hash,
            self.export_nonce,
            self.ethereum_recipient,
            self.amount_atoms,
            self.destination_deadline_seconds,
            self.refund_delay_blocks
        )
        .into_bytes();
        if let Some(packet_digest) = &self.ethereum_packet_digest {
            bytes.extend_from_slice(
                format!("ethereum_packet_digest={packet_digest}\n").as_bytes(),
            );
        }
        if let Some(schema_version) = self.ethereum_packet_schema_version {
            bytes.extend_from_slice(
                format!("ethereum_packet_schema_version={schema_version}\n").as_bytes(),
            );
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapDestinationConsumeOperation {
    pub operator: String,
    pub route_id: String,
    pub packet_hash: String,
    pub ethereum_consume_tx_hash: String,
    pub consumed_height: u64,
    pub finalized_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_event_proof: Option<EthereumExternalEventProofV1>,
}

impl PftlUniswapDestinationConsumeOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("pftl_uniswap_destination_consume.operator", &self.operator)?;
        validate_text_field("pftl_uniswap_destination_consume.route_id", &self.route_id)?;
        validate_lower_hex_len(
            "pftl_uniswap_destination_consume.packet_hash",
            &self.packet_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_destination_consume.ethereum_consume_tx_hash",
            &self.ethereum_consume_tx_hash,
            64,
        )?;
        if self.consumed_height == 0 || self.finalized_height == 0 {
            return Err("pftl_uniswap_destination_consume heights must be nonzero".to_string());
        }
        if let Some(proof) = &self.external_event_proof {
            proof.validate_bounds().map_err(|error| {
                format!("pftl_uniswap_destination_consume Ethereum proof: {error:?}")
            })?;
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "operator={}\nroute_id={}\npacket_hash={}\nethereum_consume_tx_hash={}\nconsumed_height={}\nfinalized_height={}\n",
            self.operator,
            self.route_id,
            self.packet_hash,
            self.ethereum_consume_tx_hash,
            self.consumed_height,
            self.finalized_height
        )
        .into_bytes();
        append_external_event_proof_commitment(&mut bytes, self.external_event_proof.as_ref());
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapRefundSourceOperation {
    pub operator: String,
    pub route_id: String,
    pub packet_hash: String,
    pub non_consumption_proof_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_event_proof: Option<EthereumExternalEventProofV1>,
}

impl PftlUniswapRefundSourceOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("pftl_uniswap_refund_source.operator", &self.operator)?;
        validate_text_field("pftl_uniswap_refund_source.route_id", &self.route_id)?;
        validate_lower_hex_len(
            "pftl_uniswap_refund_source.packet_hash",
            &self.packet_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_refund_source.non_consumption_proof_hash",
            &self.non_consumption_proof_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        if let Some(proof) = &self.external_event_proof {
            proof.validate_bounds().map_err(|error| {
                format!("pftl_uniswap_refund_source Ethereum proof: {error:?}")
            })?;
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "operator={}\nroute_id={}\npacket_hash={}\nnon_consumption_proof_hash={}\n",
            self.operator, self.route_id, self.packet_hash, self.non_consumption_proof_hash
        )
        .into_bytes();
        append_external_event_proof_commitment(&mut bytes, self.external_event_proof.as_ref());
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReturnImportOperation {
    pub operator: String,
    pub route_id: String,
    pub burn_event_hash: String,
    pub ethereum_chain_id: u64,
    pub bridge_controller: String,
    pub wrapped_navcoin_token: String,
    pub native_nav_asset_id: String,
    pub ethereum_sender: String,
    pub pftl_recipient: String,
    pub amount_atoms: u64,
    pub return_nonce: String,
    pub burn_height: u64,
    pub finalized_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_event_proof: Option<EthereumExternalEventProofV1>,
}

impl PftlUniswapReturnImportOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("pftl_uniswap_return_import.operator", &self.operator)?;
        validate_text_field("pftl_uniswap_return_import.route_id", &self.route_id)?;
        validate_lower_hex_len(
            "pftl_uniswap_return_import.burn_event_hash",
            &self.burn_event_hash,
            64,
        )?;
        if self.ethereum_chain_id == 0 {
            return Err("pftl_uniswap_return_import.ethereum_chain_id must be nonzero".to_string());
        }
        validate_evm_address_text(
            "pftl_uniswap_return_import.bridge_controller",
            &self.bridge_controller,
        )?;
        validate_evm_address_text(
            "pftl_uniswap_return_import.wrapped_navcoin_token",
            &self.wrapped_navcoin_token,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_return_import.native_nav_asset_id",
            &self.native_nav_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_evm_address_text(
            "pftl_uniswap_return_import.ethereum_sender",
            &self.ethereum_sender,
        )?;
        validate_text_field(
            "pftl_uniswap_return_import.pftl_recipient",
            &self.pftl_recipient,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_return_import.return_nonce",
            &self.return_nonce,
            64,
        )?;
        if self.amount_atoms == 0 || self.burn_height == 0 || self.finalized_height == 0 {
            return Err(
                "pftl_uniswap_return_import amount and heights must be nonzero".to_string(),
            );
        }
        if let Some(proof) = &self.external_event_proof {
            proof.validate_bounds().map_err(|error| {
                format!("pftl_uniswap_return_import Ethereum proof: {error:?}")
            })?;
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "operator={}\nroute_id={}\nburn_event_hash={}\nethereum_chain_id={}\nbridge_controller={}\nwrapped_navcoin_token={}\nnative_nav_asset_id={}\nethereum_sender={}\npftl_recipient={}\namount_atoms={}\nreturn_nonce={}\nburn_height={}\nfinalized_height={}\n",
            self.operator,
            self.route_id,
            self.burn_event_hash,
            self.ethereum_chain_id,
            self.bridge_controller,
            self.wrapped_navcoin_token,
            self.native_nav_asset_id,
            self.ethereum_sender,
            self.pftl_recipient,
            self.amount_atoms,
            self.return_nonce,
            self.burn_height,
            self.finalized_height
        )
        .into_bytes();
        append_external_event_proof_commitment(&mut bytes, self.external_event_proof.as_ref());
        bytes
    }
}

fn append_external_event_proof_commitment(
    bytes: &mut Vec<u8>,
    proof: Option<&EthereumExternalEventProofV1>,
) {
    if let Some(proof) = proof {
        let commitment = proof.commitment().unwrap_or_else(|_| {
            let fallback = serde_json::to_vec(proof).unwrap_or_default();
            FastSwapOpaqueHashV1(hash48(
                b"postfiat.ethereum.invalid-external-event-proof.v1",
                &fallback,
            ))
        });
        bytes.extend_from_slice(
            format!("external_event_proof_commitment={}\n", bytes_to_hex(&commitment.0))
                .as_bytes(),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "operation")]
pub enum AssetTransactionOperation {
    #[serde(rename = "asset_create")]
    AssetCreate(AssetCreateOperation),
    #[serde(rename = "trust_set")]
    TrustSet(TrustSetOperation),
    #[serde(rename = "issued_payment")]
    IssuedPayment(IssuedPaymentOperation),
    #[serde(rename = "asset_burn")]
    AssetBurn(AssetBurnOperation),
    #[serde(rename = "asset_clawback")]
    AssetClawback(AssetClawbackOperation),
    #[serde(rename = "nav_asset_register")]
    NavAssetRegister(NavAssetRegisterOperation),
    #[serde(rename = "nav_reserve_submit")]
    NavReserveSubmit(NavReserveSubmitOperation),
    #[serde(rename = "nav_reserve_challenge")]
    NavReserveChallenge(NavReserveChallengeOperation),
    #[serde(rename = "nav_epoch_finalize")]
    NavEpochFinalize(NavEpochFinalizeOperation),
    #[serde(rename = "market_ops_policy_register")]
    MarketOpsPolicyRegister(MarketOpsPolicyRegisterOperation),
    #[serde(rename = "market_ops_finalize")]
    MarketOpsFinalize(MarketOpsFinalizeOperation),
    #[serde(rename = "nav_mint_at_nav")]
    NavMintAtNav(NavMintAtNavOperation),
    #[serde(rename = "nav_redeem_at_nav")]
    NavRedeemAtNav(NavRedeemAtNavOperation),
    #[serde(rename = "nav_halt")]
    NavHalt(NavHaltOperation),
    #[serde(rename = "nav_profile_register")]
    NavProfileRegister(NavProfileRegisterOperation),
    #[serde(rename = "nav_redeem_settle")]
    NavRedeemSettle(NavRedeemSettleOperation),
    #[serde(rename = "nav_reserve_attest")]
    NavReserveAttest(NavReserveAttestOperation),
    #[serde(rename = "nav_attestor_register")]
    NavAttestorRegister(NavAttestorRegisterOperation),
    #[serde(rename = "vault_bridge_deposit_propose")]
    VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation),
    #[serde(rename = "vault_bridge_deposit_challenge")]
    VaultBridgeDepositChallenge(VaultBridgeDepositChallengeOperation),
    #[serde(rename = "vault_bridge_deposit_attest")]
    VaultBridgeDepositAttest(VaultBridgeDepositAttestOperation),
    #[serde(rename = "vault_bridge_deposit_finalize")]
    VaultBridgeDepositFinalize(VaultBridgeDepositFinalizeOperation),
    #[serde(rename = "vault_bridge_deposit_claim")]
    VaultBridgeDepositClaim(VaultBridgeDepositClaimOperation),
    #[serde(rename = "vault_bridge_receipt_submit")]
    VaultBridgeReceiptSubmit(VaultBridgeReceiptSubmitOperation),
    #[serde(rename = "vault_bridge_receipt_count")]
    VaultBridgeReceiptCount(VaultBridgeReceiptCountOperation),
    #[serde(rename = "vault_bridge_mint_from_receipts")]
    VaultBridgeMintFromReceipts(VaultBridgeMintFromReceiptsOperation),
    #[serde(rename = "vault_bridge_burn_to_redeem")]
    VaultBridgeBurnToRedeem(VaultBridgeBurnToRedeemOperation),
    #[serde(rename = "vault_bridge_redeem_settle")]
    VaultBridgeRedeemSettle(VaultBridgeRedeemSettleOperation),
    #[serde(rename = "vault_bridge_bucket_impair")]
    VaultBridgeBucketImpair(VaultBridgeBucketImpairOperation),
    #[serde(rename = "vault_bridge_nav_subscription_allocate")]
    VaultBridgeNavSubscriptionAllocate(VaultBridgeNavSubscriptionAllocateOperation),
    #[serde(rename = "pftl_uniswap_route_init")]
    PftlUniswapRouteInit(PftlUniswapRouteInitOperation),
    #[serde(rename = "pftl_uniswap_primary_subscribe")]
    PftlUniswapPrimarySubscribe(PftlUniswapPrimarySubscribeOperation),
    #[serde(rename = "pftl_uniswap_export_debit")]
    PftlUniswapExportDebit(PftlUniswapExportDebitOperation),
    #[serde(rename = "pftl_uniswap_destination_consume")]
    PftlUniswapDestinationConsume(PftlUniswapDestinationConsumeOperation),
    #[serde(rename = "pftl_uniswap_refund_source")]
    PftlUniswapRefundSource(PftlUniswapRefundSourceOperation),
    #[serde(rename = "pftl_uniswap_return_import")]
    PftlUniswapReturnImport(PftlUniswapReturnImportOperation),
}

impl<'de> Deserialize<'de> for AssetTransactionOperation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let operation = value
            .as_object_mut()
            .and_then(|object| object.remove("operation"))
            .ok_or_else(|| serde::de::Error::custom("missing operation"))?;
        let operation = operation
            .as_str()
            .ok_or_else(|| serde::de::Error::custom("operation must be a string"))?;
        macro_rules! decode_operation {
            ($operation_type:ty, $variant:ident) => {
                serde_json::from_value::<$operation_type>(value)
                    .map(Self::$variant)
                    .map_err(serde::de::Error::custom)
            };
        }
        match operation {
            "asset_create" => decode_operation!(AssetCreateOperation, AssetCreate),
            "trust_set" => decode_operation!(TrustSetOperation, TrustSet),
            "issued_payment" => decode_operation!(IssuedPaymentOperation, IssuedPayment),
            "asset_burn" => decode_operation!(AssetBurnOperation, AssetBurn),
            "asset_clawback" => decode_operation!(AssetClawbackOperation, AssetClawback),
            "nav_asset_register" => {
                decode_operation!(NavAssetRegisterOperation, NavAssetRegister)
            }
            "nav_reserve_submit" => {
                decode_operation!(NavReserveSubmitOperation, NavReserveSubmit)
            }
            "nav_reserve_challenge" => {
                decode_operation!(NavReserveChallengeOperation, NavReserveChallenge)
            }
            "nav_epoch_finalize" => {
                decode_operation!(NavEpochFinalizeOperation, NavEpochFinalize)
            }
            "market_ops_policy_register" => {
                decode_operation!(MarketOpsPolicyRegisterOperation, MarketOpsPolicyRegister)
            }
            "market_ops_finalize" => {
                decode_operation!(MarketOpsFinalizeOperation, MarketOpsFinalize)
            }
            "nav_mint_at_nav" => decode_operation!(NavMintAtNavOperation, NavMintAtNav),
            "nav_redeem_at_nav" => decode_operation!(NavRedeemAtNavOperation, NavRedeemAtNav),
            "nav_halt" => decode_operation!(NavHaltOperation, NavHalt),
            "nav_profile_register" => {
                decode_operation!(NavProfileRegisterOperation, NavProfileRegister)
            }
            "nav_redeem_settle" => {
                decode_operation!(NavRedeemSettleOperation, NavRedeemSettle)
            }
            "nav_reserve_attest" => {
                decode_operation!(NavReserveAttestOperation, NavReserveAttest)
            }
            "nav_attestor_register" => {
                decode_operation!(NavAttestorRegisterOperation, NavAttestorRegister)
            }
            "vault_bridge_deposit_propose" => decode_operation!(
                VaultBridgeDepositProposeOperation,
                VaultBridgeDepositPropose
            ),
            "vault_bridge_deposit_challenge" => decode_operation!(
                VaultBridgeDepositChallengeOperation,
                VaultBridgeDepositChallenge
            ),
            "vault_bridge_deposit_attest" => {
                decode_operation!(VaultBridgeDepositAttestOperation, VaultBridgeDepositAttest)
            }
            "vault_bridge_deposit_finalize" => decode_operation!(
                VaultBridgeDepositFinalizeOperation,
                VaultBridgeDepositFinalize
            ),
            "vault_bridge_deposit_claim" => {
                decode_operation!(VaultBridgeDepositClaimOperation, VaultBridgeDepositClaim)
            }
            "vault_bridge_receipt_submit" => {
                decode_operation!(VaultBridgeReceiptSubmitOperation, VaultBridgeReceiptSubmit)
            }
            "vault_bridge_receipt_count" => {
                decode_operation!(VaultBridgeReceiptCountOperation, VaultBridgeReceiptCount)
            }
            "vault_bridge_mint_from_receipts" => decode_operation!(
                VaultBridgeMintFromReceiptsOperation,
                VaultBridgeMintFromReceipts
            ),
            "vault_bridge_burn_to_redeem" => {
                decode_operation!(VaultBridgeBurnToRedeemOperation, VaultBridgeBurnToRedeem)
            }
            "vault_bridge_redeem_settle" => {
                decode_operation!(VaultBridgeRedeemSettleOperation, VaultBridgeRedeemSettle)
            }
            "vault_bridge_bucket_impair" => {
                decode_operation!(VaultBridgeBucketImpairOperation, VaultBridgeBucketImpair)
            }
            "vault_bridge_nav_subscription_allocate" => decode_operation!(
                VaultBridgeNavSubscriptionAllocateOperation,
                VaultBridgeNavSubscriptionAllocate
            ),
            "pftl_uniswap_route_init" => {
                decode_operation!(PftlUniswapRouteInitOperation, PftlUniswapRouteInit)
            }
            "pftl_uniswap_primary_subscribe" => decode_operation!(
                PftlUniswapPrimarySubscribeOperation,
                PftlUniswapPrimarySubscribe
            ),
            "pftl_uniswap_export_debit" => {
                decode_operation!(PftlUniswapExportDebitOperation, PftlUniswapExportDebit)
            }
            "pftl_uniswap_destination_consume" => decode_operation!(
                PftlUniswapDestinationConsumeOperation,
                PftlUniswapDestinationConsume
            ),
            "pftl_uniswap_refund_source" => {
                decode_operation!(PftlUniswapRefundSourceOperation, PftlUniswapRefundSource)
            }
            "pftl_uniswap_return_import" => {
                decode_operation!(PftlUniswapReturnImportOperation, PftlUniswapReturnImport)
            }
            other => Err(serde::de::Error::custom(format!(
                "unknown asset operation `{other}`"
            ))),
        }
    }
}

impl AssetTransactionOperation {
    pub fn transaction_kind(&self) -> &'static str {
        match self {
            Self::AssetCreate(_) => ASSET_CREATE_TRANSACTION_KIND,
            Self::TrustSet(_) => TRUST_SET_TRANSACTION_KIND,
            Self::IssuedPayment(_) => ISSUED_PAYMENT_TRANSACTION_KIND,
            Self::AssetBurn(_) => ASSET_BURN_TRANSACTION_KIND,
            Self::AssetClawback(_) => ASSET_CLAWBACK_TRANSACTION_KIND,
            Self::NavAssetRegister(_) => NAV_ASSET_REGISTER_TRANSACTION_KIND,
            Self::NavReserveSubmit(_) => NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            Self::NavReserveChallenge(_) => NAV_RESERVE_CHALLENGE_TRANSACTION_KIND,
            Self::NavEpochFinalize(_) => NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            Self::MarketOpsPolicyRegister(_) => MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND,
            Self::MarketOpsFinalize(_) => MARKET_OPS_FINALIZE_TRANSACTION_KIND,
            Self::NavMintAtNav(_) => NAV_MINT_AT_NAV_TRANSACTION_KIND,
            Self::NavRedeemAtNav(_) => NAV_REDEEM_AT_NAV_TRANSACTION_KIND,
            Self::NavHalt(_) => NAV_HALT_TRANSACTION_KIND,
            Self::NavProfileRegister(_) => NAV_PROFILE_REGISTER_TRANSACTION_KIND,
            Self::NavRedeemSettle(_) => NAV_REDEEM_SETTLE_TRANSACTION_KIND,
            Self::NavReserveAttest(_) => NAV_RESERVE_ATTEST_TRANSACTION_KIND,
            Self::NavAttestorRegister(_) => NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
            Self::VaultBridgeDepositPropose(_) => VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
            Self::VaultBridgeDepositChallenge(_) => VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND,
            Self::VaultBridgeDepositAttest(_) => VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
            Self::VaultBridgeDepositFinalize(_) => VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
            Self::VaultBridgeDepositClaim(_) => VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
            Self::VaultBridgeReceiptSubmit(_) => VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND,
            Self::VaultBridgeReceiptCount(_) => VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND,
            Self::VaultBridgeMintFromReceipts(_) => {
                VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND
            }
            Self::VaultBridgeBurnToRedeem(_) => VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
            Self::VaultBridgeRedeemSettle(_) => VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND,
            Self::VaultBridgeBucketImpair(_) => VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND,
            Self::VaultBridgeNavSubscriptionAllocate(_) => {
                VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND
            }
            Self::PftlUniswapRouteInit(_) => PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND,
            Self::PftlUniswapPrimarySubscribe(_) => PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND,
            Self::PftlUniswapExportDebit(_) => PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND,
            Self::PftlUniswapDestinationConsume(_) => {
                PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND
            }
            Self::PftlUniswapRefundSource(_) => PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND,
            Self::PftlUniswapReturnImport(_) => PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::AssetCreate(operation) => operation.validate(),
            Self::TrustSet(operation) => operation.validate(),
            Self::IssuedPayment(operation) => operation.validate(),
            Self::AssetBurn(operation) => operation.validate(),
            Self::AssetClawback(operation) => operation.validate(),
            Self::NavAssetRegister(operation) => operation.validate(),
            Self::NavReserveSubmit(operation) => operation.validate(),
            Self::NavReserveChallenge(operation) => operation.validate(),
            Self::NavEpochFinalize(operation) => operation.validate(),
            Self::MarketOpsPolicyRegister(operation) => operation.validate(),
            Self::MarketOpsFinalize(operation) => operation.validate(),
            Self::NavMintAtNav(operation) => operation.validate(),
            Self::NavRedeemAtNav(operation) => operation.validate(),
            Self::NavHalt(operation) => operation.validate(),
            Self::NavProfileRegister(operation) => operation.validate(),
            Self::NavRedeemSettle(operation) => operation.validate(),
            Self::NavReserveAttest(operation) => operation.validate(),
            Self::NavAttestorRegister(operation) => operation.validate(),
            Self::VaultBridgeDepositPropose(operation) => operation.validate(),
            Self::VaultBridgeDepositChallenge(operation) => operation.validate(),
            Self::VaultBridgeDepositAttest(operation) => operation.validate(),
            Self::VaultBridgeDepositFinalize(operation) => operation.validate(),
            Self::VaultBridgeDepositClaim(operation) => operation.validate(),
            Self::VaultBridgeReceiptSubmit(operation) => operation.validate(),
            Self::VaultBridgeReceiptCount(operation) => operation.validate(),
            Self::VaultBridgeMintFromReceipts(operation) => operation.validate(),
            Self::VaultBridgeBurnToRedeem(operation) => operation.validate(),
            Self::VaultBridgeRedeemSettle(operation) => operation.validate(),
            Self::VaultBridgeBucketImpair(operation) => operation.validate(),
            Self::VaultBridgeNavSubscriptionAllocate(operation) => operation.validate(),
            Self::PftlUniswapRouteInit(operation) => operation.validate(),
            Self::PftlUniswapPrimarySubscribe(operation) => operation.validate(),
            Self::PftlUniswapExportDebit(operation) => operation.validate(),
            Self::PftlUniswapDestinationConsume(operation) => operation.validate(),
            Self::PftlUniswapRefundSource(operation) => operation.validate(),
            Self::PftlUniswapReturnImport(operation) => operation.validate(),
        }
    }

    fn source_matches(&self, source: &str) -> bool {
        self.source_matches_with_legacy_vault_bridge_consume_supply_operator(source, false)
    }

    fn source_matches_with_legacy_vault_bridge_consume_supply_operator(
        &self,
        source: &str,
        allow_legacy_vault_bridge_consume_supply_operator: bool,
    ) -> bool {
        match self {
            Self::AssetCreate(operation) => operation.issuer == source,
            Self::TrustSet(operation) => operation.account == source || operation.issuer == source,
            Self::IssuedPayment(operation) => operation.from == source,
            Self::AssetBurn(operation) => operation.owner == source,
            Self::AssetClawback(operation) => operation.issuer == source,
            Self::NavAssetRegister(operation) => operation.issuer == source,
            Self::NavReserveSubmit(operation) => operation.submitter == source,
            Self::NavReserveChallenge(operation) => operation.challenger == source,
            Self::NavEpochFinalize(operation) => operation.issuer == source,
            Self::MarketOpsPolicyRegister(operation) => operation.issuer == source,
            Self::MarketOpsFinalize(operation) => operation.issuer == source,
            Self::NavMintAtNav(operation) => operation.issuer == source,
            Self::NavRedeemAtNav(operation) => operation.owner == source,
            Self::NavHalt(operation) => operation.issuer == source,
            Self::NavProfileRegister(operation) => operation.registrant == source,
            Self::NavRedeemSettle(operation) => operation.issuer == source,
            Self::NavReserveAttest(operation) => operation.attestor == source,
            Self::NavAttestorRegister(operation) => operation.attestor == source,
            Self::VaultBridgeDepositPropose(operation) => operation.proposer == source,
            Self::VaultBridgeDepositChallenge(operation) => operation.challenger == source,
            Self::VaultBridgeDepositAttest(operation) => operation.attestor == source,
            Self::VaultBridgeDepositFinalize(operation) => operation.finalizer == source,
            Self::VaultBridgeDepositClaim(operation) => operation.claimer == source,
            Self::VaultBridgeReceiptSubmit(operation) => operation.operator == source,
            Self::VaultBridgeReceiptCount(operation) => operation.operator == source,
            Self::VaultBridgeMintFromReceipts(operation) => operation.issuer == source,
            Self::VaultBridgeBurnToRedeem(operation) => operation.owner == source,
            Self::VaultBridgeRedeemSettle(operation) => {
                operation.issuer_or_redemption_account == source
            }
            Self::VaultBridgeBucketImpair(operation) => operation.operator == source,
            Self::VaultBridgeNavSubscriptionAllocate(operation) => {
                if allow_legacy_vault_bridge_consume_supply_operator
                    && operation.consume_supply_owner.is_some()
                    && operation.operator == source
                {
                    true
                } else {
                    operation
                        .consume_supply_owner
                        .as_ref()
                        .map_or(operation.operator == source, |owner| owner == source)
                }
            }
            Self::PftlUniswapRouteInit(operation) => operation.operator == source,
            Self::PftlUniswapPrimarySubscribe(operation) => operation.subscriber == source,
            Self::PftlUniswapExportDebit(operation) => operation.owner == source,
            Self::PftlUniswapDestinationConsume(operation) => operation.operator == source,
            Self::PftlUniswapRefundSource(operation) => operation.operator == source,
            Self::PftlUniswapReturnImport(operation) => operation.operator == source,
        }
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("operation={}\n", self.transaction_kind()).into_bytes();
        match self {
            Self::AssetCreate(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::TrustSet(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::IssuedPayment(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::AssetBurn(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::AssetClawback(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::NavAssetRegister(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::NavReserveSubmit(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::NavReserveChallenge(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::NavEpochFinalize(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::MarketOpsPolicyRegister(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::MarketOpsFinalize(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::NavMintAtNav(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::NavRedeemAtNav(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::NavHalt(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::NavProfileRegister(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::NavRedeemSettle(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::NavReserveAttest(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::NavAttestorRegister(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeDepositPropose(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeDepositChallenge(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeDepositAttest(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeDepositFinalize(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeDepositClaim(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeReceiptSubmit(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeReceiptCount(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeMintFromReceipts(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeBurnToRedeem(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeRedeemSettle(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeBucketImpair(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::VaultBridgeNavSubscriptionAllocate(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::PftlUniswapRouteInit(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::PftlUniswapPrimarySubscribe(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::PftlUniswapExportDebit(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::PftlUniswapDestinationConsume(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::PftlUniswapRefundSource(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
            Self::PftlUniswapReturnImport(operation) => {
                bytes.extend_from_slice(&operation.signing_bytes())
            }
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedAssetTransaction {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address_namespace: String,
    pub transaction_kind: String,
    pub signature_algorithm_id: String,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    #[serde(flatten)]
    pub operation: AssetTransactionOperation,
}

impl UnsignedAssetTransaction {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_with_legacy_vault_bridge_consume_supply_operator(false)
    }

    pub fn validate_with_legacy_vault_bridge_consume_supply_operator(
        &self,
        allow_legacy_vault_bridge_consume_supply_operator: bool,
    ) -> Result<(), String> {
        validate_text_field("asset_tx.chain_id", &self.chain_id)?;
        validate_lower_hex_len("asset_tx.genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err("asset_tx.protocol_version must be nonzero".to_string());
        }
        validate_text_field("asset_tx.address_namespace", &self.address_namespace)?;
        validate_text_field("asset_tx.transaction_kind", &self.transaction_kind)?;
        if self.transaction_kind != self.operation.transaction_kind() {
            return Err(format!(
                "asset_tx.transaction_kind must match operation `{}`",
                self.operation.transaction_kind()
            ));
        }
        validate_text_field(
            "asset_tx.signature_algorithm_id",
            &self.signature_algorithm_id,
        )?;
        validate_text_field("asset_tx.source", &self.source)?;
        self.operation.validate()?;
        let source_matches = if allow_legacy_vault_bridge_consume_supply_operator {
            self.operation
                .source_matches_with_legacy_vault_bridge_consume_supply_operator(&self.source, true)
        } else {
            self.operation.source_matches(&self.source)
        };
        if !source_matches {
            return Err("asset_tx.source is not authorized by operation fields".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "postfiat.asset_transaction.v1\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id={}\nsource={}\nfee={}\nsequence={}\n",
            self.chain_id,
            self.genesis_hash,
            self.protocol_version,
            self.address_namespace,
            self.transaction_kind,
            self.signature_algorithm_id,
            self.source,
            self.fee,
            self.sequence
        )
        .into_bytes();
        out.extend_from_slice(&self.operation.signing_bytes());
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedAssetTransaction {
    pub unsigned: UnsignedAssetTransaction,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl SignedAssetTransaction {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_with_legacy_vault_bridge_consume_supply_operator(false)
    }

    pub fn validate_with_legacy_vault_bridge_consume_supply_operator(
        &self,
        allow_legacy_vault_bridge_consume_supply_operator: bool,
    ) -> Result<(), String> {
        self.unsigned
            .validate_with_legacy_vault_bridge_consume_supply_operator(
                allow_legacy_vault_bridge_consume_supply_operator,
            )?;
        validate_text_field("asset_tx.algorithm_id", &self.algorithm_id)?;
        validate_lower_hex_max(
            "asset_tx.public_key_hex",
            &self.public_key_hex,
            MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
        )?;
        validate_lower_hex_max(
            "asset_tx.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )?;
        Ok(())
    }

    pub fn tx_id_preimage_bytes(&self) -> Vec<u8> {
        let mut bytes = self.unsigned.signing_bytes();
        bytes.extend_from_slice(b"algorithm=");
        bytes.extend_from_slice(self.algorithm_id.as_bytes());
        bytes.extend_from_slice(b"\npublic_key=");
        bytes.extend_from_slice(self.public_key_hex.as_bytes());
        bytes.extend_from_slice(b"\nsignature=");
        bytes.extend_from_slice(self.signature_hex.as_bytes());
        bytes.extend_from_slice(b"\n");
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSwapLeg {
    pub owner: String,
    pub recipient: String,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
    pub sequence: u64,
    pub fee: u64,
}

impl AtomicSwapLeg {
    fn validate(&self, field: &str) -> Result<(), String> {
        validate_postfiat_address(&format!("{field}.owner"), &self.owner)?;
        validate_postfiat_address(&format!("{field}.recipient"), &self.recipient)?;
        validate_postfiat_address(&format!("{field}.issuer"), &self.issuer)?;
        validate_lower_hex_len(
            &format!("{field}.asset_id"),
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.amount == 0 {
            return Err(format!("{field}.amount must be nonzero"));
        }
        if self.owner == self.issuer || self.recipient == self.issuer {
            return Err(format!(
                "{field} issuer legs are not supported in atomic swap v1"
            ));
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "owner={}\nrecipient={}\nissuer={}\nasset_id={}\namount={}\nsequence={}\nfee={}\n",
            self.owner,
            self.recipient,
            self.issuer,
            self.asset_id,
            self.amount,
            self.sequence,
            self.fee
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedAtomicSwapTransaction {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address_namespace: String,
    pub signature_algorithm_id: String,
    pub rfq_hash: String,
    pub market_envelope_hash: String,
    pub nav_epoch: u64,
    pub expires_at_height: u64,
    pub swap_nonce: String,
    pub leg_0: AtomicSwapLeg,
    pub leg_1: AtomicSwapLeg,
}

impl UnsignedAtomicSwapTransaction {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("atomic_swap.chain_id", &self.chain_id)?;
        validate_lower_hex_len("atomic_swap.genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err("atomic_swap.protocol_version must be nonzero".to_string());
        }
        validate_text_field("atomic_swap.address_namespace", &self.address_namespace)?;
        validate_text_field(
            "atomic_swap.signature_algorithm_id",
            &self.signature_algorithm_id,
        )?;
        validate_lower_hex_len("atomic_swap.rfq_hash", &self.rfq_hash, 96)?;
        validate_lower_hex_len(
            "atomic_swap.market_envelope_hash",
            &self.market_envelope_hash,
            96,
        )?;
        validate_lower_hex_len("atomic_swap.swap_nonce", &self.swap_nonce, 96)?;
        self.leg_0.validate("atomic_swap.leg_0")?;
        self.leg_1.validate("atomic_swap.leg_1")?;
        if self.leg_0.owner == self.leg_1.owner {
            return Err("atomic_swap owners must differ".to_string());
        }
        if self.leg_0.owner != self.leg_1.recipient || self.leg_1.owner != self.leg_0.recipient {
            return Err("atomic_swap legs must be reciprocal".to_string());
        }
        if self.leg_0.asset_id == self.leg_1.asset_id {
            return Err("atomic_swap assets must differ".to_string());
        }
        if (&self.leg_0.asset_id, &self.leg_0.owner) >= (&self.leg_1.asset_id, &self.leg_1.owner) {
            return Err("atomic_swap legs must use canonical (asset_id, owner) order".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        // The v1 signing layout retains the exact ruled `market_policy_hash`
        // key label even though the wire field names its envelope-hash value.
        let mut bytes = format!(
            "{}\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\nsignature_algorithm_id={}\nrfq_hash={}\nmarket_policy_hash={}\nnav_epoch={}\nexpires_at_height={}\nswap_nonce={}\nleg_0=\n",
            ATOMIC_SWAP_TRANSACTION_SIGNING_DOMAIN,
            self.chain_id,
            self.genesis_hash,
            self.protocol_version,
            self.address_namespace,
            self.signature_algorithm_id,
            self.rfq_hash,
            self.market_envelope_hash,
            self.nav_epoch,
            self.expires_at_height,
            self.swap_nonce
        )
        .into_bytes();
        bytes.extend_from_slice(&self.leg_0.signing_bytes());
        bytes.extend_from_slice(b"leg_1=\n");
        bytes.extend_from_slice(&self.leg_1.signing_bytes());
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSwapAuthorization {
    pub owner: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl AtomicSwapAuthorization {
    fn validate(&self, field: &str) -> Result<(), String> {
        validate_postfiat_address(&format!("{field}.owner"), &self.owner)?;
        validate_text_field(&format!("{field}.algorithm_id"), &self.algorithm_id)?;
        validate_lower_hex_max(
            &format!("{field}.public_key_hex"),
            &self.public_key_hex,
            MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
        )?;
        validate_lower_hex_max(
            &format!("{field}.signature_hex"),
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )
    }

    fn append_tx_id_preimage_bytes(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(b"algorithm=");
        bytes.extend_from_slice(self.algorithm_id.as_bytes());
        bytes.extend_from_slice(b"\npublic_key=");
        bytes.extend_from_slice(self.public_key_hex.as_bytes());
        bytes.extend_from_slice(b"\nsignature=");
        bytes.extend_from_slice(self.signature_hex.as_bytes());
        bytes.extend_from_slice(b"\n");
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedAtomicSwapTransaction {
    pub unsigned: UnsignedAtomicSwapTransaction,
    pub authorization_0: AtomicSwapAuthorization,
    pub authorization_1: AtomicSwapAuthorization,
}

impl SignedAtomicSwapTransaction {
    pub fn validate(&self) -> Result<(), String> {
        self.unsigned.validate()?;
        self.authorization_0
            .validate("atomic_swap.authorization_0")?;
        self.authorization_1
            .validate("atomic_swap.authorization_1")?;
        if self.authorization_0.owner != self.unsigned.leg_0.owner {
            return Err("atomic_swap.authorization_0 owner must match leg_0 owner".to_string());
        }
        if self.authorization_1.owner != self.unsigned.leg_1.owner {
            return Err("atomic_swap.authorization_1 owner must match leg_1 owner".to_string());
        }
        if self.authorization_0.algorithm_id != self.unsigned.signature_algorithm_id
            || self.authorization_1.algorithm_id != self.unsigned.signature_algorithm_id
        {
            return Err(
                "atomic_swap authorization algorithms must match signature_algorithm_id"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub fn tx_id_preimage_bytes(&self) -> Vec<u8> {
        let mut bytes = self.unsigned.signing_bytes();
        self.authorization_0.append_tx_id_preimage_bytes(&mut bytes);
        self.authorization_1.append_tx_id_preimage_bytes(&mut bytes);
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowCreateOperation {
    pub owner: String,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub condition: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub finish_after: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub cancel_after: u64,
}

impl EscrowCreateOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("escrow_create.owner", &self.owner)?;
        validate_text_field("escrow_create.recipient", &self.recipient)?;
        if self.owner == self.recipient {
            return Err("escrow_create.owner must differ from recipient".to_string());
        }
        validate_escrow_asset_id(&self.asset_id)?;
        if self.amount == 0 {
            return Err("escrow_create.amount must be nonzero".to_string());
        }
        validate_optional_text_field(
            "escrow_create.condition",
            &self.condition,
            MAX_ESCROW_CONDITION_BYTES,
        )?;
        if self.condition.is_empty() && self.finish_after == 0 && self.cancel_after == 0 {
            return Err(
                "escrow_create must declare condition, finish_after, or cancel_after".to_string(),
            );
        }
        if self.finish_after != 0
            && self.cancel_after != 0
            && self.cancel_after <= self.finish_after
        {
            return Err("escrow_create.cancel_after must be greater than finish_after".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "owner={}\nrecipient={}\nasset_id={}\namount={}\ncondition_bytes={}\ncondition={}\nfinish_after={}\ncancel_after={}\n",
            self.owner,
            self.recipient,
            self.asset_id,
            self.amount,
            self.condition.len(),
            self.condition,
            self.finish_after,
            self.cancel_after
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowFinishOperation {
    pub escrow_id: String,
    pub owner: String,
    pub recipient: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fulfillment: String,
}

impl EscrowFinishOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "escrow_finish.escrow_id",
            &self.escrow_id,
            ESCROW_ID_HEX_LEN,
        )?;
        validate_text_field("escrow_finish.owner", &self.owner)?;
        validate_text_field("escrow_finish.recipient", &self.recipient)?;
        if self.owner == self.recipient {
            return Err("escrow_finish.owner must differ from recipient".to_string());
        }
        validate_optional_text_field(
            "escrow_finish.fulfillment",
            &self.fulfillment,
            MAX_ESCROW_FULFILLMENT_BYTES,
        )?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "escrow_id={}\nowner={}\nrecipient={}\nfulfillment_bytes={}\nfulfillment={}\n",
            self.escrow_id,
            self.owner,
            self.recipient,
            self.fulfillment.len(),
            self.fulfillment
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowCancelOperation {
    pub escrow_id: String,
    pub owner: String,
}

impl EscrowCancelOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "escrow_cancel.escrow_id",
            &self.escrow_id,
            ESCROW_ID_HEX_LEN,
        )?;
        validate_text_field("escrow_cancel.owner", &self.owner)?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!("escrow_id={}\nowner={}\n", self.escrow_id, self.owner).into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum EscrowTransactionOperation {
    #[serde(rename = "escrow_create")]
    EscrowCreate(EscrowCreateOperation),
    #[serde(rename = "escrow_finish")]
    EscrowFinish(EscrowFinishOperation),
    #[serde(rename = "escrow_cancel")]
    EscrowCancel(EscrowCancelOperation),
}

impl EscrowTransactionOperation {
    pub fn transaction_kind(&self) -> &'static str {
        match self {
            Self::EscrowCreate(_) => ESCROW_CREATE_TRANSACTION_KIND,
            Self::EscrowFinish(_) => ESCROW_FINISH_TRANSACTION_KIND,
            Self::EscrowCancel(_) => ESCROW_CANCEL_TRANSACTION_KIND,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::EscrowCreate(operation) => operation.validate(),
            Self::EscrowFinish(operation) => operation.validate(),
            Self::EscrowCancel(operation) => operation.validate(),
        }
    }

    fn source_matches(&self, source: &str) -> bool {
        match self {
            Self::EscrowCreate(operation) => operation.owner == source,
            Self::EscrowFinish(operation) => operation.recipient == source,
            Self::EscrowCancel(operation) => operation.owner == source,
        }
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("operation={}\n", self.transaction_kind()).into_bytes();
        match self {
            Self::EscrowCreate(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::EscrowFinish(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::EscrowCancel(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedEscrowTransaction {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address_namespace: String,
    pub transaction_kind: String,
    pub signature_algorithm_id: String,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    #[serde(flatten)]
    pub operation: EscrowTransactionOperation,
}

impl UnsignedEscrowTransaction {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("escrow_tx.chain_id", &self.chain_id)?;
        validate_lower_hex_len("escrow_tx.genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err("escrow_tx.protocol_version must be nonzero".to_string());
        }
        validate_text_field("escrow_tx.address_namespace", &self.address_namespace)?;
        validate_text_field("escrow_tx.transaction_kind", &self.transaction_kind)?;
        if self.transaction_kind != self.operation.transaction_kind() {
            return Err(format!(
                "escrow_tx.transaction_kind must match operation `{}`",
                self.operation.transaction_kind()
            ));
        }
        validate_text_field(
            "escrow_tx.signature_algorithm_id",
            &self.signature_algorithm_id,
        )?;
        validate_text_field("escrow_tx.source", &self.source)?;
        self.operation.validate()?;
        if !self.operation.source_matches(&self.source) {
            return Err("escrow_tx.source is not authorized by operation fields".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "postfiat.escrow_transaction.v1\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id={}\nsource={}\nfee={}\nsequence={}\n",
            self.chain_id,
            self.genesis_hash,
            self.protocol_version,
            self.address_namespace,
            self.transaction_kind,
            self.signature_algorithm_id,
            self.source,
            self.fee,
            self.sequence
        )
        .into_bytes();
        out.extend_from_slice(&self.operation.signing_bytes());
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedEscrowTransaction {
    pub unsigned: UnsignedEscrowTransaction,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl SignedEscrowTransaction {
    pub fn validate(&self) -> Result<(), String> {
        self.unsigned.validate()?;
        validate_text_field("escrow_tx.algorithm_id", &self.algorithm_id)?;
        validate_lower_hex_max(
            "escrow_tx.public_key_hex",
            &self.public_key_hex,
            MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
        )?;
        validate_lower_hex_max(
            "escrow_tx.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )?;
        Ok(())
    }

    pub fn tx_id_preimage_bytes(&self) -> Vec<u8> {
        let mut bytes = self.unsigned.signing_bytes();
        bytes.extend_from_slice(b"algorithm=");
        bytes.extend_from_slice(self.algorithm_id.as_bytes());
        bytes.extend_from_slice(b"\npublic_key=");
        bytes.extend_from_slice(self.public_key_hex.as_bytes());
        bytes.extend_from_slice(b"\nsignature=");
        bytes.extend_from_slice(self.signature_hex.as_bytes());
        bytes.extend_from_slice(b"\n");
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftMintOperation {
    pub issuer: String,
    pub collection_id: String,
    pub serial: u64,
    pub owner: String,
    pub metadata_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub metadata_uri: String,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub flags: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub collection_flags: u32,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub issuer_transfer_fee: u64,
}

impl NftMintOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nft_mint.issuer", &self.issuer)?;
        validate_nft_collection_id(&self.collection_id)?;
        if self.serial == 0 {
            return Err("nft_mint.serial must be nonzero".to_string());
        }
        validate_text_field("nft_mint.owner", &self.owner)?;
        validate_lower_hex_max(
            "nft_mint.metadata_hash",
            &self.metadata_hash,
            MAX_NFT_METADATA_HASH_BYTES * 2,
        )?;
        validate_optional_text_field(
            "nft_mint.metadata_uri",
            &self.metadata_uri,
            MAX_NFT_METADATA_URI_BYTES,
        )?;
        if self.flags & !NFT_ALLOWED_FLAGS != 0 {
            return Err("nft_mint.flags contains unsupported bits".to_string());
        }
        if self.collection_flags & !NFT_COLLECTION_ALLOWED_FLAGS != 0 {
            return Err("nft_mint.collection_flags contains unsupported bits".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "issuer={}\ncollection_id_bytes={}\ncollection_id={}\nserial={}\nowner={}\nmetadata_hash_bytes={}\nmetadata_hash={}\nmetadata_uri_bytes={}\nmetadata_uri={}\nflags={}\n",
            self.issuer,
            self.collection_id.len(),
            self.collection_id,
            self.serial,
            self.owner,
            hex_encoded_byte_len(&self.metadata_hash),
            self.metadata_hash,
            self.metadata_uri.len(),
            self.metadata_uri,
            self.flags
        )
        .into_bytes();
        if self.collection_flags != 0 {
            bytes.extend_from_slice(
                format!("collection_flags={}\n", self.collection_flags).as_bytes(),
            );
        }
        if self.issuer_transfer_fee != 0 {
            bytes.extend_from_slice(
                format!("issuer_transfer_fee={}\n", self.issuer_transfer_fee).as_bytes(),
            );
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftTransferOperation {
    pub nft_id: String,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub issuer: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub issuer_transfer_fee: u64,
}

impl NftTransferOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("nft_transfer.nft_id", &self.nft_id, NFT_ID_HEX_LEN)?;
        validate_text_field("nft_transfer.from", &self.from)?;
        validate_text_field("nft_transfer.to", &self.to)?;
        if self.from == self.to {
            return Err("nft_transfer.from must differ from nft_transfer.to".to_string());
        }
        if !self.issuer.is_empty() {
            validate_text_field("nft_transfer.issuer", &self.issuer)?;
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!(
            "nft_id={}\nfrom={}\nto={}\n",
            self.nft_id, self.from, self.to
        )
        .into_bytes();
        if !self.issuer.is_empty() {
            bytes.extend_from_slice(format!("issuer={}\n", self.issuer).as_bytes());
        }
        if self.issuer_transfer_fee != 0 {
            bytes.extend_from_slice(
                format!("issuer_transfer_fee={}\n", self.issuer_transfer_fee).as_bytes(),
            );
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftBurnOperation {
    pub nft_id: String,
    pub owner: String,
}

impl NftBurnOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("nft_burn.nft_id", &self.nft_id, NFT_ID_HEX_LEN)?;
        validate_text_field("nft_burn.owner", &self.owner)?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!("nft_id={}\nowner={}\n", self.nft_id, self.owner).into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum NftTransactionOperation {
    #[serde(rename = "nft_mint")]
    NftMint(NftMintOperation),
    #[serde(rename = "nft_transfer")]
    NftTransfer(NftTransferOperation),
    #[serde(rename = "nft_burn")]
    NftBurn(NftBurnOperation),
}

impl NftTransactionOperation {
    pub fn transaction_kind(&self) -> &'static str {
        match self {
            Self::NftMint(_) => NFT_MINT_TRANSACTION_KIND,
            Self::NftTransfer(_) => NFT_TRANSFER_TRANSACTION_KIND,
            Self::NftBurn(_) => NFT_BURN_TRANSACTION_KIND,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::NftMint(operation) => operation.validate(),
            Self::NftTransfer(operation) => operation.validate(),
            Self::NftBurn(operation) => operation.validate(),
        }
    }

    fn source_matches(&self, source: &str) -> bool {
        match self {
            Self::NftMint(operation) => operation.issuer == source,
            Self::NftTransfer(operation) => operation.from == source,
            Self::NftBurn(operation) => operation.owner == source,
        }
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("operation={}\n", self.transaction_kind()).into_bytes();
        match self {
            Self::NftMint(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::NftTransfer(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::NftBurn(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedNftTransaction {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address_namespace: String,
    pub transaction_kind: String,
    pub signature_algorithm_id: String,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    #[serde(flatten)]
    pub operation: NftTransactionOperation,
}

impl UnsignedNftTransaction {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("nft_tx.chain_id", &self.chain_id)?;
        validate_lower_hex_len("nft_tx.genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err("nft_tx.protocol_version must be nonzero".to_string());
        }
        validate_text_field("nft_tx.address_namespace", &self.address_namespace)?;
        validate_text_field("nft_tx.transaction_kind", &self.transaction_kind)?;
        if self.transaction_kind != self.operation.transaction_kind() {
            return Err(format!(
                "nft_tx.transaction_kind must match operation `{}`",
                self.operation.transaction_kind()
            ));
        }
        validate_text_field(
            "nft_tx.signature_algorithm_id",
            &self.signature_algorithm_id,
        )?;
        validate_text_field("nft_tx.source", &self.source)?;
        self.operation.validate()?;
        if !self.operation.source_matches(&self.source) {
            return Err("nft_tx.source is not authorized by operation fields".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "postfiat.nft_transaction.v1\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id={}\nsource={}\nfee={}\nsequence={}\n",
            self.chain_id,
            self.genesis_hash,
            self.protocol_version,
            self.address_namespace,
            self.transaction_kind,
            self.signature_algorithm_id,
            self.source,
            self.fee,
            self.sequence
        )
        .into_bytes();
        out.extend_from_slice(&self.operation.signing_bytes());
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedNftTransaction {
    pub unsigned: UnsignedNftTransaction,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl SignedNftTransaction {
    pub fn validate(&self) -> Result<(), String> {
        self.unsigned.validate()?;
        validate_text_field("nft_tx.algorithm_id", &self.algorithm_id)?;
        validate_lower_hex_max(
            "nft_tx.public_key_hex",
            &self.public_key_hex,
            MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
        )?;
        validate_lower_hex_max(
            "nft_tx.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )?;
        Ok(())
    }

    pub fn tx_id_preimage_bytes(&self) -> Vec<u8> {
        let mut bytes = self.unsigned.signing_bytes();
        bytes.extend_from_slice(b"algorithm=");
        bytes.extend_from_slice(self.algorithm_id.as_bytes());
        bytes.extend_from_slice(b"\npublic_key=");
        bytes.extend_from_slice(self.public_key_hex.as_bytes());
        bytes.extend_from_slice(b"\nsignature=");
        bytes.extend_from_slice(self.signature_hex.as_bytes());
        bytes.extend_from_slice(b"\n");
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferCreateOperation {
    pub owner: String,
    pub taker_gets_asset_id: String,
    pub taker_gets_amount: u64,
    pub taker_pays_asset_id: String,
    pub taker_pays_amount: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub expiration_height: u64,
}

impl OfferCreateOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("offer_create.owner", &self.owner)?;
        validate_dex_asset_id(
            "offer_create.taker_gets_asset_id",
            &self.taker_gets_asset_id,
        )?;
        validate_dex_asset_id(
            "offer_create.taker_pays_asset_id",
            &self.taker_pays_asset_id,
        )?;
        if self.taker_gets_asset_id == self.taker_pays_asset_id {
            return Err("offer_create assets must differ".to_string());
        }
        if self.taker_gets_amount == 0 {
            return Err("offer_create.taker_gets_amount must be nonzero".to_string());
        }
        if self.taker_pays_amount == 0 {
            return Err("offer_create.taker_pays_amount must be nonzero".to_string());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "owner={}\ntaker_gets_asset_id={}\ntaker_gets_amount={}\ntaker_pays_asset_id={}\ntaker_pays_amount={}\nexpiration_height={}\n",
            self.owner,
            self.taker_gets_asset_id,
            self.taker_gets_amount,
            self.taker_pays_asset_id,
            self.taker_pays_amount,
            self.expiration_height
        )
        .into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferCancelOperation {
    pub offer_id: String,
    pub owner: String,
}

impl OfferCancelOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("offer_cancel.offer_id", &self.offer_id, OFFER_ID_HEX_LEN)?;
        validate_text_field("offer_cancel.owner", &self.owner)?;
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        format!("offer_id={}\nowner={}\n", self.offer_id, self.owner).into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum OfferTransactionOperation {
    #[serde(rename = "offer_create")]
    OfferCreate(OfferCreateOperation),
    #[serde(rename = "offer_cancel")]
    OfferCancel(OfferCancelOperation),
}

impl OfferTransactionOperation {
    pub fn transaction_kind(&self) -> &'static str {
        match self {
            Self::OfferCreate(_) => OFFER_CREATE_TRANSACTION_KIND,
            Self::OfferCancel(_) => OFFER_CANCEL_TRANSACTION_KIND,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::OfferCreate(operation) => operation.validate(),
            Self::OfferCancel(operation) => operation.validate(),
        }
    }

    fn source_matches(&self, source: &str) -> bool {
        match self {
            Self::OfferCreate(operation) => operation.owner == source,
            Self::OfferCancel(operation) => operation.owner == source,
        }
    }

    fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = format!("operation={}\n", self.transaction_kind()).into_bytes();
        match self {
            Self::OfferCreate(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
            Self::OfferCancel(operation) => bytes.extend_from_slice(&operation.signing_bytes()),
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsignedOfferTransaction {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address_namespace: String,
    pub transaction_kind: String,
    pub signature_algorithm_id: String,
    pub source: String,
    pub fee: u64,
    pub sequence: u64,
    #[serde(flatten)]
    pub operation: OfferTransactionOperation,
}

impl UnsignedOfferTransaction {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("offer_tx.chain_id", &self.chain_id)?;
        validate_lower_hex_len("offer_tx.genesis_hash", &self.genesis_hash, 96)?;
        if self.protocol_version == 0 {
            return Err("offer_tx.protocol_version must be nonzero".to_string());
        }
        validate_text_field("offer_tx.address_namespace", &self.address_namespace)?;
        validate_text_field("offer_tx.transaction_kind", &self.transaction_kind)?;
        if self.transaction_kind != self.operation.transaction_kind() {
            return Err(format!(
                "offer_tx.transaction_kind must match operation `{}`",
                self.operation.transaction_kind()
            ));
        }
        validate_text_field(
            "offer_tx.signature_algorithm_id",
            &self.signature_algorithm_id,
        )?;
        validate_text_field("offer_tx.source", &self.source)?;
        self.operation.validate()?;
        if !self.operation.source_matches(&self.source) {
            return Err("offer_tx.source is not authorized by operation fields".to_string());
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = format!(
            "postfiat.offer_transaction.v1\nchain_id={}\ngenesis_hash={}\nprotocol_version={}\naddress_namespace={}\ntransaction_kind={}\nsignature_algorithm_id={}\nsource={}\nfee={}\nsequence={}\n",
            self.chain_id,
            self.genesis_hash,
            self.protocol_version,
            self.address_namespace,
            self.transaction_kind,
            self.signature_algorithm_id,
            self.source,
            self.fee,
            self.sequence
        )
        .into_bytes();
        out.extend_from_slice(&self.operation.signing_bytes());
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedOfferTransaction {
    pub unsigned: UnsignedOfferTransaction,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl SignedOfferTransaction {
    pub fn validate(&self) -> Result<(), String> {
        self.unsigned.validate()?;
        validate_text_field("offer_tx.algorithm_id", &self.algorithm_id)?;
        validate_lower_hex_max(
            "offer_tx.public_key_hex",
            &self.public_key_hex,
            MAX_TRANSFER_PUBLIC_KEY_HEX_LEN,
        )?;
        validate_lower_hex_max(
            "offer_tx.signature_hex",
            &self.signature_hex,
            MAX_TRANSFER_SIGNATURE_HEX_LEN,
        )?;
        Ok(())
    }

    pub fn tx_id_preimage_bytes(&self) -> Vec<u8> {
        let mut bytes = self.unsigned.signing_bytes();
        bytes.extend_from_slice(b"algorithm=");
        bytes.extend_from_slice(self.algorithm_id.as_bytes());
        bytes.extend_from_slice(b"\npublic_key=");
        bytes.extend_from_slice(self.public_key_hex.as_bytes());
        bytes.extend_from_slice(b"\nsignature=");
        bytes.extend_from_slice(self.signature_hex.as_bytes());
        bytes.extend_from_slice(b"\n");
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionBatch {
    pub batch_id: String,
    #[serde(default)]
    pub transactions: Vec<SignedTransfer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub payments_v2: Vec<SignedPaymentV2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_transactions: Vec<SignedAssetTransaction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub atomic_swap_transactions: Vec<SignedAtomicSwapTransaction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastlane_primary_transactions: Vec<FastLanePrimaryTransactionV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escrow_transactions: Vec<SignedEscrowTransaction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nft_transactions: Vec<SignedNftTransaction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offer_transactions: Vec<SignedOfferTransaction>,
}

impl TransactionBatch {
    pub fn new(batch_id: impl Into<String>, transactions: Vec<SignedTransfer>) -> Self {
        Self {
            batch_id: batch_id.into(),
            transactions,
            payments_v2: Vec::new(),
            asset_transactions: Vec::new(),
            atomic_swap_transactions: Vec::new(),
            fastlane_primary_transactions: Vec::new(),
            escrow_transactions: Vec::new(),
            nft_transactions: Vec::new(),
            offer_transactions: Vec::new(),
        }
    }

    pub fn new_with_payments_v2(
        batch_id: impl Into<String>,
        transactions: Vec<SignedTransfer>,
        payments_v2: Vec<SignedPaymentV2>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            transactions,
            payments_v2,
            asset_transactions: Vec::new(),
            atomic_swap_transactions: Vec::new(),
            fastlane_primary_transactions: Vec::new(),
            escrow_transactions: Vec::new(),
            nft_transactions: Vec::new(),
            offer_transactions: Vec::new(),
        }
    }

    pub fn new_with_asset_transactions(
        batch_id: impl Into<String>,
        transactions: Vec<SignedTransfer>,
        payments_v2: Vec<SignedPaymentV2>,
        asset_transactions: Vec<SignedAssetTransaction>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            transactions,
            payments_v2,
            asset_transactions,
            atomic_swap_transactions: Vec::new(),
            fastlane_primary_transactions: Vec::new(),
            escrow_transactions: Vec::new(),
            nft_transactions: Vec::new(),
            offer_transactions: Vec::new(),
        }
    }

    pub fn new_with_escrow_transactions(
        batch_id: impl Into<String>,
        transactions: Vec<SignedTransfer>,
        payments_v2: Vec<SignedPaymentV2>,
        asset_transactions: Vec<SignedAssetTransaction>,
        escrow_transactions: Vec<SignedEscrowTransaction>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            transactions,
            payments_v2,
            asset_transactions,
            atomic_swap_transactions: Vec::new(),
            fastlane_primary_transactions: Vec::new(),
            escrow_transactions,
            nft_transactions: Vec::new(),
            offer_transactions: Vec::new(),
        }
    }

    pub fn new_with_nft_transactions(
        batch_id: impl Into<String>,
        transactions: Vec<SignedTransfer>,
        payments_v2: Vec<SignedPaymentV2>,
        asset_transactions: Vec<SignedAssetTransaction>,
        escrow_transactions: Vec<SignedEscrowTransaction>,
        nft_transactions: Vec<SignedNftTransaction>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            transactions,
            payments_v2,
            asset_transactions,
            atomic_swap_transactions: Vec::new(),
            fastlane_primary_transactions: Vec::new(),
            escrow_transactions,
            nft_transactions,
            offer_transactions: Vec::new(),
        }
    }

    pub fn new_with_offer_transactions(
        batch_id: impl Into<String>,
        transactions: Vec<SignedTransfer>,
        payments_v2: Vec<SignedPaymentV2>,
        asset_transactions: Vec<SignedAssetTransaction>,
        escrow_transactions: Vec<SignedEscrowTransaction>,
        nft_transactions: Vec<SignedNftTransaction>,
        offer_transactions: Vec<SignedOfferTransaction>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            transactions,
            payments_v2,
            asset_transactions,
            atomic_swap_transactions: Vec::new(),
            fastlane_primary_transactions: Vec::new(),
            escrow_transactions,
            nft_transactions,
            offer_transactions,
        }
    }

    pub fn new_with_fastlane_primary_transactions(
        batch_id: impl Into<String>,
        fastlane_primary_transactions: Vec<FastLanePrimaryTransactionV1>,
    ) -> Self {
        let mut batch = Self::new(batch_id, Vec::new());
        batch.fastlane_primary_transactions = fastlane_primary_transactions;
        batch
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_atomic_swap_transactions(
        batch_id: impl Into<String>,
        transactions: Vec<SignedTransfer>,
        payments_v2: Vec<SignedPaymentV2>,
        asset_transactions: Vec<SignedAssetTransaction>,
        atomic_swap_transactions: Vec<SignedAtomicSwapTransaction>,
        escrow_transactions: Vec<SignedEscrowTransaction>,
        nft_transactions: Vec<SignedNftTransaction>,
        offer_transactions: Vec<SignedOfferTransaction>,
    ) -> Self {
        Self {
            batch_id: batch_id.into(),
            transactions,
            payments_v2,
            asset_transactions,
            atomic_swap_transactions,
            fastlane_primary_transactions: Vec::new(),
            escrow_transactions,
            nft_transactions,
            offer_transactions,
        }
    }

    pub fn transaction_count(&self) -> usize {
        self.transactions
            .len()
            .saturating_add(self.payments_v2.len())
            .saturating_add(self.asset_transactions.len())
            .saturating_add(self.atomic_swap_transactions.len())
            .saturating_add(self.fastlane_primary_transactions.len())
            .saturating_add(self.escrow_transactions.len())
            .saturating_add(self.nft_transactions.len())
            .saturating_add(self.offer_transactions.len())
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
            && self.payments_v2.is_empty()
            && self.asset_transactions.is_empty()
            && self.atomic_swap_transactions.is_empty()
            && self.fastlane_primary_transactions.is_empty()
            && self.escrow_transactions.is_empty()
            && self.nft_transactions.is_empty()
            && self.offer_transactions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolEntry {
    pub tx_id: String,
    pub transfer: SignedTransfer,
}

impl MempoolEntry {
    pub fn new(tx_id: impl Into<String>, transfer: SignedTransfer) -> Self {
        Self {
            tx_id: tx_id.into(),
            transfer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolPaymentV2Entry {
    pub tx_id: String,
    pub payment: SignedPaymentV2,
}

impl MempoolPaymentV2Entry {
    pub fn new(tx_id: impl Into<String>, payment: SignedPaymentV2) -> Self {
        Self {
            tx_id: tx_id.into(),
            payment,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolAssetTransactionEntry {
    pub tx_id: String,
    pub transaction: SignedAssetTransaction,
}

impl MempoolAssetTransactionEntry {
    pub fn new(tx_id: impl Into<String>, transaction: SignedAssetTransaction) -> Self {
        Self {
            tx_id: tx_id.into(),
            transaction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolAtomicSwapEntry {
    pub tx_id: String,
    pub transaction: SignedAtomicSwapTransaction,
}

impl MempoolAtomicSwapEntry {
    pub fn new(tx_id: impl Into<String>, transaction: SignedAtomicSwapTransaction) -> Self {
        Self {
            tx_id: tx_id.into(),
            transaction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolFastLanePrimaryEntry {
    pub tx_id: String,
    pub transaction: FastLanePrimaryTransactionV1,
}

impl MempoolFastLanePrimaryEntry {
    pub fn new(tx_id: impl Into<String>, transaction: FastLanePrimaryTransactionV1) -> Self {
        Self {
            tx_id: tx_id.into(),
            transaction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolEscrowTransactionEntry {
    pub tx_id: String,
    pub transaction: SignedEscrowTransaction,
}

impl MempoolEscrowTransactionEntry {
    pub fn new(tx_id: impl Into<String>, transaction: SignedEscrowTransaction) -> Self {
        Self {
            tx_id: tx_id.into(),
            transaction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolNftTransactionEntry {
    pub tx_id: String,
    pub transaction: SignedNftTransaction,
}

impl MempoolNftTransactionEntry {
    pub fn new(tx_id: impl Into<String>, transaction: SignedNftTransaction) -> Self {
        Self {
            tx_id: tx_id.into(),
            transaction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolOfferTransactionEntry {
    pub tx_id: String,
    pub transaction: SignedOfferTransaction,
}

impl MempoolOfferTransactionEntry {
    pub fn new(tx_id: impl Into<String>, transaction: SignedOfferTransaction) -> Self {
        Self {
            tx_id: tx_id.into(),
            transaction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolState {
    #[serde(default)]
    pub pending: Vec<MempoolEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_payment_v2: Vec<MempoolPaymentV2Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_asset_transactions: Vec<MempoolAssetTransactionEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_atomic_swaps: Vec<MempoolAtomicSwapEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_fastlane_primary: Vec<MempoolFastLanePrimaryEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_escrow_transactions: Vec<MempoolEscrowTransactionEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_nft_transactions: Vec<MempoolNftTransactionEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_offer_transactions: Vec<MempoolOfferTransactionEntry>,
}

impl MempoolState {
    pub fn empty() -> Self {
        Self {
            pending: Vec::new(),
            pending_payment_v2: Vec::new(),
            pending_asset_transactions: Vec::new(),
            pending_atomic_swaps: Vec::new(),
            pending_fastlane_primary: Vec::new(),
            pending_escrow_transactions: Vec::new(),
            pending_nft_transactions: Vec::new(),
            pending_offer_transactions: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.pending
            .len()
            .saturating_add(self.pending_payment_v2.len())
            .saturating_add(self.pending_asset_transactions.len())
            .saturating_add(self.pending_atomic_swaps.len())
            .saturating_add(self.pending_fastlane_primary.len())
            .saturating_add(self.pending_escrow_transactions.len())
            .saturating_add(self.pending_nft_transactions.len())
            .saturating_add(self.pending_offer_transactions.len())
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
            && self.pending_payment_v2.is_empty()
            && self.pending_asset_transactions.is_empty()
            && self.pending_atomic_swaps.is_empty()
            && self.pending_fastlane_primary.is_empty()
            && self.pending_escrow_transactions.is_empty()
            && self.pending_nft_transactions.is_empty()
            && self.pending_offer_transactions.is_empty()
    }

    pub fn has_sender_sequence(&self, from: &str, sequence: u64) -> bool {
        self.pending.iter().any(|entry| {
            entry.transfer.unsigned.from == from && entry.transfer.unsigned.sequence == sequence
        }) || self.pending_payment_v2.iter().any(|entry| {
            entry.payment.unsigned.from == from && entry.payment.unsigned.sequence == sequence
        }) || self.pending_asset_transactions.iter().any(|entry| {
            entry.transaction.unsigned.source == from
                && entry.transaction.unsigned.sequence == sequence
        }) || self.pending_atomic_swaps.iter().any(|entry| {
            (entry.transaction.unsigned.leg_0.owner == from
                && entry.transaction.unsigned.leg_0.sequence == sequence)
                || (entry.transaction.unsigned.leg_1.owner == from
                    && entry.transaction.unsigned.leg_1.sequence == sequence)
        }) || self.pending_fastlane_primary.iter().any(|entry| {
            match &entry.transaction.operation {
                FastLanePrimaryOperationV1::Deposit { signed } => {
                    signed.deposit.source_address == from && signed.deposit.sequence == sequence
                }
                FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
                    signed.deposit.source_address == from && signed.deposit.sequence == sequence
                }
                _ => false,
            }
        }) || self.pending_escrow_transactions.iter().any(|entry| {
            entry.transaction.unsigned.source == from
                && entry.transaction.unsigned.sequence == sequence
        }) || self.pending_nft_transactions.iter().any(|entry| {
            entry.transaction.unsigned.source == from
                && entry.transaction.unsigned.sequence == sequence
        }) || self.pending_offer_transactions.iter().any(|entry| {
            entry.transaction.unsigned.source == from
                && entry.transaction.unsigned.sequence == sequence
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub tx_id: String,
    pub accepted: bool,
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub fee_charged: u64,
    #[serde(default)]
    pub fee_burned: u64,
    #[serde(default)]
    pub minimum_fee: u64,
    #[serde(default)]
    pub account_reserve: u64,
    #[serde(default)]
    pub state_expansion_fee: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub nft_issuer_transfer_fee: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_issuer_transfer_fee_recipient: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub nft_collection_flags: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offer_fills: Vec<OfferFillReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atomic_swap_legs: Option<Vec<AtomicSwapLegReceipt>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSwapLegReceipt {
    pub owner: String,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    pub fee_charged: u64,
    pub pre_sequence: u64,
    pub post_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferFillReceipt {
    pub fill_index: u64,
    pub maker_offer_id: String,
    pub maker_owner: String,
    pub taker: String,
    pub maker_sends_asset_id: String,
    pub maker_sends_amount: u64,
    pub taker_sends_asset_id: String,
    pub taker_sends_amount: u64,
    pub maker_taker_gets_remaining: u64,
    pub maker_taker_pays_remaining: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_maker_state: Option<String>,
}

impl Receipt {
    pub fn accepted(tx_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tx_id: tx_id.into(),
            accepted: true,
            code: "accepted".to_string(),
            message: message.into(),
            fee_charged: 0,
            fee_burned: 0,
            minimum_fee: 0,
            account_reserve: 0,
            state_expansion_fee: 0,
            nft_issuer_transfer_fee: 0,
            nft_issuer_transfer_fee_recipient: None,
            nft_collection_flags: 0,
            offer_id: None,
            offer_fills: Vec::new(),
            atomic_swap_legs: None,
        }
    }

    pub fn rejected(
        tx_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            tx_id: tx_id.into(),
            accepted: false,
            code: code.into(),
            message: message.into(),
            fee_charged: 0,
            fee_burned: 0,
            minimum_fee: 0,
            account_reserve: 0,
            state_expansion_fee: 0,
            nft_issuer_transfer_fee: 0,
            nft_issuer_transfer_fee_recipient: None,
            nft_collection_flags: 0,
            offer_id: None,
            offer_fills: Vec::new(),
            atomic_swap_legs: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    pub fn with_offer_id(mut self, offer_id: impl Into<String>) -> Self {
        self.offer_id = Some(offer_id.into());
        self
    }

    pub fn with_offer_fills(mut self, fills: Vec<OfferFillReceipt>) -> Self {
        self.offer_fills = fills;
        self
    }

    pub fn with_atomic_swap_legs(mut self, legs: Vec<AtomicSwapLegReceipt>) -> Self {
        self.atomic_swap_legs = Some(legs);
        self
    }

    pub fn with_fee_policy(
        mut self,
        fee_charged: u64,
        fee_burned: u64,
        minimum_fee: u64,
        account_reserve: u64,
    ) -> Self {
        self.fee_charged = fee_charged;
        self.fee_burned = fee_burned;
        self.minimum_fee = minimum_fee;
        self.account_reserve = account_reserve;
        self
    }

    pub fn with_fee_policy_and_state_expansion(
        mut self,
        fee_charged: u64,
        fee_burned: u64,
        minimum_fee: u64,
        account_reserve: u64,
        state_expansion_fee: u64,
    ) -> Self {
        self.fee_charged = fee_charged;
        self.fee_burned = fee_burned;
        self.minimum_fee = minimum_fee;
        self.account_reserve = account_reserve;
        self.state_expansion_fee = state_expansion_fee;
        self
    }

    pub fn with_nft_issuer_transfer_fee(mut self, fee: u64, recipient: impl Into<String>) -> Self {
        self.nft_issuer_transfer_fee = fee;
        self.nft_issuer_transfer_fee_recipient = Some(recipient.into());
        self
    }

    pub fn with_nft_collection_flags(mut self, collection_flags: u32) -> Self {
        self.nft_collection_flags = collection_flags;
        self
    }
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

include!("transactions_validation_helpers.rs");
