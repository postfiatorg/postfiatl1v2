use super::*;
use sha2::{Digest, Sha384};
use std::time::SystemTime;

// Include directories as well as files, so creation/deletion and metadata-only
// writes are detected. Hash file contents to avoid printing fixture key bytes.
#[derive(Debug, PartialEq, Eq)]
struct StorageEntry {
    directory: bool,
    size: u64,
    modified: SystemTime,
    hash: Vec<u8>,
}
fn storage_snapshot(root: &Path) -> BTreeMap<PathBuf, StorageEntry> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, StorageEntry>) {
        let metadata = fs::metadata(path).unwrap();
        let directory = metadata.is_dir();
        let hash = if directory {
            vec![]
        } else {
            Sha384::digest(fs::read(path).unwrap()).to_vec()
        };
        entries.insert(
            path.strip_prefix(root).unwrap().to_path_buf(),
            StorageEntry {
                directory,
                size: metadata.len(),
                modified: metadata.modified().unwrap(),
                hash,
            },
        );
        if directory {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

#[test]
fn b23_read_only_absent_directory_is_not_created() {
    let fixture = Fixture::new(&[true], false);
    let absent = fixture.dir.join("absent-store");
    let before = storage_snapshot(&fixture.dir);
    let result = pftl_uniswap_receipt(
        NodeOptions {
            data_dir: absent.clone(),
        },
        &fixture.tx_ids[0],
    );
    assert!(result.is_err());
    assert_eq!(storage_snapshot(&fixture.dir), before);
    assert!(!absent.exists());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn b23_read_only_existing_directory_without_key_fails_without_writes() {
    let fixture = Fixture::new(&[true], false);
    let empty = fixture.dir.join("empty-store");
    fs::create_dir(&empty).unwrap();
    let before = storage_snapshot(&fixture.dir);
    let result = pftl_uniswap_receipt(NodeOptions { data_dir: empty }, &fixture.tx_ids[0]);
    assert!(result.is_err());
    assert_eq!(storage_snapshot(&fixture.dir), before);
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn b23_read_only_missing_tip_is_reconstructed_without_persisting() {
    let fixture = Fixture::new(&[true], false);
    assert_eq!(
        NodeStore::try_new_read_only(&fixture.dir)
            .unwrap()
            .read_chain_tip()
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    let before = storage_snapshot(&fixture.dir);
    let result = pftl_uniswap_receipt(fixture.options(), &fixture.tx_ids[0]).unwrap();
    assert!(result.accepted);
    assert_eq!(storage_snapshot(&fixture.dir), before);
}

#[test]
fn b23_read_only_v1_head_is_verified_without_upgrade_on_success_or_failure() {
    let fixture = Fixture::new(&[true], false);
    let store = fixture.store();
    let block = store.read_blocks().unwrap().blocks.remove(0);
    store.write_blocks(&BlockLog::empty()).unwrap();
    store.append_block_record(&block).unwrap();
    let head_path = fixture.dir.join("blocks.append.jsonl.head");
    let current: serde_json::Value =
        serde_json::from_slice(&fs::read(&head_path).unwrap()).unwrap();
    let count = current["record_count"].as_u64().unwrap();
    let chain = current["chain"].as_str().unwrap();
    let schema = "postfiat-storage-jsonl-head-v1";
    let payload = serde_json::to_vec(&(schema, count, chain)).unwrap();
    let key = postfiat_storage::integrity::IntegrityKey::load_existing(&fixture.dir).unwrap();
    let mac = bytes_to_hex(&key.mac(b"postfiat.storage.jsonl-head.v1", &payload));
    fs::write(
        &head_path,
        serde_json::to_vec(
            &serde_json::json!({"schema":schema,"record_count":count,"chain":chain,"mac":mac}),
        )
        .unwrap(),
    )
    .unwrap();
    drop(key);
    drop(store);
    // Prove this is authenticated, usable legacy storage before exercising RPC.
    assert_eq!(
        NodeStore::try_new_read_only(&fixture.dir)
            .unwrap()
            .read_blocks()
            .unwrap()
            .blocks
            .len(),
        1
    );
    let before = storage_snapshot(&fixture.dir);
    assert!(
        pftl_uniswap_receipt(fixture.options(), &fixture.tx_ids[0])
            .unwrap()
            .accepted
    );
    assert_eq!(storage_snapshot(&fixture.dir), before);
    assert!(pftl_uniswap_receipt(fixture.options(), &"ff".repeat(48)).is_err());
    assert_eq!(storage_snapshot(&fixture.dir), before);
}
