pub fn build_rbc_propose(
    domain: &CobaltDomain,
    trust_graph_root: impl Into<String>,
    sender: impl Into<String>,
    amendment_slot: u64,
    payload_hash: impl Into<String>,
    signature_hex: impl Into<String>,
) -> Result<RbcPropose, String> {
    validate_domain(domain)?;
    let mut message = RbcPropose {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: trust_graph_root.into(),
        sender: sender.into(),
        amendment_slot,
        payload_hash: payload_hash.into(),
        signature_hex: signature_hex.into(),
    };
    message.message_id = rbc_propose_message_id(&message)?;
    validate_rbc_propose(domain, &message)?;
    Ok(message)
}

pub fn build_rbc_echo(
    domain: &CobaltDomain,
    propose: &RbcPropose,
    sender: impl Into<String>,
    signature_hex: impl Into<String>,
) -> Result<RbcEcho, String> {
    validate_rbc_propose(domain, propose)?;
    let mut message = RbcEcho {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: propose.trust_graph_root.clone(),
        sender: sender.into(),
        proposer: propose.sender.clone(),
        amendment_slot: propose.amendment_slot,
        payload_hash: propose.payload_hash.clone(),
        propose_message_id: propose.message_id.clone(),
        signature_hex: signature_hex.into(),
    };
    message.message_id = rbc_echo_message_id(&message)?;
    validate_rbc_echo(domain, &message, propose)?;
    Ok(message)
}

pub fn build_rbc_ready(
    domain: &CobaltDomain,
    propose: &RbcPropose,
    sender: impl Into<String>,
    signature_hex: impl Into<String>,
) -> Result<RbcReady, String> {
    validate_rbc_propose(domain, propose)?;
    let mut message = RbcReady {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: propose.trust_graph_root.clone(),
        sender: sender.into(),
        proposer: propose.sender.clone(),
        amendment_slot: propose.amendment_slot,
        payload_hash: propose.payload_hash.clone(),
        propose_message_id: propose.message_id.clone(),
        signature_hex: signature_hex.into(),
    };
    message.message_id = rbc_ready_message_id(&message)?;
    validate_rbc_ready(domain, &message, propose)?;
    Ok(message)
}

pub fn build_rbc_accept(
    domain: &CobaltDomain,
    propose: &RbcPropose,
    sender: impl Into<String>,
    signature_hex: impl Into<String>,
) -> Result<RbcAccept, String> {
    validate_rbc_propose(domain, propose)?;
    let mut message = RbcAccept {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: propose.trust_graph_root.clone(),
        sender: sender.into(),
        proposer: propose.sender.clone(),
        amendment_slot: propose.amendment_slot,
        payload_hash: propose.payload_hash.clone(),
        propose_message_id: propose.message_id.clone(),
        signature_hex: signature_hex.into(),
    };
    message.message_id = rbc_accept_message_id(&message)?;
    validate_rbc_accept(domain, &message, propose)?;
    Ok(message)
}

pub fn validate_rbc_propose(domain: &CobaltDomain, message: &RbcPropose) -> Result<(), String> {
    validate_rbc_domain(
        domain,
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
    )?;
    validate_hash_hex("RBC trust graph root", &message.trust_graph_root)?;
    validate_node_id("RBC sender", &message.sender)?;
    validate_hash_hex("RBC payload hash", &message.payload_hash)?;
    validate_rbc_signature_hex(&message.signature_hex)?;
    let expected_id = rbc_propose_message_id(message)?;
    if message.message_id != expected_id {
        return Err("RBC propose message id mismatch".to_string());
    }
    Ok(())
}

pub fn validate_rbc_echo(
    domain: &CobaltDomain,
    message: &RbcEcho,
    propose: &RbcPropose,
) -> Result<(), String> {
    validate_rbc_linked_message(
        domain,
        "RBC echo",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.proposer,
        message.amendment_slot,
        &message.payload_hash,
        &message.propose_message_id,
        &message.signature_hex,
        propose,
    )?;
    let expected_id = rbc_echo_message_id(message)?;
    if message.message_id != expected_id {
        return Err("RBC echo message id mismatch".to_string());
    }
    Ok(())
}

