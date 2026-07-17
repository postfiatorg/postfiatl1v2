#[cfg(test)]
mod rpc_child_exe_tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        env::temp_dir().join(format!("postfiat-rpc-child-exe-{}-{name}", process::id()))
    }

    #[test]
    fn rpc_child_uses_current_exe_when_it_still_exists() {
        let current = temp_path("current");
        let fallback = temp_path("fallback");
        std::fs::write(&current, b"current").expect("write current");
        std::fs::write(&fallback, b"fallback").expect("write fallback");

        let resolved =
            resolve_rpc_child_exe_from(current.clone(), Some(fallback.clone()), &env::temp_dir());

        assert_eq!(resolved, current);
        let _ = std::fs::remove_file(current);
        let _ = std::fs::remove_file(fallback);
    }

    #[test]
    fn rpc_child_falls_back_to_absolute_argv0_after_binary_replacement() {
        let current = temp_path("deleted-current");
        let fallback = temp_path("absolute-fallback");
        std::fs::write(&fallback, b"fallback").expect("write fallback");

        let resolved =
            resolve_rpc_child_exe_from(current, Some(fallback.clone()), &env::temp_dir());

        assert_eq!(resolved, fallback);
        let _ = std::fs::remove_file(fallback);
    }

    #[test]
    fn rpc_child_resolves_relative_argv0_against_cwd() {
        let cwd = temp_path("cwd");
        let bin_dir = cwd.join("bin");
        let fallback = bin_dir.join("postfiat-node");
        std::fs::create_dir_all(&bin_dir).expect("create bin dir");
        std::fs::write(&fallback, b"fallback").expect("write fallback");

        let resolved = resolve_rpc_child_exe_from(
            cwd.join("missing-current"),
            Some(PathBuf::from("bin/postfiat-node")),
            &cwd,
        );

        assert_eq!(resolved, fallback);
        let _ = std::fs::remove_file(fallback);
        let _ = std::fs::remove_dir(bin_dir);
        let _ = std::fs::remove_dir(cwd);
    }

    #[test]
    fn rpc_child_wait_enforces_timeout() {
        let child = Command::new("sh")
            .arg("-c")
            .arg("sleep 1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn slow child");

        let error = wait_for_child_output_with_timeout(child, Duration::from_millis(0))
            .expect_err("slow child times out");

        assert!(error.contains("rpc serve child timed out"), "{error}");
    }

    #[test]
    fn rpc_child_wait_drains_large_stdout() {
        let child = Command::new("sh")
            .arg("-c")
            .arg("i=0; while [ $i -lt 20000 ]; do printf 0123456789; i=$((i + 1)); done")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn large-output child");

        let output = wait_for_child_output_with_timeout(child, Duration::from_secs(5))
            .expect("large child output is drained");

        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 200_000);
        assert!(output.stderr.is_empty());
    }
}
