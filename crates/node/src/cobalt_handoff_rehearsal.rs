//! Disposable-clone rehearsal for the versioned Foundation-to-Cobalt handoff.
//!
//! The rehearsal never mutates a live validator. Remote signing commands read a
//! validator key file and write one signed artifact to stdout; all state
//! transitions are applied only to the clone supplied by the operator.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::cobalt_authority_certificate::{
    cobalt_validator_update_payload_hash, compact_cobalt_validator_update_decision_certificate,
    verify_cobalt_validator_update_decision_certificate,
};
use crate::cobalt_handoff::{
    apply_cobalt_authority_transition, cobalt_authority_transition_approval_signing_bytes,
    cobalt_authority_transition_id, cobalt_governance_state_commitment,
    cobalt_validator_update_authorization_signing_bytes, verify_cobalt_authority_history,
    verify_cobalt_authority_transition, verify_cobalt_scoped_governance_batch,
    verify_cobalt_validator_trust_update, COBALT_AUTHORITY_TRANSITION_SIGNATURE_CONTEXT_V1,
    COBALT_VALIDATOR_UPDATE_SIGNATURE_CONTEXT_V1,
};
use crate::{
    validator_key_record, validator_registry_root, ValidatorKeyFile, ValidatorKeyRecord,
    ValidatorRegistry,
};
use postfiat_consensus_cobalt::{
    certify_validator_registry_update_with_trust_graph_transition,
    ratify_governance_amendment_with_lifecycle, trust_graph_transition_id, EssentialSubsetConfig,
    GovernanceAmendmentLifecycle, TrustGraphTransition, ValidatorRegistryUpdateRequest,
    VALIDATOR_REGISTRY_OP_ROTATE_KEY,
};
use postfiat_crypto_provider::{
    bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context,
    ML_DSA_65_ALGORITHM,
};
use postfiat_execution::genesis_hash;
use postfiat_types::{
    CobaltGovernanceAuthorityTransitionV1, CobaltValidatorUpdateDecisionCertificateV1, Genesis,
    GovernanceState, SignedCobaltAuthorityTransitionApprovalV1,
    SignedCobaltValidatorUpdateAuthorizationV1, ValidatorRegistryEntry,
    ValidatorRegistryUpdateRecord, COBALT_AUTHORITY_SCOPE_VALIDATOR_TRUST_V1,
    COBALT_AUTHORITY_TRANSITION_ACTIVATE, COBALT_AUTHORITY_TRANSITION_ROLLBACK,
    COBALT_AUTHORITY_TRANSITION_SCHEMA_V1, GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
    GOVERNANCE_AUTHORITY_MODE_FOUNDATION, GOVERNANCE_KIND_CRYPTO_POLICY,
    SIGNED_COBALT_AUTHORITY_TRANSITION_APPROVAL_SCHEMA_V1,
    SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1,
};
use serde::Deserialize;
use serde_json::{json, Value};
use zeroize::Zeroizing;

const MANIFEST_SCHEMA: &str = "postfiat-cobalt-handoff-clone-manifest-v1";
const RESULT_SCHEMA: &str = "postfiat-cobalt-handoff-rehearsal-result-v1";

#[derive(Debug, Deserialize)]
struct CloneManifest {
    schema: String,
    source_commit: String,
    genesis: Genesis,
    registry: ValidatorRegistry,
    registry_root: String,
    trust_graph_root: String,
    cobalt_lock_hash: String,
    anchor_height: u64,
    anchor_genesis_hash: String,
    anchor_block_hash: String,
    anchor_state_root: String,
    activation_height: u64,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> io::Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(16 * 1024 * 1024)
        .read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let encoded = serde_json::to_vec_pretty(value).map_err(|error| invalid(error.to_string()))?;
    fs::write(path, encoded)?;
    Ok(())
}

fn optional_arg(args: &[String], name: &str) -> Option<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| PathBuf::from(&pair[1]))
}

fn required_arg(args: &[String], name: &str) -> io::Result<PathBuf> {
    optional_arg(args, name).ok_or_else(|| invalid(format!("missing required argument {name}")))
}

fn required_u64(args: &[String], name: &str) -> io::Result<u64> {
    let value = required_arg(args, name)?;
    value
        .to_str()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| invalid(format!("invalid integer argument {name}")))
}

