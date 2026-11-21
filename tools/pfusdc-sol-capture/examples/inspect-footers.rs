//! Inspect rooted block footers in a running source validator's read-only blockstore.
use anyhow::{Context, Result, ensure};
use solana_entry::block_component::{VersionedBlockFooter, VersionedBlockMarker};
use solana_ledger::{
    blockstore::Blockstore,
    blockstore_meta::BlockLocation,
    blockstore_options::{AccessType, BlockstoreOptions},
};
use std::{env, path::Path};

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [ledger, first, last] = args.as_slice() else {
        anyhow::bail!("usage: inspect-footers LEDGER FIRST LAST")
    };
    let (first, last): (u64, u64) = (first.parse()?, last.parse()?);
    ensure!(
        first <= last && last - first <= 4096,
        "invalid audit interval"
    );
    let store = Blockstore::open_with_options(
        Path::new(ledger),
        BlockstoreOptions {
            access_type: AccessType::ReadOnly,
            ..Default::default()
        },
    )?;
    ensure!(
        store.is_root(first),
        "first slot is unavailable or unrooted"
    );
    let id = store
        .get_double_merkle_root(first, BlockLocation::Original)?
        .context("missing target block ID")?;
    let mut footers = Vec::new();
    for slot in first..=last {
        if !store.is_root(slot) {
            continue;
        }
        let (components, _, full) = store.get_slot_components_with_shred_info(slot, 0, false)?;
        ensure!(full, "incomplete rooted slot {slot}");
        for c in components {
            if let Some(VersionedBlockMarker::V1(marker)) = c.as_marker()
                && let Some(VersionedBlockFooter::V1(footer)) = marker.as_block_footer()
            {
                let cert = footer.block_final_cert.as_ref();
                footers.push(
                    serde_json::json!({"footer_slot":slot,"certified_slot":cert.map(|c|c.slot),
                    "certified_block_id":cert.map(|c|c.block_id.to_hash().to_string())}),
                );
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"slot":first,"through_slot":last,"target_block_id":id.to_string(),"footers":footers})
    );
    Ok(())
}
