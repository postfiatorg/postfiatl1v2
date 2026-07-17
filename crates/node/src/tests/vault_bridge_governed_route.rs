use postfiat_execution::AssetExecutionCompatibility;
use postfiat_types::{
    pftl_chain_numeric_id, vault_bridge_deposit_id, vault_bridge_pftl_recipient_hash,
    vault_bridge_route_amendment_kind, vault_bridge_route_binding,
    vault_bridge_source_root_for_asset, vault_bridge_withdrawal_execution_observation_root,
    Account, AssetCreateOperation, AssetDefinition, AssetTransactionOperation,
    GovernanceActionBatch, GovernanceAmendment, GovernanceState, NavAssetRegisterOperation,
    NavAttestorRegisterOperation, NavEpochFinalizeOperation, NavProfileRegisterOperation,
    NavProofProfile, NavReserveAttestOperation, NavReserveSubmitOperation, NavTrackedAsset,
    SignedAssetTransaction, TransactionBatch, TrustLine, UnsignedAssetTransaction,
    VaultBridgeDepositEvidence, VaultBridgeDepositFinalizeOperation,
    VaultBridgeDepositProposeOperation, VaultBridgeDepositRecord, VaultBridgeRedeemSettleOperation,
    VaultBridgeRouteProfileActivationV1, VaultBridgeRouteProfileV1,
    VaultBridgeWithdrawalExecutionAttestation, VaultBridgeWithdrawalExecutionObservation,
    ADDRESS_NAMESPACE, ASSET_CREATE_TRANSACTION_KIND,
    GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
    NAV_ASSET_REGISTER_TRANSACTION_KIND, NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
    NAV_EPOCH_FINALIZE_TRANSACTION_KIND, NAV_PROFILE_REGISTER_TRANSACTION_KIND,
    NAV_PROFILE_VERIFIER_MULTI_FETCH, NAV_PROFILE_VERIFIER_SP1_GROTH16,
    NAV_RESERVE_ATTEST_TRANSACTION_KIND, NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
    NAV_SP1_PROOF_ENCODING_GROTH16, VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
    VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND, VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
    VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED, VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND,
    VAULT_BRIDGE_REDEMPTION_STATE_SETTLED, VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1,
    VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1, VAULT_BRIDGE_UNIT,
};

use super::*;

fn amendment(kind: &str, value: u32, activation_height: u64) -> GovernanceAmendment {
    GovernanceAmendment {
        amendment_id: format!("governed-vault-route:{value}:{activation_height}"),
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "11".repeat(48),
        protocol_version: 1,
        instance_id: "governed-vault-route-instance".to_string(),
        proposal_id: "governed-vault-route-proposal".to_string(),
        certificate_id: "governed-vault-route-certificate".to_string(),
        proposer: "validator-0".to_string(),
        validators: vec!["validator-0".to_string()],
        quorum: 1,
        kind: kind.to_string(),
        value,
        activation_height,
        veto_until_height: 0,
        paused: false,
        support: vec!["validator-0".to_string()],
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
    }
}

fn sign_single_validator_amendment(
    data_dir: &Path,
    amendment_file: &Path,
    proposal_slot: u64,
    label: &str,
) -> PathBuf {
    let authorization_file = data_dir.join(format!("{label}.authorization.json"));
    sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
        data_dir: data_dir.to_path_buf(),
        amendment_file: amendment_file.to_path_buf(),
        validator: "validator-0".to_string(),
        validator_key_file: data_dir.join(VALIDATOR_KEYS_FILE),
        proposal_slot,
        expires_at_height: proposal_slot + 8,
        authorization_file: authorization_file.clone(),
    })
    .expect("sign single-validator governance authorization");
    let signed_file = data_dir.join(format!("{label}.signed.json"));
    assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
        data_dir: data_dir.to_path_buf(),
        amendment_file: amendment_file.to_path_buf(),
        authorization_files: vec![authorization_file],
        proposal_slot,
        output_file: signed_file.clone(),
    })
    .expect("assemble single-validator governance amendment");
    signed_file
}

fn route(epoch: u32, activation_height: u64, vault_hash_byte: &str) -> VaultBridgeRouteProfileV1 {
    VaultBridgeRouteProfileV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
        route_id: "arbitrum-pfusdc".to_string(),
        asset_id: "21".repeat(48),
        source_chain_id: 42_161,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        vault_runtime_code_hash: format!("0x{}", vault_hash_byte.repeat(32)),
        token_address: "0x3333333333333333333333333333333333333333".to_string(),
        token_runtime_code_hash: format!("0x{}", "44".repeat(32)),
        route_epoch: epoch,
        verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
        evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED.to_string(),
        verifier_policy_hash: String::new(),
        verifier_program_vkey: String::new(),
        verifier_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 6,
        max_epoch_gap_blocks: 1_000,
        settle_deadline_blocks: 1_000,
        min_challenge_bond: 1,
        min_attestations: 2,
        minimum_confirmations: 64,
        activation_height,
        expires_at_height: activation_height + 10_000,
    }
}

fn deposit_transaction(
    genesis: &Genesis,
    route: &VaultBridgeRouteProfileV1,
) -> SignedAssetTransaction {
    let recipient = "route-test-proposer".to_string();
    let mut evidence = VaultBridgeDepositEvidence {
        source_chain_id: route.source_chain_id,
        vault_address: route.vault_address.clone(),
        token_address: route.token_address.clone(),
        depositor: "0x5555555555555555555555555555555555555555".to_string(),
        pftl_recipient_hash: vault_bridge_pftl_recipient_hash(&recipient).expect("recipient hash"),
        pftl_recipient: recipient,
        amount_atoms: 1,
        nonce: "77".repeat(32),
        route_binding: vault_bridge_route_binding(
            &route.profile_hash().expect("route profile hash"),
            route.route_epoch,
        )
        .expect("route binding"),
        deposit_id: String::new(),
        block_hash: "99".repeat(32),
        tx_hash: "aa".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = vault_bridge_deposit_id(&evidence).expect("deposit id");
    SignedAssetTransaction {
        unsigned: UnsignedAssetTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: "route-test-proposer".to_string(),
            fee: 1,
            sequence: 1,
            operation: AssetTransactionOperation::VaultBridgeDepositPropose(
                VaultBridgeDepositProposeOperation {
                    proposer: "route-test-proposer".to_string(),
                    asset_id: route.asset_id.clone(),
                    evidence_root: "bb".repeat(48),
                    evidence,
                    policy_hash: route.profile_hash().expect("route profile hash"),
                    source_proof_kind: String::new(),
                    source_proof_hash: String::new(),
                    source_public_values_hash: String::new(),
                    expires_at_height: 1_000,
                },
            ),
        },
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "cc".repeat(32),
        signature_hex: "dd".repeat(64),
    }
}

fn route_ledger(route: &VaultBridgeRouteProfileV1) -> LedgerState {
    let route_hash = route.profile_hash().expect("route hash");
    let receipt_proven = route.verifier_kind == NAV_PROFILE_VERIFIER_SP1_GROTH16;
    let profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
        "issuer",
        route.verifier_kind.clone(),
        format!("vault_bridge:{}", route.source_domain()),
        route.max_snapshot_age_blocks,
        route.challenge_window_blocks,
        route.max_epoch_gap_blocks,
        route.settle_deadline_blocks,
        route.min_challenge_bond,
        route.min_attestations,
        10,
        if receipt_proven {
            0
        } else {
            route.minimum_confirmations
        },
        if receipt_proven {
            route.verifier_policy_hash.clone()
        } else {
            route_hash.clone()
        },
        if receipt_proven {
            route.verifier_program_vkey.clone()
        } else {
            String::new()
        },
        if receipt_proven {
            route.verifier_proof_encoding.clone()
        } else {
            String::new()
        },
        route.max_proof_bytes,
        route.max_public_values_bytes,
    )
    .expect("NAV proof profile")
    .with_vault_bridge_route_policy_hash(route_hash)
    .expect("route-bound NAV proof profile");
    let asset = NavTrackedAsset::new(
        route.asset_id.clone(),
        "issuer",
        "reserve-operator",
        profile.profile_id.clone(),
        "USDC",
        "redemption-account",
    )
    .expect("NAV asset");
    let mut ledger = LedgerState::new(Vec::new());
    ledger.nav_proof_profiles.push(profile);
    ledger.nav_assets.push(asset);
    ledger
}

fn activate_route(
    governance: &mut GovernanceState,
    ledger: &mut LedgerState,
    route: &VaultBridgeRouteProfileV1,
) -> Receipt {
    let activation = VaultBridgeRouteProfileActivationV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
        profile: route.clone(),
        amendment: amendment(
            &vault_bridge_route_amendment_kind(route).expect("route amendment kind"),
            route.route_epoch,
            route.activation_height,
        ),
    };
    let batch = GovernanceActionBatch::with_vault_bridge_route_profile_activation(
        format!("route-activation-{}", route.route_epoch),
        activation,
    );
    execute_governance_batch(governance, Some(ledger), &batch, route.activation_height)
        .into_iter()
        .next()
        .expect("route activation receipt")
}

fn execute_route_candidate(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &mut LedgerState,
    transaction: SignedAssetTransaction,
    height: u64,
) -> Receipt {
    let batch = TransactionBatch::new_with_asset_transactions(
        "governed-vault-route-batch",
        Vec::new(),
        Vec::new(),
        vec![transaction],
    );
    execute_transparent_batch(
        genesis,
        governance,
        ledger,
        &batch,
        height,
        AssetExecutionCompatibility::strict(),
    )
    .into_iter()
    .next()
    .expect("one route receipt")
}

