use std::io::{BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_rbc_accept, build_rbc_echo,
    build_rbc_propose, build_rbc_ready, build_trust_graph, build_trust_view,
    detect_rbc_conflicting_accept, evaluate_rbc_echo_support, evaluate_rbc_ready_support,
    rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo, validate_rbc_accept, CobaltDomain,
    CobaltFaultModel, EssentialSubset, RbcAccept, RbcEcho, RbcPropose, RbcReady, TrustGraph,
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
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkerResult {
    validator: String,
    trust_view_id: String,
    echo_strong: bool,
    ready_from_echo_allowed: bool,
    ready_strong: bool,
    accept_allowed: bool,
    accepted: bool,
    accept_message_id: String,
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
    );
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
    );
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
    );
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
    Ok(WorkerResult {
        validator: validator.to_string(),
        trust_view_id: view.trust_view_id.clone(),
        echo_strong: echo.strong_support,
        ready_from_echo_allowed,
        ready_strong: ready.strong_support,
        accept_allowed,
        accepted: echo.strong_support
            && ready_from_echo_allowed
            && ready.strong_support
            && accept_allowed,
        accept_message_id: accept.message_id.clone(),
    })
}

fn worker(validator: String, listener: TcpListener) -> Result<(), String> {
    let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
    let request: WorkerRequest =
        serde_json::from_reader(BufReader::new(&stream)).map_err(|error| error.to_string())?;
    let result = evaluate_worker(&validator, request)?;
    serde_json::to_writer(&mut stream, &result).map_err(|error| error.to_string())?;
    stream.write_all(b"\n").map_err(|error| error.to_string())?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let validator_ids = validators(7);
    let propose = build_rbc_propose(
        &domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        401,
        root('f'),
        "",
    )?;
    let echoes = validator_ids
        .iter()
        .map(|sender| build_rbc_echo(&domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let readies = validator_ids
        .iter()
        .map(|sender| build_rbc_ready(&domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let accepts = validator_ids
        .iter()
        .map(|sender| build_rbc_accept(&domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let request = WorkerRequest {
        domain,
        graph,
        propose,
        echoes,
        readies,
        accepts,
    };

    let mut handles = Vec::new();
    let mut endpoints = Vec::new();
    for validator in &validator_ids {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        endpoints.push((validator.clone(), listener.local_addr()?));
        let validator = validator.clone();
        handles.push(thread::spawn(move || worker(validator, listener)));
    }

    let mut results = Vec::new();
    for (validator, address) in &endpoints {
        let mut stream = TcpStream::connect(address)?;
        serde_json::to_writer(&mut stream, &request)?;
        stream.shutdown(Shutdown::Write)?;
        let result: WorkerResult = serde_json::from_reader(BufReader::new(stream))?;
        if result.validator != *validator {
            return Err(format!("worker identity mismatch for {validator}").into());
        }
        results.push(result);
    }
    for handle in handles {
        handle
            .join()
            .map_err(|_| "worker thread panicked")?
            .map_err(|error| format!("worker failed: {error}"))?;
    }
    results.sort_by(|left, right| left.validator.cmp(&right.validator));
    let distinct_trust_views = results
        .iter()
        .map(|result| result.trust_view_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let accepted_by = results
        .iter()
        .filter(|result| result.accepted)
        .map(|result| result.validator.clone())
        .collect::<Vec<_>>();
    let mut same_payload_conflict_evidence_absent = true;
    for left_index in 0..request.accepts.len() {
        for right_index in (left_index + 1)..request.accepts.len() {
            let conflict = detect_rbc_conflicting_accept(
                &request.domain,
                &request.graph,
                &request.propose,
                &request.accepts[left_index],
                &request.propose,
                &request.accepts[right_index],
            )?;
            if conflict.is_some() {
                same_payload_conflict_evidence_absent = false;
            }
        }
    }
    let generated_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = json!({
        "schema": "postfiat-testnet-cobalt-rbc-nonuniform-tcp-drill-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "transport": "tcp-loopback",
        "validator_count": validator_ids.len(),
        "distinct_trust_view_count": distinct_trust_views.len(),
        "accepted_by": accepted_by,
        "checks": {
            "seven_validator_tcp_workers_ok": validator_ids.len() == 7,
            "loopback_tcp_transport_ok": results.len() == validator_ids.len(),
            "minimum_three_distinct_trust_views_ok": distinct_trust_views.len() >= 3,
            "all_workers_echo_strong": results.iter().all(|result| result.echo_strong),
            "all_workers_ready_from_echo_allowed": results.iter().all(|result| result.ready_from_echo_allowed),
            "all_workers_ready_strong": results.iter().all(|result| result.ready_strong),
            "all_workers_accept_allowed": results.iter().all(|result| result.accept_allowed),
            "all_workers_accepted_same_payload": results.iter().all(|result| result.accepted),
            "same_payload_conflict_evidence_absent": same_payload_conflict_evidence_absent,
        },
        "results": results,
    });
    let ok = report["checks"]
        .as_object()
        .expect("checks object")
        .values()
        .all(|value| value.as_bool() == Some(true));
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("RBC non-uniform TCP drill failed".into())
    }
}
