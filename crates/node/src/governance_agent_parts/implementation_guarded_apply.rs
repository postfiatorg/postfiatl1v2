pub fn governance_agent_implementation_execution(
    options: GovernanceAgentImplementationExecutionOptions,
) -> io::Result<GovernanceAgentImplementationExecutionReport> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent implementation execution report",
    )?;
    let work_item_value =
        read_governance_agent_json_value(&options.work_item_file, "governance work item")?;
    let work_item: GovernanceAgentImplementationWorkItem =
        serde_json::from_value(work_item_value.clone()).map_err(invalid_data)?;
    let validation = validate_governance_agent_implementation_work_item(&work_item)?;
    let work_item_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.implementation_work_item.v1",
        &work_item_value,
    )?;
    let mut report = GovernanceAgentImplementationExecutionReport {
        schema: GOVERNANCE_AGENT_IMPLEMENTATION_EXECUTION_REPORT_SCHEMA.to_string(),
        gate: "implementation-package-execution".to_string(),
        verified: validation.verified,
        work_item_file: options.work_item_file.display().to_string(),
        work_item_id: work_item.work_item_id,
        work_package_id: work_item.work_package_id,
        authorization_plan_id: work_item.authorization_plan_id,
        queue_decision_id: work_item.queue_decision_id,
        work_item_hash,
        allowed_surface_count: work_item.allowed_surfaces.len(),
        touched_surface_count: work_item.touched_surfaces.len(),
        authorized_surface_expansion_count: work_item.authorized_surface_expansions.len(),
        touched_surfaces_authorized: validation.touched_surfaces_authorized,
        forbidden_actions_bound: validation.forbidden_actions_bound,
        live_actions_forbidden: validation.live_actions_forbidden,
        required_gates_bound: validation.required_gates_bound,
        rollback_or_noop_fallback_defined: validation.rollback_or_noop_fallback_defined,
        provider_spend_command_executed: false,
        paid_replay_regenerated: false,
        live_authority_change: false,
        no_spend: true,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent implementation execution failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

struct GovernanceAgentImplementationValidation {
    verified: bool,
    touched_surfaces_authorized: bool,
    forbidden_actions_bound: bool,
    live_actions_forbidden: bool,
    required_gates_bound: bool,
    rollback_or_noop_fallback_defined: bool,
}

fn validate_governance_agent_implementation_work_item(
    work_item: &GovernanceAgentImplementationWorkItem,
) -> io::Result<GovernanceAgentImplementationValidation> {
    if work_item.schema != GOVERNANCE_AGENT_IMPLEMENTATION_WORK_ITEM_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance agent implementation work item schema",
        ));
    }
    validate_governance_text_id("implementation work_item_id", &work_item.work_item_id)?;
    validate_governance_text_id("implementation work_package_id", &work_item.work_package_id)?;
    validate_governance_text_id(
        "implementation authorization_plan_id",
        &work_item.authorization_plan_id,
    )?;
    validate_governance_text_id("implementation queue_decision_id", &work_item.queue_decision_id)?;
    validate_governance_text_id("implementation scope", &work_item.scope)?;
    if work_item.work_item_id != GOVERNANCE_AGENT_DGA_200_WORK_ITEM_ID
        || work_item.work_package_id != GOVERNANCE_AGENT_DGA_200_WORK_PACKAGE_ID
        || work_item.authorization_plan_id != GOVERNANCE_AGENT_DGA_200_AUTHORIZATION_PLAN_ID
        || work_item.queue_decision_id != GOVERNANCE_AGENT_DGA_200_QUEUE_DECISION_ID
        || work_item.source_authorization_doc != GOVERNANCE_AGENT_DGA_200_AUTHORIZATION_DOC
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance implementation work item is not bound to DGA-200 authorization lineage",
        ));
    }
    ensure_unique_nonempty_governance_agent_text_list(
        "implementation exact_targets",
        &work_item.exact_targets,
    )?;
    ensure_unique_nonempty_governance_agent_text_list(
        "implementation allowed_surfaces",
        &work_item.allowed_surfaces,
    )?;
    ensure_unique_nonempty_governance_agent_text_list(
        "implementation touched_surfaces",
        &work_item.touched_surfaces,
    )?;
    ensure_unique_nonempty_governance_agent_text_list(
        "implementation forbidden_actions",
        &work_item.forbidden_actions,
    )?;
    let touched_surfaces_authorized =
        governance_agent_touched_surfaces_authorized(work_item)?;
    let forbidden_actions_bound =
        governance_agent_required_forbidden_actions_bound(&work_item.forbidden_actions);
    let live_actions_forbidden =
        governance_agent_live_action_flags_all_forbidden(&work_item.live_action_flags);
    let required_gates_bound = governance_agent_required_gates_bound(&work_item.required_gates)?;
    let rollback_or_noop_fallback_defined =
        governance_agent_rollback_or_noop_fallback_defined(&work_item.rollback_or_noop_fallback);
    Ok(GovernanceAgentImplementationValidation {
        verified: touched_surfaces_authorized
            && forbidden_actions_bound
            && live_actions_forbidden
            && required_gates_bound
            && rollback_or_noop_fallback_defined,
        touched_surfaces_authorized,
        forbidden_actions_bound,
        live_actions_forbidden,
        required_gates_bound,
        rollback_or_noop_fallback_defined,
    })
}

fn ensure_unique_nonempty_governance_agent_text_list(
    label: &str,
    values: &[String],
) -> io::Result<()> {
    if values.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must not be empty"),
        ));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        validate_governance_text_id(label, value)?;
        if !seen.insert(value.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{label} contains a duplicate entry"),
            ));
        }
    }
    Ok(())
}

fn governance_agent_touched_surfaces_authorized(
    work_item: &GovernanceAgentImplementationWorkItem,
) -> io::Result<bool> {
    let expansion_surfaces = work_item
        .authorized_surface_expansions
        .iter()
        .map(|expansion| expansion.surface.as_str())
        .collect::<BTreeSet<_>>();
    for expansion in &work_item.authorized_surface_expansions {
        validate_governance_text_id("implementation expansion surface", &expansion.surface)?;
        validate_governance_text_id("implementation expansion reason", &expansion.reason)?;
        validate_governance_text_id("implementation expansion boundary", &expansion.boundary)?;
        if !work_item
            .touched_surfaces
            .iter()
            .any(|surface| surface == &expansion.surface)
        {
            return Ok(false);
        }
        let boundary = expansion.boundary.to_ascii_lowercase();
        if !boundary.contains("no live authority change") {
            return Ok(false);
        }
    }
    Ok(work_item.touched_surfaces.iter().all(|surface| {
        work_item
            .allowed_surfaces
            .iter()
            .any(|pattern| governance_agent_surface_matches(pattern, surface))
            || expansion_surfaces.contains(surface.as_str())
    }))
}

fn governance_agent_surface_matches(pattern: &str, surface: &str) -> bool {
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        surface.starts_with(prefix) && surface.ends_with(suffix)
    } else {
        pattern == surface
    }
}

fn governance_agent_required_forbidden_actions_bound(actions: &[String]) -> bool {
    const REQUIRED: &[&str] = &[
        "live validator-registry mutation",
        "Cobalt amendment submission",
        "provider start, stop, or delete commands",
        "hidden 100-run or 1000-run replay regeneration",
        "validator-side sidecar activation",
        "commit-reveal activation",
        "tensor parallelism greater than one",
        "canonical publishing-authority transfer",
        "secret capture",
    ];
    REQUIRED
        .iter()
        .all(|required| actions.iter().any(|action| action == required))
}

fn governance_agent_live_action_flags_all_forbidden(
    flags: &GovernanceAgentImplementationLiveActionFlags,
) -> bool {
    !flags.registry_mutation
        && !flags.cobalt_amendment_submission
        && !flags.provider_start_stop_delete
        && !flags.paid_replay_regeneration
        && !flags.validator_side_sidecar_activation
        && !flags.commit_reveal_activation
        && !flags.tensor_parallelism_greater_than_one
        && !flags.authority_transfer
        && !flags.secret_capture
}

