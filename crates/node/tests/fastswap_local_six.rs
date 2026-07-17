use std::collections::BTreeMap;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use postfiat_crypto_provider::hex_to_bytes;
use postfiat_node::{init, InitOptions, ValidatorKeyFile};
use postfiat_rpc_sdk::{
    drive_fastswap_three_wave, fastswap_catch_up_request, fastswap_effects_request,
    fastswap_status_request, preview_fastswap, reconcile_fastswap_replication,
    wallet_backup_from_master_seed, wallet_dual_sign_fastswap_intent, wallet_identity_from_backup,
    FastSwapProductStateV1, FastSwapRpcTransportV1, FastSwapWalletSessionV1, SwapSettlementModeV1,
    TcpFastSwapTransportV1, WalletBackupFile,
};
use postfiat_storage::{fastswap_store::encode_fastlane_state_file, NodeStore};
use postfiat_types::{
    FastAssetControlStateV1, FastAssetDefinitionHashV1, FastAssetIdV1, FastAssetObjectV1,
    FastAssetRuleHashV1, FastAssetRuleV1, FastLaneStateV1, FastObjectIdV1, FastObjectKeyV1,
    FastObjectOriginV1, FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
    FastSwapCommitteeV1, FastSwapDecisionV1, FastSwapDepositIdV1, FastSwapEffectsResponseV1,
    FastSwapEffectsV1, FastSwapIntentV1, FastSwapLocalStatusV1, FastSwapMarketEnvelopeHashV1,
    FastSwapOpaqueHashV1, FastSwapPartyV1, FastSwapPolicyHashV1, FastSwapPolicySnapshotV1,
    FastSwapQuoteRoundingV1, FastSwapRfqHashV1, FastSwapStatusResponseV1, FastSwapValidatorV1,
    FastSwapVoteV1, SignedFastSwapIntentV1, FASTSWAP_SCHEMA_VERSION_V1,
};

const VALIDATORS: usize = 6;
const CHAIN_ID: &str = "postfiat-fastswap-local-six";

struct Harness {
    root: PathBuf,
    children: Vec<Child>,
}

impl Harness {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "postfiat-fastswap-local-six-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create FastSwap harness root");
        Self {
            root,
            children: Vec::new(),
        }
    }

    fn node(&self, index: usize) -> PathBuf {
        self.root.join(format!("validator-{index}"))
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for child in &mut self.children {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if std::env::var_os("POSTFIAT_KEEP_FASTSWAP_LOCAL_SIX").is_none() {
            let _ = fs::remove_dir_all(&self.root);
        } else {
            eprintln!("preserved FastSwap harness at {}", self.root.display());
        }
    }
}

fn node_bin() -> &'static str {
    env!("CARGO_BIN_EXE_postfiat-node")
}

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied directory");
    for entry in fs::read_dir(source).expect("read copied directory") {
        let entry = entry.expect("read copied entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("copied entry type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy node file");
        }
    }
}

fn rewrite_node_identity(data_dir: &Path, node_id: &str) {
    let store = NodeStore::new(data_dir);
    let mut state = store.read_node_state().expect("read node state");
    state.node_id = node_id.to_owned();
    store.write_node_state(&state).expect("write node state");
}

fn free_base_port() -> u16 {
    for base in (31_000u16..60_000).step_by(16) {
        let mut listeners = Vec::new();
        let mut available = true;
        for offset in 0..VALIDATORS as u16 {
            match TcpListener::bind(("127.0.0.1", base + offset)) {
                Ok(listener) => listeners.push(listener),
                Err(_) => {
                    available = false;
                    break;
                }
            }
        }
        if available {
            return base;
        }
    }
    panic!("no contiguous local port range available");
}

fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {}", path.display());
}

fn fixed48(hex: &str) -> [u8; 48] {
    hex_to_bytes(hex)
        .expect("48-byte hex")
        .try_into()
        .expect("48-byte value")
}

