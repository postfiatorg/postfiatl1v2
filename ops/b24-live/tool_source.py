#!/usr/bin/env python3
"""The source text a rehearsal tool records: the commit it was built from, then how its tree differs.

`<commit>` (9 hex digits) for an unchanged tree. `<commit>+placed:<name>=<sha256[:16]>,...` when the
only differences are files rehearsal_tool.sh put in place for a commit that predates them (build.rs,
src/main.rs and Cargo.lock of tools/b23-nav-canary, each equal to the given content), names sorted.
`<commit>+modified` for any other difference. The job refuses a `+modified` tool.
Usage: tool_source.py <worktree> [<relative path> ...]
"""
import hashlib,subprocess,sys
from pathlib import Path
TOOL='tools/b23-nav-canary/'
PLACEABLE=('build.rs','src/main.rs','Cargo.lock')

def changed(tree):
    out=subprocess.run(['git','-C',str(tree),'status','--porcelain=v1','--untracked-files=all','-z'],capture_output=True,text=True,check=True).stdout
    return sorted({entry[3:] for entry in out.split('\0') if entry})

def text(tree,placed=()):
    tree=Path(tree)
    commit=subprocess.run(['git','-C',str(tree),'rev-parse','--short=9','HEAD'],capture_output=True,text=True,check=True).stdout.strip()
    names=sorted(p[len(TOOL):] for p in placed if p.startswith(TOOL))
    if len(names)!=len(placed) or any(n not in PLACEABLE for n in names) or changed(tree)!=sorted(placed):
        return commit+'+modified'
    if not names:return commit
    return commit+'+placed:'+','.join(f'{n}={hashlib.sha256((tree/TOOL/n).read_bytes()).hexdigest()[:16]}' for n in names)

if __name__=='__main__':print(text(sys.argv[1],sys.argv[2:]))
