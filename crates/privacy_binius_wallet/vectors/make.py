"""Regenerate the public HKDF-SHA-384 vector with Python's standard library."""
import hashlib
import hmac
import json
from pathlib import Path

seed = bytes(range(48))
chain = "postfiat-wallet-vector"
genesis = bytes(reversed(seed))
account, index = 0x01020304, 0x10203040


def field(value):
    return len(value).to_bytes(4, "big") + value


def mac(key, data):
    return hmac.digest(key, data, "sha384")


def expand(role, size):
    label = f"postfiat/pq-wallet/v1/{role}".encode()
    info = b"".join(map(field, (label, chain.encode(), genesis,
                                 account.to_bytes(4, "big"), index.to_bytes(4, "big"))))
    prk = mac(b"postfiat/pq-wallet/v1/hkdf-salt", seed)
    first = mac(prk, info + b"\x01")
    return (first + mac(prk, first + info + b"\x02"))[:size]


spend = expand("spend", 48)
vector = dict(chain_id=chain, genesis_hash=genesis.hex(), seed=seed.hex(), account=account,
              index=index, spend=spend.hex(), kem=expand("kem", 64).hex(),
              owner=hashlib.shake_256(b"\0" + spend).hexdigest(48))
Path(__file__).with_name("derivation.json").write_text(json.dumps(vector, indent=2) + "\n")
