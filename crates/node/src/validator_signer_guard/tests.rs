use super::*;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Barrier, Mutex};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "pf-signer-guard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&p).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
    fn state(&self) -> PathBuf {
        self.0.join("state")
    }
    fn open(&self, initialize: bool) -> SignerGuard {
        SignerGuard::open(&self.state(), initialize, vec!["test-key".into()]).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn public() -> ValidatorPublicKey {
    ValidatorPublicKey {
        node_id: "validator-0".into(),
        algorithm_id: ML_DSA_65_ALGORITHM.into(),
        public_key_hex: "ab".repeat(1952),
    }
}
fn vote(height: u64, view: u64, phase: ConsensusV2Phase) -> ConsensusV2Vote {
    ConsensusV2Vote {
        schema: "postfiat-consensus-vote-v2".into(),
        domain: ConsensusV2Domain {
            schema: "postfiat-consensus-domain-v2".into(),
            chain_id: "guard-test".into(),
            genesis_hash: "a".repeat(96),
            protocol_version: 1,
            committee_epoch: 1,
            committee_root: "b".repeat(96),
        },
        round: ConsensusV2Round { height, view },
        phase,
        block: None,
        validator: "validator-0".into(),
        signature: ConsensusV2Signature {
            algorithm_id: ML_DSA_65_ALGORITHM.into(),
            signer: "validator-0".into(),
            public_key_hex: public().public_key_hex,
            signature_hex: "00".into(),
        },
    }
}
fn bytes(v: &ConsensusV2Vote) -> Vec<u8> {
    consensus_v2_vote_signing_bytes(v).unwrap()
}

#[test]
fn guard_persists_before_signing_and_refuses_conflict_after_restart() {
    let d = Directory::new();
    let p = public();
    let v = vote(9, 2, ConsensusV2Phase::Prepare);
    let mut guard = d.open(true);
    guard
        .reserve(&p, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&v))
        .unwrap();
    drop(guard); // Crash after reservation and before signature is equally safe.
    let mut guard = d.open(false);
    guard
        .reserve(&p, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&v))
        .unwrap();
    let mut conflict = v.clone();
    conflict.domain.committee_root = "c".repeat(96);
    assert!(guard
        .reserve(&p, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&conflict))
        .unwrap_err()
        .to_string()
        .contains("double_sign"));
    conflict.domain.protocol_version = 2;
    assert!(guard
        .reserve(&p, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&conflict))
        .is_err());
    guard
        .reserve(
            &p,
            CONSENSUS_V2_VOTE_CONTEXT,
            &bytes(&vote(10, 0, ConsensusV2Phase::Prepare)),
        )
        .unwrap();
    assert!(guard
        .reserve(&p, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&v))
        .unwrap_err()
        .to_string()
        .contains("stale_round"));
}

#[test]
fn guard_serializes_conflicting_concurrent_requests() {
    let d = Directory::new();
    let shared = Arc::new(Mutex::new(d.open(true)));
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|i| {
            let shared = shared.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut v = vote(1, 0, ConsensusV2Phase::Prepare);
                v.domain.committee_root = if i == 0 { "b" } else { "c" }.repeat(96);
                barrier.wait();
                shared
                    .lock()
                    .unwrap()
                    .reserve(&public(), CONSENSUS_V2_VOTE_CONTEXT, &bytes(&v))
                    .is_ok()
            })
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter(|v| *v)
            .count(),
        1
    );
}

#[test]
fn guard_separates_phase_and_chain_but_not_key_aliases() {
    let d = Directory::new();
    let mut guard = d.open(true);
    let p = public();
    let v = vote(3, 0, ConsensusV2Phase::Prepare);
    guard
        .reserve(&p, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&v))
        .unwrap();
    guard
        .reserve(
            &p,
            CONSENSUS_V2_VOTE_CONTEXT,
            &bytes(&vote(3, 0, ConsensusV2Phase::Precommit)),
        )
        .unwrap();
    let mut other_chain = v.clone();
    other_chain.domain.chain_id = "another-chain".into();
    guard
        .reserve(&p, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&other_chain))
        .unwrap();
    let mut alias = p.clone();
    alias.node_id = "alias".into();
    let mut other_id = v.clone();
    other_id.validator = alias.node_id.clone();
    assert!(guard
        .reserve(&alias, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&other_id))
        .unwrap_err()
        .to_string()
        .contains("double_sign"));
    alias.public_key_hex.make_ascii_uppercase();
    assert!(guard
        .reserve(&alias, CONSENSUS_V2_VOTE_CONTEXT, &bytes(&other_id))
        .is_err());
}

