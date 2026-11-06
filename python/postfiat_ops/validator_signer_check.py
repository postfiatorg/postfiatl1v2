"""Read-only Linux custody/deployment check; emits JSON and a local HTML report.

Run on the rehearsal host with permission to inspect all three processes. This
does not start services, sign, read key bytes or grant deployment approval.
"""
from __future__ import annotations

import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import socket
import stat
import sys
import threading
import time

# Key file names a data directory must not hold once the signer has the key:
# rpc-serve's default --finality-key-file, at any depth (also node/).
KEY_FILE_NAMES = ('validator_keys.json',)
CAPABILITY_SETS = ('CapEff', 'CapPrm', 'CapAmb')


def digest(path):
    h = hashlib.sha256()
    with open(path, "rb") as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(part)
    return h.hexdigest()


def start_time(pid):
    # comm may itself contain spaces and parentheses.
    return (Path('/proc') / str(pid) / 'stat').read_text().rsplit(')', 1)[1].split()[19]


def process(pid):
    base = Path('/proc') / str(pid)
    start = start_time(pid)
    status = dict(line.split(':', 1) for line in (base / 'status').read_text().splitlines() if ':' in line)
    args = (base / 'cmdline').read_bytes().rstrip(b'\0').decode().split('\0')
    fds = [os.readlink(p) for p in (base / 'fd').iterdir()]
    result = {'pid': pid, 'start_time': start, 'uids': [int(v) for v in status['Uid'].split()],
              'capabilities': {name: int(status[name].strip(), 16) for name in CAPABILITY_SETS},
              'args': args, 'cwd': os.readlink(base / 'cwd'), 'fds': fds,
              'exe_sha256': digest(base / 'exe'),
              'same_network_namespace': os.stat(base / 'ns/net').st_ino == os.stat('/proc/self/ns/net').st_ino}
    if start != start_time(pid):
        raise ValueError('process changed during inspection')
    return result


def option(args, name):
    values = []
    for i, arg in enumerate(args):
        if arg == name and i + 1 < len(args):
            values.append(args[i + 1])
        elif arg.startswith(name + '='):
            values.append(arg[len(name) + 1:])
    return values


def argument_path(proc, name, expected):
    values = option(proc['args'], name)
    return len(values) == 1 and (Path(proc['cwd']) / values[0]).resolve() == Path(expected).resolve()


def private_path(path, uid, directory=False, *, test_root=None):
    """Owner-only path whose ancestors no other user can replace. `test_root`
    is for unit tests only: the walk stops above that private root."""
    path = Path(path)
    info = path.lstat()
    if info.st_uid != uid or stat.S_IMODE(info.st_mode) & 0o077:
        return False
    if not (stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)):
        return False
    root = Path(test_root).resolve() if test_root is not None else None
    for parent in path.resolve().parents:
        if root is not None and parent != root and root.is_relative_to(parent):
            break
        s = parent.stat()
        sticky = s.st_uid == 0 and s.st_mode & stat.S_ISVTX
        if s.st_uid not in (0, uid) or (s.st_mode & 0o022 and not sticky):
            return False
    return True


def key_copies(data_dir, key_files=()):
    """Regular files under the data directory named like a validator key file
    or like the signer's key files. Names only: no file is read."""
    names = set(KEY_FILE_NAMES) | {Path(path).name for path in key_files}
    found = []
    for directory, _, files in os.walk(data_dir):
        found += [str(Path(directory) / name) for name in files if name in names]
    return sorted(found)


def owns_listener(proc, port):
    if not proc['same_network_namespace']:
        return False
    inodes = {fd[8:-1] for fd in proc['fds'] if fd.startswith('socket:[')}
    for table in ('tcp', 'tcp6'):
        for line in (Path('/proc/net') / table).read_text().splitlines()[1:]:
            row = line.split()
            if int(row[1].split(':')[1], 16) == port and row[3] == '0A' and row[9] in inodes:
                return True
    return False


