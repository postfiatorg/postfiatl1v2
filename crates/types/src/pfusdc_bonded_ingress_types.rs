pub const PFUSDC_BONDED_INGRESS_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.pfusdc.bonded_ingress_public_values.v1";
const PFUSDC_BONDED_INGRESS_COMMITMENT_DOMAIN_V1: &str =
    "postfiat.pfusdc.bonded_ingress_public_values.commitment.v1";
pub const PFUSDC_FAST_INGRESS_DEPOSIT_KEY_DOMAIN_V1: &str = "PFTL_FAST_INGRESS_DEPOSIT_V1";
pub const PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1: &str =
    "postfiat.pfusdc.bonded_lifecycle_public_values.v1";
const PFUSDC_BONDED_LIFECYCLE_COMMITMENT_DOMAIN_V1: &str =
    "postfiat.pfusdc.bonded_lifecycle_public_values.commitment.v1";
pub const PFUSDC_BONDED_LIFECYCLE_UPDATE_CONFIRMED: &str = "CONFIRMED";
pub const PFUSDC_BONDED_LIFECYCLE_UPDATE_REVERTED: &str = "REVERTED";
pub const PFUSDC_FAST_INGRESS_VERIFIER_CONFIG_SCHEMA_V1: &str =
    "postfiat.pfusdc.fast_ingress_verifier_config.v1";

/// Secondary ingress verifier bound to an existing Tier-4 route profile.
///
/// Keeping this authority separate preserves the route-profile hash and epoch
/// already committed by the Ethereum anchor and Arbitrum egress verifier. The
/// confirmed ingress and egress route therefore remain usable while this
/// additional proof kind coexists on PFTL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastIngressVerifierConfigV1 {
    pub schema: String,
    pub base_route_profile_hash: String,
    pub route_epoch: u64,
    pub deployment_manifest_hash: String,
    pub asset_id: String,
    pub cap_atoms: u64,
    pub age_margin_blocks: u64,
    pub verifier_kind: String,
    pub verifier_policy_hash: String,
    pub verifier_program_vkey: String,
    pub verifier_proof_encoding: String,
    pub max_proof_bytes: u64,
    pub max_public_values_bytes: u64,
}

impl FastIngressVerifierConfigV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PFUSDC_FAST_INGRESS_VERIFIER_CONFIG_SCHEMA_V1
            || self.route_epoch == 0
            || self.cap_atoms == 0
            || self.age_margin_blocks < 64
            || self.verifier_kind != crate::NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
            || self.verifier_proof_encoding != "groth16"
            || self.max_proof_bytes == 0
            || self.max_public_values_bytes == 0
        {
            return Err("fast-ingress verifier configuration is invalid".to_string());
        }
        pfusdc_validate_hex_fields(&[
            (
                "fast_ingress_config.base_route_profile_hash",
                &self.base_route_profile_hash,
                96,
            ),
            (
                "fast_ingress_config.deployment_manifest_hash",
                &self.deployment_manifest_hash,
                64,
            ),
            (
                "fast_ingress_config.verifier_policy_hash",
                &self.verifier_policy_hash,
                64,
            ),
        ])?;
        let vkey = self.verifier_program_vkey.strip_prefix("0x").ok_or_else(|| {
            "fast-ingress verifier program vkey must have a 0x prefix".to_string()
        })?;
        validate_lower_hex_len("fast_ingress_config.verifier_program_vkey", vkey, 64)?;
        pfusdc_validate_text("fast_ingress_config.asset_id", &self.asset_id)?;
        Ok(())
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self)
            .map_err(|error| format!("encode fast-ingress verifier config: {error}"))
    }
}

/// Canonical public output of the optimistic/bonded pfUSDC ingress guest.
///
/// `source_assertion_id` is an Ethereum-authenticated, currently bonded
/// RollupCore assertion. It is intentionally named separately from confirmed
/// finality so callers cannot accidentally present this path as settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcBondedIngressPublicValuesV1 {
    pub schema: String,
    pub proof_program_version: u32,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
    pub route_id: String,
    pub route_profile_hash: String,
    pub route_epoch: u64,
    pub manifest_hash: String,
    pub verifier_policy_hash: String,
    pub cap_atoms: u64,
    pub age_margin_blocks: u64,
    pub ethereum_chain_id: u64,
    pub prior_ethereum_finalized_beacon_root: String,
    pub prior_ethereum_finalized_slot: u64,
    pub ethereum_finalized_beacon_root: String,
    pub ethereum_finalized_slot: u64,
    pub ethereum_finalized_execution_block_number: u64,
    pub ethereum_finalized_execution_block_hash: String,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: String,
    pub arbitrum_rollup_runtime_code_hash: String,
    pub assertion_protocol_adapter_id: String,
    pub latest_confirmed_assertion_id: String,
    pub source_assertion_id: String,
    pub source_assertion_parent_id: String,
    pub source_assertion_created_at_l1_block: u64,
    pub source_assertion_confirmation_period_blocks: u64,
    pub source_assertion_l2_block_number: u64,
    pub source_assertion_l2_block_hash: String,
    pub source_assertion_l2_state_root: String,
    pub source_assertion_send_root: String,
    pub source_staker: String,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub asset_id: String,
    pub depositor: String,
    pub pftl_recipient: String,
    pub pftl_recipient_hash: String,
    pub amount_atoms: u64,
    pub deposit_nonce: String,
    pub route_binding: String,
    pub deposit_id: String,
    pub deposit_key: String,
    pub deposit_seen_storage_slot: String,
    pub evidence_root: String,
    pub public_values_commitment: String,
}

