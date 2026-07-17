pub const PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.pfusdc.ingress_public_values.v1";
pub const PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.pfusdc.egress_public_values.v1";
pub const BRIDGE_EXIT_LEAF_SCHEMA_V1: &str = "postfiat.bridge_exit_leaf.v1";
pub const BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE: &str = "accepted";

const PFUSDC_INGRESS_COMMITMENT_DOMAIN_V1: &str =
    "postfiat.pfusdc.ingress_public_values.commitment.v1";
const PFUSDC_EGRESS_COMMITMENT_DOMAIN_V1: &str =
    "postfiat.pfusdc.egress_public_values.commitment.v1";
const BRIDGE_EXIT_LEAF_COMMITMENT_DOMAIN_V1: &str =
    "postfiat.bridge_exit_leaf.commitment.v1";
const BRIDGE_EXIT_EMPTY_ROOT_DOMAIN_V1: &str = "postfiat.bridge_exit_tree.empty.v1";
const BRIDGE_EXIT_NODE_DOMAIN_V1: &str = "postfiat.bridge_exit_tree.node.v1";
const PFUSDC_TIER4_CANONICAL_MAGIC: &[u8] = b"PFTL-PFUSDC-TIER4";
const PFUSDC_MAX_TEXT_BYTES: usize = 256;
const BRIDGE_EXIT_MAX_LEAVES_PER_BLOCK_V1: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeExitLeafV1 {
    pub schema: String,
    pub route_epoch: u64,
    pub asset_id: String,
    pub burn_tx_id: String,
    pub withdrawal_id: String,
    pub source_bucket_id: String,
    pub amount_atoms: u64,
    pub recipient: String,
    pub destination_hash: String,
    pub evidence_root: String,
    pub finalized_height: u64,
    pub accepted_receipt_id: String,
    pub accepted_receipt_code: String,
    pub withdrawal_packet_hash: String,
    pub withdrawal_packet_evm_digest: String,
}

