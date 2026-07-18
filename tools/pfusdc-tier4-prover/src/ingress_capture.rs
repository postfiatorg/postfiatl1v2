use std::{fs, path::PathBuf};

use alloy::{
    primitives::{Address, Bytes, FixedBytes, B256, U256},
    rpc::types::{Block, EIP1186AccountProofResponse},
};
use alloy_rlp::Encodable;
use alloy_sol_types::{sol, SolCall, SolEvent};
use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use helios_consensus_core::{
    apply_bootstrap, calc_sync_period,
    consensus_spec::MainnetConsensusSpec,
    types::{Bootstrap, FinalityUpdate, Fork, Forks, LightClientStore, Update},
    verify_bootstrap,
};
use pfusdc_ingress_program::{
    verify_ingress_witness_v2, NitroAssertionWitnessV1, NitroSendWitnessV1,
    PfUsdcIngressProofPolicyV2, PfUsdcIngressProofWitnessV2,
    PFUSDC_INGRESS_PROOF_WITNESS_SCHEMA_V2,
};
use postfiat_types::{VaultBridgeDepositEvidence, VaultBridgeRouteProfileV1};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sp1_helios_primitives::types::{ContractStorage, ProofInputs, StorageSlotWithProof};

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
    pub route_profile: PathBuf,
    #[arg(long)]
    pub policy: PathBuf,
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

pub async fn capture(args: IngressCaptureArgs) -> Result<()> {
    let route_profile: VaultBridgeRouteProfileV1 = read_json(&args.route_profile)?;
    let policy: PfUsdcIngressProofPolicyV2 = read_json(&args.policy)?;
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
    let (evidence, output_index, output) = decode_deposit_receipt(&receipt, deposit_tx, &policy)?;

    let helios = capture_helios_inputs(&rpc, &policy, &args.ethereum_consensus_rpc).await?;
    let execution = helios
        .finality_update
        .finalized_header()
        .execution()
        .map_err(|_| anyhow!("finalized Helios header omitted execution payload"))?;
    let ethereum_block_number = *execution.block_number();
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
) -> Result<ProofInputs> {
    let finality: BeaconData<FinalityUpdate<MainnetConsensusSpec>> = rpc
        .beacon_get(consensus_rpc, "eth/v1/beacon/light_client/finality_update")
        .await?;
    let finality_update = finality.data;
    let final_slot = finality_update.finalized_header().beacon().slot;
    let mut bootstrap_slot = final_slot.saturating_sub(CHECKPOINTS_BEHIND * 32) / 32 * 32;
    let checkpoint_root = loop {
        if let Some(root) = rpc.beacon_root(consensus_rpc, bootstrap_slot).await? {
            break root;
        }
        bootstrap_slot = bootstrap_slot
            .checked_sub(32)
            .ok_or_else(|| anyhow!("checkpoint search underflow"))?;
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
        vault_address: deposit.vault.to_string(),
        token_address: deposit.token.to_string(),
        depositor: deposit.depositor.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
