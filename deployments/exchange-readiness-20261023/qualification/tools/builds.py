"""Two clean postfiat-node builds, 2026-09-28 procedure (fastlane-cap-20261014 tools/builds.py)."""
import datetime, hashlib, json, os, pathlib, shutil, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor

ROOT = pathlib.Path('/home/postfiatchad/.cache/release-exchange-readiness-20261023')
REPO = pathlib.Path('/home/postfiatchad/repos/postfiatl1v2')
SOURCE = sys.argv[1]
CACHE = pathlib.Path('/home/postfiatchad/.cache/release-repair-20260928/target-1/release')
JOBS = '6'
ENV = dict(CARGO_BUILD_JOBS=JOBS, CARGO_INCREMENTAL='0', CARGO_TERM_COLOR='never', TMPDIR=str(ROOT / 'tmp'),
           RUST_TEST_THREADS='2', RAYON_NUM_THREADS='2', OMP_NUM_THREADS='2', MALLOC_ARENA_MAX='2', PYTHONDONTWRITEBYTECODE='1')
FLAGS = ' '.join([f'--remap-path-prefix={ROOT}/source-{i}=/src/postfiatl1v2' for i in (1, 2)]
                 + [f'--remap-path-prefix={ROOT}/target-{i}=/target' for i in (1, 2)])
COMMON = dict(SOURCE_DATE_EPOCH='1789514690', CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER='/usr/bin/gcc', RUSTFLAGS=FLAGS)
CMD = ['cargo', 'build', '--release', '--locked', '-p', 'postfiat-node', '--bin', 'postfiat-node']


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def sha(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def git(cwd, *args):
    return subprocess.check_output(['git', *args], cwd=cwd, text=True).strip()


def build(i):
    cwd, target = ROOT / f'source-{i}', ROOT / f'target-{i}'
    subprocess.run(['git', 'worktree', 'add', '--detach', str(cwd), SOURCE], cwd=REPO, check=True)
    assert git(cwd, 'rev-parse', 'HEAD') == SOURCE
    clean = git(cwd, 'status', '--porcelain') == ''
    assert clean
    target.mkdir()
    subprocess.run(['cp', '-a', str(CACHE), str(target / 'release')], check=True)
    extra = COMMON | dict(CARGO_TARGET_DIR=str(target))
    env = os.environ | ENV | extra
    rec = dict(name=f'node-build-{i}', command=CMD, cwd=str(cwd), started_at=now(),
               stdout=f'logs/node-build-{i}.stdout', stderr=f'logs/node-build-{i}.stderr',
               environment_overrides=ENV | extra)
    t0 = time.monotonic()
    with open(ROOT / 'logs' / f'node-build-{i}.stdout', 'wb') as o, open(ROOT / 'logs' / f'node-build-{i}.stderr', 'wb') as e:
        code = subprocess.run(CMD, cwd=cwd, env=env, stdout=o, stderr=e).returncode
    rec.update(exit_code=code, status='PASS' if code == 0 else 'FAIL', completed_at=now(),
               elapsed_seconds=round(time.monotonic() - t0, 1), source_commit=SOURCE,
               source_tree=git(cwd, 'rev-parse', 'HEAD^{tree}'), source_clean_at_collection=clean,
               cargo_lock_sha256=sha(cwd / 'Cargo.lock'), cache_origin=str(CACHE),
               cache_scope='separate copy of the 2026-09-28 release-profile cache; not empty-cache')
    if code == 0:
        binary = ROOT / 'binaries' / f'candidate-{i}'
        shutil.copy2(target / 'release/postfiat-node', binary)
        rec.update(binary_sha256=sha(binary), binary_bytes=binary.stat().st_size, binary_path=str(binary))
        dynamic = subprocess.check_output(['readelf', '-d', str(binary)], text=True)
        comment = subprocess.run(['readelf', '-p', '.comment', str(binary)], capture_output=True, text=True).stdout
        (ROOT / 'logs' / f'node-build-{i}-dynamic.stdout').write_text(dynamic)
        (ROOT / 'logs' / f'node-build-{i}-comment.stdout').write_text(comment)
        rec['has_rpath_or_runpath'] = any(t in dynamic for t in ('(RPATH)', '(RUNPATH)'))
        rec['zig_in_comment'] = 'zig' in comment.lower()
        rec['gcc_in_comment'] = 'GCC' in comment
        rec['source_clean_after_build'] = git(cwd, 'status', '--porcelain') == ''
        if rec['has_rpath_or_runpath'] or rec['zig_in_comment'] or not rec['source_clean_after_build']:
            rec.update(status='FAIL', error='RUNPATH, linker or source cleanliness gate failed')
    return rec


for d in ('logs', 'binaries', 'tmp'):
    (ROOT / d).mkdir(parents=True, exist_ok=True)
with ThreadPoolExecutor(2) as pool:
    builds = list(pool.map(build, (1, 2)))
result, cmp = 'FAIL', None
if all(b['status'] == 'PASS' for b in builds):
    c = subprocess.run(['cmp', ROOT / 'binaries/candidate-1', ROOT / 'binaries/candidate-2'], capture_output=True, text=True)
    cmp = dict(name='node-reproducibility', command=['cmp', 'binaries/candidate-1', 'binaries/candidate-2'],
               exit_code=c.returncode, stdout=c.stdout, status='PASS' if c.returncode == 0 else 'FAIL')
    result = cmp['status']
toolchain = dict(rustc=subprocess.check_output(['rustc', '-Vv'], cwd=REPO, text=True).strip().splitlines(),
                 cargo=subprocess.check_output(['cargo', '-V'], cwd=REPO, text=True).strip(),
                 gcc=subprocess.check_output(['/usr/bin/gcc', '--version'], text=True).splitlines()[0],
                 rust_toolchain_file=(REPO / 'rust-toolchain.toml').read_text() if (REPO / 'rust-toolchain.toml').exists() else None)
out = dict(result=result, source_commit=SOURCE, toolchain=toolchain, builds=builds, comparison=cmp,
           scope='Two clean worktrees of the tip, built in parallel; distinct copied caches; identical remaps; GCC linker; '
                 'SOURCE_DATE_EPOCH and RUSTFLAGS as on 2026-09-28; CARGO_BUILD_JOBS 6 instead of 2 (scheduling only, '
                 'does not change codegen)')
(ROOT / 'node-builds.json').write_text(json.dumps(out, indent=2) + '\n')
print(json.dumps(dict(result=result, hashes=[b.get('binary_sha256') for b in builds])), flush=True)
