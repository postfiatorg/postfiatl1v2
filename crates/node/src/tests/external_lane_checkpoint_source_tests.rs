// Included in `genesis_external_lane_tests::release_path::checkpoint_source`:
// the Base checkpoint source commands against a mock Base JSON-RPC.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

const LOCK: &str = "0x1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a";
const TOKEN: &str = "0x2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b";
const OTHER_TOKEN: &str = "0x3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c";
const HOLDER: [u8; 20] = [0x11; 20];
const LOCK_CODE: &[u8] = b"\x60\x80PFTLockV1 runtime";
const TOKEN_CODE: &[u8] = b"\x60\x80PFT ERC-20 runtime";
const BLOCK: u64 = 1_000;
const HEAD: u64 = 1_100;
const LOCK_NONCE: u64 = 7;

/// A Base chain as one JSON-RPC endpoint serves it.
#[derive(Clone)]
struct BaseState {
    chain_id: u64,
    head: u64,
    finalized: u64,
    blocks: BTreeMap<u64, Value>,
    receipts: BTreeMap<u64, Vec<Value>>,
    codes: BTreeMap<String, Vec<u8>>,
}

impl BaseState {
    fn respond(&self, method: &str, params: &Value) -> Value {
        let quantity = |value: u64| Value::String(format!("0x{value:x}"));
        let block_number = |tag: &Value| match tag.as_str().expect("block tag") {
            "finalized" => self.finalized,
            "latest" => self.head,
            hex => u64::from_str_radix(hex.trim_start_matches("0x"), 16).expect("block number"),
        };
        match method {
            "eth_chainId" => quantity(self.chain_id),
            "eth_blockNumber" => quantity(self.head),
            "eth_getBlockByNumber" => self
                .blocks
                .get(&block_number(&params[0]))
                .cloned()
                .unwrap_or(Value::Null),
            "eth_getBlockByHash" => self
                .blocks
                .values()
                .find(|block| block["hash"] == params[0])
                .cloned()
                .unwrap_or(Value::Null),
            "eth_getCode" => {
                let address = params[0].as_str().expect("address").to_ascii_lowercase();
                let code = self.codes.get(&address).cloned().unwrap_or_default();
                Value::String(format!("0x{}", bytes_to_hex(&code)))
            }
            "eth_getTransactionReceipt" => self
                .receipts
                .values()
                .flatten()
                .find(|receipt| receipt["transactionHash"] == params[0])
                .cloned()
                .unwrap_or(Value::Null),
            "eth_getBlockReceipts" => self
                .receipts
                .get(&block_number(&params[0]))
                .map(|receipts| Value::Array(receipts.clone()))
                .unwrap_or(Value::Null),
            other => panic!("mock Base RPC has no method {other}"),
        }
    }
}

/// A local HTTP JSON-RPC server answering from a [`BaseState`] until dropped.
struct MockBase {
    url: String,
    state: Arc<Mutex<BaseState>>,
    stop: Arc<AtomicBool>,
    address: std::net::SocketAddr,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MockBase {
    fn serve(state: BaseState) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock Base RPC");
        let address = listener.local_addr().expect("mock address");
        let state = Arc::new(Mutex::new(state));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let state = Arc::clone(&state);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let mut stream = stream.expect("accept mock Base request");
                    let request = read_request(&mut stream);
                    let method = request["method"].as_str().expect("method").to_string();
                    let result = state.lock().expect("state").respond(&method, &request["params"]);
                    let body = serde_json::to_vec(&json!({"jsonrpc": "2.0", "id": 1, "result": result}))
                        .expect("response json");
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(&body);
                }
            })
        };
        Self {
            url: format!("http://{address}"),
            state,
            stop,
            address,
            thread: Some(thread),
        }
    }

    fn edit(&self, edit: impl FnOnce(&mut BaseState)) {
        edit(&mut self.state.lock().expect("state"));
    }
}

impl Drop for MockBase {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_request(stream: &mut TcpStream) -> Value {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = stream.read(&mut chunk).expect("read mock request");
        assert!(count > 0, "request closed early");
        request.extend_from_slice(&chunk[..count]);
        let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") else {
            continue;
        };
        let headers = std::str::from_utf8(&request[..end]).expect("headers").to_ascii_lowercase();
        let length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .expect("content length");
        if request.len() >= end + 4 + length {
            return serde_json::from_slice(&request[end + 4..end + 4 + length]).expect("request json");
        }
    }
}

fn hex32(byte: u8) -> String {
    format!("0x{}", "00".repeat(31) + &format!("{byte:02x}"))
}

fn topic_address(address: &[u8]) -> String {
    format!("0x{}{}", "00".repeat(12), bytes_to_hex(address))
}

fn log(address: &str, topics: Vec<String>, data: Vec<u8>) -> Value {
    json!({"address": address, "topics": topics, "data": format!("0x{}", bytes_to_hex(&data))})
}

/// `PFTLockV1.Locked(nonce, holder, keccak(recipient), recipient, amount)`.
fn locked_log(nonce: u64, recipient: &str, amount: u64) -> Value {
    let mut data = word(0x40).to_vec();
    data.extend_from_slice(&word(amount));
    data.extend_from_slice(&word(recipient.len() as u64));
    data.extend_from_slice(recipient.as_bytes());
    data.resize(data.len().div_ceil(32) * 32, 0);
    log(
        LOCK,
        vec![
            format!("0x{}", bytes_to_hex(&keccak(b"Locked(uint64,address,bytes32,bytes,uint64)"))),
            format!("0x{}", bytes_to_hex(&word(nonce))),
            topic_address(&HOLDER),
            format!("0x{}", bytes_to_hex(&keccak(recipient.as_bytes()))),
        ],
        data,
    )
}

