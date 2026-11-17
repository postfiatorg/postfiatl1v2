#!/usr/bin/env python3
"""ONE issued_payment from Bob (StakeHub wallet b23-bob) to DEST, via the six-validator finality path.
Usage: bob_issued_payment.py LABEL ASSET_ID DEST ATOMS   (chain.lock must contain MINT; release-4 signer)"""
import json, sys
from pathlib import Path
sys.path.insert(0, "/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit")
sys.path.insert(0, "/home/postfiat/repos/StakeHub")
import pftl
BOB = "pf5c266a1bdb18320e140961411a8c4d8383eaafb2"
label, asset, dest, atoms = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
pftl.NODE = Path("/home/postfiat/repos/b23-coord/artifacts/release4/postfiat-node")
pftl.EVIDENCE = Path("/home/postfiat/repos/b23-coord/evidence/browser-journey/chain") / label
issuer = pftl.rpc("asset_info", {"asset_id": asset}, "asset")["asset"]["issuer"]
from pft_wallet.wallets import signing_files
pw = Path("/home/postfiat/.pft/b23-bob.passphrase").read_text().rstrip("\r\n")
with signing_files("b23-bob", pw) as (public, key_file, _b):
    del pw
    assert public.ce22_address == BOB
    op = dict(operation="issued_payment", issuer=issuer, asset_id=asset, amount=atoms, **{"from": BOB, "to": dest})
    r = pftl.asset_op("pay", BOB, Path(key_file), op)
res = r.get("result", r)
print(json.dumps({"tx_id": res.get("tx_id"), "keys": sorted(res.keys())[:12]}))
tx = res.get("tx_id")
print(json.dumps(pftl.rpc("receipts", {"tx_id": tx}, "rcpt"))[:600])
