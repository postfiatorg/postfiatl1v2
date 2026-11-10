#!/usr/bin/env python3
"""Relay one PFT lane export from PFTL to the Base lock (design note, "Base half, step 4").

`release` checks before sending every revert the chain state shows: sizes,
program version, bindings and pins, live lock and token code, prior
checkpoint and committee, consumed withdrawal and nullifier, zero recipient,
withdrawal height, and the amount against locked minus released. No pre-send
check exists for the lock's consumed burn-tx commitment (a private mapping
without a getter; the verifier's withdrawal and nullifier records cover the
same packet), for the SP1 proof itself, or for a race with another relay
between the reads and the send (the losing transaction reverts and costs gas
only).

Steps, as the A666 relay: wait until the export's PFTL block is final (`input`),
prove (`prove`), call `release` on the lock (`release`), and wait for the
receipt. `advance-checkpoint` moves the verifier's checkpoint with the
Checkpoint variant; `status` reports one packet.

`prove --mode execute-only` runs the guest in SP1's CPU executor and writes a
PLACEHOLDER proof: only a verifier constructed with a mock SP1 verifier accepts
it. `release` sends it only when the runtime code hash at the verifier's
`sp1Verifier` equals the mock artifact's (`--mock-sp1-artifact`) AND the RPC is
a loopback host whose client reports anvil, or `--i-know-this-is-a-mock-verifier`
names the lock's verifier. The chain id alone proves nothing: the rehearsal's
own Anvil uses 84532, Base Sepolia's public chain id. A private key is
accepted for a loopback Anvil only; elsewhere use `--account` and
`--password-file` (a Foundry keystore). Keys are never written or printed:
the pinned cast (Foundry 1.8.1) reads no raw private key from its environment,
so on a loopback Anvil the key is a child argument, and every child failure,
timeout included, is reported with the key replaced by `<redacted>`.

Cost: one proof carries at most 64 finality ancestry blocks
(`PFUSDC_EGRESS_MAX_ANCESTRY_BLOCKS_V1`), so the verifier's checkpoint must
advance at least every 64 PFTL blocks (releases or `advance-checkpoint`).
Measured guest instructions: 33.6 M (4 validators, no ancestry), 43.0 M
(4, one block), 48.9 M (6, none), 76.7 M (6, two blocks).
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def _load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Keccak-256 and static calldata of the deployment-order rehearsal, imported, not copied.
_ORDER = _load('rehearse_phase3_deployment_order', 'scripts/rehearse-phase3-deployment-order.py')
keccak256 = _ORDER.keccak256
call_data = _ORDER.call_data

PLACEHOLDER_PROOF = b'PFT-LANE-EXPORT-PLACEHOLDER: NOT AN SP1 PROOF; ONLY A MOCK SP1 VERIFIER ACCEPTS IT'
PROOF_SCHEMA = 'postfiat.pft_lane_export_relay.proof.v1'
RELEASED_TOPIC = '0x' + keccak256(b'Released(bytes32,bytes32,bytes32,address,uint256)').hex()


class RelayRefusal(RuntimeError):
    """The relay refuses: sending would revert, or an input is unsafe."""


def require(condition, message):
    """Raise unless `condition` holds; unlike `assert`, PYTHONOPTIMIZE keeps it."""
    if not condition:
        raise RelayRefusal(message)


def commitment(hex48):
    """keccak256 over the 48 raw bytes of a PFTL hash, as the verifier stores it."""
    raw = bytes.fromhex(hex48.removeprefix('0x'))
    require(len(raw) == 48, f'{hex48!r} is not a 48-byte PFTL hash')
    return '0x' + keccak256(raw).hex()


def word_hex(value):
    return '0x' + value.lower().removeprefix('0x').rjust(64, '0')


def confined(path, *, fresh=False):
    """An output path: never inside the checkout's docs/, fresh when asked."""
    resolved = Path(path).resolve()
    require(not resolved.is_relative_to(ROOT / 'docs'), f'{resolved} is inside docs/; write run output elsewhere')
    if fresh:
        require(not resolved.exists(), f'{resolved} already exists; name a fresh directory')
    return resolved


def is_loopback_url(url):
    host = urllib.parse.urlparse(url).hostname or ''
    try:
        return host == 'localhost' or ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


