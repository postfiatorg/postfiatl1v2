//! Prove attested normalized-input provenance and the unchanged target function.

use crate::yolo_collection::{
    domain_sha256, execute_yolo_collection_proof, YoloCollectionProofWitnessV1,
    YOLO_COLLECTION_WITNESS_SCHEMA_V1, YOLO_EPOCH_SCHEMA_V2, YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2,
};
use crate::yolo_nitro::NitroVerifier;
use crate::yolo_target::{create_yolo_portfolio_target_v1, YoloPortfolioTargetStatusV1};
use crate::yolo_witness::{decode_hex, TargetProofWitnessV1};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use postfiat_types::{
    YoloTargetPublicValuesV1, YoloTargetStatusV1, YOLO_TARGET_PUBLIC_VALUES_SCHEMA_V1,
};
use serde_json::json;
use sha2::{Digest, Sha256};

pub const TARGET_INPUT_BINDING_SCHEMA: &str = "postfiat.yolo.attested_target_input.v1";
pub const TARGET_ATTESTED_COLLECTION_SCHEMA: &str = "postfiat.yolo.target_attested_collection.v1";

pub fn target_input_commitment(witness: &TargetProofWitnessV1) -> Result<String, String> {
    domain_sha256(
        TARGET_INPUT_BINDING_SCHEMA,
        &json!({
            "schema": TARGET_INPUT_BINDING_SCHEMA,
            "collectionManifestSha256": witness.manifest.sha256()?,
            "parameterManifestSha256": witness.parameters.sha256()?,
            "targetInput": &witness.target_input,
        }),
    )
}

