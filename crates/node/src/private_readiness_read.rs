//! Readiness of this node for user-proven private actions, attested from one
//! committed chain-tip state and the running binary: the pinned verifier
//! identities and whether each verifier is actually built in this process,
//! the governed activations, the governed A666/pfUSDC swap route, and the
//! serving process's custody of its store and validator key. Configuration
//! alone can never make `ready` true, and neither can a local read of a data
//! directory, a comparison store, a plaintext key-file signer or a key file
//! another local user can read.

use super::asset_orchard_pricing_reads::{read_finalized_pair_snapshot, FinalizedPairSnapshot};
use super::*;
use postfiat_privacy_orchard::{asset_orchard as ao, asset_orchard_circuit as aoc};

pub const PRIVATE_READINESS_SCHEMA: &str = "postfiat.private_readiness.v1";
/// The private-action admission switch is `rpc-serve
/// --allow-private-action-admission`: configuration of the serving process,
/// which this read (run by the RPC dispatcher, also from the CLI) does not
/// receive. It is reported `unknown` rather than taken from a caller-supplied
/// parameter, and it never makes `ready` true.
pub const PRIVATE_ACTION_ADMISSION_SWITCH_KEYS: [&str; 1] = ["--allow-private-action-admission"];
/// The custody refusals, in report order. The W-1 manifest lists the same.
pub const PRIVATE_READINESS_CUSTODY_REASONS: [&str; 5] = [
    "readiness_not_published_by_running_service",
    "store_not_transactional",
    "validator_signer_unsafe_file",
    "validator_key_file_missing",
    "validator_key_file_not_owner_only",
];
const PRIVATE_READINESS_MAX_ROUTES: usize = 16;
/// The W-1 pair on the devnet, as the wallet contract pins it
/// (`crates/node/tests/fixtures/private-actions-v1/assets.json`): the A666 NAV
/// asset and pfUSDC. Readiness finds the governed route by these ids, as the
/// pricing rule finds a claim's route (`enforce_asset_orchard_at_nav_pricing_rules`);
/// asset codes are not unique and select nothing.
pub const PRIVATE_READINESS_A666_ASSET_ID: &str = "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c";
pub const PRIVATE_READINESS_PFUSDC_ASSET_ID: &str = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateReadinessOptions {
    pub data_dir: PathBuf,
    pub pool_id: Option<String>,
    /// The serving process's own `--allow-private-action-admission` setting;
    /// `None` (a local read with no server) reports `unknown`. Never taken
    /// from request parameters.
    pub admission_switch: Option<bool>,
    /// The running service's own custody, supplied only by `rpc-serve`;
    /// `None` is a local read of `data_dir`, which may be a copy.
    pub service: Option<PrivateReadinessService>,
}

/// How the serving process signs as its validator: the facts `rpc-serve`
/// knows about itself, never request parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateReadinessService {
    pub key_file: PathBuf,
    pub signer: PrivateValidatorSigner,
}

/// The validator signer of the serving process. A plaintext key file is the
/// only signer built today (`--finality-key-file`, the same exportable file
/// that `--unsafe-devnet-file-signer` acknowledges), so a release build never
/// reports custody satisfied. The hardened signer is test-only until a
/// remote signer exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateValidatorSigner {
    PlaintextKeyFile,
    #[cfg(test)]
    TestHardened,
}

impl PrivateValidatorSigner {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlaintextKeyFile => "plaintext_key_file",
            #[cfg(test)]
            Self::TestHardened => "hardened",
        }
    }
}

/// Who published the report and what custody the publisher has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateReadinessCustody {
    /// `rpc_serve` when the running service published it, else `local_read`.
    pub published_by: String,
    /// The backend of the store read: `transactional`, or a JSONL
    /// comparison backend (`bounded-jsonl`, `legacy-jsonl`).
    pub store_backend: String,
    /// `plaintext_key_file`, or `none` for a local read.
    pub validator_signer: String,
    /// Whether the validator key file grants no group or other access;
    /// `None` when there is no key file to inspect.
    pub key_file_owner_only: Option<bool>,
}

