#!/usr/bin/env python3
"""Rehearse the Phase 3 deployment order (export design note, section 5) on a local Anvil.

Predicts the verifier's and the lock's CREATE addresses from a disposable
deployer and its next two nonces, predicts both runtime code hashes on an Anvil
snapshot that is then reverted, pins the predicted lock in a PFTL lane genesis
read by the node, deploys the verifier and then the lock at the predicted
nonces, and checks the pins on chain. It also shows that a deployer nonce off
by one is refused before any deployment.

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
MAX_LOCK_AMOUNT = 6_000_000
MINIMUM_LOCK_AMOUNT = 10
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
# The files the public record keeps; the manifest lists every other one by sha256 and size only.
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


def refuse_unless_planned_nonce(deployer, current_nonce, planned_nonce, predicted_verifier, predicted_lock):
    """The guard run before any deployment: the deployer's next nonce must be the
    planned one, so the verifier and the lock land on the predicted addresses that
    genesis pins. Returns the two addresses the deployment would produce."""
    would_verifier = create_address(deployer, current_nonce)
    would_lock = create_address(deployer, current_nonce + 1)
    require(current_nonce == planned_nonce and would_verifier == predicted_verifier.lower()
            and would_lock == predicted_lock.lower(),
            f'deployer {deployer} is at nonce {current_nonce}, planned {planned_nonce}: the verifier would land at '
            f'{would_verifier} (predicted {predicted_verifier}) and the lock at {would_lock} (predicted '
            f'{predicted_lock}); the genesis pins can no longer be met, refusing to deploy')
    return would_verifier, would_lock


def pin_lane_genesis(fixture: dict, *, lock_address, lock_code_hash, token_address, token_code_hash):
    """The fixture lane genesis with the lane's lock and token pins replaced."""
    genesis = copy.deepcopy(fixture)
    lanes = [lane for lane in genesis.get('external_lanes') or [] if lane.get('id') == LANE_ID]
    require(len(lanes) == 1, f'the genesis fixture must hold exactly one `{LANE_ID}` lane')
    policy = lanes[0]['policy']
    policy.update(chain_id=CHAIN_ID, lock_contract_address=lock_address.lower(),
                  lock_runtime_code_hash=lock_code_hash.lower(), token_address=token_address.lower(),
                  token_runtime_code_hash=token_code_hash.lower())
    return genesis


def lane_policy(genesis: dict) -> dict:
    return next(lane['policy'] for lane in genesis['external_lanes'] if lane['id'] == LANE_ID)


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


