//! Orchard/Halo2 adapter boundary for PostFiat shielded privacy.
//!
//! This crate intentionally exposes PostFiat-owned wrapper types instead of
//! leaking upstream Orchard structs into consensus serialization.

#![allow(
    clippy::let_and_return,
    clippy::manual_contains,
    clippy::manual_div_ceil,
    clippy::manual_is_multiple_of,
    clippy::needless_range_loop,
    clippy::too_many_arguments
)]

pub mod asset_orchard;
pub mod asset_orchard_circuit;
mod asset_orchard_note_encryption;
pub mod asset_orchard_sinsemilla;
mod timing;
mod types;
mod verify;

pub use asset_orchard::{
    asset_derive_nullifier, asset_note_message_bits, asset_orchard_accounting_commitment_sum,
    asset_orchard_accounting_record, asset_orchard_accounting_value_commitment,
    asset_orchard_disclosed_egress_sighash, asset_orchard_egress_randomizer,
    asset_orchard_private_egress_exit_binding_hash, asset_orchard_private_egress_sighash,
    asset_orchard_scalar_from_hex, asset_orchard_swap_accounting_records,
    asset_orchard_wallet_note_nullifier, asset_output_rho,
    build_asset_orchard_disclosed_egress_authorization, build_asset_orchard_wallet_note,
    build_asset_orchard_wallet_note_with_rho, const_field, encrypted_output_hash, field_enc,
    h_action, h_sig, hash_to_pallas_base, hash_to_pallas_scalar_nonzero, orchard_commit_ivk,
    orchard_psi, orchard_rcm, point_enc, pool_domain, poseidon_hash1, poseidon_hash2,
    private_egress_action_binding_hash, private_egress_h_action, scalar_enc, swap_binding_hash,
    validate_asset_orchard_wallet_note_for_pool, verify_asset_orchard_disclosed_egress,
    AssetNoteOpening, AssetOrchardActionPublicFields, AssetOrchardBoundedBytes,
    AssetOrchardDisclosedEgressAuthorization, AssetOrchardDisclosedEgressCheck,
    AssetOrchardDisclosedEgressPreimage, AssetOrchardError, AssetOrchardFieldElement,
    AssetOrchardPoint, AssetOrchardPricingClaim, AssetOrchardPricingPublicFields,
    AssetOrchardPrivateEgressAction, AssetOrchardPrivateEgressExitBindingPreimage,
    AssetOrchardPrivateEgressPreimage, AssetOrchardPrivateEgressPublicFields,
    AssetOrchardProofBytes, AssetOrchardPublicNoteOpening, AssetOrchardSigPreimage,
    AssetOrchardSpendAuthSignature, AssetOrchardSwapAccountingRecord, AssetOrchardSwapAction,
    AssetOrchardSwapBindingHash, AssetOrchardWalletNote, AssetTag, EncryptedOutputHash,
    PoolDomainInput, RandomizedVerificationKeyFields, ASSET_ORCHARD_ACTION_SCHEMA_V1,
    ASSET_ORCHARD_ACTION_VERSION_V1, ASSET_ORCHARD_CIRCUIT_ID_V1,
    ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY, ASSET_ORCHARD_CIRCUIT_ID_V4,
    ASSET_ORCHARD_DISCLOSED_EGRESS_SCHEMA_V1, ASSET_ORCHARD_DIVERSIFIER_BYTES,
    ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES, ASSET_ORCHARD_FIELD_BYTES, ASSET_ORCHARD_LEG_COUNT,
    ASSET_ORCHARD_MAX_ASSET_ID_BYTES, ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1,
    ASSET_ORCHARD_NOTE_VERSION_V1, ASSET_ORCHARD_POINT_BYTES, ASSET_ORCHARD_POOL_ID_V1,
    ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1, ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
    ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY, ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2,
    ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN, ASSET_ORCHARD_PROOF_MAX_BYTES,
    ASSET_ORCHARD_PROOF_SYSTEM_ID_V1, ASSET_ORCHARD_RSEED_BYTES, ASSET_ORCHARD_SIGHASH_BYTES,
    ASSET_ORCHARD_SPEND_AUTH_SIGNATURE_BYTES, ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES,
};
pub use asset_orchard_circuit::{
    build_asset_orchard_private_egress_action, build_asset_orchard_swap_action,
    AssetOrchardPrivateEgressBuildResult, AssetOrchardPrivateEgressCircuit,
    AssetOrchardPrivateEgressPinnedMetadata, AssetOrchardPrivateEgressProvingKey,
    AssetOrchardPrivateEgressVerifyingKey, AssetOrchardSwapBuildResult,
    AssetOrchardSwapConservationCircuit, AssetOrchardSwapConservationConfig,
    AssetOrchardSwapPinnedMetadata, AssetOrchardSwapPrivateLeg, AssetOrchardSwapProvingKey,
    AssetOrchardSwapVerifyingKey, ASSET_ORCHARD_PRIVATE_EGRESS_V1_K,
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_MERKLE_PARAMETER_HASH,
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_NOTE_MESSAGE_LAYOUT_HASH,
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_PARAMS_HASH,
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_POSEIDON_PARAMETER_HASH,
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_PUBLIC_INSTANCE_LAYOUT_HASH,
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT,
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_HASH, ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH,
    ASSET_ORCHARD_SWAP_V1_K, ASSET_ORCHARD_SWAP_V1_MERKLE_PARAMETER_HASH,
    ASSET_ORCHARD_SWAP_V1_NOTE_MESSAGE_LAYOUT_HASH, ASSET_ORCHARD_SWAP_V1_PARAMS_HASH,
    ASSET_ORCHARD_SWAP_V1_POSEIDON_PARAMETER_HASH,
    ASSET_ORCHARD_SWAP_V1_PUBLIC_INSTANCE_LAYOUT_HASH, ASSET_ORCHARD_SWAP_V1_VK_HASH,
    ASSET_ORCHARD_SWAP_V3_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT,
    ASSET_ORCHARD_SWAP_V3_REPLAY_VK_HASH,
};
pub use asset_orchard_note_encryption::{
    decrypt_asset_orchard_wallet_note, encrypt_asset_orchard_wallet_note,
    ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC,
};
pub use timing::{
    reset_asset_orchard_private_egress_timings, reset_asset_orchard_swap_timings,
    take_asset_orchard_private_egress_timings, take_asset_orchard_swap_timings,
    AssetOrchardPrivateEgressActionBuildTimingReport,
    AssetOrchardPrivateEgressActionVerifyTimingReport,
    AssetOrchardPrivateEgressProofVerifyTimingReport, AssetOrchardPrivateEgressTimingReport,
    AssetOrchardPrivateEgressVkBuildTimingReport, AssetOrchardPrivateEgressVkCachedTimingReport,
    AssetOrchardSwapProofVerifyTimingReport, AssetOrchardSwapTimingReport,
    AssetOrchardSwapVkBuildTimingReport, AssetOrchardSwapVkCachedTimingReport,
};
pub use types::{
    BoundedHexBlob, EncryptedShieldedOutput, OrchardAnchor, OrchardBindingSignature,
    OrchardCircuitId, OrchardFlags, OrchardNullifier, OrchardOutputCommitment, OrchardProofBytes,
    OrchardProofSystemId, OrchardRandomizedVerificationKey, OrchardShieldedAction,
    OrchardSpendAuthSignature, OrchardTypeError, OrchardValueCommitment, ShieldedSwapAction,
    ShieldedSwapCommitment, ORCHARD_ACTION_CIRCUIT_ID, ORCHARD_ANCHOR_BYTES,
    ORCHARD_CIPHERTEXT_MAX_BYTES, ORCHARD_COMMITMENT_BYTES, ORCHARD_COMPACT_CIPHERTEXT_BYTES,
    ORCHARD_ENC_CIPHERTEXT_BYTES, ORCHARD_EPK_BYTES, ORCHARD_EXTERNAL_BINDING_HASH_BYTES,
    ORCHARD_NULLIFIER_BYTES, ORCHARD_OUT_CIPHERTEXT_BYTES, ORCHARD_PROOF_MAX_BYTES,
    ORCHARD_PROOF_SYSTEM_ID, ORCHARD_RANDOMIZED_VERIFICATION_KEY_BYTES,
    ORCHARD_REDPALLAS_SIGNATURE_BYTES, ORCHARD_VALUE_COMMITMENT_BYTES, SHIELDED_SWAP_ACTION_SCHEMA,
    SHIELDED_SWAP_CIRCUIT_ID, SHIELDED_SWAP_COMMITMENT_BYTES,
    SHIELDED_SWAP_LEGACY_TRANSCRIPT_HASH_BYTES, SHIELDED_SWAP_LEG_COUNT,
    SHIELDED_SWAP_PROOF_SYSTEM_ID,
};
pub use verify::{
    asset_orchard_domain_genesis_hash, orchard_action_from_authorized_bundle,
    orchard_action_from_authorized_bundle_with_external_binding, orchard_anchor_from_commitments,
    orchard_authorizing_sighash, orchard_authorizing_sighash_with_external_binding,
    orchard_build_output_action, orchard_build_output_action_test_vector,
    orchard_build_output_action_with_external_binding, orchard_build_spend_action,
    orchard_build_withdraw_action, orchard_bundle_from_action,
    orchard_default_address_from_full_viewing_key, orchard_default_address_from_spending_key,
    orchard_empty_anchor, orchard_frontier_snapshot_append_commitments,
    orchard_frontier_snapshot_from_commitments, orchard_full_viewing_key_from_spending_key,
    orchard_merkle_witness_from_commitments, orchard_scan_encrypted_outputs_with_full_viewing_key,
    orchard_scan_encrypted_outputs_with_spending_key, orchard_spending_key_from_zip32_seed,
    shielded_swap_asset_commitment, shielded_swap_authorization_commitment,
    shielded_swap_authorization_proof, shielded_swap_build_action_test_vector,
    shielded_swap_transcript_hash, shielded_swap_value_commitment,
    validate_asset_orchard_pricing_policy, verify_authorized_bundle,
    verify_serialized_asset_orchard_private_egress_action,
    verify_serialized_asset_orchard_private_egress_action_for_archive_replay,
    verify_serialized_asset_orchard_swap_action,
    verify_serialized_asset_orchard_swap_action_for_archive_replay,
    verify_serialized_orchard_action, verify_serialized_orchard_action_with_built_key,
    verify_serialized_shielded_swap_action, AssetOrchardPricingClaimEvidence,
    AssetOrchardPricingClaimProvenance, AssetOrchardPricingPolicy, OrchardAuthorizingDomain,
    OrchardDecryptedOutput, OrchardFrontierSnapshot, OrchardMerkleWitness, OrchardSpendNote,
    OrchardVerificationContext, OrchardVerificationError, ShieldedSwapPrivateInput,
    ShieldedSwapPrivateOutput, VerifiedAssetOrchardPricingClaim, VerifiedAssetOrchardPrivateEgress,
    VerifiedAssetOrchardSwap, VerifiedOrchardBundle, VerifiedShieldedSwap,
    DEFAULT_MAX_ORCHARD_ACTIONS, ORCHARD_AUTHORIZING_SIGHASH_DOMAIN, ORCHARD_MEMO_BYTES,
    ORCHARD_RAW_ADDRESS_BYTES, POSTFIAT_ORCHARD_COIN_TYPE,
};

pub const CRATE_PURPOSE: &str = "Orchard/Halo2 shielded pool adapter boundary";

#[cfg(test)]
mod tests {
    use orchard::{
        bundle::{Authorized, Bundle},
        circuit::VerifyingKey,
        note::Nullifier,
    };

    #[test]
    fn orchard_public_api_is_available() {
        let bundle_type = std::any::type_name::<Bundle<Authorized, ()>>();
        let nullifier_type = std::any::type_name::<Nullifier>();
        let verifying_key_type = std::any::type_name::<VerifyingKey>();

        assert!(bundle_type.contains("orchard"));
        assert!(nullifier_type.contains("orchard"));
        assert!(verifying_key_type.contains("orchard"));
    }
}
