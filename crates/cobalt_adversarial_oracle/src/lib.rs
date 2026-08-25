//! Second independent oracle and deterministic corpus for Cobalt E1.
//!
//! This crate deliberately has no dependency on production Cobalt, the first
//! decision oracle, or any other PostFiat protocol crate. The rules below are
//! derived directly from the locked adversarial-verification specification.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CORPUS_SCHEMA: &str = "postfiat-cobalt-adversarial-e1-corpus-v1";
pub const GENERATOR_VERSION: &str = "cobalt-adversarial-graph-generator-v1";
pub const ORACLE_VERSION: &str = "cobalt-independent-essential-subset-oracle-v2";
pub const DEFAULT_CASE_COUNT: usize = 10_240;
pub const DEFAULT_SEED: u64 = 0xc0ba_1720_2608_2501;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EssentialSubset {
    pub validators: Vec<String>,
    pub quorum: usize,
    pub max_active_byzantine: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustView {
    pub essential_subsets: Vec<EssentialSubset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphCase {
    pub id: String,
    pub boundary_tags: Vec<String>,
    pub validators: Vec<String>,
    pub actively_byzantine: Vec<String>,
    pub support: Vec<String>,
    pub trust_views: BTreeMap<String, TrustView>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValidatorPair {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleClassification {
    pub valid: bool,
    pub validation_error: Option<String>,
    pub linked_pairs: Vec<ValidatorPair>,
    pub fully_linked_pairs: Vec<ValidatorPair>,
    pub strongly_connected_validators: Vec<String>,
    pub strong_support_validators: Vec<String>,
    pub compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusManifest {
    pub schema: String,
    pub generator_version: String,
    pub oracle_version: String,
    pub seed: u64,
    pub case_count: usize,
    pub validator_count_min: usize,
    pub validator_count_max: usize,
    pub boundary_case_counts: BTreeMap<String, usize>,
    pub corpus_sha256: String,
    pub manifest_sha256: String,
}

pub fn generate_corpus(seed: u64, count: usize) -> Vec<GraphCase> {
    let mut rng = DeterministicRng(seed);
    (0..count)
        .map(|index| generate_case(index, &mut rng))
        .collect()
}

pub fn build_manifest(seed: u64, count: usize) -> CorpusManifest {
    let cases = generate_corpus(seed, count);
    let mut boundary_case_counts = BTreeMap::new();
    for case in &cases {
        for tag in &case.boundary_tags {
            *boundary_case_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    let mut manifest = CorpusManifest {
        schema: CORPUS_SCHEMA.to_string(),
        generator_version: GENERATOR_VERSION.to_string(),
        oracle_version: ORACLE_VERSION.to_string(),
        seed,
        case_count: count,
        validator_count_min: cases
            .iter()
            .map(|case| case.validators.len())
            .min()
            .unwrap_or(0),
        validator_count_max: cases
            .iter()
            .map(|case| case.validators.len())
            .max()
            .unwrap_or(0),
        boundary_case_counts,
        corpus_sha256: corpus_sha256(&cases),
        manifest_sha256: String::new(),
    };
    manifest.manifest_sha256 = manifest_sha256(&manifest);
    manifest
}

pub fn verify_manifest(manifest: &CorpusManifest) -> Result<Vec<GraphCase>, String> {
    if manifest.schema != CORPUS_SCHEMA
        || manifest.generator_version != GENERATOR_VERSION
        || manifest.oracle_version != ORACLE_VERSION
    {
        return Err("unsupported E1 corpus manifest identity".to_string());
    }
    if manifest.case_count < 10_000
        || manifest.validator_count_min != 6
        || manifest.validator_count_max != 20
    {
        return Err("E1 corpus manifest does not meet size or validator-range gate".to_string());
    }
    if manifest.manifest_sha256 != manifest_sha256(manifest) {
        return Err("E1 corpus manifest SHA-256 mismatch".to_string());
    }
    let cases = generate_corpus(manifest.seed, manifest.case_count);
    if corpus_sha256(&cases) != manifest.corpus_sha256 {
        return Err("regenerated E1 corpus SHA-256 mismatch".to_string());
    }
    let rebuilt = build_manifest(manifest.seed, manifest.case_count);
    if rebuilt.boundary_case_counts != manifest.boundary_case_counts {
        return Err("E1 boundary coverage counts mismatch".to_string());
    }
    Ok(cases)
}

pub fn evaluate(case: &GraphCase) -> OracleClassification {
    if let Err(error) = validate_case(case) {
        return OracleClassification {
            valid: false,
            validation_error: Some(error),
            linked_pairs: Vec::new(),
            fully_linked_pairs: Vec::new(),
            strongly_connected_validators: Vec::new(),
            strong_support_validators: Vec::new(),
            compatible: false,
        };
    }

    let active: BTreeSet<&str> = case.actively_byzantine.iter().map(String::as_str).collect();
    let support: BTreeSet<&str> = case.support.iter().map(String::as_str).collect();
    let mut linked_pairs = Vec::new();
    let mut fully_linked_pairs = Vec::new();
    for left_index in 0..case.validators.len() {
        for right_index in (left_index + 1)..case.validators.len() {
            let pair = ValidatorPair {
                left: case.validators[left_index].clone(),
                right: case.validators[right_index].clone(),
            };
            if pair_linked(case, &pair.left, &pair.right, &active, false) {
                linked_pairs.push(pair.clone());
            }
            if pair_linked(case, &pair.left, &pair.right, &active, true) {
                fully_linked_pairs.push(pair);
            }
        }
    }

    let strongly_connected_validators = case
        .validators
        .iter()
        .filter(|validator| closure_fully_linked(case, validator, &active))
        .cloned()
        .collect::<Vec<_>>();
    let strong_support_validators = case
        .validators
        .iter()
        .filter(|validator| has_strong_support(&case.trust_views[*validator], &support))
        .cloned()
        .collect::<Vec<_>>();
    let pair_count = case
        .validators
        .len()
        .saturating_mul(case.validators.len().saturating_sub(1))
        / 2;
    let compatible = fully_linked_pairs.len() == pair_count
        && strongly_connected_validators.len() == case.validators.len()
        && strong_support_validators.len() == case.validators.len();

    OracleClassification {
        valid: true,
        validation_error: None,
        linked_pairs,
        fully_linked_pairs,
        strongly_connected_validators,
        strong_support_validators,
        compatible,
    }
}

pub fn corpus_sha256(cases: &[GraphCase]) -> String {
    sha256_hex(&serde_json::to_vec(cases).expect("GraphCase serialization is infallible"))
}

pub fn manifest_sha256(manifest: &CorpusManifest) -> String {
    let mut canonical = manifest.clone();
    canonical.manifest_sha256.clear();
    sha256_hex(&serde_json::to_vec(&canonical).expect("CorpusManifest serialization is infallible"))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_case(case: &GraphCase) -> Result<(), String> {
    validate_sorted_unique("validators", &case.validators)?;
    validate_sorted_unique("actively_byzantine", &case.actively_byzantine)?;
    validate_sorted_unique("support", &case.support)?;
    if !(6..=20).contains(&case.validators.len()) {
        return Err("validator count is outside 6..=20".to_string());
    }
    let known = case
        .validators
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if case
        .actively_byzantine
        .iter()
        .chain(&case.support)
        .any(|validator| !known.contains(validator.as_str()))
    {
        return Err("fault or support set references an unknown validator".to_string());
    }
    if case.trust_views.len() != case.validators.len()
        || case
            .validators
            .iter()
            .any(|validator| !case.trust_views.contains_key(validator))
    {
        return Err("trust views do not exactly cover validators".to_string());
    }
    for validator in &case.validators {
        let view = &case.trust_views[validator];
        if view.essential_subsets.is_empty() {
            return Err(format!("{validator} has no essential subset"));
        }
        let mut unique = BTreeSet::new();
        let mut owner_seen = false;
        for subset in &view.essential_subsets {
            validate_sorted_unique("essential subset validators", &subset.validators)?;
            if subset.validators.is_empty()
                || subset
                    .validators
                    .iter()
                    .any(|member| !known.contains(member.as_str()))
            {
                return Err(format!("{validator} has an invalid subset scope"));
            }
            owner_seen |= subset.validators.binary_search(validator).is_ok();
            if !unique.insert((
                subset.validators.clone(),
                subset.quorum,
                subset.max_active_byzantine,
            )) {
                return Err(format!("{validator} has a duplicate essential subset"));
            }
            let n = subset.validators.len();
            let q = subset.quorum;
            let t = subset.max_active_byzantine;
            if q == 0 || q > n {
                return Err("essential subset violates 0 < q <= n".to_string());
            }
            if t >= q.saturating_mul(2).saturating_sub(n) {
                return Err("essential subset violates t < 2q - n".to_string());
            }
            if t.saturating_mul(2) >= q {
                return Err("essential subset violates 2t < q".to_string());
            }
        }
        if !owner_seen {
            return Err(format!("{validator} owner is outside its derived UNL"));
        }
    }
    Ok(())
}

fn validate_sorted_unique(label: &str, values: &[String]) -> Result<(), String> {
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(format!("{label} entries must be nonempty"));
    }
    let mut canonical = values.to_vec();
    canonical.sort();
    canonical.dedup();
    if canonical != values {
        return Err(format!("{label} must be sorted unique"));
    }
    Ok(())
}

fn pair_linked(
    case: &GraphCase,
    left: &str,
    right: &str,
    active: &BTreeSet<&str>,
    require_liveness: bool,
) -> bool {
    let right_subsets = &case.trust_views[right].essential_subsets;
    case.trust_views[left]
        .essential_subsets
        .iter()
        .any(|subset| {
            right_subsets.contains(subset) && {
                let faults = subset
                    .validators
                    .iter()
                    .filter(|validator| active.contains(validator.as_str()))
                    .count();
                faults <= subset.max_active_byzantine
                    && (!require_liveness
                        || (subset.validators.len().saturating_sub(faults) >= subset.quorum
                            && subset.max_active_byzantine
                                <= subset.validators.len().saturating_sub(subset.quorum)))
            }
        })
}

fn has_strong_support(view: &TrustView, support: &BTreeSet<&str>) -> bool {
    view.essential_subsets.iter().all(|subset| {
        subset
            .validators
            .iter()
            .filter(|validator| support.contains(validator.as_str()))
            .count()
            >= subset.quorum
    })
}

fn closure_fully_linked(case: &GraphCase, start: &str, active: &BTreeSet<&str>) -> bool {
    let mut closure = BTreeSet::new();
    let mut frontier = VecDeque::from([start.to_string()]);
    while let Some(validator) = frontier.pop_front() {
        if !closure.insert(validator.clone()) {
            continue;
        }
        for subset in &case.trust_views[&validator].essential_subsets {
            for member in &subset.validators {
                if !closure.contains(member) {
                    frontier.push_back(member.clone());
                }
            }
        }
    }
    let closure = closure.into_iter().collect::<Vec<_>>();
    for left_index in 0..closure.len() {
        for right_index in (left_index + 1)..closure.len() {
            if !pair_linked(
                case,
                &closure[left_index],
                &closure[right_index],
                active,
                true,
            ) {
                return false;
            }
        }
    }
    true
}

fn generate_case(index: usize, rng: &mut DeterministicRng) -> GraphCase {
    let validator_count = 6 + index % 15;
    let validators = (0..validator_count)
        .map(|number| format!("v{number:02}"))
        .collect::<Vec<_>>();
    let mut views = validators
        .iter()
        .map(|validator| {
            (
                validator.clone(),
                TrustView {
                    essential_subsets: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut active = Vec::new();
    let mut tags = Vec::new();

    match index % 12 {
        0 => {
            let quorum = validator_count / 2 + 1;
            let t = quorum * 2 - validator_count - 1;
            add_global(&mut views, subset(&validators, quorum, t));
            tags.push("t_plus_one_equals_2q_minus_n".to_string());
        }
        1 => {
            let quorum = if validator_count % 2 == 0 {
                validator_count - 1
            } else {
                validator_count
            };
            add_global(&mut views, subset(&validators, quorum, (quorum - 1) / 2));
            tags.push("two_t_plus_one_equals_q".to_string());
        }
        2 => {
            add_global(&mut views, subset(&validators, validator_count - 1, 1));
            tags.push("t_equals_n_minus_q".to_string());
        }
        3 => {
            add_global(&mut views, subset(&validators, validator_count - 1, 2));
            tags.push("t_equals_n_minus_q_plus_one".to_string());
        }
        4 => {
            add_global(&mut views, subset(&validators, validator_count - 1, 1));
            active.push(validators[0].clone());
            tags.extend([
                "active_faults_equal_t".to_string(),
                "responsive_correct_equal_q".to_string(),
            ]);
        }
        5 => {
            add_global(&mut views, subset(&validators, validator_count - 1, 1));
            active.extend([validators[0].clone(), validators[1].clone()]);
            tags.extend([
                "active_faults_equal_t_plus_one".to_string(),
                "responsive_correct_equal_q_minus_one".to_string(),
            ]);
        }
        6 => {
            let valid = valid_parameters(validator_count);
            let (quorum, t) = valid[rng.choose(valid.len())];
            add_global(&mut views, subset(&validators, quorum, t));
            tags.push("deterministic_random_valid_parameters".to_string());
        }
        7 => {
            let split = validator_count / 2;
            let groups = [&validators[..split], &validators[split..]];
            for group in groups {
                let shared = subset(group, group.len(), 0);
                for validator in group {
                    views
                        .get_mut(validator)
                        .expect("partition validator")
                        .essential_subsets
                        .push(shared.clone());
                }
            }
            tags.push("deterministic_random_partition".to_string());
        }
        8 => {
            for (owner_index, owner) in validators.iter().enumerate() {
                let size = 3 + rng.choose(validator_count - 3);
                let members = ring_members(&validators, owner_index, size);
                views
                    .get_mut(owner)
                    .expect("ring owner")
                    .essential_subsets
                    .push(subset(&members, members.len(), 0));
            }
            tags.push("deterministic_random_local_views".to_string());
        }
        9 => {
            let quorum = validator_count / 2 + 1;
            add_global(
                &mut views,
                subset(&validators, quorum, quorum * 2 - validator_count),
            );
            tags.push("invalid_t_equals_2q_minus_n".to_string());
        }
        10 => {
            let quorum = if validator_count % 2 == 0 {
                validator_count
            } else {
                validator_count - 1
            };
            add_global(&mut views, subset(&validators, quorum, quorum / 2));
            tags.push("invalid_two_t_equals_q".to_string());
        }
        11 => {
            add_global(&mut views, subset(&validators, validator_count - 1, 1));
            add_random_owner_subsets(&mut views, &validators, rng);
            tags.push("deterministic_random_multi_subset".to_string());
        }
        _ => unreachable!(),
    }

    for view in views.values_mut() {
        view.essential_subsets.sort_by(|left, right| {
            (&left.validators, left.quorum, left.max_active_byzantine).cmp(&(
                &right.validators,
                right.quorum,
                right.max_active_byzantine,
            ))
        });
        view.essential_subsets.dedup();
    }
    active.sort();
    tags.sort();

    GraphCase {
        id: format!("e1-graph-{index:05}"),
        boundary_tags: tags,
        validators: validators.clone(),
        actively_byzantine: active,
        support: validators,
        trust_views: views,
    }
}

fn add_global(views: &mut BTreeMap<String, TrustView>, shared: EssentialSubset) {
    for view in views.values_mut() {
        view.essential_subsets.push(shared.clone());
    }
}

fn add_random_owner_subsets(
    views: &mut BTreeMap<String, TrustView>,
    validators: &[String],
    rng: &mut DeterministicRng,
) {
    for (owner_index, owner) in validators.iter().enumerate() {
        let size = 3 + rng.choose(validators.len() - 2);
        let members = ring_members(validators, owner_index, size.min(validators.len()));
        views
            .get_mut(owner)
            .expect("random subset owner")
            .essential_subsets
            .push(subset(&members, members.len(), 0));
    }
}

fn ring_members(validators: &[String], start: usize, count: usize) -> Vec<String> {
    let mut members = (0..count)
        .map(|offset| validators[(start + offset) % validators.len()].clone())
        .collect::<Vec<_>>();
    members.sort();
    members.dedup();
    members
}

fn subset(validators: &[String], quorum: usize, t: usize) -> EssentialSubset {
    EssentialSubset {
        validators: validators.to_vec(),
        quorum,
        max_active_byzantine: t,
    }
}

fn valid_parameters(n: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    for quorum in 1..=n {
        for t in 0..=n {
            if t < quorum.saturating_mul(2).saturating_sub(n) && t.saturating_mul(2) < quorum {
                result.push((quorum, t));
            }
        }
    }
    result
}

struct DeterministicRng(u64);

impl DeterministicRng {
    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn choose(&mut self, upper: usize) -> usize {
        (self.next() as usize) % upper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_is_deterministic_and_covers_the_gate() {
        let first = build_manifest(DEFAULT_SEED, DEFAULT_CASE_COUNT);
        let second = build_manifest(DEFAULT_SEED, DEFAULT_CASE_COUNT);
        assert_eq!(first, second);
        assert_eq!(first.case_count, 10_240);
        assert_eq!(first.validator_count_min, 6);
        assert_eq!(first.validator_count_max, 20);
        for tag in [
            "t_plus_one_equals_2q_minus_n",
            "two_t_plus_one_equals_q",
            "active_faults_equal_t",
            "active_faults_equal_t_plus_one",
            "responsive_correct_equal_q",
            "responsive_correct_equal_q_minus_one",
            "t_equals_n_minus_q",
            "t_equals_n_minus_q_plus_one",
            "invalid_t_equals_2q_minus_n",
            "invalid_two_t_equals_q",
        ] {
            assert!(first.boundary_case_counts.contains_key(tag), "{tag}");
        }
        verify_manifest(&first).expect("frozen corpus verifies");
    }

    #[test]
    fn valid_and_invalid_boundary_cases_are_distinguished() {
        let cases = generate_corpus(DEFAULT_SEED, 12);
        assert!(evaluate(&cases[0]).valid);
        assert!(evaluate(&cases[2]).compatible);
        assert!(!evaluate(&cases[3]).compatible);
        assert!(!evaluate(&cases[9]).valid);
        assert!(!evaluate(&cases[10]).valid);
    }

    #[test]
    fn partition_is_not_fully_linked() {
        let cases = generate_corpus(DEFAULT_SEED, 8);
        let classification = evaluate(&cases[7]);
        assert!(classification.valid);
        assert!(!classification.compatible);
        assert!(
            classification.fully_linked_pairs.len()
                < cases[7].validators.len() * (cases[7].validators.len() - 1) / 2
        );
    }
}