class Rpc:
    """Minimal JSON-RPC client; tests replace it with a mock exposing the same calls."""

    def __init__(self, url):
        self.url = url

    def request(self, method, params=()):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode()
        req = urllib.request.Request(self.url, data=body, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=20) as response:
            value = json.load(response)
        if 'error' in value:
            raise RelayRefusal(f'{method}: {value["error"]}')
        return value['result']

    def chain_id(self):
        return int(self.request('eth_chainId'), 16)

    def client_version(self):
        return str(self.request('web3_clientVersion'))

    def code_hash(self, address):
        return code_hash_at(self, address)

    def call(self, address, signature, *args):
        data = self.request('eth_call', [{'to': address, 'data': call_data(signature, *args)}, 'latest'])
        require(len(data) == 66, f'{signature} on {address} returned {data}')
        return data.lower()


def is_local_anvil(rpc):
    """(loopback RPC host, client reports anvil). Neither, nor the chain id,
    shows what the verifier accepts: the rehearsal's own Anvil uses 84532,
    Base Sepolia's public chain id."""
    return is_loopback_url(rpc.url), 'anvil' in rpc.client_version().lower()


def artifact_code_hash(artifact):
    """keccak256 of a Foundry artifact's runtime code (the mock has no immutables)."""
    return '0x' + keccak256(bytes.fromhex(artifact['deployedBytecode']['object'].removeprefix('0x'))).hex()


def code_hash_at(rpc, address):
    return '0x' + keccak256(bytes.fromhex(rpc.request('eth_getCode', [address, 'latest']).removeprefix('0x'))).hex()


def as_int(word):
    return int(word, 16)


def as_address(word):
    return '0x' + word[-40:]


def read_release_state(rpc, lock, values):
    """Everything `release` depends on, read before sending."""
    verifier = as_address(rpc.call(lock, 'finalityVerifier()'))
    prior = commitment(values['prior_checkpoint_block_id'])
    withdrawal = commitment(values['withdrawal_id'])
    reads = {
        'chain_id': rpc.chain_id(),
        'lock': lock.lower(),
        'lock.finalityVerifier': verifier,
        'lock.token': as_address(rpc.call(lock, 'token()')),
        'lock.totalLocked': as_int(rpc.call(lock, 'totalLocked()')),
        'lock.totalReleased': as_int(rpc.call(lock, 'totalReleased()')),
    }
    for getter in ('pftlChainIdHash', 'pftlGenesisHashCommitment', 'routeProfileHashCommitment', 'assetIdCommitment',
                   'vaultRuntimeCodeHash', 'tokenRuntimeCodeHash', 'latestCheckpointCommitment'):
        reads[f'verifier.{getter}'] = rpc.call(verifier, f'{getter}()')
    for getter in ('pftlProtocolVersion', 'routeEpoch', 'arbitrumChainId', 'latestFinalizedHeight'):
        reads[f'verifier.{getter}'] = as_int(rpc.call(verifier, f'{getter}()'))
    reads['verifier.token'] = as_address(rpc.call(verifier, 'token()'))
    reads['verifier.sp1Verifier'] = as_address(rpc.call(verifier, 'sp1Verifier()'))
    reads['verifier.sp1VerifierCodeHash'] = rpc.code_hash(reads['verifier.sp1Verifier'])
    reads['verifier.programVKey'] = rpc.call(verifier, 'programVKey()')
    reads['verifier.maxProofBytes'] = as_int(rpc.call(verifier, 'maxProofBytes()'))
    reads['verifier.maxPublicValuesBytes'] = as_int(rpc.call(verifier, 'maxPublicValuesBytes()'))
    reads['lock.liveCodeHash'] = rpc.code_hash(lock)
    reads['token.liveCodeHash'] = rpc.code_hash(reads['lock.token'])
    reads['verifier.priorCommitteeRootCommitment'] = rpc.call(verifier, 'checkpointCommitteeRootCommitment(bytes32)',
                                                               prior)
    reads['verifier.withdrawalConsumed'] = as_int(rpc.call(verifier, 'consumedWithdrawalIdCommitment(bytes32)',
                                                           withdrawal)) != 0
    reads['verifier.nullifierConsumed'] = as_int(rpc.call(verifier, 'consumedProofNullifier(bytes32)',
                                                          word_hex(values['proof_nullifier']))) != 0
    return reads


