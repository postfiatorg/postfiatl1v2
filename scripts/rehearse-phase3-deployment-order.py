#!/usr/bin/env python3
"""Rehearse the Phase 3 deployment order (export design note, section 5) on a local Anvil.

Predicts the verifier's CREATE address from its disposable deployer's next
nonce and the lock's from its own deployer's, predicts both runtime code hashes
on an Anvil snapshot that is then reverted, pins the predicted lock in a PFTL
lane genesis read by the node, deploys the verifier and then the lock at the
planned nonces, and checks the pins on chain. It also shows, once per deployer,
that a nonce off by one is refused before any deployment.

Owns a disposable loopback Anvil (chain id 84532). Never accepts a remote URL
or a key; the deployer keys are generated at run time and never written.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CHAIN_ID = 84532
TOKEN_CRATE = 'crates/pft-token-contracts'
VERIFIER_CRATE = 'crates/ethereum-contracts'
GENESIS_FIXTURE = 'crates/node/testdata/external-lane-genesis/genesis.json'
LANE_ID = 'base-erc20'
# The lock's floor `PFTL_ACCOUNT_RESERVE` (PFTLockV1.sol), the PFTL account
# reserve `ACCOUNT_RESERVE` of crates/execution/src/entrypoints.rs.
MINIMUM_LOCK_AMOUNT = 10
UINT64_MAX = (1 << 64) - 1
# The sources whose sha256 the manifest records at the recorded commit.
SOURCE_FILES = (
    'crates/pft-token-contracts/src/PFTLockV1.sol',
    'crates/pft-token-contracts/src/PFTToken.sol',
    'crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol',
    'crates/types/src/external_lane_types.rs',
    GENESIS_FIXTURE,
    'scripts/rehearse-phase3-deployment-order.py',
)
ARTIFACTS = {
    'PFTToken': (TOKEN_CRATE, 'PFTToken.sol/PFTToken.json'),
    'PFTLockV1': (TOKEN_CRATE, 'PFTLockV1.sol/PFTLockV1.json'),
    'PFTLFinalityVerifierV1': (VERIFIER_CRATE, 'PFTLFinalityVerifierV1.sol/PFTLFinalityVerifierV1.json'),
}
# The files the public record keeps; the manifest lists every other file under the
# output directory, recursively, by relative path, sha256 and size only.
PUBLISHED = ('summary.json', 'manifest.json')


class RehearsalFailure(RuntimeError):
    """A success predicate of the rehearsal did not hold."""


def require(condition, message):
    """Raise unless `condition` holds; unlike `assert`, PYTHONOPTIMIZE keeps it."""
    if not condition:
        raise RehearsalFailure(message)


# --- Keccak-256 (the Ethereum variant, 0x01 padding), so tests need no library.
_RC = (
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
)
_ROT = ((0, 36, 3, 41, 18), (1, 44, 10, 45, 2), (62, 6, 43, 15, 61), (28, 55, 25, 21, 56), (27, 20, 39, 8, 14))
_MASK = (1 << 64) - 1


def _rol(value, shift):
    return ((value << shift) | (value >> (64 - shift))) & _MASK if shift else value


def _keccak_f(a):
    for rc in _RC:
        c = [a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20] for x in range(5)]
        d = [c[(x - 1) % 5] ^ _rol(c[(x + 1) % 5], 1) for x in range(5)]
        for i in range(25):
            a[i] ^= d[i % 5]
        b = [0] * 25
        for x in range(5):
            for y in range(5):
                b[y + 5 * ((2 * x + 3 * y) % 5)] = _rol(a[x + 5 * y], _ROT[x][y])
        for x in range(5):
            for y in range(5):
                a[x + 5 * y] = b[x + 5 * y] ^ (~b[(x + 1) % 5 + 5 * y] & b[(x + 2) % 5 + 5 * y])
        a[0] ^= rc


def keccak256(data: bytes) -> bytes:
    rate = 136
    message = bytearray(data) + b'\x01'
    message += b'\x00' * (-len(message) % rate)
    message[-1] |= 0x80
    state = [0] * 25
    for offset in range(0, len(message), rate):
        for i in range(rate // 8):
            state[i] ^= int.from_bytes(message[offset + 8 * i:offset + 8 * i + 8], 'little')
        _keccak_f(state)
    return b''.join(state[i].to_bytes(8, 'little') for i in range(4))


def _rlp_bytes(value: bytes) -> bytes:
    if len(value) == 1 and value[0] < 0x80:
        return value
    require(len(value) < 56, 'only short RLP strings are needed here')
    return bytes([0x80 + len(value)]) + value


def create_address(deployer: str, nonce: int) -> str:
    """The CREATE address: keccak256(rlp([deployer, nonce]))[12:], lower-case hex."""
    require(isinstance(nonce, int) and nonce >= 0, f'nonce must be a non-negative integer, got {nonce!r}')
    sender = bytes.fromhex(deployer.lower().removeprefix('0x'))
    require(len(sender) == 20, f'deployer must be a 20-byte address, got {deployer}')
    nonce_bytes = nonce.to_bytes((nonce.bit_length() + 7) // 8, 'big') if nonce else b''
    payload = _rlp_bytes(sender) + _rlp_bytes(nonce_bytes)
    return '0x' + keccak256(bytes([0xc0 + len(payload)]) + payload)[12:].hex()


def code_hash(runtime_hex: str) -> str:
    return '0x' + keccak256(bytes.fromhex(runtime_hex.removeprefix('0x'))).hex()


def refuse_unless_planned_nonce(deployer, nonces, planned_nonce, predicted):
    """The guard run immediately before each deployment. `nonces` holds the
    deployer's `latest` and `pending` transaction counts: they must agree (no
    transaction in flight) and equal `planned_nonce`, and `predicted` (the
    addresses planned at `planned_nonce`, `planned_nonce + 1`, ...) must be the
    CREATE addresses at those nonces, so the deployments land where genesis
    pins them. Returns the addresses the deployments would produce."""
    latest, pending = nonces['latest'], nonces['pending']
    require(latest == pending,
            f'deployer {deployer} has latest nonce {latest} but pending nonce {pending}: a transaction is in flight '
            f'and could take a planned nonce; refusing to deploy')
    would = tuple(create_address(deployer, latest + offset) for offset in range(len(predicted)))
    require(latest == planned_nonce and would == tuple(address.lower() for address in predicted),
            f'deployer {deployer} is at nonce {latest}, planned {planned_nonce}: the next deployments would land at '
            f'{", ".join(would)} (predicted {", ".join(predicted)}); the genesis pins can no longer be met, '
            f'refusing to deploy')
    return would


def deployment_plan(*, verifier_deployer, verifier_nonce, lock_deployer, lock_nonce):
    """Each contract's dedicated deployer, its planned nonce and the predicted CREATE address."""
    require(verifier_deployer.lower() != lock_deployer.lower(), 'the verifier and the lock need two deployers')
    return {role: {'deployer': deployer.lower(), 'planned_nonce': nonce, 'predicted': create_address(deployer, nonce)}
            for role, deployer, nonce in (('verifier', verifier_deployer, verifier_nonce),
                                          ('lock', lock_deployer, lock_nonce))}


