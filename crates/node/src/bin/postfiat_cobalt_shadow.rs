use std::env;
use std::fs;
use std::io;
use std::net::TcpListener;
use std::path::PathBuf;

use postfiat_consensus_cobalt::{DabcRatifiedAmendment, RbcPropose};
use postfiat_node::cobalt_shadow::{
    assemble_protocol_transcript_at_activation_height_extending, build_registry_binding_manifest,
    run_cobalt_shadow_adversarial_drill, CobaltShadowHistoryRange, CobaltShadowIdentity,
    CobaltShadowLimits, CobaltShadowProtocolContribution, CobaltShadowRegistryBinding,
    CobaltShadowService, CobaltShadowValidatorBinding,
};
use postfiat_node::cobalt_shadow_runtime::{
    compressed_commit_request, parse_endpoint, read_transcript, request,
    run_cobalt_shadow_network_drill, serve_listener, validate_listen_address,
    CobaltShadowRpcRequest,
};
use postfiat_node::ValidatorRegistry;
use postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED;
use serde::Deserialize;

fn main() {
    if let Err(error) = run() {
        eprintln!("Cobalt shadow service failed: {error}");
        std::process::exit(2);
    }
}

fn run() -> io::Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    let output = match command {
        "init" => {
            let data_dir = required_path(&args, "--data-dir")?;
            let protocol_version = optional_flag(&args, "--protocol-version")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| invalid("--protocol-version must be an integer"))?;
            let service = CobaltShadowService::initialize(
                data_dir,
                CobaltShadowIdentity {
                    node_id: required_flag(&args, "--node-id")?.to_string(),
                    chain_id: required_flag(&args, "--chain-id")?.to_string(),
                    genesis_hash: required_flag(&args, "--genesis-hash")?.to_string(),
                    protocol_version,
                },
                CobaltShadowLimits::default(),
            )?;
            serde_json::to_value(service.status()).map_err(json_error)?
        }
        "status" => {
            let status = CobaltShadowService::inspect(required_path(&args, "--data-dir")?)?;
            serde_json::to_value(status).map_err(json_error)?
        }
        "validator-binding" => {
            let service = CobaltShadowService::open(required_path(&args, "--data-dir")?)?;
            let binding = service.create_validator_binding(
                required_flag(&args, "--registry-root")?,
                &required_path(&args, "--validator-key-file")?,
            )?;
            serde_json::to_value(binding).map_err(json_error)?
        }
        "build-binding" => {
            let quorum = required_flag(&args, "--quorum")?
                .parse::<usize>()
                .map_err(|_| invalid("--quorum must be an integer"))?;
            let activation_height = required_flag(&args, "--activation-height")?
                .parse::<u64>()
                .map_err(|_| invalid("--activation-height must be an integer"))?;
            let validator_registry: ValidatorRegistry = read_bounded_json(
                &required_path(&args, "--validator-registry")?,
                2 * 1024 * 1024,
            )?;
            let validator_bindings: Vec<CobaltShadowValidatorBinding> = read_bounded_json(
                &required_path(&args, "--validator-bindings")?,
                4 * 1024 * 1024,
            )?;
            let binding = build_registry_binding_manifest(
                required_flag(&args, "--registry-root")?,
                validator_registry,
                validator_bindings,
                quorum,
                activation_height,
            )?;
            serde_json::to_value(binding).map_err(json_error)?
        }
        "bind" => {
            let mut service = CobaltShadowService::open(required_path(&args, "--data-dir")?)?;
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                2 * 1024 * 1024,
            )?;
            service.bind_registry_manifest(&binding)?;
            serde_json::to_value(service.status()).map_err(json_error)?
        }
        "authority-lineage-reset" => {
            let data_dir = required_path(&args, "--data-dir")?;
            let node_data_dir = required_path(&args, "--node-data-dir")?;
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let store = postfiat_storage::NodeStore::try_new(&node_data_dir)?;
            let genesis = store.read_genesis()?;
            let governance = store.read_governance()?;
            let registry: ValidatorRegistry = read_bounded_json(
                &node_data_dir.join("validator_registry.json"),
                2 * 1024 * 1024,
            )?;
            if governance.authority_mode != GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED {
                return Err(invalid(
                    "authority lineage reset requires an active Cobalt authority epoch",
                ));
            }
            let transition = governance
                .cobalt_authority_transitions
                .last()
                .cloned()
                .ok_or_else(|| invalid("active Cobalt authority has no transition record"))?;
            if transition.transition_kind != postfiat_types::COBALT_AUTHORITY_TRANSITION_ACTIVATE {
                return Err(invalid(
                    "authority lineage reset requires the latest transition to activate Cobalt",
                ));
            }
            let mut prior_governance = governance.clone();
            let removed = prior_governance
                .cobalt_authority_transitions
                .pop()
                .ok_or_else(|| invalid("active Cobalt authority has no transition record"))?;
            if removed.transition_id != transition.transition_id {
                return Err(invalid("authority transition history changed during reset"));
            }
            prior_governance.authority_mode = transition.from_authority_mode;
            postfiat_node::cobalt_handoff::verify_cobalt_authority_history(
                &genesis,
                &prior_governance,
            )?;
            postfiat_node::cobalt_handoff::verify_cobalt_authority_transition(
                &genesis,
                &prior_governance,
                &registry,
                &transition,
                transition.activation_height,
            )?;
            let governance_ratification_anchor = governance
                .validator_registry_updates
                .iter()
                .rev()
                .find(|update| !update.cobalt_authorizations.is_empty())
                .map(|update| {
                    postfiat_node::cobalt_authority_certificate::cobalt_decision_ratification(
                        update.cobalt_decision_certificate.as_ref().ok_or_else(|| {
                            invalid("stored Cobalt validator update has no decision certificate")
                        })?,
                    )
                })
                .transpose()?;
            let mut service = CobaltShadowService::open_for_authority_lineage_reset(data_dir)?;
            let receipt = service.reset_authority_lineage(
                &binding,
                &transition,
                governance_ratification_anchor.as_ref(),
                required_path(&args, "--archive-dir")?,
            )?;
            serde_json::to_value(receipt).map_err(json_error)?
        }
        "registry-lineage-reset" => {
            let data_dir = required_path(&args, "--data-dir")?;
            let node_data_dir = required_path(&args, "--node-data-dir")?;
            let previous_binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--previous-registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let next_binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let store = postfiat_storage::NodeStore::try_new(&node_data_dir)?;
            let genesis = store.read_genesis()?;
            let governance = store.read_governance()?;
            let live_status = postfiat_node::status(postfiat_node::NodeOptions {
                data_dir: node_data_dir.clone(),
            })?;
            let live_registry: ValidatorRegistry = read_bounded_json(
                &node_data_dir.join("validator_registry.json"),
                2 * 1024 * 1024,
            )?;
            if governance.authority_mode != GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED {
                return Err(invalid(
                    "registry lineage reset requires active Cobalt authority",
                ));
            }
            postfiat_node::cobalt_handoff::verify_cobalt_authority_history(&genesis, &governance)?;
            let update = governance
                .validator_registry_updates
                .last()
                .ok_or_else(|| invalid("registry lineage reset requires a live validator update"))?
                .clone();
            if update.activation_height > live_status.block_height
                || update.new_registry_root != next_binding.registry_root
                || live_registry != next_binding.validator_registry
            {
                return Err(invalid(
                    "registry lineage reset binding does not match live consensus state",
                ));
            }
            let mut service = CobaltShadowService::open_for_registry_lineage_reset(
                data_dir,
                &previous_binding,
                &next_binding,
            )?;
            let receipt = service.reset_registry_lineage(
                &previous_binding,
                &next_binding,
                &update,
                required_path(&args, "--archive-dir")?,
            )?;
            serde_json::to_value(receipt).map_err(json_error)?
        }
        "propose" => {
            let mut service = CobaltShadowService::open(required_path(&args, "--data-dir")?)?;
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let round = required_flag(&args, "--round")?
                .parse::<u64>()
                .map_err(|_| invalid("--round must be an integer"))?;
            let proposal = service.create_protocol_proposal(
                &binding,
                round,
                required_flag(&args, "--payload-hash")?,
            )?;
            serde_json::to_value(proposal).map_err(json_error)?
        }
        "propose-rpc" => {
            let endpoint = parse_endpoint(required_flag(&args, "--endpoint")?)?;
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let round = required_flag(&args, "--round")?
                .parse::<u64>()
                .map_err(|_| invalid("--round must be an integer"))?;
            request(
                endpoint,
                &CobaltShadowRpcRequest::CreateProposal {
                    binding: Box::new(binding),
                    round,
                    payload_hash: required_flag(&args, "--payload-hash")?.to_string(),
                },
            )?
        }
        "contribute" => {
            let mut service = CobaltShadowService::open(required_path(&args, "--data-dir")?)?;
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let proposal: RbcPropose =
                read_bounded_json(&required_path(&args, "--proposal")?, 2 * 1024 * 1024)?;
            let activation_height = optional_flag(&args, "--activation-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| invalid("--activation-height must be an integer"))
                })
                .transpose()?;
            let contribution = match activation_height {
                Some(activation_height) => service
                    .create_protocol_contribution_at_activation_height(
                        &binding,
                        &proposal,
                        activation_height,
                    )?,
                None => service.create_protocol_contribution(&binding, &proposal)?,
            };
            serde_json::to_value(contribution).map_err(json_error)?
        }
        "contribute-rpc" => {
            let endpoint = parse_endpoint(required_flag(&args, "--endpoint")?)?;
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let proposal: RbcPropose =
                read_bounded_json(&required_path(&args, "--proposal")?, 2 * 1024 * 1024)?;
            let activation_height = optional_flag(&args, "--activation-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| invalid("--activation-height must be an integer"))
                })
                .transpose()?;
            request(
                endpoint,
                &CobaltShadowRpcRequest::CreateContribution {
                    binding: Box::new(binding),
                    propose: Box::new(proposal),
                    activation_height,
                },
            )?
        }
        "assemble" => {
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let proposal: RbcPropose =
                read_bounded_json(&required_path(&args, "--proposal")?, 2 * 1024 * 1024)?;
            let contributions: Vec<CobaltShadowProtocolContribution> =
                read_bounded_json(&required_path(&args, "--contributions")?, 16 * 1024 * 1024)?;
            let activation_height = required_flag(&args, "--activation-height")?
                .parse::<u64>()
                .map_err(|_| invalid("--activation-height must be an integer"))?;
            let previous = optional_flag(&args, "--previous-ratification")
                .map(|path| {
                    read_bounded_json::<DabcRatifiedAmendment>(
                        &PathBuf::from(path),
                        2 * 1024 * 1024,
                    )
                })
                .transpose()?;
            let transcript = assemble_protocol_transcript_at_activation_height_extending(
                &binding,
                proposal,
                contributions,
                previous.as_ref(),
                activation_height,
            )?;
            serde_json::to_value(transcript).map_err(json_error)?
        }
        "reserve" => {
            let mut service = CobaltShadowService::open(required_path(&args, "--data-dir")?)?;
            let round = required_flag(&args, "--round")?
                .parse::<u64>()
                .map_err(|_| invalid("--round must be an integer"))?;
            service.reserve_protocol_round(
                round,
                required_flag(&args, "--payload-hash")?.to_string(),
            )?;
            serde_json::to_value(service.status()).map_err(json_error)?
        }
        "run" => {
            let data_dir = required_path(&args, "--data-dir")?;
            let listen = parse_endpoint(required_flag(&args, "--listen")?)?;
            validate_listen_address(listen, flag_present(&args, "--allow-private-network"))?;
            let service = CobaltShadowService::open(data_dir)?;
            let listener = TcpListener::bind(listen)?;
            let bound = listener.local_addr()?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "schema": "postfiat-cobalt-shadow-listener-v1",
                    "status": "ready",
                    "listen": bound.to_string(),
                    "live_authority": false,
                    "controls_block_consensus": false
                }))
                .map_err(json_error)?
            );
            let _service = serve_listener(service, listener, false)?;
            return Ok(());
        }
        "probe" | "snapshot" | "replay" => {
            let endpoint = parse_endpoint(required_flag(&args, "--endpoint")?)?;
            let request_body = match command {
                "probe" => CobaltShadowRpcRequest::Probe,
                "snapshot" => CobaltShadowRpcRequest::Snapshot,
                "replay" => CobaltShadowRpcRequest::Replay,
                _ => unreachable!(),
            };
            request(endpoint, &request_body)?
        }
        "history-export" => {
            let endpoint = parse_endpoint(required_flag(&args, "--endpoint")?)?;
            let start_sequence = required_flag(&args, "--start-sequence")?
                .parse::<u64>()
                .map_err(|_| invalid("--start-sequence must be an integer"))?;
            let limit = required_flag(&args, "--limit")?
                .parse::<usize>()
                .map_err(|_| invalid("--limit must be an integer"))?;
            request(
                endpoint,
                &CobaltShadowRpcRequest::HistoryRange {
                    start_sequence,
                    limit,
                },
            )?
        }
        "history-verify" => {
            let service = CobaltShadowService::open(required_path(&args, "--data-dir")?)?;
            let range: CobaltShadowHistoryRange =
                read_bounded_json(&required_path(&args, "--range")?, 16 * 1024 * 1024)?;
            service.verify_history_range(&range)?;
            serde_json::json!({
                "verified": true,
                "start_sequence": range.start_sequence,
                "end_sequence": range.end_sequence,
                "range_hash": range.range_hash,
            })
        }
        "catch-up" => {
            let source = parse_endpoint(required_flag(&args, "--source-endpoint")?)?;
            let target = parse_endpoint(required_flag(&args, "--target-endpoint")?)?;
            let start_sequence = required_flag(&args, "--start-sequence")?
                .parse::<u64>()
                .map_err(|_| invalid("--start-sequence must be an integer"))?;
            let limit = required_flag(&args, "--limit")?
                .parse::<usize>()
                .map_err(|_| invalid("--limit must be an integer"))?;
            let range: CobaltShadowHistoryRange = serde_json::from_value(request(
                source,
                &CobaltShadowRpcRequest::HistoryRange {
                    start_sequence,
                    limit,
                },
            )?)
            .map_err(json_error)?;
            request(
                target,
                &CobaltShadowRpcRequest::CatchUp {
                    range: Box::new(range),
                },
            )?
        }
        "commit" => {
            let endpoint = parse_endpoint(required_flag(&args, "--endpoint")?)?;
            let transcript = read_transcript(&required_path(&args, "--transcript")?)?;
            request(endpoint, &compressed_commit_request(&transcript)?)?
        }
        "drill" => {
            let report = run_cobalt_shadow_adversarial_drill(required_path(&args, "--data-dir")?)?;
            if !report.ok {
                return Err(invalid("adversarial drill did not converge"));
            }
            serde_json::to_value(report).map_err(json_error)?
        }
        "network-drill" => {
            let report = run_cobalt_shadow_network_drill(required_path(&args, "--data-dir")?)?;
            if !report.ok {
                return Err(invalid("network drill did not converge"));
            }
            serde_json::to_value(report).map_err(json_error)?
        }
        "help" | "--help" | "-h" => {
            println!("{}", usage_text());
            return Ok(());
        }
        _ => return Err(usage()),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(json_error)?
    );
    Ok(())
}

