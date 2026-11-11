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
    export_tx_at(chain, lane_id, amount, recipient, sender, faucet_sequence(chain))
}

/// `export_tx_from` at an explicit faucet `sequence`.
fn export_tx_at(
    chain: &Chain,
    lane_id: &str,
    amount: u64,
    recipient: &str,
    sender: &str,
    sequence: u64,
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
        sequence,
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
    let to_token = export_tx(&chain, "base-erc20", ACCOUNT_RESERVE + 1_000, &format!("0x{}", "2b".repeat(20)));
    refused_at_every_entry_point(
        &chain,
        &to_token,
        postfiat_types::EXTERNAL_LANE_EXPORT_RECIPIENT_IS_TOKEN,
        "to-token",
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

/// Consensus v2 and the export root start after the committee bootstrap.
const EXPORT_ROOT_HEIGHT: u64 = 2;

/// Exports enabled, with consensus v2 and the export root from [`EXPORT_ROOT_HEIGHT`].
fn export_chain(label: &str) -> Chain {
    export_chain_from(label, EXPORT_ROOT_HEIGHT)
}

/// Exports enabled, with consensus v2 and the export root from `height`.
fn export_chain_from(label: &str, height: u64) -> Chain {
    lane_chain_with_genesis(label, 86_400, Some(height), 4, |genesis| {
        genesis.external_lanes.as_mut().expect("lanes")[0].policy.exports_enabled = true;
        genesis.external_lane_export_root_activation_height = Some(height);
    })
}

/// A lane chain with exports enabled after one release of `amount` atoms.
fn released_export_chain(label: &str, amount: u64) -> Chain {
    released_export_chain_from(label, amount, EXPORT_ROOT_HEIGHT)
}

fn released_export_chain_from(label: &str, amount: u64, height: u64) -> Chain {
    let chain = export_chain_from(label, height);
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
    // The governance round is a legacy block; consensus v2 and the export root
    // start at the export's height.
    let chain = released_export_chain_from("export-halted", ACCOUNT_RESERVE + 1_000, 4);
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
    // A dry run opens no key file: the sender comes from `--sender`.
    let sender = dry_run.then(|| crate::read_key_file(&key_file).expect("key").address);
    export_command_as(chain, key_file, sender, lane_id, amount, recipient, dry_run)
}

fn export_command_as(
    chain: &Chain,
    key_file: PathBuf,
    sender: Option<String>,
    lane_id: &str,
    amount: u64,
    recipient: &str,
    dry_run: bool,
) -> std::io::Result<crate::ExternalLaneExportReport> {
    crate::external_lane_export(crate::ExternalLaneExportOptions {
        data_dir: chain.data_dir.clone(),
        key_file,
        sender,
        lane_id: lane_id.to_string(),
        amount_atoms: amount,
        base_recipient: recipient.to_string(),
        dry_run,
        max_fee_atoms: crate::EXTERNAL_LANE_EXPORT_DEFAULT_MAX_FEE_ATOMS,
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
    let receipts = apply_mempool_batch(chain, label);
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    receipts.into_iter().next().expect("receipt")
}

/// Proposes a batch from the mempool and applies it, with a consensus v2
/// certificate once consensus v2 is active.
fn apply_mempool_batch(chain: &Chain, label: &str) -> Vec<Receipt> {
    let store = NodeStore::new(&chain.data_dir);
    let next = next_block_height_from_chain_tip(&store, &chain.genesis).expect("next height");
    let batch_file = chain.data_dir.join(format!("{label}.batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 8,
    })
    .expect("mempool batch");
    let certificate_file = consensus_v2_active_at(&chain.genesis, next)
        .then(|| consensus_v2_certificate(&chain.data_dir, &batch_file, label));
    apply_batch(ApplyBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file,
        certificate_file,
    })
    .expect("apply batch")
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
    // A dry run signs nothing: no signed transaction, no tx id; the quote
    // carries the unsigned fields.
    assert_eq!((&dry.signed_asset_transaction, &dry.tx_id), (&None, &None), "{dry:?}");
    let quote = dry.quote.as_ref().expect("the dry run prints the quote");
    assert_eq!((quote.sequence, quote.minimum_fee, quote.source.as_str()), (dry.sequence, dry.fee, dry.sender.as_str()));
    assert!(
        matches!(&quote.operation, AssetTransactionOperation::ExternalLaneExportV1(export)
            if export.amount_atoms == 400 && export.base_recipient == base_recipient()),
        "{quote:?}"
    );
    assert_eq!(store.read_mempool().expect("mempool"), mempool_before, "the dry run submitted");
    // The key file is never read in a dry run: a path that does not exist
    // still gives a clean dry run.
    let missing_key = chain.data_dir.join("no-such-key.json");
    let keyless = export_command_as(&chain, missing_key.clone(), Some(dry.sender.clone()), "base-erc20", 400, &base_recipient(), true)
        .expect("a dry run never opens the key file");
    assert_eq!(keyless, dry);
    let error = export_command_as(&chain, missing_key, None, "base-erc20", 400, &base_recipient(), true)
        .unwrap_err()
        .to_string();
    assert!(error.contains("--sender"), "{error}");
    assert_eq!(export_page(&chain.data_dir, None, None).export_count, 0);

    let report = export_command(&chain, "base-erc20", 400, &base_recipient(), false).expect("export");
    assert!(report.submitted && !report.dry_run && report.quote.is_none(), "{report:?}");
    let signed = report.signed_asset_transaction.as_ref().expect("signed");
    assert_eq!(signed.unsigned.fee, report.fee);
    let tx_id = report.tx_id.clone().expect("tx id");
    assert_eq!(tx_id, postfiat_execution::asset_transaction_tx_id(signed));
    let receipt = apply_mempool(&chain, "export-command");
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.tx_id, tx_id);

    let status = export_status(&chain, crate::ExternalLaneExportSelector::TxId(tx_id));
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
    assert!(
        status.base_payout.contains("pays out on Base only once a release relay proves this packet")
            && !status.base_payout.contains("does not exist"),
        "{status:?}"
    );

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
    // The governance round is a legacy block, as in the halt test above.
    let chain = released_export_chain_from("export-command-halt", ACCOUNT_RESERVE + 1_000, 4);
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
    // A malformed sender (a dry run's `--sender`) has its own code, not the
    // recipient's.
    let error = export_command_as(
        &chain,
        chain.data_dir.join("no-such-key.json"),
        Some("not-an-address".to_string()),
        "base-erc20",
        1,
        &base_recipient(),
        true,
    )
    .unwrap_err()
    .to_string();
    assert!(error.starts_with("refused before signing: external_lane_export_sender_invalid: "), "{error}");
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
    let error = faucet_refused(&chain, "base-erc20", 1, &token, postfiat_types::EXTERNAL_LANE_EXPORT_RECIPIENT_IS_TOKEN);
    assert!(error.contains("token contract"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The release recipient holds exactly the released atoms: exporting 1,000
/// of them leaves less than the fee and the reserve.
#[test]
fn external_lane_export_command_refuses_a_balance_short_of_amount_fee_and_reserve_before_signing() {
    let chain = released_export_chain("export-command-balance", ACCOUNT_RESERVE + 1_000);
    let key_file = release_recipient_key_file(&chain);
    let error = command_refused_before_signing(&chain, key_file, "base-erc20", 1_000, &base_recipient(), "insufficient_funds");
    assert!(error.contains("cannot cover the amount 1000"), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// A quote whose fee is above `--max-fee-atoms` is refused before signing,
/// with both numbers; a ceiling at the fee lets it through.
#[test]
fn external_lane_export_command_refuses_a_fee_above_the_ceiling_before_signing() {
    let chain = released_export_chain("export-command-fee-ceiling", ACCOUNT_RESERVE + 1_000);
    let fee = export_command(&chain, "base-erc20", 400, &base_recipient(), true).expect("dry run").fee;
    let store = NodeStore::new(&chain.data_dir);
    let mempool_before = store.read_mempool().expect("mempool");
    let faucet = crate::read_key_file(&chain.data_dir.join(FAUCET_KEY_FILE)).expect("key").address;
    for dry_run in [true, false] {
        let error = crate::external_lane_export(crate::ExternalLaneExportOptions {
            data_dir: chain.data_dir.clone(),
            key_file: chain.data_dir.join(FAUCET_KEY_FILE),
            sender: Some(faucet.clone()),
            lane_id: "base-erc20".to_string(),
            amount_atoms: 400,
            base_recipient: base_recipient(),
            dry_run,
            max_fee_atoms: fee - 1,
        })
        .expect_err("a fee above the ceiling")
        .to_string();
        assert_eq!(
            error,
            format!(
                "refused before signing: external_lane_export_fee_above_ceiling: the quoted minimum fee {fee} atoms is above the ceiling {} atoms (--max-fee-atoms)",
                fee - 1
            )
        );
        assert_eq!(store.read_mempool().expect("mempool"), mempool_before, "signed and submitted");
    }
    assert!(crate::export_fee_within_ceiling(fee, fee).is_ok());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The default ceiling lets today's fees through: the largest export
/// (longest chain id, lane id and numbers) pays at most
/// `EXTERNAL_LANE_EXPORT_LARGEST_FEE_ATOMS`, and the default is 4x that.
#[test]
fn external_lane_export_default_fee_ceiling_covers_the_largest_export_fee() {
    let chain = released_export_chain("export-command-fee-largest", ACCOUNT_RESERVE + 1_000);
    let store = NodeStore::new(&chain.data_dir);
    let mut genesis = store.read_genesis().expect("genesis");
    let ledger = store.read_ledger().expect("ledger");
    let today = export_command(&chain, "base-erc20", 400, &base_recipient(), true).expect("dry run").fee;
    genesis.chain_id = "c".repeat(postfiat_types::MAX_TEXT_FIELD_BYTES);
    let operation = AssetTransactionOperation::ExternalLaneExportV1(postfiat_types::ExternalLaneExportOperationV1 {
        sender: chain.recipient.clone(),
        lane_id: format!("l{}", "9".repeat(postfiat_types::EXTERNAL_LANE_ID_MAX_BYTES - 1)),
        amount_atoms: u64::MAX,
        base_recipient: base_recipient(),
    });
    let largest = crate::quote_signed_asset_transaction(&genesis, chain.recipient.clone(), u64::MAX, u64::MAX, operation)
        .expect("the largest export");
    let fee = postfiat_execution::minimum_asset_transaction_fee_for_ledger(&ledger, &largest);
    assert_eq!(postfiat_execution::asset_transaction_state_expansion_fee(&ledger, &largest), 0);
    assert_eq!((today, fee), (22, crate::EXTERNAL_LANE_EXPORT_LARGEST_FEE_ATOMS));
    assert_eq!(crate::EXTERNAL_LANE_EXPORT_DEFAULT_MAX_FEE_ATOMS, 4 * crate::EXTERNAL_LANE_EXPORT_LARGEST_FEE_ATOMS);
    assert!(crate::export_fee_within_ceiling(fee, crate::EXTERNAL_LANE_EXPORT_DEFAULT_MAX_FEE_ATOMS).is_ok());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The key of the release recipient (`chain.recipient`), written to a file.
fn release_recipient_key_file(chain: &Chain) -> PathBuf {
    let key_file = chain.data_dir.join("recipient-key.json");
    write_key_file(&key_file, &release_recipient_key(chain)).expect("recipient key");
    key_file
}

fn release_recipient_key(chain: &Chain) -> DevKeyFile {
    let key_pair = ml_dsa_65_keygen_from_seed(&[0x99; 32]);
    let key = DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: address_from_public_key(&key_pair.public_key),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        private_key_hex: bytes_to_hex(&key_pair.private_key),
    };
    assert_eq!(key.address, chain.recipient);
    key
}

/// Submits `operation` signed by `signer` at its next ledger sequence.
fn submit_escrow_tx(chain: &Chain, signer: &DevKeyFile, operation: EscrowTransactionOperation, kind: &str) {
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
    let sequence = ledger.account(&signer.address).expect("signer").sequence + 1;
    let signed = signed_escrow_transaction_for_test(
        &chain.genesis,
        &ledger,
        &signer.address,
        &signer.public_key_hex,
        &signer.private_key_hex,
        kind,
        sequence,
        operation,
    );
    submit_signed_escrow_transaction_json_to_mempool(SignedEscrowTransactionJsonSubmitOptions {
        data_dir: chain.data_dir.clone(),
        signed_escrow_transaction_json: serde_json::to_string(&signed).expect("escrow json"),
    })
    .expect("escrow admitted");
}

/// A pending entry of a family admission runs after a new asset transaction
/// does not count before signing: the exporter's pending escrow finish that
/// would credit it does not make an export that fits only with the credit
/// pass, since admission executes the export on the ledger before the finish. Once the finish is
/// committed the same export is signed and admitted.
#[test]
fn external_lane_export_command_does_not_count_a_later_family_credit_before_signing() {
    let released = ACCOUNT_RESERVE + 1_000;
    let chain = released_export_chain("export-command-later-credit", released);
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let escrow_amount = 10_000;
    let create = EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
        owner: faucet.address.clone(),
        recipient: chain.recipient.clone(),
        asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
        amount: escrow_amount,
        condition: "later-credit".to_string(),
        finish_after: 0,
        cancel_after: 50,
    });
    let faucet_key = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    submit_escrow_tx(&chain, &faucet_key, create, ESCROW_CREATE_TRANSACTION_KIND);
    assert!(apply_mempool(&chain, "later-credit-create").accepted);
    let escrow_id = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger").escrows[0]
        .escrow_id
        .clone();
    let finish = EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
        escrow_id,
        owner: faucet.address.clone(),
        recipient: chain.recipient.clone(),
        fulfillment: "later-credit".to_string(),
    });
    // The exporter finishes the escrow to itself: a pending credit in a
    // family that runs after asset transactions.
    submit_escrow_tx(&chain, &release_recipient_key(&chain), finish, ESCROW_FINISH_TRANSACTION_KIND);

    let key_file = release_recipient_key_file(&chain);
    command_refused_before_signing(&chain, key_file.clone(), "base-erc20", 1_000, &base_recipient(), "insufficient_funds");

    assert!(apply_mempool(&chain, "later-credit-finish").accepted);
    let report = export_command_with_key(&chain, key_file, "base-erc20", 1_000, &base_recipient(), false)
        .expect("fits once the finish is committed");
    let receipt = apply_mempool(&chain, "later-credit-export");
    assert!(receipt.accepted && Some(&receipt.tx_id) == report.tx_id.as_ref(), "{receipt:?}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The exporter's own pending escrow holds its next sequence: the quote
/// signs at the sequence after it, but admission executes the export before
/// escrows, on a ledger where the sender's sequence is unchanged, and would
/// refuse it with `bad_sequence`. The command refuses before signing; once
/// the escrow is committed the same export is signed and admitted.
#[test]
fn external_lane_export_command_refuses_a_sequence_held_by_a_later_family_before_signing() {
    let chain = released_export_chain("export-command-later-sequence", ACCOUNT_RESERVE + 1_000);
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let create = EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
        owner: faucet.address.clone(),
        recipient: chain.recipient.clone(),
        asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
        amount: 10_000,
        condition: "later-sequence".to_string(),
        finish_after: 0,
        cancel_after: 50,
    });
    submit_escrow_tx(&chain, &faucet, create, ESCROW_CREATE_TRANSACTION_KIND);

    let error = faucet_refused(&chain, "base-erc20", 1_000, &base_recipient(), "bad_sequence");
    assert!(error.contains("runs after asset transactions"), "{error}");

    assert!(apply_mempool(&chain, "later-sequence-escrow").accepted);
    let report = export_command(&chain, "base-erc20", 1_000, &base_recipient(), false)
        .expect("signed once the escrow is committed");
    let receipt = apply_mempool(&chain, "later-sequence-export");
    assert!(receipt.accepted && Some(&receipt.tx_id) == report.tx_id.as_ref(), "{receipt:?}");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// Fills the stored mempool with `count` copies of the faucet's one pending
/// escrow (fresh tx ids and sequences; `source` replaced when given). Escrows
/// run after asset transactions, so the export's quote never executes them:
/// only admission's limits see them.
fn fill_mempool_with_escrow_copies(chain: &Chain, count: usize, source: Option<&str>) {
    let store = NodeStore::new(&chain.data_dir);
    let mut mempool = store.read_mempool().expect("mempool");
    let template = mempool.pending_escrow_transactions[0].clone();
    for index in 0..count {
        let mut entry = template.clone();
        entry.tx_id = format!("{:096x}", index + 1);
        entry.transaction.unsigned.sequence += 1 + index as u64;
        if let Some(source) = source {
            entry.transaction.unsigned.source = source.to_string();
        }
        mempool.pending_escrow_transactions.push(entry);
    }
    store.write_mempool(&mempool).expect("write mempool");
}

fn submit_faucet_escrow(chain: &Chain, condition: &str) {
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let create = EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
        owner: faucet.address.clone(),
        recipient: chain.recipient.clone(),
        asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
        amount: 10_000,
        condition: condition.to_string(),
        finish_after: 0,
        cancel_after: 50,
    });
    submit_escrow_tx(chain, &faucet, create, ESCROW_CREATE_TRANSACTION_KIND);
}

/// Admission's mempool limits are refused before signing: the sender's
/// pending count at its limit, and a full mempool.
#[test]
fn external_lane_export_command_refuses_the_mempool_limits_before_signing() {
    let chain = released_export_chain("export-command-sender-limit", ACCOUNT_RESERVE + 1_000);
    submit_faucet_escrow(&chain, "sender-limit");
    fill_mempool_with_escrow_copies(&chain, crate::MAX_MEMPOOL_PENDING_PER_SENDER - 1, None);
    let error = faucet_refused(&chain, "base-erc20", 1, &base_recipient(), "mempool_sender_pending_limit");
    assert!(error.contains(&format!("{}", crate::MAX_MEMPOOL_PENDING_PER_SENDER)), "{error}");
    let _ = fs::remove_dir_all(&chain.data_dir);

    let chain = released_export_chain("export-command-mempool-full", ACCOUNT_RESERVE + 1_000);
    submit_faucet_escrow(&chain, "mempool-full");
    fill_mempool_with_escrow_copies(&chain, crate::MAX_MEMPOOL_PENDING_TRANSACTIONS - 1, Some(&chain.recipient));
    let error = faucet_refused(&chain, "base-erc20", 1, &base_recipient(), "mempool_full");
    assert!(error.contains(&format!("{}", crate::MAX_MEMPOOL_PENDING_TRANSACTIONS)), "{error}");
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
    let receipts = apply_mempool_batch(&chain, "pending");
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
            && error.contains("payment_v2 entries and asset transactions (pending releases and exports included)"),
        "{error}"
    );
    let receipts = apply_mempool_batch(&chain, "pending-release");
    assert!(receipts.len() == 2 && receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    assert_eq!(Some(&receipts[1].tx_id), report.tx_id.as_ref());
    let row = export_page(&chain.data_dir, None, None);
    assert_eq!((row.state.released_atoms, row.state.exported_atoms), (2 * released, amount));
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The quote of an export says it was checked (`export_checked`); a quote
/// without the marker comes from a node too old to check an export, and the
/// command refuses it before signing. A quote of another kind is unchanged:
/// no marker, the same fields.
#[test]
fn external_lane_export_command_refuses_a_quote_that_was_not_checked() {
    let chain = released_export_chain("export-command-checked", ACCOUNT_RESERVE + 1_000);
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let quote_json = |operation: &AssetTransactionOperation| {
        let quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: chain.data_dir.clone(),
            source: faucet.address.clone(),
            operation_json: serde_json::to_string(operation).expect("operation json"),
            sequence: None,
        })
        .expect("quote");
        serde_json::to_value(&quote).expect("quote json")
    };
    let export = AssetTransactionOperation::ExternalLaneExportV1(postfiat_types::ExternalLaneExportOperationV1 {
        sender: faucet.address.clone(),
        lane_id: "base-erc20".to_string(),
        amount_atoms: 400,
        base_recipient: base_recipient(),
    });
    let mut checked = quote_json(&export);
    assert_eq!(checked["export_checked"], serde_json::Value::Bool(true), "{checked}");
    let report: crate::AssetFeeQuoteReport = serde_json::from_value(checked.clone()).expect("quote");
    crate::export_quote_was_checked(&report).expect("a checked quote");
    checked.as_object_mut().expect("object").remove("export_checked");
    let old_node: crate::AssetFeeQuoteReport = serde_json::from_value(checked).expect("old node quote");
    let error = crate::export_quote_was_checked(&old_node).expect_err("refused").to_string();
    assert!(
        error.starts_with("refused before signing: external_lane_export_not_checked: ") && error.contains("too old"),
        "{error}"
    );

    let other = quote_json(&AssetTransactionOperation::AssetCreate(AssetCreateOperation {
        issuer: faucet.address.clone(),
        code: "EUR".to_string(),
        version: 1,
        precision: 2,
        display_name: "Euro".to_string(),
        max_supply: Some(10_000),
        requires_authorization: false,
        freeze_enabled: false,
        clawback_enabled: false,
    }));
    let keys: Vec<&str> = other.as_object().expect("object").keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "account_reserve",
            "asset_weight_bytes",
            "base_asset_fee",
            "chain_id",
            "genesis_hash",
            "mempool_pending_for_sender",
            "minimum_fee",
            "operation",
            "protocol_version",
            "schema",
            "sender_balance",
            "sender_balance_after_fee",
            "sender_meets_reserve_after_fee",
            "sender_sequence",
            "sequence",
            "sequence_source",
            "source",
            "state_expansion_fee",
            "transaction_kind",
            "transfer_fee_byte_quantum",
            "transfer_fee_per_quantum",
        ]
    );
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// Before the export root's activation height the dry run returns
/// execution's refusal, so the command refuses before signing.
#[test]
fn external_lane_export_command_refuses_before_the_export_root_is_active() {
    let chain = released_export_chain_from("export-command-root", ACCOUNT_RESERVE + 1_000, 100);
    let error = faucet_refused(&chain, "base-erc20", 1, &base_recipient(), postfiat_types::EXTERNAL_LANE_EXPORT_ROOT_NOT_ACTIVE);
    assert!(error.contains("export root"), "{error}");
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

fn header_at(chain: &Chain, height: u64) -> BlockHeader {
    NodeStore::new(&chain.data_dir)
        .read_blocks()
        .expect("blocks")
        .blocks
        .into_iter()
        .find(|block| block.header.height == height)
        .expect("block")
        .header
}

/// Before the activation height a header has no export root and an export is
/// refused at mempool admission, batch building and execution; from it every
/// header has one: the root of the block's export leaves, or the empty value.
#[test]
fn external_lane_export_root_starts_at_its_activation_height() {
    let chain = released_export_chain_from("export-root-boundary", ACCOUNT_RESERVE + 1_000, 4);
    assert_eq!(tip_height(&chain), 2);
    let early = export_tx(&chain, "base-erc20", 7, &base_recipient());
    refused_at_every_entry_point(&chain, &early, postfiat_types::EXTERNAL_LANE_EXPORT_ROOT_NOT_ACTIVE, "early");
    let receipt = admit_and_apply(
        &chain,
        &release_tx(&chain, "base-erc20", 2, 1_000, faucet_sequence(&chain)),
        "release-3",
    );
    assert!(receipt.accepted, "{receipt:?}");
    for height in [2, 3] {
        assert_eq!(header_at(&chain, height).external_lane_export_root, None, "height {height}");
    }

    let receipt = admit_and_apply(&chain, &export_tx(&chain, "base-erc20", 7, &base_recipient()), "export-4");
    assert!(receipt.accepted, "{receipt:?}");
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
    let packet = ledger.external_lanes[0].exports[0].clone();
    assert_eq!(packet.exported_at_height, 4);
    let leaf = packet.export_root_leaf(&chain.genesis.chain_id).expect("leaf");
    let root = header_at(&chain, 4).external_lane_export_root.expect("root at the activation height");
    assert_eq!(root, postfiat_types::external_lane_export_root_v1(&[leaf]).expect("root"));
    let proof = postfiat_types::external_lane_export_root_proof_v1(&[leaf], 0).expect("proof");
    postfiat_types::verify_external_lane_export_root_proof_v1(&root, &proof).expect("member");

    let receipt = admit_and_apply(
        &chain,
        &release_tx(&chain, "base-erc20", 3, 1_000, faucet_sequence(&chain)),
        "release-5",
    );
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(header_at(&chain, 5).external_lane_export_root, Some(postfiat_types::bridge_exit_empty_root_v1()));
    verify_blocks(NodeOptions { data_dir: chain.data_dir.clone() }).expect("replay verifies the roots");

    // A snapshot round trip keeps the headers' roots and still verifies.
    let snapshot_dir = sibling_dir(&chain.data_dir, "export-root-snapshot");
    let restored_dir = sibling_dir(&chain.data_dir, "export-root-restored");
    export_snapshot(SnapshotExportOptions { data_dir: chain.data_dir.clone(), snapshot_dir: snapshot_dir.clone() })
        .expect("export snapshot");
    import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: Some("validator-restored".to_string()),
    })
    .expect("import snapshot");
    let restored = NodeStore::new(&restored_dir).read_blocks().expect("restored blocks");
    for block in &restored.blocks {
        assert_eq!(
            block.header.external_lane_export_root,
            header_at(&chain, block.header.height).external_lane_export_root,
            "height {}",
            block.header.height
        );
    }
    assert_eq!(restored.blocks.iter().filter(|block| block.header.external_lane_export_root.is_some()).count(), 2);
    for dir in [snapshot_dir, restored_dir] {
        let _ = fs::remove_dir_all(dir);
    }

    // A block whose export root was changed after the fact no longer verifies.
    let store = NodeStore::new(&chain.data_dir);
    let mut blocks = store.read_blocks().expect("blocks");
    let tampered = blocks.blocks.iter_mut().find(|block| block.header.height == 4).expect("block 4");
    tampered.header.external_lane_export_root = Some(postfiat_types::bridge_exit_empty_root_v1());
    store.write_blocks(&blocks).expect("write tampered blocks");
    assert!(verify_blocks(NodeOptions { data_dir: chain.data_dir.clone() }).is_err());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

fn export_receipt(tx_id: &str, accepted: bool) -> Receipt {
    serde_json::from_value(serde_json::json!({
        "tx_id": tx_id, "accepted": accepted, "code": if accepted { "accepted" } else { "refused" },
        "message": "", "fee_charged": 0, "fee_burned": 0, "minimum_fee": 0, "account_reserve": 0,
        "state_expansion_fee": 0, "nft_issuer_transfer_fee": 0, "nft_collection_flags": 0,
        "offer_fills": []
    }))
    .expect("receipt")
}

/// Two exports of one lane and one of another in one block: the leaves follow
/// the receipts (execution order) across lanes, and each has a proof.
#[test]
fn external_lane_export_root_orders_leaves_by_receipt_across_lanes() {
    let packet = |lane: &str, nonce: u64, tx: &str| postfiat_types::ExternalLaneExportPacket {
        lane_id: lane.to_string(),
        export_nonce: nonce,
        sender: "pftl-sender".to_string(),
        base_recipient: base_recipient(),
        amount_atoms: 5,
        exported_at_height: 9,
        tx_id: tx.repeat(48),
    };
    let mut ledger = LedgerState::new(Vec::new());
    let lane = |id: &str| {
        let mut lane = lane_genesis("postfiat-local", 4).external_lanes.expect("lanes")[0].clone();
        lane.id = id.to_string();
        postfiat_types::ExternalLaneState::genesis(&lane)
    };
    let mut first = lane("base-erc20");
    first.exports = vec![packet("base-erc20", 0, "0a"), packet("base-erc20", 1, "0c")];
    let mut second = lane("base-other");
    // An earlier block's export is not a leaf of this one.
    let mut old = packet("base-other", 0, "0d");
    old.exported_at_height = 8;
    second.exports = vec![old, packet("base-other", 1, "0b")];
    ledger.external_lanes = vec![first, second];
    let receipts = ["0a", "0e", "0b", "0c"]
        .iter()
        .map(|tx| export_receipt(&tx.repeat(48), *tx != "0e"))
        .collect::<Vec<_>>();
    let packets = external_lane_export_packets_for_block(&ledger, &receipts, 9).expect("packets");
    let order = packets.iter().map(|packet| (packet.lane_id.as_str(), packet.export_nonce)).collect::<Vec<_>>();
    assert_eq!(order, [("base-erc20", 0), ("base-other", 1), ("base-erc20", 1)]);
    let leaves = packets
        .iter()
        .map(|packet| packet.export_root_leaf("postfiat-local").expect("leaf"))
        .collect::<Vec<_>>();
    let root = postfiat_types::external_lane_export_root_v1(&leaves).expect("root");
    for index in 0..3 {
        let proof = postfiat_types::external_lane_export_root_proof_v1(&leaves, index).expect("proof");
        postfiat_types::verify_external_lane_export_root_proof_v1(&root, &proof).expect("member");
    }
    // A packet of the block without an accepted receipt is a corrupt block.
    let missing = receipts[..3].to_vec();
    assert!(external_lane_export_packets_for_block(&ledger, &missing, 9).is_err());
}

/// A mempool with bound + 3 exports gives a block of exactly the bound and the
/// next block the 3 left waiting; a block over the bound is invalid when it is
/// applied and when it is replayed. The test lowers the bound to 3 on its
/// thread (`EXTERNAL_LANE_EXPORT_BLOCK_BOUND_FOR_TESTS`): the real bound, 4,096,
/// is above the 1,024-transaction batch cap.
#[test]
fn external_lane_exports_past_the_block_bound_wait_for_the_next_block() {
    let bound = 3;
    EXTERNAL_LANE_EXPORT_BLOCK_BOUND_FOR_TESTS.with(|cell| cell.set(Some(bound)));
    let chain = released_export_chain("export-bound", ACCOUNT_RESERVE + 100_000);
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let first = faucet_sequence(&chain);
    for offset in 0..(bound as u64 + 3) {
        let tx = export_tx_at(&chain, "base-erc20", 10, &base_recipient(), &faucet.address, first + offset);
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: chain.data_dir.clone(),
            signed_asset_transaction_json: serde_json::to_string(&tx).expect("tx json"),
        })
        .expect("mempool admits the export");
    }
    let next_batch = |label: &str| {
        let batch_file = chain.data_dir.join(format!("{label}.batch.json"));
        let batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file: batch_file.clone(),
            max_transactions: 8,
        })
        .expect("mempool batch");
        (batch, batch_file)
    };
    let apply = |batch_file: PathBuf, label: &str| {
        let certificate_file = Some(consensus_v2_certificate(&chain.data_dir, &batch_file, label));
        apply_batch(ApplyBatchOptions { data_dir: chain.data_dir.clone(), batch_file, certificate_file })
    };
    let apply_certified = |batch_file: PathBuf, certificate_file: PathBuf| {
        apply_batch(ApplyBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file,
            certificate_file: Some(certificate_file),
        })
    };
    let (batch, batch_file) = next_batch("bound-1");
    assert_eq!(batch.asset_transactions.len(), bound);
    let store = NodeStore::new(&chain.data_dir);
    assert_eq!(store.read_mempool().expect("mempool").pending_asset_transactions.len(), 3);
    let receipts = apply(batch_file, "bound-1").expect("block of the bound");
    assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    let (batch, batch_file) = next_batch("bound-2");
    assert_eq!(batch.asset_transactions.len(), 3);
    // Certified under a bound of 3, the same block is refused by a validator
    // whose bound is 2.
    let certificate_file = consensus_v2_certificate(&chain.data_dir, &batch_file, "bound-2");
    EXTERNAL_LANE_EXPORT_BLOCK_BOUND_FOR_TESTS.with(|cell| cell.set(Some(2)));
    let error = apply_certified(batch_file.clone(), certificate_file.clone()).unwrap_err().to_string();
    assert!(error.contains(postfiat_types::EXTERNAL_LANE_EXPORT_ROOT_FULL), "{error}");
    EXTERNAL_LANE_EXPORT_BLOCK_BOUND_FOR_TESTS.with(|cell| cell.set(Some(bound)));
    apply_certified(batch_file, certificate_file).expect("block of the remaining three");
    let tip = tip_height(&chain);
    for height in [tip - 1, tip] {
        let leaves = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger").external_lanes[0]
            .exports
            .iter()
            .filter(|packet| packet.exported_at_height == height)
            .count();
        assert_eq!(leaves, bound, "height {height}");
    }
    verify_blocks(NodeOptions { data_dir: chain.data_dir.clone() }).expect("replay within the bound");
    // Replay under a bound of 2 finds both blocks over it.
    EXTERNAL_LANE_EXPORT_BLOCK_BOUND_FOR_TESTS.with(|cell| cell.set(Some(2)));
    let error = verify_blocks(NodeOptions { data_dir: chain.data_dir.clone() }).unwrap_err().to_string();
    assert!(error.contains(postfiat_types::EXTERNAL_LANE_EXPORT_ROOT_FULL), "{error}");
    EXTERNAL_LANE_EXPORT_BLOCK_BOUND_FOR_TESTS.with(|cell| cell.set(None));
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// Every consensus v2 block reference in a certificate file, edited by `edit`.
fn edit_certificate_blocks(certificate_file: &Path, label: &str, edit: impl Fn(&mut serde_json::Map<String, serde_json::Value>)) -> PathBuf {
    fn walk(value: &mut serde_json::Value, edit: &dyn Fn(&mut serde_json::Map<String, serde_json::Value>)) {
        match value {
            serde_json::Value::Object(map) => {
                if map.contains_key("payload_hash") && map.contains_key("parent_block_id") {
                    edit(map);
                }
                for child in map.values_mut() {
                    walk(child, edit);
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(|item| walk(item, edit)),
            _ => {}
        }
    }
    let mut certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(certificate_file).expect("certificate")).expect("certificate json");
    walk(&mut certificate, &edit);
    let edited = certificate_file.with_file_name(format!("{label}.certificate.json"));
    fs::write(&edited, serde_json::to_vec(&certificate).expect("json")).expect("write certificate");
    edited
}

/// A validator applies a consensus v2 commit only when its block's lane export
/// root is the one its own execution gives: a different root, a root before the
/// activation height and no root from it on are all refused, and the tip stays.
/// The edited certificates fail at the block-id recomputation, which binds the
/// root, before the comparison with the validator's execution in
/// `storage_commit.rs`.
#[test]
fn external_lane_export_root_of_a_commit_must_match_the_validators_execution() {
    // Consensus v2 from height 2, the export root from height 4.
    let chain = lane_chain_with_genesis("export-root-commit", 86_400, Some(2), 4, |genesis| {
        genesis.external_lanes.as_mut().expect("lanes")[0].policy.exports_enabled = true;
        genesis.external_lane_export_root_activation_height = Some(4);
    });
    let receipt = admit_and_apply(&chain, &release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 100_000, 1), "release");
    assert!(receipt.accepted, "{receipt:?}");
    let prepared = |tx: &SignedAssetTransaction, label: &str| {
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: chain.data_dir.clone(),
            signed_asset_transaction_json: serde_json::to_string(tx).expect("tx json"),
        })
        .expect("mempool admits");
        let batch_file = chain.data_dir.join(format!("{label}.batch.json"));
        create_mempool_batch(MempoolBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file: batch_file.clone(),
            max_transactions: 8,
        })
        .expect("batch");
        let certificate = consensus_v2_certificate(&chain.data_dir, &batch_file, label);
        (batch_file, certificate)
    };
    let refused = |batch_file: &Path, certificate: PathBuf| {
        let tip = tip_height(&chain);
        let result = apply_batch(ApplyBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file: batch_file.to_path_buf(),
            certificate_file: Some(certificate),
        });
        assert!(result.is_err(), "{result:?}");
        assert_eq!(tip_height(&chain), tip);
        result.unwrap_err().to_string()
    };
    let apply = |batch_file: &Path, certificate: PathBuf| {
        apply_batch(ApplyBatchOptions {
            data_dir: chain.data_dir.clone(),
            batch_file: batch_file.to_path_buf(),
            certificate_file: Some(certificate),
        })
        .expect("apply")
    };
    let other_root = postfiat_types::external_lane_export_root_v1(&[[9; 48]]).expect("root");

    // Height 3, before the activation height: a root is refused.
    let (batch, certificate) = prepared(&release_tx(&chain, "base-erc20", 2, 1_000, faucet_sequence(&chain)), "commit-3");
    let early = edit_certificate_blocks(&certificate, "commit-3-root", |block| {
        block.insert("external_lane_export_root".to_string(), serde_json::json!(other_root));
    });
    let error = refused(&batch, early);
    assert!(error.contains("consensus v2 proposal block ID mismatch"), "{error}");
    apply(&batch, certificate);

    // Height 4, the activation height: another root, or none, is refused.
    let (batch, certificate) = prepared(&export_tx(&chain, "base-erc20", 10, &base_recipient()), "commit-4");
    let wrong = edit_certificate_blocks(&certificate, "commit-4-wrong", |block| {
        block.insert("external_lane_export_root".to_string(), serde_json::json!(other_root));
    });
    let error = refused(&batch, wrong);
    assert!(error.contains("consensus v2 proposal block ID mismatch"), "{error}");
    let missing = edit_certificate_blocks(&certificate, "commit-4-none", |block| {
        block.remove("external_lane_export_root");
    });
    let error = refused(&batch, missing);
    assert!(error.contains("consensus v2 proposal block ID mismatch"), "{error}");
    let receipts = apply(&batch, certificate);
    assert!(receipts[0].accepted, "{receipts:?}");
    assert!(header_at(&chain, 4).external_lane_export_root.is_some());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// Block hash, proposal hash, vote message and consensus v2 block id of a
