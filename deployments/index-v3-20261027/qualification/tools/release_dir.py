"""Generate the machine-made parts of deployments/index-v3-20261027/ from the local stage and qualification
(exchange-readiness-20261023 tools/release_dir.py)."""
import hashlib, json, pathlib, re, shutil, sys
R = 'index-v3-20261027'; PREV = 'exchange-readiness-20261023'; PREV2 = 'store-handle-20261021'
Q = pathlib.Path('/home/postfiatchad/.cache/release-index-v3-20261027')
SRC_TREE = pathlib.Path(sys.argv[1])
OUT = SRC_TREE/'deployments'/R
OLD = Q/'prev/deployments'/PREV   # git archive of branch release/exchange-readiness-20261023
S = pathlib.Path(f'/home/postfiatchad/.postfiat/deployments/{R}/stage')
C = S/'rootfs/etc/postfiat/releases'/R; U = S/'rootfs/etc/systemd/system'
sha = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
OUT.mkdir(parents=True, exist_ok=True)
dst = OUT/'rootfs/etc'
shutil.copytree(U, dst/'systemd/system', dirs_exist_ok=True)
(dst/'postfiat/releases'/R).mkdir(parents=True, exist_ok=True)
for f in sorted(C.iterdir()):
    if f.name not in ('deployment-manifest.json', 'deployment.public.json'):
        shutil.copy2(f, dst/'postfiat/releases'/R/f.name)
shutil.copy2(C/'deployment-manifest.json', OUT/'deployment-manifest.signed.json')
for name, target in (('stage-report.json', 'stage-report.template.json'), ('validator-bindings.signing.json', 'validator-bindings.signing.template.json')):
    (OUT/target).write_text((S/name).read_text().replace(str(S), '@STAGE@'))
manifest = json.loads((C/'deployment-manifest.json').read_text())
inp = json.loads((OLD/'manifest-input.unsigned.json').read_text())
inp.update(deployment_id=R, previous_release_id=PREV, git_revision=manifest['git_revision'], binary_sha256=manifest['binary_sha256'],
           service_unit_file='rootfs/etc/systemd/system/postfiat-validator-0-rpc.service',
           environment_file=f'rootfs/etc/postfiat/releases/{R}/validator-0.rpc.env',
           service_unit_sha256=manifest['service_unit_sha256'], environment_sha256=manifest['environment_sha256'],
           validator_bindings=manifest['validator_bindings'], topology_sha256=manifest['topology_sha256'],
           swap_circuit_metadata_sha256=manifest['swap_circuit_metadata_sha256'],
           private_egress_circuit_metadata_sha256=manifest['private_egress_circuit_metadata_sha256'],
           trusted_publisher=manifest['publisher'], trusted_publisher_file_sha256=sha(C/'deployment.public.json'))
for k in ('chain_id', 'genesis_hash', 'build_profile', 'build_features', 'protocol_version', 'rpc_schema'):
    assert inp[k] == manifest[k], k
assert sha(OUT/inp['service_unit_file']) == inp['service_unit_sha256'] and sha(OUT/inp['environment_file']) == inp['environment_sha256']
(OUT/'manifest-input.unsigned.json').write_text(json.dumps(inp, indent=2)+'\n')
shutil.copy2(OLD/'inventory.txt', OUT/'inventory.txt')
shutil.copy2(Q/'node-builds.json', OUT/'node-builds.json')
# Scripts adapted from the previous release.
lp = (OLD/'local-preflight.py').read_text()
assert f'for {PREV} (adapted from {PREV2})' in lp and f'deploy-{PREV}' in lp
(OUT/'local-preflight.py').write_text(lp.replace(f'for {PREV} (adapted from {PREV2})', f'for {R} (adapted from {PREV})').replace(f'deploy-{PREV}', f'deploy-{R}'))
of = (OLD/'observe-fleet.py').read_text()
assert f'observation for {PREV}' in of and f'the rest on {PREV2}' in of
of = of.replace(f'observation for {PREV}', f'observation for {R}').replace(f'the rest on {PREV2}', f'the rest on {PREV}')
of, n = re.subn(r'PREVIOUS = \{.*?\n\}', 'PREVIOUS = {\n    "release_id": "exchange-readiness-20261023",\n    "binary_sha256": "4085ac70c7bb3c21b46cc67be8cb1bbb3556bfea9f49fbf6b128cdac2adcbef9",\n    "build_git_revision": "6b1a0585",\n    "manifest_sha256": "210544a8627ab366f6b982fe3d0f1994a6e2e2025db5b2cc3ce03139a0923bc1",\n}', of, flags=re.S)
assert n == 1
(OUT/'observe-fleet.py').write_text(of)
dp = (OLD/'demo-preflight.py').read_text(); assert f'({PREV})' in dp
(OUT/'demo-preflight.py').write_text(dp.replace(f'({PREV})', f'({R})'))
# rollback-one.sh: back to exchange-readiness-20261023 with that release's unit hashes.
rb = (OLD/'rollback-one.sh').read_text()
pairs = [(f'return ONE validator from {PREV} to\n# {PREV2} (executable eea96ba6...)', f'return ONE validator from {R} to\n# {PREV} (executable 4085ac70...)'),
         (f'{PREV2}/rollback-one.sh (branch release/{PREV2})', f'{PREV}/rollback-one.sh (branch release/{PREV})'),
         (f"validator's two {PREV2} units from\n# the local 2026-10-21 stage into the host's 2026-10-21 release directory", f"validator's two {PREV} units from\n# the local 2026-10-23 stage into the host's 2026-10-23 release directory"),
         ('signed 2026-10-21 manifest', 'signed 2026-10-23 manifest'), ('the 2026-10-21 executable', 'the 2026-10-23 executable'),
         (PREV2, PREV), ('eea96ba6761ed223047d7781663100d34efe5641d36031bd50874bd9c8b41f36', '4085ac70c7bb3c21b46cc67be8cb1bbb3556bfea9f49fbf6b128cdac2adcbef9')]
for a, b in pairs:
    assert a in rb, a
    rb = rb.replace(a, b)
for kind, suffix in (('transport', ''), ('rpc', '-rpc')):
    for i in range(6):
        want = sha(OLD/f'rootfs/etc/systemd/system/postfiat-validator-{i}{suffix}.service')
        rb, n = re.subn(rf'(declare -A want_{kind}=\((?:\n  \[validator-\d\]=[0-9a-f]+)*?\n  \[validator-{i}\]=)[0-9a-f]+', rf'\g<1>{want}', rb)
        assert n == 1
(OUT/'rollback-one.sh').write_text(rb); (OUT/'rollback-one.sh').chmod(0o755)
print('generated', sum(1 for p in OUT.rglob('*') if p.is_file()), 'files')