pub fn validate_rbc_ready(
    domain: &CobaltDomain,
    message: &RbcReady,
    propose: &RbcPropose,
) -> Result<(), String> {
    validate_rbc_linked_message(
        domain,
        "RBC ready",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.proposer,
        message.amendment_slot,
        &message.payload_hash,
        &message.propose_message_id,
        &message.signature_hex,
        propose,
    )?;
    let expected_id = rbc_ready_message_id(message)?;
    if message.message_id != expected_id {
        return Err("RBC ready message id mismatch".to_string());
    }
    Ok(())
}

pub fn validate_rbc_accept(
    domain: &CobaltDomain,
    message: &RbcAccept,
    propose: &RbcPropose,
) -> Result<(), String> {
    validate_rbc_linked_message(
        domain,
        "RBC accept",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.proposer,
        message.amendment_slot,
        &message.payload_hash,
        &message.propose_message_id,
        &message.signature_hex,
        propose,
    )?;
    let expected_id = rbc_accept_message_id(message)?;
    if message.message_id != expected_id {
        return Err("RBC accept message id mismatch".to_string());
    }
    Ok(())
}

pub fn rbc_propose_signing_payload_bytes(message: &RbcPropose) -> Result<Vec<u8>, String> {
    rbc_signing_payload_bytes(
        "propose",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        None,
        message.amendment_slot,
        &message.payload_hash,
        None,
    )
}

pub fn rbc_echo_signing_payload_bytes(message: &RbcEcho) -> Result<Vec<u8>, String> {
    rbc_signing_payload_bytes(
        "echo",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        Some(&message.proposer),
        message.amendment_slot,
        &message.payload_hash,
        Some(&message.propose_message_id),
    )
}

pub fn rbc_ready_signing_payload_bytes(message: &RbcReady) -> Result<Vec<u8>, String> {
    rbc_signing_payload_bytes(
        "ready",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        Some(&message.proposer),
        message.amendment_slot,
        &message.payload_hash,
        Some(&message.propose_message_id),
    )
}

pub fn rbc_accept_signing_payload_bytes(message: &RbcAccept) -> Result<Vec<u8>, String> {
    rbc_signing_payload_bytes(
        "accept",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        Some(&message.proposer),
        message.amendment_slot,
        &message.payload_hash,
        Some(&message.propose_message_id),
    )
}

pub fn rbc_propose_message_id(message: &RbcPropose) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.rbc.propose.v1",
        &rbc_propose_signing_payload_bytes(message)?,
    ))
}

pub fn rbc_echo_message_id(message: &RbcEcho) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.rbc.echo.v1",
        &rbc_echo_signing_payload_bytes(message)?,
    ))
}

pub fn rbc_ready_message_id(message: &RbcReady) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.rbc.ready.v1",
        &rbc_ready_signing_payload_bytes(message)?,
    ))
}

pub fn rbc_accept_message_id(message: &RbcAccept) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.rbc.accept.v1",
        &rbc_accept_signing_payload_bytes(message)?,
    ))
}

pub fn evaluate_rbc_echo_support(
    domain: &CobaltDomain,
    view: &TrustView,
    propose: &RbcPropose,
    echoes: &[RbcEcho],
) -> Result<RbcSupportEvaluation, String> {
    validate_trust_view(domain, view)?;
    validate_rbc_propose(domain, propose)?;
    let mut support = Vec::with_capacity(echoes.len());
    for echo in echoes {
        validate_rbc_echo(domain, echo, propose)?;
        support.push(echo.sender.clone());
    }
    evaluate_rbc_support(view, "echo", propose, support)
}

pub fn evaluate_rbc_ready_support(
    domain: &CobaltDomain,
    view: &TrustView,
    propose: &RbcPropose,
    readies: &[RbcReady],
) -> Result<RbcSupportEvaluation, String> {
    validate_trust_view(domain, view)?;
    validate_rbc_propose(domain, propose)?;
    let mut support = Vec::with_capacity(readies.len());
    for ready in readies {
        validate_rbc_ready(domain, ready, propose)?;
        support.push(ready.sender.clone());
    }
    evaluate_rbc_support(view, "ready", propose, support)
}

pub fn rbc_ready_allowed_from_echo(evaluation: &RbcSupportEvaluation) -> bool {
    evaluation.message_kind == "echo" && evaluation.strong_support
}

pub fn rbc_ready_allowed_from_ready(evaluation: &RbcSupportEvaluation) -> bool {
    evaluation.message_kind == "ready" && evaluation.weak_support
}

pub fn rbc_accept_allowed_from_ready(evaluation: &RbcSupportEvaluation) -> bool {
    evaluation.message_kind == "ready" && evaluation.strong_support
}

