//! Local-only PFSH02 wallet service. A disposable subprocess owns each job so
//! cancellation can interrupt a CPU-bound prover without publishing half a job.
use postfiat_privacy_binius_wallet::{
    service,
    storage::{private_dir, Store},
    Result,
};
use serde_json::{json, Value};
use std::{
    env, fs,
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, OpenOptionsExt, PermissionsExt},
        io::{AsRawFd, OwnedFd},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn read_request(reader: impl Read) -> Result<Value> {
    let mut bytes = Vec::new();
    reader.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("oversize local command".into());
    }
    Ok(serde_json::from_slice(&bytes).map_err(|_| "invalid local command")?)
}
fn send(mut socket: impl Write, result: Value) -> Result<()> {
    serde_json::to_writer(&mut socket, &result)?;
    Ok(())
}
fn worker(home: &Path, key: &Path) -> Result<()> {
    let result = service::execute(
        home,
        &Store::new(key, home)?,
        read_request(std::io::stdin())?,
    );
    send(
        std::io::stdout(),
        match result {
            Ok(v) => json!({"result":v}),
            Err(e) => json!({"error":e.to_string()}),
        },
    )
}
fn serve(home: &Path, key: &Path) -> Result<()> {
    private_dir(home)?;
    Store::new(key, home)?;
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(home.join("daemon.lock"))?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("wallet daemon already running".into());
    }
    let path = home.join("wallet.sock");
    if let Ok(m) = fs::symlink_metadata(&path) {
        if !m.file_type().is_socket() {
            return Err("socket path collision".into());
        }
        fs::remove_file(&path)?;
    }
    let server = UnixListener::bind(&path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let mut active: Option<(Child, String, Instant)> = None;
    for stream in server.incoming() {
        let mut socket = stream?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        if active
            .as_mut()
            .is_some_and(|(c, _, _)| c.try_wait().ok().flatten().is_some())
        {
            active = None;
        }
        let Ok(cmd) = read_request(&mut socket) else {
            let _ = send(socket, json!({"error":"invalid local command"}));
            continue;
        };
        let op = cmd["op"].as_str().unwrap_or("");
        if op == "progress" || op == "cancel" {
            let stage = active
                .as_ref()
                .map(|(_, s, t)| json!({"stage":s,"seconds":t.elapsed().as_secs_f64()}));
            if op == "cancel" {
                if let Some((mut child, _, _)) = active.take() {
                    let _ = child.kill();
                    child.wait()?;
                }
            }
            let _ = send(
                socket,
                json!({"result":{"active":stage,"cancel_requested":op=="cancel"}}),
            );
            continue;
        }
        if active.is_some() {
            let _ = send(socket, json!({"error":"wallet busy"}));
            continue;
        }
        let output = OwnedFd::from(socket.try_clone()?);
        let parent = std::process::id();
        let mut command = Command::new("/proc/self/exe");
        use std::os::unix::process::CommandExt;
        // The worker cannot outlive its supervising lock after daemon death.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0
                    || libc::getppid() != parent as i32
                {
                    return Err(std::io::Error::other("wallet parent lost"));
                }
                Ok(())
            });
        }
        let mut child = command
            .arg("worker")
            .arg(home)
            .arg(key)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(output))
            .stderr(Stdio::null())
            .spawn()?;
        serde_json::to_writer(child.stdin.take().unwrap(), &cmd)?;
        active = Some((child, op.into(), Instant::now()));
    }
    Ok(())
}
fn main() {
    unsafe {
        libc::umask(0o077);
    }
    let args: Vec<_> = env::args_os().skip(1).collect();
    let result = match args.as_slice() {
        [op, h, k] if op == "worker" => worker(&PathBuf::from(h), &PathBuf::from(k)),
        [h, k] => serve(&PathBuf::from(h), &PathBuf::from(k)),
        _ => Err("usage: pftl-pq-walletd WALLET_DIRECTORY WRAPPING_KEY_FILE".into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
