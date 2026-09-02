# Arc registry proof source options

**Date:** 2026-09-02

**Status:** Research result. No live Arc transaction, signing, PFTL devnet
contact, or change to the current-v2 guest is authorized by this document.

## Result

None of the eight no-key Arc Testnet HTTP endpoints/aliases probed below served
`eth_getProof` for the exact historical block used by the existing authenticated
registry fixture. Circle's primary endpoint does not expose the method; the
three partner services expose it at the tip but rejected the requested numeric
historical block because of their proof-state windows.

The shortest credible path is therefore to qualify the already-running Vast
instance as an operator-owned Arc proof node, with an explicit Reth proof
window, while treating a paid managed archive endpoint as the fallback. A
narrow proof generator is not the first choice because an EVM header state root
and registry values are insufficient to reconstruct the required Merkle-Patricia
proof nodes.

## Exact current-v2 requirement

The governing requirement is Section 3.4 and primitive P7 of
`docs/specs/pfusdc-arc-current-devnet-zellic-readiness-spec-20260902.md`.
The consuming implementation is:

- constants and witness types in
  `programs/pfusdc-arc-ingress/src/lib.rs:13-130`;
- the deposit-header and post-state binding in
  `programs/pfusdc-arc-ingress/src/lib.rs:232-270`;
- account, storage, implementation, and output-set verification in
  `programs/pfusdc-arc-ingress/src/lib.rs:421-648`; and
- exact-block capture in
  `tools/pfusdc-tier4-prover/src/arc_ingress_capture.rs:606-724`.

### Address and account proofs

The proof is for Arc Testnet chain ID `5042002` and MUST include:

1. an account proof for validator-registry proxy
   `0x3600000000000000000000000000000000000002` against the finalized
   header's `stateRoot`;
2. that account's proxy code hash
   `0x4df0ba7cf2eea00b109c6e96a21da38b43b7c9d107a94ff017a24e3409c78c2f`;
3. all storage proofs below against the proven proxy account's `storageRoot`;
4. an account proof, at the same `stateRoot`, for the implementation address
   read from the ERC-1967 slot; and
5. implementation code hash
   `0xb04771f96d0e33612a9ebb87eb7eb5ae07adbf4a7e6b5e44f362e5a9d5c67313`.

The implementation address is not trusted as a capture-time constant. The
historical and current value observed in the ERC-1967 proof is
`0xfcc314dd5ad756c6bba725617438c0d25450a0de`, but the guest derives it from
the proven slot and then checks the pinned implementation code hash.

### Required proxy storage slots

Let:

```text
BASE = 0xb58da0dce03316992faea3e12c60705b8ac05a309e27e3bc8421e5b271c9d200
ERC1967_IMPLEMENTATION =
  0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc
ACTIVE_SET = BASE + 1 =
  0xb58da0dce03316992faea3e12c60705b8ac05a309e27e3bc8421e5b271c9d201
ACTIVE_VALUES = keccak256(ACTIVE_SET)
```

The required keys and values are:

| Key | Required meaning/value |
| --- | --- |
| `ERC1967_IMPLEMENTATION` | zero-padded implementation address |
| `ACTIVE_SET` | exact active-array length `N` |
| `ACTIVE_VALUES + i`, for every `0 <= i < N` | nonzero registration ID in registry order |
| `V = keccak256(pad32(registration_id) || BASE)` | validator status word `2` (`Active`) |
| `V + 1` | Solidity dynamic-bytes word `65` for the 32-byte Ed25519 public key |
| `keccak256(V + 1)` | exact 32-byte Ed25519 public key |
| `V + 2` | exact `u64` voting power |

Thus a set of `N` active registrations requires exactly `2 + 5N` distinct
storage proofs, plus the proxy and implementation account proofs. The guest
derives each validator address from the public key, rejects duplicate IDs or
addresses, removes zero-power entries, sorts by descending voting power then
ascending address, and requires the result and its commitment to equal the
declared output set.

`eth_call(getActiveValidatorSet())` is used by the host only to discover the
candidate set and therefore the keys to request. It is not authenticated state
and cannot replace any proof above.

### Exact block anchor

For a deposit in finalized Arc block `H`:

- the commit certificate authenticates the exact canonical block hash and RLP
  header at `H`;
