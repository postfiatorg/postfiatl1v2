#!/usr/bin/env python3
"""Orchestrator-only bridge switch. With no arguments, print the plan. See report.md."""
import argparse
import ast
import fcntl
import hashlib
import ipaddress
import json
import os
import shlex
import socket
import subprocess
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
LIVE = Path.home() / 'repos/postfiatl1v2-live/ops/sol-live'
STATE = Path('/mnt/HC_Volume_106212907/stakehub-hyperliquid/sol-live')
HOSTS = Path.home()/'.ssh/known_hosts'
JOURNAL = HERE/'cutover-transaction-mixed'
CAPTURE, GO, TUNNEL = 'mainnet-capture.py', 'mainnet-go.py', 'prover-tunnel.py'
FILES = {name: LIVE/name for name in (CAPTURE, GO, TUNNEL)}
FILES['gpu.json'] = STATE/'gpu.json'
PINS = {CAPTURE:'ee0fffc31742061981917261621864d5e8775815bf7b55460aec5c8bdc0c7674',
        GO:'42b714817f708d95e0eff39d0800fab692bbd5d4fb09452922cc9d3224f407d6',
        TUNNEL:'110230176465b838d1f07e0e6f5110ac62ce897446eae141fb0268fb52526c77'}
NEW = [(61511840,'solbridge-node-2'), (62235352,'solbridge-live-b')]
OLD = [(61511840,'solbridge-node-2'), (61487772,'solbridge-live')]
TIMER = 'pft-sol-mainnet-reanchor.timer'
ONE = 'pft-sol-mainnet-reanchor.service'
RELAY, HEALTH, LINK = ['pft-sol-'+x+'.service' for x in ('mainnet-relay','mainnet-health','prover-tunnel')]
CTL = ['systemctl','--user']
def require(ok,message):
    if not ok: raise SystemExit(message)
def out(args,**kwargs): return subprocess.check_output(args,text=True,**kwargs).strip()
def ctl(*args): subprocess.run(CTL+list(args),check=True)
def state(unit): return out(CTL+['show','-p','ActiveState','--value',unit])
def write(path,data,mode):
    with tempfile.NamedTemporaryFile(dir=path.parent,delete=False) as f:
        tmp=Path(f.name)
        try:
            os.fchmod(f.fileno(),mode)
            f.write(data); f.flush(); os.fsync(f.fileno())
        except BaseException:
            tmp.unlink(missing_ok=True)
            raise
    os.replace(tmp,path)
def originals():
    base={k:(HERE/'baseline'/k).read_bytes() for k in FILES}
    for k,v in PINS.items(): require(hashlib.sha256(base[k]).hexdigest()==v,'Baseline changed: '+k)
    require(json.loads(base['gpu.json'])=={'new_contract':61487772},'Unexpected original GPU selector')
    return base
def render(base,obs,new):
    data=dict(base)
    host,port=obs
    for name in (CAPTURE,GO):
        s=data[name].decode()
        for a,b in [('37.63.76.95',host),('37815',str(port))]:
            require(s.count(a)==2,'Unexpected endpoint occurrences in '+name)
            s=s.replace(a,b)
        ast.parse(s); data[name]=s.encode()
    if new:
        bill=b'instances = {61511840, read(STATE / "gpu.json")["new_contract"]}'
        require(data[GO].count(bill)==1,'Launch cost accounting changed')
        data[GO]=data[GO].replace(bill,b'instances = {61511840, 61487772, 62235343, read(STATE / "gpu.json")["new_contract"]}')
        marker=b'i["label"] == "solbridge-live"'
        require(data[TUNNEL].count(marker)==1,'Unexpected tunnel selector')
        data[TUNNEL]=data[TUNNEL].replace(marker,b'i["label"] == "solbridge-live-b"')
        # At boot and after an SSH reconnect, both paths share one lock.
        old_guard=(b'"while :; do if flock -n prover.lock true; then "\n'
                   b'        "nohup ./prover-start.sh >> supervised.log 2>&1 </dev/null & fi; sleep 10; done"')
        require(old_guard in data[TUNNEL],'Tunnel supervisor changed')
        data[TUNNEL]=data[TUNNEL].replace(old_guard,b'"while :; do ./prover-boot.sh; sleep 10; done"')
        ast.parse(data[TUNNEL])
        data['gpu.json']=b'{"new_contract":62235352}\n'
    return data
