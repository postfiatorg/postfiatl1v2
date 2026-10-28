use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};
fn run() -> Result<(), &'static str> {
    // Disable dumps before opening a key. The launcher supplies a network namespace.
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0
            || libc::prctl(libc::PR_SET_DUMPABLE, 0) != 0
        {
            return Err("cannot disable core dumps");
        }
    }
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 || args[1] != "--key-file" || args[3] != "--validator-id" {
        return Err("usage: tasknode-l1-validator-signer --key-file PATH --validator-id ID (challenge on stdin)");
    }
    let mut challenge = String::new();
    std::io::stdin()
        .take(514)
        .read_to_string(&mut challenge)
        .map_err(|_| "invalid challenge input")?;
    // Do not trim: exact UTF-8 challenge bytes are signed.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_secs();
    let proof = tasknode_l1_validator_signer::sign_file(&args[2], &args[4], &challenge, now)?;
    println!(
        "{}",
        serde_json::to_string(&proof).map_err(|_| "proof serialization failed")?
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("L1 validator connect: {e}");
        std::process::exit(1);
    }
}
