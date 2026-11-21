use super::*;
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen, ml_dsa_65_sign, ML_DSA_65_ALGORITHM,
};
use postfiat_storage::NodeStore;
use postfiat_types::{
    OfferCancelOperation, OfferTransactionOperation, SignedOfferTransaction,
    UnsignedOfferTransaction, ADDRESS_NAMESPACE,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn rpc_serve_offer_preview_reuses_live_redb_handle() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!("b23-rpc-redb-held-{}-{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())));
    init_consensus_v2(InitConsensusV2Options {
        data_dir: fixture.0.clone(),
        chain_id: "b23-held-redb".into(),
        node_id: "validator-0".into(),
        validator_count: 2,
        activation_height: 1,
        storage_activation_height: Some(1),
    })
    .unwrap();
    let store = NodeStore::new(&fixture.0);
    let genesis = store.read_genesis().unwrap();
    let proposer = block_proposer(BlockProposerOptions {
        data_dir: fixture.0.clone(),
        block_height: 1,
        view: 0,
    })
    .unwrap()
    .proposer;
    let mut state = store.read_node_state().unwrap();
    state.node_id = proposer;
    store.write_node_state(&state).unwrap();
    let topology = postfiat_network::local_topology(
        postfiat_network::NetworkDomain {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: postfiat_execution::genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
        },
        2,
        40000,
    )
    .unwrap();
    let topology_file = fixture.0.join("topology.json");
    std::fs::write(&topology_file, serde_json::to_vec(&topology).unwrap()).unwrap();
    let key = ml_dsa_65_keygen().unwrap();
    let owner = address_from_public_key(&key.public_key);
    let operation = OfferTransactionOperation::OfferCancel(OfferCancelOperation {
        owner: owner.clone(),
        offer_id: "42".repeat(48),
    });
    let unsigned = UnsignedOfferTransaction {
        chain_id: genesis.chain_id,
        genesis_hash: postfiat_execution::genesis_hash(&store.read_genesis().unwrap()),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.into(),
        transaction_kind: operation.transaction_kind().into(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.into(),
        source: owner,
        fee: 100_000,
        sequence: 1,
        operation,
    };
    let signature = ml_dsa_65_sign(&key.private_key, &unsigned.signing_bytes()).unwrap();
    let signed = SignedOfferTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.into(),
        public_key_hex: bytes_to_hex(&key.public_key),
        signature_hex: bytes_to_hex(&signature),
    };
    // Keep the actual process-shared redb writer lease alive across the real TCP
    // server's read and finality requests. An offline read-only open must fail.
    let held_db = store.transactional_store().unwrap();
    let ledger_before = store.read_ledger().unwrap();
    let tip_before = store.read_chain_tip().unwrap();
    held_db.reset_work_counters();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let root = fixture.0.clone();
    let server = std::thread::spawn(move || {
        rpc_serve_inner(
            RpcServeOptions {
                data_dir: root.clone(),
                spool_dir: root.join("spool"),
                ready_file: root.join("ready.json"),
                bind_host: "127.0.0.1".into(),
                port,
                max_requests: 2,
                timeout_ms: 10_000,
                child_timeout_ms: 10_000,
                event_log: None,
                allow_mempool_submit: false,
                allow_mempool_submit_finality: true,
                allow_orchard_batch_create: false,
                allow_private_action_admission: false,
                owned_lane_enabled: true,
                finality_topology_file: topology_file,
                finality_key_file: root.join(VALIDATOR_KEYS_FILE),
                finality_signer_socket: None,
                finality_proposal_key_file: None,
                finality_artifact_root: root.join("rounds"),
                finality_timeout_ms: 10_000,
                finality_send_retries: 0,
                finality_retry_backoff_ms: 0,
                finality_vote_phase_deadline_ms: PEER_VOTE_PHASE_DEADLINE_MS,
                finality_quorum_early_full_propagation: false,
                max_mempool_submit_per_peer: 8,
                max_mempool_submit_total: 8,
                max_orchard_batch_create_per_peer: 1,
                max_orchard_batch_create_total: 1,
                max_orchard_batch_create_concurrent: 1,
                max_child_dispatch_concurrent: 2,
                max_child_dispatch_per_peer: 2,
                keep_alive: false,
            },
            Some(listener),
        )
    });
    let call = |request: RpcRequest| {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        serde_json::to_writer(&mut stream, &request).unwrap();
        stream.write_all(b"\n").unwrap();
        let mut response = String::new();
        BufReader::new(stream).read_line(&mut response).unwrap();
        serde_json::from_str::<RpcResponse>(&response).unwrap()
    };
    let health = call(RpcRequest::new(
        "health",
        "status",
        serde_json::json!({}),
    ));
    let response = call(RpcRequest::new(
        "offer",
        "mempool_submit_signed_offer_transaction_finality",
        serde_json::json!({"signed_offer_transaction_json":serde_json::to_string(&signed).unwrap()}),
    ));
    server.join().unwrap().unwrap();
    assert!(health.ok, "health: {:?}", health.error);
    assert_eq!(held_db.meta().unwrap().backend, "redb");
    assert!(
        response.ok,
        "offer through running rpc-serve with held redb: {:?}",
        response.error
    );
    let result = response.result.unwrap();
    assert_eq!(result["finality"]["receipt"]["accepted"], false);
    assert_eq!(result["finality"]["confirmed"], false);
    assert_eq!(
        result["finality"]["verification_mode"],
        "mempool-admission-preview"
    );
    assert_eq!(result["finality"]["receipt"]["code"], "missing_sender");
    assert_eq!(store.read_ledger().unwrap(), ledger_before);
    assert_eq!(store.read_chain_tip().unwrap(), tip_before);
    assert_eq!(held_db.work_counters().committed_write_transactions, 0);
    println!("real rpc-serve with held redb: literal missing_sender rejection; no committed DB writes or tip/ledger change");
}
