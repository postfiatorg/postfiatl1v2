"""Send one small real USDC deposit from the setup payer to the b23-bob test wallet through the
live Solana vault, using the reviewed instruction builder. The relay must prove and credit it."""
import json, runpy, secrets, sys, time
from pathlib import Path
from types import SimpleNamespace
g = runpy.run_path(str(Path.home() / "repos/postfiatl1v2-live/ops/sol-live/mainnet-go.py"), run_name="deposit_test")
Cluster, DIRECTORY, URL, BASE, GENESIS, PAYER, MINTS = (g[k] for k in ("Cluster", "DIRECTORY", "URL", "BASE", "GENESIS", "PAYER", "MINTS"))
load, pk, pda, deposit_ix, read, six, balance, series_id, require = (g[k] for k in ("load", "pk", "pda", "deposit_ix", "read", "six", "balance", "series_id", "require"))
atoms = int(sys.argv[1]); recipient = "pf5c266a1bdb18320e140961411a8c4d8383eaafb2"
c = Cluster(SimpleNamespace(rpc=URL, payer=DIRECTORY / "mainnet-payer.json", log=BASE / "transactions.jsonl"))
require(c.genesis == GENESIS and str(c.owner) == PAYER, "unexpected mainnet genesis or payer")
activation = read(BASE / "activation.json")
epoch = c.call("getEpochInfo", {"commitment": "finalized"})["epoch"]
require(epoch == activation["profile"]["solana"]["anchor"]["epoch"], f"route is anchored to another epoch than {epoch}")
vault = load(DIRECTORY / "vault.json").pubkey(); mint = pk(MINTS[GENESIS])
rows = c.call("getTokenAccountsByOwner", PAYER, {"mint": str(mint)}, {"encoding": "jsonParsed", **c.level})["value"]
source = next(r for r in rows if int(r["account"]["data"]["parsed"]["info"]["tokenAmount"]["amount"]) >= atoms)
asset = series_id(activation); before = six(lambda i: balance(recipient, asset, i)); require(len(set(before)) == 1, "opening balances disagree")
nonce = secrets.token_bytes(32)
out = Path.home() / "var/sol-exit/live/test-deposit.json"
rec = dict(nonce=nonce.hex(), source=source["pubkey"], amount_atoms=atoms, recipient=recipient, asset_id=asset, opening_balance=before[0], sent_at=time.time())
out.write_text(json.dumps(rec, indent=1))
ix = deposit_ix(c.owner, vault, pk(source["pubkey"]), mint, atoms, recipient, nonce)
rec["signature"] = c.send("mainnet.deposit", [ix]); out.write_text(json.dumps(rec, indent=1))
print("sent", rec["signature"], "asset", asset[:16], "opening", before[0], flush=True)
for _ in range(120):
    now = six(lambda i: balance(recipient, asset, i))
    if len(set(now)) == 1 and now[0] == before[0] + atoms:
        rec["credited_after_s"] = round(time.time() - rec["sent_at"]); out.write_text(json.dumps(rec, indent=1))
        print("credited on six validators after", rec["credited_after_s"], "s; balance", now[0], flush=True); break
    time.sleep(10)
else: print("NOT CREDITED within 20 minutes", flush=True); sys.exit(1)
