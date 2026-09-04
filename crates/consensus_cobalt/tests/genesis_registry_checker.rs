// Work-sequence step 3 of docs/architecture/genesis-registry-proposal-path.md:
// run the pinned Cobalt checker against fixture G0/T0 pairs across
// n_S = 12-20, including every rejection case. SHADOW_ONLY rehearsal data:
// nothing here grants authority, mutates a registry, or contacts any network.
//
// The checker consumes the canonical types from
// crates/types/src/genesis_registry.rs directly (postfiat-types is already a
// dependency of this crate), so no production adapter was needed. The
// test-side harness projects an accepted ProposedGenesisRegistryV1 into the
// live Cobalt objects and runs the same validate_essential_subset /
// validate_trust_view / validate_trust_graph / analyze_trust_graph path the
// controlled devnet uses (design §5.2 item 3).
//
// Pinned checker stages (design §5.2, without the step-4 source-admission
// stage, which fetches round artifacts and lands as dynamic_unl_source.rs):
//   1. canonical decode + schema validation (the named-error contract shared
//      with the mutation fixtures and the Python reference implementation);
//   2. announced-hash binding: digest("L1V2_PROPOSED_GENESIS_REGISTRY_V1", .)
//      must equal the announced hash (design §4.3);
//   3. launch-profile size bounds: the n_S >= 12 floor and the fork
//      selector's maximum (design §5.2 item 4);
//   4. template recomputation: T0 is computed from n_S, never chosen
//      (design §4.4);
//   5. Cobalt trust-graph soundness on the G0/T0 projection: the
//      inequalities 2*t_S < q_S and t_S < 2*q_S - n_S, linkage, and the
//      uniform trust-view support rules.

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_trust_graph, build_trust_view,
    validate_essential_subset, validate_trust_graph, CobaltDomain, CobaltFaultModel, LinkageReport,
    TrustGraph,
};
use postfiat_crypto_provider::hash_hex;
use postfiat_types::{template_trust_graph_for, ProposedGenesisEntryV1, ProposedGenesisRegistryV1};
use std::path::PathBuf;

/// Launch-profile registry floor (design §5.2 item 4 and §7).
const LAUNCH_PROFILE_MIN_ENTRIES: usize = 12;
/// The fork selector's maximum size, as frozen in the archived rounds'
/// execution manifests (design §2.2; every archived round selects 20).
const FORK_SELECTOR_MAX_ENTRIES: usize = 20;
const CHECKER_PROTOCOL_VERSION: u32 = 1;
const GOLDEN_ROUNDS: [u64; 8] = [12, 13, 14, 15, 16, 17, 18, 19];
const MUTATION_COUNT: usize = 25;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/genesis-registry/fixtures")
}

