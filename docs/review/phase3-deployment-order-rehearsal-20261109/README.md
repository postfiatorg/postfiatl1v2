# Phase 3 deployment-order rehearsal, local Anvil, 2026-11-10

This is a record of one run, on 2026-11-10, of
`scripts/rehearse-phase3-deployment-order.py` (see
`scripts/rehearse-phase3-deployment-order.md`). It replaces the run of
2026-11-09 after a correctness review. It rehearses the order of the export
design note, section 5. The run used a loopback Anvil with chain id 84532 and
disposable keys. No public network was touched.

- **Source:** commit `da049f00e6582f34d35f9d0cfe2dae2116f9a229`, tree
  `fb44d2923d73009b6cdce1647f62f87468c7dc2a`.
- **Tools:** Foundry 1.8.1, solc 0.8.24, OpenZeppelin Contracts at
  `cab19933c33c2ad1d4c7a84864a3601dddfd16f3`, cargo and rustc 1.95.0, Python
  3.14.4; the node is a debug build.
- **Files:**
  - `summary.json` is the script's summary.
  - `manifest.json` holds the addresses, code hashes, nonces, tool versions, the
    host OS and the source and artifact hashes. `not_published` lists all 326
    other files of the output directory, recursively, by relative path, sha256
    and size.
  - `genesis-lane-pins.json` is the generated genesis's `base-erc20` lane with
    its four pins, and the sha256 of the whole generated `genesis.json`
    (`7b01ebf5…529854cd`), which is not published.
  - `postfiat_node_sha256` identifies the exact node binary used. It is not
    expected to reproduce across builds; the source, configuration and artifact
    hashes are.
- **Exit code:** 0.

**Deployers.** Three disposable keys: the token's deployer
`0x7d0a54ce42d205a35d7b309c913bf3f7b07b6907`, the verifier's deployer
`0x18275b842323245deb060915569498fb03f44926` and the lock's deployer
`0x9df9133325ceed11952bc0075b96fa9c17a0940e`. A fourth key holds the token's
supply.

| | Predicted | Deployed |
| --- | --- | --- |
| Verifier (`PFTLFinalityVerifierV1`), verifier deployer nonce 0 | `0x56c2a3a7dde8d8740c021b54d9fe8ef398cdae2a` | same |
| Verifier runtime code hash | `0xca4f7ed49450e2712241f8f1e3ebeb467587c3d8c3995edeb02bb5cdffd2158f` | same |
| Lock (`PFTLockV1`), lock deployer nonce 0 | `0xad0bb02f04f9bdde7afb68109d31822a37eb15ff` | same |
| Lock runtime code hash | `0x32b473cebd6e9fb2583fcc01450fa512551d572849edaa739bffd82726fc2040` | same |

**How the code hashes were predicted.** Both contracts were deployed from their
deployers at the planned nonces on an `evm_snapshot`, their `eth_getCode` was
hashed, and the snapshot was reverted. The lock was hashed first, with a probe
verifier at the predicted address. The verifier was then hashed with that lock
code hash.

**Genesis.** The lane genesis pins the predicted lock address and code hash.
`postfiat-node init --genesis-file` validated it, and the pins read back from
the node's data directory equal the deployed lock. The lock's `maxLockAmount`,
6,000,000 atoms, equals the lane's per-window release cap (300,000,000 atoms at
200 basis points); `minimumLockAmount` is the 10-atom PFTL account reserve.

**On chain.** These checks held:

- every constructor value, read back through its getter, equals the value the
  script passed: the 17 fields of the verifier's `Config` and the lock's
  `token`, `finalityVerifier`, `maxLockAmount` and `minimumLockAmount`;
- one `approve` and one `lock` of 10 atoms from the holder to
  `pf500c2bfeb17a2797d9a8726cbc3a162763309ba6` left `nextNonce()` at 1,
  `totalLocked()` and the lock's token balance at 10, and `lockRecord(0)` with
  the holder, the amount and the recipient's hash; `token.decimals()` is 6.

**Nonce guard.** Immediately before the verifier both deployers' `latest` and
`pending` nonces were their planned 0, and before the lock the lock deployer's
were. Each `forge create` pinned its nonce with `--nonce`.

**Nonce off by one, per deployer.** A stray transaction on the verifier
deployer moved it to nonce 1, so the verifier would have landed at
`0xcb473560ecf0b2ab83d58ffec0ba9be600d7cca0`; the lock's predicted address
stayed intact. A stray transaction on the lock deployer moved the lock to
`0x4874fea3be232f2acc99d0abe22abb7c6fc3238e`; the verifier's predicted address
stayed intact. Both times the guard refused the pins as a whole before any
deployment.

**Limits.** The verifier's checkpoint, genesis-hash commitment, route profile
and asset id are fixtures from `crates/ethereum-contracts/test/PFUSDCTier4.t.sol`.
In production the first finalized PFTL checkpoint and the Base route profile
provide them. The SP1 verifier is a fixture address, so no proof or `release`
ran, and no PFTL chain was run beyond `init`. The rules for a public network
(keystores, a fork simulation, a versioned SP1 verifier, a constant route epoch)
are in the script's `.md` and are not implemented here.
