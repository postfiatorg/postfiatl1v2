#!/usr/bin/env python3
"""The Phase 3 deployment-order dry run on Base Sepolia as ONE resumable command
(export design note, section 5; runbook docs/runbooks/lane-base-sepolia-dry-run.md).

Stages, in order, each run once (state.json in the output directory):
plan, fleet, preflight, deploy, lock, release-on-pftl, export,
prove-and-release, record.

`--fork` runs every stage against a local Anvil fork of Base Sepolia with
impersonated, disposable accounts: nothing is sent to Base Sepolia. Without
`--fork`, `deploy` and `lock` send real Base Sepolia transactions from Foundry
keystores named by `--*-account` and `--*-password-file`; a raw private key is
never accepted. The verifier's SP1 verifier is Succinct's versioned
SP1 v6.1.0 Groth16 verifier (the circuit SP1 6.3.1 proves for); a mock SP1
verifier is refused unless the RPC is a loopback Anvil.

The PFTL "fleet" is the round-trip rehearsal's driver: ONE generated data
directory driven by an ignored node test, every block carrying consensus v2
commits from all six validators; no separate node processes or network. It
lives in this process, so `fleet` through `export` must run in one invocation.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def _load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ORDER = _load('rehearse_phase3_deployment_order', 'scripts/rehearse-phase3-deployment-order.py')
RT = _load('rehearse_lane_roundtrip', 'scripts/rehearse-lane-roundtrip.py')
RELAY = _load('pft_lane_export_relay', 'scripts/pft-lane-export-relay.py')
keccak256 = ORDER.keccak256
require = ORDER.require
RehearsalFailure = ORDER.RehearsalFailure

STAGES = ('plan', 'fleet', 'preflight', 'deploy', 'lock', 'release-on-pftl', 'export', 'prove-and-release', 'record')
DRIVER_STAGES = ('fleet', 'preflight', 'deploy', 'lock', 'release-on-pftl', 'export')
BASE_SEPOLIA_RPC = 'https://sepolia.base.org'
CHAIN_ID = 84532
TOKEN = '0x90ce303356d80ecbe359faf061c84130f40eacf0'
HOLDER = '0x6e4e8e0ed73f807d8e8479cb8f25d87ce8811e17'
LOCK_AMOUNT = 1_000_000
EXPORT_AMOUNT = 400_000
# Succinct's versioned SP1 v6.1.0 Groth16 verifier on Base Sepolia (no owner, no
# upgrade path); SP1 6.3.1 proves for the v6.1.0 circuit (VERIFIER_HASH below).
SP1_VERIFIER = {
    'address': '0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2',
    'name': 'V6_1_0_SP1_VERIFIER_GROTH16',
    'source': 'https://github.com/succinctlabs/sp1-contracts/blob/v6.1.0/contracts/deployments/84532.json',
    'source_commit': '2ac5ecbbe473421a963d67e55f182e9a36576f7c',
    'runtime_code_hash': '0xcceb864cd8a5a36b2073a8f2b32a773835cd2dd2c78a56f8e6fdb942feff04dd',
    'verifier_hash': '0x4388a21c687fdd5f218d7e3d13190cac4c5355818d3605fd5fb811df468ee696',
    'version': 'v6.1.0',
}
GAS_MARGIN = 3
PROGRAM_INFO = ROOT / 'programs/pft-lane-export/program-info.json'
COMMITTED_ELF = ROOT / 'programs/pft-lane-export/elf/pft-lane-export-program'


def check_committed_identity(program_info, elf_path):
    """The ELF's sha256 must be the program info's: the key belongs to that ELF."""
    actual = hashlib.sha256(Path(elf_path).read_bytes()).hexdigest()
    require(actual == program_info['elf_sha256'],
            f'the ELF {elf_path} has sha256 {actual}, but the program info names {program_info["elf_sha256"]}')
    return actual


# --- the state machine --------------------------------------------------------

def check_deployer_keystores(plan, derive):
    """Each keystore must be the deployer the plan predicted from; checked before any broadcast."""
    for role in ('verifier', 'lock'):
        derived = derive(role).lower()
        require(derived == plan[role]['deployer'].lower(),
                f'the {role} keystore is {derived}, but the plan predicted from {plan[role]["deployer"]}; '
                'nothing was sent')


def check_planned_nonces(plan, nonce):
    """Both deployers' latest and pending nonces must still be the planned ones."""
    for role in ('verifier', 'lock'):
        for tag in ('latest', 'pending'):
            current = nonce(plan[role]['deployer'], tag)
            require(current == plan[role]['planned_nonce'],
                    f'the {role} deployer\'s {tag} nonce is {current}, the plan needs {plan[role]["planned_nonce"]}; '
                    'nothing was sent')


def check_landed(plan, role, address, code_hash, expected_code):
    require(address.lower() == plan[role]['predicted'].lower() and code_hash == expected_code,
            f'the {role} landed at {address} with code hash {code_hash}; the plan predicted '
            f'{plan[role]["predicted"]} with {expected_code}. The plan is dead: its nonce is spent'
            + ('; the lock was NOT deployed' if role == 'verifier' else ''))


def deploy_pair(plan, *, send, code_hash, expected_code):
    """The verifier, then the lock. Each must land at its predicted address with
    the preflight's code hash; a verifier that does not stops the run before the
    lock is sent."""
    result = {}
    for role in ('verifier', 'lock'):
        address, transaction = send(role)
        check_landed(plan, role, address, code_hash(address), expected_code[role])
        result[role] = {'address': address.lower(), 'transaction': transaction}
    return result


RELEASES = ROOT / 'programs/pft-lane-export/releases.json'