fn governance_agent_required_gates_bound(
    gates: &[GovernanceAgentImplementationRequiredGate],
) -> io::Result<bool> {
    if gates.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "implementation required_gates must not be empty",
        ));
    }
    let mut names = BTreeSet::new();
    for gate in gates {
        validate_governance_text_id("implementation required gate name", &gate.name)?;
        validate_governance_text_id("implementation required gate command", &gate.command)?;
        if !names.insert(gate.name.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "implementation required_gates contains a duplicate name",
            ));
        }
    }
    const REQUIRED_GATES: &[(&str, &str)] = &[
        (
            "implementation-execution-report",
            "scripts/gov-inference-implementation-execution",
        ),
        (
            "release-handoff-no-refresh",
            "scripts/gov-inference-release-handoff --no-refresh-local",
        ),
        ("docs-site-build", "scripts/docs-site-build"),
        ("repository-checks", "scripts/check"),
    ];
    Ok(REQUIRED_GATES.iter().all(|(name, command)| {
        gates.iter().any(|gate| {
            gate.name == *name
                && gate.command == *command
                && gate.required_before_live_mutation
                && gate.no_spend
        })
    }))
}

fn governance_agent_rollback_or_noop_fallback_defined(fallback: &str) -> bool {
    let fallback = fallback.to_ascii_lowercase();
    fallback.contains("no-op")
        && fallback.contains("registry")
        && fallback.contains("cobalt")
        && fallback.contains("provider")
        && fallback.contains("authority")
}

pub fn governance_agent_canonical_json_bytes(
    value: &serde_json::Value,
) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    write_governance_agent_canonical_json(value, &mut output)?;
    Ok(output)
}

pub fn default_governance_agent_round_seed_input() -> GovernanceAgentRoundSeedInput {
    GovernanceAgentRoundSeedInput {
        schema: GOVERNANCE_AGENT_ROUND_SEED_SCHEMA.to_string(),
        cobalt_certificate_hash: hash_hex(
            "postfiat.governance_agent.local_cobalt_certificate.v1",
            b"postfiat-governance-agent-gate-3_6-local-cobalt-certificate",
        ),
        round_id: GOVERNANCE_AGENT_DEFAULT_ROUND_ID.to_string(),
        domain: GOVERNANCE_AGENT_DEFAULT_ROUND_DOMAIN.to_string(),
    }
}

pub fn governance_agent_round_seed_input_from_optional_parts(
    cobalt_certificate_hash: Option<String>,
    round_id: Option<String>,
    domain: Option<String>,
) -> io::Result<Option<GovernanceAgentRoundSeedInput>> {
    match (cobalt_certificate_hash, round_id, domain) {
        (None, None, None) => Ok(None),
        (Some(cobalt_certificate_hash), Some(round_id), Some(domain)) => {
            let input = GovernanceAgentRoundSeedInput {
                schema: GOVERNANCE_AGENT_ROUND_SEED_SCHEMA.to_string(),
                cobalt_certificate_hash,
                round_id,
                domain,
            };
            validate_governance_agent_round_seed_input(&input)?;
            Ok(Some(input))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance agent round seed requires --cobalt-certificate-hash, --round-id, and --round-domain together",
        )),
    }
}

fn build_governance_agent_model_request(agent_dir: &Path) -> io::Result<GovernanceAgentModelRequest> {
    build_governance_agent_model_request_with_seed(
        agent_dir,
        default_governance_agent_round_seed_input(),
    )
}

fn build_governance_agent_model_request_with_seed(
    agent_dir: &Path,
    round_seed_input: GovernanceAgentRoundSeedInput,
) -> io::Result<GovernanceAgentModelRequest> {
    validate_governance_agent_round_seed_input(&round_seed_input)?;
    let round_seed = governance_agent_round_seed(&round_seed_input)?;
    let validation = validate_governance_agent_source_bundle(agent_dir)?;
    let ruleset_schema_path = agent_dir.join("ruleset_schema.json");
    let ruleset_schema =
        read_governance_agent_json_value(&ruleset_schema_path, "governance ruleset schema")?;
    let validator_evidence_packet_schema_path =
        agent_dir.join(GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_PACKET_SCHEMA_FILE_NAME);
    let validator_evidence_packet_schema = read_governance_agent_json_value(
        &validator_evidence_packet_schema_path,
        "validator evidence packet schema",
    )?;
    let validator_evidence_packet_schema_hash =
        governance_agent_canonical_json_sha384_hash(&validator_evidence_packet_schema)?;
    let validator_evidence_field_registry_path =
        governance_agent_validator_evidence_field_registry_path(agent_dir);
    let validator_evidence_field_registry_bytes = read_bounded_governance_agent_bytes(
        &validator_evidence_field_registry_path,
        "validator evidence field registry",
    )?;
    let validator_evidence_field_registry_hash =
        governance_agent_sha384_hex(&validator_evidence_field_registry_bytes);
    let valid_fixture_path = agent_dir.join("fixtures/valid_ruleset.json");
    let valid_fixture =
        read_governance_agent_json_value(&valid_fixture_path, "valid governance ruleset fixture")?;
    let valid_fixture_hash = validation.valid_fixture.canonical_hash.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "valid governance ruleset fixture did not produce a canonical hash",
        )
    })?;
    let statements = governance_agent_request_statements(agent_dir, &validation.statement_hashes)?;
    let runtime_manifest = governance_agent_runtime_manifest();
    let output_contract = governance_agent_output_contract();
    let messages = governance_agent_model_messages(GovernanceAgentModelMessageContext {
        bundle_hash: &validation.bundle_hash,
        ruleset_schema_hash: &validation.ruleset_schema_hash,
        validator_evidence_packet_schema_hash: &validator_evidence_packet_schema_hash,
        validator_evidence_field_registry_hash: &validator_evidence_field_registry_hash,
        valid_fixture_hash: &valid_fixture_hash,
        valid_fixture: &valid_fixture,
        statements: &statements,
        round_seed_input: &round_seed_input,
        round_seed: &round_seed,
    });
    let openai_chat_request = governance_agent_openai_chat_request(&messages, &output_contract);
    let governed_inputs = GovernanceAgentRequestInputs {
        bundle_hash: validation.bundle_hash.clone(),
        statement_hashes: validation.statement_hashes,
        statements,
        ruleset_schema_hash: validation.ruleset_schema_hash.clone(),
        ruleset_schema,
        validator_evidence_packet_schema_path: validator_evidence_packet_schema_path
            .display()
            .to_string(),
        validator_evidence_packet_schema_hash,
        validator_evidence_field_registry_path: validator_evidence_field_registry_path
            .display()
            .to_string(),
        validator_evidence_field_registry_hash,
        valid_fixture_hash,
        valid_fixture,
    };
    let mut request = GovernanceAgentModelRequest {
        schema: GOVERNANCE_AGENT_MODEL_REQUEST_SCHEMA.to_string(),
        request_id: "dga-gate-3_6-timelocked-dry-run-request-v1".to_string(),
        request_hash: String::new(),
        bundle_hash: validation.bundle_hash,
        evidence_root: GOVERNANCE_AGENT_EVIDENCE_ROOT.to_string(),
        round_seed_input,
        round_seed,
        runtime_manifest,
        output_contract,
        governed_inputs,
        openai_chat_request,
        request_hash_includes: vec![
            "bundle_hash".to_string(),
            "evidence_root".to_string(),
            "round_seed_input".to_string(),
            "round_seed".to_string(),
            "runtime_manifest".to_string(),
            "output_contract".to_string(),
            "governed_inputs".to_string(),
            "openai_chat_request".to_string(),
        ],
        redaction_checked: false,
    };
    request.request_hash = governance_agent_full_model_request_hash(&request)?;
    ensure_governance_agent_model_request_seed(&request, &request.round_seed_input)?;
    ensure_governance_agent_model_request_json_only(&request)?;
    let probe = serde_json::to_string(&request).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    request.redaction_checked = true;
    Ok(request)
}

