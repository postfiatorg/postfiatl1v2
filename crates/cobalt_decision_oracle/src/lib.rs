//! Independent decision oracle for the Cobalt activate-or-retire benchmark.
//!
//! This crate intentionally has no dependency on `postfiat-consensus-cobalt`,
//! `postfiat-node`, or any production PostFiat protocol crate. It implements the
//! frozen benchmark contract from explicit essential-subset inputs.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const INPUT_SCHEMA: &str = "postfiat-cobalt-decisive-input-v1";
pub const MANIFEST_SCHEMA: &str = "postfiat-cobalt-decisive-manifest-v1";
pub const ORACLE_RULES_VERSION: &str = "cobalt-essential-subset-oracle-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleInput {
    pub schema: String,
    pub source_pins: BTreeMap<String, String>,
    pub cases: Vec<ScenarioInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioInput {
    pub id: String,
    pub fault_class: String,
    pub validators: Vec<String>,
    pub correct_nodes: Vec<String>,
    #[serde(default)]
    pub unavailable: Vec<String>,
    #[serde(default)]
    pub actively_byzantine: Vec<String>,
    pub trust_views: BTreeMap<String, TrustViewInput>,
    pub local_unls: BTreeMap<String, Vec<String>>,
    pub local_quorums: BTreeMap<String, usize>,
    pub proposals: Vec<ProposalInput>,
    pub event_schedule: EventSchedule,
    pub transition: TransitionInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustViewInput {
    pub essential_subsets: Vec<EssentialSubsetInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EssentialSubsetInput {
    pub validators: Vec<String>,
    pub quorum: usize,
    pub max_active_byzantine: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalInput {
    pub registry_root: String,
    pub supporters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSchedule {
    #[serde(default)]
    pub delayed: bool,
    #[serde(default)]
    pub duplicated: bool,
    #[serde(default)]
    pub reordered: bool,
    #[serde(default)]
    pub stale_replay: bool,
    #[serde(default)]
    pub recover_unavailable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionInput {
    pub kind: String,
    #[serde(default)]
    pub removed: Vec<String>,
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub rotated: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisiveManifest {
    pub schema: String,
    pub oracle: OracleIdentity,
    pub input_sha256: String,
    pub source_pins: BTreeMap<String, String>,
    pub adapter_sha256: BTreeMap<String, String>,
    pub cases: Vec<ScenarioDecision>,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleIdentity {
    pub rules_version: String,
    pub implementation_boundary: String,
    pub source_sha256: String,
    pub contract_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioDecision {
    #[serde(flatten)]
    pub input: ScenarioInput,
    pub expected: ExpectedDecision,
    pub oracle_trace: OracleTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedDecision {
    pub classification: String,
    pub cobalt_nodes: BTreeMap<String, NodeDecision>,
    pub rippled_nodes: BTreeMap<String, NodeDecision>,
    pub cobalt_conflicting_roots: usize,
    pub rippled_conflicting_roots: usize,
    pub material_safety_delta: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDecision {
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_root: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleTrace {
    pub responsive_correct_nodes: Vec<String>,
    pub fully_linked_pairs: Vec<NodePair>,
    pub unlinked_pairs: Vec<NodePair>,
    pub strong_support: BTreeMap<String, BTreeMap<String, bool>>,
    pub strongly_connected: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodePair {
    pub left: String,
    pub right: String,
}

pub fn build_manifest(
    input: OracleInput,
    input_bytes: &[u8],
    source_sha256: String,
    contract_sha256: String,
    adapter_sha256: BTreeMap<String, String>,
) -> Result<DecisiveManifest, String> {
    if input.schema != INPUT_SCHEMA {
        return Err(format!("unsupported oracle input schema {}", input.schema));
    }
    if input.cases.is_empty() {
        return Err("oracle input requires at least one scenario".to_string());
    }
    let mut ids = BTreeSet::new();
    let mut cases = Vec::with_capacity(input.cases.len());
    for scenario in input.cases {
        if !ids.insert(scenario.id.clone()) {
            return Err(format!("duplicate scenario id {}", scenario.id));
        }
        cases.push(evaluate_scenario(scenario)?);
    }
    let mut manifest = DecisiveManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        oracle: OracleIdentity {
            rules_version: ORACLE_RULES_VERSION.to_string(),
            implementation_boundary:
                "standalone crate; no production PostFiat protocol dependencies".to_string(),
            source_sha256,
            contract_sha256,
        },
        input_sha256: sha256_hex(input_bytes),
        source_pins: input.source_pins,
        adapter_sha256,
        cases,
        manifest_sha256: String::new(),
    };
    manifest.manifest_sha256 = canonical_manifest_hash(&manifest)?;
    Ok(manifest)
}

pub fn evaluate_scenario(input: ScenarioInput) -> Result<ScenarioDecision, String> {
    validate_scenario(&input)?;
    let active_faults: BTreeSet<&str> = input
        .actively_byzantine
        .iter()
        .map(String::as_str)
        .collect();
    let unavailable: BTreeSet<&str> = if input.event_schedule.recover_unavailable {
        BTreeSet::new()
    } else {
        input.unavailable.iter().map(String::as_str).collect()
    };
    let correct: BTreeSet<&str> = input.correct_nodes.iter().map(String::as_str).collect();
    let responsive_correct: Vec<String> = input
        .correct_nodes
        .iter()
        .filter(|node| !unavailable.contains(node.as_str()))
        .cloned()
        .collect();

    let mut fully_linked_pairs = Vec::new();
    let mut unlinked_pairs = Vec::new();
    for left_index in 0..responsive_correct.len() {
        for right_index in (left_index + 1)..responsive_correct.len() {
            let pair = NodePair {
                left: responsive_correct[left_index].clone(),
                right: responsive_correct[right_index].clone(),
            };
            if fully_linked(
                &input,
                &pair.left,
                &pair.right,
                &active_faults,
                &unavailable,
            )? {
                fully_linked_pairs.push(pair);
            } else {
                unlinked_pairs.push(pair);
            }
        }
    }

    let proposal_support: BTreeMap<&str, BTreeSet<&str>> = input
        .proposals
        .iter()
        .map(|proposal| {
            (
                proposal.registry_root.as_str(),
                proposal.supporters.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    let mut strong_support = BTreeMap::new();
    let mut strongly_connected = BTreeMap::new();
    for node in &responsive_correct {
        let mut roots = BTreeMap::new();
        for proposal in &input.proposals {
            roots.insert(
                proposal.registry_root.clone(),
                sees_strong_support(
                    &input,
                    node,
                    &proposal_support[proposal.registry_root.as_str()],
                )?,
            );
        }
        strong_support.insert(node.clone(), roots);
        strongly_connected.insert(
            node.clone(),
            closure_strongly_connected(&input, node, &active_faults, &unavailable, &correct)?,
        );
    }

    let common_root = unique_common_strong_root(&input, &responsive_correct, &strong_support);
    let compatible = !responsive_correct.is_empty()
        && common_root.is_some()
        && strongly_connected.values().all(|connected| *connected)
        && unlinked_pairs.is_empty();

    let mut cobalt_nodes = BTreeMap::new();
    for node in &input.correct_nodes {
        let decision =
            if unavailable.contains(node.as_str()) && !input.event_schedule.recover_unavailable {
                NodeDecision {
                    outcome: "unavailable".to_string(),
                    registry_root: None,
                    reason: "node remains unavailable at the observation boundary".to_string(),
                }
            } else if compatible {
                NodeDecision {
                    outcome: "decide".to_string(),
                    registry_root: common_root.clone(),
                    reason: "strongly connected closure and one common strongly supported root"
                        .to_string(),
                }
            } else {
                NodeDecision {
                    outcome: "halt".to_string(),
                    registry_root: None,
                    reason: incompatibility_reason(
                        common_root.as_deref(),
                        &unlinked_pairs,
                        &strongly_connected,
                    ),
                }
            };
        cobalt_nodes.insert(node.clone(), decision);
    }

    let mut rippled_nodes = BTreeMap::new();
    for node in &input.correct_nodes {
        let decision =
            if unavailable.contains(node.as_str()) && !input.event_schedule.recover_unavailable {
                NodeDecision {
                    outcome: "unavailable".to_string(),
                    registry_root: None,
                    reason: "node remains unavailable at the observation boundary".to_string(),
                }
            } else {
                rippled_local_decision(&input, node, &proposal_support)?
            };
        rippled_nodes.insert(node.clone(), decision);
    }

    let cobalt_conflicting_roots = conflicting_roots(&cobalt_nodes);
    let rippled_conflicting_roots = conflicting_roots(&rippled_nodes);
    let expected = ExpectedDecision {
        classification: if compatible {
            "compatible"
        } else {
            "incompatible"
        }
        .to_string(),
        cobalt_nodes,
        rippled_nodes,
        cobalt_conflicting_roots,
        rippled_conflicting_roots,
        material_safety_delta: cobalt_conflicting_roots == 0 && rippled_conflicting_roots > 0,
    };
    Ok(ScenarioDecision {
        input,
        expected,
        oracle_trace: OracleTrace {
            responsive_correct_nodes: responsive_correct,
            fully_linked_pairs,
            unlinked_pairs,
            strong_support,
            strongly_connected,
        },
    })
}

fn validate_scenario(input: &ScenarioInput) -> Result<(), String> {
    validate_sorted_unique("validators", &input.validators)?;
    validate_sorted_unique("correct_nodes", &input.correct_nodes)?;
    validate_sorted_unique("unavailable", &input.unavailable)?;
    validate_sorted_unique("actively_byzantine", &input.actively_byzantine)?;
    if input.validators.is_empty() || input.correct_nodes.is_empty() {
        return Err(format!(
            "{} requires validators and correct nodes",
            input.id
        ));
    }
    let validators: BTreeSet<&str> = input.validators.iter().map(String::as_str).collect();
    for (label, nodes) in [
        ("correct_nodes", &input.correct_nodes),
        ("unavailable", &input.unavailable),
        ("actively_byzantine", &input.actively_byzantine),
    ] {
        if nodes.iter().any(|node| !validators.contains(node.as_str())) {
            return Err(format!(
                "{} {label} references an unknown validator",
                input.id
            ));
        }
    }
    if input
        .correct_nodes
        .iter()
        .any(|node| input.actively_byzantine.binary_search(node).is_ok())
    {
        return Err(format!(
            "{} Byzantine validators cannot be correct nodes",
            input.id
        ));
    }
    if input.proposals.is_empty() {
        return Err(format!("{} requires at least one proposal", input.id));
    }
    let mut roots = BTreeSet::new();
    for proposal in &input.proposals {
        if proposal.registry_root.is_empty() || !roots.insert(proposal.registry_root.as_str()) {
            return Err(format!(
                "{} proposal roots must be nonempty and unique",
                input.id
            ));
        }
        validate_sorted_unique("proposal supporters", &proposal.supporters)?;
        if proposal
            .supporters
            .iter()
            .any(|node| !validators.contains(node.as_str()))
        {
            return Err(format!("{} proposal has an unknown supporter", input.id));
        }
    }
    for validator in &input.validators {
        let view = input
            .trust_views
            .get(validator)
            .ok_or_else(|| format!("{} missing trust view for {validator}", input.id))?;
        if view.essential_subsets.is_empty() {
            return Err(format!(
                "{} {validator} requires an essential subset",
                input.id
            ));
        }
        let unl = input
            .local_unls
            .get(validator)
            .ok_or_else(|| format!("{} missing local UNL for {validator}", input.id))?;
        validate_sorted_unique("local UNL", unl)?;
        if unl.iter().any(|node| !validators.contains(node.as_str())) {
            return Err(format!("{} local UNL has an unknown validator", input.id));
        }
        let local_quorum = input
            .local_quorums
            .get(validator)
            .ok_or_else(|| format!("{} missing local quorum for {validator}", input.id))?;
        if *local_quorum == 0 || *local_quorum > unl.len() {
            return Err(format!(
                "{} has an invalid local quorum for {validator}",
                input.id
            ));
        }
        let mut subset_keys = BTreeSet::new();
        for subset in &view.essential_subsets {
            validate_subset(input, subset)?;
            if !subset_keys.insert(subset_key(subset)?) {
                return Err(format!(
                    "{} {validator} has a duplicate essential subset",
                    input.id
                ));
            }
        }
    }
    if input.trust_views.len() != input.validators.len()
        || input.local_unls.len() != input.validators.len()
        || input.local_quorums.len() != input.validators.len()
    {
        return Err(format!(
            "{} has extra per-validator configuration",
            input.id
        ));
    }
    Ok(())
}

fn validate_subset(input: &ScenarioInput, subset: &EssentialSubsetInput) -> Result<(), String> {
    validate_sorted_unique("essential subset validators", &subset.validators)?;
    if subset.validators.is_empty()
        || subset
            .validators
            .iter()
            .any(|node| input.validators.binary_search(node).is_err())
    {
        return Err(format!(
            "{} has an invalid essential subset scope",
            input.id
        ));
    }
    let n = subset.validators.len();
    let q = subset.quorum;
    let t = subset.max_active_byzantine;
    if q == 0
        || q > n
        || t > n
        || t >= q.saturating_mul(2).saturating_sub(n)
        || t.saturating_mul(2) >= q
    {
        return Err(format!(
            "{} essential subset violates 0<q<=n, t<2q-n, or 2t<q",
            input.id
        ));
    }
    Ok(())
}

fn fully_linked(
    input: &ScenarioInput,
    left: &str,
    right: &str,
    active_faults: &BTreeSet<&str>,
    unavailable: &BTreeSet<&str>,
) -> Result<bool, String> {
    let left_view = &input.trust_views[left];
    let right_keys: BTreeMap<String, &EssentialSubsetInput> = input.trust_views[right]
        .essential_subsets
        .iter()
        .map(|subset| Ok((subset_key(subset)?, subset)))
        .collect::<Result<_, String>>()?;
    for subset in &left_view.essential_subsets {
        let key = subset_key(subset)?;
        if right_keys.contains_key(&key) {
            let active = subset
                .validators
                .iter()
                .filter(|node| active_faults.contains(node.as_str()))
                .count();
            let responsive_correct = subset
                .validators
                .iter()
                .filter(|node| {
                    !active_faults.contains(node.as_str()) && !unavailable.contains(node.as_str())
                })
                .count();
            if active <= subset.max_active_byzantine
                && responsive_correct >= subset.quorum
                && subset.max_active_byzantine
                    <= subset.validators.len().saturating_sub(subset.quorum)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn sees_strong_support(
    input: &ScenarioInput,
    node: &str,
    supporters: &BTreeSet<&str>,
) -> Result<bool, String> {
    Ok(input.trust_views[node]
        .essential_subsets
        .iter()
        .all(|subset| {
            subset
                .validators
                .iter()
                .filter(|member| supporters.contains(member.as_str()))
                .count()
                >= subset.quorum
        }))
}

fn closure_strongly_connected(
    input: &ScenarioInput,
    start: &str,
    active_faults: &BTreeSet<&str>,
    unavailable: &BTreeSet<&str>,
    correct: &BTreeSet<&str>,
) -> Result<bool, String> {
    let mut closure = BTreeSet::new();
    let mut queue = VecDeque::from([start.to_string()]);
    while let Some(node) = queue.pop_front() {
        if !closure.insert(node.clone()) {
            continue;
        }
        for subset in &input.trust_views[&node].essential_subsets {
            for member in &subset.validators {
                if !closure.contains(member) {
                    queue.push_back(member.clone());
                }
            }
        }
    }
    let healthy: Vec<&str> = closure
        .iter()
        .map(String::as_str)
        .filter(|node| correct.contains(node) && !unavailable.contains(node))
        .collect();
    for left_index in 0..healthy.len() {
        for right_index in (left_index + 1)..healthy.len() {
            if !fully_linked(
                input,
                healthy[left_index],
                healthy[right_index],
                active_faults,
                unavailable,
            )? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn unique_common_strong_root(
    input: &ScenarioInput,
    responsive_correct: &[String],
    support: &BTreeMap<String, BTreeMap<String, bool>>,
) -> Option<String> {
    let roots: Vec<String> = input
        .proposals
        .iter()
        .filter(|proposal| {
            responsive_correct.iter().all(|node| {
                support
                    .get(node)
                    .and_then(|rows| rows.get(&proposal.registry_root))
                    .copied()
                    .unwrap_or(false)
            })
        })
        .map(|proposal| proposal.registry_root.clone())
        .collect();
    (roots.len() == 1).then(|| roots[0].clone())
}

fn rippled_local_decision(
    input: &ScenarioInput,
    node: &str,
    proposal_support: &BTreeMap<&str, BTreeSet<&str>>,
) -> Result<NodeDecision, String> {
    let unl = &input.local_unls[node];
    let quorum = input.local_quorums[node];
    let admitted: Vec<String> = input
        .proposals
        .iter()
        .filter(|proposal| {
            unl.iter()
                .filter(|member| {
                    proposal_support[proposal.registry_root.as_str()].contains(member.as_str())
                })
                .count()
                >= quorum
        })
        .map(|proposal| proposal.registry_root.clone())
        .collect();
    Ok(match admitted.as_slice() {
        [root] => NodeDecision {
            outcome: "decide".to_string(),
            registry_root: Some(root.clone()),
            reason: "one candidate reaches the node's local UNL quorum".to_string(),
        },
        [] => NodeDecision {
            outcome: "halt".to_string(),
            registry_root: None,
            reason: "no candidate reaches the node's local UNL quorum".to_string(),
        },
        _ => NodeDecision {
            outcome: "halt".to_string(),
            registry_root: None,
            reason: "multiple candidates reach local quorum; admission is ambiguous".to_string(),
        },
    })
}

fn incompatibility_reason(
    common_root: Option<&str>,
    unlinked_pairs: &[NodePair],
    strongly_connected: &BTreeMap<String, bool>,
) -> String {
    if !unlinked_pairs.is_empty() {
        "at least one responsive correct-node pair is not fully linked".to_string()
    } else if strongly_connected.values().any(|connected| !connected) {
        "at least one correct node's known closure is not strongly connected".to_string()
    } else if common_root.is_none() {
        "there is not exactly one root with strong support at every responsive correct node"
            .to_string()
    } else {
        "scenario is outside the frozen compatibility contract".to_string()
    }
}

fn conflicting_roots(nodes: &BTreeMap<String, NodeDecision>) -> usize {
    let roots: BTreeSet<&str> = nodes
        .values()
        .filter(|row| row.outcome == "decide")
        .filter_map(|row| row.registry_root.as_deref())
        .collect();
    roots.len().saturating_sub(1)
}

fn subset_key(subset: &EssentialSubsetInput) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(
        subset.validators.as_slice(),
        subset.quorum,
        subset.max_active_byzantine,
    ))
    .map_err(|error| error.to_string())?;
    Ok(sha256_hex(&bytes))
}

fn validate_sorted_unique(label: &str, values: &[String]) -> Result<(), String> {
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!("{label} must be nonempty"));
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    if sorted != values {
        return Err(format!("{label} must be sorted unique"));
    }
    Ok(())
}

fn canonical_manifest_hash(manifest: &DecisiveManifest) -> Result<String, String> {
    let mut canonical = manifest.clone();
    canonical.manifest_sha256.clear();
    let bytes = serde_json::to_vec(&canonical).map_err(|error| error.to_string())?;
    Ok(sha256_hex(&bytes))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subset(
        validators: &[&str],
        quorum: usize,
        max_active_byzantine: usize,
    ) -> EssentialSubsetInput {
        EssentialSubsetInput {
            validators: validators
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            quorum,
            max_active_byzantine,
        }
    }

    fn base_case() -> ScenarioInput {
        let validators = ["a", "b", "c", "d"];
        let ids: Vec<String> = validators
            .iter()
            .map(|value| (*value).to_string())
            .collect();
        let shared = subset(&validators, 3, 0);
        ScenarioInput {
            id: "compatible".to_string(),
            fault_class: "control".to_string(),
            validators: ids.clone(),
            correct_nodes: ids.clone(),
            unavailable: Vec::new(),
            actively_byzantine: Vec::new(),
            trust_views: ids
                .iter()
                .map(|node| {
                    (
                        node.clone(),
                        TrustViewInput {
                            essential_subsets: vec![shared.clone()],
                        },
                    )
                })
                .collect(),
            local_unls: ids.iter().map(|node| (node.clone(), ids.clone())).collect(),
            local_quorums: ids.iter().map(|node| (node.clone(), 3)).collect(),
            proposals: vec![ProposalInput {
                registry_root: "root-a".to_string(),
                supporters: ids,
            }],
            event_schedule: EventSchedule {
                delayed: false,
                duplicated: false,
                reordered: false,
                stale_replay: false,
                recover_unavailable: false,
            },
            transition: TransitionInput {
                kind: "none".to_string(),
                removed: Vec::new(),
                added: Vec::new(),
                rotated: Vec::new(),
            },
        }
    }

    #[test]
    fn compatible_shared_subset_decides() {
        let decision = evaluate_scenario(base_case()).expect("oracle decision");
        assert_eq!(decision.expected.classification, "compatible");
        assert!(decision
            .expected
            .cobalt_nodes
            .values()
            .all(|row| row.outcome == "decide"));
        assert_eq!(decision.expected.cobalt_conflicting_roots, 0);
    }

    #[test]
    fn missing_shared_subset_halts() {
        let mut scenario = base_case();
        scenario.id = "unlinked".to_string();
        scenario
            .trust_views
            .get_mut("d")
            .expect("view")
            .essential_subsets = vec![subset(&["b", "c", "d"], 3, 0)];
        let decision = evaluate_scenario(scenario).expect("oracle decision");
        assert_eq!(decision.expected.classification, "incompatible");
        assert!(decision
            .expected
            .cobalt_nodes
            .values()
            .all(|row| row.outcome == "halt"));
    }

    #[test]
    fn local_unl_split_exposes_material_delta() {
        let mut scenario = base_case();
        scenario.id = "split".to_string();
        scenario.trust_views = BTreeMap::from([
            (
                "a".to_string(),
                TrustViewInput {
                    essential_subsets: vec![subset(&["a", "b"], 2, 0)],
                },
            ),
            (
                "b".to_string(),
                TrustViewInput {
                    essential_subsets: vec![subset(&["a", "b"], 2, 0)],
                },
            ),
            (
                "c".to_string(),
                TrustViewInput {
                    essential_subsets: vec![subset(&["c", "d"], 2, 0)],
                },
            ),
            (
                "d".to_string(),
                TrustViewInput {
                    essential_subsets: vec![subset(&["c", "d"], 2, 0)],
                },
            ),
        ]);
        scenario.local_unls = BTreeMap::from([
            ("a".to_string(), vec!["a".to_string(), "b".to_string()]),
            ("b".to_string(), vec!["a".to_string(), "b".to_string()]),
            ("c".to_string(), vec!["c".to_string(), "d".to_string()]),
            ("d".to_string(), vec!["c".to_string(), "d".to_string()]),
        ]);
        scenario.local_quorums = scenario
            .validators
            .iter()
            .map(|node| (node.clone(), 2))
            .collect();
        scenario.proposals = vec![
            ProposalInput {
                registry_root: "root-a".to_string(),
                supporters: vec!["a".to_string(), "b".to_string()],
            },
            ProposalInput {
                registry_root: "root-b".to_string(),
                supporters: vec!["c".to_string(), "d".to_string()],
            },
        ];
        let decision = evaluate_scenario(scenario).expect("oracle decision");
        assert_eq!(decision.expected.cobalt_conflicting_roots, 0);
        assert_eq!(decision.expected.rippled_conflicting_roots, 1);
        assert!(decision.expected.material_safety_delta);
    }
}