#[test]
fn activated_route_authority_rejects_unruled_and_rotated_deposits_without_mutation() {
    let genesis = Genesis::new("postfiat-local");
    let first = route(1, 2, "22");
    let second = route(2, 3, "55");
    let mut governance = GovernanceState::new(1);
    governance.apply(amendment(
        GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
        2,
        0,
    ));
    let original = route_ledger(&first);

    let mut pre_activation_ledger = original.clone();
    let pre_activation = execute_route_candidate(
        &genesis,
        &governance,
        &mut pre_activation_ledger,
        deposit_transaction(&genesis, &first),
        1,
    );
    assert_ne!(pre_activation.code, "vault_bridge_route_authority_mismatch");

    let mut unruled_ledger = original.clone();
    let unruled = execute_route_candidate(
        &genesis,
        &governance,
        &mut unruled_ledger,
        deposit_transaction(&genesis, &first),
        2,
    );
    assert_eq!(unruled.code, "vault_bridge_route_authority_mismatch");
    assert_eq!(unruled_ledger, original, "route rejection mutated ledger");

    let mut ruled_ledger = original.clone();
    let activation_receipt = activate_route(&mut governance, &mut ruled_ledger, &first);
    assert!(activation_receipt.accepted, "{activation_receipt:?}");
    let ruled = execute_route_candidate(
        &genesis,
        &governance,
        &mut ruled_ledger,
        deposit_transaction(&genesis, &first),
        2,
    );
    assert_ne!(ruled.code, "vault_bridge_route_authority_mismatch");

    for mutation in ["binding", "vault", "token", "source-chain"] {
        let mut candidate = deposit_transaction(&genesis, &first);
        let AssetTransactionOperation::VaultBridgeDepositPropose(operation) =
            &mut candidate.unsigned.operation
        else {
            unreachable!("fixture is a vault bridge proposal");
        };
        match mutation {
            "binding" => operation.evidence.route_binding = "ff".repeat(32),
            "vault" => {
                operation.evidence.vault_address =
                    "0x9999999999999999999999999999999999999999".to_string()
            }
            "token" => {
                operation.evidence.token_address =
                    "0x8888888888888888888888888888888888888888".to_string()
            }
            "source-chain" => operation.evidence.source_chain_id = 1,
            _ => unreachable!(),
        }
        let before = ruled_ledger.clone();
        let receipt =
            execute_route_candidate(&genesis, &governance, &mut ruled_ledger, candidate, 2);
        assert_eq!(
            receipt.code, "vault_bridge_route_authority_mismatch",
            "{mutation} substitution was not rejected: {receipt:?}"
        );
        assert_eq!(ruled_ledger, before, "{mutation} rejection mutated ledger");
    }

    let mut rotated_route_ledger = route_ledger(&second);
    let rotation_receipt = activate_route(&mut governance, &mut rotated_route_ledger, &second);
    assert!(rotation_receipt.accepted, "{rotation_receipt:?}");
    let mut stale_ledger = original.clone();
    let stale = execute_route_candidate(
        &genesis,
        &governance,
        &mut stale_ledger,
        deposit_transaction(&genesis, &first),
        3,
    );
    assert_eq!(stale.code, "vault_bridge_route_authority_mismatch");
    assert_eq!(
        stale_ledger, original,
        "stale-route rejection mutated ledger"
    );

    let mut current_ledger = original.clone();
    let current = execute_route_candidate(
        &genesis,
        &governance,
        &mut current_ledger,
        deposit_transaction(&genesis, &second),
        3,
    );
    assert_ne!(current.code, "vault_bridge_route_authority_mismatch");

    let mut pinned_ledger = original.clone();
    let mut pinned_finalize = deposit_transaction(&genesis, &first);
    let AssetTransactionOperation::VaultBridgeDepositPropose(proposal) =
        &pinned_finalize.unsigned.operation
    else {
        unreachable!("fixture proposal")
    };
    let evidence_root =
        vault_bridge_deposit_evidence_root(&proposal.evidence).expect("pinned evidence root");
    pinned_ledger.vault_bridge_deposits.push(
        VaultBridgeDepositRecord::new(
            first.asset_id.clone(),
            evidence_root.clone(),
            proposal.evidence.clone(),
            first.profile_hash().expect("first route hash"),
            "",
            "",
            "",
            "route-test-proposer",
            2,
            1_000,
        )
        .expect("pinned deposit record"),
    );
    pinned_finalize.unsigned.transaction_kind =
        VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND.to_string();
    pinned_finalize.unsigned.operation = AssetTransactionOperation::VaultBridgeDepositFinalize(
        VaultBridgeDepositFinalizeOperation {
            finalizer: "route-test-proposer".to_string(),
            asset_id: first.asset_id.clone(),
            evidence_root,
        },
    );
    let pinned = execute_route_candidate(
        &genesis,
        &governance,
        &mut pinned_ledger,
        pinned_finalize,
        3,
    );
    assert_ne!(
        pinned.code, "vault_bridge_route_authority_mismatch",
        "rotation must not strand an operation pinned before rotation: {pinned:?}"
    );
}

#[test]
fn route_profile_record_commits_every_field_and_is_order_independent() {
    let first = route(1, 2, "22");
    let activation = VaultBridgeRouteProfileActivationV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
        profile: first.clone(),
        amendment: amendment(
            &vault_bridge_route_amendment_kind(&first).expect("route kind"),
            first.route_epoch,
            first.activation_height,
        ),
    };
    let record =
        postfiat_types::VaultBridgeRouteProfileRecordV1::new(&activation, 2).expect("route record");
    let mut baseline = Vec::new();
    append_vault_bridge_route_profile_record(&mut baseline, "route", &record);

    let mutations = [
        (
            "/schema",
            serde_json::json!("postfiat.vault_bridge.route_record.changed"),
        ),
        ("/profile_hash", serde_json::json!("ab".repeat(48))),
        (
            "/profile/schema",
            serde_json::json!("postfiat.vault_bridge.route_profile.changed"),
        ),
        ("/profile/route_id", serde_json::json!("changed-route")),
        ("/profile/asset_id", serde_json::json!("31".repeat(48))),
        ("/profile/source_chain_id", serde_json::json!(1)),
        (
            "/profile/vault_address",
            serde_json::json!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        ),
        (
            "/profile/vault_runtime_code_hash",
            serde_json::json!(format!("0x{}", "aa".repeat(32))),
        ),
        (
            "/profile/token_address",
            serde_json::json!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ),
        (
            "/profile/token_runtime_code_hash",
            serde_json::json!(format!("0x{}", "bb".repeat(32))),
        ),
        ("/profile/route_epoch", serde_json::json!(9)),
        (
            "/profile/verifier_kind",
            serde_json::json!("changed-verifier"),
        ),
        ("/profile/evidence_tier", serde_json::json!("changed-tier")),
        (
            "/profile/verifier_policy_hash",
            serde_json::json!("11".repeat(32)),
        ),
        (
            "/profile/verifier_program_vkey",
            serde_json::json!(format!("0x{}", "22".repeat(32))),
        ),
        (
            "/profile/verifier_proof_encoding",
            serde_json::json!(NAV_SP1_PROOF_ENCODING_GROTH16),
        ),
        ("/profile/max_proof_bytes", serde_json::json!(1)),
        ("/profile/max_public_values_bytes", serde_json::json!(1)),
        ("/profile/max_snapshot_age_blocks", serde_json::json!(101)),
        ("/profile/challenge_window_blocks", serde_json::json!(7)),
        ("/profile/max_epoch_gap_blocks", serde_json::json!(1_001)),
        ("/profile/settle_deadline_blocks", serde_json::json!(1_002)),
        ("/profile/min_challenge_bond", serde_json::json!(2)),
        ("/profile/min_attestations", serde_json::json!(3)),
        ("/profile/minimum_confirmations", serde_json::json!(65)),
        ("/profile/activation_height", serde_json::json!(3)),
        ("/profile/expires_at_height", serde_json::json!(10_003)),
        (
            "/governance_amendment_id",
            serde_json::json!("changed-amendment"),
        ),
        ("/authorized_height", serde_json::json!(3)),
    ];
    for (pointer, value) in mutations {
        let mut json = serde_json::to_value(&record).expect("record JSON");
        *json.pointer_mut(pointer).expect("record field") = value;
        let changed = serde_json::from_value(json).expect("changed record");
        let mut encoded = Vec::new();
        append_vault_bridge_route_profile_record(&mut encoded, "route", &changed);
        assert_ne!(
            encoded, baseline,
            "field {pointer} was omitted from state commitment"
        );
    }

    let second = route(2, 3, "55");
    let second_activation = VaultBridgeRouteProfileActivationV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
        profile: second.clone(),
        amendment: amendment(
            &vault_bridge_route_amendment_kind(&second).expect("second route kind"),
            second.route_epoch,
            second.activation_height,
        ),
    };
    let second_record = postfiat_types::VaultBridgeRouteProfileRecordV1::new(&second_activation, 3)
        .expect("second route record");
    let mut governance = GovernanceState::new(1);
    governance.vault_bridge_route_profiles = vec![record, second_record];
    let mut reversed = governance.clone();
    reversed.vault_bridge_route_profiles.reverse();
    let mut ordered_bytes = Vec::new();
    let mut reversed_bytes = Vec::new();
    append_governance_state(&mut ordered_bytes, &governance);
    append_governance_state(&mut reversed_bytes, &reversed);
    assert_eq!(
        ordered_bytes, reversed_bytes,
        "route record order changed state root"
    );

    let mut legacy_json = serde_json::to_value(GovernanceState::new(1)).expect("governance JSON");
    legacy_json
        .as_object_mut()
        .expect("governance object")
        .remove("vault_bridge_route_profiles");
    let restored: GovernanceState = serde_json::from_value(legacy_json).expect("legacy state");
    assert!(restored.vault_bridge_route_profiles.is_empty());
}