pub fn detect_rbc_conflicting_accept(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    left_propose: &RbcPropose,
    left_accept: &RbcAccept,
    right_propose: &RbcPropose,
    right_accept: &RbcAccept,
) -> Result<Option<RbcConflictingAcceptEvidence>, String> {
    validate_trust_graph(domain, graph)?;
    validate_rbc_propose(domain, left_propose)?;
    validate_rbc_propose(domain, right_propose)?;
    validate_rbc_accept(domain, left_accept, left_propose)?;
    validate_rbc_accept(domain, right_accept, right_propose)?;
    if left_accept.trust_graph_root != graph.trust_graph_root
        || right_accept.trust_graph_root != graph.trust_graph_root
    {
        return Err("RBC conflicting accept evidence trust graph root mismatch".to_string());
    }
    if left_accept.amendment_slot != right_accept.amendment_slot
        || left_accept.proposer != right_accept.proposer
    {
        return Ok(None);
    }
    if left_accept.payload_hash == right_accept.payload_hash
        && left_accept.propose_message_id == right_accept.propose_message_id
    {
        return Ok(None);
    }
    if left_accept.sender == right_accept.sender {
        return Err("RBC conflicting accept evidence requires two validators".to_string());
    }
    let left_view = trust_view_for_validator(graph, &left_accept.sender)?;
    let right_view = trust_view_for_validator(graph, &right_accept.sender)?;
    let actively_byzantine = BTreeSet::new();
    let linked = linked_shared_subset(left_view, right_view, &actively_byzantine).is_some();
    if !linked {
        return Ok(None);
    }
    let fully_linked =
        fully_linked_shared_subset(left_view, right_view, &actively_byzantine).is_some();
    let (
        left_sender,
        right_sender,
        left_payload_hash,
        right_payload_hash,
        left_propose_id,
        right_propose_id,
    ) = if left_accept.sender <= right_accept.sender {
        (
            left_accept.sender.clone(),
            right_accept.sender.clone(),
            left_accept.payload_hash.clone(),
            right_accept.payload_hash.clone(),
            left_accept.propose_message_id.clone(),
            right_accept.propose_message_id.clone(),
        )
    } else {
        (
            right_accept.sender.clone(),
            left_accept.sender.clone(),
            right_accept.payload_hash.clone(),
            left_accept.payload_hash.clone(),
            right_accept.propose_message_id.clone(),
            left_accept.propose_message_id.clone(),
        )
    };
    let mut evidence = RbcConflictingAcceptEvidence {
        evidence_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: graph.trust_graph_root.clone(),
        amendment_slot: left_accept.amendment_slot,
        proposer: left_accept.proposer.clone(),
        left_sender,
        right_sender,
        left_payload_hash,
        right_payload_hash,
        left_propose_message_id: left_propose_id,
        right_propose_message_id: right_propose_id,
        linked,
        fully_linked,
        reason: "linked validators accepted conflicting RBC payloads".to_string(),
    };
    evidence.evidence_id = rbc_conflicting_accept_evidence_id(domain, &evidence)?;
    Ok(Some(evidence))
}

pub fn build_abba_init(
    domain: &CobaltDomain,
    trust_graph_root: impl Into<String>,
    sender: impl Into<String>,
    agreement_id: impl Into<String>,
    round: u64,
    value: bool,
    signature_hex: impl Into<String>,
) -> Result<AbbaInit, String> {
    let mut message = AbbaInit {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: trust_graph_root.into(),
        sender: sender.into(),
        agreement_id: agreement_id.into(),
        round,
        value,
        signature_hex: signature_hex.into(),
    };
    message.message_id = abba_init_message_id(&message)?;
    validate_abba_init(domain, &message)?;
    Ok(message)
}

pub fn build_abba_aux(
    domain: &CobaltDomain,
    trust_graph_root: impl Into<String>,
    sender: impl Into<String>,
    agreement_id: impl Into<String>,
    round: u64,
    value: bool,
    signature_hex: impl Into<String>,
) -> Result<AbbaAux, String> {
    let mut message = AbbaAux {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: trust_graph_root.into(),
        sender: sender.into(),
        agreement_id: agreement_id.into(),
        round,
        value,
        signature_hex: signature_hex.into(),
    };
    message.message_id = abba_aux_message_id(&message)?;
    validate_abba_aux(domain, &message)?;
    Ok(message)
}

