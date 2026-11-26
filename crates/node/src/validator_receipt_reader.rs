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

type Reply = Result<Option<ReceiptPage>, String>;

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
    let answer = (|| -> io::Result<Option<ReceiptPage>> {
        if !(1..=MAX_READ_QUERY_LIMIT).contains(&query.limit) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid receipt query limit",
            ));
        }
        let Some(snapshot) = store.held_transactional_read_snapshot()? else {
            return Ok(None);
        };
        let rows = snapshot.receipts_tail(
            query.tx_id.as_deref(),
            query.limit,
            MAX_REPLY,
            Instant::now() + WAIT,
        )?;
        Ok(Some(ReceiptPage {
            height: snapshot.meta().finalized_height,
            block_hash: snapshot.meta().finalized_block_hash.clone(),
            rows,
        }))
    })()
    .map_err(|e| e.to_string());
    let mut body = serde_json::to_vec(&answer)?;
    if body.len() > MAX_REPLY {
        body = serde_json::to_vec(&Reply::Err(
            "receipt reply exceeds the RPC frame limit; request fewer rows".into(),
        ))?;
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

fn read_held(data_dir: &Path, query: &Query) -> io::Result<Option<ReceiptPage>> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    let mut stream = match socket.connect_timeout(&address(data_dir)?, WAIT) {
        Ok(()) => UnixStream::from(OwnedFd::from(socket)),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) =>
        {
            return Ok(None)
        }
        Err(e) => return Err(e),
    };
    check_peer(&stream)?;
    let body = serde_json::to_vec(query)?;
    if body.len() > MAX_REQUEST {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "oversize receipt query",
        ));
    }
    write_frame(&mut stream, &body)?;
    let reply: Reply = serde_json::from_slice(&read_frame(&mut stream, MAX_REPLY)?)?;
    reply.map_err(io::Error::other)
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
    if let Some(page) = read_held(&options.data_dir, &query)? {
        return Ok((
            page.rows,
            Some(format!(
                "receipt snapshot at height {} block {}",
                page.height, page.block_hash
            )),
        ));
    }
    direct().map(|rows| (rows, None))
}