def endpoints(new):
    rows=json.loads(out([Path.home()/'.local/bin/vastai','show','instances','--raw']))
    pair=[]
    for ident,label in NEW if new else OLD:
        x=next((r for r in rows if r['id']==ident and r['label']==label),{})
        require(x.get('actual_status')=='running','Vast instance unavailable: '+label)
        mapping=(x.get('ports') or {}).get('22/tcp')
        require(mapping,'No direct SSH mapping for '+label)
        pair.append((str(ipaddress.ip_address(x['public_ipaddr'])),int(mapping[0]['HostPort'])))
    if new: require(pair==[('37.63.76.95',37815),('211.72.13.201',42940)],'Replacement mappings changed. Reverify host trust and gossip mapping.')
    return pair
def ssh(ep,kh,command):
    return out(['ssh','-T','-oBatchMode=yes','-oConnectTimeout=8','-oStrictHostKeyChecking=yes',
                '-oUpdateHostKeys=no','-oUserKnownHostsFile='+str(kh),'-p',str(ep[1]),'root@'+ep[0],command])
def ping_socket(path):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(10); s.connect(str(path)); s.sendall(b'status\n'); s.shutdown(socket.SHUT_WR)
        require(json.loads(s.makefile('rb').readline(512)).get('error')=='expected guest mode byte_length','Unexpected prover health reply')
def probe(pair,new,known):
    obs,gpu=pair
    status=(HERE/'observer-status.py').read_text()
    status=status.replace('154.42.3.203:40007',f'{obs[0]}:37521')
    result=json.loads(ssh(obs,HOSTS,'python3 -c '+shlex.quote(status)))
    print('Observer:',json.dumps(result),flush=True)
    require(result['ready'],'Observer has not reached capture and gossip readiness')
    cmd="import socket,json;s=socket.socket(socket.AF_UNIX);s.settimeout(10);s.connect('/workspace/sol-ingress/speed-i2/prover.sock');s.sendall(b'status\\n');s.shutdown(socket.SHUT_WR);print(s.makefile('rb').readline(512).decode())"
    require(json.loads(ssh(gpu,known,'python3 -c '+shlex.quote(cmd))).get('error')=='expected guest mode byte_length','Prover not ready')
    if new:
        fresh=json.loads(ssh(gpu,known,'cat /workspace/no-submit/verified.json'))
        active=json.loads((STATE/'mainnet/activation.json').read_text())['profile']
        require(fresh['verified'] and fresh['native_journal_match'] and fresh['program_vkey']==active['verifier_program_vkey'],'Replacement proof does not match the active verifier')
