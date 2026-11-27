//! A certified shielded head names a backwards chain of immutable pair pages.
//! Publish pages before a journal or redb transaction can expose the new head.
//! An interrupted prepare leaves at most unreachable content-addressed pages.
use super::*;
use postfiat_types::pq_payment::{PqHead, PqPair, PqPool};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};

const DIRECTORY: &str = "pq_pool_pages_v1";
const MAX_PAGE: u64 = 8192;
fn bad(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn page_path(dir: &Path, id: &str) -> io::Result<PathBuf> {
    if id.len() != 96
        || !id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(bad("invalid PQ page id"));
    }
    Ok(dir.join(DIRECTORY).join(&id[..2]).join(id))
}

pub fn read_page(dir: &Path, id: &str) -> io::Result<PqPair> {
    let mut bytes = Vec::new();
    let path = page_path(dir, id)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > MAX_PAGE {
        return Err(bad("invalid PQ page file"));
    }
    File::open(path)?
        .take(MAX_PAGE + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PAGE {
        return Err(bad("oversize PQ page"));
    }
    let page: PqPair = serde_json::from_slice(&bytes).map_err(bad)?;
    if page.digest() != id {
        return Err(bad("PQ page digest mismatch"));
    }
    Ok(page)
}

fn persist(dir: &Path, page: &PqPair) -> io::Result<()> {
    let id = page.digest();
    let path = page_path(dir, &id)?;
    match read_page(dir, &id) {
        Ok(_) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e),
    }
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    sync_parent_dir(parent.parent().unwrap())?;
    sync_parent_dir(parent)?;
    atomic_write(path, serde_json::to_vec(page).map_err(bad)?)
}

pub fn persist_pending(dir: &Path, state: &ShieldedState) -> io::Result<()> {
    if let Some(pool) = &state.pq_pool_v1 {
        for pair in &pool.pending {
            persist(dir, pair)?;
        }
        // A journal read after a crash has only the head. Its pages must
        // already be durable, including for idempotent commit retries.
        read_page(dir, &pool.head.last_page)?;
    }
    Ok(())
}

type Views = VecDeque<(PathBuf, PqPool)>;
fn views() -> &'static Mutex<Views> {
    static VIEWS: OnceLock<Mutex<Views>> = OnceLock::new();
    VIEWS.get_or_init(Default::default)
}

// Untrusted page counts never size an allocation. Each iteration consumes one
// bounded, digest-checked page. An absent/truncated chain fails before publish.
fn rebuild(dir: &Path, head: &PqHead, cached: bool, copy_to: Option<&Path>) -> io::Result<PqPool> {
    if head.pairs == 0 {
        return Err(bad("present PQ pool has no accepted pair"));
    }
    let mut ids = Vec::new();
    let mut id = head.last_page.clone();
    let mut remaining = head.pairs;
    let mut view = PqPool::default();
    while remaining != 0 {
        let pair = read_page(dir, &id)?;
        if let Some(dest) = copy_to {
            persist(dest, &pair)?;
        }
        if cached {
            let cache = views().lock().map_err(|_| bad("PQ view lock"))?;
            if let Some((_, base)) = cache.iter().find(|(path, v)| {
                path == dir && v.head.pairs == remaining && v.head.last_page == id
            }) {
                view = base.clone();
                break;
            }
        }
        ids.push(id);
        id = pair.previous;
        remaining -= 1;
    }
    if remaining == 0 && !id.is_empty() {
        return Err(bad("PQ page count mismatch"));
    }
    for id in ids.into_iter().rev() {
        view = view.append_pair(read_page(dir, &id)?).map_err(bad)?;
        view.pending.clear();
    }
    if view.head != *head {
        return Err(bad("rebuilt PQ indexes disagree with committed head"));
    }
    if cached {
        let mut cache = views().lock().map_err(|_| bad("PQ view lock"))?;
        cache.retain(|(path, existing)| {
            path != dir || existing.head.last_page != view.head.last_page
        });
        cache.push_front((dir.to_owned(), view.clone()));
        cache.truncate(8);
    }
    Ok(view)
}

pub fn hydrate(dir: &Path, state: &mut ShieldedState) -> io::Result<()> {
    if let Some(pool) = &mut state.pq_pool_v1 {
        *pool = rebuild(dir, &pool.head, true, None)?;
    }
    Ok(())
}

/// Cold validation used at recovery boundaries. A portable snapshot carries
/// the same immutable pages, whose root is in its certified shielded state.
pub fn audit(dir: &Path, state: &ShieldedState, copy_to: Option<&Path>) -> io::Result<()> {
    if let Some(pool) = &state.pq_pool_v1 {
        rebuild(dir, &pool.head, false, copy_to)?;
    }
    Ok(())
}
