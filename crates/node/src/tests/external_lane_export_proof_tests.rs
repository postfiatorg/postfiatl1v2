// Included in `genesis_external_lane_tests::release_path::export_path`: the
// lane export proof input and its statement (design note, "Base half, step 3").

use postfiat_pft_lane_proofs::verify_external_lane_export_witness_v1;

/// Admits `txs` and applies them in one consensus v2 block.
fn apply_in_one_block(chain: &Chain, txs: &[SignedAssetTransaction], label: &str) -> Vec<Receipt> {
    for tx in txs {
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: chain.data_dir.clone(),
            signed_asset_transaction_json: serde_json::to_string(tx).expect("tx json"),
        })
        .expect("mempool admits the export");
    }
    let batch_file = chain.data_dir.join(format!("{label}.batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 8,
    })
    .expect("mempool batch");
    let certificate_file = Some(consensus_v2_certificate(
        &chain.data_dir,
        &batch_file,
        label,
    ));
    apply_batch(ApplyBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file,
        certificate_file,
    })
    .expect("apply batch")
}

/// A release, then exports 0 and 1 in one block and export 2 in the next.
/// Returns the chain and the release block's id: the Base verifier's checkpoint.
fn proof_chain(label: &str) -> (Chain, String) {
    let chain = released_export_chain(label, ACCOUNT_RESERVE + 100_000);
    let prior = header_at(&chain, tip_height(&chain)).block_hash;
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let first = faucet_sequence(&chain);
    let recipient = base_recipient();
    let txs = [
        export_tx_at(&chain, "base-erc20", 10, &recipient, &faucet.address, first),
        export_tx_at(
            &chain,
            "base-erc20",
            20,
            &recipient,
            &faucet.address,
            first + 1,
        ),
    ];
    let receipts = apply_in_one_block(&chain, &txs, "proof-exports-1");
    assert!(
        receipts.len() == 2 && receipts.iter().all(|receipt| receipt.accepted),
        "{receipts:?}"
    );
    let tx = export_tx_at(
        &chain,
        "base-erc20",
        30,
        &recipient,
        &faucet.address,
        first + 2,
    );
    let receipts = apply_in_one_block(&chain, &[tx], "proof-exports-2");
    assert!(receipts[0].accepted, "{receipts:?}");
    (chain, prior)
}

fn proof_input(chain: &Chain, nonce: u64, prior: &str) -> io::Result<ExternalLaneExportProofInput> {
    external_lane_export_proof_input(ExternalLaneExportProofInputOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        export_nonce: Some(nonce),
        tx_id: None,
        prior_checkpoint_block_id: prior.to_string(),
    })
}

/// The binding's public values for `witness`, from the block's header and
/// commit only, without the witness check.
fn binding_values(
    witness: &postfiat_types::ExternalLaneExportProofWitnessV1,
) -> postfiat_types::PfUsdcEgressPublicValuesV1 {
    let header = &witness.block.header;
    let domain = &header
        .consensus_v2_commit
        .as_ref()
        .expect("commit")
        .proposal
        .domain;
    let lane = &witness.genesis.external_lanes.as_deref().expect("lanes")[0];
    let checkpoint = postfiat_types::ExternalLaneExportCheckpointV1 {
        pftl_chain_id: witness.genesis.chain_id.clone(),
        pftl_genesis_hash: genesis_hash(&witness.genesis),
        pftl_protocol_version: witness.genesis.protocol_version,
        prior_checkpoint_block_id: witness.prior_checkpoint_block_id.clone(),
        committee_epoch: domain.committee_epoch,
        committee_root: domain.committee_root.clone(),
        committee_transition_commitment: String::new(),
        finalized_block_height: header.height,
        finalized_block_view: header.view,
        finalized_block_id: header.block_hash.clone(),
        finalized_parent_block_id: header.parent_hash.clone(),
        finalized_state_root: header.state_root.clone(),
        export: postfiat_types::ExternalLaneExportInclusionV1 {
            block_id: header.block_hash.clone(),
            block_height: header.height,
            export_root: header.external_lane_export_root.clone().expect("root"),
            proof: witness.export_root_proof.clone(),
        },
    };
    postfiat_types::external_lane_export_egress_public_values(lane, &witness.packet, &checkpoint)
        .expect("binding")
}

