use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_canonical_unl_trust_graph, build_essential_subset, build_trust_view,
    build_trust_view_update_transition, CobaltDomain, CobaltFaultModel, EssentialSubset,
};
use serde_json::json;

fn domain() -> CobaltDomain {
    CobaltDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        protocol_version: 1,
    }
}

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn validators(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("validator-{index}"))
        .collect()
}

fn subset(domain: &CobaltDomain, members: &[&str], t_s: usize, q_s: usize) -> EssentialSubset {
    build_essential_subset(
        domain,
        members
            .iter()
            .map(|validator| (*validator).to_string())
            .collect(),
        t_s,
        q_s,
        Vec::new(),
        1,
        None,
    )
    .expect("build essential subset")
}

fn main() {
    let domain = domain();
    let g0 = build_canonical_unl_trust_graph(&domain, 1, root('a'), 1, None, validators(7), 5)
        .expect("canonical G0");
    let validator_1_g0 = g0
        .trust_views
        .iter()
        .find(|view| view.validator == "validator-1")
        .expect("validator-1 G0");
    let mut g1_subsets = validator_1_g0.essential_subsets.clone();
    g1_subsets.push(subset(
        &domain,
        &[
            "validator-0",
            "validator-1",
            "validator-2",
            "validator-3",
            "validator-4",
        ],
        1,
        4,
    ));
    let validator_1_g1 =
        build_trust_view(&domain, "validator-1", 2, g1_subsets, "").expect("validator-1 G1");
    let (g1, record) = build_trust_view_update_transition(
        &domain,
        &g0,
        validator_1_g1,
        30,
        &CobaltFaultModel::default(),
    )
    .expect("G1 lifecycle transition");
    let linkage =
        analyze_trust_graph(&domain, &g1, &CobaltFaultModel::default()).expect("G1 linkage");

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema": "postfiat-cobalt-current-trust-graph-root-v1",
            "cobalt_mode": "non_uniform",
            "active_graph": "G1",
            "g0_trust_graph_root": g0.trust_graph_root,
            "g1_trust_graph_root": g1.trust_graph_root,
            "trust_graph_root": g1.trust_graph_root,
            "g1_activation_height": record.activation_height,
            "g1_linkedness_report_hash": linkage.report_hash,
            "g1_trust_view_count": g1.trust_views.len(),
            "g1_non_identical_trust_views": true
        }))
        .expect("serialize current trust graph root")
    );
}
