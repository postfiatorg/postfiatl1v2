use std::{fs, path::PathBuf};

use alloy::{
    primitives::{keccak256, Address, Bytes, FixedBytes, B256, U256},
    rpc::types::{Block, EIP1186AccountProofResponse},
};
use alloy_rlp::Encodable;
use alloy_sol_types::{sol, SolCall, SolEvent, SolValue};
use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use helios_consensus_core::{
    apply_bootstrap, apply_finality_update, apply_update, calc_sync_period,
    consensus_spec::MainnetConsensusSpec,
    types::{Bootstrap, FinalityUpdate, Fork, Forks, LightClientStore, Update},
    verify_bootstrap, verify_finality_update, verify_update,
};
use pfusdc_ingress_program::{
    ingress_policy_hash_v2, verify_ingress_witness_v2, NitroAssertionWitnessV1, NitroSendWitnessV1,
    PfUsdcIngressProofPolicyV2, PfUsdcIngressProofWitnessV2,
    PFUSDC_INGRESS_PROOF_WITNESS_SCHEMA_V2,
};
use postfiat_types::{
    EthereumArbitrumCheckpointV1, EthereumArbitrumFinalityStateV2, VaultBridgeDepositEvidence,
    VaultBridgeRouteProfileV1, ETHEREUM_ARBITRUM_FINALITY_STATE_SCHEMA_V2,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use sp1_helios_primitives::types::{ContractStorage, ProofInputs, StorageSlotWithProof};
use sp1_helios_primitives::verify_storage_slot_proofs;
use tree_hash::TreeHash;

const CHECKPOINTS_BEHIND: u64 = 16;
const MAX_HELIOS_UPDATES: u8 = 8;
const ARBSYS_ADDRESS: Address = Address::new([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x64,
]);
const NODE_INTERFACE_ADDRESS: Address = Address::new([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xc8,
]);

sol! {
    struct CaptureGlobalState {
        bytes32[2] bytes32Vals;
        uint64[2] u64Vals;
    }

    struct CaptureAssertionState {
        CaptureGlobalState globalState;
        uint8 machineStatus;
        bytes32 endHistoryRoot;
    }

    struct CaptureConfigData {
        bytes32 wasmModuleRoot;
        uint256 requiredStake;
        address challengeManager;
        uint64 confirmPeriodBlocks;
        uint64 nextInboxPosition;
    }

    struct CaptureBeforeStateData {
        bytes32 prevPrevAssertionHash;
        bytes32 sequencerBatchAcc;
        CaptureConfigData configData;
    }

    struct CaptureAssertionInputs {
        CaptureBeforeStateData beforeStateData;
        CaptureAssertionState beforeState;
        CaptureAssertionState afterState;
    }

    event AssertionCreated(
        bytes32 indexed assertionHash,
        bytes32 indexed parentAssertionHash,
        CaptureAssertionInputs assertion,
        bytes32 afterInboxBatchAcc,
        uint256 inboxMaxCount,
        bytes32 wasmModuleRoot,
        uint256 requiredStake,
        address challengeManager,
        uint64 confirmPeriodBlocks
    );

    event ERC20BridgeDepositedV2(
        bytes32 indexed depositId,
        address indexed depositor,
        bytes32 indexed pftlRecipientHash,
        string pftlRecipient,
        uint256 amount,
        bytes32 nonce,
        bytes32 routeBinding,
        uint256 sourceChainId,
        address vault,
        address token
    );

    event Tier4IngressCommitment(
        bytes32 indexed depositId,
        uint256 indexed outputIndex,
        address indexed anchor
    );

    event L2ToL1Tx(
        address caller,
        address indexed destination,
        uint256 indexed hash,
        uint256 indexed position,
        uint256 arbBlockNum,
        uint256 ethBlockNum,
        uint256 timestamp,
        uint256 callvalue,
        bytes data
    );

    function getAssertionCreationBlockForLogLookup(bytes32 assertionHash)
        external view returns (uint256);
    function sendMerkleTreeState()
        external view returns (uint256 size, bytes32 root, bytes32[] memory partials);
    function constructOutboxProof(uint64 size, uint64 leaf)
        external view returns (bytes32 send, bytes32 root, bytes32[] memory proof);
}

#[derive(Debug, Clone, Args)]
pub struct IngressCaptureArgs {
    #[arg(long)]
    pub manifest: PathBuf,
    #[arg(long)]
    pub prior_finality_state: PathBuf,
    #[arg(long)]
    pub ethereum_rpc: String,
    #[arg(long)]
    pub ethereum_consensus_rpc: String,
    #[arg(long)]
    pub arbitrum_rpc: String,
    #[arg(long)]
    pub deposit_tx: String,
    #[arg(long)]
    pub pftl_chain_id: String,
    #[arg(long)]
    pub pftl_genesis_hash: String,
    #[arg(long)]
    pub pftl_protocol_version: u32,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct FinalityBootstrapArgs {
    #[arg(long)]
    pub manifest: PathBuf,
    #[arg(long)]
    pub ethereum_rpc: String,
    #[arg(long)]
    pub ethereum_consensus_rpc: String,
    #[arg(long)]
    pub arbitrum_rpc: String,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct IngressAuditArgs {
    #[arg(long)]
    pub witness: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Clone)]
struct RpcClient {
    http: reqwest::Client,
}

impl RpcClient {
    fn new() -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .user_agent("postfiat-pfusdc-tier4/1")
                .build()?,
        })
    }

    async fn call<T: DeserializeOwned>(&self, url: &str, method: &str, params: Value) -> Result<T> {
        let response: Value = self
            .http
            .post(url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params,
            }))
            .send()
            .await
            .with_context(|| format!("send {method}"))?
            .error_for_status()
            .with_context(|| format!("HTTP failure from {method}"))?
            .json()
            .await
            .with_context(|| format!("decode {method} response"))?;
        if let Some(error) = response.get("error") {
            bail!("{method} RPC error: {error}");
        }
        serde_json::from_value(
            response
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("{method} response omitted result"))?,
        )
        .with_context(|| format!("decode {method} result"))
    }

    async fn beacon_get<T: DeserializeOwned>(&self, url: &str, path: &str) -> Result<T> {
        self.http
            .get(format!("{}/{}", url.trim_end_matches('/'), path))
            .send()
            .await
            .with_context(|| format!("GET beacon {path}"))?
            .error_for_status()
            .with_context(|| format!("beacon endpoint rejected {path}"))?
            .json()
            .await
            .with_context(|| format!("decode beacon {path}"))
    }

    async fn beacon_root(&self, url: &str, slot: u64) -> Result<Option<B256>> {
        let response = self
            .http
            .get(format!(
                "{}/eth/v1/beacon/blocks/{slot}/root",
                url.trim_end_matches('/')
            ))
            .send()
            .await
            .with_context(|| format!("GET beacon root at slot {slot}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let body: BeaconRootResponse = response.error_for_status()?.json().await?;
        Ok(Some(body.data.root))
    }
}

#[derive(Debug, serde::Deserialize)]
struct BeaconData<T> {
    data: T,
}

#[derive(Debug, serde::Deserialize)]
struct BeaconRootResponse {
    data: BeaconRootData,
}

#[derive(Debug, serde::Deserialize)]
struct BeaconRootData {
    root: B256,
}

#[derive(Debug)]
struct VerifiedFinality {
    prior_root: B256,
    prior_slot: u64,
    final_root: B256,
    final_slot: u64,
    execution_state_root: B256,
    execution_block_number: u64,
}

pub async fn capture_finality_bootstrap(args: FinalityBootstrapArgs) -> Result<()> {
    let (route_profile, policy) = read_manifest_bindings(&args.manifest)?;
    let rpc = RpcClient::new()?;
    let helios = capture_helios_inputs(&rpc, &policy, &args.ethereum_consensus_rpc, None).await?;
    let finality = verify_helios_inputs_host(&helios, &policy)?;
    let ethereum_block = quantity(finality.execution_block_number);

    let rollup_storage = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        &[policy.rollup_latest_confirmed_storage_slot],
        &ethereum_block,
    )
    .await?;
    anyhow::ensure!(
        rollup_storage.address == policy.arbitrum_rollup_address
            && rollup_storage.value.code_hash == policy.arbitrum_rollup_runtime_code_hash
            && rollup_storage.storage_slots.len() == 1
            && rollup_storage.storage_slots[0].key == policy.rollup_latest_confirmed_storage_slot,
        "Rollup proof does not match pinned address, code, and latestConfirmed slot"
    );
    let verified_slots = verify_storage_slot_proofs(finality.execution_state_root, &rollup_storage)
        .map_err(|error| anyhow!("invalid RollupCore account/storage proof: {error}"))?;
    anyhow::ensure!(
        verified_slots.len() == 1,
        "Rollup latestConfirmed proof must contain exactly one slot"
    );
    let assertion_hash = B256::from(rollup_storage.storage_slots[0].value.to_be_bytes::<32>());
    anyhow::ensure!(
        verified_slots[0].value == assertion_hash,
        "Rollup latestConfirmed storage proof returned a different value"
    );

    let anchor_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.ethereum_ingress_anchor_address,
        &[],
        &ethereum_block,
    )
    .await?;
    verify_account_code_host(
        finality.execution_state_root,
        &anchor_account,
        policy.ethereum_ingress_anchor_address,
        policy.ethereum_ingress_anchor_runtime_code_hash,
        "Ethereum ingress anchor",
    )?;

    let assertion = capture_assertion(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        assertion_hash,
        &ethereum_block,
    )
    .await?;
    anyhow::ensure!(
        assertion.machine_status == 1 && nitro_assertion_hash(&assertion) == assertion_hash,
        "latestConfirmed does not match the canonical Nitro assertion preimage"
    );
    let asserted_block: Block = rpc
        .call(
            &args.arbitrum_rpc,
            "eth_getBlockByHash",
            json!([assertion.block_hash, false]),
        )
        .await?;
    anyhow::ensure!(
        asserted_block.header.hash == assertion.block_hash
            && asserted_block.header.inner.hash_slow() == assertion.block_hash,
        "Arbitrum RPC returned a noncanonical asserted block"
    );
    let asserted_l2_block = quantity(asserted_block.header.inner.number);
    let vault_account = get_account_proof(
        &rpc,
        &args.arbitrum_rpc,
        policy.arbitrum_vault_address,
        &[],
        &asserted_l2_block,
    )
    .await?;
    let vault_label = format!(
        "Arbitrum vault at asserted L2 block {}",
        asserted_block.header.inner.number
    );
    verify_account_code_host(
        asserted_block.header.inner.state_root,
        &vault_account,
        policy.arbitrum_vault_address,
        policy.arbitrum_vault_runtime_code_hash,
        &vault_label,
    )?;
    let token_account = get_account_proof(
        &rpc,
        &args.arbitrum_rpc,
        policy.arbitrum_token_address,
        &[],
        &asserted_l2_block,
    )
    .await?;
    verify_account_code_host(
        asserted_block.header.inner.state_root,
        &token_account,
        policy.arbitrum_token_address,
        policy.arbitrum_token_runtime_code_hash,
        "Arbitrum token",
    )?;

    let checkpoint = EthereumArbitrumCheckpointV1 {
        ethereum_finalized_beacon_root: hex32(finality.final_root),
        ethereum_finalized_slot: finality.final_slot,
        arbitrum_assertion_hash: hex32(assertion_hash),
        assertion_l2_block_hash: hex32(assertion.block_hash),
        assertion_send_root: hex32(assertion.send_root),
    };
    let state = finality_state_from_checkpoint(&route_profile, &policy, checkpoint)?;
    write_new_json(&args.output, &state)?;
    println!(
        "captured Tier-4 finality bootstrap: Ethereum slot {}, assertion {}",
        state.latest.ethereum_finalized_slot, state.latest.arbitrum_assertion_hash
    );
    println!("wrote {}", args.output.display());
    Ok(())
}

