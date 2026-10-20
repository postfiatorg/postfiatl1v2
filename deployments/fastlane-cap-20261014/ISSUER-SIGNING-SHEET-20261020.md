# Issuer signing sheet, 2026-10-20

Two issuer-signed bridge policy updates move the A666 and BMNRC routes to the
epoch-2 checkpoint committee. Key material never leaves the founder's host:
only the quote goes to it and only the signed transaction comes back.

## 1. Window and files

- **Window:** 2026-10-20 12:00–16:00 UTC. Order: **A666 first, then BMNRC**.
- **Release:** `fastlane-cap-20261014` (executable `b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405`).
- **Files** (branch `release/fastlane-cap-20261014`, `deployments/fastlane-cap-20261014/observed/`):

| Route | File | File sha256 | Operation sha256 |
|---|---|---|---|
| `pftl-a666-ethereum-wA666-usdc-v1`, issuer `pffcb93d9f87a843a8aa34e1adf241f5d58143e81b` | `a666-bridge-policy-update.unsigned.json` | `24aa2e28ff942d0be9557416d2b796d6b9a8131abf7704c6ad1348539f51c03f` | `f6b1fd225186efec87ff3e232432582d3b4f6c9ca698642bc1be71be2b86df72` |
| `pftl-bmnrc-ethereum-wBMNRC-usdc-v1`, issuer `pf9bd7958a0779f4de39e871878bbd6552dc461c3f` | `bmnrc-bridge-policy-update.unsigned.json` | `5179d6f7b1a67971d6e591597e3ee63a0775b9af001e1b5bc27ae64969603650` | `10887bf36eefbcc1ef1994dbd8f11d298f1a5c3e15964e32a2efa3750043b665` |

- Both set `authority_epoch` 2 and `committee_root`
  `72836827f9f3ecd3042261a77abff72235f0238b6a81f673630a62199415021a1be0cbd5451878edca0e110434c1da37`.
  They echo each route's current 12 confirmations and two code hashes (A666 `4c62b7d8…`,
  BMNRC `837e8554…`; both `671ee905…`), read from the verified height-1123 backup.
- The operation sha256 is over `.operations[0].operation`, compact with sorted keys. It does
  not change when you edit `key_file`. The `quote` command below prints it. The A666
  operation also equals `"7.3".operation` in
  [bridge-activation-record.json](observed/bridge-activation/bridge-activation-record.json).
- `key_file` is a placeholder, and this path does not read it: you sign with `--key-file`.

## 2. Where each step runs

- **Sign:** on the founder's host, with a `postfiat-node` of release `8bed9b90`
  (`/opt/postfiat/releases/fastlane-cap-20261014/postfiat-node` copied from any validator
  host; sha256 above).
- **Quote, submit, read:** as root on the validator host that proposes the next height. The
  RPC submit path accepts only the local proposer. Each host's RPC listens on
  `127.0.0.1:2765N` (validator-N). Its `rpc-serve` unit already carries
  `--allow-mempool-submit-finality`, `--finality-topology /etc/postfiat/releases/fastlane-cap-20261014/topology.json`
  and the validator's key, so you pass no topology or key file.

| Update | Height | Proposer (view 0) | RPC on that host |
|---|---|---|---|
| A666 | 1132 | validator-4 | `127.0.0.1:27654` |
| BMNRC | 1133 | validator-5 | `127.0.0.1:27655` |

- **Method:** `mempool_submit_signed_asset_transaction_finality`, the same RPC finality path
  as the 2026-10-15 `mempool_submit_fastlane_primary_finality` submission (from a file,
  over the socket, on the next proposer). The update is an ordinary issuer-signed asset
  transaction, not a FastLane object, so `--fastlane-primary-file` does not apply. Do not use
  `pftl-submit-certified-asset-ops`: it signs and proposes in one process, so the issuer key
  would have to be on the validator host.

## 3. Helpers (create once on each submit host, as root)

