use super::{PfUsdcBeaconFinalityStateV1, PfUsdcEthereumIngressPublicValuesV1};

impl PfUsdcBeaconFinalityStateV1 {
    /// Resolve the ledger anchor before admitting a verified Ethereum proof.
    pub fn advance_for_deposit(
        states: &[Self],
        profile_hash: &str,
        values: &PfUsdcEthereumIngressPublicValuesV1,
    ) -> Result<Self, (&'static str, String)> {
        let mut matches = states.iter().filter(|state| state.route_profile_hash == profile_hash);
        let mut state = matches.next().ok_or_else(|| (
            "pfusdc_beacon_finality_state_missing",
            "Ethereum ingress requires a governance-pinned beacon finality anchor".to_string(),
        ))?.clone();
        if matches.next().is_some() {
            return Err(("pfusdc_beacon_finality_state_ambiguous", "duplicate beacon anchors".into()));
        }
        state.verify_and_advance(values)
            .map_err(|error| ("pfusdc_beacon_finality_state_rejected", error))?;
        Ok(state)
    }
}
