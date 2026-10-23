"""Read-only admission probe: two mempool_submit_signed_transfer_finality requests that can never be admitted
(wrong chain domain, zeroed signature, and a `transfer_file` parameter that the finality handler refuses before
any lock or round). The destination check runs first: a malformed recipient is refused with its address code;
a canonical recipient passes it and is refused by the protocol check instead."""
import json, socket, sys
port = int(sys.argv[1])
def tx(to):
    return {"unsigned": {"chain_id": "postfiat-admission-probe", "genesis_hash": "00" * 48, "protocol_version": 1,
            "address_namespace": "postfiat.address.v1", "transaction_kind": "transparent_transfer", "signature_algorithm_id": "ML-DSA-65",
            "from": "pf" + "0" * 40, "to": to, "amount": 1, "fee": 32, "sequence": 1},
            "algorithm_id": "ML-DSA-65", "public_key_hex": "00", "signature_hex": "00"}
def call(to):
    req = {"version": "postfiat-local-rpc-v1", "id": "admission-probe", "method": "mempool_submit_signed_transfer_finality",
           "params": {"signed_transfer_json": json.dumps(tx(to), separators=(",", ":")), "transfer_file": "admission-probe-not-a-file"}}
    with socket.create_connection(("127.0.0.1", port), timeout=60) as s:
        f = s.makefile("rwb"); f.write((json.dumps(req) + "\n").encode()); f.flush()
        r = json.loads(f.readline(8 << 20))
    e = r.get("error") or {}
    return {"to": to, "ok": r.get("ok"), "error_code": e.get("code"), "error_message": (e.get("message") or "")[:200]}
out = [call("pf6395ef7f43ffaa74ce178d320b9bbbfd45532b3"), call("pf6395ef7f43ffaa74ce178d320b9bbbfd45532b38")]
print(json.dumps(out, indent=1))
