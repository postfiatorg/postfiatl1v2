use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use postfiat_storage::transactional::{
    CommitFinalizedBlock, CurrentStateUpdate, TransactionalStore,
};
use postfiat_storage::OrderedHistoryCommitment;
use postfiat_types::{
    BatchArchiveEntry, BlockCertificate, BlockHeader, BlockRecord, ChainTipState,
};

fn genesis_tip() -> ChainTipState {
    ChainTipState {
        schema: "postfiat-chain-tip-v1".to_owned(),
        chain_id: "transactional-crash-fixture".to_owned(),
        genesis_hash: "transactional-crash-genesis".to_owned(),
        protocol_version: 1,
        height: 0,
        block_hash: "genesis".to_owned(),
        state_root: "state-0".to_owned(),
        ordered_batch_count: 0,
        receipt_count: 0,
        history_base_height: 0,
    }
}

fn next_tip() -> ChainTipState {
    ChainTipState {
        height: 1,
        block_hash: "block-1".to_owned(),
        state_root: "state-1".to_owned(),
        ordered_batch_count: 1,
        ..genesis_tip()
    }
}

fn block() -> BlockRecord {
    BlockRecord {
        header: BlockHeader {
            height: 1,
            view: 0,
            parent_hash: "genesis".to_owned(),
            proposer: "validator-0".to_owned(),
            batch_kind: "transparent".to_owned(),
            batch_id: "batch-1".to_owned(),
            state_root: "state-1".to_owned(),
            bridge_exit_root: None,
            pftl_uniswap_receipt_root: None,
            receipt_count: 0,
            certificate_id: "certificate-1".to_owned(),
            certificate: BlockCertificate {
                validators: Vec::new(),
                quorum: 0,
                registry_root: String::new(),
                votes: Vec::new(),
            },
            consensus_v2_commit: None,
            block_hash: "block-1".to_owned(),
        },
        receipt_ids: Vec::new(),
        fastpay_pre_state_effects: Vec::new(),
    }
}

fn archive() -> BatchArchiveEntry {
    BatchArchiveEntry {
        batch_kind: "transparent".to_owned(),
        batch_id: "batch-1".to_owned(),
        payload_hash: "payload-hash".to_owned(),
        payload_json: "{}".to_owned(),
    }
}

fn initialize(path: &Path) -> Result<(), String> {
    let store = TransactionalStore::open(path).map_err(|error| error.to_string())?;
    let tip = genesis_tip();
    let commitment =
        OrderedHistoryCommitment::genesis(&tip.chain_id, &tip.genesis_hash, tip.protocol_version)
            .map_err(|error| error.to_string())?;
    store
        .initialize(&tip, &commitment, CurrentStateUpdate::default())
        .map_err(|error| error.to_string())?;
    store
        .verify_and_mark_full_integrity()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn signal(path: &Path) -> Result<(), String> {
    fs::write(path, b"ready\n").map_err(|error| error.to_string())?;
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| error.to_string())
}

fn wait_forever() -> ! {
    loop {
        std::thread::park_timeout(Duration::from_secs(60));
    }
}

fn commit(path: &Path, ready: Option<&Path>, during: bool) -> Result<(), String> {
    let store = TransactionalStore::open(path).map_err(|error| error.to_string())?;
    let old_tip = genesis_tip();
    let new_tip = next_tip();
    let old_commitment = OrderedHistoryCommitment::genesis(
        &old_tip.chain_id,
        &old_tip.genesis_hash,
        old_tip.protocol_version,
    )
    .map_err(|error| error.to_string())?;
    let new_commitment = old_commitment
        .append("batch-1")
        .map_err(|error| error.to_string())?;
    let block = block();
    let archive = archive();
    let request = CommitFinalizedBlock {
        expected_tip: &old_tip,
        new_tip: &new_tip,
        block: &block,
        receipts: &[],
        archive_entry: &archive,
        batch_id: "batch-1",
        ordered_history: &new_commitment,
        current_state: CurrentStateUpdate::default(),
        scheduled_activation_height: None,
        allow_legacy_receipt_id_mismatch: false,
    };
    if during {
        let ready = ready.ok_or_else(|| "during mode requires a ready file".to_owned())?;
        store
            .commit_finalized_block_with_precommit_hook(request, || {
                if let Err(error) = signal(ready) {
                    eprintln!("transactional-crash-fixture-signal-failed: {error}");
                    std::process::exit(2);
                }
                wait_forever()
            })
            .map_err(|error| error.to_string())?;
    } else {
        store
            .commit_finalized_block(request)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn parse_args() -> Result<(String, PathBuf, Option<PathBuf>), String> {
    let mut args = env::args().skip(1);
    let mode = args.next().ok_or_else(|| "missing mode".to_owned())?;
    let data_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "missing data directory".to_owned())?;
    let ready = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }
    Ok((mode, data_dir, ready))
}

fn run() -> Result<(), String> {
    let (mode, data_dir, ready) = parse_args()?;
    match mode.as_str() {
        "prepare" => initialize(&data_dir),
        "commit" => commit(&data_dir, None, false),
        "before" => {
            signal(
                ready
                    .as_deref()
                    .ok_or_else(|| "before mode requires a ready file".to_owned())?,
            )?;
            wait_forever()
        }
        "during" => commit(&data_dir, ready.as_deref(), true),
        "after" => {
            commit(&data_dir, None, false)?;
            signal(
                ready
                    .as_deref()
                    .ok_or_else(|| "after mode requires a ready file".to_owned())?,
            )?;
            wait_forever()
        }
        _ => Err(format!("unknown mode: {mode}")),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("transactional-crash-fixture-failed: {error}");
        std::process::exit(1);
    }
}
