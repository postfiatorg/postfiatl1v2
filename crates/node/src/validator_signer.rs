//! The validator signer that `rpc-serve` signs finality with.
//!
//! `KeyFileSigner` reads the plaintext key file (`--finality-key-file`) in the
//! signing process. `SocketSigner` asks a separate process,
//! `postfiat-node validator-signer-serve`, over a Unix socket, so the serving
//! process never holds the key. Both implement `ValidatorSigner`, and every
//! finality signing site resolves its signer with `validator_signer_for`:
//! the socket signer `rpc-serve --finality-signer-socket` registered for that
//! path, otherwise the key file. A registered socket never falls back to a
//! file.
//!
//! The protocol is one request per connection: a 4-byte big-endian length,
//! then that many bytes of JSON, answered the same way. The server checks the
//! connecting peer's uid against its allow-list before it reads anything,
//! bounds the request size and gives each request a deadline. It signs only
//! the message kinds the key-file signer signs, chosen by ML-DSA context, and
//! never returns key material.

use super::*;
use postfiat_ordering_fast::{
    CONSENSUS_V2_PROPOSAL_CONTEXT, CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT, CONSENSUS_V2_VOTE_CONTEXT,
};
use std::io::Read;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const VALIDATOR_SIGNER_REQUEST_SCHEMA: &str = "postfiat.validator_signer.request.v1";
pub const VALIDATOR_SIGNER_RESPONSE_SCHEMA: &str = "postfiat.validator_signer.response.v1";
/// The largest messages signed are proposals. Measured on the 1,168 blocks of
/// devnet-2: a consensus-v2 proposal is at most 6.2 KB of JSON without its
/// signature and a block proposal's receipt ids at most 298 bytes. At the
/// batch cap (`MAX_BATCH_TRANSACTIONS`, 1,024 receipt ids of 96 hex
/// characters) a block proposal message is about 100 KB. Votes and the
/// transport authentication message (`transport_auth_message`: the frame's
/// metadata and payload hash, not its payload) are under 1 KB. The request
/// carries the message as hex inside JSON, so 1 MiB leaves about five times
/// the worst case.
pub const VALIDATOR_SIGNER_MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub const VALIDATOR_SIGNER_MAX_RESPONSE_BYTES: usize = 64 * 1024;
pub const VALIDATOR_SIGNER_DEFAULT_DEADLINE_MS: u64 = 5_000;
const VALIDATOR_SIGNER_MAX_CONNECTIONS: usize = 32;
/// The socket's mode after bind. On Linux a connect to a Unix stream socket
/// needs write permission on the socket file, so the serving user reaches it
/// through the socket's group; the peer-uid allow-list is the real gate.
pub const VALIDATOR_SIGNER_SOCKET_MODE: u32 = 0o660;
/// The context of the authentication on every transport envelope a node
/// sends its peers (`sign_transport_envelope_auth`).
pub const TRANSPORT_AUTH_CONTEXT: &[u8] = b"postfiat-l1-v2/transport-auth/v1";

/// Which key signs a message: the vote key (`--finality-key-file`, `--key-file`)
/// or the proposal key (`--finality-proposal-key-file`, `--proposal-key-file`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorSignerKey {
    Vote,
    Proposal,
}

