// Real EVM transactions on a disposable loopback Anvil instance. This is
// deliberately ignored in the ordinary unit suite; the Python runner owns it.
#[test]
#[ignore = "requires scripts/rehearse-base-lock.py and disposable Anvil contracts"]
fn local_evm_lock_checkpoint_release_and_replay_refusal() {
    let config_path = std::env::var("POSTFIAT_LOCAL_EVM_REHEARSAL").expect("local rehearsal config");
    let config: Value = serde_json::from_slice(&fs::read(config_path).unwrap()).unwrap();
    let url = config["rpc"].as_str().unwrap();
    let port: u16 = url.strip_prefix("http://127.0.0.1:").expect("loopback only").parse().expect("port only");
    assert!(port > 1024);
    let out = PathBuf::from(config["output"].as_str().unwrap());
    let cast = |args: &[&str]| -> Value {
        let result = std::process::Command::new("cast").args(args).args(["--rpc-url", url]).output().expect("cast");
        assert!(result.status.success(), "cast failed: {}", String::from_utf8_lossy(&result.stderr));
        serde_json::from_slice(&result.stdout).expect("cast JSON")
    };
    assert!(cast(&["rpc", "web3_clientVersion"]).as_str().unwrap().to_lowercase().contains("anvil"));
    assert_eq!(cast(&["rpc", "eth_chainId"]), "0x14a34"); // 84532, local simulation only
    let lock = config["lock"].as_str().unwrap();
    let token = config["token"].as_str().unwrap();
    let holder = config["holder"].as_str().unwrap();
    // The unanimously authorized committee bootstrap is block 1; consensus-v2
    // begins at block 2, which certifies the actual release below.
    let chain = lane_chain_configured("local-evm", 86_400, Some(2), 6, |policy| {
        policy.chain_id = 84_532;
        policy.lock_contract_address = lock.to_string();
        policy.token_address = token.to_string();
        policy.lock_runtime_code_hash = config["lock_code_hash"].as_str().unwrap().into();
        policy.token_runtime_code_hash = config["token_code_hash"].as_str().unwrap().into();
    });
    let quantity = amount().to_string();
    let approve = cast(&["send", token, "approve(address,uint256)", lock, &quantity, "--unlocked", "--from", holder, "--json"]);
    assert_eq!(approve["status"], "0x1");
    let recipient_hex = format!("0x{}", bytes_to_hex(chain.recipient.as_bytes()));
    let receipt = cast(&["send", lock, "lock(uint64,bytes)", &quantity, &recipient_hex, "--unlocked", "--from", holder, "--json"]);
    assert_eq!(receipt["status"], "0x1");
    let tx = receipt["transactionHash"].as_str().unwrap();
    let block = u64::from_str_radix(receipt["blockNumber"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
    cast(&["rpc", "anvil_mine", "0x80"]);
    let checkpoint = out.join("checkpoint.json");
    observe_external_lane_checkpoint(ExternalLaneCheckpointObserveOptions {
        data_dir:chain.data_dir.clone(), lane_id:"base-erc20".into(), base_rpc:url.into(),
        block_number:Some(block), checkpoint_file:checkpoint.clone(),
    }).expect("observe real EVM block");
    let mut votes = Vec::new();
    for index in 0..5 {
        let vote = out.join(format!("checkpoint-vote-{index}.json"));
        sign_external_lane_checkpoint_vote(ExternalLaneCheckpointSignOptions {
            data_dir:chain.data_dir.clone(), checkpoint_file:checkpoint.clone(), base_rpc:url.into(),
            validator:format!("validator-{index}"), validator_key_file:committee_key_file(&chain,index,index), vote_file:vote.clone(),
        }).expect("reobserve and sign test checkpoint");
        votes.push(vote);
    }
    let certificate = out.join("certificate.json");
    assemble_external_lane_checkpoint_certificate(ExternalLaneCheckpointAssembleOptions {
        data_dir:chain.data_dir.clone(), checkpoint_file:checkpoint, vote_files:votes, certificate_file:certificate.clone(),
    }).expect("assemble five votes");
    let operation = out.join("operation.json");
    let submitter = read_transfer_key_file(&chain.data_dir,None).unwrap().address;
    build_external_lane_lock_proof(ExternalLaneLockProofOptions {
        data_dir:chain.data_dir.clone(), lane_id:"base-erc20".into(), base_rpc:url.into(), transaction_hash:tx.into(),
        certificate_file:certificate, submitter, log_index:None, operation_file:operation.clone(),
    }).expect("prove real EVM receipt");
    let release_receipt = admit_and_apply_with(&chain,&signed_release(&chain,&operation),"real-lock-release",true);
    assert!(release_receipt.accepted,"{release_receipt:?}");
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().unwrap();
    let balance = ledger.account(&chain.recipient).unwrap().balance;
    assert_eq!(balance,amount());
    refused_everywhere(&chain,&signed_release(&chain,&operation),"external_lane_nonce_already_released","replay");
    verify_blocks(NodeOptions{data_dir:chain.data_dir.clone()}).expect("replay blocks");
    verify_state(NodeOptions{data_dir:chain.data_dir.clone()}).expect("replay state");
    fs::write(out.join("lock-receipt.json"),serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
    fs::write(out.join("native-result.json"),serde_json::to_vec_pretty(&json!({
        "passed":true,"scope":"disposable local Anvil and generated six-validator PFT chain",
        "data_dir":chain.data_dir,"lock_transaction_hash":tx,"lock_block":block,"amount_atoms":amount(),
        "recipient":chain.recipient,"recipient_balance":balance,"checkpoint_votes":5,
        "release_receipt":release_receipt,"replay_refused":true,"blocks_replayed":true,"state_replayed":true,
        "public_testnet_transaction":false,"phase3_verifier":"test stand-in; export path untested"
    })).unwrap()).unwrap();
    // The runner starts a read-only RPC over this generated chain for the
    // actual Python watcher, then removes it. No fixture keys are published.
}
