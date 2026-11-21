use super::*;

impl PfUsdcBeaconFinalityStateV1 {
    /// Historical authorization bytes only, never replicated-state/admission
    /// validation. The empty-committee case is byte-for-byte the pre-B23 format.
    pub(crate) fn historical_bootstrap_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        if !self.current_sync_committee_root.is_empty() {
            return self.state_commitment_bytes();
        }
        if self.schema != PFUSDC_BEACON_FINALITY_STATE_SCHEMA_V1
            || self.route_epoch == 0
            || self.source_chain_id == 0
            || self.finalized_slot == 0
        {
            return Err("pfUSDC beacon finality state has invalid schema or scalars".to_string());
        }
        validate_evm_address_text("pfusdc_beacon_finality.vault_address", &self.vault_address)?;
        validate_evm_address_text("pfusdc_beacon_finality.token_address", &self.token_address)?;
        pfusdc_validate_hex_fields(&[
            (
                "pfusdc_beacon_finality.route_profile_hash",
                &self.route_profile_hash,
                96,
            ),
            (
                "pfusdc_beacon_finality.route_binding",
                &self.route_binding,
                64,
            ),
            (
                "pfusdc_beacon_finality.finalized_beacon_root",
                &self.finalized_beacon_root,
                64,
            ),
        ])?;
        let mut out = pfusdc_canonical_prefix(PFUSDC_BEACON_FINALITY_STATE_SCHEMA_V1);
        pfusdc_append_text(&mut out, 1, &self.schema)?;
        pfusdc_append_hex(&mut out, 2, &self.route_profile_hash)?;
        pfusdc_append_u64(&mut out, 3, self.route_epoch);
        pfusdc_append_u64(&mut out, 4, self.source_chain_id);
        pfusdc_append_evm_address(&mut out, 5, &self.vault_address)?;
        pfusdc_append_evm_address(&mut out, 6, &self.token_address)?;
        pfusdc_append_hex(&mut out, 7, &self.route_binding)?;
        pfusdc_append_hex(&mut out, 8, &self.finalized_beacon_root)?;
        pfusdc_append_u64(&mut out, 9, self.finalized_slot);
        Ok(out)
    }
}

/// Ethereum ingress public values in the 26-field layout that the guest
/// committed before the anchored beacon route (no route epoch, no sync
/// committee roots). A route profile with no beacon anchor admits deposits in
/// this layout, as main did; a profile with an anchor refuses them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcEthereumIngressPublicValuesLegacyV1 {
    pub schema: String,
    pub route_id: String,
    pub source_chain_id: u64,
    pub prior_finalized_beacon_root: String,
    pub prior_finalized_slot: u64,
    pub finalized_beacon_root: String,
    pub finalized_slot: u64,
    pub finalized_execution_block_hash: String,
    pub finalized_execution_block_number: u64,
    pub execution_state_root: String,
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
    pub manifest_hash: String,
    pub deposit_nullifier: String,
    pub total_obligations_atoms: String,
    pub vault_token_balance_atoms: String,
}

