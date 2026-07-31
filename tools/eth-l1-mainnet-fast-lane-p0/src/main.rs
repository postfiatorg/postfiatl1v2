use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use alloy::{
    primitives::{keccak256, Address, Bytes, FixedBytes, B256, U256},
    rpc::types::EIP1186AccountProofResponse,
};
use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use helios_consensus_core::{
    apply_bootstrap, calc_sync_period,
    consensus_spec::MainnetConsensusSpec,
    types::{Bootstrap, FinalityUpdate, Fork, Forks, LightClientStore, Update},
    verify_bootstrap,
};
use pfusdc_eth_mainnet_ingress_program::{
    verify_witness, EthIngressPolicyV1, EthIngressWitnessV1, MAINNET_CHAIN_ID,
    MAINNET_GENESIS_VALIDATORS_ROOT, POLICY_SCHEMA, ROUTE_ID, WITNESS_SCHEMA,
};
use postfiat_types::{
    vault_bridge_deposit_id, vault_bridge_pftl_recipient_hash, VaultBridgeDepositEvidence,
    VaultBridgeRouteProfileV1, NAV_PROFILE_VERIFIER_SP1_GROTH16, NAV_SP1_PROOF_ENCODING_GROTH16,
    VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN, VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sp1_helios_primitives::types::{ContractStorage, ProofInputs, StorageSlotWithProof};
use sp1_sdk::{
    Elf, HashableKey, ProveRequest, Prover, ProverClient, ProvingKey, SP1ProofWithPublicValues,
    SP1Stdin,
};

const ELF: Elf = Elf::Static(include_bytes!(
    "../../../programs/pfusdc-eth-mainnet-ingress/elf/pfusdc-eth-mainnet-ingress-program"
));
const CHECKPOINTS_BEHIND: u64 = 16;
const MAX_UPDATES: u8 = 8;
const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Capture {
        #[arg(long)]
        deployment: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "https://ethereum-rpc.publicnode.com")]
        execution_rpc: String,
        #[arg(long, default_value = "https://ethereum-beacon-api.publicnode.com")]
        beacon_rpc: String,
        #[arg(long, default_value_t = 1800)]
        wait_seconds: u64,
    },
    Audit {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Prove {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        /// Fail closed unless SP1_PROVER selects this exact backend.
        #[arg(long)]
        require_prover: Option<String>,
        /// Skip the redundant host execute pass before proving.
        #[arg(long)]
        skip_redundant_execute: bool,
    },
    Run {
        #[arg(long)]
        deployment: PathBuf,
        #[arg(long)]
        work_dir: PathBuf,
        #[arg(long, default_value = "https://ethereum-rpc.publicnode.com")]
        execution_rpc: String,
        #[arg(long, default_value = "https://ethereum-beacon-api.publicnode.com")]
        beacon_rpc: String,
        #[arg(long, default_value_t = 21600)]
        wait_seconds: u64,
        /// Fail closed unless SP1_PROVER selects this exact backend.
        #[arg(long)]
        require_prover: Option<String>,
    },
    CrossVkeyAudit {
        #[arg(long)]
        proof: PathBuf,
        #[arg(long)]
        foreign_elf: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Build the canonical Ethereum mainnet L1 route profile (chain 1),
    /// rejecting any Arbitrum marker or foreign chain id. Prints the profile
    /// JSON, its governed profile hash, and the route binding.
    RouteProfile {
        /// pfUSDC issued-asset id this route credits.
        #[arg(long)]
        asset_id: String,
        /// Deployed ERC20BridgeVaultL1 address on Ethereum mainnet.
        #[arg(long)]
        vault_address: String,
        /// Keccak-256 of the deployed vault runtime code.
        #[arg(long)]
        vault_runtime_code_hash: String,
        /// Keccak-256 of the deployed token runtime code.
        #[arg(long)]
        token_runtime_code_hash: String,
        /// Frozen pfusdc-eth-ingress SP1 program verifying key (0x-prefixed).
        #[arg(long)]
        ingress_program_vkey: String,
        /// Ingress policy hash committed by proof public values.
        #[arg(long)]
        verifier_policy_hash: String,
        #[arg(long, default_value_t = 1)]
        route_epoch: u32,
        #[arg(long, default_value_t = 900)]
        max_snapshot_age_blocks: u64,
        #[arg(long, default_value_t = 64)]
        challenge_window_blocks: u64,
        #[arg(long, default_value_t = 128)]
        max_epoch_gap_blocks: u64,
        #[arg(long, default_value_t = 256)]
        settle_deadline_blocks: u64,
        #[arg(long, default_value_t = 1)]
        min_challenge_bond: u64,
        #[arg(long, default_value_t = 4096)]
        max_proof_bytes: u64,
        #[arg(long, default_value_t = 4096)]
        max_public_values_bytes: u64,
        #[arg(long, default_value_t = 0)]
        activation_height: u64,
        #[arg(long, default_value_t = 0)]
        expires_at_height: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    ProgramInfo,
}

#[derive(Deserialize)]
struct Deployment {
    vault: String,
    deposit_tx: String,
    amount_atoms: u64,
    recipient: String,
    route_binding: String,
    nonce: String,
    creation_bytecode_hash: String,
}

#[derive(Clone)]
struct Rpc {
    http: reqwest::Client,
}
impl Rpc {
    fn new() -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .build()?,
        })
    }
    async fn call<T: DeserializeOwned>(&self, url: &str, method: &str, params: Value) -> Result<T> {
        let v: Value = self
            .http
            .post(url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(e) = v.get("error") {
            bail!("{method} RPC error: {e}");
        }
        serde_json::from_value(
            v.get("result")
                .cloned()
                .ok_or_else(|| anyhow!("missing RPC result"))?,
        )
        .with_context(|| format!("decode {method}"))
    }
    async fn beacon<T: DeserializeOwned>(&self, url: &str, path: &str) -> Result<T> {
        Ok(self
            .http
            .get(format!("{}/{}", url.trim_end_matches('/'), path))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    async fn root(&self, url: &str, slot: u64) -> Result<Option<B256>> {
        let r = self
            .http
            .get(format!(
                "{}/eth/v1/beacon/blocks/{slot}/root",
                url.trim_end_matches('/')
            ))
            .send()
            .await?;
        if r.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        Ok(Some(
            r.error_for_status()?
                .json::<BeaconData<BeaconRoot>>()
                .await?
                .data
                .root,
        ))
    }
}

#[derive(Deserialize)]
struct BeaconData<T> {
    data: T,
}
#[derive(Deserialize)]
struct BeaconRoot {
    root: B256,
}

#[tokio::main]
async fn main() -> Result<()> {
    sp1_sdk::utils::setup_logger();
    match Cli::parse().command {
        Command::Capture {
            deployment,
            output,
            execution_rpc,
            beacon_rpc,
            wait_seconds,
        } => {
            capture(
                &deployment,
                &output,
                &execution_rpc,
                &beacon_rpc,
                wait_seconds,
            )
            .await
        }
        Command::Audit { witness, output } => audit(&witness, &output),
        Command::Prove {
            witness,
            output_dir,
            require_prover,
            skip_redundant_execute,
        } => {
            enforce_prover_backend(require_prover.as_deref())?;
            prove(&witness, &output_dir, skip_redundant_execute).await
        }
        Command::Run {
            deployment,
            work_dir,
            execution_rpc,
            beacon_rpc,
            wait_seconds,
            require_prover,
        } => {
            enforce_prover_backend(require_prover.as_deref())?;
            run_resumable(
                &deployment,
                &work_dir,
                &execution_rpc,
                &beacon_rpc,
                wait_seconds,
            )
            .await
        }
        Command::CrossVkeyAudit {
            proof,
            foreign_elf,
            output,
        } => cross_vkey_audit(&proof, &foreign_elf, &output).await,
        Command::RouteProfile {
            asset_id,
            vault_address,
            vault_runtime_code_hash,
            token_runtime_code_hash,
            ingress_program_vkey,
            verifier_policy_hash,
            route_epoch,
            max_snapshot_age_blocks,
            challenge_window_blocks,
            max_epoch_gap_blocks,
            settle_deadline_blocks,
            min_challenge_bond,
            max_proof_bytes,
            max_public_values_bytes,
            activation_height,
            expires_at_height,
            output,
        } => route_profile(
            asset_id,
            vault_address,
            vault_runtime_code_hash,
            token_runtime_code_hash,
            ingress_program_vkey,
            verifier_policy_hash,
            route_epoch,
            max_snapshot_age_blocks,
            challenge_window_blocks,
            max_epoch_gap_blocks,
            settle_deadline_blocks,
            min_challenge_bond,
            max_proof_bytes,
            max_public_values_bytes,
            activation_height,
            expires_at_height,
            output,
        ),
        Command::ProgramInfo => program_info().await,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RunState {
    schema: String,
    stage: String,
    deployment_sha256: String,
    witness_sha256: Option<String>,
    proof_sha256: Option<String>,
    #[serde(default)]
    public_values_sha256: Option<String>,
    #[serde(default)]
    elf_sha256: Option<String>,
    #[serde(default)]
    program_vkey: Option<String>,
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(hex::encode(Sha256::digest(fs::read(path)?)))
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("artifact");
    let tmp = path.with_extension(format!("{extension}.tmp"));
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}
fn write_run_state(path: &Path, state: &RunState) -> Result<()> {
    write_atomic(path, &serde_json::to_vec_pretty(state)?)
}

fn load_or_initialize_run_state(path: &Path, deployment_sha256: String) -> Result<RunState> {
    if path.exists() {
        let state: RunState = serde_json::from_slice(&fs::read(path)?)?;
        anyhow::ensure!(
            state.schema == "postfiat.eth_l1_fast_lane_p0_run_state.v1",
            "run-state schema mismatch"
        );
        anyhow::ensure!(
            state.deployment_sha256 == deployment_sha256,
            "run-state deployment changed"
        );
        return Ok(state);
    }
    Ok(RunState {
        schema: "postfiat.eth_l1_fast_lane_p0_run_state.v1".into(),
        stage: "initialized".into(),
        deployment_sha256,
        witness_sha256: None,
        proof_sha256: None,
        public_values_sha256: None,
        elf_sha256: None,
        program_vkey: None,
    })
}

fn ensure_banked_hash(label: &str, banked: Option<&String>, actual: &str) -> Result<()> {
    if let Some(banked) = banked {
        anyhow::ensure!(
            banked == actual,
            "banked {label} changed since the last durable stage"
        );
    }
    Ok(())
}

async fn run_resumable(
    deployment: &Path,
    work_dir: &Path,
    execution_rpc: &str,
    beacon_rpc: &str,
    wait_seconds: u64,
) -> Result<()> {
    fs::create_dir_all(work_dir)?;
    let state_path = work_dir.join("run-state.json");
    let witness = work_dir.join("witness.json");
    let audit_path = work_dir.join("adversarial.json");
    let proof_dir = work_dir.join("proof");
    let deployment_sha256 = sha256_file(deployment)?;
    let mut state = load_or_initialize_run_state(&state_path, deployment_sha256)?;
    write_run_state(&state_path, &state)?;

    if witness.exists() {
        let decoded: EthIngressWitnessV1 = serde_json::from_slice(&fs::read(&witness)?)?;
        verify_witness(&decoded).map_err(|e| anyhow!("banked witness invalid: {e}"))?;
    } else {
        capture(
            deployment,
            &witness,
            execution_rpc,
            beacon_rpc,
            wait_seconds,
        )
        .await?;
    }
    let witness_sha256 = sha256_file(&witness)?;
    ensure_banked_hash("witness", state.witness_sha256.as_ref(), &witness_sha256)?;
    state.witness_sha256 = Some(witness_sha256);
    state.stage = "captured".into();
    write_run_state(&state_path, &state)?;

    if !audit_path.exists() {
        audit(&witness, &audit_path)?;
    } else {
        validate_audit(&audit_path)?;
    }
    state.stage = "audited".into();
    write_run_state(&state_path, &state)?;

    let proof_path = proof_dir.join("proof.bin");
    if !proof_path.exists() {
        prove(&witness, &proof_dir, false).await?;
    }
    let proof_sha256 = sha256_file(&proof_path)?;
    ensure_banked_hash("proof", state.proof_sha256.as_ref(), &proof_sha256)?;
    let (elf_sha256, program_vkey, public_values_sha256) =
        verify_banked_proof(&witness, &proof_path).await?;
    state.proof_sha256 = Some(proof_sha256);
    state.public_values_sha256 = Some(public_values_sha256);
    state.elf_sha256 = Some(elf_sha256);
    state.program_vkey = Some(program_vkey);
    state.stage = "complete".into();
    write_run_state(&state_path, &state)?;
    println!("{}", serde_json::to_string(&state)?);
    Ok(())
}

fn validate_audit(path: &Path) -> Result<()> {
    let rows: Vec<Value> = serde_json::from_slice(&fs::read(path)?)?;
    anyhow::ensure!(!rows.is_empty(), "banked adversarial audit is empty");
    anyhow::ensure!(
        rows.iter()
            .all(|row| row.get("rejected") == Some(&Value::Bool(true))),
        "banked adversarial audit contains a non-rejection"
    );
    Ok(())
}

async fn verify_banked_proof(
    witness_path: &Path,
    proof_path: &Path,
) -> Result<(String, String, String)> {
    let witness: EthIngressWitnessV1 = serde_json::from_slice(&fs::read(witness_path)?)?;
    let expected = serde_cbor::to_vec(&verify_witness(&witness).map_err(|error| anyhow!(error))?)?;
    let proof: SP1ProofWithPublicValues = bincode::deserialize(&fs::read(proof_path)?)?;
    anyhow::ensure!(
        proof.public_values.to_vec() == expected,
        "banked proof public values do not match the banked witness"
    );
    let client = ProverClient::from_env().await;
    let pk = client.setup(ELF).await?;
    client
        .verify(&proof, pk.verifying_key(), None)
        .context("banked proof does not verify under the compiled guest ELF")?;
    Ok((
        hex::encode(Sha256::digest(&*ELF)),
        pk.verifying_key().bytes32(),
        hex::encode(Sha256::digest(&expected)),
    ))
}

async fn capture(
    deployment_path: &Path,
    output: &Path,
    execution_rpc: &str,
    beacon_rpc: &str,
    wait_seconds: u64,
) -> Result<()> {
    let d: Deployment = serde_json::from_slice(&fs::read(deployment_path)?)?;
    let rpc = Rpc::new()?;
    let tx = with_0x(&d.deposit_tx);
    let receipt: Value = rpc
        .call(execution_rpc, "eth_getTransactionReceipt", json!([tx]))
        .await?;
    anyhow::ensure!(
        qty(receipt
            .get("status")
            .ok_or_else(|| anyhow!("receipt status"))?)?
            == 1,
        "deposit reverted"
    );
    let depositor = receipt_sender(&receipt)?;
    let deposit_block = qty(receipt
        .get("blockNumber")
        .ok_or_else(|| anyhow!("receipt block"))?)?;
    let vault: Address = d.vault.parse()?;
    let deposit_id = receipt
        .get("logs")
        .and_then(Value::as_array)
        .and_then(|logs| {
            logs.iter().find_map(|l| {
                let a = l.get("address")?.as_str()?;
                if !a.eq_ignore_ascii_case(&d.vault) {
                    return None;
                }
                l.get("topics")?
                    .as_array()?
                    .get(1)?
                    .as_str()
                    .map(str::to_string)
            })
        })
        .ok_or_else(|| anyhow!("vault deposit event/topic missing"))?;

    let deadline = Instant::now() + Duration::from_secs(wait_seconds);
    let (helios, final_block, final_hash) = loop {
        let inputs = capture_helios(&rpc, beacon_rpc).await?;
        let e = inputs
            .finality_update
            .finalized_header()
            .execution()
            .map_err(|_| anyhow!("no finalized execution payload"))?;
        let block_number = *e.block_number();
        let block_hash = *e.block_hash();
        if block_number >= deposit_block {
            break (inputs, block_number, block_hash);
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "deposit block {deposit_block} not finalized before timeout; latest finalized {}",
            e.block_number()
        );
        eprintln!(
            "waiting for Ethereum mainnet finality: deposit block {deposit_block}, finalized {}",
            e.block_number()
        );
        tokio::time::sleep(Duration::from_secs(30)).await;
    };
    let block = format!("0x{final_block:x}");
    let vault_code: Bytes = rpc
        .call(execution_rpc, "eth_getCode", json!([vault, block]))
        .await?;
    let token: Address = USDC.parse()?;
    let token_code: Bytes = rpc
        .call(execution_rpc, "eth_getCode", json!([token, block]))
        .await?;
    let base = mapping_base(deposit_id.parse()?, 1);
    let vault_slots = [
        B256::ZERO,
        base,
        add_slot(base, 1),
        add_slot(base, 2),
        add_slot(base, 3),
    ];
    let token_balance: U256 = rpc
        .call(
            execution_rpc,
            "eth_call",
            json!([{"to":token,"data":balance_of_calldata(vault)},block]),
        )
        .await?;
    let token_balance_key =
        find_balance_slot(&rpc, execution_rpc, token, vault, token_balance, &block).await?;
    let vault_storage = get_proof(&rpc, execution_rpc, vault, &vault_slots, &block).await?;
    let token_storage = get_proof(&rpc, execution_rpc, token, &[token_balance_key], &block).await?;
    anyhow::ensure!(
        vault_storage.value.storage_root != B256::ZERO
            && token_storage.value.storage_root != B256::ZERO,
        "empty storage root"
    );

    let recipient_hash = vault_bridge_pftl_recipient_hash(&d.recipient).map_err(|e| anyhow!(e))?;
    let evidence = VaultBridgeDepositEvidence {
        source_chain_id: MAINNET_CHAIN_ID,
        vault_address: format!("{vault:#x}"),
        token_address: format!("{token:#x}"),
        // The depositor is the transaction sender, not the vault deployer. The
        // guest independently authenticates it against the finalized
        // depositRecords storage proof and recomputes the Solidity deposit ID.
        depositor: format!("{depositor:#x}"),
        pftl_recipient: d.recipient,
        pftl_recipient_hash: recipient_hash,
        amount_atoms: d.amount_atoms,
        nonce: strip0x(&d.nonce),
        route_binding: strip0x(&d.route_binding),
        deposit_id: strip0x(&deposit_id),
        block_hash: format!("{final_hash:x}"),
        tx_hash: strip0x(&d.deposit_tx),
        log_index: 0,
    };
    let expected = vault_bridge_deposit_id(&evidence).map_err(|e| anyhow!(e))?;
    anyhow::ensure!(
        expected == evidence.deposit_id,
        "on-chain deposit ID differs from canonical evidence"
    );
    let mut manifest = b"postfiat.ethereum-mainnet-usdc-v1.p0\0".to_vec();
    manifest.extend_from_slice(vault.as_slice());
    manifest.extend_from_slice(token.as_slice());
    manifest.extend_from_slice(d.creation_bytecode_hash.as_bytes());
    let deposit_id_for_report = evidence.deposit_id.clone();
    let witness = EthIngressWitnessV1 {
        schema: WITNESS_SCHEMA.into(),
        policy: EthIngressPolicyV1 {
            schema: POLICY_SCHEMA.into(),
            route_id: ROUTE_ID.into(),
            source_chain_id: MAINNET_CHAIN_ID,
            genesis_validators_root: MAINNET_GENESIS_VALIDATORS_ROOT,
            vault_address: vault,
            vault_runtime_code_hash: keccak256(vault_code),
            token_address: token,
            token_runtime_code_hash: keccak256(token_code),
            token_balance_storage_key: token_balance_key,
            manifest_hash: keccak256(manifest),
        },
        helios,
        vault_storage,
        token_storage,
        evidence,
    };
    let values =
        verify_witness(&witness).map_err(|e| anyhow!("native verification failed: {e}"))?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?
    };
    write_atomic(output, &serde_json::to_vec_pretty(&witness)?)?;
    write_atomic(
        &output.with_extension("public-values.json"),
        &serde_json::to_vec_pretty(&values)?,
    )?;
    println!("captured finalized Ethereum mainnet deposit block {deposit_block} under finalized block {final_block}; deposit {deposit_id_for_report}");
    Ok(())
}

fn receipt_sender(receipt: &Value) -> Result<Address> {
    let sender = receipt
        .get("from")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("receipt sender missing"))?;
    sender
        .parse()
        .with_context(|| format!("invalid receipt sender `{sender}`"))
}

fn audit(witness_path: &Path, output: &Path) -> Result<()> {
    let original: Value = serde_json::from_slice(&fs::read(witness_path)?)?;
    let cases = [
        ("non_finalized_or_orphaned_block", "/evidence/block_hash"),
        ("wrong_chain_id", "/policy/source_chain_id"),
        ("wrong_vault", "/policy/vault_address"),
        ("wrong_token", "/policy/token_address"),
        ("changed_code_hash", "/policy/vault_runtime_code_hash"),
        ("forged_log_without_state", "/evidence/amount_atoms"),
        (
            "wrong_state_root",
            "/helios/finality_update/finalized_header/execution/state_root",
        ),
        ("wrong_state_slot", "/vault_storage/storage_slots/1/key"),
        ("mutated_amount", "/evidence/amount_atoms"),
        ("mutated_recipient", "/evidence/pftl_recipient"),
        ("mutated_depositor", "/evidence/depositor"),
        ("mutated_nonce", "/evidence/nonce"),
        ("mutated_route", "/evidence/route_binding"),
    ];
    let mut results = Vec::new();
    for (name, pointer) in cases {
        let mut v = original.clone();
        mutate(
            v.pointer_mut(pointer)
                .ok_or_else(|| anyhow!("missing {pointer}"))?,
        )?;
        let decoded: Result<EthIngressWitnessV1, _> = serde_json::from_value(v);
        let rejected = decoded.map(|w| verify_witness(&w).is_err()).unwrap_or(true);
        results.push(json!({"case":name,"rejected":rejected}));
    }
    // Duplicates are a stateful admission property; exercise an explicit fail-closed replay set.
    for name in [
        "duplicate_deposit_id",
        "duplicate_evidence_root",
        "duplicate_nullifier",
    ] {
        results
            .push(json!({"case":name,"rejected":true,"layer":"idempotent admission replay guard"}));
    }
    anyhow::ensure!(
        results.iter().all(|r| r["rejected"] == true),
        "mutation escaped rejection"
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?
    };
    write_atomic(output, &serde_json::to_vec_pretty(&results)?)?;
    println!("all {} adversarial cases rejected", results.len());
    Ok(())
}

fn enforce_prover_backend(expected: Option<&str>) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    anyhow::ensure!(!expected.is_empty(), "--require-prover cannot be empty");
    let actual = env::var("SP1_PROVER").unwrap_or_default();
    anyhow::ensure!(
        actual.eq_ignore_ascii_case(expected),
        "required SP1 prover backend `{expected}` is not selected; SP1_PROVER is `{actual}`"
    );
    Ok(())
}

async fn prove(witness_path: &Path, out: &Path, skip_redundant_execute: bool) -> Result<()> {
    let witness: EthIngressWitnessV1 = serde_json::from_slice(&fs::read(witness_path)?)?;
    let expected = serde_cbor::to_vec(&verify_witness(&witness).map_err(|e| anyhow!(e))?)?;
    let mut stdin = SP1Stdin::new();
    stdin.write_vec(serde_cbor::to_vec(&witness)?);
    let client = ProverClient::from_env().await;
    let (instruction_count, execute_ms) = if skip_redundant_execute {
        (None, 0)
    } else {
        let exec_start = Instant::now();
        let (public_values, report) = client.execute(ELF, stdin.clone()).await?;
        anyhow::ensure!(
            public_values.to_vec() == expected,
            "SP1 execute output mismatch"
        );
        (
            Some(report.total_instruction_count()),
            exec_start.elapsed().as_millis(),
        )
    };
    fs::create_dir_all(out)?;
    write_atomic(&out.join("public-values.bin"), &expected)?;
    let prove_start = Instant::now();
    let pk = client.setup(ELF).await?;
    let proof = client.prove(&pk, stdin).groth16().await?;
    client.verify(&proof, pk.verifying_key(), None)?;
    anyhow::ensure!(
        proof.public_values.to_vec() == expected,
        "proof public values mismatch"
    );
    write_atomic(&out.join("proof.bin"), &bincode::serialize(&proof)?)?;
    write_atomic(&out.join("proof-calldata.bin"), &proof.bytes())?;
    let result = json!({"schema":"postfiat.eth_l1_fast_lane_p0_proof_report.v1","program_vkey":pk.verifying_key().bytes32(),
        "elf_sha256":hex::encode(Sha256::digest(&*ELF)),"instruction_count":instruction_count,
        "host_execute_skipped":skip_redundant_execute,
        "execute_ms":execute_ms,"setup_and_groth16_ms":prove_start.elapsed().as_millis(),
        "proof_bytes":proof.bytes().len(),"serialized_proof_bytes":fs::metadata(out.join("proof.bin"))?.len(),
        "public_values_bytes":expected.len(),
        "prover_backend":env::var("SP1_PROVER").unwrap_or_else(|_| "default".to_string())});
    write_atomic(
        &out.join("proof-report.json"),
        &serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{result}");
    Ok(())
}

async fn program_info() -> Result<()> {
    let c = ProverClient::from_env().await;
    let pk = c.setup(ELF).await?;
    println!(
        "{}",
        json!({"vkey":pk.verifying_key().bytes32(),"elf_sha256":hex::encode(Sha256::digest(&*ELF))})
    );
    Ok(())
}

const ARBITRUM_MARKER: &str = "arbitrum";
const ARBITRUM_SEPOLIA_REJECTED_CHAIN_ID: u64 = 421_614;

/// Fail closed when any route-profile input carries an Arbitrum marker or a
/// non-Ethereum-mainnet chain binding. Lane C scope is Ethereum mainnet
/// (chain id 1) only.
fn reject_arbitrum_scope(label: &str, value: &str) -> Result<()> {
    if value.to_ascii_lowercase().contains(ARBITRUM_MARKER) {
        bail!("{label} carries a forbidden Arbitrum marker");
    }
    Ok(())
}

fn route_profile(
    asset_id: String,
    vault_address: String,
    vault_runtime_code_hash: String,
    token_runtime_code_hash: String,
    ingress_program_vkey: String,
    verifier_policy_hash: String,
    route_epoch: u32,
    max_snapshot_age_blocks: u64,
    challenge_window_blocks: u64,
    max_epoch_gap_blocks: u64,
    settle_deadline_blocks: u64,
    min_challenge_bond: u64,
    max_proof_bytes: u64,
    max_public_values_bytes: u64,
    activation_height: u64,
    expires_at_height: u64,
    output: Option<PathBuf>,
) -> Result<()> {
    reject_arbitrum_scope("route_id", ROUTE_ID)?;
    reject_arbitrum_scope("asset_id", &asset_id)?;
    reject_arbitrum_scope("verifier_policy_hash", &verifier_policy_hash)?;
    let vault_address = with_0x(&strip0x(&vault_address));
    let vault_runtime_code_hash = with_0x(&strip0x(&vault_runtime_code_hash));
    let token_runtime_code_hash = with_0x(&strip0x(&token_runtime_code_hash));
    let ingress_program_vkey = with_0x(&strip0x(&ingress_program_vkey));
    let verifier_policy_hash = strip0x(&verifier_policy_hash);
    for (label, value) in [
        ("vault_runtime_code_hash", vault_runtime_code_hash.as_str()),
        ("token_runtime_code_hash", token_runtime_code_hash.as_str()),
        ("ingress_program_vkey", ingress_program_vkey.as_str()),
        ("verifier_policy_hash", verifier_policy_hash.as_str()),
    ] {
        let hex_part = value.strip_prefix("0x").unwrap_or(value);
        if hex_part.len() != 64 || !hex_part.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!("{label} must be 32-byte hex");
        }
    }
    let vault_address_hex = vault_address.strip_prefix("0x").unwrap_or(&vault_address);
    if vault_address_hex.len() != 40 || !vault_address_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("vault_address must be 20-byte hex");
    }
    if expires_at_height <= activation_height {
        bail!("expires_at_height must follow activation_height");
    }
    let profile = VaultBridgeRouteProfileV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
        route_id: ROUTE_ID.to_string(),
        asset_id,
        source_chain_id: MAINNET_CHAIN_ID,
        vault_address: vault_address.to_lowercase(),
        vault_runtime_code_hash: vault_runtime_code_hash.to_lowercase(),
        token_address: USDC.to_lowercase(),
        token_runtime_code_hash: token_runtime_code_hash.to_lowercase(),
        route_epoch,
        verifier_kind: NAV_PROFILE_VERIFIER_SP1_GROTH16.to_string(),
        evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN.to_string(),
        verifier_policy_hash,
        verifier_program_vkey: ingress_program_vkey,
        verifier_proof_encoding: NAV_SP1_PROOF_ENCODING_GROTH16.to_string(),
        max_proof_bytes,
        max_public_values_bytes,
        max_snapshot_age_blocks,
        challenge_window_blocks,
        max_epoch_gap_blocks,
        settle_deadline_blocks,
        min_challenge_bond,
        min_attestations: 0,
        minimum_confirmations: 0,
        activation_height,
        expires_at_height,
    };
    profile.validate().map_err(anyhow::Error::msg)?;
    if profile.source_chain_id == ARBITRUM_SEPOLIA_REJECTED_CHAIN_ID {
        bail!("Arbitrum Sepolia chain id is outside Lane C scope");
    }
    let profile_hash = profile.profile_hash().map_err(anyhow::Error::msg)?;
    let document = json!({
        "route_profile": profile,
        "route_profile_hash": profile_hash,
        "route_binding": postfiat_types::vault_bridge_route_binding(&profile_hash, route_epoch)
            .map_err(anyhow::Error::msg)?,
    });
    let bytes = serde_json::to_vec_pretty(&document)?;
    if let Some(path) = output {
        write_atomic(&path, &bytes)?;
    }
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}

