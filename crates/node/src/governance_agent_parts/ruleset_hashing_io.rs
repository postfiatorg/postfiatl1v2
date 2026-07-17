fn validate_governance_ruleset_text(raw: &str) -> io::Result<serde_json::Value> {
    validate_governance_ruleset_text_with_ruleset(raw).map(|(value, _ruleset)| value)
}

fn validate_governance_ruleset_text_with_ruleset(
    raw: &str,
) -> io::Result<(serde_json::Value, GovernanceRuleset)> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(invalid_data)?;
    let ruleset = validate_governance_ruleset_value(&value)?;
    Ok((value, ruleset))
}

fn validate_governance_ruleset_value(value: &serde_json::Value) -> io::Result<GovernanceRuleset> {
    let ruleset: GovernanceRuleset = serde_json::from_value(value.clone()).map_err(invalid_data)?;
    validate_governance_text_id("ruleset_id", &ruleset.ruleset_id)?;
    if ruleset.schema != GOVERNANCE_AGENT_RULESET_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance ruleset schema",
        ));
    }
    if ruleset.scope != "validator_registry_policy" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset scope must be validator_registry_policy",
        ));
    }
    validate_governance_ruleset_authority(&ruleset.authority)?;
    validate_governance_ruleset_inputs(&ruleset.inputs)?;
    validate_governance_ruleset_decisions(&ruleset.decisions)?;
    validate_governance_ruleset_evidence_input_binding(&ruleset.inputs, &ruleset.decisions)?;
    validate_governance_ruleset_rollback(&ruleset.rollback)?;
    Ok(ruleset)
}

fn validate_governance_ruleset_authority(
    authority: &GovernanceRulesetAuthority,
) -> io::Result<()> {
    if authority.mode != "dry_run_only" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset authority mode must be dry_run_only",
        ));
    }
    if authority.direct_state_mutation {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset authority allows direct state mutation",
        ));
    }
    if authority.self_upgrade {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset authority allows self upgrade",
        ));
    }
    if authority.scope_expansion {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset authority allows scope expansion",
        ));
    }
    Ok(())
}

fn validate_governance_ruleset_inputs(inputs: &[GovernanceRulesetInput]) -> io::Result<()> {
    if inputs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset requires at least one input declaration",
        ));
    }
    let mut seen = BTreeSet::new();
    for input in inputs {
        if !matches!(
            input.kind.as_str(),
            GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND
                | "validator_registry_snapshot"
                | "cobalt_evidence_packet"
                | "operator_manifest_set"
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported governance ruleset input kind `{}`", input.kind),
            ));
        }
        if !seen.insert(input.kind.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance ruleset input kind",
            ));
        }
        let _required = input.required;
    }
    Ok(())
}

fn validate_governance_ruleset_evidence_input_binding(
    inputs: &[GovernanceRulesetInput],
    decisions: &[GovernanceRulesetDecision],
) -> io::Result<()> {
    let cites_validator_evidence = decisions
        .iter()
        .any(|decision| !decision.evidence_field_path.is_empty());
    if !cites_validator_evidence {
        return Ok(());
    }
    let has_required_packet_input = inputs.iter().any(|input| {
        input.kind == GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND && input.required
    });
    if has_required_packet_input {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "governance ruleset decisions cite validator evidence but do not require validator_evidence_packet input",
    ))
}

fn validate_governance_ruleset_decisions(
    decisions: &[GovernanceRulesetDecision],
) -> io::Result<()> {
    if decisions.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset requires at least one decision",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut has_no_op = false;
    let registered_evidence_fields = governance_agent_registered_evidence_field_paths()?;
    for decision in decisions {
        validate_governance_text_id("decision_id", &decision.decision_id)?;
        validate_governance_text_id("condition", &decision.condition)?;
        validate_governance_text_id("rationale", &decision.rationale)?;
        validate_governance_ruleset_evidence_field_path(
            &decision.evidence_field_path,
            &registered_evidence_fields,
        )?;
        validate_governance_ruleset_required_provenance(&decision.required_provenance)?;
        validate_governance_ruleset_freshness_requirement(&decision.freshness_requirement)?;
        validate_governance_ruleset_missing_evidence_behavior(
            &decision.missing_evidence_behavior,
        )?;
        validate_governance_ruleset_conflict_behavior(&decision.conflict_behavior)?;
        validate_governance_ruleset_action_bound(&decision.action_bound)?;
        if !seen.insert(decision.decision_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance ruleset decision id",
            ));
        }
        match decision.kind.as_str() {
            "no_op" => {
                if decision.action_bound != "no_action" {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "governance no_op decisions must declare action_bound no_action",
                    ));
                }
                has_no_op = true;
            }
            "registry_delta_candidate" => {
                if matches!(
                    decision.action_bound.as_str(),
                    "informational_only" | "no_action"
                ) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "governance registry_delta_candidate decisions require an action-bearing bound",
                    ));
                }
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported governance ruleset decision kind `{other}`"),
                ));
            }
        }
    }
    if !has_no_op {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset must include an explicit no_op decision",
        ));
    }
    Ok(())
}

