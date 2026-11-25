"""Verify one finalized Solana payout against its L1 burn and attest it as paid (six signers).

General form of the canary settlement in exit_canary.py: any holder, any amount, and either the
service fee payer or the setup payer as transaction fee payer. Every check the canary makes on the
payout itself is kept: one top-level tag-2 instruction to the pinned verifier, the burn's marker
PDA, the exact custody debit and recipient credit, and the withdrawal packet on six validators.
"""
import sys, json, subprocess, base58
from exit_payout import *
from exit_payout import LIGHT, SYSTEM, VERIFIER, ATA, MINT, PREFIX, TOKEN, marker, pda, signature
from exit_setup import FEE_PAYER, MAINNET_PAYER, VAULT
from exit_precheck import agreement, parent_of
from exit_govern import fleet, six, EXIT, routes, require_descriptor, chain_rules, SERIES
import exit_rules
from exit_payout import covered_buckets
from live import survey, agreeing
from govern import govern
from solders.pubkey import Pubkey
REV, burn = sys.argv[1], sys.argv[2]
DRY = len(sys.argv) > 3 and sys.argv[3] == 'dry'
rules = chain_rules(); need = exit_rules.signatures(rules, 'exit-paid')
hosts = "64.176.220.75 95.179.184.122 66.42.48.39 149.28.63.106 95.179.179.206 45.32.110.170".split()
c = cluster()
def row_of(status):
    (r,) = [r for r in status["redemptions"] if r["burn_tx_id"] == burn]
    return r
def view(i):
    r = row_of(rpc(i, "vault_bridge_status", asset_id=ASSET))
    return {k: r.get(k) for k in ("bucket_id", "amount_atoms", "destination_ref", "withdrawal_packet_hash", "owner", "created_at_height", "settlement_receipt_hash")}
_rows = survey(view, need); red = _rows[agreeing(_rows, lambda r: json.dumps(r, sort_keys=True), need, "Validator records differ.")[0]]
covered = covered_buckets(rpc(0, "vault_bridge_status", asset_id=ASSET)["buckets"])
require(red["bucket_id"] in covered and (DRY or not red["settlement_receipt_hash"]), "Not an unsettled covered Solana redemption.")
require(red["destination_ref"].startswith(PREFIX), "Unexpected destination.")
recipient = Pubkey.from_bytes(bytes.fromhex(red["destination_ref"][len(PREFIX):]))
amount, packet = int(red["amount_atoms"]), red["withdrawal_packet_hash"]
sig = signature(c, burn, None); require(sig, "No finalized paid marker for this burn.")
tx = c.call("getTransaction", sig, dict(c.level, encoding="json", maxSupportedTransactionVersion=0))
require(tx and tx["transaction"]["signatures"] == [sig] and tx["meta"]["err"] is None and tx["slot"] > 0, "Payout not finalized.")
raw = base58.b58decode(sig); require(len(raw) == 64 and any(raw), "Invalid signature.")
msg = tx["transaction"]["message"]; keys = msg["accountKeys"]
require(not msg.get("addressTableLookups"), "Unexpected lookup-table payout.")
tag2 = lambda ix: keys[ix["programIdIndex"]] == str(VERIFIER) and base58.b58decode(ix["data"])[:1] == b"\x02"
inner = tx["meta"]["innerInstructions"]
require(isinstance(inner, list) and not any(tag2(ix) for g in inner for ix in g["instructions"]), "Cannot establish a single top-level payout.")
(ix,) = [ix for ix in msg["instructions"] if tag2(ix)]
used = [keys[i] for i in ix["accounts"]]
ata = pda(ATA, bytes(recipient), bytes(TOKEN), bytes(MINT))
tail = [SYSTEM, marker(burn), VAULT, pda(VERIFIER, b"egress-release", bytes(VAULT)), pda(VAULT, b"config"), pda(VAULT, b"custody"), ata, recipient, MINT, TOKEN]
require(len(base58.b58decode(ix["data"])) == 357 and len(used) == 15 and used[0] in (str(FEE_PAYER), str(MAINNET_PAYER))
        and used[1] == str(LIGHT) and used[5:] == list(map(str, tail)), "Payout instruction names different accounts or route.")
deltas = {}
for name, address in (("custody", pda(VAULT, b"custody")), ("recipient", ata)):
    index = keys.index(str(address)); amounts = []
    for phase in ("preTokenBalances", "postTokenBalances"):
        found = [r for r in tx["meta"][phase] if r["accountIndex"] == index]
        if not found and name == "recipient" and phase == "preTokenBalances": amounts.append(0); continue
        require(len(found) == 1 and found[0]["mint"] == str(MINT) and found[0]["uiTokenAmount"]["decimals"] == 6, "Missing USDC balance.")
        amounts.append(int(found[0]["uiTokenAmount"]["amount"]))
    deltas[name] = amounts[1] - amounts[0]
require(deltas == {"custody": -amount, "recipient": amount}, f"Token deltas differ: {deltas}")
ref = dict(packet=packet, slot=tx["slot"], ordinal=0, signature_hex=raw.hex(), kind=f"solana_exit_r8_paid:{packet}:{tx['slot']}:0:{raw.hex()}")
print("verified payout", sig, "amount", amount, "recipient", recipient, "payer", used[0], flush=True)
if DRY: print("dry run ok", json.dumps(ref)); sys.exit(0)
head = fleet(need); parent = parent_of(head)
require(head["build_git_revision"] == REV, "Fleet is not on the reviewed release.")
recovery = [r for r in routes(SERIES, need) if r["current_height"] == head["block_height"]]
require_descriptor(recovery, need)
require(len(recovery) >= need and parent_of(fleet(need)) == parent, "Recovery parent changed.")
out = govern("exit-paid", compatible=REV, qualified_parent=parent, payout=ref, rules=rules)
print("settled", json.dumps({k: out[k] for k in ("tx_id", "height")}), "signers", out.get("signers"), flush=True)
_after = survey(view, need); after = [_after[i] for i in agreeing(_after, lambda r: json.dumps(r, sort_keys=True), need, "Validator records differ.")]; print("settlement_receipt_hash", after[0]["settlement_receipt_hash"], flush=True)