/// The ERC-20 `Transfer(holder, lock, amount)` of `token`.
fn transfer_log(token: &str, amount: u64) -> Value {
    log(
        token,
        vec![
            format!("0x{}", bytes_to_hex(&keccak(b"Transfer(address,address,uint256)"))),
            topic_address(&HOLDER),
            topic_address(&[0x1a; 20]),
        ],
        word(amount).to_vec(),
    )
}

fn receipt(index: u64, tx: &str, block_hash: &str, number: u64, kind: &str, logs: Vec<Value>) -> Value {
    let mut receipt = json!({
        "type": kind,
        "status": "0x1",
        "cumulativeGasUsed": format!("0x{:x}", 21_000 * (index + 1)),
        "logs": logs,
        "logsBloom": format!("0x{}", "00".repeat(256)),
        "transactionHash": tx,
        "transactionIndex": format!("0x{index:x}"),
        "blockHash": block_hash,
        "blockNumber": format!("0x{number:x}"),
    });
    if kind == "0x7e" {
        receipt["depositNonce"] = json!("0x2a");
        receipt["depositReceiptVersion"] = json!("0x1");
    }
    receipt
}

/// The Lock transaction of `BLOCK` (index 1, after Base's L1-attributes
/// deposit), one lock-contract log of another event (index 2), one Lock
/// pulling another token (index 3); block `BLOCK + 1` holds one more Lock.
fn base_state(recipient: &str, amount: u64, block_hash_byte: u8) -> BaseState {
    let mut blocks = BTreeMap::new();
    let mut receipts = BTreeMap::new();
    for (number, hash_byte) in [(BLOCK, block_hash_byte), (BLOCK + 1, 0xb1)] {
        let hash = hex32(hash_byte);
        let tx = |index: u64| format!("0x{:02x}{}", number % 256, format!("{index:02x}").repeat(31));
        let mut block_receipts = vec![receipt(0, &tx(0), &hash, number, "0x7e", Vec::new())];
        if number == BLOCK {
            block_receipts.push(receipt(
                1,
                &tx(1),
                &hash,
                number,
                "0x2",
                vec![locked_log(LOCK_NONCE, recipient, amount), transfer_log(TOKEN, amount)],
            ));
            block_receipts.push(receipt(
                2,
                &tx(2),
                &hash,
                number,
                "0x2",
                vec![log(LOCK, vec![format!("0x{}", bytes_to_hex(&keccak(b"Paused(address)")))], Vec::new())],
            ));
            block_receipts.push(receipt(
                3,
                &tx(3),
                &hash,
                number,
                "0x2",
                vec![locked_log(LOCK_NONCE + 1, recipient, amount), transfer_log(OTHER_TOKEN, amount)],
            ));
        } else {
            block_receipts.push(receipt(
                1,
                &tx(1),
                &hash,
                number,
                "0x2",
                vec![locked_log(LOCK_NONCE + 2, recipient, amount), transfer_log(TOKEN, amount)],
            ));
        }
        let root = crate::ethereum_receipt_proof_builder::receipts_root_of(&block_receipts).expect("root");
        blocks.insert(
            number,
            json!({
                "number": format!("0x{number:x}"),
                "hash": hash,
                "receiptsRoot": format!("0x{}", bytes_to_hex(&root)),
                "transactions": block_receipts.iter().map(|r| r["transactionHash"].clone()).collect::<Vec<_>>(),
            }),
        );
        receipts.insert(number, block_receipts);
    }
    BaseState {
        chain_id: 8453,
        head: HEAD,
        finalized: BLOCK,
        blocks,
        receipts,
        codes: BTreeMap::from([
            (LOCK.to_string(), LOCK_CODE.to_vec()),
            (TOKEN.to_string(), TOKEN_CODE.to_vec()),
        ]),
    }
}

fn lock_tx(index: u64) -> String {
    format!("0x{:02x}{}", BLOCK % 256, format!("{index:02x}").repeat(31))
}

/// A six-validator lane chain (committee quorum 5) whose policy pins the
/// mock's runtime code hashes.
fn source_chain(label: &str) -> Chain {
    source_chain_of(label, 6)
}

fn source_chain_of(label: &str, validator_count: u32) -> Chain {
    lane_chain_configured(&format!("cs-{label}"), 86_400, None, validator_count, |policy| {
        policy.lock_runtime_code_hash = format!("0x{}", bytes_to_hex(&keccak(LOCK_CODE)));
        policy.token_runtime_code_hash = format!("0x{}", bytes_to_hex(&keccak(TOKEN_CODE)));
    })
}

fn refusal(error: io::Error) -> ExternalLaneCheckpointRefusal {
    external_lane_checkpoint_refusal(&error).unwrap_or_else(|| panic!("untyped error: {error}"))
}

fn observe(chain: &Chain, rpc: &MockBase, block_number: Option<u64>, name: &str) -> io::Result<PathBuf> {
    let checkpoint_file = chain.data_dir.join(format!("{name}.checkpoint.json"));
    observe_external_lane_checkpoint(ExternalLaneCheckpointObserveOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        base_rpc: rpc.url.clone(),
        block_number,
        checkpoint_file: checkpoint_file.clone(),
    })?;
    Ok(checkpoint_file)
}

/// The committee key of `validator-{index}` in its own private key file.
fn committee_key_file(chain: &Chain, index: usize, key_index: usize) -> PathBuf {
    let key = &chain.keys[key_index];
    let path = chain.data_dir.join(format!("committee-{index}-{key_index}.key.json"));
    fs::write(
        &path,
        serde_json::to_vec_pretty(&ValidatorKeyFile {
            validators: vec![ValidatorKeyRecord {
                node_id: format!("validator-{index}"),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: bytes_to_hex(&key.public_key),
                private_key_hex: bytes_to_hex(&key.private_key),
            }],
        })
        .expect("key json"),
    )
    .expect("write committee key");
    crate::set_private_file_permissions(&path).expect("protect committee key");
    path
}

