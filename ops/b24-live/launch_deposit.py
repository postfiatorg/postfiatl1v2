#!/usr/bin/env python3
"""Real USDC -> vault depositV2 from the OPS test EOA, exactly as the wallet's MetaMask path:
approve(vault, amount) then depositV2(amount, pftlRecipient, random32 nonce, routeBinding).
Key is read from its 0600 file and never printed. Refuses to run unless the bridge service
readiness says ready=true for the given binding (B10: never deposit what cannot be claimed).

Usage (StakeHub venv python for web3):
  eth_deposit.py --amount-atoms 1000000 --recipient pf... --route-binding 0x... --readiness-json FILE --out DIR
"""
import argparse, json, os, secrets, sys, time
from pathlib import Path
from web3 import Web3
from eth_account import Account

RPC = "https://ethereum-rpc.publicnode.com"
VAULT = Web3.to_checksum_address("0xe7a432a28b20621a70e845c7f202004cf98e7a2c")
USDC = Web3.to_checksum_address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
KEY = Path("/home/postfiat/.config/b24/eth-operator.key")
ERC20 = [{"name": "approve", "type": "function", "stateMutability": "nonpayable",
          "inputs": [{"name": "s", "type": "address"}, {"name": "a", "type": "uint256"}], "outputs": [{"type": "bool"}]},
         {"name": "allowance", "type": "function", "stateMutability": "view",
          "inputs": [{"name": "o", "type": "address"}, {"name": "s", "type": "address"}], "outputs": [{"type": "uint256"}]},
         {"name": "balanceOf", "type": "function", "stateMutability": "view",
          "inputs": [{"name": "o", "type": "address"}], "outputs": [{"type": "uint256"}]}]
VAULT_ABI = [{"name": "depositV2", "type": "function", "stateMutability": "nonpayable",
              "inputs": [{"name": "amount", "type": "uint256"}, {"name": "pftlRecipient", "type": "string"},
                         {"name": "nonce", "type": "bytes32"}, {"name": "routeBinding", "type": "bytes32"}],
              "outputs": [{"name": "depositId", "type": "bytes32"}]},
             {"name": "paused", "type": "function", "stateMutability": "view", "inputs": [], "outputs": [{"type": "bool"}]}]
EVENT_TOPIC = Web3.keccak(text="ERC20BridgeDepositedV2(bytes32,address,bytes32,string,uint256,bytes32,bytes32,uint256,address,address)").hex()


def live_route(port=39652):
    import socket
    with socket.create_connection(("127.0.0.1", port), timeout=15) as s:
        s.sendall(json.dumps({"version": "postfiat-local-rpc-v1", "id": "ops-eth-deposit-route", "method": "vault_bridge_route",
                              "params": {"asset_id": "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"}}).encode() + b"\n")
        resp = json.loads(s.makefile("rb").readline())
    if not resp.get("ok"):
        sys.exit(f"refusing: live route read failed: {json.dumps(resp)[:300]}")
    return resp["result"]


