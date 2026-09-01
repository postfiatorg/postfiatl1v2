use std::{fs, path::PathBuf, str::FromStr};

use alloy::{
    primitives::{keccak256, Address, Bytes, Log, LogData, B256, U256},
    rpc::types::Block,
};
use alloy_rlp::Encodable;
use alloy_sol_types::{sol, SolCall, SolEvent};
use anyhow::{anyhow, bail, Context, Result};
use arc_conformance::receipts::{build_receipt_inclusion, ReceiptBlockFixture, RpcReceipt};
use arc_conformance::{
    address_from_public_key, encode_hex, BlockFixture, CommitCertificate, GoldenFixture, Validator,
    ValidatorSetFixture, ARC_TESTNET_CHAIN_ID, ARC_VALIDATOR_REGISTRY,
};
use base64::Engine as _;
use clap::Args;
use pfusdc_arc_ingress_program::{
    validator_set_commitment, verify_arc_ingress_witness_v2, verify_arc_validator_transition_v1,
    ArcAccountProofV1, ArcCommitSignatureV1, ArcIngressWitnessV2, ArcRegisteredValidatorV1,
    ArcStorageProofV1, ArcValidatorRegistryProofV1, ArcValidatorV1, ARC_INGRESS_WITNESS_SCHEMA_V2,
    ERC1967_IMPLEMENTATION_SLOT, VALIDATOR_REGISTRY_STORAGE_BASE,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

const DEFAULT_RPC: &str = "https://rpc.testnet.arc.network";
const ARC_TESTNET_USDC: &str = "0x3600000000000000000000000000000000000000";
const ARC_NODE_COMMIT: &str = "66ad2d5aa6d9b41e8f689812004be4c7233a9e16";

sol! {
    #[derive(PartialEq, Eq)]
    enum ContractValidatorStatus { Unknown, Registered, Active }
    struct ContractValidator {
        ContractValidatorStatus status;
        bytes publicKey;
        uint64 votingPower;
    }
    function getActiveValidatorSet() external view
        returns (ContractValidator[] memory activeValidators);

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
}

#[derive(Debug, Clone, Args)]
pub struct ArcIngressCaptureArgs {
    /// Arc JSON-RPC endpoint.
    #[arg(long, default_value = DEFAULT_RPC)]
    pub rpc: String,
    /// Optional Arc JSON-RPC endpoint used only for arc_getCertificate.
    #[arg(long)]
    pub certificate_rpc: Option<String>,
    /// Transaction hash of an ERC20BridgeVaultV2 direct deposit.
    #[arg(long)]
    pub deposit_tx: String,
    /// Expected immutable route binding (32-byte hex).
    #[arg(long)]
    pub route_id: String,
    /// Expected deployed vault address.
    #[arg(long)]
    pub vault: String,
    /// Expected Arc system USDC address.
    #[arg(long, default_value = ARC_TESTNET_USDC)]
    pub token: String,
    /// Canonical proving-ready witness JSON.
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct ArcValidatorTransitionCaptureArgs {
    /// Arc JSON-RPC endpoint with historical eth_getProof support.
    #[arg(long, default_value = DEFAULT_RPC)]
    pub rpc: String,
    /// Arc execution height H whose post-state selects the signing set for H+1.
    #[arg(long)]
    pub height: u64,
    /// Reject the fixture unless the signing set changes between H and H+1.
    #[arg(long)]
    pub require_change: bool,
    /// Canonical authenticated transition fixture JSON.
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct ArcValidatorTransitionVerifyArgs {
    /// Authenticated transition fixture produced by arc-validator-transition-capture.
    #[arg(long)]
    pub fixture: PathBuf,
    /// Optional independent Arc RPC used to match the fixture to its public block header.
    #[arg(long)]
    pub header_rpc: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArcValidatorTransitionFixtureV1 {
    schema: String,
    arc_node_commit: String,
    arc_chain_id: u64,
    arc_block_height: u64,
    arc_block_hash: B256,
    arc_state_root: B256,
    validator_set_commitment_in: [u8; 32],
    validator_set_commitment_out: [u8; 32],
    validator_set_changed: bool,
    signing_validators: Vec<ArcValidatorV1>,
    next_validators: Vec<ArcValidatorV1>,
    validator_registry_proof: ArcValidatorRegistryProofV1,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetReceipt {
    #[serde(flatten)]
    receipt: RpcReceipt,
    block_number: String,
    block_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcAccountProof {
    address: String,
    balance: String,
    code_hash: String,
    nonce: String,
    storage_hash: String,
    account_proof: Vec<String>,
    storage_proof: Vec<RpcStorageProof>,
}

#[derive(Debug, Deserialize)]
struct RpcStorageProof {
    key: String,
    value: String,
    proof: Vec<String>,
}

#[derive(Clone)]
struct RpcClient {
    http: reqwest::Client,
}

impl RpcClient {
    fn new() -> Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .user_agent("postfiat-pfusdc-arc-witness/1")
                .build()?,
        })
    }

    async fn call<T: DeserializeOwned>(&self, url: &str, method: &str, params: Value) -> Result<T> {
        let response: Value = self
            .http
            .post(url)
            .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
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
}

pub async fn capture(args: ArcIngressCaptureArgs) -> Result<()> {
    let rpc = RpcClient::new()?;
    let chain_id_hex: String = rpc.call(&args.rpc, "eth_chainId", json!([])).await?;
    let chain_id = quantity(&chain_id_hex)?;
    anyhow::ensure!(
        chain_id == ARC_TESTNET_CHAIN_ID,
        "wrong Arc chain ID {chain_id}"
    );

    let target: TargetReceipt = rpc
        .call(
            &args.rpc,
            "eth_getTransactionReceipt",
            json!([args.deposit_tx]),
        )
        .await?;
    let height = quantity(&target.block_number)?;
    anyhow::ensure!(height > 0, "deposit cannot be in genesis block");
    let target_index = quantity(&target.receipt.transaction_index)?;
    let block: Block = rpc
        .call(
            &args.rpc,
            "eth_getBlockByHash",
            json!([target.block_hash, false]),
        )
        .await?;
    anyhow::ensure!(
        block.header.hash == B256::from_str(&target.block_hash)?,
        "RPC block hash mismatch"
    );
    anyhow::ensure!(
        block.header.inner.hash_slow() == block.header.hash,
        "canonical header RLP hash mismatch"
    );
    anyhow::ensure!(
        block.header.inner.number == height,
        "receipt and header heights differ"
    );
    let mut header_rlp = Vec::new();
    block.header.inner.encode(&mut header_rlp);

    let certificate_rpc = args.certificate_rpc.as_deref().unwrap_or(&args.rpc);
    let certificate: CommitCertificate = rpc
        .call(
            certificate_rpc,
            "arc_getCertificate",
            json!([format!("0x{height:x}")]),
        )
        .await?;
    let validators = capture_validators(&rpc, &args.rpc, height - 1).await?;
    let (next_validators, validator_registry_proof) =
        capture_validator_registry_proof(&rpc, &args.rpc, height).await?;
    let golden = GoldenFixture {
        schema: "arc-conformance-fixture-v1".to_owned(),
        chain_id,
        arc_node_commit: ARC_NODE_COMMIT.to_owned(),
        block: BlockFixture {
            number: height,
            hash: format!("{:#x}", block.header.hash),
            receipts_root: format!("{:#x}", block.header.inner.receipts_root),
        },
        certificate: certificate.clone(),
        validator_set: ValidatorSetFixture {
            queried_at_block: height - 1,
            validators: validators.clone(),
        },
    };
    arc_conformance::verify_fixture(&golden)
        .map_err(|error| anyhow!("Arc commit failed native verification: {error}"))?;

    let receipts: Vec<RpcReceipt> = rpc
        .call(
            &args.rpc,
            "eth_getBlockReceipts",
            json!([format!("0x{height:x}")]),
        )
        .await?;
    let receipt_block = ReceiptBlockFixture {
        number: height,
        hash: format!("{:#x}", block.header.hash),
        receipts_root: format!("{:#x}", block.header.inner.receipts_root),
    };
    let inclusion = build_receipt_inclusion(chain_id, &receipt_block, &receipts, target_index)
        .map_err(|error| anyhow!("build target receipt proof: {error}"))?;
    anyhow::ensure!(
        inclusion
            .transaction_hash
            .eq_ignore_ascii_case(&args.deposit_tx),
        "target transaction index resolves to a different hash"
    );

    let expected_route = fixed_hex::<32>(&args.route_id, "route_id")?;
    let expected_vault = Address::from_str(&args.vault).context("parse expected vault")?;
    let expected_token = Address::from_str(&args.token).context("parse expected token")?;
    let (deposit_log_index, event) = find_deposit_event(&target.receipt, expected_vault)?;
    anyhow::ensure!(
        event.routeBinding.0 == expected_route,
        "deposit route binding mismatch"
    );
    anyhow::ensure!(
        event.vault == expected_vault,
        "deposit event vault mismatch"
    );
    anyhow::ensure!(event.token == expected_token, "deposit token mismatch");
    anyhow::ensure!(
        u64::try_from(event.sourceChainId)? == chain_id,
        "deposit source chain mismatch"
    );
    let amount_atoms = u64::try_from(event.amount).context("deposit amount exceeds u64")?;

    let arc_validators = validators
        .iter()
        .map(|validator| {
            Ok(ArcValidatorV1 {
                address: fixed_hex(&validator.address, "validator.address")?,
                public_key: fixed_hex(&validator.public_key, "validator.public_key")?,
                voting_power: validator.voting_power,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let validator_commitment = validator_set_commitment(chain_id, &arc_validators)
        .map_err(|error| anyhow!("compute validator commitment: {}", error.code()))?;
    let next_validator_commitment = validator_set_commitment(chain_id, &next_validators)
        .map_err(|error| anyhow!("compute next validator commitment: {}", error.code()))?;
    let signatures = certificate
        .signatures
        .iter()
        .map(|signature| {
            Ok(ArcCommitSignatureV1 {
                address: fixed_hex(&signature.address, "signature.address")?,
                signature: base64::engine::general_purpose::STANDARD
                    .decode(&signature.signature)
                    .context("decode certificate signature")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let witness = ArcIngressWitnessV2 {
        schema: ARC_INGRESS_WITNESS_SCHEMA_V2.to_owned(),
        route_id: expected_route,
        arc_chain_id: chain_id,
        vault_address: expected_vault.0 .0,
        token_address: expected_token.0 .0,
        deposit_id: event.depositId.0,
        amount_atoms,
        pftl_recipient_hash: event.pftlRecipientHash.0,
        deposit_nonce: event.nonce.0,
        arc_block_hash: block.header.hash.0,
        arc_block_height: height,
        validator_set_commitment_in: validator_commitment,
        validator_set_commitment_out: next_validator_commitment,
        header_rlp,
        commit_round: certificate.round,
        validators: arc_validators,
        signatures,
        receipt_transaction_index: target_index,
        encoded_receipt: variable_hex(&inclusion.encoded_receipt, "encoded_receipt")?,
        receipt_proof_nodes: inclusion
            .proof_nodes
            .iter()
            .map(|node| variable_hex(node, "proof_node"))
            .collect::<Result<Vec<_>>>()?,
        deposit_log_index,
        next_validators,
        validator_registry_proof: Some(validator_registry_proof),
    };
    let public_values = verify_arc_ingress_witness_v2(&witness).map_err(|error| {
        anyhow!(
            "captured witness failed native verification: {}",
            error.code()
        )
    })?;
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serde_json::to_vec_pretty(&witness)?)?;
    println!(
        "captured Arc deposit {} at height {} (tx index {}, log {}, {} validators, {} signatures, {} public bytes) to {}",
        encode_hex(witness.deposit_id),
        height,
        target_index,
        deposit_log_index,
        witness.validators.len(),
        witness.signatures.len(),
        public_values.canonical_bytes().len(),
        args.output.display()
    );
    Ok(())
}

pub async fn capture_validator_transition(args: ArcValidatorTransitionCaptureArgs) -> Result<()> {
    anyhow::ensure!(args.height > 0, "transition height must be positive");
    let rpc = RpcClient::new()?;
    let chain_id_hex: String = rpc.call(&args.rpc, "eth_chainId", json!([])).await?;
    let chain_id = quantity(&chain_id_hex)?;
    anyhow::ensure!(
        chain_id == ARC_TESTNET_CHAIN_ID,
        "wrong Arc chain ID {chain_id}"
    );
    let block: Block = rpc
        .call(
            &args.rpc,
            "eth_getBlockByNumber",
            json!([format!("0x{:x}", args.height), false]),
        )
        .await?;
    anyhow::ensure!(
        block.header.inner.number == args.height,
        "RPC returned the wrong transition height"
    );
    anyhow::ensure!(
        block.header.inner.hash_slow() == block.header.hash,
        "canonical transition header RLP hash mismatch"
    );
    let signing_validators = capture_validators(&rpc, &args.rpc, args.height - 1).await?;
    let signing_validators = signing_validators
        .into_iter()
        .map(|validator| {
            Ok(ArcValidatorV1 {
                address: fixed_hex(&validator.address, "validator.address")?,
                public_key: fixed_hex(&validator.public_key, "validator.public_key")?,
                voting_power: validator.voting_power,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let (next_validators, proof) =
        capture_validator_registry_proof(&rpc, &args.rpc, args.height).await?;
    let changed = signing_validators != next_validators;
    if args.require_change {
        anyhow::ensure!(
            changed,
            "height {} does not change the signing validator set",
            args.height
        );
    }
    let commitment_in = validator_set_commitment(chain_id, &signing_validators)
        .map_err(|error| anyhow!("compute input validator commitment: {}", error.code()))?;
    let commitment_out = validator_set_commitment(chain_id, &next_validators)
        .map_err(|error| anyhow!("compute output validator commitment: {}", error.code()))?;
    verify_arc_validator_transition_v1(
        chain_id,
        &next_validators,
        commitment_out,
        &proof,
        block.header.inner.state_root,
    )
    .map_err(|error| {
        anyhow!(
            "transition proof failed native verification: {}",
            error.code()
        )
    })?;
    let fixture = ArcValidatorTransitionFixtureV1 {
        schema: "postfiat.pfusdc.arc_validator_transition_fixture.v1".to_owned(),
        arc_node_commit: ARC_NODE_COMMIT.to_owned(),
        arc_chain_id: chain_id,
        arc_block_height: args.height,
        arc_block_hash: block.header.hash,
        arc_state_root: block.header.inner.state_root,
        validator_set_commitment_in: commitment_in,
        validator_set_commitment_out: commitment_out,
        validator_set_changed: changed,
        signing_validators,
        next_validators,
        validator_registry_proof: proof,
    };
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serde_json::to_vec_pretty(&fixture)?)?;
    println!(
        "captured authenticated Arc validator state at height {} ({} -> {} signing validators, changed={}) to {}",
        args.height,
        fixture.signing_validators.len(),
        fixture.next_validators.len(),
        changed,
        args.output.display()
    );
    Ok(())
}

pub async fn verify_validator_transition(args: ArcValidatorTransitionVerifyArgs) -> Result<()> {
    let fixture: ArcValidatorTransitionFixtureV1 = serde_json::from_slice(
        &fs::read(&args.fixture)
            .with_context(|| format!("read transition fixture {}", args.fixture.display()))?,
    )
    .with_context(|| format!("decode transition fixture {}", args.fixture.display()))?;
    anyhow::ensure!(
        fixture.schema == "postfiat.pfusdc.arc_validator_transition_fixture.v1",
        "wrong transition fixture schema"
    );
    anyhow::ensure!(
        fixture.arc_node_commit == ARC_NODE_COMMIT,
        "transition fixture pins the wrong Arc node commit"
    );
    anyhow::ensure!(
        fixture.arc_chain_id == ARC_TESTNET_CHAIN_ID,
        "transition fixture pins the wrong Arc chain"
    );
    anyhow::ensure!(fixture.arc_block_height > 0, "transition height is zero");
    anyhow::ensure!(
        fixture.validator_set_changed == (fixture.signing_validators != fixture.next_validators),
        "validator_set_changed disagrees with the fixture sets"
    );
    let commitment_in = validator_set_commitment(fixture.arc_chain_id, &fixture.signing_validators)
        .map_err(|error| anyhow!("compute input validator commitment: {}", error.code()))?;
    anyhow::ensure!(
        commitment_in == fixture.validator_set_commitment_in,
        "input validator commitment mismatch"
    );
    let commitment_out =
        validator_set_commitment(fixture.arc_chain_id, &fixture.next_validators)
            .map_err(|error| anyhow!("compute output validator commitment: {}", error.code()))?;
    anyhow::ensure!(
        commitment_out == fixture.validator_set_commitment_out,
        "output validator commitment mismatch"
    );
    verify_arc_validator_transition_v1(
        fixture.arc_chain_id,
        &fixture.next_validators,
        fixture.validator_set_commitment_out,
        &fixture.validator_registry_proof,
        fixture.arc_state_root,
    )
    .map_err(|error| anyhow!("transition proof verification failed: {}", error.code()))?;

    let mut public_header_match = None;
    if let Some(header_rpc) = args.header_rpc.as_deref() {
        let rpc = RpcClient::new()?;
        let block: Block = rpc
            .call(
                header_rpc,
                "eth_getBlockByNumber",
                json!([format!("0x{:x}", fixture.arc_block_height), false]),
            )
            .await?;
        anyhow::ensure!(
            block.header.inner.number == fixture.arc_block_height
                && block.header.hash == fixture.arc_block_hash
                && block.header.inner.state_root == fixture.arc_state_root,
            "independent Arc RPC header does not match the fixture"
        );
        public_header_match = Some(true);
    }
    println!(
        "verified authenticated Arc validator state at height {} ({} -> {} signing validators, changed={}, public_header_match={})",
        fixture.arc_block_height,
        fixture.signing_validators.len(),
        fixture.next_validators.len(),
        fixture.validator_set_changed,
        public_header_match
            .map(|matched| matched.to_string())
            .unwrap_or_else(|| "not-requested".to_owned())
    );
    Ok(())
}

async fn capture_validators(rpc: &RpcClient, url: &str, at_height: u64) -> Result<Vec<Validator>> {
    let active = capture_active_validators_in_registry_order(rpc, url, at_height).await?;
    let mut validators = active
        .into_iter()
        .filter(|validator| validator.voting_power > 0)
        .map(|validator| Validator {
            address: encode_hex(validator.address),
            public_key: encode_hex(validator.public_key),
            voting_power: validator.voting_power,
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(!validators.is_empty(), "signing validator set is empty");
    validators.sort_by(|left, right| {
        right
            .voting_power
            .cmp(&left.voting_power)
            .then_with(|| left.address.cmp(&right.address))
    });
    Ok(validators)
}

async fn capture_active_validators_in_registry_order(
    rpc: &RpcClient,
    url: &str,
    at_height: u64,
) -> Result<Vec<ArcValidatorV1>> {
    let calldata = getActiveValidatorSetCall {}.abi_encode();
    let result: String = rpc
        .call(
            url,
            "eth_call",
            json!([{"to": ARC_VALIDATOR_REGISTRY, "data": encode_hex(calldata)}, format!("0x{at_height:x}")]),
        )
        .await?;
    let bytes = variable_hex(&result, "validator registry return")?;
    getActiveValidatorSetCall::abi_decode_returns(&bytes)
        .context("decode active validator set")?
        .into_iter()
        .map(|validator| {
            anyhow::ensure!(
                validator.status == ContractValidatorStatus::Active,
                "active validator array contains a non-active entry"
            );
            let public_key: [u8; 32] = validator
                .publicKey
                .as_ref()
                .try_into()
                .map_err(|_| anyhow!("active validator has a non-32-byte public key"))?;
            Ok(ArcValidatorV1 {
                address: address_from_public_key(&public_key),
                public_key,
                voting_power: validator.votingPower,
            })
        })
        .collect()
}

async fn capture_validator_registry_proof(
    rpc: &RpcClient,
    url: &str,
    at_height: u64,
) -> Result<(Vec<ArcValidatorV1>, ArcValidatorRegistryProofV1)> {
    let block = format!("0x{at_height:x}");
    let active_validators =
        capture_active_validators_in_registry_order(rpc, url, at_height).await?;
    anyhow::ensure!(
        !active_validators.is_empty(),
        "active validator set is empty"
    );

    let active_set_slot = add_to_slot(VALIDATOR_REGISTRY_STORAGE_BASE, 1)?;
    let active_count_rpc: String = rpc
        .call(
            url,
            "eth_getStorageAt",
            json!([ARC_VALIDATOR_REGISTRY, b256_hex(active_set_slot), block]),
        )
        .await?;
    let active_count = usize::try_from(parse_u256(&active_count_rpc)?)
        .context("active validator count exceeds usize")?;
    anyhow::ensure!(
        active_count == active_validators.len(),
        "registry array length and eth_call validator count differ"
    );

    let active_values_base = keccak256(active_set_slot);
    let mut registered = Vec::with_capacity(active_count);
    let mut storage_keys = vec![ERC1967_IMPLEMENTATION_SLOT, active_set_slot];
    for (index, validator) in active_validators.iter().enumerate() {
        let array_slot = add_to_slot(active_values_base, u64::try_from(index)?)?;
        let registration_rpc: String = rpc
            .call(
                url,
                "eth_getStorageAt",
                json!([ARC_VALIDATOR_REGISTRY, b256_hex(array_slot), block]),
            )
            .await?;
        let registration_id =
            u64::try_from(parse_u256(&registration_rpc)?).context("registration ID exceeds u64")?;
        anyhow::ensure!(registration_id > 0, "active registration ID is zero");
        registered.push(ArcRegisteredValidatorV1 {
            registration_id,
            validator: validator.clone(),
        });
        let validator_slot = mapping_slot(registration_id, VALIDATOR_REGISTRY_STORAGE_BASE);
        let public_key_slot = add_to_slot(validator_slot, 1)?;
        storage_keys.extend([
            array_slot,
            validator_slot,
            public_key_slot,
            keccak256(public_key_slot),
            add_to_slot(validator_slot, 2)?,
        ]);
    }
    storage_keys.sort_unstable();
    storage_keys.dedup();
    let registry_rpc: RpcAccountProof = rpc
        .call(
            url,
            "eth_getProof",
            json!([
                ARC_VALIDATOR_REGISTRY,
                storage_keys
                    .iter()
                    .copied()
                    .map(b256_hex)
                    .collect::<Vec<_>>(),
                block
            ]),
        )
        .await?;
    let implementation_storage = registry_rpc
        .storage_proof
        .iter()
        .find(|proof| {
            fixed_hex::<32>(&proof.key, "implementation proof key").ok()
                == Some(ERC1967_IMPLEMENTATION_SLOT.0)
        })
        .context("registry proof omitted ERC-1967 implementation slot")?;
    let implementation_word = parse_u256_word(&implementation_storage.value)?;
    anyhow::ensure!(
        implementation_word[..12] == [0; 12],
        "implementation slot has non-address high bytes"
    );
    let implementation = Address::from_slice(&implementation_word[12..]);
    let implementation_rpc: RpcAccountProof = rpc
        .call(url, "eth_getProof", json!([implementation, [], block]))
        .await?;

    let registry_account = account_proof(registry_rpc)?;
    let storage_proofs = registry_account.1;
    let implementation_account = account_proof(implementation_rpc)?.0;
    let mut canonical_next = active_validators
        .into_iter()
        .filter(|validator| validator.voting_power > 0)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !canonical_next.is_empty(),
        "next signing validator set is empty"
    );
    canonical_next.sort_by(|left, right| {
        right
            .voting_power
            .cmp(&left.voting_power)
            .then_with(|| left.address.cmp(&right.address))
    });
    Ok((
        canonical_next,
        ArcValidatorRegistryProofV1 {
            registry_account: registry_account.0,
            implementation_account,
            storage_proofs,
            active_validators: registered,
        },
    ))
}

fn account_proof(proof: RpcAccountProof) -> Result<(ArcAccountProofV1, Vec<ArcStorageProofV1>)> {
    let account = ArcAccountProofV1 {
        address: fixed_hex(&proof.address, "account proof address")?,
        nonce: quantity(&proof.nonce)?,
        balance: parse_u256_word(&proof.balance)?,
        storage_root: fixed_hex(&proof.storage_hash, "account storage root")?,
        code_hash: fixed_hex(&proof.code_hash, "account code hash")?,
        proof_nodes: proof
            .account_proof
            .iter()
            .map(|node| variable_hex(node, "account proof node"))
            .collect::<Result<Vec<_>>>()?,
    };
    let storage = proof
        .storage_proof
        .iter()
        .map(|item| {
            Ok(ArcStorageProofV1 {
                key: fixed_hex(&item.key, "storage proof key")?,
                value: parse_u256_word(&item.value)?,
                proof_nodes: item
                    .proof
                    .iter()
                    .map(|node| variable_hex(node, "storage proof node"))
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((account, storage))
}

fn add_to_slot(slot: B256, addend: u64) -> Result<B256> {
    U256::from_be_bytes(slot.0)
        .checked_add(U256::from(addend))
        .map(|value| B256::from(value.to_be_bytes::<32>()))
        .context("storage slot overflow")
}

fn mapping_slot(key: u64, slot: B256) -> B256 {
    let mut preimage = [0u8; 64];
    preimage[..32].copy_from_slice(&U256::from(key).to_be_bytes::<32>());
    preimage[32..].copy_from_slice(&slot.0);
    keccak256(preimage)
}

fn b256_hex(value: B256) -> String {
    format!("{value:#x}")
}

fn parse_u256(value: &str) -> Result<U256> {
    U256::from_str(value).with_context(|| format!("parse RPC uint256 {value}"))
}

fn parse_u256_word(value: &str) -> Result<[u8; 32]> {
    Ok(parse_u256(value)?.to_be_bytes())
}

fn find_deposit_event(
    receipt: &RpcReceipt,
    expected_vault: Address,
) -> Result<(u32, ERC20BridgeDepositedV2)> {
    let mut found = None;
    for (index, rpc_log) in receipt.logs.iter().enumerate() {
        if !rpc_log
            .address
            .eq_ignore_ascii_case(&format!("{expected_vault:#x}"))
        {
            continue;
        }
        let topics = rpc_log
            .topics
            .iter()
            .map(|topic| B256::from_str(topic).context("parse deposit topic"))
            .collect::<Result<Vec<_>>>()?;
        let data = Bytes::from(variable_hex(&rpc_log.data, "deposit log data")?);
        let log = Log {
            address: expected_vault,
            data: LogData::new(topics, data)
                .ok_or_else(|| anyhow!("deposit has too many topics"))?,
        };
        if let Ok(decoded) = ERC20BridgeDepositedV2::decode_log_validate(&log) {
            anyhow::ensure!(
                found.is_none(),
                "receipt contains multiple vault deposit events"
            );
            found = Some((u32::try_from(index)?, decoded.data));
        }
    }
    found.ok_or_else(|| anyhow!("receipt does not contain the expected vault deposit event"))
}

fn quantity(value: &str) -> Result<u64> {
    let raw = value
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("quantity lacks 0x prefix"))?;
    u64::from_str_radix(raw, 16).context("parse RPC quantity")
}

fn fixed_hex<const N: usize>(value: &str, field: &'static str) -> Result<[u8; N]> {
    variable_hex(value, field)?
        .try_into()
        .map_err(|_| anyhow!("{field} must be exactly {N} bytes"))
}

fn variable_hex(value: &str, field: &'static str) -> Result<Vec<u8>> {
    let raw = value
        .strip_prefix("0x")
        .ok_or_else(|| anyhow!("{field} lacks 0x prefix"))?;
    hex::decode(raw).with_context(|| format!("decode {field} hex"))
}