fn validate_governance_agent_source_bundle(
    agent_dir: &Path,
) -> io::Result<GovernanceAgentBundleValidation> {
    let architecture = hash_governance_agent_statement(
        agent_dir,
        "architecture_statement.md",
        "architecture_statement",
    )?;
    let objective =
        hash_governance_agent_statement(agent_dir, "objective_statement.md", "objective_statement")?;
    let constraints = hash_governance_agent_statement(
        agent_dir,
        "constitutional_constraints.md",
        "constitutional_constraints",
    )?;
    let statement_hash_one_byte_edit_detected =
        governance_agent_statement_hash_one_byte_edit_detected(
            agent_dir.join("objective_statement.md"),
            "objective_statement",
            &objective.hash,
        )?;

    let schema_path = agent_dir.join("ruleset_schema.json");
    let schema_value = read_governance_agent_json_value(&schema_path, "governance ruleset schema")?;
    let ruleset_schema_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.ruleset_schema.v1",
        &schema_value,
    )?;

    let valid_fixture_path = agent_dir.join("fixtures/valid_ruleset.json");
    let valid_fixture = validate_governance_agent_fixture(
        "valid_ruleset",
        &valid_fixture_path,
        true,
    )?;
    let valid_fixture_hash = valid_fixture.canonical_hash.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "valid governance ruleset fixture did not produce a canonical hash",
        )
    })?;

    let invalid_fixture_paths = [
        ("prose_only", agent_dir.join("fixtures/invalid/prose_only.txt")),
        (
            "unknown_field",
            agent_dir.join("fixtures/invalid/unknown_field.json"),
        ),
        (
            "evidence_unbound",
            agent_dir.join("fixtures/invalid/evidence_unbound.json"),
        ),
        (
            "missing_no_op",
            agent_dir.join("fixtures/invalid/missing_no_op.json"),
        ),
        (
            "missing_validator_evidence_packet_input",
            agent_dir.join("fixtures/invalid/missing_validator_evidence_packet_input.json"),
        ),
        (
            "authority_expanding",
            agent_dir.join("fixtures/invalid/authority_expanding.json"),
        ),
    ];
    let mut invalid_fixtures = Vec::with_capacity(invalid_fixture_paths.len());
    for (name, path) in invalid_fixture_paths {
        invalid_fixtures.push(validate_governance_agent_fixture(name, &path, false)?);
    }

    let canonical_json_key_order_stable = governance_agent_key_order_check()?;
    let bundle_hash = governance_agent_bundle_hash(
        &architecture.hash,
        &objective.hash,
        &constraints.hash,
        &ruleset_schema_hash,
        &valid_fixture_hash,
    )?;
    let round_seed_input = default_governance_agent_round_seed_input();
    let round_seed = governance_agent_round_seed(&round_seed_input)?;
    let model_request_hash = governance_agent_model_request_hash(
        &bundle_hash,
        &objective.hash,
        &ruleset_schema_hash,
        &round_seed_input,
        &round_seed,
    )?;

    Ok(GovernanceAgentBundleValidation {
        statement_hashes: vec![architecture, objective, constraints],
        ruleset_schema_hash,
        valid_fixture,
        invalid_fixtures,
        canonical_json_key_order_stable,
        statement_hash_one_byte_edit_detected,
        bundle_hash,
        model_request_hash,
    })
}

fn governance_agent_request_statements(
    agent_dir: &Path,
    statement_hashes: &[GovernanceAgentStatementHash],
) -> io::Result<Vec<GovernanceAgentRequestStatement>> {
    let statement_files = [
        ("architecture_statement", "architecture_statement.md"),
        ("objective_statement", "objective_statement.md"),
        (
            "constitutional_constraints",
            "constitutional_constraints.md",
        ),
    ];
    let mut statements = Vec::with_capacity(statement_files.len());
    for (name, file_name) in statement_files {
        let hash = statement_hashes
            .iter()
            .find(|statement| statement.name == name)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing governance agent statement hash `{name}`"),
                )
            })?;
        let path = agent_dir.join(file_name);
        let bytes = read_bounded_governance_agent_bytes(&path, "governance agent statement")?;
        let content = String::from_utf8(bytes).map_err(invalid_data)?;
        statements.push(GovernanceAgentRequestStatement {
            name: name.to_string(),
            path: path.display().to_string(),
            hash: hash.hash.clone(),
            content,
        });
    }
    Ok(statements)
}

fn governance_agent_validator_evidence_field_registry_path(agent_dir: &Path) -> PathBuf {
    let repo_relative = PathBuf::from(DEFAULT_GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_FIELD_REGISTRY_FILE);
    if repo_relative.exists() {
        return repo_relative;
    }
    if let Some(repo_root) = agent_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    {
        let candidate = repo_root.join(DEFAULT_GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_FIELD_REGISTRY_FILE);
        if candidate.exists() {
            return candidate;
        }
    }
    repo_relative
}

fn governance_agent_runtime_manifest() -> GovernanceAgentRuntimeManifest {
    GovernanceAgentRuntimeManifest {
        model_id: GOVERNANCE_AGENT_MODEL_ID.to_string(),
        runtime: GOVERNANCE_AGENT_RUNTIME_ENGINE.to_string(),
        runtime_profile: GOVERNANCE_AGENT_RUNTIME.to_string(),
        provider_profile: GOVERNANCE_AGENT_PROVIDER_PROFILE.to_string(),
        image: GOVERNANCE_AGENT_SGLANG_IMAGE.to_string(),
        tensor_parallelism: GOVERNANCE_AGENT_TENSOR_PARALLELISM,
        context_length: GOVERNANCE_AGENT_CONTEXT_LENGTH,
        chunked_prefill_size: GOVERNANCE_AGENT_CHUNKED_PREFILL_SIZE,
        max_running_requests: GOVERNANCE_AGENT_MAX_RUNNING_REQUESTS,
        deterministic_flags: vec![
            "--enable-deterministic-inference".to_string(),
            "--max-running-requests=1".to_string(),
            "--tp=1".to_string(),
            "chat_template_kwargs.enable_thinking=false".to_string(),
            "--temperature=0".to_string(),
            "--top-p=1".to_string(),
        ],
    }
}

fn governance_agent_output_contract() -> GovernanceAgentOutputContract {
    GovernanceAgentOutputContract {
        schema: GOVERNANCE_AGENT_RULESET_SCHEMA.to_string(),
        json_only: true,
        markdown_allowed: false,
        prose_allowed: false,
        unknown_fields_allowed: false,
        fallback_decision: "no_op".to_string(),
    }
}

struct GovernanceAgentModelMessageContext<'a> {
    bundle_hash: &'a str,
    ruleset_schema_hash: &'a str,
    validator_evidence_packet_schema_hash: &'a str,
    validator_evidence_field_registry_hash: &'a str,
    valid_fixture_hash: &'a str,
    valid_fixture: &'a serde_json::Value,
    statements: &'a [GovernanceAgentRequestStatement],
    round_seed_input: &'a GovernanceAgentRoundSeedInput,
    round_seed: &'a str,
}