fn sign(chain: &Chain, checkpoint_file: &Path, rpc: &MockBase, index: usize, name: &str) -> io::Result<PathBuf> {
    sign_as(chain, checkpoint_file, rpc, index, index, name)
}

fn sign_as(
    chain: &Chain,
    checkpoint_file: &Path,
    rpc: &MockBase,
    index: usize,
    key_index: usize,
    name: &str,
) -> io::Result<PathBuf> {
    let vote_file = chain.data_dir.join(format!("{name}.vote-{index}.json"));
    sign_external_lane_checkpoint_vote(ExternalLaneCheckpointSignOptions {
        data_dir: chain.data_dir.clone(),
        checkpoint_file: checkpoint_file.to_path_buf(),
        base_rpc: rpc.url.clone(),
        validator: format!("validator-{index}"),
        validator_key_file: committee_key_file(chain, index, key_index),
        vote_file: vote_file.clone(),
    })?;
    Ok(vote_file)
}

fn assemble(chain: &Chain, checkpoint_file: &Path, vote_files: Vec<PathBuf>, name: &str) -> io::Result<PathBuf> {
    let certificate_file = chain.data_dir.join(format!("{name}.certificate.json"));
    assemble_external_lane_checkpoint_certificate(ExternalLaneCheckpointAssembleOptions {
        data_dir: chain.data_dir.clone(),
        checkpoint_file: checkpoint_file.to_path_buf(),
        vote_files,
        certificate_file: certificate_file.clone(),
    })?;
    Ok(certificate_file)
}

fn lock_proof(
    chain: &Chain,
    rpc: &MockBase,
    certificate_file: &Path,
    transaction_hash: &str,
    log_index: Option<u32>,
    name: &str,
) -> io::Result<PathBuf> {
    let submitter = read_transfer_key_file(&chain.data_dir, None).expect("faucet").address;
    lock_proof_by(chain, rpc, certificate_file, transaction_hash, log_index, &submitter, name)
}

fn lock_proof_by(
    chain: &Chain,
    rpc: &MockBase,
    certificate_file: &Path,
    transaction_hash: &str,
    log_index: Option<u32>,
    submitter: &str,
    name: &str,
) -> io::Result<PathBuf> {
    let operation_file = chain.data_dir.join(format!("{name}.operation.json"));
    build_external_lane_lock_proof(ExternalLaneLockProofOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        base_rpc: rpc.url.clone(),
        transaction_hash: transaction_hash.to_string(),
        certificate_file: certificate_file.to_path_buf(),
        submitter: submitter.to_string(),
        log_index,
        operation_file: operation_file.clone(),
    })?;
    Ok(operation_file)
}

/// The certificate of `BLOCK` from the votes of validators 0–4.
fn certificate_of(chain: &Chain, rpc: &MockBase, name: &str) -> PathBuf {
    let checkpoint = observe(chain, rpc, Some(BLOCK), name).expect("observe");
    let votes = (0..5).map(|index| sign(chain, &checkpoint, rpc, index, name).expect("vote")).collect();
    assemble(chain, &checkpoint, votes, name).expect("assemble")
}

/// The signing state of `validator-{index}` for `checkpoint_file`.
fn signing_state_path(chain: &Chain, checkpoint_file: &Path, index: usize) -> PathBuf {
    let checkpoint: EthereumFinalizedCheckpointV1 =
        read_json_file(checkpoint_file, "checkpoint").expect("checkpoint");
    crate::ethereum_checkpoint_signing::ethereum_checkpoint_signing_state_path(
        &chain.data_dir.join("external-lane-checkpoint-signing"),
        &checkpoint,
        &format!("validator-{index}"),
    )
}

/// The faucet's signed release of the operation in `operation_file`.
fn signed_release(chain: &Chain, operation_file: &Path) -> SignedAssetTransaction {
    let faucet = read_transfer_key_file(&chain.data_dir, None).expect("faucet key");
    let operation: AssetTransactionOperation =
        serde_json::from_slice(&fs::read(operation_file).expect("operation")).expect("operation json");
    let unsigned = UnsignedAssetTransaction {
        chain_id: chain.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&chain.genesis),
        protocol_version: chain.genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: EXTERNAL_LANE_RELEASE_TRANSACTION_KIND_V1.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: faucet.address.clone(),
        fee: 1_000,
        sequence: faucet_sequence(chain),
        operation,
    };
    let private_key = hex_to_bytes(&faucet.private_key_hex).expect("key bytes");
    let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
    SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: faucet.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn amount() -> u64 {
    ACCOUNT_RESERVE + 4_321
}