fn validate_manifest(manifest: &CloneManifest) -> io::Result<()> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(invalid("unsupported handoff clone manifest schema"));
    }
    if manifest.activation_height <= manifest.anchor_height {
        return Err(invalid(
            "activation height must be after the cloned chain tip",
        ));
    }
    if manifest.source_commit.len() < 7 || manifest.source_commit.len() > 40 {
        return Err(invalid("source commit is not a Git commit identity"));
    }
    for (label, value) in [
        ("anchor genesis hash", &manifest.anchor_genesis_hash),
        ("anchor block hash", &manifest.anchor_block_hash),
        ("anchor state root", &manifest.anchor_state_root),
    ] {
        if value.len() != 96
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
        {
            return Err(invalid(format!(
                "{label} is not a 96-character lowercase hex digest"
            )));
        }
    }
    let validators = manifest
        .registry
        .validators
        .iter()
        .map(|record| record.node_id.clone())
        .collect::<Vec<_>>();
    if validators.len() != manifest.genesis.validator_count as usize {
        return Err(invalid("registry and genesis validator counts disagree"));
    }
    let root = validator_registry_root(&manifest.registry, &validators)?;
    if root != manifest.registry_root {
        return Err(invalid("clone registry root mismatch"));
    }
    if genesis_hash(&manifest.genesis) != manifest.anchor_genesis_hash {
        return Err(invalid("clone genesis does not match its anchor identity"));
    }
    for label in ["trust graph root", "Cobalt lock"] {
        let value = if label == "trust graph root" {
            &manifest.trust_graph_root
        } else {
            &manifest.cobalt_lock_hash
        };
        if value.len() != 96
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
        {
            return Err(invalid(format!(
                "{label} is not a 96-character lowercase hex digest"
            )));
        }
    }
    Ok(())
}

impl CloneManifest {
    fn validators(&self) -> Vec<String> {
        self.registry
            .validators
            .iter()
            .map(|record| record.node_id.clone())
            .collect()
    }

    fn initial_governance(&self) -> GovernanceState {
        let mut governance = GovernanceState::new(self.validators().len() as u32);
        governance.active_validators = self.validators();
        governance
    }
}

fn state_commitment_hex(governance: &GovernanceState) -> String {
    hash_hex(
        "postfiat.cobalt.handoff-rehearsal.governance-commitment.v1",
        &cobalt_governance_state_commitment(governance),
    )
}

fn unsigned_transition(
    manifest: &CloneManifest,
    governance: &GovernanceState,
    height: u64,
    protocol_version: u32,
    sequence: u64,
    previous_transition_id: Option<String>,
    lock_hash: String,
    trust_graph_root: String,
    old_registry_root: String,
    cobalt_registry_root: String,
    validators: Vec<String>,
    approval_quorum: usize,
) -> io::Result<CobaltGovernanceAuthorityTransitionV1> {
    let from = governance.authority_mode;
    let (to, kind) = if from == GOVERNANCE_AUTHORITY_MODE_FOUNDATION {
        (
            GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
            COBALT_AUTHORITY_TRANSITION_ACTIVATE,
        )
    } else {
        (
            GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
            COBALT_AUTHORITY_TRANSITION_ROLLBACK,
        )
    };
    let mut transition = CobaltGovernanceAuthorityTransitionV1 {
        schema: COBALT_AUTHORITY_TRANSITION_SCHEMA_V1.to_string(),
        transition_id: String::new(),
        chain_id: manifest.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&manifest.genesis),
        from_authority_mode: from,
        to_authority_mode: to,
        transition_kind: kind.to_string(),
        previous_transition_id,
        old_registry_root,
        cobalt_lock_hash: lock_hash,
        trust_graph_root,
        cobalt_registry_root,
        amendment_sequence: sequence,
        activation_height: height,
        cobalt_protocol_version: protocol_version,
        authority_scope: COBALT_AUTHORITY_SCOPE_VALIDATOR_TRUST_V1.to_string(),
        validators,
        approval_quorum,
        approvals: Vec::new(),
    };
    transition.transition_id = cobalt_authority_transition_id(&transition)?;
    Ok(transition)
}

fn prepare(manifest_path: &Path, output: &Path) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let governance = manifest.initial_governance();
    let quorum = crate::bft_quorum_threshold(manifest.validators().len())
        .map_err(|error| invalid(error.to_string()))?;
    let transition = unsigned_transition(
        &manifest,
        &governance,
        manifest.activation_height,
        manifest.genesis.protocol_version + 1,
        1,
        None,
        manifest.cobalt_lock_hash.clone(),
        manifest.trust_graph_root.clone(),
        manifest.registry_root.clone(),
        manifest.registry_root.clone(),
        manifest.validators(),
        quorum,
    )?;
    write_json(
        output,
        &json!({
            "schema": "postfiat-cobalt-handoff-unsigned-activation-v1",
            "clone_manifest_sha_input": manifest_path.display().to_string(),
            "transition": transition,
            "governance_commitment_before": state_commitment_hex(&governance),
        }),
    )
}