pub fn build_abba_conf(
    domain: &CobaltDomain,
    trust_graph_root: impl Into<String>,
    sender: impl Into<String>,
    agreement_id: impl Into<String>,
    round: u64,
    value: bool,
    signature_hex: impl Into<String>,
) -> Result<AbbaConf, String> {
    let mut message = AbbaConf {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: trust_graph_root.into(),
        sender: sender.into(),
        agreement_id: agreement_id.into(),
        round,
        value,
        signature_hex: signature_hex.into(),
    };
    message.message_id = abba_conf_message_id(&message)?;
    validate_abba_conf(domain, &message)?;
    Ok(message)
}

pub fn build_abba_finish(
    domain: &CobaltDomain,
    trust_graph_root: impl Into<String>,
    sender: impl Into<String>,
    agreement_id: impl Into<String>,
    round: u64,
    value: bool,
    signature_hex: impl Into<String>,
) -> Result<AbbaFinish, String> {
    let mut message = AbbaFinish {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: trust_graph_root.into(),
        sender: sender.into(),
        agreement_id: agreement_id.into(),
        round,
        value,
        signature_hex: signature_hex.into(),
    };
    message.message_id = abba_finish_message_id(&message)?;
    validate_abba_finish(domain, &message)?;
    Ok(message)
}

pub fn build_abba_round_state(
    trust_graph_root: impl Into<String>,
    agreement_id: impl Into<String>,
    round: u64,
) -> Result<AbbaRoundState, String> {
    let state = AbbaRoundState {
        trust_graph_root: trust_graph_root.into(),
        agreement_id: agreement_id.into(),
        round,
        init_messages: Vec::new(),
        aux_messages: Vec::new(),
        conf_messages: Vec::new(),
        finish_messages: Vec::new(),
    };
    validate_hash_hex("ABBA trust graph root", &state.trust_graph_root)?;
    validate_hash_hex("ABBA agreement id", &state.agreement_id)?;
    if state.round == 0 {
        return Err("ABBA round must be nonzero".to_string());
    }
    Ok(state)
}

pub fn validate_abba_init(domain: &CobaltDomain, message: &AbbaInit) -> Result<(), String> {
    validate_abba_message(
        domain,
        "init",
        &message.message_id,
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        &message.signature_hex,
        &abba_init_message_id(message)?,
    )
}

pub fn validate_abba_aux(domain: &CobaltDomain, message: &AbbaAux) -> Result<(), String> {
    validate_abba_message(
        domain,
        "aux",
        &message.message_id,
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        &message.signature_hex,
        &abba_aux_message_id(message)?,
    )
}

pub fn validate_abba_conf(domain: &CobaltDomain, message: &AbbaConf) -> Result<(), String> {
    validate_abba_message(
        domain,
        "conf",
        &message.message_id,
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        &message.signature_hex,
        &abba_conf_message_id(message)?,
    )
}

pub fn validate_abba_finish(domain: &CobaltDomain, message: &AbbaFinish) -> Result<(), String> {
    validate_abba_message(
        domain,
        "finish",
        &message.message_id,
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        &message.signature_hex,
        &abba_finish_message_id(message)?,
    )
}

pub fn abba_init_signing_payload_bytes(message: &AbbaInit) -> Result<Vec<u8>, String> {
    abba_signing_payload_bytes(
        "init",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        message.value,
    )
}

pub fn abba_aux_signing_payload_bytes(message: &AbbaAux) -> Result<Vec<u8>, String> {
    abba_signing_payload_bytes(
        "aux",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        message.value,
    )
}

pub fn abba_conf_signing_payload_bytes(message: &AbbaConf) -> Result<Vec<u8>, String> {
    abba_signing_payload_bytes(
        "conf",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        message.value,
    )
}

pub fn abba_finish_signing_payload_bytes(message: &AbbaFinish) -> Result<Vec<u8>, String> {
    abba_signing_payload_bytes(
        "finish",
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.trust_graph_root,
        &message.sender,
        &message.agreement_id,
        message.round,
        message.value,
    )
}

pub fn abba_init_message_id(message: &AbbaInit) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.abba.init.v1",
        &abba_init_signing_payload_bytes(message)?,
    ))
}

pub fn abba_aux_message_id(message: &AbbaAux) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.abba.aux.v1",
        &abba_aux_signing_payload_bytes(message)?,
    ))
}