fn refused(witness: &postfiat_types::ExternalLaneExportProofWitnessV1, label: &str) -> String {
    match verify_external_lane_export_witness_v1(witness) {
        Ok(_) => panic!("{label}: a changed witness was accepted"),
        Err(error) => error,
    }
}

#[test]
fn external_lane_export_proof_input_proves_each_export_and_refuses_changes() {
    let (chain, prior) = proof_chain("export-proof");
    let tip = tip_height(&chain);
    let mut witnesses = Vec::new();
    for (nonce, height, index, count) in [(0, tip - 1, 0, 2), (1, tip - 1, 1, 2), (2, tip, 0, 1)] {
        let input = proof_input(&chain, nonce, &prior).expect("proof input");
        let witness = &input.witness;
        assert_eq!(witness.packet.export_nonce, nonce);
        assert_eq!(witness.block.header.height, height);
        assert_eq!(
            (
                witness.export_root_proof.leaf_index,
                witness.export_root_proof.leaf_count
            ),
            (index, count)
        );
        assert!(!input.rebuilt_from_archive);
        let values = verify_external_lane_export_witness_v1(witness).expect("statement");
        assert_eq!(values, binding_values(witness), "nonce {nonce}");
        assert_eq!(values, input.expected_public_values);
        assert_eq!(
            bytes_to_hex(&values.canonical_bytes_without_commitment().expect("bytes")),
            input.expected_public_values_hex
        );
        // The guest's CBOR input decodes to the same witness and proves the same values.
        let decoded: postfiat_types::ExternalLaneExportProgramInputV1 =
            serde_cbor::from_slice(&input.program_input_cbor).expect("cbor");
        let postfiat_types::ExternalLaneExportProgramInputV1::Withdrawal(decoded) = decoded else {
            panic!("withdrawal input")
        };
        assert_eq!(
            verify_external_lane_export_witness_v1(&decoded).expect("decoded"),
            values
        );
        // For the SP1 execute run (tools/pft-lane-export-prover): the input files.
        if let Some(dir) = std::env::var_os("PFT_LANE_EXPORT_PROOF_INPUT_DIR") {
            write_external_lane_export_proof_input(
                &input,
                &Path::new(&dir).join(format!("nonce-{nonce}")),
            )
            .expect("write proof input");
        }
        witnesses.push(witness.clone());
    }
    // The same export by its transaction id.
    let by_tx = external_lane_export_proof_input(ExternalLaneExportProofInputOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        export_nonce: None,
        tx_id: Some(witnesses[1].packet.tx_id.clone()),
        prior_checkpoint_block_id: prior.clone(),
    })
    .expect("by tx id");
    assert_eq!(by_tx.witness, witnesses[1]);

    let base = witnesses[1].clone();
    let mutate = |edit: &dyn Fn(&mut postfiat_types::ExternalLaneExportProofWitnessV1)| {
        let mut witness = base.clone();
        edit(&mut witness);
        witness
    };
    let cases: Vec<(&str, postfiat_types::ExternalLaneExportProofWitnessV1, &str)> = vec![
        (
            "amount",
            mutate(&|w| w.packet.amount_atoms += 1),
            "another leaf",
        ),
        (
            "recipient",
            mutate(&|w| w.packet.base_recipient = format!("0x{}", "3d".repeat(20))),
            "another leaf",
        ),
        (
            "nonce",
            mutate(&|w| w.packet.export_nonce += 1),
            "another leaf",
        ),
        (
            "lane policy",
            mutate(&|w| {
                w.genesis.external_lanes.as_mut().expect("lanes")[0]
                    .policy
                    .token_address = format!("0x{}", "4e".repeat(20))
            }),
            "not the genesis the finality check proved",
        ),
        (
            "leaf index",
            mutate(&|w| w.export_root_proof.leaf_index = 0),
            "root mismatch",
        ),
        (
            "leaf count",
            mutate(&|w| w.export_root_proof.leaf_count = 3),
            "sibling depth mismatch",
        ),
        (
            "header root",
            mutate(&|w| {
                w.block.header.external_lane_export_root =
                    Some(postfiat_types::bridge_exit_empty_root_v1())
            }),
            "does not commit the header's export root",
        ),
        (
            "block id",
            mutate(&|w| w.block.header.block_hash = "ab".repeat(48)),
            "does not exactly bind block header",
        ),
        (
            "block state root",
            mutate(&|w| w.block.header.parent_hash = "cd".repeat(48)),
            "does not exactly bind block header",
        ),
        (
            "other lane",
            mutate(&|w| w.packet.lane_id = "base-other".to_string()),
            "lane",
        ),
        (
            "prior checkpoint",
            mutate(&|w| w.prior_checkpoint_block_id = witnesses[2].block.header.block_hash.clone()),
            "prior checkpoint",
        ),
        (
            "receipt list",
            mutate(&|w| w.block.receipt_ids.clear()),
            "not a receipt of the proven block",
        ),
        (
            "older export block",
            {
                let mut w = witnesses[0].clone();
                w.block = witnesses[2].block.clone();
                w.finality_ancestry = witnesses[2].finality_ancestry.clone();
                w
            },
            "older export block is not supported",
        ),
    ];
    for (label, witness, expected) in cases {
        let error = refused(&witness, label);
        assert!(error.contains(expected), "{label}: {error}");
    }
    // A finality certificate of another block over this block's fields.
    let mut other_commit = base.clone();
    other_commit.block.header.consensus_v2_commit =
        witnesses[2].block.header.consensus_v2_commit.clone();
    let error = refused(&other_commit, "other commit");
    assert!(error.contains("consensus-v2"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_proof_input_refusals() {
    let (chain, prior) = proof_chain("export-proof-refusals");
    let error = |nonce: Option<u64>, tx_id: Option<String>, lane: &str| {
        external_lane_export_proof_input(ExternalLaneExportProofInputOptions {
            data_dir: chain.data_dir.clone(),
            lane_id: lane.to_string(),
            export_nonce: nonce,
            tx_id,
            prior_checkpoint_block_id: prior.clone(),
        })
        .unwrap_err()
        .to_string()
    };
    assert!(error(Some(3), None, "base-erc20").contains("not final yet"));
    assert!(error(None, Some("ef".repeat(48)), "base-erc20").contains("unknown export"));
    assert!(error(Some(0), None, "no-such-lane").contains("genesis has no external lane"));
    let _ = fs::remove_dir_all(&chain.data_dir);

    // Exports off, and a chain without the export root.
    let chain = lane_chain_with_genesis("export-proof-off", 86_400, Some(2), 4, |_| {});
    let error = external_lane_export_proof_input(ExternalLaneExportProofInputOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        export_nonce: Some(0),
        tx_id: None,
        prior_checkpoint_block_id: "00".repeat(48),
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("has exports off"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
    let chain = lane_chain_with_genesis("export-proof-no-root", 86_400, Some(2), 4, |genesis| {
        genesis.external_lanes.as_mut().expect("lanes")[0]
            .policy
            .exports_enabled = true;
    });
    let error = external_lane_export_proof_input(ExternalLaneExportProofInputOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        export_nonce: Some(0),
        tx_id: None,
        prior_checkpoint_block_id: "00".repeat(48),
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("export root is not active"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The export's block packets pruned from the lane records: the command
/// rebuilds them by replaying the archive, and refuses without the archive.
#[test]
fn external_lane_export_proof_input_rebuilds_pruned_packets_from_the_archive() {
    let (chain, prior) = proof_chain("export-proof-pruned");
    let expected = proof_input(&chain, 1, &prior).expect("proof input");
    // Prune the lane's records as the retention rule would.
    let store = NodeStore::new(&chain.data_dir);
    let original = store.read_ledger().expect("ledger");
    let mut pruned = original.clone();
    let state = &mut pruned.external_lanes[0];
    state.pruned_export_count = state.exports.len() as u64;
    state.exports.clear();
    store.write_ledger(&pruned).expect("write pruned ledger");
    let rebuilt = proof_input(&chain, 1, &prior).expect("rebuilt from the archive");
    assert!(rebuilt.rebuilt_from_archive);
    assert_eq!(rebuilt.witness, expected.witness);
    assert_eq!(
        rebuilt.expected_public_values,
        expected.expected_public_values
    );
    // Without the archive the command refuses and names what is needed.
    store
        .write_batch_archive(&BatchArchive::empty())
        .expect("drop the archive");
    let error = proof_input(&chain, 1, &prior).unwrap_err().to_string();
    assert!(
        error.contains("pruned") && error.contains("batch archive"),
        "{error}"
    );
    store.write_ledger(&original).expect("restore ledger");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

// The lock and token `crates/pft-token-contracts/test/ExternalLaneWitnessRelease.t.sol`
// deploys on its local chain under Base chain id 8453, with their runtime code
// hashes; that test prints the new values when the bytecode changes.
const FOUNDRY_LOCK: &str = "0xf62849f9a0b5bf2913b396098f7c7019b51a820a";
const FOUNDRY_LOCK_CODE_HASH: &str =
    "0x7cb5b23356bacd0dffc245c03dc50ae95cf24bb18bca61abe66620a42d60790b";
const FOUNDRY_TOKEN: &str = "0x5615deb798bb3e4dfa0139dfa1b3d433cc23b72f";
const FOUNDRY_TOKEN_CODE_HASH: &str =
    "0x3bcc91381fd36c17047501a0cac6de10d01a4e744bd9e1a92574640df6a03e6b";
/// The proof input the lane proof crate's fixture test reads.
const WITNESS_RELEASE_INPUT: &str = "../pft_lane_proofs/testdata/witness-release-input.cbor";

/// One export on a lane that names the Foundry test's lock and token, through
/// the proof input and the statement. With EXTERNAL_LANE_WRITE_FIXTURE=1 it
/// records the input from which the Foundry fixture is derived.
#[test]
fn external_lane_export_witness_release_input_names_the_foundry_lock_and_token() {
    let chain = lane_chain_with_genesis(
        "export-proof-foundry",
        86_400,
        Some(EXPORT_ROOT_HEIGHT),
        4,
        |genesis| {
            let lane = &mut genesis.external_lanes.as_mut().expect("lanes")[0];
            lane.policy.exports_enabled = true;
            lane.policy.lock_contract_address = FOUNDRY_LOCK.to_string();
            lane.policy.lock_runtime_code_hash = FOUNDRY_LOCK_CODE_HASH.to_string();
            lane.policy.token_address = FOUNDRY_TOKEN.to_string();
            lane.policy.token_runtime_code_hash = FOUNDRY_TOKEN_CODE_HASH.to_string();
            genesis.external_lane_export_root_activation_height = Some(EXPORT_ROOT_HEIGHT);
        },
    );
    let receipt = admit_and_apply(
        &chain,
        &release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 2_000_000, 1),
        "release",
    );
    assert!(receipt.accepted, "{receipt:?}");
    let prior = header_at(&chain, tip_height(&chain)).block_hash;
    let receipt = admit_and_apply(
        &chain,
        &export_tx(&chain, "base-erc20", 1_000_000, &base_recipient()),
        "export",
    );
    assert!(receipt.accepted, "{receipt:?}");
    let input = proof_input(&chain, 0, &prior).expect("proof input");
    let values = verify_external_lane_export_witness_v1(&input.witness).expect("statement");
    assert_eq!(
        (values.vault_address.as_str(), values.token_address.as_str()),
        (FOUNDRY_LOCK, FOUNDRY_TOKEN)
    );
    // The chain's keys are fresh on every run, so the committed input is one
    // recorded run; `crates/pft_lane_proofs` derives the Foundry fixture from it.
    if std::env::var_os("EXTERNAL_LANE_WRITE_FIXTURE").is_some() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(WITNESS_RELEASE_INPUT);
        fs::write(path, &input.program_input_cbor).expect("write input");
    }
    let _ = fs::remove_dir_all(&chain.data_dir);
}