#[test]
fn governed_route_state_replays_snapshots_rolls_back_and_reapplies_byte_identically() {
    let root = unique_test_dir("postfiat-governed-route-state-matrix");
    let data_dir = root.join("source");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-route-state-matrix".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize route state matrix");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("route matrix genesis");
    let faucet = read_transfer_key_file(&data_dir, None).expect("route matrix faucet key");
    let asset_code = "pfUSDC-route-matrix";
    let asset_version = 1;
    let asset_id = postfiat_types::issued_asset_id(
        &genesis.chain_id,
        &faucet.address,
        asset_code,
        asset_version,
    )
    .expect("route matrix asset id");
    let mut profile = route(1, 3, "22");
    profile.asset_id = asset_id.clone();
    let route_hash = profile.profile_hash().expect("route matrix profile hash");
    let nav_profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
        &faucet.address,
        profile.verifier_kind.clone(),
        format!("vault_bridge:{}", profile.source_domain()),
        profile.max_snapshot_age_blocks,
        profile.challenge_window_blocks,
        profile.max_epoch_gap_blocks,
        profile.settle_deadline_blocks,
        profile.min_challenge_bond,
        profile.min_attestations,
        0,
        profile.minimum_confirmations,
        route_hash.clone(),
        "",
        "",
        0,
        0,
    )
    .expect("route matrix NAV profile")
    .with_vault_bridge_route_policy_hash(route_hash.clone())
    .expect("bind route matrix policy");

    let mut setup_ledger = store.read_ledger().expect("route matrix initial ledger");
    let create_asset = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet.address,
        &faucet.public_key_hex,
        &faucet.private_key_hex,
        ASSET_CREATE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: faucet.address.clone(),
            code: asset_code.to_string(),
            version: asset_version,
            precision: 6,
            display_name: "Route Matrix pfUSDC".to_string(),
            max_supply: Some(1_000_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
    );
    assert!(
        postfiat_execution::execute_asset_transaction(
            &genesis,
            &mut setup_ledger,
            &create_asset,
            1,
        )
        .accepted
    );
    let register_profile = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet.address,
        &faucet.public_key_hex,
        &faucet.private_key_hex,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::NavProfileRegister(NavProfileRegisterOperation {
            registrant: faucet.address.clone(),
            verifier_kind: profile.verifier_kind.clone(),
            source_class: format!("vault_bridge:{}", profile.source_domain()),
            max_snapshot_age_blocks: profile.max_snapshot_age_blocks,
            challenge_window_blocks: profile.challenge_window_blocks,
            max_epoch_gap_blocks: profile.max_epoch_gap_blocks,
            settle_deadline_blocks: profile.settle_deadline_blocks,
            min_challenge_bond: profile.min_challenge_bond,
            min_attestations: profile.min_attestations,
            tolerance_bp: 0,
            bridge_observer_min_confirmations: profile.minimum_confirmations,
            valuation_policy_hash: route_hash.clone(),
            vault_bridge_route_policy_hash: route_hash.clone(),
            sp1_program_vkey: String::new(),
            sp1_proof_encoding: String::new(),
            max_proof_bytes: 0,
            max_public_values_bytes: 0,
        }),
    );
    assert!(
        postfiat_execution::execute_asset_transaction(
            &genesis,
            &mut setup_ledger,
            &register_profile,
            1,
        )
        .accepted
    );
    let register_asset = signed_asset_transaction_for_test(
        &genesis,
        &setup_ledger,
        &faucet.address,
        &faucet.public_key_hex,
        &faucet.private_key_hex,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        3,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: faucet.address.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: faucet.address.clone(),
            proof_profile: nav_profile.profile_id.clone(),
            valuation_unit: "USDC".to_string(),
            redemption_account: faucet.address.clone(),
        }),
    );
    assert!(
        postfiat_execution::execute_asset_transaction(
            &genesis,
            &mut setup_ledger,
            &register_asset,
            1,
        )
        .accepted
    );
    let setup_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        vec![create_asset, register_profile, register_asset],
    )
    .expect("build route matrix asset batch")
    .batch;
    let setup_batch_file = root.join("setup.batch.json");
    write_batch_file(&setup_batch_file, &setup_batch).expect("write route matrix asset batch");
    let setup_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: setup_batch_file,
        certificate_file: None,
    })
    .expect("apply route matrix asset batch");
    assert!(setup_receipts.iter().all(|receipt| receipt.accepted));
    assert_eq!(
        store.read_ledger().expect("committed route ledger"),
        setup_ledger
    );

    let authority_amendment_file = root.join("authority.unsigned.json");
    ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: vec!["validator-0".to_string()],
        support: vec!["validator-0".to_string()],
        kind: GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT.to_string(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: authority_amendment_file.clone(),
    })
    .expect("create route authority amendment");
    let signed_authority =
        sign_single_validator_amendment(&data_dir, &authority_amendment_file, 2, "authority");
    let authority_batch_file = root.join("authority.batch.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(signed_authority),
        registry_update_file: None,
        batch_file: authority_batch_file.clone(),
    })
    .expect("build route authority batch");
    let authority_receipts = apply_governance_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: authority_batch_file,
        certificate_file: None,
    })
    .expect("commit route authority");
    assert!(authority_receipts.iter().all(|receipt| receipt.accepted));
    let before_route = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status before route activation");
    assert_eq!(before_route.block_height, 2);
    assert!(vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: data_dir.clone(),
        asset_id: asset_id.clone(),
    })
    .is_err());
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("pre-route history replay");

    let pre_snapshot = root.join("pre-route.snapshot");
    let pre_manifest = export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: pre_snapshot.clone(),
    })
    .expect("export pre-route snapshot");
    assert_eq!(pre_manifest.state_root, before_route.state_root);

    let profile_file = root.join("route-profile.json");
    std::fs::write(
        &profile_file,
        serde_json::to_vec_pretty(&profile).expect("route matrix profile JSON"),
    )
    .expect("write route matrix profile");
    let route_amendment_file = root.join("route.unsigned.json");
    create_vault_bridge_route_profile_governance(VaultBridgeRouteProfileGovernanceOptions {
        data_dir: data_dir.clone(),
        profile_file: profile_file.clone(),
        validators: vec!["validator-0".to_string()],
        support: vec!["validator-0".to_string()],
        veto_until_height: 0,
        amendment_file: route_amendment_file.clone(),
        batch_file: root.join("route.unsigned.batch.json"),
    })
    .expect("create route activation amendment");
    let signed_route_amendment =
        sign_single_validator_amendment(&data_dir, &route_amendment_file, 3, "route");
    let route_batch_file = root.join("route.signed.batch.json");
    assemble_signed_vault_bridge_route_profile_governance(
        SignedVaultBridgeRouteProfileGovernanceOptions {
            data_dir: data_dir.clone(),
            profile_file,
            signed_amendment_file: signed_route_amendment,
            proposal_slot: 3,
            batch_file: route_batch_file.clone(),
        },
    )
    .expect("assemble route activation batch");
    let validator_keys = read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("route matrix validator keys");
    write_split_validator_key_files(&data_dir, &validator_keys);
    let route_certificate_file = root.join("route.block-certificate.json");
    certify_batch_round(BatchCertificateRoundOptions {
        data_dir: data_dir.clone(),
        batch_kind: Some(BATCH_KIND_GOVERNANCE.to_string()),
        batch_file: route_batch_file.clone(),
        validator_key_dir: data_dir.clone(),
        vote_dir: root.join("route-votes"),
        proposal_file: root.join("route.block-proposal.json"),
        certificate_file: route_certificate_file.clone(),
        block_height: Some(3),
        view: None,
        timeout_certificate_file: None,
        skip_block_log_verify: false,
    })
    .expect("certify governed route batch");
    let route_receipts = apply_governance_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: route_batch_file.clone(),
        certificate_file: Some(route_certificate_file.clone()),
    })
    .expect("commit governed route");
    assert!(route_receipts.iter().all(|receipt| receipt.accepted));
    let after_route = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status after route activation");
    assert_eq!(after_route.block_height, 3);
    let active_route = vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: data_dir.clone(),
        asset_id: asset_id.clone(),
    })
    .expect("discover active route after activation");
    assert_eq!(active_route.profile_hash, route_hash);
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("post-route history replay");

    let post_snapshot = root.join("post-route.snapshot");
    let post_manifest = export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: post_snapshot.clone(),
    })
    .expect("export post-route snapshot");
    assert_eq!(post_manifest.state_root, after_route.state_root);

    let rollback_dir = root.join("rollback");
    let rolled_back = import_snapshot(SnapshotImportOptions {
        data_dir: rollback_dir.clone(),
        snapshot_dir: pre_snapshot,
        node_id: Some("validator-rollback".to_string()),
    })
    .expect("restore pre-route rollback point");
    assert_eq!(rolled_back.state_root, before_route.state_root);
    assert_eq!(rolled_back.block_tip_hash, before_route.block_tip_hash);
    assert!(vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: rollback_dir.clone(),
        asset_id: asset_id.clone(),
    })
    .is_err());
    let reapplied_receipts = apply_governance_batch(ApplyBatchOptions {
        data_dir: rollback_dir.clone(),
        batch_file: route_batch_file,
        certificate_file: Some(route_certificate_file),
    })
    .expect("reapply exact route batch after rollback");
    assert!(reapplied_receipts.iter().all(|receipt| receipt.accepted));
    let reapplied = status(NodeOptions {
        data_dir: rollback_dir.clone(),
    })
    .expect("status after route reapply");
    assert_eq!(reapplied.state_root, after_route.state_root);
    assert_eq!(reapplied.block_tip_hash, after_route.block_tip_hash);
    verify_blocks(NodeOptions {
        data_dir: rollback_dir,
    })
    .expect("reapplied route history replay");

    let restored_dir = root.join("post-route-restored");
    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: post_snapshot,
        node_id: Some("validator-restored".to_string()),
    })
    .expect("restore post-route snapshot");
    assert_eq!(restored.state_root, after_route.state_root);
    assert_eq!(restored.block_tip_hash, after_route.block_tip_hash);
    let restored_route = vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: restored_dir.clone(),
        asset_id,
    })
    .expect("discover route after snapshot restore");
    assert_eq!(restored_route.profile, profile);
    verify_blocks(NodeOptions {
        data_dir: restored_dir,
    })
    .expect("restored route history replay");
    std::fs::remove_dir_all(root).expect("remove route state matrix");
}