def release_refusals(reads, values, *, proof_bytes, public_values_bytes, expected):
    """The reverts of `release(publicValues, proof)` that the reads and the
    public values can show before sending; an empty list means none of them
    applies. Not checkable here: the lock's consumed burn-tx commitment (a
    private mapping without a getter; the verifier's withdrawal and nullifier
    records are checked instead), the SP1 proof itself, and a race with
    another relay between these reads and the send (it costs gas only)."""
    refusals = []

    def check(condition, code, message):
        if not condition:
            refusals.append({'code': code, 'message': message})

    lock = reads['lock']
    check(0 < proof_bytes <= reads['verifier.maxProofBytes'], 'proof_too_large',
          f'the proof has {proof_bytes} bytes; the verifier takes 1 to {reads["verifier.maxProofBytes"]}')
    check(0 < public_values_bytes <= reads['verifier.maxPublicValuesBytes'], 'public_values_too_large',
          f'the public values have {public_values_bytes} bytes; the verifier takes up to '
          f'{reads["verifier.maxPublicValuesBytes"]}')
    check(values['proof_program_version'] == 1, 'wrong_proof_program_version',
          f'proof program version {values["proof_program_version"]}, the verifier takes 1')
    check(int(values['recipient'], 16) != 0, 'zero_recipient', 'the recipient is the zero address')
    check(0 < values['withdrawal_finalized_height'] <= values['finalized_block_height'], 'wrong_withdrawal_height',
          'the export height is zero or above the finalized height')
    check(reads['lock.liveCodeHash'] == reads['verifier.vaultRuntimeCodeHash'], 'live_lock_code',
          'the lock\'s live runtime code hash is not the one the verifier pins')
    check(reads['token.liveCodeHash'] == reads['verifier.tokenRuntimeCodeHash']
          == word_hex(values['token_runtime_code_hash']), 'live_token_code',
          'the token\'s live runtime code hash differs from the verifier\'s pin or the public values')
    if expected.get('program_vkey'):
        check(reads['verifier.programVKey'] == word_hex(expected['program_vkey']), 'wrong_program_vkey',
              f'the verifier\'s programVKey is {reads["verifier.programVKey"]}, expected {expected["program_vkey"]}')
    if expected.get('sp1_verifier'):
        check(reads['verifier.sp1Verifier'] == expected['sp1_verifier'].lower(), 'wrong_sp1_verifier',
              f'the verifier\'s sp1Verifier is {reads["verifier.sp1Verifier"]}, expected {expected["sp1_verifier"]}')
    check(values['vault_address'].lower() == lock, 'wrong_vault',
          f'the public values pay from {values["vault_address"]}, not this lock {lock}')
    check(values['token_address'].lower() == reads['lock.token'] == reads['verifier.token'], 'wrong_token',
          f'token {values["token_address"]} differs from the lock\'s {reads["lock.token"]} or the verifier\'s')
    check(values['arbitrum_chain_id'] == reads['chain_id'] == reads['verifier.arbitrumChainId'], 'wrong_chain_id',
          f'chain id {values["arbitrum_chain_id"]} differs from the chain\'s {reads["chain_id"]} or the verifier\'s')
    check('0x' + keccak256(values['pftl_chain_id'].encode()).hex() == reads['verifier.pftlChainIdHash'],
          'wrong_pftl_chain', 'the PFTL chain id is not the one the verifier pins')
    check(commitment(values['pftl_genesis_hash']) == reads['verifier.pftlGenesisHashCommitment'],
          'wrong_genesis', 'the PFTL genesis hash is not the one the verifier pins')
    check(values['pftl_protocol_version'] == reads['verifier.pftlProtocolVersion'], 'wrong_protocol_version',
          'the protocol version is not the one the verifier pins')
    check(commitment(values['route_profile_hash']) == reads['verifier.routeProfileHashCommitment']
          and values['route_epoch'] == reads['verifier.routeEpoch'], 'wrong_route_profile',
          'the route profile hash or epoch is not the one the verifier pins')
    check(commitment(values['asset_id']) == reads['verifier.assetIdCommitment'], 'wrong_asset_id',
          'the asset id is not the one the verifier pins')
    check(word_hex(values['vault_runtime_code_hash']) == reads['verifier.vaultRuntimeCodeHash'], 'wrong_vault_code',
          'the lock code hash is not the one the verifier pins')
    check(word_hex(values['token_runtime_code_hash']) == reads['verifier.tokenRuntimeCodeHash'], 'wrong_token_code',
          'the token code hash is not the one the verifier pins')
    prior_root = reads['verifier.priorCommitteeRootCommitment']
    check(as_int(prior_root) != 0, 'unknown_prior_checkpoint',
          f'the verifier has not accepted the prior checkpoint {values["prior_checkpoint_block_id"]}')
    if as_int(prior_root) != 0 and not values['committee_transition_commitment']:
        check(commitment(values['committee_root']) == prior_root, 'wrong_committee_root',
              'the committee root differs from the one recorded for the prior checkpoint')
    check(not reads['verifier.withdrawalConsumed'], 'already_consumed',
          f'withdrawal id {values["withdrawal_id"]} is already consumed')
    check(not reads['verifier.nullifierConsumed'], 'proof_already_consumed', 'the proof nullifier is already consumed')
    available = reads['lock.totalLocked'] - reads['lock.totalReleased']
    check(0 < values['amount_atoms'] <= available, 'above_locked',
          f'amount {values["amount_atoms"]} is above locked minus released ({available})')
    return refusals