/// This derives claims; acceptance additionally requires independently approved
/// manifest/program pins and, for a proof, the registered SP1 verification key.
pub fn execute_yolo_target_proof(
    witness: &TargetProofWitnessV1,
) -> Result<YoloTargetPublicValuesV1, String> {
    witness.validate_bounds()?;
    let manifest = &witness.manifest;
    let policy = &manifest.proof_policy;
    let input = &witness.target_input;
    let parameter_hash = witness.parameters.sha256()?;
    let manifest_hash = manifest.sha256()?;
    if parameter_hash != manifest.parameter_manifest_sha256
        || input.series_id != manifest.series_id
        || input.underlier_id != manifest.underlier_id
        || input.trade_date != manifest.trade_date
        || input.methodology_sha256 != manifest.epoch.methodology_sha256
        || u64::from(manifest.epoch.expected_snapshot_count) != witness.parameters.observation_count
    {
        return Err(
            "target inputs differ from the committed manifest or parameter registry".into(),
        );
    }
    let epoch_hash = domain_sha256(YOLO_EPOCH_SCHEMA_V2, &manifest.epoch)?;
    // Reuse the established collection hashes and completeness checks. The old
    // adapter's opaque aggregate slot has no authority here; it is discarded.
    let collection = execute_yolo_collection_proof(&YoloCollectionProofWitnessV1 {
        schema: YOLO_COLLECTION_WITNESS_SCHEMA_V1.into(),
        program_sha256: witness.program_sha256.clone(),
        attested_collection_sha256: "00".repeat(32),
        epoch: manifest.epoch.clone(),
        commitments: witness.commitments.clone(),
    })?;
    if input.collection_sha256 != collection.commitments_sha256 {
        return Err("target input is not bound to the completed collection".into());
    }
    let mut commitments: Vec<_> = witness.commitments.iter().collect();
    commitments.sort_by_key(|commitment| commitment.sequence);
    for snapshot in &input.snapshots {
        let commitment = commitments
            .get(snapshot.sequence as usize)
            .ok_or("normalized observation has no collected snapshot")?;
        if snapshot.scheduled_at_utc != commitment.scheduled_at_utc
            || snapshot.observed_at_utc != commitment.observed_at_utc
        {
            return Err("normalized observation times differ from the attested collection".into());
        }
    }
    let root = decode_hex("root certificate", &witness.root_certificate_der_hex, 1024)?;
    let time_ms = manifest.verification_time_ms()?;
    let mut nitro = NitroVerifier::new(&root, policy, time_ms)?;
    let target_input_hash = target_input_commitment(witness)?;
    let first_evidence = &witness.statements[0].evidence;
    let mut previous = None;
    let mut statement_hashes = Vec::new();
    for (index, (statement, document_hex)) in witness
        .statements
        .iter()
        .zip(&witness.attestation_documents_hex)
        .enumerate()
    {
        if statement.sequence as usize != index
            || statement.epoch_sha256 != epoch_hash
            || statement.previous_statement_sha256 != previous
            || statement.evidence.verifier_id != policy.verifier_id
            || statement.evidence.statement_key != first_evidence.statement_key
            || statement.evidence.measurement != first_evidence.measurement
        {
            return Err("attested statement sequence, epoch, key or policy chain mismatch".into());
        }
        if let Some(commitment) = commitments.get(index) {
            if statement.statement_kind != "option-chain-snapshot"
                || statement.input_sha256 != commitment.private_snapshot_sha256
                || statement.output_sha256
                    != domain_sha256(YOLO_SNAPSHOT_COMMITMENT_SCHEMA_V2, commitment)?
            {
                return Err("snapshot statement does not bind the collected snapshot".into());
            }
        } else if statement.statement_kind != "portfolio-target-input"
            || statement.input_sha256 != collection.commitments_sha256
            || statement.output_sha256 != target_input_hash
        {
            return Err(
                "terminal statement does not bind the complete normalized target input".into(),
            );
        }
        let document = decode_hex("Nitro document", document_hex, 64 * 1024)?;
        if hex::encode(Sha256::digest(&document)) != statement.evidence.attestation_document_sha256
        {
            return Err("Nitro document does not match its statement hash".into());
        }
        let claims = nitro.verify(&document, &statement.attestation_binding_digest()?)?;
        let pcr0 = claims.pcrs.get(&0).ok_or("Nitro PCR0 is missing")?;
        if claims.statement_key != statement.evidence.statement_key
            || statement.evidence.measurement != format!("pcr0:{}", hex::encode(pcr0))
        {
            return Err("Nitro claims differ from statement evidence".into());
        }
        let key: [u8; 32] = decode_hex("statement key", &claims.statement_key, 32)?
            .try_into()
            .map_err(|_| "invalid Ed25519 key size")?;
        let key = VerifyingKey::from_bytes(&key).map_err(|_| "invalid Ed25519 key")?;
        let signature = BASE64
            .decode(&statement.signature)
            .map_err(|_| "statement signature is not canonical base64")?;
        if BASE64.encode(&signature) != statement.signature {
            return Err("statement signature base64 is noncanonical".into());
        }
        let signature =
            Signature::from_slice(&signature).map_err(|_| "statement signature has wrong size")?;
        key.verify_strict(
            &decode_hex("signing digest", &statement.signing_digest()?, 32)?,
            &signature,
        )
        .map_err(|_| "attested Ed25519 statement signature is invalid")?;
        previous = Some(statement.sha256()?);
        statement_hashes.push(previous.clone().ok_or("statement commitment is missing")?);
    }
    let aggregate_hash = domain_sha256(
        TARGET_ATTESTED_COLLECTION_SCHEMA,
        &json!({
            "schema": TARGET_ATTESTED_COLLECTION_SCHEMA,
            "collectionManifestSha256": manifest_hash,
            "epochSha256": epoch_hash,
            "commitmentsSha256": collection.commitments_sha256,
            "statementSha256s": statement_hashes,
        }),
    )?;
    let target = create_yolo_portfolio_target_v1(input, &witness.parameters)?;
    let mut positions = input.positions.clone();
    positions.sort_by(|a, b| a.occ_symbol.cmp(&b.occ_symbol));
    let mut incumbents = input.incumbent_symbols.clone();
    incumbents.sort();
    let prior_hash = domain_sha256(
        "postfiat.yolo.prior_portfolio_state.v1",
        &json!({
            "schema": "postfiat.yolo.prior_portfolio_state.v1",
            "positions": positions,
            "settledCashMicrodollars": input.settled_cash_microdollars,
            "unsettledCashMicrodollars": input.unsettled_cash_microdollars,
            "accruedLiabilitiesMicrodollars": input.accrued_liabilities_microdollars,
            "incumbentExpiration": input.incumbent_expiration,
            "incumbentSymbols": incumbents,
        }),
    )?;
    let public = YoloTargetPublicValuesV1 {
        schema: YOLO_TARGET_PUBLIC_VALUES_SCHEMA_V1.into(),
        program_sha256: witness.program_sha256.clone(),
        methodology_sha256: input.methodology_sha256.clone(),
        parameter_manifest_sha256: parameter_hash,
        collection_manifest_sha256: manifest_hash,
        series_id_sha256: domain_sha256("postfiat.yolo.series_id.v1", &input.series_id)?,
        underlier_id_sha256: domain_sha256("postfiat.yolo.underlier_id.v1", &input.underlier_id)?,
        epoch_sha256: epoch_hash,
        collection_sha256: collection.commitments_sha256,
        aggregate_attestation_sha256: aggregate_hash,
        prior_state_sha256: prior_hash,
        selected_expiration_sha256: domain_sha256(
            "postfiat.yolo.selected_expiration.v1",
            &target.selected_expiration,
        )?,
        target_sha256: target.sha256()?,
        status: match target.status {
            YoloPortfolioTargetStatusV1::TargetComputed => YoloTargetStatusV1::TargetComputed,
            YoloPortfolioTargetStatusV1::NoReconstitution => YoloTargetStatusV1::NoReconstitution,
            YoloPortfolioTargetStatusV1::NoEligibleSuccessor => {
                YoloTargetStatusV1::NoEligibleSuccessor
            }
            YoloPortfolioTargetStatusV1::HaltedInstrument => YoloTargetStatusV1::HaltedInstrument,
            YoloPortfolioTargetStatusV1::UnresolvedCapitalPolicy => {
                YoloTargetStatusV1::UnresolvedCapitalPolicy
            }
        },
        snapshot_count: u32::try_from(input.snapshots.len())
            .map_err(|_| "snapshot count overflow")?,
        selected_contract_count: u32::try_from(
            target
                .positions
                .iter()
                .filter(|row| row.target_quantity > 0)
                .count(),
        )
        .map_err(|_| "target count overflow")?,
    };
    public.validate()?;
    Ok(public)
}