#[test]
fn route_discovery_returns_only_the_profile_authenticated_by_chain_state() {
    let data_dir = unique_test_dir("postfiat-governed-route-discovery");
    let store = NodeStore::new(&data_dir);
    let genesis = Genesis::new("postfiat-local");
    let profile = route(1, 2, "22");
    let mut ledger = route_ledger(&profile);
    let mut governance = GovernanceState::new(1);
    governance.apply(amendment(
        GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
        1,
        0,
    ));
    let receipt = activate_route(&mut governance, &mut ledger, &profile);
    assert!(receipt.accepted, "{receipt:?}");
    store.write_genesis(&genesis).expect("write genesis");
    store
        .write_governance(&governance)
        .expect("write governance");
    store.write_ledger(&ledger).expect("write ledger");
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height: 2,
            block_hash: "route-activation-block".to_string(),
            state_root: "route-activation-root".to_string(),
            ordered_batch_count: 1,
            receipt_count: 1,
            history_base_height: 0,
        })
        .expect("write chain tip");

    let report = vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: data_dir.clone(),
        asset_id: profile.asset_id.clone(),
    })
    .expect("discover route from chain state");
    assert_eq!(report.profile, profile);
    assert_eq!(
        report.route_binding,
        vault_bridge_route_binding(&report.profile_hash, report.governance_route_epoch)
            .expect("reported route binding")
    );
    assert_eq!(
        report.profile_hash,
        report.profile.profile_hash().expect("profile hash")
    );
    assert_eq!(report.governance_route_epoch, 1);
    assert!(report.active);

    let missing = vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: data_dir.clone(),
        asset_id: "31".repeat(48),
    })
    .expect_err("unruled asset must fail closed");
    assert_eq!(missing.kind(), io::ErrorKind::PermissionDenied);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn route_discovery_promotes_verifier_without_changing_api_or_accounting() {
    let data_dir = unique_test_dir("postfiat-governed-route-stronger-verifier");
    let store = NodeStore::new(&data_dir);
    let genesis = Genesis::new("postfiat-local");
    let mut profile = route(2, 3, "55");
    profile.verifier_kind = NAV_PROFILE_VERIFIER_SP1_GROTH16.to_string();
    profile.evidence_tier = postfiat_types::VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN.to_string();
    profile.verifier_policy_hash = "77".repeat(32);
    profile.verifier_program_vkey = format!("0x{}", "66".repeat(32));
    profile.verifier_proof_encoding = NAV_SP1_PROOF_ENCODING_GROTH16.to_string();
    profile.min_attestations = 0;
    profile.minimum_confirmations = 0;
    let mut mismatched_ledger = route_ledger(&profile);
    mismatched_ledger.nav_proof_profiles[0].sp1_program_vkey = format!("0x{}", "99".repeat(32));
    let mut mismatched_governance = GovernanceState::new(1);
    mismatched_governance.apply(amendment(
        GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
        1,
        0,
    ));
    let mismatch = activate_route(&mut mismatched_governance, &mut mismatched_ledger, &profile);
    assert!(!mismatch.accepted, "mismatched verifier contract activated");
    assert_eq!(mismatch.code, "vault_bridge_route_profile_rejected");

    let mut ledger = route_ledger(&profile);
    let mut governance = GovernanceState::new(1);
    governance.apply(amendment(
        GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
        1,
        0,
    ));
    let receipt = activate_route(&mut governance, &mut ledger, &profile);
    assert!(receipt.accepted, "{receipt:?}");
    let ledger_before = ledger.clone();
    store.write_genesis(&genesis).expect("write genesis");
    store
        .write_governance(&governance)
        .expect("write governance");
    store.write_ledger(&ledger).expect("write ledger");
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height: 3,
            block_hash: "stronger-verifier-block".to_string(),
            state_root: "stronger-verifier-root".to_string(),
            ordered_batch_count: 1,
            receipt_count: 1,
            history_base_height: 0,
        })
        .expect("write chain tip");

    let report = vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: data_dir.clone(),
        asset_id: profile.asset_id.clone(),
    })
    .expect("discover receipt-proven route through unchanged API");
    assert_eq!(profile, report.profile);
    assert_eq!(
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        report.nav_profile_verifier_kind
    );
    assert_eq!(
        postfiat_types::VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN,
        report.profile.evidence_tier
    );
    assert_eq!(
        ledger_before,
        store
            .read_ledger()
            .expect("ledger after read-only discovery"),
        "verifier promotion changed transaction accounting"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn route_profile_governance_requires_signed_hash_bound_authorization() {
    let root = unique_test_dir("postfiat-governed-route-signed");
    let data_dir = root.join("node");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-route-signed".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize signed route fixture");
    let profile = route(1, 7, "22");
    let profile_file = root.join("route-profile.json");
    let amendment_file = root.join("route-amendment.json");
    std::fs::write(
        &profile_file,
        serde_json::to_vec_pretty(&profile).expect("profile JSON"),
    )
    .expect("write profile");
    let unsigned =
        create_vault_bridge_route_profile_governance(VaultBridgeRouteProfileGovernanceOptions {
            data_dir: data_dir.clone(),
            profile_file: profile_file.clone(),
            validators: vec!["validator-0".to_string()],
            support: vec!["validator-0".to_string()],
            veto_until_height: 0,
            amendment_file: amendment_file.clone(),
            batch_file: root.join("route-unsigned-batch.json"),
        })
        .expect("create hash-bound route governance");
    assert_eq!(
        unsigned.vault_bridge_route_profile_activations[0].profile,
        profile
    );

    let authorization_file = root.join("route-authorization.json");
    sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
        data_dir: data_dir.clone(),
        amendment_file: amendment_file.clone(),
        validator: "validator-0".to_string(),
        validator_key_file: data_dir.join(VALIDATOR_KEYS_FILE),
        proposal_slot: 1,
        expires_at_height: 9,
        authorization_file: authorization_file.clone(),
    })
    .expect("sign route governance authorization");
    let signed_amendment_file = root.join("route-signed-amendment.json");
    assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
        data_dir: data_dir.clone(),
        amendment_file,
        authorization_files: vec![authorization_file],
        proposal_slot: 1,
        output_file: signed_amendment_file.clone(),
    })
    .expect("assemble signed route amendment");
    let signed = assemble_signed_vault_bridge_route_profile_governance(
        SignedVaultBridgeRouteProfileGovernanceOptions {
            data_dir: data_dir.clone(),
            profile_file: profile_file.clone(),
            signed_amendment_file: signed_amendment_file.clone(),
            proposal_slot: 1,
            batch_file: root.join("route-signed-batch.json"),
        },
    )
    .expect("assemble signed route activation");
    assert_eq!(
        signed.vault_bridge_route_profile_activations[0]
            .amendment
            .signed_authorizations
            .len(),
        1
    );

    let mut substituted = profile;
    substituted.vault_address = "0x9999999999999999999999999999999999999999".to_string();
    std::fs::write(
        &profile_file,
        serde_json::to_vec_pretty(&substituted).expect("substituted profile JSON"),
    )
    .expect("write substituted profile");
    let error = assemble_signed_vault_bridge_route_profile_governance(
        SignedVaultBridgeRouteProfileGovernanceOptions {
            data_dir,
            profile_file,
            signed_amendment_file,
            proposal_slot: 1,
            batch_file: root.join("route-substituted-batch.json"),
        },
    )
    .expect_err("signed route authorization must not permit profile substitution");
    assert!(
        error
            .to_string()
            .contains("does not match its governance amendment"),
        "{error}"
    );

    std::fs::remove_dir_all(root).expect("remove signed route fixture");
}

const ANVIL_TEST_PRIVATE_KEY: &str =
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const ANVIL_TEST_ADDRESS: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

struct GovernedBridgeAnvil {
    child: std::process::Child,
    rpc_url: String,
}

impl Drop for GovernedBridgeAnvil {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn foundry_binary(name: &str) -> PathBuf {
    if let Ok(path) = std::env::var(format!("POSTFIAT_{}_BIN", name.to_ascii_uppercase())) {
        let path = PathBuf::from(path);
        assert!(path.is_file(), "configured {name} binary is not a file");
        return path;
    }
    if let Some(path) = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    }) {
        return path;
    }
    let home = std::env::var_os("HOME").expect("HOME is required to locate Foundry");
    let path = PathBuf::from(home).join(".foundry/bin").join(name);
    assert!(
        path.is_file(),
        "missing Foundry binary `{}`",
        path.display()
    );
    path
}

fn run_external_json(
    binary: &Path,
    current_dir: Option<&Path>,
    args: &[String],
    label: &str,
) -> serde_json::Value {
    let mut command = std::process::Command::new(binary);
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let output = command.output().unwrap_or_else(|error| {
        panic!(
            "failed to start {label} using `{}`: {error}",
            binary.display()
        )
    });
    assert!(
        output.status.success(),
        "{label} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{label} returned invalid JSON: {error}: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        )
    })
}

fn run_external_text(binary: &Path, args: &[String], label: &str) -> String {
    let output = std::process::Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to start {label} using `{}`: {error}",
                binary.display()
            )
        });
    assert!(
        output.status.success(),
        "{label} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout)
        .expect("external command output must be UTF-8")
        .trim()
        .to_string()
}

