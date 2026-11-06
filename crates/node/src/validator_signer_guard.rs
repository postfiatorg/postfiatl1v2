//! Signer-owned, persist-before-sign round floors. Never restore or delete this
//! state while its keys remain usable. The RPC user must not be able to write it.
use super::*;
use postfiat_ordering_fast::{
    consensus_v2_proposal_signing_bytes, consensus_v2_timeout_vote_signing_bytes,
    consensus_v2_vote_signing_bytes, CONSENSUS_V2_PROPOSAL_CONTEXT,
    CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT, CONSENSUS_V2_VOTE_CONTEXT,
};
use postfiat_types::{
    ConsensusV2BlockRef, ConsensusV2Domain, ConsensusV2Phase, ConsensusV2Proposal,
    ConsensusV2QcRef, ConsensusV2Round, ConsensusV2Signature, ConsensusV2TimeoutVote,
    ConsensusV2Vote,
};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const MAX_STATE_BYTES: u64 = 1024 * 1024;
const MAX_FAMILIES: usize = 512;
const SCHEMA: &str = "postfiat.validator-signer.guard.v2";

fn refusal(code: &str, detail: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{code}: {detail}"))
}
fn malformed(detail: impl std::fmt::Display) -> io::Error {
    refusal("validator_signer_message_invalid", detail)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mark {
    height: u64,
    view: u64,
    message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: String,
    keys: Vec<String>,
    /// The one chain domain this guard signs for; any other is refused, so a
    /// caller cannot open new families or set floors for invented chains.
    domain: GuardDomain,
    /// The operator's initialization floor: every consensus request below
    /// this height is refused as stale. Set only at initialization.
    min_height: Option<u64>,
    families: BTreeMap<String, Mark>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GuardDomain {
    pub(super) chain_id: String,
    pub(super) genesis_hash: String,
}

fn valid_domain(chain_id: &str, genesis_hash: &str) -> bool {
    !chain_id.is_empty()
        && chain_id.len() <= 256
        && !chain_id.chars().any(char::is_control)
        && genesis_hash.len() == 96
        && genesis_hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SealedState {
    state: State,
    digest: String,
}

/// Unit tests only: a test creates its own `0700` root and registers it, so
/// the ancestor walk ends there instead of judging whatever `TMPDIR` lies
/// under. Production builds have no such hook.
#[cfg(test)]
pub(super) mod test_roots {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    static ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    pub(crate) fn register(root: &Path) {
        let root = root.canonicalize().expect("test root");
        ROOTS.lock().expect("test roots").push(root);
    }
    /// True when `directory` lies in a registered root and `ancestor` is
    /// above that root; the root itself is still checked.
    pub(super) fn stops_above(directory: &Path, ancestor: &Path) -> bool {
        ROOTS.lock().expect("test roots").iter().any(|root| {
            directory.starts_with(root) && root.starts_with(ancestor) && root != ancestor
        })
    }
}

pub(super) struct SignerGuard {
    directory: PathBuf,
    _lock: File,
    state: State,
    poisoned: bool,
}

fn owned_private(metadata: &std::fs::Metadata, directory: bool) -> io::Result<()> {
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid
        || metadata.mode() & 0o077 != 0
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(refusal(
            "validator_signer_state_permissions",
            "state must be an owner-only regular file/directory owned by the signer",
        ));
    }
    Ok(())
}

impl SignerGuard {
    pub(super) fn available(&self) -> bool {
        !self.poisoned
    }
    pub(super) fn open(
        directory: &Path,
        initialize: bool,
        mut keys: Vec<String>,
        domain: Option<GuardDomain>,
        min_height: Option<u64>,
    ) -> io::Result<Self> {
        keys.sort();
        keys.dedup();
        if keys.is_empty() {
            return Err(refusal("validator_signer_state_invalid", "no signing keys"));
        }
        if min_height.is_some() && !initialize {
            return Err(refusal(
                "validator_signer_state_invalid",
                "a round floor is set only with --initialize-state",
            ));
        }
        if min_height == Some(0)
            || domain
                .as_ref()
                .is_some_and(|d| !valid_domain(&d.chain_id, &d.genesis_hash))
        {
            return Err(refusal(
                "validator_signer_state_invalid",
                "the domain needs a chain id and a 96-hex genesis hash; a floor a nonzero height",
            ));
        }
        let Some(configured) = domain.clone().or_else(|| {
            (!initialize).then(|| GuardDomain {
                chain_id: String::new(),
                genesis_hash: String::new(),
            })
        }) else {
            return Err(refusal(
                "validator_signer_state_invalid",
                "--initialize-state needs --chain-id and --genesis-hash",
            ));
        };
        if initialize {
            // An explicit first-use operation. Existing or missing restart state
            // is never silently reset. Do not keep this flag in a service unit.
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(directory)?;
        }
        owned_private(&std::fs::symlink_metadata(directory)?, true)?;
        let directory = directory.canonicalize()?;
        // Otherwise a different user could rename the guard directory through
        // a writable parent and substitute a fresh state at the configured path.
        // A root-owned sticky temporary directory protects its owned children.
        let uid = unsafe { libc::geteuid() }; // SAFETY: no preconditions.
        for parent in directory.ancestors().skip(1) {
            #[cfg(test)]
            if test_roots::stops_above(&directory, parent) {
                break;
            }
            let metadata = std::fs::metadata(parent)?;
            let safe_sticky = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
            if (metadata.uid() != uid && metadata.uid() != 0)
                || (metadata.mode() & 0o022 != 0 && !safe_sticky)
            {
                return Err(refusal(
                    "validator_signer_state_permissions",
                    "state ancestor permits another user to replace custody",
                ));
            }
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(initialize)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join("lock"))?;
        owned_private(&lock.metadata()?, false)?;
        lock.try_lock()
            .map_err(|error| refusal("validator_signer_state_locked", error))?;
        // Under the lock no writer is live: a `state.<pid>.<ns>.tmp` left by a
        // crash before its rename was never committed state.
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("state.") && name.ends_with(".tmp") {
                std::fs::remove_file(entry.path())?;
            }
        }
        let state = State {
            schema: SCHEMA.into(),
            keys,
            domain: configured,
            min_height,
            families: BTreeMap::new(),
        };
        let mut guard = Self {
            directory: directory.to_path_buf(),
            _lock: lock,
            state,
            poisoned: false,
        };
        if initialize {
            guard.persist(&guard.state)?;
            File::open(directory.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()?;
        } else {
            let file = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(directory.join("state.json"))?;
            owned_private(&file.metadata()?, false)?;
            let mut bytes = Vec::new();
            file.take(MAX_STATE_BYTES + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_STATE_BYTES {
                return Err(refusal("validator_signer_state_invalid", "state too large"));
            }
            let sealed: SealedState = serde_json::from_slice(&bytes)
                .map_err(|e| refusal("validator_signer_state_invalid", e))?;
            let encoded = serde_json::to_vec(&sealed.state).map_err(malformed)?;
            if sealed.digest != hash_hex(SCHEMA, &encoded)
                || sealed.state.schema != SCHEMA
                || sealed.state.keys != guard.state.keys
                || !valid_domain(
                    &sealed.state.domain.chain_id,
                    &sealed.state.domain.genesis_hash,
                )
                || sealed.state.min_height == Some(0)
                || sealed.state.families.len() > MAX_FAMILIES
                || sealed.state.families.iter().any(|(family, mark)| {
                    family.len() != 96 || mark.message.len() != 96 || mark.height == 0
                })
            {
                return Err(refusal(
                    "validator_signer_state_invalid",
                    "checksum, key set or state shape differs",
                ));
            }
            if domain.is_some_and(|domain| domain != sealed.state.domain) {
                return Err(refusal(
                    "validator_signer_state_domain_mismatch",
                    "the guard state belongs to another chain id or genesis hash",
                ));
            }
            guard.state = sealed.state;
        }
        Ok(guard)
    }

    fn persist(&self, state: &State) -> io::Result<()> {
        let encoded = serde_json::to_vec(state).map_err(malformed)?;
        let bytes = serde_json::to_vec(&SealedState {
            state: state.clone(),
            digest: hash_hex(SCHEMA, &encoded),
        })
        .map_err(malformed)?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err(refusal("validator_signer_state_invalid", "state too large"));
        }
        // The lock serializes writers. A stale temporary from a crash is never
        // read as state, nor does it replace the last committed state on open.
        let temporary = self.directory.join(format!(
            "state.{}.{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(malformed)?
                .as_nanos()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, self.directory.join("state.json"))?;
        File::open(&self.directory)?.sync_all()
    }

    pub(super) fn reserve(
        &mut self,
        public: &ValidatorPublicKey,
        context: &[u8],
        message: &[u8],
    ) -> io::Result<()> {
        if self.poisoned {
            return Err(refusal(
                "validator_signer_state_unavailable",
                "a persistence failure requires operator inspection and restart",
            ));
        }
        // Transport authentication has no consensus slot and cannot be used as
        // a vote/proposal signature under another ML-DSA context.
        if context == TRANSPORT_AUTH_CONTEXT {
            return Ok(());
        }
        let slot = decode_slot(public, context, message)?;
        if slot.chain_id != self.state.domain.chain_id
            || slot.genesis != self.state.domain.genesis_hash
        {
            return Err(refusal(
                "validator_signer_domain_refused",
                "request names a chain id or genesis hash this guard does not sign for",
            ));
        }
        if self
            .state
            .min_height
            .is_some_and(|floor| slot.height < floor)
        {
            return Err(refusal(
                "validator_signer_stale_round",
                "request is below the initialized round floor",
            ));
        }
        // Committee changes, key aliases and protocol-version changes cannot
        // create a second signing family for the same key, chain and phase.
        let public_bytes = hex_to_bytes(&public.public_key_hex).map_err(malformed)?;
        let family = hash_hex(
            "postfiat.validator-signer.family.v1",
            &serde_json::to_vec(&(
                &public_bytes,
                context,
                &slot.chain_id,
                &slot.genesis,
                &slot.phase,
            ))
            .map_err(malformed)?,
        );
        let mark = Mark {
            height: slot.height,
            view: slot.view,
            message: hash_hex("postfiat.validator-signer.message.v1", message),
        };
        if let Some(old) = self.state.families.get(&family) {
            if (mark.height, mark.view) < (old.height, old.view) {
                return Err(refusal(
                    "validator_signer_stale_round",
                    "request is below the durable round floor",
                ));
            }
            if (mark.height, mark.view) == (old.height, old.view) {
                return if mark.message == old.message {
                    Ok(())
                } else {
                    Err(refusal(
                        "validator_signer_double_sign",
                        "different message for the same durable signing slot",
                    ))
                };
            }
        } else if self.state.families.len() >= MAX_FAMILIES {
            return Err(refusal(
                "validator_signer_state_full",
                "signing family limit reached; no history is discarded",
            ));
        }
        let mut next = self.state.clone();
        next.families.insert(family, mark);
        if let Err(error) = self.persist(&next) {
            self.poisoned = true;
            return Err(refusal("validator_signer_state_unavailable", error));
        }
        self.state = next;
        Ok(())
    }
}

struct Slot {
    chain_id: String,
    genesis: String,
    height: u64,
    view: u64,
    phase: String,
}

/// Strict parser for ordering_fast's length-delimited signing encoding. Labels
/// inside a string are data; nested QC round fields cannot overwrite the vote.
struct Fields<'a>(&'a [u8]);
impl Fields<'_> {
    fn prefix(&mut self, label: &str) -> io::Result<()> {
        self.0 = self
            .0
            .strip_prefix(format!("{label}=").as_bytes())
            .ok_or_else(|| malformed(format!("expected {label}")))?;
        Ok(())
    }
    fn number(&mut self, delimiter: u8) -> io::Result<u64> {
        let end = self
            .0
            .iter()
            .position(|b| *b == delimiter)
            .ok_or_else(|| malformed("missing numeric delimiter"))?;
        let raw = &self.0[..end];
        if raw.is_empty()
            || !raw.iter().all(u8::is_ascii_digit)
            || (raw.len() > 1 && raw[0] == b'0')
        {
            return Err(malformed("noncanonical integer"));
        }
        let value = std::str::from_utf8(raw)
            .map_err(malformed)?
            .parse()
            .map_err(malformed)?;
        self.0 = &self.0[end + 1..];
        Ok(value)
    }
    fn num(&mut self, label: &str) -> io::Result<u64> {
        self.prefix(label)?;
        self.number(b'\n')
    }
    fn text(&mut self, label: &str) -> io::Result<String> {
        self.prefix(label)?;
        let length = usize::try_from(self.number(b':')?).map_err(malformed)?;
        let raw = self
            .0
            .get(..length)
            .ok_or_else(|| malformed("truncated string"))?;
        let result = std::str::from_utf8(raw).map_err(malformed)?.to_string();
        self.0 = self
            .0
            .get(length..)
            .and_then(|rest| rest.strip_prefix(b"\n"))
            .ok_or_else(|| malformed("missing string terminator"))?;
        Ok(result)
    }
    fn present(&mut self, label: &str) -> io::Result<bool> {
        match self.num(label)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(malformed("invalid presence flag")),
        }
    }
    fn round(&mut self) -> io::Result<ConsensusV2Round> {
        Ok(ConsensusV2Round {
            height: self.num("round.height")?,
            view: self.num("round.view")?,
        })
    }
    fn phase(&mut self) -> io::Result<ConsensusV2Phase> {
        match self.text("phase")?.as_str() {
            "prepare" => Ok(ConsensusV2Phase::Prepare),
            "precommit" => Ok(ConsensusV2Phase::Precommit),
            _ => Err(malformed("invalid phase")),
        }
    }
    fn optional_root(&mut self, name: &str) -> io::Result<Option<String>> {
        if self.0.starts_with(format!("{name}=").as_bytes()) {
            self.text(name).map(Some)
        } else {
            Ok(None)
        }
    }
    fn block(&mut self) -> io::Result<ConsensusV2BlockRef> {
        Ok(ConsensusV2BlockRef {
            height: self.num("block.height")?,
            block_id: self.text("block.block_id")?,
            parent_block_id: self.text("block.parent_block_id")?,
            payload_hash: self.text("block.payload_hash")?,
            state_root: self.text("block.state_root")?,
            bridge_exit_root: self.optional_root("block.bridge_exit_root")?,
            pftl_uniswap_receipt_root: self.optional_root("block.pftl_uniswap_receipt_root")?,
        })
    }
    fn qc(&mut self) -> io::Result<Option<ConsensusV2QcRef>> {
        if !self.present("qc.present")? {
            return Ok(None);
        }
        Ok(Some(ConsensusV2QcRef {
            certificate_id: self.text("qc.certificate_id")?,
            round: self.round()?,
            phase: self.phase()?,
            block: self.block()?,
        }))
    }
}