def readiness(port):
    request = {'version': 'postfiat-local-rpc-v1', 'id': 'custody-check',
               'method': 'private_readiness', 'params': {}}
    with socket.create_connection(('127.0.0.1', port), timeout=5) as stream:
        stream.settimeout(90)
        stream.sendall(json.dumps(request).encode() + b'\n')
        stream.shutdown(socket.SHUT_WR)
        line = stream.makefile('rb').readline(1024 * 1024 + 1)
    if len(line) > 1024 * 1024:
        raise ValueError('readiness response exceeds bound')
    value = json.loads(line)
    if not isinstance(value, dict) or not isinstance(value.get('result'), dict):
        raise ValueError('readiness response is not an object')
    if value.get('ok') is not True or value.get('version') != request['version'] or value.get('id') != request['id']:
        raise ValueError('readiness RPC refused')
    return value['result']


def readiness_with_database_observation(port, proc, database):
    # The store deliberately releases its writer lease while RPC is idle.
    # Observe its file descriptor during this request instead of requiring an
    # idle handle (which would block the validator's other serving process).
    expected = str(Path(database).resolve())
    observed = threading.Event()
    stop = threading.Event()
    def sample():
        while not stop.is_set():
            try:
                for fd in (Path('/proc') / str(proc['pid']) / 'fd').iterdir():
                    try:
                        if os.readlink(fd) == expected:
                            observed.set()
                    except FileNotFoundError:
                        pass
            except OSError:
                return
            stop.wait(.001)
    worker = threading.Thread(target=sample, daemon=True)
    worker.start()
    try:
        report = readiness(port)
    finally:
        stop.set()
        worker.join()
    if observed.is_set():
        proc['fds'].append(expected)
    return report


