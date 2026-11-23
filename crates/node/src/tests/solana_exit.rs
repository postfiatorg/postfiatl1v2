use super::*;
use postfiat_types::{solana_exit, VaultBridgeBurnToRedeemOperation, VaultBridgeRouteProfileV1};

const ETH_HASH: &str = "29c91daa1c8d707ea3245db091976219266c5deca82b2d4571fafb221fd2dd092eb8833a28f4422bf0b4bbde44069e28";
const SERIES: &str = "e90627502bea5857aefd003d80af518f0ccb44ad2be8098ec6db2025ccaf8390e8bef31e7538b398a3c94f019d7b3b0f";

#[test]
fn reviewed_exit_profiles_match_the_validator_hashes() {
    let eth: VaultBridgeRouteProfileV1 = serde_json::from_str(include_str!(
        "../../tests/fixtures/solana-exit/ethereum-profile.json"
    ))
    .unwrap();
    assert_eq!(eth.route_epoch, 11);
    assert_eq!(eth.profile_hash().unwrap(), ETH_HASH);
    assert_eq!(
        solana_exit::descriptor().profile_hash().unwrap(),
        solana_exit::PROFILE_HASH
    );
    assert_eq!(
        solana_exit::release_height("unrelated-chain", "other-genesis"),
        None
    );
}