fn key_record(key_file: &Path, validator: &str) -> io::Result<ValidatorKeyRecord> {
    let parsed = read_json::<ValidatorKeyFile>(key_file)?;
    let _ = validator_key_record(&parsed, validator)?;
    Ok(parsed
        .validators
        .into_iter()
        .find(|record| record.node_id == validator)
        .expect("checked"))
}

fn sign_transition(args: &[String]) -> io::Result<()> {
    let transition_path = required_arg(args, "--transition")?;
    let key_path = required_arg(args, "--key-file")?;
    let validator = required_arg(args, "--validator")?;
    let validator = validator
        .to_str()
        .ok_or_else(|| invalid("invalid validator"))?;
    let wrapper = read_json::<Value>(&transition_path)?;
    let transition: CobaltGovernanceAuthorityTransitionV1 = serde_json::from_value(
        wrapper
            .get("transition")
            .cloned()
            .unwrap_or(wrapper.clone()),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let key = key_record(&key_path, validator)?;
    if key.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(invalid(
            "handoff rehearsal requires ML-DSA-65 validator key",
        ));
    }
    let mut approval = SignedCobaltAuthorityTransitionApprovalV1 {
        schema: SIGNED_COBALT_AUTHORITY_TRANSITION_APPROVAL_SCHEMA_V1.to_string(),
        validator: validator.to_string(),
        old_registry_root: transition.old_registry_root.clone(),
        proposal_slot: transition.activation_height,
        expires_at_height: transition.activation_height + 10,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signature_hex: String::new(),
    };
    let private_key = Zeroizing::new(
        hex_to_bytes(&key.private_key_hex).map_err(|error| invalid(error.to_string()))?,
    );
    approval.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            private_key.as_slice(),
            &cobalt_authority_transition_approval_signing_bytes(&transition, &approval)?,
            COBALT_AUTHORITY_TRANSITION_SIGNATURE_CONTEXT_V1,
        )
        .map_err(|error| invalid(error.to_string()))?,
    );
    let encoded =
        serde_json::to_vec_pretty(&approval).map_err(|error| invalid(error.to_string()))?;
    io::stdout().write_all(&encoded)?;
    io::stdout().write_all(b"\n")?;
    Ok(())
}

fn insert_transition_approvals(
    mut transition: CobaltGovernanceAuthorityTransitionV1,
    approvals: Vec<SignedCobaltAuthorityTransitionApprovalV1>,
) -> io::Result<CobaltGovernanceAuthorityTransitionV1> {
    let mut by_validator = BTreeMap::<String, SignedCobaltAuthorityTransitionApprovalV1>::new();
    for approval in approvals {
        if by_validator
            .insert(approval.validator.clone(), approval)
            .is_some()
        {
            return Err(invalid("duplicate transition approval validator"));
        }
    }
    transition.approvals = by_validator.into_values().collect();
    Ok(transition)
}

fn load_transition(path: &Path) -> io::Result<CobaltGovernanceAuthorityTransitionV1> {
    let wrapper = read_json::<Value>(path)?;
    serde_json::from_value(wrapper.get("transition").cloned().unwrap_or(wrapper))
        .map_err(|error| invalid(error.to_string()))
}

fn finalize_activation(
    manifest_path: &Path,
    transition_path: &Path,
    approvals_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let mut governance = manifest.initial_governance();
    let transition = insert_transition_approvals(
        load_transition(transition_path)?,
        read_json::<Vec<_>>(approvals_path)?,
    )?;
    verify_cobalt_authority_transition(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &transition,
        transition.activation_height,
    )?;
    let before = state_commitment_hex(&governance);
    apply_cobalt_authority_transition(&mut governance, &transition, transition.activation_height)
        .map_err(invalid)?;
    verify_cobalt_authority_history(&manifest.genesis, &governance)?;
    let after = state_commitment_hex(&governance);
    write_json(
        output,
        &json!({
            "schema": RESULT_SCHEMA,
            "operation": "activation",
            "accepted": true,
            "authority_mode_before": GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
            "authority_mode_after": governance.authority_mode,
            "governance_commitment_before": before,
            "governance_commitment_after": after,
            "transition_id": transition.transition_id,
            "governance": governance,
        }),
    )
}

fn abort_activation(
    manifest_path: &Path,
    transition_path: &Path,
    approvals_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let governance = manifest.initial_governance();
    let transition = insert_transition_approvals(
        load_transition(transition_path)?,
        read_json(approvals_path)?,
    )?;
    verify_cobalt_authority_transition(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &transition,
        transition.activation_height,
    )?;
    let commitment = state_commitment_hex(&governance);
    write_json(
        output,
        &json!({
            "schema": RESULT_SCHEMA,
            "operation": "pre_activation_abort",
            "accepted": false,
            "verified_before_abort": true,
            "applied": false,
            "authority_mode_after": governance.authority_mode,
            "governance_commitment_before": commitment,
            "governance_commitment_after": commitment,
            "transition_id": transition.transition_id,
        }),
    )
}

