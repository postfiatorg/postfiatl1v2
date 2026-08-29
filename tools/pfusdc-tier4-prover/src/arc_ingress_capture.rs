use std::{fs, path::PathBuf, str::FromStr};

use alloy::{
    primitives::{Address, Bytes, Log, LogData, B256},
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
    validator_set_commitment, verify_arc_ingress_witness_v1, ArcCommitSignatureV1,
    ArcIngressWitnessV1, ArcValidatorV1, ARC_INGRESS_WITNESS_SCHEMA_V1,
};
use serde::{de::DeserializeOwned, Deserialize};
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetReceipt {
    #[serde(flatten)]
    receipt: RpcReceipt,
    block_number: String,
    block_hash: String,
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

    let certificate: CommitCertificate = rpc
        .call(
            &args.rpc,
            "arc_getCertificate",
            json!([format!("0x{height:x}")]),
        )
        .await?;
    let validators = capture_validators(&rpc, &args.rpc, height - 1).await?;
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

    let witness = ArcIngressWitnessV1 {
        schema: ARC_INGRESS_WITNESS_SCHEMA_V1.to_owned(),
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
        validator_set_commitment_out: validator_commitment,
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
        next_validators: Vec::new(),
    };
    let public_values = verify_arc_ingress_witness_v1(&witness).map_err(|error| {
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

async fn capture_validators(rpc: &RpcClient, url: &str, at_height: u64) -> Result<Vec<Validator>> {
    let calldata = getActiveValidatorSetCall {}.abi_encode();
    let result: String = rpc
        .call(
            url,
            "eth_call",
            json!([{"to": ARC_VALIDATOR_REGISTRY, "data": encode_hex(calldata)}, format!("0x{at_height:x}")]),
        )
        .await?;
    let bytes = variable_hex(&result, "validator registry return")?;
    let decoded = getActiveValidatorSetCall::abi_decode_returns(&bytes)
        .context("decode active validator set")?;
    let mut validators = decoded
        .into_iter()
        .filter(|validator| {
            validator.status == ContractValidatorStatus::Active && validator.votingPower > 0
        })
        .map(|validator| {
            let public_key: [u8; 32] = validator
                .publicKey
                .as_ref()
                .try_into()
                .map_err(|_| anyhow!("active validator has a non-32-byte public key"))?;
            Ok(Validator {
                address: encode_hex(address_from_public_key(&public_key)),
                public_key: encode_hex(public_key),
                voting_power: validator.votingPower,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    validators.sort_by(|left, right| {
        right
            .voting_power
            .cmp(&left.voting_power)
            .then_with(|| left.address.cmp(&right.address))
    });
    Ok(validators)
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
