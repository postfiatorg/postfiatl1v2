"""5a/5c and step 4, local only: a four-validator loopback devnet through the built executables.

Round 1 (height 1): every validator runs the NEW executable; validator-3's transport port sits behind a proxy that
delays its connection by 1 s (validator-3 binds 127.0.0.2, the proxy holds its 127.0.0.1 port), so its vote arrives after the quorum of three (proposer validator-1, validator-0,
validator-2) but inside the 4 s deadline (the VR-01 case). Default round flags as the live faucet rounds.
Then the OLD executable (index-v3-20261027) reads the data the new one wrote (status, checkpoint, full replay) and
round 2 (height 2, proposer validator-2, new) runs with validator-0 and validator-1 served by the OLD executable on
those data dirs. No network beyond 127.0.0.1."""
import json, os, pathlib, shutil, socket, subprocess, sys, threading, time
ROOT = pathlib.Path('/home/postfiatchad/.cache/release-vote-records-20261030')
NEW = ROOT/'binaries/candidate-1'; OLD = ROOT/'binaries/rollback'
W = ROOT/'devnet'; OUT = ROOT/'devnet-report.json'
N = 4
ENV = os.environ | dict(POSTFIAT_PREWARM_SHIELDED_VERIFIER='1', POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER='1', POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER='1')  # as the generated transport env files
if W.exists(): shutil.rmtree(W)
W.mkdir(parents=True)
report = dict(scope='local loopback devnet, four validators', new_binary=str(NEW), old_binary=str(OLD), steps=[])


def save(): OUT.write_text(json.dumps(report, indent=2) + '\n')


def run(name, cmd, check=True, timeout=300):
    p = subprocess.run([str(c) for c in cmd], capture_output=True, text=True, timeout=timeout)
    (W/f'{name}.stdout').write_text(p.stdout); (W/f'{name}.stderr').write_text(p.stderr)
    report['steps'].append(dict(name=name, command=[str(c) for c in cmd], exit_code=p.returncode))
    if check and p.returncode != 0:
        save(); raise SystemExit(f'{name} failed: {p.stderr[-2000:]}')
    return p


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0)); return s.getsockname()[1]


dirs = [W/f'validator-{i}' for i in range(N)]
for i, d in enumerate(dirs):
    run(f'init-{i}', [NEW, 'init-consensus-v2', '--data-dir', d, '--node-id', f'validator-{i}', '--validators', N, '--activation-height', 2])
keys = (dirs[0]/'validator_keys.json').read_bytes(); registry = (dirs[0]/'validator_registry.json').read_bytes()
run('export-bootstrap', [NEW, 'snapshot-export', '--data-dir', dirs[0], '--snapshot-dir', W/'bootstrap'])
for i, d in enumerate(dirs[1:], 1):
    shutil.rmtree(d)
    run(f'import-{i}', [NEW, 'snapshot-import', '--data-dir', d, '--snapshot-dir', W/'bootstrap', '--node-id', f'validator-{i}'])
    (d/'validator_registry.json').write_bytes(registry)
allkeys = json.loads(keys)
for i, d in enumerate(dirs):
    mine = [v for v in allkeys['validators'] if v['node_id'] == f'validator-{i}']
    (d/'validator_keys.json').write_text(json.dumps(dict(allkeys, validators=mine), indent=2)); os.chmod(d/'validator_keys.json', 0o600)
base = None
while base is None:
    b = free_port(); socks = []
    try:
        for k in list(range(N)) + [100 + k for k in range(N)]:
            s = socket.socket(); socks.append(s); s.bind(('127.0.0.1', b + k))
        base = b
    except OSError: pass
    finally:
        for s in socks: s.close()
topo = W/'topology.json'
run('topology', [NEW, 'topology-consensus-v2', '--validators', N, '--base-port', base, '--hosts', ','.join(['127.0.0.1'] * N), '--rpc-base-port', base + 100, '--activation-height', 2, '--output', topo])
t = json.loads(topo.read_text())
late = 'validator-3'; late_port = next(p['p2p_port'] for p in t['peers'] if p['node_id'] == late)
stop = threading.Event()


def pipe(a, b):
    try:
        while True:
            data = a.recv(65536)
            if not data: break
            b.sendall(data)
    except OSError: pass
    finally:
        for s in (a, b):
            try: s.shutdown(socket.SHUT_RDWR)
            except OSError: pass


def proxy():
    srv = socket.socket(); srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(('127.0.0.1', late_port)); srv.listen(16); srv.settimeout(0.5)
    while not stop.is_set():
        try: c, _ = srv.accept()
        except socket.timeout: continue
        def handle(c=c):
            time.sleep(1.0)  # the late validator's answer arrives about 1 s after the request
            u = socket.create_connection(('127.0.0.2', late_port))
            threading.Thread(target=pipe, args=(c, u), daemon=True).start(); pipe(u, c)
        threading.Thread(target=handle, daemon=True).start()
    srv.close()


threading.Thread(target=proxy, daemon=True).start()


def serve(i, binary, topology, tag, host='127.0.0.1'):
    d = dirs[i]
    log = open(W/f'serve-{tag}-validator-{i}.log', 'w')
    return subprocess.Popen([str(binary), 'transport-validator-serve', '--unsafe-devnet-file-signer', '--unsafe-devnet-json-storage', '--data-dir', str(d), '--topology', str(topology),
                             '--key-file', str(d/'validator_keys.json'), '--vote-dir', str(d/f'votes-{tag}'), '--bind-host', host, '--max-connections', '10000', '--timeout-ms', '900000'],
                            stdout=log, stderr=subprocess.STDOUT, env=ENV)