#[test]
fn observe_reads_the_finalized_block_and_names_the_active_committee() {
    let chain = source_chain("observe");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    rpc.edit(|state| state.finalized = BLOCK + 1);
    let checkpoint_file = observe(&chain, &rpc, None, "finalized").expect("observe finalized");
    let checkpoint: EthereumFinalizedCheckpointV1 = read_json_file(&checkpoint_file, "checkpoint").expect("read");
    let lane = &chain.genesis.external_lanes()[0];
    assert_eq!(checkpoint.block_number, BLOCK + 1, "no block number reads the finalized tag");
    let mut finalized_hash = [0_u8; 32];
    finalized_hash[31] = 0xb1;
    assert_eq!(checkpoint.block_hash, finalized_hash);
    assert_eq!(checkpoint.observed_head_number, HEAD);
    assert_eq!(checkpoint.route_id, lane.id);
    assert_eq!(checkpoint.ethereum_chain_id, 8453);
    assert_eq!(checkpoint.handoff_controller, [0x1a; 20]);
    assert_eq!(checkpoint.wrapped_navcoin_token, [0x2b; 20]);
    assert_eq!(checkpoint.handoff_controller_code_hash, keccak(LOCK_CODE));
    assert_eq!(checkpoint.wrapped_navcoin_code_hash, keccak(TOKEN_CODE));
    assert_eq!(checkpoint.authority_epoch, chain.committee.domain.committee_epoch);
    assert_eq!(checkpoint.committee_root, chain.committee.domain.committee_root);
    assert_eq!(checkpoint.pftl_domain, chain.committee.domain.chain);
    assert_eq!(u64::from(checkpoint.minimum_confirmations), lane.policy.confirmation_depth);
    let explicit = observe(&chain, &rpc, Some(BLOCK), "explicit").expect("observe an older finalized block");
    let explicit: EthereumFinalizedCheckpointV1 = read_json_file(&explicit, "checkpoint").expect("read");
    assert_eq!(explicit.block_number, BLOCK);
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn observe_refuses_chain_finality_depth_code_and_lane_mismatches() {
    let chain = source_chain("observe-refusals");
    let base = base_state(&chain.recipient, amount(), 0xa0);
    type Edit = Box<dyn Fn(&mut BaseState)>;
    let cases: [(&str, Edit, Option<u64>, ExternalLaneCheckpointRefusal); 5] = [
        ("chain", Box::new(|state| state.chain_id = 84_532), None, ExternalLaneCheckpointRefusal::ChainIdMismatch),
        ("unfinalized", Box::new(|_| {}), Some(BLOCK + 1), ExternalLaneCheckpointRefusal::BlockNotFinalized),
        ("depth", Box::new(|state| state.head = BLOCK + 63), None, ExternalLaneCheckpointRefusal::ConfirmationDepthNotReached),
        (
            "lock-code",
            Box::new(|state| {
                state.codes.insert(LOCK.to_string(), b"other lock".to_vec());
            }),
            None,
            ExternalLaneCheckpointRefusal::LockCodeHashMismatch,
        ),
        (
            "token-code",
            Box::new(|state| {
                state.codes.remove(TOKEN);
            }),
            None,
            ExternalLaneCheckpointRefusal::TokenCodeHashMismatch,
        ),
    ];
    for (name, edit, block_number, expected) in cases {
        let rpc = MockBase::serve(base.clone());
        rpc.edit(|state| edit(state));
        let error = observe(&chain, &rpc, block_number, name).unwrap_err();
        assert_eq!(refusal(error), expected, "{name}");
        assert!(!chain.data_dir.join(format!("{name}.checkpoint.json")).exists(), "{name}");
    }
    let rpc = MockBase::serve(base);
    let error = observe_external_lane_checkpoint(ExternalLaneCheckpointObserveOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "other-lane".to_string(),
        base_rpc: rpc.url.clone(),
        block_number: None,
        checkpoint_file: chain.data_dir.join("other.checkpoint.json"),
    })
    .unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::LaneNotConfigured);
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn sign_re_observes_refuses_non_members_and_never_signs_two_checkpoints_of_one_block() {
    let chain = source_chain("sign");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let fork = MockBase::serve(base_state(&chain.recipient, amount(), 0xf0));
    let checkpoint = observe(&chain, &rpc, Some(BLOCK), "a").expect("observe");
    let forked = observe(&chain, &fork, Some(BLOCK), "b").expect("observe the fork");

    // A member whose own RPC observes another block refuses to sign.
    let error = sign(&chain, &checkpoint, &fork, 1, "mismatch").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::ObservationMismatch);
    // Not a member of the active committee, or not its committee key.
    let error = sign_as(&chain, &checkpoint, &rpc, 6, 0, "outsider").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::NotCommitteeMember);
    let error = sign_as(&chain, &checkpoint, &rpc, 2, 3, "wrong-key").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::SigningKeyMismatch);

    // One vote per lane and block: the same checkpoint again returns the
    // same vote; a different checkpoint of the block is refused.
    let first = sign(&chain, &checkpoint, &rpc, 0, "first").expect("sign");
    let again = sign(&chain, &checkpoint, &rpc, 0, "again").expect("sign again");
    assert_eq!(fs::read(&first).expect("vote"), fs::read(&again).expect("vote"));
    let error = sign(&chain, &forked, &fork, 0, "conflict").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::ConflictingCheckpoint);
    assert!(!chain.data_dir.join("conflict.vote-0.json").exists());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn assemble_keeps_exactly_the_quorum_of_distinct_member_votes() {
    let chain = source_chain("assemble");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let checkpoint = observe(&chain, &rpc, None, "assemble").expect("observe");
    let votes: Vec<PathBuf> = (0..6)
        .map(|index| sign(&chain, &checkpoint, &rpc, index, "assemble").expect("vote"))
        .collect();
    let refused = |files: Vec<PathBuf>, name: &str| {
        refusal(assemble(&chain, &checkpoint, files, name).unwrap_err())
    };
    assert_eq!(
        refused([&votes[..4], &votes[..1]].concat(), "duplicate"),
        ExternalLaneCheckpointRefusal::DuplicateVote
    );
    let mut outsider: EthereumCheckpointVoteV1 = read_json_file(&votes[5], "vote").expect("vote");
    outsider.validator_id = "validator-9".to_string();
    let outsider_file = chain.data_dir.join("outsider.vote.json");
    fs::write(&outsider_file, serde_json::to_vec(&outsider).expect("vote json")).expect("write vote");
    assert_eq!(
        refused([&votes[..4], &[outsider_file]].concat(), "outsider"),
        ExternalLaneCheckpointRefusal::VoteNotMember
    );
    assert_eq!(refused(Vec::new(), "none"), ExternalLaneCheckpointRefusal::BelowQuorum);
    // Six valid votes give the certificate of the quorum's five lowest ids,
    // in any input order.
    let reversed: Vec<PathBuf> = votes.iter().rev().cloned().collect();
    let certificate = assemble(&chain, &checkpoint, reversed, "six").expect("assemble six");
    let parsed: EthereumCheckpointCertificateV1 =
        read_json_file(&certificate, "certificate").expect("certificate");
    let ids: Vec<&str> = parsed.votes.iter().map(|vote| vote.validator_id.as_str()).collect();
    assert_eq!(ids, ["validator-0", "validator-1", "validator-2", "validator-3", "validator-4"]);
    postfiat_bridge::verify_ethereum_checkpoint_certificate(&chain.committee, &parsed)
        .expect("the bridge verifier accepts the certificate");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn checkpoint_source_releases_a_base_lock_end_to_end_at_the_five_of_six_quorum() {
    let chain = source_chain("e2e");
    assert_eq!((chain.committee.domain.validator_count, chain.committee.domain.quorum), (6, 5));
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let fork = MockBase::serve(base_state(&chain.recipient, amount(), 0xf0));

    // Observe, then five members sign from their own RPC.
    let checkpoint = observe(&chain, &rpc, None, "release").expect("observe");
    let votes: Vec<PathBuf> = (0..5).map(|index| sign(&chain, &checkpoint, &rpc, index, "release").expect("vote")).collect();
    // The sixth member observes another hash at the block and votes for it.
    let forked = observe(&chain, &fork, None, "fork").expect("observe the fork");
    let sixth = sign(&chain, &forked, &fork, 5, "fork").expect("the sixth vote");

    // Assembly refuses four votes and the vote on the other block hash.
    let refused = |files: Vec<PathBuf>, name: &str| {
        refusal(assemble(&chain, &checkpoint, files, name).unwrap_err())
    };
    assert_eq!(
        refused(votes[..4].to_vec(), "four"),
        ExternalLaneCheckpointRefusal::BelowQuorum
    );
    assert_eq!(
        refused([&votes[..], &[sixth]].concat(), "sixth"),
        ExternalLaneCheckpointRefusal::VoteOnOtherCheckpoint
    );
    let certificate = assemble(&chain, &checkpoint, votes.clone(), "release").expect("assemble five");
    let parsed: EthereumCheckpointCertificateV1 = read_json_file(&certificate, "certificate").expect("certificate");
    assert_eq!(parsed.votes.len(), 5);

    // The receipt proof of the Lock, twice: byte-identical operations.
    let operation = lock_proof(&chain, &rpc, &certificate, &lock_tx(1), None, "release").expect("lock proof");
    let repeat = lock_proof(&chain, &rpc, &certificate, &lock_tx(1), Some(0), "repeat").expect("lock proof again");
    assert_eq!(fs::read(&operation).expect("operation"), fs::read(&repeat).expect("operation"));
    let AssetTransactionOperation::ExternalLaneReleaseV1(release) =
        serde_json::from_slice(&fs::read(&operation).expect("operation")).expect("operation json")
    else {
        panic!("not an external lane release");
    };
    assert_eq!(
        (release.lane_id.as_str(), release.lock_nonce, release.amount_atoms, release.recipient.as_str()),
        ("base-erc20", LOCK_NONCE, amount(), chain.recipient.as_str())
    );
    assert_eq!((release.source_chain_id, release.lock_contract_address.as_str()), (8453, LOCK));
    assert_eq!((release.proof.receipt_proof.transaction_index, release.proof.log_index), (1, 0));

    // The chain verifies the Lock and credits the recipient once.
    let receipt = admit_and_apply(&chain, &signed_release(&chain, &operation), "release");
    assert!(receipt.accepted, "{receipt:?}");
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
    assert_eq!(ledger.account(&chain.recipient).expect("recipient").balance, amount());
    refused_everywhere(
        &chain,
        &signed_release(&chain, &operation),
        "external_lane_nonce_already_released",
        "again",
    );
    let ledger = NodeStore::new(&chain.data_dir).read_ledger().expect("ledger");
    assert_eq!(ledger.account(&chain.recipient).expect("recipient").balance, amount());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn lock_proof_refuses_another_block_contract_event_or_token() {
    let chain = source_chain("proof-refusals");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let checkpoint = observe(&chain, &rpc, Some(BLOCK), "proof").expect("observe");
    let votes = (0..5).map(|index| sign(&chain, &checkpoint, &rpc, index, "proof").expect("vote")).collect();
    let certificate = assemble(&chain, &checkpoint, votes, "proof").expect("assemble");
    let next_block_lock = format!("0x{:02x}{}", (BLOCK + 1) % 256, "01".repeat(31));
    let cases = [
        ("other-block", next_block_lock, None, ExternalLaneCheckpointRefusal::ReceiptBlockMismatch),
        ("other-contract", lock_tx(1), Some(1), ExternalLaneCheckpointRefusal::LogOfAnotherContract),
        ("other-event", lock_tx(2), Some(0), ExternalLaneCheckpointRefusal::LogOfAnotherEvent),
        ("no-locked", lock_tx(2), None, ExternalLaneCheckpointRefusal::LockedLogMissing),
        ("other-token", lock_tx(3), None, ExternalLaneCheckpointRefusal::TokenMismatch),
    ];
    for (name, tx, log_index, expected) in cases {
        let error = lock_proof(&chain, &rpc, &certificate, &tx, log_index, name).unwrap_err();
        assert_eq!(refusal(error), expected, "{name}");
        assert!(!chain.data_dir.join(format!("{name}.operation.json")).exists(), "{name}");
    }
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn observe_and_sign_follow_a_governed_committee_move() {
    let chain = source_chain_of("move", 4);
    let (epoch2, keys2) = second_committee(&chain);
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    activate_committee(&chain, &epoch2);
    let before = observe(&chain, &rpc, Some(BLOCK), "before").expect("observe before the move");
    let parsed: EthereumFinalizedCheckpointV1 = read_json_file(&before, "checkpoint").expect("read");
    assert_eq!(parsed.authority_epoch, 1, "registering epoch 2 alone does not move the lane");
    govern_committee_move(&chain, &epoch2);

    let after = observe(&chain, &rpc, Some(BLOCK), "after").expect("observe after the move");
    let parsed: EthereumFinalizedCheckpointV1 = read_json_file(&after, "checkpoint").expect("read");
    assert_eq!(
        (parsed.authority_epoch, parsed.committee_root),
        (2, epoch2.domain.committee_root)
    );
    // The epoch-1 checkpoint and the epoch-1 key are refused; the epoch-2
    // key signs.
    let error = sign(&chain, &before, &rpc, 0, "old-checkpoint").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::CheckpointNotForLane);
    let error = sign(&chain, &after, &rpc, 0, "old-key").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::SigningKeyMismatch);
    let moved = Chain {
        data_dir: chain.data_dir.clone(),
        genesis: chain.genesis.clone(),
        keys: keys2,
        committee: epoch2,
        recipient: chain.recipient.clone(),
    };
    sign(&moved, &after, &rpc, 0, "new-key").expect("the epoch-2 member signs");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn lock_proof_refuses_an_invalid_submitter_or_recipient_as_a_typed_refusal() {
    let chain = source_chain("operation-invalid");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let certificate = certificate_of(&chain, &rpc, "submitter");
    let error = lock_proof_by(&chain, &rpc, &certificate, &lock_tx(1), None, "not-an-address", "submitter")
        .unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::OperationInvalid);
    assert!(!chain.data_dir.join("submitter.operation.json").exists());

    let _ = fs::remove_dir_all(&chain.data_dir);

    // A Lock whose recipient text is not a PFTL address.
    let chain = source_chain("recipient-invalid");
    let rpc = MockBase::serve(base_state("not-a-pftl-address", amount(), 0xa0));
    let certificate = certificate_of(&chain, &rpc, "recipient");
    let error = lock_proof(&chain, &rpc, &certificate, &lock_tx(1), None, "recipient").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::OperationInvalid);
    assert!(!chain.data_dir.join("recipient.operation.json").exists());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn sign_refuses_a_corrupt_or_foreign_signing_state_as_a_typed_refusal() {
    let chain = source_chain("state-invalid");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let checkpoint = observe(&chain, &rpc, Some(BLOCK), "state").expect("observe");
    sign(&chain, &checkpoint, &rpc, 0, "state").expect("sign");
    let path = signing_state_path(&chain, &checkpoint, 0);
    let original: Value = serde_json::from_slice(&fs::read(&path).expect("state")).expect("state json");
    let mut foreign = original.clone();
    foreign["validator_id"] = json!("validator-1");
    let mut forged = original.clone();
    let first = forged["vote"]["signature"][0].as_u64().expect("signature byte");
    forged["vote"]["signature"][0] = json!(first ^ 1);
    let mut schema = original.clone();
    schema["schema"] = json!("another-schema");
    for (name, state) in [("foreign", foreign), ("forged", forged), ("schema", schema)] {
        fs::write(&path, serde_json::to_vec_pretty(&state).expect("state json")).expect("write state");
        let error = sign(&chain, &checkpoint, &rpc, 0, name).unwrap_err();
        assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::SigningStateInvalid, "{name}");
        assert!(!chain.data_dir.join(format!("{name}.vote-0.json")).exists(), "{name}");
    }
    fs::write(&path, b"{ not json").expect("write state");
    let error = sign(&chain, &checkpoint, &rpc, 0, "garbage").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::SigningStateInvalid);
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn the_node_selects_locked_logs_by_the_bridge_decoders_topic() {
    let topic = postfiat_bridge::locked_event_topic();
    assert_eq!(topic, keccak(b"Locked(uint64,address,bytes32,bytes,uint64)"));
    // The topic the node selects is the one the bridge decoder accepts.
    let recipient = b"pftl-recipient";
    let mut data = word(0x40).to_vec();
    data.extend_from_slice(&word(5));
    data.extend_from_slice(&word(recipient.len() as u64));
    data.extend_from_slice(recipient);
    data.resize(data.len().div_ceil(32) * 32, 0);
    let mut holder = [0_u8; 32];
    holder[12..].copy_from_slice(&HOLDER);
    let mut log = postfiat_bridge::EthereumLogV1 {
        emitter: [0x1a; 20],
        topics: vec![topic, word(LOCK_NONCE), holder, keccak(recipient)],
        data,
    };
    let event = postfiat_bridge::decode_locked_event(&log, [0x1a; 20]).expect("decode");
    assert_eq!((event.nonce, event.amount_atoms), (LOCK_NONCE, 5));
    log.topics[0] = keccak(b"Locked(uint64,address,bytes32,bytes,uint256)");
    assert!(postfiat_bridge::decode_locked_event(&log, [0x1a; 20]).is_err());
}

