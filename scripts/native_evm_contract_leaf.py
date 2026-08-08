#!/usr/bin/env python3
"""Bounded EVM contract transaction custody leaf."""
from __future__ import annotations
import argparse, json, os, time
from pathlib import Path
from urllib.request import Request, urlopen
try:
    from . import native_agentd_leaf
except ImportError:
    import native_agentd_leaf

def rpc(url, method, params):
    req = Request(
        url,
        data=json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode(),
        headers={
            "content-type": "application/json",
            "user-agent": "a666-native-contract-leaf/1.0",
        },
    )
    with urlopen(req,timeout=15) as res: obj=json.loads(res.read().decode())
    if obj.get("error"): raise RuntimeError(str(obj["error"]))
    return obj.get("result")
def num(v): return int(v,16) if isinstance(v,str) and v.startswith("0x") else int(v)
def receipt(url,tx):
    for _ in range(120):
        got=rpc(url,"eth_getTransactionReceipt",[tx])
        if got:return got
        time.sleep(1)
    raise RuntimeError("receipt timeout")
def main(argv=None):
    p=argparse.ArgumentParser(); p.add_argument("--to",required=True); p.add_argument("--calldata",required=True); p.add_argument("--value-wei",type=int,required=True); p.add_argument("--chain-id",type=int,required=True); p.add_argument("--rpc-url",required=True); p.add_argument("--stakehub-home",required=True); p.add_argument("--artifact-dir",required=True); p.add_argument("--fee-ceiling-wei",type=int,required=True); p.add_argument("--label",required=True); a=p.parse_args(argv)
    try:
        if a.chain_id!=1: raise ValueError("chain-id must be 1")
        data=Path(a.calldata[1:]).read_text().strip() if a.calldata.startswith("@") else a.calldata
        if not data.startswith("0x"): raise ValueError("calldata must be 0x hex or @file")
        int(data[2:] or "0",16)
        gas=num(rpc(a.rpc_url,"eth_estimateGas",[{"to":a.to,"data":data,"value":hex(a.value_wei)}])); price=num(rpc(a.rpc_url,"eth_gasPrice",[]))
        if gas*price>a.fee_ceiling_wei: raise RuntimeError("estimated fee exceeds ceiling")
        os.environ.setdefault("EVM_RPC_URL",a.rpc_url)
        tx = native_agentd_leaf.evm_contract_tx(
            a.stakehub_home, a.chain_id, a.to, data, a.value_wei, a.label
        )
        if not isinstance(tx, str) or not tx:
            raise RuntimeError("agent response omitted tx hash")
        tx = tx if tx.startswith("0x") else f"0x{tx}"
        if len(tx) != 66:
            raise RuntimeError("agent response returned malformed tx hash")
        int(tx[2:], 16)
        rec=receipt(a.rpc_url,tx)
        if num(rec.get("status",0))!=1: raise RuntimeError("transaction reverted")
        out=Path(a.artifact_dir); out.mkdir(parents=True,exist_ok=True); report={"tx_hash":tx,"status":1,"block_number":num(rec.get("blockNumber",0)),"to":a.to,"value_wei":a.value_wei,"gas_used":num(rec.get("gasUsed",gas)),"effective_gas_price":num(rec.get("effectiveGasPrice",price)),"calldata":data}
        (out/"contract-tx-report.json").write_text(json.dumps(report,indent=2,sort_keys=True)+"\n"); print(json.dumps(report,sort_keys=True)); return 0
    except (OSError,RuntimeError,ValueError,TypeError) as e: print(f"STOP-no-retry: {e}"); return 2
if __name__=="__main__": raise SystemExit(main())