fn decode_slot(public: &ValidatorPublicKey, context: &[u8], message: &[u8]) -> io::Result<Slot> {
    if [
        CONSENSUS_V2_PROPOSAL_CONTEXT,
        CONSENSUS_V2_VOTE_CONTEXT,
        CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT,
    ]
    .contains(&context)
    {
        let mut f = Fields(message);
        let schema = f.text("schema")?;
        let domain = ConsensusV2Domain {
            schema: f.text("domain.schema")?,
            chain_id: f.text("domain.chain_id")?,
            genesis_hash: f.text("domain.genesis_hash")?,
            protocol_version: u32::try_from(f.num("domain.protocol_version")?)
                .map_err(malformed)?,
            committee_epoch: f.num("domain.committee_epoch")?,
            committee_root: f.text("domain.committee_root")?,
        };
        let round = f.round()?;
        let mut slot = Slot {
            chain_id: domain.chain_id.clone(),
            genesis: domain.genesis_hash.clone(),
            height: round.height,
            view: round.view,
            phase: "proposal".into(),
        };
        // Signature fields are absent from the signed encoding. The public key
        // and validator id come from held custody, never from caller metadata.
        let signature = ConsensusV2Signature {
            algorithm_id: ML_DSA_65_ALGORITHM.into(),
            signer: public.node_id.clone(),
            public_key_hex: public.public_key_hex.clone(),
            signature_hex: "00".into(),
        };
        let (encoded, validator) = if context == CONSENSUS_V2_PROPOSAL_CONTEXT {
            let block = f.block()?;
            let valid_qc = f.qc()?;
            let timeout_certificate_id = if f.present("timeout_certificate_id.present")? {
                Some(f.text("timeout_certificate_id")?)
            } else {
                None
            };
            let proposer = f.text("proposer")?;
            let value = ConsensusV2Proposal {
                schema,
                domain,
                round,
                block,
                valid_qc,
                timeout_certificate_id,
                proposer: proposer.clone(),
                signature,
            };
            (
                consensus_v2_proposal_signing_bytes(&value).map_err(malformed)?,
                proposer,
            )
        } else {
            let phase = f.phase()?;
            slot.phase = match phase {
                ConsensusV2Phase::Prepare => "prepare",
                ConsensusV2Phase::Precommit => "precommit",
            }
            .into();
            if context == CONSENSUS_V2_VOTE_CONTEXT {
                let block = if f.present("block.present")? {
                    Some(f.block()?)
                } else {
                    None
                };
                let validator = f.text("validator")?;
                let value = ConsensusV2Vote {
                    schema,
                    domain,
                    round,
                    phase,
                    block,
                    validator: validator.clone(),
                    signature,
                };
                (
                    consensus_v2_vote_signing_bytes(&value).map_err(malformed)?,
                    validator,
                )
            } else {
                let high_qc = f.qc()?;
                let validator = f.text("validator")?;
                let value = ConsensusV2TimeoutVote {
                    schema,
                    domain,
                    round,
                    phase,
                    high_qc,
                    validator: validator.clone(),
                    signature,
                };
                (
                    consensus_v2_timeout_vote_signing_bytes(&value).map_err(malformed)?,
                    validator,
                )
            }
        };
        if !f.0.is_empty() || encoded != message || validator != public.node_id {
            return Err(malformed(
                "noncanonical message or signer identity mismatch",
            ));
        }
        return Ok(slot);
    }
    legacy_slot(public, context, message)
}

