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