fn start_governed_bridge_anvil(anvil: &Path, cast: &Path) -> GovernedBridgeAnvil {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve Anvil port");
    let port = listener.local_addr().expect("Anvil port").port();
    drop(listener);
    let rpc_url = format!("http://127.0.0.1:{port}");
    let child = std::process::Command::new(anvil)
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--chain-id",
            "31337",
            "--silent",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start isolated Anvil");
    let mut process = GovernedBridgeAnvil { child, rpc_url };
    for _ in 0..100 {
        if process.child.try_wait().expect("poll Anvil").is_some() {
            panic!("isolated Anvil exited during startup");
        }
        let ready = std::process::Command::new(cast)
            .args(["chain-id", "--rpc-url", &process.rpc_url])
            .output()
            .is_ok_and(|output| output.status.success());
        if ready {
            return process;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("isolated Anvil did not become ready");
}

fn deploy_contract(
    forge: &Path,
    contracts_root: &Path,
    rpc_url: &str,
    contract: &str,
    constructor_args: &[String],
) -> (String, String) {
    let mut args = vec![
        "create".to_string(),
        "--offline".to_string(),
        "--broadcast".to_string(),
        "--json".to_string(),
        "--rpc-url".to_string(),
        rpc_url.to_string(),
        "--private-key".to_string(),
        ANVIL_TEST_PRIVATE_KEY.to_string(),
        contract.to_string(),
    ];
    if !constructor_args.is_empty() {
        args.push("--constructor-args".to_string());
        args.extend_from_slice(constructor_args);
    }
    let report = run_external_json(forge, Some(contracts_root), &args, "contract deployment");
    let address = report
        .get("deployedTo")
        .and_then(serde_json::Value::as_str)
        .expect("deployment address")
        .to_ascii_lowercase();
    let tx_hash = report
        .get("transactionHash")
        .and_then(serde_json::Value::as_str)
        .expect("deployment transaction hash")
        .to_ascii_lowercase();
    (address, tx_hash)
}

fn cast_send_receipt(
    cast: &Path,
    rpc_url: &str,
    target: &str,
    signature: &str,
    call_args: &[String],
    label: &str,
) -> serde_json::Value {
    let mut args = vec![
        "send".to_string(),
        "--json".to_string(),
        "--rpc-url".to_string(),
        rpc_url.to_string(),
        "--private-key".to_string(),
        ANVIL_TEST_PRIVATE_KEY.to_string(),
        target.to_string(),
        signature.to_string(),
    ];
    args.extend_from_slice(call_args);
    let receipt = run_external_json(cast, None, &args, label);
    assert_eq!(
        receipt.get("status").and_then(serde_json::Value::as_str),
        Some("0x1"),
        "{label} receipt was not successful"
    );
    receipt
}

fn cast_rpc(cast: &Path, rpc_url: &str, method: &str, params: &[&str]) {
    let mut args = vec![
        "rpc".to_string(),
        "--rpc-url".to_string(),
        rpc_url.to_string(),
        method.to_string(),
    ];
    args.extend(params.iter().map(|value| (*value).to_string()));
    let _ = run_external_text(cast, &args, method);
}

fn cast_call(cast: &Path, rpc_url: &str, target: &str, signature: &str, args: &[&str]) -> String {
    let mut command = vec![
        "call".to_string(),
        target.to_string(),
        signature.to_string(),
    ];
    command.extend(args.iter().map(|value| (*value).to_string()));
    command.extend(["--rpc-url".to_string(), rpc_url.to_string()]);
    run_external_text(cast, &command, signature)
}

fn cast_call_failure(
    cast: &Path,
    rpc_url: &str,
    target: &str,
    signature: &str,
    args: &[&str],
    label: &str,
) -> String {
    let mut command = std::process::Command::new(cast);
    command.args(["call", target, signature]);
    command.args(args);
    command.args(["--rpc-url", rpc_url]);
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to start {label}: {error}"));
    assert!(!output.status.success(), "{label} unexpectedly succeeded");
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

fn deployed_runtime_hash(cast: &Path, rpc_url: &str, address: &str) -> String {
    use sha3::{Digest as _, Keccak256};

    let code = run_external_text(
        cast,
        &[
            "code".to_string(),
            address.to_string(),
            "--rpc-url".to_string(),
            rpc_url.to_string(),
        ],
        "runtime code fetch",
    );
    let bytes = hex_to_bytes(code.trim_start_matches("0x")).expect("runtime code hex");
    assert!(!bytes.is_empty(), "deployed runtime code must be nonempty");
    format!("0x{}", bytes_to_hex(&Keccak256::digest(bytes)))
}

fn sign_governed_bridge_asset(
    genesis: &Genesis,
    key: &postfiat_crypto_provider::MlDsa65KeyPair,
    transaction_kind: &str,
    sequence: u64,
    operation: AssetTransactionOperation,
) -> SignedAssetTransaction {
    let unsigned = UnsignedAssetTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: transaction_kind.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: address_from_public_key(&key.public_key),
        fee: 100,
        sequence,
        operation,
    };
    let signature = ml_dsa_65_sign(&key.private_key, &unsigned.signing_bytes())
        .expect("sign governed bridge transaction");
    SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&key.public_key),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn execute_governed_bridge_asset(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &mut LedgerState,
    transaction: SignedAssetTransaction,
    height: u64,
    label: &str,
) -> Receipt {
    let batch = TransactionBatch::new_with_asset_transactions(
        format!("governed-bridge-roundtrip-{height}-{label}"),
        Vec::new(),
        Vec::new(),
        vec![transaction],
    );
    let receipts = execute_transparent_batch(
        genesis,
        governance,
        ledger,
        &batch,
        height,
        AssetExecutionCompatibility::strict(),
    );
    assert_eq!(receipts.len(), 1, "{label} must emit one PFTL receipt");
    let receipt = receipts.into_iter().next().expect("one PFTL receipt");
    assert!(receipt.accepted, "{label} rejected: {receipt:?}");
    assert_ne!(
        receipt.code, "rejected",
        "{label} has rejected receipt code"
    );
    receipt
}

fn persist_governed_bridge_state(
    store: &NodeStore,
    governance: &GovernanceState,
    ledger: &LedgerState,
    height: u64,
    receipt_count: u64,
) {
    let genesis = store.read_genesis().expect("roundtrip genesis");
    store
        .write_governance(governance)
        .expect("persist governed route");
    store.write_ledger(ledger).expect("persist bridge ledger");
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height,
            block_hash: format!("{height:096x}"),
            state_root: format!("{:096x}", height + 1),
            ordered_batch_count: height,
            receipt_count,
            history_base_height: 0,
        })
        .expect("persist bridge chain tip");
}

fn receipt_hash_field(receipt: &serde_json::Value, field: &str) -> String {
    receipt
        .get(field)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("source receipt has no `{field}`"))
        .trim_start_matches("0x")
        .to_ascii_lowercase()
}

fn receipt_log_index(receipt: &serde_json::Value) -> u64 {
    let value = receipt
        .get("logs")
        .and_then(serde_json::Value::as_array)
        .and_then(|logs| logs.last())
        .and_then(|log| log.get("logIndex"))
        .and_then(serde_json::Value::as_str)
        .expect("source receipt log index");
    u64::from_str_radix(value.trim_start_matches("0x"), 16).expect("source log index hex")
}

