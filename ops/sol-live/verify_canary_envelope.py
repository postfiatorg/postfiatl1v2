"""Verify one public, signed L1 burn offline and print its canonical transaction ID."""

import hashlib
import json
import sys

from cryptography.hazmat.primitives.asymmetric.mldsa import MLDSA65PublicKey


def verified_id(signed):
    u = signed["unsigned"]
    if not (
        signed["algorithm_id"] == u["signature_algorithm_id"] == "ML-DSA-65"
        and u["operation"] == u["transaction_kind"] == "vault_bridge_burn_to_redeem"
        and u["address_namespace"] == "postfiat.address.v1"
    ):
        raise ValueError("Unexpected envelope domain.")
    # Match UnsignedAssetTransaction and VaultBridgeBurnToRedeemOperation::signing_bytes.
    fields = (
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "address_namespace",
        "transaction_kind",
        "signature_algorithm_id",
        "source",
        "fee",
        "sequence",
        "operation",
        "owner",
        "issuer",
        "asset_id",
        "bucket_id",
        "amount_atoms",
        "epoch",
        "reserve_packet_hash",
    )
    destination = u["destination_ref"]
    body = "postfiat.asset_transaction.v1\n" + "".join(f"{k}={u[k]}\n" for k in fields)
    body += f"destination_ref_bytes={len(destination.encode())}\ndestination_ref={destination}\n"
    public = bytes.fromhex(signed["public_key_hex"])
    owner = "pf" + hashlib.sha3_384(b"postfiat.address.v1\0" + public).hexdigest()[:40]
    if owner != u["source"] or owner != u["owner"]:
        raise ValueError("Unexpected burn signer.")
    MLDSA65PublicKey.from_public_bytes(public).verify(
        bytes.fromhex(signed["signature_hex"]), body.encode(), b"postfiat-l1-v2/tx/v1"
    )
    body += f"algorithm=ML-DSA-65\npublic_key={signed['public_key_hex']}\nsignature={signed['signature_hex']}\n"
    return hashlib.sha3_384(
        b"postfiat.asset_transaction.tx_id.v1\0" + body.encode()
    ).hexdigest()


if __name__ == "__main__":
    print(verified_id(json.load(sys.stdin)))