fn governance_agent_registered_evidence_field_paths() -> io::Result<BTreeSet<String>> {
    let schema: serde_json::Value =
        serde_json::from_str(GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_PACKET_SCHEMA_JSON)
            .map_err(invalid_data)?;
    let values = schema
        .pointer("/$defs/field_path/enum")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "validator evidence packet schema is missing field_path enum",
            )
        })?;
    let mut paths = BTreeSet::new();
    for value in values {
        let path = value.as_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "validator evidence field_path enum contains a non-string value",
            )
        })?;
        if !paths.insert(path.to_string()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "validator evidence field_path enum contains a duplicate path",
            ));
        }
    }
    Ok(paths)
}

fn validate_governance_ruleset_evidence_field_path(
    field_path: &str,
    registered_evidence_fields: &BTreeSet<String>,
) -> io::Result<()> {
    if registered_evidence_fields.contains(field_path) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unregistered governance evidence field path `{field_path}`"),
        ))
    }
}

fn validate_governance_ruleset_required_provenance(provenance: &str) -> io::Result<()> {
    if matches!(
        provenance,
        "chain_derived"
            | "registry_derived"
            | "operator_signed"
            | "collector_observed"
            | "network_observed"
            | "self_asserted"
            | "third_party_attested"
    ) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported governance required_provenance `{provenance}`"),
        ))
    }
}

fn validate_governance_ruleset_freshness_requirement(freshness: &str) -> io::Result<()> {
    if matches!(
        freshness,
        "same_packet" | "recent_24h" | "recent_7d" | "recent_30d" | "epoch_bound" | "historical"
    ) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported governance freshness_requirement `{freshness}`"),
        ))
    }
}

fn validate_governance_ruleset_missing_evidence_behavior(behavior: &str) -> io::Result<()> {
    if matches!(
        behavior,
        "reject_packet" | "reject_validator_entry" | "hold" | "neutral" | "stale"
    ) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported governance missing_evidence_behavior `{behavior}`"),
        ))
    }
}

fn validate_governance_ruleset_conflict_behavior(behavior: &str) -> io::Result<()> {
    if matches!(
        behavior,
        "reject_packet" | "reject_validator_entry" | "hold"
    ) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported governance conflict_behavior `{behavior}`"),
        ))
    }
}

fn validate_governance_ruleset_action_bound(action_bound: &str) -> io::Result<()> {
    if matches!(
        action_bound,
        "informational_only"
            | "score_adjustment"
            | "admission_gate"
            | "hold_only"
            | "suspend_candidate"
            | "remove_candidate"
            | "no_action"
    ) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported governance action_bound `{action_bound}`"),
        ))
    }
}

fn validate_governance_ruleset_rollback(
    rollback: &GovernanceRulesetRollback,
) -> io::Result<()> {
    if !rollback.required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset rollback evidence must be required",
        ));
    }
    if rollback.max_mutations != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 1.5 governance ruleset rollback max_mutations must be zero",
        ));
    }
    if !rollback.operator_confirmation_required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance ruleset rollback must require operator confirmation",
        ));
    }
    Ok(())
}

fn validate_governance_text_id(label: &str, value: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > 256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be 1..=256 bytes"),
        ));
    }
    if value.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must not contain control bytes"),
        ));
    }
    Ok(())
}

fn hash_governance_agent_statement(
    agent_dir: &Path,
    file_name: &str,
    name: &str,
) -> io::Result<GovernanceAgentStatementHash> {
    let path = agent_dir.join(file_name);
    let bytes = read_bounded_governance_agent_bytes(&path, "governance agent statement")?;
    let hash = governance_agent_statement_hash(name, &bytes);
    Ok(GovernanceAgentStatementHash {
        name: name.to_string(),
        path: path.display().to_string(),
        hash,
        byte_len: bytes.len(),
    })
}

fn governance_agent_statement_hash(name: &str, bytes: &[u8]) -> String {
    hash_hex(
        &format!("postfiat.governance_agent.statement.{name}.v1"),
        bytes,
    )
}

