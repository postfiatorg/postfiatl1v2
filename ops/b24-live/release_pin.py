"""The release the six validators run, and the rehearsal tool built from the same source.

No hand-typed pin: the job reads the release from the six `status` replies and
requires its line in known-releases.txt, which names the rehearsal tool's hash.
`rehearsal_tool.sh` builds that tool after a validator release and prints the line.
Every refusal starts with the command that fixes it and is at most 280 characters, so the
alert hook, which keeps 300, carries it whole.
"""
import hashlib,json,re,shlex,subprocess
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

def short(value,size=16):
    """A hash or value shortened for a refusal, which must fit the alert hook."""
    text=str(value);return text if len(text)<=size else text[:size]+'…'

def tool_path(tool):
    """The tool's path from the coordination directory on, enough to find it."""
    parts=Path(tool).parts;return '/'.join(parts[-3:])

def fleet(statuses):
    """{validator-N: status reply, or the exception reading it} -> the one release all six run."""
    rows={}
    for name in VALIDATORS:
        s=statuses.get(name)
        if not isinstance(s,dict):raise Refusal(f'{CHECK} after restoring the RPC tunnel to {name}: it gave no status reply ({short(s,60)})')
        artifacts=s.get('deployment_runtime_artifacts')
        missing=[k for k in ('build_git_revision','deployment_manifest_verified') if k not in s]+([] if isinstance(artifacts,dict) and artifacts.get('binary_sha256') else ['deployment_runtime_artifacts.binary_sha256'])
        if missing:raise Refusal(f'{CHECK} after finishing the release install on {name}: its status has no {", ".join(missing)}')
        if not isinstance(s['build_git_revision'],str) or not s['build_git_revision']:raise Refusal(f'{CHECK} after finishing the release install on {name}: its status reports build_git_revision={short(json.dumps(s["build_git_revision"]),40)}')
        if s['deployment_manifest_verified'] is not True:raise Refusal(f'{CHECK} after reinstalling the signed deployment manifest on {name} and restarting it: it reports deployment_manifest_verified={short(json.dumps(s["deployment_manifest_verified"]),20)}')
        rows[name]=(s['build_git_revision'],artifacts['binary_sha256'])
    if len(set(rows.values()))!=1:
        groups={}
        for n,(r,h) in rows.items():groups.setdefault(f'{short(r,9)}/{short(h,8)}',[]).append(n.split('-')[1])
        seen='; '.join(f'{k} on {",".join(v)}' for k,v in groups.items())
        raise Refusal(f'{CHECK} after finishing the rollout on all six: they run {short(seen,190)}')
    revision,node=rows[VALIDATORS[0]]
    return dict(revision=revision,node_sha256=node)

def relative(path):
    path=Path(path);return path.relative_to(HERE.parents[1]) if path.is_relative_to(HERE.parents[1]) else path

def known(path=None):
    """Lines `release-name source-commit node-sha256 tool-sha256`; # starts a comment. A release name
    or a commit given twice is refused: the job would not know which line holds."""
    rows=[];path=Path(path or KNOWN);rel=relative(path)
    for number,line in enumerate(path.read_text().splitlines(),1):
        fields=line.split('#')[0].split()
        if not fields:continue
        if len(fields)!=4 or not re.fullmatch(r'[0-9a-f]{7,40}',fields[1]) or not all(re.fullmatch(r'[0-9a-f]{64}',f) for f in fields[2:]):
            raise Refusal(f'{CHECK} after fixing {rel}:{number}: a line is release, commit (7 to 40 hex), node sha256, tool sha256')
        row=dict(zip(('name','commit','node_sha256','tool_sha256'),fields),line=number)
        for other in rows:
            if other['name']==row['name'] or same(other['commit'],row['commit']):
                raise Refusal(f'{CHECK} after removing one of {rel}:{other["line"]} and {rel}:{number}: both name {short(row["name"],24) if other["name"]==row["name"] else "commit "+row["commit"]}')
        rows.append(row)
    return rows

def same(a,b):
    """Two spellings of one git revision (the node reports 8 hex digits)."""
    return len(a)>=7 and len(b)>=7 and (a.startswith(b) or b.startswith(a))

PLACED=re.compile(r'\+placed:((?:Cargo\.lock|build\.rs|src/main\.rs)=[0-9a-f]{16})(,(?:Cargo\.lock|build\.rs|src/main\.rs)=[0-9a-f]{16}){0,2}')
SOURCE=re.compile(r'([0-9a-f]{7,40})((?:\+placed:.*)?)')

def source_commit(recorded):
    """The commit of a recorded source text, or None: an unchanged tree (`<commit>`) or one whose only
    differences are files rehearsal_tool.sh put in place and named (`<commit>+placed:name=hash,...`).
    `+modified` and any other form give None."""
    m=SOURCE.fullmatch(recorded or '')
    if not m or (m.group(2) and not PLACED.fullmatch(m.group(2))):return None
    names=[part.split('=')[0] for part in m.group(2)[len('+placed:'):].split(',')] if m.group(2) else []
    return m.group(1) if len(names)==len(set(names)) else None

def tool_revision(tool):
    """The source text the tool records, or None when it does not answer build-info."""
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
            if same(source_commit(tool_revision(tool)) or '',revision):
                line=f'{tool.parent.name} {revision} {node} {hashlib.sha256(tool.read_bytes()).hexdigest()}'
                raise Refusal(f'echo {shlex.quote(line)} >> {rel}: {revision} is unlisted; its tool is already built')
        raise Refusal(f'{build_command(revision,node)}: the six run {revision}, which {rel} does not list; it builds the tool and prints the line to add')
    row=rows[0];fix=build_command(revision,node).replace('<release-name>',row['name'])
    if row['node_sha256']!=node:raise Refusal(f'{fix}: {rel}:{row["line"]} gives {row["name"]} the node {short(row["node_sha256"])}, the six run {short(node)}; replace that line with the one it prints')
    tool=Path(coord)/'artifacts'/row['name']/TOOL
    if not tool.is_file():raise Refusal(f'{fix}: no rehearsal tool for {row["name"]} at {tool_path(tool)}')
    digest=hashlib.sha256(tool.read_bytes()).hexdigest()
    if digest!=row['tool_sha256']:raise Refusal(f'{fix}: the installed tool has sha256 {short(digest)}, {rel}:{row["line"]} lists {short(row["tool_sha256"])}; the build is reproducible')
    recorded=tool_revision(tool)
    if recorded is None:raise Refusal(f'{fix}: the installed tool does not record its source (no build-info)')
    commit=source_commit(recorded)
    if commit is None:raise Refusal(f'{fix}: the installed tool records {short(recorded,40)}, a tree modified from its commit')
    if not same(commit,revision):raise Refusal(f'{fix}: the installed tool was built from {short(recorded,40)}, the six run {revision}')
    return dict(name=row['name'],commit=row['commit'],revision=revision,node_sha256=node,tool=str(tool),tool_sha256=digest,tool_revision=recorded)
