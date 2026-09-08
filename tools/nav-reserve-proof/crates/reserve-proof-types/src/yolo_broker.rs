//! Typed commitments for an attested YOLO brokerage reserve observation.
//!
//! Brokerage API facts remain `attested`. This module commits the private
//! position/account artifacts without interpreting them as cryptographic
//! custody proofs or exposing licensed market data.

use serde::{Deserialize, Serialize};

use super::{append_bytes, opaque_commitment, validate_hex, validate_identifier};

pub const YOLO_BROKER_ADAPTER_KIND_V1: &str = "yolo-broker-attested-v1";
pub const YOLO_BROKER_DISCLOSURE_SCHEMA_V1: &str = "postfiat.yolo.broker_reserve_disclosure.v1";
const YOLO_QUANTITY_COMMITMENT_LABEL_V1: &str = "yolo-broker-quantity-v1";
const YOLO_VALUATION_COMMITMENT_LABEL_V1: &str = "yolo-broker-valuation-v1";
const YOLO_DISCLOSURE_COMMITMENT_LABEL_V1: &str = "yolo-broker-disclosure-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoloBrokerReserveDisclosureV1 {
    pub schema: String,
    pub broker_source_id: String,
    pub account_application_identity_sha256: String,
    pub observed_at_unix_millis: u64,
    pub collection_epoch_sha256: String,
    pub basket_decision_sha256: String,
    pub execution_receipts_root_sha256: String,
    pub broker_response_root_sha256: String,
    pub positions_root_sha256: String,
    pub cash_root_sha256: String,
    pub liabilities_root_sha256: String,
    pub open_orders_root_sha256: String,
    pub fills_root_sha256: String,
    pub pending_events_root_sha256: String,
    pub valuation_inputs_root_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoloBrokerEvidenceCommitmentsV1 {
    pub quantity_evidence_commitment: String,
    pub valuation_evidence_commitment: String,
    pub disclosure_commitment: String,
}

