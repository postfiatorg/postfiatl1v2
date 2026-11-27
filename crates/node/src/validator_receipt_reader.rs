//! Local RPC workers cannot open redb while the validator owns its writer
//! lease. The validator can still start an MVCC read on its own handle.
//! Only receipt queries cross this channel, under the service user's uid.
use std::io::{self, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{receipts, ReceiptQueryOptions, MAX_READ_QUERY_LIMIT};
use postfiat_storage::transactional::{StorageError, StorageErrorCode};
use postfiat_storage::NodeStore;
use postfiat_types::Receipt;
use serde::{Deserialize, Serialize};
use socket2::{Domain, SockAddr, Socket, Type};

const WAIT: Duration = Duration::from_secs(3);
const MAX_REQUEST: usize = 16 * 1024;
const MAX_REPLY: usize = crate::MAX_NODE_TRANSPORT_FRAME_BYTES as usize;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    tx_id: Option<String>,
    limit: usize,
}

#[derive(Serialize, Deserialize)]
struct ReceiptPage {
    height: u64,
    block_hash: String,
    rows: Vec<Receipt>,
}

/// Why the validator's snapshot gave no page. `Unavailable` (its deadline, the
/// reply budget) means try the direct read; `Fatal` (integrity, corruption,
/// any other storage error) is the answer. On the wire:
/// `{"Err":{"kind":"unavailable"|"fatal","message":"..."}}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
enum SnapshotError {
    Unavailable(String),
    Fatal(String),
}

type Reply = Result<Option<ReceiptPage>, SnapshotError>;

/// The texts an older validator's untyped error carries when its snapshot was
/// only unavailable. Any other untyped error is taken as fatal.
const AVAILABILITY_TEXTS: [&str; 3] = [
    "receipt snapshot deadline exhausted",
    "receipt reply exceeds the RPC frame limit",
    "storage_size_limit: canonical JSON exceeds",
];

fn storage_error(error: StorageError) -> SnapshotError {
    // A size limit inside the read is its deadline or byte budget.
    if error.code() == StorageErrorCode::SizeLimit {
        SnapshotError::Unavailable(error.to_string())
    } else {
        SnapshotError::Fatal(error.to_string())
    }
}

fn address(data_dir: &Path) -> io::Result<SockAddr> {
    // Namespace by actual directory identity; relative paths, bind mounts and
    // symlinks used by separate service units must name the same listener.
    let meta = std::fs::metadata(data_dir)?;
    SockAddr::unix(format!(
        "\0postfiat.receipts.v1.{}.{}.{}",
        uid(),
        meta.dev(),
        meta.ino()
    ))
}

fn uid() -> u32 {
    // SAFETY: geteuid has no arguments or preconditions.
    unsafe { libc::geteuid() }
}

fn check_peer(stream: &UnixStream) -> io::Result<()> {
    if crate::unix_peer_uid(stream)? != uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "incorrect receipt reader uid",
        ));
    }
    Ok(())
}

fn read_frame(stream: &mut UnixStream, max: usize) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + WAIT;
    let mut read = |mut bytes: &mut [u8]| -> io::Result<()> {
        while !bytes.is_empty() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            stream.set_read_timeout(Some(remaining))?;
            let used = stream.read(bytes)?;
            if used == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            bytes = &mut bytes[used..];
        }
        Ok(())
    };
    let mut header = [0; 4];
    read(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversize receipt reader frame",
        ));
    }
    let mut bytes = vec![0; length];
    read(&mut bytes)?;
    Ok(bytes)
}

fn write_frame(stream: &mut UnixStream, body: &[u8]) -> io::Result<()> {
    stream.set_write_timeout(Some(WAIT))?;
    stream.write_all(&(body.len() as u32).to_be_bytes())?;
    stream.write_all(body)
}

fn handle(store: &NodeStore, stream: &mut UnixStream) -> io::Result<()> {
    check_peer(stream)?;
    let query: Query = serde_json::from_slice(&read_frame(stream, MAX_REQUEST)?)?;
    let answer = (|| -> Reply {
        if !(1..=MAX_READ_QUERY_LIMIT).contains(&query.limit) {
            return Err(SnapshotError::Fatal("invalid receipt query limit".into()));
        }
        let Some(snapshot) = store
            .held_transactional_read_snapshot()
            .map_err(storage_error)?
        else {
            return Ok(None);
        };
        let rows = snapshot
            .receipts_tail(
                query.tx_id.as_deref(),
                query.limit,
                MAX_REPLY,
                Instant::now() + WAIT,
            )
            .map_err(storage_error)?;
        Ok(Some(ReceiptPage {
            height: snapshot.meta().finalized_height,
            block_hash: snapshot.meta().finalized_block_hash.clone(),
            rows,
        }))
    })();
    let mut body = serde_json::to_vec(&answer)?;
    if body.len() > MAX_REPLY {
        body = serde_json::to_vec(&Reply::Err(SnapshotError::Unavailable(
            "receipt reply exceeds the RPC frame limit; request fewer rows".into(),
        )))?;
    }
    write_frame(stream, &body)
}

