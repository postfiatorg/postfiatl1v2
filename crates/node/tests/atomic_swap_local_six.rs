use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_crypto_provider::{
    bytes_to_hex, hex_to_bytes, ml_dsa_65_sign, ml_dsa_65_sign_with_context_seed,
    ML_DSA_65_ALGORITHM,
};
use postfiat_execution::genesis_hash;
use postfiat_network::{local_topology, NetworkDomain};
use postfiat_node::{
    apply_batch, apply_governance_batch, apply_shielded_batch, assemble_consensus_v2_commit,
    assemble_signed_fastswap_governance_bootstrap, assemble_signed_governance_amendment,
    asset_fee_quote, certify_and_persist_consensus_v2_votes, certify_batch_round,
    create_asset_orchard_ingress, create_asset_orchard_ingress_batch,
    create_asset_orchard_private_primary_issue, create_asset_orchard_private_primary_issue_batch,
    create_asset_orchard_private_primary_redeem, create_asset_orchard_private_primary_redeem_batch,
    create_consensus_v2_precommit_vote, create_consensus_v2_prepare_vote,
    create_consensus_v2_proposal_for_block, create_fastswap_governance_bootstrap,
    create_mempool_batch, create_transfer_batch, export_snapshot, faucet_key, import_snapshot,
    init, sign_governance_amendment_authorization, simulate_shielded_batch,
    submit_signed_asset_transaction_json_to_mempool, verify_blocks, ApplyBatchOptions,
    AssetFeeQuoteOptions, AssetOrchardIngressBatchOptions, AssetOrchardIngressCreateOptions,
    AssetOrchardPrivatePrimaryIssueBatchOptions, AssetOrchardPrivatePrimaryIssueCreateOptions,
    AssetOrchardPrivatePrimaryRedeemBatchOptions, AssetOrchardPrivatePrimaryRedeemCreateOptions,
    BatchCertificateRoundOptions, BatchTransferOptions, BlockCertificateFile, BlockProposalFile,
    DevKeyFile, FastSwapGovernanceBootstrapOptions, GovernanceAmendmentAssembleOptions,
    GovernanceAuthorizationSignOptions, InitOptions, MempoolBatchOptions, NodeOptions,
    ShieldedBatchSimulateOptions, SignedAssetTransactionJsonSubmitOptions,
    SignedFastSwapGovernanceBootstrapOptions, SnapshotExportOptions, SnapshotImportOptions,
};
use postfiat_rpc_sdk::{
    asset_fee_quote_request, atomic_swap_fee_quote_request, decode_asset_fee_quote_summary,
    decode_atomic_swap_fee_quote_summary, decode_atomic_swap_finality_summary,
    decode_transfer_fee_quote_summary,
    mempool_submit_signed_atomic_swap_transaction_finality_from_quote_request,
    mempool_submit_signed_transfer_json_request, receipts_request, status_request,
    transfer_fee_quote_request, tx_request, verify_state_request, wallet_backup_from_master_seed,
    wallet_identity_from_backup, wallet_sign_asset_transaction_from_fields,
    wallet_sign_atomic_swap_from_quote, wallet_sign_transfer_from_quote, RpcRequest, RpcResponse,
    WalletBackupFile, WalletSignAssetTransactionFields,
};
use postfiat_types::{
    issued_asset_id, market_ops_asset_id, market_ops_evidence_root, market_ops_reserve_packet_hash,
    market_ops_supply_packet_hash, pftl_uniswap_return_burn_id_from_fields,
    vault_bridge_deposit_evidence_root, vault_bridge_deposit_id,
    vault_bridge_deposit_observation_root, vault_bridge_pftl_recipient_hash,
    vault_bridge_source_root_for_asset, AssetCreateOperation, AssetTransactionOperation,
    EthereumCheckpointCertificateV1, EthereumCheckpointVoteV1, EthereumExternalEventProofV1,
    EthereumFinalizedCheckpointV1, EthereumReceiptProofV1, EthereumRouteVerificationPolicyV1,
    FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1, FastSwapCommitteeV1,
    FastSwapGovernanceBootstrapPayloadV1, FastSwapOpaqueHashV1, FastSwapValidatorV1, Genesis,
    IssuedPaymentOperation, LedgerState, MarketOpsAlignmentParams, MarketOpsEnvelope,
    MarketOpsFinalizeOperation, MarketOpsMintLimits, MarketOpsPolicyInputs,
    MarketOpsPolicyRegisterOperation, MarketOpsPolicyRegistration, MarketOpsReserveDeployLimits,
    MarketOpsVenueObservation, MempoolState, NavAssetRegisterOperation,
    NavAttestorRegisterOperation, NavEpochFinalizeOperation, NavProfileRegisterOperation,
    NavProofProfile, NavReservePublicValuesV1, NavReserveSubmitOperation,
    PftlUniswapDestinationConsumeOperation, PftlUniswapExportDebitOperation,
    PftlUniswapMintPacketV2, PftlUniswapOrderReleaseOperation, PftlUniswapOrderReserveOperation,
    PftlUniswapPrimaryMarketPolicyV2, PftlUniswapPrimaryRedeemOperation,
    PftlUniswapPrimarySubscribeV2Operation, PftlUniswapReturnImportOperation,
    PftlUniswapRouteEpochAdvanceOperation, PftlUniswapRouteInitV2Operation,
    PftlUniswapRoutePauseOperation, SignedAssetTransaction, SignedTransfer,
    UnsignedAssetTransaction, UnsignedTransfer, VaultBridgeDepositAttestOperation,
    VaultBridgeDepositClaimOperation, VaultBridgeDepositEvidence,
    VaultBridgeDepositFinalizeOperation, VaultBridgeDepositObservation,
    VaultBridgeDepositProposeOperation, ADDRESS_NAMESPACE, ETHEREUM_CHECKPOINT_SCHEMA_V1,
    ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1, FASTSWAP_SCHEMA_VERSION_V1,
    NAV_PROFILE_VERIFIER_MULTI_FETCH, NAV_PROFILE_VERIFIER_PLACEHOLDER,
    NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1, NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1,
    NAV_RESERVE_PUBLIC_VALUES_V1_BYTES, PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2,
    PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT, PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY,
    TRANSFER_TRANSACTION_KIND,
};
use serde_json::{json, Value};

const VALIDATORS: usize = 6;
const CHAIN_ID: &str = "postfiat-local";

struct Harness {
    root: PathBuf,
    children: Vec<Child>,
}

impl Harness {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "postfiat-atomic-swap-local-six-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create atomic swap harness root");
        Self {
            root,
            children: Vec::new(),
        }
    }

    fn node(&self, index: usize) -> PathBuf {
        self.root.join(format!("validator-{index}"))
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        if std::env::var_os("POSTFIAT_KEEP_ATOMIC_SWAP_LOCAL_SIX").is_none() {
            let _ = fs::remove_dir_all(&self.root);
        } else {
            eprintln!("preserved atomic swap harness at {}", self.root.display());
        }
    }
}

fn node_bin() -> &'static str {
    env!("CARGO_BIN_EXE_postfiat-node")
}

fn command_output(args: &[&str]) -> Output {
    let output = Command::new(node_bin())
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("spawn postfiat-node {args:?}: {error}"));
    assert!(
        output.status.success(),
        "postfiat-node {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn command_json(args: &[&str]) -> Value {
    let output = command_output(args);
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("parse command JSON {args:?}: {error}"))
}

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied directory");
    for entry in fs::read_dir(source).expect("read copied directory") {
        let entry = entry.expect("read copied entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("copied entry type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy node file");
        }
    }
}

fn rewrite_node_identity(data_dir: &Path, node_id: &str) {
    let path = data_dir.join("node_state.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).expect("read node state"))
        .expect("parse node state");
    state["node_id"] = json!(node_id);
    fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&state).expect("serialize node state")
        ),
    )
    .expect("write node identity");
}

fn activate_atomic_swaps_in_fresh_genesis(data_dir: &Path) {
    let path = data_dir.join("genesis.json");
    let mut genesis: Value =
        serde_json::from_slice(&fs::read(&path).expect("read genesis")).expect("parse genesis");
    genesis["atomic_swap_activation_height"] = json!(0);
    fs::write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&genesis).expect("serialize activated genesis")
        ),
    )
    .expect("activate atomic swaps in fresh integration genesis");
    let genesis: Genesis = serde_json::from_slice(
        &fs::read(data_dir.join("genesis.json")).expect("read activated genesis"),
    )
    .expect("parse activated genesis type");
    let mut chain_tip: Value = serde_json::from_slice(
        &fs::read(data_dir.join("chain_tip.json")).expect("read initial chain tip"),
    )
    .expect("parse initial chain tip");
    chain_tip["genesis_hash"] = json!(genesis_hash(&genesis));
    fs::write(
        data_dir.join("chain_tip.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&chain_tip).expect("serialize initial chain tip")
        ),
    )
    .expect("align initial chain tip with activated genesis");
}

fn activate_consensus_v2_in_fresh_genesis(data_dir: &Path) {
    let path = data_dir.join("genesis.json");
    let mut genesis: Value =
        serde_json::from_slice(&fs::read(&path).expect("read genesis")).expect("parse genesis");
    genesis["consensus_v2_activation_height"] = json!(1);
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&genesis).expect("serialize consensus-v2 genesis")
        ),
    )
    .expect("activate consensus v2 in fresh integration genesis");
    let genesis: Genesis = serde_json::from_slice(&fs::read(&path).expect("read updated genesis"))
        .expect("parse updated genesis type");
    let mut chain_tip: Value = serde_json::from_slice(
        &fs::read(data_dir.join("chain_tip.json")).expect("read initial chain tip"),
    )
    .expect("parse initial chain tip");
    chain_tip["genesis_hash"] = json!(genesis_hash(&genesis));
    fs::write(
        data_dir.join("chain_tip.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&chain_tip).expect("serialize initial chain tip")
        ),
    )
    .expect("align initial chain tip with consensus-v2 genesis");
}

fn split_validator_key(data_dir: &Path, validator: &str) -> PathBuf {
    let combined_path = data_dir.join("validator_keys.json");
    let combined: Value =
        serde_json::from_slice(&fs::read(&combined_path).expect("read validator keys"))
            .expect("parse validator keys");
    let record = combined["validators"]
        .as_array()
        .expect("validator key array")
        .iter()
        .find(|record| record["node_id"] == validator)
        .unwrap_or_else(|| panic!("missing validator key {validator}"))
        .clone();
    let path = data_dir.join(format!("{validator}.validator_keys.json"));
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({"validators": [record]}))
                .expect("serialize split validator key")
        ),
    )
    .expect("write split validator key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("set split validator key permissions");
    }
    path
}

fn free_base_port() -> u16 {
    for base in (31_000u16..60_000).step_by(32) {
        let mut listeners = Vec::new();
        let mut available = true;
        for offset in 0..(VALIDATORS as u16 * 2) {
            match TcpListener::bind(("127.0.0.1", base + offset)) {
                Ok(listener) => listeners.push(listener),
                Err(_) => {
                    available = false;
                    break;
                }
            }
        }
        if available {
            return base;
        }
    }
    panic!("no contiguous local port range available");
}

fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("timed out waiting for {}", path.display());
}

fn rpc_call_raw(port: u16, request: &RpcRequest) -> RpcResponse {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => panic!("connect RPC port {port}: {error}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(90)))
        .expect("set RPC read timeout");
    let payload = serde_json::to_vec(request).expect("serialize RPC request");
    stream.write_all(&payload).expect("write RPC request");
    stream.write_all(b"\n").expect("terminate RPC request");
    stream.flush().expect("flush RPC request");
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .expect("read RPC response");
    serde_json::from_str(&response).expect("parse RPC response")
}

fn rpc_call(port: u16, request: &RpcRequest) -> RpcResponse {
    let response = rpc_call_raw(port, request);
    assert!(
        response.ok,
        "RPC {} failed: {:?}",
        request.method, response.error
    );
    response
}

fn apply_seed_batch(data_dir: &Path, name: &str) {
    let batch = data_dir.join(format!("{name}.batch.json"));
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file: batch,
        certificate_file: None,
    })
    .unwrap_or_else(|error| panic!("apply seed batch {name}: {error}"));
    assert!(
        !receipts.is_empty() && receipts.iter().all(|receipt| receipt.accepted),
        "seed batch {name} rejected: {receipts:?}"
    );
}

