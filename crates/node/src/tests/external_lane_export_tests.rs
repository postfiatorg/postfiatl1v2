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
    export_tx_from(chain, lane_id, amount, recipient, &faucet.address)
}

/// `export_tx` with `sender` in the operation; the faucet still signs.
fn export_tx_from(
    chain: &Chain,
    lane_id: &str,
    amount: u64,
    recipient: &str,
    sender: &str,
) -> SignedAssetTransaction {
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
            sender: sender.to_string(),
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
    // bare execution entry point knows only the genesis cap, so it fails
    // closed (below), and the halt is checked at the two governed entry
    // points with nothing changed.
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

    // The bare library entry sees only the genesis cap: it refuses the export
    // instead of applying it past the halt.
    let mut ledger = ledger_before.clone();
    let receipt = postfiat_execution::execute_asset_transaction(&chain.genesis, &mut ledger, &tx, tip_before + 1);
    assert!(!receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.code, postfiat_types::EXTERNAL_LANE_EXPORT_NEEDS_GOVERNED_CAPS, "{receipt:?}");
    assert_eq!(ledger, ledger_before, "the bare entry changed the ledger");

    // A hand-assembled block carrying the export, executed as certified-block
    // execution and replay do (governed compatibility), refuses it.
    let governance = store.read_governance().expect("governance");
    let mut batch = TransactionBatch::new("export-halted-block", Vec::new());
    batch.asset_transactions.push(tx.clone());
    let compatibility = asset_execution_compatibility_for_genesis_and_governance(&chain.genesis, &governance);
    let mut after = ledger_before.clone();
    let receipts =
        execute_transparent_batch(&chain.genesis, &governance, &mut after, &batch, tip_before + 1, compatibility);
    assert_eq!(receipts[0].code, postfiat_types::EXTERNAL_LANE_EXPORT_HALTED, "{receipts:?}");
    assert!(!receipts[0].accepted, "{receipts:?}");
    assert_eq!(after.external_lanes, ledger_before.external_lanes, "the block moved the lane");
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

/// An operation sender other than the transaction source never reaches the
/// export rules: admission and execution refuse the envelope on a disabled
/// and an enabled lane alike, as before the switch existed. The rules' own
/// order (switch, then sender) is tested in `external_lane_release.rs`.
#[test]
fn external_lane_export_sender_mismatch_is_refused_as_an_envelope_on_either_lane() {
    for (chain, code) in [
        (lane_chain("export-sender-off"), "bad_asset_transaction_envelope"),
        (export_chain("export-sender-on"), "bad_asset_transaction_envelope"),
    ] {
        let tx = export_tx_from(&chain, "base-erc20", 1, &base_recipient(), &chain.recipient);
        let store = NodeStore::new(&chain.data_dir);
        let ledger_before = store.read_ledger().expect("ledger");
        let error = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: chain.data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&tx).expect("tx json"),
            },
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("source is not authorized by operation fields"), "{error}");
        let height = crate::status(NodeOptions {
            data_dir: chain.data_dir.clone(),
        })
        .expect("status")
        .block_height;
        let mut ledger = ledger_before.clone();
        let receipt =
            postfiat_execution::execute_asset_transaction(&chain.genesis, &mut ledger, &tx, height + 1);
        assert!(!receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, code, "{receipt:?}");
        assert_eq!(ledger, ledger_before, "execution changed the ledger");
        assert_eq!(store.read_ledger().expect("ledger"), ledger_before);
        let _ = fs::remove_dir_all(&chain.data_dir);
    }
}

