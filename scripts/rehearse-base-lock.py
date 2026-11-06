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


class RehearsalFailure(RuntimeError):
    """A success predicate of the rehearsal did not hold."""


def require(condition, message):
    """Raise unless `condition` holds; unlike `assert`, PYTHONOPTIMIZE keeps it."""
    if not condition:
        raise RehearsalFailure(message)


def watcher_summary(lines, *, releases_checked):
    """The watcher's documented `--once` output for this rehearsal: one summary
    line with no alarm over a lane of exactly one release, `releases_checked`
    of them checked in this run, each checked one reported as backed."""
    summaries = [line for line in lines if line.get('kind') == 'summary']
    require(len(summaries) == 1, f'watcher must print one summary line, printed {len(summaries)}')
    summary = summaries[0]
    require(summary.get('release_count') == 1, f'watcher must see one release, saw {summary.get("release_count")}')
    require(summary.get('releases_checked') == releases_checked,
            f'watcher must check {releases_checked} release(s), checked {summary.get("releases_checked")}')
    require(summary.get('alarms') == 0, f'watcher must raise no alarm, raised {summary.get("alarms")}')
    require(all(count == 0 for count in (summary.get('alarm_codes') or {}).values()), 'watcher alarm codes must all be zero')
    releases = [line for line in lines if line.get('kind') == 'release']
    require(len(releases) == releases_checked, f'watcher must report {releases_checked} release line(s), reported {len(releases)}')
    require(all(line.get('status') == 'backed' for line in releases), 'every checked release must be backed')
    return summary


CONTRACTS = 'crates/pft-token-contracts'
FOUNDRY_SHARED_DIRS = ('out', 'cache', 'broadcast')


def contracts_checkout_state():
    """What a run must leave unchanged in the checkout's contracts crate: its git
    status, and whether each shared Foundry directory exists and when it last
    changed. The directories are ignored by git, so the status alone misses them."""
    status = subprocess.check_output(['git', 'status', '--porcelain', '--ignored', CONTRACTS], cwd=ROOT, text=True)
    stamps = {}
    for name in FOUNDRY_SHARED_DIRS:
        path = ROOT / CONTRACTS / name
        stamps[name] = path.stat().st_mtime_ns if path.exists() else None
    return status, stamps


# What the ignored composition test (crates/node/src/tests/external_lane_local_evm_tests.rs)
# calls in-process: the implementations behind the four node commands, then the
# ordinary release path. It runs no CLI binary for these steps.
NODE_PLAN = [
    {'step': 'observe the Lock block as a lane checkpoint', 'cli': 'external-lane-checkpoint-observe',
     'calls': 'observe_external_lane_checkpoint', 'in': 'crates/node/src/external_lane_checkpoint_source.rs'},
    {'step': 'five validators re-observe and sign it', 'cli': 'external-lane-checkpoint-sign',
     'calls': 'sign_external_lane_checkpoint_vote', 'in': 'crates/node/src/external_lane_checkpoint_source.rs'},
    {'step': 'assemble the five votes', 'cli': 'external-lane-checkpoint-assemble',
     'calls': 'assemble_external_lane_checkpoint_certificate', 'in': 'crates/node/src/external_lane_checkpoint_source.rs'},
    {'step': 'prove the Lock receipt and build the release', 'cli': 'external-lane-lock-proof',
     'calls': 'build_external_lane_lock_proof', 'in': 'crates/node/src/external_lane_checkpoint_source.rs'},
    {'step': 'admit, batch and apply the release in a certified block, then replay it',
     'calls': 'submit_signed_asset_transaction_json_to_mempool, create_mempool_batch, apply_batch '
              '(execute_external_lane_release, crates/execution/src/external_lane_release.rs), verify_blocks, verify_state'},
]


def free_port():
    with socket.socket() as stream:
        stream.bind(('127.0.0.1', 0))
        return stream.getsockname()[1]


PORT_ATTEMPTS = 5