fn advance_certified_chain_to_height(data_dirs: &[PathBuf], target_height: u64) {
    assert_eq!(data_dirs.len(), VALIDATORS, "padding validator count");
    let data_dir = &data_dirs[0];
    let current = command_json(&[
        "status",
        "--data-dir",
        data_dir.to_str().expect("padding data dir UTF-8"),
    ])["block_height"]
        .as_u64()
        .expect("padding start height");
    assert!(
        current <= target_height,
        "padding target {target_height} is below current height {current}"
    );
    for (index, validator_dir) in data_dirs.iter().enumerate() {
        split_validator_key(data_dir, &format!("validator-{index}"));
        if index != 0 {
            split_validator_key(validator_dir, &format!("validator-{index}"));
        }
    }
    for height in (current + 1)..=target_height {
        let stem = format!("proof-height-padding-{height:04}");
        let batch_file = data_dir.join(format!("{stem}.batch.json"));
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dir.to_path_buf(),
            key_file: None,
            to: "pfproofheightpadding000000000000000000".to_string(),
            amount: 10,
            batch_file: batch_file.clone(),
        })
        .unwrap_or_else(|error| panic!("create proof-height padding batch {height}: {error}"));
        let certificate_file = data_dir.join(format!("{stem}.certificate.json"));
        certify_batch_round(BatchCertificateRoundOptions {
            data_dir: data_dir.to_path_buf(),
            batch_kind: Some("transparent".to_string()),
            batch_file: batch_file.clone(),
            validator_key_dir: data_dir.to_path_buf(),
            vote_dir: data_dir.join(format!("{stem}.votes")),
            proposal_file: data_dir.join(format!("{stem}.proposal.json")),
            certificate_file: certificate_file.clone(),
            block_height: Some(height),
            view: None,
            timeout_certificate_file: None,
            skip_block_log_verify: true,
        })
        .unwrap_or_else(|error| panic!("certify proof-height padding block {height}: {error}"));
        let proposal_file = data_dir.join(format!("{stem}.proposal.json"));
        let block_proposal: BlockProposalFile = serde_json::from_slice(
            &fs::read(&proposal_file)
                .unwrap_or_else(|error| panic!("read proof-height proposal {height}: {error}")),
        )
        .unwrap_or_else(|error| panic!("parse proof-height proposal {height}: {error}"));
        let proposer_key =
            data_dir.join(format!("{}.validator_keys.json", block_proposal.proposer));
        let consensus_proposal =
            create_consensus_v2_proposal_for_block(data_dir, &block_proposal, None, &proposer_key)
                .unwrap_or_else(|error| {
                    panic!("create proof-height v2 proposal {height}: {error}")
                });
        let prepare_votes = thread::scope(|scope| {
            let handles = data_dirs
                .iter()
                .enumerate()
                .map(|(index, validator_dir)| {
                    let proposal = &consensus_proposal;
                    scope.spawn(move || {
                        create_consensus_v2_prepare_vote(
                            validator_dir,
                            proposal,
                            None,
                            &validator_dir.join(format!("validator-{index}.validator_keys.json")),
                            &format!("validator-{index}"),
                        )
                        .unwrap_or_else(|error| {
                            panic!("create proof-height prepare vote {height}/{index}: {error}")
                        })
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("join proof-height prepare vote"))
                .collect::<Vec<_>>()
        });
        let prepare_qc = certify_and_persist_consensus_v2_votes(
            data_dir,
            consensus_proposal.round,
            postfiat_types::ConsensusV2Phase::Prepare,
            Some(consensus_proposal.block.clone()),
            prepare_votes,
        )
        .unwrap_or_else(|error| panic!("certify proof-height prepare QC {height}: {error}"));
        let precommit_votes = thread::scope(|scope| {
            let handles = data_dirs
                .iter()
                .enumerate()
                .map(|(index, validator_dir)| {
                    let qc = &prepare_qc;
                    scope.spawn(move || {
                        create_consensus_v2_precommit_vote(
                            validator_dir,
                            qc,
                            &validator_dir.join(format!("validator-{index}.validator_keys.json")),
                            &format!("validator-{index}"),
                        )
                        .unwrap_or_else(|error| {
                            panic!("create proof-height precommit vote {height}/{index}: {error}")
                        })
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("join proof-height precommit vote"))
                .collect::<Vec<_>>()
        });
        let precommit_qc = certify_and_persist_consensus_v2_votes(
            data_dir,
            consensus_proposal.round,
            postfiat_types::ConsensusV2Phase::Precommit,
            Some(consensus_proposal.block.clone()),
            precommit_votes,
        )
        .unwrap_or_else(|error| panic!("certify proof-height precommit QC {height}: {error}"));
        let commit = assemble_consensus_v2_commit(
            data_dir,
            &block_proposal,
            consensus_proposal,
            None,
            prepare_qc,
            precommit_qc,
        )
        .unwrap_or_else(|error| panic!("assemble proof-height v2 commit {height}: {error}"));
        let mut certificate: BlockCertificateFile = serde_json::from_slice(
            &fs::read(&certificate_file)
                .unwrap_or_else(|error| panic!("read proof-height certificate {height}: {error}")),
        )
        .unwrap_or_else(|error| panic!("parse proof-height certificate {height}: {error}"));
        certificate.consensus_v2_commit = Some(commit);
        fs::write(
            &certificate_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&certificate).unwrap_or_else(|error| {
                    panic!("serialize proof-height certificate {height}: {error}")
                })
            ),
        )
        .unwrap_or_else(|error| panic!("write proof-height certificate {height}: {error}"));
        for (index, validator_dir) in data_dirs.iter().enumerate() {
            let receipts = apply_batch(ApplyBatchOptions {
                data_dir: validator_dir.clone(),
                batch_file: batch_file.clone(),
                certificate_file: Some(certificate_file.clone()),
            })
            .unwrap_or_else(|error| {
                panic!("apply proof-height padding block {height}/{index}: {error}")
            });
            assert!(
                !receipts.is_empty() && receipts.iter().all(|receipt| receipt.accepted),
                "proof-height padding block {height}/{index} rejected: {receipts:?}"
            );
        }
    }
}

fn finalize_offline_batch_all_validators(
    data_dirs: &[PathBuf],
    batch_file: &Path,
    batch_kind: &str,
    label: &str,
) -> (u64, String, String) {
    assert_eq!(data_dirs.len(), VALIDATORS, "offline validator count");
    let data_dir = &data_dirs[0];
    for (index, validator_dir) in data_dirs.iter().enumerate() {
        split_validator_key(data_dir, &format!("validator-{index}"));
        if index != 0 {
            split_validator_key(validator_dir, &format!("validator-{index}"));
        }
    }
    let current = command_json(&[
        "status",
        "--data-dir",
        data_dir.to_str().expect("offline data dir UTF-8"),
    ])["block_height"]
        .as_u64()
        .expect("offline start height");
    let height = current + 1;
    let proposal_file = data_dir.join(format!("{label}.proposal.json"));
    let certificate_file = data_dir.join(format!("{label}.certificate.json"));
    certify_batch_round(BatchCertificateRoundOptions {
        data_dir: data_dir.clone(),
        batch_kind: Some(batch_kind.to_string()),
        batch_file: batch_file.to_path_buf(),
        validator_key_dir: data_dir.clone(),
        vote_dir: data_dir.join(format!("{label}.votes")),
        proposal_file: proposal_file.clone(),
        certificate_file: certificate_file.clone(),
        block_height: Some(height),
        view: None,
        timeout_certificate_file: None,
        skip_block_log_verify: false,
    })
    .unwrap_or_else(|error| panic!("certify {label} at {height}: {error}"));
    let block_proposal: BlockProposalFile = serde_json::from_slice(
        &fs::read(&proposal_file).unwrap_or_else(|error| panic!("read {label} proposal: {error}")),
    )
    .unwrap_or_else(|error| panic!("parse {label} proposal: {error}"));
    let proposer_key = data_dir.join(format!("{}.validator_keys.json", block_proposal.proposer));
    let consensus_proposal =
        create_consensus_v2_proposal_for_block(data_dir, &block_proposal, None, &proposer_key)
            .unwrap_or_else(|error| panic!("create {label} consensus-v2 proposal: {error}"));
    let prepare_votes = thread::scope(|scope| {
        let handles = data_dirs
            .iter()
            .enumerate()
            .map(|(index, validator_dir)| {
                let proposal = &consensus_proposal;
                scope.spawn(move || {
                    create_consensus_v2_prepare_vote(
                        validator_dir,
                        proposal,
                        None,
                        &validator_dir.join(format!("validator-{index}.validator_keys.json")),
                        &format!("validator-{index}"),
                    )
                    .unwrap_or_else(|error| panic!("create {label} prepare vote {index}: {error}"))
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("join offline prepare vote"))
            .collect::<Vec<_>>()
    });
    let prepare_qc = certify_and_persist_consensus_v2_votes(
        data_dir,
        consensus_proposal.round,
        postfiat_types::ConsensusV2Phase::Prepare,
        Some(consensus_proposal.block.clone()),
        prepare_votes,
    )
    .unwrap_or_else(|error| panic!("certify {label} prepare QC: {error}"));
    let precommit_votes = thread::scope(|scope| {
        let handles = data_dirs
            .iter()
            .enumerate()
            .map(|(index, validator_dir)| {
                let qc = &prepare_qc;
                scope.spawn(move || {
                    create_consensus_v2_precommit_vote(
                        validator_dir,
                        qc,
                        &validator_dir.join(format!("validator-{index}.validator_keys.json")),
                        &format!("validator-{index}"),
                    )
                    .unwrap_or_else(|error| {
                        panic!("create {label} precommit vote {index}: {error}")
                    })
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("join offline precommit vote"))
            .collect::<Vec<_>>()
    });
    let precommit_qc = certify_and_persist_consensus_v2_votes(
        data_dir,
        consensus_proposal.round,
        postfiat_types::ConsensusV2Phase::Precommit,
        Some(consensus_proposal.block.clone()),
        precommit_votes,
    )
    .unwrap_or_else(|error| panic!("certify {label} precommit QC: {error}"));
    let commit = assemble_consensus_v2_commit(
        data_dir,
        &block_proposal,
        consensus_proposal,
        None,
        prepare_qc,
        precommit_qc,
    )
    .unwrap_or_else(|error| panic!("assemble {label} consensus-v2 commit: {error}"));
    let mut certificate: BlockCertificateFile = serde_json::from_slice(
        &fs::read(&certificate_file)
            .unwrap_or_else(|error| panic!("read {label} certificate: {error}")),
    )
    .unwrap_or_else(|error| panic!("parse {label} certificate: {error}"));
    certificate.consensus_v2_commit = Some(commit);
    fs::write(
        &certificate_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&certificate)
                .unwrap_or_else(|error| panic!("serialize {label} certificate: {error}"))
        ),
    )
    .unwrap_or_else(|error| panic!("write {label} certificate: {error}"));

    let mut expected = None;
    for (index, validator_dir) in data_dirs.iter().enumerate() {
        let options = ApplyBatchOptions {
            data_dir: validator_dir.clone(),
            batch_file: batch_file.to_path_buf(),
            certificate_file: Some(certificate_file.clone()),
        };
        let receipts = match batch_kind {
            "governance" => apply_governance_batch(options),
            "shielded" => apply_shielded_batch(options),
            _ => apply_batch(options),
        }
        .unwrap_or_else(|error| panic!("apply {label} on validator {index}: {error}"));
        assert!(
            !receipts.is_empty() && receipts.iter().all(|receipt| receipt.accepted),
            "{label} rejected on validator {index}: {receipts:?}"
        );
        let value = command_json(&[
            "status",
            "--data-dir",
            validator_dir
                .to_str()
                .expect("offline validator path UTF-8"),
        ]);
        let observed = (
            value["block_height"].as_u64().expect("offline height"),
            value["block_tip_hash"]
                .as_str()
                .expect("offline tip")
                .to_string(),
            value["state_root"]
                .as_str()
                .expect("offline root")
                .to_string(),
        );
        if let Some(expected) = &expected {
            assert_eq!(&observed, expected, "{label} validator {index} divergence");
        } else {
            expected = Some(observed);
        }
    }
    expected.expect("offline finalized state")
}

fn stop_services(harness: &mut Harness) {
    for child in &mut harness.children {
        let _ = child.kill();
        let _ = child.wait();
    }
    harness.children.clear();
}

fn stop_validator_services(harness: &mut Harness, validator_index: usize) {
    assert_eq!(
        harness.children.len(),
        VALIDATORS * 2,
        "partial-outage stop requires one transport and one RPC child per validator"
    );
    assert!(
        validator_index < VALIDATORS,
        "partial-outage validator index"
    );
    let rpc_index = validator_index * 2 + 1;
    let transport_index = validator_index * 2;
    for index in [rpc_index, transport_index] {
        let mut child = harness.children.remove(index);
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn start_services(harness: &mut Harness, topology_path: &Path, rpc_ports: &[u16]) -> Vec<PathBuf> {
    let ready = spawn_services(harness, topology_path, rpc_ports);
    for path in &ready {
        wait_for_file(path, Duration::from_secs(90));
    }
    ready
}

fn bootstrap_fastswap_committee(harness: &Harness, data_dirs: &[PathBuf]) -> FastSwapCommitteeV1 {
    let data_dir = &data_dirs[0];
    let genesis: Genesis = serde_json::from_slice(
        &fs::read(data_dir.join("genesis.json")).expect("read committee genesis"),
    )
    .expect("parse committee genesis");
    let validator_keys: Value = serde_json::from_slice(
        &fs::read(data_dir.join("validator_keys.json")).expect("read committee validator keys"),
    )
    .expect("parse committee validator keys");
    let mut committee = FastSwapCommitteeV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: FastSwapChainDomainV1 {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: FastSwapOpaqueHashV1(
                    hex_to_bytes(&genesis_hash(&genesis))
                        .expect("committee genesis hash hex")
                        .try_into()
                        .expect("committee genesis hash width"),
                ),
                protocol_version: genesis.protocol_version,
            },
            fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 1,
            committee_root: FastSwapCommitteeRootV1::ZERO,
            validator_count: VALIDATORS as u16,
            quorum: 5,
        },
        validators: validator_keys["validators"]
            .as_array()
            .expect("committee validator key array")
            .iter()
            .map(|record| FastSwapValidatorV1 {
                validator_id: record["node_id"]
                    .as_str()
                    .expect("committee validator id")
                    .to_string(),
                public_key: hex_to_bytes(
                    record["public_key_hex"]
                        .as_str()
                        .expect("committee public key"),
                )
                .expect("decode committee public key"),
            })
            .collect(),
    };
    committee
        .validators
        .sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    committee.domain.committee_root = committee.computed_root().expect("committee root");

    let current = command_json(&[
        "status",
        "--data-dir",
        data_dir.to_str().expect("committee data dir UTF-8"),
    ])["block_height"]
        .as_u64()
        .expect("committee bootstrap height");
    let payload = FastSwapGovernanceBootstrapPayloadV1 {
        committee: committee.clone(),
        asset_rules: Vec::new(),
        policies: Vec::new(),
        activation_height: current + 2,
    };
    let payload_file = harness.root.join("a666-fastswap-bootstrap-payload.json");
    let amendment_file = harness.root.join("a666-fastswap-bootstrap-amendment.json");
    let signed_amendment_file = harness
        .root
        .join("a666-fastswap-bootstrap-amendment.signed.json");
    let unsigned_batch_file = harness
        .root
        .join("a666-fastswap-bootstrap-batch.unsigned.json");
    let batch_file = harness.root.join("a666-fastswap-bootstrap-batch.json");
    fs::write(
        &payload_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&payload).expect("serialize committee payload")
        ),
    )
    .expect("write committee payload");
    let validators = (0..VALIDATORS)
        .map(|index| format!("validator-{index}"))
        .collect::<Vec<_>>();
    create_fastswap_governance_bootstrap(FastSwapGovernanceBootstrapOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators.clone(),
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        payload_file: payload_file.clone(),
        amendment_file: amendment_file.clone(),
        batch_file: unsigned_batch_file,
    })
    .expect("create committee governance bootstrap");
    let authorization_files = validators
        .iter()
        .map(|validator| {
            let key_file = split_validator_key(data_dir, validator);
            let authorization_file = harness
                .root
                .join(format!("{validator}.fastswap-bootstrap-authorization.json"));
            sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                data_dir: data_dir.clone(),
                amendment_file: amendment_file.clone(),
                validator: validator.clone(),
                validator_key_file: key_file,
                proposal_slot: current + 1,
                expires_at_height: current + 100,
                authorization_file: authorization_file.clone(),
            })
            .expect("sign committee governance authorization");
            authorization_file
        })
        .collect::<Vec<_>>();
    assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
        data_dir: data_dir.clone(),
        amendment_file,
        authorization_files,
        proposal_slot: current + 1,
        output_file: signed_amendment_file.clone(),
    })
    .expect("assemble committee governance amendment");
    assemble_signed_fastswap_governance_bootstrap(SignedFastSwapGovernanceBootstrapOptions {
        data_dir: data_dir.clone(),
        payload_file,
        signed_amendment_file,
        proposal_slot: current + 1,
        batch_file: batch_file.clone(),
    })
    .expect("assemble committee governance bootstrap");
    finalize_offline_batch_all_validators(
        data_dirs,
        &batch_file,
        "governance",
        "a666-fastswap-bootstrap",
    );
    committee
}

fn ethereum_rlp_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() == 1 && bytes[0] <= 0x7f {
        return bytes.to_vec();
    }
    if bytes.len() < 56 {
        let mut encoded = vec![0x80 + bytes.len() as u8];
        encoded.extend_from_slice(bytes);
        return encoded;
    }
    let length_bytes = bytes.len().to_be_bytes();
    let first = length_bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(length_bytes.len() - 1);
    let length = &length_bytes[first..];
    let mut encoded = vec![0xb7 + length.len() as u8];
    encoded.extend_from_slice(length);
    encoded.extend_from_slice(bytes);
    encoded
}

fn ethereum_rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload = items.concat();
    if payload.len() < 56 {
        let mut encoded = vec![0xc0 + payload.len() as u8];
        encoded.extend_from_slice(&payload);
        return encoded;
    }
    let length_bytes = payload.len().to_be_bytes();
    let first = length_bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(length_bytes.len() - 1);
    let length = &length_bytes[first..];
    let mut encoded = vec![0xf7 + length.len() as u8];
    encoded.extend_from_slice(length);
    encoded.extend_from_slice(&payload);
    encoded
}

fn ethereum_abi_u64(value: u64) -> [u8; 32] {
    let mut word = [0_u8; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}

fn ethereum_abi_address(value: [u8; 20]) -> [u8; 32] {
    let mut word = [0_u8; 32];
    word[12..].copy_from_slice(&value);
    word
}

fn ethereum_abi_dynamic(value: &[u8]) -> Vec<u8> {
    let mut encoded =
        ethereum_abi_u64(u64::try_from(value.len()).expect("ABI value length fits u64")).to_vec();
    encoded.extend_from_slice(value);
    encoded.resize(encoded.len().div_ceil(32) * 32, 0);
    encoded
}

fn ethereum_receipt_proof(
    emitter: [u8; 20],
    topics: &[[u8; 32]],
    data: &[u8],
) -> ([u8; 32], EthereumReceiptProofV1) {
    let topics = topics
        .iter()
        .map(|topic| ethereum_rlp_bytes(topic))
        .collect::<Vec<_>>();
    let log = ethereum_rlp_list(&[
        ethereum_rlp_bytes(&emitter),
        ethereum_rlp_list(&topics),
        ethereum_rlp_bytes(data),
    ]);
    let receipt = ethereum_rlp_list(&[
        ethereum_rlp_bytes(&[1]),
        ethereum_rlp_bytes(&[1]),
        ethereum_rlp_bytes(&[0; 256]),
        ethereum_rlp_list(&[log]),
    ]);
    let leaf = ethereum_rlp_list(&[
        ethereum_rlp_bytes(&[0x20, 0x80]),
        ethereum_rlp_bytes(&receipt),
    ]);
    (
        postfiat_bridge::ethereum_keccak256(&leaf),
        EthereumReceiptProofV1 {
            transaction_index: 0,
            receipt_rlp: receipt,
            proof_nodes_rlp: vec![leaf],
        },
    )
}

fn ethereum_checkpoint_certificate(
    committee: &FastSwapCommitteeV1,
    data_dir: &Path,
    checkpoint: EthereumFinalizedCheckpointV1,
) -> EthereumCheckpointCertificateV1 {
    let validator_keys: Value = serde_json::from_slice(
        &fs::read(data_dir.join("validator_keys.json")).expect("read checkpoint validator keys"),
    )
    .expect("parse checkpoint validator keys");
    let votes = committee
        .validators
        .iter()
        .take(usize::from(committee.domain.quorum))
        .enumerate()
        .map(|(index, validator)| {
            let record = validator_keys["validators"]
                .as_array()
                .expect("checkpoint validator key array")
                .iter()
                .find(|record| record["node_id"] == validator.validator_id)
                .expect("checkpoint signing validator");
            let mut vote = EthereumCheckpointVoteV1 {
                validator_id: validator.validator_id.clone(),
                signature: vec![1],
            };
            vote.signature = ml_dsa_65_sign_with_context_seed(
                &hex_to_bytes(
                    record["private_key_hex"]
                        .as_str()
                        .expect("checkpoint private key"),
                )
                .expect("decode checkpoint private key"),
                &vote
                    .signing_bytes(&checkpoint)
                    .expect("checkpoint vote bytes"),
                ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
                &[0xb0 + index as u8; 32],
            )
            .expect("sign checkpoint vote");
            vote
        })
        .collect();
    EthereumCheckpointCertificateV1 { checkpoint, votes }
}

fn checked_mul_div_ceil(value: u64, multiplier: u64, denominator: u64) -> u64 {
    let numerator = u128::from(value) * u128::from(multiplier);
    numerator.div_ceil(u128::from(denominator)) as u64
}

fn checked_mul_div_floor(value: u64, multiplier: u64, denominator: u64) -> u64 {
    (u128::from(value) * u128::from(multiplier) / u128::from(denominator)) as u64
}

#[test]
#[ignore = "qualification helper for reserve proofs observed above genesis height"]
fn certified_chain_padding_reaches_requested_height() {
    let harness = Harness::new();
    let seed_dir = harness.root.join("proof-height-padding-seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: "postfiat-wan-devnet-2".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: VALIDATORS as u32,
    })
    .expect("initialize proof-height padding seed");
    activate_consensus_v2_in_fresh_genesis(&seed_dir);
    let target_height = std::env::var("POSTFIAT_PROOF_PADDING_TARGET_HEIGHT")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(8);
    let data_dirs = (0..VALIDATORS)
        .map(|index| {
            let data_dir = harness.node(index);
            copy_dir(&seed_dir, &data_dir);
            rewrite_node_identity(&data_dir, &format!("validator-{index}"));
            data_dir
        })
        .collect::<Vec<_>>();
    advance_certified_chain_to_height(&data_dirs, target_height);
    let mut roots = Vec::new();
    for data_dir in data_dirs {
        let verified = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("replay proof-height padded chain");
        assert_eq!(verified.block_count, target_height as usize);
        roots.push(verified.state_root);
    }
    assert!(roots.iter().all(|root| root == &roots[0]));
}

#[test]
#[ignore = "qualification helper for governed six-validator FastSwap bootstrap"]
fn governed_fastswap_committee_bootstrap_converges_across_six_validators() {
    let harness = Harness::new();
    let seed_dir = harness.root.join("fastswap-bootstrap-seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: "postfiat-wan-devnet-2".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: VALIDATORS as u32,
    })
    .expect("initialize FastSwap bootstrap seed");
    activate_consensus_v2_in_fresh_genesis(&seed_dir);
    let data_dirs = (0..VALIDATORS)
        .map(|index| {
            let data_dir = harness.node(index);
            copy_dir(&seed_dir, &data_dir);
            rewrite_node_identity(&data_dir, &format!("validator-{index}"));
            data_dir
        })
        .collect::<Vec<_>>();
    advance_certified_chain_to_height(&data_dirs, 2);
    let committee = bootstrap_fastswap_committee(&harness, &data_dirs);
    let mut roots = Vec::new();
    for (index, data_dir) in data_dirs.iter().enumerate() {
        let verified = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("replay governed FastSwap bootstrap");
        roots.push(verified.state_root);
        let ledger: LedgerState = serde_json::from_slice(
            &fs::read(data_dir.join("ledger.json")).expect("read FastSwap bootstrap ledger"),
        )
        .expect("parse FastSwap bootstrap ledger");
        assert_eq!(ledger.fastswap_committees, vec![committee.clone()]);
        assert_eq!(ledger.fastswap_activation_height, Some(4));
        assert_eq!(verified.block_count, 3, "validator {index}");
    }
    assert!(roots.iter().all(|root| root == &roots[0]));
}

