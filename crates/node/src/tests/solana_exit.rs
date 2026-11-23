use super::*;
use crate::consensus_artifacts::{
    governance_amendment_authorization_signing_bytes, verify_live_signed_governance_batch,
    GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2,
};
use postfiat_crypto_provider::ml_dsa_65_sign_with_context_seed;
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

/// The existing payout rehearsal consumes this public fork and its witness.
#[test]
#[ignore = "requires height-1753 archive and SOL_EXIT_EVIDENCE"]
fn copied_solana_exit_governance_and_burn() {
    let out = PathBuf::from(std::env::var_os("SOL_EXIT_EVIDENCE").unwrap());
    certified_fork(Some(&out));
}

#[test]
#[ignore = "requires the isolated certified Solana exit fork"]
fn export_local_solana_exit_chain() {
    let dir = PathBuf::from(std::env::var_os("SOL_EXIT_EVIDENCE").unwrap());
    let witness: postfiat_types::PfUsdcEgressProofWitnessV1 =
        serde_json::from_slice(&fs::read(dir.join("local-witness.json")).unwrap()).unwrap();
    assert_eq!(
        exported_witness(
            &dir.join("local-store"),
            &witness.withdrawal_packet.withdrawal_id
        ),
        witness
    );
}

fn funded_holder(ledger: &mut LedgerState) -> (String, String, String) {
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
    ledger
        .accounts
        .iter_mut()
        .find(|a| a.balance > 2_000_000)
        .unwrap()
        .balance -= 1_000_000;
    ledger.accounts.push(postfiat_types::Account::new(
        &owner,
        1_000_000,
        Some(public.clone()),
    ));
    (owner, public, private)
}

fn funded_burn(g: &Genesis, ledger: &mut LedgerState) -> SignedAssetTransaction {
    let (owner, public, private) = funded_holder(ledger);
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
    sign(op)
}

const H: u64 = 1760;
const OPEN: &str = "solana_exit_r8_open";

fn archived_parent() -> (Genesis, GovernanceState, LedgerState) {
    let store = NodeStore::new("/var/lib/postfiat/validator-3");
    (
        store.read_genesis().unwrap(),
        store.read_governance().unwrap(),
        store.read_ledger().unwrap(),
    )
}

fn descriptor_batch() -> GovernanceActionBatch {
    serde_json::from_str(include_str!(
        "../../tests/fixtures/solana-exit/governance-batch.json"
    ))
    .unwrap()
}

fn scheduled_parent(open: bool) -> (Genesis, GovernanceState, LedgerState) {
    let (g, mut gov, mut ledger) = archived_parent();
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let batch = committee.registration(&g, &gov, H);
    let registered = crate::execution_actions::execute_governance_batch(
        &mut gov,
        Some(&mut ledger),
        Some(&g),
        &batch,
        H,
    );
    assert!(registered[0].accepted, "{:?}", registered[0]);
    if open {
        let batch = committee.decision(&g, &gov, OPEN, H + 1);
        verify_live_signed_governance_batch(&g, &gov, &committee.registry, &batch, H + 1).unwrap();
        assert!(
            crate::execution_actions::execute_governance_batch(
                &mut gov,
                Some(&mut ledger),
                Some(&g),
                &batch,
                H + 1
            )[0]
            .accepted
        );
    }
    (g, gov, ledger)
}

#[test]
#[ignore = "requires the copied WAN ledger at height 1753"]
fn direct_client_explains_closed_burn_before_release() {
    let (g, _, mut local) = archived_parent();
    let tx = funded_burn(&g, &mut local);
    let denied =
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: "/var/lib/postfiat/validator-3".into(),
            signed_asset_transaction_json: serde_json::to_string(&tx).unwrap(),
        })
        .unwrap_err();
    assert!(
        denied.to_string().contains("solana_exit_burns_closed"),
        "{denied}"
    );
}

