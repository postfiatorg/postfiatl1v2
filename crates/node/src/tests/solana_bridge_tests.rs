use super::*;
use postfiat_types::{
    SolanaProofOperation, SOLANA_ACTIVATION_HEIGHT, SOLANA_DEPOSIT_PROOF, SOLANA_HANDOFF_PROOF,
};

fn profile() -> VaultBridgeRouteProfileV1 {
    serde_json::from_str(include_str!("../../testdata/solana-bridge/profile.json")).unwrap()
}

fn initial(
    key: &postfiat_crypto_provider::MlDsa65KeyPair,
) -> (Genesis, GovernanceState, LedgerState) {
    let genesis = Genesis::new("postfiat-local");
    let mut governance = GovernanceState::new(1);
    governance.apply(amendment(
        GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
        1,
        0,
    ));
    let family =
        AssetDefinition::new(&genesis.chain_id, "solana-test-issuer", "pfUSDC", 1, 6).unwrap();
    assert_eq!(family.asset_id, profile().asset_id);
    let mut ledger = LedgerState::new(vec![
        Account::new(
            address_from_public_key(&key.public_key),
            10_000_000_000,
            Some(bytes_to_hex(&key.public_key)),
        ),
        Account::new(format!("pf{}", "33".repeat(20)), 10_000_000, None),
    ]);
    ledger.nav_assets.push(
        NavTrackedAsset::new(
            &family.asset_id,
            &family.issuer,
            &family.issuer,
            "fixture-profile",
            "USDC_ATOMS",
            &family.issuer,
        )
        .unwrap(),
    );
    ledger.asset_definitions.push(family);
    (genesis, governance, ledger)
}

