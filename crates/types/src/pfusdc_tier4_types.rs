pub const PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3: &str =
    "postfiat.pfusdc.ingress_public_values.v3";
pub const PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.pfusdc.egress_public_values.v1";
pub const PFUSDC_CHECKPOINT_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.pfusdc.checkpoint_public_values.v1";
pub const BRIDGE_EXIT_LEAF_SCHEMA_V1: &str = "postfiat.bridge_exit_leaf.v1";
pub const BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE: &str = "accepted";

const PFUSDC_INGRESS_COMMITMENT_DOMAIN_V3: &str =
    "postfiat.pfusdc.ingress_public_values.commitment.v3";
const PFUSDC_INGRESS_PROOF_HASH_DOMAIN_V1: &str =
    "postfiat.pfusdc.ingress_proof.hash.v1";
const PFUSDC_INGRESS_PUBLIC_VALUES_HASH_DOMAIN_V1: &str =
    "postfiat.pfusdc.ingress_public_values.hash.v1";
const PFUSDC_EGRESS_COMMITMENT_DOMAIN_V1: &str =
    "postfiat.pfusdc.egress_public_values.commitment.v1";
const PFUSDC_CHECKPOINT_COMMITMENT_DOMAIN_V1: &str =
    "postfiat.pfusdc.checkpoint_public_values.commitment.v1";
const PFUSDC_EGRESS_NULLIFIER_DOMAIN_V1: &str = "postfiat.pfusdc.egress_proof.nullifier.v1";
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
pub struct PfUsdcIngressPublicValuesV3 {
    pub schema: String,
    pub proof_program_version: u32,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
    pub route_profile_hash: String,
    pub route_epoch: u64,
    pub ethereum_chain_id: u64,
    pub prior_ethereum_finalized_beacon_root: String,
    pub prior_ethereum_finalized_slot: u64,
    pub ethereum_finalized_beacon_root: String,
    pub ethereum_finalized_slot: u64,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: String,
    pub arbitrum_rollup_runtime_code_hash: String,
    pub rollup_latest_confirmed_storage_slot: String,
    pub arbitrum_assertion_hash: String,
    pub assertion_l2_block_hash: String,
    pub assertion_l2_state_root: String,
    pub assertion_send_root: String,
    pub output_index: u64,
    pub output_item_hash: String,
    pub output_l2_block_number: u64,
    pub output_l1_block_number: u64,
    pub output_timestamp: u64,
    pub output_sender: String,
    pub output_destination: String,
    pub ingress_anchor_runtime_code_hash: String,
    pub output_calldata_hash: String,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
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

impl PfUsdcIngressPublicValuesV3 {
    /// Strictly decode the exact byte string committed by the ingress SP1
    /// program. Tags must be contiguous and ordered, fixed-width fields must
    /// have their canonical width, and trailing bytes are rejected.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = PfusdcCanonicalReader::new(bytes, PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3)?;
        let mut values = Self {
            schema: reader.text(1)?,
            proof_program_version: reader.u32(2)?,
            pftl_chain_id: reader.text(3)?,
            pftl_genesis_hash: reader.hex(4, 48)?,
            pftl_protocol_version: reader.u32(5)?,
            route_profile_hash: reader.hex(6, 48)?,
            route_epoch: reader.u64(7)?,
            ethereum_chain_id: reader.u64(8)?,
            prior_ethereum_finalized_beacon_root: reader.hex(9, 32)?,
            prior_ethereum_finalized_slot: reader.u64(10)?,
            ethereum_finalized_beacon_root: reader.hex(11, 32)?,
            ethereum_finalized_slot: reader.u64(12)?,
            arbitrum_chain_id: reader.u64(13)?,
            arbitrum_rollup_address: reader.evm_address(14)?,
            arbitrum_assertion_hash: reader.hex(15, 32)?,
            assertion_l2_block_hash: reader.hex(16, 32)?,
            assertion_send_root: reader.hex(17, 32)?,
            output_index: reader.u64(18)?,
            output_item_hash: reader.hex(19, 32)?,
            output_l2_block_number: reader.u64(20)?,
            output_l1_block_number: reader.u64(21)?,
            output_timestamp: reader.u64(22)?,
            output_destination: reader.evm_address(23)?,
            output_calldata_hash: reader.hex(24, 32)?,
            vault_address: reader.evm_address(25)?,
            vault_runtime_code_hash: reader.hex(26, 32)?,
            token_address: reader.evm_address(27)?,
            token_runtime_code_hash: reader.hex(28, 32)?,
            depositor: reader.evm_address(29)?,
            pftl_recipient: reader.text(30)?,
            pftl_recipient_hash: reader.hex(31, 32)?,
            amount_atoms: reader.u64(32)?,
            nonce: reader.hex(33, 32)?,
            route_binding: reader.hex(34, 32)?,
            deposit_id: reader.hex(35, 32)?,
            evidence_root: reader.hex(36, 48)?,
            arbitrum_rollup_runtime_code_hash: reader.hex(37, 32)?,
            rollup_latest_confirmed_storage_slot: reader.hex(38, 32)?,
            assertion_l2_state_root: reader.hex(39, 32)?,
            output_sender: reader.evm_address(40)?,
            ingress_anchor_runtime_code_hash: reader.hex(41, 32)?,
            public_values_commitment: String::new(),
        };
        reader.finish()?;
        values.seal()?;
        values.validate()?;
        if values.canonical_bytes_without_commitment()? != bytes {
            return Err("pfUSDC ingress public values are not canonical".to_string());
        }
        Ok(values)
    }