fn required_env_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} is required for the exact A666 migration rehearsal"))
}

fn read_dev_key_from_env(name: &str) -> DevKeyFile {
    let path = required_env_path(name);
    serde_json::from_slice(
        &fs::read(&path)
            .unwrap_or_else(|error| panic!("read {name} at {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {name} at {}: {error}", path.display()))
}

fn read_raw_or_hex_from_env(name: &str) -> Vec<u8> {
    let path = required_env_path(name);
    let bytes = fs::read(&path)
        .unwrap_or_else(|error| panic!("read {name} at {}: {error}", path.display()));
    if let Ok(text) = std::str::from_utf8(&bytes) {
        let trimmed = text.trim();
        if !trimmed.is_empty()
            && trimmed.len() % 2 == 0
            && trimmed.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return hex_to_bytes(trimmed)
                .unwrap_or_else(|error| panic!("decode {name} at {}: {error}", path.display()));
        }
    }
    bytes
}

fn fund_wallet(data_dir: &Path, address: &str, name: &str) {
    let batch = data_dir.join(format!("{name}.batch.json"));
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.to_path_buf(),
        key_file: None,
        to: address.to_string(),
        amount: 1_000_000,
        batch_file: batch,
    })
    .unwrap_or_else(|error| panic!("build wallet funding batch {name}: {error}"));
    apply_seed_batch(data_dir, name);
}

fn apply_asset_operation(
    data_dir: &Path,
    backup: &WalletBackupFile,
    operation: AssetTransactionOperation,
    name: &str,
) {
    assert!(
        !matches!(operation, AssetTransactionOperation::TrustSet(_)),
        "local atomic swap seed must never create a trust line explicitly"
    );
    let identity = wallet_identity_from_backup(backup).expect("asset signer identity");
    let quote = asset_fee_quote(AssetFeeQuoteOptions {
        data_dir: data_dir.to_path_buf(),
        source: identity.address,
        operation_json: serde_json::to_string(&operation).expect("serialize asset operation"),
        sequence: None,
    })
    .unwrap_or_else(|error| panic!("asset quote {name}: {error}"));
    let signed = wallet_sign_asset_transaction_from_fields(
        backup,
        WalletSignAssetTransactionFields {
            chain_id: quote.chain_id,
            genesis_hash: quote.genesis_hash,
            protocol_version: quote.protocol_version,
            source: quote.source,
            fee: quote.minimum_fee,
            sequence: quote.sequence,
            operation: quote.operation,
        },
    )
    .unwrap_or_else(|error| panic!("sign asset operation {name}: {error}"));
    submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
        data_dir: data_dir.to_path_buf(),
        signed_asset_transaction_json: serde_json::to_string(&signed)
            .expect("serialize signed asset transaction"),
    })
    .unwrap_or_else(|error| panic!("submit asset operation {name}: {error}"));
    let batch = data_dir.join(format!("{name}.batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file: batch,
        max_transactions: 1,
    })
    .unwrap_or_else(|error| panic!("batch asset operation {name}: {error}"));
    apply_seed_batch(data_dir, name);
}

fn backup(seed_byte: u8) -> WalletBackupFile {
    wallet_backup_from_master_seed(CHAIN_ID, format!("{seed_byte:02x}").repeat(32), 0)
        .expect("deterministic wallet backup")
}

fn backup_for_chain(chain_id: &str, seed_byte: u8) -> WalletBackupFile {
    wallet_backup_from_master_seed(chain_id, format!("{seed_byte:02x}").repeat(32), 0)
        .expect("deterministic chain-bound wallet backup")
}

fn usd_e8(amount: u128) -> u128 {
    amount * 100_000_000
}

fn market_ops_policy() -> MarketOpsPolicyRegistration {
    MarketOpsPolicyRegistration {
        program_id: [0x31; 32],
        policy_hash: [0x32; 32],
        parameter_hash: [0x33; 32],
        venue_id: [0x37; 32],
        pool_config_hash: [0x38; 32],
        hook_code_hash: [0x39; 32],
        activation_epoch: 1,
        deactivation_epoch: 0,
    }
}

fn a651_market_ops_operation(
    issuer: &str,
    asset_id: &str,
    reserve_packet_hash: &str,
) -> MarketOpsFinalizeOperation {
    let policy = market_ops_policy();
    let discount_observations = vec![
        MarketOpsVenueObservation {
            dt_seconds: 4_200,
            price_usd_e8: usd_e8(475) / 100,
            volume_usd_e8: usd_e8(2_500),
        },
        MarketOpsVenueObservation {
            dt_seconds: 5_800,
            price_usd_e8: usd_e8(5),
            volume_usd_e8: usd_e8(7_500),
        },
    ];
    let premium_observations = vec![
        MarketOpsVenueObservation {
            dt_seconds: 1_800,
            price_usd_e8: usd_e8(5_625) / 1_000,
            volume_usd_e8: usd_e8(2_200),
        },
        MarketOpsVenueObservation {
            dt_seconds: 8_200,
            price_usd_e8: usd_e8(5),
            volume_usd_e8: usd_e8(7_800),
        },
    ];
    let policy_inputs = MarketOpsPolicyInputs {
        unit_scale: 1,
        floor_factor_bps: 10_000,
        alignment_params: MarketOpsAlignmentParams {
            policy_min_usd_e8: usd_e8(25_000),
            min_alignment_bps: 100,
            stress_repeat_factor_14d: 3,
            stress_repeat_factor_90d: 2,
            stale_epochs_allowed: 1,
            max_decay_per_epoch_bps: 1_000,
        },
        previous_required_alignment_reserve_usd_e8: 0,
        cost_to_restore_14d_usd_e8: vec![usd_e8(20_000), usd_e8(45_000), usd_e8(45_000)],
        cost_to_restore_90d_usd_e8: vec![usd_e8(30_000), usd_e8(45_000), usd_e8(60_000)],
        reserve_limits: MarketOpsReserveDeployLimits {
            available_alignment_reserve_usd_e8: usd_e8(150_000),
            venue_policy_cap_usd_e8: usd_e8(50_000),
            depth_limited_cap_usd_e8: usd_e8(30_000),
            cooldown_limited_cap_usd_e8: usd_e8(40_000),
        },
        mint_limits: MarketOpsMintLimits {
            policy_max_mint_atoms: 50_000,
            venue_bid_depth_atoms: 12_000,
            cooldown_mint_atoms: 10_000,
        },
        discount_observations,
        premium_observations,
    };
    let envelope = MarketOpsEnvelope {
        encoding_version: 1,
        chain_id: 1,
        adapter_address: [0x11; 20],
        vault_address: [0x12; 20],
        mint_controller_address: [0x13; 20],
        asset_id: market_ops_asset_id(asset_id).expect("derive a651 market-ops asset id"),
        epoch: 1,
        program_id: policy.program_id,
        policy_hash: policy.policy_hash,
        parameter_hash: policy.parameter_hash,
        reserve_packet_hash: market_ops_reserve_packet_hash(reserve_packet_hash)
            .expect("derive a651 market-ops reserve hash"),
        supply_packet_hash: market_ops_supply_packet_hash(asset_id, 1, 1_000_000)
            .expect("derive a651 market-ops supply hash"),
        evidence_root: market_ops_evidence_root(
            &policy_inputs.discount_observations,
            &policy_inputs.premium_observations,
        )
        .expect("derive a651 market-ops evidence root"),
        previous_market_state_hash: [0u8; 32],
        venue_id: policy.venue_id,
        pool_config_hash: policy.pool_config_hash,
        hook_code_hash: policy.hook_code_hash,
        nav_floor_usd_e8: usd_e8(5),
        valid_global_supply_atoms: 1_000_000,
        verified_net_assets_usd_e8: usd_e8(5_000_000),
        funded_alignment_reserve_usd_e8: usd_e8(150_000),
        required_alignment_reserve_usd_e8: usd_e8(135_000),
        max_reserve_deploy_usd_e8: usd_e8(25_875),
        max_mint_atoms: 0,
        discount_trigger_bps: 300,
        premium_trigger_bps: 1_000,
        data_window_start: 100,
        data_window_end: 10_100,
        valid_after: 10_100,
        expires_at: 20_100,
        cooldown_seconds: 600,
        nonce: [0x55; 32],
    };
    let envelope_hash = bytes_to_hex(&envelope.envelope_hash());
    MarketOpsFinalizeOperation {
        issuer: issuer.to_string(),
        asset_id: asset_id.to_string(),
        envelope_hash,
        envelope,
        policy_inputs,
    }
}

fn register_placeholder_nav_profile(
    data_dir: &Path,
    registrant: &WalletBackupFile,
    registrant_address: &str,
    source_class: &str,
    name: &str,
) -> String {
    let operation = NavProfileRegisterOperation {
        registrant: registrant_address.to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_PLACEHOLDER.to_string(),
        source_class: source_class.to_string(),
        max_snapshot_age_blocks: 0,
        challenge_window_blocks: 0,
        max_epoch_gap_blocks: 0,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 0,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: String::new(),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey: String::new(),
        sp1_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
        public_values_schema: String::new(),
        source_manifest_hash: String::new(),
        valuation_unit_id: String::new(),
        max_observation_span_blocks: 0,
        allow_controlled_sources: false,
    };
    let profile = NavProofProfile::new(
        operation.registrant.clone(),
        operation.verifier_kind.clone(),
        operation.source_class.clone(),
        operation.max_snapshot_age_blocks,
        operation.challenge_window_blocks,
        operation.max_epoch_gap_blocks,
        operation.settle_deadline_blocks,
        operation.min_challenge_bond,
        operation.min_attestations,
        operation.tolerance_bp,
        operation.valuation_policy_hash.clone(),
        operation.sp1_program_vkey.clone(),
        operation.sp1_proof_encoding.clone(),
        operation.max_proof_bytes,
        operation.max_public_values_bytes,
    )
    .expect("derive deterministic placeholder NAV profile");
    apply_asset_operation(
        data_dir,
        registrant,
        AssetTransactionOperation::NavProfileRegister(operation),
        name,
    );
    profile.profile_id
}

fn register_successor_nav_profile(
    data_dir: &Path,
    registrant: &WalletBackupFile,
    registrant_address: &str,
) -> String {
    let operation = successor_nav_profile_operation(registrant_address);
    let profile = operation
        .to_profile()
        .expect("derive provider-neutral successor profile through consensus path");
    apply_asset_operation(
        data_dir,
        registrant,
        AssetTransactionOperation::NavProfileRegister(operation),
        "register-provider-neutral-successor-profile",
    );
    profile.profile_id
}

fn successor_nav_profile_operation(registrant_address: &str) -> NavProfileRegisterOperation {
    NavProfileRegisterOperation {
        registrant: registrant_address.to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1.to_string(),
        source_class: "manifest-driven".to_string(),
        max_snapshot_age_blocks: 10_000,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 100,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 0,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: "04".repeat(32),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey:
            "0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100"
                .to_string(),
        sp1_proof_encoding: "groth16".to_string(),
        max_proof_bytes: 4_096,
        max_public_values_bytes: NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
        public_values_schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        source_manifest_hash: "9da4e2ba55939f138475026946d2728d9b40d3f4c7762289a70aae94584eac924b9a788c6df25c9276cc83f1616ef0e5".to_string(),
        valuation_unit_id: "05".repeat(48),
        max_observation_span_blocks: 8,
        allow_controlled_sources: true,
    }
}

fn a666_public_successor_profile_operation(
    registrant_address: &str,
) -> NavProfileRegisterOperation {
    NavProfileRegisterOperation {
        registrant: registrant_address.to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1.to_string(),
        source_class: "manifest-driven-a666-public-reserves-v1".to_string(),
        max_snapshot_age_blocks: 900,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 128,
        settle_deadline_blocks: 256,
        min_challenge_bond: 0,
        min_attestations: 0,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash:
            "350eaee0a1ca12ba51637781ba52661b8685f868657a7c5e7d07c31b2899869c"
                .to_string(),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey:
            "0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf"
                .to_string(),
        sp1_proof_encoding: "groth16".to_string(),
        max_proof_bytes: 4_096,
        max_public_values_bytes: NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
        public_values_schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        source_manifest_hash: "8abe3e59198b72945d4778a7fa91e5af157a6c65032d8940cca486850ffe59fcb567268ca5942669ff6977ef32dd3a41".to_string(),
        valuation_unit_id: "c67872c31caa85cbe6dd287a1e060f0f5cfc0e9f3c5bd85a7569897fd0cefb031583b7afc001e7d1afa492e9abf77d60".to_string(),
        max_observation_span_blocks: 8,
        allow_controlled_sources: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_nav_epoch(
    data_dir: &Path,
    issuer: &WalletBackupFile,
    issuer_address: &str,
    asset_id: &str,
    proof_profile: &str,
    nav_per_unit: u64,
    verified_net_assets: u64,
    reserve_packet_hash: &str,
    name: &str,
) {
    apply_asset_operation(
        data_dir,
        issuer,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer_address.to_string(),
            asset_id: asset_id.to_string(),
            reserve_operator: issuer_address.to_string(),
            proof_profile: proof_profile.to_string(),
            valuation_unit: "usd_e8".to_string(),
            redemption_account: issuer_address.to_string(),
        }),
        &format!("register-{name}"),
    );
    apply_asset_operation(
        data_dir,
        issuer,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer_address.to_string(),
            submitter: issuer_address.to_string(),
            asset_id: asset_id.to_string(),
            epoch: 1,
            nav_per_unit,
            circulating_supply: 1_000_000,
            verified_net_assets,
            proof_profile: proof_profile.to_string(),
            source_root: "01".repeat(48),
            attestor_root: "02".repeat(48),
            reserve_packet_hash: reserve_packet_hash.to_string(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
        &format!("reserve-{name}"),
    );
    apply_asset_operation(
        data_dir,
        issuer,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer_address.to_string(),
            asset_id: asset_id.to_string(),
            epoch: 1,
            reserve_packet_hash: reserve_packet_hash.to_string(),
        }),
        &format!("finalize-{name}"),
    );
}

fn spawn_services(harness: &mut Harness, topology: &Path, ports: &[u16]) -> Vec<PathBuf> {
    let mut ready_files = Vec::new();
    for (index, port) in ports.iter().copied().enumerate().take(VALIDATORS) {
        let validator = format!("validator-{index}");
        let data_dir = harness.node(index);
        let key = split_validator_key(&data_dir, &validator);
        let transport_ready = harness
            .root
            .join(format!("{validator}.transport.ready.json"));
        let transport_log = fs::File::create(
            harness
                .root
                .join(format!("{validator}.transport.stdout.json")),
        )
        .expect("create transport stdout");
        let transport_err = fs::File::create(
            harness
                .root
                .join(format!("{validator}.transport.stderr.log")),
        )
        .expect("create transport stderr");
        let child = Command::new(node_bin())
            .env("POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE", &transport_ready)
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
                data_dir.to_str().expect("data dir UTF-8"),
                "--topology",
                topology.to_str().expect("topology UTF-8"),
                "--key-file",
                key.to_str().expect("key path UTF-8"),
                "--vote-dir",
                harness
                    .root
                    .join(format!("{validator}.votes"))
                    .to_str()
                    .expect("vote dir UTF-8"),
                "--max-connections",
                "100",
                "--timeout-ms",
                "90000",
            ])
            .stdout(Stdio::from(transport_log))
            .stderr(Stdio::from(transport_err))
            .spawn()
            .expect("spawn validator transport service");
        harness.children.push(child);
        ready_files.push(transport_ready);

        let rpc_ready = harness.root.join(format!("{validator}.rpc.ready.json"));
        let rpc_log = fs::File::create(harness.root.join(format!("{validator}.rpc.stdout.json")))
            .expect("create RPC stdout");
        let rpc_err = fs::File::create(harness.root.join(format!("{validator}.rpc.stderr.log")))
            .expect("create RPC stderr");
        let child = Command::new(node_bin())
            .env("POSTFIAT_PREWARM_SHIELDED_VERIFIER", "1")
            .env("POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER", "1")
            .env(
                "POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER",
                "1",
            )
            .args([
                "rpc-serve",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                data_dir.to_str().expect("data dir UTF-8"),
                "--spool-dir",
                harness
                    .root
                    .join(format!("{validator}.rpc-spool"))
                    .to_str()
                    .expect("spool dir UTF-8"),
                "--ready-file",
                rpc_ready.to_str().expect("ready path UTF-8"),
                "--port",
                &port.to_string(),
                "--max-requests",
                "10000",
                "--timeout-ms",
                "90000",
                "--child-timeout-ms",
                "90000",
                "--allow-mempool-submit",
                "--allow-mempool-submit-finality",
                "--finality-topology",
                topology.to_str().expect("topology UTF-8"),
                "--finality-key-file",
                key.to_str().expect("key path UTF-8"),
                "--finality-artifact-root",
                harness
                    .root
                    .join(format!("{validator}.finality"))
                    .to_str()
                    .expect("artifact root UTF-8"),
                "--finality-timeout-ms",
                "90000",
                "--finality-send-retries",
                "2",
                "--finality-retry-backoff-ms",
                "25",
                "--keep-alive",
            ])
            .stdout(Stdio::from(rpc_log))
            .stderr(Stdio::from(rpc_err))
            .spawn()
            .expect("spawn TCP RPC service");
        harness.children.push(child);
        ready_files.push(rpc_ready);
    }
    ready_files
}

fn status_tuple(port: u16, id: &str) -> (u64, String, String) {
    let response = rpc_call(port, &status_request(id));
    let result = response.result.expect("status result");
    (
        result["block_height"].as_u64().expect("status height"),
        result["block_tip_hash"]
            .as_str()
            .expect("status tip")
            .to_string(),
        result["state_root"]
            .as_str()
            .expect("status root")
            .to_string(),
    )
}

fn wait_exact_six(ports: &[u16], expected: &(u64, String, String)) {
    assert_eq!(ports.len(), VALIDATORS, "six-validator observer count");
    wait_exact_ports(ports, expected);
}