fn governance_agent_statement_hash_one_byte_edit_detected(
    path: PathBuf,
    name: &str,
    original_hash: &str,
) -> io::Result<bool> {
    let mut bytes = read_bounded_governance_agent_bytes(&path, "governance agent statement")?;
    bytes.push(b'\n');
    Ok(governance_agent_statement_hash(name, &bytes) != original_hash)
}

fn read_governance_agent_json_value(
    path: &Path,
    label: &str,
) -> io::Result<serde_json::Value> {
    read_json_file(path, label)
}

fn read_bounded_governance_agent_bytes(path: &Path, label: &str) -> io::Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to stat {label} `{}`: {error}", path.display()),
        )
    })?;
    if metadata.len() > MAX_LOCAL_JSON_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes", path.display()),
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to read {label} `{}`: {error}", path.display()),
        )
    })?;
    if bytes.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} `{}` exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes", path.display()),
        ));
    }
    Ok(bytes)
}

fn governance_agent_canonical_json_hash(
    domain: &str,
    value: &serde_json::Value,
) -> io::Result<String> {
    let bytes = governance_agent_canonical_json_bytes(value)?;
    Ok(hash_hex(domain, &bytes))
}

fn governance_agent_sha384_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha384::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    bytes_to_hex(&digest)
}

fn governance_agent_canonical_json_sha384_hash(value: &serde_json::Value) -> io::Result<String> {
    let bytes = governance_agent_canonical_json_bytes(value)?;
    Ok(governance_agent_sha384_hex(&bytes))
}

