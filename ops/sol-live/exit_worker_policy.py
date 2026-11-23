"""Root preflight for host permissions of the network-facing worker account."""
import grp
import os
import pwd
import re
import subprocess


def check():
    if os.geteuid() != 0:
        raise RuntimeError("Worker policy checks require root.")
    account = "pf-sol-exit"
    worker = pwd.getpwnam(account)
    home = pwd.getpwnam("postfiat").pw_dir
    groups = os.getgrouplist(account, worker.pw_gid)
    try:
        docker_gid = grp.getgrnam("docker").gr_gid
    except KeyError:
        docker_gid = None
    if worker.pw_uid == 0 or docker_gid in groups:
        raise RuntimeError("Worker has root or Docker group privileges.")

    # Use actual account credentials outside the unit's filesystem namespace.
    for flag, path in [("-w", "/run/docker.sock"), ("-r", home), ("-x", home)]:
        result = subprocess.run(["/usr/bin/test", flag, path], user=worker.pw_uid,
                                group=worker.pw_gid, extra_groups=groups)
        if result.returncode != 1:
            raise RuntimeError(f"Worker access check failed for {flag} {path}.")

    # Listing another user's complete policy as root also detects passworded
    # grants and commands limited to a particular target user.
    policy = subprocess.run(["/usr/bin/sudo", "-n", "-l", "-U", account],
                            env={"LC_ALL": "C", "PATH": "/usr/sbin:/usr/bin:/sbin:/bin"},
                            capture_output=True, text=True)
    denial = re.fullmatch(r"User pf-sol-exit is not allowed to run sudo on [^\n]+\.",
                          (policy.stdout + policy.stderr).strip())
    if policy.returncode != 1 or not denial:
        raise RuntimeError("Cannot confirm the worker has no sudo permissions.")
    print("Worker host policy verified.")


if __name__ == "__main__":
    check()