fn object(
    id: u64,
    owner: &[u8],
    asset_id: FastAssetIdV1,
    rule: FastAssetRuleHashV1,
    amount_atoms: u64,
) -> FastAssetObjectV1 {
    let mut object_id = [0u8; 32];
    object_id[..8].copy_from_slice(&id.to_be_bytes());
    let mut deposit_id = [0u8; 48];
    deposit_id[..8].copy_from_slice(&id.to_be_bytes());
    FastAssetObjectV1 {
        key: FastObjectKeyV1 {
            object_id: FastObjectIdV1(object_id),
            version: 1,
        },
        owner_pubkey: owner.to_vec(),
        asset_id,
        asset_rule_hash: rule,
        amount_atoms,
        control_state: FastAssetControlStateV1::Spendable,
        origin: FastObjectOriginV1::Deposit {
            deposit_id: FastSwapDepositIdV1(deposit_id),
        },
    }
}

fn fixture(
    seed_dir: &Path,
    swap_count: usize,
) -> (
    FastLaneStateV1,
    FastSwapCommitteeV1,
    Vec<FastSwapIntentV1>,
    WalletBackupFile,
    WalletBackupFile,
) {
    assert!(swap_count > 0);
    let store = NodeStore::new(seed_dir);
    let tip = store.read_chain_tip().expect("read initial chain tip");
    let key_file: ValidatorKeyFile = serde_json::from_slice(
        &fs::read(seed_dir.join("validator_keys.json")).expect("read validator keys"),
    )
    .expect("parse validator keys");
    assert_eq!(key_file.validators.len(), VALIDATORS);
    let validators = key_file
        .validators
        .iter()
        .map(|record| FastSwapValidatorV1 {
            validator_id: record.node_id.clone(),
            public_key: hex_to_bytes(&record.public_key_hex).expect("validator public key"),
        })
        .collect::<Vec<_>>();
    let chain = FastSwapChainDomainV1 {
        chain_id: CHAIN_ID.to_owned(),
        genesis_hash: FastSwapOpaqueHashV1(fixed48(&tip.genesis_hash)),
        protocol_version: tip.protocol_version,
    };
    let mut committee = FastSwapCommitteeV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: chain.clone(),
            fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 1,
            committee_root: FastSwapCommitteeRootV1::ZERO,
            validator_count: VALIDATORS as u16,
            quorum: 5,
        },
        validators,
    };
    committee.domain.committee_root = committee.computed_root().expect("committee root");
    committee.validate().expect("valid committee");

    let owner_0 =
        wallet_backup_from_master_seed(CHAIN_ID, "01".repeat(32), 0).expect("owner 0 backup");
    let owner_1 =
        wallet_backup_from_master_seed(CHAIN_ID, "02".repeat(32), 0).expect("owner 1 backup");
    let owner_0_identity = wallet_identity_from_backup(&owner_0).expect("owner 0 identity");
    let owner_1_identity = wallet_identity_from_backup(&owner_1).expect("owner 1 identity");
    let owner_0_public =
        hex_to_bytes(&owner_0_identity.public_key_hex).expect("owner 0 public key");
    let owner_1_public =
        hex_to_bytes(&owner_1_identity.public_key_hex).expect("owner 1 public key");
    let asset_0 = FastAssetIdV1([1; 48]);
    let asset_1 = FastAssetIdV1([2; 48]);
    let asset_rule_0 = FastAssetRuleV1 {
        asset_id: asset_0,
        asset_definition_hash: FastAssetDefinitionHashV1([3; 48]),
        issuer_address: "issuer-0".to_owned(),
        issuer_control_pubkey: vec![31; 64],
        requires_authorization: false,
        freeze_enabled: false,
        clawback_enabled: false,
        fast_lane_enabled: true,
        valid_from_height: 0,
        valid_through_height: 100,
    };
    let asset_rule_1 = FastAssetRuleV1 {
        asset_id: asset_1,
        asset_definition_hash: FastAssetDefinitionHashV1([4; 48]),
        issuer_address: "issuer-1".to_owned(),
        issuer_control_pubkey: vec![32; 64],
        requires_authorization: false,
        freeze_enabled: false,
        clawback_enabled: false,
        fast_lane_enabled: true,
        valid_from_height: 0,
        valid_through_height: 100,
    };
    let rule_0 = asset_rule_0.rule_hash().expect("rule 0");
    let rule_1 = asset_rule_1.rule_hash().expect("rule 1");
    let native = FastAssetIdV1::native_pft();
    let envelope = FastSwapMarketEnvelopeHashV1([6; 48]);
    let mut policy = FastSwapPolicySnapshotV1 {
        domain: chain,
        policy_epoch: 1,
        policy_hash: FastSwapPolicyHashV1::ZERO,
        pair_asset_0: asset_0,
        pair_asset_1: asset_1,
        asset_rule_hash_0: rule_0,
        asset_rule_hash_1: rule_1,
        price_numerator: 1,
        price_denominator: 8,
        rounding: FastSwapQuoteRoundingV1::Exact,
        nav_epoch: 59,
        market_envelope_hash: envelope,
        valid_from_height: 0,
        valid_through_height: 100,
        fee_schedule_hash: FastSwapOpaqueHashV1([10; 48]),
        max_inputs_per_party: 16,
        max_outputs: 8,
        paused: false,
    };
    let policy_hash = policy.computed_hash().expect("policy hash");
    policy.policy_hash = policy_hash;
    let mut base = FastLaneStateV1::empty(committee.domain.clone());
    base.asset_rules = BTreeMap::from([(rule_0, asset_rule_0), (rule_1, asset_rule_1)]);
    base.policy_snapshots = BTreeMap::from([(policy_hash, policy)]);
    let mut intents = Vec::with_capacity(swap_count);
    for index in 0..swap_count {
        let first_id = 1 + u64::try_from(index).expect("swap index") * 4;
        let objects = [
            object(first_id, &owner_0_public, asset_0, rule_0, 10),
            object(
                first_id + 1,
                &owner_0_public,
                native,
                FastAssetRuleHashV1::ZERO,
                10,
            ),
            object(first_id + 2, &owner_1_public, asset_1, rule_1, 3),
            object(
                first_id + 3,
                &owner_1_public,
                native,
                FastAssetRuleHashV1::ZERO,
                10,
            ),
        ];
        let party_0 = FastSwapPartyV1 {
            owner_address: owner_0_identity.address.clone(),
            owner_pubkey: owner_0_public.clone(),
            offered_asset_id: asset_0,
            offered_asset_rule_hash: rule_0,
            offered_amount: 8,
            receives_asset_id: asset_1,
            receives_asset_rule_hash: rule_1,
            receives_holder_permit_id: None,
            receives_amount: 1,
            asset_inputs: vec![objects[0].key],
            fee_inputs: vec![objects[1].key],
            asset_change: 2,
            fee_change: 9,
            fee_burn_pft: 1,
        };
        let party_1 = FastSwapPartyV1 {
            owner_address: owner_1_identity.address.clone(),
            owner_pubkey: owner_1_public.clone(),
            offered_asset_id: asset_1,
            offered_asset_rule_hash: rule_1,
            offered_amount: 1,
            receives_asset_id: asset_0,
            receives_asset_rule_hash: rule_0,
            receives_holder_permit_id: None,
            receives_amount: 8,
            asset_inputs: vec![objects[2].key],
            fee_inputs: vec![objects[3].key],
            asset_change: 2,
            fee_change: 9,
            fee_burn_pft: 1,
        };
        for object in objects {
            base.objects.insert(object.key, object);
        }
        let mut nonce = [0u8; 32];
        nonce[..8].copy_from_slice(&u64::try_from(index).expect("swap index").to_be_bytes());
        nonce[31] = 8;
        let intent = FastSwapIntentV1 {
            domain: committee.domain.clone(),
            policy_hash,
            rfq_hash: FastSwapRfqHashV1([7; 48]),
            market_envelope_hash: envelope,
            nav_epoch: 59,
            expires_at_height: 100,
            nonce,
            party_0,
            party_1,
        };
        intent.canonical_bytes().expect("intent bytes");
        intents.push(intent);
    }
    (base, committee, intents, owner_0, owner_1)
}