fn governance_agent_model_messages(
    context: GovernanceAgentModelMessageContext<'_>,
) -> Vec<GovernanceAgentModelMessage> {
    let valid_fixture_json =
        serde_json::to_string_pretty(context.valid_fixture).unwrap_or_else(|_| "{}".to_string());
    let statement_summary = context
        .statements
        .iter()
        .map(|statement| {
            format!(
                "{} hash={} path={}",
                statement.name, statement.hash, statement.path
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let statement_body = context
        .statements
        .iter()
        .map(|statement| {
            format!(
                "## {}\npath: {}\nhash: {}\n\n{}",
                statement.name, statement.path, statement.hash, statement.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    vec![
        GovernanceAgentModelMessage {
            role: "system".to_string(),
            content: GOVERNANCE_AGENT_SYSTEM_PROMPT.to_string(),
        },
        GovernanceAgentModelMessage {
            role: "user".to_string(),
            content: format!(
                "{GOVERNANCE_AGENT_USER_INSTRUCTION}\n\nBundle hash: {}\nRuleset schema hash: {}\nValidator evidence packet schema hash: {}\nValidator evidence field registry hash: {}\nValid no-op fixture hash: {}\nEvidence root: {GOVERNANCE_AGENT_EVIDENCE_ROOT}\nCobalt certificate hash: {}\nCobalt round id: {}\nRound seed domain: {}\nRound seed: {}\n\nGate 3.6 binds this request to the Cobalt certificate hash, round id, and domain. Gate 3.6 has no external evidence beyond the governed source bundle; it defines the packet input contract and the reviewed evidence-universe hashes, but it does not expose live packet contents to the model. The safe output is the explicit no-op ruleset with validator_evidence_packet declared as a required input for later deterministic evidence execution. Return this exact JSON object shape and field names:\n{valid_fixture_json}\n\nGoverned statement summary:\n{statement_summary}\n\nGoverned statement text:\n{statement_body}\n\nReturn only a GovernanceRuleset JSON object. The response must parse as one JSON object with schema `{GOVERNANCE_AGENT_RULESET_SCHEMA}`.",
                context.bundle_hash,
                context.ruleset_schema_hash,
                context.validator_evidence_packet_schema_hash,
                context.validator_evidence_field_registry_hash,
                context.valid_fixture_hash,
                context.round_seed_input.cobalt_certificate_hash,
                context.round_seed_input.round_id,
                context.round_seed_input.domain,
                context.round_seed
            ),
        },
    ]
}

fn governance_agent_openai_chat_request(
    messages: &[GovernanceAgentModelMessage],
    output_contract: &GovernanceAgentOutputContract,
) -> serde_json::Value {
    serde_json::json!({
        "max_tokens": GOVERNANCE_AGENT_MAX_OUTPUT_TOKENS,
        "chat_template_kwargs": {
            "enable_thinking": false
        },
        "messages": messages,
        "model": GOVERNANCE_AGENT_MODEL_ID,
        "response_format": {
            "type": "json_object"
        },
        "stream": false,
        "temperature": 0,
        "top_p": 1,
        "postfiat_output_contract": output_contract
    })
}

fn governance_agent_full_model_request_hash(
    request: &GovernanceAgentModelRequest,
) -> io::Result<String> {
    let mut value = serde_json::to_value(request).map_err(invalid_data)?;
    let object = value.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request must encode as a JSON object",
        )
    })?;
    object.remove("request_hash");
    object.remove("redaction_checked");
    governance_agent_canonical_json_hash("postfiat.governance_agent.full_model_request.v1", &value)
}

fn resolve_governance_agent_round_seed_input(
    input: Option<GovernanceAgentRoundSeedInput>,
) -> io::Result<GovernanceAgentRoundSeedInput> {
    let input = input.unwrap_or_else(default_governance_agent_round_seed_input);
    validate_governance_agent_round_seed_input(&input)?;
    Ok(input)
}

fn governance_agent_round_seed(input: &GovernanceAgentRoundSeedInput) -> io::Result<String> {
    validate_governance_agent_round_seed_input(input)?;
    let value = serde_json::to_value(input).map_err(invalid_data)?;
    governance_agent_canonical_json_hash("postfiat.governance_agent.round_seed.v1", &value)
}

fn validate_governance_agent_round_seed_input(
    input: &GovernanceAgentRoundSeedInput,
) -> io::Result<()> {
    if input.schema != GOVERNANCE_AGENT_ROUND_SEED_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent round seed schema is unsupported",
        ));
    }
    validate_governance_agent_hash_hex(
        "cobalt_certificate_hash",
        &input.cobalt_certificate_hash,
    )?;
    validate_governance_text_id("round_id", &input.round_id)?;
    validate_governance_text_id("round_domain", &input.domain)?;
    Ok(())
}

fn validate_governance_agent_hash_hex(label: &str, value: &str) -> io::Result<()> {
    if value.len() != 96 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be a 96-character lowercase hex hash"),
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be lowercase hex"),
        ));
    }
    Ok(())
}

fn ensure_governance_agent_model_request_seed(
    request: &GovernanceAgentModelRequest,
    expected: &GovernanceAgentRoundSeedInput,
) -> io::Result<()> {
    validate_governance_agent_round_seed_input(expected)?;
    validate_governance_agent_round_seed_input(&request.round_seed_input)?;
    let observed_round_seed = governance_agent_round_seed(&request.round_seed_input)?;
    if request.round_seed != observed_round_seed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request round_seed does not match round_seed_input",
        ));
    }
    if request.round_seed_input != *expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request round_seed_input does not match expected Cobalt round seed",
        ));
    }
    let expected_round_seed = governance_agent_round_seed(expected)?;
    if request.round_seed != expected_round_seed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request round_seed does not match expected Cobalt round seed",
        ));
    }
    let recomputed_request_hash = governance_agent_full_model_request_hash(request)?;
    if request.request_hash != recomputed_request_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request hash is not self-consistent",
        ));
    }
    Ok(())
}

fn governance_agent_wrong_round_seed_input(
    expected: &GovernanceAgentRoundSeedInput,
) -> GovernanceAgentRoundSeedInput {
    GovernanceAgentRoundSeedInput {
        schema: expected.schema.clone(),
        cobalt_certificate_hash: hash_hex(
            "postfiat.governance_agent.wrong_cobalt_certificate.v1",
            expected.cobalt_certificate_hash.as_bytes(),
        ),
        round_id: expected.round_id.clone(),
        domain: expected.domain.clone(),
    }
}

fn governance_agent_stale_round_seed_input(
    expected: &GovernanceAgentRoundSeedInput,
) -> GovernanceAgentRoundSeedInput {
    GovernanceAgentRoundSeedInput {
        schema: expected.schema.clone(),
        cobalt_certificate_hash: expected.cobalt_certificate_hash.clone(),
        round_id: format!("{}-stale", expected.round_id),
        domain: expected.domain.clone(),
    }
}

fn governance_agent_seed_rejection_check(
    name: &str,
    agent_dir: &Path,
    candidate: GovernanceAgentRoundSeedInput,
    expected: &GovernanceAgentRoundSeedInput,
) -> GovernanceAgentSeedRejectionCheck {
    match build_governance_agent_model_request_with_seed(agent_dir, candidate)
        .and_then(|request| ensure_governance_agent_model_request_seed(&request, expected))
    {
        Ok(()) => GovernanceAgentSeedRejectionCheck {
            name: name.to_string(),
            accepted: true,
            error: None,
        },
        Err(error) => GovernanceAgentSeedRejectionCheck {
            name: name.to_string(),
            accepted: false,
            error: Some(error.to_string()),
        },
    }
}