def nonce_plan_status(plan, nonces):
    """For each contract of `plan`, where its deployment would land given its
    deployer's `latest` and `pending` counts in `nonces`, and whether that is
    still the planned nonce and predicted address."""
    status = {}
    for role, entry in plan.items():
        current = nonces[entry['deployer']]
        would = create_address(entry['deployer'], current['latest'])
        status[role] = {'deployer': entry['deployer'], 'planned_nonce': entry['planned_nonce'],
                        'latest_nonce': current['latest'], 'pending_nonce': current['pending'],
                        'predicted': entry['predicted'], 'would_land_at': would,
                        'intact': current['latest'] == current['pending'] == entry['planned_nonce']
                        and would == entry['predicted']}
    return status


def refuse_unless_plan_intact(plan, nonces):
    """The guard before the first deployment: every deployer must still be at its
    planned nonce, because genesis pins the lock and the lock pins the verifier,
    so one moved prediction makes the pins as a whole unmeetable."""
    refusals = []
    for role, entry in plan.items():
        try:
            refuse_unless_planned_nonce(entry['deployer'], nonces[entry['deployer']], entry['planned_nonce'],
                                        (entry['predicted'],))
        except RehearsalFailure as refusal:
            refusals.append(f'{role}: {refusal}')
    require(not refusals, 'the deployment plan is refused as a whole; ' + '; '.join(refusals))
    return nonce_plan_status(plan, nonces)


def pin_lane_genesis(fixture: dict, *, lock_address, lock_code_hash, token_address, token_code_hash):
    """The fixture lane genesis with the lane's lock and token pins replaced."""
    genesis = copy.deepcopy(fixture)
    policy = lane_policy(genesis)
    policy.update(chain_id=CHAIN_ID, lock_contract_address=lock_address.lower(),
                  lock_runtime_code_hash=lock_code_hash.lower(), token_address=token_address.lower(),
                  token_runtime_code_hash=token_code_hash.lower())
    return genesis


