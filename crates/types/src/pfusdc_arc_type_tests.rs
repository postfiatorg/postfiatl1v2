fn corrected_arc_public_values() -> PfUsdcArcIngressPublicValuesV1 {
    PfUsdcArcIngressPublicValuesV1::from_canonical_bytes(include_bytes!(
        "../../../docs/evidence/arc-mvp-20260828/arc-ingress-execute/public-values.bin"
    ))
    .expect("checked-in corrected Arc public values")
}

fn corrected_arc_profile() -> VaultBridgeRouteProfileV1 {
    serde_json::from_str(include_str!(
        "../../../docs/evidence/arc-mvp-20260828/route-profile.corrected.json"
    ))
    .expect("checked-in corrected Arc route profile")
}

fn corrected_arc_bootstrap() -> PfUsdcArcFinalityStateV1 {
    serde_json::from_str(include_str!(
        "../../../docs/evidence/arc-mvp-20260828/arc-finality-bootstrap.json"
    ))
    .expect("checked-in corrected Arc finality bootstrap")
}

fn arc_route_amendment(profile: &VaultBridgeRouteProfileV1) -> GovernanceAmendment {
    GovernanceAmendment {
        amendment_id: "pfusdc-arc-epoch8".to_string(),
        chain_id: "postfiat-wan-devnet-2".to_string(),
        genesis_hash: "ce".repeat(48),
        protocol_version: 1,
        instance_id: "arc-mvp-20260828".to_string(),
        proposal_id: "pfusdc-arc-route".to_string(),
        certificate_id: "pfusdc-arc-route-certificate".to_string(),
        proposer: "operator".to_string(),
        validators: vec!["validator-0".to_string()],
        quorum: 1,
        kind: vault_bridge_route_amendment_kind(profile).expect("route amendment kind"),
        value: profile.route_epoch,
        activation_height: profile.activation_height,
        veto_until_height: 0,
        paused: false,
        support: vec!["validator-0".to_string()],
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
    }
}

#[test]
fn arc_ingress_public_values_decode_the_live_corrected_deposit() {
    let values = corrected_arc_public_values();
    assert_eq!(
        values.route_id,
        "7e85699a3d915d61597d4e060cb0eabf5a375452e07462794176c1852ad8cbb9"
    );
    assert_eq!(values.arc_chain_id, 5_042_002);
    assert_eq!(
        values.vault_address,
        "0xee05b21b920b9367728422eea95c8426154e0de8"
    );
    assert_eq!(
        values.token_address,
        "0x3600000000000000000000000000000000000000"
    );
    assert_eq!(
        values.deposit_id,
        "b54fd8fb1e684d7f6d74c64cd4b81a35fc25845c6dc9323fbda06ed7af89edf7"
    );
    assert_eq!(values.amount_atoms, 1_000_000);
    assert_eq!(
        values.arc_block_hash,
        "f3f7627371d1a58ffd78073e00dcda4ad3241e4cdeb1b6b8c9b474991736e7ce"
    );
    assert_eq!(values.arc_block_height, 59_335_780);
    assert_eq!(
        values.validator_set_commitment_in,
        "75d2bed78b313f681ad0976e67a486032cc83582b414e04d51ff04d51cc0a2fa"
    );
    assert_eq!(
        values.validator_set_commitment_out,
        values.validator_set_commitment_in
    );

    let mut trailing = include_bytes!(
        "../../../docs/evidence/arc-mvp-20260828/arc-ingress-execute/public-values.bin"
    )
    .to_vec();
    trailing.push(0);
    assert!(PfUsdcArcIngressPublicValuesV1::from_canonical_bytes(&trailing).is_err());
}

#[test]
fn arc_finality_state_advances_once_and_rejects_replay_or_wrong_route() {
    let values = corrected_arc_public_values();
    let mut state = corrected_arc_bootstrap();
    state.validate().expect("valid Arc bootstrap");
    state
        .verify_and_advance(&values)
        .expect("live corrected proof advances the bootstrap");
    assert_eq!(state.latest_block_height, 59_335_780);
    assert_eq!(state.latest_block_hash, values.arc_block_hash);
    assert!(state.verify_and_advance(&values).is_err(), "replay is stale");

    let mut wrong_route = corrected_arc_public_values();
    wrong_route.route_id = "00".repeat(32);
    assert!(corrected_arc_bootstrap()
        .verify_and_advance(&wrong_route)
        .is_err());
}

#[test]
fn arc_route_activation_binds_profile_bootstrap_and_runtime_code() {
    let profile = corrected_arc_profile();
    let bootstrap = corrected_arc_bootstrap();
    let activation = VaultBridgeRouteProfileActivationV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
        amendment: arc_route_amendment(&profile),
        profile: profile.clone(),
        tier4_finality_bootstrap: None,
        arc_finality_bootstrap: Some(bootstrap.clone()),
    };
    activation.validate().expect("corrected Arc activation");

    let mut wrong_code = activation.clone();
    wrong_code
        .arc_finality_bootstrap
        .as_mut()
        .expect("bootstrap")
        .vault_runtime_code_hash = format!("0x{}", "00".repeat(32));
    assert!(wrong_code.validate().is_err());

    let mut wrong_route = activation;
    wrong_route
        .arc_finality_bootstrap
        .as_mut()
        .expect("bootstrap")
        .route_binding = "00".repeat(32);
    assert!(wrong_route.validate().is_err());
}
