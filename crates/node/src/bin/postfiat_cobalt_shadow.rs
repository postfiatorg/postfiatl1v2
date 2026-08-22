use std::env;
use std::io;
use std::path::PathBuf;

use postfiat_node::cobalt_shadow::{
    run_cobalt_shadow_adversarial_drill, CobaltShadowIdentity, CobaltShadowLimits,
    CobaltShadowService,
};

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
        "drill" => {
            let report = run_cobalt_shadow_adversarial_drill(required_path(&args, "--data-dir")?)?;
            if !report.ok {
                return Err(invalid("adversarial drill did not converge"));
            }
            serde_json::to_value(report).map_err(json_error)?
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

fn required_flag<'a>(args: &'a [String], name: &str) -> io::Result<&'a str> {
    optional_flag(args, name).ok_or_else(|| invalid(format!("{name} is required")))
}

fn required_path(args: &[String], name: &str) -> io::Result<PathBuf> {
    required_flag(args, name).map(PathBuf::from)
}

fn usage() -> io::Error {
    invalid(
        "usage: postfiat-cobalt-shadow init --data-dir PATH --node-id ID --chain-id ID \
         --genesis-hash HASH [--protocol-version N] | status --data-dir PATH | \
         drill --data-dir PATH",
    )
}

fn json_error(error: serde_json::Error) -> io::Error {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
