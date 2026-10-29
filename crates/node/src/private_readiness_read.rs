//! Readiness of this node for user-proven private actions, attested from one
//! committed chain-tip state and the running binary: the pinned verifier
//! identities and whether each verifier is actually built in this process,
//! the governed activations, and the governed A666/pfUSDC swap route.
//! Configuration alone can never make `ready` true.

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
const PRIVATE_READINESS_MAX_ROUTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivateReadinessOptions {
    pub data_dir: PathBuf,
    pub pool_id: Option<String>,
    /// The serving process's own `--allow-private-action-admission` setting;
    /// `None` (a local read with no server) reports `unknown`. Never taken
    /// from request parameters.
    pub admission_switch: Option<bool>,
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
    pub a666_pfusdc_route: Option<PrivateGovernedRoute>,
    pub admission_switch: PrivateAdmissionSwitch,
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

/// The exactly-one governed route whose base is an A666 NAV asset and whose
/// quote is a pfUSDC asset; codes only select it, the ids are what it binds.
fn a666_pfusdc_route(routes: &[PrivateGovernedRoute]) -> Option<PrivateGovernedRoute> {
    let matches = routes
        .iter()
        .filter(|route| {
            route
                .base_code
                .as_deref()
                .is_some_and(|code| code.eq_ignore_ascii_case("A666"))
                && route
                    .quote_code
                    .as_deref()
                    .is_some_and(|code| code.eq_ignore_ascii_case("PFUSDC"))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [route] => Some((*route).clone()),
        _ => None,
    }
}

pub fn private_readiness(options: PrivateReadinessOptions) -> io::Result<PrivateReadinessReport> {
    private_readiness_with_probe(options, &BuiltVerifiers)
}

pub(super) fn private_readiness_with_probe(
    options: PrivateReadinessOptions,
    probe: &dyn PrivateVerifierProbe,
) -> io::Result<PrivateReadinessReport> {
    let pool_id = options
        .pool_id
        .clone()
        .unwrap_or_else(|| ao::ASSET_ORCHARD_POOL_ID_V1.to_string());
    let snapshot = read_finalized_pair_snapshot(&options.data_dir)?;
    Ok(readiness_report(
        &snapshot,
        pool_id,
        probe,
        options.admission_switch,
    ))
}

fn readiness_report(
    snapshot: &FinalizedPairSnapshot,
    pool_id: String,
    probe: &dyn PrivateVerifierProbe,
    admission_switch: Option<bool>,
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
    let a666_pfusdc_route = a666_pfusdc_route(&routes);
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

    let mut reasons = Vec::new();
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
    match &a666_pfusdc_route {
        None => reasons.push("a666_pfusdc_route_absent".to_string()),
        Some(route) if route.paused => reasons.push("a666_pfusdc_route_paused".to_string()),
        Some(_) => {}
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
        a666_pfusdc_route,
        admission_switch,
        ready: reasons.is_empty(),
        reasons,
    }
}