/// The export page limit is refused before the lanes are walked, so an
/// unknown lane id does not turn a bad limit into an empty success.
#[test]
fn external_lane_info_refuses_a_bad_export_limit_for_an_unknown_lane() {
    let chain = export_chain("export-limit-unknown-lane");
    for limit in [0, EXTERNAL_LANE_RELEASE_PAGE_SIZE + 1] {
        let error = external_lane_info(ExternalLaneInfoOptions {
            data_dir: chain.data_dir.clone(),
            lane_id: Some("base-erc21".to_string()),
            release_cursor: None,
            release_limit: None,
            export_cursor: None,
            export_limit: Some(limit),
        })
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("export limit"), "{error}");
    }
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The external-lane provenance flags live in the in-memory compatibility
/// only: neither it nor the flags implement `Serialize`, so nothing hashed or
/// stored can carry them. Checked at compile time.
#[test]
fn external_lane_governed_inputs_are_never_serialized() {
    trait NotSerialize {
        const SERIALIZE: bool = false;
    }
    struct Probe<T>(std::marker::PhantomData<T>);
    impl<T> NotSerialize for Probe<T> {}
    #[allow(dead_code)]
    impl<T: serde::Serialize> Probe<T> {
        const SERIALIZE: bool = true;
    }
    const { assert!(!Probe::<postfiat_execution::AssetExecutionCompatibility>::SERIALIZE) };
    const { assert!(!Probe::<postfiat_execution::ExternalLaneGovernedInputs>::SERIALIZE) };
    // The probe does see a serializable type.
    const { assert!(Probe::<postfiat_types::ExternalLaneState>::SERIALIZE) };
}

// ---- The holder's commands: `external-lane-export` and its status read ----

fn export_command(
    chain: &Chain,
    lane_id: &str,
    amount: u64,
    recipient: &str,
    dry_run: bool,
) -> std::io::Result<crate::ExternalLaneExportReport> {
    export_command_with_key(chain, chain.data_dir.join(FAUCET_KEY_FILE), lane_id, amount, recipient, dry_run)
}

fn export_command_with_key(
    chain: &Chain,
    key_file: PathBuf,
    lane_id: &str,
    amount: u64,
    recipient: &str,
    dry_run: bool,
) -> std::io::Result<crate::ExternalLaneExportReport> {
    crate::external_lane_export(crate::ExternalLaneExportOptions {
        data_dir: chain.data_dir.clone(),
        key_file,
        lane_id: lane_id.to_string(),
        amount_atoms: amount,
        base_recipient: recipient.to_string(),
        dry_run,
    })
}

fn export_status(chain: &Chain, selector: crate::ExternalLaneExportSelector) -> crate::ExternalLaneExportStatusReport {
    export_status_with(chain, selector, false, None).expect("export status")
}

fn export_status_with(
    chain: &Chain,
    selector: crate::ExternalLaneExportSelector,
    audit: bool,
    sender: Option<&str>,
) -> std::io::Result<crate::ExternalLaneExportStatusReport> {
    crate::external_lane_export_status(crate::ExternalLaneExportStatusOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        selector,
        audit,
        sender: sender.map(str::to_string),
    })
}

/// The command refuses with `code` before signing: no report, and the
/// mempool, the ledger and the tip are unchanged.
fn command_refused_before_signing(
    chain: &Chain,
    key_file: PathBuf,
    lane_id: &str,
    amount: u64,
    recipient: &str,
    code: &str,
) -> String {
    let store = NodeStore::new(&chain.data_dir);
    let ledger_before = store.read_ledger().expect("ledger");
    let mempool_before = store.read_mempool().expect("mempool");
    let tip_before = tip_height(chain);
    for dry_run in [true, false] {
        let error = export_command_with_key(chain, key_file.clone(), lane_id, amount, recipient, dry_run)
            .expect_err("refused")
            .to_string();
        assert!(error.starts_with(&format!("refused before signing: {code}: ")), "{error}");
    }
    assert_eq!(store.read_mempool().expect("mempool"), mempool_before, "something was submitted");
    assert_eq!(store.read_ledger().expect("ledger"), ledger_before);
    assert_eq!(tip_height(chain), tip_before);
    export_command_with_key(chain, key_file, lane_id, amount, recipient, true)
        .expect_err("refused")
        .to_string()
}

fn faucet_refused(chain: &Chain, lane_id: &str, amount: u64, recipient: &str, code: &str) -> String {
    command_refused_before_signing(chain, chain.data_dir.join(FAUCET_KEY_FILE), lane_id, amount, recipient, code)
}

/// Proposes a batch from the mempool and applies it.
fn apply_mempool(chain: &Chain, label: &str) -> Receipt {
    let batch_file = chain.data_dir.join(format!("{label}.batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 8,
    })
    .expect("mempool batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply batch");
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    receipts.into_iter().next().expect("receipt")
}