impl PfUsdcBondedIngressPublicValuesV1 {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = PfusdcCanonicalReader::new(
            bytes,
            PFUSDC_BONDED_INGRESS_PUBLIC_VALUES_SCHEMA_V1,
        )?;
        let mut values = Self {
            schema: reader.text(1)?,
            proof_program_version: reader.u32(2)?,
            pftl_chain_id: reader.text(3)?,
            pftl_genesis_hash: reader.hex(4, 48)?,
            pftl_protocol_version: reader.u32(5)?,
            route_id: reader.text(6)?,
            route_profile_hash: reader.hex(7, 48)?,
            route_epoch: reader.u64(8)?,
            ethereum_chain_id: reader.u64(9)?,
            prior_ethereum_finalized_beacon_root: reader.hex(10, 32)?,
            prior_ethereum_finalized_slot: reader.u64(11)?,
            ethereum_finalized_beacon_root: reader.hex(12, 32)?,
            ethereum_finalized_slot: reader.u64(13)?,
            arbitrum_chain_id: reader.u64(14)?,
            arbitrum_rollup_address: reader.evm_address(15)?,
            arbitrum_rollup_runtime_code_hash: reader.hex(16, 32)?,
            source_assertion_id: reader.hex(17, 32)?,
            source_assertion_created_at_l1_block: reader.u64(18)?,
            source_assertion_l2_block_hash: reader.hex(19, 32)?,
            source_assertion_l2_state_root: reader.hex(20, 32)?,
            source_assertion_send_root: reader.hex(21, 32)?,
            source_staker: reader.evm_address(22)?,
            vault_address: reader.evm_address(23)?,
            vault_runtime_code_hash: reader.hex(24, 32)?,
            token_address: reader.evm_address(25)?,
            token_runtime_code_hash: reader.hex(26, 32)?,
            depositor: reader.evm_address(27)?,
            pftl_recipient: reader.text(28)?,
            pftl_recipient_hash: reader.hex(29, 32)?,
            amount_atoms: reader.u64(30)?,
            deposit_nonce: reader.hex(31, 32)?,
            route_binding: reader.hex(32, 32)?,
            deposit_id: reader.hex(33, 32)?,
            deposit_seen_storage_slot: reader.hex(34, 32)?,
            evidence_root: reader.hex(35, 48)?,
            manifest_hash: reader.hex(36, 32)?,
            ethereum_finalized_execution_block_number: reader.u64(37)?,
            ethereum_finalized_execution_block_hash: reader.hex(38, 32)?,
            assertion_protocol_adapter_id: reader.text(39)?,
            latest_confirmed_assertion_id: reader.hex(40, 32)?,
            source_assertion_parent_id: reader.hex(41, 32)?,
            source_assertion_confirmation_period_blocks: reader.u64(42)?,
            source_assertion_l2_block_number: reader.u64(43)?,
            asset_id: reader.text(44)?,
            deposit_key: reader.hex(45, 32)?,
            cap_atoms: reader.u64(46)?,
            age_margin_blocks: reader.u64(47)?,
            verifier_policy_hash: reader.hex(48, 32)?,
            public_values_commitment: String::new(),
        };
        reader.finish()?;
        values.seal()?;
        values.validate()?;
        if values.canonical_bytes_without_commitment()? != bytes {
            return Err("pfUSDC bonded-ingress public values are not canonical".to_string());
        }
        Ok(values)
    }

    pub fn canonical_bytes_without_commitment(&self) -> Result<Vec<u8>, String> {
        self.validate_fields(false)?;
        let mut out = pfusdc_canonical_prefix(PFUSDC_BONDED_INGRESS_PUBLIC_VALUES_SCHEMA_V1);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_u32(&mut out, 2, self.proof_program_version);
        pfusdc_append_text(&mut out, 3, &self.pftl_chain_id)?;
        pfusdc_append_hex(&mut out, 4, &self.pftl_genesis_hash)?;
        pfusdc_append_u32(&mut out, 5, self.pftl_protocol_version);
        pfusdc_append_text(&mut out, 6, &self.route_id)?;
        pfusdc_append_hex(&mut out, 7, &self.route_profile_hash)?;
        pfusdc_append_u64(&mut out, 8, self.route_epoch);
        pfusdc_append_u64(&mut out, 9, self.ethereum_chain_id);
        pfusdc_append_hex(&mut out, 10, &self.prior_ethereum_finalized_beacon_root)?;
        pfusdc_append_u64(&mut out, 11, self.prior_ethereum_finalized_slot);
        pfusdc_append_hex(&mut out, 12, &self.ethereum_finalized_beacon_root)?;
        pfusdc_append_u64(&mut out, 13, self.ethereum_finalized_slot);
        pfusdc_append_u64(&mut out, 14, self.arbitrum_chain_id);
        pfusdc_append_evm_address(&mut out, 15, &self.arbitrum_rollup_address)?;
        pfusdc_append_hex(&mut out, 16, &self.arbitrum_rollup_runtime_code_hash)?;
        pfusdc_append_hex(&mut out, 17, &self.source_assertion_id)?;
        pfusdc_append_u64(&mut out, 18, self.source_assertion_created_at_l1_block);
        pfusdc_append_hex(&mut out, 19, &self.source_assertion_l2_block_hash)?;
        pfusdc_append_hex(&mut out, 20, &self.source_assertion_l2_state_root)?;
        pfusdc_append_hex(&mut out, 21, &self.source_assertion_send_root)?;
        pfusdc_append_evm_address(&mut out, 22, &self.source_staker)?;
        pfusdc_append_evm_address(&mut out, 23, &self.vault_address)?;
        pfusdc_append_hex(&mut out, 24, &self.vault_runtime_code_hash)?;
        pfusdc_append_evm_address(&mut out, 25, &self.token_address)?;
        pfusdc_append_hex(&mut out, 26, &self.token_runtime_code_hash)?;
        pfusdc_append_evm_address(&mut out, 27, &self.depositor)?;
        pfusdc_append_text(&mut out, 28, &self.pftl_recipient)?;
        pfusdc_append_hex(&mut out, 29, &self.pftl_recipient_hash)?;
        pfusdc_append_u64(&mut out, 30, self.amount_atoms);
        pfusdc_append_hex(&mut out, 31, &self.deposit_nonce)?;
        pfusdc_append_hex(&mut out, 32, &self.route_binding)?;
        pfusdc_append_hex(&mut out, 33, &self.deposit_id)?;
        pfusdc_append_hex(&mut out, 34, &self.deposit_seen_storage_slot)?;
        pfusdc_append_hex(&mut out, 35, &self.evidence_root)?;
        pfusdc_append_hex(&mut out, 36, &self.manifest_hash)?;
        pfusdc_append_u64(
            &mut out,
            37,
            self.ethereum_finalized_execution_block_number,
        );
        pfusdc_append_hex(
            &mut out,
            38,
            &self.ethereum_finalized_execution_block_hash,
        )?;
        pfusdc_append_text(&mut out, 39, &self.assertion_protocol_adapter_id)?;
        pfusdc_append_hex(&mut out, 40, &self.latest_confirmed_assertion_id)?;
        pfusdc_append_hex(&mut out, 41, &self.source_assertion_parent_id)?;
        pfusdc_append_u64(
            &mut out,
            42,
            self.source_assertion_confirmation_period_blocks,
        );
        pfusdc_append_u64(&mut out, 43, self.source_assertion_l2_block_number);
        pfusdc_append_text(&mut out, 44, &self.asset_id)?;
        pfusdc_append_hex(&mut out, 45, &self.deposit_key)?;
        pfusdc_append_u64(&mut out, 46, self.cap_atoms);
        pfusdc_append_u64(&mut out, 47, self.age_margin_blocks);
        pfusdc_append_hex(&mut out, 48, &self.verifier_policy_hash)?;
        Ok(out)
    }

    pub fn seal(&mut self) -> Result<(), String> {
        self.public_values_commitment = self.expected_commitment()?;
        Ok(())
    }

    pub fn expected_commitment(&self) -> Result<String, String> {
        Ok(pfusdc_keccak_commitment(
            PFUSDC_BONDED_INGRESS_COMMITMENT_DOMAIN_V1,
            &self.canonical_bytes_without_commitment()?,
        ))
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_fields(true)
    }

    fn validate_fields(&self, check_commitment: bool) -> Result<(), String> {
        if self.schema != PFUSDC_BONDED_INGRESS_PUBLIC_VALUES_SCHEMA_V1
            || self.proof_program_version != 1
            || self.pftl_protocol_version == 0
        {
            return Err("pfUSDC bonded-ingress schema/program version mismatch".to_string());
        }
        for (field, value) in [
            ("pftl_chain_id", &self.pftl_chain_id),
            ("route_id", &self.route_id),
            ("pftl_recipient", &self.pftl_recipient),
            ("assertion_protocol_adapter_id", &self.assertion_protocol_adapter_id),
            ("asset_id", &self.asset_id),
        ] {
            pfusdc_validate_text(field, value)?;
        }
        pfusdc_validate_hex_fields(&[
            ("pftl_genesis_hash", &self.pftl_genesis_hash, 96),
            ("route_profile_hash", &self.route_profile_hash, 96),
            ("manifest_hash", &self.manifest_hash, 64),
            ("verifier_policy_hash", &self.verifier_policy_hash, 64),
            ("prior_ethereum_finalized_beacon_root", &self.prior_ethereum_finalized_beacon_root, 64),
            ("ethereum_finalized_beacon_root", &self.ethereum_finalized_beacon_root, 64),
            ("ethereum_finalized_execution_block_hash", &self.ethereum_finalized_execution_block_hash, 64),
            ("arbitrum_rollup_runtime_code_hash", &self.arbitrum_rollup_runtime_code_hash, 64),
            ("source_assertion_id", &self.source_assertion_id, 64),
            ("latest_confirmed_assertion_id", &self.latest_confirmed_assertion_id, 64),
            ("source_assertion_parent_id", &self.source_assertion_parent_id, 64),
            ("source_assertion_l2_block_hash", &self.source_assertion_l2_block_hash, 64),
            ("source_assertion_l2_state_root", &self.source_assertion_l2_state_root, 64),
            ("source_assertion_send_root", &self.source_assertion_send_root, 64),
            ("vault_runtime_code_hash", &self.vault_runtime_code_hash, 64),
            ("token_runtime_code_hash", &self.token_runtime_code_hash, 64),
            ("pftl_recipient_hash", &self.pftl_recipient_hash, 64),
            ("deposit_nonce", &self.deposit_nonce, 64),
            ("route_binding", &self.route_binding, 64),
            ("deposit_id", &self.deposit_id, 64),
            ("deposit_key", &self.deposit_key, 64),
            ("deposit_seen_storage_slot", &self.deposit_seen_storage_slot, 64),
            ("evidence_root", &self.evidence_root, 96),
        ])?;
        for (field, value) in [
            ("arbitrum_rollup_address", &self.arbitrum_rollup_address),
            ("source_staker", &self.source_staker),
            ("vault_address", &self.vault_address),
            ("token_address", &self.token_address),
            ("depositor", &self.depositor),
        ] {
            validate_evm_address_text(field, value)?;
        }
        if self.route_epoch == 0
            || self.ethereum_chain_id == 0
            || self.prior_ethereum_finalized_slot == 0
            || self.ethereum_finalized_slot <= self.prior_ethereum_finalized_slot
            || self.ethereum_finalized_execution_block_number == 0
            || self.arbitrum_chain_id == 0
            || self.source_assertion_created_at_l1_block == 0
            || self.source_assertion_confirmation_period_blocks == 0
            || self.source_assertion_l2_block_number == 0
            || self.amount_atoms == 0
            || self.cap_atoms == 0
            || self.amount_atoms > self.cap_atoms
            || self.age_margin_blocks < 64
        {
            return Err("pfUSDC bonded-ingress numeric field is invalid".to_string());
        }
        if check_commitment && self.public_values_commitment != self.expected_commitment()? {
            return Err("pfUSDC bonded-ingress commitment mismatch".to_string());
        }
        if self.deposit_key
            != pfusdc_fast_ingress_deposit_key_v1(
                self.arbitrum_chain_id,
                &self.vault_address,
                &self.deposit_id,
            )?
        {
            return Err("pfUSDC bonded-ingress deposit replay key mismatch".to_string());
        }
        Ok(())
    }
}