fn negative_cases(
    manifest_path: &Path,
    transition_path: &Path,
    update_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let governance = manifest.initial_governance();
    let transition = insert_transition_approvals(load_transition(transition_path)?, Vec::new())?;
    let before = state_commitment_hex(&governance);

    let early_error = verify_cobalt_authority_transition(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &transition,
        transition.activation_height - 1,
    )
    .expect_err("early ordering must fail")
    .to_string();

    let mut stale = transition.clone();
    stale.activation_height -= 1;
    stale.transition_id = cobalt_authority_transition_id(&stale)?;
    let stale_error = verify_cobalt_authority_transition(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &stale,
        stale.activation_height,
    )
    .expect_err("stale approval slot must fail")
    .to_string();

    let mut wrong_root = transition.clone();
    wrong_root.old_registry_root = "ff".repeat(48);
    wrong_root.cobalt_registry_root = wrong_root.old_registry_root.clone();
    wrong_root.transition_id = cobalt_authority_transition_id(&wrong_root)?;
    let wrong_root_error = verify_cobalt_authority_transition(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &wrong_root,
        wrong_root.activation_height,
    )
    .expect_err("wrong registry root must fail")
    .to_string();

    let mut self_authorized = transition.clone();
    self_authorized.validators.pop();
    self_authorized.approval_quorum = 4;
    self_authorized.cobalt_registry_root = "ee".repeat(48);
    self_authorized.transition_id = cobalt_authority_transition_id(&self_authorized)?;
    let self_authorized_error = verify_cobalt_authority_transition(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &self_authorized,
        self_authorized.activation_height,
    )
    .expect_err("self-authorized new set must fail")
    .to_string();

    // Build a valid transition first, then prove replay cannot mutate the clone.
    let mut activated = governance.clone();
    apply_cobalt_authority_transition(&mut activated, &transition, transition.activation_height)
        .map_err(invalid)?;
    let replay_error = apply_cobalt_authority_transition(
        &mut activated,
        &transition,
        transition.activation_height,
    )
    .expect_err("replay must fail")
    .to_string();

    // A structurally valid Cobalt-authorized update is inactive under Foundation authority.
    let mut update: ValidatorRegistryUpdateRecord = read_json(update_path)?;
    update
        .cobalt_authorizations
        .push(SignedCobaltValidatorUpdateAuthorizationV1 {
            schema: SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1.to_string(),
            validator: manifest.validators()[0].clone(),
            authority_transition_id: transition.transition_id.clone(),
            parent_cobalt_lock_hash: manifest.cobalt_lock_hash.clone(),
            amendment_sequence: 2,
            proposal_slot: update.activation_height,
            expires_at_height: update.activation_height + 10,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            signature_hex: "00".repeat(3309),
        });
    let mixed_authority_error = verify_cobalt_validator_trust_update(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &update,
        update.activation_height,
    )
    .expect_err("Cobalt authorization under Foundation authority must fail")
    .to_string();

    let after = state_commitment_hex(&governance);
    if before != after {
        return Err(invalid("negative cases changed durable clone state"));
    }
    write_json(
        output,
        &json!({
            "schema": "postfiat-cobalt-handoff-negative-result-v1",
            "all_rejected": true,
            "durable_state_unchanged": true,
            "cases": {
                "early": early_error,
                "stale": stale_error,
                "wrong_root": wrong_root_error,
                "self_authorized": self_authorized_error,
                "replayed": replay_error,
                "mixed_authority": mixed_authority_error,
            },
        }),
    )
}