fn legacy_slot(public: &ValidatorPublicKey, context: &[u8], message: &[u8]) -> io::Result<Slot> {
    let value: serde_json::Value = serde_json::from_slice(message).map_err(malformed)?;
    let a = value
        .as_array()
        .ok_or_else(|| malformed("expected legacy signing tuple"))?;
    let string = |a: &[serde_json::Value], i: usize| {
        a.get(i)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| malformed("invalid tuple string"))
    };
    let number = |a: &[serde_json::Value], i: usize| {
        a.get(i)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| malformed("invalid tuple integer"))
    };
    let (domain, offset, height_index, signer) = if context == BLOCK_CERTIFICATE_SIGNATURE_CONTEXT {
        match (a.len(), a.first().and_then(|v| v.as_str())) {
            (
                3,
                Some(
                    "postfiat.block_certificate_vote.message.v3"
                    | "postfiat.block_certificate_vote.message.v4",
                ),
            ) => {
                let d = a[1]
                    .as_array()
                    .ok_or_else(|| malformed("invalid vote domain"))?;
                let tail = a[2]
                    .as_array()
                    .ok_or_else(|| malformed("invalid vote tail"))?;
                let expected = if a[0] == "postfiat.block_certificate_vote.message.v4" {
                    13
                } else {
                    12
                };
                if d.len() != expected || tail.len() != 4 || !tail[3].is_boolean() {
                    return Err(malformed("invalid vote tuple shape"));
                }
                (d.as_slice(), 0, 4, string(tail, 2)?)
            }
            (16, Some("postfiat.block_certificate_vote.message.v2")) => {
                (a.as_slice(), 1, 5, string(a, 14)?)
            }
            (13, _) => (a.as_slice(), 0, 3, string(a, 11)?),
            (14, _) => (a.as_slice(), 0, 4, string(a, 12)?),
            _ => return Err(malformed("unsupported block vote tuple")),
        }
    } else if context == BLOCK_TIMEOUT_SIGNATURE_CONTEXT {
        match a.len() {
            7 => (a.as_slice(), 0, 3, string(a, 6)?),
            8 => (a.as_slice(), 0, 4, string(a, 7)?),
            _ => return Err(malformed("unsupported timeout tuple")),
        }
    } else if context == BLOCK_PROPOSAL_SIGNATURE_CONTEXT {
        match (a.len(), a.first().and_then(|v| v.as_str())) {
            (14, Some("postfiat.block_proposal.signature-message.v2"))
            | (15, Some("postfiat.block_proposal.signature-message.v3"))
            | (16, Some("postfiat.block_proposal.signature-message.v4")) => {
                (a.as_slice(), 1, 4, string(a, 7)?)
            }
            (12, _) => (a.as_slice(), 0, 3, string(a, 6)?),
            _ => return Err(malformed("unsupported proposal tuple")),
        }
    } else {
        return Err(malformed("unsupported signing context"));
    };
    let chain_id = string(domain, offset)?;
    let genesis = string(domain, offset + 1)?;
    let protocol = number(domain, offset + 2)?;
    let height = number(domain, height_index)?;
    let view = number(domain, height_index + 1)?;
    if signer != public.node_id
        || !valid_domain(&chain_id, &genesis)
        || protocol == 0
        || protocol > u32::MAX as u64
        || height == 0
    {
        return Err(malformed("invalid chain, height or validator identity"));
    }
    Ok(Slot {
        chain_id,
        genesis,
        height,
        view,
        phase: "legacy".into(),
    })
}

#[cfg(test)]
mod tests;