pub async fn capture(args: IngressCaptureArgs) -> Result<()> {
    let (route_profile, policy) = read_manifest_bindings(&args.manifest)?;
    let finality_state: EthereumArbitrumFinalityStateV2 = read_json(&args.prior_finality_state)?;
    validate_finality_state_binding(&finality_state, &route_profile, &policy)?;
    let deposit_tx: B256 = args
        .deposit_tx
        .parse()
        .context("--deposit-tx must be an EVM bytes32 hash")?;
    let rpc = RpcClient::new()?;

    let receipt: Value = rpc
        .call(
            &args.arbitrum_rpc,
            "eth_getTransactionReceipt",
            json!([deposit_tx]),
        )
        .await?;
    ensure_successful_receipt(&receipt, deposit_tx)?;
    let (mut evidence, output_index, output) =
        decode_deposit_receipt(&receipt, deposit_tx, &policy)?;

    let helios = capture_helios_inputs(
        &rpc,
        &policy,
        &args.ethereum_consensus_rpc,
        Some(&finality_state.latest),
    )
    .await?;
    let verified_finality = verify_helios_inputs_host(&helios, &policy)?;
    anyhow::ensure!(
        verified_finality.prior_root
            == parse_hex32(&finality_state.latest.ethereum_finalized_beacon_root)?,
        "Helios witness does not start from the governed finalized root"
    );
    anyhow::ensure!(
        verified_finality.prior_slot == finality_state.latest.ethereum_finalized_slot,
        "Helios witness does not start from the governed finalized slot"
    );
    let ethereum_block_number = verified_finality.execution_block_number;
    let ethereum_block = quantity(ethereum_block_number);

    let rollup_storage = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        &[policy.rollup_latest_confirmed_storage_slot],
        &ethereum_block,
    )
    .await?;
    anyhow::ensure!(
        rollup_storage.storage_slots.len() == 1,
        "Rollup proof omitted latestConfirmed slot"
    );
    let assertion_hash = B256::from(rollup_storage.storage_slots[0].value.to_be_bytes::<32>());
    let ethereum_ingress_anchor_account = get_account_proof(
        &rpc,
        &args.ethereum_rpc,
        policy.ethereum_ingress_anchor_address,
        &[],
        &ethereum_block,
    )
    .await?;

    let assertion = capture_assertion(
        &rpc,
        &args.ethereum_rpc,
        policy.arbitrum_rollup_address,
        assertion_hash,
        &ethereum_block,
    )
    .await?;
    let asserted_block: Block = rpc
        .call(
            &args.arbitrum_rpc,
            "eth_getBlockByHash",
            json!([assertion.block_hash, false]),
        )
        .await?;
    anyhow::ensure!(
        asserted_block.header.hash == assertion.block_hash,
        "Arbitrum RPC returned the wrong asserted block"
    );
    anyhow::ensure!(
        asserted_block.header.inner.hash_slow() == assertion.block_hash,
        "canonical asserted L2 header RLP does not hash to the assertion block"
    );
    let mut asserted_l2_header_rlp = Vec::new();
    asserted_block
        .header
        .inner
        .encode(&mut asserted_l2_header_rlp);
    let asserted_l2_block_number = quantity(asserted_block.header.inner.number);
    let asserted_l2_vault_account = get_account_proof(
        &rpc,
        &args.arbitrum_rpc,
        policy.arbitrum_vault_address,
        &[],
        &asserted_l2_block_number,
    )
    .await?;
    let asserted_l2_token_account = get_account_proof(
        &rpc,
        &args.arbitrum_rpc,
        policy.arbitrum_token_address,
        &[],
        &asserted_l2_block_number,
    )
    .await?;

    let send_state_bytes = eth_call(
        &rpc,
        &args.arbitrum_rpc,
        ARBSYS_ADDRESS,
        sendMerkleTreeStateCall {}.abi_encode(),
        &asserted_l2_block_number,
    )
    .await?;
    let send_state = sendMerkleTreeStateCall::abi_decode_returns(&send_state_bytes)
        .context("decode ArbSys.sendMerkleTreeState")?;
    let send_size = u256_u64(send_state.size, "ArbSys send tree size")?;
    anyhow::ensure!(
        output_index < send_size,
        "deposit output index is outside the asserted send tree"
    );
    let outbox_bytes = eth_call(
        &rpc,
        &args.arbitrum_rpc,
        NODE_INTERFACE_ADDRESS,
        constructOutboxProofCall {
            size: send_size,
            leaf: output_index,
        }
        .abi_encode(),
        &asserted_l2_block_number,
    )
    .await?;
    let outbox = constructOutboxProofCall::abi_decode_returns(&outbox_bytes)
        .context("decode NodeInterface.constructOutboxProof")?;
    anyhow::ensure!(
        outbox.root == assertion.send_root && send_state.root == assertion.send_root,
        "asserted sendRoot differs from ArbSys/NodeInterface roots"
    );
    anyhow::ensure!(
        outbox.send == B256::from(output.hash.to_be_bytes::<32>()),
        "NodeInterface send hash differs from the L2ToL1Tx event"
    );

    // Tier-4 source coordinates are the proof-authenticated assertion/outbox
    // coordinates consumed by PFTL execution, not the host-only receipt
    // lookup coordinates used above to recover the canonical message.
    evidence.block_hash = hex32(assertion.block_hash);
    evidence.tx_hash = hex32(outbox.send);
    evidence.log_index = output_index;
    evidence
        .validate()
        .map_err(|error| anyhow!("canonical Tier-4 evidence is invalid: {error}"))?;

    let witness = PfUsdcIngressProofWitnessV2 {
        schema: PFUSDC_INGRESS_PROOF_WITNESS_SCHEMA_V2.to_string(),
        route_profile,
        policy,
        helios,
        rollup_storage,
        ethereum_ingress_anchor_account,
        assertion,
        asserted_l2_header_rlp: Bytes::from(asserted_l2_header_rlp),
        asserted_l2_vault_account,
        asserted_l2_token_account,
        output: NitroSendWitnessV1 {
            output_index,
            output_proof: outbox.proof,
            l2_sender: output.caller,
            destination: output.destination,
            l2_block_number: u256_u64(output.arbBlockNum, "L2ToL1Tx arbBlockNum")?,
            l1_block_number: u256_u64(output.ethBlockNum, "L2ToL1Tx ethBlockNum")?,
            timestamp: u256_u64(output.timestamp, "L2ToL1Tx timestamp")?,
            value: output.callvalue,
            calldata: output.data,
        },
        evidence,
        pftl_chain_id: args.pftl_chain_id,
        pftl_genesis_hash: args.pftl_genesis_hash,
        pftl_protocol_version: args.pftl_protocol_version,
    };
    let public_values = verify_ingress_witness_v2(&witness)
        .map_err(|error| anyhow!("captured witness failed native verification: {error}"))?;
    let mut advanced_state = finality_state;
    advanced_state
        .verify_and_advance(&public_values)
        .map_err(|error| anyhow!("captured witness cannot advance governed finality: {error}"))?;
    let encoded = serde_json::to_vec_pretty(&witness)?;
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, encoded)?;
    println!(
        "captured and verified ingress witness: assertion {}, deposit {}, finalized Ethereum block {}, public-values commitment {}",
        hex32(assertion_hash),
        witness.evidence.deposit_id,
        ethereum_block_number,
        public_values.public_values_commitment
    );
    println!("wrote {}", args.output.display());
    Ok(())
}

