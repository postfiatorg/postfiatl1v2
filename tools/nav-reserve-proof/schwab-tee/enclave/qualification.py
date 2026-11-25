"""Run committed synthetic integration fixtures against an immutable local image.

Test dependencies are mounted read-only; no production implementation is mounted.
This operator tool is excluded from the measured image.
"""
import argparse
import ast
import hashlib
import importlib
import importlib.metadata
import json
from pathlib import Path
import shutil
import subprocess

SOURCE_ROOT = 'tools/nav-reserve-proof/schwab-tee/'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path, required=True)
    parser.add_argument('--source-revision', required=True)
    parser.add_argument('--image-id', required=True)
    parser.add_argument('--output-directory', type=Path, required=True)
    args = parser.parse_args()
    def git(*values): return subprocess.check_output(['git', '-C', str(args.repository), *values])
    revision = git('rev-parse', '--verify', args.source_revision + '^{commit}').decode().strip()
    def source(path): return git('show', revision + ':' + path)
    image = json.loads(subprocess.check_output(['docker', 'image', 'inspect', args.image_id]))[0]
    if args.image_id != image['Id']:
        raise ValueError('qualification requires the complete immutable image ID')
    binary = '/usr/local/bin/python3.13'
    installed = json.loads(subprocess.check_output(['docker', 'run', '--rm', '--network', 'none',
        '--entrypoint', binary, args.image_id, '-I', '-B', '-c',
        'from pathlib import Path; print(Path("/app/build-inputs.json").read_text())']))
    for path, digest in installed['modules'].items():
        if hashlib.sha256(source(SOURCE_ROOT + path)).hexdigest() != digest:
            raise ValueError('fixture source revision differs from the measured source manifest: ' + path)
    out = args.output_directory.resolve(); out.mkdir(parents=True, exist_ok=False)
    dependencies = {}
    for name in ('pytest', '_pytest', 'pluggy', 'packaging', 'pygments', 'iniconfig'):
        path = Path(importlib.import_module(name).__file__).parent
        shutil.copytree(path, out / 'deps' / name, ignore=shutil.ignore_patterns('__pycache__'))
        if name != '_pytest': dependencies[name] = importlib.metadata.version(name)
    shutil.copy2(Path(importlib.import_module('py').__file__), out / 'deps/py.py')
    paths = ['tests/' + name for name in ('test_yolo_nitro_run.py', 'test_yolo_nitro_runtime.py',
        'test_yolo_nitro_cms.py', 'test_yolo_nitro_attestation.py', 'test_yolo_nitro_entrypoint.py')]
    paths += ['tests/fixtures/yolo_nitro_validation_v2.json', 'docs/pre_production/yolo_portfolio_parameters_v1.json']
    for path in paths:
        destination = out / path; destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(source(path))
    # Extract unchanged fixture generators; exclude host-only CLI imports/tests.
    tree = ast.parse(source('tests/test_yolo_methodology.py'))
    tree.body = [node for node in tree.body if isinstance(node, (ast.Import, ast.ImportFrom, ast.Assign))
        or isinstance(node, ast.FunctionDef) and node.name in ('parameters', 'quote', 'snapshots', 'target_input')]
    tree.body = [node for node in tree.body if not isinstance(node, ast.ImportFrom)
        or node.module != 'pftl_schwab_tee.portfolio_cli']
    (out / 'tests/test_yolo_methodology.py').write_text(ast.unparse(tree) + '\n')
    (out / 'bootstrap.py').write_text('''import importlib, sys
sys.path[:0] = ['/qualification/deps', '/qualification/tests']
import pftl_schwab_tee as package
# Shared fixtures use package facade names. Implementations come from the image.
for name in ('canonical','models','methodology','schwab_chain','schwab_methodology','nitro_runtime','nitro_kms'):
    module = importlib.import_module('pftl_schwab_tee.' + name)
    for key in vars(module):
        if not key.startswith('_'): setattr(package, key, getattr(module, key))
import pytest
raise SystemExit(pytest.main(['-q', '-p', 'no:cacheprovider', '/qualification/tests/test_yolo_nitro_entrypoint.py', '/qualification/tests/test_yolo_nitro_run.py']))
''')
    command = ['docker', 'run', '--rm', '--network', 'none', '--read-only', '--tmpfs', '/tmp:rw,noexec,nosuid',
        '--mount', f'type=bind,src={out},dst=/qualification,readonly', '--entrypoint', binary,
        args.image_id, '-I', '-B', '/qualification/bootstrap.py']
    with (out / 'tests.log').open('xb') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    report = {'imageId': args.image_id, 'sourceRevision': revision, 'testDependencies': dependencies,
        'command': command, 'exitCode': result.returncode,
        'logSha256': hashlib.sha256((out / 'tests.log').read_bytes()).hexdigest()}
    (out / 'qualification.json').write_text(json.dumps(report, indent=2) + '\n')
    print((out / 'tests.log').read_text())
    return result.returncode


if __name__ == '__main__':
    raise SystemExit(main())