fn prepare_update(
    manifest_path: &Path,
    activation_result_path: &Path,
    replacement_record_path: Option<&Path>,
    new_trust_graph_root: Option<&str>,
    output: &Path,
    registry_output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let activation: Value = read_json(activation_result_path)?;
    let governance_value = activation.get("governance").cloned().unwrap_or(activation);
    let governance: GovernanceState =
        serde_json::from_value(governance_value).map_err(|error| invalid(error.to_string()))?;
    let transition = governance
        .cobalt_authority_transitions
        .last()
        .ok_or_else(|| invalid("activation result is not in Cobalt authority mode"))?;
    let height = transition.activation_height + 1;
    let replacement = match replacement_record_path {
        Some(path) => read_json::<ValidatorRegistryEntry>(path)?,
        None => {
            let key = ml_dsa_65_keygen_from_seed(&[0xC0; 32]);
            ValidatorRegistryEntry {
                node_id: "validator-5".to_string(),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: bytes_to_hex(&key.public_key),
                active: true,
            }
        }
    };
    if replacement.node_id != "validator-5"
        || replacement.algorithm_id != ML_DSA_65_ALGORITHM
        || !replacement.active
    {
        return Err(invalid(
            "replacement record must be an active ML-DSA-65 validator-5 record",
        ));
    }
    let mut updated_registry = manifest.registry.clone();
    let subject_index = updated_registry
        .validators
        .iter()
        .position(|record| record.node_id == "validator-5")
        .ok_or_else(|| invalid("clone lacks validator-5"))?;
    let previous_record = updated_registry.validators[subject_index].clone();
    if previous_record.public_key_hex == replacement.public_key_hex {
        return Err(invalid("replacement validator key must change"));
    }
    updated_registry.validators[subject_index].public_key_hex = replacement.public_key_hex.clone();
    let validators = manifest.validators();
    let new_root = validator_registry_root(&updated_registry, &validators)?;
    let new_graph_root = new_trust_graph_root.map(str::to_string).unwrap_or_else(|| {
        hash_hex(
            "postfiat.cobalt.handoff-rehearsal.trust-graph.v1",
            new_root.as_bytes(),
        )
    });
    let mut graph_transition = TrustGraphTransition {
        previous_registry_root: manifest.registry_root.clone(),
        new_registry_root: new_root.clone(),
        previous_trust_graph_root: transition.trust_graph_root.clone(),
        new_trust_graph_root: new_graph_root.clone(),
        activation_height: height,
        transition_id: String::new(),
    };
    let domain = postfiat_consensus_cobalt::CobaltDomain {
        chain_id: manifest.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&manifest.genesis),
        protocol_version: manifest.genesis.protocol_version,
    };
    graph_transition.transition_id =
        trust_graph_transition_id(&domain, &graph_transition).map_err(invalid)?;
    let request = ValidatorRegistryUpdateRequest {
        activation_height: height,
        previous_registry_root: manifest.registry_root.clone(),
        new_registry_root: new_root.clone(),
        previous_trust_graph_root: Some(transition.trust_graph_root.clone()),
        new_trust_graph_root: Some(new_graph_root.clone()),
        trust_graph_transition_id: Some(graph_transition.transition_id.clone()),
        previous_validators: validators.clone(),
        new_validators: validators,
        operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
        subject_node_id: "validator-5".to_string(),
        previous_record: Some(ValidatorRegistryEntry {
            node_id: previous_record.node_id.clone(),
            algorithm_id: previous_record.algorithm_id.clone(),
            public_key_hex: previous_record.public_key_hex.clone(),
            active: true,
        }),
        new_record: Some(replacement),
    };
    let quorum = crate::bft_quorum_threshold(manifest.validators().len())
        .map_err(|error| invalid(error.to_string()))?;
    let update = certify_validator_registry_update_with_trust_graph_transition(
        &domain,
        &EssentialSubsetConfig {
            validators: manifest.validators(),
            quorum,
        },
        request,
        graph_transition,
        manifest.validators()[..quorum].to_vec(),
    )
    .map_err(invalid)?;
    write_json(
        output,
        &serde_json::to_value(&update).map_err(|error| invalid(error.to_string()))?,
    )?;
    write_json(
        registry_output,
        &serde_json::to_value(&updated_registry).map_err(|error| invalid(error.to_string()))?,
    )?;
    Ok(())
}

fn print_update_payload_hash(update_path: &Path) -> io::Result<()> {
    let update: ValidatorRegistryUpdateRecord = read_json(update_path)?;
    let payload_hash = cobalt_validator_update_payload_hash(&update)?;
    let encoded = serde_json::to_vec_pretty(&json!({
        "schema": "postfiat-cobalt-validator-update-payload-v1",
        "payload_hash": payload_hash,
        "protocol_round": update.activation_height.checked_sub(1)
            .ok_or_else(|| invalid("validator update activation height has no protocol round"))?,
        "activation_height": update.activation_height,
    }))
    .map_err(|error| invalid(error.to_string()))?;
    io::stdout().write_all(&encoded)?;
    io::stdout().write_all(b"\n")
}