/// Run the scheduled release binary on the private copy mount. All changes are in
/// memory. Genesis, ledger and governance are the copied chain's canonical values.
#[test]
#[ignore = "requires copied height-1753 store and the scheduled rehearsal build"]
fn copied_solana_exit_governance_and_burn() {
    let store = NodeStore::new("/var/lib/postfiat/validator-3");
    let g = store.read_genesis().unwrap();
    let mut gov = store.read_governance().unwrap();
    let mut ledger = store.read_ledger().unwrap();
    let batch = serde_json::from_str(include_str!(
        "../../tests/fixtures/solana-exit/governance-batch.json"
    ))
    .unwrap();
    let eth = |gov: &GovernanceState| {
        let r = gov
            .active_vault_bridge_route_profile(solana_exit::FAMILY, 1755)
            .unwrap();
        assert_eq!(r.profile_hash, ETH_HASH);
        assert_eq!(r.profile.route_epoch, 11);
        serde_json::to_vec(&r.profile).unwrap()
    };
    let before = eth(&gov);
    assert!(solana_exit::active(&gov, 1753).is_err());
    let rejected = crate::execution_actions::execute_governance_batch(
        &mut gov.clone(),
        Some(&mut ledger.clone()),
        Some(&g),
        &batch,
        1753,
    );
    assert!(!rejected[0].accepted);
    let receipts = crate::execution_actions::execute_governance_batch(
        &mut gov,
        Some(&mut ledger),
        Some(&g),
        &batch,
        1754,
    );
    assert!(receipts[0].accepted, "{:?}", receipts[0]);
    let (exit, _) = solana_exit::active(&gov, 1754).unwrap();
    assert_eq!(exit.profile_hash, solana_exit::PROFILE_HASH);
    assert_eq!(eth(&gov), before);
    let expected_eth: VaultBridgeRouteProfileV1 = serde_json::from_str(include_str!(
        "../../tests/fixtures/solana-exit/ethereum-profile.json"
    ))
    .unwrap();
    assert_eq!(before, serde_json::to_vec(&expected_eth).unwrap());

    let report =
        |id: &str| crate::market_bridge::vault_bridge_route_at_height(&g, &gov, &ledger, 1755, id);
    let family = report(solana_exit::FAMILY).unwrap();
    let eth_series = report("541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7").unwrap();
    assert_eq!(family, eth_series);
    assert_eq!(serde_json::to_vec(&eth_series.profile).unwrap(), before);
    let sol = report(SERIES).unwrap();
    assert_eq!(sol.profile_hash, solana_exit::PROFILE_HASH);
    assert_eq!(sol.profile, exit.profile);
    for b in &ledger.vault_bridge_bucket_states {
        if b.source_domain.starts_with("spl:") && b.bucket_id != solana_exit::BUCKET {
            let earlier =
                postfiat_types::solana_source_series_id(&g.chain_id, &b.asset_id, &b.bucket_id);
            assert!(report(&earlier).is_err());
        }
    }

    // Reassign the test holder's local balance line to a fresh, isolated signer.
    let k = ml_dsa_65_keygen().unwrap();
    let owner = address_from_public_key(&k.public_key);
    let public = bytes_to_hex(&k.public_key);
    let private = bytes_to_hex(&k.private_key);
    let line = ledger
        .trustlines
        .iter_mut()
        .find(|l| l.asset_id == SERIES && l.account == "pf5c266a1bdb18320e140961411a8c4d8383eaafb2")
        .unwrap();
    let mut local = postfiat_types::TrustLine::new(
        &owner,
        &line.issuer,
        &line.asset_id,
        line.limit,
        line.reserve_paid,
    )
    .unwrap();
    local.balance = line.balance;
    local.authorized = line.authorized;
    *line = local;
    ledger.accounts.push(postfiat_types::Account::new(
        &owner,
        1_000_000,
        Some(public.clone()),
    ));
    let nav = ledger.nav_asset(solana_exit::FAMILY).unwrap();
    let op = VaultBridgeBurnToRedeemOperation {
        owner: owner.clone(),
        issuer: nav.issuer.clone(),
        asset_id: solana_exit::FAMILY.into(),
        bucket_id: solana_exit::BUCKET.into(),
        amount_atoms: 20000,
        epoch: nav.finalized_epoch,
        reserve_packet_hash: nav.finalized_reserve_packet_hash.clone(),
        destination_ref: format!(
            "solana-spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0:{}",
            "78".repeat(32)
        ),
    };
    let sign = |op| {
        signed_asset_transaction_for_test(
            &g,
            &ledger,
            &owner,
            &public,
            &private,
            postfiat_types::VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
            1,
            postfiat_types::AssetTransactionOperation::VaultBridgeBurnToRedeem(op),
        )
    };
    let tx = sign(op.clone());
    let mut wrong = op.clone();
    wrong.destination_ref = format!("evm-erc20:1:0x{}", "12".repeat(20));
    let compat = crate::execution_actions::asset_execution_compatibility_for_genesis_and_governance(
        &g, &gov,
    );
    let apply = |l: &mut LedgerState, t: &SignedAssetTransaction, h| {
        postfiat_execution::execute_asset_transaction_with_compatibility(&g, l, t, h, compat)
    };
    assert!(!apply(&mut ledger.clone(), &sign(wrong), 1755).accepted);
    assert!(!apply(&mut ledger.clone(), &tx, 1753).accepted);
    let before_bucket = ledger
        .vault_bridge_bucket(solana_exit::BUCKET)
        .unwrap()
        .clone();
    let shielded = store.read_shielded().unwrap();
    let bridge = store.read_bridge().unwrap();
    // This two-block fork uses a fresh local registry and its own trusted checkpoint.
    // Its signatures cannot authorize mainnet payout.
    let mut local_gov = gov.clone();
    local_gov.validator_registry_updates.clear();
    let root = |ledger: &LedgerState| {
        crate::state_commitment::replicated_state_root(
            &g,
            &local_gov,
            ledger,
            &[],
            &shielded,
            &bridge,
        )
        .unwrap()
    };
    let parent_root = root(&ledger);
    let receipt = apply(&mut ledger, &tx, 1755);
    assert!(receipt.accepted, "{receipt:?}");
    let after = ledger.vault_bridge_bucket(solana_exit::BUCKET).unwrap();
    assert_eq!(
        after.outstanding_vault_bridge_atoms + 20000,
        before_bucket.outstanding_vault_bridge_atoms
    );
    assert_eq!(
        after.redemption_queue_atoms,
        before_bucket.redemption_queue_atoms + 20000
    );
    let red = ledger
        .vault_bridge_redemptions
        .iter()
        .find(|r| r.withdrawal_packet.burn_tx_id == receipt.tx_id)
        .unwrap();
    let leaves = crate::batch_snapshot::bridge_exit_leaves_for_block(
        &gov,
        &ledger,
        std::slice::from_ref(&receipt),
        1755,
    )
    .unwrap();
    let local_path = Path::new("/evidence/local-store");
    let local = NodeStore::new(local_path);
    local.init(&g, &store.read_node_state().unwrap()).unwrap();
    local.write_governance(&local_gov).unwrap();
    local.write_ledger(&ledger).unwrap();
    local.write_shielded(&shielded).unwrap();
    local.write_bridge(&bridge).unwrap();
    local
        .write_receipts(std::slice::from_ref(&receipt))
        .unwrap();
    local
        .write_chain_tip(&store.read_chain_tip().unwrap())
        .unwrap();
    let output = serde_json::json!({"chain_id":g.chain_id,"genesis_hash":genesis_hash(&g),
        "protocol_version":g.protocol_version,"parent_state_root":parent_root,"state_root":root(&ledger),
        "prior":store.read_chain_tip().unwrap().block_hash,"transaction":tx,
        "bridge_exit_root_activation_height":crate::bridge_exit_root_activation_height_for_chain(&gov).unwrap(),"redemption":red,"receipt":receipt,"leaf":leaves[0],"profile":exit,
        "ethereum_profile_bytes_equal":true,"ethereum_epoch":11,"ethereum_profile_hash":ETH_HASH});
    std::fs::write(
        "/evidence/local-burn.json",
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .unwrap();
}

/// Install the locally certified block pair, then export the worker's witness from
/// the resulting node store through the same read path used by RPC.
#[test]
#[ignore = "requires the isolated certified Solana exit fork"]
fn export_local_solana_exit_chain() {
    let dir = Path::new("/evidence");
    let store = NodeStore::new(dir.join("local-store"));
    let witness: postfiat_types::PfUsdcEgressProofWitnessV1 =
        serde_json::from_slice(&std::fs::read(dir.join("local-witness.json")).unwrap()).unwrap();
    let blocks: BlockLog =
        serde_json::from_slice(&std::fs::read(dir.join("local-blocks.json")).unwrap()).unwrap();
    assert_eq!(blocks.blocks.len(), 2);
    assert_eq!(blocks.blocks[1], witness.block);
    let records = witness
        .committee
        .iter()
        .map(|r| crate::ValidatorRegistryRecord {
            node_id: r.node_id.clone(),
            algorithm_id: r.algorithm_id.clone(),
            public_key_hex: r.public_key_hex.clone(),
        })
        .collect();
    crate::storage_commit::write_validator_registry_file(
        &store.data_dir().join("validator_registry.json"),
        &crate::ValidatorRegistry {
            validators: records,
        },
    )
    .unwrap();
    store.write_blocks(&blocks).unwrap();
    let mut tip = store.read_chain_tip().unwrap();
    tip.height = witness.block.header.height;
    tip.block_hash = witness.block.header.block_hash.clone();
    tip.state_root = witness.block.header.state_root.clone();
    tip.receipt_count = 1;
    tip.history_base_height = blocks.blocks[0].header.height - 1;
    store.write_chain_tip(&tip).unwrap();
    let exported = crate::pfusdc_egress_witness(crate::PfUsdcEgressWitnessOptions {
        data_dir: store.data_dir().to_owned(),
        withdrawal_id: witness.withdrawal_packet.withdrawal_id.clone(),
        prior_checkpoint_block_id: Some(witness.prior_checkpoint_block_id.clone()),
    })
    .unwrap();
    assert_eq!(exported, witness);
}
