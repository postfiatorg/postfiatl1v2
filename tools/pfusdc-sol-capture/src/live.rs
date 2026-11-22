use crate::{
    local::{footer_cert, token_account},
    witness::{self, State},
};
use agave_snapshots::{SnapshotInterval, snapshot_config::SnapshotUsage};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use solana_account::{Account, ReadableAccount};
use solana_core::validator::ValidatorConfig;
use solana_gossip::node::Node;
use solana_keypair::{Keypair, read_keypair_file};
use solana_ledger::blockstore_meta::BlockLocation;
use solana_ledger::blockstore_options::BlockstoreCleanupStrategy;
use solana_local_cluster::{
    cluster::{Cluster, ClusterValidatorInfo, ValidatorInfo},
    integration_tests::{DEFAULT_NODE_STAKE, ValidatorKeys},
    local_cluster::{ClusterConfig, LocalCluster},
    validator_configs::make_identical_validator_configs,
};
use solana_net_utils::SocketAddrSpace;
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_signer::Signer;
use spl_token_interface::state::{Account as TokenAccount, AccountState, Mint};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

fn key(dir: &Path, name: &str) -> Result<Arc<Keypair>> {
    read_keypair_file(dir.join(format!("{name}.json")))
        .map(Arc::new)
        .map_err(|_| anyhow::anyhow!("cannot read key path for {name}"))
}

fn publish(file: &Path, value: Value) -> Result<()> {
    let stage = file.with_extension("tmp");
    fs::write(&stage, serde_json::to_vec_pretty(&value)?)?;
    fs::rename(stage, file)?;
    Ok(())
}

