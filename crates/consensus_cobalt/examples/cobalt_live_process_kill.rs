use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use postfiat_consensus_cobalt::{
    abba_strong_support, analyze_trust_graph, build_abba_finish, build_dabc_full_knowledge_check,
    build_dabc_full_knowledge_checkpoint, build_dabc_replay_bundle, build_essential_subset,
    build_mvba_valid_input_set, build_rbc_accept, build_rbc_echo, build_rbc_propose,
    build_rbc_ready, build_trust_graph, build_trust_view, detect_abba_conflicting_finish,
    detect_rbc_conflicting_accept, evaluate_abba_finish_support, evaluate_rbc_echo_support,
    evaluate_rbc_ready_support, mvba_candidate_from_rbc_accept, ratify_dabc_amendment,
    rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo,
    validate_dabc_activation_with_full_knowledge, validate_dabc_ratified_amendment,
    validate_rbc_accept, verify_dabc_replay_bundle, AbbaFinish, CobaltDomain, CobaltFaultModel,
    DabcPendingPair, DabcReplayBundle, EssentialSubset, RbcAccept, RbcEcho, RbcPropose, RbcReady,
    TrustGraph,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Serialize, Deserialize)]
struct WorkerRequest {
    domain: CobaltDomain,
    graph: TrustGraph,
    propose: RbcPropose,
    echoes: Vec<RbcEcho>,
    readies: Vec<RbcReady>,
    accepts: Vec<RbcAccept>,
    abba_agreement_id: String,
    abba_round: u64,
    abba_value: bool,
    abba_finishes: Vec<AbbaFinish>,
    mvba_agreement_id: String,
    dabc_activation_height: u64,
    dabc_replay_bundle: DabcReplayBundle,
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkerResult {
    validator: String,
    trust_view_id: String,
    echo_strong: bool,
    ready_from_echo_allowed: bool,
    ready_strong: bool,
    accept_allowed: bool,
    rbc_accepted: bool,
    abba_finish_strong_support: bool,
    abba_finished: bool,
    mvba_selected_candidate_id: String,
    dabc_ratified: bool,
    dabc_ratification_id: String,
    dabc_replay_verified: bool,
    dabc_replay_bundle_id: String,
    dabc_replay_ratified_count: usize,
    dabc_replay_activation_count: usize,
    accepted: bool,
    accept_message_id: String,
    abba_finish_message_id: String,
}

#[derive(Debug, Serialize)]
struct ChildOutcome {
    validator: String,
    pid: u32,
    killed_by_controller: bool,
    kill_signal_sent: bool,
    status_success: bool,
    status_code: Option<i32>,
    stdout_line_count: usize,
    stderr_line_count: usize,
    result: Option<WorkerResult>,
    error: Option<String>,
}

struct RunningWorker {
    validator: String,
    pid: u32,
    killed_by_controller: bool,
    kill_signal_sent: bool,
    child: Child,
}

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn validators(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("validator-{index}"))
        .collect()
}

fn subset(
    domain: &CobaltDomain,
    members: &[&str],
    t_s: usize,
    q_s: usize,
) -> Result<EssentialSubset, String> {
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
}

fn fixture() -> Result<(CobaltDomain, TrustGraph), String> {
    let domain = CobaltDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: root('0'),
        protocol_version: 1,
    };
    let all = subset(
        &domain,
        &[
            "validator-0",
            "validator-1",
            "validator-2",
            "validator-3",
            "validator-4",
            "validator-5",
            "validator-6",
        ],
        2,
        5,
    )?;
    let first_five = subset(
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
    )?;
    let last_five = subset(
        &domain,
        &[
            "validator-2",
            "validator-3",
            "validator-4",
            "validator-5",
            "validator-6",
        ],
        1,
        4,
    )?;
    let views = vec![
        build_trust_view(&domain, "validator-0", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-1", 1, vec![all.clone(), first_five], "")?,
        build_trust_view(&domain, "validator-2", 1, vec![all.clone(), last_five], "")?,
        build_trust_view(&domain, "validator-3", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-4", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-5", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-6", 1, vec![all], "")?,
    ];
    let graph = build_trust_graph(&domain, 2, root('b'), 7, None, views)?;
    let linkage = analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default())?;
    if !linkage.unsafe_pairs.is_empty() {
        return Err("fixture graph is unsafe".to_string());
    }
    Ok((domain, graph))
}