fn ensure_governance_agent_model_request_json_only(
    request: &GovernanceAgentModelRequest,
) -> io::Result<()> {
    if !request.output_contract.json_only
        || request.output_contract.markdown_allowed
        || request.output_contract.prose_allowed
        || request.output_contract.unknown_fields_allowed
        || request.output_contract.schema != GOVERNANCE_AGENT_RULESET_SCHEMA
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request output contract is not JSON-only",
        ));
    }
    let response_format = request
        .openai_chat_request
        .get("response_format")
        .and_then(serde_json::Value::as_object)
        .and_then(|format| format.get("type"))
        .and_then(serde_json::Value::as_str);
    if response_format != Some("json_object") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request must set response_format=json_object",
        ));
    }
    let messages = request
        .openai_chat_request
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "governance agent model request requires messages",
            )
        })?;
    let joined_messages = messages
        .iter()
        .filter_map(|message| message.get("content"))
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    if !joined_messages.contains("Return exactly one JSON object")
        || !joined_messages.contains("Do not include Markdown")
        || !joined_messages.contains("evidence_field_path")
        || !joined_messages.contains("required_provenance")
        || !joined_messages.contains("unregistered evidence fields")
        || !joined_messages.contains(GOVERNANCE_AGENT_RULESET_SCHEMA)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request is missing JSON-only prompt instructions",
        ));
    }
    validate_governance_agent_hash_hex(
        "validator_evidence_packet_schema_hash",
        &request
            .governed_inputs
            .validator_evidence_packet_schema_hash,
    )?;
    validate_governance_agent_hash_hex(
        "validator_evidence_field_registry_hash",
        &request
            .governed_inputs
            .validator_evidence_field_registry_hash,
    )?;
    if !joined_messages.contains(
        &request
            .governed_inputs
            .validator_evidence_packet_schema_hash,
    ) || !joined_messages.contains(
        &request
            .governed_inputs
            .validator_evidence_field_registry_hash,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent model request is missing validator evidence lineage hashes",
        ));
    }
    Ok(())
}

fn validate_governance_agent_report_validator_evidence_lineage(
    label: &str,
    validator_evidence_packet_schema_hash: &str,
    validator_evidence_field_registry_hash: &str,
    request: &GovernanceAgentModelRequest,
) -> io::Result<()> {
    validate_governance_agent_hash_hex(
        &format!("{label} validator_evidence_packet_schema_hash"),
        validator_evidence_packet_schema_hash,
    )?;
    validate_governance_agent_hash_hex(
        &format!("{label} validator_evidence_field_registry_hash"),
        validator_evidence_field_registry_hash,
    )?;
    if validator_evidence_packet_schema_hash
        != request
            .governed_inputs
            .validator_evidence_packet_schema_hash
            .as_str()
        || validator_evidence_field_registry_hash
            != request
                .governed_inputs
                .validator_evidence_field_registry_hash
                .as_str()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} validator evidence lineage does not match model request"),
        ));
    }
    Ok(())
}

fn governance_agent_evidence_lineage_audit_item(
    name: &str,
    path: &Path,
    expected_schema: &str,
    expected_gate: &str,
    request: &GovernanceAgentModelRequest,
) -> io::Result<GovernanceAgentEvidenceLineageAuditItem> {
    let value = read_governance_agent_json_value(path, "governance agent evidence-lineage report")?;
    let schema = governance_agent_json_string_field(&value, "schema", name)?;
    let gate = governance_agent_json_string_field(&value, "gate", name)?;
    let verified = governance_agent_json_bool_field(&value, "verified", name)?;
    let packet_schema_hash = governance_agent_json_optional_string_field(
        &value,
        "validator_evidence_packet_schema_hash",
    );
    let field_registry_hash = governance_agent_json_optional_string_field(
        &value,
        "validator_evidence_field_registry_hash",
    );
    let lineage_fields_present = packet_schema_hash.is_some() && field_registry_hash.is_some();
    let validator_evidence_packet_schema_hash = packet_schema_hash.unwrap_or_default();
    let validator_evidence_field_registry_hash = field_registry_hash.unwrap_or_default();
    let packet_hash_well_formed = validate_governance_agent_hash_hex(
        &format!("{name} validator_evidence_packet_schema_hash"),
        &validator_evidence_packet_schema_hash,
    )
    .is_ok();
    let field_hash_well_formed = validate_governance_agent_hash_hex(
        &format!("{name} validator_evidence_field_registry_hash"),
        &validator_evidence_field_registry_hash,
    )
    .is_ok();
    let hashes_well_formed = packet_hash_well_formed && field_hash_well_formed;
    let schema_matches_expected = schema == expected_schema;
    let gate_matches_expected = gate == expected_gate;
    let matches_model_request = hashes_well_formed
        && validate_governance_agent_report_validator_evidence_lineage(
            name,
            &validator_evidence_packet_schema_hash,
            &validator_evidence_field_registry_hash,
            request,
        )
        .is_ok();
    Ok(GovernanceAgentEvidenceLineageAuditItem {
        name: name.to_string(),
        path: path.display().to_string(),
        schema,
        gate,
        verified,
        validator_evidence_packet_schema_hash,
        validator_evidence_field_registry_hash,
        lineage_fields_present,
        hashes_well_formed,
        schema_matches_expected,
        gate_matches_expected,
        matches_model_request,
    })
}

fn governance_agent_json_string_field(
    value: &serde_json::Value,
    field: &str,
    label: &str,
) -> io::Result<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{label} report is missing string field `{field}`"),
            )
        })
}

fn governance_agent_json_optional_string_field(
    value: &serde_json::Value,
    field: &str,
) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn governance_agent_json_bool_field(
    value: &serde_json::Value,
    field: &str,
    label: &str,
) -> io::Result<bool> {
    value
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{label} report is missing bool field `{field}`"),
            )
        })
}

fn validate_governance_agent_fixture(
    name: &str,
    path: &Path,
    expect_valid: bool,
) -> io::Result<GovernanceAgentFixtureCheck> {
    let raw = read_bounded_json_text_file(path, "governance agent fixture")?;
    let result = validate_governance_ruleset_text(&raw);
    match (expect_valid, result) {
        (true, Ok(value)) => {
            let canonical_hash = governance_agent_canonical_json_hash(
                "postfiat.governance_agent.ruleset_fixture.v1",
                &value,
            )?;
            Ok(GovernanceAgentFixtureCheck {
                name: name.to_string(),
                path: path.display().to_string(),
                accepted: true,
                canonical_hash: Some(canonical_hash),
                error: None,
            })
        }
        (true, Err(error)) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("valid governance agent fixture `{name}` was rejected: {error}"),
        )),
        (false, Ok(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid governance agent fixture `{name}` was accepted"),
        )),
        (false, Err(error)) => Ok(GovernanceAgentFixtureCheck {
            name: name.to_string(),
            path: path.display().to_string(),
            accepted: false,
            canonical_hash: None,
            error: Some(error.to_string()),
        }),
    }
}

fn read_governance_ruleset_file(
    path: &Path,
) -> io::Result<(serde_json::Value, GovernanceRuleset)> {
    let raw = read_bounded_json_text_file(path, "governance ruleset")?;
    validate_governance_ruleset_text_with_ruleset(&raw)
}

fn read_governance_agent_evidence_snapshot(
    path: &Path,
) -> io::Result<GovernanceAgentFrozenEvidenceSnapshot> {
    let value = read_governance_agent_json_value(path, "governance agent frozen evidence")?;
    let evidence: GovernanceAgentFrozenEvidenceSnapshot =
        serde_json::from_value(value).map_err(invalid_data)?;
    validate_governance_agent_evidence_snapshot(&evidence)?;
    Ok(evidence)
}

fn validate_governance_agent_evidence_snapshot(
    evidence: &GovernanceAgentFrozenEvidenceSnapshot,
) -> io::Result<()> {
    if evidence.schema != GOVERNANCE_AGENT_EVIDENCE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance agent frozen evidence schema",
        ));
    }
    validate_governance_text_id("evidence snapshot_id", &evidence.snapshot_id)?;
    validate_governance_agent_hash_hex(
        "validator_registry_root",
        &evidence.validator_registry_root,
    )?;
    validate_governance_agent_hash_hex("cobalt_evidence_root", &evidence.cobalt_evidence_root)?;
    validate_governance_agent_hash_hex("operator_manifest_root", &evidence.operator_manifest_root)?;
    validate_governance_agent_hash_hex(
        "validator_evidence_packet_root",
        &evidence.validator_evidence_packet_root,
    )?;
    let mut seen = BTreeSet::new();
    for input in &evidence.available_inputs {
        validate_governance_agent_input_kind(input)?;
        if !seen.insert(input.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance agent evidence input kind",
            ));
        }
    }
    if !seen.contains(GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent frozen evidence missing validator evidence packet input",
        ));
    }
    if evidence.network_access_allowed
        || evidence.model_access_allowed
        || evidence.filesystem_access_allowed
        || evidence.direct_state_mutation_allowed
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent frozen evidence grants forbidden policy access",
        ));
    }
    Ok(())
}