/// The message kinds rpc-serve signs as its validator, by ML-DSA context.
/// With the vote key: block certificate votes, block timeout votes,
/// consensus-v2 prepare, precommit and timeout votes, and the transport
/// envelope that carries them to peers. With the proposal key: block proposal
/// signatures and consensus-v2 proposals. Every other context is refused.
pub fn validator_signer_key_for_context(context: &[u8]) -> Option<ValidatorSignerKey> {
    if [
        BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
        BLOCK_TIMEOUT_SIGNATURE_CONTEXT,
        CONSENSUS_V2_VOTE_CONTEXT,
        CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT,
        TRANSPORT_AUTH_CONTEXT,
    ]
    .contains(&context)
    {
        Some(ValidatorSignerKey::Vote)
    } else if [
        BLOCK_PROPOSAL_SIGNATURE_CONTEXT,
        CONSENSUS_V2_PROPOSAL_CONTEXT,
    ]
    .contains(&context)
    {
        Some(ValidatorSignerKey::Proposal)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorPublicKey {
    pub node_id: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
}

impl From<&ValidatorKeyRecord> for ValidatorPublicKey {
    fn from(record: &ValidatorKeyRecord) -> Self {
        Self {
            node_id: record.node_id.clone(),
            algorithm_id: record.algorithm_id.clone(),
            public_key_hex: record.public_key_hex.clone(),
        }
    }
}

/// Signs as the validator. `validator_id` `None` selects the only key.
pub trait ValidatorSigner: Send + Sync {
    /// The public key that signs messages under `context`.
    fn validator_key(
        &self,
        context: &[u8],
        validator_id: Option<&str>,
    ) -> io::Result<ValidatorPublicKey>;
    fn sign(&self, validator_id: &str, context: &[u8], message: &[u8]) -> io::Result<Vec<u8>>;
}

/// The plaintext key file, read into the signing process.
pub struct KeyFileSigner {
    keys: ValidatorKeyFile,
}

impl KeyFileSigner {
    pub fn open(path: &Path) -> io::Result<Self> {
        let keys = read_validator_key_file(path)?;
        validate_validator_key_file(&keys)?;
        Ok(Self { keys })
    }
}

impl ValidatorSigner for KeyFileSigner {
    fn validator_key(
        &self,
        _context: &[u8],
        validator_id: Option<&str>,
    ) -> io::Result<ValidatorPublicKey> {
        select_validator_key_record(&self.keys, validator_id).map(ValidatorPublicKey::from)
    }

    fn sign(&self, validator_id: &str, context: &[u8], message: &[u8]) -> io::Result<Vec<u8>> {
        let record = select_validator_key_record(&self.keys, Some(validator_id))?;
        sign_for_context(&record.private_key_hex, context, message)
    }
}

/// Signs as the key-file code always has: block votes, timeout votes and
/// proposals with a seed derived from the message, the rest hedged.
fn sign_for_context(private_key_hex: &str, context: &[u8], message: &[u8]) -> io::Result<Vec<u8>> {
    let private_key = Zeroizing::new(hex_to_bytes(private_key_hex).map_err(invalid_data)?);
    let seed = if context == BLOCK_CERTIFICATE_SIGNATURE_CONTEXT {
        Some(block_certificate_signature_seed(message)?)
    } else if context == BLOCK_TIMEOUT_SIGNATURE_CONTEXT {
        Some(block_timeout_signature_seed(message)?)
    } else if context == BLOCK_PROPOSAL_SIGNATURE_CONTEXT {
        Some(block_proposal_signature_seed(message)?)
    } else {
        None
    };
    match seed {
        Some(seed) => ml_dsa_65_sign_with_context_seed(&private_key, message, context, &seed),
        None => ml_dsa_65_sign_with_context(&private_key, message, context),
    }
    .map_err(invalid_data)
}

/// A typed refusal: `<code>: <detail>`, with the code first so the request
/// that failed carries it.
fn signer_error(kind: io::ErrorKind, code: &str, detail: impl std::fmt::Display) -> io::Error {
    io::Error::new(kind, format!("{code}: {detail}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorSignerRequest {
    pub schema: String,
    /// `describe` or `sign`.
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorSignerDescription {
    /// The signer process's own uid and pid.
    pub uid: u32,
    pub pid: u32,
    pub vote_keys: Vec<ValidatorPublicKey>,
    pub proposal_keys: Vec<ValidatorPublicKey>,
    #[serde(default)]
    pub durable_double_sign_protection: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorSignerFailure {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorSignerResponse {
    pub schema: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub describe: Option<ValidatorSignerDescription>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ValidatorSignerFailure>,
}

impl ValidatorSignerResponse {
    fn refusal(code: &str, message: impl std::fmt::Display) -> Self {
        Self {
            schema: VALIDATOR_SIGNER_RESPONSE_SCHEMA.to_string(),
            ok: false,
            describe: None,
            public_key_hex: None,
            signature_hex: None,
            error: Some(ValidatorSignerFailure {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

fn write_frame(stream: &mut UnixStream, body: &[u8]) -> io::Result<()> {
    let length = u32::try_from(body.len()).map_err(invalid_data)?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// Reads one frame of at most `max_bytes`, every read bounded by `deadline`.
fn read_frame(stream: &mut UnixStream, max_bytes: usize, deadline: Instant) -> io::Result<Vec<u8>> {
    let mut read_exact = |buffer: &mut [u8]| -> io::Result<()> {
        let mut filled = 0;
        while filled < buffer.len() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(signer_error(
                    io::ErrorKind::TimedOut,
                    "validator_signer_timeout",
                    "request deadline passed",
                ));
            }
            stream.set_read_timeout(Some(remaining))?;
            match stream.read(&mut buffer[filled..]) {
                Ok(0) => {
                    return Err(signer_error(
                        io::ErrorKind::UnexpectedEof,
                        "validator_signer_protocol",
                        "connection closed mid-frame",
                    ))
                }
                Ok(read) => filled += read,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(signer_error(
                        io::ErrorKind::TimedOut,
                        "validator_signer_timeout",
                        "request deadline passed",
                    ))
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    };
    let mut length = [0u8; 4];
    read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > max_bytes {
        return Err(signer_error(
            io::ErrorKind::InvalidData,
            "validator_signer_request_too_large",
            format!("frame of {length} bytes exceeds {max_bytes}"),
        ));
    }
    let mut body = vec![0u8; length];
    read_exact(&mut body)?;
    Ok(body)
}

/// The uid of the process at the other end of `stream`, from the kernel.
#[cfg(target_os = "linux")]
pub fn unix_peer_uid(stream: &UnixStream) -> io::Result<u32> {
    use std::os::unix::io::AsRawFd;
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: the fd is open for the life of `stream`, and the out-pointer
    // and its length describe one `ucred`.
    let status = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(credentials.uid)
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn unix_peer_uid(stream: &UnixStream) -> io::Result<u32> {
    use std::os::unix::io::AsRawFd;
    let mut uid = 0;
    let mut gid = 0;
    // SAFETY: the fd is open for the life of `stream`; both out-pointers are valid.
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}

fn current_euid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorSignerServeOptions {
    /// Signer-owned durable state; never shared with the RPC user or restored.
    pub state_dir: PathBuf,
    /// First use only: refuses an existing directory. Omit on every restart.
    pub initialize_state: bool,
    pub key_file: PathBuf,
    /// Defaults to `key_file`.
    pub proposal_key_file: Option<PathBuf>,
    pub socket: PathBuf,
    pub allowed_peer_uids: Vec<u32>,
    /// `--socket-group`: the group, by name or gid, the socket is given so
    /// the serving user can connect; `None` keeps the signer's primary group.
    pub socket_group: Option<String>,
    pub deadline_ms: u64,
    /// Stop after this many connections (tests); `None` serves forever.
    pub max_connections: Option<usize>,
}

/// The keys the signer process holds, by role.
struct HeldKeys {
    vote: KeyFileSigner,
    proposal: KeyFileSigner,
    guard: Mutex<crate::validator_signer_guard::SignerGuard>,
}

impl HeldKeys {
    fn for_key(&self, key: ValidatorSignerKey) -> &KeyFileSigner {
        match key {
            ValidatorSignerKey::Vote => &self.vote,
            ValidatorSignerKey::Proposal => &self.proposal,
        }
    }

    fn describe(&self) -> ValidatorSignerDescription {
        let public = |signer: &KeyFileSigner| {
            signer
                .keys
                .validators
                .iter()
                .map(ValidatorPublicKey::from)
                .collect()
        };
        ValidatorSignerDescription {
            uid: current_euid(),
            pid: std::process::id(),
            vote_keys: public(&self.vote),
            proposal_keys: public(&self.proposal),
            durable_double_sign_protection: self
                .guard
                .lock()
                .map(|guard| guard.available())
                .unwrap_or(false),
        }
    }

    fn answer(&self, request: &ValidatorSignerRequest) -> ValidatorSignerResponse {
        if request.schema != VALIDATOR_SIGNER_REQUEST_SCHEMA {
            return ValidatorSignerResponse::refusal(
                "validator_signer_protocol",
                format!("unknown request schema `{}`", request.schema),
            );
        }
        let mut response = ValidatorSignerResponse {
            schema: VALIDATOR_SIGNER_RESPONSE_SCHEMA.to_string(),
            ok: true,
            describe: None,
            public_key_hex: None,
            signature_hex: None,
            error: None,
        };
        match request.method.as_str() {
            "describe" => response.describe = Some(self.describe()),
            "sign" => {
                let (Some(validator_id), Some(context), Some(message_hex)) = (
                    request.validator_id.as_deref(),
                    request.context.as_deref(),
                    request.message_hex.as_deref(),
                ) else {
                    return ValidatorSignerResponse::refusal(
                        "validator_signer_protocol",
                        "sign needs validator_id, context and message_hex",
                    );
                };
                let Some(key) = validator_signer_key_for_context(context.as_bytes()) else {
                    return ValidatorSignerResponse::refusal(
                        "validator_signer_context_refused",
                        format!("the signer does not sign under `{context}`"),
                    );
                };
                let Ok(message) = hex_to_bytes(message_hex) else {
                    return ValidatorSignerResponse::refusal(
                        "validator_signer_protocol",
                        "message_hex is not hex",
                    );
                };
                let signer = self.for_key(key);
                let signed = signer
                    .validator_key(context.as_bytes(), Some(validator_id))
                    .and_then(|public| {
                        self.guard
                            .lock()
                            .map_err(|_| {
                                signer_error(
                                    io::ErrorKind::Other,
                                    "validator_signer_state_unavailable",
                                    "guard lock poisoned",
                                )
                            })?
                            .reserve(&public, context.as_bytes(), &message)?;
                        signer
                            .sign(validator_id, context.as_bytes(), &message)
                            .map(|signature| (public, signature))
                    });
                match signed {
                    Ok((public, signature)) => {
                        response.public_key_hex = Some(public.public_key_hex);
                        response.signature_hex = Some(bytes_to_hex(&signature));
                    }
                    Err(error) => {
                        return ValidatorSignerResponse::refusal(
                            "validator_signer_sign_failed",
                            error,
                        )
                    }
                }
            }
            other => {
                return ValidatorSignerResponse::refusal(
                    "validator_signer_protocol",
                    format!("unknown method `{other}`"),
                )
            }
        }
        response
    }
}

/// The gid of `group`: a number, or a name looked up in the group database.
fn resolve_socket_group(group: &str) -> io::Result<u32> {
    if let Ok(gid) = group.parse::<u32>() {
        return Ok(gid);
    }
    let name = std::ffi::CString::new(group).map_err(invalid_data)?;
    // SAFETY: `name` is a NUL-terminated string; the returned record is read
    // before any other group-database call on this thread.
    let record = unsafe { libc::getgrnam(name.as_ptr()) };
    if record.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("--socket-group `{group}` is not a known group"),
        ));
    }
    // SAFETY: `record` is non-null and points to a valid `group`.
    Ok(unsafe { (*record).gr_gid })
}

/// Binds `socket` with mode `VALIDATOR_SIGNER_SOCKET_MODE` and, with
/// `group`, that group. An existing socket is replaced only when stale; a
/// live one is `validator_signer_socket_in_use`, and anything else is kept.
fn bind_signer_socket(socket: &Path, group: Option<u32>) -> io::Result<UnixListener> {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    match std::fs::symlink_metadata(socket) {
        // Probe before unlinking: a socket something still listens on
        // belongs to a live signer; only a stale file is replaced.
        Ok(metadata) if metadata.file_type().is_socket() => match UnixStream::connect(socket) {
            Ok(_) => {
                return Err(signer_error(
                    io::ErrorKind::AddrInUse,
                    "validator_signer_socket_in_use",
                    format!("a live signer answers on {}", socket.display()),
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                std::fs::remove_file(socket)?
            }
            Err(error) => return Err(error),
        },
        Ok(_) => {
            return Err(signer_error(
                io::ErrorKind::AlreadyExists,
                "validator_signer_socket_path_not_a_socket",
                format!("{} exists and is not a socket", socket.display()),
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let listener = UnixListener::bind(socket)?;
    if let Some(gid) = group {
        std::os::unix::fs::chown(socket, None, Some(gid))?;
    }
    std::fs::set_permissions(
        socket,
        std::fs::Permissions::from_mode(VALIDATOR_SIGNER_SOCKET_MODE),
    )?;
    Ok(listener)
}

/// Loads the keys, binds the socket and calls `on_ready` once it accepts.
pub fn validator_signer_serve(
    options: ValidatorSignerServeOptions,
    on_ready: impl FnOnce(&ValidatorSignerDescription),
) -> io::Result<()> {
    if options.allowed_peer_uids.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validator-signer-serve needs at least one --allow-peer-uid",
        ));
    }
    if options.deadline_ms == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validator-signer-serve --deadline-ms must be positive",
        ));
    }
    let vote = KeyFileSigner::open(&options.key_file)?;
    let proposal = KeyFileSigner::open(
        options
            .proposal_key_file
            .as_deref()
            .unwrap_or(&options.key_file),
    )?;
    let fingerprints = vote
        .keys
        .validators
        .iter()
        .chain(&proposal.keys.validators)
        .map(|key| {
            hex_to_bytes(&key.public_key_hex)
                .map(|bytes| hash_hex("postfiat.validator-signer.key.v1", &bytes))
                .map_err(invalid_data)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let guard = crate::validator_signer_guard::SignerGuard::open(
        &options.state_dir,
        options.initialize_state,
        fingerprints,
    )?;
    let keys = Arc::new(HeldKeys {
        vote,
        proposal,
        guard: Mutex::new(guard),
    });
    let group = options
        .socket_group
        .as_deref()
        .map(resolve_socket_group)
        .transpose()?;
    let listener = bind_signer_socket(&options.socket, group)?;
    on_ready(&keys.describe());
    let active = Arc::new(AtomicUsize::new(0));
    let allowed = Arc::new(options.allowed_peer_uids.clone());
    let deadline = Duration::from_millis(options.deadline_ms);
    let mut handles = Vec::new();
    let mut accept_errors = AcceptErrorLog::default();
    for (served, stream) in listener.incoming().enumerate() {
        let Some(mut stream) =
            accepted_or_back_off(stream, &mut accept_errors, Instant::now(), &mut |line| {
                eprintln!("{line}")
            })
        else {
            continue;
        };
        if active.fetch_add(1, Ordering::SeqCst) >= VALIDATOR_SIGNER_MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            let refusal =
                ValidatorSignerResponse::refusal("validator_signer_busy", "too many connections");
            let _ = serde_json::to_vec(&refusal).map(|body| write_frame(&mut stream, &body));
        } else {
            let (keys, allowed, active) = (keys.clone(), allowed.clone(), active.clone());
            handles.push(std::thread::spawn(move || {
                handle_validator_signer_connection(&keys, &allowed, stream, deadline);
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        handles.retain(|handle| !handle.is_finished());
        if options.max_connections.is_some_and(|max| served + 1 >= max) {
            break;
        }
    }
    for handle in handles {
        let _ = handle.join();
    }
    Ok(())
}

/// The accept error log's rate: the first error, then at most one line per
/// `ACCEPT_ERROR_LOG_INTERVAL`, which counts the errors it did not log.
const ACCEPT_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
struct AcceptErrorLog {
    last_logged: Option<Instant>,
    unlogged: u64,
}

/// The accepted stream, or `None` after an accept error and a 50 ms
/// back-off: an accept error (`EMFILE`, `ECONNABORTED`, ...) never ends the
/// serve loop. `emit` receives the first error and then at most one line per
/// second, so a sustained error does not flood the log.
fn accepted_or_back_off(
    accepted: io::Result<UnixStream>,
    log: &mut AcceptErrorLog,
    now: Instant,
    emit: &mut dyn FnMut(&str),
) -> Option<UnixStream> {
    match accepted {
        Ok(stream) => Some(stream),
        Err(error) => {
            let due = log
                .last_logged
                .is_none_or(|last| now.duration_since(last) >= ACCEPT_ERROR_LOG_INTERVAL);
            if due {
                let unlogged = std::mem::take(&mut log.unlogged);
                emit(&if unlogged == 0 {
                    format!("validator-signer-serve: accept failed: {error}; retrying")
                } else {
                    format!(
                        "validator-signer-serve: accept failed: {error}; retrying ({unlogged} more since the last line)"
                    )
                });
                log.last_logged = Some(now);
            } else {
                log.unlogged += 1;
            }
            std::thread::sleep(Duration::from_millis(50));
            None
        }
    }
}

/// Answers one request. A peer whose uid is not allowed gets
/// `validator_signer_peer_refused` before its request is read.
fn handle_validator_signer_connection(
    keys: &HeldKeys,
    allowed_peer_uids: &[u32],
    mut stream: UnixStream,
    deadline: Duration,
) {
    let deadline = Instant::now() + deadline;
    let _ = stream.set_write_timeout(Some(deadline.saturating_duration_since(Instant::now())));
    let response = match unix_peer_uid(&stream) {
        Ok(uid) if allowed_peer_uids.contains(&uid) => {
            match read_frame(&mut stream, VALIDATOR_SIGNER_MAX_REQUEST_BYTES, deadline) {
                Ok(body) => match serde_json::from_slice::<ValidatorSignerRequest>(&body) {
                    Ok(request) => keys.answer(&request),
                    Err(error) => {
                        ValidatorSignerResponse::refusal("validator_signer_protocol", error)
                    }
                },
                Err(error) => {
                    let message = error.to_string();
                    let code = message
                        .split(':')
                        .next()
                        .unwrap_or("validator_signer_protocol");
                    ValidatorSignerResponse::refusal(code, &message)
                }
            }
        }
        Ok(uid) => ValidatorSignerResponse::refusal(
            "validator_signer_peer_refused",
            format!("peer uid {uid} is not allowed"),
        ),
        Err(error) => ValidatorSignerResponse::refusal("validator_signer_peer_refused", error),
    };
    if let Ok(body) = serde_json::to_vec(&response) {
        let _ = write_frame(&mut stream, &body);
    }
}

/// The socket signer client: one connection per request, every request
/// bounded by `deadline`. Built by `describe`, which also records the uid of
/// the process that answered.
#[derive(Debug, Clone)]
pub struct SocketSigner {
    socket: PathBuf,
    deadline: Duration,
    description: ValidatorSignerDescription,
    peer_uid: u32,
}

impl SocketSigner {
    pub fn connect(socket: &Path, deadline: Duration) -> io::Result<Self> {
        let (description, peer_uid) = describe_validator_signer(socket, deadline)?;
        if !description.durable_double_sign_protection {
            return Err(signer_error(
                io::ErrorKind::Other,
                "validator_signer_guard_unavailable",
                "signer does not attest an available durable guard",
            ));
        }
        Ok(Self {
            socket: socket.to_path_buf(),
            deadline,
            description,
            peer_uid,
        })
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub fn description(&self) -> &ValidatorSignerDescription {
        &self.description
    }

    /// The uid the kernel reported for the signer when it described itself.
    pub fn peer_uid(&self) -> u32 {
        self.peer_uid
    }

    fn keys(&self, key: ValidatorSignerKey) -> &[ValidatorPublicKey] {
        match key {
            ValidatorSignerKey::Vote => &self.description.vote_keys,
            ValidatorSignerKey::Proposal => &self.description.proposal_keys,
        }
    }
}

fn exchange(
    socket: &Path,
    deadline: Duration,
    request: &ValidatorSignerRequest,
) -> io::Result<(ValidatorSignerResponse, u32)> {
    let deadline = Instant::now() + deadline;
    let mut stream = UnixStream::connect(socket).map_err(|error| {
        signer_error(
            io::ErrorKind::NotConnected,
            "validator_signer_unreachable",
            format!("connect failed: {error}"),
        )
    })?;
    let peer_uid = unix_peer_uid(&stream)?;
    let body = serde_json::to_vec(request).map_err(invalid_data)?;
    stream.set_write_timeout(Some(deadline.saturating_duration_since(Instant::now())))?;
    write_frame(&mut stream, &body).map_err(|error| match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
            signer_error(io::ErrorKind::TimedOut, "validator_signer_timeout", error)
        }
        _ => signer_error(
            io::ErrorKind::BrokenPipe,
            "validator_signer_unreachable",
            error,
        ),
    })?;
    let body = read_frame(&mut stream, VALIDATOR_SIGNER_MAX_RESPONSE_BYTES, deadline)?;
    let response: ValidatorSignerResponse = serde_json::from_slice(&body).map_err(|error| {
        signer_error(
            io::ErrorKind::InvalidData,
            "validator_signer_protocol",
            error,
        )
    })?;
    if response.schema != VALIDATOR_SIGNER_RESPONSE_SCHEMA {
        return Err(signer_error(
            io::ErrorKind::InvalidData,
            "validator_signer_protocol",
            format!("unknown response schema `{}`", response.schema),
        ));
    }
    if !response.ok {
        let failure = response.error.unwrap_or(ValidatorSignerFailure {
            code: "validator_signer_protocol".to_string(),
            message: "refused without an error".to_string(),
        });
        return Err(signer_error(
            io::ErrorKind::PermissionDenied,
            &failure.code,
            failure.message,
        ));
    }
    Ok((response, peer_uid))
}

/// Asks the signer at `socket` to describe itself; returns its description
/// and the uid the kernel reports for the process that answered.
pub fn describe_validator_signer(
    socket: &Path,
    deadline: Duration,
) -> io::Result<(ValidatorSignerDescription, u32)> {
    let request = ValidatorSignerRequest {
        schema: VALIDATOR_SIGNER_REQUEST_SCHEMA.to_string(),
        method: "describe".to_string(),
        validator_id: None,
        context: None,
        message_hex: None,
    };
    let (response, peer_uid) = exchange(socket, deadline, &request)?;
    let description = response.describe.ok_or_else(|| {
        signer_error(
            io::ErrorKind::InvalidData,
            "validator_signer_protocol",
            "describe answered without a description",
        )
    })?;
    Ok((description, peer_uid))
}

fn select_public_key<'a>(
    keys: &'a [ValidatorPublicKey],
    validator_id: Option<&str>,
) -> io::Result<&'a ValidatorPublicKey> {
    match (validator_id.filter(|id| !id.is_empty()), keys) {
        (Some(id), keys) => keys.iter().find(|key| key.node_id == id),
        (None, [key]) => Some(key),
        (None, _) => None,
    }
    .ok_or_else(|| {
        signer_error(
            io::ErrorKind::InvalidInput,
            "validator_signer_key_missing",
            format!(
                "the signer holds no single key for `{}`",
                validator_id.unwrap_or("")
            ),
        )
    })
}

impl ValidatorSigner for SocketSigner {
    fn validator_key(
        &self,
        context: &[u8],
        validator_id: Option<&str>,
    ) -> io::Result<ValidatorPublicKey> {
        let key = validator_signer_key_for_context(context).ok_or_else(|| {
            signer_error(
                io::ErrorKind::InvalidInput,
                "validator_signer_context_refused",
                String::from_utf8_lossy(context),
            )
        })?;
        select_public_key(self.keys(key), validator_id).cloned()
    }

    /// Signs over the socket and checks the answer: the public key the signer
    /// described and a signature that verifies under it.
    fn sign(&self, validator_id: &str, context: &[u8], message: &[u8]) -> io::Result<Vec<u8>> {
        let expected = self.validator_key(context, Some(validator_id))?;
        let request = ValidatorSignerRequest {
            schema: VALIDATOR_SIGNER_REQUEST_SCHEMA.to_string(),
            method: "sign".to_string(),
            validator_id: Some(validator_id.to_string()),
            context: Some(String::from_utf8_lossy(context).into_owned()),
            message_hex: Some(bytes_to_hex(message)),
        };
        let (response, _) = exchange(&self.socket, self.deadline, &request)?;
        if response.public_key_hex.as_deref() != Some(expected.public_key_hex.as_str()) {
            return Err(signer_error(
                io::ErrorKind::InvalidData,
                "validator_signer_key_mismatch",
                "the signer answered with a key it did not describe",
            ));
        }
        let signature = response
            .signature_hex
            .as_deref()
            .map(hex_to_bytes)
            .transpose()
            .map_err(invalid_data)?
            .unwrap_or_default();
        let public_key = hex_to_bytes(&expected.public_key_hex).map_err(invalid_data)?;
        if !ml_dsa_65_verify_with_context(&public_key, message, &signature, context) {
            return Err(signer_error(
                io::ErrorKind::InvalidData,
                "validator_signer_bad_signature",
                "the signer's signature does not verify",
            ));
        }
        Ok(signature)
    }
}

fn socket_signers() -> &'static Mutex<BTreeMap<PathBuf, Arc<SocketSigner>>> {
    static SIGNERS: OnceLock<Mutex<BTreeMap<PathBuf, Arc<SocketSigner>>>> = OnceLock::new();
    SIGNERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn data_dir_signers() -> &'static Mutex<BTreeMap<PathBuf, Arc<SocketSigner>>> {
    static SIGNERS: OnceLock<Mutex<BTreeMap<PathBuf, Arc<SocketSigner>>>> = OnceLock::new();
    SIGNERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn registry_lock(
    registry: &'static Mutex<BTreeMap<PathBuf, Arc<SocketSigner>>>,
) -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, Arc<SocketSigner>>> {
    registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The environment variable that carries socket mode to child processes.
pub const VALIDATOR_SIGNER_SOCKET_MODE_ENV: &str = "POSTFIAT_VALIDATOR_SIGNER_SOCKET_MODE";
static SOCKET_SIGNER_MODE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Puts this process, and the child processes it starts, in socket mode:
/// from now on no signing path reads a key file. `rpc-serve
/// --finality-signer-socket` enters it before it serves.
pub fn enter_validator_signer_socket_mode() {
    SOCKET_SIGNER_MODE.store(true, Ordering::SeqCst);
    // Called from the CLI before rpc-serve starts its threads.
    std::env::set_var(VALIDATOR_SIGNER_SOCKET_MODE_ENV, "1");
}

/// `path` with symlinks and `.`/`..` resolved when it exists, else as given.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Refuses to read the key file at `path` in socket mode: the process (or
/// its parent rpc-serve) is in socket mode, or `path` lies inside the data
/// directory of a registered socket signer.
pub fn refuse_key_file_in_socket_mode(path: &Path) -> io::Result<()> {
    let process_mode = SOCKET_SIGNER_MODE.load(Ordering::SeqCst)
        || std::env::var_os(VALIDATOR_SIGNER_SOCKET_MODE_ENV).is_some();
    let resolved = canonical(path);
    let in_registered_data_dir = registry_lock(data_dir_signers())
        .keys()
        .any(|data_dir| resolved.starts_with(data_dir));
    if process_mode || in_registered_data_dir {
        return Err(signer_error(
            io::ErrorKind::PermissionDenied,
            "validator_signer_file_refused_in_socket_mode",
            format!(
                "a validator signer socket is in use; the key file {} is not read",
                path.display()
            ),
        ));
    }
    Ok(())
}

/// The signer behind a key path: the socket signer registered for it, else
/// the key file at that path, which socket mode refuses.
pub fn validator_signer_for(key_file: &Path) -> io::Result<Arc<dyn ValidatorSigner>> {
    if let Some(signer) = registry_lock(socket_signers()).get(&canonical(key_file)) {
        return Ok(signer.clone());
    }
    refuse_key_file_in_socket_mode(key_file)?;
    Ok(Arc::new(KeyFileSigner::open(key_file)?))
}

/// The socket signer registered for `rpc-serve` over `data_dir`, which also
/// signs its transport envelopes.
pub fn registered_validator_signer_for_data_dir(data_dir: &Path) -> Option<Arc<SocketSigner>> {
    registry_lock(data_dir_signers())
        .get(&canonical(data_dir))
        .cloned()
}

pub fn registered_socket_signer(socket: &Path) -> Option<Arc<SocketSigner>> {
    registry_lock(socket_signers())
        .get(&canonical(socket))
        .cloned()
}

/// Keeps a socket signer registered while `rpc-serve` runs.
#[derive(Debug)]
pub struct SocketSignerRegistration {
    socket: PathBuf,
    data_dir: PathBuf,
}

impl Drop for SocketSignerRegistration {
    fn drop(&mut self) {
        // `socket` and `data_dir` are the canonical paths registered.
        registry_lock(socket_signers()).remove(&self.socket);
        registry_lock(data_dir_signers()).remove(&self.data_dir);
    }
}

/// Whether a signer that described itself as `description` holds, for both
/// roles, the key the validator registry in `data_dir` records for
/// `node_id`. rpc-serve checks it at startup and on every readiness read.
pub fn validator_signer_holds_registered_key(
    description: &ValidatorSignerDescription,
    data_dir: &Path,
    node_id: &str,
) -> io::Result<()> {
    let registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    description_holds_key(
        description,
        validator_registry_record(&registry, node_id)?,
        node_id,
    )
}

/// The readiness form of [`validator_signer_holds_registered_key`]:
/// `Ok(false)` when the signer holds another key (or none) for `node_id`,
/// and an error only when the registry cannot be read or does not record
/// `node_id`, so an unreadable registry is never reported as a key mismatch.
pub fn validator_signer_key_matches_registry(
    description: &ValidatorSignerDescription,
    data_dir: &Path,
    node_id: &str,
) -> io::Result<bool> {
    let registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let registered = validator_registry_record(&registry, node_id)?;
    Ok(description_holds_key(description, registered, node_id).is_ok())
}

/// Whether `description` holds, for both roles, exactly `registered`.
fn description_holds_key(
    description: &ValidatorSignerDescription,
    registered: &ValidatorRegistryRecord,
    node_id: &str,
) -> io::Result<()> {
    for (key, held) in [
        (ValidatorSignerKey::Vote, &description.vote_keys),
        (ValidatorSignerKey::Proposal, &description.proposal_keys),
    ] {
        let held = select_public_key(held, Some(node_id))?;
        if held.algorithm_id != registered.algorithm_id
            || held.public_key_hex != registered.public_key_hex
        {
            return Err(signer_error(
                io::ErrorKind::InvalidData,
                "validator_signer_key_mismatch",
                format!("the signer's {key:?} key for `{node_id}` is not the registered key"),
            ));
        }
    }
    Ok(())
}

/// Connects `rpc-serve` over `data_dir` to the signer at `socket`: the signer
/// must answer `describe` and hold, for both roles, the key the validator
/// registry records for `node_id`. Nothing is registered on a refusal.
pub fn register_validator_signer_socket(
    socket: &Path,
    data_dir: &Path,
    node_id: &str,
    deadline: Duration,
) -> io::Result<SocketSignerRegistration> {
    let signer = SocketSigner::connect(socket, deadline)?;
    validator_signer_holds_registered_key(signer.description(), data_dir, node_id)?;
    let signer = Arc::new(signer);
    let (socket, data_dir) = (canonical(socket), canonical(data_dir));
    registry_lock(socket_signers()).insert(socket.clone(), signer.clone());
    registry_lock(data_dir_signers()).insert(data_dir.clone(), signer);
    Ok(SocketSignerRegistration { socket, data_dir })
}

#[cfg(test)]
mod validator_signer_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pf-vsig-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("root");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("private test root");
        root
    }

    fn key_file(root: &Path, name: &str, node_id: &str) -> (PathBuf, ValidatorKeyRecord) {
        let pair = ml_dsa_65_keygen_from_seed(&[name.len() as u8; 32]);
        let record = ValidatorKeyRecord {
            node_id: node_id.to_string(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&pair.public_key),
            private_key_hex: bytes_to_hex(&pair.private_key),
        };
        let path = root.join(name);
        write_validator_key_file(
            &path,
            &ValidatorKeyFile {
                validators: vec![record.clone()],
            },
        )
        .expect("write key file");
        (path, record)
    }

    fn serve(root: &Path, allowed: Vec<u32>, deadline_ms: u64) -> (PathBuf, ValidatorKeyRecord) {
        let (key, record) = key_file(root, "vote.json", "validator-0");
        let socket = root.join("signer.sock");
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let options = ValidatorSignerServeOptions {
            state_dir: root.join("guard"),
            initialize_state: true,
            key_file: key,
            proposal_key_file: None,
            socket: socket.clone(),
            allowed_peer_uids: allowed,
            socket_group: None,
            deadline_ms,
            max_connections: None,
        };
        std::thread::spawn(move || {
            validator_signer_serve(options, |_| ready_tx.send(()).expect("ready"))
                .expect("signer serve")
        });
        ready_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("signer ready");
        (socket, record)
    }

    fn raw_request(socket: &Path, body: &[u8]) -> ValidatorSignerResponse {
        let mut stream = UnixStream::connect(socket).expect("connect");
        write_frame(&mut stream, body).expect("write");
        let body = read_frame(
            &mut stream,
            VALIDATOR_SIGNER_MAX_RESPONSE_BYTES,
            Instant::now() + Duration::from_secs(10),
        )
        .expect("response");
        serde_json::from_slice(&body).expect("response json")
    }

    #[test]
    fn signer_describes_its_public_key_and_uid_and_never_returns_key_material() {
        let root = temp_root("describe");
        let (socket, record) = serve(&root, vec![current_euid()], 5_000);
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&socket)
            .expect("socket")
            .permissions()
            .mode();
        // Owner and group read-write: a peer of another user needs write
        // permission on the socket to connect.
        assert_eq!(mode & 0o777, 0o660, "socket mode {:o}", mode & 0o777);
        assert_eq!(VALIDATOR_SIGNER_SOCKET_MODE, 0o660);
        let (description, peer_uid) =
            describe_validator_signer(&socket, Duration::from_secs(5)).expect("describe");
        assert_eq!(peer_uid, current_euid());
        assert_eq!(description.uid, current_euid());
        assert_eq!(
            description.vote_keys,
            vec![ValidatorPublicKey::from(&record)]
        );
        assert_eq!(
            description.proposal_keys,
            vec![ValidatorPublicKey::from(&record)]
        );
        let raw = serde_json::to_string(&raw_request(
            &socket,
            br#"{"schema":"postfiat.validator_signer.request.v1","method":"describe"}"#,
        ))
        .expect("json");
        assert!(
            !raw.contains(&record.private_key_hex),
            "describe leaked the key"
        );
        assert!(!raw.contains("private_key"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_registered_data_directory_refuses_every_key_file_read() {
        let root = temp_root("socket-mode");
        let (socket, _) = serve(&root, vec![current_euid()], 5_000);
        let data_dir = root.join("node");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        let signer =
            Arc::new(SocketSigner::connect(&socket, Duration::from_secs(5)).expect("connect"));
        registry_lock(data_dir_signers()).insert(canonical(&data_dir), signer);
        let refused = |error: io::Error| {
            assert!(
                error
                    .to_string()
                    .starts_with("validator_signer_file_refused_in_socket_mode"),
                "{error}"
            )
        };
        refused(
            validator_signer_for(&data_dir.join(VALIDATOR_KEYS_FILE))
                .err()
                .expect("refused"),
        );
        refused(
            validator_signer_for(&data_dir.join(".").join(VALIDATOR_KEYS_FILE))
                .err()
                .expect("refused"),
        );
        // The local-certificate path that reads or creates every validator
        // key in the data directory.
        refused(
            crate::storage_commit::ensure_validator_keys_for_validators(
                &NodeStore::new(&data_dir),
                &["validator-0".to_string()],
            )
            .expect_err("refused"),
        );
        assert!(!data_dir.join(VALIDATOR_KEYS_FILE).exists());
        registry_lock(data_dir_signers()).remove(&canonical(&data_dir));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn an_accept_error_backs_off_and_keeps_serving() {
        let mut log = AcceptErrorLog::default();
        let mut lines = Vec::new();
        for errno in [libc::EMFILE, libc::ECONNABORTED, libc::ENFILE] {
            let started = Instant::now();
            let accepted = accepted_or_back_off(
                Err(io::Error::from_raw_os_error(errno)),
                &mut log,
                started,
                &mut |line| lines.push(line.to_string()),
            );
            assert!(accepted.is_none());
            assert!(started.elapsed() >= Duration::from_millis(50));
        }
        assert_eq!(
            lines.len(),
            1,
            "three errors within a second log one line: {lines:?}"
        );
        let (stream, _peer) = UnixStream::pair().expect("pair");
        assert!(accepted_or_back_off(Ok(stream), &mut log, Instant::now(), &mut |_| {}).is_some());
    }

    #[test]
    fn a_sustained_accept_error_logs_the_first_line_and_then_one_per_second() {
        let mut log = AcceptErrorLog::default();
        let mut lines = Vec::new();
        let start = Instant::now();
        // 45 EMFILE errors 50 ms apart, as the back-off spaces them: 2.2 s.
        for step in 0..45 {
            let now = start + Duration::from_millis(50 * step);
            let accepted = accepted_or_back_off(
                Err(io::Error::from_raw_os_error(libc::EMFILE)),
                &mut log,
                now,
                &mut |line| lines.push(line.to_string()),
            );
            assert!(accepted.is_none());
        }
        // At 0 s, 1 s and 2 s: three lines, not 45.
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(!lines[0].contains("more since"), "{}", lines[0]);
        assert!(
            lines[1].ends_with("(19 more since the last line)"),
            "{}",
            lines[1]
        );
        assert!(
            lines[2].ends_with("(19 more since the last line)"),
            "{}",
            lines[2]
        );
        assert_eq!(log.unlogged, 4);
    }

    #[test]
    fn readiness_separates_an_unreadable_registry_from_a_key_mismatch() {
        let root = temp_root("registry");
        let (_, record) = key_file(&root, "vote.json", "validator-0");
        let (_, other) = key_file(&root, "another.json", "validator-0");
        let describe = |key: &ValidatorKeyRecord| ValidatorSignerDescription {
            uid: current_euid(),
            pid: std::process::id(),
            durable_double_sign_protection: true,
            vote_keys: vec![ValidatorPublicKey::from(key)],
            proposal_keys: vec![ValidatorPublicKey::from(key)],
        };
        let data_dir = root.join("node");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        let registry = data_dir.join(VALIDATOR_REGISTRY_FILE);
        std::fs::write(
            &registry,
            serde_json::to_vec(&ValidatorRegistry {
                validators: vec![ValidatorRegistryRecord {
                    node_id: "validator-0".to_string(),
                    algorithm_id: record.algorithm_id.clone(),
                    public_key_hex: record.public_key_hex.clone(),
                }],
            })
            .expect("registry json"),
        )
        .expect("write registry");
        let matches =
            |key| validator_signer_key_matches_registry(&describe(key), &data_dir, "validator-0");
        assert!(matches(&record).expect("readable registry"));
        // A signer holding another key: a key mismatch, not a read failure.
        assert!(!matches(&other).expect("readable registry"));
        // An unreadable registry is an error, never a mismatch.
        std::fs::write(&registry, b"{ not a registry").expect("corrupt registry");
        assert!(matches(&record).is_err());
        std::fs::remove_file(&registry).expect("remove registry");
        assert!(matches(&record).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_live_signer_keeps_its_socket_and_a_stale_socket_is_replaced() {
        let root = temp_root("in-use");
        let (socket, _) = serve(&root, vec![current_euid()], 5_000);
        let (key, _) = key_file(&root, "second.json", "validator-0");
        let second = ValidatorSignerServeOptions {
            state_dir: root.join("second-guard"),
            initialize_state: true,
            key_file: key.clone(),
            proposal_key_file: None,
            socket: socket.clone(),
            allowed_peer_uids: vec![current_euid()],
            socket_group: None,
            deadline_ms: 5_000,
            max_connections: Some(1),
        };
        let error = validator_signer_serve(second, |_| {}).expect_err("the socket is in use");
        assert!(
            error
                .to_string()
                .starts_with("validator_signer_socket_in_use"),
            "{error}"
        );
        describe_validator_signer(&socket, Duration::from_secs(5))
            .expect("the first signer still answers");
        // A socket file with no listener is stale and is replaced.
        let stale = root.join("stale.sock");
        drop(UnixListener::bind(&stale).expect("stale listener"));
        assert!(stale.exists());
        let _listener = bind_signer_socket(&stale, None).expect("replace the stale socket");
        // A file that is not a socket is never replaced.
        let regular = root.join("regular.sock");
        std::fs::write(&regular, b"not a socket").expect("regular file");
        let error = bind_signer_socket(&regular, None).expect_err("a regular file is kept");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(
            error
                .to_string()
                .starts_with("validator_signer_socket_path_not_a_socket"),
            "{error}"
        );
        assert_eq!(
            std::fs::read(&regular).expect("regular file"),
            b"not a socket"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn signer_socket_takes_the_named_group() {
        use std::os::unix::fs::MetadataExt;
        let root = temp_root("group");
        // SAFETY: getegid has no preconditions and cannot fail.
        let egid = unsafe { libc::getegid() };
        let gid = resolve_socket_group(&egid.to_string()).expect("numeric group");
        let socket = root.join("group.sock");
        let _listener = bind_signer_socket(&socket, Some(gid)).expect("bind");
        let metadata = std::fs::metadata(&socket).expect("socket");
        assert_eq!((metadata.gid(), metadata.mode() & 0o777), (egid, 0o660));
        let error = resolve_socket_group("pf-no-such-group").expect_err("unknown group");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn signer_signs_only_the_finality_contexts_and_matches_the_key_file_signer() {
        let root = temp_root("sign");
        let (socket, record) = serve(&root, vec![current_euid()], 5_000);
        let signer = SocketSigner::connect(&socket, Duration::from_secs(5)).expect("connect");
        let file = KeyFileSigner::open(&root.join("vote.json")).expect("file signer");
        let encoded = serde_json::to_vec(&serde_json::json!([
            "signer-test",
            "a".repeat(96),
            1,
            1,
            0,
            "b".repeat(96),
            "validator-0",
            "transparent",
            "c".repeat(96),
            "d".repeat(96),
            [],
            "validator-0",
            true
        ]))
        .expect("legacy message");
        let message = encoded.as_slice();
        let signature = signer
            .sign("validator-0", BLOCK_CERTIFICATE_SIGNATURE_CONTEXT, message)
            .expect("socket sign");
        // Seeded contexts sign byte-identically to the key-file signer.
        assert_eq!(
            signature,
            file.sign("validator-0", BLOCK_CERTIFICATE_SIGNATURE_CONTEXT, message)
                .expect("file sign")
        );
        let public_key = hex_to_bytes(&record.public_key_hex).expect("public key");
        assert!(ml_dsa_65_verify_with_context(
            &public_key,
            message,
            &signature,
            BLOCK_CERTIFICATE_SIGNATURE_CONTEXT
        ));
        let mut conflict: serde_json::Value = serde_json::from_slice(message).expect("tuple");
        conflict[12] = false.into();
        let conflict = serde_json::to_vec(&conflict).expect("tuple");
        assert!(signer
            .sign(
                "validator-0",
                BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
                &conflict
            )
            .expect_err("conflicting vote refused")
            .to_string()
            .contains("validator_signer_double_sign"));
        assert_eq!(
            signature,
            signer
                .sign("validator-0", BLOCK_CERTIFICATE_SIGNATURE_CONTEXT, message)
                .expect("identical retry")
        );
        let refused = raw_request(
            &socket,
            serde_json::to_vec(&ValidatorSignerRequest {
                schema: VALIDATOR_SIGNER_REQUEST_SCHEMA.to_string(),
                method: "sign".to_string(),
                validator_id: Some("validator-0".to_string()),
                context: Some("postfiat-l1-v2/tx/v1".to_string()),
                message_hex: Some(bytes_to_hex(message)),
            })
            .expect("json")
            .as_slice(),
        );
        assert_eq!(
            refused.error.expect("refusal").code,
            "validator_signer_context_refused"
        );
        assert!(refused.signature_hex.is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn signer_refuses_a_peer_uid_outside_the_allow_list_and_signs_nothing() {
        let root = temp_root("peer");
        let other = current_euid().wrapping_add(1);
        let (socket, _) = serve(&root, vec![other], 5_000);
        let refused = raw_request(
            &socket,
            serde_json::to_vec(&ValidatorSignerRequest {
                schema: VALIDATOR_SIGNER_REQUEST_SCHEMA.to_string(),
                method: "sign".to_string(),
                validator_id: Some("validator-0".to_string()),
                context: Some(String::from_utf8_lossy(CONSENSUS_V2_VOTE_CONTEXT).into_owned()),
                message_hex: Some("00".to_string()),
            })
            .expect("json")
            .as_slice(),
        );
        assert!(!refused.ok);
        assert_eq!(
            refused.error.expect("refusal").code,
            "validator_signer_peer_refused"
        );
        assert!(refused.signature_hex.is_none() && refused.describe.is_none());
        let error = SocketSigner::connect(&socket, Duration::from_secs(5))
            .expect_err("a refused peer cannot describe");
        assert!(
            error
                .to_string()
                .starts_with("validator_signer_peer_refused"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn signer_bounds_the_request_size_and_its_deadline() {
        let root = temp_root("bounds");
        let (socket, _) = serve(&root, vec![current_euid()], 300);
        // An oversized length prefix is refused before any body is read.
        let mut stream = UnixStream::connect(&socket).expect("connect");
        stream
            .write_all(&((VALIDATOR_SIGNER_MAX_REQUEST_BYTES + 1) as u32).to_be_bytes())
            .expect("prefix");
        let body = read_frame(
            &mut stream,
            VALIDATOR_SIGNER_MAX_RESPONSE_BYTES,
            Instant::now() + Duration::from_secs(10),
        )
        .expect("refusal");
        let refused: ValidatorSignerResponse = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            refused.error.expect("refusal").code,
            "validator_signer_request_too_large"
        );
        // A client that stalls mid-request is cut off at the deadline.
        let mut stalled = UnixStream::connect(&socket).expect("connect");
        stalled.write_all(&100u32.to_be_bytes()).expect("prefix");
        let started = Instant::now();
        let body = read_frame(
            &mut stalled,
            VALIDATOR_SIGNER_MAX_RESPONSE_BYTES,
            Instant::now() + Duration::from_secs(10),
        )
        .expect("timeout refusal");
        let refused: ValidatorSignerResponse = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            refused.error.expect("refusal").code,
            "validator_signer_timeout"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        // The client side has its own deadline: a server that never answers
        // is a typed timeout, not a hang.
        let silent = root.join("silent.sock");
        let _listener = UnixListener::bind(&silent).expect("silent listener");
        let error = describe_validator_signer(&silent, Duration::from_millis(200))
            .expect_err("silent signer");
        assert!(
            error.to_string().starts_with("validator_signer_timeout"),
            "{error}"
        );
        let error =
            describe_validator_signer(&root.join("absent.sock"), Duration::from_millis(200))
                .expect_err("absent signer");
        assert!(
            error
                .to_string()
                .starts_with("validator_signer_unreachable"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