fn attach_decision_certificate(
    manifest_path: &Path,
    activation_result_path: &Path,
    update_path: &Path,
    certificate_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let activation: Value = read_json(activation_result_path)?;
    let governance: GovernanceState = serde_json::from_value(
        activation
            .get("governance")
            .cloned()
            .ok_or_else(|| invalid("activation result has no governance state"))?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let transition = governance
        .cobalt_authority_transitions
        .last()
        .ok_or_else(|| invalid("activation result has no Cobalt transition"))?;
    let mut update: ValidatorRegistryUpdateRecord = read_json(update_path)?;
    if !update.cobalt_authorizations.is_empty() || update.cobalt_decision_certificate.is_some() {
        return Err(invalid(
            "decision certificate must be attached before validator authorizations",
        ));
    }
    let certificate: CobaltValidatorUpdateDecisionCertificateV1 = read_json(certificate_path)?;
    let certificate = compact_cobalt_validator_update_decision_certificate(certificate)?;
    let payload_hash = cobalt_validator_update_payload_hash(&update)?;
    let round = update
        .activation_height
        .checked_sub(1)
        .ok_or_else(|| invalid("validator update activation height has no protocol round"))?;
    verify_cobalt_validator_update_decision_certificate(
        &certificate,
        &postfiat_consensus_cobalt::CobaltDomain {
            chain_id: manifest.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&manifest.genesis),
            protocol_version: manifest.genesis.protocol_version,
        },
        &manifest.registry,
        &manifest.validators(),
        &manifest.registry_root,
        &transition.trust_graph_root,
        &payload_hash,
        round,
        update.activation_height,
        None,
    )?;
    update.cobalt_decision_certificate = Some(certificate);
    write_json(
        output,
        &serde_json::to_value(update).map_err(|error| invalid(error.to_string()))?,
    )
}

fn sign_update(args: &[String]) -> io::Result<()> {
    let update_path = required_arg(args, "--update")?;
    let key_path = required_arg(args, "--key-file")?;
    let validator_path = required_arg(args, "--validator")?;
    let validator = validator_path
        .to_str()
        .ok_or_else(|| invalid("invalid validator"))?;
    let transition_id = required_arg(args, "--authority-transition-id")?;
    let transition_id = transition_id
        .to_str()
        .ok_or_else(|| invalid("invalid transition id"))?;
    let parent_lock = required_arg(args, "--parent-lock-hash")?;
    let parent_lock = parent_lock
        .to_str()
        .ok_or_else(|| invalid("invalid parent lock"))?;
    let sequence = required_u64(args, "--amendment-sequence")?;
    let proposal_slot = required_u64(args, "--proposal-slot")?;
    let expires_at_height = required_u64(args, "--expires-at-height")?;
    let update: ValidatorRegistryUpdateRecord = read_json(&update_path)?;
    if update.cobalt_decision_certificate.is_none() {
        return Err(invalid(
            "refusing to sign a Cobalt validator update without a protocol decision certificate",
        ));
    }
    let key = key_record(&key_path, validator)?;
    let mut authorization = SignedCobaltValidatorUpdateAuthorizationV1 {
        schema: SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1.to_string(),
        validator: validator.to_string(),
        authority_transition_id: transition_id.to_string(),
        parent_cobalt_lock_hash: parent_lock.to_string(),
        amendment_sequence: sequence,
        proposal_slot,
        expires_at_height,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signature_hex: String::new(),
    };
    let private_key = Zeroizing::new(
        hex_to_bytes(&key.private_key_hex).map_err(|error| invalid(error.to_string()))?,
    );
    authorization.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            private_key.as_slice(),
            &cobalt_validator_update_authorization_signing_bytes(&update, &authorization)?,
            COBALT_VALIDATOR_UPDATE_SIGNATURE_CONTEXT_V1,
        )
        .map_err(|error| invalid(error.to_string()))?,
    );
    let encoded =
        serde_json::to_vec_pretty(&authorization).map_err(|error| invalid(error.to_string()))?;
    io::stdout().write_all(&encoded)?;
    io::stdout().write_all(b"\n")?;
    Ok(())
}

