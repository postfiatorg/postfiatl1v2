"""The release the six validators run, and the rehearsal tool built from the same source.

No hand-typed pin: the job reads the release from the six `status` replies and
requires its line in known-releases.txt, which names the rehearsal tool's hash.
`rehearsal_tool.sh` builds that tool after a validator release and prints the line.
Every refusal is one sentence: what is wrong, then the one command that fixes it.
"""
import hashlib,json,shlex,subprocess
from pathlib import Path
HERE=Path(__file__).resolve().parent
KNOWN=HERE/'known-releases.txt'
TOOL='postfiat-b23-nav-canary'
CHECK='python3 ops/b24-live/daily.py --check'
VALIDATORS=[f'validator-{i}' for i in range(6)]

class Refusal(RuntimeError):
    """The B24 job must not run against this fleet or tool."""

def build_command(revision,node_sha):
    return f'ops/b24-live/rehearsal_tool.sh <release-name> {revision} {node_sha}'

def fleet(statuses):
    """{validator-N: status reply, or the exception reading it} -> the one release all six run."""
    rows={}
    for name in VALIDATORS:
        s=statuses.get(name)
        if not isinstance(s,dict):raise Refusal(f'{name} gave no status reply ({s}); restore its RPC tunnel, then run: {CHECK}')
        artifacts=s.get('deployment_runtime_artifacts')
        missing=[k for k in ('build_git_revision','deployment_manifest_verified') if k not in s]+([] if isinstance(artifacts,dict) and artifacts.get('binary_sha256') else ['deployment_runtime_artifacts.binary_sha256'])
        if missing:raise Refusal(f'{name} status has no {", ".join(missing)}, so it does not run a manifest-checked node; finish installing the release on it, then run: {CHECK}')
        if s['deployment_manifest_verified'] is not True:raise Refusal(f'{name} reports deployment_manifest_verified={json.dumps(s["deployment_manifest_verified"])}; reinstall its signed deployment manifest and restart it, then run: {CHECK}')
        rows[name]=(s['build_git_revision'],artifacts['binary_sha256'])
    if len(set(rows.values()))!=1:
        seen='; '.join(f'{n} {r} {h[:16]}' for n,(r,h) in rows.items())
        raise Refusal(f'The six validators do not run one release ({seen}); finish the rollout on all six, then run: {CHECK}')
    revision,node=rows[VALIDATORS[0]]
    return dict(revision=revision,node_sha256=node)

def relative(path):
    path=Path(path);return path.relative_to(HERE.parents[1]) if path.is_relative_to(HERE.parents[1]) else path

def known(path=None):
    """Lines `release-name source-commit node-sha256 tool-sha256`; # starts a comment."""
    rows=[];path=Path(path or KNOWN)
    for number,line in enumerate(path.read_text().splitlines(),1):
        fields=line.split('#')[0].split()
        if fields:
            if len(fields)!=4:raise Refusal(f'{relative(path)}:{number} needs four fields (release, commit, node sha256, tool sha256); fix that line, then run: {CHECK}')
            rows.append(dict(zip(('name','commit','node_sha256','tool_sha256'),fields),line=number))
    return rows

def same(a,b):
    """Two spellings of one git revision (the node reports 8 hex digits)."""
    return len(a)>=7 and len(b)>=7 and (a.startswith(b) or b.startswith(a))

def tool_revision(tool):
    """The revision the tool records, or None for a tool built before it recorded one."""
    result=subprocess.run([str(tool),'build-info'],capture_output=True,text=True,timeout=30)
    if result.returncode:return None
    try:return json.loads(result.stdout)['build_git_revision']
    except (ValueError,KeyError,TypeError):return None

def release(found,coord,path=None):
    """The known-releases line for the fleet and its installed tool; refuses any other source."""
    path=Path(path or KNOWN);revision,node=found['revision'],found['node_sha256'];rel=relative(path)
    rows=[r for r in known(path) if same(r['commit'],revision)]
    if not rows:
        for tool in sorted(Path(coord).glob('artifacts/*/'+TOOL)):
            if same(tool_revision(tool) or '',revision):
                line=f'{tool.parent.name} {revision} {node} {hashlib.sha256(tool.read_bytes()).hexdigest()}'
                raise Refusal(f'The six validators run {revision} (node {node}), which {rel} does not list, and {tool} was built from it; add its line with: echo {shlex.quote(line)} >> {rel}')
        raise Refusal(f'The six validators run {revision} (node {node}), which {rel} does not list; build its rehearsal tool and print the line to add with: {build_command(revision,node)}')
    row=rows[-1]
    if row['node_sha256']!=node:raise Refusal(f'{rel}:{row["line"]} gives {row["name"]} the node {row["node_sha256"]}, but the six run {node}; rebuild its rehearsal tool and replace that line with the one printed by: {build_command(revision,node).replace("<release-name>",row["name"])}')
    tool=Path(coord)/'artifacts'/row['name']/TOOL;fix=build_command(revision,node).replace('<release-name>',row['name'])
    if not tool.is_file():raise Refusal(f'The rehearsal tool for {row["name"]} is missing at {tool}; build and install it with: {fix}')
    digest=hashlib.sha256(tool.read_bytes()).hexdigest()
    if digest!=row['tool_sha256']:raise Refusal(f'The rehearsal tool at {tool} has sha256 {digest}, not the {row["tool_sha256"]} that {rel}:{row["line"]} lists for {row["name"]}; rebuild it from {revision} with: {fix}')
    recorded=tool_revision(tool)
    if recorded is not None and not same(recorded,revision):raise Refusal(f'The rehearsal tool at {tool} was built from {recorded}, not the fleet\'s {revision}; rebuild it with: {fix}')
    return dict(name=row['name'],commit=row['commit'],revision=revision,node_sha256=node,tool=str(tool),tool_sha256=digest,tool_revision=recorded or f'{row["commit"]} (from {rel}:{row["line"]})')