def wait_ports(ports):
    end = time.time() + 600
    for host, port in ports:
        while time.time() < end:
            try: socket.create_connection((host, port), timeout=0.5).close(); break
            except OSError: time.sleep(0.2)


def status(binary, d, name):
    p = run(name, [binary, 'status', '--data-dir', d], check=False)
    try: s = json.loads(p.stdout); return {k: s.get(k) for k in ('node_id', 'block_height', 'block_tip_hash', 'state_root', 'mempool_pending')}
    except Exception: return dict(exit_code=p.returncode, error=p.stderr[-500:])


def round_(height, proposer, topology):
    pd = dirs[proposer]; batch = W/f'height-{height}.batch.json'
    run(f'batch-{height}', [NEW, 'batch-transfer', '--data-dir', pd, '--key-file', dirs[0]/'faucet_key.json', '--to', 'pf' + 'f' * 40, '--amount', 1_000_000 + height, '--batch-file', batch])
    t0 = time.monotonic()
    p = run(f'round-{height}', [NEW, 'transport-peer-certified-batch-round', '--data-dir', pd, '--topology', topology, '--batch-kind', 'transparent',
                                '--batch-file', batch, '--key-file', pd/'validator_keys.json', '--proposal-key-file', pd/'validator_keys.json',
                                '--require-local-proposer', '--allow-peer-failures', '--artifact-dir', W/f'artifacts-{height}',
                                '--height', height, '--view', 0, '--timeout-ms', 4000], check=False, timeout=120)
    return p, round(time.monotonic() - t0, 2)


procs = [serve(0, NEW, topo, 'r1'), serve(2, NEW, topo, 'r1'), serve(3, NEW, topo, 'r1', host='127.0.0.2')]
wait_ports([('127.0.0.2' if p['node_id'] == late else '127.0.0.1', p['p2p_port']) for p in t['peers'] if p['node_id'] != 'validator-1'])
try:
    p, elapsed = round_(1, 1, topo)
    report['round1'] = dict(exit_code=p.returncode, elapsed_seconds=elapsed, stdout_tail=p.stdout[-1500:], stderr_tail=p.stderr[-1500:])
    time.sleep(5)
finally:
    for pr in procs: pr.terminate()
    for pr in procs: pr.wait(timeout=20)
    stop.set(); time.sleep(1.5)
rec_file = dirs[1]/'block-vote-records/height-1.json'
report['vote_record_file'] = str(rec_file)
if rec_file.exists():
    rec = json.loads(rec_file.read_text()); report['vote_record'] = rec
    entries = {e['validator']: e for e in rec.get('entries', [])}
    report['vote_record_check'] = dict(
        names_every_validator=sorted(entries) == [f'validator-{i}' for i in range(N)],
        late_validator_outcome=entries.get(late, {}).get('outcome'), late_answered_after_quorum=entries.get(late, {}).get('answered_after_quorum'),
        no_false_no_answer=all(e.get('outcome') != 'no_answer' for e in entries.values()),
        outcomes={k: e.get('outcome') for k, e in entries.items()})
report['snapshot_points_files'] = {f'validator-{i}': (d/'private_pool_snapshot_points.json').exists() for i, d in enumerate(dirs)}
report['after_round1_new'] = {f'validator-{i}': status(NEW, d, f'status-r1-new-{i}') for i, d in enumerate(dirs)}
report['after_round1_old'] = {f'validator-{i}': status(OLD, d, f'status-r1-old-{i}') for i, d in enumerate(dirs)}
for i in (0, 1):
    for cmd in ('verify-finalized-checkpoint', 'verify-state'):
        p = run(f'old-{cmd}-validator-{i}', [OLD, cmd, '--data-dir', dirs[i]], check=False, timeout=300)
        try: v = json.loads(p.stdout); blk = v if cmd == 'verify-finalized-checkpoint' else v.get('block_log', {})
        except Exception: v, blk = {}, {}
        report[f'old_{cmd.replace("-", "_")}_validator_{i}'] = dict(exit_code=p.returncode, verified=v.get('verified'),
            state_root=blk.get('checkpoint_state_root' if cmd == 'verify-finalized-checkpoint' else 'state_root'), stderr_tail=p.stderr[-500:])
save()
procs = [serve(0, OLD, topo, 'r2'), serve(1, OLD, topo, 'r2'), serve(3, NEW, topo, 'r2')]
wait_ports([('127.0.0.1', p['p2p_port']) for p in t['peers'] if p['node_id'] != 'validator-2'])
try:
    p, elapsed = round_(2, 2, topo)
    report['round2'] = dict(exit_code=p.returncode, elapsed_seconds=elapsed, stdout_tail=p.stdout[-1500:], stderr_tail=p.stderr[-1500:])
    time.sleep(5)
finally:
    for pr in procs: pr.terminate()
    for pr in procs: pr.wait(timeout=20)
report['after_round2'] = {f'validator-{i}': status(NEW if i in (2, 3) else OLD, d, f'status-r2-{i}') for i, d in enumerate(dirs)}
report['snapshot_points_after_round2'] = {f'validator-{i}': json.loads((d/'private_pool_snapshot_points.json').read_text()) if (d/'private_pool_snapshot_points.json').exists() else None for i, d in enumerate(dirs)}
save(); print(json.dumps({k: report.get(k) for k in ('round1', 'vote_record_check', 'snapshot_points_files', 'round2')}, indent=1)[:6000])