    pub fn canonical_bytes_without_commitment(&self) -> Result<Vec<u8>, String> {
        self.validate_fields(false)?;
        let mut out = pfusdc_canonical_prefix(PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_u32(&mut out, 2, self.proof_program_version);
        pfusdc_append_text(&mut out, 3, &self.pftl_chain_id)?;
        pfusdc_append_hex(&mut out, 4, &self.pftl_genesis_hash)?;
        pfusdc_append_u32(&mut out, 5, self.pftl_protocol_version);
        pfusdc_append_hex(&mut out, 6, &self.route_profile_hash)?;
        pfusdc_append_u64(&mut out, 7, self.route_epoch);
        pfusdc_append_u64(&mut out, 8, self.ethereum_chain_id);
        pfusdc_append_hex(
            &mut out,
            9,
            &self.prior_ethereum_finalized_beacon_root,
        )?;
        pfusdc_append_u64(&mut out, 10, self.prior_ethereum_finalized_slot);
        pfusdc_append_hex(&mut out, 11, &self.ethereum_finalized_beacon_root)?;
        pfusdc_append_u64(&mut out, 12, self.ethereum_finalized_slot);
        pfusdc_append_u64(&mut out, 13, self.arbitrum_chain_id);
        pfusdc_append_evm_address(&mut out, 14, &self.arbitrum_rollup_address)?;
        pfusdc_append_hex(&mut out, 15, &self.arbitrum_assertion_hash)?;
        pfusdc_append_hex(&mut out, 16, &self.assertion_l2_block_hash)?;
        pfusdc_append_hex(&mut out, 17, &self.assertion_send_root)?;
        pfusdc_append_u64(&mut out, 18, self.output_index);
        pfusdc_append_hex(&mut out, 19, &self.output_item_hash)?;
        pfusdc_append_u64(&mut out, 20, self.output_l2_block_number);
        pfusdc_append_u64(&mut out, 21, self.output_l1_block_number);
        pfusdc_append_u64(&mut out, 22, self.output_timestamp);
        pfusdc_append_evm_address(&mut out, 23, &self.output_destination)?;
        pfusdc_append_hex(&mut out, 24, &self.output_calldata_hash)?;
        pfusdc_append_evm_address(&mut out, 25, &self.vault_address)?;
        pfusdc_append_hex(&mut out, 26, &self.vault_runtime_code_hash)?;
        pfusdc_append_evm_address(&mut out, 27, &self.token_address)?;
        pfusdc_append_hex(&mut out, 28, &self.token_runtime_code_hash)?;
        pfusdc_append_evm_address(&mut out, 29, &self.depositor)?;
        pfusdc_append_text(&mut out, 30, &self.pftl_recipient)?;
        pfusdc_append_hex(&mut out, 31, &self.pftl_recipient_hash)?;
        pfusdc_append_u64(&mut out, 32, self.amount_atoms);
        pfusdc_append_hex(&mut out, 33, &self.nonce)?;
        pfusdc_append_hex(&mut out, 34, &self.route_binding)?;
        pfusdc_append_hex(&mut out, 35, &self.deposit_id)?;
        pfusdc_append_hex(&mut out, 36, &self.evidence_root)?;
        pfusdc_append_hex(&mut out, 37, &self.arbitrum_rollup_runtime_code_hash)?;
        pfusdc_append_hex(
            &mut out,
            38,
            &self.rollup_latest_confirmed_storage_slot,
        )?;
        pfusdc_append_hex(&mut out, 39, &self.assertion_l2_state_root)?;
        pfusdc_append_evm_address(&mut out, 40, &self.output_sender)?;
        pfusdc_append_hex(&mut out, 41, &self.ingress_anchor_runtime_code_hash)?;
        Ok(out)
    }

    pub fn expected_commitment(&self) -> Result<String, String> {
        Ok(pfusdc_keccak_commitment(
            PFUSDC_INGRESS_COMMITMENT_DOMAIN_V3,
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
        if self.schema != PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3 {
            return Err("pfUSDC ingress public-values schema mismatch".to_string());
        }
        if self.proof_program_version != 3 || self.pftl_protocol_version == 0 {
            return Err("pfUSDC ingress program version must be 3 and chain version nonzero".to_string());
        }
        pfusdc_validate_text("pfusdc_ingress.pftl_chain_id", &self.pftl_chain_id)?;
        pfusdc_validate_hex_fields(&[
            ("pfusdc_ingress.pftl_genesis_hash", &self.pftl_genesis_hash, 96),
            ("pfusdc_ingress.route_profile_hash", &self.route_profile_hash, 96),
            ("pfusdc_ingress.prior_ethereum_finalized_beacon_root", &self.prior_ethereum_finalized_beacon_root, 64),
            ("pfusdc_ingress.ethereum_finalized_beacon_root", &self.ethereum_finalized_beacon_root, 64),
            ("pfusdc_ingress.arbitrum_assertion_hash", &self.arbitrum_assertion_hash, 64),
            ("pfusdc_ingress.assertion_l2_block_hash", &self.assertion_l2_block_hash, 64),
            ("pfusdc_ingress.assertion_send_root", &self.assertion_send_root, 64),
            ("pfusdc_ingress.output_item_hash", &self.output_item_hash, 64),
            ("pfusdc_ingress.output_calldata_hash", &self.output_calldata_hash, 64),
            ("pfusdc_ingress.vault_runtime_code_hash", &self.vault_runtime_code_hash, 64),
            ("pfusdc_ingress.token_runtime_code_hash", &self.token_runtime_code_hash, 64),
            ("pfusdc_ingress.pftl_recipient_hash", &self.pftl_recipient_hash, 64),
            ("pfusdc_ingress.nonce", &self.nonce, 64),
            ("pfusdc_ingress.route_binding", &self.route_binding, 64),
            ("pfusdc_ingress.deposit_id", &self.deposit_id, 64),
            ("pfusdc_ingress.evidence_root", &self.evidence_root, 96),
            ("pfusdc_ingress.arbitrum_rollup_runtime_code_hash", &self.arbitrum_rollup_runtime_code_hash, 64),
            ("pfusdc_ingress.rollup_latest_confirmed_storage_slot", &self.rollup_latest_confirmed_storage_slot, 64),
            ("pfusdc_ingress.assertion_l2_state_root", &self.assertion_l2_state_root, 64),
            ("pfusdc_ingress.ingress_anchor_runtime_code_hash", &self.ingress_anchor_runtime_code_hash, 64),
        ])?;
        if [
            &self.arbitrum_rollup_runtime_code_hash,
            &self.vault_runtime_code_hash,
            &self.token_runtime_code_hash,
            &self.ingress_anchor_runtime_code_hash,
        ]
        .iter()
        .any(|value| value.bytes().all(|byte| byte == b'0'))
        {
            return Err("pfUSDC ingress runtime code hashes must be nonzero".to_string());
        }
        if self.route_epoch == 0
            || self.ethereum_chain_id == 0
            || self.prior_ethereum_finalized_slot == 0
            || self.ethereum_finalized_slot == 0
            || self.arbitrum_chain_id == 0
            || self.output_l2_block_number == 0
            || self.output_l1_block_number == 0
            || self.output_timestamp == 0
            || self.amount_atoms == 0
        {
            return Err("pfUSDC ingress numeric/finality fields are invalid".to_string());
        }
        if self.ethereum_finalized_slot <= self.prior_ethereum_finalized_slot
            || self.ethereum_finalized_beacon_root
                == self.prior_ethereum_finalized_beacon_root
        {
            return Err("pfUSDC ingress finality checkpoint must strictly advance".to_string());
        }
        validate_evm_address_text("pfusdc_ingress.arbitrum_rollup_address", &self.arbitrum_rollup_address)?;
        validate_evm_address_text("pfusdc_ingress.output_sender", &self.output_sender)?;
        validate_evm_address_text("pfusdc_ingress.output_destination", &self.output_destination)?;
        validate_evm_address_text("pfusdc_ingress.vault_address", &self.vault_address)?;
        validate_evm_address_text("pfusdc_ingress.token_address", &self.token_address)?;
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

pub fn pfusdc_ingress_proof_hash_v1(proof: &[u8]) -> String {
    pfusdc_sha3_384_commitment(PFUSDC_INGRESS_PROOF_HASH_DOMAIN_V1, proof)
}

pub fn pfusdc_ingress_public_values_hash_v1(public_values: &[u8]) -> String {
    pfusdc_sha3_384_commitment(PFUSDC_INGRESS_PUBLIC_VALUES_HASH_DOMAIN_V1, public_values)
}

pub const ETHEREUM_ARBITRUM_FINALITY_STATE_SCHEMA_V2: &str =
    "postfiat.ethereum_arbitrum_finality_state.v2";
pub const MAX_RETAINED_ETHEREUM_ARBITRUM_CHECKPOINTS_V1: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumArbitrumCheckpointV1 {
    pub ethereum_finalized_beacon_root: String,
    pub ethereum_finalized_slot: u64,
    pub arbitrum_assertion_hash: String,
    pub assertion_l2_block_hash: String,
    pub assertion_send_root: String,
}

impl EthereumArbitrumCheckpointV1 {
    pub fn validate(&self) -> Result<(), String> {
        pfusdc_validate_hex_fields(&[
            (
                "pfusdc_finality.ethereum_finalized_beacon_root",
                &self.ethereum_finalized_beacon_root,
                64,
            ),
            (
                "pfusdc_finality.arbitrum_assertion_hash",
                &self.arbitrum_assertion_hash,
                64,
            ),
            ("pfusdc_finality.assertion_l2_block_hash", &self.assertion_l2_block_hash, 64),
            ("pfusdc_finality.assertion_send_root", &self.assertion_send_root, 64),
        ])?;
        if self.ethereum_finalized_slot == 0 {
            return Err("pfUSDC finality checkpoint slot must be nonzero".to_string());
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(200);
        pfusdc_append_hex(&mut bytes, 1, &self.ethereum_finalized_beacon_root)?;
        pfusdc_append_u64(&mut bytes, 2, self.ethereum_finalized_slot);
        pfusdc_append_hex(&mut bytes, 3, &self.arbitrum_assertion_hash)?;
        pfusdc_append_hex(&mut bytes, 4, &self.assertion_l2_block_hash)?;
        pfusdc_append_hex(&mut bytes, 5, &self.assertion_send_root)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumArbitrumFinalityStateV2 {
    pub schema: String,
    pub route_profile_hash: String,
    pub route_epoch: u64,
    pub ethereum_chain_id: u64,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: String,
    pub arbitrum_rollup_runtime_code_hash: String,
    pub rollup_latest_confirmed_storage_slot: String,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub ethereum_ingress_anchor_address: String,
    pub ethereum_ingress_anchor_runtime_code_hash: String,
    pub latest: EthereumArbitrumCheckpointV1,
    pub retained: Vec<EthereumArbitrumCheckpointV1>,
}

impl EthereumArbitrumFinalityStateV2 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != ETHEREUM_ARBITRUM_FINALITY_STATE_SCHEMA_V2 {
            return Err("pfUSDC finality-state schema mismatch".to_string());
        }
        validate_lower_hex_len(
            "pfusdc_finality.route_profile_hash",
            &self.route_profile_hash,
            96,
        )?;
        if self.route_epoch == 0 || self.ethereum_chain_id == 0 || self.arbitrum_chain_id == 0 {
            return Err("pfUSDC finality-state chain/route identifiers must be nonzero".to_string());
        }
        validate_evm_address_text(
            "pfusdc_finality.arbitrum_rollup_address",
            &self.arbitrum_rollup_address,
        )?;
        validate_lower_hex_len(
            "pfusdc_finality.arbitrum_rollup_runtime_code_hash",
            self.arbitrum_rollup_runtime_code_hash
                .strip_prefix("0x")
                .unwrap_or(&self.arbitrum_rollup_runtime_code_hash),
            64,
        )?;
        validate_lower_hex_len(
            "pfusdc_finality.rollup_latest_confirmed_storage_slot",
            &self.rollup_latest_confirmed_storage_slot,
            64,
        )?;
        validate_evm_address_text("pfusdc_finality.vault_address", &self.vault_address)?;
        validate_evm_address_text("pfusdc_finality.token_address", &self.token_address)?;
        validate_evm_address_text(
            "pfusdc_finality.ethereum_ingress_anchor_address",
            &self.ethereum_ingress_anchor_address,
        )?;
        for (field, value) in [
            (
                "pfusdc_finality.vault_runtime_code_hash",
                &self.vault_runtime_code_hash,
            ),
            (
                "pfusdc_finality.token_runtime_code_hash",
                &self.token_runtime_code_hash,
            ),
            (
                "pfusdc_finality.ethereum_ingress_anchor_runtime_code_hash",
                &self.ethereum_ingress_anchor_runtime_code_hash,
            ),
        ] {
            validate_lower_hex_len(field, value.strip_prefix("0x").unwrap_or(value), 64)?;
        }
        self.latest.validate()?;
        if self.retained.is_empty()
            || self.retained.len() > MAX_RETAINED_ETHEREUM_ARBITRUM_CHECKPOINTS_V1
        {
            return Err("pfUSDC finality-state retained checkpoint window is invalid".to_string());
        }
        let mut previous_slot = 0;
        for checkpoint in &self.retained {
            checkpoint.validate()?;
            if checkpoint.ethereum_finalized_slot <= previous_slot {
                return Err("pfUSDC retained finality checkpoints must strictly advance".to_string());
            }
            previous_slot = checkpoint.ethereum_finalized_slot;
        }
        if self.retained.last() != Some(&self.latest) {
            return Err("pfUSDC finality-state latest checkpoint must end retained window".to_string());
        }
        Ok(())
    }

    pub fn recognizes_checkpoint(&self, root: &str, slot: u64) -> bool {
        self.retained.iter().any(|checkpoint| {
            checkpoint.ethereum_finalized_slot == slot
                && checkpoint.ethereum_finalized_beacon_root == root
        })
    }

    pub fn verify_and_advance(
        &mut self,
        values: &PfUsdcIngressPublicValuesV3,
    ) -> Result<(), String> {
        self.validate()?;
        if values.route_profile_hash != self.route_profile_hash
            || values.route_epoch != self.route_epoch
            || values.ethereum_chain_id != self.ethereum_chain_id
            || values.arbitrum_chain_id != self.arbitrum_chain_id
            || values.arbitrum_rollup_address != self.arbitrum_rollup_address
            || values.arbitrum_rollup_runtime_code_hash
                != self
                    .arbitrum_rollup_runtime_code_hash
                    .strip_prefix("0x")
                    .unwrap_or(&self.arbitrum_rollup_runtime_code_hash)
            || values.rollup_latest_confirmed_storage_slot
                != self.rollup_latest_confirmed_storage_slot
            || values.vault_address != self.vault_address
            || values.output_sender != self.vault_address
            || values.vault_runtime_code_hash
                != self
                    .vault_runtime_code_hash
                    .strip_prefix("0x")
                    .unwrap_or(&self.vault_runtime_code_hash)
            || values.token_address != self.token_address
            || values.token_runtime_code_hash
                != self
                    .token_runtime_code_hash
                    .strip_prefix("0x")
                    .unwrap_or(&self.token_runtime_code_hash)
            || values.output_destination != self.ethereum_ingress_anchor_address
            || values.ingress_anchor_runtime_code_hash
                != self
                    .ethereum_ingress_anchor_runtime_code_hash
                    .strip_prefix("0x")
                    .unwrap_or(&self.ethereum_ingress_anchor_runtime_code_hash)
        {
            return Err("pfUSDC proof does not match its pinned finality-state route".to_string());
        }
        if !self.recognizes_checkpoint(
            &values.prior_ethereum_finalized_beacon_root,
            values.prior_ethereum_finalized_slot,
        ) {
            return Err("pfUSDC proof does not start from a retained checkpoint".to_string());
        }
        let resulting = EthereumArbitrumCheckpointV1 {
            ethereum_finalized_beacon_root: values.ethereum_finalized_beacon_root.clone(),
            ethereum_finalized_slot: values.ethereum_finalized_slot,
            arbitrum_assertion_hash: values.arbitrum_assertion_hash.clone(),
            assertion_l2_block_hash: values.assertion_l2_block_hash.clone(),
            assertion_send_root: values.assertion_send_root.clone(),
        };
        resulting.validate()?;
        if let Some(existing) = self.retained.iter().find(|checkpoint| {
            checkpoint.ethereum_finalized_slot == resulting.ethereum_finalized_slot
        }) {
            if existing != &resulting {
                return Err("pfUSDC proof conflicts with a retained finalized checkpoint".to_string());
            }
            return Ok(());
        }
        if resulting.ethereum_finalized_slot <= self.latest.ethereum_finalized_slot {
            return Err("pfUSDC proof checkpoint does not monotonically advance".to_string());
        }
        self.retained.push(resulting.clone());
        if self.retained.len() > MAX_RETAINED_ETHEREUM_ARBITRUM_CHECKPOINTS_V1 {
            let remove = self.retained.len() - MAX_RETAINED_ETHEREUM_ARBITRUM_CHECKPOINTS_V1;
            self.retained.drain(..remove);
        }
        self.latest = resulting;
        self.validate()
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = pfusdc_canonical_prefix(ETHEREUM_ARBITRUM_FINALITY_STATE_SCHEMA_V2);
        pfusdc_append_text(&mut bytes, 1, &self.schema)?;
        pfusdc_append_hex(&mut bytes, 2, &self.route_profile_hash)?;
        pfusdc_append_u64(&mut bytes, 3, self.route_epoch);
        pfusdc_append_u64(&mut bytes, 4, self.ethereum_chain_id);
        pfusdc_append_u64(&mut bytes, 5, self.arbitrum_chain_id);
        pfusdc_append_evm_address(&mut bytes, 6, &self.arbitrum_rollup_address)?;
        let code_hash = self
            .arbitrum_rollup_runtime_code_hash
            .strip_prefix("0x")
            .unwrap_or(&self.arbitrum_rollup_runtime_code_hash);
        pfusdc_append_hex(&mut bytes, 7, code_hash)?;
        pfusdc_append_hex(
            &mut bytes,
            8,
            &self.rollup_latest_confirmed_storage_slot,
        )?;
        pfusdc_append_evm_address(&mut bytes, 9, &self.vault_address)?;
        pfusdc_append_hex(
            &mut bytes,
            10,
            self.vault_runtime_code_hash
                .strip_prefix("0x")
                .unwrap_or(&self.vault_runtime_code_hash),
        )?;
        pfusdc_append_evm_address(&mut bytes, 11, &self.token_address)?;
        pfusdc_append_hex(
            &mut bytes,
            12,
            self.token_runtime_code_hash
                .strip_prefix("0x")
                .unwrap_or(&self.token_runtime_code_hash),
        )?;
        pfusdc_append_evm_address(
            &mut bytes,
            13,
            &self.ethereum_ingress_anchor_address,
        )?;
        pfusdc_append_hex(
            &mut bytes,
            14,
            self.ethereum_ingress_anchor_runtime_code_hash
                .strip_prefix("0x")
                .unwrap_or(&self.ethereum_ingress_anchor_runtime_code_hash),
        )?;
        let retained_len = u32::try_from(self.retained.len())
            .map_err(|_| "pfUSDC retained checkpoint count exceeds u32".to_string())?;
        pfusdc_append_u32(&mut bytes, 15, retained_len);
        for checkpoint in &self.retained {
            let checkpoint_bytes = checkpoint.canonical_bytes()?;
            pfusdc_append_field(&mut bytes, 16, &checkpoint_bytes)?;
        }
        Ok(bytes)
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

pub const PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1: &str =
    "postfiat.pfusdc.egress_proof_witness.v1";
pub const PFUSDC_CHECKPOINT_PROOF_WITNESS_SCHEMA_V1: &str =
    "postfiat.pfusdc.checkpoint_proof_witness.v1";
pub const PFUSDC_EGRESS_MAX_ANCESTRY_BLOCKS_V1: usize = 64;

/// One finalized block between the contract's prior checkpoint and the exit
/// block. Governance payload material is present only when this block proves
/// the committee authorized for the following block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcEgressFinalityStepV1 {
    pub block: BlockRecord,
    pub committee_epoch: u64,
    pub committee: Vec<ValidatorRegistryEntry>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub governance_payload_json: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next_committee: Vec<ValidatorRegistryEntry>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub next_committee_epoch: u64,
}

/// A proof-only checkpoint segment. Relayers submit these when no withdrawal
/// occurs within the bounded finality window, so a later withdrawal never
/// depends on artificial traffic or an unbounded guest witness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcCheckpointProofWitnessV1 {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub prior_checkpoint_block_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finality_ancestry: Vec<PfUsdcEgressFinalityStepV1>,
    pub block: BlockRecord,
    pub committee_epoch: u64,
    pub committee: Vec<ValidatorRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "witness", rename_all = "snake_case")]
pub enum PfUsdcEgressProgramInputV1 {
    Withdrawal(PfUsdcEgressProofWitnessV1),
    Checkpoint(PfUsdcCheckpointProofWitnessV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcCheckpointPublicValuesV1 {
    pub schema: String,
    pub proof_program_version: u32,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
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
    pub public_values_commitment: String,
}

impl PfUsdcCheckpointProofWitnessV1 {
    pub fn validate_bounds(&self) -> Result<(), String> {
        if self.schema != PFUSDC_CHECKPOINT_PROOF_WITNESS_SCHEMA_V1 {
            return Err("pfUSDC checkpoint proof witness schema mismatch".to_string());
        }
        pfusdc_validate_text("pfusdc_checkpoint_witness.chain_id", &self.chain_id)?;
        validate_lower_hex_len(
            "pfusdc_checkpoint_witness.genesis_hash",
            &self.genesis_hash,
            96,
        )?;
        validate_lower_hex_len(
            "pfusdc_checkpoint_witness.prior_checkpoint_block_id",
            &self.prior_checkpoint_block_id,
            96,
        )?;
        if self.protocol_version == 0
            || self.committee_epoch == 0
            || self.committee.is_empty()
            || self.committee.len() > 64
            || self.finality_ancestry.len() > PFUSDC_EGRESS_MAX_ANCESTRY_BLOCKS_V1
        {
            return Err("pfUSDC checkpoint proof witness bounds are invalid".to_string());
        }
        validate_finality_steps_v1(&self.finality_ancestry)
    }
}

fn validate_finality_steps_v1(steps: &[PfUsdcEgressFinalityStepV1]) -> Result<(), String> {
    for step in steps {
        if step.committee_epoch == 0
            || step.committee.is_empty()
            || step.committee.len() > 64
            || step.governance_payload_json.len() > 1_048_576
            || (step.next_committee.is_empty() != (step.next_committee_epoch == 0))
            || step.next_committee.len() > 64
        {
            return Err("pfUSDC finality step bounds are invalid".to_string());
        }
        if step.governance_payload_json.is_empty() && !step.next_committee.is_empty() {
            return Err("pfUSDC committee transition requires governance payload".to_string());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcEgressProofWitnessV1 {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub bridge_exit_root_activation_height: u64,
    pub prior_checkpoint_block_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finality_ancestry: Vec<PfUsdcEgressFinalityStepV1>,
    pub route_profile: VaultBridgeRouteProfileRecordV1,
    pub block: BlockRecord,
    pub receipt: Receipt,
    pub merkle_proof: BridgeExitMerkleProofV1,
    pub withdrawal_packet: VaultBridgeWithdrawalPacket,
    pub withdrawal_packet_hash: String,
    pub withdrawal_packet_evm_digest: String,
    pub committee_epoch: u64,
    pub committee: Vec<ValidatorRegistryEntry>,
}

impl PfUsdcEgressProofWitnessV1 {
    pub fn validate_bounds(&self) -> Result<(), String> {
        if self.schema != PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1 {
            return Err("pfUSDC egress proof witness schema mismatch".to_string());
        }
        pfusdc_validate_text("pfusdc_egress_witness.chain_id", &self.chain_id)?;
        validate_lower_hex_len(
            "pfusdc_egress_witness.genesis_hash",
            &self.genesis_hash,
            96,
        )?;
        validate_lower_hex_len(
            "pfusdc_egress_witness.prior_checkpoint_block_id",
            &self.prior_checkpoint_block_id,
            96,
        )?;
        if self.protocol_version == 0
            || self.bridge_exit_root_activation_height == 0
            || self.committee_epoch == 0
            || self.committee.is_empty()
            || self.committee.len() > 64
        {
            return Err("pfUSDC egress proof witness bounds are invalid".to_string());
        }
        if self.finality_ancestry.len() > PFUSDC_EGRESS_MAX_ANCESTRY_BLOCKS_V1 {
            return Err("pfUSDC egress finality ancestry exceeds the v1 bound".to_string());
        }
        validate_finality_steps_v1(&self.finality_ancestry)?;
        self.route_profile.validate()?;
        self.withdrawal_packet.validate()?;
        validate_lower_hex_len(
            "pfusdc_egress_witness.withdrawal_packet_hash",
            &self.withdrawal_packet_hash,
            96,
        )?;
        validate_lower_hex_len(
            "pfusdc_egress_witness.withdrawal_packet_evm_digest",
            &self.withdrawal_packet_evm_digest,
            64,
        )?;
        if self.merkle_proof.siblings.len() > 12 {
            return Err("pfUSDC egress Merkle proof exceeds bounded depth".to_string());
        }
        Ok(())
    }
}

impl PfUsdcCheckpointPublicValuesV1 {
    pub fn canonical_bytes_without_commitment(&self) -> Result<Vec<u8>, String> {
        self.validate_fields(false)?;
        let mut out = pfusdc_canonical_prefix(PFUSDC_CHECKPOINT_PUBLIC_VALUES_SCHEMA_V1);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_u32(&mut out, 2, self.proof_program_version);
        pfusdc_append_text(&mut out, 3, &self.pftl_chain_id)?;
        pfusdc_append_hex(&mut out, 4, &self.pftl_genesis_hash)?;
        pfusdc_append_u32(&mut out, 5, self.pftl_protocol_version);
        pfusdc_append_hex(&mut out, 6, &self.prior_checkpoint_block_id)?;
        pfusdc_append_hex(&mut out, 7, &self.resulting_checkpoint_block_id)?;
        pfusdc_append_u64(&mut out, 8, self.committee_epoch);
        pfusdc_append_hex(&mut out, 9, &self.committee_root)?;
        pfusdc_append_optional_hex(&mut out, 10, &self.committee_transition_commitment)?;
        pfusdc_append_u64(&mut out, 11, self.finalized_block_height);
        pfusdc_append_u64(&mut out, 12, self.finalized_block_view);
        pfusdc_append_hex(&mut out, 13, &self.finalized_block_id)?;
        pfusdc_append_hex(&mut out, 14, &self.finalized_parent_block_id)?;
        pfusdc_append_hex(&mut out, 15, &self.finalized_state_root)?;
        Ok(out)
    }

    pub fn expected_commitment(&self) -> Result<String, String> {
        Ok(pfusdc_keccak_commitment(
            PFUSDC_CHECKPOINT_COMMITMENT_DOMAIN_V1,
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
        if self.schema != PFUSDC_CHECKPOINT_PUBLIC_VALUES_SCHEMA_V1 {
            return Err("pfUSDC checkpoint public-values schema mismatch".to_string());
        }
        if self.proof_program_version == 0
            || self.pftl_protocol_version == 0
            || self.committee_epoch == 0
            || self.finalized_block_height == 0
        {
            return Err("pfUSDC checkpoint versions/finality are invalid".to_string());
        }
        pfusdc_validate_text("pfusdc_checkpoint.pftl_chain_id", &self.pftl_chain_id)?;
        pfusdc_validate_hex_fields(&[
            ("pfusdc_checkpoint.pftl_genesis_hash", &self.pftl_genesis_hash, 96),
            ("pfusdc_checkpoint.prior_checkpoint_block_id", &self.prior_checkpoint_block_id, 96),
            ("pfusdc_checkpoint.resulting_checkpoint_block_id", &self.resulting_checkpoint_block_id, 96),
            ("pfusdc_checkpoint.committee_root", &self.committee_root, 96),
            ("pfusdc_checkpoint.finalized_block_id", &self.finalized_block_id, 96),
            ("pfusdc_checkpoint.finalized_parent_block_id", &self.finalized_parent_block_id, 96),
            ("pfusdc_checkpoint.finalized_state_root", &self.finalized_state_root, 96),
        ])?;
        if !self.committee_transition_commitment.is_empty() {
            validate_lower_hex_len(
                "pfusdc_checkpoint.committee_transition_commitment",
                &self.committee_transition_commitment,
                96,
            )?;
        }
        if self.resulting_checkpoint_block_id != self.finalized_block_id {
            return Err("pfUSDC checkpoint result must equal finalized block".to_string());
        }
        if check_commitment {
            validate_lower_hex_len(
                "pfusdc_checkpoint.public_values_commitment",
                &self.public_values_commitment,
                64,
            )?;
            if self.public_values_commitment != self.expected_commitment()? {
                return Err("pfUSDC checkpoint public-values commitment mismatch".to_string());
            }
        }
        Ok(())
    }
}

pub fn pfusdc_egress_proof_nullifier_v1(
    route_epoch: u64,
    burn_tx_id: &str,
    withdrawal_id: &str,
    finalized_block_id: &str,
) -> Result<String, String> {
    if route_epoch == 0 {
        return Err("pfUSDC egress nullifier route epoch must be nonzero".to_string());
    }
    let burn = pfusdc_decode_hex(burn_tx_id)?;
    let withdrawal = pfusdc_decode_hex(withdrawal_id)?;
    let block = pfusdc_decode_hex(finalized_block_id)?;
    if burn.len() != 48 || withdrawal.len() != 48 || block.len() != 48 {
        return Err("pfUSDC egress nullifier identifiers must be 48 bytes".to_string());
    }
    let mut bytes = Vec::with_capacity(152);
    bytes.extend_from_slice(&route_epoch.to_be_bytes());
    bytes.extend_from_slice(&burn);
    bytes.extend_from_slice(&withdrawal);
    bytes.extend_from_slice(&block);
    Ok(pfusdc_keccak_commitment(
        PFUSDC_EGRESS_NULLIFIER_DOMAIN_V1,
        &bytes,
    ))
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

struct PfusdcCanonicalReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> PfusdcCanonicalReader<'a> {
    fn new(bytes: &'a [u8], expected_schema: &str) -> Result<Self, String> {
        let prefix_len = PFUSDC_TIER4_CANONICAL_MAGIC
            .len()
            .checked_add(4)
            .ok_or_else(|| "pfUSDC canonical prefix overflow".to_string())?;
        if bytes.len() < prefix_len
            || &bytes[..PFUSDC_TIER4_CANONICAL_MAGIC.len()] != PFUSDC_TIER4_CANONICAL_MAGIC
        {
            return Err("pfUSDC canonical magic mismatch".to_string());
        }
        let len_offset = PFUSDC_TIER4_CANONICAL_MAGIC.len();
        let schema_len = u32::from_be_bytes(
            bytes[len_offset..len_offset + 4]
                .try_into()
                .map_err(|_| "pfUSDC canonical schema length decode failed".to_string())?,
        ) as usize;
        let schema_end = prefix_len
            .checked_add(schema_len)
            .ok_or_else(|| "pfUSDC canonical schema length overflow".to_string())?;
        if schema_end > bytes.len() || &bytes[prefix_len..schema_end] != expected_schema.as_bytes() {
            return Err("pfUSDC canonical schema prefix mismatch".to_string());
        }
        Ok(Self {
            bytes,
            cursor: schema_end,
        })
    }

    fn field(&mut self, expected_tag: u16) -> Result<&'a [u8], String> {
        let header_end = self
            .cursor
            .checked_add(6)
            .ok_or_else(|| "pfUSDC canonical field header overflow".to_string())?;
        if header_end > self.bytes.len() {
            return Err("pfUSDC canonical field header truncated".to_string());
        }
        let tag = u16::from_be_bytes(
            self.bytes[self.cursor..self.cursor + 2]
                .try_into()
                .map_err(|_| "pfUSDC canonical tag decode failed".to_string())?,
        );
        if tag != expected_tag {
            return Err(format!(
                "pfUSDC canonical tag mismatch: expected {expected_tag}, got {tag}"
            ));
        }
        let len = u32::from_be_bytes(
            self.bytes[self.cursor + 2..header_end]
                .try_into()
                .map_err(|_| "pfUSDC canonical length decode failed".to_string())?,
        ) as usize;
        let end = header_end
            .checked_add(len)
            .ok_or_else(|| "pfUSDC canonical field length overflow".to_string())?;
        if end > self.bytes.len() {
            return Err("pfUSDC canonical field truncated".to_string());
        }
        self.cursor = end;
        Ok(&self.bytes[header_end..end])
    }

    fn text(&mut self, tag: u16) -> Result<String, String> {
        let value = std::str::from_utf8(self.field(tag)?)
            .map_err(|_| "pfUSDC canonical text is not UTF-8".to_string())?
            .to_string();
        pfusdc_validate_text("pfUSDC canonical text", &value)?;
        Ok(value)
    }

    fn hex(&mut self, tag: u16, exact_len: usize) -> Result<String, String> {
        let value = self.field(tag)?;
        if value.len() != exact_len {
            return Err(format!(
                "pfUSDC canonical tag {tag} must contain exactly {exact_len} bytes"
            ));
        }
        Ok(bytes_to_hex(value))
    }

    fn evm_address(&mut self, tag: u16) -> Result<String, String> {
        Ok(format!("0x{}", self.hex(tag, 20)?))
    }

    fn u32(&mut self, tag: u16) -> Result<u32, String> {
        let value = self.field(tag)?;
        Ok(u32::from_be_bytes(value.try_into().map_err(|_| {
            format!("pfUSDC canonical tag {tag} must contain four bytes")
        })?))
    }

    fn u64(&mut self, tag: u16) -> Result<u64, String> {
        let value = self.field(tag)?;
        Ok(u64::from_be_bytes(value.try_into().map_err(|_| {
            format!("pfUSDC canonical tag {tag} must contain eight bytes")
        })?))
    }

    fn finish(self) -> Result<(), String> {
        if self.cursor != self.bytes.len() {
            return Err("pfUSDC canonical public values contain trailing bytes".to_string());
        }
        Ok(())
    }
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
