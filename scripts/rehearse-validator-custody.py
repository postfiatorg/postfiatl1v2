#!/usr/bin/env python3
"""Run an isolated three-UID custody rehearsal as root with generated keys only.

Requires an already built candidate, wallet-proxy plus its locked dependencies,
and Node.js. Creates no users, changes no units and binds only loopback sockets.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import pwd
import shutil
import socket
import subprocess
import sys
import time
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--javascript-node', type=Path, required=True)
    parser.add_argument('--proxy', type=Path, required=True)
    parser.add_argument('--python-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0, 'root needed to isolate UIDs'
    signer_uid, rpc_uid, group, gateway_uid = 61011, 61012, 61013, 61014
    for uid in (signer_uid, rpc_uid, gateway_uid):
        try:
            pwd.getpwuid(uid)
        except KeyError:
            pass
        else:
            raise RuntimeError('reserved test UID belongs to an existing account')
    for p in Path('/proc').iterdir():
        if p.name.isdigit():
            try:
                assert p.stat().st_uid not in (signer_uid, rpc_uid, gateway_uid), 'test UID already running'
            except FileNotFoundError:
                pass
    out = args.output.resolve()
    out.mkdir(mode=0o755, parents=False, exist_ok=False)
    runtime = out / 'runtime'; runtime.mkdir(mode=0o755)
    evidence = out / 'evidence'; evidence.mkdir(mode=0o700)
    data, keys, sockets, gateway = [runtime / name for name in ('node', 'signer', 'socket', 'gateway')]
    for path, uid, gid, mode in [(keys, signer_uid, group, 0o700), (sockets, signer_uid, group, 0o750), (gateway, gateway_uid, gateway_uid, 0o700)]:
        path.mkdir(mode=mode); os.chown(path, uid, gid); path.chmod(mode)
    node = str(args.node.resolve())
    children = []
    result = {'scope': 'isolated generated chain, three UIDs and real gateway; no production units or keys',
              'started_at_unix': int(time.time()), 'commands': [], 'passed': False,
              'node_sha256': hashlib.sha256(args.node.read_bytes()).hexdigest()}

    def command(name, argv, expected=0, **kwargs):
        with (evidence / f'{name}.stdout').open('w') as stdout, (evidence / f'{name}.stderr').open('w') as stderr:
            p = subprocess.run([str(x) for x in argv], stdout=stdout, stderr=stderr, timeout=120, **kwargs)
        result['commands'].append({'name': name, 'argv': [str(x) for x in argv], 'exit_code': p.returncode})
        assert p.returncode == expected, f'{name}: exit {p.returncode}'
        return (evidence / f'{name}.stdout').read_text()

    def spawn(name, argv, uid, gid, **kwargs):
        child = subprocess.Popen([str(x) for x in argv], user=uid, group=gid, extra_groups=[],
            stdout=(evidence / f'{name}.stdout').open('w'), stderr=(evidence / f'{name}.stderr').open('w'), **kwargs)
        children.append(child); return child

    def wait(check, child, name):
        for _ in range(600):
            if check(): return
            assert child.poll() is None, f'{name} exited early'
            time.sleep(.1)
        raise RuntimeError(f'{name} did not become ready')

    def port():
        with socket.socket() as stream:
            stream.bind(('127.0.0.1', 0)); return stream.getsockname()[1]

    try:
        initialized = command('init', [node, 'init-consensus-v2', '--data-dir', data, '--chain-id', 'isolated-custody-rehearsal',
                        '--node-id', 'validator-0', '--validators', '4', '--activation-height', '10', '--storage-activation-height', '1'])
        faucet = json.loads((data / 'faucet_key.json').read_text())
        batch = runtime / 'activation-batch.json'
        command('batch', [node, 'batch-transfer', '--data-dir', data, '--to', faucet['address'], '--amount', '1', '--batch-file', batch])
        command('activate-store', [node, 'apply-batch', '--data-dir', data, '--batch-file', batch])
        key = keys / 'validator_keys.json'
        (data / 'validator_keys.json').rename(key); os.chown(key, signer_uid, group); key.chmod(0o600)
        for p in [data, *data.rglob('*')]:
            assert not p.is_symlink()
            os.chown(p, rpc_uid, group)
            p.chmod(0o700 if p.is_dir() else 0o600)
        rpc_port, gateway_port = port(), port()
        topology = runtime / 'topology.json'
        command('topology', [node, 'topology-consensus-v2', '--chain-id', 'isolated-custody-rehearsal', '--validators', '4',
                '--activation-height', '10', '--storage-activation-height', '1', '--base-port', str(rpc_port - 20), '--rpc-base-port', str(rpc_port),
                '--hosts', '127.0.0.1,127.0.0.1,127.0.0.1,127.0.0.1', '--output', topology])
        topology.chmod(0o644)
        guard, sock = keys / 'guard', sockets / 'validator.sock'
        chain = json.loads(initialized)
        signer_args = [node, 'validator-signer-serve', '--state-dir', guard, '--chain-id', chain['chain_id'],
                       '--genesis-hash', chain['genesis_hash'], '--key-file', key, '--socket', sock,
                       '--socket-group', str(group), '--allow-peer-uid', str(rpc_uid)]
        signer = spawn('signer-init', [*signer_args, '--initialize-state'], signer_uid, group)
        wait(sock.exists, signer, 'initial signer')
        signer.terminate(); signer.wait(timeout=10)
        signer = spawn('signer', signer_args, signer_uid, group)
        def signer_listening():
            try:
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
                    connection.settimeout(.2); connection.connect(str(sock)); return True
            except OSError:
                return False
        wait(signer_listening, signer, 'restarted signer')
        ready = data / 'rpc-ready.json'
        rpc_process = spawn('rpc', [node, 'rpc-serve', '--data-dir', data, '--port', str(rpc_port), '--bind-host', '127.0.0.1',
                    '--max-requests', '100', '--ready-file', ready, '--allow-mempool-submit-finality',
                    '--finality-topology', topology, '--finality-signer-socket', sock, '--finality-artifact-root', data / 'finality'], rpc_uid, group)
        wait(ready.exists, rpc_process, 'RPC')
        gateway_env = {'PATH': '/usr/bin:/bin', 'HOME': str(gateway), 'RPC_HOST': '127.0.0.1', 'RPC_PORT': str(rpc_port),
                       'RPC_FLEET': f'validator-0=127.0.0.1:{rpc_port}', 'LISTEN_HOST': '127.0.0.1', 'LISTEN_PORT': str(gateway_port),
                       'ENABLE_PROPOSER_ROUTING': 'false', 'WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION': 'off'}
        gateway_process = spawn('gateway', [args.javascript_node.resolve(), args.proxy.resolve() / 'server.js'], gateway_uid, gateway_uid,
                                cwd=gateway, env=gateway_env)
        def healthy():
            try:
                with urllib.request.urlopen(f'http://127.0.0.1:{gateway_port}/healthz', timeout=1) as response:
                    return response.status == 200
            except OSError:
                return False
        wait(healthy, gateway_process, 'gateway')
        databases = list(data.rglob('*.redb')); assert len(databases) == 1, f'expected one database, got {len(databases)}'
        check = [sys.executable, '-m', 'postfiat_ops.validator_signer_check', '--signer-pid', signer.pid, '--rpc-pid', rpc_process.pid,
                 '--gateway-pid', gateway_process.pid, '--rpc-port', rpc_port, '--node-sha256', result['node_sha256'], '--data-dir', data,
                 '--database', databases[0], '--key-file', key, '--state-dir', guard, '--socket', sock, '--output', evidence / 'deployment']
        check_env = dict(os.environ, PYTHONPATH=str(args.python_root.resolve()))
        result['deployment'] = json.loads(command('deployment-check', check, env=check_env))
        # Kernel permissions, as the real serving users, must deny key reads.
        denied = 'import os,sys\ntry: os.open(sys.argv[1],os.O_RDONLY)\nexcept PermissionError: sys.exit(0)\nsys.exit(1)'
        for uid, label in [(rpc_uid, 'rpc'), (gateway_uid, 'gateway')]:
            command(label + '-key-denied', [sys.executable, '-c', denied, key], user=uid, group=group, extra_groups=[])
        # Compare only paths/names internally; never dump a process environment.
        environment = Path(f'/proc/{gateway_process.pid}/environ').read_bytes()
        result['gateway_environment_excludes_validator_paths'] = all(str(p).encode() not in environment for p in (data, keys, guard, key))
        assert result['gateway_environment_excludes_validator_paths']
        result['passed'] = True
    except Exception as error:
        result['error'] = str(error)
    finally:
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try: child.wait(timeout=10)
                except subprocess.TimeoutExpired: child.kill(); child.wait()
        result['children_stopped'] = all(p.poll() is not None for p in children)
        # Remove generated keys and stores; retain only sanitized reports/logs.
        shutil.rmtree(runtime)
        result['runtime_removed'] = True
        result['ended_at_unix'] = int(time.time())
        (evidence / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
