//! Consensus-admission verification for authoritative Cobalt decisions.
//!
//! A Cobalt-authorized validator update is valid only when it carries a signed
//! RBC -> ABBA -> MVBA -> DABC transcript bound to the exact update payload.

use super::*;
use crate::cobalt_shadow::{
    validate_validator_binding, CobaltShadowProtocolDecision, CobaltShadowProtocolTranscript,
    CobaltShadowRegistryBinding, COBALT_SHADOW_PROTOCOL_TRANSCRIPT_SCHEMA,
    COBALT_SHADOW_REGISTRY_BINDING_SCHEMA,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use postfiat_consensus_cobalt::{
    abba_strong_support, build_mvba_valid_input_set, dabc_full_knowledge_checkpoint_id,
    evaluate_abba_finish_support_signed, evaluate_rbc_ready_support_signed, has_strong_support,
    mvba_candidate_from_rbc_accept, ratify_dabc_amendment, validate_abba_aux_signed,
    validate_abba_conf_signed, validate_abba_finish_signed, validate_abba_init_signed,
    validate_dabc_full_knowledge_checkpoint_signed, validate_dabc_ratified_amendment,
    validate_rbc_accept_signed, validate_rbc_echo_signed, validate_rbc_propose_signed,
    validate_rbc_ready_signed, validate_trust_graph, CobaltSignatureCommittee,
    DabcFullKnowledgeCheck, DabcRatifiedAmendment,
};
use postfiat_types::{
    CobaltValidatorUpdateDecisionCertificateV1,
    COBALT_VALIDATOR_UPDATE_DECISION_CERTIFICATE_SCHEMA_V1,
};
use std::io::Read as _;

const COBALT_AUTHORITY_COMPACT_TRANSCRIPT_SCHEMA_V1: &str =
    "postfiat.cobalt.authority_compact_protocol_transcript.v1";
const COBALT_AUTHORITY_COMPRESSED_VALUE_SCHEMA_V1: &str =
    "postfiat.cobalt.authority_compressed_value.v1";
const COBALT_AUTHORITY_COMPRESSION_CODEC_V1: &str = "deflate-json-v1";
pub const MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES: usize = 1024 * 1024;
const MAX_COBALT_AUTHORITY_DECOMPRESSED_VALUE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CobaltAuthorityCompactProtocolTranscriptV1 {
    schema: String,
    transcript: CobaltShadowProtocolTranscript,
    full_knowledge_checks: Vec<DabcFullKnowledgeCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CobaltAuthorityCompressedValueV1 {
    schema: String,
    codec: String,
    decompressed_bytes: usize,
    payload_hash: String,
    payload_base64: String,
}

fn certificate_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message.into())
}

fn consensus_certificate_error(error: String) -> io::Error {
    certificate_error(format!("Cobalt decision certificate invalid: {error}"))
}

fn hash_serialized<T: Serialize>(domain: &str, value: &T) -> io::Result<String> {
    let encoded = serde_json::to_vec(value).map_err(invalid_data)?;
    Ok(hash_hex(domain, &encoded))
}

fn compress_authority_value<T: Serialize>(value: &T) -> io::Result<serde_json::Value> {
    let encoded = serde_json::to_vec(value).map_err(invalid_data)?;
    if encoded.len() > MAX_COBALT_AUTHORITY_DECOMPRESSED_VALUE_BYTES {
        return Err(certificate_error(format!(
            "Cobalt authority value is {} bytes after expansion; maximum is {}",
            encoded.len(),
            MAX_COBALT_AUTHORITY_DECOMPRESSED_VALUE_BYTES
        )));
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&encoded)?;
    let compressed = encoder.finish()?;
    serde_json::to_value(CobaltAuthorityCompressedValueV1 {
        schema: COBALT_AUTHORITY_COMPRESSED_VALUE_SCHEMA_V1.to_string(),
        codec: COBALT_AUTHORITY_COMPRESSION_CODEC_V1.to_string(),
        decompressed_bytes: encoded.len(),
        payload_hash: hash_hex("postfiat.cobalt.authority-compressed-value.v1", &encoded),
        payload_base64: BASE64_STANDARD.encode(compressed),
    })
    .map_err(invalid_data)
}

