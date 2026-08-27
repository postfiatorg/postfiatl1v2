//! Node-local keyed integrity for on-disk state (S1/S2 hardening).
//!
//! ## On-disk format (ADR-style note, 2026-07-25)
//!
//! State durability previously relied on plain `serde_json` payloads and
//! *unkeyed* `Sha3_384` "checksums" over a public domain string. Any process
//! with data-dir write access could recompute those checksums after
//! tampering, so corruption was not detectable. The format is now keyed:
//!
//! * A node-local 48-byte secret lives at `<data_dir>/.integrity.key`
//!   (`0600`, created from `/dev/urandom` on first use). Key files that are
//!   group/world-accessible or multiply-linked are rejected.
//! * Whole-file state (`*.json`) is stored as
//!   `<json>\npftmac1:<hex hmac-sha3-384(domain || 0x00 || json)>\n`.
//! * Append-only JSONL records are stored one JSON envelope per line:
//!   `{"pftmac":"v1","chain":<hex mac of previous record or "genesis">,
//!   "record":<payload>,"mac":<hex hmac(domain || 0x00 || chain || 0x00 ||
//!   canonical payload)>}`. Each record MAC chains over the previous
//!   record's MAC, so truncation/reorder/rewrite of history is detected.
//! * FastSwap WAL records keep their `[u32 len][payload][48-byte tag]`
//!   framing, but the tag is now the keyed MAC instead of the unkeyed hash.
//! * FastSwap snapshots gain a `"mac"` field beside the legacy `"checksum"`.
//!
//! ## Migration
//!
//! Normal opens reject legacy (unkeyed / untagged) files. Operators must use
//! the explicit one-shot migration constructors against an offline, verified
//! copy; successful migration rewrites every accepted object in keyed form.
//!
//! The default key lives in the data directory for compatibility. That detects
//! accidental corruption and tampering by an identity that cannot read the key,
//! but it cannot detect replacement by an attacker who can replace both the
//! state and that key. Production operators can anchor the key outside the data
//! directory with `load_or_create_at`, on separately protected storage.

use sha3::{Digest, Sha3_384};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

/// Keyed-MAC output size (SHA3-384).
pub const MAC_BYTES: usize = 48;
/// Trailer marker appended after the JSON body of whole-file state.
pub const FILE_MAC_MARKER: &str = "pftmac1:";
/// Sentinel `chain` value for the first record of a keyed JSONL file.
pub const JSONL_CHAIN_GENESIS: &str = "genesis";
/// JSONL envelope schema tag.
pub const JSONL_ENVELOPE_KIND: &str = "v1";

const INTEGRITY_KEY_FILE: &str = ".integrity.key";
// HMAC block size for SHA3-384. Python's `hmac` uses the digest constructor's
// `block_size` attribute (104 bytes for sha3_384: the Keccak rate minus the
// capacity-encoding suffix byte convention), and we match it so tags are
// interoperable with the reference implementation.
const HMAC_BLOCK_BYTES: usize = 104;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Node-local secret used to key every on-disk integrity tag.
#[derive(Debug, Clone)]
pub struct IntegrityKey {
    bytes: [u8; MAC_BYTES],
}

impl IntegrityKey {
    /// Load the node-local integrity key, creating it with `0600` perms on
    /// first use. Refuses keys that are group/world-accessible.
    pub fn load_or_create(data_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(data_dir)?;
        Self::load_or_create_at(&data_dir.join(INTEGRITY_KEY_FILE))
    }

    /// Load an existing node-local integrity key without creating a directory
    /// or key file. Offline verification uses this path so a missing trust
    /// anchor fails closed without mutating the inspected directory.
    pub fn load_existing(data_dir: &Path) -> io::Result<Self> {
        Self::load_existing_at(&data_dir.join(INTEGRITY_KEY_FILE))
    }

    /// Load an existing key from an operator-selected path without creating
    /// any parent directory or file.
    pub fn load_existing_at(path: &Path) -> io::Result<Self> {
        read_key_file(path)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "storage_integrity_key_missing: `{}` does not exist",
                    path.display()
                ),
            )
        })
    }

    /// Load or create an integrity key at an operator-selected path. Placing
    /// this path outside the state directory establishes an independent trust
    /// anchor against whole-directory replacement.
    pub fn load_or_create_at(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        match read_key_file(path)? {
            Some(key) => Ok(key),
            None => create_key_file(path),
        }
    }

    /// Keyed HMAC-SHA3-384 over `domain || 0x00 || payload`.
    pub fn mac(&self, domain: &[u8], payload: &[u8]) -> [u8; MAC_BYTES] {
        hmac_sha3_384(&self.bytes, domain, payload)
    }
}

/// Standard HMAC (RFC 2104) over SHA3-384, implemented inline so the storage
/// crate stays dependency-free beyond the crypto crates already in the
/// workspace. `tag = H(K ^ opad || H(K ^ ipad || domain || 0x00 || payload))`.
pub fn hmac_sha3_384(key: &[u8; MAC_BYTES], domain: &[u8], payload: &[u8]) -> [u8; MAC_BYTES] {
    const { assert!(MAC_BYTES <= HMAC_BLOCK_BYTES) };
    let mut pad = [0_u8; HMAC_BLOCK_BYTES];
    pad[..MAC_BYTES].copy_from_slice(key);

    let mut inner = Sha3_384::new();
    for byte in pad.iter_mut() {
        *byte ^= 0x36;
    }
    inner.update(pad);
    inner.update(domain);
    inner.update([0_u8]);
    inner.update(payload);
    let inner_digest: [u8; MAC_BYTES] = inner.finalize().into();

    let mut outer = Sha3_384::new();
    for byte in pad.iter_mut() {
        *byte ^= 0x36 ^ 0x5c;
    }
    outer.update(pad);
    outer.update(inner_digest);
    outer.finalize().into()
}

