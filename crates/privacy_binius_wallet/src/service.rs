use crate::{
    storage::{private_file, Store},
    wallet::{Point, Recovery, Wallet},
    Result,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
const STATE: &[u8] = b"postfiat/pq-wallet/v1/state";
const BACKUP: &[u8] = b"postfiat/pq-wallet/v1/recovery";

pub fn execute(home: &Path, store: &Store, cmd: Value) -> Result<Value> {
    let text = |k: &str| cmd[k].as_str().ok_or("missing wallet argument");
    let path = home.join("wallet.enc");
    let staging_path = |k: &str, prefix: &str| -> Result<PathBuf> {
        let value = PathBuf::from(text(k)?);
        if value.parent() != Some(home)
            || !value
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix))
        {
            return Err("invalid wallet staging path".into());
        }
        Ok(value)
    };
    let point = || -> Result<Point> {
        Ok(serde_json::from_reader(private_file(&staging_path(
            "point_file",
            "point-",
        )?)?)?)
    };

    let outside = || -> Result<PathBuf> {
        let b = PathBuf::from(text("backup")?);
        let parent = b
            .parent()
            .ok_or("backup requires absolute path")?
            .canonicalize()?;
        if !b.is_absolute() || parent.starts_with(home.canonicalize()?) {
            return Err("keep recovery backups outside the wallet directory".into());
        }
        Ok(b)
    };
    match text("op")? {
        "init" | "restore" => {
            let r: Recovery = if text("op")? == "restore" {
                store.load(&outside()?, BACKUP)?
            } else {
                Recovery::new(
                    point()?,
                    serde_json::from_value(cmd["account"].clone())?,
                    serde_json::from_value(cmd["first"].clone())?,
                    serde_json::from_value(cmd["count"].clone())?,
                )?
            };
            let w = Wallet::restore(r)?;
            store.save(&path, STATE, &w, true)?;
            return Ok(w.balance());
        }
        _ => (),
    }
    let mut w: Wallet = store.load(&path, STATE)?;
    w.import_pending()?;
    let result = match text("op")? {
        "backup" => {
            store.save(&outside()?, BACKUP, &w.recovery, true)?;
            return Ok(json!({"backed_up":true}));
        }
        "address" => {
            return Ok(
                json!({"address":w.address(serde_json::from_value::<Option<u32>>(cmd["index"].clone())?.unwrap_or(w.recovery.first))?}),
            )
        }
        "balance" => return Ok(w.balance()),
        "checkpoint" => {
            return Ok(
                json!({"network":w.point.network,"height":w.point.height,"pairs":w.point.pairs,"last_page":w.point.last_page}),
            )
        }
        "prepared" => return w.prepared(cmd["action_hash"].as_str()),
        "submitted" => {
            w.submitted(text("action_hash")?)?;
            json!({"submitted":true})
        }
        "sync" => {
            let rows = staging_path("rows", "pages-")?;
            let p = point()?;
            w.sync(p, &rows)?
        }
        "prove" => {
            let mut metrics = w.prove(serde_json::from_value(cmd["order"].clone())?)?;
            let mut usage = std::mem::MaybeUninit::uninit();
            if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
                metrics["worker_peak_rss_kib"] = json!(unsafe { usage.assume_init() }.ru_maxrss);
            }
            metrics
        }
        _ => return Err("unknown wallet command".into()),
    };
    store.save(&path, STATE, &w, false)?;
    Ok(result)
}