pub fn audit(args: IngressAuditArgs) -> Result<()> {
    let bytes =
        fs::read(&args.witness).with_context(|| format!("read {}", args.witness.display()))?;
    let original_json: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode {} as JSON", args.witness.display()))?;
    let original: PfUsdcIngressProofWitnessV2 =
        serde_json::from_value(original_json.clone()).context("decode base ingress witness")?;
    verify_ingress_witness_v2(&original)
        .map_err(|error| anyhow!("base ingress witness failed verification: {error}"))?;

    let cases = [
        ("wrong_ethereum_chain", "/policy/ethereum_chain_id"),
        ("wrong_genesis_checkpoint", "/helios/genesis_root"),
        ("wrong_rollup", "/policy/arbitrum_rollup_address"),
        (
            "wrong_latest_confirmed_slot",
            "/policy/rollup_latest_confirmed_storage_slot",
        ),
        (
            "wrong_latest_confirmed_value",
            "/rollup_storage/storage_slots/0/value",
        ),
        ("wrong_assertion_hash_preimage", "/assertion/send_root"),
        ("wrong_asserted_l2_block", "/assertion/block_hash"),
        ("wrong_asserted_l2_header", "/asserted_l2_header_rlp"),
        (
            "wrong_vault_code_hash",
            "/asserted_l2_vault_account/value/codeHash",
        ),
        (
            "wrong_token_code_hash",
            "/asserted_l2_token_account/value/codeHash",
        ),
        (
            "wrong_anchor_code_hash",
            "/ethereum_ingress_anchor_account/value/codeHash",
        ),
        ("wrong_output_index", "/output/output_index"),
        ("wrong_output_sender", "/output/l2_sender"),
        ("wrong_output_destination", "/output/destination"),
        ("wrong_output_calldata", "/output/calldata"),
        ("wrong_token", "/evidence/token_address"),
        ("wrong_recipient", "/evidence/pftl_recipient"),
        ("wrong_amount", "/evidence/amount_atoms"),
        ("wrong_nonce", "/evidence/nonce"),
        ("wrong_route", "/evidence/route_binding"),
        ("wrong_deposit_id", "/evidence/deposit_id"),
    ];
    let case_count = cases.len();
    let mut results = Vec::with_capacity(case_count);
    for (name, pointer) in cases {
        let mut mutated = original_json.clone();
        let field = mutated
            .pointer_mut(pointer)
            .ok_or_else(|| anyhow!("mutation {name} missing JSON pointer {pointer}"))?;
        mutate_json_scalar(field)
            .with_context(|| format!("mutation {name} at JSON pointer {pointer}"))?;
        anyhow::ensure!(mutated != original_json, "mutation {name} made no change");
        let rejection = match serde_json::from_value::<PfUsdcIngressProofWitnessV2>(mutated) {
            Ok(witness) => verify_ingress_witness_v2(&witness)
                .err()
                .ok_or_else(|| anyhow!("mutation {name} was accepted"))?,
            Err(error) => format!("decode rejected: {error}"),
        };
        results.push(json!({
            "case": name,
            "json_pointer": pointer,
            "rejected": true,
            "reason": rejection,
        }));
    }
    let report = json!({
        "schema": "postfiat.pfusdc.ingress_mutation_report.v1",
        "witness": args.witness,
        "base_verified": true,
        "cases": results,
        "passed": case_count,
        "failed": 0,
        "note": "SP1 proof-byte mutation and PFTL deposit replay are execution-level gates run after the single release proof exists"
    });
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    println!("ingress mutation matrix: {case_count} rejected, 0 accepted");
    println!("wrote {}", args.output.display());
    Ok(())
}

