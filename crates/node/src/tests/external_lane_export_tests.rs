// Included in `genesis_external_lane_tests::release_path::export_path`: the
// refused skeleton of `external_lane_export_v1`
// (docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md).

/// The lane fixture's lock contract (`release_tx_with`).
const LANE_LOCK: &str = "0x1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a";

fn base_recipient() -> String {
    format!("0x{}", "2c".repeat(20))
}

fn export_tx(chain: &Chain, lane_id: &str, amount: u64, recipient: &str) -> SignedAssetTransaction {
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let unsigned = UnsignedAssetTransaction {
        chain_id: chain.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&chain.genesis),
        protocol_version: chain.genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: postfiat_types::EXTERNAL_LANE_EXPORT_TRANSACTION_KIND_V1.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: faucet.address.clone(),
        fee: 1_000,
        sequence: faucet_sequence(chain),
        operation: AssetTransactionOperation::ExternalLaneExportV1(postfiat_types::ExternalLaneExportOperationV1 {
            sender: faucet.address.clone(),
            lane_id: lane_id.to_string(),
            amount_atoms: amount,
            base_recipient: recipient.to_string(),
        }),
    };
    let private_key = hex_to_bytes(&faucet.private_key_hex).expect("key bytes");
    let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
    SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: faucet.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

/// Refused with `code` at mempool admission, batch building and execution,
/// with the ledger, the mempool and the chain tip unchanged.
fn refused_at_every_entry_point(chain: &Chain, tx: &SignedAssetTransaction, code: &str, label: &str) {
    let store = NodeStore::new(&chain.data_dir);
    let ledger_before = store.read_ledger().expect("ledger");
    let mempool_before = store.read_mempool().expect("mempool");
    let tip_before = crate::status(NodeOptions {
        data_dir: chain.data_dir.clone(),
    })
    .expect("status");
    refused_everywhere(chain, tx, code, label);
    let mut ledger = ledger_before.clone();
    let receipt = postfiat_execution::execute_asset_transaction(
        &chain.genesis,
        &mut ledger,
        tx,
        tip_before.block_height + 1,
    );
    assert!(!receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.code, code, "{receipt:?}");
    assert_eq!(ledger, ledger_before, "execution changed the ledger");
    assert_eq!(store.read_ledger().expect("ledger"), ledger_before);
    assert_eq!(store.read_mempool().expect("mempool"), mempool_before);
    let tip_after = crate::status(NodeOptions {
        data_dir: chain.data_dir.clone(),
    })
    .expect("status");
    assert_eq!(
        (tip_after.block_height, tip_after.state_root),
        (tip_before.block_height, tip_before.state_root)
    );
}

#[test]
fn external_lane_export_is_refused_until_it_is_built() {
    let chain = lane_chain("export-refused");
    let tx = export_tx(&chain, "base-erc20", ACCOUNT_RESERVE + 1_000, &base_recipient());
    refused_at_every_entry_point(&chain, &tx, "external_lane_export_not_supported", "export");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_refuses_an_unknown_lane_and_the_lock_as_recipient() {
    let chain = lane_chain("export-lane-and-lock");
    let unknown = export_tx(&chain, "base-erc21", ACCOUNT_RESERVE + 1_000, &base_recipient());
    refused_at_every_entry_point(
        &chain,
        &unknown,
        postfiat_types::EXTERNAL_LANE_NOT_CONFIGURED,
        "unknown-lane",
    );
    let to_lock = export_tx(&chain, "base-erc20", ACCOUNT_RESERVE + 1_000, LANE_LOCK);
    refused_at_every_entry_point(
        &chain,
        &to_lock,
        "external_lane_export_recipient_is_lock",
        "to-lock",
    );
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_shape_refusals_reach_mempool_admission() {
    let chain = lane_chain("export-shape");
    let store = NodeStore::new(&chain.data_dir);
    let sender = read_transfer_key_file(&chain.data_dir, None).expect("faucet key").address;
    let sequence = |ledger: &LedgerState| ledger.account(&sender).expect("sender account").sequence;
    let tip = || {
        let status = crate::status(NodeOptions {
            data_dir: chain.data_dir.clone(),
        })
        .expect("status");
        (status.block_height, status.state_root)
    };
    let ledger_before = store.read_ledger().expect("ledger");
    let mempool_before = store.read_mempool().expect("mempool");
    let tip_before = tip();
    for (label, tx, expected) in [
        (
            "zero-amount",
            export_tx(&chain, "base-erc20", 0, &base_recipient()),
            "amount_atoms must be positive",
        ),
        (
            "zero-recipient",
            export_tx(&chain, "base-erc20", 1_000, &format!("0x{}", "00".repeat(20))),
            "zero address",
        ),
        (
            "bad-recipient",
            export_tx(&chain, "base-erc20", 1_000, "0x2c2c"),
            "base_recipient",
        ),
    ] {
        let error = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: chain.data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&tx).expect("tx json"),
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(expected), "{label}: {error}");
    }
    // Nothing changed: the ledger, the sender's sequence, the mempool and
    // the tip are as before the refused submissions.
    let ledger_after = store.read_ledger().expect("ledger");
    assert_eq!(sequence(&ledger_after), sequence(&ledger_before));
    assert_eq!(ledger_after, ledger_before);
    assert_eq!(store.read_mempool().expect("mempool"), mempool_before);
    assert_eq!(tip(), tip_before);
    let _ = fs::remove_dir_all(&chain.data_dir);
}