fn decompress_authority_value<T>(value: &serde_json::Value) -> io::Result<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let packed: CobaltAuthorityCompressedValueV1 =
        serde_json::from_value(value.clone()).map_err(invalid_data)?;
    if packed.schema != COBALT_AUTHORITY_COMPRESSED_VALUE_SCHEMA_V1
        || packed.codec != COBALT_AUTHORITY_COMPRESSION_CODEC_V1
        || packed.decompressed_bytes == 0
        || packed.decompressed_bytes > MAX_COBALT_AUTHORITY_DECOMPRESSED_VALUE_BYTES
    {
        return Err(certificate_error(
            "Cobalt authority compressed value header is invalid",
        ));
    }
    let compressed = BASE64_STANDARD
        .decode(&packed.payload_base64)
        .map_err(invalid_data)?;
    let mut decoder = DeflateDecoder::new(compressed.as_slice());
    let mut encoded = Vec::with_capacity(packed.decompressed_bytes);
    decoder
        .by_ref()
        .take((MAX_COBALT_AUTHORITY_DECOMPRESSED_VALUE_BYTES + 1) as u64)
        .read_to_end(&mut encoded)?;
    if encoded.len() != packed.decompressed_bytes
        || encoded.len() > MAX_COBALT_AUTHORITY_DECOMPRESSED_VALUE_BYTES
        || hash_hex("postfiat.cobalt.authority-compressed-value.v1", &encoded)
            != packed.payload_hash
    {
        return Err(certificate_error(
            "Cobalt authority compressed value length or digest mismatch",
        ));
    }
    let decoded: T = serde_json::from_slice(&encoded).map_err(invalid_data)?;
    let canonical: CobaltAuthorityCompressedValueV1 =
        serde_json::from_value(compress_authority_value(&decoded)?).map_err(invalid_data)?;
    if canonical != packed {
        return Err(certificate_error(
            "Cobalt authority compressed value is not canonically encoded",
        ));
    }
    Ok(decoded)
}

fn compact_protocol_transcript(
    mut transcript: CobaltShadowProtocolTranscript,
) -> io::Result<CobaltAuthorityCompactProtocolTranscriptV1> {
    let reference_checks = transcript
        .full_knowledge_checkpoints
        .first()
        .ok_or_else(|| certificate_error("Cobalt protocol transcript has no DABC checkpoints"))?
        .checks
        .clone();
    if reference_checks.is_empty()
        || transcript
            .full_knowledge_checkpoints
            .iter()
            .any(|checkpoint| checkpoint.checks != reference_checks)
    {
        return Err(certificate_error(
            "Cobalt DABC checkpoints must share one nonempty support certificate",
        ));
    }
    let signer_set = transcript
        .rbc_echoes
        .iter()
        .map(|message| message.sender.clone())
        .collect::<BTreeSet<_>>();
    let stage_signer_sets = [
        transcript
            .rbc_readies
            .iter()
            .map(|message| message.sender.clone())
            .collect::<BTreeSet<_>>(),
        transcript
            .rbc_accepts
            .iter()
            .map(|message| message.sender.clone())
            .collect::<BTreeSet<_>>(),
        transcript
            .abba_inits
            .iter()
            .map(|message| message.sender.clone())
            .collect::<BTreeSet<_>>(),
        transcript
            .abba_auxes
            .iter()
            .map(|message| message.sender.clone())
            .collect::<BTreeSet<_>>(),
        transcript
            .abba_confs
            .iter()
            .map(|message| message.sender.clone())
            .collect::<BTreeSet<_>>(),
        transcript
            .abba_finishes
            .iter()
            .map(|message| message.sender.clone())
            .collect::<BTreeSet<_>>(),
        reference_checks
            .iter()
            .map(|message| message.sender.clone())
            .collect::<BTreeSet<_>>(),
    ];
    if signer_set.is_empty() || stage_signer_sets.iter().any(|stage| stage != &signer_set) {
        return Err(certificate_error(
            "Cobalt authority transcript stages must share one signer set",
        ));
    }

    let mut selected = signer_set.into_iter().collect::<Vec<_>>();
    for candidate in selected.clone().into_iter().rev() {
        let trial = selected
            .iter()
            .filter(|validator| *validator != &candidate)
            .cloned()
            .collect::<Vec<_>>();
        let supports_every_view = transcript.trust_graph.trust_views.iter().all(|view| {
            let local_support = trial
                .iter()
                .filter(|validator| view.derived_unl.binary_search(validator).is_ok())
                .cloned()
                .collect::<Vec<_>>();
            has_strong_support(view, &local_support).unwrap_or(false)
        });
        if supports_every_view {
            selected = trial;
        }
    }
    let selected = selected.into_iter().collect::<BTreeSet<_>>();
    let retain = |sender: &str| selected.contains(sender);
    transcript
        .rbc_echoes
        .retain(|message| retain(&message.sender));
    transcript
        .rbc_readies
        .retain(|message| retain(&message.sender));
    transcript
        .rbc_accepts
        .retain(|message| retain(&message.sender));
    transcript
        .abba_inits
        .retain(|message| retain(&message.sender));
    transcript
        .abba_auxes
        .retain(|message| retain(&message.sender));
    transcript
        .abba_confs
        .retain(|message| retain(&message.sender));
    transcript
        .abba_finishes
        .retain(|message| retain(&message.sender));
    let reference_checks = reference_checks
        .into_iter()
        .filter(|check| retain(&check.sender))
        .collect::<Vec<_>>();
    for checkpoint in &mut transcript.full_knowledge_checkpoints {
        checkpoint.checks = reference_checks.clone();
        checkpoint.checkpoint_id = dabc_full_knowledge_checkpoint_id(
            &postfiat_consensus_cobalt::CobaltDomain {
                chain_id: transcript.trust_graph.chain_id.clone(),
                genesis_hash: transcript.trust_graph.genesis_hash.clone(),
                protocol_version: transcript.trust_graph.protocol_version,
            },
            checkpoint,
        )
        .map_err(consensus_certificate_error)?;
        checkpoint.checks.clear();
    }
    Ok(CobaltAuthorityCompactProtocolTranscriptV1 {
        schema: COBALT_AUTHORITY_COMPACT_TRANSCRIPT_SCHEMA_V1.to_string(),
        transcript,
        full_knowledge_checks: reference_checks,
    })
}