fn mutate_json_scalar(value: &mut Value) -> Result<()> {
    match value {
        Value::String(text) => {
            let last = text
                .pop()
                .ok_or_else(|| anyhow!("cannot mutate empty string"))?;
            text.push(if last == '0' { '1' } else { '0' });
            Ok(())
        }
        Value::Number(number) => {
            let value = number
                .as_u64()
                .ok_or_else(|| anyhow!("mutation supports only unsigned integer numbers"))?;
            *number = serde_json::Number::from(
                value
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("mutation integer overflow"))?,
            );
            Ok(())
        }
        _ => bail!("mutation target is not a string or unsigned integer"),
    }
}

async fn capture_helios_inputs(
    rpc: &RpcClient,
    policy: &PfUsdcIngressProofPolicyV2,
    consensus_rpc: &str,
    prior_checkpoint: Option<&EthereumArbitrumCheckpointV1>,
) -> Result<ProofInputs> {
    let finality: BeaconData<FinalityUpdate<MainnetConsensusSpec>> = rpc
        .beacon_get(consensus_rpc, "eth/v1/beacon/light_client/finality_update")
        .await?;
    let finality_update = finality.data;
    let final_slot = finality_update.finalized_header().beacon().slot;
    let (bootstrap_slot, checkpoint_root) = if let Some(checkpoint) = prior_checkpoint {
        checkpoint.validate().map_err(|error| anyhow!(error))?;
        anyhow::ensure!(
            checkpoint.ethereum_finalized_slot.is_multiple_of(32),
            "governed Ethereum checkpoint slot is not an epoch boundary"
        );
        let root = parse_hex32(&checkpoint.ethereum_finalized_beacon_root)?;
        let canonical = rpc
            .beacon_root(consensus_rpc, checkpoint.ethereum_finalized_slot)
            .await?
            .ok_or_else(|| {
                anyhow!("governed Ethereum checkpoint is unavailable from beacon RPC")
            })?;
        anyhow::ensure!(
            canonical == root,
            "beacon RPC root differs from governed Ethereum checkpoint"
        );
        (checkpoint.ethereum_finalized_slot, root)
    } else {
        let mut slot = final_slot.saturating_sub(CHECKPOINTS_BEHIND * 32) / 32 * 32;
        let root = loop {
            if let Some(root) = rpc.beacon_root(consensus_rpc, slot).await? {
                break root;
            }
            slot = slot
                .checked_sub(32)
                .ok_or_else(|| anyhow!("checkpoint search underflow"))?;
        };
        (slot, root)
    };
    let bootstrap: BeaconData<Bootstrap<MainnetConsensusSpec>> = rpc
        .beacon_get(
            consensus_rpc,
            &format!("eth/v1/beacon/light_client/bootstrap/{checkpoint_root}"),
        )
        .await?;
    let forks = supported_forks(policy.ethereum_chain_id)?;
    verify_bootstrap(&bootstrap.data, checkpoint_root, &forks)
        .map_err(|error| anyhow!("invalid Helios bootstrap: {error}"))?;
    let mut store = LightClientStore::<MainnetConsensusSpec>::default();
    apply_bootstrap(&mut store, &bootstrap.data);
    anyhow::ensure!(
        store.finalized_header.beacon().slot == bootstrap_slot,
        "Helios bootstrap returned a different checkpoint slot"
    );
    let period = calc_sync_period::<MainnetConsensusSpec>(bootstrap_slot);
    let update_response: Vec<BeaconData<Update<MainnetConsensusSpec>>> = rpc
        .beacon_get(
            consensus_rpc,
            &format!(
                "eth/v1/beacon/light_client/updates?start_period={period}&count={MAX_HELIOS_UPDATES}"
            ),
        )
        .await?;
    let mut updates: Vec<_> = update_response
        .into_iter()
        .map(|response| response.data)
        .filter(|update| {
            let update_period = calc_sync_period::<MainnetConsensusSpec>(*update.signature_slot());
            update_period >= period && update_period < period + u64::from(MAX_HELIOS_UPDATES)
        })
        .collect();
    updates.sort_by_key(|update| *update.signature_slot());
    anyhow::ensure!(
        updates.len() <= usize::from(MAX_HELIOS_UPDATES),
        "Helios update count exceeds guest bound"
    );
    anyhow::ensure!(
        final_slot > bootstrap_slot,
        "Helios finality does not advance"
    );
    let expected_current_slot = *finality_update.signature_slot();
    Ok(ProofInputs {
        updates,
        finality_update,
        expected_current_slot,
        store,
        genesis_root: policy.ethereum_genesis_validators_root,
        forks,
        contract_storage: vec![],
    })
}