impl YoloBrokerReserveDisclosureV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != YOLO_BROKER_DISCLOSURE_SCHEMA_V1 {
            return Err("YOLO broker disclosure schema mismatch".to_string());
        }
        validate_identifier("broker_source_id", &self.broker_source_id)?;
        if self.observed_at_unix_millis == 0 {
            return Err("YOLO broker observed_at_unix_millis must be nonzero".to_string());
        }
        for (field, value) in [
            (
                "account_application_identity_sha256",
                &self.account_application_identity_sha256,
            ),
            ("collection_epoch_sha256", &self.collection_epoch_sha256),
            ("basket_decision_sha256", &self.basket_decision_sha256),
            (
                "execution_receipts_root_sha256",
                &self.execution_receipts_root_sha256,
            ),
            (
                "broker_response_root_sha256",
                &self.broker_response_root_sha256,
            ),
            ("positions_root_sha256", &self.positions_root_sha256),
            ("cash_root_sha256", &self.cash_root_sha256),
            ("liabilities_root_sha256", &self.liabilities_root_sha256),
            ("open_orders_root_sha256", &self.open_orders_root_sha256),
            ("fills_root_sha256", &self.fills_root_sha256),
            (
                "pending_events_root_sha256",
                &self.pending_events_root_sha256,
            ),
            (
                "valuation_inputs_root_sha256",
                &self.valuation_inputs_root_sha256,
            ),
        ] {
            validate_hex(field, value, 32)?;
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = Vec::new();
        append_bytes(&mut out, self.schema.as_bytes())?;
        append_bytes(&mut out, self.broker_source_id.as_bytes())?;
        append_bytes(
            &mut out,
            self.account_application_identity_sha256.as_bytes(),
        )?;
        out.extend_from_slice(&self.observed_at_unix_millis.to_be_bytes());
        for value in [
            &self.collection_epoch_sha256,
            &self.basket_decision_sha256,
            &self.execution_receipts_root_sha256,
            &self.broker_response_root_sha256,
            &self.positions_root_sha256,
            &self.cash_root_sha256,
            &self.liabilities_root_sha256,
            &self.open_orders_root_sha256,
            &self.fills_root_sha256,
            &self.pending_events_root_sha256,
            &self.valuation_inputs_root_sha256,
        ] {
            append_bytes(&mut out, value.as_bytes())?;
        }
        Ok(out)
    }

    pub fn evidence_commitments(
        &self,
        gross_assets: u64,
        total_liabilities: u64,
        valuation_scale: u64,
    ) -> Result<YoloBrokerEvidenceCommitmentsV1, String> {
        if total_liabilities > gross_assets {
            return Err("YOLO broker liabilities exceed gross assets".to_string());
        }
        if valuation_scale == 0 {
            return Err("YOLO broker valuation scale must be nonzero".to_string());
        }
        let canonical = self.canonical_bytes()?;

        let mut quantity = Vec::new();
        append_bytes(
            &mut quantity,
            self.account_application_identity_sha256.as_bytes(),
        )?;
        quantity.extend_from_slice(&self.observed_at_unix_millis.to_be_bytes());
        for value in [
            &self.broker_response_root_sha256,
            &self.positions_root_sha256,
            &self.cash_root_sha256,
            &self.liabilities_root_sha256,
            &self.open_orders_root_sha256,
            &self.fills_root_sha256,
            &self.pending_events_root_sha256,
        ] {
            append_bytes(&mut quantity, value.as_bytes())?;
        }
        let quantity_evidence_commitment =
            opaque_commitment(YOLO_QUANTITY_COMMITMENT_LABEL_V1, &quantity)?;

        let mut valuation = Vec::new();
        append_bytes(&mut valuation, quantity_evidence_commitment.as_bytes())?;
        append_bytes(&mut valuation, self.valuation_inputs_root_sha256.as_bytes())?;
        valuation.extend_from_slice(&gross_assets.to_be_bytes());
        valuation.extend_from_slice(&total_liabilities.to_be_bytes());
        valuation.extend_from_slice(&valuation_scale.to_be_bytes());
        let valuation_evidence_commitment =
            opaque_commitment(YOLO_VALUATION_COMMITMENT_LABEL_V1, &valuation)?;

        let mut disclosure = canonical;
        append_bytes(&mut disclosure, quantity_evidence_commitment.as_bytes())?;
        append_bytes(&mut disclosure, valuation_evidence_commitment.as_bytes())?;
        disclosure.extend_from_slice(&gross_assets.to_be_bytes());
        disclosure.extend_from_slice(&total_liabilities.to_be_bytes());
        disclosure.extend_from_slice(&valuation_scale.to_be_bytes());
        let disclosure_commitment =
            opaque_commitment(YOLO_DISCLOSURE_COMMITMENT_LABEL_V1, &disclosure)?;

        Ok(YoloBrokerEvidenceCommitmentsV1 {
            quantity_evidence_commitment,
            valuation_evidence_commitment,
            disclosure_commitment,
        })
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::{
        ed25519_evidence_signing_statement, ed25519_verifier_commitment, execute_reserve_proof,
        EvidenceDimensionV1, FreshnessPolicyV1, LiabilityTreatmentV1, ReserveProofContextV1,
        ReserveProofWitnessV1, SourceEvidenceV1, SourceManifestEntryV1, SourceManifestV1,
        SourceObservationV1, TrustClassV1, MANIFEST_SCHEMA_V1, WITNESS_SCHEMA_V1,
    };

    fn disclosure() -> YoloBrokerReserveDisclosureV1 {
        YoloBrokerReserveDisclosureV1 {
            schema: YOLO_BROKER_DISCLOSURE_SCHEMA_V1.to_string(),
            broker_source_id: "schwab-trader-api-individual".to_string(),
            account_application_identity_sha256: "01".repeat(32),
            observed_at_unix_millis: 1_788_540_000_000,
            collection_epoch_sha256: "02".repeat(32),
            basket_decision_sha256: "03".repeat(32),
            execution_receipts_root_sha256: "04".repeat(32),
            broker_response_root_sha256: "05".repeat(32),
            positions_root_sha256: "06".repeat(32),
            cash_root_sha256: "07".repeat(32),
            liabilities_root_sha256: "08".repeat(32),
            open_orders_root_sha256: "09".repeat(32),
            fills_root_sha256: "0a".repeat(32),
            pending_events_root_sha256: "0b".repeat(32),
            valuation_inputs_root_sha256: "0c".repeat(32),
        }
    }

    #[test]
    fn disclosure_commitments_are_deterministic_and_bind_private_roots() {
        let first = disclosure()
            .evidence_commitments(1_000, 100, 1_000_000)
            .unwrap();
        assert_eq!(
            first.quantity_evidence_commitment,
            "005cac6ea12c95763e137429fa22d19058c78472b819c4ccb670013b72e817427a327336e85e1f5dc02e536898e8f5c7"
        );
        assert_eq!(
            first.valuation_evidence_commitment,
            "b2b7f36933790762618cfb528b77d5733091290f702f7c4c679d290122839c19868bf8e3204cfab891b47d7d8385dc27"
        );
        assert_eq!(
            first.disclosure_commitment,
            "782076ab3d4ae70654b6f3de5f0d555e71a78c78fee734e4dcc397322be8d5ecf6f3857fb66b3521193bbdfade90ff1b"
        );
        let mut changed = disclosure();
        changed.positions_root_sha256 = "ff".repeat(32);
        let second = changed.evidence_commitments(1_000, 100, 1_000_000).unwrap();

        assert_eq!(
            first,
            disclosure()
                .evidence_commitments(1_000, 100, 1_000_000)
                .unwrap()
        );
        assert_ne!(
            first.quantity_evidence_commitment,
            second.quantity_evidence_commitment
        );
        assert_ne!(
            first.valuation_evidence_commitment,
            second.valuation_evidence_commitment
        );
        assert_ne!(first.disclosure_commitment, second.disclosure_commitment);
    }

    #[test]
    fn disclosure_rejects_invalid_context_and_totals() {
        assert!(disclosure().evidence_commitments(100, 101, 1).is_err());
        assert!(disclosure().evidence_commitments(100, 0, 0).is_err());
        let mut invalid = disclosure();
        invalid.schema.push_str("-unknown");
        assert!(invalid.evidence_commitments(100, 0, 1).is_err());
        let mut invalid = disclosure();
        invalid.broker_source_id.clear();
        assert!(invalid.evidence_commitments(100, 0, 1).is_err());
        let mut invalid = disclosure();
        invalid.observed_at_unix_millis = 0;
        assert!(invalid.evidence_commitments(100, 0, 1).is_err());
        let mut invalid = disclosure();
        invalid.account_application_identity_sha256 = "00".repeat(31);
        assert!(invalid.evidence_commitments(100, 0, 1).is_err());
        let mut invalid = disclosure();
        invalid.valuation_inputs_root_sha256 = "zz".repeat(32);
        assert!(invalid.evidence_commitments(100, 0, 1).is_err());
    }

    #[test]
    fn complete_reserve_proof_keeps_brokerage_value_attested() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let verifying_key = hex::encode(signing_key.verifying_key().to_bytes());
        let verifier_commitment = ed25519_verifier_commitment(&verifying_key).unwrap();
        let commitments = disclosure()
            .evidence_commitments(1_000, 100, 1_000_000)
            .unwrap();
        let manifest = SourceManifestV1 {
            schema: MANIFEST_SCHEMA_V1.to_string(),
            sources: vec![SourceManifestEntryV1 {
                source_id: "broker-account".to_string(),
                adapter_kind: YOLO_BROKER_ADAPTER_KIND_V1.to_string(),
                source_domain: "schwab-trader-api-individual".to_string(),
                asset_or_position_id: "yolo-account-commitment".to_string(),
                reserve_owner_commitment: "11".repeat(48),
                quantity_verifier_commitment: verifier_commitment.clone(),
                valuation_verifier_commitment: verifier_commitment,
                quantity_evidence_class: TrustClassV1::Attested,
                valuation_evidence_class: TrustClassV1::Attested,
                freshness_policy: FreshnessPolicyV1 {
                    max_age_blocks: 5,
                    max_observation_span_blocks: 5,
                },
                haircut_policy_hash: "12".repeat(48),
                liability_treatment: LiabilityTreatmentV1::Asset,
                adapter_schema_version: 1,
            }],
        };
        let context = ReserveProofContextV1 {
            pftl_genesis_hash: "21".repeat(48),
            nav_asset_id: "22".repeat(48),
            proof_profile_id: "23".repeat(48),
            valuation_policy_hash: "24".repeat(32),
            source_manifest_hash: manifest.hash().unwrap(),
            valuation_unit_id: "25".repeat(48),
            valuation_scale: 1_000_000,
            observation_epoch: 1,
            observation_not_before: 100,
            observation_not_after: 100,
        };
        let mut observation = SourceObservationV1 {
            source_id: "broker-account".to_string(),
            observed_at_block: 100,
            gross_assets: 1_000,
            total_liabilities: 100,
            quantity_evidence: SourceEvidenceV1::AttestedEd25519 {
                evidence_commitment: commitments.quantity_evidence_commitment,
                verifier_public_key: verifying_key.clone(),
                signature: "00".repeat(64),
            },
            valuation_evidence: SourceEvidenceV1::AttestedEd25519 {
                evidence_commitment: commitments.valuation_evidence_commitment,
                verifier_public_key: verifying_key,
                signature: "00".repeat(64),
            },
            disclosure_commitment: commitments.disclosure_commitment,
        };
        for dimension in [
            EvidenceDimensionV1::Quantity,
            EvidenceDimensionV1::Valuation,
        ] {
            let statement = ed25519_evidence_signing_statement(
                &context,
                &manifest.sources[0],
                &observation,
                dimension,
            )
            .unwrap();
            let signature = hex::encode(signing_key.sign(&statement).to_bytes());
            match dimension {
                EvidenceDimensionV1::Quantity => {
                    if let SourceEvidenceV1::AttestedEd25519 {
                        signature: slot, ..
                    } = &mut observation.quantity_evidence
                    {
                        *slot = signature;
                    }
                }
                EvidenceDimensionV1::Valuation => {
                    if let SourceEvidenceV1::AttestedEd25519 {
                        signature: slot, ..
                    } = &mut observation.valuation_evidence
                    {
                        *slot = signature;
                    }
                }
            }
        }
        let witness = ReserveProofWitnessV1 {
            schema: WITNESS_SCHEMA_V1.to_string(),
            context,
            manifest,
            observations: vec![observation],
        };

        let public = execute_reserve_proof(&witness).unwrap();

        assert_eq!(public.verified_net_assets, 900);
        assert_eq!(public.attested_value, 900);
        assert_eq!(public.cryptographically_verified_value, 0);
        assert_eq!(public.quantity_trust_counts.attested, 1);
        assert_eq!(public.valuation_trust_counts.attested, 1);
    }
}
