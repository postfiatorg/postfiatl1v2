#![forbid(unsafe_code)]

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use arc_conformance::receipts::{build_receipt_fixture, ReceiptBlockFixture, RpcReceipt};
use arc_conformance::ARC_TESTNET_CHAIN_ID;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

const DEFAULT_RPC: &str = "https://rpc.testnet.arc.network";

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
    let mut height = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rpc" => rpc = required_arg(&mut args, "--rpc")?,
            "--height" => height = Some(required_arg(&mut args, "--height")?.parse::<u64>()?),
            "--out" => out = Some(PathBuf::from(required_arg(&mut args, "--out")?)),
            "--help" | "-h" => {
                println!(
                    "usage: cargo run -p arc-conformance --bin capture_receipts -- --height N --out PATH [--rpc URL]"
                );
                return Ok(());
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
    }
    let height = height.ok_or("--height is required")?;
    let out = out.ok_or("--out is required")?;
    let client = Client::builder().build()?;

    let chain_id_hex: String = rpc_call(&client, &rpc, "eth_chainId", json!([]))?;
    let chain_id = parse_quantity(&chain_id_hex)?;
    if chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(format!("wrong chain id {chain_id}").into());
    }
    let height_hex = format!("0x{height:x}");
    let block: RpcBlock = rpc_call(
        &client,
        &rpc,
        "eth_getBlockByNumber",
        json!([height_hex, false]),
    )?;
    let receipts: Vec<RpcReceipt> = rpc_call(
        &client,
        &rpc,
        "eth_getBlockReceipts",
        json!([format!("0x{height:x}")]),
    )?;
    let block = ReceiptBlockFixture {
        number: parse_quantity(&block.number)?,
        hash: block.hash,
        receipts_root: block.receipts_root,
    };
    let fixture = build_receipt_fixture(chain_id, block, &receipts)?;

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &out,
        format!("{}\n", serde_json::to_string_pretty(&fixture)?),
    )?;
    println!(
        "captured {} Arc receipt proofs at height {} to {}",
        fixture.receipts.len(),
        fixture.block.number,
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