def confined_paths(output, target_dir):
    """Resolve and check the two paths before any work: the output is a new
    directory distinct from the target and neither holds the other, and the
    target is absent, empty or a cargo target directory (CACHEDIR.TAG or a
    debug/ layout), so the run never writes into an unrelated tree."""
    out, target = output.resolve(), target_dir.resolve()
    require(out != target, f'--output and --target-dir are the same path: {out}')
    require(not out.is_relative_to(target) and not target.is_relative_to(out),
            f'--output {out} and --target-dir {target} must not contain one another')
    require(not out.exists(), f'--output {out} already exists; name a fresh directory')
    if target.exists():
        require(target.is_dir(), f'--target-dir {target} is not a directory')
        if any(target.iterdir()):
            require((target / 'CACHEDIR.TAG').is_file() or (target / 'debug').is_dir(),
                    f'--target-dir {target} is not empty and is not a cargo target directory')
    return out, target


def start_on_free_port(name, argv_for_port, ready, children, record, open_stderr=None, env=None, ticks=600):
    """Start a loopback child on a free port. The port is free when probed but
    another process can take it before the child binds, so a child that exits
    before it is ready is started again on a new port, at most PORT_ATTEMPTS
    times. Returns the port and the running child."""
    for attempt in range(1, PORT_ATTEMPTS + 1):
        port = free_port()
        argv = [str(a) for a in argv_for_port(port)]
        stderr = open_stderr() if open_stderr else subprocess.DEVNULL
        child = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=stderr, env=env)
        children.append(child)
        record.append({'name': name, 'argv': argv, 'attempt': attempt})
        for _ in range(ticks):
            if child.poll() is not None:
                break
            if ready(port):
                return port, child
            time.sleep(.1)
        else:
            raise RehearsalFailure(f'{name} did not become ready on port {port}')
        # Exited before ready: most likely the port was taken in between.
    raise RehearsalFailure(f'{name} exited before ready on {PORT_ATTEMPTS} ports')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, required=True)
    args = parser.parse_args()
    out, target = confined_paths(args.output, args.target_dir)
    print(f'output directory: {out}', file=sys.stderr)
    print(f'cargo target directory: {target}', file=sys.stderr)
    out.mkdir(parents=True, mode=0o700, exist_ok=False)
    environment = os.environ.copy()
    # Foundry writes its artifacts, cache and broadcasts under the output
    # directory, never into the checkout's shared out/, cache/ or broadcast/.
    environment.update(FOUNDRY_OUT=str(out / 'forge-out'), FOUNDRY_CACHE_PATH=str(out / 'forge-cache'),
                       FOUNDRY_BROADCAST=str(out / 'forge-broadcast'))
    environment.update(CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='2', CARGO_INCREMENTAL='0',
                       CARGO_PROFILE_DEV_DEBUG='0', PYTHONPATH=str(ROOT / 'python'),
                       POSTFIAT_LOCAL_EVM_REHEARSAL=str(out / 'config.json'))
    results = {'scope': 'local EVM and generated PFT chain only', 'started_at_unix': int(time.time()),
               'children': [], 'node_plan': NODE_PLAN,
               'source_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
               'commands': [], 'passed': False, 'public_testnet_transaction': False}
    children = []
    native = None
    url = None
    contracts_before = contracts_checkout_state()

    def command(name, argv, cwd=ROOT, expected=0, stdin_text=None):
        began = time.time()
        with (out / f'{name}.stdout').open('w') as stdout, (out / f'{name}.stderr').open('w') as stderr:
            p = subprocess.run([str(a) for a in argv], cwd=cwd, env=environment, stdout=stdout, stderr=stderr,
                               input=stdin_text, text=True, timeout=1800)
        entry = {'name': name, 'argv': [str(a) for a in argv], 'cwd': str(cwd),
                 'exit_code': p.returncode, 'seconds': round(time.time() - began, 3)}
        if stdin_text is not None:
            # Long inputs such as runtime bytecode go through stdin, recorded by hash.
            entry['stdin_sha256'] = hashlib.sha256(stdin_text.encode()).hexdigest()
            entry['stdin_bytes'] = len(stdin_text.encode())
        results['commands'].append(entry)
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
        def anvil_ready(port):
            nonlocal url
            url = f'http://127.0.0.1:{port}'
            try:
                return 'anvil' in str(rpc('web3_clientVersion')).lower()
            except OSError:
                return False

        port, _ = start_on_free_port(
            'anvil', lambda port: ['anvil', '--host', '127.0.0.1', '--port', port, '--chain-id', '84532', '--silent'],
            anvil_ready, children, results['children'], ticks=100)
        url = f'http://127.0.0.1:{port}'
        require(rpc('eth_chainId') == '0x14a34', 'local Anvil must report chain id 84532')
        holder = rpc('eth_accounts')[0]
        contract_dir = ROOT / 'crates/pft-token-contracts'
        command('contracts-build', ['forge', 'build', '--out', out / 'forge-out', '--cache-path', out / 'forge-cache'], contract_dir)

        def deploy(name, contract, constructor=()):
            argv = ['forge', 'create', contract, '--out', out / 'forge-out', '--cache-path', out / 'forge-cache',
                    '--rpc-url', url, '--unlocked', '--from', holder, '--broadcast', '--json']
            if constructor:
                argv += ['--constructor-args', *constructor]
            return json.loads(command(name, argv, contract_dir))['deployedTo'].lower()

        token = deploy('token', 'src/PFTToken.sol:PFTToken', ['Rehearsal PFT', 'rPFT', '300000000', holder])
        verifier = deploy('test-verifier', 'test/PFTLockV1.t.sol:MockFinalityVerifier')
        token_hash = command('token-codehash', ['cast', 'keccak'], stdin_text=rpc('eth_getCode', [token, 'latest'])).strip()
        lock = deploy('lock', 'src/PFTLockV1.sol:PFTLockV1', [token, verifier, token_hash, '6000000', '10'])
        lock_hash = command('lock-codehash', ['cast', 'keccak'], stdin_text=rpc('eth_getCode', [lock, 'latest'])).strip()
        config = dict(rpc=url, output=str(out), holder=holder, token=token, lock=lock,
                      token_code_hash=token_hash, lock_code_hash=lock_hash)
        (out / 'config.json').write_text(json.dumps(config, indent=2) + '\n')
        command('native-release', ['cargo', 'test', '--locked', '-p', 'postfiat-node', '--lib',
                                 'local_evm_lock_checkpoint_release_and_replay_refusal', '--', '--ignored', '--nocapture'])
        native = json.loads((out / 'native-result.json').read_text())
        require(native.get('passed') is True, 'the native composition test must pass')
        require(native.get('checkpoint_votes') == 5, 'five checkpoint votes must certify the Lock block')
        require(native.get('amount_atoms') == native.get('recipient_balance') == 4331,
                'the release must credit exactly the 4,331 locked atoms')
        command('node-build', ['cargo', 'build', '--locked', '-p', 'postfiat-node', '--bin', 'postfiat-node'])
        node = target / 'debug/postfiat-node'
        results['node_sha256'] = hashlib.sha256(node.read_bytes()).hexdigest()
        ready = out / 'rpc-ready.json'
        rpc_port, _ = start_on_free_port(
            'rpc-serve', lambda port: [node, 'rpc-serve', '--unsafe-devnet-json-storage', '--data-dir', native['data_dir'],
                                       '--bind-host', '127.0.0.1', '--port', port, '--max-requests', '100',
                                       '--ready-file', ready],
            lambda port: ready.exists(), children, results['children'],
            open_stderr=lambda: (out / 'rpc.stderr').open('a'), env=environment)
        watcher_args = [sys.executable, '-m', 'postfiat_rpc.external_lane_watcher', '--pftl-rpc', f'127.0.0.1:{rpc_port}',
                        '--lane-id', 'base-erc20', '--base-rpc', url, '--state-file', out / 'watcher-state.json', '--once']
        lines = [json.loads(line) for line in command('watcher', watcher_args).splitlines()]
        results['watcher'] = lines
        watcher_summary(lines, releases_checked=1)
        # A second read exercises the persisted cursor without inventing a new Lock.
        results['watcher_repeat'] = [json.loads(line) for line in command('watcher-repeat', watcher_args).splitlines()]
        watcher_summary(results['watcher_repeat'], releases_checked=0)
        results['native'] = native
        status, _ = contracts_checkout_state()
        require(not subprocess.check_output(['git', 'status', '--porcelain', CONTRACTS], cwd=ROOT, text=True),
                f'the run changed {CONTRACTS} in the checkout')
        require(contracts_checkout_state() == contracts_before,
                f'the run wrote the checkout\'s shared Foundry directories: {status}')
        results['contracts_checkout_unchanged'] = True
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