fn wait_exact_ports(ports: &[u16], expected: &(u64, String, String)) {
    assert!(
        !ports.is_empty(),
        "finality observer ports must not be empty"
    );
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let observed = ports
            .iter()
            .enumerate()
            .map(|(index, port)| status_tuple(*port, &format!("converge-{index}")))
            .collect::<Vec<_>>();
        if observed.iter().all(|value| value == expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "validator convergence timeout: expected {expected:?}, observed {observed:?}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn submit_asset_finality(
    harness: &Harness,
    ports: &[u16],
    signer: &WalletBackupFile,
    operation: AssetTransactionOperation,
    label: &str,
) -> (u64, String, String) {
    let parent = status_tuple(ports[0], &format!("{label}-parent"));
    let next_height = parent.0 + 1;
    let proposer_json = command_json(&[
        "block-proposer",
        "--data-dir",
        harness.node(0).to_str().expect("node path UTF-8"),
        "--height",
        &next_height.to_string(),
        "--view",
        "0",
    ]);
    let proposer_index = proposer_json["proposer"]
        .as_str()
        .expect("asset-finality proposer")
        .strip_prefix("validator-")
        .expect("validator proposer prefix")
        .parse::<usize>()
        .expect("validator proposer index");
    let proposer_port = ports[proposer_index];
    let identity = wallet_identity_from_backup(signer).expect("asset-finality signer identity");
    let quote_response = rpc_call(
        proposer_port,
        &asset_fee_quote_request(
            format!("{label}-quote"),
            identity.address,
            serde_json::to_string(&operation).expect("serialize finality operation"),
            None,
        ),
    );
    let quote = decode_asset_fee_quote_summary(&quote_response).expect("decode asset quote");
    let signed = wallet_sign_asset_transaction_from_fields(
        signer,
        WalletSignAssetTransactionFields {
            chain_id: quote.chain_id,
            genesis_hash: quote.genesis_hash,
            protocol_version: quote.protocol_version,
            source: quote.source,
            fee: quote.minimum_fee,
            sequence: quote.sequence,
            operation,
        },
    )
    .expect("sign asset-finality operation");
    let response = rpc_call(
        proposer_port,
        &RpcRequest::new(
            format!("{label}-finality"),
            "mempool_submit_signed_asset_transaction_finality",
            json!({
                "signed_asset_transaction_json": serde_json::to_string(&signed)
                    .expect("serialize signed asset-finality transaction")
            }),
        ),
    );
    let result = response.result.expect("asset-finality result");
    let finality = &result["finality"];
    assert_eq!(finality["confirmed"], true, "{label} confirmation");
    assert_eq!(
        finality["receipt"]["accepted"], true,
        "{label} receipt: {finality}"
    );
    let expected = (
        finality["block"]["header"]["height"]
            .as_u64()
            .expect("asset-finality block height"),
        finality["block"]["header"]["block_hash"]
            .as_str()
            .expect("asset-finality block hash")
            .to_string(),
        finality["tip_state_root"]
            .as_str()
            .expect("asset-finality state root")
            .to_string(),
    );
    assert_eq!(expected.0, next_height, "{label} next height");
    wait_exact_six(ports, &expected);
    expected
}

fn submit_dev_key_asset_finality(
    harness: &Harness,
    ports: &[u16],
    signer: &DevKeyFile,
    operation: AssetTransactionOperation,
    label: &str,
) -> (u64, String, String) {
    submit_dev_key_asset_finality_observed(harness, ports, ports, signer, operation, label)
}

fn submit_dev_key_asset_finality_observed(
    harness: &Harness,
    ports: &[u16],
    observer_ports: &[u16],
    signer: &DevKeyFile,
    operation: AssetTransactionOperation,
    label: &str,
) -> (u64, String, String) {
    assert_eq!(
        signer.algorithm_id, ML_DSA_65_ALGORITHM,
        "dev-key asset signer algorithm"
    );
    let parent = status_tuple(ports[0], &format!("{label}-parent"));
    let next_height = parent.0 + 1;
    let proposer_json = command_json(&[
        "block-proposer",
        "--data-dir",
        harness.node(0).to_str().expect("node path UTF-8"),
        "--height",
        &next_height.to_string(),
        "--view",
        "0",
    ]);
    let proposer_index = proposer_json["proposer"]
        .as_str()
        .expect("dev-key asset-finality proposer")
        .strip_prefix("validator-")
        .expect("validator proposer prefix")
        .parse::<usize>()
        .expect("validator proposer index");
    let proposer_port = ports[proposer_index];
    let quote_response = rpc_call(
        proposer_port,
        &asset_fee_quote_request(
            format!("{label}-quote"),
            signer.address.clone(),
            serde_json::to_string(&operation).expect("serialize dev-key asset operation"),
            None,
        ),
    );
    let quote =
        decode_asset_fee_quote_summary(&quote_response).expect("decode dev-key asset quote");
    let unsigned = UnsignedAssetTransaction {
        chain_id: quote.chain_id,
        genesis_hash: quote.genesis_hash,
        protocol_version: quote.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: quote.transaction_kind,
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: quote.source,
        fee: quote.minimum_fee,
        sequence: quote.sequence,
        operation,
    };
    let private_key = hex_to_bytes(&signer.private_key_hex).expect("decode dev-key asset signer");
    let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes())
        .expect("sign dev-key asset transaction");
    let signed = SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: signer.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    };
    let response = rpc_call(
        proposer_port,
        &RpcRequest::new(
            format!("{label}-finality"),
            "mempool_submit_signed_asset_transaction_finality",
            json!({
                "signed_asset_transaction_json": serde_json::to_string(&signed)
                    .expect("serialize dev-key asset-finality transaction")
            }),
        ),
    );
    let result = response.result.expect("dev-key asset-finality result");
    let finality = &result["finality"];
    assert_eq!(finality["confirmed"], true, "{label} confirmation");
    assert_eq!(
        finality["receipt"]["accepted"], true,
        "{label} receipt: {finality}"
    );
    let expected = (
        finality["block"]["header"]["height"]
            .as_u64()
            .expect("dev-key asset-finality block height"),
        finality["block"]["header"]["block_hash"]
            .as_str()
            .expect("dev-key asset-finality block hash")
            .to_string(),
        finality["tip_state_root"]
            .as_str()
            .expect("dev-key asset-finality state root")
            .to_string(),
    );
    assert_eq!(expected.0, next_height, "{label} next height");
    wait_exact_ports(observer_ports, &expected);
    expected
}

fn submit_faucet_transfer_finality(
    harness: &Harness,
    ports: &[u16],
    faucet: &DevKeyFile,
    recipient: &str,
    amount: u64,
    label: &str,
) -> (u64, String, String) {
    assert_eq!(
        faucet.algorithm_id, ML_DSA_65_ALGORITHM,
        "test faucet must use the transaction signature algorithm"
    );
    let parent = status_tuple(ports[0], &format!("{label}-parent"));
    let next_height = parent.0 + 1;
    let proposer_json = command_json(&[
        "block-proposer",
        "--data-dir",
        harness.node(0).to_str().expect("node path UTF-8"),
        "--height",
        &next_height.to_string(),
        "--view",
        "0",
    ]);
    let proposer_index = proposer_json["proposer"]
        .as_str()
        .expect("transfer-finality proposer")
        .strip_prefix("validator-")
        .expect("validator proposer prefix")
        .parse::<usize>()
        .expect("validator proposer index");
    let proposer_port = ports[proposer_index];
    let quote_response = rpc_call(
        proposer_port,
        &transfer_fee_quote_request(
            format!("{label}-quote"),
            faucet.address.clone(),
            recipient.to_string(),
            amount,
            None,
        ),
    );
    let quote = decode_transfer_fee_quote_summary(&quote_response).expect("decode transfer quote");
    assert_eq!(quote.from, faucet.address, "{label} quote sender");
    assert_eq!(quote.to, recipient, "{label} quote recipient");
    assert_eq!(quote.amount, amount, "{label} quote amount");
    let unsigned = UnsignedTransfer {
        chain_id: quote.chain_id,
        genesis_hash: quote.genesis_hash,
        protocol_version: quote.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: quote.from,
        to: quote.to,
        amount: quote.amount,
        fee: quote.minimum_fee,
        sequence: quote.sequence,
    };
    let private_key = hex_to_bytes(&faucet.private_key_hex).expect("decode test faucet key");
    let signature =
        ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign test faucet transfer");
    let signed = SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: faucet.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    };
    let response = rpc_call(
        proposer_port,
        &RpcRequest::new(
            format!("{label}-finality"),
            "mempool_submit_signed_transfer_finality",
            json!({
                "signed_transfer_json": serde_json::to_string(&signed)
                    .expect("serialize signed transfer-finality transaction")
            }),
        ),
    );
    let result = response.result.expect("transfer-finality result");
    let finality = &result["finality"];
    assert_eq!(finality["confirmed"], true, "{label} confirmation");
    assert_eq!(
        finality["receipt"]["accepted"], true,
        "{label} receipt: {finality}"
    );
    let expected = (
        finality["block"]["header"]["height"]
            .as_u64()
            .expect("transfer-finality block height"),
        finality["block"]["header"]["block_hash"]
            .as_str()
            .expect("transfer-finality block hash")
            .to_string(),
        finality["tip_state_root"]
            .as_str()
            .expect("transfer-finality state root")
            .to_string(),
    );
    assert_eq!(expected.0, next_height, "{label} next height");
    wait_exact_six(ports, &expected);
    expected
}

fn resume_outbox(data_dir: &Path, topology: &Path) -> Value {
    let output = Command::new(node_bin())
        .args([
            "transport-certified-send-outbox-resume",
            "--data-dir",
            data_dir.to_str().expect("outbox data dir UTF-8"),
            "--topology",
            topology.to_str().expect("outbox topology UTF-8"),
            "--max-jobs",
            "32",
        ])
        .output()
        .expect("resume durable certified-send outbox");
    assert!(
        output.status.success(),
        "durable certified-send outbox resume failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse durable outbox report")
}

#[test]
#[ignore = "mandatory provider-neutral reserve proof six-validator finality/restart smoke"]
fn provider_neutral_qnav_proof_finalizes_and_survives_six_validator_restart() {
    const QNAV_CHAIN_ID: &str = "postfiat-wan-devnet-2";
    const QNAV_ASSET_ID: &str = "3f631473a34a48cd47b4e1067546a9ccc5fcfe2f6e103655191d600d9574a5b2e6a985b7c52dcff7c9461aac872a12f5";
    const QNAV_PROFILE_ID: &str = "3d78cac1f539d3d2e56f6f38c958242aa0bcd13661c733834896bc9c49a48211d716bd4cad83d478b2fa5d85b22a0c7e";

    let mut harness = Harness::new();
    let seed_dir = harness.root.join("qnav-seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: QNAV_CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: VALIDATORS as u32,
    })
    .expect("initialize qNAV six-validator seed");
    activate_consensus_v2_in_fresh_genesis(&seed_dir);
    let qualified_genesis: Genesis = serde_json::from_slice(
        &fs::read(seed_dir.join("genesis.json")).expect("read qNAV genesis"),
    )
    .expect("parse qNAV genesis");
    assert_eq!(
        genesis_hash(&qualified_genesis),
        "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9"
    );

    let issuer = backup_for_chain(QNAV_CHAIN_ID, 0x66);
    let operator = backup_for_chain(QNAV_CHAIN_ID, 0x77);
    let issuer_id = wallet_identity_from_backup(&issuer).expect("qNAV issuer identity");
    let operator_id = wallet_identity_from_backup(&operator).expect("qNAV operator identity");
    assert_eq!(
        issuer_id.address,
        "pf0fae169e4293feebc8c9119febb4fd995a667b37"
    );
    let faucet = faucet_key(NodeOptions {
        data_dir: seed_dir.clone(),
    })
    .expect("read qNAV test faucet");
    assert_eq!(
        issued_asset_id(QNAV_CHAIN_ID, &issuer_id.address, "qNAV", 1)
            .expect("derive qNAV asset ID"),
        QNAV_ASSET_ID
    );
    let profile_operation = successor_nav_profile_operation(&issuer_id.address);
    let profile_id = profile_operation
        .to_profile()
        .expect("derive qNAV provider-neutral profile")
        .profile_id;
    assert_eq!(profile_id, QNAV_PROFILE_ID);

    for index in 0..VALIDATORS {
        copy_dir(&seed_dir, &harness.node(index));
        rewrite_node_identity(&harness.node(index), &format!("validator-{index}"));
    }
    let base_port = free_base_port();
    let topology_path = harness.root.join("qnav-topology.json");
    let topology = local_topology(
        NetworkDomain {
            chain_id: qualified_genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&qualified_genesis),
            protocol_version: qualified_genesis.protocol_version,
        },
        VALIDATORS as u32,
        base_port,
    )
    .expect("build qNAV topology");
    fs::write(
        &topology_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&topology).expect("serialize qNAV topology")
        ),
    )
    .expect("write qNAV topology");
    let rpc_ports = topology
        .peers
        .iter()
        .map(|peer| peer.rpc_port)
        .collect::<Vec<_>>();
    let ready = spawn_services(&mut harness, &topology_path, &rpc_ports);
    for path in &ready {
        wait_for_file(path, Duration::from_secs(90));
    }

    submit_faucet_transfer_finality(
        &harness,
        &rpc_ports,
        &faucet,
        &issuer_id.address,
        1_000_000,
        "fund-qnav-issuer",
    );
    submit_faucet_transfer_finality(
        &harness,
        &rpc_ports,
        &faucet,
        &operator_id.address,
        1_000_000,
        "fund-qnav-operator",
    );
    submit_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer_id.address.clone(),
            code: "qNAV".to_string(),
            version: 1,
            precision: 6,
            display_name: "Independent provider-neutral qualification NAVCoin".to_string(),
            max_supply: None,
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        "create-qnav",
    );
    submit_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavProfileRegister(profile_operation),
        "register-qnav-proof-profile",
    );
    submit_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer_id.address.clone(),
            asset_id: QNAV_ASSET_ID.to_string(),
            reserve_operator: operator_id.address.clone(),
            proof_profile: profile_id,
            valuation_unit: "qualification-unit".to_string(),
            redemption_account: issuer_id.address.clone(),
        }),
        "register-qnav",
    );

    let proof = postfiat_crypto_provider::hex_to_bytes(
        include_str!("../../execution/testdata/nav-reserve-v1-qualified-proof-calldata.hex").trim(),
    )
    .expect("decode qNAV proof fixture");
    let public_values = postfiat_crypto_provider::hex_to_bytes(
        include_str!("../../execution/testdata/nav-reserve-v1-qualified-public-values.hex").trim(),
    )
    .expect("decode qNAV public values fixture");
    let reserve_packet_hash = "55".repeat(48);
    let submitted = submit_asset_finality(
        &harness,
        &rpc_ports,
        &operator,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer_id.address.clone(),
            submitter: operator_id.address.clone(),
            asset_id: QNAV_ASSET_ID.to_string(),
            epoch: 7,
            nav_per_unit: 7_000_000,
            circulating_supply: 0,
            verified_net_assets: 1_100,
            proof_profile: QNAV_PROFILE_ID.to_string(),
            source_root: "f4bdaca02e5445e7d2c666ca692d45d63fe1c423f6b03067e9eee19f5f9334fe60920b8528feff2656d5dbe7d28d415f".to_string(),
            attestor_root: "cb34590e25db391724491b01795dee8bdbbadba3bba36fb5fc4f96bce1a87fa311426e0b76ce5ff4d775b091d94147df".to_string(),
            reserve_packet_hash: reserve_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: proof,
            sp1_public_values: public_values,
        }),
        "qnav-reserve-submit",
    );
    let finalized = submit_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer_id.address.clone(),
            asset_id: QNAV_ASSET_ID.to_string(),
            epoch: 7,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
        "qnav-epoch-finalize",
    );
    assert_eq!(finalized.0, submitted.0 + 1);

    for index in 0..VALIDATORS {
        let ledger: LedgerState = serde_json::from_slice(
            &fs::read(harness.node(index).join("ledger.json")).expect("read qNAV ledger"),
        )
        .expect("parse qNAV ledger");
        let profile = ledger
            .nav_proof_profile(QNAV_PROFILE_ID)
            .expect("qNAV profile after finality");
        assert_eq!(
            profile.verifier_kind,
            NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1
        );
        let packet = ledger
            .nav_reserve_packets
            .iter()
            .find(|packet| packet.asset_id == QNAV_ASSET_ID && packet.epoch == 7)
            .expect("qNAV packet after finality");
        assert_eq!(packet.state, "finalized", "validator {index}");
        assert_eq!(packet.proof_verified_net_assets, 1_100, "validator {index}");
        assert_eq!(packet.controlled_value, 1_100, "validator {index}");
        assert_eq!(packet.sp1_proof_bytes.len(), 356, "validator {index}");
        assert_eq!(packet.sp1_public_values.len(), 584, "validator {index}");
    }

    for child in &mut harness.children {
        let _ = child.kill();
        let _ = child.wait();
    }
    harness.children.clear();
    for path in &ready {
        let _ = fs::remove_file(path);
    }
    let restarted_ready = spawn_services(&mut harness, &topology_path, &rpc_ports);
    for path in &restarted_ready {
        wait_for_file(path, Duration::from_secs(90));
    }
    wait_exact_six(&rpc_ports, &finalized);
    for (index, port) in rpc_ports.iter().enumerate() {
        let verified = rpc_call(
            *port,
            &verify_state_request(format!("qnav-restart-{index}")),
        );
        assert_eq!(
            verified.result.expect("qNAV verify_state result")["verified"],
            true,
            "validator {index} qNAV restart verification"
        );
    }

    for child in &mut harness.children {
        let _ = child.kill();
        let _ = child.wait();
    }
    harness.children.clear();
    let snapshot_dir = harness.root.join("qnav-finalized.snapshot");
    let restored_dir = harness.root.join("qnav-finalized-restored");
    let snapshot = export_snapshot(SnapshotExportOptions {
        data_dir: harness.node(0),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export finalized provider-neutral qNAV snapshot");
    assert_eq!(snapshot.block_height, finalized.0);
    assert_eq!(snapshot.block_tip_hash, finalized.1);
    assert_eq!(snapshot.state_root, finalized.2);
    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir,
        node_id: Some("qnav-snapshot-restored".to_string()),
    })
    .expect("restore finalized provider-neutral qNAV snapshot");
    assert_eq!(restored.block_height, finalized.0);
    assert_eq!(restored.block_tip_hash, finalized.1);
    assert_eq!(restored.state_root, finalized.2);
    verify_blocks(NodeOptions {
        data_dir: restored_dir.clone(),
    })
    .expect("replay finalized provider-neutral qNAV snapshot history");
    let restored_ledger: LedgerState = serde_json::from_slice(
        &fs::read(restored_dir.join("ledger.json")).expect("read restored qNAV ledger"),
    )
    .expect("parse restored qNAV ledger");
    assert_eq!(
        restored_ledger
            .nav_proof_profile(QNAV_PROFILE_ID)
            .expect("restored qNAV profile")
            .source_manifest_hash,
        "9da4e2ba55939f138475026946d2728d9b40d3f4c7762289a70aae94584eac924b9a788c6df25c9276cc83f1616ef0e5"
    );
    assert_eq!(
        restored_ledger
            .nav_reserve_packets
            .iter()
            .find(|packet| packet.asset_id == QNAV_ASSET_ID && packet.epoch == 7)
            .expect("restored qNAV packet")
            .public_values_schema,
        NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1
    );
}