pub fn abba_conf_message_id(message: &AbbaConf) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.abba.conf.v1",
        &abba_conf_signing_payload_bytes(message)?,
    ))
}

pub fn abba_finish_message_id(message: &AbbaFinish) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.abba.finish.v1",
        &abba_finish_signing_payload_bytes(message)?,
    ))
}

pub fn evaluate_abba_aux_support(
    domain: &CobaltDomain,
    view: &TrustView,
    agreement_id: &str,
    round: u64,
    value: bool,
    messages: &[AbbaAux],
) -> Result<AbbaSupportEvaluation, String> {
    validate_trust_view(domain, view)?;
    let mut support = Vec::with_capacity(messages.len());
    let mut relevant_candidates = Vec::with_capacity(messages.len());
    for message in messages {
        validate_abba_aux(domain, message)?;
        if message.agreement_id == agreement_id && message.round == round {
            relevant_candidates.push(abba_aux_equivocation_candidate(message));
            if message.value == value {
                support.push(message.sender.clone());
            }
        }
    }
    let equivocal_senders = abba_equivocating_senders(&relevant_candidates);
    support.retain(|sender| !equivocal_senders.contains(sender));
    evaluate_abba_support(view, "aux", agreement_id, round, value, support)
}

pub fn evaluate_abba_conf_support(
    domain: &CobaltDomain,
    view: &TrustView,
    agreement_id: &str,
    round: u64,
    value: bool,
    messages: &[AbbaConf],
) -> Result<AbbaSupportEvaluation, String> {
    validate_trust_view(domain, view)?;
    let mut support = Vec::with_capacity(messages.len());
    let mut relevant_candidates = Vec::with_capacity(messages.len());
    for message in messages {
        validate_abba_conf(domain, message)?;
        if message.agreement_id == agreement_id && message.round == round {
            relevant_candidates.push(abba_conf_equivocation_candidate(message));
            if message.value == value {
                support.push(message.sender.clone());
            }
        }
    }
    let equivocal_senders = abba_equivocating_senders(&relevant_candidates);
    support.retain(|sender| !equivocal_senders.contains(sender));
    evaluate_abba_support(view, "conf", agreement_id, round, value, support)
}

pub fn evaluate_abba_finish_support(
    domain: &CobaltDomain,
    view: &TrustView,
    agreement_id: &str,
    round: u64,
    value: bool,
    messages: &[AbbaFinish],
) -> Result<AbbaSupportEvaluation, String> {
    validate_trust_view(domain, view)?;
    let mut support = Vec::with_capacity(messages.len());
    let mut relevant_candidates = Vec::with_capacity(messages.len());
    for message in messages {
        validate_abba_finish(domain, message)?;
        if message.agreement_id == agreement_id && message.round == round {
            relevant_candidates.push(abba_finish_equivocation_candidate(message));
            if message.value == value {
                support.push(message.sender.clone());
            }
        }
    }
    let equivocal_senders = abba_equivocating_senders(&relevant_candidates);
    support.retain(|sender| !equivocal_senders.contains(sender));
    evaluate_abba_support(view, "finish", agreement_id, round, value, support)
}

pub fn abba_strong_support(evaluation: &AbbaSupportEvaluation) -> bool {
    evaluation.strong_support
}

pub fn abba_weak_support(evaluation: &AbbaSupportEvaluation) -> bool {
    evaluation.weak_support
}

pub fn detect_abba_conflicting_finish(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    left: &AbbaFinish,
    right: &AbbaFinish,
) -> Result<Option<AbbaConflictingFinishEvidence>, String> {
    validate_trust_graph(domain, graph)?;
    validate_abba_finish(domain, left)?;
    validate_abba_finish(domain, right)?;
    if left.trust_graph_root != graph.trust_graph_root
        || right.trust_graph_root != graph.trust_graph_root
    {
        return Err("ABBA conflicting finish evidence trust graph root mismatch".to_string());
    }
    if left.agreement_id != right.agreement_id || left.round != right.round {
        return Ok(None);
    }
    if left.value == right.value {
        return Ok(None);
    }
    if left.sender == right.sender {
        return Err("ABBA conflicting finish evidence requires two validators".to_string());
    }
    let left_view = trust_view_for_validator(graph, &left.sender)?;
    let right_view = trust_view_for_validator(graph, &right.sender)?;
    let actively_byzantine = BTreeSet::new();
    let linked = linked_shared_subset(left_view, right_view, &actively_byzantine).is_some();
    if !linked {
        return Ok(None);
    }
    let fully_linked =
        fully_linked_shared_subset(left_view, right_view, &actively_byzantine).is_some();
    let (left_sender, right_sender, left_value, right_value) = if left.sender <= right.sender {
        (
            left.sender.clone(),
            right.sender.clone(),
            left.value,
            right.value,
        )
    } else {
        (
            right.sender.clone(),
            left.sender.clone(),
            right.value,
            left.value,
        )
    };
    let mut evidence = AbbaConflictingFinishEvidence {
        evidence_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: graph.trust_graph_root.clone(),
        agreement_id: left.agreement_id.clone(),
        round: left.round,
        left_sender,
        right_sender,
        left_value,
        right_value,
        linked,
        fully_linked,
        reason: "linked validators finished conflicting ABBA values".to_string(),
    };
    evidence.evidence_id = abba_conflicting_finish_evidence_id(domain, &evidence)?;
    Ok(Some(evidence))
}