def verifier_constructor_tuple(*, token, token_code_hash, vault_code_hash):
    c = FIXTURE_CHECKPOINT
    fields = [FIXTURE_SP1_VERIFIER, c['programVKey'], c['pftlChainIdHash'], c['pftlGenesisHashCommitment'],
              c['pftlProtocolVersion'], c['routeProfileHashCommitment'], c['routeEpoch'], c['assetIdCommitment'],
              CHAIN_ID, vault_code_hash, token, token_code_hash, c['maxProofBytes'], c['maxPublicValuesBytes'],
              c['initialCheckpointCommitment'], c['initialFinalizedHeight'], c['initialCommitteeRootCommitment']]
    return '(' + ','.join(str(field) for field in fields) + ')'


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
    files = sorted(path for path in out.iterdir() if path.is_file() and path.name != 'manifest.json')
    return {
        'source_commit': commit,
        'source_tree': tree,
        'toolchain': toolchain,
        'deployment': deployment,
        'source_files': {name: sha256_file(ROOT / name) for name in SOURCE_FILES},
        'evidence_files': {path.name: sha256_file(path) for path in files if path.name in PUBLISHED},
        'not_published': {path.name: {'sha256': sha256_file(path), 'bytes': path.stat().st_size}
                          for path in files if path.name not in PUBLISHED},
        'not_published_note': 'Raw run artifacts stay out of the docs tree; '
                              'scripts/rehearse-phase3-deployment-order.py regenerates them in its --output directory.',
    }


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

    def nonce_of(address):
        return int(rpc('eth_getTransactionCount', [address, 'latest']), 16)

    def runtime(address):
        return rpc('eth_getCode', [address, 'latest'])

    def call_word(address, signature):
        word = rpc('eth_call', [{'to': address, 'data': '0x' + keccak256(signature.encode())[:4].hex()}, 'latest'])
        require(len(word) == 66, f'{signature} on {address} returned {word}')
        return word.lower()

    def as_address(word):
        return '0x' + word[-40:]

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
        toolchain['postfiat_node_sha256'] = sha256_file(node_bin)
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

        # Disposable keys: one for the token's deployer, one for the verifier and lock deployer.
        def disposable_account():
            key = '0x' + os.urandom(32).hex()
            secrets.append(key)
            address = command(f'account-{len(secrets)}', ['cast', 'wallet', 'address', '--private-key', key]).strip().lower()
            rpc('anvil_setBalance', [address, hex(10 ** 20)])
            return key, address

        token_key, token_deployer = disposable_account()
        deployer_key, deployer = disposable_account()

        def deploy(name, crate, contract, key, constructor):
            argv = ['forge', 'create', contract, '--rpc-url', url, '--private-key', key, '--broadcast', '--json',
                    '--constructor-args', *constructor]
            return json.loads(command(name, argv, ROOT / crate, forge_env[crate]))['deployedTo'].lower()

        token = deploy('deploy-token', TOKEN_CRATE, 'src/PFTToken.sol:PFTToken', token_key,
                       ['Rehearsal PFT', 'rPFT', '300000000', token_deployer])
        token_code_hash = code_hash(runtime(token))

        # (1) Predict both CREATE addresses from the deployer and its next two nonces.
        planned_nonce = nonce_of(deployer)
        predicted_verifier = create_address(deployer, planned_nonce)
        predicted_lock = create_address(deployer, planned_nonce + 1)
        for nonce, predicted in ((planned_nonce, predicted_verifier), (planned_nonce + 1, predicted_lock)):
            computed = command(f'cast-compute-address-{nonce}',
                               ['cast', 'compute-address', deployer, '--nonce', nonce]).strip().split()[-1].lower()
            require(computed == predicted, f'nonce {nonce}: cast computes {computed}, the script {predicted}')

        lock_args = [token, predicted_verifier, token_code_hash, str(MAX_LOCK_AMOUNT), str(MINIMUM_LOCK_AMOUNT)]

        def on_snapshot(label, body):
            """Run `body` on an Anvil snapshot and revert it, checking the deployer's
            nonce and both predicted addresses are as before."""
            snapshot = rpc('evm_snapshot')
            try:
                return body()
            finally:
                require(rpc('evm_revert', [snapshot]) is True, f'{label}: evm_revert failed')
                require(nonce_of(deployer) == planned_nonce, f'{label}: the revert did not restore the nonce')
                require(runtime(predicted_verifier) == '0x' and runtime(predicted_lock) == '0x',
                        f'{label}: code remains at a predicted address after the revert')

        # (2) The lock's runtime code holds the verifier's address as an immutable, so
        # its code hash is taken from a lock deployed for the predicted verifier on a
        # snapshot. The probe verifier only has to exist there; its own vault hash is
        # a placeholder. The verifier's code hash, which holds the lock's code hash,
        # is then taken the same way.
        placeholder = '0x' + keccak256(b'phase3-rehearsal-placeholder-vault-code-hash').hex()

        def probe_lock():
            verifier = deploy('probe-verifier', VERIFIER_CRATE, 'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1',
                              deployer_key, [verifier_constructor_tuple(token=token, token_code_hash=token_code_hash,
                                                                         vault_code_hash=placeholder)])
            require(verifier == predicted_verifier, f'probe verifier at {verifier}, predicted {predicted_verifier}')
            lock = deploy('probe-lock', TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1', deployer_key, lock_args)
            require(lock == predicted_lock, f'probe lock at {lock}, predicted {predicted_lock}')
            return code_hash(runtime(lock))

        predicted_lock_code_hash = on_snapshot('lock code hash', probe_lock)

        def probe_verifier():
            verifier = deploy('probe-verifier-final', VERIFIER_CRATE,
                              'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1', deployer_key,
                              [verifier_constructor_tuple(token=token, token_code_hash=token_code_hash,
                                                          vault_code_hash=predicted_lock_code_hash)])
            require(verifier == predicted_verifier, f'probe verifier at {verifier}, predicted {predicted_verifier}')
            return code_hash(runtime(verifier))

        predicted_verifier_code_hash = on_snapshot('verifier code hash', probe_verifier)

        # (3) Pin the predicted lock in a lane genesis and read it with the node.
        fixture = json.loads((ROOT / GENESIS_FIXTURE).read_text())
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

        # (7) A deployer nonce off by one: a stray transaction moves the nonce and the
        # guard refuses before any deployment.
        def stray_nonce():
            command('stray-transaction', ['cast', 'send', deployer, '--value', '0', '--rpc-url', url,
                                          '--private-key', deployer_key])
            current = nonce_of(deployer)
            try:
                refuse_unless_planned_nonce(deployer, current, planned_nonce, predicted_verifier, predicted_lock)
            except RehearsalFailure as refusal:
                require(runtime(predicted_verifier) == '0x' and runtime(predicted_lock) == '0x',
                        'the refusal must come before any deployment')
                return {'deployer_nonce': current, 'planned_nonce': planned_nonce,
                        'verifier_would_land_at': create_address(deployer, current),
                        'lock_would_land_at': create_address(deployer, current + 1),
                        'refused': True, 'message': str(refusal)}
            raise RehearsalFailure('the guard deployed with a deployer nonce off by one')

        results['nonce_off_by_one'] = on_snapshot('nonce off by one', stray_nonce)

        # (4)-(5) Deploy the verifier with the fixture checkpoint, then the lock, at the predicted nonces.
        refuse_unless_planned_nonce(deployer, nonce_of(deployer), planned_nonce, predicted_verifier, predicted_lock)
        verifier = deploy('deploy-verifier', VERIFIER_CRATE, 'src/PFTLFinalityVerifierV1.sol:PFTLFinalityVerifierV1',
                          deployer_key, [verifier_constructor_tuple(token=token, token_code_hash=token_code_hash,
                                                                     vault_code_hash=predicted_lock_code_hash)])
        lock = deploy('deploy-lock', TOKEN_CRATE, 'src/PFTLockV1.sol:PFTLockV1', deployer_key, lock_args)

        # (6) The on-chain checks.
        deployed_verifier_code_hash = code_hash(runtime(verifier))
        deployed_lock_code_hash = code_hash(runtime(lock))
        require(verifier == predicted_verifier, f'verifier at {verifier}, predicted {predicted_verifier}')
        require(lock == predicted_lock, f'lock at {lock}, predicted {predicted_lock}')
        require(deployed_verifier_code_hash == predicted_verifier_code_hash, 'verifier code hash differs from the prediction')
        require(deployed_lock_code_hash == predicted_lock_code_hash, 'lock code hash differs from the prediction')
        require(as_address(call_word(lock, 'finalityVerifier()')) == verifier, 'lock.finalityVerifier is not the verifier')
        require(as_address(call_word(lock, 'token()')) == token, 'lock.token is not the token')
        require(call_word(verifier, 'vaultRuntimeCodeHash()') == deployed_lock_code_hash,
                'verifier.vaultRuntimeCodeHash is not the lock\'s code hash')
        require(as_address(call_word(verifier, 'token()')) == token, 'verifier.token is not the token')
        require(call_word(verifier, 'tokenRuntimeCodeHash()') == token_code_hash, 'verifier.tokenRuntimeCodeHash differs')
        require(int(call_word(verifier, 'latestFinalizedHeight()'), 16) == FIXTURE_CHECKPOINT['initialFinalizedHeight'],
                'verifier.latestFinalizedHeight is not the seeded height')
        require(call_word(verifier, 'latestCheckpointCommitment()') == FIXTURE_CHECKPOINT['initialCheckpointCommitment'],
                'verifier.latestCheckpointCommitment is not the seeded checkpoint')
        require(int(call_word(verifier, 'arbitrumChainId()'), 16) == CHAIN_ID, 'verifier chain id is not 84532')
        require((pinned['lock_contract_address'], pinned['lock_runtime_code_hash'],
                 pinned['token_address'], pinned['token_runtime_code_hash']) ==
                (lock, deployed_lock_code_hash, token, token_code_hash), 'the genesis pins do not equal the deployed lock')

        deployment = {
            'chain_id': CHAIN_ID,
            'deployer': deployer, 'token_deployer': token_deployer,
            'deployer_nonce_verifier': planned_nonce, 'deployer_nonce_lock': planned_nonce + 1,
            'token': token, 'token_runtime_code_hash': token_code_hash,
            'predicted': {'verifier': predicted_verifier, 'lock': predicted_lock,
                          'verifier_runtime_code_hash': predicted_verifier_code_hash,
                          'lock_runtime_code_hash': predicted_lock_code_hash},
            'deployed': {'verifier': verifier, 'lock': lock,
                         'verifier_runtime_code_hash': deployed_verifier_code_hash,
                         'lock_runtime_code_hash': deployed_lock_code_hash},
            'code_hash_method': 'deploy on an evm_snapshot, read eth_getCode, evm_revert',
            'lock_constructor': {'maxLockAmount': MAX_LOCK_AMOUNT, 'minimumLockAmount': MINIMUM_LOCK_AMOUNT},
            'verifier_fixture_checkpoint': FIXTURE_CHECKPOINT,
            'verifier_sp1_verifier': FIXTURE_SP1_VERIFIER,
            'genesis_fixture': GENESIS_FIXTURE,
            'genesis_sha256': sha256_file(genesis_path),
            'genesis_lane_policy': pinned,
            'genesis_parse_check': 'postfiat-node init --genesis-file validated the genesis; the lane pins were read back '
                                   'from the data directory',
            'pftl_genesis_hash': init_report.get('genesis_hash'),
            'pftl_chain_id': init_report.get('chain_id'),
        }
        results['deployment'] = deployment
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
        (out / 'summary.json').write_text(json.dumps(results, indent=2) + '\n')
        manifest = build_manifest(out, commit, tree, results.get('toolchain'), results.get('deployment'))
        (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        print(json.dumps({key: results.get(key) for key in ('passed', 'deployment', 'nonce_off_by_one', 'error')},
                         indent=2))
    return 0 if results['passed'] else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except RehearsalFailure as refusal:
        print(f'rehearse-phase3-deployment-order: {refusal}', file=sys.stderr)
        sys.exit(2)