/// Runs the production source contracts on isolated Anvil and the production
/// PFTL state transition path against one route profile. This stays ignored in
/// ordinary Rust tests because it requires Foundry binaries, but is a mandatory
/// public-candidate bridge gate.
#[test]
#[ignore = "requires local Foundry binaries and starts an isolated Anvil"]
fn governed_route_real_anvil_deposit_withdrawal_roundtrip() {
    let anvil_binary = foundry_binary("anvil");
    let cast_binary = foundry_binary("cast");
    let forge_binary = foundry_binary("forge");
    let anvil = start_governed_bridge_anvil(&anvil_binary, &cast_binary);
    let rpc_url = anvil.rpc_url.clone();
    let contracts_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ethereum-contracts");
    let root = std::env::temp_dir().join(format!(
        "postfiat-governed-bridge-anvil-roundtrip-{}",
        std::process::id()
    ));
    let data_dir = root.join("node");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create roundtrip root");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize isolated PFTL state");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("isolated PFTL genesis");
    let issuer_key = ml_dsa_65_keygen().expect("issuer key");
    let holder_key = ml_dsa_65_keygen().expect("holder key");
    let issuer = address_from_public_key(&issuer_key.public_key);
    let holder = address_from_public_key(&holder_key.public_key);
    let asset = AssetDefinition::new(&genesis.chain_id, &issuer, "pfUSDC", 1, 6)
        .expect("bridge asset definition");
    let amount = 1_000_000_u64;
    let pftl_chain_id = pftl_chain_numeric_id(&genesis.chain_id).expect("numeric PFTL chain id");

    let (token_address, token_deploy_tx) = deploy_contract(
        &forge_binary,
        &contracts_root,
        &rpc_url,
        "test/ERC20BridgeVault.t.sol:MockERC20",
        &[],
    );
    let (verifier_address, verifier_deploy_tx) = deploy_contract(
        &forge_binary,
        &contracts_root,
        &rpc_url,
        "src/PFTLWithdrawalVerifier.sol:PFTLWithdrawalVerifier",
        &[
            ANVIL_TEST_ADDRESS.to_string(),
            format!("[{ANVIL_TEST_ADDRESS}]"),
            "1".to_string(),
            "1".to_string(),
            "3600".to_string(),
        ],
    );
    let (vault_address, vault_deploy_tx) = deploy_contract(
        &forge_binary,
        &contracts_root,
        &rpc_url,
        "src/ERC20BridgeVault.sol:ERC20BridgeVault",
        &[
            token_address.clone(),
            verifier_address.clone(),
            ANVIL_TEST_ADDRESS.to_string(),
            pftl_chain_id.to_string(),
            format!("0x{}", asset.asset_id),
            "1".to_string(),
            "3600".to_string(),
        ],
    );
    let route = VaultBridgeRouteProfileV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
        route_id: "anvil-pfusdc-roundtrip".to_string(),
        asset_id: asset.asset_id.clone(),
        source_chain_id: 31_337,
        vault_address: vault_address.clone(),
        vault_runtime_code_hash: deployed_runtime_hash(&cast_binary, &rpc_url, &vault_address),
        token_address: token_address.clone(),
        token_runtime_code_hash: deployed_runtime_hash(&cast_binary, &rpc_url, &token_address),
        route_epoch: 1,
        verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
        evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED.to_string(),
        verifier_policy_hash: String::new(),
        verifier_program_vkey: String::new(),
        verifier_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 100,
        settle_deadline_blocks: 100,
        min_challenge_bond: 1,
        min_attestations: 1,
        minimum_confirmations: 1,
        activation_height: 1,
        expires_at_height: 1_000,
    };
    route.validate().expect("real governed route");
    let route_hash = route.profile_hash().expect("real governed route hash");
    let nav_profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
        issuer.clone(),
        route.verifier_kind.clone(),
        format!("vault_bridge:{}", route.source_domain()),
        route.max_snapshot_age_blocks,
        route.challenge_window_blocks,
        route.max_epoch_gap_blocks,
        route.settle_deadline_blocks,
        route.min_challenge_bond,
        route.min_attestations,
        0,
        route.minimum_confirmations,
        route_hash.clone(),
        "",
        "",
        0,
        0,
    )
    .expect("bridge NAV profile")
    .with_vault_bridge_route_policy_hash(route_hash.clone())
    .expect("route-bound NAV profile");
    let nav_asset = NavTrackedAsset::new(
        asset.asset_id.clone(),
        issuer.clone(),
        issuer.clone(),
        nav_profile.profile_id.clone(),
        "USDC",
        issuer.clone(),
    )
    .expect("bridge NAV asset");
    let trustline = TrustLine::new(
        holder.clone(),
        issuer.clone(),
        asset.asset_id.clone(),
        10_000_000,
        10,
    )
    .expect("holder trustline");
    let mut ledger = LedgerState::new(vec![
        Account::new(
            issuer.clone(),
            100_000,
            Some(bytes_to_hex(&issuer_key.public_key)),
        ),
        Account::new(
            holder.clone(),
            100_000,
            Some(bytes_to_hex(&holder_key.public_key)),
        ),
    ]);
    ledger.asset_definitions.push(asset.clone());
    ledger.nav_proof_profiles.push(nav_profile);
    ledger.nav_assets.push(nav_asset);
    ledger.trustlines.push(trustline);
    let mut governance = GovernanceState::new(1);
    governance.apply(amendment(
        GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
        1,
        0,
    ));
    let route_receipt = activate_route(&mut governance, &mut ledger, &route);
    assert!(
        route_receipt.accepted,
        "route activation: {route_receipt:?}"
    );
    persist_governed_bridge_state(&store, &governance, &ledger, 1, 1);

    let mint_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &token_address,
        "mint(address,uint256)",
        &[ANVIL_TEST_ADDRESS.to_string(), amount.to_string()],
        "test-token mint",
    );
    let approve_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &token_address,
        "approve(address,uint256)",
        &[vault_address.clone(), amount.to_string()],
        "vault allowance",
    );
    let route_binding =
        vault_bridge_route_binding(&route_hash, route.route_epoch).expect("source route binding");
    let source_deposit_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &vault_address,
        "depositV2(uint256,string,bytes32,bytes32)",
        &[
            amount.to_string(),
            holder.clone(),
            format!("0x{}", "42".repeat(32)),
            format!("0x{route_binding}"),
        ],
        "governed source deposit",
    );
    cast_rpc(&cast_binary, &rpc_url, "evm_mine", &[]);
    let deposit_tx_hash = format!(
        "0x{}",
        receipt_hash_field(&source_deposit_receipt, "transactionHash")
    );
    let relay = vault_bridge_deposit_relay_rpc_bundle(VaultBridgeDepositRelayRpcBundleOptions {
        source_rpc_url: rpc_url.clone(),
        tx_hash: deposit_tx_hash.clone(),
        cast_binary: cast_binary.display().to_string(),
        plan_options: VaultBridgeDepositPlanOptions {
            log_file: None,
            receipt_file: None,
            vault_address: Some(vault_address.clone()),
            token_address: Some(token_address.clone()),
            asset_id: asset.asset_id.clone(),
            policy_hash: route_hash.clone(),
            proposer: holder.clone(),
            finalizer: holder.clone(),
            claimer: holder.clone(),
            attestor: Some(holder.clone()),
            observer_confirmation_depth: Some(1),
            expires_at_height: 100,
            source_proof_kind: None,
            source_proof_hash: None,
            source_public_values_hash: None,
        },
        bundle_dir: root.join("deposit-relay"),
        overwrite: false,
    })
    .expect("build relay from live source receipt");
    assert!(relay.confirmation_depth >= route.minimum_confirmations);
    assert_eq!(relay.relay_bundle.plan.policy_hash, route_hash);
    assert_eq!(
        relay.relay_bundle.plan.evidence.route_binding,
        route_binding
    );
    assert_eq!(relay.relay_bundle.plan.evidence.amount_atoms, amount);

    let mut pftl_receipts = vec![route_receipt];
    let mut failed_claim_receipt = None;
    let mut failed_claim_conservation = None;
    pftl_receipts.push(execute_governed_bridge_asset(
        &genesis,
        &governance,
        &mut ledger,
        sign_governed_bridge_asset(
            &genesis,
            &holder_key,
            NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
                attestor: holder.clone(),
                domain: "anvil.local".to_string(),
                bond: 1,
            }),
        ),
        2,
        "attestor-register",
    ));
    for (height, sequence, label, transaction_kind, operation) in [
        (
            3_u64,
            2_u64,
            "deposit-propose",
            VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND,
            relay.relay_bundle.plan.propose_operation.clone(),
        ),
        (
            4,
            3,
            "deposit-attest",
            postfiat_types::VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND,
            relay
                .relay_bundle
                .plan
                .attest_operation
                .clone()
                .expect("deposit attestation"),
        ),
        (
            5,
            4,
            "deposit-finalize",
            VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
            relay.relay_bundle.plan.finalize_operation.clone(),
        ),
        (
            6,
            5,
            "deposit-claim",
            postfiat_types::VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
            relay.relay_bundle.plan.claim_operation.clone(),
        ),
    ] {
        pftl_receipts.push(execute_governed_bridge_asset(
            &genesis,
            &governance,
            &mut ledger,
            sign_governed_bridge_asset(
                &genesis,
                &holder_key,
                transaction_kind,
                sequence,
                operation,
            ),
            height,
            label,
        ));
        if height == 3 {
            persist_governed_bridge_state(
                &store,
                &governance,
                &ledger,
                height,
                pftl_receipts.len() as u64,
            );
            let observed = vault_bridge_conservation_audit(VaultBridgeConservationOptions {
                data_dir: data_dir.clone(),
                asset_id: asset.asset_id.clone(),
                source_rpc_url: rpc_url.clone(),
                cast_binary: cast_binary.clone(),
            })
            .expect("observed deposit conservation");
            assert_eq!(observed.uncredited_deposit_atoms, amount);
            assert!(observed.conserved);
        }
        if height == 5 {
            let mut wrong_claim = relay.relay_bundle.plan.claim_operation.clone();
            let AssetTransactionOperation::VaultBridgeDepositClaim(operation) = &mut wrong_claim
            else {
                panic!("relay claim operation has wrong kind");
            };
            operation.amount_atoms = operation
                .amount_atoms
                .checked_add(1)
                .expect("wrong-amount claim fixture");
            let wrong_transaction = sign_governed_bridge_asset(
                &genesis,
                &holder_key,
                postfiat_types::VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
                5,
                wrong_claim,
            );
            let wrong_batch = TransactionBatch::new_with_asset_transactions(
                "governed-bridge-roundtrip-wrong-amount-claim".to_string(),
                Vec::new(),
                Vec::new(),
                vec![wrong_transaction],
            );
            let mut failed_ledger = ledger.clone();
            let wrong_receipts = execute_transparent_batch(
                &genesis,
                &governance,
                &mut failed_ledger,
                &wrong_batch,
                6,
                AssetExecutionCompatibility::strict(),
            );
            assert_eq!(wrong_receipts.len(), 1);
            assert!(!wrong_receipts[0].accepted, "{wrong_receipts:?}");
            assert_ne!(wrong_receipts[0].code, "accepted");
            assert_eq!(failed_ledger.trustlines, ledger.trustlines);
            assert_eq!(
                failed_ledger.vault_bridge_deposits,
                ledger.vault_bridge_deposits
            );
            assert_eq!(
                failed_ledger.vault_bridge_bucket_states,
                ledger.vault_bridge_bucket_states
            );
            persist_governed_bridge_state(
                &store,
                &governance,
                &failed_ledger,
                6,
                pftl_receipts.len() as u64 + 1,
            );
            let failed_audit = vault_bridge_conservation_audit(VaultBridgeConservationOptions {
                data_dir: data_dir.clone(),
                asset_id: asset.asset_id.clone(),
                source_rpc_url: rpc_url.clone(),
                cast_binary: cast_binary.clone(),
            })
            .expect("wrong-amount claim conservation");
            assert!(failed_audit.conserved);
            assert_eq!(failed_audit.issued_supply_atoms, 0);
            assert_eq!(failed_audit.source_vault_atoms, amount);
            assert_eq!(failed_audit.uncredited_deposit_atoms, amount);
            failed_claim_receipt = wrong_receipts.into_iter().next();
            failed_claim_conservation = Some(failed_audit);
            persist_governed_bridge_state(
                &store,
                &governance,
                &ledger,
                height,
                pftl_receipts.len() as u64,
            );
        }
    }
    assert_eq!(
        ledger
            .trustline_for_account_asset(&holder, &asset.asset_id)
            .expect("holder bridge trustline")
            .balance,
        amount
    );
    persist_governed_bridge_state(&store, &governance, &ledger, 6, pftl_receipts.len() as u64);
    let claimed = vault_bridge_conservation_audit(VaultBridgeConservationOptions {
        data_dir: data_dir.clone(),
        asset_id: asset.asset_id.clone(),
        source_rpc_url: rpc_url.clone(),
        cast_binary: cast_binary.clone(),
    })
    .expect("claimed deposit conservation");
    assert_eq!(claimed.live_claim_atoms, amount);
    assert_eq!(claimed.source_vault_atoms, amount);

    let source_root =
        vault_bridge_source_root_for_asset(&ledger.vault_bridge_bucket_states, &asset.asset_id)
            .expect("bridge source root");
    let reserve_packet_hash = "93".repeat(48);
    let proof_profile = ledger.nav_assets[0].proof_profile.clone();
    pftl_receipts.push(execute_governed_bridge_asset(
        &genesis,
        &governance,
        &mut ledger,
        sign_governed_bridge_asset(
            &genesis,
            &issuer_key,
            NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
                issuer: issuer.clone(),
                submitter: issuer.clone(),
                asset_id: asset.asset_id.clone(),
                epoch: 1,
                nav_per_unit: VAULT_BRIDGE_UNIT,
                circulating_supply: amount,
                verified_net_assets: amount,
                proof_profile,
                source_root: source_root.clone(),
                attestor_root: "94".repeat(48),
                reserve_packet_hash: reserve_packet_hash.clone(),
                reserve_accounts: vec![relay.relay_bundle.plan.evidence.vault_id()],
                sp1_proof_bytes: Vec::new(),
                sp1_public_values: Vec::new(),
            }),
        ),
        7,
        "reserve-submit",
    ));
    pftl_receipts.push(execute_governed_bridge_asset(
        &genesis,
        &governance,
        &mut ledger,
        sign_governed_bridge_asset(
            &genesis,
            &holder_key,
            NAV_RESERVE_ATTEST_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
                attestor: holder.clone(),
                asset_id: asset.asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
                pass: true,
                observation_root: source_root,
            }),
        ),
        8,
        "reserve-attest",
    ));
    pftl_receipts.push(execute_governed_bridge_asset(
        &genesis,
        &governance,
        &mut ledger,
        sign_governed_bridge_asset(
            &genesis,
            &issuer_key,
            NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
                issuer: issuer.clone(),
                asset_id: asset.asset_id.clone(),
                epoch: 1,
                reserve_packet_hash: reserve_packet_hash.clone(),
            }),
        ),
        9,
        "reserve-finalize",
    ));
    persist_governed_bridge_state(&store, &governance, &ledger, 9, pftl_receipts.len() as u64);
    let burn_bundle = vault_bridge_burn_to_redeem_bundle(VaultBridgeBurnToRedeemBundleOptions {
        data_dir: data_dir.clone(),
        owner: holder.clone(),
        issuer: Some(issuer.clone()),
        asset_id: asset.asset_id.clone(),
        bucket_id: None,
        amount_atoms: amount,
        epoch: Some(1),
        reserve_packet_hash: Some(reserve_packet_hash),
        destination_ref: format!("evm-erc20:31337:{ANVIL_TEST_ADDRESS}"),
        bundle_dir: root.join("burn-to-redeem"),
        overwrite: false,
    })
    .expect("build governed burn-to-redeem operation");
    pftl_receipts.push(execute_governed_bridge_asset(
        &genesis,
        &governance,
        &mut ledger,
        sign_governed_bridge_asset(
            &genesis,
            &holder_key,
            VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND,
            7,
            burn_bundle.operation,
        ),
        10,
        "burn-to-redeem",
    ));
    persist_governed_bridge_state(&store, &governance, &ledger, 10, pftl_receipts.len() as u64);
    let burned = vault_bridge_conservation_audit(VaultBridgeConservationOptions {
        data_dir: data_dir.clone(),
        asset_id: asset.asset_id.clone(),
        source_rpc_url: rpc_url.clone(),
        cast_binary: cast_binary.clone(),
    })
    .expect("burned-unsettled conservation");
    assert_eq!(burned.burned_unsettled_atoms, amount);
    assert_eq!(burned.released_unsettled_atoms, 0);

    let redemption = ledger.vault_bridge_redemptions[0].clone();
    assert_eq!(redemption.withdrawal_packet.vault_address, vault_address);
    let unsigned_plan = vault_bridge_withdrawal_plan(VaultBridgeWithdrawalPlanOptions {
        data_dir: data_dir.clone(),
        asset_id: asset.asset_id.clone(),
        redemption_id: redemption.redemption_id.clone(),
        pftl_finalized_height: Some(10),
        evm_chain_id: Some(31_337),
        verifier_address: Some(verifier_address.clone()),
        signatures_file: None,
    })
    .expect("build source withdrawal plan");
    let proof_digest = unsigned_plan
        .verifier_proof_digest_to_sign
        .as_deref()
        .expect("withdrawal proof digest");
    let signature = run_external_text(
        &cast_binary,
        &[
            "wallet".to_string(),
            "sign".to_string(),
            "--no-hash".to_string(),
            "--private-key".to_string(),
            ANVIL_TEST_PRIVATE_KEY.to_string(),
            proof_digest.to_string(),
        ],
        "withdrawal proof signature",
    );
    let signatures_file = root.join("withdrawal-signatures.json");
    std::fs::write(
        &signatures_file,
        serde_json::to_vec_pretty(&vec![signature]).expect("signature JSON"),
    )
    .expect("write withdrawal signatures");
    let withdrawal_plan = vault_bridge_withdrawal_plan(VaultBridgeWithdrawalPlanOptions {
        data_dir: data_dir.clone(),
        asset_id: asset.asset_id.clone(),
        redemption_id: redemption.redemption_id.clone(),
        pftl_finalized_height: Some(10),
        evm_chain_id: Some(31_337),
        verifier_address: Some(verifier_address.clone()),
        signatures_file: Some(signatures_file),
    })
    .expect("build signed source withdrawal plan");
    let proof_submit_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &verifier_address,
        &withdrawal_plan.verifier_submit_proof_signature,
        &withdrawal_plan.verifier_submit_proof_cast_args,
        "PFTL withdrawal proof submit",
    );
    cast_rpc(&cast_binary, &rpc_url, "evm_increaseTime", &["0x2"]);
    cast_rpc(&cast_binary, &rpc_url, "evm_mine", &[]);
    let proof_finalize_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &verifier_address,
        "finalizeProof(bytes32)",
        std::slice::from_ref(&withdrawal_plan.verifier_pending_proof_id),
        "PFTL withdrawal proof finalization",
    );
    let withdrawal_submit_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &vault_address,
        &withdrawal_plan.vault_submit_withdrawal_signature,
        &withdrawal_plan.vault_submit_withdrawal_cast_args,
        "source withdrawal submit",
    );
    cast_rpc(&cast_binary, &rpc_url, "evm_increaseTime", &["0x2"]);
    cast_rpc(&cast_binary, &rpc_url, "evm_mine", &[]);
    let withdrawal_finalize_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &vault_address,
        "finalizeWithdrawal(bytes32)",
        std::slice::from_ref(&withdrawal_plan.vault_pending_withdrawal_id),
        "source withdrawal finalization",
    );
    let withdrawal_claim_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &vault_address,
        "claimWithdrawal(bytes32)",
        std::slice::from_ref(&withdrawal_plan.vault_pending_withdrawal_id),
        "source withdrawal claim",
    );
    cast_rpc(&cast_binary, &rpc_url, "evm_mine", &[]);
    let released = vault_bridge_conservation_audit(VaultBridgeConservationOptions {
        data_dir: data_dir.clone(),
        asset_id: asset.asset_id.clone(),
        source_rpc_url: rpc_url.clone(),
        cast_binary: cast_binary.clone(),
    })
    .expect("released-unsettled conservation");
    assert_eq!(released.source_vault_atoms, 0);
    assert_eq!(released.burned_unsettled_atoms, amount);
    assert_eq!(released.released_unsettled_atoms, amount);

    let observation = VaultBridgeWithdrawalExecutionObservation::success_for_packet(
        &redemption.withdrawal_packet,
        redemption.withdrawal_packet_hash.clone(),
        receipt_hash_field(&withdrawal_claim_receipt, "transactionHash"),
        receipt_hash_field(&withdrawal_claim_receipt, "blockHash"),
        receipt_log_index(&withdrawal_claim_receipt),
        2,
    );
    let observation_root = vault_bridge_withdrawal_execution_observation_root(&observation)
        .expect("source withdrawal observation root");
    let observation_signature =
        ml_dsa_65_sign(&holder_key.private_key, &observation.signing_bytes())
            .expect("sign source withdrawal observation");
    let settlement_operation =
        AssetTransactionOperation::VaultBridgeRedeemSettle(VaultBridgeRedeemSettleOperation {
            issuer_or_redemption_account: issuer.clone(),
            asset_id: asset.asset_id.clone(),
            redemption_id: redemption.redemption_id.clone(),
            settlement_receipt_hash: observation_root.clone(),
            settled_atoms: amount,
            withdrawal_observations: vec![VaultBridgeWithdrawalExecutionAttestation {
                attestor: holder,
                observation_root,
                signature_hex: bytes_to_hex(&observation_signature),
                observation,
            }],
        });
    pftl_receipts.push(execute_governed_bridge_asset(
        &genesis,
        &governance,
        &mut ledger,
        sign_governed_bridge_asset(
            &genesis,
            &issuer_key,
            VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND,
            3,
            settlement_operation,
        ),
        11,
        "redeem-settle",
    ));
    assert_eq!(
        ledger.vault_bridge_redemptions[0].state,
        VAULT_BRIDGE_REDEMPTION_STATE_SETTLED
    );
    persist_governed_bridge_state(&store, &governance, &ledger, 11, pftl_receipts.len() as u64);
    let final_conservation = vault_bridge_conservation_audit(VaultBridgeConservationOptions {
        data_dir,
        asset_id: asset.asset_id.clone(),
        source_rpc_url: rpc_url.clone(),
        cast_binary: cast_binary.clone(),
    })
    .expect("final governed roundtrip conservation");
    assert!(final_conservation.conserved);
    assert_eq!(final_conservation.source_vault_atoms, 0);
    assert_eq!(final_conservation.live_claim_atoms, 0);
    assert_eq!(final_conservation.uncredited_deposit_atoms, 0);
    assert_eq!(final_conservation.burned_unsettled_atoms, 0);
    assert_eq!(final_conservation.released_unsettled_atoms, 0);
    let vault_balance = cast_call(
        &cast_binary,
        &rpc_url,
        &token_address,
        "balanceOf(address)(uint256)",
        &[&vault_address],
    );
    let recipient_balance = cast_call(
        &cast_binary,
        &rpc_url,
        &token_address,
        "balanceOf(address)(uint256)",
        &[ANVIL_TEST_ADDRESS],
    );
    assert_eq!(
        vault_balance
            .split_whitespace()
            .next()
            .expect("vault balance")
            .parse::<u64>()
            .expect("vault balance number"),
        0
    );
    assert_eq!(
        recipient_balance
            .split_whitespace()
            .next()
            .expect("recipient balance")
            .parse::<u64>()
            .expect("recipient balance number"),
        amount
    );
    assert!(pftl_receipts.iter().all(|receipt| receipt.accepted));

    if let Some(report_dir) = std::env::var_os("POSTFIAT_BRIDGE_ROUNDTRIP_REPORT_DIR") {
        let report_dir = PathBuf::from(report_dir);
        std::fs::create_dir_all(&report_dir).expect("create acceptance report directory");
        let evm_receipts = [
            ("mint", mint_receipt),
            ("approve", approve_receipt),
            ("deposit", source_deposit_receipt),
            ("proof_submit", proof_submit_receipt),
            ("proof_finalize", proof_finalize_receipt),
            ("withdrawal_submit", withdrawal_submit_receipt),
            ("withdrawal_finalize", withdrawal_finalize_receipt),
            ("withdrawal_claim", withdrawal_claim_receipt),
        ]
        .into_iter()
        .map(|(label, receipt)| {
            serde_json::json!({
                "label": label,
                "status": receipt.get("status"),
                "transaction_hash": receipt.get("transactionHash"),
                "block_hash": receipt.get("blockHash"),
                "block_number": receipt.get("blockNumber"),
            })
        })
        .collect::<Vec<_>>();
        let acceptance = serde_json::json!({
            "schema": "postfiat-p0-governed-vault-bridge-anvil-roundtrip-v1",
            "source_chain": {"chain_id": 31337, "rpc_scope": "isolated-loopback-anvil"},
            "pftl_chain_id": genesis.chain_id,
            "asset_id": asset.asset_id,
            "route_profile_hash": route_hash,
            "route_profile": route,
            "contracts": {
                "token": token_address,
                "withdrawal_verifier": verifier_address,
                "vault": vault_address,
                "deploy_transactions": [token_deploy_tx, verifier_deploy_tx, vault_deploy_tx],
            },
            "deposit_transaction": deposit_tx_hash,
            "deposit_confirmation_depth": relay.confirmation_depth,
            "pftl_receipts": pftl_receipts,
            "evm_receipts": evm_receipts,
            "conservation": {
                "after_wrong_amount_claim": failed_claim_conservation,
                "after_claim": claimed,
                "after_burn": burned,
                "after_source_release": released,
                "final": final_conservation,
            },
            "rejected_failure_receipt": failed_claim_receipt,
            "terminal_balances": {
                "vault_atoms": vault_balance,
                "recipient_atoms": recipient_balance,
            },
            "private_key_material_recorded": false,
            "accepted": true,
        });
        std::fs::write(
            report_dir.join("ACCEPTANCE.json"),
            serde_json::to_vec_pretty(&acceptance).expect("acceptance report JSON"),
        )
        .expect("write acceptance report");
    }

    std::fs::remove_dir_all(root).expect("remove isolated roundtrip state");
}