pub fn detect_abba_init_equivocation(
    domain: &CobaltDomain,
    left: &AbbaInit,
    right: &AbbaInit,
) -> Result<Option<AbbaEquivocationEvidence>, String> {
    validate_abba_init(domain, left)?;
    validate_abba_init(domain, right)?;
    detect_abba_equivocation_candidates(
        domain,
        abba_init_equivocation_candidate(left),
        abba_init_equivocation_candidate(right),
    )
}

pub fn detect_abba_aux_equivocation(
    domain: &CobaltDomain,
    left: &AbbaAux,
    right: &AbbaAux,
) -> Result<Option<AbbaEquivocationEvidence>, String> {
    validate_abba_aux(domain, left)?;
    validate_abba_aux(domain, right)?;
    detect_abba_equivocation_candidates(
        domain,
        abba_aux_equivocation_candidate(left),
        abba_aux_equivocation_candidate(right),
    )
}

pub fn detect_abba_conf_equivocation(
    domain: &CobaltDomain,
    left: &AbbaConf,
    right: &AbbaConf,
) -> Result<Option<AbbaEquivocationEvidence>, String> {
    validate_abba_conf(domain, left)?;
    validate_abba_conf(domain, right)?;
    detect_abba_equivocation_candidates(
        domain,
        abba_conf_equivocation_candidate(left),
        abba_conf_equivocation_candidate(right),
    )
}

pub fn detect_abba_finish_equivocation(
    domain: &CobaltDomain,
    left: &AbbaFinish,
    right: &AbbaFinish,
) -> Result<Option<AbbaEquivocationEvidence>, String> {
    validate_abba_finish(domain, left)?;
    validate_abba_finish(domain, right)?;
    detect_abba_equivocation_candidates(
        domain,
        abba_finish_equivocation_candidate(left),
        abba_finish_equivocation_candidate(right),
    )
}

pub fn detect_abba_round_equivocations(
    domain: &CobaltDomain,
    state: &AbbaRoundState,
) -> Result<Vec<AbbaEquivocationEvidence>, String> {
    validate_domain(domain)?;
    validate_hash_hex("ABBA trust graph root", &state.trust_graph_root)?;
    validate_hash_hex("ABBA agreement id", &state.agreement_id)?;
    if state.round == 0 {
        return Err("ABBA round must be nonzero".to_string());
    }
    for message in &state.init_messages {
        validate_abba_init(domain, message)?;
    }
    for message in &state.aux_messages {
        validate_abba_aux(domain, message)?;
    }
    for message in &state.conf_messages {
        validate_abba_conf(domain, message)?;
    }
    for message in &state.finish_messages {
        validate_abba_finish(domain, message)?;
    }
    let mut evidence_by_id = BTreeMap::new();
    collect_abba_equivocations(
        domain,
        state,
        state
            .init_messages
            .iter()
            .map(abba_init_equivocation_candidate),
        &mut evidence_by_id,
    )?;
    collect_abba_equivocations(
        domain,
        state,
        state
            .aux_messages
            .iter()
            .map(abba_aux_equivocation_candidate),
        &mut evidence_by_id,
    )?;
    collect_abba_equivocations(
        domain,
        state,
        state
            .conf_messages
            .iter()
            .map(abba_conf_equivocation_candidate),
        &mut evidence_by_id,
    )?;
    collect_abba_equivocations(
        domain,
        state,
        state
            .finish_messages
            .iter()
            .map(abba_finish_equivocation_candidate),
        &mut evidence_by_id,
    )?;
    Ok(evidence_by_id.into_values().collect())
}

