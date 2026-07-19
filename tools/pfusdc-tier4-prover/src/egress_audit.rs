use std::{fs, path::PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use postfiat_pfusdc_proofs::verify_egress_witness_v1;
use postfiat_types::PfUsdcEgressProofWitnessV1;
use serde_json::{json, Value};

#[derive(Debug, Clone, Args)]
pub struct EgressAuditArgs {
    #[arg(long)]
    pub witness: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}

pub fn audit(args: EgressAuditArgs) -> Result<()> {
    let bytes =
        fs::read(&args.witness).with_context(|| format!("read {}", args.witness.display()))?;
    let original_json: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode {} as JSON", args.witness.display()))?;
    let original: PfUsdcEgressProofWitnessV1 =
        serde_json::from_value(original_json.clone()).context("decode base egress witness")?;
    verify_egress_witness_v1(&original)
        .map_err(|error| anyhow!("base egress witness failed verification: {error}"))?;

    let mut results = Vec::new();
    macro_rules! reject_case {
        ($name:literal, $mutation:expr) => {{
            results.push(reject_mutation($name, &original_json, $mutation)?);
        }};
    }

    reject_case!("wrong_chain", |value| mutate_scalar(value, "/chain_id"));
    reject_case!("wrong_genesis", |value| mutate_scalar(
        value,
        "/genesis_hash"
    ));
    reject_case!("stale_checkpoint", |value| {
        mutate_scalar(value, "/prior_checkpoint_block_id")
    });
    reject_case!("wrong_bridge_exit_activation", activation_after_block);
    reject_case!("wrong_route_profile_hash", |value| {
        mutate_scalar(value, "/route_profile/profile_hash")
    });
    reject_case!("wrong_committed_bridge_exit_root", |value| {
        mutate_scalar(value, "/block/header/bridge_exit_root")
    });
    reject_case!("proposal_only", |value| {
        set_value(value, "/block/header/consensus_v2_commit", Value::Null)
    });
    reject_case!("prepare_only", |value| {
        set_value(
            value,
            "/block/header/consensus_v2_commit/precommit_qc/phase",
            Value::String("prepare".to_string()),
        )
    });
    reject_case!("four_of_six_or_under_quorum", under_quorum);
    reject_case!("duplicate_validator", duplicate_validator);
    reject_case!("wrong_committee", |value| {
        mutate_scalar(value, "/committee/0/public_key_hex")
    });
    reject_case!("bad_mldsa_signature_or_context", |value| {
        mutate_scalar(
            value,
            "/block/header/consensus_v2_commit/precommit_qc/votes/0/signature/signature_hex",
        )
    });
    reject_case!("rejected_receipt", |value| {
        set_value(value, "/receipt/accepted", Value::Bool(false))
    });
    reject_case!("wrong_receipt_code", |value| {
        set_value(
            value,
            "/receipt/code",
            Value::String("rejected".to_string()),
        )
    });
    reject_case!("wrong_merkle_path", |value| {
        increment_u64(value, "/merkle_proof/leaf_index")
    });
    reject_case!("altered_exit_leaf", |value| {
        increment_u64(value, "/merkle_proof/leaf/amount_atoms")
    });
    reject_case!("altered_withdrawal_packet", |value| {
        increment_u64(value, "/withdrawal_packet/amount_atoms")
    });
    reject_case!("wrong_recipient", |value| {
        mutate_scalar(value, "/withdrawal_packet/recipient")
    });
    reject_case!("wrong_withdrawal_packet_hash", |value| {
        mutate_scalar(value, "/withdrawal_packet_hash")
    });
    reject_case!("wrong_withdrawal_packet_evm_digest", |value| {
        mutate_scalar(value, "/withdrawal_packet_evm_digest")
    });

    let case_count = results.len();
    let report = json!({
        "schema": "postfiat.pfusdc.egress_mutation_report.v1",
        "witness": args.witness,
        "base_verified": true,
        "cases": results,
        "passed": case_count,
        "failed": 0,
        "note": "EVM proof and withdrawal replay are checked after the single release proof is submitted; this bounded native matrix generates no proof"
    });
    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    println!("egress mutation matrix: {case_count} rejected, 0 accepted");
    println!("wrote {}", args.output.display());
    Ok(())
}

fn reject_mutation(
    name: &str,
    original: &Value,
    mutation: impl FnOnce(&mut Value) -> Result<()>,
) -> Result<Value> {
    let mut mutated = original.clone();
    mutation(&mut mutated).with_context(|| format!("apply mutation {name}"))?;
    if mutated == *original {
        bail!("mutation {name} made no change");
    }
    let rejection = match serde_json::from_value::<PfUsdcEgressProofWitnessV1>(mutated) {
        Ok(witness) => verify_egress_witness_v1(&witness)
            .err()
            .ok_or_else(|| anyhow!("mutation {name} was accepted"))?,
        Err(error) => format!("decode rejected: {error}"),
    };
    Ok(json!({
        "case": name,
        "rejected": true,
        "reason": rejection,
    }))
}

fn pointer_mut<'a>(value: &'a mut Value, pointer: &str) -> Result<&'a mut Value> {
    value
        .pointer_mut(pointer)
        .ok_or_else(|| anyhow!("missing JSON pointer {pointer}"))
}

fn set_value(value: &mut Value, pointer: &str, replacement: Value) -> Result<()> {
    *pointer_mut(value, pointer)? = replacement;
    Ok(())
}

fn mutate_scalar(value: &mut Value, pointer: &str) -> Result<()> {
    match pointer_mut(value, pointer)? {
        Value::String(text) => {
            let last = text
                .pop()
                .ok_or_else(|| anyhow!("cannot mutate empty string"))?;
            text.push(if last == '0' { '1' } else { '0' });
            Ok(())
        }
        other => bail!("mutation target {pointer} is not a string: {other}"),
    }
}

fn increment_u64(value: &mut Value, pointer: &str) -> Result<()> {
    let target = pointer_mut(value, pointer)?;
    let current = target
        .as_u64()
        .ok_or_else(|| anyhow!("mutation target {pointer} is not a u64"))?;
    *target = Value::from(
        current
            .checked_add(1)
            .ok_or_else(|| anyhow!("mutation target {pointer} overflowed"))?,
    );
    Ok(())
}

fn activation_after_block(value: &mut Value) -> Result<()> {
    let height = value
        .pointer("/block/header/height")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("missing block height"))?;
    set_value(
        value,
        "/bridge_exit_root_activation_height",
        Value::from(
            height
                .checked_add(1)
                .ok_or_else(|| anyhow!("block height overflowed"))?,
        ),
    )
}

fn under_quorum(value: &mut Value) -> Result<()> {
    let quorum = value
        .pointer("/block/header/consensus_v2_commit/precommit_qc/quorum")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("missing precommit quorum"))? as usize;
    let votes = pointer_mut(
        value,
        "/block/header/consensus_v2_commit/precommit_qc/votes",
    )?
    .as_array_mut()
    .ok_or_else(|| anyhow!("precommit votes are not an array"))?;
    if quorum == 0 || votes.len() < quorum {
        bail!("base precommit vote set is already below quorum");
    }
    votes.truncate(quorum - 1);
    Ok(())
}

fn duplicate_validator(value: &mut Value) -> Result<()> {
    let committee = pointer_mut(value, "/committee")?
        .as_array_mut()
        .ok_or_else(|| anyhow!("committee is not an array"))?;
    let first = committee
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("committee is empty"))?;
    committee.push(first);
    Ok(())
}
