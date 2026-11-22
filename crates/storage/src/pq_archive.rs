//! Portable snapshot representation and the legacy store's immutable body files.
use super::*;
use postfiat_types::pq_payment::{PqPayment, ARCHIVE_LIMIT};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn invalid(e: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

fn body_path(dir: &Path, id: &str) -> PathBuf {
    dir.join("pq_bodies_v1").join(id)
}

fn read_body(dir: &Path, id: &str) -> io::Result<PqPayment> {
    let mut bytes = Vec::new();
    File::open(body_path(dir, id))?
        .take(ARCHIVE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    let payment = PqPayment::from_bytes(bytes).map_err(invalid)?;
    if payment.digest() != id {
        return Err(invalid("PQ archive digest mismatch"));
    }
    Ok(payment)
}

fn persist_body(dir: &Path, id: &str, payment: &PqPayment) -> io::Result<()> {
    let path = body_path(dir, id);
    match read_body(dir, id) {
        Ok(existing) if existing == *payment => return Ok(()),
        Ok(_) => return Err(invalid("conflicting PQ binary body")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    sync_parent_dir(parent)?;
    atomic_write(path, payment.bytes())
}

impl NodeStore {
    pub(crate) fn persist_pq_archive(
        &self,
        entry: &BatchArchiveEntry,
    ) -> io::Result<BatchArchiveEntry> {
        self.ensure_writable()?;
        // Allocate the maximum JSONL envelope (both 48-byte MACs as hex)
        // plus its newline and the binary file's content-address key.
        let framing = if BatchArchiveEntry::may_contain_pq(&entry.batch_kind, &entry.payload_json) {
            let mac = "0".repeat(MAC_BYTES * 2);
            let envelope = serde_json::json!({"pftmac":JSONL_ENVELOPE_KIND,"chain":mac,"record":null,"mac":mac});
            serde_json::to_vec(&envelope).map_err(invalid)?.len() - 4 + 1 + 96
        } else {
            0
        };
        let (compact, bodies) = entry.compact_pq_with_framing(framing).map_err(invalid)?;
        for (id, payment) in bodies {
            persist_body(&self.data_dir, &id, &payment)?;
        }
        Ok(compact)
    }

    pub(crate) fn hydrate_pq_archive(&self, entry: &mut BatchArchiveEntry) -> io::Result<()> {
        entry
            .expand_pq(|id| read_body(&self.data_dir, id).map_err(|e| e.to_string()))
            .map_err(invalid)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBatchArchive {
    batches: Vec<BatchArchiveEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pq_bodies_v1: BTreeMap<String, PqPayment>,
}

impl SnapshotBatchArchive {
    pub fn from_archive(archive: BatchArchive) -> io::Result<Self> {
        let mut result = Self {
            batches: Vec::new(),
            pq_bodies_v1: BTreeMap::new(),
        };
        for entry in archive.batches {
            let (compact, bodies) = entry.compact_pq().map_err(invalid)?;
            result.batches.push(compact);
            result.pq_bodies_v1.extend(bodies);
        }
        Ok(result)
    }

    pub fn stage(self, dir: &Path) -> io::Result<BatchArchive> {
        let mut used = BTreeSet::new();
        for entry in &self.batches {
            let mut checked = entry.clone();
            checked
                .expand_pq(|id| {
                    let body = self
                        .pq_bodies_v1
                        .get(id)
                        .ok_or("missing PQ snapshot body")?;
                    used.insert(id.to_owned());
                    Ok(body.clone())
                })
                .map_err(invalid)?;
            if checked.compact_pq().map_err(invalid)?.0 != *entry {
                return Err(invalid("noncanonical snapshot reference"));
            }
        }
        if used.len() != self.pq_bodies_v1.len() {
            return Err(invalid("unreferenced PQ snapshot body"));
        }
        for (id, payment) in self.pq_bodies_v1 {
            persist_body(dir, &id, &payment)?;
        }
        Ok(BatchArchive {
            batches: self.batches,
        })
    }
}
