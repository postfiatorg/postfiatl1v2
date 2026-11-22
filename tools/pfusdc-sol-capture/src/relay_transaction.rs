//! Recover the signed message from the same finalized shreds used for proving.
use anyhow::{Context, Result, ensure};
use pfusdc_sol_ingress_program::shreds::{SHRED_PAYLOAD, block_from_shreds};
use solana_entry::block_component::BlockComponent;
use solana_hash::Hash;
use solana_signature::Signature;
use std::{fs, path::Path};

pub fn read(dir: &Path, slot: u64, signature: Signature) -> Result<()> {
    let dir = dir.join(slot.to_string());
    let raw = fs::read(dir.join("shreds"))?;
    ensure!(raw.len() >= 52, "missing shred header");
    let parent = u64::from_le_bytes(raw[8..16].try_into()?);
    let count = u32::from_le_bytes(raw[48..52].try_into()?) as usize;
    ensure!(
        raw.len() - 52 == count * SHRED_PAYLOAD,
        "incorrect shred count"
    );
    ensure!(
        u64::from_le_bytes(raw[..8].try_into()?) == slot,
        "incorrect slot"
    );
    let shreds = raw[52..].chunks_exact(SHRED_PAYLOAD).collect::<Vec<_>>();
    let block = block_from_shreds(slot, parent, &raw[16..48].try_into()?, &shreds)
        .map_err(|e| anyhow::anyhow!("invalid shreds: {e:?}"))?;
    let expected: Hash = fs::read_to_string(dir.join("finalized-id"))?
        .trim()
        .parse()?;
    ensure!(expected.to_bytes() == block.block_id, "wrong block ID");
    let mut start = 0;
    for end in block.component_ends {
        let component = &block.stream[start..end];
        start = end;
        if !BlockComponent::infer_is_entry_batch(component).unwrap_or(false) {
            continue;
        }
        let BlockComponent::EntryBatch(entries) =
            wincode::deserialize(component).context("Agave entry decoding")?
        else {
            continue;
        };
        for tx in entries.into_iter().flat_map(|entry| entry.transactions) {
            if tx.signatures.first() != Some(&signature) {
                continue;
            }
            let msg = &tx.message;
            let keys: Vec<_> = msg
                .static_account_keys()
                .iter()
                .map(ToString::to_string)
                .collect();
            let calls: Vec<_> = msg.instructions().iter().map(|ix| serde_json::json!({
                "programIdIndex": ix.program_id_index, "accounts":ix.accounts, "data_hex":hex::encode(&ix.data)
            })).collect();
            let lookups: Vec<_> = msg.address_table_lookups().into_iter().flatten().map(|a| serde_json::json!({
                "accountKey": a.account_key.to_string(), "writableIndexes":a.writable_indexes,
                "readonlyIndexes":a.readonly_indexes
            })).collect();
            println!(
                "{}",
                serde_json::json!({"block_id":hex::encode(block.block_id), "keys":keys,
                "instructions":calls, "recentBlockhash":msg.recent_blockhash().to_string(), "lookups":lookups})
            );
            return Ok(());
        }
    }
    anyhow::bail!("requested transaction is absent from finalized entries")
}
