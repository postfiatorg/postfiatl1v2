#!/usr/bin/env python3
"""Strict release-4 creation/proof-reuse checks on an exported ledger only.

Uses deterministic PUBLIC TEST keys, never a production key or an RPC writer.
Adds two new fee-funded test accounts to the copy; does not edit existing rows.
Stops at the actual attestation boundary instead of injecting a reserve proof.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def canonical(x):
    return json.dumps(x, sort_keys=True, separators=(',', ':')).encode()


def save(path, x):
    path.write_text(json.dumps(x, indent=2) + '\n')


def load(path):
    return json.loads(path.read_bytes())


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('snapshot', 'capture', 'proof', 'node', 'canary', 'nav_cli', 'output'):
        ap.add_argument('--' + name.replace('_', '-'), type=Path, required=True)
    a = ap.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    base = load(a.snapshot)
    initial = copy.deepcopy(base)
    manifest = load(a.capture / 'manifest.json')
    profile = load(a.capture / 'profile.json')
    values = load(a.capture / 'verified-values.json')
    chain = base['tip']['chain_id']
    height = base['tip']['height']
    receipts = []
    with tempfile.TemporaryDirectory(prefix='PUBLIC-TEST-new-navcoin-') as temp:
        keys = []
        for index in range(2):
            key = Path(temp) / f'test-{index}.json'
            # These are public deterministic test vectors, never funded live.
            seed = hashlib.sha256(f'PUBLIC TEST ONLY new-navcoin {index}'.encode()).hexdigest()
            generated = json.loads(subprocess.check_output([
                str(a.node), 'wallet-keygen', '--chain-id', chain,
                '--master-seed-hex', seed, '--key-file', str(key),
                '--backup-file', str(Path(temp) / f'backup-{index}.json')]))
            # Obtain only the public account information from the local test file.
            keydata = load(key)
            address = keydata['address']
            pub = keydata['public_key_hex']
            assert not any(x['address'] == address for x in base['ledger']['accounts'])
            base['ledger']['accounts'].append(dict(address=address, public_key_hex=pub,
                                                  balance=1_000_000_000_000, sequence=0))
            keys.append((key, address))
        issuer = keys[0][1]
        holder = keys[1][1]
        current = a.output / 'fixture-start.json'
        save(current, base)
        save(a.output / 'fixture-disclosure.json', dict(
            original_snapshot_sha256=hashlib.sha256(a.snapshot.read_bytes()).hexdigest(),
            original_tip=initial['tip'],
            modification='Append two fresh PUBLIC TEST accounts, each with 1000000000000 native fee atoms; no existing rows changed.',
            issuer=issuer, holder=holder, consensus_finality_created=False,
            production_keys_used=False, rpc_writes=0))

        def apply(label, op, accepted=True, code=None, signer=0):
            nonlocal current
            state = load(current)
            key, source = keys[signer]
            sequence = next(x['sequence'] for x in state['ledger']['accounts'] if x['address'] == source) + 1
            signed = json.loads(subprocess.check_output([
                str(a.node), 'wallet-sign-asset-transaction', '--key-file', str(key),
                '--chain-id', chain, '--genesis-hash', base['tip']['genesis_hash'],
                '--protocol-version', str(base['tip']['protocol_version']),
                '--fee', '1000000', '--sequence', str(sequence),
                '--operation-json', json.dumps(op, separators=(',', ':'))], stderr=subprocess.PIPE))
            step = dict(label=label, height=height + len(receipts) + 1,
                        transaction=signed, expect_accepted=accepted, expect_code=code)
            steps = a.output / (label + '-steps.json')
            save(steps, [step])
            out = a.output / (label + '-state.json')
            result = subprocess.run([str(a.canary), 'apply', '--snapshot', str(current),
                                     '--steps', str(steps), '--output', str(out)], capture_output=True)
            if out.exists():
                receipts.extend(load(out)['canary_receipts'])
                save(a.output / 'receipts.json', receipts)
            if result.returncode:
                raise RuntimeError(label + ': ' + result.stderr.decode())
            current = out
            return load(current)['ledger']

        state = apply('create-new-asset', dict(operation='asset_create', issuer=issuer,
            code='SCHWAB2', version=1, precision=6, display_name='Isolated test Schwab NAVCoin',
            max_supply=None, requires_authorization=False, freeze_enabled=False, clawback_enabled=False))
        asset = next(x['asset_id'] for x in state['asset_definitions'] if x['issuer'] == issuer)
        register = dict(load(a.capture / 'profile-operation.json'), operation='nav_profile_register', registrant=issuer)
        # Existing profile is immutable/content addressed; another registrant cannot recreate it.
        apply('existing-profile-already-registered', register, False, 'duplicate_nav_profile')
        new_witness = load(a.capture / 'witness.json')
        new_witness['manifest']['nav_asset_id'] = asset
        new_witness['policy']['measured_policy']['nav_asset_id'] = asset
        policy_hash = hashlib.sha3_384(b'postfiat.b23.nav_source_policy.v1\0' + canonical(new_witness['policy'])).hexdigest()
        candidate = dict(register, source_manifest_hash=policy_hash)
        state = apply('register-new-profile-candidate', candidate)
        new_profile = next(x['profile_id'] for x in state['nav_proof_profiles'] if x.get('source_manifest_hash') == policy_hash)
        save(a.output / 'candidate-profile.json', dict(operation=candidate, profile_id=new_profile,
            status='REGISTRATION ONLY: old PCR pins have NOT attested the changed measured policy; regenerate EIF and pins before use'))
        # Bind the exact existing profile to isolate the asset-ID reuse rejection.
        bind = dict(operation='nav_asset_register', issuer=issuer, asset_id=asset,
                    reserve_operator=issuer, redemption_account=issuer,
                    valuation_unit='USDC', proof_profile=profile['profile_id'])
        apply('bind-exact-existing-profile', bind)
        apply('holder-trustline', dict(operation='trust_set', account=holder, issuer=issuer,
              asset_id=asset, limit=1_000_000_000_000, reserve_paid=10, authorized=False, frozen=False), signer=1)
        proof = (a.proof / 'proof-calldata.bin').read_bytes()
        public = (a.proof / 'public-values.bin').read_bytes()
        assert public == bytes.fromhex(load(a.capture / 'result.json')['body']['public_values_hex'])
        packet = hashlib.sha3_384(b'PUBLIC TEST ONLY new-navcoin packet').hexdigest()
        submit = dict(operation='nav_reserve_submit', issuer=issuer, submitter=issuer, asset_id=asset,
            epoch=values['observation_epoch'],
            nav_per_unit=values['verified_net_assets'] * 1_000_000 // 5_000_000_000,
            circulating_supply=5_000_000_000,
            verified_net_assets=values['verified_net_assets'], proof_profile=profile['profile_id'],
            source_root=values['source_observation_root'], attestor_root=values['valuation_trust_root'],
            reserve_packet_hash=packet, reserve_accounts=[], sp1_proof_bytes=list(proof),
            sp1_public_values=list(public))
        apply('reject-existing-proof-new-asset', submit, False, 'nav_reserve_asset_mismatch')
        # A changed public value no longer has a valid Groth16 proof.
        altered = copy.deepcopy(submit)
        old = bytes.fromhex(manifest['nav_asset_id'])
        offset = public.index(old)
        altered['sp1_public_values'][offset:offset + len(old)] = bytes.fromhex(asset)
        apply('reject-relabelled-public-values', altered, False, 'sp1_proof_invalid')
        apply('reject-finalize-without-reading', dict(operation='nav_epoch_finalize', issuer=issuer,
            asset_id=asset, epoch=values['observation_epoch'], reserve_packet_hash=packet), False, 'missing_nav_reserve_packet')
        source_route = next(x for x in base['ledger']['pftl_uniswap_routes'] if x['native_nav_asset_id'] == manifest['nav_asset_id'])
        policy = copy.deepcopy(source_route['v2']['primary_market_policy'])
        policy.pop('spread_recipient', None)
        policy.update(issue_multiplier_bps=10050, redeem_multiplier_bps=9995,
                      issue_capacity_atoms=250_000_000_000, redeem_capacity_atoms=250_000_000_000,
                      max_order_atoms=250_000_000_000, policy_epoch=1,
                      pricing_reserve_packet_hash=packet)
        fields = ('policy_epoch issue_multiplier_bps redeem_multiplier_bps issue_capacity_atoms '
                  'redeem_capacity_atoms max_order_atoms min_order_atoms valid_from_height '
                  'expires_at_height max_nav_age_blocks pricing_nav_epoch pricing_reserve_packet_hash').split()
        policy['policy_hash'] = hashlib.sha3_384(b'postfiat.pftl_uniswap.primary_market_policy.v2\0' +
            ''.join(f'{k}={policy[k]}\n' for k in fields).encode()).hexdigest()
        route = {k: copy.deepcopy(source_route[k]) for k in (
            'settlement_asset_id', 'return_finality_blocks', 'ethereum_verification_policy')}
        route.update(operation='pftl_uniswap_route_init_v2', operator=issuer,
            route_id='isolated-new-schwab-legacy-policy', route_config_digest='91' * 48,
            native_nav_asset_id=asset, opening_inventory_atoms=5_000_000_000,
            opening_inventory_holder=holder, handoff_controller='0x' + '11' * 20,
            settlement_adapter='0x' + '22' * 20, wrapped_navcoin_token='0x' + '33' * 20,
            ethereum_chain_id=1, route_supply_cap_atoms=2_000_000_000_000,
            packet_notional_cap_atoms=250_000_000_000, latest_finalized_nav_epoch=3,
            route_epoch=1, outbound_verification_class='TRUSTLESS_FINALITY',
            return_verification_class='BFT_CHECKPOINT', live_value_enabled=False,
            primary_market_policy=policy)
        apply('reject-route-without-finalized-reading', route, False, 'pftl_uniswap_route_epoch_mismatch')
        apply('reject-mint-without-reading', dict(operation='nav_mint_at_nav', issuer=issuer, to=holder,
            asset_id=asset, epoch=values['observation_epoch'], reserve_packet_hash=packet,
            amount=5_000_000_000), False, 'nav_epoch_mismatch')
        apply('reject-export-without-route', dict(operation='pftl_uniswap_export_debit',
            owner=holder, route_id=route['route_id'], packet_hash='92' * 48, export_nonce='93' * 32,
            ethereum_recipient='0x' + '44' * 20, amount_atoms=5_000_000_000,
            destination_deadline_seconds=1_800_000_000, refund_delay_blocks=100),
            False, 'missing_pftl_uniswap_route', signer=1)

        # Three native checks: original succeeds; relabelling fails at measured
        # policy; updating that policy fails at signed manifest binding.
        native_checks = []
        for label in ('original-reading', 'changed-asset', 'changed-asset-and-policy'):
            witness = load(a.capture / 'witness.json')
            if label != 'original-reading':
                witness['manifest']['nav_asset_id'] = asset
            if label == 'changed-asset-and-policy':
                witness['policy']['measured_policy']['nav_asset_id'] = asset
                witness['manifest']['source_manifest_hash'] = policy_hash
            path = a.output / (label + '-witness.json')
            path.write_bytes(canonical(witness))
            result = subprocess.run([str(a.nav_cli), 'native', '--witness', str(path),
                                     '--output', str(a.output / (label + '-values.json'))], capture_output=True)
            native_checks.append(dict(label=label, returncode=result.returncode,
                                      stderr=result.stderr.decode().strip()))
            assert (result.returncode == 0) == (label == 'original-reading'), native_checks[-1]
        save(a.output / 'native-reading-checks.json', native_checks)
        final = load(current)['ledger']
        # All pre-existing ledger rows remain byte-for-byte equal.
        for field, original_rows in initial['ledger'].items():
            if isinstance(original_rows, list):
                assert final[field][:len(original_rows)] == original_rows, field
            else:
                assert final[field] == original_rows, field
        nav = next(x for x in final['nav_assets'] if x['asset_id'] == asset)
        assert nav['finalized_epoch'] == 0 and nav['circulating_supply'] == 0
        assert all(x['matched'] for x in receipts)
        assert all(not x['ledger_changed'] for x in receipts if not x['expected_accepted'])
        save(a.output / 'result.json', dict(
            passed=True, scope='CREATION AND NEGATIVE PROOF-REUSE CHECKS ONLY',
            asset_id=asset, existing_profile=profile['profile_id'], candidate_profile=new_profile,
            snapshot_tip=initial['tip'], original_ledger_rows_unchanged=True,
            accepted_steps=sum(x['receipt']['accepted'] for x in receipts),
            rejected_steps=sum(not x['receipt']['accepted'] for x in receipts),
            finalized_reading=False, route_created=False, opening_supply_minted=False,
            export_receipt_created=False, return_import_performed=False,
            blocker='Fresh asset-bound Nitro reading and Groth16 reserve proof required; no fake proof or finality injected.'))
        print(json.dumps(load(a.output / 'result.json')))


if __name__ == '__main__':
    main()