def mock_proof_refusal(proof, *, loopback, anvil, sp1_code_hash, mock_code_hash, lock_verifier, acknowledged):
    """The rule for a placeholder proof: the verifier the lock calls must use the
    repository's mock SP1 verifier (the runtime code hash at its `sp1Verifier`
    equals the mock artifact's), AND either the RPC is a loopback Anvil or the
    operator names that verifier with --i-know-this-is-a-mock-verifier."""
    if not proof.get('mock_verifier_only'):
        return None
    if mock_code_hash is None:
        return 'a placeholder proof needs --mock-sp1-artifact (the Foundry artifact of Tier4MockSP1Verifier)'
    if sp1_code_hash != mock_code_hash:
        return ('the verifier\'s sp1Verifier is not the mock SP1 verifier (runtime code hash '
                f'{sp1_code_hash}, mock {mock_code_hash}); a placeholder proof would be refused or, worse, mean '
                'a real verifier is misconfigured')
    if loopback and anvil:
        return None
    if acknowledged and acknowledged.lower() == lock_verifier.lower():
        return None
    return ('the proof is a placeholder and the RPC is not a loopback Anvil; pass '
            f'--i-know-this-is-a-mock-verifier {lock_verifier} to send it to that mock-SP1 verifier anyway')


def signer_args(args, local_anvil):
    """The `cast send` signer flags: a private key only on a loopback Anvil."""
    if args.private_key_file:
        require(local_anvil, '--private-key-file is accepted for a loopback Anvil only; use --account and '
                             '--password-file')
        key = Path(args.private_key_file).read_text().strip()
        return ['--private-key', key], key
    require(args.account and args.password_file, 'name --account and --password-file (or, on a loopback Anvil, '
                                                 '--private-key-file)')
    return ['--account', args.account, '--password-file', str(args.password_file)], None


def decode_released(receipt, lock):
    """The `Released` log of a receipt: withdrawal, burn and packet digest, recipient, amount."""
    logs = [log for log in receipt.get('logs', []) if log['address'].lower() == lock.lower()
            and log['topics'] and log['topics'][0].lower() == RELEASED_TOPIC]
    require(len(logs) == 1, f'the receipt carries {len(logs)} Released events, expected 1')
    log = logs[0]
    data = log['data'].removeprefix('0x')
    return {'withdrawal_id_commitment': log['topics'][1], 'burn_tx_id_commitment': log['topics'][2],
            'packet_digest': log['topics'][3], 'recipient': '0x' + data[24:64], 'amount': int(data[64:128], 16)}


def mock_code_hash(args):
    if not args.mock_sp1_artifact:
        return None
    return artifact_code_hash(json.loads(Path(args.mock_sp1_artifact).read_text()))


