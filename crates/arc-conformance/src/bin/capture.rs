#![forbid(unsafe_code)]

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use alloy_sol_types::{sol, SolCall};
use arc_conformance::{
    address_from_public_key, encode_hex, BlockFixture, CommitCertificate, GoldenFixture, Validator,
    ValidatorSetFixture, ARC_TESTNET_CHAIN_ID, ARC_VALIDATOR_REGISTRY,
};
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_RPC: &str = "https://rpc.testnet.arc.network";
const ARC_NODE_COMMIT: &str = "66ad2d5aa6d9b41e8f689812004be4c7233a9e16";

sol! {
    #[derive(PartialEq, Eq)]
    enum ContractValidatorStatus { Unknown, Registered, Active }
    struct ContractValidator {
        ContractValidatorStatus status;
        bytes publicKey;
        uint64 votingPower;
    }
    function getActiveValidatorSet() external view returns (ContractValidator[] memory activeValidators);
}

#[derive(Deserialize)]
struct RpcEnvelope<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcBlock {
    number: String,
    hash: String,
    receipts_root: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut rpc = DEFAULT_RPC.to_owned();
    let mut requested_height = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rpc" => rpc = required_arg(&mut args, "--rpc")?,
            "--height" => {
                requested_height = Some(required_arg(&mut args, "--height")?.parse::<u64>()?)
            }
            "--out" => out = Some(PathBuf::from(required_arg(&mut args, "--out")?)),
            "--help" | "-h" => {
                println!(
                    "usage: cargo run -p arc-conformance --bin capture -- --out PATH [--height N] [--rpc URL]"
                );
                return Ok(());
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }
    let out = out.ok_or("--out is required")?;
    let client = Client::builder().build()?;

    let chain_id_hex: String = rpc_call(&client, &rpc, "eth_chainId", json!([]))?;
    let chain_id = parse_quantity(&chain_id_hex)?;
    if chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(format!("wrong chain id {chain_id}; expected {ARC_TESTNET_CHAIN_ID}").into());
    }

    let height = match requested_height {
        Some(height) => height,
        None => {
            let latest_hex: String = rpc_call(&client, &rpc, "eth_blockNumber", json!([]))?;
            parse_quantity(&latest_hex)?
                .checked_sub(8)
                .ok_or("Arc head is too low to capture a finalized fixture")?
        }
    };
    if height == 0 {
        return Err("height must be positive".into());
    }
    let height_hex = format!("0x{height:x}");
    let block: RpcBlock = rpc_call(
        &client,
        &rpc,
        "eth_getBlockByNumber",
        json!([height_hex, false]),
    )?;
    let certificate: CommitCertificate = rpc_call(
        &client,
        &rpc,
        "arc_getCertificate",
        json!([format!("0x{height:x}")]),
    )?;

    let query_block = height - 1;
    let calldata = getActiveValidatorSetCall {}.abi_encode();
    let result_hex: String = rpc_call(
        &client,
        &rpc,
        "eth_call",
        json!([
            {"to": ARC_VALIDATOR_REGISTRY, "data": encode_hex(calldata)},
            format!("0x{query_block:x}")
        ]),
    )?;
    let result_bytes = decode_hex(&result_hex)?;
    let contract_validators: Vec<ContractValidator> =
        getActiveValidatorSetCall::abi_decode_returns(&result_bytes)?;
    if contract_validators.len() > arc_conformance::MAX_VALIDATORS {
        return Err("validator set exceeds conformance bound".into());
    }

    let mut validators = Vec::new();
    for contract_validator in contract_validators {
        if contract_validator.status != ContractValidatorStatus::Active
            || contract_validator.votingPower == 0
        {
            continue;
        }
        let public_key: [u8; 32] = contract_validator
            .publicKey
            .as_ref()
            .try_into()
            .map_err(|_| "active validator has a non-32-byte public key")?;
        validators.push(Validator {
            address: encode_hex(address_from_public_key(&public_key)),
            public_key: encode_hex(public_key),
            voting_power: contract_validator.votingPower,
        });
    }
    validators.sort_by(|left, right| {
        right
            .voting_power
            .cmp(&left.voting_power)
            .then_with(|| left.address.cmp(&right.address))
    });

    let fixture = GoldenFixture {
        schema: "arc-conformance-fixture-v1".to_owned(),
        chain_id,
        arc_node_commit: ARC_NODE_COMMIT.to_owned(),
        block: BlockFixture {
            number: parse_quantity(&block.number)?,
            hash: block.hash,
            receipts_root: block.receipts_root,
        },
        certificate,
        validator_set: ValidatorSetFixture {
            queried_at_block: query_block,
            validators,
        },
    };
    arc_conformance::verify_fixture(&fixture)?;

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &out,
        format!("{}\n", serde_json::to_string_pretty(&fixture)?),
    )?;
    println!(
        "captured Arc height {} with {} verified signatures to {}",
        fixture.block.number,
        fixture.certificate.signatures.len(),
        out.display()
    );
    Ok(())
}

fn required_arg(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn Error>> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn rpc_call<T: DeserializeOwned>(
    client: &Client,
    rpc: &str,
    method: &str,
    params: Value,
) -> Result<T, Box<dyn Error>> {
    let response = client
        .post(rpc)
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
        .send()?
        .error_for_status()?
        .json::<RpcEnvelope<T>>()?;
    if let Some(error) = response.error {
        return Err(format!("RPC {method} failed ({}): {}", error.code, error.message).into());
    }
    response
        .result
        .ok_or_else(|| format!("RPC {method} returned no result").into())
}

fn parse_quantity(value: &str) -> Result<u64, Box<dyn Error>> {
    Ok(u64::from_str_radix(
        value.strip_prefix("0x").ok_or("quantity lacks 0x prefix")?,
        16,
    )?)
}

fn decode_hex(value: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(hex::decode(
        value
            .strip_prefix("0x")
            .ok_or("hex value lacks 0x prefix")?,
    )?)
}
