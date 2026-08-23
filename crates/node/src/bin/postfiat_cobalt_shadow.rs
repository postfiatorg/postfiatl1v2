use std::env;
use std::fs;
use std::io;
use std::net::TcpListener;
use std::path::PathBuf;

use postfiat_consensus_cobalt::{DabcRatifiedAmendment, RbcPropose};
use postfiat_node::cobalt_shadow::{
    assemble_protocol_transcript, assemble_protocol_transcript_extending,
    build_registry_binding_manifest, run_cobalt_shadow_adversarial_drill, CobaltShadowHistoryRange,
    CobaltShadowIdentity, CobaltShadowLimits, CobaltShadowProtocolContribution,
    CobaltShadowRegistryBinding, CobaltShadowService, CobaltShadowValidatorBinding,
};
use postfiat_node::cobalt_shadow_runtime::{
    parse_endpoint, read_transcript, request, run_cobalt_shadow_network_drill, serve_listener,
    validate_listen_address, CobaltShadowRpcRequest,
};
use postfiat_node::ValidatorRegistry;
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
        "contribute" => {
            let mut service = CobaltShadowService::open(required_path(&args, "--data-dir")?)?;
            let binding: CobaltShadowRegistryBinding = read_bounded_json(
                &required_path(&args, "--registry-binding")?,
                4 * 1024 * 1024,
            )?;
            let proposal: RbcPropose =
                read_bounded_json(&required_path(&args, "--proposal")?, 2 * 1024 * 1024)?;
            let contribution = service.create_protocol_contribution(&binding, &proposal)?;
            serde_json::to_value(contribution).map_err(json_error)?
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
            let transcript = match optional_flag(&args, "--previous-ratification") {
                Some(path) => {
                    let previous: DabcRatifiedAmendment =
                        read_bounded_json(&PathBuf::from(path), 2 * 1024 * 1024)?;
                    assemble_protocol_transcript_extending(
                        &binding,
                        proposal,
                        contributions,
                        Some(&previous),
                    )?
                }
                None => assemble_protocol_transcript(&binding, proposal, contributions)?,
            };
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
            request(
                endpoint,
                &CobaltShadowRpcRequest::Commit {
                    transcript: Box::new(transcript),
                },
            )?
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
  postfiat-cobalt-shadow propose --data-dir PATH --registry-binding PATH --round N --payload-hash HASH
  postfiat-cobalt-shadow contribute --data-dir PATH --registry-binding PATH --proposal PATH
  postfiat-cobalt-shadow assemble --registry-binding PATH --proposal PATH --contributions PATH [--previous-ratification PATH]
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