fn load_json(path: &PathBuf) -> serde_json::Value {
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_slice(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn unhex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "odd hex length");
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn golden_fixture(round: u64) -> (Vec<u8>, String) {
    let fixture = load_json(&fixtures_root().join(format!("golden/testnet-r{round}.json")));
    let bytes = unhex(fixture["canonical_cbor_hex"].as_str().expect("cbor hex"));
    let announced = fixture["proposed_registry_hash_hex"]
        .as_str()
        .expect("hash hex")
        .to_string();
    (bytes, announced)
}

/// The Cobalt domain the checker pins for a proposed registry. The l1v2 chain
/// does not exist yet at proposal time, so the domain is derived from the
/// proposed chain id alone.
fn checker_domain(registry: &ProposedGenesisRegistryV1) -> CobaltDomain {
    CobaltDomain {
        chain_id: registry.chain_id.clone(),
        genesis_hash: hash_hex(
            "postfiat.genesis.checker_domain.v1",
            registry.chain_id.as_bytes(),
        ),
        protocol_version: CHECKER_PROTOCOL_VERSION,
    }
}

/// G0 member node ids: lowercase hex of the 33-byte fork master key. Hex
/// encoding preserves bytewise order, so entry order equals node-id order.
fn member_node_ids(registry: &ProposedGenesisRegistryV1) -> Vec<String> {
    registry
        .entries
        .iter()
        .map(|entry| hex(&entry.fork_master_key))
        .collect()
}

/// Binds `proposed_registry_hash` into the 96-hex Cobalt registry root, so
/// registry and trust graph cannot be ratified separately (design §4.3).
fn cobalt_registry_root(registry: &ProposedGenesisRegistryV1) -> String {
    let hash = registry.proposed_registry_hash().expect("registry hash");
    hash_hex("postfiat.genesis.proposed_registry_root.v1", &hash)
}

/// Projects the G0/T0 pair into the pinned Cobalt trust graph: one uniform
/// essential subset S = G0 with quorum q_S and max_active_byzantine t_S, one
/// trust view per member, no predecessor root (genesis has none).
fn build_cobalt_pair(registry: &ProposedGenesisRegistryV1) -> Result<TrustGraph, String> {
    let domain = checker_domain(registry);
    let validators = member_node_ids(registry);
    let template = &registry.template_trust_graph;
    let subset = build_essential_subset(
        &domain,
        validators.clone(),
        template.t_s as usize,
        template.q_s as usize,
        Vec::new(),
        1,
        None,
    )?;
    let mut views = Vec::with_capacity(validators.len());
    for validator in &validators {
        views.push(build_trust_view(
            &domain,
            validator,
            1,
            vec![subset.clone()],
            "",
        )?);
    }
    let graph = build_trust_graph(&domain, 1, cobalt_registry_root(registry), 1, None, views)?;
    validate_trust_graph(&domain, &graph)?;
    Ok(graph)
}

/// The pinned checker over canonical bytes. Schema-stage failures surface the
/// named-error code; later stages surface their own named reason or the
/// Cobalt validation message.
fn run_pinned_checker(
    canonical_bytes: &[u8],
    announced_hash_hex: Option<&str>,
) -> Result<(ProposedGenesisRegistryV1, TrustGraph), String> {
    let registry = ProposedGenesisRegistryV1::decode_canonical(canonical_bytes)
        .map_err(|error| error.code().to_string())?;
    let recomputed = hex(&registry
        .proposed_registry_hash()
        .map_err(|error| error.code().to_string())?);
    if let Some(announced) = announced_hash_hex {
        if announced != recomputed {
            return Err("hash_mismatch".to_string());
        }
    }
    let member_count = registry.entries.len();
    if !(LAUNCH_PROFILE_MIN_ENTRIES..=FORK_SELECTOR_MAX_ENTRIES).contains(&member_count) {
        return Err("launch_profile_size".to_string());
    }
    let expected_template =
        template_trust_graph_for(member_count as u64).map_err(|error| error.code().to_string())?;
    if expected_template != registry.template_trust_graph {
        return Err("trust_graph_mismatch".to_string());
    }
    let graph = build_cobalt_pair(&registry)?;
    Ok((registry, graph))
}

fn analyze(
    registry: &ProposedGenesisRegistryV1,
    graph: &TrustGraph,
    actively_byzantine: Vec<String>,
) -> LinkageReport {
    analyze_trust_graph(
        &checker_domain(registry),
        graph,
        &CobaltFaultModel { actively_byzantine },
    )
    .expect("linkage analysis")
}

/// Asserts the uniform G0/T0 linkage profile under zero faults. Safety
/// linkage must hold for every pair; liveness (full linkage) holds exactly
/// when t_S <= n_S - q_S, which the template guarantees only at some sizes
/// (for example n_S = 15 and n_S = 20, not n_S = 18).
fn assert_zero_fault_linkage(registry: &ProposedGenesisRegistryV1, graph: &TrustGraph) {
    let n = registry.entries.len();
    let pairs = n * (n - 1) / 2;
    let template = &registry.template_trust_graph;
    let live = template.t_s + template.q_s <= template.n_s;
    let report = analyze(registry, graph, Vec::new());
    assert_eq!(report.trust_view_count, n);
    assert_eq!(report.linked_pairs.len(), pairs, "safety linkage n_S={n}");
    if live {
        assert_eq!(report.fully_linked_pairs.len(), pairs);
        assert!(report.unsafe_pairs.is_empty());
        assert_eq!(report.strongly_connected_validators.len(), n);
        assert_eq!(report.weakly_connected_validators.len(), n);
    } else {
        assert!(report.fully_linked_pairs.is_empty());
        assert_eq!(report.unsafe_pairs.len(), pairs);
        for pair in &report.unsafe_pairs {
            assert_eq!(pair.reason, "linked but not fully linked for liveness");
        }
    }
}

/// Top-n sub-registry of a golden registry: the selector's ranked order
/// (selection_index ascending) picks members, entries re-sort by master key,
/// and T0 is recomputed from n — the same deterministic template rule the
/// design pins (§4.2, §4.4).
fn derived_registry(base: &ProposedGenesisRegistryV1, n: usize) -> ProposedGenesisRegistryV1 {
    let mut ranked: Vec<ProposedGenesisEntryV1> = base.entries.clone();
    ranked.sort_by_key(|entry| entry.selection_index);
    ranked.truncate(n);
    ranked.sort_by_key(|entry| entry.fork_master_key);
    let mut derived = base.clone();
    derived.entries = ranked;
    derived.template_trust_graph = template_trust_graph_for(n as u64).expect("template");
    derived
}

fn round12_registry() -> ProposedGenesisRegistryV1 {
    let (bytes, _) = golden_fixture(12);
    ProposedGenesisRegistryV1::decode_canonical(&bytes).expect("golden round 12")
}

#[test]
fn golden_g0_t0_pairs_accepted_by_pinned_cobalt_checker() {
    for round in GOLDEN_ROUNDS {
        let (bytes, announced) = golden_fixture(round);
        let (registry, graph) = run_pinned_checker(&bytes, Some(&announced))
            .unwrap_or_else(|reason| panic!("round {round} rejected: {reason}"));
        // Rounds 12-18 receipt every selected operator (n_S = 20); round 19
        // omits two receipts, exercising Selected ∩ Receipted (n_S = 18).
        let expected_members = if round == 19 { 18 } else { 20 };
        assert_eq!(registry.entries.len(), expected_members, "round {round}");
        assert_eq!(
            registry.template_trust_graph.n_s, expected_members as u64,
            "round {round}"
        );
        assert_eq!(graph.trust_views.len(), expected_members, "round {round}");
        assert_eq!(graph.registry_root, cobalt_registry_root(&registry));
        assert_zero_fault_linkage(&registry, &graph);
    }
}

#[test]
fn derived_g0_t0_pairs_accepted_across_n_s_12_to_20() {
    let base = round12_registry();
    for n in LAUNCH_PROFILE_MIN_ENTRIES..=FORK_SELECTOR_MAX_ENTRIES {
        let derived = derived_registry(&base, n);
        let bytes = derived.canonical_bytes().expect("canonical bytes");
        if n == base.entries.len() {
            assert_eq!(bytes, base.canonical_bytes().expect("base bytes"));
        }
        let announced = hex(&derived.proposed_registry_hash().expect("hash"));
        let (registry, graph) = run_pinned_checker(&bytes, Some(&announced))
            .unwrap_or_else(|reason| panic!("n_S={n} rejected: {reason}"));
        assert_eq!(registry.template_trust_graph.n_s, n as u64);
        assert_zero_fault_linkage(&registry, &graph);
    }
}

#[test]
fn t_s_is_the_exact_safety_linkage_bound_across_n_s_12_to_20() {
    let base = round12_registry();
    for n in LAUNCH_PROFILE_MIN_ENTRIES..=FORK_SELECTOR_MAX_ENTRIES {
        let derived = derived_registry(&base, n);
        let graph = build_cobalt_pair(&derived).expect("cobalt pair");
        let t_s = derived.template_trust_graph.t_s as usize;
        let members = member_node_ids(&derived);
        let pairs = n * (n - 1) / 2;
        // The uniform graph is symmetric, so one worst-case fault set of each
        // size covers all of them: t_S faults keep every pair safety-linked,
        // t_S + 1 faults break every pair.
        let at_bound = analyze(&derived, &graph, members[..t_s].to_vec());
        assert_eq!(at_bound.linked_pairs.len(), pairs, "n_S={n} at t_S");
        let over_bound = analyze(&derived, &graph, members[..t_s + 1].to_vec());
        assert!(over_bound.linked_pairs.is_empty(), "n_S={n} past t_S");
        assert_eq!(over_bound.unsafe_pairs.len(), pairs, "n_S={n} past t_S");
    }
}

#[test]
fn every_mutation_fixture_rejected_with_named_reason() {
    let dir = fixtures_root().join("mutations/testnet-r12");
    let mut names = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let fixture = load_json(&path);
        let name = fixture["name"].as_str().expect("name").to_string();
        let expected = fixture["expected_error"].as_str().expect("expected_error");
        let bytes = unhex(fixture["cbor_hex"].as_str().expect("cbor hex"));
        let rejection = run_pinned_checker(&bytes, None)
            .map(|_| ())
            .expect_err(&format!("mutation {name} must be rejected"));
        assert_eq!(rejection, expected, "mutation {name}");
        names.push(name);
    }
    assert_eq!(names.len(), MUTATION_COUNT, "all 25 mutations exercised");
}