def lane(genesis: dict) -> dict:
    lanes = [entry for entry in genesis.get('external_lanes') or [] if entry.get('id') == LANE_ID]
    require(len(lanes) == 1, f'the genesis must hold exactly one `{LANE_ID}` lane')
    return lanes[0]


def lane_policy(genesis: dict) -> dict:
    return lane(genesis)['policy']


def lane_window_cap(genesis: dict) -> int:
    """The lane's per-window release cap, `reserved_atoms * release_cap_bps // 10_000`
    (Phase 2 design note, section 5.1), which the lock's `maxLockAmount` must equal."""
    entry = lane(genesis)
    cap = entry['reserved_atoms'] * entry['release_cap_bps'] // 10_000
    require(MINIMUM_LOCK_AMOUNT <= cap <= UINT64_MAX,
            f'the `{LANE_ID}` lane cap {cap} is not a uint64 at least the minimum Lock {MINIMUM_LOCK_AMOUNT}')
    return cap


def refuse_unless_cap_matches(genesis: dict, max_lock_amount: int) -> int:
    cap = lane_window_cap(genesis)
    require(cap == max_lock_amount,
            f'the lock\'s maxLockAmount {max_lock_amount} differs from the `{LANE_ID}` lane\'s per-window release '
            f'cap {cap}; refusing')
    return cap


def _h48(value: int) -> bytes:
    return bytes([value]) * 48


# Constructor values of the verifier other than the bindings. They are the
# fixture checkpoint of crates/ethereum-contracts/test/PFUSDCTier4.t.sol
# (setUp); in production the first finalized PFTL checkpoint and the route
# profile provide them.
FIXTURE_CHECKPOINT = {
    'source': 'crates/ethereum-contracts/test/PFUSDCTier4.t.sol (setUp)',
    'programVKey': '0x' + keccak256(b'program-vkey').hex(),
    'pftlChainIdHash': '0x' + keccak256(b'postfiat-tier4-test').hex(),
    'pftlGenesisHashCommitment': '0x' + keccak256(_h48(0x11)).hex(),
    'pftlProtocolVersion': 1,
    'routeProfileHashCommitment': '0x' + keccak256(_h48(0x22)).hex(),
    'routeEpoch': 7,
    'assetIdCommitment': '0x' + keccak256(_h48(0x33)).hex(),
    'maxProofBytes': 4096,
    'maxPublicValuesBytes': 16384,
    'initialCheckpointCommitment': '0x' + keccak256(_h48(0x10)).hex(),
    'initialFinalizedHeight': 10,
    'initialCommitteeRootCommitment': '0x' + keccak256(_h48(0x41)).hex(),
}
# No proof is verified in this rehearsal; the constructor only refuses a zero SP1 verifier.
FIXTURE_SP1_VERIFIER = '0x00000000000000000000000000000000000000f1'


# The fields of `PFTLFinalityVerifierV1.Config`, in declaration order.
VERIFIER_CONFIG_FIELDS = (
    'sp1Verifier', 'programVKey', 'pftlChainIdHash', 'pftlGenesisHashCommitment', 'pftlProtocolVersion',
    'routeProfileHashCommitment', 'routeEpoch', 'assetIdCommitment', 'arbitrumChainId', 'vaultRuntimeCodeHash',
    'token', 'tokenRuntimeCodeHash', 'maxProofBytes', 'maxPublicValuesBytes', 'initialCheckpointCommitment',
    'initialFinalizedHeight', 'initialCommitteeRootCommitment',
)
# The verifier's public getter for each constructor value (the constructor
# stores the three `initial...` values in the `latest...` state variables).
VERIFIER_GETTERS = {field: field for field in VERIFIER_CONFIG_FIELDS} | {
    'initialCheckpointCommitment': 'latestCheckpointCommitment',
    'initialFinalizedHeight': 'latestFinalizedHeight',
    'initialCommitteeRootCommitment': 'latestCommitteeRootCommitment',
}
# The lock's constructor values that it exposes, in constructor order.
LOCK_GETTERS = ('token', 'finalityVerifier', 'maxLockAmount', 'minimumLockAmount')


def verifier_config(*, token, token_code_hash, vault_code_hash):
    """The verifier's constructor values by field name."""
    c = FIXTURE_CHECKPOINT
    config = {name: c[name] for name in VERIFIER_CONFIG_FIELDS if name in c}
    config.update(sp1Verifier=FIXTURE_SP1_VERIFIER, arbitrumChainId=CHAIN_ID, vaultRuntimeCodeHash=vault_code_hash,
                  token=token, tokenRuntimeCodeHash=token_code_hash)
    require(set(config) == set(VERIFIER_CONFIG_FIELDS), 'the verifier config must name every Config field')
    return config