fn compile_governance_agent_ruleset_policy(
    ruleset: &GovernanceRuleset,
    ruleset_hash: &str,
) -> io::Result<GovernanceAgentCompiledPolicy> {
    validate_governance_ruleset_authority(&ruleset.authority)?;
    validate_governance_ruleset_inputs(&ruleset.inputs)?;
    validate_governance_ruleset_decisions(&ruleset.decisions)?;
    validate_governance_ruleset_evidence_input_binding(&ruleset.inputs, &ruleset.decisions)?;
    validate_governance_ruleset_rollback(&ruleset.rollback)?;
    let compiled_policy_hash = governance_agent_compiled_policy_hash(ruleset, ruleset_hash)?;
    let input_kinds = ruleset
        .inputs
        .iter()
        .map(|input| input.kind.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let decision_ids = ruleset
        .decisions
        .iter()
        .map(|decision| decision.decision_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let no_op_decision_id = ruleset
        .decisions
        .iter()
        .filter(|decision| decision.kind == "no_op")
        .map(|decision| decision.decision_id.clone())
        .min()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "governance ruleset policy requires a no_op decision",
            )
        })?;
    Ok(GovernanceAgentCompiledPolicy {
        schema: GOVERNANCE_AGENT_COMPILED_POLICY_SCHEMA.to_string(),
        policy_shape: "interpreter".to_string(),
        interpreter: "postfiat-governance-agent-allowlisted-rust-interpreter-v1".to_string(),
        ruleset_hash: ruleset_hash.to_string(),
        compiled_policy_hash,
        ruleset_id: ruleset.ruleset_id.clone(),
        scope: ruleset.scope.clone(),
        input_kinds,
        decision_ids,
        no_op_decision_id,
        max_mutations: ruleset.rollback.max_mutations,
        rollback_required: ruleset.rollback.required,
        operator_confirmation_required: ruleset.rollback.operator_confirmation_required,
        sandbox: GovernanceAgentPolicySandbox {
            network_access: false,
            model_access: false,
            filesystem_access: false,
            direct_state_mutation: false,
        },
    })
}

fn execute_governance_agent_policy(
    ruleset: &GovernanceRuleset,
    policy: &GovernanceAgentCompiledPolicy,
    evidence: &GovernanceAgentFrozenEvidenceSnapshot,
    evidence_snapshot_hash: &str,
) -> io::Result<GovernanceAgentRegistryDeltaCandidate> {
    validate_governance_agent_evidence_snapshot(evidence)?;
    ensure_governance_agent_policy_sandbox(policy)?;
    ensure_governance_agent_evidence_covers_ruleset(ruleset, evidence)?;
    if policy.max_mutations != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 7.5 policy execution only permits zero-mutation dry-run rulesets",
        ));
    }
    let no_op = ruleset
        .decisions
        .iter()
        .find(|decision| decision.decision_id == policy.no_op_decision_id)
        .filter(|decision| decision.kind == "no_op")
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "compiled policy no_op decision is missing from ruleset",
            )
        })?;
    let mut candidate = GovernanceAgentRegistryDeltaCandidate {
        schema: GOVERNANCE_AGENT_REGISTRY_DELTA_SCHEMA.to_string(),
        candidate_hash: String::new(),
        ruleset_hash: policy.ruleset_hash.clone(),
        compiled_policy_hash: policy.compiled_policy_hash.clone(),
        evidence_snapshot_hash: evidence_snapshot_hash.to_string(),
        decision_id: no_op.decision_id.clone(),
        action: "no_op".to_string(),
        mutations: Vec::new(),
        mutation_count: 0,
        rollback_required: policy.rollback_required,
        operator_confirmation_required: policy.operator_confirmation_required,
        rationale: no_op.rationale.clone(),
    };
    candidate.candidate_hash = governance_agent_registry_delta_candidate_hash(&candidate)?;
    Ok(candidate)
}

fn read_governance_guarded_apply_ruleset_file(
    path: &Path,
) -> io::Result<(serde_json::Value, GovernanceRuleset)> {
    let value = read_governance_agent_json_value(path, "governance guarded-apply ruleset")?;
    let ruleset = validate_governance_guarded_apply_ruleset_value(&value)?;
    Ok((value, ruleset))
}

fn validate_governance_guarded_apply_ruleset_value(
    value: &serde_json::Value,
) -> io::Result<GovernanceRuleset> {
    let ruleset: GovernanceRuleset = serde_json::from_value(value.clone()).map_err(invalid_data)?;
    validate_governance_text_id("ruleset_id", &ruleset.ruleset_id)?;
    if ruleset.schema != GOVERNANCE_AGENT_RULESET_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance guarded-apply ruleset schema",
        ));
    }
    if ruleset.scope != "validator_registry_policy" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply ruleset scope must be validator_registry_policy",
        ));
    }
    validate_governance_ruleset_authority_guarded_apply(&ruleset.authority)?;
    validate_governance_ruleset_inputs(&ruleset.inputs)?;
    validate_governance_ruleset_decisions(&ruleset.decisions)?;
    validate_governance_ruleset_evidence_input_binding(&ruleset.inputs, &ruleset.decisions)?;
    validate_governance_ruleset_rollback_guarded_apply(&ruleset.rollback)?;
    if !ruleset
        .decisions
        .iter()
        .any(|decision| decision.kind == "registry_delta_candidate")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply ruleset requires a registry_delta_candidate decision",
        ));
    }
    Ok(ruleset)
}

fn validate_governance_ruleset_authority_guarded_apply(
    authority: &GovernanceRulesetAuthority,
) -> io::Result<()> {
    if authority.mode != "guarded_apply" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply ruleset authority mode must be guarded_apply",
        ));
    }
    if authority.direct_state_mutation {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply ruleset authority allows direct state mutation",
        ));
    }
    if authority.self_upgrade {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply ruleset authority allows self upgrade",
        ));
    }
    if authority.scope_expansion {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply ruleset authority allows scope expansion",
        ));
    }
    Ok(())
}

fn validate_governance_ruleset_rollback_guarded_apply(
    rollback: &GovernanceRulesetRollback,
) -> io::Result<()> {
    if !rollback.required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply rollback evidence must be required",
        ));
    }
    if rollback.max_mutations != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply rollback max_mutations must be one",
        ));
    }
    if rollback.operator_confirmation_required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply routine execution cannot require human approval",
        ));
    }
    Ok(())
}

fn compile_governance_agent_guarded_apply_policy(
    ruleset: &GovernanceRuleset,
    ruleset_hash: &str,
) -> io::Result<GovernanceAgentCompiledPolicy> {
    validate_governance_ruleset_authority_guarded_apply(&ruleset.authority)?;
    validate_governance_ruleset_inputs(&ruleset.inputs)?;
    validate_governance_ruleset_decisions(&ruleset.decisions)?;
    validate_governance_ruleset_evidence_input_binding(&ruleset.inputs, &ruleset.decisions)?;
    validate_governance_ruleset_rollback_guarded_apply(&ruleset.rollback)?;
    let compiled_policy_hash = governance_agent_compiled_policy_hash(ruleset, ruleset_hash)?;
    let input_kinds = ruleset
        .inputs
        .iter()
        .map(|input| input.kind.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let decision_ids = ruleset
        .decisions
        .iter()
        .map(|decision| decision.decision_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let no_op_decision_id = ruleset
        .decisions
        .iter()
        .filter(|decision| decision.kind == "no_op")
        .map(|decision| decision.decision_id.clone())
        .min()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "compiled guarded-apply policy requires a no_op decision",
            )
        })?;
    Ok(GovernanceAgentCompiledPolicy {
        schema: GOVERNANCE_AGENT_COMPILED_POLICY_SCHEMA.to_string(),
        policy_shape: "interpreter".to_string(),
        interpreter: "postfiat-governance-agent-allowlisted-rust-interpreter-v1".to_string(),
        ruleset_hash: ruleset_hash.to_string(),
        compiled_policy_hash,
        ruleset_id: ruleset.ruleset_id.clone(),
        scope: ruleset.scope.clone(),
        input_kinds,
        decision_ids,
        no_op_decision_id,
        max_mutations: ruleset.rollback.max_mutations,
        rollback_required: ruleset.rollback.required,
        operator_confirmation_required: ruleset.rollback.operator_confirmation_required,
        sandbox: GovernanceAgentPolicySandbox {
            network_access: false,
            model_access: false,
            filesystem_access: false,
            direct_state_mutation: false,
        },
    })
}