impl BridgeExitLeafV1 {
    pub fn from_withdrawal_packet(
        route_epoch: u64,
        accepted_receipt_id: impl Into<String>,
        accepted_receipt_code: impl Into<String>,
        packet: &VaultBridgeWithdrawalPacket,
        withdrawal_packet_hash: impl Into<String>,
        withdrawal_packet_evm_digest: impl Into<String>,
    ) -> Result<Self, String> {
        let leaf = Self {
            schema: BRIDGE_EXIT_LEAF_SCHEMA_V1.to_string(),
            route_epoch,
            asset_id: packet.vault_bridge_asset_id.clone(),
            burn_tx_id: packet.burn_tx_id.clone(),
            withdrawal_id: packet.withdrawal_id.clone(),
            source_bucket_id: packet.source_bucket_id.clone(),
            amount_atoms: packet.amount_atoms,
            recipient: packet.recipient.clone(),
            destination_hash: packet.destination_hash.clone(),
            evidence_root: packet.evidence_root.clone(),
            finalized_height: packet.finalized_height,
            accepted_receipt_id: accepted_receipt_id.into(),
            accepted_receipt_code: accepted_receipt_code.into(),
            withdrawal_packet_hash: withdrawal_packet_hash.into(),
            withdrawal_packet_evm_digest: withdrawal_packet_evm_digest.into(),
        };
        leaf.validate()?;
        Ok(leaf)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != BRIDGE_EXIT_LEAF_SCHEMA_V1 {
            return Err("bridge exit leaf schema mismatch".to_string());
        }
        if self.route_epoch == 0 {
            return Err("bridge exit leaf route_epoch must be nonzero".to_string());
        }
        validate_lower_hex_len("bridge_exit_leaf.asset_id", &self.asset_id, 96)?;
        validate_lower_hex_len("bridge_exit_leaf.burn_tx_id", &self.burn_tx_id, 96)?;
        validate_lower_hex_len("bridge_exit_leaf.withdrawal_id", &self.withdrawal_id, 96)?;
        validate_lower_hex_len(
            "bridge_exit_leaf.source_bucket_id",
            &self.source_bucket_id,
            96,
        )?;
        if self.amount_atoms == 0 {
            return Err("bridge exit leaf amount_atoms must be nonzero".to_string());
        }
        validate_evm_address_text("bridge_exit_leaf.recipient", &self.recipient)?;
        validate_lower_hex_len(
            "bridge_exit_leaf.destination_hash",
            &self.destination_hash,
            96,
        )?;
        validate_lower_hex_len("bridge_exit_leaf.evidence_root", &self.evidence_root, 96)?;
        if self.finalized_height == 0 {
            return Err("bridge exit leaf finalized_height must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "bridge_exit_leaf.accepted_receipt_id",
            &self.accepted_receipt_id,
            96,
        )?;
        if self.accepted_receipt_code != BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE {
            return Err("bridge exit leaf requires literal accepted receipt code".to_string());
        }
        validate_lower_hex_len(
            "bridge_exit_leaf.withdrawal_packet_hash",
            &self.withdrawal_packet_hash,
            96,
        )?;
        validate_lower_hex_len(
            "bridge_exit_leaf.withdrawal_packet_evm_digest",
            &self.withdrawal_packet_evm_digest,
            64,
        )
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = pfusdc_canonical_prefix(BRIDGE_EXIT_LEAF_SCHEMA_V1);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_u64(&mut out, 2, self.route_epoch);
        pfusdc_append_hex(&mut out, 3, &self.asset_id)?;
        pfusdc_append_hex(&mut out, 4, &self.burn_tx_id)?;
        pfusdc_append_hex(&mut out, 5, &self.withdrawal_id)?;
        pfusdc_append_hex(&mut out, 6, &self.source_bucket_id)?;
        pfusdc_append_u64(&mut out, 7, self.amount_atoms);
        pfusdc_append_evm_address(&mut out, 8, &self.recipient)?;
        pfusdc_append_hex(&mut out, 9, &self.destination_hash)?;
        pfusdc_append_hex(&mut out, 10, &self.evidence_root)?;
        pfusdc_append_u64(&mut out, 11, self.finalized_height);
        pfusdc_append_hex(&mut out, 12, &self.accepted_receipt_id)?;
        pfusdc_append_text(&mut out, 13, &self.accepted_receipt_code)?;
        pfusdc_append_hex(&mut out, 14, &self.withdrawal_packet_hash)?;
        pfusdc_append_hex(&mut out, 15, &self.withdrawal_packet_evm_digest)?;
        Ok(out)
    }

    pub fn commitment(&self) -> Result<String, String> {
        Ok(pfusdc_sha3_384_commitment(
            BRIDGE_EXIT_LEAF_COMMITMENT_DOMAIN_V1,
            &self.canonical_bytes()?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcIngressPublicValuesV1 {
    pub schema: String,
    pub proof_program_version: u32,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
    pub route_profile_hash: String,
    pub route_epoch: u64,
    pub ethereum_chain_id: u64,
    pub ethereum_finalized_beacon_root: String,
    pub ethereum_finalized_slot: u64,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: String,
    pub arbitrum_assertion_hash: String,
    pub l2_block_number: u64,
    pub l2_block_hash: String,
    pub l2_state_root: String,
    pub l2_receipts_root: String,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub transaction_hash: String,
    pub transaction_index: u64,
    pub receipt_status: u8,
    pub log_index: u64,
    pub event_signature: String,
    pub event_emitter: String,
    pub depositor: String,
    pub pftl_recipient: String,
    pub pftl_recipient_hash: String,
    pub amount_atoms: u64,
    pub nonce: String,
    pub route_binding: String,
    pub deposit_id: String,
    pub evidence_root: String,
    pub public_values_commitment: String,
}

impl PfUsdcIngressPublicValuesV1 {
    pub fn canonical_bytes_without_commitment(&self) -> Result<Vec<u8>, String> {
        self.validate_fields(false)?;
        let mut out = pfusdc_canonical_prefix(PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V1);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_u32(&mut out, 2, self.proof_program_version);
        pfusdc_append_text(&mut out, 3, &self.pftl_chain_id)?;
        pfusdc_append_hex(&mut out, 4, &self.pftl_genesis_hash)?;
        pfusdc_append_u32(&mut out, 5, self.pftl_protocol_version);
        pfusdc_append_hex(&mut out, 6, &self.route_profile_hash)?;
        pfusdc_append_u64(&mut out, 7, self.route_epoch);
        pfusdc_append_u64(&mut out, 8, self.ethereum_chain_id);
        pfusdc_append_hex(&mut out, 9, &self.ethereum_finalized_beacon_root)?;
        pfusdc_append_u64(&mut out, 10, self.ethereum_finalized_slot);
        pfusdc_append_u64(&mut out, 11, self.arbitrum_chain_id);
        pfusdc_append_evm_address(&mut out, 12, &self.arbitrum_rollup_address)?;
        pfusdc_append_hex(&mut out, 13, &self.arbitrum_assertion_hash)?;
        pfusdc_append_u64(&mut out, 14, self.l2_block_number);
        pfusdc_append_hex(&mut out, 15, &self.l2_block_hash)?;
        pfusdc_append_hex(&mut out, 16, &self.l2_state_root)?;
        pfusdc_append_hex(&mut out, 17, &self.l2_receipts_root)?;
        pfusdc_append_evm_address(&mut out, 18, &self.vault_address)?;
        pfusdc_append_hex(&mut out, 19, &self.vault_runtime_code_hash)?;
        pfusdc_append_evm_address(&mut out, 20, &self.token_address)?;
        pfusdc_append_hex(&mut out, 21, &self.token_runtime_code_hash)?;
        pfusdc_append_hex(&mut out, 22, &self.transaction_hash)?;
        pfusdc_append_u64(&mut out, 23, self.transaction_index);
        pfusdc_append_u8(&mut out, 24, self.receipt_status);
        pfusdc_append_u64(&mut out, 25, self.log_index);
        pfusdc_append_hex(&mut out, 26, &self.event_signature)?;
        pfusdc_append_evm_address(&mut out, 27, &self.event_emitter)?;
        pfusdc_append_evm_address(&mut out, 28, &self.depositor)?;
        pfusdc_append_text(&mut out, 29, &self.pftl_recipient)?;
        pfusdc_append_hex(&mut out, 30, &self.pftl_recipient_hash)?;
        pfusdc_append_u64(&mut out, 31, self.amount_atoms);
        pfusdc_append_hex(&mut out, 32, &self.nonce)?;
        pfusdc_append_hex(&mut out, 33, &self.route_binding)?;
        pfusdc_append_hex(&mut out, 34, &self.deposit_id)?;
        pfusdc_append_hex(&mut out, 35, &self.evidence_root)?;
        Ok(out)
    }

    pub fn expected_commitment(&self) -> Result<String, String> {
        Ok(pfusdc_keccak_commitment(
            PFUSDC_INGRESS_COMMITMENT_DOMAIN_V1,
            &self.canonical_bytes_without_commitment()?,
        ))
    }

    pub fn seal(&mut self) -> Result<(), String> {
        self.public_values_commitment = self.expected_commitment()?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_fields(true)
    }

    fn validate_fields(&self, check_commitment: bool) -> Result<(), String> {
        if self.schema != PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V1 {
            return Err("pfUSDC ingress public-values schema mismatch".to_string());
        }
        if self.proof_program_version == 0 || self.pftl_protocol_version == 0 {
            return Err("pfUSDC ingress versions must be nonzero".to_string());
        }
        pfusdc_validate_text("pfusdc_ingress.pftl_chain_id", &self.pftl_chain_id)?;
        pfusdc_validate_hex_fields(&[
            ("pfusdc_ingress.pftl_genesis_hash", &self.pftl_genesis_hash, 96),
            ("pfusdc_ingress.route_profile_hash", &self.route_profile_hash, 96),
            ("pfusdc_ingress.ethereum_finalized_beacon_root", &self.ethereum_finalized_beacon_root, 64),
            ("pfusdc_ingress.arbitrum_assertion_hash", &self.arbitrum_assertion_hash, 64),
            ("pfusdc_ingress.l2_block_hash", &self.l2_block_hash, 64),
            ("pfusdc_ingress.l2_state_root", &self.l2_state_root, 64),
            ("pfusdc_ingress.l2_receipts_root", &self.l2_receipts_root, 64),
            ("pfusdc_ingress.vault_runtime_code_hash", &self.vault_runtime_code_hash, 64),
            ("pfusdc_ingress.token_runtime_code_hash", &self.token_runtime_code_hash, 64),
            ("pfusdc_ingress.transaction_hash", &self.transaction_hash, 64),
            ("pfusdc_ingress.event_signature", &self.event_signature, 64),
            ("pfusdc_ingress.pftl_recipient_hash", &self.pftl_recipient_hash, 64),
            ("pfusdc_ingress.nonce", &self.nonce, 64),
            ("pfusdc_ingress.route_binding", &self.route_binding, 96),
            ("pfusdc_ingress.deposit_id", &self.deposit_id, 64),
            ("pfusdc_ingress.evidence_root", &self.evidence_root, 96),
        ])?;
        if self.route_epoch == 0
            || self.ethereum_chain_id == 0
            || self.ethereum_finalized_slot == 0
            || self.arbitrum_chain_id == 0
            || self.l2_block_number == 0
            || self.receipt_status != 1
            || self.amount_atoms == 0
        {
            return Err("pfUSDC ingress numeric/finality fields are invalid".to_string());
        }
        validate_evm_address_text("pfusdc_ingress.arbitrum_rollup_address", &self.arbitrum_rollup_address)?;
        validate_evm_address_text("pfusdc_ingress.vault_address", &self.vault_address)?;
        validate_evm_address_text("pfusdc_ingress.token_address", &self.token_address)?;
        validate_evm_address_text("pfusdc_ingress.event_emitter", &self.event_emitter)?;
        validate_evm_address_text("pfusdc_ingress.depositor", &self.depositor)?;
        pfusdc_validate_text("pfusdc_ingress.pftl_recipient", &self.pftl_recipient)?;
        if check_commitment {
            validate_lower_hex_len(
                "pfusdc_ingress.public_values_commitment",
                &self.public_values_commitment,
                64,
            )?;
            if self.public_values_commitment != self.expected_commitment()? {
                return Err("pfUSDC ingress public-values commitment mismatch".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcEgressPublicValuesV1 {
    pub schema: String,
    pub proof_program_version: u32,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
    pub route_profile_hash: String,
    pub route_epoch: u64,
    pub prior_checkpoint_block_id: String,
    pub resulting_checkpoint_block_id: String,
    pub committee_epoch: u64,
    pub committee_root: String,
    pub committee_transition_commitment: String,
    pub finalized_block_height: u64,
    pub finalized_block_view: u64,
    pub finalized_block_id: String,
    pub finalized_parent_block_id: String,
    pub finalized_state_root: String,
    pub bridge_exit_root: String,
    pub exit_leaf_index: u64,
    pub exit_leaf_commitment: String,
    pub accepted_receipt_id: String,
    pub accepted_receipt_code: String,
    pub asset_id: String,
    pub burn_tx_id: String,
    pub withdrawal_id: String,
    pub source_bucket_id: String,
    pub amount_atoms: u64,
    pub recipient: String,
    pub destination_hash: String,
    pub evidence_root: String,
    pub withdrawal_finalized_height: u64,
    pub arbitrum_chain_id: u64,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub withdrawal_packet_digest: String,
    pub withdrawal_packet_hash: String,
    pub proof_nullifier: String,
    pub public_values_commitment: String,
}

impl PfUsdcEgressPublicValuesV1 {
    pub fn canonical_bytes_without_commitment(&self) -> Result<Vec<u8>, String> {
        self.validate_fields(false)?;
        let mut out = pfusdc_canonical_prefix(PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_u32(&mut out, 2, self.proof_program_version);
        pfusdc_append_text(&mut out, 3, &self.pftl_chain_id)?;
        pfusdc_append_hex(&mut out, 4, &self.pftl_genesis_hash)?;
        pfusdc_append_u32(&mut out, 5, self.pftl_protocol_version);
        pfusdc_append_hex(&mut out, 6, &self.route_profile_hash)?;
        pfusdc_append_u64(&mut out, 7, self.route_epoch);
        pfusdc_append_hex(&mut out, 8, &self.prior_checkpoint_block_id)?;
        pfusdc_append_hex(&mut out, 9, &self.resulting_checkpoint_block_id)?;
        pfusdc_append_u64(&mut out, 10, self.committee_epoch);
        pfusdc_append_hex(&mut out, 11, &self.committee_root)?;
        pfusdc_append_optional_hex(&mut out, 12, &self.committee_transition_commitment)?;
        pfusdc_append_u64(&mut out, 13, self.finalized_block_height);
        pfusdc_append_u64(&mut out, 14, self.finalized_block_view);
        pfusdc_append_hex(&mut out, 15, &self.finalized_block_id)?;
        pfusdc_append_hex(&mut out, 16, &self.finalized_parent_block_id)?;
        pfusdc_append_hex(&mut out, 17, &self.finalized_state_root)?;
        pfusdc_append_hex(&mut out, 18, &self.bridge_exit_root)?;
        pfusdc_append_u64(&mut out, 19, self.exit_leaf_index);
        pfusdc_append_hex(&mut out, 20, &self.exit_leaf_commitment)?;
        pfusdc_append_hex(&mut out, 21, &self.accepted_receipt_id)?;
        pfusdc_append_text(&mut out, 22, &self.accepted_receipt_code)?;
        pfusdc_append_hex(&mut out, 23, &self.asset_id)?;
        pfusdc_append_hex(&mut out, 24, &self.burn_tx_id)?;
        pfusdc_append_hex(&mut out, 25, &self.withdrawal_id)?;
        pfusdc_append_hex(&mut out, 26, &self.source_bucket_id)?;
        pfusdc_append_u64(&mut out, 27, self.amount_atoms);
        pfusdc_append_evm_address(&mut out, 28, &self.recipient)?;
        pfusdc_append_hex(&mut out, 29, &self.destination_hash)?;
        pfusdc_append_hex(&mut out, 30, &self.evidence_root)?;
        pfusdc_append_u64(&mut out, 31, self.withdrawal_finalized_height);
        pfusdc_append_u64(&mut out, 32, self.arbitrum_chain_id);
        pfusdc_append_evm_address(&mut out, 33, &self.vault_address)?;
        pfusdc_append_hex(&mut out, 34, &self.vault_runtime_code_hash)?;
        pfusdc_append_evm_address(&mut out, 35, &self.token_address)?;
        pfusdc_append_hex(&mut out, 36, &self.token_runtime_code_hash)?;
        pfusdc_append_hex(&mut out, 37, &self.withdrawal_packet_digest)?;
        pfusdc_append_hex(&mut out, 38, &self.withdrawal_packet_hash)?;
        pfusdc_append_hex(&mut out, 39, &self.proof_nullifier)?;
        Ok(out)
    }

    pub fn expected_commitment(&self) -> Result<String, String> {
        Ok(pfusdc_keccak_commitment(
            PFUSDC_EGRESS_COMMITMENT_DOMAIN_V1,
            &self.canonical_bytes_without_commitment()?,
        ))
    }

    pub fn seal(&mut self) -> Result<(), String> {
        self.public_values_commitment = self.expected_commitment()?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_fields(true)
    }

    fn validate_fields(&self, check_commitment: bool) -> Result<(), String> {
        if self.schema != PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1 {
            return Err("pfUSDC egress public-values schema mismatch".to_string());
        }
        if self.proof_program_version == 0 || self.pftl_protocol_version == 0 {
            return Err("pfUSDC egress versions must be nonzero".to_string());
        }
        pfusdc_validate_text("pfusdc_egress.pftl_chain_id", &self.pftl_chain_id)?;
        pfusdc_validate_hex_fields(&[
            ("pfusdc_egress.pftl_genesis_hash", &self.pftl_genesis_hash, 96),
            ("pfusdc_egress.route_profile_hash", &self.route_profile_hash, 96),
            ("pfusdc_egress.prior_checkpoint_block_id", &self.prior_checkpoint_block_id, 96),
            ("pfusdc_egress.resulting_checkpoint_block_id", &self.resulting_checkpoint_block_id, 96),
            ("pfusdc_egress.committee_root", &self.committee_root, 96),
            ("pfusdc_egress.finalized_block_id", &self.finalized_block_id, 96),
            ("pfusdc_egress.finalized_parent_block_id", &self.finalized_parent_block_id, 96),
            ("pfusdc_egress.finalized_state_root", &self.finalized_state_root, 96),
            ("pfusdc_egress.bridge_exit_root", &self.bridge_exit_root, 96),
            ("pfusdc_egress.exit_leaf_commitment", &self.exit_leaf_commitment, 96),
            ("pfusdc_egress.accepted_receipt_id", &self.accepted_receipt_id, 96),
            ("pfusdc_egress.asset_id", &self.asset_id, 96),
            ("pfusdc_egress.burn_tx_id", &self.burn_tx_id, 96),
            ("pfusdc_egress.withdrawal_id", &self.withdrawal_id, 96),
            ("pfusdc_egress.source_bucket_id", &self.source_bucket_id, 96),
            ("pfusdc_egress.destination_hash", &self.destination_hash, 96),
            ("pfusdc_egress.evidence_root", &self.evidence_root, 96),
            ("pfusdc_egress.vault_runtime_code_hash", &self.vault_runtime_code_hash, 64),
            ("pfusdc_egress.token_runtime_code_hash", &self.token_runtime_code_hash, 64),
            ("pfusdc_egress.withdrawal_packet_digest", &self.withdrawal_packet_digest, 64),
            ("pfusdc_egress.withdrawal_packet_hash", &self.withdrawal_packet_hash, 96),
            ("pfusdc_egress.proof_nullifier", &self.proof_nullifier, 64),
        ])?;
        if !self.committee_transition_commitment.is_empty() {
            validate_lower_hex_len(
                "pfusdc_egress.committee_transition_commitment",
                &self.committee_transition_commitment,
                96,
            )?;
        }
        if self.route_epoch == 0
            || self.finalized_block_height == 0
            || self.withdrawal_finalized_height == 0
            || self.amount_atoms == 0
            || self.arbitrum_chain_id == 0
        {
            return Err("pfUSDC egress numeric/finality fields are invalid".to_string());
        }
        if self.resulting_checkpoint_block_id != self.finalized_block_id {
            return Err("pfUSDC egress resulting checkpoint must equal finalized block".to_string());
        }
        if self.accepted_receipt_code != BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE {
            return Err("pfUSDC egress requires literal accepted receipt code".to_string());
        }
        validate_evm_address_text("pfusdc_egress.recipient", &self.recipient)?;
        validate_evm_address_text("pfusdc_egress.vault_address", &self.vault_address)?;
        validate_evm_address_text("pfusdc_egress.token_address", &self.token_address)?;
        if check_commitment {
            validate_lower_hex_len(
                "pfusdc_egress.public_values_commitment",
                &self.public_values_commitment,
                64,
            )?;
            if self.public_values_commitment != self.expected_commitment()? {
                return Err("pfUSDC egress public-values commitment mismatch".to_string());
            }
        }
        Ok(())
    }
}

pub fn bridge_exit_empty_root_v1() -> String {
    pfusdc_sha3_384_commitment(BRIDGE_EXIT_EMPTY_ROOT_DOMAIN_V1, &[])
}

pub fn bridge_exit_merkle_root_v1(leaves: &[BridgeExitLeafV1]) -> Result<String, String> {
    if leaves.len() > BRIDGE_EXIT_MAX_LEAVES_PER_BLOCK_V1 {
        return Err("bridge exit tree exceeds the v1 per-block leaf limit".to_string());
    }
    if leaves.is_empty() {
        return Ok(bridge_exit_empty_root_v1());
    }
    let mut level = leaves
        .iter()
        .map(BridgeExitLeafV1::commitment)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|value| pfusdc_decode_hex(&value))
        .collect::<Result<Vec<_>, _>>()?;
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let right = pair.get(1).unwrap_or(&pair[0]);
            let mut bytes = Vec::with_capacity(pair[0].len() + right.len());
            bytes.extend_from_slice(&pair[0]);
            bytes.extend_from_slice(right);
            next.push(pfusdc_decode_hex(&pfusdc_sha3_384_commitment(
                BRIDGE_EXIT_NODE_DOMAIN_V1,
                &bytes,
            ))?);
        }
        level = next;
    }
    Ok(bytes_to_hex(&level[0]))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeExitMerkleProofV1 {
    pub leaf: BridgeExitLeafV1,
    pub leaf_index: u64,
    pub leaf_count: u64,
    pub siblings: Vec<String>,
}

pub fn bridge_exit_merkle_proof_v1(
    leaves: &[BridgeExitLeafV1],
    leaf_index: usize,
) -> Result<BridgeExitMerkleProofV1, String> {
    if leaves.is_empty() || leaves.len() > BRIDGE_EXIT_MAX_LEAVES_PER_BLOCK_V1 {
        return Err("bridge exit proof requires a nonempty bounded tree".to_string());
    }
    let leaf = leaves
        .get(leaf_index)
        .cloned()
        .ok_or_else(|| "bridge exit proof leaf index is out of bounds".to_string())?;
    let mut level = leaves
        .iter()
        .map(BridgeExitLeafV1::commitment)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|value| pfusdc_decode_hex(&value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut index = leaf_index;
    let mut siblings = Vec::new();
    while level.len() > 1 {
        let sibling_index = if index.is_multiple_of(2) {
            (index + 1).min(level.len() - 1)
        } else {
            index - 1
        };
        siblings.push(bytes_to_hex(&level[sibling_index]));
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let right = pair.get(1).unwrap_or(&pair[0]);
            let mut bytes = Vec::with_capacity(pair[0].len() + right.len());
            bytes.extend_from_slice(&pair[0]);
            bytes.extend_from_slice(right);
            next.push(pfusdc_decode_hex(&pfusdc_sha3_384_commitment(
                BRIDGE_EXIT_NODE_DOMAIN_V1,
                &bytes,
            ))?);
        }
        level = next;
        index /= 2;
    }
    Ok(BridgeExitMerkleProofV1 {
        leaf,
        leaf_index: u64::try_from(leaf_index)
            .map_err(|_| "bridge exit proof leaf index exceeds u64".to_string())?,
        leaf_count: u64::try_from(leaves.len())
            .map_err(|_| "bridge exit proof leaf count exceeds u64".to_string())?,
        siblings,
    })
}

pub fn verify_bridge_exit_merkle_proof_v1(
    expected_root: &str,
    proof: &BridgeExitMerkleProofV1,
) -> Result<(), String> {
    validate_lower_hex_len("bridge_exit_proof.expected_root", expected_root, 96)?;
    if proof.leaf_count == 0
        || proof.leaf_count > BRIDGE_EXIT_MAX_LEAVES_PER_BLOCK_V1 as u64
        || proof.leaf_index >= proof.leaf_count
    {
        return Err("bridge exit proof has invalid leaf bounds".to_string());
    }
    let expected_depth = if proof.leaf_count <= 1 {
        0
    } else {
        (u64::BITS - (proof.leaf_count - 1).leading_zeros()) as usize
    };
    if proof.siblings.len() != expected_depth {
        return Err("bridge exit proof sibling depth mismatch".to_string());
    }
    let mut current = pfusdc_decode_hex(&proof.leaf.commitment()?)?;
    let mut index = proof.leaf_index;
    for sibling in &proof.siblings {
        validate_lower_hex_len("bridge_exit_proof.sibling", sibling, 96)?;
        let sibling = pfusdc_decode_hex(sibling)?;
        let mut bytes = Vec::with_capacity(current.len() + sibling.len());
        if index.is_multiple_of(2) {
            bytes.extend_from_slice(&current);
            bytes.extend_from_slice(&sibling);
        } else {
            bytes.extend_from_slice(&sibling);
            bytes.extend_from_slice(&current);
        }
        current = pfusdc_decode_hex(&pfusdc_sha3_384_commitment(
            BRIDGE_EXIT_NODE_DOMAIN_V1,
            &bytes,
        ))?;
        index /= 2;
    }
    if bytes_to_hex(&current) != expected_root {
        return Err("bridge exit proof root mismatch".to_string());
    }
    Ok(())
}

fn pfusdc_canonical_prefix(schema: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(512);
    out.extend_from_slice(PFUSDC_TIER4_CANONICAL_MAGIC);
    out.extend_from_slice(&(schema.len() as u32).to_be_bytes());
    out.extend_from_slice(schema.as_bytes());
    out
}

fn pfusdc_append_field(out: &mut Vec<u8>, tag: u16, value: &[u8]) -> Result<(), String> {
    let len = u32::try_from(value.len())
        .map_err(|_| "pfUSDC canonical field exceeds u32 length".to_string())?;
    out.extend_from_slice(&tag.to_be_bytes());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}

fn pfusdc_append_text(out: &mut Vec<u8>, tag: u16, value: &str) -> Result<(), String> {
    pfusdc_validate_text("pfUSDC canonical text", value)?;
    pfusdc_append_field(out, tag, value.as_bytes())
}

fn pfusdc_append_hex(out: &mut Vec<u8>, tag: u16, value: &str) -> Result<(), String> {
    pfusdc_append_field(out, tag, &pfusdc_decode_hex(value)?)
}

fn pfusdc_append_optional_hex(
    out: &mut Vec<u8>,
    tag: u16,
    value: &str,
) -> Result<(), String> {
    if value.is_empty() {
        pfusdc_append_field(out, tag, &[])
    } else {
        pfusdc_append_hex(out, tag, value)
    }
}

fn pfusdc_append_evm_address(out: &mut Vec<u8>, tag: u16, value: &str) -> Result<(), String> {
    validate_evm_address_text("pfUSDC canonical EVM address", value)?;
    let stripped = value
        .strip_prefix("0x")
        .ok_or_else(|| "pfUSDC canonical EVM address must start with 0x".to_string())?;
    pfusdc_append_hex(out, tag, stripped)
}

fn pfusdc_append_u8(out: &mut Vec<u8>, tag: u16, value: u8) {
    pfusdc_append_fixed_field(out, tag, &[value]);
}

fn pfusdc_append_u32(out: &mut Vec<u8>, tag: u16, value: u32) {
    pfusdc_append_fixed_field(out, tag, &value.to_be_bytes());
}

fn pfusdc_append_u64(out: &mut Vec<u8>, tag: u16, value: u64) {
    pfusdc_append_fixed_field(out, tag, &value.to_be_bytes());
}

fn pfusdc_append_fixed_field(out: &mut Vec<u8>, tag: u16, value: &[u8]) {
    debug_assert!(value.len() <= u32::MAX as usize);
    out.extend_from_slice(&tag.to_be_bytes());
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
}

fn pfusdc_validate_text(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > PFUSDC_MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{field} must be nonempty, control-free, and at most {PFUSDC_MAX_TEXT_BYTES} bytes"
        ));
    }
    Ok(())
}

fn pfusdc_validate_hex_fields(fields: &[(&str, &String, usize)]) -> Result<(), String> {
    for (field, value, len) in fields {
        validate_lower_hex_len(field, value, *len)?;
    }
    Ok(())
}

fn pfusdc_decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2)
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err("pfUSDC canonical hex must be even-length lowercase hex".to_string());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char)
                .to_digit(16)
                .ok_or_else(|| "invalid hex".to_string())?;
            let low = (pair[1] as char)
                .to_digit(16)
                .ok_or_else(|| "invalid hex".to_string())?;
            Ok(((high << 4) | low) as u8)
        })
        .collect()
}

fn pfusdc_sha3_384_commitment(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    bytes_to_hex(&hasher.finalize())
}

fn pfusdc_keccak_commitment(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    bytes_to_hex(&hasher.finalize())
}
