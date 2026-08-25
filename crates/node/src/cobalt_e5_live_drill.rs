//! Read-only adversarial checks against a live Cobalt authority state.
//!
//! The drill reads the persisted genesis, governance history, registry, chain
//! tip, and one explicitly selected validator key. It never writes into the
//! node data directory. Mutated transitions and the stolen-key rotation are
//! verified in memory through the production Cobalt verification functions.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use postfiat_consensus_cobalt::{
    certify_validator_registry_update_with_trust_graph_transition, trust_graph_transition_id,
    CobaltDomain, EssentialSubsetConfig, TrustGraphTransition, ValidatorRegistryUpdateRequest,
    VALIDATOR_REGISTRY_OP_ROTATE_KEY,
};
use postfiat_crypto_provider::{
    bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context,
    ML_DSA_65_ALGORITHM,
};
use postfiat_execution::genesis_hash;
use postfiat_storage::NodeStore;
use postfiat_types::{
    SignedCobaltValidatorUpdateAuthorizationV1, ValidatorRegistryEntry,
    COBALT_AUTHORITY_TRANSITION_ACTIVATE, COBALT_AUTHORITY_TRANSITION_ROLLBACK,
    GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED, GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
    SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::cobalt_handoff::{
    build_unsigned_cobalt_authority_transition, cobalt_authority_transition_id,
    cobalt_validator_update_authorization_signing_bytes, verify_cobalt_authority_history,
    verify_cobalt_authority_transition, verify_cobalt_validator_trust_update,
    COBALT_VALIDATOR_UPDATE_SIGNATURE_CONTEXT_V1,
};
use crate::{
    bft_quorum_threshold, read_validator_key_file, read_validator_registry_file,
    validator_key_record, validator_registry_root, VALIDATOR_REGISTRY_FILE,
};

const RESULT_SCHEMA: &str = "postfiat-cobalt-adversarial-e5-live-negative-v1";
const MAX_STATE_BYTES: u64 = 32 * 1024 * 1024;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn required_arg(args: &[String], name: &str) -> io::Result<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| PathBuf::from(&pair[1]))
        .ok_or_else(|| invalid(format!("missing required argument {name}")))
}

fn required_string(args: &[String], name: &str) -> io::Result<String> {
    required_arg(args, name)?
        .into_os_string()
        .into_string()
        .map_err(|_| invalid(format!("argument {name} is not UTF-8")))
}