struct GovernanceAgentGuardedApplyExecutionContext<'a> {
    evidence_snapshot_hash: &'a str,
    previous_validators: &'a [String],
    new_validators: &'a [String],
    previous_registry_root: &'a str,
    new_registry_root: &'a str,
}

fn execute_governance_agent_guarded_apply_policy(
    ruleset: &GovernanceRuleset,
    policy: &GovernanceAgentCompiledPolicy,
    evidence: &GovernanceAgentFrozenEvidenceSnapshot,
    context: &GovernanceAgentGuardedApplyExecutionContext<'_>,
) -> io::Result<GovernanceAgentGuardedApplyCandidate> {
    validate_governance_agent_evidence_snapshot(evidence)?;
    ensure_governance_agent_policy_sandbox(policy)?;
    ensure_governance_agent_evidence_covers_ruleset(ruleset, evidence)?;
    if evidence.validator_registry_root != context.previous_registry_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply evidence validator registry root mismatch",
        ));
    }
    if policy.max_mutations != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply policy execution requires max_mutations=1",
        ));
    }
    if !policy.rollback_required || policy.operator_confirmation_required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply policy requires rollback without routine human approval",
        ));
    }
    let decision = ruleset
        .decisions
        .iter()
        .filter(|decision| decision.kind == "registry_delta_candidate")
        .min_by(|left, right| left.decision_id.cmp(&right.decision_id))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "guarded-apply policy has no registry_delta_candidate decision",
            )
        })?;
    let linkedness_value = serde_json::json!({
        "evidence_snapshot_hash": context.evidence_snapshot_hash,
        "new_registry_root": context.new_registry_root,
        "new_validators": context.new_validators,
        "previous_registry_root": context.previous_registry_root,
        "previous_validators": context.previous_validators
    });
    let linkedness_root = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.gate_9_5.linkedness.v1",
        &linkedness_value,
    )?;
    let mut candidate = GovernanceAgentGuardedApplyCandidate {
        schema: GOVERNANCE_AGENT_GUARDED_APPLY_CANDIDATE_SCHEMA.to_string(),
        candidate_hash: String::new(),
        ruleset_hash: policy.ruleset_hash.clone(),
        compiled_policy_hash: policy.compiled_policy_hash.clone(),
        evidence_snapshot_hash: context.evidence_snapshot_hash.to_string(),
        decision_id: decision.decision_id.clone(),
        action: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
        mutations: vec![GovernanceAgentRegistryMutation {
            operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
            subject_node_id: "validator-3".to_string(),
        }],
        mutation_count: 1,
        previous_validators: context.previous_validators.to_vec(),
        new_validators: context.new_validators.to_vec(),
        previous_registry_root: context.previous_registry_root.to_string(),
        new_registry_root: context.new_registry_root.to_string(),
        evidence_refs: vec![
            GovernanceAgentEvidenceRef {
                kind: "frozen_evidence_snapshot".to_string(),
                root: context.evidence_snapshot_hash.to_string(),
            },
            GovernanceAgentEvidenceRef {
                kind: "validator_registry_snapshot".to_string(),
                root: evidence.validator_registry_root.clone(),
            },
            GovernanceAgentEvidenceRef {
                kind: "cobalt_evidence_packet".to_string(),
                root: evidence.cobalt_evidence_root.clone(),
            },
            GovernanceAgentEvidenceRef {
                kind: "operator_manifest_set".to_string(),
                root: evidence.operator_manifest_root.clone(),
            },
            GovernanceAgentEvidenceRef {
                kind: GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND.to_string(),
                root: evidence.validator_evidence_packet_root.clone(),
            },
        ],
        concentration_checks: vec![
            GovernanceAgentConcentrationCheck {
                name: "max_single_operator_cluster_share".to_string(),
                evidence_ref_kind: "operator_manifest_set".to_string(),
                observed_bps: 2_500,
                limit_bps: 5_000,
                passed: true,
            },
            GovernanceAgentConcentrationCheck {
                name: "max_single_cobalt_evidence_cluster_share".to_string(),
                evidence_ref_kind: "cobalt_evidence_packet".to_string(),
                observed_bps: 2_500,
                limit_bps: 5_000,
                passed: true,
            },
        ],
        linkedness_root,
        linkedness_passed: true,
        rollback_required: true,
        rollback_drill_required: true,
        routine_removal: false,
        human_approval_required: false,
        rationale: decision.rationale.clone(),
    };
    candidate.candidate_hash = governance_agent_guarded_apply_candidate_hash(&candidate)?;
    Ok(candidate)
}

fn governance_agent_guarded_apply_hard_caps() -> GovernanceAgentGuardedApplyHardCaps {
    GovernanceAgentGuardedApplyHardCaps {
        max_adds_per_round: 1,
        max_registry_mutations: 1,
        routine_removals_allowed: 0,
        evidence_refs_required: true,
        concentration_caps_required: true,
        linkedness_required: true,
        rollback_required: true,
        cobalt_acceptance_required: true,
        human_approval_after_activation_allowed: false,
    }
}

fn governance_agent_gate_9_5_registry(validators: &[String]) -> io::Result<ValidatorRegistry> {
    let mut records = validators
        .iter()
        .map(|validator| Ok::<_, io::Error>(governance_agent_gate_9_5_validator_record(validator)))
        .collect::<io::Result<Vec<_>>>()?;
    sort_validator_registry_records(&mut records);
    let registry = ValidatorRegistry {
        validators: records,
    };
    validate_validator_registry(&registry)?;
    Ok(registry)
}

fn governance_agent_gate_9_5_validator_record(node_id: &str) -> ValidatorRegistryRecord {
    let digest = hash_bytes(
        "postfiat.governance_agent.gate_9_5.validator_seed.v1",
        node_id.as_bytes(),
    );
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&digest[..32]);
    let key_pair = ml_dsa_65_keygen_from_seed(&seed);
    ValidatorRegistryRecord {
        node_id: node_id.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
    }
}

fn governance_agent_registry_entry(
    record: &ValidatorRegistryRecord,
    active: bool,
) -> ValidatorRegistryEntry {
    ValidatorRegistryEntry {
        node_id: record.node_id.clone(),
        algorithm_id: record.algorithm_id.clone(),
        public_key_hex: record.public_key_hex.clone(),
        active,
    }
}