#[test]
fn sign_refuses_a_key_file_whose_private_key_is_not_its_public_key() {
    let chain = source_chain("key-halves");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let checkpoint = observe(&chain, &rpc, Some(BLOCK), "halves").expect("observe");
    // validator-0's committee public key with validator-1's private key.
    let key_file = chain.data_dir.join("mixed-halves.key.json");
    fs::write(
        &key_file,
        serde_json::to_vec_pretty(&ValidatorKeyFile {
            validators: vec![ValidatorKeyRecord {
                node_id: "validator-0".to_string(),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: bytes_to_hex(&chain.keys[0].public_key),
                private_key_hex: bytes_to_hex(&chain.keys[1].private_key),
            }],
        })
        .expect("key json"),
    )
    .expect("write key");
    crate::set_private_file_permissions(&key_file).expect("protect key");
    let vote_file = chain.data_dir.join("halves.vote-0.json");
    let error = sign_external_lane_checkpoint_vote(ExternalLaneCheckpointSignOptions {
        data_dir: chain.data_dir.clone(),
        checkpoint_file: checkpoint.clone(),
        base_rpc: rpc.url.clone(),
        validator: "validator-0".to_string(),
        validator_key_file: key_file,
        vote_file: vote_file.clone(),
    })
    .unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::SigningKeyMismatch);
    assert!(!vote_file.exists());
    // Nothing durable was written: the member's own key file still signs.
    assert!(!signing_state_path(&chain, &checkpoint, 0).exists());
    sign(&chain, &checkpoint, &rpc, 0, "halves-fixed").expect("the correct key signs");

    // The signature check after signing, below the loader's self-check.
    let signing_bytes = b"checkpoint vote signing bytes";
    let error = crate::external_lane_checkpoint_source::verified_vote_signature(
        &chain.keys[1].private_key,
        &chain.keys[0].public_key,
        signing_bytes,
    )
    .unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::SigningKeyMismatch);
    crate::external_lane_checkpoint_source::verified_vote_signature(
        &chain.keys[0].private_key,
        &chain.keys[0].public_key,
        signing_bytes,
    )
    .expect("matching halves");
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// `state` with the logs of the Lock transaction (index 1) of `BLOCK`
/// replaced, and the block's receipts root rebuilt.
fn with_lock_logs(mut state: BaseState, logs: Vec<Value>) -> BaseState {
    let receipts = state.receipts.get_mut(&BLOCK).expect("block receipts");
    receipts[1]["logs"] = json!(logs);
    let root = crate::ethereum_receipt_proof_builder::receipts_root_of(receipts).expect("root");
    state.blocks.get_mut(&BLOCK).expect("block")["receiptsRoot"] = json!(format!("0x{}", bytes_to_hex(&root)));
    state
}

