#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use postfiat_storage::transactional::TransactionalStore;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const FIXTURE: &str = env!("CARGO_BIN_EXE_transactional_crash_fixture");

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "postfiat-transactional-process-crash-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create process-crash test directory");
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_fixture(mode: &str, data_dir: &Path) {
    let status = Command::new(FIXTURE)
        .arg(mode)
        .arg(data_dir)
        .status()
        .expect("run transactional crash fixture");
    assert!(status.success(), "fixture mode {mode} failed: {status}");
}

fn wait_ready(child: &mut Child, ready: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if ready.is_file() {
            return;
        }
        if let Some(status) = child.try_wait().expect("poll crash fixture") {
            panic!("crash fixture exited before cut point: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for crash fixture cut point");
}

fn kill_at_cut(mode: &str, data_dir: &Path) {
    let ready = data_dir.join(format!("{mode}.ready"));
    let mut child = Command::new(FIXTURE)
        .arg(mode)
        .arg(data_dir)
        .arg(&ready)
        .spawn()
        .expect("spawn transactional crash fixture");
    wait_ready(&mut child, &ready);
    child.kill().expect("SIGKILL transactional crash fixture");
    let status = child.wait().expect("wait for killed crash fixture");
    assert!(!status.success(), "killed fixture unexpectedly succeeded");
}

fn assert_tip(data_dir: &Path, height: u64) {
    let store = TransactionalStore::open(data_dir).expect("reopen transactional store");
    let meta = store
        .meta()
        .expect("read transactional metadata after process kill");
    assert_eq!(meta.finalized_height, height);
    let report = store
        .verify_logical_integrity()
        .expect("verify logical integrity after process kill");
    assert_eq!(report.finalized_height, height);
}

#[test]
fn sigkill_before_during_and_after_commit_recovers_one_complete_tip() {
    let root = TestDir::new();

    let before = root.0.join("before");
    run_fixture("prepare", &before);
    kill_at_cut("before", &before);
    assert_tip(&before, 0);

    let during = root.0.join("during");
    run_fixture("prepare", &during);
    kill_at_cut("during", &during);
    assert_tip(&during, 0);
    run_fixture("commit", &during);
    assert_tip(&during, 1);
    run_fixture("commit", &during);
    assert_tip(&during, 1);

    let after = root.0.join("after");
    run_fixture("prepare", &after);
    kill_at_cut("after", &after);
    assert_tip(&after, 1);
}