/// Deploys the production mint controller and production threshold settlement
/// verifier to isolated Anvil, binds a real accepted local-PFTL receipt, and
/// proves one exact release plus fail-closed pre-certification/replay behavior.
#[test]
#[ignore = "requires local Foundry binaries and starts an isolated Anvil"]
fn mint_settlement_real_anvil_release_matches_accepted_pftl_backing() {
    use sha2::{Digest as _, Sha256, Sha384};

    let anvil_binary = foundry_binary("anvil");
    let cast_binary = foundry_binary("cast");
    let forge_binary = foundry_binary("forge");
    let anvil = start_governed_bridge_anvil(&anvil_binary, &cast_binary);
    let rpc_url = anvil.rpc_url.clone();
    let contracts_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ethereum-contracts");
    let root = std::env::temp_dir().join(format!(
        "postfiat-p0-mint-settlement-anvil-{}",
        std::process::id()
    ));
    let data_dir = root.join("pftl");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("create mint-settlement integration root");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-p0-supply-controlled".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize isolated PFTL backing ledger");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("PFTL backing genesis");
    let faucet = read_transfer_key_file(&data_dir, None).expect("PFTL backing key");
    let ledger_before = store.read_ledger().expect("PFTL ledger before backing");
    let amount_atoms = ACCOUNT_RESERVE + 100;
    let backing_recipient = "pfp0supplycontrolledbacking00000000000001".to_string();
    let backing_transfer = build_signed_transfer_for_key(
        &genesis,
        &ledger_before,
        &faucet,
        backing_recipient.clone(),
        amount_atoms,
        1,
    )
    .expect("build exact backing transfer");
    let backing_batch = build_transaction_batch(
        &mempool_batch_domain(&genesis),
        vec![backing_transfer],
    )
    .expect("build exact backing batch")
    .batch;
    let backing_batch_file = root.join("pftl-backing-batch.json");
    write_batch_file(&backing_batch_file, &backing_batch).expect("write backing batch");
    let backing_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: backing_batch_file,
        certificate_file: None,
    })
    .expect("apply exact PFTL backing transfer");
    assert_eq!(backing_receipts.len(), 1);
    let backing_receipt = &backing_receipts[0];
    assert!(backing_receipt.accepted, "{backing_receipt:?}");
    assert_eq!(backing_receipt.code, "accepted");
    let ledger_after = store.read_ledger().expect("PFTL ledger after backing");
    assert_eq!(
        ledger_after
            .account(&backing_recipient)
            .expect("backing recipient account")
            .balance,
        amount_atoms,
        "PFTL backing delta must equal the EVM release amount"
    );
    let tip = store.read_chain_tip().expect("PFTL finalized backing tip");
    assert_eq!(tip.height, 1);
    assert_eq!(tip.state_root.len(), 96);
    let receipt_hash = bytes_to_hex(&Sha384::digest(
        serde_json::to_vec(backing_receipt).expect("canonical backing receipt JSON"),
    ));
    let route_digest = bytes_to_hex(&Sha384::digest(
        b"postfiat.p0.supply.controlled-route.v1",
    ));
    let pftl_chain_id_hash = bytes_to_hex(&Sha256::digest(genesis.chain_id.as_bytes()));
    let pftl_genesis_commitment = bytes_to_hex(&Sha256::digest(
        hex_to_bytes(&genesis_hash(&genesis)).expect("PFTL genesis hash bytes"),
    ));

    let signer_keys = ["b101", "b102", "b103", "b104"];
    let mut signer_pairs = signer_keys
        .iter()
        .map(|key| {
            let private_key = format!("0x{:0>64}", key);
            let address = run_external_text(
                &cast_binary,
                &["wallet".to_string(), "address".to_string(), private_key.clone()],
                "derive controlled settlement signer",
            )
            .to_ascii_lowercase();
            (address, private_key)
        })
        .collect::<Vec<_>>();
    signer_pairs.sort_by(|left, right| left.0.cmp(&right.0));
    let signer_array = format!(
        "[{}]",
        signer_pairs
            .iter()
            .map(|(address, _)| address.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let (deployment_address, deployment_tx) = deploy_contract(
        &forge_binary,
        &contracts_root,
        &rpc_url,
        "test/P0SupplyControlledDeployment.t.sol:P0SupplyControlledDeployment",
        &[
            format!("0x{pftl_chain_id_hash}"),
            format!("0x{pftl_genesis_commitment}"),
            genesis.protocol_version.to_string(),
            "1".to_string(),
            amount_atoms.to_string(),
            tip.height.to_string(),
            format!("0x{}", tip.state_root),
            format!("0x{receipt_hash}"),
            format!("0x{route_digest}"),
            signer_array,
        ],
    );
    let escrow_id = cast_call(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "requestMint()(bytes32)",
        &[],
    );
    let request_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "requestMint()",
        &[],
        "request isolated settlement-backed mint",
    );
    let uncertified_proof = format!("0x{}", "11".repeat(32));
    let _ = cast_call_failure(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "releaseMint(bytes32,bytes32)",
        &[&escrow_id, &uncertified_proof],
        "uncertified mint release",
    );

    let certificate_digest = cast_call(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "certificateDigest(bytes32)(bytes32)",
        &[&escrow_id],
    );
    let signatures = signer_pairs
        .iter()
        .take(3)
        .map(|(_, private_key)| {
            run_external_text(
                &cast_binary,
                &[
                    "wallet".to_string(),
                    "sign".to_string(),
                    "--no-hash".to_string(),
                    certificate_digest.clone(),
                    "--private-key".to_string(),
                    private_key.clone(),
                ],
                "sign controlled settlement certificate",
            )
        })
        .collect::<Vec<_>>();
    let signature_array = format!("[{}]", signatures.join(","));
    let proof_hash = cast_call(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "settlementId(bytes32)(bytes32)",
        &[&escrow_id],
    );
    let certificate_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "submitCertificate(bytes32,bytes[])",
        &[escrow_id.clone(), signature_array.clone()],
        "submit exact threshold settlement certificate",
    );
    let release_receipt = cast_send_receipt(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "releaseMint(bytes32,bytes32)",
        &[escrow_id.clone(), proof_hash.clone()],
        "release exact settlement-backed mint",
    );
    let audit = cast_call(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "conservationAudit()(uint256,uint256,uint256,uint256,uint256,bool)",
        &[],
    );
    let audit_fields = audit.split_whitespace().collect::<Vec<_>>();
    assert_eq!(audit_fields.len(), 6, "unexpected conservation audit: {audit}");
    assert_eq!(audit_fields[0].parse::<u64>(), Ok(amount_atoms));
    assert_eq!(audit_fields[1].parse::<u64>(), Ok(amount_atoms));
    assert_eq!(audit_fields[2], "0");
    assert_eq!(audit_fields[3].parse::<u64>(), Ok(amount_atoms));
    assert_eq!(audit_fields[4], "0");
    assert_eq!(audit_fields[5], "true");

    let _ = cast_call_failure(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "submitCertificate(bytes32,bytes[])",
        &[&escrow_id, &signature_array],
        "settlement certificate replay",
    );
    let _ = cast_call_failure(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "releaseMint(bytes32,bytes32)",
        &[&escrow_id, &proof_hash],
        "mint release replay",
    );

    let token_address = cast_call(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "token()(address)",
        &[],
    );
    let controller_address = cast_call(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "controller()(address)",
        &[],
    );
    let verifier_address = cast_call(
        &cast_binary,
        &rpc_url,
        &deployment_address,
        "verifier()(address)",
        &[],
    );
    if let Some(report_dir) = std::env::var_os("POSTFIAT_MINT_SETTLEMENT_REPORT_DIR") {
        let report_dir = PathBuf::from(report_dir);
        std::fs::create_dir_all(&report_dir).expect("create mint-settlement report directory");
        let acceptance = serde_json::json!({
            "schema": "postfiat-p0-mint-settlement-anvil-release-v1",
            "accepted": true,
            "source_chain": {"kind": "isolated-local-pftl", "chain_id": genesis.chain_id},
            "destination_chain": {"kind": "isolated-loopback-anvil", "chain_id": 31337},
            "pftl_backing": {
                "height": tip.height,
                "state_root": tip.state_root,
                "receipt_tx_id": backing_receipt.tx_id,
                "receipt_hash": receipt_hash,
                "receipt_code": backing_receipt.code,
                "recipient": backing_recipient,
                "exact_delta_atoms": amount_atoms,
            },
            "deployment": {
                "orchestrator": deployment_address,
                "token": token_address,
                "controller": controller_address,
                "production_threshold_verifier": verifier_address,
                "transaction_hash": deployment_tx,
                "controller_runtime_code_hash": deployed_runtime_hash(
                    &cast_binary, &rpc_url, &controller_address
                ),
                "verifier_runtime_code_hash": deployed_runtime_hash(
                    &cast_binary, &rpc_url, &verifier_address
                ),
                "committee_size": 4,
                "threshold": 3,
            },
            "transactions": {
                "request": request_receipt.get("transactionHash"),
                "certificate": certificate_receipt.get("transactionHash"),
                "release": release_receipt.get("transactionHash"),
            },
            "binding": {
                "escrow_id": escrow_id,
                "certificate_digest": certificate_digest,
                "proof_hash": proof_hash,
                "receipt_code": "accepted",
            },
            "conservation": {
                "certified_backing_atoms": amount_atoms,
                "released_supply_atoms": amount_atoms,
                "controller_escrow_atoms": 0,
                "beneficiary_atoms": amount_atoms,
                "unresolved_escrows": 0,
                "conserved": true,
            },
            "negative_boundaries": [
                "release before certification rejected without mutation",
                "certificate replay rejected",
                "release replay rejected"
            ],
            "private_key_material_recorded": false,
        });
        std::fs::write(
            report_dir.join("ACCEPTANCE.json"),
            serde_json::to_vec_pretty(&acceptance).expect("mint-settlement acceptance JSON"),
        )
        .expect("write mint-settlement acceptance report");
    }

    std::fs::remove_dir_all(root).expect("remove isolated mint-settlement state");
}
