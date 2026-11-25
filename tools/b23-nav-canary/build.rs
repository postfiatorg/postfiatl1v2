use std::process::Command;

/// The tool records the source it was built from, as `build-info` prints it.
/// `ops/b24-live/rehearsal_tool.sh` passes `POSTFIAT_BUILD_SOURCE` (the commit, and the files it put in
/// place for a commit that predates them, from `ops/b24-live/tool_source.py`). A build without it
/// records the commit and `+modified` when the tree differs from it in any way.
fn main() {
    println!("cargo:rerun-if-env-changed=POSTFIAT_BUILD_SOURCE");
    println!("cargo:rerun-if-env-changed=PROFILE");
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim().to_string())
    };
    let revision = std::env::var("POSTFIAT_BUILD_SOURCE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            match (
                git(&["rev-parse", "--short=9", "HEAD"]),
                git(&["status", "--porcelain", "--untracked-files=all"]),
            ) {
                (Some(commit), Some(status)) if status.is_empty() => commit,
                (Some(commit), _) => format!("{commit}+modified"),
                _ => "unknown".to_string(),
            }
        });
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=POSTFIAT_BUILD_GIT_REV={revision}");
    println!("cargo:rustc-env=POSTFIAT_BUILD_PROFILE={profile}");
}