#[test]
fn guard_refuses_missing_corrupt_reinitialized_or_shared_state() {
    let d = Directory::new();
    let guard = d.open(true);
    assert!(SignerGuard::open(&d.state(), false, vec!["test-key".into()]).is_err());
    assert!(SignerGuard::open(&d.state(), true, vec!["test-key".into()]).is_err());
    drop(guard);
    assert!(SignerGuard::open(&d.state(), false, vec!["other-key".into()]).is_err());
    std::fs::write(d.state().join("state.json"), b"{torn").unwrap();
    assert!(SignerGuard::open(&d.state(), false, vec!["test-key".into()]).is_err());
    std::fs::remove_file(d.state().join("state.json")).unwrap();
    assert!(SignerGuard::open(&d.state(), false, vec!["test-key".into()]).is_err());
}

#[test]
fn guard_poisoned_after_a_failed_durable_write_never_authorizes_a_signature() {
    let d = Directory::new();
    let mut guard = d.open(true);
    std::fs::remove_file(d.state().join("state.json")).unwrap();
    std::fs::create_dir(d.state().join("state.json")).unwrap(); // atomic rename must fail
    let msg = bytes(&vote(1, 0, ConsensusV2Phase::Prepare));
    assert!(guard
        .reserve(&public(), CONSENSUS_V2_VOTE_CONTEXT, &msg)
        .is_err());
    std::fs::remove_dir(d.state().join("state.json")).unwrap();
    assert!(!guard.available());
    assert!(guard
        .reserve(&public(), CONSENSUS_V2_VOTE_CONTEXT, &msg)
        .is_err());
    assert!(guard
        .reserve(&public(), TRANSPORT_AUTH_CONTEXT, b"transport")
        .is_err());
}

