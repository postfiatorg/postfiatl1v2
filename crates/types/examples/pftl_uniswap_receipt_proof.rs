use std::{env, fs};

use postfiat_types::{pftl_uniswap_consensus_receipt_merkle_proof, LedgerState};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let ledger_path = args
        .next()
        .ok_or_else(|| "usage: pftl_uniswap_receipt_proof LEDGER.json RECEIPT_HASH".to_string())?;
    let receipt_hash = args
        .next()
        .ok_or_else(|| "usage: pftl_uniswap_receipt_proof LEDGER.json RECEIPT_HASH".to_string())?;
    if args.next().is_some() {
        return Err("unexpected extra argument".to_string());
    }

    let bytes = fs::read(&ledger_path)
        .map_err(|error| format!("failed to read `{ledger_path}`: {error}"))?;
    let ledger: LedgerState = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse `{ledger_path}`: {error}"))?;
    let receipt_hashes = ledger
        .pftl_uniswap_receipts
        .iter()
        .map(|receipt| receipt.receipt_hash.clone())
        .collect::<Vec<_>>();
    let proof = pftl_uniswap_consensus_receipt_merkle_proof(&receipt_hashes, &receipt_hash)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&proof)
            .map_err(|error| format!("failed to encode receipt proof: {error}"))?
    );
    Ok(())
}