def assess(signer, rpc, gateway, report, expected, observations):
    """Keep scope to observable deployment facts, separate from governance readiness."""
    uid = lambda p: p['uids'][1]
    custody = report.get('custody', {})
    roots = [Path(expected[k]).resolve() for k in ('data_dir', 'state_dir')]
    protected = {str(Path(expected[k]).resolve()) for k in ('key_file', 'socket', 'database')}
    gateway_open = []
    for fd in gateway['fds']:
        if fd.startswith('/'):
            p = Path(fd.removesuffix(' (deleted)')).resolve()
            if str(p) in protected or any(p == root or root in p.parents for root in roots):
                gateway_open.append(fd)
    checks = {
        'three_distinct_unprivileged_users': len({uid(signer), uid(rpc), uid(gateway)}) == 3 and all(0 not in p['uids'] and len(set(p['uids'])) == 1 for p in (signer, rpc, gateway)),
        # A non-root uid with CAP_DAC_READ_SEARCH/CAP_DAC_OVERRIDE (for example
        # systemd AmbientCapabilities) reads the key despite its mode.
        'rpc_and_gateway_hold_no_capabilities': all(isinstance(p.get('capabilities'), dict) and all(p['capabilities'].get(name) == 0 for name in CAPABILITY_SETS) for p in (rpc, gateway)),
        'node_executables_match_pin': signer['exe_sha256'] == rpc['exe_sha256'] == expected['node_sha256'],
        'signer_command': 'validator-signer-serve' in signer['args'],
        'rpc_command': 'rpc-serve' in rpc['args'],
        'signer_paths': argument_path(signer, '--key-file', expected['key_file']) and argument_path(signer, '--state-dir', expected['state_dir']) and argument_path(signer, '--socket', expected['socket']),
        'rpc_uses_expected_store_and_socket': argument_path(rpc, '--data-dir', expected['data_dir']) and argument_path(rpc, '--finality-signer-socket', expected['socket']),
        'rpc_has_no_file_signer_flags': not any(option(rpc['args'], k) for k in ('--finality-key-file', '--finality-proposal-key-file')),
        'only_rpc_uid_allowed': set(option(signer['args'], '--allow-peer-uid')) == {str(uid(rpc))},
        'restart_does_not_initialize_state': '--initialize-state' not in signer['args'],
        'expected_database_open': str(Path(expected['database']).resolve()) in rpc['fds'],
        'gateway_has_no_protected_file_open': not gateway_open,
        'live_readiness_has_durable_socket_custody': custody.get('published_by') == 'rpc_serve' and custody.get('validator_signer') == 'socket' and custody.get('transactional_active') is True and custody.get('signer_uid') == uid(signer) and custody.get('serving_uid') == uid(rpc) and custody.get('signer_double_sign_protection') is True and custody.get('data_dir_key_files') == [] and custody.get('signer_key_matches') is True and custody.get('validator_registry_readable') is True,
        **observations,
    }
    return {'schema': 'postfiat.validator-signer.deployment-check.v1',
            'observed_at_unix': int(time.time()), 'checks': checks,
            'passed': all(checks.values()), 'readiness_ready': report.get('ready'),
            'readiness_reasons': report.get('reasons'),
            'processes': {name: {k: p[k] for k in ('pid', 'start_time', 'uids', 'exe_sha256')} for name, p in [('signer', signer), ('rpc', rpc), ('gateway', gateway)]},
            'limitations': ['Point-in-time process and filesystem observation, not a trace of all access.',
                            'The operator must pin the intended live data/database paths; a supplied copy path cannot prove live deployment.',
                            'Does not certify gateway memory/environment secret absence or production approval.']}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for role in ('signer', 'rpc', 'gateway'):
        parser.add_argument(f'--{role}-pid', type=int, required=True)
    for field in ('data-dir', 'database', 'key-file', 'state-dir', 'socket', 'node-sha256'):
        parser.add_argument(f'--{field}', required=True)
    parser.add_argument('--rpc-port', type=int, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(argv)
    args.output.mkdir(parents=True, exist_ok=False)
    try:
        signer, rpc, gateway = [process(getattr(args, f'{role}_pid')) for role in ('signer', 'rpc', 'gateway')]
        uid = signer['uids'][1]
        proposal = option(signer['args'], '--proposal-key-file')
        observations = {'key_is_private': private_path(args.key_file, uid),
                        'data_dir_holds_no_key_file': not key_copies(args.data_dir, [args.key_file, *proposal]),
                        'guard_is_private': private_path(args.state_dir, uid, True) and all(private_path(Path(args.state_dir) / f, uid) for f in ('state.json', 'lock')),
                        'rpc_owns_readiness_listener': owns_listener(rpc, args.rpc_port)}
        socket_info = Path(args.socket).lstat()
        observations['socket_owned_and_group_only'] = stat.S_ISSOCK(socket_info.st_mode) and socket_info.st_uid == uid and stat.S_IMODE(socket_info.st_mode) == 0o660
        if not observations['rpc_owns_readiness_listener']:
            raise ValueError('expected rpc PID does not own the local readiness listener')
        report = readiness_with_database_observation(args.rpc_port, rpc, args.database)
        result = assess(signer, rpc, gateway, report, vars(args), observations)
        result['checks']['processes_still_same'] = all(start_time(p['pid']) == p['start_time'] for p in (signer, rpc, gateway))
        result['passed'] = all(result['checks'].values())
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        # Avoid process argv/environment or secret-file content in diagnostics.
        result = {'schema': 'postfiat.validator-signer.deployment-check.v1', 'passed': False,
                  'error': type(error).__name__, 'detail': 'Inspection unavailable or inconsistent; no custody pass. Check permissions, PIDs, listener and supplied paths.'}
    encoded = json.dumps(result, indent=2) + '\n'
    (args.output / 'report.json').write_text(encoded)
    (args.output / 'review.html').write_text('<!doctype html><meta charset="utf-8"><title>Validator custody check</title><h1>Validator custody check</h1><pre>' + html.escape(encoded) + '</pre>')
    print(encoded, end='')
    return 0 if result['passed'] else 2


if __name__ == '__main__':
    sys.exit(main())