fn expand_protocol_transcript(
    value: &serde_json::Value,
) -> io::Result<CobaltShadowProtocolTranscript> {
    let mut compact: CobaltAuthorityCompactProtocolTranscriptV1 =
        serde_json::from_value(value.clone()).map_err(invalid_data)?;
    if compact.schema != COBALT_AUTHORITY_COMPACT_TRANSCRIPT_SCHEMA_V1
        || compact.full_knowledge_checks.is_empty()
        || compact
            .transcript
            .full_knowledge_checkpoints
            .iter()
            .any(|checkpoint| !checkpoint.checks.is_empty())
    {
        return Err(certificate_error(
            "Cobalt authority transcript is not in canonical compact form",
        ));
    }
    for checkpoint in &mut compact.transcript.full_knowledge_checkpoints {
        checkpoint.checks = compact.full_knowledge_checks.clone();
    }
    Ok(compact.transcript)
}

pub fn compact_cobalt_validator_update_decision_certificate(
    mut certificate: CobaltValidatorUpdateDecisionCertificateV1,
) -> io::Result<CobaltValidatorUpdateDecisionCertificateV1> {
    let binding: CobaltShadowRegistryBinding =
        serde_json::from_value(certificate.registry_binding).map_err(invalid_data)?;
    let transcript: CobaltShadowProtocolTranscript =
        serde_json::from_value(certificate.protocol_transcript).map_err(invalid_data)?;
    certificate.registry_binding = compress_authority_value(&binding)?;
    certificate.protocol_transcript =
        compress_authority_value(&compact_protocol_transcript(transcript)?)?;
    let encoded = serde_json::to_vec(&certificate).map_err(invalid_data)?;
    if encoded.len() > MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES {
        return Err(certificate_error(format!(
            "Cobalt validator update decision certificate is {} bytes; maximum is {}",
            encoded.len(),
            MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES
        )));
    }
    Ok(certificate)
}

fn sorted_unique_senders<'a>(
    label: &str,
    senders: impl IntoIterator<Item = &'a str>,
    active_validators: &[String],
) -> io::Result<()> {
    let senders = senders.into_iter().collect::<Vec<_>>();
    if senders.is_empty() || senders.len() > active_validators.len() {
        return Err(certificate_error(format!(
            "{label} signer cardinality is invalid"
        )));
    }
    if senders.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(certificate_error(format!(
            "{label} signers must be sorted unique"
        )));
    }
    if senders.iter().any(|sender| {
        active_validators
            .binary_search_by(|value| value.as_str().cmp(sender))
            .is_err()
    }) {
        return Err(certificate_error(format!(
            "{label} contains a signer outside the active validator registry"
        )));
    }
    Ok(())
}