#[test]
fn lock_proof_refuses_two_locked_logs_a_mismatched_recipient_hash_or_a_forged_certificate() {
    // Two `Locked` logs of the lock contract and no `--log-index`.
    let chain = source_chain("ambiguous");
    let two = vec![
        locked_log(LOCK_NONCE, &chain.recipient, amount()),
        locked_log(LOCK_NONCE + 9, &chain.recipient, amount()),
        transfer_log(TOKEN, amount()),
    ];
    let rpc = MockBase::serve(with_lock_logs(base_state(&chain.recipient, amount(), 0xa0), two));
    let certificate = certificate_of(&chain, &rpc, "ambiguous");
    let error = lock_proof(&chain, &rpc, &certificate, &lock_tx(1), None, "ambiguous").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::LockedLogAmbiguous);
    assert!(!chain.data_dir.join("ambiguous.operation.json").exists());
    let _ = fs::remove_dir_all(&chain.data_dir);

    // Recipient bytes that do not hash to the indexed recipient hash.
    let chain = source_chain("recipient-hash");
    let mut locked = locked_log(LOCK_NONCE, &chain.recipient, amount());
    locked["topics"][3] = json!(format!("0x{}", bytes_to_hex(&keccak(b"another recipient"))));
    let rpc = MockBase::serve(with_lock_logs(
        base_state(&chain.recipient, amount(), 0xa0),
        vec![locked, transfer_log(TOKEN, amount())],
    ));
    let certificate = certificate_of(&chain, &rpc, "hash");
    let error = lock_proof(&chain, &rpc, &certificate, &lock_tx(1), None, "hash").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::LockedEventInvalid);
    assert!(!chain.data_dir.join("hash.operation.json").exists());
    let _ = fs::remove_dir_all(&chain.data_dir);

    // A certificate whose votes do not verify.
    let chain = source_chain("forged-certificate");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let certificate = certificate_of(&chain, &rpc, "forged");
    let mut forged: EthereumCheckpointCertificateV1 =
        read_json_file(&certificate, "certificate").expect("certificate");
    forged.votes[0].signature[0] ^= 1;
    let forged_file = chain.data_dir.join("forged.certificate.json");
    fs::write(&forged_file, serde_json::to_vec(&forged).expect("certificate json")).expect("write certificate");
    let error = lock_proof(&chain, &rpc, &forged_file, &lock_tx(1), None, "forged").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::CertificateInvalid);
    assert!(!chain.data_dir.join("forged.operation.json").exists());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn observe_sign_assemble_and_proof_refuse_a_lane_whose_committee_is_not_registered() {
    let chain = source_chain("committee-missing");
    let rpc = MockBase::serve(base_state(&chain.recipient, amount(), 0xa0));
    let checkpoint = observe(&chain, &rpc, Some(BLOCK), "registered").expect("observe");
    let votes: Vec<PathBuf> =
        (0..5).map(|index| sign(&chain, &checkpoint, &rpc, index, "registered").expect("vote")).collect();
    let certificate = assemble(&chain, &checkpoint, votes.clone(), "registered").expect("assemble");
    // The ledger no longer holds the committee the lane names.
    let store = NodeStore::new(&chain.data_dir);
    let mut ledger = store.read_ledger().expect("ledger");
    ledger.fastswap_committees.clear();
    store.write_ledger(&ledger).expect("write ledger without the committee");
    let missing = ExternalLaneCheckpointRefusal::CommitteeMissing;
    assert_eq!(refusal(observe(&chain, &rpc, Some(BLOCK), "missing").unwrap_err()), missing);
    assert_eq!(refusal(sign(&chain, &checkpoint, &rpc, 5, "missing").unwrap_err()), missing);
    assert_eq!(refusal(assemble(&chain, &checkpoint, votes, "missing").unwrap_err()), missing);
    let error = lock_proof(&chain, &rpc, &certificate, &lock_tx(1), None, "missing").unwrap_err();
    assert_eq!(refusal(error), missing);
    let _ = fs::remove_dir_all(&chain.data_dir);
}

