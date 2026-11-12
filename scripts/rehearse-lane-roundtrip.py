#!/usr/bin/env python3
"""Rehearse one full PFT lane round trip on local chains (design note, "Base half, step 4").

ERC-20 PFT locked on a local Anvil (chain id 84532) becomes native PFT on a
generated six-validator PFTL chain, and native PFT exported back is paid out of
the same lock through the real PFTLockV1 and PFTLFinalityVerifierV1.

Real: the contracts, the PFTL chain with consensus v2 finality signed by all six
validators, the Lock receipt path (observe, five signatures, assemble, prove,
release), the export, the witness check and the guest run in SP1 execute mode.
NOT real: the SP1 proof. The verifier is constructed with the Foundry tests'
mock SP1 verifier (Tier4MockSP1Verifier) and a placeholder program key, and
accepts the relay's placeholder bytes; no proof is generated. Anvil stands in
for Base, and the guest ELF is a local build without a committed key.

Owns a disposable loopback Anvil. Never accepts a remote URL or a key; the
Anvil keys are generated at run time and never written to the record.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
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


# The two rehearsal scripts and the relay, imported, not copied.
ORDER = _load('rehearse_phase3_deployment_order', 'scripts/rehearse-phase3-deployment-order.py')
BASE_LOCK = _load('rehearse_base_lock', 'scripts/rehearse-base-lock.py')
RELAY = _load('pft_lane_export_relay', 'scripts/pft-lane-export-relay.py')
keccak256 = ORDER.keccak256
require = ORDER.require
RehearsalFailure = ORDER.RehearsalFailure

CHAIN_ID = ORDER.CHAIN_ID
LANE_ID = ORDER.LANE_ID
LOCK_AMOUNT = 1_000_000
EXPORTS = (400_000, 600_000)
OVER_EXPORT = 1
EXPORT_FEE = 1_000
MOCK_SP1 = ('crates/ethereum-contracts', 'test/PFUSDCTier4.t.sol:Tier4MockSP1Verifier')
PLACEHOLDER_PROGRAM_VKEY = '0x' + keccak256(b'pft-lane-export placeholder program key: no ELF or key is committed').hex()
SOURCE_FILES = (
    'scripts/rehearse-lane-roundtrip.py',
    'scripts/pft-lane-export-relay.py',
    'scripts/rehearse-phase3-deployment-order.py',
    'scripts/rehearse-base-lock.py',
    'crates/node/src/tests/external_lane_roundtrip_tests.rs',
    'crates/node/src/external_lane_export_proof.rs',
    'crates/pft_lane_proofs/src/lib.rs',
    'programs/pft-lane-export/src/main.rs',
    'crates/pft-token-contracts/src/PFTLockV1.sol',
    'crates/pft-token-contracts/src/PFTToken.sol',
    'crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol',
    'python/tests/test_rehearse_lane_roundtrip.py',
)
PUBLISHED = ('summary.json', 'manifest.json')


PROGRAM_INFO = ROOT / 'programs/pft-lane-export/program-info.json'


def elf_note(elf_sha256, program_info=PROGRAM_INFO):
    """What ELF the run used: the committed release build when the sha256 matches, a local build otherwise."""
    committed = json.loads(Path(program_info).read_text())['elf_sha256']
    if elf_sha256 == committed:
        return (f'the committed reproducible release ELF sha256:{committed} '
                f'(programs/pft-lane-export/program-info.json), used in execute mode only; no key is committed')
    return (f'a local build sha256:{elf_sha256}, not the committed reproducible release ELF sha256:{committed} '
            f'(programs/pft-lane-export/program-info.json), used in execute mode only; no key is committed')


def fresh_base_address(label):
    """A Base address nobody holds a key for, derived from a label."""
    return '0x' + keccak256(b'postfiat.lane-roundtrip.base-recipient\x00' + label.encode())[-20:].hex()


# An absolute path not already named by a role: a '/' that does not follow a
# word character, ':', '/', '<' or '>' (so URLs, relative paths and '<output>/x'
# stay), up to the end of the path.
_ABSOLUTE_PATH = re.compile(r'(?<![\w:/<>.~-])/[^\s"\',]*')


def portable_text(text, *, out, root, roles, secrets):
    """`text` without key material or local layout: secrets become
    `<disposable-key>`, the paths in `roles` their role names, the output
    directory `<output>`, the checkout `<checkout>`, and any other absolute
    path `<path-outside-checkout>`."""
    for secret in secrets:
        text = text.replace(secret, '<disposable-key>')
    for path, role in sorted(roles.items(), key=lambda item: -len(item[0])):
        text = text.replace(str(path), role)
    text = text.replace(str(out), '<output>').replace(str(root), '<checkout>')
    return _ABSOLUTE_PATH.sub('<path-outside-checkout>', text)


def export_watch_summary(lines, *, paid, alarms=0, alarm_codes=()):
    """The watcher's `--exports` run: one summary line with `alarms` alarms of
    `alarm_codes`, and `paid` export lines, every one of them `paid`."""
    summaries = [line for line in lines if line.get('kind') == 'summary']
    require(len(summaries) == 1, f'the watcher must print one summary line, printed {len(summaries)}')
    summary = summaries[0]
    require(summary.get('alarms') == alarms, f'the watcher raised {summary.get("alarms")} alarms, expected {alarms}')
    raised = sorted(line['code'] for line in lines if line.get('kind') == 'alarm')
    require(sorted(raised) == sorted(alarm_codes), f'the watcher raised {raised}, expected {sorted(alarm_codes)}')
    exports = [line for line in lines if line.get('kind') == 'export']
    require(len(exports) == paid and all(line.get('status') == 'paid' for line in exports),
            f'the watcher must report {paid} paid exports, reported {[line.get("status") for line in exports]}')
    return {'summary': summary, 'exports': exports,
            'payouts': [line for line in lines if line.get('kind') == 'payout'],
            'alarms': [line for line in lines if line.get('kind') == 'alarm']}


def forged_public_values(public_values_hex, values, fresh):
    """Public values for a packet PFTL never recorded: the export's own, with its
    withdrawal id, burn id, packet digest and nullifier replaced by `fresh`
    bytes of the same lengths (the mock SP1 verifier accepts any proof)."""
    forged = public_values_hex
    for field in ('withdrawal_id', 'burn_tx_id', 'withdrawal_packet_digest', 'proof_nullifier'):
        old = values[field].removeprefix('0x')
        require(forged.count(old) == 1, f'{field} does not occur once in the public values')
        forged = forged.replace(old, fresh(len(old) // 2).hex())
    return forged


# The driver (cargo, and the test binary it runs) starts in its own session so that
# stopping it stops the whole group: terminating cargo alone leaves the binary running.
DRIVER_POPEN = {'start_new_session': True}


def stop_driver(process):
    """Stops the driver's whole process group, then reaps cargo."""
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(process.pid, sig)
        except ProcessLookupError:
            break
        try:
            process.wait(timeout=10)
            break
        except subprocess.TimeoutExpired:
            continue