fn finalize_update(
    manifest_path: &Path,
    activation_result_path: &Path,
    update_path: &Path,
    authorizations_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let activation: Value = read_json(activation_result_path)?;
    let mut governance: GovernanceState = serde_json::from_value(
        activation
            .get("governance")
            .cloned()
            .ok_or_else(|| invalid("missing governance"))?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let mut update: ValidatorRegistryUpdateRecord = read_json(update_path)?;
    let mut by_validator = BTreeMap::<String, SignedCobaltValidatorUpdateAuthorizationV1>::new();
    for authorization in
        read_json::<Vec<SignedCobaltValidatorUpdateAuthorizationV1>>(authorizations_path)?
    {
        by_validator.insert(authorization.validator.clone(), authorization);
    }
    update.cobalt_authorizations = by_validator.into_values().collect();
    verify_cobalt_validator_trust_update(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &update,
        update.activation_height,
    )?;

    // A non-validator governance amendment remains outside Cobalt's only authority scope.
    let quorum = crate::bft_quorum_threshold(manifest.validators().len())
        .map_err(|error| invalid(error.to_string()))?;
    let unrelated = ratify_governance_amendment_with_lifecycle(
        &postfiat_consensus_cobalt::CobaltDomain {
            chain_id: manifest.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&manifest.genesis),
            protocol_version: manifest.genesis.protocol_version,
        },
        &EssentialSubsetConfig {
            validators: manifest.validators(),
            quorum,
        },
        GOVERNANCE_KIND_CRYPTO_POLICY,
        99,
        manifest.validators()[..quorum].to_vec(),
        GovernanceAmendmentLifecycle::immediate(),
    )
    .map_err(invalid)?;
    let unrelated_error = verify_cobalt_scoped_governance_batch(
        &manifest.genesis,
        &governance,
        &manifest.registry,
        &postfiat_types::GovernanceActionBatch::new("rehearsal-unrelated", vec![unrelated]),
        update.activation_height,
    )
    .expect_err("unrelated governance kind must fail")
    .to_string();

    let before = state_commitment_hex(&governance);
    governance.validator_registry_updates.push(update.clone());
    verify_cobalt_authority_history(&manifest.genesis, &governance)?;
    let after = state_commitment_hex(&governance);
    write_json(
        output,
        &json!({
            "schema": RESULT_SCHEMA,
            "operation": "validator_trust_update",
            "accepted": true,
            "governance_commitment_before": before,
            "governance_commitment_after": after,
            "update_id": update.update_id,
            "unrelated_governance_rejected": unrelated_error,
            "governance": governance,
        }),
    )
}

