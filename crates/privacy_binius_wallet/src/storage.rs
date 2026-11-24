//! Files are sealed under a user-held, independent 256-bit wrapping key.
use crate::Result;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};
pub fn private_file(path: &Path) -> Result<File> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let m = f.metadata()?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.nlink() != 1
    {
        return Err("expected an owner-only regular file".into());
    }
    Ok(f)
}
pub fn private_dir(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err("expected an owner-only wallet directory".into());
    }
    Ok(())
}
pub struct Store {
    cipher: Aes256Gcm,
}
impl Store {
    pub fn new(path: &Path, home: &Path) -> Result<Self> {
        private_dir(home)?;
        if path.canonicalize()?.starts_with(home.canonicalize()?) {
            return Err("keep the wrapping key outside the wallet directory".into());
        }
        let mut key = [0; 32];
        let mut f = private_file(path)?;
        if f.metadata()?.len() != 32 {
            return Err("wrapping key must be 32 random bytes".into());
        }
        f.read_exact(&mut key)?;
        Ok(Self {
            cipher: Aes256Gcm::new((&key).into()),
        })
    }
    pub fn load<T: DeserializeOwned>(&self, path: &Path, tag: &[u8]) -> Result<T> {
        let mut bytes = Vec::new();
        private_file(path)?.read_to_end(&mut bytes)?;
        let nonce = bytes.get(..12).ok_or("invalid wallet file")?;
        let pt = self
            .cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: &bytes[12..],
                    aad: tag,
                },
            )
            .map_err(|_| "wallet authentication failed")?;
        serde_json::from_slice(&pt).map_err(|_| "invalid wallet record".into())
    }
    pub fn save<T: Serialize>(
        &self,
        path: &Path,
        tag: &[u8],
        record: &T,
        create: bool,
    ) -> Result<()> {
        let mut nonce = [0; 12];
        OsRng.fill_bytes(&mut nonce);
        let bytes = self
            .cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &serde_json::to_vec(record)?,
                    aad: tag,
                },
            )
            .map_err(|_| "wallet encryption failed")?;
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().as_bytes().is_empty())
            .ok_or("absolute path required")?;
        let mut f = tempfile::NamedTempFile::new_in(dir)?;
        f.write_all(&nonce)?;
        f.write_all(&bytes)?;
        f.as_file().sync_all()?;
        if create {
            f.persist_noclobber(path)?;
        } else {
            f.persist(path)?;
        }
        File::open(dir)?.sync_all()?;
        Ok(())
    }
}
