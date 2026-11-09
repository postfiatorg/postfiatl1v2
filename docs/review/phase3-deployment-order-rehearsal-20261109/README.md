# Phase 3 deployment-order rehearsal, local Anvil, 2026-11-09

This is a record of one run of `scripts/rehearse-phase3-deployment-order.py`
(see `scripts/rehearse-phase3-deployment-order.md`). It rehearses the order of
the export design note, section 5. The run used a loopback Anvil with chain id
84532 and disposable keys. No public network was touched.

- **Source:** commit `9eb3f6f0a81b01079cf94be0018f25c16ec9ba1c`, tree
  `db0080ef3e96ebde32b42a571411ba6ef036a517`.
- **Tools:** Foundry 1.8.1 and solc 0.8.24.
- **Files:**
  - `summary.json` is the script's summary.
  - `manifest.json` holds the addresses, code hashes, nonces, tool versions and
    the source and artifact hashes. It also lists the sha256 of the files that
    are not published.
- **Exit code:** 0.

| | Predicted | Deployed |
| --- | --- | --- |
| Verifier (`PFTLFinalityVerifierV1`), deployer nonce 0 | `0x6982eceff5027bf77c9a1dc70e6f9964694fd356` | same |
| Verifier runtime code hash | `0xa43b25781211daef2a875fe204f81feadbb91b0cd81b8457a22365e036a44433` | same |
| Lock (`PFTLockV1`), deployer nonce 1 | `0x34c77598d5e6932ce1ac1a958bef7f8558de9c13` | same |
| Lock runtime code hash | `0xf042074c2d734f5a5ec0c1676865fdb8b9018eea75c342b409ad3a51d69286c2` | same |

**How the code hashes were predicted.** Both contracts were deployed at the
predicted nonces on an `evm_snapshot`, their `eth_getCode` was hashed, and the
snapshot was reverted. The lock was hashed first, with a probe verifier at the
predicted address. The verifier was then hashed with that lock code hash.

**Genesis.** The lane genesis pins the predicted lock address and code hash.
`postfiat-node init --genesis-file` validated it, and the pins read back from
the node's data directory equal the deployed lock.

**On chain.** These checks held:

- `lock.finalityVerifier()` is the verifier;
- the verifier's `vaultRuntimeCodeHash` is the lock's code hash;
- its `token` and `tokenRuntimeCodeHash` are the token's;
- the seeded height and checkpoint and the chain id are as given.

**Nonce off by one.** After a stray transaction the deployer was at nonce 1
instead of 0. The verifier would have landed at the predicted lock address and
the lock at `0xb07ff5f6c12caeae9f38ad4952a28a1d4eb86936`. The guard refused
before any deployment.

**Limits.** The verifier's checkpoint, genesis-hash commitment, route profile
and asset id are fixtures from `crates/ethereum-contracts/test/PFUSDCTier4.t.sol`.
In production the first finalized PFTL checkpoint and the Base route profile
provide them. The SP1 verifier is a fixture address, so no proof or `release`
ran, and no PFTL chain was run beyond `init`. The rehearsal uses one deployer
for both contracts; section 5 recommends two.