#[test]
fn announced_hash_mismatch_rejected() {
    let (bytes, announced) = golden_fixture(12);
    let mut tampered = announced.clone();
    let last = if tampered.ends_with('0') { "1" } else { "0" };
    tampered.replace_range(tampered.len() - 1.., last);
    assert_eq!(
        run_pinned_checker(&bytes, Some(&tampered)).map(|_| ()),
        Err("hash_mismatch".to_string())
    );
    assert!(run_pinned_checker(&bytes, Some(&announced)).is_ok());
}

#[test]
fn registry_size_outside_launch_profile_rejected() {
    let base = round12_registry();
    // Below the n_S >= 12 floor.
    let small = derived_registry(&base, LAUNCH_PROFILE_MIN_ENTRIES - 1);
    let bytes = small.canonical_bytes().expect("canonical bytes");
    assert_eq!(
        run_pinned_checker(&bytes, None).map(|_| ()),
        Err("launch_profile_size".to_string())
    );
    // Above the fork selector's maximum: append a synthetic member sorting
    // after every archived 0xed-prefixed master key.
    let mut big = base.clone();
    let mut extra = base.entries.last().expect("entries").clone();
    extra.fork_master_key = [0xff; 33];
    extra.fork_master_key[0] = 0xed;
    extra.selection_index = base.entries.len() as u64;
    big.entries.push(extra);
    big.template_trust_graph =
        template_trust_graph_for(big.entries.len() as u64).expect("template");
    let bytes = big.canonical_bytes().expect("canonical bytes");
    assert_eq!(
        run_pinned_checker(&bytes, None).map(|_| ()),
        Err("launch_profile_size".to_string())
    );
}