def release_result(receipt, lock, values, *, balance_before, balance_after):
    """The result of a sent release: the `Released` event the expected lock
    emitted in a successful receipt, with the recipient, amount and the three
    commitments of these public values. The recipient's balance change only
    corroborates it: an unrelated transfer between the two reads, or a
    recipient equal to the lock, moves it, so a difference is a warning."""
    require(receipt.get('status') in ('0x1', 1), f'release transaction failed: {receipt.get("status")}')
    released = decode_released(receipt, lock)
    expected = {'withdrawal_id_commitment': commitment(values['withdrawal_id']),
                'burn_tx_id_commitment': commitment(values['burn_tx_id']),
                'packet_digest': word_hex(values['withdrawal_packet_digest']),
                'recipient': values['recipient'].lower(), 'amount': values['amount_atoms']}
    for field, want in expected.items():
        require(str(released[field]).lower() == str(want).lower(), f'Released names another {field}')
    delta = balance_after - balance_before
    warnings = [] if delta == values['amount_atoms'] else [
        f'the recipient balance moved by {delta}, not {values["amount_atoms"]}; the Released event is the result']
    return {'released': released, 'recipient_balance_before': balance_before, 'recipient_balance_after': balance_after,
            'recipient_balance_delta': delta, 'warnings': warnings}


def checkpoint_report(reads, values):
    """The prior checkpoint must be one the verifier accepted; the latest one
    says how far behind the verifier is."""
    return {'prior_checkpoint_commitment': commitment(values['prior_checkpoint_block_id']),
            'latest_checkpoint_commitment': reads['verifier.latestCheckpointCommitment'],
            'prior_is_latest': commitment(values['prior_checkpoint_block_id'])
            == reads['verifier.latestCheckpointCommitment'],
            'latest_finalized_height': reads['verifier.latestFinalizedHeight'],
            'proof_finalized_height': values['finalized_block_height']}


def load_values(input_dir):
    document = json.loads((Path(input_dir) / 'expected-public-values.json').read_text())
    return document['expected_public_values'], document['expected_public_values_hex']


def redact(text, secret):
    return text.replace(secret, '<redacted>') if secret else text


def run(argv, *, secret=None, timeout=1800, **kwargs):
    """A child process. A timeout or an OS error becomes a refusal whose text,
    like the child's output, has `secret` replaced: the exceptions' own text
    carries the whole command line."""
    try:
        p = subprocess.run([str(a) for a in argv], capture_output=True, text=True, timeout=timeout, **kwargs)
    except subprocess.TimeoutExpired as error:
        raise RelayRefusal(redact(f'{error}', secret)) from None
    except OSError as error:
        raise RelayRefusal(redact(f'{argv[0]}: {error}', secret)) from None
    if secret:
        p.stdout, p.stderr = redact(p.stdout or '', secret), redact(p.stderr or '', secret)
    return p


# --- subcommands -------------------------------------------------------------


def cmd_input(args):
    out = confined(args.output, fresh=True)
    selector = ['--export-nonce', str(args.export_nonce)] if args.export_nonce is not None else ['--tx-id', args.tx_id]
    argv = [args.node_bin, 'external-lane-export-proof-input', '--data-dir', args.data_dir, '--lane-id', args.lane_id,
            *selector, '--prior-checkpoint', args.prior_checkpoint, '--output-dir', out]
    deadline = time.time() + args.wait_seconds
    while True:
        p = run(argv)
        if p.returncode == 0:
            break
        # Refuse until the export's block is final; any other error stops at once.
        require('not final yet' in p.stderr and time.time() < deadline,
                f'external-lane-export-proof-input refused: {p.stderr.strip()}')
        time.sleep(1)
    report = json.loads(p.stdout)
    print(json.dumps(report, indent=2))
    return report