pub struct Service {
    listener: Socket,
    worker: Option<JoinHandle<()>>,
}

impl Service {
    pub fn start(data_dir: &Path) -> io::Result<Self> {
        let listener = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        listener.bind(&address(data_dir)?)?;
        listener.listen(16)?;
        let accepting = UnixListener::from(OwnedFd::from(listener.try_clone()?));
        let store = NodeStore::new(data_dir);
        let worker = thread::Builder::new()
            .name("receipt-reader".into())
            .spawn(move || loop {
                match accepting.accept() {
                    Ok((mut stream, _)) => {
                        let _ = handle(&store, &mut stream);
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            })?;
        Ok(Self {
            listener,
            worker: Some(worker),
        })
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        // Wake accept without an idle polling thread or another connection.
        let _ = self.listener.shutdown(std::net::Shutdown::Both);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// The validator's reply: a page, `Ok(None)` when it does not hold the store,
/// or why it gave none. A reply in an older validator's untyped form is an
/// availability error only when its text is a known availability text.
fn classify_reply(bytes: &[u8]) -> Result<Option<ReceiptPage>, SnapshotError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| SnapshotError::Unavailable(format!("unreadable snapshot reply: {e}")))?;
    match value.get("Err") {
        Some(serde_json::Value::String(text)) => Err(
            if AVAILABILITY_TEXTS.iter().any(|known| text.contains(known)) {
                SnapshotError::Unavailable(text.clone())
            } else {
                SnapshotError::Fatal(text.clone())
            },
        ),
        Some(typed) => Err(serde_json::from_value::<SnapshotError>(typed.clone())
            .unwrap_or_else(|_| SnapshotError::Fatal(format!("unknown snapshot error {typed}")))),
        None => serde_json::from_value(value.get("Ok").cloned().unwrap_or_default())
            .map_err(|e| SnapshotError::Unavailable(format!("unreadable snapshot reply: {e}"))),
    }
}

/// Ask the validator for its snapshot. Every failure to reach it or to read
/// its reply (no listener, a listener under another uid, a timeout, a closed
/// stream) is only unavailability.
fn read_held(data_dir: &Path, query: &Query) -> Result<Option<ReceiptPage>, SnapshotError> {
    let unavailable = |e: io::Error| SnapshotError::Unavailable(e.to_string());
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None).map_err(unavailable)?;
    let mut stream = match socket.connect_timeout(&address(data_dir).map_err(unavailable)?, WAIT) {
        Ok(()) => UnixStream::from(OwnedFd::from(socket)),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) =>
        {
            return Ok(None)
        }
        Err(e) => return Err(unavailable(e)),
    };
    check_peer(&stream).map_err(unavailable)?;
    let body = serde_json::to_vec(query).map_err(|e| unavailable(e.into()))?;
    if body.len() > MAX_REQUEST {
        return Err(SnapshotError::Unavailable("oversize receipt query".into()));
    }
    write_frame(&mut stream, &body).map_err(unavailable)?;
    classify_reply(&read_frame(&mut stream, MAX_REPLY).map_err(unavailable)?)
}

pub fn read(options: ReceiptQueryOptions) -> io::Result<(Vec<Receipt>, Option<String>)> {
    // Try the ordinary path first. A cold query during a commit then goes to
    // the process that already owns the database. A completed commit or old
    // validator binary returns us to the ordinary bounded lease wait.
    let direct = || receipts(options.clone());
    match postfiat_storage::transactional::with_writer_lease_deadline(
        Duration::from_millis(100),
        direct,
    ) {
        Ok(rows) => return Ok((rows, None)),
        Err(e)
            if e.kind() == io::ErrorKind::WouldBlock
                && e.to_string().contains("storage_writer_busy") => {}
        Err(e) => return Err(e),
    }
    let query = Query {
        tx_id: options.tx_id.clone(),
        limit: options.limit.unwrap_or(MAX_READ_QUERY_LIMIT),
    };
    // A snapshot that is only unavailable (its deadline, the reply budget, a
    // listener under another uid) is no answer: take the ordinary bounded
    // direct read, as a validator without the reader would, and say so. A
    // storage error the snapshot reports is the answer.
    let unavailable = match read_held(&options.data_dir, &query) {
        Ok(Some(page)) => {
            return Ok((
                page.rows,
                Some(format!(
                    "receipt snapshot at height {} block {}",
                    page.height, page.block_hash
                )),
            ))
        }
        Ok(None) => None,
        Err(SnapshotError::Unavailable(reason)) => Some(reason),
        Err(SnapshotError::Fatal(error)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("validator receipt snapshot: {error}"),
            ))
        }
    };
    direct().map(|rows| {
        let event = unavailable.map(|reason| {
            format!("receipts read directly; the validator's snapshot was unavailable: {reason}")
        });
        (rows, event)
    })
}