def load_program_identity(*, fork, override, worktree_clean, head, elf=COMMITTED_ELF):
    """The program key and where it comes from. Off the fork: only the committed
    program-info.json at the checked-out commit, in a clean worktree, with the
    committed ELF hashing to its elf_sha256 and releases.json naming the same
    identity and its two reproducible runs. `--program-info` only with `--fork`."""
    if not fork:
        require(override is None, '--program-info is accepted only with --fork; a real run uses the committed one')
        require(worktree_clean, 'a real run needs a clean worktree at a commit; commit or stash first')
    info = json.loads((override or PROGRAM_INFO).read_text())
    for field in ('schema', 'elf_sha256', 'program_vkey', 'source_commit', 'docker_image', 'build_command'):
        require(info.get(field), f'the program info has no {field}')
    require(info['schema'] == 'postfiat-sp1-program-info-v1', 'the program info has another schema')
    elf_sha256 = check_committed_identity(info, elf)
    runs = []
    if override is None:
        releases = json.loads(RELEASES.read_text())
        release = releases['releases'][-1]
        require((release['elf_sha256'], release['program_vkey'], release['source_commit'])
                == (info['elf_sha256'], info['program_vkey'], info['source_commit']),
                'releases.json names another identity than program-info.json')
        require(releases['docker_image'] == info['docker_image'], 'releases.json names another image')
        runs = release['runs']
        require(len(runs) >= 2, 'releases.json must record two reproducible runs')
    return {'elf_sha256': elf_sha256, 'program_vkey': info['program_vkey'], 'source_commit': info['source_commit'],
            'docker_image': info['docker_image'], 'build_command': info['build_command'], 'runs': runs,
            'program_info': str((override or PROGRAM_INFO).relative_to(ROOT)) if (override or PROGRAM_INFO)
            .resolve().is_relative_to(ROOT) else '<override>', 'checked_out_commit': head}


# --- recovery: intent before every public transaction, reconciliation on a rerun ---

def begin_intent(state, *, stage, role, sender, nonce, purpose, **extra):
    """Record the intent of a transaction before it is sent; the same intent on a rerun."""
    intents = state.setdefault('intents', [])
    for intent in intents:
        if (intent['stage'], intent['role']) == (stage, role):
            return intent
    intent = {'stage': stage, 'role': role, 'sender': sender, 'nonce': nonce, 'purpose': purpose,
              'hash': None, 'receipt': None, 'outcome': None, **extra}
    intents.append(intent)
    return intent


def reconcile_deployment(intent, chain, *, predicted, expected_code):
    """`adopt` a mined deployment that matches the plan, `send` when nothing was
    sent at the planned nonce; refuse anything else."""
    if intent.get('hash'):
        receipt = chain.receipt(intent['hash'])
        if receipt is not None:
            require(int(str(receipt.get('status')), 0) == 1,
                    f'the {intent["role"]} transaction {intent["hash"]} reverted; its nonce is spent and the plan is dead')
    if chain.nonce(intent['sender'], 'latest') > intent['nonce']:
        found = chain.code_hash(predicted)
        require(found == expected_code,
                f'the {intent["role"]} deployer\'s nonce {intent["nonce"]} is spent, but {predicted} holds code '
                f'{found}, not {expected_code}; the plan is dead')
        return 'adopt'
    require(chain.nonce(intent['sender'], 'pending') == intent['nonce'],
            f'a transaction of the {intent["role"]} deployer is pending at the planned nonce; wait for it and rerun')
    return 'send'


def reconcile_approve(intent, chain, *, owner, spender, amount):
    return 'adopt' if chain.allowance(owner, spender) >= amount else 'send'


def reconcile_lock(intent, chain, *, lock, holder, amount, recipient_hash):
    """A Lock at the recorded `nextNonce` that matches the plan is adopted; one
    that does not is refused; a second Lock is never sent for one plan."""
    before = intent['next_nonce_before']
    if chain.next_nonce(lock) > before:
        record = chain.lock_record(lock, before)
        require(record == (holder, amount, recipient_hash),
                f'Lock nonce {before} on {lock} is {record}, not this plan\'s Lock; refusing to send another')
        return 'adopt'
    if intent.get('hash'):
        receipt = chain.receipt(intent['hash'])
        require(receipt is None or int(str(receipt.get('status')), 0) != 1,
                'the Lock transaction succeeded but the lock\'s nextNonce did not move; stop and inspect')
    return 'send'


def require_no_resume_acknowledged(acknowledged):
    require(acknowledged, 'the PFTL fleet lives in this process from fleet to export and cannot be reopened: a crash '
                          'after deploy leaves contracts bound to a chain that is gone. Pass '
                          '--i-accept-no-resume-between-deploy-and-export to go on')


LOCKED_TOPIC = '0x' + keccak256(b'Locked(uint64,address,bytes32,bytes,uint64)').hex()


class BaseChain:
    """The reads reconciliation needs, over one JSON-RPC callable."""

    def __init__(self, rpc):
        self.rpc = rpc

    def receipt(self, transaction):
        return self.rpc('eth_getTransactionReceipt', [transaction])

    def nonce(self, address, tag):
        return int(self.rpc('eth_getTransactionCount', [address, tag]), 16)

    def code_hash(self, address):
        return ORDER.code_hash(self.rpc('eth_getCode', [address, 'latest']))

    def _call(self, to, signature, *args):
        return self.rpc('eth_call', [{'to': to, 'data': ORDER.call_data(signature, *args)}, 'latest'])

    def allowance(self, owner, spender):
        return int(self._call(TOKEN, 'allowance(address,address)', owner, spender), 16)

    def next_nonce(self, lock):
        return int(self._call(lock, 'nextNonce()'), 16)

    def lock_record(self, lock, nonce):
        data = self._call(lock, 'lockRecord(uint64)', nonce)[2:]
        return ('0x' + data[24:64], int(data[64:128], 16), '0x' + data[128:192])

    def lock_log(self, lock, nonce, from_block):
        logs = self.rpc('eth_getLogs', [{'address': lock, 'fromBlock': hex(from_block), 'toBlock': 'latest',
                                         'topics': [LOCKED_TOPIC, ORDER.abi_word(nonce)]}])
        require(len(logs) == 1, f'Lock nonce {nonce} has {len(logs)} Locked events')
        return logs[0]


