#!/usr/bin/env python3
"""Exercise real Lock receipts through native release and the watcher on local Anvil.

Owns a disposable loopback EVM. Never accepts a remote URL or deployer key.
The Phase 3 verifier is a test stand-in; no public testnet/mainnet transaction.
"""
from __future__ import annotations
import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def free_port():
    with socket.socket() as stream:
        stream.bind(('127.0.0.1', 0))
        return stream.getsockname()[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, mode=0o700, exist_ok=False)
    target = args.target_dir.resolve()
    environment = os.environ.copy()
    environment.update(CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='2', CARGO_INCREMENTAL='0',
                       CARGO_PROFILE_DEV_DEBUG='0', PYTHONPATH=str(ROOT / 'python'),
                       POSTFIAT_LOCAL_EVM_REHEARSAL=str(out / 'config.json'))
    results = {'scope': 'local EVM and generated PFT chain only', 'started_at_unix': int(time.time()),
               'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'commands': [], 'passed': False, 'public_testnet_transaction': False}
    children = []
    native = None

    def command(name, argv, cwd=ROOT, expected=0):
        began = time.time()
        with (out / f'{name}.stdout').open('w') as stdout, (out / f'{name}.stderr').open('w') as stderr:
            p = subprocess.run([str(a) for a in argv], cwd=cwd, env=environment, stdout=stdout, stderr=stderr, timeout=1800)
        results['commands'].append({'name': name, 'argv': [str(a) for a in argv], 'cwd': str(cwd),
                                    'exit_code': p.returncode, 'seconds': round(time.time() - began, 3)})
        if p.returncode != expected:
            raise RuntimeError(f'{name}: exit {p.returncode}, expected {expected}; see saved logs')
        return (out / f'{name}.stdout').read_text()

    def rpc(method, params=()):
        req = urllib.request.Request(url, data=json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode(), headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=5) as response:
            value = json.load(response)
        if 'error' in value:
            raise RuntimeError(f'local EVM refused {method}')
        return value['result']

    try:
        port = free_port()
        url = f'http://127.0.0.1:{port}'
        anvil = subprocess.Popen(['anvil', '--host', '127.0.0.1', '--port', str(port), '--chain-id', '84532', '--silent'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        children.append(anvil)
        for _ in range(100):
            try:
                if 'anvil' in rpc('web3_clientVersion').lower():
                    break
            except OSError:
                time.sleep(.1)
        else:
            raise RuntimeError('local Anvil did not start')
        assert rpc('eth_chainId') == '0x14a34'
        holder = rpc('eth_accounts')[0]
        contract_dir = ROOT / 'crates/pft-token-contracts'
        command('contracts-build', ['forge', 'build'], contract_dir)

        def deploy(name, contract, constructor=()):
            argv = ['forge', 'create', contract, '--rpc-url', url, '--unlocked', '--from', holder, '--broadcast', '--json']
            if constructor:
                argv += ['--constructor-args', *constructor]
            return json.loads(command(name, argv, contract_dir))['deployedTo'].lower()

        token = deploy('token', 'src/PFTToken.sol:PFTToken', ['Rehearsal PFT', 'rPFT', '300000000', holder])
        verifier = deploy('test-verifier', 'test/PFTLockV1.t.sol:MockFinalityVerifier')
        token_hash = command('token-codehash', ['cast', 'keccak', rpc('eth_getCode', [token, 'latest'])]).strip()
        lock = deploy('lock', 'src/PFTLockV1.sol:PFTLockV1', [token, verifier, token_hash, '6000000', '10'])
        lock_hash = command('lock-codehash', ['cast', 'keccak', rpc('eth_getCode', [lock, 'latest'])]).strip()
        config = dict(rpc=url, output=str(out), holder=holder, token=token, lock=lock,
                      token_code_hash=token_hash, lock_code_hash=lock_hash)
        (out / 'config.json').write_text(json.dumps(config, indent=2) + '\n')
        command('native-release', ['cargo', 'test', '--locked', '-p', 'postfiat-node', '--lib',
                                 'local_evm_lock_checkpoint_release_and_replay_refusal', '--', '--ignored', '--nocapture'])
        native = json.loads((out / 'native-result.json').read_text())
        assert native['passed'] and native['checkpoint_votes'] == 5
        command('node-build', ['cargo', 'build', '--locked', '-p', 'postfiat-node', '--bin', 'postfiat-node'])
        node = target / 'debug/postfiat-node'
        results['node_sha256'] = hashlib.sha256(node.read_bytes()).hexdigest()
        rpc_port = free_port()
        ready = out / 'rpc-ready.json'
        service = subprocess.Popen([str(node), 'rpc-serve', '--unsafe-devnet-json-storage', '--data-dir', native['data_dir'],
                                    '--bind-host', '127.0.0.1', '--port', str(rpc_port), '--max-requests', '100',
                                    '--ready-file', str(ready)], stdout=subprocess.DEVNULL, stderr=(out / 'rpc.stderr').open('w'), env=environment)
        children.append(service)
        for _ in range(600):
            if ready.exists():
                break
            if service.poll() is not None:
                raise RuntimeError('local PFT RPC exited before ready')
            time.sleep(.1)
        else:
            raise RuntimeError('local PFT RPC did not start')
        watcher_args = [sys.executable, '-m', 'postfiat_rpc.external_lane_watcher', '--pftl-rpc', f'127.0.0.1:{rpc_port}',
                        '--lane-id', 'base-erc20', '--base-rpc', url, '--state-file', out / 'watcher-state.json', '--once']
        lines = [json.loads(line) for line in command('watcher', watcher_args).splitlines()]
        results['watcher'] = lines
        assert any(line.get('kind') == 'release' or line.get('type') == 'release' for line in lines), 'watcher must inspect a nonzero release'
        # A second read exercises the persisted cursor without inventing a new Lock.
        results['watcher_repeat'] = [json.loads(line) for line in command('watcher-repeat', watcher_args).splitlines()]
        results['native'] = native
        results['passed'] = True
    except Exception as error:
        results['error'] = str(error)
    finally:
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill(); child.wait()
        results['children_stopped'] = all(child.poll() is not None for child in children)
        # Generated fixture keys are never evidence artifacts. Keep failed
        # chains locally for debugging, under their existing temporary path.
        if native and results['passed']:
            shutil.rmtree(native['data_dir'])
            results['generated_chain_removed'] = True
        results['ended_at_unix'] = int(time.time())
        encoded = json.dumps(results, indent=2) + '\n'
        (out / 'result.json').write_text(encoded)
        (out / 'review.html').write_text('<!doctype html><meta charset="utf-8"><title>Local Base Lock rehearsal</title><h1>Local Base Lock rehearsal</h1><pre>' + html.escape(encoded) + '</pre>')
        print(encoded)
    return 0 if results['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
