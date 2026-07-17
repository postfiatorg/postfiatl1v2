use postfiat_crypto_provider::hash_hex;
use postfiat_ordering_fast::{
    bft_fault_tolerance, simulate_adversarial_ordering, verify_no_conflicting_commits,
    ConsensusDomain, OrderingAdversaryFault, OrderingSimulationScenario, ValidatorSet,
};
use serde_json::json;

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn validators(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("validator-{index}"))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = ConsensusDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: root('0'),
        protocol_version: 1,
    };
    let validator_set = ValidatorSet::try_new(validators(7))?;
    let scenario = OrderingSimulationScenario {
        heights: 6,
        max_views_per_height: 3,
        faults: vec![
            OrderingAdversaryFault::FailedLeader { height: 2, view: 0 },
            OrderingAdversaryFault::DropVotes {
                height: 3,
                view: 0,
                validators: vec![
                    "validator-0".to_string(),
                    "validator-1".to_string(),
                    "validator-2".to_string(),
                ],
            },
            OrderingAdversaryFault::EquivocateProposal { height: 4, view: 0 },
            OrderingAdversaryFault::DuplicateVotes { height: 5, view: 0 },
            OrderingAdversaryFault::StaleVotes {
                height: 6,
                view: 0,
                stale_height: 5,
                stale_view: 0,
            },
        ],
    };
    let simulation = simulate_adversarial_ordering(&domain, &validator_set, &scenario)?;
    verify_no_conflicting_commits(&simulation.commits)?;
    let simulation_hash = hash_hex(
        "postfiat.hotstuff_ordering_evidence.simulation.v1",
        &serde_json::to_vec(&simulation)?,
    );
    let report = json!({
        "schema": "postfiat-hotstuff-ordering-evidence-v1",
        "status": "passed",
        "checker": "simulate_adversarial_ordering",
        "domain": domain,
        "validator_count": validator_set.validators.len(),
        "quorum": validator_set.quorum,
        "fault_tolerance": bft_fault_tolerance(validator_set.validators.len())?,
        "scenario": scenario,
        "observed": {
            "certified": simulation.certified.len(),
            "timeouts": simulation.timeouts.len(),
            "commits": simulation.commits.len(),
            "equivocations": simulation.equivocations.len(),
            "stalled_views": simulation.stalled_views.len(),
            "no_conflicting_commits": true
        },
        "simulation_hash": simulation_hash,
        "simulation": simulation
    });
    let body = serde_json::to_string_pretty(&report)? + "\n";
    if let Some(path) = std::env::var_os("REPORT") {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &body)?;
    } else {
        print!("{body}");
    }
    Ok(())
}
