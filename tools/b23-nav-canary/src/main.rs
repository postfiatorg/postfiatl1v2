//! Read-only live export and isolated strict release-4 execution. No RPC writer.
use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use postfiat_execution::AssetExecutionCompatibility;
use postfiat_storage::NodeStore;
use postfiat_types::{
    Genesis, GovernanceState, LedgerState, NavReservePublicValuesV1, SignedAssetTransaction,
    SignedOfferTransaction,
};
use serde::Deserialize;
use std::{fs, path::PathBuf};
mod packet;
#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    PacketContext {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        public_values: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
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
    transaction: serde_json::Value,
    #[serde(default)]
    kind: String,
    expect_accepted: bool,
    #[serde(default)]
    expect_code: Option<String>,
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
        Command::PacketContext {
            snapshot,
            public_values,
            output,
        } => {
            let snapshot: serde_json::Value = serde_json::from_slice(&fs::read(snapshot)?)?;
            let ledger: LedgerState = serde_json::from_value(snapshot["ledger"].clone())?;
            let values = NavReservePublicValuesV1::decode(&fs::read(public_values)?)
                .map_err(anyhow::Error::msg)?;
            let overlay = postfiat_execution::nav_subscription_reserve_overlay_for_asset(
                &ledger,
                &values.nav_asset_id,
            )
            .map_err(anyhow::Error::msg)?;
            let nav = ledger
                .nav_asset(&values.nav_asset_id)
                .ok_or_else(|| anyhow::anyhow!("NAV asset missing"))?;
            let mut result = packet::compose(&values, overlay.as_ref(), nav.circulating_supply)?;
            result["snapshot_tip"] = snapshot["tip"].clone();
            save(output, &result)?;
        }
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
            let governance: GovernanceState =
                serde_json::from_value(snapshot["governance"].clone())?;
            // Exact release-4 execution_actions.rs configuration, including
            // source-series activation. Never use replay/fixture exceptions.
            let compatibility = AssetExecutionCompatibility::strict()
                .with_bridge_verification_activation_height(
                    governance
                        .bridge_verification_activation_height()
                        .or(genesis.bridge_verification_activation_height),
                )
                .with_orchard_aware_bridge_claim_activation_height(
                    governance
                        .orchard_aware_bridge_claim_activation_height()
                        .or(genesis.orchard_aware_bridge_claim_activation_height),
                )
                .with_pfusdc_source_series_activation_height(
                    governance
                        .pfusdc_source_series_activation_height()
                        .or(genesis.pfusdc_source_series_activation_height),
                )
                .with_atomic_swap_activation_height(
                    governance
                        .atomic_swap_activation_height()
                        .or(genesis.atomic_swap_activation_height),
                )
                .with_yolo_target_activation_height(governance.yolo_target_activation_height())
                .with_atomic_swap_paused(governance.atomic_swap_paused);
            snapshot["canary_activations"] = serde_json::json!({
                "bridge_verification":compatibility.bridge_verification_activation_height,
                "orchard_aware_bridge_claim":compatibility.orchard_aware_bridge_claim_activation_height,
                "pfusdc_source_series":compatibility.pfusdc_source_series_activation_height,
                "atomic_swap":compatibility.atomic_swap_activation_height,
                "yolo_target":compatibility.yolo_target_activation_height,
                "atomic_swap_paused":compatibility.atomic_swap_paused,
                "replay_or_fixture_exceptions":false});
            let mut ledger: LedgerState = serde_json::from_value(snapshot["ledger"].clone())?;
            let steps: Vec<Step> = serde_json::from_slice(&fs::read(steps)?)?;
            let mut receipts = vec![];
            let mut pass = true;
            for step in steps {
                let before = serde_json::to_value(&ledger)?;
                let receipt = if step.kind == "offer" {
                    let transaction: SignedOfferTransaction =
                        serde_json::from_value(step.transaction)?;
                    postfiat_execution::execute_offer_transaction(
                        &genesis,
                        &mut ledger,
                        &transaction,
                        step.height,
                    )
                } else {
                    ensure!(
                        step.kind.is_empty() || step.kind == "asset",
                        "unknown transaction kind"
                    );
                    let transaction: SignedAssetTransaction =
                        serde_json::from_value(step.transaction)?;
                    postfiat_execution::execute_asset_transaction_with_compatibility(
                        &genesis,
                        &mut ledger,
                        &transaction,
                        step.height,
                        compatibility,
                    )
                };
                let value = serde_json::to_value(&receipt)?;
                let accepted = receipt.accepted == step.expect_accepted
                    && step
                        .expect_code
                        .as_ref()
                        .is_none_or(|code| code == &receipt.code);
                pass &= accepted;
                receipts.push(serde_json::json!({"label":step.label,"height":step.height,"expected_accepted":step.expect_accepted,"expected_code":step.expect_code,"matched":accepted,"ledger_changed":before!=serde_json::to_value(&ledger)?,"receipt":value}));
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