fn seed_canonical_fastswap(
    seed_dir: &Path,
    base: &FastLaneStateV1,
    committee: &FastSwapCommitteeV1,
) {
    let store = NodeStore::new(seed_dir);
    let mut ledger = store.read_ledger().expect("read seed ledger");
    ledger.fastswap_committees.push(committee.clone());
    ledger.fast_lane_asset_rules = base.asset_rules.values().cloned().collect();
    ledger.fastswap_policy_snapshots = base.policy_snapshots.values().cloned().collect();
    ledger.fastswap_activation_height = Some(0);
    store.write_ledger(&ledger).expect("write FastSwap ledger");
    let directory = seed_dir.join("fastswap-v1");
    fs::create_dir_all(&directory).expect("create FastSwap directory");
    fs::write(
        directory.join("base-state.json"),
        encode_fastlane_state_file(base).expect("encode FastSwap base"),
    )
    .expect("write FastSwap base");
    fs::write(
        directory.join("committee.json"),
        serde_json::to_vec_pretty(committee).expect("committee JSON"),
    )
    .expect("write FastSwap committee");
}

fn spawn_node(harness: &mut Harness, index: usize, port: u16, max_connections: usize) -> PathBuf {
    let data_dir = harness.node(index);
    let ready = harness
        .root
        .join(format!("validator-{index}.rpc.ready.json"));
    match fs::remove_file(&ready) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("clear stale RPC readiness {}: {error}", ready.display()),
    }
    let stdout = fs::File::create(harness.root.join(format!("validator-{index}.rpc.stdout")))
        .expect("create RPC stdout");
    let stderr = fs::File::create(harness.root.join(format!("validator-{index}.rpc.stderr")))
        .expect("create RPC stderr");
    let child = Command::new(node_bin())
        .args([
            "rpc-serve",
            "--unsafe-devnet-json-storage",
            "--data-dir",
            data_dir.to_str().expect("data dir UTF-8"),
            "--spool-dir",
            harness
                .root
                .join(format!("validator-{index}.spool"))
                .to_str()
                .expect("spool UTF-8"),
            "--ready-file",
            ready.to_str().expect("ready UTF-8"),
            "--bind-host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--max-requests",
            &max_connections.to_string(),
            "--timeout-ms",
            "30000",
            "--child-timeout-ms",
            "30000",
            "--keep-alive",
        ])
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn FastSwap RPC node");
    harness.children.push(child);
    ready
}