- the signing set for that certificate is registry state queried at `H-1`;
- the EIP-1186 proxy, storage, and implementation proofs MUST all open under
  the `stateRoot` in the authenticated header at `H`; and
- that post-block state determines the signing set for `H+1` and
  `validator_set_commitment_out`.

The proof is mandatory even when the `H` and `H+1` validator sets are equal.
A proof from another height, a `latest` proof whose block is not identified, or
an `eth_call` result cannot be substituted.

## Public endpoint probe evidence

### Method

Probe interval: `2026-09-02T10:18Z` through `2026-09-02T10:25Z`.

The exact historical target was the repository's authenticated no-change
fixture at height `59,374,844` (`0x389fcfc`): block hash
`0x4984bdccd18d238bd640ce6e0c0a1bd93e04707af6b399f8e1a609263173f34b`
and state root
`0x9afe6f9c0c5b72740d787af46a5f12bd4099bc557911ca344a1d9b4fcb91d421`.
Circle's primary endpoint returned that header and state root, so the block
history itself remained readable.

This minimal support probe requests the registry account proof and the
ERC-1967 storage proof. Success would establish method availability at an exact
numeric historical block, after which the full `2 + 5N` request and separate
implementation-account request would still have to pass native P7
verification.

Verbatim request body used for every endpoint:

```json
{"jsonrpc":"2.0","id":1186,"method":"eth_getProof","params":["0x3600000000000000000000000000000000000002",["0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"],"0x389fcfc"]}
```

The `.network` names are the aliases configured in this repository and its
August evidence. Arc's current RPC documentation publishes the corresponding
`.io` names. All eight returned `eth_chainId = 0x4cef52` before the proof call.

| Endpoint | Source | Verbatim `eth_chainId` response | Verbatim `eth_getProof` response |
| --- | --- | --- | --- |
| `https://rpc.testnet.arc.network` | repository-configured alias | `{"jsonrpc":"2.0","id":1,"result":"0x4cef52"}` | `{"jsonrpc":"2.0","id":1186,"error":{"code":-32601,"message":"method not supported"}}` |
| `https://rpc.blockdaemon.testnet.arc.network` | repository-era provider alias | `{"jsonrpc":"2.0","id":1,"result":"0x4cef52"}` | `{"jsonrpc":"2.0","id":1186,"error":{"code":-32602,"message":"distance to target block exceeds maximum proof window"}}` |
| `https://rpc.drpc.testnet.arc.network` | repository-era provider alias | `{"id":1,"jsonrpc":"2.0","result":"0x4cef52"}` | `{"id":1186,"jsonrpc":"2.0","error":{"message":"Unknown state. First available state is 1","code":27}}` |
| `https://rpc.quicknode.testnet.arc.network` | repository-era provider alias | `{"jsonrpc":"2.0","id":1,"result":"0x4cef52"}` | `{"jsonrpc":"2.0","id":1186,"error":{"code":-32614,"message":"eth_getProof is limited to a 10,000 range"}}` |
| `https://rpc.testnet.arc.io` | current Arc primary | `{"jsonrpc":"2.0","id":1,"result":"0x4cef52"}` | `{"jsonrpc":"2.0","id":1186,"error":{"code":-32601,"message":"method not supported"}}` |
| `https://rpc.blockdaemon.testnet.arc.io` | current Arc provider list | `{"jsonrpc":"2.0","id":1,"result":"0x4cef52"}` | `{"jsonrpc":"2.0","id":1186,"error":{"code":-32602,"message":"distance to target block exceeds maximum proof window"}}` |
| `https://rpc.drpc.testnet.arc.io` | current Arc provider list | `{"id":1,"jsonrpc":"2.0","result":"0x4cef52"}` | `{"id":1186,"jsonrpc":"2.0","error":{"message":"Unknown state. First available state is 1","code":27}}` |
| `https://rpc.quicknode.testnet.arc.io` | current Arc provider list | `{"jsonrpc":"2.0","id":1,"result":"0x4cef52"}` | `{"jsonrpc":"2.0","id":1186,"error":{"code":-32614,"message":"eth_getProof is limited to a 10,000 range"}}` |