def verifier_constructor_tuple(config):
    return '(' + ','.join(str(config[name]) for name in VERIFIER_CONFIG_FIELDS) + ')'


def abi_word(value) -> str:
    """The 32-byte ABI word of a uint, an address or a bytes32, as lower-case hex."""
    if isinstance(value, int):
        return f'0x{value:064x}'
    raw = value.lower().removeprefix('0x')
    require(len(raw) in (40, 64), f'{value!r} is neither an address nor a bytes32')
    return '0x' + raw.rjust(64, '0')


def check_constructor_values(call_word, *, verifier, verifier_values, lock, lock_values):
    """Read back every constructor value of the verifier and the lock through its
    getter and compare it, by name, with the value the script passed. A swap of
    two same-typed values in the constructor arguments fails here even though
    the predicted code hash, computed from the same arguments, would match.
    Returns the getters read."""
    read = []
    for contract, address, getters, values in (
            ('verifier', verifier, VERIFIER_GETTERS, verifier_values),
            ('lock', lock, {name: name for name in LOCK_GETTERS}, lock_values)):
        require(set(values) == set(getters), f'{contract}: the expected values must name every constructor value')
        for field, getter in getters.items():
            actual = call_word(address, f'{getter}()').lower()
            require(actual == abi_word(values[field]),
                    f'{contract}.{getter}() is {actual}, but the script passed {field} = {values[field]}')
            read.append(f'{contract}.{getter}()')
    return read


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def clean_source():
    """The commit and tree the run executes: the worktree must hold exactly a commit."""
    dirty = git('status', '--porcelain')
    require(not dirty, f'the worktree is not clean; commit or stash first:\n{dirty}')
    return git('rev-parse', 'HEAD'), git('rev-parse', 'HEAD^{tree}')


