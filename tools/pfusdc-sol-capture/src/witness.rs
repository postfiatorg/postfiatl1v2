use agave_snapshots::SnapshotKind;
use anyhow::{Context, Result, ensure};
use pfusdc_sol_ingress_program::{
    DepositPublicValues, verify_block_witness, verify_finalized_block_witness,
};
use solana_account::{AccountSharedData, ReadableAccount};
use solana_bls_signatures::pubkey::PubkeyCompressed;
use solana_entry::block_component::{BlockFinalizationCert, VotesAggregate};
use solana_hash::Hash;
use solana_ledger::blockstore::Blockstore;
use solana_pubkey::Pubkey;
use solana_runtime::{bank::Bank, snapshot_package::SnapshotPackage};
use std::{fs, io::Write, path::Path};

// Agave runtime/src/snapshot_package.rs copies the bank's locked lattice accumulator
// and the fields used by Bank::hash_internal_state. No scan or snapshot flush is needed.
fn preimage(out: &mut Vec<u8>, bank: &Bank) -> Result<()> {
    ensure!(
        bank.is_frozen() && bank.block_id().is_some(),
        "bank is incomplete"
    );
    let fields = SnapshotPackage::new(SnapshotKind::Fastboot, bank, vec![], vec![])
        .bank_snapshot_package
        .bank_fields;
    out.extend(fields.parent_hash.to_bytes());
    out.extend(fields.signature_count.to_le_bytes());
    out.extend(bank.last_blockhash().to_bytes());
    for lane in fields.accounts_lt_hash.0.0 {
        out.extend(lane.to_le_bytes());
    }
    Ok(())
}

fn account(out: &mut Vec<u8>, state: &AccountSharedData) -> Result<()> {
    out.extend(state.lamports().to_le_bytes());
    out.push(u8::from(state.executable()));
    out.extend(state.owner().to_bytes());
    out.extend(u32::try_from(state.data().len())?.to_le_bytes());
    out.extend(state.data());
    Ok(())
}

pub struct State {
    pub bytes: Vec<u8>,
    pub ranks: Vec<u8>,
    pub parent_slot: u64,
    pub parent_id: Hash,
    pub id: Hash,
}

impl State {
    pub fn capture(bank: &Bank, parent: &Bank, vault: &Pubkey, route: &[u8; 32]) -> Result<Self> {
        ensure!(
            bank.parent_slot() == parent.slot() && bank.parent_hash() == parent.hash(),
            "bank ancestry mismatch"
        );
        let mut bytes = Vec::new();
        bytes.extend(bank.hash().to_bytes());
        bytes.extend(vault.to_bytes());
        bytes.extend(route);
        preimage(&mut bytes, bank)?;
        preimage(&mut bytes, parent)?;
        // runtime/src/bank.rs reads the slot writes from AccountsDB. Resolve each
        // corresponding old version on this fork before the root advances.
        let mut writes = bank.get_all_accounts_modified_since_parent();
        writes.sort_unstable_by_key(|(key, _)| *key);
        bytes.extend(u32::try_from(writes.len())?.to_le_bytes());
        for (key, new) in writes {
            bytes.extend(key.to_bytes());
            account(&mut bytes, &parent.get_account(&key).unwrap_or_default())?;
            account(&mut bytes, &new)?;
        }
        let epoch = bank
            .epoch_stakes_from_slot(bank.slot())
            .context("missing epoch stakes")?;
        let ranks = epoch.bls_pubkey_to_rank_map();
        let mut set = Vec::new();
        set.extend(u128::from(epoch.total_stake()).to_le_bytes());
        set.extend(u32::try_from(ranks.len())?.to_le_bytes());
        for (_, entry) in ranks.iter() {
            set.extend(PubkeyCompressed::from(&*entry.bls_pubkey).0);
            set.extend(entry.stake.get().get().to_le_bytes());
        }
        Ok(Self {
            bytes,
            ranks: set,
            parent_slot: parent.slot(),
            parent_id: parent.block_id().context("missing parent ID")?.to_hash(),
            id: bank.block_id().context("missing block ID")?.to_hash(),
        })
    }
}

pub(crate) fn aggregate(out: &mut Vec<u8>, votes: &VotesAggregate) -> Result<()> {
    out.extend(votes.uncompress_signature()?.0);
    out.extend(u16::try_from(votes.bitmap().len())?.to_le_bytes());
    out.extend(votes.bitmap());
    Ok(())
}