def redact_all(text, secrets):
    """Keystore names and password-file paths never appear in an error."""
    for secret in sorted(secrets, key=len, reverse=True):
        text = text.replace(secret, '<redacted>')
    return text


def load_state(path):
    return json.loads(path.read_text()) if path.exists() else {'completed': [], 'data': {}}


def check_stage(state, stage):
    """A stage runs once and only after every earlier stage."""
    require(stage in STAGES, f'unknown stage {stage}')
    require(stage not in state['completed'], f'stage {stage} already ran; it runs once')
    for earlier in STAGES[:STAGES.index(stage)]:
        if earlier == 'prove-and-release' and stage == 'record':
            continue  # the record also covers a run that stopped for want of a proof
        require(earlier in state['completed'], f'stage {stage} needs {earlier} first')


def signer_flags(account, password_file):
    """Foundry keystore flags; there is no raw-key path."""
    require(account and password_file, 'name a keystore with --*-account and --*-password-file')
    return ['--account', account, '--password-file', str(password_file)]


def refuse_mock_off_loopback(*, mock_artifact, rpc_url, client_version):
    """A mock SP1 verifier lets anyone release from the lock: loopback Anvil only."""
    if mock_artifact is None:
        return
    require(RELAY.is_loopback_url(rpc_url) and 'anvil' in client_version.lower(),
            'a mock SP1 verifier is refused unless the RPC is a loopback Anvil')


def deploy_sheet(*, plan, verifier_config, gas_used, gas_price_wei, program_identity=None):
    """Every address, nonce, constructor value, gas figure and the test ETH each key needs."""
    keys = {
        'verifier deployer': ('deploy-verifier',),
        'lock deployer': ('deploy-lock',),
        'holder': ('approve', 'lock'),
    }
    needs = {}
    for key, steps in keys.items():
        gas = sum(gas_used.get(step, 0) for step in steps)
        needs[key] = {'steps': list(steps), 'gas': gas,
                      'wei_with_margin': gas * gas_price_wei * GAS_MARGIN,
                      'eth_with_margin': f'{gas * gas_price_wei * GAS_MARGIN / 1e18:.9f}'}
    return {'chain_id': CHAIN_ID, 'plan': plan, 'verifier_constructor': verifier_config,
            'program_identity': program_identity,
            'sp1_verifier': SP1_VERIFIER, 'gas_price_wei': gas_price_wei, 'gas_margin': GAS_MARGIN,
            'gas_used_on_fork': gas_used, 'test_eth_per_key': needs,
            'release_gas': 'not estimated: a release needs a real proof'}


def sheet_markdown(sheet):
    lines = ['# Lane deploy sheet (Base Sepolia, chain id 84532)', '',
             '| Role | Address | Nonce | Predicted contract |', '|---|---|---|---|']
    plan = sheet['plan']
    for role in ('verifier', 'lock'):
        lines.append(f'| {role} deployer | `{plan[role]["deployer"]}` | {plan[role]["planned_nonce"]} | '
                     f'`{plan[role]["predicted"]}` |')
    lines += ['', '| Key | Gas on the fork | Test ETH (x3 margin) |', '|---|---|---|']
    for key, need in sheet['test_eth_per_key'].items():
        lines.append(f'| {key} | {need["gas"]} | {need["eth_with_margin"]} |')
    identity = sheet.get('program_identity') or {}
    if identity:
        lines += ['', f'Program: ELF sha256 `{identity["elf_sha256"]}`, key `{identity["program_vkey"]}`, source '
                      f'commit `{identity["source_commit"]}`.']
    lines += ['', f'Gas price read: {sheet["gas_price_wei"]} wei. SP1 verifier: `{SP1_VERIFIER["address"]}` '
              f'({SP1_VERIFIER["name"]}, {SP1_VERIFIER["source"]}).', '', '## Verifier constructor', '']
    lines += [f'- `{name}`: `{value}`' for name, value in (sheet['verifier_constructor'] or {}).items()]
    return '\n'.join(lines) + '\n'


# --- the run --------------------------------------------------------------------

