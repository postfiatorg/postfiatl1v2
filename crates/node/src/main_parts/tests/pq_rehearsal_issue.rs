//! Establish the allowed source series with signed transactions so a cold node
//! can reconstruct the backing, trustline and deposit without a state overlay.
use super::private_action_six_validator_tests::{
    governance_batch, sign_amendment, signed_asset_transaction, Fleet,
};
use postfiat_crypto_provider::*;
use postfiat_node::*;
use postfiat_storage::NodeStore;
use postfiat_types::*;
use std::fs;

#[path = "../../tests/vault_bridge_route_fixture.rs"]
mod route_fixture;

fn apply(fleet: &Fleet, key: &DevKeyFile, operation: AssetTransactionOperation) {
    let store = NodeStore::new(&fleet.data_dirs[0]);
    let genesis = store.read_genesis().unwrap();
    let ledger = store.read_ledger().unwrap();
    let tx = signed_asset_transaction(
        &genesis,
        &ledger,
        key,
        operation.transaction_kind(),
        ledger.account(&key.address).unwrap().sequence + 1,
        operation,
    );
    let domain = postfiat_mempool_dag::MempoolBatchDomain {
        genesis_hash: postfiat_execution::genesis_hash(&genesis),
        chain_id: genesis.chain_id,
        protocol_version: genesis.protocol_version,
    };
    let batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
        &domain,
        vec![],
        vec![],
        vec![tx],
    )
    .unwrap()
    .batch;
    let path = fleet.root.join("issue.batch.json");
    fs::write(&path, serde_json::to_vec(&batch).unwrap()).unwrap();
    fleet.round("transparent", path);
    let receipt = store.read_receipts().unwrap().pop().unwrap();
    assert!(receipt.accepted, "{} {}", receipt.code, receipt.message);
}