def cmd_prove(args):
    input_dir = Path(args.input)
    values, expected_hex = load_values(input_dir)
    out = confined(args.output)
    out.mkdir(parents=True, exist_ok=True)
    proof = {'schema': PROOF_SCHEMA, 'mode': args.mode, 'public_values_hex': expected_hex,
             'public_values_sha256': hashlib.sha256(bytes.fromhex(expected_hex)).hexdigest(),
             'packet_digest': values['withdrawal_packet_digest'], 'withdrawal_id': values['withdrawal_id']}
    if args.mode == 'execute-only':
        require(args.elf and args.prover_tool, 'execute-only needs --elf and --prover-tool')
        p = run([args.prover_tool, 'execute', '--elf', args.elf, '--input', input_dir / 'input.cbor',
                 '--output-dir', out / 'execute'], env=dict(os.environ, SP1_PROVER='cpu'))
        require(p.returncode == 0, f'SP1 execute failed: {p.stderr.strip()[-2000:]}')
        executed = (out / 'execute' / 'public-values.bin').read_bytes().hex()
        require(executed == expected_hex, 'the guest committed other bytes than the expected public values')
        report = json.loads((out / 'execute' / 'execute-report.json').read_text())
        proof.update(proof_hex='0x' + PLACEHOLDER_PROOF.hex(), mock_verifier_only=True,
                     note='PLACEHOLDER: the guest ran in SP1 execute mode only; no proof was generated. Only a '
                          'verifier constructed with a mock SP1 verifier accepts these bytes.',
                     instruction_count=report['instruction_count'],
                     elf_sha256=hashlib.sha256(Path(args.elf).read_bytes()).hexdigest())
    else:
        # The prover-command contract: the command gets the CBOR input and an output path in its
        # environment, writes the raw SP1 proof bytes there and the committed public values beside
        # them (`<proof>.public-values`), and exits 0. The relay checks the public values.
        require(args.prover_command, '--mode command needs --prover-command')
        proof_path = out / 'proof.bin'
        env = dict(os.environ, PFT_LANE_EXPORT_INPUT=str(input_dir / 'input.cbor'),
                   PFT_LANE_EXPORT_PROOF_OUT=str(proof_path))
        try:
            p = subprocess.run(args.prover_command, shell=True, env=env, capture_output=True, text=True,
                               timeout=36000)
        except (subprocess.TimeoutExpired, OSError) as error:
            raise RelayRefusal(f'prover command failed: {error}') from None
        require(p.returncode == 0, f'prover command failed: {p.stderr.strip()[-2000:]}')
        committed = Path(str(proof_path) + '.public-values').read_bytes().hex()
        require(committed == expected_hex, 'the prover committed other bytes than the expected public values')
        require(proof_path.stat().st_size > 0, 'the prover command wrote an empty proof')
        proof.update(proof_hex='0x' + proof_path.read_bytes().hex(), mock_verifier_only=False)
    (out / 'proof.json').write_text(json.dumps(proof, indent=2) + '\n')
    print(json.dumps({key: value for key, value in proof.items() if key != 'public_values_hex'}, indent=2))
    return proof


def cmd_release(args):
    rpc = Rpc(args.rpc)
    values, expected_hex = load_values(args.input)
    proof = json.loads(Path(args.proof).read_text())
    require(proof.get('schema') == PROOF_SCHEMA and proof['public_values_hex'] == expected_hex,
            'the proof file is not for these public values')
    loopback, anvil = is_local_anvil(rpc)
    local = loopback and anvil
    reads = read_release_state(rpc, args.lock, values)
    refusal = mock_proof_refusal(proof, loopback=loopback, anvil=anvil,
                                 sp1_code_hash=reads['verifier.sp1VerifierCodeHash'],
                                 mock_code_hash=mock_code_hash(args), lock_verifier=reads['lock.finalityVerifier'],
                                 acknowledged=args.i_know_this_is_a_mock_verifier)
    require(refusal is None, refusal or '')
    proof_bytes = len(bytes.fromhex(proof['proof_hex'].removeprefix('0x')))
    refusals = release_refusals(reads, values, proof_bytes=proof_bytes,
                                public_values_bytes=len(bytes.fromhex(expected_hex)),
                                expected={'program_vkey': args.expect_program_vkey,
                                          'sp1_verifier': args.expect_sp1_verifier})
    result = {'reads_before': reads, 'checkpoint': checkpoint_report(reads, values), 'refusals': refusals,
              'sent': False}
    if refusals and not args.force_send:
        print(json.dumps(result, indent=2))
        raise RelayRefusal('release would revert: ' + '; '.join(r['code'] for r in refusals))
    recipient = values['recipient']
    balance_before = as_int(rpc.call(reads['lock.token'], 'balanceOf(address)', recipient))
    signer, secret = signer_args(args, local)
    p = run(['cast', 'send', args.lock, 'release(bytes,bytes)', '0x' + expected_hex, proof['proof_hex'],
             '--rpc-url', args.rpc, '--json', *signer], secret=secret)
    stderr = p.stderr
    result['sent'] = True
    if p.returncode != 0:
        result['reverted'] = stderr.strip()[-1500:]
        print(json.dumps(result, indent=2))
        raise RelayRefusal('release reverted on chain')
    receipt = json.loads(p.stdout)
    balance_after = as_int(rpc.call(reads['lock.token'], 'balanceOf(address)', recipient))
    result.update(release_result(receipt, args.lock, values, balance_before=balance_before,
                                 balance_after=balance_after))
    result.update(transaction=receipt['transactionHash'], block=int(str(receipt['blockNumber']), 0),
                  lock_total_released_after=as_int(rpc.call(args.lock, 'totalReleased()')))
    for warning in result['warnings']:
        print(f'pft-lane-export-relay: warning: {warning}', file=sys.stderr)
    print(json.dumps(result, indent=2))
    return result


