#!/usr/bin/env python3
"""Repeat archived synthetic-committee Groth16 checks against forked mainnet SP1.

This is NOT a new Schwab export proof. No private keys, real transactions, or
native return imports. Forge runs local tests using an owned loopback Anvil.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parent
KEY = '0x0060d83567657466eaac782e0be174b57277df6ac349f42288852a93d72f0fe1'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--rpc', default='https://ethereum-rpc.publicnode.com')
    p.add_argument('--block', type=int, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    results = []
    for repeat in (1, 2):
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        url = f'http://127.0.0.1:{port}'
        with (a.output / f'run{repeat}.anvil.log').open('w') as log:
            process = subprocess.Popen(['anvil', '--host', '127.0.0.1', '--port', str(port),
                '--chain-id', '31337', '--fork-url', a.rpc, '--fork-block-number', str(a.block),
                '--silent'], stdout=log, stderr=subprocess.STDOUT)
            try:
                def rpc(method, params):
                    data = json.dumps(dict(jsonrpc='2.0', id=1, method=method, params=params)).encode()
                    with urllib.request.urlopen(urllib.request.Request(url, data=data,
                        headers={'Content-Type': 'application/json'}), timeout=15) as response:
                        value = json.load(response)
                    if 'error' in value:
                        raise RuntimeError(value['error'])
                    return value['result']
                for attempt in range(150):
                    try:
                        assert 'anvil' in rpc('web3_clientVersion', []).lower()
                        assert int(rpc('eth_chainId', []), 16) == 31337
                        break
                    except (OSError, AssertionError):
                        if process.poll() is not None:
                            raise RuntimeError('Anvil exited')
                        time.sleep(.2)
                else:
                    raise RuntimeError('Anvil did not start')
                code = bytes.fromhex(rpc('eth_getCode', ['0x397a5f7f3dbd538f23de225b51f532c34448da9b', 'latest'])[2:])
                assert code
                env = dict(os.environ, PFTL_V2_PROGRAM_KEY=KEY)
                result = subprocess.run(['forge', 'test', '--root', str(ROOT / 'receipt-qualification'),
                    '--fork-url', url, '-vv'], env=env, capture_output=True, text=True)
                (a.output / f'run{repeat}.forge.log').write_text(result.stdout + result.stderr)
                result.check_returncode()
                assert '13 tests passed, 0 failed' in result.stdout
                results.append(dict(run=repeat, passed=True, tests_passed=13, block=a.block,
                    sp1_runtime_sha256=hashlib.sha256(code).hexdigest(), program_vkey=KEY))
            finally:
                process.terminate()
                process.wait(timeout=15)
    (a.output / 'result.json').write_text(json.dumps(dict(passed=True, runs=results,
        scope='Archived synthetic-committee receipt and rotation proofs; actual mainnet SP1 runtime in disposable Anvil forks',
        new_navcoin_export_proved=False, pool_swap_run=False, native_return_import=False,
        mainnet_transactions=0, real_keys_loaded=0), indent=2) + '\n')
    print(json.dumps(results))


if __name__ == '__main__':
    main()
