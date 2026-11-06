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

/// In a certified transparent block the refused export follows the ordinary
/// rejected-transaction fee rule, as the release does
/// (`release_above_the_cap_in_a_block_burns_the_fee_only_after_fee_activation`):
/// before `rejected_tx_fee_activation_height` nothing changes; from it the fee
/// is burned and the sequence consumed, and no lane or other state changes.
#[test]
fn external_lane_export_in_a_block_burns_the_fee_only_after_fee_activation() {
    let chain = lane_chain("export-fee");
    let store = NodeStore::new(&chain.data_dir);
    let ledger = store.read_ledger().expect("ledger");
    let height = tip_height(&chain) + 1;
    let export = export_tx(&chain, "base-erc20", ACCOUNT_RESERVE + 1_000, &base_recipient());
    let payer = export.unsigned.source.clone();
    let mut batch = TransactionBatch::new("export-fee", Vec::new());
    batch.asset_transactions.push(export);
    let unactivated = store.read_governance().expect("governance");
    let mut activated = unactivated.clone();
    let template = activated.amendments[0].clone();
    activated.apply(GovernanceAmendment {
        amendment_id: "rejected-fee-activation".to_string(),
        kind: GOVERNANCE_KIND_REJECTED_TX_FEE_ACTIVATION_HEIGHT.to_string(),
        value: u32::try_from(height).expect("height"),
        ..template
    });
    for (governance, code, burned) in [
        (unactivated, "external_lane_export_not_supported", 0),
        (activated, "tec_external_lane_export_not_supported", 1_000),
    ] {
        let compatibility =
            asset_execution_compatibility_for_genesis_and_governance(&chain.genesis, &governance);
        let mut after = ledger.clone();
        let receipts =
            execute_transparent_batch(&chain.genesis, &governance, &mut after, &batch, height, compatibility);
        assert_eq!(
            (receipts[0].code.as_str(), receipts[0].fee_burned),
            (code, burned),
            "{receipts:?}"
        );
        let (was, now) = (ledger.account(&payer).expect("payer"), after.account(&payer).expect("payer"));
        assert_eq!(now.balance, was.balance - burned, "{code}");
        assert_eq!(now.sequence, was.sequence + u64::from(burned > 0), "{code}");
        // Only the payer's account moved; the lanes and everything else are
        // as before.
        assert_eq!(after.external_lanes, ledger.external_lanes, "{code}: lane changed");
        let mut expected = ledger.clone();
        let index = expected
            .accounts
            .iter()
            .position(|account| account.address == payer)
            .expect("payer index");
        expected.accounts[index] = now.clone();
        assert_eq!(after, expected, "{code}: state beyond the payer changed");
    }
    let _ = fs::remove_dir_all(chain.data_dir);
}

// ---- Exports behind `exports_enabled` (design sections 3 and 4) ----------

fn export_chain(label: &str) -> Chain {
    lane_chain_configured(label, 86_400, None, 4, |policy| policy.exports_enabled = true)
}

/// A lane chain with exports enabled after one release of `amount` atoms.
fn released_export_chain(label: &str, amount: u64) -> Chain {
    let chain = export_chain(label);
    let receipt = admit_and_apply(&chain, &release_tx(&chain, "base-erc20", 1, amount, 1), "release");
    assert!(receipt.accepted, "{receipt:?}");
    chain
}

fn export_page(data_dir: &Path, cursor: Option<u64>, limit: Option<usize>) -> ExternalLaneInfoRow {
    let report = external_lane_info(ExternalLaneInfoOptions {
        data_dir: data_dir.to_path_buf(),
        lane_id: Some("base-erc20".to_string()),
        release_cursor: None,
        release_limit: None,
        export_cursor: cursor,
        export_limit: limit,
    })
    .expect("lane info");
    report.lanes.into_iter().next().expect("lane")
}