#[test]
#[ignore = "exact A666 public-successor six-validator migration rehearsal"]
fn a666_public_successor_proof_migrates_and_survives_six_validator_restart() {
    const A666_CHAIN_ID: &str = "postfiat-wan-devnet-2";
    const A666_ASSET_ID: &str = "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c";
    const A666_ISSUER: &str = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b";
    const A666_RESERVE_OPERATOR: &str = "pfd0c86d9084915e1fefd22eab891806397d5a5937";
    const A666_SUCCESSOR_PROFILE: &str = "f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a44171fc1e7239bc25e06ad833c14e91";
    const A666_CIRCULATING_SUPPLY: u64 = 31_597_197_455;
    const A666_VALUATION_UNIT: &str = "USD_1E8";
    const PFUSDC_VALUATION_UNIT: &str = "USDC";

    let issuer = read_dev_key_from_env("POSTFIAT_A666_ISSUER_KEY_FILE");
    let reserve_operator = read_dev_key_from_env("POSTFIAT_A666_RESERVE_KEY_FILE");
    let pfusdc_issuer = read_dev_key_from_env("POSTFIAT_PFUSDC_ISSUER_KEY_FILE");
    let holder = read_dev_key_from_env("POSTFIAT_A666_HOLDER_KEY_FILE");
    let holder_key_file = required_env_path("POSTFIAT_A666_HOLDER_KEY_FILE");
    assert_eq!(issuer.address, A666_ISSUER);
    assert_eq!(reserve_operator.address, A666_RESERVE_OPERATOR);
    assert_eq!(
        pfusdc_issuer.address,
        "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8"
    );
    let proof = read_raw_or_hex_from_env("POSTFIAT_A666_PROOF_CALLDATA_FILE");
    let public_values_bytes = read_raw_or_hex_from_env("POSTFIAT_A666_PUBLIC_VALUES_FILE");
    let next_proof = read_raw_or_hex_from_env("POSTFIAT_A666_NEXT_PROOF_CALLDATA_FILE");
    let next_public_values_bytes =
        read_raw_or_hex_from_env("POSTFIAT_A666_NEXT_PUBLIC_VALUES_FILE");
    assert!(!proof.is_empty() && proof.len() <= 4_096);
    assert!(!next_proof.is_empty() && next_proof.len() <= 4_096);
    let public_values = NavReservePublicValuesV1::decode(&public_values_bytes)
        .expect("decode exact A666 public values");
    let next_public_values = NavReservePublicValuesV1::decode(&next_public_values_bytes)
        .expect("decode next exact A666 public values");
    assert_eq!(public_values.pftl_genesis_hash, "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9");
    assert_eq!(public_values.nav_asset_id, A666_ASSET_ID);
    assert_eq!(public_values.proof_profile_id, A666_SUCCESSOR_PROFILE);
    assert_eq!(public_values.quantity_trust_counts.cryptographic, 6);
    assert_eq!(public_values.valuation_trust_counts.cryptographic, 6);
    assert_eq!(public_values.attested_value, 0);
    assert_eq!(public_values.controlled_value, 0);
    assert_eq!(
        next_public_values.observation_epoch,
        public_values.observation_epoch + 1
    );
    assert_eq!(
        next_public_values.pftl_genesis_hash,
        public_values.pftl_genesis_hash
    );
    assert_eq!(next_public_values.nav_asset_id, A666_ASSET_ID);
    assert_eq!(next_public_values.proof_profile_id, A666_SUCCESSOR_PROFILE);
    assert_eq!(next_public_values.quantity_trust_counts.cryptographic, 6);
    assert_eq!(next_public_values.valuation_trust_counts.cryptographic, 6);
    assert_eq!(next_public_values.attested_value, 0);
    assert_eq!(next_public_values.controlled_value, 0);

    let mut harness = Harness::new();
    let seed_dir = harness.root.join("a666-public-successor-seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: A666_CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: VALIDATORS as u32,
    })
    .expect("initialize exact A666 migration seed");
    activate_consensus_v2_in_fresh_genesis(&seed_dir);
    let qualified_genesis: Genesis = serde_json::from_slice(
        &fs::read(seed_dir.join("genesis.json")).expect("read exact A666 genesis"),
    )
    .expect("parse exact A666 genesis");
    assert_eq!(
        genesis_hash(&qualified_genesis),
        public_values.pftl_genesis_hash
    );
    let faucet = faucet_key(NodeOptions {
        data_dir: seed_dir.clone(),
    })
    .expect("read exact A666 rehearsal faucet");
    let data_dirs = (0..VALIDATORS)
        .map(|index| {
            let data_dir = harness.node(index);
            copy_dir(&seed_dir, &data_dir);
            rewrite_node_identity(&data_dir, &format!("validator-{index}"));
            data_dir
        })
        .collect::<Vec<_>>();
    advance_certified_chain_to_height(
        &data_dirs,
        public_values
            .observation_not_after
            .max(next_public_values.observation_not_after),
    );

    let base_port = free_base_port();
    let topology_path = harness.root.join("a666-public-successor-topology.json");
    let topology = local_topology(
        NetworkDomain {
            chain_id: qualified_genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&qualified_genesis),
            protocol_version: qualified_genesis.protocol_version,
        },
        VALIDATORS as u32,
        base_port,
    )
    .expect("build exact A666 migration topology");
    fs::write(
        &topology_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&topology)
                .expect("serialize exact A666 migration topology")
        ),
    )
    .expect("write exact A666 migration topology");
    let rpc_ports = topology
        .peers
        .iter()
        .map(|peer| peer.rpc_port)
        .collect::<Vec<_>>();
    let mut ready = spawn_services(&mut harness, &topology_path, &rpc_ports);
    for path in &ready {
        wait_for_file(path, Duration::from_secs(90));
    }

    submit_faucet_transfer_finality(
        &harness,
        &rpc_ports,
        &faucet,
        A666_ISSUER,
        1_000_000,
        "fund-a666-issuer",
    );
    submit_faucet_transfer_finality(
        &harness,
        &rpc_ports,
        &faucet,
        A666_RESERVE_OPERATOR,
        1_000_000,
        "fund-a666-reserve-operator",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: A666_ISSUER.to_string(),
            code: "A666".to_string(),
            version: 2,
            precision: 6,
            display_name: "Post Fiat NAVCoin a666".to_string(),
            max_supply: None,
            requires_authorization: true,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        "create-exact-a666-v2",
    );
    assert_eq!(
        issued_asset_id(A666_CHAIN_ID, A666_ISSUER, "A666", 2).expect("derive exact A666 ID"),
        A666_ASSET_ID
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: A666_ISSUER.to_string(),
            to: A666_RESERVE_OPERATOR.to_string(),
            issuer: A666_ISSUER.to_string(),
            asset_id: A666_ASSET_ID.to_string(),
            amount: A666_CIRCULATING_SUPPLY,
        }),
        "recreate-exact-a666-circulating-supply",
    );
    let legacy_profile_operation = NavProfileRegisterOperation {
        registrant: A666_ISSUER.to_string(),
        verifier_kind: NAV_PROFILE_VERIFIER_PLACEHOLDER.to_string(),
        source_class: "a666-legacy-migration-rehearsal".to_string(),
        max_snapshot_age_blocks: 0,
        challenge_window_blocks: 0,
        max_epoch_gap_blocks: 0,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 0,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: String::new(),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey: String::new(),
        sp1_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
        public_values_schema: String::new(),
        source_manifest_hash: String::new(),
        valuation_unit_id: String::new(),
        max_observation_span_blocks: 0,
        allow_controlled_sources: false,
    };
    let legacy_profile_id = legacy_profile_operation
        .to_profile()
        .expect("derive legacy A666 rehearsal profile")
        .profile_id;
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavProfileRegister(legacy_profile_operation),
        "register-a666-legacy-profile",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: A666_ISSUER.to_string(),
            asset_id: A666_ASSET_ID.to_string(),
            reserve_operator: A666_RESERVE_OPERATOR.to_string(),
            proof_profile: legacy_profile_id,
            valuation_unit: "USDC".to_string(),
            redemption_account: A666_ISSUER.to_string(),
        }),
        "bind-a666-legacy-profile",
    );
    let successor_operation = a666_public_successor_profile_operation(A666_ISSUER);
    assert_eq!(
        successor_operation
            .to_profile()
            .expect("derive exact A666 successor profile")
            .profile_id,
        A666_SUCCESSOR_PROFILE
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavProfileRegister(successor_operation),
        "register-a666-public-successor",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: A666_ISSUER.to_string(),
            asset_id: A666_ASSET_ID.to_string(),
            reserve_operator: A666_RESERVE_OPERATOR.to_string(),
            proof_profile: A666_SUCCESSOR_PROFILE.to_string(),
            valuation_unit: A666_VALUATION_UNIT.to_string(),
            redemption_account: A666_ISSUER.to_string(),
        }),
        "rebind-a666-public-successor",
    );
    let nav_per_unit = (u128::from(public_values.verified_net_assets) * 1_000_000_u128
        / u128::from(A666_CIRCULATING_SUPPLY)) as u64;
    assert_eq!(nav_per_unit, 89_748_188);
    let reserve_packet_hash = "a6".repeat(48);
    let submitted = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: A666_ISSUER.to_string(),
            submitter: A666_RESERVE_OPERATOR.to_string(),
            asset_id: A666_ASSET_ID.to_string(),
            epoch: public_values.observation_epoch,
            nav_per_unit,
            circulating_supply: A666_CIRCULATING_SUPPLY,
            verified_net_assets: public_values.verified_net_assets,
            proof_profile: A666_SUCCESSOR_PROFILE.to_string(),
            source_root: public_values.source_observation_root.clone(),
            attestor_root: public_values.valuation_trust_root.clone(),
            reserve_packet_hash: reserve_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: proof,
            sp1_public_values: public_values_bytes,
        }),
        "submit-a666-public-reserve-proof",
    );
    let mut finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: A666_ISSUER.to_string(),
            asset_id: A666_ASSET_ID.to_string(),
            epoch: public_values.observation_epoch,
            reserve_packet_hash: reserve_packet_hash.clone(),
        }),
        "finalize-a666-public-reserve-proof",
    );
    assert_eq!(finalized.0, submitted.0 + 1);

    for index in 0..VALIDATORS {
        let ledger: LedgerState = serde_json::from_slice(
            &fs::read(harness.node(index).join("ledger.json")).expect("read A666 ledger"),
        )
        .expect("parse A666 ledger");
        assert_eq!(
            ledger
                .nav_assets
                .iter()
                .find(|asset| asset.asset_id == A666_ASSET_ID)
                .expect("A666 NAV binding")
                .proof_profile,
            A666_SUCCESSOR_PROFILE,
            "validator {index}"
        );
        let profile = ledger
            .nav_proof_profile(A666_SUCCESSOR_PROFILE)
            .expect("A666 successor profile after finality");
        assert!(!profile.allow_controlled_sources, "validator {index}");
        let packet = ledger
            .nav_reserve_packets
            .iter()
            .find(|packet| {
                packet.asset_id == A666_ASSET_ID && packet.epoch == public_values.observation_epoch
            })
            .expect("A666 public reserve packet after finality");
        assert_eq!(packet.state, "finalized", "validator {index}");
        assert_eq!(packet.nav_per_unit, nav_per_unit, "validator {index}");
        assert_eq!(
            packet.circulating_supply, A666_CIRCULATING_SUPPLY,
            "validator {index}"
        );
        assert_eq!(
            packet.proof_verified_net_assets, public_values.verified_net_assets,
            "validator {index}"
        );
        assert_eq!(packet.attested_value, 0, "validator {index}");
        assert_eq!(packet.controlled_value, 0, "validator {index}");
    }

    stop_services(&mut harness);
    for path in &ready {
        let _ = fs::remove_file(path);
    }
    let committee = bootstrap_fastswap_committee(&harness, &data_dirs);
    ready = start_services(&mut harness, &topology_path, &rpc_ports);
    finalized = status_tuple(rpc_ports[0], "a666-after-fastswap-bootstrap");
    wait_exact_six(&rpc_ports, &finalized);

    const PFUSDC_ASSET_ID: &str = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";
    const ROUTE_ID: &str = "pftl-a666-ethereum-wA666-usdc-v1";
    const HANDOFF_CONTROLLER: &str = "0x9a0262c0572fb4db08765408eb225e207f40c3d9";
    const SETTLEMENT_ADAPTER: &str = "0x9a0262c0572fb4db08765408eb225e207f40c3d9";
    const WRAPPED_A666: &str = "0xee4c92edb03efdd9b519339edc19ad70c69a9be5";
    const HOLDER_ETHEREUM: &str = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0";
    const PFUSDC_SUPPLY: u64 = 20_000_000;

    assert_eq!(
        issued_asset_id(A666_CHAIN_ID, &pfusdc_issuer.address, "PFUSDC", 1)
            .expect("derive exact pfUSDC ID"),
        PFUSDC_ASSET_ID
    );
    submit_faucet_transfer_finality(
        &harness,
        &rpc_ports,
        &faucet,
        &pfusdc_issuer.address,
        1_000_000,
        "fund-exact-pfusdc-issuer",
    );
    submit_faucet_transfer_finality(
        &harness,
        &rpc_ports,
        &faucet,
        &holder.address,
        10_000_000,
        "fund-a666-lifecycle-holder",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &pfusdc_issuer,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: pfusdc_issuer.address.clone(),
            code: "PFUSDC".to_string(),
            version: 1,
            precision: 6,
            display_name: "proof-native pfUSDC".to_string(),
            max_supply: Some(1_000_000_000_000_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: false,
        }),
        "create-exact-pfusdc",
    );
    let pfusdc_bridge_policy_hash = "42".repeat(48);
    // This controlled observer exists only to drive the pfUSDC vault state
    // machine in an isolated migration rehearsal. It does not qualify a
    // pfUSDC source and does not replace the live Tier-4 Ethereum receipt
    // proof path. The A666 epochs below remain the exact six-source Groth16
    // proofs with zero attested or controlled reserve value.
    let mut pfusdc_bridge_evidence = VaultBridgeDepositEvidence {
        source_chain_id: 1,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        token_address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
        depositor: HOLDER_ETHEREUM.to_string(),
        pftl_recipient_hash: vault_bridge_pftl_recipient_hash(&holder.address)
            .expect("pfUSDC controlled recipient hash"),
        pftl_recipient: holder.address.clone(),
        amount_atoms: PFUSDC_SUPPLY,
        nonce: "41".repeat(32),
        route_binding: "42".repeat(32),
        deposit_id: String::new(),
        block_hash: "43".repeat(32),
        tx_hash: "44".repeat(32),
        log_index: 0,
    };
    pfusdc_bridge_evidence.deposit_id =
        vault_bridge_deposit_id(&pfusdc_bridge_evidence).expect("pfUSDC controlled deposit ID");
    let pfusdc_bridge_source_domain = pfusdc_bridge_evidence.source_domain();
    let pfusdc_bridge_evidence_root = vault_bridge_deposit_evidence_root(&pfusdc_bridge_evidence)
        .expect("pfUSDC controlled evidence root");
    let pfusdc_profile_operation = NavProfileRegisterOperation {
        registrant: pfusdc_issuer.address.clone(),
        verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
        source_class: format!("vault_bridge:{pfusdc_bridge_source_domain}"),
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 100,
        settle_deadline_blocks: 0,
        min_challenge_bond: 0,
        min_attestations: 1,
        tolerance_bp: 0,
        bridge_observer_min_confirmations: 12,
        valuation_policy_hash: pfusdc_bridge_policy_hash.clone(),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey: String::new(),
        sp1_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
        public_values_schema: String::new(),
        source_manifest_hash: String::new(),
        valuation_unit_id: String::new(),
        max_observation_span_blocks: 0,
        allow_controlled_sources: false,
    };
    let pfusdc_profile = pfusdc_profile_operation
        .to_profile()
        .expect("derive pfUSDC rehearsal profile")
        .profile_id;
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &pfusdc_issuer,
        AssetTransactionOperation::NavProfileRegister(pfusdc_profile_operation),
        "register-pfusdc-accounting-profile",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &pfusdc_issuer,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: pfusdc_issuer.address.clone(),
            asset_id: PFUSDC_ASSET_ID.to_string(),
            reserve_operator: pfusdc_issuer.address.clone(),
            proof_profile: pfusdc_profile.clone(),
            valuation_unit: PFUSDC_VALUATION_UNIT.to_string(),
            redemption_account: pfusdc_issuer.address.clone(),
        }),
        "register-pfusdc-accounting-nav",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
            attestor: holder.address.clone(),
            domain: "controlled-pfusdc-overlay.local".to_string(),
            bond: 0,
        }),
        "register-controlled-pfusdc-observer",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::VaultBridgeDepositPropose(VaultBridgeDepositProposeOperation {
            proposer: holder.address.clone(),
            asset_id: PFUSDC_ASSET_ID.to_string(),
            evidence_root: pfusdc_bridge_evidence_root.clone(),
            evidence: pfusdc_bridge_evidence.clone(),
            policy_hash: pfusdc_bridge_policy_hash.clone(),
            source_proof_kind: String::new(),
            source_proof_hash: String::new(),
            source_public_values_hash: String::new(),
            source_proof_bytes: Vec::new(),
            source_public_values: Vec::new(),
            expires_at_height: finalized.0 + 1_000,
        }),
        "propose-controlled-pfusdc-vault-deposit",
    );
    let pfusdc_bridge_observation =
        VaultBridgeDepositObservation::success_for_evidence(&pfusdc_bridge_evidence, 12);
    let pfusdc_bridge_observation_root =
        vault_bridge_deposit_observation_root(&pfusdc_bridge_observation)
            .expect("pfUSDC controlled observation root");
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::VaultBridgeDepositAttest(VaultBridgeDepositAttestOperation {
            attestor: holder.address.clone(),
            asset_id: PFUSDC_ASSET_ID.to_string(),
            evidence_root: pfusdc_bridge_evidence_root.clone(),
            pass: true,
            observation_root: pfusdc_bridge_observation_root,
            observation: Some(pfusdc_bridge_observation),
        }),
        "attest-controlled-pfusdc-vault-deposit",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            VaultBridgeDepositFinalizeOperation {
                finalizer: holder.address.clone(),
                asset_id: PFUSDC_ASSET_ID.to_string(),
                evidence_root: pfusdc_bridge_evidence_root.clone(),
            },
        ),
        "finalize-controlled-pfusdc-vault-deposit",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::VaultBridgeDepositClaim(VaultBridgeDepositClaimOperation {
            claimer: holder.address.clone(),
            asset_id: PFUSDC_ASSET_ID.to_string(),
            evidence_root: pfusdc_bridge_evidence_root,
            policy_hash: pfusdc_bridge_policy_hash,
            recipient: holder.address.clone(),
            amount_atoms: PFUSDC_SUPPLY,
        }),
        "claim-controlled-pfusdc-vault-deposit",
    );
    let pfusdc_backed_ledger: LedgerState = serde_json::from_slice(
        &fs::read(harness.node(0).join("ledger.json"))
            .expect("read controlled pfUSDC-backed ledger"),
    )
    .expect("parse controlled pfUSDC-backed ledger");
    let pfusdc_source_root = vault_bridge_source_root_for_asset(
        &pfusdc_backed_ledger.vault_bridge_bucket_states,
        PFUSDC_ASSET_ID,
    )
    .expect("derive controlled pfUSDC vault source root");
    let pfusdc_packet_hash = "f6".repeat(48);
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &pfusdc_issuer,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: pfusdc_issuer.address.clone(),
            submitter: pfusdc_issuer.address.clone(),
            asset_id: PFUSDC_ASSET_ID.to_string(),
            epoch: 1,
            nav_per_unit: 100_000_000,
            circulating_supply: PFUSDC_SUPPLY,
            verified_net_assets: 2_000_000_000,
            proof_profile: pfusdc_profile,
            source_root: pfusdc_source_root,
            attestor_root: "f8".repeat(48),
            reserve_packet_hash: pfusdc_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: Vec::new(),
            sp1_public_values: Vec::new(),
        }),
        "submit-pfusdc-accounting-packet",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &pfusdc_issuer,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: pfusdc_issuer.address.clone(),
            asset_id: PFUSDC_ASSET_ID.to_string(),
            epoch: 1,
            reserve_packet_hash: pfusdc_packet_hash,
        }),
        "finalize-pfusdc-accounting-packet",
    );

    let route_init_height = status_tuple(rpc_ports[0], "a666-route-init-parent").0 + 1;
    let mut policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 1,
        issue_multiplier_bps: 10_050,
        redeem_multiplier_bps: 9_995,
        issue_capacity_atoms: 100_000_000,
        redeem_capacity_atoms: 100_000_000,
        max_order_atoms: 10_000_000,
        min_order_atoms: 1,
        valid_from_height: route_init_height,
        expires_at_height: route_init_height + 5_000,
        max_nav_age_blocks: 5_000,
        pricing_nav_epoch: public_values.observation_epoch,
        pricing_reserve_packet_hash: reserve_packet_hash.clone(),
    };
    policy.policy_hash = policy.computed_hash();
    let verification_policy = EthereumRouteVerificationPolicyV1 {
        authority_epoch: committee.domain.committee_epoch,
        committee_root: committee.domain.committee_root,
        minimum_confirmations: 12,
        handoff_controller_code_hash: hex_to_bytes(
            "4c62b7d8b3a7928fd9667445f8fd68b3336ba0ec9a8f3e59b463b684fe6ceaaf",
        )
        .expect("decode deployed handoff controller code hash")
        .try_into()
        .expect("deployed handoff controller code hash length"),
        wrapped_navcoin_code_hash: hex_to_bytes(
            "671ee905050e2965995a8c6db8b05e4c2f30bd690eeff55093c03f9722be66b0",
        )
        .expect("decode deployed wrapped A666 code hash")
        .try_into()
        .expect("deployed wrapped A666 code hash length"),
    };
    let route_config_digest = "a1".repeat(48);
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapRouteInitV2(PftlUniswapRouteInitV2Operation {
            operator: A666_RESERVE_OPERATOR.to_string(),
            route_id: ROUTE_ID.to_string(),
            route_config_digest: route_config_digest.clone(),
            native_nav_asset_id: A666_ASSET_ID.to_string(),
            settlement_asset_id: PFUSDC_ASSET_ID.to_string(),
            opening_inventory_atoms: A666_CIRCULATING_SUPPLY,
            opening_inventory_holder: A666_RESERVE_OPERATOR.to_string(),
            handoff_controller: HANDOFF_CONTROLLER.to_string(),
            settlement_adapter: SETTLEMENT_ADAPTER.to_string(),
            wrapped_navcoin_token: WRAPPED_A666.to_string(),
            ethereum_chain_id: 1,
            route_supply_cap_atoms: 2_000_000_000_000,
            packet_notional_cap_atoms: 250_000_000_000,
            latest_finalized_nav_epoch: public_values.observation_epoch,
            return_finality_blocks: 12,
            route_epoch: 1,
            outbound_verification_class: PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY.to_string(),
            return_verification_class: PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.to_string(),
            live_value_enabled: false,
            ethereum_verification_policy: verification_policy.clone(),
            primary_market_policy: policy.clone(),
        }),
        "initialize-public-successor-a666-route",
    );
    policy.policy_epoch = 2;
    policy.valid_from_height = status_tuple(rpc_ports[0], "a666-route-activate-parent").0 + 1;
    policy.policy_hash = policy.computed_hash();
    finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(
            PftlUniswapRouteEpochAdvanceOperation {
                operator: A666_RESERVE_OPERATOR.to_string(),
                route_id: ROUTE_ID.to_string(),
                prior_route_epoch: 1,
                next_route_epoch: 2,
                next_route_config_digest: "a2".repeat(48),
                live_value_enabled: true,
                next_primary_market_policy: policy.clone(),
            },
        ),
        "activate-public-successor-a666-route",
    );

    let transparent_mint_atoms = 2_000_000;
    let transparent_base_atoms = postfiat_execution::required_vault_bridge_settlement_atoms(
        transparent_mint_atoms,
        6,
        nav_per_unit,
        A666_VALUATION_UNIT,
        PFUSDC_VALUATION_UNIT,
        6,
    )
    .expect("derive transparent A666 base settlement from registered valuation units");
    let transparent_settlement_atoms = checked_mul_div_ceil(
        transparent_base_atoms,
        u64::from(policy.issue_multiplier_bps),
        10_000,
    );
    let transparent_reservation_id = "b1".repeat(48);
    let transparent_expiry = finalized.0 + 200;
    let outage_proposer = command_json(&[
        "block-proposer",
        "--data-dir",
        harness.node(0).to_str().expect("node path UTF-8"),
        "--height",
        &(finalized.0 + 1).to_string(),
        "--view",
        "0",
    ])["proposer"]
        .as_str()
        .expect("partial-outage proposer")
        .strip_prefix("validator-")
        .expect("partial-outage proposer prefix")
        .parse::<usize>()
        .expect("partial-outage proposer index");
    let offline_validator = if outage_proposer == VALIDATORS - 1 {
        VALIDATORS - 2
    } else {
        VALIDATORS - 1
    };
    stop_validator_services(&mut harness, offline_validator);
    let online_ports = rpc_ports
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, port)| (index != offline_validator).then_some(port))
        .collect::<Vec<_>>();
    finalized = submit_dev_key_asset_finality_observed(
        &harness,
        &rpc_ports,
        &online_ports,
        &holder,
        AssetTransactionOperation::PftlUniswapOrderReserve(PftlUniswapOrderReserveOperation {
            subscriber: holder.address.clone(),
            route_id: ROUTE_ID.to_string(),
            reservation_id: transparent_reservation_id.clone(),
            ethereum_recipient: HOLDER_ETHEREUM.to_string(),
            route_epoch: 2,
            policy_epoch: policy.policy_epoch,
            policy_hash: policy.policy_hash.clone(),
            mint_amount_atoms: transparent_mint_atoms,
            max_settlement_value_atoms: transparent_settlement_atoms,
            expires_at_height: transparent_expiry,
        }),
        "reserve-transparent-a666-issue-with-one-validator-offline",
    );
    stop_services(&mut harness);
    for path in &ready {
        let _ = fs::remove_file(path);
    }
    ready = start_services(&mut harness, &topology_path, &rpc_ports);
    wait_exact_six(&rpc_ports, &finalized);
    let _transparent_issue_finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::PftlUniswapPrimarySubscribeV2(
            PftlUniswapPrimarySubscribeV2Operation {
                subscriber: holder.address.clone(),
                route_id: ROUTE_ID.to_string(),
                reservation_id: transparent_reservation_id.clone(),
                subscription_nonce: "b2".repeat(32),
                settlement_asset_id: PFUSDC_ASSET_ID.to_string(),
                settlement_value_atoms: transparent_settlement_atoms,
                pricing_nav_epoch: public_values.observation_epoch,
                pricing_reserve_packet_hash: reserve_packet_hash.clone(),
            },
        ),
        "execute-transparent-a666-issue",
    );

    let overlay_ledger: LedgerState = serde_json::from_slice(
        &fs::read(harness.node(0).join("ledger.json"))
            .expect("read controlled A666 overlay ledger"),
    )
    .expect("parse controlled A666 overlay ledger");
    let overlay = postfiat_execution::nav_subscription_reserve_overlay_for_asset(
        &overlay_ledger,
        A666_ASSET_ID,
    )
    .expect("derive controlled A666 reserve overlay")
    .expect("nonzero controlled A666 reserve overlay");
    assert_eq!(
        overlay_ledger
            .pftl_uniswap_route(ROUTE_ID)
            .expect("controlled A666 route after issue")
            .settlement_reserve_atoms,
        transparent_base_atoms
    );
    assert_eq!(
        overlay.value_nav_units,
        transparent_base_atoms * 100,
        "pfUSDC micro-units must convert exactly into A666 USD_1E8 units"
    );
    let overlay_verified_net_assets = next_public_values
        .verified_net_assets
        .checked_add(overlay.value_nav_units)
        .expect("controlled A666 overlay net assets");
    let overlay_circulating_supply = A666_CIRCULATING_SUPPLY
        .checked_add(transparent_mint_atoms)
        .expect("controlled A666 overlay supply");
    let overlay_nav_per_unit = (u128::from(overlay_verified_net_assets) * 1_000_000_u128
        / u128::from(overlay_circulating_supply)) as u64;
    let overlay_source_root =
        postfiat_execution::nav_reserve_subscription_composite_source_root_v1(
            &next_public_values,
            &overlay.source_root,
            overlay.value_nav_units,
        )
        .expect("derive controlled A666 composite source root")
        .0;
    let overlay_packet_hash = "a7".repeat(48);
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: A666_ISSUER.to_string(),
            submitter: A666_RESERVE_OPERATOR.to_string(),
            asset_id: A666_ASSET_ID.to_string(),
            epoch: next_public_values.observation_epoch,
            nav_per_unit: overlay_nav_per_unit,
            circulating_supply: overlay_circulating_supply,
            verified_net_assets: overlay_verified_net_assets,
            proof_profile: A666_SUCCESSOR_PROFILE.to_string(),
            source_root: overlay_source_root,
            attestor_root: next_public_values.valuation_trust_root.clone(),
            reserve_packet_hash: overlay_packet_hash.clone(),
            reserve_accounts: Vec::new(),
            sp1_proof_bytes: next_proof,
            sp1_public_values: next_public_values_bytes,
        }),
        "submit-overlay-aware-a666-public-reserve-proof",
    );
    finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &issuer,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: A666_ISSUER.to_string(),
            asset_id: A666_ASSET_ID.to_string(),
            epoch: next_public_values.observation_epoch,
            reserve_packet_hash: overlay_packet_hash.clone(),
        }),
        "finalize-overlay-aware-a666-public-reserve-proof",
    );
    policy.policy_epoch = 3;
    policy.valid_from_height = finalized.0 + 1;
    policy.pricing_nav_epoch = next_public_values.observation_epoch;
    policy.pricing_reserve_packet_hash = overlay_packet_hash.clone();
    policy.policy_hash = policy.computed_hash();
    let _overlay_route_finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(
            PftlUniswapRouteEpochAdvanceOperation {
                operator: A666_RESERVE_OPERATOR.to_string(),
                route_id: ROUTE_ID.to_string(),
                prior_route_epoch: 2,
                next_route_epoch: 3,
                next_route_config_digest: "a3".repeat(48),
                live_value_enabled: true,
                next_primary_market_policy: policy.clone(),
            },
        ),
        "bind-route-to-overlay-aware-a666-public-reserve-proof",
    );

    let private_mint_atoms = 1_000_000;
    let private_base_atoms = postfiat_execution::required_vault_bridge_settlement_atoms(
        private_mint_atoms,
        6,
        overlay_nav_per_unit,
        A666_VALUATION_UNIT,
        PFUSDC_VALUATION_UNIT,
        6,
    )
    .expect("derive private A666 base settlement from registered valuation units");
    let private_settlement_atoms = checked_mul_div_ceil(
        private_base_atoms,
        u64::from(policy.issue_multiplier_bps),
        10_000,
    );
    let private_redeem_atoms = checked_mul_div_floor(
        private_base_atoms,
        u64::from(policy.redeem_multiplier_bps),
        10_000,
    );
    let private_ingress_file = harness.root.join("a666-private-pfusdc-ingress.json");
    let private_pfusdc_note = harness.root.join("a666-private-pfusdc-note.json");
    create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
        data_dir: harness.node(0),
        key_file: holder_key_file.clone(),
        asset_id: PFUSDC_ASSET_ID.to_string(),
        amount: private_settlement_atoms,
        fee: 0,
        note_seed_hex: bytes_to_hex(&[0x41; 32]),
        encrypted_output_hex: None,
        ingress_file: private_ingress_file.clone(),
        note_file: private_pfusdc_note.clone(),
        overwrite: false,
    })
    .expect("create private pfUSDC ingress");
    let private_ingress_batch = harness.root.join("a666-private-pfusdc-ingress.batch.json");
    create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
        data_dir: harness.node(0),
        ingress_file: private_ingress_file,
        batch_file: private_ingress_batch.clone(),
    })
    .expect("build private pfUSDC ingress batch");
    stop_services(&mut harness);
    for path in &ready {
        let _ = fs::remove_file(path);
    }
    finalized = finalize_offline_batch_all_validators(
        &data_dirs,
        &private_ingress_batch,
        "shielded",
        "a666-private-pfusdc-ingress",
    );
    ready = start_services(&mut harness, &topology_path, &rpc_ports);
    wait_exact_six(&rpc_ports, &finalized);

    let export_packet_hash = "b7".repeat(48);
    let export_nonce = "b8".repeat(32);
    let active_route_config_digest = "a3".repeat(48);
    let export_digest = PftlUniswapMintPacketV2 {
        route_config_digest: active_route_config_digest.clone(),
        source_packet_hash: export_packet_hash.clone(),
        reservation_id: transparent_reservation_id.clone(),
        source_receipt_hash: "b9".repeat(48),
        source_receipt_root: "ba".repeat(48),
        settlement_asset_id: PFUSDC_ASSET_ID.to_string(),
        native_nav_asset_id: A666_ASSET_ID.to_string(),
        pricing_reserve_packet_hash: overlay_packet_hash.clone(),
        policy_hash_commitment: postfiat_types::pftl_uniswap_keccak_commitment48(
            "policy",
            &policy.policy_hash,
        )
        .expect("A666 policy commitment"),
        route_epoch: 3,
        pricing_nav_epoch: next_public_values.observation_epoch,
        deadline_seconds: 1_800,
        nonce: export_nonce.clone(),
        destination_chain_id: 1,
        destination_controller: HANDOFF_CONTROLLER.to_string(),
        wrapped_token: WRAPPED_A666.to_string(),
        ethereum_recipient: HOLDER_ETHEREUM.to_string(),
        mint_amount_atoms: transparent_mint_atoms,
        settlement_value_atoms: transparent_settlement_atoms,
    }
    .evm_digest()
    .expect("A666 export digest");
    let _export_finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::PftlUniswapExportDebit(PftlUniswapExportDebitOperation {
            owner: holder.address.clone(),
            route_id: ROUTE_ID.to_string(),
            packet_hash: export_packet_hash.clone(),
            export_nonce,
            ethereum_recipient: HOLDER_ETHEREUM.to_string(),
            amount_atoms: transparent_mint_atoms,
            reservation_id: Some(transparent_reservation_id),
            settlement_value_atoms: Some(transparent_settlement_atoms),
            destination_deadline_seconds: 1_800,
            refund_delay_blocks: 3,
            ethereum_packet_digest: Some(export_digest.clone()),
            ethereum_packet_schema_version: Some(PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V2),
        }),
        "export-transparent-a666-to-ethereum",
    );

    let controller: [u8; 20] = hex_to_bytes(&HANDOFF_CONTROLLER[2..])
        .expect("deployed handoff controller address hex")
        .try_into()
        .expect("deployed handoff controller address width");
    let wrapped: [u8; 20] = hex_to_bytes(&WRAPPED_A666[2..])
        .expect("deployed wrapped A666 address hex")
        .try_into()
        .expect("deployed wrapped A666 address width");
    let holder_ethereum: [u8; 20] = hex_to_bytes(&HOLDER_ETHEREUM[2..])
        .expect("deployed holder address hex")
        .try_into()
        .expect("deployed holder address width");
    let mut recipient_topic = [0_u8; 32];
    recipient_topic[12..].copy_from_slice(&holder_ethereum);
    let consumed_signature = postfiat_bridge::ethereum_keccak256(
        b"PacketConsumed(bytes32,bytes32,address,bytes32,bytes32,bytes32,uint256,uint256)",
    );
    let packet_digest_bytes: [u8; 32] = hex_to_bytes(&export_digest)
        .expect("export digest hex")
        .try_into()
        .expect("export digest width");
    let source_packet_commitment = postfiat_bridge::ethereum_keccak256(
        &hex_to_bytes(&export_packet_hash).expect("export packet hash"),
    );
    let route_config_commitment = postfiat_bridge::ethereum_keccak256(
        &hex_to_bytes(&active_route_config_digest).expect("active route config"),
    );
    let trust_class_commitment =
        postfiat_bridge::ethereum_keccak256(PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT.as_bytes());
    let mut consumed_data = route_config_commitment.to_vec();
    consumed_data.extend_from_slice(&[0x31; 32]);
    consumed_data.extend_from_slice(&trust_class_commitment);
    consumed_data.extend_from_slice(&ethereum_abi_u64(transparent_mint_atoms));
    consumed_data.extend_from_slice(&ethereum_abi_u64(transparent_settlement_atoms));
    let (consume_receipts_root, consume_receipt_proof) = ethereum_receipt_proof(
        controller,
        &[
            consumed_signature,
            packet_digest_bytes,
            source_packet_commitment,
            recipient_topic,
        ],
        &consumed_data,
    );
    let consume_height = 10_000;
    let consume_finalized_height = consume_height + 12;
    let consume_checkpoint = EthereumFinalizedCheckpointV1 {
        schema_version: ETHEREUM_CHECKPOINT_SCHEMA_V1,
        pftl_domain: committee.domain.chain.clone(),
        route_id: ROUTE_ID.to_string(),
        route_config_digest: FastSwapOpaqueHashV1(
            hex_to_bytes(&active_route_config_digest)
                .expect("route config hex")
                .try_into()
                .expect("route config width"),
        ),
        ethereum_chain_id: 1,
        block_number: consume_height,
        block_hash: [0x51; 32],
        receipts_root: consume_receipts_root,
        observed_head_number: consume_finalized_height,
        minimum_confirmations: 12,
        authority_epoch: committee.domain.committee_epoch,
        committee_root: committee.domain.committee_root,
        handoff_controller: controller,
        wrapped_navcoin_token: wrapped,
        handoff_controller_code_hash: verification_policy.handoff_controller_code_hash,
        wrapped_navcoin_code_hash: verification_policy.wrapped_navcoin_code_hash,
    };
    let _consume_finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapDestinationConsume(
            PftlUniswapDestinationConsumeOperation {
                operator: A666_RESERVE_OPERATOR.to_string(),
                route_id: ROUTE_ID.to_string(),
                packet_hash: export_packet_hash,
                ethereum_consume_tx_hash: "bb".repeat(32),
                consumed_height: consume_height,
                finalized_height: consume_finalized_height,
                external_event_proof: Some(EthereumExternalEventProofV1 {
                    checkpoint_certificate: ethereum_checkpoint_certificate(
                        &committee,
                        &harness.node(0),
                        consume_checkpoint.clone(),
                    ),
                    receipt_proof: consume_receipt_proof,
                    log_index: 0,
                }),
            },
        ),
        "consume-a666-export-on-ethereum",
    );

    let return_sender = holder_ethereum;
    let return_sender_text = HOLDER_ETHEREUM.to_string();
    let return_nonce = [0xbc; 32];
    let return_nonce_text = bytes_to_hex(&return_nonce);
    let return_burn_height = 10_100;
    let return_finalized_height = return_burn_height + 12;
    let return_burn_id = pftl_uniswap_return_burn_id_from_fields(
        1,
        HANDOFF_CONTROLLER,
        WRAPPED_A666,
        A666_ASSET_ID,
        &return_sender_text,
        &holder.address,
        transparent_mint_atoms,
        &return_nonce_text,
        return_burn_height,
    )
    .expect("canonical A666 return burn id");
    let return_burn_id_bytes: [u8; 32] = hex_to_bytes(&return_burn_id)
        .expect("return burn id hex")
        .try_into()
        .expect("return burn id width");
    let recipient_tail = ethereum_abi_dynamic(holder.address.as_bytes());
    let native_asset_tail =
        ethereum_abi_dynamic(&hex_to_bytes(A666_ASSET_ID).expect("A666 asset id for return event"));
    let mut return_data = ethereum_abi_u64(7 * 32).to_vec();
    return_data.extend_from_slice(&ethereum_abi_u64(
        u64::try_from(7 * 32 + recipient_tail.len()).expect("return asset ABI offset"),
    ));
    return_data.extend_from_slice(&ethereum_abi_u64(transparent_mint_atoms));
    return_data.extend_from_slice(&ethereum_abi_u64(1));
    return_data.extend_from_slice(&ethereum_abi_address(controller));
    return_data.extend_from_slice(&ethereum_abi_address(wrapped));
    return_data.extend_from_slice(&ethereum_abi_u64(return_burn_height));
    return_data.extend_from_slice(&recipient_tail);
    return_data.extend_from_slice(&native_asset_tail);
    let return_signature = postfiat_bridge::ethereum_keccak256(
        b"ReturnBurned(bytes32,address,bytes32,string,bytes,uint256,uint256,address,address,uint256)",
    );
    let (return_receipts_root, return_receipt_proof) = ethereum_receipt_proof(
        controller,
        &[
            return_signature,
            return_burn_id_bytes,
            ethereum_abi_address(return_sender),
            return_nonce,
        ],
        &return_data,
    );
    let mut return_checkpoint = consume_checkpoint;
    return_checkpoint.block_number = return_burn_height;
    return_checkpoint.block_hash = [0x63; 32];
    return_checkpoint.receipts_root = return_receipts_root;
    return_checkpoint.observed_head_number = return_finalized_height;
    finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapReturnImport(PftlUniswapReturnImportOperation {
            operator: A666_RESERVE_OPERATOR.to_string(),
            route_id: ROUTE_ID.to_string(),
            burn_event_hash: return_burn_id,
            ethereum_chain_id: 1,
            bridge_controller: HANDOFF_CONTROLLER.to_string(),
            wrapped_navcoin_token: WRAPPED_A666.to_string(),
            native_nav_asset_id: A666_ASSET_ID.to_string(),
            ethereum_sender: return_sender_text,
            pftl_recipient: holder.address.clone(),
            amount_atoms: transparent_mint_atoms,
            return_nonce: return_nonce_text,
            burn_height: return_burn_height,
            finalized_height: return_finalized_height,
            external_event_proof: Some(EthereumExternalEventProofV1 {
                checkpoint_certificate: ethereum_checkpoint_certificate(
                    &committee,
                    &harness.node(0),
                    return_checkpoint,
                ),
                receipt_proof: return_receipt_proof,
                log_index: 0,
            }),
        }),
        "return-wrapped-a666-to-pftl",
    );

    let transparent_redeem_atoms = 500_000;
    let transparent_redeem_base = postfiat_execution::required_vault_bridge_settlement_atoms(
        transparent_redeem_atoms,
        6,
        overlay_nav_per_unit,
        A666_VALUATION_UNIT,
        PFUSDC_VALUATION_UNIT,
        6,
    )
    .expect("derive transparent A666 redemption base from registered valuation units");
    let transparent_redeem_output = checked_mul_div_floor(
        transparent_redeem_base,
        u64::from(policy.redeem_multiplier_bps),
        10_000,
    );
    finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::PftlUniswapPrimaryRedeem(PftlUniswapPrimaryRedeemOperation {
            owner: holder.address.clone(),
            settlement_recipient: holder.address.clone(),
            route_id: ROUTE_ID.to_string(),
            redemption_nonce: "bd".repeat(32),
            nav_amount_atoms: transparent_redeem_atoms,
            min_settlement_value_atoms: transparent_redeem_output,
            route_epoch: 3,
            policy_epoch: policy.policy_epoch,
            policy_hash: policy.policy_hash.clone(),
            pricing_nav_epoch: next_public_values.observation_epoch,
            pricing_reserve_packet_hash: overlay_packet_hash.clone(),
            expires_at_height: finalized.0 + 100,
        }),
        "redeem-transparent-a666",
    );
    let private_issue_action = harness.root.join("a666-private-primary-issue.json");
    let private_a666_note = harness.root.join("a666-private-a666-note.json");
    let private_reservation_id = "b3".repeat(48);
    create_asset_orchard_private_primary_issue(AssetOrchardPrivatePrimaryIssueCreateOptions {
        data_dir: harness.node(0),
        note_file: private_pfusdc_note,
        output_note_seed_hex: bytes_to_hex(&[0x42; 32]),
        output_note_file: private_a666_note.clone(),
        route_id: ROUTE_ID.to_string(),
        subscriber: holder.address.clone(),
        ethereum_recipient: HOLDER_ETHEREUM.to_string(),
        reservation_id: private_reservation_id.clone(),
        subscription_nonce: "b4".repeat(32),
        mint_amount_atoms: private_mint_atoms,
        settlement_value_atoms: private_settlement_atoms,
        expires_at_height: finalized.0 + 100,
        pending_output_commitments: Vec::new(),
        action_file: private_issue_action.clone(),
        overwrite: false,
    })
    .expect("create private A666 primary issue");
    let private_issue_batch = harness.root.join("a666-private-primary-issue.batch.json");
    create_asset_orchard_private_primary_issue_batch(AssetOrchardPrivatePrimaryIssueBatchOptions {
        data_dir: harness.node(0),
        action_file: private_issue_action,
        batch_file: private_issue_batch.clone(),
    })
    .expect("build private A666 primary issue batch");
    stop_services(&mut harness);
    for path in &ready {
        let _ = fs::remove_file(path);
    }
    finalized = finalize_offline_batch_all_validators(
        &data_dirs,
        &private_issue_batch,
        "shielded",
        "a666-private-primary-issue",
    );
    ready = start_services(&mut harness, &topology_path, &rpc_ports);
    wait_exact_six(&rpc_ports, &finalized);

    let private_redeem_action = harness.root.join("a666-private-primary-redeem.json");
    let private_redeem_note = harness.root.join("a666-private-redeem-pfusdc-note.json");
    create_asset_orchard_private_primary_redeem(AssetOrchardPrivatePrimaryRedeemCreateOptions {
        data_dir: harness.node(0),
        note_file: private_a666_note,
        output_note_seed_hex: bytes_to_hex(&[0x43; 32]),
        output_note_file: private_redeem_note,
        route_id: ROUTE_ID.to_string(),
        owner: holder.address.clone(),
        settlement_recipient: holder.address.clone(),
        redemption_id: "b5".repeat(48),
        redemption_nonce: "b6".repeat(32),
        nav_amount_atoms: private_mint_atoms,
        settlement_output_atoms: private_redeem_atoms,
        expires_at_height: finalized.0 + 100,
        pending_output_commitments: Vec::new(),
        action_file: private_redeem_action.clone(),
        overwrite: false,
    })
    .expect("create private A666 primary redeem");
    let private_redeem_batch = harness.root.join("a666-private-primary-redeem.batch.json");
    create_asset_orchard_private_primary_redeem_batch(
        AssetOrchardPrivatePrimaryRedeemBatchOptions {
            data_dir: harness.node(0),
            action_file: private_redeem_action,
            batch_file: private_redeem_batch.clone(),
        },
    )
    .expect("build private A666 primary redeem batch");
    stop_services(&mut harness);
    for path in &ready {
        let _ = fs::remove_file(path);
    }
    finalized = finalize_offline_batch_all_validators(
        &data_dirs,
        &private_redeem_batch,
        "shielded",
        "a666-private-primary-redeem",
    );
    let replay_error = simulate_shielded_batch(ShieldedBatchSimulateOptions {
        data_dir: harness.node(0),
        batch_file: private_redeem_batch,
    })
    .expect_err("finalized private redemption batch replay must reject");
    assert!(
        replay_error.to_string().contains("already applied"),
        "unexpected private redemption replay error: {replay_error}"
    );
    let _private_redeem_ready = start_services(&mut harness, &topology_path, &rpc_ports);
    wait_exact_six(&rpc_ports, &finalized);
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &holder,
        AssetTransactionOperation::PftlUniswapOrderRelease(PftlUniswapOrderReleaseOperation {
            releaser: holder.address.clone(),
            route_id: ROUTE_ID.to_string(),
            reservation_id: private_reservation_id,
        }),
        "release-private-a666-export-entitlement",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapRoutePause(PftlUniswapRoutePauseOperation {
            operator: A666_RESERVE_OPERATOR.to_string(),
            route_id: ROUTE_ID.to_string(),
            paused: true,
        }),
        "pause-public-successor-a666-route",
    );
    submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapRoutePause(PftlUniswapRoutePauseOperation {
            operator: A666_RESERVE_OPERATOR.to_string(),
            route_id: ROUTE_ID.to_string(),
            paused: false,
        }),
        "resume-public-successor-a666-route",
    );
    policy.policy_epoch = 4;
    policy.valid_from_height = status_tuple(rpc_ports[0], "a666-rollback-parent").0 + 1;
    policy.policy_hash = policy.computed_hash();
    finalized = submit_dev_key_asset_finality(
        &harness,
        &rpc_ports,
        &reserve_operator,
        AssetTransactionOperation::PftlUniswapRouteEpochAdvance(
            PftlUniswapRouteEpochAdvanceOperation {
                operator: A666_RESERVE_OPERATOR.to_string(),
                route_id: ROUTE_ID.to_string(),
                prior_route_epoch: 3,
                next_route_epoch: 4,
                next_route_config_digest: "a4".repeat(48),
                live_value_enabled: false,
                next_primary_market_policy: policy,
            },
        ),
        "rollback-public-successor-a666-route",
    );
    wait_exact_six(&rpc_ports, &finalized);
    let expected_a666_supply = A666_CIRCULATING_SUPPLY
        .checked_add(transparent_mint_atoms)
        .and_then(|supply| supply.checked_sub(transparent_redeem_atoms))
        .expect("expected A666 lifecycle supply");
    let expected_settlement_reserve = transparent_base_atoms
        .checked_sub(transparent_redeem_base)
        .expect("expected A666 settlement reserve");
    for index in 0..VALIDATORS {
        let ledger: LedgerState = serde_json::from_slice(
            &fs::read(harness.node(index).join("ledger.json"))
                .expect("read finalized A666 lifecycle ledger"),
        )
        .expect("parse finalized A666 lifecycle ledger");
        assert_eq!(
            postfiat_execution::issued_asset_supply(&ledger, A666_ASSET_ID)
                .expect("final A666 issued supply"),
            expected_a666_supply,
            "validator {index} A666 supply conservation"
        );
        assert_eq!(
            postfiat_execution::issued_asset_supply(&ledger, PFUSDC_ASSET_ID)
                .expect("final pfUSDC issued supply"),
            PFUSDC_SUPPLY,
            "validator {index} pfUSDC supply conservation"
        );
        let nav = ledger
            .nav_asset(A666_ASSET_ID)
            .expect("final A666 NAV state");
        assert_eq!(nav.circulating_supply, expected_a666_supply);
        assert_eq!(nav.proof_profile, A666_SUCCESSOR_PROFILE);
        let route = ledger
            .pftl_uniswap_route(ROUTE_ID)
            .expect("final A666 route state");
        let v2 = route.v2.as_ref().expect("final A666 v2 route");
        assert_eq!(v2.route_epoch, 4);
        assert_eq!(v2.primary_market_policy.policy_epoch, 4);
        assert!(!route.live_value_enabled, "validator {index}");
        assert!(!route.paused, "validator {index}");
        assert_eq!(route.authorized_valid_supply_atoms, expected_a666_supply);
        assert_eq!(route.pftl_spendable_supply_atoms, expected_a666_supply);
        assert_eq!(route.ethereum_spendable_supply_atoms, 0);
        assert_eq!(route.outstanding_bridge_claims_atoms, 0);
        assert_eq!(route.settlement_reserve_atoms, expected_settlement_reserve);
        assert!(v2.active_reservations.is_empty(), "validator {index}");
        assert!(v2.export_entitlements.is_empty(), "validator {index}");
    }
    for (index, port) in rpc_ports.iter().enumerate() {
        let verified = rpc_call(
            *port,
            &verify_state_request(format!("a666-successor-restart-{index}")),
        );
        assert_eq!(
            verified.result.expect("A666 verify_state result")["verified"],
            true,
            "validator {index} A666 restart verification"
        );
    }

    for child in &mut harness.children {
        let _ = child.kill();
        let _ = child.wait();
    }
    harness.children.clear();
    let snapshot_dir = harness.root.join("a666-public-successor.snapshot");
    let restored_dir = harness.root.join("a666-public-successor-restored");
    let snapshot = export_snapshot(SnapshotExportOptions {
        data_dir: harness.node(0),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export finalized A666 successor snapshot");
    assert_eq!(snapshot.block_height, finalized.0);
    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir,
        node_id: Some("a666-public-successor-restored".to_string()),
    })
    .expect("restore finalized A666 successor snapshot");
    assert_eq!(restored.block_height, finalized.0);
    assert_eq!(restored.block_tip_hash, finalized.1);
    assert_eq!(restored.state_root, finalized.2);
    verify_blocks(NodeOptions {
        data_dir: restored_dir.clone(),
    })
    .expect("replay finalized A666 successor snapshot history");
    let restored_ledger: LedgerState = serde_json::from_slice(
        &fs::read(restored_dir.join("ledger.json")).expect("read restored A666 ledger"),
    )
    .expect("parse restored A666 ledger");
    assert_eq!(
        restored_ledger
            .nav_proof_profile(A666_SUCCESSOR_PROFILE)
            .expect("restored A666 successor profile")
            .source_manifest_hash,
        public_values.source_manifest_hash
    );
}

