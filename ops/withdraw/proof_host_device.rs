//! CUDA device choice for the withdrawal proof host. Std only, so it is tested without a GPU:
//! `rustc --edition 2021 --test ops/withdraw/proof_host_device.rs -o /tmp/proof-host-device && /tmp/proof-host-device`
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

pub const DEVICE_FLAG: &str = "--cuda-device";
pub const DEVICE_ENV: &str = "PROOF_HOST_CUDA_DEVICE";
/// The device SP1 takes when none is chosen (sp1-cuda 6.3.1 `CudaProver::new` connects to 0).
pub const DEFAULT_DEVICE: u32 = 0;

/// Removes `--cuda-device N` or `--cuda-device=N` from the arguments and returns the chosen device,
/// if any. The flag wins over the environment value; an empty environment value counts as unset.
pub fn take_device(args: Vec<String>, env: Option<&str>) -> Result<(Vec<String>, Option<u32>), String> {
    let mut rest = Vec::with_capacity(args.len());
    let mut flag = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let value = if arg == DEVICE_FLAG {
            it.next().ok_or_else(|| format!("{DEVICE_FLAG} needs a device id"))?
        } else if let Some(v) = arg.strip_prefix("--cuda-device=") {
            v.to_string()
        } else {
            rest.push(arg);
            continue;
        };
        if flag.is_some() {
            return Err(format!("{DEVICE_FLAG} given twice"));
        }
        flag = Some(parse_device(&value, DEVICE_FLAG)?);
    }
    let device = match (flag, env.filter(|v| !v.is_empty())) {
        (Some(id), _) => Some(id),
        (None, Some(v)) => Some(parse_device(v, DEVICE_ENV)?),
        (None, None) => None,
    };
    Ok((rest, device))
}

fn parse_device(value: &str, source: &str) -> Result<u32, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{source}: invalid CUDA device id {value:?}, expected a number such as 0 or 3"));
    }
    value.parse().map_err(|_| format!("{source}: CUDA device id {value:?} is out of range"))
}

/// The socket sp1-cuda 6.3.1 uses for a device (`sp1-cuda/src/client.rs`, `socket_path`).
pub fn socket_path(device: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/sp1-cuda-{device}.sock"))
}

/// Refuses a device whose socket accepts a connection: another prover's `sp1-gpu-server` holds it.
/// A socket file with no listener (left by a dead prover) is not in use.
pub fn check_free(device: u32, socket: &Path) -> Result<(), String> {
    match UnixStream::connect(socket) {
        Ok(_) => Err(format!(
            "CUDA device {device} is in use: {} accepts connections, so another prover's sp1-gpu-server \
             holds that device; choose a free device with {DEVICE_FLAG} or {DEVICE_ENV}, or stop that prover",
            socket.display()
        )),
        Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    const BASE: [&str; 6] = ["proof-host", "withdrawal", "egress.elf", "w.json", "out", "prove"];

    #[test]
    fn no_flag_and_no_environment_keeps_the_default() {
        let (rest, device) = take_device(args(&BASE), None).unwrap();
        assert_eq!(rest, args(&BASE));
        assert_eq!(device, None);
        assert_eq!(device.unwrap_or(DEFAULT_DEVICE), 0);
        assert_eq!(take_device(args(&BASE), Some("")).unwrap().1, None);
    }

    #[test]
    fn flag_is_removed_in_both_forms_and_anywhere() {
        let mut with = args(&BASE);
        with.splice(1..1, args(&["--cuda-device", "3"]));
        assert_eq!(take_device(with, None).unwrap(), (args(&BASE), Some(3)));
        let mut with = args(&BASE);
        with.push("--cuda-device=2".into());
        assert_eq!(take_device(with, None).unwrap(), (args(&BASE), Some(2)));
    }

    #[test]
    fn environment_is_used_and_the_flag_wins() {
        assert_eq!(take_device(args(&BASE), Some("3")).unwrap().1, Some(3));
        let mut with = args(&BASE);
        with.push("--cuda-device=1".into());
        assert_eq!(take_device(with, Some("3")).unwrap().1, Some(1));
    }

    #[test]
    fn bad_values_are_refused() {
        for bad in ["", "-1", "+3", "3,4", " 3", "x", "4294967296"] {
            let mut with = args(&BASE);
            with.push(format!("--cuda-device={bad}"));
            assert!(take_device(with, None).is_err(), "{bad:?} accepted");
        }
        assert!(take_device(args(&BASE), Some("abc")).unwrap_err().contains(DEVICE_ENV));
        let mut missing = args(&BASE);
        missing.push(DEVICE_FLAG.into());
        assert!(take_device(missing, None).unwrap_err().contains("needs a device id"));
        let mut twice = args(&BASE);
        twice.extend(args(&["--cuda-device=1", "--cuda-device", "2"]));
        assert!(take_device(twice, None).unwrap_err().contains("twice"));
    }

    #[test]
    fn socket_path_matches_sp1_cuda() {
        assert_eq!(socket_path(0), PathBuf::from("/tmp/sp1-cuda-0.sock"));
        assert_eq!(socket_path(3), PathBuf::from("/tmp/sp1-cuda-3.sock"));
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("proof-host-device-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_live_socket_is_refused_with_a_clear_message() {
        let dir = scratch("live");
        let socket = dir.join("sp1-cuda-3.sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        let err = check_free(3, &socket).unwrap_err();
        assert!(err.contains("CUDA device 3 is in use"), "{err}");
        assert!(err.contains(&socket.display().to_string()), "{err}");
        assert!(err.contains(DEVICE_FLAG) && err.contains(DEVICE_ENV), "{err}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_or_dead_socket_is_free() {
        let dir = scratch("dead");
        let socket = dir.join("sp1-cuda-0.sock");
        assert!(check_free(0, &socket).is_ok());
        drop(UnixListener::bind(&socket).unwrap());
        assert!(socket.exists());
        assert!(check_free(0, &socket).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