fn signed_proof(
    genesis: &Genesis,
    ledger: &LedgerState,
    profile: &VaultBridgeRouteProfileV1,
    key: &postfiat_crypto_provider::MlDsa65KeyPair,
    kind: &str,
    seq: u64,
) -> SignedAssetTransaction {
    let fixture = match kind {
        SOLANA_HANDOFF_PROOF => "handoff",
        SOLANA_DEPOSIT_PROOF => "deposit",
        _ => unreachable!(),
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/solana-bridge");
    let submitter = address_from_public_key(&key.public_key);
    let op = SolanaProofOperation {
        submitter: submitter.clone(),
        profile_hash: profile.profile_hash().unwrap(),
        proof_kind: kind.into(),
        proof: std::fs::read(dir.join(format!("{fixture}.groth16"))).unwrap(),
        public_values: std::fs::read(dir.join(format!("{fixture}.values"))).unwrap(),
    };
    let tx = signed_asset_transaction_for_test(
        genesis,
        ledger,
        &submitter,
        &bytes_to_hex(&key.public_key),
        &bytes_to_hex(&key.private_key),
        "solana_bridge_proof_v1",
        seq,
        AssetTransactionOperation::SolanaBridgeProof(op),
    );
    serde_json::from_slice(&serde_json::to_vec(&tx).unwrap()).unwrap()
}

#[test]
fn activation_is_disabled_by_default_and_refuses_early_execution() {
    let key = ml_dsa_65_keygen().unwrap();
    let (genesis, mut governance, mut ledger) = initial(&key);
    let profile = profile();
    let before = ledger.clone();
    assert!(!activate_route(&mut governance, &mut ledger, &profile).accepted);
    assert_eq!(ledger, before);
    let proof = signed_proof(&genesis, &ledger, &profile, &key, SOLANA_DEPOSIT_PROOF, 1);
    assert_eq!(
        execute_route_candidate(&genesis, &governance, &mut ledger, proof.clone(), 10).code,
        "solana_bridge_disabled"
    );
    assert_eq!(ledger, before);
    governance.apply(amendment(SOLANA_ACTIVATION_HEIGHT, 11, 0));
    assert!(!activate_route(&mut governance, &mut ledger, &profile).accepted);
    assert_eq!(ledger, before);
    assert_eq!(
        execute_route_candidate(&genesis, &governance, &mut ledger, proof, 10).code,
        "solana_bridge_disabled"
    );
    assert_eq!(ledger, before);
    assert!(serde_json::to_value(&ledger)
        .unwrap()
        .get("solana_route")
        .is_none());
}

#[test]
fn real_groth16_credit_replay_and_handoff_converge_on_six_replicas() {
    let key = ml_dsa_65_keygen().unwrap();
    let (genesis, governance, ledger) = initial(&key);
    let profile = profile();
    let mut replicas = Vec::new();
    for _ in 0..6 {
        let mut governance = governance.clone();
        let mut ledger = ledger.clone();
        governance.apply(amendment(SOLANA_ACTIVATION_HEIGHT, 10, 0));
        let receipt = activate_route(&mut governance, &mut ledger, &profile);
        assert!(receipt.accepted, "{}", receipt.message);
        replicas.push((governance, ledger));
    }
    let mut ordered = Vec::new();
    for (kind, seq, height, expected) in [
        (SOLANA_DEPOSIT_PROOF, 1, 10, "accepted"),
        (SOLANA_DEPOSIT_PROOF, 2, 11, "duplicate_solana_deposit"),
        (SOLANA_HANDOFF_PROOF, 2, 12, "accepted"),
        (SOLANA_HANDOFF_PROOF, 3, 13, "invalid_solana_handoff"),
    ] {
        let tx = signed_proof(&genesis, &replicas[0].1, &profile, &key, kind, seq);
        ordered.push(postfiat_execution::asset_transaction_tx_id(&tx));
        let mut roots = Vec::new();
        for (governance, ledger) in &mut replicas {
            let before = ledger.clone();
            let receipt = execute_route_candidate(&genesis, governance, ledger, tx.clone(), height);
            assert_eq!(receipt.code, expected, "{}", receipt.message);
            if !receipt.accepted {
                assert_eq!(*ledger, before);
            }
            assert_eq!(ledger.trustlines.len(), 1);
            let bucket = &ledger.vault_bridge_bucket_states[0];
            assert_eq!(profile.bucket_source_series_id(&genesis.chain_id, bucket).unwrap(), ledger.trustlines[0].asset_id);
            assert_eq!(
                ledger.trustlines[0].account,
                format!("pf{}", "33".repeat(20))
            );
            assert_eq!(ledger.trustlines[0].balance, 5_000_000);
            assert_eq!(
                ledger.asset_definitions[1].asset_family_id,
                profile.asset_id
            );
            assert_eq!(
                ledger.trustlines[0].asset_id,
                ledger.asset_definitions[1].asset_id
            );
            let client = &ledger.solana_route.as_ref().unwrap().client;
            if height >= 12 {
                assert_eq!((client.epoch, client.total_stake.as_str()), (1, "125"));
                assert_ne!(
                    client.validator_set_hash,
                    profile.solana.as_ref().unwrap().anchor.validator_set_hash
                );
            } else {
                assert_eq!(client, &profile.solana.as_ref().unwrap().anchor);
            }
            roots.push(
                replicated_state_root(
                    &genesis,
                    governance,
                    ledger,
                    &ordered,
                    &ShieldedState::empty(),
                    &BridgeState::empty(),
                )
                .unwrap(),
            );
        }
        assert!(roots.windows(2).all(|pair| pair[0] == pair[1]));
    }
}

#[test]
fn real_proof_refused_under_wrong_program_key_set_or_route_binding() {
    let key = ml_dsa_65_keygen().unwrap();
    for (mutation, code) in [
        ("program", "sp1_proof_invalid"),
        ("set", "solana_validator_set_mismatch"),
        ("binding", "solana_route_mismatch"),
    ] {
        let (genesis, mut governance, mut ledger) = initial(&key);
        let mut profile = profile();
        match mutation {
            "program" => profile.verifier_program_vkey = format!("0x{}", "01".repeat(32)),
            "set" => profile.solana.as_mut().unwrap().anchor.validator_set_hash = "ef".repeat(32),
            "binding" => profile.solana.as_mut().unwrap().route_binding = "ed".repeat(32),
            _ => unreachable!(),
        }
        governance.apply(amendment(SOLANA_ACTIVATION_HEIGHT, 10, 0));
        assert!(activate_route(&mut governance, &mut ledger, &profile).accepted);
        let before = ledger.clone();
        let tx = signed_proof(&genesis, &ledger, &profile, &key, SOLANA_DEPOSIT_PROOF, 1);
        let receipt = execute_route_candidate(&genesis, &governance, &mut ledger, tx, 10);
        assert_eq!(receipt.code, code, "{mutation}: {}", receipt.message);
        assert_eq!(ledger, before);
    }
}

#[test]
fn separate_solana_clusters_keep_their_governed_clients() {
    let key = ml_dsa_65_keygen().unwrap();
    let (genesis, mut governance, mut ledger) = initial(&key);
    governance.apply(amendment(SOLANA_ACTIVATION_HEIGHT, 10, 0));
    let first = profile();
    assert!(activate_route(&mut governance, &mut ledger, &first).accepted);
    let mut second = first.clone();
    second.route_id = "solana-devnet-usdc-v1".into();
    let pins = second.solana.as_mut().unwrap();
    pins.genesis_hash = "ce59db5080fc2c6d3bcf7ca90712d3c2e5e6c28f27f0dfbb9953bdb0894c03ab".into();
    pins.route_binding = postfiat_types::solana_deposit_route_binding(
        &genesis.chain_id, &genesis_hash(&genesis), &second.route_id, &pins.genesis_hash, &second.vault_address
    ).unwrap();
    let receipt = activate_route(&mut governance, &mut ledger, &second);
    assert!(receipt.accepted, "{}", receipt.message);
    assert!(!activate_route(&mut governance, &mut ledger, &second).accepted);
    let previous = ledger.solana_route.clone();
    let handoff = signed_proof(&genesis, &ledger, &second, &key, SOLANA_HANDOFF_PROOF, 1);
    let receipt = execute_route_candidate(&genesis, &governance, &mut ledger, handoff, 12);
    assert!(receipt.accepted, "{}", receipt.message);
    assert_eq!(ledger.solana_route, previous);
    assert_eq!(ledger.additional_solana_routes[&second.profile_hash().unwrap()].client.epoch, 1);
}

#[test]
fn governed_solana_revision_keeps_backing_and_retires_the_old_client() {
    let key = ml_dsa_65_keygen().unwrap();
    let (genesis, mut governance, mut ledger) = initial(&key);
    governance.apply(amendment(SOLANA_ACTIVATION_HEIGHT, 10, 0));
    let first = profile();
    assert!(activate_route(&mut governance, &mut ledger, &first).accepted);
    let deposit = signed_proof(&genesis, &ledger, &first, &key, SOLANA_DEPOSIT_PROOF, 1);
    assert!(execute_route_candidate(&genesis, &governance, &mut ledger, deposit, 10).accepted);
    let mut second = first.clone();
    second.route_epoch += 1;
    second.activation_height += 1;
    assert!(!activate_route(&mut governance, &mut ledger, &second).accepted);
    second.solana.as_mut().unwrap().anchor_slot += 1;
    let updated = activate_route(&mut governance, &mut ledger, &second);
    assert!(updated.accepted, "{}", updated.message);
    let old = signed_proof(&genesis, &ledger, &first, &key, SOLANA_DEPOSIT_PROOF, 2);
    assert_eq!(execute_route_candidate(&genesis, &governance, &mut ledger, old, 12).code, "solana_route_superseded");
    let replay = signed_proof(&genesis, &ledger, &second, &key, SOLANA_DEPOSIT_PROOF, 2);
    assert_eq!(execute_route_candidate(&genesis, &governance, &mut ledger, replay, 12).code, "duplicate_solana_deposit");
    assert_eq!(ledger.trustlines[0].balance, 5_000_000);
}

#[test]
fn key_only_revision_inherits_the_proved_client() {
    let key = ml_dsa_65_keygen().unwrap();
    let (genesis, mut governance, mut ledger) = initial(&key);
    governance.apply(amendment(SOLANA_ACTIVATION_HEIGHT, 10, 0));
    let first = profile();
    assert!(activate_route(&mut governance, &mut ledger, &first).accepted);
    let handoff = signed_proof(&genesis, &ledger, &first, &key, SOLANA_HANDOFF_PROOF, 1);
    assert!(execute_route_candidate(&genesis, &governance, &mut ledger, handoff, 10).accepted);
    let client = ledger.solana_route.as_ref().unwrap().client.clone();
    assert_ne!(client, first.solana.as_ref().unwrap().anchor);
    let mut next = first.clone();
    next.route_epoch += 1;
    next.activation_height += 1;
    next.verifier_program_vkey = format!("0x{}", "03".repeat(32));
    let mut changed = next.clone();
    changed.token_address = "19".repeat(32);
    assert!(!activate_route(&mut governance, &mut ledger, &changed).accepted);
    let update = activate_route(&mut governance, &mut ledger, &next);
    assert!(update.accepted, "{}", update.message);
    assert_eq!(ledger.additional_solana_routes[&next.profile_hash().unwrap()].client, client);
}
