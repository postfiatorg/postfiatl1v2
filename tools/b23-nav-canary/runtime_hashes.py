"""Check the RPC and transport executables for one configured validator."""
import subprocess

RELEASE_SHA = '3039c21af67420a9842b5644eb580b2809081a97419e3e8e1f849d49a487cb6f'


def runtime(peer, expected_sha=RELEASE_SHA):
    name, host, _ = peer
    # Only the checked-in inventory supplies unit names and hosts.
    command = (
        f'for unit in postfiat-{name}.service postfiat-{name}-rpc.service; do '
        'pid=$(systemctl show --value -p MainPID "$unit"); '
        'test "$pid" -gt 0 || exit 1; sha256sum /proc/$pid/exe; done'
    )
    result = subprocess.run(
        ['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'root@' + host, command],
        capture_output=True, text=True, timeout=30,
    )
    hashes = [line.split()[0] for line in result.stdout.splitlines()]
    return name, {
        'returncode': result.returncode,
        'hashes': hashes,
        'matches': result.returncode == 0 and hashes == [expected_sha, expected_sha],
    }
