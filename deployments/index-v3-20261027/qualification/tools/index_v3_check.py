"""Release checks 5a and 5c and the rollback-after-v3 question, local only, on a scratch import of the newest signed
archive (1137, validator-1, 2026-10-23). The deployed executable imports it and builds the v2 index, as the six
hosts carry today; then the new build reads it, rebuilds it as v3, and the deployed executable reads the v3 files."""
import json, pathlib, subprocess, time, hashlib
R = pathlib.Path('/home/postfiatchad/.cache/release-index-v3-20261027')
NEW, OLD = R/'binaries/candidate-1', R/'binaries/rollback'
SIGNED = pathlib.Path('/home/postfiatchad/.postfiat/deployments/exchange-readiness-20261023/evidence/pre-rollout-backup/backup-signed')
PUB = '/home/postfiatchad/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys/snapshot-publisher.public.json'
D = R/'index-check'; DATA = D/'data'; OUT = D/'out'; OUT.mkdir(parents=True, exist_ok=True)
result = dict(scope='scratch import of the 1137 signed archive; no host contact', checks={})


def sha(p): return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()


def run(name, binary, *args, ok=True):
    t0 = time.monotonic()
    p = subprocess.run([str(binary), *map(str, args)], capture_output=True)
    elapsed = round(time.monotonic() - t0, 3)
    (OUT/f'{name}.stdout').write_bytes(p.stdout); (OUT/f'{name}.stderr').write_bytes(p.stderr)
    if ok and p.returncode != 0:
        raise SystemExit(f'{name} failed ({p.returncode}): {p.stderr.decode()[-400:]}')
    return p, elapsed


def js(p): return json.loads(p.stdout)


def histories(binary, label, addresses):
    out = {}
    for i, a in enumerate(addresses):
        p, _ = run(f'{label}-{i:02d}', binary, 'account-tx', '--data-dir', DATA, '--address', a, '--limit', '100')
        out[a] = p.stdout
    return out


assert sha(NEW) != sha(OLD)
run('import-old', OLD, 'snapshot-import-signed-finalized-checkpoint', '--data-dir', DATA, '--snapshot-dir', SIGNED,
    '--trusted-publisher-key-file', PUB, '--node-id', 'validator-1')
status_new, _ = run('status-new', NEW, 'status', '--data-dir', DATA)
s = js(status_new)
result['checks']['import'] = dict(height=s.get('block_height'), tip=s.get('block_tip_hash'), root=s.get('state_root'))
p, t_v2 = run('v2-build-old', OLD, 'account-tx-index-build', '--data-dir', DATA)
v2 = js(p)
result['checks']['v2_build_old'] = dict(seconds=t_v2, rows=v2['indexed_row_count'], accounts=v2['account_count'],
                                        blocks=v2['indexed_block_count'], index_usable=v2['index_usable'], disk_index_usable=v2['disk_index_usable'])
aggregate = json.loads((DATA/'account_tx_index.json').read_text())
assert aggregate['schema'] == 'postfiat-account-tx-index-v2'
ranked = sorted(aggregate['accounts'].items(), key=lambda kv: (-len(kv[1]), kv[0]))
addresses = [a for a, _ in ranked[:20]]
result['addresses'] = [dict(address=a, v2_rows=len(rows)) for a, rows in ranked[:20]]
v2_old = histories(OLD, 'v2-old', addresses)
assert all(json.loads(b)['index_used'] for b in v2_old.values())

# 5a: the new build reports the v2 index unsupported and answers from the archive scan.
p, _ = run('v2-status-new', NEW, 'account-tx-index-status', '--data-dir', DATA)
st = js(p)
result['checks']['v2_status_new'] = {k: st.get(k) for k in ('index_present', 'index_usable', 'reason', 'disk_index_present', 'disk_index_usable', 'disk_index_reason')}
scan_new = histories(NEW, 'scan-new', addresses)
result['checks']['scan_new_index_used'] = any(json.loads(b)['index_used'] for b in scan_new.values())
result['checks']['scan_new_rows_equal_v2_rows'] = all(json.loads(scan_new[a])['rows'] == json.loads(v2_old[a])['rows'] for a in addresses)
p, t_v3 = run('v3-build-new', NEW, 'account-tx-index-build', '--data-dir', DATA)
v3 = js(p)
blocks = s.get('block_height') or v3['indexed_block_count']
result['checks']['v3_build_new'] = dict(seconds=t_v3, seconds_per_1000_blocks=round(t_v3 * 1000 / blocks, 3), chain_blocks=blocks,
                                        rows=v3['indexed_row_count'], accounts=v3['account_count'], index_usable=v3['index_usable'],
                                        disk_index_usable=v3['disk_index_usable'], disk_account_shard_count=v3['disk_account_shard_count'])
p, _ = run('v3-status-new', NEW, 'account-tx-index-status', '--data-dir', DATA)
st = js(p)
result['checks']['v3_status_new'] = {k: st.get(k) for k in ('index_usable', 'reason', 'disk_index_usable', 'disk_index_reason', 'indexed_to_height', 'disk_index_lag_blocks')}
result['checks']['v3_schema'] = json.loads((DATA/'account_tx_index_meta.json').read_text())['schema']
v3_new = histories(NEW, 'v3-new', addresses)
result['checks']['v3_new_bytes_identical_to_v2_old'] = f'{sum(v3_new[a] == v2_old[a] for a in addresses)}/{len(addresses)}'
result['checks']['v3_new_index_used'] = all(json.loads(b)['index_used'] for b in v3_new.values())

# Step 4: the deployed executable on the v3 files.
old_after = histories(OLD, 'old-after-v3', addresses)
result['checks']['old_after_v3_index_used'] = any(json.loads(b)['index_used'] for b in old_after.values())
result['checks']['old_after_v3_rows_equal'] = all(json.loads(old_after[a])['rows'] == json.loads(v2_old[a])['rows'] for a in addresses)
p, _ = run('old-status-after-v3', OLD, 'account-tx-index-status', '--data-dir', DATA, ok=False)
result['checks']['old_status_after_v3'] = dict(exit_code=p.returncode, stderr_tail=p.stderr.decode()[-300:].strip(),
                                               index_usable=(json.loads(p.stdout).get('index_usable') if p.returncode == 0 else None),
                                               reason=(json.loads(p.stdout).get('reason') if p.returncode == 0 else None))
p, t_old = run('old-rebuild-after-v3', OLD, 'account-tx-index-build', '--data-dir', DATA, ok=False)
result['checks']['old_rebuild_after_v3'] = dict(exit_code=p.returncode, seconds=t_old,
                                                index_usable=(json.loads(p.stdout).get('index_usable') if p.returncode == 0 else None),
                                                disk_index_usable=(json.loads(p.stdout).get('disk_index_usable') if p.returncode == 0 else None))
old_rebuilt = histories(OLD, 'old-rebuilt', addresses)
result['checks']['old_rebuilt_index_used'] = all(json.loads(b)['index_used'] for b in old_rebuilt.values())
result['checks']['old_rebuilt_bytes_identical_to_v2_old'] = f'{sum(old_rebuilt[a] == v2_old[a] for a in addresses)}/{len(addresses)}'

# 5c: the activation heights in the new build's status read of the scratch import.
result['checks']['status_activation_heights_new'] = {k: s[k] for k in s if 'activation_height' in k}
(D/'index-v3-check.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result['checks'], indent=1))