def roundtrip_verifier_config(*, sp1_verifier, pins, checkpoint, token, token_code_hash, lock_code_hash):
    """The verifier's constructor values: the genesis-owned pins, the first
    finalized PFTL checkpoint, the mock SP1 verifier and the placeholder key."""
    config = {
        'sp1Verifier': sp1_verifier,
        'programVKey': PLACEHOLDER_PROGRAM_VKEY,
        'pftlChainIdHash': pins['pftl_chain_id_hash'],
        'pftlGenesisHashCommitment': pins['pftl_genesis_hash_commitment'],
        'pftlProtocolVersion': 1,
        'routeProfileHashCommitment': pins['route_profile_hash_commitment'],
        'routeEpoch': pins['route_profile']['route_epoch'],
        'assetIdCommitment': pins['asset_id_commitment'],
        'arbitrumChainId': CHAIN_ID,
        'vaultRuntimeCodeHash': lock_code_hash,
        'token': token,
        'tokenRuntimeCodeHash': token_code_hash,
        'maxProofBytes': 4096,
        'maxPublicValuesBytes': 16384,
        'initialCheckpointCommitment': RELAY.commitment(checkpoint['block_id']),
        'initialFinalizedHeight': checkpoint['height'],
        'initialCommitteeRootCommitment': RELAY.commitment(checkpoint['committee_root']),
    }
    require(set(config) == set(ORDER.VERIFIER_CONFIG_FIELDS), 'the verifier config must name every Config field')
    return config


def export_operation(*, sender, base_recipient, amount):
    """The `--operation-json` of `wallet-sign-asset-transaction` for one export."""
    return {'operation': 'external_lane_export_v1', 'sender': sender, 'lane_id': LANE_ID,
            'amount_atoms': amount, 'base_recipient': base_recipient}