fn prepare_rollback(
    manifest_path: &Path,
    update_result_path: &Path,
    updated_registry_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let update_result: Value = read_json(update_result_path)?;
    let governance: GovernanceState = serde_json::from_value(
        update_result
            .get("governance")
            .cloned()
            .ok_or_else(|| invalid("missing governance"))?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let registry: ValidatorRegistry = read_json(updated_registry_path)?;
    let new_root = validator_registry_root(&registry, &manifest.validators())?;
    let activation = governance
        .cobalt_authority_transitions
        .first()
        .ok_or_else(|| invalid("missing activation transition"))?;
    let update = governance
        .validator_registry_updates
        .last()
        .ok_or_else(|| invalid("missing validator trust update"))?;
    let height = update.activation_height + 1;
    let transition = unsigned_transition(
        &manifest,
        &governance,
        height,
        activation.cobalt_protocol_version + 1,
        update
            .cobalt_authorizations
            .first()
            .ok_or_else(|| invalid("update lacks authorization sequence"))?
            .amendment_sequence
            + 1,
        Some(activation.transition_id.clone()),
        update.update_id.clone(),
        update
            .new_trust_graph_root
            .clone()
            .ok_or_else(|| invalid("update lacks graph root"))?,
        new_root.clone(),
        new_root,
        manifest.validators(),
        crate::bft_quorum_threshold(manifest.validators().len())
            .map_err(|error| invalid(error.to_string()))?,
    )?;
    write_json(
        output,
        &json!({
            "schema": "postfiat-cobalt-handoff-unsigned-rollback-v1",
            "transition": transition,
            "governance_commitment_before": state_commitment_hex(&governance),
        }),
    )
}

fn finalize_rollback(
    manifest_path: &Path,
    update_result_path: &Path,
    updated_registry_path: &Path,
    transition_path: &Path,
    approvals_path: &Path,
    output: &Path,
) -> io::Result<()> {
    let manifest = read_json::<CloneManifest>(manifest_path)?;
    validate_manifest(&manifest)?;
    let update_result: Value = read_json(update_result_path)?;
    let mut governance: GovernanceState = serde_json::from_value(
        update_result
            .get("governance")
            .cloned()
            .ok_or_else(|| invalid("missing governance"))?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let registry: ValidatorRegistry = read_json(updated_registry_path)?;
    let transition = insert_transition_approvals(
        load_transition(transition_path)?,
        read_json(approvals_path)?,
    )?;
    verify_cobalt_authority_transition(
        &manifest.genesis,
        &governance,
        &registry,
        &transition,
        transition.activation_height,
    )?;
    let before = state_commitment_hex(&governance);
    apply_cobalt_authority_transition(&mut governance, &transition, transition.activation_height)
        .map_err(invalid)?;
    verify_cobalt_authority_history(&manifest.genesis, &governance)?;
    let after = state_commitment_hex(&governance);
    if governance.authority_mode != GOVERNANCE_AUTHORITY_MODE_FOUNDATION {
        return Err(invalid(
            "forward rollback did not restore Foundation authority",
        ));
    }
    write_json(
        output,
        &json!({
            "schema": RESULT_SCHEMA,
            "operation": "forward_rollback",
            "accepted": true,
            "authority_mode_after": governance.authority_mode,
            "governance_commitment_before": before,
            "governance_commitment_after": after,
            "transition_id": transition.transition_id,
            "governance": governance,
        }),
    )
}

fn usage() -> ! {
    eprintln!(
        "usage:\n\
         postfiat-cobalt-handoff-rehearsal prepare --manifest PATH --output PATH\n\
         postfiat-cobalt-handoff-rehearsal sign-transition --transition PATH --key-file PATH --validator ID\n\
         postfiat-cobalt-handoff-rehearsal abort --manifest PATH --transition PATH --approvals PATH --output PATH\n\
         postfiat-cobalt-handoff-rehearsal finalize-activation --manifest PATH --transition PATH --approvals PATH --output PATH\n\
         postfiat-cobalt-handoff-rehearsal negative --manifest PATH --transition PATH --update PATH --output PATH\n\
         postfiat-cobalt-handoff-rehearsal prepare-update --manifest PATH --activation-result PATH [--replacement-record PATH] [--new-trust-graph-root HASH] --output PATH --registry-output PATH\n\
         postfiat-cobalt-handoff-rehearsal update-payload-hash --update PATH\n\
         postfiat-cobalt-handoff-rehearsal attach-decision --manifest PATH --activation-result PATH --update PATH --certificate PATH --output PATH\n\
         postfiat-cobalt-handoff-rehearsal sign-update --update PATH --key-file PATH --validator ID --authority-transition-id HASH --parent-lock-hash HASH --amendment-sequence N --proposal-slot N --expires-at-height N\n\
         postfiat-cobalt-handoff-rehearsal finalize-update --manifest PATH --activation-result PATH --update PATH --authorizations PATH --output PATH\n\
         postfiat-cobalt-handoff-rehearsal prepare-rollback --manifest PATH --update-result PATH --updated-registry PATH --output PATH\n\
         postfiat-cobalt-handoff-rehearsal finalize-rollback --manifest PATH --update-result PATH --updated-registry PATH --transition PATH --approvals PATH --output PATH"
    );
    std::process::exit(2);
}

pub fn main() -> io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    let Some(command) = args.get(1).map(String::as_str) else {
        usage();
    };
    let rest = args.get(2..).unwrap_or(&[]);
    match command {
        "prepare" => prepare(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--output")?,
        ),
        "sign-transition" => sign_transition(rest),
        "abort" => abort_activation(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--transition")?,
            &required_arg(rest, "--approvals")?,
            &required_arg(rest, "--output")?,
        ),
        "finalize-activation" => finalize_activation(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--transition")?,
            &required_arg(rest, "--approvals")?,
            &required_arg(rest, "--output")?,
        ),
        "negative" => negative_cases(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--transition")?,
            &required_arg(rest, "--update")?,
            &required_arg(rest, "--output")?,
        ),
        "prepare-update" => {
            let replacement_record = optional_arg(rest, "--replacement-record");
            let new_trust_graph_root = optional_arg(rest, "--new-trust-graph-root");
            prepare_update(
                &required_arg(rest, "--manifest")?,
                &required_arg(rest, "--activation-result")?,
                replacement_record.as_deref(),
                new_trust_graph_root.as_deref().and_then(Path::to_str),
                &required_arg(rest, "--output")?,
                &required_arg(rest, "--registry-output")?,
            )
        }
        "update-payload-hash" => print_update_payload_hash(&required_arg(rest, "--update")?),
        "attach-decision" => attach_decision_certificate(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--activation-result")?,
            &required_arg(rest, "--update")?,
            &required_arg(rest, "--certificate")?,
            &required_arg(rest, "--output")?,
        ),
        "sign-update" => sign_update(rest),
        "finalize-update" => finalize_update(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--activation-result")?,
            &required_arg(rest, "--update")?,
            &required_arg(rest, "--authorizations")?,
            &required_arg(rest, "--output")?,
        ),
        "prepare-rollback" => prepare_rollback(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--update-result")?,
            &required_arg(rest, "--updated-registry")?,
            &required_arg(rest, "--output")?,
        ),
        "finalize-rollback" => finalize_rollback(
            &required_arg(rest, "--manifest")?,
            &required_arg(rest, "--update-result")?,
            &required_arg(rest, "--updated-registry")?,
            &required_arg(rest, "--transition")?,
            &required_arg(rest, "--approvals")?,
            &required_arg(rest, "--output")?,
        ),
        _ => usage(),
    }
}
