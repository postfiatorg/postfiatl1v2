//! Convert a pair of public observer captures to the same archive used by the local tool.
use anyhow::{Context, Result, anyhow, ensure};
use pfusdc_sol_ingress_program::{ancestry, shreds, verify_block_witness};
use solana_entry::block_component::{
    BlockComponent, BlockFinalizationCert, VersionedBlockFooter, VersionedBlockMarker,
};
use solana_hash::Hash;
use solana_signature::Signature;
use std::{fs, path::Path};

fn read_block(dir: &Path) -> Result<(u64, Vec<u8>, shreds::Block)> {
    let mut bytes = fs::read(dir.join("block-prefix.bin"))?;
    ensure!(bytes.len() == 52, "invalid block prefix");
    let slot = u64::from_le_bytes(bytes[..8].try_into()?);
    let parent = u64::from_le_bytes(bytes[8..16].try_into()?);
    let parent_id = bytes[16..48].try_into()?;
    let count = u32::from_le_bytes(bytes[48..52].try_into()?) as usize;
    let payloads = fs::read(dir.join("shreds.bin"))?;
    ensure!(
        count.checked_mul(shreds::SHRED_PAYLOAD) == Some(payloads.len()),
        "invalid shred count"
    );
    let views: Vec<_> = payloads.chunks_exact(shreds::SHRED_PAYLOAD).collect();
    let block = shreds::block_from_shreds(slot, parent, &parent_id, &views)
        .map_err(|err| anyhow!("invalid shreds at {slot}: {err:?}"))?;
    bytes.extend(payloads);
    Ok((slot, bytes, block))
}

fn footer_cert(carrier_block: shreds::Block) -> Result<BlockFinalizationCert> {
    shreds::footer_bank_hash(&carrier_block).map_err(|e| anyhow!("carrier footer: {e:?}"))?;
    let mut start = 0;
    let mut cert = None;
    for end in carrier_block.component_ends {
        let data = &carrier_block.stream[start..end];
        start = end;
        if data.get(..8) != Some(&[0u8; 8]) {
            continue;
        }
        let marker: BlockComponent = wincode::deserialize(data)?;
        if let Some(VersionedBlockMarker::V1(marker)) = marker.as_marker()
            && let Some(VersionedBlockFooter::V1(footer)) = marker.as_block_footer()
        {
            cert = footer.block_final_cert.clone();
        }
    }
    cert.context("carrier footer has no certificate")
}

pub fn cert_target(carrier: &Path) -> Result<()> {
    let (slot, _, block) = read_block(carrier)?;
    let cert = footer_cert(block)?;
    println!(
        "carrier={slot} target={} block_id={}",
        cert.slot, cert.block_id
    );
    Ok(())
}