pub fn run(dir: &Path, keys: &Path, vault: Pubkey, recorder: Pubkey) -> Result<()> {
    agave_logger::setup_with_default("error");
    let stopping = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, stopping.clone())?;
    // Retain certified block material with the ledgers even when the relay archive is offline.
    let archive = dir.join("ledgers").join("certified-events");
    fs::create_dir_all(&archive)?;
    let identities = (0..2)
        .map(|i| {
            Ok(ValidatorKeys {
                node_keypair: key(keys, &format!("live-node-{i}"))?,
                vote_keypair: key(keys, &format!("live-vote-{i}"))?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let specification = dir.join("cluster.json");
    let mut node = ValidatorConfig::default_for_test();
    node.rpc_config.skip_preflight_health_check = true;
    node.rpc_config.enable_rpc_transaction_history = true;
    let mut nodes = if specification.exists() {
        let spec: Value = serde_json::from_slice(&fs::read(&specification)?)?;
        let net = identities
            .iter()
            .map(|k| Node::new_localhost_with_pubkey(&k.node_keypair.pubkey()))
            .collect::<Vec<_>>();
        let entry = net[0].info.clone();
        let mut threads = vec![];
        for ((keys, net), mut config) in identities
            .iter()
            .zip(net)
            .zip(make_identical_validator_configs(&node, 2))
        {
            let path = PathBuf::from(
                spec["ledgers"][keys.node_keypair.pubkey().to_string()]
                    .as_str()
                    .context("missing validator ledger")?,
            );
            ensure!(
                path.join("genesis.bin").is_file(),
                "missing persisted genesis"
            );
            let archives = path.join("live-snapshots");
            fs::create_dir_all(&archives)?;
            let snapshots = &mut config.snapshot_config;
            snapshots.usage = SnapshotUsage::LoadAndGenerate;
            snapshots.full_snapshot_archives_dir = archives.clone();
            snapshots.incremental_snapshot_archives_dir = archives.clone();
            snapshots.bank_snapshots_dir = path.join("snapshot");
            snapshots.full_snapshot_archive_interval =
                SnapshotInterval::Slots(std::num::NonZeroU64::new(512).unwrap());
            snapshots.incremental_snapshot_archive_interval = SnapshotInterval::Disabled;
            snapshots.use_direct_io = false;
            snapshots.use_registered_io_uring_buffers = false;
            // Keep both complete ledgers until there is a local recovery snapshot.
            if fs::read_dir(&archives)?.filter_map(Result::ok).any(|file| {
                let name = file.file_name();
                let name = name.to_string_lossy();
                name.starts_with("snapshot-") && name.ends_with(".tar.zst")
            }) {
                config.blockstore_cleanup_strategy =
                    BlockstoreCleanupStrategy::CountDataShreds(1_000_000);
            }
            let contact = net.info.clone();
            config.rpc_addrs = Some((
                contact.rpc().context("RPC address")?,
                contact.rpc_pubsub().context("pubsub address")?,
            ));
            let info = ClusterValidatorInfo {
                info: ValidatorInfo {
                    keypair: keys.node_keypair.clone(),
                    voting_keypair: keys.vote_keypair.clone(),
                    ledger_path: path,
                    contact_info: contact,
                },
                config,
                validator: None,
            };
            let peers = if threads.is_empty() {
                vec![]
            } else {
                vec![entry.clone()]
            };
            threads.push(thread::spawn(move || {
                LocalCluster::restart_node_with_context(
                    info,
                    (net, peers),
                    SocketAddrSpace::Unspecified,
                )
            }));
        }
        threads
            .into_iter()
            .map(|h| {
                h.join()
                    .map_err(|_| anyhow::anyhow!("validator restart failed"))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        node.wait_for_supermajority = Some(0);
        let payer = key(keys, "local-payer")?.pubkey();
        let mint = Pubkey::new_unique();
        let source = Pubkey::new_unique();
        let supply = 500_000_000;
        let mut config = ClusterConfig {
            validator_configs: make_identical_validator_configs(&node, 2),
            validator_keys: Some(identities.iter().cloned().map(|k| (k, true)).collect()),
            node_stakes: vec![DEFAULT_NODE_STAKE; 2],
            slots_per_epoch: 2048,
            stakers_slot_offset: 2048,
            skip_warmup_slots: false,
            ..Default::default()
        };
        config
            .additional_accounts
            .extend(solana_program_binaries::spl_programs(&Rent::default()));
        config.additional_accounts.extend([
            (
                payer,
                Account {
                    lamports: 100_000_000_000,
                    ..Default::default()
                }
                .into(),
            ),
            (
                mint,
                token_account(Mint {
                    supply,
                    decimals: 6,
                    is_initialized: true,
                    ..Default::default()
                })?,
            ),
            (
                source,
                token_account(TokenAccount {
                    mint,
                    owner: payer,
                    amount: supply,
                    state: AccountState::Initialized,
                    ..Default::default()
                })?,
            ),
            (
                "61BtM7BkDEE8Yq5fskEVAQT9mYA8qCejJWoLe5apqg81".parse()?,
                Account {
                    lamports: Rent::default().minimum_balance(9),
                    data: vec![0; 9],
                    owner: "Feature111111111111111111111111111111111111".parse()?,
                    rent_epoch: u64::MAX,
                    ..Default::default()
                }
                .into(),
            ),
        ]);
        let mut cluster = LocalCluster::new(&mut config, SocketAddrSpace::Unspecified);
        let ledgers: BTreeMap<_, _> = cluster
            .validators
            .iter()
            .map(|(k, v)| (k.to_string(), &v.info.ledger_path))
            .collect();
        publish(
            &specification,
            json!({"genesis":cluster.genesis_config.hash().to_string(),"ledgers":ledgers,"mint":mint.to_string(),"source":source.to_string()}),
        )?;
        std::mem::take(&mut cluster.validators)
            .into_values()
            .collect()
    };
    let urls = nodes
        .iter()
        .map(|n| {
            Ok(format!(
                "http://{}",
                n.info.contact_info.rpc().context("RPC address")?
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    publish(&dir.join("rpc.json"), json!(urls))?;
    let first = nodes[0].validator.as_ref().context("capture node")?;
    let peer = nodes[1].validator.as_ref().context("peer node")?;
    let config = Pubkey::find_program_address(&[b"config"], &vault).0;
    let mut seen = HashSet::new();
    let mut parents = BTreeMap::new();
    let mut pending = BTreeMap::new();
    let mut next = first.bank_forks.read().unwrap().root().max(1);
    // Validator drop joins worker threads. Signal exit on capture errors as well,
    // so systemd can resume from the persistent ledgers and snapshots.
    let capture_result = (|| -> Result<()> {
        while !stopping.load(Ordering::Relaxed) {
            for validator in [first, peer] {
                let forks = validator.bank_forks.read().unwrap();
                for slot in forks.active_bank_slots() {
                    if let Some(bank) = forks.get(slot) {
                        if let Some(parent) = bank.parent() {
                            parents.insert((slot, parent.hash()), parent);
                        }
                    }
                }
                for (slot, bank) in forks.frozen_banks() {
                    let Some(id) = bank.block_id() else { continue };
                    if seen.contains(&(slot, id)) {
                        continue;
                    }
                    let Some(parent) = bank
                        .parent()
                        .or_else(|| parents.get(&(slot, bank.parent_hash())).cloned())
                    else {
                        continue;
                    };
                    if parent.block_id().is_none() {
                        continue;
                    }
                    seen.insert((slot, id));
                    let writes = bank.get_all_accounts_modified_since_parent();
                    let events: Vec<_> = writes.iter().filter(|(_,a)|
                    (a.owner()==&vault && a.data().starts_with(b"PFSOLRC1")) ||
                    (a.owner()==&recorder && (a.data().starts_with(b"PFSOLST1") || a.data().starts_with(b"PFSOLKY1")))
                ).map(|(addr, a)| json!({"address":addr.to_string(),"tag":std::str::from_utf8(&a.data()[..8]).unwrap()})).collect();
                    if events.is_empty() {
                        continue;
                    }
                    let Some(cfg) = bank.get_account(&config) else {
                        continue;
                    };
                    let route = cfg.data().get(32..64).context("vault config")?.try_into()?;
                    println!("captured candidate slot={slot} events={}", events.len());
                    pending.insert(
                        (slot, id),
                        (
                            State::capture(&bank, &parent, &vault, &route)?,
                            events,
                            bank.epoch(),
                        ),
                    );
                }
            }
            let common = first
                .bank_forks
                .read()
                .unwrap()
                .root()
                .min(peer.bank_forks.read().unwrap().root());
            while next <= common {
                let slot = next;
                next += 1;
                if pending.is_empty() || !first.blockstore.is_root(slot) {
                    continue;
                }
                let Some(cert) = footer_cert(&first.blockstore, slot)? else {
                    continue;
                };
                let Some((state, events, epoch)) = pending.remove(&(cert.slot, cert.block_id))
                else {
                    continue;
                };
                ensure!(
                    peer.blockstore.is_root(cert.slot),
                    "certified slot absent from rooted peer"
                );
                let id = peer
                    .blockstore
                    .get_double_merkle_root(cert.slot, BlockLocation::Original)?
                    .context("peer block id")?;
                let clock = || {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs_f64()
                };
                let finalized_at = clock();
                let target = archive.join(cert.slot.to_string());
                if !target.exists() {
                    witness::archive(&archive, &first.blockstore, &state, &cert, id)?;
                }
                publish(
                    &target.join("events.json"),
                    json!({"epoch":epoch,"events":events,"footer_slot":slot,"finalized_at":finalized_at,"capture_complete_at":clock()}),
                )?;
            }
            parents.retain(|(slot, _), _| *slot >= common.saturating_sub(16));
            seen.retain(|(slot, _)| *slot >= common.saturating_sub(128));
            pending.retain(|(slot, _), _| *slot >= common.saturating_sub(256));
            thread::sleep(Duration::from_millis(3));
        }
        Ok(())
    })();
    if let Err(error) = &capture_result {
        eprintln!("rooted capture stopped: {error:#}");
    }
    for n in &mut nodes {
        n.validator.as_mut().unwrap().exit();
    }
    for n in &mut nodes {
        n.validator.take().unwrap().join();
    }
    capture_result
}
