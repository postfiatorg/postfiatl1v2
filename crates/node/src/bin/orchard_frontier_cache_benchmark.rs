use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use postfiat_privacy_orchard::{
    orchard_anchor_from_commitments, orchard_frontier_snapshot_append_commitments,
    orchard_frontier_snapshot_from_commitments, OrchardOutputCommitment, ORCHARD_COMMITMENT_BYTES,
};
use serde::Serialize;

#[derive(Debug)]
struct Options {
    counts: Vec<usize>,
    warm_repeats: usize,
    csv: Option<PathBuf>,
    json: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct Row {
    commitments: usize,
    cold_lazy_rebuild_ms: f64,
    old_path_full_root_ms: f64,
    warm_append_avg_ms: f64,
    warm_append_min_ms: f64,
    warm_append_max_ms: f64,
    old_path_to_warm_ratio: f64,
    cold_root: String,
    old_path_root: String,
    warm_append_root: String,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        counts: vec![1_000, 10_000, 50_000, 100_000],
        warm_repeats: 10,
        csv: None,
        json: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--counts" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--counts requires a comma-separated value".to_string())?;
                options.counts = value
                    .split(',')
                    .map(|part| {
                        part.trim()
                            .parse::<usize>()
                            .map_err(|_| format!("invalid count: {part}"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "--warm-repeats" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--warm-repeats requires a value".to_string())?;
                options.warm_repeats = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid warm repeat count: {value}"))?;
            }
            "--csv" => {
                options.csv = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--csv requires a path".to_string())?,
                ));
            }
            "--json" => {
                options.json = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--json requires a path".to_string())?,
                ));
            }
            "--help" | "-h" => {
                println!(
                    "Usage: orchard_frontier_cache_benchmark [--counts 1000,10000,50000,100000] [--warm-repeats 10] [--csv PATH] [--json PATH]"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if options.counts.is_empty() {
        return Err("--counts must include at least one count".to_string());
    }
    if options.warm_repeats == 0 {
        return Err("--warm-repeats must be positive".to_string());
    }
    Ok(options)
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn synthetic_commitment() -> OrchardOutputCommitment {
    OrchardOutputCommitment::from_bytes(&[3u8; ORCHARD_COMMITMENT_BYTES])
        .expect("fixed synthetic Orchard output commitment must parse")
}

fn write_csv(path: &PathBuf, rows: &[Row]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = String::from(
        "commitments,cold_lazy_rebuild_ms,old_path_full_root_ms,warm_append_avg_ms,warm_append_min_ms,warm_append_max_ms,old_path_to_warm_ratio,cold_root,old_path_root,warm_append_root\n",
    );
    for row in rows {
        output.push_str(&format!(
            "{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{}\n",
            row.commitments,
            row.cold_lazy_rebuild_ms,
            row.old_path_full_root_ms,
            row.warm_append_avg_ms,
            row.warm_append_min_ms,
            row.warm_append_max_ms,
            row.old_path_to_warm_ratio,
            row.cold_root,
            row.old_path_root,
            row.warm_append_root,
        ));
    }
    fs::write(path, output)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_args()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let max_count = *options
        .counts
        .iter()
        .max()
        .expect("counts were validated non-empty");
    let base_commitment = synthetic_commitment();
    let commitments = vec![base_commitment.clone(); max_count + 1];
    let mut rows = Vec::new();

    for count in options.counts {
        let prefix = &commitments[..count];

        let cold_start = Instant::now();
        let snapshot = orchard_frontier_snapshot_from_commitments(prefix)?;
        let cold_lazy_rebuild_ms = ms(cold_start);

        let old_start = Instant::now();
        let old_root = orchard_anchor_from_commitments(prefix)?;
        let old_path_full_root_ms = ms(old_start);
        if snapshot.root != old_root.as_hex() {
            return Err(format!(
                "root mismatch at {count}: cold={} old={}",
                snapshot.root,
                old_root.as_hex()
            )
            .into());
        }

        let mut warm_times = Vec::with_capacity(options.warm_repeats);
        let mut warm_root = String::new();
        for _ in 0..options.warm_repeats {
            let warm_start = Instant::now();
            let appended = orchard_frontier_snapshot_append_commitments(
                Some(&snapshot),
                &commitments[count..count + 1],
            )?;
            warm_times.push(ms(warm_start));
            warm_root = appended.root;
        }
        let warm_append_min_ms = warm_times.iter().copied().fold(f64::INFINITY, f64::min);
        let warm_append_max_ms = warm_times.iter().copied().fold(0.0_f64, f64::max);
        let warm_append_avg_ms = warm_times.iter().sum::<f64>() / warm_times.len() as f64;
        let old_path_to_warm_ratio = if warm_append_avg_ms > 0.0 {
            old_path_full_root_ms / warm_append_avg_ms
        } else {
            f64::INFINITY
        };

        rows.push(Row {
            commitments: count,
            cold_lazy_rebuild_ms,
            old_path_full_root_ms,
            warm_append_avg_ms,
            warm_append_min_ms,
            warm_append_max_ms,
            old_path_to_warm_ratio,
            cold_root: snapshot.root,
            old_path_root: old_root.as_hex().to_string(),
            warm_append_root: warm_root,
        });
    }

    if let Some(path) = &options.csv {
        write_csv(path, &rows)?;
    }
    if let Some(path) = &options.json {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(&rows)? + "\n")?;
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
