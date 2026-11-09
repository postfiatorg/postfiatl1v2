# Phase 3 deployment-order rehearsal, local Anvil, 2026-11-09

This is a record of one run of `scripts/rehearse-phase3-deployment-order.py`
(see `scripts/rehearse-phase3-deployment-order.md`). It rehearses the order of
the export design note, section 5. The run used a loopback Anvil with chain id
84532 and disposable keys. No public network was touched.

- **Source:** commit `5c4cb62e82ea06ac37f2badc9899980bb69f6121`, tree
  `a1198ed8f037b212a3d3fcf12d46c90196032bb3`.
- **Tools:** Foundry 1.8.1, solc 0.8.24, cargo and rustc 1.95.0, Python
  3.14.4; the node is a debug build.
- **Files:**
  - `summary.json` is the script's summary.
  - `manifest.json` holds the addresses, code hashes, nonces, tool versions, the
    host OS and the source and artifact hashes. `not_published` lists all 316
    other files of the output directory, recursively, by relative path, sha256
    and size.
  - `postfiat_node_sha256` identifies the exact node binary used. It is not
    expected to reproduce across builds; the source, configuration and artifact
    hashes are.
- **Exit code:** 0.

| | Predicted | Deployed |
| --- | --- | --- |
| Verifier (`PFTLFinalityVerifierV1`), deployer nonce 0 | `0x37d46f261c6fbd84c0aa5408d02aa75f8e09cf92` | same |
| Verifier runtime code hash | `0xeb74f0292bcf8f415a99bacf76b6f4a651326b33616ccbbec279864d9021b3cb` | same |
| Lock (`PFTLockV1`), deployer nonce 1 | `0x97ad1ce8fef4663874a93239e8d5fc11e5f1b48a` | same |
| Lock runtime code hash | `0x5d0aed7a516ad2c8fe42c542aa03b76bfddfa7a194677d4d4ab98f234461d6d3` | same |

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

**Nonce guard.** Immediately before each deployment the deployer's `latest`
and `pending` nonces were both the planned one (0 for the verifier, 1 for the
lock), and each `forge create` pinned that nonce with `--nonce`.

**Nonce off by one.** After a stray transaction the deployer was at nonce 1
(latest and pending) instead of 0. The verifier would have landed at the predicted lock address and
the lock at `0x5e94a54deb0111f58a498b2e1174c894036eb804`. The guard refused
before any deployment.

**Limits.** The verifier's checkpoint, genesis-hash commitment, route profile
and asset id are fixtures from `crates/ethereum-contracts/test/PFUSDCTier4.t.sol`.
In production the first finalized PFTL checkpoint and the Base route profile
provide them. The SP1 verifier is a fixture address, so no proof or `release`
ran, and no PFTL chain was run beyond `init`. The rehearsal uses one deployer
for both contracts; section 5 recommends two, and a production run must use
those two dedicated keys and nothing else may send from them.