```bash
mkdir -p /root/issuer-20261020 && cd /root/issuer-20261020
cat > issuer-rpc.py <<'EOF'
import hashlib, json, socket, sys
ROUTES = ("pftl-a666-ethereum-wA666-usdc-v1", "pftl-bmnrc-ethereum-wBMNRC-usdc-v1")
def rpc(port, method, params, rid):
    req = {"version": "postfiat-local-rpc-v1", "id": rid, "method": method, "params": params}
    with socket.create_connection(("127.0.0.1", port), timeout=600) as sock:
        f = sock.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush()
        return json.loads(f.readline(256 * 1024 * 1024))
cmd, port = sys.argv[1], int(sys.argv[2])
if cmd == "check":
    s = rpc(port, "status", {}, "issuer-check-status")["result"]
    print("height", s["block_height"], "tip", s["block_tip_hash"], "root", s["state_root"], "mempool", s["mempool_pending"])
    for r in rpc(port, "navcoin_bridge_routes", {}, "issuer-check-routes")["result"]["routes"]:
        if r["route_id"] in ROUTES:
            print(r["route_id"], "claims", r["outstanding_bridge_claims_atoms"], "returns", r["pending_return_import_claims_atoms"],
                  "open_packets", r["outstanding_export_packet_count"], "pending_burns", r["pending_return_burn_count"],
                  "paused", r["paused"], "hash", r["ledger_hash"])
elif cmd == "quote":
    op = json.load(open(sys.argv[3]))["operations"][0]
    print("operation sha256", hashlib.sha256(json.dumps(op["operation"], sort_keys=True, separators=(",", ":")).encode()).hexdigest(), file=sys.stderr)
    print(json.dumps(rpc(port, "asset_fee_quote", {"source": op["source"], "operation_json": json.dumps(op["operation"], separators=(",", ":"))}, "issuer-quote"), indent=1))
elif cmd == "submit":
    print(json.dumps(rpc(port, "mempool_submit_signed_asset_transaction_finality", {"signed_asset_transaction_json": open(sys.argv[3]).read()}, sys.argv[4]), indent=1))
elif cmd == "tx":
    r = rpc(port, "tx", {"tx_id": sys.argv[3]}, "issuer-tx")["result"]
    print("confirmed", r["confirmed"], "receipt", json.dumps(r["receipt"]),
          "block", r["block"]["header"]["height"], r["block"]["header"]["block_hash"])
EOF
cat > ledger-read.py <<'EOF'
import json, sys
d, _ = json.JSONDecoder().raw_decode(open(sys.argv[1]).read())
hx = lambda v: bytes(v).hex() if isinstance(v, list) else v
c = d["fastswap_committees"][-1]["domain"]
print("newest committee", c["committee_epoch"], hx(c["committee_root"]))
print("fenced epochs", sorted({f["committee_epoch"] for f in d["fast_lane_prepare_fences"]}))
for r in d["pftl_uniswap_routes"]:
    p = r.get("ethereum_verification_policy")
    if p and r["route_id"] in ("pftl-a666-ethereum-wA666-usdc-v1", "pftl-bmnrc-ethereum-wBMNRC-usdc-v1"):
        last = [x for x in d["pftl_uniswap_receipts"] if x["route_id"] == r["route_id"]][-1]
        print(r["route_id"], "epoch", p["authority_epoch"], "root", hx(p["committee_root"]),
              "last", last["transition"], last["block_height"], last["state_after_hash"])
EOF
```

Both are read-only except `submit`. `ledger-read.py` is for the verified backup imports on the work server only, not for the hosts.

## 4. Preconditions (we check just before 12:00 on all six; you check again on the submit host)

```bash
cd /root/issuer-20261020
N=4; H=1132   # BMNRC: N=5; H=1133
python3 issuer-rpc.py check 2765$N
python3 issuer-rpc.py tx 2765$N 2b2efa37e12f5d14c4b1f7da62a4fc7f9ea8a2d7d2d21ad20427d56646d99c158693072e9f7c022cf5f49deb8ee34826   # the epoch-2 activation: confirmed True, code fastlane_control_applied, block 1131
runuser -u postfiat -- /opt/postfiat/releases/fastlane-cap-20261014/postfiat-node block-proposer \
  --unsafe-devnet-json-storage --data-dir /var/lib/postfiat/validator-$N --height $H --view 0
```

Our read at 2026-10-20T10:13:12Z through all six RPCs: all six at height 1131, tip `9b054f09…`,
root `a12e7c5f…`, `mempool 0`. On both routes: claims 0, returns 0, open packets 0, pending
burns 0, not paused. The A666 hash is `57da3837…` and the BMNRC hash `b2817e68…`, unchanged
since their last receipts at 1111 and 1115 (verified 1123 backup). We read again before 12:00.