fn governance_agent_generation_output_files(outputs_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(outputs_dir).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "failed to read governance agent generation outputs dir `{}`: {error}",
                outputs_dir.display()
            ),
        )
    })? {
        let entry = entry.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to read governance agent generation output entry in `{}`: {error}",
                    outputs_dir.display()
                ),
            )
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("model_output_") && name.ends_with(".json") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn governance_agent_compiled_policy_hash(
    ruleset: &GovernanceRuleset,
    ruleset_hash: &str,
) -> io::Result<String> {
    let input_kinds = ruleset
        .inputs
        .iter()
        .map(|input| input.kind.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut decisions = ruleset
        .decisions
        .iter()
        .map(|decision| {
            serde_json::json!({
                "action_bound": decision.action_bound,
                "condition": decision.condition,
                "conflict_behavior": decision.conflict_behavior,
                "decision_id": decision.decision_id,
                "evidence_field_path": decision.evidence_field_path,
                "freshness_requirement": decision.freshness_requirement,
                "kind": decision.kind,
                "missing_evidence_behavior": decision.missing_evidence_behavior,
                "required_provenance": decision.required_provenance,
                "rationale": decision.rationale
            })
        })
        .collect::<Vec<_>>();
    decisions.sort_by(|left, right| {
        left.get("decision_id")
            .and_then(serde_json::Value::as_str)
            .cmp(&right.get("decision_id").and_then(serde_json::Value::as_str))
    });
    let payload = serde_json::json!({
        "authority": {
            "direct_state_mutation": ruleset.authority.direct_state_mutation,
            "mode": ruleset.authority.mode,
            "scope_expansion": ruleset.authority.scope_expansion,
            "self_upgrade": ruleset.authority.self_upgrade
        },
        "decisions": decisions,
        "input_kinds": input_kinds,
        "rollback": {
            "max_mutations": ruleset.rollback.max_mutations,
            "operator_confirmation_required": ruleset.rollback.operator_confirmation_required,
            "required": ruleset.rollback.required
        },
        "ruleset_hash": ruleset_hash,
        "ruleset_id": ruleset.ruleset_id,
        "sandbox": {
            "arbitrary_files": false,
            "direct_state_mutation": false,
            "model": false,
            "network": false
        },
        "schema": GOVERNANCE_AGENT_COMPILED_POLICY_SCHEMA,
        "scope": ruleset.scope
    });
    governance_agent_canonical_json_hash("postfiat.governance_agent.compiled_policy.v1", &payload)
}

fn governance_agent_bundle_hash(
    architecture_statement_hash: &str,
    objective_statement_hash: &str,
    constitutional_constraints_hash: &str,
    ruleset_schema_hash: &str,
    valid_ruleset_fixture_hash: &str,
) -> io::Result<String> {
    let payload = serde_json::json!({
        "architecture_statement_hash": architecture_statement_hash,
        "constitutional_constraints_hash": constitutional_constraints_hash,
        "deterministic_flags": [
            "temperature=0",
            "top_p=1",
            "json_only=true",
            "network_reads=false",
            "wall_clock_time=false"
        ],
        "model_id": GOVERNANCE_AGENT_MODEL_ID,
        "objective_statement_hash": objective_statement_hash,
        "rollback_policy": {
            "direct_state_mutation": false,
            "max_registry_mutations": 1,
            "operator_confirmation_required": false
        },
        "ruleset_schema_hash": ruleset_schema_hash,
        "schema": GOVERNANCE_AGENT_BUNDLE_SCHEMA,
        "runtime": GOVERNANCE_AGENT_RUNTIME,
        "valid_ruleset_fixture_hash": valid_ruleset_fixture_hash
    });
    governance_agent_canonical_json_hash("postfiat.governance_agent.bundle.v1", &payload)
}

fn governance_agent_model_request_hash(
    bundle_hash: &str,
    objective_statement_hash: &str,
    ruleset_schema_hash: &str,
    round_seed_input: &GovernanceAgentRoundSeedInput,
    round_seed: &str,
) -> io::Result<String> {
    let payload = serde_json::json!({
        "bundle_hash": bundle_hash,
        "evidence_root": GOVERNANCE_AGENT_EVIDENCE_ROOT,
        "json_only": true,
        "objective_statement_hash": objective_statement_hash,
        "output_schema_hash": ruleset_schema_hash,
        "round_seed_input": round_seed_input,
        "round_seed": round_seed,
        "schema": GOVERNANCE_AGENT_MODEL_REQUEST_SCHEMA
    });
    governance_agent_canonical_json_hash("postfiat.governance_agent.model_request.v1", &payload)
}

fn governance_agent_key_order_check() -> io::Result<bool> {
    let left: serde_json::Value = serde_json::from_str(r#"{"b":2,"a":1,"nested":{"z":0,"m":1}}"#)
        .map_err(invalid_data)?;
    let right: serde_json::Value =
        serde_json::from_str(r#"{"nested":{"m":1,"z":0},"a":1,"b":2}"#)
            .map_err(invalid_data)?;
    Ok(governance_agent_canonical_json_bytes(&left)?
        == governance_agent_canonical_json_bytes(&right)?)
}

fn write_governance_agent_canonical_json(
    value: &serde_json::Value,
    output: &mut Vec<u8>,
) -> io::Result<()> {
    match value {
        serde_json::Value::Null => output.extend_from_slice(b"null"),
        serde_json::Value::Bool(true) => output.extend_from_slice(b"true"),
        serde_json::Value::Bool(false) => output.extend_from_slice(b"false"),
        serde_json::Value::Number(number) => {
            if number.is_f64() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "floating-point JSON numbers are not allowed in governance agent canonical JSON",
                ));
            }
            output.extend_from_slice(number.to_string().as_bytes());
        }
        serde_json::Value::String(text) => {
            let encoded = serde_json::to_string(text).map_err(invalid_data)?;
            output.extend_from_slice(encoded.as_bytes());
        }
        serde_json::Value::Array(items) => {
            output.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_governance_agent_canonical_json(item, output)?;
            }
            output.push(b']');
        }
        serde_json::Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(key, _)| *key);
            output.push(b'{');
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                let encoded_key = serde_json::to_string(key).map_err(invalid_data)?;
                output.extend_from_slice(encoded_key.as_bytes());
                output.push(b':');
                write_governance_agent_canonical_json(item, output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn ensure_governance_agent_report_redacted(report_json: &str) -> io::Result<()> {
    const SENSITIVE_MARKERS: &[&str] = &[
        "private_key_hex",
        "seed_hex",
        "\"mnemonic\"",
        "mnemonic=",
        "ssh_cred",
        "begin private key",
        "runpod_api_key",
        "vast_api_key",
    ];
    let lower = report_json.to_ascii_lowercase();
    for marker in SENSITIVE_MARKERS {
        if lower.contains(marker) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("governance agent report contains sensitive marker `{marker}`"),
            ));
        }
    }
    Ok(())
}

struct GovernanceAgentBundleValidation {
    statement_hashes: Vec<GovernanceAgentStatementHash>,
    ruleset_schema_hash: String,
    valid_fixture: GovernanceAgentFixtureCheck,
    invalid_fixtures: Vec<GovernanceAgentFixtureCheck>,
    canonical_json_key_order_stable: bool,
    statement_hash_one_byte_edit_detected: bool,
    bundle_hash: String,
    model_request_hash: String,
}