#[test]
fn lock_proof_accepts_an_unrelated_same_shaped_transfer_beside_the_policy_token_transfer() {
    let chain = source_chain("unrelated-transfer");
    // A smart-contract wallet or batch: another contract's `Transfer` of the
    // same amount from the holder to the lock, beside the token's own.
    let logs = vec![
        locked_log(LOCK_NONCE, &chain.recipient, amount()),
        transfer_log(OTHER_TOKEN, amount()),
        transfer_log(TOKEN, amount()),
    ];
    let rpc = MockBase::serve(with_lock_logs(base_state(&chain.recipient, amount(), 0xa0), logs));
    let certificate = certificate_of(&chain, &rpc, "unrelated");
    let operation =
        lock_proof(&chain, &rpc, &certificate, &lock_tx(1), None, "unrelated").expect("the policy token's transfer suffices");
    let AssetTransactionOperation::ExternalLaneReleaseV1(release) =
        serde_json::from_slice(&fs::read(&operation).expect("operation")).expect("operation json")
    else {
        panic!("not an external lane release");
    };
    assert_eq!((release.lock_nonce, release.amount_atoms), (LOCK_NONCE, amount()));
    // A Lock receipt whose only matching transfer is another contract's is
    // still refused.
    let error = lock_proof(&chain, &rpc, &certificate, &lock_tx(3), None, "only-other").unwrap_err();
    assert_eq!(refusal(error), ExternalLaneCheckpointRefusal::TokenMismatch);
    assert!(!chain.data_dir.join("only-other.operation.json").exists());
    let _ = fs::remove_dir_all(&chain.data_dir);
}