#[test]
#[ignore = "mandatory local six-validator TCP smoke; run with --ignored --nocapture"]
fn atomic_swap_local_six_validator_tcp_finality_and_catch_up() {
    let mut harness = Harness::new();
    let seed_dir = harness.root.join("seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: VALIDATORS as u32,
    })
    .expect("initialize deterministic six-validator seed");
    activate_atomic_swaps_in_fresh_genesis(&seed_dir);

    let pfusdc_owner = backup(0x11);
    let a651_owner = backup(0x22);
    let pfusdc_issuer = backup(0x33);
    let a651_issuer = backup(0x44);
    let unrelated = backup(0x55);
    let pfusdc_owner_id =
        wallet_identity_from_backup(&pfusdc_owner).expect("pfUSDC owner identity");
    let a651_owner_id = wallet_identity_from_backup(&a651_owner).expect("a651 owner identity");
    let pfusdc_issuer_id =
        wallet_identity_from_backup(&pfusdc_issuer).expect("pfUSDC issuer identity");
    let a651_issuer_id = wallet_identity_from_backup(&a651_issuer).expect("a651 issuer identity");
    let unrelated_id = wallet_identity_from_backup(&unrelated).expect("unrelated identity");
    for (address, name) in [
        (&pfusdc_owner_id.address, "fund-pfusdc-owner"),
        (&a651_owner_id.address, "fund-a651-owner"),
        (&pfusdc_issuer_id.address, "fund-pfusdc-issuer"),
        (&a651_issuer_id.address, "fund-a651-issuer"),
        (&unrelated_id.address, "fund-unrelated"),
    ] {
        fund_wallet(&seed_dir, address, name);
    }

    let pfusdc_asset_id = issued_asset_id(CHAIN_ID, &pfusdc_issuer_id.address, "pfUSDC", 1)
        .expect("derive bridge-accounted pfUSDC asset id");
    let a651_asset_id = issued_asset_id(CHAIN_ID, &a651_issuer_id.address, "a651", 1)
        .expect("derive price-NAV a651 asset id");
    for (issuer, identity, code, display_name, name) in [
        (
            &pfusdc_issuer,
            &pfusdc_issuer_id,
            "pfUSDC",
            "Bridge-backed pfUSDC",
            "create-pfusdc",
        ),
        (
            &a651_issuer,
            &a651_issuer_id,
            "a651",
            "a651 NAV asset",
            "create-a651",
        ),
    ] {
        apply_asset_operation(
            &seed_dir,
            issuer,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: identity.address.clone(),
                code: code.to_string(),
                version: 1,
                precision: 6,
                display_name: display_name.to_string(),
                max_supply: Some(10_000_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
            name,
        );
    }
    for (issuer, identity, owner, asset, name) in [
        (
            &pfusdc_issuer,
            &pfusdc_issuer_id,
            &pfusdc_owner_id,
            &pfusdc_asset_id,
            "issue-pfusdc-owner",
        ),
        (
            &a651_issuer,
            &a651_issuer_id,
            &a651_owner_id,
            &a651_asset_id,
            "issue-a651-owner",
        ),
    ] {
        apply_asset_operation(
            &seed_dir,
            issuer,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: identity.address.clone(),
                to: owner.address.clone(),
                issuer: identity.address.clone(),
                asset_id: asset.clone(),
                amount: 500_000,
            }),
            name,
        );
    }

    // The classification regression needs the production two-row NAV shape,
    // but no external proof system. Placeholder profiles keep this harness
    // hermetic while preserving pfUSDC reserve-accounting versus a651 pricing.
    let pfusdc_profile_id = register_placeholder_nav_profile(
        &seed_dir,
        &pfusdc_issuer,
        &pfusdc_issuer_id.address,
        "pfusdc-bridge-accounting",
        "register-pfusdc-profile",
    );
    let a651_profile_id = register_placeholder_nav_profile(
        &seed_dir,
        &a651_issuer,
        &a651_issuer_id.address,
        "a651-market-nav",
        "register-a651-profile",
    );
    let successor_profile_id =
        register_successor_nav_profile(&seed_dir, &a651_issuer, &a651_issuer_id.address);
    let pfusdc_reserve_packet_hash = "b0".repeat(48);
    finalize_nav_epoch(
        &seed_dir,
        &pfusdc_issuer,
        &pfusdc_issuer_id.address,
        &pfusdc_asset_id,
        &pfusdc_profile_id,
        100_000_000,
        100_000_000_000_000,
        &pfusdc_reserve_packet_hash,
        "pfusdc",
    );
    let a651_reserve_packet_hash = "a6".repeat(48);
    finalize_nav_epoch(
        &seed_dir,
        &a651_issuer,
        &a651_issuer_id.address,
        &a651_asset_id,
        &a651_profile_id,
        500_000_000,
        500_000_000_000_000,
        &a651_reserve_packet_hash,
        "a651",
    );
    apply_asset_operation(
        &seed_dir,
        &a651_issuer,
        AssetTransactionOperation::MarketOpsPolicyRegister(MarketOpsPolicyRegisterOperation {
            issuer: a651_issuer_id.address.clone(),
            asset_id: a651_asset_id.clone(),
            policy: market_ops_policy(),
        }),
        "register-a651-market-ops-policy",
    );
    let a651_market_ops = a651_market_ops_operation(
        &a651_issuer_id.address,
        &a651_asset_id,
        &a651_reserve_packet_hash,
    );
    let a651_market_envelope_hash = a651_market_ops.envelope_hash.clone();
    apply_asset_operation(
        &seed_dir,
        &a651_issuer,
        AssetTransactionOperation::MarketOpsFinalize(a651_market_ops),
        "finalize-a651-market-ops",
    );

    let seeded_ledger: LedgerState = serde_json::from_slice(
        &fs::read(seed_dir.join("ledger.json")).expect("read real-pair seed ledger"),
    )
    .expect("parse real-pair seed ledger");
    assert!(
        seeded_ledger.nav_asset(&pfusdc_asset_id).is_some(),
        "pfUSDC must remain NAV-tracked for bridge reserve accounting"
    );
    assert!(
        seeded_ledger.nav_asset(&a651_asset_id).is_some(),
        "a651 must remain NAV-tracked"
    );
    let successor_profile = seeded_ledger
        .nav_proof_profile(&successor_profile_id)
        .expect("provider-neutral successor profile must survive seed replay");
    assert_eq!(
        successor_profile.verifier_kind,
        NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1
    );
    assert_eq!(
        successor_profile.public_values_schema,
        NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1
    );
    assert_eq!(
        successor_profile.source_manifest_hash,
        "9da4e2ba55939f138475026946d2728d9b40d3f4c7762289a70aae94584eac924b9a788c6df25c9276cc83f1616ef0e5"
    );
    assert_eq!(successor_profile.valuation_unit_id, "05".repeat(48));
    assert_eq!(successor_profile.max_observation_span_blocks, 8);
    assert!(successor_profile.allow_controlled_sources);
    assert_eq!(
        seeded_ledger.market_ops_envelopes.len(),
        1,
        "only a651 may be classified as the price-NAV leg"
    );
    assert_eq!(
        seeded_ledger.market_ops_envelopes[0].asset_id,
        a651_asset_id
    );
    assert_eq!(
        seeded_ledger.market_ops_envelopes[0].envelope_hash,
        a651_market_envelope_hash
    );
    assert!(
        seeded_ledger
            .trustline_for_account_asset(&pfusdc_owner_id.address, &a651_asset_id)
            .is_none(),
        "pfUSDC owner must not pre-create an a651 balance row"
    );
    assert!(
        seeded_ledger
            .trustline_for_account_asset(&a651_owner_id.address, &pfusdc_asset_id)
            .is_none(),
        "a651 owner must not pre-create a pfUSDC balance row"
    );

    let baseline = postfiat_node::status(NodeOptions {
        data_dir: seed_dir.clone(),
    })
    .expect("seed status");
    for index in 0..VALIDATORS {
        copy_dir(&seed_dir, &harness.node(index));
        rewrite_node_identity(&harness.node(index), &format!("validator-{index}"));
    }

    let base_port = free_base_port();
    let topology_path = harness.root.join("topology.json");
    let activated_genesis: Genesis = serde_json::from_slice(
        &fs::read(seed_dir.join("genesis.json")).expect("read activated seed genesis"),
    )
    .expect("parse activated seed genesis");
    let topology = local_topology(
        NetworkDomain {
            chain_id: activated_genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&activated_genesis),
            protocol_version: activated_genesis.protocol_version,
        },
        VALIDATORS as u32,
        base_port,
    )
    .expect("build exact activated six-validator topology");
    fs::write(
        &topology_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&topology).expect("serialize topology")
        ),
    )
    .expect("write exact activated topology");
    let rpc_ports = topology
        .peers
        .iter()
        .map(|peer| peer.rpc_port)
        .collect::<Vec<_>>();
    let ready = spawn_services(&mut harness, &topology_path, &rpc_ports);
    for path in &ready {
        wait_for_file(path, Duration::from_secs(90));
    }

    let next_height = baseline.block_height + 1;
    let proposer_json = command_json(&[
        "block-proposer",
        "--data-dir",
        harness.node(0).to_str().expect("node path UTF-8"),
        "--height",
        &next_height.to_string(),
        "--view",
        "0",
    ]);
    let proposer = proposer_json["proposer"].as_str().expect("proposer id");
    let proposer_index = proposer
        .strip_prefix("validator-")
        .expect("validator proposer prefix")
        .parse::<usize>()
        .expect("validator proposer index");
    let proposer_port = rpc_ports[proposer_index];

    let unrelated_quote = rpc_call(
        proposer_port,
        &transfer_fee_quote_request(
            "unrelated-quote",
            unrelated_id.address.clone(),
            pfusdc_issuer_id.address.clone(),
            1,
            None,
        ),
    );
    let unrelated_quote =
        decode_transfer_fee_quote_summary(&unrelated_quote).expect("decode unrelated quote");
    let unrelated_signed =
        wallet_sign_transfer_from_quote(&unrelated, &unrelated_quote).expect("sign unrelated");
    rpc_call(
        proposer_port,
        &mempool_submit_signed_transfer_json_request(
            "unrelated-submit",
            serde_json::to_string(&unrelated_signed).expect("serialize unrelated transfer"),
        ),
    );

    let (quote_request, leg_0_wallet, leg_1_wallet) = if pfusdc_asset_id < a651_asset_id {
        (
            atomic_swap_fee_quote_request(
                "atomic-quote",
                "a1".repeat(48),
                a651_market_envelope_hash.clone(),
                1,
                next_height + 100,
                "c3".repeat(48),
                pfusdc_owner_id.address.clone(),
                a651_owner_id.address.clone(),
                pfusdc_issuer_id.address.clone(),
                pfusdc_asset_id.clone(),
                20_000,
                a651_owner_id.address.clone(),
                pfusdc_owner_id.address.clone(),
                a651_issuer_id.address.clone(),
                a651_asset_id.clone(),
                30_000,
            ),
            &pfusdc_owner,
            &a651_owner,
        )
    } else {
        (
            atomic_swap_fee_quote_request(
                "atomic-quote",
                "a1".repeat(48),
                a651_market_envelope_hash.clone(),
                1,
                next_height + 100,
                "c3".repeat(48),
                a651_owner_id.address.clone(),
                pfusdc_owner_id.address.clone(),
                a651_issuer_id.address.clone(),
                a651_asset_id.clone(),
                30_000,
                pfusdc_owner_id.address.clone(),
                a651_owner_id.address.clone(),
                pfusdc_issuer_id.address.clone(),
                pfusdc_asset_id.clone(),
                20_000,
            ),
            &a651_owner,
            &pfusdc_owner,
        )
    };
    let quote_response = rpc_call(proposer_port, &quote_request);
    let quote = decode_atomic_swap_fee_quote_summary(&quote_response, &quote_request)
        .expect("decode atomic quote");
    assert_eq!(quote.parent_height, baseline.block_height);
    assert_eq!(quote.unsigned_transaction.nav_epoch, 1);
    assert_eq!(
        quote.unsigned_transaction.market_envelope_hash,
        a651_market_envelope_hash
    );
    assert_eq!(
        [
            quote.unsigned_transaction.leg_0.asset_id.as_str(),
            quote.unsigned_transaction.leg_1.asset_id.as_str(),
        ]
        .into_iter()
        .filter(|asset_id| **asset_id == a651_asset_id)
        .count(),
        1,
        "the real pair must contain exactly one finalized price-NAV leg"
    );
    let signed =
        wallet_sign_atomic_swap_from_quote(leg_0_wallet, leg_1_wallet, &quote_request, &quote)
            .expect("dual-sign exact pfUSDC/a651 quote");
    let serialized = serde_json::to_string(&signed).expect("serialize signed atomic swap");
    for forbidden in ["trust_set", "trustline", "line_create"] {
        assert!(
            !serialized.contains(forbidden),
            "found forbidden {forbidden}"
        );
    }
    let finality_request =
        mempool_submit_signed_atomic_swap_transaction_finality_from_quote_request(
            "atomic-finality",
            serialized,
            &quote,
            Some(90_000),
        );
    let finality_response = rpc_call(proposer_port, &finality_request);
    let finality = decode_atomic_swap_finality_summary(&finality_response, &finality_request)
        .expect("decode atomic finality");
    assert!(finality.accepted);
    assert_eq!(finality.block_height, next_height);
    assert_eq!(finality.validator_count, VALIDATORS as u64);
    assert_eq!(finality.vote_count, VALIDATORS as u64);

    let expected = (
        finality.block_height,
        finality.block_hash.clone(),
        finality.state_root.clone(),
    );
    wait_exact_six(&rpc_ports, &expected);

    let unknown_tx_id = "fd".repeat(48);
    assert_ne!(unknown_tx_id, finality.tx_id);
    for (index, port) in rpc_ports.iter().enumerate() {
        let unknown_tx = rpc_call_raw(
            *port,
            &tx_request(format!("unknown-tx-{index}"), unknown_tx_id.clone()),
        );
        assert!(!unknown_tx.ok, "validator {index} found an unknown tx");
        assert!(unknown_tx.result.is_none());
        let error = unknown_tx.error.expect("typed unknown-tx RPC error");
        assert_eq!(
            error.code, "rpc_tx_not_found",
            "validator {index} returned the wrong unknown-tx error: {}",
            error.message
        );

        let unknown_receipts = rpc_call(
            *port,
            &receipts_request(
                format!("unknown-receipts-{index}"),
                Some(&unknown_tx_id),
                Some(16),
            ),
        );
        assert_eq!(
            unknown_receipts.result,
            Some(json!([])),
            "validator {index} returned receipts for an unknown valid tx id"
        );
    }

    let mut completed_jobs = 0usize;
    for index in 0..VALIDATORS {
        let data_dir = harness.node(index);
        assert!(
            !data_dir.join("ordered_commit_journal.json").exists(),
            "validator {index} retained an ordered-commit journal after convergence"
        );
        let mempool: MempoolState = serde_json::from_slice(
            &fs::read(data_dir.join("mempool.json")).expect("read converged mempool"),
        )
        .expect("parse converged mempool");
        assert!(
            mempool
                .pending_atomic_swaps
                .iter()
                .all(|entry| entry.tx_id != finality.tx_id),
            "validator {index} retained the finalized atomic swap in its mempool"
        );
        let ledger: LedgerState = serde_json::from_slice(
            &fs::read(data_dir.join("ledger.json")).expect("read converged real-pair ledger"),
        )
        .expect("parse converged real-pair ledger");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&pfusdc_owner_id.address, &pfusdc_asset_id)
                .expect("pfUSDC owner source balance")
                .balance,
            480_000,
            "validator {index} pfUSDC debit"
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&a651_owner_id.address, &a651_asset_id)
                .expect("a651 owner source balance")
                .balance,
            470_000,
            "validator {index} a651 debit"
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&pfusdc_owner_id.address, &a651_asset_id)
                .expect("auto-created a651 recipient balance")
                .balance,
            30_000,
            "validator {index} a651 credit"
        );
        assert_eq!(
            ledger
                .trustline_for_account_asset(&a651_owner_id.address, &pfusdc_asset_id)
                .expect("auto-created pfUSDC recipient balance")
                .balance,
            20_000,
            "validator {index} pfUSDC credit"
        );

        let outbox_report = resume_outbox(&data_dir, &topology_path);
        assert_eq!(
            outbox_report["schema"],
            "postfiat-certified-send-outbox-resume-v1"
        );
        assert_eq!(outbox_report["discovered"], 0, "validator {index}");
        assert_eq!(outbox_report["attempted"], 0, "validator {index}");
        assert_eq!(outbox_report["pending"], 0, "validator {index}");
        assert_eq!(outbox_report["quarantined"], 0, "validator {index}");
        assert_eq!(outbox_report["all_completed"], true, "validator {index}");

        let completed_outbox = data_dir.join("certified-send-outbox/completed");
        if completed_outbox.is_dir() {
            completed_jobs += fs::read_dir(&completed_outbox)
                .expect("read completed certified-send outbox")
                .filter_map(Result::ok)
                .filter(|entry| entry.path().join("job.json").is_file())
                .count();
        }
    }
    assert_eq!(
        completed_jobs,
        VALIDATORS - 1,
        "completed delivery tombstones are allowed, but exactly one per remote validator is expected"
    );

    for (index, port) in rpc_ports.iter().enumerate() {
        if status_tuple(*port, &format!("post-outbox-{index}")) != expected {
            command_output(&[
                "rpc-catch-up-certified-delta",
                "--data-dir",
                harness.node(index).to_str().expect("laggard path UTF-8"),
                "--source-host",
                "127.0.0.1",
                "--source-rpc-port",
                &proposer_port.to_string(),
                "--expected-height",
                &expected.0.to_string(),
                "--expected-block-hash",
                &expected.1,
                "--expected-state-root",
                &expected.2,
                "--work-dir",
                harness
                    .root
                    .join(format!("validator-{index}.adaptive-delta"))
                    .to_str()
                    .expect("adaptive work dir UTF-8"),
                "--timeout-ms",
                "90000",
            ]);
        }
    }
    wait_exact_six(&rpc_ports, &expected);

    for (index, port) in rpc_ports.iter().enumerate() {
        let verified = rpc_call(*port, &verify_state_request(format!("verify-{index}")));
        assert_eq!(
            verified.result.expect("verify_state result")["verified"],
            true,
            "validator {index} state verification failed"
        );
    }

    let full_recovery = harness.root.join("full-recovery");
    let delta_recovery = harness.root.join("delta-recovery");
    copy_dir(&seed_dir, &full_recovery);
    copy_dir(&seed_dir, &delta_recovery);
    rewrite_node_identity(&full_recovery, "recovery-full");
    rewrite_node_identity(&delta_recovery, "recovery-delta");
    command_output(&[
        "rpc-catch-up",
        "--data-dir",
        full_recovery.to_str().expect("full recovery UTF-8"),
        "--source-host",
        "127.0.0.1",
        "--source-rpc-port",
        &proposer_port.to_string(),
        "--work-dir",
        harness
            .root
            .join("full-catch-up-work")
            .to_str()
            .expect("full work dir UTF-8"),
        "--timeout-ms",
        "90000",
    ]);
    command_output(&[
        "rpc-catch-up-certified-delta",
        "--data-dir",
        delta_recovery.to_str().expect("delta recovery UTF-8"),
        "--source-host",
        "127.0.0.1",
        "--source-rpc-port",
        &proposer_port.to_string(),
        "--expected-height",
        &expected.0.to_string(),
        "--expected-block-hash",
        &expected.1,
        "--expected-state-root",
        &expected.2,
        "--work-dir",
        harness
            .root
            .join("delta-catch-up-work")
            .to_str()
            .expect("delta work dir UTF-8"),
        "--timeout-ms",
        "90000",
    ]);
    for (name, data_dir) in [("full", &full_recovery), ("delta", &delta_recovery)] {
        let status = command_json(&[
            "status",
            "--data-dir",
            data_dir.to_str().expect("recovery data dir UTF-8"),
        ]);
        assert_eq!(status["block_height"], expected.0, "{name} catch-up height");
        assert_eq!(status["block_tip_hash"], expected.1, "{name} catch-up tip");
        assert_eq!(status["state_root"], expected.2, "{name} catch-up root");
        let verify = command_json(&[
            "verify-state",
            "--data-dir",
            data_dir.to_str().expect("recovery data dir UTF-8"),
        ]);
        assert_eq!(verify["verified"], true, "{name} terminal verify_state");
    }

    eprintln!(
        "atomic swap local six passed: height={} tx={} root={} proposer={}",
        finality.block_height, finality.tx_id, finality.state_root, proposer
    );
}
