use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

fn deployment() -> Deployment {
    let raw: Value = serde_json::from_str(include_str!(
        "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/mainnet-deposit.json"
    ))
    .unwrap();
    let inputs: ProofInputs = serde_json::from_value(raw["helios"].clone()).unwrap();
    let profile: VaultBridgeRouteProfileV1 = serde_json::from_str(include_str!(
        "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/mainnet-route.json"
    ))
    .unwrap();
    serde_json::from_value(json!({
        "vault": raw["evidence"]["vault_address"],
        "deposit_tx": raw["evidence"]["tx_hash"],
        "amount_atoms": raw["evidence"]["amount_atoms"],
        "recipient": raw["evidence"]["pftl_recipient"],
        "nonce": raw["evidence"]["nonce"],
        "route_binding": raw["evidence"]["route_binding"],
        "creation_bytecode_hash": "11".repeat(32),
        "beacon_anchor": {
            "schema": postfiat_types::PFUSDC_BEACON_FINALITY_STATE_SCHEMA_V1,
            "source_chain_id": 1, "route_profile_hash": profile.profile_hash().unwrap(),
            "route_epoch": profile.route_epoch, "vault_address": profile.vault_address,
            "token_address": profile.token_address, "route_binding": raw["evidence"]["route_binding"],
            "finalized_beacon_root": hex::encode(inputs.store.finalized_header.beacon().tree_hash_root()),
            "finalized_slot": inputs.store.finalized_header.beacon().slot,
            "current_sync_committee_root": hex::encode(inputs.store.current_sync_committee.tree_hash_root())
        }
    })).unwrap()
}

#[test]
fn capture_rejects_missing_and_wrong_epoch_ledger_anchors() {
    let mut d = deployment();
    validate_capture_anchor(&d).unwrap();
    d.beacon_anchor.route_epoch += 1;
    assert!(validate_capture_anchor(&d).is_err());
    let mut raw = serde_json::json!({"vault": d.vault});
    raw["beacon_anchor"] = Value::Null;
    assert!(serde_json::from_value::<Deployment>(raw).is_err());
}

#[tokio::test]
async fn capture_requests_exact_ledger_root_and_never_falls_back_to_recent_checkpoint() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/mainnet-deposit.json"
    ))
    .unwrap();
    let finality_body =
        serde_json::to_string(&json!({"data": fixture["helios"]["finality_update"]})).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let mut d = deployment();
    // Two independently supplied ledger snapshots must drive two exact roots.
    let roots = [
        d.beacon_anchor.finalized_beacon_root.clone(),
        "ef".repeat(32),
    ];
    let server = std::thread::spawn(move || {
        let mut paths = Vec::new();
        for index in 0..4 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                bytes.push(byte[0]);
            }
            let request = String::from_utf8(bytes).unwrap();
            paths.push(request.split_whitespace().nth(1).unwrap().to_owned());
            let (status, body) = if index % 2 == 0 {
                ("200 OK", finality_body.as_str())
            } else {
                ("404 Not Found", "{}")
            };
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
        paths
    });
    let rpc = Rpc::new().unwrap();
    for root in &roots {
        d.beacon_anchor.finalized_beacon_root = root.clone();
        let error = capture_helios(&rpc, &endpoint, &d.beacon_anchor)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("404"), "{error}");
    }
    let paths = server.join().unwrap();
    assert_eq!(
        paths,
        vec![
            "/eth/v1/beacon/light_client/finality_update".to_owned(),
            format!("/eth/v1/beacon/light_client/bootstrap/0x{}", roots[0]),
            "/eth/v1/beacon/light_client/finality_update".to_owned(),
            format!("/eth/v1/beacon/light_client/bootstrap/0x{}", roots[1]),
        ]
    );
}

#[tokio::test]
#[ignore = "real SP1 VM execution; run explicitly against the embedded release ELF"]
async fn rebuilt_guest_executes_honest_witness_and_rejects_swapped_committee() {
    let mut raw: Value = serde_json::from_str(include_str!(
        "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/mainnet-deposit.json"
    ))
    .unwrap();
    raw["beacon_anchor"] = serde_json::to_value(deployment().beacon_anchor).unwrap();
    let mut witness: EthIngressWitnessV1 = serde_json::from_value(raw).unwrap();
    let expected = serde_cbor::to_vec(&verify_witness(&witness).unwrap()).unwrap();
    let elf = IngressRoute::Pfusdc.elf().unwrap();
    let client = ProverClient::builder().light().build().await;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(serde_cbor::to_vec(&witness).unwrap());
    let (public, report) = client.execute(elf.clone(), stdin).await.unwrap();
    assert_eq!(report.exit_code, 0);
    assert_eq!(public.to_vec(), expected);
    println!(
        "{}",
        json!({"honest_guest_execution": "passed", "instruction_count": report.total_instruction_count(),
        "elf_sha256": hex::encode(Sha256::digest(&*elf)), "public_values_sha256": hex::encode(Sha256::digest(&expected))})
    );
    witness.helios.store.current_sync_committee =
        witness.helios.updates[0].next_sync_committee().clone();
    assert!(verify_witness(&witness)
        .unwrap_err()
        .contains("sync committee"));
    let mut bad_stdin = SP1Stdin::new();
    bad_stdin.write_vec(serde_cbor::to_vec(&witness).unwrap());
    // SP1 6.3 execute reports guest panic through exit_code, not necessarily Err.
    let (bad_public, bad_report) = client.execute(elf, bad_stdin).await.unwrap();
    assert_ne!(bad_report.exit_code, 0);
    assert!(bad_public.to_vec().is_empty());
    println!(
        "{}",
        json!({"swapped_committee_with_honest_header": "rejected",
        "guest_exit_code": bad_report.exit_code, "committed_public_values_bytes": bad_public.to_vec().len()})
    );
}

#[test]
fn legacy_pfeth_guest_cannot_be_used_with_new_ingress_abi() {
    assert!(IngressRoute::Pfeth
        .elf()
        .err()
        .unwrap()
        .to_string()
        .contains("must be rebuilt"));
}
