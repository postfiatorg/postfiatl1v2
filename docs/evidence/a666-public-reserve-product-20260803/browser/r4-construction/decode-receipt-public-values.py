#!/usr/bin/env python3
"""Decode the 1120-byte ReceiptPublicValues artifact field by field.

Field order is exactly ReceiptPublicValues in
crates/ethereum-contracts/src/PFTLReceiptFinalityVerifierV1.sol:32-68
(35 fields x 32-byte ABI words = 1120 bytes). Read-only; never modifies bytes.
"""
import json
import sys

FIELDS = [
    ("proofProgramVersion", "uint32"),
    ("pftlChainIdHash", "bytes32"),
    ("pftlGenesisHashCommitment", "bytes32"),
    ("pftlProtocolVersion", "uint32"),
    ("committeeRootCommitment", "bytes32"),
    ("committeeTransitionCommitment", "bytes32"),
    ("finalizedBlockCommitment", "bytes32"),
    ("finalizedStateRootCommitment", "bytes32"),
    ("routeEpoch", "uint64"),
    ("policyHashCommitment", "bytes32"),
    ("routeIdCommitment", "bytes32"),
    ("routeTrustClass", "bytes32"),
    ("routeConfigDigestCommitment", "bytes32"),
    ("nativeNavAssetIdCommitment", "bytes32"),
    ("settlementAssetIdCommitment", "bytes32"),
    ("pricingNavEpoch", "uint64"),
    ("pricingReservePacketHashCommitment", "bytes32"),
    ("sourceWalletCommitment", "bytes32"),
    ("sourceReceiptRootCommitment", "bytes32"),
    ("sourceReceiptHashCommitment", "bytes32"),
    ("acceptedReceiptCode", "bytes32"),
    ("packetDigest", "bytes32"),
    ("destinationChainId", "uint256"),
    ("controller", "address"),
    ("wrappedToken", "address"),
    ("recipient", "address"),
    ("mintAmountAtoms", "uint256"),
    ("settlementValueAtoms", "uint256"),
    ("packetNonce", "bytes32"),
    ("deadline", "uint64"),
    ("sourceHeight", "uint64"),
    ("priorCheckpointCommitment", "bytes32"),
    ("resultingCheckpointCommitment", "bytes32"),
    ("finalizedHeight", "uint64"),
    ("proofNullifier", "bytes32"),
]

def decode(path):
    data = open(path, "rb").read()
    assert len(data) == 1120, f"expected 1120 bytes, got {len(data)}"
    out = {}
    for i, (name, kind) in enumerate(FIELDS):
        word = data[i * 32:(i + 1) * 32]
        if kind == "bytes32":
            out[name] = {"type": kind, "value": "0x" + word.hex()}
        elif kind == "address":
            out[name] = {"type": kind, "value": "0x" + word[12:].hex()}
        else:
            out[name] = {"type": kind, "value": int.from_bytes(word, "big")}
    return out

if __name__ == "__main__":
    print(json.dumps(decode(sys.argv[1]), indent=1))
