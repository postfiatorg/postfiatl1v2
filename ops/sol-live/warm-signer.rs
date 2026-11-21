//! Private stdio worker. Use the release wallet's canonical validation and signing.
use postfiat_node::{wallet_sign_asset_transaction, WalletSignAssetTransactionOptions};
use serde_json::{json, Value};
use std::{
    env,
    io::{self, BufRead, Write},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [key, chain, genesis, source, profile] = args.as_slice() else {
        return Err("expected key path, chain, genesis, source and route profile".into());
    };
    for line in io::stdin().lock().lines() {
        let quote: Value = serde_json::from_str(&line?)?;
        let op = &quote["operation"];
        if quote["chain_id"] != *chain
            || quote["genesis_hash"] != *genesis
            || quote["source"] != *source
            || op["submitter"] != *source
            || op["profile_hash"] != *profile
            || op["operation"] != "solana_bridge_proof_v1"
        {
            return Err("unexpected signing domain".into());
        }
        let signed = wallet_sign_asset_transaction(WalletSignAssetTransactionOptions {
            key_file: PathBuf::from(key),
            chain_id: chain.clone(),
            genesis_hash: genesis.clone(),
            expected_source: Some(source.clone()),
            protocol_version: quote["protocol_version"]
                .as_u64()
                .ok_or("version")?
                .try_into()?,
            fee: quote["minimum_fee"].as_u64().ok_or("fee")?,
            sequence: quote["sequence"].as_u64().ok_or("sequence")?,
            operation: serde_json::from_value(op.clone())?,
        });
        let reply = match signed {
            Ok(tx) => json!({"signed": tx}),
            Err(_) => json!({"error": "release wallet rejected the request"}),
        };
        println!("{}", reply);
        io::stdout().flush()?;
    }
    Ok(())
}