fn hex48(bytes: &[u8; 48]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn assert_conserved(
    base: &FastLaneStateV1,
    signed: &SignedFastSwapIntentV1,
    effects: &FastSwapEffectsV1,
) {
    let mut input_totals = BTreeMap::<FastAssetIdV1, u64>::new();
    for key in &effects.consumed {
        let object = base.objects.get(key).expect("consumed input in base");
        *input_totals.entry(object.asset_id).or_default() += object.amount_atoms;
    }
    let mut output_totals = BTreeMap::<FastAssetIdV1, u64>::new();
    for object in &effects.created {
        *output_totals.entry(object.asset_id).or_default() += object.amount_atoms;
    }
    for burn in &effects.fee_burns {
        *output_totals.entry(burn.asset_id).or_default() += burn.amount_atoms;
    }
    assert_eq!(input_totals, output_totals, "atom-for-atom conservation");
    let party_0 = &signed.intent.party_0;
    let party_1 = &signed.intent.party_1;
    assert!(effects.created.iter().any(|object| {
        object.owner_pubkey == party_0.owner_pubkey
            && object.asset_id == party_0.receives_asset_id
            && object.amount_atoms == party_0.receives_amount
    }));
    assert!(effects.created.iter().any(|object| {
        object.owner_pubkey == party_1.owner_pubkey
            && object.asset_id == party_1.receives_asset_id
            && object.amount_atoms == party_1.receives_amount
    }));
}

fn percentile(samples: &[u128], percentile: usize) -> u128 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let index = (sorted.len() * percentile).div_ceil(100).saturating_sub(1);
    sorted[index]
}