impl PfUsdcEthereumIngressPublicValuesLegacyV1 {
    /// Byte-for-byte the decoder main used before the anchored route.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, String> {
        let values: Self = serde_cbor::from_slice(bytes).map_err(|error| {
            format!("Ethereum ingress public values CBOR decode failed: {error}")
        })?;
        values.validate()?;
        let canonical = serde_cbor::to_vec(&values).map_err(|error| {
            format!("Ethereum ingress public values CBOR encode failed: {error}")
        })?;
        if canonical != bytes {
            return Err("Ethereum ingress public values are not canonical CBOR".to_string());
        }
        Ok(values)
    }

    /// Main's validation before the anchored route, unchanged.
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.schema.as_str(),
            PFUSDC_ETHEREUM_INGRESS_PUBLIC_VALUES_SCHEMA_V1
                | PFETH_ETHEREUM_INGRESS_PUBLIC_VALUES_SCHEMA_V1
        ) {
            return Err("Ethereum ingress public values schema mismatch".to_string());
        }
        if self.route_id.is_empty() || self.source_chain_id == 0 || self.amount_atoms == 0 {
            return Err("Ethereum ingress route, chain, and amount must be nonzero".to_string());
        }
        if self.finalized_slot <= self.prior_finalized_slot
            || self.finalized_execution_block_number == 0
        {
            return Err("Ethereum ingress finality must advance".to_string());
        }
        validate_evm_address_text("ethereum_ingress.vault_address", &self.vault_address)?;
        validate_evm_address_text("ethereum_ingress.token_address", &self.token_address)?;
        validate_evm_address_text("ethereum_ingress.depositor", &self.depositor)?;
        pfusdc_validate_hex_fields(&[
            (
                "ethereum_ingress.prior_finalized_beacon_root",
                &self.prior_finalized_beacon_root,
                64,
            ),
            (
                "ethereum_ingress.finalized_beacon_root",
                &self.finalized_beacon_root,
                64,
            ),
            (
                "ethereum_ingress.finalized_execution_block_hash",
                &self.finalized_execution_block_hash,
                64,
            ),
            (
                "ethereum_ingress.execution_state_root",
                &self.execution_state_root,
                64,
            ),
            (
                "ethereum_ingress.vault_runtime_code_hash",
                &self.vault_runtime_code_hash,
                64,
            ),
            (
                "ethereum_ingress.token_runtime_code_hash",
                &self.token_runtime_code_hash,
                64,
            ),
            (
                "ethereum_ingress.pftl_recipient_hash",
                &self.pftl_recipient_hash,
                64,
            ),
            ("ethereum_ingress.nonce", &self.nonce, 64),
            ("ethereum_ingress.route_binding", &self.route_binding, 64),
            ("ethereum_ingress.deposit_id", &self.deposit_id, 64),
            ("ethereum_ingress.evidence_root", &self.evidence_root, 96),
            ("ethereum_ingress.manifest_hash", &self.manifest_hash, 64),
            (
                "ethereum_ingress.deposit_nullifier",
                &self.deposit_nullifier,
                64,
            ),
        ])?;
        let obligations = self
            .total_obligations_atoms
            .parse::<u128>()
            .map_err(|_| "Ethereum ingress obligations are not an integer".to_string())?;
        let balance = self
            .vault_token_balance_atoms
            .parse::<u128>()
            .map_err(|_| "Ethereum ingress token balance is not an integer".to_string())?;
        if obligations < u128::from(self.amount_atoms) || balance < obligations {
            return Err("Ethereum ingress backing is insufficient".to_string());
        }
        Ok(())
    }

    /// The shared fields in the current layout, for the route and evidence
    /// comparison only. The result carries no route epoch or committee roots,
    /// so it never validates and can never reach the anchor path.
    pub fn unanchored_comparison_view(&self) -> PfUsdcEthereumIngressPublicValuesV1 {
        PfUsdcEthereumIngressPublicValuesV1 {
            schema: self.schema.clone(),
            route_id: self.route_id.clone(),
            source_chain_id: self.source_chain_id,
            route_epoch: 0,
            prior_finalized_beacon_root: self.prior_finalized_beacon_root.clone(),
            prior_finalized_slot: self.prior_finalized_slot,
            prior_sync_committee_root: String::new(),
            finalized_beacon_root: self.finalized_beacon_root.clone(),
            finalized_slot: self.finalized_slot,
            current_sync_committee_root: String::new(),
            finalized_execution_block_hash: self.finalized_execution_block_hash.clone(),
            finalized_execution_block_number: self.finalized_execution_block_number,
            execution_state_root: self.execution_state_root.clone(),
            vault_address: self.vault_address.clone(),
            vault_runtime_code_hash: self.vault_runtime_code_hash.clone(),
            token_address: self.token_address.clone(),
            token_runtime_code_hash: self.token_runtime_code_hash.clone(),
            depositor: self.depositor.clone(),
            pftl_recipient: self.pftl_recipient.clone(),
            pftl_recipient_hash: self.pftl_recipient_hash.clone(),
            amount_atoms: self.amount_atoms,
            nonce: self.nonce.clone(),
            route_binding: self.route_binding.clone(),
            deposit_id: self.deposit_id.clone(),
            evidence_root: self.evidence_root.clone(),
            manifest_hash: self.manifest_hash.clone(),
            deposit_nullifier: self.deposit_nullifier.clone(),
            total_obligations_atoms: self.total_obligations_atoms.clone(),
            vault_token_balance_atoms: self.vault_token_balance_atoms.clone(),
        }
    }
}
