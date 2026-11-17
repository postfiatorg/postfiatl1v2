#!/usr/bin/env python3
"""Compile a small offline harness using existing Cargo artifacts read-only.

No cargo target is created or mutated. For a clean build use the dependencies
and versions in tools/pfusdc-tier4-prover or tools/b23-nav-canary/Cargo.lock.
"""
import argparse, json, subprocess
from pathlib import Path

def build(target, reference, source, output, names):
    fp=target/'.fingerprint'
    refs=list(fp.glob(reference+'*/bin-*.json'))
    assert refs, reference
    ref=max(refs,key=lambda p:p.stat().st_mtime_ns)
    selected={}
    def visit(path):
        for _,name,_,digest in json.loads(path.read_text())['deps']:
            if name in selected:continue
            candidates=[]
            for p in fp.glob(name.replace('_','-')+'*/lib-'+name):
                if int.from_bytes(bytes.fromhex(p.read_text().strip()),'little')==digest:candidates.append(p)
            # Cargo keeps underscores in some package names.
            for p in fp.glob(name+'*/lib-'+name):
                if int.from_bytes(bytes.fromhex(p.read_text().strip()),'little')==digest and p not in candidates:candidates.append(p)
            if not candidates:continue
            p=candidates[0];h=p.parent.name.rsplit('-',1)[1]
            lib=target/'deps'/f'lib{name}-{h}.rlib'
            if not lib.exists():continue
            selected[name]=lib
            visit(Path(str(p)+'.json'))
    visit(ref)
    # The proof library is used by the pfusdc host, not the receipt host.
    for name in names:
        if name not in selected:
            refs2=list(fp.glob(name.replace('_','-')+'*/lib-'+name+'.json'))
            assert len(refs2)==1,(name,refs2)
            p=refs2[0];selected[name]=target/'deps'/f'lib{name}-{p.parent.name.rsplit("-",1)[1]}.rlib';visit(p)
    output.parent.mkdir(parents=True,exist_ok=True)
    cmd=['rustc','--edition=2021','-C','opt-level=2','-C','debuginfo=0','-L','dependency='+str(target/'deps'),str(source),'-o',str(output)]
    for name in names:cmd+=['--extern',name+'='+str(selected[name])]
    # Native SDK dependencies ship link search paths in build-script output.
    for p in (target/'build').glob('*/output'):
        for line in p.read_text(errors='replace').splitlines():
            if line.startswith('cargo:rustc-link-search='):cmd+=['-L',line.split('=',1)[1]]
    output.with_suffix('.build.json').write_text(json.dumps({'reference':str(ref),'command':cmd},indent=2)+'\n')
    subprocess.run(cmd,check=True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('target',type=Path);p.add_argument('reference');p.add_argument('source',type=Path);p.add_argument('output',type=Path);p.add_argument('names',nargs='+');a=p.parse_args();build(**vars(a))