/// A release, then the export through the command (a dry run first, which
/// submits nothing), then the status read by tx id and by nonce.
#[test]
fn external_lane_export_command_runs_end_to_end_and_the_status_read_shows_the_packet() {
    let chain = released_export_chain("export-command", ACCOUNT_RESERVE + 1_000);
    let store = NodeStore::new(&chain.data_dir);
    let mempool_before = store.read_mempool().expect("mempool");

    let dry = export_command(&chain, "base-erc20", 400, &base_recipient(), true).expect("dry run");
    assert!(dry.dry_run && !dry.submitted, "{dry:?}");
    assert_eq!(dry.tx_id, postfiat_execution::asset_transaction_tx_id(&dry.signed_asset_transaction));
    assert_eq!(store.read_mempool().expect("mempool"), mempool_before, "the dry run submitted");
    assert_eq!(export_page(&chain.data_dir, None, None).export_count, 0);

    let report = export_command(&chain, "base-erc20", 400, &base_recipient(), false).expect("export");
    assert!(report.submitted && !report.dry_run, "{report:?}");
    assert_eq!(report.signed_asset_transaction.unsigned.fee, report.fee);
    let receipt = apply_mempool(&chain, "export-command");
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.tx_id, report.tx_id);

    let status = export_status(&chain, crate::ExternalLaneExportSelector::TxId(report.tx_id.clone()));
    assert_eq!(status, export_status(&chain, crate::ExternalLaneExportSelector::Nonce(0)));
    let page = export_page(&chain.data_dir, None, None);
    assert_eq!(
        (status.export_nonce, status.amount_atoms, status.base_recipient.as_str(), status.exported_at_height),
        (0, 400, base_recipient().as_str(), tip_height(&chain))
    );
    assert_eq!(status.packet_digest, page.exports[0].packet_digest);
    assert!(status.in_committed_block, "{status:?}");
    assert_eq!(status.block_log_verified, None, "no audit, no claim");
    let audited = export_status_with(&chain, crate::ExternalLaneExportSelector::Nonce(0), true, None).expect("audit");
    assert_eq!((audited.in_committed_block, audited.block_log_verified), (true, Some(true)));
    assert_eq!((status.lane_exported_atoms, status.lane_export_count), (400, 1));
    assert_eq!(page.state.exported_atoms, 400);
    assert!(status.base_payout.contains("relay that does not exist yet"), "{status:?}");

    let missing = export_status_with(&chain, crate::ExternalLaneExportSelector::Nonce(1), false, None)
        .unwrap_err()
        .to_string();
    assert!(missing.contains("no stored export"), "{missing}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_an_unknown_lane_before_signing() {
    let chain = released_export_chain("export-command-lane", ACCOUNT_RESERVE + 1_000);
    let error = faucet_refused(&chain, "base-erc21", 1, &base_recipient(), postfiat_types::EXTERNAL_LANE_NOT_CONFIGURED);
    assert!(error.contains("`base-erc21` does not exist"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_a_lane_with_exports_off_before_signing() {
    let chain = lane_chain("export-command-off");
    let receipt = admit_and_apply(&chain, &release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 1_000, 1), "release");
    assert!(receipt.accepted, "{receipt:?}");
    let error = faucet_refused(&chain, "base-erc20", 1, &base_recipient(), "external_lane_export_not_supported");
    assert!(error.contains("exports are switched off"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_a_halted_lane_before_signing() {
    let chain = released_export_chain("export-command-halt", ACCOUNT_RESERVE + 1_000);
    govern_release_cap(&chain, 0);
    let error = faucet_refused(&chain, "base-erc20", 1, &base_recipient(), postfiat_types::EXTERNAL_LANE_EXPORT_HALTED);
    assert!(error.contains("active release cap is zero"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_a_zero_amount_before_signing() {
    let chain = released_export_chain("export-command-zero", ACCOUNT_RESERVE + 1_000);
    let error = faucet_refused(&chain, "base-erc20", 0, &base_recipient(), "external_lane_export_amount_zero");
    assert!(error.contains("at least one atom"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_an_amount_above_released_minus_exported_before_signing() {
    let released = ACCOUNT_RESERVE + 1_000;
    let chain = released_export_chain("export-command-above", released);
    let code = postfiat_types::EXTERNAL_LANE_EXPORT_ABOVE_RELEASED;
    let error = faucet_refused(&chain, "base-erc20", released + 1, &base_recipient(), code);
    assert!(error.contains(&format!("released {released} minus exported 0")), "{error}");
    export_command(&chain, "base-erc20", 400, &base_recipient(), false).expect("export");
    assert!(apply_mempool(&chain, "export-command-above").accepted);
    let error = faucet_refused(&chain, "base-erc20", released - 399, &base_recipient(), code);
    assert!(error.contains(&format!("minus exported 400 is {}", released - 400)), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_a_malformed_or_zero_recipient_before_signing() {
    let chain = released_export_chain("export-command-shape", ACCOUNT_RESERVE + 1_000);
    for recipient in ["0x12", "2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c", &format!("0x{}", "2C".repeat(20))] {
        faucet_refused(&chain, "base-erc20", 1, recipient, "external_lane_export_recipient_invalid");
    }
    let error = faucet_refused(
        &chain,
        "base-erc20",
        1,
        &format!("0x{}", "00".repeat(20)),
        "external_lane_export_recipient_invalid",
    );
    assert!(error.contains("zero address"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_the_lock_as_recipient_before_signing() {
    let chain = released_export_chain("export-command-lock", ACCOUNT_RESERVE + 1_000);
    let error = faucet_refused(&chain, "base-erc20", 1, LANE_LOCK, "external_lane_export_recipient_is_lock");
    assert!(error.contains("lock contract"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_refuses_the_token_as_recipient_before_signing() {
    let chain = released_export_chain("export-command-token", ACCOUNT_RESERVE + 1_000);
    let token = export_page(&chain.data_dir, None, None).config.policy.token_address;
    let error = faucet_refused(&chain, "base-erc20", 1, &token, crate::EXTERNAL_LANE_EXPORT_RECIPIENT_IS_TOKEN);
    assert!(error.contains("token contract"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The release recipient holds exactly the released atoms: exporting 1,000
/// of them leaves less than the fee and the reserve.
#[test]
fn external_lane_export_command_refuses_a_balance_short_of_amount_fee_and_reserve_before_signing() {
    let chain = released_export_chain("export-command-balance", ACCOUNT_RESERVE + 1_000);
    let key_pair = ml_dsa_65_keygen_from_seed(&[0x99; 32]);
    let key_file = chain.data_dir.join("recipient-key.json");
    write_key_file(
        &key_file,
        &DevKeyFile {
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            address: address_from_public_key(&key_pair.public_key),
            public_key_hex: bytes_to_hex(&key_pair.public_key),
            private_key_hex: bytes_to_hex(&key_pair.private_key),
        },
    )
    .expect("recipient key");
    let error = command_refused_before_signing(&chain, key_file, "base-erc20", 1_000, &base_recipient(), "insufficient_funds");
    assert!(error.contains("cannot cover the amount 1000"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// Pending exports count before signing: the lane's committed
/// `released - exported` is not what admission checks, the ledger after the
/// executable mempool is. Two and three pending exports that exhaust the
/// lane refuse a fourth before the key signs; an export that still fits
/// after the pending ones is signed and admitted.
#[test]
fn external_lane_export_command_counts_pending_exports_before_signing() {
    let released = ACCOUNT_RESERVE + 1_000;
    let code = postfiat_types::EXTERNAL_LANE_EXPORT_ABOVE_RELEASED;
    let store_of = |chain: &Chain| NodeStore::new(&chain.data_dir);

    let chain = released_export_chain("export-command-pending-2", released);
    for amount in [600, released - 600] {
        export_command(&chain, "base-erc20", amount, &base_recipient(), false).expect("pending export");
    }
    assert_eq!(store_of(&chain).read_mempool().expect("mempool").pending_asset_transactions.len(), 2);
    faucet_refused(&chain, "base-erc20", 1, &base_recipient(), code);
    let _ = fs::remove_dir_all(&chain.data_dir);

    let chain = released_export_chain("export-command-pending-3", released);
    export_command(&chain, "base-erc20", 600, &base_recipient(), false).expect("pending export");
    export_command(&chain, "base-erc20", 1, &base_recipient(), false).expect("fits after one pending");
    // Exactly what is left after the two pending exports.
    export_command(&chain, "base-erc20", released - 601, &base_recipient(), false)
        .expect("fits after two pending");
    faucet_refused(&chain, "base-erc20", 1, &base_recipient(), code);
    let mempool = store_of(&chain).read_mempool().expect("mempool");
    assert_eq!(mempool.pending_asset_transactions.len(), 3);
    let batch_file = chain.data_dir.join("pending.batch.json");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 8,
    })
    .expect("mempool batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply batch");
    assert!(receipts.len() == 3 && receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    assert_eq!(export_page(&chain.data_dir, None, None).state.exported_atoms, released);
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// A pending release counts before signing too: with `released` committed
/// and another `released` pending, an export above the committed amount but
/// within both fits on admission's ledger, so it is signed and admitted.
#[test]
fn external_lane_export_command_counts_a_pending_release_before_signing() {
    let released = ACCOUNT_RESERVE + 1_000;
    let chain = released_export_chain("export-command-pending-release", released);
    submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
        data_dir: chain.data_dir.clone(),
        signed_asset_transaction_json: serde_json::to_string(&release_tx(&chain, "base-erc20", 2, released, 2))
            .expect("release json"),
    })
    .expect("mempool admits the second release");
    let amount = released + 500;
    let dry_run = export_command(&chain, "base-erc20", amount, &base_recipient(), true).expect("dry run");
    assert!(dry_run.dry_run && !dry_run.submitted);
    let report = export_command(&chain, "base-erc20", amount, &base_recipient(), false).expect("export");
    let mempool = NodeStore::new(&chain.data_dir).read_mempool().expect("mempool");
    assert_eq!(mempool.pending_asset_transactions.len(), 2);
    let error = faucet_refused(
        &chain,
        "base-erc20",
        released - 499,
        &base_recipient(),
        postfiat_types::EXTERNAL_LANE_EXPORT_ABOVE_RELEASED,
    );
    let left = released - 500;
    assert!(
        error.contains(&format!("released {} minus exported {amount} is {left}", 2 * released))
            && error.contains("after the executable mempool"),
        "{error}"
    );
    let batch_file = chain.data_dir.join("pending-release.batch.json");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 8,
    })
    .expect("mempool batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply batch");
    assert!(receipts.len() == 2 && receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    assert_eq!(receipts[1].tx_id, report.tx_id);
    let row = export_page(&chain.data_dir, None, None);
    assert_eq!((row.state.released_atoms, row.state.exported_atoms), (2 * released, amount));
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn external_lane_export_command_amount_is_digits_only() {
    assert_eq!(crate::parse_export_amount_atoms("1500"), Ok(1_500));
    for text in ["+1500", "-1", " 1", "1_500", "", "18446744073709551616"] {
        let error = crate::parse_export_amount_atoms(text).expect_err(text);
        assert!(error.contains("--amount-atoms"), "{error}");
    }
}

/// The export nonce is the lane's, not the sender's: `--sender` refuses a
/// packet another sender's export holds.
#[test]
fn external_lane_export_command_status_with_a_sender_refuses_another_senders_packet() {
    let chain = released_export_chain("export-command-sender", ACCOUNT_RESERVE + 1_000);
    let report = export_command(&chain, "base-erc20", 400, &base_recipient(), false).expect("export");
    assert!(apply_mempool(&chain, "export-command-sender").accepted);
    let nonce = crate::ExternalLaneExportSelector::Nonce(0);
    let mine = export_status_with(&chain, nonce.clone(), false, Some(&report.sender)).expect("own packet");
    assert_eq!(mine.sender, report.sender);
    let other = "pf0000000000000000000000000000000000000000";
    let error = export_status_with(&chain, nonce, false, Some(other)).unwrap_err().to_string();
    assert!(error.starts_with(crate::EXTERNAL_LANE_EXPORT_OTHER_SENDER), "{error}");
    assert!(error.contains("the lane's, not the sender's"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}