def parser():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--output', type=Path, required=True, help='the run directory; holds state.json')
    p.add_argument('--fork', action='store_true', help='run every stage on a local Anvil fork of Base Sepolia')
    p.add_argument('--through', choices=STAGES, default='record', help='stop after this stage')
    p.add_argument('--program-info', type=Path,
                   help='another program-info.json; only with --fork. A real run takes the committed one')
    p.add_argument('--elf', type=Path, default=COMMITTED_ELF,
                   help='the guest ELF; default: the committed one, checked against the program info')
    p.add_argument('--node-bin', type=Path, default=ROOT / 'target/debug/postfiat-node')
    p.add_argument('--proof', type=Path, help='a real SP1 proof file for prove-and-release')
    p.add_argument('--mock-sp1-artifact', type=Path, help='refused unless the RPC is a loopback Anvil')
    for role in ('verifier', 'lock', 'holder', 'relayer'):
        p.add_argument(f'--{role}-account')
        p.add_argument(f'--{role}-password-file', type=Path)
    p.add_argument('--i-accept-no-resume-between-deploy-and-export', action='store_true',
                   help='deploy goes on knowing the in-process PFTL fleet cannot be reopened after a crash')
    p.add_argument('--verifier-deployer', help='address of the one-time verifier keystore')
    p.add_argument('--lock-deployer', help='address of the one-time lock keystore')
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    out = args.output.resolve()
    require(not out.is_relative_to(ROOT / 'docs'), '--output must not be inside docs/')
    out.mkdir(parents=True, exist_ok=True, mode=0o700)
    state_path = out / 'state.json'
    state = load_state(state_path)
    data = state['data']
    state.setdefault('mode', 'fork' if args.fork else 'base-sepolia')
    require(state['mode'] == ('fork' if args.fork else 'base-sepolia'), 'this output directory is for another mode')
    # This version never deploys a mock; an artifact named off a loopback fork is refused outright.
    refuse_mock_off_loopback(mock_artifact=args.mock_sp1_artifact,
                             rpc_url='http://127.0.0.1' if args.fork else BASE_SEPOLIA_RPC,
                             client_version='anvil' if args.fork else '')
    identity = load_program_identity(
        fork=args.fork, override=args.program_info, elf=args.elf,
        worktree_clean=not subprocess.run(['git', 'status', '--porcelain'], cwd=ROOT, capture_output=True,
                                          text=True).stdout.strip(),
        head=ORDER.git('rev-parse', 'HEAD'))
    program_info = identity
    vkey = identity['program_vkey']
    children, roles = [], {str(out): '<output>'}
    environment = dict(os.environ)
    urls = {'public': BASE_SEPOLIA_RPC}

    def save():
        state_path.write_text(json.dumps(state, indent=2) + '\n')

    redacted = [str(value) for value in (args.verifier_account, args.lock_account, args.holder_account,
                                         args.relayer_account, args.verifier_password_file,
                                         args.lock_password_file, args.holder_password_file,
                                         args.relayer_password_file) if value]

    def command(name, argv, cwd=ROOT, env=None, expected=0):
        with (out / f'{name}.stdout').open('w') as so, (out / f'{name}.stderr').open('w') as se:
            try:
                p = subprocess.run([str(a) for a in argv], cwd=cwd, env=env or environment, stdout=so, stderr=se,
                                   text=True, timeout=3600)
            except (subprocess.TimeoutExpired, OSError) as error:
                raise RehearsalFailure(redact_all(f'{name}: {error}', redacted)) from None
        require(p.returncode == expected, f'{name}: exit {p.returncode}; see {out / (name + ".stderr")}')
        return (out / f'{name}.stdout').read_text()

    def rpc(url, method, params=()):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode()
        req = urllib.request.Request(url, data=body, headers={'Content-Type': 'application/json',
                                                              'User-Agent': 'postfiat-lane-dry-run/1'})
        with urllib.request.urlopen(req, timeout=30) as response:
            value = json.load(response)
        require('error' not in value, f'{method}: {value.get("error")}')
        return value['result']

    def start_fork(name, block):
        def ready(port):
            try:
                return 'anvil' in str(rpc(f'http://127.0.0.1:{port}', 'web3_clientVersion')).lower()
            except OSError:
                return False
        port, _ = BASE_LOCK_START(name, lambda port: ['anvil', '--host', '127.0.0.1', '--port', port, '--fork-url',
                                                      BASE_SEPOLIA_RPC, '--fork-block-number', str(block),
                                                      '--silent'], ready, children, [], ticks=600)
        url = f'http://127.0.0.1:{port}'
        require(int(rpc(url, 'eth_chainId'), 16) == CHAIN_ID, 'the fork must report chain id 84532')
        return url

    def impersonate(url, address):
        rpc(url, 'anvil_impersonateAccount', [address])
        rpc(url, 'anvil_setBalance', [address, hex(10 ** 19)])

    def deploy(url, name, crate, contract, deployer, constructor, nonce, fork_signer):
        signer = ['--unlocked', '--from', deployer] if fork_signer else signer_flags(
            getattr(args, f'{name}_account'), getattr(args, f'{name}_password_file'))
        argv = ['forge', 'create', contract, '--rpc-url', url, '--broadcast', '--json', '--nonce', str(nonce),
                *signer, '--constructor-args', *constructor]
        env = dict(environment, FOUNDRY_OUT=str(out / f'forge-out-{Path(crate).name}'),
                   FOUNDRY_CACHE_PATH=str(out / f'forge-cache-{Path(crate).name}'))
        result = json.loads(command(f'{name}-deploy-{len(children)}', argv, ROOT / crate, env))
        receipt = rpc(url, 'eth_getTransactionReceipt', [result['transactionHash']])
        return result['deployedTo'].lower(), int(receipt['gasUsed'], 16), result['transactionHash']

    def code_hash(url, address):
        return ORDER.code_hash(rpc(url, 'eth_getCode', [address, 'latest']))

    def lock_args(predicted_verifier, token_code_hash):
        return [TOKEN, predicted_verifier, token_code_hash, str(data['plan']['max_lock_amount']),
                str(ORDER.MINIMUM_LOCK_AMOUNT)]

    driver = {'process': None}

    def handshake(name, value):
        partial = out / 'handshake' / f'{name}.partial'
        partial.write_text(json.dumps(value))
        partial.rename(out / 'handshake' / name)

    def wait_for(name, seconds=3600):
        path = out / 'handshake' / name
        for _ in range(seconds * 10):
            if path.exists():
                return json.loads(path.read_text())
            require(driver['process'] and driver['process'].poll() is None, f'the fleet driver exited before {name}')
            time.sleep(.1)
        raise RehearsalFailure(f'the fleet driver did not write {name}')

    def target_url():
        return data['fork_url'] if args.fork else BASE_SEPOLIA_RPC

    exit_code = 0
    try:
        for stage in STAGES:
            if stage in state['completed']:
                continue
            if stage in DRIVER_STAGES[1:] and 'fleet' in state['completed'] and driver['process'] is None:
                # Reconcile what a crashed run may have sent, so the record says it, then stop.
                chain = BaseChain(lambda method, params: rpc(target_url(), method, params))
                for intent in [i for i in state.get('intents', []) if i['outcome'] in (None, 'sent')]:
                    try:
                        if intent['stage'] == 'deploy':
                            role = intent['role']
                            verdict = reconcile_deployment(
                                intent, chain, predicted=data['plan'][role]['predicted'],
                                expected_code=data['preflight'][f'{role}_runtime_code_hash'])
                        elif intent['role'] == 'approve':
                            verdict = reconcile_approve(intent, chain, owner=HOLDER, spender=data['deploy']['lock'],
                                                        amount=LOCK_AMOUNT)
                        else:
                            verdict = reconcile_lock(intent, chain, lock=data['deploy']['lock'], holder=HOLDER,
                                                     amount=LOCK_AMOUNT, recipient_hash='0x' + keccak256(
                                                         data['fleet']['recipient'].encode()).hex())
                        intent['reconciled'] = {'adopt': 'mined and matches the plan',
                                                'send': 'not mined'}[verdict]
                    except RehearsalFailure as refusal:
                        intent['reconciled'] = f'refused: {refusal}'
                save()
                raise RehearsalFailure('the fleet driver from an earlier invocation is gone; fleet through export '
                                       'run in one invocation in this version (start a fresh output directory)')
            check_stage(state, stage)
            print(f'stage {stage}', file=sys.stderr)
            if stage == 'plan':
                data['program_identity'] = identity
                head = int(rpc(BASE_SEPOLIA_RPC, 'eth_blockNumber'), 16)
                gas_price = int(rpc(BASE_SEPOLIA_RPC, 'eth_gasPrice'), 16)
                sp1_code = ORDER.code_hash(rpc(BASE_SEPOLIA_RPC, 'eth_getCode', [SP1_VERIFIER['address'], 'latest']))
                require(sp1_code == SP1_VERIFIER['runtime_code_hash'], f'the SP1 verifier code hash is {sp1_code}')
                verifier_hash = rpc(BASE_SEPOLIA_RPC, 'eth_call', [{'to': SP1_VERIFIER['address'],
                                                                    'data': ORDER.call_data('VERIFIER_HASH()')}, 'latest'])
                require(verifier_hash == SP1_VERIFIER['verifier_hash'], 'the SP1 verifier hash differs')
                token_code_hash = ORDER.code_hash(rpc(BASE_SEPOLIA_RPC, 'eth_getCode', [TOKEN, 'latest']))
                if args.fork:
                    deployers = {r: '0x' + keccak256(os.urandom(32))[-20:].hex() for r in ('verifier', 'lock')}
                else:
                    require(args.verifier_deployer and args.lock_deployer, 'name --verifier-deployer and --lock-deployer')
                    deployers = {'verifier': args.verifier_deployer.lower(), 'lock': args.lock_deployer.lower()}
                nonces = {r: int(rpc(BASE_SEPOLIA_RPC, 'eth_getTransactionCount', [a, 'latest']), 16)
                          for r, a in deployers.items()}
                plan = ORDER.deployment_plan(verifier_deployer=deployers['verifier'], verifier_nonce=nonces['verifier'],
                                             lock_deployer=deployers['lock'], lock_nonce=nonces['lock'])
                fixture = json.loads((ROOT / ORDER.GENESIS_FIXTURE).read_text())
                data['plan'] = {**plan, 'fork_block': head, 'token': TOKEN, 'token_runtime_code_hash': token_code_hash,
                                'holder': HOLDER, 'max_lock_amount': ORDER.lane_window_cap(fixture),
                                'gas_price_wei': gas_price}
                # The lock's code hash: a probe on a throwaway fork (the lock holds the verifier's address).
                probe_url = start_fork('probe-fork', head)
                for role in ('verifier', 'lock'):
                    impersonate(probe_url, deployers[role])
                probe = ORDER.verifier_config(token=TOKEN, token_code_hash=token_code_hash,
                                              vault_code_hash='0x' + keccak256(b'probe').hex())
                deploy(probe_url, 'verifier', ORDER.VERIFIER_CRATE,
                       'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1', deployers['verifier'],
                       [ORDER.verifier_constructor_tuple(probe)], plan['verifier']['planned_nonce'], True)
                probe_lock, _, _ = deploy(probe_url, 'lock', ORDER.TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1',
                                          deployers['lock'], lock_args(plan['verifier']['predicted'], token_code_hash),
                                          plan['lock']['planned_nonce'], True)
                require(probe_lock == plan['lock']['predicted'], 'the probe lock landed off its predicted address')
                data['plan']['lock_runtime_code_hash'] = code_hash(probe_url, probe_lock)
                children.pop().terminate()
                genesis = ORDER.pin_lane_genesis(fixture, lock_address=probe_lock,
                                                 lock_code_hash=data['plan']['lock_runtime_code_hash'],
                                                 token_address=TOKEN, token_code_hash=token_code_hash)
                (out / 'lane-genesis-pins.json').write_text(json.dumps(ORDER.genesis_pins_excerpt(
                    genesis, hashlib.sha256(json.dumps(genesis).encode()).hexdigest()), indent=2) + '\n')
            elif stage == 'fleet':
                (out / 'handshake').mkdir(exist_ok=True)
                (out / 'tmp').mkdir(exist_ok=True, mode=0o700)
                if args.fork:
                    data['fork_url'] = start_fork('target-fork', data['plan']['fork_block'])
                config = {'rpc': target_url(), 'handshake_dir': str(out / 'handshake'),
                          'lock': data['plan']['lock']['predicted'],
                          'lock_code_hash': data['plan']['lock_runtime_code_hash'], 'token': TOKEN,
                          'token_code_hash': data['plan']['token_runtime_code_hash']}
                (out / 'driver-config.json').write_text(json.dumps(config, indent=2))
                driver['process'] = subprocess.Popen(
                    ['cargo', 'test', '--locked', '-p', 'postfiat-node', '--lib', 'local_evm_lane_roundtrip', '--',
                     '--ignored', '--nocapture'], cwd=ROOT,
                    env=dict(environment, POSTFIAT_LANE_ROUNDTRIP=str(out / 'driver-config.json'),
                             CARGO_TARGET_DIR=str(ROOT / 'target'), TMPDIR=str(out / 'tmp')),
                    stdout=(out / 'fleet-driver.stdout').open('w'), stderr=(out / 'fleet-driver.stderr').open('w'))
                children.append(driver['process'])
                ready = wait_for('pftl-ready.json')
                data['fleet'] = {'form': 'one data directory driven by the ignored node test local_evm_lane_roundtrip; '
                                         'six validators sign every block\'s consensus v2 commit; no separate node '
                                         'processes or network',
                                 **{k: ready[k] for k in ('chain_id', 'genesis_hash', 'checkpoint', 'pins', 'recipient')}}
                data['private'] = {'data_dir': ready['data_dir'], 'recipient_key_file': ready['recipient_key_file'],
                                   'protocol_version': ready['protocol_version']}
                verifier_config = RT.roundtrip_verifier_config(
                    sp1_verifier=SP1_VERIFIER['address'], pins=ready['pins'], checkpoint=ready['checkpoint'],
                    token=TOKEN, token_code_hash=data['plan']['token_runtime_code_hash'],
                    lock_code_hash=data['plan']['lock_runtime_code_hash'])
                verifier_config['programVKey'] = vkey
                data['verifier_config'] = verifier_config
                data['verifier_constructor_record'] = {
                    'programVKey': vkey, 'program_elf_sha256': identity['elf_sha256'],
                    'program_source_commit': identity['source_commit'], 'program_info': identity['program_info']}
            elif stage == 'preflight':
                fork = start_fork('preflight-fork', data['plan']['fork_block'])
                plan, gas = data['plan'], {}
                for role in ('verifier', 'lock'):
                    impersonate(fork, plan[role]['deployer'])
                verifier, gas['deploy-verifier'], _ = deploy(
                    fork, 'verifier', ORDER.VERIFIER_CRATE, 'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1',
                    plan['verifier']['deployer'], [ORDER.verifier_constructor_tuple(data['verifier_config'])],
                    plan['verifier']['planned_nonce'], True)
                lock, gas['deploy-lock'], _ = deploy(
                    fork, 'lock', ORDER.TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1', plan['lock']['deployer'],
                    lock_args(verifier, plan['token_runtime_code_hash']), plan['lock']['planned_nonce'], True)
                require((verifier, lock) == (plan['verifier']['predicted'], plan['lock']['predicted']),
                        'the fork deployments landed off the predicted addresses')
                require(code_hash(fork, lock) == plan['lock_runtime_code_hash'], 'the fork lock code hash differs')
                read = ORDER.check_constructor_values(
                    lambda address, sig: rpc(fork, 'eth_call', [{'to': address, 'data': ORDER.call_data(sig)},
                                                                'latest']).lower(),
                    verifier=verifier, verifier_values=data['verifier_config'], lock=lock,
                    lock_values={'token': TOKEN, 'finalityVerifier': verifier, 'maxLockAmount': plan['max_lock_amount'],
                                 'minimumLockAmount': ORDER.MINIMUM_LOCK_AMOUNT})
                impersonate(fork, HOLDER)
                for step, argv in (('approve', [TOKEN, 'approve(address,uint256)', lock, str(LOCK_AMOUNT)]),
                                   ('lock', [lock, 'lock(uint64,bytes)', str(LOCK_AMOUNT),
                                             '0x' + data['fleet']['recipient'].encode().hex()])):
                    receipt = json.loads(command(f'preflight-{step}', ['cast', 'send', *argv, '--rpc-url', fork,
                                                                       '--unlocked', '--from', HOLDER, '--json']))
                    require(receipt['status'] == '0x1', f'the fork {step} failed')
                    gas[step] = int(receipt['gasUsed'], 16)
                data['preflight'] = {'verifier_runtime_code_hash': code_hash(fork, verifier),
                                     'lock_runtime_code_hash': code_hash(fork, lock), 'gas_used': gas,
                                     'constructor_values_read_back': len(read), 'matches_predictions': True}
                sheet = deploy_sheet(plan={r: plan[r] for r in ('verifier', 'lock')},
                                     verifier_config=data['verifier_config'], gas_used=gas,
                                     gas_price_wei=plan['gas_price_wei'], program_identity=identity)
                (out / 'deploy-sheet.json').write_text(json.dumps(sheet, indent=2) + '\n')
                (out / 'deploy-sheet.md').write_text(sheet_markdown(sheet))
                children.pop().terminate()
            elif stage == 'deploy':
                url, plan = target_url(), data['plan']
                if args.fork:
                    for role in ('verifier', 'lock'):
                        impersonate(url, plan[role]['deployer'])
                else:
                    check_deployer_keystores(plan, lambda role: command(f'{role}-keystore-address', [
                        'cast', 'wallet', 'address', *signer_flags(getattr(args, f'{role}_account'),
                                                                   getattr(args, f'{role}_password_file'))]).strip())
                require_no_resume_acknowledged(args.i_accept_no_resume_between_deploy_and_export)
                expected_code = {'verifier': data['preflight']['verifier_runtime_code_hash'],
                                 'lock': data['preflight']['lock_runtime_code_hash']}
                chain = BaseChain(lambda method, params: rpc(url, method, params))
                intents, actions = {}, {}
                for role in ('verifier', 'lock'):
                    intents[role] = begin_intent(state, stage='deploy', role=role, sender=plan[role]['deployer'],
                                                 nonce=plan[role]['planned_nonce'],
                                                 purpose=f'deploy the {role} at {plan[role]["predicted"]}')
                    save()
                    actions[role] = reconcile_deployment(intents[role], chain, predicted=plan[role]['predicted'],
                                                         expected_code=expected_code[role])
                if all(action == 'send' for action in actions.values()):
                    check_planned_nonces(plan, chain.nonce)

                def send(role):
                    intent = intents[role]
                    if actions[role] == 'adopt':
                        intent['outcome'] = 'adopted after a rerun'
                        save()
                        return plan[role]['predicted'], intent['hash']
                    if role == 'verifier':
                        address, _, tx = deploy(url, 'verifier', ORDER.VERIFIER_CRATE,
                                                'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1',
                                                plan['verifier']['deployer'],
                                                [ORDER.verifier_constructor_tuple(data['verifier_config'])],
                                                plan['verifier']['planned_nonce'], args.fork)
                    else:
                        address, _, tx = deploy(url, 'lock', ORDER.TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1',
                                                plan['lock']['deployer'],
                                                lock_args(plan['verifier']['predicted'],
                                                          plan['token_runtime_code_hash']),
                                                plan['lock']['planned_nonce'], args.fork)
                    intent.update(hash=tx, receipt=rpc(url, 'eth_getTransactionReceipt', [tx]).get('status'),
                                  outcome='sent')
                    save()
                    return address, tx

                deployed = deploy_pair(plan, send=send, code_hash=lambda address: code_hash(url, address),
                                       expected_code=expected_code)
                verifier, lock = deployed['verifier']['address'], deployed['lock']['address']
                vtx, ltx = deployed['verifier']['transaction'], deployed['lock']['transaction']
                data['deploy'] = {'verifier': verifier, 'lock': lock, 'transactions': [vtx, ltx],
                                  'sourcify': 'skipped on the fork' if args.fork else 'see sourcify-*.stdout'}
                if not args.fork:
                    for name, address, crate, contract in (
                            ('verifier', verifier, ORDER.VERIFIER_CRATE, 'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1'),
                            ('lock', lock, ORDER.TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1')):
                        command(f'sourcify-{name}', ['forge', 'verify-contract', '--verifier', 'sourcify', '--chain',
                                                     str(CHAIN_ID), address, contract], ROOT / crate)
            elif stage == 'lock':
                url, lock = target_url(), data['deploy']['lock']
                if args.fork:
                    impersonate(url, HOLDER)
                    signer = ['--unlocked', '--from', HOLDER]
                else:
                    signer = signer_flags(args.holder_account, args.holder_password_file)
                chain = BaseChain(lambda method, params: rpc(url, method, params))
                recipient = data['fleet']['recipient']
                recipient_hash = '0x' + keccak256(recipient.encode()).hex()

                def send_async(name, argv, intent):
                    # The hash is recorded as soon as cast prints it, then the receipt.
                    intent['hash'] = command(name, ['cast', 'send', *argv, '--rpc-url', url, *signer,
                                                    '--async']).strip().split()[-1]
                    save()
                    receipt = json.loads(command(f'{name}-receipt', ['cast', 'receipt', intent['hash'],
                                                                     '--rpc-url', url, '--json']))
                    intent.update(receipt=receipt['status'], outcome='sent')
                    save()
                    require(receipt['status'] == '0x1', f'the {name} transaction failed')
                    return receipt

                approve = begin_intent(state, stage='lock', role='approve', sender=HOLDER, nonce=None,
                                       purpose=f'approve the lock for {LOCK_AMOUNT}')
                save()
                if reconcile_approve(approve, chain, owner=HOLDER, spender=lock, amount=LOCK_AMOUNT) == 'send':
                    send_async('approve', [TOKEN, 'approve(address,uint256)', lock, str(LOCK_AMOUNT)], approve)
                lock_intent = begin_intent(state, stage='lock', role='lock', sender=HOLDER, nonce=None,
                                           purpose=f'Lock {LOCK_AMOUNT} to {recipient}',
                                           next_nonce_before=chain.next_nonce(lock))
                save()
                if reconcile_lock(lock_intent, chain, lock=lock, holder=HOLDER, amount=LOCK_AMOUNT,
                                  recipient_hash=recipient_hash) == 'adopt':
                    lock_intent['outcome'] = 'adopted after a rerun'
                    log = chain.lock_log(lock, lock_intent['next_nonce_before'], data['plan']['fork_block'])
                    receipt = {'transactionHash': log['transactionHash'], 'blockNumber': log['blockNumber']}
                else:
                    receipt = send_async('lock', [lock, 'lock(uint64,bytes)', str(LOCK_AMOUNT),
                                                  '0x' + recipient.encode().hex()], lock_intent)
                data['lock'] = {'transaction': receipt['transactionHash'], 'block': int(receipt['blockNumber'], 16),
                                'amount': LOCK_AMOUNT, 'lock_nonce': lock_intent['next_nonce_before']}
            elif stage == 'release-on-pftl':
                if not args.fork:
                    # The first wait: Base finality of the Lock's block (about an hour on 2026-11-06).
                    while int(rpc(BASE_SEPOLIA_RPC, 'eth_getBlockByNumber', ['finalized', False])['number'], 16) \
                            < data['lock']['block']:
                        time.sleep(60)
                handshake('base-locked.json', {'transaction': data['lock']['transaction'], 'block': data['lock']['block']})
                released = wait_for('pftl-released.json', seconds=4 * 3600)
                require(released['recipient_balance_after_release'] == LOCK_AMOUNT, 'the release did not credit the Lock')
                data['release_on_pftl'] = {k: released[k] for k in ('release_height', 'recipient_balance_after_release',
                                                                     'checkpoint_votes', 'lane_after_release')}
            elif stage == 'export':
                released = json.loads((out / 'handshake/pftl-released.json').read_text())
                base_recipient = RT.fresh_base_address(f'base-sepolia-dry-run-{data["plan"]["fork_block"]}')
                signed = command('export-sign', [
                    args.node_bin, 'wallet-sign-asset-transaction', '--key-file', data['private']['recipient_key_file'],
                    '--chain-id', data['fleet']['chain_id'], '--genesis-hash', data['fleet']['genesis_hash'],
                    '--protocol-version', str(data['private']['protocol_version']), '--fee', '1000',
                    '--sequence', str(released['recipient_sequence'] + 1), '--operation-json',
                    json.dumps(RT.export_operation(sender=data['fleet']['recipient'], base_recipient=base_recipient,
                                                   amount=EXPORT_AMOUNT))])
                handshake('export-1.signed.json', json.loads(signed))
                applied = wait_for('export-1.applied.json')
                require(applied['accepted'] and applied['external_lane_export_root'], 'the export was not applied')
                command('relay-input', [sys.executable, ROOT / 'scripts/pft-lane-export-relay.py', 'input',
                                        '--node-bin', args.node_bin, '--data-dir', data['private']['data_dir'],
                                        '--lane-id', ORDER.LANE_ID, '--export-nonce', '0', '--prior-checkpoint',
                                        data['fleet']['checkpoint']['block_id'], '--output', out / 'relay-input'])
                document = json.loads((out / 'relay-input/expected-public-values.json').read_text())
                data['export'] = {'amount': EXPORT_AMOUNT, 'base_recipient': base_recipient,
                                  'block_height': applied['block_height'], 'lane': applied['lane'],
                                  'input_sha256': ORDER.sha256_file(out / 'relay-input/input.cbor'),
                                  'public_values_sha256': hashlib.sha256(
                                      bytes.fromhex(document['expected_public_values_hex'])).hexdigest(),
                                  'packet_digest': document['expected_public_values']['withdrawal_packet_digest']}
                driver['process'].terminate()
            elif stage == 'prove-and-release':
                if args.proof is None:
                    message = {
                        'stopped': 'prove-and-release needs a real SP1 v6.3.1 Groth16 proof (--proof); the SP1 '
                                   f'verifier {SP1_VERIFIER["address"]} accepts no placeholder',
                        'prover_command_contract': 'run with PFT_LANE_EXPORT_INPUT=<input.cbor> and '
                                                   'PFT_LANE_EXPORT_PROOF_OUT=<path>; write the raw proof bytes there '
                                                   'and the committed public values to <path>.public-values; exit 0 '
                                                   '(scripts/pft-lane-export-relay.py prove --mode command)',
                        'program_vkey': vkey, 'input_sha256': data['export']['input_sha256'],
                        'expected_public_values_sha256': data['export']['public_values_sha256']}
                    data['prove_and_release'] = message
                    print(json.dumps(message, indent=2))
                    exit_code = 3
                    save()
                    if args.through == stage:
                        break
                    continue
                require(not args.fork, 'a real proof is released on Base Sepolia, not on the fork')
                command('relay-release', [sys.executable, ROOT / 'scripts/pft-lane-export-relay.py', 'release',
                                          '--rpc', target_url(), '--lock', data['deploy']['lock'], '--input',
                                          out / 'relay-input', '--proof', args.proof,
                                          *signer_flags(args.holder_account, args.holder_password_file),
                                          '--expect-program-vkey', vkey, '--expect-sp1-verifier',
                                          SP1_VERIFIER['address']])
            elif stage == 'record':
                summary = {'mode': state['mode'], 'public_network_transaction': not args.fork,
                           'proof_network_request': False, **{k: v for k, v in data.items() if k != 'private'},
                           'sp1_verifier': SP1_VERIFIER, 'program_info': program_info}
                text = RT.portable_text(json.dumps(summary, indent=2), out=out, root=ROOT, roles=roles, secrets=[])
                (out / 'summary.json').write_text(text + '\n')
                manifest = {'source_commit': ORDER.git('rev-parse', 'HEAD'),
                            'source_tree': ORDER.git('rev-parse', 'HEAD^{tree}'),
                            'tools': {t: ORDER.tool_version(t) for t in ('anvil', 'forge', 'cast', 'cargo')},
                            'elf_sha256': ORDER.sha256_file(args.elf) if args.elf else None,
                            'elf_note': 'checked against the program info in plan',
                            'evidence_files': {n: ORDER.sha256_file(out / n) for n in
                                               ('summary.json', 'deploy-sheet.json', 'deploy-sheet.md')}}
                (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
            state['completed'].append(stage)
            save()
            if stage == args.through:
                break
    finally:
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill()
        save()
    return exit_code


BASE_LOCK_START = _load('rehearse_base_lock', 'scripts/rehearse-base-lock.py').start_on_free_port


if __name__ == '__main__':
    try:
        sys.exit(main())
    except RehearsalFailure as refusal:
        print(f'rehearse-lane-base-sepolia: {refusal}', file=sys.stderr)
        sys.exit(2)