Control calls with the nonnumeric `latest` tag returned structurally complete
EIP-1186 proofs from Blockdaemon, dRPC, and QuickNode, including the pinned
proxy code hash and current implementation slot. Circle's primary continued to
return `-32601`. Exact numeric calls only two blocks behind the reported head
were rejected by the three partner endpoints, while a Blockdaemon call for the
exact reported head succeeded; this tip-only/racy behavior does not meet the
deposit-block requirement.

No public no-key endpoint is therefore qualified as the source. QuickNode's
product documentation says Arc Testnet is archive/unpruned, and Alchemy
advertises archive data on private keyed Arc endpoints, but neither claim was
accepted as EIP-1186 evidence: no private endpoint or API credential was in
scope, and exact-block support must be probed directly.

## Options

Effort estimates below are engineering estimates, not provider quotes.

### Option 1 — Managed exact-block archive RPC

Acquire a private Arc endpoint from a provider that contractually supports
`eth_getProof` for a numeric block tag and a proof window longer than the
capture/retry period. Qualify it with one complete current P7 request, the
derived implementation-account proof, local verification against the exact
header `stateRoot`, and at least one wrong-root/slot negative.

| Dimension | Assessment |
| --- | --- |
| Current evidence | The no-key endpoints fail. QuickNode documents Arc as archive/unpruned and Alchemy advertises archive data, but exact-block EIP-1186 is unverified on their private tiers. |
| Effort | Approximately 0.5–2 engineer-days after credentials/procurement, including full P7 and negative qualification. Provider sales/provisioning time is unknown. |
| Cash cost | Unknown; no current quote was obtained. |
| Trust | No new safety authority if the guest verifies the proof against the certificate-authenticated header root. The provider remains a liveness/retention dependency and can withhold or delay a proof. |
| Main risk | “Archive” may mean blocks/receipts/traces, not a usable Reth historical `eth_getProof` window. Only the full exact-block probe closes this. |

**Disposition:** valid fallback, not qualified today.

### Option 2 — Operator-run Arc archive/proof node

Arc publicly supports permissionless follow nodes. The documented software is
Arc Testnet v0.8.0 with `arc-node-execution` (Reth-based),
`arc-node-consensus` (Malachite-based), and `arc-snapshots`. Genesis sync is not
supported; bootstrap uses matching EL/CL snapshots. The current snapshot tool
has an explicit `--el-profile=archive` mode, and archive operation means
starting the EL with neither `--full` nor `--minimal`. Any one-time
reconciliation needed for the selected snapshot/preset must follow the matching
Arc release documentation rather than mixing profiles.

Reth separately bounds historical proof generation with
`--rpc.eth-proof-window`. Archive data alone is not sufficient: the node MUST
set a window comfortably larger than the time between the Arc deposit and
witness capture, enable `eth` RPC, and prove empirically that the chosen Arc
build serves the complete exact-block request. The configured window, oldest
served block, and full P7 result belong in the operator receipt.

| Dimension | Assessment |
| --- | --- |
| Public support | Yes: Arc documents anyone running a node, an archive snapshot profile, and unpruned operation. Exact-block `eth_getProof` behavior on Arc v0.8.0 remains an empirical gate. |
| Published minimum | Linux, high-clock CPU, 64 GB+ RAM, 1 TB+ TLC NVMe, stable 24 Mbps+. |
| Published bootstrap | Default documented snapshots are about 68 GB EL + 16 GB CL compressed and 103 GB + 36 GB extracted; 10–15 minutes at 100 Mbps. The current archive-profile size, extraction footprint, catch-up duration, and steady growth rate are not published and remain unknown. |
| Effort | Approximately 0.5–2 engineer-days if the existing instance is already synced and only needs proof-window/P7 qualification; approximately 2–5 days if it must be rebuilt from an archive-profile snapshot. Snapshot or client defects can extend this. |
| Trust | The local CL and EL independently verify/follow Arc, but the bridge still trusts only the guest-verified certificate, header, and trie proofs. Node failure is a liveness risk, not permission to assert state. |
| Main risk | Arc is alpha software; snapshot bootstrap, archive size, Reth historical-proof memory, and Arc-specific `eth_getProof` behavior are unqualified. |