/// Read-only qualification against a real Base Sepolia RPC at the http URL
/// in `POSTFIAT_LIVE_BASE_RPC`: the rehearsal lock and token code hashes,
/// the `finalized` block, five votes and the certificate, and the receipt
/// trie of that block (OP Stack deposit receipts included) rebuilt to its
/// `receiptsRoot` for its first and last transaction.
#[test]
#[ignore = "needs POSTFIAT_LIVE_BASE_RPC"]
fn live_base_sepolia_checkpoint_of_the_rehearsal_lock() {
    let rpc = std::env::var("POSTFIAT_LIVE_BASE_RPC").expect("POSTFIAT_LIVE_BASE_RPC");
    let chain = lane_chain_configured("cs-live", 86_400, None, 6, |policy| {
        policy.chain_id = 84_532;
        policy.lock_contract_address = "0xe36e7fc384564419187546c19719a5654266de0b".to_string();
        policy.token_address = "0x18fe4ca51a0ce042688d5a54917319d03b527d92".to_string();
        policy.lock_runtime_code_hash =
            "0x328a93e99cf9659bdb44807d60a9e0328e0ff867d0b584dc2eeed3b0d5442fb2".to_string();
        policy.token_runtime_code_hash =
            "0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df".to_string();
    });
    let checkpoint_file = chain.data_dir.join("live.checkpoint.json");
    let checkpoint = observe_external_lane_checkpoint(ExternalLaneCheckpointObserveOptions {
        data_dir: chain.data_dir.clone(),
        lane_id: "base-erc20".to_string(),
        base_rpc: rpc.clone(),
        block_number: None,
        checkpoint_file: checkpoint_file.clone(),
    })
    .expect("observe the finalized Base Sepolia block");
    let votes = (0..5)
        .map(|index| {
            let vote_file = chain.data_dir.join(format!("live.vote-{index}.json"));
            sign_external_lane_checkpoint_vote(ExternalLaneCheckpointSignOptions {
                data_dir: chain.data_dir.clone(),
                checkpoint_file: checkpoint_file.clone(),
                base_rpc: rpc.clone(),
                validator: format!("validator-{index}"),
                validator_key_file: committee_key_file(&chain, index, index),
                vote_file: vote_file.clone(),
            })
            .expect("re-observe and sign");
            vote_file
        })
        .collect();
    assemble(&chain, &checkpoint_file, votes, "live").expect("certificate");
    let endpoint = crate::ethereum_checkpoint_signing::EthereumRpcEndpoint::parse(&rpc).expect("endpoint");
    let block = crate::ethereum_checkpoint_signing::ethereum_rpc_call(
        &endpoint,
        "eth_getBlockByNumber",
        json!([format!("0x{:x}", checkpoint.block_number), false]),
    )
    .expect("block");
    let transactions = block["transactions"].as_array().expect("transactions").clone();
    for transaction in [transactions.first(), transactions.last()].into_iter().flatten() {
        let built = crate::ethereum_receipt_proof_builder::receipt_proof_from_rpc(
            &endpoint,
            84_532,
            transaction.as_str().expect("hash"),
        )
        .expect("the rebuilt receipt trie matches the block's receiptsRoot");
        assert_eq!(built.receipts_root, checkpoint.receipts_root);
        assert_eq!(built.block_number, checkpoint.block_number);
    }
    eprintln!(
        "live Base Sepolia checkpoint: block {} hash 0x{} head {} receipts {} epoch {}",
        checkpoint.block_number,
        bytes_to_hex(&checkpoint.block_hash),
        checkpoint.observed_head_number,
        transactions.len(),
        checkpoint.authority_epoch
    );
    let _ = fs::remove_dir_all(&chain.data_dir);
}
