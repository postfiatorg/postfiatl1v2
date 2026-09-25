import datetime, hashlib, json, os, pathlib, resource, shutil, subprocess, sys, time

ROOT = pathlib.Path('/home/postfiatchad/.cache/release-repair-20260925')
C = ROOT / 'canary-backup-1036'
R4 = pathlib.Path('/home/postfiatchad/.postfiat/deployments/fastpay-committee-20260925-r4')
MERGED = ROOT / 'binaries/candidate-1'
R4BIN = ROOT / 'binaries/rollback'
PUB = pathlib.Path('/home/postfiatchad/.postfiat/deployments/cobalt-activation-8694b99d/snapshot-keys/snapshot-publisher.public.json')
DEPLOY_PUB = R4 / 'stage/rootfs/etc/postfiat/releases/fastpay-committee-20260925-r4/deployment.public.json'
EXPECT = {MERGED: 'd66cecc36426ce05ced8730b2439a27285c6b404688acd13dc23594b884eabd6',
          R4BIN: '44b6794f2f8eab577713dffb6e59880f8ee8c811863e99ed96ed1b0f4ec66bab'}
AS_LIMIT = 20 * 1024**3
ENV = dict(RAYON_NUM_THREADS='2', OMP_NUM_THREADS='2', MALLOC_ARENA_MAX='2', TMPDIR=str(ROOT / 'tmp'))


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix('.tmp')
    tmp.write_text(json.dumps(data, indent=2) + '\n')
    tmp.replace(path)


def limits():
    resource.setrlimit(resource.RLIMIT_AS, (AS_LIMIT, AS_LIMIT))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def run(name, cmd, expect_fail=False):
    out, err = C / 'logs' / f'{name}.stdout', C / 'logs' / f'{name}.stderr'
    rec = dict(name=name, command=[str(x) for x in cmd], started_at=now(),
               address_space_limit_bytes=AS_LIMIT, environment_overrides=ENV)
    print(f'{now()} START {name}', flush=True)
    t = time.monotonic()
    with out.open('wb') as o, err.open('wb') as e:
        code = subprocess.run(rec['command'], cwd=C, env=os.environ | ENV, stdout=o, stderr=e,
                              preexec_fn=limits, timeout=1500).returncode
    ok = (code != 0) if expect_fail else (code == 0)
    rec.update(exit_code=code, completed_at=now(), elapsed_seconds=round(time.monotonic() - t, 1),
               expected='nonzero exit (rejection)' if expect_fail else 'exit 0',
               status='PASS' if ok else 'FAIL', stdout_sha256=sha(out), stderr_sha256=sha(err),
               stdout=f'logs/{name}.stdout', stderr=f'logs/{name}.stderr')
    write(C / 'receipts' / f'{name}.json', rec)
    print(f'{now()} {rec["status"]} {name} exit={code} {rec["elapsed_seconds"]}s', flush=True)
    return rec


def main():
    pre = dict(checked_at=now(), binaries={}, trusted_public_key=str(PUB), trusted_public_key_sha256=sha(PUB))
    for binary, expected in EXPECT.items():
        actual = sha(binary)
        pre['binaries'][str(binary)] = dict(expected=expected, actual=actual, match=actual == expected)
        if actual != expected:
            write(C / 'receipts' / 'preflight.json', pre)
            sys.exit(f'hash mismatch for {binary}')
    signed = C / 'r4-signed'
    if not signed.exists():
        shutil.copytree(R4 / 'evidence/pre-rollout-backup/backup-signed', signed)
        subprocess.run(['chmod', '-R', 'a-w', str(signed)], check=True)
    pre['signed_manifest_sha256'] = sha(signed / 'snapshot.signed-manifest.json')
    write(C / 'receipts' / 'preflight.json', pre)

    m_s, m_u, r4_s, neg = (C / d for d in ('merged-signed-import', 'merged-unsigned-import', 'r4-signed-import', 'negative-import'))
    run('merged-import-signed', [MERGED, 'snapshot-import-signed-finalized-checkpoint', '--data-dir', m_s,
                                 '--snapshot-dir', signed, '--trusted-publisher-key-file', PUB, '--node-id', 'validator-1'])
    run('merged-verify-checkpoint-signed', [MERGED, 'verify-finalized-checkpoint', '--data-dir', m_s])
    run('merged-status-signed', [MERGED, 'status', '--data-dir', m_s])
    run('merged-import-remote-unsigned', [MERGED, 'snapshot-import-finalized-checkpoint', '--data-dir', m_u,
                                          '--snapshot-dir', C / 'remote-unsigned', '--node-id', 'validator-1'])
    run('merged-verify-checkpoint-remote', [MERGED, 'verify-finalized-checkpoint', '--data-dir', m_u])
    run('merged-status-remote', [MERGED, 'status', '--data-dir', m_u])
    run('merged-verify-state-remote', [MERGED, 'verify-state', '--data-dir', m_u])
    run('merged-import-wrong-key', [MERGED, 'snapshot-import-signed-finalized-checkpoint', '--data-dir', neg,
                                    '--snapshot-dir', signed, '--trusted-publisher-key-file', DEPLOY_PUB,
                                    '--node-id', 'validator-1'], expect_fail=True)
    run('r4-import-signed', [R4BIN, 'snapshot-import-signed-finalized-checkpoint', '--data-dir', r4_s,
                             '--snapshot-dir', signed, '--trusted-publisher-key-file', PUB, '--node-id', 'validator-1'])
    run('r4-verify-checkpoint-signed', [R4BIN, 'verify-finalized-checkpoint', '--data-dir', r4_s])
    run('merged-verify-state-signed', [MERGED, 'verify-state', '--data-dir', m_s])
    print(f'{now()} DONE', flush=True)


if __name__ == '__main__':
    main()
