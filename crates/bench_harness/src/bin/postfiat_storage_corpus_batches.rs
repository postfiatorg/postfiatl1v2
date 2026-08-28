use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use postfiat_execution::{genesis_hash, transfer_tx_id};
use postfiat_mempool_dag::{build_transaction_batch, MempoolBatchDomain};
use postfiat_storage::NodeStore;
use postfiat_types::SignedTransfer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CORPUS_SCHEMA: &str = "postfiat-tx-latency-signed-transfer-corpus-v1";
const REPORT_SCHEMA: &str = "postfiat-storage-corpus-batch-build-report-v1";
const MAX_CORPUS_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TRANSFERS: usize = 10_000;

#[derive(Debug, Deserialize)]
struct SignedTransferCorpus {
    schema: String,
    transfers: Vec<SignedTransfer>,
}

#[derive(Debug, Serialize)]
struct BatchBuildEntry {
    corpus_index: usize,
    sequence: u64,
    tx_id: String,
    signed_transfer_sha256: String,
    batch_id: String,
    payload_hash: String,
    batch_file: String,
    batch_sha256: String,
}

#[derive(Debug, Serialize)]
struct BatchBuildReport {
    schema: &'static str,
    source_git_revision: &'static str,
    build_profile: &'static str,
    corpus_file: String,
    corpus_sha256: String,
    data_dir: String,
    output_dir: String,
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
    transfer_count: usize,
    batches: Vec<BatchBuildEntry>,
}

#[derive(Debug)]
struct Options {
    data_dir: PathBuf,
    corpus_file: PathBuf,
    output_dir: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let options = parse_options(env::args().skip(1).collect())?;
    let report = build_batches(&options)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_options(args: Vec<String>) -> Result<Options, String> {
    let mut data_dir = None;
    let mut corpus_file = None;
    let mut output_dir = None;
    let mut index = 0;
    while index < args.len() {
        let flag = &args[index];
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--data-dir" => data_dir = Some(PathBuf::from(value)),
            "--signed-transfer-corpus" => corpus_file = Some(PathBuf::from(value)),
            "--output-dir" => output_dir = Some(PathBuf::from(value)),
            other => return Err(format!("unsupported argument `{other}`")),
        }
        index += 2;
    }
    Ok(Options {
        data_dir: data_dir.ok_or("missing --data-dir")?,
        corpus_file: corpus_file.ok_or("missing --signed-transfer-corpus")?,
        output_dir: output_dir.ok_or("missing --output-dir")?,
    })
}

fn regular_file_bytes(path: &Path, maximum: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("`{}` must be a regular non-symlink file", path.display()).into());
    }
    if metadata.len() == 0 || metadata.len() > maximum {
        return Err(format!(
            "`{}` has {} bytes; expected 1..={maximum}",
            path.display(),
            metadata.len()
        )
        .into());
    }
    Ok(fs::read(path)?)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn batch_filename(index: usize) -> String {
    format!("round-{index:06}.batch.json")
}

fn build_batches(options: &Options) -> Result<BatchBuildReport, Box<dyn Error>> {
    let data_metadata = fs::symlink_metadata(&options.data_dir)?;
    if data_metadata.file_type().is_symlink() || !data_metadata.is_dir() {
        return Err("--data-dir must be a regular non-symlink directory".into());
    }
    if options.output_dir.exists() || fs::symlink_metadata(&options.output_dir).is_ok() {
        return Err(format!(
            "refusing to overwrite output directory `{}`",
            options.output_dir.display()
        )
        .into());
    }

    let corpus_bytes = regular_file_bytes(&options.corpus_file, MAX_CORPUS_BYTES)?;
    let corpus: SignedTransferCorpus = serde_json::from_slice(&corpus_bytes)?;
    if corpus.schema != CORPUS_SCHEMA {
        return Err(format!("unsupported corpus schema `{}`", corpus.schema).into());
    }
    if corpus.transfers.is_empty() || corpus.transfers.len() > MAX_TRANSFERS {
        return Err(format!(
            "corpus contains {} transfers; expected 1..={MAX_TRANSFERS}",
            corpus.transfers.len()
        )
        .into());
    }

    let genesis = NodeStore::new(&options.data_dir).read_genesis()?;
    let genesis_hash = genesis_hash(&genesis);
    let domain = MempoolBatchDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash.clone(),
        protocol_version: genesis.protocol_version,
    };
    fs::create_dir(&options.output_dir)?;

    let mut tx_ids = BTreeSet::new();
    let mut entries = Vec::with_capacity(corpus.transfers.len());
    for (offset, transfer) in corpus.transfers.into_iter().enumerate() {
        let corpus_index = offset;
        transfer.unsigned.validate()?;
        let tx_id = transfer_tx_id(&transfer);
        if !tx_ids.insert(tx_id.clone()) {
            return Err(
                format!("corpus entry {corpus_index} duplicates transaction `{tx_id}`").into(),
            );
        }
        let signed_json = serde_json::to_string(&transfer)?;
        let signed_transfer_sha256 = sha256_hex(signed_json.as_bytes());
        let sequence = transfer.unsigned.sequence;
        let available = build_transaction_batch(&domain, vec![transfer])?;
        let filename = batch_filename(offset + 1);
        let path = options.output_dir.join(&filename);
        let mut bytes = serde_json::to_vec_pretty(&available.batch)?;
        bytes.push(b'\n');
        postfiat_storage::atomic_write(&path, &bytes)?;
        entries.push(BatchBuildEntry {
            corpus_index,
            sequence,
            tx_id,
            signed_transfer_sha256,
            batch_id: available.reference.batch_id,
            payload_hash: available.reference.payload_hash,
            batch_file: filename,
            batch_sha256: sha256_hex(&bytes),
        });
    }

    Ok(BatchBuildReport {
        schema: REPORT_SCHEMA,
        source_git_revision: option_env!("POSTFIAT_BUILD_GIT_REV").unwrap_or("unknown"),
        build_profile: option_env!("POSTFIAT_BUILD_PROFILE").unwrap_or("unknown"),
        corpus_file: options.corpus_file.display().to_string(),
        corpus_sha256: sha256_hex(&corpus_bytes),
        data_dir: options.data_dir.display().to_string(),
        output_dir: options.output_dir.display().to_string(),
        chain_id: domain.chain_id,
        genesis_hash,
        protocol_version: domain.protocol_version,
        transfer_count: entries.len(),
        batches: entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_filenames_sort_in_round_order() {
        let names = (1..=10_000).map(batch_filename).collect::<Vec<_>>();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, names);
    }

    #[test]
    fn options_require_exact_supported_flags() {
        let options = parse_options(vec![
            "--data-dir".into(),
            "node".into(),
            "--signed-transfer-corpus".into(),
            "corpus.json".into(),
            "--output-dir".into(),
            "batches".into(),
        ])
        .expect("parse supported flags");
        assert_eq!(options.data_dir, PathBuf::from("node"));
        assert!(parse_options(vec!["--unknown".into(), "value".into()]).is_err());
    }
}