def send(w3, acct, tx, label, out):
    tx.update({"from": acct.address, "nonce": w3.eth.get_transaction_count(acct.address, "pending"), "chainId": 1})
    tx["gas"] = int(w3.eth.estimate_gas(tx) * 1.3)
    base = w3.eth.get_block("latest")["baseFeePerGas"]
    tx["maxPriorityFeePerGas"] = w3.to_wei(0.05, "gwei")
    tx["maxFeePerGas"] = base * 3 + tx["maxPriorityFeePerGas"]
    signed = acct.sign_transaction(tx)
    h = w3.eth.send_raw_transaction(signed.raw_transaction)
    print(f"{label}: sent 0x{h.hex().removeprefix('0x')}", flush=True)
    r = w3.eth.wait_for_transaction_receipt(h, timeout=600)
    rec = {"tx_hash": "0x" + h.hex().removeprefix("0x"), "status": r["status"], "block": r["blockNumber"], "gas_used": r["gasUsed"]}
    (out / f"{label}.json").write_text(json.dumps(rec, indent=1))
    print(f"{label}: status {r['status']} block {r['blockNumber']}", flush=True)
    if r["status"] != 1:
        sys.exit(f"{label} reverted")
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--amount-atoms", type=int, required=True)
    ap.add_argument("--recipient", required=True)
    ap.add_argument("--route-binding", required=True)
    ap.add_argument("--readiness-json", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--credited-asset", default="")
    a = ap.parse_args()
    out = Path(a.out); out.mkdir(parents=True, exist_ok=True)
    rd = json.loads(Path(a.readiness_json).read_text())
    rb = a.route_binding.lower().removeprefix("0x")
    if rd.get("ready") is not True:
        sys.exit(f"refusing: bridge readiness is not ready: {json.dumps(rd)[:400]}")
    # Readiness does not carry the binding itself, so cross-check against the live accepted
    # vault_bridge_route read-back: binding == ours, and readiness profile/epoch == the live route.
    live = live_route()
    if str(live.get("route_binding", "")).lower().removeprefix("0x") != rb:
        sys.exit(f"refusing: live route binding {live.get('route_binding')} != {rb}")
    if rd.get("route_profile_hash") != live.get("profile_hash") or rd.get("route_epoch") != live["profile"].get("route_epoch") \
            or rd.get("beacon_anchor_route_epoch") != live["profile"].get("route_epoch"):
        sys.exit("refusing: readiness profile/epoch does not match the live route read-back")
    if a.credited_asset and rd.get("credited_asset_id") != a.credited_asset:
        sys.exit(f"refusing: readiness credited_asset_id {rd.get('credited_asset_id')} != expected {a.credited_asset}")
    (out / "live-route-check.json").write_text(json.dumps({"route_binding": live.get("route_binding"), "profile_hash": live.get("profile_hash"),
                                                           "route_epoch": live["profile"].get("route_epoch"), "readiness_credited_asset_id": rd.get("credited_asset_id")}, indent=1))
    w3 = Web3(Web3.HTTPProvider(RPC, request_kwargs={"timeout": 60}))
    acct = Account.from_key(KEY.read_text().strip())
    usdc = w3.eth.contract(address=USDC, abi=ERC20); vault = w3.eth.contract(address=VAULT, abi=VAULT_ABI)
    if vault.functions.paused().call():
        sys.exit("refusing: vault paused")
    bal = usdc.functions.balanceOf(acct.address).call()
    print(f"depositor {acct.address} usdc {bal} eth {w3.eth.get_balance(acct.address)}", flush=True)
    if bal < a.amount_atoms:
        sys.exit("insufficient USDC")
    if usdc.functions.allowance(acct.address, VAULT).call() < a.amount_atoms:
        send(w3, acct, usdc.functions.approve(VAULT, a.amount_atoms).build_transaction({"from": acct.address}), "approve", out)
    nonce = "0x" + secrets.token_hex(32)
    r = send(w3, acct, vault.functions.depositV2(a.amount_atoms, a.recipient, bytes.fromhex(nonce[2:]),
                                                 bytes.fromhex(rb)).build_transaction({"from": acct.address}), "deposit", out)
    dep_logs = [l for l in r["logs"] if l["address"].lower() == VAULT.lower() and l["topics"][0].hex().removeprefix("0x") == EVENT_TOPIC.removeprefix("0x")]
    if len(dep_logs) != 1:
        sys.exit("expected exactly one ERC20BridgeDepositedV2 log")
    deposit_id = "0x" + dep_logs[0]["topics"][1].hex().removeprefix("0x")
    rec = {"depositor": acct.address.lower(), "pftl_recipient": a.recipient, "amount_atoms": str(a.amount_atoms),
           "nonce": nonce, "route_binding": "0x" + rb, "deposit_tx_hash": "0x" + r["transactionHash"].hex().removeprefix("0x"),
           "deposit_id": deposit_id, "block": r["blockNumber"]}
    (out / "deposit-record.json").write_text(json.dumps(rec, indent=1))
    print(json.dumps(rec), flush=True)


if __name__ == "__main__":
    main()