#[test]
#[ignore = "explicit six-process FastSwap release gate; run with --ignored --nocapture"]
fn fastswap_local_six_process_quorum_replication_conservation_and_restart() {
    let mut harness = Harness::new();
    let seed_dir = harness.root.join("seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: CHAIN_ID.to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: VALIDATORS as u32,
    })
    .expect("initialize six-validator seed");
    let (base, committee, mut intents, owner_0, owner_1) = fixture(&seed_dir, 1);
    let signed =
        wallet_dual_sign_fastswap_intent(&owner_0, &owner_1, intents.pop().expect("one intent"))
            .expect("dual-signed intent");
    seed_canonical_fastswap(&seed_dir, &base, &committee);
    for index in 0..VALIDATORS {
        let data_dir = harness.node(index);
        copy_dir(&seed_dir, &data_dir);
        rewrite_node_identity(&data_dir, &format!("validator-{index}"));
    }

    let base_port = free_base_port();
    let ports = (0..VALIDATORS)
        .map(|index| base_port + index as u16)
        .collect::<Vec<_>>();
    let ready = ports
        .iter()
        .enumerate()
        .map(|(index, port)| spawn_node(&mut harness, index, *port, 6))
        .collect::<Vec<_>>();
    for path in &ready {
        wait_for_file(path, Duration::from_secs(15));
    }
    let endpoints = ports
        .iter()
        .enumerate()
        .map(|(index, port)| (format!("validator-{index}"), format!("127.0.0.1:{port}")))
        .collect::<BTreeMap<_, _>>();
    let transport = TcpFastSwapTransportV1::new(endpoints, Duration::from_secs(20))
        .expect("FastSwap TCP transport");

    let preview_started = Instant::now();
    let expected = preview_fastswap(&signed, &committee, &transport).expect("quorum preview");
    let preview_ms = preview_started.elapsed().as_millis();
    let swap_id = signed.swap_id().expect("swap id");
    assert_eq!(expected.swap_id, swap_id);
    assert!(expected.receipt.accepted);
    assert_eq!(expected.receipt.code, "fastswap_applied");
    let swap_id_hex = hex48(&swap_id.0);
    for validator in &committee.validators {
        let shadow = transport
            .call(
                &validator.validator_id,
                &fastswap_status_request(
                    format!("post-preview-status-{}", validator.validator_id),
                    swap_id_hex.clone(),
                ),
            )
            .expect("post-preview status RPC")
            .result_as::<FastSwapStatusResponseV1>()
            .expect("post-preview status response");
        assert!(shadow.record.is_none());
        assert!(shadow.terminal_tombstone.is_none());
    }

    let mut session = FastSwapWalletSessionV1::new(
        SwapSettlementModeV1::FastSwapV1,
        signed.clone(),
        expected.clone(),
    )
    .expect("wallet session");
    let settlement_started = Instant::now();
    let terminal = drive_fastswap_three_wave(&mut session, &committee, &transport, |_| Ok(()))
        .expect("three-wave FastSwap settlement");
    let settlement_ms = settlement_started.elapsed().as_millis();
    assert_eq!(session.state, FastSwapProductStateV1::Accepted);
    assert!(terminal.effects.receipt.accepted);
    assert_eq!(terminal.effects.receipt.code, "fastswap_applied");
    assert!(terminal.lock_qc.votes.len() >= 5);
    assert!(terminal.decision_qc.votes.len() >= 5);
    assert!(terminal.effects_qc.votes.len() >= 5);

    let replication =
        reconcile_fastswap_replication(&mut session, &committee, &transport, |_| Ok(()))
            .expect("exact-six replication");
    assert!(replication.failed.is_empty(), "{replication:?}");
    assert!(replication.pending.is_empty(), "{replication:?}");
    assert!(session.replication_pending.is_empty());

    for validator in &committee.validators {
        let status = transport
            .call(
                &validator.validator_id,
                &fastswap_status_request(
                    format!("audit-status-{}", validator.validator_id),
                    swap_id_hex.clone(),
                ),
            )
            .expect("status RPC")
            .result_as::<FastSwapStatusResponseV1>()
            .expect("status response");
        assert_eq!(
            status.record.as_ref().map(|record| record.status),
            Some(FastSwapLocalStatusV1::Applied)
        );
        assert_eq!(
            status
                .terminal_tombstone
                .as_ref()
                .map(|tombstone| tombstone.decision),
            Some(FastSwapDecisionV1::Confirm)
        );
        let effects = transport
            .call(
                &validator.validator_id,
                &fastswap_effects_request(
                    format!("audit-effects-{}", validator.validator_id),
                    swap_id_hex.clone(),
                ),
            )
            .expect("effects RPC")
            .result_as::<FastSwapEffectsResponseV1>()
            .expect("effects response");
        assert_eq!(effects.effects, Some(expected.clone()));
    }

    assert_conserved(&base, &signed, &expected);

    // Exercise the catch-up lane on every node. Besides proving idempotence,
    // this opens the sixth and final bounded connection so each test server
    // can shut down cleanly and release its durable-store advisory lock.
    let lock_qc = serde_json::to_string(session.lock_qc.as_ref().expect("LockQC")).unwrap();
    let decision_qc =
        serde_json::to_string(session.decision_qc.as_ref().expect("DecisionQC")).unwrap();
    let signed_json = serde_json::to_string(&signed).unwrap();
    for validator in &committee.validators {
        let vote = transport
            .call(
                &validator.validator_id,
                &fastswap_catch_up_request(
                    format!("idempotent-catch-up-{}", validator.validator_id),
                    lock_qc.clone(),
                    decision_qc.clone(),
                    signed_json.clone(),
                ),
            )
            .expect("idempotent catch-up RPC")
            .result_as::<FastSwapVoteV1>()
            .expect("idempotent Effects vote");
        assert_eq!(vote.validator_id, validator.validator_id);
    }
    drop(transport);
    for child in &mut harness.children {
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().expect("query child").is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(child.try_wait().expect("query stopped child").is_some());
    }
    // The lock inode is intentionally durable. Unlinking an advisory-lock
    // pathname at shutdown can let two processes lock different inodes for
    // the same store. The restart below is the behavioral proof that the old
    // process released its flock and that the new process can safely replay.
    assert!(harness
        .node(0)
        .join("fastswap-v1/fastswap-v1.lock")
        .is_file());

    let restart_ready = spawn_node(&mut harness, 0, ports[0], 1);
    wait_for_file(&restart_ready, Duration::from_secs(10));
    let restart_transport = TcpFastSwapTransportV1::new(
        BTreeMap::from([("validator-0".to_owned(), format!("127.0.0.1:{}", ports[0]))]),
        Duration::from_secs(10),
    )
    .expect("restart transport");
    let restarted = restart_transport
        .call(
            "validator-0",
            &fastswap_status_request("restart-status", swap_id_hex),
        )
        .expect("restart terminal status")
        .result_as::<FastSwapStatusResponseV1>()
        .expect("restart status response");
    assert_eq!(
        restarted.record.as_ref().map(|record| record.status),
        Some(FastSwapLocalStatusV1::Applied)
    );
    assert!(restarted.terminal_tombstone.is_some());
    drop(restart_transport);

    let timings = session.last_timings.expect("wallet timings");
    eprintln!(
        "FASTSWAP_LOCAL_SIX_PASS preview_ms={preview_ms} settlement_ms={settlement_ms} prepare_qc_ms={} decision_qc_ms={} effects_qc_ms={} total_ms={} lock_votes={} decision_votes={} effects_votes={} exact_six=true conserved=true restart=true",
        timings.prepare_qc_ms,
        timings.decision_qc_ms,
        timings.effects_qc_ms,
        timings.total_ms,
        terminal.lock_qc.votes.len(),
        terminal.decision_qc.votes.len(),
        terminal.effects_qc.votes.len(),
    );
}