pub fn abba_common_coin(
    domain: &CobaltDomain,
    agreement_id: &str,
    round: u64,
    source: &AbbaCommonRandomSource,
    mode: CobaltRuntimeMode,
) -> Result<bool, String> {
    validate_domain(domain)?;
    validate_hash_hex("ABBA agreement id", agreement_id)?;
    if round == 0 {
        return Err("ABBA round must be nonzero".to_string());
    }
    match (source, mode) {
        (AbbaCommonRandomSource::DeterministicTest { .. }, CobaltRuntimeMode::Live) => {
            Err("deterministic ABBA test CRS cannot be used in live mode".to_string())
        }
        (AbbaCommonRandomSource::DeterministicTest { seed_hex }, CobaltRuntimeMode::Simulation) => {
            validate_lower_hex_len("ABBA deterministic test CRS seed", seed_hex, 64)?;
            let coin_hash = hash_hex(
                "postfiat.cobalt.abba.common_coin.deterministic_test.v1",
                format!(
                    "chain_id={}\ngenesis_hash={}\nprotocol_version={}\nagreement_id={agreement_id}\nround={round}\nseed_hex={seed_hex}\n",
                    domain.chain_id, domain.genesis_hash, domain.protocol_version
                )
                .as_bytes(),
            );
            Ok(hex_low_bit(&coin_hash)?)
        }
        (
            AbbaCommonRandomSource::SignedBeacon {
                beacon_id,
                output_hash,
            },
            _,
        ) => {
            validate_hash_hex("ABBA signed beacon id", beacon_id)?;
            validate_hash_hex("ABBA signed beacon output hash", output_hash)?;
            Ok(hex_low_bit(output_hash)?)
        }
    }
}

pub fn mvba_candidate_from_rbc_accept(
    domain: &CobaltDomain,
    propose: &RbcPropose,
    accept: &RbcAccept,
) -> Result<MvbaCandidate, String> {
    validate_rbc_propose(domain, propose)?;
    validate_rbc_accept(domain, accept, propose)?;
    let mut candidate = MvbaCandidate {
        candidate_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: propose.trust_graph_root.clone(),
        amendment_slot: propose.amendment_slot,
        proposer: propose.sender.clone(),
        payload_hash: propose.payload_hash.clone(),
        propose_message_id: propose.message_id.clone(),
    };
    candidate.candidate_id = mvba_candidate_id(domain, &candidate)?;
    Ok(candidate)
}

pub fn build_mvba_valid_input_set(
    domain: &CobaltDomain,
    view: &TrustView,
    agreement_id: impl Into<String>,
    candidates: Vec<MvbaCandidate>,
) -> Result<MvbaValidInputSet, String> {
    validate_trust_view(domain, view)?;
    let agreement_id = agreement_id.into();
    validate_hash_hex("MVBA agreement id", &agreement_id)?;
    let mut candidates = candidates;
    if candidates.len() > MAX_MVBA_CANDIDATES_PER_SET {
        return Err("MVBA valid input set has too many candidates".to_string());
    }
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    candidates.dedup_by(|left, right| left.candidate_id == right.candidate_id);
    if candidates.is_empty() {
        return Err("MVBA valid input set requires at least one candidate".to_string());
    }
    for candidate in &candidates {
        validate_mvba_candidate(domain, candidate)?;
    }
    let output_candidate_id = candidates[0].candidate_id.clone();
    Ok(MvbaValidInputSet {
        trust_view_id: view.trust_view_id.clone(),
        local_validator: view.validator.clone(),
        agreement_id,
        candidates,
        output_candidate_id,
    })
}

pub fn mvba_output_candidate(input_set: &MvbaValidInputSet) -> Result<&MvbaCandidate, String> {
    input_set
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == input_set.output_candidate_id)
        .ok_or_else(|| "MVBA output candidate id is not in valid input set".to_string())
}

pub fn mvba_candidate_id(
    domain: &CobaltDomain,
    candidate: &MvbaCandidate,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        candidate.trust_graph_root.as_str(),
        candidate.amendment_slot,
        candidate.proposer.as_str(),
        candidate.payload_hash.as_str(),
        candidate.propose_message_id.as_str(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.mvba.candidate.v1", &encoded))
}