#[test]
fn tampered_template_thresholds_rejected_across_n_s_12_to_20() {
    let base = round12_registry();
    for n in LAUNCH_PROFILE_MIN_ENTRIES..=FORK_SELECTOR_MAX_ENTRIES {
        let derived = derived_registry(&base, n);
        let domain = checker_domain(&derived);
        let validators = member_node_ids(&derived);
        let template = derived.template_trust_graph;
        // t_S tampered up to ceil(q_S / 2), so 2*t_S < q_S fails while
        // t_S < 2*q_S - n_S still holds (that inequality is checked first).
        let quorum_violation = build_essential_subset(
            &domain,
            validators.clone(),
            template.q_s.div_ceil(2) as usize,
            template.q_s as usize,
            Vec::new(),
            1,
            None,
        )
        .expect_err("quorum violation must be rejected");
        assert!(
            quorum_violation.contains("2t_S < q_S"),
            "n_S={n}: {quorum_violation}"
        );
        // t_S tampered up until t_S < 2*q_S - n_S fails.
        let byzantine_violation = build_essential_subset(
            &domain,
            validators.clone(),
            (2 * template.q_s - template.n_s) as usize,
            template.q_s as usize,
            Vec::new(),
            1,
            None,
        )
        .expect_err("t_S violation must be rejected");
        assert!(
            byzantine_violation.contains("t_S < 2q_S - n_S"),
            "n_S={n}: {byzantine_violation}"
        );
        // Quorum above the member count.
        let oversize_quorum = build_essential_subset(
            &domain,
            validators,
            template.t_s as usize,
            n + 1,
            Vec::new(),
            1,
            None,
        )
        .expect_err("oversized quorum must be rejected");
        assert!(
            oversize_quorum.contains("quorum exceeds"),
            "n_S={n}: {oversize_quorum}"
        );
        // A schema-level threshold tamper (q_S off by one) is caught by the
        // template-recomputation stage with the named reason.
        let mut tampered = derived.clone();
        tampered.template_trust_graph.q_s -= 1;
        assert_eq!(
            tampered.canonical_bytes().map(|_| ()).map_err(|e| e.code()),
            Err("trust_graph_mismatch"),
            "n_S={n}"
        );
    }
}

#[test]
fn wrong_ordering_rejected_by_cobalt_validation() {
    let registry = round12_registry();
    let domain = checker_domain(&registry);
    let mut graph = build_cobalt_pair(&registry).expect("cobalt pair");
    // Trust views out of validator order.
    graph.trust_views.swap(0, 1);
    let rejection = validate_trust_graph(&domain, &graph).expect_err("unsorted views");
    assert!(
        rejection.contains("must be sorted by unique validator"),
        "{rejection}"
    );
    // Essential-subset members out of order.
    let graph = build_cobalt_pair(&registry).expect("cobalt pair");
    let mut subset = graph.trust_views[0].essential_subsets[0].clone();
    subset.validators.swap(0, 1);
    let rejection = validate_essential_subset(&domain, &subset).expect_err("unsorted subset");
    assert!(rejection.contains("must be sorted unique"), "{rejection}");
}

#[test]
fn registry_root_swap_rejected_by_trust_graph_root_binding() {
    // Registry and trust graph are one hashed object (design §4.3): swapping
    // in another round's registry root breaks the trust-graph root binding.
    let r12 = round12_registry();
    let (r13_bytes, _) = golden_fixture(13);
    let r13 = ProposedGenesisRegistryV1::decode_canonical(&r13_bytes).expect("golden round 13");
    let domain = checker_domain(&r12);
    let mut graph = build_cobalt_pair(&r12).expect("cobalt pair");
    graph.registry_root = cobalt_registry_root(&r13);
    let rejection = validate_trust_graph(&domain, &graph).expect_err("swapped registry root");
    assert!(
        rejection.contains("trust graph root mismatch"),
        "{rejection}"
    );
}