def cmd_advance_checkpoint(args):
    """The Checkpoint variant: witness from the node, execute, then `advanceCheckpoint`."""
    out = confined(args.output, fresh=True)
    out.mkdir(parents=True)
    p = run([args.node_bin, 'pfusdc-checkpoint-witness', '--data-dir', args.data_dir,
             '--prior-checkpoint', args.prior_checkpoint, '--target-block', args.target_block])
    require(p.returncode == 0, f'pfusdc-checkpoint-witness refused: {p.stderr.strip()}')
    (out / 'input.json').write_text(json.dumps({'kind': 'checkpoint', 'witness': json.loads(p.stdout)}))
    p = run([args.prover_tool, 'execute', '--elf', args.elf, '--input-json', out / 'input.json',
             '--output-dir', out / 'execute'], env=dict(os.environ, SP1_PROVER='cpu'))
    require(p.returncode == 0, f'SP1 execute failed: {p.stderr.strip()[-2000:]}')
    public_values = '0x' + (out / 'execute' / 'public-values.bin').read_bytes().hex()
    rpc = Rpc(args.rpc)
    loopback, anvil = is_local_anvil(rpc)
    local = loopback and anvil
    verifier = as_address(rpc.call(args.lock, 'finalityVerifier()'))
    refusal = mock_proof_refusal({'mock_verifier_only': True}, loopback=loopback, anvil=anvil,
                                 sp1_code_hash=code_hash_at(rpc, as_address(rpc.call(verifier, 'sp1Verifier()'))),
                                 mock_code_hash=mock_code_hash(args), lock_verifier=verifier,
                                 acknowledged=args.i_know_this_is_a_mock_verifier)
    require(refusal is None, refusal or '')
    require(as_int(rpc.call(verifier, 'acceptedCheckpointCommitment(bytes32)', commitment(args.prior_checkpoint))),
            'the verifier has not accepted the prior checkpoint')
    signer, secret = signer_args(args, local)
    p = run(['cast', 'send', verifier, 'advanceCheckpoint(bytes,bytes)', public_values, '0x' + PLACEHOLDER_PROOF.hex(),
             '--rpc-url', args.rpc, '--json', *signer], secret=secret)
    require(p.returncode == 0, 'advanceCheckpoint reverted: ' + p.stderr.strip()[-1500:])
    latest = rpc.call(verifier, 'latestCheckpointCommitment()')
    require(latest == commitment(args.target_block), 'the verifier did not move to the target checkpoint')
    result = {'transaction': json.loads(p.stdout)['transactionHash'], 'latest_checkpoint_commitment': latest}
    print(json.dumps(result, indent=2))
    return result


