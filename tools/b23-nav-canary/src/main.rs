//! Read-only live export and isolated strict release-4 execution. No RPC writer.
use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use postfiat_storage::NodeStore;
use postfiat_types::{Genesis, LedgerState, SignedAssetTransaction};
use serde::Deserialize;
use std::{fs, path::PathBuf};
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
    Apply {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        steps: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
}
#[derive(Deserialize)]
struct Step {
    label: String,
    height: u64,
    transaction: SignedAssetTransaction,
    expect_status: String,
}
fn save(path: PathBuf, value: &serde_json::Value) -> Result<()> {
    use std::io::Write;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&serde_json::to_vec(value)?)?;
    Ok(())
}
fn main() -> Result<()> {
    match Args::parse().command {
        Command::Export { data_dir, output } => {
            let store = NodeStore::try_new_read_only(data_dir)?;
            let tip = store.read_chain_tip()?;
            let genesis = store.read_genesis()?;
            let ledger = store.read_ledger()?;
            let governance = store.read_governance()?;
            ensure!(
                tip == store.read_chain_tip()?,
                "chain moved during snapshot; discard export"
            );
            save(
                output,
                &serde_json::json!({"schema":"postfiat.b23.live_canary_snapshot.v1","tip":tip,"genesis":genesis,"ledger":ledger,"governance":governance}),
            )?;
        }
        Command::Apply {
            snapshot,
            steps,
            output,
        } => {
            let mut snapshot: serde_json::Value = serde_json::from_slice(&fs::read(snapshot)?)?;
            let genesis: Genesis = serde_json::from_value(snapshot["genesis"].clone())?;
            let mut ledger: LedgerState = serde_json::from_value(snapshot["ledger"].clone())?;
            let steps: Vec<Step> = serde_json::from_slice(&fs::read(steps)?)?;
            let mut receipts = vec![];
            let mut pass = true;
            for step in steps {
                let before = serde_json::to_value(&ledger)?;
                let receipt = postfiat_execution::execute_asset_transaction(
                    &genesis,
                    &mut ledger,
                    &step.transaction,
                    step.height,
                );
                let value = serde_json::to_value(&receipt)?;
                let accepted = value["status"].as_str() == Some(&step.expect_status);
                pass &= accepted;
                receipts.push(serde_json::json!({"label":step.label,"height":step.height,"expected_status":step.expect_status,"matched":accepted,"ledger_changed":before!=serde_json::to_value(&ledger)?,"receipt":value}));
                if !accepted {
                    break;
                }
            }
            snapshot["ledger"] = serde_json::to_value(ledger)?;
            snapshot["canary_receipts"] = serde_json::to_value(receipts)?;
            snapshot["canary_pass"] = pass.into();
            save(output, &snapshot)?;
            ensure!(
                pass,
                "canary expected status did not match actual release-4 execution"
            );
        }
    }
    Ok(())
}