fn verify_helios_inputs_host(
    inputs: &ProofInputs,
    policy: &PfUsdcIngressProofPolicyV2,
) -> Result<VerifiedFinality> {
    anyhow::ensure!(
        inputs.genesis_root == policy.ethereum_genesis_validators_root
            && inputs.contract_storage.is_empty()
            && inputs.expected_current_slot == *inputs.finality_update.signature_slot(),
        "Helios input does not match pinned Ethereum policy"
    );
    let expected_forks = supported_forks(policy.ethereum_chain_id)?;
    anyhow::ensure!(
        forks_equal(&inputs.forks, &expected_forks),
        "Helios fork schedule does not match pinned network"
    );
    anyhow::ensure!(
        inputs.updates.len() <= usize::from(MAX_HELIOS_UPDATES),
        "Helios update count exceeds guest bound"
    );
    let mut store = inputs.store.clone();
    store.next_sync_committee = None;
    let prior_root: B256 = store.finalized_header.beacon().tree_hash_root();
    let prior_slot = store.finalized_header.beacon().slot;
    for update in &inputs.updates {
        verify_update(
            update,
            inputs.expected_current_slot,
            &store,
            inputs.genesis_root,
            &inputs.forks,
        )
        .map_err(|error| anyhow!("invalid Helios committee update: {error}"))?;
        apply_update(&mut store, update);
    }
    verify_finality_update(
        &inputs.finality_update,
        inputs.expected_current_slot,
        &store,
        inputs.genesis_root,
        &inputs.forks,
    )
    .map_err(|error| anyhow!("invalid Helios finality update: {error}"))?;
    apply_finality_update(&mut store, &inputs.finality_update);
    let final_slot = store.finalized_header.beacon().slot;
    anyhow::ensure!(
        final_slot > prior_slot && final_slot.is_multiple_of(32),
        "Ethereum finalized checkpoint did not canonically advance"
    );
    let final_root: B256 = store.finalized_header.beacon().tree_hash_root();
    let execution = store
        .finalized_header
        .execution()
        .map_err(|_| anyhow!("finalized Ethereum header has no execution payload"))?;
    Ok(VerifiedFinality {
        prior_root,
        prior_slot,
        final_root,
        final_slot,
        execution_state_root: *execution.state_root(),
        execution_block_number: *execution.block_number(),
    })
}

fn forks_equal(left: &Forks, right: &Forks) -> bool {
    [
        (&left.genesis, &right.genesis),
        (&left.altair, &right.altair),
        (&left.bellatrix, &right.bellatrix),
        (&left.capella, &right.capella),
        (&left.deneb, &right.deneb),
        (&left.electra, &right.electra),
        (&left.fulu, &right.fulu),
    ]
    .iter()
    .all(|(left, right)| left.epoch == right.epoch && left.fork_version == right.fork_version)
}

fn supported_forks(chain_id: u64) -> Result<Forks> {
    let fork = |epoch, fork_version| Fork {
        epoch,
        fork_version: FixedBytes::<4>::from(fork_version),
    };
    match chain_id {
        1 => Ok(Forks {
            genesis: fork(0, [0, 0, 0, 0]),
            altair: fork(74_240, [1, 0, 0, 0]),
            bellatrix: fork(144_896, [2, 0, 0, 0]),
            capella: fork(194_048, [3, 0, 0, 0]),
            deneb: fork(269_568, [4, 0, 0, 0]),
            electra: fork(364_032, [5, 0, 0, 0]),
            fulu: fork(411_392, [6, 0, 0, 0]),
        }),
        11_155_111 => Ok(Forks {
            genesis: fork(0, [0x90, 0, 0, 0x69]),
            altair: fork(50, [0x90, 0, 0, 0x70]),
            bellatrix: fork(100, [0x90, 0, 0, 0x71]),
            capella: fork(56_832, [0x90, 0, 0, 0x72]),
            deneb: fork(132_608, [0x90, 0, 0, 0x73]),
            electra: fork(222_464, [0x90, 0, 0, 0x74]),
            fulu: fork(272_640, [0x90, 0, 0, 0x75]),
        }),
        _ => bail!("unsupported Ethereum chain {chain_id}"),
    }
}

async fn capture_assertion(
    rpc: &RpcClient,
    url: &str,
    rollup: Address,
    assertion_hash: B256,
    block: &str,
) -> Result<NitroAssertionWitnessV1> {
    let creation_call = getAssertionCreationBlockForLogLookupCall {
        assertionHash: assertion_hash,
    }
    .abi_encode();
    let creation_bytes = eth_call(rpc, url, rollup, creation_call, block).await?;
    let creation = getAssertionCreationBlockForLogLookupCall::abi_decode_returns(&creation_bytes)
        .context("decode assertion creation block")?;
    let creation_block = quantity_u256(creation);
    let logs: Vec<Value> = rpc
        .call(
            url,
            "eth_getLogs",
            json!([{
                "address": rollup,
                "fromBlock": creation_block,
                "toBlock": creation_block,
                "topics": [AssertionCreated::SIGNATURE_HASH, assertion_hash]
            }]),
        )
        .await?;
    anyhow::ensure!(logs.len() == 1, "expected one AssertionCreated log");
    let (topics, data) = raw_log(&logs[0])?;
    let event = AssertionCreated::decode_raw_log_validate(&topics, &data)
        .context("decode AssertionCreated log")?;
    anyhow::ensure!(event.assertionHash == assertion_hash, "wrong assertion log");
    Ok(NitroAssertionWitnessV1 {
        parent_assertion_hash: event.parentAssertionHash,
        block_hash: event.assertion.afterState.globalState.bytes32Vals[0],
        send_root: event.assertion.afterState.globalState.bytes32Vals[1],
        inbox_position: event.assertion.afterState.globalState.u64Vals[0],
        position_in_message: event.assertion.afterState.globalState.u64Vals[1],
        machine_status: event.assertion.afterState.machineStatus,
        end_history_root: event.assertion.afterState.endHistoryRoot,
        inbox_accumulator: event.afterInboxBatchAcc,
    })
}