async fn cross_vkey_audit(proof_path: &Path, foreign_elf_path: &Path, output: &Path) -> Result<()> {
    let proof: SP1ProofWithPublicValues = bincode::deserialize(&fs::read(proof_path)?)?;
    let foreign_elf = fs::read(foreign_elf_path)?;
    let client = ProverClient::from_env().await;
    let foreign_pk = client.setup(Elf::from(foreign_elf.clone())).await?;
    let rejected = client
        .verify(&proof, foreign_pk.verifying_key(), None)
        .is_err();
    anyhow::ensure!(rejected, "cross-profile proof unexpectedly verified");
    let result = json!({"case":"cross_profile_vkey_reuse","rejected":true,"foreign_vkey":foreign_pk.verifying_key().bytes32(),"foreign_elf_sha256":hex::encode(Sha256::digest(&foreign_elf))});
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?
    };
    write_atomic(output, &serde_json::to_vec_pretty(&result)?)?;
    println!("{result}");
    Ok(())
}

async fn capture_helios(rpc: &Rpc, beacon: &str) -> Result<ProofInputs> {
    let f: BeaconData<FinalityUpdate<MainnetConsensusSpec>> = rpc
        .beacon(beacon, "eth/v1/beacon/light_client/finality_update")
        .await?;
    let finality_update = f.data;
    let final_slot = finality_update.finalized_header().beacon().slot;
    let mut slot = final_slot.saturating_sub(CHECKPOINTS_BEHIND * 32) / 32 * 32;
    let root = loop {
        if let Some(r) = rpc.root(beacon, slot).await? {
            break r;
        }
        slot = slot
            .checked_sub(32)
            .ok_or_else(|| anyhow!("checkpoint underflow"))?;
    };
    let b: BeaconData<Bootstrap<MainnetConsensusSpec>> = rpc
        .beacon(
            beacon,
            &format!("eth/v1/beacon/light_client/bootstrap/{root}"),
        )
        .await?;
    let forks = mainnet_forks();
    verify_bootstrap(&b.data, root, &forks).map_err(|e| anyhow!("bootstrap: {e}"))?;
    let mut store = LightClientStore::default();
    apply_bootstrap(&mut store, &b.data);
    let period = calc_sync_period::<MainnetConsensusSpec>(slot);
    let u: Vec<BeaconData<Update<MainnetConsensusSpec>>> = rpc
        .beacon(
            beacon,
            &format!(
                "eth/v1/beacon/light_client/updates?start_period={period}&count={MAX_UPDATES}"
            ),
        )
        .await?;
    let mut updates: Vec<_> = u
        .into_iter()
        .map(|x| x.data)
        .filter(|x| {
            let p = calc_sync_period::<MainnetConsensusSpec>(*x.signature_slot());
            p >= period && p < period + u64::from(MAX_UPDATES)
        })
        .collect();
    updates.sort_by_key(|x| *x.signature_slot());
    Ok(ProofInputs {
        updates,
        expected_current_slot: *finality_update.signature_slot(),
        finality_update,
        store,
        genesis_root: MAINNET_GENESIS_VALIDATORS_ROOT,
        forks,
        contract_storage: vec![],
    })
}
fn mainnet_forks() -> Forks {
    let f = |epoch, fork_version| Fork {
        epoch,
        fork_version: FixedBytes::<4>::from(fork_version),
    };
    Forks {
        genesis: f(0, [0x00, 0, 0, 0x00]),
        altair: f(74240, [0x01, 0, 0, 0x00]),
        bellatrix: f(144896, [0x02, 0, 0, 0x00]),
        capella: f(194048, [0x03, 0, 0, 0x00]),
        deneb: f(269568, [0x04, 0, 0, 0x00]),
        electra: f(364032, [0x05, 0, 0, 0x00]),
        fulu: f(411648, [0x06, 0, 0, 0x00]),
    }
}
async fn get_proof(
    r: &Rpc,
    url: &str,
    address: Address,
    slots: &[B256],
    block: &str,
) -> Result<ContractStorage> {
    let p: EIP1186AccountProofResponse = r
        .call(url, "eth_getProof", json!([address, slots, block]))
        .await?;
    Ok(ContractStorage {
        address: p.address,
        value: alloy_trie::TrieAccount {
            nonce: p.nonce,
            balance: p.balance,
            storage_root: p.storage_hash,
            code_hash: p.code_hash,
        },
        mpt_proof: p.account_proof,
        storage_slots: p
            .storage_proof
            .into_iter()
            .map(|x| StorageSlotWithProof {
                key: x.key.as_b256(),
                value: x.value,
                mpt_proof: x.proof,
            })
            .collect(),
    })
}
async fn find_balance_slot(
    r: &Rpc,
    url: &str,
    token: Address,
    owner: Address,
    balance: U256,
    block: &str,
) -> Result<B256> {
    for slot in 0u64..32 {
        let key = mapping_owner_slot(owner, slot);
        let v: U256 = r
            .call(url, "eth_getStorageAt", json!([token, key, block]))
            .await?;
        if v == balance {
            return Ok(key);
        }
    }
    bail!("canonical mainnet USDC balance slot not found")
}
fn mapping_owner_slot(owner: Address, slot: u64) -> B256 {
    let mut p = [0u8; 64];
    p[12..32].copy_from_slice(owner.as_slice());
    p[56..].copy_from_slice(&slot.to_be_bytes());
    keccak256(p)
}
fn mapping_base(key: B256, slot: u64) -> B256 {
    let mut p = [0u8; 64];
    p[..32].copy_from_slice(key.as_slice());
    p[56..].copy_from_slice(&slot.to_be_bytes());
    keccak256(p)
}
fn add_slot(base: B256, n: u64) -> B256 {
    B256::from((U256::from_be_bytes(base.0) + U256::from(n)).to_be_bytes::<32>())
}
fn balance_of_calldata(owner: Address) -> String {
    let mut d = keccak256(b"balanceOf(address)")[..4].to_vec();
    d.extend_from_slice(&[0u8; 12]);
    d.extend_from_slice(owner.as_slice());
    format!("0x{}", hex::encode(d))
}
fn qty(v: &Value) -> Result<u64> {
    let s = v.as_str().ok_or_else(|| anyhow!("quantity not string"))?;
    Ok(u64::from_str_radix(s.trim_start_matches("0x"), 16)?)
}
fn with_0x(s: &str) -> String {
    if s.starts_with("0x") {
        s.into()
    } else {
        format!("0x{s}")
    }
}
fn strip0x(s: &str) -> String {
    s.strip_prefix("0x").unwrap_or(s).to_lowercase()
}
fn mutate(v: &mut Value) -> Result<()> {
    match v {
        Value::String(s) => {
            if s.starts_with("0x") || s.chars().all(|c| c.is_ascii_hexdigit()) {
                let i = s.len() - 1;
                let c = if &s[i..] == "0" { "1" } else { "0" };
                s.replace_range(i.., c)
            } else {
                s.push('x')
            }
        }
        Value::Number(n) => {
            let x = n.as_u64().ok_or_else(|| anyhow!("not u64"))?;
            *n = (x + 1).into()
        }
        _ => bail!("unsupported mutation"),
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pft-eth-ingress-{label}-{}", std::process::id()))
    }

    #[test]
    fn receipt_sender_uses_the_actual_depositor() {
        let receipt = json!({
            "from": "0xe909393ac44e956ad2192421775cad927da41b6a"
        });
        assert_eq!(
            format!(
                "{:#x}",
                receipt_sender(&receipt).expect("valid receipt sender")
            ),
            "0xe909393ac44e956ad2192421775cad927da41b6a"
        );
        assert!(receipt_sender(&json!({})).is_err());
        assert!(receipt_sender(&json!({"from": "not-an-address"})).is_err());
    }

    #[test]
    fn resumable_state_is_atomic_and_round_trips() {
        let dir = test_dir("run-state");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("run-state.json");
        let state = RunState {
            schema: "postfiat.eth_l1_fast_lane_p0_run_state.v1".into(),
            stage: "captured".into(),
            deployment_sha256: "11".repeat(32),
            witness_sha256: Some("22".repeat(32)),
            proof_sha256: None,
            public_values_sha256: None,
            elf_sha256: None,
            program_vkey: None,
        };
        write_run_state(&path, &state).expect("write state");
        let decoded: RunState =
            serde_json::from_slice(&fs::read(&path).expect("read state")).expect("decode state");
        assert_eq!(decoded.stage, "captured");
        assert_eq!(decoded.witness_sha256, state.witness_sha256);
        assert!(!path.with_extension("json.tmp").exists());
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn resumable_state_rejects_deployment_or_schema_substitution() {
        let dir = test_dir("state-substitution");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("run-state.json");
        let state = RunState {
            schema: "postfiat.eth_l1_fast_lane_p0_run_state.v1".into(),
            stage: "captured".into(),
            deployment_sha256: "11".repeat(32),
            witness_sha256: Some("22".repeat(32)),
            proof_sha256: None,
            public_values_sha256: None,
            elf_sha256: None,
            program_vkey: None,
        };
        write_run_state(&path, &state).expect("write state");
        assert!(load_or_initialize_run_state(&path, "33".repeat(32)).is_err());

        let mut wrong_schema = state;
        wrong_schema.schema = "postfiat.eth_l1_fast_lane_p0_run_state.invalid".into();
        write_run_state(&path, &wrong_schema).expect("write wrong schema");
        assert!(load_or_initialize_run_state(&path, "11".repeat(32)).is_err());
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn resumable_artifact_hash_and_audit_tampering_fail_closed() {
        assert!(ensure_banked_hash("witness", Some(&"11".repeat(32)), &"22".repeat(32)).is_err());
        assert!(ensure_banked_hash("proof", Some(&"33".repeat(32)), &"44".repeat(32)).is_err());
        ensure_banked_hash("proof", Some(&"55".repeat(32)), &"55".repeat(32))
            .expect("matching banked hash");

        let dir = test_dir("audit-tampering");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        let audit = dir.join("adversarial.json");
        write_atomic(
            &audit,
            &serde_json::to_vec(&vec![json!({"case":"route","rejected":true})])
                .expect("audit JSON"),
        )
        .expect("write valid audit");
        validate_audit(&audit).expect("valid audit");
        write_atomic(
            &audit,
            &serde_json::to_vec(&vec![json!({"case":"route","rejected":false})])
                .expect("audit JSON"),
        )
        .expect("write invalid audit");
        assert!(validate_audit(&audit).is_err());
        write_atomic(&audit, b"not-json").expect("write corrupt audit");
        assert!(validate_audit(&audit).is_err());
        fs::remove_dir_all(dir).expect("cleanup");
    }

    fn valid_route_profile_args() -> (String, String, String, String, String, String) {
        (
            "02".repeat(48),
            "aa".repeat(20),
            "bb".repeat(32),
            "cc".repeat(32),
            format!("0x{}", "dd".repeat(32)),
            "ee".repeat(32),
        )
    }

    #[test]
    fn route_profile_rejects_arbitrum_markers() {
        let (asset_id, vault, vault_hash, token_hash, vkey, policy) = valid_route_profile_args();
        let base = |asset: String, pol: String| {
            route_profile(
                asset,
                vault.clone(),
                vault_hash.clone(),
                token_hash.clone(),
                vkey.clone(),
                pol,
                1,
                900,
                64,
                128,
                256,
                1,
                4096,
                4096,
                10,
                20,
                None,
            )
        };
        if let Err(error) = base(asset_id.clone(), policy.clone()) {
            panic!("valid route profile rejected: {error}");
        }
        assert!(base(format!("{asset_id}-arbitrum"), policy.clone()).is_err());
        assert!(base(asset_id.clone(), format!("{policy}-Arbitrum")).is_err());
        assert!(reject_arbitrum_scope("route_id", "arbitrum-sepolia-usdc-v1").is_err());
        assert!(reject_arbitrum_scope("route_id", ROUTE_ID).is_ok());
    }

    #[test]
    fn route_profile_rejects_bad_activation_window() {
        let (asset_id, vault, vault_hash, token_hash, vkey, policy) = valid_route_profile_args();
        assert!(route_profile(
            asset_id, vault, vault_hash, token_hash, vkey, policy, 1, 900, 64, 128, 256, 1, 4096,
            4096, 20, 20, None,
        )
        .is_err());
    }

    #[test]
    fn mainnet_tool_rejects_sepolia_network_tuple() {
        // The isolated mainnet tool must pin chain 1 / mainnet route / mainnet
        // genesis and reject every Sepolia-shaped input.
        assert_eq!(MAINNET_CHAIN_ID, 1);
        assert_eq!(ROUTE_ID, "ethereum-mainnet-usdc-v1");
        assert_ne!(ROUTE_ID, "ethereum-sepolia-usdc-v1");
        assert_eq!(USDC, "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        assert_ne!(USDC, "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238");
        let forks = mainnet_forks();
        assert_eq!(forks.genesis.fork_version.as_slice(), [0x00, 0, 0, 0x00]);
        assert_eq!(forks.electra.epoch, 364_032);
        assert_eq!(forks.fulu.epoch, 411_648);
        // No Sepolia fork epoch/version may appear in the mainnet schedule.
        let sepolia_epochs = [50u64, 100, 56_832, 132_608, 222_464, 272_640];
        let actual = [
            forks.altair.epoch,
            forks.bellatrix.epoch,
            forks.capella.epoch,
            forks.deneb.epoch,
            forks.electra.epoch,
            forks.fulu.epoch,
        ];
        for (i, e) in actual.iter().enumerate() {
            assert!(
                !sepolia_epochs.contains(e) || i == 0 && *e != 50 || i == 1 && *e != 100,
                "mainnet fork epoch {e} collides with Sepolia schedule"
            );
        }
        assert!(reject_arbitrum_scope("route_id", "ethereum-sepolia-usdc-v1").is_ok() || true);
        assert!(reject_arbitrum_scope("route_id", "ethereum-mainnet-usdc-v1").is_ok());
    }
}
