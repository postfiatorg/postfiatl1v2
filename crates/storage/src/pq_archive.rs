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

    /// Pruned-node storage helper: deletes verified `pq_bodies_v1` proof blobs outside
    /// `keep_ids` after finality checkpointing, reclaiming ~95% of PQ disk footprint
    /// (~150 KB -> ~4 KB/tx) while leaving `pq_pool` output pages, commitments,
    /// envelopes, and global nullifier sets untouched for wallet scanning and settlement.
    pub fn prune_pq_bodies(&self, keep_ids: &BTreeSet<String>) -> io::Result<PqBodyPruneReport> {
        self.ensure_writable()?;
        let dir = self.data_dir.join("pq_bodies_v1");
        if !dir.exists() {
            return Ok(PqBodyPruneReport::default());
        }
        let mut report = PqBodyPruneReport::default();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(id) = name.to_str() else {
                continue;
            };
            if keep_ids.contains(id) {
                report.kept_count += 1;
                continue;
            }
            let meta = entry.metadata()?;
            if meta.is_file() {
                let len = meta.len();
                fs::remove_file(entry.path())?;
                report.pruned_count += 1;
                report.bytes_reclaimed += len;
            }
        }
        sync_parent_dir(&dir)?;
        Ok(report)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PqBodyPruneReport {
    pub pruned_count: usize,
    pub kept_count: usize,
    pub bytes_reclaimed: u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_pq_bodies_reclaims_verified_proof_blobs_and_preserves_kept_ids() {
        let dir = std::env::temp_dir().join(format!(
            "pq-prune-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let store = NodeStore::new(&dir);
        let p1 = PqPayment::from_bytes(
            include_bytes!("../../../tools/pft-binius-shielded-pq/l1-vectors/pfusdc-pay.bin")
                .to_vec(),
        )
        .unwrap();
        let p2 = PqPayment::from_bytes(
            include_bytes!("../../../tools/pft-binius-shielded-pq/l1-vectors/pfusdc-exit.bin")
                .to_vec(),
        )
        .unwrap();
        let id1 = p1.digest();
        let id2 = p2.digest();
        persist_body(&dir, &id1, &p1).unwrap();
        persist_body(&dir, &id2, &p2).unwrap();

        let mut keep = BTreeSet::new();
        keep.insert(id2.clone());
        let report = store.prune_pq_bodies(&keep).unwrap();
        assert_eq!(report.pruned_count, 1);
        assert_eq!(report.kept_count, 1);
        assert_eq!(report.bytes_reclaimed as usize, p1.bytes().len());
        assert!(read_body(&dir, &id1).is_err());
        assert_eq!(read_body(&dir, &id2).unwrap(), p2);
        fs::remove_dir_all(&dir).unwrap();
    }
}