/// header without the lane export root, pinned to the values main's code gives:
/// this branch leaves those encodings unchanged. Every header variant (no
/// optional root; bridge-exit root only; PFTL-Uniswap root only; both), each
/// with an empty and a non-empty `fastpay_pre_state_effects` array (the older
/// v2 encodings), and the vote message under both registry-root forms (the
/// encoder branches on an empty registry root).
///
/// The values were produced by running this same loop as a throwaway test in a
/// scratch worktree of origin/main (d67153bda), with main's encoders and
/// main's `BlockEvidence` (no export-root field), and printing the results; the
/// first three variants' values equal the pins first computed on main
/// 5c8f693ca. None was produced by this branch's encoder.
#[test]
fn external_lane_block_encodings_without_the_export_root_hash_as_on_main() {
    let genesis = Genesis::new("postfiat-hash-pin");
    let receipt_ids = vec!["ab".repeat(48)];
    let domain = postfiat_types::ConsensusV2Domain {
        schema: postfiat_types::CONSENSUS_V2_DOMAIN_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        committee_epoch: 1,
        committee_root: "41".repeat(48),
    };
    let fences = [postfiat_types::FastPayVersionFenceV1 {
        schema: postfiat_types::FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
        operation: postfiat_types::FastPayOperationKindV1::Transfer,
        origin: postfiat_types::FastPayFenceOriginV1::OrderedRecovery,
        committee_epoch: 1,
        registry_root: "42".repeat(48),
        lock_id: "43".repeat(48),
        inputs: vec![postfiat_types::OwnedObjectRef { id: "44".repeat(32), version: 1 }],
        decision: postfiat_types::FastPayRecoveryDecisionV1::Cancelled,
        certificate: None,
        decided_at_height: 4,
        next_versions: vec![postfiat_types::OwnedObjectRef { id: "44".repeat(32), version: 2 }],
    }];
    let (bridge, uniswap) = ("e".repeat(96), "f".repeat(96));
    let mut values = Vec::new();
    for effects in [&[][..], &fences[..]] {
        for (bridge_exit_root, pftl_uniswap_receipt_root) in [
            (None, None),
            (Some(bridge.as_str()), None),
            (None, Some(uniswap.as_str())),
            (Some(bridge.as_str()), Some(uniswap.as_str())),
        ] {
            let evidence = BlockEvidence {
                height: 5,
                view: 1,
                parent_hash: &"1".repeat(96),
                proposer: "validator-0",
                batch_kind: "transparent",
                batch_id: &"2".repeat(96),
                state_root: &"3".repeat(96),
                bridge_exit_root,
                pftl_uniswap_receipt_root,
                external_lane_export_root: None,
                receipt_ids: &receipt_ids,
                fastpay_pre_state_effects: effects,
            };
            values.push(block_hash(&genesis, &evidence, &"c".repeat(96)).expect("block hash"));
            values.push(block_proposal_hash_from_evidence(&genesis, &evidence, &"d".repeat(96)).expect("proposal hash"));
            for registry_root in ["a".repeat(96), String::new()] {
                let vote = block_certificate_vote_message(&genesis, &evidence, "validator-0", true, &registry_root)
                    .expect("vote message");
                values.push(postfiat_crypto_provider::hash_hex("postfiat.test.vote_message_pin", &vote));
            }
            let block = match (bridge_exit_root, pftl_uniswap_receipt_root) {
                (None, None) => postfiat_ordering_fast::consensus_v2_block_ref(&domain, 5, "1".repeat(96), "2".repeat(96), "3".repeat(96)),
                (Some(bridge), None) => postfiat_ordering_fast::consensus_v2_block_ref_with_bridge_exit_root(
                    &domain, 5, "1".repeat(96), "2".repeat(96), "3".repeat(96), bridge,
                ),
                (bridge, Some(uniswap)) => postfiat_ordering_fast::consensus_v2_block_ref_with_commitment_roots(
                    &domain, 5, "1".repeat(96), "2".repeat(96), "3".repeat(96), bridge.map(str::to_string), uniswap,
                ),
            };
            values.push(block.expect("consensus v2 block").block_id);
        }
    }
    // Per FastPay array (empty, then one fence) and header variant (no root,
    // bridge-exit root, PFTL-Uniswap root, both): block hash, proposal hash,
    // vote-message digest with a registry root, without one, consensus v2
    // block id.
    let expected = [
        "34fea2e2ad5af642c0d764eb5009fc2a4500dbfef37c38125d925bb94aed159071a0144a94843f0579592cdd93f077f9",
        "6e88a13f5470ac7ed85107370b3c7eea9b09d41a339f7aca1a3e27577fbbc424862e0a8e76a05fa7c84d918de5f1ac09",
        "3708b97f9061746d0abc8a75242b2734d8920a116fdd88f346d709f98ef9f0002703b4adf6f5577730309644b1f8d40e",
        "f6bbbcb8d19c53c01814f54d82ffa1d9909aa0e7041898431d5f2eebf158dbe6740a3d1abb0ad49b5c8f70a208c314e9",
        "22905f922156b15ae3a648451171cc7ff34086b69493082de8ae3cd400a80b6a18266ed9e90c2d111381c1bdf180a97f",
        "67b352f21b847efc9efdee426ef5faded4236411dd32189c02b37ca4cb54ba9a9797a9ce7a0ce1f395a624ce1c247626",
        "4ce2c2174495bfc6ce801862b1d584f2ce2ab9373420f465d7c25cc08c16a78933e55b4fb9f886e00969577ffb368c95",
        "90ab2160020855bb5cd608fbba4b124d76d110d91e9eeac8d0e4af4508c60649e806161bbbc509a00b45980630277b18",
        "9792464ef4b7759b4b38392e9c0af7f707785384872ce00ce4992b5d32fb1037c1d45d5f283e8d60b5034f98d21772a3",
        "8bf2b730aa8769993e3ba862a8b58041a7bbaa80917c6ab5146a5d2bd3783fce65db59f88b4cffd5243e7c2a7bf54cd7",
        "a2bbb53de9e7ee0f8223570000120ec5f86cd312a68f2420faa91a72aec429d622d178b6e48a7b3f1d0a51b82f081f76",
        "517cd6d714d7fb80a3394422f4dfc609047d41a0430108f4d1b5ab65f4bbcabc828a98b2fdcf85d74f82fe1fc4d41d13",
        "bb8ff901d9cac8924f99b5b5166dcd7708cb271a414786969237c858688983bd405a160f09a6f7921d1b364bf8685ae9",
        "ca7b93439a73856656b14402e4a4c0481e1440a541db26dc466fb58f4b4f47f5158287fa973980f0a9073d75cddc48a6",
        "a789b7be07d1a3449a02ed4ea83142c0fa99338d54fd110c799ab556175f93cc16ba19eac5a009a0626b068b4700dbbf",
        "d664052b432488ac3ab7d3552610dfd8266cf7a55647fdf582a3433e9e0bf1dd3c91e700f9a64532a3ef4249ef032ea9",
        "15ea0b1781104df5813deec1b76a16c6d2280cd7ba1055f98d131ce089c111d353e32e151dcca9df510395d0401ce7f1",
        "a9c6c9f50f1c4c38abb6919ad7d72583215b9dfa1ecce4cdb82ce5a120ba8e7dfd3ccd46780d675d0e02b400b584de45",
        "ad59989a14e64fe2c2354c8a329454390fa89306b564a9fdf98ce96cb8847970b2d87489e7d47fb10e8e3eefd3abd2ff",
        "fa60977290fd1d51c75993b6ff5d39bd90a8cfcb524bdfc983f2b2f49579bddcb6e1c4cbe9e206757886c42c07aa08c5",
        "d7ffa77f3f70f201304cee2837f10e1f07269ce48354f218bd74dc1b03db0f86776d35f88f539bf62939e1cc73df971d",
        "e114dca81ffb378ed7bbc9821e94dc4c08493cd27e8475f47d09a8abced9b97c47f9d619c4b65ca0b3b11eaa6211d5d5",
        "f7695b34df4c3b0ac5f23a1c532da9fd8cbdff4b22ea99d810e0d1427187dcf8a8e26b036c9787b2ebace744d7ad563b",
        "cdea573aac0ce4fd58efc75b1fc87d07629bfe5cbe75f8beb2470973aa3c4ed1b51e49eb1fd85df5ed3e3d26adfab47d",
        "22905f922156b15ae3a648451171cc7ff34086b69493082de8ae3cd400a80b6a18266ed9e90c2d111381c1bdf180a97f",
        "26ba483aa2550d6611b84b1ea3e086a362243fede0c2bd8702a2b8b6c3ecd163d01de0779780177ef13a806630e9c521",
        "a406354d3ae3a1c237ecaa8bee326776eb5fefe7938417d0a02b3602da046df9a38e7952be59ee923b8a1a2a6937956d",
        "1d869d2d4cfb627d24b1dc18aff24672b826f29875ed8e1ec107d9ddc2c449da2d46dc9690fb007a32074e7b75379b85",
        "ba5d5a5e9cbd4b063cf943201cf91fdd0eca516ef24f4f8f578d59421fcc8fde9a24f9b90c06106db14b098b1976b8f1",
        "8bf2b730aa8769993e3ba862a8b58041a7bbaa80917c6ab5146a5d2bd3783fce65db59f88b4cffd5243e7c2a7bf54cd7",
        "d12a380996c93138a3e64a9db2fb4d8cf74db20c03601fbd7c0e963154dd2f9948c15edd6c7b035e1519cf2c35658079",
        "1654af38f243f9fac50dc20c1c8eeca8b43bec725051def83f2e999ee59641f5709d8904f3794c6e69b8d35654dcc2c3",
        "1128d9e5accabd3f016b7472015c52ed2e78e88f738eb89df4409b99f7b92c93203058eb5e14afb52fe6c762e1695bde",
        "89f5f015d75b0cd3451e2408f2ed44f7e680ad057884e746d52caca5ae92b8b412d02fdc49fac03d22c1e6ffca2cf7ad",
        "a789b7be07d1a3449a02ed4ea83142c0fa99338d54fd110c799ab556175f93cc16ba19eac5a009a0626b068b4700dbbf",
        "5b745df33a7db2407a6ff039bfde356f5f7478ddf0384dda0c1d4dc84060180369da8645f86106a9e73a8be2ef0bef3f",
        "dac14f8bee904be011971a42c669fefd2ed7c3a684841d6efe7c41ff4579352c01d24e17b5b856b47f36d7df12459568",
        "86f887c8d28dfc37860c2c7d347edbdd8eb0436d867089502790d668375695748ac1ade8f88c3367ffbb30e7bf9de359",
        "466f0446a18e3a40cd0246b259310a6e79e5fc61239b5843a4cb582351adf108391b4fe52562d7cf016b0b4cd7dc28d6",
        "fa60977290fd1d51c75993b6ff5d39bd90a8cfcb524bdfc983f2b2f49579bddcb6e1c4cbe9e206757886c42c07aa08c5",
    ];
    assert_eq!(values, expected);
}