/// The identity a verifier proves it was built with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateVerifierIdentity {
    pub proof_system_id: String,
    pub circuit_id: String,
    pub k: u32,
    pub params_hash: String,
    pub vk_hash: String,
    pub runtime_pinned_vk_fingerprint: String,
    pub public_instance_layout_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateVerifierReadiness {
    /// The identity this binary pins (compiled-in constants).
    pub pinned: PrivateVerifierIdentity,
    /// Whether the verifying key was built in this process and passed its
    /// release pin.
    pub loaded: bool,
    /// The identity computed from the built verifying key.
    pub loaded_identity: Option<PrivateVerifierIdentity>,
    pub matches_pin: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateActivationReadiness {
    pub governance_kind: String,
    pub effective_height: Option<u64>,
    pub active_at_next_height: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateGovernedRoute {
    pub route_id: String,
    pub base_asset_id: String,
    pub base_code: Option<String>,
    pub quote_asset_id: String,
    pub quote_code: Option<String>,
    pub paused: bool,
}

/// The NAV base and settlement quote asset ids a governed route must carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateAssetPair {
    pub base_asset_id: String,
    pub quote_asset_id: String,
}

impl PrivateAssetPair {
    pub fn w1_a666_pfusdc() -> Self {
        Self {
            base_asset_id: PRIVATE_READINESS_A666_ASSET_ID.to_string(),
            quote_asset_id: PRIVATE_READINESS_PFUSDC_ASSET_ID.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateAdmissionSwitch {
    /// `on`, `off` or `unknown`.
    pub state: String,
    pub looked_for: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateReadinessReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    /// `POSTFIAT_BUILD_GIT_REV` and `POSTFIAT_BUILD_PROFILE`, compiled in by
    /// `crates/node/build.rs`, as `status` reports them.
    pub build_git_revision: String,
    pub build_profile: String,
    pub node_version: String,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub finalized_state_root: String,
    pub next_height: u64,
    pub pool_id: String,
    pub pool_present: bool,
    pub orchard_pool_paused: bool,
    pub swap_verifier: PrivateVerifierReadiness,
    pub private_egress_verifier: PrivateVerifierReadiness,
    pub atomic_shielded_batches: PrivateActivationReadiness,
    pub pricing_enforcement: PrivateActivationReadiness,
    pub pricing_max_nav_age_blocks: Option<u64>,
    pub governed_routes: Vec<PrivateGovernedRoute>,
    /// The pair the route is matched by.
    pub a666_pfusdc_pair: PrivateAssetPair,
    /// Every governed route with that pair, by route id: one is the route,
    /// none is `a666_pfusdc_route_absent`, more is `a666_pfusdc_route_ambiguous`.
    pub a666_pfusdc_route_matches: Vec<String>,
    pub a666_pfusdc_route: Option<PrivateGovernedRoute>,
    pub admission_switch: PrivateAdmissionSwitch,
    pub custody: PrivateReadinessCustody,
    pub ready: bool,
    pub reasons: Vec<String>,
}

/// Builds the live verifiers; tests substitute a probe.
pub(super) trait PrivateVerifierProbe {
    fn swap(&self) -> Result<PrivateVerifierIdentity, String>;
    fn private_egress(&self) -> Result<PrivateVerifierIdentity, String>;
}

/// Builds (or reuses) this process's cached verifying keys from the embedded
/// parameters and the compiled circuit, and checks their release pins, as
/// the validator's verifier prewarm does.
struct BuiltVerifiers;

impl PrivateVerifierProbe for BuiltVerifiers {
    fn swap(&self) -> Result<PrivateVerifierIdentity, String> {
        let key = postfiat_privacy_orchard::AssetOrchardSwapVerifyingKey::cached()
            .map_err(|error| error.to_string())?;
        let metadata = key.metadata();
        metadata
            .validate_release_pin()
            .map_err(|error| error.to_string())?;
        Ok(PrivateVerifierIdentity {
            proof_system_id: metadata.proof_system_id.to_string(),
            circuit_id: metadata.circuit_id.to_string(),
            k: metadata.k,
            params_hash: metadata.params_hash.clone(),
            vk_hash: metadata.vk_hash.clone(),
            runtime_pinned_vk_fingerprint: metadata.runtime_pinned_vk_fingerprint.clone(),
            public_instance_layout_hash: metadata.public_instance_layout_hash.clone(),
        })
    }

    fn private_egress(&self) -> Result<PrivateVerifierIdentity, String> {
        let key = postfiat_privacy_orchard::AssetOrchardPrivateEgressVerifyingKey::cached()
            .map_err(|error| error.to_string())?;
        let metadata = key.metadata();
        metadata
            .validate_release_pin()
            .map_err(|error| error.to_string())?;
        Ok(PrivateVerifierIdentity {
            proof_system_id: metadata.proof_system_id.to_string(),
            circuit_id: metadata.circuit_id.to_string(),
            k: metadata.k,
            params_hash: metadata.params_hash.clone(),
            vk_hash: metadata.vk_hash.clone(),
            runtime_pinned_vk_fingerprint: metadata.runtime_pinned_vk_fingerprint.clone(),
            public_instance_layout_hash: metadata.public_instance_layout_hash.clone(),
        })
    }
}

pub fn pinned_swap_verifier_identity() -> PrivateVerifierIdentity {
    PrivateVerifierIdentity {
        proof_system_id: ao::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
        circuit_id: ao::ASSET_ORCHARD_CIRCUIT_ID_V1.to_string(),
        k: aoc::ASSET_ORCHARD_SWAP_V1_K,
        params_hash: aoc::ASSET_ORCHARD_SWAP_V1_PARAMS_HASH.to_string(),
        vk_hash: aoc::ASSET_ORCHARD_SWAP_V1_VK_HASH.to_string(),
        runtime_pinned_vk_fingerprint: aoc::ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT
            .to_string(),
        public_instance_layout_hash: aoc::ASSET_ORCHARD_SWAP_V1_PUBLIC_INSTANCE_LAYOUT_HASH
            .to_string(),
    }
}

pub fn pinned_private_egress_verifier_identity() -> PrivateVerifierIdentity {
    PrivateVerifierIdentity {
        proof_system_id: ao::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
        circuit_id: ao::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1.to_string(),
        k: aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_K,
        params_hash: aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_PARAMS_HASH.to_string(),
        vk_hash: aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH.to_string(),
        runtime_pinned_vk_fingerprint:
            aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT.to_string(),
        public_instance_layout_hash:
            aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_PUBLIC_INSTANCE_LAYOUT_HASH.to_string(),
    }
}

fn verifier_readiness(
    pinned: PrivateVerifierIdentity,
    probed: Result<PrivateVerifierIdentity, String>,
) -> PrivateVerifierReadiness {
    match probed {
        Ok(identity) => PrivateVerifierReadiness {
            matches_pin: identity == pinned,
            pinned,
            loaded: true,
            loaded_identity: Some(identity),
            error: None,
        },
        Err(error) => PrivateVerifierReadiness {
            pinned,
            loaded: false,
            loaded_identity: None,
            matches_pin: false,
            error: Some(error),
        },
    }
}

fn activation(
    kind: &str,
    effective_height: Option<u64>,
    next_height: u64,
) -> PrivateActivationReadiness {
    PrivateActivationReadiness {
        governance_kind: kind.to_string(),
        effective_height,
        active_at_next_height: effective_height.is_some_and(|height| next_height >= height),
    }
}

fn governed_routes(ledger: &LedgerState) -> Vec<PrivateGovernedRoute> {
    let code_of = |asset_id: &str| {
        ledger
            .asset_definition(asset_id)
            .map(|asset| asset.code.clone())
    };
    let mut routes = ledger
        .pftl_uniswap_routes
        .iter()
        .map(|route| PrivateGovernedRoute {
            route_id: route.route_id.clone(),
            base_asset_id: route.native_nav_asset_id.clone(),
            base_code: code_of(&route.native_nav_asset_id),
            quote_asset_id: route.settlement_asset_id.clone(),
            quote_code: code_of(&route.settlement_asset_id),
            paused: route.paused,
        })
        .collect::<Vec<_>>();
    routes.sort_by(|left, right| left.route_id.cmp(&right.route_id));
    routes
}

/// The governed routes whose NAV base and settlement quote are exactly the
/// pair's asset ids, the predicate of the pricing rule; the route is the
/// single match.
fn a666_pfusdc_route(
    routes: &[PrivateGovernedRoute],
    pair: &PrivateAssetPair,
) -> (Option<PrivateGovernedRoute>, Vec<String>) {
    let matches = routes
        .iter()
        .filter(|route| {
            route.base_asset_id == pair.base_asset_id && route.quote_asset_id == pair.quote_asset_id
        })
        .collect::<Vec<_>>();
    let route = match matches.as_slice() {
        [route] => Some((*route).clone()),
        _ => None,
    };
    (
        route,
        matches.iter().map(|route| route.route_id.clone()).collect(),
    )
}

pub fn private_readiness(options: PrivateReadinessOptions) -> io::Result<PrivateReadinessReport> {
    private_readiness_with_probe(options, &BuiltVerifiers)
}

pub(super) fn private_readiness_with_probe(
    options: PrivateReadinessOptions,
    probe: &dyn PrivateVerifierProbe,
) -> io::Result<PrivateReadinessReport> {
    private_readiness_for_pair(options, probe, PrivateAssetPair::w1_a666_pfusdc())
}

pub(super) fn private_readiness_for_pair(
    options: PrivateReadinessOptions,
    probe: &dyn PrivateVerifierProbe,
    pair: PrivateAssetPair,
) -> io::Result<PrivateReadinessReport> {
    let pool_id = options
        .pool_id
        .clone()
        .unwrap_or_else(|| ao::ASSET_ORCHARD_POOL_ID_V1.to_string());
    let snapshot = read_finalized_pair_snapshot(&options.data_dir)?;
    let custody = readiness_custody(&options.data_dir, options.service.as_ref())?;
    Ok(readiness_report(
        &snapshot,
        pool_id,
        probe,
        options.admission_switch,
        pair,
        custody,
    ))
}

fn readiness_custody(
    data_dir: &Path,
    service: Option<&PrivateReadinessService>,
) -> io::Result<PrivateReadinessCustody> {
    use std::os::unix::fs::PermissionsExt;
    let store_backend = NodeStore::new(data_dir).storage_backend_mode()?;
    let key_file_owner_only = match service {
        None => None,
        Some(service) => match std::fs::metadata(&service.key_file) {
            Ok(metadata) => Some(metadata.is_file() && metadata.permissions().mode() & 0o077 == 0),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        },
    };
    Ok(PrivateReadinessCustody {
        published_by: if service.is_some() {
            "rpc_serve"
        } else {
            "local_read"
        }
        .to_string(),
        store_backend: store_backend.as_str().to_string(),
        validator_signer: service
            .map_or("none", |service| service.signer.as_str())
            .to_string(),
        key_file_owner_only,
    })
}

/// The typed refusals for a publisher's custody, in a fixed order.
pub(super) fn custody_reasons(custody: &PrivateReadinessCustody) -> Vec<String> {
    let served = custody.published_by == "rpc_serve";
    let unmet = [
        !served,
        custody.store_backend != postfiat_storage::StorageBackendMode::Transactional.as_str(),
        served && custody.validator_signer == PrivateValidatorSigner::PlaintextKeyFile.as_str(),
        served && custody.key_file_owner_only.is_none(),
        served && custody.key_file_owner_only == Some(false),
    ];
    PRIVATE_READINESS_CUSTODY_REASONS
        .into_iter()
        .zip(unmet)
        .filter(|(_, unmet)| *unmet)
        .map(|(reason, _)| reason.to_string())
        .collect()
}

fn readiness_report(
    snapshot: &FinalizedPairSnapshot,
    pool_id: String,
    probe: &dyn PrivateVerifierProbe,
    admission_switch: Option<bool>,
    pair: PrivateAssetPair,
    custody: PrivateReadinessCustody,
) -> PrivateReadinessReport {
    let next_height = snapshot.tip.height.saturating_add(1);
    let governance = &snapshot.governance;
    let swap_verifier = verifier_readiness(pinned_swap_verifier_identity(), probe.swap());
    let private_egress_verifier = verifier_readiness(
        pinned_private_egress_verifier_identity(),
        probe.private_egress(),
    );
    let atomic_shielded_batches = activation(
        GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT,
        governance.shielded_atomic_batch_activation_height(),
        next_height,
    );
    let pricing_enforcement = activation(
        GOVERNANCE_KIND_ASSET_ORCHARD_PRICING_ENFORCEMENT_ACTIVATION_HEIGHT,
        governance.asset_orchard_pricing_enforcement_activation_height(),
        next_height,
    );
    let mut routes = governed_routes(&snapshot.ledger);
    let (a666_pfusdc_route, a666_pfusdc_route_matches) = a666_pfusdc_route(&routes, &pair);
    routes.truncate(PRIVATE_READINESS_MAX_ROUTES);
    let pool_present = snapshot
        .shielded
        .orchard
        .as_ref()
        .is_some_and(|pool| pool.pool_id == pool_id);
    let admission_switch = PrivateAdmissionSwitch {
        state: match admission_switch {
            Some(true) => "on",
            Some(false) => "off",
            None => "unknown",
        }
        .to_string(),
        looked_for: PRIVATE_ACTION_ADMISSION_SWITCH_KEYS
            .iter()
            .map(|key| key.to_string())
            .collect(),
    };

    let mut reasons = custody_reasons(&custody);
    for (label, verifier) in [
        ("swap", &swap_verifier),
        ("private_egress", &private_egress_verifier),
    ] {
        if !verifier.loaded {
            reasons.push(format!("{label}_verifier_not_loaded"));
        } else if !verifier.matches_pin {
            reasons.push(format!("{label}_verifier_identity_mismatch"));
        }
    }
    if pool_id != ao::ASSET_ORCHARD_POOL_ID_V1 {
        reasons.push("pool_not_asset_orchard".to_string());
    }
    if governance.orchard_pool_paused {
        reasons.push("orchard_pool_paused".to_string());
    }
    for (label, state) in [
        ("shielded_atomic_batch", &atomic_shielded_batches),
        ("asset_orchard_pricing_enforcement", &pricing_enforcement),
    ] {
        match state.effective_height {
            None => reasons.push(format!("{label}_activation_absent")),
            Some(_) if !state.active_at_next_height => {
                reasons.push(format!("{label}_activation_pending"))
            }
            Some(_) => {}
        }
    }
    match (&a666_pfusdc_route, a666_pfusdc_route_matches.len()) {
        (Some(route), _) if route.paused => reasons.push("a666_pfusdc_route_paused".to_string()),
        (Some(_), _) => {}
        (None, 0) => reasons.push("a666_pfusdc_route_absent".to_string()),
        (None, _) => reasons.push("a666_pfusdc_route_ambiguous".to_string()),
    }

    PrivateReadinessReport {
        schema: PRIVATE_READINESS_SCHEMA.to_string(),
        chain_id: snapshot.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&snapshot.genesis),
        protocol_version: snapshot.genesis.protocol_version,
        build_git_revision: option_env!("POSTFIAT_BUILD_GIT_REV")
            .unwrap_or("unknown")
            .to_string(),
        build_profile: option_env!("POSTFIAT_BUILD_PROFILE")
            .unwrap_or("unknown")
            .to_string(),
        node_version: env!("CARGO_PKG_VERSION").to_string(),
        finalized_height: snapshot.tip.height,
        finalized_block_hash: snapshot.tip.block_hash.clone(),
        finalized_state_root: snapshot.tip.state_root.clone(),
        next_height,
        pool_id,
        pool_present,
        orchard_pool_paused: governance.orchard_pool_paused,
        swap_verifier,
        private_egress_verifier,
        atomic_shielded_batches,
        pricing_enforcement,
        pricing_max_nav_age_blocks: governance.asset_orchard_pricing_max_nav_age_blocks(),
        governed_routes: routes,
        a666_pfusdc_pair: pair,
        a666_pfusdc_route_matches,
        a666_pfusdc_route,
        admission_switch,
        custody,
        ready: reasons.is_empty(),
        reasons,
    }
}