pub fn archive(
    target: &Path,
    carrier: &Path,
    directory: &Path,
    vault: [u8; 32],
    route: [u8; 32],
    signature: Option<Signature>,
) -> Result<()> {
    let (slot, block_bytes, target_block) = read_block(target)?;
    if let Some(signature) = signature {
        let mut start = 0;
        let mut found = false;
        for &end in &target_block.component_ends {
            let data = &target_block.stream[start..end];
            start = end;
            if BlockComponent::infer_is_entry_batch(data) == Some(true)
                && let BlockComponent::EntryBatch(entries) = wincode::deserialize(data)?
                && entries
                    .iter()
                    .flat_map(|entry| &entry.transactions)
                    .any(|tx| tx.signatures.first() == Some(&signature))
            {
                found = true;
                break;
            }
        }
        ensure!(
            found,
            "transaction {signature} absent from captured entries"
        );
        println!("captured_transaction={signature}");
    }
    let (carrier_slot, _, carrier_block) = read_block(carrier)?;
    ensure!(
        carrier_slot > slot,
        "certificate must come from a later slot"
    );
    let cert = footer_cert(carrier_block)?;
    // Not every block is named by a certificate. A certified descendant commits to the target
    // through the parent block ID in each block ID, so walk those links down to it.
    let path = if cert.slot == slot {
        ensure!(
            cert.block_id.as_bytes() == &target_block.block_id,
            "carrier certifies {} {}, target is {} {}",
            cert.slot,
            cert.block_id,
            slot,
            Hash::new_from_array(target_block.block_id)
        );
        None
    } else {
        ensure!(cert.slot > slot, "carrier certifies an earlier slot");
        let siblings = carrier.parent().context("carrier has no directory")?;
        let mut links = Vec::new();
        let (mut at, mut id) = (cert.slot, *cert.block_id.as_bytes());
        let mut count = 0u32;
        while at > slot {
            ensure!((count as usize) < ancestry::MAX_LINKS, "descendant chain too long");
            let dir = siblings.join(format!("{at}-{}", Hash::new_from_array(id)));
            let (link_slot, bytes, block) = read_block(&dir)
                .with_context(|| format!("descendant {at} is not captured"))?;
            ensure!(link_slot == at && block.block_id == id, "descendant differs from its directory");
            let parent = u64::from_le_bytes(bytes[8..16].try_into()?);
            let parent_id: [u8; 32] = bytes[16..48].try_into()?;
            let payloads = &bytes[52..];
            let count_shreds = payloads.len() / shreds::SHRED_PAYLOAD;
            ensure!(
                count_shreds % shreds::DATA_SHREDS_PER_SET == 0
                    && (1..=ancestry::MAX_FEC_SETS).contains(&(count_shreds / shreds::DATA_SHREDS_PER_SET)),
                "descendant {at} has a partial erasure set"
            );
            // read_block checked every shred against its set and the sets against the block ID.
            let roots: Vec<[u8; 32]> = (0..count_shreds)
                .step_by(shreds::DATA_SHREDS_PER_SET)
                .map(|i| shreds::erasure_set_root(at, i, &payloads[i * shreds::SHRED_PAYLOAD..(i + 1) * shreds::SHRED_PAYLOAD]))
                .collect::<std::result::Result<_, _>>()
                .map_err(|err| anyhow!("descendant {at} shreds: {err:?}"))?;
            links.extend(parent.to_le_bytes());
            links.extend(parent_id);
            links.extend((roots.len() as u32).to_le_bytes());
            links.extend(roots.concat());
            (at, id) = (parent, parent_id);
            count += 1;
        }
        ensure!(
            at == slot && id == target_block.block_id,
            "certified descendant {} does not descend from the target",
            cert.slot
        );
        let mut path = cert.slot.to_le_bytes().to_vec();
        path.extend(count.to_le_bytes());
        path.extend(links);
        println!("certified_descendant={} links={count}", cert.slot);
        Some(path)
    };
    let mut certificate = vec![u8::from(cert.notar_aggregate.is_some())];
    if let Some(notar) = &cert.notar_aggregate {
        super::witness::aggregate(&mut certificate, notar)?;
    }
    super::witness::aggregate(&mut certificate, &cert.final_aggregate)?;
    let mut state = fs::read(target.join("bank-hash.bin"))?;
    ensure!(state.len() == 32, "invalid bank hash");
    state.extend(vault);
    state.extend(route);
    state.extend(fs::read(target.join("state-body.bin"))?);
    let validators = fs::read(target.join("set.bin"))?;
    let mut block_witness = block_bytes.clone();
    block_witness.extend(&state);
    println!(
        "verify_block_witness={:?}",
        verify_block_witness(&block_witness).map(|b| b.slot)
    );
    let marker: &[u8] = if path.is_some() { &[2] } else { &[] };
    let witness = [&validators[..], marker, &certificate, path.as_deref().unwrap_or(&[]), &block_witness].concat();
    super::witness::inspect(&witness)?;
    fs::create_dir_all(directory)?;
    let stage = tempfile::tempdir_in(directory)?;
    for (name, data) in [
        ("state", state),
        ("certificate", certificate),
        ("validators", validators),
        ("shreds", block_bytes),
    ] {
        fs::write(stage.path().join(name), data)?;
    }
    if let Some(path) = path {
        fs::write(stage.path().join("path"), path)?;
    }
    // The target's own block ID: with a descendant certificate the signed ID is the descendant's.
    fs::write(
        stage.path().join("finalized-id"),
        Hash::new_from_array(target_block.block_id).to_string(),
    )?;
    fs::rename(stage.path(), directory.join(slot.to_string()))?;
    println!(
        "cert_footer_slot={carrier_slot} signed_block_id={}",
        cert.block_id
    );
    Ok(())
}
