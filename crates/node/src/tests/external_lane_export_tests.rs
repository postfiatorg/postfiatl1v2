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
/// header without the lane export root, pinned to the values main's code gives
/// (computed on main 5c8f693ca): this branch leaves those encodings unchanged.
#[test]
fn block_encodings_without_the_export_root_hash_as_on_main() {
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
    let (bridge, uniswap) = ("e".repeat(96), "f".repeat(96));
    let mut values = Vec::new();
    for (bridge_exit_root, pftl_uniswap_receipt_root) in
        [(None, None), (Some(bridge.as_str()), None), (Some(bridge.as_str()), Some(uniswap.as_str()))]
    {
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
            fastpay_pre_state_effects: &[],
        };
        values.push(block_hash(&genesis, &evidence, &"c".repeat(96)).expect("block hash"));
        values.push(block_proposal_hash_from_evidence(&genesis, &evidence, &"d".repeat(96)).expect("proposal hash"));
        let vote = block_certificate_vote_message(&genesis, &evidence, "validator-0", true, &"a".repeat(96))
            .expect("vote message");
        values.push(postfiat_crypto_provider::hash_hex("postfiat.test.vote_message_pin", &vote));
        let block = match (bridge_exit_root, pftl_uniswap_receipt_root) {
            (None, _) => postfiat_ordering_fast::consensus_v2_block_ref(&domain, 5, "1".repeat(96), "2".repeat(96), "3".repeat(96)),
            (Some(bridge), None) => postfiat_ordering_fast::consensus_v2_block_ref_with_bridge_exit_root(
                &domain, 5, "1".repeat(96), "2".repeat(96), "3".repeat(96), bridge,
            ),
            (Some(bridge), Some(uniswap)) => postfiat_ordering_fast::consensus_v2_block_ref_with_commitment_roots(
                &domain, 5, "1".repeat(96), "2".repeat(96), "3".repeat(96), Some(bridge.to_string()), uniswap,
            ),
        };
        values.push(block.expect("consensus v2 block").block_id);
    }
    // Per header variant (no root, bridge-exit root, both roots): block hash,
    // proposal hash, vote-message digest, consensus v2 block id.
    let expected = [
        "34fea2e2ad5af642c0d764eb5009fc2a4500dbfef37c38125d925bb94aed159071a0144a94843f0579592cdd93f077f9",
        "6e88a13f5470ac7ed85107370b3c7eea9b09d41a339f7aca1a3e27577fbbc424862e0a8e76a05fa7c84d918de5f1ac09",
        "3708b97f9061746d0abc8a75242b2734d8920a116fdd88f346d709f98ef9f0002703b4adf6f5577730309644b1f8d40e",
        "22905f922156b15ae3a648451171cc7ff34086b69493082de8ae3cd400a80b6a18266ed9e90c2d111381c1bdf180a97f",
        "67b352f21b847efc9efdee426ef5faded4236411dd32189c02b37ca4cb54ba9a9797a9ce7a0ce1f395a624ce1c247626",
        "4ce2c2174495bfc6ce801862b1d584f2ce2ab9373420f465d7c25cc08c16a78933e55b4fb9f886e00969577ffb368c95",
        "90ab2160020855bb5cd608fbba4b124d76d110d91e9eeac8d0e4af4508c60649e806161bbbc509a00b45980630277b18",
        "8bf2b730aa8769993e3ba862a8b58041a7bbaa80917c6ab5146a5d2bd3783fce65db59f88b4cffd5243e7c2a7bf54cd7",
        "d664052b432488ac3ab7d3552610dfd8266cf7a55647fdf582a3433e9e0bf1dd3c91e700f9a64532a3ef4249ef032ea9",
        "15ea0b1781104df5813deec1b76a16c6d2280cd7ba1055f98d131ce089c111d353e32e151dcca9df510395d0401ce7f1",
        "a9c6c9f50f1c4c38abb6919ad7d72583215b9dfa1ecce4cdb82ce5a120ba8e7dfd3ccd46780d675d0e02b400b584de45",
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
