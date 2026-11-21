//! Export only public finalized data through a read-only live store; import only
//! into a fresh, explicitly local Phase-6 archive for the frozen V2 witness CLI.
use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use postfiat_storage::NodeStore;
use postfiat_types::{
    BatchArchive, BlockLog, ChainTipState, Genesis, GovernanceState, LedgerState,
};
use serde_json::{json, Value};
use std::{fs, io::Write, path::PathBuf};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Export {
        #[arg(long)]
        data_dir: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Import {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
fn write_new(path: PathBuf, value: &Value) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&serde_json::to_vec(value)?)?;
    file.sync_all()?;
    Ok(())
}
fn registry(path: PathBuf) -> Result<Value> {
    let r: Value = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        r.as_object()
            .is_some_and(|x| x.len() == 1 && x.contains_key("validators")),
        "unexpected registry envelope"
    );
    for v in r["validators"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("registry entries absent"))?
    {
        ensure!(
            v.as_object().is_some_and(|x| x.keys().all(|k| [
                "node_id",
                "algorithm_id",
                "public_key_hex",
                "active"
            ]
            .contains(&k.as_str()))),
            "registry contains non-public fields"
        );
    }
    Ok(r)
}
fn main() -> Result<()> {
    match Args::parse().command {
        Command::Export { data_dir, output } => {
            let s = NodeStore::try_new_read_only(&data_dir)?;
            let tip = s.read_chain_tip()?;
            let genesis = s.read_genesis()?;
            let governance = s.read_governance()?;
            let ledger = s.read_ledger()?;
            let blocks = s.read_blocks()?;
            let archive = s.read_batch_archive()?;
            let current_registry = registry(data_dir.join("validator_registry.json"))?;
            let genesis_registry = registry(data_dir.join("validator_registry_genesis.json"))?;
            ensure!(
                tip == s.read_chain_tip()?,
                "chain moved while exporting; discard bundle"
            );
            let last = blocks
                .blocks
                .last()
                .ok_or_else(|| anyhow::anyhow!("empty block archive"))?;
            ensure!(
                last.header.height == tip.height
                    && last.header.block_hash == tip.block_hash
                    && last.header.state_root == tip.state_root,
                "archive tip mismatch"
            );
            write_new(
                output,
                &json!({"schema":"postfiat.b24.public-offline-archive.v1","tip":tip,"genesis":genesis,"governance":governance,"ledger":ledger,"blocks":blocks,"batch_archive":archive,"validator_registry":current_registry,"validator_registry_genesis":genesis_registry}),
            )?;
        }
        Command::Import { bundle, output } => {
            ensure!(
                std::env::var("B24_OFFLINE_ARCHIVE_IMPORT").as_deref() == Ok("1"),
                "explicit offline import required"
            );
            let root = PathBuf::from("/home/postfiat/repos/b23-coord/evidence/tee/phase6")
                .canonicalize()?;
            ensure!(
                !output.exists()
                    && output
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("parent absent"))?
                        .canonicalize()?
                        .starts_with(root),
                "import must create a fresh Phase-6 archive directory"
            );
            let v: Value = serde_json::from_slice(&fs::read(bundle)?)?;
            ensure!(v["schema"] == "postfiat.b24.public-offline-archive.v1");
            let tip: ChainTipState = serde_json::from_value(v["tip"].clone())?;
            let genesis: Genesis = serde_json::from_value(v["genesis"].clone())?;
            let governance: GovernanceState = serde_json::from_value(v["governance"].clone())?;
            let ledger: LedgerState = serde_json::from_value(v["ledger"].clone())?;
            let blocks: BlockLog = serde_json::from_value(v["blocks"].clone())?;
            let archive: BatchArchive = serde_json::from_value(v["batch_archive"].clone())?;
            fs::create_dir(&output)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&output, fs::Permissions::from_mode(0o700))?;
            }
            let s = NodeStore::try_new(&output)?;
            s.write_genesis(&genesis)?;
            s.write_governance(&governance)?;
            s.write_ledger(&ledger)?;
            s.write_blocks(&blocks)?;
            s.write_batch_archive(&archive)?;
            s.write_chain_tip(&tip)?;
            write_new(
                output.join("validator_registry.json"),
                &v["validator_registry"],
            )?;
            write_new(
                output.join("validator_registry_genesis.json"),
                &v["validator_registry_genesis"],
            )?;
            ensure!(
                serde_json::to_value(s.read_ledger()?)? == v["ledger"]
                    && serde_json::to_value(s.read_blocks()?)? == v["blocks"]
                    && serde_json::to_value(s.read_batch_archive()?)? == v["batch_archive"]
                    && s.read_chain_tip()? == tip,
                "offline roundtrip differs"
            );
            println!(
                "{}",
                json!({"offline_import_verified":true,"height":tip.height,"blocks":blocks.blocks.len(),"batches":archive.batches.len(),"validator_private_keys_copied":false})
            );
        }
    }
    Ok(())
}
