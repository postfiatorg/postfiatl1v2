use super::*;
use postfiat_types::*;

// A claim whose shielded family supply crosses the previous NAV checkpoint.
// Deposit finality is fixture pre-state; this test exercises signed execution,
// archive routing and supply accounting rather than an SP1 prover.
#[test]
fn archive_bridge_claim_matches_live_orchard_supply_at_activation() {
    let mut genesis = Genesis::new("archive-bridge-supply-regression");
    genesis.orchard_aware_bridge_claim_activation_height = Some(10);
    genesis.pfusdc_source_series_activation_height = Some(10);
    let key = ml_dsa_65_keygen().expect("issuer key");
    let issuer = address_from_public_key(&key.public_key);
    let recipient = address_from_public_key(&ml_dsa_65_keygen().unwrap().public_key);
    let asset = AssetDefinition::new(&genesis.chain_id, &issuer, "PFUSDC", 1, 6).unwrap();
    let policy_hash = "88".repeat(48);
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: ETHEREUM_MAINNET_CHAIN_ID,
        vault_address: "0x1111111111111111111111111111111111111111".into(),
        token_address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(),
        depositor: "0x3333333333333333333333333333333333333333".into(),
        pftl_recipient_hash: vault_bridge_pftl_recipient_hash(&issuer).unwrap(),
        pftl_recipient: issuer.clone(),
        amount_atoms: 15_000_000,
        nonce: "44".repeat(32),
        route_binding: vault_bridge_route_binding(&policy_hash, 6).unwrap(),
        deposit_id: String::new(),
        block_hash: "66".repeat(32),
        tx_hash: "77".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).unwrap();
    let evidence_root = vault_bridge_deposit_evidence_root(&evidence).unwrap();
    let profile = NavProofProfile::new(
        issuer.clone(), NAV_PROFILE_VERIFIER_SP1_GROTH16,
        format!("vault_bridge:{}", evidence.source_domain()),
        7_200, 64, 7_200, 7_200, 0, 0, 0,
        "99".repeat(32), format!("0x{}", "aa".repeat(32)),
        NAV_SP1_PROOF_ENCODING_GROTH16,
        DEFAULT_MAX_NAV_SP1_PROOF_BYTES, DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES,
    ).unwrap().with_vault_bridge_route_policy_hash(policy_hash.clone()).unwrap();
    let mut nav = NavTrackedAsset::new(
        asset.asset_id.clone(), issuer.clone(), issuer.clone(),
        profile.profile_id.clone(), "USDC_ATOMS", issuer.clone(),
    ).unwrap();
    nav.circulating_supply = 297_933_789;
    nav.nav_per_unit = 1_000_000;
    let mut deposit = VaultBridgeDepositRecord::new_with_source_nullifier(
        asset.asset_id.clone(), evidence_root.clone(), evidence, policy_hash.clone(),
        SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1,
        "bb".repeat(48), "cc".repeat(48), "dd".repeat(32), "fixture-proposer", 1, 10_000,
    ).unwrap();
    deposit.status = VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED.into();
    deposit.finalized_at_height = 1;
    let mut initial = LedgerState::new(vec![
        Account::new(issuer.clone(), 1_000_000, Some(bytes_to_hex(&key.public_key))),
        Account::new("pf-existing-holder", 10_000, None),
        Account::new(recipient.clone(), 10_000, None),
    ]);
    let mut line = TrustLine::new(
        "pf-existing-holder", issuer.clone(), asset.asset_id.clone(), u64::MAX, 0,
    ).unwrap();
    line.balance = 269_700_595;
    initial.asset_definitions.push(asset.clone());
    initial.nav_proof_profiles.push(profile);
    initial.nav_assets.push(nav);
    initial.trustlines.push(line);
    initial.vault_bridge_deposits.push(deposit);
    let orchard = vec![AssetOrchardAssetBalance {
        asset_id: asset.asset_id.clone(), ingress_total: 20_000_000,
        egress_total: 0, live_total: 20_000_000,
    }];
    let governance = GovernanceState::new(4);
    let compatibility = asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
    for height in [9, 10, 11] {
        let operation = VaultBridgeDepositClaimOperation {
            claimer: issuer.clone(), asset_id: asset.asset_id.clone(),
            evidence_root: evidence_root.clone(), policy_hash: policy_hash.clone(),
            route_epoch: if height >= 10 { 6 } else { 0 },
            recipient: recipient.clone(), amount_atoms: 15_000_000,
        };
        let tx = signed_asset_transaction_for_test(
            &genesis, &initial, &issuer, &bytes_to_hex(&key.public_key),
            &bytes_to_hex(&key.private_key), VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
            1, AssetTransactionOperation::VaultBridgeDepositClaim(operation),
        );
        let mut live = initial.clone();
        let live_receipt = execute_asset_transaction_with_compatibility_and_orchard(
            &genesis, &mut live, &tx, height, compatibility, &orchard,
        );
        assert!(live_receipt.accepted, "height {height}: {live_receipt:?}");
        let block = dummy_block_record(height);
        let mut replay = initial.clone();
        let replay_receipt = execute_asset_transaction_for_archive_replay(
            &genesis, &mut replay, &tx, &block, 0, &governance, &orchard,
        ).unwrap();
        assert_eq!(replay_receipt, live_receipt);
        assert_eq!(replay, live, "complete live/replay state at height {height}");
        assert_eq!(replay.nav_assets[0].circulating_supply,
            if height < 10 { 297_933_789 } else { 304_700_595 });
        let before = replay.clone();
        let duplicate = execute_asset_transaction_for_archive_replay(
            &genesis, &mut replay, &tx, &block, 0, &governance, &orchard,
        ).unwrap();
        assert!(!duplicate.accepted);
        assert_eq!(replay, before, "rejected repeat must preserve state");
    }
}