Submit only if every line holds:

1. On each route, `claims 0` and `returns 0`. Consensus refuses the update otherwise
   (`pftl_uniswap_bridge_policy_handoff_active`).
2. Committee epoch 2 is the newest and has no fence: the activation transaction above reports
   `confirmed True` with receipt code `fastlane_control_applied` in block 1131, and the chain tip is
   still that block before A666 (`height 1131`). Do NOT read `/var/lib/postfiat/validator-N/ledger.json`
   on the hosts for this: it is a legacy file that still shows committee 1
   (`pftl_uniswap_bridge_policy_committee_not_current` is the refusal if epoch 2 were not current).
3. There is no open handoff on the route: `open_packets 0`, `pending_burns 0`, `paused False`.
4. The chain has not moved. Before A666: `height 1131`, tip `9b054f09…`, root `a12e7c5f…`,
   `mempool 0`, A666 `hash 57da3837…`, BMNRC `hash b2817e68…`, both routes `epoch 1` root
   `a2eebcba…`. Before BMNRC: `height 1132`, and the tip is the block that `issuer-rpc.py tx`
   reports for the A666 update. BMNRC is still on `epoch 1`.
5. `block-proposer` reports `"local_is_proposer": true`.

**If any precondition fails, do not submit.** Write what you saw in a handoff and stop. We
re-check and name a new window.

## 5. Submission (A666, then BMNRC)

For A666, use `a666`, validator-4, port 27654 and height 1132. For BMNRC, use `bmnrc`,
validator-5, port 27655 and height 1133.

1. **Submit host:** fetch the file and check it.
   ```bash
   cd /root/issuer-20261020
   curl -fsSLO https://raw.githubusercontent.com/postfiatorg/postfiatl1v2/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/observed/a666-bridge-policy-update.unsigned.json
   sha256sum a666-bridge-policy-update.unsigned.json      # must equal the table in §1
   python3 issuer-rpc.py quote 27654 a666-bridge-policy-update.unsigned.json > a666.quote.json
   ```
   The `operation sha256` line must equal §1. The quote must have `"ok": true`, `source` set to
   the issuer, `sender_meets_reserve_after_fee: true` and the same operation.
2. **Copy `a666.quote.json` to the founder's host.** It contains no secret.
3. **Founder's host:** sign.
   ```bash
   postfiat-node wallet-sign-asset-transaction --key-file <A666 issuer key file> \
     --quote-file a666.quote.json > a666.signed.json
   ```
   The command refuses a key whose address is not the quote's `source`. For BMNRC, unseal the
   key on this host only and reseal it after signing.
4. **Copy `a666.signed.json` back to the submit host.** It contains a signature, not a key.
5. **Submit host:** run §4 again, then submit once.
   ```bash
   python3 issuer-rpc.py submit 27654 a666.signed.json issuer-a666-bridge-policy-update-20261020 | tee a666.submit.json
   ```
   Expected: `"ok": true`, `result.finality.confirmed: true` and `receipt.accepted: true` with
   code `accepted`. The route records the transition `route_bridge_policy_updated`. If the
   reply is an error, do not resend. Run `issuer-rpc.py tx 27654 <tx_id>` and write it in the handoff.

## 6. After-reads

After A666, on validator-4:

```bash
python3 issuer-rpc.py tx 27654 <tx_id>   # confirmed True, accepted
python3 issuer-rpc.py check 27654        # height 1132, mempool 0, A666 hash changed, claims 0
```

The `tx` read must show `confirmed True` with receipt code `accepted`; the `check` read must show
`height 1132`, `mempool 0` and a changed A666 `hash` (it was `57da3837…`), with the BMNRC hash
unchanged (`b2817e68…`). The authority epoch 2 of the route is confirmed from the work server after the
window with the verified ledger import; the hosts' `ledger.json` is a legacy file and is not read.

After BMNRC, on validator-5, the same reads must show `height 1133` (1131 + 2) and a changed BMNRC
`hash`.
We then confirm six-way agreement (height, tip, root and route hashes) through all six RPCs.

Then the next Ethereum checkpoint is signed by the epoch-2 committee, 5 of 6
([DEPLOY-SHEET §7](DEPLOY-SHEET.md#7-signer-group-activation-bridge-half)). That step is ours,
on a later go.
