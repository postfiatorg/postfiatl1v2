"""Generate the machine-made parts of deployments/store-handle-20261021/ from the local stage and qualification
(fastlane-cap-20261014 tools/release_dir.py)."""
import hashlib, json, pathlib, re, shutil, sys
R = 'store-handle-20261021'; PREV = 'fastlane-cap-20261014'
Q = pathlib.Path('/home/postfiatchad/.cache/release-store-handle-20261021')
SRC_TREE = pathlib.Path(sys.argv[1])
OUT = SRC_TREE/'deployments'/R
OLD = pathlib.Path('/home/postfiatchad/repos/postfiatl1v2-rel20261014/deployments')/PREV   # branch release/fastlane-cap-20261014
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
assert f'for {PREV} (adapted from signer-rotation-20261007)' in lp and f'deploy-{PREV}' in lp
(OUT/'local-preflight.py').write_text(lp.replace(f'for {PREV} (adapted from signer-rotation-20261007)', f'for {R} (adapted from {PREV})').replace(f'deploy-{PREV}', f'deploy-{R}'))
of = (OLD/'observe-fleet.py').read_text()
assert f'observation for {PREV}' in of and 'the rest on signer-rotation-20261007' in of
of = of.replace(f'observation for {PREV}', f'observation for {R}').replace('the rest on signer-rotation-20261007', f'the rest on {PREV}')
of, n = re.subn(r'PREVIOUS = \{.*?\n\}', 'PREVIOUS = {\n    "release_id": "fastlane-cap-20261014",\n    "binary_sha256": "b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405",\n    "build_git_revision": "8bed9b90",\n    "manifest_sha256": "4f379c57b42c4a050154a344e7cea72e1af381e9f2a05ff21d3b2a1831465b48",\n}', of, flags=re.S)
assert n == 1
(OUT/'observe-fleet.py').write_text(of)
dp = (OLD/'demo-preflight.py').read_text(); assert f'({PREV})' in dp
(OUT/'demo-preflight.py').write_text(dp.replace(f'({PREV})', f'({R})'))
# rollback-one.sh: back to fastlane-cap-20261014 with that release's unit hashes.
rb = (OLD/'rollback-one.sh').read_text()
pairs = [(f'return ONE validator from {PREV} to\n# signer-rotation-20261007 (executable decaa411...)', f'return ONE validator from {R} to\n# {PREV} (executable b1b3dbb1...)'),
         ('signer-rotation-20261007/rollback-one.sh (branch release/signer-rotation-20261007)', f'{PREV}/rollback-one.sh (branch release/{PREV})'),
         ("validator's two signer-rotation-20261007 units from\n# the local 2026-09-28 stage into the host's 2026-10-07 release directory", f"validator's two {PREV} units from\n# the local 2026-10-15 stage into the host's 2026-10-15 release directory"),
         ('signed 2026-10-07 manifest', 'signed 2026-10-15 manifest'), ('the 2026-10-07 executable', 'the 2026-10-15 executable'),
         ('signer-rotation-20261007', PREV), ('decaa411376a125fb377a29037b6dd470afb2208a268d30a3ce8130f571b370f', 'b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405')]
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
