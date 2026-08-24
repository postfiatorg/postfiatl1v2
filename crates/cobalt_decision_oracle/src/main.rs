use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use postfiat_cobalt_decision_oracle::{build_manifest, sha256_hex, OracleInput};

fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}

fn required_arg(args: &[String], name: &str) -> Result<PathBuf, std::io::Error> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| PathBuf::from(&pair[1]))
        .ok_or_else(|| invalid(format!("missing {name}")))
}

fn named_paths(args: &[String], name: &str) -> Result<BTreeMap<String, PathBuf>, std::io::Error> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == name {
            let value = args
                .get(index + 1)
                .ok_or_else(|| invalid(format!("missing value for {name}")))?;
            let (label, path) = value
                .split_once('=')
                .ok_or_else(|| invalid(format!("{name} values must be LABEL=PATH")))?;
            if label.is_empty()
                || values
                    .insert(label.to_string(), PathBuf::from(path))
                    .is_some()
            {
                return Err(invalid(format!("duplicate or empty {name} label")));
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(values)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let input_path = required_arg(&args, "--input")?;
    let output_path = required_arg(&args, "--output")?;
    let source_path = required_arg(&args, "--oracle-source")?;
    let contract_path = required_arg(&args, "--contract")?;
    let adapter_paths = named_paths(&args, "--adapter")?;
    if adapter_paths.is_empty() {
        return Err(invalid("at least one --adapter LABEL=PATH is required").into());
    }

    let input_bytes = fs::read(&input_path)?;
    let input: OracleInput = serde_json::from_slice(&input_bytes)?;
    let source_sha256 = sha256_hex(&fs::read(source_path)?);
    let contract_sha256 = sha256_hex(&fs::read(contract_path)?);
    let adapter_sha256 = adapter_paths
        .into_iter()
        .map(|(label, path)| Ok((label, sha256_hex(&fs::read(path)?))))
        .collect::<Result<BTreeMap<_, _>, std::io::Error>>()?;
    let manifest = build_manifest(
        input,
        &input_bytes,
        source_sha256,
        contract_sha256,
        adapter_sha256,
    )
    .map_err(invalid)?;
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    fs::write(&output_path, bytes)?;
    println!(
        "schema={} cases={} input_sha256={} manifest_sha256={}",
        manifest.schema,
        manifest.cases.len(),
        manifest.input_sha256,
        manifest.manifest_sha256
    );
    Ok(())
}