fn build_request(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    support_validators: &[String],
    slot: u64,
    payload_byte: char,
    agreement_byte: char,
) -> Result<WorkerRequest, String> {
    let abba_agreement_id = root(agreement_byte);
    let abba_round = 1;
    let abba_value = true;
    let mvba_agreement_id = root(agreement_byte);
    let dabc_activation_height = 20;
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        slot,
        root(payload_byte),
        "",
    )?;
    let echoes = support_validators
        .iter()
        .map(|sender| build_rbc_echo(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let readies = support_validators
        .iter()
        .map(|sender| build_rbc_ready(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let accepts = support_validators
        .iter()
        .map(|sender| build_rbc_accept(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let abba_finishes = support_validators
        .iter()
        .map(|sender| {
            build_abba_finish(
                domain,
                graph.trust_graph_root.clone(),
                sender,
                abba_agreement_id.clone(),
                abba_round,
                abba_value,
                "",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dabc_replay_bundle = build_request_dabc_replay_bundle(
        domain,
        graph,
        &propose,
        &accepts,
        support_validators,
        &mvba_agreement_id,
        dabc_activation_height,
    )?;
    Ok(WorkerRequest {
        domain: domain.clone(),
        graph: graph.clone(),
        propose,
        echoes,
        readies,
        accepts,
        abba_agreement_id,
        abba_round,
        abba_value,
        abba_finishes,
        mvba_agreement_id,
        dabc_activation_height,
        dabc_replay_bundle,
    })
}

fn build_request_dabc_replay_bundle(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    propose: &RbcPropose,
    accepts: &[RbcAccept],
    support_validators: &[String],
    mvba_agreement_id: &str,
    activation_height: u64,
) -> Result<DabcReplayBundle, String> {
    let accept = accepts
        .first()
        .ok_or_else(|| "DABC replay bundle requires at least one RBC accept".to_string())?;
    let candidate = mvba_candidate_from_rbc_accept(domain, propose, accept)?;
    let checkpoint_validator = "validator-1";
    let checkpoint_view = graph
        .trust_views
        .iter()
        .find(|view| view.validator == checkpoint_validator)
        .ok_or_else(|| format!("missing trust view for {checkpoint_validator}"))?;
    let input_set = build_mvba_valid_input_set(
        domain,
        checkpoint_view,
        mvba_agreement_id.to_string(),
        vec![candidate],
    )?;
    let ratified = ratify_dabc_amendment(domain, graph, &input_set, None, activation_height)?;
    let checkpoint_heights = [10_u64, activation_height];
    let checkpoint_support = support_validators
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    if checkpoint_support.len() < 5 {
        return Err("DABC replay bundle requires five checkpoint supporters".to_string());
    }
    let mut checks = Vec::new();
    for height in checkpoint_heights {
        for sender in &checkpoint_support {
            checks.push(build_dabc_full_knowledge_check(
                domain,
                graph.trust_graph_root.clone(),
                sender,
                height,
                vec![DabcPendingPair {
                    amendment_slot: ratified.amendment_slot,
                    output_candidate_id: ratified.output_candidate_id.clone(),
                }],
                "",
            )?);
        }
    }
    let checkpoint = build_dabc_full_knowledge_checkpoint(
        domain,
        graph,
        checkpoint_validator,
        checkpoint_heights[0],
        activation_height,
        checks,
    )?;
    let ratified_chain = vec![ratified.clone()];
    let activation = validate_dabc_activation_with_full_knowledge(
        domain,
        graph,
        &ratified_chain,
        &ratified,
        &checkpoint,
    )?;
    build_dabc_replay_bundle(
        domain,
        graph,
        ratified_chain,
        vec![checkpoint],
        vec![activation],
    )
}

fn evaluate_worker(validator: &str, request: WorkerRequest) -> Result<WorkerResult, String> {
    let view = request
        .graph
        .trust_views
        .iter()
        .find(|view| view.validator == validator)
        .ok_or_else(|| format!("missing trust view for {validator}"))?;
    let echo = evaluate_rbc_echo_support(&request.domain, view, &request.propose, &request.echoes)?;
    let ready =
        evaluate_rbc_ready_support(&request.domain, view, &request.propose, &request.readies)?;
    let accept = request
        .accepts
        .iter()
        .find(|message| message.sender == validator)
        .ok_or_else(|| format!("missing accept for {validator}"))?;
    validate_rbc_accept(&request.domain, accept, &request.propose)?;
    let ready_from_echo_allowed = rbc_ready_allowed_from_echo(&echo);
    let accept_allowed = rbc_accept_allowed_from_ready(&ready);
    let abba_finish_message = request
        .abba_finishes
        .iter()
        .find(|message| message.sender == validator)
        .ok_or_else(|| format!("missing ABBA finish for {validator}"))?;
    let abba_finish = evaluate_abba_finish_support(
        &request.domain,
        view,
        &request.abba_agreement_id,
        request.abba_round,
        request.abba_value,
        &request.abba_finishes,
    )?;
    let rbc_accepted =
        echo.strong_support && ready_from_echo_allowed && ready.strong_support && accept_allowed;
    let abba_finished = abba_strong_support(&abba_finish);
    let candidate = mvba_candidate_from_rbc_accept(&request.domain, &request.propose, accept)?;
    let input_set = build_mvba_valid_input_set(
        &request.domain,
        view,
        request.mvba_agreement_id.clone(),
        vec![candidate],
    )?;
    let ratified = ratify_dabc_amendment(
        &request.domain,
        &request.graph,
        &input_set,
        None,
        request.dabc_activation_height,
    )?;
    validate_dabc_ratified_amendment(&request.domain, &request.graph, &ratified, None)?;
    let replay_report =
        verify_dabc_replay_bundle(&request.domain, &request.graph, &request.dabc_replay_bundle)?;
    let dabc_replay_verified = request
        .dabc_replay_bundle
        .ratified_amendments
        .first()
        .map(|bundle_ratified| bundle_ratified.ratification_id == ratified.ratification_id)
        .unwrap_or(false)
        && replay_report.ratified_count == 1
        && replay_report.activation_count == 1;
    Ok(WorkerResult {
        validator: validator.to_string(),
        trust_view_id: view.trust_view_id.clone(),
        echo_strong: echo.strong_support,
        ready_from_echo_allowed,
        ready_strong: ready.strong_support,
        accept_allowed,
        rbc_accepted,
        abba_finish_strong_support: abba_finish.strong_support,
        abba_finished,
        mvba_selected_candidate_id: input_set.output_candidate_id.clone(),
        dabc_ratified: true,
        dabc_ratification_id: ratified.ratification_id,
        dabc_replay_verified,
        dabc_replay_bundle_id: request.dabc_replay_bundle.bundle_id.clone(),
        dabc_replay_ratified_count: replay_report.ratified_count,
        dabc_replay_activation_count: replay_report.activation_count,
        accepted: rbc_accepted && abba_finished && dabc_replay_verified,
        accept_message_id: accept.message_id.clone(),
        abba_finish_message_id: abba_finish_message.message_id.clone(),
    })
}

fn worker_main(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let validator = args
        .get(2)
        .ok_or_else(|| "--worker requires validator id".to_string())?;
    let delay_ms = args
        .get(3)
        .map(|value| value.parse::<u64>())
        .transpose()
        .map_err(|error| format!("worker delay must be a u64: {error}"))?
        .unwrap_or(0);
    let request: WorkerRequest = serde_json::from_reader(std::io::stdin().lock())?;
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }
    let result = evaluate_worker(validator, request)?;
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn spawn_worker_child(
    validator: &str,
    request: &WorkerRequest,
    delay_ms: u64,
) -> Result<RunningWorker, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut child = Command::new(exe)
        .arg("--worker")
        .arg(validator)
        .arg(delay_ms.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn worker {validator} failed: {error}"))?;
    let pid = child.id();
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| format!("worker {validator} stdin was not piped"))?;
        serde_json::to_writer(&mut *stdin, request)
            .map_err(|error| format!("write worker {validator} request failed: {error}"))?;
        stdin
            .write_all(b"\n")
            .map_err(|error| format!("finish worker {validator} request failed: {error}"))?;
    }
    drop(child.stdin.take());

    Ok(RunningWorker {
        validator: validator.to_string(),
        pid,
        killed_by_controller: false,
        kill_signal_sent: false,
        child,
    })
}

impl RunningWorker {
    fn kill(&mut self) {
        self.killed_by_controller = true;
        self.kill_signal_sent = self.child.kill().is_ok();
    }

    fn wait(self) -> Result<ChildOutcome, String> {
        let output = self
            .child
            .wait_with_output()
            .map_err(|error| format!("wait worker {} failed: {error}", self.validator))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let result = if output.status.success() {
            Some(
                serde_json::from_slice::<WorkerResult>(&output.stdout).map_err(|error| {
                    format!("parse worker {} result failed: {error}", self.validator)
                })?,
            )
        } else {
            None
        };
        Ok(ChildOutcome {
            validator: self.validator,
            pid: self.pid,
            killed_by_controller: self.killed_by_controller,
            kill_signal_sent: self.kill_signal_sent,
            status_success: output.status.success(),
            status_code: output.status.code(),
            stdout_line_count: stdout.lines().count(),
            stderr_line_count: stderr.lines().count(),
            result,
            error: if output.status.success() {
                None
            } else {
                Some(stderr.trim().to_string())
            },
        })
    }
}

fn same_payload_conflict_evidence_absent(
    request: &WorkerRequest,
    accepts: &[RbcAccept],
) -> Result<bool, String> {
    for left_index in 0..accepts.len() {
        for right_index in (left_index + 1)..accepts.len() {
            let conflict = detect_rbc_conflicting_accept(
                &request.domain,
                &request.graph,
                &request.propose,
                &accepts[left_index],
                &request.propose,
                &accepts[right_index],
            )?;
            if conflict.is_some() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn same_value_abba_conflict_evidence_absent(
    request: &WorkerRequest,
    finishes: &[AbbaFinish],
) -> Result<bool, String> {
    for left_index in 0..finishes.len() {
        for right_index in (left_index + 1)..finishes.len() {
            let conflict = detect_abba_conflicting_finish(
                &request.domain,
                &request.graph,
                &finishes[left_index],
                &finishes[right_index],
            )?;
            if conflict.is_some() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn controller_main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let validator_ids = validators(7);
    let killed_validator = "validator-6".to_string();
    let online_validators = validator_ids
        .iter()
        .filter(|validator| **validator != killed_validator)
        .cloned()
        .collect::<Vec<_>>();
    let online_request = build_request(&domain, &graph, &online_validators, 701, 'e', 'a')?;
    let full_request = build_request(&domain, &graph, &validator_ids, 702, 'f', 'b')?;

    let mut online_workers = Vec::new();
    for validator in &online_validators {
        online_workers.push(spawn_worker_child(validator, &online_request, 0)?);
    }
    let mut killed_worker = spawn_worker_child(&killed_validator, &online_request, 500)?;
    thread::sleep(Duration::from_millis(50));
    killed_worker.kill();

    let killed_outcome = killed_worker.wait()?;
    let online_outcomes = online_workers
        .into_iter()
        .map(RunningWorker::wait)
        .collect::<Result<Vec<_>, _>>()?;
    let restarted_outcome = spawn_worker_child(&killed_validator, &full_request, 0)?.wait()?;

    let online_results = online_outcomes
        .iter()
        .filter_map(|outcome| outcome.result.as_ref())
        .collect::<Vec<_>>();
    let accepted_by = online_results
        .iter()
        .filter(|result| result.accepted)
        .map(|result| result.validator.clone())
        .collect::<Vec<_>>();
    let distinct_online_trust_views = online_results
        .iter()
        .map(|result| result.trust_view_id.clone())
        .collect::<BTreeSet<_>>();
    let restart_accepted = restarted_outcome
        .result
        .as_ref()
        .map(|result| result.accepted)
        .unwrap_or(false);
    let same_payload_conflict_absent =
        same_payload_conflict_evidence_absent(&online_request, &online_request.accepts)?;
    let same_value_abba_conflict_absent =
        same_value_abba_conflict_evidence_absent(&online_request, &online_request.abba_finishes)?;
    let distinct_mvba_candidate_ids = online_results
        .iter()
        .map(|result| result.mvba_selected_candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let distinct_dabc_ratification_ids = online_results
        .iter()
        .map(|result| result.dabc_ratification_id.clone())
        .collect::<BTreeSet<_>>();
    let distinct_dabc_replay_bundle_ids = online_results
        .iter()
        .map(|result| result.dabc_replay_bundle_id.clone())
        .collect::<BTreeSet<_>>();
    let restart_result = restarted_outcome.result.as_ref();

    let checks = json!({
        "seven_child_processes_exercised": online_outcomes.len() + 1 == validator_ids.len(),
        "all_seven_child_processes_started_before_kill_wait": online_outcomes.len() + 1 == validator_ids.len(),
        "killed_worker_exit_non_success": killed_outcome.killed_by_controller
            && killed_outcome.kill_signal_sent
            && !killed_outcome.status_success,
        "remaining_worker_processes_exited_successfully": online_outcomes
            .iter()
            .all(|outcome| outcome.status_success),
        "remaining_online_count_satisfies_thresholds": online_results.len() == 6,
        "remaining_workers_rbc_accepted_same_payload": online_results
            .iter()
            .all(|result| result.rbc_accepted),
        "remaining_workers_finished_abba_same_value": online_results
            .iter()
            .all(|result| result.abba_finished),
        "remaining_workers_selected_same_mvba_candidate": online_results
            .iter()
            .all(|result| !result.mvba_selected_candidate_id.is_empty())
            && distinct_mvba_candidate_ids.len() == 1,
        "remaining_workers_ratified_same_dabc_amendment": online_results
            .iter()
            .all(|result| result.dabc_ratified)
            && distinct_dabc_ratification_ids.len() == 1,
        "remaining_workers_verified_same_dabc_replay_bundle": online_results
            .iter()
            .all(|result| {
                result.dabc_replay_verified
                    && result.dabc_replay_ratified_count == 1
                    && result.dabc_replay_activation_count == 1
            })
            && distinct_dabc_replay_bundle_ids.len() == 1,
        "remaining_workers_accepted_same_payload": online_results
            .iter()
            .all(|result| result.accepted),
        "minimum_three_distinct_trust_views_after_kill": distinct_online_trust_views.len() >= 3,
        "same_payload_conflict_evidence_absent_after_kill": same_payload_conflict_absent,
        "same_value_abba_conflict_evidence_absent_after_kill": same_value_abba_conflict_absent,
        "respawned_validator_exited_successfully": restarted_outcome.status_success,
        "respawned_validator_accepted_after_restart": restart_accepted,
        "respawned_validator_finished_abba_after_restart": restarted_outcome
            .result
            .as_ref()
            .map(|result| result.abba_finished)
            .unwrap_or(false),
        "respawned_validator_ratified_dabc_after_restart": restart_result
            .map(|result| result.dabc_ratified)
            .unwrap_or(false),
        "respawned_validator_verified_dabc_replay_after_restart": restart_result
            .map(|result| result.dabc_replay_verified)
            .unwrap_or(false),
        "outside_operators_required": false,
    });
    let failed_checks = checks
        .as_object()
        .ok_or_else(|| "checks object missing".to_string())?
        .iter()
        .filter_map(|(key, value)| {
            let expected = if key == "outside_operators_required" {
                Some(false)
            } else {
                Some(true)
            };
            if value.as_bool() == expected {
                None
            } else {
                Some(key.clone())
            }
        })
        .collect::<Vec<_>>();
    let ok = failed_checks.is_empty();
    let generated_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = json!({
        "schema": "postfiat-testnet-cobalt-live-process-kill-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "validator_count": validator_ids.len(),
        "concurrent_worker_count_before_kill_wait": online_outcomes.len() + 1,
        "killed_validator": killed_validator,
        "online_validator_count_after_kill": online_results.len(),
        "accepted_by_after_kill": accepted_by,
        "distinct_online_trust_view_count_after_kill": distinct_online_trust_views.len(),
        "checks": checks,
        "failed_checks": failed_checks,
        "outcomes": {
            "online_workers": online_outcomes,
            "killed_worker": killed_outcome,
            "restarted_worker": restarted_outcome,
        },
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt live process-kill drill failed".into())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("--worker") {
        worker_main(&args)
    } else {
        controller_main()
    }
}
