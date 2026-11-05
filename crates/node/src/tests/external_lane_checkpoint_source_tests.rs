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
