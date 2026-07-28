use std::{env, fs};

use postfiat_types::PftlUniswapMintPacketV2;

fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| "usage: pftl_uniswap_packet_digest PACKET.json".to_string())?;
    let bytes = fs::read(&path).map_err(|error| format!("failed to read `{path}`: {error}"))?;
    let packet: PftlUniswapMintPacketV2 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse `{path}`: {error}"))?;
    println!("{}", packet.evm_digest()?);
    Ok(())
}