#[test]
fn external_lane_export_with_no_releases_yet_is_refused_everywhere() {
    let chain = export_chain("export-unreleased");
    let tx = export_tx(&chain, "base-erc20", 1, &base_recipient());
    refused_at_every_entry_point(&chain, &tx, postfiat_types::EXTERNAL_LANE_EXPORT_ABOVE_RELEASED, "unreleased");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// A release then an export through mempool admission, batch building and
/// execution: the sender pays amount and fee, the lane takes the amount
/// back, and live + lanes' remainder + burns stays the genesis supply.
#[test]
fn external_lane_release_then_export_conserves_native_supply() {
    let released = ACCOUNT_RESERVE + 1_000;
    let chain = released_export_chain("export-conserves", released);
    let store = NodeStore::new(&chain.data_dir);
    let supply = |store: &NodeStore| {
        native_pft_live_total(&store.read_ledger().expect("ledger"), &store.read_shielded().expect("shielded"))
            .expect("live")
    };
    let sender = read_transfer_key_file(&chain.data_dir, None).expect("faucet key").address;
    let ledger_before = store.read_ledger().expect("ledger");
    let lane_remainder_before = lane_info(&chain.data_dir).remainder_atoms;
    let live_before = supply(&store);

    // One atom above released - exported is refused with nothing changed.
    let above = export_tx(&chain, "base-erc20", released + 1, &base_recipient());
    refused_at_every_entry_point(&chain, &above, postfiat_types::EXTERNAL_LANE_EXPORT_ABOVE_RELEASED, "above");

    // Exactly released - exported is accepted.
    let first = 400;
    let tx = export_tx(&chain, "base-erc20", first, &base_recipient());
    let receipt = admit_and_apply(&chain, &tx, "export-0");
    assert!(receipt.accepted, "{receipt:?}");
    let tx = export_tx(&chain, "base-erc20", released - first, &base_recipient());
    let second = admit_and_apply(&chain, &tx, "export-1");
    assert!(second.accepted, "{second:?}");
    let burned = receipt.fee_burned + second.fee_burned;

    let ledger = store.read_ledger().expect("ledger");
    assert_eq!(
        ledger.account(&sender).expect("sender").balance,
        ledger_before.account(&sender).expect("sender").balance - released - burned
    );
    let info = export_page(&chain.data_dir, None, None);
    assert_eq!((info.state.exported_atoms, info.state.next_export_nonce), (released, 2));
    assert_eq!(info.remainder_atoms, lane_remainder_before + released);
    assert_eq!(info.remainder_atoms, LANE_RESERVED_ATOMS);
    // The live total counts the lane remainder: only the fees left it.
    assert_eq!(supply(&store) + u128::from(burned), live_before);

    // The packets, newest first, with their digest and commitments.
    assert_eq!((info.export_count, info.pruned_export_count), (2, 0));
    assert_eq!(info.export_history_root, info.state.export_history_root);
    assert!(info.state.exports.is_empty(), "packets are paged, not in the state");
    assert_eq!(info.exports.len(), 2);
    let newest = &info.exports[0];
    assert_eq!(newest.packet.export_nonce, 1);
    assert_eq!(newest.packet.amount_atoms, released - first);
    assert_eq!(newest.packet.sender, sender);
    assert_eq!(newest.packet.base_recipient, base_recipient());
    assert_eq!(newest.packet.exported_at_height, tip_height(&chain));
    assert_eq!(newest.packet.tx_id, postfiat_execution::asset_transaction_tx_id(&tx));
    assert_eq!(
        newest.packet_digest,
        format!("0x{}", bytes_to_hex(&newest.packet.digest().expect("digest")))
    );
    assert_eq!(
        newest.withdrawal_id_commitment,
        format!(
            "0x{}",
            bytes_to_hex(&newest.packet.withdrawal_id_commitment(&chain.genesis.chain_id).expect("commitment"))
        )
    );
    assert_eq!(
        newest.burn_tx_commitment,
        format!("0x{}", bytes_to_hex(&newest.packet.burn_tx_commitment(&chain.genesis.chain_id)))
    );
    let page = export_page(&chain.data_dir, None, Some(1));
    assert_eq!(page.exports.len(), 1);
    assert_eq!(page.next_export_cursor, Some(1));
    let older = export_page(&chain.data_dir, page.next_export_cursor, Some(1));
    assert_eq!(older.exports[0].packet.export_nonce, 0);
    assert_eq!(older.next_export_cursor, None);

    // Nothing more to take back: one atom is refused.
    let empty = export_tx(&chain, "base-erc20", 1, &base_recipient());
    refused_at_every_entry_point(&chain, &empty, postfiat_types::EXTERNAL_LANE_EXPORT_ABOVE_RELEASED, "drained");

    // The per-block conservation check holds over the replay from genesis.
    verify_blocks(NodeOptions {
        data_dir: chain.data_dir.clone(),
    })
    .expect("replay from genesis through the release and the exports");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// Under the governed zero-cap halt an export is refused after the lane and
/// lock-address checks, and nothing changes.
#[test]
fn external_lane_export_is_halted_under_a_zero_active_cap() {
    let chain = released_export_chain("export-halted", ACCOUNT_RESERVE + 1_000);
    govern_release_cap(&chain, 0);
    // Mempool admission and batch building run under the governed cap; the
    // bare execution entry point knows only the genesis cap, so the halt is
    // checked at the two governed entry points with nothing changed.
    let store = NodeStore::new(&chain.data_dir);
    let ledger_before = store.read_ledger().expect("ledger");
    let mempool_before = store.read_mempool().expect("mempool");
    let tip_before = tip_height(&chain);
    let tx = export_tx(&chain, "base-erc20", 1, &base_recipient());
    refused_everywhere(&chain, &tx, postfiat_types::EXTERNAL_LANE_EXPORT_HALTED, "halted");
    // The lane and lock-address checks still come first.
    let to_lock = export_tx(&chain, "base-erc20", 1, LANE_LOCK);
    refused_everywhere(&chain, &to_lock, "external_lane_export_recipient_is_lock", "halted-lock");
    let unknown = export_tx(&chain, "base-erc21", 1, &base_recipient());
    refused_everywhere(&chain, &unknown, postfiat_types::EXTERNAL_LANE_NOT_CONFIGURED, "halted-lane");
    assert_eq!(store.read_ledger().expect("ledger"), ledger_before);
    assert_eq!(store.read_mempool().expect("mempool"), mempool_before);
    assert_eq!(tip_height(&chain), tip_before);
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// With the switch off, a released lane still refuses every export as the
/// skeleton did.
#[test]
fn external_lane_export_stays_refused_without_the_switch_after_a_release() {
    let chain = lane_chain("export-switch-off");
    let receipt = admit_and_apply(&chain, &release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 1_000, 1), "release");
    assert!(receipt.accepted, "{receipt:?}");
    let tx = export_tx(&chain, "base-erc20", 1, &base_recipient());
    refused_at_every_entry_point(&chain, &tx, "external_lane_export_not_supported", "switch-off");
    let info = export_page(&chain.data_dir, None, None);
    assert_eq!((info.state.exported_atoms, info.state.next_export_nonce, info.export_count), (0, 0, 0));
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The wallet signer runs the operation's `validate()` and nothing else of
/// the export rules: it refuses a bad shape and signs a valid one.
#[test]
fn external_lane_export_wallet_signer_runs_validate_only() {
    let chain = lane_chain("export-signer");
    let options = |amount: u64, recipient: String| WalletSignAssetTransactionOptions {
        key_file: chain.data_dir.join(FAUCET_KEY_FILE),
        chain_id: chain.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&chain.genesis),
        protocol_version: chain.genesis.protocol_version,
        fee: 1_000,
        sequence: 1,
        expected_source: None,
        operation: AssetTransactionOperation::ExternalLaneExportV1(postfiat_types::ExternalLaneExportOperationV1 {
            sender: read_transfer_key_file(&chain.data_dir, None).expect("faucet key").address,
            lane_id: "base-erc20".to_string(),
            amount_atoms: amount,
            base_recipient: recipient,
        }),
    };
    let error = wallet_sign_asset_transaction(options(0, base_recipient())).unwrap_err().to_string();
    assert!(error.contains("amount_atoms must be positive"), "{error}");
    let error = wallet_sign_asset_transaction(options(1, format!("0x{}", "00".repeat(20))))
        .unwrap_err()
        .to_string();
    assert!(error.contains("zero address"), "{error}");
    wallet_sign_asset_transaction(options(1, base_recipient())).expect("a valid export signs");
    let _ = fs::remove_dir_all(&chain.data_dir);
}