pub fn cobalt_validator_update_payload_hash(
    update: &ValidatorRegistryUpdateRecord,
) -> io::Result<String> {
    let mut unsigned = update.clone();
    unsigned.signed_authorizations.clear();
    unsigned.cobalt_authorizations.clear();
    unsigned.cobalt_decision_certificate = None;
    hash_serialized("postfiat.cobalt.validator-update-payload.v1", &unsigned)
}

fn validate_registry_binding(
    binding: &CobaltShadowRegistryBinding,
    expected_domain: &CobaltDomain,
    registry: &ValidatorRegistry,
    active_validators: &[String],
    registry_root: &str,
    trust_graph_root: &str,
) -> io::Result<CobaltSignatureCommittee> {
    if binding.schema != COBALT_SHADOW_REGISTRY_BINDING_SCHEMA
        || &binding.validator_registry != registry
        || binding.registry_root != registry_root
        || binding.active_validators != active_validators
    {
        return Err(certificate_error(
            "Cobalt certificate registry binding does not match live consensus state",
        ));
    }
    if active_validators.len() < 3
        || active_validators.windows(2).any(|pair| pair[0] >= pair[1])
        || validator_registry_root(registry, active_validators)? != registry_root
    {
        return Err(certificate_error(
            "Cobalt certificate active validator registry is invalid",
        ));
    }
    let peer_ids = binding.peers.keys().cloned().collect::<Vec<_>>();
    let graph_ids = binding
        .trust_graph
        .trust_views
        .iter()
        .map(|view| view.validator.clone())
        .collect::<Vec<_>>();
    if peer_ids != active_validators
        || graph_ids != active_validators
        || binding.trust_graph.registry_root != registry_root
        || binding.trust_graph.trust_graph_root != trust_graph_root
        || binding.validator_bindings.len() != active_validators.len()
    {
        return Err(certificate_error(
            "Cobalt certificate trust graph is not bound to the active validator registry",
        ));
    }
    let domain = CobaltDomain {
        chain_id: binding.trust_graph.chain_id.clone(),
        genesis_hash: binding.trust_graph.genesis_hash.clone(),
        protocol_version: binding.trust_graph.protocol_version,
    };
    if &domain != expected_domain {
        return Err(certificate_error(
            "Cobalt certificate domain does not match the live chain",
        ));
    }
    validate_trust_graph(&domain, &binding.trust_graph).map_err(consensus_certificate_error)?;

    let mut seen = BTreeSet::new();
    for validator_binding in &binding.validator_bindings {
        validate_validator_binding(validator_binding)?;
        if !seen.insert(validator_binding.node_id.clone())
            || validator_binding.chain_id != domain.chain_id
            || validator_binding.genesis_hash != domain.genesis_hash
            || validator_binding.protocol_version != domain.protocol_version
            || validator_binding.registry_root != registry_root
            || binding.peers.get(&validator_binding.node_id)
                != Some(&validator_binding.cobalt_public_key_hex)
        {
            return Err(certificate_error(
                "Cobalt validator-key binding is duplicated or domain-mismatched",
            ));
        }
        let registry_record = validator_registry_record(registry, &validator_binding.node_id)?;
        if registry_record.algorithm_id != validator_binding.validator_algorithm
            || registry_record.public_key_hex != validator_binding.validator_public_key_hex
        {
            return Err(certificate_error(
                "Cobalt protocol key is not authorized by the live validator key",
            ));
        }
    }

    let mut committee = CobaltSignatureCommittee::default();
    for (validator, public_key_hex) in &binding.peers {
        committee
            .insert_hex(validator.clone(), public_key_hex)
            .map_err(consensus_certificate_error)?;
    }
    Ok(committee)
}

