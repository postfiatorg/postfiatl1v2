use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use postfiat_cobalt_adversarial_oracle::{
    evaluate as evaluate_second_oracle, sha256_hex, verify_manifest, CorpusManifest, GraphCase,
    OracleClassification, ValidatorPair,
};
use postfiat_cobalt_decision_oracle::{
    evaluate_scenario as evaluate_first_oracle, EssentialSubsetInput, EventSchedule, ProposalInput,
    ScenarioInput, TransitionInput, TrustViewInput,
};
use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_trust_graph, build_trust_view,
    certify_nonuniform_governance_amendment, has_strong_support,
    propose_nonuniform_governance_amendment, verify_nonuniform_governance_certificate,
    CobaltDomain, CobaltFaultModel, LinkageReport, TrustGraph,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const REPORT_SCHEMA: &str = "postfiat-cobalt-adversarial-e1-comparison-v1";

#[derive(Debug, Clone, Serialize)]
struct RouteResult {
    valid: bool,
    compatible: bool,
    strong_support_validators: Vec<String>,
    fully_linked_pairs: Vec<ValidatorPair>,
    strongly_connected_validators: Vec<String>,
    certificate_accepted: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ComparisonRecord {
    case_id: String,
    validator_count: usize,
    boundary_tags: Vec<String>,
    second_oracle: RouteResult,
    first_oracle: RouteResult,
    production: RouteResult,
    validity_agreement: bool,
    first_oracle_agreement: bool,
    production_linkage_agreement: bool,
    production_support_agreement: bool,
    production_certificate_agreement: bool,
    complete_agreement: bool,
}

#[derive(Debug, Clone, Serialize)]
struct FrozenDisagreement {
    case_definition: GraphCase,
    comparison: ComparisonRecord,
}

#[derive(Debug, Clone, Serialize)]
struct Summary {
    schema: &'static str,
    corpus_sha256: String,
    case_count: usize,
    valid_case_count: usize,
    invalid_boundary_case_count: usize,
    compatible_case_count: usize,
    incompatible_case_count: usize,
    disagreement_count: usize,
    classification_sha256: String,
    second_oracle: &'static str,
    first_oracle: &'static str,
    production_routes: [&'static str; 3],
    summary_only: bool,
    pass: bool,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read_manifest(path: &Path) -> io::Result<CorpusManifest> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let mut file = File::options().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(io::Error::other)?;
    file.write_all(b"\n")
}

fn pair(left: &str, right: &str) -> ValidatorPair {
    ValidatorPair {
        left: left.to_string(),
        right: right.to_string(),
    }
}

fn route_from_second(classification: &OracleClassification) -> RouteResult {
    RouteResult {
        valid: classification.valid,
        compatible: classification.compatible,
        strong_support_validators: classification.strong_support_validators.clone(),
        fully_linked_pairs: classification.fully_linked_pairs.clone(),
        strongly_connected_validators: classification.strongly_connected_validators.clone(),
        certificate_accepted: classification.compatible,
        error: classification.validation_error.clone(),
    }
}

fn first_oracle_input(case: &GraphCase) -> ScenarioInput {
    let trust_views = case
        .trust_views
        .iter()
        .map(|(validator, view)| {
            (
                validator.clone(),
                TrustViewInput {
                    essential_subsets: view
                        .essential_subsets
                        .iter()
                        .map(|subset| EssentialSubsetInput {
                            validators: subset.validators.clone(),
                            quorum: subset.quorum,
                            max_active_byzantine: subset.max_active_byzantine,
                        })
                        .collect(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let local_unls = case
        .trust_views
        .iter()
        .map(|(validator, view)| {
            let unl = view
                .essential_subsets
                .iter()
                .flat_map(|subset| subset.validators.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            (validator.clone(), unl)
        })
        .collect::<BTreeMap<_, _>>();
    let local_quorums = case
        .trust_views
        .iter()
        .map(|(validator, view)| {
            (
                validator.clone(),
                view.essential_subsets
                    .first()
                    .map(|subset| subset.quorum)
                    .unwrap_or(1),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let active = case
        .actively_byzantine
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let correct_nodes = case
        .validators
        .iter()
        .filter(|validator| !active.contains(validator.as_str()))
        .cloned()
        .collect();

    ScenarioInput {
        id: case.id.clone(),
        fault_class: case.boundary_tags.join("+"),
        validators: case.validators.clone(),
        correct_nodes,
        unavailable: Vec::new(),
        actively_byzantine: case.actively_byzantine.clone(),
        trust_views,
        local_unls,
        local_quorums,
        proposals: vec![ProposalInput {
            registry_root: "candidate-registry-root".to_string(),
            supporters: case.support.clone(),
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

fn classify_first(case: &GraphCase) -> RouteResult {
    match evaluate_first_oracle(first_oracle_input(case)) {
        Ok(decision) => {
            let strong_support_validators = decision
                .oracle_trace
                .strong_support
                .iter()
                .filter(|(_, roots)| roots.values().all(|supported| *supported))
                .map(|(validator, _)| validator.clone())
                .collect();
            let strongly_connected_validators = decision
                .oracle_trace
                .strongly_connected
                .iter()
                .filter(|(_, connected)| **connected)
                .map(|(validator, _)| validator.clone())
                .collect();
            RouteResult {
                valid: true,
                compatible: decision.expected.classification == "compatible",
                strong_support_validators,
                fully_linked_pairs: decision
                    .oracle_trace
                    .fully_linked_pairs
                    .iter()
                    .map(|value| pair(&value.left, &value.right))
                    .collect(),
                strongly_connected_validators,
                certificate_accepted: false,
                error: None,
            }
        }
        Err(error) => RouteResult {
            valid: false,
            compatible: false,
            strong_support_validators: Vec::new(),
            fully_linked_pairs: Vec::new(),
            strongly_connected_validators: Vec::new(),
            certificate_accepted: false,
            error: Some(error),
        },
    }
}

fn domain() -> CobaltDomain {
    CobaltDomain {
        chain_id: "postfiat-cobalt-e1".to_string(),
        genesis_hash: "11".repeat(32),
        protocol_version: 1,
    }
}

fn build_production_graph(case: &GraphCase, domain: &CobaltDomain) -> Result<TrustGraph, String> {
    let mut views = Vec::new();
    for validator in &case.validators {
        let subsets = case.trust_views[validator]
            .essential_subsets
            .iter()
            .map(|subset| {
                build_essential_subset(
                    domain,
                    subset.validators.clone(),
                    subset.max_active_byzantine,
                    subset.quorum,
                    Vec::new(),
                    1,
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        views.push(build_trust_view(domain, validator, 1, subsets, "")?);
    }
    build_trust_graph(domain, 1, "22".repeat(32), 1, None, views)
}

fn production_pairs(report: &LinkageReport) -> Vec<ValidatorPair> {
    report
        .fully_linked_pairs
        .iter()
        .map(|value| pair(&value.left, &value.right))
        .collect()
}

fn classify_production(case: &GraphCase) -> RouteResult {
    let domain = domain();
    let graph = match build_production_graph(case, &domain) {
        Ok(graph) => graph,
        Err(error) => {
            return RouteResult {
                valid: false,
                compatible: false,
                strong_support_validators: Vec::new(),
                fully_linked_pairs: Vec::new(),
                strongly_connected_validators: Vec::new(),
                certificate_accepted: false,
                error: Some(error),
            }
        }
    };
    let report = match analyze_trust_graph(
        &domain,
        &graph,
        &CobaltFaultModel {
            actively_byzantine: case.actively_byzantine.clone(),
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            return RouteResult {
                valid: false,
                compatible: false,
                strong_support_validators: Vec::new(),
                fully_linked_pairs: Vec::new(),
                strongly_connected_validators: Vec::new(),
                certificate_accepted: false,
                error: Some(error),
            }
        }
    };
    let strong_support_validators = graph
        .trust_views
        .iter()
        .filter_map(|view| {
            has_strong_support(view, &case.support)
                .ok()
                .filter(|supported| *supported)
                .map(|_| view.validator.clone())
        })
        .collect::<Vec<_>>();
    let certificate_result = (|| {
        let proposal =
            propose_nonuniform_governance_amendment(&domain, &graph, "validator_set", 1)?;
        let local_validator = case
            .validators
            .first()
            .ok_or_else(|| "missing local validator".to_string())?;
        let certificate = certify_nonuniform_governance_amendment(
            &domain,
            &graph,
            &report,
            local_validator,
            &proposal,
            case.support.clone(),
            1,
        )?;
        verify_nonuniform_governance_certificate(
            &domain,
            &graph,
            &report,
            &proposal,
            &certificate,
            1,
        )
    })();
    let certificate_accepted = certificate_result.is_ok();
    let compatible = report.unsafe_pairs.is_empty()
        && report.strongly_connected_validators.len() == case.validators.len()
        && strong_support_validators.len() == case.validators.len()
        && certificate_accepted;
    RouteResult {
        valid: true,
        compatible,
        strong_support_validators,
        fully_linked_pairs: production_pairs(&report),
        strongly_connected_validators: report.strongly_connected_validators,
        certificate_accepted,
        error: certificate_result.err(),
    }
}

fn filtered_for_correct(values: &[String], actively_byzantine: &[String]) -> Vec<String> {
    let active = actively_byzantine
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    values
        .iter()
        .filter(|validator| !active.contains(validator.as_str()))
        .cloned()
        .collect()
}

fn filtered_pairs_for_correct(
    values: &[ValidatorPair],
    actively_byzantine: &[String],
) -> Vec<ValidatorPair> {
    let active = actively_byzantine
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    values
        .iter()
        .filter(|value| {
            !active.contains(value.left.as_str()) && !active.contains(value.right.as_str())
        })
        .cloned()
        .collect()
}

fn compare_case(case: &GraphCase) -> ComparisonRecord {
    let second_classification = evaluate_second_oracle(case);
    let second = route_from_second(&second_classification);
    let first = classify_first(case);
    let production = classify_production(case);

    let validity_agreement = second.valid == first.valid && second.valid == production.valid;
    let expected_correct_support =
        filtered_for_correct(&second.strong_support_validators, &case.actively_byzantine);
    let expected_correct_connectivity = filtered_for_correct(
        &second.strongly_connected_validators,
        &case.actively_byzantine,
    );
    let expected_correct_pairs =
        filtered_pairs_for_correct(&second.fully_linked_pairs, &case.actively_byzantine);
    let first_oracle_agreement = second.compatible == first.compatible
        && expected_correct_support == first.strong_support_validators
        && expected_correct_connectivity == first.strongly_connected_validators
        && expected_correct_pairs == first.fully_linked_pairs;
    let production_linkage_agreement = second.fully_linked_pairs == production.fully_linked_pairs
        && second.strongly_connected_validators == production.strongly_connected_validators;
    let production_support_agreement =
        second.strong_support_validators == production.strong_support_validators;
    let production_certificate_agreement = second.compatible == production.certificate_accepted;
    let complete_agreement = validity_agreement
        && first_oracle_agreement
        && production_linkage_agreement
        && production_support_agreement
        && production_certificate_agreement
        && second.compatible == production.compatible;

    ComparisonRecord {
        case_id: case.id.clone(),
        validator_count: case.validators.len(),
        boundary_tags: case.boundary_tags.clone(),
        second_oracle: second,
        first_oracle: first,
        production,
        validity_agreement,
        first_oracle_agreement,
        production_linkage_agreement,
        production_support_agreement,
        production_certificate_agreement,
        complete_agreement,
    }
}

fn run(manifest_path: &Path, output_dir: &Path, summary_only: bool) -> io::Result<Summary> {
    let manifest = read_manifest(manifest_path)?;
    let cases = verify_manifest(&manifest).map_err(invalid)?;
    fs::create_dir(output_dir)?;
    let mut classifications = if summary_only {
        None
    } else {
        Some(
            File::options()
                .write(true)
                .create_new(true)
                .open(output_dir.join("classifications.jsonl"))?,
        )
    };
    let mut classification_hasher = Sha256::new();
    let mut disagreements = Vec::new();
    let mut valid = 0;
    let mut compatible = 0;

    for case in &cases {
        let comparison = compare_case(case);
        valid += usize::from(comparison.second_oracle.valid);
        compatible += usize::from(comparison.second_oracle.compatible);
        let mut line = serde_json::to_vec(&comparison).map_err(io::Error::other)?;
        line.push(b'\n');
        classification_hasher.update(&line);
        if let Some(file) = classifications.as_mut() {
            file.write_all(&line)?;
        }
        if !comparison.complete_agreement {
            disagreements.push(FrozenDisagreement {
                case_definition: case.clone(),
                comparison,
            });
        }
    }

    let classification_sha256 = classification_hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let summary = Summary {
        schema: REPORT_SCHEMA,
        corpus_sha256: manifest.corpus_sha256,
        case_count: cases.len(),
        valid_case_count: valid,
        invalid_boundary_case_count: cases.len() - valid,
        compatible_case_count: compatible,
        incompatible_case_count: valid - compatible,
        disagreement_count: disagreements.len(),
        classification_sha256,
        second_oracle: "postfiat-cobalt-adversarial-oracle (independent v2)",
        first_oracle: "postfiat-cobalt-decision-oracle (activation v1)",
        production_routes: [
            "analyze_trust_graph",
            "has_strong_support",
            "verify_nonuniform_governance_certificate",
        ],
        summary_only,
        pass: disagreements.is_empty(),
    };
    write_new_json(&output_dir.join("disagreements.json"), &disagreements)?;
    write_new_json(&output_dir.join("summary.json"), &summary)?;
    Ok(summary)
}

fn main() -> io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) != Some("compare") {
        return Err(invalid(
            "usage: postfiat-cobalt-e1-harness compare MANIFEST OUTPUT_DIR [--summary-only]",
        ));
    }
    let manifest = args
        .get(2)
        .map(PathBuf::from)
        .ok_or_else(|| invalid("missing manifest path"))?;
    let output = args
        .get(3)
        .map(PathBuf::from)
        .ok_or_else(|| invalid("missing output directory"))?;
    let summary_only = args.get(4).map(String::as_str) == Some("--summary-only");
    let summary = run(&manifest, &output, summary_only)?;
    println!(
        "E1 compared {} cases: disagreements={}, corpus={}, classifications={}",
        summary.case_count,
        summary.disagreement_count,
        summary.corpus_sha256,
        summary.classification_sha256
    );
    if !summary.pass {
        return Err(invalid("E1 comparison found frozen disagreements"));
    }
    let summary_bytes = serde_json::to_vec(&summary).map_err(io::Error::other)?;
    println!("summary_sha256={}", sha256_hex(&summary_bytes));
    Ok(())
}