fn optional_flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

fn flag_present(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn required_flag<'a>(args: &'a [String], name: &str) -> io::Result<&'a str> {
    optional_flag(args, name).ok_or_else(|| invalid(format!("{name} is required")))
}

fn required_path(args: &[String], name: &str) -> io::Result<PathBuf> {
    required_flag(args, name).map(PathBuf::from)
}

fn usage() -> io::Error {
    invalid(usage_text())
}

fn usage_text() -> &'static str {
    "usage:
  postfiat-cobalt-shadow init --data-dir PATH --node-id ID --chain-id ID --genesis-hash HASH [--protocol-version N]
  postfiat-cobalt-shadow status --data-dir PATH
  postfiat-cobalt-shadow validator-binding --data-dir PATH --registry-root HASH --validator-key-file PATH
  postfiat-cobalt-shadow build-binding --registry-root HASH --validator-registry PATH --validator-bindings PATH --quorum N --activation-height N
  postfiat-cobalt-shadow bind --data-dir PATH --registry-binding PATH
  postfiat-cobalt-shadow authority-lineage-reset --data-dir PATH --node-data-dir PATH --registry-binding PATH --archive-dir PATH
  postfiat-cobalt-shadow registry-lineage-reset --data-dir PATH --node-data-dir PATH --previous-registry-binding PATH --registry-binding PATH --archive-dir PATH
  postfiat-cobalt-shadow propose --data-dir PATH --registry-binding PATH --round N --payload-hash HASH
  postfiat-cobalt-shadow propose-rpc --endpoint IP:PORT --registry-binding PATH --round N --payload-hash HASH
  postfiat-cobalt-shadow contribute --data-dir PATH --registry-binding PATH --proposal PATH [--activation-height N]
  postfiat-cobalt-shadow contribute-rpc --endpoint IP:PORT --registry-binding PATH --proposal PATH [--activation-height N]
  postfiat-cobalt-shadow assemble --registry-binding PATH --proposal PATH --contributions PATH --activation-height N [--previous-ratification PATH]
  postfiat-cobalt-shadow reserve --data-dir PATH --round N --payload-hash HASH
  postfiat-cobalt-shadow run --data-dir PATH --listen IP:PORT [--allow-private-network]
  postfiat-cobalt-shadow probe --endpoint IP:PORT
  postfiat-cobalt-shadow snapshot --endpoint IP:PORT
  postfiat-cobalt-shadow replay --endpoint IP:PORT
  postfiat-cobalt-shadow history-export --endpoint IP:PORT --start-sequence N --limit N
  postfiat-cobalt-shadow history-verify --data-dir PATH --range PATH
  postfiat-cobalt-shadow catch-up --source-endpoint IP:PORT --target-endpoint IP:PORT --start-sequence N --limit N
  postfiat-cobalt-shadow commit --endpoint IP:PORT --transcript PATH
  postfiat-cobalt-shadow drill --data-dir PATH
  postfiat-cobalt-shadow network-drill --data-dir PATH"
}

fn read_bounded_json<T: for<'de> Deserialize<'de>>(path: &PathBuf, max: u64) -> io::Result<T> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > max {
        return Err(invalid("JSON input exceeds bound"));
    }
    serde_json::from_slice(&fs::read(path)?).map_err(json_error)
}

fn json_error(error: serde_json::Error) -> io::Error {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