#[allow(clippy::too_many_arguments)]
pub fn verify_cobalt_validator_update_decision_certificate(
    certificate: &CobaltValidatorUpdateDecisionCertificateV1,
    expected_domain: &CobaltDomain,
    registry: &ValidatorRegistry,
    active_validators: &[String],
    registry_root: &str,
    trust_graph_root: &str,
    expected_payload_hash: &str,
    expected_round: u64,
    expected_activation_height: u64,
    previous: Option<&DabcRatifiedAmendment>,
) -> io::Result<CobaltShadowProtocolDecision> {
    if certificate.schema != COBALT_VALIDATOR_UPDATE_DECISION_CERTIFICATE_SCHEMA_V1 {
        return Err(certificate_error(
            "Cobalt validator update decision certificate schema mismatch",
        ));
    }
    let encoded_certificate = serde_json::to_vec(certificate).map_err(invalid_data)?;
    if encoded_certificate.len() > MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES {
        return Err(certificate_error(format!(
            "Cobalt validator update decision certificate is {} bytes; maximum is {}",
            encoded_certificate.len(),
            MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES
        )));
    }
    let binding: CobaltShadowRegistryBinding =
        decompress_authority_value(&certificate.registry_binding)?;
    let compact: CobaltAuthorityCompactProtocolTranscriptV1 =
        decompress_authority_value(&certificate.protocol_transcript)?;
    let transcript =
        expand_protocol_transcript(&serde_json::to_value(&compact).map_err(invalid_data)?)?;
    if compact_protocol_transcript(transcript.clone())? != compact {
        return Err(certificate_error(
            "Cobalt authority transcript does not use the canonical minimal signer set",
        ));
    }
    let committee = validate_registry_binding(
        &binding,
        expected_domain,
        registry,
        active_validators,
        registry_root,
        trust_graph_root,
    )?;
    let domain = CobaltDomain {
        chain_id: binding.trust_graph.chain_id.clone(),
        genesis_hash: binding.trust_graph.genesis_hash.clone(),
        protocol_version: binding.trust_graph.protocol_version,
    };
    if transcript.schema != COBALT_SHADOW_PROTOCOL_TRANSCRIPT_SCHEMA
        || transcript.registry_root != registry_root
        || transcript.trust_graph != binding.trust_graph
        || transcript.round != expected_round
        || transcript.payload_hash != expected_payload_hash
        || transcript.payload_hash != transcript.rbc_propose.payload_hash
        || transcript.ratification.activation_height != expected_activation_height
    {
        return Err(certificate_error(
            "Cobalt protocol transcript is not bound to this validator update",
        ));
    }

    validate_rbc_propose_signed(&domain, &committee, &transcript.rbc_propose)
        .map_err(consensus_certificate_error)?;
    sorted_unique_senders(
        "RBC echo",
        transcript
            .rbc_echoes
            .iter()
            .map(|message| message.sender.as_str()),
        active_validators,
    )?;
    sorted_unique_senders(
        "RBC ready",
        transcript
            .rbc_readies
            .iter()
            .map(|message| message.sender.as_str()),
        active_validators,
    )?;
    sorted_unique_senders(
        "RBC accept",
        transcript
            .rbc_accepts
            .iter()
            .map(|message| message.sender.as_str()),
        active_validators,
    )?;
    for message in &transcript.rbc_echoes {
        validate_rbc_echo_signed(&domain, &committee, message, &transcript.rbc_propose)
            .map_err(consensus_certificate_error)?;
    }
    for message in &transcript.rbc_readies {
        validate_rbc_ready_signed(&domain, &committee, message, &transcript.rbc_propose)
            .map_err(consensus_certificate_error)?;
    }
    for message in &transcript.rbc_accepts {
        validate_rbc_accept_signed(&domain, &committee, message, &transcript.rbc_propose)
            .map_err(consensus_certificate_error)?;
    }
    for view in &transcript.trust_graph.trust_views {
        let support = evaluate_rbc_ready_support_signed(
            &domain,
            &committee,
            view,
            &transcript.rbc_propose,
            &transcript.rbc_readies,
        )
        .map_err(consensus_certificate_error)?;
        if !support.strong_support {
            return Err(certificate_error(format!(
                "Cobalt RBC lacks strong support for trust view {}",
                view.validator
            )));
        }
    }

    sorted_unique_senders(
        "ABBA init",
        transcript
            .abba_inits
            .iter()
            .map(|message| message.sender.as_str()),
        active_validators,
    )?;
    sorted_unique_senders(
        "ABBA aux",
        transcript
            .abba_auxes
            .iter()
            .map(|message| message.sender.as_str()),
        active_validators,
    )?;
    sorted_unique_senders(
        "ABBA conf",
        transcript
            .abba_confs
            .iter()
            .map(|message| message.sender.as_str()),
        active_validators,
    )?;
    sorted_unique_senders(
        "ABBA finish",
        transcript
            .abba_finishes
            .iter()
            .map(|message| message.sender.as_str()),
        active_validators,
    )?;
    let contribution_signers = transcript
        .rbc_echoes
        .iter()
        .map(|message| message.sender.as_str())
        .collect::<Vec<_>>();
    let stage_signers = [
        transcript
            .rbc_readies
            .iter()
            .map(|message| message.sender.as_str())
            .collect::<Vec<_>>(),
        transcript
            .rbc_accepts
            .iter()
            .map(|message| message.sender.as_str())
            .collect::<Vec<_>>(),
        transcript
            .abba_inits
            .iter()
            .map(|message| message.sender.as_str())
            .collect::<Vec<_>>(),
        transcript
            .abba_auxes
            .iter()
            .map(|message| message.sender.as_str())
            .collect::<Vec<_>>(),
        transcript
            .abba_confs
            .iter()
            .map(|message| message.sender.as_str())
            .collect::<Vec<_>>(),
        transcript
            .abba_finishes
            .iter()
            .map(|message| message.sender.as_str())
            .collect::<Vec<_>>(),
    ];
    if stage_signers
        .iter()
        .any(|stage| stage != &contribution_signers)
    {
        return Err(certificate_error(
            "Cobalt authority transcript stages do not share one signer set",
        ));
    }
    for message in &transcript.abba_inits {
        validate_abba_init_signed(&domain, &committee, message)
            .map_err(consensus_certificate_error)?;
    }
    for message in &transcript.abba_auxes {
        validate_abba_aux_signed(&domain, &committee, message)
            .map_err(consensus_certificate_error)?;
    }
    for message in &transcript.abba_confs {
        validate_abba_conf_signed(&domain, &committee, message)
            .map_err(consensus_certificate_error)?;
    }
    for message in &transcript.abba_finishes {
        validate_abba_finish_signed(&domain, &committee, message)
            .map_err(consensus_certificate_error)?;
    }
    for view in &transcript.trust_graph.trust_views {
        let support = evaluate_abba_finish_support_signed(
            &domain,
            &committee,
            view,
            &transcript.agreement_id,
            transcript.round,
            true,
            &transcript.abba_finishes,
        )
        .map_err(consensus_certificate_error)?;
        if !abba_strong_support(&support) {
            return Err(certificate_error(format!(
                "Cobalt ABBA lacks strong support for trust view {}",
                view.validator
            )));
        }
    }

    let first_accept = transcript
        .rbc_accepts
        .first()
        .ok_or_else(|| certificate_error("Cobalt transcript has no RBC accept"))?;
    let candidate = mvba_candidate_from_rbc_accept(&domain, &transcript.rbc_propose, first_accept)
        .map_err(consensus_certificate_error)?;
    let input_view = transcript
        .trust_graph
        .trust_views
        .iter()
        .find(|view| view.trust_view_id == transcript.mvba_input.trust_view_id)
        .ok_or_else(|| certificate_error("Cobalt MVBA input view is not in the trust graph"))?;
    let expected_input = build_mvba_valid_input_set(
        &domain,
        input_view,
        transcript.agreement_id.clone(),
        vec![candidate],
    )
    .map_err(consensus_certificate_error)?;
    if expected_input != transcript.mvba_input {
        return Err(certificate_error("Cobalt MVBA input set mismatch"));
    }

    validate_dabc_ratified_amendment(
        &domain,
        &transcript.trust_graph,
        &transcript.ratification,
        previous,
    )
    .map_err(consensus_certificate_error)?;
    let expected_ratification = ratify_dabc_amendment(
        &domain,
        &transcript.trust_graph,
        &transcript.mvba_input,
        previous,
        expected_activation_height,
    )
    .map_err(consensus_certificate_error)?;
    if expected_ratification != transcript.ratification {
        return Err(certificate_error("Cobalt DABC ratification mismatch"));
    }

    let expected_checkpoint_validators = active_validators.iter().cloned().collect::<BTreeSet<_>>();
    let checkpoint_validators = transcript
        .full_knowledge_checkpoints
        .iter()
        .map(|checkpoint| checkpoint.local_validator.clone())
        .collect::<BTreeSet<_>>();
    if transcript.full_knowledge_checkpoints.len() != active_validators.len()
        || checkpoint_validators != expected_checkpoint_validators
    {
        return Err(certificate_error(
            "Cobalt DABC full-knowledge certificates do not cover every trust view",
        ));
    }
    let mut reference_checks = None;
    for checkpoint in &transcript.full_knowledge_checkpoints {
        validate_dabc_full_knowledge_checkpoint_signed(
            &domain,
            &committee,
            &transcript.trust_graph,
            checkpoint,
        )
        .map_err(consensus_certificate_error)?;
        sorted_unique_senders(
            "DABC full-knowledge",
            checkpoint.checks.iter().map(|check| check.sender.as_str()),
            active_validators,
        )?;
        if checkpoint
            .checks
            .iter()
            .map(|check| check.sender.as_str())
            .collect::<Vec<_>>()
            != contribution_signers
        {
            return Err(certificate_error(
                "Cobalt DABC support signers do not match the protocol signer set",
            ));
        }
        if let Some(expected) = reference_checks {
            if checkpoint.checks.as_slice() != expected {
                return Err(certificate_error(
                    "Cobalt DABC trust views carry different support certificates",
                ));
            }
        } else {
            reference_checks = Some(checkpoint.checks.as_slice());
        }
    }

    let transcript_hash =
        hash_serialized("postfiat.cobalt.shadow.protocol-transcript.v1", &transcript)?;
    let support_certificate_hash = hash_serialized(
        "postfiat.cobalt.shadow.support-certificate.v1",
        &(
            &transcript.rbc_echoes,
            &transcript.rbc_readies,
            &transcript.rbc_accepts,
            &transcript.abba_inits,
            &transcript.abba_auxes,
            &transcript.abba_confs,
            &transcript.abba_finishes,
            &transcript.full_knowledge_checkpoints,
        ),
    )?;
    let decision_id = hash_serialized(
        "postfiat.cobalt.shadow.protocol-decision.v1",
        &(
            transcript.round,
            &transcript.payload_hash,
            &transcript.agreement_id,
            &transcript.mvba_input.output_candidate_id,
            &transcript.ratification.ratification_id,
            &transcript.registry_root,
            &transcript.trust_graph.trust_graph_root,
        ),
    )?;
    let certificate_signer_count = reference_checks
        .map(|checks| {
            checks
                .iter()
                .map(|check| &check.sender)
                .collect::<BTreeSet<_>>()
                .len()
        })
        .unwrap_or_default();
    let signed_message_count = 1
        + transcript.rbc_echoes.len()
        + transcript.rbc_readies.len()
        + transcript.rbc_accepts.len()
        + transcript.abba_inits.len()
        + transcript.abba_auxes.len()
        + transcript.abba_confs.len()
        + transcript.abba_finishes.len()
        + reference_checks.map(<[_]>::len).unwrap_or_default();
    Ok(CobaltShadowProtocolDecision {
        round: transcript.round,
        decision_id,
        transcript_hash,
        support_certificate_hash,
        payload_hash: transcript.payload_hash,
        agreement_id: transcript.agreement_id,
        output_candidate_id: transcript.mvba_input.output_candidate_id,
        ratification_id: transcript.ratification.ratification_id,
        registry_root: transcript.registry_root,
        trust_graph_root: transcript.trust_graph.trust_graph_root,
        certificate_signer_count,
        signed_message_count,
    })
}

pub fn cobalt_decision_ratification(
    certificate: &CobaltValidatorUpdateDecisionCertificateV1,
) -> io::Result<DabcRatifiedAmendment> {
    let compact: CobaltAuthorityCompactProtocolTranscriptV1 =
        decompress_authority_value(&certificate.protocol_transcript)?;
    let transcript =
        expand_protocol_transcript(&serde_json::to_value(compact).map_err(invalid_data)?)?;
    Ok(transcript.ratification)
}

#[cfg(test)]
pub(super) fn rewrite_cobalt_certificate_domain_for_test(
    certificate: &mut CobaltValidatorUpdateDecisionCertificateV1,
    chain_id: &str,
) -> io::Result<()> {
    let mut binding: CobaltShadowRegistryBinding =
        decompress_authority_value(&certificate.registry_binding)?;
    binding.trust_graph.chain_id = chain_id.to_string();
    certificate.registry_binding = compress_authority_value(&binding)?;
    let mut compact: CobaltAuthorityCompactProtocolTranscriptV1 =
        decompress_authority_value(&certificate.protocol_transcript)?;
    compact.transcript.trust_graph.chain_id = chain_id.to_string();
    certificate.protocol_transcript = compress_authority_value(&compact)?;
    Ok(())
}
