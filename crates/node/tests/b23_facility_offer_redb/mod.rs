use super::*;
use postfiat_storage::{
    transactional::NamedStateValue, CurrentStateUpdate, OrderedHistoryCommitment,
};

// Seed only this disposable genesis fixture, before services start. Reinitialize
// the authenticated redb genesis after adding the test holder and trust line.
pub(super) fn initialize_fixture_storage(store: &NodeStore) {
    let registry = store
        .transactional_store()
        .unwrap()
        .current_state_raw("validator_registry")
        .unwrap()
        .unwrap();
    fs::remove_file(
        store
            .data_dir()
            .join(postfiat_storage::transactional::TRANSACTIONAL_DATABASE_FILE),
    )
    .unwrap();
    postfiat_node::status(postfiat_node::NodeOptions {
        data_dir: store.data_dir().to_owned(),
    })
    .unwrap();
    let tip = store.read_chain_tip().unwrap();
    let genesis = store.read_genesis().unwrap();
    let ledger = store.read_ledger().unwrap();
    let governance = store.read_governance().unwrap();
    let shielded = store.read_shielded().unwrap();
    let bridge = store.read_bridge().unwrap();
    let state = store.read_node_state().unwrap();
    let commitment = OrderedHistoryCommitment::genesis(
        &genesis.chain_id,
        &genesis_hash(&genesis),
        genesis.protocol_version,
    )
    .unwrap();
    let db = store.transactional_store().unwrap();
    db.initialize_with_activation(
        &tip,
        &commitment,
        CurrentStateUpdate {
            ledger: Some(&ledger),
            governance: Some(&governance),
            shielded: Some(&shielded),
            bridge: Some(&bridge),
            node_state: Some(&state),
            additional: &[NamedStateValue {
                domain: "validator_registry".into(),
                canonical_bytes: registry,
            }],
        },
        Some(1),
    )
    .unwrap();
    db.verify_and_mark_full_integrity().unwrap();
}

#[test]
fn b23_rpc_serve_redb_offer_create_cancel_and_rejection() {
    let server = Server::with_backend(true, 20, true);
    let first = server.sign_quoted(server.create(60));
    let response = server.submit(&first);
    assert!(response.ok, "create: {:?}", response.error);
    assert_eq!(
        response.result.as_ref().unwrap()["finality"]["receipt"]["accepted"],
        true
    );
    let store = NodeStore::new(&server.dir);
    assert!(store.transactional_storage_active().unwrap());
    let info = server.call("server_info", json!({}));
    assert!(info.ok, "server_info: {:?}", info.error);
    assert_eq!(info.result.as_ref().unwrap()["storage"]["backend"], "redb");
    assert_eq!(
        info.result.as_ref().unwrap()["storage"]["transactional_active"],
        true
    );
    let offer_id = store.read_ledger().unwrap().offers[0].offer_id.clone();
    let cancel = server.sign_quoted(OfferTransactionOperation::OfferCancel(
        OfferCancelOperation {
            owner: first.unsigned.source.clone(),
            offer_id,
        },
    ));
    let response = server.submit(&cancel);
    assert!(response.ok, "cancel: {:?}", response.error);
    assert_eq!(
        response.result.as_ref().unwrap()["finality"]["receipt"]["accepted"],
        true
    );
    let returned = response.result.as_ref().unwrap()["finality"]["receipt"].clone();
    let stored = store
        .read_receipts()
        .unwrap()
        .into_iter()
        .find(|r| r.tx_id == offer_transaction_tx_id(&cancel))
        .unwrap();
    assert_eq!(returned, serde_json::to_value(stored).unwrap());
    assert_eq!(
        store.read_ledger().unwrap().offers[0].state,
        OFFER_STATE_CANCELED
    );
    // A second creation exercises admission against an already activated DB.
    let second = server.sign_quoted(server.create(30));
    let response = server.submit(&second);
    assert!(response.ok, "second create: {:?}", response.error);
    assert_eq!(
        response.result.as_ref().unwrap()["finality"]["receipt"]["accepted"],
        true
    );
    let bad = server.sign_quoted(server.create(2_000_000));
    let before = store.read_chain_tip().unwrap();
    let ledger_before = store.read_ledger().unwrap();
    let response = server.submit(&bad);
    assert!(response.ok, "rejected offer: {:?}", response.error);
    let result = response.result.unwrap();
    assert_eq!(result["finality"]["receipt"]["accepted"], false);
    assert_eq!(result["finality"]["confirmed"], false);
    assert_eq!(
        result["finality"]["verification_mode"],
        "mempool-admission-preview"
    );
    assert_eq!(store.read_chain_tip().unwrap(), before);
    assert_eq!(store.read_ledger().unwrap(), ledger_before);
    println!("redb rpc-serve: create, cancel, second create accepted; rejection preserved ledger/tip; stored receipt matched");
}