fn block_burn(
    g: &Genesis,
    gov: &GovernanceState,
    ledger: &mut LedgerState,
    tx: &SignedAssetTransaction,
    h: u64,
) -> Receipt {
    let compat =
        crate::execution_actions::asset_execution_compatibility_for_genesis_and_governance(g, gov);
    let batch = TransactionBatch::new_with_asset_transactions(
        String::new(),
        vec![],
        vec![],
        vec![tx.clone()],
    );
    crate::execution_actions::execute_transparent_batch(g, gov, ledger, &batch, h, compat).remove(0)
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn solana_exit_registration_leaves_burns_closed() {
    let (g, gov, mut ledger) = scheduled_parent(false);
    let tx = funded_burn(&g, &mut ledger);
    let lookup = |id: &str| {
        crate::market_bridge::vault_bridge_route_at_height(&g, &gov, &ledger, &[], H, id)
    };
    let family = lookup(solana_exit::FAMILY).unwrap();
    assert_eq!(family, lookup("541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7").unwrap());
    let expected_eth: VaultBridgeRouteProfileV1 = serde_json::from_str(include_str!(
        "../../tests/fixtures/solana-exit/ethereum-profile.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&family.profile).unwrap(),
        serde_json::to_vec(&expected_eth).unwrap()
    );
    for b in &ledger.vault_bridge_bucket_states {
        if b.source_domain.starts_with("spl:") && b.bucket_id != solana_exit::BUCKET {
            let earlier =
                postfiat_types::solana_source_series_id(&g.chain_id, &b.asset_id, &b.bucket_id);
            assert!(lookup(&earlier).is_err());
        }
    }
    let report = lookup(SERIES).unwrap();
    assert!(report.active);
    assert_eq!(report.burns_open, Some(false));
    let receipt = block_burn(&g, &gov, &mut ledger, &tx, H + 1);
    assert_eq!(receipt.code, "solana_exit_burns_closed");
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn solana_exit_requires_active_full_backing() {
    let (g, gov, mut ledger) = scheduled_parent(true);
    let tx = funded_burn(&g, &mut ledger);
    let bucket = ledger
        .vault_bridge_bucket_states
        .iter_mut()
        .find(|b| b.bucket_id == solana_exit::BUCKET)
        .unwrap();
    bucket.status = postfiat_types::VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED.into();
    bucket.counted_value_atoms = bucket.allocated_atoms().unwrap() / 2;
    bucket.impairment_factor_bps = 5000;
    bucket.validate().unwrap();
    let receipt = block_burn(&g, &gov, &mut ledger, &tx, H + 2);
    assert_eq!(receipt.code, "solana_exit_requires_full_backing");
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn solana_exit_debits_series_with_pooled_switch_off() {
    let (g, mut gov, mut ledger) = scheduled_parent(true);
    gov.amendments
        .retain(|a| a.kind != GOVERNANCE_KIND_PFUSDC_SOURCE_SERIES_ACTIVATION_HEIGHT);
    let tx = funded_burn(&g, &mut ledger);
    let balance = |l: &LedgerState| {
        l.trustlines
            .iter()
            .find(|t| t.account == tx.unsigned.source && t.asset_id == SERIES)
            .unwrap()
            .balance
    };
    let before = balance(&ledger);
    let receipt = block_burn(&g, &gov, &mut ledger, &tx, H + 2);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(balance(&ledger), before - 20_000);
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn solana_exit_route_reserves_queued_burn_once() {
    let (g, gov, mut ledger) = scheduled_parent(true);
    let tx = funded_burn(&g, &mut ledger);
    let row = |l: &LedgerState| {
        l.vault_bridge_route_backing(solana_exit::FAMILY)
            .unwrap()
            .into_iter()
            .find(|r| r.route_id == "solana-mainnet-usdc-v1")
            .unwrap()
    };
    let before = row(&ledger);
    let receipt = block_burn(&g, &gov, &mut ledger, &tx, H + 2);
    assert!(receipt.accepted, "{receipt:?}");
    let after = row(&ledger);
    assert_eq!(
        after.withdrawals_reserved,
        before.withdrawals_reserved + 20_000
    );
    assert_eq!(
        after.available_withdrawal_liquidity().unwrap() + 20_000,
        before.available_withdrawal_liquidity().unwrap()
    );
}

#[test]
#[ignore = "copied WAN ledger"]
fn ethereum_series_selects_bucket_policy_across_epochs() {
    let (g, gov, ledger) = archived_parent();
    let lookup = |id: &str| {
        crate::market_bridge::vault_bridge_route_at_height(&g, &gov, &ledger, &[], 1753, id)
            .unwrap()
    };
    let id = ledger
        .asset_definitions
        .iter()
        .find(|a| a.asset_id.starts_with("2bae082a"))
        .unwrap();
    let bucket = ledger.vault_bridge_bucket(&id.source_bucket_id).unwrap();
    let series = lookup(&id.asset_id);
    assert_eq!(series.profile.route_epoch, 10);
    assert_eq!(series.profile_hash, bucket.policy_hash);
    assert_eq!(
        serde_json::to_value(series).unwrap()["nav_profile_policy_hash"],
        bucket.policy_hash
    );
    assert_eq!(lookup(solana_exit::FAMILY).profile_hash, ETH_HASH);
}

struct LocalCommittee {
    registry: ValidatorRegistry,
    keys: Vec<postfiat_crypto_provider::MlDsa65KeyPair>,
}

impl LocalCommittee {
    fn new(names: &[String]) -> Self {
        // Reproduce the same locally signed registration across release builds.
        let keys: Vec<_> = (0..names.len())
            .map(|i| postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[i as u8; 32]))
            .collect();
        let registry = ValidatorRegistry {
            validators: names
                .iter()
                .zip(&keys)
                .map(|(name, k)| ValidatorRegistryRecord {
                    node_id: name.clone(),
                    algorithm_id: ML_DSA_65_ALGORITHM.into(),
                    public_key_hex: bytes_to_hex(&k.public_key),
                })
                .collect(),
        };
        Self { registry, keys }
    }

    fn sign(&self, gov: &GovernanceState, a: &mut GovernanceAmendment, h: u64) {
        let root = validator_registry_root(&self.registry, &a.validators).unwrap();
        let epoch = gov
            .validator_registry_updates
            .iter()
            .filter(|u| u.activation_height < h)
            .count() as u64;
        a.signed_authorizations = a
            .votes
            .iter()
            .zip(&self.keys)
            .map(|(v, k)| {
                let mut auth = SignedGovernanceAuthorizationV2 {
                    schema: SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2.into(),
                    validator: v.validator.clone(),
                    vote_id: v.vote_id.clone(),
                    old_registry_root: root.clone(),
                    committee_epoch: epoch,
                    proposal_slot: h,
                    expires_at_height: h,
                    algorithm_id: ML_DSA_65_ALGORITHM.into(),
                    signature_hex: String::new(),
                };
                let message = governance_amendment_authorization_signing_bytes(a, &auth).unwrap();
                auth.signature_hex = bytes_to_hex(
                    &ml_dsa_65_sign_with_context_seed(
                        &k.private_key,
                        &message,
                        GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2,
                        &[0; 32],
                    )
                    .unwrap(),
                );
                auth
            })
            .collect();
    }

    fn registration(&self, g: &Genesis, gov: &GovernanceState, h: u64) -> GovernanceActionBatch {
        let mut act = descriptor_batch()
            .vault_bridge_route_profile_activations
            .remove(0);
        self.sign(gov, &mut act.amendment, h);
        build_governance_action_batch_with_vault_bridge_route_profile_activation(g, act).unwrap()
    }

    fn decision(
        &self,
        g: &Genesis,
        gov: &GovernanceState,
        kind: &str,
        h: u64,
    ) -> GovernanceActionBatch {
        let names = active_validator_ids(gov).unwrap();
        let mut a = ratify_governance_amendment_with_lifecycle(
            &cobalt_domain(g),
            &EssentialSubsetConfig::all_of(names.clone()),
            kind,
            1,
            names,
            GovernanceAmendmentLifecycle {
                activation_height: h,
                veto_until_height: 0,
                paused: false,
            },
        )
        .unwrap();
        self.sign(gov, &mut a, h);
        build_governance_action_batch(g, vec![a], vec![]).unwrap()
    }
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn signed_solana_exit_registration_checks_release_slot() {
    let (g, mut gov, mut ledger) = archived_parent();
    // The deterministic fork committee signs for its own registry.
    gov.validator_registry_updates.clear();
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let before = committee.registration(&g, &gov, H - 1);
    verify_live_signed_governance_batch(&g, &gov, &committee.registry, &before, H - 1)
        .expect("the legacy block reaches execution before release");
    let mut state = gov.clone();
    let original = ledger.clone();
    let receipts = crate::execution_actions::execute_governance_batch(
        &mut state,
        Some(&mut ledger),
        Some(&g),
        &before,
        H - 1,
    );
    assert_eq!(receipts.len(), 1);
    assert!(!receipts[0].accepted);
    assert!(
        receipts[0].message.contains("advance monotonically"),
        "{}",
        receipts[0].message
    );
    assert!(ledger == original && state == gov);
    let source = NodeStore::new("/var/lib/postfiat/validator-3");
    let dir = unique_test_dir("early-registration");
    let store = NodeStore::new(&dir);
    store.init(&g, &source.read_node_state().unwrap()).unwrap();
    store
        .write_storage_backend_mode(postfiat_storage::StorageBackendMode::LegacyJsonl)
        .unwrap();
    let mut ordered = source.read_ordered_batches().unwrap();
    for height in (ordered.len() as u64 + 1)..=H - 2 {
        ordered.push(postfiat_crypto_provider::hash_hex(
            "local history",
            &height.to_be_bytes(),
        ));
    }
    store.write_ordered_batches(&ordered).unwrap();
    store.write_governance(&gov).unwrap();
    store.write_ledger(&ledger).unwrap();
    store
        .write_shielded(&source.read_shielded().unwrap())
        .unwrap();
    store.write_bridge(&source.read_bridge().unwrap()).unwrap();
    let mut tip = source.read_chain_tip().unwrap();
    tip.height = H - 2;
    tip.ordered_batch_count = tip.height;
    store.write_chain_tip(&tip).unwrap();
    crate::storage_commit::write_validator_registry_file(
        &dir.join(VALIDATOR_REGISTRY_FILE),
        &committee.registry,
    )
    .unwrap();
    let path = dir.join("early.json");
    write_governance_action_batch_file(&path, &before).unwrap();
    let (proposal, _) = crate::mempool_proposals::build_ordered_batch_proposal_with_timings(
        &store,
        BATCH_KIND_GOVERNANCE,
        &path,
        Some(0),
        None,
    )
    .unwrap();
    assert_eq!(proposal.block_height, H - 1);
    assert_eq!(proposal.receipt_ids, vec![receipts[0].tx_id.clone()]);
    assert!(crate::check_operator_batch_submission(&proposal, &path).is_err());
    println!(
        "H-1 comparison {}",
        serde_json::json!({
            "proposal_digest": postfiat_crypto_provider::hash_hex(
                "test proposal", &serde_json::to_vec(&proposal).unwrap()),
            "height": proposal.block_height, "state_root": proposal.state_root,
            "payload_hash": proposal.payload_hash, "receipts": receipts,
        })
    );
    fs::remove_dir_all(dir).unwrap();
}

fn bucket_amounts(l: &LedgerState) -> (u64, u64, u64, u64) {
    let b = l.vault_bridge_bucket(solana_exit::BUCKET).unwrap();
    let supply: u64 = l
        .vault_bridge_allocations
        .iter()
        .filter(|a| {
            a.bucket_id == b.bucket_id
                && a.purpose == postfiat_types::VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY
        })
        .map(|a| a.amount_atoms - a.released_atoms)
        .sum();
    (
        b.counted_value_atoms,
        b.redemption_queue_atoms,
        supply,
        b.outstanding_vault_bridge_atoms,
    )
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn signed_solana_exit_settlement_conserves_backing_and_supply() {
    let (g, mut gov, mut ledger) = scheduled_parent(true);
    let tx = funded_burn(&g, &mut ledger);
    let receipt = block_burn(&g, &gov, &mut ledger, &tx, H + 2);
    assert!(receipt.accepted, "{receipt:?}");
    let red = ledger.vault_bridge_redemptions.last().unwrap();
    let kind = format!(
        "solana_exit_r8_paid:{}:12345:0:{}",
        red.withdrawal_packet_hash,
        "ab".repeat(64)
    );
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let batch = committee.decision(&g, &gov, &kind, H + 3);
    verify_live_signed_governance_batch(&g, &gov, &committee.registry, &batch, H + 3).unwrap();
    let (c, q, s, o) = bucket_amounts(&ledger);
    let mut short = ledger.clone();
    short
        .vault_bridge_allocations
        .retain(|a| a.bucket_id != solana_exit::BUCKET);
    let original = short.clone();
    let receipts = crate::execution_actions::execute_governance_batch(
        &mut gov.clone(),
        Some(&mut short),
        Some(&g),
        &batch,
        H + 3,
    );
    assert!(!receipts[0].accepted);
    assert_eq!(short, original);
    let receipts = crate::execution_actions::execute_governance_batch(
        &mut gov,
        Some(&mut ledger),
        Some(&g),
        &batch,
        H + 3,
    );
    assert!(receipts[0].accepted, "{:?}", receipts[0]);
    assert_eq!(
        bucket_amounts(&ledger),
        (c - 20_000, q - 20_000, s - 20_000, o)
    );
    ledger.validate_asset_state(&g.chain_id).unwrap();
    let settled = ledger.vault_bridge_redemptions.last().unwrap();
    assert_eq!(settled.settled_atoms, 20_000);
    assert_eq!(
        settled.state,
        postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_SETTLED
    );
    assert_eq!(
        settled.settlement_receipt_hash,
        batch.amendments[0].amendment_id
    );
    let route = ledger
        .vault_bridge_route_backing(solana_exit::FAMILY)
        .unwrap()
        .into_iter()
        .find(|r| r.route_id == "solana-mainnet-usdc-v1")
        .unwrap();
    assert_eq!(
        (route.withdrawals_paid, route.withdrawals_reserved),
        (20_000, 20_000)
    );
    assert_eq!(
        route.available_withdrawal_liquidity().unwrap(),
        route.deposits_verified - 20_000
    );
    let paid = ledger.clone();
    let again = committee.decision(&g, &gov, &kind, H + 4);
    verify_live_signed_governance_batch(&g, &gov, &committee.registry, &again, H + 4).unwrap();
    assert!(
        !crate::execution_actions::execute_governance_batch(
            &mut gov,
            Some(&mut ledger),
            Some(&g),
            &again,
            H + 4
        )[0]
        .accepted
    );
    assert_eq!(ledger, paid);
    let stale = kind.replace(&"ab".repeat(64), &"de".repeat(64));
    let changed = committee.decision(&g, &gov, &stale, H + 4);
    verify_live_signed_governance_batch(&g, &gov, &committee.registry, &changed, H + 4).unwrap();
    assert!(
        !crate::execution_actions::execute_governance_batch(
            &mut gov,
            Some(&mut ledger),
            Some(&g),
            &changed,
            H + 4
        )[0]
        .accepted
    );
    assert_eq!(ledger, paid);
}

fn exported_witness(dir: &Path, withdrawal: &str) -> postfiat_types::PfUsdcEgressProofWitnessV1 {
    crate::pfusdc_egress_witness(crate::PfUsdcEgressWitnessOptions {
        data_dir: dir.into(),
        withdrawal_id: withdrawal.into(),
        prior_checkpoint_block_id: None,
    })
    .unwrap()
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn signed_blocks_replay_registration_burn_and_settlement() {
    certified_fork(None);
}

fn certified_fork(output: Option<&Path>) {
    use super::genesis_external_lane_tests::release_path::signed_batch_certificate;
    let source = NodeStore::new("/var/lib/postfiat/validator-3");
    let (g, mut gov, mut ledger) = archived_parent();
    let h = solana_exit::release_height(&g.chain_id, &genesis_hash(&g)).unwrap();
    if output.is_none() {
        assert_eq!(h, H);
    }
    let temp = unique_test_dir("solana-exit-certified-fork");
    fs::create_dir_all(&temp).unwrap();
    let dir = output.unwrap_or(&temp).join("local-store");
    assert!(!dir.exists(), "fork directory must be fresh");
    let store = NodeStore::new(&dir);
    store.init(&g, &source.read_node_state().unwrap()).unwrap();
    store
        .write_storage_backend_mode(postfiat_storage::StorageBackendMode::LegacyJsonl)
        .unwrap();
    let signing = NodeStore::new(temp.join("signers"));
    fs::create_dir_all(signing.data_dir()).unwrap();
    crate::storage_commit::ensure_validator_keys(&signing, g.validator_count).unwrap();
    let key_file = signing.data_dir().join(VALIDATOR_KEYS_FILE);
    let registry =
        read_validator_registry_file(&signing.data_dir().join(VALIDATOR_REGISTRY_FILE)).unwrap();
    crate::storage_commit::write_validator_registry_file(
        &dir.join(VALIDATOR_REGISTRY_FILE),
        &registry,
    )
    .unwrap();
    crate::storage_commit::write_validator_registry_file(
        &dir.join("validator_registry_genesis.json"),
        &registry,
    )
    .unwrap();
    // Local committee and holder. Preserve the archive's bridge and balance state.
    gov.validator_registry_updates.clear();
    let tx = funded_burn(&g, &mut ledger);
    store.write_ledger(&ledger).unwrap();
    store.write_governance(&gov).unwrap();
    let mut shielded = source.read_shielded().unwrap();
    let mut bridge = source.read_bridge().unwrap();
    store.write_shielded(&shielded).unwrap();
    store.write_bridge(&bridge).unwrap();
    store
        .write_receipts(&source.read_receipts().unwrap())
        .unwrap();
    store
        .write_ordered_batches(&source.read_ordered_batches().unwrap())
        .unwrap();
    let mut tip = source.read_chain_tip().unwrap();
    assert_eq!(tip.height, 1753);
    store.write_chain_tip(&tip).unwrap();
    tip.state_root = crate::lifecycle_queries::current_replicated_state_root(&store, &g).unwrap();
    store.write_chain_tip(&tip).unwrap();

    // Bulk-load the archived prefix with authenticated bounded appends, then
    // exercise normal legacy JSONL commits against the local fork.
    store
        .write_storage_backend_mode(postfiat_storage::StorageBackendMode::BoundedJsonl)
        .unwrap();
    let mut history = source.read_blocks().unwrap();
    let tail = history.blocks.split_off(history.blocks.len() / 2);
    store.write_blocks(&history).unwrap();
    for block in tail {
        store.append_block(block).unwrap();
    }
    store
        .write_storage_backend_mode(postfiat_storage::StorageBackendMode::LegacyJsonl)
        .unwrap();

    let names = active_validator_ids(&gov).unwrap();
    let sign = |a: GovernanceAmendment, height: u64| {
        let path = temp.join("amendment.json");
        write_amendment_file(&path, &a).unwrap();
        let mut signed = a;
        signed.signed_authorizations = names
            .iter()
            .map(|name| {
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: dir.clone(),
                    amendment_file: path.clone(),
                    validator: name.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: height,
                    expires_at_height: height,
                    authorization_file: temp.join("authorization.json"),
                })
                .unwrap()
            })
            .collect();
        signed
    };
    let decision = |kind: &str, value: u32, height: u64| {
        let a = ratify_governance_amendment_with_lifecycle(
            &cobalt_domain(&g),
            &EssentialSubsetConfig::all_of(names.clone()),
            kind,
            value,
            names.clone(),
            GovernanceAmendmentLifecycle {
                activation_height: height,
                paused: false,
                veto_until_height: 0,
            },
        )
        .unwrap();
        build_governance_action_batch(&g, vec![sign(a, height)], vec![]).unwrap()
    };
    let registration = |height| {
        let mut act = descriptor_batch()
            .vault_bridge_route_profile_activations
            .remove(0);
        act.amendment = sign(act.amendment, height);
        build_governance_action_batch_with_vault_bridge_route_profile_activation(&g, act).unwrap()
    };
    let mut commit = |kind: &str, id: &str, payload: String, accepted: bool| {
        let path = temp.join("batch.json");
        fs::write(&path, &payload).unwrap();
        let height = store.read_chain_tip().unwrap().height + 1;
        let cert =
            signed_batch_certificate(&dir, &path, &format!("test-{height}"), kind, &key_file);
        let opts = ApplyBatchOptions {
            data_dir: dir.clone(),
            batch_file: path,
            certificate_file: Some(cert),
        };
        let receipts = if kind == BATCH_KIND_GOVERNANCE {
            apply_governance_batch(opts)
        } else {
            apply_batch(opts)
        }
        .unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].accepted, accepted, "{:?}", receipts[0]);
        let block = store.read_blocks().unwrap().blocks.pop().unwrap();
        assert_eq!(block.header.batch_id, id);
        assert!(block.header.consensus_v2_commit.is_some());
        let entry = BatchArchiveEntry {
            batch_kind: kind.into(),
            batch_id: id.into(),
            payload_hash: batch_archive_payload_hash(&g, kind, id, &payload).unwrap(),
            payload_json: payload,
        };
        let replay = crate::block_replay_wallet::replay_archived_payload(
            &g,
            &block,
            &entry,
            crate::block_replay_wallet::ArchivedReplayState {
                parent_numbered_count: crate::solana_exit_governance::numbered_parent_count(&gov),
                governance: &mut gov,
                ledger: &mut ledger,
                shielded: &mut shielded,
                bridge: &mut bridge,
                validator_registry: &registry,
            },
        )
        .unwrap();
        assert_eq!(replay, receipts);
        assert!(
            ledger == store.read_ledger().unwrap(),
            "ledger replay mismatch at {height}"
        );
        assert!(
            gov == store.read_governance().unwrap(),
            "governance replay mismatch at {height}"
        );
        receipts.into_iter().next().unwrap()
    };
    let submit = || {
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: dir.clone(),
            signed_asset_transaction_json: serde_json::to_string(&tx).unwrap(),
        })
    };
    let mut next = 1754;
    while next < h {
        if next == h - 1 {
            let draft = descriptor_batch()
                .vault_bridge_route_profile_activations
                .remove(0)
                .amendment;
            let path = temp.join("early.json");
            write_amendment_file(&path, &draft).unwrap();
            let denied =
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: dir.clone(),
                    amendment_file: path,
                    validator: names[0].clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: next,
                    expires_at_height: next,
                    authorization_file: temp.join("early-authorization.json"),
                })
                .unwrap_err();
            assert!(
                denied.to_string().contains("Solana exit release"),
                "{denied}"
            );
            assert!(submit()
                .unwrap_err()
                .to_string()
                .contains("solana_exit_burns_closed"));
        }
        // Advance the local fork with ordinary signed governance blocks.
        let batch = decision(
            GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH,
            100 + next as u32,
            next,
        );
        commit(
            BATCH_KIND_GOVERNANCE,
            &batch.batch_id,
            serde_json::to_string(&batch).unwrap(),
            true,
        );
        next += 1;
    }
    assert!(submit()
        .unwrap_err()
        .to_string()
        .contains("solana_exit_burns_closed"));
    let batch = registration(h);
    let mut incomplete = batch.clone();
    incomplete.vault_bridge_route_profile_activations[0]
        .amendment
        .signed_authorizations
        .pop();
    assert!(verify_live_signed_governance_batch(
        &g,
        &store.read_governance().unwrap(),
        &registry,
        &incomplete,
        h
    )
    .is_err());
    commit(
        BATCH_KIND_GOVERNANCE,
        &batch.batch_id,
        serde_json::to_string(&batch).unwrap(),
        true,
    );
    assert!(submit()
        .unwrap_err()
        .to_string()
        .contains("solana_exit_burns_closed"));
    let batch = decision(OPEN, 1, h + 1);
    commit(
        BATCH_KIND_GOVERNANCE,
        &batch.batch_id,
        serde_json::to_string(&batch).unwrap(),
        true,
    );
    assert_eq!(
        crate::market_bridge::vault_bridge_route_at_height(
            &g,
            &store.read_governance().unwrap(),
            &store.read_ledger().unwrap(),
            store.read_shielded().unwrap().asset_orchard_balances(),
            h + 1,
            SERIES
        )
        .unwrap()
        .burns_open,
        Some(true)
    );
    submit().unwrap();
    let path = temp.join("burn.json");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: dir.clone(),
        batch_file: path.clone(),
        max_transactions: 1,
    })
    .unwrap();
    let before = store.read_ledger().unwrap();
    let (counted, queue, supply, outstanding) = bucket_amounts(&before);
    let holder_balance = |l: &LedgerState| {
        l.trustlines
            .iter()
            .find(|line| line.account == tx.unsigned.source && line.asset_id == SERIES)
            .unwrap()
            .balance
    };
    let batch = read_batch_file(&path).unwrap();
    assert_eq!(batch.asset_transactions, vec![tx.clone()]);
    let receipt = commit(
        BATCH_KIND_TRANSPARENT,
        &batch.batch_id,
        serde_json::to_string(&batch).unwrap(),
        true,
    );
    let after = store.read_ledger().unwrap();
    assert_eq!(
        bucket_amounts(&after),
        (counted, queue + 20_000, supply, outstanding - 20_000)
    );
    assert_eq!(holder_balance(&after) + 20_000, holder_balance(&before));
    let burn = after
        .vault_bridge_redemptions
        .iter()
        .find(|r| r.withdrawal_packet.burn_tx_id == receipt.tx_id)
        .unwrap();
    let witness = exported_witness(&dir, &burn.redemption_id);
    assert_eq!(witness.receipt, receipt);
    if let Some(out) = output {
        fs::write(
            out.join("local-witness.json"),
            serde_json::to_vec(&witness).unwrap(),
        )
        .unwrap();
        fs::write(
            out.join("local-burn.json"),
            serde_json::to_vec(
                &serde_json::json!({"redemption":burn,"transaction":tx,"receipt":receipt}),
            )
            .unwrap(),
        )
        .unwrap();
    } else {
        let paid = format!(
            "solana_exit_r8_paid:{}:12345:0:{}",
            burn.withdrawal_packet_hash,
            "bc".repeat(64)
        );
        let close = decision(solana_exit::CLOSE, 1, h + 3);
        commit(
            BATCH_KIND_GOVERNANCE,
            &close.batch_id,
            serde_json::to_string(&close).unwrap(),
            true,
        );
        assert!(solana_exit::open_height(&store.read_governance().unwrap()).is_none());
        let batch = decision(&paid, 1, h + 4);
        commit(
            BATCH_KIND_GOVERNANCE,
            &batch.batch_id,
            serde_json::to_string(&batch).unwrap(),
            true,
        );
        let settled = store.read_ledger().unwrap();
        assert_eq!(
            bucket_amounts(&settled),
            (
                counted - 20_000,
                queue,
                supply - 20_000,
                outstanding - 20_000
            )
        );
        assert_eq!(holder_balance(&settled), holder_balance(&after));
        settled.validate_asset_state(&g.chain_id).unwrap();
        let batch = decision(&paid, 1, h + 5);
        commit(
            BATCH_KIND_GOVERNANCE,
            &batch.batch_id,
            serde_json::to_string(&batch).unwrap(),
            false,
        );
        assert_eq!(store.read_ledger().unwrap(), settled);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_registration_requires_all_six_active_signers() {
    let (g, gov, _) = archived_parent();
    let names = active_validator_ids(&gov).unwrap();
    let committee = LocalCommittee::new(&names);
    let mut act = descriptor_batch()
        .vault_bridge_route_profile_activations
        .remove(0);
    let original = &act.amendment;
    let quorum = names.len() - 1;
    act.amendment = ratify_governance_amendment_with_lifecycle(
        &cobalt_domain(&g),
        &EssentialSubsetConfig {
            validators: names.clone(),
            quorum,
        },
        &original.kind,
        original.value,
        names[..quorum].to_vec(),
        GovernanceAmendmentLifecycle {
            activation_height: original.activation_height,
            paused: false,
            veto_until_height: 0,
        },
    )
    .unwrap();
    committee.sign(&gov, &mut act.amendment, H);
    let batch =
        build_governance_action_batch_with_vault_bridge_route_profile_activation(&g, act).unwrap();
    let error = verify_live_signed_governance_batch(&g, &gov, &committee.registry, &batch, H)
        .expect_err("five valid signatures cannot register the exit");
    assert!(
        error.to_string().contains("every active validator"),
        "{error}"
    );
    let all = committee.registration(&g, &gov, H);
    verify_live_signed_governance_batch(&g, &gov, &committee.registry, &all, H).unwrap();
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_route_distinguishes_family_nav_and_settlement() {
    let (g, gov, ledger) = scheduled_parent(true);
    let lookup = |id| {
        serde_json::to_value(
            crate::market_bridge::vault_bridge_route_at_height(&g, &gov, &ledger, &[], H + 2, id)
                .unwrap(),
        )
        .unwrap()
    };
    let sol = lookup(SERIES);
    let family = lookup(solana_exit::FAMILY);
    assert_eq!(sol["route_trust_class"], "VALIDATOR_ATTESTED_SETTLEMENT");
    assert_eq!(
        sol["family_nav_profile"]["policy_hash"],
        family["nav_profile_policy_hash"]
    );
    assert!(sol.get("nav_profile_policy_hash").is_none());
    assert_eq!(sol["source_backing"]["revision"], 8);
    assert_eq!(sol["source_backing"]["bucket_id"], solana_exit::BUCKET);
    assert_eq!(sol["source_backing"]["policy_hash"], solana_exit::POLICY);
    assert_eq!(family["route_trust_class"], "TRUSTLESS_FINALITY");
    use postfiat_rpc_sdk::{success_response, validate_health_response, RpcResponseKind};
    let validate = |report| {
        validate_health_response(
            &success_response("route", report, vec![]).unwrap(),
            RpcResponseKind::VaultBridgeRoute,
        )
    };
    validate(&sol).unwrap();
    let mut misleading = sol.clone();
    misleading["route_trust_class"] = "TRUSTLESS_FINALITY".into();
    assert!(validate(&misleading).is_err());
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_open_checks_checkpoint_history() {
    let (g, mut gov, _) = scheduled_parent(false);
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let open = |gov: &GovernanceState| {
        let batch = committee.decision(&g, gov, OPEN, H + 1);
        verify_live_signed_governance_batch(&g, gov, &committee.registry, &batch, H + 1)
    };
    // The saved Cobalt-only transitions at 917 and 924 precede the checkpoint.
    open(&gov).unwrap();
    let mut rotation = gov.validator_registry_updates.last().unwrap().clone();
    assert!(
        !rotation.cobalt_authorizations.is_empty() && rotation.signed_authorizations.is_empty()
    );
    // Model the same authorization form in the checkpoint-to-release gap.
    rotation.activation_height = 1755;
    gov.validator_registry_updates.push(rotation);
    let error = open(&gov).expect_err("a disconnected checker cannot open burns");
    assert!(error.to_string().contains("1755"), "{error}");
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_close_drain_rotate_and_reopen() {
    let (g, mut gov, mut ledger) = scheduled_parent(true);
    let (registry, keys, batch) = crate::cobalt_handoff::signed_exit_rotation(&g, &mut gov, H + 6);
    let committee = LocalCommittee { registry, keys };
    let decision = |gov: &mut GovernanceState, ledger: &mut LedgerState, kind, height| {
        let batch = committee.decision(&g, gov, kind, height);
        verify_live_signed_governance_batch(&g, gov, &committee.registry, &batch, height)?;
        let receipts = crate::execution_actions::execute_governance_batch(
            gov,
            Some(ledger),
            Some(&g),
            &batch,
            height,
        );
        assert!(receipts[0].accepted, "{}", receipts[0].message);
        Ok::<(), io::Error>(())
    };
    let tx = funded_burn(&g, &mut ledger);
    let source = NodeStore::new("/var/lib/postfiat/validator-3");
    let mut block = source.read_blocks().unwrap().blocks.pop().unwrap();
    block.header.height = H + 6;
    block.header.batch_kind = "governance".into();
    block.header.batch_id = batch.batch_id.clone();
    let payload = serde_json::to_string(&batch).unwrap();
    let entry = BatchArchiveEntry {
        batch_kind: "governance".into(),
        batch_id: batch.batch_id.clone(),
        payload_hash: batch_archive_payload_hash(&g, "governance", &batch.batch_id, &payload)
            .unwrap(),
        payload_json: payload,
    };
    let rotation = |gov: &GovernanceState, ledger: &LedgerState| {
        let mut rotated = gov.clone();
        let receipts = crate::block_replay_wallet::replay_archived_payload(
            &g,
            &block,
            &entry,
            crate::block_replay_wallet::ArchivedReplayState {
                parent_numbered_count: crate::solana_exit_governance::numbered_parent_count(gov),
                governance: &mut rotated,
                ledger: &mut ledger.clone(),
                shielded: &mut source.read_shielded().unwrap(),
                bridge: &mut source.read_bridge().unwrap(),
                validator_registry: &committee.registry,
            },
        )?;
        assert!(receipts[0].accepted);
        Ok::<_, io::Error>(rotated)
    };
    let error = rotation(&gov, &ledger)
        .err()
        .expect("open admission blocks rotation with an empty queue");
    assert!(error.to_string().contains("burns are open"), "{error}");
    assert!(block_burn(&g, &gov, &mut ledger, &tx, H + 2).accepted);
    let packet = ledger
        .vault_bridge_redemptions
        .last()
        .unwrap()
        .withdrawal_packet_hash
        .clone();
    decision(&mut gov, &mut ledger, "solana_exit_r8_close", H + 3).unwrap();
    assert_eq!(solana_exit::open_height(&gov), None);
    let pending = rotation(&gov, &ledger)
        .err()
        .expect("closed pending queue blocks rotation");
    assert!(
        pending.to_string().contains("redemptions are pending"),
        "{pending}"
    );
    let payout = format!(
        "{}{}:12345:0:{}",
        solana_exit::PAID,
        packet,
        "ab".repeat(64)
    );
    decision(&mut gov, &mut ledger, &payout, H + 4).unwrap();
    assert_eq!(bucket_amounts(&ledger).3, 1_610_000);
    let error = rotation(&gov, &ledger)
        .err()
        .expect("outstanding holders still need an exit");
    assert!(error.to_string().contains("outstanding"), "{error}");
    decision(&mut gov, &mut ledger, OPEN, H + 5).unwrap();
    assert_eq!(solana_exit::open_height(&gov), Some(H + 5));
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_unanimous_close_stops_new_burns() {
    let (g, mut gov, mut ledger) = scheduled_parent(true);
    let tx = funded_burn(&g, &mut ledger);
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let batch = committee.decision(&g, &gov, "solana_exit_r8_close", H + 2);
    let mut weak = batch.clone();
    weak.amendments[0].signed_authorizations.pop();
    assert!(
        verify_live_signed_governance_batch(&g, &gov, &committee.registry, &weak, H + 2).is_err()
    );
    verify_live_signed_governance_batch(&g, &gov, &committee.registry, &batch, H + 2).unwrap();
    let receipts = crate::execution_actions::execute_governance_batch(
        &mut gov,
        Some(&mut ledger),
        Some(&g),
        &batch,
        H + 2,
    );
    assert!(receipts[0].accepted);
    assert_eq!(
        block_burn(&g, &gov, &mut ledger, &tx, H + 3).code,
        "solana_exit_burns_closed"
    );
    let repeat = committee.decision(&g, &gov, "solana_exit_r8_close", H + 3);
    assert!(
        verify_live_signed_governance_batch(&g, &gov, &committee.registry, &repeat, H + 3).is_err()
    );
}

#[test]
#[ignore = "copied transactional WAN ledger at height 1753"]
fn exit_review_route_reads_transactional_archive() {
    let data_dir = PathBuf::from("/var/lib/postfiat/validator-3");
    assert!(NodeStore::new(&data_dir)
        .transactional_storage_active()
        .unwrap());
    let route = vault_bridge_route(VaultBridgeRouteOptions {
        data_dir,
        asset_id: solana_exit::FAMILY.into(),
    })
    .unwrap();
    assert_eq!(route.current_height, 1753);
    assert_eq!(route.profile_hash, ETH_HASH);
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_route_waits_for_committed_close() {
    let source = NodeStore::new("/var/lib/postfiat/validator-3");
    let (g, mut gov, mut ledger) = scheduled_parent(true);
    let dir = unique_test_dir("route-commit");
    let store = NodeStore::new(&dir);
    store.init(&g, &source.read_node_state().unwrap()).unwrap();
    store.write_governance(&gov).unwrap();
    store.write_ledger(&ledger).unwrap();
    let mut tip = source.read_chain_tip().unwrap();
    tip.height = H + 2;
    store.write_chain_tip(&tip).unwrap();
    let guard = store.lock_ordered_commit().unwrap();
    let (started, start_rx) = std::sync::mpsc::channel();
    let (done, rx) = std::sync::mpsc::channel();
    let path = dir.clone();
    let reader = std::thread::spawn(move || {
        started.send(()).unwrap();
        done.send(vault_bridge_route(VaultBridgeRouteOptions {
            data_dir: path,
            asset_id: SERIES.into(),
        }))
        .unwrap();
    });
    start_rx.recv().unwrap();
    assert!(
        matches!(
            rx.recv_timeout(std::time::Duration::from_millis(500)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ),
        "route read returned during an open commit"
    );
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let batch = committee.decision(&g, &gov, solana_exit::CLOSE, H + 3);
    assert!(
        crate::execution_actions::execute_governance_batch(
            &mut gov,
            Some(&mut ledger),
            Some(&g),
            &batch,
            H + 3
        )[0]
        .accepted
    );
    store.write_governance(&gov).unwrap();
    tip.height = H + 3;
    store.write_chain_tip(&tip).unwrap();
    drop(guard);
    let report = rx.recv().unwrap().unwrap();
    assert_eq!(
        (report.current_height, report.burns_open),
        (H + 3, Some(false))
    );
    reader.join().unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_numbered_set_uses_parent_membership() {
    let (g, gov, ledger) = scheduled_parent(true);
    let parent = active_validator_ids(&gov).unwrap();
    assert_eq!(parent.len(), 6);
    let five = local_validator_ids(5).unwrap();
    let amendment = ratify_governance_amendment_with_lifecycle(
        &cobalt_domain(&g),
        &EssentialSubsetConfig::all_of(five.clone()),
        GOVERNANCE_KIND_VALIDATOR_SET,
        5,
        five,
        GovernanceAmendmentLifecycle {
            activation_height: H + 1,
            veto_until_height: 0,
            paused: false,
        },
    )
    .unwrap();
    let batch = build_governance_action_batch(&g, vec![amendment], vec![]).unwrap();
    assert!(crate::solana_exit_governance::needs_signatures(
        &g,
        &batch,
        H + 1
    ));
    assert!(crate::solana_exit_governance::verify_rotation_interlock(
        &g,
        &gov,
        &ledger,
        crate::solana_exit_governance::numbered_parent_count(&gov),
        &batch,
        H + 1,
    )
    .is_err());
    let committee = LocalCommittee::new(&parent);
    assert!(
        verify_live_signed_governance_batch(&g, &gov, &committee.registry, &batch, H + 1).is_err()
    );
    let mut same = ratify_governance_amendment_with_lifecycle(
        &cobalt_domain(&g),
        &EssentialSubsetConfig::all_of(parent.clone()),
        GOVERNANCE_KIND_VALIDATOR_SET,
        6,
        parent,
        GovernanceAmendmentLifecycle {
            activation_height: H + 1,
            veto_until_height: 0,
            paused: false,
        },
    )
    .unwrap();
    let unsigned = build_governance_action_batch(&g, vec![same.clone()], vec![]).unwrap();
    assert!(crate::solana_exit_governance::needs_signatures(
        &g,
        &unsigned,
        H + 1
    ));
    assert!(
        verify_live_signed_governance_batch(&g, &gov, &committee.registry, &unsigned, H + 1)
            .is_err()
    );
    committee.sign(&gov, &mut same, H + 1);
    let signed = build_governance_action_batch(&g, vec![same], vec![]).unwrap();
    verify_live_signed_governance_batch(&g, &gov, &committee.registry, &signed, H + 1).unwrap();
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review_registration_holds_rotation_until_open_or_retired() {
    let (g, mut gov, mut ledger) = scheduled_parent(false);
    let (registry, keys, rotation) =
        crate::cobalt_handoff::signed_exit_rotation(&g, &mut gov, H + 6);
    let committee = LocalCommittee { registry, keys };
    let check = |gov: &GovernanceState, ledger: &LedgerState| {
        crate::solana_exit_governance::verify_rotation_interlock(
            &g,
            gov,
            ledger,
            crate::solana_exit_governance::numbered_parent_count(gov),
            &rotation,
            H + 6,
        )
    };
    let retire = |gov: &mut GovernanceState, ledger: &mut LedgerState| {
        let batch = committee.decision(&g, gov, solana_exit::RETIRE, H + 2);
        verify_live_signed_governance_batch(&g, gov, &committee.registry, &batch, H + 2).unwrap();
        crate::execution_actions::execute_governance_batch(
            gov,
            Some(ledger),
            Some(&g),
            &batch,
            H + 2,
        )
        .remove(0)
    };
    let bucket = ledger
        .vault_bridge_bucket_states
        .iter_mut()
        .find(|b| solana_exit::matches_bucket(b))
        .unwrap();
    bucket.outstanding_vault_bridge_atoms = 0;
    // Model a drained series before testing the remaining ingress obligation.
    for line in ledger.trustlines.iter_mut().filter(|l| l.asset_id == SERIES) {
        line.balance = 0;
    }
    assert!(
        check(&gov, &ledger).is_err(),
        "new registration must protect permanent initialization"
    );
    assert!(
        !retire(&mut gov, &mut ledger).accepted,
        "revision 8 can still receive deposits"
    );
    // Once opened and closed, direct deposits alone still require a usable checker.
    let mut closed = gov.clone();
    for (kind, height) in [(OPEN, H + 1), (solana_exit::CLOSE, H + 2)] {
        let batch = committee.decision(&g, &closed, kind, height);
        verify_live_signed_governance_batch(&g, &closed, &committee.registry, &batch, height)
            .unwrap();
        assert!(
            crate::execution_actions::execute_governance_batch(
                &mut closed,
                Some(&mut ledger),
                Some(&g),
                &batch,
                height,
            )[0]
            .accepted
        );
    }
    assert!(check(&closed, &ledger)
        .unwrap_err()
        .to_string()
        .contains("receive deposits"));
    // A higher ingress revision permanently supersedes revision 8.
    let mut successor = ledger
        .solana_clients()
        .max_by_key(|r| r.profile.route_epoch)
        .unwrap()
        .clone();
    successor.profile.route_epoch += 1;
    let hash = successor.profile.profile_hash().unwrap();
    ledger.additional_solana_routes.insert(hash, successor);
    check(&closed, &ledger).unwrap();
    assert!(
        check(&gov, &ledger).is_err(),
        "registration still protects the first open"
    );
    ledger
        .vault_bridge_bucket_states
        .iter_mut()
        .find(|b| solana_exit::matches_bucket(b))
        .unwrap()
        .outstanding_vault_bridge_atoms = 1;
    assert!(!retire(&mut gov, &mut ledger).accepted);
    ledger
        .vault_bridge_bucket_states
        .iter_mut()
        .find(|b| solana_exit::matches_bucket(b))
        .unwrap()
        .outstanding_vault_bridge_atoms = 0;
    let mut short = committee.decision(&g, &gov, solana_exit::RETIRE, H + 2);
    short.amendments[0].signed_authorizations.pop();
    assert!(
        verify_live_signed_governance_batch(&g, &gov, &committee.registry, &short, H + 2).is_err()
    );
    assert!(retire(&mut gov, &mut ledger).accepted);
    check(&gov, &ledger).unwrap();
    assert!(solana_exit::active(&gov, H + 1).is_ok());
    assert!(solana_exit::active(&gov, H + 3).is_err());
    let open = committee.decision(&g, &gov, OPEN, H + 3);
    assert!(
        verify_live_signed_governance_batch(&g, &gov, &committee.registry, &open, H + 3).is_err()
    );
}

fn check_registration_refusal(
    g: &Genesis,
    gov: &GovernanceState,
    ledger: &LedgerState,
    detail: &str,
) {
    let committee = LocalCommittee::new(&active_validator_ids(gov).unwrap());
    let register = committee.registration(g, gov, H);
    verify_live_signed_governance_batch(g, gov, &committee.registry, &register, H).unwrap();
    let mut state = gov.clone();
    let receipts = crate::execution_actions::execute_governance_batch(
        &mut state,
        Some(&mut ledger.clone()),
        Some(g),
        &register,
        H,
    );
    assert_eq!(receipts.len(), 1);
    assert!(!receipts[0].accepted, "registration accepted");
    assert!(
        receipts[0].message.contains(detail),
        "{}",
        receipts[0].message
    );
    assert!(state == *gov, "registration changed governance");
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review4_registration_checks_checkpoint_rotation() {
    let (g, mut gov, ledger) = archived_parent();
    for height in [1755, H + 5] {
        let (_, _, batch) = crate::cobalt_handoff::signed_exit_rotation(&g, &mut gov, height);
        gov.validator_registry_updates
            .extend(batch.validator_registry_updates);
        check_registration_refusal(&g, &gov, &ledger, &height.to_string());
        gov.validator_registry_updates.pop();
    }
}

fn export_root_schedule(
    g: &Genesis,
    gov: &GovernanceState,
    height: u64,
    effective: u64,
) -> GovernanceActionBatch {
    signed_amendment_batch(
        g,
        gov,
        postfiat_types::GOVERNANCE_KIND_EXTERNAL_LANE_EXPORT_ROOT_ACTIVATION_HEIGHT,
        effective.try_into().unwrap(),
        height,
    )
}

fn signed_amendment_batch(
    g: &Genesis,
    gov: &GovernanceState,
    kind: &str,
    value: u32,
    height: u64,
) -> GovernanceActionBatch {
    let names = active_validator_ids(gov).unwrap();
    let mut amendment = ratify_governance_amendment_with_lifecycle(
        &cobalt_domain(g),
        &EssentialSubsetConfig::all_of(names.clone()),
        kind,
        value,
        names.clone(),
        GovernanceAmendmentLifecycle {
            activation_height: height,
            paused: false,
            veto_until_height: 0,
        },
    )
    .unwrap();
    let committee = LocalCommittee::new(&names);
    committee.sign(gov, &mut amendment, height);
    let batch = build_governance_action_batch(g, vec![amendment], vec![]).unwrap();
    verify_live_signed_governance_batch(g, gov, &committee.registry, &batch, height).unwrap();
    batch
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review4_registration_checks_export_root_schedule() {
    let (g, parent, ledger) = archived_parent();
    for effective in [H - 1, H, H + 3] {
        let mut gov = parent.clone();
        let schedule = export_root_schedule(&g, &gov, H - 2, effective);
        let receipts = crate::execution_actions::execute_governance_batch(
            &mut gov,
            Some(&mut ledger.clone()),
            Some(&g),
            &schedule,
            H - 2,
        );
        assert!(receipts[0].accepted, "{}", receipts[0].message);
        check_registration_refusal(&g, &gov, &ledger, "export");
    }
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review4_protected_exit_rejects_export_root_schedule() {
    for open in [false, true] {
        let (g, gov, ledger) = scheduled_parent(open);
        let batch = export_root_schedule(&g, &gov, H + 2, H + 3);
        let error = crate::solana_exit_governance::verify_rotation_interlock(
            &g,
            &gov,
            &ledger,
            crate::solana_exit_governance::numbered_parent_count(&gov),
            &batch,
            H + 2,
        )
        .expect_err("a protected checker cannot schedule the new header");
        assert!(error.to_string().contains("export"), "{error}");
    }
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review4_protected_exit_rejects_export_root_activation() {
    let (g, mut gov, ledger) = scheduled_parent(true);
    let batch = export_root_schedule(&g, &gov, H + 2, H + 3);
    // Model a retained schedule to exercise activation independently of admission.
    gov.amendments.extend(batch.amendments);
    for height in [H + 3, H + 4] {
        let error = crate::batch_snapshot::external_lane_export_root_for_block(
            &g,
            &gov,
            &ledger,
            &[],
            height,
        )
        .expect_err("an empty export block still changes the signed header");
        assert!(error.to_string().contains("export"), "{error}");
    }
    assert!(crate::batch_snapshot::external_lane_export_root_for_block(
        &g,
        &gov,
        &ledger,
        &[],
        H + 2
    )
    .unwrap()
    .is_none());
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review4_open_checks_retained_export_schedule() {
    let (g, mut gov, _) = scheduled_parent(false);
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let batch = export_root_schedule(&g, &gov, H + 1, H + 5);
    gov.amendments.extend(batch.amendments);
    let open = committee.decision(&g, &gov, OPEN, H + 2);
    let error = verify_live_signed_governance_batch(&g, &gov, &committee.registry, &open, H + 2)
        .expect_err("opening would cross the retained header schedule");
    assert!(error.to_string().contains("export"), "{error}");
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review4_rejects_due_registry_update_before_using_new_committee() {
    let (g, mut gov, _) = scheduled_parent(true);
    let (_, _, batch) = crate::cobalt_handoff::signed_exit_rotation(&g, &mut gov, H + 3);
    gov.validator_registry_updates
        .extend(batch.validator_registry_updates);
    let store = NodeStore::new("/var/lib/postfiat/validator-3");
    for height in [H + 3, H + 4] {
        let error =
            activate_due_validator_registry_updates_for_commit(&store, &g, &mut gov, height)
                .err()
                .expect("a retained rotation must not activate while burns are open");
        assert!(error.to_string().contains("burns are open"), "{error}");
    }
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review4_registration_checks_recorded_numbered_set_change() {
    let (g, mut gov, ledger) = archived_parent();
    // Even restoring the six members before registration leaves a gap in ancestry.
    for (height, count) in [(1755, 5), (1756, 6)] {
        let batch = signed_amendment_batch(&g, &gov, GOVERNANCE_KIND_VALIDATOR_SET, count, height);
        let receipts = crate::execution_actions::execute_governance_batch(
            &mut gov,
            Some(&mut ledger.clone()),
            Some(&g),
            &batch,
            height,
        );
        assert!(receipts[0].accepted, "{}", receipts[0].message);
    }
    assert_eq!(gov.active_validator_count, 6);
    check_registration_refusal(&g, &gov, &ledger, "validator-set amendment at height 1755");
}

#[test]
#[ignore = "copied WAN ledger at checkpoint 1753"]
fn exit_review4_export_root_genesis_schedule_is_incompatible() {
    let (mut g, gov, _) = archived_parent();
    // This genesis would have another hash and cannot use the WAN release.
    g.external_lane_export_root_activation_height = Some(H + 9);
    assert!(
        crate::solana_exit_governance::verify_checkpoint_history(&g, &gov)
            .unwrap_err()
            .contains("1769")
    );
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review5_burn_reconciles_orchard_and_public_claims() {
    let (g, gov, mut ledger) = scheduled_parent(true);
    let tx = funded_burn(&g, &mut ledger);
    let batch =
        TransactionBatch::new_with_asset_transactions(String::new(), vec![], vec![], vec![tx]);
    let orchard = [postfiat_types::AssetOrchardAssetBalance {
        asset_id: SERIES.into(),
        ingress_total: 1,
        egress_total: 0,
        live_total: 1,
    }];
    let compat = crate::execution_actions::asset_execution_compatibility_for_genesis_and_governance(
        &g, &gov,
    );
    let run = |ledger: &mut LedgerState| {
        crate::execution_actions::execute_transparent_batch_with_orchard(
            &g,
            &gov,
            ledger,
            &batch,
            H + 2,
            compat,
            &orchard,
        )
        .remove(0)
    };
    let before = bucket_amounts(&ledger);
    let receipt = run(&mut ledger);
    assert_eq!(receipt.code, "solana_exit_supply_mismatch");
    assert_eq!(bucket_amounts(&ledger), before);
    ledger
        .trustlines
        .iter_mut()
        .find(|l| l.asset_id == SERIES && l.balance > 20_000)
        .unwrap()
        .balance -= 1;
    let receipt = run(&mut ledger);
    assert!(receipt.accepted, "{}", receipt.message);
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review5_open_rejects_unreconciled_supply() {
    let (g, mut gov, mut ledger) = scheduled_parent(false);
    ledger
        .trustlines
        .iter_mut()
        .find(|l| l.asset_id == SERIES && l.balance > 0)
        .unwrap()
        .balance -= 1;
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let batch = committee.decision(&g, &gov, OPEN, H + 1);
    let receipt = crate::execution_actions::execute_governance_batch(
        &mut gov,
        Some(&mut ledger),
        Some(&g),
        &batch,
        H + 1,
    )
    .remove(0);
    assert!(
        !receipt.accepted && receipt.message.contains("outstanding"),
        "{}",
        receipt.message
    );
    assert!(solana_exit::open_height(&gov).is_none());
    let orchard = [postfiat_types::AssetOrchardAssetBalance {
        asset_id: SERIES.into(),
        ingress_total: 1,
        egress_total: 0,
        live_total: 1,
    }];
    let receipt = crate::execution_actions::execute_governance_batch_with_replay(
        &mut gov,
        Some(&mut ledger),
        Some(&g),
        &batch,
        H + 1,
        false,
        &orchard,
    )
    .remove(0);
    assert!(receipt.accepted, "{}", receipt.message);
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review5_route_reports_both_claim_totals() {
    let (g, gov, mut ledger) = scheduled_parent(false);
    ledger
        .trustlines
        .iter_mut()
        .find(|l| l.asset_id == SERIES && l.balance > 0)
        .unwrap()
        .balance -= 1;
    let route =
        crate::market_bridge::vault_bridge_route_at_height(&g, &gov, &ledger, &[], H, SERIES)
            .unwrap();
    let backing = &serde_json::to_value(route).unwrap()["source_backing"];
    assert_eq!(backing["series_supply_atoms"], 1_629_999);
    assert_eq!(backing["outstanding_vault_bridge_atoms"], 1_630_000);
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review5_standalone_asset_burn_cannot_orphan_backing() {
    let (g, gov, mut ledger) = archived_parent();
    let (owner, public, private) = funded_holder(&mut ledger);
    let op = postfiat_types::AssetBurnOperation {
        owner: owner.clone(),
        issuer: ledger.asset_definition(SERIES).unwrap().issuer.clone(),
        asset_id: SERIES.into(),
        amount: 1,
    };
    let tx = signed_asset_transaction_for_test(
        &g,
        &ledger,
        &owner,
        &public,
        &private,
        postfiat_types::ASSET_BURN_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::AssetBurn(op),
    );
    assert!(block_burn(&g, &gov, &mut ledger.clone(), &tx, H - 1).accepted);
    let compat = crate::execution_actions::asset_execution_compatibility_for_genesis_and_governance(
        &g, &gov,
    );
    let paired = postfiat_execution::execute_asset_orchard_ingress_burn(
        &g,
        &mut ledger.clone(),
        &tx,
        H,
        compat,
    );
    assert!(paired.accepted, "{}", paired.message);
    assert_eq!(
        block_burn(&g, &gov, &mut ledger, &tx, H).code,
        "solana_exit_unmatched_supply_change"
    );
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review5_batch_builder_and_committed_route_include_orchard() {
    let source = NodeStore::new("/var/lib/postfiat/validator-3");
    let (g, gov, mut ledger) = scheduled_parent(true);
    let dir = unique_test_dir("revision8-parent");
    let store = NodeStore::new(&dir);
    store.init(&g, &source.read_node_state().unwrap()).unwrap();
    let burn = funded_burn(&g, &mut ledger);
    let path = dir.join("burn.json");
    std::fs::write(&path, serde_json::to_vec(&burn).unwrap()).unwrap();
    let mut tip = source.read_chain_tip().unwrap();
    tip.height = H + 1;
    tip.ordered_batch_count = tip.height;
    store.write_chain_tip(&tip).unwrap();
    store.write_governance(&gov).unwrap();
    let mut shielded = source.read_shielded().unwrap();
    let balances = &mut shielded.orchard.as_mut().unwrap().asset_orchard_balances;
    assert!(!balances.iter().any(|b| b.asset_id == SERIES));
    balances.push(postfiat_types::AssetOrchardAssetBalance {
        asset_id: SERIES.into(),
        ingress_total: 1,
        egress_total: 0,
        live_total: 1,
    });
    store.write_shielded(&shielded).unwrap();
    store.write_ledger(&ledger).unwrap();
    let batch = || {
        create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
            data_dir: dir.clone(),
            signed_asset_transaction_files: vec![path.clone()],
            batch_file: dir.join("batch.json"),
        })
    };
    let route = || {
        serde_json::to_value(
            vault_bridge_route(VaultBridgeRouteOptions {
                data_dir: dir.clone(),
                asset_id: SERIES.into(),
            })
            .unwrap(),
        )
        .unwrap()
    };
    assert!(batch()
        .err()
        .expect("batch must refuse excess Orchard claims")
        .to_string()
        .contains("solana_exit_supply_mismatch"));
    let r = route();
    assert_eq!(r["source_backing"]["series_supply_atoms"], 1_630_001);
    assert_eq!(
        r["source_backing"]["outstanding_vault_bridge_atoms"],
        1_630_000
    );
    ledger
        .trustlines
        .iter_mut()
        .find(|l| l.asset_id == SERIES && l.balance > 20_000)
        .unwrap()
        .balance -= 1;
    store.write_ledger(&ledger).unwrap();
    batch().unwrap();
    assert_eq!(route()["source_backing"]["series_supply_atoms"], 1_630_000);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "copied WAN ledger, release height 1760"]
fn exit_review6_registration_reconciles_public_and_orchard_claims() {
    let (g, mut gov, mut ledger) = archived_parent();
    let committee = LocalCommittee::new(&active_validator_ids(&gov).unwrap());
    let registration = committee.registration(&g, &gov, H);
    let (owner, public, private) = funded_holder(&mut ledger);
    let burn = postfiat_types::AssetBurnOperation {
        owner: owner.clone(),
        issuer: ledger.asset_definition(SERIES).unwrap().issuer.clone(),
        asset_id: SERIES.into(),
        amount: 1,
    };
    let tx = signed_asset_transaction_for_test(
        &g,
        &ledger,
        &owner,
        &public,
        &private,
        postfiat_types::ASSET_BURN_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::AssetBurn(burn),
    );
    let receipt = block_burn(&g, &gov, &mut ledger, &tx, H - 1);
    assert!(receipt.accepted, "{}", receipt.message);
    let parent = gov.clone();
    let after_burn = ledger.clone();
    let register = |gov: &mut GovernanceState,
                    ledger: &mut LedgerState,
                    balances: &[postfiat_types::AssetOrchardAssetBalance]| {
        crate::execution_actions::execute_governance_batch_with_replay(
            gov,
            Some(ledger),
            Some(&g),
            &registration,
            H,
            false,
            balances,
        )
        .remove(0)
    };
    let refusal = register(&mut gov, &mut ledger, &[]);
    assert!(!refusal.accepted, "registered after a one-atom holder burn");
    assert_eq!(
        refusal.message,
        "revision-8 series supply 1629999 differs from bucket outstanding 1630000"
    );
    assert!(gov == parent && ledger == after_burn);

    // The same transparent balance is reconciled if the atom is in Orchard.
    let orchard = [postfiat_types::AssetOrchardAssetBalance {
        asset_id: SERIES.into(),
        ingress_total: 1,
        egress_total: 0,
        live_total: 1,
    }];
    let excess = [postfiat_types::AssetOrchardAssetBalance {
        ingress_total: 2,
        live_total: 2,
        ..orchard[0].clone()
    }];
    let refusal = register(&mut gov, &mut ledger, &excess);
    assert!(!refusal.accepted);
    assert_eq!(
        refusal.message,
        "revision-8 series supply 1630001 differs from bucket outstanding 1630000"
    );
    assert!(gov == parent && ledger == after_burn);
    let accepted = register(&mut gov, &mut ledger, &orchard);
    assert!(accepted.accepted, "{}", accepted.message);
    assert!(solana_exit::registered(&gov).is_some());
}