/// Legacy unkeyed `Sha3_384(domain || 0x00 || payload)` checksum, retained
/// only to verify pre-MAC files during the migration window.
pub fn legacy_checksum(domain: &[u8], payload: &[u8]) -> [u8; MAC_BYTES] {
    let mut hasher = Sha3_384::new();
    hasher.update(domain);
    hasher.update([0_u8]);
    hasher.update(payload);
    hasher.finalize().into()
}

pub fn to_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(HEX_DIGITS[(byte >> 4) as usize] as char);
        output.push(HEX_DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

pub fn from_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Some(bytes)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn macs_equal(expected: &[u8; MAC_BYTES], actual: &[u8]) -> bool {
    if actual.len() != MAC_BYTES {
        return false;
    }
    let mut diff = 0_u8;
    for (left, right) in expected.iter().zip(actual.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

fn read_key_file(path: &Path) -> io::Result<Option<IntegrityKey>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    #[cfg(unix)]
    {
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("integrity key `{}` is not a regular file", path.display()),
            ));
        }
        if metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "integrity key `{}` must not be group/world-accessible (mode {:o}); \
                     run `chmod 600` after verifying it was not copied",
                    path.display(),
                    metadata.mode() & 0o777
                ),
            ));
        }
        if metadata.nlink() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "integrity key `{}` has multiple hard links; refusing to trust it",
                    path.display()
                ),
            ));
        }
    }
    let mut bytes = Vec::with_capacity(MAC_BYTES);
    file.read_to_end(&mut bytes)?;
    let bytes: [u8; MAC_BYTES] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "integrity key `{}` is {} bytes; expected {MAC_BYTES}",
                path.display(),
                bytes.len()
            ),
        )
    })?;
    Ok(Some(IntegrityKey { bytes }))
}

fn create_key_file(path: &Path) -> io::Result<IntegrityKey> {
    let bytes = random_key_bytes()?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return read_key_file(path)?.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "integrity key raced with creation but is unavailable",
                )
            });
        }
        Err(error) => return Err(error),
    };
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(IntegrityKey { bytes })
}

fn random_key_bytes() -> io::Result<[u8; MAC_BYTES]> {
    let mut bytes = [0_u8; MAC_BYTES];
    let mut source = fs::File::open("/dev/urandom")?;
    source.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_is_key_sensitive_and_domain_sensitive() {
        let key = [0xaa_u8; MAC_BYTES];
        let other_key = [0xbb_u8; MAC_BYTES];
        let tag = hmac_sha3_384(&key, b"domain", b"payload");
        assert!(macs_equal(
            &tag,
            &hmac_sha3_384(&key, b"domain", b"payload")
        ));
        assert!(!macs_equal(&tag, &hmac_sha3_384(&key, b"domain", b"other")));
        assert!(!macs_equal(
            &tag,
            &hmac_sha3_384(&key, b"other", b"payload")
        ));
        assert!(!macs_equal(
            &tag,
            &hmac_sha3_384(&other_key, b"domain", b"payload")
        ));
        // Distinct from the legacy unkeyed checksum.
        assert!(!macs_equal(&tag, &legacy_checksum(b"domain", b"payload")));
    }

    #[test]
    fn hmac_matches_python_sha3_384_vector() {
        // Python: hmac.new(b"\xaa"*20, b"\x00" + b"\xdd"*50, hashlib.sha3_384)
        // (the leading 0x00 is our domain separator with an empty domain);
        // (keys shorter than the 104-byte block are zero-padded).
        let mut key = [0_u8; MAC_BYTES];
        key[..20].fill(0xaa);
        let tag = hmac_sha3_384(&key, b"", &[0xdd_u8; 50]);
        assert_eq!(
            to_hex(&tag),
            "8518186759eca8b420bae6c8448a181351142a4c0d8206b9cefa3fbed872a587\
             a203e2dfdc9d50d300a5675d16a0b0c6"
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        );
    }

    #[test]
    fn hex_roundtrips() {
        let bytes = [0x00_u8, 0x0f, 0xf0, 0xff];
        let hex = to_hex(&bytes);
        assert_eq!(hex, "000ff0ff");
        assert_eq!(from_hex(&hex).expect("decode"), bytes);
        assert!(from_hex("0g").is_none());
        assert!(from_hex("abc").is_none());
    }

    #[test]
    fn key_is_created_then_reloaded() {
        let dir = std::env::temp_dir().join(format!(
            "postfiat-integrity-key-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let key = IntegrityKey::load_or_create(&dir).expect("create");
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(dir.join(INTEGRITY_KEY_FILE))
                .expect("key file")
                .mode()
                & 0o777,
            0o600
        );
        let reloaded = IntegrityKey::load_or_create(&dir).expect("reload");
        assert_eq!(
            key.mac(b"domain", b"payload"),
            reloaded.mac(b"domain", b"payload")
        );
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn world_readable_key_is_rejected() {
        let dir = std::env::temp_dir().join(format!(
            "postfiat-integrity-perm-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("dir");
        let key_path = dir.join(INTEGRITY_KEY_FILE);
        fs::write(&key_path, [7_u8; MAC_BYTES]).expect("key");
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644)).expect("chmod");
        let error = IntegrityKey::load_or_create(&dir).expect_err("must reject");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        fs::remove_dir_all(dir).expect("cleanup");
    }
}