Operator-supplied fact: a Vast instance labeled
`arc-archive-proof-20260902` was started on 2026-09-02 at approximately
`$0.28/hour`. It most likely represents prior provisioning of this option and
may already be in progress. At that rate, continuous compute is approximately
`$6.72/day` or `$201.60/30 days`, excluding storage/egress or provider pricing.
This research did not touch, probe, or connect to the instance, so its software,
sync height, retention profile, proof window, and health are unknown.

**Disposition:** recommended primary path because capacity may already be
running, subject to a full exact-block P7 qualification before any deposit.

### Option 3 — Narrow proof-generation/capture path

An EVM `stateRoot` is a commitment, not enough data to synthesize an account or
storage proof. Tracking only the registry values is also insufficient: the
proxy account proof contains global state-trie sibling nodes that can change
when unrelated accounts change, and each storage proof contains registry
storage-trie sibling nodes that can change when other registry keys change.
Raw headers, blocks, receipts, logs, `eth_call`, and `eth_getStorageAt` do not
provide those nodes.

A safe “narrow” implementation therefore needs one of these data planes:

1. a continuously captured EIP-1186 proof for every finalized head, immediately
   verified against a matching exact header and retained until the capture tool
   consumes it; or
2. an Arc/Reth execution database with changesets and trie material, using
   Reth's state-proof APIs to generate only the proxy, required slots, and
   implementation proofs.

The first is a fragile recorder around the partner endpoints, not independent
proof generation. Their observed tip-only behavior creates races between head
selection and proof retrieval and gives no backfill after a missed block. The
second still has to execute and retain enough global state/history to reproduce
the exact root; it is essentially Option 2 with a smaller proof API and perhaps
a bounded window. Reconstructing from public raw blocks alone requires
bootstrap state plus deterministic execution of every intervening transaction,
which is node implementation work rather than a registry-only witness.

| Dimension | Assessment |
| --- | --- |
| Effort | Approximately 3–5 engineer-days for a thin capture/Reth proof sidecar after a working local Arc node exists; at least 1–2 weeks for an independently engineered block-execution/trie witness path, with high uncertainty. |
| Cash cost | Mostly engineering plus a node/state host; it does not eliminate the EL storage/compute dependency. |
| Trust | No new safety authority if every emitted proof is verified against the authenticated exact header root. A buggy generator should fail verification, but can cause missed deposits or capture outages. |
| Main risk | Underestimating the global trie/state dependency and accidentally treating values, logs, or calls as proofs. |

**Disposition:** reserve fallback if the Arc node's standard Reth proof path
cannot be configured reliably; do not build it before qualifying the existing
instance.

## Recommendation

Assign an owner immediately to qualify Option 2 on the already-running Vast
instance: confirm archive-profile data, set/record an adequate
`--rpc.eth-proof-window`, and make the existing capture tool pass the complete
P7 proof plus negatives at a numeric historical block. In parallel, request
one private managed endpoint trial as a time-bounded fallback; do not start a
live deposit and do not implement Option 3 unless the local standard proof path
fails with a recorded cause.

The go/no-go criterion is not “node synced” or “`eth_getProof` returned JSON.”
It is native verification of the full proxy account, all `2 + 5N` slots, the
derived implementation account/code hash, and the resulting validator set
under the exact finalized header `stateRoot`.

The operator MUST record this one-line decision before execution:

```text
REGISTRY_PROOF_SOURCE_DECISION owner=<person-or-team>; deadline=<YYYY-MM-DDTHH:MM:SSZ>; chosen_option=<managed-archive-rpc|vast-arc-archive-proof-20260902|narrow-proof-generator>
```

## Public references

- [Arc RPC endpoints](https://docs.arc.io/arc/references/rpc-endpoints)
- [Arc node requirements](https://docs.arc.io/arc/references/node-requirements)
- [Running an Arc node](https://github.com/circlefin/arc-node/blob/main/docs/running-an-arc-node.md)
- [Arc snapshot profiles](https://github.com/circlefin/arc-node/blob/main/crates/snapshots/README.md)
- [Reth RPC proof-window configuration](https://reth.rs/docs/reth/core/args/struct.RpcServerArgs.html)
- [QuickNode Arc API overview](https://www.quicknode.com/docs/arc/api-overview)
- [Alchemy Arc Testnet endpoints](https://www.alchemy.com/rpc/arc-testnet)
- [EIP-1186](https://eips.ethereum.org/EIPS/eip-1186)