fn decode_deposit_receipt(
    receipt: &Value,
    tx_hash: B256,
    policy: &PfUsdcIngressProofPolicyV2,
) -> Result<(VaultBridgeDepositEvidence, u64, L2ToL1Tx)> {
    let block_hash: B256 = parse_value(
        receipt
            .get("blockHash")
            .ok_or_else(|| anyhow!("receipt omitted blockHash"))?,
    )?;
    let logs = receipt
        .get("logs")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("receipt omitted logs"))?;
    let mut deposit = None;
    let mut deposit_log_index = None;
    let mut commitment = None;
    let mut output = None;
    for log in logs {
        let address: Address = parse_value(
            log.get("address")
                .ok_or_else(|| anyhow!("log omitted address"))?,
        )?;
        let (topics, data) = raw_log(log)?;
        let Some(signature) = topics.first() else {
            continue;
        };
        if address == policy.arbitrum_vault_address
            && *signature == ERC20BridgeDepositedV2::SIGNATURE_HASH
        {
            anyhow::ensure!(deposit.is_none(), "duplicate deposit event in receipt");
            deposit = Some(
                ERC20BridgeDepositedV2::decode_raw_log_validate(&topics, &data)
                    .context("decode ERC20BridgeDepositedV2")?,
            );
            deposit_log_index = Some(parse_quantity_value(
                log.get("logIndex")
                    .ok_or_else(|| anyhow!("deposit log omitted logIndex"))?,
            )?);
        } else if address == policy.arbitrum_vault_address
            && *signature == Tier4IngressCommitment::SIGNATURE_HASH
        {
            anyhow::ensure!(
                commitment.is_none(),
                "duplicate ingress commitment in receipt"
            );
            commitment = Some(
                Tier4IngressCommitment::decode_raw_log_validate(&topics, &data)
                    .context("decode Tier4IngressCommitment")?,
            );
        } else if address == ARBSYS_ADDRESS && *signature == L2ToL1Tx::SIGNATURE_HASH {
            let candidate = L2ToL1Tx::decode_raw_log_validate(&topics, &data)
                .context("decode ArbSys L2ToL1Tx")?;
            if candidate.caller == policy.arbitrum_vault_address {
                anyhow::ensure!(output.is_none(), "duplicate vault L2ToL1Tx in receipt");
                output = Some(candidate);
            }
        }
    }
    let deposit = deposit.ok_or_else(|| anyhow!("receipt omitted canonical deposit event"))?;
    let commitment = commitment.ok_or_else(|| anyhow!("receipt omitted Tier4IngressCommitment"))?;
    let output = output.ok_or_else(|| anyhow!("receipt omitted vault L2ToL1Tx"))?;
    anyhow::ensure!(
        deposit.depositId == commitment.depositId
            && deposit.depositId == B256::from(output_data_deposit_id(&output.data)?),
        "deposit, commitment, and output calldata IDs differ"
    );
    anyhow::ensure!(
        commitment.anchor == policy.ethereum_ingress_anchor_address
            && output.destination == policy.ethereum_ingress_anchor_address,
        "receipt output has wrong ingress anchor"
    );
    let amount_atoms = u256_u64(deposit.amount, "deposit amount")?;
    let evidence = VaultBridgeDepositEvidence {
        source_chain_id: u256_u64(deposit.sourceChainId, "deposit sourceChainId")?,
        vault_address: evm_address_text(deposit.vault),
        token_address: evm_address_text(deposit.token),
        depositor: evm_address_text(deposit.depositor),
        pftl_recipient: deposit.pftlRecipient,
        pftl_recipient_hash: hex32(deposit.pftlRecipientHash),
        amount_atoms,
        nonce: hex32(deposit.nonce),
        route_binding: hex32(deposit.routeBinding),
        deposit_id: hex32(deposit.depositId),
        block_hash: hex32(block_hash),
        tx_hash: hex32(tx_hash),
        log_index: deposit_log_index.ok_or_else(|| anyhow!("missing deposit log index"))?,
    };
    evidence.validate().map_err(|error| anyhow!(error))?;
    Ok((
        evidence,
        u256_u64(commitment.outputIndex, "commitment outputIndex")?,
        output,
    ))
}

fn output_data_deposit_id(data: &[u8]) -> Result<[u8; 32]> {
    anyhow::ensure!(data.len() >= 36, "recordDepositV1 calldata is truncated");
    let mut id = [0_u8; 32];
    id.copy_from_slice(&data[4..36]);
    Ok(id)
}

async fn get_account_proof(
    rpc: &RpcClient,
    url: &str,
    address: Address,
    slots: &[B256],
    block: &str,
) -> Result<ContractStorage> {
    let proof: EIP1186AccountProofResponse = rpc
        .call(url, "eth_getProof", json!([address, slots, block]))
        .await?;
    Ok(ContractStorage {
        address: proof.address,
        value: alloy_trie::TrieAccount {
            nonce: proof.nonce,
            balance: proof.balance,
            storage_root: proof.storage_hash,
            code_hash: proof.code_hash,
        },
        mpt_proof: proof.account_proof,
        storage_slots: proof
            .storage_proof
            .into_iter()
            .map(|entry| StorageSlotWithProof {
                key: entry.key.as_b256(),
                value: entry.value,
                mpt_proof: entry.proof,
            })
            .collect(),
    })
}

async fn eth_call(
    rpc: &RpcClient,
    url: &str,
    to: Address,
    data: Vec<u8>,
    block: &str,
) -> Result<Bytes> {
    rpc.call(
        url,
        "eth_call",
        json!([{"to": to, "data": Bytes::from(data)}, block]),
    )
    .await
}

fn ensure_successful_receipt(receipt: &Value, expected_tx: B256) -> Result<()> {
    anyhow::ensure!(!receipt.is_null(), "deposit transaction has no receipt");
    let status = parse_quantity_value(
        receipt
            .get("status")
            .ok_or_else(|| anyhow!("receipt omitted status"))?,
    )?;
    anyhow::ensure!(status == 1, "deposit transaction reverted");
    let tx_hash: B256 = parse_value(
        receipt
            .get("transactionHash")
            .ok_or_else(|| anyhow!("receipt omitted transactionHash"))?,
    )?;
    anyhow::ensure!(tx_hash == expected_tx, "RPC returned the wrong receipt");
    Ok(())
}