fn validate_governance_agent_guarded_apply_candidate(
    candidate: &GovernanceAgentGuardedApplyCandidate,
    hard_caps: &GovernanceAgentGuardedApplyHardCaps,
) -> io::Result<()> {
    if candidate.schema != GOVERNANCE_AGENT_GUARDED_APPLY_CANDIDATE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance agent guarded-apply candidate schema",
        ));
    }
    validate_governance_agent_hash_hex("candidate_hash", &candidate.candidate_hash)?;
    if candidate.candidate_hash != governance_agent_guarded_apply_candidate_hash(candidate)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply candidate hash mismatch",
        ));
    }
    validate_governance_agent_hash_hex("ruleset_hash", &candidate.ruleset_hash)?;
    validate_governance_agent_hash_hex("compiled_policy_hash", &candidate.compiled_policy_hash)?;
    validate_governance_agent_hash_hex(
        "evidence_snapshot_hash",
        &candidate.evidence_snapshot_hash,
    )?;
    validate_governance_agent_hash_hex(
        "previous_registry_root",
        &candidate.previous_registry_root,
    )?;
    validate_governance_agent_hash_hex("new_registry_root", &candidate.new_registry_root)?;
    validate_governance_agent_hash_hex("linkedness_root", &candidate.linkedness_root)?;
    validate_governance_text_id("decision_id", &candidate.decision_id)?;
    validate_governance_text_id("guarded_apply_action", &candidate.action)?;
    if candidate.mutation_count != candidate.mutations.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate mutation_count mismatch",
        ));
    }
    if candidate.previous_validators.iter().collect::<BTreeSet<_>>().len()
        != candidate.previous_validators.len()
        || candidate.new_validators.iter().collect::<BTreeSet<_>>().len()
            != candidate.new_validators.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate validator scopes must be unique",
        ));
    }
    let mut add_count = 0u32;
    let mut routine_remove_count = 0u32;
    for mutation in &candidate.mutations {
        validate_governance_text_id("mutation subject_node_id", &mutation.subject_node_id)?;
        match mutation.operation.as_str() {
            VALIDATOR_REGISTRY_OP_ADMIT => add_count += 1,
            VALIDATOR_REGISTRY_OP_REMOVE => routine_remove_count += 1,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported guarded-apply mutation operation `{other}`"),
                ));
            }
        }
    }
    if candidate.action != VALIDATOR_REGISTRY_OP_ADMIT || add_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate must be a bounded admit",
        ));
    }
    if add_count > hard_caps.max_adds_per_round {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate exceeds max one add per round",
        ));
    }
    if candidate.mutation_count > hard_caps.max_registry_mutations as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate exceeds max registry mutations",
        ));
    }
    if candidate.routine_removal || routine_remove_count > hard_caps.routine_removals_allowed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate includes a routine removal",
        ));
    }
    if hard_caps.evidence_refs_required
        && !governance_agent_guarded_apply_evidence_refs_valid(candidate)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate missing valid evidence refs",
        ));
    }
    if hard_caps.concentration_caps_required
        && !governance_agent_guarded_apply_concentration_caps_passed(candidate)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate failed concentration caps",
        ));
    }
    if hard_caps.linkedness_required && !candidate.linkedness_passed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate failed linkedness",
        ));
    }
    if hard_caps.rollback_required && (!candidate.rollback_required || !candidate.rollback_drill_required)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate missing rollback requirement",
        ));
    }
    if !hard_caps.human_approval_after_activation_allowed && candidate.human_approval_required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guarded-apply candidate requires routine human approval",
        ));
    }
    Ok(())
}

fn governance_agent_guarded_apply_evidence_refs_valid(
    candidate: &GovernanceAgentGuardedApplyCandidate,
) -> bool {
    let required = [
        "frozen_evidence_snapshot",
        "validator_registry_snapshot",
        "cobalt_evidence_packet",
        "operator_manifest_set",
        GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND,
    ];
    let mut kinds = BTreeSet::new();
    for evidence_ref in &candidate.evidence_refs {
        if validate_governance_text_id("evidence_ref kind", &evidence_ref.kind).is_err()
            || validate_governance_agent_hash_hex("evidence_ref root", &evidence_ref.root).is_err()
        {
            return false;
        }
        kinds.insert(evidence_ref.kind.as_str());
    }
    required.iter().all(|kind| kinds.contains(kind))
}

fn governance_agent_guarded_apply_concentration_caps_passed(
    candidate: &GovernanceAgentGuardedApplyCandidate,
) -> bool {
    !candidate.concentration_checks.is_empty()
        && candidate.concentration_checks.iter().all(|check| {
            check.passed
                && check.observed_bps <= check.limit_bps
                && check.limit_bps <= 10_000
                && candidate
                    .evidence_refs
                    .iter()
                    .any(|evidence_ref| evidence_ref.kind == check.evidence_ref_kind)
        })
}

fn governance_agent_gate_9_5_rejection_checks(
    valid: &GovernanceAgentGuardedApplyCandidate,
    hard_caps: &GovernanceAgentGuardedApplyHardCaps,
) -> io::Result<Vec<GovernanceAgentGuardedApplyRejectionCheck>> {
    let mut more_than_one_add = valid.clone();
    more_than_one_add.mutations.push(GovernanceAgentRegistryMutation {
        operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
        subject_node_id: "validator-4".to_string(),
    });
    more_than_one_add.mutation_count = more_than_one_add.mutations.len();
    more_than_one_add.candidate_hash =
        governance_agent_guarded_apply_candidate_hash(&more_than_one_add)?;

    let mut routine_removal = valid.clone();
    routine_removal.action = VALIDATOR_REGISTRY_OP_REMOVE.to_string();
    routine_removal.mutations = vec![GovernanceAgentRegistryMutation {
        operation: VALIDATOR_REGISTRY_OP_REMOVE.to_string(),
        subject_node_id: "validator-1".to_string(),
    }];
    routine_removal.mutation_count = routine_removal.mutations.len();
    routine_removal.routine_removal = true;
    routine_removal.candidate_hash =
        governance_agent_guarded_apply_candidate_hash(&routine_removal)?;

    let mut linkedness_failure = valid.clone();
    linkedness_failure.linkedness_passed = false;
    linkedness_failure.candidate_hash =
        governance_agent_guarded_apply_candidate_hash(&linkedness_failure)?;

    let mut missing_evidence_refs = valid.clone();
    missing_evidence_refs.evidence_refs.clear();
    missing_evidence_refs.candidate_hash =
        governance_agent_guarded_apply_candidate_hash(&missing_evidence_refs)?;

    Ok(vec![
        governance_agent_gate_9_5_rejection_check(
            "more_than_one_add",
            &more_than_one_add,
            hard_caps,
        ),
        governance_agent_gate_9_5_rejection_check(
            "routine_removal",
            &routine_removal,
            hard_caps,
        ),
        governance_agent_gate_9_5_rejection_check(
            "linkedness_failure",
            &linkedness_failure,
            hard_caps,
        ),
        governance_agent_gate_9_5_rejection_check(
            "missing_evidence_refs",
            &missing_evidence_refs,
            hard_caps,
        ),
    ])
}

fn governance_agent_gate_9_5_rejection_check(
    name: &str,
    candidate: &GovernanceAgentGuardedApplyCandidate,
    hard_caps: &GovernanceAgentGuardedApplyHardCaps,
) -> GovernanceAgentGuardedApplyRejectionCheck {
    match validate_governance_agent_guarded_apply_candidate(candidate, hard_caps) {
        Ok(()) => GovernanceAgentGuardedApplyRejectionCheck {
            name: name.to_string(),
            rejected: false,
            error: None,
        },
        Err(error) => GovernanceAgentGuardedApplyRejectionCheck {
            name: name.to_string(),
            rejected: true,
            error: Some(error.to_string()),
        },
    }
}

fn governance_agent_guarded_apply_rejection_check_passed(
    checks: &[GovernanceAgentGuardedApplyRejectionCheck],
    name: &str,
) -> bool {
    checks
        .iter()
        .any(|check| check.name == name && check.rejected && check.error.is_some())
}

fn governance_agent_guarded_apply_candidate_hash(
    candidate: &GovernanceAgentGuardedApplyCandidate,
) -> io::Result<String> {
    let mut value = serde_json::to_value(candidate).map_err(invalid_data)?;
    let object = value.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance guarded-apply candidate must encode as a JSON object",
        )
    })?;
    object.remove("candidate_hash");
    governance_agent_canonical_json_hash(
        "postfiat.governance_agent.guarded_apply_candidate.v1",
        &value,
    )
}