def hold():
    ctl('stop',TIMER)
    for _ in range(330):
        if state(ONE) not in ('activating','active','deactivating'): break
        time.sleep(3)
    else: raise SystemExit('Re-anchor still active; timer held')
    require(state('pft-sol-mainnet-go.timer')=='inactive' and state('pft-sol-mainnet-go.service') not in ('activating','active'),'Go launcher is active; coordinate the maintenance window')
    main=STATE/'mainnet'
    require(not (main/'relay-stopped.json').exists() and not list((main/'retrying').glob('*.json')),'Review the held relay job before switching')
    active=json.loads((main/'activation.json').read_text())['profile_hash']
    route=STATE/'relay'/active
    unfinished=[str(p.parent.name) for p in (route/'jobs').glob('*/job.json') if json.loads(p.read_text()).get('status') not in ('accepted','failed')]
    pending=[str(p.parent.name) for p in (route/'outbox').glob('*/signed.json') if not (p.parent/'finality.json').exists()]
    require(not unfinished and not pending,'In-flight relay work: '+str(unfinished+pending))
    ctl('stop',RELAY,HEALTH,LINK)
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action',nargs='?',default='plan',choices=['plan','check','apply','rollback','resume-timer'])
    action=parser.parse_args().action
    base=originals()
    for file,digest in [('bridge_known_hosts','1f6cfaac5ec223290ddd9a48b4a17c0168746eb013c44aec1d71b7d666f3c826'),('proxy_known_hosts','d18205c8b26e647cf5c2671b41daf1aae0ce76adbb01eab0987b8fd8068a91ec')]:
        require(hashlib.sha256((HERE/file).read_bytes()).hexdigest()==digest,'Host key file changed: '+file)
    if action=='plan':
        for k,p in FILES.items(): print(k,p)
        print('Add verified direct and proxy hosts to',HOSTS)
        print('Hold',TIMER,'and drain running oneshot. Reload',LINK,HEALTH,RELAY,'in that order. Timer resumes separately.')
        return
    if action=='resume-timer':
        flags=json.loads((JOURNAL/'units.json').read_text())
        require((JOURNAL/'switched').exists(),'Switch incomplete')
        for unit in (LINK,HEALTH,RELAY): require(state(unit)=='active',unit+' is not active')
        if flags[TIMER]=='active': ctl('start',TIMER)
        return
    new=action!='rollback'
    pair=endpoints(new)
    known=HERE/'bridge_known_hosts' if new else HOSTS
    probe(pair,new,known)
    if action=='check': return
    JOURNAL.mkdir(mode=0o700,exist_ok=True)
    with (JOURNAL/'lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        newfiles=render(base,('37.63.76.95',37815),True)
        target=render(base,pair[0],new)
        flagsfile=JOURNAL/'units.json'
        if not flagsfile.exists():
            require(new and all(p.read_bytes()==base[k] for k,p in FILES.items()),'Live sources differ from original baseline')
            flags={u:state(u) for u in (LINK,HEALTH,RELAY,TIMER)}
            require(flags[RELAY]=='active' and flags[HEALTH]=='active','Unexpected baseline health/relay state')
            for k,p in FILES.items(): write(JOURNAL/k,p.read_bytes(),0o600)
            write(flagsfile,json.dumps(flags).encode(),0o600)
        for k,p in FILES.items():
            require(not p.is_symlink() and p.read_bytes() in (base[k],newfiles[k],target[k]),'Unrecognized live change: '+str(p))
        # The timer stays held until the orchestrator explicitly resumes it. A failed
        # restart leaves the evidence and backups in place for a retry or rollback.
        hold()
        (JOURNAL/'switched').unlink(missing_ok=True)
        # Keep unrelated host enrollments, including any added during the window.
        block=b'# solbridge replacement contracts 62235343 and 62235352\n'+(HERE/'bridge_known_hosts').read_bytes()+(HERE/'proxy_known_hosts').read_bytes()+b'# end solbridge replacement contracts\n'
        previous=HOSTS.read_bytes()
        require(not previous or previous.endswith(b'\n'),'known_hosts has an incomplete last line')
        updated=(previous if block in previous else previous+block) if new else previous.replace(block,b'')
        if previous!=updated: write(HOSTS,updated,HOSTS.stat().st_mode & 0o777)
        for k,p in FILES.items():
            if p.read_bytes()!=target[k]: write(p,target[k],p.stat().st_mode & 0o777)
        ctl('restart',LINK)
        local=Path(os.environ.get('XDG_RUNTIME_DIR',f'/run/user/{os.getuid()}'))/'pft-sol-prover/prover.sock'
        for _ in range(45):
            try: ping_socket(local); break
            except (OSError,ValueError): time.sleep(2)
        else: raise SystemExit('Tunnel failed its socket check; health and relay are held')
        ctl('start',HEALTH); ctl('start',RELAY)
        for u in (LINK,HEALTH,RELAY): require(state(u)=='active','Check '+u)
        write(JOURNAL/'switched',(action+'\n').encode(),0o600)
        print(action,'complete; timer remains held. See report.md for re-anchor and readiness.')
if __name__=='__main__': main()
