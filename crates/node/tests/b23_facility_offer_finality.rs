use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign,
    MlDsa65KeyPair, ML_DSA_65_ALGORITHM,
};
use postfiat_execution::{genesis_hash, offer_transaction_tx_id};
use postfiat_network::{local_topology, NetworkDomain};
use postfiat_node::{init, InitOptions};
use postfiat_rpc_sdk::{RpcRequest, RpcResponse};
use postfiat_storage::NodeStore;
use postfiat_types::*;
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Server {
    root: PathBuf,
    dir: PathBuf,
    ports: Vec<u16>,
    children: Vec<Child>,
    key: MlDsa65KeyPair,
    asset: String,
}
impl Drop for Server {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
impl Server {
    fn new(finality: bool, max_submits: u64) -> Self {
        let root = std::env::temp_dir().join(format!(
            "b23-offer-rpc-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = root.join("node");
        init(InitOptions {
            data_dir: dir.clone(),
            chain_id: "b23-offer-rpc".into(),
            node_id: "validator-0".into(),
            validator_count: 2,
        })
        .unwrap();
        let store = NodeStore::new(&dir);
        let genesis = store.read_genesis().unwrap();
        let mut ledger = store.read_ledger().unwrap();
        let key = ml_dsa_65_keygen_from_seed(&[0x81; 32]);
        let owner = address_from_public_key(&key.public_key);
        ledger.accounts.push(Account::new(
            owner.clone(),
            1_000_000,
            Some(bytes_to_hex(&key.public_key)),
        ));
        let issuer = ledger.accounts[0].address.clone();
        let asset = AssetDefinition::new(&genesis.chain_id, &issuer, "B23TEST", 1, 6).unwrap();
        ledger
            .trustlines
            .push(TrustLine::new(owner, issuer, asset.asset_id.clone(), 100_000, 0).unwrap());
        ledger.asset_definitions.push(asset.clone());
        store.write_ledger(&ledger).unwrap();
        let tip = dir.join("chain_tip.json");
        if tip.exists() {
            fs::remove_file(tip).unwrap();
        }
        fn copy_dir(source: &std::path::Path, target: &std::path::Path) {
            fs::create_dir_all(target).unwrap();
            for entry in fs::read_dir(source).unwrap() {
                let path = entry.unwrap().path();
                let target = target.join(path.file_name().unwrap());
                if path.is_dir() {
                    copy_dir(&path, &target);
                } else {
                    fs::copy(path, target).unwrap();
                }
            }
        }
        let peer_dir = root.join("node1");
        copy_dir(&dir, &peer_dir);
        let peer = NodeStore::new(&peer_dir);
        let mut state = peer.read_node_state().unwrap();
        state.node_id = "validator-1".into();
        peer.write_node_state(&state).unwrap();
        let base = loop {
            let first = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let base = first.local_addr().unwrap().port();
            if base > 65531 {
                continue;
            }
            let listeners = (1..4)
                .map(|offset| TcpListener::bind(("127.0.0.1", base + offset)))
                .collect::<Result<Vec<_>, _>>();
            if listeners.is_ok() {
                break base;
            }
        };
        let topology = local_topology(
            NetworkDomain {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
            },
            2,
            base,
        )
        .unwrap();
        let topology_path = root.join("topology.json");
        fs::write(&topology_path, serde_json::to_vec(&topology).unwrap()).unwrap();
        let mut server = Self {
            root,
            dir,
            ports: vec![base + 1, base + 3],
            children: vec![],
            key,
            asset: asset.asset_id,
        };
        for index in 0..2 {
            let data = if index == 0 {
                server.dir.clone()
            } else {
                peer_dir.clone()
            };
            let name = format!("validator-{index}");
            let combined: Value =
                serde_json::from_slice(&fs::read(data.join("validator_keys.json")).unwrap())
                    .unwrap();
            let record = combined["validators"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["node_id"] == name)
                .unwrap();
            let key_file = data.join("test-validator-key.json");
            fs::write(
                &key_file,
                serde_json::to_vec(&json!({"validators":[record]})).unwrap(),
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&key_file, fs::Permissions::from_mode(0o600)).unwrap();
            }
            let log = server.root.join(format!("{index}.transport.err"));
            let child = Command::new(env!("CARGO_BIN_EXE_postfiat-node"))
                .env("POSTFIAT_PREWARM_SHIELDED_VERIFIER", "1")
                .env("POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER", "1")
                .env(
                    "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER",
                    "1",
                )
                .args([
                    "transport-validator-serve",
                    "--unsafe-devnet-file-signer",
                    "--unsafe-devnet-json-storage",
                    "--data-dir",
                    data.to_str().unwrap(),
                    "--topology",
                    topology_path.to_str().unwrap(),
                    "--key-file",
                    key_file.to_str().unwrap(),
                    "--vote-dir",
                    data.join("votes").to_str().unwrap(),
                    "--max-connections",
                    "100",
                    "--timeout-ms",
                    "30000",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::from(fs::File::create(&log).unwrap()))
                .spawn()
                .unwrap();
            server.children.push(child);
            let deadline = Instant::now() + Duration::from_secs(20);
            while TcpStream::connect(("127.0.0.1", base + 2 * index as u16)).is_err() {
                if let Some(status) = server.children.last_mut().unwrap().try_wait().unwrap() {
                    panic!(
                        "transport exited {status}: {}",
                        fs::read_to_string(&log).unwrap()
                    );
                }
                assert!(Instant::now() < deadline, "transport listen timeout");
                thread::sleep(Duration::from_millis(20));
            }
            let ready = server.root.join(format!("{index}.rpc.ready"));
            let log = server.root.join(format!("{index}.rpc.err"));
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_postfiat-node"));
            cmd.args([
                "rpc-serve",
                "--unsafe-devnet-file-signer",
                "--unsafe-devnet-json-storage",
                "--disable-owned-lane",
                "--data-dir",
                data.to_str().unwrap(),
                "--spool-dir",
                data.join("rpc-spool").to_str().unwrap(),
                "--ready-file",
                ready.to_str().unwrap(),
                "--bind-host",
                "127.0.0.1",
                "--port",
                &server.ports[index].to_string(),
                "--max-requests",
                "100",
                "--timeout-ms",
                "30000",
                "--child-timeout-ms",
                "30000",
                "--finality-topology",
                topology_path.to_str().unwrap(),
                "--finality-key-file",
                key_file.to_str().unwrap(),
                "--finality-artifact-root",
                data.join("rounds").to_str().unwrap(),
                "--finality-timeout-ms",
                "10000",
                "--max-mempool-submit-per-peer",
                &max_submits.to_string(),
                "--max-mempool-submit-total",
                &max_submits.to_string(),
            ]);
            if finality {
                cmd.arg("--allow-mempool-submit-finality");
            }
            let child = cmd
                .stdout(Stdio::null())
                .stderr(Stdio::from(fs::File::create(&log).unwrap()))
                .spawn()
                .unwrap();
            server.children.push(child);
            server.wait_ready(&ready, &log);
        }
        server
    }
    fn wait_ready(&mut self, ready: &std::path::Path, log: &std::path::Path) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ready.exists() {
            if let Some(status) = self.children.last_mut().unwrap().try_wait().unwrap() {
                panic!(
                    "service exited {status}: {}",
                    fs::read_to_string(log).unwrap()
                );
            }
            assert!(
                Instant::now() < deadline,
                "ready timeout: {}",
                fs::read_to_string(log).unwrap()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn call(&self, method: &str, params: Value) -> RpcResponse {
        let req = RpcRequest::new(method, method, params);
        let proposer = postfiat_node::block_proposer(postfiat_node::BlockProposerOptions {
            data_dir: self.dir.clone(),
            block_height: NodeStore::try_new_read_only(&self.dir)
                .unwrap()
                .read_blocks()
                .unwrap()
                .blocks
                .len() as u64
                + 1,
            view: 0,
        })
        .unwrap()
        .proposer;
        let index = proposer
            .strip_prefix("validator-")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", self.ports[index])).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        serde_json::to_writer(&mut stream, &req).unwrap();
        stream.write_all(b"\n").unwrap();
        stream.flush().unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        let response: RpcResponse = serde_json::from_str(&line).unwrap();
        if response
            .result
            .as_ref()
            .is_some_and(|r| r["finality"]["confirmed"] == true)
        {
            let target = response.result.as_ref().unwrap()["finality"]["block"]["header"]
                ["block_hash"]
                .as_str()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let ready = [self.dir.clone(), self.root.join("node1")]
                    .iter()
                    .all(|dir| {
                        NodeStore::try_new_read_only(dir)
                            .unwrap()
                            .read_blocks()
                            .is_ok_and(|blocks| blocks.tip_hash() == target)
                    });
                if ready {
                    break;
                }
                assert!(Instant::now() < deadline, "replica convergence timeout");
                thread::sleep(Duration::from_millis(20));
            }
        }
        response
    }
    fn create(&self, amount: u64) -> OfferTransactionOperation {
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: address_from_public_key(&self.key.public_key),
            taker_gets_asset_id: "PFT".into(),
            taker_gets_amount: amount,
            taker_pays_asset_id: self.asset.clone(),
            taker_pays_amount: 30,
            expiration_height: 100,
        })
    }
    fn sign_quoted(&self, op: OfferTransactionOperation) -> SignedOfferTransaction {
        let source = address_from_public_key(&self.key.public_key);
        let quote = self.call(
            "offer_fee_quote",
            json!({"source":source,"operation_json":serde_json::to_string(&op).unwrap()}),
        );
        assert!(quote.ok, "quote: {:?}", quote.error);
        let q = quote.result.unwrap();
        let unsigned = UnsignedOfferTransaction {
            chain_id: q["chain_id"].as_str().unwrap().into(),
            genesis_hash: q["genesis_hash"].as_str().unwrap().into(),
            protocol_version: q["protocol_version"].as_u64().unwrap() as u32,
            address_namespace: ADDRESS_NAMESPACE.into(),
            transaction_kind: op.transaction_kind().into(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.into(),
            source,
            fee: q["minimum_fee"].as_u64().unwrap(),
            sequence: q["sequence"].as_u64().unwrap(),
            operation: op,
        };
        let signature = ml_dsa_65_sign(&self.key.private_key, &unsigned.signing_bytes()).unwrap();
        SignedOfferTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.into(),
            public_key_hex: bytes_to_hex(&self.key.public_key),
            signature_hex: bytes_to_hex(&signature),
        }
    }
    fn submit(&self, tx: &SignedOfferTransaction) -> RpcResponse {
        self.call(
            "mempool_submit_signed_offer_transaction_finality",
            json!({"signed_offer_transaction_json":serde_json::to_string(tx).unwrap()}),
        )
    }
}

#[test]
fn b23_rpc_serve_offer_create_cancel_finality_returns_literal_receipts() {
    let server = Server::new(true, 20);
    let tx = server.sign_quoted(server.create(60));
    println!(
        "RELEASE2_CREATE_REQUEST={}",
        serde_json::to_string(&RpcRequest::new(
            "mempool_submit_signed_offer_transaction_finality",
            "mempool_submit_signed_offer_transaction_finality",
            json!({"signed_offer_transaction_json":serde_json::to_string(&tx).unwrap()})
        ))
        .unwrap()
    );
    let response = server.submit(&tx);
    assert!(response.ok, "create: {:?}", response.error);
    let result = response.result.as_ref().unwrap();
    assert_eq!(result["finality"]["confirmed"], true);
    assert_eq!(result["finality"]["receipt"]["accepted"], true);
    assert_eq!(result["tx_id"], offer_transaction_tx_id(&tx));
    let create_receipt = result["finality"]["receipt"].clone();
    println!(
        "RELEASE2_CREATE_RESPONSE={}",
        serde_json::to_string(&response).unwrap()
    );
    let store = NodeStore::try_new_read_only(&server.dir).unwrap();
    let ledger = store.read_ledger().unwrap();
    assert_eq!(ledger.offers.len(), 1);
    let id = ledger.offers[0].offer_id.clone();
    let cancel = server.sign_quoted(OfferTransactionOperation::OfferCancel(
        OfferCancelOperation {
            owner: tx.unsigned.source.clone(),
            offer_id: id,
        },
    ));
    println!(
        "RELEASE2_CANCEL_REQUEST={}",
        serde_json::to_string(&RpcRequest::new(
            "mempool_submit_signed_offer_transaction_finality",
            "mempool_submit_signed_offer_transaction_finality",
            json!({"signed_offer_transaction_json":serde_json::to_string(&cancel).unwrap()})
        ))
        .unwrap()
    );
    let response = server.submit(&cancel);
    assert!(response.ok, "cancel: {:?}", response.error);
    let result = response.result.as_ref().unwrap();
    assert_eq!(result["finality"]["confirmed"], true);
    assert_eq!(result["finality"]["receipt"]["accepted"], true);
    assert_eq!(result["tx_id"], offer_transaction_tx_id(&cancel));
    let cancel_receipt = result["finality"]["receipt"].clone();
    println!(
        "RELEASE2_CANCEL_RESPONSE={}",
        serde_json::to_string(&response).unwrap()
    );
    assert_eq!(
        store.read_ledger().unwrap().offers[0].state,
        OFFER_STATE_CANCELED
    );
    for (tx, returned) in [(tx, create_receipt), (cancel, cancel_receipt)] {
        let receipt = store
            .read_receipts()
            .unwrap()
            .into_iter()
            .find(|r| r.tx_id == offer_transaction_tx_id(&tx))
            .unwrap();
        assert!(receipt.accepted);
        assert_eq!(serde_json::to_value(receipt).unwrap(), returned);
    }
}

#[test]
fn b23_rpc_serve_rejected_offer_has_false_receipt_and_no_commit() {
    let server = Server::new(true, 20);
    let tx = server.sign_quoted(server.create(2_000_000));
    let store = NodeStore::try_new_read_only(&server.dir).unwrap();
    let before = store.read_ledger().unwrap();
    let response = server.submit(&tx);
    assert!(response.ok, "reject: {:?}", response.error);
    let result = response.result.as_ref().unwrap();
    assert_eq!(result["finality"]["confirmed"], false);
    assert_eq!(result["finality"]["receipt"]["accepted"], false);
    assert_eq!(result["round_ok"], false);
    assert!(result["finality"]["block"].is_null());
    assert_eq!(store.read_ledger().unwrap(), before);
    assert!(store.read_blocks().unwrap().blocks.is_empty());
    assert!(store
        .read_mempool()
        .unwrap()
        .pending_offer_transactions
        .is_empty());
    println!(
        "RELEASE2_REJECTED_RESPONSE={}",
        serde_json::to_string(&response).unwrap()
    );
}

#[test]
fn b23_rpc_serve_finality_only_receipt_read_and_offer_rate_limit() {
    let server = Server::new(true, 1);
    let response = server.call("pftl_uniswap_receipt", json!({"tx_id":"ff".repeat(48)}));
    assert!(!response.ok);
    let error = response.error.unwrap();
    assert_ne!(error.code, "rpc_method_not_allowed");
    assert!(
        error.message.contains("transaction receipt unavailable"),
        "{error:?}"
    );
    let tx = server.sign_quoted(server.create(2_000_000));
    let raw = server.call(
        "mempool_submit_signed_offer_transaction",
        json!({"signed_offer_transaction_json":serde_json::to_string(&tx).unwrap()}),
    );
    assert_eq!(raw.error.unwrap().code, "rpc_method_not_allowed");
    let first = server.submit(&tx);
    assert!(first.ok, "first: {:?}", first.error);
    assert_eq!(
        first.result.unwrap()["finality"]["receipt"]["accepted"],
        false
    );
    let second = server.call(
        "mempool_submit_signed_asset_transaction_finality",
        json!({"signed_asset_transaction_json":"{}"}),
    );
    assert!(second.error.unwrap().code.contains("rate_limited"));
}

#[test]
fn b23_rpc_serve_offer_finality_requires_finality_flag() {
    let server = Server::new(false, 20);
    let response = server.call(
        "mempool_submit_signed_offer_transaction_finality",
        json!({"signed_offer_transaction_json":"{}"}),
    );
    assert_eq!(response.error.unwrap().code, "rpc_method_not_allowed");
}

#[test]
fn b23_rpc_serve_offer_json_uses_bounded_signed_payload_limit() {
    for method in [
        "mempool_submit_signed_offer_transaction",
        "mempool_submit_signed_offer_transaction_finality",
    ] {
        let request = RpcRequest::new(
            "size",
            method,
            json!({"signed_offer_transaction_json":"x".repeat(4097)}),
        );
        request.validate_protocol().unwrap();
        let request = RpcRequest::new(
            "size",
            method,
            json!({"signed_offer_transaction_json":"x".repeat(128*1024+1)}),
        );
        assert!(request.validate_protocol().is_err());
    }
}
