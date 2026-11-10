// The PFTL half of scripts/rehearse-lane-roundtrip.py: a generated
// six-validator lane chain whose blocks are certified by consensus v2 commits
// of all six validators, driven step by step through files the script writes
// (`POSTFIAT_LANE_ROUNDTRIP` names its config). Ignored: it needs the script's
// local Anvil.

/// Waits for `name` in the handshake directory, written by the script.
fn roundtrip_wait(dir: &Path, name: &str) -> Value {
    let path = dir.join(name);
    for _ in 0..12_000 {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(value) = serde_json::from_slice(&bytes) {
                return value;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("the rehearsal did not write {name}");
}

fn roundtrip_write(dir: &Path, name: &str, value: &Value) {
    let partial = dir.join(format!("{name}.partial"));
    fs::write(&partial, serde_json::to_vec_pretty(value).expect("json")).expect("write");
    fs::rename(partial, dir.join(name)).expect("rename");
}

/// The block header at `height`.
fn roundtrip_header(chain: &Chain, height: u64) -> BlockHeader {
    NodeStore::new(&chain.data_dir)
        .read_blocks()
        .expect("blocks")
        .blocks
        .into_iter()
        .find(|block| block.header.height == height)
        .expect("block")
        .header
}

/// The lane's counters, its remainder and the per-block conservation replay.
fn roundtrip_lane(chain: &Chain) -> Value {
    let row = lane_info(&chain.data_dir);
    let store = NodeStore::new(&chain.data_dir);
    let live = native_pft_live_total(
        &store.read_ledger().expect("ledger"),
        &store.read_shielded().expect("shielded"),
    )
    .expect("live");
    verify_blocks(NodeOptions {
        data_dir: chain.data_dir.clone(),
    })
    .expect("replay blocks");
    verify_state(NodeOptions {
        data_dir: chain.data_dir.clone(),
    })
    .expect("replay state");
    let state = &row.state;
    assert_eq!(
        row.remainder_atoms,
        row.config.reserved_atoms - (state.released_atoms - state.exported_atoms),
        "lane remainder"
    );
    json!({
        "reserved_atoms": row.config.reserved_atoms, "released_atoms": state.released_atoms,
        "exported_atoms": state.exported_atoms, "remainder_atoms": row.remainder_atoms,
        "next_export_nonce": state.next_export_nonce, "native_live_atoms": live,
        "tip_height": tip_height(chain), "blocks_and_state_replayed": true,
    })
}

#[test]
#[ignore = "driven by scripts/rehearse-lane-roundtrip.py on a local Anvil"]
fn local_evm_lane_roundtrip() {
    let config_path = std::env::var("POSTFIAT_LANE_ROUNDTRIP").expect("round-trip config");
    let config: Value = serde_json::from_slice(&fs::read(config_path).unwrap()).unwrap();
    let url = config["rpc"].as_str().unwrap();
    assert!(url.starts_with("http://127.0.0.1:"), "loopback only");
    let dir = PathBuf::from(config["handshake_dir"].as_str().unwrap());
    let cast = |args: &[&str]| -> Value {
        let result = std::process::Command::new("cast")
            .args(args)
            .args(["--rpc-url", url])
            .output()
            .expect("cast");
        assert!(
            result.status.success(),
            "cast failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice(&result.stdout).expect("cast JSON")
    };
    assert_eq!(cast(&["rpc", "eth_chainId"]), "0x14a34");
    // Block 1 is the unanimous committee bootstrap; consensus v2 and the lane
    // export root start at block 2, the first block that can carry either.
    let chain = lane_chain_with_genesis("lane-roundtrip", 86_400, Some(2), 6, |genesis| {
        let policy = &mut genesis.external_lanes.as_mut().expect("lanes")[0].policy;
        policy.chain_id = 84_532;
        policy.lock_contract_address = config["lock"].as_str().unwrap().into();
        policy.lock_runtime_code_hash = config["lock_code_hash"].as_str().unwrap().into();
        policy.token_address = config["token"].as_str().unwrap().into();
        policy.token_runtime_code_hash = config["token_code_hash"].as_str().unwrap().into();
        policy.exports_enabled = true;
        genesis.external_lane_export_root_activation_height = Some(2);
    });
    let pins = chain
        .genesis
        .external_lane_verifier_pins("base-erc20")
        .expect("pins");
    let (domain, _) = live_consensus_v2_context(&chain.data_dir).expect("consensus v2 context");
    let checkpoint = roundtrip_header(&chain, tip_height(&chain));
    // The recipient's key, for `wallet-sign-asset-transaction`; it stays in the
    // generated data directory, which the script removes.
    let recipient_key = ml_dsa_65_keygen_from_seed(&[0x99; 32]);
    let key_file = chain.data_dir.join("roundtrip-recipient-key.json");
    fs::write(
        &key_file,
        serde_json::to_vec(&DevKeyFile {
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            address: chain.recipient.clone(),
            public_key_hex: bytes_to_hex(&recipient_key.public_key),
            private_key_hex: bytes_to_hex(&recipient_key.private_key),
        })
        .unwrap(),
    )
    .unwrap();
    roundtrip_write(
        &dir,
        "pftl-ready.json",
        &json!({
            "data_dir": chain.data_dir, "chain_id": chain.genesis.chain_id, "genesis_hash": genesis_hash(&chain.genesis),
            "protocol_version": chain.genesis.protocol_version, "validators": 6, "pins": pins,
            "checkpoint": {"block_id": checkpoint.block_hash, "height": checkpoint.height,
                           "committee_root": domain.committee_root, "committee_epoch": domain.committee_epoch},
            "recipient": chain.recipient, "recipient_key_file": key_file, "lane": roundtrip_lane(&chain),
        }),
    );

    // (b) The script deployed the verifier and the lock and locked on Base.
    let locked = roundtrip_wait(&dir, "base-locked.json");
    let tx = locked["transaction"].as_str().unwrap();
    let block = locked["block"].as_u64().unwrap();
    cast(&["rpc", "anvil_mine", "0x80"]);
    let checkpoint_file = dir.join("base-checkpoint.json");
    observe_external_lane_checkpoint(ExternalLaneCheckpointObserveOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".into(),
        base_rpc: url.into(),
        block_number: Some(block),
        checkpoint_file: checkpoint_file.clone(),
    })
    .expect("observe the Lock block");
    let mut votes = Vec::new();
    for index in 0..5 {
        let vote = dir.join(format!("base-checkpoint-vote-{index}.json"));
        sign_external_lane_checkpoint_vote(ExternalLaneCheckpointSignOptions {
            data_dir: chain.data_dir.clone(),
            checkpoint_file: checkpoint_file.clone(),
            base_rpc: url.into(),
            validator: format!("validator-{index}"),
            validator_key_file: committee_key_file(&chain, index, index),
            vote_file: vote.clone(),
        })
        .expect("re-observe and sign");
        votes.push(vote);
    }
    let certificate = dir.join("base-checkpoint-certificate.json");
    assemble_external_lane_checkpoint_certificate(ExternalLaneCheckpointAssembleOptions {
        data_dir: chain.data_dir.clone(),
        checkpoint_file: checkpoint_file.clone(),
        vote_files: votes,
        certificate_file: certificate.clone(),
    })
    .expect("assemble five votes");
    let operation = dir.join("release-operation.json");
    let submitter = read_transfer_key_file(&chain.data_dir, None)
        .unwrap()
        .address;
    build_external_lane_lock_proof(ExternalLaneLockProofOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".into(),
        base_rpc: url.into(),
        transaction_hash: tx.into(),
        certificate_file: certificate,
        submitter,
        log_index: None,
        operation_file: operation.clone(),
    })
    .expect("prove the Lock receipt");
    let release = admit_and_apply_with(
        &chain,
        &signed_release(&chain, &operation),
        "roundtrip-release",
        true,
    );
    assert!(release.accepted, "{release:?}");
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().unwrap();
    let balance = ledger.account(&chain.recipient).unwrap().balance;
    let release_height = tip_height(&chain);
    let release_lane = roundtrip_lane(&chain);
    // Fees for the two exports: the faucet sends the recipient 10,000 native atoms.
    let batch_file = chain.data_dir.join("roundtrip-fees.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: chain.data_dir.clone(),
        key_file: None,
        to: chain.recipient.clone(),
        amount: 10_000,
        batch_file: batch_file.clone(),
    })
    .expect("fee transfer batch");
    let certificate_file = Some(consensus_v2_certificate(
        &chain.data_dir,
        &batch_file,
        "roundtrip-fees",
    ));
    let fees = apply_batch(ApplyBatchOptions {
        data_dir: chain.data_dir.clone(),
        batch_file,
        certificate_file,
    })
    .expect("fee transfer");
    assert!(fees[0].accepted, "{fees:?}");
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().unwrap();
    roundtrip_write(
        &dir,
        "pftl-released.json",
        &json!({
            "release_receipt": release, "release_height": release_height, "recipient_balance_after_release": balance,
            "checkpoint_votes": 5, "lane_after_release": release_lane, "fee_top_up_atoms": 10_000,
            "recipient_sequence": ledger.account(&chain.recipient).unwrap().sequence,
            "recipient_balance": ledger.account(&chain.recipient).unwrap().balance, "lane": roundtrip_lane(&chain),
        }),
    );

    // (c), (f), (g) Exports the script signs with `wallet-sign-asset-transaction`.
    for (index, expect_accepted) in [(1, true), (2, true), (3, false)] {
        let signed = roundtrip_wait(&dir, &format!("export-{index}.signed.json"));
        let tx: SignedAssetTransaction = serde_json::from_value(signed).expect("signed export");
        let applied = if expect_accepted {
            let receipt =
                admit_and_apply_with(&chain, &tx, &format!("roundtrip-export-{index}"), true);
            assert!(receipt.accepted, "{receipt:?}");
            let header = roundtrip_header(&chain, tip_height(&chain));
            assert!(
                header.external_lane_export_root.is_some(),
                "the export block carries the lane export root"
            );
            let ledger = NodeStore::new(&chain.data_dir).read_ledger().unwrap();
            json!({
                "accepted": true, "receipt": receipt, "block_height": header.height, "block_id": header.block_hash,
                "external_lane_export_root": header.external_lane_export_root,
                "recipient_balance": ledger.account(&chain.recipient).unwrap().balance,
                "recipient_sequence": ledger.account(&chain.recipient).unwrap().sequence,
                "lane": roundtrip_lane(&chain),
            })
        } else {
            let error = submit_signed_asset_transaction_json_to_mempool(
                SignedAssetTransactionJsonSubmitOptions {
                    data_dir: chain.data_dir.clone(),
                    signed_asset_transaction_json: serde_json::to_string(&tx).expect("tx json"),
                },
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(postfiat_types::EXTERNAL_LANE_EXPORT_ABOVE_RELEASED),
                "{error}"
            );
            json!({"accepted": false, "mempool_error": error, "lane": roundtrip_lane(&chain)})
        };
        roundtrip_write(&dir, &format!("export-{index}.applied.json"), &applied);
    }
    roundtrip_write(
        &dir,
        "pftl-done.json",
        &json!({"lane": roundtrip_lane(&chain), "data_dir": chain.data_dir}),
    );
}
