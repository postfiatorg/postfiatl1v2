#!/usr/bin/env python3
"""Fail-closed B24 launch preflight and mainnet-fork qualification.

No private-key loading and no live transaction transport. A complete launch is
BLOCKED until authentic deposit finality, native signed execution, export proof
and destination acknowledgement can be qualified together. --dry-run stops at
that boundary. --qualify-components also measures LP and Universal Router swap
using an explicitly recorded wrapper-balance fixture; it still exits 2 and
never reports the eleven-step launch as passed.
"""
import argparse
import copy
import hashlib
import json
import math
import os
import socket
import subprocess
import time
import uuid
from datetime import datetime, timezone
from decimal import Decimal, getcontext, ROUND_CEILING, ROUND_FLOOR
from pathlib import Path
from eth_abi import encode, decode
from web3 import Web3
from web3.exceptions import ContractCustomError

COORD = Path('/home/postfiat/repos/b23-coord')
PRIOR = COORD / 'evidence/tee/phase6'
RPC = 'https://ethereum-rpc.publicnode.com'
OPERATOR = Web3.to_checksum_address('0xF77B1876803c01967C5a62EF28C5f6de4574F7E1')
TOKEN = Web3.to_checksum_address('0x540FDa223a2198690d31920a921A0b9DfF335dbF')
USDC = Web3.to_checksum_address('0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48')
PERMIT = Web3.to_checksum_address('0x000000000022D473030F116dDEE9F6B43aC78BA3')
POSITION = Web3.to_checksum_address('0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e')
ROUTER = Web3.to_checksum_address('0x66a9893cc07d91d95644aedd05d03f95e1dba8af')
QUOTER = Web3.to_checksum_address('0x99bd42B2B22e531f283b7F8a86dFf631EB16A9D1')
VIEW = Web3.to_checksum_address('0x7ffe42c4a5deea5b0fec41c94c136cf115597227')
ROUTE = 'pftl-b24-ethereum-wB24-usdc-v1'
POOL = '0x70ab0a6fd1e33c7d3dfeae11a1ec60682efa2a9e95f028df1964b167326cc92c'
KEY_TYPE = '(address,address,uint24,int24,address)'
POOL_KEY = (TOKEN, USDC, 500, 10, '0x' + '00' * 20)
getcontext().prec = 90


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def load(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def hx(value):
    return '0x' + bytes(value).hex()


def encode_call(signature, types=(), args=()):
    return Web3.keccak(text=signature)[:4] + encode(types, args)


def read(w, address, signature, types=(), args=(), outputs=('uint256',)):
    return decode(outputs, w.eth.call({'to': address, 'data': encode_call(signature, types, args)}))


def balance(w, token, owner):
    return read(w, token, 'balanceOf(address)', ['address'], [owner])[0]


def held_locks(path, locks=None):
    """Observe Linux lock ownership without taking even a temporary flock."""
    st = path.stat()
    ident = f'{os.major(st.st_dev):02x}:{os.minor(st.st_dev):02x}:{st.st_ino}'
    return [line for line in (locks if locks is not None else Path('/proc/locks').read_text()).splitlines()
            if ident in line.split()]


def check_coordination(required_holder=None):
    lock = COORD / 'chain.lock'
    holder = lock.read_text().strip()
    if required_holder is None:
        require(not holder and not held_locks(lock), 'chain.lock is occupied; refusing')
    else:
        require(holder == required_holder and bool(held_locks(lock)), 'Expected an existing held TEE lease')
    unit = subprocess.check_output(['systemctl', '--user', 'show', 'b24-tee-nav-daily.service',
                                   '-p', 'LoadState', '-p', 'ActiveState', '-p', 'SubState', '-p', 'MainPID', '-p', 'Result'], text=True)
    state = dict(line.split('=', 1) for line in unit.splitlines() if '=' in line)
    require(state.get('LoadState') == 'loaded', 'Daily service state is unknown')
    require(state.get('MainPID') == '0' and state.get('ActiveState') == 'inactive'
            and state.get('Result') == 'success', 'Daily job is running or needs verified recovery')
    daily_lock = COORD / 'evidence/tee/b24-daily/run.lock'
    require(not daily_lock.exists() or not held_locks(daily_lock), 'Daily run.lock is held')
    now = datetime.now(timezone.utc)
    minute = now.hour * 60 + now.minute
    require(not 17 * 60 + 35 <= minute < 18 * 60 + 30,
            'Daily capture window 17:35–18:30 UTC; refusing to compete with 17:55 job')
    return dict(at=now.isoformat(), chain_lock_free=not holder, chain_lock_acquired=False, daily=state)


class ReadOnlyProvider(Web3.HTTPProvider):
    ALLOWED = {'web3_clientVersion', 'eth_chainId', 'eth_blockNumber', 'eth_getBlockByNumber',
               'eth_getBlockByHash', 'eth_getBalance', 'eth_getCode', 'eth_getTransactionCount',
               'eth_call', 'eth_gasPrice', 'eth_maxPriorityFeePerGas'}

    def make_request(self, method, params):
        require(method in self.ALLOWED, 'Live RPC method forbidden: ' + method)
        return super().make_request(method, params)


class Fork:
    def __init__(self, output, block):
        self.output, self.block, self.process, self.log = output, block, None, None
        self.rows = []

    def __enter__(self):
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        self.log = (self.output / 'anvil.log').open('w')
        self.process = subprocess.Popen(['anvil', '--silent', '--host', '127.0.0.1', '--port', str(port),
                                         '--chain-id', '1', '--fork-url', RPC, '--fork-block-number', str(self.block)],
                                        stdout=self.log, stderr=self.log)
        self.w = Web3(Web3.HTTPProvider(f'http://127.0.0.1:{port}', request_kwargs={'timeout': 60}))
        for _ in range(120):
            if self.process.poll() is not None:
                raise RuntimeError('Anvil exited during startup')
            if self.w.is_connected():
                self.guard()
                return self
            time.sleep(.25)
        self.__exit__(None, None, None)
        raise RuntimeError('Anvil startup timed out')

    def __exit__(self, *_):
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        if self.log:
            self.log.close()

    def guard(self):
        require(self.w.provider.endpoint_uri.startswith('http://127.0.0.1:')
                and 'anvil' in self.w.client_version.lower() and self.w.eth.chain_id == 1,
                'Writes require the local Anvil fork')

    def admin(self, method, args):
        self.guard()
        result = self.w.provider.make_request(method, args)
        require('error' not in result, str(result.get('error')))
        return result.get('result')

    def fixture_balance(self, token, owner, amount, slot):
        old = balance(self.w, token, owner)
        key = Web3.keccak(encode(['address', 'uint256'], [owner, slot]))
        self.admin('anvil_setStorageAt', [token, hx(key), hx(amount.to_bytes(32, 'big'))])
        require(balance(self.w, token, owner) == amount, 'Fixture storage layout changed')
        return dict(token=token, owner=owner, prior_atoms=old, fixture_atoms=amount, slot=slot)

    def send(self, label, sender, to, data):
        self.guard()
        started = time.monotonic()
        tx = {'from': sender, 'to': to, 'data': hx(data), 'value': 0}
        tx['gas'] = math.ceil(self.w.eth.estimate_gas(tx) * 1.3)
        receipt = self.w.eth.wait_for_transaction_receipt(self.w.eth.send_transaction(tx))
        require(receipt.status == 1, label + ' reverted')
        row = dict(label=label, sender=sender, gas_used=receipt.gasUsed, gas_limit=tx['gas'],
                   effective_gas_price_wei=receipt.effectiveGasPrice,
                   cost_wei=receipt.gasUsed * receipt.effectiveGasPrice,
                   wall_seconds=time.monotonic() - started, transaction=tx, tx_hash=hx(receipt.transactionHash))
        self.rows.append(row)
        save(self.output / 'fork-transactions.json', self.rows)
        save(self.output / (label + '-receipt.json'), json.loads(self.w.to_json(receipt)))
        return receipt


def pool(w):
    pid = [bytes.fromhex(POOL[2:])]
    return dict(slot0=list(read(w, VIEW, 'getSlot0(bytes32)', ['bytes32'], pid,
                               ['uint160', 'int24', 'uint24', 'uint24'])),
                liquidity=read(w, VIEW, 'getLiquidity(bytes32)', ['bytes32'], pid, ['uint128'])[0])


def seed_pool(fork, nav):
    w = fork.w
    require(nav == 1000000 and pool(w)['slot0'][0] == 2**96 and pool(w)['liquidity'] == 0,
            'NAV/pool price differs; LP strategy must be requalified')
    retained = balance(w, TOKEN, OPERATOR)
    fixture = fork.fixture_balance(TOKEN, OPERATOR, retained + 5000000000, 5)
    require(balance(w, USDC, OPERATOR) == 2000000000, 'Deposit did not retain exactly 2000 USDC for LP')
    for token, amount in [(TOKEN, 5000000000), (USDC, 2000000000)]:
        fork.send('LP-erc20-' + ('B24' if token == TOKEN else 'USDC'), OPERATOR, token,
                  encode_call('approve(address,uint256)', ['address', 'uint256'], [PERMIT, amount]))
        fork.send('LP-permit2-' + ('B24' if token == TOKEN else 'USDC'), OPERATOR, PERMIT,
                  encode_call('approve(address,address,uint160,uint48)', ['address', 'address', 'uint160', 'uint48'],
                              [token, POSITION, amount, w.eth.get_block('latest')['timestamp'] + 3600]))
    positions = []
    for lo, hi, label in [(-210, 200, 'LP-centered'), (0, 200, 'LP-ask')]:
        before = (balance(w, TOKEN, OPERATOR), balance(w, USDC, OPERATOR))
        a0, a1 = before[0] - retained, before[1]
        sa, sb, s = Decimal('1.0001')**(Decimal(lo)/2), Decimal('1.0001')**(Decimal(hi)/2), Decimal(1)
        liquidity = int(Decimal(a0)*sa*sb/(sb-sa) if s <= sa else
                        min(Decimal(a0)*s*sb/(sb-s), Decimal(a1)/(s-sa))) - 1
        require(liquidity > 0, 'No liquidity can be seeded')
        params = [encode([KEY_TYPE, 'int24', 'int24', 'uint256', 'uint128', 'uint128', 'address', 'bytes'],
                         [POOL_KEY, lo, hi, liquidity, a0, a1, OPERATOR, b'']),
                  encode(['address', 'address'], [TOKEN, USDC])]
        token_id = read(w, POSITION, 'nextTokenId()')[0]
        fork.send(label, OPERATOR, POSITION, encode_call('modifyLiquidities(bytes,uint256)', ['bytes', 'uint256'],
                  [encode(['bytes', 'bytes[]'], [bytes.fromhex('020d'), params]), w.eth.get_block('latest')['timestamp'] + 600]))
        require(read(w, POSITION, 'ownerOf(uint256)', ['uint256'], [token_id], ['address'])[0].lower() == OPERATOR.lower(), 'LP NFT owner differs')
        require(read(w, POSITION, 'getPositionLiquidity(uint256)', ['uint256'], [token_id], ['uint128'])[0] == liquidity, 'LP liquidity differs')
        positions.append(dict(label=label, ticks=[lo, hi], token_id=token_id, liquidity=liquidity,
                              wrapped_atoms=before[0]-balance(w, TOKEN, OPERATOR),
                              USDC_atoms=before[1]-balance(w, USDC, OPERATOR)))
    require(balance(w, TOKEN, OPERATOR) == retained and balance(w, USDC, OPERATOR) == 0, 'Unexpected LP dust or spent test inventory')
    return dict(wrapper_fixture=fixture, positions=positions, retained_test_B24_atoms=retained,
                exact_LP_debits=True, authentic_mint=False, native_supply_invariant_tested=False)


def acceptance_swap(fork, slippage_bps):
    w = fork.w
    buyer = Web3.to_checksum_address(Web3.keccak(text='ready-buyer-' + uuid.uuid4().hex)[-20:].hex())
    require(w.eth.get_transaction_count(buyer) == 0 and balance(w, USDC, buyer) == 0 and balance(w, TOKEN, buyer) == 0, 'Buyer not fresh')
    fork.admin('anvil_impersonateAccount', [buyer])
    fork.admin('anvil_setBalance', [buyer, hex(10**17)])
    fixture = fork.fixture_balance(USDC, buyer, 2000000000, 9)
    fork.send('buyer-erc20-approval', buyer, USDC, encode_call('approve(address,uint256)', ['address', 'uint256'], [PERMIT, 2000000000]))
    fork.send('buyer-permit2-approval', buyer, PERMIT, encode_call('approve(address,address,uint160,uint48)',
              ['address', 'address', 'uint160', 'uint48'], [USDC, ROUTER, 2000000000, w.eth.get_block('latest')['timestamp'] + 600]))
    params_type = '(' + KEY_TYPE + ',bool,uint128,bytes)'
    quote, quote_gas = read(w, QUOTER, 'quoteExactInputSingle(' + params_type + ')', [params_type],
                           [(POOL_KEY, False, 2000000000, b'')], ['uint256', 'uint256'])
    minimum = quote * (10000 - slippage_bps) // 10000
    require(quote > 0 and minimum > 0, 'Invalid swap quote/minimum')
    def calldata(min_out):
        params = [encode(['(' + KEY_TYPE + ',bool,uint128,uint128,bytes)'], [(POOL_KEY, False, 2000000000, min_out, b'')]),
                  encode(['address', 'uint256'], [USDC, 2000000000]), encode(['address', 'uint256'], [TOKEN, min_out])]
        return encode_call('execute(bytes,bytes[],uint256)', ['bytes', 'bytes[]', 'uint256'],
                           [b'\x10', [encode(['bytes', 'bytes[]'], [bytes.fromhex('060c0f'), params])],
                            w.eth.get_block('latest')['timestamp'] + 300])
    # eth_call checks a stricter minimum fails without changing pool state.
    try:
        w.eth.call({'from': buyer, 'to': ROUTER, 'data': calldata(quote + 1)})
    except ContractCustomError as error:
        expected = encode_call('V4TooLittleReceived(uint256,uint256)', ['uint256', 'uint256'], [quote + 1, quote])
        require(getattr(error, 'data', None) == hx(expected), 'Unexpected custom error: ' + str(error))
    else:
        raise RuntimeError('Above-quote minimum unexpectedly accepted')
    before = pool(w)
    fork.send('acceptance-2000-USDC', buyer, ROUTER, calldata(minimum))
    out = balance(w, TOKEN, buyer)
    require(out == quote and out >= minimum and balance(w, USDC, buyer) == 0, 'Swap fill/debit differs from quote')
    return dict(passed=True, buyer=buyer, buyer_fixture=fixture, router=ROUTER,
                command='0x10', actions='0x060c0f', USDC_in_atoms=2000000000, wrapped_out_atoms=out,
                quote_atoms=quote, minimum_out_atoms=minimum, slippage_tolerance_bps=slippage_bps,
                realized_slippage_vs_quote_bps='0', fill_USDC_per_B24=str(Decimal(2000000000)/out),
                output_shortfall_vs_NAV_bps=str((1-Decimal(out)/2000000000)*10000),
                fill_premium_vs_NAV_bps=str((Decimal(2000000000)/out-1)*10000),
                quote_gas_estimate=quote_gas, rejects_above_quote_minimum=True,
                pool_before=before, pool_after=pool(w))


def dry_run(args):
    started = time.monotonic()
    coordination = check_coordination()
    args.output.mkdir(parents=True, exist_ok=False)
    save(args.output / 'coordination.json', coordination)
    from snapshot import export
    from rpc import call, read as native_read
    from phase6_export import operations
    from qualify import S
    s = export(args.output / 'current-ledger.json')
    states = [native_read('status', node=i) for i in range(6)]
    require(all(x['block_height'] == s['tip']['height'] and x['state_root'] == s['tip']['state_root'] for x in states), 'Native snapshot not agreed by all six')
    save(args.output / 'six-status.json', states)
    bridge = call('vault_bridge_route', {'asset_id': '02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b'})
    save(args.output / 'bridge-route.json', bridge)
    # Existing authenticated GET helper reads the proxy bearer-token file, never
    # any wallet key. Its readiness action cannot submit a bridge job.
    readiness_call = subprocess.run(['python3', '/home/postfiat/repos/postfiatl1v2-b23-ops/ops/b23/bridge_job.py',
                                    'readiness', str(args.output / 'bridge-readiness.json')],
                                   capture_output=True, text=True, timeout=90, check=True)
    readiness = load(args.output / 'bridge-readiness.json')
    require(readiness_call.stdout.startswith('200 ') and readiness.get('ready') is True
            and readiness.get('beacon_anchor_authenticated') is True
            and readiness.get('credited_asset_id') == S
            and readiness.get('route_epoch') == bridge['governance_route_epoch']
            and readiness.get('route_profile_hash') == bridge['profile_hash'], 'Bridge readiness/bindings failed')
    plan = operations(s, 5000000000, 'ready-' + uuid.uuid4().hex, S)
    save(args.output / 'native-operations-UNSIGNED.json', plan)
    due = plan[0]['operation']['max_settlement_value_atoms']
    route = next(x for x in s['ledger']['pftl_uniswap_routes'] if x['route_id'] == ROUTE)
    nav = next(x for x in s['ledger']['nav_assets'] if x['asset_id'] == route['native_nav_asset_id'])
    require(due == 5025000000, 'NAV changed: 7025 USDC no longer exactly funds the requested launch')
    w0 = Web3(ReadOnlyProvider(RPC, request_kwargs={'timeout': 60}))
    require(w0.eth.chain_id == 1, 'Upstream is not mainnet')
    block = w0.eth.block_number
    before_eth = w0.eth.get_balance(OPERATOR)
    expected = load(PRIOR / 'deployment-expected.json')
    runtime = {}
    for name in ('token', 'verifier', 'controller'):
        address = expected['operator'][name]
        runtime[name] = hx(Web3.keccak(w0.eth.get_code(address)))
        require(runtime[name] == expected['runtime'][name + '_runtime_code_hash'], name + ' runtime differs')
    for name, address in [('USDC', USDC), ('Permit2', PERMIT), ('PositionManager', POSITION)]:
        runtime[name] = hx(Web3.keccak(w0.eth.get_code(address)))
        require(runtime[name] == load(PRIOR / 'lp-qualification/result.json')['code_hashes'][name], name + ' runtime differs')
    runtime['UniversalRouter'] = hx(Web3.keccak(w0.eth.get_code(ROUTER)))
    require(len(w0.eth.get_code(ROUTER)) > 0, 'Router code absent')
    controller = expected['operator']['controller']
    require(read(w0, controller, 'mintPaused()', outputs=['bool'])[0] and read(w0, controller, 'returnPaused()', outputs=['bool'])[0], 'Pause state differs')
    require(read(w0, TOKEN, 'totalSupply()')[0] == 1000000 and balance(w0, TOKEN, OPERATOR) == 1000000, 'Prior one-B24 supply changed')
    fee = max(2000000000, 2*w0.eth.get_block(block)['baseFeePerGas'] + w0.eth.max_priority_fee)
    prior = load(PRIOR / 'funding-final/sequence.json')
    sequence = copy.deepcopy(prior)
    sequence.update(prepared=True, executed=False, full_dry_run_passed=False, refreshed_at=datetime.now(timezone.utc).isoformat(),
                    native_snapshot_height=s['tip']['height'], NAV_atoms=nav['nav_per_unit'], native_NAV_epoch=nav['finalized_epoch'],
                    native_route_epoch=route['v2']['route_epoch'], native_policy=route['v2']['primary_market_policy'],
                    current_subscription_due_atoms=due, operator_USDC_atoms=balance(w0, USDC, OPERATOR),
                    operator_ETH_wei=before_eth, operator_nonce=w0.eth.get_transaction_count(OPERATOR, 'pending'),
                    ethereum_block=block, fee_cap_wei=fee, bridge_ready_now=True,
                    conditional_S_copy_passed=None, runtime_code_hashes=runtime,
                    acceptance_buyer_USDC_atoms=2000000000, combined_launch_and_buyer_USDC_atoms=9025000000,
                    refreshed_fields=['native_snapshot_height', 'NAV_epoch', 'route_epoch', 'policy_hash', 'pricing_reserve_packet_hash',
                                      'route_digest', 'deposit_nonce', 'reservation_id', 'subscription_nonce', 'export_nonce',
                                      'packet_hash', 'deadlines', 'operator_nonce', 'balances', 'Ethereum_block', 'fees', 'runtime_code_hashes'],
                    unverified_fields=['new_native_signed_execution', 'new_finality', 'new_proof', 'new_checkpoint_votes'])
    save(args.output / 'sequence.json', sequence)
    blockers = ['An Anvil deposit has no canonical Ethereum beacon finality; the live relay must not claim it.',
                'Actual reserve/subscribe/export authorization requires controlled signatures; conditional test identities cannot establish it.',
                'A current ledger copy cannot produce authentic committee finality for a new 5000-B24 export.',
                'The archived one-B24 proof binds another amount/packet and cannot authorize this mint.',
                'A fork mint receipt cannot obtain authentic live Ethereum checkpoint votes.',
                'Validator-5 authority key mismatch remains unrepaired; five matching public keys are not a fresh availability test.']
    result = dict(full_launch_passed=False, live_execution_implemented=False, mainnet_transactions=0, native_chain_writes=0,
                  fork_block=block, blockers=blockers, gas_by_sequence_step=[
                      dict(step=i, status='UNEXECUTED', gas_used=None) for i in range(1, 12)])
    save(args.output / 'result.json', result)
    check_coordination()
    with Fork(args.output, block) as fork:
        w = fork.w
        fork.admin('anvil_impersonateAccount', [OPERATOR])
        require(w.eth.get_balance(OPERATOR) == before_eth, 'Operator ETH changed across fork read')
        fixtures = [fork.fixture_balance(USDC, OPERATOR, 7025000000, 9)]
        vault = Web3.to_checksum_address(bridge['profile']['vault_address'])
        require(hx(Web3.keccak(w.eth.get_code(vault))) == bridge['profile']['vault_runtime_code_hash'], 'Vault code changed')
        require(not read(w, vault, 'paused()', outputs=['bool'])[0], 'Vault is paused')
        nonce = Web3.keccak(text='ready-deposit-' + uuid.uuid4().hex)
        before_vault = balance(w, USDC, vault)
        fork.send('approve-vault-5025', OPERATOR, USDC, encode_call('approve(address,uint256)', ['address', 'uint256'], [vault, due]))
        receipt = fork.send('depositV2-5025', OPERATOR, vault, encode_call('depositV2(uint256,string,bytes32,bytes32)',
                     ['uint256', 'string', 'bytes32', 'bytes32'], [due, plan[0]['source'], nonce, bytes.fromhex(bridge['route_binding'])]))
        topic = Web3.keccak(text='ERC20BridgeDepositedV2(bytes32,address,bytes32,string,uint256,bytes32,bytes32,uint256,address,address)')
        logs = [x for x in receipt.logs if x['address'] == vault and x['topics'][0] == topic]
        require(len(logs) == 1, 'Deposit event absent or ambiguous')
        event = logs[0]
        values = decode(['string', 'uint256', 'bytes32', 'bytes32', 'uint256', 'address', 'address'], event['data'])
        require(values[0] == plan[0]['source'] and values[1] == due and values[2] == nonce
                and values[3].hex() == bridge['route_binding'] and values[4] == 1
                and values[5].lower() == vault.lower() and values[6].lower() == USDC.lower()
                and event['topics'][2][-20:].hex() == OPERATOR[2:].lower()
                and event['topics'][3] == Web3.keccak(text=plan[0]['source']), 'Deposit event binding differs')
        require(balance(w, USDC, vault) == before_vault + due and balance(w, USDC, OPERATOR) == 2000000000, 'Deposit balances differ')
        save(args.output / 'deposit-event.json', dict(fork_only=True, deposit_id=hx(event['topics'][1]), nonce=hx(nonce),
                                                    route_binding=bridge['route_binding'], amount_atoms=due, recipient=values[0]))
        result['gas_by_sequence_step'][0].update(status='FORK_FUNDED_FIXTURE', gas_used=0)
        result['gas_by_sequence_step'][1].update(status='FORK_DEPOSIT_PASSED', gas_used=sum(x['gas_used'] for x in fork.rows))
        if args.qualify_components:
            native_copy = Path('/mnt/HC_Volume_106212907/sec-fundamentals-proxy/b23-tee-nav-canary-target/release/b24-ready-native-copy')
            require(native_copy.is_file(), 'Build the isolated b24-ready-native-copy binary first')
            native_run = subprocess.run([str(native_copy), '--snapshot', str(args.output / 'current-ledger.json'),
                                         '--operations', str(args.output / 'native-operations-UNSIGNED.json'),
                                         '--output', str(args.output / 'conditional-native-copy.json'),
                                         '--allow-explicit-test-identity-and-credit'], capture_output=True, text=True, timeout=120)
            (args.output / 'conditional-native-copy.log').write_text(native_run.stdout + native_run.stderr)
            require(native_run.returncode == 0, 'Conditional native copy failed: ' + native_run.stderr[-1500:])
            result['native_components'] = load(args.output / 'conditional-native-copy.json')
            require(result['native_components']['conditional_native_copy_passed'], 'Conditional copy did not pass')
            sequence.update(conditional_S_copy_passed=True, conditional_copy_uses_test_identity=True,
                            conditional_copy_evidence=str(args.output / 'conditional-native-copy.json'),
                            LP_fork_proof=str(args.output / 'result.json'))
            save(args.output / 'sequence.json', sequence)
            for index in (3, 4):
                result['gas_by_sequence_step'][index].update(status='COPY_WITH_TEST_IDENTITY_AND_CREDIT_NO_FINALITY', gas_used=0)
            unpause = fork.send('unpause-mint-component', OPERATOR, controller,
                                encode_call('setMintPaused(bool)', ['bool'], [False]))
            require(not read(w, controller, 'mintPaused()', outputs=['bool'])[0]
                    and read(w, controller, 'returnPaused()', outputs=['bool'])[0], 'Fork pause state differs')
            result['gas_by_sequence_step'][7].update(status='UNPAUSE_ONLY_NO_PROOF_ACCEPTANCE_OR_MINT', gas_used=unpause.gasUsed)
            result['LP_components'] = seed_pool(fork, nav['nav_per_unit'])
            result['acceptance_swap'] = acceptance_swap(fork, args.slippage_bps)
            result['gas_by_sequence_step'][9].update(status='FORK_COMPONENT_WITH_WRAPPER_FIXTURE', gas_used=sum(x['gas_used'] for x in fork.rows if x['label'].startswith('LP-')))
            result['gas_by_sequence_step'][10].update(status='FORK_NFT_BALANCES_VERIFIED_ONLY', gas_used=0)
        result.update(fixtures=fixtures, measured_gas_used=sum(x['gas_used'] for x in fork.rows),
                      measured_gas_limit=sum(x['gas_limit'] for x in fork.rows),
                      operator_measured_gas=sum(x['gas_used'] for x in fork.rows if x['sender'] == OPERATOR),
                      buyer_measured_gas=sum(x['gas_used'] for x in fork.rows if x['sender'] != OPERATOR),
                      measured_maximum_cost_ETH=str(Decimal(sum(x['gas_limit'] for x in fork.rows)*fee)/10**18),
                      operator_held_ETH=str(Decimal(before_eth)/10**18), fee_cap_wei=fee,
                      full_launch_gas_used=None, full_launch_ETH_needed=None, full_launch_wall_seconds=None,
                      measured_component_wall_seconds=time.monotonic()-started)
        # This is a planning allowance, not measured 5000-unit proof/mint gas.
        # Retain the old exact one-unit acceptance/mint limits including their
        # margins, then add 10% for the complete Ethereum operator allowance.
        old_mint = load(PRIOR / 'mint-unsigned.json')['transactions']
        remaining_mint_limit = sum(x['transaction']['gas'] for x in old_mint
                                   if x['label'] != 'unpause-test-mint' or not args.qualify_components)
        operator_limit = sum(x['gas_limit'] for x in fork.rows if x['sender'] == OPERATOR)
        buyer_limit = sum(x['gas_limit'] for x in fork.rows if x['sender'] != OPERATOR)
        planned = math.ceil((operator_limit + remaining_mint_limit) * 1.1)
        result['conditional_budget'] = dict(
            basis='Measured fork deposit/unpause/LP limits plus historical one-B24 accept/mint/re-pause limits, then 10%; NOT measured full launch',
            historical_remaining_mint_limit=remaining_mint_limit,
            operator_gas_limit=planned, operator_ETH_at_fee_cap=str(Decimal(planned*fee)/10**18),
            buyer_gas_limit=buyer_limit, buyer_ETH_at_fee_cap=str(Decimal(buyer_limit*fee)/10**18),
            operator_balance_covers_conditional_budget=before_eth >= planned*fee,
            maximum_fee_gwei_covered_by_operator=str(Decimal(before_eth)/planned/10**9)) if args.qualify_components else None
    save(args.output / 'result.json', result)
    print(json.dumps(result, indent=2))
    return 2


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--dry-run', action='store_true')
    ap.add_argument('--qualify-components', action='store_true')
    ap.add_argument('--slippage-bps', type=int, default=50)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    require(0 < args.slippage_bps <= 100, 'Slippage must be 1–100 basis points')
    check_coordination()
    require(args.dry_run, 'LIVE LAUNCH BLOCKED: the eleven-step finality/signing pipeline has not passed; no transactions sent')
    try:
        return dry_run(args)
    except Exception as error:
        if args.output.exists():
            save(args.output / 'failure.json', dict(error=str(error), full_launch_passed=False,
                                                   mainnet_transactions=0, native_chain_writes=0))
        raise


if __name__ == '__main__':
    raise SystemExit(main())