def sha256_file(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def confined_output(output):
    out = output.resolve()
    require(not out.exists(), f'--output {out} already exists; name a fresh directory')
    require(not out.is_relative_to(ROOT / 'docs'), '--output must not be inside docs/; publish the record by hand')
    return out


def build_manifest(out, commit, tree, toolchain, deployment):
    files = {path.relative_to(out).as_posix(): path for path in sorted(out.rglob('*')) if path.is_file()}
    files.pop('manifest.json', None)
    return {
        'source_commit': commit,
        'source_tree': tree,
        'toolchain': toolchain,
        'deployment': deployment,
        'source_files': {name: sha256_file(ROOT / name) for name in SOURCE_FILES},
        'evidence_files': {name: sha256_file(path) for name, path in files.items() if name in PUBLISHED},
        'not_published': {name: {'sha256': sha256_file(path), 'bytes': path.stat().st_size}
                          for name, path in files.items() if name not in PUBLISHED},
        'not_published_note': 'Raw run artifacts stay out of the docs tree; '
                              'scripts/rehearse-phase3-deployment-order.py regenerates them in its --output directory.',
    }


def record_step(deployment, step, **fields):
    """Record what a step put on chain or fixed, as soon as it succeeded, so a
    failed run still says what landed."""
    deployment.update(fields)
    deployment['steps_completed'].append(step)


def write_records(out, results, commit, tree):
    """Write summary.json and manifest.json, on success and on failure alike."""
    (out / 'summary.json').write_text(json.dumps(results, indent=2) + '\n')
    manifest = build_manifest(out, commit, tree, results.get('toolchain'), results.get('deployment'))
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    return manifest


def tool_version(tool):
    """The first line of `tool --version`, or None when the tool is absent."""
    try:
        return subprocess.check_output([tool, '--version'], cwd=ROOT, text=True).strip().splitlines()[0]
    except (OSError, subprocess.CalledProcessError):
        return None


def build_provenance(node_build_profile):
    """The host and Rust toolchain the run used. The node binary's sha256 identifies
    the exact binary; a debug build is not expected to reproduce it."""
    return {'cargo': tool_version('cargo'), 'rustc': tool_version('rustc'),
            'python': platform.python_version(), 'host_os': platform.platform(),
            'postfiat_node_build_profile': node_build_profile}


def free_port():
    with socket.socket() as stream:
        stream.bind(('127.0.0.1', 0))
        return stream.getsockname()[1]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--output', type=Path, required=True, help='a fresh directory for the run artifacts')
    parser.add_argument('--node-bin', type=Path,
                        help='a built postfiat-node; default: cargo build into this checkout\'s ./target')
    args = parser.parse_args(argv)
    out = confined_output(args.output)
    commit, tree = clean_source()
    out.mkdir(parents=True, mode=0o700)
    print(f'output directory: {out}', file=sys.stderr)
    environment = os.environ.copy()
    results = {'scope': 'local Anvil (chain id 84532) and a generated PFTL genesis only',
               'public_network_transaction': False, 'source_commit': commit, 'source_tree': tree,
               'started_at_unix': int(time.time()), 'commands': [], 'passed': False}
    # Filled as each step succeeds; written on failure too.
    deployment = results['deployment'] = {'chain_id': CHAIN_ID, 'complete': False, 'steps_completed': []}
    secrets = []
    children = []
    url = None

    def portable(value):
        """A recorded path or argument without the local checkout or output path."""
        text = str(value)
        if text in secrets:
            return '<disposable-key>'
        return text.replace(str(out), '<output>').replace(str(ROOT), '<checkout>')

    def redacted(argv):
        return [portable(a) for a in argv]

    def command(name, argv, cwd=ROOT, env=None):
        began = time.time()
        with (out / f'{name}.stdout').open('w') as stdout, (out / f'{name}.stderr').open('w') as stderr:
            p = subprocess.run([str(a) for a in argv], cwd=cwd, env=env or environment, stdout=stdout,
                               stderr=stderr, text=True, timeout=1800)
        results['commands'].append({'name': name, 'argv': redacted(argv), 'cwd': portable(cwd),
                                    'exit_code': p.returncode, 'seconds': round(time.time() - began, 3)})
        if p.returncode != 0:
            raise RehearsalFailure(f'{name}: exit {p.returncode}; see {out / (name + ".stderr")}')
        return (out / f'{name}.stdout').read_text()

    def rpc(method, params=()):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode()
        request = urllib.request.Request(url, data=body, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=10) as response:
            value = json.load(response)
        if 'error' in value:
            raise RehearsalFailure(f'local Anvil refused {method}: {value["error"]}')
        return value['result']

    def nonces_of(address):
        return {tag: int(rpc('eth_getTransactionCount', [address, tag]), 16) for tag in ('latest', 'pending')}

    def runtime(address):
        return rpc('eth_getCode', [address, 'latest'])

    def call_word(address, signature):
        word = rpc('eth_call', [{'to': address, 'data': '0x' + keccak256(signature.encode())[:4].hex()}, 'latest'])
        require(len(word) == 66, f'{signature} on {address} returned {word}')
        return word.lower()

    try:
        # Tools, contracts and the node binary.
        toolchain = {tool: subprocess.check_output([tool, '--version'], text=True).strip().splitlines()[0]
                     for tool in ('anvil', 'forge', 'cast')}
        forge_env = {}
        for crate in (TOKEN_CRATE, VERIFIER_CRATE):
            tag = Path(crate).name
            forge_env[crate] = dict(environment, FOUNDRY_OUT=str(out / f'forge-out-{tag}'),
                                    FOUNDRY_CACHE_PATH=str(out / f'forge-cache-{tag}'),
                                    FOUNDRY_BROADCAST=str(out / f'forge-broadcast-{tag}'))
            command(f'forge-build-{tag}', ['forge', 'build'], ROOT / crate, forge_env[crate])
        artifacts = {}
        for name, (crate, relative) in ARTIFACTS.items():
            path = out / f'forge-out-{Path(crate).name}' / relative
            artifact = json.loads(path.read_text())
            artifacts[name] = {
                'artifact_sha256': sha256_file(path),
                'deployed_bytecode_sha256': hashlib.sha256(
                    artifact['deployedBytecode']['object'].encode()).hexdigest(),
                'solc_version': ((artifact.get('metadata') or {}).get('compiler') or {}).get('version'),
            }
        toolchain['artifacts'] = artifacts
        toolchain['foundry_toml_sha256'] = {crate: sha256_file(ROOT / crate / 'foundry.toml')
                                            for crate in (TOKEN_CRATE, VERIFIER_CRATE)}
        node_bin = args.node_bin
        if node_bin is None:
            command('node-build', ['cargo', 'build', '--locked', '-p', 'postfiat-node', '--bin', 'postfiat-node'])
            node_bin = ROOT / 'target/debug/postfiat-node'
        node_bin = node_bin.resolve()
        toolchain.update(build_provenance('dev (debug), cargo build --locked' if args.node_bin is None
                                          else 'unknown (given by --node-bin)'))
        toolchain['postfiat_node_sha256'] = sha256_file(node_bin)
        toolchain['postfiat_node_sha256_note'] = ('identifies the exact binary used; not expected to reproduce '
                                                  'across builds')
        results['toolchain'] = toolchain

        # A fresh loopback Anvil.
        for _ in range(5):
            port = free_port()
            child = subprocess.Popen(['anvil', '--host', '127.0.0.1', '--port', str(port), '--chain-id',
                                      str(CHAIN_ID), '--silent'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            children.append(child)
            url = f'http://127.0.0.1:{port}'
            for _ in range(100):
                if child.poll() is not None:
                    break
                try:
                    if 'anvil' in str(rpc('web3_clientVersion')).lower():
                        break
                except OSError:
                    time.sleep(.1)
            if child.poll() is None:
                break
        require(children and children[-1].poll() is None, 'Anvil did not start')
        require(int(rpc('eth_chainId'), 16) == CHAIN_ID, f'the local Anvil must report chain id {CHAIN_ID}')

        # Disposable keys: the token's deployer, the verifier's deployer and the lock's deployer
        # (export design note, section 5: one dedicated deployer per contract).
        def disposable_account():
            key = '0x' + os.urandom(32).hex()
            secrets.append(key)
            address = command(f'account-{len(secrets)}', ['cast', 'wallet', 'address', '--private-key', key]).strip().lower()
            rpc('anvil_setBalance', [address, hex(10 ** 20)])
            return key, address

        token_key, token_deployer = disposable_account()
        verifier_key, verifier_deployer = disposable_account()
        lock_key, lock_deployer = disposable_account()
        require(len({token_deployer, verifier_deployer, lock_deployer}) == 3, 'the three deployers must differ')
        keys = {verifier_deployer: verifier_key, lock_deployer: lock_key}

        def deploy(name, crate, contract, key, constructor, nonce=None):
            """`forge create`; with `nonce`, the transaction's nonce is pinned, so it lands at that nonce or fails."""
            pin = [] if nonce is None else ['--nonce', str(nonce)]
            argv = ['forge', 'create', contract, '--rpc-url', url, '--private-key', key, '--broadcast', '--json',
                    *pin, '--constructor-args', *constructor]
            return json.loads(command(name, argv, ROOT / crate, forge_env[crate]))['deployedTo'].lower()

        token = deploy('deploy-token', TOKEN_CRATE, 'src/PFTToken.sol:PFTToken', token_key,
                       ['Rehearsal PFT', 'rPFT', '300000000', token_deployer])
        token_code_hash = code_hash(runtime(token))
        record_step(deployment, 'token deployed',
                    deployers={'token': token_deployer, 'verifier': verifier_deployer, 'lock': lock_deployer},
                    token=token, token_runtime_code_hash=token_code_hash)

        # (1) Predict the verifier's CREATE address from its deployer's next nonce and the
        # lock's from its own deployer's next nonce.
        def all_nonces():
            return {address: nonces_of(address) for address in (verifier_deployer, lock_deployer)}

        current = all_nonces()
        plan = deployment_plan(verifier_deployer=verifier_deployer, verifier_nonce=current[verifier_deployer]['latest'],
                               lock_deployer=lock_deployer, lock_nonce=current[lock_deployer]['latest'])
        refuse_unless_plan_intact(plan, current)
        predicted_verifier, predicted_lock = plan['verifier']['predicted'], plan['lock']['predicted']
        verifier_nonce, lock_nonce = plan['verifier']['planned_nonce'], plan['lock']['planned_nonce']
        for role, entry in plan.items():
            computed = command(f'cast-compute-address-{role}',
                               ['cast', 'compute-address', entry['deployer'], '--nonce', entry['planned_nonce']]
                               ).strip().split()[-1].lower()
            require(computed == entry['predicted'], f'{role}: cast computes {computed}, the script {entry["predicted"]}')
        record_step(deployment, 'addresses predicted', deployer_nonce_verifier=verifier_nonce,
                    deployer_nonce_lock=lock_nonce, predicted={'verifier': predicted_verifier, 'lock': predicted_lock})

        # The lock's cap is the per-window release cap of the lane the genesis pins.
        fixture = json.loads((ROOT / GENESIS_FIXTURE).read_text())
        max_lock_amount = lane_window_cap(fixture)
        lock_values = {'token': token, 'finalityVerifier': predicted_verifier, 'maxLockAmount': max_lock_amount,
                       'minimumLockAmount': MINIMUM_LOCK_AMOUNT}
        lock_args = [token, predicted_verifier, token_code_hash, str(max_lock_amount), str(MINIMUM_LOCK_AMOUNT)]

        def on_snapshot(label, body):
            """Run `body` on an Anvil snapshot and revert it, checking both deployers'
            nonces and both predicted addresses are as before."""
            snapshot = rpc('evm_snapshot')
            try:
                return body()
            finally:
                require(rpc('evm_revert', [snapshot]) is True, f'{label}: evm_revert failed')
                require(all(nonce_plan_status(plan, all_nonces())[role]['intact'] for role in plan),
                        f'{label}: the revert did not restore the deployers\' nonces')
                require(runtime(predicted_verifier) == '0x' and runtime(predicted_lock) == '0x',
                        f'{label}: code remains at a predicted address after the revert')

        # (2) The lock's runtime code holds the verifier's address as an immutable, so
        # its code hash is taken from a lock deployed for the predicted verifier on a
        # snapshot. The probe verifier only has to exist there; its own vault hash is
        # a placeholder. The verifier's code hash, which holds the lock's code hash,
        # is then taken the same way.
        placeholder = '0x' + keccak256(b'phase3-rehearsal-placeholder-vault-code-hash').hex()

        probe_values = verifier_config(token=token, token_code_hash=token_code_hash, vault_code_hash=placeholder)

        def probe_lock():
            verifier = deploy('probe-verifier', VERIFIER_CRATE, 'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1',
                              verifier_key, [verifier_constructor_tuple(probe_values)],
                              nonce=verifier_nonce)
            require(verifier == predicted_verifier, f'probe verifier at {verifier}, predicted {predicted_verifier}')
            lock = deploy('probe-lock', TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1', lock_key, lock_args,
                          nonce=lock_nonce)
            require(lock == predicted_lock, f'probe lock at {lock}, predicted {predicted_lock}')
            return code_hash(runtime(lock))

        predicted_lock_code_hash = on_snapshot('lock code hash', probe_lock)

        verifier_values = verifier_config(token=token, token_code_hash=token_code_hash,
                                          vault_code_hash=predicted_lock_code_hash)

        def probe_verifier():
            verifier = deploy('probe-verifier-final', VERIFIER_CRATE,
                              'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1', verifier_key,
                              [verifier_constructor_tuple(verifier_values)],
                              nonce=verifier_nonce)
            require(verifier == predicted_verifier, f'probe verifier at {verifier}, predicted {predicted_verifier}')
            return code_hash(runtime(verifier))

        predicted_verifier_code_hash = on_snapshot('verifier code hash', probe_verifier)
        deployment['predicted'].update(verifier_runtime_code_hash=predicted_verifier_code_hash,
                                       lock_runtime_code_hash=predicted_lock_code_hash)
        record_step(deployment, 'code hashes predicted',
                    code_hash_method='deploy on an evm_snapshot, read eth_getCode, evm_revert',
                    lock_constructor={'maxLockAmount': max_lock_amount, 'minimumLockAmount': MINIMUM_LOCK_AMOUNT,
                                      'maxLockAmount_source': 'the lane per-window release cap, reserved_atoms * '
                                                              'release_cap_bps // 10_000, of the pinned genesis',
                                      'minimumLockAmount_source': 'PFTLockV1.PFTL_ACCOUNT_RESERVE, the PFTL account '
                                                                  'reserve'},
                    verifier_fixture_checkpoint=FIXTURE_CHECKPOINT, verifier_sp1_verifier=FIXTURE_SP1_VERIFIER)

        # (3) Pin the predicted lock in a lane genesis and read it with the node.
        genesis = pin_lane_genesis(fixture, lock_address=predicted_lock, lock_code_hash=predicted_lock_code_hash,
                                   token_address=token, token_code_hash=token_code_hash)
        genesis_path = out / 'genesis.json'
        genesis_path.write_text(json.dumps(genesis, indent=2) + '\n')
        node_dir = out / 'pftl-node'
        init_report = json.loads(command('node-init', [node_bin, 'init', '--genesis-file', genesis_path,
                                                       '--data-dir', node_dir, '--node-id', 'validator-0']))
        # The node stores its genesis as JSON followed by a MAC line.
        node_genesis, _ = json.JSONDecoder().raw_decode((node_dir / 'genesis.json').read_text())
        pinned = lane_policy(node_genesis)
        require((pinned['lock_contract_address'], pinned['lock_runtime_code_hash']) ==
                (predicted_lock, predicted_lock_code_hash), 'the node read other lock pins than the ones written')
        refuse_unless_cap_matches(node_genesis, max_lock_amount)
        record_step(deployment, 'genesis pinned',
                    genesis_fixture=GENESIS_FIXTURE, genesis_sha256=sha256_file(genesis_path),
                    genesis_lane_policy=pinned,
                    genesis_parse_check='postfiat-node init --genesis-file validated the genesis; the lane pins were '
                                        'read back from the data directory',
                    pftl_genesis_hash=init_report.get('genesis_hash'), pftl_chain_id=init_report.get('chain_id'))

        # (7) A deployer nonce off by one, once per deployer: a stray transaction moves that
        # deployer's nonce. The other contract's predicted address stays intact, but the
        # pins as a whole are refused before any deployment.
        def stray_nonce(role):
            def body():
                deployer = plan[role]['deployer']
                command(f'stray-transaction-{role}', ['cast', 'send', deployer, '--value', '0', '--rpc-url', url,
                                                      '--private-key', keys[deployer]])
                current = all_nonces()
                status = nonce_plan_status(plan, current)
                other = 'lock' if role == 'verifier' else 'verifier'
                require(not status[role]['intact'], f'a stray transaction on the {role} deployer did not move it')
                require(status[other]['intact'] and status[other]['would_land_at'] == plan[other]['predicted'],
                        f'a stray transaction on the {role} deployer moved the {other}\'s predicted address')
                try:
                    refuse_unless_plan_intact(plan, current)
                except RehearsalFailure as refusal:
                    require(runtime(predicted_verifier) == '0x' and runtime(predicted_lock) == '0x',
                            'the refusal must come before any deployment')
                    return {'stray_transaction_on': f'{role} deployer', 'status': status, 'refused': True,
                            'message': str(refusal)}
                raise RehearsalFailure(f'the guard accepted the plan with the {role} deployer off by one')
            return body

        results['nonce_off_by_one'] = {role: on_snapshot(f'nonce off by one ({role})', stray_nonce(role))
                                       for role in plan}

        # (4)-(5) Deploy the verifier with the fixture checkpoint, then the lock, each from its
        # deployer at its planned nonce. Before the verifier the guard re-reads both deployers'
        # latest and pending nonces, before the lock the lock deployer's; each transaction pins
        # its nonce, so it lands at the predicted address or fails.
        refuse_unless_plan_intact(plan, all_nonces())
        verifier = deploy('deploy-verifier', VERIFIER_CRATE, 'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1',
                          verifier_key, [verifier_constructor_tuple(verifier_values)], nonce=verifier_nonce)
        deployment['deployed'] = {'verifier': verifier, 'verifier_runtime_code_hash': code_hash(runtime(verifier))}
        record_step(deployment, 'verifier deployed')
        require(verifier == predicted_verifier, f'verifier at {verifier}, predicted {predicted_verifier}')
        refuse_unless_planned_nonce(lock_deployer, nonces_of(lock_deployer), lock_nonce, (predicted_lock,))
        lock = deploy('deploy-lock', TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1', lock_key, lock_args, nonce=lock_nonce)
        deployment['deployed'].update(lock=lock, lock_runtime_code_hash=code_hash(runtime(lock)))
        record_step(deployment, 'lock deployed')

        # (6) The on-chain checks.
        deployed_verifier_code_hash = code_hash(runtime(verifier))
        deployed_lock_code_hash = code_hash(runtime(lock))
        require(verifier == predicted_verifier, f'verifier at {verifier}, predicted {predicted_verifier}')
        require(lock == predicted_lock, f'lock at {lock}, predicted {predicted_lock}')
        require(deployed_verifier_code_hash == predicted_verifier_code_hash, 'verifier code hash differs from the prediction')
        require(deployed_lock_code_hash == predicted_lock_code_hash, 'lock code hash differs from the prediction')
        constructor_values_read = check_constructor_values(
            call_word, verifier=verifier,
            verifier_values=verifier_values, lock=lock, lock_values=lock_values)
        require((pinned['lock_contract_address'], pinned['lock_runtime_code_hash'],
                 pinned['token_address'], pinned['token_runtime_code_hash']) ==
                (lock, deployed_lock_code_hash, token, token_code_hash), 'the genesis pins do not equal the deployed lock')

        record_step(deployment, 'on-chain checks held', constructor_values_read_back=constructor_values_read,
                    complete=True)
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
                    child.kill()
                    child.wait()
        results['children_stopped'] = all(child.poll() is not None for child in children)
        secrets.clear()
        # The generated node directory holds dev validator keys; it is never evidence.
        if (out / 'pftl-node').exists() and results['passed']:
            shutil.rmtree(out / 'pftl-node')
            results['generated_node_dir_removed'] = True
        results['ended_at_unix'] = int(time.time())
        write_records(out, results, commit, tree)
        print(json.dumps({key: results.get(key) for key in ('passed', 'deployment', 'nonce_off_by_one', 'error')},
                         indent=2))
    return 0 if results['passed'] else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except RehearsalFailure as refusal:
        print(f'rehearse-phase3-deployment-order: {refusal}', file=sys.stderr)
        sys.exit(2)