#[test]
#[ignore = "explicit 100-warm-operation real-process performance gate"]
fn fastswap_local_six_process_hundred_warm_wallet_operations_meet_gate() {
    const WARM_OPERATIONS: usize = 100;
    let mut harness = Harness::new();
    let seed_dir = harness.root.join("seed");
    init(InitOptions {
        data_dir: seed_dir.clone(),
        chain_id: CHAIN_ID.to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: VALIDATORS as u32,
    })
    .expect("initialize performance seed");
    let (base, committee, intents, owner_0, owner_1) = fixture(&seed_dir, WARM_OPERATIONS + 1);
    seed_canonical_fastswap(&seed_dir, &base, &committee);
    for index in 0..VALIDATORS {
        let data_dir = harness.node(index);
        copy_dir(&seed_dir, &data_dir);
        rewrite_node_identity(&data_dir, &format!("validator-{index}"));
    }
    let base_port = free_base_port();
    let ports = (0..VALIDATORS)
        .map(|index| base_port + index as u16)
        .collect::<Vec<_>>();
    let ready = ports
        .iter()
        .enumerate()
        // Leave bounded headroom for deliberate reconnects after the server's
        // 30-second idle close; the transport regression test separately
        // proves concurrent callers cannot proliferate same-lane sockets.
        .map(|(index, port)| spawn_node(&mut harness, index, *port, 64))
        .collect::<Vec<_>>();
    for path in &ready {
        wait_for_file(path, Duration::from_secs(15));
    }
    let transport = TcpFastSwapTransportV1::new(
        ports
            .iter()
            .enumerate()
            .map(|(index, port)| (format!("validator-{index}"), format!("127.0.0.1:{port}")))
            .collect(),
        Duration::from_secs(20),
    )
    .expect("performance transport");
    transport
        .prewarm_fastswap_runtime_v2(&committee)
        .expect("wire-v2 negotiation and lane prewarm");
    assert!(transport.compact_payloads_enabled());

    let mut cold_ms = 0u128;
    let mut warm_total_ms = Vec::with_capacity(WARM_OPERATIONS);
    let mut warm_sign_ms = Vec::with_capacity(WARM_OPERATIONS);
    let mut warm_preview_ms = Vec::with_capacity(WARM_OPERATIONS);
    let mut warm_prepare_ms = Vec::with_capacity(WARM_OPERATIONS);
    let mut warm_decision_ms = Vec::with_capacity(WARM_OPERATIONS);
    let mut warm_effects_ms = Vec::with_capacity(WARM_OPERATIONS);
    let mut last_replay = None;
    for (index, intent) in intents.into_iter().enumerate() {
        let operation_started = Instant::now();
        let signing_started = Instant::now();
        let signed = wallet_dual_sign_fastswap_intent(&owner_0, &owner_1, intent)
            .expect("parallel dual signing");
        let sign_ms = signing_started.elapsed().as_millis();
        let preview_started = Instant::now();
        let expected = preview_fastswap(&signed, &committee, &transport).expect("quorum preview");
        let preview_ms = preview_started.elapsed().as_millis();
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            signed.clone(),
            expected.clone(),
        )
        .expect("performance wallet session");
        let terminal = drive_fastswap_three_wave(&mut session, &committee, &transport, |_| Ok(()))
            .expect("performance settlement");
        let critical_ms = operation_started.elapsed().as_millis();
        assert!(terminal.effects.receipt.accepted);
        assert_eq!(terminal.effects.receipt.code, "fastswap_applied");
        assert!(terminal.lock_qc.votes.len() >= 5);
        assert!(terminal.decision_qc.votes.len() >= 5);
        assert!(terminal.effects_qc.votes.len() >= 5);
        let timings = session.last_timings.clone().expect("stage timings");
        if index == 0 {
            cold_ms = critical_ms;
        } else {
            warm_total_ms.push(critical_ms);
            warm_sign_ms.push(sign_ms);
            warm_preview_ms.push(preview_ms);
            warm_prepare_ms.push(u128::from(timings.prepare_qc_ms));
            warm_decision_ms.push(u128::from(timings.decision_qc_ms));
            warm_effects_ms.push(u128::from(timings.effects_qc_ms));
        }

        let replication =
            reconcile_fastswap_replication(&mut session, &committee, &transport, |_| Ok(()))
                .expect("background exact-six repair");
        assert!(
            replication.failed.is_empty(),
            "row {index}: {replication:?}"
        );
        assert!(
            replication.pending.is_empty(),
            "row {index}: {replication:?}"
        );
        let swap_id = signed.swap_id().expect("swap id");
        let swap_id_hex = hex48(&swap_id.0);
        for validator in &committee.validators {
            let status = transport
                .call(
                    &validator.validator_id,
                    &fastswap_status_request(
                        format!("perf-status-{index}-{}", validator.validator_id),
                        swap_id_hex.clone(),
                    ),
                )
                .expect("performance status")
                .result_as::<FastSwapStatusResponseV1>()
                .expect("performance status response");
            assert_eq!(
                status.record.as_ref().map(|record| record.status),
                Some(FastSwapLocalStatusV1::Applied)
            );
            let effects = transport
                .call(
                    &validator.validator_id,
                    &fastswap_effects_request(
                        format!("perf-effects-{index}-{}", validator.validator_id),
                        swap_id_hex.clone(),
                    ),
                )
                .expect("performance effects")
                .result_as::<FastSwapEffectsResponseV1>()
                .expect("performance effects response");
            assert_eq!(effects.effects, Some(expected.clone()));
        }
        assert_conserved(&base, &signed, &expected);
        last_replay = Some((signed, session));
    }

    let (last_signed, last_session) = last_replay.expect("last completed swap");
    let lock_qc = serde_json::to_string(last_session.lock_qc.as_ref().expect("last LockQC"))
        .expect("last LockQC JSON");
    let decision_qc =
        serde_json::to_string(last_session.decision_qc.as_ref().expect("last DecisionQC"))
            .expect("last DecisionQC JSON");
    let signed_json = serde_json::to_string(&last_signed).expect("last intent JSON");
    for validator in &committee.validators {
        transport
            .call(
                &validator.validator_id,
                &fastswap_catch_up_request(
                    format!("perf-final-catch-up-{}", validator.validator_id),
                    lock_qc.clone(),
                    decision_qc.clone(),
                    signed_json.clone(),
                ),
            )
            .expect("final idempotent catch-up")
            .result_as::<FastSwapVoteV1>()
            .expect("final catch-up Effects vote");
    }

    let p50 = percentile(&warm_total_ms, 50);
    let p95 = percentile(&warm_total_ms, 95);
    let p99 = percentile(&warm_total_ms, 99);
    eprintln!(
        "FASTSWAP_LOCAL_SIX_100_WARM cold_ms={cold_ms} p50_ms={p50} p95_ms={p95} p99_ms={p99} sign_p50_ms={} preview_p50_ms={} prepare_p50_ms={} decision_p50_ms={} effects_p50_ms={} accepted=101 exact_six=101 conserved=101",
        percentile(&warm_sign_ms, 50),
        percentile(&warm_preview_ms, 50),
        percentile(&warm_prepare_ms, 50),
        percentile(&warm_decision_ms, 50),
        percentile(&warm_effects_ms, 50),
    );
    assert!(p50 <= 2_000, "warm p50 {p50}ms exceeded 2000ms");
    assert!(p95 <= 3_000, "warm p95 {p95}ms exceeded 3000ms");
    assert!(p99 <= 5_000, "warm p99 {p99}ms exceeded 5000ms");
    assert!(
        cold_ms <= 5_000,
        "cold first swap {cold_ms}ms exceeded 5000ms"
    );
}
