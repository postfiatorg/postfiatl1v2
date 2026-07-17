use std::cell::RefCell;
use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetOrchardSwapTimingReport {
    pub schema: String,
    pub vk_builds: Vec<AssetOrchardSwapVkBuildTimingReport>,
    pub vk_cached_calls: Vec<AssetOrchardSwapVkCachedTimingReport>,
    #[serde(default)]
    pub proof_verifications: Vec<AssetOrchardSwapProofVerifyTimingReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardSwapVkBuildTimingReport {
    pub schema: String,
    pub artifact_mode: String,
    pub total_ms: f64,
    pub params_new_ms: f64,
    pub full_shape_ms: f64,
    pub artifact_read_ms: f64,
    pub artifact_decode_ms: f64,
    pub artifact_vk_reconstruct_ms: f64,
    pub keygen_vk_ms: f64,
    pub metadata_ms: f64,
    pub release_pin_validation_ms: f64,
    pub artifact_write_ms: f64,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardSwapVkCachedTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub cache_was_populated: bool,
    pub build_triggered: bool,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardSwapProofVerifyTimingReport {
    pub schema: String,
    pub halo2_verify_proof_ms: f64,
    pub result: String,
}

#[derive(Default)]
struct AssetOrchardSwapTimingCollector {
    vk_builds: Vec<AssetOrchardSwapVkBuildTimingReport>,
    vk_cached_calls: Vec<AssetOrchardSwapVkCachedTimingReport>,
    proof_verifications: Vec<AssetOrchardSwapProofVerifyTimingReport>,
}

thread_local! {
    static SWAP_TIMINGS: RefCell<AssetOrchardSwapTimingCollector> =
        RefCell::new(AssetOrchardSwapTimingCollector::default());
}

pub fn reset_asset_orchard_swap_timings() {
    SWAP_TIMINGS.with(|collector| {
        *collector.borrow_mut() = AssetOrchardSwapTimingCollector::default();
    });
}

pub fn take_asset_orchard_swap_timings() -> AssetOrchardSwapTimingReport {
    SWAP_TIMINGS.with(|collector| {
        let mut collector = collector.borrow_mut();
        AssetOrchardSwapTimingReport {
            schema: "postfiat.asset_orchard_swap.timings.v1".to_string(),
            vk_builds: std::mem::take(&mut collector.vk_builds),
            vk_cached_calls: std::mem::take(&mut collector.vk_cached_calls),
            proof_verifications: std::mem::take(&mut collector.proof_verifications),
        }
    })
}

pub(crate) fn record_asset_orchard_swap_vk_build_timing(
    timing: AssetOrchardSwapVkBuildTimingReport,
) {
    SWAP_TIMINGS.with(|collector| collector.borrow_mut().vk_builds.push(timing));
}

pub(crate) fn record_asset_orchard_swap_vk_cached_timing(
    timing: AssetOrchardSwapVkCachedTimingReport,
) {
    SWAP_TIMINGS.with(|collector| collector.borrow_mut().vk_cached_calls.push(timing));
}

pub(crate) fn record_asset_orchard_swap_proof_verify_timing(
    timing: AssetOrchardSwapProofVerifyTimingReport,
) {
    SWAP_TIMINGS.with(|collector| collector.borrow_mut().proof_verifications.push(timing));
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetOrchardPrivateEgressTimingReport {
    pub schema: String,
    pub action_builds: Vec<AssetOrchardPrivateEgressActionBuildTimingReport>,
    pub vk_builds: Vec<AssetOrchardPrivateEgressVkBuildTimingReport>,
    pub vk_cached_calls: Vec<AssetOrchardPrivateEgressVkCachedTimingReport>,
    pub proof_verifications: Vec<AssetOrchardPrivateEgressProofVerifyTimingReport>,
    pub action_verifications: Vec<AssetOrchardPrivateEgressActionVerifyTimingReport>,
}

impl AssetOrchardPrivateEgressTimingReport {
    pub fn observed_total_ms(&self) -> f64 {
        self.action_builds
            .iter()
            .map(|timing| timing.total_ms)
            .sum::<f64>()
            + self
                .vk_cached_calls
                .iter()
                .map(|timing| timing.total_ms)
                .sum::<f64>()
            + self
                .action_verifications
                .iter()
                .map(|timing| timing.total_ms)
                .sum::<f64>()
    }

    pub fn halo2_verify_proof_ms(&self) -> f64 {
        self.proof_verifications
            .iter()
            .map(|timing| timing.halo2_verify_proof_ms)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.action_builds.is_empty()
            && self.vk_builds.is_empty()
            && self.vk_cached_calls.is_empty()
            && self.proof_verifications.is_empty()
            && self.action_verifications.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressActionBuildTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub pre_key_ms: f64,
    pub key_build_ms: f64,
    pub proof_gen_ms: f64,
    pub post_proof_ms: f64,
    pub validation_domain_ms: f64,
    pub merkle_witness_ms: f64,
    pub signing_witness_prep_ms: f64,
    pub nullifier_rvk_ms: f64,
    pub exit_binding_public_fields_circuit_ms: f64,
    pub proving_key_cached_ms: f64,
    pub proof_generation_ms: f64,
    pub action_assembly_sighash_signature_ms: f64,
    pub result: String,
}

impl Default for AssetOrchardPrivateEgressActionBuildTimingReport {
    fn default() -> Self {
        Self {
            schema: "postfiat.asset_orchard_private_egress.action_build_timing.v1".to_string(),
            total_ms: 0.0,
            pre_key_ms: 0.0,
            key_build_ms: 0.0,
            proof_gen_ms: 0.0,
            post_proof_ms: 0.0,
            validation_domain_ms: 0.0,
            merkle_witness_ms: 0.0,
            signing_witness_prep_ms: 0.0,
            nullifier_rvk_ms: 0.0,
            exit_binding_public_fields_circuit_ms: 0.0,
            proving_key_cached_ms: 0.0,
            proof_generation_ms: 0.0,
            action_assembly_sighash_signature_ms: 0.0,
            result: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressVkBuildTimingReport {
    pub schema: String,
    pub artifact_mode: String,
    pub total_ms: f64,
    pub params_new_ms: f64,
    pub full_shape_ms: f64,
    pub artifact_read_ms: f64,
    pub artifact_decode_ms: f64,
    pub artifact_vk_reconstruct_ms: f64,
    pub keygen_vk_ms: f64,
    pub metadata_ms: f64,
    pub release_pin_validation_ms: f64,
    pub artifact_write_ms: f64,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressVkCachedTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub cache_was_populated: bool,
    pub build_triggered: bool,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressProofVerifyTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub vk_metadata_recompute_ms: f64,
    pub instance_setup_ms: f64,
    pub verifier_strategy_ms: f64,
    pub transcript_init_ms: f64,
    pub halo2_verify_proof_ms: f64,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressActionVerifyTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub metadata_pin_validation_ms: f64,
    pub domain_binding_ms: f64,
    pub exit_binding_ms: f64,
    pub spend_auth_verification_ms: f64,
    pub public_instance_construction_ms: f64,
    pub proof_bytes_ms: f64,
    pub verifying_key_cached_ms: f64,
    pub halo2_verify_proof_ms: f64,
    pub result: String,
}

impl Default for AssetOrchardPrivateEgressActionVerifyTimingReport {
    fn default() -> Self {
        Self {
            schema: "postfiat.asset_orchard_private_egress.action_verify_timing.v1".to_string(),
            total_ms: 0.0,
            metadata_pin_validation_ms: 0.0,
            domain_binding_ms: 0.0,
            exit_binding_ms: 0.0,
            spend_auth_verification_ms: 0.0,
            public_instance_construction_ms: 0.0,
            proof_bytes_ms: 0.0,
            verifying_key_cached_ms: 0.0,
            halo2_verify_proof_ms: 0.0,
            result: "unknown".to_string(),
        }
    }
}

#[derive(Default)]
struct AssetOrchardPrivateEgressTimingCollector {
    action_builds: Vec<AssetOrchardPrivateEgressActionBuildTimingReport>,
    vk_builds: Vec<AssetOrchardPrivateEgressVkBuildTimingReport>,
    vk_cached_calls: Vec<AssetOrchardPrivateEgressVkCachedTimingReport>,
    proof_verifications: Vec<AssetOrchardPrivateEgressProofVerifyTimingReport>,
    action_verifications: Vec<AssetOrchardPrivateEgressActionVerifyTimingReport>,
}

thread_local! {
    static PRIVATE_EGRESS_TIMINGS: RefCell<AssetOrchardPrivateEgressTimingCollector> =
        RefCell::new(AssetOrchardPrivateEgressTimingCollector::default());
}

pub fn asset_orchard_timing_elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

pub fn reset_asset_orchard_private_egress_timings() {
    PRIVATE_EGRESS_TIMINGS.with(|collector| {
        *collector.borrow_mut() = AssetOrchardPrivateEgressTimingCollector::default();
    });
}

pub fn take_asset_orchard_private_egress_timings() -> AssetOrchardPrivateEgressTimingReport {
    PRIVATE_EGRESS_TIMINGS.with(|collector| {
        let mut collector = collector.borrow_mut();
        let report = AssetOrchardPrivateEgressTimingReport {
            schema: "postfiat.asset_orchard_private_egress.timings.v1".to_string(),
            action_builds: std::mem::take(&mut collector.action_builds),
            vk_builds: std::mem::take(&mut collector.vk_builds),
            vk_cached_calls: std::mem::take(&mut collector.vk_cached_calls),
            proof_verifications: std::mem::take(&mut collector.proof_verifications),
            action_verifications: std::mem::take(&mut collector.action_verifications),
        };
        report
    })
}

pub(crate) fn record_asset_orchard_private_egress_action_build_timing(
    timing: AssetOrchardPrivateEgressActionBuildTimingReport,
) {
    PRIVATE_EGRESS_TIMINGS.with(|collector| {
        collector.borrow_mut().action_builds.push(timing);
    });
}

pub(crate) fn record_asset_orchard_private_egress_vk_build_timing(
    timing: AssetOrchardPrivateEgressVkBuildTimingReport,
) {
    PRIVATE_EGRESS_TIMINGS.with(|collector| {
        collector.borrow_mut().vk_builds.push(timing);
    });
}

pub(crate) fn record_asset_orchard_private_egress_vk_cached_timing(
    timing: AssetOrchardPrivateEgressVkCachedTimingReport,
) {
    PRIVATE_EGRESS_TIMINGS.with(|collector| {
        collector.borrow_mut().vk_cached_calls.push(timing);
    });
}

pub(crate) fn record_asset_orchard_private_egress_proof_verify_timing(
    timing: AssetOrchardPrivateEgressProofVerifyTimingReport,
) {
    PRIVATE_EGRESS_TIMINGS.with(|collector| {
        collector.borrow_mut().proof_verifications.push(timing);
    });
}

pub(crate) fn record_asset_orchard_private_egress_action_verify_timing(
    timing: AssetOrchardPrivateEgressActionVerifyTimingReport,
) {
    PRIVATE_EGRESS_TIMINGS.with(|collector| {
        collector.borrow_mut().action_verifications.push(timing);
    });
}