pub const FAST_INGRESS_CAMPAIGN_SCHEMA_V1: &str = "postfiat.pfusdc.fast_ingress_campaign.v1";
pub const FAST_INGRESS_MINT_STATUS_ESCROWED: &str = "ESCROWED";
pub const FAST_INGRESS_MINT_STATUS_RELEASED_UNCONFIRMED: &str = "RELEASED_UNCONFIRMED";
pub const FAST_INGRESS_MINT_STATUS_FINAL: &str = "FINAL";
pub const FAST_INGRESS_MINT_STATUS_REVERTED_ESCROWED: &str = "REVERTED_ESCROWED";
pub const FAST_INGRESS_MINT_STATUS_REVERTED_RELEASED: &str = "REVERTED_RELEASED";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastIngressMintRecordV1 {
    pub mint_id: String,
    pub deposit_key: String,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub deposit_id: String,
    pub amount_atoms: u64,
    pub recipient: String,
    pub route_id: String,
    pub route_epoch: u64,
    pub source_assertion_id: String,
    pub initial_latest_confirmed_assertion_id: String,
    pub source_l1_block_hash: String,
    pub source_l2_block_hash: String,
    pub accepted_height: u64,
    pub status: String,
    #[serde(default)]
    pub claimed: bool,
}

impl FastIngressMintRecordV1 {
    pub fn validate(&self) -> Result<(), String> {
        pfusdc_validate_hex_fields(&[
            ("fast_ingress.mint_id", &self.mint_id, 64),
            ("fast_ingress.deposit_key", &self.deposit_key, 64),
            ("fast_ingress.deposit_id", &self.deposit_id, 64),
            ("fast_ingress.source_assertion_id", &self.source_assertion_id, 64),
            ("fast_ingress.initial_latest_confirmed_assertion_id", &self.initial_latest_confirmed_assertion_id, 64),
            ("fast_ingress.source_l1_block_hash", &self.source_l1_block_hash, 64),
            ("fast_ingress.source_l2_block_hash", &self.source_l2_block_hash, 64),
        ])?;
        validate_evm_address_text("fast_ingress.vault_address", &self.vault_address)?;
        for (field, value) in [
            ("fast_ingress.recipient", &self.recipient),
            ("fast_ingress.route_id", &self.route_id),
        ] {
            pfusdc_validate_text(field, value)?;
        }
        if self.source_chain_id == 0
            || self.amount_atoms == 0
            || self.route_epoch == 0
            || self.accepted_height == 0
            || !matches!(
                self.status.as_str(),
                FAST_INGRESS_MINT_STATUS_ESCROWED
                    | FAST_INGRESS_MINT_STATUS_RELEASED_UNCONFIRMED
                    | FAST_INGRESS_MINT_STATUS_FINAL
                    | FAST_INGRESS_MINT_STATUS_REVERTED_ESCROWED
                    | FAST_INGRESS_MINT_STATUS_REVERTED_RELEASED
            )
            || (self.claimed
                && !matches!(
                    self.status.as_str(),
                    FAST_INGRESS_MINT_STATUS_RELEASED_UNCONFIRMED
                        | FAST_INGRESS_MINT_STATUS_FINAL
                        | FAST_INGRESS_MINT_STATUS_REVERTED_RELEASED
                ))
        {
            return Err("fast-ingress mint record is invalid".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastIngressCampaignStateV1 {
    pub schema: String,
    pub route_profile_hash: String,
    pub route_epoch: u64,
    pub manifest_hash: String,
    pub asset_id: String,
    pub cap_atoms: u64,
    pub age_margin_blocks: u64,
    #[serde(default)]
    pub age_release_enabled: bool,
    pub exposure_total_atoms: u64,
    #[serde(default)]
    pub paused: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mints: Vec<FastIngressMintRecordV1>,
}

impl FastIngressCampaignStateV1 {
    pub fn from_public_values(values: &PfUsdcBondedIngressPublicValuesV1) -> Result<Self, String> {
        values.validate()?;
        let state = Self {
            schema: FAST_INGRESS_CAMPAIGN_SCHEMA_V1.to_string(),
            route_profile_hash: values.route_profile_hash.clone(),
            route_epoch: values.route_epoch,
            manifest_hash: values.manifest_hash.clone(),
            asset_id: values.asset_id.clone(),
            cap_atoms: values.cap_atoms,
            age_margin_blocks: values.age_margin_blocks,
            // Fail closed at launch: the demo can only release via an
            // authenticated confirmation proof until a separately reviewed
            // no-active-challenge age proof exists.
            age_release_enabled: false,
            exposure_total_atoms: 0,
            paused: false,
            mints: Vec::new(),
        };
        state.validate()?;
        Ok(state)
    }

    pub fn accept_escrowed_mint(
        &mut self,
        values: &PfUsdcBondedIngressPublicValuesV1,
        accepted_height: u64,
    ) -> Result<String, String> {
        self.validate()?;
        values.validate()?;
        if self.paused
            || accepted_height == 0
            || self.route_profile_hash != values.route_profile_hash
            || self.route_epoch != values.route_epoch
            || self.manifest_hash != values.manifest_hash
            || self.asset_id != values.asset_id
            || self.cap_atoms != values.cap_atoms
            || self.age_margin_blocks != values.age_margin_blocks
        {
            return Err("fast-ingress proof does not match its campaign".to_string());
        }
        if self.mints.iter().any(|mint| mint.deposit_key == values.deposit_key) {
            return Err("fast-ingress deposit replay key is already consumed".to_string());
        }
        let exposure_after = self
            .exposure_total_atoms
            .checked_add(values.amount_atoms)
            .ok_or_else(|| "fast-ingress exposure overflow".to_string())?;
        if exposure_after > self.cap_atoms {
            return Err("fast-ingress campaign cap exceeded".to_string());
        }
        let mint_id = pfusdc_keccak_commitment(
            "PFTL_FAST_INGRESS_MINT_V1",
            values.deposit_key.as_bytes(),
        );
        let record = FastIngressMintRecordV1 {
            mint_id: mint_id.clone(),
            deposit_key: values.deposit_key.clone(),
            source_chain_id: values.arbitrum_chain_id,
            vault_address: values.vault_address.clone(),
            deposit_id: values.deposit_id.clone(),
            amount_atoms: values.amount_atoms,
            recipient: values.pftl_recipient.clone(),
            route_id: values.route_id.clone(),
            route_epoch: values.route_epoch,
            source_assertion_id: values.source_assertion_id.clone(),
            initial_latest_confirmed_assertion_id: values.latest_confirmed_assertion_id.clone(),
            source_l1_block_hash: values.ethereum_finalized_execution_block_hash.clone(),
            source_l2_block_hash: values.source_assertion_l2_block_hash.clone(),
            accepted_height,
            status: FAST_INGRESS_MINT_STATUS_ESCROWED.to_string(),
            claimed: false,
        };
        record.validate()?;
        self.mints.push(record);
        self.exposure_total_atoms = exposure_after;
        self.validate()?;
        Ok(mint_id)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FAST_INGRESS_CAMPAIGN_SCHEMA_V1
            || self.route_epoch == 0
            || self.cap_atoms == 0
            || self.age_margin_blocks < 64
            || self.exposure_total_atoms > self.cap_atoms
        {
            return Err("fast-ingress campaign configuration is invalid".to_string());
        }
        pfusdc_validate_hex_fields(&[
            ("fast_ingress.route_profile_hash", &self.route_profile_hash, 96),
            ("fast_ingress.manifest_hash", &self.manifest_hash, 64),
        ])?;
        pfusdc_validate_text("fast_ingress.asset_id", &self.asset_id)?;
        let mut keys = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        let mut exposure = 0_u64;
        for mint in &self.mints {
            mint.validate()?;
            if mint.route_epoch != self.route_epoch
                || !keys.insert(&mint.deposit_key)
                || !ids.insert(&mint.mint_id)
            {
                return Err("fast-ingress campaign contains a duplicate or mismatched mint".to_string());
            }
            if mint.status != FAST_INGRESS_MINT_STATUS_FINAL {
                exposure = exposure
                    .checked_add(mint.amount_atoms)
                    .ok_or_else(|| "fast-ingress exposure overflow".to_string())?;
            }
        }
        if exposure != self.exposure_total_atoms {
            return Err("fast-ingress exposure invariant mismatch".to_string());
        }
        Ok(())
    }

    pub fn apply_confirmation(
        &mut self,
        values: &PfUsdcBondedLifecyclePublicValuesV1,
    ) -> Result<u64, String> {
        self.validate()?;
        values.validate()?;
        if self.route_profile_hash != values.route_profile_hash
            || self.route_epoch != values.route_epoch
            || self.manifest_hash != values.manifest_hash
            || values.update_kind != PFUSDC_BONDED_LIFECYCLE_UPDATE_CONFIRMED
        {
            return Err("fast-ingress confirmation does not match its campaign".to_string());
        }
        let mut released = 0_u64;
        let mut matched = false;
        for mint in self
            .mints
            .iter_mut()
            .filter(|mint| mint.source_assertion_id == values.source_assertion_id)
        {
            matched = true;
            match mint.status.as_str() {
                FAST_INGRESS_MINT_STATUS_ESCROWED
                | FAST_INGRESS_MINT_STATUS_RELEASED_UNCONFIRMED => {
                    mint.status = FAST_INGRESS_MINT_STATUS_FINAL.to_string();
                    released = released
                        .checked_add(mint.amount_atoms)
                        .ok_or_else(|| "fast-ingress confirmation overflow".to_string())?;
                }
                FAST_INGRESS_MINT_STATUS_FINAL => {}
                FAST_INGRESS_MINT_STATUS_REVERTED_ESCROWED
                | FAST_INGRESS_MINT_STATUS_REVERTED_RELEASED => {
                    return Err("cannot confirm a reverted fast-ingress assertion".to_string());
                }
                _ => return Err("unknown fast-ingress mint status".to_string()),
            }
        }
        if !matched {
            return Err("fast-ingress confirmation has no associated mints".to_string());
        }
        if released == 0 {
            return Err("fast-ingress confirmation was already applied".to_string());
        }
        self.exposure_total_atoms = self
            .exposure_total_atoms
            .checked_sub(released)
            .ok_or_else(|| "fast-ingress confirmation underflows exposure".to_string())?;
        self.validate()?;
        Ok(released)
    }

    pub fn apply_reversion(
        &mut self,
        values: &PfUsdcBondedLifecyclePublicValuesV1,
    ) -> Result<u64, String> {
        self.validate()?;
        values.validate()?;
        if self.route_profile_hash != values.route_profile_hash
            || self.route_epoch != values.route_epoch
            || self.manifest_hash != values.manifest_hash
            || values.update_kind != PFUSDC_BONDED_LIFECYCLE_UPDATE_REVERTED
        {
            return Err("fast-ingress reversion does not match its campaign".to_string());
        }
        let mut affected = 0_u64;
        let mut matched = false;
        for mint in self
            .mints
            .iter_mut()
            .filter(|mint| mint.source_assertion_id == values.source_assertion_id)
        {
            matched = true;
            if mint.initial_latest_confirmed_assertion_id
                != values.common_ancestor_assertion_id
            {
                return Err("fast-ingress reversion common ancestor mismatch".to_string());
            }
            match mint.status.as_str() {
                FAST_INGRESS_MINT_STATUS_ESCROWED => {
                    mint.status = FAST_INGRESS_MINT_STATUS_REVERTED_ESCROWED.to_string();
                }
                FAST_INGRESS_MINT_STATUS_RELEASED_UNCONFIRMED => {
                    mint.status = FAST_INGRESS_MINT_STATUS_REVERTED_RELEASED.to_string();
                }
                FAST_INGRESS_MINT_STATUS_FINAL => {
                    return Err("cannot revert a confirmed fast-ingress assertion".to_string());
                }
                FAST_INGRESS_MINT_STATUS_REVERTED_ESCROWED
                | FAST_INGRESS_MINT_STATUS_REVERTED_RELEASED => {
                    return Err("fast-ingress reversion was already applied".to_string());
                }
                _ => return Err("unknown fast-ingress mint status".to_string()),
            }
            affected = affected
                .checked_add(mint.amount_atoms)
                .ok_or_else(|| "fast-ingress reversion overflow".to_string())?;
        }
        if !matched || affected == 0 {
            return Err("fast-ingress reversion has no associated mints".to_string());
        }
        // Reverted exposure remains charged as bad debt. It cannot recycle CAP.
        self.paused = true;
        self.validate()?;
        Ok(affected)
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| format!("encode fast-ingress campaign: {error}"))
    }
}

pub fn pfusdc_fast_ingress_deposit_key_v1(
    arbitrum_chain_id: u64,
    vault_address: &str,
    deposit_id: &str,
) -> Result<String, String> {
    validate_evm_address_text("fast_ingress.vault_address", vault_address)?;
    let vault = pfusdc_decode_hex(vault_address.trim_start_matches("0x"))?;
    let deposit = pfusdc_decode_hex(deposit_id.trim_start_matches("0x"))?;
    if vault.len() != 20 || deposit.len() != 32 {
        return Err("fast-ingress replay key inputs have invalid widths".to_string());
    }
    let mut preimage = Vec::with_capacity(
        PFUSDC_FAST_INGRESS_DEPOSIT_KEY_DOMAIN_V1.len() + 1 + 8 + vault.len() + deposit.len(),
    );
    preimage.extend_from_slice(PFUSDC_FAST_INGRESS_DEPOSIT_KEY_DOMAIN_V1.as_bytes());
    preimage.push(0);
    preimage.extend_from_slice(&arbitrum_chain_id.to_be_bytes());
    preimage.extend_from_slice(&vault);
    preimage.extend_from_slice(&deposit);
    let mut hasher = Keccak256::new();
    hasher.update(preimage);
    Ok(bytes_to_hex(&hasher.finalize()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcBondedLifecyclePublicValuesV1 {
    pub schema: String,
    pub proof_program_version: u32,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
    pub route_profile_hash: String,
    pub route_epoch: u64,
    pub manifest_hash: String,
    pub verifier_policy_hash: String,
    pub ethereum_chain_id: u64,
    pub prior_ethereum_finalized_beacon_root: String,
    pub prior_ethereum_finalized_slot: u64,
    pub ethereum_finalized_beacon_root: String,
    pub ethereum_finalized_slot: u64,
    pub ethereum_finalized_execution_block_number: u64,
    pub ethereum_finalized_execution_block_hash: String,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: String,
    pub arbitrum_rollup_runtime_code_hash: String,
    pub source_assertion_id: String,
    pub latest_confirmed_assertion_id: String,
    pub latest_confirmed_l2_block_hash: String,
    pub latest_confirmed_send_root: String,
    pub common_ancestor_assertion_id: String,
    pub update_kind: String,
    pub public_values_commitment: String,
}

impl PfUsdcBondedLifecyclePublicValuesV1 {
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = PfusdcCanonicalReader::new(
            bytes,
            PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1,
        )?;
        let mut values = Self {
            schema: reader.text(1)?,
            proof_program_version: reader.u32(2)?,
            pftl_chain_id: reader.text(3)?,
            pftl_genesis_hash: reader.hex(4, 48)?,
            pftl_protocol_version: reader.u32(5)?,
            route_profile_hash: reader.hex(6, 48)?,
            route_epoch: reader.u64(7)?,
            manifest_hash: reader.hex(8, 32)?,
            ethereum_chain_id: reader.u64(9)?,
            prior_ethereum_finalized_beacon_root: reader.hex(10, 32)?,
            prior_ethereum_finalized_slot: reader.u64(11)?,
            ethereum_finalized_beacon_root: reader.hex(12, 32)?,
            ethereum_finalized_slot: reader.u64(13)?,
            ethereum_finalized_execution_block_number: reader.u64(14)?,
            ethereum_finalized_execution_block_hash: reader.hex(15, 32)?,
            arbitrum_chain_id: reader.u64(16)?,
            arbitrum_rollup_address: reader.evm_address(17)?,
            arbitrum_rollup_runtime_code_hash: reader.hex(18, 32)?,
            source_assertion_id: reader.hex(19, 32)?,
            latest_confirmed_assertion_id: reader.hex(20, 32)?,
            update_kind: reader.text(21)?,
            latest_confirmed_l2_block_hash: reader.hex(22, 32)?,
            latest_confirmed_send_root: reader.hex(23, 32)?,
            common_ancestor_assertion_id: reader.hex(24, 32)?,
            verifier_policy_hash: reader.hex(25, 32)?,
            public_values_commitment: String::new(),
        };
        reader.finish()?;
        values.seal()?;
        values.validate()?;
        if values.canonical_bytes_without_commitment()? != bytes {
            return Err("pfUSDC bonded lifecycle public values are not canonical".to_string());
        }
        Ok(values)
    }

    pub fn canonical_bytes_without_commitment(&self) -> Result<Vec<u8>, String> {
        self.validate_fields(false)?;
        let mut out = pfusdc_canonical_prefix(PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_u32(&mut out, 2, self.proof_program_version);
        pfusdc_append_text(&mut out, 3, &self.pftl_chain_id)?;
        pfusdc_append_hex(&mut out, 4, &self.pftl_genesis_hash)?;
        pfusdc_append_u32(&mut out, 5, self.pftl_protocol_version);
        pfusdc_append_hex(&mut out, 6, &self.route_profile_hash)?;
        pfusdc_append_u64(&mut out, 7, self.route_epoch);
        pfusdc_append_hex(&mut out, 8, &self.manifest_hash)?;
        pfusdc_append_u64(&mut out, 9, self.ethereum_chain_id);
        pfusdc_append_hex(&mut out, 10, &self.prior_ethereum_finalized_beacon_root)?;
        pfusdc_append_u64(&mut out, 11, self.prior_ethereum_finalized_slot);
        pfusdc_append_hex(&mut out, 12, &self.ethereum_finalized_beacon_root)?;
        pfusdc_append_u64(&mut out, 13, self.ethereum_finalized_slot);
        pfusdc_append_u64(&mut out, 14, self.ethereum_finalized_execution_block_number);
        pfusdc_append_hex(&mut out, 15, &self.ethereum_finalized_execution_block_hash)?;
        pfusdc_append_u64(&mut out, 16, self.arbitrum_chain_id);
        pfusdc_append_evm_address(&mut out, 17, &self.arbitrum_rollup_address)?;
        pfusdc_append_hex(&mut out, 18, &self.arbitrum_rollup_runtime_code_hash)?;
        pfusdc_append_hex(&mut out, 19, &self.source_assertion_id)?;
        pfusdc_append_hex(&mut out, 20, &self.latest_confirmed_assertion_id)?;
        pfusdc_append_text(&mut out, 21, &self.update_kind)?;
        pfusdc_append_hex(&mut out, 22, &self.latest_confirmed_l2_block_hash)?;
        pfusdc_append_hex(&mut out, 23, &self.latest_confirmed_send_root)?;
        pfusdc_append_hex(&mut out, 24, &self.common_ancestor_assertion_id)?;
        pfusdc_append_hex(&mut out, 25, &self.verifier_policy_hash)?;
        Ok(out)
    }

    pub fn seal(&mut self) -> Result<(), String> {
        self.public_values_commitment = self.expected_commitment()?;
        Ok(())
    }

    pub fn expected_commitment(&self) -> Result<String, String> {
        Ok(pfusdc_keccak_commitment(
            PFUSDC_BONDED_LIFECYCLE_COMMITMENT_DOMAIN_V1,
            &self.canonical_bytes_without_commitment()?,
        ))
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_fields(true)
    }

    fn validate_fields(&self, check_commitment: bool) -> Result<(), String> {
        if self.schema != PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1
            || self.proof_program_version != 1
            || self.pftl_protocol_version == 0
            || self.route_epoch == 0
            || self.ethereum_chain_id == 0
            || self.prior_ethereum_finalized_slot == 0
            || self.ethereum_finalized_slot <= self.prior_ethereum_finalized_slot
            || self.ethereum_finalized_execution_block_number == 0
            || self.arbitrum_chain_id == 0
            || !matches!(
                self.update_kind.as_str(),
                PFUSDC_BONDED_LIFECYCLE_UPDATE_CONFIRMED
                    | PFUSDC_BONDED_LIFECYCLE_UPDATE_REVERTED
            )
        {
            return Err("pfUSDC bonded lifecycle fields are invalid".to_string());
        }
        for (field, value) in [
            ("bonded_lifecycle.pftl_chain_id", &self.pftl_chain_id),
            ("bonded_lifecycle.update_kind", &self.update_kind),
        ] {
            pfusdc_validate_text(field, value)?;
        }
        pfusdc_validate_hex_fields(&[
            ("bonded_lifecycle.pftl_genesis_hash", &self.pftl_genesis_hash, 96),
            ("bonded_lifecycle.route_profile_hash", &self.route_profile_hash, 96),
            ("bonded_lifecycle.manifest_hash", &self.manifest_hash, 64),
            ("bonded_lifecycle.verifier_policy_hash", &self.verifier_policy_hash, 64),
            ("bonded_lifecycle.prior_root", &self.prior_ethereum_finalized_beacon_root, 64),
            ("bonded_lifecycle.final_root", &self.ethereum_finalized_beacon_root, 64),
            ("bonded_lifecycle.execution_hash", &self.ethereum_finalized_execution_block_hash, 64),
            ("bonded_lifecycle.rollup_code_hash", &self.arbitrum_rollup_runtime_code_hash, 64),
            ("bonded_lifecycle.source_assertion", &self.source_assertion_id, 64),
            ("bonded_lifecycle.latest_confirmed", &self.latest_confirmed_assertion_id, 64),
            ("bonded_lifecycle.latest_confirmed_l2_block_hash", &self.latest_confirmed_l2_block_hash, 64),
            ("bonded_lifecycle.latest_confirmed_send_root", &self.latest_confirmed_send_root, 64),
            ("bonded_lifecycle.common_ancestor", &self.common_ancestor_assertion_id, 64),
        ])?;
        validate_evm_address_text(
            "bonded_lifecycle.rollup_address",
            &self.arbitrum_rollup_address,
        )?;
        if check_commitment && self.public_values_commitment != self.expected_commitment()? {
            return Err("pfUSDC bonded lifecycle commitment mismatch".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod pfusdc_bonded_accounting_tests {
    use super::*;

    fn ingress(deposit_byte: &str, amount_atoms: u64, cap_atoms: u64) -> PfUsdcBondedIngressPublicValuesV1 {
        let deposit_id = deposit_byte.repeat(32);
        let vault = "0x1111111111111111111111111111111111111111".to_string();
        let mut values = PfUsdcBondedIngressPublicValuesV1 {
            schema: PFUSDC_BONDED_INGRESS_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            proof_program_version: 1,
            pftl_chain_id: "postfiat-ce22".to_string(),
            pftl_genesis_hash: "22".repeat(48),
            pftl_protocol_version: 1,
            route_id: "pfusdc-fast".to_string(),
            route_profile_hash: "33".repeat(48),
            route_epoch: 7,
            manifest_hash: "44".repeat(32),
            verifier_policy_hash: "45".repeat(32),
            cap_atoms,
            age_margin_blocks: 64,
            ethereum_chain_id: 1,
            prior_ethereum_finalized_beacon_root: "55".repeat(32),
            prior_ethereum_finalized_slot: 32,
            ethereum_finalized_beacon_root: "56".repeat(32),
            ethereum_finalized_slot: 64,
            ethereum_finalized_execution_block_number: 100,
            ethereum_finalized_execution_block_hash: "57".repeat(32),
            arbitrum_chain_id: 42_161,
            arbitrum_rollup_address: "0x2222222222222222222222222222222222222222".to_string(),
            arbitrum_rollup_runtime_code_hash: "58".repeat(32),
            assertion_protocol_adapter_id: "arbitrum-one-bold-v1".to_string(),
            latest_confirmed_assertion_id: "99".repeat(32),
            source_assertion_id: "aa".repeat(32),
            source_assertion_parent_id: "ab".repeat(32),
            source_assertion_created_at_l1_block: 90,
            source_assertion_confirmation_period_blocks: 45_818,
            source_assertion_l2_block_number: 200,
            source_assertion_l2_block_hash: "59".repeat(32),
            source_assertion_l2_state_root: "5a".repeat(32),
            source_assertion_send_root: "5b".repeat(32),
            source_staker: "0x3333333333333333333333333333333333333333".to_string(),
            vault_address: vault.clone(),
            vault_runtime_code_hash: "5c".repeat(32),
            token_address: "0x4444444444444444444444444444444444444444".to_string(),
            token_runtime_code_hash: "5d".repeat(32),
            asset_id: "02".repeat(48),
            depositor: "0x5555555555555555555555555555555555555555".to_string(),
            pftl_recipient: "pftl1recipient".to_string(),
            pftl_recipient_hash: "5e".repeat(32),
            amount_atoms,
            deposit_nonce: "5f".repeat(32),
            route_binding: "60".repeat(32),
            deposit_id: deposit_id.clone(),
            deposit_key: pfusdc_fast_ingress_deposit_key_v1(42_161, &vault, &deposit_id)
                .expect("deposit key"),
            deposit_seen_storage_slot: "61".repeat(32),
            evidence_root: "62".repeat(48),
            public_values_commitment: String::new(),
        };
        values.seal().expect("seal ingress values");
        values.validate().expect("valid ingress values");
        values
    }

    fn lifecycle(kind: &str, common: &str) -> PfUsdcBondedLifecyclePublicValuesV1 {
        let mut values = PfUsdcBondedLifecyclePublicValuesV1 {
            schema: PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            proof_program_version: 1,
            pftl_chain_id: "postfiat-ce22".to_string(),
            pftl_genesis_hash: "22".repeat(48),
            pftl_protocol_version: 1,
            route_profile_hash: "33".repeat(48),
            route_epoch: 7,
            manifest_hash: "44".repeat(32),
            verifier_policy_hash: "45".repeat(32),
            ethereum_chain_id: 1,
            prior_ethereum_finalized_beacon_root: "55".repeat(32),
            prior_ethereum_finalized_slot: 64,
            ethereum_finalized_beacon_root: "56".repeat(32),
            ethereum_finalized_slot: 96,
            ethereum_finalized_execution_block_number: 101,
            ethereum_finalized_execution_block_hash: "57".repeat(32),
            arbitrum_chain_id: 42_161,
            arbitrum_rollup_address: "0x2222222222222222222222222222222222222222".to_string(),
            arbitrum_rollup_runtime_code_hash: "58".repeat(32),
            source_assertion_id: "aa".repeat(32),
            latest_confirmed_assertion_id: "bb".repeat(32),
            latest_confirmed_l2_block_hash: "bc".repeat(32),
            latest_confirmed_send_root: "bd".repeat(32),
            common_ancestor_assertion_id: common.repeat(32),
            update_kind: kind.to_string(),
            public_values_commitment: String::new(),
        };
        values.seal().expect("seal lifecycle values");
        values.validate().expect("valid lifecycle values");
        values
    }

    #[test]
    fn deposit_key_is_independent_of_route_and_recipient() {
        let first = ingress("01", 1, 5);
        let mut changed = first.clone();
        changed.route_epoch += 1;
        changed.pftl_recipient = "pftl1different".to_string();
        changed.seal().expect("reseal changed values");
        assert_eq!(first.deposit_key, changed.deposit_key);
    }

    #[test]
    fn campaign_enforces_replay_cap_and_escrow() {
        let first = ingress("01", 3, 5);
        let mut campaign = FastIngressCampaignStateV1::from_public_values(&first).unwrap();
        campaign.accept_escrowed_mint(&first, 10).unwrap();
        assert_eq!(campaign.exposure_total_atoms, 3);
        assert_eq!(campaign.mints[0].status, FAST_INGRESS_MINT_STATUS_ESCROWED);
        assert!(campaign.accept_escrowed_mint(&first, 11).is_err());
        let second = ingress("02", 3, 5);
        assert!(campaign.accept_escrowed_mint(&second, 12).is_err());
        assert_eq!(campaign.exposure_total_atoms, 3);
    }

    #[test]
    fn confirmation_releases_exposure_once() {
        let ingress = ingress("01", 3, 5);
        let mut campaign = FastIngressCampaignStateV1::from_public_values(&ingress).unwrap();
        campaign.accept_escrowed_mint(&ingress, 10).unwrap();
        let confirmation = lifecycle(PFUSDC_BONDED_LIFECYCLE_UPDATE_CONFIRMED, "aa");
        assert_eq!(campaign.apply_confirmation(&confirmation).unwrap(), 3);
        assert_eq!(campaign.exposure_total_atoms, 0);
        assert_eq!(campaign.mints[0].status, FAST_INGRESS_MINT_STATUS_FINAL);
        assert!(campaign.apply_confirmation(&confirmation).is_err());
    }

    #[test]
    fn reversion_pauses_and_does_not_restore_capacity() {
        let ingress = ingress("01", 3, 5);
        let mut campaign = FastIngressCampaignStateV1::from_public_values(&ingress).unwrap();
        campaign.accept_escrowed_mint(&ingress, 10).unwrap();
        let reversion = lifecycle(PFUSDC_BONDED_LIFECYCLE_UPDATE_REVERTED, "99");
        assert_eq!(campaign.apply_reversion(&reversion).unwrap(), 3);
        assert!(campaign.paused);
        assert_eq!(campaign.exposure_total_atoms, 3);
        assert_eq!(
            campaign.mints[0].status,
            FAST_INGRESS_MINT_STATUS_REVERTED_ESCROWED
        );
        assert!(campaign.apply_reversion(&reversion).is_err());
    }
}
