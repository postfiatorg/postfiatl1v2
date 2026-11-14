"""Existing six validator loopback tunnels, with no automatic transaction retry."""
import json,socket,time
def envelope(method,params=None,node=0,timeout=90):
    with socket.create_connection(('127.0.0.1',39650+node),20) as s:
        s.settimeout(timeout);s.sendall((json.dumps(dict(version='postfiat-local-rpc-v1',id='b24-'+str(time.time_ns()),method=method,params=params or {}))+'\n').encode())
        return json.loads(s.makefile().readline())
def call(method,params=None,node=0):
    r=envelope(method,params,node)
    if not r.get('ok'):raise RuntimeError(method+': '+json.dumps(r.get('error')))
    return r['result']