pub(super) fn shield_source_series(
    fleet: &Fleet,
    ids: &[String],
) -> pft_binius_shielded::payment::ShieldedWallet {
    let primary = &fleet.data_dirs[0];
    let store = NodeStore::new(primary);
    let genesis = store.read_genesis().unwrap();
    let faucet: DevKeyFile =
        serde_json::from_slice(&fs::read(primary.join("faucet_key.json")).unwrap()).unwrap();
    let holder: DevKeyFile =
        serde_json::from_slice(&fs::read(&fleet.holder_key_file).unwrap()).unwrap();
    let pair = ml_dsa_65_keygen_from_seed(&[7; 32]);
    let issuer = DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.into(),
        address: address_from_public_key(&pair.public_key),
        public_key_hex: bytes_to_hex(&pair.public_key),
        private_key_hex: bytes_to_hex(&pair.private_key),
    };
    let path = fleet.root.join("issuer-funding.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: primary.clone(),
        key_file: Some(primary.join("faucet_key.json")),
        to: issuer.address.clone(),
        amount: 100_000,
        batch_file: path.clone(),
    })
    .unwrap();
    fleet.round("transparent", path);
    let family = issued_asset_id(&genesis.chain_id, &issuer.address, "pfUSDC", 1).unwrap();
    let mut route = route_fixture::route(1, 1, "22");
    route.asset_id = family.clone();
    let policy = route.profile_hash().unwrap();
    let series = pfusdc_source_series_id(
        &genesis.chain_id,
        &family,
        route.source_chain_id,
        &route.vault_address,
        &route.token_address,
        1,
        &policy,
    )
    .unwrap();
    assert_eq!(series, PQ_REHEARSAL_ISSUE);
    for kind in [
        GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_PFUSDC_SOURCE_SERIES_ACTIVATION_HEIGHT,
    ] {
        let slot = fleet.height() + 1;
        let batch = governance_batch(primary, &fleet.root, ids, kind, (slot + 1) as u32, slot);
        fleet.round("governance", batch);
    }
    use AssetTransactionOperation as Op;
    apply(
        fleet,
        &issuer,
        Op::AssetCreate(AssetCreateOperation {
            issuer: issuer.address.clone(),
            code: "pfUSDC".into(),
            version: 1,
            precision: 6,
            display_name: "M8 governed asset".into(),
            max_supply: Some(110),
            requires_authorization: true,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    apply(fleet,&issuer,Op::NavProfileRegister(serde_json::from_value(serde_json::json!({
        "registrant":issuer.address,"verifier_kind":route.verifier_kind,"source_class":format!("vault_bridge:{}",route.source_domain()),
        "max_snapshot_age_blocks":route.max_snapshot_age_blocks,"challenge_window_blocks":route.challenge_window_blocks,
        "max_epoch_gap_blocks":route.max_epoch_gap_blocks,"settle_deadline_blocks":route.settle_deadline_blocks,
        "min_challenge_bond":route.min_challenge_bond,"min_attestations":route.min_attestations,
        "bridge_observer_min_confirmations":route.minimum_confirmations,"valuation_policy_hash":policy,"vault_bridge_route_policy_hash":policy
    })).unwrap()));
    let profile = store
        .read_ledger()
        .unwrap()
        .nav_proof_profiles
        .last()
        .unwrap()
        .profile_id
        .clone();
    apply(
        fleet,
        &issuer,
        Op::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.address.clone(),
            asset_id: family.clone(),
            reserve_operator: issuer.address.clone(),
            proof_profile: profile,
            valuation_unit: "USDC".into(),
            redemption_account: issuer.address.clone(),
        }),
    );
    let profile_file = fleet.root.join("route.json");
    fs::write(&profile_file, serde_json::to_vec(&route).unwrap()).unwrap();
    let unsigned = fleet.root.join("route.amendment");
    let batch = fleet.root.join("route.batch");
    create_vault_bridge_route_profile_governance(VaultBridgeRouteProfileGovernanceOptions {
        data_dir: primary.clone(),
        profile_file: profile_file.clone(),
        tier4_finality_bootstrap_file: None,
        validators: ids.to_vec(),
        support: ids.to_vec(),
        veto_until_height: 0,
        amendment_file: unsigned.clone(),
        batch_file: batch.clone(),
    })
    .unwrap();
    let slot = fleet.height() + 1;
    let signed = sign_amendment(primary, &fleet.root, ids, &unsigned, slot);
    assemble_signed_vault_bridge_route_profile_governance(
        SignedVaultBridgeRouteProfileGovernanceOptions {
            data_dir: primary.clone(),
            profile_file,
            tier4_finality_bootstrap_file: None,
            signed_amendment_file: signed,
            proposal_slot: slot,
            batch_file: batch.clone(),
        },
    )
    .unwrap();
    fleet.round("governance", batch);
    for (key, domain) in [
        (&faucet, "observer-one.test"),
        (&holder, "observer-two.test"),
    ] {
        apply(
            fleet,
            key,
            Op::NavAttestorRegister(NavAttestorRegisterOperation {
                attestor: key.address.clone(),
                domain: domain.into(),
                bond: 1,
            }),
        );
    }
    // These two local observers sign synthetic EVM facts for an owned rehearsal.
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: route.source_chain_id,
        vault_address: route.vault_address.clone(),
        token_address: route.token_address.clone(),
        depositor: format!("0x{}", "55".repeat(20)),
        pftl_recipient: holder.address.clone(),
        pftl_recipient_hash: vault_bridge_pftl_recipient_hash(&holder.address).unwrap(),
        amount_atoms: 100,
        nonce: "77".repeat(32),
        route_binding: vault_bridge_route_binding(&policy, 1).unwrap(),
        deposit_id: String::new(),
        block_hash: "99".repeat(32),
        tx_hash: "aa".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).unwrap();
    let root = vault_bridge_deposit_evidence_root(&evidence).unwrap();
    apply(
        fleet,
        &holder,
        Op::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: holder.address.clone(),
            asset_id: family.clone(),
            evidence_root: root.clone(),
            evidence: evidence.clone(),
            policy_hash: policy.clone(),
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: vec![],
            source_public_values: vec![],
            expires_at_height: fleet.height() + 90,
        }),
    );
    let proposed = fleet.height();
    let observation =
        VaultBridgeDepositObservation::success_for_evidence(&evidence, route.minimum_confirmations);
    for key in [&faucet, &holder] {
        apply(
            fleet,
            key,
            Op::VaultBridgeDepositAttest(VaultBridgeDepositAttestOperation {
                attestor: key.address.clone(),
                asset_id: family.clone(),
                evidence_root: root.clone(),
                pass: true,
                observation_root: vault_bridge_deposit_observation_root(&observation).unwrap(),
                observation: Some(observation.clone()),
            }),
        );
    }
    while fleet.height() < proposed + route.challenge_window_blocks {
        fleet.transparent_round();
    }
    apply(
        fleet,
        &holder,
        Op::VaultBridgeDepositFinalize(VaultBridgeDepositFinalizeOperation {
            finalizer: holder.address.clone(),
            asset_id: family.clone(),
            evidence_root: root.clone(),
        }),
    );
    apply(
        fleet,
        &holder,
        Op::VaultBridgeDepositClaim(VaultBridgeDepositClaimOperation {
            claimer: holder.address.clone(),
            asset_id: family.clone(),
            evidence_root: root,
            policy_hash: policy,
            recipient: holder.address.clone(),
            route_epoch: 1,
            amount_atoms: 100,
        }),
    );
    let ledger = store.read_ledger().unwrap();
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder.address, &series)
            .unwrap()
            .balance,
        100
    );
    use pft_binius_shielded::payment::{create_profiled_tx, ShieldedWallet, SpendInput};
    use postfiat_privacy_binius::{sign_l1_shield, ShieldOutputIntent};
    use rand::{rngs::StdRng, SeedableRng};
    let mut rng = StdRng::seed_from_u64(908);
    let wallet = ShieldedWallet::from_seed(&mut rng);
    let chain = pq_payment::Chain {
        id: &genesis.chain_id,
        genesis: hex_to_bytes(&postfiat_execution::genesis_hash(&genesis))
            .unwrap()
            .try_into()
            .unwrap(),
        protocol: genesis.protocol_version,
    };
    let terms = pq_payment::Terms {
        mode: pq_payment::Mode::Shield,
        asset: hex_to_bytes(&series).unwrap().try_into().unwrap(),
        epoch: 0,
        public_fee: 3,
        account: Some(pq_payment::Account {
            id: pq_payment::destination(&holder.address).unwrap()[12..]
                .try_into()
                .unwrap(),
            sequence: ledger.account(&holder.address).unwrap().sequence + 1,
            public_key: Box::new(
                hex_to_bytes(&holder.public_key_hex)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            ),
        }),
    };
    let (tx, r0, r1) = create_profiled_tx(
        &postfiat_privacy_binius::MerkleTree24::new(),
        SpendInput::Dummy,
        SpendInput::Dummy,
        &wallet.stealth_address(),
        100,
        &wallet.stealth_address(),
        0,
        100,
        0,
        [0; 32],
        0,
        None,
        Some(&|raw| terms.statement_context(chain, raw)),
        &mut rng,
    )
    .unwrap();
    let intents = [
        ShieldOutputIntent {
            owner: &wallet.addr,
            value: 100,
            blinder: &r0,
            envelope: &tx.env0,
        },
        ShieldOutputIntent {
            owner: &wallet.addr,
            value: 0,
            blinder: &r1,
            envelope: &tx.env1,
        },
    ];
    let payment = sign_l1_shield(
        chain,
        &terms,
        &tx.to_bytes(),
        100,
        0,
        intents,
        &hex_to_bytes(&holder.private_key_hex).unwrap(),
    )
    .unwrap();
    let batch =
        build_atomic_shielded_action_batch(&genesis, vec![ShieldedAction::PqPaymentV1(payment)])
            .unwrap();
    let path = fleet.root.join("pfusdc-shield.batch");
    fs::write(&path, serde_json::to_vec(&batch).unwrap()).unwrap();
    fleet.round("shielded", path);
    let shielded = store.read_shielded().unwrap();
    assert_eq!(
        shielded.pq_pool_v1.as_ref().unwrap().head.assets[&series].reserve,
        100
    );
    assert_eq!(
        global_issued_asset_supply(&store.read_ledger().unwrap(), &shielded, &family).unwrap(),
        100
    );
    wallet
}