def cmd_status(args):
    """One packet: exported (in the input), final (input built), proven, released (Base)."""
    values, expected_hex = load_values(args.input)
    proven = False
    if args.proof and Path(args.proof).exists():
        proven = json.loads(Path(args.proof).read_text()).get('public_values_hex') == expected_hex
    status = {'packet_digest': values['withdrawal_packet_digest'], 'exported': True, 'final': True, 'proven': proven}
    rpc = Rpc(args.rpc)
    verifier = as_address(rpc.call(args.lock, 'finalityVerifier()'))
    consumed = as_int(rpc.call(verifier, 'consumedWithdrawalIdCommitment(bytes32)', commitment(values['withdrawal_id'])))
    status['released'] = consumed != 0
    if consumed:
        logs = rpc.request('eth_getLogs', [{'address': args.lock, 'fromBlock': hex(args.from_block), 'toBlock': 'latest',
                                            'topics': [RELEASED_TOPIC, None, None,
                                                       word_hex(values['withdrawal_packet_digest'])]}])
        status['base_transactions'] = [log['transactionHash'] for log in logs]
    print(json.dumps(status, indent=2))
    return status


def parser():
    top = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = top.add_subparsers(dest='command', required=True)

    p = sub.add_parser('input', help='write the proof input once the export block is final')
    p.add_argument('--node-bin', type=Path, required=True)
    p.add_argument('--data-dir', type=Path, required=True)
    p.add_argument('--lane-id', required=True)
    which = p.add_mutually_exclusive_group(required=True)
    which.add_argument('--export-nonce', type=int)
    which.add_argument('--tx-id')
    p.add_argument('--prior-checkpoint', required=True, help="the verifier's latest accepted PFTL block id")
    p.add_argument('--wait-seconds', type=int, default=0, help='retry while the export is not final yet')
    p.add_argument('--output', type=Path, required=True)
    p.set_defaults(func=cmd_input)

    p = sub.add_parser('prove', help='prove (or, execute-only, run the guest and write a placeholder)')
    p.add_argument('--input', type=Path, required=True)
    p.add_argument('--mode', choices=('execute-only', 'command'), required=True)
    p.add_argument('--elf', type=Path)
    p.add_argument('--prover-tool', type=Path)
    p.add_argument('--prover-command')
    p.add_argument('--output', type=Path, required=True)
    p.set_defaults(func=cmd_prove)

    def base(p):
        p.add_argument('--rpc', required=True)
        p.add_argument('--lock', required=True)
        p.add_argument('--private-key-file', type=Path, help='loopback Anvil only')
        p.add_argument('--account')
        p.add_argument('--password-file', type=Path)
        p.add_argument('--i-know-this-is-a-mock-verifier', metavar='VERIFIER',
                       help='send a placeholder proof off a loopback Anvil, to this verifier only, and only if its '
                            'sp1Verifier has the mock\'s code hash')
        p.add_argument('--mock-sp1-artifact', type=Path,
                       help='Foundry artifact of Tier4MockSP1Verifier; required to send a placeholder proof')

    p = sub.add_parser('release', help='check the reverts that can be read before sending, then call release and report Released')
    base(p)
    p.add_argument('--input', type=Path, required=True)
    p.add_argument('--proof', type=Path, required=True)
    p.add_argument('--force-send', action='store_true', help='send despite refusals (to show the on-chain revert)')
    p.add_argument('--expect-program-vkey', help="refuse unless the verifier's programVKey is this")
    p.add_argument('--expect-sp1-verifier', help="refuse unless the verifier's sp1Verifier is this")
    p.set_defaults(func=cmd_release)

    p = sub.add_parser('advance-checkpoint', help="move the verifier's checkpoint with the Checkpoint variant")
    base(p)
    p.add_argument('--node-bin', type=Path, required=True)
    p.add_argument('--data-dir', type=Path, required=True)
    p.add_argument('--prior-checkpoint', required=True)
    p.add_argument('--target-block', required=True)
    p.add_argument('--elf', type=Path, required=True)
    p.add_argument('--prover-tool', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.set_defaults(func=cmd_advance_checkpoint)

    p = sub.add_parser('status', help='exported, final, proven, released for one packet')
    p.add_argument('--rpc', required=True)
    p.add_argument('--lock', required=True)
    p.add_argument('--input', type=Path, required=True)
    p.add_argument('--proof', type=Path, help='the proof.json of `prove`; proven when it is for these values')
    p.add_argument('--from-block', type=int, required=True, help='the Base block to search Released logs from')
    p.set_defaults(func=cmd_status)
    return top


def main(argv=None):
    args = parser().parse_args(argv)
    try:
        args.func(args)
    except RelayRefusal as refusal:
        print(f'pft-lane-export-relay: {refusal}', file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