/// The genesis hash the verifier pins commit to is the one execution computes
/// and a real finalized block's consensus v2 domain carries, which the
/// checkpoint public values take as `pftl_genesis_hash`.
#[test]
fn external_lane_verifier_pins_use_the_chains_genesis_hash() {
    let chain = released_export_chain("export-pins", ACCOUNT_RESERVE + 1_000);
    let pins = chain.genesis.external_lane_verifier_pins("base-erc20").expect("pins");
    assert_eq!(pins.pftl_genesis_hash, genesis_hash(&chain.genesis));
    let block = NodeStore::new(&chain.data_dir)
        .read_blocks()
        .expect("blocks")
        .blocks
        .into_iter()
        .rev()
        .find(|block| block.header.consensus_v2_commit.is_some())
        .expect("a consensus v2 block");
    let commit = block.header.consensus_v2_commit.expect("commit");
    assert_eq!(commit.proposal.domain.genesis_hash, pins.pftl_genesis_hash);
    assert_eq!(commit.proposal.domain.chain_id, pins.pftl_chain_id);
    let committed = hex_to_bytes(&commit.proposal.domain.genesis_hash).expect("hex");
    assert_eq!(
        pins.pftl_genesis_hash_commitment,
        format!("0x{}", bytes_to_hex(&sha3::Keccak256::digest(&committed)))
    );
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// `init` refuses a genesis that activates the export root at a height where
/// consensus v2 is not active, since no block from that height could be built.
#[test]
fn external_lane_export_root_genesis_needs_consensus_v2_at_its_height() {
    for (consensus_v2, accepted) in [(None, false), (Some(6), false), (Some(5), true), (Some(1), true)] {
        let mut genesis = lane_genesis("postfiat-lane-root-init", 4);
        genesis.external_lane_export_root_activation_height = Some(5);
        genesis.consensus_v2_activation_height = consensus_v2;
        let data_dir = unique_test_dir("postfiat-external-lane-root-init");
        let result = init_with_genesis(data_dir.clone(), "validator-0".to_string(), genesis);
        assert_eq!(result.is_ok(), accepted, "consensus v2 at {consensus_v2:?}: {result:?}");
        if let Err(error) = result {
            assert!(error.to_string().contains("consensus v2 is not active"), "{error}");
        }
        let _ = fs::remove_dir_all(data_dir);
    }
}

/// An export-root activation amendment ratified at `value`, its lifecycle
/// activation at `slot`, supported by all of `validators`.
fn export_root_amendment(genesis: &Genesis, validators: &[String], value: u32, slot: u64) -> GovernanceAmendment {
    postfiat_consensus_cobalt::ratify_governance_amendment_with_lifecycle(
        &cobalt_domain(genesis),
        &EssentialSubsetConfig { validators: validators.to_vec(), quorum: validators.len() },
        postfiat_types::GOVERNANCE_KIND_EXTERNAL_LANE_EXPORT_ROOT_ACTIVATION_HEIGHT,
        value,
        validators.to_vec(),
        GovernanceAmendmentLifecycle { activation_height: slot, veto_until_height: 0, paused: false },
    )
    .expect("ratified")
}

const REQUIRES_CONSENSUS_V2: &str = "external_lane_export_root_activation_requires_consensus_v2";

/// An export-root activation below the consensus v2 activation height would
/// stop block production there, so batch admission and building, execution,
/// the archived replay and the ledgerless certificate replay all refuse it,
/// and accept one at or above that height.
#[test]
fn external_lane_export_root_amendment_needs_consensus_v2_at_its_height() {
    let mut genesis = lane_genesis("postfiat-lane-root-amendment-v2", 4);
    genesis.consensus_v2_activation_height = Some(6);
    let validators = local_validator_ids(4).expect("validators");
    for (value, accepted) in [(5, false), (6, true), (7, true)] {
        let batch = GovernanceActionBatch::new(
            format!("export-root-{value}"),
            vec![export_root_amendment(&genesis, &validators, value, 1)],
        );
        let ready = ensure_governance_batch_lifecycle_ready(&genesis, &batch, 1);
        assert_eq!(ready.is_ok(), accepted, "{value}: {ready:?}");
        if let Err(error) = ready {
            assert!(error.to_string().starts_with(REQUIRES_CONSENSUS_V2), "{error}");
        }
        let receipts = [
            crate::execution_actions::execute_governance_batch(
                &mut GovernanceState::new(4),
                None,
                Some(&genesis),
                &batch,
                1,
            ),
            crate::execution_actions::execute_archived_governance_batch(
                &mut GovernanceState::new(4),
                None,
                Some(&genesis),
                &batch,
                1,
            ),
            crate::execution_actions::execute_ledgerless_archived_governance_batch(
                &mut GovernanceState::new(4),
                &genesis,
                &batch,
                1,
            ),
        ];
        for receipt in receipts.iter().map(|receipts| &receipts[0]) {
            assert_eq!(receipt.accepted, accepted, "{value}: {receipt:?}");
            if !accepted {
                assert_eq!(receipt.code, REQUIRES_CONSENSUS_V2, "{receipt:?}");
            }
        }
        // Without a genesis the consensus v2 activation is unknown: refused.
        let receipts = crate::execution_actions::execute_governance_batch(&mut GovernanceState::new(4), None, None, &batch, 1);
        assert_eq!(receipts[0].code, REQUIRES_CONSENSUS_V2, "{receipts:?}");
    }
    // A genesis without consensus v2 refuses every height.
    genesis.consensus_v2_activation_height = None;
    let batch = GovernanceActionBatch::new("export-root-no-v2", vec![export_root_amendment(&genesis, &validators, 9, 1)]);
    assert!(ensure_governance_batch_lifecycle_ready(&genesis, &batch, 1).is_err());
}

/// On a chain with consensus v2 from height 6, the governed round refuses an
/// activation at 5 at admission and accepts one at 6; the chain then replays.
#[test]
fn external_lane_export_root_governed_activation_at_consensus_v2_replays() {
    let chain = lane_chain_with_genesis("export-root-governed-v2", 86_400, Some(6), 4, |_| {});
    let validators = local_validator_ids(4).expect("validators");
    let slot = tip_height(&chain) + 1;
    let ratify = |value: u32, tag: &str| {
        let amendment_file = chain.data_dir.join(format!("{tag}.unsigned.json"));
        let amendment = export_root_amendment(&chain.genesis, &validators, value, slot);
        fs::write(&amendment_file, serde_json::to_vec_pretty(&amendment).expect("json")).expect("write");
        sign_and_apply_governance(&chain, amendment_file, &validators, tag)
    };
    let error = ratify(5, "export-root-5").expect_err("below consensus v2").to_string();
    assert!(error.contains(REQUIRES_CONSENSUS_V2), "{error}");
    assert_eq!(tip_height(&chain), slot - 1);
    let receipts = ratify(6, "export-root-6").expect("at consensus v2");
    assert!(receipts[0].accepted, "{receipts:?}");
    let governance = NodeStore::new(&chain.data_dir).read_governance().expect("governance");
    assert_eq!(postfiat_types::external_lane_export_root_activation_height(&chain.genesis, &governance), Some(6));
    verify_blocks(NodeOptions { data_dir: chain.data_dir.clone() }).expect("replay with the accepted amendment");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// The node's ratify command for the export-root activation, driven through
/// the governed round on six validators as the lane amendment rehearsal is
/// (`lane_amendment_ratify_command_on_six_validators_accepts_the_registry_threshold`):
/// the kind needs all six; a height at its own block is refused; a ratified
/// height becomes the governed activation, the block before it has no root
/// and refuses an export, and the block at it carries the root and accepts one.
#[test]
fn external_lane_export_root_ratify_command_activates_the_root_at_its_height() {
    let chain = lane_chain_with_genesis("export-root-cli", 86_400, Some(5), 6, |genesis| {
        genesis.external_lanes.as_mut().expect("lanes")[0].policy.exports_enabled = true;
    });
    let receipt = admit_and_apply(&chain, &release_tx(&chain, "base-erc20", 1, ACCOUNT_RESERVE + 1_000, 1), "release");
    assert!(receipt.accepted, "{receipt:?}");
    let validators = local_validator_ids(6).expect("validator ids");
    let slot = tip_height(&chain) + 1;
    assert_eq!(slot, 3);
    let kind = postfiat_types::GOVERNANCE_KIND_EXTERNAL_LANE_EXPORT_ROOT_ACTIVATION_HEIGHT;
    let ratify = |value: u32, support: &[String], file: &str| {
        ratify_governance(RatifyGovernanceOptions {
            data_dir: chain.data_dir.clone(),
            validators: validators.clone(),
            support: support.to_vec(),
            kind: kind.to_string(),
            value,
            activation_height: slot,
            veto_until_height: 0,
            paused: false,
            amendment_file: chain.data_dir.join(file),
        })
    };
    let error = ratify(5, &validators[1..], "root-five.json").expect_err("five of six").to_string();
    assert!(error.contains("need 6"), "{error}");

    // A height at the amendment's own block is refused.
    let own_block = u32::try_from(slot).expect("slot");
    let refused = ratify(own_block, &validators, "root-own.json")
        .and_then(|_| sign_and_apply_governance(&chain, chain.data_dir.join("root-own.json"), &validators, "root-own"))
        .expect_err("activation at its own block")
        .to_string();
    assert!(refused.contains("invalid_external_lane_export_root_activation_height"), "{refused}");
    assert_eq!(tip_height(&chain), slot - 1);

    let amendment = ratify(5, &validators, "root-5.json").expect("six of six ratified");
    assert_eq!((amendment.kind.as_str(), amendment.value, amendment.quorum), (kind, 5, 6));
    let receipts =
        sign_and_apply_governance(&chain, chain.data_dir.join("root-5.json"), &validators, "root-5").expect("admitted");
    assert!(receipts[0].accepted, "{receipts:?}");
    let governance = NodeStore::new(&chain.data_dir).read_governance().expect("governance");
    assert_eq!(governance.external_lane_export_root_activation_height(), Some(5));
    assert_eq!(postfiat_types::external_lane_export_root_activation_height(&chain.genesis, &governance), Some(5));

    // Block 4, before the height: no root, and an export is refused.
    let early = export_tx(&chain, "base-erc20", 7, &base_recipient());
    refused_everywhere(&chain, &early, postfiat_types::EXTERNAL_LANE_EXPORT_ROOT_NOT_ACTIVE, "root-early");
    let receipt = admit_and_apply(&chain, &release_tx(&chain, "base-erc20", 2, 1_000, faucet_sequence(&chain)), "release-4");
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(header_at(&chain, 4).external_lane_export_root, None);

    // Block 5, at the height: the export is accepted and the header has its root.
    let receipt = admit_and_apply(&chain, &export_tx(&chain, "base-erc20", 7, &base_recipient()), "export-5");
    assert!(receipt.accepted, "{receipt:?}");
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
    let leaf = ledger.external_lanes[0].exports[0].export_root_leaf(&chain.genesis.chain_id).expect("leaf");
    assert_eq!(
        header_at(&chain, 5).external_lane_export_root,
        Some(postfiat_types::external_lane_export_root_v1(&[leaf]).expect("root"))
    );
    verify_blocks(NodeOptions { data_dir: chain.data_dir.clone() }).expect("replay verifies the governed root");
    let _ = fs::remove_dir_all(&chain.data_dir);
}