pub fn archive(
    dir: &Path,
    store: &Blockstore,
    state: &State,
    cert: &BlockFinalizationCert,
    finalized_id: Hash,
) -> Result<()> {
    ensure!(
        cert.block_id.to_hash() == state.id && state.id == finalized_id,
        "certified, frozen and rooted IDs differ"
    );
    let slot = cert.slot;
    let shreds = store.get_data_shreds_for_slot(slot, 0)?;
    let mut block = Vec::new();
    block.extend(slot.to_le_bytes());
    block.extend(state.parent_slot.to_le_bytes());
    block.extend(state.parent_id.to_bytes());
    block.extend(u32::try_from(shreds.len())?.to_le_bytes());
    for shred in &shreds {
        block.extend(shred.payload().as_ref());
    }
    let mut finality = vec![u8::from(cert.notar_aggregate.is_some())];
    if let Some(notar) = &cert.notar_aggregate {
        aggregate(&mut finality, notar)?;
    }
    aggregate(&mut finality, &cert.final_aggregate)?;

    let stage = tempfile::tempdir_in(dir)?;
    for (name, bytes) in [
        ("validators", &state.ranks),
        ("certificate", &finality),
        ("shreds", &block),
        ("state", &state.bytes),
    ] {
        fs::write(stage.path().join(name), bytes)?;
    }
    fs::write(stage.path().join("finalized-id"), finalized_id.to_string())?;
    fs::rename(stage.path(), dir.join(slot.to_string()))?;
    println!(
        "archived_slot={slot} data_shreds={} cluster_finalized_block_id={finalized_id}",
        shreds.len()
    );
    Ok(())
}

pub fn inspect(witness: &[u8]) -> Result<DepositPublicValues> {
    let values = verify_finalized_block_witness(witness)
        .map_err(|e| anyhow::anyhow!("verification failed: {e:?}"))?;
    let block = &values.block;
    println!(
        "verified_slot={} block_id={} changed_accounts={} receipts={} finality={} total_stake={} witness_bytes={}",
        block.slot,
        Hash::new_from_array(block.block_id),
        block.state.changed_accounts,
        block.state.receipts.len(),
        if values.fast_finality { "fast" } else { "slow" },
        values.total_stake,
        witness.len()
    );
    Ok(values)
}

/// The deposit witness for an archived slot. An archive with a `path` file proves finality
/// through a certified descendant: marker 2, the descendant's certificate, then the parent links.
pub fn assemble(source: &Path) -> Result<Vec<u8>> {
    let mut witness = fs::read(source.join("validators"))?;
    let path = source.join("path");
    if path.exists() {
        witness.push(2);
    }
    witness.extend(fs::read(source.join("certificate"))?);
    if path.exists() {
        witness.extend(fs::read(path)?);
    }
    witness.extend(fs::read(source.join("shreds"))?);
    witness.extend(fs::read(source.join("state"))?);
    Ok(witness)
}

/// Check the strict native block gate against an independently observed finalization and route.
pub fn verify_receipt_block(
    archive: &Path,
    slot: u64,
    finalized_id: Hash,
    vault: Pubkey,
    route: Hash,
) -> Result<()> {
    let source = archive.join(slot.to_string());
    let mut block_bytes = fs::read(source.join("shreds"))?;
    block_bytes.extend(fs::read(source.join("state"))?);
    let block = verify_block_witness(&block_bytes)
        .map_err(|err| anyhow::anyhow!("verify_block_witness: {err:?}"))?;
    ensure!(
        block.slot == slot && block.block_id == finalized_id.to_bytes(),
        "observed slot or block ID differs"
    );
    ensure!(
        block.state.vault_program == vault.to_bytes()
            && block.state.route_binding == route.to_bytes(),
        "vault or route binding differs"
    );
    let certified = inspect(&assemble(&source)?)?;
    ensure!(
        certified.block == block,
        "certified block differs from strict block"
    );
    println!(
        "verify_block_witness=accepted slot={slot} block_id={finalized_id} receipts={} certificate_slot={}",
        block.state.receipts.len(),
        certified.certificate_slot
    );
    Ok(())
}

pub fn build(dir: &Path, slot: u64, output: &Path) -> Result<()> {
    let source = dir.join(slot.to_string());
    let witness = assemble(&source)?;
    let values = inspect(&witness)?;
    let expected: Hash = fs::read_to_string(source.join("finalized-id"))?
        .trim()
        .parse()?;
    ensure!(
        values.block.slot == slot && values.block.block_id == expected.to_bytes(),
        "slot or finalized ID mismatch"
    );
    let mut file = tempfile::NamedTempFile::new_in(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    file.write_all(&witness)?;
    file.persist(output)?;
    Ok(())
}
