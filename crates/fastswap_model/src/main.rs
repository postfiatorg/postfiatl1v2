use postfiat_fastswap_model::{check, ModelConfig};

fn usage() -> &'static str {
    "usage: fastswap-model-check --n <4|6> [--depth <positive-u8>] [--unsafe-no-stale-qc-guard]"
}

fn parse_args() -> Result<ModelConfig, String> {
    let mut args = std::env::args().skip(1);
    let mut validator_count = None;
    let mut max_depth = 28_u8;
    let mut enforce_stale_qc_guard = true;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--n" => {
                let value = args.next().ok_or_else(|| usage().to_owned())?;
                validator_count = Some(value.parse::<u8>().map_err(|_| usage().to_owned())?);
            }
            "--depth" => {
                let value = args.next().ok_or_else(|| usage().to_owned())?;
                max_depth = value.parse::<u8>().map_err(|_| usage().to_owned())?;
            }
            "--unsafe-no-stale-qc-guard" => enforce_stale_qc_guard = false,
            _ => return Err(usage().to_owned()),
        }
    }
    Ok(ModelConfig {
        validator_count: validator_count.ok_or_else(|| usage().to_owned())?,
        max_depth,
        enforce_stale_qc_guard,
    })
}

fn main() {
    let config = parse_args().unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let report = check(config).unwrap_or_else(|error| {
        eprintln!("model configuration error: {error:?}");
        std::process::exit(2);
    });
    println!(
        "n={} f={} q={} depth={} states={} transitions={} deepest={} guard={} result={}",
        report.validator_count,
        report.fault_tolerance,
        report.quorum,
        report.max_depth,
        report.states_explored,
        report.transitions_explored,
        report.deepest_state,
        config.enforce_stale_qc_guard,
        if report.counterexample.is_none() {
            "PASS"
        } else {
            "FAIL"
        },
    );
    if let Some(counterexample) = report.counterexample {
        eprintln!("violation: {:?}", counterexample.violation);
        for (index, step) in counterexample.trace.iter().enumerate() {
            eprintln!("{:02}: {:?}", index + 1, step);
        }
        std::process::exit(1);
    }
}
