use super::{PfUsdcBeaconFinalityStateV1, PfUsdcEthereumIngressPublicValuesV1};

impl PfUsdcBeaconFinalityStateV1 {
    /// Resolve the ledger anchor before admitting a verified Ethereum proof.
    pub fn advance_for_deposit(
        states: &[Self],
        profile_hash: &str,
        values: &PfUsdcEthereumIngressPublicValuesV1,
    ) -> Result<Self, (&'static str, String)> {
        let mut matches = states.iter().filter(|state| {
            state.route_profile_hash == profile_hash && state.route_epoch == values.route_epoch
        });
        let mut state = matches
            .next()
            .ok_or_else(|| {
                (
                    "pfusdc_beacon_finality_state_missing",
                    "Ethereum ingress requires a governance-pinned beacon finality anchor"
                        .to_string(),
                )
            })?
            .clone();
        if matches.next().is_some() {
            return Err((
                "pfusdc_beacon_finality_state_ambiguous",
                "duplicate beacon anchors".into(),
            ));
        }
        let epoch = u32::try_from(values.route_epoch).map_err(|_| {
            (
                "pfusdc_beacon_route_epoch_invalid",
                "beacon route epoch exceeds u32".into(),
            )
        })?;
        let expected_binding = super::vault_bridge_route_binding(profile_hash, epoch)
            .map_err(|error| ("pfusdc_beacon_route_epoch_invalid", error))?;
        if values.route_binding != expected_binding {
            return Err((
                "pfusdc_beacon_route_epoch_invalid",
                "proof epoch does not match the deposit route binding".into(),
            ));
        }
        state
            .verify_and_advance(values)
            .map_err(|error| ("pfusdc_beacon_finality_state_rejected", error))?;
        Ok(state)
    }
}
