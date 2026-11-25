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

# A validator can answer a status read with rpc_status_failed for a moment after a new block.
RETRIED=('storage_writer_busy','rpc_child_timeout','rpc_server_error','rpc_status_failed','Database already open. Cannot acquire lock.')
def read(method,params=None,node=0):
    """Retry only bounded, idempotent status/receipt reads after lease failures."""
    assert method in ('status','receipts')
    for attempt in range(12):
        try:return call(method,params,node)
        except RuntimeError as error:
            if not any(x in str(error) for x in RETRIED) or attempt==11:
                raise RuntimeError(f'validator-{node}: {error}') from error
            time.sleep(2)