def check_base_conservation(*, lock_balance, total_locked, total_released):
    """`token.balanceOf(lock) == totalLocked - totalReleased`."""
    require(lock_balance == total_locked - total_released,
            f'Base: the lock holds {lock_balance}, but totalLocked - totalReleased is {total_locked - total_released}')
    return {'lock_balance': lock_balance, 'total_locked': total_locked, 'total_released': total_released}


def check_lane_conservation(lane):
    """The lane's remainder is reserved - (released - exported), and the step replayed."""
    expected = lane['reserved_atoms'] - (lane['released_atoms'] - lane['exported_atoms'])
    require(lane['remainder_atoms'] == expected, f'PFTL: remainder {lane["remainder_atoms"]}, expected {expected}')
    require(lane.get('blocks_and_state_replayed') is True, 'PFTL: the step was not replayed')
    return lane


def check_cross_chain(lane, base):
    """Locked on Base minus released on Base equals released on PFTL minus exported."""
    require(base['total_locked'] - base['total_released'] == lane['released_atoms'] - lane['exported_atoms'],
            'the lock\'s outstanding balance differs from the PFTL lane\'s net released atoms')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--output', type=Path, required=True, help='a fresh directory for the run artifacts')
    parser.add_argument('--elf', type=Path, required=True, help='a local build of programs/pft-lane-export')
    parser.add_argument('--prover-tool', type=Path, required=True, help='tools/pft-lane-export-prover, built')
    parser.add_argument('--node-bin', type=Path, help="a built postfiat-node; default: this checkout's ./target")
    args = parser.parse_args(argv)
    out = ORDER.confined_output(args.output)
    commit, tree = ORDER.clean_source()
    out.mkdir(parents=True, mode=0o700)
    hand = out / 'handshake'
    hand.mkdir()
    environment = os.environ.copy()
    results = {'scope': 'local Anvil (chain id 84532) and a generated six-validator PFTL chain only',
               'public_network_transaction': False, 'proof_network_request': False,
               'source_commit': commit, 'source_tree': tree, 'started_at_unix': int(time.time()),
               'commands': [], 'steps': {}, 'passed': False,
               'not_real': ['SP1 proof: a mock SP1 verifier accepted placeholder bytes; no proof was generated',
                            'Base: a local Anvil with chain id 84532',
                            'guest ELF: a local build; no ELF or key is committed']}
    secrets, children, drivers = [], [], []
    state = {'url': None, 'data_dir': None}

    roles = {}

    def portable(value):
        return portable_text(str(value), out=out, root=ROOT, roles=roles, secrets=secrets)

    def command(name, argv, cwd=ROOT, env=None, expected=0):
        began = time.time()
        with (out / f'{name}.stdout').open('w') as stdout, (out / f'{name}.stderr').open('w') as stderr:
            p = subprocess.run([str(a) for a in argv], cwd=cwd, env=env or environment, stdout=stdout,
                               stderr=stderr, text=True, timeout=3600)
        results['commands'].append({'name': name, 'argv': [portable(a) for a in argv], 'exit_code': p.returncode,
                                    'seconds': round(time.time() - began, 3)})
        if p.returncode != expected:
            raise RehearsalFailure(f'{name}: exit {p.returncode}, expected {expected}; see {out / (name + ".stderr")}')
        return (out / f'{name}.stdout').read_text()

    def rpc(method, params=()):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode()
        request = urllib.request.Request(state['url'], data=body, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=10) as response:
            value = json.load(response)
        if 'error' in value:
            raise RehearsalFailure(f'local Anvil refused {method}: {value["error"]}')
        return value['result']

    def call_int(address, signature, *args):
        return int(rpc('eth_call', [{'to': address, 'data': ORDER.call_data(signature, *args)}, 'latest']), 16)

    def wait_for(name, seconds=1200):
        path = hand / name
        for _ in range(seconds * 10):
            if path.exists():
                return json.loads(path.read_text())
            require(driver.poll() is None, f'the PFTL driver exited before writing {name}; see pftl-driver.stderr')
            time.sleep(.1)
        raise RehearsalFailure(f'the PFTL driver did not write {name}')

    def handshake(name, value):
        partial = hand / f'{name}.partial'
        partial.write_text(json.dumps(value))
        partial.rename(hand / name)

    def base_state(token, lock):
        return check_base_conservation(lock_balance=call_int(token, 'balanceOf(address)', lock),
                                       total_locked=call_int(lock, 'totalLocked()'),
                                       total_released=call_int(lock, 'totalReleased()'))

    driver = None
    try:
        toolchain = {tool: ORDER.tool_version(tool) for tool in ('anvil', 'forge', 'cast', 'cargo', 'rustc')}
        toolchain['elf_sha256'] = ORDER.sha256_file(args.elf)
        roles[str(args.elf.resolve())] = f'<elf sha256:{toolchain["elf_sha256"]}>'
        roles[str(args.elf)] = roles[str(args.elf.resolve())]
        roles[str(args.prover_tool.resolve())] = f'<prover-tool sha256:{ORDER.sha256_file(args.prover_tool)}>'
        roles[sys.executable] = '<python>'
        toolchain['elf_note'] = elf_note(toolchain['elf_sha256'])
        toolchain['prover_tool_sha256'] = ORDER.sha256_file(args.prover_tool)
        forge_env = {}
        for crate in (ORDER.TOKEN_CRATE, ORDER.VERIFIER_CRATE):
            tag = Path(crate).name
            forge_env[crate] = dict(environment, FOUNDRY_OUT=str(out / f'forge-out-{tag}'),
                                    FOUNDRY_CACHE_PATH=str(out / f'forge-cache-{tag}'),
                                    FOUNDRY_BROADCAST=str(out / f'forge-broadcast-{tag}'))
            command(f'forge-build-{tag}', ['forge', 'build'], ROOT / crate, forge_env[crate])
        node_bin = (args.node_bin or ROOT / 'target/debug/postfiat-node').resolve()
        require(node_bin.is_file(), f'{node_bin} is not built; cargo build -p postfiat-node --bin postfiat-node')
        toolchain['postfiat_node_sha256'] = ORDER.sha256_file(node_bin)
        results['toolchain'] = toolchain

        # A fresh loopback Anvil, started the base-lock rehearsal's way.
        def anvil_ready(port):
            state['url'] = f'http://127.0.0.1:{port}'
            try:
                return 'anvil' in str(rpc('web3_clientVersion')).lower()
            except OSError:
                return False

        port, _ = BASE_LOCK.start_on_free_port(
            'anvil', lambda port: ['anvil', '--host', '127.0.0.1', '--port', port, '--chain-id', str(CHAIN_ID),
                                   '--silent'], anvil_ready, children, results.setdefault('children', []), ticks=100)
        url = state['url'] = f'http://127.0.0.1:{port}'
        require(int(rpc('eth_chainId'), 16) == CHAIN_ID, f'the local Anvil must report chain id {CHAIN_ID}')

        def account(role):
            key = '0x' + os.urandom(32).hex()
            secrets.append(key)
            address = command(f'account-{role}', ['cast', 'wallet', 'address', '--private-key', key]).strip().lower()
            rpc('anvil_setBalance', [address, hex(10 ** 20)])
            return key, address

        keys = {role: account(role) for role in ('token', 'verifier', 'lock', 'holder', 'relayer')}

        def deploy(name, crate, contract, role, constructor=(), nonce=None):
            pin = [] if nonce is None else ['--nonce', str(nonce)]
            argv = ['forge', 'create', contract, '--rpc-url', url, '--private-key', keys[role][0], '--broadcast',
                    '--json', *pin]
            if constructor:
                argv += ['--constructor-args', *constructor]
            return json.loads(command(name, argv, ROOT / crate, forge_env[crate]))['deployedTo'].lower()

        fixture = json.loads((ROOT / ORDER.GENESIS_FIXTURE).read_text())
        holder = keys['holder'][1]
        token = deploy('deploy-token', ORDER.TOKEN_CRATE, 'src/PFTToken.sol:PFTToken', 'token',
                       ['Rehearsal PFT', 'rPFT', str(ORDER.lane(fixture)['reserved_atoms']), holder])
        token_code_hash = ORDER.code_hash(rpc('eth_getCode', [token, 'latest']))
        mock_sp1 = deploy('deploy-mock-sp1', MOCK_SP1[0], MOCK_SP1[1], 'token')

        # Predict both addresses and the lock's code hash (the deployment-order rehearsal's plan).
        def nonces(address):
            return {tag: int(rpc('eth_getTransactionCount', [address, tag]), 16) for tag in ('latest', 'pending')}

        verifier_deployer, lock_deployer = keys['verifier'][1], keys['lock'][1]
        current = {verifier_deployer: nonces(verifier_deployer), lock_deployer: nonces(lock_deployer)}
        plan = ORDER.deployment_plan(verifier_deployer=verifier_deployer,
                                     verifier_nonce=current[verifier_deployer]['latest'],
                                     lock_deployer=lock_deployer, lock_nonce=current[lock_deployer]['latest'])
        ORDER.refuse_unless_plan_intact(plan, current)
        predicted_verifier, predicted_lock = plan['verifier']['predicted'], plan['lock']['predicted']
        max_lock_amount = ORDER.lane_window_cap(fixture)
        lock_args = [token, predicted_verifier, token_code_hash, str(max_lock_amount), str(ORDER.MINIMUM_LOCK_AMOUNT)]
        snapshot = rpc('evm_snapshot')
        probe = ORDER.verifier_config(token=token, token_code_hash=token_code_hash,
                                      vault_code_hash='0x' + keccak256(b'probe').hex())
        deploy('probe-verifier', ORDER.VERIFIER_CRATE, 'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1',
               'verifier', [ORDER.verifier_constructor_tuple(probe)], nonce=plan['verifier']['planned_nonce'])
        probe_lock = deploy('probe-lock', ORDER.TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1', 'lock', lock_args,
                            nonce=plan['lock']['planned_nonce'])
        require(probe_lock == predicted_lock, f'probe lock at {probe_lock}, predicted {predicted_lock}')
        lock_code_hash = ORDER.code_hash(rpc('eth_getCode', [probe_lock, 'latest']))
        require(rpc('evm_revert', [snapshot]) is True, 'evm_revert failed')
        results['steps']['base_predicted'] = {
            'token': token, 'token_runtime_code_hash': token_code_hash, 'mock_sp1_verifier': mock_sp1,
            'mock_sp1_contract': MOCK_SP1[1], 'predicted_verifier': predicted_verifier, 'predicted_lock': predicted_lock,
            'predicted_lock_runtime_code_hash': lock_code_hash, 'deployers': {r: keys[r][1] for r in keys}}

        # The PFTL chain from a genesis that pins the predicted lock.
        config = {'rpc': url, 'handshake_dir': str(hand), 'lock': predicted_lock, 'lock_code_hash': lock_code_hash,
                  'token': token, 'token_code_hash': token_code_hash}
        (out / 'driver-config.json').write_text(json.dumps(config, indent=2))
        # The generated chain lives under the output directory, never in the system temp directory.
        (out / 'tmp').mkdir(mode=0o700)
        driver_env = dict(environment, POSTFIAT_LANE_ROUNDTRIP=str(out / 'driver-config.json'),
                          CARGO_TARGET_DIR=str(ROOT / 'target'), TMPDIR=str(out / 'tmp'))
        driver_out = (out / 'pftl-driver.stdout').open('w')
        driver = subprocess.Popen(['cargo', 'test', '--locked', '-p', 'postfiat-node', '--lib', 'local_evm_lane_roundtrip',
                                   '--', '--ignored', '--nocapture'], cwd=ROOT, env=driver_env, stdout=driver_out,
                                  stderr=(out / 'pftl-driver.stderr').open('w'), **DRIVER_POPEN)
        children.append(driver)
        drivers.append(driver)
        ready = wait_for('pftl-ready.json', seconds=2400)
        state['data_dir'] = Path(ready['data_dir'])
        pins = ready['pins']
        results['steps']['pftl_started'] = {k: ready[k] for k in ('chain_id', 'genesis_hash', 'validators',
                                                                   'checkpoint', 'pins', 'lane')}

        # Deploy the verifier with the pins and the first checkpoint, then the lock, at the planned nonces.
        verifier_values = roundtrip_verifier_config(sp1_verifier=mock_sp1, pins=pins, checkpoint=ready['checkpoint'],
                                                    token=token, token_code_hash=token_code_hash,
                                                    lock_code_hash=lock_code_hash)
        ORDER.refuse_unless_plan_intact(plan, {a: nonces(a) for a in (verifier_deployer, lock_deployer)})
        verifier = deploy('deploy-verifier', ORDER.VERIFIER_CRATE,
                          'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1', 'verifier',
                          [ORDER.verifier_constructor_tuple(verifier_values)], nonce=plan['verifier']['planned_nonce'])
        lock = deploy('deploy-lock', ORDER.TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1', 'lock', lock_args,
                      nonce=plan['lock']['planned_nonce'])
        require((verifier, lock) == (predicted_verifier, predicted_lock), 'a contract landed off its predicted address')
        require(ORDER.code_hash(rpc('eth_getCode', [lock, 'latest'])) == lock_code_hash, 'lock code hash differs')

        def call_word(address, signature):
            return rpc('eth_call', [{'to': address, 'data': ORDER.call_data(signature)}, 'latest']).lower()

        read_back = ORDER.check_constructor_values(
            call_word, verifier=verifier, verifier_values=verifier_values, lock=lock,
            lock_values={'token': token, 'finalityVerifier': verifier, 'maxLockAmount': max_lock_amount,
                         'minimumLockAmount': ORDER.MINIMUM_LOCK_AMOUNT})
        results['steps']['base_deployed'] = {'verifier': verifier, 'lock': lock, 'verifier_values': verifier_values,
                                             'program_vkey_note': 'placeholder; no ELF or key is committed',
                                             'constructor_values_read_back': read_back,
                                             'base': base_state(token, lock)}

        # (a) The holder approves and locks 1,000,000 units to the PFTL recipient.
        recipient = ready['recipient']
        holder_key = keys['holder'][0]
        command('holder-approve', ['cast', 'send', token, 'approve(address,uint256)', lock, str(LOCK_AMOUNT),
                                   '--rpc-url', url, '--private-key', holder_key])
        locked = json.loads(command('holder-lock', ['cast', 'send', lock, 'lock(uint64,bytes)', str(LOCK_AMOUNT),
                                                    '0x' + recipient.encode().hex(), '--rpc-url', url, '--private-key',
                                                    holder_key, '--json']))
        require(locked['status'] == '0x1', 'the Lock transaction failed')
        base = base_state(token, lock)
        require(base['total_locked'] == LOCK_AMOUNT, 'totalLocked is not the Lock amount')
        locked_block = int(locked['blockNumber'], 16)
        results['steps']['a_lock'] = {'transaction': locked['transactionHash'], 'block': int(locked['blockNumber'], 16),
                                      'amount': LOCK_AMOUNT, 'pftl_recipient': recipient, 'base': base}

        # (b) The four checkpoint commands and the release on PFTL (the driver).
        handshake('base-locked.json', {'transaction': locked['transactionHash'],
                                       'block': int(locked['blockNumber'], 16)})
        released = wait_for('pftl-released.json')
        require(released['recipient_balance_after_release'] == LOCK_AMOUNT, 'the recipient does not hold the release')
        check_lane_conservation(released['lane_after_release'])
        check_cross_chain(released['lane'], base)
        results['steps']['b_release_on_pftl'] = released

        # A read-only PFTL RPC for the watcher, kept for the rest of the run.
        ready_file = out / 'rpc-ready.json'
        watch_env = dict(environment, PYTHONPATH=str(ROOT / 'python'))
        rpc_port, _ = BASE_LOCK.start_on_free_port(
            'rpc-serve', lambda port: [node_bin, 'rpc-serve', '--unsafe-devnet-json-storage', '--data-dir',
                                       state['data_dir'], '--bind-host', '127.0.0.1', '--port', port,
                                       '--max-requests', '10000', '--ready-file', ready_file],
            lambda port: ready_file.exists(), children, results['children'],
            open_stderr=lambda: (out / 'rpc-serve.stderr').open('a'), env=watch_env, ready_file=ready_file)

        def export_watch(name, expected=0):
            # Released events count only once finalized: mine past Anvil's finality depth first.
            rpc('anvil_mine', ['0x80'])
            argv = [sys.executable, '-m', 'postfiat_rpc.external_lane_watcher', '--pftl-rpc',
                    f'127.0.0.1:{rpc_port}', '--lane-id', LANE_ID, '--base-rpc', url, '--once', '--exports',
                    '--lock-from-block', '0', '--unpaid-warning-blocks', '64']
            return [json.loads(line) for line in command(name, argv, env=watch_env, expected=expected).splitlines()]

        prior = ready['checkpoint']['block_id']
        sequence = released['recipient_sequence']
        relay = [sys.executable, ROOT / 'scripts/pft-lane-export-relay.py']
        key_file = out / 'relayer.key'
        key_file.write_text(keys['relayer'][0])
        key_file.chmod(0o600)
        outcomes = []
        for index, amount in enumerate(EXPORTS, start=1):
            base_recipient = fresh_base_address(f'export-{index}')
            # (c)/(f) The recipient signs the export with wallet-sign-asset-transaction.
            sequence += 1
            signed = command(f'export-{index}-sign', [
                node_bin, 'wallet-sign-asset-transaction', '--key-file', ready['recipient_key_file'],
                '--chain-id', ready['chain_id'], '--genesis-hash', ready['genesis_hash'],
                '--protocol-version', str(ready['protocol_version']), '--fee', str(EXPORT_FEE),
                '--sequence', str(sequence), '--operation-json',
                json.dumps(export_operation(sender=recipient, base_recipient=base_recipient, amount=amount))])
            handshake(f'export-{index}.signed.json', json.loads(signed))
            applied = wait_for(f'export-{index}.applied.json')
            require(applied['accepted'] and applied['external_lane_export_root'], f'export {index} not in a rooted block')
            check_lane_conservation(applied['lane'])

            # (d) The relay: input, prove (execute-only), release.
            work = out / f'relay-{index}'
            command(f'relay-{index}-input', [*relay, 'input', '--node-bin', node_bin, '--data-dir', state['data_dir'],
                                             '--lane-id', LANE_ID, '--export-nonce', str(index - 1),
                                             '--prior-checkpoint', prior, '--output', work / 'input'])
            proof = json.loads(command(f'relay-{index}-prove', [*relay, 'prove', '--input', work / 'input', '--mode',
                                                                'execute-only', '--elf', args.elf.resolve(),
                                                                '--prover-tool', args.prover_tool.resolve(),
                                                                '--output', work / 'proof']))
            values = json.loads((work / 'input/expected-public-values.json').read_text())['expected_public_values']
            release_argv = [*relay, 'release', '--rpc', url, '--lock', lock, '--input', work / 'input',
                            '--proof', work / 'proof/proof.json', '--private-key-file', key_file,
                            '--mock-sp1-artifact',
                            out / 'forge-out-ethereum-contracts/PFUSDCTier4.t.sol/Tier4MockSP1Verifier.json']
            sent = json.loads(command(f'relay-{index}-release', release_argv))
            base = base_state(token, lock)
            require(call_int(token, 'balanceOf(address)', base_recipient) == amount, 'the Base recipient was not paid')
            require(not sent['warnings'], f'the relay warned: {sent["warnings"]}')
            require(sent['released']['packet_digest'] == RELAY.word_hex(values['withdrawal_packet_digest']),
                    'Released names another packet digest')
            check_cross_chain(applied['lane'], base)
            outcome = {'amount': amount, 'base_recipient': base_recipient, 'pftl_export': applied,
                       'packet_digest': values['withdrawal_packet_digest'],
                       'public_values_sha256': proof['public_values_sha256'],
                       'guest_instruction_count': proof['instruction_count'], 'proof_mode': proof['mode'],
                       'proof_is_placeholder': proof['mock_verifier_only'], 'release': sent, 'base': base,
                       'status': json.loads(command(f'relay-{index}-status', [
                           *relay, 'status', '--rpc', url, '--lock', lock, '--input', work / 'input',
                           '--proof', work / 'proof/proof.json', '--from-block', str(locked_block)]))}
            if index == 1:
                # (e) The same packet again: refused before sending; forced, it reverts (in the gas estimate).
                command('relay-1-release-again', release_argv, expected=2)
                again = (out / 'relay-1-release-again.stdout').read_text()
                require('already_consumed' in again, 'the second release was not refused as already consumed')
                command('relay-1-release-forced', [*release_argv, '--force-send'], expected=2)
                forced = (out / 'relay-1-release-forced.stdout').read_text()
                forced_result = json.loads(forced)
                require(forced_result.get('reverted') is True, 'the forced second release did not revert')
                require(base_state(token, lock) == base, 'the refused release moved Base state')
                outcome['second_release'] = {'refused_before_sending': 'already_consumed',
                                             'forced_send_reverted': True, 'forced_send_sent': forced_result['sent'],
                                             'forced_send_failure': forced_result['failure']}
            outcome['watcher'] = export_watch_summary(export_watch(f'watcher-exports-{index}'), paid=index)
            if index == 1:
                # (4) The unsound-verifier case on a snapshot: a payout for a packet PFTL never recorded.
                snapshot = rpc('evm_snapshot')
                forged = forged_public_values(json.loads((work / 'input/expected-public-values.json').read_text())
                                              ['expected_public_values_hex'], values, os.urandom)
                forced = json.loads(command('forged-release', [
                    'cast', 'send', lock, 'release(bytes,bytes)', '0x' + forged,
                    '0x' + RELAY.PLACEHOLDER_PROOF.hex(), '--rpc-url', url, '--private-key', keys['relayer'][0],
                    '--json']))
                require(forced['status'] == '0x1', 'the forged release did not pay')
                caught = export_watch_summary(
                    export_watch('watcher-exports-forged', expected=2), paid=1, alarms=2,
                    alarm_codes=('payout_without_export', 'exported_below_paid'))
                require(rpc('evm_revert', [snapshot]) is True, 'evm_revert failed')
                require(base_state(token, lock) == base, 'the snapshot revert did not restore the lock')
                outcome['forged_payout'] = {'transaction': forced['transactionHash'],
                                            'alarms': caught['alarms'], 'reverted': True}
            prior = values['resulting_checkpoint_block_id']
            outcomes.append(outcome)
            results['steps'][f'export_{index}'] = outcome

        # (f) The round is empty on both chains.
        final_lane = outcomes[-1]['pftl_export']['lane']
        require((final_lane['released_atoms'], final_lane['exported_atoms']) == (LOCK_AMOUNT, LOCK_AMOUNT),
                'the lane did not export everything it released')
        base = base_state(token, lock)
        require(base['total_locked'] - base['total_released'] == 0 and base['lock_balance'] == 0,
                'the lock still holds tokens')
        # (g) A third export of one atom is refused on PFTL.
        sequence += 1
        signed = command('export-3-sign', [
            node_bin, 'wallet-sign-asset-transaction', '--key-file', ready['recipient_key_file'],
            '--chain-id', ready['chain_id'], '--genesis-hash', ready['genesis_hash'],
            '--protocol-version', str(ready['protocol_version']), '--fee', str(EXPORT_FEE),
            '--sequence', str(sequence), '--operation-json',
            json.dumps(export_operation(sender=recipient, base_recipient=fresh_base_address('export-3'),
                                        amount=OVER_EXPORT))])
        handshake('export-3.signed.json', json.loads(signed))
        refused = wait_for('export-3.applied.json')
        require(not refused['accepted'] and 'external_lane_export_above_released' in refused['mempool_error'],
                'the over-export was not refused')
        done = wait_for('pftl-done.json')
        require(driver.wait(timeout=600) == 0, 'the PFTL driver test failed; see pftl-driver.stdout')
        results['steps']['f_round_empty'] = {'lane': done['lane'], 'base': base}
        results['steps']['g_over_export_refused'] = refused

        # (i) The release view (Phase 2) and the export view, at the end.
        lines = export_watch('watcher-final')
        final = export_watch_summary(lines, paid=len(EXPORTS))
        results['steps']['i_watcher'] = {
            'summary': BASE_LOCK.watcher_summary(lines, releases_checked=1),
            'releases': [line for line in lines if line.get('kind') == 'release'],
            'exports': final['exports'], 'payouts': final['payouts'],
            'export_totals': final['summary']['exports']['totals']}
        results['passed'] = True
    except Exception as error:
        results['error'] = portable(error)
    finally:
        for driver_process in drivers:
            stop_driver(driver_process)
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
        results['children_stopped'] = all(child.poll() is not None for child in children)
        (out / 'relayer.key').unlink(missing_ok=True)
        secrets_seen = list(secrets)
        secrets.clear()
        # The generated chain holds dev validator keys and the recipient key; never evidence.
        if state['data_dir'] and state['data_dir'].exists() and results['passed']:
            shutil.rmtree(state['data_dir'])
            results['generated_chain_removed'] = True
        results['ended_at_unix'] = int(time.time())
        # Local paths are not evidence: name them by role.
        text = portable_text(json.dumps(results, indent=2), out=out, root=ROOT, roles=roles, secrets=secrets_seen)
        (out / 'summary.json').write_text(text + '\n')
        manifest = {'source_commit': commit, 'source_tree': tree, 'toolchain': results.get('toolchain'),
                    'source_files': {name: ORDER.sha256_file(ROOT / name) for name in SOURCE_FILES},
                    'evidence_files': {'summary.json': ORDER.sha256_file(out / 'summary.json')},
                    'not_published': {p.relative_to(out).as_posix(): ORDER.sha256_file(p)
                                      for p in sorted(out.rglob('*')) if p.is_file()
                                      and p.name not in PUBLISHED},
                    'note': 'no key material; the generated chain and the disposable keys are not recorded'}
        (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        print(json.dumps({key: results.get(key) for key in ('passed', 'error', 'children_stopped')}, indent=2))
    return 0 if results['passed'] else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except RehearsalFailure as refusal:
        print(f'rehearse-lane-roundtrip: {refusal}', file=sys.stderr)
        sys.exit(2)