fn bounded_digest(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_STATE_BYTES {
        return Err(invalid(format!(
            "{} exceeds the read-only drill limit",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes_to_hex(&Sha256::digest(bytes)))
}

fn rejection(error: io::Error) -> Value {
    json!({"rejected": true, "reason": error.to_string()})
}

fn current_progress(
    governance: &postfiat_types::GovernanceState,
) -> io::Result<(String, String, u64)> {
    let transition = governance
        .cobalt_authority_transitions
        .last()
        .ok_or_else(|| invalid("live Cobalt state has no authority transition"))?;
    let mut parent_lock_hash = transition.cobalt_lock_hash.clone();
    let mut trust_graph_root = transition.trust_graph_root.clone();
    let mut amendment_sequence = transition.amendment_sequence;
    for update in &governance.validator_registry_updates {
        let Some(first) = update.cobalt_authorizations.first() else {
            continue;
        };
        if first.authority_transition_id == transition.transition_id
            && first.amendment_sequence > amendment_sequence
        {
            parent_lock_hash = update.update_id.clone();
            trust_graph_root = update
                .new_trust_graph_root
                .clone()
                .ok_or_else(|| invalid("live Cobalt update has no trust graph root"))?;
            amendment_sequence = first.amendment_sequence;
        }
    }
    Ok((parent_lock_hash, trust_graph_root, amendment_sequence))
}

fn run(args: &[String]) -> io::Result<Value> {
    let data_dir = required_arg(args, "--data-dir")?;
    let validator_key_file = required_arg(args, "--validator-key-file")?;
    let stolen_validator = required_string(args, "--stolen-validator")?;
    let source_commit = required_string(args, "--source-commit")?;
    if source_commit.len() != 40 || !source_commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(
            "--source-commit must be a full Git commit identity",
        ));
    }

    let governance_path = data_dir.join("governance.json");
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let governance_sha256_before = bounded_digest(&governance_path)?;
    let registry_sha256_before = bounded_digest(&registry_path)?;

    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let registry = read_validator_registry_file(&registry_path)?;
    let tip = store.read_chain_tip()?;
    verify_cobalt_authority_history(&genesis, &governance)?;
    if governance.authority_mode != GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED {
        return Err(invalid("stolen-key drill requires live Cobalt authority"));
    }
    let validators = governance.active_validators.clone();
    let quorum =
        bft_quorum_threshold(validators.len()).map_err(|error| invalid(error.to_string()))?;
    if validators.len() != 6 || quorum != 5 || !validators.contains(&stolen_validator) {
        return Err(invalid(
            "live validator set is not the locked six-of-five topology",
        ));
    }
    let registry_root = validator_registry_root(&registry, &validators)?;
    let next_height = tip
        .height
        .checked_add(1)
        .ok_or_else(|| invalid("chain height overflow"))?;
    let unsigned = build_unsigned_cobalt_authority_transition(
        &genesis,
        &governance,
        &registry,
        next_height,
        None,
        None,
    )?;

    let early =
        verify_cobalt_authority_transition(&genesis, &governance, &registry, &unsigned, tip.height)
            .expect_err("early transition must reject");

    let mut stale = unsigned.clone();
    stale.activation_height = governance
        .cobalt_authority_transitions
        .last()
        .ok_or_else(|| invalid("missing current transition"))?
        .activation_height;
    stale.transition_id = cobalt_authority_transition_id(&stale)?;
    let stale_error = verify_cobalt_authority_transition(
        &genesis,
        &governance,
        &registry,
        &stale,
        stale.activation_height,
    )
    .expect_err("stale transition must reject");

    let latest = governance
        .cobalt_authority_transitions
        .last()
        .ok_or_else(|| invalid("missing current transition"))?;
    let replay_error =
        verify_cobalt_authority_transition(&genesis, &governance, &registry, latest, next_height)
            .expect_err("accepted transition replay must reject");

    let mut wrong_root = unsigned.clone();
    wrong_root.old_registry_root = "ff".repeat(48);
    wrong_root.cobalt_registry_root = wrong_root.old_registry_root.clone();
    wrong_root.transition_id = cobalt_authority_transition_id(&wrong_root)?;
    let wrong_root_error = verify_cobalt_authority_transition(
        &genesis,
        &governance,
        &registry,
        &wrong_root,
        next_height,
    )
    .expect_err("wrong-root transition must reject");

    let mut cross_chain = unsigned.clone();
    cross_chain.chain_id.push_str("-adversarial");
    cross_chain.transition_id = cobalt_authority_transition_id(&cross_chain)?;
    let cross_chain_error = verify_cobalt_authority_transition(
        &genesis,
        &governance,
        &registry,
        &cross_chain,
        next_height,
    )
    .expect_err("cross-chain transition must reject");

    let mut mixed_authority = unsigned.clone();
    mixed_authority.from_authority_mode = GOVERNANCE_AUTHORITY_MODE_FOUNDATION;
    mixed_authority.to_authority_mode = GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED;
    mixed_authority.transition_kind = COBALT_AUTHORITY_TRANSITION_ACTIVATE.to_string();
    mixed_authority.transition_id = cobalt_authority_transition_id(&mixed_authority)?;
    let mixed_authority_error = verify_cobalt_authority_transition(
        &genesis,
        &governance,
        &registry,
        &mixed_authority,
        next_height,
    )
    .expect_err("mixed-authority transition must reject");

    let mut self_authorized = unsigned.clone();
    self_authorized.validators.pop();
    self_authorized.approval_quorum = quorum - 1;
    self_authorized.transition_id = cobalt_authority_transition_id(&self_authorized)?;
    let self_authorized_error = verify_cobalt_authority_transition(
        &genesis,
        &governance,
        &registry,
        &self_authorized,
        next_height,
    )
    .expect_err("self-authorized transition must reject");

    let replayed_rollback = governance
        .cobalt_authority_transitions
        .iter()
        .rev()
        .find(|transition| transition.transition_kind == COBALT_AUTHORITY_TRANSITION_ROLLBACK)
        .map(|rollback| {
            rejection(
                verify_cobalt_authority_transition(
                    &genesis,
                    &governance,
                    &registry,
                    rollback,
                    next_height,
                )
                .expect_err("accepted rollback replay must reject"),
            )
        });

    let (parent_lock_hash, trust_graph_root, amendment_sequence) = current_progress(&governance)?;
    let attacker_key = ml_dsa_65_keygen_from_seed(&[0xE5; 32]);
    let replacement = ValidatorRegistryEntry {
        node_id: stolen_validator.clone(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&attacker_key.public_key),
        active: true,
    };
    let mut updated_registry = registry.clone();
    let subject_index = updated_registry
        .validators
        .iter()
        .position(|entry| entry.node_id == stolen_validator)
        .ok_or_else(|| invalid("stolen validator is absent from the registry"))?;
    let previous_record = updated_registry.validators[subject_index].clone();
    updated_registry.validators[subject_index].public_key_hex = replacement.public_key_hex.clone();
    let new_registry_root = validator_registry_root(&updated_registry, &validators)?;
    let new_trust_graph_root = hash_hex(
        "postfiat.cobalt.adversarial.e5.stolen-key-trust-root.v1",
        new_registry_root.as_bytes(),
    );
    let domain = CobaltDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
    };
    let mut graph_transition = TrustGraphTransition {
        previous_registry_root: registry_root.clone(),
        new_registry_root: new_registry_root.clone(),
        previous_trust_graph_root: trust_graph_root.clone(),
        new_trust_graph_root: new_trust_graph_root.clone(),
        activation_height: next_height,
        transition_id: String::new(),
    };
    graph_transition.transition_id =
        trust_graph_transition_id(&domain, &graph_transition).map_err(invalid)?;
    let request = ValidatorRegistryUpdateRequest {
        activation_height: next_height,
        previous_registry_root: registry_root.clone(),
        new_registry_root: new_registry_root.clone(),
        previous_trust_graph_root: Some(trust_graph_root),
        new_trust_graph_root: Some(new_trust_graph_root),
        trust_graph_transition_id: Some(graph_transition.transition_id.clone()),
        previous_validators: validators.clone(),
        new_validators: validators.clone(),
        operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
        subject_node_id: stolen_validator.clone(),
        previous_record: Some(ValidatorRegistryEntry {
            node_id: previous_record.node_id,
            algorithm_id: previous_record.algorithm_id,
            public_key_hex: previous_record.public_key_hex,
            active: true,
        }),
        new_record: Some(replacement),
    };
    let mut stolen_update = certify_validator_registry_update_with_trust_graph_transition(
        &domain,
        &EssentialSubsetConfig {
            validators: validators.clone(),
            quorum,
        },
        request,
        graph_transition,
        validators.clone(),
    )
    .map_err(invalid)?;
    let key_file = read_validator_key_file(&validator_key_file)?;
    let key = validator_key_record(&key_file, &stolen_validator)?;
    let active_record = updated_registry
        .validators
        .iter()
        .find(|entry| entry.node_id == stolen_validator)
        .ok_or_else(|| invalid("missing updated registry subject"))?;
    if key.algorithm_id != ML_DSA_65_ALGORITHM
        || key.public_key_hex == active_record.public_key_hex
        || key.public_key_hex != previous_record_public_key(&registry, &stolen_validator)?
    {
        return Err(invalid(
            "stolen key does not match the current active registry record",
        ));
    }
    let mut authorization = SignedCobaltValidatorUpdateAuthorizationV1 {
        schema: SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1.to_string(),
        validator: stolen_validator.clone(),
        authority_transition_id: latest.transition_id.clone(),
        parent_cobalt_lock_hash: parent_lock_hash,
        amendment_sequence: amendment_sequence + 1,
        proposal_slot: next_height,
        expires_at_height: next_height + 10,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signature_hex: String::new(),
    };
    let private_key = Zeroizing::new(
        hex_to_bytes(&key.private_key_hex).map_err(|error| invalid(error.to_string()))?,
    );
    authorization.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &private_key,
            &cobalt_validator_update_authorization_signing_bytes(&stolen_update, &authorization)?,
            COBALT_VALIDATOR_UPDATE_SIGNATURE_CONTEXT_V1,
        )
        .map_err(|error| invalid(error.to_string()))?,
    );
    stolen_update.cobalt_authorizations = vec![authorization];
    let stolen_key_error = verify_cobalt_validator_trust_update(
        &genesis,
        &governance,
        &registry,
        &stolen_update,
        next_height,
    )
    .expect_err("one stolen key must not authorize a registry rotation");

    let governance_sha256_after = bounded_digest(&governance_path)?;
    let registry_sha256_after = bounded_digest(&registry_path)?;
    if governance_sha256_before != governance_sha256_after
        || registry_sha256_before != registry_sha256_after
    {
        return Err(invalid("read-only drill observed durable state mutation"));
    }
    let cases = json!({
        "early": rejection(early),
        "stale": rejection(stale_error),
        "replayed": rejection(replay_error),
        "wrong_root": rejection(wrong_root_error),
        "cross_chain": rejection(cross_chain_error),
        "mixed_authority": rejection(mixed_authority_error),
        "self_authorized": rejection(self_authorized_error),
        "replayed_rollback": replayed_rollback,
        "stolen_key_rotation": {
            "rejected": true,
            "reason": stolen_key_error.to_string(),
            "stolen_validator": stolen_validator,
            "signature_count": 1,
            "decision_certificate_present": false,
            "attacker_replacement_public_key_sha256": bytes_to_hex(&Sha256::digest(attacker_key.public_key)),
        },
    });
    Ok(json!({
        "schema": RESULT_SCHEMA,
        "status": "passed",
        "source_commit": source_commit,
        "chain": {
            "chain_id": genesis.chain_id,
            "genesis_hash": genesis_hash(&genesis),
            "observed_height": tip.height,
            "block_tip_hash": tip.block_hash,
            "state_root": tip.state_root,
        },
        "authority": {
            "mode": governance.authority_mode,
            "transition_id": latest.transition_id,
            "registry_root": registry_root,
            "validator_count": validators.len(),
            "approval_quorum": quorum,
        },
        "cases": cases,
        "checks": {
            "all_required_pre_transition_cases_rejected": true,
            "stolen_key_rotation_rejected": true,
            "durable_state_unchanged": true,
            "production_verifier_path": true,
            "private_key_material_redacted": true,
        },
        "state_files": {
            "governance_sha256_before": governance_sha256_before,
            "governance_sha256_after": governance_sha256_after,
            "registry_sha256_before": registry_sha256_before,
            "registry_sha256_after": registry_sha256_after,
        },
        "claims_not_made": [
            "the adversary possessed more than one active validator key",
            "the adversary produced a Cobalt protocol decision certificate",
            "the drill mutated live validator state",
        ],
    }))
}

fn previous_record_public_key(
    registry: &crate::ValidatorRegistry,
    validator: &str,
) -> io::Result<String> {
    registry
        .validators
        .iter()
        .find(|entry| entry.node_id == validator)
        .map(|entry| entry.public_key_hex.clone())
        .ok_or_else(|| invalid("validator is absent from the current registry"))
}

fn usage() -> ! {
    eprintln!(
        "usage: postfiat-cobalt-e5-live-drill --data-dir PATH --validator-key-file PATH \
         --stolen-validator ID --source-commit COMMIT"
    );
    std::process::exit(2);
}

pub fn main() -> io::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|arg| arg == "--help") {
        usage();
    }
    let report = run(&args)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| invalid(error.to_string()))?
    );
    Ok(())
}
