import json, subprocess, sys, pathlib
root=pathlib.Path('/work/g/main-ledger/public-capture')
if not list(root.glob('finality-*.jsonl')):
    sys.exit('no head window yet')
j=sorted(root.glob('finality-*.jsonl'))[-1]
_,first,last=j.stem.split('-')
p=subprocess.run([sys.executable,'/work/g/pairs.py',str(root)],capture_output=True,text=True,check=True)
rows=[]
for ln in p.stdout.splitlines():
    s,c=map(int,ln.split())
    if int(first)<=s<=c<=int(last) and (s<=c-1):
        t,=root.glob(str(s)+'-*');k,=root.glob(str(c)+'-*')
        rows.append(dict(target=s,carrier=c,t=t.name,k=k.name,j=j.name,target_state_size=(t/'state-body.bin').stat().st_size))
print(json.dumps(sorted(rows,key=lambda r:r['target_state_size'])))