fn raw_log(log: &Value) -> Result<(Vec<B256>, Bytes)> {
    let topics: Vec<B256> = parse_value(
        log.get("topics")
            .ok_or_else(|| anyhow!("log omitted topics"))?,
    )?;
    let data: Bytes = parse_value(log.get("data").ok_or_else(|| anyhow!("log omitted data"))?)?;
    Ok((topics, data))
}

fn read_manifest_bindings(
    path: &PathBuf,
) -> Result<(VaultBridgeRouteProfileV1, PfUsdcIngressProofPolicyV2)> {
    let document: Value = read_json(path)?;
    anyhow::ensure!(
        document.get("schema").and_then(Value::as_str)
            == Some("postfiat.pfusdc.tier4_deployment_manifest.v1"),
        "deployment manifest schema mismatch"
    );
    let route_profile: VaultBridgeRouteProfileV1 = serde_json::from_value(
        document
            .pointer("/route_profile/profile")
            .cloned()
            .ok_or_else(|| anyhow!("deployment manifest omitted route profile"))?,
    )
    .context("decode deployment manifest route profile")?;
    let policy: PfUsdcIngressProofPolicyV2 = serde_json::from_value(
        document
            .pointer("/ingress_policy/policy")
            .cloned()
            .ok_or_else(|| anyhow!("deployment manifest omitted ingress policy"))?,
    )
    .context("decode deployment manifest ingress policy")?;
    route_profile.validate().map_err(|error| anyhow!(error))?;
    let profile_hash = route_profile
        .profile_hash()
        .map_err(|error| anyhow!(error))?;
    let declared_profile_hash = document
        .pointer("/route_profile/profile_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("deployment manifest omitted route profile hash"))?;
    anyhow::ensure!(
        declared_profile_hash == profile_hash,
        "deployment manifest route profile hash mismatch"
    );
    let policy_hash = ingress_policy_hash_v2(&policy);
    let declared_policy_hash = document
        .pointer("/ingress_policy/policy_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("deployment manifest omitted ingress policy hash"))?;
    anyhow::ensure!(
        declared_policy_hash == policy_hash,
        "deployment manifest ingress policy hash mismatch"
    );
    validate_route_policy_binding(&route_profile, &policy)?;
    Ok((route_profile, policy))
}

fn validate_route_policy_binding(
    route_profile: &VaultBridgeRouteProfileV1,
    policy: &PfUsdcIngressProofPolicyV2,
) -> Result<()> {
    anyhow::ensure!(
        route_profile.verifier_kind == NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
            && route_profile.verifier_policy_hash == ingress_policy_hash_v2(policy)
            && route_profile.source_chain_id == policy.arbitrum_chain_id
            && route_profile.vault_address == evm_address_text(policy.arbitrum_vault_address)
            && normalized_hash(&route_profile.vault_runtime_code_hash)
                == hex32(policy.arbitrum_vault_runtime_code_hash)
            && route_profile.token_address == evm_address_text(policy.arbitrum_token_address)
            && normalized_hash(&route_profile.token_runtime_code_hash)
                == hex32(policy.arbitrum_token_runtime_code_hash),
        "deployment manifest route and ingress policy are not exactly bound"
    );
    Ok(())
}

fn finality_state_from_checkpoint(
    route_profile: &VaultBridgeRouteProfileV1,
    policy: &PfUsdcIngressProofPolicyV2,
    checkpoint: EthereumArbitrumCheckpointV1,
) -> Result<EthereumArbitrumFinalityStateV2> {
    validate_route_policy_binding(route_profile, policy)?;
    checkpoint.validate().map_err(|error| anyhow!(error))?;
    let state = EthereumArbitrumFinalityStateV2 {
        schema: ETHEREUM_ARBITRUM_FINALITY_STATE_SCHEMA_V2.to_string(),
        route_profile_hash: route_profile
            .profile_hash()
            .map_err(|error| anyhow!(error))?,
        route_epoch: u64::from(route_profile.route_epoch),
        ethereum_chain_id: policy.ethereum_chain_id,
        arbitrum_chain_id: policy.arbitrum_chain_id,
        arbitrum_rollup_address: evm_address_text(policy.arbitrum_rollup_address),
        arbitrum_rollup_runtime_code_hash: format!(
            "{:#x}",
            policy.arbitrum_rollup_runtime_code_hash
        ),
        rollup_latest_confirmed_storage_slot: hex32(policy.rollup_latest_confirmed_storage_slot),
        vault_address: route_profile.vault_address.clone(),
        vault_runtime_code_hash: route_profile.vault_runtime_code_hash.clone(),
        token_address: route_profile.token_address.clone(),
        token_runtime_code_hash: route_profile.token_runtime_code_hash.clone(),
        ethereum_ingress_anchor_address: evm_address_text(policy.ethereum_ingress_anchor_address),
        ethereum_ingress_anchor_runtime_code_hash: format!(
            "{:#x}",
            policy.ethereum_ingress_anchor_runtime_code_hash
        ),
        latest: checkpoint.clone(),
        retained: vec![checkpoint],
    };
    state.validate().map_err(|error| anyhow!(error))?;
    Ok(state)
}

fn validate_finality_state_binding(
    state: &EthereumArbitrumFinalityStateV2,
    route_profile: &VaultBridgeRouteProfileV1,
    policy: &PfUsdcIngressProofPolicyV2,
) -> Result<()> {
    state.validate().map_err(|error| anyhow!(error))?;
    let mut expected = finality_state_from_checkpoint(route_profile, policy, state.latest.clone())?;
    expected.retained.clone_from(&state.retained);
    anyhow::ensure!(
        state == &expected,
        "governed finality state does not exactly match frozen route and policy"
    );
    Ok(())
}

fn verify_account_code_host(
    state_root: B256,
    account: &ContractStorage,
    expected_address: Address,
    expected_code_hash: B256,
    label: &str,
) -> Result<()> {
    anyhow::ensure!(
        account.address == expected_address
            && account.value.code_hash == expected_code_hash
            && account.storage_slots.is_empty(),
        "{label} account proof does not match pinned address/code: observed_address={:#x} expected_address={:#x} observed_code_hash={:#x} expected_code_hash={:#x} storage_slots={}",
        account.address,
        expected_address,
        account.value.code_hash,
        expected_code_hash,
        account.storage_slots.len()
    );
    let slots = verify_storage_slot_proofs(state_root, account)
        .map_err(|error| anyhow!("invalid {label} account proof: {error}"))?;
    anyhow::ensure!(
        slots.is_empty(),
        "{label} proof returned unexpected storage"
    );
    Ok(())
}

fn nitro_assertion_hash(assertion: &NitroAssertionWitnessV1) -> B256 {
    let global = CaptureGlobalState {
        bytes32Vals: [assertion.block_hash, assertion.send_root],
        u64Vals: [assertion.inbox_position, assertion.position_in_message],
    };
    let state = CaptureAssertionState {
        globalState: global,
        machineStatus: assertion.machine_status,
        endHistoryRoot: assertion.end_history_root,
    };
    let state_hash = keccak256(state.abi_encode());
    let mut preimage = Vec::with_capacity(96);
    preimage.extend_from_slice(assertion.parent_assertion_hash.as_slice());
    preimage.extend_from_slice(state_hash.as_slice());
    preimage.extend_from_slice(assertion.inbox_accumulator.as_slice());
    keccak256(preimage)
}

fn write_new_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<()> {
    anyhow::ensure!(!path.exists(), "refusing to overwrite {}", path.display());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let temporary = path.with_extension("json.tmp");
    anyhow::ensure!(
        !temporary.exists(),
        "refusing to overwrite temporary output {}",
        temporary.display()
    );
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn parse_hex32(value: &str) -> Result<B256> {
    let normalized = value.strip_prefix("0x").unwrap_or(value);
    anyhow::ensure!(
        normalized.len() == 64
            && normalized.bytes().all(|byte| byte.is_ascii_hexdigit())
            && normalized == normalized.to_ascii_lowercase(),
        "expected canonical lowercase bytes32"
    );
    format!("0x{normalized}")
        .parse()
        .context("decode canonical bytes32")
}

fn normalized_hash(value: &str) -> &str {
    value.strip_prefix("0x").unwrap_or(value)
}

fn read_json<T: DeserializeOwned>(path: &PathBuf) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("decode {}", path.display()))
}

fn parse_value<T: DeserializeOwned>(value: &Value) -> Result<T> {
    serde_json::from_value(value.clone()).map_err(Into::into)
}

fn parse_quantity_value(value: &Value) -> Result<u64> {
    let text = value
        .as_str()
        .ok_or_else(|| anyhow!("RPC quantity is not a string"))?;
    u64::from_str_radix(text.strip_prefix("0x").unwrap_or(text), 16)
        .with_context(|| format!("invalid RPC quantity {text}"))
}

fn u256_u64(value: U256, label: &str) -> Result<u64> {
    value.try_into().map_err(|_| anyhow!("{label} exceeds u64"))
}

fn quantity(value: u64) -> String {
    format!("0x{value:x}")
}

fn quantity_u256(value: U256) -> String {
    format!("0x{value:x}")
}

fn hex32(value: B256) -> String {
    format!("{value:x}")
}

fn evm_address_text(value: Address) -> String {
    format!("{value:#x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frozen_manifest() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../deployments/pfusdc-tier4-sepolia-20260718/manifest.json")
    }

    fn checkpoint() -> EthereumArbitrumCheckpointV1 {
        EthereumArbitrumCheckpointV1 {
            ethereum_finalized_beacon_root: "11".repeat(32),
            ethereum_finalized_slot: 32,
            arbitrum_assertion_hash: "22".repeat(32),
            assertion_l2_block_hash: "33".repeat(32),
            assertion_send_root: "44".repeat(32),
        }
    }

    #[test]
    fn capture_abis_pin_official_arbos_surfaces() {
        assert_eq!(
            L2ToL1Tx::SIGNATURE,
            "L2ToL1Tx(address,address,uint256,uint256,uint256,uint256,uint256,uint256,bytes)"
        );
        assert_eq!(
            constructOutboxProofCall::SIGNATURE,
            "constructOutboxProof(uint64,uint64)"
        );
        assert_eq!(sendMerkleTreeStateCall::SIGNATURE, "sendMerkleTreeState()");
        assert_eq!(
            getAssertionCreationBlockForLogLookupCall::SIGNATURE,
            "getAssertionCreationBlockForLogLookup(bytes32)"
        );
    }

    #[test]
    fn capture_forks_match_guest_allowlist() {
        let mainnet = supported_forks(1).expect("mainnet forks");
        assert_eq!(mainnet.electra.epoch, 364_032);
        assert_eq!(mainnet.fulu.fork_version, FixedBytes::from([6, 0, 0, 0]));
        let sepolia = supported_forks(11_155_111).expect("sepolia forks");
        assert_eq!(sepolia.electra.epoch, 222_464);
        assert_eq!(
            sepolia.fulu.fork_version,
            FixedBytes::from([0x90, 0, 0, 0x75])
        );
        assert!(supported_forks(42).is_err());
    }

    #[test]
    fn mutation_and_calldata_extractors_are_bounded() {
        let id = [0x44_u8; 32];
        let mut calldata = vec![0xaa, 0xbb, 0xcc, 0xdd];
        calldata.extend_from_slice(&id);
        assert_eq!(output_data_deposit_id(&calldata).expect("deposit id"), id);
        assert!(output_data_deposit_id(&calldata[..35]).is_err());

        let mut text = Value::String("0x10".to_string());
        mutate_json_scalar(&mut text).expect("string mutation");
        assert_eq!(text, Value::String("0x11".to_string()));
        let mut number = json!(7);
        mutate_json_scalar(&mut number).expect("number mutation");
        assert_eq!(number, json!(8));
        assert!(mutate_json_scalar(&mut Value::Bool(true)).is_err());
    }

    #[test]
    fn frozen_manifest_builds_exact_finality_state_binding() {
        let (route, policy) = read_manifest_bindings(&frozen_manifest()).expect("frozen manifest");
        let state = finality_state_from_checkpoint(&route, &policy, checkpoint())
            .expect("valid finality state");
        validate_finality_state_binding(&state, &route, &policy).expect("exact binding");
        assert_eq!(
            state.route_profile_hash,
            "7b93053c2a1a26b918c3bd2cd4737d1e00f3f5cf0f8cb8fba9aff1a1126eac10516881b4a2bb153200f9c08fe8c1b5ef"
        );
        assert_eq!(state.latest, state.retained[0]);
    }

    #[test]
    fn finality_binding_and_checkpoint_encoding_fail_closed() {
        let (route, policy) = read_manifest_bindings(&frozen_manifest()).expect("frozen manifest");
        let mut state = finality_state_from_checkpoint(&route, &policy, checkpoint())
            .expect("valid finality state");
        state.arbitrum_rollup_address = "0x0000000000000000000000000000000000000001".to_string();
        assert!(validate_finality_state_binding(&state, &route, &policy).is_err());
        assert!(parse_hex32(&"AA".repeat(32)).is_err());
        assert!(parse_hex32(&"00".repeat(31)).is_err());
        assert_eq!(
            parse_hex32(&"ab".repeat(32)).expect("bytes32"),
            B256::repeat_byte(0xab)
        );
    }
}