#[test]
fn guard_refuses_symlink_and_world_readable_state() {
    let d = Directory::new();
    drop(d.open(true));
    let path = d.state().join("state.json");
    let moved = d.0.join("moved.json");
    std::fs::rename(&path, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &path).unwrap();
    assert!(SignerGuard::open(&d.state(), false, vec!["test-key".into()]).is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::rename(&moved, &path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(SignerGuard::open(&d.state(), false, vec!["test-key".into()]).is_err());
}

#[test]
fn guard_refuses_writable_ancestors_and_valid_json_checksum_tampering() {
    let d = Directory::new();
    drop(d.open(true));
    std::fs::set_permissions(&d.0, std::fs::Permissions::from_mode(0o770)).unwrap();
    assert!(SignerGuard::open(&d.state(), false, vec!["test-key".into()]).is_err());
    std::fs::set_permissions(&d.0, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = d.state().join("state.json");
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    state["state"]["schema"] = "another-schema".into();
    std::fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    assert!(SignerGuard::open(&d.state(), false, vec!["test-key".into()]).is_err());
}

#[test]
fn decoder_refuses_malformed_trailing_and_spoofed_identity() {
    let v = vote(1, 0, ConsensusV2Phase::Prepare);
    let mut msg = bytes(&v);
    msg.extend_from_slice(b"round.height=2\n");
    assert!(decode_slot(&public(), CONSENSUS_V2_VOTE_CONTEXT, &msg).is_err());
    let mut bad = v.clone();
    bad.validator = "validator-1".into();
    assert!(decode_slot(&public(), CONSENSUS_V2_VOTE_CONTEXT, &bytes(&bad)).is_err());
    assert!(decode_slot(&public(), CONSENSUS_V2_VOTE_CONTEXT, b"schema=99999:x").is_err());
    assert!(decode_slot(
        &public(),
        BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
        b"arbitrary bytes"
    )
    .is_err());
}

#[test]
fn decoder_uses_outer_round_with_nested_qc_and_both_optional_roots() {
    let v = vote(9, 3, ConsensusV2Phase::Prepare);
    let block = ConsensusV2BlockRef {
        height: 9,
        block_id: "a".repeat(96),
        parent_block_id: "b".repeat(96),
        payload_hash: "c".repeat(96),
        state_root: "d".repeat(96),
        bridge_exit_root: Some("e".repeat(96)),
        pftl_uniswap_receipt_root: Some("f".repeat(96)),
    };
    let qc = ConsensusV2QcRef {
        certificate_id: "a".repeat(96),
        round: ConsensusV2Round {
            height: 8,
            view: 99,
        },
        phase: ConsensusV2Phase::Precommit,
        block: block.clone(),
    };
    let proposal = ConsensusV2Proposal {
        schema: "postfiat-consensus-proposal-v2".into(),
        domain: v.domain.clone(),
        round: v.round,
        block,
        valid_qc: Some(qc.clone()),
        timeout_certificate_id: Some("b".repeat(96)),
        proposer: v.validator.clone(),
        signature: v.signature.clone(),
    };
    let slot = decode_slot(
        &public(),
        CONSENSUS_V2_PROPOSAL_CONTEXT,
        &consensus_v2_proposal_signing_bytes(&proposal).unwrap(),
    )
    .unwrap();
    assert_eq!((slot.height, slot.view), (9, 3));
    let timeout = ConsensusV2TimeoutVote {
        schema: "postfiat-consensus-timeout-vote-v2".into(),
        domain: v.domain,
        round: v.round,
        phase: v.phase,
        high_qc: Some(qc),
        validator: v.validator,
        signature: v.signature,
    };
    let slot = decode_slot(
        &public(),
        CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT,
        &consensus_v2_timeout_vote_signing_bytes(&timeout).unwrap(),
    )
    .unwrap();
    assert_eq!((slot.height, slot.view), (9, 3));
}

#[test]
fn decoder_matches_every_legacy_runtime_encoder_variant() {
    use crate::consensus_artifacts::{
        block_certificate_vote_message, block_proposal_signature_message,
        block_timeout_vote_message,
    };
    use crate::storage_commit::BlockEvidence;
    let genesis = Genesis::new("guard-legacy");
    let root = "a".repeat(96);
    let fence: postfiat_types::FastPayVersionFenceV1 = serde_json::from_value(serde_json::json!({
        "schema":"postfiat-fastpay-version-fence-v1", "operation":"transfer",
        "origin":"ordered_recovery", "committee_epoch":1, "registry_root":root,
        "lock_id":root, "inputs":[{"id":"owned", "version":0}],
        "decision":"cancelled", "decided_at_height":8,
        "next_versions":[{"id":"owned", "version":1}]
    }))
    .unwrap();
    let effects = [fence];
    let check = |context: &[u8], message: Vec<u8>| {
        let slot = decode_slot(&public(), context, &message).unwrap();
        assert_eq!((slot.height, slot.view), (9, 3));
        assert_eq!(slot.chain_id, genesis.chain_id);
    };
    for variant in 0..4 {
        let evidence = BlockEvidence {
            height: 9,
            view: 3,
            parent_hash: &root,
            proposer: "validator-0",
            batch_kind: "transparent",
            batch_id: &root,
            state_root: &root,
            bridge_exit_root: (variant >= 2).then_some(root.as_str()),
            pftl_uniswap_receipt_root: (variant >= 3).then_some(root.as_str()),
            receipt_ids: &[],
            fastpay_pre_state_effects: if variant >= 1 { &effects } else { &[] },
        };
        for registry in ["", root.as_str()] {
            check(
                BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
                block_certificate_vote_message(&genesis, &evidence, "validator-0", true, registry)
                    .unwrap(),
            );
        }
        let proposal = BlockProposalFile {
            schema: "postfiat-block-proposal-v1".into(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            block_height: 9,
            view: 3,
            parent_hash: root.clone(),
            proposer: "validator-0".into(),
            batch_kind: "transparent".into(),
            batch_id: root.clone(),
            payload_hash: root.clone(),
            state_root: root.clone(),
            bridge_exit_root: evidence.bridge_exit_root.map(str::to_string),
            pftl_uniswap_receipt_root: evidence.pftl_uniswap_receipt_root.map(str::to_string),
            receipt_count: 0,
            receipt_ids: vec![],
            fastpay_pre_state_effects: evidence.fastpay_pre_state_effects.to_vec(),
            signature: None,
        };
        check(
            BLOCK_PROPOSAL_SIGNATURE_CONTEXT,
            block_proposal_signature_message(&proposal).unwrap(),
        );
    }
    for registry in ["", root.as_str()] {
        check(
            BLOCK_TIMEOUT_SIGNATURE_CONTEXT,
            block_timeout_vote_message(&genesis, 9, 3, &root, "validator-0", registry).unwrap(),
        );
    }
}
